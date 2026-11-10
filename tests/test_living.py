"""M2: living collections — incremental verified backup without renames, cleanup or propagated deletions.

Fixtures only: temporary folders and fixture volume identities.
"""
import gzip
import json
import os
import tempfile
import unittest
from pathlib import Path

from media import living
from media.catalog import Catalog
from media.common import MediaError
from media.service import Service
from tests.test_media_engine import FixtureVolumes


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


def bump(path, data):
    """Rewrite with a later mtime even on coarse clocks."""
    st = path.stat()
    path.write_bytes(data)
    os.utime(path, ns=(st.st_atime_ns, st.st_mtime_ns + 2_000_000_000))


class LivingTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.src, self.media, self.lib = self.root / 'ssd1/abletonlive/projects', self.root / 'media502', self.root / 'ssd1/abletonlive/samples'
        self.media.mkdir(parents=True)
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)
        self.backend = FixtureVolumes()
        self.catalog.register('MEDIA502', 'archive', self.media, self.backend.describe(self.media), '001_raw')
        write(self.src / 'song Project/song.als', b'v1-set')
        write(self.src / 'song Project/Samples/Recorded/take1.wav', b'take' * 100)
        write(self.lib / 'kick.wav', b'kick')
        with gzip.open(self.src / 'song Project/song.als', 'wb') as stream:
            stream.write('<Ableton><Path Value="{}"/><Path Value="/nowhere/gone.wav"/><Path Value="{}"/></Ableton>'.format(
                self.lib / 'kick.wav', self.src / 'song Project/Samples/Recorded/take1.wav').encode())
        self.folder = '005_produccion/projects'

    def run_once(self, **extra):
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder, **extra)
        return value, living.apply(self.catalog, self.backend, value['id'], value['digest'])

    def archived(self, rel):
        return self.media / self.folder / rel

    def test_first_run_copies_everything_with_original_paths(self):
        value, result = self.run_once(name='Ableton projects')
        self.assertEqual(sorted(i['path'] for i in value['new']), ['song Project/Samples/Recorded/take1.wav', 'song Project/song.als'])
        self.assertEqual(result['result']['copied'], 2)
        self.assertEqual(self.archived('song Project/Samples/Recorded/take1.wav').read_bytes(), b'take' * 100)
        manifest = json.loads(self.archived('_nuun/manifest.json').read_text())
        self.assertEqual(set(manifest['files']), {'song Project/Samples/Recorded/take1.wav', 'song Project/song.als'})
        self.assertEqual(living.collections(self.catalog)[0]['files'], 2)
        self.assertTrue((self.src / 'song Project/song.als').exists())

    def test_second_run_copies_only_new_and_never_rereads_unchanged(self):
        self.run_once()
        write(self.src / 'song Project/Samples/Recorded/take2.wav', b'new take')
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual([i['path'] for i in value['new']], ['song Project/Samples/Recorded/take2.wav'])
        self.assertEqual(value['changed'], [])
        self.assertEqual(value['unchanged']['files'], 2)
        result = living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['result']['copied'], 1)

    def test_changed_file_keeps_the_previous_version_in_the_archive(self):
        self.run_once()
        bump(self.src / 'song Project/song.als', b'v2-set-longer')
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual([i['path'] for i in value['changed']], ['song Project/song.als'])
        result = living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['result']['versioned'], 1)
        self.assertEqual(self.archived('song Project/song.als').read_bytes(), b'v2-set-longer')
        aside = self.media / result['result']['versions'][0]['aside']
        self.assertTrue(str(aside).startswith(str(self.media / self.folder / '_nuun/versions')))
        self.assertNotEqual(aside.read_bytes(), b'v2-set-longer')

    def test_touched_but_identical_file_is_adopted_not_versioned(self):
        self.run_once()
        path = self.src / 'song Project/Samples/Recorded/take1.wav'
        bump(path, b'take' * 100)
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        result = living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual((result['result']['adopted'], result['result']['versioned']), (1, 0))
        self.assertFalse((self.media / self.folder / '_nuun/versions').exists())

    def test_deleted_at_source_stays_in_archive_and_is_reported(self):
        self.run_once()
        (self.src / 'song Project/Samples/Recorded/take1.wav').unlink()
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual(value['gone_from_source'], ['song Project/Samples/Recorded/take1.wav'])
        living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertTrue(self.archived('song Project/Samples/Recorded/take1.wav').exists())

    def test_file_changed_after_review_is_skipped_not_fatal(self):
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        bump(self.src / 'song Project/song.als', b'edited while reviewing')
        result = living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['result']['changed_since_review'], 1)
        self.assertFalse(self.archived('song Project/song.als').exists())
        again = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual([i['path'] for i in again['new']], ['song Project/song.als'])

    def test_interrupted_run_resumes_and_adopts_what_was_already_copied(self):
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)

        def stop(stage, item):
            if stage == 'after_copy':
                raise RuntimeError('power cut')
        with self.assertRaises(RuntimeError):
            living.apply(self.catalog, self.backend, value['id'], value['digest'], hook=stop)
        # The index row of the first copy may not have been flushed; resuming must not copy it twice or fail.
        result = living.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['state'], 'completed')
        self.assertEqual(len(living._index(self.catalog, value['collection_id'])), 2)

    def test_archive_file_missing_is_copied_again(self):
        self.run_once()
        self.archived('song Project/song.als').unlink()
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual([i['path'] for i in value['changed']], ['song Project/song.als'])

    def test_ableton_report_lists_external_samples_not_covered(self):
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        report = value['ableton']
        self.assertEqual(report['sets'], 1)
        self.assertEqual(report['missing'], 1)
        self.assertEqual([r['path'] for r in report['not_covered']], [str(self.lib / 'kick.wav')])
        # Once the sample library is its own collection, the same reference is covered.
        living.apply(self.catalog, self.backend, *self._plan_lib())
        self.assertEqual(living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)['ableton']['not_covered'], [])

    def _plan_lib(self):
        value = living.plan(self.catalog, self.backend, str(self.lib), 'MEDIA502', '005_produccion/samples')
        return value['id'], value['digest']

    def test_refusals(self):
        with self.assertRaises(MediaError) as error:
            living.plan(self.catalog, self.backend, str(self.media / 'x'), 'MEDIA502', 'f')
        self.assertEqual(error.exception.code, 'invalid_request')
        (self.media / 'inner').mkdir()
        with self.assertRaises(MediaError) as error:
            living.plan(self.catalog, self.backend, str(self.media / 'inner'), 'MEDIA502', 'f')
        self.assertEqual(error.exception.code, 'overlapping_roots')
        living.apply(self.catalog, self.backend, *self._plan_lib())
        with self.assertRaises(MediaError) as error:
            living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', '005_produccion/samples/sub')
        self.assertEqual(error.exception.code, 'overlapping_roots')
        with self.assertRaises(MediaError) as error:
            living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', '005_produccion/_nuun')
        self.assertEqual(error.exception.code, 'invalid_request')
        value = living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        with self.assertRaises(MediaError) as error:
            living.apply(self.catalog, self.backend, value['id'], 'x' * 64)
        self.assertEqual(error.exception.code, 'plan_changed')
        self.backend.physical[str(self.src)] = 'same'
        self.backend.physical[str(self.media)] = 'same'
        with self.assertRaises(MediaError) as error:
            living.plan(self.catalog, self.backend, str(self.src), 'MEDIA502', self.folder)
        self.assertEqual(error.exception.code, 'same_physical_disk')

    def test_service_shape(self):
        service = Service(self.catalog, self.backend)
        value = service.call('living_plan', {'source': str(self.src), 'archive': 'MEDIA502', 'folder': self.folder})
        service.call('living_apply', {'plan_id': value['id'], 'digest': value['digest']})
        self.assertEqual(service.call('living_collections', {})[0]['files'], 2)


if __name__ == '__main__':
    unittest.main()
