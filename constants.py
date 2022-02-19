from os.path import join as join_path
from sys import platform

DEBUG = True
# DEBUG = False

LOGGER_NAME = 'nu9ve'

ROOT_DIR = '/Volumes/MEDIA501'
SD_ROOT_DIR = '/Volumes'
ROOT_RECORDINGS_DIR = '/Volumes/SSD1'

if platform == "linux" or platform == "linux2":
  # linux
  print('linux')
elif platform == "darwin":
  # OS X
  SD_ROOT_DIR = '/Volumes'
elif platform == "win32":
  # Windows...
  print('windows')

AUDIO_HIJACK_DIR = '~/Music/Audio Hijack/'
FINAL_CORRUPT_DIR = join_path(ROOT_DIR, 'corrupt')
FINAL_DRON_DIR = join_path(ROOT_DIR, 'vdron')
FINAL_4K_DIR = join_path(ROOT_DIR, 'v4k')
FINAL_VIDEOS_DIR = join_path(ROOT_DIR, 'videos')
FINAL_IMAGES_DIR = join_path(ROOT_DIR, 'images')
FINAL_RAW_DIR = join_path(ROOT_DIR, 'rimages')
FINAL_SCREENS_DIR = join_path(ROOT_DIR, 'screens')
FINAL_SCREENR_DIR = join_path(ROOT_DIR, 'screens', 'recordings')
FINAL_RECORDINGS_DIR = join_path(ROOT_DIR, 'recordings')
FINAL_LONG_RECORDINGS_DIR = join_path(ROOT_DIR, 'recordingslong')
FINAL_HIJACK_DIR = join_path(ROOT_DIR, 'hijack')
FINAL_LONG_HIJACK_DIR = join_path(ROOT_DIR, 'hijacklong')
SSD_RECORDINGS_DIR = join_path(ROOT_RECORDINGS_DIR, 'recordings')
SSD_LONG_RECORDINGS_DIR = join_path(ROOT_RECORDINGS_DIR, 'recordingslong')
SSD_HIJACK_DIR = join_path(ROOT_RECORDINGS_DIR, 'hijack')
SSD_LONG_HIJACK_DIR = join_path(ROOT_RECORDINGS_DIR, 'hijacklong')

SD_SRC_NAME = {
  'EOS_DIGITAL': 'SL3',
  'GOPRO64': 'GOPRO4',
  'MAVIC200': 'MAVIC2',
  'H6_SD': 'H6'
}



# os.unmount(path)
# os.system("drutil tray open")
# linux
# os.system("eject -t cdrom")

# /audio
#   /recordings
#   /recordings_long
#   /sounds
#   /songs
#
# /images
#   /camera
#   /phone
#   /screens
#
# /video
#   /drone
#   /4k
#   /1080

FINAL_DIRS = {
  'DRONE': 'dron'
}

MONTH_STRING_NUMBERS = {
  'Jan': '01',
  'Feb': '02',
  'Mar': '03',
  'Apr': '04',
  'May': '05',
  'Jun': '06',
  'Jul': '07',
  'Aug': '08',
  'Sep': '09',
  'Oct': '10',
  'Nov': '11',
  'Dec': '12',
}


MONTH_NUMBER_STRINGS = {
  '01': 'Jan',
  '02': 'Feb',
  '03': 'Mar',
  '04': 'Apr',
  '05': 'May',
  '06': 'Jun',
  '07': 'Jul',
  '08': 'Aug',
  '09': 'Sep',
  '10': 'Oct',
  '11': 'Nov',
  '12': 'Dec',
}


MONTH_NUMBER_DAYS = {
  '01': 31,
  '02': 28,
  '03': 31,
  '04': 30,
  '05': 31,
  '06': 30,
  '07': 31,
  '08': 31,
  '09': 30,
  '10': 31,
  '11': 30,
  '12': 31,
}