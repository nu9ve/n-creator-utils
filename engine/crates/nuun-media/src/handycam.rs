use crate::tree::{Root, absent};
use nu9ve_json::{Error, Result, require};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn needs_conversion(path: &str) -> bool {
    path.to_ascii_lowercase().starts_with("mp_root/")
        && path.to_ascii_lowercase().ends_with(".mpg")
        && crate::layout::handycam_sequence(path).is_some()
}

pub(crate) fn is_history_movie(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".mp4") && crate::layout::handycam_sequence(path).is_some()
}

/// Compare only candidates with the same sequence and byte count. Variants get their own path.
pub(crate) fn review_history(
    source: &Path,
    archive: &Value,
    files: &[Value],
    layout: &mut serde_json::Map<String, Value>,
) -> Result<BTreeSet<String>> {
    let source = Root::open(source)?;
    let archive_root = Root::open(Path::new(archive["root"].as_str().unwrap_or_default()))?;
    let prefix = archive["prefix"].as_str().unwrap_or_default();
    let base = archive_root.path.join(prefix).join("handycam");
    let mut existing: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    if base.exists() {
        for file in crate::scan::scan(&base, false, false, false)?.files {
            let path = file["path"].as_str().unwrap_or_default();
            let name = path.rsplit('/').next().unwrap_or_default();
            if let Some(sequence) = name.split('-').find_map(|part| {
                let sequence = part.get(..8)?;
                crate::layout::handycam_sequence(&format!("{sequence}.mp4"))
            }) {
                existing
                    .entry((sequence, file["stamp"]["size"].as_u64().unwrap_or(0)))
                    .or_default()
                    .push(format!("{prefix}/handycam/{path}"));
            }
        }
    }
    let mut selected: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut taken = BTreeSet::new();
    let mut adopted = BTreeSet::new();
    for file in files {
        let path = file["path"].as_str().unwrap_or_default();
        let Some(sequence) = crate::layout::handycam_sequence(path) else {
            continue;
        };
        let key = (sequence, file["stamp"]["size"].as_u64().unwrap_or(0));
        let previous = selected.entry(key.clone()).or_default();
        let archived = existing.get(&key).map_or(&[][..], Vec::as_slice);
        if !previous.is_empty() || !archived.is_empty() {
            let hash = source_hash(&source, path)?;
            let mut matches = false;
            for (tree, candidates) in [(&source, previous.as_slice()), (&archive_root, archived)] {
                for candidate in candidates {
                    let hash_key = tree.path.join(candidate);
                    let candidate_hash = match hashes.get(&hash_key) {
                        Some(hash) => hash,
                        None => hashes
                            .entry(hash_key)
                            .or_insert(source_hash(tree, candidate)?),
                    };
                    if candidate_hash == &hash {
                        matches = true;
                        break;
                    }
                }
                if matches {
                    break;
                }
            }
            hashes.insert(source.path.join(path), hash);
            if matches {
                adopted.insert(path.to_owned());
                layout.remove(path);
                continue;
            }
        }
        previous.push(path.to_owned());
        let initial = layout[path]["archive"].as_str().unwrap_or_default();
        require(
            initial.starts_with("handycam/") && initial.contains("[]"),
            "invalid_plan",
            "Falta el nombre de archivo de la Handycam",
        )?;
        let stem = initial.strip_suffix("[].MP4").unwrap_or(initial);
        let stem = stem
            .rsplit_once('-')
            .filter(|(_, suffix)| suffix.bytes().all(|byte| byte.is_ascii_digit()))
            .map_or(stem, |(name, _)| name);
        let initial = format!("{stem}[].MP4");
        let mut target = initial.to_owned();
        for variant in 2.. {
            if !taken.contains(&target.to_ascii_lowercase())
                && archive_root.stat(&format!("{prefix}/{target}"))?.is_none()
            {
                break;
            }
            target = initial.replace("[]", &format!("-{variant}[]"));
        }
        taken.insert(target.to_ascii_lowercase());
        layout.insert(path.to_owned(), json!({"archive": target, "work": null}));
    }
    Ok(adopted)
}

pub(crate) fn source_hash(root: &Root, path: &str) -> Result<String> {
    let mut file = nu9ve_volumes::fs::verify_read(
        root.open_read(path)?
            .ok_or_else(|| Error::new("source_changed", format!("Falta {path}")))?,
    );
    crate::transfer::hash_file(&mut file, |_| {})
}

fn remove(root: &Root, path: &str) -> Result<()> {
    match root.unlink(path, None) {
        Err(error) if absent(&error) => Ok(()),
        result => result,
    }
}

/// The archive holds at most one pair of transient files while its writer lock is held.
pub(crate) struct Staged<'a> {
    archive: &'a Root,
    input: String,
    pub output: String,
}

impl Drop for Staged<'_> {
    fn drop(&mut self) {
        let _ = remove(self.archive, &self.input);
        let _ = remove(self.archive, &self.output);
    }
}

fn run(command: &mut Command, failure: &str) -> Result<()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| Error::new("handycam_conversion", error.to_string()))?;
    loop {
        #[cfg(target_os = "macos")]
        if crate::interrupt::cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new("cancelled", "Conversión pausada"));
        }
        match child.try_wait() {
            Ok(Some(result)) if result.success() => return Ok(()),
            Ok(Some(_)) => return Err(Error::new("handycam_conversion", failure)),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::new("handycam_conversion", error.to_string()));
            }
        }
    }
}

pub(crate) fn stage<'a>(source: &Root, archive: &'a Root, file: &Value) -> Result<Staged<'a>> {
    let path = file["path"].as_str().unwrap_or_default();
    let original = file["handycam_original_sha256"]
        .as_str()
        .ok_or_else(|| Error::new("invalid_plan", "Falta el hash de origen de la Handycam"))?;
    let ffmpeg = crate::layout::which("ffmpeg").ok_or_else(|| {
        Error::new(
            "handycam_conversion",
            "Se necesita ffmpeg para convertir los MPG",
        )
    })?;
    let staging = Staged {
        archive,
        input: ".ncreator/handycam-stage/input.mpg".to_owned(),
        output: ".ncreator/handycam-stage/output.mp4".to_owned(),
    };
    remove(archive, &staging.input)?;
    remove(archive, &staging.output)?;
    let mut src = nu9ve_volumes::fs::verify_read(
        source
            .open_read(path)?
            .ok_or_else(|| Error::new("source_changed", format!("Falta {path}")))?,
    );
    let mut copy = archive.create_new(&staging.input)?;
    let copied = std::io::copy(&mut src, &mut copy)
        .map_err(|error| Error::new("handycam_conversion", error.to_string()))?;
    nu9ve_volumes::fs::full_sync(&copy)?;
    drop(copy);
    require(
        copied == file["stamp"]["size"].as_u64().unwrap_or_default()
            && source_hash(archive, &staging.input)? == original,
        "source_changed",
        "El MPG temporal difiere del original revisado",
    )?;
    // Reserve both names through the checked archive tree; ffmpeg only writes the private staging file.
    drop(archive.create_new(&staging.output)?);
    let input = archive.path.join(&staging.input);
    let output = archive.path.join(&staging.output);
    run(
        Command::new(&ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-xerror",
                "-y",
                "-i",
            ])
            .arg(&input)
            .args([
                "-map",
                "0:v:0",
                "-map",
                "0:a?",
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "23",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-map_metadata",
                "0",
                "-movflags",
                "+faststart",
            ])
            .arg(&output),
        "FFmpeg no pudo convertir el MPG",
    )?;
    verify(&ffmpeg, &output)?;
    let before = crate::layout::probe(&input);
    let after = crate::layout::probe(&output);
    require(
        before.width.is_some()
            && before.width == after.width
            && before
                .seconds
                .zip(after.seconds)
                .is_some_and(|(a, b)| a.is_finite() && b > 0.0 && (a - b).abs() <= 0.25),
        "handycam_conversion",
        "El tamaño de imagen o la duración del MP4 difieren del MPG",
    )?;
    if let Ok(modified) = src.metadata().and_then(|meta| meta.modified()) {
        archive
            .open_read(&staging.output)?
            .ok_or_else(|| Error::new("handycam_conversion", "Falta el MP4"))?
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .map_err(|error| Error::new("handycam_conversion", error.to_string()))?;
    }
    let stamp = archive
        .stat(&staging.output)?
        .ok_or_else(|| Error::new("handycam_conversion", "Falta el MP4"))?;
    require(
        stamp.size <= copied.saturating_mul(2),
        "handycam_conversion",
        "El MP4 supera el espacio previsto",
    )?;
    Ok(staging)
}

fn verify(ffmpeg: &Path, output: &Path) -> Result<()> {
    run(
        Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-xerror",
                "-i",
            ])
            .arg(output)
            .args(["-map", "0:v:0", "-map", "0:a?", "-f", "null", "-"]),
        "No se pudo decodificar el MP4 completo",
    )
}
