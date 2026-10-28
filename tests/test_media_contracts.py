"""Shared producer fixtures, no personal catalogs/media or other app writes."""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import uuid
from unittest import mock

from test_media_engine import MediaFixture
from media import clipper
from media.common import MediaError, canonical
from media.evidence import capture_time, stalker_time
from media.library import handoff, sources
from media.retitle import CAPABILITY, cancel_plan, retitle
from media.service import Service

FIXTURES = Path(__file__).parent / 'fixtures/contracts'


class ContractTests(MediaFixture):
    def session(self):
        bundle = self.source / 'fixture.stalker'
        bundle.mkdir()
        manifest = json.loads((FIXTURES / 'stalker-v3.json').read_text())
        (bundle / 'manifest.json').write_bytes((FIXTURES / 'stalker-v3.json').read_bytes())
        (bundle / 'screen.mov').write_bytes(b'synthetic screen')
        return bundle, manifest

    def test_time_evidence_and_old_manifest(self):
        for case in json.loads((FIXTURES / 'capture-times.json').read_text()):
            result = capture_time(case['value'], 'fixture')
            self.assertEqual(result['captured_at_absolute'], case['utc'])
            self.assertEqual(result['offset_seconds'], case['offset'])
            self.assertEqual(result['original'], case['value'])
        manifest = json.loads((FIXTURES / 'stalker-v3.json').read_text())
        self.assertEqual(stalker_time(manifest)['captured_at_absolute'], '2026-10-28T17:00:00.123000Z')
        manifest['epochWallClock'] = 'broken'
        self.assertEqual(stalker_time(manifest)['captured_at_absolute'], manifest['createdAt'])
        self.assertEqual(stalker_time({})['captured_at_absolute'], None)

    def test_sources_receipts_schema_and_profile(self):
        bundle, manifest = self.session()
        Service(self.catalog).call('save_profile', {'id':'e64', 'name':'local', 'raw_dir':'e64_raw', 'clipper_profile':'erikiado64'})
        plan = self.plan(cleanup=False)
        self.run_plan(plan)
        assets = self.catalog.search()
        asset = next(a for a in assets if a['kind'] == 'stalker_session')
        # Showing a list must not probe/hash/read manifests from the media library.
        with mock.patch('media.common.hash_fd', side_effect=AssertionError('hash during list')), mock.patch('pathlib.Path.read_bytes', side_effect=AssertionError('read during list')):
            listing = sources(self.catalog, self.backend, 'intro')
            exported = handoff(self.catalog, self.backend, [asset['id']])
        entry = next(s for s in listing['sources'] if s['kind'] == 'stalker_session')
        self.assertEqual(entry['manifest_sha256'], hashlib.sha256((FIXTURES / 'stalker-v3.json').read_bytes()).hexdigest())
        self.assertNotEqual(entry['manifest_sha256'], asset['capture_manifest_digest'])
        self.assertEqual(entry['media_bytes'], len(b'synthetic screen'))
        self.assertGreater(entry['bytes'], entry['media_bytes'])
        self.assertEqual(entry['clipper_profile'], 'erikiado64')
        self.assertEqual(entry['capture_time']['time_zone'], 'America/Hermosillo')
        self.assertEqual(entry['revision'], asset['revision'])
        self.assertEqual(entry['content_revision'], exported['assets'][0]['content_revision'])
        self.assertEqual(entry['bytes'], exported['assets'][0]['bytes'])
        location = next(loc for loc in exported['assets'][0]['locations'] if loc['volume_id'] == entry['volume_id'])
        self.assertEqual(entry['receipt_id'], location['receipt_id'])
        self.validate_schema(exported)
        self.validate_schema(json.loads(Path('docs/contracts/examples/handoff.json').read_text()))
        # Archived old asset records stay listable; don't guess UTC from old names.
        asset.pop('capture_time', None)
        self.catalog.db.execute('UPDATE assets SET payload=? WHERE id=?', (canonical(asset).decode(), asset['id']))
        self.catalog.db.commit()
        old = next(s for s in sources(self.catalog, self.backend, 'intro')['sources'] if s['kind'] == 'stalker_session')
        self.assertIsNone(old['capture_time']['captured_at_absolute'])
        # Legacy profile config has no inferred Clipper alias.
        self.catalog.set('creative-profiles', {})
        self.assertIsNone(sources(self.catalog, self.backend, 'intro')['clipper_profile'])

    def validate_schema(self, document):
        try:
            import jsonschema  # optional validation tool; no runtime dependency
        except ImportError:
            self.skipTest('JSON Schema validation tool unavailable')
        schema = json.loads(Path('docs/contracts/media-handoff.schema.json').read_text())
        jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).validate(document)

    def test_camera_media_bytes_and_location_members(self):
        (self.source / 'DCIM/photo.XML').write_bytes(b'sidecar')
        self.run_plan(self.plan(cleanup=False))
        asset = self.catalog.search()[0]
        entry = sources(self.catalog, self.backend, 'intro')['sources'][0]
        self.assertEqual(entry['media_bytes'], len(b'photo-data') + len(b'raw-data'))
        self.assertEqual(entry['bytes'], entry['media_bytes'] + len(b'sidecar'))
        self.assertEqual(len(entry['editable_members']), 2)
        exported = handoff(self.catalog, self.backend, [asset['id']])
        self.validate_schema(exported)
        for loc in exported['assets'][0]['locations']:
            volume = self.catalog.volume(loc['volume_id'])
            for member in loc['members']:
                path = Path(volume['root']) / member['volume_relative_path']
                self.assertEqual(path.stat().st_size, member['bytes'])
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), member['sha256'])

    def request(self):
        bundle, manifest = self.session()
        return dict(operation_id=str(uuid.uuid4()), path=str(bundle), session_id=manifest['sessionID'],
                    expected_manifest_sha256=hashlib.sha256((bundle / 'manifest.json').read_bytes()).hexdigest(),
                    project={'version':1, 'name':'new', 'country':'mx'})

    def empty_inspect(self):
        return {'blockers':[], 'relinks':[], 'dependencies':{'projects':[], 'offline_projects':[]}}

    def producer_fixture(self, action, req):
        if action == 'capabilities':
            return {'schema':'stalker-capabilities/1', 'capabilities':[CAPABILITY]}
        if action == 'status':
            return {'schema':'stalker-retitle-result/1','operation_id':req['operation_id'], 'session_id':req['session_id'], 'status':'not_found'}
        # Fixture writer only: production coordinator never writes producer media.
        manifest_path = Path(req['path']) / 'manifest.json'
        manifest = json.loads(manifest_path.read_bytes())
        manifest['project'] = req['project']
        manifest_path.write_bytes(canonical(manifest))
        return {'schema':'stalker-retitle-result/1', 'status':'applied', 'operation_id':req['operation_id'],
                'session_id':req['session_id'], 'path':req['path'], 'project':req['project'],
                'previous_manifest_sha256':req['expected_manifest_sha256'],
                'manifest_sha256':hashlib.sha256(canonical(manifest)).hexdigest()}

    @contextlib.contextmanager
    def mocked_boundary(self, producer=None, inspect=None):
        with mock.patch('media.clipper.release_guard', return_value=contextlib.nullcontext()), \
             mock.patch('media.clipper.inspect', return_value=inspect or self.empty_inspect()), \
             mock.patch('media.retitle.producer', side_effect=lambda c, a, r: (producer or self.producer_fixture)(a, r)) as call:
            yield call

    def test_retitle_idempotence_conflict_and_pending_recovery(self):
        request = self.request()
        with self.mocked_boundary() as producer:
            result = retitle(self.catalog, **request)
            self.assertEqual(result['state'], 'applied')
            count = producer.call_count
            self.assertEqual(retitle(self.catalog, **request), result)
            self.assertEqual(producer.call_count, count)
            with self.assertRaisesRegex(MediaError, 'different payload'):
                retitle(self.catalog, **dict(request, project={'version':1,'name':'other','country':'mx'}))
        self.assertEqual(self.catalog.search(), [])  # No new revision adopted into the catalog/Clipper.

    def test_old_producer_plan_archive_and_references_block(self):
        request = self.request()
        original = (Path(request['path']) / 'manifest.json').read_bytes()
        with self.mocked_boundary(producer=lambda a,r: {'schema':'stalker-capabilities/1','capabilities':[]}):
            with self.assertRaisesRegex(MediaError, 'Update/restart'):
                retitle(self.catalog, **request)
        self.assertEqual((Path(request['path']) / 'manifest.json').read_bytes(), original)
        plan = self.plan(cleanup=False)
        with self.mocked_boundary():
            with self.assertRaisesRegex(MediaError, 'Cancel the reviewed plan'):
                retitle(self.catalog, **request)
        cancel_plan(self.catalog, plan['id'])
        references = self.empty_inspect()
        references['dependencies']['projects'] = [{'assets':[{'path':request['path'], 'items':[]}]}]
        with self.mocked_boundary(inspect=references):
            with self.assertRaisesRegex(MediaError, 'Clipper references'):
                retitle(self.catalog, **request)
        plan = self.plan(cleanup=False)
        self.run_plan(plan)
        with self.mocked_boundary():
            with self.assertRaisesRegex(MediaError, 'verified archived'):
                retitle(self.catalog, **request)

    def test_interruption_after_producer_write_recovers_without_second_apply(self):
        request = self.request()
        saved = {}
        def crash(action, req):
            if action == 'apply':
                saved.update(self.producer_fixture(action, req))
                raise RuntimeError('connection lost')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=crash):
            with self.assertRaisesRegex(RuntimeError, 'connection lost'):
                retitle(self.catalog, **request)
        with self.assertRaisesRegex(MediaError, 'Recover the pending'):
            self.plan(cleanup=False)
        def recover(action, req):
            if action == 'status': return saved
            self.assertNotEqual(action, 'apply')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=recover):
            self.assertEqual(retitle(self.catalog, **request)['state'], 'applied')
        self.assertTrue(self.plan(cleanup=False))

    def test_catalog_exclusion_spans_producer_mutation_and_kernel_recovers(self):
        request = self.request()
        def producer(action, req):
            if action == 'apply':
                script = "from media.catalog import Catalog; c=Catalog(" + repr(str(self.catalog.home)) + ");\nwith c.writer(): pass"
                child = subprocess.run([sys.executable, '-c', script], capture_output=True)
                self.assertNotEqual(child.returncode, 0)
                self.assertIn(b'Another media operation', child.stderr)
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=producer):
            retitle(self.catalog, **request)
        with self.catalog.writer(): pass

    def test_unknown_deps_and_queue_fail_before_mutation(self):
        request = self.request()
        for reason in ('offline projects', 'unknown queue', 'queued export'):
            with self.mocked_boundary(inspect=dict(self.empty_inspect(), blockers=[reason])) as producer:
                with self.assertRaisesRegex(MediaError, reason):
                    retitle(self.catalog, **request)
                self.assertEqual([c.args[1] for c in producer.call_args_list], ['capabilities'])

    def test_real_producer_cli_and_release_lock_exclusion(self):
        request = self.request()
        executable = self.root / 'stalker-session'
        executable.write_bytes((FIXTURES / 'stalker-producer.py').read_bytes())
        executable.chmod(0o700)
        self.catalog.set('stalker-executable', str(executable))
        # Real existing media-release-lock/1, isolated HOME; dependency report is synthetic.
        report = {'capabilities':['media-release-lock/1']}
        with mock.patch('pathlib.Path.home', return_value=self.root), \
                mock.patch('media.clipper.dependencies', return_value=report), \
                mock.patch('media.clipper.inspect', return_value=self.empty_inspect()), \
                mock.patch.dict(os.environ, {'STALKER_RETITLE_FIXTURE_ROOT':str(self.root)}):
            with clipper.release_guard(self.catalog):
                result = retitle(self.catalog, **request)
            self.assertEqual(result['state'], 'applied')
            self.assertEqual(retitle(self.catalog, **request), result)
            with clipper.release_guard(self.catalog):
                script = '''import fcntl, sys
with open(sys.argv[1], 'a+') as handle:
    fcntl.flock(handle, fcntl.LOCK_SH | fcntl.LOCK_NB)
'''
                child = subprocess.run([sys.executable, '-c', script, str(self.root / '.cache/clipper/.media-access.lock')], capture_output=True)
                self.assertNotEqual(child.returncode, 0)
                self.assertIn(b'BlockingIOError', child.stderr)

    def test_recovery_requires_matching_ack_and_preserves_non_project_metadata(self):
        request = self.request()
        def bad(action, req):
            result = self.producer_fixture(action, req)
            if action == 'apply':
                path = Path(req['path']) / 'manifest.json'
                value = json.loads(path.read_bytes())
                value['createdAt'] = '2020-01-01T00:00:00Z'
                path.write_bytes(canonical(value))
                result['manifest_sha256'] = hashlib.sha256(canonical(value)).hexdigest()
            return result
        with self.mocked_boundary(producer=bad):
            with self.assertRaisesRegex(MediaError, 'Published manifest'):
                retitle(self.catalog, **request)
        self.assertEqual(self.catalog.get('retitle:' + request['operation_id'])['state'], 'pending')
        with self.assertRaisesRegex(MediaError, 'Recover the pending'):
            self.plan(cleanup=False)

    def test_cancelled_and_old_plans_never_apply_after_retitle(self):
        request = self.request()
        old = self.plan(cleanup=False)
        cancel_plan(self.catalog, old['id'])
        with self.mocked_boundary():
            retitle(self.catalog, **request)
        with self.assertRaisesRegex(MediaError, 'Stalker metadata changed'):
            self.run_plan(old)
        self.assertEqual(self.catalog.jobs(), [])

    def test_dead_catalog_owner_releases_kernel_lock(self):
        script = '''from media.catalog import Catalog
import sys, time
c = Catalog(sys.argv[1])
with c.writer():
    print('locked', flush=True)
    time.sleep(30)
'''
        child = subprocess.Popen([sys.executable, '-c', script, str(self.catalog.home)], stdout=subprocess.PIPE)
        try:
            self.assertEqual(child.stdout.readline().strip(), b'locked')
            with self.assertRaisesRegex(MediaError, 'Another media operation'):
                with self.catalog.writer(): pass
        finally:
            child.kill(); child.wait(); child.stdout.close()
        with self.catalog.writer(): pass

    def test_producer_inherits_exclusion_when_coordinator_is_killed(self):
        import time
        request = self.request()
        executable = self.root / 'stalker-session'
        executable.write_bytes((FIXTURES / 'stalker-producer.py').read_bytes())
        executable.chmod(0o700)
        self.catalog.set('stalker-executable', str(executable))
        script = '''import json, sys
from pathlib import Path
from unittest import mock
from media.catalog import Catalog
from media.retitle import retitle
root, request = Path(sys.argv[1]), json.loads(sys.argv[2])
with mock.patch('pathlib.Path.home', return_value=root), mock.patch('media.clipper.dependencies', return_value={'capabilities':['media-release-lock/1']}), mock.patch('media.clipper.inspect', return_value={'blockers':[], 'dependencies':{'projects':[]}}):
    retitle(Catalog(root / 'state'), **request)
'''
        env = dict(os.environ, STALKER_RETITLE_FIXTURE_ROOT=str(self.root), STALKER_RETITLE_FIXTURE_PAUSE='1')
        child = subprocess.Popen([sys.executable, '-c', script, str(self.root), json.dumps(request)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 8
            while not (self.root / 'producer-entered').exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue((self.root / 'producer-entered').exists())
            child.kill(); child.wait()
            with self.assertRaisesRegex(MediaError, 'Another media operation'):
                with self.catalog.writer(): pass
            # The editor lock is held by the orphan producer too.
            import fcntl
            with (self.root / '.cache/clipper/.media-access.lock').open('a+') as lock:
                with self.assertRaises(BlockingIOError): fcntl.flock(lock, fcntl.LOCK_SH | fcntl.LOCK_NB)
        finally:
            (self.root / 'producer-resume').touch()
            if child.poll() is None: child.kill(); child.wait()
            child.stderr.close()
        deadline = time.monotonic() + 8
        marker = self.root / (request['operation_id'] + '.result.json')
        while not marker.exists() and time.monotonic() < deadline: time.sleep(0.02)
        self.assertTrue(marker.exists())
        # Wait for the fixture child to release its inherited descriptors.
        while True:
            try:
                with self.catalog.writer(): pass
                break
            except MediaError:
                self.assertLess(time.monotonic(), deadline)
                time.sleep(0.02)
        with mock.patch('pathlib.Path.home', return_value=self.root), mock.patch('media.clipper.dependencies', return_value={'capabilities':['media-release-lock/1']}), mock.patch.dict(os.environ, {'STALKER_RETITLE_FIXTURE_ROOT':str(self.root)}):
            result = retitle(self.catalog, **request)
            self.assertEqual(result['state'], 'applied')

    def test_legacy_versions_and_unknown_major(self):
        from media.retitle import manifest_at
        bundle, manifest = self.session()
        for version in (1, 2, 3):
            value = dict(manifest, schemaVersion=version)
            value.pop('epochWallClock'); value.pop('captureTimeZone')
            (bundle / 'manifest.json').write_bytes(canonical(value))
            self.assertEqual(manifest_at(bundle)[0]['schemaVersion'], version)
            self.assertEqual(stalker_time(value)['captured_at_absolute'], value['createdAt'])
        (bundle / 'manifest.json').write_bytes(canonical(dict(manifest, schemaVersion=4)))
        with self.assertRaisesRegex(MediaError, 'supported Stalker'):
            manifest_at(bundle)
        legacy = json.loads((FIXTURES / 'handoff-v1-legacy.json').read_text())
        self.validate_schema(legacy)
        import jsonschema
        legacy['schema'] = 'creator-media-handoff/2'
        with self.assertRaises(jsonschema.ValidationError): self.validate_schema(legacy)

    def test_old_profile_client_preserves_explicit_mapping(self):
        service = Service(self.catalog)
        value = {'id':'e64','name':'e64','raw_dir':'e64_raw'}
        service.call('save_profile', dict(value, clipper_profile='opaque:clipper-profile'))
        self.assertEqual(service.call('save_profile', value)['clipper_profile'], 'opaque:clipper-profile')
        self.assertIsNone(service.call('save_profile', dict(value, clipper_profile=None))['clipper_profile'])

    def test_retitle_schema_fixtures_and_cli_envelope(self):
        try:
            import jsonschema
        except ImportError:
            self.skipTest('JSON Schema validation tool unavailable')
        request = self.request()
        schema = json.loads(Path('docs/contracts/stalker-retitle.schema.json').read_text())
        validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker())
        with self.mocked_boundary():
            operation = retitle(self.catalog, **request)
        validator.validate(operation['request'])
        validator.validate(operation['result'])
        validator.validate(self.producer_fixture('capabilities', {}))
        child = subprocess.run([sys.executable, 'nu9ve.py', 'media', '--state-dir', str(self.catalog.home), 'retitle-operations'], capture_output=True)
        self.assertEqual(child.returncode, 0, child.stdout)
        envelope = json.loads(child.stdout)
        self.assertTrue(envelope['ok'])
        self.assertEqual(envelope['data'][0]['state'], 'applied')

    def pending_setup(self):
        request = self.request()
        def crash(action, req):
            if action == 'apply': raise RuntimeError('crash')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=crash):
            with self.assertRaisesRegex(RuntimeError, 'crash'): retitle(self.catalog, **request)
        target = Path(request['path']).with_name('renamed.stalker')
        Path(request['path']).rename(target)
        value = json.loads((target / 'manifest.json').read_bytes())
        value['project'] = request['project']
        (target / 'manifest.json').write_bytes(canonical(value))
        pending = dict(schema='stalker-retitle-result/1', status='pending', operation_id=request['operation_id'],
                       session_id=request['session_id'].lower(), path=str(target), project=request['project'],
                       previous_manifest_sha256=request['expected_manifest_sha256'],
                       manifest_sha256=hashlib.sha256(canonical(value)).hexdigest())
        return request, pending

    def test_pending_dispatch_after_rename_checks_both_paths_and_is_idempotent(self):
        request, pending = self.pending_setup()
        def recover(action, req):
            if action == 'status': return pending
            if action == 'apply': return dict(pending, status='applied')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=recover) as producer, mock.patch('media.clipper.inspect', return_value=self.empty_inspect()) as inspect:
            result = retitle(self.catalog, **request)
            self.assertEqual(result['state'], 'applied')
            self.assertEqual([list(c.args[1])[0] for c in inspect.call_args_list], [request['path'], pending['path']])
            self.assertEqual([c.args[1] for c in producer.call_args_list], ['capabilities','status','apply'])
            self.assertEqual(retitle(self.catalog, **request), result)
            self.assertEqual(producer.call_count, 3)

    def test_pending_target_dependencies_or_invalid_result_never_dispatch(self):
        request, pending = self.pending_setup()
        def recover(action, req):
            if action == 'status': return pending
            self.assertNotEqual(action, 'apply')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=recover), mock.patch('media.clipper.inspect', side_effect=[self.empty_inspect(),dict(self.empty_inspect(), blockers=['target queued'])]):
            with self.assertRaisesRegex(MediaError, 'target queued'): retitle(self.catalog, **request)
        for field, value in [('path',str(self.root / 'elsewhere.stalker')), ('previous_manifest_sha256','0'*64), ('project',{}), ('manifest_sha256','invalid')]:
            bad = dict(pending, **{field:value})
            with self.mocked_boundary(producer=lambda a,r: bad if a == 'status' else self.producer_fixture(a,r)) as producer:
                with self.assertRaisesRegex(MediaError, 'Invalid pending'): retitle(self.catalog, **request)
                self.assertNotIn('apply', [c.args[1] for c in producer.call_args_list])
        self.assertEqual(self.catalog.get('retitle:' + request['operation_id'])['state'], 'pending')

    def test_pending_recovery_failure_remains_pending(self):
        request, pending = self.pending_setup()
        def recover(action, req):
            if action == 'status': return pending
            if action == 'apply': raise RuntimeError('producer failed')
            return self.producer_fixture(action, req)
        with self.mocked_boundary(producer=recover):
            with self.assertRaisesRegex(RuntimeError, 'producer failed'): retitle(self.catalog, **request)
        self.assertEqual(self.catalog.get('retitle:' + request['operation_id'])['state'], 'pending')
