#!/usr/bin/env python3
"""List UI strings missing from a translation table (keys are the Spanish source text).

The Swift compiler extracts every LocalizedStringKey/String(localized:) literal; nothing is guessed.
"""
import argparse
import glob
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--language', default='en')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    package = repo / 'native/MediaMenu'
    with tempfile.TemporaryDirectory(prefix='nuun-strings-') as folder:
        for source in (package / 'Sources').rglob('*.swift'):
            source.touch()  # incremental builds skip unchanged files and their strings
        subprocess.run(['swift', 'build', '--package-path', str(package), '-Xswiftc', '-emit-localized-strings',
                        '-Xswiftc', '-emit-localized-strings-path', '-Xswiftc', folder], check=True, capture_output=True)
        keys = set()
        for path in glob.glob(folder + '/*.stringsdata'):
            for entries in json.load(open(path)).get('tables', {}).values():
                keys.update(entry['key'] for entry in entries)
    table = package / 'Localization' / (args.language + '.lproj') / 'Localizable.strings'
    translated = plistlib.loads(subprocess.run(['/usr/bin/plutil', '-convert', 'xml1', '-o', '-', str(table)],
                                               check=True, capture_output=True).stdout)
    missing, unused = sorted(keys - set(translated)), sorted(set(translated) - keys)
    print(json.dumps({'language': args.language, 'keys': len(keys), 'missing': missing, 'unused': unused}, ensure_ascii=False, indent=2))
    return 1 if missing else 0


if __name__ == '__main__':
    sys.exit(main())
