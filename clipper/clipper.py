
import sys
import json
from inspect import getmembers, isfunction
import subprocess


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

def cut_range(video_path, start, end, count):
	# ffmpeg -ss 00:00:30.0 -i input.wmv -c copy -t 00:00:10.0 output.wmv
	output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(count))
	cut_cmd = ['ffmpeg', '-ss', start, '-i', video_path, '-c', 'copy', '-t', end, output_path]
	print(cut_cmd)
	subprocess.run(cut_cmd)

def get_times(time_string):
	ts = time_string.split(':')
	return int(ts[0]), int(ts[1]), int(ts[2])


# main
# clipper.py video.mp4 config.json
if len(sys.argv) == 2:
	print('ok')
	video_path = sys.argv[1]
	print(video_path)
	clips = read_from_json()
	for i, c in enumerate(clips):
		end_string = get_time_delta_string(c)
		cut_range(video_path, c['start'], end_string, i)
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



