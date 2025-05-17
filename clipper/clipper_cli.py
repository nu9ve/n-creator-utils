import sys
import logging
import argparse
from constants import LOGGER_NAME
from clipper.clipper import Clipper

# cmd line interface
def clipper():
  help_error = '''
  * nu9ve clip src [config_path] --preview
      src             video file path to clip
      [config_path]   path to the file with the configuration of clips (json or txt) - optional (defaults to config.json)
      --preview       saves a single frame of each clip to be created - optional (defaults to False)
  '''

  clipper_logger = logging.getLogger(LOGGER_NAME)

	# clipper.py video.mp4 [config.json|.txt]
  if len(sys.argv) != 4 and len(sys.argv) != 3:
    clipper_logger.error(help_error)
    return

  parser = argparse.ArgumentParser(description='clip a video into different segments and export into multiple formats specified in a config file')
  parser.add_argument('program', metavar='program', type=str,
                      help='function to run')
  parser.add_argument('video_path', metavar='video_path', type=str,
                      help='path to the video to clip')
  parser.add_argument('config_path', metavar='config_path', type=str, default="", nargs='?',
                      help='path to the file with the configuration of clips (json or txt)')
  parser.add_argument('--preview', dest='preview', action='store_const',
                    const=True, default=False,
                    help='print single frame of clips')
  parser.add_argument('--verbose', dest='verbose', action='store_const',
                    const=True, default=False, help='print debug messages')
  parser.add_argument('--openai', dest='openai', action='store_const',
                    const=True, default=0, help='use openai to generate clips')
  args = parser.parse_args()
  
  # print(args)
  clipper = Clipper(args.video_path, args.config_path, args.preview, clipper_logger, generate_clips=args.openai)
  clipper.clip_video()