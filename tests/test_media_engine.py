import copy
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
import uuid

from media.catalog import Catalog
from media.common import MediaError, Tree, content_stamp
from media.library import deliver_work, handoff, resolve, restore, set_work_state
from media.planner import build_plan, destination_path
from media.service import Service
from media.transfer import Transfer
from media.volumes import VolumeBackend


class FixtureVolumes(VolumeBackend):
    def __init__(self):
        self.offline = set()
        self.low_space = set()
        self.physical = {}
        self.replaced = set()

    def fast_check(self, root, token):
        info = self.describe(root)
        if any(info[k] != token[k] for k in ('volume_uuid', 'device', 'root_inode')):
            raise MediaError('volume_replaced', 'Fixture volume changed')
        return info

    def describe(self, root, external_required=False):
        root = Path(root).absolute()
        if str(root) in self.offline or not root.is_dir():
            raise MediaError('volume_offline', 'Fixture disk disconnected')
        st = root.stat()
        return {'mode': 'fixture', 'volume_uuid': str(root) + ('-replaced' if str(root) in self.replaced else ''),
                'disk_uuid': 'fixture', 'mount': str(root), 'external': True,
                'physical': self.physical.get(str(root), str(root)), 'filesystem': 'fixture', 'block_size': 4096,
                'root_inode': st.st_ino, 'device': st.st_dev, 'free': 0 if str(root) in self.low_space else 10**12,
                'removable': True}


class MediaFixture(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.source, self.archive, self.work = [self.root / name for name in ('card', 'archive', 'work')]
        for root in (self.source, self.archive, self.work): root.mkdir()
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)
        self.backend = FixtureVolumes()
        self.a = self.catalog.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        self.w = self.catalog.register('ESSD', 'work', self.work, self.backend.describe(self.work), '001_proyectos')
        (self.source / 'DCIM').mkdir()
        (self.source / 'DCIM/photo.JPG').write_bytes(b'photo-data')
        (self.source / 'DCIM/photo.DNG').write_bytes(b'raw-data')

    def plan(self, **overrides):
        params = dict(source=str(self.source), archive='MEDIA', work='ESSD', project='intro', profile='e64', cleanup=True)
        params.update(overrides)
        return build_plan(self.catalog, self.backend, **params)

    def run_plan(self, plan=None, hook=None, cancel=None):
        plan = plan or self.plan()
        return Transfer(self.catalog, self.backend, cancel, hook).apply(plan, plan['digest'])


class MediaEngineTests(MediaFixture):
    def test_verified_both_destinations_then_cleanup_and_idempotent_retry(self):
        plan = self.plan()
        result = self.run_plan(plan)
        self.assertEqual(result['state'], 'completed')
        self.assertEqual(list(self.source.rglob('*.*')), [])
        for volume in (self.a, self.w):
            self.assertEqual(len(list(Path(volume['root']).rglob('*.JPG'))), 1)
            self.assertEqual(next(Path(volume['root']).rglob('*.DNG')).read_bytes(), b'raw-data')
            self.assertTrue((Path(volume['root']) / '.ncreator/receipts' / (plan['id'] + '.json')).is_file())
        self.assertEqual(self.run_plan(plan)['state'], 'completed')
        self.assertEqual(len(list(self.archive.rglob('*.JPG'))), 1)
        assets = self.catalog.search()
        self.assertEqual(len(assets), 1)  # JPG+DNG stay paired.
        self.assertEqual(assets[0]['work_delivery_status'], 'ready')

    def test_single_read_fans_out_and_reports_progress(self):
        from unittest import mock
        opened = []
        real_open = Tree.open
        def spy(tree, rel, *args, **kwargs):
            if tree.root == self.source: opened.append(rel)
            return real_open(tree, rel, *args, **kwargs)
        with mock.patch.object(Tree, 'open', spy):
            result = self.run_plan()
        self.assertEqual(result['state'], 'completed')
        # Copy read + pre-cleanup re-read per file; never a second copy read for the other destination.
        self.assertEqual(sorted(opened), sorted(['DCIM/photo.DNG', 'DCIM/photo.JPG'] * 2))
        progress = result['progress']
        self.assertEqual((progress['copied_bytes'], progress['bytes_total']), (18, 18))
        self.assertEqual((progress['verified_bytes'], progress['verify_total']), (36, 36))
        self.assertEqual((progress['files_done'], progress['phase']), (2, 'completed'))

    def test_no_media_writes_by_plan_and_stable_plan_digest(self):
        first = self.plan(cleanup=False)
        second = self.plan(cleanup=False)
        self.assertEqual(first, second)
        self.assertEqual(list(self.archive.iterdir()), [])
        self.assertEqual(list(self.work.iterdir()), [])
        self.assertEqual(self.catalog.jobs(), [])

    def test_missing_work_requires_explicit_exception_then_deferred_delivery(self):
        self.backend.offline.add(str(self.work))
        with self.assertRaises(MediaError): self.plan()
        with self.assertRaisesRegex(MediaError, 'Confirm'): self.plan(archive_only=True)
        plan = self.plan(archive_only=True, confirm_archive_only=True)
        self.assertEqual(self.run_plan(plan)['state'], 'completed')
        pending = self.catalog.search(work_state='pending')
        self.assertEqual(len(pending), 1)
        self.assertFalse(any(self.source.rglob('*.JPG')))
        self.backend.offline.clear()
        deliver_work(self.catalog, self.backend, [pending[0]['id']], 'ESSD')
        deliver_work(self.catalog, self.backend, [pending[0]['id']], 'ESSD')
        self.assertEqual(len(list(self.work.rglob('*.JPG'))), 1)
        self.assertEqual(self.catalog.search()[0]['work_delivery_status'], 'ready')

    def test_explicit_skip_does_not_remain_pending(self):
        plan = self.plan(archive_only=True, confirm_archive_only=True, work_decision='skipped')
        self.run_plan(plan)
        self.assertEqual(self.catalog.search(work_state='pending'), [])
        asset = self.catalog.search()[0]
        set_work_state(self.catalog, [asset['id']], 'pending')
        self.assertEqual(len(self.catalog.search(work_state='pending')), 1)

    def test_corruption_with_same_size_never_cleans_source(self):
        def corrupt(stage, data):
            if stage == 'before_verify':
                p = Path(data['root']) / data['path']
                p.write_bytes(b'x' * p.stat().st_size)
        with self.assertRaisesRegex(MediaError, 'verification failed'):
            self.run_plan(hook=corrupt)
        self.assertEqual((self.source / 'DCIM/photo.JPG').read_bytes(), b'photo-data')

    def test_interrupted_copy_resumes_without_duplicate(self):
        plan = self.plan()
        def fail(stage, data):
            if stage == 'after_file_verified': raise OSError('simulated disconnect')
        with self.assertRaises(OSError): self.run_plan(plan, hook=fail)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        self.assertEqual(self.run_plan(plan)['state'], 'completed')
        self.assertEqual(len(list(self.archive.rglob('*.JPG'))), 1)
        self.assertEqual(list(self.archive.rglob('*.ncpartial')), [])

    def test_partial_cleanup_resumes_from_journal(self):
        plan = self.plan()
        counter = [0]
        def fail(stage, data):
            if stage == 'cleanup_file':
                counter[0] += 1
                if counter[0] == 2: raise OSError('power loss')
        with self.assertRaises(OSError): self.run_plan(plan, hook=fail)
        self.assertEqual(len(list(self.source.rglob('*.*'))), 1)
        self.assertEqual(self.run_plan(plan)['state'], 'completed')

    def test_new_file_before_cleanup_blocks_all_deletion(self):
        def add(stage, data):
            if stage == 'before_cleanup': (self.source / 'new.mp4').write_bytes(b'new recording')
        with self.assertRaisesRegex(MediaError, 'membership changed'): self.run_plan(hook=add)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        self.assertTrue((self.source / 'new.mp4').exists())

    def test_new_file_during_cleanup_is_kept_and_reported(self):
        plan = self.plan()
        def add(stage, data):
            if stage == 'cleanup_file' and not (self.source / 'new.mp4').exists(): (self.source / 'new.mp4').write_bytes(b'new recording')
        self.assertEqual(self.run_plan(plan, hook=add)['state'], 'source_has_new_material')
        self.assertFalse((self.source / 'DCIM/photo.JPG').exists())
        self.assertEqual((self.source / 'new.mp4').read_bytes(), b'new recording')
        self.assertEqual(self.run_plan(plan)['state'], 'source_has_new_material')  # Terminal; no repeated cleanup.
        self.assertTrue((self.source / 'new.mp4').exists())

    def test_transient_bad_source_read_never_cleans_card(self):
        # One corrupted SD read feeds both replicas; the fresh re-read before unlink must catch it.
        from unittest import mock
        real_read, target = os.read, (self.source / 'DCIM/photo.JPG').stat().st_ino
        state = {'corrupted': False}
        def flaky(fd, size):
            block = real_read(fd, size)
            if block and not state['corrupted'] and os.fstat(fd).st_ino == target:
                state['corrupted'] = True
                return b'X' * len(block)
            return block
        with mock.patch('os.read', side_effect=flaky):
            with self.assertRaisesRegex(MediaError, 'Source re-read differs'):
                self.run_plan()
        self.assertEqual((self.source / 'DCIM/photo.JPG').read_bytes(), b'photo-data')

    def test_finder_metadata_does_not_block_or_remain(self):
        def browse(stage, data):
            if stage == 'before_cleanup':
                (self.source / '.DS_Store').write_bytes(b'finder')
                (self.source / 'DCIM/._photo.JPG').write_bytes(b'appledouble')
        self.assertEqual(self.run_plan(hook=browse)['state'], 'completed')
        self.assertFalse((self.source / 'DCIM/photo.JPG').exists())
        self.assertTrue((self.source / '.DS_Store').exists())  # Metadata is ignored, not deleted.

    def test_crash_with_partial_copy_is_resumable(self):
        plan = self.plan()
        def crash(stage, data):
            if stage == 'before_verify': raise OSError('power loss before rename')
        with self.assertRaises(OSError): self.run_plan(plan, hook=crash)
        self.assertTrue(list(self.work.rglob('*.ncpartial')))
        self.assertEqual(self.run_plan(plan)['state'], 'completed')
        self.assertEqual(list(self.work.rglob('*.ncpartial')), [])

    def test_disk_image_is_never_an_independent_replica(self):
        from media.volumes import independent
        self.assertTrue(independent({'physical': 'disk4'}, {'physical': 'disk6'}))
        self.assertFalse(independent({'physical': 'disk4'}, {'physical': 'disk9', 'disk_image': True}))

    def test_source_modified_before_or_during_copy_blocks_cleanup(self):
        plan = self.plan()
        (self.source / 'DCIM/photo.JPG').write_bytes(b'new version')
        with self.assertRaisesRegex(MediaError, 'changed'): self.run_plan(plan)
        self.assertEqual(list(self.archive.iterdir()), [])

    def test_destination_disconnect_before_cleanup_keeps_card(self):
        def offline(stage, data):
            if stage == 'before_cleanup': self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError): self.run_plan(hook=offline)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())

    def test_wrong_disk_same_name_and_low_space_block_before_copy(self):
        self.backend.replaced.add(str(self.archive))
        with self.assertRaises(MediaError): self.plan()
        self.backend.replaced.clear(); self.backend.low_space.add(str(self.work))
        with self.assertRaisesRegex(MediaError, 'space'): self.plan()
        self.assertEqual(list(self.archive.iterdir()), [])

    def test_same_physical_disks_and_overlap_rejected(self):
        self.backend.physical[str(self.work)] = str(self.archive)
        with self.assertRaisesRegex(MediaError, 'physical'): self.plan()
        self.backend.physical.clear()
        with self.assertRaisesRegex(MediaError, 'overlap'): self.plan(source=str(self.root), cleanup=False)

    def test_links_source_and_target_never_followed(self):
        (self.source / 'linked').symlink_to(self.archive, target_is_directory=True)
        with self.assertRaises(MediaError): self.plan()
        (self.source / 'linked').unlink()
        plan = self.plan()
        (self.archive / '001_raw').symlink_to(self.work, target_is_directory=True)
        with self.assertRaises(OSError): self.run_plan(plan)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())

    def test_foreign_destination_not_overwritten(self):
        plan = self.plan()
        destination = self.work / destination_path(self.w, plan, plan['units'][0], plan['files'][0]['path'])
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b'other!' + b'x' * (plan['files'][0]['stamp']['size'] - 6))  # Same size, other bytes.
        with self.assertRaisesRegex(MediaError, 'different file'): self.run_plan(plan)
        self.assertTrue(destination.read_bytes().startswith(b'other!'))
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        destination.write_bytes(b'different size')
        with self.assertRaisesRegex(MediaError, 'different file'): self.plan()  # Caught at review time.

    def test_identical_existing_destination_is_adopted(self):
        plan = self.plan()
        for file in plan['files']:
            unit = next(u for u in plan['units'] if u['id'] == file['unit_id'])
            target = self.archive / destination_path(self.a, plan, unit, file['path'])
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.source / file['path'], target)
        self.assertEqual(self.run_plan(plan)['state'], 'completed')
        self.assertEqual(len(list(self.archive.rglob('*.JPG'))), 1)

    def test_plan_digest_and_mutation_lock(self):
        plan = self.plan(); changed = copy.deepcopy(plan); changed['cleanup'] = False
        with self.assertRaises(MediaError): self.run_plan(changed)
        with self.catalog.writer():
            other = Catalog(self.catalog.home)
            try:
                with self.assertRaisesRegex(MediaError, 'running'):
                    Transfer(other, self.backend).apply(plan, plan['digest'])
            finally: other.close()

    def test_sony_preserves_card_for_camera_cleanup(self):
        sony = self.source / 'PRIVATE/M4ROOT/CLIP'
        sony.mkdir(parents=True); (sony / 'C0001.MP4').write_bytes(b'sony')
        plan = self.plan()
        self.assertEqual(self.run_plan(plan)['state'], 'camera_action_required')
        self.assertTrue((sony / 'C0001.MP4').exists())

    def test_dji_requires_per_file_classification_and_mixed_assignments(self):
        (self.source / 'DJI_001.MP4').write_bytes(b'dji')
        with self.assertRaisesRegex(MediaError, 'Classify DJI'): self.plan()
        plan = self.plan(assignments={'DJI_001.MP4': {'camera': 'dji-360', 'project': 'other'}})
        self.assertEqual({u['project'] for u in plan['units']}, {'intro', 'other'})
        self.run_plan(plan)

    def test_pair_assignment_conflicts_rejected(self):
        with self.assertRaisesRegex(MediaError, 'together'):
            self.plan(assignments={'DCIM/photo.JPG': {'project': 'a'}, 'DCIM/photo.DNG': {'project': 'b'}})

    def test_restore_after_source_cleanup_and_handoff(self):
        self.run_plan()
        asset = self.catalog.search()[0]
        output = self.root / 'downloads'; output.mkdir()
        restored = restore(self.catalog, self.backend, asset['id'], output)
        self.assertTrue(restored['verified'])
        self.assertEqual(next(output.rglob('photo.DNG')).read_bytes(), b'raw-data')
        exported = handoff(self.catalog, self.backend, [asset['id']])
        self.assertEqual(exported['schema'], 'creator-media-handoff/1')
        self.assertEqual(len(exported['assets'][0]['locations']), 2)
        self.assertEqual(len(exported['assets'][0]['members']), 2)
        self.backend.offline.add(str(self.archive))
        located = resolve(self.catalog, self.backend, asset['id'])
        self.assertTrue(located['locations'][0]['online'])
        self.assertEqual(located['locations'][0]['role'], 'work')

    def test_silent_without_registered_online_destinations(self):
        self.backend.offline = {str(self.archive), str(self.work)}
        result = Service(self.catalog, self.backend).call('suggest', {'source': '/does/not/exist'})
        self.assertFalse(result['show'])  # Must not even inspect source.
        self.backend.offline.clear()
        self.assertTrue(Service(self.catalog, self.backend).call('suggest', {'source': str(self.source)})['show'])

    def test_cancel_before_copy_keeps_source_and_can_resume(self):
        plan = self.plan()
        with self.assertRaisesRegex(MediaError, 'cancelled'): self.run_plan(plan, cancel=lambda: True)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        self.assertEqual(self.run_plan(plan)['state'], 'completed')


class StalkerEngineTests(MediaFixture):
    def session(self, name='sample.stalker', status='completed'):
        root = self.source / name; root.mkdir()
        (root / 'screen.mov').write_bytes(b'screen')
        (root / 'sources').mkdir(); (root / 'sources/mic.mov').write_bytes(b'raw microphone')
        (root / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3,
            'sessionID': str(uuid.uuid4()), 'status': status,
            'tracks': [{'relativePath': 'screen.mov'}]}))
        return root

    def test_stalker_complete_bundle_and_recording_untouched(self):
        bundle = self.session(); recording = self.session('active.stalker', 'recording')
        plan = self.plan(source_kind='stalker', cleanup=False)
        self.assertEqual(plan['skipped'], ['active.stalker'])
        self.run_plan(plan)
        self.assertTrue((recording / 'screen.mov').exists())
        saved = next(self.archive.rglob('*_STALKER_*.stalker'))
        self.assertEqual((saved / 'sources/mic.mov').read_bytes(), b'raw microphone')
        asset = next(a for a in self.catalog.search() if a['kind'] == 'stalker_session')
        exported = handoff(self.catalog, self.backend, [asset['id']])
        self.assertTrue(exported['assets'][0]['locations'][0]['relative_path'].endswith('.stalker'))
        self.assertEqual({m['relative_path'] for m in exported['assets'][0]['members']},
                         {'manifest.json', 'screen.mov', 'sources/mic.mov'})

    def test_stalker_manifest_changed_after_plan_blocks_copy(self):
        bundle = self.session()
        plan = self.plan(source_kind='stalker', cleanup=False)
        manifest = json.loads((bundle / 'manifest.json').read_text()); manifest['status'] = 'finalizing'
        (bundle / 'manifest.json').write_text(json.dumps(manifest))
        with self.assertRaises(MediaError): self.run_plan(plan)
        self.assertEqual(list(self.archive.iterdir()), [])

class NamedLayoutTests(MediaFixture):
    SONY_XML = """<?xml version="1.0" encoding="UTF-8"?>
<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00">
  <Duration value="30210"/>
  <CreationDate value="2026-10-24T19:46:50-07:00"/>
  <VideoFormat><VideoFrame videoCodec="AVC_1920_1080_HP@L42" captureFps="59.94p" formatFps="59.94p"/>
  <VideoLayout pixel="1920" numOfVerticalLine="1080" aspectRatio="16:9"/></VideoFormat>
  <Device manufacturer="Sony" modelName="ILCE-6400" serialNo="1"/>
</NonRealTimeMeta>"""

    def sony_card(self):
        card = self.root / 'sony'
        for rel, data in {'PRIVATE/M4ROOT/CLIP/C0001.MP4': b'clip-one', 'PRIVATE/M4ROOT/CLIP/C0001M01.XML': self.SONY_XML.encode(),
                          'PRIVATE/M4ROOT/THMBNL/C0001T01.JPG': b'thumb', 'PRIVATE/M4ROOT/MEDIAPRO.XML': b'<index/>',
                          'PRIVATE/SONY/SONYCARD.IND': b'ind', 'AVF_INFO/AVIN0001.BNP': b'bnp'}.items():
            (card / rel).parent.mkdir(parents=True, exist_ok=True)
            (card / rel).write_bytes(data)
        return card

    def test_sony_card_uses_dated_names_and_work_gets_only_clips(self):
        card = self.sony_card()
        plan = self.plan(source=str(card), project='debloateando mi tele', cleanup=False)
        name = '26OCT24-194650_SONYA6400_08-24_MX-DEBLOATEANDO-MI-TELE[]'
        self.assertEqual(plan['layout']['PRIVATE/M4ROOT/CLIP/C0001.MP4'],
                         {'archive': 'videos/2026/10/' + name + '.MP4', 'work': 'e64_raw/debloateando-mi-tele/' + name + '.MP4'})
        self.assertEqual(plan['layout']['PRIVATE/M4ROOT/CLIP/C0001M01.XML'], {'archive': 'metadata/sony/2026/10/' + name + '.XML', 'work': None})
        self.assertEqual(plan['layout']['PRIVATE/M4ROOT/THMBNL/C0001T01.JPG']['work'], None)
        self.assertTrue(plan['layout']['AVF_INFO/AVIN0001.BNP']['archive'].startswith(
            'metadata/sony/support/2026/10/2410-194650_MX-DEBLOATEANDO-MI-TELE_SONYA6400/AVF_INFO/'))
        self.assertEqual(self.run_plan(plan)['state'], 'source_retained')
        work_files = sorted(p.relative_to(self.work).as_posix() for p in self.work.rglob('*') if p.is_file() and '.ncreator' not in p.parts)
        self.assertEqual(work_files, ['001_proyectos/e64_raw/debloateando-mi-tele/' + name + '.MP4'])
        self.assertEqual((self.archive / '001_raw/videos/2026/10' / (name + '.MP4')).read_bytes(), b'clip-one')
        self.assertEqual(self.catalog.search(work_state='pending'), [])  # Support units are not work.
        clip = next(a for a in self.catalog.search() if 'PRIVATE/M4ROOT/CLIP/C0001.MP4' in a['paths'])
        exported = handoff(self.catalog, self.backend, [clip['id']])['assets'][0]
        self.assertEqual({l['role'] for l in exported['locations']}, {'archive', 'work'})
        self.assertEqual([m['relative_path'] for m in exported['members']], [name + '.MP4'])

    def test_stalker_bundle_takes_dated_name_and_interleaves_with_clips(self):
        card = self.sony_card()
        bundle = card / 'Stalker_2026-10-24_19-58-50.stalker'; bundle.mkdir()
        (bundle / 'screen.mov').write_bytes(b'screen')
        (bundle / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'sessionID': str(uuid.uuid4()),
            'status': 'completed', 'createdAt': '2026-10-25T02:58:50Z', 'duration': {'value': 227618267500, 'timescale': 1000000000},
            'tracks': [{'relativePath': 'screen.mov'}]}))
        os.environ['TZ'] = 'America/Hermosillo'; __import__('time').tzset()
        self.addCleanup(lambda: (os.environ.pop('TZ', None), __import__('time').tzset()))
        plan = self.plan(source=str(card), project='debloateando mi tele', source_kind='stalker', cleanup=False)
        name = '26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker'
        self.assertEqual(plan['layout']['Stalker_2026-10-24_19-58-50.stalker/screen.mov'],
                         {'archive': 'stalker/2026/10/' + name + '/screen.mov', 'work': 'e64_raw/debloateando-mi-tele/' + name + '/screen.mov'})
        self.assertLess('26OCT24-194650_SONYA6400', name)  # Sorts after the 19:46 clip, before 20:41.
        self.assertLess(name, '26OCT24-204110_SONYA6400')

    def test_relayout_renames_copied_bundle_in_place_on_every_volume(self):
        from unittest import mock
        from media import layout
        from media.library import relayout
        from media.recovery import recover_catalog
        card = self.root / 'captures'; bundle = card / 'Stalker_2026-10-24_19-58-50.stalker'; bundle.mkdir(parents=True)
        (bundle / 'screen.mov').write_bytes(b'screen'); (bundle / 'sources').mkdir(); (bundle / 'sources/mic.mov').write_bytes(b'mic')
        (bundle / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'sessionID': str(uuid.uuid4()),
            'status': 'completed', 'createdAt': '2026-10-25T02:58:50Z', 'duration': {'value': 227618267500, 'timescale': 1000000000},
            'tracks': [{'relativePath': 'screen.mov'}]}))
        real = layout.stalker_bundle_name
        with mock.patch.object(layout, 'stalker_bundle_name', lambda m, f, c, p: ('Stalker_2026-10-24_19-58-50.stalker', real(m, f, c, p)[1])):
            plan = self.plan(source=str(card), project='tele', source_kind='stalker', cleanup=False)
        self.run_plan(plan)
        self.assertTrue(list(self.work.rglob('Stalker_2026-10-24_19-58-50.stalker')))
        result = relayout(self.catalog, self.backend, plan['id'])
        self.assertEqual(len(result['renamed']), 2)  # Archive and work.
        for root in (self.archive, self.work):
            self.assertEqual(list(root.rglob('Stalker_2026-10-24_19-58-50.stalker')), [])
            renamed = next(root.rglob('*_STALKER_03-48_MX-TELE[[]].stalker'))
            self.assertEqual((renamed / 'sources/mic.mov').read_bytes(), b'mic')
        relayout(self.catalog, self.backend, plan['id'])  # Idempotent.
        asset = next(a for a in self.catalog.search() if a['kind'] == 'stalker_session')
        locations = handoff(self.catalog, self.backend, [asset['id']])['assets'][0]['locations']
        self.assertTrue(all('_STALKER_' in l['relative_path'] for l in locations))
        replacement = Catalog(self.root / 'recovered'); self.addCleanup(replacement.close)
        replacement.register('A-NEW', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        self.assertTrue(recover_catalog(replacement, self.backend, 'A-NEW')['verified'])

    def test_camera_cleanup_confirmed_only_when_backed_up_clips_are_gone(self):
        from media.transfer import confirm_camera_cleanup
        card = self.sony_card()
        plan = self.plan(source=str(card), project='tele')  # cleanup=True, Sony => camera action.
        self.assertEqual(self.run_plan(plan)['state'], 'camera_action_required')
        with self.assertRaisesRegex(MediaError, 'still on the card'):
            confirm_camera_cleanup(self.catalog, self.backend, plan['id'], str(card))
        for rel in ('PRIVATE/M4ROOT/CLIP/C0001.MP4', 'PRIVATE/M4ROOT/CLIP/C0001M01.XML', 'PRIVATE/M4ROOT/THMBNL/C0001T01.JPG'):
            (card / rel).unlink()  # What a camera format leaves: indexes, no clips.
        (card / 'PRIVATE/M4ROOT/CLIP/C0001.MP4').write_bytes(b'a brand new recording')  # Reused name, new clip.
        self.assertEqual(confirm_camera_cleanup(self.catalog, self.backend, plan['id'], str(card))['state'], 'completed')
        self.assertTrue((self.archive / '001_raw/videos/2026/10').is_dir())

    def test_same_second_clips_get_distinct_names(self):
        card = self.root / 'twins'; (card / 'DCIM').mkdir(parents=True)
        for name in ('A.MP4', 'B.MP4'):
            (card / 'DCIM' / name).write_bytes(name.encode())
            os.utime(card / 'DCIM' / name, ns=(10**18, 10**18))
        plan = self.plan(source=str(card), cleanup=False)
        targets = [entry['work'] for entry in plan['layout'].values()]
        self.assertEqual(len(set(targets)), 2)
        self.assertTrue(any(t.endswith('-2[].MP4') for t in targets))


class CaptureArchiveStatusTests(MediaFixture):
    def test_uppercase_session_ids_are_reported_archived(self):
        from media.captures import captures
        library = self.root / 'library'; bundle = library / 'Stalker_2026-10-24_19-58-50.stalker'; bundle.mkdir(parents=True)
        (bundle / 'screen.mov').write_bytes(b'screen')
        (bundle / 'manifest.json').write_text(json.dumps({'producer': 'stalker', 'schemaVersion': 3, 'status': 'completed',
            'sessionID': str(uuid.uuid4()).upper(), 'tracks': [{'relativePath': 'screen.mov'}]}))
        self.assertEqual(len(captures(self.catalog, library)['pending']), 1)
        self.run_plan(self.plan(source=str(library), source_kind='stalker', cleanup=False))
        self.assertEqual(captures(self.catalog, library)['pending'], [])


class VolumeBackendTests(unittest.TestCase):
    def test_stalled_diskutil_is_a_media_error(self):
        import subprocess, sys
        from unittest import mock
        if sys.platform != 'darwin':
            self.skipTest('macOS identity backend')
        with tempfile.TemporaryDirectory() as root, mock.patch('subprocess.run', side_effect=subprocess.TimeoutExpired('diskutil', 30)):
            with self.assertRaisesRegex(MediaError, 'retry in a moment'):
                VolumeBackend().describe(Path(root).resolve())


class CoverageTests(MediaFixture):
    def test_coverage_is_read_only_and_matches_by_content(self):
        from media.coverage import coverage
        plan = self.plan(cleanup=False); self.run_plan(plan)  # Receipted work copies.
        old = self.work / '001_proyectos/viejo'; old.mkdir(parents=True)
        (old / 'renamed.mov').write_bytes(b'same bytes')
        (self.archive / '001_raw/videos').mkdir(parents=True)
        (self.archive / '001_raw/videos/26OCT01-000000_X[].MOV').write_bytes(b'same bytes')
        (old / 'impostor.mov').write_bytes(b'other byte')  # Same size, different content.
        (old / 'unique.wav').write_bytes(b'only here!!!')
        (old / '.DS_Store').write_bytes(b'finder')
        before = sorted((p, p.stat().st_mtime_ns) for root in (self.work, self.archive) for p in root.rglob('*'))
        result = coverage(self.catalog, self.backend, 'ESSD', 'MEDIA')
        after = sorted((p, p.stat().st_mtime_ns) for root in (self.work, self.archive) for p in root.rglob('*'))
        self.assertEqual(before, after)
        self.assertEqual(result['totals']['in_archive']['files'], 3)  # Two receipted copies + renamed.mov.
        self.assertEqual(sorted(f['path'] for f in result['only_work_sample']),
                         ['001_proyectos/viejo/impostor.mov', '001_proyectos/viejo/unique.wav'])
        fast = coverage(self.catalog, self.backend, 'ESSD', 'MEDIA', path='001_proyectos/viejo', hash_matches=False)
        self.assertEqual((fast['totals']['size_match']['files'], fast['totals']['only_work']['files']), (2, 1))
        self.assertTrue(Path(result['report']).is_file())


class AdditionalFailureTests(MediaFixture):
    def test_process_death_during_copy_and_after_unlink_is_recoverable(self):
        import subprocess, sys
        for mode in ('copy_block', 'after_receipts', 'after_unlink'):
            with self.subTest(mode=mode):
                # Fresh paths and catalog per actual subprocess crash.
                area = self.root / mode; area.mkdir()
                source = area / 'card'; archive = area / 'archive'; work = area / 'work'
                for path in (source, archive, work): path.mkdir()
                (source / 'a.jpg').write_bytes(b'file-a'); (source / 'b.jpg').write_bytes(b'file-b')
                state = Catalog(area / 'state')
                try:
                    backend = FixtureVolumes()
                    state.register('A', 'archive', archive, backend.describe(archive), '001_raw')
                    state.register('W', 'work', work, backend.describe(work), '001_proyectos')
                    plan = build_plan(state, backend, str(source), 'A', 'W', 'crash', cleanup=True)
                    plan_file = area / 'plan.json'; plan_file.write_text(json.dumps(plan))
                    script = '''
import json, os, sys
sys.path.insert(0, 'tests')
from test_media_engine import FixtureVolumes
from media.catalog import Catalog
from media.common import Tree
from media.transfer import Transfer
plan=json.load(open(sys.argv[1])); mode=sys.argv[2]
catalog=Catalog(sys.argv[3]); backend=FixtureVolumes()
original=Tree.unlink
def unlink(self, rel, expected=None):
    original(self, rel, expected)
    if mode=='after_unlink' and str(self.root)==plan['source']: os._exit(77)
Tree.unlink=unlink
def hook(stage, data):
    if stage==mode: os._exit(77)
Transfer(catalog,backend,hook=hook).apply(plan,plan['digest'])
'''
                    child = subprocess.run([sys.executable, '-S', '-c', script, str(plan_file), mode, str(area / 'state')], capture_output=True, text=True)
                    self.assertEqual(child.returncode, 77, child.stderr)
                    result = Transfer(state, backend).apply(plan, plan['digest'])
                    self.assertEqual(result['state'], 'completed')
                    self.assertEqual(list(source.iterdir()), [])
                    self.assertEqual(len(list(archive.rglob('*.JPG'))), 2)  # a.jpg and b.jpg, no duplicates.
                finally: state.close()

    def test_corruption_during_cleanup_blocks_deletion_of_affected_source(self):
        plan = self.plan()
        def corrupt(stage, file):
            if stage == 'cleanup_file':
                unit = next(u for u in plan['units'] if u['id'] == file['unit_id'])
                path = self.archive / destination_path(self.a, plan, unit, file['path'])
                path.write_bytes(b'x' * path.stat().st_size)
        with self.assertRaisesRegex(MediaError, 'changed during cleanup'):
            self.run_plan(plan, hook=corrupt)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())
        self.assertTrue((self.source / 'DCIM/photo.DNG').exists())

    def test_disconnect_during_cleanup_does_not_delete_next_source(self):
        def disconnect(stage, file):
            if stage == 'cleanup_file': self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError): self.run_plan(hook=disconnect)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())

    def test_project_rename_keeps_identity_and_paths(self):
        self.run_plan()
        asset = self.catalog.search()[0]
        before = handoff(self.catalog, self.backend, [asset['id']])
        self.catalog.rename_project(asset['project_id'], 'new-title')
        after = handoff(self.catalog, self.backend, [asset['id']])
        self.assertEqual(before['project_id'], after['project_id'])
        self.assertEqual(before['assets'][0]['locations'], after['assets'][0]['locations'])
        self.assertEqual(self.catalog.search(project='new-title')[0]['id'], asset['id'])

    def test_shared_asset_retains_multiple_project_associations(self):
        first = self.plan(cleanup=False); self.run_plan(first)
        second = self.plan(cleanup=False, project='second-project'); self.run_plan(second)
        self.assertEqual(len(self.catalog.search()), 1)
        asset = self.catalog.search()[0]
        self.assertEqual({p['title'] for p in asset['projects']}, {'intro', 'second-project'})

    def test_publication_is_per_variant_and_channel(self):
        from media.publication import set_publication
        plan = self.plan(source_kind='delivery', cleanup=False, archive_only=True, confirm_archive_only=True, work_decision='skipped')
        self.run_plan(plan)
        asset = self.catalog.search()[0]
        set_publication(self.catalog, asset['id'], 'DCIM/photo.JPG', 'example-channel', 'published')
        rows = self.catalog.db.execute('SELECT * FROM publications').fetchall()
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['variant'], 'DCIM/photo.JPG')

    def test_mcp_is_read_only_and_does_not_invoke_mutations(self):
        import io
        from media.mcp import serve
        requests = [
            {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},
            {'jsonrpc':'2.0','method':'notifications/initialized'},
            {'jsonrpc':'2.0','id':2,'method':'tools/list'},
            {'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'media_library_search','arguments':{}}},
            {'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'apply','arguments':{}}},
        ]
        output = io.StringIO()
        serve(Service(self.catalog, self.backend), io.BytesIO(('\n'.join(json.dumps(r) for r in requests)+'\n').encode()), output)
        responses = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(len(responses), 4)
        self.assertTrue(all(t['annotations']['readOnlyHint'] for t in responses[1]['result']['tools']))
        self.assertFalse(responses[2]['result']['isError'])
        self.assertIn('error', responses[3])
        self.assertEqual(self.catalog.jobs(), [])

class RecoveryAndCaptureTests(MediaFixture):
    def test_recover_from_archive_after_source_cleanup_and_missing_local_catalog(self):
        from media.recovery import recover_catalog
        self.run_plan()
        original_asset = self.catalog.search()[0]
        replacement = Catalog(self.root / 'replacement-state')
        self.addCleanup(replacement.close)
        replacement.register('A-NEW', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        result = recover_catalog(replacement, self.backend, 'A-NEW')
        self.assertTrue(result['verified'])
        recovered = replacement.search()[0]
        self.assertEqual(recovered['id'], original_asset['id'])
        self.assertEqual(recovered['project_id'], original_asset['project_id'])
        self.assertEqual(replacement.jobs()[0]['state'], 'recovered')
        with self.assertRaises(MediaError): Service(replacement, self.backend).call('resume', {'job_id': recovered['job_id']})
        target = self.root / 'recovered-output'; target.mkdir()
        self.assertTrue(restore(replacement, self.backend, recovered['id'], target)['verified'])
        self.assertEqual(next(target.rglob('photo.JPG')).read_bytes(), b'photo-data')

    def test_recovery_ignores_appledouble_receipt_companions(self):
        from media.recovery import recover_catalog
        plan = self.plan(); self.run_plan(plan)
        (self.archive / '.ncreator/receipts' / ('._' + plan['id'] + '.json')).write_bytes(b'\x00\x05\x16\x07 appledouble')
        replacement = Catalog(self.root / 'exfat-state'); self.addCleanup(replacement.close)
        replacement.register('A-NEW', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        self.assertTrue(recover_catalog(replacement, self.backend, 'A-NEW')['verified'])

    def test_recovery_rejects_corrupt_archive(self):
        from media.recovery import recover_catalog
        self.run_plan()
        next(self.archive.rglob('*.JPG')).write_bytes(b'corrupt!!!')
        replacement = Catalog(self.root / 'corrupt-state'); self.addCleanup(replacement.close)
        replacement.register('A-NEW', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        with self.assertRaises(MediaError): recover_catalog(replacement, self.backend, 'A-NEW')
        self.assertEqual(replacement.search(), [])

    def test_capture_discovery_is_incremental_and_archived_sessions_are_skipped(self):
        from media.captures import captures
        library = self.root / 'captures'; library.mkdir()
        bundle = library / 'first.stalker'; bundle.mkdir()
        (bundle / 'screen.mov').write_bytes(b'video')
        manifest = {'producer':'stalker','schemaVersion':3,'sessionID':str(uuid.uuid4()),'status':'recording', 'tracks':[{'relativePath':'screen.mov'}]}
        marker = bundle / 'manifest.json'; marker.write_text(json.dumps(manifest))
        self.assertEqual(captures(self.catalog, str(library))['pending'], [])
        manifest['status'] = 'completed'; marker.write_text(json.dumps(manifest))
        self.assertEqual(len(captures(self.catalog, str(library), [str(marker)])['pending']), 1)
        plan = self.plan(source=str(library), source_kind='stalker', cleanup=False)
        self.run_plan(plan)
        self.assertEqual(captures(self.catalog, str(library), [str(marker)])['pending'], [])
        with self.assertRaisesRegex(MediaError, 'No completed'):
            self.plan(source=str(library), source_kind='stalker', cleanup=False)
        # Same session, regenerated manifest: a different revision must be offered.
        manifest['warnings'] = ['regenerated']; marker.write_text(json.dumps(manifest))
        self.assertEqual(len(captures(self.catalog, str(library), [str(marker)])['pending']), 1)

    def test_apfs_physical_store_overlap_is_not_redundancy(self):
        from media.volumes import independent
        self.assertFalse(independent({'physical_disks':['disk0','disk1']}, {'physical_disks':['disk1']}))
        self.assertTrue(independent({'physical_disks':['disk0']}, {'physical_disks':['disk2']}))
