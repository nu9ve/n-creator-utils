
import sys
import json
from inspect import getmembers, isfunction
import subprocess


# detect input dimensions
# vertical to vertical, horizontal, square
# horizontal to vertical, horizontal, square

def read_from_txt():
	pass

def read_from_json():
	with open('config.json') as f:
		d = json.load(f)
		return d['clips']

def time_component_str(t):
	if t < 10:
		return '0'+str(t)
	return str(t)

def get_time_delta_string(clip):
	hh, mm ,ss = get_times(clip['start'])
	hhf, mmf ,ssf = get_times(clip['end'])
	deltas =  [hhf-hh, mmf-mm, ssf-ss]
	delta = ':'.join(map(time_component_str, deltas))
	return delta

def cut_range(video_path, clip, output_path):
	start = clip['start']
	end = clip['ffmpeg_end']
	# ffmpeg -ss 00:00:30.0 -i input.wmv -c copy -t 00:00:10.0 output.wmv
	cut_cmd = ['ffmpeg', '-ss', start, '-i', video_path, '-c', 
			   'copy', '-t', end, output_path]
	print(' '.join(cut_cmd))
	subprocess.run(cut_cmd)

def print_video_info(video_path):
	# ffmpeg -ss 00:00:30.0 -i input.wmv -c copy -t 00:00:10.0 output.wmv
	# output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(count))
	probe_cmd = [ 'ffprobe', '-v', 'error', '-select_streams', 'v:0', 
				  '-show_entries', 'stream=width,height,r_frame_rate', '-of', 'csv=s=x:p=0', video_path]
	# print(' '.join(probe_cmd))
	result = subprocess.run(probe_cmd, capture_output=True)
	dims = str(result.stdout).split('x')
	v_w = dims[0].split("'")[1]
	v_h = dims[1].split("'")[0].replace('\\n','')
	v_r = float(v_w)/float(v_h)
	v_f = dims[2].replace("\\n'", '')
	v_tf = v_f.split('/')
	v_f = float(v_tf[0])/float(v_tf[1])
	video_file = video_path.split('/')[len(video_path.split('/'))-1]
	print()
	print(video_file)
	print('w:{} h:{}'.format(v_w,v_h))
	print('ratio: {}'.format(v_r))
	print('fps: {}'.format(v_f))
	print()

def get_times(time_string):
	ts = time_string.split(':')
	return int(ts[0]), int(ts[1]), int(ts[2])

def horizontal_to_vertical(video_path):
	output_path = video_path.replace('.mp4', '_vertical.mp4')
	# fill_cmd = [ "ffmpeg", "-i", video_path, "-lavfi", 
	# 			 # "'[0:v]scale=ih*16/9:-1,boxblur=luma_radius=min(h\,w)/20:luma_power=1:chroma_radius=min(cw\,ch)/20:chroma_power=1[bg];[bg][0:v]overlay=(W-w)/2:(H-h)/2,crop=h=iw*9/16'", 
	# 			 "'[0:v]scale=ih*16/9:-1'",#,boxblur=luma_radius=min(h\,w)/20:luma_power=1:chroma_radius=min(cw\,ch)/20:chroma_power=1[bg];[bg][0:v]overlay=(W-w)/2:(H-h)/2,crop=h=iw*9/16'", 
	# 			 "-vb", "800K", output_path]

	fill_cmd = [
		"ffmpeg", "-i", 
		video_path, "-vf", 
		# "'split[original][copy];[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale=1280:2282,gblur=sigma=20[blurred];[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2'",
		"split[original][copy];[copy]crop=ih*9/16:ih:iw/2-ow/2:0,scale=1280:2282,gblur=sigma=20[blurred];[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
		# "'scale=1280:2282'",
		# "gblur=sigma=20",#;[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2'",
		# "'split", "[original][copy];", "[copy]", "crop=ih*9/16:ih:iw/2-ow/2:0,", "scale=1280:2282,", "gblur=sigma=20[blurred];" "[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2'",
		output_path

	]
	subprocess.run(fill_cmd)



# main
# clipper.py video.mp4 config.json
if len(sys.argv) == 2:
	print('ok')
	video_path = sys.argv[1]
	print(video_path)
	clips = read_from_json()
	for i, c in enumerate(clips):
		end_string = get_time_delta_string(c)
		c['ffmpeg_end'] = end_string
		print(video_path)
		output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(i))
		print(video_path)
		print_video_info(video_path)
		cut_range(video_path, c, output_path)
		horizontal_to_vertical(output_path)

	# check video exists
	# print video info
	# read timestamps
else:
	print('nel')


# 00:00:10 - 00:00:10 titulo del clip
# 00:10 - 00:10 titulo del clip 2

# 1920x1080

# // # 00:00:10 - 00:00:10 titulo del clip
# // # 00:10 - 00:10 titulo del clip 2



