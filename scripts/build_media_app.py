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

# Signing is the suite's (nu9ve/core scripts/nu9ve_sign.py, practices §2.1), from the pinned copy. No __pycache__ in
# vendor/: sync-core.sh --check would refuse it.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'vendor' / 'nu9ve-core' / 'scripts'))
import nu9ve_sign  # noqa: E402


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, help='New .app path; an existing output is never removed')
    parser.add_argument('--configuration', choices=['debug', 'release'], default='release')
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--python', help='Explicit Python executable for development only')
    parser.add_argument('--runtime', help='Relocatable Python runtime root containing bin/python3 and its license')
    parser.add_argument('--identity', help='Signing identity (Apple Development or Developer ID, by name or SHA-1); ad-hoc if omitted')
    parser.add_argument('--no-engine', action='store_true', help='Leave the Rust engine out: Python answers every read')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    output = Path(args.output).absolute()
    if output.exists() or output.is_symlink(): parser.error('Output exists; choose a new artifact path')
    if bool(args.python) == bool(args.runtime): parser.error('Choose --python for development or --runtime for a bundled build')
    if not args.skip_build:
        subprocess.run(['swift', 'build', '--package-path', str(repo / 'native/Nuun'), '-c', args.configuration], check=True)
    result = subprocess.run(['swift', 'build', '--package-path', str(repo / 'native/Nuun'), '-c', args.configuration, '--show-bin-path'],
                            check=True, capture_output=True, text=True)
    binary = Path(result.stdout.strip()) / 'Nuun'
    if not binary.is_file(): parser.error('Build the native executable first')
    contents = output / 'Contents'
    executable_dir = contents / 'MacOS'; executable_dir.mkdir(parents=True)
    resources = contents / 'Resources'; resources.mkdir()
    with tempfile.TemporaryDirectory(prefix='creator-icon-') as temporary:
        iconset = Path(temporary) / 'Creator.iconset'
        subprocess.run(['swift', str(repo / 'scripts/media_icon.swift'), str(iconset)], check=True)
        subprocess.run(['/usr/bin/iconutil', '-c', 'icns', str(iconset), '-o', str(resources / 'Creator.icns')], check=True)
    shutil.copy2(binary, executable_dir / 'Nuun')
    # Spanish is the development language (keys); other languages ship as .lproj tables.
    for table in sorted((repo / 'native/Nuun/Localization').glob('*.lproj')):
        shutil.copytree(table, resources / table.name)
    backend = resources / 'backend'; source = backend / 'source'; source.mkdir(parents=True)
    for name in ['nu9ve.py', 'configuration.py', 'constants.py']:
        shutil.copy2(repo / name, source / name)
    for name in ['media', 'manager']:
        shutil.copytree(repo / name, source / name, ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
    engine = None
    if not args.no_engine:
        # The Rust engine answers reads first; Python answers what it does not have and every write (PLAN, paso 7).
        subprocess.run(['cargo', 'build', '--release', '--offline', '--manifest-path', str(repo / 'engine/Cargo.toml')], check=True)
        engine = backend / 'nuun-engine'
        shutil.copy2(repo / 'engine/target/release/nuun-engine', engine)
        engine.chmod(0o755)
        subprocess.run([str(engine), '--version'], check=True, capture_output=True)
        # The suite agent's binary (nu9ve-agent-v1), from the pinned core and engine/Cargo.lock. The app only runs
        # `nu9ve agent state` and, when the person asks in Settings, `agent install --by nuun`; never `serve`.
        subprocess.run(['cargo', 'build', '--release', '--offline', '--manifest-path', str(repo / 'engine/Cargo.toml'),
                        '-p', 'nu9ve-mcp', '--bin', 'nu9ve'], check=True)
        agent = backend / 'nu9ve'
        shutil.copy2(repo / 'engine/target/release/nu9ve', agent)
        agent.chmod(0o755)
        subprocess.run([str(agent), '--version'], check=True, capture_output=True)
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
    inputs = [repo / name for name in ['nu9ve.py', 'configuration.py', 'constants.py', 'scripts/build_media_app.py', 'scripts/media_icon.swift',
                                        'native/Nuun/Package.swift', 'vendor/nu9ve-core/swift/Package.swift']]
    for folder in ['media', 'manager', 'native/Nuun/Sources', 'native/Nuun/Localization',
                   'vendor/nu9ve-core/swift/Sources/Nu9veTheme']:
        inputs.extend(p for p in (repo / folder).rglob('*') if p.suffix in {'.py', '.swift', '.strings', '.stringsdict'})
    if engine is not None:
        inputs.extend(p for p in (repo / 'engine').rglob('*') if 'target' not in p.relative_to(repo / 'engine').parts
                      and (p.suffix in {'.rs', '.json'} or p.name in {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'}))
        inputs.extend(p for p in (repo / 'vendor/nu9ve-core/rust').rglob('*') if p.suffix == '.rs' or p.name == 'Cargo.toml')
    for path in sorted(inputs):
        fingerprint.update(str(path.relative_to(repo)).encode()); fingerprint.update(b'\0'); fingerprint.update(path.read_bytes())
    build_id = fingerprint.hexdigest()[:12]
    info = {'CFBundleIdentifier': 'app.nu9ve.media', 'CFBundleName': 'nüün', 'CFBundleDisplayName': 'nüün',
            'CFBundleExecutable': 'Nuun', 'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': '0.2.0',
            'CFBundleVersion': '1', 'LSMinimumSystemVersion': '15.0', 'CFBundleDevelopmentRegion': 'es', 'CFBundleLocalizations': ['es', 'en'], 'LSUIElement': True,
            'CFBundleIconFile': 'Creator.icns', 'LSApplicationCategoryType': 'public.app-category.productivity',
            'NSHumanReadableCopyright': '© 2026 nu9ve',
            'LSHasLocalizedDisplayName': True, 'Nu9veSearchAliases': ['nuun', 'nun', 'creator utils'],
            'NSHighResolutionCapable': True, 'NSRemovableVolumesUsageDescription': 'Respaldar y organizar media de tus tarjetas y discos elegidos.',
            'Nu9veBuildID': build_id, 'Nu9veBuildDate': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
    identity = args.identity or nu9ve_sign.AD_HOC
    # Inside out, as the core signs: every Mach-O of the bundled runtime, then the engine and the agent, then the app;
    # hardened runtime, then `codesign --verify --deep --strict`.
    inner = [str(path.relative_to(output)) for path in (engine, agent if engine is not None else None) if path is not None]
    nested = [str((backend / 'runtime').relative_to(output))] if args.runtime else []
    nu9ve_sign.sign(output, identity, inner=inner, nested=nested)
    print(json.dumps({'app': str(output), 'bundled_runtime': bool(args.runtime), 'rust_engine': engine is not None, 'suite_agent': engine is not None,
                      # A stable identity may be Apple Development, which is not Developer ID: only a name says so.
                      'signed_with_identity': bool(args.identity),
                      'developer_id_signed': 'Developer ID' in (args.identity or ''),
                      'notarized': False, 'release_ready': False, 'remaining_gates': ['camera/device validation', 'GUI acceptance', 'notarization and Gatekeeper acceptance']}, indent=2))


if __name__ == '__main__': main()
