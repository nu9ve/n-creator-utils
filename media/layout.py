"""Human destination names, decided at plan time and stored in the plan.

Archive mirrors the historical MEDIA layout (type/year/month, dated names, Sony XML under
metadata/). Work copies are flat per project and contain only editable media and Stalker
bundles, so a folder lists in capture order.
"""
import datetime
import json
from pathlib import Path
import re
import shutil
import subprocess
import wave
import xml.etree.ElementTree as ET

from manager.inventory import KINDS
from .common import slug

MODELS = {'ILCE-6400': 'SONYA6400'}
MEDIA_KINDS = ('video', 'audio', 'image', 'raw_image')
LONG_RECORDING_SECONDS = 600


def kind_of(path):
    lowered = '/' + path.lower()
    # Camera structure (Sony thumbnails/indexes, AVF_INFO) is support even when it is a JPG.
    if '/thmbnl/' in lowered or '/avf_info/' in lowered or '/private/sony/' in lowered or ('/m4root/' in lowered and '/m4root/clip/' not in lowered):
        return 'support'
    suffix = Path(path).suffix.lower()
    return next((kind for kind, suffixes in KINDS.items() if suffix in suffixes), None)


def tag(value):
    return slug(value).upper()


def sony_xml(path):
    try:
        if path.stat().st_size > 1024 * 1024:
            return {}
        root = ET.fromstring(path.read_bytes())
    except (OSError, ET.ParseError):
        return {}
    found = {}
    for element in root.iter():
        name = element.tag.split('}')[-1]
        if name == 'CreationDate' and element.get('value'):
            found['created'] = datetime.datetime.fromisoformat(element.get('value'))
        elif name == 'Duration' and element.get('value', '').isdigit():
            found['frames'] = int(element.get('value'))
        elif name == 'VideoFrame':
            fps = re.match(r'[0-9.]+', element.get('captureFps') or element.get('formatFps') or '')
            found['fps'] = float(fps.group()) if fps else None
        elif name == 'VideoLayout' and (element.get('pixel') or '').isdigit():
            found['width'] = int(element.get('pixel'))
        elif name == 'Device':
            model = element.get('modelName') or ''
            found['model'] = MODELS.get(model, tag((element.get('manufacturer') or '') + model)) if model else None
    if found.get('frames') is not None and found.get('fps'):
        found['seconds'] = found['frames'] / found['fps']
    return found


def probe(path):
    """Optional ffprobe facts; the copy engine never depends on it."""
    if not shutil.which('ffprobe'):
        return {}
    try:
        result = subprocess.run(['ffprobe', '-v', 'error', '-print_format', 'json', '-show_format', '-show_streams', str(path)],
                                capture_output=True, timeout=30)
        data = json.loads(result.stdout or b'{}')
    except (OSError, subprocess.SubprocessError, ValueError):
        return {}
    found = {}
    fmt = data.get('format') or {}
    try:
        found['seconds'] = float(fmt['duration'])
    except (KeyError, TypeError, ValueError):
        pass
    created = (fmt.get('tags') or {}).get('creation_time')
    if created:
        try:
            found['created'] = datetime.datetime.fromisoformat(created.replace('Z', '+00:00')).astimezone()
        except ValueError:
            pass
    widths = [s.get('width') for s in data.get('streams') or [] if s.get('codec_type') == 'video' and s.get('width')]
    if widths:
        found['width'] = max(widths)
    return found


def wav_seconds(path):
    try:
        with wave.open(str(path), 'rb') as stream:
            return stream.getnframes() / float(stream.getframerate())
    except (OSError, EOFError, wave.Error, ZeroDivisionError):
        return None


def facts(source, file):
    """Capture time (local wall clock), duration and model for one media file."""
    path = source / file['path']
    kind = kind_of(file['path'])
    found = {}
    if kind == 'video' and path.suffix.lower() == '.mp4':
        found = sony_xml(path.with_name(path.stem + 'M01.XML'))
    if kind in {'video', 'audio'} and 'seconds' not in found:
        if path.suffix.lower() == '.wav':
            seconds = wav_seconds(path)
            if seconds is not None:
                found['seconds'] = seconds
        if 'seconds' not in found or 'created' not in found:
            found = dict(probe(path), **found)
    return dict(found, created=found.get('created') or datetime.datetime.fromtimestamp(file['stamp']['mtime_ns'] / 1e9).astimezone())


def stamp(created):
    return created.strftime('%y') + created.strftime('%b').upper() + created.strftime('%d-%H%M%S')


def duration(seconds):
    total = int(round(seconds or 0))
    return '{:02d}-{:02d}'.format(total // 60, total % 60)


def model_for(unit, file, found):
    if found.get('model'):
        return found['model']
    if unit['camera'] == 'sony':
        return MODELS['ILCE-6400']
    if unit['camera'] == 'h6':
        mic = re.search(r'_(TR\d+|LR|MS)$', Path(file['path']).stem.upper())
        return 'H6' + (mic.group(1) if mic else '')
    return tag(unit['camera'])


def archive_folder(kind, found, camera):
    if kind == 'video':
        if camera in {'dji-drone'}:
            return 'vdron'
        if camera in {'dji-360', 'insta360'}:
            return '360/videos'
        return 'v4k' if (found.get('width') or 0) >= 3840 else 'videos'
    if kind == 'audio':
        return 'recordingslong' if (found.get('seconds') or 0) > LONG_RECORDING_SECONDS else 'recordings'
    if kind == 'raw_image':
        return 'rimages'
    return '360/images' if camera in {'dji-360', 'insta360'} else 'images'


def bundle_name(plan, bundle):
    return plan['source_name'] if bundle == '.' else bundle.rsplit('/', 1)[-1]


def bundle_date(name, files):
    match = re.search(r'(\d{4})-(\d{2})-(\d{2})', name)
    if match:
        return match.group(1) + '/' + match.group(2)
    newest = max(f['stamp']['mtime_ns'] for f in files)
    return datetime.datetime.fromtimestamp(newest / 1e9).strftime('%Y/%m')


def build_layout(source, plan, units, files):
    """Map every source path to {'archive': rel, 'work': rel|None} below each volume prefix."""
    source = Path(source)
    by_unit = {}
    for file in files:
        by_unit.setdefault(file['unit_id'], []).append(file)
    layout, taken, support = {}, {}, []
    first_capture = None

    def claim(rel):
        # Deterministic, plan-local disambiguation for clips recorded in the same second.
        stem, dot, ext = rel.rpartition('.')
        candidate, n = rel, 2
        while candidate.lower() in taken:
            candidate = stem.replace('[]', '-{}[]'.format(n)) + dot + ext
            n += 1
        taken[candidate.lower()] = True
        return candidate

    for unit in sorted(units, key=lambda u: u['id']):
        members = sorted(by_unit[unit['id']], key=lambda f: f['path'])
        project = slug(unit['project'])
        work_root = slug(unit['profile']) + '_raw/' + project
        label = '{}-{}'.format(tag(plan['country']), tag(project))
        if unit['kind'] == 'stalker_session':
            name = bundle_name(plan, unit['bundle'])
            prefix = '' if unit['bundle'] == '.' else unit['bundle'] + '/'
            month = bundle_date(name, members)
            for file in members:
                inner = file['path'][len(prefix):]
                layout[file['path']] = {'archive': 'stalker/{}/{}/{}'.format(month, name, inner),
                                        'work': '{}/{}/{}'.format(work_root, name, inner)}
            continue
        media = [f for f in members if kind_of(f['path']) in MEDIA_KINDS]
        if not media:
            support.append((unit, members))
            continue
        primary_base = None
        for file in media:
            kind = kind_of(file['path'])
            found = facts(source, file)
            created = found['created']
            first_capture = created if first_capture is None or created < first_capture else first_capture
            parts = [stamp(created), model_for(unit, file, found)]
            if kind in {'video', 'audio'}:
                parts.append(duration(found.get('seconds')))
            ext = Path(file['path']).suffix.lstrip('.').upper()
            folder = archive_folder(kind, found, unit['camera'])
            name = claim('{}/{}_{}[].{}'.format(created.strftime('%Y/%m'), '_'.join(parts), label, ext)).split('/', 2)[2]
            layout[file['path']] = {'archive': '{}/{}/{}'.format(folder, created.strftime('%Y/%m'), name),
                                    'work': '{}/{}'.format(work_root, name)}
            primary_base = primary_base or (created.strftime('%Y/%m'), name.rsplit('.', 1)[0])
        family = 'sony' if unit['camera'] == 'sony' else slug(unit['camera'])
        for file in members:
            if file['path'] in layout:
                continue
            # Sidecars (Sony XML, LRV/THM) keep the clip's name under metadata/, archive only.
            ext = Path(file['path']).suffix.lstrip('.').upper()
            rel = claim('metadata/{}/{}/{}.{}'.format(family, primary_base[0], primary_base[1], ext))
            layout[file['path']] = {'archive': rel, 'work': None}

    if support:
        when = first_capture or max(datetime.datetime.fromtimestamp(f['stamp']['mtime_ns'] / 1e9) for _, m in support for f in m)
        for unit, members in support:
            family = 'sony' if unit['camera'] == 'sony' or any('/private/' in '/' + f['path'].lower() or f['path'].lower().startswith(('avf_info/', 'private/')) for f in members) else slug(unit['camera'])
            model = MODELS['ILCE-6400'] if family == 'sony' else tag(unit['camera'])
            folder = 'metadata/{}/support/{}/{}_{}-{}_{}'.format(family, when.strftime('%Y/%m'), when.strftime('%d%m-%H%M%S'),
                                                                 tag(plan['country']), tag(unit['project']), model)
            for file in members:
                # Camera indexes/thumbnails keep their original tree so the card can be reconstructed.
                layout[file['path']] = {'archive': '{}/{}'.format(folder, file['path']), 'work': None}
    return layout
