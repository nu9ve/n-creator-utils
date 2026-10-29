"""Real local CLIs, generated media, disposable catalog/volumes and HOME only."""
import contextlib
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import unittest
import uuid
from unittest import mock

from test_media_engine import MediaFixture
from media import clipper
from media.common import MediaError, canonical
from media.library import handoff, sources, resolve
from media.revisions import publish_revision
from media.service import Service

REPO = Path(__file__).resolve().parents[1]
CLIPPER = REPO.parents[1] / 'ai/clipper'
STALKER = REPO.parents[1] / 'ai/stalker'
BINARY = STALKER / '.build/debug/stalker-session'
PYTHON = CLIPPER / '.venv/bin/python'


class StandaloneTests(MediaFixture):
    def test_standalone_and_known_offline(self):
        with mock.patch('media.clipper.executable', side_effect=MediaError('clipper_unavailable', 'absent')), mock.patch('pathlib.Path.home', return_value=self.root), mock.patch.dict(os.environ, {'CLIPPER_PROJECTS_ROOT':str(self.root/'projects')}):
            service = Service(self.catalog, self.backend)
            self.assertFalse(service.call('integration_capabilities')['clipper']['available'])
            self.run_plan(self.plan(cleanup=False))
            asset = self.catalog.search()[0]
            self.assertTrue(handoff(self.catalog, self.backend, [asset['id']]))
            self.assertEqual(clipper.dependencies(self.catalog)['integration'], 'never_configured')
            self.catalog.set('clipper-known', True)
            with self.assertRaises(MediaError): clipper.dependencies(self.catalog)
            self.assertFalse(service.call('integration_capabilities')['clipper']['available'])
            self.catalog.set('clipper-known', False)
            with mock.patch('media.clipper.executable', return_value=str(self.root/'disappeared-clipper')):
                with self.assertRaises(MediaError): clipper.dependencies(self.catalog)
            self.assertTrue(self.catalog.get('clipper-known'))
            with self.assertRaises(MediaError): clipper.dependencies(self.catalog)


@unittest.skipUnless(PYTHON.is_file() and BINARY.is_file() and shutil.which('ffmpeg'), 'requires local Clipper/Stalker CLIs and ffmpeg')
class RealClosureTests(MediaFixture):
    def setUp(self):
        super().setUp()
        self.stack = contextlib.ExitStack(); self.addCleanup(self.stack.close)
        self.stack.enter_context(mock.patch('pathlib.Path.home', return_value=self.root))
        env = {'CLIPPER_PROJECTS_ROOT':str(self.root/'projects'), 'CLIPPER_CACHE_ROOT':str(self.root/'.cache/clipper'),
               'CLIPPER_PROFILES_ROOT':str(self.root/'profiles'), 'CLIPPER_LIBRARY_PROJECTS_ROOT':str(self.root/'editor/{profile}'),
               'CLIPPER_RENDER_TEMP_ROOT':str(self.root/'render-temp'), 'STALKER_RETITLE_FIXTURE_ROOT':str(self.root),
               'CLIPPER_NU9VE_STATE_DIR':str(self.catalog.home), 'PYTHONDONTWRITEBYTECODE':'1'}
        launcher = self.root/'nuun.py'
        launcher.write_text(f'''import sys
sys.path[:0]=[{str(REPO)!r}, {str(REPO/'tests')!r}]
from test_media_engine import FixtureVolumes
import media.service
media.service.VolumeBackend=FixtureVolumes
from media.cli import main
raise SystemExit(main(sys.argv[2:]))
''')
        env['CLIPPER_NU9VE'] = f'{sys.executable} {launcher}'
        self.stack.enter_context(mock.patch.dict(os.environ, env))
        self.cli = self.root/'clipper'
        self.cli.write_text(f'#!{PYTHON}\nfrom pathlib import Path\nPath.home=classmethod(lambda cls:Path({str(self.root)!r}))\nfrom clipper_tui.cli import app\napp()\n')
        self.cli.chmod(0o700)
        self.catalog.set('clipper-executable', str(self.cli))
        self.catalog.set('stalker-executable', str(BINARY))
        for path in (self.source/'DCIM').iterdir(): path.unlink()
        self.old_files = {}

    def command(self, *args, ok=True):
        result = subprocess.run([str(self.cli), *map(str,args)], capture_output=True, text=True, timeout=90)
        if ok: self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        else: self.assertNotEqual(result.returncode, 0, result.stdout)
        return json.loads(result.stdout) if result.returncode == 0 else result

    def video(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i','color=s=64x64:d=1:r=10','-c:v','libx264',str(path)],check=True,capture_output=True)

    def exported(self, asset, volume=None, name='handoff.json'):
        doc = handoff(self.catalog, self.backend, [asset['id']], volume)
        path = self.root/name; path.write_bytes(canonical(doc))
        return path, doc

    def delivery(self):
        self.video(self.source/'a/first.mp4'); self.video(self.source/'b/second.mp4')
        (self.source/'notes.txt').write_text('fixture auxiliary')
        plan = self.plan(source_kind='delivery', cleanup=False)
        self.run_plan(plan)
        return self.catalog.search()[0]

    def test_delivery_complete_queue_relink_ack_release_and_retry(self):
        from media.release import plan_release, relink_release, apply_release
        asset = self.delivery()
        # Explicitly differing member names/folders in the archive, persisted in real receipts.
        for row in list(self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'],self.a['id']))):
            c = json.loads(row[0]); old = self.archive/c['destination']
            c['destination'] = '002_finales/renamed/' + str(uuid.uuid4()) + old.suffix
            dest = self.archive/c['destination']; dest.parent.mkdir(parents=True,exist_ok=True); old.rename(dest)
            with self.catalog.db:
                self.catalog.db.execute('UPDATE copies SET payload=? WHERE job_id=? AND volume_id=? AND relative_path=?',(canonical(c).decode(),asset['job_id'],self.a['id'],c['path']))
        receipt = self.archive/'.ncreator/receipts'/f"{asset['job_id']}.json"
        data = json.loads(receipt.read_bytes())
        data['files'] = [json.loads(r[0]) for r in self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?',(asset['job_id'],self.a['id']))]
        receipt.write_bytes(canonical(data))
        entry = sources(self.catalog,self.backend,'intro')['sources'][0]
        self.assertEqual(len(entry['media_members']),3)
        path, doc = self.exported(asset)
        self.assertEqual(len(doc['assets'][0]['members']),3)
        imported = self.command('library','import','intro','--json')
        self.assertEqual(len(imported['added']),1)
        report = self.command('library','handoff',path,'--json')
        project = report['project_root']
        self.assertTrue(self.command('library','handoff',path,'--json')['idempotent'])
        for member in entry['media_members']:
            if member['original_path'].endswith('.mp4'):
                self.command('app-service','analyze-item','--project',project,'--item',member['path'],'--json')
        self.command('export-queue','--enqueue',project)
        # Release only media according to the existing raw-folder policy; auxiliary stays on work.
        folder = str(Path(entry['path']).parent.parent.relative_to(self.work))
        plan = plan_release(self.catalog,self.backend,'MEDIA',work='ESSD',path=folder,project='intro')
        self.assertTrue(plan['consumers']['blockers'])
        with self.assertRaises(MediaError): apply_release(self.catalog,self.backend,plan['id'],plan['digest'])
        self.command('export-queue','--clear')
        result = relink_release(self.catalog,self.backend,plan['id'],plan['digest'])
        ack = result['acks'][0]
        self.assertEqual(len(ack['member_targets']),3)
        self.assertTrue(all(Path(p).is_relative_to(self.archive) for p in ack['member_targets'].values()))
        request = dict(project=project,asset_id=asset['id'],revision=asset['revision'],
                       **{'from':ack['from'],'to':ack['to']},kind='delivery')
        real_invoke = clipper.invoke
        def incomplete_ack(catalog, args, **kwargs):
            response = real_invoke(catalog,args,**kwargs)
            return dict(response,member_targets={}) if args[0]=='relink' else response
        with mock.patch('media.clipper.invoke',side_effect=incomplete_ack):
            with self.assertRaises(MediaError):clipper.relink(self.catalog,request,ack['operation_id'])
        self.assertTrue(all((self.work/e['path']).exists() for e in plan['entries']))
        self.assertEqual(clipper.relink(self.catalog,request,ack['operation_id']),ack)
        self.assertFalse(clipper.inspect(self.catalog,plan['replacements'])['relinks'])
        self.assertEqual(apply_release(self.catalog,self.backend,plan['id'],plan['digest'])['state'],'completed')
        self.assertTrue((self.source/'notes.txt').exists())

    def session(self):
        root = self.source/'original.stalker'; root.mkdir()
        self.video(root/'screen.mov')
        (root/'sources').mkdir()
        for name in ('mic.wav','sources/mic.mov','sources/system.mov'):
            subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i','sine=duration=1','-c:a','pcm_s16le',str(root/name)],check=True,capture_output=True)
        manifest = json.loads((STALKER/'Tests/Fixtures/retitle/stalker-v3.json').read_bytes())
        manifest['sessionID'] = str(uuid.uuid4()).upper()
        for track in manifest['tracks']:
            track.update(acceptedSamples=2, firstPTS={'value':0,'timescale':1}, lastEndPTS={'value':1,'timescale':1})
        manifest['project'] = {'version':1,'name':'old','country':'mx'}
        manifest['outputs'] = {'voice':'mic.wav'}
        (root/'manifest.json').write_bytes(canonical(manifest))
        (root/'events.ndjson').write_text('')
        plan = self.plan(source_kind='stalker',cleanup=False)
        self.run_plan(plan)
        return self.catalog.search()[0]

    def test_real_publication_adoption_preserves_old_queue_and_history(self):
        asset = self.session()
        path, _ = self.exported(asset)
        project = self.command('library','handoff',path,'--json')['project_root']
        work_location = next(l for l in resolve(self.catalog,self.backend,asset['id'])['locations'] if l['role']=='work')
        screen = next(f['local_path'] for f in work_location['files'] if f['path'].endswith('/screen.mov'))
        self.command('app-service','analyze-item','--project',project,'--item',screen,'--json')
        self.command('export-queue','--enqueue',project)
        old_locations = resolve(self.catalog,self.backend,asset['id'])
        before = {f['local_path']:Path(f['local_path']).read_bytes() for l in old_locations['locations'] for f in l['files']}
        req = dict(operation_id=str(uuid.uuid4()),asset_id=asset['id'],expected_revision=asset['revision'],
                   project={'version':1,'name':'corrected','country':'mx'},archive='MEDIA')
        result = publish_revision(self.catalog,self.backend,**req)
        self.assertEqual(result['state'],'published')
        self.assertEqual(publish_revision(self.catalog,self.backend,**req),result)
        new = result['asset']; self.assertNotEqual(new['revision'],asset['revision'])
        self.assertEqual(len(Service(self.catalog,self.backend).call('asset_revisions',{'asset_id':asset['id']})),2)
        for path, data in before.items(): self.assertEqual(Path(path).read_bytes(),data)
        path, _ = self.exported(new, self.a['id'], 'revision.json')
        plan = self.command('library','adopt-revision',project,asset['id'],path,'--from-revision',asset['revision'])
        self.assertEqual(plan['status'],'planned')
        ack = self.command('library','adopt-revision',project,asset['id'],path,'--from-revision',asset['revision'],'--apply','--operation-id',plan['operation_id'])
        self.assertEqual(ack['status'],'applied'); self.assertTrue(ack['still_pinned'])
        inspection = clipper.inspect(self.catalog,{ack['from']:None})
        self.assertTrue(inspection['blockers'])
        self.command('export-queue','--clear')
        self.assertFalse(clipper.inspect(self.catalog,{ack['from']:None})['blockers'])
        self.assertTrue(resolve(self.catalog,self.backend,asset['id'],asset['revision'])['locations'])
        delivered = Service(self.catalog,self.backend).call('deliver_work',{'asset_ids':[asset['id']],'work':'ESSD'})
        self.assertEqual(delivered['delivered'],[asset['id']])
        self.assertEqual(len(resolve(self.catalog,self.backend,asset['id'])['locations']),2)

    def test_publication_crash_retries_real_producer_and_coordinator(self):
        asset = self.session()
        for index, stage in enumerate(('copy_verified','producer_applied','receipt_durable','published','manifest')):
            with self.subTest(stage=stage):
                asset = self.catalog.search()[0]
                req = dict(operation_id=str(uuid.uuid4()),asset_id=asset['id'],expected_revision=asset['revision'],
                           project={'version':1,'name':f'retry-{index}','country':'mx'},archive='MEDIA')
                worker = self.root/'publication-worker.py'
                worker.write_text(f'''import sys,os
sys.path[:0]=[{str(REPO)!r},{str(REPO/'tests')!r}]
from pathlib import Path
Path.home=classmethod(lambda cls:Path({str(self.root)!r}))
from media.catalog import Catalog
from test_media_engine import FixtureVolumes
from media.revisions import publish_revision
catalog=Catalog({str(self.catalog.home)!r})
def crash(stage):
    if stage=={stage!r}:os._exit(97)
publish_revision(catalog,FixtureVolumes(),**{req!r},hook=crash)
''')
                env = os.environ.copy()
                if stage == 'manifest':env['STALKER_RETITLE_FIXTURE_CRASH']='manifest'
                result = subprocess.run([sys.executable,str(worker)],env=env,capture_output=True,text=True,timeout=60)
                self.assertNotEqual(result.returncode,0,result.stdout)
                op = self.catalog.get('revision-publication:'+req['operation_id'])
                self.assertEqual(op['state'],'published' if stage=='published' else 'pending')
                before = resolve(self.catalog,self.backend,asset['id'],asset['revision'])
                published = publish_revision(self.catalog,self.backend,**req)
                self.assertEqual(published['state'],'published')
                self.assertEqual(publish_revision(self.catalog,self.backend,**req),published)
                for loc in before['locations']:
                    self.assertTrue(all(Path(f['local_path']).is_file() for f in loc['files']))
                conflict = dict(req,project={'version':1,'name':'different','country':'mx'})
                with self.assertRaisesRegex(MediaError,'payload'):publish_revision(self.catalog,self.backend,**conflict)
        from media.catalog import Catalog
        from media.recovery import recover_catalog
        recovered = Catalog(self.root/'recovered'); self.addCleanup(recovered.close)
        recovered.register('MEDIA','archive',self.archive,self.a['identity'],self.a['prefix'])
        recover_catalog(recovered,self.backend,'MEDIA')
        self.assertEqual(recovered.search()[0]['revision'],published['asset']['revision'])
        self.assertEqual(recovered.db.execute('SELECT count(*) FROM asset_revisions').fetchone()[0],6)

    def test_delivery_partial_hash_offline_and_retry(self):
        asset = self.delivery()
        path, doc = self.exported(asset,self.w['id'])
        partial = copy.deepcopy(doc)
        partial['assets'][0]['members'] = partial['assets'][0]['members'][:1]
        partial['assets'][0]['locations'][0]['members'] = partial['assets'][0]['members']
        path.write_bytes(canonical(partial))
        self.command('library','handoff',path,'--json',ok=False)
        path.write_bytes(canonical(doc))
        member = self.work/doc['assets'][0]['locations'][0]['members'][-1]['volume_relative_path']
        before = member.read_bytes(); member.write_bytes(b'x'*len(before))
        self.command('library','handoff',path,'--json',ok=False)
        member.write_bytes(before)
        missing = member.with_suffix('.offline'); member.rename(missing)
        self.command('library','handoff',path,'--json',ok=False)
        missing.rename(member)
        self.assertEqual(len(self.command('library','handoff',path,'--json')['added']),1)
        bad = copy.deepcopy(doc); bad['schema']='creator-media-handoff/99'
        path.write_bytes(canonical(bad));self.command('library','handoff',path,'--json',ok=False)

    def test_publication_capability_and_offline_failures_keep_old(self):
        asset = self.session()
        req = dict(operation_id=str(uuid.uuid4()),asset_id=asset['id'],expected_revision=asset['revision'],
                   project={'version':1,'name':'new','country':'mx'},archive='MEDIA')
        for response in ({'schema':'stalker-capabilities/99','capabilities':['stalker-coordinated-retitle/1']},
                         {'schema':'stalker-capabilities/1','capabilities':[]}):
            with mock.patch('media.revisions.producer', return_value=response):
                with self.assertRaises(MediaError):publish_revision(self.catalog,self.backend,**req)
        self.assertIsNone(self.catalog.get('revision-publication:'+req['operation_id']))
        self.backend.offline.add(str(self.archive))
        with self.assertRaises(MediaError):publish_revision(self.catalog,self.backend,**req)
        self.backend.offline.clear()
        self.assertEqual(self.catalog.search()[0]['revision'],asset['revision'])
        self.assertEqual(publish_revision(self.catalog,self.backend,**req)['state'],'published')
