"""Shadow of the Rust engine (docs/platforms/rust-engine.md §7, PLAN «Motor en Rust · ruta»).

Python stays the engine that answers. When a shadow engine is configured, the same *read* is asked of
`nuun-engine rpc-serve` afterwards, on a background thread, and only the comparison is recorded: same answer,
different (with the JSON paths that differ), `not_ported`, or an error, plus both times. The comparison is strict
in the type (`1` is not `1.0`) and the log holds paths, codes and times, never values (nu9ve-engine-v1 §10). Nothing here can change,
delay or fail Python's answer, and nothing is sent anywhere: the log is a local file under the catalog folder.

Configured by `NUUN_SHADOW_ENGINE` (path to the binary) or by `<catalog>/shadow.json` `{"engine": "/path"}`;
`NUUN_SHADOW_ENGINE=off` disables it whatever the file says. Writes are never shadowed: `rpc-serve` only reads,
and the Rust engine refuses mutations there too.
"""
import json
import os
import queue
import re
import subprocess
import threading
import time
from pathlib import Path

LOG_DIR = 'shadow'
MAX_DIFFS = 20
# Rust names what it cannot answer yet inside an answer; outside the comparison, reported apart.
RUST_ONLY_KEYS = ('not_ported',)
# nu9ve-engine-v1 §3 and §5: what a code and a `not_ported` reason may look like; anything else is not logged.
CODE = re.compile(r'[a-z_]{1,64}')
REASON = re.compile(r'[a-z_]{1,40}')
# Values that change between two runs of the same read (the time it was asked): removed from both answers before
# comparing; that the key is there is still compared (§10).
VOLATILE = {'finals_status': ('checked_at',)}


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


def differences(oracle, candidate, path='$', out=None):
    """JSON paths where the two answers differ, at most MAX_DIFFS + 1.

    Strict in the type (§10, decision 2): `1` is not `1.0` and `true` is not `1`, as in canonical JSON. Only paths:
    the values are the person's titles and paths and never reach the log (decision 3).
    """
    out = [] if out is None else out
    if len(out) > MAX_DIFFS:
        return out
    if type(oracle) is not type(candidate):
        out.append(path)
    elif isinstance(oracle, dict):
        for key in sorted(set(oracle) | set(candidate)):
            if key not in oracle or key not in candidate:
                out.append(path + '.' + key)
            else:
                differences(oracle[key], candidate[key], path + '.' + key, out)
    elif isinstance(oracle, list):
        if len(oracle) != len(candidate):
            out.append(path + '.length')
        for index, (a, b) in enumerate(zip(oracle, candidate)):
            differences(a, b, '%s[%d]' % (path, index), out)
    elif oracle != candidate:
        out.append(path)
    return out


def _code(envelope):
    """The error code of an envelope, only if it is a code token; `None` for an answer."""
    if envelope.get('ok') is True:
        return None
    code = (envelope.get('error') or {}).get('code')
    return code if isinstance(code, str) and CODE.fullmatch(code) else 'invalid_code'


def _without(data, keys):
    if isinstance(data, dict) and any(key in data for key in keys):
        return {key: value for key, value in data.items() if key not in keys}
    return data


def _steady(data, keys):
    """The answer with its volatile values blanked, so only whether each key is there is compared."""
    if isinstance(data, dict) and any(key in data for key in keys):
        return {key: (None if key in keys else value) for key, value in data.items()}
    return data


def compare(method, oracle, candidate, volatile=None):
    """One log entry (without times) for the same request answered by Python (the oracle) and Rust.

    Only the method, the outcome, JSON paths, codes and a `not_ported` reason: never a value nor an error message.
    """
    volatile = VOLATILE.get(method, ()) if volatile is None else tuple(volatile)
    entry = {'method': method}
    candidate_code = _code(candidate)
    if candidate_code == 'not_ported':
        entry['outcome'] = 'not_ported'
        reason = (candidate.get('error') or {}).get('reason')
        if isinstance(reason, str) and REASON.fullmatch(reason):
            entry['reason'] = reason
        return entry
    oracle_code = _code(oracle)
    if oracle_code is not None or candidate_code is not None:
        entry['outcome'] = 'same_error' if oracle_code == candidate_code else 'error_mismatch'
        entry['oracle_error'], entry['candidate_error'] = oracle_code, candidate_code
        return entry
    candidate_data = candidate.get('data')
    partial = [key for key in RUST_ONLY_KEYS if isinstance(candidate_data, dict) and key in candidate_data]
    found = differences(_steady(oracle.get('data'), volatile),
                        _steady(_without(candidate_data, RUST_ONLY_KEYS), volatile))
    entry['outcome'] = 'same' if not found else 'diff'
    if partial:
        entry['rust_partial'] = partial
    if found:
        entry['diffs'] = found[:MAX_DIFFS]
        entry['more_diffs'] = len(found) > MAX_DIFFS
    return entry


def failure_reason(error):
    """`shadow_failed`: why the candidate gave no comparable answer, as a token (the message may carry paths)."""
    if isinstance(error, FileNotFoundError):
        return 'engine_missing'
    if isinstance(error, (BrokenPipeError, ConnectionError, EOFError)):
        return 'connection_lost'
    if isinstance(error, ValueError):
        return 'invalid_envelope'
    if isinstance(error, OSError):
        return 'engine_unavailable'
    return 'internal_error'


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
            raise EOFError('shadow engine closed its output')
        envelope = json.loads(line)
        # §2.1: an answer to another id is a protocol error, never the answer to this request.
        if not isinstance(envelope, dict) or envelope.get('id') != request['id'] or not isinstance(envelope.get('ok'), bool):
            raise ValueError('not the envelope of this request')
        return envelope, elapsed

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
                entry.update(method=method, outcome='shadow_failed', reason=failure_reason(error))
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
