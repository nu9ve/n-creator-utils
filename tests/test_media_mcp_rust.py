"""F5 (d) and (f): `nuun-engine media mcp` answers the conversation `nu9ve.py media mcp` (media/mcp.py) answered.

One conversation (initialize, tools/list, every tool, bad calls, notifications, parse errors; scripts/media_answers.py)
on two catalogs of the golden file, against Python's responses frozen in
tests/fixtures/engine-golden/media-answers.json. The `text` of a tool result is JSON and is compared parsed, since
Rust sorts keys. Also: `nu9ve.py media mcp` hands the process to Rust, or keeps Python with an older engine.
Skipped when the engine is not built (`cargo build -p nuun-engine`).
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from tests.test_engine_examples import rust_engine
from tests.test_media_cli_rust import A

REPO = Path(__file__).resolve().parents[1]


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class MediaMcpTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.saved = json.loads(A.TARGET.read_text(encoding='utf-8'))['mcp']
        cls.homes = A.homes()

    def mcp(self, argv, home, **env):
        return subprocess.run([*argv, '--state-dir', str(home), 'mcp'], input=A.LINES, capture_output=True, text=True,
                              cwd=REPO, env=dict(A.environment(), **env), timeout=120)

    def test_the_conversation_python_answered(self):
        for case, home in self.homes.items():
            with self.subTest(case=case):
                rust = self.mcp([rust_engine(), 'media'], home)
                self.assertEqual(rust.returncode, 0, rust.stderr)
                ours, theirs = A.mcp_answers(rust.stdout), self.saved[case]['responses']
                self.assertEqual(len(ours), len(theirs))
                self.maxDiff = None
                for mine, expected in zip(ours, theirs):
                    self.assertEqual(mine, expected)
                # Tools answered something real, not only errors.
                self.assertFalse(ours[4]['result']['isError'])
                self.assertFalse(ours[7]['result']['isError'])

    def test_only_reads(self):
        tools = A.mcp_answers(self.mcp([rust_engine(), 'media'], self.homes['reads']).stdout)[3]['result']['tools']
        self.assertEqual(len(tools), 8)
        self.assertTrue(all(t['annotations']['readOnlyHint'] and not t['annotations']['destructiveHint'] for t in tools))

    def test_nu9ve_py_hands_the_mcp_to_rust(self):
        home = self.homes['reads']
        with tempfile.TemporaryDirectory() as temp:
            old = Path(temp) / 'old-engine'
            old.write_text('#!/bin/sh\necho "usage: nuun-engine rpc-serve | rpc" >&2\nexit 2\n', encoding='utf-8')
            old.chmod(0o755)
            nu9ve = [sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media']
            env = {k: v for k, v in A.environment().items() if k != 'NUUN_MEDIA_ENGINE'}
            run = lambda engine: subprocess.run([*nu9ve, '--state-dir', str(home), 'mcp'], input=A.LINES, capture_output=True,
                                                text=True, cwd=REPO, env=dict(env, NUUN_ENGINE=engine), timeout=120)
            handed, kept = run(rust_engine()).stdout, run(str(old)).stdout
        # Who answered shows in the key order of the first line: Rust sorts (`error` first), Python does not.
        self.assertTrue(handed.startswith('{"error"'), handed[:80])
        self.assertTrue(kept.startswith('{"jsonrpc"'), kept[:80])
        ids = [response.get('id') for response in self.saved['reads']['responses']]
        self.assertEqual([r.get('id') for r in A.mcp_answers(handed)], ids)
        self.assertEqual([r.get('id') for r in A.mcp_answers(kept)], ids)


if __name__ == '__main__':
    unittest.main()
