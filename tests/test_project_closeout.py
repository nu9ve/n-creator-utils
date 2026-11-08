"""Destructive project closeout uses fixture volume identities and disposable media only."""
import copy
from pathlib import Path
from unittest import mock

from test_media_release import WorkflowFixture, empty_deps
from media import clipper, closure
from media.common import MediaError
from media.release import apply_release
from media.service import Service


class ClosureTests(WorkflowFixture):
    def setUp(self):
        super().setUp()
        patcher = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        self.deps = patcher.start(); self.addCleanup(patcher.stop)
        # Never ask a real Clipper from tests; each Clipper test fakes its CLI answers.
        patcher = mock.patch('media.clipper.invoke', side_effect=MediaError('clipper_unavailable', 'Clipper en pruebas'))
        self.invoke = patcher.start(); self.addCleanup(patcher.stop)
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
        self.assertFalse(plan['already_complete'])
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

    def test_apply_reports_one_overall_measure_with_a_log(self):
        selected = self.work / '002_finales/e64/03_subidos/26-11-04-intro.mov'
        selected.parent.mkdir(parents=True)
        selected.write_bytes(b'new final')
        self.configure(selected, self.w)
        plan = self.plan_close()
        events = []
        self.apply(plan, progress=lambda stage, **details: events.append(dict(details, stage=stage)))
        measured = [e for e in events if 'overall_total' in e]
        # nüün releases files here, so every backup copy is hashed once (and only once) before the first release.
        verify = sum((self.archive / f['archive_path']).stat().st_size for f in plan['closure_finals'])
        total = plan_total = 2 * sum(f['stamp']['size'] for t in plan['closure_copies'] for f in t['files']) + verify + plan['bytes']
        self.assertTrue(measured and all(e['overall_total'] == total for e in measured))
        done = [e['overall_done'] for e in measured]
        self.assertEqual(done, sorted(done), 'overall progress never goes back')
        last = measured[-1]
        self.assertEqual(last['stage'], 'release_complete')
        self.assertEqual(last['overall_done'], plan_total)
        self.assertEqual(last['files_done'], last['files_total'])
        kinds = [entry['kind'] for entry in last['log']]
        self.assertIn('copied', kinds)
        self.assertEqual(kinds.count('released'), len(plan['entries']))
        self.assertTrue(any(e['current'].get('action') == 'copy' and e['current']['name'] == selected.name for e in measured))

    def test_apply_without_release_watches_reviewed_stamps_instead_of_rehashing(self):
        from media import release
        selected = self.work / '002_finales/e64/02_shorts/intro_vertical_1.mov'
        selected.parent.mkdir(parents=True); selected.write_bytes(b'kept for publishing')
        finals = closure.settings(self.catalog, self.project_id)['finals'] + [
            {'volume_id': self.w['id'], 'path': str(selected.relative_to(self.work)), 'role': 'vertical', 'published': False}]
        self.configure(required=['horizontal', 'vertical'], published=False, finals=finals)
        self.apply(self.plan_close())  # Releases raws and renders; the vertical is copied and kept.
        plan = self.plan_close()
        self.assertEqual((plan['entries'], plan['closure_copies']), ([], []))
        self.assertTrue(all(f.get('archive_stamp') for f in plan['closure_finals']), 'the review stamped every backup copy')
        hashed = []
        original = closure.checked_hash_progress
        def counting(tree, path, *args, **kwargs):
            hashed.append(path)
            return original(tree, path, *args, **kwargs)
        with mock.patch('media.closure.checked_hash_progress', side_effect=counting), \
             mock.patch.object(release, 'checked_hash', side_effect=AssertionError('the backup is not read a second time')):
            self.apply(plan)
        self.assertEqual(hashed, [], 'nothing of nüün released: unchanged reviewed copies are not read again')
        # A backup copy changed after the review is hashed again and the closeout stops.
        plan = self.plan_close()
        self.final.write_bytes(b'tampered')
        with self.assertRaisesRegex(MediaError, 'íntegro'):
            self.apply(plan)

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

    def test_applied_closeout_leaves_no_progress_key(self):
        plan = self.plan_close()
        Service(self.catalog, self.backend).call('closure_apply', {'plan_id': plan['id'], 'digest': plan['digest'], 'confirm_discard': True})
        self.assertIsNone(self.catalog.get('closure-progress:' + plan['id']))

    def test_review_reports_verification_bytes_without_copy_or_release(self):
        from uuid import uuid4

        service = Service(self.catalog, self.backend)
        review_id = str(uuid4())
        key = 'closure-progress:' + review_id
        with mock.patch.object(self.catalog, 'set', wraps=self.catalog.set) as write:
            plan = service.call('closure_plan', {'project_id': self.project_id, 'archive': 'MEDIA',
                                                 'work': 'ESSD', 'review_id': review_id})
        observed = [call.args[1] for call in write.call_args_list if call.args[0] == key]
        self.assertTrue(any(value['stage'] == 'review_final' and value['bytes_done'] > 0 for value in observed))
        self.assertEqual(service.call('closure_progress', {'plan_id': review_id}), {})
        self.assertEqual(plan['state'], 'review')
        self.assertTrue((self.editor / 'renders/intermediate.mov').exists())
        self.assertTrue((self.work / '001_proyectos/e64_raw/intro').exists())

    def test_review_reports_one_overall_measure_and_the_file_in_hand(self):
        selected = self.work / '002_finales/e64/02_shorts/intro_vertical_1.mov'
        selected.parent.mkdir(parents=True); selected.write_bytes(b'vertical waiting')
        finals = closure.settings(self.catalog, self.project_id)['finals'] + [
            {'volume_id': self.w['id'], 'path': str(selected.relative_to(self.work)), 'role': 'vertical', 'published': False}]
        self.configure(required=['horizontal', 'vertical'], published=False, finals=finals)
        events = []
        closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD',
                           progress=lambda stage, **details: events.append(dict(details, stage=stage)))
        measured = [e for e in events if 'overall_total' in e]
        self.assertTrue(measured)
        self.assertEqual(len({e['overall_total'] for e in measured}), 1, 'one total from the start')
        done = [e['overall_done'] for e in measured]
        self.assertEqual(done, sorted(done), 'never goes back across review steps')
        self.assertEqual(measured[-1]['overall_done'], measured[-1]['overall_total'])
        self.assertEqual(measured[-1]['overall_files_done'], measured[-1]['overall_files_total'])
        stages = {e['stage'] for e in measured if e.get('file_total')}
        self.assertTrue({'hashing_source', 'matching_archive', 'review_intermediates', 'review_final'} <= stages)
        self.assertTrue(all(0 < e['file_done'] <= e['file_total'] for e in measured if e.get('file_total')))
        self.assertTrue(any(e['current'].get('from') == 'MEDIA' for e in measured))

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

    def test_projects_report_finals_awaiting_publication_until_backed_up(self):
        from media.projects import overview
        shorts = self.work / '002_finales/e64/02_shorts'
        shorts.mkdir(parents=True)
        waiting = [shorts / f'intro_vertical_{n}.mov' for n in (1, 2)]
        for index, path in enumerate(waiting):
            path.write_bytes(b'vertical %d' % index)
        finals = closure.settings(self.catalog, self.project_id)['finals'] + [
            {'volume_id': self.w['id'], 'path': str(p.relative_to(self.work)), 'role': 'vertical', 'published': False} for p in waiting]
        self.configure(required=['horizontal', 'vertical'], published=False, finals=finals)
        self.apply(self.plan_close())
        row = lambda: next(p for p in overview(self.catalog, [], self.backend) if p['project_id'] == self.project_id)
        sheet = lambda: closure.candidates(self.catalog, self.backend, self.project_id)['deliveries']
        self.assertEqual(row()['stage'], 'finished')
        self.assertTrue(row()['released'])
        self.assertEqual((row()['deliveries']['awaiting'], row()['deliveries']['published']), (2, 0))
        self.assertEqual(sorted(s['name'] for s in sheet() if s['state'] == 'awaiting'), sorted(row()['deliveries']['awaiting_names']))
        # Publishing by moving one short into the published folder is the same file, reported but not relinked.
        uploaded = self.work / '002_finales/e64/03_subidos/shorts' / waiting[0].name
        uploaded.parent.mkdir(parents=True)
        waiting[0].rename(uploaded)
        moved = next(s for s in sheet() if s['name'] == uploaded.name)
        self.assertEqual((moved['state'], moved['current_path']), ('moved', str(uploaded.relative_to(self.work))))
        self.assertEqual((row()['deliveries']['awaiting'], row()['deliveries']['published']), (1, 1))
        self.assertEqual(closure.settings(self.catalog, self.project_id)['finals'][1]['path'], str(waiting[0].relative_to(self.work)))
        # The sheet saves the new location as published; «Respaldar publicados…» clears the mark by itself.
        config = closure.settings(self.catalog, self.project_id)
        for item in config['finals']:
            if Path(item['path']).name == uploaded.name:
                item.update(path=moved['current_path'], published=True)
        self.configure(required=['horizontal', 'vertical'], published=False, finals=config['finals'])
        published_plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD', release_work=False)
        self.apply(published_plan)
        self.assertFalse(uploaded.exists())
        self.assertEqual((row()['deliveries']['awaiting'], row()['deliveries']['published']), (1, 0))
        self.assertEqual([s['state'] for s in sheet() if s['name'] == uploaded.name], ['archived'])
        # A short gone for an unknown reason is not counted as waiting, and nothing claims where it went.
        waiting[1].unlink()
        self.assertEqual((row()['deliveries']['awaiting'], row()['deliveries']['missing']), (0, 1))

    def linked_rerecording(self, retired=True):
        """A second project (`intro-extra`) recorded with Stalker, its editor folder, Clipper cache and a final."""
        import json, uuid
        from media.planner import build_plan
        from media.transfer import Transfer
        card = self.root / 'stalker-card'; bundle = card / 'take.stalker'; bundle.mkdir(parents=True)
        (bundle / 'screen.mov').write_bytes(b'extra screen')
        (bundle / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'sessionID': str(uuid.uuid4()),
                                                          'status': 'completed', 'tracks': [{'relativePath': 'screen.mov'}]}))
        plan = build_plan(self.catalog, self.backend, source=str(card), archive='MEDIA', work='ESSD', project='intro-extra',
                          profile='e64', source_kind='stalker', cleanup=False)
        Transfer(self.catalog, self.backend).apply(plan, plan['digest'])
        extra_id = self.catalog.db.execute("SELECT id FROM projects WHERE title='intro-extra'").fetchone()[0]
        editor = self.work / '001_proyectos/e64/intro-extra'
        (editor / 'renders').mkdir(parents=True); (editor / 'renders/extra-intermediate.mov').write_bytes(b'extra render')
        cache = editor / 'clipper/cache/stalker'; cache.mkdir(parents=True)
        (cache / 'abc.wav').write_bytes(b'extracted audio')
        (cache / '.abc.wav.lock').write_bytes(b'')
        (editor / 'clipper/transcripts').mkdir(); (editor / 'clipper/transcripts/take.json').write_text('{}')
        (editor / 'clipper' / ('clipper-retired-project.json' if retired else 'clipper-project.json')).write_text('{}')
        horizontal = self.archive / '002_finales/e64/2026/intro-extra.mov'
        horizontal.write_bytes(b'extra final')
        closure.save(self.catalog, self.backend, extra_id, ['horizontal'], False, True,
                     [{'volume_id': self.a['id'], 'path': str(horizontal.relative_to(self.archive)), 'role': 'horizontal'}])
        return extra_id, editor

    def test_projects_overview_survives_the_finales_collection(self):
        from media.projects import overview
        self.catalog.project_for('e64', 'finales')
        self.catalog.set('project-closure:' + self.catalog.project_for('e64', 'finales'), {'required': ['horizontal'], 'finals': [], 'no_more': False, 'published': False})
        finales = self.catalog.project_for('e64', 'finales')
        asset = dict(self.catalog.search()[0], project='finales', project_id=finales)
        with mock.patch.object(self.catalog, 'search', return_value=self.catalog.search() + [asset]):
            rows = overview(self.catalog, [], self.backend)
        self.assertEqual(next(r for r in rows if r['project'] == 'finales')['related'], [])
        self.assertIn('intro', [r['project'] for r in rows])

    def regenerable(self, folder, state='retired', complete=True, root=None):
        """A `clipper-regenerable-files/1` document as Clipper's `library cleanup-plan` returns it."""
        import os
        wav = folder / 'cache/stalker/abc.wav'
        export = folder / 'exports/preview.mp4'
        export.parent.mkdir(parents=True, exist_ok=True); export.write_bytes(b'preview')
        rows = [(wav, 'project_cache'), (export, 'exports')]
        entries = [{'path': os.path.realpath(p), 'category': c, 'bytes': p.stat().st_size, 'hardlink_duplicate': False,
                    'stamp': {'device': 1, 'inode': 2, 'size': p.stat().st_size, 'mtime_ns': 3, 'ctime_ns': 4}} for p, c in rows]
        return {'schema': 'clipper-regenerable-files/1', 'project_root': root or os.path.realpath(folder), 'project_state': state,
                'inventory_complete': complete, 'errors': [], 'preserved': ['transcripts'],
                'categories': [{'id': 'project_cache', 'label': 'Project media caches', 'files': 1, 'bytes': wav.stat().st_size,
                                'default_selected': state == 'retired'},
                               {'id': 'exports', 'label': 'Project renders / previews', 'files': 1, 'bytes': export.stat().st_size,
                                'default_selected': False}],
                'entries': entries, 'digest': 'd' * 64}

    def fake_clipper(self, documents, deleted=None, fail=None):
        """Answers `cleanup-plan` from `documents` (by folder) and records `cleanup` requests without deleting."""
        import json, os
        calls = []

        def invoke(catalog, args, timeout=120):
            if args[0] == 'transcripts-export':
                raise MediaError('clipper_failed', 'sin exportación de transcripciones en esta prueba')
            if args[0] == 'cleanup-plan':
                document = documents.get(os.path.realpath(args[1]))
                if document is None:
                    raise MediaError('clipper_failed', 'not a Clipper project')
                return document
            self.assertEqual(args[0], 'cleanup')
            plan = json.loads(Path(args[args.index('--plan') + 1]).read_text())
            categories = [args[i + 1] for i, a in enumerate(args) if a == '--category']
            calls.append({'plan': plan, 'digest': args[args.index('--digest') + 1], 'categories': categories})
            if fail:
                raise fail
            return {'schema': 'clipper-regenerable-cleanup/1', 'project_root': plan['project_root'], 'plan_digest': plan['digest'],
                    'categories': sorted(categories), 'deleted_files': 1, 'deleted_bytes': deleted or 15, 'skipped': []}
        self.invoke.side_effect = invoke
        return calls

    def test_linked_projects_close_together_and_clipper_deletes_its_reviewed_regenerables(self):
        import os
        extra_id, editor = self.linked_rerecording()
        folder = editor / 'clipper'
        document = self.regenerable(folder)
        calls = self.fake_clipper({os.path.realpath(folder): document})
        self.assertTrue(next(c for c in closure.link_candidates(self.catalog, self.project_id) if c['id'] == extra_id)['suggested'])
        before = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        self.assertFalse(any('intro-extra' in e['path'] for e in before['entries']), 'nothing of another project before linking')
        self.assertEqual(before['clipper_regenerable'], [])
        closure.link(self.catalog, self.project_id, extra_id)
        self.assertEqual(closure.related(self.catalog, extra_id), [self.project_id])
        # The rerecording's horizontal covers this project's requirement without being reassigned.
        self.configure(required=['horizontal'], finals=[])
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        self.assertEqual(set(plan['closure_group']), {self.project_id, extra_id})
        self.assertFalse(any('/clipper/' in e['path'] for e in plan['entries']), 'nüün itself retires nothing inside clipper/')
        [group] = plan['clipper_regenerable']
        self.assertEqual(group['editor'], '001_proyectos/e64/intro-extra/clipper')
        self.assertEqual([c['id'] for c in group['categories']], ['project_cache'])
        self.assertEqual([Path(e['path']).name for e in group['entries']], ['abc.wav'])
        self.assertEqual((plan['clipper_files'], plan['clipper_bytes']), (1, len(b'extracted audio')))
        self.assertTrue(any('exports' in n or 'Project renders' in n for n in plan['retained_editors']), 'unselected categories are listed as kept')
        self.assertTrue(any('intro/clipper' in n and 'se conserva todo' in n for n in plan['retained_editors']), 'no answer offers nothing')
        self.assertIn('001_proyectos/e64/intro-extra/renders/extra-intermediate.mov', [e['path'] for e in plan['entries']])
        self.assertTrue(any(f['owner'] == extra_id for f in plan['closure_finals'] if f['role'] == 'horizontal'))
        with self.assertRaisesRegex(MediaError, 'final_already_linked|otro proyecto'):
            self.configure(finals=closure.settings(self.catalog, extra_id)['finals'])
        result = self.apply(plan)
        self.assertEqual(calls, [{'plan': document, 'digest': 'd' * 64, 'categories': ['project_cache']}])
        self.assertEqual(result['clipper_cleaned'][document['project_root']]['deleted_bytes'], 15)
        # Clipper was faked: whatever is still there is what nüün did not delete on its own.
        for kept in ('cache/stalker/abc.wav', 'cache/stalker/.abc.wav.lock', 'transcripts/take.json', 'clipper-retired-project.json', 'exports/preview.mp4'):
            self.assertTrue((folder / kept).exists(), kept)
        self.assertFalse((editor / 'renders/extra-intermediate.mov').exists())
        for pid in (self.project_id, extra_id):
            self.assertEqual(self.catalog.get('project-state:' + pid)['state'], 'finished')
            self.assertTrue(self.catalog.get('project-released:' + pid))
        self.assertEqual(self.catalog.db.execute("SELECT COUNT(*) FROM projects WHERE title IN ('intro','intro-extra')").fetchone()[0], 2)
        self.apply(plan)
        self.assertEqual(len(calls), 1, 'a completed request is not repeated')

    def test_clipper_failure_after_release_is_recorded_and_asked_again(self):
        import os
        extra_id, editor = self.linked_rerecording()
        closure.link(self.catalog, self.project_id, extra_id)
        self.configure(required=['horizontal'], finals=[])
        document = self.regenerable(editor / 'clipper')
        calls = self.fake_clipper({os.path.realpath(editor / 'clipper'): document}, fail=MediaError('clipper_failed', 'Storage ocupado'))
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        result = self.apply(plan)
        self.assertEqual(result['state'], 'completed')
        self.assertIn('Storage ocupado', result['clipper_cleaned'][document['project_root']]['error'])
        self.fake_clipper({os.path.realpath(editor / 'clipper'): document})
        again = self.apply(plan)
        self.assertEqual(again['clipper_cleaned'][document['project_root']]['deleted_files'], 1)
        self.assertEqual(len(calls), 1)

    def test_clipper_regenerables_need_a_complete_matching_answer(self):
        import os
        extra_id, editor = self.linked_rerecording(retired=False)
        closure.link(self.catalog, self.project_id, extra_id)
        folder = os.path.realpath(editor / 'clipper')
        cases = {
            'active': self.regenerable(editor / 'clipper', state='active'),
            'incomplete': self.regenerable(editor / 'clipper', complete=False),
            'other folder': self.regenerable(editor / 'clipper', root='/elsewhere/clipper'),
            'bad schema': dict(self.regenerable(editor / 'clipper'), schema='clipper-regenerable-files/2'),
        }
        for name, document in cases.items():
            self.fake_clipper({folder: document})
            plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
            self.assertEqual(plan['clipper_regenerable'], [], name)
            self.assertTrue(any('intro-extra/clipper' in n for n in plan['retained_editors']), name)
        self.invoke.side_effect = MediaError('clipper_unavailable', 'Clipper offline')
        offline = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        self.assertEqual(offline['clipper_regenerable'], [], 'unknown Clipper keeps everything')
        self.assertTrue((editor / 'clipper/cache/stalker/abc.wav').exists())

    def test_review_from_the_old_stalker_audio_exception_must_be_redone(self):
        plan = self.plan_close()
        stored = self.catalog.get('release:' + plan['id'])
        stored['entries'].append(dict(stored['entries'][0], path='001_proyectos/e64/intro/clipper/cache/stalker/a.wav',
                                      category='intermediate', origin='clipper_cache'))
        self.catalog.set('release:' + plan['id'], stored)
        with self.assertRaisesRegex(MediaError, 'anterior a la lista'):
            self.apply(stored)

    def test_link_change_after_review_invalidates_the_plan(self):
        extra_id, _ = self.linked_rerecording()
        closure.link(self.catalog, self.project_id, extra_id)
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        closure.link(self.catalog, self.project_id, extra_id, linked=False)
        with self.assertRaisesRegex(MediaError, 'Cambió'):
            self.apply(plan)
        self.assertTrue((self.work / '001_proyectos/e64/intro-extra/clipper/cache/stalker/abc.wav').exists())

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
        self.assertTrue(already_backed_up['already_complete'])
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
