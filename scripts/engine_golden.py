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
import hashlib
import json
import sqlite3
import shutil
import sys
import tempfile
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

from media.common import MediaError, canonical, digest  # noqa: E402
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


def sql_dump(db):
    # Python's sqlite3 moved sqlite_sequence to the end of iterdump in 3.12.
    # Keep its statements in alphabetical table order in both versions.
    statements = list(db.iterdump())
    sequence = [line for line in statements if line.startswith(('DELETE FROM "sqlite_sequence"', 'INSERT INTO "sqlite_sequence"'))]
    statements = [line for line in statements if line not in sequence]
    after = next((i for i, line in enumerate(statements)
                  if line.startswith('CREATE TABLE ') and line.split()[2].split('(')[0].strip('"') > 'sqlite_sequence'),
                 len(statements) - 1)
    return '\n'.join(statements[:after] + sequence + statements[after:])


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
        ('stalker_bundle', {
            'S.stalker/manifest.json': json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'status': 'completed',
                'sessionID': 'd9391e8f-0ddd-4f03-b745-b8bbd15832d9', 'createdAt': '2026-11-04T23:32:12Z',
                'duration': {'value': 123, 'timescale': 1}, 'tracks': [{'relativePath': 'screen.mov'}]}).encode(),
            'S.stalker/screen.mov': b'screen',
            'S.stalker/events.json': b'[]',
        }, {}, dict(base, source_kind='stalker')),
        ('finals_delivery', {'26-10-31-toma.MP4': b'final', 'shorts/vertical.MP4': b'v',
                            '26-10-31-existente.MP4': b'already'}, {'002_finales/e64/old/26-10-31-existente.MP4': b'already'},
         dict(base, source_kind='delivery', finals_route=True, work=None, archive_only=True,
              confirm_archive_only=True, work_decision='skipped')),
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
                if params.get('source_kind') == 'stalker':
                    catalog.set('retain-stalker-source', True)
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


def release_cases():
    from media.catalog import Catalog
    from media.release import apply_release, plan_release, review_release
    from tests.test_media_engine import FixtureVolumes
    from unittest.mock import patch
    root = fresh_root('release-plan')
    archive, work = (root / name for name in ('archive', 'work'))
    scope = '001_proyectos/intro'
    files = {'work/' + scope + '/intro.mov': 'identical',
             'work/' + scope + '/renders/cache.mov': 'protected',
             'work/' + scope + '/notes.txt': 'notes',
             'archive/old/renamed.mov': 'identical',
             'archive/old/impostor.mov': 'different'}
    for name, content in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
    backend = FixtureVolumes()
    initial = {'blockers': [], 'relinks': [], 'dependencies': None}
    refreshed = dict(initial, blockers=['fixture blocker'])
    with deterministic(), contextlib.closing(Catalog(root / 'state')) as catalog:
        for folder, name, role, prefix in ((archive, 'MEDIA', 'archive', '001_raw'), (work, 'ESSD', 'work', '001_proyectos')):
            catalog.register(name, role, folder, backend.describe(folder), prefix)
        with patch('media.clipper.inspect', return_value=initial):
            plan = plan_release(catalog, backend, archive='MEDIA', work='ESSD', path=scope, project='intro')
        with patch('media.clipper.inspect', return_value=refreshed):
            updated = review_release(catalog, backend, plan['id'], plan['digest'])
        assert catalog.get('release:' + plan['id']) == updated
        from contextlib import nullcontext
        with patch('media.clipper.release_guard', return_value=nullcontext()), patch('media.clipper.inspect', return_value=initial):
            applied = apply_release(catalog, backend, plan['id'], plan['digest'])
        remaining = sorted(str(path.relative_to(work)) for path in work.rglob('*') if path.is_file())
    def normalize(value):
        value = dict(value)
        for key in ('id', 'created_at', 'digest', 'metadata_backup'):
            value.pop(key, None)
        value['source_identity'].update(root_inode='<INODE>', device='<DEVICE>')
        for entry in value['entries']:
            entry['stamp'].update(inode='<INODE>', ctime_ns='<CTIME>', mtime_ns='<MTIME>')
        value['archive_id'] = 'MEDIA'
        value['work_id'] = 'ESSD'
        return json.loads(json.dumps(value).replace(str(root), '<ROOT>'))
    return {'files': files, 'plan': normalize(plan), 'review': normalize(updated),
            'applied': normalize(applied), 'remaining': remaining}


def closeout_cases():
    from media.catalog import Catalog
    from media.closure import apply_close, plan_close, save
    from tests.test_media_engine import FixtureVolumes
    from unittest.mock import patch
    root = fresh_root('closeout')
    archive, work = (root / name for name in ('archive', 'work'))
    published = '002_finales/general/intro/main.mov'
    unpublished = '002_finales/general/intro/preview.mov'
    files = {'work/' + published: 'published', 'work/' + unpublished: 'preview',
             'archive/' + published: 'published'}
    for name, content in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
    backend = FixtureVolumes()
    consumers = {'blockers': [], 'relinks': [], 'dependencies': None}
    with contextlib.closing(Catalog(root / 'state')) as catalog:
        for folder, name, role, prefix in ((archive, 'MEDIA', 'archive', '001_raw'), (work, 'ESSD', 'work', '001_proyectos')):
            catalog.register(name, role, folder, backend.describe(folder), prefix)
        with catalog.db:
            catalog.db.execute("INSERT INTO projects VALUES('p1','general','intro')")
        save(catalog, backend, 'p1', ['horizontal'], True, False, [
            {'volume_id': catalog.volume('ESSD')['id'], 'path': published, 'role': 'horizontal', 'published': True},
            {'volume_id': catalog.volume('ESSD')['id'], 'path': unpublished, 'role': 'auxiliary', 'published': False}])
        with patch('media.clipper.inspect', return_value=consumers), patch('media.clipper.release_guard', return_value=contextlib.nullcontext()):
            plan = plan_close(catalog, backend, 'p1', 'MEDIA', 'ESSD', release_work=False)
            applied = apply_close(catalog, backend, plan['id'], plan['digest'], confirm_discard=True)
        ids = {catalog.volume(name)['id']: name for name in ('MEDIA', 'ESSD')}
    def normalize(value):
        def visit(item):
            if isinstance(item, dict):
                return {k.replace(str(root), '<ROOT>'): ('<' + k.upper() + '>' if k in ('inode', 'ctime_ns', 'mtime_ns', 'root_inode', 'device')
                            else visit(v)) for k, v in item.items()
                        if k not in ('created_at', 'updated_at', 'closure_settings_digest', 'digest', 'metadata_backup')}
            if isinstance(item, list):
                return [visit(v) for v in item]
            if isinstance(item, str):
                return ids.get(item, item.replace(str(root), '<ROOT>'))
            return item
        return visit({k: v for k, v in value.items() if k != 'id'})
    return {'plan': normalize(plan), 'applied': normalize(applied)}


def transcript_cases():
    import hashlib
    from media import transcripts
    from media.catalog import Catalog
    from media.common import fingerprint
    from tests.test_media_engine import FixtureVolumes
    from unittest.mock import patch

    root = fresh_root('transcripts')
    archive, work = (root / name for name in ('archive', 'work'))
    archive.mkdir()
    editor = work / '001_proyectos/general/intro/clipper'
    path = editor / 'transcripts/clip.srt'
    path.parent.mkdir(parents=True)
    path.write_bytes(b'original transcript')
    (editor / 'clipper-retired-project.json').write_text('{}')
    backend = FixtureVolumes()
    member = 'DCIM/clip.mov'
    archived = '001_raw/video/2026/11/clip.mov'
    editable = '001_proyectos/general_raw/intro/clip.mov'
    with contextlib.closing(Catalog(root / 'state')) as catalog:
        media = catalog.register('MEDIA', 'archive', archive, backend.describe(archive), '001_raw')
        essd = catalog.register('ESSD', 'work', work, backend.describe(work), '001_proyectos')
        with catalog.db:
            catalog.db.execute("INSERT INTO projects VALUES('p1','general','intro')")
            catalog.db.execute("INSERT INTO jobs VALUES('job','digest','{}','completed',NULL,'t','t')")
            catalog.db.execute("INSERT INTO assets VALUES('asset','job',?)",
                               (json.dumps({'id': 'asset', 'job_id': 'job', 'paths': [member]}),))
            catalog.db.execute("INSERT INTO work VALUES('asset','ready',NULL,'t')")
            catalog.db.execute("INSERT INTO members(job_id,relative_path,sha256) VALUES('job',?,'member-sha')", (member,))
            for volume, destination in ((media, archived), (essd, editable)):
                row = json.dumps({'path': member, 'destination': destination})
                catalog.db.execute("INSERT INTO copies VALUES('job',?,?,'member-sha',?)", (volume['id'], member, row))
        def invoke(_, args, timeout):
            assert args == ['transcripts-export', str(editor)] and timeout == 600
            body = path.read_bytes()
            return {'schema': 'clipper-transcript-export/1', 'project_root': str(editor), 'digest': 'e' * 64,
                    'entries': [{'path': str(path), 'format': 'srt', 'bytes': len(body), 'stamp': fingerprint(path.stat()),
                                 'sha256': hashlib.sha256(body).hexdigest(), 'source': {'path': str(work / editable)}}]}
        def normalize(value, id_):
            return json.loads(json.dumps(value).replace(str(root), '<ROOT>').replace(media['id'], 'MEDIA').replace(id_, '<ID>'))
        cases = []
        with patch('media.clipper.invoke', side_effect=invoke):
            for body in (b'original transcript', b'another version', b'another version'):
                path.write_bytes(body)
                with patch('media.transcripts.now', return_value='2026-11-12T02:03:04Z'):
                    result = transcripts.project_backup(catalog, backend, 'p1', 'MEDIA')
                receipt = catalog.get('data-receipt:' + result['receipt_id'])
                result.pop('digest')
                cases.append({'result': normalize(result, result['receipt_id']),
                              'receipt': normalize(receipt, result['receipt_id'])})
        return cases


def handoff_cases():
    from media.catalog import Catalog
    from media.library import handoff
    from tests.test_media_engine import FixtureVolumes
    root = fresh_root('handoff')
    backend = FixtureVolumes()
    with deterministic(), contextlib.closing(Catalog(root / 'state')) as catalog:
        volumes = []
        for folder, name, role, prefix in (('archive', 'MEDIA', 'archive', '001_raw'), ('work', 'ESSD', 'work', '001_proyectos')):
            path = root / folder
            path.mkdir()
            identity = dict(backend.describe(path), root_inode=1, device=1)
            volumes.append(catalog.register(name, role, path, identity, prefix))
        catalog.set('creative-profiles', {'general': {'clipper_profile': 'editor'}})
        sets = [
            ('a1', 'p1', 'stalker_session', 'S1.stalker', ['S1.stalker/manifest.json', 'S1.stalker/screen.mov']),
            ('a2', 'p1', 'delivery', '.', ['extra.txt', 'main.mov']),
            ('a3', 'p2', 'file', '.', ['photo.jpg']),
            ('a4', 'p1', 'file', '.', ['conflict.mov']),
        ]
        for aid, pid, kind, bundle, paths in sets:
            job_id = 'job-' + aid
            plan = {'id': job_id, 'source_kind': 'delivery' if kind == 'delivery' else 'stalker',
                    'files': [{'path': path, 'stamp': {'size': index + 5}} for index, path in enumerate(paths)],
                    'layout': {path: {'archive': 'A/' + path, 'work': 'W/' + path} for path in paths}}
            asset = {'id': aid, 'project_id': pid, 'profile': 'general', 'kind': kind, 'bundle': bundle,
                     'paths': paths, 'job_id': job_id, 'revision': 'revision-' + aid}
            if kind == 'stalker_session':
                asset['session_id'] = '11111111-1111-4111-8111-111111111111'
            with catalog.db:
                catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',
                                   (job_id, 'digest', json.dumps(plan), 'completed', None, 't', 't'))
                catalog.db.execute('INSERT INTO assets VALUES(?,?,?)', (aid, job_id, json.dumps(asset)))
                for v in volumes:
                    for i, path in enumerate(paths):
                        if aid == 'a2' and v['role'] == 'work' and i == 0:
                            continue
                        prefix = v.get('finals_prefix', '002_finales') if kind == 'delivery' else v['prefix']
                        sha = 'conflict' if aid == 'a4' and v['role'] == 'work' else 'sha-' + str(i)
                        row = {'path': path, 'destination': prefix + '/' + plan['layout'][path][v['role']],
                               'volume_id': v['id'], 'sha256': sha, 'bytes': i + 5}
                        catalog.db.execute('INSERT INTO copies VALUES(?,?,?,?,?)',
                                           (job_id, v['id'], path, sha, json.dumps(row)))
        requests = [{'asset_ids': ['a2', 'a1']}, {'asset_ids': ['a1'], 'volume_id': volumes[0]['id']},
                    {'asset_ids': ['a2'], 'volume_id': volumes[1]['id']}, {'asset_ids': ['a1', 'a3']},
                    {'asset_ids': ['a4']}, {'asset_ids': ['a1', 'a1']}, {'asset_ids': ['missing']}]
        answers = []
        for params in requests:
            try:
                value = handoff(catalog, backend, **params)
                for key in ('handoff_id', 'created_at'):
                    value.pop(key)
                answers.append({'data': value})
            except MediaError as error:
                answers.append({'error': error.code})
        return {'sql': sql_dump(catalog.db).replace(str(root), '<ROOT>'), 'requests': requests, 'answers': answers}


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
    # One-shot reads (Python answers them through `rpc`, not `rpc-serve`).
    ('publications', {}), ('retitle_operations', {}), ('collection_release_plans', {}),
    ('resolve', {'asset_id': 'a1'}), ('resolve', {'asset_id': 'a1', 'revision': 'r1'}),
    ('resolve', {'asset_id': 'a1', 'revision': 'r9'}), ('resolve', {'asset_id': 'nope'}),
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
                db.executemany('INSERT INTO publications VALUES(?,?,?,?,?,?)', [
                    ('a1', 'horizontal', 'youtube', 'published', 'https://x', '2026-11-08'),
                    ('a2', 'vertical', 'tiktok', 'pending', None, '2026-11-09')])
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
                'retitle:op1': {'id': 'op1', 'state': 'applied', 'request': {'session_id': 's'}},
                'collection-release:c1': {'id': 'c1', 'source': '/x', 'entries': [1], 'kept': [], 'moved': [], 'bytes': 5},
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
            sql = sql_dump(catalog.db)
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
            from media.captures import captures
            changed = captures(catalog, str(root / 'lib'), changed_paths=[str(root / 'lib/S1.stalker/manifest.json'),
                                                                         str(root / 'elsewhere/x')])
            sql = sql_dump(catalog.db)
        finally:
            catalog.close()
    for value in changed['sessions'] + changed['pending']:
        value.pop('stamp', None)
    changed = {k: sorted(v, key=lambda e: e['path']) for k, v in changed.items()}
    text = json.dumps(result, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    changed_text = json.dumps(changed, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    return {'files': files, 'registry': registry, 'sql': sql.replace(str(root), '<ROOT>'), 'mtime_ns': PLAN_MTIME_NS,
            'result': json.loads(text),
            'changed': json.loads(changed_text)}


# Projects and closeout deliveries (`closure_deliveries`, the projects of `capture_overview`): a work disk with
# finals, an archive, a registered disk that is gone, receipts, aliases, sessions and job states.
def project_cases():
    import shutil
    from media.catalog import Catalog
    from media.closure import delivery_states
    from media.projects import overview
    from tests.test_media_engine import FixtureVolumes

    class StableVolumes(FixtureVolumes):
        def describe(self, root, external_required=False):
            info = super().describe(root, external_required)
            info.update(root_inode=stable_inode(Path(root).absolute().stat()), device=1)
            return info

    root = fresh_root('projects')
    files = {'work/002_finales/e64/terra/final-h.mp4': 'h', 'work/002_finales/e64/terra/final-v.mp4': 'v',
             'work/002_finales/e64/01_subidos/final-t.mp4': 't', 'archive/x/thumb.jpg': 'j',
             'work/002_finales/e64/01_subidos/26-11-04-terra.mp4': 'dated', 'work/002_finales/e64/shorts/terra-old.mp4': 's',
             'work/002_finales/e64/terra/Terra_Short-2.MOV': 'v2', 'work/002_finales/e64/terra/terra.thumbnail.jpg': 'j',
             'work/002_finales/e64/terra/notas.txt': 'n', 'work/002_finales/e64/otro/otro.mp4': 'o',
             'work/002_finales/e64/Proyecto.fcpbundle/x.mp4': 'b', 'work/002_finales/e64/.oculto/x.mp4': 'h',
             'archive/002_finales/e64/2026/terra.mp4': 'a', 'work/002_finales/e64/01_subidos/terra-short.mp4': 'sv',
             'work/002_finales/e64/otro/terra.mp4': 'a',
             'work/001_proyectos/e64_raw/terra/26NOV04-163212_SONY_05-09_MX-TERRA[].MP4': 'clip',
             'archive/001_raw/2026/11/26NOV04-163212_SONY_05-09_MX-TERRA[].MP4': 'clip',
             'work/001_proyectos/e64_raw/terra/S1.stalker/manifest.json': 'm',
             'archive/002_finales/e64/2026/entrega.mp4': 'ent'}
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    (root / 'gone').mkdir()
    backend = StableVolumes()
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            media = catalog.register('MEDIA', 'archive', root / 'archive', backend.describe(root / 'archive'), '001_raw')
            essd = catalog.register('ESSD', 'work', root / 'work', backend.describe(root / 'work'), '001_proyectos')
            old = catalog.register('OLD', 'work', root / 'gone', backend.describe(root / 'gone'), '001_proyectos')
            shutil.rmtree(root / 'gone')
            prefix = '002_finales/e64/'
            asset = lambda i, job, title, pid, **extra: json.dumps(dict(  # noqa: E731
                {'id': i, 'job_id': job, 'kind': 'camera_clip', 'paths': ['CLIP/%s.MP4' % i], 'project': title,
                 'project_id': pid, 'profile': 'e64'}, **extra))
            with catalog.db:
                db = catalog.db
                db.executemany('INSERT INTO projects VALUES(?,?,?)', [
                    ('p1', 'e64', 'terra'), ('p2', 'e64', 'terra-extra'), ('p3', 'e64', 'finales')])
                db.executemany('INSERT INTO project_aliases VALUES(?,?,?)', [
                    ('e64', 'terra', 'p1'), ('e64', 'terra-old', 'p1'), ('e64', 'terra-extra', 'p2')])
                db.executemany('INSERT INTO assets VALUES(?,?,?)', [
                    ('a1', 'j1', asset('a1', 'j1', 'terra', 'p1', camera='sony-a6400', country='mx', city='hermosillo')),
                    ('a2', 'j2', asset('a2', 'j2', 'terra', 'p1', camera='generic', country='pe')),
                    ('a3', 'j1', asset('a3', 'j1', 'terra-extra', 'p2', kind='stalker_session', bundle='S9.stalker')),
                    ('a4', 'j1', asset('a4', 'j1', 'borrador', '')),
                    ('a5', 'j1', asset('a5', 'j1', 'finales', 'p3'))])
                db.executemany('INSERT INTO work VALUES(?,?,?,?)', [
                    ('a1', 'ready', None, 't'), ('a2', 'pending', None, 't'), ('a3', 'skipped', None, 't'),
                    ('a4', 'ready', None, 't'), ('a5', 'ready', None, 't')])
                db.executemany('INSERT INTO asset_projects VALUES(?,?)', [
                    ('a1', 'p1'), ('a2', 'p1'), ('a3', 'p2'), ('a5', 'p3')])
                plan = {'source': str(root / 'work' / '002_finales' / 'e64'), 'source_kind': 'delivery',
                        'units': [{'profile': 'e64'}], 'layout': {}}
                db.executemany('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', [
                    ('j1', 'd', json.dumps({'source_kind': 'card'}), 'completed', None, 't', '2026-11-08T11:00:00Z'),
                    ('j2', 'd', json.dumps({'source_kind': 'card'}), 'failed', None, 't', '2026-11-09T09:00:00Z'),
                    ('j3', 'd', json.dumps(plan), 'completed', None, 't', '2026-11-07T09:00:00Z')])
                copy = {'path': '01_subidos/gone-r.mp4', 'volume_id': media['id'],
                        'destination': '002_finales/e64/2026/gone-r.mp4', 'stamp': {'size': 5}}
                db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', ('j3', media['id'], copy['path'], 'h', json.dumps(copy)))
                # Editor sources (`sources`): a file on both disks, a Stalker session missing a file on the work
                # disk, a bundle only on the disk that is gone, and a delivery.
                stamp = lambda size: {'size': size, 'mtime_ns': 1, 'ctime_ns': 1, 'inode': 1}  # noqa: E731
                files_plan = [{'path': 'CLIP/C1.MP4', 'stamp': stamp(4)}, {'path': 'S1.stalker/manifest.json', 'stamp': stamp(1)},
                              {'path': 'S1.stalker/screen.mov', 'stamp': stamp(3)}, {'path': 'DCIM/B1.JPG', 'stamp': stamp(2)},
                              {'path': 'entrega.mp4', 'stamp': stamp(3)}]
                db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',
                           ('j4', 'd', json.dumps({'files': files_plan}), 'completed', None, 't', '2026-11-06T00:00:00Z'))
                source = lambda i, kind, paths, **extra: json.dumps(dict(  # noqa: E731
                    {'id': i, 'job_id': 'j4', 'kind': kind, 'paths': paths, 'project': 'terra', 'project_id': 'p1',
                     'profile': 'e64', 'revision': 'r-' + i}, **extra))
                db.executemany('INSERT INTO assets VALUES(?,?,?)', [
                    ('s1', 'j4', source('s1', 'file', ['CLIP/C1.MP4'], camera='sony')),
                    ('s2', 'j4', source('s2', 'stalker_session', ['S1.stalker/manifest.json', 'S1.stalker/screen.mov'],
                                        bundle='S1.stalker', session_id='11111111-1111-1111-1111-111111111111')),
                    ('s3', 'j4', source('s3', 'camera_bundle', ['DCIM/B1.JPG'])),
                    ('s4', 'j4', source('s4', 'delivery', ['entrega.mp4']))])
                db.executemany('INSERT INTO work VALUES(?,?,?,?)', [(i, 'ready', None, 't') for i in ('s1', 's2', 's3', 's4')])
                db.executemany('INSERT INTO asset_projects VALUES(?,?)', [(i, 'p1') for i in ('s1', 's2', 's3', 's4')])
                copy_rows = [
                    (essd['id'], 'CLIP/C1.MP4', '001_proyectos/e64_raw/terra/26NOV04-163212_SONY_05-09_MX-TERRA[].MP4', 4, 'h1'),
                    (media['id'], 'CLIP/C1.MP4', '001_raw/2026/11/26NOV04-163212_SONY_05-09_MX-TERRA[].MP4', 4, 'h1'),
                    (essd['id'], 'S1.stalker/manifest.json', '001_proyectos/e64_raw/terra/S1.stalker/manifest.json', 1, 'hm'),
                    (essd['id'], 'S1.stalker/screen.mov', '001_proyectos/e64_raw/terra/S1.stalker/screen.mov', 3, 'hs'),
                    (old['id'], 'DCIM/B1.JPG', 'x/B1.JPG', 2, 'hb'),
                    (media['id'], 'entrega.mp4', '002_finales/e64/2026/entrega.mp4', 3, 'he')]
                for volume_id, path, destination, size, sha in copy_rows:
                    payload = {'path': path, 'volume_id': volume_id, 'destination': destination, 'bytes': size, 'sha256': sha}
                    db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', ('j4', volume_id, path, sha, json.dumps(payload)))
            catalog.set('project-closure:p1', {'required': ['horizontal'], 'no_more': True, 'published': False,
                                               'related': ['p2', 'gone'], 'finals': [
                {'role': 'horizontal', 'volume_id': essd['id'], 'path': prefix + 'terra/final-h.mp4', 'stamp': {'size': 0}},
                {'role': 'vertical', 'volume_id': essd['id'], 'path': prefix + 'terra/final-v.mp4', 'published': True,
                 'stamp': {'size': 0}},
                {'role': 'thumbnail', 'volume_id': media['id'], 'path': 'x/thumb.jpg'},
                {'role': 'auxiliary', 'volume_id': old['id'], 'path': 'a.mp4'},
                {'role': 'horizontal', 'volume_id': essd['id'], 'path': prefix + 'terra/final-t.mp4',
                 'stamp': {'size': 1, 'mtime_ns': 1, 'inode': 1}},
                {'role': 'vertical', 'volume_id': essd['id'], 'path': prefix + 'terra/gone-r.mp4', 'stamp': {'size': 5}}]})
            catalog.set('project-state:p2', {'state': 'finished'})
            catalog.set('project-touch:p1', '2026-11-10T00:00:00Z')
            catalog.set('project-released:p2', {'at': 't'})
            sessions = [
                {'tag': {'name': 'terra'}, 'status': 'completed', 'archived': False, 'created_at': '2026-11-09T10:00:00Z',
                 'path': '/x/S1.stalker', 'preview_path': 'screen.mov'},
                {'tag': {'name': 'nuevo'}, 'status': 'completed', 'archived': False, 'created_at': '2026-11-11T10:00:00Z',
                 'path': '/x/S2.stalker', 'preview_path': ''},
                {'tag': {'name': 'terra'}, 'status': 'recording', 'archived': False, 'path': '/x/S3.stalker'},
                {'tag': {'name': 'terra-old'}, 'status': 'completed', 'archived': True, 'path': '/x/S4.stalker'}]
            catalog.set('project-closure:p2', {'required': ['horizontal'], 'no_more': False, 'published': True,
                                               'related': ['p1'], 'finals': [
                {'role': 'horizontal', 'volume_id': essd['id'], 'path': prefix + 'otro/otro.mp4', 'stamp': {'size': 0}}]})
            from media.closure import candidates
            found = {}
            for name, args in (('p1', ('p1', None)), ('p1-query', ('p1', 'final')), ('p2', ('p2', None)),
                               ('p2-all', ('p2', ''))):
                found[name] = candidates(catalog, backend, *args)
            from media.library import sources
            editor = {}
            for name, args in (('edit', ('terra', 'edit')), ('archive', ('terra', 'archive')), ('nope', ('nope', 'edit')),
                               ('bad', ('terra', 'x'))):
                try:
                    editor[name] = {'ok': True, 'data': sources(catalog, backend, *args)}
                except MediaError as error:
                    editor[name] = {'ok': False, 'code': error.code}
            from media.finals import finals_status
            finals = finals_status(catalog, backend)
            finals.pop('checked_at')
            deliveries = delivery_states(catalog, backend, 'p1')
            projects = overview(catalog, sessions, backend)
            sql = sql_dump(catalog.db)
        finally:
            catalog.close()
    text = json.dumps({'deliveries': deliveries, 'projects': projects, 'candidates': found, 'finals': finals,
                       'sources': editor},
                      sort_keys=True).replace(str(root), '<ROOT>')
    return dict(json.loads(text), files=files, sessions=sessions, sql=sql.replace(str(root), '<ROOT>'))


# Mission exports (`missions_review`): an inbox with new, older, deleting, invalid and already incorporated
# exports, against a registry already on the archive; stamps are left out (they name this run's inodes).
def mission_cases():
    import copy as copy_module
    import os
    from media.catalog import Catalog
    from media.missions import empty_state, merge, render, review
    from media.recipes import TEMPLATE
    from tests.test_media_engine import FixtureVolumes

    class StableVolumes(FixtureVolumes):
        def describe(self, root, external_required=False):
            info = super().describe(root, external_required)
            info.update(root_inode=stable_inode(Path(root).absolute().stat()), device=1)
            return info

    root = fresh_root('missions')
    inbox, elsewhere, archive = root / 'inbox', root / 'elsewhere', root / 'archive'
    for folder in (inbox, elsewhere, archive):
        folder.mkdir()
    plan = lambda pid, when, name: {'id': pid, 'name': name, 'updatedAt': when}  # noqa: E731
    capture = lambda cid, at, **extra: dict({'captureID': cid, 'capturedAt': at}, **extra)  # noqa: E731
    exports = {
        'inbox/coverage-1.json': {'schemaVersion': 2, 'exportedAt': '2026-11-01T10:00:00Z', 'plans': [plan('P1', 't1', 'Calles')],
                                  'captureLog': [capture('C1', '2026-11-01T09:00:00Z', mediaCreatedAt='2026-11-01 02:00:00')]},
        'inbox/coverage-2.json': {'schemaVersion': 3, 'exportedAt': '2026-11-05T10:00:00Z', 'exportWindow': {'kind': 'since'},
                                  'plans': [plan('P1', 't2', 'Calles 2'), plan('P2', 't2', 'Río')],
                                  'captureLog': [capture('C1', '2026-11-01T09:00:00Z', mediaCreatedAt='2026-11-01 02:00:00'),
                                                 capture('C2', '2026-11-05T08:00:00+00:00')]},
        'inbox/JSON-old.json': {'schemaVersion': 1, 'exportedAt': '2026-10-01T10:00:00Z', 'plans': [plan('P1', 't0', 'Viejo')]},
        'inbox/coverage-3.json': {'schemaVersion': 3, 'exportedAt': '2026-11-06T10:00:00Z', 'plans': [],
                                  'deletedPlanIDs': ['P2', 'P9']},
        'inbox/coverage-bad.json': {'schemaVersion': 99, 'exportedAt': '2026-11-06T10:00:00Z', 'plans': []},
        'inbox/coverage-noplans.json': {'schemaVersion': 2, 'exportedAt': '2026-11-06T10:00:00Z'},
        'inbox/notes.json': {'schemaVersion': 2},
        'elsewhere/coverage-x.json': {'schemaVersion': 2, 'exportedAt': '2026-11-07T10:00:00+02:00',
                                      'plans': [plan('P3', 't3', 'Otro')], 'captures': [capture(7, '2026-11-07T07:00:00Z')]},
    }
    files = {}
    for rel, value in exports.items():
        text = json.dumps(value, sort_keys=True)
        files[rel] = text
        (root / rel).write_text(text)
    recipe = copy_module.deepcopy(TEMPLATE)
    recipe['missions'][0]['inbox'] = str(inbox)
    flow = recipe['missions'][0]
    first = (root / 'inbox/coverage-1.json').read_bytes()
    state = empty_state(flow)
    merge(state, json.loads(first), hashlib.sha256(first).hexdigest(), 'coverage-1.json', flow)
    for source in state['nuun']['sources'].values():
        source['incorporated_at'] = '2026-11-02T00:00:00Z'
    history = root / 'archive' / flow['archive_area'] / 'history.json'
    history.parent.mkdir(parents=True)
    history.write_bytes(render(state, flow))
    files['archive/' + flow['archive_area'] + '/history.json'] = history.read_text()
    backend = StableVolumes()
    results = {}
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            catalog.register('MEDIA', 'archive', archive, backend.describe(archive), '001_raw')
            catalog.set('recipe', recipe)
            for name, params in (('inbox', {'archive': 'MEDIA'}), ('no-archive', {}),
                                 ('paths', {'archive': 'MEDIA', 'paths': [str(root / 'elsewhere/coverage-x.json'),
                                                                          str(root / 'inbox/coverage-2.json')]}),
                                 ('unknown-flow', {'flow_id': 'nope'}), ('bad-paths', {'paths': 'x'})):
                try:
                    results[name] = {'ok': True, 'data': review(catalog, backend, **params)}
                except MediaError as error:
                    results[name] = {'ok': False, 'code': error.code}
            sql = sql_dump(catalog.db)
        finally:
            catalog.close()
    for result in results.values():
        for entry in (result.get('data') or {}).get('files', []):
            entry.pop('stamp', None)
    text = json.dumps(results, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    return {'files': files, 'sql': sql.replace(str(root), '<ROOT>'), 'results': json.loads(text)}


# The disk index (`storage_overview`, `storage_tree`, `storage_progress`): Python scans an archive and a work
# disk, then the run times, pids and this machine's free space are fixed so the answers do not depend on the run.
def storage_cases():
    import os
    from media.catalog import Catalog
    from media import storage_index
    from tests.test_media_engine import FixtureVolumes

    class StableVolumes(FixtureVolumes):
        def describe(self, root, external_required=False):
            info = super().describe(root, external_required)
            info.update(root_inode=stable_inode(Path(root).absolute().stat()), device=1)
            if Path(root).name == "observed":
                info["removable"] = False
            return info

    root = fresh_root('storage')
    files = {'archive/001_raw/raw/a.mov': 'A' * 4000, 'archive/001_raw/raw/b.mov': 'B' * 9000,
             'archive/002_finales/e64/f.mp4': 'F' * 100,
             'work/001_proyectos/e64_raw/terra/a.mov': 'A' * 4000, 'work/001_proyectos/e64_raw/terra/c.mov': 'C' * 9000,
             'work/001_proyectos/e64_raw/terra/d.mov': 'D' * 7000, 'work/001_proyectos/e64/terra.fcpbundle/x.plist': 'p',
             'work/node_modules/m.js': 'm' * 300, 'work/notes.txt': 'n', 'work/.hidden/h.mov': 'h'}
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        os.utime(path, ns=(PLAN_MTIME_NS, PLAN_MTIME_NS))
    backend = StableVolumes()
    results = {}
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            media = catalog.register('MEDIA', 'archive', root / 'archive', backend.describe(root / 'archive'), '001_raw')
            essd = catalog.register('ESSD', 'work', root / 'work', backend.describe(root / 'work'), '001_proyectos')
            with catalog.db:
                catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',
                                   ('j1', 'd', '{}', 'completed', None, 't', 't'))
                for volume, destination in ((media['id'], '001_raw/raw/a.mov'), (essd['id'], '001_proyectos/e64_raw/terra/a.mov')):
                    payload = {'destination': destination, 'path': 'CLIP/a.mov', 'bytes': 4000, 'verified_at': '2026-11-01'}
                    catalog.db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', ('j1', volume, 'CLIP/a.mov', 'SHA-A', json.dumps(payload)))
            for target in ('MEDIA', 'ESSD'):
                storage_index.scan(catalog, backend, target)
            index = sqlite3.connect(str(catalog.home / 'storage-index.sqlite3'))
            with index:
                index.execute("UPDATE runs SET pid=0,started_at='2026-11-02T00:00:00Z',finished_at='2026-11-02T00:01:00Z'")
                index.execute("UPDATE indexes SET indexed_at='2026-11-02T00:01:00Z',seconds=0.1236,fs_total=1000000,fs_used=600000,fs_free=400000")
                index.execute("INSERT INTO runs(target_id,kind,state,pid,started_at) VALUES('ESSD-x','scan','running',999999999,'t')")
            sql_index = sql_dump(index)
            index.close()
            call = lambda method, params: storage_index.call(catalog, backend, method, params)  # noqa: E731
            requests = [('storage_progress', {}), ('storage_tree', {'target': 'ESSD'}),
                        ('storage_tree', {'target': essd['id'], 'path': '001_proyectos/e64_raw/terra'}),
                        ('storage_tree', {'target': 'ESSD', 'path': '001_proyectos', 'limit': 1}),
                        ('storage_tree', {'target': 'ESSD', 'path': '001_proyectos/e64/terra.fcpbundle'}),
                        ('storage_tree', {'target': 'MEDIA', 'path': '001_raw/raw'}),
                        ('storage_tree', {'target': 'ESSD', 'path': 'nope'}), ('storage_tree', {'target': 'X'}),
                        ('storage_tree', {'target': 'ESSD', 'path': '../x'}), ('storage_tree', {'target': 'ESSD', 'limit': 5000}),
                        ('storage_overview', {})]
            for method, params in requests:
                try:
                    value = {'ok': True, 'data': call(method, dict(params))}
                except MediaError as error:
                    value = {'ok': False, 'code': error.code}
                results[json.dumps([method, params], sort_keys=True)] = value
            sql = sql_dump(catalog.db)
        finally:
            catalog.close()
    overview = results[json.dumps(['storage_overview', {}])]['data']
    # This Mac is a real disk, and online disks report this machine's free space: both out of the comparison.
    overview['targets'] = [t for t in overview['targets'] if t['id'] != 'this-mac']
    for target in overview['targets']:
        for key in ('fs_total', 'fs_used', 'fs_free', 'uncounted'):
            target.pop(key, None)
    text = json.dumps(results, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    from media.coverage import coverage
    from unittest.mock import patch

    reports = []
    with contextlib.closing(Catalog(root / 'state')) as catalog, patch('media.storage_index.SYSTEM_ROOTS', ('/System', '/Library')):
        observed_root = root / 'observed'
        observed_root.mkdir()
        files['observed/keep.wav'] = 'keep'
        (observed_root / 'keep.wav').write_text(files['observed/keep.wav'])
        selected = storage_index.add_folder(catalog, backend, observed_root, 'Observada')
        storage_index.scan(catalog, backend, selected['id'])
        index = sqlite3.connect(str(catalog.home / 'storage-index.sqlite3'))
        indexed_target = index.execute('SELECT target_id FROM indexes WHERE target_id=?', (selected['id'],)).fetchone()[0]
        index.close()
        assert indexed_target == selected['id']
        removed = storage_index.remove_folder(catalog, 'Observada')
        for scoped, hashed in (('', True), ('001_proyectos/e64_raw/terra', False)):
            reply = coverage(catalog, backend, 'ESSD', 'MEDIA', scoped, hashed)
            report = json.loads(Path(reply.pop('report')).read_text())
            report.pop('created_at')
            reports.append({'path': scoped, 'no_hash': not hashed, 'reply': reply, 'report': report})
        assert (observed_root / 'keep.wav').read_text() == 'keep'
    selected['identity']['root_inode'] = '<INODE>'
    selected['id'] = '<ID>'
    removed['removed'] = '<ID>'
    writes = json.loads(json.dumps({'added': selected, 'removed': removed, 'coverage': reports}).replace(str(root), '<ROOT>'))
    return {'files': files, 'sql': sql.replace(str(root), '<ROOT>'), 'index_sql': sql_index.replace(str(root), '<ROOT>'),
            'results': json.loads(text), 'writes': writes}



# Clipper guards (`clipper.inspect`) and `capture_discard_check`: a fake Clipper whose report uses one session, a
# legacy project it does not report, a linked-projects registry with a missing root and an export queue, all under
# a fixture HOME and CLIPPER_PROJECTS_ROOT.
def clipper_cases():
    import os
    from media import clipper
    from media.captures import discard_check
    from media.catalog import Catalog
    root = fresh_root('clipper')
    stalker = lambda status, sid: json.dumps({'producer': 'stalker', 'schemaVersion': 2, 'status': status,  # noqa: E731
                                               'sessionID': sid, 'createdAt': '2026-11-04T23:32:12Z'})
    deps = {'schema': 'clipper-library-deps/1', 'capabilities': ['media-release-lock/1'], 'offline_projects': [],
            'projects': [{'clipper_root': '<ROOT>/clipper-projects/current',
                          'assets': [{'path': '<ROOT>/home/Movies/Stalker/S1.stalker', 'in_use': True, 'queued_exports': [],
                                      'items': [], 'asset_id': 'A1', 'revision': 'r1', 'kind': 'stalker_session'}],
                          'relinks': [{'from': '<ROOT>/home/Movies/Stalker/S1.stalker', 'still_pinned': ['q1']}]}]}
    files = {
        'home/Movies/Stalker/S1.stalker/manifest.json': stalker('completed', '11111111-1111-1111-1111-111111111111'),
        'home/Movies/Stalker/S1.stalker/screen.mov': 'v',
        'home/Movies/Stalker/S2.stalker/manifest.json': stalker('recording', '22222222-2222-2222-2222-222222222222'),
        'home/Movies/Stalker/S4.stalker/manifest.json': stalker('completed', '44444444-4444-4444-4444-444444444444'),
        'home/Movies/Stalker/S5.stalker/manifest.json': stalker('completed', '55555555-5555-5555-5555-555555555555'),
        'home/Movies/Stalker/S5.stalker/pistas/mic.wav': 'aa',
        'home/Downloads/old/clipper-project.json': json.dumps({'items': [{'source': '<ROOT>/home/Movies/Stalker/S4.stalker/screen.mov'}],
                                                               'notes': '<ROOT>/home/Movies/Stalker/S5.stalker'}),
        'clipper-projects/current/clipper-project.json': json.dumps({'items': []}),
        'clipper-projects/linked/clipper-project.json': json.dumps({'items': ['/elsewhere/x.mov']}),
        'clipper-projects/.linked-projects.json': json.dumps({'schema': 'clipper-linked-projects/1',
                                                              'roots': ['<ROOT>/clipper-projects/linked', '<ROOT>/missing']}),
        'clipper-projects/.export-queue/queue.json': json.dumps({'schema_version': 1, 'jobs': [
            {'id': 'q1', 'status': 'queued', 'sources': {'<ROOT>/home/Movies/Stalker/S1.stalker/screen.mov': {}}},
            {'id': 'q2', 'status': 'completed', 'sources': {'<ROOT>/home/Movies/Stalker/S5.stalker': {}}}]}),
        'bin/clipper': '#!/bin/sh\ncat <<\'X\'\n' + json.dumps(deps) + '\nX\n',
    }
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text.replace('<ROOT>', str(root)))
    (root / 'bin/clipper').chmod(0o755)
    # The capture index keeps stamps: fixed times, and inodes numbered by path, not by Python's set order.
    for path in sorted(root.rglob('*')):
        if path.is_file():
            os.utime(path, ns=(PLAN_MTIME_NS, PLAN_MTIME_NS))
    for path in sorted(root.rglob('*')):
        stable_inode(os.lstat(path))
    saved = {k: os.environ.get(k) for k in ('HOME', 'CLIPPER_PROJECTS_ROOT')}
    os.environ['HOME'], os.environ['CLIPPER_PROJECTS_ROOT'] = str(root / 'home'), str(root / 'clipper-projects')
    results = {}
    stalker_root = str(root / 'home/Movies/Stalker')
    try:
        with deterministic():
            catalog = Catalog(root / 'state')
            try:
                catalog.set('clipper-executable', str(root / 'bin/clipper'))
                for name, paths in (('S1', {stalker_root + '/S1.stalker': None}),
                                    ('S1-relink', {stalker_root + '/S1.stalker': str(root / 'new/S1.stalker')}),
                                    ('S4', {stalker_root + '/S4.stalker': None}), ('S5', {stalker_root + '/S5.stalker': None})):
                    try:
                        results['inspect:' + name] = {'ok': True, 'data': clipper.inspect(catalog, paths)}
                    except MediaError as error:
                        results['inspect:' + name] = {'ok': False, 'code': error.code}
                for name in ('S1', 'S2', 'S4', 'S5', 'nope'):
                    try:
                        results['discard:' + name] = {'ok': True, 'data': discard_check(catalog, stalker_root + '/' + name + '.stalker')}
                    except MediaError as error:
                        results['discard:' + name] = {'ok': False, 'code': error.code}
                sql = sql_dump(catalog.db)
            finally:
                catalog.close()
    finally:
        for key, value in saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    text = json.dumps(results, ensure_ascii=False, sort_keys=True).replace(str(root), '<ROOT>')
    return {'files': files, 'sql': sql.replace(str(root), '<ROOT>'), 'results': json.loads(text)}


# E1 · catalog writes: the same base catalog, the same sequence of writes (valid and refused), the same answers and
# the same final catalog, table by table. Times are `<NOW>`.
U1, U2 = '123E4567E89B12D3A456426614174000', '223e4567-e89b-12d3-a456-426614174000'
WRITE_REQUESTS = [
    ('defaults', {'project': 'terra-6-1', 'city': 'cdmx'}), ('defaults', {'bad': 1}),
    ('integration_settings', {'capture_override': '/x', 'retain_stalker_source': True}),
    ('integration_settings', {'retain_stalker_source': 'yes'}),
    ('dismiss_job', {'job_id': 'j1', 'dismissed': True}), ('dismiss_job', {'job_id': 'nope', 'dismissed': True}),
    ('dismiss_job', {'job_id': 'j1', 'dismissed': 'x'}),
    ('touch_project', {'key': 'p1'}), ('touch_project', {'key': ''}),
    ('set_project_state', {'project_id': 'p1', 'state': 'paused'}), ('set_project_state', {'project_id': 'p1', 'state': 'x'}),
    ('set_project_state', {'project_id': 'nope', 'state': 'active'}),
    ('save_capture_tags', {'session_ids': [U1, U2], 'project': 'Terra Nueva', 'country': 'mx'}),
    ('save_capture_tag', {'session_id': 'bad', 'project': 'x'}), ('save_capture_tags', {'session_ids': [], 'project': 'x'}),
    ('rename_project', {'key': 'p1', 'title': 'Terra Final'}), ('rename_project', {'project_id': 'p2', 'title': 'terra-6-1'}),
    ('rename_project', {'key': 'draft:Terra-Nueva', 'title': 'nueva2'}), ('rename_project', {'key': 'nope', 'title': 'x'}),
    ('set_work_state', {'asset_ids': ['a1', 'a2'], 'state': 'skipped'}),
    ('set_work_state', {'asset_ids': ['nope'], 'state': 'pending'}), ('set_work_state', {'asset_ids': ['a1'], 'state': 'x'}),
    ('set_publication', {'asset_id': 'd1', 'variant': 'v.mp4', 'channel': 'youtube', 'state': 'published', 'url': 'https://y'}),
    ('set_publication', {'asset_id': 'd1', 'variant': 'v.mp4', 'channel': 'youtube', 'state': 'published', 'url': 'ftp://x'}),
    ('set_publication', {'asset_id': 'd1', 'variant': 'z.mp4', 'channel': 'youtube', 'state': 'candidate'}),
    ('set_publication', {'asset_id': 'a1', 'variant': 'CLIP/a1.MP4', 'channel': 'youtube', 'state': 'candidate'}),
    ('set_finals_folder', {'profile': 'e64', 'folder': '01_subidos'}), ('set_finals_folder', {'profile': '9logs', 'folder': None}),
    ('set_finals_folder', {'profile': '../x', 'folder': ''}),
    ('cancel_plan', {'plan_id': 'rp1'}), ('cancel_plan', {'plan_id': 'j1'}), ('cancel_plan', {'plan_id': 'nope'}),
    ('capture_discarded', {'session_id': U1, 'name': 'S.stalker', 'trash_path': '/T/S', 'bytes': 5}),
    ('closure_save', {'project_id': 'p1', 'required': ['vertical', 'horizontal', 'vertical'], 'no_more': True,
                      'published': True, 'finals': [], 'profile_default': True}),
    ('closure_save', {'project_id': 'p1', 'required': ['auxiliary'], 'no_more': False, 'published': False, 'finals': []}),
    ('closure_link', {'project_id': 'p1', 'other_id': 'p3'}), ('closure_link', {'project_id': 'p1', 'other_id': 'p3', 'linked': False}),
    ('closure_link', {'project_id': 'p1', 'other_id': 'p4'}), ('closure_link', {'project_id': 'p1', 'other_id': 'p1'}),
    ('save_profile', {'id': 'e64', 'name': ' E64 nuevo ', 'raw_dir': 'e64_raw', 'work': 'ESSD', 'archive': 'MEDIA'}),
    ('save_profile', {'id': 'E 64', 'name': 'x', 'raw_dir': 'r'}), ('save_profile', {'id': 'x', 'name': 'x', 'raw_dir': 'r', 'work': 'MEDIA'}),
    ('confirm_device', {'make': 'DJI', 'model': 'FC3170', 'serial': 'X1', 'category': 'dji-drone', 'label': 'Mavic'}),
    ('confirm_device', {'make': 'DJI', 'model': 'FC3170', 'serial': 'X1', 'category': 'x'}),
    ('confirm_device', {'make': 'DJI', 'model': 'FC3170', 'serial': ' ', 'category': 'dji-drone'}),
]
WRITE_TABLES = {'metadata': 'key', 'projects': 'id', 'project_aliases': 'profile,alias', 'assets': 'id', 'work': 'asset_id',
                'publications': 'asset_id,variant,channel', 'events': 'seq'}
NOW_PATTERN = r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|\+00:00)'


def write_cases():
    import re
    from media.catalog import Catalog
    from media.service import Service
    root = fresh_root('writes')
    with deterministic():
        catalog = Catalog(root / 'state')
        try:
            db = catalog.db
            identity = {'mode': 'fixture', 'volume_uuid': 'v', 'disk_uuid': 'd', 'root_inode': 1, 'external': True}
            catalog.register('MEDIA', 'archive', root / 'archive', identity, '001_raw')
            catalog.register('ESSD', 'work', root / 'work', dict(identity, volume_uuid='w'), '001_proyectos')
            asset = lambda i, job, title, pid, **extra: json.dumps(dict(  # noqa: E731
                {'id': i, 'job_id': job, 'kind': 'camera_clip', 'paths': ['CLIP/%s.MP4' % i], 'project': title,
                 'project_id': pid, 'profile': 'e64', 'revision': 'r1'}, **extra))
            with db:
                db.executemany('INSERT INTO projects VALUES(?,?,?)', [
                    ('p1', 'e64', 'terra-6-1'), ('p2', 'e64', 'Terra 6 1 extra'), ('p3', 'e64', 'otro'), ('p4', '9logs', 'vlog')])
                db.executemany('INSERT INTO project_aliases VALUES(?,?,?)', [
                    ('e64', 'terra-6-1', 'p1'), ('e64', 'Terra 6 1 extra', 'p2'), ('e64', 'otro', 'p3'), ('9logs', 'vlog', 'p4')])
                db.executemany('INSERT INTO assets VALUES(?,?,?)', [
                    ('a1', 'j1', asset('a1', 'j1', 'terra-6-1', 'p1')), ('a2', 'j1', asset('a2', 'j1', 'Terra 6 1 extra', 'p2')),
                    ('d1', 'j2', asset('d1', 'j2', 'terra-6-1', 'p1', kind='delivery', paths=['v.mp4', 'h.mp4']))])
                db.executemany('INSERT INTO work VALUES(?,?,?,?)', [(i, 'ready', None, 't') for i in ('a1', 'a2', 'd1')])
                db.executemany('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', [
                    ('j1', 'd', '{"files":[]}', 'completed', None, 't', 't'), ('j2', 'd', '{"files":[]}', 'completed', None, 't', 't')])
            for key, value in {'reviewed-plan:rp1': {'id': 'rp1'}, 'reviewed-plan:j1': {'id': 'j1'},
                               'capture-tag:' + U2: {'name': 'terra-6-1', 'country': 'mx'},
                               'creative-profiles': {'e64': {'id': 'e64', 'name': 'E64', 'clipper_profile': 'clip-e64'}}}.items():
                catalog.set(key, value)
            base = sql_dump(db)
            service, answers = Service(catalog), []
            for method, params in WRITE_REQUESTS:
                try:
                    answers.append({'ok': True, 'data': service.call(method, dict(params))})
                except MediaError as error:
                    answers.append({'ok': False, 'code': error.code})
                except (KeyError, TypeError, ValueError):
                    answers.append({'ok': False, 'code': 'operation_failed'})
            tables = {name: [list(row) for row in db.execute('SELECT * FROM %s ORDER BY %s' % (name, order))]
                      for name, order in WRITE_TABLES.items()}
        finally:
            catalog.close()
    text = re.sub(NOW_PATTERN, '<NOW>', json.dumps({'answers': answers, 'tables': tables}, ensure_ascii=False, sort_keys=True))
    return dict(json.loads(text.replace(str(root), '<ROOT>')), sql=base.replace(str(root), '<ROOT>'),
                requests=[[m, p] for m, p in WRITE_REQUESTS], now=NOW_PATTERN, tables_order=WRITE_TABLES)


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
        'release': release_cases(),
        'closeout': closeout_cases(),
        'handoff': handoff_cases(),
        'transcripts': transcript_cases(),
        # Catalog reads through the engine's own dispatch, as rpc-serve answers them.
        'reads': reads_cases(),
        # Stalker libraries read from bundles on disk.
        'captures': capture_cases(),
        # Closeout deliveries and the projects overview, with registered (fixture) disks.
        'projects': project_cases(),
        # Mission exports reviewed against a registry on the archive.
        'missions': mission_cases(),
        # The disk index: overview, folders and progress.
        'storage': storage_cases(),
        # Clipper's guards and discarding a Stalker session, against a fixture Clipper and HOME.
        'clipper': clipper_cases(),
        # E1 · catalog writes: answers and the catalog they leave, table by table.
        'writes': write_cases(),
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
