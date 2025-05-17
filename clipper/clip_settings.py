import sys
import json
import logging
import os
import copy
from os.path import exists as path_exists, join as join_path
from inspect import getmembers, isfunction
import subprocess
import pathlib
import argparse
import datetime as dt



def time_component_str(t):
	if t < 10:
		return '0'+str(t)
	return str(t)


def get_times(time_string):
  ts = time_string.split(':')
  ssms = ts[2].split('.')
  ss = ssms[0]
  if len(ssms) == 1:
    ms = '00'
  else:
    ms = ssms[1]
  return int(ts[0]), int(ts[1]), int(ss), int(ms)


def get_time_delta_string(clip):
	hh, mm ,ss, ms = get_times(clip['start'])
	hhf, mmf ,ssf, msf = get_times(clip['end'])
	enter = dt.time(hh, mm, ss, ms)
	exit = dt.time(hhf, mmf, ssf, msf)
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

  clip_data['start'] = str(clip['start'])
  # if type(clip['start']) == float or type(clip['start']) == int:
  #   clip_data['start'] = str(clip['start'])
  # else:
  #   clip_data['start'] = get_time_delta_string(clip)
  if type(clip['end']) == float or type(clip['end']) == int or '.' in clip['end']:
    clip_data['ffmpeg_end'] = str(clip['end'])
  else:
    clip_data['ffmpeg_end'] = get_time_delta_string(clip)
  clip_data['mode'] = clip.get('mode', '#1F1F1F')
  clip_data['valid'] = True
  
  if 'vertical_only' in clip:
    clip_data['vertical_only'] = clip['vertical_only']
  else:
    clip_data['vertical_only'] = False
  
  if 'view' in clip:
    clip_data['view'] = clip['view']
  else:
    clip_data['view'] = 90
  
  clip_data['texts'] = clip['texts']
  clip_data['inputs'] = clip['inputs']
  clip_data['input_types'] = clip['input_types']
  clip_data['background'] = clip['background']
  return clip_data


def read_from_json(file_path, video_path):
    # if file not json and not exists return empty clips
    clips_data = []
    with open(file_path) as f:
      d = json.load(f)
      default_clip_output = dict()
      
      default_clip_output['inputs'] = [video_path]
      default_clip_output['input_types'] = []

      default_clip_output['text_size'] = d.get('text_size', 3)
      default_clip_output['texts'] = d.get('texts', [])
      default_clip_output['vertical_only'] = d.get('vertical_only', True)

      if d.get('mode', False):
        default_clip_output['mode'] = d.get('mode', '#101010')	
      if d.get('view', False):
        default_clip_output['view'] = d.get('view', 90)
      if d.get('logo', False):
        default_clip_output['logo'] = d.get('logo')
        default_clip_output['inputs'].append(d.get('logo'))
        default_clip_output['input_types'].append('logo')
      if d.get('background', False):
        default_clip_output['mode'] = d.get('background', '#101010')
        default_clip_output['background'] = d.get('background')
        default_clip_output['inputs'].append(d.get('background'))
        default_clip_output['input_types'].append('background')

      # if d.get('logo', False):
        # default_clip_output['logo'] = d.get('logo') # filepath normalize

      for i, c in enumerate(d['clips']):
        clip_data = copy.deepcopy(c)
        clip_data['inputs'] = default_clip_output['inputs']
        clip_data['input_types'] = default_clip_output['input_types']
        if 'vertical_only' not in c:
          clip_data['vertical_only'] = default_clip_output['vertical_only']
        if 'mode' not in c:
          clip_data['mode'] = d.get('mode', default_clip_output['mode'])
        if 'view' not in c:
          clip_data['view'] = d.get('view', default_clip_output['view'])
        if 'texts' in c:
          clip_data['texts'] = copy.deepcopy(default_clip_output['texts']) + c['texts']
        else:
          clip_data['texts'] = copy.deepcopy(default_clip_output['texts'])
        if 'title' in c:
          # clip_data['view'] = 80
          title = dict(text=c['title'], y=(98-clip_data['view']))
          clip_data['texts'].append(title)
        if 'logo' in default_clip_output:
          clip_data['logo'] = default_clip_output['logo']
        if 'background' in default_clip_output:
          clip_data['background'] = default_clip_output['background']
        for i, t in enumerate(clip_data['texts']):
          # clip_data['']
          clip_data['texts'][i]['text_size'] = default_clip_output['text_size']
        clips_data.append(clip_data)
    return clips_data
