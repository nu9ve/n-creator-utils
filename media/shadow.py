"""Shadow of the Rust engine (docs/platforms/rust-engine.md §7, PLAN «Motor en Rust · ruta»).

Python stays the engine that answers. When a shadow engine is configured, the same *read* is asked of
`nuun-engine rpc-serve` afterwards, on a background thread, and only the comparison is recorded: same answer,
different (with the JSON paths that differ), `not_ported`, or an error, plus both times. Nothing here can change,
delay or fail Python's answer, and nothing is sent anywhere: the log is a local file under the catalog folder.

Configured by `NUUN_SHADOW_ENGINE` (path to the binary) or by `<catalog>/shadow.json` `{"engine": "/path"}`;
`NUUN_SHADOW_ENGINE=off` disables it whatever the file says. Writes are never shadowed: `rpc-serve` only reads,
and the Rust engine refuses mutations there too.
"""
import json
import os
import queue
import subprocess
import threading
import time
from pathlib import Path

LOG_DIR = 'shadow'
MAX_DIFFS = 20
MAX_VALUE = 160
# Rust names what it cannot answer yet inside an answer; outside the comparison, reported apart.
RUST_ONLY_KEYS = ('not_ported',)


def configured_engine(home):
    value = os.environ.get('NUUN_SHADOW_ENGINE')
    if value is not None:
        return None if value.strip().lower() in {'', 'off', '0', 'false'} else value
    try:
        config = json.loads((Path(home) / 'shadow.json').read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return None
    engine = config.get('engine') if isinstance(config, dict) and config.get('enabled', True) else None
    return engine if isinstance(engine, str) and engine else None


def _short(value):
    text = json.dumps(value, ensure_ascii=False, sort_keys=True)
    return text if len(text) <= MAX_VALUE else text[:MAX_VALUE] + '…'


def differences(python, rust, path='$', out=None):
    """JSON paths where the two answers differ, as (path, python, rust), at most MAX_DIFFS + 1."""
    out = [] if out is None else out
    if len(out) > MAX_DIFFS:
        return out
    if isinstance(python, dict) and isinstance(rust, dict):
        for key in sorted(set(python) | set(rust)):
            if key not in rust:
                out.append((path + '.' + key, _short(python[key]), '<missing>'))
            elif key not in python:
                out.append((path + '.' + key, '<missing>', _short(rust[key])))
            else:
                differences(python[key], rust[key], path + '.' + key, out)
    elif isinstance(python, list) and isinstance(rust, list):
        if len(python) != len(rust):
            out.append((path + '.length', len(python), len(rust)))
        for index, (a, b) in enumerate(zip(python, rust)):
            differences(a, b, '%s[%d]' % (path, index), out)
    elif type(python) is not type(rust) and not (isinstance(python, (int, float)) and isinstance(rust, (int, float))
                                                  and not isinstance(python, bool) and not isinstance(rust, bool)):
        out.append((path, _short(python), _short(rust)))
    elif python != rust:
        out.append((path, _short(python), _short(rust)))
    return out


def compare(method, python_envelope, rust_envelope):
    """One log entry (without times) for the same request answered by both engines."""
    entry = {'method': method}
    rust_error = (rust_envelope.get('error') or {}) if not rust_envelope.get('ok') else None
    if rust_error and rust_error.get('code') == 'not_ported':
        entry['outcome'] = 'not_ported'
        return entry
    if not python_envelope.get('ok') or rust_error is not None:
        python_code = None if python_envelope.get('ok') else (python_envelope.get('error') or {}).get('code')
        rust_code = rust_error.get('code') if rust_error is not None else None
        entry['outcome'] = 'same_error' if python_code == rust_code else 'error_mismatch'
        entry['errors'] = {'python': python_code, 'rust': rust_code}
        if rust_error is not None and python_code != rust_code:
            entry['rust_message'] = str(rust_error.get('message', ''))[:MAX_VALUE]
        return entry
    rust_data = rust_envelope.get('data')
    partial = []
    if isinstance(rust_data, dict):
        rust_data = dict(rust_data)
        for key in RUST_ONLY_KEYS:
            if key in rust_data:
                partial.append(key)
                rust_data.pop(key)
    found = differences(python_envelope.get('data'), rust_data)
    entry['outcome'] = 'same' if not found else 'diff'
    if partial:
        entry['rust_partial'] = partial
    if found:
        entry['diffs'] = [{'path': p, 'python': a, 'rust': b} for p, a, b in found[:MAX_DIFFS]]
        entry['more_diffs'] = len(found) > MAX_DIFFS
    return entry


class RustReader:
    """A persistent `nuun-engine rpc-serve` on the same catalog, asked one request at a time."""

    def __init__(self, binary, home):
        self.process = subprocess.Popen([str(binary), '--state-dir', str(home), 'rpc-serve'],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        text=True, encoding='utf-8', bufsize=1)
        self.sequence = 0

    def ask(self, method, params):
        self.sequence += 1
        request = {'id': 'shadow-%d' % self.sequence, 'method': method, 'params': params or {}}
        started = time.perf_counter()
        self.process.stdin.write(json.dumps(request, ensure_ascii=False) + '\n')
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        elapsed = time.perf_counter() - started
        if not line:
            raise RuntimeError('shadow engine closed its output')
        return json.loads(line), elapsed

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=2)
        except Exception:  # noqa: BLE001 - closing a shadow never fails the caller
            self.process.kill()


class ShadowLog:
    def __init__(self, home):
        self.folder = Path(home) / LOG_DIR

    def write(self, entry):
        self.folder.mkdir(exist_ok=True)
        name = 'shadow-%s.ndjson' % time.strftime('%Y%m%d')
        with open(self.folder / name, 'a', encoding='utf-8') as handle:
            handle.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + '\n')


class Shadow:
    """Background comparer for `rpc-serve`: `submit` never blocks and drops work when the queue is full."""

    def __init__(self, binary, home, source='rpc-serve'):
        self.binary, self.home, self.source = binary, home, source
        self.log = ShadowLog(home)
        self.queue = queue.Queue(maxsize=32)
        self.reader = None
        self.thread = threading.Thread(target=self._run, name='nuun-shadow', daemon=True)
        self.thread.start()

    @classmethod
    def from_environment(cls, home):
        binary = configured_engine(home)
        if not binary:
            return None
        try:
            return cls(binary, home)
        except Exception:  # noqa: BLE001 - a broken shadow must not stop the reader
            return None

    def submit(self, method, params, envelope, python_seconds):
        try:
            self.queue.put_nowait((method, params, envelope, python_seconds))
        except queue.Full:
            pass

    def _run(self):
        while True:
            item = self.queue.get()
            if item is None:
                break
            method, params, envelope, python_seconds = item
            entry = {'at': time.strftime('%Y-%m-%dT%H:%M:%S'), 'source': self.source,
                     'python_ms': round(python_seconds * 1000, 2)}
            try:
                if self.reader is None:
                    self.reader = RustReader(self.binary, self.home)
                rust_envelope, rust_seconds = self.reader.ask(method, params)
                entry.update(compare(method, envelope, rust_envelope))
                entry['rust_ms'] = round(rust_seconds * 1000, 2)
            except Exception as error:  # noqa: BLE001 - recorded, never raised
                entry.update(method=method, outcome='shadow_failed', message=str(error)[:MAX_VALUE])
                if self.reader is not None:
                    self.reader.close()
                    self.reader = None
            try:
                self.log.write(entry)
            except OSError:
                pass

    def close(self, wait=2.0):
        try:
            self.queue.put_nowait(None)
        except queue.Full:
            pass
        self.thread.join(wait)
        if self.reader is not None:
            self.reader.close()


def summarize(entries):
    """Per method: how many answers were the same, different, not ported or failed, and median times."""
    by_method = {}
    for entry in entries:
        row = by_method.setdefault(entry.get('method', '?'), {'count': 0, 'outcomes': {}, 'python_ms': [], 'rust_ms': []})
        row['count'] += 1
        outcome = entry.get('outcome', '?')
        row['outcomes'][outcome] = row['outcomes'].get(outcome, 0) + 1
        if outcome in {'same', 'diff', 'same_error'}:
            for key in ('python_ms', 'rust_ms'):
                if isinstance(entry.get(key), (int, float)):
                    row[key].append(entry[key])
    result = {}
    for method, row in sorted(by_method.items()):
        def median(values):
            values = sorted(values)
            return round(values[len(values) // 2], 2) if values else None
        result[method] = {'count': row['count'], 'outcomes': row['outcomes'],
                          'python_ms_median': median(row['python_ms']), 'rust_ms_median': median(row['rust_ms'])}
    return result
