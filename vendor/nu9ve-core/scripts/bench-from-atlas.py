#!/usr/bin/env python3
"""Translates Atlas benchmarks to nu9ve-bench-v1 runs (contracts/nu9ve-bench-v1.md §10). Read-only.

    python3 -I scripts/bench-from-atlas.py <atlas repo>              NDJSON on stdout
    python3 -I scripts/bench-from-atlas.py <atlas repo> --out <log>  appended to a log, skipping runs it has

Reads the run files under derived/bench/<stage>/<dataset>/ and the baselines in config/bench.json. A run
file that imported a baseline (its source starts with "imported") replaces that baseline, matched by
stage, dataset, engine and time_s, so the same measurement is not counted twice. Every run gets the id
the contract derives from its origin, so translating twice gives the same ids.

Since the minor version of 2026-11-24 (§6.1 and §10 of the contract) it also reads:
- analysis/history/evolucion.json: one quality-only run per milestone (suite atlas-quality), dated by the
  milestone, else by its iteration in docs/log/*.md, else by the file's modification day;
- derived/bench/*.ndjson: logs Atlas already writes in the suite format. Their runs come first and win
  over a translated run with the same run_id. Their invalid lines are reported, not fatal.

The Rust translator, nu9ve_bench::atlas, gives the same runs; its tests compare with this script.
"""

import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ next to the script, nor in an app's vendor/ copy
sys.path.insert(0, str(Path(__file__).resolve().parent))
import nu9ve_bench as bench  # noqa: E402

GIB = 2 ** 30  # Atlas memory_gb and peak_memory_gb: hw.memsize / 2**30
GB = 10 ** 9   # Atlas disk_gb: bytes / 1e9
MAX_NOTE = 500


def slug(text):
    """A dataset or engine name of Atlas as a part of `case` (§2)."""
    out = ''.join(c if c.isalnum() or c in '._-' else '-' for c in str(text).lower())
    return out.strip('-.') or 'unknown'


def flatten(value, prefix=''):
    """Atlas quality as one metric per numeric or boolean leaf, with dotted names."""
    if isinstance(value, dict):
        for key, item in value.items():
            yield from flatten(item, prefix + '.' + str(key) if prefix else str(key))
    elif isinstance(value, bool) or (isinstance(value, (int, float)) and value == value):
        yield prefix, value


def commit_of(git):
    """`e702ac2+dirty` → ('e702ac2', True); `e702ac2` → ('e702ac2', False); anything else → (None, None)."""
    if not isinstance(git, str) or not git:
        return None, None
    commit, _, flag = git.partition('+')
    if not bench.COMMIT.fullmatch(commit):
        return None, None
    return commit, flag == 'dirty'


def to_run(entry, origin, imported):
    commit, dirty = commit_of(entry.get('git'))
    atlas_machine = entry.get('machine') or {}
    params = {'dataset': entry.get('dataset')}
    params.update(entry.get('params') or {})
    if 'docker_memory_gb' in atlas_machine:
        params['docker_memory_gb'] = atlas_machine['docker_memory_gb']
    if entry.get('tag'):
        params['tag'] = entry['tag']
    memory_gb = atlas_machine.get('memory_gb')
    run = {
        'contract': bench.CONTRACT,
        'run_id': bench.imported_run_id(origin),
        'at': entry['date'],
        'suite': 'atlas-pipeline',
        'case': slug(entry['stage']) + '/' + slug(entry['dataset']),
        'category': slug(entry['stage']),
        'subject': {'app': 'atlas', 'engine': slug(entry['engine']), 'version': None, 'commit': commit, 'dirty': dirty},
        'machine': {'model': None, 'chip': atlas_machine.get('chip'), 'cpu_cores': atlas_machine.get('cores'),
                    'memory_bytes': int(memory_gb * GIB) if isinstance(memory_gb, (int, float)) else None,
                    'os': atlas_machine.get('os'), 'arch': None},
        'inputs': [],
        'params': params,
        'cache': 'unknown',
    }
    time_s = entry.get('time_s')
    status = entry.get('status', 'ok')
    if status == 'ok' and isinstance(time_s, (int, float)):
        run.update({'repeats': 1, 'warmup': 0, 'status': 'ok'})
        metrics = {'wall_ms': bench.distribution([time_s * 1000])}
        if isinstance(entry.get('peak_memory_gb'), (int, float)):
            measured = entry.get('memory_measure') in ('footprint', 'rss')
            metrics['peak_memory'] = {'bytes': round(entry['peak_memory_gb'] * GIB),
                                      'measure': entry['memory_measure'] if measured else 'unknown', 'scope': 'tree'}
        if isinstance(entry.get('disk_gb'), (int, float)):
            metrics['temp_disk_bytes'] = {'bytes': round(entry['disk_gb'] * GB)}
        run['metrics'] = metrics
    elif status == 'ok':
        run.update({'repeats': 0, 'warmup': 0, 'status': 'skipped'})
    else:
        run.update({'repeats': 0, 'warmup': 0, 'status': 'failed',
                    'error': {'code': 'atlas_' + slug(status), 'message': 'Atlas status: ' + str(status)}})
    quality = [{'metric': name, 'value': value, 'reference': None, 'pass': None}
               for name, value in flatten(entry.get('quality') or {}) if 1 <= len(name) <= 128]
    if quality:
        run['quality'] = quality
    run['privacy'] = 'restricted'  # Atlas photos are the person's; only numbers travel, but not as public
    run['source'] = {'kind': 'imported', 'origin': origin}
    note = 'atlas: ' + ('baseline' if imported == 'baseline' else str(entry.get('source') or 'run'))
    if entry.get('notes'):
        note += '. ' + str(entry['notes'])
    run['note'] = note if len(note) <= MAX_NOTE else note[:MAX_NOTE - 1] + '…'
    return run


QUALITY_KEYS = ('auc', 'auc_candidates', 'auc_flat', 'auc_curb', 'rho_bombs', 'judged', 'adopted')
ITERATION = re.compile(r'## Iteración ([0-9]+) · ([0-9]{4}-[0-9]{2}-[0-9]{2})\b')


def log_dates(root):
    """Iteration number → day, from the headings of Atlas's log (the first heading of each number wins)."""
    dates = {}
    log_dir = root / 'docs' / 'log'
    for path in sorted(log_dir.glob('*.md')) if log_dir.is_dir() else []:
        for line in path.read_text(encoding='utf-8').splitlines():
            match = ITERATION.match(line)
            if match:
                dates.setdefault(int(match.group(1)), match.group(2))
    return dates


def milestone_slug(key):
    """The key of a milestone as the last part of `case`; `+` is spelled out so `auto-v6+` is not `auto-v6`."""
    return slug(str(key).replace('+', '-plus'))


def milestone_run(kind, entry, at):
    key = entry.get('key')
    origin = 'atlas analysis/history/evolucion.json#%s/%s' % (kind, key)
    label = entry.get('label')
    run = {
        'contract': bench.CONTRACT,
        'run_id': bench.imported_run_id(origin),
        'at': at,
        'suite': 'atlas-quality',
        'case': kind + '/' + milestone_slug(key),
        'category': 'quality',
        'subject': {'app': 'atlas', 'engine': 'model', 'version': label if isinstance(label, str) and label else None,
                    'commit': None, 'dirty': None},
        'machine': {'model': None, 'chip': None, 'cpu_cores': None, 'memory_bytes': None, 'os': None, 'arch': None},
        'inputs': [],
        'params': {'iteration': entry.get('iteration')},
        'cache': 'unknown',
        'repeats': 0,
        'warmup': 0,
        'status': 'ok',
        'quality': [{'metric': name, 'value': entry[name], 'reference': None, 'pass': None}
                    for name in QUALITY_KEYS
                    if isinstance(entry.get(name), bool) or isinstance(entry.get(name), (int, float))],
        'privacy': 'restricted',
        'source': {'kind': 'imported', 'origin': origin},
    }
    what = entry.get('what')
    if isinstance(what, str) and what:
        run['note'] = what if len(what) <= MAX_NOTE else what[:MAX_NOTE - 1] + '…'
    return run


def milestones(root):
    path = root / 'analysis' / 'history' / 'evolucion.json'
    if not path.is_file():
        return []
    history = json.loads(path.read_text(encoding='utf-8'))
    dates = log_dates(root)
    modified = datetime.fromtimestamp(path.stat().st_mtime, timezone.utc).date().isoformat()
    runs = []
    for kind in ('evidence', 'gravity'):
        for entry in history.get(kind) or []:
            if not isinstance(entry, dict) or not isinstance(entry.get('key'), str):
                continue
            at = entry.get('date')
            if not (isinstance(at, str) and bench.DAY.fullmatch(at)):
                at = dates.get(entry.get('iteration'), modified)
            run = milestone_run(kind, entry, at)
            if run['quality']:  # a milestone with every metric null measured nothing
                runs.append(run)
    return runs


def suite_logs(root):
    """derived/bench/*.ndjson, read as logs (§1): their runs, and their skipped lines as (where, problem)."""
    runs, problems = [], []
    bench_dir = root / 'derived' / 'bench'
    for path in sorted(bench_dir.glob('*.ndjson')) if bench_dir.is_dir() else []:
        log = bench.read_log(path.read_text(encoding='utf-8'))
        where = path.relative_to(root).as_posix()
        runs += log['runs']
        problems += [('%s:%d' % (where, p['line']), p['problem']) for p in log['problems']]
    return runs, problems


def translate(root):
    """The runs of an Atlas repo; exits on a translated run that is invalid. See translate_with_problems."""
    return translate_with_problems(root)[0]


def translate_with_problems(root):
    root = Path(root)
    written, problems = suite_logs(root)
    runs, covered = [], set()
    bench_dir = root / 'derived' / 'bench'
    for path in sorted(bench_dir.glob('*/*/*.json')) if bench_dir.is_dir() else []:
        entry = json.loads(path.read_text(encoding='utf-8'))
        if not all(k in entry for k in ('stage', 'dataset', 'engine', 'date')):
            continue
        origin = 'atlas ' + path.relative_to(root).as_posix()
        runs.append(to_run(entry, origin, 'run'))
        if str(entry.get('source', '')).startswith('imported'):
            covered.add((entry['stage'], entry['dataset'], entry['engine'], entry.get('time_s')))
    config = root / 'config' / 'bench.json'
    if config.is_file():
        for index, entry in enumerate(json.loads(config.read_text(encoding='utf-8')).get('baselines', [])):
            if (entry.get('stage'), entry.get('dataset'), entry.get('engine'), entry.get('time_s')) in covered:
                continue
            runs.append(to_run(entry, 'atlas config/bench.json#baselines/%d' % index, 'baseline'))
    runs += milestones(root)
    for run in runs:
        problem = bench.run_problem(run)
        if problem:
            raise SystemExit('bench-from-atlas: %s gives an invalid run: %s' % (run['source']['origin'], problem))
    seen, kept = set(), []
    for run in written + runs:  # the first run of each run_id wins (§1): Atlas's own logs before translations
        if run['run_id'] not in seen:
            seen.add(run['run_id'])
            kept.append(run)
    return kept, problems


def main(argv):
    if len(argv) not in (1, 3) or len(argv) == 3 and argv[1] != '--out':
        print(__doc__.strip(), file=sys.stderr)
        return 2
    runs, problems = translate_with_problems(argv[0])
    for where, problem in problems:
        print('bench-from-atlas: skipped %s: %s' % (where, problem), file=sys.stderr)
    if len(argv) == 1:
        for run in runs:
            print(bench.dumps(run))
        return 0
    log = Path(argv[2])
    have = {r['run_id'] for r in bench.read(log)['runs']} if log.is_file() else set()
    for run in runs:
        if run['run_id'] not in have:
            bench.append(log, run)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
