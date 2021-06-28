
import sys
import json
import logging
import os
import copy
from os.path import exists as path_exists, join as join_path
from inspect import getmembers, isfunction
import subprocess
import pathlib

import datetime as dt

# pathlib.Path(__file__).parent.absolute()

# pathlib.Path().absolute()

from constants import LOGGER_NAME, DEBUG
from clipper.ffmpeg import (cut_range, render_portrait_video,
	render_landscape_video, render_square_video, render_filters_texts)


logger = logging.getLogger(LOGGER_NAME)

# detect input dimensions
# vertical to vertical, horizontal, square
# horizontal to vertical, horizontal, square

def read_from_txt():
	pass

def read_from_json(file_path):
  # if file not json and not exists return empty clips
  clips_data = []
  with open(file_path) as f:
    d = json.load(f)
    default_clip_output = dict()
    default_clip_output['texts'] = d.get('texts', [])
    if d.get('mode', False):
      default_clip_output['mode'] = d.get('mode', '#101010')	
    if d.get('view', False):
      default_clip_output['view'] = d.get('view', 90)

    for i, c in enumerate(d['clips']):
      clip_data = copy.deepcopy(c)
      if 'mode' not in c:
        clip_data['mode'] = d.get('mode', default_clip_output['mode'])
      if 'view' not in c:
        clip_data['view'] = d.get('view', default_clip_output['view'])
      
      if 'texts' in c:
        clip_data['texts'] = default_clip_output['texts'] + c['texts']
      else:
        clip_data['texts'] = default_clip_output['texts']

      clips_data.append(clip_data)
  return clips_data


def time_component_str(t):
	if t < 10:
		return '0'+str(t)
	return str(t)


def get_times(time_string):
	ts = time_string.split(':')
	return int(ts[0]), int(ts[1]), int(ts[2])


def get_time_delta_string(clip):
	hh, mm ,ss = get_times(clip['start'])
	hhf, mmf ,ssf = get_times(clip['end'])
	enter = dt.time(hh, mm, ss)
	exit = dt.time(hhf, mmf, ssf)
	deltat = dt.datetime.combine(dt.date.min, exit) - dt.datetime.combine(dt.date.min, enter)
	delta = str(deltat)
	return delta


def get_video_info(video_path):
	probe_cmd = [ 'ffprobe', '-v', 'error', '-select_streams', 'v:0', 
				  '-show_entries', 'stream=width,height,r_frame_rate', '-of', 'csv=s=x:p=0', video_path]
	result = subprocess.run(probe_cmd, capture_output=True)
	dims = str(result.stdout).split('x')
	v_f = dims[2].replace("\\n'", '')
	v_tf = v_f.split('/')
	video_data = dict()
	video_data['width'] = dims[0].split("'")[1]
	video_data['height'] = dims[1].split("'")[0].replace('\\n','')
	video_data['ratio'] = float(video_data['width'])/float(video_data['height'])
	video_data['frame_rate'] = float(v_tf[0])/float(v_tf[1])
	video_data['file_name'] = video_path.split('/')[len(video_path.split('/'))-1]
	video_data['format'] = video_path.split('.')[len(video_path.split('.'))-1]
	video_data['full_path'] = video_path
	video_data['path'] = video_path
	return video_data


def parse_clip_data(video_data, clip):
	clip_data = dict()
	clip_data['width'] = video_data['width']
	clip_data['height'] = video_data['height']
	clip_data['ratio'] = video_data['ratio']
	clip_data['frame_rate'] = video_data['frame_rate']
	clip_data['file_name'] = video_data['file_name']
	clip_data['format'] = video_data['format']
	clip_data['full_path'] = video_data['full_path']
	clip_data['path'] = video_data['path']

	clip_data['start'] = clip['start']
	clip_data['ffmpeg_end'] = get_time_delta_string(clip)
	clip_data['mode'] = clip.get('mode', '#1F1F1F')
	clip_data['valid'] = True

	clip_data['texts'] = clip['texts']
	return clip_data

def clip_video(video_path, clips):
	logger.info(video_path)
	video_data = get_video_info(video_path)
	message = '''{}
	w: {} h:{}
	ratio: {}
	fps: {}\n'''.format(video_data['full_path'], video_data['width'], video_data['height'], video_data['ratio'], video_data['frame_rate'])
	logger.info(message)
	is_landscape = video_data['ratio'] > 1

	for i, c in enumerate(clips):
		output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(i))
		clip_data = parse_clip_data(video_data, c)

		if not clip_data['valid']:
			invalid_error = '''clip {} config error\n'''.format(i)
			logger.error(invalid_error)
		cut_range(video_path, clip_data, output_path)
		clip_data['is_landscape'] = is_landscape

		if is_landscape:
			render_portrait_video(output_path, clip_data)
		else:
			render_landscape_video(output_path, clip_data)
		render_square_video(output_path, clip_data)
		render_filters_texts(output_path, clip_data)




		# if is_landscape:
		# 	cut_output_path = output_path.replace('.mp4', '_horizontal.mp4')
		# 	render_portrait_video(output_path, clip_data)

			# clip_data['text'] = 'LEL, TIENE QUE SER ASI\nY A VER QP'
			# clip_data['text'] = clip_data['title']
			# clip_data['mode'] = 'crop_center'
			# render_portrait_video(output_path, clip_data)
			# clip_data['mode'] = 'blurred'
			# clip_data['view'] = 100
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 90
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 50
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 0
			# render_portrait_video(output_path, clip_data)

			# clip_data['mode'] = '#FF00AA'
			# clip_data['view'] = 100
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 90
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 50
			# render_portrait_video(output_path, clip_data)
			# clip_data['view'] = 0
			# render_portrait_video(output_path, clip_data)


		# else:
		# 	cut_output_path = output_path.replace('.mp4', '_vertical.mp4')
		# 	render_landscape_video(output_path, clip_data)


			# clip_data['text'] = 'LEL, TIENE QUE SER ASI\nY A VER QP'
			# clip_data['text'] = clip_data['title']
			# clip_data['mode'] = 'crop_center'
			# render_landscape_video(output_path, clip_data)
			# clip_data['mode'] = 'blurred'
			# clip_data['view'] = 100
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 90
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 50
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 0
			# render_landscape_video(output_path, clip_data)

			# clip_data['mode'] = '#FF00AA'
			# clip_data['view'] = 100
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 90
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 50
			# render_landscape_video(output_path, clip_data)
			# clip_data['view'] = 0
			# render_landscape_video(output_path, clip_data)

		# clip_data['mode'] = '#AA2288'
		# clip_data['mode'] = 'blurred'
		# clip_data['view'] = 0
		# render_square_video(output_path, clip_data)
		# clip_data['view'] = 50
		# render_square_video(output_path, clip_data)
		# clip_data['view'] = 90
		# render_square_video(output_path, clip_data)

		# Pending 34, vertical is fine
		# clip_data['mode'] = '#22CCCC'
		# clip_data['view'] = 0
		# render_34_video(output_path, clip_data)
		# clip_data['mode'] = 'crop_center'
		# render_34_video(output_path, clip_data)
		# clip_data['mode'] = 'blurred'
		# render_34_video(output_path, clip_data)
		# rename_first_cut(output_path, cut_output_path)
		
# 00:00:10 - 00:00:10 titulo del clip
# 00:10 - 00:10 titulo del clip 2

# 1920x1080

# // # 00:00:10 - 00:00:10 titulo del clip
# // # 00:10 - 00:10 titulo del clip 2


# cmd line interface
def clipper():
  help_error = '''nu9ve clipper src [config]\n\t
        all - saves configured sd card names\n'''

	# clipper.py video.mp4 [config.json|.txt]
  if len(sys.argv) != 4 and len(sys.argv) != 3:
    logger.error(help_error)
    return

  video_path = sys.argv[2]
  # logger.info(video_path)
  video_root = pathlib.Path(video_path).parent.absolute()
  # logger.info(video_root)

  #default on script folder
  config_file = None
  #search config in video root
  config_video_path = join_path(video_root, 'config.json')
  if path_exists(config_video_path):
    config_file = config_video_path
  #parameter
  elif len(sys.argv) == 4:
    config_file = str(sys.argv[3])
    
  if not config_file:
    logger.error('config file not found')
    return
  file_ext = video_path.split('.')[len(video_path.split('.'))-1].lower()
  if file_ext != 'mp4' and file_ext != 'mov':
    logger.error(f'{file_ext} not supported. [mp4, mov]')
    return
  clips = read_from_json(config_file)
  clip_video(video_path, clips)



