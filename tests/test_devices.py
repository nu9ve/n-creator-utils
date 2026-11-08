"""Device registry and drone stills (Atlas A15/A1) on synthetic EXIF/XMP fixtures only."""
import struct
import unittest

from media.common import MediaError
from media.photo_meta import read
from media.recipes import TEMPLATE
from media.service import Service
from tests.test_media_engine import MediaFixture

XMP = ('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description tiff:Model="Test_Pro" '
       'drone-dji:RelativeAltitude="+45.10" drone-dji:GimbalPitchDegree="-89.90" '
       'drone-dji:FlightYawDegree="-100.40"/></rdf:RDF></x:xmpmeta>')


def _ascii(tag, text):
    payload = text.encode() + b'\x00'
    return (tag, 2, payload, len(payload))


def _rationals(tag, values):
    return (tag, 5, b''.join(struct.pack('<II', int(v * 100), 100) for v in values), len(values))


def tiff(make, model, serial=None, when='2026:11:04 16:32:12', camera_serial=False, xmp=None, gps=True):
    ifd0 = [_ascii(0x010F, make), _ascii(0x0110, model)]
    if serial and camera_serial:
        ifd0.append(_ascii(0xC62F, serial))
    if xmp:
        ifd0.append((0x02BC, 1, xmp.encode(), len(xmp.encode())))
    exif = [_ascii(0x9003, when)] + ([_ascii(0xA431, serial)] if serial and not camera_serial else [])
    gps_ifd = [_ascii(1, 'N'), _rationals(2, (29, 5, 54.29)), _ascii(3, 'W'), _rationals(4, (110, 57, 44.18))] if gps else None
    ifd0.append((0x8769, 4, None, 1))
    if gps_ifd:
        ifd0.append((0x8825, 4, None, 1))
    ifd0.sort(key=lambda e: e[0])
    size = lambda entries: 2 + 12 * len(entries) + 4
    exif_at = 8 + size(ifd0)
    gps_at = exif_at + size(exif)
    data_at = gps_at + (size(gps_ifd) if gps_ifd else 0)
    data = bytearray()

    def encode(entries):
        out = struct.pack('<H', len(entries))
        for tag, kind, payload, count in entries:
            payload = struct.pack('<I', exif_at if tag == 0x8769 else gps_at) if payload is None else payload
            if len(payload) <= 4:
                field = payload.ljust(4, b'\x00')
            else:
                field = struct.pack('<I', data_at + len(data))
                data.extend(payload)
            out += struct.pack('<HHI', tag, kind, count) + field
        return out + b'\x00' * 4

    body = encode(ifd0) + encode(exif) + (encode(gps_ifd) if gps_ifd else b'')
    return b'II*\x00' + struct.pack('<I', 8) + body + bytes(data)


def segment(marker, body):
    return b'\xff' + bytes([marker]) + struct.pack('>H', len(body) + 2) + body


def jpeg(make='DJI', model='FC3170', serial='1TCLH9N03BJ5N9', when='2026:11:04 16:32:12', flight=True):
    out = b'\xff\xd8' + segment(0xE1, b'Exif\x00\x00' + tiff(make, model, serial, when))
    if flight:
        out += segment(0xE1, b'http://ns.adobe.com/xap/1.0/\x00' + XMP.encode())
    return out + segment(0xDA, b'\x00' * 8) + b'pixels' + b'\xff\xd9'


def dng(make='DJI', model='FC3170', serial='1TCLH9N03BJ5N9', when='2026:11:04 16:32:12', flight=True):
    return tiff(make, model, serial, when, camera_serial=True, xmp=XMP if flight else None) + b'raw-pixels'


class PhotoMetaTests(MediaFixture):
    def test_jpeg_and_dng_identify_the_same_device_and_flight(self):
        (self.root / 'a.JPG').write_bytes(jpeg())
        (self.root / 'a.DNG').write_bytes(dng())
        for name in ('a.JPG', 'a.DNG'):
            found = read(self.root / name)
            self.assertEqual((found['make'], found['model'], found['serial']), ('DJI', 'FC3170', '1TCLH9N03BJ5N9'))
            self.assertEqual(found['captured_at'], '2026-11-04T16:32:12')
            self.assertEqual(found['flight'], {'relative_altitude': 45.1, 'gimbal_pitch': -89.9, 'flight_yaw': -100.4})
            self.assertAlmostEqual(found['gps']['lat'], 29.0984139, places=5)
            self.assertLess(found['gps']['lon'], 0)

    def test_damaged_or_foreign_files_give_nothing(self):
        for name, data in (('x.JPG', b'photo-data'), ('y.DNG', b'II*\x00\xff\xff\xff\xff'), ('z.JPG', b'\xff\xd8\xff\xe1\x00')):
            (self.root / name).write_bytes(data)
            self.assertEqual(read(self.root / name), {})


class DroneStillTests(MediaFixture):
    def setUp(self):
        super().setUp()
        for name in ('photo.JPG', 'photo.DNG'):
            (self.source / 'DCIM' / name).unlink()
        self.folder = self.source / 'DCIM/101MEDIA'
        self.folder.mkdir()
        (self.source / 'MISC/THM/101').mkdir(parents=True)
        (self.source / 'DCIM/100CANON').mkdir()

    def pair(self, stem, folder=None, **facts):
        folder = folder or self.folder
        folder.mkdir(parents=True, exist_ok=True)
        (folder / (stem + '.JPG')).write_bytes(jpeg(**facts))
        (folder / (stem + '.DNG')).write_bytes(dng(**facts))

    def test_known_drone_is_classified_by_exif_and_named_by_the_recipe(self):
        self.pair('DJI_0001')
        plan = self.plan()
        unit = next(u for u in plan['units'] if not u.get('support'))
        self.assertEqual((unit['camera'], unit['kind']), ('dji-drone', 'camera_bundle'))
        self.assertEqual(unit['device'], {'make': 'DJI', 'model': 'FC3170', 'serial': '1TCLH9N03BJ5N9', 'source': 'MAVICAIR2',
                                          'label': 'DJI Mavic Air 2', 'rule': 'template', 'category': 'dji-drone'})
        self.assertEqual(plan['recipe']['id'], 'nu9ve-default')
        layout = plan['layout']
        self.assertEqual(layout['DCIM/101MEDIA/DJI_0001.JPG']['archive'], 'dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG')
        self.assertEqual(layout['DCIM/101MEDIA/DJI_0001.DNG']['archive'], 'dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].DNG')
        self.run_plan(plan)
        self.assertTrue((self.archive / '001_raw/dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].DNG').is_file())

    def test_restarted_dji_numbering_never_pairs_across_dcim_folders(self):
        self.pair('DJI_0342', when='2026:05:30 10:00:00')
        self.pair('DJI_0342', folder=self.source / 'DCIM/102MEDIA', when='2026:11:05 17:33:00')
        plan = self.plan()
        units = [u for u in plan['units'] if not u.get('support')]
        self.assertEqual(sorted(len(u['paths']) for u in units), [2, 2])
        names = {plan['layout'][p]['archive'] for u in units for p in u['paths']}
        self.assertEqual(len(names), 4)
        self.assertIn('dron/fotos/2026/05/26MAY30-100000_MAVICAIR2_MX-INTRO[].JPG', names)

    def test_dji_without_flight_evidence_or_new_model_still_asks(self):
        self.pair('DJI_0001', model='FC9999', flight=False)
        with self.assertRaisesRegex(MediaError, 'Classify DJI') as raised:
            self.plan()
        self.assertNotIn('suggested', str(raised.exception))
        for name in ('DJI_0001.JPG', 'DJI_0001.DNG'):
            (self.folder / name).unlink()
        self.pair('DJI_0001', model='FC9999', serial='NEWSERIAL')
        with self.assertRaisesRegex(MediaError, 'suggested: dji-drone'):
            self.plan()
        explicit = self.plan(assignments={'DCIM/101MEDIA/DJI_0001.JPG': {'camera': 'dji-drone'},
                                          'DCIM/101MEDIA/DJI_0001.DNG': {'camera': 'dji-drone'}})
        self.assertEqual(next(u for u in explicit['units'] if not u.get('support'))['device']['rule'], 'flight_evidence')

    def test_confirmed_device_is_remembered_and_wins_over_the_template(self):
        service = Service(self.catalog, self.backend)
        self.pair('DJI_0001', model='FC9999', serial='NEWSERIAL')
        service.call('confirm_device', {'make': 'DJI', 'model': 'FC9999', 'serial': 'NEWSERIAL',
                                        'category': 'dji-drone', 'source': 'Mini 4', 'label': 'DJI Mini 4'})
        listing = service.call('devices')
        self.assertEqual(listing['recipe']['id'], 'nu9ve-default')
        self.assertEqual([d['source'] for d in listing['registry']], ['MINI4'])
        plan = self.plan()
        unit = next(u for u in plan['units'] if not u.get('support'))
        self.assertEqual((unit['camera'], unit['device']['rule'], unit['device']['source']), ('dji-drone', 'registry', 'MINI4'))
        self.assertIn('_MINI4_', plan['layout']['DCIM/101MEDIA/DJI_0001.JPG']['archive'])
        with self.assertRaises(MediaError):
            service.call('confirm_device', {'make': 'DJI', 'model': 'FC9999', 'serial': '', 'category': 'dji-drone'})
        with self.assertRaises(MediaError):
            service.call('confirm_device', {'make': 'DJI', 'model': 'FC9999', 'serial': 'X', 'category': 'drone'})

    def test_a_pair_from_two_devices_is_not_classified(self):
        self.folder.mkdir(exist_ok=True)
        (self.folder / 'DJI_0001.JPG').write_bytes(jpeg())
        (self.folder / 'DJI_0001.DNG').write_bytes(dng(serial='OTHER'))
        with self.assertRaisesRegex(MediaError, 'Classify DJI'):
            self.plan()

    def test_template_is_data_for_other_setups(self):
        # The user's devices are only the initial template; nothing in the planner names a drone model.
        self.assertEqual(TEMPLATE['schema'], 'nu9ve-recipe/1')
        import inspect
        import media.layout
        import media.planner
        for module in (media.planner, media.layout):
            self.assertNotIn('FC3170', inspect.getsource(module))
            self.assertNotIn('MAVICAIR2', inspect.getsource(module))


if __name__ == '__main__':
    unittest.main()
