"""F5 (d): `nuun-engine media mcp` answers the conversation `nu9ve.py media mcp` (media/mcp.py) answers.

One conversation (initialize, tools/list, every tool, bad calls, notifications, parse errors) goes to both servers
on two catalogs of the golden file. Responses must be equal as values; the `text` of a tool result is JSON and is
compared parsed, since Rust sorts keys. The free and used bytes of a live file system and `measured_at` are left out. `HOME` is a temporary folder, so no tool reads this Mac's agent folders.
Skipped when the engine is not built (`cargo build -p nuun-engine`).
"""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest

from tests.test_engine_examples import rust_engine

REPO = Path(__file__).resolve().parents[1]
GOLDEN = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'recipe.json'


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


def steady(value):
    """Without what moves between the two runs: the free and used bytes of a live file system and the second
    a collection was measured in."""
    if isinstance(value, dict):
        return {k: steady(v) for k, v in value.items() if k not in {'fs_free', 'fs_used', 'measured_at'}}
    if isinstance(value, list):
        return [steady(v) for v in value]
    return value


def answers(stdout):
    out = []
    for line in stdout.splitlines():
        response = json.loads(line)
        for content in (response.get('result') or {}).get('content') or []:
            if not response['result']['isError']:
                content['text'] = json.loads(content['text'])
        out.append(steady(response))
    return out


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class MediaMcpTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='nuun-media-mcp-')
        root = Path(cls.temp.name).resolve()
        golden = json.loads(GOLDEN.read_text(encoding='utf-8'))
        cls.homes = {}
        for case in ('reads', 'projects'):
            home = root / case
            home.mkdir()
            db = sqlite3.connect(home / 'catalog.sqlite3')
            db.executescript(golden[case]['sql'])
            db.commit()
            db.close()
            cls.homes[case] = home
        (root / 'user').mkdir()
        cls.env = dict(os.environ, HOME=str(root / 'user'), NUUN_MEDIA_ENGINE='python')

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def run_both(self, home):
        python = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', str(home), 'mcp'],
                                input=LINES, capture_output=True, text=True, cwd=REPO, env=self.env, timeout=120)
        rust = subprocess.run([rust_engine(), 'media', '--state-dir', str(home), 'mcp'],
                              input=LINES, capture_output=True, text=True, env=self.env, timeout=120)
        return python, rust

    def test_the_same_conversation(self):
        for case, home in self.homes.items():
            with self.subTest(case=case):
                python, rust = self.run_both(home)
                self.assertEqual((rust.returncode, python.returncode), (0, 0), rust.stderr + python.stderr)
                theirs, ours = answers(python.stdout), answers(rust.stdout)
                self.assertEqual(len(ours), len(theirs))
                self.maxDiff = None
                for mine, expected in zip(ours, theirs):
                    self.assertEqual(mine, expected)
                # Every tool answered something real at least once, not only an error.
                self.assertFalse(ours[4]['result']['isError'])
                self.assertFalse(ours[7]['result']['isError'])

    def test_only_reads(self):
        _, rust = self.run_both(self.homes['reads'])
        tools = answers(rust.stdout)[3]['result']['tools']
        self.assertEqual(len(tools), 8)
        self.assertTrue(all(t['annotations']['readOnlyHint'] and not t['annotations']['destructiveHint'] for t in tools))

    def test_nu9ve_py_hands_the_mcp_to_rust(self):
        home = str(self.homes['reads'])
        old = Path(self.temp.name) / 'old-engine'
        old.write_text('#!/bin/sh\necho "usage: nuun-engine rpc-serve | rpc" >&2\nexit 2\n', encoding='utf-8')
        old.chmod(0o755)
        def nu9ve(**env):
            env = dict(self.env, **env)
            env.pop('NUUN_MEDIA_ENGINE', None)
            return subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', home, 'mcp'],
                                  input=LINES, capture_output=True, text=True, cwd=REPO, env=env, timeout=120)
        rust = subprocess.run([rust_engine(), 'media', '--state-dir', home, 'mcp'], input=LINES, capture_output=True,
                              text=True, env=self.env, timeout=120)
        python, _ = self.run_both(self.homes['reads'])
        # Who answered shows in the key order of the first line: Rust sorts (`error` first), Python does not.
        handed, kept = nu9ve(NUUN_ENGINE=rust_engine()).stdout, nu9ve(NUUN_ENGINE=str(old)).stdout
        self.assertTrue(handed.startswith('{"error"') and rust.stdout.startswith('{"error"'))
        self.assertTrue(kept.startswith('{"jsonrpc"') and python.stdout.startswith('{"jsonrpc"'))
        ids = lambda out: [response.get('id') for response in answers(out)]
        self.assertEqual(ids(handed), ids(rust.stdout))
        self.assertEqual(ids(kept), ids(python.stdout))


if __name__ == '__main__':
    unittest.main()
