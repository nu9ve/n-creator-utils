"""Read-only audit: which files on the work disk already exist, byte-identical, on the archive."""
import json
import os
import stat

from manager.inventory import IGNORED_DIRS, IGNORED_FILES
from .common import Tree, hash_fd, now, relative, require, uncached

SKIPPED_DIRS = IGNORED_DIRS | {'.ncreator', '.DocumentRevisions-V100'}


def walk(tree, start='', cancel=None):
    """Yield (relative path, size) for regular files below `start`; links and special entries are reported, never followed."""
    stack = [start]
    while stack:
        require(not (cancel and cancel()), 'cancelled', 'Coverage audit cancelled')
        folder = stack.pop()
        with tree.parent((folder + '/' if folder else '') + 'x') as (fd, _):
            names = sorted(os.listdir(fd))
            for name in names:
                if name in IGNORED_FILES or name.startswith('._') or name.endswith('.ncpartial'):
                    continue
                rel = (folder + '/' if folder else '') + name
                st = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if stat.S_ISDIR(st.st_mode):
                    if name not in SKIPPED_DIRS:
                        stack.append(rel)
                elif stat.S_ISREG(st.st_mode):
                    yield rel, st.st_size
                else:
                    yield rel, None


def coverage(catalog, backend, work, archive, path='', hash_matches=True, cancel=None):
    work_record, archive_record = catalog.volume(work), catalog.volume(archive)
    require(work_record['role'] == 'work' and archive_record['role'] == 'archive', 'invalid_role', 'Compare a work volume against an archive volume')
    backend.check(work_record); backend.check(archive_record)
    start = relative(path) if path else ''
    # Files this tool ingested carry a verified archive receipt; no rehash needed for them.
    receipted = set()
    for row in catalog.db.execute('SELECT payload FROM copies'):
        copy = json.loads(row[0])
        if copy['volume_id'] == work_record['id']:
            archived = catalog.db.execute('SELECT 1 FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                          (copy['job_id'], archive_record['id'], copy['path'])).fetchone()
            if archived:
                receipted.add(copy['destination'])
    report = {'schema': 'nu9ve-coverage/1', 'created_at': now(), 'work': work_record['name'], 'archive': archive_record['name'],
              'path': start, 'hashed': hash_matches, 'folders': {}, 'only_work': [], 'links_or_special': []}
    with Tree(work_record['root']) as work_tree, Tree(archive_record['root']) as archive_tree:
        by_size = {}
        for rel, size in walk(archive_tree, cancel=cancel):
            if size:
                by_size.setdefault(size, []).append(rel)
        archive_hashes = {}

        def digest_of(tree, rel, cache=None):
            if cache is not None and rel in cache:
                return cache[rel]
            fd = tree.open(rel)
            try:
                value = hash_fd(uncached(fd), cancel or (lambda: False))
            finally:
                os.close(fd)
            if cache is not None:
                cache[rel] = value
            return value

        for rel, size in walk(work_tree, start, cancel):
            folder = '/'.join(rel.split('/')[:2]) if '/' in rel else '.'
            bucket = report['folders'].setdefault(folder, {k: {'files': 0, 'bytes': 0} for k in ('in_archive', 'size_match', 'only_work')})
            if size is None:
                report['links_or_special'].append(rel)
                continue
            if rel in receipted:
                status, match = 'in_archive', 'receipt'
            elif size == 0 or size not in by_size:
                status, match = 'only_work', None
            elif not hash_matches:
                status, match = 'size_match', by_size[size][0]
            else:
                wanted = digest_of(work_tree, rel)
                match = next((c for c in by_size[size] if digest_of(archive_tree, c, archive_hashes) == wanted), None)
                status = 'in_archive' if match else 'only_work'
            bucket[status]['files'] += 1
            bucket[status]['bytes'] += size
            if status == 'only_work':
                report['only_work'].append({'path': rel, 'bytes': size})
    totals = {k: {'files': sum(f[k]['files'] for f in report['folders'].values()), 'bytes': sum(f[k]['bytes'] for f in report['folders'].values())}
              for k in ('in_archive', 'size_match', 'only_work')}
    report['totals'] = totals
    folder = catalog.home / 'reports'
    folder.mkdir(parents=True, exist_ok=True, mode=0o700)
    target = folder / 'coverage-{}.json'.format(report['created_at'].replace(':', '-'))
    target.write_text(json.dumps(report, indent=2, ensure_ascii=False))
    return {'report': str(target), 'totals': totals, 'folders': report['folders'], 'only_work_sample': report['only_work'][:50]}
