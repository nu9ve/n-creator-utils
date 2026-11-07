"""One overall measure for a long action (nu9ve-interaction-v1 «Acciones largas»).

The pattern of closure.ActionProgress, shared by ingest, release and delivery from the archive:
`overall_done`/`overall_total` in bytes that never go back, `files_done`/`files_total`, the file in hand with its
origin and destination (`current`), and a short log of finished steps. It only reports; it never decides what is
copied, verified or removed.
"""
from pathlib import Path

from .common import now

LOG_LIMIT = 60


class ActionMeter:
    def __init__(self, overall_total, files_total):
        self.total = max(0, int(overall_total))
        self.done = 0
        self.files_total = files_total
        self.files_done = 0
        self.current = {}
        self.log = []

    def reach(self, value):
        """Move forward to `value` bytes; a smaller value (a retry, a re-read) never moves the bar back."""
        self.done = max(self.done, min(int(value), self.total))

    def holding(self, name, origin='', target='', action='copy'):
        self.current = {'name': Path(name).name, 'from': origin, 'to': target, 'action': action}

    def finished(self, kind, path, target=''):
        self.files_done = min(self.files_total, self.files_done + 1) if self.files_total else self.files_done + 1
        self.log.append({'at': now(), 'kind': kind, 'name': Path(path).name, 'target': target})
        del self.log[:-LOG_LIMIT]

    def fields(self):
        return {'overall_done': self.done, 'overall_total': self.total, 'files_done': self.files_done,
                'files_total': self.files_total, 'current': dict(self.current), 'log': list(self.log)}
