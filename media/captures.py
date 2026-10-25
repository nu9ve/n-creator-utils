"""Incremental manifest discovery for a configured Stalker library."""
import os
from pathlib import Path
import uuid

from .common import digest, MediaError, content_stamp, read_json, require


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
            if previous.get(raw, {}).get('stamp') == stamp: continue
            manifest = read_json(marker, 1024 * 1024)
            require(manifest.get('producer') == 'stalker' and manifest.get('schemaVersion') in {1, 2, 3}, 'invalid_bundle', 'Unsupported capture')
            sid = str(uuid.UUID(manifest['sessionID']))
            previous[raw] = {'path': raw, 'session_id': sid, 'status': manifest.get('status'), 'manifest_digest': digest(manifest), 'stamp': stamp}
        except (OSError, ValueError, KeyError, TypeError, MediaError):
            previous[raw] = {'path': raw, 'status': 'needs_review'}
    if previous != catalog.get(key, {}): catalog.set(key, previous)
    archived = {(row[0], row[1]) for row in catalog.db.execute('SELECT session_id,manifest_digest FROM sessions')}
    sessions = [dict(value, archived=(value.get('session_id'), value.get('manifest_digest')) in archived) for value in previous.values()]
    return {'sessions': sessions, 'pending': [s for s in sessions if s.get('status') == 'completed' and not s['archived']]}
