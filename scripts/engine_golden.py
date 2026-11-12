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


# Catalog reads (rpc-serve) on one fixture catalog: Python writes the catalog as SQL next to its answers, and the
# Rust engine loads the same SQL and must answer the same. Each method ported in phase L adds its requests here.
READ_REQUESTS = [
    ('project_preview', {'name': 'Terra 6 · ñandú!'}), ('project_preview', {'name': '  '}), ('project_preview', {}),
    ('closure_overview', {}),
    ('closure_link_candidates', {'project_id': 'p1'}), ('closure_link_candidates', {'project_id': 'p4'}),
    ('closure_link_candidates', {'project_id': 'p5'}), ('closure_link_candidates', {'project_id': 'nope'}),
    ('closure_progress', {'plan_id': 'x1'}), ('closure_progress', {'plan_id': 'absent'}),
    ('action_progress', {'id': 'a-1'}), ('action_progress', {'id': 'absent'}),
    ('asset_revisions', {'asset_id': 'a1'}), ('asset_revisions', {'asset_id': 'a2'}),
    ('asset_revisions', {'asset_id': 'nope'}),
    ('living_collections', {}),
    ('missions_status', {}), ('missions_status', {'flow_id': 'subaereo'}), ('missions_status', {'flow_id': 'nope'}),
    ('jobs', {}), ('library', {}), ('library', {'project': 'TERRA'}), ('projects', {}), ('profiles', {}),
    ('release_plans', {}), ('recipe', {}), ('devices', {}), ('defaults', {}), ('integration_settings', {}),
]


def reads_cases():
    from media.catalog import Catalog
    from media.common import MediaError
    from media.service import Service
    root = fresh_root('reads')
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            db = catalog.db
            with db:
                db.executemany('INSERT INTO projects VALUES(?,?,?)', [
                    ('p1', 'e64', 'terra-6-1'), ('p2', 'e64', 'Terra 6 1 extra'), ('p3', 'e64', 'otro_proyecto'),
                    ('p4', '9logs', 'vlog'), ('p5', 'e64', 'finales'), ('p6', 'e64', 'terra__6')])
                asset = lambda i, job, project, pid, rev: json.dumps(  # noqa: E731
                    {'id': i, 'job_id': job, 'kind': 'camera_clip', 'paths': ['CLIP/%s.MP4' % i], 'project': project,
                     'project_id': pid, 'profile': 'e64', 'revision': rev})
                db.executemany('INSERT INTO assets VALUES(?,?,?)', [
                    ('a1', 'j1', asset('a1', 'j1', 'terra-6-1', 'p1', 'r2')),
                    ('a2', 'j1', asset('a2', 'j1', 'otro_proyecto', 'p3', 'r1')),
                    ('a3', 'j2', asset('a3', 'j2', 'Terra 6 1 extra', 'p2', 'r1'))])
                db.executemany('INSERT INTO asset_revisions VALUES(?,?,?)', [
                    ('a1', 'r1', asset('a1', 'j1', 'terra-6-1', 'p1', 'r1')),
                    ('a1', 'r2', asset('a1', 'j1', 'terra-6-1', 'p1', 'r2'))])
                db.executemany('INSERT INTO work VALUES(?,?,?,?)', [
                    ('a1', 'ready', None, '2026-11-08'), ('a2', 'pending', None, '2026-11-08'),
                    ('a3', 'pending', None, '2026-11-09')])
                db.executemany('INSERT INTO asset_projects VALUES(?,?)', [('a1', 'p1'), ('a2', 'p3'), ('a3', 'p2')])
                db.executemany('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', [
                    ('j1', 'd1', json.dumps({'source_kind': 'card'}), 'completed', None, '2026-11-08T10:00:00Z',
                     '2026-11-08T11:00:00Z'),
                    ('j2', 'd2', json.dumps({'source_kind': 'card'}), 'completed', None, '2026-11-09T10:00:00Z',
                     '2026-11-09T11:00:00Z')])
                db.executemany('INSERT INTO living_files VALUES(?,?,?,?,?,?,?)', [
                    ('c1', 'a.wav', 10, 1, 'h', 'x/a.wav', 't'), ('c1', 'b.wav', 32, 1, 'h', 'x/b.wav', 't')])
            for key, value in {
                'project-closure:p1': {'required': ['horizontal'], 'no_more': True, 'published': False,
                                       'finals': [{'role': 'horizontal', 'volume_id': 'v', 'path': 'f.mp4'}],
                                       'related': ['p2', 'gone', 'p1']},
                'project-closure:p4': {'required': ['horizontal'], 'no_more': False, 'published': False, 'finals': []},
                'project-released:p4': {'at': '2026-11-01'},
                'closure-progress:x1': {'stage': 'hashing', 'files_done': 3},
                'action-progress:a-1': {'phase': 'copy', 'done': 1, 'total': 4},
                'living-collection:c2': {'id': 'c2', 'name': 'zeta', 'source': '/x/zeta'},
                'living-collection:c1': {'id': 'c1', 'name': 'alpha', 'source': '/x/alpha'},
                'mission-registry:subaereo': {'flow': 'subaereo', 'coverage': {'until': '2026-11-05'}},
                'release:r1': {'id': 'r1'},
                'creative-profiles': {'e64': {'id': 'e64', 'name': 'E64', 'clipper_profile': 'clip-e64'}},
            }.items():
                catalog.set(key, value)
            service = Service(catalog)
            cases = []
            for method, params in READ_REQUESTS:
                try:
                    answer = {'ok': True, 'data': service.call(method, dict(params))}
                except MediaError as error:
                    answer = {'ok': False, 'code': error.code}
                except (KeyError, TypeError, ValueError) as error:
                    answer = {'ok': False, 'code': 'operation_failed', 'error': type(error).__name__}
                cases.append({'method': method, 'params': params, 'answer': answer})
            sql = '\n'.join(catalog.db.iterdump())
        finally:
            catalog.close()
    return {'sql': sql, 'cases': cases}


# Stalker libraries (`capture_libraries`): bundles on disk under one root, a producer registry, the archived
# sessions table and a tag. Rust rebuilds the same files and compares with `<ROOT>` in place of the folder.
CAPTURE_SID = '123E4567E89B12D3A456426614174000'


def capture_cases():
    import os
    import time
    from media.captures import capture_libraries
    from media.catalog import Catalog
    os.environ['TZ'] = PLAN_TZ
    time.tzset()

    def manifest(**extra):
        return dict({'producer': 'stalker', 'schemaVersion': 2, 'status': 'completed'}, **extra)

    bundles = {
        'lib/S1.stalker': {'manifest.json': manifest(
            sessionID=CAPTURE_SID, createdAt='2026-11-04T23:32:12Z', duration={'value': 3000, 'timescale': 10},
            project={'version': 1, 'name': 'terra', 'country': 'mx'},
            tracks=[{'relativePath': 'cam.mov', 'kind': 'camera'}, {'relativePath': 'Screen/screen.mov'},
                    {'relativePath': 'mic.wav'}]),
            'cam.mov': 'c', 'Screen/screen.mov': 's', 'mic.wav': 'm'},
        'lib/S2.stalker': {'manifest.json': manifest(sessionID='223e4567-e89b-12d3-a456-426614174000', status='recording',
                                                     schemaVersion=3, createdAt='2026-11-05T08:00:00')},
        'lib/S3.STALKER': {'manifest.json': '{broken'},
        'lib/S4.stalker': {'manifest.json': dict(manifest(sessionID=CAPTURE_SID), producer='other')},
        'lib/S5.stalker': {'manifest.json': manifest(sessionID='323E4567E89B12D3A456426614174000',
                                                     createdAt='2026-11-01T10:00:00-07:00', tracks=[])},
        'lib/S6.stalker': {'notes.txt': 'no manifest'},
        'lib/S7.stalker': {'manifest.json': manifest(sessionID='423e4567e89b12d3a456426614174000', createdAt='ayer',
                                                     duration={'value': 5, 'timescale': 0},
                                                     tracks=[{'relativePath': 'gone.mp4', 'role': 'display'}])},
        'lib/folder': {'x.txt': 'x'},
        'lib2/S8.stalker': {'manifest.json': manifest(sessionID='523e4567e89b12d3a456426614174000',
                                                      createdAt='2026-11-06T12:00:00Z')},
    }
    registry = {'schema': 'stalker-library/1', 'path': '<ROOT>/lib2',
                'previous': [{'path': '<ROOT>/lib'}, 'relative/not-absolute', '<ROOT>/missing']}
    root = fresh_root('captures')
    files = {}
    for folder, content in bundles.items():
        for rel, value in content.items():
            files[folder + '/' + rel] = value if isinstance(value, str) else json.dumps(value)
    files['lib/x.stalker'] = 'a file, not a bundle'
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        os.utime(path, ns=(PLAN_MTIME_NS, PLAN_MTIME_NS))
    (root / 'registry.json').write_text(json.dumps(registry).replace('<ROOT>', str(root)))
    archived_manifest = json.loads(files['lib/S5.stalker/manifest.json'])
    # Python visits the bundles in set order, which changes per process: number every inode by path first.
    for path in sorted(root.rglob('*')):
        stable_inode(os.lstat(path))
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            with catalog.db:
                catalog.db.execute('INSERT INTO sessions VALUES(?,?,?)',
                                   ('323E4567-E89B-12D3-A456-426614174000', digest(archived_manifest), 'a1'))
            catalog.set('capture-tag:123e4567-e89b-12d3-a456-426614174000', {'name': 'terra', 'country': 'mx'})
            result = capture_libraries(catalog, '', registry=str(root / 'registry.json'), common=[])
            sql = '\n'.join(catalog.db.iterdump())
        finally:
            catalog.close()
    text = json.dumps(result, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    return {'files': files, 'registry': registry, 'sql': sql, 'mtime_ns': PLAN_MTIME_NS, 'result': json.loads(text)}


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
        # Catalog reads through the engine's own dispatch, as rpc-serve answers them.
        'reads': reads_cases(),
        # Stalker libraries read from bundles on disk.
        'captures': capture_cases(),
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
