"""Mission or not for drone stills, work JPG only and units.ndjson (Atlas A12), on temporary fixtures only."""
import json
import unittest

from media.service import Service
from tests.test_devices import DroneFixture
from tests.test_missions import capture, export

NAME = '26NOV04-163212_MAVICAIR2_MX-{}[].{}'
LAT, LON = 29.0984139, -110.9622728   # the fixture EXIF position


class MissionMatchTests(DroneFixture):
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

    def test_discover_tells_the_app_which_device_needs_no_question(self):
        units = self.service.call('discover', {'source': str(self.source)})['units']
        device = next(u for u in units if u['paths'][0].startswith('DCIM/101MEDIA/'))['device']
        self.assertEqual((device['category'], device['requires_confirmation'], device['source']), ('dji-drone', False, 'MAVICAIR2'))


if __name__ == '__main__':
    unittest.main()
