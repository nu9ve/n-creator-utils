use crate::tree::Root;
use nu9ve_json::{Error, Result, require};
use regex::bytes::Regex;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

const LIMIT: u64 = 256 * 1024 * 1024;

fn decode(root: &Root, path: &str) -> Result<Vec<u8>> {
    let mut file = root
        .open_read(path)?
        .ok_or_else(|| Error::new("operation_failed", "Set missing"))?;
    let mut header = [0; 2];
    let n = file.read(&mut header).map_err(io)?;
    if n == 0 {
        return Ok(Vec::new());
    }
    require(
        header == [0x1f, 0x8b],
        "operation_failed",
        format!(
            "Not a gzipped file ({:?})",
            String::from_utf8_lossy(&header[..n])
        ),
    )?;
    let mut child = Command::new("/usr/bin/gzip")
        .arg("-dc")
        .env_remove("GZIP")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(io)?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap().take(LIMIT + 1);
    std::thread::scope(|scope| {
        let feed = scope.spawn(move || {
            stdin.write_all(&header)?;
            std::io::copy(&mut file, &mut stdin).map(|_| ())
        });
        let mut data = Vec::new();
        let read = stdout.read_to_end(&mut data);
        if read.is_err() || data.len() as u64 > LIMIT {
            let _ = child.kill();
        }
        drop(stdout);
        let status = child.wait().map_err(io)?;
        let fed = feed.join().expect("gzip input thread");
        read.map_err(io)?;
        require(
            data.len() as u64 <= LIMIT,
            "invalid_request",
            "Set too large",
        )?;
        require(
            status.success(),
            "operation_failed",
            "Invalid or truncated Ableton gzip",
        )?;
        fed.map_err(io)?;
        Ok(data)
    })
}

fn io(error: std::io::Error) -> Error {
    Error::new("operation_failed", error.to_string())
}

pub(crate) fn references(
    root: &Path,
    absolute_only: bool,
    mut visit: impl FnMut(&str, &str),
) -> Result<(usize, Vec<Value>)> {
    require(
        root.is_dir(),
        "invalid_request",
        format!("Ableton folder not found: {}", root.display()),
    )?;
    let tree = Root::open(root)?;
    let pattern = if absolute_only {
        r#"(?i-u)Value="(/[^"]*\.(?:wav|aif|aiff|mp3|flac|m4a|ogg|alac|mid|midi))""#
    } else {
        r#"(?i-u)Value="([^"]*\.(?:wav|aif|aiff|mp3|flac|m4a|ogg|alac))""#
    };
    let pattern = Regex::new(pattern).expect("Ableton reference pattern");
    let mut dirs = vec![String::new()];
    let mut sets = 0;
    let mut unreadable = Vec::new();
    while let Some(rel) = dirs.pop() {
        let folder = root.join(&rel);
        let entries = std::fs::read_dir(&folder).map_err(io)?;
        let mut subdirs = Vec::new();
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::new("unsafe_path", "Invalid UTF-8 filename"))?;
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if entry.file_type().map_err(io)?.is_dir() {
                if !name.starts_with('.') {
                    subdirs.push(path);
                }
            } else if name.to_ascii_lowercase().ends_with(".als") && !name.starts_with("._") {
                files.push(path);
            }
        }
        for path in files {
            match decode(&tree, &path) {
                Ok(bytes) => {
                    sets += 1;
                    for found in pattern.captures_iter(&bytes) {
                        visit(&path, &String::from_utf8_lossy(&found[1]));
                    }
                }
                Err(error) => unreadable.push(json!({
                    "path": if absolute_only { json!(path) } else { json!(root.join(path)) },
                    "error":error.message.chars().take(200).collect::<String>(),
                })),
            }
        }
        dirs.extend(subdirs.into_iter().rev());
    }
    Ok((sets, unreadable))
}
