"""Generated media only; never touches a card or a configured archive."""
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from manager import file as media


@unittest.skipUnless(shutil.which('ffmpeg') and shutil.which('ffprobe'), 'requires FFmpeg/FFprobe')
class MediaIntegrationTests(unittest.TestCase):
    def test_real_video_metadata_and_legacy_import(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'card'
            source.mkdir()
            archive = root / 'archive'
            archive.mkdir()
            video = source / 'source with spaces.MP4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                            'color=c=blue:s=64x64:r=10:d=1', '-c:v', 'mpeg4', str(video)], check=True)
            original = video.read_bytes()
            with patch.multiple(media, ROOT_DIR=str(archive),
                                ROOT_RECORDINGS_DIR=str(root / 'secondary'),
                                FINAL_VIDEOS_DIR=str(archive / 'videos')):
                media.save_tag_files(str(source), 'TEST', 'mx', 'hmo')
            saved = list(archive.rglob('*.MP4'))
            self.assertEqual(len(saved), 1)
            self.assertEqual(saved[0].read_bytes(), original)
            self.assertFalse(video.exists())
