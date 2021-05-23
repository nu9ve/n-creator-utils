import os, time, sys
import shutil
import logging
from os.path import join as join_path, exists as path_exists

from constants import (LOGGER_NAME, MONTH_STRING_NUMBERS, MONTH_NUMBER_DAYS,
  SD_SRC_NAME, FINAL_DRON_DIR, FINAL_4K_DIR, FINAL_VIDEOS_DIR, FINAL_IMAGES_DIR, 
  FINAL_SCREENS_DIR, MONTH_NUMBER_STRINGS, SD_ROOT_DIR, FINAL_RECORDINGS_DIR,
  FINAL_LONG_RECORDINGS_DIR, SSD_RECORDINGS_DIR, SSD_LONG_RECORDINGS_DIR,
  FINAL_CORRUPT_DIR)
from manager.video import is_4k, get_video_file_duration
from manager.audio import get_audio_file_description


logger = logging.getLogger(LOGGER_NAME)

def get_file_modified_time(file_path):
  # ['Wed', 'Apr', '20', '06:13:20', '2016']
  # ['06', '13', '20']
  time_string = time.ctime(os.path.getmtime(file_path))
  ts = list(filter(None, time_string.split(' ')))
  tds = ts[3].split(':')
  return ts, tds

def get_file_description(file_path, video_file, image_file, audio_file):
  file_data = dict()
  ts, tds = get_file_modified_time(file_path)
  if video_file:
    duration, file_duration_string = get_video_file_duration(file_path)
    file_data['duration'] = duration
    file_data['duration_string'] = file_duration_string
  if audio_file:
    audio_data = get_audio_file_description(file_path)
    file_data = {**file_data, **audio_data}
  file_data['date'] = ts
  file_data['time'] = tds
  return file_data

def complete_time_string(ts):
  time_string = str(ts)
  if ts < 10:
    time_string = '0' + str(ts)
  return time_string

def update_file_time(ts, tds):
  # update for gopro no time setup
  # gopro settings
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

def save_tag_files(src_dir, src_name, country_code, city_name):
  for r, ds, fs in os.walk(src_dir):
    for media_file in fs:
      copy_file = False
      move_file = False
      delete_file = False
      video_file = False
      image_file = False
      audio_file = False
      corrupt_file = False
      source_path = join_path(r, media_file)
      media_file_parts = media_file.split('.')
      file_extension = media_file_parts[len(media_file_parts)-1].lower()

      if 'png' == file_extension or 'jpg' == file_extension or 'jpeg' == file_extension:
        image_file = True
      if 'mov' == file_extension or 'mp4' == file_extension:
        video_file = True
      if 'wav' == file_extension:
        audio_file = True
      if 'dat' == file_extension:
        corrupt_file = True
      if 'lrv' == file_extension or 'thm' == file_extension or 'hprj' == file_extension:
        delete_file = True

      file_data = get_file_description(source_path, video_file, image_file, audio_file)
      
      if 'GOP' in media_file[:3] or 'GP' in media_file[:3] or 'G0' in media_file[:2]:
        src_name = 'GOPRO'
        if int(file_data['date'][4]) < 2018:
          file_data['date'], file_data['time'] = update_file_time(file_data['date'], file_data['time'])
      
      file_data['day'] = file_data['date'][2]
      file_data['month'] = MONTH_STRING_NUMBERS[file_data['date'][1]]
      file_data['year'] = file_data['date'][4]

      file_date_string = '{}{}{}-{}{}{}'.format(file_data['date'][4][-2:],file_data['date'][1].upper(),file_data['date'][2],file_data['time'][0],file_data['time'][1],file_data['time'][2])
      new_file_name = '{}_{}'.format(file_date_string, src_name.upper())

      if corrupt_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        final_dir = join_path(FINAL_CORRUPT_DIR,file_data['year'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if image_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        if 'screenshot' in source_path or 'Screenshot' in source_path:
          final_dir = join_path(FINAL_SCREENS_DIR,file_data['year'],file_data['month'])
        else:
          final_dir = join_path(FINAL_IMAGES_DIR,file_data['year'],file_data['month'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if video_file:
        new_file_name += '_{}'.format(file_data['duration_string'])
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        is4k = is_4k(source_path)
        category_dir = FINAL_VIDEOS_DIR
        if is4k:
          category_dir = FINAL_4K_DIR
        final_dir = join_path(category_dir,file_data['year'],file_data['month'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if audio_file:
        new_file_name += '{}_{}'.format(file_data['mic_name'].upper(), file_data['duration_string'])
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        category_dir = FINAL_RECORDINGS_DIR
        backup_category_dir = SSD_RECORDINGS_DIR
        if file_data['duration'] > 600:
          category_dir = FINAL_LONG_RECORDINGS_DIR
          backup_category_dir = SSD_LONG_RECORDINGS_DIR
        final_dir = join_path(category_dir,file_data['year'],file_data['month'])
        backup_dir = join_path(backup_category_dir,file_data['year'],file_data['month'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        if not path_exists(backup_dir): 
          os.makedirs(backup_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True
        backup_path = join_path(backup_dir, new_file_name)
        copy_file = True

      logger.info('source: {}'.format(source_path))
      if copy_file:
        logger.info('copying to: {}'.format(backup_path))
        shutil.copyfile(source_path, final_path)
      if move_file:
        logger.info('moving to: {}\n'.format(final_path))
        shutil.move(source_path, final_path)
      if delete_file:
        logger.info('deleting: {}\n'.format(source_path))
        os.remove(source_path)
      if not delete_file and not move_file:
        logger.info('no action: {}\n'.format(source_path))


# cmd line interface
def save_files():
  # erikpyado save
  #   check plugged memories, if name available in SD_SRC_NAME
  # erikpyado videos
  # 2019/
  #   month (1-12)
  #     year(19)month(01-12)day(01-31)_micname_duration(mmm-ss)_
  #     191131-2355_H6LR_20-02_JP-TOKYO[optional(place,mood)].WAV

  # get_external_storage
  help_error = '''erikpyado save [src_dir|all] jp tag [src_name]\n\t
        all - saves configured sd card names\n\t
        src_dir - requires src_name'''

  if len(sys.argv) != 6 and len(sys.argv) != 5:
    logger.error(help_error)
    return

  src_dir = sys.argv[2]
  country_code = sys.argv[3]
  city_name = sys.argv[4]

  if src_dir == 'all':
    # look for sds

    for sd in SD_SRC_NAME:
      src_path = join_path(SD_ROOT_DIR, sd)
      if path_exists(src_path):
        logger.info('{} found. Saving...'.format(src_path))
        save_tag_files(src_path, SD_SRC_NAME[sd], country_code, city_name)
  else:
    if len(sys.argv) != 6:
      logger.error(help_error)
      return

    src_name = sys.argv[5]
    save_tag_files(src_dir, src_name, country_code, city_name)

