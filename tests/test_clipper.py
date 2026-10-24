import json
import logging
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from clipper.clipper import Clipper
from clipper.clip_settings import get_time_delta_string, parse_clip_data, read_from_json
from clipper.ffmpeg import build_text_filter, run_ffmpeg_cmd


class ClipperTests(unittest.TestCase):
    def test_fractional_end_is_absolute_and_supports_long_videos(self):
        for start, end, expected in [('00:01:10.70', '00:01:15.90', '5.20'),
                                      (70.5, 75.75, '5.25'), ('25:00:00', '25:00:05', '5')]:
            with self.subTest(start=start):
                self.assertEqual(get_time_delta_string({'start': start, 'end': end}), expected)
        for start, end in [('00:00:10', '00:00:09'), (0, float('nan')), (0, '00:60:01')]:
            with self.assertRaises(ValueError):
                get_time_delta_string({'start': start, 'end': end})

    def test_minimal_config_defaults_and_independent_clip_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'config.json'
            path.write_text(json.dumps({'clips': [{'start': 0, 'end': 1}, {'start': 1, 'end': 2}]}))
            clips = read_from_json(path, 'video.mp4')
            self.assertEqual(clips[0]['mode'], '#101010')
            clips[0]['inputs'][0] = 'cut.mp4'
            self.assertEqual(clips[1]['inputs'], ['video.mp4'])
            self.assertIsNone(parse_clip_data({'full_path': 'video.mp4'}, clips[1])['background'])

    def test_explicit_config_wins_and_assets_resolve_relative_to_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'config.json').write_text('{"clips": []}')
            (root / 'custom.json').write_text(json.dumps({'background': '#FFFFFF', 'logo': 'logo.png',
                'clips': [{'start': 0, 'end': 1}]}))
            clipper = Clipper(str(root / 'video.mp4'), 'custom.json', False, logging.getLogger('test'))
            clipper.load_clips_config()
            self.assertEqual(len(clipper.clips), 1)
            self.assertEqual(clipper.clips[0]['mode'], '#FFFFFF')
            self.assertEqual(clipper.clips[0]['inputs'], [str(root / 'video.mp4'), str((root / 'logo.png').resolve())])

    def test_multiple_texts_share_a_single_filter_chain(self):
        result = build_text_filter({'texts': [{'text': 'one'}, {'text': 'two'}], 'input_types': ['logo']}, {})
        self.assertEqual(result.count('drawtext='), 2)
        self.assertEqual(result.count('[m1]'), 1)
        self.assertTrue(result.endswith('[m1]'))

    def test_ffmpeg_failure_stops_pipeline(self):
        with patch('clipper.ffmpeg.DEBUG', False), patch('clipper.ffmpeg.subprocess.Popen') as popen:
            popen.return_value.returncode = 1
            with self.assertRaises(RuntimeError):
                run_ffmpeg_cmd(['ffmpeg', '-i', 'missing.mp4'])
