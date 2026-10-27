import importlib.util
from pathlib import Path
import plistlib
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('media_installer', Path(__file__).resolve().parents[1] / 'scripts/install_media_app.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class AppInstallTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.app = self.root / 'build/New.app'
        (self.app / 'Contents').mkdir(parents=True)
        (self.app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier': 'app.nu9ve.media', 'Nu9veBuildID': 'new'}))
        self.destination = self.root / 'Applications/Nu9ve Media.app'

    def test_upgrade_keeps_previous_bundle_outside_launcher_directory(self):
        (self.destination / 'Contents').mkdir(parents=True)
        (self.destination / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier': 'app.nu9ve.media', 'Nu9veBuildID': 'old'}))
        with mock.patch.object(installer.subprocess, 'run', return_value=SimpleNamespace(stdout='')):
            installer.install(self.app, self.destination)
        self.assertEqual(plistlib.loads((self.destination / 'Contents/Info.plist').read_bytes())['Nu9veBuildID'], 'new')
        backups = list((self.root / 'Library/Application Support/n-creator-utils/app-backups').glob('*.app'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(plistlib.loads((backups[0] / 'Contents/Info.plist').read_bytes())['Nu9veBuildID'], 'old')
        self.assertEqual(list(self.destination.parent.glob('.Nu9veMedia*')), [])

    def test_running_app_is_never_replaced(self):
        with mock.patch.object(installer.subprocess, 'run', return_value=SimpleNamespace(stdout='/Users/a/Applications/Nu9ve Media.app/Contents/MacOS/MediaMenu\n')):
            with self.assertRaisesRegex(RuntimeError, 'Close Media'):
                installer.install(self.app, self.destination)
        self.assertFalse(self.destination.exists())

    def test_failed_swap_restores_previous_bundle(self):
        (self.destination / 'Contents').mkdir(parents=True)
        (self.destination / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier': 'app.nu9ve.media', 'Nu9veBuildID': 'old'}))
        rename = Path.rename
        def fail_staged(path, target):
            if path.name.startswith('.Nu9veMedia-install-'):
                raise OSError('simulated install failure')
            return rename(path, target)
        with mock.patch.object(installer.subprocess, 'run', return_value=SimpleNamespace(stdout='')), mock.patch.object(Path, 'rename', fail_staged):
            with self.assertRaisesRegex(OSError, 'simulated'):
                installer.install(self.app, self.destination)
        self.assertEqual(plistlib.loads((self.destination / 'Contents/Info.plist').read_bytes())['Nu9veBuildID'], 'old')
        self.assertEqual(list(self.destination.parent.glob('.Nu9veMedia*')), [])


if __name__ == '__main__': unittest.main()
