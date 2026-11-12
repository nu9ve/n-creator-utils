#!/usr/bin/env python3
"""Compare the Rust engine with Python on a real catalog, read only, and time both.

    python3 scripts/engine_shadow.py replay [--state-dir DIR] [--engine BIN] [--repeat N] [--json]
    python3 scripts/engine_shadow.py report [--state-dir DIR] [--days N] [--json]
    python3 scripts/engine_shadow.py enable --engine BIN [--state-dir DIR]   # the app's rpc-serve shadows reads
    python3 scripts/engine_shadow.py disable [--state-dir DIR]

`replay` starts `python -S nu9ve.py media rpc-serve` and `nuun-engine rpc-serve` on the same catalog and asks
both the same reads: every parameterless read, plus `job` and `library` for ids and projects taken from the catalog
itself. Both are asked through their pipes, so the times are comparable (request written → line read); startup is
timed apart. Nothing is written to the catalog or to any media: `rpc-serve` refuses mutations in both engines.
The comparison goes to `<catalog>/shadow/shadow-<date>.ndjson` with source `replay`, next to what the app's shadow
records (media/shadow.py), and `report` summarizes both.
"""
import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

from media.catalog import default_home  # noqa: E402
from media.cli import READ_METHODS, READ_WHEN_EMPTY  # noqa: E402
from media.shadow import LOG_DIR, ShadowLog, compare, summarize  # noqa: E402

DEFAULT_ENGINE = REPO / 'engine' / 'target' / 'release' / 'nuun-engine'
# Reads that need parameters the replay cannot invent; they are covered by the app's shadow when it asks them.
NEEDS_PARAMS = {'project_preview', 'closure_candidates', 'closure_deliveries', 'closure_link_candidates',
                'closure_progress', 'action_progress', 'job', 'suggest', 'asset_revisions', 'storage_progress',
                'storage_tree', 'missions_review'}


class Pipe:
    def __init__(self, argv, env=None):
        started = time.perf_counter()
        self.process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        text=True, encoding='utf-8', bufsize=1, env=env, cwd=str(REPO))
        self.sequence = 0
        # Startup: until the first answer (recipe is cheap in both).
        self.ask('recipe', {})
        self.startup = time.perf_counter() - started

    def ask(self, method, params):
        self.sequence += 1
        started = time.perf_counter()
        self.process.stdin.write(json.dumps({'id': self.sequence, 'method': method, 'params': params}) + '\n')
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        elapsed = time.perf_counter() - started
        if not line:
            raise RuntimeError('%s closed: %s' % (self.process.args[0], self.process.stderr.read()[-500:]))
        return json.loads(line), elapsed

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()


def requests_for(python):
    """The reads to replay: parameterless ones, then ids and projects found in this catalog."""
    requests = [(m, {}) for m in sorted(READ_METHODS - NEEDS_PARAMS)] + [(m, {}) for m in sorted(READ_WHEN_EMPTY)]
    jobs, _ = python.ask('jobs', {})
    for job in (jobs.get('data') or [])[:5]:
        if isinstance(job, dict) and job.get('id'):
            requests.append(('job', {'job_id': job['id']}))
    projects, _ = python.ask('projects', {})
    names = [p.get('name') for p in (projects.get('data') or []) if isinstance(p, dict) and p.get('name')]
    for name in names[:3]:
        requests.append(('library', {'project': name}))
    requests.append(('library', {'work_state': 'pending'}))
    for project in (projects.get('data') or [])[:3]:
        if isinstance(project, dict) and project.get('project_id'):
            requests.append(('closure_link_candidates', {'project_id': project['project_id']}))
    library, _ = python.ask('library', {})
    for asset in (library.get('data') or [])[:5]:
        if isinstance(asset, dict) and asset.get('id'):
            requests.append(('asset_revisions', {'asset_id': asset['id']}))
    for name in ('Terra 6 · ñandú', 'nuevo proyecto'):
        requests.append(('project_preview', {'name': name}))
    requests.append(('closure_progress', {'plan_id': 'shadow-absent'}))
    requests.append(('action_progress', {'id': 'shadow-absent'}))
    return requests


def replay(args):
    home = Path(args.state_dir).expanduser() if args.state_dir else default_home()
    engine = Path(args.engine) if args.engine else DEFAULT_ENGINE
    if not engine.is_file():
        print('No engine at %s: cd engine && cargo build --release --offline' % engine, file=sys.stderr)
        return 2
    env = dict(os.environ, NUUN_SHADOW_ENGINE='off')
    python = Pipe([sys.executable, '-S', 'nu9ve.py', 'media', '--state-dir', str(home), 'rpc-serve'], env=env)
    rust = Pipe([str(engine), '--state-dir', str(home), 'rpc-serve'])
    log = ShadowLog(home)
    entries = []
    try:
        for method, params in requests_for(python):
            for _ in range(args.repeat):
                theirs, python_seconds = python.ask(method, params)
                mine, rust_seconds = rust.ask(method, params)
                entry = {'at': time.strftime('%Y-%m-%dT%H:%M:%S'), 'source': 'replay',
                         'python_ms': round(python_seconds * 1000, 2), 'rust_ms': round(rust_seconds * 1000, 2)}
                entry.update(compare(method, theirs, mine))
                if params:
                    entry['params'] = sorted(params)
                entries.append(entry)
                log.write(entry)
    finally:
        python.close()
        rust.close()
    summary = {'catalog': str(home), 'engine': str(engine),
               'startup_ms': {'python': round(python.startup * 1000, 1), 'rust': round(rust.startup * 1000, 1)},
               'methods': summarize(entries),
               'diffs': [e for e in entries if e['outcome'] in {'diff', 'error_mismatch'}][:args.show]}
    print_summary(summary, args.json)
    return 0


def report(args):
    home = Path(args.state_dir).expanduser() if args.state_dir else default_home()
    folder = home / LOG_DIR
    cutoff = time.strftime('%Y%m%d', time.localtime(time.time() - args.days * 86400))
    entries = []
    for path in sorted(folder.glob('shadow-*.ndjson')) if folder.is_dir() else []:
        if path.stem.split('-', 1)[1] >= cutoff:
            entries += [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines() if line.strip()]
    summary = {'catalog': str(home), 'entries': len(entries), 'methods': summarize(entries),
               'diffs': [e for e in entries if e.get('outcome') in {'diff', 'error_mismatch'}][-args.show:]}
    print_summary(summary, args.json)
    return 0


def print_summary(summary, as_json):
    if as_json:
        print(json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True))
        return
    print('catalog:', summary['catalog'])
    if 'startup_ms' in summary:
        print('startup ms: python %(python)s · rust %(rust)s' % summary['startup_ms'])
    print('%-26s %5s  %-44s %10s %10s' % ('method', 'n', 'outcomes', 'py ms', 'rust ms'))
    for method, row in summary['methods'].items():
        outcomes = ', '.join('%s %d' % kv for kv in sorted(row['outcomes'].items()))
        print('%-26s %5d  %-44s %10s %10s' % (method, row['count'], outcomes, row['python_ms_median'], row['rust_ms_median']))
    for entry in summary['diffs']:
        print('\n%s (%s)' % (entry['method'], entry['outcome']))
        for diff in entry.get('diffs', [])[:8]:
            print('  %s\n    python: %s\n    rust:   %s' % (diff['path'], diff['python'], diff['rust']))
        if entry.get('errors'):
            print('  errors:', entry['errors'], entry.get('rust_message', ''))


def set_enabled(args, enabled):
    home = Path(args.state_dir).expanduser() if args.state_dir else default_home()
    path = home / 'shadow.json'
    if enabled:
        engine = Path(args.engine).expanduser().resolve()
        if not engine.is_file():
            print('No engine at %s' % engine, file=sys.stderr)
            return 2
        path.write_text(json.dumps({'enabled': True, 'engine': str(engine)}, indent=2) + '\n', encoding='utf-8')
        print('Shadow on: the next rpc-serve (reopen nüün) compares reads with %s' % engine)
    else:
        if path.exists():
            path.unlink()
        print('Shadow off')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('replay', 'report', 'enable', 'disable'):
        command = sub.add_parser(name)
        command.add_argument('--state-dir')
        if name in {'replay', 'enable'}:
            command.add_argument('--engine', required=name == 'enable')
        if name in {'replay', 'report'}:
            command.add_argument('--json', action='store_true')
            command.add_argument('--show', type=int, default=10, help='differing answers to print')
        if name == 'replay':
            command.add_argument('--repeat', type=int, default=3)
        if name == 'report':
            command.add_argument('--days', type=int, default=7)
    args = parser.parse_args()
    if args.command == 'replay':
        return replay(args)
    if args.command == 'report':
        return report(args)
    return set_enabled(args, args.command == 'enable')


if __name__ == '__main__':
    sys.exit(main())
