#!/usr/bin/env python3
"""Writes data/bench-examples-v1.json, the examples of contracts/nu9ve-bench-v1.md.

Runs go as JSON *text*, so each language reads them with its own parser. Every example passes through the
reference in scripts/nu9ve_bench.py while this file is written, so an example that contradicts it is never
written. The Atlas section is the translation of data/bench-fixtures/atlas by scripts/bench-from-atlas.py.

    python3 -I scripts/bench-examples.py          rewrite the file
    python3 -I scripts/bench-examples.py --check  fail if the file is not what this script writes
"""

import copy
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True  # no __pycache__ next to the script, nor in an app's vendor/ copy
sys.path.insert(0, str(HERE))
import nu9ve_bench as bench  # noqa: E402

ROOT = HERE.parent
OUT = ROOT / 'data' / 'bench-examples-v1.json'
ATLAS_FIXTURE = ROOT / 'data' / 'bench-fixtures' / 'atlas'

MACHINE = {'model': 'MacBookAir10,1', 'chip': 'Apple M1', 'cpu_cores': 8, 'memory_bytes': 17179869184,
           'os': 'macOS 26.1', 'arch': 'aarch64'}
INPUT = {'name': 'fixture/project-a', 'sha256': 'a' * 64, 'bytes': 48213}


def base(engine='rust', samples=(41.2, 40.8, 41.0, 43.9, 40.9), run_id='3f6c0a7e9b1d4c2a8e5f0b6d7c9a1e3f', **changes):
    run = {
        'contract': 'nu9ve-bench/1',
        'run_id': run_id,
        'at': '2026-11-21T18:04:05Z',
        'suite': 'clipper-engine',
        'case': 'reads/project-summary',
        'category': 'read',
        'subject': {'app': 'clipper', 'engine': engine, 'version': '0.9.0', 'commit': 'c856924a', 'dirty': False},
        'machine': dict(MACHINE),
        'inputs': [dict(INPUT)],
        'params': {'mode': 'process'},
        'cache': 'warm',
        'repeats': len(samples),
        'warmup': 1,
        'status': 'ok',
        'metrics': {
            'wall_ms': bench.distribution(list(samples)),
            'peak_memory': {'bytes': 23592960, 'measure': 'rss', 'scope': 'process'},
        },
        'quality': [{'metric': 'response_bytes_equal', 'value': True, 'reference': 'python', 'limit': True, 'op': 'eq', 'pass': True}],
        'privacy': 'public',
        'source': {'kind': 'measured', 'tool': 'nu9ve-bench 0.1.0'},
    }
    for key, value in changes.items():
        if value is None:
            run.pop(key, None)
        else:
            run[key] = value
    return run


def edit(run, path, value):
    """Copy of `run` with the value at `path` (keys and indexes) replaced, or removed when value is DELETE."""
    run = copy.deepcopy(run)
    target = run
    for key in path[:-1]:
        target = target[key]
    if value is DELETE:
        del target[path[-1]]
    else:
        target[path[-1]] = value
    return run


DELETE = object()


def quality_run(run_id='0' * 31 + '3', auc=0.81):
    """§6.1: a model milestone measured by its quality only, with no time."""
    return base(run_id=run_id, at='2026-11-20', suite='atlas-quality', case='evidence/m3', category='quality',
                subject={'app': 'atlas', 'engine': 'python', 'version': None, 'commit': 'e702ac2', 'dirty': False},
                inputs=[], params={'iteration': 3}, cache='unknown', repeats=0, warmup=0, metrics=None,
                quality=[{'metric': 'auc', 'value': auc, 'reference': None, 'pass': None},
                         {'metric': 'adopted', 'value': True, 'reference': None, 'pass': None}],
                privacy='restricted', source={'kind': 'imported', 'origin': 'atlas analysis/history/evolucion.json#evidence/m3'})


def soak_run(run_id='0' * 31 + '5'):
    """§5.2: 2000 requests to an engine after 200 of warmup, with its memory and CPU sampled every 50 ms."""
    rss = [41943040 + 4096 * i for i in range(12)]
    cpu = [0, 40, 81, 120, 162, 201, 240, 282, 321, 360, 401, 433]
    return base(run_id=run_id, suite='nove-engine', case='soak/app-shape', category='soak',
                subject={'app': 'nove', 'engine': 'rust', 'version': '0.12.0', 'commit': '995a1b4', 'dirty': False},
                params={'iterations': 2000, 'warmup_iterations': 200, 'interval_ms': 50}, repeats=1, warmup=0,
                metrics={'wall_ms': bench.distribution([540.5]), 'cpu_ms': bench.distribution([402.0]),
                         'peak_memory': {'bytes': rss[-1], 'measure': 'rss', 'scope': 'process'},
                         'throughput': {'value': 3700.277520814061, 'unit': 'iter/s'},
                         'resources': {'interval_ms': 50, 'rss_bytes': rss, 'cpu_ms': cpu}},
                quality=[{'metric': bench.SOAK_SLOPE, 'value': 20.48, 'reference': None, 'limit': 64.0, 'op': 'le', 'pass': True},
                         {'metric': bench.SOAK_GROWTH, 'value': 40960, 'reference': None, 'pass': None}])


def valid_runs():
    ok = base()
    with_cpu = base()
    with_cpu['metrics']['cpu_ms'] = bench.distribution([39.0, 38.7, 38.9, 41.2, 38.8])
    with_cpu['metrics']['temp_disk_bytes'] = {'bytes': 0}
    with_cpu['metrics']['throughput'] = {'value': 1250.5, 'unit': 'frames/s'}
    unknown_machine = base(machine={'model': None, 'chip': None, 'cpu_cores': None, 'memory_bytes': None, 'os': None, 'arch': None})
    failed = base(status='failed', repeats=2, samples=(40.0, 41.0),
                  error={'code': 'internal_error', 'message': 'the engine closed the pipe'}, quality=None)
    failed_bare = base(status='failed', repeats=0, metrics=None, quality=None,
                       error={'code': 'exit_status', 'message': 'exit status 1'})
    not_ported = base(status='not_ported', repeats=0, warmup=0, metrics=None, quality=None)
    skipped = base(status='skipped', repeats=0, warmup=0, metrics=None, quality=None, note='Docker is not running')
    day = base(at='2026-11-13', source={'kind': 'imported', 'origin': 'atlas config/bench.json#baselines/0'},
               subject={'app': 'atlas', 'engine': 'odm-docker', 'version': None, 'commit': None, 'dirty': None})
    one = base(samples=(1260000,))
    no_limit = base(quality=[{'metric': 'days_offset_max_m', 'value': 0.56, 'reference': None, 'pass': None}])
    tolerance = base(quality=[{'metric': 'max_abs_diff', 'value': 3e-6, 'reference': 'golden', 'limit': 1e-5, 'op': 'le', 'pass': True},
                              {'metric': 'lufs', 'value': -14.2, 'reference': 'golden', 'limit': -14.0, 'op': 'ge', 'pass': False}])
    future = base(future_field={'x': 1})
    personal = base(privacy='personal', inputs=[], params=None)
    empty = base(inputs=[], params={})
    return [
        ('measured run', ok),
        ('with CPU, disk and throughput', with_cpu),
        ('machine unknown', unknown_machine),
        ('failed with what was measured', failed),
        ('failed before measuring', failed_bare),
        ('not ported', not_ported),
        ('skipped with a note', skipped),
        ('imported with a civil date', day),
        ('one sample', one),
        ('quality without a limit', no_limit),
        ('quality with tolerances, one failing', tolerance),
        ('a field the reader does not know is ignored', future),
        ('personal, no inputs', personal),
        ('no inputs and empty params', empty),
        ('quality only, without time (minor version)', quality_run()),
        ('soak with resources, slope and growth (minor version)', soak_run()),
        ('a soak whose memory grows too fast fails its limit, still valid',
         edit(edit(soak_run(), ['quality', 0, 'value'], 812.5), ['quality', 0, 'pass'], False)),
        ('a soak that shrank: negative growth', edit(soak_run(), ['quality', 1, 'value'], -8192)),
        ('a failed soak needs no resources', base(category='soak', status='failed', repeats=0, warmup=0, metrics=None, quality=None,
                                                  error={'code': 'step_failed', 'message': 'iteration 12 failed: BrokenPipe'})),
        ('resources on a run that is not a soak', edit(base(), ['metrics', 'resources'], {'interval_ms': 250, 'rss_bytes': [1024], 'cpu_ms': [0]})),
    ]


def invalid_runs():
    ok = base()
    summary = edit(ok, ['metrics', 'wall_ms', 'median'], 41.2)
    p95_wrong = edit(ok, ['metrics', 'wall_ms', 'p95'], 41.2)
    return [
        ('not an object', [], 'not_object'),
        ('missing run_id', edit(ok, ['run_id'], DELETE), 'missing_field'),
        ('repeats as a string', edit(ok, ['repeats'], '5'), 'wrong_type'),
        ('repeats as a decimal', edit(ok, ['repeats'], 5.0), 'wrong_type'),
        ('integer beyond 2^53 - 1', edit(ok, ['inputs', 0, 'bytes'], 2 ** 53), 'bad_number'),
        ('another major version', edit(ok, ['contract'], 'nu9ve-bench/2'), 'unknown_contract'),
        ('run_id upper case', edit(ok, ['run_id'], '3F6C0A7E9B1D4C2A8E5F0B6D7C9A1E3F'), 'bad_run_id'),
        ('time with an offset', edit(ok, ['at'], '2026-11-21T11:04:05-07:00'), 'bad_time'),
        ('suite with upper case', edit(ok, ['suite'], 'Clipper-engine'), 'bad_name'),
        ('category with a slash', edit(ok, ['category'], 'read/write'), 'bad_name'),
        ('case with ..', edit(ok, ['case'], 'reads/../secrets'), 'bad_case'),
        ('case with a space', edit(ok, ['case'], 'reads/project summary'), 'bad_case'),
        ('engine with upper case', edit(ok, ['subject', 'engine'], 'Rust'), 'bad_subject'),
        ('commit not hexadecimal', edit(ok, ['subject', 'commit'], 'main'), 'bad_subject'),
        ('subject without dirty', edit(ok, ['subject', 'dirty'], DELETE), 'bad_subject'),
        ('arm64 instead of aarch64', edit(ok, ['machine', 'arch'], 'arm64'), 'bad_machine'),
        ('machine without os', edit(ok, ['machine', 'os'], DELETE), 'bad_machine'),
        ('input hash too short', edit(ok, ['inputs', 0, 'sha256'], 'a' * 63), 'bad_inputs'),
        ('input named by its path', edit(ok, ['inputs', 0, 'name'], '/Users/ana/Movies/boda'), 'bad_inputs'),
        ('params not an object', edit(ok, ['params'], ['mode', 'process']), 'bad_params'),
        ('cache hot', edit(ok, ['cache'], 'hot'), 'bad_cache'),
        ('negative warmup', edit(ok, ['warmup'], -1), 'bad_count'),
        ('status passed', edit(ok, ['status'], 'passed'), 'bad_status'),
        ('ok without metrics', edit(ok, ['metrics'], DELETE), 'missing_metrics'),
        ('ok without wall_ms', edit(ok, ['metrics', 'wall_ms'], DELETE), 'missing_metrics'),
        ('without time outside the quality category', edit(quality_run(), ['category'], 'read'), 'missing_metrics'),
        ('quality category without quality', edit(quality_run(), ['quality'], []), 'missing_metrics'),
        ('not_ported with metrics', edit(ok, ['status'], 'not_ported'), 'unexpected_metrics'),
        ('unknown metric', edit(ok, ['metrics', 'gpu_ms'], {'bytes': 1}), 'bad_metrics'),
        ('negative sample', edit(ok, ['metrics', 'wall_ms', 'samples'], [-1.0, 40.8, 41.0, 43.9, 40.9]), 'bad_metrics'),
        ('memory measure unknown word', edit(ok, ['metrics', 'peak_memory', 'measure'], 'resident'), 'bad_metrics'),
        ('throughput of zero', edit(ok, ['metrics', 'throughput'], {'value': 0, 'unit': 'frames/s'}), 'bad_metrics'),
        ('fewer samples than repeats', edit(ok, ['repeats'], 6), 'samples_mismatch'),
        ('median that is not the median', summary, 'summary_mismatch'),
        ('p95 interpolated instead of nearest rank', p95_wrong, 'summary_mismatch'),
        ('failed without error', edit(edit(ok, ['status'], 'failed'), ['quality'], DELETE), 'missing_error'),
        ('pass that contradicts the limit', edit(ok, ['quality', 0, 'value'], False), 'quality_mismatch'),
        ('op without a limit', edit(ok, ['quality', 0, 'limit'], DELETE), 'bad_quality'),
        ('le on a boolean', edit(ok, ['quality', 0, 'op'], 'le'), 'bad_quality'),
        ('quality without reference', edit(ok, ['quality', 0, 'reference'], DELETE), 'bad_quality'),
        ('privacy private', edit(ok, ['privacy'], 'private'), 'bad_privacy'),
        ('source without tool', edit(ok, ['source'], {'kind': 'measured'}), 'bad_source'),
        ('imported from an absolute path', edit(ok, ['source'], {'kind': 'imported', 'origin': '/Users/ana/atlas/config/bench.json'}), 'bad_source'),
        ('note over 500 characters', edit(ok, ['note'], 'x' * 501), 'bad_note'),
        ('a home path in params', edit(ok, ['params', 'library'], '~/Movies/Clipper'), 'personal_path'),
        ('a Windows path in the note', edit(ok, ['note'], 'read from C:\\Users\\ana\\Videos'), 'personal_path'),
        ('resources with series of different length', edit(soak_run(), ['metrics', 'resources', 'cpu_ms'], [0, 1]), 'bad_metrics'),
        ('resources with CPU going back', edit(soak_run(), ['metrics', 'resources', 'cpu_ms', 5], 100), 'bad_metrics'),
        ('resources with a decimal byte count', edit(soak_run(), ['metrics', 'resources', 'rss_bytes', 0], 1.5), 'bad_metrics'),
        ('resources with an interval of zero', edit(soak_run(), ['metrics', 'resources', 'interval_ms'], 0), 'bad_metrics'),
        ('resources without samples', edit(edit(soak_run(), ['metrics', 'resources', 'rss_bytes'], []), ['metrics', 'resources', 'cpu_ms'], []), 'bad_metrics'),
        ('resources over 10 000 samples', edit(edit(soak_run(), ['metrics', 'resources', 'rss_bytes'], [1] * 10001),
                                               ['metrics', 'resources', 'cpu_ms'], [0] * 10001), 'bad_metrics'),
        ('resources with another key', edit(soak_run(), ['metrics', 'resources', 'footprint_bytes'], [1] * 12), 'bad_metrics'),
        ('soak without resources', edit(soak_run(), ['metrics', 'resources'], DELETE), 'bad_soak'),
        ('soak slope without a limit', edit(edit(edit(soak_run(), ['quality', 0, 'limit'], DELETE), ['quality', 0, 'op'], DELETE),
                                            ['quality', 0, 'pass'], None), 'bad_soak'),
        ('soak slope with ge', edit(edit(soak_run(), ['quality', 0, 'op'], 'ge'), ['quality', 0, 'pass'], False), 'bad_soak'),
        ('soak without growth', edit(soak_run(), ['quality'], [soak_run()['quality'][0]]), 'bad_soak'),
        ('soak growth as a decimal', edit(soak_run(), ['quality', 1, 'value'], 40960.5), 'bad_soak'),
        ('a path in the error message', edit(edit(edit(ok, ['status'], 'failed'), ['quality'], DELETE), ['error'],
                                             {'code': 'io', 'message': '/Users/ana/Movies/boda.mov: no such file'}), 'personal_path'),
    ]


def distributions():
    cases = [[5], [3, 1, 2], [4, 1, 3, 2], [0.1, 0.2], list(range(1, 21)), list(range(1, 22)), [7] * 3,
             [41.2, 40.8, 41.0, 43.9, 40.9], [float(i) for i in range(100, 0, -1)]]
    return [bench.distribution(samples) for samples in cases]


def slopes():
    """§5.2: the slope each language must give, bit for bit."""
    cases = [
        ('flat', [200, 400, 600, 800], [1000, 1000, 1000, 1000]),
        ('one byte per iteration', [200, 400, 600, 800], [1200, 1400, 1600, 1800]),
        ('noisy growth', [200, 251, 310, 377, 430, 498, 560, 2200],
         [41943040, 41947136, 41951232, 41943040, 41959424, 41963520, 41967616, 41975808]),
        ('shrinking', [0, 10, 20], [3000, 2000, 1000]),
        ('one sample has no slope', [200], [1000]),
        ('all at the same iteration', [200, 200, 200], [1, 2, 3]),
    ]
    return [{'name': name, 'iterations': xs, 'rss_bytes': ys, 'slope': bench.memory_slope(xs, ys)} for name, xs, ys in cases]


def reductions():
    """§5.1: how a series longer than the budget is halved, keeping the last sample."""
    def series(n):
        return [{'rss_bytes': 1000 + i, 'cpu_ms': i * 3 - (4 if i == 4 else 0)} for i in range(n)]
    cases = [('fits', 50, series(5), 10), ('halved once', 50, series(11), 6), ('halved twice', 50, series(9), 3),
             ('a rounded-down CPU reading is held', 50, series(6), 10)]
    return [{'name': name, 'interval_ms': interval, 'samples': samples, 'max_samples': most,
             'result': bench.resources(interval, samples, most)} for name, interval, samples, most in cases]


def logs():
    a = base(run_id='0' * 31 + '1')
    b = base(run_id='0' * 31 + '2', engine='python', samples=(80.0, 82.0, 81.0, 90.0, 79.5))
    bad = edit(a, ['cache'], 'hot')
    line = bench.dumps
    cases = [
        ('two runs', line(a) + '\n' + line(b) + '\n'),
        ('half-written last line is ignored', line(a) + '\n' + line(b)[:40]),
        ('empty lines are skipped', '\n' + line(a) + '\n\n' + line(b) + '\n'),
        ('an invalid line is reported and skipped', line(a) + '\n' + line(bad) + '\n' + line(b) + '\n'),
        ('not JSON', line(a) + '\n{"contract": nope}\n'),
        ('the same run_id twice keeps the first', line(a) + '\n' + line(edit(a, ['cache'], 'cold')) + '\n'),
        ('empty log', ''),
    ]
    out = []
    for name, text in cases:
        got = bench.read_log(text)
        out.append({'name': name, 'text': text, 'runs': [r['run_id'] for r in got['runs']], 'problems': got['problems'],
                    'duplicates': got['duplicates'], 'partial_last_line': got['partial_last_line']})
    return out


def comparisons():
    python = base(engine='python', run_id='0' * 31 + '1', samples=(80.0, 82.0, 81.0, 90.0, 79.5))
    rust = base(run_id='0' * 31 + '2')
    other_machine = edit(rust, ['machine', 'chip'], 'Apple M4')
    cold = edit(rust, ['cache'], 'cold')
    dirty = edit(rust, ['subject', 'dirty'], True)
    params = edit(rust, ['params', 'mode'], 'serve')
    other_case = edit(rust, ['case'], 'reads/queue-view')
    other_input = edit(rust, ['inputs', 0, 'sha256'], 'b' * 64)
    failed = base(status='failed', repeats=0, metrics=None, quality=None, error={'code': 'x', 'message': 'y'})
    cases = [
        ('python to rust on the same machine', python, rust),
        ('another machine is flagged', python, other_machine),
        ('cold against warm is flagged', python, cold),
        ('a dirty checkout is flagged', python, dirty),
        ('other params are flagged', python, params),
        ('another case is not comparable', python, other_case),
        ('other inputs are not comparable', python, other_input),
        ('a failed run is not comparable', python, failed),
        ('quality-only runs compare without a speedup', quality_run(), quality_run(run_id='0' * 31 + '4', auc=0.84)),
    ]
    return [{'name': name, 'baseline': bench.dumps(a), 'candidate': bench.dumps(b), 'result': bench.compare(a, b)}
            for name, a, b in cases]


def atlas():
    spec = importlib.util.spec_from_file_location('bench_from_atlas', HERE / 'bench-from-atlas.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    runs = module.translate(ATLAS_FIXTURE)
    again = module.translate(ATLAS_FIXTURE)
    assert [r['run_id'] for r in runs] == [r['run_id'] for r in again]  # translating twice gives the same ids
    return {'fixture': 'data/bench-fixtures/atlas', 'translator': 'scripts/bench-from-atlas.py',
            'runs': [bench.dumps(run) for run in runs]}


def build():
    valid = []
    for name, run in valid_runs():
        text = bench.dumps(run)
        assert bench.line_problem(text) is None, (name, bench.line_problem(text))
        valid.append({'name': name, 'text': text})
    invalid = []
    for name, run, want in invalid_runs():
        text = bench.dumps(run)
        got = bench.line_problem(text)
        assert got == want, (name, got, want)
        invalid.append({'name': name, 'text': text, 'problem': want})
    for name, text, want in [('NaN is not JSON', bench.dumps(base()).replace('41.2', 'NaN', 1), 'not_json'),
                             ('over 64 KiB', bench.dumps(base(note=None, params={'padding': 'x' * bench.MAX_LINE})), 'too_large')]:
        assert bench.line_problem(text) == want, name
        invalid.append({'name': name, 'text': text, 'problem': want})
    codes = {case['problem'] for case in invalid}
    assert len(codes) >= 25, sorted(codes)
    document = {
        'contract': 'contracts/nu9ve-bench-v1.md',
        'regenerate': 'python3 -I scripts/bench-examples.py',
        'note': 'valid and invalid: one run as a line of the log (problem is the first one found, §2 to §9); '
                'distributions: §5 summaries; logs: what a reader keeps (§1); comparisons: §8; '
                'atlas: the translation of the fixture (§10); '
                'slopes: §5.2 memory_slope; reductions: §5.1 halving of a long resources series.',
        'valid': valid,
        'invalid': invalid,
        'distributions': distributions(),
        'logs': logs(),
        'comparisons': comparisons(),
        'slopes': slopes(),
        'reductions': reductions(),
        'atlas': atlas(),
    }
    return json.dumps(document, ensure_ascii=False, indent=2) + '\n'


def main():
    text = build()
    if sys.argv[1:] == ['--check']:
        if not OUT.is_file() or OUT.read_text(encoding='utf-8') != text:
            print('%s is out of date: python3 -I scripts/bench-examples.py' % OUT, file=sys.stderr)
            return 1
        return 0
    if sys.argv[1:]:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    OUT.write_text(text, encoding='utf-8')
    return 0


if __name__ == '__main__':
    sys.exit(main())
