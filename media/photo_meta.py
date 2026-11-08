"""Bounded, read-only EXIF/XMP facts of a still (JPEG or TIFF/DNG). No dependencies.

Only what identifies the device and the capture: Make, Model, serial, DateTimeOriginal (+ offset),
GPS and the DJI flight XMP (`drone-dji:*`). Never raises for a damaged or foreign file: returns {}.
The XMP `tiff:Model` of DJI files is unreliable (observed `Test_Pro`), so Model always comes from EXIF.
"""
import re
import struct

PHOTO_SUFFIXES = {'.jpg', '.jpeg', '.dng'}
JPEG_HEAD = 1024 * 1024
MAX_ENTRIES = 512
MAX_VALUE = 256 * 1024
XMP_HEADER = b'http://ns.adobe.com/xap/1.0/\x00'
FLIGHT_KEYS = {'GimbalPitchDegree': 'gimbal_pitch', 'GimbalYawDegree': 'gimbal_yaw', 'GimbalRollDegree': 'gimbal_roll',
               'FlightYawDegree': 'flight_yaw', 'FlightPitchDegree': 'flight_pitch', 'FlightRollDegree': 'flight_roll',
               'RelativeAltitude': 'relative_altitude', 'AbsoluteAltitude': 'absolute_altitude'}
TYPE_SIZES = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 7: 1, 9: 4, 10: 8}


class _Tiff:
    def __init__(self, read, base=0):
        self.read, self.base = read, base
        order = read(base, 2)
        if order not in (b'II', b'MM'):
            raise ValueError('not a TIFF stream')
        self.end = '<' if order == b'II' else '>'
        magic, self.first = struct.unpack(self.end + 'HI', read(base + 2, 6))
        if magic != 42:
            raise ValueError('not a TIFF stream')

    def ifd(self, offset):
        count = struct.unpack(self.end + 'H', self.read(self.base + offset, 2))[0]
        if count > MAX_ENTRIES:
            raise ValueError('implausible IFD')
        raw = self.read(self.base + offset + 2, 12 * count)
        entries = {}
        for i in range(count):
            tag, kind, n, value = struct.unpack(self.end + 'HHI4s', raw[12 * i:12 * i + 12])
            entries[tag] = (kind, n, value)
        return entries

    def value(self, entry):
        kind, n, inline = entry
        size = TYPE_SIZES.get(kind, 0) * n
        if not size or size > MAX_VALUE:
            return None
        data = inline[:size] if size <= 4 else self.read(self.base + struct.unpack(self.end + 'I', inline)[0], size)
        if len(data) < size:
            return None
        if kind == 2:
            return data.split(b'\x00', 1)[0].decode('utf-8', 'replace').strip() or None
        if kind in (1, 7):
            return data
        if kind == 3:
            return struct.unpack(self.end + 'H' * n, data)
        if kind in (4, 9):
            return struct.unpack(self.end + ('I' if kind == 4 else 'i') * n, data)
        pairs = struct.unpack(self.end + ('I' if kind == 5 else 'i') * (2 * n), data)
        return tuple(pairs[i] / pairs[i + 1] if pairs[i + 1] else None for i in range(0, len(pairs), 2))

    def offset(self, entries, tag):
        value = self.value(entries[tag]) if tag in entries else None
        return value[0] if value else None


def _degrees(values, ref):
    if not values or len(values) != 3 or None in values:
        return None
    result = values[0] + values[1] / 60 + values[2] / 3600
    return round(-result if ref in ('S', 'W') else result, 7)


def _tiff_facts(read, base=0):
    tiff = _Tiff(read, base)
    ifd0 = tiff.ifd(tiff.first)
    found = {'make': tiff.value(ifd0[0x010F]) if 0x010F in ifd0 else None,
             'model': tiff.value(ifd0[0x0110]) if 0x0110 in ifd0 else None,
             'serial': tiff.value(ifd0[0xC62F]) if 0xC62F in ifd0 else None}
    xmp = tiff.value(ifd0[0x02BC]) if 0x02BC in ifd0 else None
    exif_at = tiff.offset(ifd0, 0x8769)
    if exif_at:
        exif = tiff.ifd(exif_at)
        found['serial'] = found['serial'] or (tiff.value(exif[0xA431]) if 0xA431 in exif else None)
        when = tiff.value(exif[0x9003]) if 0x9003 in exif else None
        if when and re.fullmatch(r'\d{4}:\d{2}:\d{2} \d{2}:\d{2}:\d{2}', when):
            found['captured_at'] = when[:10].replace(':', '-') + 'T' + when[11:]
        offset = tiff.value(exif[0x9011]) if 0x9011 in exif else None
        if offset and re.fullmatch(r'[+-]\d{2}:\d{2}', offset):
            found['offset'] = offset
    gps_at = tiff.offset(ifd0, 0x8825)
    if gps_at:
        gps = tiff.ifd(gps_at)
        get = lambda tag: tiff.value(gps[tag]) if tag in gps else None
        lat, lon = _degrees(get(2), get(1)), _degrees(get(4), get(3))
        if lat is not None and lon is not None:
            found['gps'] = {'lat': lat, 'lon': lon}
            altitude = get(6)
            if altitude and altitude[0] is not None:
                found['gps']['alt'] = round(-altitude[0] if (get(5) or b'\x00')[:1] == b'\x01' else altitude[0], 2)
    return found, xmp


def _xmp_facts(xmp):
    if isinstance(xmp, bytes):
        xmp = xmp.decode('utf-8', 'replace')
    flight = {}
    for key, name in FLIGHT_KEYS.items():
        match = (re.search(r'drone-dji:%s="\s*([+-]?[0-9.]+)\s*"' % key, xmp)
                 or re.search(r'<drone-dji:%s>\s*([+-]?[0-9.]+)\s*<' % key, xmp))
        if match:
            try:
                flight[name] = float(match.group(1))
            except ValueError:
                pass
    return flight


def _jpeg(handle):
    head = handle.read(JPEG_HEAD)
    if head[:2] != b'\xff\xd8':
        return {}, None
    position, exif, xmp = 2, None, None
    while position + 4 <= len(head) and head[position] == 0xFF:
        marker = head[position + 1]
        if marker in (0xD9, 0xDA):  # end of image / start of scan: no more metadata
            break
        length = struct.unpack('>H', head[position + 2:position + 4])[0]
        body = head[position + 4:position + 2 + length]
        if marker == 0xE1 and body.startswith(b'Exif\x00\x00') and exif is None:
            exif = body[6:]
        elif marker == 0xE1 and body.startswith(XMP_HEADER) and xmp is None:
            xmp = body[len(XMP_HEADER):]
        position += 2 + length
    if exif is None:
        return {}, xmp
    found, embedded = _tiff_facts(lambda at, size: exif[at:at + size])
    return found, xmp or embedded


def read(path):
    """{make, model, serial, captured_at, offset, gps, flight} for a JPEG/DNG; {} when absent or unreadable."""
    try:
        with open(path, 'rb') as handle:
            if str(path).lower().endswith('.dng'):
                def at(offset, size):
                    handle.seek(offset)
                    return handle.read(size)
                found, xmp = _tiff_facts(at)
            else:
                found, xmp = _jpeg(handle)
    except (OSError, ValueError, struct.error, KeyError, IndexError, TypeError):
        return {}
    found = {k: v for k, v in found.items() if v is not None}
    flight = _xmp_facts(xmp) if xmp else {}
    if flight:
        found['flight'] = flight
    return found if found.get('make') or found.get('captured_at') else {}
