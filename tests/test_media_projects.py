import json
import os
from pathlib import Path
import tempfile
import unittest
import uuid

from media.captures import capture_libraries, save_capture_tags
from media.catalog import Catalog
from media.common import MediaError, canonical
from media.finals import finals_status
from media.projects import overview, rename, set_state, touch_project
from media.service import Service
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


class FinalsTests(ProjectFixture):
    def setUp(self):
        super().setUp()
        self.backend = FixtureVolumes()
        self.work, self.archive = self.root / 'work', self.root / 'archive'
        self.work.mkdir(); self.archive.mkdir()
        self.catalog.register('ESSD', 'work', self.work, self.backend.describe(self.work), '001_proyectos')
        self.catalog.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')

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
        self.assertEqual(groups['e64']['pending'], 2)
        self.assertEqual(groups['e64']['pending_bytes'], 50)
        self.assertEqual(sorted(f['name'] for f in groups['e64']['files']), ['02_shorts/cambio.mov', '02_shorts/nuevo.mov'])
        self.assertEqual(groups['e64']['destination'], 'MEDIA/002_finales/e64')
        self.assertEqual(groups['9logs']['pending'], 1)
        self.assertEqual(before, sorted(str(p) for p in self.archive.rglob('*')))
        self.assertTrue(bundle.exists())

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
