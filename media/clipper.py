"""The Clipper CLI is the only writer of Clipper projects. Fail closed on unknown dependencies."""
import json
import contextvars
import functools
import fcntl
import secrets
from contextlib import contextmanager
import os
from pathlib import Path
import shutil
import subprocess
import sys

from .common import MediaError, read_json, require

_RELEASE_TOKEN = contextvars.ContextVar('clipper_release_token', default=None)
_RELEASE_FD = contextvars.ContextVar('clipper_release_fd', default=None)


def executable(catalog):
    configured = catalog.get('clipper-executable', '')
    candidates = [configured] if configured else [shutil.which('clipper'), str(Path.home() / '.local/bin/clipper'), '/opt/homebrew/bin/clipper']
    result = next((p for p in candidates if p and Path(p).is_absolute() and os.access(p, os.X_OK) and Path(p).is_file()), None)
    require(result, 'clipper_unavailable', 'Configura el ejecutable de Clipper en Ajustes → Capturas antes de liberar espacio')
    return result


def invoke(catalog, args, timeout=120):
    try:
        env = os.environ.copy()
        if _RELEASE_TOKEN.get():env['CLIPPER_MEDIA_RELEASE_TOKEN'] = _RELEASE_TOKEN.get()
        command = executable(catalog)
        catalog.set('clipper-known', True)  # Even an incompatible/disappearing discovered consumer remains known.
        result = subprocess.run([command, 'library', *args, '--json'], capture_output=True, timeout=timeout, env=env)
        require(result.returncode == 0, 'clipper_failed', 'Clipper no pudo comprobar/actualizar sus dependencias; no se libera el origen')
        require(len(result.stdout) <= 16 * 1024 * 1024, 'clipper_invalid', 'Clipper response too large')
        return json.loads(result.stdout)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise MediaError('clipper_unavailable', 'No se pudo consultar Clipper: ' + str(error)) from error


def dependencies(catalog, timeout=120):
    try:
        data = invoke(catalog, ['deps'], timeout=timeout)
    except MediaError as error:
        base = Path(os.environ.get('CLIPPER_PROJECTS_ROOT') or Path.home() / 'Downloads/clipper-projects')
        known = (catalog.get('clipper-known', False) or catalog.get('clipper-executable', '')
                 or (base / '.linked-projects.json').exists() or (base / '.export-queue/queue.json').exists())
        if error.code == 'clipper_unavailable' and not known:
            # Only executable absence, never a failed/invalid installed consumer.
            try:
                executable(catalog)
            except MediaError:
                return {'schema': 'clipper-library-deps/1', 'capabilities': [],
                        'projects': [], 'offline_projects': [], 'integration': 'never_configured'}
        raise

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
    catalog.set('clipper-known', True)
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
                target = paths.get(asset['path']) or next((paths[p] for p in hits if paths[p]), None)
                if target and not asset['queued_exports']:
                    relinks.append({'project': project['clipper_root'], 'asset_id': asset['asset_id'],
                                    'revision': asset.get('revision'), 'from': asset['path'], 'to': target,
                                    'kind': asset.get('kind')})
                else:
                    blockers.append('Fuente en uso sin reemplazo editable: ' + asset['path'])
    # Historical rows can pin the same logical asset at another revision/path.
    ids = {r['asset_id'] for r in relinks}
    for project in data['projects']:
        if any(a['asset_id'] in ids and a['queued_exports'] for a in project['assets']):
            blockers.append('Una cola conserva miembros/revisiones anteriores del asset')
    relinks = list({(r['project'], r['asset_id']): r for r in relinks}.values())
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
    from .common import Tree
    from .library import handoff
    row = catalog.db.execute('SELECT payload FROM assets WHERE id=?', (request['asset_id'],)).fetchone()
    record = json.loads(row[0]) if row else None
    record = record if record and record['kind'] == 'delivery' else None
    document = None
    expected_to = request['to']
    if record is not None:
        # Resolve by each receipt's exact destination, never by a basename or role preference.
        volume_id = None
        for volume in catalog.volumes():
            for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (record['job_id'], volume['id'])):
                copy = json.loads(row[0])
                if copy['path'] in record['paths'] and str(Path(volume['root']) / copy['destination']) == request['to']:
                    volume_id = volume['id']
        require(volume_id, 'missing_receipt', 'Delivery replacement has no recorded location')
        key = 'clipper-handoff:' + operation_id
        intent = catalog.get(key)
        if intent:
            require(intent['request'] == request, 'operation_conflict', 'Relink operation reused with another payload')
            document = intent['document']
        else:
            document = handoff(catalog, None, [record['id']], volume_id)
            catalog.set(key, {'request': request, 'document': document})
        name = 'handoffs/' + operation_id + '.json'
        with Tree(catalog.home) as tree:
            tree.write_json(name, document)
        expected_to = str(Path(catalog.volume(volume_id)['root']) / document['assets'][0]['locations'][0]['relative_path'])
        args = ['--handoff', str(catalog.home / name)]
    else:
        args = ['--to', request['to']]
    ack = invoke(catalog, ['relink', request['project'], request['asset_id'], *args, '--operation-id', operation_id])
    require(isinstance(ack, dict) and ack.get('schema') == 'clipper-relink-ack/1' and ack.get('status') == 'applied'
            and ack.get('operation_id') == operation_id and ack.get('asset_id') == request['asset_id']
            and ack.get('revision') == request.get('revision') and ack.get('from') == request['from']
            and Path(ack.get('to', '')) == Path(expected_to) and ack.get('still_pinned') == [],
            'clipper_ack_invalid', 'Clipper no confirmó un reenlace completo y libre de exportaciones')
    if document is not None:
        location = document['assets'][0]['locations'][0]
        volume = catalog.volume(location['volume_id'])
        expected = {m['original_path']: str(Path(volume['root']) / m['volume_relative_path']) for m in location['members']}
        require(ack.get('member_targets') == expected and len(ack.get('from_members', [])) == len(expected),
                'clipper_ack_invalid', 'Clipper did not ACK every delivery member')
    catalog.set('clipper-ack:' + operation_id, ack)
    return ack


@contextmanager
def release_guard(catalog):
    """Hold Clipper's kernel exclusion through checks/relinks/release.

    Older/unavailable Clipper fails closed. Parent death closes the pipe, so a
    crashed release cannot leave a time-based stale lease behind.
    """
    if _RELEASE_TOKEN.get():
        yield _RELEASE_FD.get()
        return
    lock = Path.home()/'.cache/clipper/.media-access.lock'
    lock.parent.mkdir(parents=True, exist_ok=True)
    try:
        with lock.open('a+') as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise MediaError('clipper_busy', 'Clipper tiene trabajo activo; se conserva el origen') from None
            token = secrets.token_hex(32)
            handle.seek(0);handle.truncate()
            handle.write(json.dumps({'pid': os.getpid(), 'token': token}))
            handle.flush()
            reset = _RELEASE_TOKEN.set(token)
            fd_reset = _RELEASE_FD.set(handle.fileno())
            try:
                data = dependencies(catalog)
                require(data.get('integration') == 'never_configured' or 'media-release-lock/1' in data.get('capabilities', []),
                        'clipper_upgrade_required', 'Actualiza y reinicia Clipper para liberar media con exclusión coordinada')
                yield handle.fileno()
            finally:
                _RELEASE_FD.reset(fd_reset)
                _RELEASE_TOKEN.reset(reset)
    except OSError as error:
        raise MediaError('clipper_unavailable', 'No se pudo bloquear Clipper; se conserva el origen: ' + str(error)) from error


def guarded_release(function):
    @functools.wraps(function)
    def wrapped(catalog, *args, **kwargs):
        with release_guard(catalog):
            return function(catalog, *args, **kwargs)
    return wrapped
