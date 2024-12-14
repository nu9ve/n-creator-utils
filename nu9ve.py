import hardware
import sys
import logging

from constants import LOGGER_NAME
# from manager.audio import store_audio_files
# from manager.video import store_video_files
from manager.file import save_files, downloader, splitter
from clipper.clipper_cli import clipper
# from hardware.control import hardware_control

help_text = '''
  nu9ve [program]
    save  - save all files or files in a directory
    clip  - clip all files or files in a directory
    control - control hardware
    download - download files from the cloud
    split - download/split audio files into stems
'''

if __name__ == "__main__":
  # MAIN
  logger = logging.getLogger(LOGGER_NAME)
  logger.setLevel(logging.DEBUG)
  ch = logging.StreamHandler()
  ch.setLevel(logging.DEBUG)
  formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
  ch.setFormatter(formatter)
  logger.addHandler(ch)
  
  if len(sys.argv) > 1 and sys.argv[1] in ['save', 'clip', 'control', 'download', 'split']:
    if sys.argv[1] == 'save':
      # all or /directory
      save_files()
    elif sys.argv[1] == 'download':
      downloader()
    elif sys.argv[1] == 'split':
      splitter()
    elif sys.argv[1] == 'clip':
      clipper()
    
    # if sys.argv[1] == 'control':
    #   # all or /directory
    #   hardware_control()
    else:
      error = f'program not found: {sys.argv[1]} \n\n{help_text}'
      logger.error(error)
  else:
    error = f'missing arguments\n\n{help_text}'
    logger.error(error)

