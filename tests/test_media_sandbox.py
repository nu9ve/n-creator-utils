"""The test environment (scripts/sandbox.py) is the only place a disk image counts as a card or a USB disk."""
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from media.volumes import SANDBOX_MARKER, sandbox_image, sandbox_root


def hdiutil(image, mount):
    return {'images': [{'image-path': str(image), 'system-entities': [{'content-hint': 'x'}, {'mount-point': mount}]}]}


class Sandbox(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve() / 'sandbox'
        (self.root / 'images').mkdir(parents=True)
        (self.root / SANDBOX_MARKER).write_text('nuun test environment\n')

    def tearDown(self):
        self.temp.cleanup()

    def env(self, **values):
        return mock.patch.dict(os.environ, values, clear=False)

    def test_all_three_conditions_are_needed(self):
        state = str(self.root / 'state')
        with mock.patch.dict(os.environ, {}, clear=False):
            os.environ.pop('NU9VE_SANDBOX', None)
            os.environ.pop('NU9VE_MEDIA_HOME', None)
            self.assertIsNone(sandbox_root())
        with self.env(NU9VE_SANDBOX=str(self.root), NU9VE_MEDIA_HOME=str(Path(self.temp.name) / 'real-catalog')):
            self.assertIsNone(sandbox_root(), 'a catalog outside the environment never enables it')
        with self.env(NU9VE_SANDBOX=str(self.root), NU9VE_MEDIA_HOME=state):
            self.assertEqual(sandbox_root(), self.root)
            (self.root / SANDBOX_MARKER).unlink()
            self.assertIsNone(sandbox_root(), 'without the marker it is just a folder')

    def test_only_images_inside_the_environment_count(self):
        inside, outside = self.root / 'images/SD-SONY.sparseimage', Path(self.temp.name) / 'other.sparseimage'
        with self.env(NU9VE_SANDBOX=str(self.root), NU9VE_MEDIA_HOME=str(self.root / 'state')):
            self.assertEqual(sandbox_image('/Volumes/SD-SONY', hdiutil(inside, '/Volumes/SD-SONY')), inside)
            self.assertIsNone(sandbox_image('/Volumes/SD-SONY', hdiutil(outside, '/Volumes/SD-SONY')))
            self.assertIsNone(sandbox_image('/Volumes/OTHER', hdiutil(inside, '/Volumes/SD-SONY')))
        os.environ.pop('NU9VE_SANDBOX', None)
        self.assertIsNone(sandbox_image('/Volumes/SD-SONY', hdiutil(inside, '/Volumes/SD-SONY')))


if __name__ == '__main__':
    unittest.main()
