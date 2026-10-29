"""Rebuild library metadata from portable receipts, verifying every destination byte."""
import json
import os
import re
import uuid

from .common import Tree, canonical, content_stamp, digest, hash_fd, now, relative, require
from .planner import validate_plan
from .transfer import Transfer


def recover_catalog(catalog, backend, volume):
    record = catalog.volume(volume)
    require(record['role'] == 'archive', 'invalid_role', 'Recover from an archive volume')
    backend.check(record)
    restored, publications = [], []
    with catalog.writer(), Tree(record['root']) as tree:
        with tree.parent('.ncreator/receipts/entry.json') as (fd, _):
            names = sorted(os.listdir(fd))
        for name in names:
            # ExFAT gets AppleDouble `._*.json` companions from macOS; they are never receipts.
            if not name.endswith('.json') or name.startswith('.'): continue
            fd = tree.open('.ncreator/receipts/' + name)
            with os.fdopen(fd, 'rb') as stream: raw = stream.read(16 * 1024 * 1024 + 1)
            require(len(raw) <= 16 * 1024 * 1024, 'receipt_too_large', 'Receipt exceeds input limit')
            receipt = json.loads(raw)
            if receipt.get('schema') != 'nu9ve-transfer-receipt/1': continue
            require(isinstance(receipt.get('plan'), dict), 'unsupported_receipt', 'Receipt has no recoverable plan')
            original = receipt['plan']; validate_plan(original)
            require(original['id'] == receipt['job_id'] and original['digest'] == receipt['plan_digest'], 'invalid_receipt', 'Plan/receipt mismatch')
            expected = {file['path']: file for file in original['files']}
            copies = receipt['files']
            require(len(copies) == len(expected) and {c['path'] for c in copies} == set(expected), 'invalid_receipt', 'Receipt membership mismatch')
            verified = []
            for copy in copies:
                relative(copy['destination'])
                require(re.fullmatch('[a-f0-9]{64}', copy['sha256']) is not None, 'invalid_receipt', 'Invalid receipt hash')
                require(copy['bytes'] == expected[copy['path']]['stamp']['size'], 'invalid_receipt', 'Receipt size mismatch')
                fd = tree.open(copy['destination'])
                try:
                    stamp = content_stamp(os.fstat(fd))
                    require(stamp['size'] == copy['bytes'] and hash_fd(fd) == copy['sha256'], 'checksum_mismatch', 'Archive member does not match receipt')
                finally: os.close(fd)
                verified.append(dict(copy, volume_id=record['id'], receipt_id=receipt['receipt_id'], stamp=stamp, verified_at=now()))
            backend.check(record)
            for unit in receipt['units']:
                project_id = str(uuid.UUID(unit['project_id']))
                alias = catalog.db.execute('SELECT project_id FROM project_aliases WHERE profile=? AND alias=?', (unit['profile'], unit['project'])).fetchone()
                require(alias is None or alias[0] == project_id, 'project_conflict', 'Resolve conflicting project identity before recovery')
            old = catalog.db.execute('SELECT plan FROM jobs WHERE id=?', (original['id'],)).fetchone()
            if old:
                plan = json.loads(old[0])
            else:
                # Imported records cannot trigger SD cleanup and do not claim a restored work copy.
                plan = dict(original, archive_id=record['id'], work_id=None, cleanup=False, archive_only_confirmed=True,
                            work_delivery_status='pending' if original.get('expected_work_id') and original['work_delivery_status'] != 'skipped' else 'skipped')
                plan['destinations'] = {record['id']: {k: record[k] for k in ('root','prefix','role')}}
                plan.pop('digest'); plan['digest'] = digest(plan)
            with catalog.db:
                if not old:
                    catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', (plan['id'], plan['digest'], canonical(plan).decode(), 'recovered', None, now(), now()))
                for unit in receipt['units']:
                    catalog.db.execute('INSERT OR IGNORE INTO projects VALUES(?,?,?)', (unit['project_id'], unit['profile'], unit['project']))
                    catalog.db.execute('INSERT OR IGNORE INTO project_aliases VALUES(?,?,?)', (unit['profile'], unit['project'], unit['project_id']))
                for copy in verified:
                    catalog.db.execute('INSERT OR IGNORE INTO members(job_id,relative_path,sha256) VALUES(?,?,?)', (plan['id'], copy['path'], copy['sha256']))
                    catalog.db.execute('INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)', (plan['id'], record['id'], copy['path'], copy['sha256'], canonical(copy).decode()))
            if receipt.get('revision_publication'):
                publications.append((receipt['revision_publication'], plan))
            else:
                Transfer(catalog, backend)._index(plan)
            catalog.event(plan['id'], 'catalog_recovered', {'volume_id': record['id']})
            restored.append(plan['id'])
        # Index publication history only after its predecessor receipt has verified.
        # UUID filename order does not encode revision order.
        pending = list(publications)
        while pending:
            progress = False
            for publication, plan in list(pending):
                old, new = publication['previous_asset'], publication['asset']
                row = catalog.db.execute('SELECT payload FROM asset_revisions WHERE asset_id=? AND revision=?',
                                         (old['id'], old['revision'])).fetchone()
                if row is None:
                    continue
                require(json.loads(row[0])['job_id'] == old['job_id'], 'invalid_receipt', 'Predecessor job differs')
                require(new['id'] == old['id'] and new['job_id'] == plan['id']
                        and new['project_id'] == old['project_id'] and new['profile'] == old['profile']
                        and new['session_id'] == old['session_id'], 'invalid_receipt', 'Publication identity differs')
                Transfer(catalog, backend)._index(plan)
                indexed = catalog.db.execute('SELECT payload FROM asset_revisions WHERE asset_id=? AND revision=?',
                                             (new['id'], new['revision'])).fetchone()
                require(indexed is not None and json.loads(indexed[0])['job_id'] == new['job_id'],
                        'invalid_receipt', 'Publication revision differs from verified members')
                pending.remove((publication, plan)); progress = True
            require(progress, 'missing_receipt', 'Recover predecessor receipts before publishing revision history')
        predecessors = {p['previous_asset']['revision'] for p, _ in publications}
        with catalog.db:
            for publication, _ in publications:
                new = publication['asset']
                if new['revision'] not in predecessors:
                    current = json.loads(catalog.db.execute('SELECT payload FROM assets WHERE id=?', (new['id'],)).fetchone()[0])
                    if current['revision'] in predecessors:
                        catalog.db.execute('UPDATE assets SET job_id=?,payload=? WHERE id=?',
                                           (new['job_id'], canonical(new).decode(), new['id']))
                        catalog.db.execute('UPDATE work SET state=?,target=NULL,updated_at=? WHERE asset_id=?',
                                           ('pending', now(), new['id']))
    return {'recovered_jobs': restored, 'verified': True}
