"""Ejection boundary tests: temporary fake cards only, never system mounts."""
from unittest import mock

from test_media_engine import MediaFixture
from media.common import MediaError
from media.service import Service
from media.volumes import VolumeBackend, card_identity


class EjectTests(MediaFixture):
    def setUp(self):
        super().setUp()
        self.service = Service(self.catalog, self.backend)
        self.backend.mounted_roots = lambda: [str(self.source), str(self.archive), str(self.work)]
        self.ejected = []
        self.backend.eject = lambda root, identity: self.ejected.append(str(root))

    def token(self):
        return card_identity(self.backend.describe(self.source))

    def eject(self, **params):
        return self.service.call('eject_card', dict(source=str(self.source), identity=self.token(), **params))

    def test_listing_is_metadata_only_and_excludes_destinations(self):
        with mock.patch('media.planner.scan', side_effect=AssertionError('No scan')):
            cards = self.service.call('source_cards')
        self.assertEqual([c['path'] for c in cards], [str(self.source)])
        self.assertEqual(self.ejected, [])

    def test_explicit_eject_before_and_after_job(self):
        self.assertTrue(self.eject()['ejected'])
        self.run_plan(self.plan(cleanup=False))
        self.assertTrue(self.eject()['ejected'])
        self.assertEqual(self.ejected, [str(self.source)] * 2)

    def test_transfer_blocks_eject_and_releases_lock_afterwards(self):
        seen = []
        def hook(stage, data):
            if stage == 'copy_block' and not seen:
                with self.assertRaises(MediaError) as failure:
                    self.eject()
                self.assertEqual(failure.exception.code, 'job_busy')
                seen.append(stage)
        self.run_plan(self.plan(cleanup=False), hook=hook)
        self.assertEqual(seen, ['copy_block'])
        self.assertEqual(self.ejected, [])
        self.assertTrue(self.eject()['ejected'])

    def test_replacement_rejected(self):
        token = self.token()
        self.backend.replaced.add(str(self.source))
        with self.assertRaises(MediaError) as failure:
            self.service.call('eject_card', {'source': str(self.source), 'identity': token})
        self.assertEqual(failure.exception.code, 'volume_replaced')
        self.assertEqual(self.ejected, [])

    def test_registered_destinations_and_shared_physical_disk_rejected(self):
        for path in (self.archive, self.work):
            with self.assertRaises(MediaError) as failure:
                self.service.call('eject_card', {'source': str(path), 'identity': card_identity(self.backend.describe(path))})
            self.assertEqual(failure.exception.code, 'destination_volume')
        self.backend.physical[str(self.source)] = str(self.archive)
        with self.assertRaises(MediaError) as failure:
            self.eject()
        self.assertEqual(failure.exception.code, 'destination_volume')
        self.assertEqual(self.service.call('source_cards'), [])

    def test_non_cards_images_and_subdirectories_rejected(self):
        original = self.backend.describe
        for change in ({'removable': False}, {'external': False}, {'disk_image': True}, {'mount': '/different-mount'}):
            with mock.patch.object(self.backend, 'describe', side_effect=lambda root, external_required=False: dict(original(root), **change)):
                with self.assertRaises(MediaError) as failure:
                    self.eject()
                self.assertEqual(failure.exception.code, 'not_a_card')
        self.assertEqual(self.ejected, [])

    def test_backend_uses_only_normal_eject_and_reports_failure(self):
        info = self.backend.describe(self.source)
        with mock.patch('media.volumes.sys.platform', 'darwin'), \
                mock.patch.object(VolumeBackend, 'describe', return_value=info), \
                mock.patch('media.volumes.subprocess.run', return_value=mock.Mock(returncode=1)) as run:
            with self.assertRaises(MediaError) as failure:
                VolumeBackend().eject(self.source, card_identity(info))
        self.assertEqual(failure.exception.code, 'eject_failed')
        self.assertEqual(run.call_args.args[0], ['/usr/sbin/diskutil', 'eject', str(self.source)])

    def test_missing_identity_rejected(self):
        with self.assertRaises(MediaError):
            self.service.call('eject_card', {'source': str(self.source)})
        self.assertEqual(self.ejected, [])

    def test_register_respects_the_same_mutation_lock(self):
        with self.catalog.writer(), self.assertRaises(MediaError) as failure:
            self.service.call('register_volume', {'root': str(self.source), 'name': 'new', 'role': 'archive'})
        self.assertEqual(failure.exception.code, 'job_busy')

    def test_eject_timeout_is_explicit_and_never_claims_success(self):
        import subprocess
        info = self.backend.describe(self.source)
        with mock.patch('media.volumes.sys.platform', 'darwin'), \
                mock.patch.object(VolumeBackend, 'describe', return_value=info), \
                mock.patch('media.volumes.subprocess.run', side_effect=subprocess.TimeoutExpired('diskutil', 30)):
            with self.assertRaises(MediaError) as failure:
                VolumeBackend().eject(self.source, card_identity(info))
            self.assertEqual(failure.exception.code, 'eject_uncertain')

    def test_protects_destination_after_physical_disk_number_changes(self):
        self.backend.physical[str(self.archive)] = 'remounted-physical-store'
        self.backend.physical[str(self.source)] = 'remounted-physical-store'
        with self.assertRaises(MediaError) as failure:
            self.eject()
        self.assertEqual(failure.exception.code, 'destination_volume')
        self.assertEqual(self.service.call('source_cards'), [])
        self.assertEqual(self.ejected, [])
