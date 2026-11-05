"""Assignment, support recovery and progress boundaries; disposable fixture volumes only."""
import copy
from unittest import mock

from test_media_engine import MediaFixture
from media.catalog import Catalog
from media.common import MediaError, digest
from media.planner import validate_plan
from media.recovery import recover_catalog
from media.service import Service
from media.transfer import Transfer


class SimplifiedIngestTests(MediaFixture):
    def support(self):
        path = self.source / 'AVF_INFO/AVIN0001.BNP'
        path.parent.mkdir(); path.write_bytes(b'camera index')
        (self.source / 'MEDIAPRO.XML').write_text('<support/>')

    def test_per_content_project_and_camera_support_need_no_batch_project(self):
        self.support()
        assignments = {p: {'project': 'assigned'} for p in ('DCIM/photo.JPG', 'DCIM/photo.DNG')}
        plan = self.plan(project='', assignments=assignments)
        support = next(u for u in plan['units'] if u.get('support'))
        self.assertFalse(support['work_selected'])
        self.assertEqual(len(support['paths']), 2)
        self.run_plan(plan)
        self.assertEqual([r[0] for r in self.catalog.db.execute('SELECT title FROM projects')], ['assigned'])
        import json
        receipt = json.loads(next((self.archive / '.ncreator/receipts').glob('*.json')).read_text())
        self.assertEqual(receipt['schema'], 'nu9ve-transfer-receipt/2')
        self.assertIsNone(next(u for u in receipt['units'] if u.get('support'))['project_id'])
        self.assertEqual(len(list(self.catalog.db.execute('SELECT * FROM copies WHERE volume_id=?', (self.a['id'],)))), 4)
        fresh = Catalog(self.root / 'recovered')
        self.addCleanup(fresh.close)
        fresh.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        recover_catalog(fresh, self.backend, 'MEDIA')
        self.assertEqual([r[0] for r in fresh.db.execute('SELECT title FROM projects')], ['assigned'])
        self.assertEqual(len(fresh.search()), 1)

    def test_unassigned_primary_still_blocks_and_paired_sidecars_inherit(self):
        self.support()
        with self.assertRaises(MediaError): self.plan(project='')
        plan = self.plan(project='', assignments={'DCIM/photo.JPG': {'project': 'one'}})
        self.assertEqual({u['project'] for u in plan['units'] if not u.get('support')}, {'one'})
        discovered = Service(self.catalog, self.backend).call('discover', {'source': str(self.source)})
        self.assertEqual(len(discovered['units']), 2)
        self.assertEqual(sum(u['category'] == 'support' for u in discovered['units']), 1)

    def test_sony_plan_explains_index_retention_and_work_copies(self):
        self.support()
        plan = self.plan(camera='sony')
        rows = {r['source']: r for r in plan['preview']}
        self.assertEqual(rows['AVF_INFO/AVIN0001.BNP']['source_action'], 'camera_format')
        self.assertEqual(rows['DCIM/photo.JPG']['source_action'], 'release_verified')
        self.assertIsNone(rows['AVF_INFO/AVIN0001.BNP']['work_path'])
        self.assertEqual(self.run_plan(plan)['state'], 'camera_action_required')
        self.assertTrue((self.source / 'AVF_INFO/AVIN0001.BNP').exists())

    def test_a_media_unit_cannot_be_disguised_as_support(self):
        plan = self.plan()
        plan['units'][0]['support'] = True
        plan.pop('digest'); plan['digest'] = digest(plan)
        with self.assertRaises(MediaError): validate_plan(plan)

    def test_global_progress_does_not_reset_between_files_verification_and_cleanup(self):
        observed = []
        original = self.catalog.set
        def record(key, value):
            if key.startswith('progress:'): observed.append(copy.deepcopy(value))
            return original(key, value)
        transfer = Transfer(self.catalog, self.backend)
        transfer.hook = lambda stage, _: transfer.report(force=True) if stage in {'before_verify', 'after_file_verified', 'before_cleanup'} else None
        with mock.patch.object(self.catalog, 'set', side_effect=record):
            plan = self.plan()
            result = transfer.apply(plan, plan['digest'])
        fractions = [p['global_fraction'] for p in observed]
        self.assertEqual(fractions, sorted(fractions))
        self.assertEqual(fractions[-1], 1)
        self.assertTrue(any(0 < f < 1 for f in fractions))
        self.assertEqual(result['state'], 'completed')
        self.assertEqual(observed[-1]['global_done'], observed[-1]['global_total'])
        self.assertGreater(observed[-1]['cleanup_bytes'], 0)

    def test_failure_never_reports_global_completion_or_cleans_unverified_sources(self):
        plan = self.plan()
        def fail(stage, _):
            if stage == 'before_verify': raise RuntimeError('verification interrupted')
        with self.assertRaises(RuntimeError): self.run_plan(plan, hook=fail)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        self.assertLess(self.catalog.get('progress:' + plan['id'])['global_fraction'], 1)

    def test_support_only_can_be_archived_without_creating_a_project(self):
        for p in (self.source / 'DCIM').iterdir(): p.unlink()
        self.support()
        plan = self.plan(project='', work=None, archive_only=True, confirm_archive_only=True, work_decision='skipped')
        self.run_plan(plan)
        self.assertFalse(self.catalog.search())
        self.assertFalse(list(self.catalog.db.execute('SELECT * FROM projects')))
        from media.volumes import card_hint
        hint = card_hint(self.source, self.catalog, self.backend.describe(self.source))
        self.assertNotIn('last_project', hint)

    def test_recovery_rejects_unknown_receipt_major(self):
        import json
        self.support()
        self.run_plan()
        path = next((self.archive / '.ncreator/receipts').glob('*.json'))
        receipt = json.loads(path.read_text()); receipt['schema'] = 'nu9ve-transfer-receipt/999'
        path.write_text(json.dumps(receipt))
        fresh = Catalog(self.root / 'new-catalog'); self.addCleanup(fresh.close)
        fresh.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        with self.assertRaisesRegex(MediaError, 'Unsupported transfer receipt'): recover_catalog(fresh, self.backend, 'MEDIA')
        self.assertFalse(fresh.search())

    def test_new_review_fields_do_not_reuse_a_legacy_job_id_with_another_digest(self):
        import uuid
        from media.common import canonical, now
        plan = self.plan(cleanup=False)
        basis = {k: v for k, v in plan.items() if k not in {'id', 'digest', 'preview', 'destinations', 'planner_revision'}}
        legacy_id = str(uuid.uuid5(uuid.NAMESPACE_URL, digest(basis)))
        self.assertNotEqual(plan['id'], legacy_id)
        with self.catalog.db:
            self.catalog.db.execute('INSERT INTO jobs VALUES(?,?,?,?,?,?,?)',
                                    (legacy_id, 'legacy-digest', canonical(dict(plan, id=legacy_id)).decode(), 'source_retained', None, now(), now()))
        self.assertEqual(self.plan(cleanup=False)['id'], plan['id'])
        self.assertEqual(self.run_plan(plan)['state'], 'source_retained')
