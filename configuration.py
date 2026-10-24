"""User settings, independent from CLI and optional media dependencies."""

import datetime
import json
import os
from pathlib import Path


class ConfigurationError(ValueError):
    pass


def load_settings(config_path=None, environ=None):
    """Load defaults < local JSON < NU9VE_* environment variables.

    Relative JSON paths belong to the config directory; environment paths belong
    to the working directory. Nothing is created while reading configuration.
    """
    env = os.environ if environ is None else environ
    home = Path.home()
    year = datetime.datetime.now().strftime('%Y')
    settings = {
        'root_dir': '/Volumes/MEDIA502/001_raw',
        'sd_root_dir': '/Volumes',
        'root_recordings_dir': '/Volumes/SSD1',
        'audio_hijack_dir': str(home / 'Music/Audio Hijack'),
        'audio_samples_dir': str(home / f'Music/01_produccion/samples/{year}/raw'),
        'audio_splits_dir': str(home / 'Music/01_produccion/splits'),
        'audio_songs_dir': str(home / 'Music/02_mix/latest'),
        'tmp_video_dir': str(home / 'data/projectIX/raw'),
        'clipper_font_file': str(home / 'Downloads/test_clipper/coolvetica.ttf'),
        'yt_dlp_cookies_browser': 'brave',
        'debug': False,
        'sd_src_name': {
            'EOS_DIGITAL': 'SL3', 'NIKON D3300': 'NIKON', 'GOPRO64': 'GOPRO4',
            'MAVIC200': 'MAVIC2', 'H6_SD': 'H6', 'INSTA360': 'INSTA360',
            'Untitled': 'AUTO', 'NO NAME': 'SONYA6400',
        },
    }
    explicit = config_path is not None or 'NU9VE_CONFIG' in env
    path = Path(config_path if config_path is not None else env.get(
        'NU9VE_CONFIG', str(Path(__file__).with_name('config.local.json')))).expanduser()
    if explicit and not path.is_file():
        raise ConfigurationError(f'Configuration file does not exist: {path}')
    local = {}
    if path.is_file():
        try:
            local = json.loads(path.read_text(encoding='utf-8'))
        except (OSError, ValueError) as error:
            raise ConfigurationError(f'Cannot read configuration {path}: {error}') from error
        if not isinstance(local, dict):
            raise ConfigurationError('Configuration must be a JSON object')
        unknown = set(local) - set(settings)
        if unknown:
            raise ConfigurationError(f'Unknown configuration keys: {", ".join(sorted(unknown))}')
        settings.update(local)
    for key, value in settings.items():
        env_key = 'NU9VE_' + key.upper()
        if key == 'sd_src_name':
            if not isinstance(value, dict) or any(
                not isinstance(k, str) or not k or k in {'.', '..'} or '/' in k or '\\' in k
                or not isinstance(v, str) or not v.strip() for k, v in value.items()
            ):
                raise ConfigurationError('sd_src_name must map volume names to nonempty profile names')
            continue
        value = env.get(env_key, value)
        if key == 'debug':
            if isinstance(value, str) and value.lower() in {'true', 'false', '1', '0'}:
                value = value.lower() in {'true', '1'}
            if not isinstance(value, bool):
                raise ConfigurationError('debug must be true or false')
        elif not isinstance(value, str) or (not value.strip() and key != 'yt_dlp_cookies_browser'):
            raise ConfigurationError(f'{key} must be a nonempty string')
        elif key != 'yt_dlp_cookies_browser':
            value_path = Path(os.path.expandvars(value)).expanduser()
            if not value_path.is_absolute():
                base = Path.cwd() if env_key in env or key not in local else path.resolve().parent
                value_path = base / value_path
            value = str(value_path.resolve())
        settings[key] = value
    return settings
