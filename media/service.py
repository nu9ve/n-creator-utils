"""Single-request JSON boundary shared by native UI and CLI."""
from pathlib import Path
import contextlib
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
        if method == 'project_preview':
            from .common import slug
            return {'name': slug(params.get('name'))}
        if method.startswith('storage_'):
            from .storage_index import call
            return call(self.catalog, self.backend, method, params, self.cancel)
        if method == 'publish_revision':
            from .revisions import publish_revision
            return publish_revision(self.catalog, self.backend, cancel=self.cancel, **params)
        if method == 'asset_revisions':
            from .library import asset_record
            current = asset_record(self.catalog, params['asset_id'])
            history = [json.loads(r[0]) for r in self.catalog.db.execute('SELECT payload FROM asset_revisions WHERE asset_id=?', (current['id'],))]
            return history or [current]
        if method == 'integration_capabilities':
            from .integrations import capabilities
            return capabilities(self.catalog)
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
        if method == 'snapshot':
            return self._snapshot(params)
        if method.startswith('closure_'):
            from . import closure
            from .volumes import CachedBackend
            if method == 'closure_overview':
                return closure.overview(self.catalog)
            if method == 'closure_candidates':
                return closure.candidates(self.catalog, self.backend, params['project_id'], params.get('query'), self.cancel)
            if method == 'closure_save':
                return closure.save(self.catalog, self.backend, **params)
            if method == 'closure_link':
                return closure.link(self.catalog, params['project_id'], params['other_id'], params.get('linked', True))
            if method == 'closure_link_candidates':
                return closure.link_candidates(self.catalog, params['project_id'])
            if method == 'closure_deliveries':
                return closure.delivery_states(self.catalog, CachedBackend(self.backend), params['project_id'])
            if method == 'closure_plan':
                options = dict(params)
                review_id = options.pop('review_id', None)
                require('progress' not in options, 'invalid_request', 'Progress callbacks are internal to the local service')
                if review_id is None:
                    return closure.plan_close(self.catalog, self.backend, cancel=self.cancel, **options)
                require(isinstance(review_id, str) and str(uuid.UUID(review_id)) == review_id.lower(),
                        'invalid_request', 'Expected a review UUID')
                key = 'closure-progress:' + review_id
                last_stage, last_at = None, 0

                def progress(stage, **details):
                    nonlocal last_stage, last_at
                    current = time.monotonic()
                    if stage != last_stage or current - last_at >= 0.25:
                        self.catalog.set(key, dict(details, stage=stage))
                        last_stage, last_at = stage, current

                try:
                    return closure.plan_close(self.catalog, self.backend, cancel=self.cancel, progress=progress, **options)
                finally:
                    with self.catalog.db:
                        self.catalog.db.execute('DELETE FROM metadata WHERE key=?', (key,))
            if method == 'closure_progress':
                return self.catalog.get('closure-progress:' + params['plan_id'], {})
            if method == 'closure_apply':
                last = {'at': 0, 'done': 0}
                def progress(stage, **details):
                    current = time.monotonic()
                    if stage == 'releasing_originals':
                        last['done'] = max(last['done'], details.get('files_done', 0))
                    if current - last['at'] >= 0.25 or stage == 'release_complete':
                        self.catalog.set('closure-progress:' + params['plan_id'], dict(details, stage=stage, released_files=last['done']))
                        last['at'] = current
                try:
                    return closure.apply_close(self.catalog, self.backend, params['plan_id'], params['digest'], self.cancel, progress,
                                               confirm_discard=params.get('confirm_discard', False))
                finally:
                    # Observers only need it while the action runs; a finished closeout leaves nothing behind.
                    self._forget('closure-progress:' + params['plan_id'])
            require(False, 'unknown_method', 'Unknown closure method')
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
        if method == 'capture_overview':
            from .captures import capture_libraries
            from .projects import overview
            from .volumes import CachedBackend
            data = capture_libraries(self.catalog, params.get('override') or self.catalog.get('capture-override', ''))
            return dict(data, projects=overview(self.catalog, data['sessions'], CachedBackend(self.backend)))
        if method == 'save_capture_tag':
            from .captures import save_capture_tag
            return save_capture_tag(self.catalog, **params)
        if method == 'save_capture_tags':
            from .captures import save_capture_tags
            return save_capture_tags(self.catalog, **params)
        if method == 'touch_project':
            from .projects import touch_project
            return touch_project(self.catalog, params['key'])
        if method == 'set_project_state':
            from .projects import set_state
            return set_state(self.catalog, params['project_id'], params['state'])
        if method == 'dismiss_job':
            self.catalog.job(params['job_id'])
            require(isinstance(params.get('dismissed'), bool), 'invalid_request', 'dismissed must be boolean')
            self.catalog.set('job-dismissed:' + params['job_id'], params['dismissed'])
            return {'job_id': params['job_id'], 'dismissed': params['dismissed']}
        if method == 'finals_status':
            from .finals import finals_status
            return finals_status(self.catalog, self.backend, params.get('archive'), self.cancel)
        if method in {'plan_finals', 'plan_finals_document'}:
            from .finals import plan_params
            plan = build_plan(self.catalog, self.backend, cancel=self.cancel, **plan_params(self.catalog, self.backend, **params))
            return canonical(plan).decode('utf-8') if method == 'plan_finals_document' else plan
        if method == 'set_finals_folder':
            from .finals import set_published_folder
            return set_published_folder(self.catalog, params['profile'], params.get('folder'))
        if method == 'capture_discard_check':
            from .captures import discard_check
            return discard_check(self.catalog, params['path'], params.get('override') or self.catalog.get('capture-override', ''))
        if method == 'capture_discarded':
            from .captures import record_discard
            return record_discard(self.catalog, params['session_id'], params['name'], params['trash_path'], params['bytes'])
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
        if method == 'apply_release':
            from .release import apply_release
            with self._action_progress(params['plan_id']) as progress:
                return apply_release(self.catalog, self.backend, params['plan_id'], params['digest'], self.cancel, progress=progress)
        if method == 'relink_release':
            from .release import relink_release
            return relink_release(self.catalog, self.backend, params['plan_id'], params['digest'], self.cancel)
        if method == 'transcripts_backup':
            from .transcripts import project_backup
            progress_id = params.get('progress_id') or str(uuid.uuid4())
            with self._action_progress(progress_id) as progress:
                return project_backup(self.catalog, self.backend, params['project_id'], params['archive'], self.cancel, progress)
        if method == 'action_progress':
            return self.catalog.get('action-progress:' + params['id'], {})
        if method == 'release_plans':
            from .release import release_plans
            return release_plans(self.catalog)
        if method == 'review_release':
            from .release import review_release
            return review_release(self.catalog, self.backend, params['plan_id'], params['digest'])
        if method == 'profiles':
            from .workspaces import profiles
            return profiles(self.catalog)
        if method == 'devices':
            from .recipes import devices
            return devices(self.catalog)
        if method == 'missions_review':
            from .missions import review
            return review(self.catalog, self.backend, **params)
        if method == 'missions_incorporate':
            from .missions import incorporate
            return incorporate(self.catalog, self.backend, **params)
        if method == 'missions_status':
            from .missions import status
            return status(self.catalog, **params)
        if method == 'confirm_device':
            from .recipes import confirm_device
            return confirm_device(self.catalog, **params)
        if method == 'save_profile':
            from .workspaces import save_profile
            return save_profile(self.catalog, **params)
        if method == 'discover':
            from .planner import scan, grouped_files, pending_stalker_files
            from .layout import kind_of, stalker_facts
            from .common import read_json
            files, bundles, skipped = scan(params['source'], skip_active=True, cancel=self.cancel, stalker_only=params.get('source_kind') == 'stalker')
            if params.get('source_kind') == 'stalker' and params.get('archive'):
                files, bundles = pending_stalker_files(self.catalog, self.catalog.volume(params['archive']), files, bundles)
            groups = grouped_files(files)
            units = []
            from .recipes import recipe as current_recipe, unit_device
            rules = current_recipe(self.catalog)
            mission_indexes = {}
            for key, members in groups.items():
                evidence = [e for f in members for e in f['evidence']]
                device = unit_device(members, self.catalog, rules) if key != 'camera-support' else None
                if device and not device.get('conflict'):
                    # The planner decides the same way; the app only stops asking when no confirmation is needed.
                    metadata_device = {'device': {k: device.get(k) for k in ('make', 'model', 'source', 'label', 'category', 'rule', 'requires_confirmation')}}
                    mission = self._discover_mission(members, device, rules, params.get('archive'), mission_indexes)
                    if mission:
                        metadata_device['mission'] = mission
                else:
                    metadata_device = {}
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
                              **metadata_device, **metadata})
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
            return resolve(self.catalog, self.backend, params['asset_id'], params.get('revision'))
        if method == 'deliver_work':
            progress_id = params.get('progress_id')
            if progress_id is None:
                return deliver_work(self.catalog, self.backend, params['asset_ids'], params['work'], self.cancel)
            require(isinstance(progress_id, str) and str(uuid.UUID(progress_id)) == progress_id.lower(), 'invalid_request', 'Expected a progress UUID')
            with self._action_progress(progress_id) as progress:
                return deliver_work(self.catalog, self.backend, params['asset_ids'], params['work'], self.cancel, progress=progress)
        if method == 'set_work_state':
            return set_work_state(self.catalog, params['asset_ids'], params['state'])
        if method == 'restore':
            return restore(self.catalog, self.backend, params['asset_id'], params['destination'])
        if method == 'confirm_camera_cleanup':
            from .transfer import confirm_camera_cleanup
            return confirm_camera_cleanup(self.catalog, self.backend, params['job_id'], params['source'])
        if method == 'reconcile_sony_cleanup':
            from .transfer import reconcile_sony_cleanup
            return reconcile_sony_cleanup(self.catalog, self.backend, params['source'], params['identity'], cancel=self.cancel)
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
            return handoff(self.catalog, self.backend, params['asset_ids'], params.get('volume_id'))
        if method == 'rename_project':
            from .projects import rename
            return rename(self.catalog, params.get('key') or params['project_id'], params['title'])
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

    @contextlib.contextmanager
    def _action_progress(self, key):
        """Persist a long action's measure for the app's reader (at most every 0.25 s, always on a stage change)."""
        last = {'stage': None, 'at': 0}

        def progress(stage, **details):
            moment = time.monotonic()
            if (stage != last['stage'] or moment - last['at'] >= 0.25 or stage.endswith('complete')
                    or details.get('overall_done') == details.get('overall_total')):
                self.catalog.set('action-progress:' + key, dict(details, stage=stage))
                last.update(stage=stage, at=moment)
        try:
            yield progress
        finally:
            self._forget('action-progress:' + key)

    def _forget(self, key):
        with self.catalog.db:
            self.catalog.db.execute('DELETE FROM metadata WHERE key=?', (key,))

    def _snapshot(self, params):
        """Everything the native shell shows at rest, in one read; each mounted volume is described once."""
        from .integrations import capabilities
        from .release import release_plans
        from .volumes import CachedBackend, source_cards
        from .workspaces import profiles
        backend = CachedBackend(self.backend)
        volumes = []
        for record in self.catalog.volumes():
            current = backend.online(record)
            volumes.append(dict(record, online=bool(current), free=current['free'] if current else None))
        result = {'volumes': volumes, 'library': self.catalog.search(None, None), 'jobs': self.catalog.jobs(),
                  'profiles': profiles(self.catalog), 'projects': projects(self.catalog),
                  'release_plans': release_plans(self.catalog),
                  'integration_settings': self.call('integration_settings'),
                  'defaults': self.call('defaults'), 'errors': {}}
        from .closure import overview as closure_overview
        result['project_closures'] = closure_overview(self.catalog)
        try:
            result['capabilities'] = capabilities(self.catalog)
        except (MediaError, OSError, ValueError, RuntimeError) as error:
            result['capabilities'], result['errors']['capabilities'] = {}, str(error)
        if params.get('cards', True):
            try:
                result['source_cards'] = source_cards(self.catalog, backend, any(v['online'] for v in volumes))
            except (MediaError, OSError, ValueError) as error:
                result['source_cards'], result['errors']['source_cards'] = [], str(error)
        return result

    def _discover_mission(self, members, device, rules, archive, indexes):
        """Mission or not while inspecting, with the same rule as the planner; None without a flow or archive."""
        if device.get('requires_confirmation') or not archive:
            return None
        flow = next((f for f in rules.get('missions') or [] if device.get('category') in f.get('devices', [])), None)
        from .layout import kind_of
        still = next((f for f in sorted(members, key=lambda f: kind_of(f['path']) != 'image')
                      if (f.get('photo') or {}).get('captured_at')), None)
        if not flow or not still:
            return None
        if flow['id'] not in indexes:
            try:
                from .missions import index_for
                record = self.catalog.volume(archive)
                self.backend.check(record)
                indexes[flow['id']] = index_for(record, flow)
            except (MediaError, OSError, ValueError):
                indexes[flow['id']] = None  # archive offline: the plan decides later
        if indexes[flow['id']] is None:
            return None
        found = indexes[flow['id']].match(still['photo'], Path(still['path']).name)
        return dict(found, flow=flow['id'], flow_label=flow['label'], project=flow['project'] if found['state'] == 'matched' else None)

    def _apply_ingest(self, plan, reviewed_digest):
        if plan.get('cleanup') and plan.get('source_kind') == 'delivery':
            from .finals import clipper_blockers
            from .clipper import release_guard
            with release_guard(self.catalog):
                blockers = clipper_blockers(self.catalog, plan['source'])
                require(not blockers, 'source_in_use', 'Clipper uses these finals: ' + '; '.join(blockers[:3]))
                result = Transfer(self.catalog, self.backend, self.cancel).apply(plan, reviewed_digest)
        else:
            result = Transfer(self.catalog, self.backend, self.cancel).apply(plan, reviewed_digest)
        if any(u.get('mission') for u in plan['units']) and result.get('state') in {'source_retained', 'completed', 'camera_action_required'}:
            # The consumer's map (units.ndjson); a failure here must not hide a completed ingest.
            try:
                from .missions import write_units_map
                result['units_map'] = write_units_map(self.catalog, self.backend, plan)
            except (MediaError, OSError, ValueError, RuntimeError) as error:
                result['units_map'], result['units_map_error'] = 'failed', str(error)
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
        overall = self.catalog.get('progress:' + ingest_plan['id'], {})
        release_done = 0
        for index, unit in enumerate(units):
            release_plan = None
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
                value = dict(overall, **base_progress)
                value.update(details)
                value['stage'] = stage
                if 'global_total' in overall:
                    done = 0
                    if release_plan and stage == 'releasing_originals':
                        done = sum(e['stamp']['size'] for e in release_plan['entries'][:details.get('files_done', 0)])
                    overall['global_done'] = max(overall.get('global_done', 0), overall['backup_budget'] + release_done + done)
                    value.update(global_done=overall['global_done'], global_fraction=min(0.999, overall['global_done'] / max(1, overall['global_total'])), eta_seconds=None)
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
                if any(s['session_id'] == unit.get('session_id') for s in summary['released']):
                    release_done += sum(f['stamp']['size'] for f in ingest_plan['files'] if f['unit_id'] == unit['id'])
                update_progress('session_finished')
                self.catalog.set('progress:' + ingest_plan['id'], {
                    **self.catalog.get('progress:' + ingest_plan['id'], {}), 'sessions_done': index + 1, 'stage': 'session_finished',
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
            **self.catalog.get('progress:' + ingest_plan['id'], {}),
            'job_id': ingest_plan['id'], 'phase': 'stalker_release_done',
            'sessions_total': len(units), 'sessions_done': len(units),
            **({'global_fraction': 1.0, 'global_done': overall.get('global_total', 0)} if not summary['retained'] else {}),
        })
        return summary
