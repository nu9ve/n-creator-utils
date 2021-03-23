import os, time, sys
import wave
import subprocess
import contextlib
import shutil
import logging
from os.path import join as join_path, exists as path_exists

from utils.constants import (LOGGER_NAME, MONTH_STRING_NUMBERS, MONTH_NUMBER_DAYS,
  SD_CAM_NAME, FINAL_DRON_DIR, FINAL_4K_DIR, FINAL_VIDEOS_DIR, FINAL_IMAGES_DIR, 
  FINAL_SCREENS_DIR, MONTH_NUMBER_STRINGS)


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

def get_file_modified_time(file_path):
  # ['Wed', 'Apr', '20', '06:13:20', '2016']
  # ['06', '13', '20']
  time_string = time.ctime(os.path.getmtime(file_path))
  ts = list(filter(None, time_string.split(' ')))
  tds = ts[3].split(':')
  return ts, tds

def get_video_file_description(folder, cam_name, video_file):
  video_data = dict()
  video_file_path = join_path(folder, video_file)
  duration, file_duration_string = get_video_file_duration(video_file_path)
  ts, tds = get_file_modified_time(video_file_path)
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}_{}'.format(file_date_string, cam_name.upper(), file_duration_string)
  video_data['duration'] = duration
  video_data['day'] = ts[2]
  video_data['month'] = MONTH_STRING_NUMBERS[ts[1]]
  video_data['year'] = ts[4]
  return new_name, video_data

def get_file_description(file_path, video_file, image_file, audio_file):
  file_data = dict()
  ts, tds = get_file_modified_time(file_path)
  if video_file:
    duration, file_duration_string = get_video_file_duration(file_path)
    file_data['duration'] = duration
    file_data['duration_string'] = file_duration_string

  file_data['date'] = ts
  file_data['time'] = tds
  return file_data

def complete_time_string(ts):
  time_string = str(ts)
  if ts < 10:
    time_string = '0' + str(ts)
  return time_string

def is_4k(file_path):
  is4k = False
  probe_cmd = [ 'ffprobe', '-v', 'error', '-select_streams', 'v:0', 
          '-show_entries', 'stream=width,height,r_frame_rate', '-of', 'csv=s=x:p=0', file_path]
  result = subprocess.run(probe_cmd, capture_output=True)
  dims = str(result.stdout).split('x')
  v_w = dims[0].split("'")[1]
  v_h = dims[1].split("'")[0].replace('\\n','')
  if int(v_w) > 2000 or int(v_h) > 2000:
    is4k = True
  return is4k

def update_file_time(ts, tds):
  # update for gopro no time setup

  day_update = 13
  month_update = 10
  minute_update = 48
  year_update = 4
  update_day = False
  update_month = False
  update_year = False

  new_year = int(ts[4]) + year_update
  new_minute = int(tds[1]) + minute_update
  new_month = int(MONTH_STRING_NUMBERS[ts[1]]) + month_update
  new_day = int(ts[2]) + day_update

  if new_minute > 59:
    new_hour = int(tds[0]) + 1
    new_minute = new_minute % 60
    tds[1] = complete_time_string(new_minute)
    if new_hour > 24:
      new_hour = new_hour % 24
      update_day = True
    tds[0] = complete_time_string(new_hour)
  if new_month > 12:
    new_month = new_month % 12
    update_year = True
  month_string = complete_time_string(new_month)  

  if update_day:
    new_day = new_day + 1
  if new_day > MONTH_NUMBER_DAYS[month_string]:
    update_month = True
    new_day = new_day % MONTH_NUMBER_DAYS[month_string]
  if update_month:
    new_month = new_month + 1
    if new_month > 12:
      new_month = new_month % 12
      update_year = True
    month_string = complete_time_string(new_month)  
  if update_year:
    new_year = new_year + 1
  
  ts[1] = MONTH_NUMBER_STRINGS[month_string]
  ts[2] = str(new_day)
  ts[3] = ':'.join(tds)
  ts[4] = str(new_year)
  
  return ts, tds

def store_video_files():
  # erikpyado save
  #   check plugged memories, if name available in SD_CAM_NAME
  # erikpyado videos
  # 2019/
  #   month (1-12)
  #     year(19)month(01-12)day(01-31)_micname_duration(mmm-ss)_
  #     191131-2355_H6LR_20-02_JP-TOKYO[optional(place,mood)].WAV

  done_test = False

  # get_external_storage
  if len(sys.argv) != 6:
    error = 'erikpyado videos src_dir cam jp tag'
    logger.error(error)
    return

  source_dir = sys.argv[2]
  cam_name = sys.argv[3]
  country_code = sys.argv[4]
  city_name = sys.argv[5]

  for r, ds, fs in os.walk(source_dir):
    for media_file in fs:
      move_file = False
      delete_file = False
      video_file = False
      image_file = False
      audio_file = False
      source_path = join_path(source_dir, media_file)
      media_file_parts = media_file.split('.')
      file_extension = media_file_parts[len(media_file_parts)-1].lower()

      if 'png' == file_extension or 'jpg' == file_extension or 'jpeg' == file_extension:
        image_file = True
      if 'mov' == file_extension or 'mp4' == file_extension:
        video_file = True
      if 'wav' == file_extension:
        audio_file = True
      if 'lrv' == file_extension or 'thm' == file_extension:
        delete_file = True

      file_data = get_file_description(source_path, video_file, image_file, audio_file)
      
      if 'GOP' in media_file[:3] or 'GP' in media_file[:3] or 'G0' in media_file[:2]:
        cam_name = 'GOPRO'
        if int(file_data['date'][4]) < 2018:
          file_data['date'], file_data['time'] = update_file_time(file_data['date'], file_data['time'])
      
      file_data['day'] = file_data['date'][2]
      file_data['month'] = MONTH_STRING_NUMBERS[file_data['date'][1]]
      file_data['year'] = file_data['date'][4]

      file_date_string = '{}{}{}-{}{}{}'.format(file_data['date'][4][-2:],file_data['date'][1].upper(),file_data['date'][2],file_data['time'][0],file_data['time'][1],file_data['time'][2])
      new_file_name = '{}_{}'.format(file_date_string, cam_name.upper())

      if image_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension)
        if 'screenshot' in source_path or 'Screenshot' in source_path:
          final_dir = join_path(FINAL_SCREENS_DIR,file_data['year'],file_data['month'])
        else:
          final_dir = join_path(FINAL_IMAGES_DIR,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if video_file:
        new_file_name += '_{}'.format(file_data['duration_string'])
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension)
        is4k = is_4k(source_path)
        category_dir = FINAL_VIDEOS_DIR
        if is4k:
          category_dir = FINAL_4K_DIR
        final_dir = join_path(category_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      logger.info('source: {}'.format(source_path))
      if move_file:
        logger.info('moving to: {}\n'.format(final_path))
        shutil.move(source_path, final_path)
      if delete_file:
        logger.info('deleting: {}\n'.format(source_path))
        os.remove(source_path)
      if not delete_file and not move_file:
        logger.info('no action: {}\n'.format(source_path))


