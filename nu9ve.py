"""CLI entry point; optional modules are loaded only by their command."""

import argparse
import json
import logging
import shutil
import subprocess
import sys


COMMANDS = {'save', 'inspect', 'download', 'split', 'doctor', 'control', 'media', 'legacy-save'}


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
