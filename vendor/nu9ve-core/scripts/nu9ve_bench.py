"""nu9ve-bench-v1 in Python: the reference for contracts/nu9ve-bench-v1.md and a helper for the labs.

Standard library only, so an app can use its pinned copy without installing anything:

    import sys
    sys.dont_write_bytecode = True  # a __pycache__ inside vendor/ would fail sync-core.sh --check
    sys.path.insert(0, 'vendor/nu9ve-core/scripts')
    import nu9ve_bench as bench

    run = bench.new_run(
        suite='clipper-engine', case='reads/project-summary', category='read',
        subject=bench.subject('clipper', 'python', '0.9.0', repo='.'),
        inputs=[bench.file_input('fixture/project-a', 'tests/fixtures/project-a/project.json')],
        cache='warm', privacy='public',
        **bench.measure_command(['python3', '-m', 'clipper_tui.cli', 'project-summary'], repeats=5, warmup=1),
    )
    bench.append('bench/clipper-engine.ndjson', run)

A soak (§5.2) repeats one operation while a thread samples a process; its result goes straight into new_run:

    run = bench.new_run(suite='nove-engine', case='soak/app-shape', category='soak', ..., cache='warm',
                        privacy='public', **bench.measure_soak(step, iterations=2000, warmup=200,
                                                               slope_limit=64.0, pid=engine.pid))

`run_problem` is the validator: None when a run follows the contract, otherwise the code of the first
problem. scripts/bench-examples.py writes data/bench-examples-v1.json through it, and the Rust crate
answers the same codes.
"""

import hashlib
import json
import math
import os
import platform
import re
import secrets
import subprocess
import threading
import time
from datetime import datetime, timezone

CONTRACT = 'nu9ve-bench/1'
TOOL = 'nu9ve_bench.py 1'
MAX_LINE = 64 * 1024
MAX_SAFE = 2 ** 53 - 1

NAME = re.compile(r'[a-z][a-z0-9-]{0,63}')
APP = re.compile(r'[a-z][a-z0-9-]{0,31}')
CASE = re.compile(r'[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*')
HEX32 = re.compile(r'[0-9a-f]{32}')
SHA256 = re.compile(r'[0-9a-f]{64}')
COMMIT = re.compile(r'[0-9a-f]{7,40}')
UTC = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,9})?Z')
DAY = re.compile(r'[0-9]{4}-[0-9]{2}-[0-9]{2}')
WINDOWS_PATH = re.compile(r'[A-Za-z]:\\')

REQUIRED = {
    'contract': str, 'run_id': str, 'at': str, 'suite': str, 'case': str, 'category': str,
    'subject': dict, 'machine': dict, 'inputs': list, 'cache': str, 'repeats': int, 'warmup': int,
    'status': str, 'privacy': str, 'source': dict,
}
DISTRIBUTIONS = ('wall_ms', 'cpu_ms', 'startup_ms')
METRICS = DISTRIBUTIONS + ('peak_memory', 'temp_disk_bytes', 'throughput', 'resources')
MAX_RESOURCE_SAMPLES = 10000
SOAK_SLOPE = 'memory_slope_bytes_per_iter'
SOAK_GROWTH = 'rss_end_minus_start_bytes'


# --------------------------------------------------------------------------------------------- values


def is_int(value):
    return isinstance(value, int) and not isinstance(value, bool)


def is_number(value):
    if isinstance(value, bool):
        return False
    if isinstance(value, int):
        return abs(value) <= MAX_SAFE
    return isinstance(value, float) and math.isfinite(value)


def is_count(value):
    return is_int(value) and 0 <= value <= MAX_SAFE


def printable(text):
    """No control characters and no whitespace but the plain space: the same rule in every language."""
    return all(c == ' ' or (ord(c) >= 0x20 and not 0x7f <= ord(c) <= 0x9f and not c.isspace()) for c in text)


def nullable(value, check):
    return value is None or check(value)


def load(text):
    def reject(constant):
        raise ValueError('not JSON: ' + constant)
    return json.loads(text, parse_constant=reject)


def dumps(run):
    """The line as written: compact, keys in the order given, non-ASCII kept."""
    return json.dumps(run, ensure_ascii=False, separators=(',', ':'), allow_nan=False)


# --------------------------------------------------------------------------------------------- §5


def median(samples):
    ordered = sorted(samples)
    n = len(ordered)
    if n % 2:
        return ordered[n // 2]
    return (ordered[n // 2 - 1] + ordered[n // 2]) / 2


def p95(samples):
    ordered = sorted(samples)
    k = (95 * len(ordered) + 99) // 100
    return ordered[k - 1]


def distribution(samples):
    """§5: the summary the contract asks for, from the samples in order."""
    samples = list(samples)
    if not samples:
        raise ValueError('a distribution needs at least one sample')
    return {'samples': samples, 'median': median(samples), 'p95': p95(samples), 'min': min(samples), 'max': max(samples)}


def distribution_problem(value, repeats):
    if not isinstance(value, dict) or not isinstance(value.get('samples'), list):
        return 'bad_metrics'
    samples = value['samples']
    if not all(is_number(s) and s >= 0 for s in samples):
        return 'bad_metrics'
    if len(samples) != repeats or not samples:
        return 'samples_mismatch'
    for key in ('median', 'p95', 'min', 'max'):
        if not is_number(value.get(key)):
            return 'bad_metrics'
    expected = distribution(samples)
    if any(float(value[k]) != float(expected[k]) for k in ('median', 'p95', 'min', 'max')):
        return 'summary_mismatch'
    return None


def resources_ok(value):
    """§5.1: {"interval_ms", "rss_bytes", "cpu_ms"}, integers, 1 to 10 000 samples in both, CPU never going back."""
    if not isinstance(value, dict) or set(value) != {'interval_ms', 'rss_bytes', 'cpu_ms'}:
        return False
    rss, cpu = value['rss_bytes'], value['cpu_ms']
    return (is_count(value['interval_ms']) and value['interval_ms'] >= 1
            and isinstance(rss, list) and isinstance(cpu, list)
            and 1 <= len(rss) <= MAX_RESOURCE_SAMPLES and len(rss) == len(cpu)
            and all(is_count(v) for v in rss) and all(is_count(v) for v in cpu)
            and all(a <= b for a, b in zip(cpu, cpu[1:])))


def metrics_problem(metrics, repeats):
    if not isinstance(metrics, dict) or any(k not in METRICS for k in metrics):
        return 'bad_metrics'
    for key in DISTRIBUTIONS:
        if key in metrics:
            problem = distribution_problem(metrics[key], repeats)
            if problem:
                return problem
    if 'peak_memory' in metrics:
        memory = metrics['peak_memory']
        if (not isinstance(memory, dict) or not is_count(memory.get('bytes'))
                or memory.get('measure') not in ('rss', 'footprint', 'unknown')
                or memory.get('scope') not in ('process', 'tree')):
            return 'bad_metrics'
    if 'temp_disk_bytes' in metrics:
        disk = metrics['temp_disk_bytes']
        if not isinstance(disk, dict) or not is_count(disk.get('bytes')):
            return 'bad_metrics'
    if 'resources' in metrics and not resources_ok(metrics['resources']):
        return 'bad_metrics'
    if 'throughput' in metrics:
        rate = metrics['throughput']
        if not isinstance(rate, dict):
            return 'bad_metrics'
        unit = rate.get('unit')
        if (not is_number(rate.get('value')) or rate['value'] <= 0 or not isinstance(unit, str)
                or not 1 <= len(unit) <= 32 or not printable(unit)):
            return 'bad_metrics'
    return None


# --------------------------------------------------------------------------------------------- §6


def quality_problem(quality):
    if not isinstance(quality, list):
        return 'bad_quality'
    for item in quality:
        if not isinstance(item, dict):
            return 'bad_quality'
        metric, value, reference = item.get('metric'), item.get('value'), item.get('reference', 0)
        if not isinstance(metric, str) or not 1 <= len(metric) <= 128 or not printable(metric):
            return 'bad_quality'
        if not (isinstance(value, bool) or is_number(value)):
            return 'bad_quality'
        if 'reference' not in item or not nullable(reference, lambda r: isinstance(r, str) and len(r) <= 128 and CASE.fullmatch(r)):
            return 'bad_quality'
        if 'pass' not in item or not nullable(item['pass'], lambda p: isinstance(p, bool)):
            return 'bad_quality'
        if 'limit' in item:
            limit, op = item['limit'], item.get('op')
            if isinstance(limit, bool) != isinstance(value, bool) or not (isinstance(limit, bool) or is_number(limit)):
                return 'bad_quality'
            if op not in ('le', 'ge', 'eq') or isinstance(value, bool) and op != 'eq':
                return 'bad_quality'
            passed = value <= limit if op == 'le' else value >= limit if op == 'ge' else value == limit
            if item['pass'] is not passed:
                return 'quality_mismatch'
        elif 'op' in item:
            return 'bad_quality'
    return None


# --------------------------------------------------------------------------------------------- §5.2


def memory_slope(iterations, rss):
    """§5.2: the least-squares slope of `rss` on `iterations` (bytes per iteration), or None with fewer than two
    samples or all at the same iteration. Plain sums in order (not sum(), which compensates since 3.12), so the
    Rust crate gives the same bits."""
    n = len(iterations)
    if n < 2 or len(rss) != n:
        return None
    xs, ys = [float(x) for x in iterations], [float(y) for y in rss]
    total_x = total_y = 0.0
    for x, y in zip(xs, ys):
        total_x += x
        total_y += y
    mean_x, mean_y = total_x / n, total_y / n
    sxy = sxx = 0.0
    for x, y in zip(xs, ys):
        sxy += (x - mean_x) * (y - mean_y)
        sxx += (x - mean_x) * (x - mean_x)
    return sxy / sxx if sxx > 0 else None


def soak_problem(run):
    """§5.2: an `ok` soak carries `resources`, the slope with its limit and `le`, and the growth."""
    if run['category'] != 'soak' or run['status'] != 'ok':
        return None
    if not isinstance(run.get('metrics'), dict) or 'resources' not in run['metrics']:
        return 'bad_soak'
    items = run.get('quality') if isinstance(run.get('quality'), list) else []
    found = {q.get('metric'): q for q in reversed(items) if isinstance(q, dict)}
    slope, growth = found.get(SOAK_SLOPE), found.get(SOAK_GROWTH)
    slope_ok = (slope is not None and is_number(slope.get('value')) and is_number(slope.get('limit'))
                and slope.get('op') == 'le')
    growth_ok = growth is not None and is_int(growth.get('value')) and abs(growth['value']) <= MAX_SAFE
    return None if slope_ok and growth_ok else 'bad_soak'


# --------------------------------------------------------------------------------------------- §9


def leaks_path(text):
    return text.startswith(('/', '~')) or '\\\\' in text or WINDOWS_PATH.search(text) is not None


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for key, item in value.items():
            yield key
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def numbers_ok(value):
    if isinstance(value, dict):
        return all(numbers_ok(v) for v in value.values())
    if isinstance(value, list):
        return all(numbers_ok(v) for v in value)
    if isinstance(value, float):
        return math.isfinite(value)
    if is_int(value):
        return abs(value) <= MAX_SAFE
    return True


# --------------------------------------------------------------------------------------------- §2 to §9


def quality_only(run):
    """§6.1 (minor version of 2026-11-24): an `ok` run of category `quality` with quality and no time."""
    return run.get('category') == 'quality' and isinstance(run.get('quality'), list) and len(run['quality']) > 0


def run_problem(run):
    """None when `run` (a parsed JSON value) follows the contract, otherwise the code of the first problem."""
    if not isinstance(run, dict):
        return 'not_object'
    for name, kind in REQUIRED.items():
        if name not in run:
            return 'missing_field'
        if kind is int and not is_int(run[name]) or kind is not int and not isinstance(run[name], kind):
            return 'wrong_type'
    if not numbers_ok(run):
        return 'bad_number'
    if run['contract'] != CONTRACT:
        return 'unknown_contract'
    if not HEX32.fullmatch(run['run_id']):
        return 'bad_run_id'
    if not (UTC.fullmatch(run['at']) or DAY.fullmatch(run['at'])):
        return 'bad_time'
    if not NAME.fullmatch(run['suite']) or not NAME.fullmatch(run['category']):
        return 'bad_name'
    if len(run['case']) > 128 or not CASE.fullmatch(run['case']):
        return 'bad_case'

    subject = run['subject']
    if (not isinstance(subject.get('app'), str) or not APP.fullmatch(subject['app'])
            or not isinstance(subject.get('engine'), str) or not APP.fullmatch(subject['engine'])
            or any(k not in subject for k in ('version', 'commit', 'dirty'))
            or not nullable(subject['version'], lambda v: isinstance(v, str) and 1 <= len(v) <= 64)
            or not nullable(subject['commit'], lambda c: isinstance(c, str) and COMMIT.fullmatch(c))
            or not nullable(subject['dirty'], lambda d: isinstance(d, bool))):
        return 'bad_subject'

    machine = run['machine']
    text = lambda v: isinstance(v, str) and 1 <= len(v) <= 128
    if (any(k not in machine for k in ('model', 'chip', 'cpu_cores', 'memory_bytes', 'os', 'arch'))
            or not all(nullable(machine[k], text) for k in ('model', 'chip', 'os'))
            or not nullable(machine['arch'], lambda a: a in ('aarch64', 'x86_64'))
            or not nullable(machine['cpu_cores'], lambda c: is_int(c) and c >= 1)
            or not nullable(machine['memory_bytes'], lambda m: is_count(m) and m >= 1)):
        return 'bad_machine'

    for item in run['inputs']:
        if (not isinstance(item, dict) or not isinstance(item.get('name'), str)
                or len(item['name']) > 128 or not CASE.fullmatch(item['name'])
                or not isinstance(item.get('sha256'), str) or not SHA256.fullmatch(item['sha256'])
                or 'bytes' not in item or not nullable(item['bytes'], is_count)):
            return 'bad_inputs'
    if 'params' in run and not isinstance(run['params'], dict):
        return 'bad_params'
    if run['cache'] not in ('cold', 'warm', 'unknown'):
        return 'bad_cache'
    if not is_count(run['repeats']) or not is_count(run['warmup']):
        return 'bad_count'

    status = run['status']
    if status not in ('ok', 'failed', 'not_ported', 'skipped'):
        return 'bad_status'
    if (status == 'ok' and not quality_only(run)
            and ('metrics' not in run or not isinstance(run['metrics'], dict) or 'wall_ms' not in run['metrics'])):
        return 'missing_metrics'
    if status in ('not_ported', 'skipped') and ('metrics' in run or run['repeats'] != 0):
        return 'unexpected_metrics'
    if 'metrics' in run:
        problem = metrics_problem(run['metrics'], run['repeats'])
        if problem:
            return problem
    if status == 'failed':
        error = run.get('error')
        if (not isinstance(error, dict) or not isinstance(error.get('code'), str) or not error['code']
                or not isinstance(error.get('message'), str)):
            return 'missing_error'
    if 'quality' in run:
        problem = quality_problem(run['quality'])
        if problem:
            return problem
    problem = soak_problem(run)
    if problem:
        return problem

    if run['privacy'] not in ('public', 'restricted', 'personal'):
        return 'bad_privacy'
    source = run['source']
    if source.get('kind') == 'measured':
        if not text(source.get('tool')):
            return 'bad_source'
    elif source.get('kind') == 'imported':
        if not isinstance(source.get('origin'), str) or not 1 <= len(source['origin']) <= 512 or leaks_path(source['origin']):
            return 'bad_source'
    else:
        return 'bad_source'
    if 'note' in run and (not isinstance(run['note'], str) or len(run['note']) > 500):
        return 'bad_note'

    exposed = [item['name'] for item in run['inputs']]
    exposed += list(strings(run.get('params', {})))
    exposed += [run.get('note', '')]
    if isinstance(run.get('error'), dict):
        exposed.append(run['error'].get('message', ''))
    if any(isinstance(s, str) and leaks_path(s) for s in exposed):
        return 'personal_path'
    return None


def line_problem(line):
    """A line of the log (without its newline), as a reader sees it."""
    if len(line.encode('utf-8')) > MAX_LINE:
        return 'too_large'
    try:
        value = load(line)
    except ValueError:
        return 'not_json'
    return run_problem(value)


def read_log(text):
    """§1: the runs a reader keeps, the lines it skips and whether the last line was half written."""
    lines = text.split('\n')
    partial = lines[-1] != ''
    lines = lines[:-1]  # the piece after the last newline: empty, or a line still being written
    runs, problems, seen, duplicates = [], [], set(), []
    for number, line in enumerate(lines, start=1):
        if line.strip() == '':
            continue
        problem = line_problem(line)
        if problem:
            problems.append({'line': number, 'problem': problem})
            continue
        run = load(line)
        if run['run_id'] in seen:
            duplicates.append(run['run_id'])
            continue
        seen.add(run['run_id'])
        runs.append(run)
    return {'runs': runs, 'problems': problems, 'duplicates': duplicates, 'partial_last_line': partial}


# --------------------------------------------------------------------------------------------- §8


def machine_key(run):
    m = run['machine']
    return (m['model'], m['chip'], m['memory_bytes'])


def inputs_key(run):
    return sorted((i['name'], i['sha256']) for i in run['inputs'])


def wall_median(run):
    """The median wall time of an `ok` run, or None for a quality-only run (§6.1)."""
    wall = run.get('metrics', {}).get('wall_ms')
    return wall['median'] if isinstance(wall, dict) else None


def compare(baseline, candidate):
    """§8: the speedup of `candidate` over `baseline`, with what differs between them."""
    for run in (baseline, candidate):
        if run_problem(run) or run['status'] != 'ok':
            return {'comparable': False, 'reason': 'not_ok'}
    if (baseline['suite'], baseline['case']) != (candidate['suite'], candidate['case']):
        return {'comparable': False, 'reason': 'other_case'}
    if inputs_key(baseline) != inputs_key(candidate):
        return {'comparable': False, 'reason': 'other_inputs'}
    flags = []
    if machine_key(baseline) != machine_key(candidate):
        flags.append('machine')
    if baseline['cache'] != candidate['cache']:
        flags.append('cache')
    if baseline.get('params', {}) != candidate.get('params', {}):
        flags.append('params')
    if baseline['subject']['dirty'] is not False or candidate['subject']['dirty'] is not False:
        flags.append('dirty')
    a, b = wall_median(baseline), wall_median(candidate)
    speedup = None if a is None or b is None or b == 0 else a / b
    return {'comparable': True, 'speedup': speedup, 'flags': flags}


# --------------------------------------------------------------------------------------------- helpers


def now_utc():
    return datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%S.%fZ')


def new_run_id():
    return secrets.token_hex(16)


def imported_run_id(origin):
    return hashlib.sha256(origin.encode('utf-8')).hexdigest()[:32]


def _command(*argv):
    try:
        return subprocess.run(list(argv), capture_output=True, text=True, check=True).stdout.strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


def machine():
    """§4 for this computer: never the host name, the user or a serial number."""
    arch = {'arm64': 'aarch64', 'aarch64': 'aarch64', 'x86_64': 'x86_64', 'AMD64': 'x86_64'}.get(platform.machine())
    info = {'model': None, 'chip': None, 'cpu_cores': os.cpu_count(), 'memory_bytes': None, 'os': None, 'arch': arch}
    system = platform.system()
    if system == 'Darwin':
        info['model'] = _command('sysctl', '-n', 'hw.model')
        info['chip'] = _command('sysctl', '-n', 'machdep.cpu.brand_string')
        memory = _command('sysctl', '-n', 'hw.memsize')
        info['memory_bytes'] = int(memory) if memory and memory.isdigit() else None
        version = _command('sw_vers', '-productVersion')
        info['os'] = 'macOS ' + version if version else None
    elif system == 'Linux':
        try:
            with open('/proc/meminfo', encoding='ascii') as meminfo:
                for line in meminfo:
                    if line.startswith('MemTotal:'):
                        info['memory_bytes'] = int(line.split()[1]) * 1024
        except OSError:
            pass
        info['os'] = 'Linux ' + platform.release()
    elif system == 'Windows':
        info['os'] = 'Windows ' + platform.version()
    return info


def subject(app, engine, version=None, repo=None):
    """§3, with the commit and whether the checkout is dirty read from `repo` when given."""
    commit = dirty = None
    if repo is not None:
        commit = _command('git', '-C', str(repo), 'rev-parse', '--short=12', 'HEAD')
        status = _command('git', '-C', str(repo), 'status', '--porcelain', '--untracked-files=no')
        if commit is not None:
            dirty = bool(status)
    return {'app': app, 'engine': engine, 'version': version, 'commit': commit, 'dirty': dirty}


def file_input(name, path):
    """§5: an input by its generic name, with the hash and size of the file at `path` (read in blocks)."""
    digest = hashlib.sha256()
    size = 0
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
            size += len(block)
    return {'name': name, 'sha256': digest.hexdigest(), 'bytes': size}


def measure(function, repeats=5, warmup=1):
    """Times `function()` in this process: wall and CPU time of each repetition."""
    for _ in range(warmup):
        function()
    wall, cpu = [], []
    for _ in range(repeats):
        start_wall, start_cpu = time.perf_counter(), time.process_time()
        function()
        cpu.append((time.process_time() - start_cpu) * 1000)
        wall.append((time.perf_counter() - start_wall) * 1000)
    return {'repeats': repeats, 'warmup': warmup, 'status': 'ok',
            'metrics': {'wall_ms': distribution(wall), 'cpu_ms': distribution(cpu)}}


def measure_command(argv, repeats=5, warmup=1, env=None, cwd=None, stdin=None):
    """Runs `argv` once per repetition and measures it as a process (macOS and Linux).

    Wall time from spawn to exit, CPU time and peak RSS of that process from wait4, so the numbers are
    exact for the process but do not include its grandchildren (contract §14). A non-zero exit stops
    the measurement and returns a failed run with what was measured before it.
    """
    wall, cpu, peak = [], [], 0
    rss_unit = 1 if platform.system() == 'Darwin' else 1024
    for index in range(warmup + repeats):
        start = time.perf_counter()
        process = subprocess.Popen(argv, env=env, cwd=cwd, stdin=subprocess.PIPE if stdin is not None else subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if stdin is not None:
            process.stdin.write(stdin)
            process.stdin.close()
        _, status, usage = os.wait4(process.pid, 0)
        elapsed = (time.perf_counter() - start) * 1000
        process.returncode = os.waitstatus_to_exitcode(status)
        if process.returncode != 0:
            result = {'repeats': len(wall), 'warmup': warmup, 'status': 'failed',
                      'error': {'code': 'exit_status', 'message': 'exit status %d' % process.returncode}}
            if wall:
                result['metrics'] = {'wall_ms': distribution(wall), 'cpu_ms': distribution(cpu)}
            return result
        if index < warmup:
            continue
        wall.append(elapsed)
        cpu.append((usage.ru_utime + usage.ru_stime) * 1000)
        peak = max(peak, usage.ru_maxrss * rss_unit)
    return {'repeats': repeats, 'warmup': warmup, 'status': 'ok', 'metrics': {
        'wall_ms': distribution(wall), 'cpu_ms': distribution(cpu),
        'peak_memory': {'bytes': peak, 'measure': 'rss', 'scope': 'process'}}}


# --------------------------------------------------------------------------------------------- soak (§5.1, §5.2)

_RUSAGE_V2 = ('user_time', 'system_time', 'pkg_idle_wkups', 'interrupt_wkups', 'pageins', 'wired_size',
              'resident_size', 'phys_footprint', 'proc_start_abstime', 'proc_exit_abstime', 'child_user_time',
              'child_system_time', 'child_pkg_idle_wkups', 'child_interrupt_wkups', 'child_pageins',
              'child_elapsed_abstime', 'diskio_bytesread', 'diskio_byteswritten')
_darwin = {}


def _darwin_sample(pid):
    import ctypes
    if not _darwin:
        class Info(ctypes.Structure):
            _fields_ = [('uuid', ctypes.c_uint8 * 16)] + [(name, ctypes.c_uint64) for name in _RUSAGE_V2]

        class Timebase(ctypes.Structure):
            _fields_ = [('numer', ctypes.c_uint32), ('denom', ctypes.c_uint32)]

        system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
        system.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
        system.proc_pid_rusage.restype = ctypes.c_int
        base = Timebase()
        if system.mach_timebase_info(ctypes.byref(base)) != 0 or base.denom == 0:
            base.numer = base.denom = 1
        _darwin.update(system=system, info=Info, numer=base.numer, denom=base.denom)
    info = _darwin['info']()
    if _darwin['system'].proc_pid_rusage(pid, 2, ctypes.byref(info)) != 0:  # RUSAGE_INFO_V2
        return None
    # ri_user_time and ri_system_time are Mach absolute time units, not nanoseconds on Apple silicon.
    nanos = (info.user_time + info.system_time) * _darwin['numer'] // _darwin['denom']
    return {'rss_bytes': info.resident_size, 'cpu_ms': nanos // 1000000}


def _linux_sample(pid):
    try:
        with open('/proc/%d/statm' % pid, encoding='ascii') as handle:
            resident = int(handle.read().split()[1])
        with open('/proc/%d/stat' % pid, encoding='utf-8', errors='replace') as handle:
            stat = handle.read()
    except (OSError, ValueError, IndexError):
        return None
    fields = stat[stat.rfind(')') + 1:].split()
    ticks = os.sysconf('SC_CLK_TCK')
    user, system = int(fields[11]), int(fields[12])
    return {'rss_bytes': resident * os.sysconf('SC_PAGE_SIZE'), 'cpu_ms': (user + system) * 1000 // ticks}


def process_sample(pid=None):
    """Resident memory and accumulated CPU time of `pid` (this process when None): proc_pid_rusage on macOS,
    /proc on Linux. None when it cannot be read (the process ended, or another system)."""
    pid = os.getpid() if pid is None else pid
    try:
        if platform.system() == 'Darwin':
            return _darwin_sample(pid)
        if platform.system() == 'Linux':
            return _linux_sample(pid)
    except OSError:
        return None
    return None


def resources(interval_ms, samples, max_samples=2000):
    """§5.1: halves the series (every other sample, keeping the last) until it fits, doubling the interval."""
    kept, interval = list(samples), max(1, interval_ms)
    limit = min(max(max_samples, 2), MAX_RESOURCE_SAMPLES)
    while len(kept) > limit:
        last = len(kept) - 1
        kept = [s for i, s in enumerate(kept) if i % 2 == 0 or i == last]
        interval *= 2
    cpu, highest = [], 0
    for sample in kept:
        highest = max(highest, sample['cpu_ms'])  # CPU time never goes back in the series
        cpu.append(highest)
    return {'interval_ms': interval, 'rss_bytes': [s['rss_bytes'] for s in kept], 'cpu_ms': cpu}


def measure_soak(step, iterations, slope_limit, warmup=0, interval_ms=50, pid=None, max_samples=2000):
    """§5.2: runs `step()` warmup + iterations times while a thread samples `pid` (this process when None).

    Gives the fields of new_run: one repetition with the wall and CPU time of the measured iterations,
    peak_memory, resources over the whole soak and throughput in iterations per second; quality with the slope
    of the resident memory on the iteration count after the warmup (limit `slope_limit`, `le`) and the growth
    from the end of the warmup to the end; params with iterations, warmup_iterations and interval_ms. An
    exception in `step`, or a process that cannot be sampled, gives a failed run.
    """
    if iterations < 1 or not math.isfinite(slope_limit):
        raise ValueError('a soak needs at least one iteration and a finite slope limit')
    interval_ms = max(1, interval_ms)
    params = {'iterations': iterations, 'warmup_iterations': warmup, 'interval_ms': interval_ms}

    def failed(code, message):
        return {'repeats': 0, 'warmup': warmup, 'status': 'failed', 'params': params,
                'error': {'code': code, 'message': message}}

    first = process_sample(pid)
    if first is None:
        return failed('sample_failed', 'the process could not be sampled')
    points, lock, stop, finished = [(0, first)], threading.Lock(), threading.Event(), [0]

    def sampler():
        while not stop.wait(interval_ms / 1000):
            sample = process_sample(pid)
            if sample is not None:
                with lock:
                    points.append((finished[0], sample))

    def mark(count):
        sample = process_sample(pid)
        if sample is not None:
            with lock:
                points.append((count, sample))
        return sample

    thread = threading.Thread(target=sampler, daemon=True)
    thread.start()
    try:
        for index in range(warmup):
            try:
                step()
            except Exception as error:  # noqa: BLE001 - any failure of the operation ends the soak
                return failed('step_failed', 'warmup iteration %d failed: %s' % (index, type(error).__name__))
            finished[0] = index + 1
        start = mark(warmup)
        if start is None:
            return failed('sample_failed', 'the process could not be sampled')
        clock = time.perf_counter()
        for index in range(iterations):
            try:
                step()
            except Exception as error:  # noqa: BLE001
                return failed('step_failed', 'iteration %d failed: %s' % (index, type(error).__name__))
            finished[0] = warmup + index + 1
        wall = (time.perf_counter() - clock) * 1000
        end = mark(warmup + iterations)
        if end is None:
            return failed('sample_failed', 'the process could not be sampled')
    finally:
        stop.set()
        thread.join()

    after = [(n, s) for n, s in points if n >= warmup]
    slope = memory_slope([n for n, _ in after], [s['rss_bytes'] for _, s in after])
    slope = 0.0 if slope is None else slope
    samples = [s for _, s in points]
    metrics = {'wall_ms': distribution([wall]), 'cpu_ms': distribution([float(max(0, end['cpu_ms'] - start['cpu_ms']))]),
               'peak_memory': {'bytes': max(s['rss_bytes'] for s in samples), 'measure': 'rss', 'scope': 'process'}}
    if wall > 0:
        metrics['throughput'] = {'value': iterations / (wall / 1000), 'unit': 'iter/s'}
    metrics['resources'] = resources(interval_ms, samples, max_samples)
    quality = [{'metric': SOAK_SLOPE, 'value': slope, 'reference': None, 'limit': float(slope_limit), 'op': 'le',
                'pass': slope <= slope_limit},
               {'metric': SOAK_GROWTH, 'value': end['rss_bytes'] - start['rss_bytes'], 'reference': None, 'pass': None}]
    return {'repeats': 1, 'warmup': 0, 'status': 'ok', 'metrics': metrics, 'quality': quality, 'params': params}


def new_run(suite, case, category, subject, inputs, cache, privacy, repeats, warmup, status,
            metrics=None, quality=None, params=None, error=None, note=None, machine_info=None):
    """A measured run with a fresh id and the current time, in the contract's field order."""
    run = {'contract': CONTRACT, 'run_id': new_run_id(), 'at': now_utc(), 'suite': suite, 'case': case,
           'category': category, 'subject': subject, 'machine': machine_info or machine(), 'inputs': inputs}
    if params is not None:
        run['params'] = params
    run.update({'cache': cache, 'repeats': repeats, 'warmup': warmup, 'status': status})
    if metrics is not None:
        run['metrics'] = metrics
    if quality is not None:
        run['quality'] = quality
    if error is not None:
        run['error'] = error
    run.update({'privacy': privacy, 'source': {'kind': 'measured', 'tool': TOOL}})
    if note is not None:
        run['note'] = note
    return run


def append(path, run):
    """§1: validates the run, then appends it as one line under the exclusive lock of the log."""
    problem = run_problem(run)
    if problem:
        raise ValueError('not a nu9ve-bench/1 run: ' + problem)
    line = (dumps(run) + '\n').encode('utf-8')
    if len(line) - 1 > MAX_LINE:
        raise ValueError('not a nu9ve-bench/1 run: too_large')
    import fcntl
    descriptor = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        written = os.write(descriptor, line)
        if written != len(line):
            raise OSError('short write to %s' % path)
    finally:
        os.close(descriptor)


def read(path):
    with open(path, encoding='utf-8') as handle:
        return read_log(handle.read())
