import sys
import logging

from utils.constants import LOGGER_NAME
# from manager.audio import store_audio_files
# from manager.video import store_video_files
from manager.file import save_files

if __name__ == "__main__":
  # MAIN
  logger = logging.getLogger(LOGGER_NAME)
  logger.setLevel(logging.DEBUG)
  ch = logging.StreamHandler()
  ch.setLevel(logging.DEBUG)
  formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
  ch.setFormatter(formatter)
  logger.addHandler(ch)
  
  if len(sys.argv) > 2:
    if sys.argv[1] == 'save':
      # all or /directory
      save_files()
    # if sys.argv[1] == 'zoom':
    #   store_audio_files()
    # if sys.argv[1] == 'videos':
    #   store_video_files()
    # if sys.argv[1] == 'yt2mix2stems':
    #   download_youtube_song_split_stem()
    # if sys.argv[1] == 'file2stems':
    #   song_split_stem(2)
    # if sys.argv[1] == 'file4stems':
    #   song_split_stem(4)
    # if sys.argv[1] == 'soundfx':
    #   download_youtube('/Users/erikiado/Movies/sounds/fx')
    # if sys.argv[1] == 'social':
    # if sys.argv[1] == 'audio':
    # if sys.argv[1] == 'visuals':
  else:
    error = 'Missing arguments \n\n\t\terikpyado [program]\n\n\t\tsave\n\n'
    logger.error(error)

