#!/usr/bin/env python3
"""Benchmarks de nüün como corridas de nu9ve-bench-v1 (vendor/nu9ve-core/contracts/nu9ve-bench-v1.md).

Agrega a un registro NDJSON que sólo crece (por omisión `bench/nuun.ndjson`, ignorado por git) lo que nüün ya
compara, siempre sobre fixtures y raíces temporales, nunca sobre discos ni la biblioteca de la persona:

  golden   lecturas del dorado (tests/fixtures/engine-golden) con el motor de Python y el de Rust, proceso por
           petición: tiempo, CPU y memoria, y si la respuesta es igual a la de Python (calidad).
  walk     recorrido de un árbol de fixture: `storage_scan` (índice de discos, Python) y
           `storage_accumulations_scan` (acumulados, Rust), en archivos por segundo.
  soak     N lecturas del catálogo (projects, library…) a un `nuun-engine rpc-serve` vivo, muestreando su memoria:
           `memory_slope_bytes_per_iter` con su límite, para detectar fugas en el gate.
  copy     «Copiar al otro disco» en discos de fixture (64 × 8 MiB, F6): la copia verificada en GiB/s, y la
           revisión de esa copia ya hecha con su recibo (U1.3) y sin él (como antes de U1.3). Lo mide la prueba
           `work_copy::tests::bench_work_copy` de Rust, compilada con `--release`; no entra en los casos por omisión.

Usa el ayudante del core (vendor/nu9ve-core/scripts/nu9ve_bench.py), sin copiarlo. nove lee el registro como
fuente de sólo lectura y une cada corrida con las sesiones de agentes por `subject.commit`.

    python3 scripts/engine_bench.py [golden walk soak copy] [--log bench/nuun.ndjson] [--engine PATH]
            [--repeats 5] [--iterations 300] [--slope-limit 4096] [--check-soak]
"""
import argparse
import importlib.util
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CORE = REPO / 'vendor' / 'nu9ve-core' / 'scripts'
GOLDEN = REPO / 'tests' / 'fixtures' / 'engine-golden' / 'recipe.json'
DEFAULT_LOG = REPO / 'bench' / 'nuun.ndjson'
DEFAULT_ENGINE = REPO / 'engine' / 'target' / 'release' / 'nuun-engine'
# The soak asks catalog reads only: `snapshot` would also ask the volume backend about whatever disks are
# mounted (diskutil), which is slow and is not the fixture.
SOAK_READS = ('projects', 'library', 'living_collections', 'missions_status', 'jobs')
GOLDEN_READS = ('projects', 'library', 'jobs', 'recipe', 'living_collections', 'missions_status')


def bench_module():
    spec = importlib.util.spec_from_file_location('nu9ve_bench', CORE / 'nu9ve_bench.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


B = bench_module()


def version():
    for line in (REPO / 'pyproject.toml').read_text(encoding='utf-8').splitlines():
        if line.startswith('version'):
            return line.split('=', 1)[1].strip().strip('"\'')
    return None


def isolated_env(home, user_home=None):
    """QA: every shared suite file goes to the temporary root, never to the person's."""
    env = dict(os.environ, NU9VE_MEDIA_HOME=str(home), NU9VE_SANDBOX='',
               NU9VE_PROFILES_FILE=str(Path(home) / 'qa' / 'profiles.json'),
               NU9VE_DEVICE_FILE=str(Path(home) / 'qa' / 'device.json'),
               NU9VE_ENGINES_DIR=str(Path(home) / 'qa' / 'engines'), TZ='America/Phoenix')
    env.pop('NU9VE_SANDBOX')
    if user_home:
        env['HOME'] = str(user_home)
    return env


def golden_catalog(root):
    golden = json.loads(GOLDEN.read_text(encoding='utf-8'))
    home = Path(root) / 'golden'
    home.mkdir()
    db = sqlite3.connect(str(home / 'catalog.sqlite3'))
    db.executescript(golden['reads']['sql'])
    db.close()
    return home


def python_argv(home):
    return [sys.executable, '-S', str(REPO / 'nu9ve.py'), 'media', '--state-dir', str(home), 'rpc']


def rust_argv(engine, home):
    return [str(engine), '--state-dir', str(home), 'rpc']


def answer(argv, request, env):
    done = subprocess.run(argv, input=request, capture_output=True, env=env, cwd=REPO, check=False)
    try:
        value = json.loads(done.stdout)
    except ValueError:
        return None
    return value.get('data') if value.get('ok') else {'error': (value.get('error') or {}).get('code')}


def golden(log, engine, repeats, subject_py, subject_rs, machine):
    with tempfile.TemporaryDirectory() as root:
        home = golden_catalog(Path(root).resolve())
        inputs = [B.file_input('fixture/engine-golden/recipe.json', GOLDEN)]
        env = isolated_env(home)
        for method in GOLDEN_READS:
            request = json.dumps({'id': 1, 'method': method, 'params': {}}).encode()
            reference = answer(python_argv(home), request, env)
            for name, argv, subject in (('python', python_argv(home), subject_py), ('rust', rust_argv(engine, home), subject_rs)):
                measured = B.measure_command(argv, repeats=repeats, warmup=1, env=env, cwd=str(REPO), stdin=request)
                quality = None
                if name == 'rust':
                    same = answer(argv, request, env) == reference
                    quality = [{'metric': 'response_equal', 'value': same, 'reference': 'python', 'limit': True,
                                'op': 'eq', 'pass': same}]
                run = B.new_run('nuun-engine', 'golden/' + method, 'read', subject, inputs, 'warm', 'public',
                                measured['repeats'], measured['warmup'], measured['status'], measured.get('metrics'),
                                quality, {'mode': 'process'}, measured.get('error'), machine_info=machine)
                B.append(str(log), run)
                print('golden/%-20s %-6s %s' % (method, name, summary(run)))


def fixture_tree(root, folders=40, files=100):
    """A tree of small files with a Rust target/ and node_modules/, all generated, the same every time."""
    tree = Path(root) / 'code'
    for project in range(folders):
        base = tree / ('p%02d' % project)
        (base / 'src').mkdir(parents=True)
        for index in range(files):
            (base / 'src' / ('f%03d.txt' % index)).write_bytes(b'x' * (index * 37 % 4096))
        if project % 4 == 0:
            (base / 'Cargo.toml').write_text('[package]\n')
            (base / 'target' / 'release').mkdir(parents=True)
            (base / 'target' / 'CACHEDIR.TAG').write_text('Signature: 8a477f597d28d172789f06886806bc55\n')
            for index in range(files):
                (base / 'target' / 'release' / ('o%03d' % index)).write_bytes(b'o' * 8192)
        if project % 4 == 1:
            (base / 'package.json').write_text('{}')
            (base / 'node_modules' / 'm').mkdir(parents=True)
            for index in range(files):
                (base / 'node_modules' / 'm' / ('j%03d.js' % index)).write_bytes(b'j' * 2048)
    return tree, sum(1 for p in tree.rglob('*') if p.is_file())


def walk(log, engine, repeats, subject_py, subject_rs, machine):
    from media.catalog import Catalog
    from media.service import Service
    with tempfile.TemporaryDirectory() as root:
        root = Path(root).resolve()
        tree, count = fixture_tree(root)
        params = {'files': count}
        # Python: the disk index (storage_scan over a chosen folder), in this process.
        home = root / 'nuun'
        catalog = Catalog(home)
        service = Service(catalog)
        # The fixture lives in the temporary folder (/private/var on macOS), which nüün refuses as a system
        # folder for a person; here it is generated, as tests/test_media_storage.py does.
        from media import storage_index
        storage_index.SYSTEM_ROOTS = ('/System', '/Library')
        target = service.call('storage_add_folder', {'root': str(tree)})['id']
        measured = B.measure(lambda: service.call('storage_scan', {'target': target}), repeats=repeats, warmup=1)
        catalog.close()
        add_throughput(measured, count)
        run = B.new_run('nuun-engine', 'walk/storage-index', 'read', subject_py, [], 'warm', 'public',
                        measured['repeats'], measured['warmup'], measured['status'], measured['metrics'],
                        params=params, machine_info=machine)
        B.append(str(log), run)
        print('walk/storage-index    python %s' % summary(run))
        # Rust: the accumulations scan over the same tree, as a process.
        rhome = root / 'nuun-rs'
        rhome.mkdir()
        env = isolated_env(rhome, user_home=root)
        request = json.dumps({'id': 1, 'method': 'storage_accumulations_scan', 'params': {}}).encode()
        measured = B.measure_command(rust_argv(engine, rhome), repeats=repeats, warmup=1, env=env, cwd=str(REPO), stdin=request)
        add_throughput(measured, count)
        run = B.new_run('nuun-engine', 'walk/accumulations', 'read', subject_rs, [], 'warm', 'public',
                        measured['repeats'], measured['warmup'], measured['status'], measured.get('metrics'),
                        params=params, error=measured.get('error'), machine_info=machine)
        B.append(str(log), run)
        print('walk/accumulations    rust   %s' % summary(run))


def add_throughput(measured, count):
    if measured.get('status') == 'ok':
        median = measured['metrics']['wall_ms']['median']
        if median > 0:
            measured['metrics']['throughput'] = {'value': count / (median / 1000), 'unit': 'files/s'}


def soak(log, engine, iterations, slope_limit, subject_rs, machine):
    """§5.2: one live `rpc-serve`, `iterations` catalog reads, its memory sampled the whole time."""
    with tempfile.TemporaryDirectory() as root:
        home = golden_catalog(Path(root).resolve())
        env = isolated_env(home)
        process = subprocess.Popen([str(engine), '--state-dir', str(home), 'rpc-serve'], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env, cwd=REPO)
        counter = [0]

        def step():
            counter[0] += 1
            method = SOAK_READS[counter[0] % len(SOAK_READS)]
            process.stdin.write((json.dumps({'id': counter[0], 'method': method, 'params': {}}) + '\n').encode())
            process.stdin.flush()
            line = process.stdout.readline()
            if not line or json.loads(line).get('ok') is not True:
                raise RuntimeError(method + ' failed')

        try:
            result = B.measure_soak(step, iterations, slope_limit, warmup=max(10, iterations // 10), interval_ms=20,
                                    pid=process.pid)
        finally:
            process.stdin.close()
            process.wait(timeout=10)
        inputs = [B.file_input('fixture/engine-golden/recipe.json', GOLDEN)]
        run = B.new_run('nuun-engine', 'soak/catalog-reads', 'soak', subject_rs, inputs, 'warm', 'public',
                        result['repeats'], result['warmup'], result['status'], result.get('metrics'),
                        result.get('quality'), result.get('params'), result.get('error'), machine_info=machine)
        B.append(str(log), run)
        print('soak/catalog-reads    rust   %s' % summary(run))
        return run


def copy(log, repeats, subject_rs, machine):
    """The copy and its review, timed inside the Rust test on fixture disks; one run per measure."""
    with tempfile.TemporaryDirectory() as root:
        out = Path(root) / 'bench.json'
        env = dict(os.environ, NUUN_BENCH_OUT=str(out), NUUN_BENCH_REPEATS=str(repeats),
                   TMPDIR=str(Path(root).resolve()) + '/')
        done = subprocess.run(['cargo', 'test', '--release', '-p', 'nuun-media', '--lib', 'work_copy::tests::bench_work_copy',
                               '--', '--ignored', '--exact'], cwd=REPO / 'engine', env=env, capture_output=True, text=True)
        if done.returncode != 0 or not out.exists():
            raise SystemExit('bench_work_copy failed:\n' + done.stdout[-2000:] + done.stderr[-2000:])
        result = json.loads(out.read_text(encoding='utf-8'))
    params = {'files': result['files'], 'bytes': result['bytes']}
    runs = []
    for case, key, category, extra in (('copy/work-copy', 'copy_ms', 'write', {}),
                                       ('review/work-copy-receipt', 'review_receipt_ms', 'read', {'mode': 'receipt'}),
                                       ('review/work-copy-hash', 'review_hash_ms', 'read', {'mode': 'hash'})):
        samples = result[key]
        metrics = {'wall_ms': B.distribution(samples)}
        median = metrics['wall_ms']['median']
        if case == 'copy/work-copy' and median > 0:
            metrics['throughput'] = {'value': result['bytes'] / 2 ** 30 / (median / 1000), 'unit': 'GiB/s'}
        elif median > 0:
            metrics['throughput'] = {'value': result['files'] / (median / 1000), 'unit': 'files/s'}
        run = B.new_run('nuun-engine', case, category, subject_rs, [], 'warm', 'public', len(samples), 0, 'ok',
                        metrics, params=dict(params, **extra), machine_info=machine)
        B.append(str(log), run)
        runs.append(run)
        print('%-24s rust   %s' % (case, summary(run)))
    speedup = runs[2]['metrics']['wall_ms']['median'] / max(runs[1]['metrics']['wall_ms']['median'], 1e-9)
    print('revisión con recibo: %.0f× más rápida que sin él' % speedup)
    return runs


def summary(run):
    if run['status'] != 'ok':
        return 'failed: %s' % (run.get('error') or {}).get('message')
    m = run['metrics']
    text = 'median %.1f ms' % m['wall_ms']['median']
    if 'peak_memory' in m:
        text += ', peak %.1f MiB' % (m['peak_memory']['bytes'] / 1048576)
    if 'throughput' in m:
        value = m['throughput']['value']
        text += (', %.0f %s' if value >= 100 else ', %.3g %s') % (value, m['throughput']['unit'])
    for q in run.get('quality') or []:
        text += ', %s=%s%s' % (q['metric'], q['value'] if not isinstance(q['value'], float) else round(q['value'], 1),
                               '' if q['pass'] in (True, None) else ' FAIL')
    return text


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('cases', nargs='*', help='golden, walk, soak, copy (golden, walk and soak when none)')
    parser.add_argument('--log', type=Path, default=DEFAULT_LOG)
    parser.add_argument('--engine', type=Path, default=Path(os.environ.get('NUUN_ENGINE', DEFAULT_ENGINE)))
    parser.add_argument('--repeats', type=int, default=5)
    parser.add_argument('--iterations', type=int, default=300)
    parser.add_argument('--slope-limit', type=float, default=4096.0, help='bytes per iteration')
    parser.add_argument('--check-soak', action='store_true', help='exit 1 if the soak slope is over its limit')
    args = parser.parse_args(argv)
    args.cases = args.cases or ['golden', 'walk', 'soak']
    if set(args.cases) - {'golden', 'walk', 'soak', 'copy'}:
        parser.error('cases are golden, walk, soak and copy')
    if not args.engine.exists():
        parser.error('no engine at %s: cargo build --release in engine/' % args.engine)
    args.log.parent.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(REPO))
    machine = B.machine()
    subject_py = B.subject('nuun', 'python', version(), REPO)
    subject_rs = B.subject('nuun', 'rust', version(), REPO)
    if 'golden' in args.cases:
        golden(args.log, args.engine, args.repeats, subject_py, subject_rs, machine)
    if 'walk' in args.cases:
        walk(args.log, args.engine, args.repeats, subject_py, subject_rs, machine)
    if 'copy' in args.cases:
        copy(args.log, min(args.repeats, 3), subject_rs, machine)
    if 'soak' in args.cases:
        run = soak(args.log, args.engine, args.iterations, args.slope_limit, subject_rs, machine)
        if args.check_soak and (run['status'] != 'ok' or not run['quality'][0]['pass']):
            return 1
    print('registro: %s' % args.log)
    return 0


if __name__ == '__main__':
    sys.exit(main())
