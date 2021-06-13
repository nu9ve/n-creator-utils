import logging
import subprocess
import pathlib
import copy
import math

from constants import LOGGER_NAME, DEBUG


logger = logging.getLogger(LOGGER_NAME)

# ffmpeg -i main.mkv -i facecloseup.mkv
# -filter_complex "[1]trim=end_frame=1,
#  geq='st(3,pow(X-(W/2),2)+pow(Y-(H/2),2));if(lte(ld(3),pow(min(W/2,H/2),2)),255,0)':128:128,
#  loop=-1:1,setpts=N/FRAME_RATE/TB[mask];
#  [1][mask]alphamerge[cutout];
#  [0][cutout]overlay=x=W-w:y=0[v];
#  [0][1]amix=2[a]"
# -map "[v]" -map "[a]"  out.mp4


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

def build_portrait_filter(video_data, clip_filter):
	text_string = None
	mode = video_data['mode']
	mode_view = video_data.get('view', 0)
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


def build_landscape_filter(video_data, clip_filter):
	text_string = None
	mode = video_data['mode']
	mode_view = video_data.get('view', 0)
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
		scale_factor = (input_height/input_width) / mode_view
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			",crop=iw:{}:0:ih/4".format(input_width), # crop top-bottom
			",pad=2*trunc(ih*16/18):ih:(ow-iw)/2:(oh-ih)/2:{}".format(main_color), # grow bars
		]
	if mode == 'crop_center':
		scale_factor = (input_height/input_width)
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			",crop=iw:{}:0:ih/2-oh/2".format(input_width) # crop top-bottom
		]
	elif mode == 'blurred':
		mode_view = mode_view/100
		mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
		filter_cmd_list = [
			"split[original][copy];",
			# "[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
			"[copy]crop=iw/1.5:iw*9/16/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
			"[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
			"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
			",setsar=1,scale={}:{}".format(input_height, input_width),
		]

	if 'text' in video_data:
		filter_cmd_list.append(",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/Platinum Sign.ttf:text='{}':fontcolor=white:fontsize=32:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
		# filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
	
	flt = ''.join(filter_cmd_list)
	return flt


def build_square_filter(video_data, clip_filter):
	text_string = None
	mode = video_data['mode']
	mode_view = video_data.get('view', 0)
	input_width = float(video_data['width'])
	input_height = float(video_data['height'])
	filter_cmd_list = []

	if mode == 'background':
		main_color = video_data['color']
		# takes 0-100 converts to 3-1 so 0-100 scales
		mode_view = mode_view/100
		if video_data['is_landscape']:
			scale_factor = (input_width/input_height)
			cc_filter = ",crop=ih:ih:iw/2-ow/2:ih/2-oh/2:0"
		else:
			scale_factor = (input_height/input_width)
			cc_filter = ",crop=iw:iw:iw/2-ow/2:ih/2-oh/2:0"
		scale_width = input_width * scale_factor # 1080
		scale_height = input_height * scale_factor # 1918
		mult_pad = 1 + (1 - mode_view)
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			cc_filter,
			",pad={}*iw:{}*ih:(ow-iw)/2:(oh-ih)/2:{}".format(mult_pad, mult_pad, main_color), # grow bars
		]
	if mode == 'crop_center':
		if video_data['is_landscape']:
			scale_factor = (input_width/input_height)
			cc_filter = ",crop=ih:ih:iw/2-ow/2:ih/2-oh/2"
		else:
			scale_factor = (input_height/input_width)
			cc_filter = ",crop=iw:iw:iw/2-ow/2:ih/2-oh/2"
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height), # scale up video
			cc_filter
		]
	elif mode == 'blurred':
		mode_view = mode_view/100
		mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
		input_larger = input_width
		if input_height > input_width:
			input_larger = input_height
		if video_data['is_landscape']:
			# bc_filter = "[copy]crop=iw/1.5:iw*9/16/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width)
			# cc_filter = "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
			bc_filter = "[copy]crop=ih/1.5:ih/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_width, input_width)
			cc_filter = "[original]scale={}:{},crop=ih:ih:iw/2-ow/2:ih/2-oh/2[original];".format(input_width/mode_view, input_height/mode_view)
		else:
			bc_filter = "[copy]crop=iw/1.5:iw/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_height)
			cc_filter = "[original]scale={}:{},crop=iw:iw:iw/2-ow/2:ih/2-oh/2[original];".format(input_width/mode_view, input_height/mode_view)
		filter_cmd_list = [
			"split[original][copy];",
			bc_filter,
			cc_filter,
			"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
			",setsar=1,scale={}:{}".format(input_larger, input_larger),
		]

	if 'text' in video_data:
		filter_cmd_list.append(",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/Platinum Sign.ttf:text='{}':fontcolor=white:fontsize=32:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
		# filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
	
	flt = ''.join(filter_cmd_list)
	return flt


def build_34_filter(video_data, clip_filter):
	text_string = None
	mode = video_data['mode']
	mode_view = video_data.get('view', 0)
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
		if video_data['is_landscape']:
			scale_factor = (input_width/input_height)
			scale_width = input_width * scale_factor
			scale_height = input_height * scale_factor
			scale_filter = "scale={}:{}".format(scale_width, scale_height) # scale up video
			cc_filter = ",crop=ih*0.75:ih:iw/2-ow/2:ih/2-oh/2".format(scale_factor)
		else:
			scale_filter = ""
			# scale_factor = (input_height/input_width)
			cc_filter = "crop=iw:ih*0.75:iw/2-ow/2:ih/2-oh/2" #",crop=iw:ih*0.75:iw/2-ow/2:ih/2-oh/2".format(scale_factor)
		filter_cmd_list = [
			scale_filter,
			cc_filter
		]
	elif mode == 'blurred':
		# mode_view = mode_view/100
		# mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
		# filter_cmd_list = [
		# 	"split[original][copy];",
		# 	"[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
		# 	"[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
		# 	"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
		# 	",setsar=1,scale={}:{}".format(input_height, input_width),
		# ]

		mode_view = mode_view/100
		mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
		input_larger = input_width
		if input_height > input_width:
			input_larger = input_height

		if video_data['is_landscape']:
			# bc_filter = "[copy]crop=iw/1.5:iw*9/16/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width)
			# cc_filter = "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
			bc_filter = "[copy]crop=ih/1.5:ih/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_width, input_width)
			cc_filter = "[original]scale={}:{},crop=ih:ih:iw/2-ow/2:ih/2-oh/2[original];".format(input_width/mode_view, input_height/mode_view)
		else:
			bc_filter = "[copy]crop=iw/1.5:iw/1.5:iw/2-ow/2:ih/2-oh/2,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_height)
			cc_filter = "[original]scale={}:{},crop=iw:iw:iw/2-ow/2:ih/2-oh/2[original];".format(input_width/mode_view, input_height/mode_view)
		filter_cmd_list = [
			"split[original][copy];",
			bc_filter,
			cc_filter,
			"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
			",setsar=1,scale={}:{}".format(input_larger, input_larger),
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
	if 'view' in clip_data:
		output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_portrait_filter(clip_data, format_data)
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
	if 'view' in clip_data:
		output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_landscape_filter(clip_data, format_data)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}horizontal to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)


def render_square_video(video_path, video_data):
	output_path = video_path.replace('.{}'.format(video_data['format']), '_square.{}'.format(video_data['format']))
	# filters = dict()
	format_data = video_data.get('square', dict())
	clip_data = copy.deepcopy(video_data)
	video_mode = clip_data['mode']
	if video_mode != 'crop_center' and video_mode != 'blurred':
		video_mode = 'background'
		clip_data['color'] = clip_data['mode']
		clip_data['mode'] =  video_mode
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(video_mode, clip_data['format']))
	if 'view' in clip_data:
		output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_square_filter(clip_data, format_data)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}square to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)


def render_34_video(video_path, video_data):
	output_path = video_path.replace('.{}'.format(video_data['format']), '_34.{}'.format(video_data['format']))
	# filters = dict()
	format_data = video_data.get('34', dict())
	clip_data = copy.deepcopy(video_data)
	video_mode = clip_data['mode']
	if video_mode != 'crop_center' and video_mode != 'blurred':
		video_mode = 'background'
		clip_data['color'] = clip_data['mode']
		clip_data['mode'] =  video_mode
	output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(video_mode, clip_data['format']))
	if 'view' in clip_data:
		output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
	flt_cmd = build_34_filter(clip_data, format_data)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}3:4 to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)