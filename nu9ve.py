"""CLI entry point; optional modules are loaded only by their command."""

import argparse
import json
import logging
import shutil
import subprocess
import sys


COMMANDS = {'save', 'inspect', 'download', 'split', 'doctor', 'control', 'media', 'legacy-save'}


# F5 (c), 2026-11-27: the read orders other apps send (docs/platforms/media-cli-callers.md) go to the Rust engine,
# `nuun-engine media <order>`, which answers the same values (tests/test_media_cli_rust.py). Everything else, and
# any order Rust refuses as usage (exit 2: a usage error, or an engine older than its `media` CLI), stays in Python.
# `NUUN_MEDIA_ENGINE=python` keeps Python (the oracle tests); `NUUN_ENGINE` names the engine, as for Stalker.
# F5 (d): `media mcp` too, by handing the process to the engine.
RUST_ORDERS = {'projects', 'sources', 'volumes', 'resolve', 'captures', 'integration-capabilities', 'retitle-operations'}


def rust_engines():
    """Candidate engines: only `NUUN_ENGINE` when it is set; otherwise, nearest first, the one beside this file in
    the app bundle, this checkout's builds and the installed app."""
    import os
    from pathlib import Path

    here = Path(__file__).resolve().parent
    named = os.environ.get('NUUN_ENGINE')
    if named:  # the one named, and only it
        candidates = [Path(named)]
    else:
        # nuun.app/Contents/Resources/backend/{source/nu9ve.py,nuun-engine}, then this checkout's builds.
        candidates = [here.parent / 'nuun-engine',
                      here / 'engine' / 'target' / 'release' / 'nuun-engine',
                      here / 'engine' / 'target' / 'debug' / 'nuun-engine']
        candidates += [Path(root).expanduser() / 'nuun.app' / 'Contents' / 'Resources' / 'backend' / 'nuun-engine'
                       for root in ('~/Applications', '/Applications')]
    return [path for path in candidates if path.is_file() and os.access(path, os.X_OK)]


def media_order(args):
    """The order of `media [--state-dir D] <order> …`, or None when the arguments are not the plain form Rust reads."""
    rest = list(args)
    while rest[:1] == ['--state-dir'] and len(rest) >= 2:
        rest = rest[2:]
    if not rest or any(arg in {'-h', '--help'} or arg.startswith('--state-dir') for arg in rest):
        return None
    return rest[0]


def rust_media(args):
    """Answer `media …` with the Rust engine; None when Python should (not a Rust order, no engine, or exit 2)."""
    import os

    if os.environ.get('NUUN_MEDIA_ENGINE') == 'python':
        return None
    if media_order(args) == 'mcp':
        # F5 (d): the MCP stdio server is a conversation, so the engine takes over this process. An engine that
        # answers an empty conversation without a usage error (exit 2) has it.
        for engine in rust_engines():
            try:
                probe = subprocess.run([str(engine), 'media', *args], input=b'', capture_output=True, timeout=10)
            except (OSError, subprocess.SubprocessError):
                continue
            if probe.returncode != 2:
                sys.stdout.flush()
                os.execv(str(engine), [str(engine), 'media', *args])
        return None
    if media_order(args) not in RUST_ORDERS:
        return None
    for engine in rust_engines():
        try:
            child = subprocess.run([str(engine), 'media', *args], capture_output=True)
        except OSError:
            continue
        if child.returncode == 2:
            continue
        sys.stdout.buffer.write(child.stdout)
        sys.stdout.buffer.flush()
        sys.stderr.buffer.write(child.stderr)
        return child.returncode
    return None


def doctor():
    """Print machine-readable local diagnostics without touching media."""
    from constants import SETTINGS
    from pathlib import Path

    report = {
        'settings': SETTINGS,
        'executables': {name: shutil.which(name) for name in (
            'ffmpeg', 'ffprobe', 'SetFile', 'yt-dlp', 'scdl', 'demucs')},
        'directories': {key: Path(SETTINGS[key]).is_dir() for key in (
            'root_dir', 'sd_root_dir', 'root_recordings_dir')},
    }
    print(json.dumps(report, indent=2, ensure_ascii=False))


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description='Inspeccionar, importar y organizar media creativa.')
    parser.add_argument('command', choices=sorted(COMMANDS), nargs='?',
                        help='inspect / save / download / split / doctor; control es un prototipo sin conectar')
    # Keep each command's --help with its own parser.
    if not args or args[0] in {'-h', '--help'}:
        parser.print_help()
        return 0
    if args[0] == 'clip':
        print('nu9ve clip fue retirado. Usa el editor independiente: '
              'https://github.com/nu9ve/clipper', file=sys.stderr)
        return 2
    command = parser.parse_args(args[:1]).command
    logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
    try:
        if command == 'media':
            forwarded = rust_media(args[1:])
            if forwarded is not None:
                return forwarded
            from media.cli import main as media_main
            return media_main(args[1:])
        elif command == 'save':
            from media.save import main as safe_save
            return safe_save(args[1:])
        elif command == 'control':
            parser.error('control no está conectado al CLI; consulta docs/modules.md')
        elif command == 'doctor':
            parser.parse_args(args)  # reject unused flags
            doctor()
        elif command == 'inspect':
            from manager.inventory import inspect_command
            inspect_command(args[1:])
        else:
            from manager.file import save_files, downloader, splitter
            {'legacy-save': save_files, 'download': downloader, 'split': splitter}[command](args[1:])
    except KeyboardInterrupt:
        print('Cancelado; revisa el job para reanudar.', file=sys.stderr)
        return 130
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, ImportError) as error:
        logging.getLogger('nu9ve').error('%s', error)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
