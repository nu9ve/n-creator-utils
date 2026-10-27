"""Consistent full SQLite snapshots, including human decisions and workspace profiles."""
import os
import sqlite3
import tempfile
from pathlib import Path

from .common import Tree, hash_fd, now, require, uid
from .transfer import Transfer


def backup_metadata(catalog, backend, archive, cancel=None, locked=False):
    if not locked:
        with catalog.writer():
            return backup_metadata(catalog, backend, archive, cancel, locked=True)
    record = catalog.volume(archive)
    require(record['role'] == 'archive', 'invalid_role', 'Metadata backup requires an archive disk')
    backend.check(record)
    name = now().replace(':', '-') + '-' + uid() + '.sqlite3'
    with tempfile.TemporaryDirectory(prefix='catalog-backup-', dir=catalog.home) as temporary:
        path = Path(temporary) / 'catalog.sqlite3'
        output = sqlite3.connect(str(path))
        try:
            catalog.db.backup(output)
            require(output.execute('PRAGMA integrity_check').fetchone()[0] == 'ok', 'invalid_backup', 'Catalog snapshot failed integrity check')
        finally:
            output.close()
        with Tree(temporary) as source, Tree(record['root']) as target:
            fd = source.open(path.name)
            try:
                sha = hash_fd(fd, cancel or (lambda: False))
            finally:
                os.close(fd)
            rel = '.ncreator/catalog-backups/' + name
            Transfer(catalog, backend, cancel).copy_file(source, path.name, target, rel, record, expected=sha)
            receipt = {'schema': 'nu9ve-catalog-backup/1', 'path': rel, 'sha256': sha, 'created_at': now(),
                       'includes': ['all SQLite tables', 'projects', 'aliases', 'profiles', 'capture tags', 'integration settings', 'release journals']}
            target.write_json(rel + '.json', receipt)
    catalog.set('last-metadata-backup', receipt)
    return receipt
