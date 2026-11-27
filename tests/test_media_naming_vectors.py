"""The shared naming vectors (docs/contracts/examples/media-naming-vectors-v1.json) against nüün's Python.

The same file is checked by the Rust engine (`layout.rs`, `naming_vectors`) and copied by Stalker into its own tests,
so a change of the grammar shows in all three instead of being copied by hand. A failure here means the producer
changed: update the vectors on purpose and tell Stalker (docs/contracts/README.md).
"""
import datetime
import json
import os
from pathlib import Path
import time
import unittest

from media import layout
from media.common import MediaError, slug

VECTORS = Path(__file__).resolve().parents[1] / 'docs' / 'contracts' / 'examples' / 'media-naming-vectors-v1.json'


class NamingVectorsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.vectors = json.loads(VECTORS.read_text(encoding='utf-8'))

    def text_cases(self, name, fn):
        for case in self.vectors[name]:
            with self.subTest(name, input=case['input']):
                if 'error' in case:
                    with self.assertRaises(MediaError) as raised:
                        fn(case['input'])
                    self.assertEqual(raised.exception.code, case['error'])
                else:
                    self.assertEqual(fn(case['input']), case['output'])

    def test_slug(self):
        self.text_cases('slug', slug)

    def test_tag(self):
        self.text_cases('tag', layout.tag)

    def test_duration(self):
        for case in self.vectors['duration']:
            self.assertEqual(layout.duration(case['seconds']), case['output'], case)

    def test_stamp(self):
        for case in self.vectors['stamp']:
            self.assertEqual(layout.stamp(datetime.datetime.fromisoformat(case['local'])), case['output'], case)

    def test_bundle_name(self):
        # The bundle name uses the host zone, as at ingest; the vectors fix it.
        before = os.environ.get('TZ')
        os.environ['TZ'] = self.vectors['time_zone']
        time.tzset()
        try:
            for case in self.vectors['bundle_name']:
                manifest = {'createdAt': case['created_at']}
                if case['duration']:
                    manifest['duration'] = case['duration']
                base = layout.stalker_bundle_name(manifest, 0, case['country'], case['project'])[0]
                taken = {}
                for _ in range(case['attempt']):
                    name = layout.claim_name(taken, base)
                self.assertEqual(name, case['output'], case)
        finally:
            if before is None:
                os.environ.pop('TZ', None)
            else:
                os.environ['TZ'] = before
            time.tzset()

    def test_every_section_has_cases(self):
        for name in ('slug', 'tag', 'duration', 'stamp', 'bundle_name'):
            self.assertTrue(self.vectors[name], name)


if __name__ == '__main__':
    unittest.main()
