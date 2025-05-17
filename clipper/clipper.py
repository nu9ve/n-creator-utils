from os.path import exists as path_exists, join as join_path
import pathlib
from clipper.clip_settings import read_from_json, get_video_info, parse_clip_data
import replicate
import json
import openai
import pickle
from pprint import pformat

from clipper.ffmpeg import (cut_range, render_portrait_video, extract_audio,
	render_landscape_video, render_square_video, render_filters_texts)


# detect input dimensions
# vertical to vertical, horizontal, square
# horizontal to vertical, horizontal, square

# def clip_video(video_path, clips, preview):
#   logger.info(video_path)
#   video_data = get_video_info(video_path)
#   message = '''{}
#   w: {} h:{}
#   ratio: {}
#   fps: {}\n'''.format(video_data['full_path'], video_data['width'], video_data['height'], video_data['ratio'], video_data['frame_rate'])
#   logger.info(message)
#   is_landscape = video_data['ratio'] > 1
#   for i, c in enumerate(clips):
#     output_path = video_path.replace('.mp4', '_clip_{}.mp4'.format(i))
#     clip_data = parse_clip_data(video_data, c)
#     if not clip_data['valid']:
#       invalid_error = '''clip {} config error\n'''.format(i)
#       logger.error(invalid_error)
#     cut_range(video_path, clip_data, output_path)
#     clip_data['is_landscape'] = is_landscape
#     clip_data['inputs'][0] = output_path
#     clip_data['preview'] = preview

#     if is_landscape:
#       render_portrait_video(output_path, clip_data)
#     else:
#       render_landscape_video(output_path, clip_data)
#     if not clip_data.get('vertical_only', False):
#       render_square_video(output_path, clip_data)
#       render_filters_texts(output_path, clip_data)





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


# clipper_logger
class Clipper:
   
  video_root = None
  video_path = None
  audio_path = None
  config_file = None

  logger = None
  replicate_client = None

  num_clips = 1
  clips = None

  # modes
  preview = False
  generate_clip_config = 0

  template = """
  The following is a transcript of a youtube video with timestamps, i want to create the best {num_clips} clips from this video.
  the best clips contains the most complex ideas in the shortest amount of time and has a clear explanation of the idea.
  output a short title and description in youtube format that gets lots of views.
  titles should be no more than 100 characters and the descriptions should be no more than 300 characters.
  duration of clips is within a 2-4 minute (120-300 seconds) window, no less than 100 seconds and no more than 300 seconds.
  for each clip with start and end timestamps and the duration of the clip in seconds.
  join all metadata in a list of clips with start, end, title in the detected language of the video script, description in the detected language of the video script, duration
  output all metadata in a JSON format.

  here is the first segment of timestamps and the script of the video:
  ## START SCRIPT SEGMENT
  {segments}
  ## END SCRIPT SEGMENT

  detect language from SCRIPT and output titles and descriptions in that language and all clip metadata into a JSON format
  """

  def __init__(self, video_path, config_file, preview, logger, generate_clips=False):
    self.video_path = video_path
    self.video_root = pathlib.Path(video_path).parent.absolute()
    self.config_file = config_file
    self.preview = preview
    self.logger = logger
    self.generate_clip_config = generate_clips
    self.config_format = "json"
    self.replicate_client = replicate.Client(api_token="ee353fdb2c553a6cde7e191ffa62f693c5723c6c")


  def clip_video(self):
    if not self.video_path:
      self.logger.error('No video path')
      return 
    self.load_clips_config()
    if len(self.clips) == 0:
      self.logger.error('No clips configured [], nothing to do\n\nplease configure a config.json file, see: https://github.com/nu9ve/n-creator-utils/tree/master/clipper') # todo: add link to docs in github
      return

    self.clip_loaded_video()
  
  # def clip_from_json(self):
  #   clips = read_from_json(self.config_file, self.video_path)
  #   self.clip_loaded_video(self.video_path, clips, self.preview)

  # def clip_from_txt(self):
  #   clips = read_from_txt(self.config_file, self.video_path)
  #   self.clip_loaded_video(self.video_path, clips, self.preview)

  def load_clips_config(self):
    #default on script folder
    clips_config = []
    #search config in video root
    default_json_config_path = join_path(self.video_root, 'config.json')
    param_config_path = join_path(self.video_root, self.config_file or 'config.json')

    if self.generate_clip_config:
      self.logger.info('Generating config.json')
      clips_config = self.generate_clips_with_open_ai()

    elif path_exists(default_json_config_path):
      # clips_config = config_video_path
      clips_config = read_from_json(default_json_config_path, self.video_path)

    elif path_exists(param_config_path):
      # clips_config = config_video_path
      clips_config = read_from_json(self.config_file, self.video_path)    

    self.clips = clips_config

  def generate_clips_with_open_ai(self):
    clips = []
    if not path_exists(self.video_path):
      self.logger.error('Video not found')
      return clips
    
    # get audio from video
    audio_path = extract_audio(self.video_path)

    if not path_exists(audio_path):
      self.logger.info('Audio not extracted')
      return clips
    
    # check file size
    audio_size = pathlib.Path(audio_path).stat().st_size
    if audio_size < 1000000:
      self.logger.info('Audio file too small')
      return clips
    
    self.logger.info('Audio file size: {}'.format(audio_size))
    self.audio_path = audio_path

    segments_file_name = "segments_{}.p".format(self.video_path.split('/')[-1].split('.')[0])
    if path_exists(segments_file_name):
      self.logger.info('Loading segments from pickle: {}'.format(segments_file_name) )
      clean_segments = pickle.load(open(segments_file_name, "rb"))
    else:
      clean_segments = self.generate_clips_with_whisper()

    if len(clean_segments) == 0:
      self.logger.info('No words found')
      return clips
    
    # save segments to pickle
    if not path_exists(segments_file_name):
      pickle.dump(clean_segments, open(segments_file_name, "wb"))

    clips = self.choose_best_clips_with_gpt(clean_segments)
    return clips
  
  def choose_best_clips_with_gpt(self, clean_segments):

    # get best clips range with timestamps from chatgpt
    segments = ""
    for segment in clean_segments[:40]:
      segments += f"{segment['start']}: {segment['text']}\n"
    segments = segments.strip()

    self.logger.info("sending to gpt to choose best clips...")

    prompt = self.template.format(segments=segments, num_clips=self.num_clips)
    completion = openai.ChatCompletion.create(
      model="gpt-3.5-turbo",
      messages=[
        {"role": "user", "content": prompt},
      ]
    )
    self.logger.info("prompt")
    self.logger.info(pformat(prompt))
    self.logger.info("completion")
    self.logger.info(pformat(completion))
    clips_json = json.loads(completion["choices"][0]["message"]["content"])
    clips_json = clips_json['clips'] if 'clips' in clips_json else clips_json
    clips = self.format_gpt_clips(clips_json)
    # if 'texts' in c:
    #   clip_data['texts'] = copy.deepcopy(default_clip_output['texts']) + c['texts']
    # else:
    #   clip_data['texts'] = copy.deepcopy(default_clip_output['texts'])
    # if 'title' in c:
    #   # clip_data['view'] = 80
    #   title = dict(text=c['title'], y=(98-clip_data['view']))
    #   clip_data['texts'].append(title)
    return clips
  
  def format_gpt_clips(self, clips_json):
    clips = []
    for c in clips_json:
      print(c)
      clip_data = c
      clip_data['inputs'] = [self.video_path]
      clip_data['input_types'] = []
      clip_data['view'] = 90
      clip_data['texts'] = []
      clip_data['background'] = "#101070"
      # if 'background' in default_clip_output:
      # if 'texts' in c:
      #   clip_data['texts'] = copy.deepcopy(default_clip_output['texts']) + c['texts']
      # else:
      #   clip_data['texts'] = copy.deepcopy(default_clip_output['texts'])
      if 'title' in c:
        # clip_data['view'] = 80
        title = dict(text=c['title'], y=(98-clip_data['view']))
        clip_data['texts'].append(title)
      clips.append(clip_data)
    return clips
  
  def generate_clips_with_whisper(self):
    self.logger.info("sending to whisper...")
    output = self.replicate_client.run(
      "hnesk/whisper-wordtimestamps:4a60104c44dd709fc08a03dfeca6c6906257633dd03fd58663ec896a4eeba30e",
      input={"audio": open(self.audio_path, "rb")},
    )
    # self.logger.info(pformat(output))
    clean_segments = self.clean_segments(output["segments"])
    self.logger.info("whisper output")
    self.logger.info(pformat(clean_segments))
    return clean_segments

  def clean_segments(self, segments):
    clean_segments = []
    for segment in segments:
      if segment["text"] != "":
        s = dict()
        s["start"] = segment["start"]
        s["end"] = segment["end"]
        s["text"] = segment["text"]
        clean_segments.append(s)
    return clean_segments

  def clip_loaded_video(self):
    self.logger.info(self.video_path)
    self.logger.info(pformat(self.clips))
    video_data = get_video_info(self.video_path)
    message = '''
      {}
      w: {} h:{}
      ratio: {}
      fps: {}\n'''.format(video_data['full_path'], video_data['width'], video_data['height'], video_data['ratio'], video_data['frame_rate'])
    self.logger.info(message)

    is_landscape = video_data['ratio'] > 1
    for i, c in enumerate(self.clips):
      output_path = self.video_path.replace('.mp4', '_clip_{}.mp4'.format(i)).replace('.mov', '_clip_{}.mp4'.format(i))
      clip_data = parse_clip_data(video_data, c)
      if not clip_data['valid']:
        invalid_error = '''clip {} config error\n'''.format(i)
        self.logger.error(invalid_error)
      cut_range(self.video_path, clip_data, output_path)
      clip_data['is_landscape'] = is_landscape
      clip_data['inputs'][0] = output_path
      clip_data['preview'] = self.preview

      if is_landscape:
        render_portrait_video(output_path, clip_data)
      else:
        render_landscape_video(output_path, clip_data)
      if not clip_data.get('vertical_only', False):
        render_square_video(output_path, clip_data)
        render_filters_texts(output_path, clip_data)
