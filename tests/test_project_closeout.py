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
