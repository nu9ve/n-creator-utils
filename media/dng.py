"""Raw stills in the consumer's work copy (Atlas A16): keep a reviewed sample, release the rest.

The archive keeps every JPG+DNG. The work volume keeps JPGs, plus the DNGs of a sample chosen from data:
the consumer's labelled photos, a recent session per light class, deliberately poor photos (to see whether
the raw recovers them) and an older good mission. Rules live in the recipe's mission flow (`dng`).

`plan` is read-only and returns a digest. `apply` requires it and, per DNG to release: hashes the work file,
requires it to equal the archive copy's verified SHA-256 (whose receipt stamp must still match on the
archive) and only then moves it to the work volume's Trash (`.Trashes/<uid>`; emptying it is the person's
step). Sample DNGs missing from work are copied from the archive next to their JPG and verified. Stills that
have no backup are moved to the Trash only when listed explicitly. Every step is an event of the operation.
"""
import datetime
import fnmatch
import glob
import hashlib
import json
import os
from pathlib import Path

from .common import MediaError, Tree, content_stamp, digest, hash_fd, now, require, uid
from .missions import _paths, dumps
from .recipes import mission_flow, recipe

PLAN_SCHEMA = 'nu9ve-dng-plan/1'


def _units(tree, rel):
    raw = tree.read_bytes(rel)
    require(raw is not None, 'no_units_map', 'No units map yet: ingest drone stills first')
    return [json.loads(line) for line in raw.decode('utf-8').splitlines() if line.strip()]


def _evenly(items, count):
    if len(items) <= count:
        return list(items)
    step = len(items) / count
    return [items[int(i * step)] for i in range(count)]


def _label_photos(work_root, prefix, rules):
    """Capture ids (`DJI_0377_20261105174152`) in the consumer's label manifests; read only."""
    found = {}
    pattern = os.path.join(work_root, prefix.strip('/'), rules['labels'])
    for path in sorted(glob.glob(pattern)):
        try:
            manifest = json.loads(Path(path).read_text(encoding='utf-8'))
        except (OSError, ValueError):
            continue
        for capture in manifest.get(rules.get('labels_key', 'photos')) or []:
            if isinstance(capture, str):
                found[capture] = manifest.get('set') or Path(path).parent.name
    return found


def _capture_id(unit):
    """The consumer's photo id: original stem + EXIF time (`DJI_0377_20261105174152`)."""
    when = (unit.get('captured_at') or '').replace('-', '').replace(':', '').replace('T', '')
    return Path(unit['original_name']).stem + '_' + when


def select(units, history, labels, rules):
    """Sample groups from data: labelled, newest session per light class, poor photos, an older good mission."""
    log = {r.get('captureID'): r for key in ('captureLog', 'captures') for r in history.get(key) or [] if r.get('captureID')}
    plans = {p.get('id'): p.get('name') for p in history.get('plans') or []}
    sessions = {}
    for unit in units:
        mission = unit.get('mission')
        if not mission or not unit.get('archive', {}).get('dng'):
            continue
        record = log.get(mission['captureID'], {})
        session = sessions.setdefault(mission['sessionID'], {'units': [], 'light': {}, 'quality': {}, 'plan': plans.get(mission['planID']) or unit.get('plan_name')})
        session['units'].append((unit, record))
        for key, field in (('light', 'lightClass'), ('quality', 'qualityGrade')):
            value = record.get(field)
            if value:
                session[key][value] = session[key].get(value, 0) + 1
    for session in sessions.values():
        session['units'].sort(key=lambda pair: pair[0].get('captured_at') or '')
        session['start'] = session['units'][0][0].get('captured_at') or ''
    ordered = sorted(sessions.items(), key=lambda kv: kv[1]['start'], reverse=True)
    groups, taken = [], set()

    def add(reason, detail, members):
        chosen = [u for u in members if u['asset_id'] not in taken]
        taken.update(u['asset_id'] for u in chosen)
        if chosen:
            groups.append({'reason': reason, 'detail': detail, 'asset_ids': [u['asset_id'] for u in chosen]})

    by_capture = {_capture_id(u): u for u in units if u.get('archive', {}).get('dng')}
    labelled = [by_capture[c] for c in sorted(labels) if c in by_capture]
    add('labelled', ', '.join(sorted(set(labels.values()))), labelled)
    used_sessions = set()
    for light in rules['light_classes']:
        for session_id, session in ordered:
            total = sum(session['light'].values())
            if total and session['light'].get(light, 0) / total >= rules['dominance']:
                best = [u for u, r in session['units'] if r.get('qualityGrade') in ('good', 'fair')] or [u for u, _ in session['units']]
                add('light', '{} · {}'.format(light, session['plan']), _evenly(best, rules['per_light']))
                used_sessions.add(session_id)
                break
    for session_id, session in ordered:
        total = sum(session['quality'].values())
        if session_id not in used_sessions and total and session['quality'].get('poor', 0) / total >= rules['dominance']:
            poor = [u for u, r in session['units'] if r.get('qualityGrade') == 'poor']
            add('poor', session['plan'], _evenly(poor, rules['poor_sample']))
            break
    cutoff = (datetime.datetime.now() - datetime.timedelta(days=rules['older_than_days'])).isoformat()
    older = [s for _, s in ordered if s['start'] < cutoff]
    if older:
        largest = max(older, key=lambda s: len(s['units']))
        add('older', largest['plan'], _evenly([u for u, _ in largest['units']], rules['older_sample']))
    return groups


def _trash(record):
    """The volume's own Trash, as Finder uses it on external disks."""
    return '.Trashes/{}'.format(os.getuid())


def plan(catalog, backend, archive, work, keep=None, trash_unbacked=None, flow_id=None):
    """Read only: which DNGs stay in work (sample), which are copied there, which go to the Trash."""
    flow = mission_flow(recipe(catalog), flow_id)
    rules = flow['dng']
    archive_record, work_record = catalog.volume(archive), catalog.volume(work)
    require(archive_record['role'] == 'archive' and work_record['role'] == 'work', 'invalid_role', 'Choose the archive and the work volume')
    backend.check(archive_record)
    backend.check(work_record)
    map_rel = work_record['prefix'].strip('/') + '/' + flow['units_map'].strip('/')
    base = os.path.dirname(os.path.dirname(map_rel))
    with Tree(work_record['root']) as tree:
        units = _units(tree, map_rel)
    with Tree(archive_record['root']) as tree:
        history = json.loads(tree.read_bytes(_paths(flow)[0]) or b'{}')
    labels = _label_photos(work_record['root'], work_record['prefix'], rules)
    groups = select(units, history, labels, rules) if keep is None else [{'reason': 'chosen', 'detail': '', 'asset_ids': list(keep)}]
    keep_ids = {a for g in groups for a in g['asset_ids']}
    receipts = {}
    for (payload,) in catalog.db.execute('SELECT payload FROM copies WHERE volume_id=?', (archive_record['id'],)):
        receipt = json.loads(payload)
        receipts[receipt['destination']] = receipt
    release, copy, kept, problems = [], [], [], []
    known = set()
    root = Path(work_record['root'])
    for unit in units:
        archive_dng, sha = unit.get('archive', {}).get('dng'), unit.get('sha256', {}).get('dng')
        jpg = unit.get('work', {}).get('jpg')
        if not archive_dng or not jpg:
            continue
        work_dng = str(Path(base) / Path(jpg).with_suffix('.DNG'))
        known.update({work_dng, str(Path(base) / jpg)})  # backed-up stills never qualify as «without backup»
        present = (root / work_dng).is_file()
        receipt = receipts.get(archive_dng)
        if unit['asset_id'] in keep_ids:
            if present:
                kept.append({'asset_id': unit['asset_id'], 'work': work_dng})
            elif receipt and receipt['sha256'] == sha:
                copy.append({'asset_id': unit['asset_id'], 'archive': archive_dng, 'sha256': sha, 'work': work_dng, 'bytes': receipt['bytes']})
            else:
                problems.append({'asset_id': unit['asset_id'], 'reason': 'no_verified_archive_copy'})
        elif present:
            if receipt and receipt['sha256'] == sha:
                release.append({'asset_id': unit['asset_id'], 'work': work_dng, 'archive': archive_dng, 'sha256': sha,
                                'archive_stamp': receipt['stamp'], 'bytes': (root / work_dng).stat().st_size})
            else:
                problems.append({'asset_id': unit['asset_id'], 'work': work_dng, 'reason': 'no_verified_archive_copy'})
    unbacked = []
    for rel in sorted(trash_unbacked or []):
        require(isinstance(rel, str) and rel.startswith(base + '/') and rel not in known, 'invalid_request',
                'Only stills without a backup, inside the consumer root, can be listed: ' + str(rel))
        path = root / rel
        require(path.is_file() and not path.is_symlink(), 'invalid_request', 'Not a regular file: ' + rel)
        unbacked.append({'work': rel, 'stamp': content_stamp(path.lstat()), 'bytes': path.lstat().st_size})
    value = {'schema': PLAN_SCHEMA, 'flow': flow['id'], 'archive_id': archive_record['id'], 'work_id': work_record['id'],
             'groups': groups, 'kept': kept, 'copy': copy, 'release': release, 'trash_unbacked': unbacked, 'problems': problems,
             'totals': {'release_files': len(release), 'release_bytes': sum(r['bytes'] for r in release),
                        'copy_files': len(copy), 'copy_bytes': sum(c['bytes'] for c in copy), 'kept_files': len(kept),
                        'unbacked_files': len(unbacked), 'unbacked_bytes': sum(u['bytes'] for u in unbacked)}}
    value['digest'] = digest(value)
    return value


def apply(catalog, backend, value, digest_value, cancel=None):
    """Execute exactly the reviewed plan; stops at the first mismatch. Nothing is deleted, only moved to the Trash."""
    from .transfer import Transfer
    require(value.get('schema') == PLAN_SCHEMA and digest({k: v for k, v in value.items() if k != 'digest'}) == digest_value,
            'review_changed', 'The DNG plan changed; review it again')
    archive_record, work_record = catalog.volume(value['archive_id']), catalog.volume(value['work_id'])
    cancel = cancel or (lambda: False)
    operation = 'dng-' + digest_value[:16]
    done = {'copied': 0, 'released': 0, 'trashed_unbacked': 0}
    with catalog.writer():
        backend.check(archive_record)
        backend.check(work_record)
        engine = Transfer(catalog, backend, cancel)
        with Tree(archive_record['root']) as archive, Tree(work_record['root']) as work:
            for item in value['copy']:
                require(not cancel(), 'cancelled', 'Paused; verified steps are kept')
                sha, _ = engine.copy_file(archive, item['archive'], work, item['work'], work_record, expected=item['sha256'])
                require(sha == item['sha256'], 'checksum_mismatch', 'Copied DNG differs: ' + item['work'])
                catalog.event(operation, 'dng_copied', {'asset_id': item['asset_id'], 'work': item['work'], 'sha256': sha})
                done['copied'] += 1
            trash = _trash(work_record)
            for item in value['release']:
                require(not cancel(), 'cancelled', 'Paused; verified steps are kept')
                try:
                    fd = work.open(item['work'])
                except FileNotFoundError:
                    continue  # moved by an earlier, interrupted run
                try:
                    require(hash_fd(fd, cancel) == item['sha256'], 'checksum_mismatch', 'Work DNG differs from the archive: ' + item['work'])
                finally:
                    os.close(fd)
                require(content_stamp(archive.stat(item['archive'])) == item['archive_stamp'], 'archive_changed',
                        'The archive copy changed since it was verified: ' + item['archive'])
                _move_to_trash(work_record, item['work'], trash)
                catalog.event(operation, 'dng_released', {'asset_id': item['asset_id'], 'work': item['work'], 'archive': item['archive']})
                done['released'] += 1
            for item in value['trash_unbacked']:
                path = Path(work_record['root']) / item['work']
                if not path.exists():
                    continue
                require(content_stamp(path.lstat()) == item['stamp'], 'source_changed', 'Changed since review: ' + item['work'])
                _move_to_trash(work_record, item['work'], trash)
                catalog.event(operation, 'unbacked_trashed', {'work': item['work']})
                done['trashed_unbacked'] += 1
        _update_units_map(catalog, work_record, value['flow'])
    return dict(done, operation=operation)


def _move_to_trash(record, rel, trash):
    root = Path(record['root'])
    folder = root / trash
    folder.mkdir(parents=True, exist_ok=True)
    name = Path(rel).name
    target, n = folder / name, 2
    while target.exists():
        target = folder / '{} {}{}'.format(Path(name).stem, n, Path(name).suffix)
        n += 1
    os.rename(root / rel, target)


def _update_units_map(catalog, work_record, flow_id):
    """work.dng says where a DNG is in work now (sample) or null (archive only)."""
    flow = mission_flow(recipe(catalog), flow_id)
    rel = work_record['prefix'].strip('/') + '/' + flow['units_map'].strip('/')
    base = os.path.dirname(os.path.dirname(rel))
    with Tree(work_record['root']) as tree:
        lines = _units(tree, rel)
        for unit in lines:
            jpg = unit.get('work', {}).get('jpg')
            dng = str(Path(jpg).with_suffix('.DNG')) if jpg else None
            unit.setdefault('work', {})['dng'] = dng if dng and (Path(work_record['root']) / base / dng).is_file() else None
        data = ''.join(dumps(u) + '\n' for u in lines).encode('utf-8')
        tree.write_bytes(rel, data)
        require(tree.read_bytes(rel) == data, 'checksum_mismatch', 'units map did not read back identical')
