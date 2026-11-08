"""Clipper transcripts kept in MEDIA next to the backed-up video (contracts/clipper-transcripts-v1.md).

Fixture volumes and a fake `transcripts-export`; never ESSD or MEDIA502."""
import hashlib
import json
import os
from pathlib import Path
from unittest import mock

from test_media_release import WorkflowFixture, empty_deps
from media import closure, transcripts
from media.common import MediaError, canonical, content_stamp, fingerprint


class TranscriptTests(WorkflowFixture):
    def setUp(self):
        super().setUp()
        patcher = mock.patch('media.clipper.dependencies', side_effect=lambda _: empty_deps())
        patcher.start(); self.addCleanup(patcher.stop)
        self.run_plan()
        self.project_id = self.catalog.db.execute("SELECT id FROM projects WHERE title='intro'").fetchone()[0]
        self.asset = next(a for a in self.catalog.search() if a['project_id'] == self.project_id)
        self.member = next(p for p in self.asset['paths'] if p.endswith('.JPG'))
        self.member_sha = self.catalog.db.execute('SELECT sha256 FROM members WHERE job_id=? AND relative_path=?',
                                                  (self.asset['job_id'], self.member)).fetchone()[0]
        self.archived = json.loads(self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                                           (self.asset['job_id'], self.a['id'], self.member)).fetchone()[0])['destination']
        self.work_copy = json.loads(self.catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                                            (self.asset['job_id'], self.w['id'], self.member)).fetchone()[0])['destination']
        self.clipper = self.work / '001_proyectos/e64/intro/clipper'
        (self.clipper / 'transcripts').mkdir(parents=True)
        (self.clipper / 'clipper-project.json').write_text('{}')
        self.files = {}
        for suffix, fmt, body in (('.transcript.json', 'transcript.json', b'{"words": []}'), ('.srt', 'srt', b'1\n00:00 --> 00:01\nhola\n')):
            path = self.clipper / 'transcripts' / ('photo' + suffix)
            path.write_bytes(body)
            self.files[fmt] = path
        self.source = {'path': str(self.work / self.work_copy), 'source_id': 'photo', 'asset_id': self.asset['id'],
                       'member_sha256': self.member_sha, 'clipper_fingerprint': 'x' * 64}
        self.fail = None
        patcher = mock.patch('media.clipper.invoke', side_effect=self.fake_invoke)
        self.invoke = patcher.start(); self.addCleanup(patcher.stop)

    def fake_invoke(self, catalog, args, timeout=120):
        if self.fail:
            raise MediaError('clipper_failed', self.fail)
        if args[0] != 'transcripts-export':
            raise MediaError('clipper_failed', 'only transcripts in this test')
        entries = []
        for fmt, path in self.files.items():
            data = path.read_bytes()
            entries.append({'path': str(path), 'format': fmt, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
                            'stamp': {k: v for k, v in fingerprint(path.stat()).items()}, 'source': dict(self.source)})
        value = {'schema': 'clipper-transcript-export/1', 'project_root': os.path.realpath(args[1]), 'project_state': 'active',
                 'generated_at': '2026-11-07T00:00:00Z', 'entries': entries}
        value['digest'] = hashlib.sha256(canonical(value)).hexdigest()
        return value

    def transcript_plan(self):
        return transcripts.plan(self.catalog, self.backend, self.a['id'], [self.clipper])

    def test_copies_next_to_the_archived_clip_with_a_receipt_and_adopts_on_repeat(self):
        value = self.transcript_plan()
        folder = '004_datos/clipper/transcripts/' + transcripts.target_folder(self.archived)
        self.assertRegex(folder, r'^004_datos/clipper/transcripts/\d{4}/\d{2}/')
        self.assertEqual(sorted(e['target'] for e in value['entries']),
                         sorted(folder + '/' + p.name for p in self.files.values()))
        self.assertEqual({e['action'] for e in value['entries']}, {'copy'})
        result = transcripts.copy(self.catalog, self.backend, value)
        self.assertEqual(result['copied'], 2)
        for path in self.files.values():
            self.assertEqual((self.archive / folder / path.name).read_bytes(), path.read_bytes())
            self.assertTrue(path.exists(), 'nothing is deleted from clipper/')
        receipt = json.loads((self.archive / ('.ncreator/receipts/data-' + result['receipt_id'] + '.json')).read_text())
        self.assertEqual(receipt['schema'], 'nu9ve-data-receipt/1')
        self.assertEqual({f['asset_id'] for f in receipt['files']}, {self.asset['id']})
        again = self.transcript_plan()
        self.assertEqual(({e['action'] for e in again['entries']}, again['files']), ({'archived'}, 0))

    def test_a_new_transcript_is_kept_beside_the_earlier_one(self):
        transcripts.copy(self.catalog, self.backend, self.transcript_plan())
        self.files['srt'].write_bytes(b'1\n00:00 --> 00:01\nhola de nuevo\n')
        value = self.transcript_plan()
        version = next(e for e in value['entries'] if e['format'] == 'srt')
        self.assertEqual(version['action'], 'version')
        transcripts.copy(self.catalog, self.backend, value)
        folder = self.archive / Path(version['target']).parent
        self.assertEqual(len(list(folder.glob('photo*.srt'))), 2)
        self.assertEqual(self.transcript_plan()['files'], 0, 'the version is adopted next time')

    def test_source_without_backup_is_noted_and_path_matching_links_without_ids(self):
        self.source.update(asset_id=None, member_sha256=None)
        linked = self.transcript_plan()
        self.assertEqual({e['asset_id'] for e in linked['entries']}, {self.asset['id']})
        self.source['path'] = str(self.work / 'elsewhere/unknown.mov')
        orphan = self.transcript_plan()
        self.assertEqual(orphan['entries'], [])
        self.assertTrue(any('no está respaldada' in n for n in orphan['notes']))

    def test_changed_source_after_export_stops_without_touching_media(self):
        value = self.transcript_plan()
        self.files['srt'].write_bytes(b'edited after export')
        with self.assertRaisesRegex(MediaError, 'cambió'):
            transcripts.copy(self.catalog, self.backend, value)

    def test_closeout_backs_up_transcripts_before_release_and_notes_clipper_offline(self):
        final = self.archive / '002_finales/e64/2026/intro.mov'
        final.parent.mkdir(parents=True); final.write_bytes(b'final')
        closure.save(self.catalog, self.backend, self.project_id, ['horizontal'], True, True,
                     [{'volume_id': self.a['id'], 'path': str(final.relative_to(self.archive)), 'role': 'horizontal'}])
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        self.assertEqual(plan['transcripts']['files'], 2)
        self.assertFalse(plan['already_complete'])
        raw = self.work / '001_proyectos/e64_raw/intro'
        self.files['srt'].write_bytes(b'changed after review')
        with self.assertRaises(MediaError):
            closure.apply_close(self.catalog, self.backend, plan['id'], plan['digest'], confirm_discard=True)
        self.assertTrue(raw.exists(), 'a transcript that cannot be verified stops the closeout before any release')
        plan = closure.plan_close(self.catalog, self.backend, self.project_id, 'MEDIA', 'ESSD')
        result = closure.apply_close(self.catalog, self.backend, plan['id'], plan['digest'], confirm_discard=True)
        self.assertEqual(result['state'], 'completed')
        # What the stopped run already verified is adopted, not copied twice.
        self.assertEqual((result['transcripts_result']['copied'], result['transcripts_result']['adopted']), (1, 1))
        self.assertTrue(any((self.archive / '004_datos').rglob('photo.srt')))
        self.fail = 'Clipper apagado'
        offline = transcripts.plan(self.catalog, self.backend, self.a['id'], [self.clipper])
        self.assertEqual((offline['entries'], offline['files']), ([], 0))
        self.assertTrue(any('no exportó' in n for n in offline['notes']))
