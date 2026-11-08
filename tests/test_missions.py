"""Mission registry (Atlas A13) on synthetic exports in temporary folders only."""
import gzip
import hashlib
import json
import unittest

from media.common import MediaError
from media.recipes import TEMPLATE
from media.service import Service
from tests.test_media_engine import MediaFixture

AREA = '004_datos/subaereo/coverage'


def capture(cid, at, **extra):
    return dict({'captureID': cid, 'capturedAt': at, 'planID': 'P1', 'sessionID': 'S1',
                 'mediaCreatedAt': at[:10] + ' ' + at[11:19], 'imageJPEGReference': 'DJI_0001.JPG'}, **extra)


def export(at, kind=None, plans=(), log=(), **extra):
    value = {'schemaVersion': 3, 'exportedAt': at, 'plans': list(plans), 'captureLog': list(log), 'captures': [],
             'flightSessions': [], 'fieldNotes': []}
    if kind:
        value['exportWindow'] = {'kind': kind}
    value.update(extra)
    return value


class MissionRegistryTests(MediaFixture):
    def setUp(self):
        super().setUp()
        self.inbox = self.root / 'Downloads'
        self.inbox.mkdir()
        value = json.loads(json.dumps(TEMPLATE))
        value['missions'][0]['inbox'] = str(self.inbox)
        self.catalog.set('recipe', value)
        self.service = Service(self.catalog, self.backend)

    def drop(self, name, value, folder=None):
        path = (folder or self.inbox) / name
        path.write_text(json.dumps(value))
        return path

    def review(self, **params):
        return self.service.call('missions_review', dict({'archive': 'MEDIA'}, **params))

    def incorporate(self, reviewed, **params):
        return self.service.call('missions_incorporate', dict({'archive': 'MEDIA', 'work': 'ESSD',
                                                               'digest_value': reviewed['digest']}, **params))

    def history(self):
        return json.loads((self.archive / AREA / 'history.json').read_text())

    def changes(self):
        return [json.loads(l) for l in (self.archive / AREA / 'changes.ndjson').read_text().splitlines()]

    def test_review_reads_only_and_incorporate_merges_mirrors_and_clears_the_inbox(self):
        full = self.drop('coverage-history-1.json', export('2026-11-06T05:00:00Z', 'all', [{'id': 'P1', 'name': 'Calles', 'updatedAt': 'a'}],
                                                           [capture('C1', '2026-11-05T17:33:00Z'), capture('C2', '2026-11-05T17:33:05Z')]))
        self.drop('JSON-24h.json', export('2026-11-08T08:00:00Z', '24h', [{'id': 'P1', 'name': 'Calles', 'updatedAt': 'b'}],
                                          [capture('C2', '2026-11-05T17:33:05Z', lightClass='night'), capture('C3', '2026-11-07T23:13:40Z')]))
        (self.inbox / 'JSON-broken.json').write_text('{"plans": [')
        (self.inbox / 'notes.json').write_text('{}')
        reviewed = self.review()
        self.assertFalse((self.archive / '004_datos').exists())
        states = {f['name']: f['state'] for f in reviewed['files']}
        self.assertEqual(states, {'coverage-history-1.json': 'new', 'JSON-24h.json': 'new', 'JSON-broken.json': 'invalid'})
        self.assertEqual(next(f for f in reviewed['files'] if f['name'] == 'JSON-24h.json')['changes'], {'new': 1, 'updated': 2, 'deleted': 0})
        raw = full.read_bytes()
        result = self.incorporate(reviewed)
        self.assertEqual(result['changes'], {'new': 4, 'updated': 2, 'deleted': 0})
        self.assertEqual(sorted(result['removed_from_inbox']), ['JSON-24h.json', 'coverage-history-1.json'])
        self.assertEqual(sorted(p.name for p in self.inbox.iterdir()), ['JSON-broken.json', 'notes.json'])
        history = self.history()
        self.assertEqual([c['captureID'] for c in history['captureLog']], ['C1', 'C2', 'C3'])
        self.assertEqual(history['captureLog'][1]['lightClass'], 'night')
        self.assertEqual(history['plans'][0]['updatedAt'], 'b')
        self.assertEqual(history['exportWindow']['kind'], 'merged')
        self.assertEqual([s['file'] for s in history['mergedFrom']], ['coverage-history-1.json', 'JSON-24h.json'])
        sha = hashlib.sha256(raw).hexdigest()
        self.assertEqual(history['nuun']['records']['captureLog']['C1'], sha)
        self.assertEqual(gzip.decompress((self.archive / AREA / 'raw/2026/11/coverage-history-1.json.gz').read_bytes()), raw)
        lines = self.changes()
        self.assertEqual(len(lines), 6)
        self.assertEqual({(l['list'], l['id'], l['change']) for l in lines if l['export'] == 'JSON-24h.json'},
                         {('captureLog', 'C2', 'updated'), ('captureLog', 'C3', 'new'), ('plans', 'P1', 'updated')})
        mirror = self.work / '001_proyectos/atlas-subaereo/_missions'
        self.assertEqual((mirror / 'history.json').read_bytes(), (self.archive / AREA / 'history.json').read_bytes())
        self.assertEqual((mirror / 'changes.ndjson').read_bytes(), (self.archive / AREA / 'changes.ndjson').read_bytes())
        status = self.service.call('missions_status', {})
        self.assertEqual(status['coverage']['last_capture_at'], '2026-11-07T23:13:40Z')
        self.assertEqual(status['coverage']['exported_until'], '2026-11-08T08:00:00Z')
        self.assertEqual(status['mirror'], 'written')

    def test_same_bytes_again_add_nothing_and_still_leave_the_inbox(self):
        value = export('2026-11-06T05:00:00Z', 'all', log=[capture('C1', '2026-11-05T17:33:00Z')])
        self.drop('coverage-a.json', value)
        self.incorporate(self.review())
        self.drop('coverage-a.json', value)
        reviewed = self.review()
        self.assertEqual(reviewed['files'][0]['state'], 'incorporated')
        result = self.incorporate(reviewed)
        self.assertEqual((result['incorporated'], result['removed_from_inbox']), ([], ['coverage-a.json']))
        self.assertEqual(len(self.changes()), 1)

    def test_an_older_export_arriving_later_never_wins(self):
        self.drop('JSON-new.json', export('2026-11-08T08:00:00Z', '24h', log=[capture('C1', '2026-11-05T17:33:00Z', lightClass='night')]))
        self.incorporate(self.review())
        self.drop('coverage-old.json', export('2026-06-01T07:00:00Z', log=[capture('C1', '2026-11-05T17:33:00Z', lightClass='day')]))
        reviewed = self.review()
        self.assertEqual(reviewed['files'][0]['state'], 'nothing_new')
        self.incorporate(reviewed)
        history = self.history()
        self.assertEqual(history['captureLog'][0]['lightClass'], 'night')
        self.assertEqual(history['exportedAt'], '2026-11-08T08:00:00Z')
        self.assertEqual(len(history['mergedFrom']), 2)

    def test_files_outside_the_inbox_are_imported_but_never_deleted(self):
        atlas = self.root / 'atlas/coverage_exports'
        atlas.mkdir(parents=True)
        path = self.drop('coverage-export-20260531-165155.json', export('2026-05-31T23:51:55Z', schemaVersion=1, plans=[{'id': 'P0'}]), atlas)
        self.drop('JSON-today.json', export('2026-11-08T08:00:00Z', log=[capture('C9', '2026-11-07T23:00:00Z')]))
        reviewed = self.review(paths=[str(path)])
        self.assertEqual([f['in_inbox'] for f in reviewed['files']], [False])
        result = self.incorporate(reviewed, paths=[str(path)])
        self.assertEqual((result['incorporated'], result['removed_from_inbox']), (['coverage-export-20260531-165155.json'], []))
        self.assertTrue(path.exists())
        self.assertTrue((self.inbox / 'JSON-today.json').exists())
        kept = self.review()
        self.incorporate(kept, remove_from_inbox=False)
        self.assertTrue((self.inbox / 'JSON-today.json').exists())

    def test_a_changed_export_or_registry_needs_a_new_review(self):
        path = self.drop('JSON-a.json', export('2026-11-08T08:00:00Z', log=[capture('C1', '2026-11-05T17:33:00Z')]))
        reviewed = self.review()
        path.write_text(json.dumps(export('2026-11-08T09:00:00Z', log=[capture('C2', '2026-11-05T17:33:00Z')])))
        with self.assertRaises(MediaError) as raised:
            self.incorporate(reviewed)
        self.assertEqual(raised.exception.code, 'review_changed')
        self.assertFalse((self.archive / AREA).exists())

    def test_an_interrupted_import_never_leaves_extra_change_lines(self):
        self.drop('JSON-a.json', export('2026-11-07T08:00:00Z', log=[capture('C1', '2026-11-05T17:33:00Z')]))
        self.incorporate(self.review())
        with (self.archive / AREA / 'changes.ndjson').open('a') as stream:
            stream.write('{"list":"captureLog","id":"ghost","change":"new"}\n')
        self.drop('JSON-b.json', export('2026-11-08T08:00:00Z', log=[capture('C2', '2026-11-06T17:33:00Z')]))
        self.incorporate(self.review())
        self.assertEqual([l['id'] for l in self.changes()], ['C1', 'C2'])
        self.assertEqual(self.history()['nuun']['changes_count'], 2)

    def test_tombstones_delete_and_an_older_export_does_not_bring_back(self):
        self.drop('JSON-a.json', export('2026-11-07T08:00:00Z', plans=[{'id': 'P1'}, {'id': 'DUP'}]))
        self.incorporate(self.review())
        self.drop('JSON-b.json', export('2026-11-08T08:00:00Z', plans=[{'id': 'P1'}], deletedPlanIDs=['DUP']))
        self.drop('coverage-old.json', export('2026-11-06T08:00:00Z', plans=[{'id': 'DUP'}]))
        self.incorporate(self.review())
        self.assertEqual([p['id'] for p in self.history()['plans']], ['P1'])
        self.assertIn(('plans', 'DUP', 'deleted'), {(l['list'], l['id'], l['change']) for l in self.changes()})

    def test_offline_work_leaves_the_mirror_pending(self):
        self.drop('JSON-a.json', export('2026-11-08T08:00:00Z', log=[capture('C1', '2026-11-05T17:33:00Z')]))
        self.backend.offline.add(str(self.work))
        result = self.incorporate(self.review())
        self.assertEqual(result['mirror'], 'pending')
        self.assertTrue((self.archive / AREA / 'history.json').exists())

    def test_without_archive_review_only_describes(self):
        self.drop('JSON-a.json', export('2026-11-08T08:00:00Z', log=[capture('C1', '2026-11-05T17:33:00Z')]))
        reviewed = self.service.call('missions_review', {})
        self.assertEqual(reviewed['files'][0]['state'], 'unknown_registry')
        self.assertEqual(reviewed['files'][0]['photos'], 1)


if __name__ == '__main__':
    unittest.main()
