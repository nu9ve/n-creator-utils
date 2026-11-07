"""Project closeout: explicit delivery membership and reviewed release, never an ingest replacement.

Only named regenerable folders may be discarded without a replica. Editor projects,
FCP libraries and unknown files are preserved. All media IO uses the shared engines.
"""
import json
import os
from contextlib import nullcontext
from pathlib import Path
import re
import stat

from . import clipper
from .common import Tree, content_stamp, digest, now, relative, require, uid
from .finals import OPAQUE, published_folder, published_receipts, subfolders, year_in_name
from .layout import MEDIA_KINDS, kind_of
from .release import checked_hash, checked_hash_progress, plan_release, report
from .volumes import CachedBackend, independent

ROLES = {'horizontal', 'vertical', 'thumbnail', 'auxiliary'}


def project_record(catalog, project_id):
    row = catalog.db.execute('SELECT * FROM projects WHERE id=?', (project_id,)).fetchone()
    require(row is not None, 'unknown_project', 'Proyecto desconocido')
    require(row['title'] != 'finales', 'invalid_project', 'Elige el proyecto original, no la colección finales')
    return dict(row)


def settings(catalog, project_id):
    project = project_record(catalog, project_id)
    default = ['horizontal'] if project['profile'] == '9logs' else ['horizontal', 'vertical', 'thumbnail']
    profile = catalog.get('closure-profile:' + project['profile'], default)
    return catalog.get('project-closure:' + project_id,
                       {'required': profile, 'no_more': False, 'published': False, 'finals': []})


def save(catalog, backend, project_id, required, no_more, published, finals, profile_default=False):
    with catalog.writer():
        return _save(catalog, backend, project_id, required, no_more, published, finals, profile_default)


def _save(catalog, backend, project_id, required, no_more, published, finals, profile_default):
    project = project_record(catalog, project_id)
    previous_digest = digest(settings(catalog, project_id))
    require(isinstance(required, list) and required and set(required) <= ROLES - {'auxiliary'},
            'invalid_request', 'Elige los tipos de entrega requeridos')
    require(all(isinstance(v, bool) for v in (no_more, published, profile_default)), 'invalid_request', 'Decisiones inválidas')
    require(isinstance(finals, list) and len(finals) <= 1000, 'invalid_request', 'Selección de finales inválida')
    normalized, seen = [], set()
    receipts = published_receipts(catalog) if not published and any(isinstance(f, dict) and 'published' not in f for f in finals) else {}
    for item in finals:
        require(isinstance(item, dict) and item.get('role') in ROLES, 'invalid_request', 'Rol de entrega inválido')
        volume = catalog.volume(item['volume_id'])
        path = relative(item['path'])
        prefix = volume.get('finals_prefix', '002_finales') + '/' + project['profile'] + '/'
        require(volume['role'] in {'archive', 'work'} and path.startswith(prefix), 'unsafe_path', 'Selecciona finales del mismo perfil')
        require(not any(p.startswith('.') or p.lower().endswith(('.fcpbundle', '.stalker', '.app', '.photoslibrary')) for p in Path(path).parts),
                'unsafe_path', 'No se adoptan bibliotecas ni bundles como finales')
        key = (volume['id'], path)
        require(key not in seen, 'invalid_request', 'Final repetido')
        seen.add(key)
        backend.check(volume)
        with Tree(volume['root']) as tree:
            stamp = content_stamp(tree.stat(path))
        # A final cannot be silently reassigned from another project.
        for row in catalog.db.execute("SELECT key,value FROM metadata WHERE key LIKE 'project-closure:%'"):
            if row['key'] == 'project-closure:' + project_id:
                continue
            require(not any((f['volume_id'], f['path']) == key for f in json.loads(row['value']).get('finals', [])),
                    'final_already_linked', 'Este final ya pertenece a otro proyecto')
        require('published' not in item or isinstance(item['published'], bool), 'invalid_request', 'Publicado debe ser booleano')
        profile_root = Path(volume['root']) / volume.get('finals_prefix', '002_finales') / project['profile']
        folder = published_folder(catalog, project['profile'], subfolders(profile_root)) if profile_root.is_dir() else ''
        in_published = bool(folder) and Path(path).relative_to(volume.get('finals_prefix', '002_finales') + '/' + project['profile']).parts[0] == folder
        normalized.append({'volume_id': volume['id'], 'path': path, 'role': item['role'], 'stamp': stamp,
                           'published': published or item.get('published', in_published or receipts.get(key) == stamp)})
    value = {'required': sorted(set(required)), 'no_more': no_more, 'published': bool(normalized) and all(f['published'] for f in normalized),
             'finals': normalized, 'updated_at': now()}
    changes = {'project-closure:' + project_id: value}
    if profile_default:
        changes['closure-profile:' + project['profile']] = value['required']
    catalog.set_many(changes, event=(project_id, 'project_delivery_decision', {
        'project_id': project_id, 'previous_digest': previous_digest, 'required': value['required'],
        'no_more': no_more, 'published': value['published'],
        'finals': [{k: f[k] for k in ('volume_id', 'path', 'role', 'published')} for f in normalized]}))
    return value


def role_hint(path):
    text = str(path).lower()
    if Path(text).suffix in {'.jpg', '.jpeg', '.png', '.webp', '.heic'}:
        return 'thumbnail'
    if Path(text).suffix in {'.txt', '.srt', '.vtt'}:
        return 'auxiliary'
    folders = Path(text).parts[:-1]
    vertical_folder = any(re.fullmatch(r'(?:\d+[_-])?(?:shorts|verticales|reels)', folder) for folder in folders)
    return 'vertical' if kind_of(text) == 'video' and (vertical_folder or VERTICAL_SUFFIX.search(Path(text).stem)) else 'horizontal'


VERTICAL_SUFFIX = re.compile(r'[._ -](?:short|vertical|reel)(?:[._ -]?\d+)?$', re.IGNORECASE)


def normalized_name(value):
    return re.sub(r'[^\w]+|_', '-', value.casefold()).strip('-')


def delivery_title(name):
    stem = Path(name).stem.casefold()
    if year_in_name(stem):
        stem = re.sub(r'^(?:\d{2,4}-\d{2}-\d{2}|\d{6})[_-]+', '', stem)
    stem = VERTICAL_SUFFIX.sub('', stem)
    stem = re.sub(r'\.thumbnail(?:[-_.].*)?$', '', stem)
    return normalized_name(stem.removeprefix('titulos-'))


def delivery_decisions(catalog):
    result = {}
    prefix = 'project-closure:'
    for key, payload in catalog.db.execute("SELECT key,value FROM metadata WHERE key LIKE 'project-closure:%'"):
        config = json.loads(payload)
        for final in config['finals']:
            result[(final['volume_id'], final['path'])] = (
                key[len(prefix):], dict(final, published=final.get('published', config['published'])))
    return result


def publication_hint(key, info, in_published, receipts, saved):
    stamp = content_stamp(info)
    if saved and saved[1]['stamp'] == stamp:
        return saved[1]['published']
    return in_published or receipts.get(key) == stamp


def project_suggestions(catalog, verticals):
    """Only an exact title/alias or an explicit association can suggest a project in Inicio."""
    projects = {p['id']: dict(p) for p in catalog.db.execute("SELECT * FROM projects WHERE title!='finales'")}
    names = {}
    for profile, alias, project_id in catalog.db.execute('SELECT profile,alias,project_id FROM project_aliases'):
        names.setdefault((profile, normalized_name(alias)), set()).add(project_id)
    counts = {}
    for profile, files in verticals.items():
        for name, assigned in files.items():
            matches = assigned or names.get((profile, delivery_title(name)), set())
            if len(matches) == 1:
                project_id = next(iter(matches))
                if project_id in projects:
                    counts[project_id] = counts.get(project_id, 0) + 1
    return [dict(projects[project_id], published_verticals=count) for project_id, count in counts.items()
            if not catalog.get('project-released:' + project_id)]


def delivery_files(root, cancel=None):
    for folder, directories, names in os.walk(root, followlinks=False):
        require(not (cancel and cancel()), 'cancelled', 'Búsqueda cancelada')
        directories[:] = sorted(d for d in directories if not d.startswith('.') and Path(d).suffix.lower() not in OPAQUE)
        for name in sorted(names):
            path = Path(folder) / name
            if name.startswith('.') or (kind_of(name) not in MEDIA_KINDS and path.suffix.lower() not in {'.txt', '.srt', '.vtt'}):
                continue
            info = path.lstat()
            if stat.S_ISREG(info.st_mode):
                yield path, info


def candidates(catalog, backend, project_id, query=None, cancel=None):
    project = project_record(catalog, project_id)
    default_query = query is None
    query = project['title'] if query is None else query
    require(isinstance(query, str), 'invalid_request', 'Búsqueda inválida')
    tokens = [s for s in normalized_name(query).split('-') if len(s) > 1]
    values = []
    saved = settings(catalog, project_id)
    decisions = delivery_decisions(catalog)
    receipts = published_receipts(catalog)
    aliases = {normalized_name(project['title'])}
    aliases.update(normalized_name(row[0]) for row in catalog.db.execute(
        'SELECT alias FROM project_aliases WHERE project_id=?', (project_id,)))
    for volume in catalog.volumes():
        if not backend.online(volume):
            continue
        prefix = volume.get('finals_prefix', '002_finales') + '/' + project['profile']
        root = Path(volume['root']) / prefix
        published = published_folder(catalog, project['profile'], subfolders(root)) if root.is_dir() else ''
        for path, info in delivery_files(Path(volume['root']) / prefix, cancel):
            name = str(path.relative_to(Path(volume['root']) / prefix))
            suggested = delivery_title(name) in aliases
            if not (default_query and suggested) and tokens and not all(t in normalized_name(name) for t in tokens):
                continue
            rel = str(path.relative_to(volume['root']))
            key = (volume['id'], rel)
            prior = decisions.get(key)
            if prior and prior[0] != project_id:
                continue
            declared = publication_hint(key, info, bool(published) and Path(name).parts[0] == published, receipts, prior)
            values.append({'volume_id': volume['id'], 'volume': volume['name'], 'path': rel,
                           'name': name, 'bytes': info.st_size, 'role': role_hint(name), 'archived': volume['role'] == 'archive',
                           'published': declared, 'suggested': suggested,
                           'publication_changed': bool(prior and prior[1]['stamp'] != content_stamp(info))})
    values.sort(key=lambda v: (not v['suggested'], v['archived'], v['name']))
    return {'files': values[:1000], 'truncated': len(values) > 1000, 'settings': settings(catalog, project_id)}


def roots(catalog, backend, project):
    """Receipt paths win over current profile defaults or renamed project titles."""
    assets = [a for a in catalog.search() if a.get('project_id') == project['id'] and a['kind'] != 'delivery']
    found = {}
    for volume in catalog.volumes():
        if volume['role'] != 'work' or not backend.online(volume):
            continue
        raw = set()
        for asset in assets:
            for row in catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=?', (asset['job_id'], volume['id'])):
                copy = json.loads(row[0])
                if copy['path'] not in asset['paths']:
                    continue
                parts = Path(copy['destination']).parts
                # A release scope must be the project below the registered work prefix/raw directory.
                job = catalog.job(asset['job_id'])['plan']
                route = job.get('profile_routes', {}).get(asset['profile'], asset['profile'] + '_raw')
                depth = len(Path(volume['prefix']).parts) + len(Path(route).parts) + 1
                if len(parts) >= depth + 1:
                    raw.add('/'.join(parts[:depth]))
        aliases = [r[0] for r in catalog.db.execute('SELECT alias FROM project_aliases WHERE project_id=?', (project['id'],))]
        titles = set([project['title']] + aliases)
        raw_dir = catalog.get('creative-profiles', {}).get(project['profile'], {}).get('raw_dir', project['profile'] + '_raw')
        raw.update(volume['prefix'] + '/' + raw_dir + '/' + title for title in titles)
        editor = [volume['prefix'] + '/' + project['profile'] + '/' + title for title in titles]
        found[volume['id']] = {'volume': volume, 'raw': sorted(raw), 'editor': sorted(editor)}
    return found


def folder_files(tree, path, cancel=None):
    """Opaque FCP/editor bundles are never traversed, even inside an intermediate folder."""
    stack = [path]
    while stack:
        folder = stack.pop()
        with tree.parent(folder + '/x') as (fd, _):
            for name in sorted(os.listdir(fd)):
                require(not (cancel and cancel()), 'cancelled', 'Operación cancelada')
                rel = folder + '/' + name
                st = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if name.startswith('.') or name.lower().endswith(('.fcpbundle', '.fcpcache', '.stalker', '.photoslibrary', '.app')):
                    continue
                require(not stat.S_ISLNK(st.st_mode), 'unsafe_file', 'Enlace en intermedios; se conserva: ' + rel)
                if stat.S_ISDIR(st.st_mode):
                    stack.append(rel)
                else:
                    require(stat.S_ISREG(st.st_mode), 'unsafe_file', 'Archivo especial: ' + rel)
                    yield rel


def inventory(tree, plan, cancel=None):
    from .release import selected_files
    paths, preserved = [], []
    for scope in plan['closure_scopes']:
        try:
            if scope['kind'] == 'raw':
                files, kept = selected_files(tree, scope['path'], cancel=cancel)
                preserved.extend(kept)
            else:
                files = list(folder_files(tree, scope['path'], cancel))
        except FileNotFoundError:
            files = []
        paths.extend(files)
    for entry in plan['entries']:
        if entry.get('category') == 'final':
            try:
                tree.stat(entry['path'])
                paths.append(entry['path'])
            except FileNotFoundError:
                pass
    return paths, preserved


def validate(catalog, backend, plan, cancel=None):
    """Frozen human decisions and every final are gates, including already-moved deliveries."""
    require(digest(settings(catalog, plan['closure_project_id'])) == plan['closure_settings_digest'],
            'plan_changed', 'Cambió el cierre del proyecto; revisa un plan nuevo')
    archive = catalog.volume(plan['archive_id'])
    backend.check(archive)
    guards = []
    kept = [final for final in plan['closure_finals'] if final.get('keep_work')]
    if kept:
        with Tree(plan['source_root']) as work:
            for final in kept:
                require(content_stamp(work.stat(final['original']['path'])) == final['original_stamp'],
                        'source_changed', 'Una entrega conservada cambió; revisa un plan nuevo')
    with Tree(archive['root']) as tree:
        for final in plan['closure_finals']:
            sha, stamp = checked_hash(tree, final['archive_path'], cancel)
            require(sha == final['sha256'],
                    'checksum_mismatch', 'Final sin respaldo íntegro: ' + final['archive_path'])
            guards.append(dict(final, stamp=stamp))
    return guards


def overview(catalog):
    """Metadata only: this is a reminder to review, never a claim that hashes still match."""
    values = []
    for row in catalog.db.execute("SELECT key,value FROM metadata WHERE key LIKE 'project-closure:%'"):
        project_id = row['key'][len('project-closure:'):]
        project = project_record(catalog, project_id)
        config = json.loads(row['value'])
        released = catalog.get('project-released:' + project_id)
        complete = set(config['required']) <= {f['role'] for f in config['finals']}
        values.append(dict(project, **config, released=released,
                           needs_release=not released and config['no_more'] and complete and bool(config['finals'])))
    return values


def archived_final_settings(catalog, plan, guards):
    """Build the association update for the release's atomic terminal commit.

    Use the stamps just checked under the release guard; no source or editor JSON
    is rewritten, and a later changed final will still require a fresh review.
    """
    config = settings(catalog, plan['closure_project_id'])
    stamps = {g['archive_path']: g['stamp'] for g in guards}
    by_original = {(f['original']['volume_id'], f['original']['path']): f
                   for f in plan['closure_finals'] if f.get('original') and not f.get('keep_work')}
    finals, seen = [], set()
    for item in config['finals']:
        final = by_original.get((item['volume_id'], item['path']))
        if final:
            item = dict(item, volume_id=plan['archive_id'], path=final['archive_path'], stamp=stamps[final['archive_path']])
        key = (item['volume_id'], item['path'])
        if key not in seen:
            seen.add(key); finals.append(item)
    return dict(config, finals=finals, updated_at=now())


def plan_close(catalog, backend, project_id, archive, work, cancel=None, release_work=True, progress=None):
    from .planner import build_plan, destination_path
    live_backend, backend = backend, CachedBackend(backend)
    report(progress, 'review_inventory')
    project = project_record(catalog, project_id)
    config = settings(catalog, project_id)
    config_digest = digest(config)
    require(isinstance(release_work, bool), 'invalid_request', 'release_work debe ser booleano')
    if release_work:
        require(config['no_more'], 'project_incomplete', 'Confirma que la selección contiene todas las entregas definitivas')
        require(set(config['required']) <= {f['role'] for f in config['finals']}, 'missing_deliveries', 'Faltan tipos de entrega requeridos')
    else:
        config = dict(config, finals=[f for f in config['finals'] if f.get('published', config['published'])])
        require(config['finals'], 'project_incomplete', 'Marca al menos un final como publicado')
    target, source = catalog.volume(archive), catalog.volume(work)
    require(source['role'] == 'work' and target['role'] == 'archive', 'invalid_role', 'Elige trabajo y respaldo')
    require(independent(backend.check(source), backend.check(target)), 'same_physical_disk', 'Trabajo y respaldo deben ser discos distintos')
    scoped = roots(catalog, backend, project).get(source['id'], {'raw': [], 'editor': []}) if release_work else {'raw': [], 'editor': []}
    entries, preserved, replacements, scopes, copies, finals, retained = [], [], {}, [], [], [], []
    for path in scoped['raw']:
        # Freeze known scopes even when absent, so arrivals invalidate a reviewed plan.
        scopes.append({'kind': 'raw', 'path': path})
        try:
            with Tree(source['root']) as tree:
                from .release import selected_files
                selected, kept = selected_files(tree, path, cancel=cancel)
        except FileNotFoundError:
            continue
        preserved.extend(kept)
        if not selected:
            continue
        part = plan_release(catalog, backend, target['id'], work=source['id'], path=path, project=project['title'],
                            cancel=cancel, progress=progress)
        entries.extend(dict(e, category='raw') for e in part['entries'])
        replacements.update(part['replacements'])
    # A project with no work receipts is not silently assumed to have backed-up sources.
    assets = [a for a in catalog.search() if a.get('project_id') == project_id and a['kind'] != 'delivery'] if release_work else []
    require(assets or not release_work, 'missing_sources', 'Este proyecto no tiene fuentes catalogadas; adopta/verifica sus raws antes de liberar')
    job_states = dict(catalog.db.execute('SELECT id,state FROM jobs'))
    already_verified = {(e['archive_path'], e['sha256']) for e in entries}
    raw_to_verify = {}
    for asset in assets:
        require(job_states.get(asset['job_id']) not in {'copying', 'cleaning_source', 'paused', 'failed', 'backup_verified'},
                'open_job', 'Hay una operación de fuentes pendiente')
        # Validate every original member via its receipt, also when its work copy was already removed.
        for original in asset['paths']:
            row = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                     (asset['job_id'], target['id'], original)).fetchone()
            require(row, 'missing_receipt', 'Falta respaldo registrado de una fuente del proyecto')
            copy = json.loads(row[0])
            key = (copy['destination'], copy['sha256'])
            if key not in already_verified:
                raw_to_verify[key] = None
            finals.append({'archive_path': copy['destination'], 'sha256': copy['sha256'], 'role': 'source'})
    if raw_to_verify:
        with Tree(target['root']) as tree:
            for index, (path, expected) in enumerate(raw_to_verify):
                sha, _ = checked_hash_progress(tree, path, cancel, progress, 'review_raw_archive', index, len(raw_to_verify))
                require(sha == expected, 'checksum_mismatch', 'Respaldo raw alterado')
    with Tree(source['root']) as tree:
        for editor in scoped['editor']:
            renders = editor + '/renders'
            scopes.append({'kind': 'discard', 'path': renders})
            if (Path(source['root']) / renders).is_dir():
                for path in folder_files(tree, renders, cancel):
                    sha, stamp = checked_hash_progress(tree, path, cancel, progress, 'review_intermediates')
                    entries.append({'path': path, 'stamp': stamp, 'sha256': sha, 'archive_path': None, 'category': 'intermediate'})
                    replacements[str(Path(source['root']) / path)] = None
            if (Path(source['root']) / editor).is_dir():
                retained.append(editor + ' (editable, Final Cut y contenido fuera de renders se conservan)')
    archived_candidates = None
    for index, item in enumerate(config['finals']):
        volume = catalog.volume(item['volume_id'])
        backend.check(volume)
        with Tree(volume['root']) as tree:
            sha, stamp = checked_hash_progress(tree, item['path'], cancel, progress, 'review_final', index, len(config['finals']))
        require(stamp == item['stamp'], 'source_changed', 'Un final cambió; vuelve a vincularlo')
        copy_required = keep_work = False
        if volume['role'] == 'archive':
            require(volume['id'] == target['id'], 'invalid_volume', 'El final está en otro respaldo')
            archive_path = item['path']
        else:
            require(volume['id'] == source['id'], 'invalid_volume', 'El final está en otro disco de trabajo')
            keep_work = not item.get('published', config['published'])
            # Selected deliveries go through the same verified Transfer engine, without ingest cleanup.
            prefix = source.get('finals_prefix', '002_finales') + '/' + project['profile']
            rel = str(Path(item['path']).relative_to(prefix))
            existing = None
            if archived_candidates is None:
                archived_candidates = {}
                for candidate, info in delivery_files(Path(target['root']) / target.get('finals_prefix', '002_finales') / project['profile'], cancel):
                    archived_candidates.setdefault((candidate.name, info.st_size), []).append(str(candidate.relative_to(target['root'])))
            for candidate_rel in archived_candidates.get((Path(rel).name, stamp['size']), []):
                with Tree(target['root']) as tree:
                    if checked_hash_progress(tree, candidate_rel, cancel, progress, 'review_final_archive')[0] == sha:
                        existing = candidate_rel
                        break
            if existing:
                archive_path = existing
            else:
                parent = str((Path(source['root']) / item['path']).parent)
                role = item['role']
                if role == 'auxiliary' and any('thumbnail' in p.lower() for p in Path(item['path']).parts):
                    role = 'thumbnail'
                transfer = build_plan(catalog, backend, source=parent, archive=target['id'], project=project['title'], profile=project['profile'],
                                      archive_only=True, confirm_archive_only=True, work_decision='skipped', cleanup=False,
                                      camera='generic', source_kind='delivery', delivery_paths=[Path(item['path']).name],
                                      finals_route=True, finals_role=role, cancel=cancel)
                archive_path = destination_path(target, transfer, transfer['units'][0], transfer['files'][0]['path'])
                with Tree(target['root']) as tree:
                    try:
                        existing_hash, _ = checked_hash_progress(tree, archive_path, cancel, progress, 'review_final_archive')
                    except FileNotFoundError:
                        pass
                    else:
                        require(existing_hash == sha, 'destination_conflict', 'Ya existe otro final distinto en el destino elegido')
                copies.append(transfer)
                copy_required = True
            if not keep_work:
                entries.append({'path': item['path'], 'stamp': stamp, 'sha256': sha, 'archive_path': archive_path, 'category': 'final'})
                replacements[str(Path(source['root']) / item['path'])] = str(Path(target['root']) / archive_path)
        finals.append({'archive_path': archive_path, 'sha256': sha, 'role': item['role'],
                       'original': {'volume_id': item['volume_id'], 'path': item['path']},
                       'original_stamp': stamp, 'copy_required': copy_required, 'keep_work': keep_work})
    targets, roles = {}, {}
    for final in finals:
        previous = targets.setdefault(final['archive_path'], final['sha256'])
        require(previous == final['sha256'], 'destination_conflict', 'Dos archivos distintos llegarían al mismo destino')
        if final['role'] != 'source':
            require(roles.setdefault(final['archive_path'], final['role']) == final['role'],
                    'destination_conflict', 'La misma entrega no puede contar como dos tipos distintos')
    needed = sum(f['stamp']['size'] for p in copies for f in p['files'])
    source_info, target_info = live_backend.check(source), live_backend.check(target)
    require(independent(source_info, target_info), 'same_physical_disk', 'Trabajo y respaldo deben ser discos distintos')
    require(not needed or target_info['free'] >= needed + max(64 * 1024 * 1024, needed // 20),
            'insufficient_space', 'No hay espacio para todos los finales seleccionados')
    require(release_work or entries, 'nothing_to_release', 'Los finales publicados elegidos ya están en el respaldo')
    require(len({e['path'] for e in entries}) == len(entries), 'overlapping_roots', 'Hay carpetas superpuestas; revisa las asociaciones')
    report(progress, 'inspecting_clipper' if entries else 'review_finishing')
    try:
        consumers = clipper.inspect(catalog, replacements) if entries else {'blockers': [], 'relinks': [], 'dependencies': None}
    except (RuntimeError, OSError, ValueError) as error:
        consumers = {'blockers': [str(error)], 'relinks': [], 'dependencies': None}
    value = {'schema': 'nu9ve-release/1', 'id': uid(), 'created_at': now(), 'archive_id': target['id'],
             'source_root': source['root'], 'source_identity': source_info, 'work_id': source['id'],
             'scope': source['prefix'], 'project': project['title'], 'entries': entries, 'preserved': sorted(set(preserved)),
             'replacements': replacements, 'replacement_checks': [], 'bytes': sum(e['stamp']['size'] for e in entries),
             'consumers': consumers, 'state': 'review', 'cleaned': {}, 'retained_editors': retained, 'closure_project_id': project_id,
             'closure_settings_digest': config_digest, 'closure_scopes': scopes, 'closure_finals': finals, 'closure_copies': copies,
             'closure_finish': release_work, 'closure_inventory_checked': True}
    value['digest'] = digest(value)
    with catalog.writer():
        catalog.set('release:' + value['id'], value)
    return value


def apply_close(catalog, backend, plan_id, reviewed_digest, cancel=None, progress=None, confirm_discard=False):
    from .release import apply_release, relink_release, stored_plan, check_source, validate_remaining
    from .transfer import Transfer
    plan = stored_plan(catalog, plan_id, reviewed_digest)
    require(plan.get('closure_project_id'), 'invalid_plan', 'No es un plan de cierre')
    if plan['state'] == 'completed':
        return dict(plan, closure_settings=settings(catalog, plan['closure_project_id']))
    require(confirm_discard is True, 'confirmation_required', 'Confirma el plan revisado antes de completar')
    with clipper.release_guard(catalog) if plan['entries'] else nullcontext():
        with catalog.writer():
            catalog.set('closure-consent:' + plan_id, {'digest': reviewed_digest, 'confirmed_at': now()})
        check_source(backend, plan)
        require(digest(settings(catalog, plan['closure_project_id'])) == plan['closure_settings_digest'], 'plan_changed', 'Cambió el cierre; revisa de nuevo')
        with Tree(plan['source_root']) as tree:
            validate_remaining(tree, plan, cancel)
        tracker = ActionProgress(progress, plan, catalog)
        report = tracker.stage
        relinks = []
        if plan['entries']:
            consumers = clipper.inspect(catalog, plan['replacements'])
            require(not consumers['blockers'], 'dependencies_blocked', '; '.join(consumers['blockers']))
            relinks = consumers['relinks']
        for transfer in plan['closure_copies']:
            running = []
            copier = Transfer(catalog, backend, cancel, hook=lambda stage, payload: tracker.copy_event(stage, payload, running))
            running.append(copier)
            copier.apply(transfer, transfer['digest'])
            tracker.copy_finished(transfer)
        report('verifying_finals')
        validate(catalog, backend, plan, cancel)
        if relinks:
            relink_release(catalog, backend, plan_id, reviewed_digest, cancel, progress=tracker.passthrough)
        result = apply_release(catalog, backend, plan_id, reviewed_digest, cancel, progress=tracker.release_event)
        report('release_complete', files_done=len(plan['entries']), files_total=len(plan['entries']))
        return dict(result, closure_settings=settings(catalog, plan['closure_project_id']))


class ActionProgress:
    """One overall measure for a closeout: copy + verify each final, then hash and retire each editing copy.

    Observers get `overall_done`/`overall_total` in bytes, the current file with its source and destination, and a
    short rolling log of finished steps. It only reports; it never decides what is copied or removed.
    """
    LOG_LIMIT = 60

    def __init__(self, progress, plan, catalog):
        self.progress = progress
        self.copy_total = sum(f['stamp']['size'] for t in plan['closure_copies'] for f in t['files'])
        self.release_sizes = [e['stamp']['size'] for e in plan['entries']]
        self.total = 2 * self.copy_total + sum(self.release_sizes)
        self.copy_base = 0
        self.files_total = sum(len(t['files']) for t in plan['closure_copies']) + len(plan['entries'])
        self.files_done = 0
        archive = catalog.volume(plan['archive_id']) or {}
        work = catalog.volume(plan['work_id']) or {} if plan.get('work_id') else {}
        self.archive_name, self.work_name = archive.get('name', ''), work.get('name', '')
        self.log, self.current, self.stage_name, self.done = [], {}, '', 0
        self.retired = set()

    def emit(self, **details):
        if self.progress:
            self.progress(self.stage_name, overall_done=min(self.done, self.total), overall_total=self.total,
                          files_done=self.files_done, files_total=self.files_total, current=self.current,
                          log=self.log[-self.LOG_LIMIT:], **details)

    def note(self, kind, path, target=''):
        self.log.append({'at': now(), 'kind': kind, 'name': Path(path).name, 'target': target})

    def stage(self, name, **details):
        self.stage_name = name
        self.emit()

    def passthrough(self, name, **details):
        self.stage(name)

    def copy_event(self, stage, payload, running):
        self.stage_name = 'copying_finals'
        copier = running[0] if running else None
        if stage == 'before_file':
            self.current = {'name': Path(payload['path']).name, 'from': self.work_name, 'to': self.archive_name, 'action': 'copy'}
        elif stage == 'after_file_verified':
            self.files_done += 1
            self.note('copied', payload.get('path') or payload.get('source', ''), self.archive_name)
        if copier and copier.progress:
            self.done = self.copy_base + copier.progress.get('copied_bytes', 0) + copier.progress.get('verified_bytes', 0)
        self.emit()

    def copy_finished(self, transfer):
        self.copy_base += 2 * sum(f['stamp']['size'] for f in transfer['files'])
        self.done = self.copy_base

    def release_event(self, stage, **details):
        self.stage_name = stage
        if stage == 'releasing_originals' and details.get('path'):
            index = details.get('files_done', 0)
            path = details['path']
            self.current = {'name': Path(path).name, 'from': self.work_name, 'to': '', 'action': 'release'}
            finished = 'bytes_done' not in details
            reading = 0 if finished else details.get('bytes_done', 0)
            completed = index if finished else index
            self.done = 2 * self.copy_total + sum(self.release_sizes[:completed]) + reading
            if finished and path not in self.retired:
                self.retired.add(path)
                self.files_done += 1
                self.note('released', path, self.work_name)
        self.emit()
