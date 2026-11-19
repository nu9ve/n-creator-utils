"""`storage_collections`: the agents' folders measured from metadata only, on a fixture home (never the real one).

Python (media/agent_storage.py) is the oracle; the Rust engine, when built, must answer the same JSON.
"""
import builtins
import io
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from media import agent_storage
from media.catalog import Catalog
from media.mcp import TOOLS
from media.service import Service
from tests.test_engine_examples import rust_engine

REPO = Path(__file__).resolve().parents[1]
DAY = 86400
SECRET = 'TOKEN=sk-fixture-never-printed'


def write(path, size, age_days=0, now=None, text=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes((text or '').encode() + b'x' * (size - len(text or '')))
    stamp = (now or time.time()) - age_days * DAY
    os.utime(path, (stamp, stamp))
    return path


def fixture(home, now):
    claude = home / '.claude'
    write(claude / 'projects' / '-Users-someone-secret-repo' / 'session-id-1234.jsonl', 5000, 0, now, SECRET)
    write(claude / 'projects' / '-Users-someone-secret-repo' / 'older.jsonl', 3000, 15, now)
    write(claude / 'file-history' / 'abc' / 'v1', 700, 15, now)
    os.link(claude / 'file-history' / 'abc' / 'v1', claude / 'file-history' / 'abc' / 'v1-link')  # counted once
    write(claude / 'history.jsonl', 300, 200, now)  # outside the growth window
    write(claude / 'shell-snapshots' / 'snapshot-zsh.sh', 400, 1, now)
    write(claude / 'debug' / 'trace.txt', 200, 1, now)
    write(claude / 'settings.json', 100, 30, now)
    write(claude / 'mystery' / 'thing.bin', 50, 1, now)  # unknown name: other, never cache
    outside = write(home / 'outside' / 'big.bin', 1_000_000, 1, now)
    (claude / 'projects' / 'link-to-big').symlink_to(outside)  # the link, never its target
    codex = home / '.codex'
    write(codex / 'sessions' / '2026' / '11' / '10' / 'rollout-private.jsonl.zst', 6000, 8, now)
    write(codex / 'archived_sessions' / 'rollout-old.jsonl.zst', 900, 60, now)
    write(codex / 'thread_history_1.sqlite', 4000, 0, now)
    write(codex / 'thread_history_1.sqlite-wal', 100, 0, now)
    write(codex / 'logs_2.sqlite', 2000, 0, now)
    write(codex / 'config.toml', 80, 90, now)
    write(codex / '..state.json.tmp-1784-0f1e2d3c-uuid-a', 30, 3, now)  # temporary files with ids: one part
    write(codex / '..state.json.tmp-1785-0f1e2d3c-uuid-b', 20, 3, now)
    app = home / 'Library' / 'Application Support' / 'Claude'
    write(app / 'Cache' / 'Cache_Data' / 'data_0', 8000, 2, now)
    write(app / 'Code Cache' / 'js' / 'index', 1500, 2, now)
    write(app / 'DawnGraphiteCache' / 'data_1', 500, 2, now)
    write(app / 'claude-code' / '2.1.0' / 'claude', 9000, 20, now)
    write(app / 'IndexedDB' / 'https_claude.ai_0.indexeddb.leveldb' / '000003.log', 1200, 1, now)
    write(app / 'config.json', 60, 1, now)


def agents(answer):
    return answer['collections'][0]


def root(answer, root_id):
    return next(item for item in agents(answer)['roots'] if item['id'] == root_id)


class AgentStorage(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.home = self.root / 'home'
        self.now = time.time()
        fixture(self.home, self.now)

    def tearDown(self):
        for path in self.home.rglob('*'):
            if path.is_dir() and not path.is_symlink():
                path.chmod(0o755)
        self.temp.cleanup()

    def measure(self):
        return agent_storage.collections(str(self.home), self.now)

    def test_classes_and_bytes_from_metadata(self):
        answer = self.measure()
        claude = root(answer, 'claude_code')
        self.assertEqual(claude['state'], 'ok')
        parts = {part['name']: part for part in claude['parts']}
        self.assertEqual(parts['file-history']['bytes'], 700)  # the hard link once
        self.assertEqual(parts['file-history']['files'], 1)
        self.assertLess(parts['projects']['bytes'], 1_000_000)  # the link, not the 1 MB target
        self.assertEqual(parts['projects']['files'], 3)
        self.assertEqual(parts['mystery']['class'], 'other')
        self.assertEqual(claude['classes']['history']['bytes'], parts['projects']['bytes'] + 700 + 300)
        self.assertEqual(claude['classes']['cache']['bytes'], 400)
        self.assertEqual(claude['classes']['logs']['bytes'], 200)
        self.assertEqual(claude['classes']['state']['bytes'], 100)
        codex = root(answer, 'codex')
        self.assertEqual(codex['classes']['history']['bytes'], 6000 + 900 + 4000 + 100)
        self.assertEqual(codex['classes']['logs']['bytes'], 2000)
        self.assertEqual(codex['classes']['state']['bytes'], 80)
        temporary = next(part for part in codex['parts'] if part['name'] == '*.tmp-*')
        self.assertEqual((temporary['class'], temporary['files'], temporary['bytes']), ('cache', 2, 50))
        app = root(answer, 'claude_app')
        self.assertEqual(app['classes']['cache']['bytes'], 8000 + 1500 + 500 + 9000)
        self.assertEqual(app['classes']['state']['bytes'], 1200 + 60)
        total = agents(answer)
        self.assertEqual(total['bytes'], sum(item['bytes'] for item in total['roots']))
        self.assertEqual(total['classes']['cache']['allocated'], total['release']['candidates'])
        self.assertEqual(total['release'], {'automatic': False, 'candidates': total['classes']['cache']['allocated'], 'needs': 'reviewed_plan'})

    def test_never_opens_a_file_nor_returns_deep_names_or_content(self):
        def refuse(*args, **kwargs):
            raise AssertionError('opened a file: %r' % (args,))
        with mock.patch.object(builtins, 'open', refuse), mock.patch.object(io, 'open', refuse), mock.patch.object(os, 'open', refuse):
            answer = self.measure()
        text = json.dumps(answer)
        for private in ('session-id-1234', 'secret-repo', 'rollout', 'sk-fixture', 'Cache_Data', 'snapshot-zsh', 'indexeddb.leveldb', 'uuid-a'):
            self.assertNotIn(private, text)
        self.assertNotIn(str(self.home), text)

    def test_weekly_growth_by_mtime(self):
        growth = agents(self.measure())['growth']
        self.assertEqual(growth['basis'], 'created' if hasattr(os.stat_result, 'st_birthtime') else 'mtime')
        self.assertEqual(len(growth['weeks']), agent_storage.WEEKS + 1)
        self.assertFalse(growth['weeks'][-1]['complete'])
        self.assertTrue(all(week['complete'] for week in growth['weeks'][:-1]))
        starts = [week['start'] for week in growth['weeks']]
        self.assertEqual(starts, sorted(starts))
        counted = sum(week['allocated'] for week in growth['weeks'])
        # history.jsonl (200 days), archived (60 days) and config.toml (90 days) are older than the window.
        self.assertGreater(counted, 0)
        self.assertLess(counted, agents(self.measure())['allocated'])
        two_weeks_ago = time.strftime('%Y-%m-%d', time.gmtime((agent_storage._week_start(int((self.now - 15 * DAY) // DAY))) * DAY))
        week = next(item for item in growth['weeks'] if item['start'] == two_weeks_ago)
        self.assertGreater(week['classes']['history'], 0)
        full = [item for item in growth['weeks'] if item['complete']][-4:]
        self.assertEqual(growth['weekly_average'], sum(item['allocated'] for item in full) // 4)

    def test_missing_and_unreadable(self):
        import shutil
        shutil.rmtree(self.home / '.codex')
        locked = self.home / '.claude' / 'projects' / '-Users-someone-secret-repo'
        locked.chmod(0)
        answer = self.measure()
        self.assertEqual(root(answer, 'codex')['state'], 'missing')
        self.assertEqual(root(answer, 'codex')['files'], 0)
        claude = root(answer, 'claude_code')
        if os.geteuid() != 0:
            self.assertEqual(claude['unreadable'], 1)
        self.assertEqual(claude['state'], 'ok')

    def test_service_and_mcp_expose_it_read_only(self):
        self.assertEqual(TOOLS['media_storage_collections'], ('storage_collections', {}, []))
        catalog = Catalog(self.root / 'state')
        with mock.patch.dict(os.environ, {'HOME': str(self.home)}):
            answer = Service(catalog).call('storage_collections')
        self.assertEqual(root(answer, 'codex')['classes']['history']['bytes'], 11000)
        with self.assertRaises(Exception) as raised:
            Service(catalog).call('storage_collections', {'home': '/'})
        self.assertEqual(getattr(raised.exception, 'code', None), 'invalid_params')

    @unittest.skipUnless(rust_engine(), 'Rust engine not built: cd engine && cargo build --offline')
    def test_rust_engine_answers_the_same(self):
        line = b'{"id":1,"method":"storage_collections"}\n'
        state = str(self.root / 'state')
        env = dict(os.environ, HOME=str(self.home), NUUN_SHADOW_ENGINE='off')
        python = subprocess.run([sys.executable, '-S', 'nu9ve.py', 'media', '--state-dir', state, 'rpc-serve'], input=line,
                                capture_output=True, timeout=60, cwd=REPO, env=env, check=True)
        rust = subprocess.run([rust_engine(), '--state-dir', state, 'rpc-serve'], input=line,
                              capture_output=True, timeout=60, cwd=REPO, env=env, check=True)
        expected, got = json.loads(python.stdout), json.loads(rust.stdout)
        self.assertTrue(expected['ok'], expected)
        self.assertTrue(got['ok'], got)
        for answer in (expected, got):
            agents(answer['data']).pop('measured_at')
        self.assertEqual(got['data'], expected['data'])


if __name__ == '__main__':
    unittest.main()
