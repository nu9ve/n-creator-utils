"""Video metadata via a single checked, structured ffprobe interface."""

import json
import math
import subprocess
from pathlib import Path


def probe_video(filename):
    result = subprocess.run([
        'ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries',
        'stream=width,height,r_frame_rate:format=duration', '-of', 'json', str(filename),
    ], capture_output=True, text=True)
    if result.returncode:
        raise ValueError('ffprobe could not read {}: {}'.format(filename, result.stderr.strip()))
    try:
        data = json.loads(result.stdout)
        if not data.get('streams'):
            raise ValueError('No video stream in {}'.format(filename))
        return data
    except (TypeError, json.JSONDecodeError) as error:
        raise ValueError('Invalid ffprobe output for {}'.format(filename)) from error


def get_video_length(filename):
    try:
        duration = float(probe_video(filename)['format']['duration'])
        if duration < 0 or not math.isfinite(duration):
            raise ValueError('Invalid duration')
        return duration
    except (KeyError, TypeError) as error:
        raise ValueError('No duration in {}'.format(filename)) from error


def get_video_file_duration(file_path):
    if Path(file_path).suffix.lower() not in {'.mov', '.mp4', '.mpg', '.mts', '.insv'}:
        return -1, '00-00'
    duration = int(get_video_length(file_path))
    minutes, seconds = divmod(duration, 60)
    return duration, '{:02d}-{:02d}'.format(minutes, seconds)


def is_4k(file_path):
    stream = probe_video(file_path)['streams'][0]
    # Keep the original routing threshold (either dimension > 2000).
    return int(stream['width']) > 2000 or int(stream['height']) > 2000
