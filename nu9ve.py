"""CLI entry point; optional modules are loaded only by their command."""

import argparse
import json
import logging
import shutil
import subprocess
import sys


COMMANDS = {'save', 'clip', 'download', 'split', 'doctor', 'control'}


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
        'clipper_font_exists': Path(SETTINGS['clipper_font_file']).is_file(),
    }
    print(json.dumps(report, indent=2, ensure_ascii=False))


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description='Importar, organizar y recortar media creativa.')
    parser.add_argument('command', choices=sorted(COMMANDS), nargs='?',
                        help='save / clip / download / split / doctor; control es un prototipo sin conectar')
    # Keep each command's --help with its own parser.
    if not args or args[0] in {'-h', '--help'}:
        parser.print_help()
        return 0
    command = parser.parse_args(args[:1]).command
    logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
    try:
        if command == 'control':
            parser.error('control no está conectado al CLI; consulta docs/modules.md')
        elif command == 'doctor':
            parser.parse_args(args)  # reject unused flags
            doctor()
        elif command == 'clip':
            from clipper.clipper_cli import clipper
            clipper(args[1:])
        else:
            from manager.file import save_files, downloader, splitter
            {'save': save_files, 'download': downloader, 'split': splitter}[command](args[1:])
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, ImportError) as error:
        logging.getLogger('nu9ve').error('%s', error)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
