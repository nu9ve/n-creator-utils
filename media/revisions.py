"""Publish a project-only Stalker revision from a separate verified copy.

Stalker alone patches the manifest. Old jobs/copies/portable receipts remain immutable.
The operation journal precedes copying and the producer keeps both inherited locks.
"""
import copy
import json
from pathlib import Path
import uuid

from .common import Tree, canonical, digest, now, require
from .library import asset_record
from .retitle import CAPABILITY, exclusion, manifest_at, producer
from .transfer import Transfer


def inventory(catalog, asset, volume_id):
    rows = {r['path']: r for r in (json.loads(row[0]) for row in catalog.db.execute(
        'SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'], volume_id))) if r['path'] in asset['paths']}
    require(set(rows) == set(asset['paths']), 'missing_receipt', 'Complete revision receipts required')
    require(digest([dict(path=p, sha256=rows[p]['sha256']) for p in asset['paths']]) == asset['revision'],
            'revision_mismatch', 'Receipt inventory differs from the asset revision')
    return rows


def verify(catalog, backend, asset, volume_id, rows):
    from .release import checked_hash
    from .planner import scan
    volume = catalog.volume(volume_id)
    backend.check(volume)
    prefix = '' if asset['bundle'] == '.' else asset['bundle'] + '/'
    sample = rows[prefix + 'manifest.json']
    root = sample['destination'][:-len('manifest.json') - 1]
    with Tree(volume['root']) as tree:
        for path, row in rows.items():
            require(row['destination'] == root + '/' + path[len(prefix):], 'revision_mismatch', 'Member outside revision bundle')
            sha, stamp = checked_hash(tree, row['destination'])
            require(sha == row['sha256'] and stamp['size'] == row['bytes'], 'revision_mismatch', 'Revision bytes changed')
    files, _, _ = scan(str(Path(volume['root']) / root))
    require({f['path'] for f in files} == {p[len(prefix):] for p in rows}, 'revision_mismatch', 'Revision membership changed')
    return str(Path(volume['root']) / root)


def publish_revision(catalog, backend, operation_id, asset_id, expected_revision, project, archive, cancel=None, hook=None):
    operation_id = str(uuid.UUID(operation_id))
    require(isinstance(project, dict) and set(project) == {'version', 'name', 'country'}
            and type(project['version']) is int and project['version'] == 1
            and all(isinstance(project[k], str) and 0 < len(project[k]) <= 120 for k in ('name', 'country')),
            'invalid_request', 'project must be {version:1,name,country}')
    volume = catalog.volume(archive)
    require(volume['role'] == 'archive', 'invalid_role', 'Publish to a registered archive')
    request = dict(operation_id=operation_id, asset_id=asset_id, expected_revision=expected_revision, project=project, archive=volume['id'])
    key = 'revision-publication:' + operation_id
    def checkpoint(stage):
        if hook: hook(stage)
    with exclusion(catalog):
        op = catalog.get(key)
        require(not op or op['request'] == request, 'operation_conflict', 'operation_id reused with another payload')
        if op and op['state'] == 'published':
            return op
        current = asset_record(catalog, asset_id)
        require(current['kind'] == 'stalker_session' and current['revision'] == expected_revision,
                'revision_mismatch', 'Review the current Stalker asset revision')
        operations = [json.loads(r[0]) for r in catalog.db.execute(
            "SELECT value FROM metadata WHERE key LIKE 'revision-publication:%'")]
        require(not any(value['state'] != 'published' and value['request']['asset_id'] == asset_id
                        and value['request']['operation_id'] != operation_id for value in operations),
                'revision_pending', 'Recover the existing publication with its operation ID')
        capabilities = producer(catalog, 'capabilities', {})
        require(capabilities.get('schema') == 'stalker-capabilities/1' and CAPABILITY in capabilities.get('capabilities', []),
                'stalker_upgrade_required', 'Stalker coordinated writer unavailable')
        if not op:
            old_rows = inventory(catalog, current, volume['id'])
            source = verify(catalog, backend, current, volume['id'], old_rows)
            manifest, sha = manifest_at(source)
            require(str(uuid.UUID(manifest['sessionID'])) == str(uuid.UUID(current['session_id'])),
                    'revision_mismatch', 'Archived session identity differs')
            require(manifest.get('project') != project, 'revision_mismatch', 'Project has not changed')
            parent = volume['prefix'] + '/revisions/' + operation_id
            staging = parent + '/revision.stalker'
            op = dict(schema='nuun-revision-publication/1', request=request, state='pending',
                      previous_asset=current, previous_copies=old_rows, previous_manifest=manifest,
                      previous_manifest_sha256=sha, staging=staging, parent=parent, updated_at=now())
            catalog.set(key, op)
        old = op['previous_asset']; rows = op['previous_copies']
        verify(catalog, backend, old, volume['id'], rows)
        prefix = '' if old['bundle'] == '.' else old['bundle'] + '/'
        producer_request = dict(schema=CAPABILITY, operation_id=operation_id,
                                path=str(Path(volume['root']) / op['staging']), session_id=str(uuid.UUID(old['session_id'])),
                                expected_manifest_sha256=op['previous_manifest_sha256'], project=project, catalog_id=str(catalog.home))
        backend.check(volume)
        with Tree(volume['root']) as tree:
            with tree.parent(op['staging'], True):
                pass
        status = producer(catalog, 'status', producer_request)
        require(status.get('schema') == 'stalker-retitle-result/1' and status.get('operation_id') == operation_id
                and status.get('session_id') == producer_request['session_id'] and status.get('status') in {'not_found', 'pending', 'applied'},
                'stalker_invalid', 'Unresolved producer status; retain both revisions')
        if status['status'] == 'pending':
            require(Path(status.get('path') or '').parent == Path(producer_request['path']).parent
                    and status.get('previous_manifest_sha256') == op['previous_manifest_sha256']
                    and status.get('project') == project and isinstance(status.get('manifest_sha256'), str)
                    and len(status['manifest_sha256']) == 64
                    and all(c in '0123456789abcdef' for c in status['manifest_sha256']),
                    'stalker_invalid', 'Pending producer journal differs from the publication')
        if status['status'] == 'not_found':
            engine = Transfer(catalog, backend, cancel)
            with Tree(volume['root']) as tree:
                for path, row in rows.items():
                    engine.copy_file(tree, row['destination'], tree, op['staging'] + '/' + path[len(prefix):], volume, expected=row['sha256'])
            checkpoint('copy_verified')
        if status['status'] != 'applied':
            status = producer(catalog, 'apply', producer_request)
        target = status.get('path', '')
        require(status.get('schema') == 'stalker-retitle-result/1' and status.get('status') == 'applied'
                and status.get('operation_id') == operation_id and status.get('session_id') == producer_request['session_id']
                and status.get('previous_manifest_sha256') == op['previous_manifest_sha256'] and status.get('project') == project
                and Path(target).parent == Path(producer_request['path']).parent and Path(target).suffix == '.stalker',
                'stalker_invalid', 'Producer did not ACK the isolated copy')
        checkpoint('producer_applied')
        manifest, sha = manifest_at(target)
        require(sha == status.get('manifest_sha256') and manifest.get('project') == project
                and {k:v for k,v in manifest.items() if k != 'project'} == {k:v for k,v in op['previous_manifest'].items() if k != 'project'},
                'revision_mismatch', 'Only manifest.project may change')
        from .planner import scan
        files, _, _ = scan(target)
        require({f['path'] for f in files} == {p[len(prefix):] for p in rows}, 'revision_mismatch', 'Membership changed')
        from .release import checked_hash
        copies, hashes = [], []
        receipt_id = str(uuid.uuid5(uuid.NAMESPACE_URL, operation_id + ':' + volume['id']))
        target_rel = str(Path(target).relative_to(volume['root']))
        with Tree(volume['root']) as tree:
            for path in old['paths']:
                inner = path[len(prefix):]
                dest = target_rel + '/' + inner
                new_sha, stamp = checked_hash(tree, dest, cancel)
                require((inner == 'manifest.json' and new_sha == sha) or
                        (inner != 'manifest.json' and new_sha == rows[path]['sha256'] and stamp['size'] == rows[path]['bytes']),
                        'revision_mismatch', 'Media/events must remain identical')
                copies.append(dict(job_id=operation_id, volume_id=volume['id'], path=path, destination=dest,
                                   sha256=new_sha, bytes=stamp['size'], stamp=stamp, receipt_id=receipt_id, verified_at=now()))
                hashes.append(dict(path=path, sha256=new_sha))
            verify(catalog, backend, old, volume['id'], rows)
            new = dict(old, job_id=operation_id, revision=digest(hashes), capture_manifest_digest=digest(manifest), source=target)
            plan = copy.deepcopy(catalog.job(old['job_id'])['plan'])
            unit = {k:v for k,v in new.items() if k not in {'job_id', 'revision', 'source', 'project_id'}}
            plan.update(id=operation_id, source=target, cleanup=False, release_stalker_source=False, units=[unit],
                        archive_id=volume['id'], work_id=None, archive_only_confirmed=True, work_delivery_status='pending', expected_work_id=plan.get('expected_work_id'),
                        files=[dict(path=c['path'], unit_id=asset_id, stamp=c['stamp'], bundle=old['bundle'], evidence=[]) for c in copies],
                        layout={c['path']:dict(archive=c['destination'][len(volume['prefix'])+1:],
                            work=old['profile'] + '_raw/' + old['project'] + '/revisions/' + operation_id + '/' + Path(target).name + '/' + c['path'][len(prefix):]) for c in copies})
            plan.pop('digest', None); plan['digest'] = digest(plan)
            from .planner import validate_plan
            validate_plan(plan)
            tree.write_json('.ncreator/receipts/' + operation_id + '.json', dict(
                schema='nu9ve-transfer-receipt/1', receipt_id=receipt_id, job_id=operation_id, plan_digest=plan['digest'],
                volume_id=volume['id'], verified_at=now(), files=copies, units=[dict(unit, project_id=old['project_id'])], plan=plan,
                country=plan['country'], city=plan['city'], work_delivery_status='pending',
                revision_publication=dict(previous_asset=old, asset=new, operation_id=operation_id)))
        checkpoint('receipt_durable')
        op.update(state='published', asset=new, result=status, updated_at=now())
        # History, current pointer, copies and publication ACK become visible together.
        with catalog.db:
            for asset in (old, new):
                catalog.db.execute('INSERT OR IGNORE INTO asset_revisions VALUES(?,?,?)', (asset_id, asset['revision'], canonical(asset).decode()))
            catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', (operation_id, plan['digest'], canonical(plan).decode(), 'recovered', None, now(), now()))
            for c in copies:
                catalog.db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', (operation_id, volume['id'], c['path'], c['sha256'], canonical(c).decode()))
                catalog.db.execute('INSERT INTO members(job_id,relative_path,sha256) VALUES(?,?,?)', (operation_id, c['path'], c['sha256']))
            catalog.db.execute('INSERT INTO asset_jobs VALUES(?,?)', (asset_id, operation_id))
            catalog.db.execute('INSERT OR IGNORE INTO sessions VALUES(?,?,?)', (new['session_id'], new['capture_manifest_digest'], asset_id))
            catalog.db.execute('UPDATE assets SET job_id=?,payload=? WHERE id=?', (operation_id, canonical(new).decode(), asset_id))
            catalog.db.execute('UPDATE work SET state=?,target=NULL,updated_at=? WHERE asset_id=?', ('pending', now(), asset_id))
            catalog.db.execute('INSERT OR REPLACE INTO metadata VALUES(?,?)', (key, canonical(op).decode()))
        checkpoint('published')
        return op
