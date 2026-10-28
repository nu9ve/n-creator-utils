"""Additive capture evidence; never infer a camera zone from the host timezone."""
import datetime as dt
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError


def capture_time(value=None, provenance=None, zone=None, originals=None):
    result = {'original': value, 'provenance': provenance, 'originals': originals or {},
              'captured_at_absolute': None, 'time_zone': None, 'offset_seconds': None}
    try:
        parsed = dt.datetime.fromisoformat(value.replace('Z', '+00:00'))
        if parsed.tzinfo is not None:
            result['captured_at_absolute'] = parsed.astimezone(dt.timezone.utc).isoformat().replace('+00:00', 'Z')
            result['offset_seconds'] = int(parsed.utcoffset().total_seconds())
            if zone:
                try:
                    tz = ZoneInfo(zone)
                    result['time_zone'] = zone
                    result['offset_seconds'] = int(parsed.astimezone(tz).utcoffset().total_seconds())
                except (ZoneInfoNotFoundError, ValueError, TypeError):
                    pass
    except (ValueError, TypeError, AttributeError):
        pass
    return result


def stalker_time(manifest):
    originals = {k: manifest[k] for k in ('epochWallClock', 'createdAt', 'captureTimeZone') if k in manifest}
    for key in ('epochWallClock', 'createdAt'):
        result = capture_time(manifest.get(key), 'manifest.' + key, manifest.get('captureTimeZone'), originals)
        if result['captured_at_absolute']:
            return result
    return capture_time(manifest.get('createdAt'), 'manifest.createdAt', originals=originals)
