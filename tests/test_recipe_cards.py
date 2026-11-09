"""Card families and naming tags come from recipe data; the template reproduces the previous built-in rules."""
import copy
import tempfile
import unittest
from pathlib import Path

from media.common import MediaError
from media.recipes import TEMPLATE, card_family, model_source, recipe
from media.volumes import card_hint


def family_of(folders, value=None):
    with tempfile.TemporaryDirectory() as root:
        for name in folders:
            path = Path(root) / name
            path.mkdir(parents=True)
            (path / 'x').write_bytes(b'1')   # folders with content decide first
        return card_hint(root, value and _Catalog(value))['family']


class _Catalog:
    def __init__(self, saved):
        self.saved = saved

    def get(self, key, default=None):
        return self.saved if key == 'recipe' else default


class RecipeCardTests(unittest.TestCase):
    def test_template_reproduces_builtin_families(self):
        cases = {
            'stalker': ['S_1.stalker'], 'sony': ['PRIVATE/M4ROOT/CLIP'], 'h6': ['ZOOM0001'], 'gopro': ['DCIM/100GOPRO'],
            'insta360': ['DCIM/Camera01', 'MISC'], 'dji': ['DCIM/101MEDIA', 'MISC'], 'canon': ['DCIM/100CANON'],
            'generic': ['DCIM/100MEDIA'],
        }
        for family, folders in cases.items():
            with self.subTest(family=family):
                self.assertEqual(family_of(folders), family)
        self.assertEqual(family_of(['AVF_INFO']), 'sony')
        self.assertEqual(family_of(['DJI_001']), 'dji')

    def test_order_and_recommendations_are_data(self):
        top, second = {'private'}, {'private': {'m4root'}}
        rule = card_family(TEMPLATE, top, second)
        self.assertEqual((rule['family'], rule['recommendation'], rule['cleanup']), ('sony', 'sony_format_in_camera', 'sony_clips_then_format'))
        self.assertIs(card_family(TEMPLATE, set(), {}), TEMPLATE['card_default'])

    def test_a_new_camera_is_a_recipe_entry_not_code(self):
        value = copy.deepcopy(TEMPLATE)
        value['cards'].insert(0, {'family': 'blackmagic', 'label': 'Blackmagic', 'recommendation': 'archive_and_work',
                                  'any': [{'top_prefix': ['blackmagic']}]})
        self.assertEqual(family_of(['BlackmagicPocket'], value), 'blackmagic')
        self.assertEqual(family_of(['BlackmagicPocket']), 'generic')

    def test_unknown_condition_is_rejected(self):
        value = copy.deepcopy(TEMPLATE)
        value['cards'] = [{'family': 'x', 'recommendation': 'archive_and_work', 'any': [{'top_glob': ['*']}]}]
        with self.assertRaises(MediaError):
            card_family(value, {'a'}, {})

    def test_saved_recipe_without_cards_inherits_template_sections(self):
        saved = {k: v for k, v in copy.deepcopy(TEMPLATE).items() if k not in {'cards', 'card_default', 'models', 'family_source'}}
        effective = recipe(_Catalog(saved))
        self.assertEqual(effective['cards'], TEMPLATE['cards'])
        self.assertEqual(model_source(effective, 'ILCE-6400'), 'SONYA6400')


if __name__ == '__main__':
    unittest.main()


class RecipeServiceTests(unittest.TestCase):
    def test_recipe_rpc_is_read_only_summary(self):
        from media.catalog import Catalog
        from media.cli import READ_METHODS
        from media.service import Service
        with tempfile.TemporaryDirectory() as state:
            catalog = Catalog(Path(state).resolve())
            try:
                data = Service(catalog).call('recipe', {})
            finally:
                catalog.close()
        self.assertIn('recipe', READ_METHODS)
        self.assertEqual(data['recipe']['id'], 'nu9ve-default')
        self.assertEqual([c['family'] for c in data['cards']][:2], ['stalker', 'sony'])
        self.assertEqual(data['missions'][0]['id'], 'subaereo')
        self.assertIn('dji-drone', data['categories'])
