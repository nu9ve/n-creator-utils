"""Mission registry: exports of a flight app merged into one history on the archive (Atlas A13).

The producer (for the user, subaereo) exports full or partial JSON. Every record has a stable id, so
merging is an upsert per list (`merge_keys`); when two exports carry the same record, the newest
`exportedAt` wins. Rules, paths and patterns come from the recipe's `missions` flow; nothing here names
a producer.

On the archive (`archive_area`, below the volume root):
  history.json    the merged export, compatible with the producer's own merge tool, plus a `nuun`
                  block: sources by SHA-256, which source holds each record, tombstones, changes count
  changes.ndjson  one line per record that became new, updated or deleted, with its export
  raw/AAAA/MM/    each incorporated export, gzip of its exact bytes (lets the union be redone)
A byte-identical mirror of history.json and changes.ndjson goes to the work volume (`work_mirror`) so a
consumer (Atlas) reads it without the archive mounted.

`review` is read-only and returns a digest. `incorporate` requires that digest, writes raw → changes →
history (history records how many change lines it covers, so an interrupted run never leaves extra lines),
mirrors, and only then removes from the inbox the files it verified there. Files outside the inbox are
never deleted.
"""
import copy
import datetime
import fnmatch
import gzip
import hashlib
import json
import os
from pathlib import Path

from .common import MediaError, Tree, content_stamp, digest, now, require
from .recipes import mission_flow, recipe

HISTORY_SCHEMA = 'nu9ve-mission-history/1'
MAX_EXPORT = 64 * 1024 * 1024
TOP_FIELDS = ('schemaVersion', 'exportedAt', 'exportedAtLocal', 'timeZoneIdentifier', 'cameraCapabilities')
EPOCH = datetime.datetime.min.replace(tzinfo=datetime.timezone.utc)


def parse_date(text):
    if not isinstance(text, str) or not text:
        return None
    try:
        value = datetime.datetime.fromisoformat(text.replace('Z', '+00:00'))
    except ValueError:
        return None
    return value if value.tzinfo else value.replace(tzinfo=datetime.timezone.utc)


def dumps(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))


def window_kind(export):
    kind = (export.get('exportWindow') or {}).get('kind')
    if kind:
        return kind
    # Older exports: a full history unless it holds a single plan.
    return 'plan' if len(export.get('plans') or []) == 1 else 'all'


def validate(data, flow):
    require(isinstance(data, dict), 'invalid_export', 'Not a JSON object')
    require(data.get('schemaVersion') in flow['schema_versions'], 'invalid_export',
            'Unsupported schemaVersion: {}'.format(data.get('schemaVersion')))
    require(parse_date(data.get('exportedAt')) is not None, 'invalid_export', 'exportedAt missing or invalid')
    require(isinstance(data.get('plans'), list), 'invalid_export', 'plans list missing')
    for key, field in flow['merge_keys'].items():
        records = data.get(key)
        if records is None:
            continue
        require(isinstance(records, list) and all(isinstance(r, dict) for r in records), 'invalid_export', key + ' must be a list of objects')
        require(all(isinstance(r.get(field), (str, int)) for r in records), 'invalid_export', key + ' records need ' + field)
    for key, field in (flow.get('tombstones') or {}).items():
        require(data.get(field) is None or isinstance(data[field], list), 'invalid_export', field + ' must be a list')


def summary(data, flow):
    log = data.get('captureLog') or data.get('captures') or []
    dates = sorted(r['capturedAt'] for r in log if isinstance(r.get('capturedAt'), str))
    return {'exportedAt': data['exportedAt'], 'kind': window_kind(data), 'schemaVersion': data['schemaVersion'],
            'plans': len(data.get('plans') or []), 'photos': len(log),
            'first': dates[0] if dates else None, 'last': dates[-1] if dates else None}


def empty_state(flow):
    keys = flow['merge_keys']
    return {'top': {'schemaVersion': max(flow['schema_versions'])}, 'lists': {k: {} for k in keys},
            'nuun': {'schema': HISTORY_SCHEMA, 'flow': flow['id'], 'sources': {}, 'records': {k: {} for k in keys},
                     'tombstones': {k: {} for k in (flow.get('tombstones') or {})}, 'changes_count': 0}}


def load_state(raw, flow):
    if raw is None:
        return empty_state(flow)
    history = json.loads(raw)
    nuun = history.get('nuun') or {}
    require(nuun.get('schema') == HISTORY_SCHEMA and nuun.get('flow') == flow['id'], 'invalid_history',
            'The mission history on the archive is not a nüün registry for this flow')
    state = empty_state(flow)
    state['top'] = {k: history[k] for k in TOP_FIELDS if k in history}
    for key, field in flow['merge_keys'].items():
        state['lists'][key] = {str(r[field]): r for r in history.get(key) or []}
        state['nuun']['records'][key] = dict(nuun.get('records', {}).get(key) or {})
    for key in state['nuun']['tombstones']:
        state['nuun']['tombstones'][key] = dict(nuun.get('tombstones', {}).get(key) or {})
    state['nuun']['sources'] = dict(nuun.get('sources') or {})
    state['nuun']['changes_count'] = int(nuun.get('changes_count') or 0)
    return state


def render(state, flow):
    """history.json bytes: the producer's merged shape (schema, lists, exportWindow, mergedFrom) + `nuun`."""
    sources = sorted(state['nuun']['sources'].values(), key=lambda s: (parse_date(s['exportedAt']) or EPOCH, s['sha256']))
    history = dict(state['top'])
    history['exportWindow'] = {'kind': 'merged', 'since': None, 'until': history.get('exportedAt'), 'mergeKeys': flow['merge_keys']}
    history['mergedFrom'] = [{'file': s['file'], 'exportedAt': s['exportedAt'], 'kind': s['kind'], 'sha256': s['sha256']} for s in sources]
    for key in flow['merge_keys']:
        date = flow['record_dates'].get(key)
        history[key] = sorted(state['lists'][key].values(), key=lambda r: (str(r.get(date) or ''), str(r.get(flow['merge_keys'][key]))))
    history['nuun'] = state['nuun']
    return (dumps(history) + '\n').encode('utf-8')


def merge(state, data, sha, name, flow, raw_rel=None):
    """Upsert one export into `state`; returns its change records. Never lets an older export win."""
    incoming = parse_date(data['exportedAt'])
    nuun, changes = state['nuun'], []

    def stored_at(key, rid):
        source = nuun['sources'].get(nuun['records'][key].get(rid))
        return parse_date(source['exportedAt']) if source else EPOCH

    def change(key, rid, kind):
        changes.append({'list': key, 'id': rid, 'change': kind, 'export': name, 'export_sha256': sha, 'exportedAt': data['exportedAt']})

    for key, field in flow['merge_keys'].items():
        for record in data.get(key) or []:
            rid = str(record[field])
            tomb = nuun['tombstones'].get(key, {}).get(rid)
            if tomb and parse_date(tomb['exportedAt']) >= incoming:
                continue  # deleted by a newer export: an older one never brings it back
            stored = state['lists'][key].get(rid)
            if stored is None:
                state['lists'][key][rid] = record
                nuun['records'][key][rid] = sha
                change(key, rid, 'new')
            elif incoming >= stored_at(key, rid) and dumps(stored) != dumps(record):
                state['lists'][key][rid] = record
                nuun['records'][key][rid] = sha
                change(key, rid, 'updated')
    for key, field in (flow.get('tombstones') or {}).items():
        for rid in data.get(field) or []:
            rid = str(rid)
            if rid in state['lists'][key] and incoming >= stored_at(key, rid):
                del state['lists'][key][rid]
                nuun['records'][key].pop(rid, None)
                change(key, rid, 'deleted')
            previous = nuun['tombstones'][key].get(rid)
            if not previous or parse_date(previous['exportedAt']) < incoming:
                nuun['tombstones'][key][rid] = {'export_sha256': sha, 'exportedAt': data['exportedAt']}
    newest = max((parse_date(s['exportedAt']) for s in nuun['sources'].values()), default=EPOCH)
    if incoming >= newest:
        state['top'] = {k: data[k] for k in TOP_FIELDS if k in data}
    counts = {kind: sum(c['change'] == kind for c in changes) for kind in ('new', 'updated', 'deleted')}
    nuun['sources'][sha] = dict(summary(data, flow), file=name, sha256=sha, raw=raw_rel, incorporated_at=now(), changes=counts)
    return changes


def coverage(state):
    """What the registry covers: newest export, last capture (UTC) and last camera time (local, EXIF-like)."""
    records = list(state['lists'].get('captureLog', {}).values()) + list(state['lists'].get('captures', {}).values())
    captured = [r['capturedAt'] for r in records if isinstance(r.get('capturedAt'), str)]
    media = [r['mediaCreatedAt'] for r in records if isinstance(r.get('mediaCreatedAt'), str)]
    exported = [s['exportedAt'] for s in state['nuun']['sources'].values()]
    return {'exported_until': max(exported, key=lambda v: parse_date(v) or EPOCH) if exported else None,
            'last_capture_at': max(captured, key=lambda v: parse_date(v) or EPOCH) if captured else None,
            'last_media_created_at': max(media) if media else None,
            'records': {k: len(v) for k, v in state['lists'].items()}, 'sources': len(state['nuun']['sources'])}


def _paths(flow):
    area = flow['archive_area'].strip('/')
    return area + '/history.json', area + '/changes.ndjson'


def _read_export(path):
    path = Path(path)
    with Tree(path.parent) as tree:
        st = tree.stat(path.name)
        require(st.st_size <= MAX_EXPORT, 'file_too_large', 'Export larger than 64 MiB: ' + path.name)
        raw = tree.read_bytes(path.name, MAX_EXPORT)
    return raw, content_stamp(st)


def _candidates(flow, paths):
    inbox = Path(flow['inbox']).expanduser().absolute()
    if paths is not None:
        require(isinstance(paths, list) and all(isinstance(p, str) for p in paths), 'invalid_request', 'paths must be a list')
        return inbox, [Path(p).expanduser().absolute() for p in paths]
    try:
        with os.scandir(inbox) as entries:
            found = [Path(e.path) for e in entries if e.is_file(follow_symlinks=False)
                     and any(fnmatch.fnmatch(e.name, pattern) for pattern in flow['patterns'])]
    except FileNotFoundError:
        found = []
    return inbox, sorted(found)


def _archive_state(catalog, backend, archive, flow):
    record = catalog.volume(archive)
    require(record['role'] == 'archive', 'invalid_role', 'The mission registry lives on the archive')
    backend.check(record)
    history_rel, changes_rel = _paths(flow)
    with Tree(record['root']) as tree:
        state = load_state(tree.read_bytes(history_rel), flow)
        changes = tree.read_bytes(changes_rel) or b''
    return record, state, changes


def review(catalog, backend=None, archive=None, paths=None, flow_id=None):
    """Read-only: what each export brings to the registry. The digest authorizes exactly this incorporation."""
    flow = mission_flow(recipe(catalog), flow_id)
    inbox, candidates = _candidates(flow, paths)
    state = None
    if archive is not None:
        _, state, _ = _archive_state(catalog, backend, archive, flow)
    files, loaded = [], []
    for path in candidates:
        entry = {'path': str(path), 'name': path.name, 'in_inbox': path.parent == inbox}
        try:
            raw, stamp = _read_export(path)
            data = json.loads(raw)
            validate(data, flow)
        except (OSError, ValueError, MediaError) as error:
            entry.update(state='invalid', error=str(error))
            files.append(entry)
            continue
        entry.update(sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw), stamp=stamp, **summary(data, flow))
        files.append(entry)
        loaded.append((entry, data))
    if state is not None:
        trial = copy.deepcopy(state)
        for entry, data in sorted(loaded, key=lambda item: parse_date(item[1]['exportedAt'])):
            if entry['sha256'] in trial['nuun']['sources']:
                entry.update(state='incorporated', changes={'new': 0, 'updated': 0, 'deleted': 0})
                continue
            changes = merge(trial, data, entry['sha256'], entry['name'], flow)
            entry['changes'] = {kind: sum(c['change'] == kind for c in changes) for kind in ('new', 'updated', 'deleted')}
            entry['state'] = 'new' if changes else 'nothing_new'
        registry = coverage(trial)
    else:
        for entry, _ in loaded:
            entry['state'] = 'unknown_registry'
        registry = None
    result = {'schema': 'nu9ve-missions-review/1', 'flow': flow['id'], 'label': flow['label'], 'inbox': str(inbox),
              'archive': archive, 'files': files, 'coverage_after': registry}
    result['digest'] = digest({'flow': flow['id'], 'archive': archive,
                               'files': [[f['path'], f.get('sha256'), f['state']] for f in files]})
    return result


def incorporate(catalog, backend, archive, digest_value, paths=None, work=None, flow_id=None, remove_from_inbox=True):
    """Merge the reviewed exports into the archive registry, mirror it, then clear verified inbox files."""
    flow = mission_flow(recipe(catalog), flow_id)
    history_rel, changes_rel = _paths(flow)
    with catalog.writer():
        reviewed = review(catalog, backend, archive, paths, flow_id)
        require(reviewed['digest'] == digest_value, 'review_changed', 'The exports or the registry changed; review again')
        record, state, change_bytes = _archive_state(catalog, backend, archive, flow)
        lines = [line for line in change_bytes.decode('utf-8').splitlines() if line.strip()]
        lines = lines[:state['nuun']['changes_count']]  # drop lines of an import that never reached history.json
        valid = [f for f in reviewed['files'] if f['state'] != 'invalid']
        incorporated, added = [], []
        with Tree(record['root']) as tree:
            for entry in sorted(valid, key=lambda f: parse_date(f['exportedAt'])):
                raw, stamp = _read_export(entry['path'])
                require(hashlib.sha256(raw).hexdigest() == entry['sha256'] and stamp == entry['stamp'], 'source_changed',
                        'An export changed after review: ' + entry['name'])
                if entry['sha256'] in state['nuun']['sources']:
                    continue
                raw_rel = _store_raw(tree, flow, entry, raw)
                changes = merge(state, json.loads(raw), entry['sha256'], entry['name'], flow, raw_rel)
                stamp_at = now()
                added.extend(dumps(dict(c, at=stamp_at, flow=flow['id'])) for c in changes)
                incorporated.append(entry['name'])
            if incorporated:
                lines += added
                change_out = ('\n'.join(lines) + '\n').encode('utf-8') if lines else b''
                state['nuun']['changes_count'] = len(lines)
                history_out = render(state, flow)
                tree.write_bytes(changes_rel, change_out)
                tree.write_bytes(history_rel, history_out)
                require(tree.read_bytes(history_rel) == history_out and tree.read_bytes(changes_rel) == change_out,
                        'checksum_mismatch', 'The mission registry did not read back identical')
            else:
                history_out = tree.read_bytes(history_rel) or render(state, flow)
                change_out = tree.read_bytes(changes_rel) or b''
        mirror = _mirror(catalog, backend, work, flow, history_out, change_out)
        removed, kept = [], []
        for entry in reviewed['files']:
            # Only a verified inbox file whose bytes are in the registry (now or before) leaves the inbox.
            if entry['state'] != 'invalid' and entry['in_inbox'] and remove_from_inbox and entry['sha256'] in state['nuun']['sources']:
                path = Path(entry['path'])
                with Tree(path.parent) as inbox:
                    inbox.unlink(path.name, expected=entry['stamp'])
                removed.append(entry['name'])
            else:
                kept.append(entry['name'])
        summary_value = {'flow': flow['id'], 'archive_id': record['id'], 'updated_at': now(),
                         'history_sha256': hashlib.sha256(history_out).hexdigest(), 'mirror': mirror,
                         'work_id': catalog.volume(work)['id'] if work else None, 'coverage': coverage(state)}
        catalog.set('mission-registry:' + flow['id'], summary_value)
    return {'incorporated': incorporated, 'changes': {k: sum(json.loads(l)['change'] == k for l in added) for k in ('new', 'updated', 'deleted')},
            'removed_from_inbox': removed, 'kept': kept, 'invalid': [f['name'] for f in reviewed['files'] if f['state'] == 'invalid'],
            'mirror': mirror, 'coverage': summary_value['coverage'], 'history': history_rel}


def _store_raw(tree, flow, entry, raw):
    """gzip of the exact export below raw/AAAA/MM/, verified by decompressing; an identical copy is adopted."""
    when = parse_date(entry['exportedAt']).astimezone(datetime.timezone.utc)
    folder = '{}/raw/{}'.format(flow['archive_area'].strip('/'), when.strftime('%Y/%m'))
    packed = gzip.compress(raw, mtime=0)
    for name in (entry['name'] + '.gz', '{}-{}.gz'.format(entry['name'], entry['sha256'][:8])):
        rel = folder + '/' + name
        existing = tree.read_bytes(rel)
        if existing is not None:
            if hashlib.sha256(gzip.decompress(existing)).hexdigest() == entry['sha256']:
                return rel
            continue
        tree.write_bytes(rel, packed, replace=False)
        require(hashlib.sha256(gzip.decompress(tree.read_bytes(rel))).hexdigest() == entry['sha256'],
                'checksum_mismatch', 'Raw export did not verify: ' + name)
        return rel
    raise MediaError('destination_exists', 'Two different exports share the name ' + entry['name'])


def _mirror(catalog, backend, work, flow, history_out, change_out):
    if not work:
        return 'skipped'
    record = catalog.volume(work)
    require(record['role'] == 'work', 'invalid_role', 'The mirror goes to a work volume')
    try:
        backend.check(record)
    except MediaError as error:
        if error.code in {'volume_offline', 'volume_replaced'}:
            return 'pending'
        raise
    base = record['prefix'].strip('/') + '/' + flow['work_mirror'].strip('/')
    with Tree(record['root']) as tree:
        for name, data in (('changes.ndjson', change_out), ('history.json', history_out)):
            if tree.read_bytes(base + '/' + name) != data:
                tree.write_bytes(base + '/' + name, data)
            require(tree.read_bytes(base + '/' + name) == data, 'checksum_mismatch', 'Mirror did not read back identical')
    return 'written'


def status(catalog, flow_id=None):
    flow = mission_flow(recipe(catalog), flow_id)
    return catalog.get('mission-registry:' + flow['id'], {}) or {'flow': flow['id'], 'coverage': None}


# --- Matching stills to missions (Atlas A12) -------------------------------------------------------------

def _local(record):
    """Camera wall clock of a record as naive local time: mediaCreatedAt, else capturedAtLocal, else capturedAt."""
    value = record.get('mediaCreatedAt')
    if isinstance(value, str):
        try:
            return datetime.datetime.fromisoformat(value.replace(' ', 'T'))
        except ValueError:
            pass
    value = record.get('capturedAtLocal')
    if isinstance(value, str):
        try:
            return datetime.datetime.fromisoformat(value).replace(tzinfo=None, microsecond=0)
        except ValueError:
            pass
    when = parse_date(record.get('capturedAt'))
    return when.astimezone().replace(tzinfo=None, microsecond=0) if when else None


def _position(record):
    for lat, lon in (('latitude', 'longitude'), ('actualLatitude', 'actualLongitude')):
        if isinstance(record.get(lat), (int, float)) and isinstance(record.get(lon), (int, float)):
            return record[lat], record[lon]
    return None


def meters(a, b):
    import math
    lat1, lon1, lat2, lon2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    h = math.sin((lat2 - lat1) / 2) ** 2 + math.cos(lat1) * math.cos(lat2) * math.sin((lon2 - lon1) / 2) ** 2
    return 6371008.8 * 2 * math.asin(math.sqrt(h))


def _stem(name):
    return Path(name).stem.upper() if isinstance(name, str) and name else None


class MissionIndex:
    """Registry records by original file name, for the time/name/position rule of the flow."""

    def __init__(self, history, flow, history_sha256=None):
        self.flow, self.history_sha256 = flow, history_sha256
        plans = {str(p.get('id')): p for p in history.get('plans') or []}
        for plan in plans.values():  # schema 1–2 exports nest captures inside their plan
            for record in plan.get('captures') or []:
                record.setdefault('planID', plan.get('id'))
        records = list(history.get('captureLog') or []) + list(history.get('captures') or []) + \
            [r for p in plans.values() for r in p.get('captures') or []]
        self.by_name, self.sessions = {}, {}
        for record in records:
            stem, when = _stem(record.get('imageJPEGReference') or record.get('imageReference')), _local(record)
            if not stem or when is None:
                continue
            plan = plans.get(str(record.get('planID'))) or {}
            session = record.get('sessionID') or record.get('flightRunID') or record.get('planID')
            # Older exports only carry the command time, a few seconds after the camera's own clock.
            exact = isinstance(record.get('mediaCreatedAt'), str)
            entry = {'when': when, 'exact': exact, 'position': _position(record), 'planID': record.get('planID'), 'sessionID': session,
                     'captureID': record.get('captureID'), 'plan_name': plan.get('name') or record.get('planName') or 'MISION'}
            self.by_name.setdefault(stem, []).append(entry)
            if session and (session not in self.sessions or when < self.sessions[session]):
                self.sessions[session] = when
        fulls = [parse_date(s['exportedAt']) for s in (history.get('nuun') or {}).get('sources', {}).values() if s.get('kind') in ('all', 'merged')]
        newest = max(fulls) if fulls else None
        # A full export covers every photo taken before it; later photos wait for the next export.
        self.covered_until = newest.astimezone().replace(tzinfo=None) if newest else None

    def match(self, photo, original_name):
        when = photo.get('captured_at')
        if not when:
            return {'state': 'pending_export', 'reason': 'no_capture_time'}
        when = datetime.datetime.fromisoformat(when)
        rule, best = self.flow['match'], None
        for entry in self.by_name.get(_stem(original_name), []):
            delta = abs((entry['when'] - when).total_seconds())
            if delta > (rule['seconds'] if entry['exact'] else rule.get('seconds_command_time', rule['seconds'])):
                continue
            gps = photo.get('gps')
            if gps and entry['position'] and meters((gps['lat'], gps['lon']), entry['position']) > rule['meters']:
                continue
            if best is None or delta < best[0]:
                best = (delta, entry)
        if best:
            entry = best[1]
            start = self.sessions.get(entry['sessionID'], entry['when'])
            return {'state': 'matched', 'planID': entry['planID'], 'sessionID': entry['sessionID'], 'captureID': entry['captureID'],
                    'plan_name': entry['plan_name'], 'session_start': start.isoformat()}
        if self.covered_until and when <= self.covered_until:
            return {'state': 'none'}
        return {'state': 'pending_export', 'likely_mission': likely_mission(photo, self.flow)}


def likely_mission(photo, flow):
    """Cheap evidence only, never a classification: nadir gimbal at mission height."""
    hint = flow.get('hint') or {}
    flight = photo.get('flight') or {}
    pitch, height = flight.get('gimbal_pitch'), flight.get('relative_altitude')
    return (pitch is not None and pitch <= hint.get('max_gimbal_pitch', -80)
            and height is not None and height >= hint.get('min_relative_altitude', 15))


def index_for(archive_record, flow):
    """The archive's mission registry for planning, or an empty index when none exists yet."""
    history_rel, _ = _paths(flow)
    with Tree(archive_record['root']) as tree:
        raw = tree.read_bytes(history_rel)
    if raw is None:
        return MissionIndex({}, flow)
    return MissionIndex(json.loads(raw), flow, hashlib.sha256(raw).hexdigest())


# --- units.ndjson for the consumer (Atlas A12) ------------------------------------------------------------

UNIT_SCHEMA = 'nu9ve-drone-unit/1'


def _utc(local_text, offset=None):
    """UTC of a camera wall-clock time: the EXIF offset when present, else the host's zone (stated as such)."""
    when = datetime.datetime.fromisoformat(local_text + (offset or ''))
    basis = 'exif_offset' if offset else 'host_zone'
    when = when if when.tzinfo else when.astimezone()
    return when.astimezone(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'), basis


def unit_entries(catalog, plan, map_rel):
    """One line per still unit of a mission flow, keyed by asset_id."""
    from .layout import kind_of
    from .planner import destination_path
    archive = catalog.volume(plan['archive_id'])
    work_id = plan.get('work_id') or plan.get('expected_work_id')
    work = catalog.volume(work_id) if work_id else None
    base = os.path.dirname(os.path.dirname(map_rel))  # the consumer's root: parent of `_nuun/`
    files = {f['path']: f for f in plan['files']}
    entries = []
    for unit in plan['units']:
        mission = unit.get('mission')
        if not mission:
            continue
        member = {}
        for path in unit['paths']:
            kind = kind_of(path)
            label = 'jpg' if kind == 'image' else 'dng' if kind == 'raw_image' else None
            if not label:
                continue
            row = catalog.db.execute('SELECT sha256 FROM members WHERE job_id=? AND relative_path=?', (plan['id'], path)).fetchone()
            work_rel = destination_path(work, plan, unit, path) if work else None
            member[label] = {'path': path, 'sha256': row[0] if row else None,
                             'archive': destination_path(archive, plan, unit, path),
                             'work': (work_rel[len(base) + 1:] if work_rel and work_rel.startswith(base + '/') else work_rel)}
        first = member.get('jpg') or member.get('dng')
        if not first:
            continue
        photo = files[first['path']].get('photo') or {}
        utc, basis = _utc(photo['captured_at'], photo.get('offset')) if photo.get('captured_at') else (None, None)
        device = unit.get('device') or {}
        entries.append({
            'schema': UNIT_SCHEMA, 'asset_id': unit['id'], 'job_id': plan['id'], 'project': unit['project'],
            'archive': {'volume': archive['name'], 'jpg': (member.get('jpg') or {}).get('archive'), 'dng': (member.get('dng') or {}).get('archive')},
            'work': {'volume': work['name'] if work else None, 'jpg': (member.get('jpg') or {}).get('work'), 'dng': (member.get('dng') or {}).get('work')},
            'original_name': Path(first['path']).name, 'dcim_folder': Path(first['path']).parent.name,
            'card_uuid': (plan.get('source_identity') or {}).get('volume_uuid'),
            'captured_at': photo.get('captured_at'), 'captured_at_utc': utc, 'utc_basis': basis,
            'make': device.get('make'), 'model': device.get('model'), 'serial': device.get('serial'),
            'sha256': {'jpg': (member.get('jpg') or {}).get('sha256'), 'dng': (member.get('dng') or {}).get('sha256')},
            'gps': photo.get('gps'), 'flight': photo.get('flight'),
            'mission': {k: mission[k] for k in ('planID', 'sessionID', 'captureID')} if mission['state'] == 'matched' else None,
            'mission_state': mission['state'], 'plan_name': mission.get('plan_name'), 'updated_at': now()})
    return entries


def write_units_map(catalog, backend, plan):
    """Upsert this job's drone units into the consumer's map on the work volume. 'pending' when it is offline."""
    flows = {f['id']: f for f in recipe(catalog).get('missions') or []}
    used = {u['mission']['flow'] for u in plan['units'] if u.get('mission')}
    work_id = plan.get('work_id') or plan.get('expected_work_id')
    if not used or not work_id:
        return 'skipped'
    record = catalog.volume(work_id)
    try:
        backend.check(record)
    except MediaError as error:
        if error.code in {'volume_offline', 'volume_replaced'}:
            catalog.set('units-map-pending:' + plan['id'], {'job_id': plan['id'], 'since': now()})
            return 'pending'
        raise
    with catalog.writer(), Tree(record['root']) as tree:
        for flow_id in sorted(used):
            rel = record['prefix'].strip('/') + '/' + flows[flow_id]['units_map'].strip('/')
            entries = {e['asset_id']: e for e in unit_entries(catalog, plan, rel)
                       if (next(u for u in plan['units'] if u['id'] == e['asset_id'])['mission']['flow'] == flow_id)}
            current = {}
            for line in (tree.read_bytes(rel) or b'').decode('utf-8').splitlines():
                if line.strip():
                    value = json.loads(line)
                    current[value['asset_id']] = value
            current.update(entries)
            ordered = sorted(current.values(), key=lambda e: (e.get('captured_at') or '', e['asset_id']))
            data = ''.join(dumps(e) + '\n' for e in ordered).encode('utf-8')
            tree.write_bytes(rel, data)
            require(tree.read_bytes(rel) == data, 'checksum_mismatch', 'units map did not read back identical')
        catalog.set('units-map-pending:' + plan['id'], None)
    return 'written'
