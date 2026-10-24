import json
import tempfile
import unittest
from pathlib import Path

from configuration import ConfigurationError, load_settings


class ConfigurationTests(unittest.TestCase):
    def test_defaults_preserve_archive_and_use_actual_home(self):
        settings = load_settings(environ={})
        self.assertEqual(settings['root_dir'], '/Volumes/MEDIA502/001_raw')
        self.assertEqual(settings['audio_hijack_dir'], str(Path.home() / 'Music/Audio Hijack'))

    def test_json_environment_precedence_and_relative_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'local.json'
            path.write_text(json.dumps({'root_dir': 'archive', 'sd_src_name': {'CARD': 'AUTO'}}))
            local = load_settings(path, environ={})
            self.assertEqual(local['root_dir'], str((Path(tmp) / 'archive').resolve()))
            self.assertEqual(local['sd_src_name'], {'CARD': 'AUTO'})
            env = load_settings(environ={'NU9VE_CONFIG': str(path), 'NU9VE_ROOT_DIR': tmp,
                                         'NU9VE_DEBUG': 'true', 'NU9VE_YT_DLP_COOKIES_BROWSER': ''})
            self.assertEqual(env['root_dir'], str(Path(tmp).resolve()))
            self.assertTrue(env['debug'])
            self.assertEqual(env['yt_dlp_cookies_browser'], '')
            self.assertFalse((Path(tmp) / 'archive').exists())

    def test_explicit_missing_configuration_is_error(self):
        with self.assertRaises(ConfigurationError):
            load_settings('/this-config-does-not-exist.json', environ={})

    def test_reject_invalid_types_unknown_keys_and_volume_traversal(self):
        invalid = [[], {'root_diir': 'x'}, {'root_dir': ''}, {'debug': 1},
                   {'sd_src_name': {'../CARD': 'AUTO'}}, {'sd_src_name': {'CARD': ''}}]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'local.json'
            for value in invalid:
                with self.subTest(value=value):
                    path.write_text(json.dumps(value))
                    with self.assertRaises(ConfigurationError):
                        load_settings(path, environ={})
