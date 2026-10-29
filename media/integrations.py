"""Optional integration discovery; failure is local to the requested capability."""
from .common import MediaError


def capabilities(catalog):
    from . import clipper
    from .retitle import producer, CAPABILITY
    result = {'clipper': {'available': False}, 'stalker': {'available': False}}
    try:
        data = clipper.dependencies(catalog, timeout=3)
        available = data.get('integration') != 'never_configured' and 'media-release-lock/1' in data.get('capabilities', [])
        result['clipper'] = {'available': available, 'capabilities': data.get('capabilities', []),
                             'known': bool(catalog.get('clipper-known') or catalog.get('clipper-executable'))}
    except (MediaError, OSError, ValueError, TypeError, KeyError) as error:
        result['clipper']['error'] = str(error)
    try:
        data = producer(catalog, 'capabilities', {}, timeout=3)
        result['stalker'] = {'available': data.get('schema') == 'stalker-capabilities/1' and CAPABILITY in data.get('capabilities', []),
                             'capabilities': data.get('capabilities', [])}
    except (MediaError, OSError, ValueError, TypeError, KeyError) as error:
        result['stalker']['error'] = str(error)
    return result
