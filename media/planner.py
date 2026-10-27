"""Read-only plans: preserve original trees and treat Stalker as an atomic unit."""
import datetime
import json
import os
from pathlib import Path
import stat
import unicodedata
import uuid

from manager.inventory import camera_evidence, IGNORED_DIRS, IGNORED_FILES
from .volumes import independent
from .common import Tree, content_stamp, digest, fingerprint, read_json, relative, require, slug
from .layout import build_layout

CAMERAS = {'sony', 'canon', 'nikon', 'gopro', 'h6', 'dji-drone', 'dji-360', 'dji-action', 'insta360', 'generic'}


def scan(root, skip_active=False, cancel=None):
    files, bundles, skipped = [], {}, []
    root = Path(root).absolute()
    with Tree(root) as tree:
        def visit(folder, prefix='', bundle=None):
            require(not (cancel and cancel()), 'cancelled', 'Discovery cancelled')
            require(not (folder / 'clipper-project.json').exists() and folder.suffix.lower() not in {'.fcpbundle', '.fcpcache'},
                    'editor_project', 'Do not ingest an editor project with camera media')
            if folder.suffix.lower() == '.stalker':
                manifest = read_json(folder / 'manifest.json', 1024 * 1024)
                require(isinstance(manifest, dict), 'invalid_bundle', 'Invalid Stalker manifest')
                if manifest.get('status') != 'completed':
                    require(skip_active, 'active_capture', 'Stalker session is not completed')
                    skipped.append(prefix or '.')
                    return
                require(manifest.get('producer') == 'stalker' and manifest.get('schemaVersion') in {1, 2, 3},
                        'unsupported_bundle', 'Unsupported Stalker producer/version')
                try:
                    str(uuid.UUID(manifest['sessionID']))
                except (ValueError, KeyError, TypeError) as error:
                    from .common import MediaError
                    raise MediaError('invalid_bundle', 'Invalid Stalker session ID') from error
                tracks = manifest.get('tracks')
                require(isinstance(tracks, list) and tracks, 'invalid_bundle', 'Stalker tracks missing')
                for track in tracks:
                    require(isinstance(track, dict) and isinstance(track.get('relativePath'), str), 'invalid_bundle', 'Invalid track')
                    relative(track['relativePath'])
                    probe_rel = '/'.join(p for p in [prefix, track['relativePath']] if p)
                    tree.stat(probe_rel)
                for key in ('voice', 'events', 'highlights'):
                    name = (manifest.get('outputs') or {}).get(key)
                    if name:
                        relative(name)
                        tree.stat('/'.join(p for p in [prefix, name] if p))
                bundle = prefix or '.'
                bundles[bundle] = {'session_id': manifest['sessionID'], 'manifest_digest': digest(manifest)}
            with os.scandir(folder) as entries:
                entries = sorted(entries, key=lambda e: e.name)
            for entry in entries:
                rel = '/'.join(p for p in [prefix, entry.name] if p)
                relative(rel)
                require(not entry.is_symlink(), 'unsafe_file', 'Symlink in source: ' + rel)
                if entry.is_dir(follow_symlinks=False):
                    if entry.name not in IGNORED_DIRS:
                        visit(Path(entry.path), rel, bundle)
                elif entry.name in IGNORED_FILES or entry.name.startswith('._'):
                    continue
                else:
                    require(entry.is_file(follow_symlinks=False), 'unsafe_file', 'Special entry in source: ' + rel)
                    st = tree.stat(rel)
                    files.append({'path': rel, 'stamp': content_stamp(st), 'bundle': bundle,
                                  'evidence': camera_evidence(Path(entry.path))})
        visit(root)
    normalized = [unicodedata.normalize('NFC', f['path']).casefold() for f in files]
    require(len(set(normalized)) == len(normalized), 'name_collision', 'Source has case/Unicode name collisions')
    return files, bundles, skipped


def group_key(file):
    if file['bundle'] is not None:
        return 'stalker:' + file['bundle']
    path = Path(file['path'])
    stem = path.stem.upper()
    if path.suffix.lower() == '.xml' and stem.endswith('M01'):
        stem = stem[:-3]
    return str(path.parent) + '/' + stem


def build_plan(catalog, backend, source, archive, work=None, project='', profile='e64', country='mx', city='hermosillo',
               archive_only=False, confirm_archive_only=False, work_decision='pending', cleanup=False,
               assignments=None, camera=None, source_kind='card', cancel=None):
    require(isinstance(archive_only, bool) and isinstance(confirm_archive_only, bool), 'invalid_policy', 'Archive-only flags must be boolean')
    require(source_kind in {'card', 'stalker', 'delivery'}, 'invalid_source_kind', 'Invalid source kind')
    require(isinstance(cleanup, bool), 'invalid_policy', 'cleanup must be boolean')
    require(not cleanup or source_kind == 'card', 'cleanup_not_supported', 'Only card sources support automatic cleanup')
    project, profile = slug(project), slug(profile)
    country, city = slug(country), slug(city)
    require(camera is None or camera in CAMERAS, 'invalid_camera', 'Unknown camera category')
    source = Path(source).expanduser().absolute()
    source_info = backend.describe(source, external_required=cleanup)
    require(not cleanup or source_info['external'] and Path(source_info['mount']) == source,
            'card_root_required', 'Cleanup requires the root of a mounted external card')
    archive_record = catalog.volume(archive)
    require(archive_record['role'] == 'archive', 'invalid_role', 'Archive destination must have archive role')
    archive_info = backend.check(archive_record)
    work_record = catalog.volume(work) if work else None
    require(work_record is None or work_record['role'] == 'work', 'invalid_role', 'Work destination must have work role')
    expected_work = work_record['id'] if work_record else None
    if archive_only:
        require(confirm_archive_only is True, 'confirmation_required', 'Confirm archive-only for this operation')
        require(work_decision in {'pending', 'skipped'}, 'invalid_policy', 'Choose pending or skipped work delivery')
        require(work_record is not None or work_decision == 'skipped', 'work_target_required', 'Register/select the expected work volume for pending delivery')
        active_work = None
    else:
        require(work_record is not None, 'work_required', 'Select work volume, or explicitly confirm archive-only')
        work_info = backend.check(work_record)
        require(independent(archive_info, work_info), 'same_physical_disk', 'Archive and work must be different physical disks')
        active_work = work_record
        work_decision = 'ready'
    for target in [archive_record] + ([active_work] if active_work else []):
        root = Path(target['root']).absolute()
        require(source != root and source not in root.parents and root not in source.parents,
                'overlapping_roots', 'Source and destination roots must not overlap')
    files, bundles, skipped = scan(source, skip_active=source_kind == 'stalker', cancel=cancel)
    if source_kind == 'stalker':
        # Completed sessions already verified at this archive are not copied again.
        candidates = []
        present_bundles = {}
        for file in files:
            if file['bundle'] is None:
                continue
            session = bundles[file['bundle']]
            if file['bundle'] in present_bundles:
                if not present_bundles[file['bundle']]: candidates.append(file)
                continue
            archived = [json.loads(row[0]) for row in catalog.db.execute('SELECT assets.payload FROM assets JOIN sessions ON sessions.asset_id=assets.id WHERE sessions.session_id=? AND sessions.manifest_digest=?', (session['session_id'], session['manifest_digest']))]
            present = False
            for asset in archived:
                paths = asset['paths']
                rows = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'], archive_record['id'])).fetchall()
                copies = {r['path']: r for r in [json.loads(row[0]) for row in rows] if r['path'] in paths}
                if set(copies) != set(paths):
                    continue
                try:
                    with Tree(archive_record['root']) as target:
                        present = all(content_stamp(target.stat(c['destination'])) == c['stamp'] for c in copies.values())
                except (OSError, RuntimeError):
                    present = False
                if present: break
            present_bundles[file['bundle']] = present
            if not present:
                candidates.append(file)
        files = candidates
        retained_bundles = {f['bundle'] for f in files}
        bundles = {k: v for k, v in bundles.items() if k in retained_bundles}
    require(files, 'empty_source', 'No completed files to ingest')
    require(not cleanup or not skipped, 'active_capture', 'Cannot clean a source with active captures')
    assignments = assignments or {}
    require(isinstance(assignments, dict), 'invalid_assignment', 'Assignments must be a JSON object keyed by file path')
    known = {f['path'] for f in files}
    require(set(assignments) <= known, 'invalid_assignment', 'Assignment references an unknown file')
    groups = {}
    for file in files:
        groups.setdefault('delivery' if source_kind == 'delivery' else group_key(file), []).append(file)
    source_key = {k: source_info[k] for k in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode')}
    units = []
    for key, members in sorted(groups.items()):
        choices = [assignments[f['path']] for f in members if f['path'] in assignments]
        for choice in choices:
            require(isinstance(choice, dict) and not (set(choice) - {'project', 'profile', 'camera', 'work'}), 'invalid_assignment', 'Invalid assignment fields')
            require('work' not in choice or isinstance(choice['work'], bool), 'invalid_assignment', 'work must be boolean')
        require(not choices or all(c == choices[0] for c in choices), 'assignment_conflict', 'Paired files/bundles must be assigned together')
        assignment = choices[0] if choices else {}
        unit_project, unit_profile = slug(assignment.get('project', project)), slug(assignment.get('profile', profile))
        selected_camera = assignment.get('camera', camera)
        require(selected_camera is None or selected_camera in CAMERAS, 'invalid_camera', 'Unknown assigned camera category')
        evidence = [e for f in members for e in f['evidence']]
        if selected_camera is None:
            families = {e['family'] for e in evidence if e.get('family') and e.get('confidence') != 'low'}
            selected_camera = next(iter(families)) if len(families) == 1 else 'generic'
            require(not any(e.get('family') == 'dji' for e in evidence), 'camera_review_required', 'Classify DJI files as drone/360/action in assignments before ingest')
        bundle = members[0]['bundle']
        session = bundles.get(bundle) if bundle is not None else None
        unit_id = str(uuid.uuid5(uuid.NAMESPACE_URL, digest({'source': source_key, 'members': members})))
        unit = {'id': unit_id, 'project': unit_project, 'profile': unit_profile, 'camera': selected_camera,
                'kind': 'delivery' if source_kind == 'delivery' else ('stalker_session' if session else ('camera_bundle' if len(members) > 1 else 'file')),
                'paths': [f['path'] for f in members], 'bundle': bundle, 'session_id': session['session_id'] if session else None, 'capture_manifest_digest': session['manifest_digest'] if session else None}
        if 'work' in assignment:
            unit['work_selected'] = assignment['work']
        units.append(unit)
        for file in members:
            file['unit_id'] = unit_id
    require(archive_only or any(u.get('work_selected', True) for u in units),
            'confirmation_required', 'No editing copies selected; explicitly confirm archive-only')
    # A plan ID/digest is stable for an unchanged source, routing and policy.
    timestamp = datetime.datetime.fromtimestamp(max(f['stamp']['mtime_ns'] for f in files) / 1e9, datetime.timezone.utc)
    plan = {'schema': 'nu9ve-ingest-plan/2', 'source': str(source), 'source_identity': source_key,
            'source_kind': source_kind, 'archive_id': archive_record['id'], 'work_id': active_work['id'] if active_work else None,
            'expected_work_id': expected_work, 'archive_only_confirmed': archive_only and confirm_archive_only,
            'work_delivery_status': work_decision, 'cleanup': cleanup, 'country': country, 'city': city,
            'media_date': timestamp.strftime('%Y/%m'), 'source_name': relative(source.name),
            'files': files, 'units': units, 'bundles': bundles, 'skipped': skipped}
    if source_kind != 'delivery':
        plan['layout'] = build_layout(source, plan, units, files)
    sony = any('/private/m4root/' in ('/' + f['path']).lower() or '/private/avchd/' in ('/' + f['path']).lower()
               or f['path'].lower().startswith('avf_info/') for f in files)
    # Sony: after verification the clips leave the card (a fast camera format), but the camera's own indexes are
    # never edited here; the job then waits for that format (`confirm-cleanup`).
    plan['cleanup_profile'] = 'sony_clips_then_format' if sony or any(u['camera'] == 'sony' for u in units) else 'files'
    plan['id'] = str(uuid.uuid5(uuid.NAMESPACE_URL, digest(plan)))
    units_by_id = {u['id']: u for u in units}
    plan['destinations'] = {record['id']: {k: record[k] for k in ('root', 'prefix', 'role')} for record in [archive_record] + ([active_work] if active_work else [])}
    plan['preview'] = [{'source': f['path'], 'bytes': f['stamp']['size'], 'project': units_by_id[f['unit_id']]['project'],
                       'profile': units_by_id[f['unit_id']]['profile'], 'camera': units_by_id[f['unit_id']]['camera'],
                       'archive_path': str(Path(archive_record['root']) / destination_path(archive_record, plan, units_by_id[f['unit_id']], f['path'])),
                       'work_path': str(Path(active_work['root']) / destination_path(active_work, plan, units_by_id[f['unit_id']], f['path']))
                                    if active_work and destination_path(active_work, plan, units_by_id[f['unit_id']], f['path']) else None}
                      for f in files]
    plan['preview'].sort(key=lambda row: row['archive_path'])
    for record in [archive_record] + ([active_work] if active_work else []):
        # Fail at review time, not mid-copy, when a different file already holds a planned name.
        with Tree(record['root']) as target:
            for f in files:
                rel = destination_path(record, plan, units_by_id[f['unit_id']], f['path'])
                if rel is None:
                    continue
                try:
                    size = target.stat(rel).st_size
                except FileNotFoundError:
                    continue
                require(size == f['stamp']['size'], 'destination_conflict', 'A different file already exists at ' + rel)
    require(len(files) <= 100000 and len(__import__('json').dumps(plan).encode()) <= 12 * 1024 * 1024, 'plan_too_large', 'Split this source into smaller ingest operations')
    plan['digest'] = digest(plan)
    preflight(catalog, backend, plan, reserve=True)
    return plan


def validate_plan(plan):
    require(isinstance(plan, dict) and plan.get('schema') in {'nu9ve-ingest-plan/1', 'nu9ve-ingest-plan/2'}, 'invalid_plan', 'Unsupported plan')
    raw = dict(plan); supplied = raw.pop('digest', None)
    require(digest(raw) == supplied, 'plan_changed', 'Plan digest mismatch')
    try:
        uuid.UUID(plan['id'])
        for unit in plan['units']:
            uuid.UUID(unit['id'])
            slug(unit['project']); slug(unit['profile'])
        require(isinstance(plan['cleanup'], bool), 'invalid_plan', 'Invalid cleanup policy')
        require(plan['source_kind'] in {'card', 'stalker', 'delivery'}, 'invalid_plan', 'Invalid source type')
        require(not plan['cleanup'] or plan['source_kind'] == 'card', 'invalid_plan', 'Only cards may be cleaned')
        relative(plan['media_date']); relative(plan['source_name'])
        require('/' not in plan['source_name'], 'invalid_plan', 'Source name must be one component')
        ids = {u['id']: u for u in plan['units']}
        require(len(ids) == len(plan['units']), 'invalid_plan', 'Duplicate units')
        seen = set()
        for file in plan['files']:
            relative(file['path'])
            require(file['path'] not in seen and file['unit_id'] in ids, 'invalid_plan', 'Duplicate file or unknown unit')
            seen.add(file['path'])
            require(file['path'] in ids[file['unit_id']]['paths'], 'invalid_plan', 'Unit/member mismatch')
            require(isinstance(file['stamp']['size'], int) and file['stamp']['size'] >= 0, 'invalid_plan', 'Invalid file size')
        require(seen and seen == {p for u in ids.values() for p in u['paths']}, 'invalid_plan', 'Empty or inconsistent plan')
        if plan['schema'] == 'nu9ve-ingest-plan/2' and plan['source_kind'] != 'delivery':
            layout = plan['layout']
            require(set(layout) == seen, 'invalid_plan', 'Layout must cover every planned file')
            for role in ('archive', 'work'):
                targets = [entry[role] for entry in layout.values() if entry[role] is not None]
                for target in targets:
                    relative(target)
                require(len({t.lower() for t in targets}) == len(targets), 'invalid_plan', 'Two files share one destination')
            require(all(entry['archive'] for entry in layout.values()), 'invalid_plan', 'Every file needs an archive destination')
        if plan['work_id'] is None:
            require(plan['archive_only_confirmed'] is True and plan['work_delivery_status'] in {'pending', 'skipped'},
                    'confirmation_required', 'Archive-only confirmation missing')
        require(plan['cleanup_profile'] in {'files', 'camera_action_required', 'sony_clips_then_format'}, 'invalid_plan', 'Invalid cleanup profile')
        require(set(plan['source_identity']) == {'mode', 'volume_uuid', 'disk_uuid', 'root_inode'}, 'invalid_plan', 'Source identity is incomplete')
        sony_paths = any('/private/m4root/' in ('/' + f['path']).lower() or '/private/avchd/' in ('/' + f['path']).lower() or f['path'].lower().startswith('avf_info/') for f in plan['files'])
        require(not (sony_paths or any(u['camera'] == 'sony' for u in plan['units'])) or plan['cleanup_profile'] in {'camera_action_required', 'sony_clips_then_format'},
                'invalid_plan', 'Sony cleanup requires camera action until a validated profile exists')
    except (KeyError, TypeError, ValueError) as error:
        from .common import MediaError
        raise MediaError('invalid_plan', 'Malformed ingest plan') from error


def destination_path(record, plan, unit, source_path, selected_only=True):
    """Relative destination below the volume root, or None when this role does not receive the file."""
    relative(record['prefix'])
    if selected_only and record['role'] == 'work' and unit.get('work_selected') is False:
        return None
    if 'layout' in plan:
        target = plan['layout'][source_path][record['role']]
        return None if target is None else record['prefix'] + '/' + relative(target)
    # Plans before schema 2 nest original trees under per-job/per-unit containers.
    if record['role'] == 'archive':
        prefix = (record.get('finals_prefix', '002_finales') if plan['source_kind'] == 'delivery' else record['prefix']) + '/' + ('stalker' if unit['kind'] == 'stalker_session' else 'camera-bundles') + '/' + plan['media_date']
    else:
        prefix = record['prefix'] + '/' + slug(unit['profile']) + '_raw'
    return '/'.join([prefix, slug(unit['project']), plan['id'], unit['id'], relative(plan['source_name']), relative(source_path)])


def preflight(catalog, backend, plan, reserve=False):
    validate_plan(plan)
    infos = []
    for volume_id in [plan['archive_id']] + ([plan['work_id']] if plan['work_id'] else []):
        record = catalog.volume(volume_id)
        expected_role = 'archive' if volume_id == plan['archive_id'] else 'work'
        require(record['role'] == expected_role, 'invalid_role', 'Destination role changed')
        require(plan['destinations'].get(volume_id) == {k: record[k] for k in ('root', 'prefix', 'role')}, 'destination_changed', 'Registered destination changed after planning')
        info = backend.check(record)
        source, root = Path(plan['source']), Path(record['root'])
        require(source != root and source not in root.parents and root not in source.parents, 'overlapping_roots', 'Overlapping roots')
        block = max(int(info['block_size']), 4096)
        units = {u['id']: u for u in plan['units']}
        needed = sum(((f['stamp']['size'] + block - 1) // block) * block for f in plan['files']
                     if destination_path(record, plan, units[f['unit_id']], f['path']))
        if reserve:
            require(info['free'] >= needed + max(64 * 1024 * 1024, needed // 20), 'insufficient_space', 'Insufficient free space on ' + record['name'])
        infos.append(info)
    require(len(infos) == 1 or independent(infos[0], infos[1]), 'same_physical_disk', 'Destinations share a physical disk')
