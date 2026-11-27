"""F5 (b): `nuun-engine media <order>` answers what `nu9ve.py media <order>` answers, for the orders other apps send
(docs/platforms/media-cli-callers.md: Clipper, Kyū and Stalker's development fallback).

Both run on two catalogs of the golden file (scripts/engine_golden.py): `reads` and `projects`, which has
the editor sources and three registered disks (offline here, the same for both). The values must be equal; key order is not
compared, since the Rust engine sorts keys and the callers parse the JSON. Usage errors exit 2 in both.
Since F5 (c) `nu9ve.py media <order>` hands these orders to Rust; the comparison keeps Python with
`NUUN_MEDIA_ENGINE=python`, and `ForwardTest` checks the hand-off and its fallbacks.
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

ORDERS = {'reads': [
    ['projects'],
    ['sources', '--project', 'nope'],
    ['volumes', 'list'],
    ['resolve', 'a1'],
    ['resolve', 'a1', '--revision', 'r1'],
    ['resolve', 'a1', '--revision', 'r9'],
    ['resolve', 'nope'],
    ['integration-capabilities'],
    ['retitle-operations'],
    ['captures', '/nonexistent-nuun-library'],
], 'projects': [
    ['projects'],
    ['sources', '--project', 'terra'],
    ['sources', '--project', 'terra', '--purpose', 'archive'],
    ['sources', '--purpose', 'edit', '--project', 'terra'],
    ['volumes', 'list'],
]}
PYTHON = dict(os.environ, NUUN_MEDIA_ENGINE='python')
USAGE = [['sources'], ['sources', '--project', 'p', '--purpose', 'other'], ['resolve'], ['volumes'], ['nope'], []]


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class MediaCliTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='nuun-media-cli-')
        golden = json.loads(GOLDEN.read_text(encoding='utf-8'))
        cls.homes = {}
        for case in ORDERS:
            # Python refuses a state path through a symlink (/var → /private/var).
            home = Path(cls.temp.name).resolve() / case
            home.mkdir()
            db = sqlite3.connect(home / 'catalog.sqlite3')
            db.executescript(golden[case]['sql'])
            db.commit()
            db.close()
            cls.homes[case] = home
        cls.home = cls.homes['reads']

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def run_both(self, order, home=None):
        home = str(home or self.home)
        python = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', home, *order],
                                capture_output=True, text=True, cwd=REPO, env=PYTHON)
        rust = subprocess.run([rust_engine(), 'media', '--state-dir', home, *order], capture_output=True, text=True)
        return python, rust

    def test_orders_answer_the_same(self):
        answered = set()
        for case, orders in ORDERS.items():
            for order in orders:
                with self.subTest(case=case, order=order):
                    python, rust = self.run_both(order, self.homes[case])
                    self.assertEqual(rust.returncode, python.returncode, rust.stdout + rust.stderr)
                    self.assertEqual(json.loads(rust.stdout), json.loads(python.stdout))
                    if json.loads(rust.stdout)['ok'] and json.loads(rust.stdout)['data']:
                        answered.add(order[0])
        # Each order the other apps send answered with data at least once, not only with the same error.
        self.assertLessEqual({'projects', 'sources', 'volumes', 'resolve'}, answered)

    def test_usage_errors_exit_2_without_an_envelope(self):
        for order in USAGE:
            with self.subTest(order=order):
                python, rust = self.run_both(order)
                self.assertEqual((python.returncode, rust.returncode), (2, 2))
                self.assertEqual(rust.stdout, '')

    def test_state_dir_goes_before_or_after_media(self):
        before = subprocess.run([rust_engine(), '--state-dir', str(self.home), 'media', 'projects'], capture_output=True, text=True)
        after = subprocess.run([rust_engine(), 'media', '--state-dir', str(self.home), 'projects'], capture_output=True, text=True)
        self.assertEqual(json.loads(before.stdout), json.loads(after.stdout))
        self.assertTrue(json.loads(after.stdout)['data'])


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class ForwardTest(unittest.TestCase):
    """F5 (c): `nu9ve.py media` answers the read orders with Rust, and with Python when Rust cannot."""

    @classmethod
    def setUpClass(cls):
        MediaCliTest.setUpClass.__func__(cls)
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
        return subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', str(self.home), *order],
                              capture_output=True, text=True, cwd=REPO, env=dict(os.environ, **env))

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
