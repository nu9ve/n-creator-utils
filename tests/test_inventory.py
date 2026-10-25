import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from manager.inventory import inspect_source
from manager import file as media


class InventoryTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def session(self, status='completed'):
        bundle = self.root / 'capture.stalker'
        bundle.mkdir()
        (bundle / 'manifest.json').write_text(json.dumps({
            'producer': 'stalker', 'schemaVersion': 3, 'sessionID': 'example', 'status': status}))
        (bundle / 'screen.mov').write_bytes(b'screen')
        (bundle / 'sources').mkdir()
        (bundle / 'sources/mic.mov').write_bytes(b'mic')
        return bundle

    def test_bundle_is_one_unit_and_inspection_preserves_files(self):
        bundle = self.session()
        before = {p.relative_to(self.root): (p.read_bytes(), p.stat().st_mtime_ns)
                  for p in self.root.rglob('*') if p.is_file()}
        report = inspect_source(self.root)
        self.assertEqual(report['unit_count'], 1)
        self.assertEqual(report['units'][0]['kind'], 'stalker_session')
        self.assertEqual(len(report['units'][0]['files']), 3)
        self.assertEqual(report['observed_bytes'], sum(len(data[0]) for data in before.values()))
        self.assertFalse(report['transfer_plan'])
        after = {p.relative_to(self.root): (p.read_bytes(), p.stat().st_mtime_ns)
                 for p in self.root.rglob('*') if p.is_file()}
        self.assertEqual(before, after)
        self.assertTrue(bundle.is_dir())

    def test_recording_is_not_scanned(self):
        bundle = self.session('recording')
        report = inspect_source(bundle)['units'][0]
        self.assertIn('session_not_completed', report['issues'])
        self.assertIsNone(report['bytes'])
        self.assertEqual(report['files'], [])

    def test_malformed_manifest_and_external_symlink_are_reported(self):
        bundle = self.session()
        (bundle / 'manifest.json').write_text('[')
        self.assertIn('invalid_manifest', inspect_source(bundle)['units'][0]['issues'])
        (self.root / 'external').symlink_to('/etc', target_is_directory=True)
        report = inspect_source(self.root)
        link = next(unit for unit in report['units'] if unit['kind'] == 'symlink')
        self.assertEqual(link['issues'], ['not_followed'])

    def test_generic_save_rejects_nested_package_before_moving_anything(self):
        self.session()
        photo = self.root / 'a.jpg'
        photo.write_bytes(b'keep')
        with patch.object(media, 'move_preserving_metadata') as move:
            with self.assertRaisesRegex(ValueError, 'package'):
                media.save_tag_files(str(self.root), 'PHONE', 'mx', 'hmo')
        move.assert_not_called()
        self.assertEqual(photo.read_bytes(), b'keep')

    def test_generic_save_rejects_inside_package_and_clipper_project(self):
        bundle = self.session()
        with self.assertRaisesRegex(ValueError, 'package'):
            media.save_tag_files(str(bundle / 'sources'), 'PHONE', 'mx', 'hmo')
        project = self.root / 'project'
        project.mkdir()
        (project / 'clipper-project.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'package'):
            media.save_tag_files(str(project), 'PHONE', 'mx', 'hmo')

    def test_sony_residual_directory_does_not_classify_dji_as_sony(self):
        (self.root / 'AVF_INFO').mkdir()
        (self.root / 'DJI_001.MP4').write_bytes(b'dji')
        (self.root / 'other.insv').write_bytes(b'insta')
        units = {unit['relative_path']: unit for unit in inspect_source(self.root)['units']}
        self.assertEqual(units['DJI_001.MP4']['camera_evidence'], [
            {'source': 'filename', 'family': 'dji', 'confidence': 'low', 'requires_review': True}])
        self.assertEqual(units['other.insv']['camera_evidence'][0]['family'], 'insta360')

    def test_sony_model_comes_from_file_sidecar(self):
        (self.root / 'C0001.MP4').write_bytes(b'video')
        (self.root / 'C0001M01.XML').write_text(
            '<Meta xmlns="urn:sony"><Device manufacturer="Sony" modelName="ILCE-6400"/></Meta>')
        unit = next(u for u in inspect_source(self.root)['units'] if u['kind'] == 'video')
        self.assertEqual(unit['camera_evidence'][0]['model'], 'ILCE-6400')

    def test_generic_save_rejects_links_before_deletion_rules(self):
        target = self.root / 'original.bmp'
        target.write_bytes(b'keep')
        card = self.root / 'card'
        card.mkdir()
        (card / 'linked.bmp').symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'links'):
            media.save_tag_files(str(card), 'PHONE', 'mx', 'hmo')
        self.assertEqual(target.read_bytes(), b'keep')
        alias = self.root / 'card-alias'
        alias.symlink_to(card, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'links'):
            media.save_tag_files(str(alias), 'PHONE', 'mx', 'hmo')
