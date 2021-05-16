import logging
import subprocess
import pathlib

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


def render_portrait_video(video_path, video_data):
	output_path = video_path.replace('.mp4', '_vertical.mp4')
	mode = video_data.get('mode','#101010')
	text_string = None
	input_width = float(video_data['width'])
	input_height = float(video_data['height'])
	# filters = dict()
	filter_cmd_list = [
		# "pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{},setsar=1".format(background)
		"pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(mode),
		",setsar=1",
		",scale={}:{}".format(input_height, input_width)
	]
	if mode == 'crop_center':
		scale_factor = (input_width/input_height)
		scale_width = input_width * scale_factor
		scale_height = input_height * scale_factor
		filter_cmd_list = [
			"scale={}:{}".format(scale_width, scale_height),
			",crop={}:ih:iw/4:0".format(input_height)
		]
	elif mode == 'blurred':
		filter_cmd_list = [
			"split[original][copy];",
			# "[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
			"[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
			"[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
			",setsar=1,scale={}:{}".format(input_height, input_width),
		]

	if text_string:
		filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(text_string))		
	flt_cmd = ''.join(filter_cmd_list)
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", flt_cmd, output_path]
	clipping_log = '{}vertical to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)


def render_landscape_video(video_path, video_data):
	output_path = video_path.replace('.mp4', '_horizontal.mp4')
	filter_cmd = "split[original][copy];[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale=1280:2282,gblur=sigma=20[blurred];[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2"
	fill_cmd = [ "ffmpeg", "-i", video_path, "-vf", filter_cmd, output_path]
	clipping_log = '{}horizontal to:{} {}'.format('\033[1m', '\033[0m', output_path)
	logger.info(clipping_log)
	run_ffmpeg_cmd(fill_cmd)


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