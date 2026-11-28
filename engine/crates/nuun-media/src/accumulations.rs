//! Acumulados en disco: builds, dependencias, cachés y respaldos que crecen sin que nadie los mire
//! (docs/features/storage-map.md#acumulados). Fase 1: inventario y plan en **sólo lectura**.
//!
//! - Las reglas son datos (`accumulations/rules.json`): por tipo, su señal, si se regenera y cuánto cuesta.
//! - Sólo metadatos: se listan carpetas y se hace `lstat`; no se sigue ningún enlace, no se sale del disco de la
//!   raíz y cada hard link cuenta una vez. Las únicas lecturas de contenido son los `gitdir` de los worktrees de
//!   git (unos bytes) para saber si quedaron huérfanos.
//! - Las bibliotecas de cada perfil y app (`profiles.json`) son datos vivos: nunca son un hallazgo ni se recorren.
//! - El tamaño es el asignado en disco (bloques × 512). En APFS un clon cuenta en cada copia y lo comprimido
//!   cuenta comprimido; `bytes_logical` da la suma de tamaños aparentes.
//! - El último uso es el mtime más nuevo del árbol: el atime de APFS no es fiable para esto.
//! - Cada escaneo es una revisión guardada en `<home>/accumulations/` (derivada, reconstruible): un JSON con el
//!   inventario y un NDJSON con una fila por hallazgo, que `data_export` entrega a la suite (nu9ve-data-v1).
//!
//! Nada aquí borra. El plan dice qué, cuánto, cómo se regenera y el riesgo; ejecutarlo (a la Papelera, con
//! recibo) es la fase 2.
use crate::catalog::{Catalog, WritableCatalog, writer_lock};
use nu9ve_json::{Error, Result, require};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SETTINGS: &str = "accumulations-settings";
pub const DIR: &str = "accumulations";
pub const DATASET: &str = "storage-accumulations";
const KEEP_SCANS: usize = 26;
const MIN_BYTES: u64 = 1024 * 1024;
const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: u64 = 5_000_000;
const GIB: u64 = 1024 * 1024 * 1024;
/// Umbral por omisión del disco interno: decisión abierta (PLAN), configurable en Ajustes.
const INTERNAL_MIN_FREE: u64 = 20 * GIB;
const EXTERNAL_MIN_FREE_PERCENT: u64 = 5;
/// Carpetas de código que se recorren si la persona no eligió otras.
const DEFAULT_ROOTS: [&str; 5] = ["code", "Developer", "Projects", "src", "dev"];

/// Las reglas declaradas (rules.json), validadas al cargar.
pub fn rules() -> Vec<Value> {
    let doc: Value =
        serde_json::from_str(include_str!("accumulations/rules.json")).unwrap_or_default();
    doc["rules"].as_array().cloned().unwrap_or_default()
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

// ------------------------------------------------------------------ metadata

#[cfg(unix)]
fn on_disk(m: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.blocks().saturating_mul(512)
}
#[cfg(not(unix))]
fn on_disk(m: &Metadata) -> u64 {
    m.len()
}
#[cfg(unix)]
fn device(m: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.dev()
}
#[cfg(not(unix))]
fn device(_: &Metadata) -> u64 {
    0
}
/// `(dev, ino)` of a file with more than one link, to count it once.
#[cfg(unix)]
fn link_key(m: &Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    (m.nlink() > 1 && !m.is_dir()).then(|| (m.dev(), m.ino()))
}
#[cfg(not(unix))]
fn link_key(_: &Metadata) -> Option<(u64, u64)> {
    None
}

fn mtime(m: &Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn utc(secs: i64) -> Value {
    if secs <= 0 {
        Value::Null
    } else {
        json!(nu9ve_profiles::utc_at(secs))
    }
}

#[derive(Default, Clone, Copy)]
struct Size {
    on_disk: u64,
    logical: u64,
    files: u64,
    newest: i64,
}

/// The size of a tree by `lstat` only: no link followed, never off `dev`, hard links once.
fn measure(root: &Path, dev: u64, seen: &mut HashSet<(u64, u64)>, budget: &mut u64) -> Size {
    let mut size = Size::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(m) = fs::symlink_metadata(&dir) {
            size.on_disk += on_disk(&m);
            size.newest = size.newest.max(mtime(&m));
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if *budget == 0 {
                return size;
            }
            *budget -= 1;
            let Ok(m) = entry.metadata() else { continue };
            if device(&m) != dev {
                continue;
            }
            if m.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if let Some(key) = link_key(&m)
                && !seen.insert(key)
            {
                continue;
            }
            size.on_disk += on_disk(&m);
            size.logical += m.len();
            size.files += 1;
            size.newest = size.newest.max(mtime(&m));
        }
    }
    size
}

// ------------------------------------------------------------------ signals

fn has_any(dir: &Path, names: &[&str]) -> bool {
    names
        .iter()
        .any(|n| fs::symlink_metadata(dir.join(n)).is_ok())
}

/// Any entry of `dir` (or of its subfolders, one level down) ending in one of `exts`.
fn contains_ext(dir: &Path, exts: &[&str]) -> bool {
    let matches = |name: &str| {
        let lower = name.to_ascii_lowercase();
        exts.iter().any(|e| lower.ends_with(e))
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten().take(2000) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches(&name) {
            return true;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir())
            && let Ok(inner) = fs::read_dir(entry.path())
            && inner
                .flatten()
                .take(2000)
                .any(|e| matches(&e.file_name().to_string_lossy()))
        {
            return true;
        }
    }
    false
}

/// The first rule whose `names` and conditions this folder meets.
fn match_rule<'a>(rules: &'a [Value], dir: &Path, name: &str) -> Option<&'a Value> {
    let parent = dir.parent()?;
    rules.iter().find(|rule| {
        strs(&rule["names"]).contains(&name)
            && (rule["inside_any"].is_null() || has_any(dir, &strs(&rule["inside_any"])))
            && (rule["sibling_any"].is_null() || has_any(parent, &strs(&rule["sibling_any"])))
            && (rule["contains_ext_any"].is_null()
                || contains_ext(dir, &strs(&rule["contains_ext_any"])))
    })
}

// ------------------------------------------------------------------ owners

/// Claude Code keeps a folder per project named after its path, every non-alphanumeric byte as `-`.
fn claude_project(user_home: &Path, project: &Path) -> PathBuf {
    let name: String = project
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    user_home.join(".claude/projects").join(name)
}

/// The newest mtime among a folder's direct entries (names are never returned).
fn newest_entry(dir: &Path) -> i64 {
    let mut newest = fs::symlink_metadata(dir).map_or(0, |m| mtime(&m));
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten().take(10_000) {
            if let Ok(m) = entry.metadata() {
                newest = newest.max(mtime(&m));
            }
        }
    }
    newest
}

/// The nearest folder with `.git` from `start` up to `stop` (inclusive).
fn repo_of(start: &Path, stop: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if fs::symlink_metadata(d.join(".git")).is_ok() {
            return Some(d.to_path_buf());
        }
        if d == stop {
            break;
        }
        dir = d.parent();
    }
    None
}

/// U2 (2026-11-27): the repository's remote, `origin` or else the first one, from its `.git/config` (the one file
/// read besides worktree `gitdir`s). A user name or token in an `https` URL is left out.
fn repo_remote(repo: &Path) -> Option<String> {
    let mut git = repo.join(".git");
    if git.is_file() {
        // A worktree: `gitdir: <path>/.git/worktrees/<name>`, whose `commondir` leads to the shared config.
        let text = read_small(&git)?;
        let pointed = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
        let common = read_small(&pointed.join("commondir")).map(|c| pointed.join(c.trim()));
        git = common.unwrap_or(pointed);
    }
    let config = read_small(&git.join("config"))?;
    let mut remotes: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            current = line
                .strip_prefix("[remote \"")
                .and_then(|rest| rest.strip_suffix("\"]"))
                .map(str::to_owned);
        } else if let (Some(name), Some(url)) = (&current, line.strip_prefix("url"))
            && let Some(url) = url.trim_start().strip_prefix('=')
        {
            remotes.push((name.clone(), without_credentials(url.trim())));
        }
    }
    remotes
        .iter()
        .find(|(name, _)| name == "origin")
        .or(remotes.first())
        .map(|(_, url)| url.clone())
}

/// `https://user:token@host/x` → `https://host/x`; other forms as they are.
fn without_credentials(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) if scheme.starts_with("http") => match rest.split_once('@') {
            Some((auth, host)) if !auth.contains('/') => format!("{scheme}://{host}"),
            _ => url.to_owned(),
        },
        _ => url.to_owned(),
    }
}

fn read_small(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    (meta.is_file() && meta.len() <= 64 * 1024).then(|| fs::read_to_string(path).ok())?
}

/// U2: a project's own icon, the first of the usual places that exists (metadata only; the app draws it).
fn project_icon(project: &Path) -> Option<PathBuf> {
    const PLACES: [&str; 10] = [
        "favicon.ico",
        "public/favicon.ico",
        "app/favicon.ico",
        "src/app/favicon.ico",
        "static/favicon.ico",
        "public/favicon.svg",
        "public/icon.png",
        "icon.png",
        "assets/icon.png",
        "docs/icon.png",
    ];
    PLACES
        .iter()
        .map(|p| project.join(p))
        .find(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_file() && m.len() <= 1024 * 1024))
}

/// The last commit, checkout or fetch of a repository, by the mtime of its reflog (metadata only).
fn repo_activity(repo: &Path) -> i64 {
    let git = repo.join(".git");
    ["logs/HEAD", "index", "FETCH_HEAD", "HEAD"]
        .iter()
        .filter_map(|p| fs::symlink_metadata(git.join(p)).ok())
        .map(|m| mtime(&m))
        .max()
        .or_else(|| fs::symlink_metadata(&git).ok().map(|m| mtime(&m)))
        .unwrap_or(0)
}

struct Library {
    profile_id: String,
    display_name: String,
    app: String,
    path: PathBuf,
}

fn library_of<'a>(libraries: &'a [Library], path: &Path) -> Option<&'a Library> {
    libraries.iter().find(|l| path.starts_with(&l.path))
}

// ------------------------------------------------------------------ git

/// A repository's `.git`: packs, loose objects and worktrees whose folder is gone.
fn git_detail(git: &Path) -> Value {
    let objects = git.join("objects");
    let (mut loose, mut loose_bytes) = (0u64, 0u64);
    if let Ok(entries) = fs::read_dir(&objects) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() == 2 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
                for object in fs::read_dir(entry.path()).into_iter().flatten().flatten() {
                    if let Ok(m) = object.metadata() {
                        loose += 1;
                        loose_bytes += on_disk(&m);
                    }
                }
            }
        }
    }
    let pack_bytes: u64 = fs::read_dir(objects.join("pack"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| on_disk(&m))
        .sum();
    let (mut worktrees, mut orphaned) = (0u64, 0u64);
    for entry in fs::read_dir(git.join("worktrees"))
        .into_iter()
        .flatten()
        .flatten()
    {
        worktrees += 1;
        let mut text = String::new();
        if let Ok(f) = fs::File::open(entry.path().join("gitdir")) {
            use std::io::Read;
            let _ = f.take(4096).read_to_string(&mut text);
        }
        let target = PathBuf::from(text.trim());
        if text.trim().is_empty() || fs::symlink_metadata(&target).is_err() {
            orphaned += 1;
        }
    }
    json!({"pack_bytes": pack_bytes, "loose_objects": loose, "loose_bytes": loose_bytes,
           "worktrees": worktrees, "orphaned_worktrees": orphaned})
}

// ------------------------------------------------------------------ volumes

#[cfg(unix)]
fn space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `s` a writable statvfs.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let block = s.f_frsize as u64;
    Some((s.f_blocks as u64 * block, s.f_bavail as u64 * block))
}
#[cfg(not(unix))]
fn space(_: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(target_os = "macos")]
fn mount_point(path: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `s` a writable statfs.
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    // SAFETY: the kernel fills f_mntonname with a NUL-terminated string.
    let name = unsafe { std::ffi::CStr::from_ptr(s.f_mntonname.as_ptr()) };
    Some(name.to_string_lossy().into_owned())
}
#[cfg(not(target_os = "macos"))]
fn mount_point(_: &Path) -> Option<String> {
    None
}

fn internal(mount: &str) -> bool {
    mount == "/" || mount == "/System/Volumes/Data"
}

/// A volume now: its space and whether it is almost full by the configured thresholds.
fn volume_now(path: &Path, settings: &Value) -> Value {
    let mount = mount_point(path).unwrap_or_else(|| "/".to_owned());
    let is_internal = internal(&mount);
    let (total, free) = space(path).unwrap_or((0, 0));
    let min_free = if is_internal {
        settings["internal_min_free_bytes"]
            .as_u64()
            .unwrap_or(INTERNAL_MIN_FREE)
    } else {
        total / 100
            * settings["external_min_free_percent"]
                .as_u64()
                .unwrap_or(EXTERNAL_MIN_FREE_PERCENT)
    };
    let label = if is_internal {
        "Esta Mac".to_owned()
    } else {
        Path::new(&mount)
            .file_name()
            .map_or(mount.clone(), |n| n.to_string_lossy().into_owned())
    };
    json!({"mount": mount, "label": label, "internal": is_internal, "total_bytes": total,
           "free_bytes": free, "min_free_bytes": min_free, "almost_full": total > 0 && free < min_free})
}

// ------------------------------------------------------------------ settings

fn settings_of(catalog: &Catalog) -> Result<Value> {
    let saved = catalog.get(SETTINGS)?.unwrap_or(json!({}));
    Ok(json!({
        "roots": saved.get("roots").cloned().unwrap_or(Value::Null),
        "internal_min_free_bytes": saved["internal_min_free_bytes"].as_u64().unwrap_or(INTERNAL_MIN_FREE),
        "external_min_free_percent": saved["external_min_free_percent"].as_u64().unwrap_or(EXTERNAL_MIN_FREE_PERCENT)}))
}

/// `storage_accumulations_settings`: sin parámetros lee; con ellos guarda en el catálogo, bajo su candado.
pub fn settings(home: &Path, params: &Value) -> Result<Value> {
    let given = params
        .as_object()
        .filter(|p| {
            p.keys().all(|k| {
                [
                    "roots",
                    "internal_min_free_bytes",
                    "external_min_free_percent",
                ]
                .contains(&k.as_str())
            })
        })
        .ok_or_else(|| Error::new("invalid_params", "Unknown parameters"))?;
    if given.is_empty() {
        return settings_of(&Catalog::open(home)?);
    }
    let _lock = writer_lock(home)?;
    let mut value = settings_of(&Catalog::open(home)?)?;
    if let Some(roots) = given.get("roots") {
        if roots.is_null() {
            value["roots"] = Value::Null;
        } else {
            let list = roots.as_array().filter(|l| l.len() <= 32).ok_or_else(|| {
                Error::new("invalid_params", "roots is a list of up to 32 folders")
            })?;
            for root in list {
                let p = root.as_str().map(Path::new);
                require(
                    p.is_some_and(|p| p.is_absolute() && p != Path::new("/")),
                    "invalid_params",
                    "Each root is an absolute folder other than /",
                )?;
            }
            value["roots"] = roots.clone();
        }
    }
    if let Some(v) = given.get("internal_min_free_bytes") {
        value["internal_min_free_bytes"] = json!(
            v.as_u64()
                .filter(|n| *n <= 1024 * GIB)
                .ok_or_else(|| Error::new("invalid_params", "internal_min_free_bytes is bytes"))?
        );
    }
    if let Some(v) = given.get("external_min_free_percent") {
        value["external_min_free_percent"] = json!(
            v.as_u64()
                .filter(|n| *n <= 50)
                .ok_or_else(|| Error::new("invalid_params", "external_min_free_percent is 0–50"))?
        );
    }
    WritableCatalog::open(home)?.set(SETTINGS, &value)?;
    Ok(value)
}

// ------------------------------------------------------------------ scan

/// Everything a scan needs, so tests pass their own folders.
pub struct Context {
    pub user_home: PathBuf,
    pub roots: Vec<PathBuf>,
    pub profiles: Option<Value>,
    pub max_entries: u64,
}

impl Context {
    /// The roots chosen in the settings, or the usual code folders under the person's home that exist.
    pub fn new(user_home: PathBuf, settings: &Value, profiles: Option<Value>) -> Self {
        let roots = match settings["roots"].as_array() {
            Some(list) => list
                .iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect(),
            None => DEFAULT_ROOTS
                .iter()
                .map(|r| user_home.join(r))
                .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
                .collect(),
        };
        Context {
            user_home,
            roots,
            profiles,
            max_entries: MAX_ENTRIES,
        }
    }
}

struct Found<'a> {
    rule: &'a Value,
    path: PathBuf,
    size: Size,
    dev: u64,
    detail: Option<Value>,
}

fn libraries_of(ctx: &Context) -> Vec<Library> {
    crate::suite_profiles::libraries(ctx.profiles.as_ref())
        .into_iter()
        .map(|(profile_id, display_name, app, path)| Library {
            profile_id,
            display_name,
            app,
            path,
        })
        .collect()
}

/// The inventory: findings, their owners and volumes, and the plan. Never writes.
pub fn inventory(ctx: &Context, settings: &Value, scanned_at: &str, scan_id: &str) -> Value {
    let rules = rules();
    let libraries = libraries_of(ctx);
    let mut seen = HashSet::new();
    let mut budget = ctx.max_entries;
    let mut found: Vec<Found> = Vec::new();
    let mut walked_roots = Vec::new();
    // 1. Code roots: folders matched by name and signal.
    for root in &ctx.roots {
        let Ok(meta) = fs::symlink_metadata(root) else {
            walked_roots.push(json!({"path": root, "state": "missing"}));
            continue;
        };
        if !meta.is_dir() {
            walked_roots.push(json!({"path": root, "state": "not_a_folder"}));
            continue;
        }
        walked_roots.push(json!({"path": root, "state": "ok"}));
        let dev = device(&meta);
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if budget == 0 {
                    break;
                }
                budget -= 1;
                let Ok(m) = entry.metadata() else { continue };
                if !m.is_dir() || device(&m) != dev {
                    continue;
                }
                let path = entry.path();
                if library_of(&libraries, &path).is_some() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(rule) = match_rule(&rules, &path, &name) {
                    let size = measure(&path, dev, &mut seen, &mut budget);
                    let detail = (rule["git"] == true).then(|| git_detail(&path));
                    found.push(Found {
                        rule,
                        path,
                        size,
                        dev,
                        detail,
                    });
                } else if depth < MAX_DEPTH && name != ".git" {
                    stack.push((path, depth + 1));
                }
            }
        }
    }
    // 2. Fixed places under the person's home.
    for rule in &rules {
        let mut places: Vec<PathBuf> = Vec::new();
        for p in strs(&rule["paths"]) {
            let at = ctx.user_home.join(p);
            if rule["children"] == true {
                let mut kids: Vec<PathBuf> = fs::read_dir(&at)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .collect();
                kids.sort();
                places.extend(kids);
            } else {
                places.push(at);
            }
        }
        for p in strs(&rule["children_of"]) {
            let mut kids: Vec<PathBuf> = fs::read_dir(ctx.user_home.join(p))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            kids.sort();
            places.extend(kids);
        }
        for path in places {
            let Ok(m) = fs::symlink_metadata(&path) else {
                continue;
            };
            if m.file_type().is_symlink() || library_of(&libraries, &path).is_some() {
                continue;
            }
            let dev = device(&m);
            let size = if m.is_dir() {
                measure(&path, dev, &mut seen, &mut budget)
            } else {
                Size {
                    on_disk: on_disk(&m),
                    logical: m.len(),
                    files: 1,
                    newest: mtime(&m),
                }
            };
            found.push(Found {
                rule,
                path,
                size,
                dev,
                detail: None,
            });
        }
    }
    // 3. Aggregate per project where the rule asks (`__pycache__`), drop what is too small.
    let mut merged: Vec<(Found, u64)> = Vec::new();
    let mut by_project: HashMap<(String, PathBuf), usize> = HashMap::new();
    for f in found {
        if f.rule["aggregate"] == "project" {
            let root = f.path.parent().map(Path::to_path_buf).unwrap_or_default();
            // The repository, else the first folder under the scanned root: the project.
            let owner = ctx
                .roots
                .iter()
                .find(|r| root.starts_with(r))
                .and_then(|r| {
                    repo_of(&root, r).or_else(|| {
                        let first = root.strip_prefix(r).ok()?.components().next()?;
                        Some(r.join(first))
                    })
                })
                .unwrap_or(root);
            let key = (
                f.rule["id"].as_str().unwrap_or_default().to_owned(),
                owner.clone(),
            );
            if let Some(&i) = by_project.get(&key) {
                let (into, count) = &mut merged[i];
                into.size.on_disk += f.size.on_disk;
                into.size.logical += f.size.logical;
                into.size.files += f.size.files;
                into.size.newest = into.size.newest.max(f.size.newest);
                *count += 1;
                continue;
            }
            by_project.insert(key, merged.len());
            merged.push((Found { path: owner, ..f }, 1));
        } else {
            merged.push((f, 1));
        }
    }
    let mut rows = Vec::new();
    let mut volumes: BTreeMap<u64, PathBuf> = BTreeMap::new();
    for (f, count) in merged {
        let min = f.rule["min_bytes"].as_u64().unwrap_or(MIN_BYTES);
        if f.size.on_disk < min {
            continue;
        }
        volumes.entry(f.dev).or_insert_with(|| f.path.clone());
        let aggregated = f.rule["aggregate"] == "project";
        let project = if aggregated {
            f.path.clone()
        } else {
            f.path.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        let stop = ctx
            .roots
            .iter()
            .find(|r| project.starts_with(r))
            .cloned()
            .unwrap_or_else(|| ctx.user_home.clone());
        let repo = if f.rule["git"] == true {
            Some(project.clone())
        } else {
            repo_of(&project, &stop)
        };
        let agents = repo
            .iter()
            .chain(std::iter::once(&project))
            .map(|p| newest_entry(&claude_project(&ctx.user_home, p)))
            .max()
            .unwrap_or(0);
        let owner_library = library_of(&libraries, &f.path);
        let id = nu9ve_json::sha256_hex(f.path.to_string_lossy().as_bytes())[..16].to_owned();
        let mut row = json!({
            "id": id, "scan_id": scan_id, "scanned_at": scanned_at,
            "path": f.path, "rule": f.rule["id"], "kind": f.rule["kind"], "label": f.rule["label"],
            "bytes_on_disk": f.size.on_disk, "bytes_logical": f.size.logical, "files": f.size.files,
            "folders": if aggregated { json!(count) } else { json!(1) },
            "last_used_at": utc(f.size.newest), "last_used_source": "mtime_newest",
            "project": if f.rule["paths"].is_null() && f.rule["children_of"].is_null() { json!(project) } else { Value::Null },
            "repo": repo, "repo_activity_at": repo.as_deref().map_or(Value::Null, |r| utc(repo_activity(r))),
            "agent_activity_at": utc(agents), "agent_activity_source": "claude_projects_mtime",
            "owner_app": f.rule.get("app").cloned().or_else(|| owner_library.map(|l| json!(l.app))).unwrap_or(Value::Null),
            "owner_profile": owner_library.map_or(Value::Null, |l| json!(l.profile_id)),
            "regenerates": f.rule["regenerates"], "cost": f.rule["cost"], "cost_note": f.rule["cost_note"],
            "risk": f.rule["risk"], "action": f.rule["action"], "confirmations": f.rule["confirmations"],
            "volume": mount_point(&f.path).unwrap_or_else(|| "/".to_owned())});
        if let Some(detail) = f.detail {
            row["git"] = detail;
        }
        if let Some(remote) = repo.as_deref().and_then(repo_remote) {
            row["repo_remote"] = json!(remote);
        }
        if let Some(icon) = repo
            .iter()
            .chain(std::iter::once(&project))
            .find_map(|p| project_icon(p))
        {
            row["project_icon"] = json!(icon);
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| {
        b["bytes_on_disk"]
            .as_u64()
            .cmp(&a["bytes_on_disk"].as_u64())
            .then_with(|| a["path"].as_str().cmp(&b["path"].as_str()))
    });
    let volume_paths: Vec<PathBuf> = volumes.into_values().collect();
    json!({
        "contract": "nuun-accumulations/1", "scan_id": scan_id, "scanned_at": scanned_at,
        "roots": walked_roots, "truncated": budget == 0,
        "findings": rows, "volume_paths": volume_paths,
        "libraries": libraries_view(&libraries),
        "summary": summary(&rows_ref(&rows)), "settings": settings})
}

fn rows_ref(rows: &[Value]) -> Vec<&Value> {
    rows.iter().collect()
}

/// Bytes and counts per kind, for the history of what grows.
fn summary(rows: &[&Value]) -> Value {
    let mut by_kind: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut total = 0u64;
    for r in rows {
        let bytes = r["bytes_on_disk"].as_u64().unwrap_or(0);
        total += bytes;
        let e = by_kind
            .entry(r["kind"].as_str().unwrap_or("other").to_owned())
            .or_default();
        e.0 += bytes;
        e.1 += 1;
    }
    json!({"bytes": total, "findings": rows.len(),
           "by_kind": by_kind.into_iter().map(|(k, (b, n))| (k, json!({"bytes": b, "findings": n}))).collect::<Map<_, _>>()})
}

/// The libraries of every profile and app, grouped by profile: live data, never accumulated.
fn libraries_view(libraries: &[Library]) -> Value {
    let mut by_profile: BTreeMap<&str, (String, Vec<Value>)> = BTreeMap::new();
    for l in libraries {
        let entry = by_profile
            .entry(&l.profile_id)
            .or_insert_with(|| (l.display_name.clone(), Vec::new()));
        entry.1.push(json!({"app": l.app, "library": l.path,
                            "present": fs::symlink_metadata(&l.path).is_ok_and(|m| m.is_dir())}));
    }
    json!(
        by_profile
            .into_iter()
            .map(|(id, (name, apps))| json!({"profile_id": id, "display_name": name, "apps": apps}))
            .collect::<Vec<_>>()
    )
}

/// The cleanup plan by kind: what, how much it frees, how it comes back, the risk and how many confirmations.
/// Nothing in it is an action: `action: manual` groups are never offered for the Trash.
pub fn plan(rows: &[Value]) -> Value {
    let mut groups: BTreeMap<String, Value> = BTreeMap::new();
    for row in rows {
        let rule = row["rule"].as_str().unwrap_or_default().to_owned();
        let g = groups.entry(rule.clone()).or_insert_with(|| {
            json!({"rule": rule, "kind": row["kind"], "label": row["label"], "regenerates": row["regenerates"],
                   "cost": row["cost"], "cost_note": row["cost_note"], "risk": row["risk"],
                   "action": row["action"], "confirmations": row["confirmations"],
                   "bytes": 0, "findings": 0, "ids": []})
        });
        g["bytes"] =
            json!(g["bytes"].as_u64().unwrap_or(0) + row["bytes_on_disk"].as_u64().unwrap_or(0));
        g["findings"] = json!(g["findings"].as_u64().unwrap_or(0) + 1);
        g["ids"].as_array_mut().unwrap().push(row["id"].clone());
    }
    let mut list: Vec<Value> = groups.into_values().collect();
    list.sort_by(|a, b| b["bytes"].as_u64().cmp(&a["bytes"].as_u64()));
    let trash: u64 = list
        .iter()
        .filter(|g| g["action"] == "trash")
        .map(|g| g["bytes"].as_u64().unwrap_or(0))
        .sum();
    json!({"groups": list, "trash_bytes": trash, "automatic": false, "needs": "reviewed_plan",
           "default_destination": "trash"})
}

// ------------------------------------------------------------------ persistence

fn scans_dir(home: &Path) -> PathBuf {
    home.join(DIR)
}

/// The scans kept, newest last, as `(scan_id, json path)`.
fn scans(home: &Path) -> Vec<(String, PathBuf)> {
    let mut list: Vec<(String, PathBuf)> = fs::read_dir(scans_dir(home))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?.to_owned();
            Some((id, e.path()))
        })
        .collect();
    list.sort();
    list
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// The newest scan, with the space of its volumes now and the plan; plus the totals of every kept scan.
pub fn read(home: &Path) -> Result<Value> {
    let settings = settings_of(&Catalog::open(home)?)?;
    let kept = scans(home);
    let history: Vec<Value> = kept
        .iter()
        .filter_map(|(id, path)| {
            let scan = read_json(path)?;
            Some(json!({"scan_id": id, "scanned_at": scan["scanned_at"], "summary": scan["summary"]}))
        })
        .collect();
    let Some(scan) = kept.last().and_then(|(_, p)| read_json(p)) else {
        return Ok(
            json!({"state": "none", "settings": settings, "history": history,
                         "volumes": [volume_now(Path::new("/"), &settings)]}),
        );
    };
    Ok(with_live(scan, &settings, history))
}

fn with_live(mut scan: Value, settings: &Value, history: Vec<Value>) -> Value {
    let mut volumes: BTreeMap<String, Value> = BTreeMap::new();
    let internal = volume_now(Path::new("/"), settings);
    volumes.insert(
        internal["mount"].as_str().unwrap_or("/").to_owned(),
        internal,
    );
    for p in scan["volume_paths"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let v = volume_now(Path::new(p), settings);
        volumes
            .entry(v["mount"].as_str().unwrap_or("/").to_owned())
            .or_insert(v);
    }
    let rows: Vec<Value> = scan["findings"].as_array().cloned().unwrap_or_default();
    for v in volumes.values_mut() {
        let mount = v["mount"].clone();
        let here: Vec<&Value> = rows.iter().filter(|r| r["volume"] == mount).collect();
        v["accumulated"] = summary(&here);
    }
    scan["plan"] = plan(&rows);
    scan["volumes"] = json!(volumes.into_values().collect::<Vec<_>>());
    scan["history"] = json!(history);
    scan["settings"] = settings.clone();
    scan["state"] = json!("ok");
    scan
}

/// `storage_accumulations_scan`: one scan, kept as a revision (JSON + NDJSON rows), older ones beyond
/// `KEEP_SCANS` removed. Writes only under `<home>/accumulations/`.
pub fn scan(home: &Path, user_home: &Path, isolated: bool, params: &Value) -> Result<Value> {
    require(
        params
            .as_object()
            .is_some_and(|p| p.keys().all(|k| k == "max_entries")),
        "invalid_params",
        "Only max_entries is accepted",
    )?;
    let settings = settings_of(&Catalog::open(home)?)?;
    let (profiles, _) =
        crate::suite_profiles::load(crate::suite_profiles::file(isolated).as_deref());
    let mut ctx = Context::new(user_home.to_path_buf(), &settings, profiles);
    if let Some(n) = params["max_entries"].as_u64() {
        ctx.max_entries = n.clamp(1, MAX_ENTRIES * 4);
    }
    scan_with(home, &ctx, &settings)
}

pub fn scan_with(home: &Path, ctx: &Context, settings: &Value) -> Result<Value> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let scanned_at = nu9ve_profiles::utc_at(now.as_secs() as i64);
    let stamp: String = scanned_at
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    // Sorted by name = sorted by time: microseconds, then a random tail.
    let scan_id = format!(
        "{}{:06}-{}",
        stamp.to_ascii_lowercase(),
        now.subsec_micros(),
        &nu9ve_json::uid()?[..6]
    );
    let result = inventory(ctx, settings, &scanned_at, &scan_id);
    let dir = scans_dir(home);
    fs::create_dir_all(&dir).map_err(|e| Error::new("internal_error", e.to_string()))?;
    let mut lines = Vec::new();
    for row in result["findings"].as_array().into_iter().flatten() {
        lines.extend(serde_json::to_vec(row).unwrap_or_default());
        lines.push(b'\n');
    }
    let write = |path: PathBuf, bytes: &[u8]| {
        nu9ve_fs::atomic_write(&path, bytes)
            .map_err(|e| Error::new("internal_error", format!("{e:?}")))
    };
    // Rows first: the JSON names a scan only once its rows exist.
    write(dir.join(format!("{scan_id}.ndjson")), &lines)?;
    write(
        dir.join(format!("{scan_id}.json")),
        &serde_json::to_vec(&result).unwrap_or_default(),
    )?;
    let kept = scans(home);
    if kept.len() > KEEP_SCANS {
        for (id, path) in &kept[..kept.len() - KEEP_SCANS] {
            let _ = fs::remove_file(path);
            let _ = fs::remove_file(dir.join(format!("{id}.ndjson")));
        }
    }
    read(home)
}

/// The dataset of the newest scan for `data_catalog` (nu9ve-data-v1): one row per finding, personal.
pub fn dataset(home: &Path) -> Option<Value> {
    let (id, path) = scans(home).pop()?;
    let scan = read_json(&path)?;
    let rows = scans_dir(home).join(format!("{id}.ndjson"));
    let bytes = fs::read(&rows).ok()?;
    let at = scan["scanned_at"].clone();
    Some(json!({
        "id": DATASET, "owner": "nuun", "title": "Acumulados en disco",
        "description": "Builds, dependencias, cachés y respaldos encontrados por nüün; una fila por hallazgo",
        "modality": "table", "privacy": "personal", "storage": "copied", "projects": [],
        "size": {"bytes": bytes.len(), "items": scan["findings"].as_array().map_or(0, Vec::len), "unit": "rows"},
        "hash": {"algorithm": "sha256", "value": nu9ve_json::sha256_hex(&bytes), "of": "original", "verified_at": at},
        "revision": {"id": id, "collected_at": at},
        "state": "ok", "updated_at": at}))
}

/// `data_export` of the dataset: the NDJSON of the newest scan, which is never rewritten.
pub fn export(home: &Path, max_bytes: u64) -> Option<Value> {
    let (id, _) = scans(home).pop()?;
    let path = scans_dir(home).join(format!("{id}.ndjson"));
    let bytes = fs::read(&path).ok()?;
    if bytes.len() as u64 > max_bytes {
        return Some(
            json!({"contract": "nu9ve-export/1", "dataset": DATASET, "status": "too_large",
                           "bytes": bytes.len(), "where": {"volume_id": null, "path": path}}),
        );
    }
    Some(
        json!({"contract": "nu9ve-export/1", "dataset": DATASET, "status": "ready", "path": path,
                "bytes": bytes.len(), "media_type": "application/x-ndjson",
                "sha256": nu9ve_json::sha256_hex(&bytes)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    /// A fixture home: a Rust repo with target/, a Node app, a venv, pycache, a dist with a .dmg,
    /// a lookalike `target` without signal, a library of a profile, and app backups.
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("user");
        let code = home.join("code");
        let mb = 1024 * 1024;
        touch(&code.join("nove/Cargo.toml"), 10);
        fs::create_dir_all(code.join("nove/.git/objects/pack")).unwrap();
        touch(&code.join("nove/.git/logs/HEAD"), 10);
        fs::write(
            code.join("nove/.git/config"),
            "[core]\n\tbare = false\n[remote \"backup\"]\n\turl = /Volumes/X/nove.git\n[remote \"origin\"]\n\turl = https://someone:tok3n@github.com/acme/nove.git\n",
        )
        .unwrap();
        touch(&code.join("nove/public/favicon.ico"), 10);
        touch(&code.join("nove/target/CACHEDIR.TAG"), 10);
        touch(&code.join("nove/target/release/big"), 3 * mb);
        touch(&code.join("nove/dist/nove-1.dmg"), 2 * mb);
        touch(&code.join("web/package.json"), 10);
        touch(&code.join("web/node_modules/a/index.js"), 2 * mb);
        touch(&code.join("web/.next/cache/x"), 2 * mb);
        touch(&code.join("py/.venv/pyvenv.cfg"), 10);
        touch(&code.join("py/.venv/lib/site.py"), 2 * mb);
        touch(&code.join("py/a/__pycache__/m.pyc"), mb);
        touch(&code.join("py/b/__pycache__/m.pyc"), mb);
        // No signal: a folder called target with media in it is never a finding.
        touch(&code.join("video/target/clip.mov"), 4 * mb);
        // A library of a profile under the code root: live data.
        touch(&code.join("lib-canal-a/node_modules/x"), 4 * mb);
        touch(&code.join("lib-canal-a/package.json"), 10);
        touch(
            &home.join("Library/Application Support/nove-update-backups/2026-11-24/nove.app/bin"),
            2 * mb,
        );
        touch(&home.join(".claude/projects/x/s.jsonl"), 10);
        (dir, home)
    }

    fn profiles(home: &Path) -> Value {
        json!({"profiles": [{"profile_id": "canal-a", "display_name": "Ana",
               "apps": {"nove": {"library": home.join("code/lib-canal-a")}}}]})
    }

    #[test]
    fn rules_are_valid_data() {
        let rules = rules();
        assert!(rules.len() >= 14);
        for r in &rules {
            assert!(
                ["yes", "depends", "partial", "no"].contains(&r["regenerates"].as_str().unwrap()),
                "{r}"
            );
            assert!(
                ["trash", "manual"].contains(&r["action"].as_str().unwrap()),
                "{r}"
            );
            assert!(
                r["confirmations"]
                    .as_u64()
                    .is_some_and(|n| n == 1 || n == 2)
            );
            if r["regenerates"] == "no" {
                assert_eq!(r["confirmations"], 2, "{r}");
            }
        }
    }

    #[test]
    fn the_inventory_recognizes_by_signal_and_keeps_libraries_out() {
        let (_dir, home) = fixture();
        let settings = json!({"internal_min_free_bytes": INTERNAL_MIN_FREE});
        let ctx = Context::new(home.clone(), &json!({}), Some(profiles(&home)));
        assert_eq!(ctx.roots, vec![home.join("code")]);
        let inv = inventory(&ctx, &settings, "2026-11-25T00:00:00Z", "t");
        let rows = inv["findings"].as_array().unwrap();
        let rule_of = |p: &str| {
            rows.iter()
                .find(|r| r["path"].as_str().unwrap().ends_with(p))
                .map(|r| r["rule"].as_str().unwrap().to_owned())
        };
        assert_eq!(rule_of("nove/target").as_deref(), Some("rust-target"));
        assert_eq!(rule_of("nove/dist").as_deref(), Some("packaging"));
        assert_eq!(rule_of("web/node_modules").as_deref(), Some("node-modules"));
        assert_eq!(rule_of("web/.next").as_deref(), Some("next"));
        assert_eq!(rule_of("py/.venv").as_deref(), Some("python-venv"));
        assert_eq!(
            rule_of("2026-11-24").as_deref(),
            Some("nove-update-backups")
        );
        assert_eq!(rule_of("video/target"), None);
        assert!(
            rows.iter()
                .all(|r| !r["path"].as_str().unwrap().contains("lib-canal-a"))
        );
        // __pycache__ of one project is one finding with both folders.
        let pyc: Vec<_> = rows.iter().filter(|r| r["rule"] == "pycache").collect();
        assert_eq!(pyc.len(), 1);
        assert_eq!(pyc[0]["folders"], 2);
        let target = rows.iter().find(|r| r["rule"] == "rust-target").unwrap();
        assert!(target["bytes_on_disk"].as_u64().unwrap() >= 3 * 1024 * 1024);
        assert!(target["repo"].as_str().unwrap().ends_with("code/nove"));
        // U2: `origin`, without the token; and the project's icon.
        assert_eq!(target["repo_remote"], "https://github.com/acme/nove.git");
        assert!(
            target["project_icon"]
                .as_str()
                .unwrap()
                .ends_with("nove/public/favicon.ico")
        );
        assert_eq!(
            without_credentials("git@github.com:acme/nove.git"),
            "git@github.com:acme/nove.git"
        );
        assert!(target["repo_activity_at"].is_string());
        assert!(target["last_used_at"].is_string());
        let backup = rows
            .iter()
            .find(|r| r["rule"] == "nove-update-backups")
            .unwrap();
        assert_eq!(backup["owner_app"], "nove");
        assert_eq!(backup["confirmations"], 2);
        assert_eq!(backup["regenerates"], "no");
        assert_eq!(inv["libraries"][0]["profile_id"], "canal-a");
        let plan = plan(rows);
        assert_eq!(plan["automatic"], false);
        assert!(plan["trash_bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn hard_links_count_once_and_links_are_not_followed() {
        let (_dir, home) = fixture();
        let target = home.join("code/nove/target");
        fs::hard_link(target.join("release/big"), target.join("release/big2")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(home.join("code/video"), target.join("video-link")).unwrap();
        let ctx = Context::new(home.clone(), &json!({}), None);
        let inv = inventory(&ctx, &json!({}), "2026-11-25T00:00:00Z", "t");
        let row = inv["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["rule"] == "rust-target")
            .unwrap();
        let bytes = row["bytes_on_disk"].as_u64().unwrap();
        assert!(bytes < 4 * 1024 * 1024, "{bytes}");
    }

    #[test]
    fn scans_are_revisions_and_the_dataset_exports_the_newest() {
        let (dir, home) = fixture();
        let nuun = dir.path().canonicalize().unwrap().join("media");
        crate::catalog::fixture::create(&nuun);
        let settings = settings_of(&Catalog::open(&nuun).unwrap()).unwrap();
        let ctx = Context::new(home.clone(), &settings, None);
        let first = scan_with(&nuun, &ctx, &settings).unwrap();
        assert_eq!(first["state"], "ok");
        assert!(
            first["volumes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["internal"] == true)
        );
        touch(&home.join("code/nove/target/release/more"), 2 * 1024 * 1024);
        let second = scan_with(&nuun, &ctx, &settings).unwrap();
        assert_eq!(second["history"].as_array().unwrap().len(), 2);
        assert!(second["summary"]["bytes"].as_u64() > first["summary"]["bytes"].as_u64());
        let d = dataset(&nuun).unwrap();
        assert_eq!(d["revision"]["id"], second["scan_id"]);
        assert_eq!(d["privacy"], "personal");
        let e = export(&nuun, 64 * 1024 * 1024).unwrap();
        assert_eq!(e["status"], "ready");
        assert_eq!(e["sha256"], d["hash"]["value"]);
        let rows = fs::read_to_string(e["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            rows.lines().count() as u64,
            d["size"]["items"].as_u64().unwrap()
        );
        assert_eq!(export(&nuun, 10).unwrap()["status"], "too_large");
        // The suite sees it through data_catalog and data_export (nu9ve-data-v1, nu9ve-export-v1).
        let page = crate::suite_volume::data_catalog(&nuun, &json!({})).unwrap();
        let listed = page["datasets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == DATASET)
            .unwrap();
        assert_eq!(listed["modality"], "table");
        assert!(listed.get("location").is_none());
        let exported =
            crate::suite_volume::data_export(&nuun, &json!({"dataset": DATASET})).unwrap();
        assert_eq!(exported["status"], "ready");
    }

    #[test]
    fn settings_validate_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let nuun = dir.path().canonicalize().unwrap().join("media");
        crate::catalog::fixture::create(&nuun);
        assert_eq!(
            settings(&nuun, &json!({})).unwrap()["internal_min_free_bytes"],
            INTERNAL_MIN_FREE
        );
        assert!(settings(&nuun, &json!({"roots": ["relative"]})).is_err());
        assert!(settings(&nuun, &json!({"other": 1})).is_err());
        let saved = settings(
            &nuun,
            &json!({"internal_min_free_bytes": 5, "roots": ["/tmp/x"]}),
        )
        .unwrap();
        assert_eq!(saved["internal_min_free_bytes"], 5);
        assert_eq!(
            settings(&nuun, &json!({})).unwrap()["roots"],
            json!(["/tmp/x"])
        );
    }
}
