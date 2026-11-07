"""Destructive project closeout uses fixture volume identities and disposable media only."""
import copy
from pathlib import Path
from unittest import mock

from test_media_release import WorkflowFixture, empty_deps
from media import closure
from media.common import MediaError
from media.release import apply_release
from media.service import Service


class ClosureTests(WorkflowFixture):
    def setUp(self):
        super().setUp()
        patcher = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        self.deps = patcher.start(); self.addCleanup(patcher.stop)
        self.run_plan()
        self.project_id = self.catalog.db.execute("SELECT id FROM projects WHERE title='intro'").fetchone()[0]
        self.editor = self.work / '001_proyectos/e64/intro'
        (self.editor / 'renders').mkdir(parents=True)
        (self.editor / 'renders/intermediate.mov').write_bytes(b'intermediate')
        (self.editor / 'clipper').mkdir()
        (self.editor / 'clipper/clipper-project.json').write_text('{"authored":"keep"}')
        (self.editor / 'edit.fcpbundle').mkdir()
        (self.editor / 'edit.fcpbundle/library').write_bytes(b'fcp')
        (self.editor / 'notes.txt').write_text('keep')
        self.final = self.archive / '002_finales/e64/2026/intro.mov'
        self.final.parent.mkdir(parents=True)
        self.final.write_bytes(b'final')
        self.configure()

    def configure(self, final=None, volume=None, **overrides):
        params = dict(project_id=self.project_id, required=['horizontal'], no_more=True, published=True,
                      finals=[{'volume_id': (volume or self.a)['id'], 'path': str((final or self.final).relative_to((volume or self.a)['root'])), 'role': 'horizontal'}])
        params.update(overrides)
        return closure.save(self.catalog, self.backend, **params)

    def plan_close(self):
        return closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')

    def apply(self, plan, **kwargs):
        return closure.apply_close(self.catalog, self.backend, plan['id'], plan['digest'], confirm_discard=True, **kwargs)

    def test_archived_final_closes_and_discards_renders_without_archiving_editables(self):
        plan = self.plan_close()
        self.assertEqual({e['category'] for e in plan['entries']}, {'raw', 'intermediate'})
        self.assertTrue(closure.overview(self.catalog)[0]['needs_release'])
        result = self.apply(plan)
        self.assertEqual(result['state'], 'completed')
        self.assertFalse((self.editor / 'renders/intermediate.mov').exists())
        self.assertFalse((self.work / '001_proyectos/e64_raw/intro').exists())
        self.assertTrue((self.editor / 'clipper/clipper-project.json').exists())
        self.assertTrue((self.editor / 'edit.fcpbundle/library').exists())
        self.assertTrue((self.editor / 'notes.txt').exists())
        self.assertTrue(self.final.exists())
        self.assertFalse(closure.overview(self.catalog)[0]['needs_release'])
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_copy_selected_work_final_verify_and_release_in_one_apply(self):
        selected = self.work / '002_finales/e64/03_subidos/26-11-04-intro.mov'
        selected.parent.mkdir(parents=True)
        selected.write_bytes(b'new final')
        unrelated = selected.with_name('unrelated.mov'); unrelated.write_bytes(b'not selected')
        self.configure(selected, self.w)
        plan = self.plan_close()
        self.assertEqual(len(plan['closure_copies']), 1)
        self.assertTrue(selected.exists())
        self.assertEqual(self.apply(plan)['state'], 'completed')
        self.assertFalse(selected.exists())
        self.assertTrue(unrelated.exists())
        self.assertEqual((self.archive / '002_finales/e64/2026/26-11-04-intro.mov').read_bytes(), b'new final')

    def test_vertical_and_thumbnail_route_and_requirements(self):
        selected = self.work / '002_finales/e64/shorts/intro-short.mov'
        selected.parent.mkdir(parents=True); selected.write_bytes(b'short')
        thumb = self.work / '002_finales/e64/thumbs/intro.jpg'
        thumb.parent.mkdir(parents=True); thumb.write_bytes(b'thumb')
        config = closure.settings(self.catalog, self.project_id)
        self.configure(required=['horizontal', 'vertical', 'thumbnail'])
        with self.assertRaisesRegex(MediaError, 'Faltan'): self.plan_close()
        config['finals'] += [{'volume_id': self.w['id'], 'path': str(p.relative_to(self.work)), 'role': role}
                             for p, role in [(selected, 'vertical'), (thumb, 'thumbnail')]]
        self.configure(required=['horizontal', 'vertical', 'thumbnail'], finals=config['finals'])
        self.assertEqual(self.apply(self.plan_close())['state'], 'completed')
        self.assertTrue((self.archive / '002_finales/e64/shorts/intro-short.mov').exists())
        self.assertTrue((self.archive / '002_finales/e64/thumbnails/intro.jpg').exists())

    def test_suggest_exact_published_vertical_and_retain_publication_after_archiving(self):
        from media.finals import finals_status, plan_params
        from media.planner import build_plan
        from media.transfer import Transfer

        published = self.work / '002_finales/e64/03_subidos/shorts'
        published.mkdir(parents=True)
        vertical = published / '26-11-04-intro_vertical_1.mov'
        vertical.write_bytes(b'short')
        (published / '26-11-04-intro-extra_vertical_1.mov').write_bytes(b'other project')
        waiting = self.work / '002_finales/e64/02_shorts'
        waiting.mkdir()
        (waiting / '26-11-04-intro_vertical_2.mov').write_bytes(b'unpublished')
        suggestions = lambda: finals_status(self.catalog, self.backend)['closure_suggestions']
        self.assertEqual([(v['id'], v['published_verticals']) for v in suggestions()], [(self.project_id, 1)])
        files = closure.candidates(self.catalog, self.backend, self.project_id)['files']
        self.assertTrue(next(f for f in files if f['name'].endswith(vertical.name))['suggested'])
        self.assertFalse(next(f for f in files if 'intro-extra' in f['name'])['suggested'])
        plan = build_plan(self.catalog, self.backend, **plan_params(self.catalog, self.backend, 'e64', release=False))
        Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        vertical.unlink()
        archive = self.archive / '002_finales/e64/shorts' / vertical.name
        archived = next(f for f in closure.candidates(self.catalog, self.backend, self.project_id)['files']
                        if f['archived'] and f['name'].endswith(vertical.name))
        self.assertTrue(archived['published'])
        self.assertEqual(suggestions()[0]['published_verticals'], 1)
        archive.write_bytes(b'changed after receipt')
        self.assertEqual(suggestions(), [])

    def test_already_released_raws_do_not_block_remaining_intermediates(self):
        from media.release import plan_release
        raw = plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path='001_proyectos/e64_raw/intro', project='intro')
        apply_release(self.catalog, self.backend, raw['id'], raw['digest'])
        self.assertEqual(self.apply(self.plan_close())['state'], 'completed')

    def test_changed_final_or_raw_backup_blocks_before_any_delete(self):
        for kind in ('final', 'raw'):
            plan = self.plan_close()
            path = self.final if kind == 'final' else self.archive / plan['entries'][0]['archive_path']
            original = path.read_bytes(); path.write_bytes(b'x' * len(original))
            with self.assertRaises(MediaError): self.apply(plan)
            self.assertTrue((self.editor / 'renders/intermediate.mov').exists())
            self.assertTrue((self.work / plan['entries'][0]['path']).exists())
            path.write_bytes(original)
            self.configure()

    def test_source_or_membership_changes_after_review_block_deletion(self):
        plan = self.plan_close()
        (self.editor / 'renders/new.mov').write_bytes(b'new')
        with self.assertRaisesRegex(MediaError, 'New material'): self.apply(plan)
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())

    def test_unknown_dependencies_and_offline_volumes_retain_work(self):
        plan = self.plan_close()
        self.deps.side_effect = MediaError('clipper_unavailable', 'unknown')
        with self.assertRaises(MediaError): self.apply(plan)
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())
        self.deps.side_effect = lambda _: empty_deps()
        self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError): self.apply(plan)

    def test_changed_human_decisions_or_wrong_digest_cannot_apply(self):
        plan = self.plan_close()
        with self.assertRaises(MediaError): closure.apply_close(self.catalog, self.backend, plan['id'], 'wrong')
        self.configure(no_more=False)
        with self.assertRaisesRegex(MediaError, 'Cambió'): self.apply(plan)
        with self.assertRaisesRegex(MediaError, 'Confirma'): self.plan_close()

    def test_crash_after_discard_unlink_resumes_only_journaled_members(self):
        plan = self.plan_close()
        from media.common import Tree
        real = Tree.unlink
        def crash(tree, path, expected=None):
            real(tree, path, expected)
            if path.endswith('intermediate.mov'): raise RuntimeError('power loss')
        with mock.patch.object(Tree, 'unlink', crash), self.assertRaisesRegex(RuntimeError, 'power loss'):
            self.apply(plan)
        self.assertEqual(self.apply(plan)['state'], 'completed')

    def test_fcp_bundles_nested_in_renders_are_preserved_and_symlinks_block(self):
        bundle = self.editor / 'renders/nested.fcpbundle'; bundle.mkdir(); (bundle / 'library').write_text('keep')
        plan = self.plan_close()
        self.assertFalse(any('nested.fcpbundle' in e['path'] for e in plan['entries']))
        link = self.editor / 'renders/link.mov'; link.symlink_to(self.final)
        with self.assertRaises(MediaError): self.apply(plan)
        self.assertTrue(self.final.exists())

    def test_corruption_during_release_blocks_intermediate_discard(self):
        plan = self.plan_close()
        self.catalog.set('closure-consent:' + plan['id'], {'digest': plan['digest']})
        def corrupt(stage, entry):
            if stage == 'before_unlink': self.final.write_bytes(b'xxxxx')
        with self.assertRaises(MediaError):
            apply_release(self.catalog, self.backend, plan['id'], plan['digest'], hook=corrupt)
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())

    def test_service_boundary_and_read_only_candidates(self):
        service = Service(self.catalog, self.backend)
        data = service.call('closure_candidates', {'project_id': self.project_id})
        self.assertEqual(data['files'][0]['role'], 'horizontal')
        self.assertEqual(service.call('snapshot', {'cards': False})['project_closures'][0]['id'], self.project_id)
        with self.assertRaises(MediaError): self.configure(finals=[{'volume_id': self.a['id'], 'path': '../escape.mov', 'role': 'horizontal'}])

    def test_discard_requires_recorded_consent_even_through_raw_release_endpoint(self):
        plan = self.plan_close()
        with self.assertRaisesRegex(MediaError, 'Confirma'):
            closure.apply_close(self.catalog, self.backend, plan['id'], plan['digest'])
        with self.assertRaisesRegex(MediaError, 'Confirma'):
            apply_release(self.catalog, self.backend, plan['id'], plan['digest'])
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())

    def test_published_folder_is_declaration_but_not_a_backup(self):
        selected = self.work / '002_finales/e64/03_subidos/intro-publicado.mov'
        selected.parent.mkdir(parents=True); selected.write_bytes(b'published final')
        found = next(f for f in closure.candidates(self.catalog, self.backend, self.project_id)['files'] if f['volume_id'] == self.w['id'])
        self.assertTrue(found['published'])
        self.assertFalse(found['archived'])
        config = self.configure(selected, self.w, published=False)
        self.assertTrue(config['published'])
        self.assertTrue(selected.exists())
        result = self.apply(self.plan_close())
        saved = closure.settings(self.catalog, self.project_id)
        self.assertEqual(saved['finals'][0]['volume_id'], self.a['id'])
        self.assertTrue(saved['finals'][0]['published'])
        self.assertTrue((self.archive / saved['finals'][0]['path']).exists())
        self.assertEqual(result['state'], 'completed')

    def test_complete_before_publication_backs_up_and_keeps_editing_final(self):
        selected = self.work / '002_finales/e64/intro-publicado.mov'
        selected.parent.mkdir(parents=True); selected.write_bytes(b'not yet published')
        item = {'volume_id': self.w['id'], 'path': str(selected.relative_to(self.work)), 'role': 'horizontal', 'published': False}
        self.configure(published=False, finals=[item])
        plan = self.plan_close()
        self.assertTrue(next(f for f in plan['closure_finals'] if f['role'] == 'horizontal')['keep_work'])
        self.assertEqual({e['category'] for e in plan['entries']}, {'raw', 'intermediate'})
        self.assertEqual(self.apply(plan)['state'], 'completed')
        saved = closure.settings(self.catalog, self.project_id)
        self.assertFalse(saved['published'])
        self.assertEqual(saved['finals'][0]['volume_id'], self.w['id'])
        self.assertTrue(selected.exists())
        self.assertFalse((self.editor / 'renders/intermediate.mov').exists())
        self.assertEqual(self.catalog.get('project-state:' + self.project_id)['state'], 'finished')
        item['published'] = True
        config = self.configure(published=False, finals=[item])
        self.assertTrue(config['published'])
        published_plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD', release_work=False)
        self.assertEqual(published_plan['closure_copies'], [])
        self.assertEqual(self.apply(published_plan)['state'], 'completed')
        self.assertFalse(selected.exists())

    def test_already_absent_work_closes_without_deletion_or_clipper(self):
        import shutil

        raw = self.work / '001_proyectos/e64_raw/intro'
        shutil.rmtree(raw)
        shutil.rmtree(self.editor / 'renders')
        short = self.work / '002_finales/e64/02_shorts/intro_vertical_1.mov'
        short.parent.mkdir(parents=True)
        short.write_bytes(b'final vertical')
        finals = closure.settings(self.catalog, self.project_id)['finals']
        finals.append({'volume_id': self.w['id'], 'path': str(short.relative_to(self.work)), 'role': 'vertical', 'published': False})
        self.configure(required=['horizontal', 'vertical'], published=False, finals=finals)
        self.deps.side_effect = RuntimeError('Clipper offline')
        plan = self.plan_close()
        self.assertEqual(plan['entries'], [])
        self.assertEqual(len(plan['closure_copies']), 1)
        self.assertIn({'kind': 'raw', 'path': str(raw.relative_to(self.work))}, plan['closure_scopes'])
        self.assertEqual(self.apply(plan)['state'], 'completed')
        self.assertTrue(self.final.exists())
        self.assertEqual(short.read_bytes(), (self.archive / '002_finales/e64/shorts' / short.name).read_bytes())
        self.assertEqual(self.catalog.get('project-released:' + self.project_id)['bytes'], 0)
        already_backed_up = self.plan_close()
        self.assertEqual(already_backed_up['closure_copies'], [])
        self.assertEqual(self.apply(already_backed_up)['state'], 'completed')

    def test_new_files_in_previously_absent_scopes_invalidate_close(self):
        import shutil

        renders = self.editor / 'renders'
        shutil.rmtree(renders)
        plan = self.plan_close()
        renders.mkdir()
        (renders / 'new.mov').write_bytes(b'new work')
        with self.assertRaisesRegex(MediaError, 'New material'):
            self.apply(plan)
        self.assertTrue((renders / 'new.mov').exists())

    def test_changed_unpublished_work_copy_blocks_other_removals(self):
        work_final = self.work / '002_finales/e64/intro.mov'
        work_final.parent.mkdir(parents=True)
        work_final.write_bytes(self.final.read_bytes())
        self.configure(work_final, self.w, published=False)
        plan = self.plan_close()
        self.assertFalse(plan['closure_copies'])
        work_final.write_bytes(b'new version')
        with self.assertRaisesRegex(MediaError, 'conservada cambió'):
            self.apply(plan)
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())
        self.assertTrue((self.work / '001_proyectos/e64_raw/intro').exists())

    def test_backup_published_finals_before_project_completion_preserves_work(self):
        published = self.work / '002_finales/e64/intro-published.mov'
        waiting = self.work / '002_finales/e64/intro-waiting.mov'
        published.parent.mkdir(parents=True); published.write_bytes(b'published'); waiting.write_bytes(b'waiting')
        items = [{'volume_id': self.w['id'], 'path': str(p.relative_to(self.work)), 'role': 'horizontal', 'published': flag}
                 for p, flag in [(published, True), (waiting, False)]]
        self.configure(no_more=False, published=False, finals=items)
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD', release_work=False)
        self.assertEqual({e['category'] for e in plan['entries']}, {'final'})
        self.assertEqual(len(plan['entries']), 1)
        self.apply(plan)
        self.assertFalse(published.exists()); self.assertTrue(waiting.exists())
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())
        self.assertTrue((self.work / '001_proyectos/e64_raw/intro').exists())
        self.assertIsNone(self.catalog.get('project-released:' + self.project_id))
        config = closure.settings(self.catalog, self.project_id)
        self.assertFalse(config['no_more']); self.assertFalse(config['published'])
        self.assertEqual(config['finals'][0]['volume_id'], self.a['id'])
        for item in config['finals']: item['published'] = True
        self.configure(no_more=True, published=False, finals=config['finals'])
        self.assertEqual(self.apply(self.plan_close())['state'], 'completed')
        self.assertFalse((self.editor / 'renders/intermediate.mov').exists())

    def test_terminal_metadata_failure_does_not_strand_a_journaled_release(self):
        import sqlite3
        from media.common import digest
        plan = self.plan_close()
        before = digest(closure.settings(self.catalog, self.project_id))
        # Abort after some metadata rows were inserted, simulating interruption at final publication.
        self.catalog.db.execute("CREATE TRIGGER fail_close BEFORE INSERT ON metadata WHEN NEW.key='project-state:" + self.project_id + "' BEGIN SELECT RAISE(ABORT,'fixture interruption'); END")
        with self.assertRaises(sqlite3.IntegrityError): self.apply(plan)
        self.assertEqual(digest(closure.settings(self.catalog, self.project_id)), before)
        self.assertEqual(self.catalog.get('release:' + plan['id'])['state'], 'applying')
        self.assertIsNone(self.catalog.get('project-released:' + self.project_id))
        self.catalog.db.execute('DROP TRIGGER fail_close')
        self.assertEqual(self.apply(plan)['state'], 'completed')
        self.assertEqual(self.catalog.db.execute("SELECT count(*) FROM events WHERE job_id=? AND kind='release_completed'", (plan['id'],)).fetchone()[0], 1)

    def test_publication_decisions_have_a_durable_audit_event(self):
        import json
        from media.common import digest
        before = digest(closure.settings(self.catalog, self.project_id))
        self.configure(no_more=False)
        row = self.catalog.db.execute("SELECT payload FROM events WHERE job_id=? AND kind='project_delivery_decision' ORDER BY seq DESC LIMIT 1", (self.project_id,)).fetchone()
        event = json.loads(row[0])
        self.assertEqual(event['previous_digest'], before)
        self.assertFalse(event['no_more'])
        self.assertTrue(event['finals'][0]['published'])
        self.assertTrue(self.final.exists())
