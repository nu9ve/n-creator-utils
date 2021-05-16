
import sys
import json
import logging
import os
from os.path import exists as path_exists, join as join_path
from inspect import getmembers, isfunction
import subprocess
import pathlib
# pathlib.Path(__file__).parent.absolute()

# pathlib.Path().absolute()

from constants import LOGGER_NAME, DEBUG
from clipper.ffmpeg import cut_range, render_portrait_video, render_landscape_video, render_square_video, render_34_video


logger = logging.getLogger(LOGGER_NAME)

# detect input dimensions
# vertical to vertical, horizontal, square
# horizontal to vertical, horizontal, square

def read_from_txt():
	pass

def read_from_json(file_path):
	with open(file_path) as f:
		d = json.load(f)
		default_clip_output = dict()
		default_clip_output['mode'] = '#101010'
		for i, c in enumerate(d['clips']):
			if 'mode' not in c:
				d['clips'][i]['mode'] = d.get('mode', default_clip_output['mode'])
		return d['clips']


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
	deltas =  [hhf-hh, mmf-mm, ssf-ss]
	delta = ':'.join(map(time_component_str, deltas))
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
	video_data['full_path'] = video_path
	video_data['path'] = video_path
	return video_data
	

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
		end_string = get_time_delta_string(c)
		c['ffmpeg_end'] = end_string
		output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(i))
		video_data['mode'] = c.get('mode','blurred')
		cut_range(video_path, c, output_path)
		if is_landscape:
			cut_output_path = output_path.replace('.mp4', '_horizontal.mp4')
			render_portrait_video(output_path, video_data)
		else:
			cut_output_path = output_path.replace('.mp4', '_vertical.mp4')
			render_landscape_video(output_path, video_data)
		render_square_video(output_path, video_data)
		render_34_video(output_path, video_data)
		if DEBUG:
			op_parts = output_path.split('/')
			cop_parts = cut_output_path.split('/')
			num_parts = len(op_parts)
			logger.debug('rename {} to {}'.format(op_parts[num_parts-1], cop_parts[num_parts-1]))
		else:
			os.rename(output_path, cut_output_path)



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
  clips = read_from_json(config_file)
  clip_video(video_path, clips)



