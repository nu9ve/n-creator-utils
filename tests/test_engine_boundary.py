"""The Rust engine (engine/nuun-engine) must know the same read methods as media/cli.py.

A read Python adds and Rust does not list is refused by Rust's `rpc-serve` as a write (`read_only`) instead of
answering `not_ported`, so the app's shadow and a method-by-method client would treat it wrongly. Found
2026-11-11 with `living_collections` (M2) by scripts/engine_shadow.py on a real catalog.
"""
import re
import unittest
from pathlib import Path

from media.cli import READ_METHODS, READ_WHEN_EMPTY

MAIN = Path(__file__).resolve().parents[1] / 'engine' / 'nuun-engine' / 'src' / 'main.rs'


def rust_list(name):
    match = re.search(r'const %s: \[&str; (\d+)\] = \[(.*?)\];' % name, MAIN.read_text(encoding='utf-8'), re.S)
    assert match, name
    values = re.findall(r'"([^"]+)"', match.group(2))
    assert len(values) == int(match.group(1)), name
    return set(values)


class EngineBoundary(unittest.TestCase):
    def test_rust_lists_the_same_reads_as_python(self):
        self.assertEqual(rust_list('READ_METHODS'), set(READ_METHODS))
        self.assertEqual(rust_list('READ_WHEN_EMPTY'), set(READ_WHEN_EMPTY))


if __name__ == '__main__':
    unittest.main()
