#!/usr/bin/env python3
"""F5 (f), 2026-11-27: Python's answers to the orders and the MCP conversation other apps send to `nu9ve.py media`,
frozen in git (tests/fixtures/engine-golden/media-answers.json), so the Rust engine is still checked against them
when `media/` is gone.

    python3 scripts/media_answers.py           # write the file from Python (only on purpose; review the diff)
    python3 scripts/media_answers.py --check   # Python still answers what the file says (while media/ exists)

The catalogs are two of the golden file (`reads` and `projects`, from tests/fixtures/engine-golden/recipe.json),
built under one fixed folder so paths are the same on every run; `HOME` is a folder inside it, so no answer reads
this Mac's agent folders. What depends on the Mac or moves between runs (its disk's size and use, the second a
collection was measured in) is left out, and so is `integration-capabilities`, which says what this Mac has
installed. tests/test_media_cli_rust.py and tests/test_media_mcp_rust.py compare
`nuun-engine media` against this file.
"""
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys

REPO = Path(__file__).resolve().parents[1]
GOLDEN = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'recipe.json'
TARGET = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'media-answers.json'
# Resolved, as the scan refuses /tmp → /private/tmp; only this script and the tests write there.
ROOT = Path('/private/tmp/nuun-media-answers')
# What depends on this Mac or moves between runs: the size and use of its disk, and when a collection was measured.
MOVING = {'fs_total', 'fs_free', 'fs_used', 'measured_at'}

# docs/platforms/media-cli-callers.md: Clipper, Kyū and Stalker's development fallback.
ORDERS = {'reads': [
    ['projects'],
    ['sources', '--project', 'nope'],
    ['volumes', 'list'],
    ['resolve', 'a1'],
    ['resolve', 'a1', '--revision', 'r1'],
    ['resolve', 'a1', '--revision', 'r9'],
    ['resolve', 'nope'],
    ['retitle-operations'],
    ['captures', '/nonexistent-nuun-library'],
], 'projects': [
    ['projects'],
    ['sources', '--project', 'terra'],
    ['sources', '--project', 'terra', '--purpose', 'archive'],
    ['sources', '--purpose', 'edit', '--project', 'terra'],
    ['volumes', 'list'],
]}
USAGE = [['sources'], ['sources', '--project', 'p', '--purpose', 'other'], ['resolve'], ['volumes'], ['nope'], []]


def call(n, name, arguments=None):
    params = {'name': name} if arguments is None else {'name': name, 'arguments': arguments}
    return {'jsonrpc': '2.0', 'id': n, 'method': 'tools/call', 'params': params}


CONVERSATION = [
    {'jsonrpc': '2.0', 'id': 0, 'method': 'tools/list'},  # before initialize
    {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2024-11-05'}},
    {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
    {'jsonrpc': '2.0', 'id': 2, 'method': 'ping'},
    {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/list'},
    call(4, 'media_library_search', {}),
    call(5, 'media_library_search', {'project': 'terra'}),
    call(6, 'media_library_search', {'work_state': 'ready'}),
    call(7, 'media_asset_resolve', {'asset_id': 'a1'}),
    call(8, 'media_asset_resolve', {'asset_id': 'nope'}),
    call(9, 'media_jobs', {}),
    call(10, 'media_job_status', {'job_id': 'nope'}),
    call(11, 'media_volumes'),
    call(12, 'media_storage_overview', {}),
    call(13, 'media_storage_collections', {}),
    call(14, 'media_asset_resolve', {}),  # missing argument
    call(15, 'media_jobs', {'extra': 'x'}),
    call(16, 'media_job_status', {'job_id': 3}),
    call(17, 'apply', {}),  # not a tool: never a write
    {'jsonrpc': '2.0', 'id': 18, 'method': 'resources/list'},
    {'jsonrpc': '2.0', 'id': 19, 'method': 7},
    {'jsonrpc': '1.0', 'id': 20, 'method': 'ping'},
    {'jsonrpc': '2.0', 'id': 21, 'method': 'initialize', 'params': {'protocolVersion': '1999-01-01'}},
]
LINES = '\n'.join(json.dumps(message) for message in CONVERSATION) + '\nnot json\n'


def homes():
    """The two catalogs, rebuilt from the golden SQL in their fixed folders, and the HOME of the run."""
    golden = json.loads(GOLDEN.read_text(encoding='utf-8'))
    if ROOT.exists():
        shutil.rmtree(ROOT)
    out = {}
    for case in ORDERS:
        home = ROOT / case
        home.mkdir(parents=True)
        db = sqlite3.connect(home / 'catalog.sqlite3')
        db.executescript(golden[case]['sql'])
        db.commit()
        db.close()
        out[case] = home
    (ROOT / 'user').mkdir()
    return out


def environment():
    return dict(os.environ, HOME=str(ROOT / 'user'), NUUN_MEDIA_ENGINE='python')


def steady(value):
    if isinstance(value, dict):
        return {k: steady(v) for k, v in value.items() if k not in MOVING}
    if isinstance(value, list):
        return [steady(v) for v in value]
    return value


def mcp_answers(stdout):
    """The MCP responses as values: a tool result's `text` is JSON, compared parsed (Rust sorts keys)."""
    out = []
    for line in stdout.splitlines():
        response = json.loads(line)
        for content in (response.get('result') or {}).get('content') or []:
            if not response['result']['isError']:
                content['text'] = json.loads(content['text'])
        out.append(steady(response))
    return out


def answers(media, homes):
    """What `media` (an argv prefix before `--state-dir`) answers: each order, usage errors and the MCP."""
    env = environment()
    run = lambda home, *args, **kw: subprocess.run([*media, '--state-dir', str(home), *args], capture_output=True,
                                                   text=True, cwd=REPO, env=env, timeout=120, **kw)
    out = {'orders': {}, 'usage': [], 'mcp': {}}
    for case, orders in ORDERS.items():
        out['orders'][case] = []
        for order in orders:
            child = run(homes[case], *order)
            out['orders'][case].append({'order': order, 'exit': child.returncode, 'answer': steady(json.loads(child.stdout))})
    for order in USAGE:
        child = run(homes['reads'], *order)
        out['usage'].append({'order': order, 'exit': child.returncode, 'stdout': child.stdout})
    for case, home in homes.items():
        child = run(home, 'mcp', input=LINES)
        out['mcp'][case] = {'exit': child.returncode, 'responses': mcp_answers(child.stdout)}
    return out


def python_media():
    return [sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media']


def main():
    found = answers(python_media(), homes())
    if '--check' in sys.argv[1:]:
        saved = json.loads(TARGET.read_text(encoding='utf-8'))
        if saved == json.loads(json.dumps(found)):
            print('media answers: up to date')
            return 0
        print('media answers: %s differs from Python; run scripts/media_answers.py and review the diff' % TARGET)
        return 1
    TARGET.write_text(json.dumps(found, ensure_ascii=False, indent=1, sort_keys=True) + '\n', encoding='utf-8')
    print(TARGET)
    return 0


if __name__ == '__main__':
    sys.exit(main())
