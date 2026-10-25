"""Single-request JSON boundary shared by native UI and CLI."""
from pathlib import Path

from .catalog import Catalog
from .common import require, relative
from .library import deliver_work, handoff, relayout, resolve, restore, set_work_state
from .planner import build_plan
from .transfer import Transfer
from .volumes import VolumeBackend


class Service:
    def __init__(self, catalog, backend=None, cancel=None):
        self.catalog, self.backend, self.cancel = catalog, backend or VolumeBackend(), cancel

    def call(self, method, params=None):
        params = params or {}
        require(isinstance(params, dict), 'invalid_request', 'params must be an object')
        if method == 'volumes':
            return [dict(v, online=bool(self.backend.online(v))) for v in self.catalog.volumes()]
        if method == 'register_volume':
            root = Path(params['root']).expanduser().absolute()
            info = self.backend.describe(root, external_required=not params.get('allow_local', False))
            prefix = params.get('prefix') or ('001_raw' if params['role'] == 'archive' else '001_proyectos')
            return self.catalog.register(params['name'], params['role'], root, info, prefix)
        if method == 'captures':
            from .captures import captures
            return captures(self.catalog, **params)
        if method == 'discover':
            from .planner import scan, group_key
            files, bundles, skipped = scan(params['source'], skip_active=True, cancel=self.cancel)
            groups = {}
            for file in files:
                groups.setdefault(group_key(file), []).append(file)
            return {'units': [{'id': key, 'paths': [f['path'] for f in members], 'bytes': sum(f['stamp']['size'] for f in members), 'evidence': [e for f in members for e in f['evidence']]} for key, members in groups.items()], 'skipped': skipped}
        if method == 'plan':
            return build_plan(self.catalog, self.backend, cancel=self.cancel, **params)
        if method == 'apply':
            return Transfer(self.catalog, self.backend, self.cancel).apply(params['plan'], params['digest'])
        if method == 'recover_catalog':
            from .recovery import recover_catalog
            return recover_catalog(self.catalog, self.backend, params['volume'])
        if method == 'resume':
            job = self.catalog.job(params['job_id'])
            require(job['state'] != 'recovered', 'source_not_resumable', 'Recovered archive jobs are read-only; use restore or deliver-work')
            return Transfer(self.catalog, self.backend, self.cancel).apply(job['plan'], job['digest'])
        if method == 'jobs':
            return self.catalog.jobs()
        if method == 'job':
            return self.catalog.job(params['job_id'])
        if method == 'library':
            return self.catalog.search(params.get('project'), params.get('work_state'))
        if method == 'resolve':
            return resolve(self.catalog, self.backend, params['asset_id'])
        if method == 'deliver_work':
            return deliver_work(self.catalog, self.backend, params['asset_ids'], params['work'], self.cancel)
        if method == 'set_work_state':
            return set_work_state(self.catalog, params['asset_ids'], params['state'])
        if method == 'restore':
            return restore(self.catalog, self.backend, params['asset_id'], params['destination'])
        if method == 'relayout':
            return relayout(self.catalog, self.backend, params['job_id'])
        if method == 'handoff':
            return handoff(self.catalog, self.backend, params['asset_ids'])
        if method == 'rename_project':
            return self.catalog.rename_project(params['project_id'], params['title'])
        if method == 'set_publication':
            from .publication import set_publication
            return set_publication(self.catalog, **params)
        if method == 'publications':
            return [dict(row) for row in self.catalog.db.execute('SELECT * FROM publications ORDER BY updated_at DESC')]
        if method == 'defaults':
            if params:
                allowed = {'project', 'profile', 'country', 'city', 'archive', 'work'}
                require(not set(params) - allowed and all(isinstance(v, str) for v in params.values()),
                        'invalid_settings', 'Invalid defaults')
                self.catalog.set('defaults', dict(self.catalog.get('defaults', {}), **params))
            return self.catalog.get('defaults', {'profile': 'e64', 'country': 'mx', 'city': 'hermosillo'})
        if method == 'suggest':
            # Cheap destination gate precedes any source discovery/probing.
            online = [v for v in self.catalog.volumes() if self.backend.online(v)]
            if not online:
                return {'show': False, 'reason': 'no_registered_destinations', 'pending_work': []}
            pending = self.catalog.search(work_state='pending')
            source = params.get('source')
            if not source:
                return {'show': False, 'reason': 'no_source', 'pending_work': pending}
            root = Path(source).absolute()
            if any(Path(v['root']) == root or Path(v['root']) in root.parents for v in self.catalog.volumes()):
                return {'show': False, 'reason': 'destination_volume', 'pending_work': pending}
            info = self.backend.describe(root)
            if any(v['identity']['volume_uuid'] == info['volume_uuid'] for v in self.catalog.volumes()):
                return {'show': False, 'reason': 'destination_volume', 'pending_work': pending}
            is_card = info['external'] and info['mount'] == str(root) and info['removable']
            return {'show': is_card, 'reason': 'card' if is_card else 'not_a_card', 'pending_work': pending}
        require(False, 'unknown_method', 'Unknown media service method: ' + str(method))
