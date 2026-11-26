#!/usr/bin/env python3
"""CLI de los métodos nuevos del motor de Rust: acumulados en disco y perfiles de la suite en esta Mac.

Llama a `nuun-engine rpc` (el mismo RPC que usa la app) y muestra el resultado. Sólo lee, salvo `scan`, que guarda
un escaneo como revisión en la carpeta de nüün, `register --yes`, que agrega la entrada `nuun` a profiles.json, y
`settings`, que guarda las carpetas o el umbral en el catálogo. Nada borra ni mueve archivos.

    python3 scripts/suite_storage.py accumulations [--scan] [--json]
    python3 scripts/suite_storage.py settings [--root DIR ...] [--default-roots] [--internal-min-free-gib N]
    python3 scripts/suite_storage.py profiles [--json]
    python3 scripts/suite_storage.py register --yes
"""
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_ENGINE = REPO / 'engine' / 'target' / 'release' / 'nuun-engine'


def call(engine, method, params, state_dir=None):
    argv = [str(engine)] + (['--state-dir', state_dir] if state_dir else []) + ['rpc']
    done = subprocess.run(argv, input=json.dumps({'id': 1, 'method': method, 'params': params}).encode(),
                          capture_output=True, check=False)
    reply = json.loads(done.stdout or b'{}')
    if not reply.get('ok'):
        error = reply.get('error') or {}
        sys.exit('%s: %s' % (error.get('code', 'error'), error.get('message', done.stderr.decode(errors='replace'))))
    return reply['data']


def size(n):
    n = float(n or 0)
    for unit in ('B', 'KB', 'MB', 'GB', 'TB'):
        if n < 1000 or unit == 'TB':
            return '%.1f %s' % (n, unit) if unit != 'B' else '%d B' % n
        n /= 1000


def show_accumulations(data):
    for v in data.get('volumes', []):
        flag = '  CASI LLENO' if v.get('almost_full') else ''
        print('%-12s %s libres de %s%s' % (v['label'], size(v['free_bytes']), size(v['total_bytes']), flag))
    if data.get('state') == 'none':
        print('\nSin escanear. Usa --scan.')
        return
    s = data['summary']
    print('\n%s en %d hallazgos · escaneo %s%s' % (size(s['bytes']), s['findings'], data['scanned_at'],
                                                  ' (incompleto)' if data.get('truncated') else ''))
    print('\nPlan por tipo (nada se borra desde aquí):')
    for g in data['plan']['groups']:
        extra = ' · manual' if g['action'] == 'manual' else (' · 2 confirmaciones' if g['confirmations'] >= 2 else '')
        print('  %-34s %10s  %3d  %s%s' % (g['label'], size(g['bytes']), g['findings'], g['cost_note'], extra))
    print('\nMás grandes:')
    for r in data['findings'][:20]:
        print('  %10s  %-16s %s' % (size(r['bytes_on_disk']), r['rule'], r['path']))


def show_profiles(data):
    print('profiles.json: %s %s' % (data['file']['state'], data['file'].get('path', '')))
    for p in data['profiles']:
        print('\n%s (%s) · %s' % (p['display_name'], p['profile_id'], p['kind']))
        for a in p['apps']:
            print('  %s %-6s %s%s' % ('*' if a['own'] else ' ', a['app'], a['library'], ' (predeterminado)' if a['default'] else ''))
    pending = [p['id'] for p in data['nuun'] if not p['registered']]
    if pending:
        print('\nPerfiles de nüün sin registrar: %s (register --yes)' % ', '.join(pending))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--engine', type=Path, default=Path(os.environ.get('NUUN_ENGINE', DEFAULT_ENGINE)))
    parser.add_argument('--state-dir')
    sub = parser.add_subparsers(dest='command', required=True)
    acc = sub.add_parser('accumulations')
    acc.add_argument('--scan', action='store_true')
    acc.add_argument('--json', action='store_true')
    settings = sub.add_parser('settings')
    settings.add_argument('--root', action='append')
    settings.add_argument('--default-roots', action='store_true')
    settings.add_argument('--internal-min-free-gib', type=int)
    prof = sub.add_parser('profiles')
    prof.add_argument('--json', action='store_true')
    reg = sub.add_parser('register')
    reg.add_argument('--yes', action='store_true', help='confirm adding nüün to profiles.json')
    args = parser.parse_args(argv)
    if args.command == 'accumulations':
        data = call(args.engine, 'storage_accumulations_scan' if args.scan else 'storage_accumulations', {}, args.state_dir)
        print(json.dumps(data, ensure_ascii=False, indent=2)) if args.json else show_accumulations(data)
    elif args.command == 'settings':
        params = {}
        if args.root:
            params['roots'] = [str(Path(r).resolve()) for r in args.root]
        if args.default_roots:
            params['roots'] = None
        if args.internal_min_free_gib is not None:
            params['internal_min_free_bytes'] = args.internal_min_free_gib * 1024 ** 3
        print(json.dumps(call(args.engine, 'storage_accumulations_settings', params, args.state_dir), indent=2))
    elif args.command == 'profiles':
        data = call(args.engine, 'suite_profiles', {}, args.state_dir)
        print(json.dumps(data, ensure_ascii=False, indent=2)) if args.json else show_profiles(data)
    else:
        if not args.yes:
            parser.error('register changes profiles.json: pass --yes to confirm')
        show_profiles(call(args.engine, 'suite_profiles_register', {'confirm': True}, args.state_dir))
    return 0


if __name__ == '__main__':
    sys.exit(main())
