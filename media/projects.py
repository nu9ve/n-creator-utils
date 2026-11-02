"""What the app shows about a project: stage, links and quick-pick order. Files on disk never change here."""
from .common import now, require, slug

STATES = {'active', 'paused', 'finished'}
# Backup is not settled while one of its jobs can still resume.
OPEN_JOBS = {'failed', 'paused', 'copying', 'cleaning_source', 'backup_verified'}


def key_for(catalog, title):
    """A named project without receipts yet is a draft: capture tags are its only record."""
    row = catalog.db.execute('SELECT project_id FROM project_aliases WHERE alias=? ORDER BY profile LIMIT 1', (title,)).fetchone()
    return row[0] if row else 'draft:' + title


def touch(catalog, key):
    catalog.set('project-touch:' + key, now())


def touch_project(catalog, key):
    require(isinstance(key, str) and key, 'invalid_request', 'Project key required')
    touch(catalog, key)
    return {'key': key}


def set_state(catalog, project_id, state):
    """Declared by the person, never inferred; it deletes nothing."""
    require(state in STATES, 'invalid_project_state', 'Project state must be active, paused or finished')
    require(catalog.db.execute('SELECT 1 FROM projects WHERE id=?', (project_id,)).fetchone() is not None, 'unknown_project', 'Project not found')
    catalog.set('project-state:' + project_id, {'state': state, 'updated_at': now()})
    return {'project_id': project_id, 'state': state}


def _retag(catalog, old_titles, title):
    rows = catalog.db.execute("SELECT key,value FROM metadata WHERE key LIKE 'capture-tag:%'").fetchall()
    import json
    for row in rows:
        tag = json.loads(row['value'])
        if tag.get('name') in old_titles:
            catalog.set(row['key'], dict(tag, name=title))
    defaults = catalog.get('defaults', {})
    if defaults.get('project') in old_titles:
        catalog.set('defaults', dict(defaults, project=title))


def rename(catalog, key, title):
    """Catalog, tags and defaults follow the new name; old copies keep their folders, the alias still resolves them."""
    title = slug(title)
    if key.startswith('draft:'):
        old = key[len('draft:'):]
        with catalog.writer():
            _retag(catalog, {old}, title)
            catalog.set('project-touch:' + key_for(catalog, title), now())
        return {'key': key_for(catalog, title), 'title': title, 'previous': old}
    row = catalog.db.execute('SELECT title FROM projects WHERE id=?', (key,)).fetchone()
    require(row is not None, 'unknown_project', 'Project not found')
    catalog.rename_project(key, title)
    with catalog.writer():
        _retag(catalog, {row['title']}, title)
    return {'key': key, 'project_id': key, 'title': title, 'previous': row['title']}


def overview(catalog, sessions):
    """Newest first. `sessions` come from capture_libraries so a draft shows up as soon as it is tagged."""
    aliases, names = {}, {}
    for row in catalog.db.execute('SELECT alias,project_id FROM project_aliases ORDER BY profile'):
        aliases.setdefault(row['alias'], row['project_id'])
        names.setdefault(row['project_id'], []).append(row['alias'])
    profiles = {row['id']: (row['title'], row['profile']) for row in catalog.db.execute('SELECT id,title,profile FROM projects')}
    jobs = {row['id']: (row['state'], row['updated_at']) for row in catalog.db.execute('SELECT id,state,updated_at FROM jobs')}
    entries = {}

    def entry(key, title, profile='', project_id=''):
        return entries.setdefault(key, {'key': key, 'project_id': project_id, 'project': title, 'profile': profile,
                                        'assets': 0, 'kinds': {}, 'cameras': set(), 'places': set(), 'jobs': set(),
                                        'pending_sessions': 0, 'work_pending': 0, 'work_ready': 0, 'work_skipped': 0,
                                        'preview': {}, 'seen_at': ''})

    for asset in catalog.search():
        project_id = asset.get('project_id') or ''
        item = entry(project_id or 'draft:' + asset['project'], asset['project'], asset['profile'], project_id)
        item['assets'] += 1
        item['kinds'][asset['kind']] = item['kinds'].get(asset['kind'], 0) + 1
        item['work_' + asset['work_delivery_status']] += 1
        item['jobs'].add(asset['job_id'])
        if asset.get('camera') and asset['camera'] != 'generic':
            item['cameras'].add(asset['camera'])
        place = '/'.join(p for p in (asset.get('country'), asset.get('city')) if p)
        if place:
            item['places'].add(place)
    for session in sessions:
        title = (session.get('tag') or {}).get('name')
        if not title or session.get('status') != 'completed' or session.get('archived'):
            continue
        project_id = aliases.get(title, '')
        known = profiles.get(project_id)
        item = entry(project_id or 'draft:' + title, known[0] if known else title, known[1] if known else '', project_id)
        item['pending_sessions'] += 1
        item['seen_at'] = max(item['seen_at'], session.get('created_at') or '')
        if not item['preview'] and session.get('preview_path'):
            item['preview'] = {'path': session['path'], 'preview_path': session['preview_path']}
    result = []
    for key, item in entries.items():
        linked = item.pop('jobs')
        seen = item.pop('seen_at')
        state = catalog.get('project-state:' + key, {}).get('state', 'active') if item['project_id'] else 'active'
        open_jobs = sum(1 for job in linked if job in jobs and jobs[job][0] in OPEN_JOBS)
        if state == 'finished':
            stage = 'finished'
        elif not item['assets']:
            stage = 'draft'
        elif item['pending_sessions'] or item['work_pending'] or open_jobs:
            stage = 'in_progress'
        else:
            stage = 'backed_up'
        touched = catalog.get('project-touch:' + key, '')
        item.update(state=state, stage=stage, jobs=len(linked), open_jobs=open_jobs, cameras=sorted(item['cameras']), places=sorted(item['places']),
                    aliases=[name for name in names.get(item['project_id'], []) if name != item['project']],
                    updated_at=max([touched, seen] + [jobs[job][1] for job in linked if job in jobs]))
        result.append(item)
    return sorted(result, key=lambda e: (e['updated_at'], e['project']), reverse=True)
