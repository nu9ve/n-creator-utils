# import os, time, sys
# import shutil
# import logging
# from os.path import join as join_path, exists as path_exists

# from constants import (LOGGER_NAME, MONTH_STRING_NUMBERS, MONTH_NUMBER_DAYS,
#   SD_SRC_NAME, FINAL_DRON_DIR, FINAL_4K_DIR, FINAL_VIDEOS_DIR, FINAL_IMAGES_DIR, 
#   FINAL_SCREENS_DIR, MONTH_NUMBER_STRINGS, SD_ROOT_DIR, FINAL_RECORDINGS_DIR,
#   FINAL_LONG_RECORDINGS_DIR, SSD_RECORDINGS_DIR, SSD_LONG_RECORDINGS_DIR,
#   FINAL_CORRUPT_DIR)
# from manager.video import is_4k, get_video_file_duration
# from manager.audio import get_audio_file_description
from hardware.novation import LaunchControlXL

# cmd line interface
def hardware_control():
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

  # if len(sys.argv) != 6 and len(sys.argv) != 5:
  #   logger.error(help_error)
  #   return

  # src_dir = sys.argv[2]
  # country_code = sys.argv[3]
  # city_name = sys.argv[4]

  # if src_dir == 'all':
  #   # look for sds

  #   for sd in SD_SRC_NAME:
  #     src_path = join_path(SD_ROOT_DIR, sd)
  #     if path_exists(src_path):
  #       logger.info('{} found. Saving...'.format(src_path))
  #       save_tag_files(src_path, SD_SRC_NAME[sd], country_code, city_name)
  # else:
  #   if len(sys.argv) != 6:
  #     logger.error(help_error)
  #     return

  #   src_name = sys.argv[5]
  #   save_tag_files(src_dir, src_name, country_code, city_name)

