#!/usr/bin/env python3
"""Run Stalker's isolated bridge with automatic nüün pending recovery.

Requires the sibling Stalker checkout/debug binary; no build/install or personal state.
The upstream suite is loaded read-only. Its historical manual recovery is replaced
with the public coordinator retry, exercising the production branch.
"""
import argparse
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest import mock
import os


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stalker-repo', type=Path, default=Path(__file__).resolve().parents[3] / 'ai/stalker')
    args = parser.parse_args()
    script = args.stalker_repo / 'scripts/test-retitle-bridge.py'
    spec = importlib.util.spec_from_file_location('stalker_bridge', script)
    bridge = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bridge)

    def crash_recovery(self, stage):
        with mock.patch.dict(os.environ, {'STALKER_RETITLE_FIXTURE_CRASH': stage}):
            with self.assertRaises(bridge.MediaError): bridge.retitle(self.catalog, **self.req)
        operation = self.catalog.get('retitle:' + self.req['operation_id'])
        self.assertEqual(operation['state'], 'pending')
        status = bridge.producer(self.catalog, 'status', operation['request'])
        self.assertEqual(status['status'], 'applied' if stage == 'applied' else ('not_found' if stage == 'before_intent' else 'pending'))
        # No direct producer apply or manual journal edits: the real coordinator dispatches recovery.
        result = bridge.retitle(self.catalog, **self.req)
        self.assertEqual(result['state'], 'applied')
        self.assert_intact(Path(result['result']['path']))
        self.assertEqual(bridge.retitle(self.catalog, **self.req), result)

    bridge.Bridge.crash_recovery = crash_recovery
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(bridge))
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    sys.exit(main())
