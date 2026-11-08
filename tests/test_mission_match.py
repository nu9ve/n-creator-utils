"""Mission or not for drone stills, work JPG only and units.ndjson (Atlas A12), on temporary fixtures only."""
import json
import unittest

from media.service import Service
from tests.test_devices import DroneFixture
from tests.test_missions import capture, export

NAME = '26NOV04-163212_MAVICAIR2_MX-{}[].{}'
LAT, LON = 29.0984139, -110.9622728   # the fixture EXIF position


class MissionFixture(DroneFixture):
    """A drone card plus a mission registry; helpers only, no tests."""
    def setUp(self):
        super().setUp()
        self.inbox = self.root / 'Downloads'
        self.inbox.mkdir()
        value = self.catalog.get('recipe') or json.loads(json.dumps(__import__('media.recipes', fromlist=['TEMPLATE']).TEMPLATE))
        value['missions'][0]['inbox'] = str(self.inbox)
        self.catalog.set('recipe', value)
        self.service = Service(self.catalog, self.backend)
        self.pair('DJI_0001')

    def registry(self, *records, exported='2026-11-05T05:00:00Z', plans=None):
        (self.inbox / 'coverage-history.json').write_text(json.dumps(export(
            exported, 'all', plans or [{'id': 'P1', 'name': 'Calles-45m-1104-1625'}], list(records))))
        reviewed = self.service.call('missions_review', {'archive': 'MEDIA'})
        self.service.call('missions_incorporate', {'archive': 'MEDIA', 'digest_value': reviewed['digest']})

    def record(self, when='2026-11-04 16:32:13', name='DJI_0001.JPG', cid='C1', **extra):
        value = capture(cid, '2026-11-04T23:32:13Z', **dict({'imageJPEGReference': name, 'latitude': LAT, 'longitude': LON}, **extra))
        value['mediaCreatedAt'] = when
        return value

    def unit(self, plan):
        return next(u for u in plan['units'] if not u.get('support'))

    def apply(self, plan):
        return self.service.call('apply', {'plan': plan, 'digest': plan['digest']})

    def units_map(self):
        path = self.work / '001_proyectos/atlas-subaereo/_nuun/units.ndjson'
        return [json.loads(line) for line in path.read_text().splitlines()]


class MissionMatchTests(MissionFixture):
    def test_matched_photo_takes_the_mission_project_folder_and_only_its_jpg_goes_to_work(self):
        self.registry(self.record(when='2026-11-04 16:30:00', name='DJI_0000.JPG', cid='C0'), self.record())
        plan = self.plan()
        unit = self.unit(plan)
        self.assertEqual(unit['project'], 'subaereo')
        self.assertEqual({k: unit['mission'][k] for k in ('state', 'planID', 'sessionID', 'captureID')},
                         {'state': 'matched', 'planID': 'P1', 'sessionID': 'S1', 'captureID': 'C1'})
        session = '2026/11/26NOV04-163000_CALLES-45M-1104-1625/'
        jpg, dng = plan['layout']['DCIM/101MEDIA/DJI_0001.JPG'], plan['layout']['DCIM/101MEDIA/DJI_0001.DNG']
        self.assertEqual(jpg, {'archive': 'dron/misiones/' + session + NAME.format('SUBAEREO', 'JPG'),
                               'work': 'atlas-subaereo/misiones/' + session + NAME.format('SUBAEREO', 'JPG')})
        self.assertEqual(dng, {'archive': 'dron/misiones/' + session + NAME.format('SUBAEREO', 'DNG'), 'work': None})
        self.assertEqual(plan['missions']['subaereo']['units'], {'matched': 1, 'none': 0, 'pending_export': 0})
        result = self.apply(plan)
        self.assertEqual(result['units_map'], 'written')
        self.assertTrue((self.work / '001_proyectos/atlas-subaereo/misiones' / session / NAME.format('SUBAEREO', 'JPG')).is_file())
        self.assertFalse((self.work / '001_proyectos/atlas-subaereo/misiones' / session / NAME.format('SUBAEREO', 'DNG')).exists())
        [line] = self.units_map()
        self.assertEqual(line['mission'], {'planID': 'P1', 'sessionID': 'S1', 'captureID': 'C1'})
        self.assertEqual((line['original_name'], line['dcim_folder'], line['serial']), ('DJI_0001.JPG', '101MEDIA', '1TCLH9N03BJ5N9'))
        self.assertEqual(line['work']['jpg'], 'misiones/' + session + NAME.format('SUBAEREO', 'JPG'))
        self.assertIsNone(line['work']['dng'])
        self.assertEqual(line['archive']['dng'], '001_raw/dron/misiones/' + session + NAME.format('SUBAEREO', 'DNG'))
        self.assertEqual(len(line['sha256']['dng']), 64)
        self.assertEqual(line['card_uuid'], str(self.source))

    def test_far_or_unnamed_records_are_not_a_mission_inside_the_covered_period(self):
        self.registry(self.record(latitude=LAT + 0.001))   # ~110 m away
        plan = self.plan()
        unit = self.unit(plan)
        self.assertEqual((unit['mission']['state'], unit['project']), ('none', 'intro'))
        self.assertEqual(plan['layout']['DCIM/101MEDIA/DJI_0001.JPG']['work'], 'e64_raw/intro/' + NAME.format('INTRO', 'JPG'))
        self.assertIsNone(plan['layout']['DCIM/101MEDIA/DJI_0001.DNG']['work'])

    def test_photo_newer_than_the_registry_waits_for_the_export_with_work_pending(self):
        self.registry(exported='2026-11-01T00:00:00Z')
        plan = self.plan()
        mission = self.unit(plan)['mission']
        self.assertEqual((mission['state'], mission['likely_mission']), ('pending_export', True))
        self.assertTrue(all(v['work'] is None for k, v in plan['layout'].items() if k.startswith('DCIM/101MEDIA/')))
        self.assertEqual(plan['missions']['subaereo']['likely'], 1)
        self.apply(plan)
        asset = next(a for a in self.catalog.search() if a.get('mission'))
        self.assertEqual(asset['work_delivery_status'], 'pending')
        [line] = self.units_map()
        self.assertEqual((line['mission'], line['mission_state']), (None, 'pending_export'))

    def test_without_any_registry_drone_stills_wait_for_an_export(self):
        self.assertEqual(self.unit(self.plan())['mission']['state'], 'pending_export')

    def test_an_explicit_project_wins_over_the_mission_default(self):
        self.registry(self.record())
        plan = self.plan(assignments={p: {'project': 'otra'} for p in ('DCIM/101MEDIA/DJI_0001.JPG', 'DCIM/101MEDIA/DJI_0001.DNG')})
        unit = self.unit(plan)
        self.assertEqual((unit['mission']['state'], unit['project']), ('matched', 'otra'))

    def test_older_exports_match_on_command_time_within_the_wider_window(self):
        old = capture('C5', '2026-11-04T23:32:17Z', imageJPEGReference='DJI_0001.jpg', actualLatitude=LAT, actualLongitude=LON,
                      capturedAtLocal='2026-11-04T16:32:17.100-07:00', flightRunID='RUN')
        del old['mediaCreatedAt'], old['sessionID']
        self.registry(old)
        mission = self.unit(self.plan())['mission']
        self.assertEqual((mission['state'], mission['captureID'], mission['sessionID']), ('matched', 'C5', 'RUN'))

    def test_a_still_already_in_the_work_root_is_not_copied_again(self):
        from tests.test_devices import jpeg
        self.registry(self.record())
        old = self.work / '001_proyectos/atlas-subaereo/102MEDIA'
        old.mkdir(parents=True)
        (old / 'DJI_0001.JPG').write_bytes(jpeg())
        plan = self.plan()
        unit = self.unit(plan)
        self.assertEqual(unit['work_existing'], {'DCIM/101MEDIA/DJI_0001.JPG': '001_proyectos/atlas-subaereo/102MEDIA/DJI_0001.JPG'})
        self.assertIsNone(plan['layout']['DCIM/101MEDIA/DJI_0001.JPG']['work'])
        self.assertEqual(plan['missions']['subaereo']['work_existing'], 1)
        self.apply(plan)
        [line] = self.units_map()
        self.assertEqual(line['work']['jpg'], '102MEDIA/DJI_0001.JPG')
        self.assertFalse((self.work / '001_proyectos/atlas-subaereo/misiones').exists())

    def test_same_name_with_another_capture_is_still_copied(self):
        from tests.test_devices import jpeg
        self.registry(self.record())
        old = self.work / '001_proyectos/atlas-subaereo/101MEDIA'
        old.mkdir(parents=True)
        (old / 'DJI_0001.JPG').write_bytes(jpeg(when='2026:05:30 10:00:00'))
        plan = self.plan()
        self.assertNotIn('work_existing', self.unit(plan))
        self.assertIsNotNone(plan['layout']['DCIM/101MEDIA/DJI_0001.JPG']['work'])

    def test_a_discarded_still_is_not_backed_up_and_leaves_the_card_with_cleanup(self):
        self.pair('DJI_0340', when='2026:11:04 16:40:00')
        bad = {p: {'discard': True} for p in ('DCIM/101MEDIA/DJI_0340.JPG', 'DCIM/101MEDIA/DJI_0340.DNG')}
        plan = self.plan(assignments=bad)
        self.assertEqual([d['paths'] for d in plan['discarded']], [['DCIM/101MEDIA/DJI_0340.DNG', 'DCIM/101MEDIA/DJI_0340.JPG']])
        self.assertNotIn('DCIM/101MEDIA/DJI_0340.JPG', plan['layout'])
        result = self.apply(plan)
        self.assertEqual(result['state'], 'completed')
        self.assertFalse((self.folder / 'DJI_0340.JPG').exists() or (self.folder / 'DJI_0340.DNG').exists())
        self.assertFalse((self.folder / 'DJI_0001.JPG').exists())
        self.assertFalse(any('DJI_0340' in str(p) for p in self.archive.rglob('*')))
        removed = [e for e in self.catalog.db.execute("SELECT payload FROM events WHERE job_id=? AND kind='discard_removed'", (plan['id'],))]
        self.assertEqual(len(removed), 2)

    def test_backing_up_the_consumers_own_folder_keeps_it_as_the_work_copy(self):
        from tests.test_devices import jpeg, dng
        self.registry(self.record())
        folder = self.work / '001_proyectos/atlas-subaereo/101MEDIA'
        folder.mkdir(parents=True)
        (folder / 'DJI_0001.JPG').write_bytes(jpeg())
        (folder / 'DJI_0001.DNG').write_bytes(dng())
        plan = self.plan(source=str(folder), work=None, cleanup=False, archive_only=True, confirm_archive_only=True, work_decision='skipped')
        unit = self.unit(plan)
        self.assertEqual(unit['work_existing'], {'DJI_0001.JPG': '001_proyectos/atlas-subaereo/101MEDIA/DJI_0001.JPG'})
        self.assertEqual(plan['source_work_id'], self.w['id'])
        result = self.apply(plan)
        self.assertEqual((result['state'], result['units_map']), ('source_retained', 'written'))
        self.assertTrue((folder / 'DJI_0001.JPG').exists() and (folder / 'DJI_0001.DNG').exists())
        self.assertTrue((self.archive / '001_raw/dron/misiones/2026/11/26NOV04-163213_CALLES-45M-1104-1625' / NAME.format('SUBAEREO', 'DNG')).is_file())
        [line] = self.units_map()
        self.assertEqual((line['work']['jpg'], line['dcim_folder']), ('101MEDIA/DJI_0001.JPG', '101MEDIA'))

    def test_a_photo_already_backed_up_from_its_card_is_not_backed_up_twice(self):
        from tests.test_devices import jpeg, dng
        self.registry(self.record())
        self.apply(self.plan(cleanup=False))
        folder = self.work / '001_proyectos/atlas-subaereo/101MEDIA'
        folder.mkdir(parents=True)
        (folder / 'DJI_0001.JPG').write_bytes(jpeg())
        (folder / 'DJI_0001.DNG').write_bytes(dng())
        (folder / 'DJI_0002.JPG').write_bytes(jpeg(when='2026:05:30 10:00:00'))
        (folder / 'DJI_0002.DNG').write_bytes(dng(when='2026:05:30 10:00:00'))
        plan = self.plan(source=str(folder), work=None, cleanup=False, archive_only=True, confirm_archive_only=True, work_decision='skipped')
        self.assertEqual(plan['already_archived'], ['DJI_0001.DNG', 'DJI_0001.JPG'])
        self.assertEqual((plan['missions']['subaereo']['units']['matched'] + plan['missions']['subaereo']['units']['none'],
                          plan['missions']['subaereo']['already_archived']), (1, 1))
        self.assertEqual([u['paths'] for u in plan['units'] if not u.get('support')], [['DJI_0002.DNG', 'DJI_0002.JPG']])
        result = self.apply(plan)
        self.assertEqual(result['state'], 'source_retained')
        self.assertEqual(len(self.units_map()), 2)

    def test_card_support_files_go_under_the_drone_family(self):
        (self.source / 'MISC/GIS').mkdir(parents=True)
        (self.source / 'MISC/GIS/dji.gis').write_bytes(b'geofence')
        plan = self.plan()
        target = plan['layout']['MISC/GIS/dji.gis']['archive']
        self.assertTrue(target.startswith('metadata/dji/support/2026/11/'), target)
        self.assertTrue(target.endswith('_MAVICAIR2/MISC/GIS/dji.gis'), target)

    def test_without_cleanup_a_discarded_still_stays_on_the_card(self):
        self.pair('DJI_0340', when='2026:11:04 16:40:00')
        bad = {p: {'discard': True} for p in ('DCIM/101MEDIA/DJI_0340.JPG', 'DCIM/101MEDIA/DJI_0340.DNG')}
        plan = self.plan(assignments=bad, cleanup=False)
        self.apply(plan)
        self.assertTrue((self.folder / 'DJI_0340.JPG').exists())

    def test_a_discarded_still_changed_after_review_stops_the_cleanup(self):
        self.pair('DJI_0340', when='2026:11:04 16:40:00')
        bad = {p: {'discard': True} for p in ('DCIM/101MEDIA/DJI_0340.JPG', 'DCIM/101MEDIA/DJI_0340.DNG')}
        plan = self.plan(assignments=bad)
        (self.folder / 'DJI_0340.JPG').write_bytes(b'replaced after review')
        with self.assertRaises(Exception):
            self.apply(plan)
        self.assertTrue((self.folder / 'DJI_0340.JPG').exists())

    def test_an_unregistered_survey_is_a_hint_not_a_mission(self):
        self.registry(exported='2026-11-06T00:00:00Z')
        plan = self.plan()
        mission = self.unit(plan)['mission']
        self.assertEqual((mission['state'], mission['likely_mission']), ('none', True))
        self.assertEqual((plan['missions']['subaereo']['unregistered_likely'], plan['missions']['subaereo']['likely']), (1, 0))
        self.assertEqual(self.unit(plan)['project'], 'intro')

    def test_discover_tells_the_app_which_device_needs_no_question(self):
        self.registry(self.record())
        units = self.service.call('discover', {'source': str(self.source), 'archive': 'MEDIA'})['units']
        unit = next(u for u in units if u['paths'][0].startswith('DCIM/101MEDIA/'))
        self.assertEqual((unit['device']['category'], unit['device']['requires_confirmation'], unit['device']['source']), ('dji-drone', False, 'MAVICAIR2'))
        self.assertEqual((unit['mission']['state'], unit['mission']['project']), ('matched', 'subaereo'))
        offline = self.service.call('discover', {'source': str(self.source)})['units']
        self.assertNotIn('mission', next(u for u in offline if u['paths'][0].startswith('DCIM/101MEDIA/')))

    def test_card_hint_names_the_drone_not_an_empty_residue_folder(self):
        from media.volumes import card_hint
        hint = card_hint(self.source, self.catalog)
        self.assertEqual((hint['family'], hint['recommendation'], hint['device']['label']), ('dji', 'drone_missions', 'DJI Mavic Air 2'))


if __name__ == '__main__':
    unittest.main()
