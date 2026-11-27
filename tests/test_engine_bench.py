"""F6: the soak of `scripts/engine_bench.py` in the gate (nu9ve-bench-v1 §5.2).

A live `nuun-engine rpc-serve` answers catalog reads of the golden fixture while its memory is sampled; the run
must follow the contract and its `memory_slope_bytes_per_iter` must stay under the limit (4 096 B/iter), the
same check as `engine_bench.py soak --check-soak`. The run goes to a temporary log, never to `bench/`.
Skipped when the engine is not built (`cargo build -p nuun-engine`).
"""
import importlib.util
from pathlib import Path
import tempfile
import unittest

from tests.test_engine_examples import rust_engine

REPO = Path(__file__).resolve().parents[1]


def engine_bench():
    spec = importlib.util.spec_from_file_location('engine_bench', REPO / 'scripts' / 'engine_bench.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@unittest.skipUnless(rust_engine(), 'cargo build -p nuun-engine')
class SoakTest(unittest.TestCase):
    def test_catalog_reads_do_not_grow_the_engine(self):
        bench = engine_bench()
        with tempfile.TemporaryDirectory() as root:
            log = Path(root) / 'nuun.ndjson'
            subject = bench.B.subject('nuun', 'rust', bench.version(), REPO)
            run = bench.soak(log, Path(rust_engine()), 400, 4096.0, subject, bench.B.machine())
            self.assertIsNone(bench.B.run_problem(run))
            self.assertEqual(run['status'], 'ok', run.get('error'))
            slope = run['quality'][0]
            self.assertEqual(slope['metric'], 'memory_slope_bytes_per_iter')
            self.assertTrue(slope['pass'], slope)
            self.assertEqual(len(log.read_text(encoding='utf-8').splitlines()), 1)


if __name__ == '__main__':
    unittest.main()
