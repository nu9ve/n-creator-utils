import os, time, sys
import wave
import subprocess
import contextlib
import shutil
from pathlib import Path
import math

from constants import MONTH_STRING_NUMBERS


def metatag_updater(directory):
  pass

def get_audio_file_duration(file_path):
  duration = -1
  mm = 0
  ss = 0
  file_duration_string = ''
  if Path(file_path).suffix.lower() == '.wav':
    with contextlib.closing(wave.open(file_path,'r')) as f:
      frames = f.getnframes()
      rate = f.getframerate()
      duration = frames / float(rate)
      minutes = duration/60
      mm = int(minutes)
      ss = int((minutes - mm) * 60)
  elif Path(file_path).suffix.lower() == '.mp3':
    result = subprocess.run([
      'ffprobe', '-v', 'error', '-show_entries', 'format=duration',
      '-of', 'default=noprint_wrappers=1:nokey=1', str(file_path),
    ], capture_output=True, text=True)
    if result.returncode:
      raise ValueError('ffprobe could not read audio: {}'.format(file_path))
    duration = float(result.stdout.strip())
    if not math.isfinite(duration) or duration < 0:
      raise ValueError('Invalid audio duration: {}'.format(file_path))
    mm, ss = divmod(int(duration), 60)
  if duration == -1:
    return duration, '00-00'
  if mm == 0:
    file_duration_string = '00-'
  elif mm > 0 and mm < 10:
    file_duration_string = '0{}-'.format(mm)
  else:
    file_duration_string = '{}-'.format(mm)
  if ss == 0:
    file_duration_string += '00'
  elif ss > 0 and ss < 10:
    file_duration_string += '0{}'.format(ss)
  else:
    file_duration_string += '{}'.format(ss)
  return duration, file_duration_string

def get_audio_file_description(file_path, src_name):
  audio_data = dict()
  if src_name == 'hijack':
    mic_name = ''
  else:
    mic_file = Path(file_path).name
    parts = mic_file.split('.')[0].split('_')
    mic_name = parts[1] if len(parts) > 1 else ''
  duration, duration_string = get_audio_file_duration(file_path)
  audio_data['duration'] = duration
  audio_data['duration_string'] = duration_string
  audio_data['mic_name'] = mic_name
  return audio_data
