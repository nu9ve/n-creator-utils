import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from manager import file as media

REPO = Path(__file__).resolve().parents[1]


class CommandTests(unittest.TestCase):
    def test_help_and_doctor_work_without_site_packages(self):
        for args in [['--help'], ['clip', '--help'], ['save', '--help'], ['doctor']]:
            result = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), *args], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            if args == ['doctor']:
                self.assertIn('executables', json.loads(result.stdout))

    def test_invalid_usage_and_missing_files_have_nonzero_exit(self):
        for args in [['save'], ['download', 'wrong', 'https://example.com'], ['clip', '/missing.mp4'], ['save', '/missing-source', 'mx', 'hmo', 'AUTO'], ['control']]:
            result = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), *args], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn('Traceback', result.stderr)

    def test_youtube_url_and_spaces_are_preserved_and_destination_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            downloaded = Path(tmp) / 'video with spaces.mp3'
            downloaded.write_bytes(b'audio')
            result = subprocess.CompletedProcess([], 0, str(downloaded) + '\n', '')
            url = 'https://youtube.com/watch?v=abc&list=xyz'
            with patch.object(media.subprocess, 'run', return_value=result) as run:
                self.assertEqual(media.download_file(url, 'youtube', 'song'), str(downloaded))
                self.assertEqual(run.call_args.args[0][-1], url)
                self.assertIn('after_move:filepath', run.call_args.args[0])
            with patch.object(media, 'AUDIO_SONGS_DIR', str(Path(tmp) / 'songs')):
                destination = media.move_to_file_type_dir('song', str(downloaded))
            self.assertEqual(Path(destination).read_bytes(), b'audio')
            self.assertEqual(Path(destination).name, downloaded.name)

    def test_service_detection_uses_host_and_unsupported_services_fail(self):
        self.assertEqual(media.get_source_service('https://youtube.com.evil.example/watch'), '')
        self.assertEqual(media.get_source_service('https://example.com/youtube.com'), '')
        self.assertEqual(media.get_source_service('https://www.youtube.com/watch?v=a'), 'youtube')
        with self.assertRaises(ValueError):
            media.download_file('https://spotify.com/track', 'spotify', 'song')

    def test_download_failure_does_not_echo_tokens(self):
        failure = subprocess.CompletedProcess([], 1, '', 'secret-token')
        with patch.object(media.subprocess, 'run', return_value=failure):
            with self.assertRaises(RuntimeError) as error:
                media.download_file('https://soundcloud.com/a', 'soundcloud', 'song', auth_token='secret-token')
        self.assertNotIn('secret-token', str(error.exception))

    def test_split_success_archives_original_and_failure_keeps_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'my_audio.wav'
            source.write_bytes(b'audio')
            cwd = Path.cwd()
            with patch.object(media, 'AUDIO_SPLITS_DIR', str(root / 'splits')), patch.object(media, 'split_file') as split:
                split.side_effect = RuntimeError('demucs failed')
                with self.assertRaises(RuntimeError):
                    media.splitter([str(source)])
                normalized = root / 'myaudio.wav'
                self.assertTrue(normalized.exists())
                self.assertFalse((root / 'splits/splitted').exists())
                self.assertEqual(Path.cwd(), cwd)
                split.side_effect = None
                archived = Path(media.splitter([str(normalized)]))
                self.assertEqual(archived.read_bytes(), b'audio')
                self.assertFalse(normalized.exists())
                self.assertEqual(Path.cwd(), cwd)
