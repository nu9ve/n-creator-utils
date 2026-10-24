"""Read clip jobs without loading cloud clients or rendering media."""

import copy
import json
import math
from decimal import Decimal, InvalidOperation
from pathlib import Path


def time_component_str(t):
    return str(t).zfill(2)


def timestamp_seconds(value):
    try:
        parts = str(value).split(':')
        if len(parts) == 1:
            seconds = Decimal(parts[0])
        elif len(parts) == 3:
            hours, minutes = int(parts[0]), int(parts[1])
            fraction = Decimal(parts[2])
            if hours < 0 or not 0 <= minutes < 60 or not 0 <= fraction < 60:
                raise ValueError('Invalid timestamp: {}'.format(value))
            seconds = hours * 3600 + minutes * 60 + fraction
        else:
            raise ValueError('Use seconds or HH:MM:SS[.fraction]: {}'.format(value))
        if not seconds.is_finite() or seconds < 0:
            raise ValueError('Timestamp must be finite and nonnegative')
        return seconds
    except (InvalidOperation, TypeError) as error:
        raise ValueError('Invalid timestamp: {}'.format(value)) from error


def get_times(time_string):
    seconds = timestamp_seconds(time_string)
    whole = int(seconds)
    return whole // 3600, (whole // 60) % 60, whole % 60, int((seconds - whole) * 1000000)


def get_time_delta_string(clip):
    duration = timestamp_seconds(clip['end']) - timestamp_seconds(clip['start'])
    if duration <= 0:
        raise ValueError('Clip end must be greater than start')
    return format(duration, 'f')


def get_video_info(video_path):
    from manager.video import probe_video
    stream = probe_video(video_path)['streams'][0]
    numerator, denominator = stream.get('r_frame_rate', '0/1').split('/')
    width, height = int(stream['width']), int(stream['height'])
    if width <= 0 or height <= 0 or float(denominator) == 0:
        raise ValueError('Invalid video dimensions or frame rate')
    return {
        'width': str(width), 'height': str(height), 'ratio': width / height,
        'frame_rate': float(numerator) / float(denominator),
        'file_name': Path(video_path).name, 'format': Path(video_path).suffix.lstrip('.'),
        'full_path': video_path, 'path': video_path,
    }


def parse_clip_data(video_data, clip):
    data = dict(video_data)
    data.update({
        'start': str(clip['start']), 'ffmpeg_end': get_time_delta_string(clip),
        'mode': clip.get('mode', '#1F1F1F'), 'valid': True,
        'vertical_only': clip.get('vertical_only', True), 'view': clip.get('view', 90),
        'texts': copy.deepcopy(clip.get('texts', [])),
        'inputs': list(clip.get('inputs', [video_data['full_path']])),
        'input_types': list(clip.get('input_types', [])),
        'background': clip.get('background'),
    })
    return data


def read_from_json(file_path, video_path):
    with open(file_path, encoding='utf-8') as source:
        config = json.load(source)
    if not isinstance(config, dict) or not isinstance(config.get('clips'), list):
        raise ValueError('Clip configuration must contain a clips array')
    base = Path(file_path).resolve().parent
    inputs, input_types = [video_path], []
    if config.get('logo'):
        logo = Path(config['logo']).expanduser()
        inputs.append(str(logo if logo.is_absolute() else base / logo))
        input_types.append('logo')
    background = config.get('background')
    mode = config.get('mode', '#101010')
    # Old examples used background for both colors and file inputs.
    if background:
        if background.startswith('#'):
            mode = background
        else:
            path = Path(background).expanduser()
            background = str(path if path.is_absolute() else base / path)
            inputs.append(background)
            input_types.append('background')
    clips = []
    for index, entry in enumerate(config['clips']):
        if not isinstance(entry, dict) or 'start' not in entry or 'end' not in entry:
            raise ValueError('Clip {} requires start and end'.format(index))
        get_time_delta_string(entry)
        clip = copy.deepcopy(entry)
        clip['mode'] = entry.get('mode', mode)
        clip['view'] = entry.get('view', config.get('view', 90))
        if isinstance(clip['view'], bool) or not isinstance(clip['view'], (int, float)) or not 0 <= clip['view'] <= 100:
            raise ValueError('Clip view must be a number between 0 and 100')
        clip['vertical_only'] = entry.get('vertical_only', config.get('vertical_only', True))
        if not isinstance(clip['vertical_only'], bool):
            raise ValueError('vertical_only must be true or false')
        clip['inputs'], clip['input_types'] = list(inputs), list(input_types)
        clip['background'] = background
        global_texts, local_texts = config.get('texts', []), entry.get('texts', [])
        if not isinstance(global_texts, list) or not isinstance(local_texts, list):
            raise ValueError('texts must be an array')
        clip['texts'] = copy.deepcopy(global_texts + local_texts)
        if 'title' in entry:
            clip['texts'].append({'text': entry['title'], 'y': 98 - clip['view']})
        text_size = config.get('text_size', 3)
        for text in clip['texts']:
            if not isinstance(text, dict) or not isinstance(text.get('text'), str):
                raise ValueError('Each text requires a text string')
            text.setdefault('text_size', text_size)
            size = float(text['text_size'])
            if not math.isfinite(size) or size <= 0:
                raise ValueError('text_size must be positive and finite')
        clips.append(clip)
    return clips
