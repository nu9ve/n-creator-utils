"""nu9ve-engine-v1 §11: the engine cases of the pinned examples, through `rpc-serve` of both engines.

Python always runs. The Rust engine runs when it is built (`NUUN_ENGINE_BIN`, or engine/target/{debug,release});
`cargo test` runs the same cases in engine/nuun-engine/src/main.rs (`engine_examples_v1`), so CI covers both.
Each run uses a state folder that does not exist: the cases never reach the catalog.
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
EXAMPLES = REPO / 'vendor' / 'nu9ve-core' / 'data' / 'engine-examples-v1.json'


def rust_engine():
    configured = os.environ.get('NUUN_ENGINE_BIN')
    if configured:
        return configured if Path(configured).is_file() else None
    for profile in ('debug', 'release'):
        path = REPO / 'engine' / 'target' / profile / 'nuun-engine'
        if path.is_file():
            return str(path)
    return None


def exchanges():
    """(name, request line, expected response or None) in order, `then` right after its case."""
    cases = json.loads(EXAMPLES.read_text(encoding='utf-8'))['server']
    for case in cases:
        request = case['request']
        if 'pad_bytes' in case:
            request = request.replace('<pad>', 'x' * case['pad_bytes'])
        yield case['name'], request, case['response']
        if 'then' in case:
            yield case['name'] + ' (then)', case['then']['request'], case['then']['response']


def run(argv):
    """Every case to one `rpc-serve`, then a last `engine_info` that proves nothing extra was written."""
    lines = [request for _, request, _ in exchanges()] + ['{"id":"last","method":"engine_info"}']
    done = subprocess.run(argv, input=''.join(line + '\n' for line in lines).encode('utf-8'),
                          capture_output=True, timeout=120, cwd=str(REPO),
                          env=dict(os.environ, NUUN_SHADOW_ENGINE='off'))
    return done.stdout.decode('utf-8').splitlines()


class EngineExamples(unittest.TestCase):
    def check(self, argv):
        answers = iter(run(argv))
        passed = 0
        for name, _, expected in exchanges():
            if expected is None:
                continue  # a blank line has no answer
            with self.subTest(name):
                answer = json.loads(next(answers))
                expected_id = json.loads(expected['id'])
                # The same value and the same type: 7 is not "7", and a big integer keeps its digits.
                self.assertEqual((type(answer['id']), answer['id']), (type(expected_id), expected_id), answer)
                self.assertIs(answer['ok'], expected['ok'], answer)
                if not expected['ok']:
                    self.assertEqual(answer['error']['code'], expected['code'], answer)
                passed += 1
        last = json.loads(next(answers))
        self.assertEqual(last['id'], 'last')
        self.assertEqual(last['data']['contract'], 'nu9ve-engine/1')
        self.assertEqual(last['data']['protocol'], 'nuun-media/1')
        self.assertEqual(passed, 20)

    def test_python(self):
        with tempfile.TemporaryDirectory() as temp:
            self.check([sys.executable, '-S', 'nu9ve.py', 'media', '--state-dir', str(Path(temp).resolve() / 'absent'), 'rpc-serve'])

    @unittest.skipUnless(rust_engine(), 'Rust engine not built: cd engine && cargo build --offline')
    def test_rust(self):
        with tempfile.TemporaryDirectory() as temp:
            self.check([rust_engine(), '--state-dir', str(Path(temp).resolve() / 'absent'), 'rpc-serve'])


if __name__ == '__main__':
    unittest.main()
