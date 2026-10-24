import json
import tempfile
import unittest
import wave
from pathlib import Path
from unittest.mock import patch

from manager import file as media
from manager.audio import get_audio_file_description


class ImportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'card'
        self.archive = self.root / 'archive'
        self.backup = self.root / 'backup'
        for path in (self.source, self.archive, self.backup):
            path.mkdir()
        self.patcher = patch.multiple(media, ROOT_DIR=str(self.archive), ROOT_RECORDINGS_DIR=str(self.backup),
            FINAL_IMAGES_DIR=str(self.archive / 'images'), FINAL_RECORDINGS_DIR=str(self.archive / 'recordings'),
            SSD_RECORDINGS_DIR=str(self.backup / 'recordings'), FINAL_INSTA360_DIR=str(self.archive / 'insta360'),
            FINAL_VIDEOS_DIR=str(self.archive / 'videos'), FINAL_METADATA_DIR=str(self.archive / 'metadata'))
        self.patcher.start()
        self.addCleanup(self.patcher.stop)

    def test_photo_move_preserves_bytes_mtime_and_avoids_collisions(self):
        first = self.source / 'a.jpg'
        first.write_bytes(b'first photo')
        import os
        os.utime(first, (1600000000, 1600000000))
        second = self.source / 'b.jpg'
        second.write_bytes(b'second photo')
        os.utime(second, (1600000000, 1600000000))
        media.save_tag_files(str(self.source), 'PHONE', 'mx', 'hermosillo')
        saved = sorted((self.archive / 'images').rglob('*.JPG'))
        self.assertEqual(len(saved), 2)
        self.assertEqual({p.read_bytes() for p in saved}, {b'first photo', b'second photo'})
        self.assertTrue(any('-02.JPG' in p.name for p in saved))
        self.assertEqual(saved[0].stat().st_mtime, 1600000000)
        self.assertEqual(list(self.source.iterdir()), [])

    def test_missing_archive_root_preserves_source(self):
        image = self.source / 'a.jpg'
        image.write_bytes(b'photo')
        with patch.object(media, 'ROOT_DIR', str(self.root / 'unmounted')):
            with self.assertRaises(ValueError):
                media.save_tag_files(str(self.source), 'PHONE', 'mx', 'hmo')
        self.assertTrue(image.is_file())
        self.assertFalse((self.root / 'unmounted').exists())

    def test_source_archive_overlap_is_rejected(self):
        with self.assertRaises(ValueError):
            media.save_tag_files(str(self.root), 'PHONE', 'mx', 'hmo')

    def test_copy_failure_preserves_source(self):
        source, destination = self.source / 'a.jpg', self.archive / 'a.jpg'
        source.write_bytes(b'original')
        with patch.object(media.shutil, 'copy2', side_effect=OSError('disk full')):
            with self.assertRaises(OSError):
                media.move_preserving_metadata(str(source), str(destination))
        self.assertEqual(source.read_bytes(), b'original')

    def test_truncated_copy_preserves_source(self):
        source, destination = self.source / 'a.jpg', self.archive / 'a.jpg'
        source.write_bytes(b'original')
        def truncated(src, dst):
            Path(dst).write_bytes(b'x')
        with patch.object(media.shutil, 'copy2', side_effect=truncated):
            with self.assertRaises(OSError):
                media.move_preserving_metadata(str(source), str(destination))
        self.assertTrue(source.is_file())

    def test_insta360_bundle_keeps_names_and_existing_manifest(self):
        dcim = self.source / 'DCIM'
        dcim.mkdir()
        (dcim / 'VID_001.insv').write_bytes(b'360 original')
        (dcim / 'VID_001.lrv').write_bytes(b'proxy')
        (self.source / 'manifest.json').write_text('original manifest')
        media.save_tag_files(str(self.source), 'INSTA360', 'mx', 'hmo')
        saved = next((self.archive / 'insta360').rglob('VID_001.insv'))
        bundle = saved.parent.parent
        self.assertEqual(saved.read_bytes(), b'360 original')
        self.assertEqual((bundle / 'manifest.json').read_text(), 'original manifest')
        manifest = json.loads((bundle / 'manifest-02.json').read_text())
        self.assertEqual(len(manifest['files']), 3)
        self.assertFalse((dcim / 'VID_001.insv').exists())

    def test_sony_media_sidecar_and_support_are_moved(self):
        clips = self.source / 'PRIVATE/M4ROOT/CLIP'
        clips.mkdir(parents=True)
        (clips / 'C0001.MP4').write_bytes(b'video bytes')
        (clips / 'C0001M01.XML').write_text('<NonRealTimeMeta><CreationDate value="2026-06-14T12:00:00+00:00"/></NonRealTimeMeta>')
        support = self.source / 'PRIVATE/M4ROOT/MEDIAPRO.XML'
        support.write_text('<support/>')
        with patch.object(media, 'get_video_file_duration', return_value=(5, '00-05')), patch.object(media, 'is_4k', return_value=False):
            media.save_tag_files(str(self.source), 'SONYA6400', 'mx', 'hmo')
        video = next((self.archive / 'videos/2026/06').glob('*.MP4'))
        self.assertIn('26JUN14-120000', video.name)
        self.assertTrue((self.archive / 'metadata/sony/2026/06' / (video.stem + '.XML')).is_file())
        self.assertEqual(len(list((self.archive / 'metadata/sony/support').rglob('MEDIAPRO.XML'))), 1)
        self.assertFalse((clips / 'C0001.MP4').exists())
        self.assertFalse(support.exists())
        self.assertTrue(clips.is_dir())

    def test_lowercase_wav_generic_name_and_audio_backups_never_overwrite(self):
        path = self.source / 'recording.wav'
        with wave.open(str(path), 'wb') as audio:
            audio.setnchannels(1)
            audio.setsampwidth(2)
            audio.setframerate(8000)
            audio.writeframes(b'\x00\x00' * 8000)
        description = get_audio_file_description(str(path), 'RECORDER')
        self.assertEqual(description['duration'], 1)
        self.assertEqual(description['mic_name'], '')
        # A collision in only the secondary backup must preserve that old backup.
        old_backup = self.backup / 'recordings/keep.wav'
        old_backup.parent.mkdir(parents=True)
        old_backup.write_bytes(b'previous backup')
        media.copy_preserving_metadata(str(path), str(old_backup))
        self.assertEqual(old_backup.read_bytes(), b'previous backup')
        self.assertTrue(old_backup.with_name('keep-02.wav').exists())
        media.save_tag_files(str(self.source), 'RECORDER', 'mx', 'hmo')
        self.assertFalse(path.exists())
        self.assertEqual(len(list((self.archive / 'recordings').rglob('*.WAV'))), 1)
