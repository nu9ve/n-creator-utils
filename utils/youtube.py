import os
import subprocess

def download_youtube(final_dir):
  youtube_link = sys.argv[2]
  youtube_dl_cmd = 'youtube-dl --extract-audio --audio-format mp3 '+ youtube_link
  os.system(youtube_dl_cmd)
  youtube_name_cmd = 'youtube-dl --get-filename '+ youtube_link
  ytcmdout = subprocess.check_output(youtube_name_cmd, shell=True)
  ytcmdout = ytcmdout.replace(b"'",b"\'")
  outfilename = str(ytcmdout)[2:-8]+'.mp3'
  outfilename = outfilename.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6","[Mukbang] 세상에서 가장 매운 과자 도전 먹방🔥Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6")
  video_name = str(ytcmdout)[2:-20]
  video_name = video_name.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung","[ASMR,Mukbang] Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound")
  final_file_path = os.path.join(final_dir,video_name+'.mp3')
  shutil.move(os.path.join(os.getcwd(),outfilename),final_file_path) 

