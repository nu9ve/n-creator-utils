import os, time, sys
import wave
import subprocess
import contextlib
import shutil
import logging
from os.path import join as join_path

from utils.constants import LOGGER_NAME, MONTH_STRING_NUMBERS, SD_CARD_NAME


logger = logging.getLogger(LOGGER_NAME)


# FFPROBE CMDS
def get_video_length(filename):
    result = subprocess.run(["ffprobe", "-v", "error", "-show_entries",
                             "format=duration", "-of",
                             "default=noprint_wrappers=1:nokey=1", filename],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT)
    return float(result.stdout)


def get_video_file_duration(file_path):
  duration = -1
  mm = 0
  ss = 0
  file_duration_string = ''
  if '.MP4' in file_path or '.mp4' in file_path:
    duration = int(get_video_length(file_path))
    minutes = duration/60
    mm = int(minutes)
    ss = int((minutes - mm) * 60)
  if '.MOV' in file_path or '.mov' in file_path:
    duration = int(get_video_length(file_path))
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


def get_video_file_description(folder, cam_name, video_file):
  video_data = dict()
  video_file_path = join_path(folder, video_file)
  time_string = time.ctime(os.path.getmtime(video_file_path))
  ts = list(filter(None, time_string.split(' ')))
  duration, file_duration_string = get_video_file_duration(video_file_path)
  tds = ts[3].split(':')
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}_{}'.format(file_date_string, cam_name.upper(), file_duration_string)
  video_data['duration'] = duration
  video_data['day'] = ts[2]
  video_data['month'] = MONTH_STRING_NUMBERS[ts[1]]
  video_data['year'] = ts[4]
  return new_name, video_data


def get_image_file_description(folder, cam_name, video_file):
  video_data = dict()
  video_file_path = join_path(folder, video_file)
  time_string = time.ctime(os.path.getmtime(video_file_path))
  ts = list(filter(None, time_string.split(' ')))
  tds = ts[3].split(':')
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}'.format(file_date_string, cam_name.upper())
  video_data['day'] = ts[2]
  video_data['month'] = MONTH_STRING_NUMBERS[ts[1]]
  video_data['year'] = ts[4]
  return new_name, video_data


def store_video_files():
  # erikpyado save
  #   check plugged memories, if name available in SD_CARD_NAME
  # erikpyado videos
  # 2019/
  #   month (1-12)
  #     year(19)month(01-12)day(01-31)_micname_duration(mmm-ss)_
  #     191131-2355_H6LR_20-02_JP-TOKYO[optional(place,mood)].WAV
  done_test = False
  root_dir = '/Volumes/HDD4'
  final_dron_dir = join_path(root_dir,'vdron')
  final_4k_dir = join_path(root_dir,'v4k')
  final_videos_dir = join_path(root_dir,'videos')
  final_images_dir = join_path(root_dir,'images')
  final_screens_dir = join_path(root_dir,'screens')

  if len(sys.argv) != 6:
    error = 'erikpyado videos src_dir cam jp tag'
    logger.error(error)
    return
  source_dir = sys.argv[2]
  cam_name = sys.argv[3]
  country_code = sys.argv[4]
  city_name = sys.argv[5]
  for r, ds, fs in os.walk(source_dir):
    # for folder in ds:
      # recording_folder = join_path(r, folder)
      # for vr, vds, vfs in os.walk(recording_folder):
    for video_file in fs:
      if '.PNG' == video_file[-4:] or '.png' == video_file[-4:]:
        new_file_name, file_data = get_image_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].png'.format(country_code.upper(), city_name.upper())
        initial_path = join_path(source_dir, video_file)
        if 'screenshot' in initial_path or 'Screenshot' in initial_path:
          final_dir = join_path(final_screens_dir,file_data['year'],file_data['month'])
        else:
          final_dir = join_path(final_images_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.JPG' == video_file[-4:] or '.jpg' == video_file[-4:]:
        if 'GOP' in video_file[:3] or 'GP' in video_file[:3] or 'G0' in video_file[:2]:
          cam_name = 'GOPRO'
        new_file_name, file_data = get_image_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].jpg'.format(country_code.upper(), city_name.upper())
        initial_path = join_path(source_dir, video_file)
        if 'screenshot' in initial_path or 'Screenshot' in initial_path:
          final_dir = join_path(final_screens_dir,file_data['year'],file_data['month'])
        else:
          final_dir = join_path(final_images_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.MP4' == video_file[-4:] or '.mp4' == video_file[-4:]:
        if 'GOP' in video_file[:3] or 'GP' in video_file[:3]:
          cam_name = 'GOPRO'
        new_file_name, file_data = get_video_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].mp4'.format(country_code.upper(), city_name.upper())
        # if file_data['duration'] > 600: #10 min
        #   final_dir = join_path(long_recordings_dir,file_data['year'],file_data['month'])
        # else:
        final_dir = join_path(final_videos_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        initial_path = join_path(source_dir, video_file)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.MOV' == video_file[-4:] or '.mov' == video_file[-4:]:
        new_file_name, file_data = get_video_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].mov'.format(country_code.upper(), city_name.upper())
        # if file_data['duration'] > 600: #10 min
        #   final_dir = join_path(long_recordings_dir,file_data['year'],file_data['month'])
        # else:
        final_dir = join_path(final_videos_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        initial_path = join_path(source_dir, video_file)
        print(final_path)
        shutil.move(initial_path, final_path)

