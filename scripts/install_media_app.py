#!/usr/bin/env python3
"""Rebuild and install one canonical local development app, keeping the previous build."""
import argparse
import datetime
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import uuid


def install(app, destination):
    app, destination = Path(app).absolute(), Path(destination).expanduser().absolute()
    if destination.suffix != '.app' or destination.is_symlink():
        raise ValueError('Choose a real .app destination, not a symlink')
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != 'app.nu9ve.media':
        raise ValueError('Not a Nu9ve Media app')
    subprocess.run(['/usr/bin/codesign', '--verify', '--strict', str(app)], check=True)
    processes = subprocess.run(['/bin/ps', '-axo', 'command='], check=True, capture_output=True, text=True).stdout
    if any('/Contents/MacOS/MediaMenu' in line for line in processes.splitlines() if not line.lstrip().startswith(('python', '/bin/zsh', '/bin/bash'))):
        raise RuntimeError('Close Media from its menu before updating. An active operation will not be interrupted.')
    destination.parent.mkdir(parents=True, exist_ok=True)
    token = uuid.uuid4().hex[:8]
    staged = destination.parent / ('.Nu9veMedia-install-' + token + '.app')
    # Keep the old bundle outside Applications so launchers do not index a duplicate.
    backup = destination.parent.parent / 'Library/Application Support/n-creator-utils/app-backups' / (token + '.app')
    previous = destination
    legacy = destination.parent / 'Nu9ve Media.app'
    if not destination.exists() and destination.name == 'n creator utils.app' and legacy.exists():
        previous = legacy
    if previous.is_symlink(): raise ValueError('Previous app must not be a symlink')
    if previous.exists():
        old = plistlib.loads((previous / 'Contents/Info.plist').read_bytes())
        if old.get('CFBundleIdentifier') != 'app.nu9ve.media':
            raise ValueError('Destination belongs to another app')
    try:
        shutil.copytree(app, staged, symlinks=True)
        subprocess.run(['/usr/bin/codesign', '--verify', '--strict', str(staged)], check=True)
        if previous.exists():
            backup.parent.mkdir(parents=True, exist_ok=True)
            previous.rename(backup)
        try:
            staged.rename(destination)
        except BaseException:
            if backup.exists(): backup.rename(previous)
            raise
    finally:
        if staged.exists(): shutil.rmtree(staged)
    subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister', '-f', str(destination)], check=True)
    print('Installed: ' + str(destination))
    print('Build: ' + str(info.get('Nu9veBuildID', 'unknown')))
    if backup.exists(): print('Previous build: ' + str(backup))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--python', default=sys.executable, help='External Python for this local development install')
    parser.add_argument('--destination', default=str(Path.home() / 'Applications/n creator utils.app'))
    parser.add_argument('--app', help='Install an already-built signed bundle; otherwise rebuild current source')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    app = Path(args.app).absolute() if args.app else repo / 'dist' / ('Nu9veMedia-local-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:6] + '.app')
    if not args.app:
        subprocess.run([sys.executable, str(repo / 'scripts/build_media_app.py'), '--output', str(app), '--python', str(Path(args.python).absolute())], check=True)
    install(app, args.destination)


if __name__ == '__main__':
    main()
