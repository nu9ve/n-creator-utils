import json

from test_media_engine import MediaFixture
from media.common import MediaError
from media.service import Service


class PlanDocumentTests(MediaFixture):
    def document(self):
        return Service(self.catalog, self.backend).call('plan_document', {
            'source': str(self.source), 'archive': 'MEDIA', 'work': 'ESSD',
            'project': 'transport', 'cleanup': False})

    def test_original_document_applies_and_preserves_source(self):
        document = self.document()
        plan = json.loads(document)
        result = Service(self.catalog, self.backend).call('apply', {'plan': document, 'digest': plan['digest']})
        self.assertEqual(result['state'], 'source_retained')
        self.assertEqual(result['plan'], plan)
        self.assertTrue((self.source / 'DCIM/photo.JPG').exists())

    def test_tampered_document_is_rejected_before_job_or_copy(self):
        plan = json.loads(self.document())
        digest = plan['digest']
        plan['project'] = 'tampered'
        with self.assertRaises(MediaError) as failure:
            Service(self.catalog, self.backend).call('apply', {'plan': json.dumps(plan), 'digest': digest})
        self.assertEqual(failure.exception.code, 'plan_changed')
        self.assertEqual(self.catalog.jobs(), [])
        self.assertEqual(list(self.archive.iterdir()), [])

    def test_wrong_reviewed_digest_is_rejected(self):
        with self.assertRaises(MediaError) as failure:
            Service(self.catalog, self.backend).call('apply', {'plan': self.document(), 'digest': '0' * 64})
        self.assertEqual(failure.exception.code, 'plan_changed')
        self.assertEqual(self.catalog.jobs(), [])

    def test_dict_plan_clients_remain_supported(self):
        plan = json.loads(self.document())
        result = Service(self.catalog, self.backend).call('apply', {'plan': plan, 'digest': plan['digest']})
        self.assertEqual(result['state'], 'source_retained')
