import os, time, sys
import wave
import subprocess
import contextlib
import shutil
# from spleeter.separator import Separator


month_string_to_num = {
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
  audio_data['month'] = month_string_to_num[ts[1]]
  audio_data['year'] = ts[4]
  return new_name, audio_data

def get_video_file_description(folder, cam_name, video_file):
  video_data = dict()
  video_file_path = os.path.join(folder, video_file)
  time_string = time.ctime(os.path.getmtime(video_file_path))
  ts = list(filter(None, time_string.split(' ')))
  duration, file_duration_string = get_video_file_duration(video_file_path)
  tds = ts[3].split(':')
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}_{}'.format(file_date_string, cam_name.upper(), file_duration_string)
  video_data['duration'] = duration
  video_data['day'] = ts[2]
  video_data['month'] = month_string_to_num[ts[1]]
  video_data['year'] = ts[4]
  return new_name, video_data

def get_image_file_description(folder, cam_name, video_file):
  video_data = dict()
  video_file_path = os.path.join(folder, video_file)
  time_string = time.ctime(os.path.getmtime(video_file_path))
  ts = list(filter(None, time_string.split(' ')))
  tds = ts[3].split(':')
  file_date_string = '{}{}{}-{}{}{}'.format(ts[4][-2:],ts[1].upper(),ts[2],tds[0],tds[1],tds[2])
  new_name = '{}_{}'.format(file_date_string, cam_name.upper())
  video_data['day'] = ts[2]
  video_data['month'] = month_string_to_num[ts[1]]
  video_data['year'] = ts[4]
  return new_name, video_data

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


def store_video_files():
  # 2019/
  #   month (1-12)
  #     year(19)month(01-12)day(01-31)_micname_duration(mmm-ss)_
  #     191131-2355_H6LR_20-02_JP-TOKYO[optional(place,mood)].WAV
  done_test = False
  final_videos_dir = '/Volumes/HDD4/videos/'
  final_images_dir = '/Volumes/HDD4/images/'
  final_screens_dir = '/Volumes/HDD4/screens/'
  if len(sys.argv) != 6:
    print('erikpyado videos src_dir cam jp tag')
    return
  source_dir = sys.argv[2]
  cam_name = sys.argv[3]
  country_code = sys.argv[4]
  city_name = sys.argv[5]
  for r, ds, fs in os.walk(source_dir):
    # for folder in ds:
      # recording_folder = os.path.join(r, folder)
      # for vr, vds, vfs in os.walk(recording_folder):
    for video_file in fs:
      if '.PNG' == video_file[-4:] or '.png' == video_file[-4:]:
        new_file_name, file_data = get_image_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].png'.format(country_code.upper(), city_name.upper())
        initial_path = os.path.join(source_dir, video_file)
        if 'screenshot' in initial_path or 'Screenshot' in initial_path:
          final_dir = os.path.join(final_screens_dir,file_data['year'],file_data['month'])
        else:
          final_dir = os.path.join(final_images_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = os.path.join(final_dir, new_file_name)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.JPG' == video_file[-4:] or '.jpg' == video_file[-4:]:
        if 'GOP' in video_file[:3] or 'GP' in video_file[:3] or 'G0' in video_file[:2]:
          cam_name = 'GOPRO'
        new_file_name, file_data = get_image_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].jpg'.format(country_code.upper(), city_name.upper())
        initial_path = os.path.join(source_dir, video_file)
        if 'screenshot' in initial_path or 'Screenshot' in initial_path:
          final_dir = os.path.join(final_screens_dir,file_data['year'],file_data['month'])
        else:
          final_dir = os.path.join(final_images_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = os.path.join(final_dir, new_file_name)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.MP4' == video_file[-4:] or '.mp4' == video_file[-4:]:
        if 'GOP' in video_file[:3] or 'GP' in video_file[:3]:
          cam_name = 'GOPRO'
        new_file_name, file_data = get_video_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].mp4'.format(country_code.upper(), city_name.upper())
        # if file_data['duration'] > 600: #10 min
        #   final_dir = os.path.join(long_recordings_dir,file_data['year'],file_data['month'])
        # else:
        final_dir = os.path.join(final_videos_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = os.path.join(final_dir, new_file_name)
        initial_path = os.path.join(source_dir, video_file)
        print(final_path)
        shutil.move(initial_path, final_path)
      if '.MOV' == video_file[-4:] or '.mov' == video_file[-4:]:
        new_file_name, file_data = get_video_file_description(source_dir,cam_name,video_file)
        new_file_name += '_{}-{}[].mov'.format(country_code.upper(), city_name.upper())
        # if file_data['duration'] > 600: #10 min
        #   final_dir = os.path.join(long_recordings_dir,file_data['year'],file_data['month'])
        # else:
        final_dir = os.path.join(final_videos_dir,file_data['year'],file_data['month'])
        if not os.path.exists(final_dir): 
          os.makedirs(final_dir)
        final_path = os.path.join(final_dir, new_file_name)
        initial_path = os.path.join(source_dir, video_file)
        print(final_path)
        shutil.move(initial_path, final_path)


def download_youtube(final_dir):
  youtube_link = sys.argv[2]
  youtube_dl_cmd = 'youtube-dl --extract-audio --audio-format mp3 '+ youtube_link
  os.system(youtube_dl_cmd)
  youtube_name_cmd = 'youtube-dl --get-filename '+ youtube_link
  ytcmdout = subprocess.check_output(youtube_name_cmd, shell=True)
  ytcmdout = ytcmdout.replace(b"'",b"\'")
  outfilename = str(ytcmdout)[2:-8]+'.mp3'
  outfilename = outfilename.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6","[Mukbang] 세상에서 가장 매운 과자 도전 먹방🔥Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6")
  video_name = str(ytcmdout)[2:-20]
  video_name = video_name.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung","[ASMR,Mukbang] Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound")
  final_file_path = os.path.join(final_dir,video_name+'.mp3')
  shutil.move(os.path.join(os.getcwd(),outfilename),final_file_path) 



def download_youtube_song_split_stem():
  youtube_link = sys.argv[2]
  mix_path = '/Volumes/SSD1/mix/000/'
  stems_path = '/Volumes/SSD1/abletonlive/stems/'
  youtube_dl_cmd = 'youtube-dl --extract-audio --audio-format mp3 '+ youtube_link
  os.system(youtube_dl_cmd)
  youtube_name_cmd = 'youtube-dl --get-filename '+ youtube_link
  ytcmdout = subprocess.check_output(youtube_name_cmd, shell=True)
  ytcmdout = ytcmdout.replace(b"'",b"\'")
  # outfilename = str(ytcmdout)[2:-7]+'.mp3'
  outfilename = str(ytcmdout)[2:-8]+'.mp3'

  print()
  print('downloaded filename:',outfilename)
  print('Video name:',ytcmdout)
  print()
  # outfilename = outfilename.replace("\\xe2\\x80\\x93","–")
  # outfilename = outfilename.replace("\\xc3\\xb1","ñ")
  # outfilename = outfilename.replace("\\xc3\\xad","í")
  # outfilename = outfilename.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c \xea\xb0\x80\xec\x9e\xa5 \xeb\xa7\xa4\xec\x9a\xb4 \xea\xb3\xbc\xec\x9e\x90 \xeb\x8f\x84\xec\xa0\x84 \xeb\xa8\xb9\xeb\xb0\xa9\xf0\x9f\x94\xa5','')
  # outfilename = outfilename.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c','')
  outfilename = outfilename.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6","[Mukbang] 세상에서 가장 매운 과자 도전 먹방🔥Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6")
  # video_name = str(ytcmdout)[2:-19]
  video_name = str(ytcmdout)[2:-20]
  video_name = video_name.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung","[ASMR,Mukbang] Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound")
  # video_name = video_name.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c','')
  # video_name = video_name.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c \xea\xb0\x80\xec\x9e\xa5 \xeb\xa7\xa4\xec\x9a\xb4 \xea\xb3\xbc\xec\x9e\x90 \xeb\x8f\x84\xec\xa0\x84 \xeb\xa8\xb9\xeb\xb0\xa9\xf0\x9f\x94\xa5','')

  print()
  print('replaced filename:',outfilename)
  print('replaced Video name:',video_name)
  print()
  final_file_path = os.path.join(mix_path,video_name+'.mp3')
  shutil.move(os.path.join(os.getcwd(),outfilename),final_file_path) 
  print(final_file_path)
  # separator = Separator('spleeter:2stems')
  separator = Separator('spleeter:4stems')
  separator.separate_to_file(final_file_path, stems_path)

def song_split_stem(s):
  file_name = sys.argv[2]
  stems_path = '/Volumes/SSD1/abletonlive/stems/'
  file_path = os.path.join(os.getcwd(),file_name)
  separator = Separator('spleeter:'+str(s)+'stems')
  separator.separate_to_file(file_path, stems_path)


if len(sys.argv) > 2:
  if sys.argv[1] == 'zoom':
    store_audio_files()
  if sys.argv[1] == 'videos':
    store_video_files()
  if sys.argv[1] == 'yt2mix2stems':
    download_youtube_song_split_stem()
  if sys.argv[1] == 'file2stems':
    song_split_stem(2)
  if sys.argv[1] == 'file4stems':
    song_split_stem(4)
  if sys.argv[1] == 'soundfx':
    download_youtube('/Users/erikiado/Movies/sounds/fx')
  # if sys.argv[1] == 'social':
  # if sys.argv[1] == 'audio':
  # if sys.argv[1] == 'visuals':
    

