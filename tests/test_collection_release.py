"""M1: free a living collection folder against byte-identical copies already on registered archives.

Fixtures only: temporary folders and fixture volume identities, never personal disks.
"""
import gzip
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from media import collection_release as cr
from media.catalog import Catalog
from media.common import MediaError
from media.service import Service
from tests.test_media_engine import FixtureVolumes

NO_CLIPPER = {'schema': 'clipper-library-deps/1', 'capabilities': [], 'projects': [], 'offline_projects': [],
              'integration': 'never_configured'}


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


def ableton_set(path, *references):
    body = ''.join('<FileRef><Path Value="{}"/></FileRef>'.format(r) for r in references)
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, 'wb') as stream:
        stream.write(('<?xml version="1.0"?><Ableton>' + body + '</Ableton>').encode())


class CollectionReleaseTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.ssd, self.old, self.new = [self.root / n for n in ('ssd1', 'media500', 'media502')]
        for folder in (self.ssd, self.old, self.new): folder.mkdir()
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)
        self.backend = FixtureVolumes()
        self.m500 = self.catalog.register('MEDIA500', 'archive', self.old, self.backend.describe(self.old), '001_raw')
        self.m502 = self.catalog.register('MEDIA502', 'archive', self.new, self.backend.describe(self.new), '001_raw')
        # Never the person's Clipper registry or Downloads: an isolated home and projects root.
        for patcher in (mock.patch('media.clipper.dependencies', return_value=NO_CLIPPER),
                        mock.patch('pathlib.Path.home', return_value=self.root / 'home'),
                        mock.patch.dict(os.environ, {'CLIPPER_PROJECTS_ROOT': str(self.root / 'home/clipper-projects')})):
            patcher.start()
            self.addCleanup(patcher.stop)
        rec = self.ssd / 'recordingslong'
        # 2019: backed up under another name on MEDIA500 (historical rename) — released.
        write(rec / '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV', b'tokyo' * 1000)
        write(self.old / 'audio/2019/tokyo-renamed.wav', b'tokyo' * 1000)
        # 2019: used by an Ableton set — kept although it has a copy.
        write(rec / '2019/11/19NOV4-035845_H6LR_04-14_JP-KYOTOSAKA[].WAV', b'kyoto' * 1000)
        write(self.old / 'audio/2019/kyoto.wav', b'kyoto' * 1000)
        # 2020: has an .asd next to it — kept.
        write(rec / '2020/02/20FEB6-120737_H6LR_00-26_PE-PISAQ[].WAV', b'pisaq' * 1000)
        write(rec / '2020/02/20FEB6-120737_H6LR_00-26_PE-PISAQ[].WAV.asd', b'analysis')
        write(self.new / '001_raw/recordingslong/pisaq.wav', b'pisaq' * 1000)
        # 2021: same size on the archive but different bytes — no copy, stays.
        write(rec / '2021/09/21SEP19-105748_H6LR_00-31_MX-QROMIX[].WAV', b'qro-a' * 1000)
        write(self.old / 'audio/2021/qro.wav', b'qro-b' * 1000)
        # 2022: no file of that size anywhere — never hashed, stays.
        write(rec / '2022/01/22JAN01-000000_H6LR_00-10_MX-X[].WAV', b'only-here' * 7)
        # 2023: covered on the second archive.
        write(rec / '2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV', b'shink' * 1000)
        write(self.new / '001_raw/recordingslong/2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV', b'shink' * 1000)
        self.sets = self.root / 'ableton'
        ableton_set(self.sets / 'erikiado Project/erikiado.als',
                    '/Volumes/SSD1/recordingslong/#2019:11:19NOV4-035845_H6LR_04-14_JP-KYOTOSAKA[].WAV')
        self.rec = rec

    def make(self, **extra):
        keep = extra.pop('keep', {'als_roots': [str(self.sets)]})
        return cr.plan(self.catalog, self.backend, str(self.rec), ['MEDIA500', 'MEDIA502'], keep=keep, **extra)

    def test_plan_classifies_without_moving_anything(self):
        before = sorted(p for p in self.root.rglob('*') if p.is_file() and 'state' not in p.parts)
        value = self.make()
        self.assertEqual(sorted(e['path'] for e in value['entries']), [
            '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV', '2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV'])
        self.assertEqual({e['archive_path'] for e in value['entries']},
                         {'audio/2019/tokyo-renamed.wav', '001_raw/recordingslong/2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV'})
        self.assertEqual({k['path'].split('/')[0]: k['reason'] for k in value['kept']}, {'2019': 'ableton_set', '2020': 'ableton_analysis'})
        self.assertEqual({u['path'].split('/')[0]: u['reason'] for u in value['uncovered']},
                         {'2021': 'no_identical_copy', '2022': 'no_same_size'})
        self.assertNotIn('sha256', next(u for u in value['uncovered'] if u['reason'] == 'no_same_size'))
        self.assertIn('2020/02/20FEB6-120737_H6LR_00-26_PE-PISAQ[].WAV.asd', value['preserved'])
        self.assertEqual(value['keep_rules']['ableton_sets'], 1)
        self.assertEqual(sorted(p for p in self.root.rglob('*') if p.is_file() and 'state' not in p.parts), before)
        years = {g['folder']: g for g in value['summary']}
        self.assertEqual(years['2019']['release'][0] + years['2019']['keep'][0], 2)

    def test_apply_moves_only_reviewed_files_to_the_volume_trash(self):
        value = self.make()
        result = cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['state'], 'completed')
        trash = Path(result['trash'])
        self.assertTrue((trash / '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV').exists())
        self.assertFalse((self.rec / '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV').exists())
        for kept in ('2019/11/19NOV4-035845_H6LR_04-14_JP-KYOTOSAKA[].WAV', '2020/02/20FEB6-120737_H6LR_00-26_PE-PISAQ[].WAV',
                     '2021/09/21SEP19-105748_H6LR_00-31_MX-QROMIX[].WAV', '2022/01/22JAN01-000000_H6LR_00-10_MX-X[].WAV'):
            self.assertTrue((self.rec / kept).exists(), kept)
        self.assertTrue((self.old / 'audio/2019/tokyo-renamed.wav').exists())
        events = [r[0] for r in self.catalog.db.execute('SELECT kind FROM events WHERE job_id=?', (value['id'],))]
        self.assertEqual(events.count('collection_released'), 2)
        self.assertEqual(cr.apply(self.catalog, self.backend, value['id'], value['digest'])['state'], 'completed')

    def test_archive_copy_changed_after_review_stops_before_moving(self):
        value = self.make()
        write(self.old / 'audio/2019/tokyo-renamed.wav', b'TOKYO' * 1000)
        with self.assertRaises(MediaError) as error:
            cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(error.exception.code, 'checksum_mismatch')
        self.assertTrue((self.rec / '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV').exists())

    def test_source_changed_after_review_is_not_moved(self):
        value = self.make()
        target = self.rec / '2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV'
        target.write_bytes(b'SHINK' * 1000)
        with self.assertRaises(MediaError) as error:
            cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(error.exception.code, 'source_changed')
        self.assertTrue(target.exists())

    def test_wrong_digest_and_offline_archive_refuse(self):
        value = self.make()
        with self.assertRaises(MediaError) as error:
            cr.apply(self.catalog, self.backend, value['id'], 'x' * 64)
        self.assertEqual(error.exception.code, 'plan_changed')
        self.backend.offline.add(str(self.old))
        with self.assertRaises(MediaError):
            cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertTrue((self.rec / '2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV').exists())

    def test_interrupted_after_rename_resumes_without_moving_twice(self):
        value = self.make()
        calls = []

        def crash(stage, entry):
            if stage == 'after_move' and not calls:
                calls.append(entry['path'])
                raise RuntimeError('power cut')
        with self.assertRaises(RuntimeError):
            cr.apply(self.catalog, self.backend, value['id'], value['digest'], hook=crash)
        stored = self.catalog.get(cr.KEY + value['id'])
        self.assertEqual(stored['moved'][calls[0]], 'done')
        # A crash between the rename and the journal update leaves `intent`; resume must accept the Trash copy.
        other = next(e['path'] for e in value['entries'] if e['path'] != calls[0])
        entry = next(e for e in value['entries'] if e['path'] == other)
        stored['moved'][other] = 'intent'
        self.catalog.set(cr.KEY + value['id'], stored)
        cr._move(*self._volume_move_args(value, entry))
        result = cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(result['state'], 'completed')
        self.assertEqual(set(result['moved'].values()), {'done'})

    def _volume_move_args(self, value, entry):
        from media.common import Tree
        tree = Tree(value['volume_root']).__enter__()
        self.addCleanup(tree.__exit__, None, None, None)
        return tree, entry['path'], cr._trash_folder(value) + '/' + entry['path'], entry['stamp']

    def test_new_recording_after_review_does_not_block_and_is_not_touched(self):
        value = self.make()
        fresh = write(self.rec / '2026/11/26NOV09-101010_H6LR_00-10_MX-NEW[].WAV', b'new' * 10)
        cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertTrue(fresh.exists())

    def test_clipper_dependency_keeps_the_file(self):
        path = str(self.rec / '2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV')
        deps = dict(NO_CLIPPER, integration='configured', projects=[{
            'clipper_root': '/p', 'relinks': [], 'assets': [{'asset_id': 'a1', 'path': path, 'items': [], 'in_use': True,
                                                          'queued_exports': [], 'revision': 1, 'kind': 'audio'}]}])
        with mock.patch('media.clipper.dependencies', return_value=deps), mock.patch('media.clipper.queue_ids', return_value=set()):
            value = self.make()
        self.assertIn('2023/05/23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV',
                      [k['path'] for k in value['kept'] if k['reason'] == 'clipper'])

    def test_source_overlapping_an_archive_or_same_disk_is_refused(self):
        with self.assertRaises(MediaError) as error:
            cr.plan(self.catalog, self.backend, str(self.old / 'audio'), ['MEDIA500'])
        self.assertEqual(error.exception.code, 'overlapping_roots')
        self.backend.physical[str(self.rec)] = 'disk-x'
        self.backend.physical[str(self.old)] = 'disk-x'
        with self.assertRaises(MediaError) as error:
            cr.plan(self.catalog, self.backend, str(self.rec), ['MEDIA500'])
        self.assertEqual(error.exception.code, 'same_physical_disk')

    def test_search_folder_limits_where_copies_are_looked_for(self):
        value = self.make(search=[{'archive': 'MEDIA500', 'path': 'audio/2021'}])
        self.assertNotIn('2019/10/19OCT31-200854_H6LR_03-10_JP-TOKYO[].WAV', [e['path'] for e in value['entries']])

    def test_service_and_cli_shape(self):
        service = Service(self.catalog, self.backend)
        value = service.call('collection_release_plan', {'source': str(self.rec), 'archives': ['MEDIA500', 'MEDIA502'],
                                                         'keep': {'als_roots': [str(self.sets)]}})
        listed = service.call('collection_release_plans', {})
        self.assertEqual([p['id'] for p in listed], [value['id']])
        self.assertNotIn('entries', listed[0])
        with self.assertRaises(MediaError):
            service.call('collection_release_plan', {'source': str(self.rec), 'archives': ['MEDIA500'], 'progress': None})
        from media.cli import parser
        args = parser().parse_args(['collection-release-plan', '--source', 's', '--archive', 'MEDIA500', '--archive', 'MEDIA501',
                                    '--search', 'MEDIA500:001_raw/recordingslong', '--als-root', 'a', '--keep', '2024'])
        self.assertEqual(args.archive, ['MEDIA500', 'MEDIA501'])

    def test_offline_clipper_project_blocks_apply_but_not_the_plan(self):
        value = self.make()
        with mock.patch('media.clipper.inspect', return_value={'blockers': ['Proyecto Clipper no disponible: /x'], 'relinks': []}):
            with self.assertRaises(MediaError) as error:
                cr.apply(self.catalog, self.backend, value['id'], value['digest'])
        self.assertEqual(error.exception.code, 'dependencies_blocked')
        self.assertTrue(all((self.rec / e['path']).exists() for e in value['entries']))
        self.assertEqual(cr.apply(self.catalog, self.backend, value['id'], value['digest'])['state'], 'completed')


if __name__ == '__main__':
    unittest.main()
