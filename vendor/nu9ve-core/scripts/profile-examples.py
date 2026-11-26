#!/usr/bin/env python3
"""Writes data/profile-examples-v1.json, the examples of contracts/nu9ve-profile-v1.md.

Files go as JSON *text*, so each language reads them with its own parser. Every example runs through the
reference in scripts/nu9ve_profiles.py while this file is written, so an example that contradicts it is never
written.

    python3 -I scripts/profile-examples.py          rewrite the file
    python3 -I scripts/profile-examples.py --check  fail if the file is not what this script writes
"""

import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
OUT = HERE.parent / 'data' / 'profile-examples-v1.json'
sys.path.insert(0, str(HERE))
import nu9ve_profiles as profiles  # noqa: E402

DEVICE = '6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f'
LIB_NOVE = '/Users/qa/Library/Application Support/nove-development/engine'
LIB_PAPA = '/Users/qa/Library/Application Support/nove-development/profiles/papa/engine'
LIB_NUUN = '/Users/qa/Library/Application Support/nuun/profiles/e64'
NULL = object()  # a field that is present and null (None removes it)


def base(**changes):
    doc = {
        'contract': 'nu9ve-profile/1',
        'device_id': DEVICE,
        'profiles': [
            {'profile_id': 'e64', 'display_name': 'Erik', 'kind': 'person', 'aliases': ['erikiado64'],
             'default_space': None, 'account_link': None, 'created_at': '2026-11-25T18:00:00Z',
             'apps': {
                 'nuun': {'library': LIB_NUUN, 'last_used_at': '2026-11-25T18:00:00Z', 'default': True,
                          'ui': None},
                 'nove': {'library': LIB_NOVE, 'last_used_at': '2026-11-25T19:30:00Z', 'default': True,
                          'ui': 'full'}}},
            {'profile_id': 'papa', 'display_name': 'Papá', 'kind': 'viewer', 'aliases': [],
             'default_space': None, 'account_link': None, 'created_at': '2026-11-25T19:00:00Z',
             'apps': {'nove': {'library': LIB_PAPA, 'last_used_at': '2026-11-25T19:10:00Z', 'default': False,
                               'ui': 'simple'}}},
        ],
        'updated_at': '2026-11-25T19:30:00Z',
    }
    for key, val in changes.items():
        if val is None:
            doc.pop(key, None)
        else:
            doc[key] = None if val is NULL else val
    return doc


def text(doc):
    return profiles.dumps(doc)


def with_profile(index, **changes):
    doc = base()
    profile = doc['profiles'][index]
    for key, val in changes.items():
        if val is None:
            profile.pop(key, None)
        else:
            profile[key] = val
    return text(doc)


def with_entry(**changes):
    doc = base()
    entry = doc['profiles'][1]['apps']['nove']
    for key, val in changes.items():
        if val is None:
            entry.pop(key, None)
        else:
            entry[key] = val
    return text(doc)


def build():
    two_defaults = base()
    two_defaults['profiles'][1]['apps']['nove']['default'] = True
    duplicate = base()
    duplicate['profiles'][1]['profile_id'] = 'e64'
    many_apps = {'app%d' % i: {'library': '/x', 'default': False} for i in range(33)}
    many_profiles = base(profiles=[
        {'profile_id': 'p%d' % i, 'display_name': 'P', 'kind': 'person', 'created_at': '2026-11-25T18:00:00Z',
         'apps': {}} for i in range(65)])
    files = [
        ('two profiles in two apps', text(base()), None),
        ('no device yet', text(base(device_id=NULL)), None),
        ('empty', text(base(profiles=[])), None),
        ('optional profile fields absent', with_profile(1, aliases=None, default_space=None, account_link=None), None),
        ('optional entry fields absent', with_entry(last_used_at=None, ui=None), None),
        ('a Windows library', with_entry(library='C:\\Users\\papa\\AppData\\Local\\nove\\engine'), None),
        ('unknown fields are ignored', with_profile(0, avatar={'emoji': 'e'}), None),
        ('a team with a space and an account link', with_profile(1, kind='team', default_space='space-acme',
                                                                 account_link='fractos:acct-1'), None),
        ('retired ids', text(base(retired=['old-1'])), None),
        ('missing profiles', text(base(profiles=None)), 'missing_field'),
        ('device_id absent (it is null when there is no device)', text(base(device_id=None)), 'missing_field'),
        ('profiles is an object', text(base(profiles={})), 'wrong_type'),
        ('other contract', text(base(contract='nu9ve-profile/2')), 'unknown_contract'),
        ('device_id not a UUID 4', text(base(device_id='M4')), 'bad_device_id'),
        ('updated_at with an offset', text(base(updated_at='2026-11-25T13:30:00-06:00')), 'bad_time'),
        ('more than 64 profiles', text(many_profiles), 'too_many_profiles'),
        ('profile_id with upper case', with_profile(0, profile_id='E64'), 'bad_profile_id'),
        ('profile_id starts with a hyphen', with_profile(0, profile_id='-e64'), 'bad_profile_id'),
        ('profile_id longer than 32', with_profile(0, profile_id='e' * 33), 'bad_profile_id'),
        ('empty display_name', with_profile(0, display_name=''), 'bad_name'),
        ('unknown kind', with_profile(0, kind='family'), 'bad_kind'),
        ('created_at missing', with_profile(0, created_at=None), 'missing_field'),
        ('aliases repeat', with_profile(0, aliases=['a', 'a']), 'bad_alias'),
        ('alias with a control character', with_profile(0, aliases=['e\n']), 'bad_alias'),
        ('empty default_space', with_profile(0, default_space=''), 'bad_space'),
        ('account_link as a number', with_profile(0, account_link=7), 'wrong_type'),
        ('apps as a list', with_profile(0, apps=[]), 'wrong_type'),
        ('app key with upper case', with_profile(1, apps={'Nove': {'library': '/x', 'default': False}}), 'bad_app'),
        ('more than 32 apps', with_profile(1, apps=many_apps), 'too_many_apps'),
        ('relative library', with_entry(library='profiles/papa'), 'bad_library'),
        ('library missing', with_entry(library=None), 'missing_field'),
        ('default as a string', with_entry(default='no'), 'wrong_type'),
        ('unknown ui', with_entry(ui='kiosk'), 'bad_ui'),
        ('last_used_at not a time', with_entry(last_used_at='ayer'), 'bad_time'),
        ('the same profile_id twice', text(duplicate), 'duplicate_profile'),
        ('two defaults for one app', text(two_defaults), 'two_defaults'),
        ('a retired id still in use', text(base(retired=['papa'])), 'bad_retired'),
        ('not an object', '[]', 'not_object'),
        ('not JSON', '{"contract":', 'not_json'),
        ('NaN is not JSON', '{"contract":NaN}', 'not_json'),
        ('starts with a BOM', '\ufeff' + text(base()), 'bom'),
        ('larger than 64 KiB', text(base(padding='x' * profiles.MAX_BYTES)), 'too_large'),
    ]
    file_cases = []
    for name, body, want in files:
        got = profiles.file_problem(body)
        assert got == want, (name, got, want)
        file_cases.append({'name': name, 'text': body, 'problem': want})

    t = ['2026-11-25T20:00:0%dZ' % i for i in range(10)]
    sequences = [
        ('nove adopts the personal library as e64, nüün already had it', text(base(profiles=[{
            'profile_id': 'e64', 'display_name': 'Erik', 'kind': 'person', 'aliases': ['erikiado64'],
            'default_space': None, 'account_link': None, 'created_at': '2026-11-25T18:00:00Z',
            'apps': {'nuun': {'library': LIB_NUUN, 'last_used_at': '2026-11-25T18:00:00Z', 'default': True,
                              'ui': None, 'pinned': True}}}])), None, [
            ({'op': 'create_profile', 'profile_id': 'e64', 'display_name': 'Erik', 'kind': 'person'}, t[0],
             'profile_exists'),
            ({'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nove', 'library': LIB_NOVE, 'default': True,
              'ui': 'full'}, t[1], None),
        ]),
        ('first writer: no file, a new profile with its library', None, DEVICE, [
            ({'op': 'create_profile', 'profile_id': 'papa', 'display_name': 'Papá', 'kind': 'viewer'}, t[0], None),
            ({'op': 'upsert_app_entry', 'profile_id': 'papa', 'app': 'nove', 'library': LIB_PAPA, 'ui': 'simple'},
             t[1], None),
            ({'op': 'upsert_app_entry', 'profile_id': 'nadie', 'app': 'nove', 'library': LIB_PAPA}, t[2],
             'unknown_profile'),
        ]),
        ('a default moves; the other app is untouched', text(base()), None, [
            ({'op': 'upsert_app_entry', 'profile_id': 'papa', 'app': 'nove', 'library': LIB_PAPA, 'default': True,
              'ui': 'simple'}, t[0], None),
        ]),
        ('delete only when no app uses it; the id is never reused', text(base()), None, [
            ({'op': 'delete_profile', 'profile_id': 'papa'}, t[0], 'profile_in_use'),
            ({'op': 'remove_app_entry', 'profile_id': 'papa', 'app': 'nove'}, t[1], None),
            ({'op': 'remove_app_entry', 'profile_id': 'papa', 'app': 'nove'}, t[2], None),
            ({'op': 'delete_profile', 'profile_id': 'papa'}, t[3], None),
            ({'op': 'create_profile', 'profile_id': 'papa', 'display_name': 'Otro', 'kind': 'person'}, t[4],
             'profile_retired'),
            ({'op': 'delete_profile', 'profile_id': 'papa'}, t[5], 'unknown_profile'),
        ]),
        ('updates and refusals', text(base()), None, [
            ({'op': 'update_profile', 'profile_id': 'papa', 'display_name': 'Papá (Windows)',
              'aliases': ['papa-win'], 'default_space': 'space-family'}, t[0], None),
            ({'op': 'update_profile', 'profile_id': 'papa', 'kind': 'robot'}, t[1], 'bad_kind'),
            ({'op': 'create_profile', 'profile_id': 'Team', 'display_name': 'T', 'kind': 'team'}, t[2],
             'bad_profile_id'),
            ({'op': 'create_profile', 'profile_id': 'team', 'kind': 'team'}, t[3], 'bad_change'),
            ({'op': 'create_profile', 'profile_id': 'team', 'display_name': '', 'kind': 'team'}, t[4], 'bad_name'),
            ({'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'Nove', 'library': LIB_NOVE}, t[5], 'bad_app'),
            ({'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nove', 'library': 'engine'}, t[6],
             'bad_library'),
            ({'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nove', 'library': LIB_NOVE, 'ui': 'tiny'},
             t[7], 'bad_ui'),
            ({'op': 'rename_everything', 'profile_id': 'e64'}, t[8], 'bad_change'),
        ]),
    ]
    sequence_cases = []
    for name, start, device_id, steps in sequences:
        doc = profiles.load(start) if start is not None else profiles.new_document(device_id, '2026-11-25T20:00:00Z')
        step_cases = []
        for op, now, want in steps:
            try:
                doc = profiles.apply(doc, op, now)
                got = None
            except profiles.ProfileError as error:
                got = error.code
            assert got == want, (name, op, got, want)
            step_cases.append({'change': op, 'now': now, 'error': want})
        assert profiles.document_problem(doc) is None, name
        sequence_cases.append({'name': name, 'start': start, 'device_id': device_id if start is None else None,
                               'steps': step_cases, 'result': doc})

    listings = []
    for app in ('nove', 'nuun', 'clipper'):
        listings.append({'app': app, 'text': text(base()),
                         'profiles': [r['profile_id'] for r in profiles.for_app(base(), app)]})
    assert listings[0]['profiles'] == ['e64', 'papa'] and listings[2]['profiles'] == []
    recent = base()
    recent['profiles'][0]['apps']['nove']['default'] = False
    listings.append({'app': 'nove', 'text': text(recent),
                     'profiles': [r['profile_id'] for r in profiles.for_app(recent, 'nove')]})
    assert listings[-1]['profiles'] == ['e64', 'papa']
    recent['profiles'][1]['apps']['nove']['last_used_at'] = '2026-11-25T21:00:00Z'
    listings.append({'app': 'nove', 'text': text(recent),
                     'profiles': [r['profile_id'] for r in profiles.for_app(recent, 'nove')]})
    assert listings[-1]['profiles'] == ['papa', 'e64']

    locations = [
        ('macos', {'HOME': '/Users/ana'}, '/Users/ana/Library/Application Support/nu9ve/profiles.json'),
        ('windows', {'LOCALAPPDATA': 'C:\\Users\\ana\\AppData\\Local'},
         'C:\\Users\\ana\\AppData\\Local\\nu9ve\\profiles.json'),
        ('linux', {'HOME': '/home/ana'}, '/home/ana/.local/share/nu9ve/profiles.json'),
        ('linux', {'HOME': '/home/ana', 'XDG_DATA_HOME': '/data/ana'}, '/data/ana/nu9ve/profiles.json'),
        ('macos', {'HOME': '/Users/ana', 'NU9VE_PROFILES_FILE': '/tmp/qa/profiles.json'}, '/tmp/qa/profiles.json'),
        ('macos', {'HOME': '/Users/ana', 'NU9VE_PROFILES_FILE': 'qa/profiles.json'}, None),
        ('windows', {'NU9VE_PROFILES_FILE': 'D:\\qa\\profiles.json'}, 'D:\\qa\\profiles.json'),
        ('windows', {}, None),
    ]
    location_cases = []
    for platform, env, want in locations:
        got = profiles.profiles_file(platform, env)
        assert got == want, (platform, env, got)
        location_cases.append({'platform': platform, 'env': env, 'file': want})

    document = {
        'contract': 'contracts/nu9ve-profile-v1.md',
        'regenerate': 'python3 -I scripts/profile-examples.py',
        'note': 'locations: §1; files: problem null means valid (§2, §3); sequences: changes in order (§5), '
                'start null means no file (a new document with device_id and updated_at 2026-11-25T20:00:00Z), '
                'error null means the change applied, result is the document after the last step; '
                'listings: profile ids of one app in order (§6).',
        'locations': location_cases,
        'files': file_cases,
        'sequences': sequence_cases,
        'listings': listings,
    }
    return json.dumps(document, ensure_ascii=False, indent=2) + '\n'


def main():
    body = build()
    if sys.argv[1:] == ['--check']:
        if not OUT.is_file() or OUT.read_text(encoding='utf-8') != body:
            print('%s is out of date: python3 -I scripts/profile-examples.py' % OUT, file=sys.stderr)
            return 1
        return 0
    if sys.argv[1:]:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    OUT.write_text(body, encoding='utf-8')
    return 0


if __name__ == '__main__':
    sys.exit(main())
