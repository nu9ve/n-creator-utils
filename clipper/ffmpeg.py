import logging
import subprocess
import pathlib
import copy
import math

from constants import LOGGER_NAME, DEBUG


logger = logging.getLogger(LOGGER_NAME)


def run_ffmpeg_cmd(cmd_list):
	if DEBUG:
		cmd_string = ' '.join(cmd_list)
		debug_cmd_log = ''' Run:
			{}\n'''.format(cmd_string)
		logger.debug(debug_cmd_log)
	else:
		subprocess.run(cmd_list)


def cut_range(video_path, clip, output_path):
	start = clip['start']
	end = clip['ffmpeg_end']
	# ffmpeg -ss 00:00:30.0 -i input.wmv -c copy -t 00:00:10.0 output.wmv
	cut_cmd = ['ffmpeg', '-ss', start, '-i', video_path, '-c', 
			   'copy', '-t', end, output_path]
	clipping_log = '{}clipping to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(cut_cmd)

def build_filter(video_data, clip_filter):
	text_string = None
	mode = video_data['mode']
	mode_view = video_data['view']
	input_width = float(video_data['width'])
	input_height = float(video_data['height'])

	filter_cmd_list = [
		"pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(mode), # grow black bars
		",setsar=1", # make pixels square
		",scale={}:{}".format(input_height, input_width) # invert size
	]

	if mode == 'background':
		main_color = video_data['color']
		# takes 0-100 converts to 3-1 so 0-100 scales
		mode_view = mode_view/100
		mode_view = ((1 - mode_view)/.3) # = (0 - 3.33)
		mode_view = (math.sqrt(mode_view + 1) * 1.99) - .99
		scale_factor = (input_width/input_height) / mode_view
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			",crop={}:ih:iw/4:0".format(input_height), # crop sides
			",pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(main_color), # grow bars
			# ",setsar=1,scale={}:{}".format(input_height, input_width) # make pxls square and invert size
		]
	if mode == 'crop_center':
		scale_factor = (input_width/input_height)
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			",crop={}:ih:iw/4:0".format(input_height) # crop sides
		]
	elif mode == 'blurred':
		mode_view = mode_view/100
		mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
		filter_cmd_list = [
			"split[original][copy];",
			"[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
			"[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
			"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
			",setsar=1,scale={}:{}".format(input_height, input_width),
		]

	if 'text' in video_data:
		filter_cmd_list.append(",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/Platinum Sign.ttf:text='{}':fontcolor=white:fontsize=32:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
		# filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
	
	flt = ''.join(filter_cmd_list)
	return flt

def render_portrait_video(video_path, video_data):
	output_path = video_path.replace('.{}'.format(video_data['format']), '_vertical.{}'.format(video_data['format']))
	# filters = dict()
	format_data = video_data.get('vertical', dict())
	clip_data = copy.deepcopy(video_data)
	video_mode = clip_data['mode']
	if video_mode != 'crop_center' and video_mode != 'blurred':
		video_mode = 'background'
		clip_data['color'] = clip_data['mode']
		clip_data['mode'] =  video_mode
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(video_mode, clip_data['format']))
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_filter(clip_data, format_data)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}vertical to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)


def render_landscape_video(video_path, video_data):
	output_path = video_path.replace('.{}'.format(video_data['format']), '_horizontal.{}'.format(video_data['format']))
	# filters = dict()
	format_data = video_data.get('horizontal', dict())
	clip_data = copy.deepcopy(video_data)
	video_mode = clip_data['mode']
	if video_mode != 'crop_center' and video_mode != 'blurred':
		video_mode = 'background'
		clip_data['color'] = clip_data['mode']
		clip_data['mode'] =  video_mode
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(video_mode, clip_data['format']))
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_filter(clip_data, format_data)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}horizontal to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)

	# output_path = video_path.replace('.mp4', '_horizontal.mp4')
	# mode = video_data['mode']
	# text_string = None
	# input_width = float(video_data['width'])
	# input_height = float(video_data['height'])
	# # filters = dict()
	# filter_cmd_list = [
	# 	# "pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{},setsar=1".format(background)
	# 	"pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(mode),
	# 	",setsar=1",
	# 	",scale={}:{}".format(input_height, input_width)
	# ]
	# if mode == 'crop_center':
	# 	scale_factor = (input_width/input_height)
	# 	scale_width = input_width * scale_factor
	# 	scale_height = input_height * scale_factor
	# 	filter_cmd_list = [
	# 		"scale={}:{}".format(scale_width, scale_height),
	# 		",crop={}:ih:iw/4:0".format(input_height)
	# 	]
	# elif mode == 'blurred':
	# 	filter_cmd_list = [
	# 		"split[original][copy];",
	# 		# "[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
	# 		"[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
	# 		"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
	# 		",setsar=1,scale={}:{}".format(input_height, input_width),
	# 	]

	# if text_string:
	# 	filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(text_string))		
	# flt_cmd = ''.join(filter_cmd_list)
	# fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	# clipping_log = '{}vertical to:{} {}'.format('\033[1m', '\033[0m', output_path)
	# logger.info(clipping_log)
	# run_ffmpeg_cmd(fill_cmd)


def render_square_video(video_path, video_data):
	output_path = video_path.replace('.mp4', '_square.mp4')
	# ffmpeg -i main.mkv -i facecloseup.mkv
 # -filter_complex "[1]trim=end_frame=1,
 #  geq='st(3,pow(X-(W/2),2)+pow(Y-(H/2),2));if(lte(ld(3),pow(min(W/2,H/2),2)),255,0)':128:128,
 #  loop=-1:1,setpts=N/FRAME_RATE/TB[mask];
 #  [1][mask]alphamerge[cutout];
 #  [0][cutout]overlay=x=W-w:y=0[v];
 #  [0][1]amix=2[a]"
 # -map "[v]" -map "[a]"  out.mp4
	filter_cmd = "split[original][copy];[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale=1280:2282,gblur=sigma=20[blurred];[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2"
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", filter_cmd, output_path]
	clipping_log = '{}square to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)

def render_34_video(video_path, video_data):
	output_path = video_path.replace('.mp4', '_34.mp4')
	filter_cmd = "split[original][copy];[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale=1280:2282,gblur=sigma=20[blurred];[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2"
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", filter_cmd, output_path]
	clipping_log = '{}3:4 to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)