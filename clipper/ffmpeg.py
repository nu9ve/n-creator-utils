import logging
import subprocess
import shutil
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

def build_text_filter(clip_data, clip_filter):
  cmd_list = []
  y_texts = [] # if texts doesnt include y or sector
  dim_to_size = {
    '1920x1080': 7,
    '1080x1920': 4,
  }
  for ct in clip_data['texts']:
    logger.info(clip_data)
    ct['text'] = ct.get('text', '@@@@@@ @@@ @@@@@@@@')
    ct['y'] = ct.get('y', '25')
    # ct['y'] = ct.get('y', '25')
    text_size = ct.get('text_size', 4)
    font_size = "h*{}/100".format(text_size)
    # if clip_data['is_landscape']:
      # font_size = "w*10/100"
    flt_cmd = "".join([ 
      ",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/coolvetica.ttf:",
      "text='{}':".format(ct['text']),
      # "fontcolor=white:fontsize=32:",
      "fontcolor=black:fontsize={}:".format(font_size),
      # "box=1:boxcolor=black@0.5:boxborderw=5:",
      # "x=(w-text_w)/2:y=((h*3/4)-text_h)/2"])		
      "x=(w-text_w)/2:y=(h*{}/100)-text_h".format(ct['y'])])
    cmd_list.append(flt_cmd)
    cmd_list.append("[m1]")
  return ''.join(cmd_list)
  # if 'bottom_text' in clip_data:
  # 	flt_cmd = "".join([flt_cmd, 
  # 		",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/coolvetica.ttf:",
  # 		"text='{}':".format(clip_data['bottom_text']),
  # 		"fontcolor=white:fontsize=32:",
  # 		"box=1:boxcolor=black@0.5:boxborderw=5:",
  # 		# "x=(w-text_w)/2:y=((h*3/4)-text_h)/2"])		
  # 		"x=(w-text_w)/2:y=(h*3/4)-text_h"])		
  # 	# filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
  # if 'top_text' in clip_data:
  # 	flt_cmd = "".join([flt_cmd, 
  # 		",drawtext=fontfile=/Users/nu9ve/Downloads/test_clipper/coolvetica.ttf:",
  # 		"text='{}':".format(clip_data['top_text']),
  # 		"fontcolor=white:fontsize=32:",
  # 		"box=1:boxcolor=black@0.5:boxborderw=5:",
  # 		# "x=(w-text_w)/2:y=((h*3/4)-text_h)/2"])		
  # 		"x=(w-text_w)/2:y=(h*3/4)-text_h"])		
  # 	# filter_cmd_list.append(",drawtext=fontfile=/path/to/font.ttf:text='{}':fontcolor=white:fontsize=24:box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)/2:y=(h-text_h)/2".format(video_data['text']))		
  
def build_logo_filter(clip_data, clip_filter):
  cmd_list = []
  flt_cmd = "".join([ 
    ";[1]format=rgb24,colorkey=black,colorchannelmixer=aa=0.5,scale=150:150[1d];",
    "[m1][1d]overlay=main_w-overlay_w-20:(2*main_h/3)+(2*overlay_h)"])
    # "[m1][1d]overlay=main_w-overlay_w-20:main_h-overlay_h-20"])
  cmd_list.append(flt_cmd)
  return ''.join(cmd_list)

def build_portrait_filter(video_data, clip_filter):
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
      ",crop={}:ih".format(input_height), # crop sides
      ",pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(main_color), # grow bars
      # ",setsar=1,scale={}:{}".format(input_height, input_width) # make pxls square and invert size
    ]
  if mode == 'crop_center':
    scale_factor = (input_width/input_height)
    scale_width = input_width * scale_factor
    scale_height = input_height * scale_factor
    filter_cmd_list = [
      "scale={}:{}".format(scale_width, scale_height), # scale up video
      ",crop={}:ih".format(input_height) # crop sides
    ]
  elif mode == 'blurred':
    mode_view = mode_view/100
    mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
    filter_cmd_list = [
      "[0]split[original][copy];",
      "[copy]crop=ih*9/16:ih,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
      "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
      "[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
      ",setsar=1,scale={}:{}".format(input_height, input_width),
    ]
  
  flt = ''.join(filter_cmd_list)
  return flt


def build_landscape_filter(video_data, clip_filter):
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
    mode_view = (math.sqrt(mode_view + 1) * 1.99) - .99 # 1 - 3+?
    scale_factor = (input_height/input_width) / mode_view
    scale_width = input_width * scale_factor
    scale_height = input_height * scale_factor
    filter_cmd_list = [
      "scale={}:{}".format(scale_width, input_width), # scale up video
      ",crop=iw:{}".format(input_width),#".format(input_width), # crop top-bottom
      ",pad=2*trunc(ih*16/18):ih:(ow-iw)/2:(oh-ih)/2:{}".format(main_color), # grow bars
    ]
  if mode == 'crop_center':
    scale_factor = (input_height/input_width)
    scale_width = input_width * scale_factor
    scale_height = input_height * scale_factor
    filter_cmd_list = [
      "scale={}:{}".format(scale_width, scale_height), # scale up video
      ",crop=iw:{}".format(input_width) # crop top-bottom
    ]
  elif mode == 'blurred':
    mode_view = mode_view/100
    mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
    filter_cmd_list = [
      "[0]split[original][copy];",
      # "[copy]crop=ih*9/16:ih,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
      "[copy]crop=iw/1.5:iw*9/16/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
      "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
      "[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
      ",setsar=1,scale={}:{}".format(input_height, input_width),
    ]
  
  flt = ''.join(filter_cmd_list)
  return flt


def build_square_filter(video_data, clip_filter):
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
      cc_filter = ",crop=ih:ih:0"
    else:
      scale_factor = (input_height/input_width)
      cc_filter = ",crop=iw:iw:0"
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
      cc_filter = ",crop=ih:ih"
    else:
      scale_factor = (input_height/input_width)
      cc_filter = ",crop=iw:iw"
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
      # bc_filter = "[copy]crop=iw/1.5:iw*9/16/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width)
      # cc_filter = "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
      bc_filter = "[copy]crop=ih/1.5:ih/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_width, input_width)
      cc_filter = "[original]scale={}:{},crop=ih:ih[original];".format(input_width/mode_view, input_height/mode_view)
    else:
      bc_filter = "[copy]crop=iw/1.5:iw/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_height)
      cc_filter = "[original]scale={}:{},crop=iw:iw[original];".format(input_width/mode_view, input_height/mode_view)
    filter_cmd_list = [
      "[0]split[original][copy];",
      bc_filter,
      cc_filter,
      "[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
      ",setsar=1,scale={}:{}".format(input_larger, input_larger),
    ]
  
  flt = ''.join(filter_cmd_list)
  return flt


def build_34_filter(video_data, clip_filter):
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
      ",crop={}:ih".format(input_height), # crop sides
      ",pad=iw:2*trunc(iw*16/18):(ow-iw)/2:(oh-ih)/2:{}".format(main_color), # grow bars
      # ",setsar=1,scale={}:{}".format(input_height, input_width) # make pxls square and invert size
    ]
  if mode == 'crop_center':
    if video_data['is_landscape']:
      scale_factor = (input_width/input_height)
      scale_width = input_width * scale_factor
      scale_height = input_height * scale_factor
      scale_filter = "scale={}:{}".format(scale_width, scale_height) # scale up video
      cc_filter = ",crop=ih*0.75:ih".format(scale_factor)
    else:
      scale_filter = ""
      # scale_factor = (input_height/input_width)
      cc_filter = "crop=iw:ih*0.75" #",crop=iw:ih*0.75".format(scale_factor)
    filter_cmd_list = [
      scale_filter,
      cc_filter
    ]
  elif mode == 'blurred':
    # mode_view = mode_view/100
    # mode_view = 2 - (mode_view * 1.5) # 1.0 to 0.5; 0.5 to 1~; 0 to 2
    # filter_cmd_list = [
    # 	"split[original][copy];",
    # 	"[copy]crop=ih*9/16:ih,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width),
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
      # bc_filter = "[copy]crop=iw/1.5:iw*9/16/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_width)
      # cc_filter = "[original]scale={}:{}[original];".format(input_width/mode_view, input_height/mode_view),
      bc_filter = "[copy]crop=ih/1.5:ih/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_width, input_width)
      cc_filter = "[original]scale={}:{},crop=ih:ih[original];".format(input_width/mode_view, input_height/mode_view)
    else:
      bc_filter = "[copy]crop=iw/1.5:iw/1.5,scale={}:{},gblur=sigma=20[blurred];".format(input_height, input_height)
      cc_filter = "[original]scale={}:{},crop=iw:iw[original];".format(input_width/mode_view, input_height/mode_view)
    filter_cmd_list = [
      "[0]split[original][copy];",
      bc_filter,
      cc_filter,
      "[blurred][original]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2",
      ",setsar=1,scale={}:{}".format(input_larger, input_larger),
    ]

  flt = ''.join(filter_cmd_list)
  return flt

def build_filter_texts(video_data, clip_filter):
  mode = video_data['mode']
  mode_view = video_data.get('view', 0)
  input_width = float(video_data['width'])
  input_height = float(video_data['height'])

  filter_cmd_list = []

  flt = ''.join(filter_cmd_list)
  flt_cmd = ''.join([flt, build_text_filter(video_data, clip_filter)])
  if flt_cmd and flt_cmd[0] == ',':
    flt_cmd = flt_cmd[1:]

  return flt_cmd


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
  flt_cmd = ''.join([flt_cmd, build_text_filter(clip_data, format_data)])

  ff_filter_type = "-vf"
  if len(clip_data['inputs']) > 1:
    flt_cmd = ''.join([flt_cmd, build_logo_filter(clip_data, format_data)])
    ff_filter_type = "-filter_complex"
  inputs = []
  for x in clip_data['inputs']:
    inputs.append('-i')
    inputs.append(x)
  if clip_data.get('preview', False):
    fill_cmd = [ "ffmpeg", "-ss","00:00:01",*inputs, ff_filter_type, flt_cmd, '-vframes', '1', '-q:v','2', output_path.replace('.mp4','.jpg')]
  else:
    fill_cmd = [ "ffmpeg", *inputs, ff_filter_type, flt_cmd, output_path]
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
  flt_cmd = ''.join([flt_cmd, build_text_filter(clip_data, format_data)])
  ff_filter_type = "-vf"
  if len(clip_data['inputs']) > 1:
    flt_cmd = ''.join([flt_cmd, build_logo_filter(clip_data, format_data)])
    ff_filter_type = "-filter_complex"
  inputs = []
  for x in clip_data['inputs']:
    inputs.append('-i')
    inputs.append(x)
  if clip_data.get('preview', False):
    fill_cmd = [ "ffmpeg", "-ss","00:00:01",*inputs, ff_filter_type, flt_cmd, '-vframes', '1', '-q:v','2', output_path.replace('.mp4','.jpg')]
  else:
    fill_cmd = [ "ffmpeg", *inputs, ff_filter_type, flt_cmd, output_path]
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
  flt_cmd = ''.join([flt_cmd, build_text_filter(clip_data, format_data)])
  ff_filter_type = "-vf"
  if len(clip_data['inputs']) > 1:
    flt_cmd = ''.join([flt_cmd, build_logo_filter(clip_data, format_data)])
    ff_filter_type = "-filter_complex"
  inputs = []
  for x in clip_data['inputs']:
    inputs.append('-i')
    inputs.append(x)
  if clip_data.get('preview', False):
    fill_cmd = [ "ffmpeg", "-ss","00:00:01",*inputs, ff_filter_type, flt_cmd, '-vframes', '1', '-q:v','2', output_path.replace('.mp4','.jpg')]
  else:
    fill_cmd = [ "ffmpeg", *inputs, ff_filter_type, flt_cmd, output_path]
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
  flt_cmd = ''.join([flt_cmd, build_text_filter(clip_data, format_data)])
  ff_filter_type = "-vf"
  if len(clip_data['inputs']) > 1:
    flt_cmd = ''.join([flt_cmd, build_logo_filter(clip_data, format_data)])
    ff_filter_type = "-filter_complex"
  inputs = []
  for x in clip_data['inputs']:
    inputs.append('-i')
    inputs.append(x)
  if clip_data.get('preview', False):
    fill_cmd = [ "ffmpeg", "-ss","00:00:01",*inputs, ff_filter_type, flt_cmd, '-vframes', '1', '-q:v','2', output_path.replace('.mp4','.jpg')]
  else:
    fill_cmd = [ "ffmpeg", *inputs, ff_filter_type, flt_cmd, output_path]
  clipping_log = '{}3:4 to:{} {}'.format('\033[1m', '\033[0m', output_path)
  logger.info(clipping_log)
  run_ffmpeg_cmd(fill_cmd)


def render_filters_texts(video_path, clip_data):
# def rename_filter_first_cut(output_path, cut_output_path, clip_data):
  if clip_data['is_landscape']:
    fmt_data = 'horizontal'
  else:
    fmt_data = 'vertical'
  output_path = video_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(fmt_data, clip_data['format']))
  # filters = dict()
  format_data = clip_data.get(fmt_data, dict())
  clip_data = copy.deepcopy(clip_data)
  video_mode = clip_data['mode']
  if video_mode != 'crop_center' and video_mode != 'blurred':
    video_mode = 'background'
    clip_data['color'] = clip_data['mode']
    clip_data['mode'] =  video_mode
  output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(video_mode, clip_data['format']))
  if 'view' in clip_data:
    output_path = output_path.replace('.{}'.format(clip_data['format']), '_{}.{}'.format(clip_data['view'], clip_data['format']))
  if 'texts' not in clip_data or not clip_data['texts']:
    shutil.move(video_path, output_path)
    return
  flt_cmd = build_filter_texts(clip_data, format_data)
  ff_filter_type = "-vf"
  if len(clip_data['inputs']) > 1:
    flt_cmd = ''.join([flt_cmd, build_logo_filter(clip_data, format_data)])
    ff_filter_type = "-filter_complex"
  inputs = []
  for x in clip_data['inputs']:
    inputs.append('-i')
    inputs.append(x)
  if clip_data.get('preview', False):
    fill_cmd = [ "ffmpeg", "-ss","00:00:01",*inputs, ff_filter_type, flt_cmd, '-vframes', '1', '-q:v','2', output_path.replace('.mp4','.jpg')]
  else:
    fill_cmd = [ "ffmpeg", *inputs, ff_filter_type, flt_cmd, output_path]
  clipping_log = '{}Render texts to:{} {}'.format('\033[1m', '\033[0m', output_path)
  logger.info(clipping_log)
  run_ffmpeg_cmd(fill_cmd)