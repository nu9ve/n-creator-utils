"""Local creative profiles. Routing is frozen in each ingest plan, never retroactive."""
from .common import relative, require, slug


def profiles(catalog):
    return sorted(catalog.get('creative-profiles', {}).values(), key=lambda value: value['id'])


def save_profile(catalog, id, name, raw_dir, work='', archive=''):
    profile_id = slug(id)
    require(profile_id == id, 'invalid_profile', 'Use a normalized profile ID')
    require(isinstance(name, str) and 0 < len(name.strip()) <= 120, 'invalid_profile', 'Choose a profile label')
    raw_dir = relative(raw_dir)
    resolved = {'work': '', 'archive': ''}
    for selected, role in ((work, 'work'), (archive, 'archive')):
        require(isinstance(selected, str), 'invalid_profile', 'Volume selection must be a string')
        if selected:
            record = catalog.volume(selected)
            require(record['role'] == role, 'invalid_role', 'Wrong profile volume role')
            resolved[role] = record['id']
    value = {'id': profile_id, 'name': name.strip(), 'raw_dir': raw_dir, **resolved}
    with catalog.writer():
        entries = catalog.get('creative-profiles', {})
        entries[profile_id] = value
        catalog.set('creative-profiles', entries)
    return value
