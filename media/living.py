"""Living collections (M2): incremental, verified backup of a folder that keeps changing.

A living collection is one source folder (an Ableton projects folder, `~/Music/01_produccion`, a sample library)
mirrored under one folder of a registered archive, keeping its own relative paths. Rules:

- Only what is new or changed is copied; a copy is verified by SHA-256 before it gets its final name (Transfer).
- Nothing is renamed, the source is never cleaned, and a file deleted at the source stays in the archive.
- A changed file never overwrites the archive: the previous bytes move inside the archive to
  `_nuun/versions/<run>/<path>` first, so every version that was backed up stays.
- Unchanged files (same size and mtime as the last verified copy, still present on the archive) are not re-read.
- The per-file index lives in the catalog (`living_files`); `_nuun/manifest.json` on the archive repeats it so the
  index can be rebuilt without the catalog.

The plan is read-only and reviewable; apply copies exactly the reviewed items. A file that changed after review is
skipped and reported (the next run picks it up) instead of failing the whole run.
"""
import gzip
import json
import os
from pathlib import Path
import re
import stat
import time
import uuid
import zlib

from .common import MediaError, Tree, canonical, content_stamp, digest, full_sync, now, relative, require, uid
from .coverage import walk
from .release import report
from .volumes import independent

SCHEMA = 'nu9ve-living-plan/1'
KEY = 'living-plan:'
COLLECTION = 'living-collection:'
META = '_nuun'
ABSOLUTE_REFERENCE = re.compile(rb'Value="(/[^"]*\.(?:wav|aif|aiff|mp3|flac|m4a|ogg|alac|mid|midi))"', re.IGNORECASE)
ALS_LIMIT = 256 * 1024 * 1024
FLUSH_EVERY = 200


def collection_id(source, archive_id, folder):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, 'nu9ve-living:{}|{}|{}'.format(Path(source), archive_id, folder)))


def collections(catalog):
    rows = catalog.db.execute("SELECT value FROM metadata WHERE key LIKE ?", (COLLECTION + '%',)).fetchall()
    result = []
    for row in rows:
        value = json.loads(row[0])
        count, size = catalog.db.execute('SELECT count(*), coalesce(sum(size),0) FROM living_files WHERE collection_id=?',
                                         (value['id'],)).fetchone()
        result.append(dict(value, files=count, bytes=size))
    return sorted(result, key=lambda c: c['name'])


def _index(catalog, cid):
    return {row['path']: dict(row) for row in catalog.db.execute(
        'SELECT path, size, mtime_ns, sha256, archive_path FROM living_files WHERE collection_id=?', (cid,))}


def ableton_external(root, collections_sources, cancel=None):
    """Absolute sample paths that sets inside `root` use outside it, and whether some collection covers them.

    Only absolute `Path Value` references are read (Ableton 10+); older relative references are counted as unknown
    by Ableton itself when it opens the set, so they are not guessed here."""
    root = Path(root)
    sets, unreadable, refs = 0, [], {}
    for folder, dirs, files in os.walk(root):
        require(not (cancel and cancel()), 'cancelled', 'Planning cancelled')
        dirs[:] = [d for d in dirs if not d.startswith('.')]
        for name in files:
            if not name.lower().endswith('.als') or name.startswith('._'):
                continue
            path = os.path.join(folder, name)
            try:
                with gzip.open(path) as stream:
                    data = stream.read(ALS_LIMIT + 1)
                require(len(data) <= ALS_LIMIT, 'invalid_request', 'Set too large')
            except (OSError, EOFError, zlib.error, ValueError, MediaError) as error:
                unreadable.append({'path': os.path.relpath(path, root), 'error': str(error)[:200]})
                continue
            sets += 1
            for match in ABSOLUTE_REFERENCE.finditer(data):
                target = match.group(1).decode('utf-8', 'replace')
                if target == str(root) or target.startswith(str(root) + '/'):
                    continue
                refs.setdefault(target, set()).add(os.path.relpath(path, root).split('/')[0])
    rows = []
    for target, used_by in sorted(refs.items()):
        exists = os.path.isfile(target)
        covered = next((c['name'] for c in collections_sources if target.startswith(c['source'].rstrip('/') + '/')), None)
        rows.append({'path': target, 'exists': exists, 'covered_by': covered, 'used_by': sorted(used_by)[:5],
                     'bytes': os.path.getsize(target) if exists else None})
    return {'sets': sets, 'unreadable': unreadable, 'references': len(rows),
            'missing': sum(1 for r in rows if not r['exists']),
            'not_covered': [r for r in rows if r['exists'] and not r['covered_by']],
            'not_covered_bytes': sum(r['bytes'] for r in rows if r['exists'] and not r['covered_by'])}


def plan(catalog, backend, source, archive, folder, name=None, cancel=None, progress=None):
    """Read-only: what this run would copy, what changed, what is unchanged, what disappeared at the source."""
    root = Path(source).expanduser().absolute()
    require(root.is_dir() and not root.is_symlink(), 'invalid_request', 'Source folder not found: ' + str(root))
    folder = relative(folder)
    require(folder.split('/')[0] not in {'.ncreator', META} and META not in folder.split('/'), 'invalid_request', 'Reserved archive folder')
    record = catalog.volume(archive)
    require(record['role'] == 'archive', 'invalid_role', 'Archive required')
    archive_info = backend.check(record)
    archive_root = Path(record['root'])
    require(root != archive_root and archive_root not in root.parents and root not in archive_root.parents,
            'overlapping_roots', 'Source overlaps the archive')
    source_info = backend.describe(root)
    require(independent(source_info, archive_info), 'same_physical_disk', 'Archive and source must be independent disks')
    cid = collection_id(root, record['id'], folder)
    for other in collections(catalog):
        if other['id'] == cid or other['archive_id'] != record['id']:
            continue
        clash = other['folder'] == folder or other['folder'].startswith(folder + '/') or folder.startswith(other['folder'] + '/')
        require(not clash, 'overlapping_roots', 'Another collection already uses this archive folder: ' + other['folder'])
    report(progress, 'preparing')
    index = _index(catalog, cid)
    new, changed, unchanged_count, unchanged_bytes, present = [], [], 0, 0, set()
    with Tree(root) as origin, Tree(archive_root) as target:
        for rel, size in walk(origin, '', cancel):
            require(size is not None, 'unsafe_file', 'Links/special files are not followed: ' + rel)
            require(rel.split('/')[0] != META, 'invalid_request', 'The source has a reserved _nuun folder')
            present.add(rel)
            stamp = content_stamp(origin.stat(rel))
            known = index.get(rel)
            destination = folder + '/' + rel
            if known and known['size'] == stamp['size'] and known['mtime_ns'] == stamp['mtime_ns']:
                try:
                    on_archive = target.stat(known['archive_path']).st_size == known['size']
                except (FileNotFoundError, MediaError):
                    on_archive = False
                if on_archive:
                    unchanged_count += 1
                    unchanged_bytes += size
                    continue
            item = {'path': rel, 'stamp': stamp, 'archive_path': destination}
            (changed if known else new).append(item)
            if len(present) % 2000 == 0:
                report(progress, 'listing_source', files_done=len(present))
    gone = sorted(set(index) - present)
    value = {'schema': SCHEMA, 'id': uid(), 'created_at': now(), 'collection_id': cid,
             'name': name or root.name, 'source': str(root),
             'source_identity': {k: source_info.get(k) for k in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode')},
             'archive_id': record['id'], 'folder': folder, 'new': new, 'changed': changed,
             'unchanged': {'files': unchanged_count, 'bytes': unchanged_bytes},
             'gone_from_source': gone[:1000], 'gone_count': len(gone),
             'bytes': sum(i['stamp']['size'] for i in new + changed),
             'free': archive_info.get('free'),
             'ableton': ableton_external(root, [c for c in collections(catalog) if c['id'] != cid] +
                                         [{'name': name or root.name, 'source': str(root)}], cancel),
             'state': 'review', 'done': {}}
    require(value['free'] is None or value['bytes'] <= value['free'], 'insufficient_space',
            'Not enough free space on {} for {} bytes'.format(record['name'], value['bytes']))
    value['digest'] = digest(value)
    with catalog.writer():
        catalog.set(KEY + value['id'], value)
    return value


def stored(catalog, plan_id, reviewed_digest):
    value = catalog.get(KEY + plan_id)
    require(value and value['digest'] == reviewed_digest, 'plan_changed', 'Use the recorded reviewed plan')
    return value


def _version_aside(target, folder, rel, run):
    """Move the archive's previous bytes to `<folder>/_nuun/versions/<run>/<rel>` in the same archive (never deleted)."""
    archive_path = folder + '/' + rel
    aside = '{}/{}/versions/{}/{}'.format(folder, META, run, rel)
    with target.parent(archive_path) as (src, name), target.parent(aside, create=True) as (dst, final):
        st = os.stat(name, dir_fd=src, follow_symlinks=False)
        require(stat.S_ISREG(st.st_mode), 'unsafe_file', 'Not a regular file: ' + archive_path)
        try:
            os.stat(final, dir_fd=dst, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            require(False, 'destination_exists', 'Version already set aside: ' + aside)
        os.rename(name, final, src_dir_fd=src, dst_dir_fd=dst)
        full_sync(dst)
        full_sync(src)
    return aside


def apply(catalog, backend, plan_id, reviewed_digest, cancel=None, progress=None, hook=None):
    from .transfer import Transfer
    with catalog.writer():
        value = stored(catalog, plan_id, reviewed_digest)
        if value['state'] == 'completed':
            return value
        record = catalog.volume(value['archive_id'])
        backend.check(record)
        current = backend.describe(value['source'])
        require(all(current.get(k) == value['source_identity'].get(k) for k in value['source_identity']),
                'source_changed', 'Source folder identity changed')
        engine = Transfer(catalog, backend, cancel)
        run = time.strftime('%Y%m%d-%H%M%S', time.gmtime()) + '-' + value['id'][:8]
        items = value['new'] + value['changed']
        total = sum(i['stamp']['size'] for i in items)
        copied = adopted = versioned = skipped = 0
        done_bytes, pending_rows, versions = 0, [], []

        def flush():
            if pending_rows:
                with catalog.db:
                    catalog.db.executemany('INSERT OR REPLACE INTO living_files VALUES(?,?,?,?,?,?,?)', pending_rows)
                pending_rows.clear()
            catalog.set(KEY + plan_id, value)

        collection = {'id': value['collection_id'], 'name': value['name'], 'source': value['source'],
                      'archive_id': value['archive_id'], 'folder': value['folder'], 'last_plan': plan_id, 'updated_at': now()}
        value['state'] = 'applying'
        catalog.set_many({KEY + plan_id: value, COLLECTION + value['collection_id']: collection})
        with Tree(value['source']) as origin, Tree(record['root']) as target:
            for position, item in enumerate(items):
                path = item['path']
                if value['done'].get(path):
                    continue
                require(not (cancel and cancel()), 'cancelled', 'Paused; verified copies are kept, resume the same plan')
                report(progress, 'copying', path=path, files_done=position, files_total=len(items), bytes_done=done_bytes, bytes_total=total)
                if hook: hook('before_copy', item)
                try:
                    stamp_now = content_stamp(origin.stat(path))
                except FileNotFoundError:
                    stamp_now = None
                if stamp_now != item['stamp']:
                    value['done'][path] = 'changed_since_review'
                    skipped += 1
                    continue
                try:
                    target.stat(item['archive_path'])
                    existed = True
                except (FileNotFoundError, MediaError):
                    existed = False
                try:
                    # Identical bytes already there (an interrupted run, a manual copy) are adopted, not rewritten.
                    sha, _ = engine.copy_file(origin, path, target, item['archive_path'], record, source_stamp=item['stamp'])
                    state = 'adopted' if existed else 'copied'
                except MediaError as error:
                    if error.code != 'destination_conflict':
                        raise
                    aside = _version_aside(target, value['folder'], path, run)
                    versions.append({'path': path, 'aside': aside})
                    catalog.event(value['collection_id'], 'living_versioned', {'path': path, 'aside': aside, 'plan_id': plan_id})
                    sha, _ = engine.copy_file(origin, path, target, item['archive_path'], record, source_stamp=item['stamp'])
                    state = 'versioned'
                copied += state == 'copied'
                adopted += state == 'adopted'
                versioned += state == 'versioned'
                value['done'][path] = state
                pending_rows.append((value['collection_id'], path, item['stamp']['size'], item['stamp']['mtime_ns'], sha,
                                     item['archive_path'], now()))
                done_bytes += item['stamp']['size']
                if len(pending_rows) >= FLUSH_EVERY:
                    flush()
                if hook: hook('after_copy', item)
            flush()
            manifest = {'schema': 'nu9ve-living-manifest/1', 'collection_id': value['collection_id'], 'name': value['name'],
                        'source': value['source'], 'updated_at': now(),
                        'files': {r['path']: {'size': r['size'], 'sha256': r['sha256'], 'archive_path': r['archive_path']}
                                  for r in _index(catalog, value['collection_id']).values()}}
            target.write_bytes(value['folder'] + '/' + META + '/manifest.json', canonical(manifest) + b'\n')
        collection['updated_at'] = now()
        value.update(state='completed', completed_at=now(),
                     result={'copied': copied, 'adopted': adopted, 'versioned': versioned, 'changed_since_review': skipped,
                             'versions': versions[:1000]})
        catalog.set_many({KEY + plan_id: value, COLLECTION + value['collection_id']: collection},
                         event=(value['collection_id'], 'living_backup_completed',
                                dict(value['result'], plan_id=plan_id, bytes=done_bytes, versions=len(versions))))
        report(progress, 'complete', files_done=len(items), files_total=len(items), bytes_done=total, bytes_total=total)
        return value

