"""Every long action reports one overall measure (nu9ve-interaction-v1 «Acciones largas»).

Same contract as the closeout's test_apply_reports_one_overall_measure_with_a_log: bytes that never go back, 100 % at
the end, files counted and a short log of finished steps. Fixture volumes and temporary media only.
"""
import itertools
from unittest import mock

from test_media_engine import MediaFixture
from test_media_release import WorkflowFixture, empty_deps
from media.library import deliver_work
from media.release import apply_release, plan_release
from media.service import Service
from media.transfer import Transfer


def flushing_every_report():
    """Transfer writes at most once per second; a clock that jumps lets the test see every report."""
    clock = itertools.count(0, 5)
    return mock.patch('media.transfer.time.monotonic', side_effect=lambda: next(clock))


def assert_one_measure(test, measured, log_kinds):
    test.assertTrue(measured, 'the action reported its measure')
    totals = {e['overall_total'] for e in measured}
    test.assertEqual(len(totals), 1, 'one total for the whole action')
    total = totals.pop()
    test.assertGreater(total, 0)
    done = [e['overall_done'] for e in measured]
    test.assertEqual(done, sorted(done), 'overall progress never goes back')
    test.assertEqual(measured[-1]['overall_done'], total, 'ends at 100 %')
    kinds = [line['kind'] for line in measured[-1]['log']]
    for kind, count in log_kinds.items():
        test.assertEqual(kinds.count(kind), count, kind)
    return total


class IngestProgressTests(MediaFixture):
    def test_apply_reports_one_overall_measure_with_a_log(self):
        plan = self.plan()
        engine = Transfer(self.catalog, self.backend)
        snapshots = []
        engine.sink = snapshots.append
        with flushing_every_report():
            result = engine.apply(plan, plan['digest'])
        self.assertEqual(result['state'], 'completed')
        measured = [s for s in snapshots if 'overall_total' in s]
        files = len(plan['files'])
        total = assert_one_measure(self, measured, {'copied': files, 'cleaned': files})
        size = sum(f['stamp']['size'] for f in plan['files'])
        # Read once, verified on two disks, re-read before cleaning the card.
        self.assertEqual(total, size + 2 * size + size)
        copied = next(line for line in measured[-1]['log'] if line['kind'] == 'copied')
        self.assertEqual(copied['target'], 'ESSD + MEDIA')
        self.assertTrue(any(s['current'].get('action') == 'copy' and s['current']['from'] == 'card' for s in measured))
        self.assertTrue(any(s['current'].get('action') == 'release' for s in measured))
        # The job's own progress (what Actividad and the app poll) carries the same fields.
        stored = self.catalog.get('progress:' + plan['id'], {})
        self.assertEqual(stored['overall_done'], stored['overall_total'])


class ReleaseProgressTests(WorkflowFixture):
    def setUp(self):
        super().setUp()
        patcher = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        patcher.start(); self.addCleanup(patcher.stop)
        self.run_plan()

    def test_apply_reports_one_overall_measure_with_a_log(self):
        plan = plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path='001_proyectos/e64_raw/intro', project='intro')
        events = []
        result = apply_release(self.catalog, self.backend, plan['id'], plan['digest'],
                               progress=lambda stage, **details: events.append(dict(details, stage=stage)))
        self.assertEqual(result['state'], 'completed')
        measured = [e for e in events if 'overall_total' in e]
        total = assert_one_measure(self, measured, {'released': len(plan['entries'])})
        # Verify each archive copy, then re-read each original before retiring it.
        self.assertEqual(total, 2 * plan['bytes'])
        self.assertEqual(measured[-1]['stage'], 'release_complete')
        self.assertTrue(any(e['current'].get('action') == 'verify' and e['current']['from'] == 'MEDIA' for e in measured))
        self.assertTrue(any(e['current'].get('action') == 'release' and e['current']['from'] == 'ESSD' for e in measured))
        self.assertEqual({line['target'] for line in measured[-1]['log']}, {'ESSD'})

    def test_service_keeps_the_measure_for_the_reader(self):
        plan = plan_release(self.catalog, self.backend, 'MEDIA', work='ESSD', path='001_proyectos/e64_raw/intro', project='intro')
        service = Service(self.catalog, self.backend)
        seen = []
        original = service.catalog.set
        def remember(key, value):
            if key == 'action-progress:' + plan['id']:
                seen.append(value)
            return original(key, value)
        with mock.patch.object(service.catalog, 'set', side_effect=remember):
            service.call('apply_release', {'plan_id': plan['id'], 'digest': plan['digest']})
        self.assertEqual(seen[-1]['stage'], 'release_complete')
        self.assertEqual(seen[-1]['overall_done'], seen[-1]['overall_total'])
        # The reader saw it while it ran; a finished action leaves no key behind.
        self.assertEqual(service.call('action_progress', {'id': plan['id']}), {})


class DeliveryProgressTests(MediaFixture):
    def test_apply_reports_one_overall_measure_with_a_log(self):
        self.backend.offline.add(str(self.work))
        self.run_plan(self.plan(archive_only=True, confirm_archive_only=True))
        self.backend.offline.clear()
        pending = self.catalog.search(work_state='pending')
        events = []
        with flushing_every_report():
            deliver_work(self.catalog, self.backend, [pending[0]['id']], 'ESSD',
                         progress=lambda stage, **details: events.append(dict(details, stage=stage)))
        measured = [e for e in events if 'overall_total' in e]
        delivered = list(self.work.rglob('*.JPG')) + list(self.work.rglob('*.DNG'))
        total = assert_one_measure(self, measured, {'copied': len(delivered)})
        self.assertEqual(total, 2 * sum(p.stat().st_size for p in delivered))
        self.assertEqual(measured[-1]['files_done'], measured[-1]['files_total'])
        self.assertTrue(any(e['current'].get('from') == 'MEDIA' and e['current'].get('to') == 'ESSD' for e in measured))
        self.assertEqual(self.catalog.search()[0]['work_delivery_status'], 'ready')
