import os, time, sys
import wave
import subprocess
import contextlib
import shutil

from utils.constants import MONTH_STRING_NUMBERS


def metatag_updater(directory):
  pass

def get_audio_file_duration(file_path):
  duration = -1
  mm = 0
  ss = 0
  file_duration_string = ''
  if '.WAV' in file_path:
    with contextlib.closing(wave.open(file_path,'r')) as f:
      frames = f.getnframes()
      rate = f.getframerate()
      duration = frames / float(rate)
      minutes = duration/60
      mm = int(minutes)
      ss = int((minutes - mm) * 60)
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

def get_audio_file_description(file_path):
  audio_data = dict()
  path_parts = file_path.split('/')
  mic_file = path_parts[len(path_parts)-1]
  mic_name = mic_file.split('.')[0].split('_')[1]
  duration, duration_string = get_audio_file_duration(mic_file_path)
  audio_data['duration'] = duration
  audio_data['duration'] = duration_string
  audio_data['mic_name'] = mic_name
  return audio_data
