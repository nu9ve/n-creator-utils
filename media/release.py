"""Explicit, catalog-recorded release plans. No deletion from a coverage summary or caller-supplied plan."""
import json
import os
from pathlib import Path
import uuid

from . import clipper
from .common import Tree, content_stamp, digest, hash_fd, now, read_json, relative, require, uid, uncached
from .coverage import walk
from .layout import MEDIA_KINDS, kind_of
from .metadata_backup import backup_metadata
from .volumes import independent


def checked_hash(tree, path, cancel=None):
    fd = tree.open(path)
    try:
        before = content_stamp(os.fstat(fd))
        value = hash_fd(uncached(fd), cancel or (lambda: False))
        require(content_stamp(os.fstat(fd)) == before, 'source_changed', 'File changed while hashing: ' + path)
        return value, before
    finally:
        os.close(fd)


def protected(path):
    return any(p.casefold() in {'clipper', 'renders', '.ncreator'} or p.lower().endswith(('.fcpbundle', '.fcpcache')) for p in Path(path).parts)


def selected_files(tree, scope, local=False, cancel=None):
    values, preserved = [], []
    for path, size in walk(tree, scope, cancel):
        require(size is not None, 'unsafe_file', 'Links/special files prevent release: ' + path)
        in_bundle = any(p.lower().endswith('.stalker') for p in Path(path).parts[:-1])
        if protected(path) or (not local and not in_bundle and kind_of(path) not in MEDIA_KINDS):
            preserved.append(path)
        else:
            values.append(path)
    return values, preserved


def bundle_root(path):
    parts = Path(path).parts
    for index, part in enumerate(parts):
        if part.lower().endswith('.stalker'):
            return '/'.join(parts[:index + 1])
    return None


def prefer_work(catalog, backend, archive, entries, replacements, cancel=None):
    """A local capture should leave Clipper on its existing verified SSD copy when possible."""
    checks = []
    archive_copies = {json.loads(row[0])['destination']: json.loads(row[0]) for row in
                      catalog.db.execute('SELECT payload FROM copies WHERE volume_id=?', (archive['id'],))}
    for original, fallback in list(replacements.items()):
        if not original.lower().endswith('.stalker'): continue
        related = [e for e in entries if str(Path(archive['root']) / e['archive_path']).startswith(fallback + '/')]
        if not related: continue
        for volume in catalog.volumes():
            if volume['role'] != 'work' or not backend.online(volume): continue
            candidate, verified = None, []
            try:
                with Tree(volume['root']) as tree:
                    for entry in related:
                        archive_copy = archive_copies.get(entry['archive_path'])
                        require(archive_copy, 'missing_receipt', 'Archive receipt missing')
                        row = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                                 (archive_copy['job_id'], volume['id'], archive_copy['path'])).fetchone()
                        require(row, 'missing_receipt', 'Work receipt missing')
                        copy = json.loads(row[0])
                        root = bundle_root(copy['destination'])
                        require(root and (candidate is None or candidate == root), 'revision_mismatch', 'Work bundle mismatch')
                        require(checked_hash(tree, copy['destination'], cancel)[0] == entry['sha256'], 'checksum_mismatch', 'Work changed')
                        candidate = root
                        verified.append({'volume_id': volume['id'], 'path': copy['destination'], 'sha256': entry['sha256']})
                if candidate:
                    replacement = str(Path(volume['root']) / candidate)
                    require(not clipper.overlaps(original, replacement), 'overlapping_roots', 'Replacement overlaps released source')
                    replacements[original] = replacement
                    checks.extend(verified)
                    break
            except (OSError, RuntimeError):
                require(not (cancel and cancel()), 'cancelled', 'Release planning cancelled')
                continue
    return checks


def verify_replacements(catalog, backend, plan, cancel=None):
    for entry in plan.get('replacement_checks', []):
        record = catalog.volume(entry['volume_id'])
        backend.check(record)
        with Tree(record['root']) as tree:
            require(checked_hash(tree, entry['path'], cancel)[0] == entry['sha256'], 'checksum_mismatch', 'Replacement work copy changed')


def plan_release(catalog, backend, archive, work=None, path='', source=None, project='', cancel=None):
    archive_record = catalog.volume(archive)
    require(archive_record['role'] == 'archive', 'invalid_role', 'Archive required')
    archive_info = backend.check(archive_record)
    require(bool(source) != bool(work), 'invalid_release', 'Choose one Stalker original or a work folder')
    require(isinstance(project, str) and project.strip(), 'invalid_release', 'Name the project being released')
    if source:
        root = Path(source).expanduser().absolute()
        require(root.suffix.lower() == '.stalker', 'invalid_release', 'Local release only supports one completed Stalker bundle')
        manifest = read_json(root / 'manifest.json', 1024 * 1024)
        require(manifest.get('status') == 'completed' and manifest.get('producer') == 'stalker', 'active_capture', 'Only completed Stalker captures may be released')
        matches = catalog.db.execute('SELECT assets.payload FROM assets JOIN sessions ON sessions.asset_id=assets.id WHERE lower(sessions.session_id)=? AND sessions.manifest_digest=?',
                                     (str(uuid.UUID(manifest['sessionID'])), digest(manifest))).fetchall()
        require(matches, 'missing_receipt', 'Back up this exact Stalker revision before releasing it')
        assets = [json.loads(r[0]) for r in matches]
        source_root, scope, work_id = root.parent, root.name, None
    else:
        work_record = catalog.volume(work)
        require(work_record['role'] == 'work', 'invalid_role', 'Work disk required')
        backend.check(work_record)
        scope = relative(path)
        require(scope.startswith(work_record['prefix'] + '/') and not protected(scope), 'invalid_release', 'Choose a raw project folder inside the work prefix')
        source_root, work_id = Path(work_record['root']), work_record['id']
        assets = []
    source_info = backend.describe(source_root)
    require(independent(source_info, archive_info), 'same_physical_disk', 'Archive and released source must be independent disks')
    require(source_root != Path(archive_record['root']) and Path(archive_record['root']) not in source_root.parents
            and source_root not in Path(archive_record['root']).parents, 'overlapping_roots', 'Source overlaps archive')
    entries, preserved, replacements = [], [], {}
    with Tree(source_root) as origin, Tree(archive_record['root']) as target:
        paths, preserved = selected_files(origin, scope, bool(source), cancel)
        require(paths, 'empty_selection', 'No editable media to release in this folder')
        # First prefer receipts. Legacy files are matched by content, even after historical renames.
        receipt_candidates = {}
        for row in catalog.db.execute('SELECT payload FROM copies WHERE volume_id=?', (archive_record['id'],)):
            copy = json.loads(row[0])
            receipt_candidates.setdefault(copy['sha256'], []).append(copy['destination'])
        legacy_by_size = None
        cache = {}
        for path in paths:
            sha, stamp = checked_hash(origin, path, cancel)
            candidates = receipt_candidates.get(sha, [])
            if source:
                inner = path[len(scope) + 1:]
                candidates = []
                for asset in assets:
                    plan = catalog.job(asset['job_id'])['plan']
                    prefix = '' if asset['bundle'] == '.' else asset['bundle'] + '/'
                    for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                                   (plan['id'], archive_record['id'], prefix + inner)):
                        copy = json.loads(row[0])
                        if copy['sha256'] == sha: candidates.append(copy['destination'])
            if not candidates and not source:
                if legacy_by_size is None:
                    legacy_by_size = {}
                    for candidate, size in walk(target, cancel=cancel):
                        if size is not None: legacy_by_size.setdefault(size, []).append(candidate)
                candidates = legacy_by_size.get(stamp['size'], [])
            match = None
            for candidate in candidates:
                try:
                    if candidate not in cache: cache[candidate] = checked_hash(target, candidate, cancel)
                    if cache[candidate][0] == sha:
                        match = candidate
                        break
                except FileNotFoundError:
                    continue
            require(match, 'not_covered', 'No verified archive match; archive first: ' + path)
            entries.append({'path': path, 'stamp': stamp, 'sha256': sha, 'archive_path': match})
            original = str(source_root / path)
            replacement = str(Path(archive_record['root']) / match)
            old_bundle, new_bundle = bundle_root(path), bundle_root(match)
            if old_bundle:
                require(new_bundle, 'invalid_release', 'Bundle needs a complete bundle replacement')
                original, replacement = str(source_root / old_bundle), str(Path(archive_record['root']) / new_bundle)
            require(original not in replacements or replacements[original] == replacement, 'invalid_release', 'Bundle members resolve to different archive bundles')
            replacements[original] = replacement
        # Validate every complete capture on both sides; no partial legacy bundle releases.
        for original, replacement in replacements.items():
            if original.lower().endswith('.stalker'):
                left = read_json(Path(original) / 'manifest.json', 1024 * 1024)
                right = read_json(Path(replacement) / 'manifest.json', 1024 * 1024)
                require(left.get('producer') == 'stalker' and left.get('status') == 'completed' and digest(left) == digest(right),
                        'revision_mismatch', 'Stalker replacement must have the same completed manifest')
                from .planner import scan
                left_files, _, _ = scan(original)
                right_files, _, _ = scan(replacement)
                require({f['path'] for f in left_files} == {f['path'] for f in right_files}, 'revision_mismatch', 'Stalker bundle membership differs')
    replacement_checks = prefer_work(catalog, backend, archive_record, entries, replacements, cancel) if source else []
    try:
        consumers = clipper.inspect(catalog, replacements)
    except (RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
        consumers = {'blockers': [str(error)], 'relinks': [], 'dependencies': None}
    plan = {'schema': 'nu9ve-release/1', 'id': uid(), 'created_at': now(), 'archive_id': archive_record['id'],
            'source_root': str(source_root), 'source_identity': source_info, 'scope': scope, 'work_id': work_id,
            'project': project, 'entries': entries, 'preserved': preserved, 'replacements': replacements,
            'replacement_checks': replacement_checks,
            'bytes': sum(e['stamp']['size'] for e in entries), 'consumers': consumers, 'state': 'review', 'cleaned': {}}
    plan['digest'] = digest(plan)
    with catalog.writer():
        catalog.set('release:' + plan['id'], plan)
    return plan


def stored_plan(catalog, plan_id, reviewed_digest):
    plan = catalog.get('release:' + plan_id)
    require(plan and plan['digest'] == reviewed_digest, 'plan_changed', 'Use the recorded reviewed release plan')
    return plan


def check_source(backend, plan):
    current = backend.describe(plan['source_root'])
    require(all(current.get(k) == plan['source_identity'].get(k) for k in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode')),
            'source_changed', 'Release source identity changed')


def validate_remaining(tree, plan, cancel=None):
    try:
        actual, preserved = selected_files(tree, plan['scope'], not plan['work_id'], cancel)
    except FileNotFoundError:
        require(all(plan['cleaned'].get(e['path']) in {'intent', 'done'} for e in plan['entries']) and not plan['preserved'],
                'source_changed', 'Release folder disappeared before cleanup')
        actual, preserved = [], []
    expected = {e['path']: e for e in plan['entries']}
    require(set(actual) <= set(expected) and set(preserved) == set(plan['preserved']), 'source_changed', 'New material appeared; review a fresh release plan')
    for path, entry in expected.items():
        if path not in actual:
            require(plan['cleaned'].get(path) in {'intent', 'done'}, 'source_changed', 'Release source is missing before cleanup')
        else:
            require(content_stamp(tree.stat(path)) == entry['stamp'], 'source_changed', 'Release source changed: ' + path)


def relink_release(catalog, backend, plan_id, digest, cancel=None):
    with catalog.writer():
        plan = stored_plan(catalog, plan_id, digest)
        require(plan['state'] in {'review', 'applying'}, 'invalid_state', 'Release already completed')
        check_source(backend, plan)
        archive = catalog.volume(plan['archive_id'])
        backend.check(archive)
        with Tree(plan['source_root']) as origin, Tree(archive['root']) as target:
            validate_remaining(origin, plan, cancel)
            for entry in plan['entries']:
                require(checked_hash(target, entry['archive_path'], cancel)[0] == entry['sha256'], 'checksum_mismatch', 'Archive changed before relink')
        consumers = clipper.inspect(catalog, plan['replacements'])
        require(not consumers['blockers'], 'dependencies_blocked', '; '.join(consumers['blockers']))
        verify_replacements(catalog, backend, plan, cancel)
        backup_metadata(catalog, backend, archive['id'], cancel, locked=True)
        acks = []
        for request in consumers['relinks']:
            operation_id = str(uuid.uuid5(uuid.UUID(plan_id), request['project'] + ':' + request['asset_id'] + ':' + request['to']))
            acks.append(clipper.relink(catalog, request, operation_id))
        consumers = clipper.inspect(catalog, plan['replacements'])
        require(not consumers['blockers'] and not consumers['relinks'], 'dependencies_blocked', 'Clipper still references released paths')
        catalog.set('release-acks:' + plan_id, acks)
        plan['consumers'] = consumers
        catalog.set('release:' + plan_id, plan)
        backup_metadata(catalog, backend, archive['id'], cancel, locked=True)
        return {'acks': acks, 'consumers': consumers}


def review_release(catalog, backend, plan_id, digest):
    with catalog.writer():
        plan = stored_plan(catalog, plan_id, digest)
        if plan['state'] != 'completed':
            check_source(backend, plan)
            try:
                plan['consumers'] = clipper.inspect(catalog, plan['replacements'])
            except (OSError, RuntimeError, ValueError, KeyError, TypeError) as error:
                plan['consumers'] = {'blockers': [str(error)], 'relinks': [], 'dependencies': None}
            catalog.set('release:' + plan_id, plan)
        return plan


def apply_release(catalog, backend, plan_id, digest, cancel=None, hook=None):
    with catalog.writer():
        plan = stored_plan(catalog, plan_id, digest)
        if plan['state'] == 'completed': return plan
        check_source(backend, plan)
        archive = catalog.volume(plan['archive_id'])
        backend.check(archive)
        if plan['work_id']: backend.check(catalog.volume(plan['work_id']))
        consumers = clipper.inspect(catalog, plan['replacements'])
        require(not consumers['blockers'] and not consumers['relinks'], 'dependencies_blocked', 'Resolve Clipper references/queues/offline projects before releasing')
        verify_replacements(catalog, backend, plan, cancel)
        with Tree(plan['source_root']) as origin, Tree(archive['root']) as target:
            validate_remaining(origin, plan, cancel)
            # All archive copies must verify again before the first deletion.
            for entry in plan['entries']:
                require(checked_hash(target, entry['archive_path'], cancel)[0] == entry['sha256'], 'checksum_mismatch', 'Archive changed before release')
            backup_metadata(catalog, backend, archive['id'], cancel, locked=True)
            plan['state'] = 'applying'
            catalog.set('release:' + plan_id, plan)
            for entry in plan['entries']:
                path = entry['path']
                if plan['cleaned'].get(path) == 'done': continue
                require(not (cancel and cancel()), 'cancelled', 'Release paused; review and resume the same plan')
                check_source(backend, plan)
                backend.check(archive)
                if hook: hook('before_unlink', entry)
                # Re-read consumer state each time: no cached audit can authorize removal.
                consumers = clipper.inspect(catalog, plan['replacements'])
                require(not consumers['blockers'] and not consumers['relinks'], 'dependencies_blocked', 'Clipper dependencies changed during release')
                require(checked_hash(target, entry['archive_path'], cancel)[0] == entry['sha256'], 'checksum_mismatch', 'Archive changed during release')
                try:
                    sha, stamp = checked_hash(origin, path, cancel)
                except FileNotFoundError:
                    require(plan['cleaned'].get(path) == 'intent', 'source_changed', 'Unexpected missing release source')
                else:
                    require(sha == entry['sha256'] and stamp == entry['stamp'], 'source_changed', 'Source changed before release')
                    plan['cleaned'][path] = 'intent'
                    catalog.set('release:' + plan_id, plan)
                    origin.unlink(path, expected=stamp)
                plan['cleaned'][path] = 'done'
                catalog.set('release:' + plan_id, plan)
                if hook: hook('after_unlink', entry)
            validate_remaining(origin, plan, cancel)
            # Remove only empty directories through descriptor-relative IO. Ignored Finder metadata stays.
            parents = {str(Path(e['path']).parent) for e in plan['entries']}
            for path in sorted(parents, key=lambda p: len(Path(p).parts), reverse=True):
                while path != '.' and (path == plan['scope'] or path.startswith(plan['scope'] + '/')):
                    try:
                        with origin.parent(path) as (fd, name):
                            os.rmdir(name, dir_fd=fd)
                            from .common import full_sync
                            full_sync(fd)
                    except OSError:
                        break
                    path = str(Path(path).parent)
        if hook: hook('before_complete', None)
        plan['state'] = 'completed'
        if plan['work_id']:
            removed = {entry['path'] for entry in plan['entries']}
            for asset in catalog.search():
                copies = [json.loads(row[0]) for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'], plan['work_id']))]
                paths = {c['destination'] for c in copies if c['path'] in asset['paths']}
                if paths and paths <= removed:
                    with catalog.db:
                        catalog.db.execute('UPDATE work SET state=?,target=NULL,updated_at=? WHERE asset_id=? AND target=?', ('skipped', now(), asset['id'], plan['work_id']))
        catalog.set('release:' + plan_id, plan)
        catalog.event(plan_id, 'release_completed', {'bytes': plan['bytes'], 'scope': plan['scope']})
        try:
            plan['metadata_backup'] = backup_metadata(catalog, backend, archive['id'], cancel, locked=True)
        except (OSError, RuntimeError) as error:
            plan['metadata_backup_error'] = str(error)
        catalog.set('release:' + plan_id, plan)
        return plan


def release_plans(catalog):
    return [json.loads(r[0]) for r in catalog.db.execute("SELECT value FROM metadata WHERE key LIKE 'release:%'")]
