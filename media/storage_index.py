"""Disk index: what occupies each disk and whether it already has a copy on the archive.

A scan reads directory metadata only; file contents are read only by an explicit compare. The
index is derived and rebuildable, so it lives apart from the catalog. It never authorizes a
deletion: release plans re-hash current bytes on their own.
"""
import contextlib
import errno
import fcntl
import json
import os
from pathlib import Path
import shutil
import sqlite3
import stat
import time
import unicodedata

from manager.inventory import KINDS
from .common import MediaError, Tree, hash_fd, now, require, uid, uncached
from .volumes import independent

SCHEMA = 1
OBSERVED_KEY = 'storage-observed'
# Folders you choose can hold millions of small files. Those stay in the rollups only, and a parent is
# always at least as large as its child, so the kept folders still form a connected tree.
OBSERVED_ENTRY_MIN_BYTES = 10 * 1024 * 1024
OBSERVED_FOLDER_MIN_BYTES = 50 * 1024 * 1024
# The index must never be what fills a nearly full disk.
MINIMUM_FREE_BYTES = 512 * 1024 * 1024
FLUSH_ROWS = 5000
FLUSH_SECONDS = 1.0
SAFETY = ('verified', 'probable', 'only_here', 'derived', 'unknown')
SUFFIX_KIND = {suffix: kind for kind, suffixes in KINDS.items() for suffix in suffixes if kind != 'auxiliary'}
PACKAGE_KIND = {'.stalker': 'capture', '.fcpbundle': 'editable', '.logicx': 'editable', '.band': 'editable',
                '.imovielibrary': 'editable', '.photoslibrary': 'image', '.app': 'other', '.framework': 'other'}
# Only a suggestion for the view: nothing here is ever removed because of its name.
DERIVED_DIRS = {'node_modules', '__pycache__', '.venv', 'venv', '.build', 'DerivedData', 'Caches', '.cache', '.next',
                'Render Files', 'Transcoded Media', 'Proxy Media', 'Peak Files', 'Thumbnail Media'}
OPAQUE_DIRS = {'.git', '.Trashes', '.Trash', '.Spotlight-V100', '.fseventsd', '.DocumentRevisions-V100', '.TemporaryItems'}
THIS_MAC = 'this-mac'
# The data volume holds everything on the internal disk that is not the sealed system; other disks appear as mounts and stay out.
THIS_MAC_ROOT = '/System/Volumes/Data'
SYSTEM_ROOTS = ('/System', '/Library', '/private', '/usr', '/bin', '/sbin', '/etc', '/var', '/dev', '/cores', '/opt')


def _storable(path):
    try:
        path.encode('utf-8')
        return True
    except UnicodeEncodeError:
        return False


def _printable(path):
    return path.encode('utf-8', 'backslashreplace').decode('utf-8')


def _rel(value):
    if value in (None, ''):
        return ''
    require(isinstance(value, str) and '\x00' not in value and not value.startswith('/')
            and all(part not in {'', '.', '..'} for part in value.split('/')), 'unsafe_path', 'Path must remain within its root')
    return value


def _nfc(path):
    # HFS+ answers decomposed names while receipts hold the composed name of the same file.
    return unicodedata.normalize('NFC', path)


def _alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except OSError:
        return True


def _create(db):
    db.execute('PRAGMA auto_vacuum=INCREMENTAL')
    db.executescript('''
      CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS runs(id INTEGER PRIMARY KEY AUTOINCREMENT, target_id TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL,
        pid INTEGER NOT NULL, path TEXT NOT NULL DEFAULT '', started_at TEXT NOT NULL, finished_at TEXT, files INTEGER NOT NULL DEFAULT 0,
        allocated INTEGER NOT NULL DEFAULT 0, total_files INTEGER NOT NULL DEFAULT 0, total_bytes INTEGER NOT NULL DEFAULT 0,
        unreadable INTEGER NOT NULL DEFAULT 0, changed INTEGER NOT NULL DEFAULT 0, samples TEXT NOT NULL DEFAULT '[]', error TEXT);
      CREATE TABLE IF NOT EXISTS indexes(target_id TEXT PRIMARY KEY, run_id INTEGER NOT NULL, indexed_at TEXT NOT NULL, seconds REAL NOT NULL,
        fs_total INTEGER NOT NULL, fs_used INTEGER NOT NULL, fs_free INTEGER NOT NULL, unreadable INTEGER NOT NULL, samples TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS entries(target_id TEXT NOT NULL, run_id INTEGER NOT NULL, path TEXT NOT NULL, parent TEXT NOT NULL,
        size INTEGER NOT NULL, allocated INTEGER NOT NULL, mtime_ns INTEGER NOT NULL, kind TEXT NOT NULL, PRIMARY KEY(target_id,run_id,path)) WITHOUT ROWID;
      CREATE INDEX IF NOT EXISTS entries_parent ON entries(target_id,run_id,parent,allocated);
      CREATE TABLE IF NOT EXISTS folders(target_id TEXT NOT NULL, run_id INTEGER NOT NULL, path TEXT NOT NULL, parent TEXT NOT NULL, files INTEGER NOT NULL,
        bytes INTEGER NOT NULL, allocated INTEGER NOT NULL, kinds TEXT NOT NULL, kind TEXT NOT NULL, opaque INTEGER NOT NULL, PRIMARY KEY(target_id,run_id,path)) WITHOUT ROWID;
      CREATE INDEX IF NOT EXISTS folders_parent ON folders(target_id,run_id,parent,allocated);
      CREATE TABLE IF NOT EXISTS safety(target_id TEXT NOT NULL, path TEXT NOT NULL, verified INTEGER NOT NULL, probable INTEGER NOT NULL, only_here INTEGER NOT NULL,
        derived INTEGER NOT NULL, unknown INTEGER NOT NULL, PRIMARY KEY(target_id,path)) WITHOUT ROWID;
      CREATE TABLE IF NOT EXISTS hashes(target_id TEXT NOT NULL, path TEXT NOT NULL, size INTEGER NOT NULL, mtime_ns INTEGER NOT NULL, sha256 TEXT NOT NULL,
        origin TEXT NOT NULL, checked_at TEXT NOT NULL, PRIMARY KEY(target_id,path)) WITHOUT ROWID;
    ''')
    db.execute('PRAGMA user_version=' + str(SCHEMA))
    db.commit()


def open_index(catalog):
    """A damaged or foreign index is rebuilt: everything in it can be scanned again."""
    path = catalog.home / 'storage-index.sqlite3'
    require(not path.is_symlink(), 'unsafe_state', 'Storage index must not be a symlink')
    for attempt in (1, 2):
        db = None
        try:
            if not path.exists():
                os.close(os.open(str(path), os.O_CREAT | os.O_WRONLY | os.O_NOFOLLOW, 0o600))
            db = sqlite3.connect(str(path), timeout=15)
            db.row_factory = sqlite3.Row
            if db.execute('PRAGMA user_version').fetchone()[0] != SCHEMA:
                require(db.execute('SELECT COUNT(*) FROM sqlite_master').fetchone()[0] == 0, 'index_version', 'Storage index version changed')
                _create(db)
            db.execute('PRAGMA journal_mode=WAL')
            db.execute('PRAGMA synchronous=NORMAL')
            return db
        except (sqlite3.DatabaseError, MediaError) as error:
            if db is not None:
                db.close()
            if attempt == 2 or (isinstance(error, MediaError) and error.code != 'index_version'):
                raise
            for suffix in ('', '-wal', '-shm'):
                with contextlib.suppress(FileNotFoundError):
                    os.unlink(str(path) + suffix)


@contextlib.contextmanager
def _exclusive(catalog):
    """One scan or compare at a time, per catalog. Independent of the transfer lock: an index never blocks a copy."""
    fd = os.open(str(catalog.home / 'storage.lock'), os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise MediaError('storage_busy', 'Another disk scan or comparison is running') from error
        yield
    finally:
        os.close(fd)


# Targets: registered disks, plus folders chosen only to be seen. Those stay out of the volumes table so
# they never count as destinations, never wake the card popup and can never be picked as archive or work.

def targets(catalog):
    registered = [dict(v, observed=False) for v in catalog.volumes()]
    found = registered + [dict(v, observed=True) for v in catalog.get(OBSERVED_KEY, [])]
    if os.path.isdir(THIS_MAC_ROOT):
        found.append({'id': THIS_MAC, 'name': 'Mac', 'role': 'observed', 'root': THIS_MAC_ROOT, 'prefix': '', 'identity': {}, 'observed': True, 'builtin': True})
    return found


def _target(catalog, name):
    found = next((t for t in targets(catalog) if name in {t['id'], t['name']}), None)
    require(found is not None, 'unknown_volume', 'Unknown disk or folder: ' + str(name))
    return found


def _online(backend, target):
    if not target['observed']:
        return backend.online(target)
    try:
        info = backend.describe(target['root'])
    except (OSError, MediaError, ValueError):
        return None
    return info if target.get('builtin') or info.get('root_inode') == target['identity'].get('root_inode') else None


def add_folder(catalog, backend, root, name=None):
    root = Path(root).expanduser().absolute()
    require(str(root) != '/' and not any(str(root) == base or str(root).startswith(base + '/') for base in SYSTEM_ROOTS),
            'unsafe_path', 'Choose a folder of yours or a disk, not the system')
    info = backend.describe(root)
    require(not (info.get('removable') and info.get('mount') == str(root)), 'card_not_indexable', 'Memory cards are read through Ingreso, not indexed')
    for existing in targets(catalog):
        other = Path(existing['root'])
        require(other != root and other not in root.parents and root not in other.parents,
                'volume_exists', 'That folder overlaps ' + existing['name'])
    label = name or root.name
    value = {'id': uid(), 'name': label, 'role': 'observed', 'root': str(root), 'prefix': '', 'identity': info}
    require(not any(t['name'] == label for t in targets(catalog)), 'volume_exists', 'Name already in use')
    catalog.set(OBSERVED_KEY, catalog.get(OBSERVED_KEY, []) + [value])
    return value


def remove_folder(catalog, db, target):
    """Forgets a chosen folder and its index; the folder itself is not touched."""
    found = _target(catalog, target)
    require(found['observed'] and not found.get('builtin'), 'invalid_role', 'Only chosen folders can be removed from the index')
    catalog.set(OBSERVED_KEY, [v for v in catalog.get(OBSERVED_KEY, []) if v['id'] != found['id']])
    with db:
        for table in ('entries', 'folders', 'safety', 'hashes', 'indexes', 'runs'):
            db.execute('DELETE FROM {} WHERE target_id=?'.format(table), (found['id'],))
    return {'removed': found['id']}


# Scan

class _Frame:
    __slots__ = ('path', 'parent', 'kind', 'opaque', 'hidden', 'storable', 'files', 'bytes', 'allocated', 'kinds', 'children')

    def __init__(self, path, parent, kind='', opaque=False, hidden=False, storable=True):
        self.path, self.parent, self.kind, self.opaque, self.hidden, self.storable = path, parent, kind, opaque, hidden, storable
        self.files = self.bytes = self.allocated = 0
        self.kinds, self.children = {}, None


def _child_frame(frame, name):
    path = (frame.path + '/' if frame.path else '') + name
    hidden = frame.opaque or frame.hidden
    if hidden:
        return _Frame(path, frame, frame.kind, False, True, frame.storable)
    suffix = os.path.splitext(name)[1].lower()
    storable = frame.storable and _storable(name)
    if suffix in PACKAGE_KIND:
        return _Frame(path, frame, PACKAGE_KIND[suffix], True, False, storable)
    if name in DERIVED_DIRS:
        return _Frame(path, frame, 'derived', True, False, storable)
    if name in OPAQUE_DIRS:
        return _Frame(path, frame, 'other', True, False, storable)
    return _Frame(path, frame, frame.kind, False, False, storable)


def _walk(root, device, keep_small, cancel, add_entry, add_folder_row, on_unreadable, on_changed, tick):
    """Post-order walk on directory metadata; no file is opened and no link followed."""
    seen_links, stack, top = set(), [], _Frame('', None)
    stack.append(top)
    while stack:
        frame = stack[-1]
        if frame.children is None:
            require(not (cancel and cancel()), 'cancelled', 'Scan cancelled')
            frame.children = []
            full = os.path.join(root, frame.path) if frame.path else root
            try:
                found = []
                with os.scandir(full) as listing:
                    for entry in listing:
                        try:
                            found.append((entry.name, entry.stat(follow_symlinks=False)))
                        except FileNotFoundError:
                            on_changed()
            except FileNotFoundError:
                require(frame.parent is not None, 'volume_offline', 'Root disappeared during scan')
                on_changed()
                found = []
            except OSError as error:
                # A disconnect looks like per-folder errors; never publish that as an emptier disk.
                current = os.stat(root, follow_symlinks=False)
                require(current.st_dev == device and frame.parent is not None, 'volume_offline', 'Disk disconnected during scan')
                if error.errno not in (errno.EACCES, errno.EPERM, errno.ELOOP, errno.ENOTDIR):
                    os.listdir(root)
                on_unreadable(frame.path)
                found = []
            if not frame.opaque and not frame.hidden and any(name == 'clipper-project.json' for name, _ in found):
                frame.kind, frame.opaque = 'editable', True
            for name, st in found:
                if stat.S_ISDIR(st.st_mode):
                    # A folder that is its own mount belongs to another disk.
                    if st.st_dev == device:
                        frame.children.append(_child_frame(frame, name))
                    continue
                if not stat.S_ISREG(st.st_mode):
                    continue
                if st.st_nlink > 1:
                    key = (st.st_dev, st.st_ino)
                    if key in seen_links:
                        continue
                    seen_links.add(key)
                allocated = getattr(st, 'st_blocks', 0) * 512 or st.st_size
                kind = frame.kind or SUFFIX_KIND.get(os.path.splitext(name)[1].lower(), 'other')
                frame.files += 1; frame.bytes += st.st_size; frame.allocated += allocated
                frame.kinds[kind] = frame.kinds.get(kind, 0) + allocated
                path = (frame.path + '/' if frame.path else '') + name
                if not (frame.storable and _storable(name)):
                    on_unreadable(path)
                elif keep_small or (st.st_size >= OBSERVED_ENTRY_MIN_BYTES and kind != 'derived'):
                    add_entry(path, frame.path, st.st_size, allocated, st.st_mtime_ns, kind)
            tick(frame)
        elif frame.children:
            stack.append(frame.children.pop())
        else:
            stack.pop()
            if frame.storable and not frame.hidden and (keep_small or frame.parent is None or frame.allocated >= OBSERVED_FOLDER_MIN_BYTES):
                add_folder_row(frame)
            if frame.parent is not None:
                parent = frame.parent
                parent.files += frame.files; parent.bytes += frame.bytes; parent.allocated += frame.allocated
                for kind, value in frame.kinds.items():
                    parent.kinds[kind] = parent.kinds.get(kind, 0) + value
    return top


def scan(catalog, backend, target, cancel=None):
    found = _target(catalog, target)
    with _exclusive(catalog), contextlib.closing(open_index(catalog)) as db:
        info = _online(backend, found)
        require(info is not None, 'volume_offline', found['name'] + ' is disconnected')
        root = found['root']
        before = os.stat(root, follow_symlinks=False)
        # Under the lock, anything unpublished belongs to a dead scan.
        with db:
            db.execute("UPDATE runs SET state='interrupted',finished_at=? WHERE state='running'", (now(),))
            db.execute('DELETE FROM entries WHERE run_id NOT IN (SELECT run_id FROM indexes)')
            db.execute('DELETE FROM folders WHERE run_id NOT IN (SELECT run_id FROM indexes)')
            db.execute('DELETE FROM runs WHERE id NOT IN (SELECT id FROM runs ORDER BY id DESC LIMIT 200)')
            run = db.execute("INSERT INTO runs(target_id,kind,state,pid,started_at) VALUES(?,?,?,?,?)",
                             (found['id'], 'scan', 'running', os.getpid(), now())).lastrowid
        started = time.monotonic()
        entries, folders, samples = [], [], []
        counters = {'unreadable': 0, 'changed': 0, 'files': 0, 'allocated': 0, 'flushed': started}

        def flush():
            require(shutil.disk_usage(str(catalog.home)).free >= MINIMUM_FREE_BYTES, 'low_space', 'Too little free space on this Mac to keep the index')
            db.executemany('INSERT OR REPLACE INTO entries VALUES(?,?,?,?,?,?,?,?)', entries)
            db.executemany('INSERT OR REPLACE INTO folders VALUES(?,?,?,?,?,?,?,?,?,?)', folders)
            entries.clear(); folders.clear()
            db.execute('UPDATE runs SET files=?,allocated=?,unreadable=?,changed=? WHERE id=?',
                       (counters['files'], counters['allocated'], counters['unreadable'], counters['changed'], run))
            db.commit()
            counters['flushed'] = time.monotonic()

        def add_entry(path, parent, size, allocated, mtime_ns, kind):
            entries.append((found['id'], run, path, parent, size, allocated, mtime_ns, kind))

        def add_folder_row(frame):
            parent = frame.parent.path if frame.parent is not None else ''
            folders.append((found['id'], run, frame.path, parent, frame.files, frame.bytes, frame.allocated,
                            json.dumps(frame.kinds, sort_keys=True), frame.kind, int(frame.opaque)))

        def unreadable(path):
            counters['unreadable'] += 1
            if len(samples) < 50:
                samples.append(_printable(path))

        def changed():
            counters['changed'] += 1

        def tick(frame):
            counters['files'] += frame.files
            counters['allocated'] += frame.allocated
            if len(entries) + len(folders) >= FLUSH_ROWS or time.monotonic() - counters['flushed'] >= FLUSH_SECONDS:
                flush()

        try:
            top = _walk(root, before.st_dev, not found['observed'], cancel, add_entry, add_folder_row, unreadable, changed, tick)
            flush()
            current = _online(backend, found)
            after = os.stat(root, follow_symlinks=False)
            require(current is not None and (after.st_dev, after.st_ino) == (before.st_dev, before.st_ino),
                    'volume_replaced', 'Disk changed during scan')
            usage = shutil.disk_usage(root)
            with db:
                old = db.execute('SELECT run_id FROM indexes WHERE target_id=?', (found['id'],)).fetchone()
                db.execute('INSERT OR REPLACE INTO indexes VALUES(?,?,?,?,?,?,?,?,?)',
                           (found['id'], run, now(), time.monotonic() - started, usage.total, usage.used, usage.free, counters['unreadable'], json.dumps(samples)))
                if old:
                    db.execute('DELETE FROM entries WHERE target_id=? AND run_id=?', (found['id'], old[0]))
                    db.execute('DELETE FROM folders WHERE target_id=? AND run_id=?', (found['id'], old[0]))
                db.execute("UPDATE runs SET state='finished',finished_at=?,files=?,allocated=?,samples=? WHERE id=?",
                           (now(), top.files, top.allocated, json.dumps(samples), run))
        except BaseException as error:
            cancelled = isinstance(error, MediaError) and error.code == 'cancelled'
            db.rollback()
            with db:
                db.execute('DELETE FROM entries WHERE target_id=? AND run_id=?', (found['id'], run))
                db.execute('DELETE FROM folders WHERE target_id=? AND run_id=?', (found['id'], run))
                db.execute('UPDATE runs SET state=?,finished_at=?,error=? WHERE id=?',
                           ('cancelled' if cancelled else 'failed', now(), None if cancelled else str(error), run))
            _reclaim(db)
            if cancelled:
                return {'state': 'cancelled', 'target': found['id']}
            raise
        _reclaim(db)
        # Published: a failure from here on leaves the new index in place, only less classified.
        _seed_receipts(catalog, db, found, run)
        _refresh_independence(catalog, backend, db)
        _recompute_all(catalog, db)
        return {'state': 'finished', 'target': found['id'], 'files': top.files, 'allocated': top.allocated, 'bytes': top.bytes,
                'unreadable': counters['unreadable'], 'seconds': round(time.monotonic() - started, 3)}


def _reclaim(db):
    """Replaced and abandoned rows give their pages back; the index file should not only grow."""
    db.execute('PRAGMA incremental_vacuum').fetchall()
    db.execute('PRAGMA wal_checkpoint(TRUNCATE)').fetchall()


def _receipts(catalog, volume_id=None):
    query = 'SELECT volume_id,sha256,payload FROM copies' + (' WHERE volume_id=?' if volume_id else '')
    for row in catalog.db.execute(query, (volume_id,) if volume_id else ()):
        payload = json.loads(row['payload'])
        if payload.get('destination'):
            yield row['volume_id'], row['sha256'], payload


def _seed_receipts(catalog, db, target, run):
    """Receipts stand in for hashing while path and size still match; stale hashes leave the cache here."""
    with db:
        db.execute('''DELETE FROM hashes WHERE target_id=? AND NOT EXISTS (SELECT 1 FROM entries e WHERE e.target_id=hashes.target_id AND e.run_id=?
                      AND e.path=hashes.path AND e.size=hashes.size AND e.mtime_ns=hashes.mtime_ns)''', (target['id'], run))
        if target['observed']:
            return
        receipts = {_nfc(p['destination']): (sha, p) for _, sha, p in _receipts(catalog, target['id'])}
        if not receipts:
            return
        rows = []
        for entry in db.execute('SELECT path,size,mtime_ns FROM entries WHERE target_id=? AND run_id=?', (target['id'], run)):
            match = receipts.get(_nfc(entry['path']))
            if match and match[1].get('bytes') == entry['size']:
                rows.append((target['id'], entry['path'], entry['size'], entry['mtime_ns'], match[0], 'receipt', match[1].get('verified_at') or ''))
        db.executemany('INSERT OR IGNORE INTO hashes VALUES(?,?,?,?,?,?,?)', rows)


def _meta(db, key, default=None):
    row = db.execute('SELECT value FROM meta WHERE key=?', (key,)).fetchone()
    return json.loads(row[0]) if row else default


def _refresh_independence(catalog, backend, db):
    """Disk numbers change between mounts, so physical overlap is only asked while both are connected."""
    found = targets(catalog)
    current = {t['id']: _online(backend, t) for t in found}
    with db:
        for archive in (t for t in found if t['role'] == 'archive'):
            for target in found:
                if target['id'] != archive['id'] and current[target['id']] and current[archive['id']]:
                    db.execute('INSERT OR REPLACE INTO meta VALUES(?,?)', ('independent:{}:{}'.format(target['id'], archive['id']),
                               json.dumps(independent(current[target['id']], current[archive['id']]))))


class _Evidence:
    """What the archive is known to hold, from its index and its receipts, for classifying one target."""

    def __init__(self, catalog, db, target):
        self.target = target
        self.archives = [t for t in targets(catalog) if t['role'] == 'archive' and t['id'] != target['id']
                         and _meta(db, 'independent:{}:{}'.format(target['id'], t['id']), True)
                         and t['identity'].get('volume_uuid') != target['identity'].get('volume_uuid')]
        self.located, self.sizes, self.indexed = {}, {}, []
        for archive in self.archives:
            for _, sha, payload in _receipts(catalog, archive['id']):
                self.located.setdefault(sha, (archive['name'], payload['destination'], 'receipt'))
            index = db.execute('SELECT run_id,indexed_at FROM indexes WHERE target_id=?', (archive['id'],)).fetchone()
            if index is None:
                continue
            self.indexed.append({'id': archive['id'], 'name': archive['name'], 'indexed_at': index['indexed_at']})
            for row in db.execute('SELECT path,sha256,origin FROM hashes WHERE target_id=?', (archive['id'],)):
                self.located.setdefault(row['sha256'], (archive['name'], row['path'], row['origin']))
            for row in db.execute('''SELECT e.size,COUNT(*) AS total,COUNT(h.sha256) AS hashed FROM entries e LEFT JOIN hashes h
                                     ON h.target_id=e.target_id AND h.path=e.path AND h.size=e.size AND h.mtime_ns=e.mtime_ns
                                     WHERE e.target_id=? AND e.run_id=? AND e.size>0 GROUP BY e.size''', (archive['id'], index['run_id'])):
                total, hashed = self.sizes.get(row['size'], (0, 0))
                self.sizes[row['size']] = (total + row['total'], hashed + row['hashed'])

    def state(self, size, kind, sha):
        if sha and sha in self.located:
            return 'verified'
        if kind == 'derived':
            return 'derived'
        if not self.indexed:
            return 'unknown'
        if size not in self.sizes:
            return 'only_here'
        total, hashed = self.sizes[size]
        # Same size only means "maybe" until both sides have a hash.
        return 'only_here' if sha and hashed == total else 'probable'


def _recompute_all(catalog, db):
    for target in targets(catalog):
        if target['role'] != 'archive':
            _recompute(catalog, db, target)


def _recompute(catalog, db, target):
    index = db.execute('SELECT run_id FROM indexes WHERE target_id=?', (target['id'],)).fetchone()
    if index is None:
        return
    evidence, run = _Evidence(catalog, db, target), index['run_id']
    totals, chains = {}, {}

    def chain(folder):
        cached = chains.get(folder)
        if cached is None:
            bucket = totals.setdefault(folder, dict.fromkeys(SAFETY, 0))
            cached = (bucket,) if not folder else (bucket,) + chain(folder.rpartition('/')[0])
            chains[folder] = cached
        return cached

    rows = db.execute('''SELECT e.parent,e.size,e.allocated,e.kind,h.sha256 FROM entries e LEFT JOIN hashes h ON h.target_id=e.target_id AND h.path=e.path
                         AND h.size=e.size AND h.mtime_ns=e.mtime_ns WHERE e.target_id=? AND e.run_id=?''', (target['id'], run))
    for row in rows:
        state = evidence.state(row['size'], row['kind'], row['sha256'])
        for bucket in chain(row['parent']):
            bucket[state] += row['allocated']
    output = []
    for row in db.execute('SELECT path,allocated,kinds FROM folders WHERE target_id=? AND run_id=?', (target['id'], run)):
        bucket = totals.get(row['path'], dict.fromkeys(SAFETY, 0))
        # Files too small to keep a row are still in the rollup: derived by name, otherwise not reviewed.
        rest = max(0, row['allocated'] - sum(bucket.values()))
        derived_rest = min(rest, max(0, json.loads(row['kinds']).get('derived', 0) - bucket['derived']))
        output.append((target['id'], row['path'], bucket['verified'], bucket['probable'], bucket['only_here'],
                       bucket['derived'] + derived_rest, bucket['unknown'] + rest - derived_rest))
    with db:
        db.execute('DELETE FROM safety WHERE target_id=?', (target['id'],))
        db.executemany('INSERT INTO safety VALUES(?,?,?,?,?,?,?)', output)


# Compare

def compare(catalog, backend, target, path='', cancel=None):
    """Hash only what could match the archive, largest first. Finished hashes survive an interruption."""
    found = _target(catalog, target)
    path = _rel(path)
    with _exclusive(catalog), contextlib.closing(open_index(catalog)) as db:
        require(found['role'] != 'archive', 'invalid_role', 'The archive is what gets compared against')
        index = db.execute('SELECT run_id FROM indexes WHERE target_id=?', (found['id'],)).fetchone()
        require(index is not None, 'not_indexed', 'Scan ' + found['name'] + ' first')
        require(_online(backend, found) is not None, 'volume_offline', found['name'] + ' is disconnected')
        _refresh_independence(catalog, backend, db)
        evidence = _Evidence(catalog, db, found)
        archives = [a for a in evidence.archives if a['id'] in {i['id'] for i in evidence.indexed} and _online(backend, a) is not None]
        require(archives, 'archive_not_available', 'Connect and scan the backup disk first')
        run = index['run_id']
        wanted = {}
        for archive in archives:
            archive_run = db.execute('SELECT run_id FROM indexes WHERE target_id=?', (archive['id'],)).fetchone()['run_id']
            for row in db.execute('SELECT path,size,mtime_ns FROM entries WHERE target_id=? AND run_id=? AND size>0', (archive['id'], archive_run)):
                wanted.setdefault(row['size'], []).append((archive, row['path'], row['mtime_ns']))
        scope = (run, found['id']) + ((path, path + '/', path + '0') if path else ())
        where = 'AND (e.path=? OR (e.path>=? AND e.path<?))' if path else ''
        candidates = [row for row in db.execute('''SELECT e.path,e.size,e.mtime_ns,h.sha256 FROM entries e LEFT JOIN hashes h ON h.target_id=e.target_id AND h.path=e.path
                      AND h.size=e.size AND h.mtime_ns=e.mtime_ns WHERE e.run_id=? AND e.target_id=? AND e.size>0 AND e.kind!='derived' {} ORDER BY e.size DESC'''.format(where), scope)
                      if row['size'] in wanted and row['sha256'] not in evidence.located]
        total_bytes = sum(row['size'] for row in candidates)
        with db:
            run_id = db.execute("INSERT INTO runs(target_id,kind,state,pid,path,started_at,total_files,total_bytes) VALUES(?,?,?,?,?,?,?,?)",
                                (found['id'], 'compare', 'running', os.getpid(), path, now(), len(candidates), total_bytes)).lastrowid
        counters = {'files': 0, 'hashed': 0, 'matched': 0, 'skipped': 0, 'bytes': 0, 'touched': time.monotonic()}

        def report(force=False):
            if force or time.monotonic() - counters['touched'] >= FLUSH_SECONDS:
                with db:
                    db.execute('UPDATE runs SET files=?,allocated=?,changed=? WHERE id=?', (counters['files'], counters['bytes'], counters['skipped'], run_id))
                counters['touched'] = time.monotonic()

        def advance(count):
            counters['bytes'] += count
            report()

        def digest_of(tree, owner, rel, size, mtime_ns, cached, track):
            """None when the file no longer is what the index recorded; that one waits for the next scan."""
            if cached:
                return cached
            row = db.execute('SELECT sha256 FROM hashes WHERE target_id=? AND path=? AND size=? AND mtime_ns=?', (owner, rel, size, mtime_ns)).fetchone()
            if row:
                return row[0]
            try:
                fd = tree.open(rel)
            except (OSError, MediaError) as error:
                # One file gone waits for the next scan; a disk gone stops the comparison.
                tree.check()
                return None
            try:
                first = os.fstat(fd)
                if (first.st_size, first.st_mtime_ns) != (size, mtime_ns):
                    return None
                value = hash_fd(uncached(fd), cancel or (lambda: False), advance if track else None)
                last = os.fstat(fd)
                if (last.st_size, last.st_mtime_ns) != (size, mtime_ns):
                    return None
            finally:
                os.close(fd)
            with db:
                db.execute('INSERT OR REPLACE INTO hashes VALUES(?,?,?,?,?,?,?)', (owner, rel, size, mtime_ns, value, 'computed', now()))
            counters['hashed'] += 1
            return value

        state, trees = 'failed', {}
        try:
            with Tree(found['root']) as source:
                for archive in archives:
                    trees[archive['id']] = Tree(archive['root']).__enter__()
                for row in candidates:
                    require(not (cancel and cancel()), 'cancelled', 'Comparison cancelled')
                    sha = digest_of(source, found['id'], row['path'], row['size'], row['mtime_ns'], row['sha256'], True)
                    if sha is None:
                        counters['skipped'] += 1
                    else:
                        for archive, other, mtime_ns in wanted[row['size']]:
                            other_sha = digest_of(trees[archive['id']], archive['id'], other, row['size'], mtime_ns, None, False)
                            if other_sha == sha:
                                counters['matched'] += 1
                                break
                    counters['files'] += 1
                    report()
            state = 'finished'
        except MediaError as error:
            if error.code != 'cancelled':
                with db:
                    db.execute('UPDATE runs SET error=? WHERE id=?', (str(error), run_id))
                raise
            state = 'cancelled'
        except OSError as error:
            with db:
                db.execute('UPDATE runs SET error=? WHERE id=?', (str(error), run_id))
            raise
        finally:
            for tree in trees.values():
                tree.__exit__()
            report(True)
            with db:
                db.execute('UPDATE runs SET state=?,finished_at=? WHERE id=?', (state, now(), run_id))
            _recompute(catalog, db, found)
        return {'state': state, 'target': found['id'], 'path': path, 'files': counters['files'], 'total_files': len(candidates),
                'hashed': counters['hashed'], 'matched': counters['matched'], 'skipped': counters['skipped'], 'bytes': counters['bytes']}


# Reads

def _run_view(row):
    if row is None:
        return None
    value = dict(row)
    value['samples'] = json.loads(value['samples'])
    if value['state'] == 'running' and not _alive(value['pid']):
        value['state'] = 'interrupted'
    return value


def _safety_view(db, target, path, allocated):
    if target['role'] == 'archive':
        return {'backup': allocated}
    row = db.execute('SELECT * FROM safety WHERE target_id=? AND path=?', (target['id'], path)).fetchone()
    return {key: row[key] for key in SAFETY} if row else {'unknown': allocated}


def progress(db):
    row = db.execute("SELECT * FROM runs WHERE state='running' ORDER BY id DESC LIMIT 1").fetchone()
    view = _run_view(row)
    return view if view and view['state'] == 'running' else None


def overview(catalog, backend, db):
    from .volumes import CachedBackend
    backend, result = CachedBackend(backend), []
    for target in targets(catalog):
        info = _online(backend, target)
        index = db.execute('SELECT * FROM indexes WHERE target_id=?', (target['id'],)).fetchone()
        last = _run_view(db.execute('SELECT * FROM runs WHERE target_id=? ORDER BY id DESC LIMIT 1', (target['id'],)).fetchone())
        value = {'id': target['id'], 'name': target['name'], 'role': target['role'], 'root': target['root'], 'observed': target['observed'], 'builtin': bool(target.get('builtin')),
                 'online': info is not None, 'indexed': index is not None, 'last_run': last}
        usage = None
        if info is not None:
            with contextlib.suppress(OSError):
                usage = shutil.disk_usage(target['root'])
        if index is not None:
            root = db.execute('SELECT files,bytes,allocated,kinds FROM folders WHERE target_id=? AND run_id=? AND path=?', (target['id'], index['run_id'], '')).fetchone()
            value.update({'indexed_at': index['indexed_at'], 'seconds': round(index['seconds'], 3), 'unreadable': index['unreadable'], 'samples': json.loads(index['samples']),
                          'files': root['files'], 'bytes': root['bytes'], 'allocated': root['allocated'], 'kinds': json.loads(root['kinds']),
                          'safety': _safety_view(db, target, '', root['allocated'])})
        value['fs_total'] = usage.total if usage else index['fs_total'] if index else None
        value['fs_used'] = usage.used if usage else index['fs_used'] if index else None
        value['fs_free'] = usage.free if usage else index['fs_free'] if index else None
        # Used space the walk did not reach: system, snapshots, purgeable, other folders or denied access.
        value['uncounted'] = max(0, value['fs_used'] - value['allocated']) if index is not None and value['fs_used'] is not None else None
        result.append(value)
    return {'targets': result, 'running': progress(db)}


def tree(catalog, db, target, path='', limit=200):
    found = _target(catalog, target)
    path = _rel(path)
    require(isinstance(limit, int) and 0 < limit <= 1000, 'invalid_request', 'Invalid page size')
    index = db.execute('SELECT * FROM indexes WHERE target_id=?', (found['id'],)).fetchone()
    if index is None:
        return {'target': found['id'], 'indexed': False, 'path': path, 'folders': [], 'files': []}
    run = index['run_id']
    row = db.execute('SELECT * FROM folders WHERE target_id=? AND run_id=? AND path=?', (found['id'], run, path)).fetchone()
    require(row is not None, 'unknown_path', 'Folder is not in the index: ' + _printable(path))

    def folder_view(item):
        return {'path': item['path'], 'name': item['path'].rpartition('/')[2], 'files': item['files'], 'bytes': item['bytes'], 'allocated': item['allocated'],
                'kinds': json.loads(item['kinds']), 'kind': item['kind'], 'opaque': bool(item['opaque']), 'safety': _safety_view(db, found, item['path'], item['allocated'])}

    children = [] if row['opaque'] else [folder_view(item) for item in db.execute(
        "SELECT * FROM folders WHERE target_id=? AND run_id=? AND parent=? AND path!='' ORDER BY allocated DESC LIMIT ?", (found['id'], run, path, limit))]
    files = []
    if not row['opaque']:
        rows = db.execute('''SELECT e.path,e.size,e.allocated,e.mtime_ns,e.kind,h.sha256,h.origin,h.checked_at FROM entries e LEFT JOIN hashes h ON h.target_id=e.target_id AND h.path=e.path
                             AND h.size=e.size AND h.mtime_ns=e.mtime_ns WHERE e.target_id=? AND e.run_id=? AND e.parent=? ORDER BY e.allocated DESC LIMIT ?''', (found['id'], run, path, limit)).fetchall()
        if rows:
            evidence = None if found['role'] == 'archive' else _Evidence(catalog, db, found)
            for item in rows:
                state = 'backup' if evidence is None else evidence.state(item['size'], item['kind'], item['sha256'])
                value = {'path': item['path'], 'name': item['path'].rpartition('/')[2], 'bytes': item['size'], 'allocated': item['allocated'], 'kind': item['kind'],
                         'state': state, 'checked_at': item['checked_at']}
                if state == 'verified':
                    value['copy'] = {'volume': evidence.located[item['sha256']][0], 'path': evidence.located[item['sha256']][1], 'origin': evidence.located[item['sha256']][2]}
                files.append(value)
    listed = sum(item['allocated'] for item in children) + sum(item['allocated'] for item in files)
    return {'target': found['id'], 'indexed': True, 'indexed_at': index['indexed_at'], 'role': found['role'], 'root': found['root'], 'path': path, 'name': path.rpartition('/')[2] or found['name'], 'parent': row['parent'],
            'files_total': row['files'], 'bytes': row['bytes'], 'allocated': row['allocated'], 'kinds': json.loads(row['kinds']), 'kind': row['kind'], 'opaque': bool(row['opaque']),
            'safety': _safety_view(db, found, path, row['allocated']), 'folders': children, 'files': files, 'rest': max(0, row['allocated'] - listed)}


def call(catalog, backend, method, params, cancel=None):
    try:
        if method == 'storage_scan':
            return scan(catalog, backend, params['target'], cancel)
        if method == 'storage_compare':
            return compare(catalog, backend, params['target'], params.get('path') or '', cancel)
        if method == 'storage_add_folder':
            return add_folder(catalog, backend, params['root'], params.get('name'))
        with contextlib.closing(open_index(catalog)) as db:
            if method == 'storage_overview':
                return overview(catalog, backend, db)
            if method == 'storage_progress':
                return {'running': progress(db)}
            if method == 'storage_tree':
                return tree(catalog, db, params['target'], params.get('path') or '', params.get('limit') or 200)
            if method == 'storage_remove_folder':
                return remove_folder(catalog, db, params['target'])
    except sqlite3.Error as error:
        raise MediaError('storage_index_error', 'Disk index unavailable: ' + str(error)) from error
    require(False, 'unknown_method', 'Unknown storage method: ' + str(method))
