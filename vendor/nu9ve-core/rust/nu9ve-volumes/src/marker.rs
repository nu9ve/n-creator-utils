//! The suite's volume marker, observation states and per-app folders (contracts/nu9ve-volume-v1.md).
//!
//! Pure rules: callers read the bytes with their own path checks (`.nu9ve/volume.json` at the volume root, at
//! most [`MARKER_LIMIT`] bytes) and pass what the system says about the volume. Writing the marker, remembering
//! serials (§4) and asking the person (§6) belong to the caller. Names of apps arrive as parameters.

use serde_json::Value;
use std::collections::BTreeMap;

/// `contract` of every marker (§1).
pub const CONTRACT: &str = "nu9ve-volume/1";
/// The marker, relative to the volume root (§1).
pub const MARKER_PATH: &str = ".nu9ve/volume.json";
/// Largest marker a reader accepts (§1).
pub const MARKER_LIMIT: usize = 16 * 1024;
/// Largest app index a reader accepts (§8).
pub const INDEX_LIMIT: usize = 8 * 1024 * 1024;

/// The roles of §1.
pub const ROLES: [&str; 3] = ["archive", "work", "other"];
/// The systems of `fs.os` (§1).
pub const SYSTEMS: [&str; 3] = ["macos", "windows", "linux"];
/// The file systems of `fs.kind` (§1).
pub const KINDS: [&str; 7] = ["apfs", "hfsplus", "exfat", "fat32", "ntfs", "ext4", "other"];

/// A lowercase UUID 4 with hyphens.
pub fn uuid4(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b[14] == b'4'
        && b"89ab".contains(&b[19])
        && b.iter().enumerate().all(|(i, &c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
            }
        })
}

/// RFC 3339 in UTC with `Z`, optionally with 1 to 9 fraction digits: `2026-11-22T21:00:00Z`.
pub fn utc_time(s: &str) -> bool {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    if b.len() < 20 || b[b.len() - 1] != b'Z' {
        return false;
    }
    let shape = digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'T'
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19);
    let fraction = &b[19..b.len() - 1];
    shape
        && (fraction.is_empty()
            || (fraction[0] == b'.'
                && (2..=10).contains(&fraction.len())
                && fraction[1..].iter().all(u8::is_ascii_digit)))
}

/// 1 to `longest` characters, none of them a control character.
pub fn text(s: &str, longest: usize) -> bool {
    (1..=longest).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}

/// An app's name, as in the registry: `^[a-z][a-z0-9-]{0,31}$`. Also the only folder names `apps/` lists (§8).
pub fn app_name(s: &str) -> bool {
    s.len() <= 32
        && s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The folder an app writes on a volume, relative to its root (§8), or `None` for a name that is not an app's.
pub fn app_folder(app: &str) -> Option<String> {
    app_name(app).then(|| format!(".nu9ve/apps/{app}"))
}

/// The app's index on a volume, relative to its root (§8).
pub fn index_path(app: &str) -> Option<String> {
    app_folder(app).map(|folder| folder + "/index.json")
}

/// Why `bytes` is not a valid marker (§1), as the reference names it (`data/volume-examples-v1.json`), or `None`.
pub fn marker_problem(bytes: &[u8]) -> Option<&'static str> {
    parse_marker(bytes).err()
}

/// The JSON type a required field must have.
type Kind = fn(&Value) -> bool;

/// The marker in `bytes`, or why it is not valid (§1). Unknown fields are kept.
pub fn parse_marker(bytes: &[u8]) -> Result<Value, &'static str> {
    if bytes.len() > MARKER_LIMIT {
        return Err("too_large");
    }
    if bytes.starts_with(b"\xef\xbb\xbf") {
        return Err("bom");
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "not_json")?;
    let Some(object) = value.as_object() else {
        return Err("not_object");
    };
    let kinds: [(&str, Kind); 7] = [
        ("contract", Value::is_string),
        ("volume_id", Value::is_string),
        ("role", Value::is_string),
        ("label", Value::is_string),
        ("registered_at", Value::is_string),
        ("registered_by", Value::is_object),
        ("fs", Value::is_object),
    ];
    for (name, kind) in kinds {
        match object.get(name) {
            None => return Err("missing_field"),
            Some(v) if !kind(v) => return Err("wrong_type"),
            _ => {}
        }
    }
    let field = |name: &str| object[name].as_str().unwrap_or_default();
    if field("contract") != CONTRACT {
        return Err("unknown_contract");
    }
    if !uuid4(field("volume_id")) {
        return Err("bad_volume_id");
    }
    if !ROLES.contains(&field("role")) {
        return Err("bad_role");
    }
    if !text(field("label"), 64) {
        return Err("bad_label");
    }
    if !utc_time(field("registered_at")) {
        return Err("bad_time");
    }
    let by = &object["registered_by"];
    if !(by["device_id"].as_str().is_some_and(uuid4) && by["app"].as_str().is_some_and(app_name)) {
        return Err("bad_registered_by");
    }
    let fs = &object["fs"];
    if !(fs["os"].as_str().is_some_and(|s| SYSTEMS.contains(&s))
        && fs["kind"].as_str().is_some_and(|s| KINDS.contains(&s))
        && fs["serial"].as_str().is_some_and(|s| text(s, 128)))
    {
        return Err("bad_fs");
    }
    if object
        .get("role_changed_at")
        .is_some_and(|v| !v.as_str().is_some_and(utc_time))
    {
        return Err("bad_time");
    }
    if let Some(previous) = object.get("previous_volume_id")
        && !previous
            .as_str()
            .is_some_and(|p| uuid4(p) && p != field("volume_id"))
    {
        return Err("bad_previous_volume_id");
    }
    Ok(value)
}

/// §2: any app registers `work` and `other`; `archive` only the app the suite gives that policy, which the
/// caller says with `may_archive`.
pub fn may_register(role: &str, may_archive: bool) -> bool {
    ROLES.contains(&role) && (role != "archive" || may_archive)
}

/// `fs.kind` for what a system calls its file system (`diskutil`'s `FilesystemType`, Windows' file system name).
pub fn fs_kind(system_name: &str) -> &'static str {
    match system_name.to_ascii_lowercase().as_str() {
        "apfs" => "apfs",
        "hfs" | "hfs+" | "hfsplus" => "hfsplus",
        "exfat" => "exfat",
        "msdos" | "fat32" | "fat" => "fat32",
        "ntfs" => "ntfs",
        "ext4" => "ext4",
        _ => "other",
    }
}

/// `fs.serial` on Windows: `VolumeSerialNumber` as `XXXX-XXXX` in uppercase hexadecimal (§3).
pub fn windows_serial(serial: u32) -> String {
    format!("{:04X}-{:04X}", serial >> 16, serial & 0xffff)
}

/// What an observer knows about one mounted volume (§5).
pub struct Observation<'a> {
    /// The system can read its files (HFS+ on Windows cannot).
    pub readable: bool,
    /// The bytes of `.nu9ve/volume.json`, or `None` when there is none.
    pub marker: Option<&'a [u8]>,
    /// The serial the system gives now (§3), or `None`.
    pub serial: Option<&'a str>,
    /// This observer's system: `macos`, `windows` or `linux`.
    pub os: &'a str,
    /// The `volume_id`s of the other volumes mounted at the same time on this machine.
    pub other_mounted: &'a [String],
    /// The serial this observer remembers for this `volume_id`, by system (§4).
    pub remembered: &'a BTreeMap<String, String>,
}

/// The state of §5, in its order of decision. An observer that gets `ok` with a marker from another system and
/// nothing remembered for its own keeps the serial it saw (§4, step 6).
pub fn state(seen: &Observation) -> &'static str {
    if !seen.readable {
        return "unreadable";
    }
    let Some(bytes) = seen.marker else {
        return "unmarked";
    };
    let Ok(marker) = parse_marker(bytes) else {
        return "invalid";
    };
    let id = marker["volume_id"].as_str().unwrap_or_default();
    if seen.other_mounted.iter().any(|other| other == id) {
        return "duplicate";
    }
    let Some(serial) = seen.serial else {
        return "unverified";
    };
    let expected = match seen.remembered.get(seen.os) {
        Some(known) => Some(known.as_str()),
        None => (marker["fs"]["os"] == seen.os)
            .then(|| marker["fs"]["serial"].as_str().unwrap_or_default()),
    };
    match expected {
        Some(expected) if expected != serial => "suspect",
        _ => "ok",
    }
}

/// Whether a state lets an app use the volume's identity: `ok` and `unverified` (with a warning) do (§5, §6).
pub fn usable(state: &str) -> bool {
    matches!(state, "ok" | "unverified")
}

/// Why an app's index is not acceptable at the level of this contract (§8), or `None`: size, JSON, the folder's
/// app and one document with no `next_cursor`. The caller still validates it as an `nu9ve-data-v1` catalog,
/// whose problems (`bad_root` and the rest) the data contract names.
pub fn index_problem(folder_app: &str, bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() > INDEX_LIMIT {
        return Some("too_large");
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Some("not_json");
    };
    if !value.is_object() {
        return Some("not_object");
    }
    if value["app"] != folder_app {
        return Some("app_mismatch");
    }
    if !value["next_cursor"].is_null() {
        return Some("paged_index");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn examples() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/volume-examples-v1.json"
        );
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn s(v: &Value) -> &str {
        v.as_str().unwrap()
    }

    #[test]
    fn markers_match_the_reference() {
        for case in examples()["markers"].as_array().unwrap() {
            let problem = marker_problem(s(&case["text"]).as_bytes());
            assert_eq!(problem, case["problem"].as_str(), "{}", case["name"]);
        }
    }

    #[test]
    fn registering_matches_the_reference() {
        let examples = examples();
        let cases = examples["registering"].as_array().unwrap();
        // The app with the archive policy, as the examples say it: no app name in shared code.
        let archivist = cases
            .iter()
            .find(|c| c["role"] == "archive" && c["allowed"] == true)
            .map(|c| s(&c["app"]))
            .unwrap();
        for case in cases {
            let allowed = may_register(s(&case["role"]), s(&case["app"]) == archivist);
            assert_eq!(json!(allowed), case["allowed"], "{case}");
        }
    }

    #[test]
    fn states_match_the_reference() {
        for case in examples()["states"].as_array().unwrap() {
            let volume = &case["volume"];
            let others: Vec<String> = case["other_mounted"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| s(v).to_owned())
                .collect();
            let remembered: BTreeMap<String, String> = case["remembered"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), s(v).to_owned()))
                .collect();
            let seen = Observation {
                readable: volume["readable"] == true,
                marker: volume["marker"].as_str().map(str::as_bytes),
                serial: volume["serial"].as_str(),
                os: s(&case["os"]),
                other_mounted: &others,
                remembered: &remembered,
            };
            assert_eq!(state(&seen), s(&case["state"]), "{}", case["name"]);
        }
    }

    #[test]
    fn app_folders_match_the_reference() {
        for case in examples()["app_folders"].as_array().unwrap() {
            assert_eq!(
                json!(app_folder(s(&case["name"])).is_some()),
                case["listed"],
                "{case}"
            );
        }
        assert_eq!(
            index_path("atlas").as_deref(),
            Some(".nu9ve/apps/atlas/index.json")
        );
        assert_eq!(index_path("../x"), None);
    }

    #[test]
    fn index_envelope_matches_the_reference() {
        for case in examples()["indexes"].as_array().unwrap() {
            let problem = index_problem(s(&case["folder"]), s(&case["text"]).as_bytes());
            // `bad_root` and the other catalog problems are nu9ve-data's: this contract checks the envelope.
            let expected = case["problem"]
                .as_str()
                .filter(|p| matches!(*p, "app_mismatch" | "paged_index"));
            assert_eq!(problem, expected, "{}", case["name"]);
        }
        assert_eq!(
            index_problem("atlas", &vec![b' '; INDEX_LIMIT + 1]),
            Some("too_large")
        );
    }

    #[test]
    fn serials_and_kinds() {
        assert_eq!(windows_serial(0x5A1C_93F2), "5A1C-93F2");
        assert_eq!(windows_serial(0x0000_00AB), "0000-00AB");
        assert_eq!(fs_kind("ExFAT"), "exfat");
        assert_eq!(fs_kind("hfs"), "hfsplus");
        assert_eq!(fs_kind("msdos"), "fat32");
        assert_eq!(fs_kind("zfs"), "other");
        assert!(utc_time("2026-11-23T00:56:50.219009Z"));
        assert!(!utc_time("2026-11-23T00:56:50+00:00"));
        assert!(!utc_time("2026-11-23T00:56:50.Z"));
        assert!(usable("unverified") && !usable("suspect"));
    }
}
