"""nu9ve-profile-v1 in Python: the reference for contracts/nu9ve-profile-v1.md and a helper for nüün and Atlas.

Standard library only, so an app can use its pinned copy without installing anything:

    import sys
    sys.dont_write_bytecode = True  # a __pycache__ inside vendor/ would fail sync-core.sh --check
    sys.path.insert(0, 'vendor/nu9ve-core/scripts')
    import nu9ve_profiles as profiles

    path = profiles.default_file()                  # None: no file for this process (§1)
    doc = profiles.read(path)                       # None: no file yet; ProfileError('invalid', …) if broken
    for entry in profiles.for_app(doc, 'nuun'):     # this app's profiles, the default one first
        print(entry['profile_id'], entry['library'])
    profiles.change(path, {'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nuun',
                           'library': '/Users/ana/Library/Application Support/nuun/profiles/e64'})

`file_problem` is the validator: None when a file follows the contract, otherwise the code of the first
problem. `apply` is a change on a document in memory; `change` does it on the file, under the lock (§4).
scripts/profile-examples.py writes data/profile-examples-v1.json through both, and the Rust crate
`nu9ve-profiles` answers the same codes.
"""

import copy
import json
import os
import re
import unicodedata
from datetime import datetime, timezone

CONTRACT = 'nu9ve-profile/1'
PROFILE_ID = re.compile(r'[a-z0-9][a-z0-9-]{0,31}')
APP = re.compile(r'[a-z][a-z0-9-]{0,31}')
UUID4 = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}')
RFC3339_UTC = re.compile(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?Z')
KINDS = ('person', 'team', 'viewer')
UIS = ('full', 'simple')
MAX_BYTES = 64 * 1024
MAX_PROFILES = 64
MAX_APPS = 32
MAX_ALIASES = 16
MAX_RETIRED = 1024
MAX_PATH = 1024
LOCK_TIMEOUT_S = 5.0


class ProfileError(Exception):
    """`code` is the contract's code: a problem of the file (§3) or why a change was refused (§5)."""

    def __init__(self, code, message=''):
        super().__init__(message or code)
        self.code = code


def load(text):
    def reject(constant):
        raise ValueError('not JSON: ' + constant)
    return json.loads(text, parse_constant=reject)


def profiles_file(platform, env):
    """§1: the file, or None when there is none (never a guess)."""
    override = env.get('NU9VE_PROFILES_FILE')
    if override is not None:
        absolute = re.match(r'[A-Za-z]:\\', override) if platform == 'windows' else override.startswith('/')
        return override if absolute else None
    if platform == 'macos' and env.get('HOME', '').startswith('/'):
        return env['HOME'].rstrip('/') + '/Library/Application Support/nu9ve/profiles.json'
    if platform == 'windows' and env.get('LOCALAPPDATA'):
        return env['LOCALAPPDATA'].rstrip('\\') + '\\nu9ve\\profiles.json'
    if platform == 'linux':
        data = env.get('XDG_DATA_HOME', '')
        if data.startswith('/'):
            return data.rstrip('/') + '/nu9ve/profiles.json'
        if env.get('HOME', '').startswith('/'):
            return env['HOME'].rstrip('/') + '/.local/share/nu9ve/profiles.json'
    return None


def this_platform():
    if os.name == 'nt':
        return 'windows'
    return 'macos' if os.uname().sysname == 'Darwin' else 'linux'


def default_file():
    """§1 for this process."""
    return profiles_file(this_platform(), os.environ)


def now_utc():
    return datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def text_ok(value, longest):
    return (isinstance(value, str) and 1 <= len(value) <= longest
            and not any(unicodedata.category(c) == 'Cc' for c in value))


def name_ok(value):
    """§2: a name a person reads: 1 to 64 characters, no control characters."""
    return text_ok(value, 64)


def library_ok(value):
    """§2: an absolute path of this machine (POSIX, drive or UNC), up to 1024 characters."""
    return (text_ok(value, MAX_PATH)
            and (value.startswith('/') or re.match(r'[A-Za-z]:\\', value) is not None or value.startswith('\\\\')))


def _fields(obj, required, optional):
    """missing_field or wrong_type for the fields of one object; kinds are str, list, dict, bool, 'str?'."""
    def fits(value, kind):
        if kind == 'str?':
            return value is None or isinstance(value, str)
        return isinstance(value, kind)
    for name, kind in required:
        if name not in obj:
            return 'missing_field'
        if not fits(obj[name], kind):
            return 'wrong_type'
    for name, kind in optional:
        if name in obj and not fits(obj[name], kind):
            return 'wrong_type'
    return None


def _entry_problem(entry):
    if not isinstance(entry, dict):
        return 'wrong_type'
    problem = _fields(entry, (('library', str), ('default', bool)), (('last_used_at', 'str?'), ('ui', 'str?')))
    if problem:
        return problem
    if not library_ok(entry['library']):
        return 'bad_library'
    if entry.get('last_used_at') is not None and not RFC3339_UTC.fullmatch(entry['last_used_at']):
        return 'bad_time'
    if entry.get('ui') is not None and entry['ui'] not in UIS:
        return 'bad_ui'
    return None


def _profile_problem(profile):
    if not isinstance(profile, dict):
        return 'wrong_type'
    problem = _fields(
        profile,
        (('profile_id', str), ('display_name', str), ('kind', str), ('created_at', str), ('apps', dict)),
        (('aliases', list), ('default_space', 'str?'), ('account_link', 'str?')))
    if problem:
        return problem
    if not PROFILE_ID.fullmatch(profile['profile_id']):
        return 'bad_profile_id'
    if not name_ok(profile['display_name']):
        return 'bad_name'
    if profile['kind'] not in KINDS:
        return 'bad_kind'
    if not RFC3339_UTC.fullmatch(profile['created_at']):
        return 'bad_time'
    aliases = profile.get('aliases', [])
    if len(aliases) > MAX_ALIASES or not all(name_ok(a) for a in aliases) or len(set(aliases)) != len(aliases):
        return 'bad_alias'
    if profile.get('default_space') is not None and not text_ok(profile['default_space'], 128):
        return 'bad_space'
    if profile.get('account_link') is not None and not text_ok(profile['account_link'], 256):
        return 'bad_account_link'
    apps = profile['apps']
    if len(apps) > MAX_APPS:
        return 'too_many_apps'
    for app in sorted(apps):
        if not APP.fullmatch(app):
            return 'bad_app'
        problem = _entry_problem(apps[app])
        if problem:
            return problem
    return None


def document_problem(value):
    """§2 and §3 on a parsed document: None when valid, otherwise the first problem."""
    if not isinstance(value, dict):
        return 'not_object'
    problem = _fields(value, (('contract', str), ('device_id', 'str?'), ('profiles', list), ('updated_at', str)),
                      (('retired', list),))
    if problem:
        return problem
    if value['contract'] != CONTRACT:
        return 'unknown_contract'
    if value['device_id'] is not None and not UUID4.fullmatch(value['device_id']):
        return 'bad_device_id'
    if not RFC3339_UTC.fullmatch(value['updated_at']):
        return 'bad_time'
    if len(value['profiles']) > MAX_PROFILES:
        return 'too_many_profiles'
    seen = set()
    for profile in value['profiles']:
        problem = _profile_problem(profile)
        if problem:
            return problem
        if profile['profile_id'] in seen:
            return 'duplicate_profile'
        seen.add(profile['profile_id'])
    defaults = {}
    for profile in value['profiles']:
        for app, entry in profile['apps'].items():
            if entry['default']:
                defaults[app] = defaults.get(app, 0) + 1
    if any(count > 1 for count in defaults.values()):
        return 'two_defaults'
    retired = value.get('retired', [])
    if (len(retired) > MAX_RETIRED
            or not all(isinstance(r, str) and PROFILE_ID.fullmatch(r) and r not in seen for r in retired)
            or len(set(retired)) != len(retired)):
        return 'bad_retired'
    return None


def file_problem(text):
    """§3: None when the file is valid, otherwise why not."""
    if len(text.encode('utf-8')) > MAX_BYTES:
        return 'too_large'
    if text.startswith('﻿'):
        return 'bom'
    try:
        value = load(text)
    except ValueError:
        return 'not_json'
    return document_problem(value)


def dumps(value):
    """How a writer serialises the document: compact UTF-8."""
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'))


def new_document(device_id, now):
    """§4: what a writer starts from when there is no file."""
    return {'contract': CONTRACT, 'device_id': device_id, 'profiles': [], 'updated_at': now}


def _find(doc, profile_id):
    for profile in doc['profiles']:
        if profile['profile_id'] == profile_id:
            return profile
    raise ProfileError('unknown_profile')


def _check_fields(op):
    if 'display_name' in op and not name_ok(op['display_name']):
        raise ProfileError('bad_name')
    if 'kind' in op and op['kind'] not in KINDS:
        raise ProfileError('bad_kind')
    if 'aliases' in op:
        aliases = op['aliases']
        if (not isinstance(aliases, list) or len(aliases) > MAX_ALIASES
                or not all(name_ok(a) for a in aliases) or len(set(aliases)) != len(aliases)):
            raise ProfileError('bad_alias')
    if op.get('default_space') is not None and not text_ok(op['default_space'], 128):
        raise ProfileError('bad_space')
    if op.get('account_link') is not None and not text_ok(op['account_link'], 256):
        raise ProfileError('bad_account_link')


PROFILE_FIELDS = ('display_name', 'kind', 'aliases', 'default_space', 'account_link')


def apply(doc, op, now):
    """§5: one change on a valid document; returns the new document or raises ProfileError(code)."""
    doc = copy.deepcopy(doc)
    kind = op.get('op')
    profile_id = op.get('profile_id')
    if not isinstance(profile_id, str) or not PROFILE_ID.fullmatch(profile_id):
        raise ProfileError('bad_profile_id')
    if kind == 'create_profile':
        if 'display_name' not in op or 'kind' not in op:
            raise ProfileError('bad_change')
        _check_fields(op)
        if any(p['profile_id'] == profile_id for p in doc['profiles']):
            raise ProfileError('profile_exists')
        if profile_id in doc.get('retired', []):
            raise ProfileError('profile_retired')
        if len(doc['profiles']) >= MAX_PROFILES:
            raise ProfileError('too_many_profiles')
        doc['profiles'].append({
            'profile_id': profile_id, 'display_name': op['display_name'], 'kind': op['kind'],
            'aliases': list(op.get('aliases', [])), 'default_space': op.get('default_space'),
            'account_link': op.get('account_link'), 'created_at': now, 'apps': {},
        })
    elif kind == 'update_profile':
        _check_fields(op)
        profile = _find(doc, profile_id)
        for field in PROFILE_FIELDS:
            if field in op:
                profile[field] = copy.deepcopy(op[field])
    elif kind in ('upsert_app_entry', 'remove_app_entry'):
        app = op.get('app')
        if not isinstance(app, str) or not APP.fullmatch(app):
            raise ProfileError('bad_app')
        profile = _find(doc, profile_id)
        if kind == 'remove_app_entry':
            profile['apps'].pop(app, None)
        else:
            if not library_ok(op.get('library')):
                raise ProfileError('bad_library')
            default = op.get('default', False)
            if not isinstance(default, bool):
                raise ProfileError('bad_change')
            ui = op.get('ui')
            if ui is not None and ui not in UIS:
                raise ProfileError('bad_ui')
            if app not in profile['apps'] and len(profile['apps']) >= MAX_APPS:
                raise ProfileError('too_many_apps')
            entry = profile['apps'].setdefault(app, {})
            entry.update({'library': op['library'], 'last_used_at': now, 'default': default, 'ui': ui})
            if default:
                for other in doc['profiles']:
                    if other is not profile and app in other['apps']:
                        other['apps'][app]['default'] = False
    elif kind == 'delete_profile':
        profile = _find(doc, profile_id)
        if profile['apps']:
            raise ProfileError('profile_in_use')
        doc['profiles'] = [p for p in doc['profiles'] if p is not profile]
        doc.setdefault('retired', []).append(profile_id)
    else:
        raise ProfileError('bad_change')
    doc['updated_at'] = now
    problem = file_problem(dumps(doc))
    if problem:
        raise ProfileError(problem)
    return doc


def for_app(doc, app):
    """§6: the profiles with an entry for `app`: the default one first, then the most recently used."""
    if doc is None:
        return []
    rows = []
    for profile in doc['profiles']:
        entry = profile['apps'].get(app)
        if entry is None:
            continue
        rows.append({
            'profile_id': profile['profile_id'], 'display_name': profile['display_name'], 'kind': profile['kind'],
            'library': entry['library'], 'last_used_at': entry.get('last_used_at'), 'default': entry['default'],
            'ui': entry.get('ui'),
        })
    rows.sort(key=lambda r: r['profile_id'])
    rows.sort(key=lambda r: r['last_used_at'] or '', reverse=True)
    rows.sort(key=lambda r: not r['default'])
    return rows


def read(path):
    """§4: the document, None when there is no file, ProfileError('invalid'…) when it breaks §3 or, on
    macOS and Linux, is not a regular file or the group or others can read it."""
    import stat
    try:
        info = os.lstat(path)
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(info.st_mode) or (os.name != 'nt' and info.st_mode & 0o077):
        raise ProfileError('invalid', 'unsafe_permissions')
    try:
        with open(path, 'rb') as f:
            data = f.read(MAX_BYTES + 1)
    except FileNotFoundError:
        return None
    try:
        text = data.decode('utf-8')
    except UnicodeDecodeError:
        raise ProfileError('invalid', 'not_json')
    problem = file_problem(text)
    if problem:
        raise ProfileError('invalid', problem)
    return load(text)


class _Lock:
    """§4: the exclusive lock on `profiles.json.lock`, the same one the Rust crate takes."""

    def __init__(self, path):
        self.path = path + '.lock'
        self.fd = None

    def __enter__(self):
        import time
        self.fd = os.open(self.path, os.O_RDWR | os.O_CREAT, 0o600)
        deadline = time.monotonic() + LOCK_TIMEOUT_S
        while True:
            try:
                if os.name == 'nt':
                    import msvcrt
                    msvcrt.locking(self.fd, msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return self
            except OSError:
                if time.monotonic() > deadline:
                    os.close(self.fd)
                    raise ProfileError('busy', 'profiles.json is locked by another process')
                time.sleep(0.01)

    def __exit__(self, *exc):
        os.close(self.fd)  # closing releases the lock


def change(path, op, device_id=None, now=None):
    """§4 and §5: apply one change to the file under the lock and publish it; returns the new document."""
    parent = os.path.dirname(path)
    os.makedirs(parent, mode=0o700, exist_ok=True)
    if os.name != 'nt':
        os.chmod(parent, 0o700)
    with _Lock(path):
        doc = read(path)
        now = now or now_utc()
        if doc is None:
            doc = new_document(device_id, now)
        elif doc['device_id'] is None and device_id is not None:
            doc['device_id'] = device_id
        doc = apply(doc, op, now)
        temp = '%s.%s.tmp' % (path, os.urandom(6).hex())
        fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            with os.fdopen(fd, 'wb') as f:
                f.write(dumps(doc).encode('utf-8'))
                f.flush()
                os.fsync(f.fileno())
            os.replace(temp, path)
        except BaseException:
            if os.path.exists(temp):
                os.remove(temp)
            raise
    return doc
