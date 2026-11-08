"""Recipes: device and flow rules kept as data, not code.

`nu9ve-default` is the user's own setup and only the *initial template*: another person will not have
a Mavic Air 2 or subaereo. Later the recipe becomes editable by nodes; until then the planner reads it
from here and freezes what it used (id + digest) in each plan, like `profile_routes`.

Device identity is Make + Model + Serial (never a filename, DCIM folder or card name). Confirmed devices
live in the catalog (`device-registry`) and win over template rules.
"""
import copy
import datetime

from .common import digest, require, slug

TEMPLATE = {
    'schema': 'nu9ve-recipe/1',
    'id': 'nu9ve-default',
    'name': 'nu9ve · plantilla inicial',
    # Devices by EXIF Make + Model; `source` is FUENTE in media-naming-v1.
    'devices': [
        {'make': 'DJI', 'model': 'FC3170', 'category': 'dji-drone', 'source': 'MAVICAIR2',
         'label': 'DJI Mavic Air 2', 'aliases': ['MAVIC2']},
    ],
    # Evidence that a still was taken in flight (drone, not DJI 360/action). Any key is enough.
    'flight_evidence': {'xmp': ['gimbal_pitch', 'flight_yaw', 'relative_altitude'], 'category': 'dji-drone'},
    # Makes whose files never classify by brand alone: a camera category must come from evidence.
    'review_makes': ['DJI'],
    # Archive folders per category and media kind (below the archive prefix, then AAAA/MM).
    'categories': {
        'dji-drone': {'archive': {'video': 'vdron', 'image': 'dron/fotos', 'raw_image': 'dron/fotos'},
                      'work_kinds': ['video', 'image']},
    },
    # Mission registries: exports of a flight app merged into one history (Atlas A13). Another setup may have
    # none, or another producer; nothing in media/missions.py names subaereo.
    'missions': [{
        'id': 'subaereo',
        'label': 'Misiones de subaereo',
        'project': 'subaereo',                     # default project of a photo matched to a mission
        'devices': ['dji-drone'],                  # categories whose stills may belong to a mission
        'inbox': '~/Downloads',                    # where AirDrop leaves the exports
        'patterns': ['coverage-*.json', 'JSON-*.json'],
        'schema_versions': [1, 2, 3],
        'merge_keys': {'plans': 'id', 'captures': 'captureID', 'captureLog': 'captureID',
                       'flightSessions': 'id', 'fieldNotes': 'id'},
        'record_dates': {'plans': 'updatedAt', 'captures': 'capturedAt', 'captureLog': 'capturedAt',
                         'flightSessions': 'updatedAt', 'fieldNotes': 'createdAt'},
        'tombstones': {'plans': 'deletedPlanIDs'},  # additive list once the app publishes deletions
        'archive_area': '004_datos/subaereo/coverage',   # below the archive volume root
        'work_mirror': 'atlas-subaereo/_missions',       # below the work volume prefix
        # Same original name, camera time ±2 s (±8 s against the command time of older exports), ≤ 40 m.
        'match': {'seconds': 2, 'seconds_command_time': 8, 'meters': 40},
        'hint': {'max_gimbal_pitch': -80, 'min_relative_altitude': 15},   # «probably a mission» while the export is missing
        'archive_folder': 'dron/misiones',          # + AAAA/MM/<session start>_<PLAN>/
        'work_folder': 'atlas-subaereo/misiones',   # below the work prefix; same session folder
        'work_root': 'atlas-subaereo',              # the consumer's root: a still already anywhere here is not copied again
        'work_kinds': ['image'],                    # JPG to work; DNG stays in the archive until asked (A16)
        'units_map': 'atlas-subaereo/_nuun/units.ndjson',
    }],
}


def mission_flow(value, flow_id=None):
    flows = value.get('missions') or []
    require(flows, 'no_mission_flow', 'This recipe has no mission registry')
    if flow_id is None:
        return flows[0]
    flow = next((f for f in flows if f['id'] == flow_id), None)
    require(flow is not None, 'no_mission_flow', 'Unknown mission registry: ' + str(flow_id))
    return flow

REGISTRY_KEY = 'device-registry'


def recipe(catalog=None):
    """Effective recipe: an edited one saved in the catalog (`recipe`), else the initial template."""
    saved = catalog.get('recipe') if catalog is not None else None
    if isinstance(saved, dict) and saved.get('schema') == TEMPLATE['schema']:
        return copy.deepcopy(saved)
    return copy.deepcopy(TEMPLATE)


def recipe_ref(value):
    return {'id': value['id'], 'digest': digest(value)}


def device_key(make, model, serial):
    return '|'.join((part or '').strip().upper() for part in (make, model, serial))


def _same(a, b):
    return (a or '').strip().upper() == (b or '').strip().upper()


def resolve(meta, catalog=None, value=None):
    """Device opinion for one still's facts, or None when it has no Make.

    `requires_confirmation` means the planner must not choose a category on its own: either the brand is
    ambiguous (DJI drone/360/action) without flight evidence, or the model is new to the recipe.
    """
    if not meta or not meta.get('make'):
        return None
    value = value or recipe(catalog)
    make, model, serial = meta.get('make'), meta.get('model'), meta.get('serial')
    device = {'make': make, 'model': model, 'serial': serial, 'key': device_key(make, model, serial)}
    registry = catalog.get(REGISTRY_KEY, {}) if catalog is not None else {}
    known = registry.get(device['key'])
    if known:
        return dict(device, category=known['category'], source=known['source'], label=known.get('label'),
                    rule='registry', requires_confirmation=False)
    rule = next((r for r in value['devices'] if _same(r['make'], make) and _same(r['model'], model)), None)
    if rule:
        return dict(device, category=rule['category'], source=rule['source'], label=rule.get('label'),
                    rule='template', requires_confirmation=False)
    flight = set((meta.get('flight') or {})) & set(value['flight_evidence']['xmp'])
    source = slug(model or make).upper().replace('-', '')
    if flight:
        # A new flying model: suggest the drone category once, the user confirms and it is registered.
        return dict(device, category=value['flight_evidence']['category'], source=source, label=None,
                    rule='flight_evidence', requires_confirmation=True)
    review = any(_same(m, make) for m in value['review_makes'])
    return dict(device, category=None, source=source, label=None, rule='unknown', requires_confirmation=review)


def unit_device(members, catalog=None, value=None):
    """One device for a unit (JPG+DNG pair), or a conflict when members disagree."""
    found = [f['photo'] for f in members if f.get('photo')]
    if not found:
        return None
    keys = {device_key(m.get('make'), m.get('model'), m.get('serial')) for m in found}
    if len(keys) != 1:
        return {'conflict': True, 'requires_confirmation': True, 'category': None}
    return resolve(found[0], catalog, value)


def devices(catalog):
    value = recipe(catalog)
    registry = catalog.get(REGISTRY_KEY, {})
    return {'recipe': recipe_ref(value), 'rules': value['devices'],
            'registry': sorted(registry.values(), key=lambda d: d['key'])}


def confirm_device(catalog, make, model, serial, category, source=None, label=None):
    """Remember a confirmed device so its files are classified without asking again."""
    require(all(isinstance(v, str) and v.strip() for v in (make, model, serial)), 'invalid_device',
            'Device needs Make, Model and Serial from the file')
    from .planner import CAMERAS
    require(category in CAMERAS, 'invalid_camera', 'Unknown camera category')
    source = slug(source or model).upper().replace('-', '')
    require(source and len(source) <= 40, 'invalid_device', 'Invalid source tag')
    require(label is None or isinstance(label, str) and len(label) <= 120, 'invalid_device', 'Invalid label')
    key = device_key(make, model, serial)
    entry = {'key': key, 'make': make.strip(), 'model': model.strip(), 'serial': serial.strip(), 'category': category,
             'source': source, 'label': label, 'confirmed_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    with catalog.writer():
        registry = catalog.get(REGISTRY_KEY, {})
        registry[key] = entry
        catalog.set(REGISTRY_KEY, registry)
    return entry
