"""Verified, resumable transfers. Source cleanup is gated by durable receipts."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import uuid

from .common import CHUNK_SIZE, MediaError, Tree, canonical, content_stamp, digest, hash_fd, now, require
from .planner import destination_path, preflight, scan, validate_plan


def claim_directory(tree, rel, owner):
    with tree.parent(rel, True) as (fd, name):
        try:
            os.mkdir(name, 0o700, dir_fd=fd)
            os.fsync(fd)
            created = True
        except FileExistsError:
            created = False
    marker = rel + '/.ncreator-owner.json'
    if created:
        tree.write_json(marker, owner)
    else:
        fd = tree.open(marker)
        with os.fdopen(fd, 'rb') as stream:
            actual = json.loads(stream.read(4096))
        require(actual == owner, 'destination_conflict', 'Destination is owned by another operation')


class Transfer:
    def __init__(self, catalog, backend, cancel=None, hook=None):
        self.catalog, self.backend = catalog, backend
        self.cancel = cancel or (lambda: False)
        self.hook = hook or (lambda *args: None)
        self.volume_tokens = {}
        self.source_token = None

    def check_volume(self, record, full=False):
        if full or record['id'] not in self.volume_tokens:
            self.volume_tokens[record['id']] = self.backend.check(record)
        else:
            self.backend.fast_check(record['root'], self.volume_tokens[record['id']])
        return self.volume_tokens[record['id']]

    def checkpoint(self, stage, payload):
        require(not self.cancel(), 'cancelled', 'Operation cancelled; original retained unless verified cleanup already began')
        self.hook(stage, payload)

    def source_check(self, plan, full=False):
        if self.source_token is not None and not full:
            info = self.backend.fast_check(plan['source'], self.source_token)
        else:
            info = self.backend.describe(plan['source'], external_required=plan['cleanup'])
            self.source_token = info
        require(all(info.get(k) == v for k, v in plan['source_identity'].items()), 'source_replaced', 'Source volume/directory identity changed')
        require(not plan['cleanup'] or info['external'] and Path(info['mount']) == Path(plan['source']),
                'card_root_required', 'Cleanup requires the registered card root identity')

    def validate_source(self, plan):
        self.source_check(plan, full=True)
        actual, bundles, skipped = scan(plan['source'], skip_active=plan['source_kind'] == 'stalker', cancel=self.cancel)
        actual_map = {f['path']: f for f in actual}
        expected = {f['path']: f for f in plan['files']}
        if plan['source_kind'] == 'stalker':
            # New recordings outside a completed bundle do not invalidate it.
            for key, bundle in plan['bundles'].items():
                require(bundles.get(key) == bundle, 'source_changed', 'Stalker manifest changed since planning')
                prefix = key + '/' if key != '.' else ''
                require({p for p in actual_map if p.startswith(prefix)} == {p for p in expected if p.startswith(prefix)},
                        'source_changed', 'Stalker bundle membership changed')
        else:
            removed = {row['relative_path'] for row in self.catalog.db.execute(
                'SELECT relative_path FROM members WHERE job_id=? AND cleaned IN (1,-1)', (plan['id'],))}
            require(set(actual_map) <= set(expected) and set(expected) - set(actual_map) <= removed,
                    'source_changed', 'Source membership changed; new/missing files require review')
        for path, file in expected.items():
            if path in actual_map:
                require(actual_map[path]['stamp'] == file['stamp'], 'source_changed', 'Source changed: ' + path)
        return actual_map

    def copy_file(self, source_tree, source_rel, target_tree, target_rel, record, expected=None, source_stamp=None, owned_partial=False):
        self.check_volume(record)
        source_tree.check(); target_tree.check()
        src = source_tree.open(source_rel)
        try:
            initial = os.fstat(src)
            if source_stamp is not None:
                require(content_stamp(initial) == source_stamp, 'source_changed', 'Source file changed before reading')
            # A prior partial belongs to this job's exclusively claimed directory.
            try:
                existing = target_tree.open(target_rel)
            except FileNotFoundError:
                existing = None
            if existing is not None:
                try:
                    existing_hash = hash_fd(existing, self.cancel)
                    if expected and existing_hash == expected:
                        return expected, content_stamp(os.fstat(existing))
                    require(owned_partial, 'destination_conflict', 'Existing destination differs from verified content')
                finally:
                    os.close(existing)
                target_tree.unlink(target_rel)
            dst = target_tree.open(target_rel, os.O_WRONLY | os.O_CREAT | os.O_EXCL, True)
            h = hashlib.sha256()
            try:
                while True:
                    self.checkpoint('copy_block', {'path': source_rel})
                    block = os.read(src, CHUNK_SIZE)
                    if not block:
                        break
                    h.update(block)
                    view = memoryview(block)
                    while view:
                        written = os.write(dst, view)
                        require(written > 0, 'write_failed', 'Short write without progress')
                        view = view[written:]
                os.fsync(dst)
                os.utime(dst, ns=(initial.st_atime_ns, initial.st_mtime_ns))
                os.fsync(dst)
            finally:
                os.close(dst)
            with target_tree.parent(target_rel) as (parent, _):
                os.fsync(parent)
            require(content_stamp(os.fstat(src)) == content_stamp(initial), 'source_changed', 'Source changed during copy')
            sha = h.hexdigest()
            require(expected is None or sha == expected, 'source_changed', 'Copy source differs from recorded content')
            self.checkpoint('before_verify', {'path': target_rel, 'root': str(target_tree.root)})
            verified = target_tree.open(target_rel)
            try:
                target_hash = hash_fd(verified, self.cancel)
                stamp = content_stamp(os.fstat(verified))
            finally:
                os.close(verified)
            require(target_hash == sha and stamp['size'] == initial.st_size, 'checksum_mismatch', 'Destination verification failed: ' + target_rel)
            self.check_volume(record); target_tree.check()
            return sha, stamp
        finally:
            os.close(src)

    def apply(self, plan, expected_digest):
        validate_plan(plan)
        require(expected_digest == plan['digest'], 'plan_changed', 'Apply requires the reviewed plan digest')
        with self.catalog.writer():
            existing = self.catalog.db.execute('SELECT digest,state FROM jobs WHERE id=?', (plan['id'],)).fetchone()
            require(existing is None or existing['digest'] == expected_digest, 'job_conflict', 'Job ID reused with another plan')
            if existing and existing['state'] in {'completed', 'camera_action_required', 'source_retained'}:
                # Returning a prior receipt does not repeat destructive actions.
                return self.catalog.job(plan['id'])
            preflight(self.catalog, self.backend, plan, reserve=existing is None)
            if existing is None:
                with self.catalog.db:
                    self.catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',
                        (plan['id'], plan['digest'], canonical(plan).decode(), 'planned', None, now(), now(),))
            try:
                return self._run(plan)
            except BaseException as error:
                code = error.code if isinstance(error, MediaError) else type(error).__name__
                self.catalog.state(plan['id'], 'paused' if code in {'cancelled', 'KeyboardInterrupt'} else 'failed', code)
                raise

    def _run(self, plan):
        self.validate_source(plan)
        units = {u['id']: u for u in plan['units']}
        records = [self.catalog.volume(plan['work_id'])] if plan['work_id'] else []
        records.append(self.catalog.volume(plan['archive_id']))
        verified_in_run = []
        self.catalog.state(plan['id'], 'copying')
        with Tree(plan['source']) as source_tree:
            for record in records:
                self.check_volume(record, full=True)
                receipts = []
                with Tree(record['root']) as target:
                    claimed = set()
                    for file in plan['files']:
                        self.checkpoint('before_file', file)
                        unit = units[file['unit_id']]
                        dest = destination_path(record, plan, unit, file['path'])
                        # Exclusive per-job/project container, distinct from original directory tree.
                        base = dest[:dest.index('/' + plan['id'] + '/')] + '/' + plan['id']
                        if base not in claimed:
                            claim_directory(target, base, {'job_id': plan['id'], 'digest': plan['digest'], 'volume_id': record['id']})
                            claimed.add(base)
                        member = self.catalog.db.execute('SELECT * FROM members WHERE job_id=? AND relative_path=?', (plan['id'], file['path'])).fetchone()
                        prior = self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                                       (plan['id'], record['id'], file['path'])).fetchone()
                        expected = member['sha256'] if member else None
                        if prior:
                            data = json.loads(prior[0])
                            fd = target.open(dest)
                            try:
                                require(hash_fd(fd, self.cancel) == data['sha256'], 'checksum_mismatch', 'Previously verified copy is corrupt')
                                stamp = content_stamp(os.fstat(fd))
                            finally:
                                os.close(fd)
                            sha = data['sha256']
                        else:
                            # Archive reads from verified SSD when available, not the SD twice.
                            work_copy = self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND relative_path=? LIMIT 1',
                                                               (plan['id'], file['path'])).fetchone()
                            if work_copy:
                                previous = json.loads(work_copy[0])
                                previous_volume = self.catalog.volume(previous['volume_id'])
                                self.backend.check(previous_volume)
                                with Tree(previous_volume['root']) as previous_tree:
                                    sha, stamp = self.copy_file(previous_tree, previous['destination'], target, dest, record,
                                                                expected=expected, owned_partial=True)
                            else:
                                sha, stamp = self.copy_file(source_tree, file['path'], target, dest, record,
                                                            expected=expected, source_stamp=file['stamp'], owned_partial=True)
                            require(expected is None or sha == expected, 'checksum_mismatch', 'Replica hashes differ')
                            with self.catalog.db:
                                self.catalog.db.execute('INSERT OR IGNORE INTO members(job_id,relative_path,sha256) VALUES(?,?,?)',
                                                        (plan['id'], file['path'], sha))
                        receipt = {'job_id': plan['id'], 'volume_id': record['id'], 'path': file['path'], 'destination': dest,
                                   'sha256': sha, 'bytes': file['stamp']['size'], 'verified_at': now(), 'stamp': stamp, 'receipt_id': str(uuid.uuid5(uuid.NAMESPACE_URL, plan['id'] + ':' + record['id']))}
                        with self.catalog.db:
                            self.catalog.db.execute('INSERT OR REPLACE INTO copies VALUES(?,?,?,?,?)',
                                (plan['id'], record['id'], file['path'], sha, canonical(receipt).decode()))
                        receipts.append(receipt); verified_in_run.append((record, dest, stamp, file['path']))
                        self.checkpoint('after_file_verified', receipt)
                    self.check_volume(record)
                    self.check_volume(record, full=True)
                    target.write_json('.ncreator/receipts/' + plan['id'] + '.json', {
                        'schema': 'nu9ve-transfer-receipt/1', 'receipt_id': str(uuid.uuid5(uuid.NAMESPACE_URL, plan['id'] + ':' + record['id'])),
                        'job_id': plan['id'], 'plan_digest': plan['digest'], 'volume_id': record['id'],
                        'verified_at': now(), 'files': receipts, 'units': [dict(u, project_id=self.catalog.project_for(u['profile'], u['project'])) for u in plan['units']], 'plan': plan,
                        'country': plan['country'], 'city': plan['city'], 'work_delivery_status': plan['work_delivery_status']})
            self.checkpoint('after_receipts', plan)
            self.validate_source(plan)
            self.catalog.state(plan['id'], 'backup_verified')
            self._index(plan)
            if not plan['cleanup']:
                final = 'source_retained'
            elif plan['cleanup_profile'] == 'camera_action_required' or any(u['kind'] == 'stalker_session' for u in plan['units']):
                final = 'camera_action_required'
            else:
                self.catalog.state(plan['id'], 'cleaning_source')
                self.checkpoint('before_cleanup', plan)
                self.validate_source(plan)
                # Receipts verified during this run; rehash only copies whose stamps changed.
                for record, dest, stamp, original in verified_in_run:
                    self.check_volume(record)
                    with Tree(record['root']) as target:
                        current = content_stamp(target.stat(dest))
                        if current != stamp:
                            copy = self.catalog.db.execute('SELECT sha256 FROM copies WHERE job_id=? AND volume_id=? AND json_extract(payload,\'$.destination\')=?',
                                                          (plan['id'], record['id'], dest)).fetchone()
                            fd = target.open(dest)
                            try:
                                require(copy and hash_fd(fd, self.cancel) == copy[0], 'checksum_mismatch', 'Verified destination changed before cleanup')
                            finally:
                                os.close(fd)
                per_source = {}
                for record, dest, stamp, original in verified_in_run:
                    per_source.setdefault(original, []).append((record, dest, stamp))
                for file in plan['files']:
                    self.checkpoint('cleanup_file', file)
                    self.source_check(plan)
                    for required in records:
                        self.check_volume(required)
                    for record, dest, stamp in per_source[file['path']]:
                        with Tree(record['root']) as target:
                            if content_stamp(target.stat(dest)) != stamp:
                                expected = self.catalog.db.execute('SELECT sha256 FROM members WHERE job_id=? AND relative_path=?', (plan['id'], file['path'])).fetchone()[0]
                                fd = target.open(dest)
                                try:
                                    require(hash_fd(fd, self.cancel) == expected, 'checksum_mismatch', 'Destination changed during cleanup')
                                finally:
                                    os.close(fd)
                    row = self.catalog.db.execute('SELECT cleaned FROM members WHERE job_id=? AND relative_path=?', (plan['id'], file['path'])).fetchone()
                    require(row is not None, 'missing_receipt', 'No verified member for cleanup')
                    try:
                        st = source_tree.stat(file['path'])
                    except FileNotFoundError:
                        require(row[0] in {-1, 1}, 'source_changed', 'Unexpected missing source during cleanup')
                    else:
                        require(row[0] != 1, 'source_changed', 'New file appeared at a cleaned source path')
                        require(content_stamp(st) == file['stamp'], 'source_changed', 'Source changed before cleanup')
                        with self.catalog.db:
                            self.catalog.db.execute('UPDATE members SET cleaned=-1 WHERE job_id=? AND relative_path=?', (plan['id'], file['path']))
                        source_tree.unlink(file['path'], file['stamp'])
                    with self.catalog.db:
                        self.catalog.db.execute('UPDATE members SET cleaned=1 WHERE job_id=? AND relative_path=?', (plan['id'], file['path']))
                remaining, _, _ = scan(plan['source'])
                final = 'completed' if not remaining else 'source_changed'
                require(not remaining, 'source_changed', 'Source has remaining/new material; do not mark ready')
            self.catalog.state(plan['id'], final)
        return self.catalog.job(plan['id'])

    def _index(self, plan):
        for unit in plan['units']:
            hashes = []
            for path in unit['paths']:
                row = self.catalog.db.execute('SELECT sha256 FROM members WHERE job_id=? AND relative_path=?', (plan['id'], path)).fetchone()
                require(row is not None, 'missing_receipt', 'Missing member receipt')
                hashes.append({'path': path, 'sha256': row[0]})
            project_id = self.catalog.project_for(unit['profile'], unit['project'])
            asset = dict(unit, project_id=project_id, job_id=plan['id'], source=plan['source'], revision=digest(hashes), country=plan['country'], city=plan['city'])
            with self.catalog.db:
                self.catalog.db.execute('INSERT OR IGNORE INTO assets VALUES(?,?,?)', (unit['id'], plan['id'], canonical(asset).decode()))
                self.catalog.db.execute('INSERT OR IGNORE INTO asset_projects VALUES(?,?)', (unit['id'], project_id))
                self.catalog.db.execute('INSERT OR IGNORE INTO asset_jobs VALUES(?,?)', (unit['id'], plan['id']))
                if unit.get('session_id') and unit.get('capture_manifest_digest'):
                    self.catalog.db.execute('INSERT OR IGNORE INTO sessions VALUES(?,?,?)', (unit['session_id'], unit['capture_manifest_digest'], unit['id']))
                self.catalog.db.execute('INSERT OR IGNORE INTO work VALUES(?,?,?,?)',
                    (unit['id'], plan['work_delivery_status'], plan['expected_work_id'], now()))
                if plan['work_delivery_status'] == 'ready':
                    self.catalog.db.execute('UPDATE work SET state=?,target=?,updated_at=? WHERE asset_id=?', ('ready', plan['work_id'], now(), unit['id']))
