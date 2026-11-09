"""Volume identity is independent of a display name or transient disk number."""
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys

from .common import MediaError, require


class VolumeBackend:
    def describe(self, root, external_required=False):
        root = Path(root).absolute()
        require(root.is_dir() and not root.is_symlink(), 'volume_offline', 'Root unavailable: ' + str(root))
        for path in (root, *root.parents):
            require(not path.is_symlink(), 'unsafe_path', 'Volume root contains a symlink')
        st = root.stat()
        if sys.platform == 'darwin':
            try:
                mounted = subprocess.run(['/bin/df', '-P', str(root)], capture_output=True, text=True, timeout=10)
            except subprocess.TimeoutExpired as error:
                raise MediaError('volume_identity_unavailable', 'df did not answer for ' + str(root) + '; retry in a moment') from error
            require(mounted.returncode == 0, 'volume_identity_unavailable', 'Cannot determine source filesystem')
            rows = mounted.stdout.strip().splitlines()
            require(len(rows) >= 2 and len(rows[-1].split(None, 5)) == 6, 'volume_identity_unavailable', 'Unexpected filesystem report')
            mount_path = rows[-1].split(None, 5)[-1]
            try:
                result = subprocess.run(['/usr/sbin/diskutil', 'info', '-plist', mount_path], capture_output=True, timeout=30)
            except subprocess.TimeoutExpired as error:
                # Freshly mounted cards can stall diskutil while Spotlight indexes them; fail closed, retry later.
                raise MediaError('volume_identity_unavailable', 'diskutil did not answer for ' + mount_path + '; retry in a moment') from error
            require(result.returncode == 0, 'volume_identity_unavailable', 'Cannot establish disk identity')
            data = plistlib.loads(result.stdout)
            mount = data.get('MountPoint')
            require(mount and data.get('VolumeUUID'), 'volume_identity_unavailable', 'Missing mounted volume identity')
            external = data.get('Internal') is False
            stores = data.get('APFSPhysicalStores') or []
            physical_disks = sorted({re.sub(r's[0-9]+(?:s[0-9]+)?$', '', item['APFSPhysicalStore']) for item in stores if item.get('APFSPhysicalStore')})
            if not physical_disks:
                physical_disks = [data.get('ParentWholeDisk') or data.get('DeviceIdentifier')]
            require(all(physical_disks), 'volume_identity_unavailable', 'Physical disk identity missing')
            require(not external_required or external, 'external_required', 'Use a mounted external volume')
            return {'mode': 'macos', 'volume_uuid': data['VolumeUUID'], 'disk_uuid': data.get('DiskUUID'),
                    'mount': mount, 'external': external, 'physical': ','.join(physical_disks), 'physical_disks': physical_disks,
                    'filesystem': data.get('FilesystemType', ''), 'block_size': data.get('VolumeAllocationBlockSize', 4096),
                    'root_inode': st.st_ino, 'device': st.st_dev, 'free': shutil.disk_usage(root).free,
                    'removable': bool(data.get('RemovableMedia') or data.get('Removable')),
                    'disk_image': data.get('BusProtocol') == 'Disk Image'}
        require(not external_required, 'unsupported_platform', 'External identity backend is currently macOS only')
        return {'mode': 'local', 'volume_uuid': str(st.st_dev), 'disk_uuid': None, 'mount': str(root),
                'external': False, 'physical': str(st.st_dev), 'filesystem': 'local', 'block_size': 4096,
                'root_inode': st.st_ino, 'device': st.st_dev, 'free': shutil.disk_usage(root).free, 'removable': False}

    def mounted_roots(self):
        """Only mount metadata; never recurse into source media."""
        if sys.platform != 'darwin':
            return []
        try:
            result = subprocess.run(['/usr/sbin/diskutil', 'list', '-plist', 'external'],
                                    capture_output=True, timeout=30)
        except subprocess.TimeoutExpired as error:
            raise MediaError('volume_identity_unavailable', 'Mount listing timed out; retry shortly') from error
        require(result.returncode == 0, 'volume_identity_unavailable', 'Cannot list external mounts')
        def mounts(nodes):
            for node in nodes:
                if node.get('MountPoint'):
                    yield node['MountPoint']
                for key in ('Partitions', 'APFSVolumes'):
                    yield from mounts(node.get(key, []))
        return sorted(set(mounts(plistlib.loads(result.stdout).get('AllDisksAndPartitions', []))))

    def eject(self, root, expected):
        require(sys.platform == 'darwin', 'unsupported_platform', 'Safe eject requires macOS')
        current = self.describe(root, external_required=True)
        validate_card(root, current)
        require(card_identity(current) == expected, 'volume_replaced', 'Card identity changed; select it again')
        # Normal eject only: no force, no fallback unmount, no arbitrary disk identifier from the caller.
        try:
            result = subprocess.run(['/usr/sbin/diskutil', 'eject', current['mount']],
                                    capture_output=True, text=True, timeout=30)
        except subprocess.TimeoutExpired as error:
            raise MediaError('eject_uncertain', 'Eject did not answer; refresh mounts before removing the card') from error
        require(result.returncode == 0, 'eject_failed', 'Could not eject card; close files and retry')

    def fast_check(self, root, token):
        st = os.stat(str(root), follow_symlinks=False)
        require(st.st_dev == token['device'] and st.st_ino == token['root_inode'],
                'volume_replaced', 'Volume disconnected or root changed')
        return token

    def check(self, record):
        current = self.describe(record['root'], record['identity']['external'])
        for key in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode'):
            require(current.get(key) == record['identity'].get(key), 'volume_replaced', 'Registered volume identity changed: ' + record['name'])
        return current

    def online(self, record):
        try:
            return self.check(record)
        except (OSError, MediaError, subprocess.SubprocessError, ValueError):
            return None


def independent(first, second):
    # A mounted image's bytes live on some other disk; it never counts as a separate replica.
    if first.get('disk_image') or second.get('disk_image'):
        return False
    left = set(first.get('physical_disks') or [first['physical']])
    right = set(second.get('physical_disks') or [second['physical']])
    return left.isdisjoint(right)


def card_identity(info):
    return {key: info.get(key) for key in
            ('mode', 'volume_uuid', 'disk_uuid', 'mount', 'physical', 'root_inode', 'device')}


def validate_card(root, info):
    require(info.get('external') and info.get('removable') and not info.get('disk_image')
            and info.get('mount') == str(Path(root).absolute()),
            'not_a_card', 'Choose the root of a mounted removable SD/camera card')


def destination_identities(catalog, backend):
    # disk numbers change on remount: compare current physical stores when available.
    return [backend.online(record) or record['identity'] for record in catalog.volumes()]


def reject_destination(info, identities):
    require(all(info.get('volume_uuid') != identity.get('volume_uuid')
                and independent(info, identity) for identity in identities),
            'destination_volume', 'Archive/work disks cannot be ejected from Ingreso')


class CachedBackend:
    """Read-only memo for one request: each mounted root is described once (df/diskutil are slow)."""

    def __init__(self, backend):
        self._backend, self._described = backend, {}

    def __getattr__(self, name):
        return getattr(self._backend, name)

    def describe(self, root, external_required=False):
        key = (str(Path(root).absolute()), bool(external_required))
        if key not in self._described:
            self._described[key] = self._backend.describe(root, external_required)
        return self._described[key]

    def online(self, record):
        try:
            return self.check(record)
        except (OSError, MediaError, subprocess.SubprocessError, ValueError):
            return None

    def check(self, record):
        current = self.describe(record['root'], record['identity']['external'])
        for key in ('mode', 'volume_uuid', 'disk_uuid', 'root_inode'):
            require(current.get(key) == record['identity'].get(key), 'volume_replaced', 'Registered volume identity changed: ' + record['name'])
        return current


def card_device(root, catalog=None, limit=64):
    """Device of the first still under DCIM (one bounded EXIF read; no deep scan on insertion)."""
    from .photo_meta import PHOTO_SUFFIXES, read
    from .recipes import recipe, resolve
    dcim = next((Path(root) / n for n in ('DCIM', 'dcim') if (Path(root) / n).is_dir()), None)
    if dcim is None:
        return None
    seen = 0
    try:
        folders = sorted(e.path for e in os.scandir(dcim) if e.is_dir(follow_symlinks=False) and not e.name.startswith('.'))
        for folder in folders:
            for entry in sorted(os.scandir(folder), key=lambda e: e.name):
                seen += 1
                if seen > limit:
                    return None
                if entry.is_file(follow_symlinks=False) and Path(entry.name).suffix.lower() in PHOTO_SUFFIXES:
                    found = resolve(read(entry.path), catalog, recipe(catalog))
                    if not found:
                        continue
                    flow = next((f for f in recipe(catalog).get('missions') or [] if found.get('category') in f.get('devices', [])), None)
                    return {k: found.get(k) for k in ('make', 'model', 'source', 'label', 'category', 'requires_confirmation')} | \
                        ({'mission_flow': flow['label']} if flow and not found['requires_confirmation'] else {})
    except OSError:
        return None
    return None


def card_hint(root, catalog=None, identity=None):
    root = Path(root)
    def names(path):
        try:
            with os.scandir(path) as entries:
                return {entry.name.lower() for entry in entries if not entry.name.startswith('.') or entry.name.endswith('.stalker')}
        except OSError:
            return set()
    from .recipes import card_family, card_parents, recipe
    rules = recipe(catalog)
    parents = card_parents(rules)
    top, second = names(root), {}
    if top & parents:
        with os.scandir(root) as entries:
            for entry in entries:
                if entry.name.lower() in parents and entry.is_dir(follow_symlinks=False):
                    second[entry.name.lower()] = names(entry.path)
    # Folders with content decide first: an empty residue (100CANON left by another camera) must not win over
    # 101MEDIA full of photos. A freshly formatted card (only empty folders) still gets its family.
    filled = {key: {n for n in values if names(os.path.join(root, key, n)) or not os.path.isdir(os.path.join(root, key, n))}
              for key, values in second.items()}
    rule = card_family(rules, top, filled)
    if rule is rules['card_default']:
        rule = card_family(rules, top, second)
    family = rule['family']
    device = card_device(root, catalog)
    try:
        usage = shutil.disk_usage(root)
        used, capacity = usage.used, usage.total
    except OSError:
        used = capacity = 0
    hint = {'family': family, 'recommendation': rule['recommendation'],
            'used_bytes': used, 'capacity_bytes': capacity}
    if device:
        hint['device'] = device
        if device.get('category') and not device.get('requires_confirmation'):
            # A known device needs no category question; a mission flow separates its stills.
            hint['recommendation'] = 'drone_missions' if device.get('mission_flow') else 'archive_and_work'
    if catalog is not None and identity and identity.get('volume_uuid'):
        row = catalog.db.execute(
            "SELECT json_extract(unit.value,'$.project') AS project, json_extract(unit.value,'$.profile') AS profile "
            "FROM jobs, json_each(jobs.plan,'$.units') AS unit "
            "WHERE json_extract(jobs.plan,'$.source_identity.volume_uuid')=? "
            "AND coalesce(json_extract(unit.value,'$.support'),0)=0 ORDER BY jobs.created_at DESC,unit.key LIMIT 1",
            (identity['volume_uuid'],)).fetchone()
        if row and row['project']:
            hint['last_project'], hint['last_profile'] = row['project'], row['profile'] or ''
    return hint


def source_cards(catalog, backend, hints=None):
    result = []
    protected = destination_identities(catalog, backend)
    # Without a registered destination online, mounts stay silent: no listing of card contents.
    if hints is None:
        hints = any(backend.online(record) for record in catalog.volumes())
    for root in backend.mounted_roots():
        try:
            info = backend.describe(root)
            validate_card(root, info)
            reject_destination(info, protected)
        except (OSError, MediaError, subprocess.SubprocessError, ValueError):
            continue
        card = {'path': str(root), 'name': Path(root).name, 'kind': 'card', 'identity': card_identity(info)}
        if hints:
            card['hint'] = card_hint(root, catalog, info)
        result.append(card)
    return result


def eject_card(catalog, backend, source, identity):
    require(isinstance(source, str) and isinstance(identity, dict), 'invalid_request', 'Source and identity required')
    # The same interprocess lock as copy/verify/cleanup. Conservatively blocks during any mutation.
    with catalog.writer():
        root = Path(source).expanduser().absolute()
        info = backend.describe(root, external_required=True)
        validate_card(root, info)
        reject_destination(info, destination_identities(catalog, backend))
        require(card_identity(info) == identity, 'volume_replaced', 'Card identity changed; select it again')
        backend.eject(root, identity)
    return {'ejected': True, 'source': str(root)}
