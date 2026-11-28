"""F5 (b) and (f): `nuun-engine media <order>` answers what `nu9ve.py media <order>` answered, for the orders other
apps send (docs/platforms/media-cli-callers.md: Clipper, Kyū and Stalker's development fallback).

Since (f) Python's answers are frozen in tests/fixtures/engine-golden/media-answers.json (scripts/media_answers.py,
which also says how the catalogs are built), so Rust is checked against them without running Python. Values are
compared, not key order: the Rust engine sorts keys and the callers parse the JSON. Usage errors exit 2.
`ForwardTest` checks that `nu9ve.py media` hands these orders to Rust (F5 c) and falls back to Python.
Skipped when the engine is not built (`cargo build -p nuun-engine`).
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from tests.test_engine_examples import rust_engine

REPO = Path(__file__).resolve().parents[1]


def media_answers():
    spec = importlib.util.spec_from_file_location('media_answers', REPO / 'scripts' / 'media_answers.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


A = media_answers()


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class MediaCliTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.saved = json.loads(A.TARGET.read_text(encoding='utf-8'))
        cls.found = A.answers([rust_engine(), 'media'], A.homes())

    def test_orders_answer_what_python_answered(self):
        answered = set()
        for case, orders in self.saved['orders'].items():
            for expected, found in zip(orders, self.found['orders'][case]):
                with self.subTest(case=case, order=expected['order']):
                    self.assertEqual(found['order'], expected['order'])
                    self.assertEqual((found['exit'], found['answer']), (expected['exit'], expected['answer']))
                    if found['answer']['ok'] and found['answer']['data']:
                        answered.add(expected['order'][0])
            self.assertEqual(len(self.found['orders'][case]), len(orders))
        # Each order the other apps send answered with data at least once, not only with the same error.
        self.assertLessEqual({'projects', 'sources', 'volumes', 'resolve'}, answered)

    def test_usage_errors_exit_2_without_an_envelope(self):
        for expected, found in zip(self.saved['usage'], self.found['usage']):
            with self.subTest(order=expected['order']):
                self.assertEqual((found['exit'], found['stdout']), (2, ''))
                self.assertEqual(expected['exit'], 2)

    def test_integration_capabilities_answers(self):
        # Not frozen: it says what this Mac has installed.
        child = subprocess.run([rust_engine(), 'media', '--state-dir', str(A.ROOT / 'reads'), 'integration-capabilities'],
                               capture_output=True, text=True, env=A.environment())
        answer = json.loads(child.stdout)
        self.assertTrue(answer['ok'])
        self.assertLessEqual({'clipper', 'stalker'}, set(answer['data']))

    def test_state_dir_goes_before_or_after_media(self):
        home = str(A.ROOT / 'reads')
        before = subprocess.run([rust_engine(), '--state-dir', home, 'media', 'projects'], capture_output=True, text=True)
        after = subprocess.run([rust_engine(), 'media', '--state-dir', home, 'projects'], capture_output=True, text=True)
        self.assertEqual(json.loads(before.stdout), json.loads(after.stdout))
        self.assertTrue(json.loads(after.stdout)['data'])


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class ForwardTest(unittest.TestCase):
    """F5 (c): `nu9ve.py media` answers the read orders with Rust, and with Python when Rust cannot."""

    @classmethod
    def setUpClass(cls):
        cls.home = A.homes()['reads']
        cls.temp = tempfile.TemporaryDirectory(prefix='nuun-forward-')
        # An engine older than `nuun-engine media` prints its usage and exits 2; another one only says it ran.
        cls.old = Path(cls.temp.name) / 'old-engine'
        cls.old.write_text('#!/bin/sh\necho "usage: nuun-engine rpc-serve | rpc" >&2\nexit 2\n', encoding='utf-8')
        cls.marker = Path(cls.temp.name) / 'marker-engine'
        cls.marker.write_text('#!/bin/sh\necho \'{"ok": true, "command": "marker", "data": {}}\'\n', encoding='utf-8')
        for script in (cls.old, cls.marker):
            script.chmod(0o755)

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def nu9ve(self, order, **env):
        base = {k: v for k, v in os.environ.items() if k != 'NUUN_MEDIA_ENGINE'}
        return subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', str(self.home), *order],
                              capture_output=True, text=True, cwd=REPO, env=dict(base, **env))

    def test_read_orders_are_rusts_answer(self):
        for order in (['projects'], ['resolve', 'a1'], ['volumes', 'list'], ['resolve', 'nope']):
            with self.subTest(order=order):
                forwarded = self.nu9ve(order, NUUN_ENGINE=rust_engine())
                rust = subprocess.run([rust_engine(), 'media', '--state-dir', str(self.home), *order], capture_output=True, text=True)
                self.assertEqual((forwarded.returncode, forwarded.stdout), (rust.returncode, rust.stdout))

    def test_an_engine_without_the_cli_leaves_it_to_python(self):
        forwarded = self.nu9ve(['projects'], NUUN_ENGINE=str(self.old))
        python = self.nu9ve(['projects'], NUUN_MEDIA_ENGINE='python')
        self.assertEqual((forwarded.returncode, forwarded.stdout), (python.returncode, python.stdout))
        self.assertTrue(json.loads(forwarded.stdout)['ok'])

    def test_usage_errors_are_pythons(self):
        forwarded = self.nu9ve(['sources'], NUUN_ENGINE=rust_engine())
        self.assertEqual((forwarded.returncode, forwarded.stdout), (2, ''))
        self.assertIn('--project', forwarded.stderr)

    def test_other_orders_stay_in_python(self):
        for order in (['recipe'], ['devices'], ['projects', '--help']):
            with self.subTest(order=order):
                answer = self.nu9ve(order, NUUN_ENGINE=str(self.marker))
                self.assertNotIn('marker', answer.stdout)
        self.assertIn('marker', self.nu9ve(['projects'], NUUN_ENGINE=str(self.marker)).stdout)
        self.assertNotIn('marker', self.nu9ve(['projects'], NUUN_ENGINE=str(self.marker), NUUN_MEDIA_ENGINE='python').stdout)


if __name__ == '__main__':
    unittest.main()
