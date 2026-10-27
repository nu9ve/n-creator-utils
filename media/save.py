"""Human-facing safe save flow, including a positional project shorthand."""
import argparse
import json
from pathlib import Path
import sys

from .catalog import Catalog
from .common import MediaError, read_json, require
from .planner import build_plan
from .transfer import Transfer
from .volumes import VolumeBackend


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description='Planear, verificar y liberar SD. Atajo: save FUENTE PAIS PROYECTO; motor histórico: legacy-save.')
    parser.add_argument('source', help='raíz de SD, carpeta de capturas o all')
    parser.add_argument('tags', nargs='*', metavar='PAIS PROYECTO', help='atajo de --country y --project (no ciudad)')
    parser.add_argument('--state-dir')
    parser.add_argument('--archive'); parser.add_argument('--work'); parser.add_argument('--project')
    parser.add_argument('--profile'); parser.add_argument('--country'); parser.add_argument('--city')
    parser.add_argument('--assignments'); parser.add_argument('--camera')
    parser.add_argument('--source-kind', choices=['card', 'stalker', 'delivery'], default='card')
    parser.add_argument('--keep-source', action='store_true')
    parser.add_argument('--archive-only', action='store_true'); parser.add_argument('--confirm-archive-only', action='store_true')
    parser.add_argument('--no-work', action='store_true', help='sólo archivo, sin copia de trabajo ni pendiente; requiere confirmación archive-only')
    parser.add_argument('--work-decision', choices=['pending', 'skipped'])
    parser.add_argument('--dry-run', action='store_true'); parser.add_argument('--yes', action='store_true', help='Aplicar plan estándar sin prompt; archive-only requiere confirmación propia')
    args = parser.parse_args(argv)
    if args.tags:
        if len(args.tags) != 2 or args.country is not None or args.project is not None:
            parser.error('Usa FUENTE PAIS PROYECTO o FUENTE --country PAIS --project PROYECTO, sin mezclarlos. Ciudad: --city.')
        args.country, args.project = args.tags
    if args.no_work:
        if args.work or args.work_decision == 'pending':
            parser.error('--no-work no admite --work ni --work-decision pending')
        args.archive_only = True
        args.work_decision = 'skipped'
    args.work_decision = args.work_decision or 'pending'
    return args


def main(argv=None):
    args = parse_args(argv)
    catalog = Catalog(args.state_dir); backend = VolumeBackend()
    try:
        defaults = catalog.get('defaults', {})
        archive = args.archive or defaults.get('archive')
        work = args.work or defaults.get('work')
        if not archive:
            archives = [v for v in catalog.volumes() if v['role'] == 'archive']
            archive = archives[0]['id'] if len(archives) == 1 else None
        require(archive, 'archive_required', 'Register/select an archive volume with nu9ve media volumes register')
        if not work:
            choices = [v for v in catalog.volumes() if v['role'] == 'work']
            work = choices[0]['id'] if len(choices) == 1 else None
        project = args.project or defaults.get('project', '')
        if not project and sys.stdin.isatty() and not args.dry_run:
            project = input('Proyecto: ').strip()
        require(project, 'project_required', 'Choose --project; country/city are separate flags')
        archive_only, confirmed = args.archive_only, args.confirm_archive_only
        work_online = backend.online(catalog.volume(work)) if work and not archive_only else None
        if (archive_only and not confirmed or not archive_only and not work_online) and sys.stdin.isatty() and not args.dry_run:
            print('Sólo se respaldará en el archivo seleccionado; la SD se puede liberar tras verificar el respaldo.')
            print('Trabajo: ' + ('omitido deliberadamente' if args.work_decision == 'skipped' else 'pendiente de entrega al SSD'))
            confirmed = input('Escribe SOLO MEDIA para confirmar esta operación: ').strip() == 'SOLO MEDIA'
            require(confirmed, 'confirmation_required', 'Archive-only was not confirmed')
            archive_only = True
        sources = [Path(args.source).expanduser().absolute()]
        if args.source == 'all':
            from configuration import load_settings
            settings = load_settings()
            registered = {Path(v['root']) for v in catalog.volumes()}
            sources = []
            for path in sorted(Path(settings['sd_root_dir']).iterdir()):
                if path in registered or not path.is_dir() or path.is_symlink(): continue
                info = backend.describe(path)
                if info['external'] and (info['removable'] or path.name in settings['sd_src_name']):
                    sources.append(path)
        require(sources, 'no_cards', 'No eligible card is connected')
        plans = [build_plan(catalog, backend, str(source), archive, None if args.no_work else work, project,
                    args.profile or defaults.get('profile', 'e64'), args.country or defaults.get('country', 'mx'),
                    args.city or defaults.get('city', 'hermosillo'), archive_only, confirmed, args.work_decision,
                    not args.keep_source and args.source_kind == 'card',
                    read_json(args.assignments) if args.assignments else None, args.camera, args.source_kind) for source in sources]
        if args.dry_run:
            print(json.dumps(plans[0] if len(plans) == 1 else plans, indent=2, ensure_ascii=False)); return 0
        for plan in plans:
            size = sum(f['stamp']['size'] for f in plan['files'])
            print('{}: {} archivos, {:.2f} GB, {}. {}'.format(plan['source'], len(plan['files']), size / 1e9,
                'sólo archivo confirmado' if plan['work_id'] is None else 'archivo + trabajo',
                'Liberación en cámara pendiente' if plan['cleanup_profile'] == 'camera_action_required' else
                'Borrar clips tras verificar; luego formatear en cámara' if plan['cleanup_profile'] == 'sony_clips_then_format' and plan['cleanup'] else
                ('Limpiar SD tras verificar' if plan['cleanup'] else 'Conservar origen')))
            print('Trabajo: {}'.format(plan['work_delivery_status']))
            for unit in plan['preview']:
                print(json.dumps(unit, ensure_ascii=False))
        if not args.yes:
            require(sys.stdin.isatty(), 'confirmation_required', 'Review with --dry-run, then use apply with digest or save --yes')
            require(input('Escribe GUARDAR para ejecutar estos planes: ').strip() == 'GUARDAR', 'cancelled', 'No files changed')
        for plan in plans:
            result = Transfer(catalog, backend).apply(plan, plan['digest'])
            print(json.dumps({'job_id': plan['id'], 'state': result['state']}, ensure_ascii=False))
        catalog.set('defaults', {'project': project, 'profile': plans[-1]['units'][0]['profile'], 'country': plans[-1]['country'],
                                'city': plans[-1]['city'], 'archive': archive, 'work': work or ''})
        return 0
    finally:
        catalog.close()
