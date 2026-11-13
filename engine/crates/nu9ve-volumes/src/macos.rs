//! macOS volumes from the same sources a Python `diskutil`/`df` reader uses, so identities match exactly:
//! `statfs` for the mount of a path (what `df -P` prints), `statvfs` for space (what `shutil.disk_usage` reads)
//! and `diskutil … -plist` for identity, parsed from its XML property list into JSON.

use serde_json::{Map, Number, Value};
use std::ffi::{CStr, CString};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The mount point of the file system holding `path` (`statfs` `f_mntonname`; `df -P <path>`'s last column).
pub fn mount_point(path: &Path) -> std::io::Result<String> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut info: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path and `info` a zeroed statfs the call fills.
    if unsafe { libc::statfs(c.as_ptr(), &mut info) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the kernel NUL-terminates f_mntonname inside its fixed buffer.
    let name = unsafe { CStr::from_ptr(info.f_mntonname.as_ptr()) };
    Ok(name.to_string_lossy().into_owned())
}

/// `shutil.disk_usage(path)`: `(total, used, free)` bytes, free being what an unprivileged user may write.
pub fn disk_usage(path: &Path) -> std::io::Result<(u64, u64, u64)> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: as above, with statvfs.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let frsize = s.f_frsize as u64;
    let total = s.f_blocks as u64 * frsize;
    let free = s.f_bavail as u64 * frsize;
    let used = (s.f_blocks as u64).saturating_sub(s.f_bfree as u64) * frsize;
    Ok((total, used, free))
}

/// How a `diskutil` call ended without an answer.
#[derive(Debug)]
pub enum Failure {
    /// It did not answer in time (freshly mounted cards can stall it while Spotlight indexes them).
    Timeout,
    /// It answered with a non-zero status.
    Status,
    /// It could not start, or its output was not a property list.
    Other(String),
}

/// `/usr/sbin/diskutil <args> -plist` with a time limit, as JSON.
pub fn diskutil(args: &[&str], timeout: Duration) -> Result<Value, Failure> {
    plist_command("/usr/sbin/diskutil", args, timeout)
}

/// Ask macOS to eject the currently identified mount, without forcing it.
pub fn eject(mount: &str) -> Result<(), Failure> {
    let mut child = Command::new("/usr/sbin/diskutil")
        .args(["eject", mount])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Failure::Other(e.to_string()))?;
    wait(&mut child, Duration::from_secs(30))
}

/// A system tool that answers a property list (`diskutil`, `hdiutil info -plist`), with a time limit, as JSON.
pub fn plist_command(executable: &str, args: &[&str], timeout: Duration) -> Result<Value, Failure> {
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Failure::Other(e.to_string()))?;
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut buffer);
        }
        buffer
    });
    wait(&mut child, timeout)?;
    let output = reader.join().unwrap_or_default();
    plist(&output).ok_or_else(|| Failure::Other("Invalid property list".into()))
}

fn wait(child: &mut std::process::Child, timeout: Duration) -> Result<(), Failure> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(Failure::Status),
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(2)),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match outcome {
                    Err(error) => Failure::Other(error.to_string()),
                    _ => Failure::Timeout,
                });
            }
        }
    }
}

enum Token {
    Open(String),
    Close,
    Empty(String),
    Text(String),
}

fn tokens(bytes: &[u8]) -> Option<Vec<Token>> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    let mut out = Vec::new();
    let mut text: Option<String> = None;
    loop {
        let event = reader.read_event_into(&mut buffer).ok()?;
        match event {
            Event::Text(t) => text.get_or_insert_with(String::new).push_str({
                let raw: &str = &t;
                raw
            }),
            Event::CData(t) => text.get_or_insert_with(String::new).push_str({
                let raw: &str = &t;
                raw
            }),
            Event::GeneralRef(r) => {
                let name: &str = &r;
                let entity = format!("&{name};");
                let resolved = quick_xml::escape::unescape(&entity).ok()?.into_owned();
                text.get_or_insert_with(String::new).push_str(&resolved);
            }
            other => {
                if let Some(t) = text.take() {
                    out.push(Token::Text(t));
                }
                match other {
                    Event::Start(e) => out.push(Token::Open(e.name().as_ref().to_owned())),
                    Event::Empty(e) => out.push(Token::Empty(e.name().as_ref().to_owned())),
                    Event::End(_) => out.push(Token::Close),
                    Event::Eof => return Some(out),
                    _ => {}
                }
            }
        }
        buffer.clear();
    }
}

fn skip_space(tokens: &[Token], i: &mut usize) {
    while matches!(tokens.get(*i), Some(Token::Text(t)) if t.trim().is_empty()) {
        *i += 1;
    }
}

/// The text inside an element whose `Open` was just consumed, through its `Close`.
fn inner_text(tokens: &[Token], i: &mut usize) -> Option<String> {
    let mut text = String::new();
    loop {
        match tokens.get(*i)? {
            Token::Text(t) => text.push_str(t),
            Token::Close => {
                *i += 1;
                return Some(text);
            }
            _ => return None,
        }
        *i += 1;
    }
}

fn value(tokens: &[Token], i: &mut usize) -> Option<Value> {
    skip_space(tokens, i);
    match tokens.get(*i)? {
        Token::Empty(name) => {
            *i += 1;
            Some(match name.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "dict" => Value::Object(Map::new()),
                "array" => Value::Array(Vec::new()),
                "string" | "data" | "date" => Value::String(String::new()),
                _ => return None,
            })
        }
        Token::Open(name) => {
            let name = name.clone();
            *i += 1;
            match name.as_str() {
                "plist" => {
                    let inner = value(tokens, i)?;
                    skip_space(tokens, i);
                    matches!(tokens.get(*i), Some(Token::Close)).then(|| *i += 1)?;
                    Some(inner)
                }
                "dict" => {
                    let mut map = Map::new();
                    loop {
                        skip_space(tokens, i);
                        match tokens.get(*i)? {
                            Token::Close => {
                                *i += 1;
                                return Some(Value::Object(map));
                            }
                            Token::Open(k) if k == "key" => {
                                *i += 1;
                                let key = inner_text(tokens, i)?;
                                let item = value(tokens, i)?;
                                map.insert(key, item);
                            }
                            _ => return None,
                        }
                    }
                }
                "array" => {
                    let mut items = Vec::new();
                    loop {
                        skip_space(tokens, i);
                        if matches!(tokens.get(*i)?, Token::Close) {
                            *i += 1;
                            return Some(Value::Array(items));
                        }
                        items.push(value(tokens, i)?);
                    }
                }
                "string" | "data" | "date" => inner_text(tokens, i).map(Value::String),
                "integer" => {
                    let text = inner_text(tokens, i)?;
                    let text = text.trim();
                    text.parse::<i64>()
                        .map(Value::from)
                        .or_else(|_| text.parse::<u64>().map(Value::from))
                        .ok()
                }
                "real" => {
                    let number: f64 = inner_text(tokens, i)?.trim().parse().ok()?;
                    Number::from_f64(number).map(Value::Number)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// An XML property list as JSON (`plistlib.loads` for the types `diskutil` prints).
pub fn plist(bytes: &[u8]) -> Option<Value> {
    let tokens = tokens(bytes)?;
    let mut i = 0;
    let found = value(&tokens, &mut i)?;
    skip_space(&tokens, &mut i);
    (i == tokens.len()).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn property_lists_read_like_plistlib() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>APFSPhysicalStores</key>
	<array>
		<dict><key>APFSPhysicalStore</key><string>disk0s2</string></dict>
	</array>
	<key>Internal</key>
	<false/>
	<key>MountPoint</key>
	<string>/Volumes/A &amp; B</string>
	<key>VolumeAllocationBlockSize</key>
	<integer>131072</integer>
	<key>Ratio</key>
	<real>0.5</real>
	<key>Empty</key>
	<string></string>
	<key>None</key>
	<array/>
</dict>
</plist>
"#;
        assert_eq!(
            plist(xml).unwrap(),
            json!({"APFSPhysicalStores": [{"APFSPhysicalStore": "disk0s2"}], "Internal": false,
                   "MountPoint": "/Volumes/A & B", "VolumeAllocationBlockSize": 131072, "Ratio": 0.5,
                   "Empty": "", "None": []})
        );
        assert!(plist(b"<plist><dict><key>a</key></dict></plist>").is_none());
    }

    #[test]
    fn the_system_volume_has_a_mount_space_and_identity() {
        assert_eq!(mount_point(Path::new("/")).unwrap(), "/");
        let (total, _, free) = disk_usage(Path::new("/")).unwrap();
        assert!(total > 0 && free <= total);
        let info = diskutil(&["info", "-plist", "/"], Duration::from_secs(30)).unwrap();
        assert!(info["VolumeUUID"].is_string(), "{info}");
        assert_eq!(info["MountPoint"], "/");
    }
}
