import os
import subprocess
# from spleeter.separator import Separator



def download_youtube_song_split_stem():
  youtube_link = sys.argv[2]
  mix_path = '/Volumes/SSD1/mix/000/'
  stems_path = '/Volumes/SSD1/abletonlive/stems/'
  youtube_dl_cmd = 'youtube-dl --extract-audio --audio-format mp3 '+ youtube_link
  os.system(youtube_dl_cmd)
  youtube_name_cmd = 'youtube-dl --get-filename '+ youtube_link
  ytcmdout = subprocess.check_output(youtube_name_cmd, shell=True)
  ytcmdout = ytcmdout.replace(b"'",b"\'")
  # outfilename = str(ytcmdout)[2:-7]+'.mp3'
  outfilename = str(ytcmdout)[2:-8]+'.mp3'

  print()
  print('downloaded filename:',outfilename)
  print('Video name:',ytcmdout)
  print()
  # outfilename = outfilename.replace("\\xe2\\x80\\x93","–")
  # outfilename = outfilename.replace("\\xc3\\xb1","ñ")
  # outfilename = outfilename.replace("\\xc3\\xad","í")
  # outfilename = outfilename.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c \xea\xb0\x80\xec\x9e\xa5 \xeb\xa7\xa4\xec\x9a\xb4 \xea\xb3\xbc\xec\x9e\x90 \xeb\x8f\x84\xec\xa0\x84 \xeb\xa8\xb9\xeb\xb0\xa9\xf0\x9f\x94\xa5','')
  # outfilename = outfilename.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c','')
  outfilename = outfilename.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6","[Mukbang] 세상에서 가장 매운 과자 도전 먹방🔥Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung-SkNH5oOfn6")
  # video_name = str(ytcmdout)[2:-19]
  video_name = str(ytcmdout)[2:-20]
  video_name = video_name.replace("[Mukbang] \\xec\\x84\\xb8\\xec\\x83\\x81\\xec\\x97\\x90\\xec\\x84\\x9c \\xea\\xb0\\x80\\xec\\x9e\\xa5 \\xeb\\xa7\\xa4\\xec\\x9a\\xb4 \\xea\\xb3\\xbc\\xec\\x9e\\x90 \\xeb\\x8f\\x84\\xec\\xa0\\x84 \\xeb\\xa8\\xb9\\xeb\\xb0\\xa9\\xf0\\x9f\\x94\\xa5Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound ASMR Ssoyoung","[ASMR,Mukbang] Hottest Chip PAQUI One CHIP CHALLENGE Eatingsound")
  # video_name = video_name.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c','')
  # video_name = video_name.replace('\xec\x84\xb8\xec\x83\x81\xec\x97\x90\xec\x84\x9c \xea\xb0\x80\xec\x9e\xa5 \xeb\xa7\xa4\xec\x9a\xb4 \xea\xb3\xbc\xec\x9e\x90 \xeb\x8f\x84\xec\xa0\x84 \xeb\xa8\xb9\xeb\xb0\xa9\xf0\x9f\x94\xa5','')

  print()
  print('replaced filename:',outfilename)
  print('replaced Video name:',video_name)
  print()
  final_file_path = os.path.join(mix_path,video_name+'.mp3')
  shutil.move(os.path.join(os.getcwd(),outfilename),final_file_path) 
  print(final_file_path)
  # separator = Separator('spleeter:2stems')
  separator = Separator('spleeter:4stems')
  separator.separate_to_file(final_file_path, stems_path)

def song_split_stem(s):
  file_name = sys.argv[2]
  stems_path = '/Volumes/SSD1/abletonlive/stems/'
  file_path = os.path.join(os.getcwd(),file_name)
  separator = Separator('spleeter:'+str(s)+'stems')
  separator.separate_to_file(file_path, stems_path)