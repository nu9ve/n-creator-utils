import os, time, sys
import wave
import subprocess
import contextlib
import shutil

from utils.constants import MONTH_STRING_NUMBERS


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


def get_audio_file_description(folder, recorder, mic_file):
  audio_data = dict()
  mic_name = mic_file.split('.')[0].split('_')[1]
  mic_file_path = os.path.join(folder, mic_file)
  time_string = time.ctime(os.path.getmtime(mic_file_path))
  ts = list(filter(None, time_string.split(' ')))
  duration, file_duration_string = get_audio_file_duration(mic_file_path)
  tds = ts[3].split(':')
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}{}_{}'.format(file_date_string, recorder, mic_name.upper(), file_duration_string)
  audio_data['duration'] = duration
  audio_data['day'] = ts[2]
  audio_data['month'] = MONTH_STRING_NUMBERS[ts[1]]
  audio_data['year'] = ts[4]
  return new_name, audio_data


def store_audio_files():
  # 2019/
  #   month (1-12)
  #     year(19)month(01-12)day(01-31)_micname_duration(mmm-ss)_
  #     191131-2355_H6LR_20-02_JP-TOKYO[optional(place,mood)].WAV
  done_test = False
  zoom_dir = '/Volumes/H6_SD/'
  recordings_dir = '/Volumes/SSD1/recordings/'
  long_recordings_dir = '/Volumes/SSD1/recordingslong/'
  if len(sys.argv) != 5:
    print('erikpyado zoom /Volumes/SSD1/zoom/tokyo jp tokyo')
    return
  country_code = sys.argv[3]
  city_name = sys.argv[4]
  for r, ds, fs in os.walk(os.path.join(zoom_dir, sys.argv[2])):
    for folder in ds:
      recording_folder = os.path.join(r, folder)
      for zr, zds, zfs in os.walk(recording_folder):
        for mic_file in zfs:
          if '.WAV' == mic_file[-4:]:
            new_file_name, file_data = get_audio_file_description(recording_folder,'H6',mic_file)
            new_file_name += '_{}-{}[].WAV'.format(country_code.upper(), city_name.upper())
            if file_data['duration'] > 600: #10 min
              final_dir = os.path.join(long_recordings_dir,file_data['year'],file_data['month'])
            else:
              final_dir = os.path.join(recordings_dir,file_data['year'],file_data['month'])
            if not os.path.exists(final_dir): 
              os.makedirs(final_dir)
            final_path = os.path.join(final_dir, new_file_name)
            initial_path = os.path.join(recording_folder, mic_file)
            print(final_path)
            shutil.move(initial_path, final_path)

