"""The Clipper CLI is the only writer of Clipper projects. Fail closed on unknown dependencies."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from .common import MediaError, read_json, require


def executable(catalog):
    configured = catalog.get('clipper-executable', '')
    candidates = [configured] if configured else [shutil.which('clipper'), str(Path.home() / '.local/bin/clipper'), '/opt/homebrew/bin/clipper']
    result = next((p for p in candidates if p and Path(p).is_absolute() and os.access(p, os.X_OK) and Path(p).is_file()), None)
    require(result, 'clipper_unavailable', 'Configura el ejecutable de Clipper en Ajustes → Capturas antes de liberar espacio')
    return result


def invoke(catalog, args):
    try:
        result = subprocess.run([executable(catalog), 'library', *args, '--json'], capture_output=True, timeout=120)
        require(result.returncode == 0, 'clipper_failed', 'Clipper no pudo comprobar/actualizar sus dependencias; no se libera el origen')
        require(len(result.stdout) <= 16 * 1024 * 1024, 'clipper_invalid', 'Clipper response too large')
        return json.loads(result.stdout)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise MediaError('clipper_unavailable', 'No se pudo consultar Clipper: ' + str(error)) from error


def dependencies(catalog):
    data = invoke(catalog, ['deps'])
    require(isinstance(data, dict) and data.get('schema') == 'clipper-library-deps/1'
            and isinstance(data.get('projects'), list) and isinstance(data.get('offline_projects'), list),
            'clipper_invalid', 'Unsupported Clipper dependencies')
    # Missing fields must never turn an unknown dependency into an unused path.
    for project in data['projects']:
        require(isinstance(project, dict) and isinstance(project.get('clipper_root'), str)
                and isinstance(project.get('assets'), list) and isinstance(project.get('relinks'), list), 'clipper_invalid', 'Incomplete Clipper project dependencies')
        for asset in project['assets']:
            require(isinstance(asset, dict) and isinstance(asset.get('path'), str) and bool(asset['path'])
                    and Path(asset['path']).is_absolute() and isinstance(asset.get('in_use'), bool)
                    and isinstance(asset.get('queued_exports'), list) and isinstance(asset.get('items'), list)
                    and isinstance(asset.get('asset_id'), str), 'clipper_invalid', 'Incomplete Clipper asset dependencies')
    return data


def overlaps(a, b):
    a, b = os.path.realpath(a), os.path.realpath(b)
    if sys.platform == 'darwin':
        a, b = a.casefold(), b.casefold()
    a, b = Path(a), Path(b)
    return a == b or a in b.parents or b in a.parents


def inspect(catalog, paths, targets=None):
    """paths maps a local asset/bundle root to its verified replacement root."""
    data = dependencies(catalog)
    blockers, relinks = [], []
    if data['offline_projects']:
        blockers.append('Hay proyectos Clipper desconectados; dependencias desconocidas')
    active_queue = queue_ids()
    for project in data['projects']:
        # A queued snapshot may keep the old path even after the live asset was relinked.
        for ack in project['relinks']:
            require(isinstance(ack, dict), 'clipper_invalid', 'Invalid relink ACK')
            pins = ack.get('still_pinned')
            if pins and (active_queue is None or any(pin in active_queue for pin in pins)) and any(overlaps(ack.get('from') or '/', p) for p in paths):
                blockers.append('Una exportación aún fija la ruta anterior: ' + str(ack.get('from')))
        for asset in project['assets']:
            used = [asset['path'], *asset['items']]
            hits = [p for p in paths if any(isinstance(value, str) and overlaps(p, value) for value in used)]
            if not hits:
                continue
            if asset['queued_exports']:
                blockers.append('Exportación en cola: ' + asset['path'])
            if asset['in_use'] or asset.get('sync_groups'):
                target = next((paths[p] for p in hits if paths[p]), None)
                if target and not asset['queued_exports']:
                    relinks.append({'project': project['clipper_root'], 'asset_id': asset['asset_id'],
                                    'revision': asset.get('revision'), 'from': asset['path'], 'to': target})
                else:
                    blockers.append('Fuente en uso sin reemplazo editable: ' + asset['path'])
    blockers.extend(unreported_references(paths, data))
    return {'blockers': list(dict.fromkeys(blockers)), 'relinks': relinks, 'dependencies': data}


def queue_ids():
    base = Path(os.environ.get('CLIPPER_PROJECTS_ROOT') or Path.home() / 'Downloads/clipper-projects').expanduser()
    marker = base / '.export-queue/queue.json'
    if not marker.exists(): return None
    try:
        data = read_json(marker)
        require(data.get('schema_version') == 1 and isinstance(data.get('jobs'), list), 'clipper_invalid', 'Unknown export queue')
        return {job['id'] for job in data['jobs'] if job['status'] != 'completed'}
    except (OSError, RuntimeError, TypeError, KeyError, ValueError):
        return None


def unreported_references(paths, data, home=None):
    """Read-only guard for old projects absent from Clipper's linked-library report.

    Never edits producer JSON. Such projects must adopt the library in Clipper before relinking.
    """
    home = Path(home or Path.home())
    base = Path(os.environ.get('CLIPPER_PROJECTS_ROOT') or home / 'Downloads/clipper-projects').expanduser()
    blockers, roots = [], set()
    reported = {os.path.realpath(p['clipper_root']) for p in data['projects']}
    for parent in (base, home / 'Downloads'):
        if not parent.exists(): continue
        try:
            require(not parent.is_symlink(), 'clipper_invalid', 'Clipper projects folder is a symlink')
            roots.update(p.parent for p in parent.glob('*/clipper-project.json'))
        except (OSError, RuntimeError) as error:
            blockers.append('No se pudieron revisar proyectos antiguos de Clipper: ' + str(error))
    registry = base / '.linked-projects.json'
    if registry.exists():
        try:
            linked = read_json(registry)
            require(linked.get('schema') == 'clipper-linked-projects/1' and isinstance(linked.get('roots'), list), 'clipper_invalid', 'Invalid Clipper project registry')
            for root in linked['roots']:
                require(isinstance(root, str) and Path(root).is_absolute(), 'clipper_invalid', 'Invalid Clipper project root')
                if not (Path(root) / 'clipper-project.json').is_file(): blockers.append('Proyecto Clipper no disponible: ' + root)
                else: roots.add(Path(root))
        except (OSError, RuntimeError, ValueError, TypeError) as error:
            blockers.append('Registro Clipper desconocido: ' + str(error))
    def strings(value):
        if isinstance(value, str):
            yield value
        elif isinstance(value, list):
            for item in value: yield from strings(item)
        elif isinstance(value, dict):
            for key, item in value.items():
                if isinstance(key, str) and key.startswith('/'): yield key
                yield from strings(item)
    for root in roots:
        if os.path.realpath(root) in reported: continue
        try:
            project = read_json(root / 'clipper-project.json')
            require(isinstance(project, dict), 'clipper_invalid', 'Invalid legacy project')
            active = {key: project.get(key) for key in ('items', 'capture_sessions', 'sync_groups', 'assembly_timeline')}
            if any(value.startswith('/') and any(overlaps(value, path) for path in paths) for value in strings(active)):
                blockers.append('Proyecto antiguo usa esta ruta; adopta/reenlaza la fuente desde Clipper antes de liberar: ' + str(root))
        except (OSError, RuntimeError, ValueError, TypeError) as error:
            blockers.append('Proyecto Clipper no revisable: ' + str(root) + ': ' + str(error))
    # Also guard old snapshots that Clipper's deps cannot associate with a current asset.
    queue = base / '.export-queue/queue.json'
    if queue.exists():
        try:
            snapshot = read_json(queue)
            require(snapshot.get('schema_version') == 1 and isinstance(snapshot.get('jobs'), list), 'clipper_invalid', 'Unknown export queue')
            for job in snapshot['jobs']:
                require(isinstance(job, dict) and isinstance(job.get('sources'), dict) and isinstance(job.get('status'), str), 'clipper_invalid', 'Invalid queue job')
                if job['status'] == 'completed': continue
                if any(any(overlaps(value, path) for path in paths) for value in job['sources']):
                    blockers.append('Snapshot de exportación conserva la ruta original: ' + str(job.get('id')))
        except (OSError, RuntimeError, ValueError, TypeError) as error:
            blockers.append('Cola Clipper no revisable: ' + str(error))
    return blockers


def relink(catalog, request, operation_id):
    ack = invoke(catalog, ['relink', request['project'], request['asset_id'], '--to', request['to'], '--operation-id', operation_id])
    require(isinstance(ack, dict) and ack.get('schema') == 'clipper-relink-ack/1' and ack.get('status') == 'applied'
            and ack.get('operation_id') == operation_id and ack.get('asset_id') == request['asset_id']
            and ack.get('revision') == request.get('revision') and ack.get('from') == request['from']
            and Path(ack.get('to', '')) == Path(request['to']) and ack.get('still_pinned') == [],
            'clipper_ack_invalid', 'Clipper no confirmó un reenlace completo y libre de exportaciones')
    catalog.set('clipper-ack:' + operation_id, ack)
    return ack
