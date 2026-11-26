"""Perfiles de la suite (nu9ve-profile-v1) y acumulados en disco por el motor de Rust, con raíces aisladas.

Nunca el archivo de perfiles de la persona ni su carpeta: `--state-dir`, `NU9VE_PROFILES_FILE` y un `HOME` de
fixture. Los escritores son el motor (Rust, `suite_profiles_register`) y el ayudante del core (Python), a la vez.
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'vendor' / 'nu9ve-core' / 'scripts'))
import nu9ve_profiles  # noqa: E402

from tests.test_engine_examples import rust_engine  # noqa: E402
from media.catalog import Catalog  # noqa: E402

ENGINE = rust_engine()


@unittest.skipUnless(ENGINE, 'build the engine first: cargo build in engine/')
class SuiteStorageTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(os.path.realpath(self.tmp.name))
        self.home = self.root / 'nuun'
        catalog = Catalog(self.home)
        with catalog.writer():
            catalog.set('creative-profiles', {
                'e64': {'id': 'e64', 'name': 'erikiado64', 'raw_dir': 'e64_raw'},
                'nu9ve-xyz': {'id': 'nu9ve-xyz', 'name': 'nu9ve.xyz', 'raw_dir': 'nu9ve-xyz'}})
            catalog.set('defaults', {'profile': 'e64'})
        catalog.close()
        self.profiles = self.root / 'qa' / 'nu9ve' / 'profiles.json'
        self.user = self.root / 'user'
        self.user.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def env(self):
        env = dict(os.environ, NU9VE_PROFILES_FILE=str(self.profiles), HOME=str(self.user),
                   NU9VE_DEVICE_FILE=str(self.root / 'qa' / 'device.json'),
                   NU9VE_ENGINES_DIR=str(self.root / 'qa' / 'engines'))
        env.pop('NU9VE_MEDIA_HOME', None)
        return env

    def rpc(self, method, params=None):
        done = subprocess.run([ENGINE, '--state-dir', str(self.home), 'rpc'], capture_output=True, env=self.env(),
                              input=json.dumps({'id': 1, 'method': method, 'params': params or {}}).encode(), check=False)
        return json.loads(done.stdout)

    def test_no_file_then_register_creates_it(self):
        read = self.rpc('suite_profiles')['data']
        self.assertEqual(read['file']['state'], 'missing')
        self.assertFalse(self.profiles.exists())
        refused = self.rpc('suite_profiles_register')
        self.assertEqual(refused['error']['code'], 'confirmation_required')
        self.assertFalse(self.profiles.exists())
        done = self.rpc('suite_profiles_register', {'confirm': True})['data']
        self.assertEqual({p['id'] for p in done['registered']}, {'e64', 'nu9ve-xyz'})
        doc = nu9ve_profiles.read(str(self.profiles))
        self.assertIsNone(nu9ve_profiles.document_problem(doc))
        self.assertEqual(oct(self.profiles.stat().st_mode & 0o777), oct(0o600))
        self.assertEqual(oct(self.profiles.parent.stat().st_mode & 0o777), oct(0o700))
        self.assertEqual([p.profile_id if hasattr(p, 'profile_id') else p['profile_id']
                          for p in nu9ve_profiles.for_app(doc, 'nuun')][0], 'e64')

    def test_file_from_nove_with_e64_only_gains_nuun(self):
        nu9ve_profiles.change(str(self.profiles), {'op': 'create_profile', 'profile_id': 'e64',
                                                   'display_name': 'Erik', 'kind': 'person'})
        nu9ve_profiles.change(str(self.profiles), {'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nove',
                                                   'library': '/Users/ana/nove/engine', 'default': True, 'ui': 'full'})
        nove = nu9ve_profiles.read(str(self.profiles))['profiles'][0]['apps']['nove']
        self.rpc('suite_profiles_register', {'confirm': True, 'profiles': ['e64']})
        doc = nu9ve_profiles.read(str(self.profiles))
        e64 = doc['profiles'][0]
        self.assertEqual(e64['display_name'], 'Erik')
        self.assertEqual(e64['apps']['nove'], nove)
        self.assertEqual(e64['apps']['nuun']['library'], str(self.home))
        self.assertEqual(e64['aliases'], ['erikiado64'])
        read = self.rpc('suite_profiles')['data']
        self.assertEqual([(a['app'], a['own']) for a in read['profiles'][0]['apps']], [('nove', False), ('nuun', True)])

    def test_two_writers_rust_and_python_at_once(self):
        nu9ve_profiles.change(str(self.profiles), {'op': 'create_profile', 'profile_id': 'e64',
                                                   'display_name': 'Erik', 'kind': 'person'})
        errors = []

        def python_writer():
            try:
                for index in range(20):
                    nu9ve_profiles.change(str(self.profiles), {
                        'op': 'upsert_app_entry', 'profile_id': 'e64', 'app': 'nove',
                        'library': '/Users/ana/nove/%d' % index, 'default': True})
            except Exception as error:  # noqa: BLE001
                errors.append(error)

        thread = threading.Thread(target=python_writer)
        thread.start()
        for _ in range(5):
            reply = self.rpc('suite_profiles_register', {'confirm': True})
            self.assertTrue(reply['ok'], reply)
        thread.join()
        self.assertEqual(errors, [])
        doc = nu9ve_profiles.read(str(self.profiles))
        self.assertIsNone(nu9ve_profiles.document_problem(doc))
        e64 = next(p for p in doc['profiles'] if p['profile_id'] == 'e64')
        self.assertEqual(e64['apps']['nove']['library'], '/Users/ana/nove/19')
        self.assertIn('nuun', e64['apps'])
        self.assertEqual(len(doc['profiles']), 2)

    def test_an_invalid_file_is_never_overwritten(self):
        self.profiles.parent.mkdir(parents=True)
        self.profiles.write_text('{"contract": "otro"}')
        self.profiles.chmod(0o600)
        read = self.rpc('suite_profiles')['data']
        self.assertEqual(read['file']['state'], 'invalid')
        reply = self.rpc('suite_profiles_register', {'confirm': True})
        self.assertEqual(reply['error']['code'], 'profiles_invalid')
        self.assertEqual(self.profiles.read_text(), '{"contract": "otro"}')

    def test_accumulations_scan_read_and_settings(self):
        code = self.user / 'code' / 'app'
        (code / 'target' / 'release').mkdir(parents=True)
        (code / 'Cargo.toml').write_text('[package]\n')
        (code / 'target' / 'CACHEDIR.TAG').write_text('Signature\n')
        (code / 'target' / 'release' / 'bin').write_bytes(b'x' * (3 << 20))
        self.assertEqual(self.rpc('storage_accumulations')['data']['state'], 'none')
        scan = self.rpc('storage_accumulations_scan')['data']
        self.assertEqual(scan['state'], 'ok')
        self.assertEqual([r['rule'] for r in scan['findings']], ['rust-target'])
        self.assertEqual(scan['plan']['groups'][0]['action'], 'trash')
        self.assertTrue((code / 'target' / 'release' / 'bin').exists())
        saved = self.rpc('storage_accumulations_settings', {'internal_min_free_bytes': 1024})['data']
        self.assertEqual(saved['internal_min_free_bytes'], 1024)
        again = self.rpc('storage_accumulations')['data']
        self.assertEqual(again['scan_id'], scan['scan_id'])
        self.assertEqual(again['volumes'][0]['min_free_bytes'], 1024)


if __name__ == '__main__':
    unittest.main()
