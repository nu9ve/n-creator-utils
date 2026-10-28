"""Single-request JSON boundary shared by native UI and CLI."""
from pathlib import Path
import json
import time
import uuid

from .catalog import Catalog
from .common import MediaError, require, relative, canonical
from .library import deliver_work, handoff, relayout, resolve, restore, set_work_state, sources, projects
from .planner import build_plan
from .transfer import Transfer
from .volumes import VolumeBackend


class Service:
    def __init__(self, catalog, backend=None, cancel=None):
        self.catalog, self.backend, self.cancel = catalog, backend or VolumeBackend(), cancel

    def call(self, method, params=None):
        params = params or {}
        require(isinstance(params, dict), 'invalid_request', 'params must be an object')
        if method == 'retitle_capture':
            from .retitle import retitle
            return retitle(self.catalog, **params)
        if method == 'cancel_plan':
            from .retitle import cancel_plan
            return cancel_plan(self.catalog, params['plan_id'])
        if method == 'retitle_operations':
            from .retitle import records
            return records(self.catalog, 'retitle:')
        if method == 'source_cards':
            from .volumes import source_cards
            return source_cards(self.catalog, self.backend)
        if method == 'eject_card':
            from .volumes import eject_card
            return eject_card(self.catalog, self.backend, params.get('source'), params.get('identity'))
        if method == 'volumes':
            return [dict(v, online=bool(self.backend.online(v))) for v in self.catalog.volumes()]
        if method == 'register_volume':
            root = Path(params['root']).expanduser().absolute()
            info = self.backend.describe(root, external_required=not params.get('allow_local', False))
            prefix = params.get('prefix') or ('001_raw' if params['role'] == 'archive' else '001_proyectos')
            with self.catalog.writer():
                return self.catalog.register(params['name'], params['role'], root, info, prefix)
        if method == 'captures':
            from .captures import captures
            return captures(self.catalog, **params)
        if method == 'capture_libraries':
            from .captures import capture_libraries
            return capture_libraries(self.catalog, params.get('override') or self.catalog.get('capture-override', ''))
        if method == 'save_capture_tag':
            from .captures import save_capture_tag
            return save_capture_tag(self.catalog, **params)
        if method == 'integration_settings':
            if params:
                allowed = {'clipper_executable', 'stalker_executable', 'capture_override', 'retain_stalker_source'}
                require(not set(params) - allowed
                        and all(isinstance(v, str) for key, v in params.items() if key != 'retain_stalker_source')
                        and ('retain_stalker_source' not in params or isinstance(params['retain_stalker_source'], bool)),
                        'invalid_settings', 'Invalid integration settings')
                with self.catalog.writer():
                    for key, value in params.items(): self.catalog.set(key.replace('_', '-'), value)
            return {**{key: self.catalog.get(key.replace('_', '-'), '') for key in ('clipper_executable', 'stalker_executable', 'capture_override')},
                    'retain_stalker_source': self.catalog.get('retain-stalker-source', False)}
        if method == 'backup_metadata':
            from .metadata_backup import backup_metadata
            return backup_metadata(self.catalog, self.backend, params['archive'], self.cancel)
        if method == 'plan_release':
            from .release import plan_release
            require('progress' not in params, 'invalid_request', 'Progress callbacks are internal to the local service')
            return plan_release(self.catalog, self.backend, cancel=self.cancel, **params)
        if method in {'apply_release', 'relink_release'}:
            from .release import apply_release, relink_release
            action = apply_release if method == 'apply_release' else relink_release
            return action(self.catalog, self.backend, params['plan_id'], params['digest'], self.cancel)
        if method == 'release_plans':
            from .release import release_plans
            return release_plans(self.catalog)
        if method == 'review_release':
            from .release import review_release
            return review_release(self.catalog, self.backend, params['plan_id'], params['digest'])
        if method == 'profiles':
            from .workspaces import profiles
            return profiles(self.catalog)
        if method == 'save_profile':
            from .workspaces import save_profile
            return save_profile(self.catalog, **params)
        if method == 'discover':
            from .planner import scan, group_key, pending_stalker_files
            from .layout import kind_of, stalker_facts
            from .common import read_json
            files, bundles, skipped = scan(params['source'], skip_active=True, cancel=self.cancel, stalker_only=params.get('source_kind') == 'stalker')
            if params.get('source_kind') == 'stalker' and params.get('archive'):
                files, bundles = pending_stalker_files(self.catalog, self.catalog.volume(params['archive']), files, bundles)
            groups = {}
            for file in files:
                groups.setdefault(group_key(file), []).append(file)
            units = []
            for key, members in groups.items():
                evidence = [e for f in members for e in f['evidence']]
                kinds = [kind_of(f['path']) for f in members]
                bundle = members[0]['bundle']
                metadata = {}
                if bundle is not None:
                    from .captures import project_hint, preview_track_path
                    manifest = read_json(Path(params['source']) / ('' if bundle == '.' else bundle) / 'manifest.json', 1024 * 1024)
                    facts = stalker_facts(manifest, min(f['stamp']['mtime_ns'] for f in members))
                    track_preview = preview_track_path(manifest)
                    preview_path = ('' if bundle == '.' else bundle + '/') + track_preview if track_preview else ''
                    metadata = {'session_id': manifest['sessionID'], 'created_at': facts['created'].isoformat(),
                                'duration_seconds': facts.get('seconds'), 'track_count': len(manifest.get('tracks', [])),
                                'preview_path': preview_path,
                                'project_hint': project_hint(manifest),
                                'tag': self.catalog.get('capture-tag:' + str(uuid.UUID(manifest['sessionID'])), {})}
                units.append({'id': key, 'paths': [f['path'] for f in members],
                              'bytes': sum(f['stamp']['size'] for f in members), 'evidence': evidence,
                              'category': 'stalker' if bundle is not None else next((k for k in kinds if k in {'video', 'image', 'raw_image', 'audio'}), 'support'),
                              'camera': next((e['model'] for e in evidence if e.get('model')), '') or next((e['family'] for e in evidence if e.get('family')), ''),
                              **metadata})
            return {'units': units, 'skipped': skipped, 'bytes': sum(f['stamp']['size'] for f in files)}
        if method in {'plan', 'plan_document'}:
            plan = build_plan(self.catalog, self.backend, cancel=self.cancel, **params)
            # Native clients display a decoded view but return the original document untouched.
            return canonical(plan).decode('utf-8') if method == 'plan_document' else plan
        if method == 'apply':
            plan = params['plan']
            if isinstance(plan, str):
                require(len(plan.encode('utf-8')) <= 12 * 1024 * 1024, 'plan_too_large', 'Plan document exceeds 12 MiB')
                plan = json.loads(plan)
            return self._apply_ingest(plan, params['digest'])
        if method == 'recover_catalog':
            from .recovery import recover_catalog
            return recover_catalog(self.catalog, self.backend, params['volume'])
        if method == 'resume':
            job = self.catalog.job(params['job_id'])
            require(job['state'] != 'recovered', 'source_not_resumable', 'Recovered archive jobs are read-only; use restore or deliver-work')
            return self._apply_ingest(job['plan'], job['digest'])
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
        if method == 'confirm_camera_cleanup':
            from .transfer import confirm_camera_cleanup
            return confirm_camera_cleanup(self.catalog, self.backend, params['job_id'], params['source'])
        if method == 'coverage':
            from .coverage import coverage
            return coverage(self.catalog, self.backend, params['work'], params['archive'], params.get('path') or '',
                            not params.get('no_hash', False), self.cancel)
        if method == 'projects':
            return projects(self.catalog)
        if method == 'sources':
            return sources(self.catalog, self.backend, params['project'], params.get('purpose') or 'edit')
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

    def _apply_ingest(self, plan, reviewed_digest):
        result = Transfer(self.catalog, self.backend, self.cancel).apply(plan, reviewed_digest)
        release_summary = None
        if plan.get('release_stalker_source') and result['state'] in {'source_retained', 'completed'}:
            release_summary = self._release_stalker_sources(plan)
            self.catalog.set('stalker-release:' + plan['id'], release_summary)
            result = self.catalog.job(plan['id'])
            result['stalker_release'] = release_summary
        # A metadata-backup failure must not hide a successfully completed ingest.
        if not (release_summary and release_summary['released']):
            try:
                from .metadata_backup import backup_metadata
                result['metadata_backup'] = backup_metadata(self.catalog, self.backend, plan['archive_id'], self.cancel)
            except (OSError, RuntimeError) as error:
                result['metadata_backup_error'] = str(error)
        return result

    def _release_stalker_sources(self, ingest_plan):
        """Release Stalker bundles after receipts exist; resume journaled releases on retry."""
        from .release import apply_release, plan_release, release_plans, relink_release, review_release

        units = [unit for unit in ingest_plan['units'] if unit.get('kind') == 'stalker_session']
        summary = {'status': 'released', 'released': [], 'retained': []}
        last_progress = {'at': 0.0, 'stage': None}
        for index, unit in enumerate(units):
            source = Path(ingest_plan['source']).expanduser().absolute()
            if unit.get('bundle') and unit['bundle'] != '.':
                source = source / unit['bundle']
            project = unit.get('project') or 'Stalker'
            base_progress = {'job_id': ingest_plan['id'], 'phase': 'releasing_stalker',
                             'sessions_total': len(units), 'sessions_done': index,
                             'session_index': index + 1, 'session_name': source.name,
                             'files_total': len(unit.get('paths', [])), 'files_done': 0}

            def update_progress(stage, **details):
                current = time.monotonic()
                force = stage != last_progress['stage']
                if not force and current - last_progress['at'] < 1:
                    return
                value = dict(base_progress)
                value.update(details)
                value['stage'] = stage
                self.catalog.set('progress:' + ingest_plan['id'], value)
                last_progress.update(at=current, stage=stage)

            update_progress('preparing')
            try:
                prior = next((candidate for candidate in reversed(release_plans(self.catalog))
                              if candidate.get('auto_ingest_job_id') == ingest_plan['id']
                              and candidate.get('capture_manifest_digest') == unit.get('capture_manifest_digest')
                              and candidate.get('archive_id') == ingest_plan['archive_id']), None)
                if prior and prior.get('state') == 'completed':
                    summary['released'].append({'session_id': unit.get('session_id'), 'path': str(source)})
                    release_plan = None
                elif prior:
                    release_plan = review_release(self.catalog, self.backend, prior['id'], prior['digest'])
                else:
                    release_plan = plan_release(
                        self.catalog, self.backend, ingest_plan['archive_id'], source=str(source),
                        project=project, cancel=self.cancel, auto_ingest_job_id=ingest_plan['id'],
                        progress=update_progress)
            except MediaError as error:
                summary['retained'].append({'session_id': unit.get('session_id'), 'path': str(source), 'reason': str(error)})
                release_plan = None

            try:
                if release_plan is not None:
                    consumers = release_plan.get('consumers') or {}
                    if consumers.get('blockers'):
                        summary['retained'].append({'session_id': unit.get('session_id'), 'path': str(source),
                                                    'reason': '; '.join(consumers['blockers'])})
                    elif consumers.get('relinks'):
                        try:
                            relink_release(self.catalog, self.backend, release_plan['id'], release_plan['digest'],
                                           self.cancel, progress=update_progress)
                        except MediaError as error:
                            summary['retained'].append({'session_id': unit.get('session_id'), 'path': str(source), 'reason': str(error)})
                        else:
                            completed = apply_release(self.catalog, self.backend, release_plan['id'], release_plan['digest'],
                                                      self.cancel, progress=update_progress)
                            if completed.get('state') == 'completed':
                                summary['released'].append({'session_id': unit.get('session_id'), 'path': str(source)})
                            else:
                                summary['retained'].append({'session_id': unit.get('session_id'), 'path': str(source),
                                                            'reason': 'La liberación no terminó; el original se conserva para reintentar'})
                    else:
                        completed = apply_release(self.catalog, self.backend, release_plan['id'], release_plan['digest'],
                                                  self.cancel, progress=update_progress)
                        if completed.get('state') == 'completed':
                            summary['released'].append({'session_id': unit.get('session_id'), 'path': str(source)})
                        else:
                            summary['retained'].append({'session_id': unit.get('session_id'), 'path': str(source),
                                                        'reason': 'La liberación no terminó; el original se conserva para reintentar'})
            finally:
                self.catalog.set('progress:' + ingest_plan['id'], {
                    **base_progress, 'sessions_done': index + 1, 'stage': 'session_finished',
                })
                last_progress['stage'] = 'session_finished'
                last_progress['at'] = time.monotonic()
        if summary['retained']:
            summary['status'] = 'partial' if summary['released'] else 'retained'
            self.catalog.state(ingest_plan['id'], 'source_retained')
        else:
            summary['status'] = 'released'
            self.catalog.state(ingest_plan['id'], 'completed')
        self.catalog.set('progress:' + ingest_plan['id'], {
            'job_id': ingest_plan['id'], 'phase': 'stalker_release_done',
            'sessions_total': len(units), 'sessions_done': len(units),
        })
        return summary
