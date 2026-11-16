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

    def check_line_limit(self, argv):
        limit = 1024 * 1024
        request = b'{"id":7,"method":"engine_info"}'
        wire = (
            request + b' ' * (limit - len(request) - 1) + b'\n'  # exactly 1 MiB with LF
            + b' ' * (limit * 3) + b'\n'                          # oversized even though blank
            + b'{"id":"' + ('ñ' * (limit // 2)).encode() + b'","method":"engine_info"}\n'
            + b'{"id":"\xff","method":"engine_info"}\n'
            + request + b'\n'
            + b' ' * (limit + 1)                                   # oversized at EOF
        )
        done = subprocess.run(argv, input=wire, capture_output=True, check=True, timeout=120, cwd=REPO,
                              env=dict(os.environ, NUUN_SHADOW_ENGINE='off'))
        replies = [json.loads(line) for line in done.stdout.splitlines()]
        self.assertEqual([(r['id'], r.get('error', {}).get('code')) for r in replies],
                         [(7, None), (None, 'request_too_large'), (None, 'request_too_large'),
                          (None, 'invalid_request'), (7, None), (None, 'request_too_large')])

    def test_python_one_shot_envelope(self):
        cases = [
            (b'{"id":18446744073709551616,"method":"engine_info"}', 18446744073709551616, None),
            (b'{"method":"engine_info"}', None, None),
            (b'{"id":"broken method","method":9}', 'broken method', 'invalid_request'),
            (b'{"id":"bad params","method":"engine_info","params":[]}', 'bad params', 'invalid_params'),
            (b'{"id":"unknown","method":"absent"}', 'unknown', 'unknown_method'),
            (b'{"id":true,"method":"engine_info"}', None, 'invalid_request'),
            (b'{"id":7,', None, 'invalid_request'),
            (b' ' * (16 * 1024 * 1024 + 1), None, 'request_too_large'),
        ]
        with tempfile.TemporaryDirectory() as temp:
            argv = [sys.executable, '-S', 'nu9ve.py', 'media', '--state-dir', str(Path(temp).resolve() / 'absent'), 'rpc']
            for request, identifier, code in cases:
                done = subprocess.run(argv, input=request, capture_output=True, timeout=30, cwd=REPO)
                reply = json.loads(done.stdout)
                self.assertEqual((type(reply['id']), reply['id']), (type(identifier), identifier))
                self.assertEqual(reply['ok'], code is None)
                self.assertEqual(done.returncode, int(code is not None))
                if code is None:
                    self.assertEqual(reply['command'], 'engine_info')
                else:
                    self.assertEqual(reply['error']['code'], code)

    def test_python(self):
        with tempfile.TemporaryDirectory() as temp:
            argv = [sys.executable, '-S', 'nu9ve.py', 'media', '--state-dir', str(Path(temp).resolve() / 'absent'), 'rpc-serve']
            self.check(argv)
            self.check_line_limit(argv)

    @unittest.skipUnless(rust_engine(), 'Rust engine not built: cd engine && cargo build --offline')
    def test_rust(self):
        with tempfile.TemporaryDirectory() as temp:
            argv = [rust_engine(), '--state-dir', str(Path(temp).resolve() / 'absent'), 'rpc-serve']
            self.check(argv)
            self.check_line_limit(argv)


if __name__ == '__main__':
    unittest.main()
