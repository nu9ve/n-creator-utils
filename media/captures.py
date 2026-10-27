"""Incremental manifest discovery for a configured Stalker library."""
import os
from pathlib import Path
import uuid

from .common import digest, MediaError, content_stamp, read_json, require


def project_hint(manifest):
    value = manifest.get('project')
    if not isinstance(value, dict) or value.get('version') != 1:
        return {}
    return {k: value[k] for k in ('name', 'country') if isinstance(value.get(k), str)}


def capture_libraries(catalog, override='', registry=None):
    """Producer registry is read-only. A manual root augments, rather than hides, old roots."""
    marker = Path(registry) if registry else Path.home() / 'Library/Application Support/Stalker/library.json'
    roots, errors = [], []
    if override:
        roots.append(str(Path(override).expanduser().absolute()))
    if marker.exists():
        try:
            data = read_json(marker, 1024 * 1024)
            require(data.get('schema') == 'stalker-library/1', 'unsupported_library', 'Unsupported Stalker library registry')
            for value in [data.get('path'), *(data.get('previous') or [])]:
                path = value.get('path') if isinstance(value, dict) else value
                if isinstance(path, str) and Path(path).expanduser().is_absolute():
                    roots.append(str(Path(path).expanduser().absolute()))
        except (OSError, MediaError, TypeError, ValueError) as error:
            errors.append(str(error))
    if not roots:
        roots.append(str(Path.home() / 'Movies/Stalker'))
    sessions = {}
    available = []
    for root in dict.fromkeys(roots):
        try:
            data = captures(catalog, root)
            available.append(root)
            for session in data['sessions']:
                sessions[session['path']] = dict(session, root=root)
        except (OSError, MediaError) as error:
            errors.append(root + ': ' + str(error))
    values = sorted(sessions.values(), key=lambda s: (s.get('created_at') or '', s['path']), reverse=True)
    return {'roots': available, 'sessions': values,
            'pending': [s for s in values if s.get('status') == 'completed' and not s['archived']], 'errors': errors}


def save_capture_tag(catalog, session_id, project, country='mx'):
    from .common import slug
    sid = str(uuid.UUID(session_id))
    value = {'name': slug(project), 'country': slug(country)}
    with catalog.writer():
        catalog.set('capture-tag:' + sid, value)
    return value


def captures(catalog, root, changed_paths=None):
    root = Path(root).expanduser().absolute()
    require(root.is_dir() and not root.is_symlink(), 'capture_root_unavailable', 'Capture library is unavailable')
    key = 'capture-index:' + str(root)
    previous = catalog.get(key, {})
    candidates = set()
    if changed_paths:
        for raw in changed_paths[:10000]:
            path = Path(raw).absolute()
            if root != path and root not in path.parents: continue
            while path != root and path.suffix.lower() != '.stalker': path = path.parent
            if path.suffix.lower() == '.stalker': candidates.add(str(path))
    else:
        with os.scandir(root) as entries:
            for entry in entries:
                if entry.name.lower().endswith('.stalker') and entry.is_dir(follow_symlinks=False) and not entry.is_symlink(): candidates.add(entry.path)
        if root.suffix.lower() == '.stalker': candidates.add(str(root))
        previous = {p: v for p, v in previous.items() if p in candidates}
    for raw in candidates:
        folder = Path(raw)
        if not folder.is_dir() or folder.is_symlink(): previous.pop(raw, None); continue
        marker = folder / 'manifest.json'
        try:
            stamp = content_stamp(os.stat(marker, follow_symlinks=False))
            if previous.get(raw, {}).get('stamp') == stamp and 'project_hint' in previous[raw]: continue
            manifest = read_json(marker, 1024 * 1024)
            require(manifest.get('producer') == 'stalker' and manifest.get('schemaVersion') in {1, 2, 3}, 'invalid_bundle', 'Unsupported capture')
            sid = str(uuid.UUID(manifest['sessionID']))
            previous[raw] = {'path': raw, 'session_id': sid, 'status': manifest.get('status'), 'manifest_digest': digest(manifest), 'stamp': stamp,
                             'project_hint': project_hint(manifest), 'created_at': manifest.get('createdAt'),
                             'track_count': len(manifest.get('tracks') or [])}
        except (OSError, ValueError, KeyError, TypeError, MediaError):
            previous[raw] = {'path': raw, 'status': 'needs_review'}
    if previous != catalog.get(key, {}): catalog.set(key, previous)
    # Plans keep the manifest's spelling of sessionID (Stalker writes uppercase); compare as UUIDs.
    archived = {(row[0].lower(), row[1]) for row in catalog.db.execute('SELECT session_id,manifest_digest FROM sessions')}
    sessions = [dict(value, archived=(value.get('session_id'), value.get('manifest_digest')) in archived) for value in previous.values()]
    for value in sessions:
        value['tag'] = catalog.get('capture-tag:' + value.get('session_id', ''), {})
    return {'sessions': sessions, 'pending': [s for s in sessions if s.get('status') == 'completed' and not s['archived']]}
