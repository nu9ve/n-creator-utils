"""Coordinate completed-session metadata changes. Stalker remains the only writer."""
from contextlib import contextmanager
import contextvars
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

from . import clipper
from .common import MediaError, Tree, canonical, now, require

CAPABILITY = 'stalker-coordinated-retitle/1'
_COORDINATOR_FDS = contextvars.ContextVar('retitle_fds', default=())


@contextmanager
def exclusion(catalog):
    with clipper.release_guard(catalog) as editor_fd, catalog.writer() as writer_fd:
        token = _COORDINATOR_FDS.set(tuple(fd for fd in (editor_fd, writer_fd) if fd is not None))
        try:
            yield
        finally:
            _COORDINATOR_FDS.reset(token)


def records(catalog, prefix):
    return [json.loads(r[0]) for r in catalog.db.execute('SELECT value FROM metadata WHERE key LIKE ?', (prefix + '%',))]


def check_plan(catalog, plan):
    for unit in plan['units']:
        if not unit.get('session_id'):
            continue
        sid = str(uuid.UUID(unit['session_id']))
        require(not any(op['request']['session_id'] == sid and op['state'] != 'applied' for op in records(catalog, 'retitle:')),
                'retitle_pending', 'Recover the pending Stalker retitle before planning/applying')
        latest = catalog.get('retitle-result:' + sid)
        require(not latest or unit.get('capture_manifest_digest') == latest['manifest_digest'],
                'plan_changed', 'Stalker metadata changed; discard the old plan and review again')


def cancel_plan(catalog, plan_id):
    with catalog.writer():
        key = 'reviewed-plan:' + plan_id
        require(catalog.get(key) is not None, 'unknown_plan', 'Unknown reviewed plan')
        require(not catalog.db.execute('SELECT 1 FROM jobs WHERE id=?', (plan_id,)).fetchone(),
                'retitle_plan_active', 'An ingest job already started; cancellation cannot discard its journal')
        catalog.set(key, dict(catalog.get(key), cancelled=True))
    return {'plan_id': plan_id, 'cancelled': True}


def producer(catalog, action, request):
    executable = catalog.get('stalker-executable')
    require(isinstance(executable, str) and Path(executable).is_absolute() and os.access(executable, os.X_OK),
            'stalker_upgrade_required', 'Configure stalker_executable implementing ' + CAPABILITY + '; archived:false is insufficient. See docs/contracts/stalker-retitle-v1.md')
    try:
        result = subprocess.run([executable, 'nuun-retitle', action, '--json'], input=canonical(request),
                                capture_output=True, timeout=60, pass_fds=_COORDINATOR_FDS.get(), env={**os.environ,
                                    'NUUN_RETITLE_LOCK_FDS': ','.join(map(str, _COORDINATOR_FDS.get())),
                                    'CLIPPER_MEDIA_RELEASE_TOKEN': clipper._RELEASE_TOKEN.get() or ''})
        require(result.returncode == 0 and len(result.stdout) <= 1024 * 1024,
                'stalker_upgrade_required' if action == 'capabilities' else 'stalker_failed',
                'Stalker must implement ' + CAPABILITY + ' (docs/contracts/stalker-retitle-v1.md); retry the same operation_id to reconcile')
        data = json.loads(result.stdout)
        require(isinstance(data, dict), 'stalker_invalid', 'Invalid Stalker response')
        return data
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise MediaError('stalker_failed', 'Stalker unavailable; recover with the same operation_id: ' + str(error)) from error


def manifest_at(path):
    with Tree(path) as tree:
        fd = tree.open('manifest.json')
        with os.fdopen(fd, 'rb') as stream:
            raw = stream.read(1024 * 1024 + 1)
    require(len(raw) <= 1024 * 1024, 'invalid_bundle', 'Manifest too large')
    value = json.loads(raw)
    require(value.get('producer') == 'stalker' and type(value.get('schemaVersion')) is int and value.get('schemaVersion') in {1, 2, 3}
            and value.get('status') == 'completed', 'invalid_bundle', 'Requires a completed supported Stalker session')
    return value, hashlib.sha256(raw).hexdigest()


def blockers(catalog, request):
    sid, path = request['session_id'], request['path']
    require(not catalog.db.execute('SELECT 1 FROM sessions WHERE lower(session_id)=?', (sid,)).fetchone(),
            'retitle_archived', 'A verified archived revision exists; keep the manifest immutable and use catalog tags')
    for row in catalog.db.execute('SELECT plan,state FROM jobs'):
        plan = json.loads(row[0])
        if any(str(u.get('session_id', '')).lower() == sid for u in plan['units']):
            require(False, 'retitle_plan_active', 'An ingest job references the session; resolve it before retitling')
    for plan in records(catalog, 'reviewed-plan:'):
        if not plan.get('cancelled') and sid in plan['sessions']:
            require(False, 'retitle_plan_active', 'Cancel the reviewed plan before retitling: ' + plan['id'])
    for plan in records(catalog, 'release:'):
        if plan.get('state') != 'completed':
            require(False, 'retitle_plan_active', 'Resolve all active release plans before retitling')
    inspection = clipper.inspect(catalog, {path: None})
    require(not inspection['blockers'], 'retitle_dependencies', '; '.join(inspection['blockers']))
    for project in inspection['dependencies']['projects']:
        require(not any(clipper.overlaps(path, value) for a in project['assets'] for value in [a['path'], *a['items']]),
                'retitle_dependencies', 'Clipper references this session; a new revision requires explicit consumer adoption')


def retitle(catalog, operation_id, path, session_id, expected_manifest_sha256, project):
    operation_id, sid = str(uuid.UUID(operation_id)), str(uuid.UUID(session_id))
    require(isinstance(project, dict) and set(project) == {'version', 'name', 'country'} and type(project['version']) is int and project['version'] == 1
            and all(isinstance(project[k], str) and 0 < len(project[k]) <= 120 for k in ('name', 'country')),
            'invalid_request', 'project must be {version:1,name,country}')
    require(isinstance(path, str) and Path(path).is_absolute() and not Path(path).is_symlink(), 'unsafe_path', 'Absolute session path required')
    require(isinstance(expected_manifest_sha256, str) and len(expected_manifest_sha256) == 64
            and all(c in '0123456789abcdef' for c in expected_manifest_sha256), 'invalid_request', 'Expected raw manifest SHA-256 required')
    request = {'schema': CAPABILITY, 'operation_id': operation_id, 'path': path, 'session_id': sid,
               'expected_manifest_sha256': expected_manifest_sha256, 'project': project,
               'catalog_id': str(catalog.home)}
    # Same lock order as release: editor exclusion, then catalog writer. Both span checks and producer mutation.
    with exclusion(catalog):
        key = 'retitle:' + operation_id
        prior = catalog.get(key)
        require(not prior or prior['request'] == request, 'operation_conflict', 'operation_id reused with different payload')
        if prior and prior['state'] == 'applied':
            return prior
        require(not any(op['request']['session_id'] == sid and op['request']['operation_id'] != operation_id and op['state'] != 'applied'
                        for op in records(catalog, 'retitle:')), 'retitle_pending', 'Recover the existing operation first')
        capabilities = producer(catalog, 'capabilities', {})
        require(capabilities.get('schema') == 'stalker-capabilities/1' and CAPABILITY in capabilities.get('capabilities', []),
                'stalker_upgrade_required', 'Update/restart Stalker with ' + CAPABILITY + ' (docs/contracts/stalker-retitle-v1.md); no manifest was changed')
        # Persist intent BEFORE contacting the mutation API; ambiguity survives process death.
        if not prior:
            blockers(catalog, request)
            manifest, sha = manifest_at(path)
            require(str(uuid.UUID(manifest['sessionID'])) == sid and sha == expected_manifest_sha256,
                    'revision_mismatch', 'Stalker session changed; review metadata again')
            prior = {'schema': 'nuun-retitle-operation/1', 'request': request, 'previous_manifest': manifest, 'state': 'pending', 'updated_at': now()}
            catalog.set(key, prior)
        result = producer(catalog, 'status', request)
        require(result.get('schema') == 'stalker-retitle-result/1' and result.get('operation_id') == operation_id
                and result.get('session_id') == sid, 'stalker_invalid', 'Invalid producer status; operation remains pending')
        require(result.get('status') in {'not_found', 'applied'}, 'retitle_recovery_required', 'Producer operation is unresolved; retain session and recover in Stalker')
        if result['status'] == 'not_found':
            blockers(catalog, request)
            manifest, sha = manifest_at(path)
            require(sha == expected_manifest_sha256 and str(uuid.UUID(manifest['sessionID'])) == sid,
                    'revision_mismatch', 'Manifest changed while recovering retitle')
            result = producer(catalog, 'apply', request)
        require(result.get('schema') == 'stalker-retitle-result/1' and result.get('operation_id') == operation_id
                and result.get('session_id') == sid and result.get('status') == 'applied'
                and result.get('previous_manifest_sha256') == expected_manifest_sha256
                and result.get('project') == project and isinstance(result.get('path'), str)
                and Path(result['path']).is_absolute() and Path(result['path']).parent == Path(path).parent,
                'stalker_invalid', 'Stalker did not publish a matching result; operation remains pending')
        manifest, sha = manifest_at(result['path'])
        require(sha == result.get('manifest_sha256') and str(uuid.UUID(manifest['sessionID'])) == sid
                and manifest.get('project') == project
                and {k: v for k, v in manifest.items() if k != 'project'} ==
                    {k: v for k, v in prior['previous_manifest'].items() if k != 'project'}, 'revision_mismatch', 'Published manifest does not match the producer result')
        from .common import digest
        # Publish result + invalidation atomically; never alter a Clipper project or adopt its new revision.
        with catalog.db:
            catalog.db.execute('INSERT OR REPLACE INTO metadata VALUES(?,?)',
                               ('retitle-result:' + sid, canonical(dict(result, manifest_digest=digest(manifest))).decode()))
            prior.update(state='applied', result=result, updated_at=now())
            catalog.db.execute('INSERT OR REPLACE INTO metadata VALUES(?,?)', (key, canonical(prior).decode()))
        return prior
