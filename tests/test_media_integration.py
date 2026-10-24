"""Local generated media only; no SDs, downloads, accounts or cloud calls."""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which('ffmpeg') and shutil.which('ffprobe'), 'FFmpeg/ffprobe required')
class MediaIntegrationTests(unittest.TestCase):
    def test_clip_two_ranges_from_uppercase_source_and_preview(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            video = root / 'source video.MP4'
            generation = subprocess.run([
                'ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'color=c=blue:s=320x180:r=25',
                '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=44100',
                '-t', '4', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', str(video),
            ], capture_output=True, text=True)
            self.assertEqual(generation.returncode, 0, generation.stderr)
            original = video.read_bytes()
            config = root / 'custom.json'
            config.write_text(json.dumps({'clips': [{'start': 0, 'end': 2}, {'start': 1, 'end': 3}]}))
            # A default config must never shadow the explicitly selected config.
            (root / 'config.json').write_text('{"clips": []}')
            environment = dict(os.environ, NU9VE_DEBUG='false')
            preview = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'clip',
                str(video), str(config), '--preview', '--verbose'], capture_output=True, text=True, env=environment)
            self.assertEqual(preview.returncode, 0, preview.stderr[-5000:])
            self.assertEqual(len(list(root.glob('*_vertical*.jpg'))), 2)
            self.assertEqual(video.read_bytes(), original)
            for cut in (root / 'source video_clip_0.mp4', root / 'source video_clip_1.mp4'):
                metadata = subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration',
                    '-of', 'json', str(cut)], capture_output=True, text=True, check=True)
                self.assertGreater(float(json.loads(metadata.stdout)['format']['duration']), 0)

    def test_minimal_config_with_two_texts_renders(self):
        filters = subprocess.run(['ffmpeg', '-hide_banner', '-filters'], capture_output=True, text=True)
        if 'drawtext' not in filters.stdout:
            self.skipTest('This FFmpeg build has no drawtext filter')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            video = root / 'source.mp4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'color=c=blue:s=320x180:r=25',
                '-t', '3', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', str(video)],
                capture_output=True, check=True)
            (root / 'config.json').write_text(json.dumps({'texts': [{'text': 'uno'}, {'text': 'dos', 'y': 80}],
                'clips': [{'title': 'Prueba', 'start': 0, 'end': 2}]}))
            result = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'clip', str(video), '--preview'],
                capture_output=True, text=True, env=dict(os.environ, NU9VE_DEBUG='false'))
            self.assertEqual(result.returncode, 0, result.stderr[-5000:])
            self.assertEqual(len(list(root.glob('*.jpg'))), 1)

    def test_full_render_retains_all_requested_formats_for_both_orientations(self):
        for dimensions in ('320x180', '180x320'):
            with self.subTest(dimensions=dimensions), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                video = root / 'source.mp4'
                subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                    'color=c=blue:s={}:r=25'.format(dimensions), '-t', '3', '-c:v', 'libx264',
                    '-pix_fmt', 'yuv420p', str(video)], capture_output=True, check=True)
                (root / 'config.json').write_text(json.dumps({'vertical_only': False,
                    'clips': [{'start': 0, 'end': 2}]}))
                result = subprocess.run([sys.executable, '-S', str(REPO / 'nu9ve.py'), 'clip', str(video)],
                    capture_output=True, text=True, env=dict(os.environ, NU9VE_DEBUG='false'))
                self.assertEqual(result.returncode, 0, result.stderr[-5000:])
                for suffix in ('vertical', 'horizontal', 'square'):
                    self.assertEqual(len(list(root.glob('source_clip_0_{}*.mp4'.format(suffix)))), 1)
                self.assertTrue(video.is_file())
