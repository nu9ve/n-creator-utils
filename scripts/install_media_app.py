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

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'vendor' / 'nu9ve-core' / 'scripts'))
import nu9ve_sign  # noqa: E402


def install(app, destination):
    app, destination = Path(app).absolute(), Path(destination).expanduser().absolute()
    if destination.suffix != '.app' or destination.is_symlink():
        raise ValueError('Choose a real .app destination, not a symlink')
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != 'app.nu9ve.media':
        raise ValueError('Not a Nu9ve Media app')
    subprocess.run(['/usr/bin/codesign', '--verify', '--strict', str(app)], check=True)
    processes = subprocess.run(['/bin/ps', '-axo', 'command='], check=True, capture_output=True, text=True).stdout
    # The executable was MediaMenu until 2026-11-26 (F2): an older build still running is refused too.
    if any(any(name in line for name in ('/Contents/MacOS/Nuun', '/Contents/MacOS/MediaMenu')) for line in processes.splitlines() if not line.lstrip().startswith(('python', '/bin/zsh', '/bin/bash'))):
        raise RuntimeError('Close Media from its menu before updating. An active operation will not be interrupted.')
    destination.parent.mkdir(parents=True, exist_ok=True)
    token = uuid.uuid4().hex[:8]
    staged = destination.parent / ('.Nu9veMedia-install-' + token + '.app')
    # Keep the old bundle outside Applications so launchers do not index a duplicate.
    backup = destination.parent.parent / 'Library/Application Support/n-creator-utils/app-backups' / (token + '.app')
    previous = destination
    legacy_names = ['n creator utils.app', 'Nu9ve Media.app'] if destination.name == 'nuun.app' else ['Nu9ve Media.app'] if destination.name == 'n creator utils.app' else []
    for legacy_name in legacy_names:
        legacy = destination.parent / legacy_name
        if not destination.exists() and legacy.exists():
            previous = legacy
            break
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
    if destination.name == 'nuun.app':
        keywords = plistlib.dumps(['nüün', 'nuun', 'nun', 'creator utils'], fmt=plistlib.FMT_BINARY).hex()
        subprocess.run(['/usr/bin/xattr', '-wx', 'com.apple.metadata:kMDItemKeywords', keywords, str(destination)], check=True)
        subprocess.run(['/usr/bin/mdimport', str(destination)], check=False, capture_output=True)
    print('Installed: ' + str(destination))
    print('Build: ' + str(info.get('Nu9veBuildID', 'unknown')))
    if backup.exists(): print('Previous build: ' + str(backup))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--python', default=sys.executable, help='External Python for this local development install')
    parser.add_argument('--destination', default=str(Path.home() / 'Applications/nuun.app'), help='ASCII filename makes nuun searchable; display name is nüün')
    parser.add_argument('--app', help='Install an already-built signed bundle; otherwise rebuild current source')
    # A stable identity (a certificate name or its SHA-1) keeps macOS privacy permissions across rebuilds;
    # ad-hoc signing changes the app's identity with every build, so macOS asks again.
    parser.add_argument('--identity',
                        help='Signing identity for the rebuild (default: $NU9VE_SIGN_IDENTITY, else the only '
                             '«Apple Development» identity in the keychain, else ad-hoc)')
    parser.add_argument('--ad-hoc', action='store_true', help='Sign ad-hoc even if a development identity exists')
    args = parser.parse_args()
    if not args.app:
        # The core's rule (nu9ve_sign.choose_identity): --ad-hoc, --identity, NU9VE_SIGN_IDENTITY, the only
        # «Apple Development» identity in the keychain, else ad hoc. The note shows a SHA-1 prefix, never a name.
        identity, note = nu9ve_sign.choose_identity(explicit=args.identity, ad_hoc=args.ad_hoc)
        args.identity = None if identity == nu9ve_sign.AD_HOC else identity
        print('Signing: ' + note + ('' if args.identity else '; macOS will ask for permissions again'))
    repo = Path(__file__).resolve().parents[1]
    app = Path(args.app).absolute() if args.app else repo / 'dist' / ('Nuun-local-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:6] + '.app')
    if not args.app:
        build = [sys.executable, str(repo / 'scripts/build_media_app.py'), '--output', str(app), '--python', str(Path(args.python).absolute())]
        if args.identity: build += ['--identity', args.identity]
        subprocess.run(build, check=True)
    install(app, args.destination)


if __name__ == '__main__':
    main()
