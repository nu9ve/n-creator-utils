"""Deferred work, restoration and portable editor handoffs; never edits an editor's project."""
import json
import os
from pathlib import Path
import uuid

from .volumes import independent
from .common import Tree, canonical, content_stamp, full_sync, now, require, uid
from .layout import MEDIA_KINDS, kind_of
from .planner import destination_path
from .transfer import Transfer, claim_directory


def asset_record(catalog, asset_id):
    row = catalog.db.execute('SELECT payload FROM assets WHERE id=?', (asset_id,)).fetchone()
    require(row is not None, 'unknown_asset', 'Asset not found')
    return json.loads(row[0])


def set_work_state(catalog, asset_ids, state):
    require(state in {'pending', 'skipped'}, 'invalid_state', 'Choose pending or skipped')
    with catalog.writer():
        for asset_id in asset_ids:
            asset_record(catalog, asset_id)
            with catalog.db:
                catalog.db.execute('UPDATE work SET state=?,updated_at=? WHERE asset_id=?', (state, now(), asset_id))
            catalog.event(asset_record(catalog, asset_id)['job_id'], 'work_' + state, {'asset_id': asset_id})
    return {'updated': asset_ids, 'state': state}


def deliver_work(catalog, backend, asset_ids, work, cancel=None):
    require(asset_ids, 'empty_selection', 'Select assets to deliver')
    record = catalog.volume(work)
    require(record['role'] == 'work', 'invalid_role', 'Work volume required')
    engine = Transfer(catalog, backend, cancel)
    delivered = []
    with catalog.writer():
        for asset_id in asset_ids:
            asset = asset_record(catalog, asset_id)
            plan = catalog.job(asset['job_id'])['plan']
            archive = catalog.volume(plan['archive_id'])
            archive_info, work_info = backend.check(archive), backend.check(record)
            require(independent(archive_info, work_info), 'same_physical_disk', 'Work and archive must be separate physical disks')
            files = [f for f in plan['files'] if f['unit_id'] == asset_id]
            needed = sum(f['stamp']['size'] for f in files)
            require(work_info['free'] >= needed + 64 * 1024 * 1024, 'insufficient_space', 'Insufficient work space')
            copies = []
            with Tree(archive['root']) as source, Tree(record['root']) as target:
                files = [f for f in files if destination_path(record, plan, asset, f['path'])]
                override = catalog.get('layout_override:' + plan['id'], {})
                if 'layout' not in plan:
                    first_dest = destination_path(record, plan, asset, files[0]['path'])
                    base = first_dest[:first_dest.index('/' + plan['id'] + '/')] + '/' + plan['id']
                    claim_directory(target, base, {'job_id': plan['id'], 'digest': plan['digest'], 'volume_id': record['id']})
                for file in files:
                    prior = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                               (plan['id'], archive['id'], file['path'])).fetchone()
                    require(prior is not None, 'missing_receipt', 'Archive copy is not verified')
                    prior = json.loads(prior[0])
                    dest = destination_path(record, plan, asset, file['path'])
                    if override.get(file['path'], {}).get('work'):
                        dest = record['prefix'] + '/' + override[file['path']]['work']  # Bundle renamed in place after ingest.
                    sha, stamp = engine.copy_file(source, prior['destination'], target, dest, record, expected=prior['sha256'])
                    copy = {'job_id': plan['id'], 'volume_id': record['id'], 'path': file['path'], 'destination': dest,
                            'sha256': sha, 'bytes': file['stamp']['size'], 'verified_at': now(), 'stamp': stamp, 'receipt_id': str(uuid.uuid5(uuid.NAMESPACE_URL, plan['id'] + ':' + record['id'] + ':' + asset_id))}
                    with catalog.db:
                        catalog.db.execute('INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)',
                                           (plan['id'], record['id'], file['path'], sha, canonical(copy).decode()))
                    copies.append(copy)
                target.write_json('.ncreator/receipts/' + plan['id'] + '-' + asset_id + '.json',
                                  {'schema': 'nu9ve-work-receipt/1', 'receipt_id': copies[0]['receipt_id'], 'asset_id': asset_id, 'job_id': plan['id'], 'files': copies, 'verified_at': now()})
            with catalog.db:
                catalog.db.execute('UPDATE work SET state=?,target=?,updated_at=? WHERE asset_id=?', ('ready', record['id'], now(), asset_id))
            catalog.event(plan['id'], 'work_ready', {'asset_id': asset_id, 'volume_id': record['id']})
            delivered.append(asset_id)
    return {'delivered': delivered, 'volume_id': record['id']}


def resolve(catalog, backend, asset_id):
    asset = asset_record(catalog, asset_id)
    locations = []
    for volume in catalog.volumes():
        rows = [json.loads(row[0]) for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'], volume['id']))]
        selected = [row for row in rows if row['path'] in asset['paths']]
        if len(selected) != len(asset['paths']):
            continue
        online = backend.online(volume)
        locations.append({'volume_id': volume['id'], 'volume': volume['name'], 'role': volume['role'],
                          'online': bool(online), 'files': [{'path': row['path'], 'local_path': str(Path(volume['root']) / row['destination']),
                          'sha256': row['sha256']} for row in selected]})
    locations.sort(key=lambda l: (not l['online'], l['role'] != 'work'))
    return {'asset': asset, 'locations': locations}


def handoff(catalog, backend, asset_ids):
    require(asset_ids and len(set(asset_ids)) == len(asset_ids), 'invalid_selection', 'Select distinct assets')
    assets = [asset_record(catalog, value) for value in asset_ids]
    require(len({(a['project_id'], a['profile']) for a in assets}) == 1, 'mixed_project', 'Handoff must be one project/profile')
    project, profile = assets[0]['project'], assets[0]['profile']
    result = {'schema': 'creator-media-handoff/1', 'handoff_id': uid(), 'created_at': now(),
              'project_id': assets[0]['project_id'], 'profile_id': profile, 'assets': []}
    for order, asset in enumerate(assets):
        plan = catalog.job(asset['job_id'])['plan']
        locations, member_list = [], None
        for record in catalog.volumes():
            rows = [json.loads(row[0]) for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (plan['id'], record['id']))]
            copies = {r['path']: r for r in rows if r['path'] in asset['paths']}
            if 'layout' in plan:
                # Named layouts: each role holds its own subset (no Sony XML on work); hand off editable media only.
                expected = {p for p in asset['paths'] if destination_path(record, plan, asset, p)}
                if not expected or set(copies) != expected:
                    continue
                if asset['kind'] == 'stalker_session':
                    prefix = '' if asset['bundle'] == '.' else asset['bundle'] + '/'
                    sample = next(iter(copies.values()))
                    location_path = sample['destination'][:-len(sample['path'][len(prefix):]) - 1]
                    members = [{'relative_path': row['destination'][len(location_path) + 1:], 'bytes': row['bytes'], 'sha256': row['sha256']}
                               for _, row in sorted(copies.items())]
                else:
                    media = [row for _, row in sorted(copies.items()) if kind_of(row['path']) in MEDIA_KINDS] or [row for _, row in sorted(copies.items())]
                    location_path = media[0]['destination']
                    members = [{'relative_path': Path(row['destination']).name, 'bytes': row['bytes'], 'sha256': row['sha256']} for row in media]
                    sample = media[0]
            elif set(copies) != set(asset['paths']):
                continue
            elif asset['kind'] == 'file':
                sample = next(iter(copies.values()))
                location_path = sample['destination']
                members = [{'relative_path': Path(location_path).name, 'bytes': sample['bytes'], 'sha256': sample['sha256']}]
            else:
                # Schema-1 unit-exclusive container; bundle root is preserved beneath it.
                sample = next(iter(copies.values()))
                marker = '/' + asset['id'] + '/'
                base = sample['destination'].split(marker)[0] + marker + plan['source_name']
                if asset['bundle'] not in {None, '.'}:
                    base += '/' + asset['bundle']
                location_path = base
                members = [{'relative_path': row['destination'][len(base) + 1:], 'bytes': row['bytes'], 'sha256': row['sha256']}
                           for _, row in sorted(copies.items())]
            if member_list is None:
                member_list = members
            else:
                require(member_list == members, 'revision_mismatch', 'Replica membership differs')
            locations.append({'volume_id': record['id'], 'relative_path': location_path, 'role': record['role'],
                              'receipt_id': sample.get('receipt_id') or str(uuid.uuid5(uuid.NAMESPACE_URL, plan['id'] + ':' + record['id']))})
        require(locations, 'missing_receipt', 'No complete verified location')
        item = {'asset_id': asset['id'], 'content_revision': str(uuid.uuid5(uuid.NAMESPACE_URL, asset['revision'])),
                'kind': asset['kind'], 'order': order, 'members': member_list, 'locations': locations}
        if asset['session_id']:
            item['capture_session_id'] = asset['session_id']
        result['assets'].append(item)
    return result


def restore(catalog, backend, asset_id, destination):
    asset = asset_record(catalog, asset_id)
    plan = catalog.job(asset['job_id'])['plan']
    source_volume = catalog.volume(plan['archive_id'])
    backend.check(source_volume)
    root = Path(destination).expanduser().absolute()
    info = backend.describe(root)
    source_root = Path(source_volume['root'])
    require(root != source_root and source_root not in root.parents and root not in source_root.parents,
            'overlapping_roots', 'Restore outside the archive volume root')
    record = {'id': str(uuid.uuid5(uuid.NAMESPACE_URL, str(root))), 'name': 'restore', 'role': 'work', 'root': str(root), 'identity': info}
    engine = Transfer(catalog, backend)
    files = [f for f in plan['files'] if f['unit_id'] == asset_id]
    require(info['free'] >= sum(f['stamp']['size'] for f in files) + 64 * 1024 * 1024, 'insufficient_space', 'Insufficient restore space')
    base = asset['project'] + '/' + asset_id
    with catalog.writer(), Tree(source_volume['root']) as source, Tree(root) as target:
        claim_directory(target, base, {'asset_id': asset_id, 'revision': asset['revision']})
        for file in files:
            row = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                     (plan['id'], source_volume['id'], file['path'])).fetchone()
            require(row is not None, 'missing_receipt', 'Archive copy missing')
            row = json.loads(row[0])
            engine.copy_file(source, row['destination'], target, base + '/' + plan['source_name'] + '/' + file['path'], record,
                             expected=row['sha256'])
        target.write_json(base + '/.ncreator-restore.json', {'asset_id': asset_id, 'revision': asset['revision'], 'verified_at': now()})
        catalog.set('restore:' + asset_id, {'root': str(root), 'path': base, 'verified_at': now()})
    return {'asset_id': asset_id, 'path': str(root / base / plan['source_name']), 'verified': True}


def relayout(catalog, backend, job_id):
    """Rename already verified Stalker bundle folders to the dated name in place; bytes are never copied or rewritten."""
    from .layout import stalker_bundle_name
    job = catalog.job(job_id)
    plan = job['plan']
    require(job['state'] in {'completed', 'camera_action_required', 'source_retained', 'source_has_new_material'},
            'job_not_finished', 'Only finished jobs can be renamed in place')
    sessions = [u for u in plan['units'] if u['kind'] == 'stalker_session']
    require(sessions, 'nothing_to_relayout', 'This job has no Stalker sessions')
    archive = catalog.volume(plan['archive_id'])
    renamed = []
    with catalog.writer():
        rows = {}
        for row in catalog.db.execute('SELECT volume_id, payload FROM copies WHERE job_id=?', (job_id,)):
            payload = json.loads(row[1])
            rows.setdefault(payload['volume_id'], {})[payload['path']] = payload
        override = catalog.get('layout_override:' + job_id, {})
        for unit in sessions:
            prefix = '' if unit['bundle'] == '.' else unit['bundle'] + '/'
            manifest_copy = rows[archive['id']][prefix + 'manifest.json']
            backend.check(archive)
            with Tree(archive['root']) as tree:
                fd = tree.open(manifest_copy['destination'])
                with os.fdopen(fd, 'rb') as stream:
                    manifest = json.loads(stream.read(1024 * 1024))
            require(manifest.get('sessionID', '').lower() == (unit['session_id'] or '').lower(), 'invalid_bundle', 'Archived manifest is not this session')
            name, created = stalker_bundle_name(manifest, min(f['stamp']['mtime_ns'] for f in plan['files'] if f['unit_id'] == unit['id']),
                                                plan['country'], unit['project'])
            for volume_id, copies in rows.items():
                members = [copies[p] for p in unit['paths'] if p in copies]
                if not members:
                    continue
                record = catalog.volume(volume_id)
                backend.check(record)
                sample = members[0]
                old_dir = sample['destination'][:-len(sample['path'][len(prefix):]) - 1]
                parent = old_dir.rsplit('/', 1)[0]
                if record['role'] == 'archive':
                    parent = parent.rsplit('/', 2)[0] + '/' + created.strftime('%Y/%m')
                new_dir = parent + '/' + name
                with Tree(record['root']) as tree:
                    if old_dir != new_dir:
                        old_parent_rel, old_name = old_dir.rsplit('/', 1)
                        new_parent_rel, new_name = new_dir.rsplit('/', 1)
                        with tree.parent(old_parent_rel + '/x') as (src_fd, _), tree.parent(new_parent_rel + '/x', True) as (dst_fd, _):
                            exists = lambda fd, entry: entry in os.listdir(fd)
                            if exists(src_fd, old_name):
                                require(not exists(dst_fd, new_name), 'destination_conflict', 'Target bundle name already exists: ' + new_dir)
                                os.rename(old_name, new_name, src_dir_fd=src_fd, dst_dir_fd=dst_fd)
                                full_sync(src_fd); full_sync(dst_fd)
                            else:
                                # Idempotent re-run after a crash between the rename and the catalog update.
                                require(exists(dst_fd, new_name), 'missing_copy', 'Neither old nor new bundle folder exists: ' + old_dir)
                    for member in members:
                        destination = new_dir + member['destination'][len(old_dir):]
                        st = tree.stat(destination)
                        require(st.st_size == member['bytes'], 'checksum_mismatch', 'Renamed member size differs: ' + destination)
                        member.update(destination=destination, stamp=content_stamp(st))
                        with catalog.db:
                            catalog.db.execute('UPDATE copies SET payload=? WHERE job_id=? AND volume_id=? AND relative_path=?',
                                               (canonical(member).decode(), job_id, volume_id, member['path']))
                        override.setdefault(member['path'], {})[record['role']] = destination[len(record['prefix']) + 1:]
                renamed.append({'session_id': unit['session_id'], 'volume_id': volume_id, 'from': old_dir, 'to': new_dir})
        # Portable receipts must name the new folders, or recovery would look for the old ones.
        for volume_id, copies in rows.items():
            record = catalog.volume(volume_id)
            with Tree(record['root']) as tree:
                for receipt_name in ['.ncreator/receipts/' + job_id + '.json']:
                    try:
                        fd = tree.open(receipt_name)
                    except FileNotFoundError:
                        continue
                    with os.fdopen(fd, 'rb') as stream:
                        receipt = json.loads(stream.read(16 * 1024 * 1024))
                    for item in receipt['files']:
                        if item['path'] in copies:
                            item['destination'], item['stamp'] = copies[item['path']]['destination'], copies[item['path']]['stamp']
                    receipt['relayout'] = {'at': now(), 'renamed': [r for r in renamed if r['volume_id'] == volume_id]}
                    tree.write_json(receipt_name, receipt)
        catalog.set('layout_override:' + job_id, override)
        catalog.event(job_id, 'relayout', {'renamed': renamed})
    return {'job_id': job_id, 'renamed': renamed}
