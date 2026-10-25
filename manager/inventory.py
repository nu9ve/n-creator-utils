"""Read-only discovery. Observations are not a transfer plan or validation receipt."""

import argparse
import json
import os
from pathlib import Path
import xml.etree.ElementTree as ET


IGNORED_DIRS = {'.fseventsd', '.Spotlight-V100', '.Trashes', 'System Volume Information'}
KINDS = {
    'video': {'.mov', '.mp4', '.mpg', '.mts', '.insv'},
    'image': {'.png', '.jpg', '.jpeg', '.gif', '.heic', '.insp'},
    'raw_image': {'.cr2', '.cr3', '.nef', '.arw', '.dng'},
    'audio': {'.wav', '.mp3', '.caf', '.flac', '.aif', '.aiff'},
    'auxiliary': {'.xml', '.lrv', '.thm', '.gyro', '.aae', '.bmp', '.hprj'},
}


def package_kind(path):
    if path.name.lower().endswith('.stalker'):
        return 'stalker_session'
    if (path / 'clipper-project.json').exists():
        return 'clipper_project'
    return None


def walk_entries(root):
    """Do not follow symlinks; surface IO errors instead of a partial success."""
    with os.scandir(root) as entries:
        children = sorted(entries, key=lambda entry: entry.name)
    for entry in children:
        path = Path(entry.path)
        if entry.is_symlink():
            yield path, 'symlink'
        elif entry.is_dir(follow_symlinks=False):
            if entry.name not in IGNORED_DIRS:
                yield path, 'directory'
        elif entry.is_file(follow_symlinks=False):
            yield path, 'file'
        else:
            yield path, 'special'


def assert_generic_source(source):
    """Fail before the legacy importer changes any file, even for nested bundles."""
    source = Path(source)
    if source.is_symlink():
        raise ValueError('Generic save refuses links/special files: {}'.format(source))
    source = source.resolve()
    for ancestor in (source, *source.parents):
        if package_kind(ancestor):
            raise ValueError('Generic save cannot process a Stalker/Clipper package: {}'.format(ancestor))

    def visit(directory):
        if package_kind(directory):
            raise ValueError('Generic save cannot process a Stalker/Clipper package: {}'.format(directory))
        for child, kind in walk_entries(directory):
            if kind in {'symlink', 'special'}:
                raise ValueError('Generic save refuses links/special files: {}'.format(child))
            if kind == 'directory':
                visit(child)
    visit(source)


def camera_evidence(path):
    """File-local hints only. DJI filenames and directory residues prove no model."""
    evidence = []
    if path.suffix.lower() in {'.insv', '.insp'}:
        evidence.append({'source': 'extension', 'family': 'insta360', 'confidence': 'high'})
    parts = [part.lower() for part in path.parts]
    if any(parts[i:i + 3] == ['private', 'm4root', 'clip'] for i in range(len(parts) - 2)):
        evidence.append({'source': 'file_path', 'family': 'sony', 'confidence': 'medium'})
    if path.suffix.lower() == '.mp4':
        sidecar = path.with_name(path.stem + 'M01.XML')
        if sidecar.is_file() and not sidecar.is_symlink():
            if sidecar.stat().st_size <= 1024 * 1024:
                try:
                    root = ET.fromstring(sidecar.read_bytes())
                    for element in root.iter():
                        if element.tag.split('}')[-1] == 'Device':
                            evidence.append({'source': 'sony_xml', 'confidence': 'high',
                                             'manufacturer': element.get('manufacturer'),
                                             'model': element.get('modelName')})
                            break
                except ET.ParseError:
                    evidence.append({'source': 'sony_xml', 'error': 'invalid_xml'})
    if path.name.upper().startswith('DJI'):
        evidence.append({'source': 'filename', 'family': 'dji', 'confidence': 'low',
                         'requires_review': True})
    return evidence


def inspect_source(source):
    source = Path(source).expanduser()
    if source.is_symlink() or not source.is_dir():
        raise ValueError('Inspection requires an existing directory, not a link: {}'.format(source))
    source = source.resolve()
    units = []

    def observe_package(path, kind):
        record = {'relative_path': path.relative_to(source).as_posix(), 'kind': kind,
                  'issues': [], 'files': [], 'bytes': 0}
        marker = path / ('manifest.json' if kind == 'stalker_session' else 'clipper-project.json')
        if marker.is_symlink() or not marker.is_file() or marker.stat().st_size > 1024 * 1024:
            record['issues'].append('missing_unsafe_or_large_manifest')
        else:
            try:
                data = json.loads(marker.read_text(encoding='utf-8'))
                if not isinstance(data, dict):
                    raise ValueError('Manifest must be an object')
                if kind == 'stalker_session':
                    record.update(session_id=data.get('sessionID'), status=data.get('status'),
                                  producer=data.get('producer'), schema_version=data.get('schemaVersion'))
                    if data.get('status') != 'completed':
                        record['issues'].append('session_not_completed')
            except (ValueError, UnicodeError):
                record['issues'].append('invalid_manifest')
        # Do not scan a recording in progress; even a completed snapshot can change.
        if kind == 'stalker_session' and (record['issues'] or record.get('producer') != 'stalker'):
            record['issues'].append('not_scanned')
            record['bytes'] = None
            units.append(record)
            return

        def collect(directory):
            for child, entry_kind in walk_entries(directory):
                if entry_kind == 'directory':
                    collect(child)
                elif entry_kind == 'file':
                    size = child.stat().st_size
                    record['files'].append({'relative_path': child.relative_to(path).as_posix(), 'bytes': size})
                    record['bytes'] += size
                else:
                    record['issues'].append('unsafe_entry:' + child.relative_to(path).as_posix())
        collect(path)
        units.append(record)

    def visit(directory):
        package = package_kind(directory)
        if package:
            observe_package(directory, package)
            return
        for path, entry_kind in walk_entries(directory):
            if entry_kind == 'directory':
                visit(path)
            elif entry_kind == 'file':
                stat = path.stat()
                units.append({'relative_path': path.relative_to(source).as_posix(),
                              'kind': next((k for k, extensions in KINDS.items() if path.suffix.lower() in extensions), 'unknown'),
                              'bytes': stat.st_size, 'mtime_ns': stat.st_mtime_ns,
                              'camera_evidence': camera_evidence(path)})
            else:
                units.append({'relative_path': path.relative_to(source).as_posix(),
                              'kind': entry_kind, 'issues': ['not_followed']})
    visit(source)
    return {'schema': 'nu9ve-inspection/1', 'source': str(source), 'read_only': True,
            'transfer_plan': False, 'integrity_verified': False,
            'units': units, 'unit_count': len(units),
            'observed_bytes': sum(unit.get('bytes') or 0 for unit in units)}


def inspect_command(argv=None):
    parser = argparse.ArgumentParser(description='Inspección JSON de solo lectura; no copia ni verifica integridad.')
    parser.add_argument('source', help='directorio de muestra, tarjeta o bundle')
    args = parser.parse_args(argv)
    print(json.dumps(inspect_source(args.source), indent=2, ensure_ascii=False))
