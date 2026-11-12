//! Volume identity rules and source cards. Port of `media/volumes.py` (`card_identity`, `validate_card`,
//! `independent`, `destination_identities`, `reject_destination`, `check`/`online`, `source_cards`, `card_hint`)
//! and of the `volumes` RPC in `media/service.py`.
//!
//! Divergences, all on purpose:
//! - Windows identity is new (`mode: "windows"`, see nu9ve-volumes); Python has no Windows backend.
//! - A card's `name` is the folder name as in Python, or, for a drive root (which has none), its label or letter.

use crate::catalog::Catalog;
use nu9ve_json::{Error, Result, require};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// What the engine needs from the operating system. Tests use a fixture backend; they never list real mounts.
pub trait Backend: Sync {
    /// Mounted roots that could be cards. Metadata only: never recurses into media.
    fn mounted_roots(&self) -> Vec<PathBuf>;
    /// The identity of the volume at `root` (the `info` dictionary of Python's `describe`).
    fn describe(&self, root: &Path, external_required: bool) -> Result<Value>;
    /// `(used, capacity)` bytes, like `shutil.disk_usage`.
    fn usage(&self, root: &Path) -> Option<(u64, u64)>;
    /// Python `fast_check(root, token)`: the volume described in `token` is still the one at `root`. By default a
    /// full description compared on `volume_uuid`, `device` and `root_inode`.
    fn fast_check(&self, root: &Path, token: &Value) -> Result<Value> {
        let info = self.describe(root, false)?;
        for key in ["volume_uuid", "device", "root_inode"] {
            require(
                info.get(key) == token.get(key),
                "volume_replaced",
                "Volume disconnected or root changed",
            )?;
        }
        Ok(token.clone())
    }
    /// Whether this backend may move media (plan, apply, resume) and register volumes. A new backend reads first
    /// and opens its writes only after a physical run (docs/release.md).
    fn writes(&self) -> bool {
        true
    }
}

/// The comparable form of a root: drive roots as `E:\`, other paths as written.
pub fn normalize(root: &Path) -> String {
    if nu9ve_volumes::is_drive_root(root) {
        nu9ve_volumes::drive_root_of(root)
            .map(|r| r.display().to_string())
            .unwrap_or_default()
    } else {
        std::path::absolute(root)
            .unwrap_or_else(|_| root.to_path_buf())
            .display()
            .to_string()
    }
}

/// The real volume backend the Rust engine has for this platform, or `None` where volumes are still Python's.
///
/// This is the only place that asks which operating system it is (audit B6): the binary asks for the capability.
/// Windows: `WindowsBackend`. macOS: none yet; `media/volumes.py` (diskutil/DiskArbitration) answers there until
/// the macOS backend lands (brief S16) and is added here. Linux: none (UDisks2 later, docs/platforms/linux.md).
pub fn platform_backend() -> Option<&'static dyn Backend> {
    #[cfg(windows)]
    {
        Some(&WindowsBackend)
    }
    #[cfg(target_os = "macos")]
    {
        Some(&MacBackend)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// The name `engine_info` gives the backend of `platform_backend` (its `mode` in a volume identity).
pub fn platform_backend_name() -> Option<&'static str> {
    if cfg!(windows) {
        Some("windows")
    } else if cfg!(target_os = "macos") {
        Some("macos")
    } else {
        None
    }
}

/// The real backend on macOS: Python's `VolumeBackend.describe` step by step, from the same sources (`statfs` for
/// what `df -P` prints, `diskutil info -plist`, `statvfs`), so a volume Python registered is online here as is.
/// Read only for now (`writes` is false): volumes, source_cards, snapshot and the reads that ask for a disk.
#[cfg(target_os = "macos")]
pub struct MacBackend;

#[cfg(target_os = "macos")]
fn identity_unavailable(message: impl Into<String>) -> Error {
    Error::new("volume_identity_unavailable", message)
}

/// Python `re.sub(r's[0-9]+(?:s[0-9]+)?$', '', store)`: the whole disk of an APFS physical store (`disk0s2` → `disk0`).
pub fn whole_disk(store: &str) -> String {
    fn strip_slice(text: &str) -> Option<&str> {
        let digits = text.trim_end_matches(|c: char| c.is_ascii_digit());
        (digits.len() < text.len()).then_some(())?;
        digits.strip_suffix('s')
    }
    match strip_slice(store) {
        Some(once) => strip_slice(once).unwrap_or(once).to_owned(),
        None => store.to_owned(),
    }
}

#[cfg(target_os = "macos")]
impl Backend for MacBackend {
    /// `diskutil list -plist external`: every mount point in the tree, sorted. Metadata only.
    fn mounted_roots(&self) -> Vec<PathBuf> {
        fn mounts(nodes: &Value, out: &mut BTreeSet<String>) {
            for node in nodes.as_array().into_iter().flatten() {
                if let Some(mount) = node["MountPoint"].as_str().filter(|m| !m.is_empty()) {
                    out.insert(mount.to_owned());
                }
                for key in ["Partitions", "APFSVolumes"] {
                    mounts(&node[key], out);
                }
            }
        }
        let Ok(list) = nu9ve_volumes::macos::diskutil(
            &["list", "-plist", "external"],
            std::time::Duration::from_secs(30),
        ) else {
            return Vec::new();
        };
        let mut found = BTreeSet::new();
        mounts(&list["AllDisksAndPartitions"], &mut found);
        found.into_iter().map(PathBuf::from).collect()
    }

    fn describe(&self, root: &Path, external_required: bool) -> Result<Value> {
        use nu9ve_volumes::macos::{Failure, disk_usage, diskutil, mount_point};
        use std::os::unix::fs::MetadataExt;
        let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
        let link =
            |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
        require(
            root.is_dir() && !link(&root),
            "volume_offline",
            format!("Root unavailable: {}", root.display()),
        )?;
        for path in root.ancestors() {
            require(!link(path), "unsafe_path", "Volume root contains a symlink")?;
        }
        let st =
            std::fs::metadata(&root).map_err(|e| Error::new("operation_failed", e.to_string()))?;
        let mount_path = mount_point(&root)
            .map_err(|_| identity_unavailable("Cannot determine source filesystem"))?;
        let data = diskutil(
            &["info", "-plist", &mount_path],
            std::time::Duration::from_secs(30),
        )
        .map_err(|failure| match failure {
            Failure::Timeout => identity_unavailable(format!(
                "diskutil did not answer for {mount_path}; retry in a moment"
            )),
            _ => identity_unavailable("Cannot establish disk identity"),
        })?;
        let truthy = |v: &Value| match v {
            Value::Null | Value::Bool(false) => false,
            Value::String(s) => !s.is_empty(),
            _ => true,
        };
        let mount = data["MountPoint"].clone();
        require(
            truthy(&mount) && truthy(&data["VolumeUUID"]),
            "volume_identity_unavailable",
            "Missing mounted volume identity",
        )?;
        let external = data["Internal"] == false;
        let mut physical: Vec<String> = data["APFSPhysicalStores"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["APFSPhysicalStore"].as_str().filter(|s| !s.is_empty()))
            .map(whole_disk)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if physical.is_empty() {
            let parent = [&data["ParentWholeDisk"], &data["DeviceIdentifier"]]
                .into_iter()
                .find(|v| truthy(v))
                .and_then(Value::as_str)
                .unwrap_or_default();
            physical.push(parent.to_owned());
        }
        require(
            physical.iter().all(|p| !p.is_empty()),
            "volume_identity_unavailable",
            "Physical disk identity missing",
        )?;
        require(
            !external_required || external,
            "external_required",
            "Use a mounted external volume",
        )?;
        let (_, _, free) =
            disk_usage(&root).map_err(|e| Error::new("operation_failed", e.to_string()))?;
        Ok(json!({
            "mode": "macos",
            "volume_uuid": data["VolumeUUID"],
            "disk_uuid": data.get("DiskUUID").cloned().unwrap_or(Value::Null),
            "mount": mount,
            "external": external,
            "physical": physical.join(","),
            "physical_disks": physical,
            "filesystem": data.get("FilesystemType").cloned().unwrap_or(json!("")),
            "block_size": data.get("VolumeAllocationBlockSize").cloned().unwrap_or(json!(4096)),
            "root_inode": st.ino(),
            // dev_t is a signed 32-bit number on macOS, as Python reports it.
            "device": st.dev() as i32,
            "free": free,
            "removable": truthy(&data["RemovableMedia"]) || truthy(&data["Removable"]),
            "disk_image": data["BusProtocol"] == "Disk Image",
        }))
    }

    fn usage(&self, root: &Path) -> Option<(u64, u64)> {
        nu9ve_volumes::macos::disk_usage(root)
            .ok()
            .map(|(total, used, _)| (used, total))
    }

    /// Python `fast_check`: the root still has the device and inode of the token, without asking diskutil.
    fn fast_check(&self, root: &Path, token: &Value) -> Result<Value> {
        use std::os::unix::fs::MetadataExt;
        let st = std::fs::symlink_metadata(root)
            .map_err(|_| Error::new("volume_replaced", "Volume disconnected or root changed"))?;
        require(
            token["device"] == json!(st.dev() as i32) && token["root_inode"] == json!(st.ino()),
            "volume_replaced",
            "Volume disconnected or root changed",
        )?;
        Ok(token.clone())
    }

    /// Reads only until a physical run with a card and a USB disk on this Mac (docs/release.md).
    fn writes(&self) -> bool {
        false
    }
}

/// The real backend on Windows. It compiles everywhere; only `platform_backend` decides where it is used.
pub struct WindowsBackend;

impl Backend for WindowsBackend {
    fn mounted_roots(&self) -> Vec<PathBuf> {
        use nu9ve_volumes::DriveKind;
        // Network shares and optical drives are never cards, and asking them can block.
        nu9ve_volumes::mounted_roots()
            .into_iter()
            .filter(|root| {
                matches!(
                    nu9ve_volumes::drive_kind(root),
                    DriveKind::Removable | DriveKind::Fixed
                )
            })
            .collect()
    }

    fn describe(&self, root: &Path, external_required: bool) -> Result<Value> {
        require(
            root.is_dir() && !root.is_symlink(),
            "volume_offline",
            format!("Root unavailable: {}", root.display()),
        )?;
        let info = nu9ve_volumes::describe(root)?;
        // Fail closed like Python: no physical identity, no volume.
        require(
            !info.physical_disks.is_empty(),
            "volume_identity_unavailable",
            "Physical disk identity missing",
        )?;
        require(
            !external_required || info.external(),
            "external_required",
            "Use a mounted external volume",
        )?;
        Ok(json!({
            "mode": "windows",
            "volume_uuid": info.volume_guid,
            "disk_uuid": info.serial,
            "mount": info.root.display().to_string(),
            "external": info.external(),
            "physical": info.physical_disks.join(","),
            "physical_disks": info.physical_disks,
            "filesystem": info.filesystem,
            "block_size": info.block_size,
            // The registered folder itself, as Python stats the root it was given: a replaced folder is a new volume.
            "root_inode": nu9ve_volumes::stamp(root).map(|s| s.inode).unwrap_or(info.root_index),
            "device": info.serial_number,
            "free": info.free,
            "removable": info.removable(),
            "disk_image": info.disk_image(),
            "label": info.label,
            "bus": info.bus.id(),
        }))
    }

    fn usage(&self, root: &Path) -> Option<(u64, u64)> {
        nu9ve_volumes::describe(root)
            .ok()
            .map(|i| (i.total.saturating_sub(i.total_free), i.total))
    }

    /// The root folder is still there with the same file index: a quick look between files, as Python stats the
    /// root against its token. A full `check` runs at the start and before receipts and cleanup.
    fn fast_check(&self, root: &Path, token: &Value) -> Result<Value> {
        let same = nu9ve_volumes::fs::stamp_opt(root)?.is_some_and(|s| {
            s.kind == nu9ve_volumes::EntryKind::Dir && Some(s.inode) == token["root_inode"].as_u64()
        });
        require(
            same,
            "volume_replaced",
            "Volume disconnected or root changed",
        )?;
        Ok(token.clone())
    }
}

/// One request's memo: each root is described once (Python `CachedBackend`).
pub struct Cached<'a> {
    inner: &'a dyn Backend,
    described: Mutex<HashMap<(String, bool), Result<Value>>>,
    mounts: Mutex<Option<Vec<PathBuf>>>,
}

impl<'a> Cached<'a> {
    pub fn new(inner: &'a dyn Backend) -> Self {
        Self {
            inner,
            described: Mutex::new(HashMap::new()),
            mounts: Mutex::new(None),
        }
    }
}

impl Backend for Cached<'_> {
    fn mounted_roots(&self) -> Vec<PathBuf> {
        let mut mounts = self.mounts.lock().unwrap_or_else(|e| e.into_inner());
        mounts
            .get_or_insert_with(|| self.inner.mounted_roots())
            .clone()
    }

    fn describe(&self, root: &Path, external_required: bool) -> Result<Value> {
        let key = (normalize(root), external_required);
        if let Some(found) = self
            .described
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
        {
            return found;
        }
        let result = self.inner.describe(root, external_required);
        self.described
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, result.clone());
        result
    }

    fn usage(&self, root: &Path) -> Option<(u64, u64)> {
        self.inner.usage(root)
    }

    fn fast_check(&self, root: &Path, token: &Value) -> Result<Value> {
        self.inner.fast_check(root, token)
    }

    fn writes(&self) -> bool {
        self.inner.writes()
    }
}

impl Cached<'_> {
    /// Describes these roots at the same time (each `diskutil` call takes ~0.1 s; Python asks one after another)
    /// and keeps the answers for the rest of the request.
    pub fn prefetch(&self, roots: &[(PathBuf, bool)]) {
        let pending: Vec<&(PathBuf, bool)> = roots
            .iter()
            .filter(|(root, external)| {
                !self
                    .described
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains_key(&(normalize(root), *external))
            })
            .collect();
        if pending.len() < 2 {
            return;
        }
        let inner = self.inner;
        let answers: Vec<((String, bool), Result<Value>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = pending
                .iter()
                .map(|(root, external)| {
                    scope.spawn(move || {
                        (
                            (normalize(root), *external),
                            inner.describe(root, *external),
                        )
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        let mut described = self.described.lock().unwrap_or_else(|e| e.into_inner());
        for (key, answer) in answers {
            described.entry(key).or_insert(answer);
        }
    }
}

/// The keys that identify a card for one operation (eject, plan).
pub fn card_identity(info: &Value) -> Value {
    let keys = [
        "mode",
        "volume_uuid",
        "disk_uuid",
        "mount",
        "physical",
        "root_inode",
        "device",
    ];
    Value::Object(
        keys.iter()
            .map(|k| {
                (
                    (*k).to_owned(),
                    info.get(*k).cloned().unwrap_or(Value::Null),
                )
            })
            .collect(),
    )
}

pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn validate_card(root: &Path, info: &Value) -> Result<()> {
    let mount_matches =
        info["mount"].as_str().map(|m| normalize(Path::new(m))) == Some(normalize(root));
    require(
        truthy(&info["external"])
            && truthy(&info["removable"])
            && !truthy(&info["disk_image"])
            && mount_matches,
        "not_a_card",
        "Choose the root of a mounted removable SD/camera card",
    )
}

fn disks(info: &Value) -> BTreeSet<String> {
    match info["physical_disks"].as_array() {
        Some(list) if !list.is_empty() => list
            .iter()
            .filter_map(|d| d.as_str().map(str::to_owned))
            .collect(),
        _ => info["physical"]
            .as_str()
            .map(|p| [p.to_owned()].into())
            .unwrap_or_default(),
    }
}

/// A mounted image's bytes live on some other disk; it never counts as a separate replica.
pub fn independent(first: &Value, second: &Value) -> bool {
    if truthy(&first["disk_image"]) || truthy(&second["disk_image"]) {
        return false;
    }
    disks(first).is_disjoint(&disks(second))
}

/// Current identity of a registered volume, or an error when it is offline or was replaced.
pub fn check(backend: &dyn Backend, record: &Value) -> Result<Value> {
    let root = record["root"].as_str().unwrap_or_default();
    let identity = &record["identity"];
    let current = backend.describe(Path::new(root), truthy(&identity["external"]))?;
    for key in ["mode", "volume_uuid", "disk_uuid", "root_inode"] {
        let expected = identity.get(key).unwrap_or(&Value::Null);
        require(
            current.get(key).unwrap_or(&Value::Null) == expected,
            "volume_replaced",
            format!(
                "Registered volume identity changed: {}",
                record["name"].as_str().unwrap_or("")
            ),
        )?;
    }
    Ok(current)
}

pub fn online(backend: &dyn Backend, record: &Value) -> Option<Value> {
    check(backend, record).ok()
}

/// Describes, at the same time, every root this request will ask about: each registered destination (as
/// `check` asks for it) and, with `cards`, each mount. Later calls on `cached` answer from memory.
pub fn prefetch(cached: &Cached, catalog: &Catalog, cards: bool) -> Result<()> {
    let mut roots: Vec<(PathBuf, bool)> = catalog
        .volumes()?
        .iter()
        .map(|record| {
            (
                PathBuf::from(record["root"].as_str().unwrap_or_default()),
                truthy(&record["identity"]["external"]),
            )
        })
        .collect();
    if cards {
        roots.extend(cached.mounted_roots().into_iter().map(|root| (root, false)));
    }
    cached.prefetch(&roots);
    Ok(())
}

/// `str(Path(value).absolute())`: joined to the working folder, empty and `.` parts dropped, `..` kept.
fn python_absolute(value: &str) -> PathBuf {
    let path = std::path::absolute(value).unwrap_or_else(|_| PathBuf::from(value));
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            // The drive (`C:`) and the root stay as they are; `.` parts go, `..` stays (as Python).
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Python `Service.call('suggest')`: whether the app should offer an ingest for `source`. The cheap destination
/// gate runs before anything looks at the source.
pub fn suggest(catalog: &Catalog, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let cached = Cached::new(backend);
    prefetch(&cached, catalog, false)?;
    let records = catalog.volumes()?;
    if !records
        .iter()
        .any(|record| online(&cached, record).is_some())
    {
        return Ok(
            json!({"show": false, "reason": "no_registered_destinations", "pending_work": []}),
        );
    }
    let pending = crate::reads::library(catalog, &json!({"work_state": "pending"}))?;
    let source = match params.get("source") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => return Ok(json!({"show": false, "reason": "no_source", "pending_work": pending})),
    };
    let root = python_absolute(&source);
    let destination =
        json!({"show": false, "reason": "destination_volume", "pending_work": pending});
    if records.iter().any(|record| {
        let registered = python_absolute(record["root"].as_str().unwrap_or_default());
        root.starts_with(&registered)
    }) {
        return Ok(destination);
    }
    let info = cached.describe(&root, false)?;
    if records
        .iter()
        .any(|record| record["identity"]["volume_uuid"] == info["volume_uuid"])
    {
        return Ok(destination);
    }
    let is_card = info["external"] == true
        && info["mount"] == json!(root.display().to_string())
        && info["removable"] == true;
    Ok(
        json!({"show": is_card, "reason": if is_card { "card" } else { "not_a_card" }, "pending_work": pending}),
    )
}

/// Disk numbers change on remount: compare current physical stores when available.
pub fn destination_identities(catalog: &Catalog, backend: &dyn Backend) -> Result<Vec<Value>> {
    Ok(catalog
        .volumes()?
        .iter()
        .map(|record| online(backend, record).unwrap_or_else(|| record["identity"].clone()))
        .collect())
}

pub fn reject_destination(info: &Value, identities: &[Value]) -> Result<()> {
    require(
        identities.iter().all(|identity| {
            info.get("volume_uuid") != identity.get("volume_uuid") && independent(info, identity)
        }),
        "destination_volume",
        "Archive/work disks cannot be ejected from Ingreso",
    )
}

/// The `volumes` RPC: each registered record with `online`.
pub fn volumes(catalog: &Catalog, backend: &dyn Backend) -> Result<Value> {
    let cached = Cached::new(backend);
    prefetch(&cached, catalog, false)?;
    Ok(Value::Array(
        catalog
            .volumes()?
            .into_iter()
            .map(|record| {
                let up = online(&cached, &record).is_some();
                let mut record = record;
                if let Some(map) = record.as_object_mut() {
                    map.insert("online".into(), Value::Bool(up));
                }
                record
            })
            .collect(),
    ))
}

/// Family, recommendation, usage and last project of a mounted card (volumes.py `card_hint`).
pub fn card_hint(
    root: &Path,
    catalog: &Catalog,
    backend: &dyn Backend,
    identity: Option<&Value>,
) -> Result<Value> {
    let recipe = nuun_recipe::effective(catalog.get("recipe")?.as_ref());
    let rule = nuun_recipe::card_rule(root, &recipe)?;
    let (used, capacity) = backend.usage(root).unwrap_or((0, 0));
    let mut hint = Map::new();
    hint.insert("family".into(), rule["family"].clone());
    hint.insert("recommendation".into(), rule["recommendation"].clone());
    hint.insert("used_bytes".into(), json!(used));
    hint.insert("capacity_bytes".into(), json!(capacity));
    let registry = catalog.get("device-registry")?;
    if let Some(device) = crate::devices::card_device(root, registry.as_ref(), &recipe, 64)? {
        if device["category"].as_str().is_some_and(|c| !c.is_empty())
            && device["requires_confirmation"] != true
        {
            // A known device needs no category question; a mission flow separates its stills.
            let next = if device.get("mission_flow").is_some() {
                "drone_missions"
            } else {
                "archive_and_work"
            };
            hint.insert("recommendation".into(), json!(next));
        }
        hint.insert("device".into(), device);
    }
    if let Some(uuid) = identity
        .and_then(|i| i["volume_uuid"].as_str())
        .filter(|u| !u.is_empty())
        && let Some((project, profile)) = catalog.last_project(uuid)?
    {
        hint.insert("last_project".into(), json!(project));
        hint.insert("last_profile".into(), json!(profile));
    }
    Ok(Value::Object(hint))
}

/// The `source_cards` RPC. Without a registered destination online, mounts stay silent: cards are listed without
/// reading their contents (`hint`), as in Python.
pub fn source_cards(
    catalog: &Catalog,
    backend: &dyn Backend,
    hints: Option<bool>,
) -> Result<Value> {
    let cached = Cached::new(backend);
    prefetch(&cached, catalog, true)?;
    let protected = destination_identities(catalog, &cached)?;
    let hints = match hints {
        Some(h) => h,
        None => catalog
            .volumes()?
            .iter()
            .any(|record| online(&cached, record).is_some()),
    };
    let mut cards = Vec::new();
    for root in cached.mounted_roots() {
        let Ok(info) = cached
            .describe(&root, false)
            .and_then(|info| validate_card(&root, &info).map(|_| info))
            .and_then(|info| reject_destination(&info, &protected).map(|_| info))
        else {
            continue;
        };
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .or_else(|| {
                info["label"]
                    .as_str()
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| normalize(&root).trim_end_matches('\\').to_owned());
        let identity = card_identity(&info);
        let mut card = json!({"path": root.display().to_string(), "name": name, "kind": "card", "identity": identity});
        if hints {
            card["hint"] = card_hint(&root, catalog, &cached, Some(&identity))?;
        }
        cards.push(card);
    }
    Ok(Value::Array(cards))
}

fn expand_user(root: &str) -> PathBuf {
    match root.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(root),
    }
}

/// The `register_volume` method (media/service.py): describe the root (external unless `allow_local`), take the
/// writer lock, then `Catalog.register`. Configuration only: no file on the volume is read or written.
pub fn register_volume(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    let text = |key: &str| -> Result<String> {
        params[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::new("operation_failed", format!("'{key}'")))
    };
    let (name, role, root) = (text("name")?, text("role")?, text("root")?);
    let allow_local = params["allow_local"].as_bool().unwrap_or(false);
    let root = PathBuf::from(normalize(&expand_user(&root)));
    let info = backend.describe(&root, !allow_local)?;
    let prefix = params["prefix"]
        .as_str()
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if role == "archive" {
                "001_raw"
            } else {
                "001_proyectos"
            }
            .to_owned()
        });
    let _lock = crate::catalog::writer_lock(home)?;
    let catalog = crate::catalog::WritableCatalog::open(home)?;
    catalog.register(&name, &role, &root.display().to_string(), &info, &prefix)
}

/// Not a card, a destination or offline: why, in words (for tests and diagnosis).
pub fn explain(error: &Error) -> String {
    format!("{}: {}", error.code, error.message)
}

#[cfg(test)]
pub mod test_backend {
    use super::*;
    use crate::catalog::fixture;
    use std::collections::HashSet;

    /// Port of tests/test_media_engine.py `FixtureVolumes`: temporary folders that behave like mounted volumes.
    #[derive(Default)]
    pub struct Fixture {
        pub roots: Vec<PathBuf>,
        pub offline: HashSet<String>,
        pub physical: HashMap<String, String>,
        pub replaced: HashSet<String>,
        pub overrides: HashMap<String, Value>,
    }

    impl Backend for Fixture {
        fn mounted_roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
        fn describe(&self, root: &Path, _: bool) -> Result<Value> {
            let key = normalize(root);
            if self.offline.contains(&key) || !root.is_dir() {
                return Err(Error::new("volume_offline", "Fixture disk disconnected"));
            }
            let mut info = json!({"mode": "fixture",
                "volume_uuid": format!("{key}{}", if self.replaced.contains(&key) { "-replaced" } else { "" }),
                "disk_uuid": "fixture", "mount": key, "external": true,
                "physical": self.physical.get(&key).cloned().unwrap_or_else(|| key.clone()),
                "filesystem": "fixture", "block_size": 4096, "root_inode": 1, "device": 1, "free": 1_000_000_000_000u64,
                "removable": true});
            if let Some(Value::Object(changes)) = self.overrides.get(&key) {
                for (k, v) in changes {
                    info[k] = v.clone();
                }
            }
            Ok(info)
        }
        fn usage(&self, _: &Path) -> Option<(u64, u64)> {
            Some((21_400_000_000, 64_000_000_000))
        }
    }

    pub struct Setup {
        pub _temp: tempfile::TempDir,
        pub home: PathBuf,
        pub card: PathBuf,
        pub archive: PathBuf,
        pub work: PathBuf,
        pub backend: Fixture,
    }

    /// MediaFixture.setUp: a card, MEDIA (archive) and ESSD (work) registered with their fixture identities.
    pub fn setup() -> Setup {
        let temp = tempfile::tempdir().unwrap();
        // macOS temporary folders sit under /var, a symlink the scan refuses (Python's tests resolve it too).
        #[cfg(unix)]
        let root = std::fs::canonicalize(temp.path()).unwrap();
        #[cfg(not(unix))]
        let root = temp.path().to_path_buf();
        let (card, archive, work) = (root.join("card"), root.join("archive"), root.join("work"));
        for path in [&card, &archive, &work] {
            std::fs::create_dir_all(path).unwrap();
        }
        let backend = Fixture {
            roots: vec![card.clone(), archive.clone(), work.clone()],
            ..Default::default()
        };
        let home = root.join("state");
        let db = fixture::create(&home);
        fixture::register(
            &db,
            "MEDIA",
            "archive",
            &archive,
            &backend.describe(&archive, false).unwrap(),
            "001_raw",
        );
        fixture::register(
            &db,
            "ESSD",
            "work",
            &work,
            &backend.describe(&work, false).unwrap(),
            "001_proyectos",
        );
        Setup {
            _temp: temp,
            home,
            card,
            archive,
            work,
            backend,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_backend::{Fixture, Setup, setup};
    use super::*;
    use crate::catalog::fixture;

    fn catalog(s: &Setup) -> Catalog {
        Catalog::open(&s.home).unwrap()
    }

    fn paths(cards: &Value) -> Vec<String> {
        cards
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["path"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn listing_is_metadata_only_and_excludes_destinations() {
        let s = setup();
        let cards = source_cards(&catalog(&s), &s.backend, None).unwrap();
        assert_eq!(paths(&cards), [s.card.display().to_string()]);
        assert_eq!(cards[0]["name"], "card");
        assert_eq!(cards[0]["identity"]["mode"], "fixture");
    }

    #[test]
    fn a_card_on_the_same_physical_disk_as_a_destination_is_rejected() {
        let mut s = setup();
        s.backend
            .physical
            .insert(normalize(&s.card), normalize(&s.archive));
        assert_eq!(
            source_cards(&catalog(&s), &s.backend, None).unwrap(),
            json!([])
        );
    }

    #[test]
    fn non_cards_and_images_are_rejected() {
        for change in [
            json!({"removable": false}),
            json!({"external": false}),
            json!({"disk_image": true}),
            json!({"mount": "/different-mount"}),
        ] {
            let mut s = setup();
            s.backend
                .overrides
                .insert(normalize(&s.card), change.clone());
            assert_eq!(
                source_cards(&catalog(&s), &s.backend, None).unwrap(),
                json!([]),
                "{change}"
            );
        }
    }

    #[test]
    fn hints_read_the_family_when_a_destination_is_online() {
        let s = setup();
        std::fs::create_dir_all(s.card.join("DCIM/100GOPRO")).unwrap();
        let hint = &source_cards(&catalog(&s), &s.backend, None).unwrap()[0]["hint"];
        assert_eq!(hint["family"], "gopro");
        assert_eq!(hint["recommendation"], "archive_and_work");
        assert_eq!(hint["capacity_bytes"], 64_000_000_000u64);
        assert!(hint.get("last_project").is_none());
    }

    #[test]
    fn a_known_drone_still_changes_the_recommendation() {
        let s = setup();
        let folder = s.card.join("DCIM/100MEDIA");
        std::fs::create_dir_all(&folder).unwrap();
        let jpg = crate::photo_meta::sample::jpeg("DJI", "FC3170", "2026:05:14 10:20:30", None);
        std::fs::write(folder.join("DJI_0001.JPG"), jpg).unwrap();
        let hint = &source_cards(&catalog(&s), &s.backend, None).unwrap()[0]["hint"];
        assert_eq!(
            hint["family"], "generic",
            "100MEDIA without MISC: the family stays generic, the EXIF decides"
        );
        assert_eq!(hint["device"]["label"], "DJI Mavic Air 2");
        assert_eq!(hint["recommendation"], "drone_missions");
    }

    #[test]
    fn offline_destinations_keep_cards_without_content_hints() {
        let mut s = setup();
        std::fs::create_dir_all(s.card.join("DCIM/100GOPRO")).unwrap();
        s.backend.offline.insert(normalize(&s.archive));
        s.backend.offline.insert(normalize(&s.work));
        let cards = source_cards(&catalog(&s), &s.backend, None).unwrap();
        assert_eq!(cards.as_array().unwrap().len(), 1);
        assert!(cards[0].get("hint").is_none());
        // Without hints requested, never the contents.
        let s = setup();
        assert!(
            source_cards(&catalog(&s), &s.backend, Some(false)).unwrap()[0]
                .get("hint")
                .is_none()
        );
    }

    #[test]
    fn volumes_say_online_and_a_replaced_disk_is_offline() {
        let mut s = setup();
        let list = volumes(&catalog(&s), &s.backend).unwrap();
        assert_eq!(
            list.as_array()
                .unwrap()
                .iter()
                .map(|v| (v["name"].as_str().unwrap(), v["online"].as_bool().unwrap()))
                .collect::<Vec<_>>(),
            [("ESSD", true), ("MEDIA", true)]
        );
        s.backend.replaced.insert(normalize(&s.work));
        let list = volumes(&catalog(&s), &s.backend).unwrap();
        assert_eq!(list[0]["online"], false);
        assert_eq!(
            check(&s.backend, &list[0]).unwrap_err().code,
            "volume_replaced"
        );
    }

    #[test]
    fn last_project_comes_from_the_newest_plan_of_that_card() {
        let s = setup();
        let db = rusqlite::Connection::open(s.home.join("catalog.sqlite3")).unwrap();
        let uuid = normalize(&s.card);
        let plan = |project: &str| {
            json!({"source_identity": {"volume_uuid": uuid}, "units": [{"project": "support", "support": true}, {"project": project, "profile": "e64"}]}).to_string()
        };
        db.execute(
            "INSERT INTO jobs VALUES('j1','d',?,'completed',NULL,'2026-11-01','2026-11-01')",
            [plan("viejo-demo")],
        )
        .unwrap();
        db.execute(
            "INSERT INTO jobs VALUES('j2','d',?,'completed',NULL,'2026-11-08','2026-11-08')",
            [plan("nuevo-demo")],
        )
        .unwrap();
        drop(db);
        let hint = &source_cards(&catalog(&s), &s.backend, None).unwrap()[0]["hint"];
        assert_eq!(
            (hint["last_project"].as_str(), hint["last_profile"].as_str()),
            (Some("nuevo-demo"), Some("e64"))
        );
    }

    #[test]
    fn a_saved_recipe_changes_what_a_card_is() {
        let s = setup();
        std::fs::create_dir_all(s.card.join("BlackmagicPocket")).unwrap();
        let mut saved = nuun_recipe::template();
        saved["cards"].as_array_mut().unwrap().insert(0, json!({"family": "blackmagic", "label": "Blackmagic", "recommendation": "archive_and_work", "any": [{"top_prefix": ["blackmagic"]}]}));
        let db = rusqlite::Connection::open(s.home.join("catalog.sqlite3")).unwrap();
        db.execute(
            "INSERT INTO metadata VALUES('recipe',?)",
            [saved.to_string()],
        )
        .unwrap();
        drop(db);
        assert_eq!(
            source_cards(&catalog(&s), &s.backend, None).unwrap()[0]["hint"]["family"],
            "blackmagic"
        );
    }

    #[test]
    fn missing_catalog_is_empty_and_an_unknown_version_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let empty = Catalog::open(temp.path()).unwrap();
        assert!(empty.volumes().unwrap().is_empty());
        assert!(
            !temp.path().join("catalog.sqlite3").exists(),
            "reading never creates the catalog"
        );
        let db = fixture::create(temp.path());
        db.execute("UPDATE metadata SET value='2' WHERE key='schema'", [])
            .unwrap();
        drop(db);
        assert_eq!(
            Catalog::open(temp.path()).err().unwrap().code,
            "unsupported_catalog"
        );
    }

    #[test]
    fn independence_follows_physical_disks_and_images() {
        let a = json!({"physical_disks": ["PhysicalDrive1"], "physical": "PhysicalDrive1"});
        let b = json!({"physical_disks": ["PhysicalDrive2"]});
        let same = json!({"physical": "PhysicalDrive1"});
        assert!(independent(&a, &b));
        assert!(!independent(&a, &same));
        assert!(!independent(
            &a,
            &json!({"physical_disks": ["PhysicalDrive9"], "disk_image": true})
        ));
    }

    #[test]
    fn register_creates_a_python_catalog_and_the_card_gets_hints() {
        let s = setup();
        let home = s._temp.path().join("fresh-state");
        let media = s._temp.path().join("media2");
        std::fs::create_dir_all(&media).unwrap();
        let params =
            json!({"name": "MEDIA2", "role": "archive", "root": media.display().to_string()});
        let record = register_volume(&home, &s.backend, &params).unwrap();
        assert_eq!(record["prefix"], "001_raw");
        assert_eq!(record["identity"]["mode"], "fixture");
        assert_eq!(record["id"].as_str().unwrap().len(), 36);
        // The catalog has Python's schema: every table, version 1, WAL.
        let db = rusqlite::Connection::open(home.join("catalog.sqlite3")).unwrap();
        let tables: Vec<String> = db
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            tables,
            [
                "asset_jobs",
                "asset_projects",
                "asset_revisions",
                "assets",
                "copies",
                "events",
                "jobs",
                "members",
                "metadata",
                "project_aliases",
                "projects",
                "publications",
                "sessions",
                "volumes",
                "work"
            ]
        );
        let mode: String = db
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        drop(db);
        // A registered destination online: the card is now read for hints.
        std::fs::create_dir_all(s.card.join("PRIVATE/M4ROOT/CLIP")).unwrap();
        std::fs::write(s.card.join("PRIVATE/M4ROOT/CLIP/C0001.MP4"), b"1").unwrap();
        let fixture = Fixture {
            roots: vec![s.card.clone(), media.clone()],
            ..Default::default()
        };
        let cards = source_cards(&Catalog::open(&home).unwrap(), &fixture, None).unwrap();
        assert_eq!(paths(&cards), [s.card.display().to_string()]);
        assert_eq!(cards[0]["hint"]["family"], "sony");
    }

    #[test]
    fn register_refuses_like_python() {
        let s = setup();
        let reg = |params: Value| {
            register_volume(&s.home, &s.backend, &params)
                .unwrap_err()
                .code
        };
        let root = s.card.display().to_string();
        assert_eq!(
            reg(json!({"name": "MEDIA", "role": "archive", "root": root})),
            "volume_exists"
        );
        assert_eq!(
            reg(json!({"name": "X", "role": "archive", "root": s.archive.display().to_string()})),
            "volume_exists"
        );
        assert_eq!(
            reg(json!({"name": "X", "role": "backup", "root": root})),
            "invalid_role"
        );
        assert_eq!(
            reg(json!({"name": "-_-", "role": "work", "root": root})),
            "invalid_tag"
        );
        assert_eq!(
            reg(json!({"name": "X", "role": "work", "root": root, "prefix": "../up"})),
            "unsafe_path"
        );
        assert_eq!(
            reg(json!({"role": "work", "root": root})),
            "operation_failed"
        );
        let missing = s._temp.path().join("not-mounted").display().to_string();
        assert_eq!(
            reg(json!({"name": "X", "role": "work", "root": missing})),
            "volume_offline"
        );
        // Nothing above reached the catalog.
        assert_eq!(Catalog::open(&s.home).unwrap().volumes().unwrap().len(), 2);
    }

    #[test]
    fn the_writer_lock_excludes_a_second_writer() {
        let s = setup();
        let held = crate::catalog::writer_lock(&s.home).unwrap();
        let params = json!({"name": "CARD", "role": "work", "root": s.card.display().to_string()});
        assert_eq!(
            register_volume(&s.home, &s.backend, &params)
                .unwrap_err()
                .code,
            "job_busy"
        );
        drop(held);
        assert!(register_volume(&s.home, &s.backend, &params).is_ok());
    }

    // The real Windows backend, on the temporary folder's volume only: the system disk is never a card.
    #[cfg(windows)]
    #[test]
    fn a_local_folder_registers_only_when_allowed() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("state");
        let target = temp.path().join("destino");
        std::fs::create_dir_all(&target).unwrap();
        let root = target.display().to_string();
        let refused = register_volume(
            &home,
            &WindowsBackend,
            &json!({"name": "LOCAL", "role": "work", "root": root}),
        )
        .unwrap_err();
        assert_eq!(refused.code, "external_required");
        let record = register_volume(
            &home,
            &WindowsBackend,
            &json!({"name": "LOCAL", "role": "work", "root": root, "allow_local": true}),
        )
        .unwrap();
        assert_eq!(record["identity"]["mode"], "windows");
        let listed = volumes(&Catalog::open(&home).unwrap(), &WindowsBackend).unwrap();
        assert_eq!(listed[0]["online"], true);
    }

    #[cfg(windows)]
    #[test]
    fn the_system_volume_has_a_windows_identity_and_is_not_a_card() {
        let folder = tempfile::tempdir().unwrap();
        let root = nu9ve_volumes::drive_root_of(folder.path()).unwrap();
        let info = WindowsBackend.describe(&root, false).unwrap();
        assert_eq!(info["mode"], "windows");
        assert!(
            info["volume_uuid"]
                .as_str()
                .unwrap()
                .starts_with("\\\\?\\Volume{")
        );
        assert!(!info["physical_disks"].as_array().unwrap().is_empty());
        assert_eq!(validate_card(&root, &info).unwrap_err().code, "not_a_card");
        // Registered as a destination, it is online with the same identity and offline once replaced.
        let record = json!({"name": "SYS", "root": root.display().to_string(), "identity": info});
        assert!(online(&WindowsBackend, &record).is_some());
        let mut replaced = record.clone();
        replaced["identity"]["volume_uuid"] =
            json!("\\\\?\\Volume{00000000-0000-0000-0000-000000000000}\\");
        assert_eq!(
            check(&WindowsBackend, &replaced).unwrap_err().code,
            "volume_replaced"
        );
    }
}
