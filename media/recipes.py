"""Recipes: device and flow rules kept as data, not code.

`nu9ve-default` is the user's own setup and only the *initial template*: another person will not have
a Mavic Air 2 or subaereo. Later the recipe becomes editable by nodes; until then the planner reads it
from here and freezes what it used (id + digest) in each plan, like `profile_routes`.

Device identity is Make + Model + Serial (never a filename, DCIM folder or card name). Confirmed devices
live in the catalog (`device-registry`) and win over template rules.
"""
import copy
import datetime
import re

from .common import digest, require, slug

TEMPLATE = {
    'schema': 'nu9ve-recipe/1',
    'id': 'nu9ve-default',
    'name': 'nu9ve · plantilla inicial',
    # Card families from the top two directory levels only. A family suggests a flow (`recommendation`); it never
    # classifies files (DJI stays ambiguous). First match wins, so order matters. Each entry lists alternative
    # clauses (`any`); a clause holds when all of its conditions hold, and a condition holds when any of its names
    # matches (lowercase). Conditions: `top`/`top_prefix`/`top_suffix` (names at the root), `in` ({folder: names}
    # one level down), `dcim_prefix`/`dcim_suffix`/`dcim_regex` (folders under DCIM). Safety invariants (Sony
    # indexes never deleted, cleanup only after verification) stay in code and cannot be edited here.
    'cards': [
        {'family': 'stalker', 'label': 'Stalker', 'recommendation': 'stalker_batch', 'any': [{'top_suffix': ['.stalker']}]},
        {'family': 'sony', 'label': 'Sony', 'recommendation': 'sony_format_in_camera', 'cleanup': 'sony_clips_then_format',
         'any': [{'in': {'private': ['m4root']}}, {'top': ['avf_info']}]},
        {'family': 'h6', 'label': 'Zoom H6', 'recommendation': 'audio_needs_work_disk',
         'any': [{'top_prefix': ['zoom', 'folder']}, {'top': ['h6_settings']}]},
        {'family': 'gopro', 'label': 'GoPro', 'recommendation': 'archive_and_work', 'any': [{'dcim_suffix': ['gopro']}]},
        {'family': 'insta360', 'label': 'Insta360', 'recommendation': 'dji_choose_category',
         'any': [{'dcim_suffix': ['insta360'], 'top': ['misc']}, {'dcim_prefix': ['camera'], 'top': ['misc']}]},
        {'family': 'dji', 'label': 'DJI', 'recommendation': 'dji_choose_category',
         'any': [{'top_prefix': ['dji']}, {'dcim_prefix': ['dji']}, {'top': ['misc'], 'dcim_regex': r'\d{3}media'}]},
        {'family': 'canon', 'label': 'Canon', 'recommendation': 'archive_and_work',
         'any': [{'dcim_suffix': ['canon']}, {'dcim_prefix': ['eos']}]},
    ],
    'card_default': {'family': 'generic', 'recommendation': 'archive_and_work'},
    # FUENTE tag for a camera model read from its own metadata (Sony XML `modelName`), and the tag of a family whose
    # clips carry no model (historical names on MEDIA keep it).
    'models': {'ILCE-6400': 'SONYA6400'},
    'family_source': {'sony': 'SONYA6400'},
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
        # Which DNGs stay in work (A16 sample); the archive keeps all of them.
        'dng': {'labels': 'atlas-derived/labels/cells/*/manifest.json', 'labels_key': 'photos',
                'light_classes': ['directSun', 'lowSun', 'diffuseDusk', 'deepDusk', 'night'], 'dominance': 0.6,
                'per_light': 24, 'poor_sample': 24, 'older_sample': 12, 'older_than_days': 30},
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
        # A saved recipe predating a section inherits the template's section, never a silent empty rule set.
        return copy.deepcopy({**TEMPLATE, **saved})
    return copy.deepcopy(TEMPLATE)


def _clause(clause, top, second):
    dcim = second.get('dcim', set())
    checks = {
        'top': lambda names: bool(top & set(names)),
        'top_prefix': lambda names: any(n.startswith(tuple(names)) for n in top),
        'top_suffix': lambda names: any(n.endswith(tuple(names)) for n in top),
        'in': lambda folders: all(set(names) & second.get(folder, set()) for folder, names in folders.items()),
        'dcim_prefix': lambda names: any(n.startswith(tuple(names)) for n in dcim),
        'dcim_suffix': lambda names: any(n.endswith(tuple(names)) for n in dcim),
        'dcim_regex': lambda pattern: any(re.fullmatch(pattern, n) for n in dcim),
    }
    require(clause and set(clause) <= set(checks), 'invalid_recipe', 'Unknown card condition: ' + ', '.join(sorted(set(clause) - set(checks))))
    return all(checks[key](value) for key, value in clause.items())


def card_parents(value):
    """Top-level folders whose children the card rules look at (always DCIM)."""
    return {'dcim'} | {folder for card in value['cards'] for clause in card['any'] for folder in clause.get('in', {})}


def card_family(value, top, second):
    """First card rule matching these lowercase names, or the default family."""
    rule = next((card for card in value['cards'] if any(_clause(c, top, second) for c in card['any'])), None)
    return rule or value['card_default']


def model_source(value, model_name):
    return value['models'].get(model_name) if model_name else None


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


def summary(catalog):
    """Read-only view of the effective recipe for the Flujos tab: what is recognized and what each flow does."""
    from .planner import CAMERAS
    value = recipe(catalog)
    registry = catalog.get(REGISTRY_KEY, {}) if catalog is not None else {}
    return {'recipe': recipe_ref(value), 'name': value['name'],
            'cards': [{k: card.get(k) for k in ('family', 'label', 'recommendation', 'cleanup')} for card in value['cards']],
            'devices': [{k: d.get(k) for k in ('make', 'model', 'label', 'category', 'source')} for d in value['devices']],
            'confirmed_devices': len(registry),
            'categories': sorted(CAMERAS | set(value['categories'])),
            'missions': [{'id': f['id'], 'label': f['label'], 'project': f['project'], 'devices': f['devices'],
                          'archive_folder': f['archive_folder'], 'work_folder': f['work_folder']}
                         for f in value.get('missions') or []]}
