from os.path import join as join_path
import datetime
import pathlib
from configuration import load_settings

SETTINGS = load_settings()
DEBUG = SETTINGS['debug']

LOGGER_NAME = 'nu9ve'

DRIVE_NAME = 'MEDIA502'
ROOT_DIR = SETTINGS['root_dir']
SD_ROOT_DIR = SETTINGS['sd_root_dir']
ROOT_RECORDINGS_DIR = SETTINGS['root_recordings_dir']
CLIPPER_FONT_FILE = SETTINGS['clipper_font_file']
YT_DLP_COOKIES_BROWSER = SETTINGS['yt_dlp_cookies_browser']
username = pathlib.Path.home().name

# get current year string
CURRENT_YEAR = datetime.datetime.now().strftime('%Y')

AUDIO_HIJACK_DIR = SETTINGS['audio_hijack_dir']
AUDIO_SAMPLES_DIR = SETTINGS['audio_samples_dir']
AUDIO_SPLITS_DIR = SETTINGS['audio_splits_dir']
AUDIO_SONGS_DIR = SETTINGS['audio_songs_dir']
TMP_VIDEO_DIR = SETTINGS['tmp_video_dir']
FINAL_CORRUPT_DIR = join_path(ROOT_DIR, 'corrupt')
FINAL_DRON_DIR = join_path(ROOT_DIR, 'vdron')
FINAL_4K_DIR = join_path(ROOT_DIR, 'v4k')
FINAL_360_VID_DIR = join_path(ROOT_DIR, '360/videos')
FINAL_360_IMG_DIR = join_path(ROOT_DIR, '360/images')
FINAL_INSTA360_DIR = join_path(ROOT_DIR, 'insta360')
FINAL_VIDEOS_DIR = join_path(ROOT_DIR, 'videos')
FINAL_METADATA_DIR = join_path(ROOT_DIR, 'metadata')
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

SD_SRC_NAME = SETTINGS['sd_src_name']



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
