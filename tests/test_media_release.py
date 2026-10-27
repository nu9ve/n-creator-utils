"""Release/destructive-boundary checks use tiny files and fixture identities only."""
import copy
import json
import os
from pathlib import Path
import sqlite3
import uuid
from unittest import mock

from test_media_engine import MediaFixture
from media import clipper
from media.captures import capture_libraries, save_capture_tag
from media.common import MediaError, digest
from media.coverage import coverage
from media.metadata_backup import backup_metadata
from media.release import plan_release, apply_release, relink_release
from media.service import Service


def empty_deps():
    return {'schema': 'clipper-library-deps/1', 'projects': [], 'offline_projects': []}


class WorkflowFixture(MediaFixture):
    def setUp(self):
        super().setUp()
        patcher = mock.patch('pathlib.Path.home', return_value=self.root)
        patcher.start(); self.addCleanup(patcher.stop)
        environment = mock.patch.dict(os.environ, {'CLIPPER_PROJECTS_ROOT': str(self.root / 'Downloads/clipper-projects')})
        environment.start(); self.addCleanup(environment.stop)


class ReleaseTests(WorkflowFixture):
    def setUp(self):
        super().setUp()
        # dependencies receives the catalog; fixtures never execute the user's Clipper.
        patcher = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        self.deps = patcher.start(); self.addCleanup(patcher.stop)
        self.run_plan()
        self.folder = '001_proyectos/e64_raw/intro'

    def release(self):
        return plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path=self.folder, project='intro')

    def apply(self, plan, **kwargs):
        return apply_release(self.catalog, self.backend, plan['id'], plan['digest'], **kwargs)

    def test_release_verifies_archive_and_keeps_editor_and_renders(self):
        folder = self.work / self.folder
        (folder / 'clipper').mkdir(); (folder / 'clipper/clipper-project.json').write_text('{}')
        (folder / 'renders').mkdir(); (folder / 'renders/final.MP4').write_bytes(b'final')
        (folder / 'notes.txt').write_text('human notes')
        plan = self.release()
        self.assertEqual(len(plan['entries']), 2)
        self.assertEqual(len(plan['preserved']), 3)
        self.assertEqual(self.apply(plan)['state'], 'completed')
        self.assertTrue((folder / 'renders/final.MP4').exists())
        self.assertTrue((folder / 'clipper/clipper-project.json').exists())
        self.assertTrue((folder / 'notes.txt').exists())
        self.assertTrue(list(self.archive.glob('.ncreator/catalog-backups/*.sqlite3')))
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_archive_disconnected_before_apply_retains_everything(self):
        plan = self.release(); self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_same_size_archive_corruption_blocks_all_deletion(self):
        plan = self.release()
        target = self.archive / plan['entries'][0]['archive_path']
        target.write_bytes(b'x' * target.stat().st_size)
        with self.assertRaisesRegex(MediaError, 'Archive changed'): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_source_modified_after_review_blocks_deletion(self):
        plan = self.release(); (self.work / plan['entries'][0]['path']).write_bytes(b'mutated')
        with self.assertRaisesRegex(MediaError, 'changed'): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_same_physical_archive_blocks_plan(self):
        self.backend.physical[str(self.work)] = str(self.archive)
        with self.assertRaisesRegex(MediaError, 'independent'): self.release()

    def test_new_material_after_review_blocks_deletion(self):
        plan = self.release(); (self.work / self.folder / 'new.JPG').write_bytes(b'new')
        with self.assertRaisesRegex(MediaError, 'New material'): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_cancel_and_crash_after_unlink_can_resume(self):
        plan = self.release()
        with self.assertRaisesRegex(MediaError, 'cancelled|paused'): self.apply(plan, cancel=lambda: True)
        def crash(stage, entry):
            if stage == 'after_unlink': raise RuntimeError('power loss')
        with self.assertRaisesRegex(RuntimeError, 'power loss'): self.apply(plan, hook=crash)
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_crash_before_journal_commit_after_unlink_can_resume(self):
        plan = self.release()
        from media.common import Tree
        real = Tree.unlink
        def crash(tree, path, expected=None):
            real(tree, path, expected)
            raise RuntimeError('power loss after unlink')
        with mock.patch.object(Tree, 'unlink', crash), self.assertRaises(RuntimeError): self.apply(plan)
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_missing_source_without_intent_is_not_adopted(self):
        plan = self.release(); (self.work / plan['entries'][0]['path']).unlink()
        with self.assertRaisesRegex(MediaError, 'missing'): self.apply(plan)

    def test_unknown_or_unreviewed_plan_cannot_delete(self):
        plan = self.release()
        with self.assertRaisesRegex(MediaError, 'recorded'): apply_release(self.catalog, self.backend, plan['id'], 'wrong')
        with self.assertRaisesRegex(MediaError, 'recorded'): apply_release(self.catalog, self.backend, str(uuid.uuid4()), plan['digest'])

    def test_legacy_renamed_identical_files_can_release_but_uncovered_cannot(self):
        folder = self.work / '001_proyectos/old_raw/project'; folder.mkdir(parents=True)
        (folder / 'old.JPG').write_bytes(b'identical')
        (self.archive / 'historical.JPG').write_bytes(b'identical')
        plan = plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path='001_proyectos/old_raw/project', project='old')
        self.assertEqual(self.apply(plan)['state'], 'completed')
        folder.mkdir(exist_ok=True); (folder / 'only.JPG').write_bytes(b'only here')
        with self.assertRaisesRegex(MediaError, 'archive first'): plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path='001_proyectos/old_raw/project', project='old')
        self.assertTrue((folder / 'only.JPG').exists())

    def test_clipper_missing_or_offline_or_queued_blocks_release(self):
        self.deps.side_effect = MediaError('clipper_unavailable', 'missing')
        plan = self.release(); self.assertTrue(plan['consumers']['blockers'])
        with self.assertRaises(MediaError): self.apply(plan)
        self.deps.side_effect = None
        self.deps.return_value = dict(empty_deps(), offline_projects=[{'clipper_root': '/offline'}])
        plan = self.release()
        with self.assertRaisesRegex(MediaError, 'Resolve Clipper'): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def dependency(self, plan, queued=False):
        path = str(self.work / plan['entries'][0]['path'])
        return dict(empty_deps(), projects=[{'clipper_root': '/fixture/clipper/project', 'assets': [
            {'asset_id': 'asset', 'revision': 'revision', 'path': path, 'in_use': True, 'items': [path], 'sync_groups': [], 'queued_exports': ['export'] if queued else []}], 'relinks': []}])

    def test_explicit_relink_ack_then_fresh_deps_required(self):
        initial = self.release(); deps = self.dependency(initial)
        self.deps.side_effect = None; self.deps.return_value = deps
        plan = self.release(); self.assertEqual(len(plan['consumers']['relinks']), 1)
        with self.assertRaisesRegex(MediaError, 'Resolve Clipper'): self.apply(plan)
        def relink(catalog, request, op):
            self.deps.return_value = empty_deps()
            return {'operation_id': op}
        with mock.patch('media.clipper.relink', side_effect=relink):
            result = relink_release(self.catalog, self.backend, plan['id'], plan['digest'])
        self.assertEqual(len(result['acks']), 1)
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_queue_cannot_be_overridden_by_relink(self):
        initial = self.release(); self.deps.side_effect = None; self.deps.return_value = self.dependency(initial, queued=True)
        plan = self.release(); self.assertTrue(plan['consumers']['blockers'])
        with mock.patch('media.clipper.relink') as relink:
            with self.assertRaisesRegex(MediaError, 'cola'): relink_release(self.catalog, self.backend, plan['id'], plan['digest'])
            relink.assert_not_called()

    def test_dependency_appearing_during_release_stops_next_unlink(self):
        plan = self.release()
        def appear(stage, entry):
            if stage == 'before_unlink':
                self.deps.side_effect = None; self.deps.return_value = self.dependency(plan, queued=True)
        with self.assertRaisesRegex(MediaError, 'changed'): self.apply(plan, hook=appear)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_archive_corruption_during_cleanup_keeps_affected_source(self):
        plan = self.release()
        def corrupt(stage, entry):
            if stage == 'before_unlink': (self.archive / entry['archive_path']).write_bytes(b'corrupt')
        with self.assertRaisesRegex(MediaError, 'Archive changed'): self.apply(plan, hook=corrupt)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_symlink_never_followed(self):
        (self.work / self.folder / 'link.JPG').symlink_to(self.source / 'DCIM/photo.JPG')
        with self.assertRaisesRegex(MediaError, 'Links'): self.release()

    def test_crash_after_empty_directory_pruning_can_resume(self):
        plan = self.release()
        def crash(stage, entry):
            if stage == 'before_complete': raise RuntimeError('crash after pruning')
        with self.assertRaisesRegex(RuntimeError, 'pruning'): self.apply(plan, hook=crash)
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_metadata_snapshot_failure_before_cleanup_keeps_sources(self):
        plan = self.release()
        with mock.patch('media.release.backup_metadata', side_effect=OSError('metadata disk failure')):
            with self.assertRaises(OSError): self.apply(plan)
        self.assertTrue(all((self.work / e['path']).exists() for e in plan['entries']))

    def test_saved_plan_dependencies_can_refresh_after_clipper_configuration(self):
        from media.release import review_release
        self.deps.side_effect = MediaError('clipper_unavailable', 'missing')
        plan = self.release(); self.assertTrue(plan['consumers']['blockers'])
        self.deps.side_effect = lambda _: empty_deps()
        refreshed = review_release(self.catalog, self.backend, plan['id'], plan['digest'])
        self.assertFalse(refreshed['consumers']['blockers'])
        self.assertEqual(refreshed['digest'], plan['digest'])
        self.assertEqual(self.apply(plan)['state'], 'completed')


class CaptureWorkflowTests(WorkflowFixture):
    def session(self, root=None, project=None, status='completed'):
        root = root or self.source
        folder = root / (str(uuid.uuid4()) + '.stalker'); folder.mkdir()
        data = {'producer': 'stalker', 'schemaVersion': 3, 'status': status, 'sessionID': str(uuid.uuid4()).upper(),
                'createdAt': '2026-10-26T13:00:00Z', 'tracks': [{'relativePath': 'screen.mov'}]}
        if project is not None: data['project'] = project
        (folder / 'manifest.json').write_text(json.dumps(data)); (folder / 'screen.mov').write_bytes(b'screen')
        return folder, data

    def test_registry_current_previous_override_and_missing_paths(self):
        current = self.root / 'new'; current.mkdir()
        old = self.root / 'old'; old.mkdir()
        one, _ = self.session(current, {'version': 1, 'name': 'one', 'country': 'mx'})
        two, _ = self.session(old); self.session(old, status='recording')
        override, _ = self.session()
        marker = self.root / 'library.json'
        marker.write_text(json.dumps({'schema': 'stalker-library/1', 'path': str(current), 'previous': [str(old), str(current), str(self.root / 'missing')]}))
        result = capture_libraries(self.catalog, str(self.source), marker)
        self.assertEqual(len(result['pending']), 3)
        self.assertEqual(result['roots'], [str(self.source), str(current), str(old)])
        self.assertTrue(result['errors'])
        self.assertEqual(next(s for s in result['pending'] if s['path'] == str(one))['project_hint']['name'], 'one')
        self.assertNotIn('name', next(s for s in result['pending'] if s['path'] == str(two))['project_hint'])

    def test_suggestions_are_explicit_tags_survive_rename_and_manifest_untouched(self):
        folder, manifest = self.session(project={'version': 1, 'name': 'proposed', 'country': 'us'})
        original = (folder / 'manifest.json').read_bytes()
        save_capture_tag(self.catalog, manifest['sessionID'], 'assigned', 'mx')
        changed = folder.with_name('renamed.stalker'); folder.rename(changed)
        from media.captures import captures
        value = captures(self.catalog, self.source)['pending'][0]
        self.assertEqual(value['tag']['name'], 'assigned')
        self.assertEqual(value['project_hint']['name'], 'proposed')
        self.assertEqual((changed / 'manifest.json').read_bytes(), original)
        discovered = Service(self.catalog, self.backend).call('discover', {'source': str(changed), 'source_kind': 'stalker'})
        self.assertEqual(discovered['units'][0]['tag']['country'], 'mx')

    def test_unknown_project_version_does_not_become_suggestion(self):
        self.session(project={'version': 9, 'name': 'future'})
        from media.captures import captures
        self.assertEqual(captures(self.catalog, self.source)['pending'][0]['project_hint'], {})

    def test_local_release_requires_exact_archived_revision_and_keeps_active(self):
        folder, manifest = self.session()
        deps = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        with deps:
            with self.assertRaisesRegex(MediaError, 'Back up'): plan_release(self.catalog, self.backend, 'MEDIA', source=str(folder), project='test')
            self.run_plan(self.plan(source=str(folder), source_kind='stalker', cleanup=False))
            plan = plan_release(self.catalog, self.backend, 'MEDIA', source=str(folder), project='test')
            apply_release(self.catalog, self.backend, plan['id'], plan['digest'])
            self.assertFalse(folder.exists())
            active, _ = self.session(status='recording')
            with self.assertRaisesRegex(MediaError, 'completed'): plan_release(self.catalog, self.backend, 'MEDIA', source=str(active), project='test')
            self.assertTrue((active / 'screen.mov').exists())

    def test_full_metadata_backup_includes_profiles_tags_decisions_and_ack(self):
        self.catalog.set('creative-profiles', {'e64': {'raw_dir': 'raw'}})
        self.catalog.set('capture-tag:example', {'name': 'human'})
        self.catalog.set('clipper-ack:example', {'status': 'applied'})
        result = backup_metadata(self.catalog, self.backend, 'MEDIA')
        snapshot = sqlite3.connect(str(self.archive / result['path']))
        try:
            self.assertEqual(snapshot.execute('PRAGMA integrity_check').fetchone()[0], 'ok')
            values = dict(snapshot.execute('SELECT key,value FROM metadata'))
            self.assertEqual(json.loads(values['capture-tag:example'])['name'], 'human')
            self.assertIn('creative-profiles', values); self.assertIn('clipper-ack:example', values)
        finally: snapshot.close()

    def test_per_session_country_is_frozen_in_layout(self):
        folder, manifest = self.session()
        plan = self.plan(source=str(folder), source_kind='stalker', cleanup=False,
                         assignments={name: {'project': 'other', 'country': 'us'} for name in ('manifest.json', 'screen.mov')})
        self.assertTrue(all('_US-OTHER[]' in item['archive'] for item in plan['layout'].values()))

    def test_local_release_prefers_verified_work_for_clipper(self):
        folder, _ = self.session()
        self.run_plan(self.plan(source=str(folder), source_kind='stalker', cleanup=False))
        with mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps()):
            plan = plan_release(self.catalog, self.backend, 'MEDIA', source=str(folder), project='test')
        self.assertTrue(plan['replacement_checks'])
        self.assertTrue(plan['replacements'][str(folder)].startswith(str(self.work) + '/'))

    def test_coverage_rehashes_receipts_instead_of_trusting_old_presence(self):
        plan = self.plan(cleanup=False); self.run_plan(plan)
        next(p for p in self.archive.rglob('*.JPG')).write_bytes(b'not-the-photo')
        result = coverage(self.catalog, self.backend, 'ESSD', 'MEDIA')
        self.assertEqual(result['totals']['only_work']['files'], 1)


class ClipperBridgeTests(WorkflowFixture):
    def test_malformed_dependency_response_blocks_release(self):
        from media.clipper import dependencies
        for response in ({}, {'schema': 'clipper-library-deps/2', 'projects': [], 'offline_projects': []},
                         dict(empty_deps(), projects=[{'clipper_root': '/project', 'assets': [{}], 'relinks': []}])):
            with mock.patch('media.clipper.invoke', return_value=response):
                with self.assertRaises(MediaError): dependencies(self.catalog)

    def test_relink_ack_must_match_operation_revision_and_be_unpinned(self):
        request = {'project': '/project', 'asset_id': 'id', 'revision': 'rev', 'from': '/local/file', 'to': '/work/file'}
        op = str(uuid.uuid4())
        ack = {'schema': 'clipper-relink-ack/1', 'status': 'applied', 'operation_id': op, 'asset_id': 'id',
               'revision': 'rev', 'from': '/local/file', 'to': '/work/file', 'still_pinned': []}
        for changes in ({'operation_id': 'different'}, {'revision': 'other'}, {'still_pinned': ['queued']}, {'to': '/wrong'}):
            with mock.patch('media.clipper.invoke', return_value=dict(ack, **changes)), self.assertRaises(MediaError):
                clipper.relink(self.catalog, request, op)
        with mock.patch('media.clipper.invoke', return_value=ack):
            self.assertEqual(clipper.relink(self.catalog, request, op), ack)
        self.assertEqual(self.catalog.get('clipper-ack:' + op), ack)

    def test_old_queued_snapshot_after_relink_remains_blocked(self):
        deps = dict(empty_deps(), projects=[{'clipper_root': '/project', 'assets': [], 'relinks': [
            {'from': '/local/session.stalker', 'still_pinned': ['queue']}]}])
        with mock.patch('media.clipper.dependencies', return_value=deps):
            result = clipper.inspect(self.catalog, {'/local/session.stalker': '/work/session.stalker'})
        self.assertTrue(result['blockers'])

    def test_real_subprocess_bridge_uses_argument_vector_with_spaces(self):
        import sys
        executable = self.root / 'fake clipper'
        executable.write_text('#!' + sys.executable + '\nimport json, sys\nassert sys.argv[1:] == ["library", "deps", "--json"]\nprint(json.dumps(' + repr(empty_deps()) + '))\n')
        executable.chmod(0o700)
        self.catalog.set('clipper-executable', str(executable))
        self.assertEqual(clipper.dependencies(self.catalog), empty_deps())

    def test_unlinked_legacy_project_blocks_local_path_even_when_deps_is_empty(self):
        folder = self.root / 'Downloads/clipper-projects/old'; folder.mkdir(parents=True)
        marker = folder / 'clipper-project.json'
        marker.write_text(json.dumps({'items': [{'path': '/local/capture.stalker/screen.mov'}]}))
        with mock.patch('media.clipper.dependencies', return_value=empty_deps()):
            result = clipper.inspect(self.catalog, {'/local/capture.stalker': '/work/capture.stalker'})
        self.assertTrue(result['blockers'])
        self.assertIn('antiguo', result['blockers'][0])
        self.assertEqual(json.loads(marker.read_text())['items'][0]['path'], '/local/capture.stalker/screen.mov')

    def test_legacy_export_snapshot_blocks_after_live_relink(self):
        folder = self.root / 'Downloads/clipper-projects/.export-queue'; folder.mkdir(parents=True)
        (folder / 'queue.json').write_text(json.dumps({'schema_version': 1, 'jobs': [
            {'id': 'q', 'status': 'failed', 'sources': {'/local/capture.stalker/screen.mov': [1]}}]}))
        with mock.patch('media.clipper.dependencies', return_value=empty_deps()):
            result = clipper.inspect(self.catalog, {'/local/capture.stalker': '/work/capture.stalker'})
        self.assertTrue(result['blockers'])

    def test_completed_queue_clears_old_ack_pin_but_failed_snapshot_stays_pinned(self):
        folder = self.root / 'Downloads/clipper-projects/.export-queue'; folder.mkdir(parents=True)
        marker = folder / 'queue.json'
        deps = dict(empty_deps(), projects=[{'clipper_root': '/project', 'assets': [], 'relinks': [{'from': '/local/file', 'still_pinned': ['q']}]}])
        for status in ('completed', 'failed'):
            marker.write_text(json.dumps({'schema_version': 1, 'jobs': [{'id': 'q', 'status': status, 'sources': {'/local/file': []}}]}))
            with mock.patch('media.clipper.dependencies', return_value=deps):
                result = clipper.inspect(self.catalog, {'/local/file': '/work/file'})
            self.assertEqual(bool(result['blockers']), status == 'failed')

    def test_release_cli_and_rpc_share_recorded_plan_boundary(self):
        from media.cli import parser
        parsed = parser().parse_args(['plan-release', '--archive', 'MEDIA', '--work', 'ESSD', '--path', '001_proyectos/raw/intro', '--project', 'intro'])
        self.assertEqual(parsed.work, 'ESSD')
        result = Service(self.catalog, self.backend).call('release_plans')
        self.assertEqual(result, [])
        with self.assertRaisesRegex(MediaError, 'recorded'):
            Service(self.catalog, self.backend).call('apply_release', {'plan_id': str(uuid.uuid4()), 'digest': 'forged'})
