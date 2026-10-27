#!/usr/bin/env python3
"""Build a reviewable .app. Distribution requires a supplied relocatable runtime."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, help='New .app path; an existing output is never removed')
    parser.add_argument('--configuration', choices=['debug', 'release'], default='release')
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--python', help='Explicit Python executable for development only')
    parser.add_argument('--runtime', help='Relocatable Python runtime root containing bin/python3 and its license')
    parser.add_argument('--identity', help='Developer ID identity; optional ad-hoc signing for development')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    output = Path(args.output).absolute()
    if output.exists() or output.is_symlink(): parser.error('Output exists; choose a new artifact path')
    if bool(args.python) == bool(args.runtime): parser.error('Choose --python for development or --runtime for a bundled build')
    if not args.skip_build:
        subprocess.run(['swift', 'build', '--package-path', str(repo / 'native/MediaMenu'), '-c', args.configuration], check=True)
    result = subprocess.run(['swift', 'build', '--package-path', str(repo / 'native/MediaMenu'), '-c', args.configuration, '--show-bin-path'],
                            check=True, capture_output=True, text=True)
    binary = Path(result.stdout.strip()) / 'MediaMenu'
    if not binary.is_file(): parser.error('Build the native executable first')
    contents = output / 'Contents'
    executable_dir = contents / 'MacOS'; executable_dir.mkdir(parents=True)
    resources = contents / 'Resources'; resources.mkdir()
    with tempfile.TemporaryDirectory(prefix='creator-icon-') as temporary:
        iconset = Path(temporary) / 'Creator.iconset'
        subprocess.run(['swift', str(repo / 'scripts/media_icon.swift'), str(iconset)], check=True)
        subprocess.run(['/usr/bin/iconutil', '-c', 'icns', str(iconset), '-o', str(resources / 'Creator.icns')], check=True)
    shutil.copy2(binary, executable_dir / 'MediaMenu')
    backend = resources / 'backend'; source = backend / 'source'; source.mkdir(parents=True)
    for name in ['nu9ve.py', 'configuration.py', 'constants.py']:
        shutil.copy2(repo / name, source / name)
    for name in ['media', 'manager']:
        shutil.copytree(repo / name, source / name, ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
    if args.runtime:
        runtime = Path(args.runtime).resolve()
        if not (runtime / 'bin/python3').exists(): parser.error('Runtime must contain bin/python3')
        if not any(runtime.glob('LICENSE*')): parser.error('Bundle the Python runtime license')
        # Preserve only links confined to the runtime; external SDK/runtime links are not portable.
        for path in runtime.rglob('*'):
            if path.is_symlink() and runtime not in path.resolve().parents:
                parser.error('Runtime has external symlink: ' + str(path))
        shutil.copytree(runtime, backend / 'runtime', symlinks=True)
        python = backend / 'runtime/bin/python3'
        subprocess.run([str(python), '-I', '-S', '-c', 'import sqlite3, hashlib, ssl, json; print("Bundled runtime OK")'], check=True)
    else:
        python = Path(args.python).expanduser().absolute()
        subprocess.run([str(python), '-S', '-c', 'import sqlite3, hashlib, json'], check=True)
        # Relative to Resources: installation must not retain the build directory path.
        (resources / 'backend-config.json').write_text(json.dumps({'python': str(python), 'script': 'backend/source/nu9ve.py'}))
    fingerprint = hashlib.sha256()
    inputs = [repo / name for name in ['nu9ve.py', 'configuration.py', 'constants.py', 'scripts/build_media_app.py', 'scripts/media_icon.swift']]
    for folder in ['media', 'manager', 'native/MediaMenu/Sources']:
        inputs.extend(p for p in (repo / folder).rglob('*') if p.suffix in {'.py', '.swift'})
    for path in sorted(inputs):
        fingerprint.update(str(path.relative_to(repo)).encode()); fingerprint.update(b'\0'); fingerprint.update(path.read_bytes())
    build_id = fingerprint.hexdigest()[:12]
    info = {'CFBundleIdentifier': 'app.nu9ve.media', 'CFBundleName': 'n creator utils', 'CFBundleDisplayName': 'n creator utils',
            'CFBundleExecutable': 'MediaMenu', 'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': '0.2.0',
            'CFBundleVersion': '1', 'LSMinimumSystemVersion': '14.0', 'LSUIElement': True,
            'CFBundleIconFile': 'Creator.icns', 'LSApplicationCategoryType': 'public.app-category.productivity',
            'NSHighResolutionCapable': True, 'NSRemovableVolumesUsageDescription': 'Respaldar y organizar media de tus tarjetas y discos elegidos.',
            'Nu9veBuildID': build_id, 'Nu9veBuildDate': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
    identity = args.identity or '-'
    if args.runtime:
        # Sign real Mach-O objects deepest first; no --deep to hide nested failures.
        magic = {b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca'}
        for path in sorted((backend / 'runtime').rglob('*'), key=lambda p: len(p.parts), reverse=True):
            if path.is_file() and not path.is_symlink():
                with path.open('rb') as stream: header = stream.read(4)
                if header in magic:
                    subprocess.run(['/usr/bin/codesign', '--force', '--sign', identity, '--options', 'runtime', str(path)], check=True)
    subprocess.run(['/usr/bin/codesign', '--force', '--sign', identity, '--options', 'runtime', str(output)], check=True)
    subprocess.run(['/usr/bin/codesign', '--verify', '--strict', str(output)], check=True)
    print(json.dumps({'app': str(output), 'bundled_runtime': bool(args.runtime), 'developer_id_signed': bool(args.identity),
                      'notarized': False, 'release_ready': False, 'remaining_gates': ['camera/device validation', 'GUI acceptance', 'notarization and Gatekeeper acceptance']}, indent=2))


if __name__ == '__main__': main()
