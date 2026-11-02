"""Finals on an online work disk against the archive's profile folder, by name and size; nothing here writes media.

Only what the person already moved to the published folder (`subidos`) is due for backup: the rest is still in use.
"""
import datetime
import os
from pathlib import Path
import re

from .common import MediaError, now, require
from .layout import MEDIA_KINDS, kind_of
from .volumes import independent

LISTED = 200
OPAQUE = {'.fcpbundle', '.stalker', '.app', '.photoslibrary'}
PUBLISHED_NAME = re.compile(r'^(\d+[_-])?(subidos|publicados)$', re.IGNORECASE)


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


def year_in_name(name):
    """26-10-27-titulo, 2026-10-27-titulo or 261027_titulo. A re-export keeps its name but not its dates."""
    match = re.match(r'^(?:(\d{4})|(\d{2}))-(\d{2})-(\d{2})(?!\d)', name) or re.match(r'^(\d{0})(\d{2})(\d{2})(\d{2})(?=[_-])', name)
    if not match:
        return None
    long_year, short_year, month, day = match.groups()
    if not (1 <= int(month) <= 12 and 1 <= int(day) <= 31):
        return None
    return long_year if len(long_year or '') == 4 else '20' + short_year


def created_year(path):
    info = os.stat(path, follow_symlinks=False)
    return str(datetime.datetime.fromtimestamp(getattr(info, 'st_birthtime', info.st_mtime)).year)


def route_files(archive, profile, source, files):
    """Where each published file goes, as `<profile>/` in the archive's finals: subfolders stay, loose masters go under their year.

    Skips what the profile's archive folder already has elsewhere under the same name and size, and keeps what sits exactly
    at its destination so the copy step can verify and adopt it. A different file there still blocks the plan.
    """
    source = Path(source)
    root = Path(archive['root']) / archive.get('finals_prefix', '002_finales')
    existing = {(path.name, info.st_size): path for path, info in _walk(root / profile, None)}
    layout, kept, skipped, taken = {}, [], [], set()
    for file in files:
        rel = file['path']
        name = Path(rel).name
        target = profile + '/' + (rel if '/' in rel else (year_in_name(name) or created_year(source / rel)) + '/' + name)
        found = existing.get((name, file['stamp']['size']))
        if found is not None and found != root / target:
            skipped.append(rel)
            continue
        require(target.lower() not in taken, 'destination_conflict', 'Two finals would land on ' + target)
        taken.add(target.lower())
        layout[rel] = {'archive': target, 'work': None}
        kept.append(file)
    return kept, layout, skipped


def clipper_blockers(catalog, source):
    """A concrete use stops the move. Not knowing does not: the verified copy is one restore away, unlike a raw with no backup."""
    from . import clipper
    try:
        report = clipper.inspect(catalog, {str(source): None})
    except MediaError:
        return []
    return [value for value in report['blockers'] if value != clipper.OFFLINE_PROJECTS]


def plan_params(catalog, backend, profile, archive=None, work=None, release=True):
    """build_plan arguments for one profile's published folder. Moving is the point of publishing: `release` frees the
    editing disk in the same reviewed apply, through the same verified, re-read, journaled cleanup as a card."""
    require(isinstance(release, bool), 'invalid_policy', 'release must be boolean')
    require(isinstance(profile, str) and profile == Path(profile).name and not profile.startswith('.'), 'invalid_folder', 'Invalid profile')
    volumes = [v for v in catalog.volumes() if backend.online(v)]
    targets = [v for v in volumes if v['role'] == 'archive' and (not archive or archive in {v['id'], v['name']})]
    require(targets, 'archive_offline', 'Connect the backup disk')
    for candidate in volumes:
        if candidate['role'] != 'work' or (work and work not in {candidate['id'], candidate['name']}):
            continue
        folder = Path(candidate['root']) / candidate.get('finals_prefix', '002_finales') / profile
        if not folder.is_dir() or folder.is_symlink():
            continue
        published = published_folder(catalog, profile, subfolders(folder))
        require(published, 'no_published_folder', 'Choose this profile\'s published folder in Flows first')
        if release:
            # Two folders on one disk are one copy.
            require(independent(backend.check(targets[0]), backend.check(candidate)), 'same_physical_disk',
                    'Backup and editing disk are the same physical disk; keep the source instead')
            blockers = clipper_blockers(catalog, folder / published)
            require(not blockers, 'source_in_use', 'Clipper uses these finals: ' + '; '.join(blockers[:3]))
        return dict(source=str(folder / published), archive=targets[0]['id'], work=None, project='finales', profile=profile,
                    archive_only=True, confirm_archive_only=True, work_decision='skipped', cleanup=release, camera='generic',
                    source_kind='delivery', finals_route=True)
    require(False, 'work_offline', 'This profile has no finals on a connected editing disk')


def subfolders(profile):
    return sorted(p.name for p in profile.iterdir() if p.is_dir() and not p.is_symlink() and not p.name.startswith('.') and p.suffix.lower() not in OPAQUE)


def published_folder(catalog, profile, names):
    """An explicit choice wins, including '' for none; a chosen folder that no longer exists means none, never everything."""
    chosen = catalog.get('finals-published:' + profile)
    if chosen is not None:
        return chosen if chosen in names else ''
    return next((name for name in names if PUBLISHED_NAME.match(name)), '')


def set_published_folder(catalog, profile, folder):
    """folder: a direct subfolder name, '' for none, or None to go back to detecting it."""
    for value in (profile, folder or 'x'):
        require(isinstance(value, str) and value == Path(value).name and value not in {'.', '..'} and not value.startswith('.'),
                'invalid_folder', 'Use a folder directly inside the profile')
    catalog.set('finals-published:' + profile, folder)
    return {'profile': profile, 'folder': folder}


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
            names = subfolders(profile)
            published = published_folder(catalog, profile.name, names)
            total = pending_files = pending_bytes = waiting = waiting_bytes = in_archive = in_archive_bytes = published_total = 0
            listed = []
            for path, info in _walk(profile, cancel):
                total += 1
                inside = bool(published) and path.relative_to(profile).parts[0] == published
                published_total += inside
                if (path.name, info.st_size) in archived:
                    in_archive += 1
                    in_archive_bytes += info.st_size
                elif published and not inside:
                    waiting += 1
                    waiting_bytes += info.st_size
                else:
                    pending_files += 1
                    pending_bytes += info.st_size
                    if len(listed) < LISTED:
                        listed.append({'path': str(path), 'name': str(path.relative_to(profile)), 'bytes': info.st_size})
            if total or published:
                result['groups'].append({'work': work['name'], 'profile': profile.name, 'folder': str(profile),
                                         'destination': target['name'] + '/' + prefix + '/' + profile.name,
                                         'published_folder': published, 'subfolders': names, 'published_total': published_total,
                                         'total': total, 'pending': pending_files, 'pending_bytes': pending_bytes,
                                         'waiting': waiting, 'waiting_bytes': waiting_bytes,
                                         'in_archive': in_archive, 'in_archive_bytes': in_archive_bytes, 'files': listed})
    return result
