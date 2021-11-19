import hardware
import sys
import logging

from constants import LOGGER_NAME
# from manager.audio import store_audio_files
# from manager.video import store_video_files
from manager.file import save_files
from clipper.clipper import clipper
# from hardware.control import hardware_control

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
    if sys.argv[1] == 'clip':
      # all or /directory
      clipper()
    
    # if sys.argv[1] == 'control':
    #   # all or /directory
    #   hardware_control()

  else:
    error = 'Missing arguments \n\n\t\tnu9ve [program]\n\n\t\tsave\n\t\tclip\n\t\tcontrol\n\n'
    logger.error(error)

