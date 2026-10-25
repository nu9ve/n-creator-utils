"""Explicit publication state per variant/channel; no network publication is performed."""
from pathlib import Path
from urllib.parse import urlparse

from .common import now, relative, require
from .library import asset_record


def set_publication(catalog, asset_id, variant, channel, state, url=None):
    require(state in {'candidate', 'approved', 'published'}, 'invalid_state', 'Invalid publication state')
    relative(variant)
    require(isinstance(channel, str) and 0 < len(channel) <= 160, 'invalid_channel', 'Choose a channel')
    require(url is None or isinstance(url, str) and urlparse(url).scheme in {'https', 'http'}, 'invalid_url', 'Publication URL must be HTTP(S)')
    asset = asset_record(catalog, asset_id)
    require(asset['kind'] == 'delivery', 'invalid_asset', 'Publication state belongs to a delivery')
    require(variant in asset['paths'], 'unknown_variant', 'Variant is not a member of this delivery')
    with catalog.writer(), catalog.db:
        catalog.db.execute('INSERT OR REPLACE INTO publications VALUES(?,?,?,?,?,?)', (asset_id, variant, channel, state, url, now()))
    catalog.event(asset['job_id'], 'publication_' + state, {'asset_id': asset_id, 'variant': variant, 'channel': channel})
    return {'asset_id': asset_id, 'variant': variant, 'channel': channel, 'state': state, 'url': url}
