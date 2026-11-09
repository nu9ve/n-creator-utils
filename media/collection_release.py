"""Free space in a living collection (SSD1 recordings, libraries) against copies already on registered archives.

Unlike `release.py`, the source is not a nüün work disk and nothing here was copied by nüün: a file is released
only when a byte-identical copy (SHA-256, any name) exists on a registered archive, found by size first so that
files with no candidate are never hashed. A reviewed plan freezes what is released, what is kept and why, and what
has no copy. Apply re-verifies both sides of every file right before moving it to the volume's own Trash
(`.Trashes/<uid>/nuun-<plan>/<original path>`); nothing is unlinked, and emptying the Trash is the person's call.

Keep rules (any one keeps the file): an Ableton analysis file next to it (`<name>.asd`), a reference from an
Ableton set (`.als` under the given roots, matched by file name: a conservative match keeps more), an explicit
path prefix, or a Clipper dependency. Files that are not media are never touched.
"""
import gzip
import os
from pathlib import Path
import re
import stat
import zlib

from . import clipper
from .common import Tree, content_stamp, digest, full_sync, now, relative, require, uid
from .coverage import walk
from .layout import MEDIA_KINDS, kind_of
from .release import checked_hash, checked_hash_progress, report
from .volumes import independent

SCHEMA = 'nu9ve-collection-release/1'
KEY = 'collection-release:'
REFERENCE = re.compile(rb'Value="([^"]*\.(?:wav|aif|aiff|mp3|flac|m4a|ogg|alac))"', re.IGNORECASE)
ALS_LIMIT = 256 * 1024 * 1024  # decompressed bytes per set; a larger one is reported, not trusted
CLIPPER_RECHECK_SECONDS = 60


def mount_root(path):
    path = Path(path)
    while not os.path.ismount(path) and path != path.parent:
        path = path.parent
    return path


def ableton_references(roots, cancel=None):
    """Lowercase audio file names referenced by any `.als` under roots, plus sets that could not be read."""
    names, unreadable, sets = set(), [], 0
    for root in roots or []:
        root = Path(root).expanduser().absolute()
        require(root.is_dir(), 'invalid_request', 'Ableton folder not found: ' + str(root))
        for folder, dirs, files in os.walk(root):
            require(not (cancel and cancel()), 'cancelled', 'Collection release planning cancelled')
            dirs[:] = [d for d in dirs if not d.startswith('.')]
            for name in files:
                if not name.lower().endswith('.als') or name.startswith('._'):
                    continue
                path = os.path.join(folder, name)
                try:
                    with gzip.open(path) as stream:
                        data = stream.read(ALS_LIMIT + 1)
                    require(len(data) <= ALS_LIMIT, 'invalid_request', 'Set too large')
                except (OSError, EOFError, zlib.error, ValueError) as error:
                    unreadable.append({'path': path, 'error': str(error)[:200]})
                    continue
                sets += 1
                for match in REFERENCE.finditer(data):
                    names.add(re.split(r'[/\\:]', match.group(1).decode('utf-8', 'replace'))[-1].lower())
    return {'names': names, 'sets': sets, 'unreadable': unreadable}


def _keep_reason(rel, siblings, references, prefixes):
    lowered = rel.lower()
    if lowered + '.asd' in siblings:
        return 'ableton_analysis'
    if os.path.basename(lowered) in references:
        return 'ableton_set'
    if any(rel == p or rel.startswith(p + '/') for p in prefixes):
        return 'explicit'
    return None


def _archive_index(catalog, backend, archives, search, source_info, cancel, progress):
    """{size: [(archive_id, path)]} over the given archives, metadata only."""
    index, records = {}, {}
    for archive_id in archives:
        record = catalog.volume(archive_id)
        require(record['role'] == 'archive', 'invalid_role', 'Archive required: ' + record['name'])
        info = backend.check(record)
        require(independent(source_info, info), 'same_physical_disk', 'Archive and source must be independent disks: ' + record['name'])
        records[record['id']] = record
        starts = [relative(s['path']) for s in search or [] if s.get('archive') in {record['id'], record['name']}] or ['']
        with Tree(record['root']) as tree:
            for start in starts:
                report(progress, 'indexing_archive', archive=record['name'], path=start)
                try:
                    for rel, size in walk(tree, start, cancel):
                        if size:
                            index.setdefault(size, []).append((record['id'], rel))
                except FileNotFoundError:
                    require(False, 'invalid_request', 'Search folder not found on {}: {}'.format(record['name'], start))
    return index, records


def plan(catalog, backend, source, archives, search=None, keep=None, cancel=None, progress=None):
    """Read-only on every disk: hashes, matches and records a plan in the catalog for review."""
    require(isinstance(archives, list) and archives, 'invalid_request', 'Choose at least one archive')
    keep = keep or {}
    root = Path(source).expanduser().absolute()
    require(root.is_dir() and not root.is_symlink(), 'invalid_request', 'Source folder not found: ' + str(root))
    for record in catalog.volumes():
        archive_root = Path(record['root'])
        require(root != archive_root and archive_root not in root.parents and root not in archive_root.parents,
                'overlapping_roots', 'Source overlaps a registered volume: ' + record['name'])
    report(progress, 'preparing')
    source_info = backend.describe(root)
    volume_root = Path(source_info.get('mount') or mount_root(root))
    require(root == volume_root or volume_root in root.parents, 'invalid_request', 'Source is not inside its volume')
    index, records = _archive_index(catalog, backend, archives, search, source_info, cancel, progress)
    refs = ableton_references(keep.get('als_roots'), cancel) if keep.get('als_roots') else {'names': set(), 'sets': 0, 'unreadable': []}
    prefixes = [relative(p) for p in keep.get('paths') or []]
    entries, kept, uncovered, preserved = [], [], [], []
    with Tree(root) as origin:
        listing = []
        for rel, size in walk(origin, '', cancel):
            require(size is not None, 'unsafe_file', 'Links/special files prevent release: ' + rel)
            listing.append((rel, size))
        siblings = {rel.lower() for rel, _ in listing}
        media = []
        for rel, size in listing:
            if kind_of(rel) not in MEDIA_KINDS:
                preserved.append(rel)
                continue
            reason = _keep_reason(rel, siblings if keep.get('asd', True) else set(), refs['names'], prefixes)
            if reason:
                kept.append({'path': rel, 'bytes': size, 'reason': reason})
            elif size not in index:
                uncovered.append({'path': rel, 'bytes': size, 'reason': 'no_same_size'})
            else:
                media.append((rel, size))
        total = sum(size for _, size in media)
        done, cache, open_trees = 0, {}, {}
        try:
            for position, (rel, size) in enumerate(media):
                report(progress, 'hashing_source', path=rel, files_done=position, files_total=len(media), bytes_done=done, bytes_total=total)
                sha, stamp = checked_hash_progress(origin, rel, cancel, progress, 'hashing_source', position, len(media))
                done += size
                name = os.path.basename(rel).lower()
                candidates = sorted(index[size], key=lambda c: os.path.basename(c[1]).lower() != name)
                match = None
                for archive_id, candidate in candidates:
                    key = (archive_id, candidate)
                    if key not in cache:
                        tree = open_trees.get(archive_id)
                        if tree is None:
                            tree = open_trees[archive_id] = Tree(records[archive_id]['root']).__enter__()
                        try:
                            cache[key] = checked_hash_progress(tree, candidate, cancel, progress, 'matching_archive', position, len(media))
                        except FileNotFoundError:
                            cache[key] = (None, None)
                    if cache[key][0] == sha:
                        match = (archive_id, candidate, cache[key][1])
                        break
                if match:
                    entries.append({'path': rel, 'stamp': stamp, 'sha256': sha, 'archive_id': match[0],
                                    'archive_path': match[1], 'archive_stamp': match[2]})
                else:
                    uncovered.append({'path': rel, 'bytes': size, 'sha256': sha, 'reason': 'no_identical_copy'})
        finally:
            for tree in open_trees.values():
                tree.__exit__(None, None, None)
    report(progress, 'inspecting_clipper')
    entries, clipper_kept, consumers = _clipper_split(catalog, root, entries, records)
    kept.extend(clipper_kept)
    value = {'schema': SCHEMA, 'id': uid(), 'created_at': now(), 'source_root': str(root),
             'source_identity': {k: source_info.get(k) for k in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode')},
             'volume_root': str(volume_root), 'archives': sorted(records), 'search': search or [],
             'keep_rules': {'asd': bool(keep.get('asd', True)), 'als_roots': [str(Path(p).expanduser().absolute()) for p in keep.get('als_roots') or []],
                            'paths': prefixes, 'ableton_sets': refs['sets'], 'unreadable_sets': refs['unreadable']},
             'entries': entries, 'kept': kept, 'uncovered': uncovered, 'preserved': preserved,
             'bytes': sum(e['stamp']['size'] for e in entries), 'consumers': consumers,
             'summary': summary(entries, kept, uncovered), 'state': 'review', 'moved': {}}
    value['digest'] = digest(value)
    with catalog.writer():
        catalog.set(KEY + value['id'], value)
    return value


def _clipper_split(catalog, root, entries, records):
    """A recording Clipper still uses stays; release never relinks a living collection behind Clipper's back."""
    if not entries:
        return entries, [], {'blockers': [], 'relinks': []}
    paths = {str(root / e['path']): str(Path(records[e['archive_id']]['root']) / e['archive_path']) for e in entries}
    try:
        consumers = clipper.inspect(catalog, paths)
    except (RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
        return entries, [], {'blockers': [str(error)], 'relinks': []}
    used = {r['from'] for r in consumers['relinks']}
    held = [e for e in entries if any(clipper.overlaps(str(root / e['path']), u) for u in used)]
    rest = [e for e in entries if e not in held]
    return rest, [{'path': e['path'], 'bytes': e['stamp']['size'], 'reason': 'clipper'} for e in held], \
        {'blockers': consumers['blockers'], 'relinks': []}


def summary(entries, kept, uncovered):
    """Per first folder (a year in `recordingslong`): what is released, kept, and has no copy."""
    groups = {}
    def add(path, key, size):
        top = path.split('/')[0] if '/' in path else '.'
        group = groups.setdefault(top, {'folder': top, 'release': [0, 0], 'keep': [0, 0], 'uncovered': [0, 0]})
        group[key][0] += 1
        group[key][1] += size
    for e in entries: add(e['path'], 'release', e['stamp']['size'])
    for k in kept: add(k['path'], 'keep', k['bytes'])
    for u in uncovered: add(u['path'], 'uncovered', u['bytes'])
    return sorted(groups.values(), key=lambda g: g['folder'])


def stored(catalog, plan_id, reviewed_digest):
    value = catalog.get(KEY + plan_id)
    require(value and value['digest'] == reviewed_digest, 'plan_changed', 'Use the recorded reviewed plan')
    return value


def plans(catalog):
    import json
    rows = catalog.db.execute("SELECT value FROM metadata WHERE key LIKE ?", (KEY + '%',)).fetchall()
    return [{k: v for k, v in json.loads(r[0]).items() if k not in {'entries', 'kept', 'uncovered', 'preserved', 'moved'}} for r in rows]


def _check_source(backend, value):
    current = backend.describe(value['source_root'])
    require(all(current.get(k) == value['source_identity'].get(k) for k in value['source_identity']),
            'source_changed', 'Source identity changed')


def _trash_folder(value):
    return '.Trashes/{}/nuun-{}'.format(os.getuid(), value['id'][:8])


def _trash_dir(volume_root, rel_dir, device):
    """Open (creating) a Trash folder by path: an external volume's `.Trashes` is write/search-only, never readable.

    Each component is checked with lstat (no links) and must stay on the source device."""
    path = Path(volume_root)
    for part in relative(rel_dir).split('/'):
        path = path / part
        try:
            st = os.lstat(path)
        except FileNotFoundError:
            os.mkdir(path, 0o700)
            st = os.lstat(path)
        require(stat.S_ISDIR(st.st_mode) and st.st_dev == device, 'unsafe_path', 'Unsafe Trash folder: ' + str(path))
    fd = os.open(str(path), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    require(os.fstat(fd).st_dev == device, 'unsafe_path', 'Trash folder left the volume')
    return fd


def _move(volume, from_rel, to_rel, expected):
    """fd-relative rename inside one volume; refuses links, a changed file or an existing target."""
    with volume.parent(from_rel) as (src, name):
        st = os.stat(name, dir_fd=src, follow_symlinks=False)
        require(stat.S_ISREG(st.st_mode), 'unsafe_file', 'Not a regular file: ' + from_rel)
        require(content_stamp(st) == expected, 'source_changed', 'File changed before release: ' + from_rel)
        dst = _trash_dir(volume.root, str(Path(to_rel).parent), st.st_dev)
        try:
            target = Path(to_rel).name
            try:
                os.stat(target, dir_fd=dst, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                require(False, 'destination_exists', 'Already in the Trash: ' + to_rel)
            os.rename(name, target, src_dir_fd=src, dst_dir_fd=dst)
            full_sync(dst)
            full_sync(src)
        finally:
            os.close(dst)


def _trash_hash(volume_root, rel, cancel):
    """Hash a file inside the Trash by path (its parents are not readable)."""
    from .common import hash_fd
    fd = os.open(str(Path(volume_root) / relative(rel)), os.O_RDONLY | os.O_NOFOLLOW)
    try:
        require(stat.S_ISREG(os.fstat(fd).st_mode), 'unsafe_file', 'Not a regular file: ' + rel)
        return hash_fd(fd, cancel or (lambda: False))
    finally:
        os.close(fd)


def apply(catalog, backend, plan_id, reviewed_digest, cancel=None, progress=None, hook=None):
    """Move exactly the reviewed entries to the Trash, each after re-hashing its archive copy and itself."""
    import time
    with catalog.writer():
        value = stored(catalog, plan_id, reviewed_digest)
        if value['state'] == 'completed':
            return value
        # Blockers seen while planning (an offline Clipper project) are not final: the loop asks Clipper again before
        # the first move and every CLIPPER_RECHECK_SECONDS, and refuses on any blocker or use of these files.
        _check_source(backend, value)
        records = {a: catalog.volume(a) for a in value['archives']}
        for record in records.values():
            backend.check(record)
        root, volume_root = Path(value['source_root']), Path(value['volume_root'])
        prefix = relative(str(root.relative_to(volume_root))) if root != volume_root else ''
        trash = _trash_folder(value)
        absolute = {str(root / e['path']): str(Path(records[e['archive_id']]['root']) / e['archive_path']) for e in value['entries']}
        checked_at = None  # never skipped: the first move always asks Clipper first
        value['state'] = 'applying'
        catalog.set(KEY + plan_id, value)
        total = sum(e['stamp']['size'] for e in value['entries'])
        done_bytes = sum(e['stamp']['size'] for e in value['entries'] if value['moved'].get(e['path']) == 'done')
        with Tree(root) as origin, Tree(volume_root) as volume:
            archive_trees = {a: Tree(r['root']).__enter__() for a, r in records.items()}
            try:
                for index, entry in enumerate(value['entries']):
                    path = entry['path']
                    state = value['moved'].get(path)
                    if state == 'done':
                        continue
                    require(not (cancel and cancel()), 'cancelled', 'Paused; resume the same reviewed plan')
                    report(progress, 'releasing', path=path, files_done=index, files_total=len(value['entries']),
                           bytes_done=done_bytes, bytes_total=total)
                    in_trash = trash + '/' + (prefix + '/' if prefix else '') + path
                    if state == 'intent':
                        try:
                            origin.stat(path)
                        except FileNotFoundError:
                            # Interrupted after the rename: the Trash copy must be the reviewed bytes.
                            require(_trash_hash(volume_root, in_trash, cancel) == entry['sha256'], 'source_changed',
                                    'Interrupted release left an unexpected file: ' + path)
                            value['moved'][path] = 'done'
                            catalog.set(KEY + plan_id, value)
                            done_bytes += entry['stamp']['size']
                            continue
                    if checked_at is None or time.monotonic() - checked_at > CLIPPER_RECHECK_SECONDS:
                        consumers = clipper.inspect(catalog, absolute)
                        require(not consumers['blockers'] and not consumers['relinks'], 'dependencies_blocked',
                                'Clipper started using a file in this plan; review again')
                        checked_at = time.monotonic()
                    _check_source(backend, value)
                    backend.check(records[entry['archive_id']])
                    if hook: hook('before_move', entry)
                    archive_sha, archive_stamp = checked_hash(archive_trees[entry['archive_id']], entry['archive_path'], cancel)
                    require(archive_sha == entry['sha256'], 'checksum_mismatch', 'Archive copy changed: ' + entry['archive_path'])
                    sha, stamp = checked_hash_progress(origin, path, cancel, progress, 'releasing', index, len(value['entries']))
                    require(sha == entry['sha256'] and stamp == entry['stamp'], 'source_changed', 'Source changed since review: ' + path)
                    value['moved'][path] = 'intent'
                    catalog.set(KEY + plan_id, value)
                    _move(volume, (prefix + '/' if prefix else '') + path, in_trash, stamp)
                    value['moved'][path] = 'done'
                    catalog.set(KEY + plan_id, value)
                    catalog.event(plan_id, 'collection_released', {'path': path, 'archive_id': entry['archive_id'],
                                                                     'archive_path': entry['archive_path'], 'sha256': sha, 'trash': in_trash})
                    done_bytes += entry['stamp']['size']
                    if hook: hook('after_move', entry)
            finally:
                for tree in archive_trees.values():
                    tree.__exit__(None, None, None)
        value['state'] = 'completed'
        value['completed_at'] = now()
        value['trash'] = str(volume_root / trash)
        catalog.set(KEY + plan_id, value)
        catalog.event(plan_id, 'collection_release_completed', {'bytes': value['bytes'], 'files': len(value['entries']), 'trash': value['trash']})
        report(progress, 'release_complete', files_done=len(value['entries']), files_total=len(value['entries']), bytes_done=total, bytes_total=total)
        return value
