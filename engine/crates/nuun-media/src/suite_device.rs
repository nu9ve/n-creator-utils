//! Local installation identity (nu9ve-device/1).
use crate::tree::Root;
use nu9ve_json::{Error, Result, require, sha256_hex, uid};
use nu9ve_volumes::fs;
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) const OS: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(windows) {
    "windows"
} else {
    "linux"
};

pub(crate) fn uuid(value: &Value) -> bool {
    let Some(s) = value.as_str() else {
        return false;
    };
    s.len() == 36
        && s.as_bytes()[14] == b'4'
        && b"89ab".contains(&s.as_bytes()[19])
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                lower_hex(b)
            }
        })
}

fn lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

pub(crate) fn digest(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.len() == 64 && s.bytes().all(lower_hex))
}

pub(crate) fn time(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        s.ends_with('Z')
            && s.as_bytes().get(10) == Some(&b'T')
            && s.as_bytes().get(13) == Some(&b':')
            && s.as_bytes().get(16) == Some(&b':')
            && nu9ve_time::DateTime::fromisoformat(&s[..s.len() - 1]).is_some()
    })
}

pub(crate) fn name(value: &Value, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| (1..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control))
}

pub(crate) fn os(value: &Value) -> bool {
    matches!(value.as_str(), Some("macos" | "windows" | "linux"))
}

fn platform_id() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/sbin/ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|line| {
                let (_, value) = line.split_once("\"IOPlatformUUID\" = ")?;
                Some(value.trim().trim_matches('"').to_owned())
            })
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("reg.exe")
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Cryptography",
                "/v",
                "MachineGuid",
                "/reg:64",
            ])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|line| {
                let (_, value) = line
                    .trim()
                    .strip_prefix("MachineGuid")?
                    .split_once("REG_SZ")?;
                Some(value.trim().to_owned())
            })
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        std::fs::read_to_string("/etc/machine-id")
            .ok()
            .map(|s| s.trim_end_matches('\n').to_owned())
    }
}

fn device_file(isolated: bool) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NU9VE_DEVICE_FILE") {
        let path = PathBuf::from(path);
        return path.is_absolute().then_some(path);
    }
    if isolated
        || std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some()
        || std::env::var_os("PACKAGE_FAMILY_NAME").is_some()
    {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let base = match OS {
        "macos" => {
            let home = home?;
            if home
                .components()
                .any(|part| part.as_os_str() == "Containers")
            {
                return None;
            }
            home.join("Library/Application Support")
        }
        "windows" => PathBuf::from(std::env::var_os("LOCALAPPDATA")?),
        _ => std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|p| p.join(".local/share")))?,
    };
    base.is_absolute().then(|| base.join("nu9ve/device.json"))
}

fn hash(id: &Value, platform: &str) -> String {
    sha256_hex(format!("nu9ve-device/1\n{}\n{platform}", id.as_str().unwrap()).as_bytes())
}

fn read(root: &Root, file: &str) -> Result<Option<Value>> {
    let Some(input) = root.open_read(file)? else {
        return Ok(None);
    };
    let path = root.path.join(file);
    let metadata = input.metadata().map_err(|e| fs::io_error(e, &path))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid reads the calling process's effective user id.
        require(
            metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o077 == 0,
            "invalid_device",
            "device.json has an unexpected owner or permissions",
        )?;
    }
    require(
        metadata.is_file(),
        "invalid_device",
        "device.json is not a regular file",
    )?;
    let mut bytes = Vec::new();
    input
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|e| fs::io_error(e, &path))?;
    let invalid = || Error::new("invalid_device", "Invalid device.json");
    require(
        bytes.len() <= 4096,
        "invalid_device",
        "device.json exceeds 4 KiB",
    )?;
    let device: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    require(
        device["contract"] == "nu9ve-device/1"
            && uuid(&device["device_id"])
            && name(&device["name"], 64)
            && time(&device["created_at"])
            && os(&device["os"])
            && device.get("host_check").is_none_or(digest)
            && device.get("previous_device_id").is_none_or(uuid),
        "invalid_device",
        "Invalid device.json",
    )?;
    Ok(Some(device))
}

fn load(
    path: &Path,
    platform: Option<&str>,
    chosen_name: Option<&str>,
) -> Result<(Value, Option<Value>)> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("device_unavailable", "Missing device directory"))?;
    let file = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::new("device_unavailable", "Invalid device filename"))?;
    require(
        parent.ancestors().all(|dir| !dir.is_symlink()),
        "device_unavailable",
        "Device directory must not contain links",
    )?;
    if !parent.is_dir() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(parent)
            .map_err(|e| fs::io_error(e, parent))?;
    }
    let root = Root::open(parent)?;
    // All updates by nüün share the same lock, even when their media catalogs differ.
    let _lock = crate::catalog::operation_lock(
        parent,
        &format!(".{file}.lock"),
        "device_busy",
        "Another process is updating device.json",
    )?;
    let existing = read(&root, file)?;
    let copied = existing.as_ref().is_some_and(|old| {
        platform.is_some_and(|id| {
            old.get("host_check")
                .is_some_and(|known| *known != hash(&old["device_id"], id))
        })
    });
    let mut device = if !copied && let Some(old) = &existing {
        if chosen_name.is_none() {
            return Ok((old.clone(), None));
        }
        let mut updated = old.clone();
        updated["name"] = json!(chosen_name);
        updated
    } else {
        let id = uid()?;
        let prefix = match OS {
            "macos" => "Mac",
            "windows" => "PC",
            _ => "Linux",
        };
        let mut new = json!({"contract":"nu9ve-device/1", "device_id": id,
            "name": format!("{prefix} {}", &id[..4]), "os": OS, "created_at":nu9ve_time::now_utc()});
        if let Some(old) = &existing {
            new["previous_device_id"] = old["device_id"].clone();
        }
        if let Some(platform) = platform {
            new["host_check"] = json!(hash(&new["device_id"], platform));
        }
        new
    };
    if let Some(chosen) = chosen_name {
        device["name"] = json!(chosen);
    }
    let mut bytes = nu9ve_json::canonical(&device);
    bytes.push(b'\n');
    require(
        bytes.len() <= 4096,
        "invalid_device",
        "device.json exceeds 4 KiB",
    )?;
    let result = root.write_bytes(file, &bytes, existing.is_some());
    if let Some(winner) = read(&root, file)?
        && (result.is_ok() || existing.is_none() && winner["device_id"] != device["device_id"])
    {
        require(
            platform.is_none_or(|id| {
                winner
                    .get("host_check")
                    .is_none_or(|known| *known == hash(&winner["device_id"], id))
            }),
            "copied_device",
            "The concurrently published device belongs to another machine",
        )?;
        if copied && chosen_name.is_none() {
            return Ok((winner, Some(existing.unwrap()["name"].clone())));
        }
        return Ok((winner, None));
    }
    result?;
    Err(Error::new(
        "device_unavailable",
        "Could not read back device.json",
    ))
}

pub fn current(isolated: bool, params: &Value) -> Result<Value> {
    let chosen = params.get("name");
    require(
        chosen.is_none_or(|v| name(v, 64)),
        "invalid_params",
        "Use a device name of 1–64 characters",
    )?;
    // `peek` shows the name without creating device.json: the first registration creates it (device §9.2).
    let peek = chosen.is_none() && params.get("peek") == Some(&json!(true));
    let outcome = device_file(isolated)
        .ok_or_else(|| {
            Error::new(
                "device_unavailable",
                "The shared device file is unavailable",
            )
        })
        .and_then(|path| {
            if peek && !path.exists() {
                return Err(Error::new("missing", "No device.json yet"));
            }
            load(
                &path,
                platform_id().as_deref().filter(|s| !s.is_empty()),
                chosen.and_then(Value::as_str),
            )
        });
    Ok(match outcome {
        Ok((device, suggestion)) => {
            let mut result = json!({"state":"ok", "device":device});
            if let Some(suggestion) = suggestion {
                result["suggested_name"] = suggestion;
            }
            result
        }
        Err(error) => json!({"state":"none", "reason":error.code, "message":error.message}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identity_rename_migration_and_invalid_file() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder
            .path()
            .canonicalize()
            .unwrap()
            .join("suite/device.json");
        let (device, _) = load(&path, Some("first"), None).unwrap();
        assert_eq!(device["host_check"], hash(&device["device_id"], "first"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                path.parent()
                    .unwrap()
                    .metadata()
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        }
        let (renamed, _) = load(&path, Some("first"), Some("M4")).unwrap();
        assert_eq!(renamed["device_id"], device["device_id"]);
        assert_eq!(renamed["created_at"], device["created_at"]);
        let (migrated, suggestion) = load(&path, Some("second"), None).unwrap();
        assert_ne!(migrated["device_id"], device["device_id"]);
        assert_eq!(migrated["previous_device_id"], device["device_id"]);
        assert_eq!(suggestion, Some(json!("M4")));
        let invalid = vec![b' '; 4097];
        std::fs::write(&path, &invalid).unwrap();
        assert_eq!(
            load(&path, Some("second"), None).unwrap_err().code,
            "invalid_device"
        );
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
    }
}
