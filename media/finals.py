"""Read-only: finals on an online work disk with no same-name, same-size file in the archive's profile folder."""
import os
from pathlib import Path

from .common import now, require
from .layout import MEDIA_KINDS, kind_of

LISTED = 200
OPAQUE = {'.fcpbundle', '.stalker', '.app', '.photoslibrary'}


def _walk(root, cancel):
    """Names and sizes only; hidden entries and opaque bundles stay closed and symlinks are not followed."""
    for folder, directories, names in os.walk(root, followlinks=False):
        require(not (cancel and cancel()), 'cancelled', 'Finals check cancelled')
        directories[:] = sorted(d for d in directories if not d.startswith('.') and Path(d).suffix.lower() not in OPAQUE)
        for name in sorted(names):
            path = Path(folder) / name
            if name.startswith('.') or kind_of(name) not in MEDIA_KINDS:
                continue
            try:
                info = os.stat(path, follow_symlinks=False)
            except OSError:
                continue
            if os.path.isfile(path) and not path.is_symlink():
                yield path, info


def finals_status(catalog, backend, archive=None, cancel=None):
    """Destination layouts differ between disks (shorts/ vs year folders), so a relative path would prove nothing."""
    volumes = [dict(v, current=backend.online(v)) for v in catalog.volumes()]
    archives = [v for v in volumes if v['role'] == 'archive' and v['current'] and (not archive or archive in {v['id'], v['name']})]
    works = [v for v in volumes if v['role'] == 'work' and v['current']]
    result = {'checked_at': now(), 'archive': '', 'groups': [], 'reason': ''}
    if not archives or not works:
        return dict(result, reason='archive_offline' if not archives else 'work_offline')
    target = archives[0]
    result['archive'] = target['name']
    prefix = target.get('finals_prefix', '002_finales')
    archive_root = Path(target['root']) / prefix
    for work in works:
        root = Path(work['root']) / work.get('finals_prefix', '002_finales')
        if not root.is_dir() or root.is_symlink():
            continue
        for profile in sorted(p for p in root.iterdir() if p.is_dir() and not p.is_symlink() and not p.name.startswith('.')):
            archived = {(path.name, info.st_size) for path, info in _walk(archive_root / profile.name, cancel)}
            total = pending_files = pending_bytes = 0
            listed = []
            for path, info in _walk(profile, cancel):
                total += 1
                if (path.name, info.st_size) in archived:
                    continue
                pending_files += 1
                pending_bytes += info.st_size
                if len(listed) < LISTED:
                    listed.append({'path': str(path), 'name': str(path.relative_to(profile)), 'bytes': info.st_size})
            if total:
                result['groups'].append({'work': work['name'], 'profile': profile.name, 'folder': str(profile),
                                         'destination': target['name'] + '/' + prefix + '/' + profile.name,
                                         'total': total, 'pending': pending_files, 'pending_bytes': pending_bytes, 'files': listed})
    return result
