"""Clipper transcripts kept in MEDIA next to the backed-up video (contracts/clipper-transcripts-v1.md).

Clipper only exports (`library transcripts-export`); nüün is the only writer in MEDIA. Each file is linked to the
archived member it transcribes, copied with the verified Transfer engine into
`004_datos/clipper/transcripts/AAAA/MM/<archived clip>/` and recorded with a receipt. Nothing is deleted here.
"""
import json
import os
from pathlib import Path
import re

from . import clipper
from .common import MediaError, Tree, content_stamp, digest, hash_fd, now, require, uid

EXPORT_SCHEMA = 'clipper-transcript-export/1'
RECEIPT_SCHEMA = 'nu9ve-data-receipt/1'
AREA = '004_datos/clipper/transcripts'
FORMATS = {'transcript.json', 'provider-response.json', 'srt', 'vtt', 'ass'}
STAMP_KEYS = ('device', 'inode', 'size', 'mtime_ns', 'ctime_ns')


def export(catalog, root, timeout=600):
    """Clipper's read-only list of the current transcripts of one project folder, checked before use."""
    data = clipper.invoke(catalog, ['transcripts-export', str(root)], timeout=timeout)
    require(isinstance(data, dict) and data.get('schema') == EXPORT_SCHEMA
            and data.get('project_root') == os.path.realpath(root) and isinstance(data.get('entries'), list)
            and isinstance(data.get('digest'), str) and len(data['digest']) == 64,
            'clipper_invalid', 'Exportación de transcripciones de Clipper desconocida')
    for entry in data['entries']:
        source = entry.get('source') if isinstance(entry, dict) else None
        require(isinstance(entry, dict) and isinstance(entry.get('path'), str) and Path(entry['path']).is_absolute()
                and entry.get('format') in FORMATS and isinstance(entry.get('bytes'), int)
                and isinstance(entry.get('sha256'), str) and re.fullmatch('[0-9a-f]{64}', entry['sha256'])
                and isinstance(entry.get('stamp'), dict) and all(isinstance(entry['stamp'].get(k), int) for k in STAMP_KEYS)
                and entry['bytes'] >= 0 and entry['bytes'] == entry['stamp']['size']
                and isinstance(source, dict) and isinstance(source.get('path'), str),
                'clipper_invalid', 'Entrada de transcripción incompleta')
    return data


def _archived_member(catalog, archive_id, entry):
    """(asset_id, member path, archive destination) of the backed-up member this transcript belongs to, or None."""
    source = entry['source']
    asset_id, member_sha = source.get('asset_id'), source.get('member_sha256')
    if asset_id and member_sha:
        row = catalog.db.execute('SELECT payload FROM assets WHERE id=?', (asset_id,)).fetchone()
        if row:
            asset = json.loads(row[0])
            for (path,) in catalog.db.execute('SELECT relative_path FROM members WHERE job_id=? AND sha256=?',
                                              (asset['job_id'], member_sha)):
                if path in asset['paths']:
                    copy = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                              (asset['job_id'], archive_id, path)).fetchone()
                    if copy:
                        return asset_id, path, json.loads(copy[0])['destination']
    # No nüün identity: match the path Clipper transcribed with a registered work copy.
    wanted = os.path.realpath(source['path'])
    for volume in catalog.volumes():
        if volume['role'] != 'work' or not wanted.startswith(os.path.realpath(volume['root']) + '/'):
            continue
        rel = os.path.relpath(wanted, os.path.realpath(volume['root']))
        for payload, job_id in catalog.db.execute('SELECT payload,job_id FROM copies WHERE volume_id=?', (volume['id'],)):
            copy = json.loads(payload)
            destination = copy['destination']
            if rel == destination or rel.startswith(destination.rstrip('/') + '/'):
                member = copy['path'] if rel == destination else copy['path'].rstrip('/') + '/' + rel[len(destination) + 1:]
                archived = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                              (job_id, archive_id, member)).fetchone()
                if archived:
                    owner = next((a['id'] for a in catalog.search() if a['job_id'] == job_id and member in a['paths']), None)
                    return owner, member, json.loads(archived[0])['destination']
    return None


def target_folder(destination):
    """`AAAA/MM/<clip>` from the archived member: the year/month folders and the rest of its path without extension."""
    parts = Path(destination).parts
    for index in range(len(parts) - 2):
        if re.fullmatch(r'\d{4}', parts[index]) and re.fullmatch(r'\d{2}', parts[index + 1]):
            rest = list(parts[index + 2:])
            rest[-1] = Path(rest[-1]).stem
            return '/'.join([parts[index], parts[index + 1]] + rest)
    return 'sin-fecha/' + Path(destination).stem


def plan(catalog, backend, archive_id, roots, cancel=None):
    """What to copy for these Clipper project folders. Returns entries (copy / version / archived), notes and digests."""
    archive = catalog.volume(archive_id)
    require(archive['role'] == 'archive', 'invalid_role', 'Elige un volumen de respaldo')
    backend.check(archive)
    entries, notes, exports = [], [], []
    with Tree(archive['root']) as tree:
        for root in roots:
            require(not (cancel and cancel()), 'cancelled', 'Operación cancelada')
            try:
                data = export(catalog, root)
            except MediaError as error:
                notes.append(str(root) + ': Clipper no exportó sus transcripciones; siguen en clipper/ (' + str(error) + ')')
                continue
            exports.append({'project_root': data['project_root'], 'digest': data['digest']})
            for entry in data['entries']:
                found = _archived_member(catalog, archive['id'], entry)
                if not found:
                    notes.append(Path(entry['path']).name + ': su fuente no está respaldada; no se copia')
                    continue
                asset_id, member, destination = found
                name = Path(entry['path']).name
                folder = AREA + '/' + target_folder(destination)
                action, target = 'copy', folder + '/' + name
                existing = _sha(tree, target)
                if existing == entry['sha256']:
                    action = 'archived'
                elif existing is not None:
                    # A newer transcript keeps the earlier one; an identical earlier version is adopted instead.
                    base, _, ext = name.partition('.')
                    versions = sorted(_siblings(tree, folder, base + '.v'))
                    match = next((v for v in versions if _sha(tree, folder + '/' + v) == entry['sha256']), None)
                    if match:
                        action, target = 'archived', folder + '/' + match
                    else:
                        action, target = 'version', folder + '/' + base + '.v' + now()[:19].replace('-', '').replace(':', '') + '.' + ext
                entries.append({'path': entry['path'], 'format': entry['format'], 'bytes': entry['bytes'], 'sha256': entry['sha256'],
                                'stamp': {k: entry['stamp'][k] for k in STAMP_KEYS}, 'asset_id': asset_id, 'member': member,
                                'member_destination': destination, 'target': target, 'action': action,
                                'project_root': data['project_root']})
    pending = [e for e in entries if e['action'] != 'archived']
    return {'schema': 'nu9ve-transcripts-plan/1', 'archive_id': archive_id, 'entries': entries, 'notes': notes, 'exports': exports,
            'files': len(pending), 'bytes': sum(e['bytes'] for e in pending)}


def _sha(tree, rel):
    try:
        fd = tree.open(rel)
    except (FileNotFoundError, NotADirectoryError):
        return None
    try:
        return hash_fd(fd)
    finally:
        os.close(fd)


def _siblings(tree, folder, prefix):
    try:
        with tree.parent(folder + '/x') as (fd, _):
            return [n for n in os.listdir(fd) if n.startswith(prefix)]
    except (FileNotFoundError, NotADirectoryError):
        return []


def copy(catalog, backend, value, receipt_id=None, cancel=None, progress=None):
    """Copy and verify every pending transcript, write a receipt, and return what was copied and adopted.

    A source whose stamp changed since the export, or a destination that appeared with other bytes, stops before any
    further copy; nothing already verified is undone and nothing is ever deleted."""
    from .progress import ActionMeter
    from .transfer import Transfer
    archive = catalog.volume(value['archive_id'])
    require(archive['role'] == 'archive', 'invalid_role', 'Elige un volumen de respaldo')
    backend.check(archive)
    engine = Transfer(catalog, backend, cancel)
    pending = [e for e in value['entries'] if e['action'] != 'archived']
    meter = ActionMeter(2 * sum(e['bytes'] for e in pending), len(pending))
    done = []
    with Tree(archive['root']) as target:
        for entry in value['entries']:
            if entry['action'] == 'archived':
                require(_sha(target, entry['target']) == entry['sha256'], 'checksum_mismatch',
                        'Cambió una transcripción archivada; revisa de nuevo')
        for entry in pending:
            require(not (cancel and cancel()), 'cancelled', 'Copia de transcripciones pausada')
            meter.holding(entry['target'], 'clipper', archive['name'], 'copy')
            if progress:
                progress('copying_transcripts', **meter.fields())
            if _sha(target, entry['target']) == entry['sha256']:
                done.append(dict(entry, action='archived'))  # Copied by an earlier, interrupted run.
            else:
                source = Path(entry['path'])
                with Tree(source.parent) as origin:
                    require(content_stamp(origin.stat(source.name)) == {k: v for k, v in entry['stamp'].items() if k != 'device'},
                            'source_changed', 'Una transcripción cambió desde que Clipper la exportó; revisa de nuevo')
                    sha, stamp = engine.copy_file(origin, source.name, target, entry['target'], archive, expected=entry['sha256'])
                require(sha == entry['sha256'], 'checksum_mismatch', 'Transcripción copiada con otro contenido')
                done.append(dict(entry, action='copied', verified_at=now(), archive_stamp=stamp))
            meter.reach(meter.done + 2 * entry['bytes'])
            meter.finished('copied', entry['target'], archive['name'])
            if progress:
                progress('copying_transcripts', **meter.fields())
        receipt = {'schema': RECEIPT_SCHEMA, 'receipt_id': receipt_id or uid(), 'kind': 'clipper_transcripts',
                   'volume_id': archive['id'], 'created_at': now(), 'exports': value['exports'],
                   'files': [{k: e[k] for k in ('asset_id', 'member', 'member_destination', 'format', 'sha256', 'bytes', 'target', 'action')}
                             for e in done + [e for e in value['entries'] if e['action'] == 'archived']]}
        if done:
            target.write_json('.ncreator/receipts/data-' + receipt['receipt_id'] + '.json', receipt)
    with catalog.writer():
        catalog.set('data-receipt:' + receipt['receipt_id'], receipt)
    return {'receipt_id': receipt['receipt_id'], 'copied': sum(e['action'] == 'copied' for e in done),
            'adopted': sum(e['action'] == 'archived' for e in done) + sum(e['action'] == 'archived' for e in value['entries']),
            'bytes': sum(e['bytes'] for e in done if e['action'] == 'copied'), 'notes': value['notes']}


def clipper_roots(source_root, editors):
    """Clipper project folders inside the closeout's editor folders (`<editor>/clipper` or the editor itself)."""
    roots = []
    for editor in editors:
        for rel in (editor + '/clipper', editor):
            folder = Path(source_root) / rel
            if (folder / 'clipper-project.json').is_file() or (folder / 'clipper-retired-project.json').is_file():
                roots.append(folder)
                break
    return roots


def project_backup(catalog, backend, project_id, archive_id, cancel=None, progress=None):
    """«Respaldar transcripciones» for one project (and its linked ones): plan and copy in one step; nothing is deleted."""
    from .closure import project_record, related, roots as closure_roots
    roots = []
    for pid in [project_id] + related(catalog, project_id):
        record = project_record(catalog, pid)
        for volume_id, found in closure_roots(catalog, backend, record).items():
            roots += [r for r in clipper_roots(found['volume']['root'], found['editor']) if r not in roots]
    value = plan(catalog, backend, archive_id, roots, cancel)
    result = copy(catalog, backend, value, cancel=cancel, progress=progress)
    return dict(result, planned=value['files'], projects=len(roots), digest=digest(value))
