#!/usr/bin/env python3
"""Phase 0 of the Rust engine (docs/platforms/rust-engine.md §7): answers of the Python oracle that the Rust
engine must reproduce byte for byte.

Pure: it reads no disk, card or catalog of the user; it only imports media/, builds cards under /tmp/nuun-engine-golden
and writes tests/fixtures/engine-golden/recipe.json (`--check` only compares). Run from the checkout:

    python3 scripts/engine_golden.py [--check]
    cd engine && TZ=America/Hermosillo TMPDIR="$(cd "$TMPDIR" && pwd -P)" cargo test --offline -- --ignored

(`TZ`: plan names use the host zone for files without a camera time; `TMPDIR` resolved because macOS puts
temporary folders under /var, a symlink the scan refuses.)

Regenerate only on purpose (a change in TEMPLATE or in canonical JSON) and review the diff.

Deterministic on any Mac, so the file lives in git and `--check` can tell whether Python's answers moved: cards are
written under one fixed folder (`GOLDEN_ROOT`), and inode, ctime, device and catalog ids come from counters, not from
the run. Plan and apply share the same patched `fingerprint`, so Python's own change checks still hold.
"""
import contextlib
import json
import shutil
import sys
import tempfile
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

from media.common import canonical, digest  # noqa: E402
from media.recipes import TEMPLATE, recipe_ref, summary  # noqa: E402
from media.volumes import card_hint  # noqa: E402

# Resolved, so on macOS it is /private/tmp/... (the scan refuses the /tmp symlink). Only this script writes there.
GOLDEN_ROOT = Path('/tmp/nuun-engine-golden')
_INODES = {}


def stable_inode(st):
    return _INODES.setdefault((st.st_dev, st.st_ino), 1000 + len(_INODES))


def fresh_root(name):
    """An empty, resolved folder for one case: the same path on every run and every Mac."""
    base = GOLDEN_ROOT.resolve()
    base.mkdir(exist_ok=True)
    root = base / name
    if root.exists():
        shutil.rmtree(root)
    root.mkdir()
    _INODES.clear()
    return root


@contextlib.contextmanager
def deterministic():
    """Stamps and catalog ids that do not depend on the run (counters instead of inode, ctime, device, uuid4)."""
    import media.catalog as catalog
    import media.common as common
    saved = common.fingerprint, catalog.uid
    real = common.fingerprint
    ids = iter(range(1, 1 << 30))

    def fingerprint(st):
        value = real(st)
        value.update(inode=stable_inode(st), ctime_ns=st.st_mtime_ns, device=1)
        return value

    common.fingerprint = fingerprint
    catalog.uid = lambda: str(uuid.UUID(int=next(ids)))
    try:
        yield
    finally:
        common.fingerprint, catalog.uid = saved

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


# Card trees (path: text) for `discover`: units, bytes, evidence, category and camera. No EXIF in these files, so
# neither engine adds a device or a camera time; `not_ported` (Rust only) is left out of the comparison.
SONY_XML = ('<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">'
            '<Device manufacturer="Sony" modelName="ILCE-6400"/></NonRealTimeMeta>')
DISCOVER_CASES = [
    {'PRIVATE/M4ROOT/CLIP/C0001.MP4': 'video', 'PRIVATE/M4ROOT/CLIP/C0001M01.XML': SONY_XML,
     'PRIVATE/M4ROOT/THMBNL/C0001T01.JPG': 't', 'PRIVATE/M4ROOT/MEDIAPRO.XML': '<a/>', '.DS_Store': 'x'},
    {'DCIM/100GOPRO/GX010001.MP4': 'v', 'DCIM/100GOPRO/GX010001.LRV': 'l', 'DCIM/100GOPRO/GX010001.THM': 't'},
    {'DCIM/100MEDIA/DJI_0001.JPG': 'j', 'DCIM/100MEDIA/DJI_0001.DNG': 'd', 'MISC/x.db': 'm'},
    {'DCIM/Camera01/VID_0001.insv': 'v', 'DCIM/Camera01/._VID_0001.insv': 'x'},
    {'CLIP/C0002.MP4': 'v', 'CLIP/C0002M01.XML': '<broken'},
    {'ZOOM0001/ZOOM0001_LR.WAV': 'a', 'ZOOM0001/ZOOM0001_Tr1.WAV': 'b', 'H6_SETTINGS/x.txt': 's'},
]


def discover_cases():
    from media.catalog import Catalog
    from media.service import Service
    results = []
    for number, tree in enumerate(DISCOVER_CASES):
        base = fresh_root('discover-%d' % number)
        root = str(base / 'card')
        Path(root).mkdir()
        for rel, text in tree.items():
            path = Path(root) / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        with deterministic():
            catalog = Catalog(base / 'state')
            try:
                found = Service(catalog).call('discover', {'source': root})
            finally:
                catalog.close()
            results.append({'tree': tree, 'discover': found})
    return results


# `plan` on temporary cards (a Python `FixtureVolumes` backend, as tests/test_media_engine.py). Names use the host zone
# for files without a camera time, so the zone is fixed here and the Rust test requires the same `TZ`.
PLAN_TZ = 'America/Hermosillo'
PLAN_MTIME_NS = 1778000000123456789
SONY_CLIP_XML = ('<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">'
                 '<Duration value="300"/><CreationDate value="2026-11-04T16:32:12-07:00"/>'
                 '<VideoFormat><VideoFrame captureFps="29.97p" formatFps="29.97p"/>'
                 '<VideoLayout pixel="3840" numOfVerticalLine="2160"/></VideoFormat>'
                 '<Device manufacturer="Sony" modelName="ILCE-6400"/></NonRealTimeMeta>').encode()


def pcm_wav(seconds, rate=48000, channels=2, width=3):
    import struct
    data = b'\x00' * (rate * channels * width * seconds)
    fmt = struct.pack('<HHIIHH', 1, channels, rate, rate * channels * width, channels * width, width * 8)
    body = b'WAVE' + b'fmt ' + struct.pack('<I', len(fmt)) + fmt + b'data' + struct.pack('<I', len(data)) + data
    return b'RIFF' + struct.pack('<I', len(body)) + body


def plan_case_specs():
    from tests.test_devices import dng, jpeg
    from tests.test_mission_match import LAT, LON
    drone = {'DCIM/101MEDIA/DJI_0001.JPG': jpeg(), 'DCIM/101MEDIA/DJI_0001.DNG': dng(), 'MISC/THM/101/DJI_0001.THM': b'thm'}
    record = {'captureID': 'C1', 'planID': 'P1', 'sessionID': 'S1', 'mediaCreatedAt': '2026-11-04 16:32:13',
              'capturedAt': '2026-11-04T23:32:13Z', 'imageJPEGReference': 'DJI_0001.JPG', 'latitude': LAT, 'longitude': LON}
    history = {'plans': [{'id': 'P1', 'name': 'Calles-45m-1104-1625'}], 'captureLog': [record],
               'nuun': {'sources': {'s': {'kind': 'all', 'exportedAt': '2026-11-05T05:00:00Z'}}}}
    base = {'archive': 'MEDIA', 'work': 'ESSD', 'project': 'intro'}
    return [
        ('drone_pending_export', drone, {}, base),
        ('drone_matched_mission', drone,
         {'004_datos/subaereo/coverage/history.json': json.dumps(history).encode()}, base),
        ('sony_clip', {'PRIVATE/M4ROOT/CLIP/C0001.MP4': b'video', 'PRIVATE/M4ROOT/CLIP/C0001M01.XML': SONY_CLIP_XML,
                       'PRIVATE/M4ROOT/THMBNL/C0001T01.JPG': b'thumb', 'PRIVATE/M4ROOT/MEDIAPRO.XML': b'<a/>'}, {}, base),
        ('gopro_by_mtime', {'DCIM/100GOPRO/GX010001.MP4': b'v', 'DCIM/100GOPRO/GX010001.LRV': b'l',
                            'DCIM/100GOPRO/GX010001.THM': b't'}, {}, base),
        # 8 kHz mono keeps the durations (2 s, 1 s) at a few KB in the fixture: the plan reads the header, not the audio.
        ('h6_wav', {'ZOOM0001/ZOOM0001_LR.WAV': pcm_wav(2, 8000, 1, 2),
                    'ZOOM0001/ZOOM0001_Tr1.WAV': pcm_wav(1, 8000, 1, 2)}, {}, base),
        ('archive_only_with_assignments', {'DCIM/100CANON/IMG_0001.JPG': b'x', 'DCIM/100CANON/IMG_0002.JPG': b'y'}, {},
         {'archive': 'MEDIA', 'work': 'ESSD', 'project': 'intro', 'archive_only': True, 'confirm_archive_only': True,
          'assignments': {'DCIM/100CANON/IMG_0002.JPG': {'discard': True},
                          'DCIM/100CANON/IMG_0001.JPG': {'country': 'pe', 'work': False}}}),
        ('dji_unknown_model_asks', {'DCIM/101MEDIA/DJI_0009.JPG': jpeg(model='FC9999', flight=False)}, {}, base),
    ]


def plan_cases():
    import os
    import time
    os.environ['TZ'] = PLAN_TZ
    time.tzset()
    from media.catalog import Catalog
    from media.common import MediaError
    from media.planner import build_plan, scan
    from tests.test_media_engine import FixtureVolumes

    class StableVolumes(FixtureVolumes):
        # The fixture backend reads the root's inode and device itself: the same counters as the stamps.
        def describe(self, root, external_required=False):
            info = super().describe(root, external_required)
            info.update(root_inode=stable_inode(Path(root).absolute().stat()), device=1)
            return info

    results = []
    for name, card_tree, archive_tree, params in plan_case_specs():
        with deterministic():
            root = fresh_root('plan-' + name)
            card, archive, work = (root / n for n in ('card', 'archive', 'work'))
            for folder in (card, archive, work):
                folder.mkdir()
            for rel, data in card_tree.items():
                path = card / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
                os.utime(path, ns=(PLAN_MTIME_NS, PLAN_MTIME_NS))
            for rel, data in archive_tree.items():
                path = archive / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            backend = StableVolumes()
            catalog = Catalog(root / 'state')
            try:
                a = catalog.register('MEDIA', 'archive', archive, backend.describe(archive), '001_raw')
                w = catalog.register('ESSD', 'work', work, backend.describe(work), '001_proyectos')
                info = backend.describe(card)
                files, _, _ = scan(card)
                applied = None
                try:
                    plan = build_plan(catalog, backend, source=str(card), **params)
                    # The same plan applied: final state and verified copies (no cleanup in these cases).
                    from media.transfer import Transfer
                    result = Transfer(catalog, backend).apply(plan, plan['digest'])
                    names = {a['id']: 'MEDIA', w['id']: 'ESSD'}
                    copies = sorted([names[row[0]], json.loads(row[1])['destination'], json.loads(row[1])['sha256']]
                                    for row in catalog.db.execute('SELECT volume_id,payload FROM copies'))
                    applied = {'state': result['state'], 'copies': copies}
                except MediaError as error:
                    plan = {'error': error.code}
            finally:
                catalog.close()
            results.append({
                'name': name, 'root': str(root), 'mtime_ns': PLAN_MTIME_NS, 'params': params,
                'card': {k: v.hex() for k, v in card_tree.items()}, 'archive': {k: v.hex() for k, v in archive_tree.items()},
                'work': {}, 'volume_ids': {'MEDIA': a['id'], 'ESSD': w['id']},
                'source_identity': {k: info[k] for k in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode')},
                'stamps': {f['path']: f['stamp'] for f in files}, 'plan': plan, 'apply': applied})
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
        # `discover` on temporary card trees (Service with a temporary catalog).
        'discover': discover_cases(),
        # `plan` on temporary cards (devices, missions, Sony, mtime names, WAV, archive-only, assignments, errors).
        'plan_tz': PLAN_TZ,
        'plans': plan_cases(),
    }
    target = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'recipe.json'
    text = json.dumps(out, ensure_ascii=False, indent=2, sort_keys=True) + '\n'
    if '--check' in sys.argv[1:]:
        # CI: Python's answers must still be the ones the Rust tests compare against.
        # Only the answers count: `python` records which interpreter wrote the file, and CI may run another.
        saved = json.loads(target.read_text(encoding='utf-8')) if target.is_file() else {}
        saved.pop('python', None)
        out.pop('python')
        if saved == json.loads(json.dumps(out, ensure_ascii=False, sort_keys=True)):
            print('engine golden: up to date')
            return 0
        print('engine golden: %s differs from Python; run scripts/engine_golden.py and review the diff' % target)
        return 1
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding='utf-8')
    print(target)
    return 0


if __name__ == '__main__':
    sys.exit(main())
