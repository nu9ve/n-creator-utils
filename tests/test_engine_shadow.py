"""media/shadow.py: the Rust shadow compares reads after Python answers and never changes the answer."""
import io
import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path

from media.cli import serve_reads
from media.shadow import Shadow, compare, configured_engine, differences, summarize

EXAMPLES = Path(__file__).resolve().parents[1] / 'vendor' / 'nu9ve-core' / 'data' / 'engine-examples-v1.json'

# A stand-in for `nuun-engine rpc-serve`: answers `recipe` like Python, `jobs` differently, the rest not_ported.
FAKE_ENGINE = r'''#!%s
import json, sys
for line in sys.stdin:
    request = json.loads(line)
    if request['method'] == 'recipe':
        out = {'id': request['id'], 'ok': True, 'data': {'id': 'nu9ve-default'}}
    elif request['method'] == 'jobs':
        out = {'id': request['id'], 'ok': True, 'data': [{'id': 'j1', 'state': 'failed'}]}
    else:
        out = {'id': request['id'], 'ok': False, 'error': {'code': 'not_ported', 'message': 'x'}}
    sys.stdout.write(json.dumps(out) + '\n'); sys.stdout.flush()
''' % sys.executable


class FakeService:
    def call(self, method, params):
        if method == 'recipe':
            return {'id': 'nu9ve-default'}
        if method == 'jobs':
            return [{'id': 'j1', 'state': 'completed'}]
        return {}


class Recorder:
    def __init__(self, stdout):
        self.stdout, self.calls = stdout, []

    def submit(self, method, params, envelope, seconds):
        # The answer is already written when the shadow sees it.
        self.calls.append((method, envelope['ok'], self.stdout.getvalue().count('\n')))


class ShadowCompare(unittest.TestCase):
    def test_outcomes(self):
        ok = lambda data: {'ok': True, 'data': data}  # noqa: E731
        self.assertEqual(compare('m', ok({'a': 1}), ok({'a': 1}))['outcome'], 'same')
        # Strict in the type (nu9ve-engine-v1 §10): 1 is not 1.0 and true is not 1.
        self.assertEqual(compare('m', ok(1), ok(1.0))['diffs'], ['$'])
        self.assertEqual(compare('m', ok([True]), ok([1]))['diffs'], ['$[0]'])
        entry = compare('m', ok({'a': [1, 2]}), ok({'a': [1, 3], 'b': 0}))
        self.assertEqual(entry['outcome'], 'diff')
        self.assertEqual(entry['diffs'], ['$.a[1]', '$.b'])
        self.assertEqual(compare('m', ok({}), {'ok': False, 'error': {'code': 'not_ported'}})['outcome'], 'not_ported')
        failed = {'ok': False, 'error': {'code': 'job_busy'}}
        self.assertEqual(compare('m', failed, failed)['outcome'], 'same_error')
        self.assertEqual(compare('m', ok({}), failed)['outcome'], 'error_mismatch')
        partial = compare('m', ok({'a': 1}), ok({'a': 1, 'not_ported': ['mission']}))
        self.assertEqual((partial['outcome'], partial['rust_partial']), ('same', ['not_ported']))
        self.assertTrue(len(differences(list(range(50)), list(range(1, 51)))) <= 21)

    def test_pinned_shadow_examples(self):
        for case in json.loads(EXAMPLES.read_text(encoding='utf-8'))['shadow']:
            with self.subTest(case['name']):
                entry = compare('m', json.loads(case['oracle']), json.loads(case['candidate']), case['volatile'])
                self.assertEqual(entry['outcome'], case['outcome'])
                self.assertEqual(entry.get('diffs', []), case.get('diffs', []))
                self.assertEqual(entry.get('reason'), case.get('reason'))
                for key in ('oracle_error', 'candidate_error'):
                    if key in case:
                        self.assertEqual(entry[key], case[key])

    def test_the_log_keeps_paths_and_codes_never_values(self):
        secret = '/Volumes/MEDIA/Proyecto de Ana/clip.mov'
        entries = [
            compare('m', {'ok': True, 'data': {'title': secret}}, {'ok': True, 'data': {'title': secret + 'x'}}),
            compare('m', {'ok': True, 'data': {'title': secret}}, {'ok': True, 'data': {}}),
            compare('m', {'ok': False, 'error': {'code': 'project_not_found', 'message': secret}},
                    {'ok': False, 'error': {'code': 'internal_error', 'message': secret}}),
            compare('m', {'ok': True, 'data': 1}, {'ok': False, 'error': {'code': 'not_ported', 'reason': secret}}),
            compare('m', {'ok': True, 'data': 1}, {'ok': False, 'error': {'code': secret}}),
        ]
        self.assertEqual([e['outcome'] for e in entries], ['diff', 'diff', 'error_mismatch', 'not_ported', 'error_mismatch'])
        self.assertEqual(entries[4]['candidate_error'], 'invalid_code')
        for entry in entries:
            self.assertNotIn('Ana', json.dumps(entry, ensure_ascii=False))

    def test_volatile_values_are_blanked_by_method(self):
        first = {'ok': True, 'data': {'checked_at': '2026-11-16T10:00:00', 'groups': []}}
        second = {'ok': True, 'data': {'checked_at': '2026-11-16T10:00:01', 'groups': []}}
        self.assertEqual(compare('finals_status', first, second)['outcome'], 'same')
        self.assertEqual(compare('other', first, second)['diffs'], ['$.checked_at'])

    def test_volatile_keys_inside_list_items(self):
        def overview(free, used, uncounted, name='MEDIA502'):
            return {'ok': True, 'data': {'targets': [{'name': name, 'fs_free': free, 'fs_used': used,
                                                      'uncounted': uncounted, 'allocated': 10}]}}
        self.assertEqual(compare('storage_overview', overview(5, 20, 10), overview(4, 21, 11))['outcome'], 'same')
        # Only those values: anything else in the item still counts.
        self.assertEqual(compare('storage_overview', overview(5, 20, 10), overview(4, 21, 11, 'ESSD'))['diffs'],
                         ['$.targets[0].name'])
        # Whether the key is there is still compared.
        missing = overview(5, 20, 10)
        del missing['data']['targets'][0]['uncounted']
        self.assertEqual(compare('storage_overview', overview(5, 20, 10), missing)['outcome'], 'diff')

    def test_summary_counts_and_medians(self):
        rows = summarize([{'method': 'jobs', 'outcome': 'same', 'python_ms': 3, 'rust_ms': 1},
                          {'method': 'jobs', 'outcome': 'diff', 'python_ms': 5, 'rust_ms': 2},
                          {'method': 'x', 'outcome': 'not_ported'}])
        self.assertEqual(rows['jobs']['outcomes'], {'same': 1, 'diff': 1})
        self.assertEqual((rows['jobs']['python_ms_median'], rows['x']['rust_ms_median']), (5, None))

    def test_configuration_env_wins_and_off_disables(self):
        with tempfile.TemporaryDirectory() as home:
            (Path(home) / 'shadow.json').write_text(json.dumps({'engine': '/bin/engine'}))
            saved = os.environ.pop('NUUN_SHADOW_ENGINE', None)
            try:
                self.assertEqual(configured_engine(home), '/bin/engine')
                os.environ['NUUN_SHADOW_ENGINE'] = 'off'
                self.assertIsNone(configured_engine(home))
                os.environ['NUUN_SHADOW_ENGINE'] = '/other'
                self.assertEqual(configured_engine(home), '/other')
            finally:
                os.environ.pop('NUUN_SHADOW_ENGINE', None)
                if saved is not None:
                    os.environ['NUUN_SHADOW_ENGINE'] = saved
            (Path(home) / 'shadow.json').write_text(json.dumps({'engine': '/bin/engine', 'enabled': False}))
            os.environ.pop('NUUN_SHADOW_ENGINE', None)
            self.assertIsNone(configured_engine(home))


class ShadowServe(unittest.TestCase):
    def test_answer_first_and_refused_writes_are_not_shadowed(self):
        stdout = io.StringIO()
        recorder = Recorder(stdout)
        lines = [json.dumps({'id': 1, 'method': 'recipe'}), json.dumps({'id': 2, 'method': 'apply', 'params': {}})]
        serve_reads(FakeService(), stdin=io.StringIO('\n'.join(lines) + '\n'), stdout=stdout, shadow=recorder)
        self.assertEqual(recorder.calls, [('recipe', True, 1)])
        answers = [json.loads(line) for line in stdout.getvalue().splitlines()]
        self.assertEqual([a['ok'] for a in answers], [True, False])

    def test_shadow_with_an_engine_process_logs_comparisons(self):
        with tempfile.TemporaryDirectory() as home:
            engine = Path(home) / 'fake-engine'
            engine.write_text(FAKE_ENGINE)
            engine.chmod(0o755)
            shadow = Shadow(str(engine), home)
            stdout = io.StringIO()
            requests = ''.join(json.dumps({'id': i, 'method': m}) + '\n'
                               for i, m in enumerate(['recipe', 'jobs', 'library']))
            serve_reads(FakeService(), stdin=io.StringIO(requests), stdout=stdout, shadow=shadow)
            deadline = time.time() + 10
            log = None
            while time.time() < deadline:
                logs = list((Path(home) / 'shadow').glob('*.ndjson'))
                if logs and len(logs[0].read_text().splitlines()) == 3:
                    log = logs[0]
                    break
                time.sleep(0.05)
            shadow.close()
            self.assertIsNotNone(log, 'shadow log not written')
            entries = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertEqual([(e['method'], e['outcome']) for e in entries],
                             [('recipe', 'same'), ('jobs', 'diff'), ('library', 'not_ported')])
            self.assertEqual(entries[1]['diffs'], ['$[0].state'])
            self.assertNotIn('failed', log.read_text())
            self.assertTrue(all('python_ms' in e for e in entries))
            # Python's answers are untouched by the shadow.
            self.assertEqual(json.loads(stdout.getvalue().splitlines()[1])['data'][0]['state'], 'completed')

    def test_a_missing_engine_is_logged_not_raised(self):
        with tempfile.TemporaryDirectory() as home:
            shadow = Shadow(str(Path(home) / 'absent'), home)
            serve_reads(FakeService(), stdin=io.StringIO(json.dumps({'id': 1, 'method': 'recipe'}) + '\n'),
                        stdout=io.StringIO(), shadow=shadow)
            shadow.close(wait=5)
            entries = [json.loads(line) for p in (Path(home) / 'shadow').glob('*.ndjson')
                       for line in p.read_text().splitlines()]
            self.assertEqual([(e['outcome'], e['reason']) for e in entries], [('shadow_failed', 'engine_missing')])
            self.assertNotIn('message', entries[0])


if __name__ == '__main__':
    unittest.main()
