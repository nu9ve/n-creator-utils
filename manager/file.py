import os, time, sys
import shutil
import logging
from os.path import join as join_path, exists as path_exists
import subprocess
import re
import datetime
import xml.etree.ElementTree as ET
import json
import argparse
from pathlib import Path

from constants import (LOGGER_NAME, MONTH_STRING_NUMBERS, MONTH_NUMBER_DAYS,
  SD_SRC_NAME, FINAL_DRON_DIR, FINAL_4K_DIR, FINAL_VIDEOS_DIR, FINAL_IMAGES_DIR, 
  FINAL_SCREENS_DIR, MONTH_NUMBER_STRINGS, SD_ROOT_DIR, FINAL_RECORDINGS_DIR,
  FINAL_LONG_RECORDINGS_DIR, SSD_RECORDINGS_DIR, SSD_LONG_RECORDINGS_DIR,
  FINAL_LONG_HIJACK_DIR, FINAL_HIJACK_DIR, SSD_HIJACK_DIR, SSD_LONG_HIJACK_DIR,
  FINAL_CORRUPT_DIR, FINAL_RAW_DIR, FINAL_SCREENR_DIR, AUDIO_HIJACK_DIR,
  FINAL_360_VID_DIR, FINAL_360_IMG_DIR, AUDIO_SAMPLES_DIR, AUDIO_SPLITS_DIR, 
  AUDIO_SONGS_DIR, TMP_VIDEO_DIR, FINAL_METADATA_DIR, FINAL_INSTA360_DIR,
  ROOT_DIR, ROOT_RECORDINGS_DIR, YT_DLP_COOKIES_BROWSER)
from manager.video import is_4k, get_video_file_duration
from manager.audio import get_audio_file_description


logger = logging.getLogger(LOGGER_NAME)

IGNORED_DIR_NAMES = {
  '.fseventsd',
  '.Spotlight-V100',
  '.Trashes',
  'System Volume Information',
}

INSTA360_EXTENSIONS = {'insv', 'lrv', 'thm', 'insp'}
AUTO_SRC_NAME = 'AUTO'
SONY_SUPPORT_PATHS = [
  ('AVF_INFO',),
  ('PRIVATE', 'M4ROOT', 'THMBNL'),
  ('PRIVATE', 'M4ROOT', 'SUB'),
  ('PRIVATE', 'M4ROOT', 'GENERAL'),
  ('PRIVATE', 'M4ROOT', 'MEDIAPRO.XML'),
  ('PRIVATE', 'M4ROOT', 'STATUS.BIN'),
  ('PRIVATE', 'AVCHD', 'BDMV', 'INDEX.BDM'),
  ('PRIVATE', 'AVCHD', 'BDMV', 'MOVIEOBJ.BDM'),
  ('PRIVATE', 'AVCHD', 'BDMV', 'PLAYLIST'),
  ('PRIVATE', 'AVCHD', 'BDMV', 'CLIPINF'),
]

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
    try:
      shutil.copystat(source_path, file_path)
    except PermissionError as error:
      logger.warning('could not copy full file flags from {} to {}: {}'.format(source_path, file_path, error))
      source_stat = os.stat(source_path)
      os.utime(file_path, (source_stat.st_atime, source_stat.st_mtime))
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
  verify_copy_size(source_path, final_path)
  os.remove(source_path)
  return final_path

def copy_preserving_metadata(source_path, final_path, file_data=None):
  final_path = unique_destination_path(final_path)
  shutil.copy2(source_path, final_path)
  set_file_dates(final_path, file_data, source_path)
  verify_copy_size(source_path, final_path)
  return final_path

def verify_copy_size(source_path, final_path):
  if os.path.getsize(source_path) != os.path.getsize(final_path):
    raise OSError('Copied size differs; source preserved: {}'.format(source_path))

def move_exact_preserving_metadata(source_path, final_path):
  shutil.copyfile(source_path, final_path)
  set_file_dates(final_path, None, source_path)
  verify_copy_size(source_path, final_path)
  try:
    os.remove(source_path)
  except PermissionError as error:
    logger.warning('could not remove source after copy {}: {}'.format(source_path, error))
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
  verify_copy_size(sidecar_path, sidecar_dest)
  if remove_source:
    os.remove(sidecar_path)

def is_sony_m4root_clip(source_path):
  normalized_path = source_path.replace(os.sep, '/').lower()
  return '/private/m4root/clip/' in normalized_path

def is_sony_avchd_stream(source_path):
  normalized_path = source_path.replace(os.sep, '/').lower()
  return '/private/avchd/bdmv/stream/' in normalized_path

def has_sony_camera_structure(src_dir):
  sony_paths = [
    join_path(src_dir, 'PRIVATE', 'M4ROOT', 'CLIP'),
    join_path(src_dir, 'PRIVATE', 'M4ROOT', 'THMBNL'),
    join_path(src_dir, 'PRIVATE', 'AVCHD', 'BDMV', 'STREAM'),
    join_path(src_dir, 'AVF_INFO'),
  ]
  return any(path_exists(sony_path) for sony_path in sony_paths)

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
  return False

def is_insta360_source(src_name):
  return src_name.upper() == 'INSTA360'

def is_auto_source(src_name):
  return src_name.upper() == AUTO_SRC_NAME

def get_file_extension(file_path):
  return os.path.splitext(file_path)[1].lower().lstrip('.')

def sanitize_path_tag(value):
  clean_value = re.sub(r'[^A-Za-z0-9]+', '-', value.strip().upper()).strip('-')
  return clean_value or 'UNTAGGED'

def iter_importable_files(src_dir):
  for root, dirs, files in os.walk(src_dir):
    dirs[:] = [d for d in dirs if not is_ignored_dir(join_path(root, d))]
    for file_name in files:
      yield join_path(root, file_name)

def get_insta360_import_datetime(src_dir):
  media_times = []
  for source_path in sorted(iter_importable_files(src_dir)):
    if get_file_extension(source_path) in INSTA360_EXTENSIONS:
      media_times.append(os.path.getmtime(source_path))
  if media_times:
    return datetime.datetime.fromtimestamp(min(media_times))
  return datetime.datetime.now()

def has_insta360_media(src_dir):
  for source_path in iter_importable_files(src_dir):
    if get_file_extension(source_path) in INSTA360_EXTENSIONS:
      return True
  return False

def has_insta360_camera_structure(src_dir):
  insta360_paths = [
    join_path(src_dir, 'DCIM', 'fileinfo_list.list'),
  ]
  return any(path_exists(insta360_path) for insta360_path in insta360_paths) or has_insta360_media(src_dir)

def detect_sd_source_name(src_dir):
  if has_sony_camera_structure(src_dir):
    return 'SONYA6400'
  if has_insta360_camera_structure(src_dir):
    return 'INSTA360'
  return ''

def resolve_src_name(src_dir, configured_src_name):
  detected_src_name = detect_sd_source_name(src_dir)
  if detected_src_name:
    if not is_auto_source(configured_src_name) and configured_src_name.upper() != detected_src_name:
      logger.info('detected {} for {}; overriding configured {}'.format(detected_src_name, src_dir, configured_src_name))
    return detected_src_name
  if is_auto_source(configured_src_name):
    logger.info('skipping {}: could not auto-detect camera profile\n'.format(src_dir))
    return ''
  return configured_src_name

def build_insta360_import_dir(src_dir, country_code, city_name, src_name):
  import_datetime = get_insta360_import_datetime(src_dir)
  import_root = join_path(
    FINAL_INSTA360_DIR,
    import_datetime.strftime('%Y'),
    import_datetime.strftime('%m'),
  )
  folder_name = '{}_{}-{}_{}'.format(
    import_datetime.strftime('%d%m-%H%M%S'),
    sanitize_path_tag(country_code),
    sanitize_path_tag(city_name),
    sanitize_path_tag(src_name),
  )
  return unique_destination_path(join_path(import_root, folder_name)), import_datetime

def save_insta360_bundle(src_dir, src_name, country_code, city_name):
  if not has_insta360_media(src_dir):
    logger.info('no Insta360 media found in: {}\n'.format(src_dir))
    return

  final_dir, import_datetime = build_insta360_import_dir(src_dir, country_code, city_name, src_name)
  logger.info('moving Insta360 bundle to: {}'.format(final_dir))
  os.makedirs(final_dir)

  manifest = {
    'source_dir': src_dir,
    'source_name': src_name,
    'country': country_code,
    'city': city_name,
    'imported_at': datetime.datetime.now().isoformat(timespec='seconds'),
    'media_date': import_datetime.isoformat(timespec='seconds'),
    'files': [],
  }

  for source_path in sorted(iter_importable_files(src_dir)):
    relative_path = os.path.relpath(source_path, src_dir)
    final_path = join_path(final_dir, relative_path)
    final_parent = os.path.dirname(final_path)
    if not path_exists(final_parent):
      os.makedirs(final_parent)
    file_record = {
      'source': source_path,
      'relative_path': relative_path,
      'size': os.path.getsize(source_path),
      'extension': get_file_extension(source_path),
    }
    move_exact_preserving_metadata(source_path, final_path)
    file_record['destination'] = final_path
    manifest['files'].append(file_record)

  manifest_path = unique_destination_path(join_path(final_dir, 'manifest.json'))
  with open(manifest_path, 'w') as manifest_file:
    json.dump(manifest, manifest_file, indent=2, sort_keys=True)
    manifest_file.write('\n')

  logger.info('moved {} files to: {}\n'.format(len(manifest['files']), final_dir))

def remove_empty_dirs(dir_path):
  for root, dirs, files in os.walk(dir_path, topdown=False):
    if root == dir_path:
      continue
    if files:
      continue
    try:
      os.rmdir(root)
    except OSError:
      pass

def build_sony_support_dir(file_data, country_code, city_name):
  support_root = join_path(
    FINAL_METADATA_DIR,
    'sony',
    'support',
    file_data['year'],
    file_data['month'],
  )
  folder_name = '{}_{}-{}_SONYA6400'.format(
    '{}{}-{}{}{}'.format(
      file_data['day'].zfill(2),
      file_data['month'],
      file_data['time'][0],
      file_data['time'][1],
      file_data['time'][2],
    ),
    sanitize_path_tag(country_code),
    sanitize_path_tag(city_name),
  )
  return unique_destination_path(join_path(support_root, folder_name))

def archive_path_contents(source_path, source_root, final_root, manifest):
  if os.path.isfile(source_path):
    relative_path = os.path.relpath(source_path, source_root)
    final_path = join_path(final_root, relative_path)
    final_parent = os.path.dirname(final_path)
    if not path_exists(final_parent):
      os.makedirs(final_parent)
    file_record = {
      'source': source_path,
      'relative_path': relative_path,
      'size': os.path.getsize(source_path),
      'extension': get_file_extension(source_path),
    }
    move_exact_preserving_metadata(source_path, final_path)
    file_record['destination'] = final_path
    manifest['files'].append(file_record)
    return

  for root, dirs, files in os.walk(source_path):
    dirs[:] = [d for d in dirs if not is_ignored_dir(join_path(root, d))]
    for file_name in sorted(files):
      child_path = join_path(root, file_name)
      relative_path = os.path.relpath(child_path, source_root)
      final_path = join_path(final_root, relative_path)
      final_parent = os.path.dirname(final_path)
      if not path_exists(final_parent):
        os.makedirs(final_parent)
      file_record = {
        'source': child_path,
        'relative_path': relative_path,
        'size': os.path.getsize(child_path),
        'extension': get_file_extension(child_path),
      }
      move_exact_preserving_metadata(child_path, final_path)
      file_record['destination'] = final_path
      manifest['files'].append(file_record)

def archive_sony_support_files(src_dir, file_data, country_code, city_name):
  final_dir = build_sony_support_dir(file_data, country_code, city_name)
  manifest = {
    'source_dir': src_dir,
    'source_name': 'SONYA6400',
    'country': country_code,
    'city': city_name,
    'archived_at': datetime.datetime.now().isoformat(timespec='seconds'),
    'media_date': file_data.get('media_datetime', '').isoformat(timespec='seconds') if file_data.get('media_datetime') else '',
    'files': [],
  }

  for relative_parts in SONY_SUPPORT_PATHS:
    source_path = join_path(src_dir, *relative_parts)
    if path_exists(source_path):
      if not path_exists(final_dir):
        os.makedirs(final_dir)
      archive_path_contents(source_path, src_dir, final_dir, manifest)

  if not manifest['files']:
    return

  manifest_path = unique_destination_path(join_path(final_dir, 'manifest.json'))
  with open(manifest_path, 'w') as manifest_file:
    json.dump(manifest, manifest_file, indent=2, sort_keys=True)
    manifest_file.write('\n')

  logger.info('archived {} Sony support files to: {}\n'.format(len(manifest['files']), final_dir))

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
  for tag in (src_name, country_code, city_name):
    if not tag.strip() or '/' in tag or '\\' in tag or tag in {'.', '..'}:
      raise ValueError('Source, country and city must be nonempty tags without path separators')
  source = Path(src_dir).resolve()
  if not source.is_dir():
    raise ValueError('Source directory does not exist: {}'.format(source))
  # Never create an absent archive root: an unmounted disk must not redirect an
  # import onto the system disk. Subdirectories are still created as before.
  if not Path(ROOT_DIR).is_dir():
    raise ValueError('Archive root must already exist: {}'.format(ROOT_DIR))
  for destination in (ROOT_DIR, ROOT_RECORDINGS_DIR):
    target = Path(destination).resolve()
    if source == target or source in target.parents or target in source.parents:
      raise ValueError('Source and archive roots must not overlap: {}'.format(source))
  if is_insta360_source(src_name):
    save_insta360_bundle(src_dir, src_name, country_code, city_name)
    return

  sony_moved_count = 0
  sony_file_data = None

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

      if 'png' == file_extension or 'jpg' == file_extension or 'jpeg' == file_extension or 'cr3' == file_extension or 'cr2' == file_extension or 'dng' == file_extension or 'heic' == file_extension or 'gif' == file_extension or 'nef' == file_extension or 'arw' == file_extension:
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
        elif'cr3' == file_extension or 'cr2' == file_extension or 'nef' == file_extension or 'arw' == file_extension:
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
        if not Path(ROOT_RECORDINGS_DIR).is_dir():
          raise ValueError('Audio backup root must already exist: {}'.format(ROOT_RECORDINGS_DIR))
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
        copy_preserving_metadata(source_path, backup_path, file_data)
      if move_file:
        preserve_source = should_preserve_source_after_save(effective_src_name)
        if preserve_source:
          logger.info('copying to: {}\n'.format(final_path))
          final_path = copy_preserving_metadata(source_path, final_path, file_data)
        else:
          logger.info('moving to: {}\n'.format(final_path))
          final_path = move_preserving_metadata(source_path, final_path, file_data)
          if effective_src_name.upper() == 'SONYA6400':
            sony_moved_count += 1
            sony_file_data = file_data
        if is_sony_m4root_clip(source_path) and video_file:
          save_sony_sidecar(source_path, final_path, file_data, remove_source=not preserve_source)
      if not delete_file and not move_file:
        logger.info('no action: {}\n'.format(source_path))

  if src_name.upper() == 'SONYA6400' and sony_moved_count > 0 and sony_file_data:
    archive_sony_support_files(src_dir, sony_file_data, country_code, city_name)


# cmd line interface
def save_files(argv=None):
  parser = argparse.ArgumentParser(description='Importa media; mueve archivos y elimina originales tras copiar.')
  parser.add_argument('source', help='directorio, all o hijack')
  parser.add_argument('country')
  parser.add_argument('city')
  parser.add_argument('source_name', nargs='?', help='perfil/etiqueta; obligatorio para directorios')
  args = parser.parse_args(sys.argv[2:] if argv is None else argv)
  src_dir, country_code, city_name = args.source, args.country, args.city
  if src_dir not in {'all', 'hijack'} and not args.source_name:
    parser.error('source_name es obligatorio para un directorio (puede ser AUTO)')

  if src_dir == 'all':
    # look for sds
    for sd in SD_SRC_NAME:
      src_path = join_path(SD_ROOT_DIR, sd)
      if path_exists(src_path):
        src_name = resolve_src_name(src_path, SD_SRC_NAME[sd])
        if not src_name:
          continue
        logger.info('{} found as {}. Saving...'.format(src_path, src_name))
        save_tag_files(src_path, src_name, country_code, city_name)
  elif src_dir == 'hijack':
    src_name = 'hijack'
    src_dir = AUDIO_HIJACK_DIR
    save_tag_files(src_dir, src_name, country_code, city_name)
  else:
    src_name = args.source_name
    if not Path(src_dir).is_dir():
      raise ValueError('Source directory does not exist: {}'.format(src_dir))
    if is_auto_source(src_name):
      src_name = resolve_src_name(src_dir, src_name)
      if not src_name:
        return
    save_tag_files(src_dir, src_name, country_code, city_name)


def get_source_service(download_url):
  from urllib.parse import urlparse
  host = (urlparse(download_url).hostname or '').lower()
  services = {
    'youtube.com': 'youtube', 'youtu.be': 'youtube', 'soundcloud.com': 'soundcloud',
    'spotify.com': 'spotify', 'bandcamp.com': 'bandcamp', 'mixcloud.com': 'mixcloud',
    'vimeo.com': 'vimeo', 'twitch.tv': 'twitch', 'facebook.com': 'facebook',
    'instagram.com': 'instagram', 'twitter.com': 'x', 'x.com': 'x', 't.co': 'x',
    'tiktok.com': 'tiktok',
  }
  for domain, service in services.items():
    if host == domain or host.endswith('.' + domain):
      return service
  return ''


def download_file(download_url, source_service, file_type, client_id='', auth_token='', cwd=None):
  if file_type not in {'video', 'song', 'sample'}:
    raise ValueError('Unsupported download type: {}'.format(file_type))
  logger.info('downloading from %s to %ss', source_service, file_type)
  if source_service == 'youtube':
    command = ['yt-dlp', '--print', 'after_move:filepath']
    if file_type in {'song', 'sample'}:
      command += ['-x', '--audio-format', 'mp3']
      if YT_DLP_COOKIES_BROWSER:
        command += ['--cookies-from-browser', YT_DLP_COOKIES_BROWSER]
    else:
      command += ['-f', 'bestvideo[ext=mp4]+bestaudio[ext=m4a]/mp4']
    command += ['--', download_url]
  elif source_service == 'soundcloud':
    command = ['scdl', '-l', download_url]
    client_id = client_id or os.environ.get('SOUNDCLOUD_CLIENT_ID', '')
    auth_token = auth_token or os.environ.get('SOUNDCLOUD_AUTH_TOKEN', '')
    if client_id:
      command += ['--client-id', client_id]
    if auth_token:
      command += ['--auth-token', auth_token]
  else:
    raise ValueError('Download service not implemented: {}'.format(source_service or 'unknown'))

  result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
  if result.returncode:
    # Do not echo command arguments or stderr: provider diagnostics may contain tokens.
    raise RuntimeError('{} failed (exit {})'.format(command[0], result.returncode))
  file_name = ''
  if source_service == 'youtube':
    lines = result.stdout.strip().splitlines()
    if lines:
      file_name = lines[-1].strip()
  else:
    for line in (result.stderr + '\n' + result.stdout).splitlines():
      if ' Downloaded.' in line:
        file_name = re.sub(r'\x1b\[[0-9;]*[mGK]', '', line.split(' Downloaded.')[0]).strip()
        break
  if not file_name:
    raise RuntimeError('Downloader did not report a file; inspect its working directory')
  path = Path(file_name)
  if not path.is_absolute():
    path = Path(cwd or os.getcwd()) / path
  if not path.is_file():
    raise RuntimeError('Downloaded file not found: {}'.format(path))
  return str(path)


def move_to_file_type_dir(file_type, file_name):
  destinations = {'sample': AUDIO_SAMPLES_DIR, 'song': AUDIO_SONGS_DIR, 'video': TMP_VIDEO_DIR}
  if file_type not in destinations:
    raise ValueError('Unsupported download type: {}'.format(file_type))
  final_dir = destinations[file_type]
  os.makedirs(final_dir, exist_ok=True)
  destination = unique_destination_path(join_path(final_dir, os.path.basename(file_name)))
  if Path(file_name).resolve() != Path(destination).resolve():
    shutil.move(file_name, destination)
  return destination


def downloader(argv=None):
  parser = argparse.ArgumentParser(description='Descarga desde YouTube o SoundCloud.')
  parser.add_argument('file_type', choices=['video', 'song', 'sample'])
  parser.add_argument('url')
  args = parser.parse_args(sys.argv[2:] if argv is None else argv)
  file_name = download_file(args.url, get_source_service(args.url), args.file_type)
  return move_to_file_type_dir(args.file_type, file_name)


def split_file(file_path):
  result = subprocess.run(['demucs', file_path], cwd=AUDIO_SPLITS_DIR)
  if result.returncode:
    raise RuntimeError('demucs failed (exit {})'.format(result.returncode))
  logger.info('split file: %s', file_path)


def splitter(argv=None):
  parser = argparse.ArgumentParser(description='Separa stems con Demucs desde un archivo o URL.')
  parser.add_argument('source')
  args = parser.parse_args(sys.argv[2:] if argv is None else argv)
  os.makedirs(AUDIO_SPLITS_DIR, exist_ok=True)
  from urllib.parse import urlparse
  if urlparse(args.source).scheme in {'http', 'https'}:
    file_path = download_file(args.source, get_source_service(args.source), 'song', cwd=AUDIO_SPLITS_DIR)
  else:
    file_path = str(Path(args.source).expanduser().resolve())
  if not Path(file_path).is_file():
    raise ValueError('Audio file does not exist: {}'.format(file_path))

  # Preserve the historical normalization, without replacing an existing file.
  name = Path(file_path).name.replace(' ', '')
  prefix, separator, suffix = name.partition('[')
  name = prefix.replace('_', '') + separator + suffix
  normal_file_path = str(Path(file_path).with_name(name))
  if Path(file_path) != Path(normal_file_path):
    normal_file_path = unique_destination_path(normal_file_path)
    shutil.move(file_path, normal_file_path)
  logger.info('splitting %s', normal_file_path)
  split_file(normal_file_path)
  final_dir = join_path(AUDIO_SPLITS_DIR, 'splitted')
  os.makedirs(final_dir, exist_ok=True)
  destination = unique_destination_path(join_path(final_dir, Path(normal_file_path).name))
  shutil.move(normal_file_path, destination)
  return destination
