import argparse
import logging


def clipper(argv=None):
    parser = argparse.ArgumentParser(description='Recorta un video usando configuración JSON.')
    parser.add_argument('video_path', help='video fuente')
    parser.add_argument('config_path', nargs='?', default='', help='JSON; relativo a la carpeta del video')
    parser.add_argument('--preview', action='store_true', help='exporta imágenes de los encuadres')
    parser.add_argument('--verbose', action='store_true', help='mensajes de diagnóstico')
    parser.add_argument('--openai', action='store_true', help='prototipo de generación con IA (SDK legado)')
    args = parser.parse_args(argv)
    from constants import LOGGER_NAME
    from clipper.clipper import Clipper
    logger = logging.getLogger(LOGGER_NAME)
    if args.verbose:
        logger.setLevel(logging.DEBUG)
    instance = Clipper(args.video_path, args.config_path, args.preview, logger,
                       generate_clips=args.openai)
    instance.clip_video()
