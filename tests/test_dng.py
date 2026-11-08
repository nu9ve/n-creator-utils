"""DNG sample in work and release of the rest (Atlas A16), on temporary fixtures only."""
import json
import os
import unittest

from media.common import MediaError
from media.dng import select
from tests.test_devices import dng, jpeg
from tests.test_mission_match import MissionFixture


class DngTests(MissionFixture):
    """Reuses the mission fixture: card with DJI_0001, registry, archive and work volumes."""

    def setUp(self):
        super().setUp()
        self.registry(self.record(lightClass='night', qualityGrade='poor'),
                      self.record(when='2026-11-04 16:33:00', name='DJI_0002.JPG', cid='C2', lightClass='night', qualityGrade='poor'))
        self.pair('DJI_0002', when='2026:11:04 16:33:00')
        old = self.work / '001_proyectos/atlas-subaereo/101MEDIA'
        old.mkdir(parents=True)
        (old / 'DJI_0001.JPG').write_bytes(jpeg())
        (old / 'DJI_0001.DNG').write_bytes(dng())
        (old / 'DJI_0911.DNG').write_bytes(b'august, never backed up')
        self.apply(self.plan(cleanup=False))
        self.ids = {line['original_name']: line['asset_id'] for line in self.units_map()}

    def dng_plan(self, **params):
        return self.service.call('dng_plan', dict({'archive': 'MEDIA', 'work': 'ESSD'}, **params))

    def trash(self):
        folder = self.work / '.Trashes' / str(os.getuid())
        return sorted(p.name for p in folder.iterdir()) if folder.exists() else []

    def test_release_moves_verified_work_dngs_to_the_trash_and_copies_the_sample(self):
        value = self.dng_plan(keep=[self.ids['DJI_0002.JPG']], trash_unbacked=['001_proyectos/atlas-subaereo/101MEDIA/DJI_0911.DNG'])
        self.assertEqual([r['work'] for r in value['release']], ['001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.DNG'])
        self.assertEqual(len(value['copy']), 1)
        self.assertTrue(value['copy'][0]['work'].startswith('001_proyectos/atlas-subaereo/misiones/'))
        result = self.service.call('dng_apply', {'plan': value, 'digest': value['digest']})
        self.assertEqual((result['copied'], result['released'], result['trashed_unbacked']), (1, 1, 1))
        self.assertFalse((self.work / '001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.DNG').exists())
        self.assertTrue((self.work / '001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.JPG').exists())
        self.assertTrue((self.work / value['copy'][0]['work']).is_file())
        self.assertEqual(self.trash(), ['DJI_0001.DNG', 'DJI_0911.DNG'])
        lines = {line['original_name']: line for line in self.units_map()}
        self.assertIsNone(lines['DJI_0001.JPG']['work']['dng'])
        self.assertTrue(lines['DJI_0002.JPG']['work']['dng'].startswith('misiones/'))

    def test_a_work_dng_that_differs_from_the_archive_is_never_released(self):
        value = self.dng_plan(keep=[])
        (self.work / '001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.DNG').write_bytes(dng(when='2026:05:30 10:00:00'))
        with self.assertRaises(MediaError) as raised:
            self.service.call('dng_apply', {'plan': value, 'digest': value['digest']})
        self.assertEqual(raised.exception.code, 'checksum_mismatch')
        self.assertTrue((self.work / '001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.DNG').exists())
        self.assertEqual(self.trash(), [])

    def test_only_listed_unbacked_files_inside_the_root_can_go_to_the_trash(self):
        for bad in (['../outside.DNG'], ['001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.DNG'], ['001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.JPG']):
            with self.assertRaises(MediaError):
                self.dng_plan(keep=[], trash_unbacked=bad)
        value = self.dng_plan(keep=[])
        with self.assertRaises(MediaError) as raised:
            self.service.call('dng_apply', {'plan': value, 'digest': 'other'})
        self.assertEqual(raised.exception.code, 'review_changed')


class SelectTests(unittest.TestCase):
    def test_sample_covers_labels_each_light_poor_and_an_older_mission(self):
        units, log, plans = [], [], []
        def session(sid, plan, start, n, light, quality):
            plans.append({'id': plan, 'name': plan})
            for i in range(n):
                cid = '{}-{}'.format(sid, i)
                units.append({'asset_id': cid, 'original_name': 'DJI_{:04d}.JPG'.format(i), 'captured_at': '{}T10:{:02d}:00'.format(start, i),
                              'archive': {'dng': 'x/{}.DNG'.format(cid)}, 'mission': {'sessionID': sid, 'planID': plan, 'captureID': cid}})
                log.append({'captureID': cid, 'lightClass': light, 'qualityGrade': quality})
        session('old', 'grid-may', '2026-05-30', 40, None, None)
        session('dusk', 'dusk-plan', '2026-11-05', 30, 'diffuseDusk', 'good')
        session('night1', 'night-a', '2026-11-06', 30, 'night', 'poor')
        session('night2', 'night-b', '2026-11-07', 30, 'night', 'poor')
        session('sun', 'sun-plan', '2026-11-07', 30, 'lowSun', 'good')
        labels = {'DJI_0003_20261105100300': 'set-v1'}
        rules = {'light_classes': ['lowSun', 'diffuseDusk', 'night'], 'dominance': 0.6, 'per_light': 5, 'poor_sample': 4,
                 'older_sample': 3, 'older_than_days': 30}
        groups = {g['reason'] + ':' + g['detail']: g['asset_ids'] for g in select(units, {'captureLog': log, 'plans': plans}, labels, rules)}
        self.assertEqual(groups['labelled:set-v1'], ['dusk-3'])
        self.assertEqual(len(groups['light:lowSun · sun-plan']), 5)
        self.assertEqual(len(groups['light:diffuseDusk · dusk-plan']), 5)
        self.assertNotIn('dusk-3', groups['light:diffuseDusk · dusk-plan'])  # already kept as labelled
        self.assertEqual(len(groups['light:night · night-b']), 5)          # newest night session
        self.assertEqual(len(groups['poor:night-a']), 4)                   # poor from another session
        self.assertEqual(len(groups['older:grid-may']), 3)
        ids = [a for g in groups.values() for a in g]
        self.assertEqual(len(ids), len(set(ids)))


if __name__ == '__main__':
    unittest.main()
