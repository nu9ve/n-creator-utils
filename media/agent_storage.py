"""`storage_collections`: space held by folders that are not media, starting with the agents' (Claude, Codex).

Only metadata: the walk lists folders and reads `lstat` (size, blocks, mtime). It never opens a file, never
follows a link and never leaves the root's device, so session content is neither read nor returned. Only the
names of each root's first level appear in the answer (fixed folders like `projects` or `Cache`); deeper names,
which carry repository paths and session ids, are summed into their first-level part.

What can be freed is a hint (`cache`), never an action: retiring a cache goes through a reviewed plan.
Weekly growth is estimated per file from when it appeared (`basis: created`, the macOS birth time; `mtime`
elsewhere): a session log counts whole in the week it was created, even if it grew later, and a deleted file is
not seen. The real series comes from sampling this answer over time (nove).

engine/crates/nuun-media/src/agent_storage.rs is the same reading in Rust; tests/test_agent_storage.py keeps
both equal on a fixture tree.
"""
import fnmatch
import os
import stat
import time
from datetime import datetime, timedelta, timezone

from .common import require

COLLECTION = 'agents'
CLASSES = ('history', 'state', 'cache', 'logs', 'other')
WEEKS = 8
DAY = 86400

# (id, name, path under the home folder, [(first-level glob, class)]). The first matching glob wins; a name
# that matches none is `other`, never assumed regenerable.
ROOTS = (
    ('claude_code', 'Claude Code', '.claude', (
        ('projects', 'history'), ('file-history', 'history'), ('history.jsonl', 'history'), ('todos', 'history'),
        ('plans', 'history'), ('transcripts', 'history'),
        ('shell-snapshots', 'cache'), ('session-env', 'cache'), ('statsig', 'cache'), ('cache', 'cache'),
        ('paste-cache', 'cache'), ('image-cache', 'cache'), ('ide', 'cache'),
        ('debug', 'logs'), ('logs', 'logs'), ('telemetry', 'logs'),
        ('settings*.json', 'state'), ('*.md', 'state'), ('agents', 'state'), ('commands', 'state'),
        ('skills', 'state'), ('plugins', 'state'), ('hooks', 'state'), ('keybindings.json', 'state'),
        ('.credentials.json', 'state'), ('memory', 'state'), ('.claude.json', 'state'), ('backups', 'state'),
        ('state', 'state'), ('.last-cleanup', 'state'), ('sessions', 'history'), ('mcp-needs-auth-cache.json', 'cache'))),
    ('codex', 'Codex', '.codex', (
        ('sessions', 'history'), ('archived_sessions', 'history'), ('history.jsonl', 'history'),
        ('thread_history*.sqlite*', 'history'),
        ('log', 'logs'), ('logs', 'logs'), ('logs_*.sqlite*', 'logs'),
        ('cache', 'cache'), ('tmp', 'cache'), ('.tmp', 'cache'), ('models_cache.json', 'cache'),
        ('config.toml', 'state'), ('auth.json', 'state'), ('*.md', 'state'), ('prompts', 'state'),
        ('skills', 'state'), ('rules', 'state'), ('memories', 'state'), ('version.json', 'state'),
        ('state*.sqlite*', 'state'), ('session_index.jsonl*', 'history'), ('attachments', 'history'),
        ('generated_images', 'history'), ('visualizations', 'history'), ('dictation-history', 'history'),
        ('transcription-history.jsonl', 'history'), ('*.tmp-*', 'cache'), ('shell_snapshots', 'cache'),
        ('vendor_imports', 'cache'), ('plugins', 'state'), ('goals_*.sqlite*', 'state'), ('memories_*.sqlite*', 'state'),
        ('queue_*.sqlite*', 'state'), ('.codex-global-state.json*', 'state'), ('*.lock', 'state'))),
    ('claude_app', 'Claude (app)', os.path.join('Library', 'Application Support', 'Claude'), (
        ('Cache', 'cache'), ('Code Cache', 'cache'), ('GPUCache', 'cache'), ('*Dawn*Cache', 'cache'),
        ('GPUPersistentCache', 'cache'), ('VideoDecodeStats', 'cache'), ('fcache', 'cache'), ('claude-code-spare', 'cache'),
        ('ShaderCache', 'cache'), ('GrShaderCache', 'cache'), ('Service Worker', 'cache'),
        ('Shared Dictionary', 'cache'), ('claude-code', 'cache'), ('claude-code-vm', 'cache'),
        ('logs', 'logs'), ('Crashpad', 'logs'), ('sentry', 'logs'),
        ('IndexedDB', 'state'), ('Local Storage', 'state'), ('Session Storage', 'state'), ('WebStorage', 'state'),
        ('blob_storage', 'state'), ('Preferences', 'state'), ('*.json', 'state'), ('Cookies*', 'state'),
        ('Network Persistent State', 'state'), ('TransportSecurity', 'state'), ('Trust Tokens*', 'state'),
        ('local-agent-mode-sessions', 'history'), ('claude-code-sessions', 'history'),
        ('side-session-notes', 'history'), ('DIPS*', 'state'), ('Local State', 'state'), ('Extension State', 'state'),
        ('shared_proto_db', 'state'))),
)


def classify(rules, name):
    """(part, class): names matched by a glob share one part named by the glob, so temporary files with ids
    become one line; an unmatched name is its own `other` part."""
    for pattern, kind in rules:
        if fnmatch.fnmatchcase(name, pattern):
            return (pattern if '*' in pattern else name), kind
    return name, 'other'


def _created(st):
    """When the file appeared: macOS birth time, else mtime. Growth by creation does not move a large file
    that is rewritten in place (an SQLite history) into this week."""
    return getattr(st, 'st_birthtime', st.st_mtime)


def _week_start(day):
    return day - (day + 3) % 7  # 1970-01-01 was a Thursday; weeks start Monday, UTC


def _date(day):
    return (datetime(1970, 1, 1, tzinfo=timezone.utc) + timedelta(days=day)).date().isoformat()


def _empty():
    return {'files': 0, 'bytes': 0, 'allocated': 0}


def _add(total, size, allocated):
    total['files'] += 1
    total['bytes'] += size
    total['allocated'] += allocated


class _Walk:
    def __init__(self, first_week):
        self.seen, self.unreadable, self.first_week = set(), 0, first_week
        self.weeks = {}

    def week(self, kind, size, mtime):
        start = _week_start(int(mtime // DAY))
        if start >= self.first_week:
            row = self.weeks.setdefault(start, {name: 0 for name in CLASSES})
            row[kind] += size

    def entry(self, path, st, device, kind, part):
        """One `lstat` already taken. Directories recurse; a hard link counts once; nothing is opened."""
        if stat.S_ISDIR(st.st_mode):
            if st.st_dev != device:
                return
            try:
                with os.scandir(path) as listing:
                    children = [(entry.path, entry.stat(follow_symlinks=False)) for entry in listing]
            except OSError:
                self.unreadable += 1
                return
            for child, child_st in sorted(children):
                self.entry(child, child_st, device, kind, part)
            return
        if st.st_nlink > 1:
            key = (st.st_dev, st.st_ino)
            if key in self.seen:
                return
            self.seen.add(key)
        allocated = getattr(st, 'st_blocks', 0) * 512 or st.st_size
        _add(part, st.st_size, allocated)
        part['newest'] = max(part.get('newest') or 0, int(st.st_mtime))
        self.week(kind, allocated, _created(st))


def _iso(seconds):
    return datetime.fromtimestamp(seconds, timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ') if seconds else None


def _root(walk, home, root_id, name, rel, rules):
    path = os.path.join(home, rel)
    view = {'id': root_id, 'name': name, 'path': '~/' + rel.replace(os.sep, '/'), 'state': 'ok', **_empty(),
            'newest': None, 'classes': {kind: _empty() for kind in CLASSES}, 'parts': [], 'unreadable': 0}
    try:
        top = os.stat(path)  # the root itself may be a link (dotfiles); nothing under it is followed
    except FileNotFoundError:
        view['state'] = 'missing'
        return view
    except OSError:
        view['state'] = 'unreadable'
        return view
    if not stat.S_ISDIR(top.st_mode):
        view['state'] = 'unreadable'
        return view
    try:
        with os.scandir(path) as listing:
            children = sorted((entry.name, entry.stat(follow_symlinks=False)) for entry in listing)
    except OSError:
        view['state'] = 'unreadable'
        return view
    before = walk.unreadable
    parts = {}
    for child, st in children:
        key, kind = classify(rules, child)
        part = parts.setdefault(key, {'name': key, 'class': kind, **_empty(), 'newest': 0})
        walk.entry(os.path.join(path, child), st, top.st_dev, kind, part)
    for part in parts.values():
        for key in ('files', 'bytes', 'allocated'):
            view[key] += part[key]
            view['classes'][part['class']][key] += part[key]
        view['newest'] = max(view['newest'] or 0, part['newest']) or None
        part['newest'] = _iso(part['newest'])
        view['parts'].append(part)
    view['newest'] = _iso(view['newest'])
    view['unreadable'] = walk.unreadable - before
    view['parts'].sort(key=lambda item: (-item['allocated'], item['name']))
    return view


def collections(home=None, now=None):
    home = home or os.path.expanduser('~')
    now = time.time() if now is None else now
    this_week = _week_start(int(now // DAY))
    first_week = this_week - 7 * WEEKS
    walk = _Walk(first_week)
    roots = [_root(walk, home, *spec) for spec in ROOTS]
    classes = {kind: _empty() for kind in CLASSES}
    for root in roots:
        for kind in CLASSES:
            for key in ('files', 'bytes', 'allocated'):
                classes[kind][key] += root['classes'][kind][key]
    weeks = []
    for start in range(first_week, this_week + 1, 7):
        row = walk.weeks.get(start, {kind: 0 for kind in CLASSES})
        weeks.append({'start': _date(start), 'complete': start < this_week, 'allocated': sum(row.values()), 'classes': dict(row)})
    full = [week for week in weeks if week['complete']][-4:]
    total = {key: sum(classes[kind][key] for kind in CLASSES) for key in ('files', 'bytes', 'allocated')}
    agents = {
        'id': COLLECTION, 'name': 'Agentes', 'basis': 'metadata', 'measured_at': _iso(int(now)), **total,
        'classes': classes, 'roots': roots,
        'growth': {'basis': 'created' if hasattr(os.stat_result, 'st_birthtime') else 'mtime', 'weeks': weeks,
                   'weekly_average': sum(week['allocated'] for week in full) // max(1, len(full)),
                   'history_weekly_average': sum(week['classes']['history'] for week in full) // max(1, len(full))},
        'release': {'automatic': False, 'candidates': classes['cache']['allocated'], 'needs': 'reviewed_plan'},
    }
    return {'collections': [agents]}


def call(params):
    require(not params, 'invalid_params', 'storage_collections accepts an empty params object.')
    return collections()
