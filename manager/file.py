import os, time, sys
import shutil
import logging
from os.path import join as join_path, exists as path_exists
import subprocess
import re
import datetime
import xml.etree.ElementTree as ET

from constants import (LOGGER_NAME, MONTH_STRING_NUMBERS, MONTH_NUMBER_DAYS,
  SD_SRC_NAME, FINAL_DRON_DIR, FINAL_4K_DIR, FINAL_VIDEOS_DIR, FINAL_IMAGES_DIR, 
  FINAL_SCREENS_DIR, MONTH_NUMBER_STRINGS, SD_ROOT_DIR, FINAL_RECORDINGS_DIR,
  FINAL_LONG_RECORDINGS_DIR, SSD_RECORDINGS_DIR, SSD_LONG_RECORDINGS_DIR,
  FINAL_LONG_HIJACK_DIR, FINAL_HIJACK_DIR, SSD_HIJACK_DIR, SSD_LONG_HIJACK_DIR,
  FINAL_CORRUPT_DIR, FINAL_RAW_DIR, FINAL_SCREENR_DIR, AUDIO_HIJACK_DIR,
  FINAL_360_VID_DIR, FINAL_360_IMG_DIR, AUDIO_SAMPLES_DIR, AUDIO_SPLITS_DIR, 
  AUDIO_SONGS_DIR, TMP_VIDEO_DIR, FINAL_METADATA_DIR)
from manager.video import is_4k, get_video_file_duration
from manager.audio import get_audio_file_description


logger = logging.getLogger(LOGGER_NAME)

IGNORED_DIR_NAMES = {
  '.fseventsd',
  '.Spotlight-V100',
  '.Trashes',
  'System Volume Information',
}

def get_file_modified_time(file_path):
  # ['Wed', 'Apr', '20', '06:13:20', '2016']
  # ['06', '13', '20']
  time_string = time.ctime(os.path.getmtime(file_path))
  ts = list(filter(None, time_string.split(' ')))
  tds = ts[3].split(':')
  return ts, tds

def parse_datetime(value):
  if not value:
    return None
  try:
    clean_value = value.replace('Z', '+00:00')
    return datetime.datetime.fromisoformat(clean_value)
  except ValueError:
    return None

def get_sony_sidecar_path(file_path):
  base_name = os.path.basename(file_path)
  file_root, file_ext = os.path.splitext(base_name)
  if file_ext.lower() != '.mp4':
    return ''
  sidecar_name = '{}M01.XML'.format(file_root)
  sidecar_path = join_path(os.path.dirname(file_path), sidecar_name)
  if path_exists(sidecar_path):
    return sidecar_path
  return ''

def parse_sony_xml_metadata(xml_path):
  metadata = {}
  if not xml_path or not path_exists(xml_path):
    return metadata
  try:
    root = ET.parse(xml_path).getroot()
  except ET.ParseError as error:
    logger.warning('could not parse Sony XML {}: {}'.format(xml_path, error))
    return metadata

  def find_first(local_name):
    for element in root.iter():
      if element.tag.split('}')[-1] == local_name:
        return element
    return None

  creation_date = find_first('CreationDate')
  if creation_date is not None:
    metadata['creation_datetime'] = parse_datetime(creation_date.attrib.get('value'))

  video_frame = find_first('VideoFrame')
  if video_frame is not None:
    metadata['video_codec'] = video_frame.attrib.get('videoCodec', '')
    metadata['capture_fps'] = video_frame.attrib.get('captureFps', '')
    metadata['format_fps'] = video_frame.attrib.get('formatFps', '')

  video_layout = find_first('VideoLayout')
  if video_layout is not None:
    metadata['width'] = video_layout.attrib.get('pixel', '')
    metadata['height'] = video_layout.attrib.get('numOfVerticalLine', '')

  device = find_first('Device')
  if device is not None:
    metadata['manufacturer'] = device.attrib.get('manufacturer', '')
    metadata['model'] = device.attrib.get('modelName', '')

  return metadata

def get_video_creation_datetime(file_path):
  try:
    result = subprocess.run([
      'ffprobe', '-v', 'error',
      '-show_entries', 'format_tags=creation_time',
      '-of', 'default=noprint_wrappers=1:nokey=1',
      file_path,
    ], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
  except FileNotFoundError:
    return None
  if result.returncode != 0:
    return None
  return parse_datetime(result.stdout.strip())

def datetime_to_file_time(media_datetime):
  month = media_datetime.strftime('%b')
  return (
    [media_datetime.strftime('%a'), month, str(media_datetime.day), media_datetime.strftime('%H:%M:%S'), str(media_datetime.year)],
    [media_datetime.strftime('%H'), media_datetime.strftime('%M'), media_datetime.strftime('%S')],
  )

def get_media_creation_datetime(file_path, video_file):
  if video_file:
    sony_metadata = parse_sony_xml_metadata(get_sony_sidecar_path(file_path))
    if sony_metadata.get('creation_datetime'):
      return sony_metadata['creation_datetime'], sony_metadata
    video_datetime = get_video_creation_datetime(file_path)
    if video_datetime:
      local_datetime = video_datetime.astimezone() if video_datetime.tzinfo else video_datetime
      return local_datetime, sony_metadata
    return None, sony_metadata
  return None, {}

def get_file_description(file_path, src_name, video_file, image_file, audio_file):
  file_data = dict()
  media_datetime, media_metadata = get_media_creation_datetime(file_path, video_file)
  if media_datetime:
    ts, tds = datetime_to_file_time(media_datetime)
    file_data['media_datetime'] = media_datetime
  else:
    ts, tds = get_file_modified_time(file_path)
  file_data['media_metadata'] = media_metadata
  if video_file:
    duration, file_duration_string = get_video_file_duration(file_path)
    file_data['duration'] = duration
    file_data['duration_string'] = file_duration_string
  if audio_file:
    audio_data = get_audio_file_description(file_path, src_name)
    file_data = {**file_data, **audio_data}
  file_data['date'] = ts
  file_data['time'] = tds
  return file_data

def date_string_from_file_data(file_data):
  return '{}{}{}-{}{}{}'.format(
    file_data['date'][4][-2:],
    file_data['date'][1].upper(),
    file_data['date'][2].zfill(2),
    file_data['time'][0],
    file_data['time'][1],
    file_data['time'][2],
  )

def format_setfile_datetime(media_datetime):
  return media_datetime.strftime('%m/%d/%Y %H:%M:%S')

def set_file_dates(file_path, file_data=None, source_path=None):
  if file_data and file_data.get('media_datetime'):
    timestamp = file_data['media_datetime'].timestamp()
    os.utime(file_path, (timestamp, timestamp))
    setfile_date = format_setfile_datetime(file_data['media_datetime'])
  elif source_path:
    shutil.copystat(source_path, file_path)
    try:
      birth_timestamp = os.stat(source_path).st_birthtime
      birth_datetime = datetime.datetime.fromtimestamp(birth_timestamp)
      setfile_date = format_setfile_datetime(birth_datetime)
    except AttributeError:
      setfile_date = ''
  else:
    setfile_date = ''

  if setfile_date and shutil.which('SetFile'):
    subprocess.run(['SetFile', '-d', setfile_date, file_path], check=False)

def unique_destination_path(final_path):
  if not path_exists(final_path):
    return final_path
  file_dir = os.path.dirname(final_path)
  file_name = os.path.basename(final_path)
  file_root, file_ext = os.path.splitext(file_name)
  counter = 2
  while True:
    candidate = join_path(file_dir, '{}-{:02d}{}'.format(file_root, counter, file_ext))
    if not path_exists(candidate):
      return candidate
    counter += 1

def move_preserving_metadata(source_path, final_path, file_data=None):
  final_path = unique_destination_path(final_path)
  shutil.copy2(source_path, final_path)
  set_file_dates(final_path, file_data, source_path)
  os.remove(source_path)
  return final_path

def copy_preserving_metadata(source_path, final_path, file_data=None):
  final_path = unique_destination_path(final_path)
  shutil.copy2(source_path, final_path)
  set_file_dates(final_path, file_data, source_path)
  return final_path

def save_sony_sidecar(source_path, final_path, file_data, remove_source=True):
  sidecar_path = get_sony_sidecar_path(source_path)
  if not sidecar_path:
    return
  sidecar_dir = join_path(FINAL_METADATA_DIR, 'sony', file_data['year'], file_data['month'])
  if not path_exists(sidecar_dir):
    os.makedirs(sidecar_dir)
  sidecar_dest = join_path(sidecar_dir, os.path.splitext(os.path.basename(final_path))[0] + '.XML')
  sidecar_dest = unique_destination_path(sidecar_dest)
  shutil.copy2(sidecar_path, sidecar_dest)
  set_file_dates(sidecar_dest, file_data, sidecar_path)
  if remove_source:
    os.remove(sidecar_path)

def is_sony_m4root_clip(source_path):
  normalized_path = source_path.replace(os.sep, '/').lower()
  return '/private/m4root/clip/' in normalized_path

def is_sony_avchd_stream(source_path):
  normalized_path = source_path.replace(os.sep, '/').lower()
  return '/private/avchd/bdmv/stream/' in normalized_path

def is_sony_camera_support_file(source_path, media_file):
  normalized_path = source_path.replace(os.sep, '/').lower()
  file_extension = media_file.split('.')[-1].lower()
  if '/avf_info/' in normalized_path:
    return True
  if '/private/m4root/thmbnl/' in normalized_path:
    return True
  if normalized_path.endswith('/private/m4root/mediapro.xml'):
    return True
  if normalized_path.endswith('/private/m4root/status.bin'):
    return True
  if '/private/avchd/bdmv/' in normalized_path and file_extension in {'bdm', 'cpi', 'mpl'}:
    return True
  return False

def is_ignored_dir(dir_path):
  return os.path.basename(dir_path) in IGNORED_DIR_NAMES

def should_preserve_source_after_save(effective_src_name):
  return effective_src_name.upper() == 'SONYA6400'

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
    ds[:] = [d for d in ds if not is_ignored_dir(join_path(r, d))]
    for media_file in fs:
      effective_src_name = src_name
      copy_file = False
      move_file = False
      delete_file = False
      video_file = False
      image_file = False
      video_360_file = False
      image_360_file = False
      audio_file = False
      corrupt_file = False
      source_path = join_path(r, media_file)
      media_file_parts = media_file.split('.')
      file_extension = media_file_parts[len(media_file_parts)-1].lower()

      if is_sony_camera_support_file(source_path, media_file):
        logger.info('leaving camera support file: {}\n'.format(source_path))
        continue

      if 'png' == file_extension or 'jpg' == file_extension or 'jpeg' == file_extension or 'cr3' == file_extension or 'cr2' == file_extension or 'dng' == file_extension or 'heic' == file_extension or 'gif' == file_extension or 'nef' == file_extension:
        image_file = True
      if 'mov' == file_extension or 'mp4' == file_extension or 'mpg' == file_extension or 'mts' == file_extension:
        video_file = True
      if 'insv' == file_extension or 'lrv' == file_extension or 'thm' == file_extension:
        video_360_file = True
      if 'insp' == file_extension:
        image_360_file = True
      if 'wav' == file_extension or 'mp3' == file_extension:
        audio_file = True
      if 'dat' == file_extension:
        corrupt_file = True
      # delete low res files
      if 'hprj' == file_extension or 'bmp' == file_extension or 'gyro' == file_extension or 'aae' == file_extension:
        delete_file = True

      if not path_exists(source_path):
        continue
      if delete_file:
        logger.info('deleting: {}\n'.format(source_path))
        os.remove(source_path)
        continue

      file_data = get_file_description(source_path, src_name, video_file or video_360_file, image_file or image_360_file, audio_file)
      
      if 'GOP' in media_file[:3] or 'GP' in media_file[:3] or 'G0' in media_file[:2]:
        effective_src_name = 'GOPRO'
        if int(file_data['date'][4]) < 2018:
          file_data['date'], file_data['time'] = update_file_time(file_data['date'], file_data['time'])
      if is_sony_m4root_clip(source_path) and video_file:
        effective_src_name = 'SONYA6400'
      if is_sony_avchd_stream(source_path) and video_file:
        effective_src_name = 'SONYA6400'
      
      file_data['day'] = file_data['date'][2]
      file_data['month'] = MONTH_STRING_NUMBERS[file_data['date'][1]]
      file_data['year'] = file_data['date'][4]

      file_date_string = date_string_from_file_data(file_data)
      new_file_name = '{}_{}'.format(file_date_string, effective_src_name.upper())

      if corrupt_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        final_dir = join_path(FINAL_CORRUPT_DIR,file_data['year'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if image_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        if 'screenshot' in source_path.lower() or 'screen shot' in source_path.lower():
          final_dir = join_path(FINAL_SCREENS_DIR,file_data['year'],file_data['month'])
        elif'cr3' == file_extension or 'cr2' == file_extension or 'nef' == file_extension:
          final_dir = join_path(FINAL_RAW_DIR,file_data['year'],file_data['month'])
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
        if 'screenrecording' in source_path.lower() or 'screen recording' in source_path.lower():
          final_dir = join_path(FINAL_SCREENR_DIR,file_data['year'],file_data['month'])
        if 'mavic2' in source_path.lower():
          final_dir = join_path(FINAL_DRON_DIR,file_data['year'],file_data['month'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if video_360_file:
        new_file_name += '_{}'.format(file_data['duration_string'])
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        final_dir = join_path(FINAL_360_VID_DIR,file_data['year'],file_data['month'])
        if not path_exists(final_dir): 
          os.makedirs(final_dir)
        final_path = join_path(final_dir, new_file_name)
        move_file = True

      if image_360_file:
        new_file_name += '_{}-{}[].{}'.format(country_code.upper(), city_name.upper(), file_extension.upper())
        final_dir = join_path(FINAL_360_IMG_DIR,file_data['year'],file_data['month'])
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
        if src_name.lower() == 'hijack':
          category_dir = FINAL_HIJACK_DIR
          backup_category_dir = SSD_HIJACK_DIR
          if file_data['duration'] > 600:
            category_dir = FINAL_LONG_HIJACK_DIR
            backup_category_dir = SSD_LONG_HIJACK_DIR
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
        shutil.copy2(source_path, backup_path)
        set_file_dates(backup_path, file_data, source_path)
      if move_file:
        preserve_source = should_preserve_source_after_save(effective_src_name)
        if preserve_source:
          logger.info('copying to: {}\n'.format(final_path))
          final_path = copy_preserving_metadata(source_path, final_path, file_data)
        else:
          logger.info('moving to: {}\n'.format(final_path))
          final_path = move_preserving_metadata(source_path, final_path, file_data)
        if is_sony_m4root_clip(source_path) and video_file:
          save_sony_sidecar(source_path, final_path, file_data, remove_source=not preserve_source)
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
  elif src_dir == 'hijack':
    src_name = 'hijack'
    src_dir = AUDIO_HIJACK_DIR
    save_tag_files(src_dir, src_name, country_code, city_name)
  else:
    if len(sys.argv) != 6:
      logger.error(help_error)
      return

    src_name = sys.argv[5]
    save_tag_files(src_dir, src_name, country_code, city_name)


def get_source_service(download_url):
  service = ''
  if 'youtube' in download_url or 'youtu.be' in download_url:
    service = 'youtube'
  elif 'soundcloud' in download_url:
    service = 'soundcloud'
  elif 'spotify' in download_url:
    service = 'spotify'
  elif 'bandcamp' in download_url:
    service = 'bandcamp'
  elif 'mixcloud' in download_url:
    service = 'mixcloud'
  elif 'vimeo' in download_url:
    service = 'vimeo'
  elif 'twitch' in download_url:
    service = 'twitch'
  elif 'facebook' in download_url:
    service = 'facebook'
  elif 'instagram' in download_url:
    service = 'instagram'
  elif 'twitter' in download_url or 't.co' in download_url or 'twimg' in download_url or 'x.co' in download_url:
    service = 'x'
  elif 'tiktok' in download_url:
    service = 'tiktok'
  return service

def download_file(download_url, source_service, file_type, client_id='', auth_token=''):
  # download file
  # save to file_type
  download_cmd = ''
  logger.info(f"downloading {download_url} from {source_service} to {file_type}s")

  if source_service == 'youtube':
    # use yt-dlp to download
    if file_type == 'song' or file_type == 'sample':
      download_cmd = f'yt-dlp -x --audio-format mp3 {download_url} --cookies-from-browser BRAVE'
    elif file_type == 'video':
      download_cmd = f'yt-dlp -f bestvideo[ext=mp4]+bestaudio[ext=m4a]/mp4 {download_url}'
    else:
      logger.error(f"file type {file_type} not supported")
  elif source_service == 'soundcloud':
    # use scdl to download
    client_id = 'sQHBwYwmzeqpmKktQSeKYpDpE1YsCSWl'
    auth_token = '2-302616-6841298-jMTsUUMTCpWvycEd'
    download_cmd = f'scdl -l {download_url} --client-id={client_id} --auth-token={auth_token}'
  elif source_service == 'spotify':
    # use spotify-dl to download
    logger.info('missing, should implement')
  elif source_service == 'bandcamp':
    # use bandcamp-dl to download
    logger.info('missing, should implement')
  elif source_service == 'mixcloud':
    # use mixcloud-dl to download
    logger.info('missing, should implement')
  elif source_service == 'vimeo':
    # use vimeo-dl to download
    logger.info('missing, should implement')
  elif source_service == 'twitch':
    # use twitch-dl to download
    logger.info('missing, should implement')
  elif source_service == 'facebook':
    # use facebook-dl to download
    logger.info('missing, should implement')
  elif source_service == 'instagram':
    # use instagram-dl to download
    logger.info('missing, should implement')
  elif source_service == 'x':
    # use x-dl to download
    logger.info('missing, should implement')
  elif source_service == 'tiktok':
    # use tiktok-dl to download
    logger.info('missing, should implement')
  if not download_cmd:
    return ""
    # process = subprocess.Popen(cmd_list)
    # print(process)
    # # , stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    # process.wait()

  cmd_list = download_cmd.split(' ')
  # Run the command and capture the output
  process = subprocess.Popen(cmd_list, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
  stdout, stderr = process.communicate()
  file_name = ''
  # print("return code:", process.returncode)
  # print(stdout)
  # print(stderr)
  if process.returncode == 0:
      # Parse the output to find the downloaded file name
      for line in stderr.splitlines():
          # if scdl
          if "Downloaded" in line:  # Adjust based on scdl output format
              file_name = line.split(" Downloaded.")[0]
              file_name = re.sub(r'\x1b\[[0-9;]*[mGK]', '', file_name)
              file_name = file_name.strip()
              logger.info(f"Downloaded file: {file_name}")
              break
      for line in stdout.splitlines():
          if "[ExtractAudio] Destination:" in line:
              file_name = line.split("[ExtractAudio] Destination: ")[1]
              file_name = re.sub(r'\x1b\[[0-9;]*[mGK]', '', file_name)
              file_name = file_name.strip()
              logger.info(f"Downloaded file: {file_name}")
              break
          if "[Merger] Merging formats into" in line:
              # extract file_name from template
              # [Merger] Merging formats into "{file_name}"
              file_name = re.search(r'\"(.+?)\"', line).group(1)
              break
  else:
      print(f"Error: {stderr}")
  # get downloaded file name from cmd output
  # print process output
  # print(process.stdout.read())
  return file_name

def move_to_file_type_dir(file_type, file_name):
  file_path = ''
  src_path = file_name
  if file_type == 'sample':
    dest_path = join_path(AUDIO_SAMPLES_DIR, file_name)
    logger.info(f"moving {src_path} to {dest_path}")
    shutil.move(src_path, dest_path)
  elif file_type == 'song':
    dest_path = join_path(AUDIO_SONGS_DIR, file_name)
    logger.info(f"moving {src_path} to {dest_path}")
    shutil.move(src_path, dest_path)
  elif file_type == 'video':
    dest_path = join_path(TMP_VIDEO_DIR, file_name)
    logger.info(f"moving {src_path} to {dest_path}")
    shutil.move(src_path, dest_path)
  return file_path

# cmd line interface
def downloader():
  # nu9ve download [video|song|sample] [link]
  #   [video|song|sample] - type of file to download

  # get_external_storage
  help_error = '''nu9ve download [video|song|sample] [link]\n\t
        [video|song|sample] - type of file to download (where to save)\n\t
        link - url to download'''

  if len(sys.argv) < 4:
    logger.error(help_error)
    return

  # check if necessary directories exist if not create
  if not path_exists(AUDIO_SAMPLES_DIR):
    os.makedirs(AUDIO_SAMPLES_DIR)
  if not path_exists(AUDIO_SPLITS_DIR):
    os.makedirs(AUDIO_SPLITS_DIR)
  if not path_exists(AUDIO_SONGS_DIR):
    os.makedirs(AUDIO_SONGS_DIR)
  if not path_exists(TMP_VIDEO_DIR):
    os.makedirs(TMP_VIDEO_DIR)

  file_type = sys.argv[2]
  download_url = sys.argv[3]
  # set it or scrape it
  # client_id = sys.argv[4]
  # auth_token = sys.argv[5]

  source_service = get_source_service(download_url)
  
  file_name = download_file(download_url, source_service, file_type)
  
  if not file_name:
    logger.error(f"failed to download {download_url}")
    return

  move_to_file_type_dir(file_type, file_name)

def split_file(file_path):
  # run demucs on file via subprocess
  cmd_list = ['demucs', file_path]
  # Run the command and capture the output
  process = subprocess.Popen(cmd_list, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
  stdout, stderr = process.communicate()
  # print("return code:", process.returncode)
  # print(stdout)
  # print(stderr)
  if process.returncode == 0:
      # print(stdout)
      logger.info(f"splitted file: {file_path}")
  else:
      print(f"Error: {stderr}")
  
def splitter():
  help_error = '''nu9ve split [link]\n\t
        [link] - file to split'''

  if len(sys.argv) != 3:
    logger.error(help_error)
    return

  if not path_exists(AUDIO_SPLITS_DIR):
    os.makedirs(AUDIO_SPLITS_DIR)

  file_url = sys.argv[2]

  current_dir = os.getcwd()
  os.chdir(AUDIO_SPLITS_DIR)

  if 'http' not in file_url:
    file_path = join_path(current_dir, file_url)
  else:
    source_service = get_source_service(file_url)
    file_path = download_file(file_url, source_service, 'song')


  # rename file to normalized name
  file_path_split = file_path.split('/')
  file_name = file_path_split[-1]
  file_dir = file_path_split[:-1]
  file_name = file_name.replace(' ', '')
  # Only_Now_-_Eyes_of_Pain_[4be7e1cc4RI]
  if '[' in file_name:
    file_parts = file_name.split('[')
    tmp_file_name = file_parts[0]
  else:
    tmp_file_name = file_name
  tmp_file_name = tmp_file_name.replace(' ', '')
  tmp_file_name = tmp_file_name.replace('_', '')
  if '[' in file_name:
    file_name = tmp_file_name + '[' + file_parts[1]
  else:
    file_name = tmp_file_name
  normal_file_path = join_path("/".join(file_dir), file_name)
  shutil.move(file_path, normal_file_path)
  logger.info(f"renamed {file_path} to {normal_file_path}")
  logger.info(f"splitting {normal_file_path}")

  split_file(normal_file_path)

  shutil.move(normal_file_path, join_path(AUDIO_SPLITS_DIR, 'splitted', file_name))

  os.chdir(current_dir)
