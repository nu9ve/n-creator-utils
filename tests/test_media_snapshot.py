import io
import json
from pathlib import Path
import tempfile
import unittest

from media.catalog import Catalog
from media.cli import serve_reads
from media.service import Service
from media.volumes import card_hint, source_cards
from tests.test_media_engine import FixtureVolumes


class CardVolumes(FixtureVolumes):
    """Fixture mounts: a card root is listed only when the test mounts it."""

    def __init__(self):
        super().__init__()
        self.mounted, self.describes = [], 0

    def mounted_roots(self):
        return list(self.mounted)

    def describe(self, root, external_required=False):
        self.describes += 1
        return super().describe(root, external_required)


class SnapshotFixture(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.card, self.archive = self.root / 'card', self.root / 'archive'
        self.card.mkdir(); self.archive.mkdir()
        self.catalog = Catalog(self.root / 'state')
        self.addCleanup(self.catalog.close)
        self.backend = CardVolumes()
        self.catalog.register('MEDIA', 'archive', self.archive, self.backend.describe(self.archive), '001_raw')
        self.service = Service(self.catalog, self.backend)

    def test_snapshot_returns_shell_state_and_describes_each_root_once(self):
        (self.card / 'PRIVATE/M4ROOT').mkdir(parents=True)
        self.backend.mounted = [str(self.card)]
        self.backend.describes = 0
        data = self.service.call('snapshot')
        for key in ('volumes', 'library', 'jobs', 'profiles', 'projects', 'release_plans',
                    'integration_settings', 'defaults', 'capabilities', 'source_cards'):
            self.assertIn(key, data)
        self.assertTrue(data['volumes'][0]['online'])
        self.assertEqual(data['source_cards'][0]['hint']['family'], 'sony')
        self.assertEqual(data['source_cards'][0]['hint']['recommendation'], 'sony_format_in_camera')
        # archive + card, each once, despite volumes/destination/card checks reading both.
        self.assertEqual(self.backend.describes, 2)

    def test_offline_destinations_keep_cards_without_content_hints(self):
        (self.card / 'DCIM/100GOPRO').mkdir(parents=True)
        self.backend.mounted = [str(self.card)]
        self.backend.offline.add(str(self.archive))
        cards = self.service.call('snapshot')['source_cards']
        self.assertEqual(len(cards), 1)
        self.assertNotIn('hint', cards[0])

    def test_card_hints_never_classify_dji_and_detect_known_families(self):
        cases = {'h6': ['ZOOM0001'], 'gopro': ['DCIM/100GOPRO'], 'dji': ['DCIM/DJI_001'],
                 'stalker': ['Stalker_x.stalker'], 'generic': ['DCIM/100MEDIA']}
        for family, folders in cases.items():
            with self.subTest(family=family), tempfile.TemporaryDirectory() as folder:
                for name in folders:
                    (Path(folder) / name).mkdir(parents=True)
                hint = card_hint(folder)
                self.assertEqual(hint['family'], family)
        self.assertEqual(card_hint(self.card)['recommendation'], 'archive_and_work')
        dji = self.root / 'dji'; (dji / 'DCIM/DJI_001').mkdir(parents=True)
        self.assertEqual(card_hint(dji)['recommendation'], 'dji_choose_category')

    def test_source_cards_without_hints_flag_lists_no_contents(self):
        self.backend.mounted = [str(self.card)]
        self.assertNotIn('hint', source_cards(self.catalog, self.backend, hints=False)[0])

    def test_read_server_answers_reads_and_refuses_mutations(self):
        requests = [{'id': 1, 'method': 'volumes'}, {'id': 2, 'method': 'defaults', 'params': {'project': 'x'}},
                    {'id': 3, 'method': 'apply', 'params': {}}, {'id': 4, 'method': 'defaults'}]
        output = io.StringIO()
        serve_reads(self.service, io.StringIO('\n'.join(json.dumps(r) for r in requests) + '\n\n'), output)
        lines = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([line['id'] for line in lines], [1, 2, 3, 4])
        self.assertTrue(lines[0]['ok'])
        self.assertEqual(lines[1]['error']['code'], 'read_only')
        self.assertEqual(lines[2]['error']['code'], 'read_only')
        self.assertTrue(lines[3]['ok'])
        self.assertNotEqual(self.catalog.get('defaults', {}).get('project'), 'x')


if __name__ == '__main__':
    unittest.main()
