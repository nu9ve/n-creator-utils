use super::*;
use crate::tree::Root;
use nu9ve_volumes::fs;
use std::collections::{BTreeMap, HashSet};
use std::fs::{File, Metadata, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::time::Instant;

const MIN_FREE: u64 = 512 * 1024 * 1024;
const MIN_ENTRY: u64 = 10 * 1024 * 1024;
const MIN_FOLDER: u64 = 50 * 1024 * 1024;

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap_or_default()
}
fn num(row: &Value, key: &str) -> u64 {
    row[key].as_u64().unwrap_or_default()
}
fn now() -> String {
    nu9ve_time::now_utc()
}

fn writable(home: &Path) -> Result<Connection> {
    let path = home.join("storage-index.sqlite3");
    for attempt in 0..2 {
        for suffix in ["", "-wal", "-shm"] {
            require(
                !home
                    .join(format!("storage-index.sqlite3{suffix}"))
                    .is_symlink(),
                "unsafe_state",
                "Storage index must not be a symlink",
            )?;
        }
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|e| fs::io_error(e, &path))?;
        let db = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(index_error)?;
        db.busy_timeout(std::time::Duration::from_secs(15))
            .map_err(index_error)?;
        let version = db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0));
        let init = match version {
            Ok(SCHEMA) => Ok(()),
            Ok(_) => {
                let count = db
                    .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .map_err(index_error)?;
                if count == 0 {
                    db.execute_batch(concat!(
                        "PRAGMA auto_vacuum=INCREMENTAL; BEGIN;",
                        include_str!("schema.sql"),
                        "PRAGMA user_version=1; COMMIT;"
                    ))
                    .map_err(index_error)
                } else {
                    Err(Error::new("index_version", "Storage index version changed"))
                }
            }
            Err(ref error)
                if matches!(
                    error.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
                ) =>
            {
                Err(Error::new("index_version", "Storage index is damaged"))
            }
            Err(error) => return Err(index_error(error)),
        };
        match init {
            Ok(()) => {
                db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL")
                    .map_err(index_error)?;
                return Ok(db);
            }
            Err(error) if attempt == 0 && error.code == "index_version" => {
                drop(db);
                for suffix in ["", "-wal", "-shm"] {
                    let file = home.join(format!("storage-index.sqlite3{suffix}"));
                    if let Err(error) = std::fs::remove_file(&file)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        return Err(fs::io_error(error, &file));
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

#[derive(Clone, Default)]
struct Rollup {
    files: u64,
    bytes: u64,
    allocated: u64,
    kinds: BTreeMap<&'static str, u64>,
}

impl Rollup {
    fn merge(&mut self, child: &Self) {
        self.files += child.files;
        self.bytes += child.bytes;
        self.allocated += child.allocated;
        for (kind, bytes) in &child.kinds {
            *self.kinds.entry(kind).or_default() += bytes;
        }
    }
}

fn suffix(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn file_kind(name: &str) -> &'static str {
    match suffix(name).as_str() {
        "mov" | "mp4" | "mpg" | "mts" | "insv" => "video",
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "insp" => "image",
        "cr2" | "cr3" | "nef" | "arw" | "dng" => "raw_image",
        "wav" | "mp3" | "caf" | "flac" | "aif" | "aiff" => "audio",
        _ => "other",
    }
}

fn folder_kind(name: &str) -> Option<&'static str> {
    match suffix(name).as_str() {
        "stalker" => Some("capture"),
        "fcpbundle" | "logicx" | "band" | "imovielibrary" => Some("editable"),
        "photoslibrary" => Some("image"),
        "app" | "framework" => Some("other"),
        _ => match name {
            "node_modules" | "__pycache__" | ".venv" | "venv" | ".build" | "DerivedData"
            | "Caches" | ".cache" | ".next" | "Render Files" | "Transcoded Media"
            | "Proxy Media" | "Peak Files" | "Thumbnail Media" => Some("derived"),
            ".git"
            | ".Trashes"
            | ".Trash"
            | ".Spotlight-V100"
            | ".fseventsd"
            | ".DocumentRevisions-V100"
            | ".TemporaryItems" => Some("other"),
            _ => None,
        },
    }
}

struct Frame {
    rel: PathBuf,
    kind: &'static str,
    hidden: bool,
    opaque: bool,
    children: Vec<Frame>,
    loaded: bool,
    rollup: Rollup,
}

impl Frame {
    fn new(rel: PathBuf, name: &str, parent: Option<&Self>) -> Self {
        let hidden = parent.is_some_and(|p| p.hidden || p.opaque);
        let kind = if hidden { None } else { folder_kind(name) };
        Self {
            rel,
            kind: kind.or(parent.map(|p| p.kind)).unwrap_or(""),
            hidden,
            opaque: kind.is_some(),
            children: Vec::new(),
            loaded: false,
            rollup: Rollup::default(),
        }
    }
}

struct Entry {
    path: String,
    parent: String,
    size: u64,
    allocated: u64,
    mtime: i64,
    kind: &'static str,
}
struct Folder {
    path: String,
    parent: String,
    rollup: Rollup,
    kind: &'static str,
    opaque: bool,
}

struct Scan<'a> {
    db: &'a mut Connection,
    target: &'a str,
    run: i64,
    root: &'a DiskRoot,
    home: &'a Path,
    cancelled: &'a dyn Fn() -> bool,
    keep_small: bool,
    seen: HashSet<u64>,
    entries: Vec<Entry>,
    folders: Vec<Folder>,
    samples: Vec<String>,
    unreadable: u64,
    changed: u64,
    files: u64,
    allocated: u64,
    flushed: Instant,
}

fn mtime(stat: &Metadata) -> i64 {
    stat.mtime() * 1_000_000_000 + stat.mtime_nsec()
}
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

impl Scan<'_> {
    fn unreadable(&mut self, path: &Path) {
        self.unreadable += 1;
        if self.samples.len() < 50 {
            self.samples.push(path.to_string_lossy().into_owned());
        }
    }

    fn flush(&mut self) -> Result<()> {
        require(
            nu9ve_volumes::macos::disk_usage(self.home)
                .map_err(|e| fs::io_error(e, self.home))?
                .2
                >= MIN_FREE,
            "low_space",
            "Too little free space on this Mac to keep the index",
        )?;
        let tx = self.db.transaction().map_err(index_error)?;
        {
            let mut stmt = tx
                .prepare("INSERT OR REPLACE INTO entries VALUES(?,?,?,?,?,?,?,?)")
                .map_err(index_error)?;
            for row in self.entries.drain(..) {
                stmt.execute((
                    self.target,
                    self.run,
                    row.path,
                    row.parent,
                    row.size,
                    row.allocated,
                    row.mtime,
                    row.kind,
                ))
                .map_err(index_error)?;
            }
            let mut stmt = tx
                .prepare("INSERT OR REPLACE INTO folders VALUES(?,?,?,?,?,?,?,?,?,?)")
                .map_err(index_error)?;
            for row in self.folders.drain(..) {
                stmt.execute((
                    self.target,
                    self.run,
                    row.path,
                    row.parent,
                    row.rollup.files,
                    row.rollup.bytes,
                    row.rollup.allocated,
                    serde_json::to_string(&row.rollup.kinds).expect("rollup JSON"),
                    row.kind,
                    row.opaque,
                ))
                .map_err(index_error)?;
            }
        }
        tx.execute(
            "UPDATE runs SET files=?,allocated=?,unreadable=?,changed=? WHERE id=?",
            (
                self.files,
                self.allocated,
                self.unreadable,
                self.changed,
                self.run,
            ),
        )
        .map_err(index_error)?;
        tx.commit().map_err(index_error)?;
        self.flushed = Instant::now();
        Ok(())
    }

    fn load(&mut self, frame: &mut Frame) -> Result<()> {
        let mut found = Vec::new();
        let listing = (|| -> io::Result<()> {
            let mut dir = Directory::open(self.root.directory(&frame.rel)?)?;
            while let Some((name, stat)) = dir.next()? {
                if (self.cancelled)() {
                    break;
                }
                match stat {
                    Ok(stat) => found.push((name, stat)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => self.changed += 1,
                    Err(_) => self.unreadable(&frame.rel.join(name)),
                }
            }
            Ok(())
        })();
        check_cancel(self.cancelled, "Scan cancelled")?;
        if let Err(error) = listing {
            self.root.check()?;
            if frame.rel.as_os_str().is_empty() {
                return Err(fs::io_error(error, &self.root.path));
            }
            if error.kind() == io::ErrorKind::NotFound {
                self.changed += 1;
            } else {
                if !matches!(
                    error.raw_os_error(),
                    Some(libc::EACCES | libc::EPERM | libc::ELOOP | libc::ENOTDIR | libc::EXDEV)
                ) {
                    Directory::open(
                        self.root
                            .directory(Path::new(""))
                            .map_err(|e| fs::io_error(e, &self.root.path))?,
                    )
                    .and_then(|mut dir| dir.next())
                    .map_err(|e| fs::io_error(e, &self.root.path))?;
                }
                self.unreadable(&frame.rel);
            }
            found.clear();
        }
        if !frame.opaque
            && !frame.hidden
            && found.iter().any(|(name, _)| name == "clipper-project.json")
        {
            frame.kind = "editable";
            frame.opaque = true;
        }
        for (name, stat) in found {
            let path = frame.rel.join(&name);
            if stat.st_mode & libc::S_IFMT == libc::S_IFDIR {
                if stat.st_dev as u64 == self.root.dev {
                    frame
                        .children
                        .push(Frame::new(path, &name.to_string_lossy(), Some(frame)));
                }
                continue;
            }
            if stat.st_mode & libc::S_IFMT != libc::S_IFREG
                || (stat.st_nlink > 1 && !self.seen.insert(stat.st_ino))
            {
                continue;
            }
            let kind = if frame.kind.is_empty() {
                file_kind(&name.to_string_lossy())
            } else {
                frame.kind
            };
            let size = stat.st_size as u64;
            let allocated = if stat.st_blocks == 0 {
                size
            } else {
                stat.st_blocks as u64 * 512
            };
            frame.rollup.files += 1;
            frame.rollup.bytes += size;
            frame.rollup.allocated += allocated;
            *frame.rollup.kinds.entry(kind).or_default() += allocated;
            if let Some(path) = path.to_str() {
                if self.keep_small || size >= MIN_ENTRY && kind != "derived" {
                    self.entries.push(Entry {
                        path: path.to_owned(),
                        parent: parent(path).to_owned(),
                        size,
                        allocated,
                        mtime: stat.st_mtime * 1_000_000_000 + stat.st_mtime_nsec,
                        kind,
                    });
                }
            } else {
                self.unreadable(&path);
            }
        }
        self.files += frame.rollup.files;
        self.allocated += frame.rollup.allocated;
        Ok(())
    }

    fn walk(&mut self) -> Result<Rollup> {
        let mut stack = vec![Frame::new(PathBuf::new(), "", None)];
        loop {
            let frame = stack.last_mut().expect("root frame");
            if !frame.loaded {
                check_cancel(self.cancelled, "Scan cancelled")?;
                self.load(frame)?;
                frame.loaded = true;
            } else if let Some(child) = frame.children.pop() {
                stack.push(child);
            } else {
                let frame = stack.pop().unwrap();
                let top = if let Some(parent) = stack.last_mut() {
                    parent.rollup.merge(&frame.rollup);
                    None
                } else {
                    Some(frame.rollup.clone())
                };
                if !frame.hidden
                    && (self.keep_small || top.is_some() || frame.rollup.allocated >= MIN_FOLDER)
                    && let Some(path) = frame.rel.to_str()
                {
                    self.folders.push(Folder {
                        path: path.to_owned(),
                        parent: parent(path).to_owned(),
                        rollup: frame.rollup,
                        kind: frame.kind,
                        opaque: frame.opaque,
                    });
                }
                if let Some(top) = top {
                    self.flush()?;
                    return Ok(top);
                }
            }
            if self.entries.len() + self.folders.len() >= 5000
                || self.flushed.elapsed().as_secs() >= 1
            {
                self.flush()?;
            }
        }
    }
}

pub fn scan(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let _lock = storage_lock(home)?;
    let _cancel = Cancel::install()?;
    scan_with(home, backend, params, Some(THIS_MAC_ROOT), &cancelled)
}

pub(super) fn scan_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    this_mac: Option<&str>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let targets = targets(&catalog, this_mac)?;
    let found = target(&targets, params)?;
    let id = text(&found, "id");
    require(
        online(backend, &found).is_some(),
        "volume_offline",
        format!("{} is disconnected", text(&found, "name")),
    )?;
    let root = Path::new(text(&found, "root"));
    let disk = DiskRoot::open(root)?;
    let mut db = writable(home)?;
    let tx = db.transaction().map_err(index_error)?;
    tx.execute(
        "UPDATE runs SET state='interrupted',finished_at=? WHERE state='running'",
        [now()],
    )
    .map_err(index_error)?;
    tx.execute_batch(
        "DELETE FROM entries WHERE run_id NOT IN (SELECT run_id FROM indexes);
        DELETE FROM folders WHERE run_id NOT IN (SELECT run_id FROM indexes);
        DELETE FROM runs WHERE id NOT IN (SELECT id FROM runs ORDER BY id DESC LIMIT 200)",
    )
    .map_err(index_error)?;
    tx.execute(
        "INSERT INTO runs(target_id,kind,state,pid,started_at) VALUES(?,'scan','running',?,?)",
        (id, std::process::id(), now()),
    )
    .map_err(index_error)?;
    let run = tx.last_insert_rowid();
    tx.commit().map_err(index_error)?;
    let started = Instant::now();
    let mut scan = Scan {
        db: &mut db,
        target: id,
        run,
        root: &disk,
        home,
        cancelled,
        keep_small: found["observed"] != true,
        seen: HashSet::new(),
        entries: Vec::new(),
        folders: Vec::new(),
        samples: Vec::new(),
        unreadable: 0,
        changed: 0,
        files: 0,
        allocated: 0,
        flushed: started,
    };
    let result = scan.walk().and_then(|top| {
        check_cancel(cancelled, "Scan cancelled")?;
        require(
            online(backend, &found).is_some() && disk.check().is_ok(),
            "volume_replaced",
            "Disk changed during scan",
        )?;
        let (total, used, free) = nu9ve_volumes::macos::disk_usage(root)
            .map_err(|e| fs::io_error(e, root))?;
        check_cancel(cancelled, "Scan cancelled")?;
        let tx = scan.db.transaction().map_err(index_error)?;
        let samples = json!(scan.samples).to_string();
        tx.execute(
            "INSERT OR REPLACE INTO indexes VALUES(?,?,?,?,?,?,?,?,?)",
            (id, run, now(), started.elapsed().as_secs_f64(), total, used, free, scan.unreadable, &samples),
        ).map_err(index_error)?;
        tx.execute("DELETE FROM entries WHERE target_id=? AND run_id<>?", (id, run))
            .map_err(index_error)?;
        tx.execute("DELETE FROM folders WHERE target_id=? AND run_id<>?", (id, run))
            .map_err(index_error)?;
        // New metadata invalidates the old cache and its derived folder totals at publication.
        tx.execute(
            "DELETE FROM hashes WHERE target_id=? AND NOT EXISTS
             (SELECT 1 FROM entries e WHERE e.target_id=hashes.target_id AND e.run_id=?
              AND e.path=hashes.path AND e.size=hashes.size AND e.mtime_ns=hashes.mtime_ns)",
            (id, run),
        ).map_err(index_error)?;
        if found["role"] == "archive" {
            tx.execute("DELETE FROM safety", []).map_err(index_error)?;
        } else {
            tx.execute("DELETE FROM safety WHERE target_id=?", [id]).map_err(index_error)?;
        }
        tx.execute(
            "UPDATE runs SET state='finished',finished_at=?,files=?,allocated=?,samples=? WHERE id=?",
            (now(), top.files, top.allocated, samples, run),
        ).map_err(index_error)?;
        tx.commit().map_err(index_error)?;
        Ok(top)
    });
    let unreadable = scan.unreadable;
    let top = match result {
        Ok(top) => top,
        Err(error) => {
            let tx = db.transaction().map_err(index_error)?;
            tx.execute(
                "DELETE FROM entries WHERE target_id=? AND run_id=?",
                (id, run),
            )
            .map_err(index_error)?;
            tx.execute(
                "DELETE FROM folders WHERE target_id=? AND run_id=?",
                (id, run),
            )
            .map_err(index_error)?;
            let cancelled = error.code == "cancelled";
            let (state, message) = if cancelled {
                ("cancelled", None)
            } else {
                ("failed", Some(&error.message))
            };
            tx.execute(
                "UPDATE runs SET state=?,finished_at=?,error=? WHERE id=?",
                (state, now(), message, run),
            )
            .map_err(index_error)?;
            tx.commit().map_err(index_error)?;
            reclaim(&db)?;
            if cancelled {
                return Ok(json!({"state":"cancelled","target":id}));
            }
            return Err(error);
        }
    };
    reclaim(&db)?;
    seed_receipts(&catalog, &mut db, &found, run)?;
    independence(backend, &mut db, &targets)?;
    for target in &targets {
        if target["role"] != "archive" {
            recompute(&catalog, &mut db, target, &targets)?;
        }
    }
    Ok(
        json!({"state":"finished","target":id,"files":top.files,"allocated":top.allocated,"bytes":top.bytes,
        "unreadable":unreadable,"seconds":round3(started.elapsed().as_secs_f64())}),
    )
}

fn reclaim(db: &Connection) -> Result<()> {
    db.execute_batch("PRAGMA incremental_vacuum; PRAGMA wal_checkpoint(TRUNCATE)")
        .map_err(index_error)
}

fn receipts(catalog: &Catalog, id: &str) -> Result<Vec<Value>> {
    catalog.json_rows(
        "SELECT json_object('sha256',sha256,'copy',json(payload)) FROM copies WHERE volume_id=?",
        [id],
    )
}

// HFS+ returns decomposed filenames even when the receipt used a composed filename.
fn nfc(path: &str) -> String {
    use std::ffi::c_void;
    type CfString = *const c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            allocator: CfString,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> CfString;
        fn CFStringCreateMutableCopy(
            allocator: CfString,
            limit: isize,
            source: CfString,
        ) -> CfString;
        fn CFStringNormalize(string: CfString, form: isize);
        fn CFStringGetLength(string: CfString) -> isize;
        fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
        fn CFStringGetCString(
            string: CfString,
            buffer: *mut u8,
            capacity: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: CfString);
    }
    if path.is_ascii() {
        return path.to_owned();
    }
    const UTF8: u32 = 0x08000100;
    // CoreFoundation owns the strings. Only the mutable copy is normalized.
    unsafe {
        let original = CFStringCreateWithBytes(
            std::ptr::null(),
            path.as_ptr(),
            path.len() as isize,
            UTF8,
            0,
        );
        if original.is_null() {
            return path.to_owned();
        }
        let normalized = CFStringCreateMutableCopy(std::ptr::null(), 0, original);
        CFRelease(original);
        if normalized.is_null() {
            return path.to_owned();
        }
        CFStringNormalize(normalized, 2);
        let size = CFStringGetMaximumSizeForEncoding(CFStringGetLength(normalized), UTF8) + 1;
        let mut buffer = vec![0; size.max(1) as usize];
        let success =
            CFStringGetCString(normalized, buffer.as_mut_ptr(), buffer.len() as isize, UTF8);
        CFRelease(normalized);
        if success != 0
            && let Ok(value) = std::ffi::CStr::from_bytes_until_nul(&buffer)
            && let Ok(value) = value.to_str()
        {
            return value.to_owned();
        }
    }
    path.to_owned()
}

fn seed_receipts(catalog: &Catalog, db: &mut Connection, target: &Value, run: i64) -> Result<()> {
    let id = text(target, "id");
    let tx = db.transaction().map_err(index_error)?;
    if target["observed"] != true {
        let receipts: HashMap<_, _> = receipts(catalog, id)?
            .into_iter()
            .filter_map(|row| {
                let destination = text(&row["copy"], "destination");
                if destination.is_empty() {
                    None
                } else {
                    Some((nfc(destination), row))
                }
            })
            .collect();
        if !receipts.is_empty() {
            let mut stmt = tx
                .prepare("SELECT path,size,mtime_ns FROM entries WHERE target_id=? AND run_id=?")
                .map_err(index_error)?;
            let mut entries = stmt.query((id, run)).map_err(index_error)?;
            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO hashes VALUES(?,?,?,?,?,'receipt',?)")
                .map_err(index_error)?;
            while let Some(row) = entries.next().map_err(index_error)? {
                let path: String = row.get(0).map_err(index_error)?;
                let size: u64 = row.get(1).map_err(index_error)?;
                let mtime: i64 = row.get(2).map_err(index_error)?;
                if let Some(receipt) = receipts.get(&nfc(&path))
                    && receipt["copy"]["bytes"] == size
                {
                    insert
                        .execute((
                            id,
                            path,
                            size,
                            mtime,
                            text(receipt, "sha256"),
                            text(&receipt["copy"], "verified_at"),
                        ))
                        .map_err(index_error)?;
                }
            }
        }
    }
    tx.commit().map_err(index_error)
}

fn independence(backend: &dyn Backend, db: &mut Connection, targets: &[Value]) -> Result<()> {
    let online: HashMap<_, _> = targets
        .iter()
        .filter_map(|target| Some((text(target, "id"), online(backend, target)?)))
        .collect();
    let tx = db.transaction().map_err(index_error)?;
    for archive in targets.iter().filter(|row| row["role"] == "archive") {
        for target in targets.iter().filter(|row| row["id"] != archive["id"]) {
            if let (Some(a), Some(b)) = (
                online.get(text(target, "id")),
                online.get(text(archive, "id")),
            ) {
                tx.execute(
                    "INSERT OR REPLACE INTO meta VALUES(?,?)",
                    (
                        independence_key(target, archive),
                        volumes::independent(a, b).to_string(),
                    ),
                )
                .map_err(index_error)?;
            }
        }
    }
    tx.commit().map_err(index_error)
}

fn indexed_run(db: &Connection, id: &str) -> Result<Option<i64>> {
    db.query_row(
        "SELECT run_id FROM indexes WHERE target_id=?",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(index_error)
}

const HASH_JOIN: &str = "LEFT JOIN hashes h ON h.target_id=e.target_id AND h.path=e.path
                       AND h.size=e.size AND h.mtime_ns=e.mtime_ns";

fn recompute(
    catalog: &Catalog,
    db: &mut Connection,
    target: &Value,
    targets: &[Value],
) -> Result<()> {
    let id = text(target, "id");
    let Some(run) = indexed_run(db, id)? else {
        return Ok(());
    };
    let evidence = Evidence::new(catalog, db, target, targets)?;
    let tx = db.transaction().map_err(index_error)?;
    tx.execute("DELETE FROM safety WHERE target_id=?", [id])
        .map_err(index_error)?;
    {
        let mut totals: HashMap<String, [u64; 5]> = HashMap::new();
        let mut stmt = tx
            .prepare(&format!(
                "SELECT e.parent,e.size,e.allocated,e.kind,h.sha256 FROM entries e {HASH_JOIN}
            WHERE e.target_id=? AND e.run_id=?"
            ))
            .map_err(index_error)?;
        let mut rows = stmt.query((id, run)).map_err(index_error)?;
        while let Some(row) = rows.next().map_err(index_error)? {
            let path: String = row.get(0).map_err(index_error)?;
            let size: i64 = row.get(1).map_err(index_error)?;
            let allocated: u64 = row.get(2).map_err(index_error)?;
            let kind: String = row.get(3).map_err(index_error)?;
            let sha: Option<String> = row.get(4).map_err(index_error)?;
            let state = SAFETY
                .iter()
                .position(|key| *key == evidence.state(size, &kind, sha.as_deref()))
                .expect("known safety state");
            let mut folder = path.as_str();
            loop {
                if let Some(bucket) = totals.get_mut(folder) {
                    bucket[state] += allocated;
                } else {
                    let mut bucket = [0; 5];
                    bucket[state] = allocated;
                    totals.insert(folder.to_owned(), bucket);
                }
                if folder.is_empty() {
                    break;
                }
                folder = parent(folder);
            }
        }
        let mut stmt = tx
            .prepare("SELECT path,allocated,kinds FROM folders WHERE target_id=? AND run_id=?")
            .map_err(index_error)?;
        let mut folders = stmt.query_map((id, run), row_json).map_err(index_error)?;
        let mut insert = tx
            .prepare("INSERT INTO safety VALUES(?,?,?,?,?,?,?)")
            .map_err(index_error)?;
        folders.try_for_each(|row| {
            let row = row.map_err(index_error)?;
            let path = text(&row, "path");
            let [verified, probable, only_here, derived, unknown] =
                totals.remove(path).unwrap_or_default();
            let rest = num(&row, "allocated")
                .saturating_sub(verified + probable + only_here + derived + unknown);
            let derived_rest =
                rest.min(num(&parse(&row["kinds"]), "derived").saturating_sub(derived));
            insert
                .execute((
                    id,
                    path,
                    verified,
                    probable,
                    only_here,
                    derived + derived_rest,
                    unknown + rest - derived_rest,
                ))
                .map_err(index_error)?;
            Ok(())
        })?;
    }
    tx.commit().map_err(index_error)
}

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

struct DiskRoot {
    path: PathBuf,
    file: File,
    dev: u64,
    ino: u64,
}

fn open_at(dir: &File, name: &CStr, flags: i32) -> io::Result<File> {
    // A successful openat returns an owned descriptor. No component may be a symlink.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

impl DiskRoot {
    fn open(path: &Path) -> Result<Self> {
        Root::open(path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| fs::io_error(e, path))?;
        let stat = file.metadata().map_err(|e| fs::io_error(e, path))?;
        let root = Self {
            path: path.to_owned(),
            file,
            dev: stat.dev(),
            ino: stat.ino(),
        };
        root.check()?;
        Ok(root)
    }

    fn check(&self) -> Result<()> {
        require(
            std::fs::symlink_metadata(&self.path).is_ok_and(|stat| {
                stat.is_dir() && (stat.dev(), stat.ino()) == (self.dev, self.ino)
            }),
            "root_changed",
            format!(
                "Root directory was replaced or disconnected: {}",
                self.path.display()
            ),
        )
    }

    fn directory(&self, rel: &Path) -> io::Result<File> {
        // Open "." independently so readdir cannot change the root descriptor's offset.
        let mut dir = open_at(&self.file, c".", libc::O_RDONLY | libc::O_DIRECTORY)?;
        for component in rel.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(io::Error::from_raw_os_error(libc::EINVAL));
            };
            let name = CString::new(name.as_bytes())?;
            dir = open_at(&dir, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
            if dir.metadata()?.dev() != self.dev {
                return Err(io::Error::from_raw_os_error(libc::EXDEV));
            }
        }
        Ok(dir)
    }

    fn open_file(&self, rel: &str) -> Result<Option<File>> {
        self.check()?;
        let name = rel.rsplit('/').next().unwrap_or_default();
        let open = || {
            let dir = self.directory(Path::new(parent(rel)))?;
            let name = CString::new(name)?;
            open_at(&dir, &name, libc::O_RDONLY | libc::O_NONBLOCK)
        };
        match open() {
            Ok(file) => Ok(Some(file)),
            Err(_) => {
                self.check()?;
                Ok(None)
            }
        }
    }
}

struct Directory(*mut libc::DIR);

impl Directory {
    fn open(file: File) -> io::Result<Self> {
        // fdopendir owns the file descriptor on success.
        let dir = unsafe { libc::fdopendir(file.as_raw_fd()) };
        if dir.is_null() {
            return Err(io::Error::last_os_error());
        }
        let _ = file.into_raw_fd();
        Ok(Self(dir))
    }

    fn next(&mut self) -> io::Result<Option<(std::ffi::OsString, io::Result<libc::stat>)>> {
        // readdir's pointer is valid until the next call. Copy its name before returning.
        unsafe {
            loop {
                *libc::__error() = 0;
                let entry = libc::readdir(self.0);
                if entry.is_null() {
                    return if *libc::__error() == 0 {
                        Ok(None)
                    } else {
                        Err(io::Error::last_os_error())
                    };
                }
                let name = CStr::from_ptr((*entry).d_name.as_ptr());
                if matches!(name.to_bytes(), b"." | b"..") {
                    continue;
                }
                let mut stat = std::mem::MaybeUninit::uninit();
                let code = libc::fstatat(
                    libc::dirfd(self.0),
                    name.as_ptr(),
                    stat.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                );
                let stat = if code == 0 {
                    Ok(stat.assume_init())
                } else {
                    Err(io::Error::last_os_error())
                };
                return Ok(Some((
                    std::ffi::OsString::from_vec(name.to_bytes().to_vec()),
                    stat,
                )));
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

struct Candidate {
    path: String,
    size: u64,
    mtime: i64,
    sha: Option<String>,
}

struct Compare<'a> {
    db: &'a Connection,
    cancelled: &'a dyn Fn() -> bool,
    run: i64,
    files: u64,
    bytes: u64,
    skipped: u64,
    matched: u64,
    hashed: u64,
    touched: Instant,
}

impl Compare<'_> {
    fn report(&mut self, force: bool) -> Result<()> {
        if force || self.touched.elapsed().as_secs() >= 1 {
            self.db
                .execute(
                    "UPDATE runs SET files=?,allocated=?,changed=? WHERE id=?",
                    (self.files, self.bytes, self.skipped, self.run),
                )
                .map_err(index_error)?;
            self.touched = Instant::now();
        }
        Ok(())
    }

    fn digest(
        &mut self,
        tree: &DiskRoot,
        id: &str,
        row: &Candidate,
        track: bool,
    ) -> Result<Option<String>> {
        tree.check()?;
        if row.sha.is_some() {
            return Ok(row.sha.clone());
        }
        let cached = self
            .db
            .query_row(
                "SELECT sha256 FROM hashes WHERE target_id=? AND path=? AND size=? AND mtime_ns=?",
                (id, &row.path, row.size, row.mtime),
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(index_error)?;
        if cached.is_some() {
            return Ok(cached);
        }
        let Some(file) = tree.open_file(&row.path)? else {
            return Ok(None);
        };
        let mut file = fs::uncached(file);
        let metadata = || {
            file.metadata()
                .map_err(|e| fs::io_error(e, &tree.path.join(&row.path)))
        };
        let first = metadata()?;
        let matches = |stat: &Metadata| {
            stat.is_file()
                && stat.dev() == tree.dev
                && (stat.len(), mtime(stat)) == (row.size, row.mtime)
        };
        if !matches(&first) {
            return Ok(None);
        }
        let sha = crate::transfer::hash_file_counted(&mut file, |count| {
            check_cancel(self.cancelled, "Comparison cancelled")?;
            if track {
                self.bytes += count;
                self.report(false)?;
            }
            Ok(())
        })?;
        check_cancel(self.cancelled, "Comparison cancelled")?;
        tree.check()?;
        let last = file
            .metadata()
            .map_err(|e| fs::io_error(e, &tree.path.join(&row.path)))?;
        if !matches(&last) {
            return Ok(None);
        }
        self.db
            .execute(
                "INSERT OR REPLACE INTO hashes VALUES(?,?,?,?,?,'computed',?)",
                (id, &row.path, row.size, row.mtime, &sha, now()),
            )
            .map_err(index_error)?;
        self.hashed += 1;
        Ok(Some(sha))
    }
}

pub fn compare(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let _lock = storage_lock(home)?;
    let _cancel = Cancel::install()?;
    compare_with(home, backend, params, Some(THIS_MAC_ROOT), &cancelled)
}

pub(super) fn compare_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    this_mac: Option<&str>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value> {
    let catalog = Catalog::open(home)?;
    let targets = targets(&catalog, this_mac)?;
    let found = target(&targets, params)?;
    let id = text(&found, "id");
    let path = if volumes::truthy(&params["path"]) {
        relative(&params["path"])?
    } else {
        String::new()
    };
    require(
        found["role"] != "archive",
        "invalid_role",
        "The archive is what gets compared against",
    )?;
    let mut db = writable(home)?;
    let index = indexed_run(&db, id)?.ok_or_else(|| {
        Error::new(
            "not_indexed",
            format!("Scan {} first", text(&found, "name")),
        )
    })?;
    require(
        online(backend, &found).is_some(),
        "volume_offline",
        format!("{} is disconnected", text(&found, "name")),
    )?;
    independence(backend, &mut db, &targets)?;
    let evidence = Evidence::new(&catalog, &db, &found, &targets)?;
    let archives: Vec<_> = evidence
        .archives
        .iter()
        .filter(|(archive, _)| online(backend, archive).is_some())
        .collect();
    require(
        !archives.is_empty(),
        "archive_not_available",
        "Connect and scan the backup disk first",
    )?;
    let mut wanted: HashMap<u64, Vec<(usize, Candidate)>> = HashMap::new();
    {
        let mut stmt = db.prepare("SELECT path,size,mtime_ns,NULL FROM entries WHERE target_id=? AND run_id=? AND size>0").map_err(index_error)?;
        for (i, (archive, run)) in archives.iter().enumerate() {
            let rows = stmt
                .query_map((text(archive, "id"), run), candidate)
                .map_err(index_error)?;
            for row in rows {
                let row = row.map_err(index_error)?;
                wanted.entry(row.size).or_default().push((i, row));
            }
        }
    }
    let mut candidates = Vec::new();
    let mut total_bytes = 0;
    {
        let mut stmt = db
            .prepare(&format!(
                "SELECT e.path,e.size,e.mtime_ns,h.sha256 FROM entries e {HASH_JOIN}
            WHERE e.run_id=? AND e.target_id=? AND e.size>0 AND e.kind!='derived'
            AND (?3='' OR e.path=?3 OR (e.path>=?4 AND e.path<?5)) ORDER BY e.size DESC"
            ))
            .map_err(index_error)?;
        for row in stmt
            .query_map(
                (index, id, &path, format!("{path}/"), format!("{path}0")),
                candidate,
            )
            .map_err(index_error)?
        {
            let row = row.map_err(index_error)?;
            if wanted.contains_key(&row.size)
                && row
                    .sha
                    .as_ref()
                    .is_none_or(|sha| !evidence.located.contains_key(sha))
            {
                total_bytes += row.size;
                candidates.push(row);
            }
        }
    }
    db.execute(
        "UPDATE runs SET state='interrupted',finished_at=? WHERE state='running'",
        [now()],
    )
    .map_err(index_error)?;
    db.execute("INSERT INTO runs(target_id,kind,state,pid,path,started_at,total_files,total_bytes) VALUES(?,'compare','running',?,?,?,?,?)",
        (id,std::process::id(),&path,now(),candidates.len(),total_bytes)).map_err(index_error)?;
    let run = db.last_insert_rowid();
    let mut comparison = Compare {
        db: &db,
        cancelled,
        run,
        files: 0,
        bytes: 0,
        skipped: 0,
        matched: 0,
        hashed: 0,
        touched: Instant::now(),
    };
    let result = (|| -> Result<()> {
        let source = DiskRoot::open(Path::new(text(&found, "root")))?;
        let archive_roots = archives
            .iter()
            .map(|(archive, _)| DiskRoot::open(Path::new(text(archive, "root"))))
            .collect::<Result<Vec<_>>>()?;
        for row in &candidates {
            check_cancel(cancelled, "Comparison cancelled")?;
            if let Some(sha) = comparison.digest(&source, id, row, true)? {
                for (i, other) in &wanted[&row.size] {
                    if comparison
                        .digest(&archive_roots[*i], text(archives[*i].0, "id"), other, false)?
                        .as_deref()
                        == Some(&sha)
                    {
                        comparison.matched += 1;
                        break;
                    }
                }
            } else {
                comparison.skipped += 1;
            }
            comparison.files += 1;
            comparison.report(false)?;
        }
        Ok(())
    })();
    let (state, error) = match &result {
        Ok(()) => ("finished", None),
        Err(error) if error.code == "cancelled" => ("cancelled", None),
        Err(error) => ("failed", Some(error.message.as_str())),
    };
    comparison.report(true)?;
    db.execute(
        "UPDATE runs SET state=?,finished_at=?,error=? WHERE id=?",
        (state, now(), error, run),
    )
    .map_err(index_error)?;
    let response = json!({"state":state,"target":id,"path":path,"files":comparison.files,"total_files":candidates.len(),
        "hashed":comparison.hashed,"matched":comparison.matched,"skipped":comparison.skipped,"bytes":comparison.bytes});
    recompute(&catalog, &mut db, &found, &targets)?;
    if state == "failed" {
        result?;
    }
    Ok(response)
}

fn candidate(row: &Row) -> rusqlite::Result<Candidate> {
    Ok(Candidate {
        path: row.get(0)?,
        size: row.get(1)?,
        mtime: row.get(2)?,
        sha: row.get(3)?,
    })
}

use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn interrupt(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

struct Cancel(libc::sigaction);

impl Cancel {
    // Storage mutations run in a dedicated one-shot worker.
    fn install() -> Result<Self> {
        CANCELLED.store(false, Ordering::Relaxed);
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = interrupt as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut old = std::mem::MaybeUninit::uninit();
            if libc::sigaction(libc::SIGINT, &action, old.as_mut_ptr()) != 0 {
                return Err(Error::new(
                    "operation_failed",
                    io::Error::last_os_error().to_string(),
                ));
            }
            Ok(Self(old.assume_init()))
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
    }
}

fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

fn check_cancel(cancelled: &dyn Fn() -> bool, message: &str) -> Result<()> {
    require(!cancelled(), "cancelled", message)
}
