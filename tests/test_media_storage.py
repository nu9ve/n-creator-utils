import io
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest import mock

from media import storage_index
from media.catalog import Catalog
from media.cli import serve_reads
from media.common import MediaError, canonical
from media.service import Service
from tests.test_media_engine import FixtureVolumes


class FixedDisks(FixtureVolumes):
    """Folders that are not cards: `removable` only for the ones a test says so."""

    def __init__(self):
        super().__init__()
        self.cards = set()

    def describe(self, root, external_required=False):
        info = super().describe(root, external_required)
        info['removable'] = str(Path(root).absolute()) in self.cards
        return info


def write(root, rel, data=b'x', mtime=None):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    if mtime:
        os.utime(path, (mtime, mtime))
    return path


class StorageFixture(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.archive, self.work = self.root / 'archive', self.root / 'work'
        self.archive.mkdir(); self.work.mkdir()
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)
        self.backend = FixedDisks()
        self.a = self.catalog.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        self.w = self.catalog.register('ESSD', 'work', self.work, self.backend.describe(self.work), '001_proyectos')
        self.service = Service(self.catalog, self.backend)

    def call(self, method, **params):
        return self.service.call(method, params)

    def target(self, name):
        return next(t for t in self.call('storage_overview')['targets'] if t['name'] == name)

    def db(self):
        return storage_index.open_index(self.catalog)

    def hashed_files(self):
        db = self.db()
        try:
            return db.execute("SELECT COUNT(*) FROM hashes WHERE origin='computed'").fetchone()[0]
        finally:
            db.close()


class ScanTests(StorageFixture):
    def test_scan_rolls_up_sizes_and_kinds_like_a_disk_tool(self):
        write(self.work, 'projects/a/clip.MP4', b'v' * 8192)
        write(self.work, 'projects/a/photo.ARW', b'r' * 4096)
        write(self.work, 'projects/song.wav', b'a' * 4096)
        write(self.work, 'notes.txt', b'n')
        result = self.call('storage_scan', target='ESSD')
        self.assertEqual((result['state'], result['files']), ('finished', 4))
        tree = self.call('storage_tree', target='ESSD')
        self.assertEqual([f['name'] for f in tree['folders']], ['projects'])
        self.assertEqual(tree['files_total'], 4)
        self.assertEqual(sorted(tree['kinds']), ['audio', 'other', 'raw_image', 'video'])
        projects = self.call('storage_tree', target='ESSD', path='projects')
        self.assertEqual(projects['folders'][0]['path'], 'projects/a')
        self.assertEqual(projects['files'][0]['name'], 'song.wav')

    def test_scan_opens_no_file_and_reports_denied_folders(self):
        secret = write(self.work, 'keep/locked.mov', b'v' * 100)
        secret.chmod(0)
        denied = write(self.work, 'denied/inner.mov', b'v').parent
        denied.chmod(0)
        self.addCleanup(denied.chmod, 0o700)
        self.addCleanup(secret.chmod, 0o600)
        result = self.call('storage_scan', target='ESSD')
        self.assertEqual(result['state'], 'finished')
        self.assertEqual(result['files'], 1)  # the unreadable file is sized from metadata alone
        self.assertEqual(result['unreadable'], 1)
        self.assertIn('denied', self.target('ESSD')['samples'])

    def test_links_are_not_followed_and_hard_links_count_once(self):
        big = write(self.work, 'real.mov', b'v' * 4096)
        outside = write(self.root, 'outside/huge.mov', b'o' * 65536)
        (self.work / 'link-file').symlink_to(outside)
        (self.work / 'link-dir').symlink_to(outside.parent)
        os.link(big, self.work / 'same.mov')
        result = self.call('storage_scan', target='ESSD')
        self.assertEqual(result['files'], 1)
        self.assertEqual(result['bytes'], 4096)

    def test_packages_and_derived_folders_stay_closed(self):
        write(self.work, 'edit.fcpbundle/inner/media.mov', b'm' * 100)
        write(self.work, 'session.stalker/screen.mov', b's' * 100)
        write(self.work, 'clipperproj/clipper-project.json', b'{}')
        write(self.work, 'clipperproj/renders/out.mov', b'r' * 100)
        write(self.work, 'app/node_modules/a/b.js', b'j' * 100)
        self.call('storage_scan', target='ESSD')
        tree = self.call('storage_tree', target='ESSD')
        kinds = {f['name']: (f['kind'], f['opaque']) for f in tree['folders']}
        self.assertEqual(kinds['edit.fcpbundle'], ('editable', True))
        self.assertEqual(kinds['session.stalker'], ('capture', True))
        self.assertEqual(kinds['clipperproj'], ('editable', True))
        self.assertEqual(self.call('storage_tree', target='ESSD', path='edit.fcpbundle')['folders'], [])
        with self.assertRaises(MediaError) as caught:
            self.call('storage_tree', target='ESSD', path='edit.fcpbundle/inner')
        self.assertEqual(caught.exception.code, 'unknown_path')
        app = self.call('storage_tree', target='ESSD', path='app')
        self.assertEqual(app['folders'][0]['kind'], 'derived')
        self.assertGreater(app['safety']['derived'], 0)

    def test_interrupted_scan_keeps_the_previous_index(self):
        write(self.work, 'one.mov', b'v' * 4096)
        self.call('storage_scan', target='ESSD')
        write(self.work, 'two.mov', b'v' * 4096)
        cancelled = Service(self.catalog, self.backend, cancel=lambda: True)
        self.assertEqual(cancelled.call('storage_scan', {'target': 'ESSD'})['state'], 'cancelled')
        self.assertEqual(self.target('ESSD')['files'], 1)
        self.assertEqual(self.target('ESSD')['last_run']['state'], 'cancelled')
        db = self.db()
        try:
            self.assertEqual(db.execute('SELECT COUNT(*) FROM entries').fetchone()[0], 1)
        finally:
            db.close()

    def test_disk_lost_during_scan_never_publishes_a_smaller_disk(self):
        write(self.work, 'a/one.mov', b'v' * 4096)
        self.call('storage_scan', target='ESSD')
        original = storage_index.os.scandir

        def failing(path):
            if str(path).endswith('/a'):
                self.backend.offline.add(str(self.work))
                raise OSError(5, 'Input/output error')
            return original(path)

        with mock.patch.object(storage_index.os, 'scandir', failing):
            with self.assertRaises(MediaError):
                self.call('storage_scan', target='ESSD')
        self.backend.offline.clear()
        self.assertEqual(self.target('ESSD')['files'], 1)
        self.assertEqual(self.target('ESSD')['last_run']['state'], 'failed')

    def test_offline_disk_keeps_its_last_index_and_cannot_scan(self):
        write(self.work, 'one.mov', b'v' * 4096)
        self.call('storage_scan', target='ESSD')
        self.backend.offline.add(str(self.work))
        target = self.target('ESSD')
        self.assertFalse(target['online'])
        self.assertTrue(target['indexed'])
        self.assertIsNotNone(target['indexed_at'])
        self.assertEqual(self.call('storage_tree', target='ESSD')['files'][0]['name'], 'one.mov')
        with self.assertRaises(MediaError) as caught:
            self.call('storage_scan', target='ESSD')
        self.assertEqual(caught.exception.code, 'volume_offline')

    def test_one_operation_at_a_time(self):
        with storage_index._exclusive(self.catalog):
            with self.assertRaises(MediaError) as caught:
                self.call('storage_scan', target='ESSD')
        self.assertEqual(caught.exception.code, 'storage_busy')

    def test_damaged_index_is_rebuilt_without_touching_the_catalog(self):
        write(self.work, 'one.mov', b'v')
        self.call('storage_scan', target='ESSD')
        path = self.catalog.home / 'storage-index.sqlite3'
        for suffix in ('-wal', '-shm'):
            Path(str(path) + suffix).unlink(missing_ok=True)
        path.write_bytes(b'not a database at all' * 100)
        self.assertFalse(self.target('ESSD')['indexed'])
        self.assertEqual(self.call('storage_scan', target='ESSD')['files'], 1)
        self.assertEqual(self.catalog.volume('ESSD')['id'], self.w['id'])

    def test_catalog_snapshot_stays_free_of_the_index(self):
        write(self.work, 'one.mov', b'v')
        self.call('storage_scan', target='ESSD')
        tables = {r[0] for r in self.catalog.db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        self.assertFalse(tables & {'entries', 'folders', 'hashes', 'safety'})

    def test_scan_stops_before_the_index_fills_the_mac(self):
        write(self.work, 'one.mov', b'v')
        self.call('storage_scan', target='ESSD')
        write(self.work, 'two.mov', b'v')
        usage = mock.Mock(free=10, total=100, used=90)
        with mock.patch.object(storage_index.shutil, 'disk_usage', return_value=usage):
            with self.assertRaises(MediaError) as caught:
                self.call('storage_scan', target='ESSD')
        self.assertEqual(caught.exception.code, 'low_space')
        self.assertEqual(self.target('ESSD')['files'], 1)

    def test_replaced_rows_give_their_space_back(self):
        for index in range(3000):
            write(self.work, 'many/folder{}/{}.mov'.format(index % 50, 'x' * 150 + str(index)), b'v')
        self.call('storage_scan', target='ESSD')
        path = self.catalog.home / 'storage-index.sqlite3'
        full = path.stat().st_size
        for child in (self.work / 'many').iterdir():
            for item in child.iterdir():
                item.unlink()
            child.rmdir()
        self.call('storage_scan', target='ESSD')
        self.assertLess(path.stat().st_size, full / 2)

    def test_undecodable_names_are_reported_and_still_counted(self):
        self.assertTrue(storage_index._storable('clip ñ.mov'))
        self.assertFalse(storage_index._storable('clip\udcff.mov'))
        self.assertEqual(storage_index._printable('clip\udcff.mov'), 'clip\\udcff.mov')


class SafetyTests(StorageFixture):
    def test_without_a_backup_index_nothing_is_called_only_here(self):
        write(self.work, 'one.mov', b'v' * 4096)
        self.call('storage_scan', target='ESSD')
        self.assertEqual(self.target('ESSD')['safety']['only_here'], 0)
        self.assertGreater(self.target('ESSD')['safety']['unknown'], 0)

    def test_sizes_alone_separate_only_here_from_probable_without_hashing(self):
        write(self.work, 'same-size.mov', b'a' * 4096)
        write(self.work, 'unique.mov', b'b' * 8192)
        write(self.archive, 'renamed.mov', b'c' * 4096)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        files = {f['name']: f['state'] for f in self.call('storage_tree', target='ESSD')['files']}
        self.assertEqual(files, {'same-size.mov': 'probable', 'unique.mov': 'only_here'})
        self.assertEqual(self.hashed_files(), 0)

    def test_compare_recognizes_renamed_copies_and_rejects_same_size_impostors(self):
        write(self.work, 'acomodar/video.mov', b'a' * 4096)
        write(self.work, 'acomodar/impostor.mov', b'b' * 8192)
        write(self.archive, 'renamed/26OCT-video.mov', b'a' * 4096)
        write(self.archive, 'other.mov', b'c' * 8192)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        result = self.call('storage_compare', target='ESSD', path='acomodar')
        self.assertEqual((result['state'], result['matched']), ('finished', 1))
        files = {f['name']: f for f in self.call('storage_tree', target='ESSD', path='acomodar')['files']}
        self.assertEqual(files['video.mov']['state'], 'verified')
        self.assertEqual(files['video.mov']['copy']['path'], 'renamed/26OCT-video.mov')
        self.assertEqual(files['video.mov']['copy']['volume'], 'MEDIA')
        self.assertEqual(files['impostor.mov']['state'], 'only_here')

    def test_hashes_are_reused_until_size_or_mtime_changes(self):
        path = write(self.work, 'video.mov', b'a' * 4096, mtime=1700000000)
        write(self.work, 'other.mov', b'o' * 8192, mtime=1700000000)
        write(self.archive, 'copy.mov', b'a' * 4096, mtime=1700000000)
        write(self.archive, 'different.mov', b'x' * 8192, mtime=1700000000)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        self.assertEqual(self.call('storage_compare', target='ESSD')['hashed'], 4)
        self.call('storage_scan', target='ESSD')
        self.call('storage_scan', target='MEDIA')
        again = self.call('storage_compare', target='ESSD')
        self.assertEqual(again['hashed'], 0)
        path.write_bytes(b'z' * 4096)
        os.utime(path, (1700000500, 1700000500))
        self.call('storage_scan', target='ESSD')
        files = {f['name']: f['state'] for f in self.call('storage_tree', target='ESSD')['files']}
        self.assertEqual(files['video.mov'], 'probable')
        self.assertEqual(self.call('storage_compare', target='ESSD')['hashed'], 1)
        files = {f['name']: f['state'] for f in self.call('storage_tree', target='ESSD')['files']}
        self.assertEqual(files['video.mov'], 'only_here')

    def test_interrupted_compare_keeps_finished_hashes(self):
        for index in range(3):
            write(self.work, 'f{}.mov'.format(index), bytes([index]) * (4096 + index))
            write(self.archive, 'c{}.mov'.format(index), bytes([index]) * (4096 + index))
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        answers = iter([False] * 8)
        service = Service(self.catalog, self.backend, cancel=lambda: next(answers, True))
        result = service.call('storage_compare', {'target': 'ESSD'})
        self.assertEqual(result['state'], 'cancelled')
        self.assertGreater(result['hashed'], 0)
        done = self.hashed_files()
        self.assertEqual(self.call('storage_compare', target='ESSD')['hashed'], 6 - done)

    def test_file_changed_after_scan_is_not_trusted(self):
        path = write(self.work, 'video.mov', b'a' * 4096, mtime=1700000000)
        write(self.archive, 'copy.mov', b'a' * 4096)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        os.utime(path, (1700000500, 1700000500))
        result = self.call('storage_compare', target='ESSD')
        self.assertEqual((result['matched'], result['skipped']), (0, 1))

    def test_backup_disconnected_during_compare_fails_visibly(self):
        write(self.work, 'video.mov', b'a' * 4096)
        write(self.archive, 'copy.mov', b'a' * 4096)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError) as caught:
            self.call('storage_compare', target='ESSD')
        self.assertEqual(caught.exception.code, 'archive_not_available')

    def test_same_physical_disk_never_counts_as_a_copy(self):
        write(self.work, 'video.mov', b'a' * 4096)
        write(self.archive, 'copy.mov', b'a' * 4096)
        self.backend.physical = {str(self.work): 'disk9', str(self.archive): 'disk9'}
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        self.assertEqual(self.call('storage_tree', target='ESSD')['files'][0]['state'], 'unknown')
        with self.assertRaises(MediaError) as caught:
            self.call('storage_compare', target='ESSD')
        self.assertEqual(caught.exception.code, 'archive_not_available')

    def test_receipts_answer_without_hashing_even_with_the_backup_offline(self):
        # Decomposed on disk (HFS+) and composed in the receipt: the same file.
        decomposed = 'can' + chr(0x303) + 'a/video.mov'
        write(self.work, decomposed, b'a' * 4096)
        sha = 'f' * 64
        with self.catalog.db:
            self.catalog.db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', ('job', self.w['id'], 'C0001.MP4', sha, canonical(
                {'destination': 'ca' + chr(0xf1) + 'a/video.mov', 'bytes': 4096, 'sha256': sha, 'verified_at': '2026-10-25T00:00:00Z'}).decode()))
            self.catalog.db.execute('INSERT INTO copies VALUES(?,?,?,?,?)', ('job', self.a['id'], 'C0001.MP4', sha, canonical(
                {'destination': '001_raw/videos/video.mov', 'bytes': 4096, 'sha256': sha}).decode()))
        self.backend.offline.add(str(self.archive))
        self.call('storage_scan', target='ESSD')
        files = self.call('storage_tree', target='ESSD', path=decomposed.split('/')[0])['files']
        self.assertEqual(files[0]['state'], 'verified')
        self.assertEqual(files[0]['copy']['origin'], 'receipt')
        self.assertEqual(self.hashed_files(), 0)

    def test_the_index_cannot_authorize_a_release(self):
        write(self.work, '001_proyectos/e64_raw/p/video.mov', b'a' * 4096)
        write(self.archive, 'copy.mov', b'a' * 4096)
        self.call('storage_scan', target='MEDIA')
        self.call('storage_scan', target='ESSD')
        self.call('storage_compare', target='ESSD')
        self.assertEqual(self.call('storage_tree', target='ESSD', path='001_proyectos/e64_raw/p')['files'][0]['state'], 'verified')
        # The backup changed after comparing: the release plan reads current bytes and refuses.
        write(self.archive, 'copy.mov', b'b' * 4096)
        with self.assertRaises(MediaError) as caught:
            self.call('plan_release', work='ESSD', archive='MEDIA', path='001_proyectos/e64_raw/p', project='p')
        self.assertNotEqual(caught.exception.code, 'unknown_method')


class ObservedFolderTests(StorageFixture):
    def setUp(self):
        super().setUp()
        self.home = self.root / 'home'
        self.home.mkdir()
        # Temporary folders live under /private on macOS; the refusal itself is checked below.
        roots = mock.patch.object(storage_index, 'SYSTEM_ROOTS', ('/System', '/Library'))
        roots.start()
        self.addCleanup(roots.stop)

    def test_chosen_folder_is_never_a_destination(self):
        added = self.call('storage_add_folder', root=str(self.home))
        self.assertEqual(added['role'], 'observed')
        self.assertEqual(len(self.call('volumes')), 2)
        self.assertEqual({v['role'] for v in self.call('snapshot', cards=False)['volumes']}, {'archive', 'work'})
        with self.assertRaises(MediaError):
            self.catalog.volume(added['id'])

    def test_small_files_stay_in_rollups_only(self):
        write(self.home, 'Music/big.wav', b'a' * (2 * 1024 * 1024))
        write(self.home, 'Music/small.wav', b'b' * 4096)
        write(self.home, 'Music/tiny/one.wav', b'c' * 4096)
        write(self.home, 'code/node_modules/x/huge.bin', b'n' * (2 * 1024 * 1024))
        limits = mock.patch.multiple(storage_index, OBSERVED_ENTRY_MIN_BYTES=1024 * 1024, OBSERVED_FOLDER_MIN_BYTES=1024 * 1024)
        limits.start()
        self.addCleanup(limits.stop)
        self.call('storage_add_folder', root=str(self.home), name='Inicio')
        self.call('storage_scan', target='Inicio')
        music = self.call('storage_tree', target='Inicio', path='Music')
        self.assertEqual([f['name'] for f in music['files']], ['big.wav'])
        self.assertEqual(music['files_total'], 3)
        self.assertEqual(music['folders'], [])  # too small to keep a row, still counted in its parent
        self.assertGreater(music['rest'], 0)
        self.assertEqual(self.call('storage_tree', target='Inicio', path='code')['safety']['derived'], self.call('storage_tree', target='Inicio', path='code')['allocated'])

    def test_overlaps_system_folders_and_cards_are_refused(self):
        for root in ('/', '/System/Library', '/Library', str(self.work), str(self.work / 'sub')):
            (self.work / 'sub').mkdir(exist_ok=True)
            with self.assertRaises(MediaError, msg=root):
                self.call('storage_add_folder', root=root)
        self.backend.cards.add(str(self.home))
        with self.assertRaises(MediaError) as caught:
            self.call('storage_add_folder', root=str(self.home))
        self.assertEqual(caught.exception.code, 'card_not_indexable')

    def test_forgetting_a_folder_leaves_its_files(self):
        path = write(self.home, 'a.wav', b'a' * 4096)
        added = self.call('storage_add_folder', root=str(self.home))
        self.call('storage_scan', target=added['id'])
        self.call('storage_remove_folder', target=added['id'])
        self.assertTrue(path.exists())
        self.assertNotIn(added['id'], [t['id'] for t in self.call('storage_overview')['targets']])
        with self.assertRaises(MediaError) as caught:
            self.call('storage_remove_folder', target='ESSD')
        self.assertEqual(caught.exception.code, 'invalid_role')

    def test_overview_shows_used_space_the_walk_did_not_reach(self):
        write(self.work, 'one.mov', b'v' * 4096)
        self.call('storage_scan', target='ESSD')
        target = self.target('ESSD')
        self.assertEqual(target['uncounted'], max(0, target['fs_used'] - target['allocated']))
        self.assertGreater(target['fs_total'], 0)


class BoundaryTests(StorageFixture):
    def test_reader_answers_reads_and_refuses_scans(self):
        write(self.work, 'one.mov', b'v')
        self.call('storage_scan', target='ESSD')
        lines = [{'id': 1, 'method': 'storage_overview'}, {'id': 2, 'method': 'storage_tree', 'params': {'target': 'ESSD'}},
                 {'id': 3, 'method': 'storage_scan', 'params': {'target': 'ESSD'}}, {'id': 4, 'method': 'storage_compare', 'params': {'target': 'ESSD'}}]
        output = io.StringIO()
        serve_reads(self.service, io.StringIO('\n'.join(json.dumps(line) for line in lines) + '\n'), output)
        answers = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertTrue(answers[0]['ok'] and answers[1]['ok'])
        self.assertEqual([a['error']['code'] for a in answers[2:]], ['read_only', 'read_only'])

    def test_broken_index_is_a_service_error_not_a_crash(self):
        with mock.patch.object(storage_index, 'overview', side_effect=sqlite3.OperationalError('locked')):
            with self.assertRaises(MediaError) as caught:
                self.call('storage_overview')
        self.assertEqual(caught.exception.code, 'storage_index_error')

    def test_paths_cannot_leave_the_root(self):
        write(self.work, 'one.mov', b'v')
        self.call('storage_scan', target='ESSD')
        for path in ('../state', '/etc', 'a/../..'):
            with self.assertRaises(MediaError):
                self.call('storage_tree', target='ESSD', path=path)
            with self.assertRaises(MediaError):
                self.call('storage_compare', target='ESSD', path=path)


if __name__ == '__main__':
    unittest.main()
