# nu9ve clipper

the *clipper* (nu9ve clip) helps reducing some time and efforts on the distribution of a clipable video. it allows the clipping and reformating of a full length video into various segments via timestamps on a config files. it exports into multiple formats so they are easily shared in any platform.


### getting started
the fastest way to get started is to copy a config file from the examples into your working directory 
```console
nu9ve clip [videopath]
```

you can run the clipper as a standalone script or configure [nu9ve](https://github.com/nu9ve/n-creator-utils/blob/master/README.md) to run as subcommand. these are the dependencies:
```
python 3.9
ffmpeg
```

to run this script as standalone replace in the examples `nu9ve` with `python` and run the script from your video project directory like this:
```console
/pc/videos/project $ python /path/to/clipper/clipper.py [videopath]
```

your project directory should include the video file, configuration json and optional added assets like logos or backgrounds
you should structure it something similar to this:
```
/project/
  videofile.mp4
  config.json
  [watermark.jpg]
```

### running command

basic command with config json in working directory:
```console
nu9ve clip [videopath]
```

preview current configuration before rendering full clips:
```console
nu9ve clip [filepath] --preview
```

### configuration files

config json can help output clips into different formats and with varied styles, texts and images

<!-- formats:
```
    portrait
    landscape
    square
``` 
-->

video and clip parameters:
```
    view
    texts
    logo
```

clip parameters
```
    title

    mode
        background
        blurred
        crop_center

    view
        100 - fills entire screen (similar to crop_center)
        90 - fills 90% of the screen with the centered video
        10 - the video fills only 10% of the 
```

simple config:
```json
{
	"clips":[{
		"title": "primer titulo",
		"start": "00:01:10",
		"end": "00:01:15"
	}]
}
```

full config:
```json
{
	"mode": "#213233",
	"view": 50,
	"texts": [
		{
			"text": "hola que pets"
		},
		{
			"text": "xiao",
			"y": "80"
		}
	],
	"clips":[{
		"title": "primer titulo",
		"start": "00:01:10",
		"end": "00:01:15",
		"view": 90
	},
	{
		"title": "segundo titulo",
		"start": "00:02:50",
		"end": "00:04:15",
		"texts": [
			{
				"text": "holi",
				"y": "10"
			},
			{
				"text": "holi2",
				"y": "20"
			},
			{
				"text": "bottttom text"
			},
			{
				"text": "holi3",
				"y": "40"
			}
		]
	}]
}
```

creativo:
```json
{
	"background": "#FFFFFF",
	"logo": "[/path/to/logo.jpg]",
	"view": 80,
	"text_size": "4",
	"clips":[{
		"title": "Mi arranque de rockstar",
		"start": "01:27:40.70",
		"end": "01:34:32"
	},
	{
		"title": "El wey que he querido ser",
		"start": "01:34:33.50",
		"end": "01:40:06"
	},
	{
		"title": "El que no conoce a dios...",
		"start": "01:40:11",
		"end": "01:52:37"
	},
	{
		"title": "Ser un camaleon social y adaptarte",
		"start": "01:04:11.70",
		"end": "01:12:37"
	},
	{
		"title": "9 morras me batearon",
		"start": "00:48:35.70",
		"end": "00:57:41"
	},
	{
		"title": "Como revolucionar la comedia",
		"start": "00:28:33",
		"end": "00:36:51"
	},
	{
		"title": "Me hubiera comprado el BMW",
		"start": "01:13:02.20",
		"end": "01:26:43"
	},
	{
		"title": "La disciplina supera al talento",
		"start": "00:06:21.50",
		"end": "00:13:53"
	}]
}
```


### help commands

cropping youtube videos:
```
youtube-dl -f 'bestvideo[ext=mp4]+bestaudio[ext=m4a]/mp4' [youtube_url]
```


<!-- ## running the tests (xd)

filemanager should output correct actions
clipper should check ffmpeg version or export is working

```
nu9ve test [function]
``` 

## contributing

please read [CONTRIBUTING.md](https://gist.github.com/PurpleBooth/b24679402957c63ec426) for details on our code of conduct, and the process for submitting pull requests to us.

## license

this project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details

## acknowledgments

* hat tip to anyone whose code was used
* thanks for the [template](https://gist.github.com/PurpleBooth/b24679402957c63ec426) purplebooth
-->