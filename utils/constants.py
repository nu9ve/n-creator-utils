from os.path import join as join_path


LOGGER_NAME = 'kiddie'

ROOT_DIR = '/Volumes/HDD4'

FINAL_DRON_DIR = join_path(ROOT_DIR,'vdron')
FINAL_4K_DIR = join_path(ROOT_DIR,'v4k')
FINAL_VIDEOS_DIR = join_path(ROOT_DIR,'videos')
FINAL_IMAGES_DIR = join_path(ROOT_DIR,'images')
FINAL_SCREENS_DIR = join_path(ROOT_DIR,'screens')

SD_CAM_NAME = {
  'EOS_DIGITAL': '',
  'GO': '',
  'downloads': '',
  'DJI': ''
}

SD_RECORDER_NAME = {
  'H6_SD': ''
}

# from sys import platform
# if platform == "linux" or platform == "linux2":
#     # linux
# elif platform == "darwin":
#     # OS X
# elif platform == "win32":
#     # Windows...

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