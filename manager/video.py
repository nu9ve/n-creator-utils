import subprocess
import logging

from utils.constants import LOGGER_NAME


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


