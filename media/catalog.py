"""One local durable catalog; receipts on destination volumes are independent."""
import contextlib
import json
import os
from pathlib import Path
import sqlite3
import sys

from .common import MediaError, canonical, now, require, uid


def default_home():
    if os.environ.get('NU9VE_MEDIA_HOME'):
        return Path(os.environ['NU9VE_MEDIA_HOME']).expanduser()
    if sys.platform == 'darwin':
        return Path.home() / 'Library/Application Support/n-creator-utils/media'
    return Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share')) / 'n-creator-utils/media'


class Catalog:
    def __init__(self, home=None):
        self.home = Path(home or default_home()).expanduser().absolute()
        for path in (self.home, *self.home.parents):
            require(not path.is_symlink(), 'unsafe_state', 'State path must not contain symlinks: ' + str(path))
        self.home.mkdir(parents=True, exist_ok=True, mode=0o700)
        db = self.home / 'catalog.sqlite3'
        require(not db.is_symlink(), 'unsafe_state', 'Catalog must not be a symlink')
        self.db = sqlite3.connect(str(db), timeout=15)
        self.db.row_factory = sqlite3.Row
        exists = self.db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='metadata'").fetchone()
        if exists:
            version = self.db.execute("SELECT value FROM metadata WHERE key='schema'").fetchone()
            require(version is not None and version[0] == '1', 'unsupported_catalog', 'Unsupported catalog version')
        self.db.execute('PRAGMA journal_mode=WAL')
        self.db.execute('PRAGMA synchronous=FULL')
        self.db.executescript('''
          CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS volumes(id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, digest TEXT NOT NULL, plan TEXT NOT NULL,
            state TEXT NOT NULL, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT, job_id TEXT NOT NULL,
            kind TEXT NOT NULL, payload TEXT NOT NULL, created_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS members(job_id TEXT NOT NULL, relative_path TEXT NOT NULL,
            sha256 TEXT NOT NULL, cleaned INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(job_id,relative_path));
          CREATE TABLE IF NOT EXISTS copies(job_id TEXT NOT NULL, volume_id TEXT NOT NULL,
            relative_path TEXT NOT NULL, sha256 TEXT NOT NULL, payload TEXT NOT NULL,
            PRIMARY KEY(job_id,volume_id,relative_path));
          CREATE TABLE IF NOT EXISTS sessions(session_id TEXT NOT NULL, manifest_digest TEXT NOT NULL, asset_id TEXT NOT NULL, PRIMARY KEY(session_id,manifest_digest,asset_id));
          CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, profile TEXT NOT NULL, title TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS project_aliases(profile TEXT NOT NULL, alias TEXT NOT NULL, project_id TEXT NOT NULL, PRIMARY KEY(profile,alias));
          CREATE TABLE IF NOT EXISTS asset_projects(asset_id TEXT NOT NULL, project_id TEXT NOT NULL, PRIMARY KEY(asset_id,project_id));
          CREATE TABLE IF NOT EXISTS asset_jobs(asset_id TEXT NOT NULL, job_id TEXT NOT NULL, PRIMARY KEY(asset_id,job_id));
          CREATE TABLE IF NOT EXISTS publications(asset_id TEXT NOT NULL, variant TEXT NOT NULL, channel TEXT NOT NULL, state TEXT NOT NULL,
            url TEXT, updated_at TEXT NOT NULL, PRIMARY KEY(asset_id,variant,channel));
          CREATE TABLE IF NOT EXISTS assets(id TEXT PRIMARY KEY, job_id TEXT NOT NULL, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS work(asset_id TEXT PRIMARY KEY, state TEXT NOT NULL,
            target TEXT, updated_at TEXT NOT NULL);
        ''')
        version = self.db.execute("SELECT value FROM metadata WHERE key='schema'").fetchone()
        require(version is None or version[0] == '1', 'unsupported_catalog', 'Unsupported catalog version')
        self.db.execute("INSERT OR IGNORE INTO metadata VALUES('schema','1')")
        self.db.commit()

    def close(self):
        self.db.close()

    def set(self, key, value):
        with self.db:
            self.db.execute('INSERT OR REPLACE INTO metadata VALUES(?,?)', (key, canonical(value).decode()))

    def get(self, key, default=None):
        row = self.db.execute('SELECT value FROM metadata WHERE key=?', (key,)).fetchone()
        return json.loads(row[0]) if row else default

    def volumes(self):
        return [json.loads(row[0]) for row in self.db.execute('SELECT payload FROM volumes ORDER BY name')]

    def volume(self, name):
        result = next((v for v in self.volumes() if name in {v['name'], v['id']}), None)
        require(result is not None, 'unknown_volume', 'Unknown registered volume: ' + str(name))
        return result

    def register(self, name, role, root, identity, prefix):
        require(role in {'archive', 'work'}, 'invalid_role', 'Volume role must be archive or work')
        from .common import relative, slug
        slug(name)
        relative(prefix)
        require(not any(v['root'] == str(root) or v['name'] == name for v in self.volumes()),
                'volume_exists', 'Root/name already registered')
        value = {'id': uid(), 'name': name, 'role': role, 'root': str(root), 'identity': identity, 'prefix': prefix}
        with self.db:
            self.db.execute('INSERT INTO volumes VALUES(?,?,?)', (value['id'], name, canonical(value).decode()))
        return value

    @contextlib.contextmanager
    def writer(self):
        # Global mutation lock covers overlapping sources, CLI and app jobs.
        import fcntl
        path = self.home / 'writer.lock'
        fd = os.open(str(path), os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        try:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise MediaError('job_busy', 'Another media operation is running') from error
            yield
        finally:
            os.close(fd)

    def event(self, job_id, kind, data):
        with self.db:
            self.db.execute('INSERT INTO events(job_id,kind,payload,created_at) VALUES(?,?,?,?)',
                            (job_id, kind, canonical(data).decode(), now()))

    def state(self, job_id, state, error=None):
        with self.db:
            self.db.execute('UPDATE jobs SET state=?,error=?,updated_at=? WHERE id=?',
                            (state, error, now(), job_id))
        self.event(job_id, state, {'error': error} if error else {})

    def job(self, job_id):
        row = self.db.execute('SELECT * FROM jobs WHERE id=?', (job_id,)).fetchone()
        require(row is not None, 'unknown_job', 'Job not found')
        result = dict(row)
        result['plan'] = json.loads(result['plan'])
        result['progress'] = {'verified_copies': self.db.execute('SELECT COUNT(*) FROM copies WHERE job_id=?', (job_id,)).fetchone()[0],
                              'cleaned_files': self.db.execute('SELECT COUNT(*) FROM members WHERE job_id=? AND cleaned=1', (job_id,)).fetchone()[0],
                              'total_files': len(result['plan']['files'])}
        return result

    def jobs(self):
        return [dict(row) for row in self.db.execute('SELECT id,state,error,updated_at FROM jobs ORDER BY created_at DESC LIMIT 200')]

    def project_for(self, profile, title):
        row = self.db.execute('SELECT project_id FROM project_aliases WHERE profile=? AND alias=?', (profile, title)).fetchone()
        if row:
            return row[0]
        project_id = uid()
        with self.db:
            self.db.execute('INSERT INTO projects VALUES(?,?,?)', (project_id, profile, title))
            self.db.execute('INSERT INTO project_aliases VALUES(?,?,?)', (profile, title, project_id))
        return project_id

    def rename_project(self, project_id, title):
        from .common import slug
        title = slug(title)
        with self.writer(), self.db:
            row = self.db.execute('SELECT profile FROM projects WHERE id=?', (project_id,)).fetchone()
            require(row is not None, 'unknown_project', 'Project not found')
            other = self.db.execute('SELECT project_id FROM project_aliases WHERE profile=? AND alias=?', (row[0], title)).fetchone()
            require(other is None or other[0] == project_id, 'project_conflict', 'Project title is already in use')
            self.db.execute('INSERT OR IGNORE INTO project_aliases VALUES(?,?,?)', (row[0], title, project_id))
            self.db.execute('UPDATE projects SET title=? WHERE id=?', (title, project_id))
            for item in self.db.execute('SELECT id,payload FROM assets').fetchall():
                asset = json.loads(item['payload'])
                if asset.get('project_id') == project_id:
                    asset['project'] = title
                    self.db.execute('UPDATE assets SET payload=? WHERE id=?', (canonical(asset).decode(), item['id']))
        return {'project_id': project_id, 'title': title}

    def search(self, project=None, work_state=None):
        result = []
        for row in self.db.execute('SELECT assets.payload,work.state FROM assets JOIN work ON work.asset_id=assets.id'):
            value = json.loads(row[0]); value['work_delivery_status'] = row[1]
            value['projects'] = [dict(p) for p in self.db.execute('SELECT projects.id,projects.title,projects.profile FROM projects JOIN asset_projects ON asset_projects.project_id=projects.id WHERE asset_projects.asset_id=?', (value['id'],))]
            if (not project or any(project.casefold() in p['title'].casefold() for p in value['projects'])) and (not work_state or row[1] == work_state):
                result.append(value)
        return result
