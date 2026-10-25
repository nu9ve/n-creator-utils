"""Read-only MCP stdio server. No HTTP listener and no destructive tools."""
import json
import sys

from .common import MediaError

TOOLS = {
    'media_library_search': ('library', {'project': {'type': 'string'}, 'work_state': {'enum': ['pending', 'ready', 'skipped']}}, []),
    'media_asset_resolve': ('resolve', {'asset_id': {'type': 'string'}}, ['asset_id']),
    'media_jobs': ('jobs', {}, []),
    'media_job_status': ('job', {'job_id': {'type': 'string'}}, ['job_id']),
    'media_volumes': ('volumes', {}, []),
}


def handle(service, message, initialized):
    if not isinstance(message, dict) or message.get('jsonrpc') != '2.0' or not isinstance(message.get('method'), str):
        return {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32600, 'message': 'Invalid request'}}
    method, request_id = message['method'], message.get('id')
    if 'id' not in message: return None
    def result(value): return {'jsonrpc': '2.0', 'id': request_id, 'result': value}
    if method == 'initialize':
        version = message.get('params', {}).get('protocolVersion')
        supported = {'2025-11-25', '2025-06-18', '2025-03-26', '2024-11-05'}
        return result({'protocolVersion': version if version in supported else '2025-11-25', 'capabilities': {'tools': {}},
                       'serverInfo': {'name': 'ncreator-media', 'version': '0.2.0'},
                       'instructions': 'Read-only local catalog. Import/cleanup requires the reviewed CLI/native plan flow.'})
    if not initialized:
        return {'jsonrpc': '2.0', 'id': request_id, 'error': {'code': -32000, 'message': 'Initialize first'}}
    if method == 'ping': return result({})
    if method == 'tools/list':
        return result({'tools': [{'name': name, 'description': 'Read local media ' + target,
                        'inputSchema': {'type': 'object', 'properties': properties, 'required': required, 'additionalProperties': False},
                        'annotations': {'readOnlyHint': True, 'destructiveHint': False, 'openWorldHint': False}}
                       for name, (target, properties, required) in TOOLS.items()]})
    if method == 'tools/call':
        params = message.get('params') or {}
        if params.get('name') not in TOOLS:
            return {'jsonrpc': '2.0', 'id': request_id, 'error': {'code': -32602, 'message': 'Unknown tool'}}
        target, properties, required = TOOLS[params['name']]
        args = params.get('arguments', {})
        if not isinstance(args, dict) or set(args) - set(properties) or set(required) - set(args) or any(not isinstance(v, str) for v in args.values()):
            return {'jsonrpc': '2.0', 'id': request_id, 'error': {'code': -32602, 'message': 'Invalid tool arguments'}}
        try:
            data = service.call(target, args)
            # Keep a structured object, even when the domain response is an array.
            structured = {'data': data}
            return result({'content': [{'type': 'text', 'text': json.dumps(structured, ensure_ascii=False)}], 'structuredContent': structured, 'isError': False})
        except (MediaError, OSError, ValueError, KeyError, TypeError) as error:
            return result({'content': [{'type': 'text', 'text': str(error)}], 'isError': True})
    return {'jsonrpc': '2.0', 'id': request_id, 'error': {'code': -32601, 'message': 'Method not found'}}


def serve(service, reader=None, writer=None):
    reader, writer = reader or sys.stdin.buffer, writer or sys.stdout
    initialized = False
    while True:
        line = reader.readline(16 * 1024 * 1024 + 1)
        if not line: return 0
        if len(line) > 16 * 1024 * 1024: return 1
        try:
            message = json.loads(line)
            response = handle(service, message, initialized)
            if isinstance(message, dict) and message.get('method') == 'initialize': initialized = True
        except (ValueError, TypeError, AttributeError):
            response = {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32700, 'message': 'Parse error'}}
        if response is not None:
            writer.write(json.dumps(response, ensure_ascii=False, separators=(',', ':')) + '\n')
            writer.flush()
