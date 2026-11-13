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
from .common import MediaError, Tree, content_stamp, digest, now, relative, require, uid
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


def related(catalog, project_id):
    """Projects the person confirmed as the same work (a re-recording). Symmetric, one level, same profile."""
    known = {row[0] for row in catalog.db.execute('SELECT id FROM projects')}
    return [p for p in settings(catalog, project_id).get('related', []) if p in known and p != project_id]


def group(catalog, project_id):
    return [project_id] + related(catalog, project_id)


def group_digest(catalog, project_id):
    """Digest of every linked project's decisions; a lone project keeps its historical digest."""
    ids = group(catalog, project_id)
    if len(ids) == 1:
        return digest(settings(catalog, project_id))
    return digest({pid: settings(catalog, pid) for pid in ids})


def link_candidates(catalog, project_id):
    """Same-profile projects to link. Suggested when one title is the other's followed by «-»
    (terra-6-1 ↔ terra-6-1-extra); the person confirms, nothing is linked here."""
    project = project_record(catalog, project_id)
    mine, linked = normalized_name(project['title']), set(related(catalog, project_id))
    values = []
    for row in catalog.db.execute("SELECT id,title,profile FROM projects WHERE profile=? AND id!=? AND title!='finales'",
                                  (project['profile'], project_id)):
        other = normalized_name(row['title'])
        values.append({'id': row['id'], 'title': row['title'], 'linked': row['id'] in linked,
                       'suggested': other.startswith(mine + '-') or mine.startswith(other + '-')})
    values.sort(key=lambda v: (not v['linked'], not v['suggested'], v['title']))
    return values


def link(catalog, project_id, other_id, linked=True):
    """Record (or remove) the person's confirmation that two projects are one work. Moves no media."""
    first, second = project_record(catalog, project_id), project_record(catalog, other_id)
    require(isinstance(linked, bool), 'invalid_request', 'linked debe ser booleano')
    require(first['id'] != second['id'], 'invalid_request', 'Elige otro proyecto')
    require(first['profile'] == second['profile'], 'invalid_request', 'Vincula proyectos del mismo perfil')
    with catalog.writer():
        changes = {}
        for me, other in ((first, second), (second, first)):
            config = settings(catalog, me['id'])
            ids = set(config.get('related', []))
            if linked:
                ids.add(other['id'])
            else:
                ids.discard(other['id'])
            changes['project-closure:' + me['id']] = dict(config, related=sorted(ids), updated_at=now())
        catalog.set_many(changes, event=(project_id, 'project_relation_confirmed' if linked else 'project_relation_removed',
                                         {'projects': [first['id'], second['id']], 'kind': 'rerecording'}))
    return {'project_id': project_id, 'related': related(catalog, project_id)}


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
             'finals': normalized, 'updated_at': now(), 'related': settings(catalog, project_id).get('related', [])}
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
    linked = [{'id': pid, 'title': project_record(catalog, pid)['title'],
               'finals': [{'volume_id': f['volume_id'], 'path': f['path'], 'name': Path(f['path']).name, 'role': f['role'],
                           'published': f.get('published', False)} for f in settings(catalog, pid)['finals']]}
              for pid in related(catalog, project_id)]
    return {'files': values[:1000], 'truncated': len(values) > 1000, 'settings': saved,
            'deliveries': delivery_states(catalog, backend, project_id, saved), 'group': linked}


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


def verification_work(catalog, plan, quick=False):
    """[(final, needs_hash, bytes)] for applying. Every backup copy is hashed when nüün releases something; otherwise
    only copies the review did not stamp, copies made by this apply and copies whose stamp changed since the review."""
    archive = catalog.volume(plan['archive_id'])
    work = []
    with Tree(archive['root']) as tree:
        for final in plan['closure_finals']:
            try:
                current = content_stamp(tree.stat(final['archive_path']))
            except FileNotFoundError:
                require(final.get('copy_required'), 'checksum_mismatch', 'Falta en el respaldo: ' + final['archive_path'])
                work.append((final, True, final['original_stamp']['size']))  # Copied by this apply, hashed after.
                continue
            unchanged = quick and final.get('archive_stamp') and current == final['archive_stamp']
            work.append((final, not unchanged, 0 if unchanged else current['size']))
    return work


def validate(catalog, backend, plan, cancel=None, quick=False, progress=None):
    """Frozen human decisions and every final are gates, including already-moved deliveries.

    `quick` (nothing of nüün's is released): a backup copy whose stamp still matches what the review hashed is not
    read again; any change, or a copy made by this apply, is hashed in full."""
    require(group_digest(catalog, plan['closure_project_id']) == plan['closure_settings_digest'],
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
    work = verification_work(catalog, plan, quick)
    hashed = sum(1 for _, needs, _ in work if needs)
    index = 0
    with Tree(archive['root']) as tree:
        for final, needs, _ in work:
            if not needs:
                guards.append(dict(final, stamp=final['archive_stamp']))
                continue
            sha, stamp = checked_hash_progress(tree, final['archive_path'], cancel, progress, 'verifying_finals', index, hashed)
            index += 1
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


def delivery_states(catalog, backend, project_id, config=None):
    """Where each saved delivery stands now; read-only, one stat per final on an online editing disk.

    Projects and the reopened closeout sheet read this same list, so the row and the summary always agree.
    - `archived`: the selection already points at the backup.
    - `awaiting`: kept on the editing disk and not published yet («por publicar»).
    - `published`: published on the editing disk, waiting for «Respaldar publicados…».
    - `moved`: the same file (name, size, mtime and inode) now sits in the profile's published folder; reported with
      `current_path` so the sheet can offer it. The saved selection is never rewritten here.
    - `published_archived`: gone from the editing disk after a verified transfer out of the published folder.
    - `missing`: gone for an unknown reason; not counted as waiting, and nothing infers why.
    A disk that is offline keeps the saved decision and reports `offline`.
    """
    config = settings(catalog, project_id) if config is None else config
    profile = project_record(catalog, project_id)['profile']
    volumes = {v['id']: v for v in catalog.volumes()}
    online, receipts, states = {}, None, []
    for final in config.get('finals', []):
        volume = volumes.get(final['volume_id']) or {}
        published = bool(final.get('published', config.get('published', False)))
        item = {'volume_id': final['volume_id'], 'path': final['path'], 'role': final['role'],
                'name': Path(final['path']).name, 'published': published}
        if volume.get('role') != 'work':
            states.append(dict(item, state='archived'))
            continue
        if volume['id'] not in online:
            online[volume['id']] = bool(backend is not None and backend.online(volume))
        state = 'published' if published else 'awaiting'
        if not online[volume['id']]:
            states.append(dict(item, state=state, offline=True))
            continue
        root = Path(volume['root'])
        try:
            os.stat(root / final['path'], follow_symlinks=False)
        except FileNotFoundError:
            saved = final.get('stamp') or {}
            profile_root = root / volume.get('finals_prefix', '002_finales') / profile
            folder = published_folder(catalog, profile, subfolders(profile_root)) if profile_root.is_dir() else ''
            moved = ''
            if folder and not published:
                for path, info in delivery_files(profile_root / folder):
                    if path.name == item['name'] and (info.st_size, info.st_mtime_ns, info.st_ino) == (
                            saved.get('size'), saved.get('mtime_ns'), saved.get('inode')):
                        moved = str(path.relative_to(root))
                        break
            if moved:
                state = 'moved'
                item['current_path'] = moved
            else:
                if receipts is None:
                    receipts = published_receipts(catalog)
                # A receipt keeps the destination stamp: name and size identify the delivery, the closeout rehashes later.
                matched = any(Path(dest).name == item['name'] and (stamp or {}).get('size') == saved.get('size')
                              for (_, dest), stamp in receipts.items())
                state = 'published_archived' if matched else 'missing'
        states.append(dict(item, state=state))
    return states


def delivery_summary(states):
    """The counts a project row shows: what still waits on the editing disk, never a backup claim."""
    awaiting = [s for s in states if s['state'] == 'awaiting']
    return {'awaiting': len(awaiting), 'awaiting_names': [s['name'] for s in awaiting][:20],
            'published': sum(s['state'] in {'published', 'moved'} for s in states),
            'missing': sum(s['state'] == 'missing' for s in states),
            'offline': any(s.get('offline') for s in states)}


def archived_final_settings(catalog, plan, guards, project_id=None):
    """Build the association update for the release's atomic terminal commit, for one project of the plan's group.

    Use the stamps just checked under the release guard; no source or editor JSON
    is rewritten, and a later changed final will still require a fresh review.
    """
    config = settings(catalog, project_id or plan['closure_project_id'])
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
    config_digest = group_digest(catalog, project_id)
    # Linked projects are one work: one list of deliveries (each keeps its owner) and every project's sources/scopes.
    members = [project] + [project_record(catalog, pid) for pid in related(catalog, project_id)]
    owned = []
    for member in members:
        member_config = config if member['id'] == project_id else settings(catalog, member['id'])
        owned += [dict(f, owner=member['id'], published=f.get('published', member_config['published'])) for f in member_config['finals']]
    config = dict(config, finals=owned)
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
    scoped = {'raw': [], 'editor': [], 'owner': {}}
    if release_work:
        for member in members:
            found = roots(catalog, backend, member).get(source['id'], {'raw': [], 'editor': []})
            for kind in ('raw', 'editor'):
                for path in found[kind]:
                    if path not in scoped[kind]:
                        scoped[kind].append(path)
                        scoped['owner'][path] = member['id']
    if progress:
        total, count = review_estimate(catalog, members, config, source, target, scoped, release_work, cancel)
        progress = ReviewMeter(progress, total, count, source['name'], target['name'])
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
    member_ids = {m['id'] for m in members}
    assets = [a for a in catalog.search() if a.get('project_id') in member_ids and a['kind'] != 'delivery'] if release_work else []
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
    reviewed_stamps = {}
    if raw_to_verify:
        with Tree(target['root']) as tree:
            for index, (path, expected) in enumerate(raw_to_verify):
                sha, verified = checked_hash_progress(tree, path, cancel, progress, 'review_raw_archive', index, len(raw_to_verify))
                require(sha == expected, 'checksum_mismatch', 'Respaldo raw alterado')
                reviewed_stamps[path] = verified
    # The stamp each backup copy had when this review hashed it: applying watches it instead of hashing again
    # when nothing of nüün's is released (the same stamp guard Transfer uses for verified destinations).
    for final in finals:
        if final['archive_path'] in reviewed_stamps:
            final['archive_stamp'] = reviewed_stamps[final['archive_path']]
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
        archive_stamp = None
        if volume['role'] == 'archive':
            require(volume['id'] == target['id'], 'invalid_volume', 'El final está en otro respaldo')
            archive_path = item['path']
            archive_stamp = stamp
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
                    candidate_sha, candidate_stamp = checked_hash_progress(tree, candidate_rel, cancel, progress, 'review_final_archive')
                    if candidate_sha == sha:
                        existing, archive_stamp = candidate_rel, candidate_stamp
                        break
            if existing:
                archive_path = existing
            else:
                parent = str((Path(source['root']) / item['path']).parent)
                role = item['role']
                if role == 'auxiliary' and any('thumbnail' in p.lower() for p in Path(item['path']).parts):
                    role = 'thumbnail'
                owner = next((m for m in members if m['id'] == item.get('owner')), project)
                transfer = build_plan(catalog, backend, source=parent, archive=target['id'], project=owner['title'], profile=project['profile'],
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
        finals.append({'archive_path': archive_path, 'sha256': sha, 'role': item['role'], 'owner': item.get('owner', project_id),
                       'original': {'volume_id': item['volume_id'], 'path': item['path']},
                       'original_stamp': stamp, 'copy_required': copy_required, 'keep_work': keep_work,
                       'archive_stamp': archive_stamp})
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
    if progress:
        progress.finish()
    # Inside clipper/ only Clipper's own reviewed list is offered, and only Clipper deletes it.
    report(progress, 'review_clipper_regenerable')
    regenerable, notes = clipper_regenerables(catalog, source, scoped['editor'], cancel) if release_work else ([], [])
    transcripts = None
    if release_work:
        # Clipper's current transcripts go to MEDIA next to their backed-up video before anything is released
        # (contracts/clipper-transcripts-v1.md). Clipper offline or a source without backup is a note, not a block.
        from .transcripts import clipper_roots, plan as transcript_plan
        report(progress, 'review_transcripts')
        transcripts = transcript_plan(catalog, backend, target['id'], clipper_roots(source['root'], scoped['editor']), cancel)
    retained.extend(notes)
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
             'closure_finish': release_work, 'closure_inventory_checked': True, 'closure_group': [m['id'] for m in members],
             'closure_group_titles': {m['id']: m['title'] for m in members},
             'clipper_regenerable': regenerable, 'clipper_files': sum(g['files'] for g in regenerable),
             'clipper_bytes': sum(g['bytes'] for g in regenerable), 'clipper_cleaned': {}, 'transcripts': transcripts,
             # Already closed and this review finds nothing to copy or retire: applying would only rewrite the record.
             'already_complete': all(catalog.get('project-released:' + m['id']) for m in members) and not entries and not copies
                                 and not regenerable and not (transcripts and transcripts['files'])}
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
        # A request to Clipper that failed or was interrupted after the release is asked again with the same digest.
        plan = clipper_cleanup(catalog, plan)
        return dict(plan, closure_settings=settings(catalog, plan['closure_project_id']))
    require(confirm_discard is True, 'confirmation_required', 'Confirma el plan revisado antes de completar')
    # Reviews from before 2026-11-07 let nüün delete Clipper's Stalker audio itself; that exception is gone.
    require(not any(e.get('origin') == 'clipper_cache' for e in plan['entries']),
            'plan_changed', 'Este plan es anterior a la lista de regenerables de Clipper; revisa un plan nuevo')
    with clipper.release_guard(catalog) if plan['entries'] else nullcontext():
        with catalog.writer():
            catalog.set('closure-consent:' + plan_id, {'digest': reviewed_digest, 'confirmed_at': now()})
        check_source(backend, plan)
        require(group_digest(catalog, plan['closure_project_id']) == plan['closure_settings_digest'], 'plan_changed', 'Cambió el cierre; revisa de nuevo')
        with Tree(plan['source_root']) as tree:
            validate_remaining(tree, plan, cancel)
        # One backup check per apply. When nüün releases nothing, the review's fresh hashes are watched by stamp.
        quick = not plan['entries']
        tracker = ActionProgress(progress, plan, catalog,
                                 verify_total=sum(size for _, needs, size in verification_work(catalog, plan, quick) if needs))
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
        guards = validate(catalog, backend, plan, cancel, quick=quick, progress=tracker.verify_event)
        if relinks or catalog.get('release-relink-intent:' + plan_id):
            relink_release(catalog, backend, plan_id, reviewed_digest, cancel, progress=tracker.passthrough)
        copied_transcripts = None
        if plan.get('transcripts') and plan['transcripts']['files']:
            # Verified in MEDIA before the first file is released; a failure stops the closeout here.
            from .transcripts import copy as copy_transcripts
            report('copying_transcripts')
            copied_transcripts = copy_transcripts(catalog, backend, plan['transcripts'], receipt_id=plan_id, cancel=cancel)
        result = apply_release(catalog, backend, plan_id, reviewed_digest, cancel, progress=tracker.release_event,
                               archive_guards=guards)
        if copied_transcripts:
            with catalog.writer():
                stored = catalog.get('release:' + plan_id)
                stored['transcripts_result'] = copied_transcripts
                catalog.set('release:' + plan_id, stored)
            result = dict(result, transcripts_result=copied_transcripts)
        if result.get('clipper_regenerable'):
            report('clipper_cleanup')
            result = clipper_cleanup(catalog, result)
        report('release_complete', files_done=len(plan['entries']), files_total=len(plan['entries']))
        return dict(result, closure_settings=settings(catalog, plan['closure_project_id']))


def clipper_regenerables(catalog, source, editors, cancel=None):
    """What Clipper says it can rebuild in each editor's Clipper folder (`library cleanup-plan`, 2026-11-07).

    Clipper is the only source of what is regenerable inside `clipper/`. The review shows the categories Clipper
    preselects (for a retired project, `project_cache` with `cache/stalker/*.wav` among them) and, with the
    closeout's consent, Clipper deletes them with the reviewed digest. No answer, an incomplete inventory or an
    unexpected document offers nothing. Returns the groups to offer and notes for what is kept."""
    groups, notes = [], []
    for editor in editors:
        for rel in (editor + '/clipper', editor):
            folder = Path(source['root']) / rel
            if (folder / 'clipper-project.json').is_file() or (folder / 'clipper-retired-project.json').is_file():
                break
        else:
            continue
        require(not (cancel and cancel()), 'cancelled', 'Operación cancelada')
        try:
            document = clipper.cleanup_plan(catalog, folder)
        except (MediaError, RuntimeError, OSError, ValueError, KeyError, TypeError) as error:
            notes.append(rel + ': Clipper no dio su lista de regenerables; se conserva todo (' + str(error) + ')')
            continue
        if not document['inventory_complete']:
            notes.append(rel + ': el inventario de Clipper está incompleto; se conserva todo')
            continue
        chosen = [c for c in document['categories'] if c['default_selected']]
        for category in document['categories']:
            if not category['default_selected']:
                notes.append('%s: %s (%d archivos) se conserva; Clipper no lo preselecciona'
                             % (rel, category.get('label') or category['id'], category['files']))
        if not chosen:
            continue
        ids = {c['id'] for c in chosen}
        groups.append({'editor': rel, 'project_root': document['project_root'], 'project_state': document['project_state'],
                       'digest': document['digest'], 'categories': chosen,
                       'entries': [e for e in document['entries'] if e['category'] in ids],
                       'files': sum(c['files'] for c in chosen), 'bytes': sum(c['bytes'] for c in chosen),
                       'document': document})
    return groups, notes


def clipper_cleanup(catalog, plan):
    """After the release, ask Clipper to delete what the review showed. nüün never deletes inside `clipper/` itself.

    Clipper revalidates category, owner and stamp and skips what changed. A failure is recorded, not raised: the
    closeout already completed and those files simply stay, to be asked again or cleaned from Clipper's Storage."""
    done = dict(plan.get('clipper_cleaned') or {})
    pending = [(i, g) for i, g in enumerate(plan.get('clipper_regenerable') or [])
               if g['project_root'] not in done or done[g['project_root']].get('error')]
    if not pending:
        return plan
    for index, group in pending:
        name = 'clipper-cleanup/%s-%d.json' % (plan['id'], index)
        try:
            with Tree(catalog.home) as tree:
                tree.write_json(name, group['document'])
            ack = clipper.cleanup(catalog, Path(catalog.home) / name, group['digest'], [c['id'] for c in group['categories']])
            done[group['project_root']] = {'deleted_files': ack['deleted_files'], 'deleted_bytes': ack['deleted_bytes'],
                                           'skipped': ack['skipped'][:200], 'skipped_count': len(ack['skipped']), 'at': now()}
        except (MediaError, RuntimeError, OSError, ValueError) as error:
            done[group['project_root']] = {'error': str(error), 'deleted_files': 0, 'deleted_bytes': 0, 'skipped': [], 'at': now()}
    with catalog.writer():
        stored = catalog.get('release:' + plan['id'])
        stored['clipper_cleaned'] = done
        catalog.set('release:' + plan['id'], stored)
    return stored


def review_estimate(catalog, members, config, source, target, scoped, release_work, cancel=None):
    """Bytes and files a review will read, from sizes only (no hashing), so its bar is one measure from the start.

    Raw copies on the editing disk are read there and matched in the backup (twice), catalogued sources once in the
    backup, intermediates once, every chosen final once and an editing final once more when the backup has a file of
    the same name and size to compare. It is an estimate: overlaps make it a little high, never the bar go back.
    """
    from .release import selected_files
    total = count = 0
    with Tree(source['root']) as tree:
        for path in scoped['raw']:
            try:
                selected, _ = selected_files(tree, path, cancel=cancel)
            except FileNotFoundError:
                continue
            for item in selected:
                total += 2 * tree.stat(item).st_size
                count += 2
        for editor in scoped['editor']:
            if (Path(source['root']) / editor / 'renders').is_dir():
                for item in folder_files(tree, editor + '/renders', cancel):
                    total += tree.stat(item).st_size
                    count += 1
    project = members[0]
    member_ids = {m['id'] for m in members}
    if release_work:
        for asset in (a for a in catalog.search() if a.get('project_id') in member_ids and a['kind'] != 'delivery'):
            for original in asset['paths']:
                row = catalog.db.execute('SELECT payload FROM copies WHERE job_id=? AND volume_id=? AND relative_path=?',
                                         (asset['job_id'], target['id'], original)).fetchone()
                if row:
                    total += json.loads(row[0])['bytes']
                    count += 1
    archived = {}
    archive_root = Path(target['root']) / target.get('finals_prefix', '002_finales') / project['profile']
    if any(catalog.volume(f['volume_id'])['role'] == 'work' for f in config['finals']) and archive_root.is_dir():
        archived = {(p.name, info.st_size) for p, info in delivery_files(archive_root, cancel)}
    for final in config['finals']:
        size = final['stamp']['size']
        total += size
        count += 1
        if catalog.volume(final['volume_id'])['role'] == 'work' and (Path(final['path']).name, size) in archived:
            total += size
            count += 1
    return total, count


class ReviewMeter:
    """Turns a review's per-step reports into one overall measure plus the file in hand (`file_done`/`file_total`).

    Each hashed file adds what it read; the bar stops just short of the end until the review finishes, so an estimate
    that ran low never makes it go back."""

    def __init__(self, progress, total, count, work_name, archive_name):
        from .progress import ActionMeter
        self.progress, self.meter = progress, ActionMeter(max(1, total), count)
        self.names = {'review_raw_archive': archive_name, 'review_final_archive': archive_name, 'matching_archive': archive_name}
        self.work_name = work_name
        self.base, self.key, self.read = 0, None, 0

    def __call__(self, stage, **details):
        # Only a file being read counts: release also sends a stage total and a start marker at 0 bytes.
        reading = bool(details.get('path')) and details.get('bytes_done', 0) > 0
        if reading:
            key = (stage, details.get('path'))
            if key != self.key or details['bytes_done'] < self.read:
                self.base += self.read
                if self.key is not None:
                    self.meter.files_done = min(self.meter.files_total, self.meter.files_done + 1)
                self.key, self.read = key, 0
            self.read = details['bytes_done']
            self.meter.reach(min(self.base + self.read, int(self.meter.total * 0.99)))
            self.meter.holding(details.get('path', ''), self.names.get(stage, self.work_name), '', 'verify')
        file = {'file_done': details['bytes_done'], 'file_total': details.get('bytes_total', 0)} if reading else {}
        self.progress(stage, **dict(details, **file, overall_done=self.meter.done, overall_total=self.meter.total,
                                   overall_files_done=self.meter.files_done, overall_files_total=self.meter.files_total,
                                   current=dict(self.meter.current)))

    def finish(self):
        self.meter.reach(self.meter.total)
        self.meter.files_done = self.meter.files_total
        self.meter.current = {}


class ActionProgress:
    """One overall measure for a closeout: copy + verify each final, then hash and retire each editing copy.

    Observers get `overall_done`/`overall_total` in bytes, the current file with its source and destination, and a
    short rolling log of finished steps. It only reports; it never decides what is copied or removed.
    """
    LOG_LIMIT = 60

    def __init__(self, progress, plan, catalog, verify_total=0):
        self.progress = progress
        self.copy_total = sum(f['stamp']['size'] for t in plan['closure_copies'] for f in t['files'])
        self.release_sizes = [e['stamp']['size'] for e in plan['entries']]
        # Copy + verify each new copy, then check the backup (one pass), then hash and retire each editing copy.
        self.verify_total, self.verify_base, self.verify_key, self.verify_read = verify_total, 0, None, 0
        self.total = 2 * self.copy_total + verify_total + sum(self.release_sizes)
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

    def verify_event(self, stage, **details):
        """Hashing a backup copy before release: the file in hand and its share of the one measure."""
        self.stage_name = 'verifying_finals'
        path = details.get('path')
        if path and details.get('bytes_done', 0) > 0:
            if path != self.verify_key:
                self.verify_base += self.verify_read
                self.verify_key, self.verify_read = path, 0
            self.verify_read = details['bytes_done']
            self.current = {'name': Path(path).name, 'from': self.archive_name, 'to': '', 'action': 'verify'}
            self.done = max(self.done, 2 * self.copy_total + min(self.verify_total, self.verify_base + self.verify_read))
            self.emit(file_done=details['bytes_done'], file_total=details.get('bytes_total', 0))
            return
        self.emit()

    def release_event(self, stage, **details):
        self.stage_name = stage
        if stage == 'releasing_originals' and details.get('path'):
            index = details.get('files_done', 0)
            path = details['path']
            self.current = {'name': Path(path).name, 'from': self.work_name, 'to': '', 'action': 'release'}
            finished = 'bytes_done' not in details
            reading = 0 if finished else details.get('bytes_done', 0)
            completed = index if finished else index
            self.done = 2 * self.copy_total + self.verify_total + sum(self.release_sizes[:completed]) + reading
            if finished and path not in self.retired:
                self.retired.add(path)
                self.files_done += 1
                self.note('released', path, self.work_name)
        self.emit()
