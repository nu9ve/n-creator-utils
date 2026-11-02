import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import uuid

from media import clipper
from media.captures import capture_libraries, discard_check, record_discard, save_capture_tags
from media.catalog import Catalog
from media.common import MediaError, canonical
from media.finals import created_year, finals_status, plan_params, set_published_folder, year_in_name
from media.planner import build_plan
from media.projects import overview, rename, set_state, touch_project
from media.service import Service
from media.transfer import Transfer
from tests.test_media_engine import FixtureVolumes


class ProjectFixture(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)

    def asset(self, title, work='ready', job_state='completed', kind='file', updated='2026-10-01T00:00:00Z'):
        project_id = self.catalog.project_for('e64', title)
        job = str(uuid.uuid4()); asset = str(uuid.uuid4())
        payload = {'id': asset, 'job_id': job, 'project': title, 'project_id': project_id, 'profile': 'e64', 'kind': kind,
                   'paths': ['a.mov'], 'camera': 'sony', 'country': 'mx', 'city': 'hermosillo'}
        with self.catalog.db as db:
            db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', (job, 'd', '{"files":[]}', job_state, None, updated, updated))
            db.execute('INSERT INTO assets VALUES(?,?,?)', (asset, job, canonical(payload).decode()))
            db.execute('INSERT INTO work VALUES(?,?,?,?)', (asset, work, None, updated))
        return project_id

    def session(self, name=None, archived=False, created='2026-11-01T00:00:00Z'):
        return {'path': str(self.root / (str(uuid.uuid4()) + '.stalker')), 'status': 'completed', 'archived': archived,
                'created_at': created, 'tag': {'name': name} if name else {}, 'preview_path': 'screen.mov'}

    def by_title(self, sessions=()):
        return {item['project']: item for item in overview(self.catalog, list(sessions))}


class ProjectOverviewTests(ProjectFixture):
    def test_stage_comes_from_receipts_pending_sessions_and_work(self):
        self.asset('listo')
        self.asset('copias', work='pending')
        self.asset('atorado', job_state='failed')
        self.asset('con-sesion')
        items = self.by_title([self.session('borrador'), self.session('con-sesion'), self.session()])
        self.assertEqual(items['listo']['stage'], 'backed_up')
        self.assertEqual(items['copias']['stage'], 'in_progress')
        self.assertEqual(items['atorado']['stage'], 'in_progress')
        self.assertEqual(items['con-sesion']['pending_sessions'], 1)
        self.assertEqual(items['con-sesion']['stage'], 'in_progress')
        self.assertEqual(items['borrador']['stage'], 'draft')
        self.assertEqual(items['borrador']['key'], 'draft:borrador')
        self.assertEqual(items['borrador']['preview']['preview_path'], 'screen.mov')
        self.assertEqual(len(items), 5, 'an untagged session creates no project')

    def test_camera_format_reminder_does_not_keep_a_backup_in_progress(self):
        self.asset('sony', job_state='camera_action_required')
        self.assertEqual(self.by_title()['sony']['stage'], 'backed_up')

    def test_state_is_declared_and_finished_wins_without_touching_anything(self):
        project = self.asset('cerrado', work='pending')
        set_state(self.catalog, project, 'finished')
        item = self.by_title()['cerrado']
        self.assertEqual((item['state'], item['stage'], item['work_pending']), ('finished', 'finished', 1))
        set_state(self.catalog, project, 'active')
        self.assertEqual(self.by_title()['cerrado']['stage'], 'in_progress')
        with self.assertRaises(MediaError):
            set_state(self.catalog, project, 'borrado')
        with self.assertRaises(MediaError):
            set_state(self.catalog, 'nope', 'finished')

    def test_bump_moves_a_project_to_the_top_and_tagging_counts_as_use(self):
        old = self.asset('viejo', updated='2026-01-01T00:00:00Z')
        self.asset('nuevo', updated='2026-10-01T00:00:00Z')
        self.assertEqual([i['project'] for i in overview(self.catalog, [])], ['nuevo', 'viejo'])
        touch_project(self.catalog, old)
        self.assertEqual([i['project'] for i in overview(self.catalog, [])], ['viejo', 'nuevo'])
        self.asset('otro', updated='2026-10-02T00:00:00Z')
        session = str(uuid.uuid4())
        save_capture_tags(self.catalog, [session], 'nuevo')
        self.assertEqual(overview(self.catalog, [])[0]['project'], 'nuevo')

    def test_bulk_tags_share_one_project_and_validate_every_id(self):
        ids = [str(uuid.uuid4()), str(uuid.uuid4())]
        value = save_capture_tags(self.catalog, ids, 'Mi Proyecto', 'MX')
        self.assertEqual(value, {'name': 'Mi-Proyecto', 'country': 'MX'})
        self.assertEqual(self.catalog.get('capture-tag:' + ids[1])['name'], 'Mi-Proyecto')
        with self.assertRaises(ValueError):
            save_capture_tags(self.catalog, [ids[0], 'bad'], 'otro')
        self.assertEqual(self.catalog.get('capture-tag:' + ids[0])['name'], 'Mi-Proyecto')

    def test_rename_links_catalog_tags_and_defaults_keeping_the_old_alias(self):
        project = self.asset('terra')
        session = str(uuid.uuid4())
        save_capture_tags(self.catalog, [session], 'terra')
        self.catalog.set('defaults', {'project': 'terra', 'profile': 'e64'})
        result = rename(self.catalog, project, 'terra-nueva')
        self.assertEqual(result['previous'], 'terra')
        self.assertEqual(self.catalog.get('capture-tag:' + session)['name'], 'terra-nueva')
        self.assertEqual(self.catalog.get('defaults')['project'], 'terra-nueva')
        self.assertEqual(self.catalog.search()[0]['project'], 'terra-nueva')
        self.assertEqual(self.catalog.project_for('e64', 'terra'), project)
        self.assertEqual(self.by_title()['terra-nueva']['aliases'], ['terra'])

    def test_renaming_a_draft_onto_a_known_project_links_its_sessions(self):
        project = self.asset('existente')
        session = str(uuid.uuid4())
        save_capture_tags(self.catalog, [session], 'borrador')
        result = rename(self.catalog, 'draft:borrador', 'existente')
        self.assertEqual(result['key'], project)
        self.assertEqual(self.catalog.get('capture-tag:' + session)['name'], 'existente')

    def test_rename_conflict_is_refused(self):
        project = self.asset('uno'); self.asset('dos')
        with self.assertRaises(MediaError):
            rename(self.catalog, project, 'dos')

    def test_dismissing_a_job_keeps_state_and_can_be_undone(self):
        self.asset('sony', job_state='camera_action_required')
        service = Service(self.catalog, FixtureVolumes())
        job = self.catalog.jobs()[0]
        self.assertFalse(job['dismissed'])
        service.call('dismiss_job', {'job_id': job['id'], 'dismissed': True})
        after = self.catalog.jobs()[0]
        self.assertTrue(after['dismissed'])
        self.assertEqual(after['state'], 'camera_action_required')
        service.call('dismiss_job', {'job_id': job['id'], 'dismissed': False})
        self.assertFalse(self.catalog.jobs()[0]['dismissed'])
        with self.assertRaises(MediaError):
            service.call('dismiss_job', {'job_id': 'nope', 'dismissed': True})


class UsualFolderTests(ProjectFixture):
    def bundle(self, folder, status='completed'):
        path = folder / (str(uuid.uuid4()) + '.stalker')
        path.mkdir(parents=True)
        (path / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'sessionID': str(uuid.uuid4()),
                                                        'status': status, 'createdAt': '2026-10-01T00:00:00Z', 'tracks': []}))
        return path

    def test_usual_folders_count_only_with_sessions(self):
        registry = self.root / 'none.json'
        movies, downloads, empty = self.root / 'Movies/Stalker', self.root / 'Downloads', self.root / 'Vacia'
        self.bundle(downloads); empty.mkdir(); movies.mkdir(parents=True); (downloads / 'otra-cosa.mov').write_text('x')
        result = capture_libraries(self.catalog, '', registry, common=[movies, downloads, empty, self.root / 'Falta'])
        self.assertEqual(result['roots'], [str(downloads)])
        self.assertEqual(len(result['pending']), 1)
        self.assertEqual(result['errors'], [])


class DiscardCaptureTests(ProjectFixture):
    def setUp(self):
        super().setUp()
        self.library = self.root / 'Stalker'
        self.library.mkdir()
        self.sid = str(uuid.uuid4())
        self.folder = self.bundle(self.library / 'prueba.stalker', self.sid)
        patcher = mock.patch('media.clipper.inspect', return_value={'blockers': [], 'relinks': [], 'dependencies': {}})
        self.inspect = patcher.start()
        self.addCleanup(patcher.stop)

    def bundle(self, folder, sid, status='completed'):
        folder.mkdir()
        (folder / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'sessionID': sid, 'status': status,
                                                          'createdAt': '2026-11-01T00:00:00Z', 'tracks': [{'relativePath': 'screen.mov'}]}))
        (folder / 'screen.mov').write_bytes(b'x' * 100)
        return folder

    def check(self, path=None):
        return discard_check(self.catalog, str(path or self.folder), str(self.library), self.root / 'sin-registro.json', [])

    def test_reports_what_would_go_and_touches_nothing(self):
        result = self.check()
        self.assertEqual((result['session_id'], result['tracks'], result['blockers']), (self.sid, 1, []))
        self.assertGreater(result['bytes'], 100)
        self.assertTrue((self.folder / 'screen.mov').exists())
        self.inspect.assert_called_once()

    def test_only_a_finished_session_of_a_known_library(self):
        active = self.bundle(self.library / 'grabando.stalker', str(uuid.uuid4()), status='recording')
        outside = self.root / 'fuera'; outside.mkdir()
        foreign = self.bundle(outside / 'ajena.stalker', str(uuid.uuid4()))
        link = self.library / 'enlace.stalker'; link.symlink_to(foreign)
        for path, code in ((active, 'capture_not_completed'), (foreign, 'capture_not_found'), (link, 'capture_not_found'), (self.library, 'capture_not_found')):
            with self.assertRaises(MediaError) as caught:
                self.check(path)
            self.assertEqual(caught.exception.code, code)

    def test_a_backed_up_session_goes_through_release(self):
        with mock.patch('media.captures.capture_libraries', return_value={'sessions': [{'path': str(self.folder), 'status': 'completed', 'archived': True}]}):
            with self.assertRaises(MediaError) as caught:
                self.check()
        self.assertEqual(caught.exception.code, 'capture_archived')

    def test_an_unfinished_operation_blocks_it(self):
        plan = {'source': str(self.library), 'files': [{'path': 'prueba.stalker/screen.mov'}]}
        with self.catalog.db as db:
            db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)', ('j', 'd', json.dumps(plan), 'paused', None, 'x', 'x'))
        with self.assertRaises(MediaError) as caught:
            self.check()
        self.assertEqual(caught.exception.code, 'capture_in_use')
        with self.catalog.db as db:
            db.execute("UPDATE jobs SET state='completed'")
        self.assertEqual(self.check()['blockers'], [])

    def test_clipper_use_blocks_and_unknown_only_warns(self):
        self.inspect.return_value = {'blockers': ['Fuente en uso sin reemplazo editable: x'], 'relinks': [], 'dependencies': {}}
        self.assertEqual(self.check()['blockers'], ['Fuente en uso sin reemplazo editable: x'])
        self.inspect.return_value = {'blockers': [clipper.OFFLINE_PROJECTS], 'relinks': [], 'dependencies': {}}
        result = self.check()
        self.assertEqual((result['blockers'], result['warnings']), ([], ['offline_projects']))
        self.inspect.side_effect = MediaError('clipper_unavailable', 'x')
        result = self.check()
        self.assertEqual((result['blockers'], result['warnings']), ([], ['clipper_unknown']))

    def test_the_decision_is_recorded_and_the_tag_survives(self):
        save_capture_tags(self.catalog, [self.sid], 'prueba')
        record_discard(self.catalog, self.sid, 'prueba.stalker', '/Trash/prueba.stalker', 10)
        self.assertEqual(self.catalog.get('capture-discarded:' + self.sid)['trash_path'], '/Trash/prueba.stalker')
        self.assertEqual(self.catalog.get('capture-tag:' + self.sid)['name'], 'prueba')


class FinalsTests(ProjectFixture):
    def setUp(self):
        super().setUp()
        self.backend = FixtureVolumes()
        self.work, self.archive = self.root / 'work', self.root / 'archive'
        self.work.mkdir(); self.archive.mkdir()
        self.catalog.register('ESSD', 'work', self.work, self.backend.describe(self.work), '001_proyectos')
        self.catalog.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        patcher = mock.patch('media.clipper.inspect', return_value={'blockers': [], 'relinks': [], 'dependencies': {}})
        self.inspect = patcher.start()
        self.addCleanup(patcher.stop)

    def file(self, root, relative, size=10):
        path = root / '002_finales' / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'x' * size)
        return path

    def test_compares_name_and_size_anywhere_in_the_profile_without_writing(self):
        self.file(self.work, 'e64/03_subidos/hecho.mov')
        self.file(self.archive, 'e64/2026/hecho.mov')
        self.file(self.work, 'e64/02_shorts/nuevo.mov', 20)
        self.file(self.work, 'e64/02_shorts/cambio.mov', 30)
        self.file(self.archive, 'e64/2026/cambio.mov', 31)
        self.file(self.archive, '9logs/hecho.mov')
        self.file(self.work, '9logs/otro-perfil.mov')
        self.file(self.work, 'e64/.DS_Store'); self.file(self.work, 'e64/notas.txt')
        bundle = self.file(self.work, 'e64/edicion.fcpbundle/render.mov')
        before = sorted(str(p) for p in self.archive.rglob('*'))
        result = finals_status(self.catalog, self.backend)
        groups = {g['profile']: g for g in result['groups']}
        self.assertEqual(result['archive'], 'MEDIA')
        self.assertEqual(groups['e64']['total'], 3)
        self.assertEqual(groups['e64']['published_folder'], '03_subidos')
        self.assertEqual((groups['e64']['pending'], groups['e64']['waiting'], groups['e64']['waiting_bytes']), (0, 2, 50))
        self.assertEqual((groups['e64']['in_archive'], groups['e64']['in_archive_bytes']), (1, 10))
        self.assertEqual(groups['e64']['destination'], 'MEDIA/002_finales/e64')
        self.assertEqual((groups['9logs']['published_folder'], groups['9logs']['pending']), ('', 1))
        self.assertEqual(before, sorted(str(p) for p in self.archive.rglob('*')))
        self.assertTrue(bundle.exists())

    def test_only_the_published_folder_is_due_for_backup(self):
        self.file(self.work, 'e64/03_subidos/shorts/subido.mov', 20)
        self.file(self.work, 'e64/03_subidos/maestro.mov', 30)
        self.file(self.work, 'e64/02_shorts/en-uso.mov', 40)
        group = finals_status(self.catalog, self.backend)['groups'][0]
        self.assertEqual((group['published_total'], group['pending'], group['pending_bytes']), (2, 2, 50))
        self.assertEqual(sorted(f['name'] for f in group['files']), ['03_subidos/maestro.mov', '03_subidos/shorts/subido.mov'])
        self.assertEqual(group['waiting'], 1)
        self.assertEqual(group['subfolders'], ['02_shorts', '03_subidos'])

    def test_an_empty_published_folder_still_means_nothing_is_due(self):
        (self.work / '002_finales/9logs/subidos').mkdir(parents=True)
        self.file(self.work, '9logs/sin-publicar.mov')
        group = finals_status(self.catalog, self.backend)['groups'][0]
        self.assertEqual((group['published_folder'], group['pending'], group['waiting']), ('subidos', 0, 1))

    def test_the_published_folder_can_be_chosen_cleared_and_detected_again(self):
        self.file(self.work, 'e64/03_subidos/a.mov')
        self.file(self.work, 'e64/listos/b.mov')
        def group(): return finals_status(self.catalog, self.backend)['groups'][0]
        set_published_folder(self.catalog, 'e64', 'listos')
        self.assertEqual((group()['published_folder'], group()['files'][0]['name']), ('listos', 'listos/b.mov'))
        set_published_folder(self.catalog, 'e64', '')
        self.assertEqual((group()['published_folder'], group()['pending']), ('', 2))
        set_published_folder(self.catalog, 'e64', 'ya-no-existe')
        self.assertEqual(group()['published_folder'], '')
        set_published_folder(self.catalog, 'e64', None)
        self.assertEqual(group()['published_folder'], '03_subidos')
        for bad in ('../fuera', 'a/b', '.oculta'):
            with self.assertRaises(MediaError):
                set_published_folder(self.catalog, 'e64', bad)

    def route(self, release=True):
        plan = build_plan(self.catalog, self.backend, **plan_params(self.catalog, self.backend, 'e64', release=release))
        return plan, {row['source']: row['archive_path'] for row in plan['preview']}

    def test_year_comes_from_the_name_first(self):
        self.assertEqual(year_in_name('26-10-27-terra.mov'), '2026')
        self.assertEqual(year_in_name('2024-03-09-titulo.mov'), '2024')
        self.assertEqual(year_in_name('240905_davidcordova.mov'), '2024')
        for name in ('008vibecoding.mp4', '26-13-27-mes-malo.mov', 'semana7-2026-e64.mov', '26-10-271.mov', '000.mov'):
            self.assertIsNone(year_in_name(name), name)

    def test_route_keeps_subfolders_and_dates_loose_masters(self):
        self.file(self.work, 'e64/03_subidos/26-10-27-terra.mov')
        loose = self.file(self.work, 'e64/03_subidos/reexport.mov')
        self.file(self.work, 'e64/03_subidos/shorts/26-09-26-corto_vertical_1.mov')
        self.file(self.work, 'e64/03_subidos/thumbnails/titulos.txt')
        self.file(self.work, 'e64/02_shorts/sin-publicar.mov')
        plan, routes = self.route()
        base = str(self.archive / '002_finales/e64')
        self.assertEqual(routes['26-10-27-terra.mov'], base + '/2026/26-10-27-terra.mov')
        self.assertEqual(routes['reexport.mov'], base + '/' + created_year(loose) + '/reexport.mov')
        self.assertEqual(routes['shorts/26-09-26-corto_vertical_1.mov'], base + '/shorts/26-09-26-corto_vertical_1.mov')
        self.assertEqual(routes['thumbnails/titulos.txt'], base + '/thumbnails/titulos.txt')
        self.assertEqual(len(routes), 4)
        self.assertTrue(plan['cleanup'])
        self.assertIsNone(plan['work_id'])

    def test_keeping_the_source_is_a_choice(self):
        source = self.file(self.work, 'e64/03_subidos/26-10-27-terra.mov', 30)
        short = self.file(self.work, 'e64/03_subidos/shorts/corto.mov', 20)
        waiting = self.file(self.work, 'e64/02_shorts/en-uso.mov', 40)
        defaults = self.catalog.get('defaults')
        plan, _ = self.route(release=False)
        result = Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        self.assertEqual(result['state'], 'source_retained')
        self.assertEqual((self.archive / '002_finales/e64/2026/26-10-27-terra.mov').stat().st_size, 30)
        self.assertEqual((self.archive / '002_finales/e64/shorts/corto.mov').stat().st_size, 20)
        self.assertTrue(source.exists() and short.exists() and waiting.exists())
        self.assertFalse(list(self.archive.rglob('*.ncpartial')))
        group = finals_status(self.catalog, self.backend)['groups'][0]
        self.assertEqual((group['pending'], group['waiting'], group['in_archive']), (0, 1, 2))
        self.assertEqual(self.catalog.get('defaults'), defaults)
        # Nothing left to copy for what is still sitting in the published folder: same place, adopted after hashing.
        again, _ = self.route(release=False)
        repeat = Transfer(self.catalog, self.backend).apply(again, again['digest'])
        self.assertEqual(repeat['state'], 'source_retained')
        self.assertEqual(len(list((self.archive / '002_finales').rglob('*.mov'))), 2)

    def test_moving_frees_only_what_was_verified_and_keeps_the_folders(self):
        master = self.file(self.work, 'e64/03_subidos/26-10-27-terra.mov', 30)
        short = self.file(self.work, 'e64/03_subidos/shorts/corto.mov', 20)
        note = self.file(self.work, 'e64/03_subidos/thumbnails/titulos.txt', 5)
        waiting = self.file(self.work, 'e64/02_shorts/en-uso.mov', 40)
        plan, _ = self.route()
        result = Service(self.catalog, self.backend).call('apply', {'plan': plan, 'digest': plan['digest']})
        self.assertEqual(result['state'], 'completed')
        self.assertFalse(master.exists() or short.exists() or note.exists())
        self.assertTrue(waiting.exists() and short.parent.is_dir())
        final = self.archive / '002_finales/e64'
        self.assertEqual((final / '2026/26-10-27-terra.mov').stat().st_size, 30)
        self.assertEqual((final / 'shorts/corto.mov').stat().st_size, 20)
        self.assertEqual((final / 'thumbnails/titulos.txt').stat().st_size, 5)
        group = finals_status(self.catalog, self.backend)['groups'][0]
        self.assertEqual((group['pending'], group['waiting'], group['published_total']), (0, 1, 0))

    def test_what_was_not_copied_is_never_freed(self):
        self.file(self.archive, 'e64/2023/viejo.mov', 15)
        kept = self.file(self.work, 'e64/03_subidos/viejo.mov', 15)
        moved = self.file(self.work, 'e64/03_subidos/nuevo.mov', 25)
        plan, _ = self.route()
        result = Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        self.assertEqual(result['state'], 'completed')
        self.assertTrue(kept.exists())
        self.assertFalse(moved.exists())

    def test_a_final_that_arrives_during_the_copy_stays_and_is_reported(self):
        self.file(self.work, 'e64/03_subidos/a.mov')
        plan, _ = self.route()
        late = self.file(self.work, 'e64/03_subidos/llego-tarde.mov', 11)
        result = Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        self.assertEqual(result['state'], 'source_has_new_material')
        self.assertTrue(late.exists())
        self.assertFalse((self.work / '002_finales/e64/03_subidos/a.mov').exists())

    def test_a_changed_final_stops_before_anything_is_freed(self):
        changed = self.file(self.work, 'e64/03_subidos/a.mov', 10)
        other = self.file(self.work, 'e64/03_subidos/b.mov', 10)
        plan, _ = self.route()
        changed.write_bytes(b'y' * 12)
        with self.assertRaises(MediaError) as caught:
            Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        self.assertEqual(caught.exception.code, 'source_changed')
        self.assertTrue(changed.exists() and other.exists())

    def test_one_physical_disk_or_clipper_use_refuses_to_free(self):
        self.file(self.work, 'e64/03_subidos/a.mov')
        self.inspect.return_value = {'blockers': ['Fuente en uso sin reemplazo editable: a.mov'], 'relinks': [], 'dependencies': {}}
        with self.assertRaises(MediaError) as caught:
            self.route()
        self.assertEqual(caught.exception.code, 'source_in_use')
        self.assertFalse(self.route(release=False)[0]['cleanup'])
        self.inspect.return_value = {'blockers': [], 'relinks': [], 'dependencies': {}}
        plan, _ = self.route()
        self.inspect.return_value = {'blockers': ['Exportación en cola: a.mov'], 'relinks': [], 'dependencies': {}}
        with self.assertRaises(MediaError):
            Service(self.catalog, self.backend).call('apply', {'plan': plan, 'digest': plan['digest']})
        self.assertTrue((self.work / '002_finales/e64/03_subidos/a.mov').exists())
        self.inspect.return_value = {'blockers': [], 'relinks': [], 'dependencies': {}}
        self.backend.physical[str(self.work)] = self.backend.physical[str(self.archive)] = 'mismo'
        with self.assertRaises(MediaError) as caught:
            self.route()
        self.assertEqual(caught.exception.code, 'same_physical_disk')

    def test_a_copy_already_elsewhere_in_the_profile_is_not_duplicated(self):
        self.file(self.archive, 'e64/2023/viejo.mov', 15)
        self.file(self.work, 'e64/03_subidos/viejo.mov', 15)
        self.file(self.work, 'e64/03_subidos/nuevo.mov', 25)
        plan, routes = self.route()
        self.assertEqual(list(routes), ['nuevo.mov'])
        self.assertEqual(plan['already_archived'], ['viejo.mov'])

    def test_a_different_file_at_the_destination_blocks_and_nothing_is_written(self):
        self.file(self.work, 'e64/03_subidos/26-10-27-terra.mov', 30)
        self.file(self.archive, 'e64/2026/26-10-27-terra.mov', 31)
        with self.assertRaises(MediaError) as caught:
            self.route()
        self.assertEqual(caught.exception.code, 'destination_conflict')
        self.assertEqual((self.archive / '002_finales/e64/2026/26-10-27-terra.mov').stat().st_size, 31)

    def test_same_bytes_at_the_destination_are_adopted_and_different_bytes_stop(self):
        self.file(self.work, 'e64/03_subidos/26-10-27-igual.mov', 12)
        self.file(self.archive, 'e64/2026/26-10-27-igual.mov', 12)
        source = self.work / '002_finales/e64/03_subidos/26-10-27-igual.mov'
        plan, routes = self.route()
        self.assertEqual(plan['already_archived'], [])
        self.assertEqual(len(routes), 1)
        self.assertEqual(Transfer(self.catalog, self.backend).apply(plan, plan['digest'])['state'], 'completed')
        self.assertFalse(source.exists())

    def test_needs_a_published_folder_and_both_disks(self):
        self.file(self.work, 'e64/02_shorts/a.mov')
        with self.assertRaises(MediaError) as caught:
            plan_params(self.catalog, self.backend, 'e64')
        self.assertEqual(caught.exception.code, 'no_published_folder')
        self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError) as caught:
            plan_params(self.catalog, self.backend, 'e64')
        self.assertEqual(caught.exception.code, 'archive_offline')

    def test_a_tampered_finals_plan_is_rejected(self):
        self.file(self.work, 'e64/03_subidos/26-10-27-terra.mov')
        plan, _ = self.route()
        plan['layout']['26-10-27-terra.mov']['archive'] = 'e64/otro/lugar.mov'
        with self.assertRaises(MediaError):
            Transfer(self.catalog, self.backend).apply(plan, plan['digest'])

    def test_offline_archive_answers_nothing_instead_of_all_pending(self):
        self.file(self.work, 'e64/nuevo.mov')
        self.backend.offline.add(str(self.archive))
        result = finals_status(self.catalog, self.backend)
        self.assertEqual((result['reason'], result['groups']), ('archive_offline', []))

    def test_cancel_stops_the_walk(self):
        self.file(self.work, 'e64/nuevo.mov')
        with self.assertRaises(MediaError):
            finals_status(self.catalog, self.backend, cancel=lambda: True)


if __name__ == '__main__':
    unittest.main()
