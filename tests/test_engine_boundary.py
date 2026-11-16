"""The Rust engine (engine/nuun-engine) must know the same read methods as media/cli.py.

A read Python adds and Rust does not list is refused by Rust's `rpc-serve` as a write (`read_only`) instead of
answering `not_ported`, so the app's shadow and a method-by-method client would treat it wrongly. Found
2026-11-11 with `living_collections` (M2) by scripts/engine_shadow.py on a real catalog.
"""
import ast
import re
import unittest
from pathlib import Path

from media.cli import READ_METHODS, READ_WHEN_EMPTY, VERSION
from media.service import METHODS

REPO = Path(__file__).resolve().parents[1]
MAIN = REPO / 'engine' / 'nuun-engine' / 'src' / 'main.rs'


def dispatched(path):
    """Every literal a dispatcher compares `method` with (`method == 'x'`, `method in {...}`)."""
    names = set()
    for node in ast.walk(ast.parse(path.read_text(encoding='utf-8'))):
        if isinstance(node, ast.Compare) and isinstance(node.left, ast.Name) and node.left.id == 'method':
            for op, right in zip(node.ops, node.comparators):
                if isinstance(op, ast.Eq) and isinstance(right, ast.Constant):
                    names.add(right.value)
                elif isinstance(op, ast.In) and isinstance(right, (ast.Set, ast.Tuple, ast.List)):
                    names |= {item.value for item in right.elts}
    return names


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

    def test_methods_is_what_the_service_dispatches(self):
        # `unknown_method` is decided from METHODS before `read_only` (nu9ve-engine-v1 §7): it must be exact.
        self.assertEqual(dispatched(REPO / 'media' / 'service.py') | dispatched(REPO / 'media' / 'storage_index.py'),
                         set(METHODS))
        self.assertLessEqual(set(READ_METHODS) | set(READ_WHEN_EMPTY), set(METHODS))

    def test_rust_knows_the_oracle_catalog(self):
        self.assertEqual(rust_list('ORACLE_METHODS'), set(METHODS))
        self.assertFalse(rust_list('NATIVE') & set(METHODS))

    def test_engine_info_version_is_the_package_version(self):
        pyproject = (REPO / 'pyproject.toml').read_text(encoding='utf-8')
        self.assertIn('\nversion = "%s"\n' % VERSION, pyproject)


if __name__ == '__main__':
    unittest.main()
