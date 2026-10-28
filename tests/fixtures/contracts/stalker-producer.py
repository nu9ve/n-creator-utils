#!/usr/bin/env python3
"""Executable protocol fixture. Writes ONLY under an explicitly supplied temporary fixture root."""
import hashlib
import json
import os
from pathlib import Path
import sys
import time

root = Path(os.environ['STALKER_RETITLE_FIXTURE_ROOT']).resolve(strict=True)
request = json.load(sys.stdin)
action = sys.argv[2]
capability = 'stalker-coordinated-retitle/1'
if action == 'capabilities':
    print(json.dumps({'schema':'stalker-capabilities/1','capabilities':[capability]}))
    sys.exit(0)
marker = root / (request['operation_id'] + '.result.json')
if marker.exists():
    prior = json.loads(marker.read_text())
    if prior['request'] != request:
        sys.exit(2)
    print(json.dumps(prior['result']))
    sys.exit(0)
if action == 'status':
    print(json.dumps({'schema':'stalker-retitle-result/1','operation_id':request['operation_id'],'session_id':request['session_id'],'status':'not_found'}))
    sys.exit(0)
assert action == 'apply'
# Both kernel locks survive coordinator death until this child finishes.
fds = [int(fd) for fd in os.environ['NUUN_RETITLE_LOCK_FDS'].split(',')]
assert len(fds) == 2
for fd in fds: os.fstat(fd)
if os.environ.get('STALKER_RETITLE_FIXTURE_PAUSE'):
    (root / 'producer-entered').write_text(str(os.getpid()))
    deadline = time.monotonic() + 15
    while not (root / 'producer-resume').exists():
        assert time.monotonic() < deadline
        time.sleep(0.02)
path = Path(request['path']).resolve(strict=True)
assert root in path.parents
manifest_path = path / 'manifest.json'
raw = manifest_path.read_bytes()
assert hashlib.sha256(raw).hexdigest() == request['expected_manifest_sha256']
manifest = json.loads(raw)
manifest['project'] = request['project']
raw = json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode()
manifest_path.write_bytes(raw)
result = {'schema':'stalker-retitle-result/1','operation_id':request['operation_id'],
          'session_id':request['session_id'],'status':'applied','path':str(path),
          'project':request['project'],'previous_manifest_sha256':request['expected_manifest_sha256'],
          'manifest_sha256':hashlib.sha256(raw).hexdigest()}
marker.write_text(json.dumps({'request':request,'result':result}))
print(json.dumps(result))
