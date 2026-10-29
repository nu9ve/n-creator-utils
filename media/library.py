"""Deferred work, restoration and portable editor handoffs; never edits an editor's project."""
import json
import os
from pathlib import Path
import uuid

from .volumes import independent
from .common import Tree, canonical, content_stamp, full_sync, now, require, uid
from .layout import MEDIA_KINDS, kind_of, parse_name
from .planner import destination_path
from .transfer import Transfer, claim_directory


def asset_record(catalog, asset_id, revision=None):
    row = catalog.db.execute('SELECT payload FROM assets WHERE id=?', (asset_id,)).fetchone()
    require(row is not None, 'unknown_asset', 'Asset not found')
    asset = json.loads(row[0])
    if revision is not None and asset['revision'] != revision:
        row = catalog.db.execute('SELECT payload FROM asset_revisions WHERE asset_id=? AND revision=?', (asset_id, revision)).fetchone()
        require(row is not None, 'unknown_revision', 'Asset revision not found')
        asset = json.loads(row[0])
    return asset


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
                files = [f for f in files if destination_path(record, plan, asset, f['path'], selected_only=False)]
                override = catalog.get('layout_override:' + plan['id'], {})
                if 'layout' not in plan:
                    first_dest = destination_path(record, plan, asset, files[0]['path'], selected_only=False)
                    base = first_dest[:first_dest.index('/' + plan['id'] + '/')] + '/' + plan['id']
                    claim_directory(target, base, {'job_id': plan['id'], 'digest': plan['digest'], 'volume_id': record['id']})
                for file in files:
                    prior = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                               (plan['id'], archive['id'], file['path'])).fetchone()
                    require(prior is not None, 'missing_receipt', 'Archive copy is not verified')
                    prior = json.loads(prior[0])
                    dest = destination_path(record, plan, asset, file['path'], selected_only=False)
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


def resolve(catalog, backend, asset_id, revision=None):
    asset = asset_record(catalog, asset_id, revision)
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


def clipper_profile(catalog, profile):
    return catalog.get('creative-profiles', {}).get(profile, {}).get('clipper_profile')


def receipt_metadata(catalog, asset, copies):
    """Use stored receipts only. Old catalogs deliberately report unknown civil time."""
    from .evidence import capture_time
    rows = {r['path']: r for group in copies.values() for r in group.values()}
    prefix = '' if asset.get('bundle') in {None, '.'} else asset['bundle'] + '/'
    editable = [p for p in asset['paths'] if kind_of(p) in MEDIA_KINDS]
    members = [{'path': p, 'bytes': rows[p]['bytes'], 'sha256': rows[p]['sha256']} for p in sorted(editable) if p in rows]
    return {'content_revision': str(uuid.uuid5(uuid.NAMESPACE_URL, asset['revision'])),
            'manifest_sha256': rows.get(prefix + 'manifest.json', {}).get('sha256') if asset['kind'] == 'stalker_session' else None,
            'media_bytes': sum(m['bytes'] for m in members), 'editable_members': members,
            'capture_time': asset.get('capture_time') or capture_time(),
            'clipper_profile': clipper_profile(catalog, asset['profile'])}


def handoff(catalog, backend, asset_ids, volume_id=None):
    require(asset_ids and len(set(asset_ids)) == len(asset_ids), 'invalid_selection', 'Select distinct assets')
    assets = [asset_record(catalog, value) for value in asset_ids]
    require(len({(a['project_id'], a['profile']) for a in assets}) == 1, 'mixed_project', 'Handoff must be one project/profile')
    result = {'schema': 'creator-media-handoff/1', 'handoff_id': uid(), 'created_at': now(),
              'project_id': assets[0]['project_id'], 'profile_id': assets[0]['profile'],
              'clipper_profile': clipper_profile(catalog, assets[0]['profile']), 'assets': []}
    for order, asset in enumerate(assets):
        plan = catalog.job(asset['job_id'])['plan']
        locations, groups = [], {}
        for record in catalog.volumes():
            if volume_id is not None and record['id'] != volume_id:
                continue
            rows = [json.loads(r[0]) for r in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (plan['id'], record['id']))]
            copies = {r['path']: r for r in rows if r['path'] in asset['paths']}
            groups[record['id']] = copies
            expected = {p for p in asset['paths'] if destination_path(record, plan, asset, p, selected_only=False)}
            if not expected or set(copies) != expected:
                continue
            selected = [r for p, r in sorted(copies.items()) if asset['kind'] in {'stalker_session', 'delivery'} or kind_of(p) in MEDIA_KINDS]
            if not selected:
                selected = [r for _, r in sorted(copies.items())]
            if asset['kind'] == 'delivery':
                selected.sort(key=lambda r: (kind_of(r['path']) not in MEDIA_KINDS, r['path']))
            sample = selected[0]
            if asset['kind'] == 'stalker_session':
                prefix = '' if asset['bundle'] == '.' else asset['bundle'] + '/'
                location_path = sample['destination'][:-len(sample['path'][len(prefix):]) - 1]
                member_path = lambda r: r['destination'][len(location_path) + 1:]
                path_kind = 'directory'
            else:
                # File locations identify the primary file; each member also carries a volume-relative path.
                location_path = sample['destination']
                member_path = lambda r: Path(r['destination']).name
                path_kind = 'file'
            members = [{'relative_path': member_path(r), 'original_path': r['path'], 'bytes': r['bytes'],
                        'sha256': r['sha256'], 'volume_relative_path': r['destination']} for r in selected]
            locations.append({'volume_id': record['id'], 'relative_path': location_path, 'path_kind': path_kind,
                              'role': record['role'], 'members': members,
                              'receipt_id': sample.get('receipt_id') or str(uuid.uuid5(uuid.NAMESPACE_URL, plan['id'] + ':' + record['id']))})
        require(locations, 'missing_receipt', 'No complete verified location')
        # Replicas may have different names, but a shared original member must have identical bytes/hash.
        seen = {}
        for location in locations:
            for member in location['members']:
                identity = (member['bytes'], member['sha256'])
                require(member['original_path'] not in seen or seen[member['original_path']] == identity,
                        'revision_mismatch', 'Replica content differs')
                seen[member['original_path']] = identity
        item = {'asset_id': asset['id'], 'revision': asset['revision'], 'kind': asset['kind'], 'order': order,
                'members': locations[0]['members'], 'locations': locations,
                'bytes': sum(f['stamp']['size'] for f in plan['files'] if f['path'] in asset['paths']),
                **receipt_metadata(catalog, asset, groups)}
        if asset.get('session_id'):
            item['capture_session_id'] = str(uuid.UUID(asset['session_id']))
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


def present(root, copy):
    """Cheap check that a recorded copy is still there at its size; hashes stay with verify/recovery."""
    try:
        st = os.stat(Path(root) / copy['destination'], follow_symlinks=False)
    except OSError:
        return False
    return st.st_size == copy['bytes']


def sources(catalog, backend, project, purpose='edit'):
    """Editable sources of a project in capture order, one call, for an editor's quick import (read-only)."""
    require(purpose in {'edit', 'archive'}, 'invalid_purpose', 'Purpose must be edit or archive')
    assets = [a for a in catalog.search(project) if a['kind'] in {'stalker_session', 'camera_bundle', 'file', 'delivery'}]
    require(assets, 'unknown_project', 'No ingested assets for project ' + str(project))
    volumes = {v['id']: dict(v, online=bool(backend.online(v))) for v in catalog.volumes()}
    preference = ('work', 'archive') if purpose == 'edit' else ('archive', 'work')
    result, offline, removed, warnings = [], [], [], []
    for asset in assets:
        copies = {}
        for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=?', (asset['job_id'],)):
            copy = json.loads(row[0])
            if copy['path'] in asset['paths']:
                copies.setdefault(copy['volume_id'], {})[copy['path']] = copy
        if asset['kind'] == 'stalker_session':
            prefix = '' if asset['bundle'] == '.' else asset['bundle'] + '/'
            primary = [prefix + 'manifest.json']
        else:
            primary = sorted(p for p in asset['paths'] if kind_of(p) in MEDIA_KINDS)
        if not primary:
            continue  # Camera indexes/thumbnails are never editor sources.
        candidates, missing = [], []
        for volume_id, rows in copies.items():
            if not all(p in rows for p in (asset['paths'] if asset['kind'] in {'delivery', 'stalker_session'} else primary)):
                continue
            volume = volumes[volume_id]
            sample = rows[primary[0]]
            if asset['kind'] == 'stalker_session':
                relative_path = sample['destination'][:-len(sample['path'][len(prefix):]) - 1]
            else:
                relative_path = sample['destination']
            location = {'volume_id': volume_id, 'receipt_id': sample.get('receipt_id') or str(uuid.uuid5(uuid.NAMESPACE_URL, asset['job_id'] + ':' + volume_id)), 'volume': volume['name'], 'role': volume['role'], 'online': volume['online'], 'path': str(Path(volume['root']) / relative_path)}
            if volume['online'] and not all(present(volume['root'], row) for row in rows.values()):
                # Deleted or edited by hand (e.g. an unusable clip removed in Finder): report it, never fail or resurrect it.
                missing.append(location)
                continue
            candidates.append((not volume['online'], preference.index(volume['role']) if volume['role'] in preference else 9, location, relative_path, sample))
        if not candidates and not missing:
            continue
        candidates.sort(key=lambda c: (c[0], c[1]))
        name = (candidates[0][3] if candidates else missing[0]['path']).rsplit('/', 1)[-1]
        entry = {'asset_id': asset['id'], 'revision': asset['revision'], 'kind': asset['kind'], 'session_id': asset.get('session_id'),
                 'name': name, 'bytes': sum(f['stamp']['size'] for f in catalog.job(asset['job_id'])['plan']['files'] if f['path'] in asset['paths']),
                 'sha256': None if asset['kind'] == 'stalker_session' or not candidates else candidates[0][4]['sha256'],
                 # Wall-clock capture start (seconds precision, no zone) and rounded duration, for time-based grouping.
                 **(parse_name(name) or {'captured_at': None, 'duration_seconds': None}),
                 'alternatives': [c[2] for c in candidates[1:]], 'missing': missing}
        entry.update(receipt_metadata(catalog, asset, copies))
        if candidates:
            selected_rows = copies[candidates[0][2]['volume_id']]
            entry['editable_members'] = [dict(m, local_path=str(Path(volumes[candidates[0][2]['volume_id']]['root']) / selected_rows[m['path']]['destination']))
                                         for m in entry['editable_members'] if m['path'] in selected_rows]
            if asset['kind'] == 'delivery':
                root = Path(volumes[candidates[0][2]['volume_id']]['root'])
                entry['media_members'] = [dict(original_path=p, path=str(root / selected_rows[p]['destination']),
                                                bytes=selected_rows[p]['bytes'], sha256=selected_rows[p]['sha256']) for p in asset['paths']]
                # The current handoff consumer compares this field against ALL supplied members.
                # Complete delivery evidence lives in media_members; media_bytes remains media-only.
                entry.pop('editable_members', None)
            entry.update({k: candidates[0][2][k] for k in ('path', 'volume', 'volume_id', 'receipt_id', 'role', 'online')})
        if any(m['role'] == 'archive' for m in missing):
            warnings.append({'asset_id': asset['id'], 'name': name, 'problem': 'archive_copy_missing', 'locations': missing})
        if purpose == 'edit' and any(m['role'] == 'work' for m in missing):
            removed.append(entry)  # The user took it out of the working set; it stays available from the archive.
        elif candidates and candidates[0][2]['online']:
            result.append(entry)
        else:
            offline.append(entry)
    # Dated names sort as capture time; this is the order the editor should present.
    result.sort(key=lambda e: e['name'])
    offline.sort(key=lambda e: e['name'])
    removed.sort(key=lambda e: e['name'])
    for order, entry in enumerate(result):
        entry['order'] = order
    return {'project': assets[0]['project'], 'project_id': assets[0]['project_id'], 'profile': assets[0]['profile'],
            'clipper_profile': clipper_profile(catalog, assets[0]['profile']), 'purpose': purpose, 'sources': result, 'offline': offline, 'removed_from_work': removed, 'warnings': warnings}


def projects(catalog):
    """Projects with ingested editable sources, newest first, for an editor's project picker (read-only)."""
    found = {}
    for asset in catalog.search():
        if asset['kind'] != 'stalker_session' and not any(kind_of(p) in MEDIA_KINDS for p in asset['paths']):
            continue  # Camera indexes/thumbnails are not sources.
        entry = found.setdefault(asset['project_id'], {'project': asset['project'], 'project_id': asset['project_id'],
                                                       'profile': asset['profile'], 'clipper_profile': clipper_profile(catalog, asset['profile']), 'assets': 0, 'jobs': set()})
        entry['assets'] += 1
        entry['jobs'].add(asset['job_id'])
    for entry in found.values():
        jobs = sorted(entry.pop('jobs'))
        entry['updated_at'] = max(catalog.job(job)['updated_at'] for job in jobs)
    return sorted(found.values(), key=lambda e: e['updated_at'], reverse=True)
