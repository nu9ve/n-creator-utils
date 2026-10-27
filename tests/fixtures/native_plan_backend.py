"""Fixture-only RPC for Swift plan/apply transport tests; all state lives under --state-dir."""
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(REPO), str(REPO / 'tests')]
from test_media_engine import FixtureVolumes
from media.catalog import Catalog
from media.common import canonical, digest
from media.service import Service

state = Path(sys.argv[sys.argv.index('--state-dir') + 1])
source = state / 'card'
class Backend(FixtureVolumes):
    def describe(self, root, external_required=False):
        info = super().describe(root, external_required)
        if Path(root) == source:
            info['root_inode'] = 2**64 - 1
        return info

backend = Backend()
for folder in ('card', 'archive', 'work'):
    (state / folder).mkdir(parents=True, exist_ok=True)
if not (source / 'photo.JPG').exists():
    (source / 'photo.JPG').write_bytes(b'fixture-photo')
catalog = Catalog(state / 'catalog')
if not catalog.volumes():
    for name, role, folder, prefix in [('MEDIA', 'archive', 'archive', '001_raw'), ('ESSD', 'work', 'work', '001_proyectos')]:
        root = state / folder
        catalog.register(name, role, root, backend.describe(root), prefix)
request = json.load(sys.stdin)
try:
    service = Service(catalog, backend)
    if request['method'] == 'plan_document':
        plan = service.call('plan', request['params'])
        # Unknown compatible metadata exercises exact numeric transport, including 1.0 vs 1.
        plan['numeric_transport_fixture'] = {'integral_float': 1.0, 'fraction': 0.12345678901234568,
                                             'small': 1e-7, 'unsigned': 2**64 - 1}
        plan.pop('digest')
        plan['digest'] = digest(plan)
        value = canonical(plan).decode()
    else:
        value = service.call(request['method'], request.get('params'))
    print(json.dumps({'ok': True, 'data': value}))
except Exception as error:
    print(json.dumps({'ok': False, 'error': {'code': getattr(error, 'code', 'fixture_error'), 'message': str(error)}}))
finally:
    catalog.close()
