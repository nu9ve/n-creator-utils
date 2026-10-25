"""Machine-readable operational CLI; media mutations require an explicit command."""
import argparse
import json
import signal
import sys

from .catalog import Catalog
from .common import MediaError, read_json, require
from .service import Service


def parser():
    root = argparse.ArgumentParser(description='Verified media ingest, catalog and deferred work')
    root.add_argument('--state-dir', help='Local catalog directory (never a media destination)')
    commands = root.add_subparsers(dest='command', required=True)
    commands.add_parser('mcp', help='Read-only local MCP stdio server')
    commands.add_parser('rpc', help='One JSON request on stdin; one JSON envelope on stdout')
    volumes = commands.add_parser('volumes')
    sub = volumes.add_subparsers(dest='action', required=True)
    sub.add_parser('list')
    register = sub.add_parser('register')
    register.add_argument('name'); register.add_argument('root')
    register.add_argument('--role', choices=['archive', 'work'], required=True)
    register.add_argument('--prefix')
    register.add_argument('--allow-local', action='store_true', help='Explicitly register a local directory; external required by default')
    plan = commands.add_parser('plan')
    plan.add_argument('source'); plan.add_argument('--archive', required=True); plan.add_argument('--work')
    plan.add_argument('--project', required=True); plan.add_argument('--profile', default='e64')
    plan.add_argument('--country', default='mx'); plan.add_argument('--city', default='hermosillo')
    plan.add_argument('--archive-only', action='store_true'); plan.add_argument('--confirm-archive-only', action='store_true')
    plan.add_argument('--work-decision', choices=['pending', 'skipped'], default='pending')
    plan.add_argument('--cleanup', action='store_true', help='Clean card after all required copies verify; Sony requires camera action')
    plan.add_argument('--source-kind', choices=['card', 'stalker', 'delivery'], default='card')
    plan.add_argument('--camera', choices=['sony', 'canon', 'nikon', 'gopro', 'h6', 'dji-drone', 'dji-360', 'dji-action', 'insta360', 'generic'])
    plan.add_argument('--assignments', help='JSON mapping file paths to project/profile/camera overrides')
    apply = commands.add_parser('apply'); apply.add_argument('plan'); apply.add_argument('--digest', required=True)
    resume = commands.add_parser('resume'); resume.add_argument('job_id')
    recover = commands.add_parser('recover-catalog'); recover.add_argument('volume')
    rename = commands.add_parser('rename-project'); rename.add_argument('project_id'); rename.add_argument('title')
    publication = commands.add_parser('publication'); publication.add_argument('asset_id'); publication.add_argument('variant'); publication.add_argument('--channel', required=True); publication.add_argument('--state', choices=['candidate','approved','published'], required=True); publication.add_argument('--url')
    commands.add_parser('publications')
    commands.add_parser('jobs')
    job = commands.add_parser('job'); job.add_argument('job_id')
    library = commands.add_parser('library'); library.add_argument('--project'); library.add_argument('--work-state', choices=['pending', 'ready', 'skipped'])
    resolve = commands.add_parser('resolve'); resolve.add_argument('asset_id')
    deliver = commands.add_parser('deliver'); deliver.add_argument('asset_ids', nargs='+'); deliver.add_argument('--work', required=True)
    state = commands.add_parser('work-state'); state.add_argument('asset_ids', nargs='+'); state.add_argument('--state', choices=['pending', 'skipped'], required=True)
    restore = commands.add_parser('restore'); restore.add_argument('asset_id'); restore.add_argument('destination')
    handoff = commands.add_parser('handoff'); handoff.add_argument('asset_ids', nargs='+')
    return root


def main(argv=None):
    args = parser().parse_args(argv)
    cancelled = [False]
    def interrupt(signum, frame):
        cancelled[0] = True
    previous = {sig: signal.signal(sig, interrupt) for sig in (() if args.command == 'mcp' else (signal.SIGINT, signal.SIGTERM))}
    catalog = None
    try:
        catalog = Catalog(args.state_dir)
        service = Service(catalog, cancel=lambda: cancelled[0])
        if args.command == 'mcp':
            from .mcp import serve
            return serve(service)
        if args.command == 'rpc':
            raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
            require(len(raw) <= 16 * 1024 * 1024, 'request_too_large', 'Request exceeds 16 MiB')
            request = json.loads(raw)
            require(isinstance(request, dict) and isinstance(request.get('method'), str), 'invalid_request', 'Expected method/params JSON')
            method, params = request['method'], request.get('params', {})
        else:
            params = vars(args).copy()
            method = params.pop('command'); params.pop('state_dir')
            if method == 'volumes':
                method = 'register_volume' if params.pop('action') == 'register' else 'volumes'
                if method == 'volumes': params = {}
            elif method == 'plan' and params['assignments']:
                params['assignments'] = read_json(params['assignments'])
            elif method == 'apply':
                value = read_json(params['plan'])
                params['plan'] = value['data'] if isinstance(value, dict) and value.get('ok') is True else value
            method = {'deliver': 'deliver_work', 'work-state': 'set_work_state', 'recover-catalog': 'recover_catalog', 'rename-project': 'rename_project', 'publication': 'set_publication'}.get(method, method)
        data = service.call(method, params)
        print(json.dumps({'ok': True, 'command': method, 'data': data}, ensure_ascii=False, indent=2))
        return 0
    except (MediaError, OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        code = error.code if isinstance(error, MediaError) else 'operation_failed'
        print(json.dumps({'ok': False, 'error': {'code': code, 'message': str(error)}}, ensure_ascii=False))
        return 1
    finally:
        if catalog:
            catalog.close()
        for sig, handler in previous.items():
            signal.signal(sig, handler)
