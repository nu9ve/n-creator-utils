#!/usr/bin/env python3
"""Test environment for nüün: test disks, test cards and their own catalog, to walk whole flows in the app without
touching a real SD, MEDIA, ESSD or SSD1.

    python3 scripts/sandbox.py create          # disks (sparse images), cards with test media, catalog, registered disks
    python3 scripts/sandbox.py open            # mounts what is missing and opens nüün inside the environment
    python3 scripts/sandbox.py card sony|drone|gopro|h6|canon   # (re)fills that card with fresh test media
    python3 scripts/sandbox.py compare sony    # the same card ingest (plan, verified copy, cleanup) by Python, then Rust
    python3 scripts/sandbox.py status          # what is mounted and registered
    python3 scripts/sandbox.py env             # shell exports to use the CLI inside the environment
    python3 scripts/sandbox.py detach          # ejects every test disk
    python3 scripts/sandbox.py remove          # ejects and deletes the whole environment

Everything lives in one folder (default ~/Library/Application Support/n-creator-utils/sandbox, never inside the real
catalog folder). Its marker `.nuun-sandbox`, `NU9VE_SANDBOX` and a catalog inside it are what let nüün count these
images as a card or a USB disk (media/volumes.py `sandbox_image`); without the three, an image is still never a card
or a replica. nüün shows a yellow «Entorno de prueba» strip while it runs here, and its engines use a HOME inside the
environment, so Stalker libraries, the missions inbox and Clipper projects are test ones too.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

DEFAULT_ROOT = Path.home() / 'Library/Application Support/n-creator-utils/sandbox'
REAL_CATALOG = Path.home() / 'Library/Application Support/n-creator-utils/media'
MARKER = '.nuun-sandbox'
APP = Path.home() / 'Applications/nuun.app'
# name: (filesystem, size, role); SD* images are cards, the rest USB disks.
DISKS = {
    'MEDIA-TEST': ('ExFAT', '8g', 'archive'),
    'ESSD-TEST': ('JHFS+', '8g', 'work'),
    'SD-SONY': ('MS-DOS', '1g', 'card'),
    'SD-DRONE': ('MS-DOS', '1g', 'card'),
}
CARDS = {'sony': 'SD-SONY', 'drone': 'SD-DRONE', 'gopro': 'SD-SONY', 'h6': 'SD-SONY', 'canon': 'SD-DRONE'}


def run(*args, **kwargs):
    return subprocess.run(list(args), check=True, capture_output=True, **kwargs)


def environment(root):
    return dict(os.environ, NU9VE_SANDBOX=str(root), NU9VE_MEDIA_HOME=str(root / 'state'), HOME=str(root / 'home'),
                CLIPPER_PROJECTS_ROOT=str(root / 'home/Downloads/clipper-projects'))


ENGINE = REPO / 'engine/target/release/nuun-engine'


def rpc(root, method, params=None, engine='python', check=True):
    """One request to an engine inside the environment (`rust`: engine/target/release/nuun-engine)."""
    command = ([str(ENGINE), 'rpc'] if engine == 'rust' else [sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', 'rpc'])
    done = subprocess.run(command, cwd=str(REPO),
                          input=json.dumps({'method': method, 'params': params or {}}), capture_output=True, text=True,
                          env=environment(root))
    answer = json.loads(done.stdout or '{}')
    if not check:
        return answer
    if not answer.get('ok'):
        raise SystemExit('%s (%s): %s' % (method, engine, answer.get('error') or done.stderr[-400:]))
    return answer['data']


def mounted():
    """{image path: mount point} of every attached image."""
    info = plistlib.loads(run('/usr/bin/hdiutil', 'info', '-plist').stdout)
    found = {}
    for image in info.get('images', []):
        mounts = [e['mount-point'] for e in image.get('system-entities', []) if e.get('mount-point')]
        if mounts:
            found[str(Path(image['image-path']).resolve())] = mounts[0]
    return found


def image_path(root, name):
    path = root / 'images' / (name + '.sparseimage')
    if path.resolve() != path:
        raise SystemExit('Test images must live under the sandbox without symlinks: ' + str(path))
    return path


def attach(root, name):
    path = image_path(root, name)
    current = mounted().get(str(path))
    if current:
        return current
    info = plistlib.loads(run('/usr/bin/hdiutil', 'attach', '-plist', str(path)).stdout)
    return next(e['mount-point'] for e in info['system-entities'] if e.get('mount-point'))


def check_root(root):
    root = root.expanduser().resolve()
    real = REAL_CATALOG.resolve()
    if root == real or real in root.parents or root in real.parents:
        raise SystemExit('The test environment must not contain, or be inside, the real catalog folder: ' + str(real))
    return root


def fill_card(root, kind):
    """A card's content as a camera leaves it; the card is emptied first (it is a test image)."""
    from tests.test_devices import dng, jpeg
    mount = Path(attach(root, CARDS[kind]))
    for entry in mount.iterdir():
        if entry.name.startswith('.'):
            continue
        # FAT cards: macOS adds and drops `._*` companions while a folder is emptied.
        shutil.rmtree(entry, ignore_errors=True) if entry.is_dir() else entry.unlink(missing_ok=True)
    sony_xml = ('<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00"><Duration value="300"/>'
                '<CreationDate value="2026-11-04T16:32:12-07:00"/><VideoFormat><VideoFrame captureFps="29.97p" '
                'formatFps="29.97p"/><VideoLayout pixel="3840" numOfVerticalLine="2160"/></VideoFormat>'
                '<Device manufacturer="Sony" modelName="ILCE-6400"/></NonRealTimeMeta>')
    files = {}

    def fake(seed, size):
        # The same bytes for the same card every time, so two ingests of a refilled card can be compared.
        import hashlib
        block = hashlib.sha256(seed.encode()).digest()
        return (block * (size // len(block) + 1))[:size]

    if kind == 'sony':
        for n in (1, 2):
            files['PRIVATE/M4ROOT/CLIP/C000%d.MP4' % n] = fake('sony-%d' % n, 256 * 1024)
            files['PRIVATE/M4ROOT/CLIP/C000%dM01.XML' % n] = sony_xml.replace('16:32:12', '16:3%d:12' % (2 + n)).encode()
            files['PRIVATE/M4ROOT/THMBNL/C000%dT01.JPG' % n] = b'thumb'
        files['PRIVATE/M4ROOT/MEDIAPRO.XML'] = b'<MediaProfile/>'
        files['DCIM/100MSDCF/DSC00001.JPG'] = jpeg(make='SONY', model='ILCE-6400', serial='12345', flight=False)
    elif kind == 'drone':
        for n in (1, 2, 3):
            when = '2026:11:04 16:3%d:12' % n
            files['DCIM/101MEDIA/DJI_000%d.JPG' % n] = jpeg(when=when)
            files['DCIM/101MEDIA/DJI_000%d.DNG' % n] = dng(when=when)
        files['DCIM/101MEDIA/DJI_0004.MP4'] = fake('drone-4', 256 * 1024)
        files['MISC/THM/101/DJI_0004.THM'] = b'thm'
    elif kind == 'gopro':
        files['DCIM/100GOPRO/GX010001.MP4'] = fake('gopro-mp4', 256 * 1024)
        files['DCIM/100GOPRO/GX010001.LRV'] = fake('gopro-lrv', 32 * 1024)
        files['DCIM/100GOPRO/GX010001.THM'] = b'thm'
    elif kind == 'h6':
        from scripts.engine_golden import pcm_wav
        files['ZOOM0001/ZOOM0001_LR.WAV'] = pcm_wav(3, 48000, 2, 3)
        files['ZOOM0001/ZOOM0001_Tr1.WAV'] = pcm_wav(3, 48000, 1, 3)
    elif kind == 'canon':
        files['DCIM/100CANON/IMG_0001.JPG'] = jpeg(make='Canon', model='Canon EOS R6', serial='C1', flight=False)
        files['DCIM/100CANON/IMG_0002.JPG'] = jpeg(make='Canon', model='Canon EOS R6', serial='C1', flight=False,
                                                   when='2026:11:04 16:32:14')
    for rel, data in files.items():
        path = mount / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        # Give the separate H6 recordings distinct capture times. FAT stores mtimes in two-second increments.
        timestamp = 1778000004 if kind == 'h6' and rel.endswith('_Tr1.WAV') else 1778000000
        os.utime(path, (timestamp, timestamp))
    return mount, len(files)


def create(root):
    if (root / MARKER).exists():
        raise SystemExit('Already created: %s (use `remove` first, or `open`)' % root)
    (root / 'images').mkdir(parents=True, exist_ok=True)
    (root / 'home/Movies/Stalker').mkdir(parents=True, exist_ok=True)
    (root / 'home/Downloads').mkdir(parents=True, exist_ok=True)
    (root / MARKER).write_text('nüün test environment (scripts/sandbox.py). Disks and catalog here are test data.\n')
    for name, (filesystem, size, _) in DISKS.items():
        if not image_path(root, name).exists():
            run('/usr/bin/hdiutil', 'create', '-size', size, '-fs', filesystem, '-volname', name, '-type', 'SPARSE',
                str(root / 'images' / name))
        print('disk', name, '→', attach(root, name))
    for name, (_, _, role) in DISKS.items():
        if role != 'card':
            mount = attach(root, name)
            rpc(root, 'register_volume', {'root': mount, 'name': name, 'role': role})
    rpc(root, 'defaults', {'archive': 'MEDIA-TEST', 'work': 'ESSD-TEST', 'project': 'prueba'})
    for kind in ('sony', 'drone'):
        mount, count = fill_card(root, kind)
        print('card', kind, '→', mount, count, 'files')
    print('Test environment ready:', root)


def files_under(folder, plan=None):
    """{relative path: sha256} of the visible files below a mount (hidden and `._*` entries left out)."""
    import hashlib
    import uuid
    found = {}
    for path in sorted(Path(folder).rglob('*')):
        rel = path.relative_to(folder)
        if path.is_file() and not any(part.startswith('.') for part in rel.parts):
            if plan and rel.name == 'units.ndjson' and rel.parent.name == '_nuun':
                rows = [json.loads(line) for line in path.read_bytes().splitlines()]
                for row in rows:
                    # Refilled cards get new file identities; new jobs also get a UUID and time.
                    uuid.UUID(row['job_id'])
                    uuid.UUID(row['asset_id'].removeprefix('ingest-'))
                    expected_units = {unit['id'] for unit in plan['units'] if unit.get('mission')}
                    if row['job_id'] != plan['id'] or row['asset_id'] not in expected_units:
                        raise SystemExit('The units map references a different job or unit')
                    row['job_id'] = '<job>'
                    row['asset_id'] = '<unit>'
                    del row['updated_at']
                found[str(rel)] = sorted(rows, key=lambda row: row['original_name'])
            else:
                found[str(rel)] = hashlib.sha256(path.read_bytes()).hexdigest()
    return found


def reset(root, engine):
    """Empty test disks and a fresh catalog with the test disks registered again (test images only)."""
    for name, (_, _, role) in DISKS.items():
        if role != 'card':
            mount = Path(attach(root, name))
            for entry in mount.iterdir():
                if not entry.name.startswith('.'):
                    shutil.rmtree(entry, ignore_errors=True) if entry.is_dir() else entry.unlink(missing_ok=True)
    shutil.rmtree(root / 'state', ignore_errors=True)
    for name, (_, _, role) in DISKS.items():
        if role != 'card':
            rpc(root, 'register_volume', {'root': attach(root, name), 'name': name, 'role': role}, engine)
    rpc(root, 'defaults', {'archive': 'MEDIA-TEST', 'work': 'ESSD-TEST', 'project': 'prueba'}, engine)


def seed_drone_mission(root):
    """Install one synthetic Atlas history using the same schema as the existing differential fixtures."""
    import hashlib
    from media.missions import empty_state, merge, render
    from media.recipes import TEMPLATE
    from tests.test_missions import capture, export

    flow = TEMPLATE['missions'][0]
    captures = [capture('C%d' % n, '2026-11-04T23:3%d:12Z' % n,
                        mediaCreatedAt='2026-11-04 16:3%d:12' % n,
                        imageJPEGReference='DJI_000%d.JPG' % n,
                        latitude=29.0984139, longitude=-110.9622722) for n in (1, 2, 3)]
    data = export('2026-11-05T05:00:00Z', 'all', [{'id': 'P1', 'name': 'Mision de prueba'}], captures)
    history = empty_state(flow)
    merge(history, data, hashlib.sha256(json.dumps(data).encode()).hexdigest(), 'coverage-fixture.json', flow)
    for source in history['nuun']['sources'].values():
        source['incorporated_at'] = '2026-11-05T05:00:00Z'
    path = Path(attach(root, 'MEDIA-TEST')) / flow['archive_area'] / 'history.json'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(render(history, flow))


def ingest(root, kind, engine, cleanup=True, with_mission=False):
    """A whole card ingest by one engine: plan, reviewed document, verified copy and cleanup. What it leaves."""
    reset(root, engine)
    card, _ = fill_card(root, kind)
    if with_mission:
        seed_drone_mission(root)
    params = {'source': str(card), 'archive': 'MEDIA-TEST', 'work': 'ESSD-TEST', 'project': 'prueba', 'cleanup': cleanup}
    if kind == 'drone':
        params['camera'] = 'dji-drone'
    card_before = files_under(card)
    planned = rpc(root, 'plan_document', params, engine, check=False)
    oracle = rpc(root, 'plan_document', params, 'rust' if engine == 'python' else 'python', check=False)
    # Exact plan parity uses the same inode, ctime, mount and catalog IDs. The refilled card in the
    # second apply gets new identities, so the post-apply comparison checks routes and content.
    if planned.get('ok') != oracle.get('ok') or (
        planned.get('data') != oracle.get('data') if planned.get('ok')
        else planned.get('error', {}).get('code') != oracle.get('error', {}).get('code')
    ):
        raise SystemExit('Plan or error differs on the same card and catalog: ' + kind)
    if not planned.get('ok'):
        # A refused plan is a result too: both engines must refuse the same card for the same reason.
        return {'plan_error': planned.get('error', {}).get('code'), 'card_after': files_under(card),
                'card_retained': files_under(card) == card_before,
                'archive': files_under(attach(root, 'MEDIA-TEST')), 'work': files_under(attach(root, 'ESSD-TEST'))}
    document = planned['data']
    plan = json.loads(document)
    applied = rpc(root, 'apply', {'plan': document, 'digest': plan['digest']}, engine, check=False)
    job = rpc(root, 'job', {'job_id': plan['id']}, engine) if applied.get('ok') else None
    resumed = rpc(root, 'resume', {'job_id': plan['id']}, engine, check=False) if job else {}
    if applied.get('data', {}).get('verify_cache'):
        print('  %s verification cache: %s' % (engine, applied['data']['verify_cache']))
    return {'plan': {k: plan[k] for k in ('layout', 'preview', 'cleanup_profile')},
            'apply': applied.get('data', {}).get('state') if applied.get('ok') else applied.get('error'),
            'job_state': job and job['state'],
            'resume_state': resumed.get('data', {}).get('state') if resumed.get('ok') else resumed.get('error'),
            'archive': files_under(attach(root, 'MEDIA-TEST')), 'work': files_under(attach(root, 'ESSD-TEST'), plan),
            'card_after': files_under(card), 'card_retained': files_under(card) == card_before}


def compare(root, kind, cleanup=True, with_mission=False):
    if not ENGINE.is_file():
        raise SystemExit('Build the engine first: cd engine && cargo build --release --offline')
    import re

    def stable(value):
        # Unit ids hash the card files' inode and ctime, new each time the card is filled: not an engine difference.
        return json.loads(re.sub(r'ingest-[0-9a-f-]{36}', 'ingest-<unit>', json.dumps(value)))

    python = stable(ingest(root, kind, 'python', cleanup, with_mission))
    rust = stable(ingest(root, kind, 'rust', cleanup, with_mission))
    same = True
    for key in sorted(python.keys() | rust.keys()):
        if python.get(key) != rust.get(key):
            same = False
            print('DIFFERS', key)
            print('  python:', json.dumps(python.get(key), sort_keys=True)[:600])
            print('  rust:  ', json.dumps(rust.get(key), sort_keys=True)[:600])
    print('%s card%s: %s' % (kind, '' if cleanup else ' (no cleanup)', 'same result in both engines' if same else 'results differ'))
    if 'plan_error' in python:
        print('  plan refused by both:', python['plan_error'], '| left on card:', len(python['card_after']))
    else:
        print('  apply:', python['apply'], '| archive files:', len(python['archive']), '| work files:', len(python['work']),
              '| left on card:', len(python['card_after']))
    if 'plan_error' in python:
        accepted = (kind == 'drone' and cleanup and not with_mission and python['plan_error'] == 'mission_export_missing'
                    and python['card_retained'] and not python['archive'] and not python['work'])
    else:
        expected = 'source_retained' if not cleanup else 'camera_action_required' if kind == 'sony' else 'completed'
        accepted = all(python.get(key) == expected for key in ('apply', 'job_state', 'resume_state'))
        accepted = accepted and bool(python['archive']) and bool(python['work'])
        accepted = accepted and (python['card_retained'] if not cleanup else not python['card_retained'])
        if with_mission:
            rows = next((rows for path, rows in python['work'].items() if path.endswith('/_nuun/units.ndjson')), [])
            accepted = accepted and len(rows) == 3 and all(row['mission_state'] == 'matched' for row in rows)
    if same and not accepted:
        print('  unexpected outcome; matching failures do not validate ingest')
    return 0 if same and accepted else 1


def status(root):
    current = mounted()
    for name, (_, _, role) in DISKS.items():
        print('%-11s %-8s %s' % (name, role, current.get(str(image_path(root, name)), '—')))
    if (root / 'state/catalog.sqlite3').exists():
        print('registered:', ', '.join(v['name'] + (' (online)' if v['online'] else '') for v in rpc(root, 'volumes')))


def open_app(root, app):
    for name in DISKS:
        attach(root, name)
    running = subprocess.run(['/usr/bin/pgrep', '-f', 'Contents/MacOS/MediaMenu'], capture_output=True)
    if running.returncode == 0:
        raise SystemExit('Close nüün first: the test environment opens its own instance.')
    env = environment(root)
    args = ['/usr/bin/open', '-n', '-a', str(app)]
    for key in ('NU9VE_SANDBOX', 'NU9VE_MEDIA_HOME'):
        args += ['--env', key + '=' + env[key]]
    subprocess.run(args, check=True)
    print('nüün opened in the test environment (yellow strip). Close it before opening your normal nüün.')


def detach(root):
    current = mounted()
    for name in DISKS:
        mount = current.get(str(image_path(root, name)))
        if mount:
            run('/usr/bin/hdiutil', 'detach', mount)
            print('ejected', name)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('command', choices=['create', 'open', 'card', 'compare', 'status', 'env', 'detach', 'remove'])
    parser.add_argument('kind', nargs='?', choices=sorted(CARDS))
    parser.add_argument('--root', default=str(DEFAULT_ROOT))
    parser.add_argument('--app', type=Path, default=APP, help='open: path to a development .app')
    parser.add_argument('--with-mission', action='store_true', help='compare drone: seed a matching Atlas export')
    parser.add_argument('--no-cleanup', action='store_true', help='compare: back up without cleaning the card')
    args = parser.parse_args()
    if args.with_mission and (args.command != 'compare' or args.kind != 'drone'):
        parser.error('--with-mission requires compare drone')
    root = check_root(Path(args.root))
    if args.command == 'create':
        return create(root)
    if not (root / MARKER).exists():
        raise SystemExit('No test environment at %s: run `create` first' % root)
    if args.command == 'open':
        return open_app(root, args.app.expanduser().resolve())
    if args.command == 'card':
        if not args.kind:
            raise SystemExit('Which card? ' + '|'.join(sorted(CARDS)))
        mount, count = fill_card(root, args.kind)
        return print('card', args.kind, '→', mount, count, 'files')
    if args.command == 'compare':
        return compare(root, args.kind or 'sony', not args.no_cleanup, args.with_mission)
    if args.command == 'status':
        return status(root)
    if args.command == 'env':
        env = environment(root)
        for key in ('NU9VE_SANDBOX', 'NU9VE_MEDIA_HOME', 'HOME', 'CLIPPER_PROJECTS_ROOT'):
            print('export %s=%s' % (key, json.dumps(env[key])))
        return None
    if args.command == 'detach':
        return detach(root)
    if args.command == 'remove':
        detach(root)
        shutil.rmtree(root)
        return print('removed', root)
    return None


if __name__ == '__main__':
    sys.exit(main())
