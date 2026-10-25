"""Bounded, durable IO and explicit service errors."""
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import unicodedata
import uuid

CHUNK_SIZE = 4 * 1024 * 1024


class MediaError(RuntimeError):
    def __init__(self, code, message, details=None):
        super().__init__(message)
        self.code, self.details = code, details or {}


def require(condition, code, message):
    if not condition:
        raise MediaError(code, message)


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z')


def uid():
    return str(uuid.uuid4())


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def relative(value):
    require(isinstance(value, str) and value and '\\' not in value and ':' not in value and '\x00' not in value,
            'unsafe_path', 'Invalid relative path')
    require(not value.startswith('/') and all(p not in {'', '.', '..'} for p in value.split('/')),
            'unsafe_path', 'Path must remain within its root')
    return value


def slug(value):
    import re
    require(isinstance(value, str) and len(value) <= 160, 'invalid_tag', 'Invalid project/profile tag')
    cleaned = re.sub(r'[^\w-]+', '-', unicodedata.normalize('NFC', value).strip(), flags=re.UNICODE).strip('-_')
    require(bool(cleaned) and cleaned not in {'.', '..'}, 'invalid_tag', 'Project/profile cannot be empty')
    return cleaned


def fingerprint(st):
    return {'size': st.st_size, 'mtime_ns': st.st_mtime_ns, 'ctime_ns': st.st_ctime_ns,
            'inode': st.st_ino, 'device': st.st_dev}


def content_stamp(st):
    # Device numbers can change after a genuine external volume remount.
    return {k: v for k, v in fingerprint(st).items() if k != 'device'}


class Tree:
    """All traversal is relative to an open root; reject symlinks at every level."""
    def __init__(self, root):
        self.root = Path(root).absolute()
        self.fd = None

    def __enter__(self):
        require(hasattr(os, 'O_NOFOLLOW'), 'unsupported_platform', 'Safe filesystem backend is unavailable')
        path = self.root
        for parent in (path, *path.parents):
            require(not parent.is_symlink(), 'unsafe_path', 'Symlink in root path: ' + str(parent))
        self.fd = os.open(str(path), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        self.identity = os.fstat(self.fd)
        return self

    def __exit__(self, *args):
        os.close(self.fd)

    def check(self):
        st = os.stat(str(self.root), follow_symlinks=False)
        require((st.st_dev, st.st_ino) == (self.identity.st_dev, self.identity.st_ino),
                'root_changed', 'Root directory was replaced or disconnected: ' + str(self.root))

    @contextlib.contextmanager
    def parent(self, rel, create=False):
        self.check()
        parts = relative(rel).split('/')
        fd = os.dup(self.fd)
        try:
            for part in parts[:-1]:
                if create:
                    try:
                        os.mkdir(part, 0o700, dir_fd=fd)
                        os.fsync(fd)
                    except FileExistsError:
                        pass
                next_fd = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                os.close(fd)
                fd = next_fd
            yield fd, parts[-1]
        finally:
            os.close(fd)

    def open(self, rel, flags=os.O_RDONLY, create_parents=False):
        with self.parent(rel, create_parents) as (fd, name):
            result = os.open(name, flags | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600, dir_fd=fd)
        try:
            require(stat.S_ISREG(os.fstat(result).st_mode), 'unsafe_file', 'Not a regular file: ' + rel)
            return result
        except BaseException:
            os.close(result)
            raise

    def stat(self, rel):
        with self.parent(rel) as (fd, name):
            st = os.stat(name, dir_fd=fd, follow_symlinks=False)
            require(stat.S_ISREG(st.st_mode), 'unsafe_file', 'Not a regular file: ' + rel)
            return st

    def unlink(self, rel, expected=None):
        with self.parent(rel) as (fd, name):
            st = os.stat(name, dir_fd=fd, follow_symlinks=False)
            require(stat.S_ISREG(st.st_mode), 'unsafe_file', 'Not a regular file: ' + rel)
            if expected is not None:
                require(content_stamp(st) == expected, 'source_changed', 'File changed before cleanup: ' + rel)
            os.unlink(name, dir_fd=fd)
            os.fsync(fd)

    def write_json(self, rel, value):
        with self.parent(rel, True) as (fd, name):
            temp = '.' + name + '.' + uid() + '.tmp'
            out = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=fd)
            try:
                with os.fdopen(out, 'wb') as stream:
                    stream.write(canonical(value) + b'\n')
                    stream.flush()
                    os.fsync(stream.fileno())
                # JSON is owned metadata. Refuse replacing links/special entries.
                try:
                    old = os.stat(name, dir_fd=fd, follow_symlinks=False)
                    require(stat.S_ISREG(old.st_mode), 'unsafe_file', 'Unsafe metadata target')
                except FileNotFoundError:
                    pass
                os.replace(temp, name, src_dir_fd=fd, dst_dir_fd=fd)
                os.fsync(fd)
            finally:
                try:
                    os.unlink(temp, dir_fd=fd)
                except FileNotFoundError:
                    pass


def hash_fd(fd, cancel=lambda: False):
    os.lseek(fd, 0, os.SEEK_SET)
    h = hashlib.sha256()
    while True:
        require(not cancel(), 'cancelled', 'Operation cancelled')
        block = os.read(fd, CHUNK_SIZE)
        if not block:
            return h.hexdigest()
        h.update(block)


def read_json(path, limit=16 * 1024 * 1024):
    path = Path(path).absolute()
    with Tree(path.parent) as tree:
        fd = tree.open(path.name)
        with os.fdopen(fd, 'rb') as stream:
            raw = stream.read(limit + 1)
    require(len(raw) <= limit, 'document_too_large', 'JSON input exceeds size limit')
    try:
        return json.loads(raw)
    except (ValueError, UnicodeError) as error:
        raise MediaError('invalid_json', 'Invalid JSON document') from error
