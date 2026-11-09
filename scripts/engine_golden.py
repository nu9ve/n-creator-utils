#!/usr/bin/env python3
"""Phase 0 of the Rust engine (docs/platforms/rust-engine.md §7): answers of the Python oracle that the Rust
engine must reproduce byte for byte.

Pure: it reads no disk, card or catalog of the user; it only imports media/ and writes
tests/fixtures/engine-golden/recipe.json. Run from the checkout:

    python3 scripts/engine_golden.py
    cd engine && cargo test --offline -- --ignored

Regenerate only on purpose (a change in TEMPLATE or in canonical JSON) and review the diff.
"""
import json
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

from media.common import canonical, digest  # noqa: E402
from media.recipes import TEMPLATE, recipe_ref, summary  # noqa: E402
from media.volumes import card_hint  # noqa: E402

# Shapes the plan digest depends on: key order, escapes, non-ASCII and Python's float repr.
CANONICAL_CASES = [
    {'b': [1, 2.0, 'ñ'], 'a': {'z': None, 'y': True}, 'Á': 'x'},
    'a"b\\c\n\r\t\b\f\x01\x1f/\x7f' + chr(0x2028) + 'é',
    [1.0, 0.1, 0.6, -0.0, 0.0, 1e-07, 0.0001, 1e-05, 1e16, 1.5e16, 1234567890123456.0, 12345678901234567.0,
     0.12345678901234568, 1.5e300, -2.5e-310, 123.456, 100.0, 1e22],
    # tests/fixtures/native_plan_backend.py numeric_transport_fixture
    {'integral_float': 1.0, 'fraction': 0.12345678901234568, 'small': 1e-7, 'unsigned': 2 ** 64 - 1},
]

# Card trees (folder: has content) for the family detection, including the cases the content-first rule decides.
CARD_CASES = [
    {'S_1.stalker': True}, {'PRIVATE/M4ROOT/CLIP': True}, {'AVF_INFO': True}, {'ZOOM0001': True},
    {'DCIM/100GOPRO': True}, {'DCIM/Camera01': True, 'MISC': True}, {'DCIM/101MEDIA': True, 'MISC': True},
    {'DCIM/100CANON': True}, {'DCIM/100MEDIA': True}, {'DJI_001': True}, {'DCIM/DJI_001': False},
    {'DCIM/100GOPRO': False, 'DCIM/Camera01': True, 'MISC': True},
    {'DCIM/100CANON': False, 'DCIM/101MEDIA': True}, {'DCIM/100CANON': False}, {},
]


def card_cases():
    results = []
    for tree in CARD_CASES:
        with tempfile.TemporaryDirectory() as root:
            for folder, filled in tree.items():
                path = Path(root) / folder
                path.mkdir(parents=True)
                if filled:
                    (path / 'x').write_bytes(b'1')
            hint = card_hint(root)
            results.append({'tree': tree, 'family': hint['family'], 'recommendation': hint['recommendation']})
    return results


def main():
    out = {
        'generator': 'scripts/engine_golden.py',
        'python': sys.version.split()[0],
        'canonical': [{'value': value, 'canonical': canonical(value).decode('utf-8'), 'digest': digest(value)}
                      for value in CANONICAL_CASES],
        'recipe_ref': recipe_ref(TEMPLATE),
        # The `recipe` RPC with an empty catalog (no saved recipe, no confirmed devices).
        'recipe_summary': summary(None),
        # Family detection on temporary trees (volumes.py card_hint without a catalog).
        'cards': card_cases(),
    }
    target = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'recipe.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(out, ensure_ascii=False, indent=2, sort_keys=True) + '\n', encoding='utf-8')
    print(target)
    return 0


if __name__ == '__main__':
    sys.exit(main())
