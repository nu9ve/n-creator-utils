//! Human destination names, decided at plan time and stored in the plan. Port of `media/layout.py` and of
//! `capture_time` in `media/evidence.py`.
//!
//! Archive mirrors the historical MEDIA layout (type/year/month, dated names, Sony XML under `metadata/`); work
//! copies are flat per project. Times are Python's: a camera's own clock keeps its offset, a still's EXIF time is
//! local wall time, and a file without one falls back to its modification time in the host zone.
//!
//! Not ported: Stalker sessions (their names need `captures.py`/`zoneinfo`); `build_layout` refuses them with
//! `not_ported`. WAV durations read PCM and WAVE_FORMAT_EXTENSIBLE with a PCM subformat, as Python 3.12+; an older
//! Python reads only PCM and asks ffprobe for the rest.

use crate::rules::slug;
use crate::snapshot::kind_of;
use nu9ve_json::{Error, Result};
use nu9ve_time::DateTime;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub const MEDIA_KINDS: [&str; 4] = ["video", "audio", "image", "raw_image"];
const LONG_RECORDING_SECONDS: f64 = 600.0;

/// Python `layout.sony_card_path`: a file in Sony's card structure (the card needs the Sony cleanup profile).
pub fn sony_card_path(path: &str) -> bool {
    let lowered = format!("/{}", path.to_lowercase());
    lowered.contains("/private/m4root/")
        || lowered.contains("/private/avchd/")
        || lowered.starts_with("/avf_info/")
}

/// Python `layout.sony_index_member`: Sony camera databases/indexes, never deleted or edited here.
pub fn sony_index_member(path: &str) -> bool {
    let lowered = format!("/{}", path.to_lowercase());
    if lowered.contains("/avf_info/") || lowered.contains("/private/sony/") {
        return true;
    }
    if lowered.contains("/m4root/") {
        return !["/m4root/clip/", "/m4root/thmbnl/", "/m4root/sub/"]
            .iter()
            .any(|p| lowered.contains(p));
    }
    if lowered.contains("/avchd/") {
        return !lowered.contains("/stream/");
    }
    false
}

/// Python `layout.tag`.
pub fn tag(value: &str) -> Result<String> {
    Ok(slug(value)?.to_uppercase())
}

/// Python `evidence.capture_time(value, provenance)` without a zone: what the source said and, when it carries an
/// offset, the absolute instant.
pub fn capture_time(value: Option<&str>, provenance: &str) -> Value {
    let mut result = json!({"original": value, "provenance": provenance, "originals": {},
                            "captured_at_absolute": null, "time_zone": null, "offset_seconds": null});
    if let Some(parsed) = value.and_then(|v| DateTime::fromisoformat(&v.replace('Z', "+00:00")))
        && let Some(offset) = parsed.offset
    {
        result["captured_at_absolute"] = json!(parsed.to_utc().isoformat().replace("+00:00", "Z"));
        result["offset_seconds"] = json!(offset);
    }
    result
}

/// What `facts` learned about one media file (Python's `found` dict).
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub created: Option<DateTime>,
    pub capture_time: Option<Value>,
    pub seconds: Option<f64>,
    pub width: Option<i64>,
    pub model: Option<String>,
}

impl Facts {
    /// Python `dict(other, **self)`: this one's keys win.
    fn over(self, other: Facts) -> Facts {
        Facts {
            created: self.created.or(other.created),
            capture_time: self.capture_time.or(other.capture_time),
            seconds: self.seconds.or(other.seconds),
            width: self.width.or(other.width),
            model: self.model.or(other.model),
        }
    }
}

fn bad(message: impl Into<String>) -> Error {
    Error::new("operation_failed", message)
}

/// Python `layout.sony_xml`: facts of a Sony clip's `M01.XML` (creation, duration, fps, width, model).
pub fn sony_xml(path: &Path) -> Result<Facts> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(Facts::default());
    };
    if meta.len() > 1024 * 1024 {
        return Ok(Facts::default());
    }
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(Facts::default());
    };
    let Some(elements) = xml_elements(&bytes) else {
        return Ok(Facts::default());
    };
    let models = nuun_recipe::template()["models"].clone();
    let (mut found, mut frames, mut fps) = (Facts::default(), None::<i64>, None::<f64>);
    for (name, attributes) in elements {
        let get = |key: &str| {
            attributes
                .get(key)
                .map(String::as_str)
                .filter(|v| !v.is_empty())
        };
        match name.as_str() {
            "CreationDate" => {
                if let Some(value) = get("value") {
                    let created = DateTime::fromisoformat(value)
                        .ok_or_else(|| bad(format!("Invalid isoformat string: '{value}'")))?;
                    found.created = Some(created);
                    found.capture_time = Some(capture_time(Some(value), "sony.CreationDate"));
                }
            }
            "Duration" => {
                if let Some(v) = attributes
                    .get("value")
                    .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()))
                {
                    frames = v.parse().ok();
                }
            }
            "VideoFrame" => {
                let text = get("captureFps").or_else(|| get("formatFps")).unwrap_or("");
                let lead: String = text
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                fps =
                    if lead.is_empty() {
                        None
                    } else {
                        Some(lead.parse::<f64>().map_err(|_| {
                            bad(format!("could not convert string to float: '{lead}'"))
                        })?)
                    };
            }
            "VideoLayout" => {
                if let Some(v) = get("pixel").filter(|v| v.chars().all(|c| c.is_ascii_digit())) {
                    found.width = v.parse().ok();
                }
            }
            "Device" => {
                let model = attributes.get("modelName").cloned().unwrap_or_default();
                found.model = if model.is_empty() {
                    None
                } else {
                    match models[&model].as_str() {
                        Some(tagged) => Some(tagged.to_owned()),
                        None => Some(tag(&format!(
                            "{}{model}",
                            attributes.get("manufacturer").cloned().unwrap_or_default()
                        ))?),
                    }
                };
            }
            _ => {}
        }
    }
    if let (Some(frames), Some(fps)) = (frames, fps)
        && fps != 0.0
    {
        found.seconds = Some(frames as f64 / fps);
    }
    Ok(found)
}

/// Element local names with their attributes, in document order (Python `root.iter()`); `None` when ElementTree
/// would refuse the document: no root, unclosed or mismatched elements, a second root or text outside it.
fn xml_elements(bytes: &[u8]) -> Option<Vec<(String, BTreeMap<String, String>)>> {
    use quick_xml::events::{BytesStart, Event};
    fn element(e: &BytesStart<'_>) -> Option<(String, BTreeMap<String, String>)> {
        let mut attributes = BTreeMap::new();
        for a in e.attributes() {
            let a = a.ok()?;
            let raw: &str = &a.value;
            let value = quick_xml::escape::unescape(raw).ok()?.into_owned();
            attributes.insert(a.key.local_name().into_inner().to_owned(), value);
        }
        Some((e.local_name().into_inner().to_owned(), attributes))
    }
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    let (mut out, mut depth, mut roots) = (Vec::new(), 0usize, 0usize);
    loop {
        let event = reader.read_event_into(&mut buffer).ok()?;
        match event {
            Event::Start(e) => {
                roots += usize::from(depth == 0);
                depth += 1;
                out.push(element(&e)?);
            }
            Event::Empty(e) => {
                roots += usize::from(depth == 0);
                out.push(element(&e)?);
            }
            Event::End(_) => depth = depth.checked_sub(1)?,
            Event::Text(t) if depth == 0 => {
                let raw: &str = &t;
                if !raw.trim().is_empty() {
                    return None;
                }
            }
            Event::Eof => return (roots == 1 && depth == 0).then_some(out),
            _ => {}
        }
        if roots > 1 {
            return None;
        }
        buffer.clear();
    }
}

/// Python `layout.probe`: optional ffprobe facts (duration, creation time, width); the copy never depends on it.
pub fn probe(path: &Path) -> Facts {
    let Some(ffprobe) = which("ffprobe") else {
        return Facts::default();
    };
    let Ok(mut child) = std::process::Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Facts::default();
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        out
    });
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            _ => {
                // Python: TimeoutExpired → no facts.
                let _ = child.kill();
                let _ = child.wait();
                return Facts::default();
            }
        }
    }
    let out = reader.join().unwrap_or_default();
    let text: &[u8] = if out.is_empty() { b"{}" } else { &out };
    match serde_json::from_slice::<Value>(text) {
        Ok(data) => probe_facts(&data),
        Err(_) => Facts::default(),
    }
}

/// The facts of an ffprobe JSON answer.
pub fn probe_facts(data: &Value) -> Facts {
    let mut found = Facts::default();
    let format = &data["format"];
    found.seconds = match &format["duration"] {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    };
    if let Some(created) = format["tags"]["creation_time"]
        .as_str()
        .filter(|c| !c.is_empty())
        && let Some(parsed) = DateTime::fromisoformat(&created.replace('Z', "+00:00"))
    {
        found.created = Some(if parsed.offset.is_some() {
            parsed.astimezone()
        } else {
            parsed
        });
        found.capture_time = Some(capture_time(Some(created), "ffprobe.creation_time"));
    }
    found.width = data["streams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["codec_type"] == "video")
        .filter_map(|s| s["width"].as_i64().filter(|w| *w != 0))
        .max();
    found
}

/// Python `shutil.which(name)` on the current PATH.
fn which(name: &str) -> Option<std::path::PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&exe))
        .find(|p| p.is_file())
}

/// KSDATAFORMAT_SUBTYPE_PCM, the subformat GUID of a WAVE_FORMAT_EXTENSIBLE PCM file.
const PCM_SUBFORMAT: [u8; 16] = [
    1, 0, 0, 0, 0, 0, 16, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71,
];

/// Python `layout.wav_seconds` (`wave` module): frames / frame rate of a PCM WAV, `None` when unreadable.
pub fn wav_seconds(path: &Path) -> Option<f64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return None;
    }
    let mut format: Option<(u16, u32, u16)> = None;
    loop {
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk).ok()?;
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        match &chunk[0..4] {
            b"fmt " => {
                let mut body = vec![0u8; size as usize];
                file.read_exact(&mut body).ok()?;
                if body.len() < 16 {
                    return None;
                }
                let tag = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                let pcm = tag == 1
                    || (tag == 0xFFFE && body.len() >= 40 && body[24..40] == PCM_SUBFORMAT);
                if !pcm {
                    return None; // wave.Error: unknown format
                }
                format = Some((channels, rate, bits));
                if size % 2 == 1 {
                    file.seek(SeekFrom::Current(1)).ok()?;
                }
            }
            b"data" => {
                let (channels, rate, bits) = format?;
                let frame = u64::from(channels) * u64::from(bits.div_ceil(8));
                if frame == 0 || rate == 0 {
                    return None;
                }
                return Some((u64::from(size) / frame) as f64 / f64::from(rate));
            }
            _ => {
                file.seek(SeekFrom::Current(i64::from(size) + i64::from(size % 2)))
                    .ok()?;
            }
        }
    }
}

/// Python `layout.photo_facts`: capture start of a still from its EXIF time (+ offset when present).
pub fn photo_facts(photo: &Value) -> Result<Facts> {
    let value = format!(
        "{}{}",
        photo["captured_at"].as_str().unwrap_or_default(),
        photo["offset"].as_str().unwrap_or_default()
    );
    let created = DateTime::fromisoformat(&value)
        .ok_or_else(|| bad(format!("Invalid isoformat string: '{value}'")))?;
    Ok(Facts {
        created: Some(created),
        capture_time: Some(capture_time(Some(&value), "exif.DateTimeOriginal")),
        ..Facts::default()
    })
}

/// Python `Path(path).suffix.lower()`.
fn suffix(path: &str) -> String {
    crate::scan::suffix(path.rsplit('/').next().unwrap_or(path))
}

/// Python `Path(path).stem`.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// Python `layout.facts`: capture time (local wall clock), duration and model for one media file.
pub fn facts(source: &Path, file: &Value, use_photo: bool) -> Result<Facts> {
    let rel = file["path"].as_str().unwrap_or_default();
    let path = source.join(rel);
    let kind = kind_of(rel);
    let mut found = Facts::default();
    if use_photo
        && matches!(kind, Some("image" | "raw_image"))
        && file["photo"]["captured_at"]
            .as_str()
            .is_some_and(|c| !c.is_empty())
    {
        found = photo_facts(&file["photo"])?;
    }
    if kind == Some("video") && suffix(rel) == ".mp4" {
        let xml = path.with_file_name(format!("{}M01.XML", stem(rel)));
        found = sony_xml(&xml)?;
    }
    if matches!(kind, Some("video" | "audio")) && found.seconds.is_none() {
        if suffix(rel) == ".wav"
            && let Some(seconds) = wav_seconds(&path)
        {
            found.seconds = Some(seconds);
        }
        if found.seconds.is_none() || found.created.is_none() {
            found = found.over(probe(&path));
        }
    }
    if found.created.is_none() {
        let mtime = file["stamp"]["mtime_ns"].as_i64().unwrap_or(0) as f64 / 1e9;
        found.created = Some(DateTime::fromtimestamp(mtime).astimezone());
    }
    Ok(found)
}

/// Python `layout.stamp`: `26NOV04-163212`.
pub fn stamp(created: &DateTime) -> String {
    created.strftime("%y") + &created.strftime("%b").to_uppercase() + &created.strftime("%d-%H%M%S")
}

/// Python `layout.duration`: `MM-SS`, rounded half to even like `round`.
pub fn duration(seconds: Option<f64>) -> String {
    let total = seconds.unwrap_or(0.0).round_ties_even() as i64;
    format!("{:02}-{:02}", total.div_euclid(60), total.rem_euclid(60))
}

/// The H6 microphone of a file stem (Python `re.search(r'_(TR\d+|LR|MS)$', stem.upper())`).
fn h6_mic(upper: &str) -> Option<String> {
    for fixed in ["_LR", "_MS"] {
        if upper.ends_with(fixed) {
            return Some(fixed[1..].to_owned());
        }
    }
    let at = upper.rfind("_TR")?;
    let digits = &upper[at + 3..];
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("TR{digits}"))
}

/// Python `layout.model_for`: FUENTE of a file.
pub fn model_for(unit: &Value, file: &Value, found: &Facts) -> Result<String> {
    if let Some(source) = unit["device"]["source"].as_str().filter(|s| !s.is_empty()) {
        return Ok(source.to_owned());
    }
    if let Some(model) = found.model.as_deref().filter(|m| !m.is_empty()) {
        return Ok(model.to_owned());
    }
    let camera = unit["camera"].as_str().unwrap_or_default();
    if let Some(source) = nuun_recipe::template()["family_source"][camera].as_str() {
        return Ok(source.to_owned());
    }
    if camera == "h6" {
        let upper = stem(file["path"].as_str().unwrap_or_default()).to_uppercase();
        return Ok(format!("H6{}", h6_mic(&upper).unwrap_or_default()));
    }
    tag(camera)
}

/// Python `layout.archive_folder`.
pub fn archive_folder(kind: &str, found: &Facts, camera: &str, folders: &Value) -> String {
    if let Some(folder) = folders[camera][kind].as_str().filter(|f| !f.is_empty()) {
        return folder.to_owned(); // recipe folders, frozen in the plan
    }
    let three_sixty = matches!(camera, "dji-360" | "insta360");
    let folder = match kind {
        "video" if camera == "dji-drone" => "vdron",
        "video" if three_sixty => "360/videos",
        "video" if found.width.unwrap_or(0) >= 3840 => "v4k",
        "video" => "videos",
        "audio" if found.seconds.unwrap_or(0.0) > LONG_RECORDING_SECONDS => "recordingslong",
        "audio" => "recordings",
        "raw_image" => "rimages",
        _ if three_sixty => "360/images",
        _ => "images",
    };
    folder.to_owned()
}

/// Python's `claim`: deterministic, plan-local disambiguation for names taken in the same second.
fn claim(taken: &mut HashSet<String>, rel: &str) -> String {
    let (stem, ext) = match rel.rfind('.') {
        Some(i) => (&rel[..i], &rel[i..]),
        None => ("", rel),
    };
    let mut candidate = rel.to_owned();
    let mut n = 2;
    while taken.contains(&candidate.to_lowercase()) {
        candidate = format!("{}{}", stem.replace("[]", &format!("-{n}[]")), ext);
        n += 1;
    }
    taken.insert(candidate.to_lowercase());
    candidate
}

/// `split('/', 2)[2]`: the name after `YYYY/MM/`.
fn after_month(rel: &str) -> String {
    rel.splitn(3, '/').nth(2).unwrap_or_default().to_owned()
}

/// Python comparison of two aware datetimes (`a < b`): by instant.
fn earlier(a: &DateTime, b: &DateTime) -> bool {
    let key = |d: &DateTime| {
        (
            d.timestamp().unwrap_or(i64::MAX),
            d.wall.and_utc().timestamp_subsec_micros(),
        )
    };
    key(a) < key(b)
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

/// Python `layout.build_layout`: every source path → `{archive, work}` below each volume prefix. Units gain their
/// `capture_time`, as in Python, before the plan is hashed.
pub fn build_layout(
    source: &Path,
    plan: &Value,
    units: &mut [Value],
    files: &[Value],
) -> Result<Map<String, Value>> {
    let mut by_unit: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for file in files {
        by_unit
            .entry(text(file, "unit_id").to_owned())
            .or_default()
            .push(file);
    }
    let mut layout = Map::new();
    let mut taken = HashSet::new();
    let mut support: Vec<(usize, Vec<&Value>)> = Vec::new();
    let mut first_capture: Option<DateTime> = None;
    let mut order: Vec<usize> = (0..units.len()).collect();
    order.sort_by(|a, b| text(&units[*a], "id").cmp(text(&units[*b], "id")));
    let plan_country = text(plan, "country").to_owned();
    for index in order {
        let unit = units[index].clone();
        let mut members: Vec<&Value> = by_unit.get(text(&unit, "id")).cloned().unwrap_or_default();
        members.sort_by(|a, b| text(a, "path").cmp(text(b, "path")));
        let project = slug(text(&unit, "project"))?;
        let profile = text(&unit, "profile");
        let route = match plan["profile_routes"][profile].as_str() {
            Some(r) => r.to_owned(),
            None => format!("{}_raw", slug(profile)?),
        };
        let work_root = format!("{route}/{project}");
        let country = unit["country"].as_str().unwrap_or(&plan_country).to_owned();
        let label = format!("{}-{}", tag(&country)?, tag(&project)?);
        if unit["kind"] == "stalker_session" {
            let prefix = match text(&unit, "bundle") {
                "." => String::new(),
                bundle => format!("{bundle}/"),
            };
            let manifest_path = format!("{prefix}manifest.json");
            let raw = crate::tree::read_bytes(source, &manifest_path, 1024 * 1024)?
                .ok_or_else(|| Error::new("operation_failed", "Stalker manifest disappeared"))?;
            let manifest: Value = serde_json::from_slice(&raw)
                .map_err(|_| Error::new("invalid_json", "Invalid JSON document"))?;
            let mut evidence = capture_time(
                manifest["epochWallClock"].as_str(),
                "manifest.epochWallClock",
            );
            if evidence["captured_at_absolute"].is_null() {
                evidence = capture_time(manifest["createdAt"].as_str(), "manifest.createdAt");
            }
            nu9ve_json::require(
                evidence["captured_at_absolute"].is_null() || !truthy(&manifest["captureTimeZone"]),
                "not_ported",
                "Stalker plans with a named capture timezone still use Python",
            )?;
            let originals: Map<String, Value> = ["epochWallClock", "createdAt", "captureTimeZone"]
                .into_iter()
                .filter_map(|key| {
                    manifest
                        .get(key)
                        .map(|value| (key.to_owned(), value.clone()))
                })
                .collect();
            evidence["originals"] = Value::Object(originals);
            units[index]["capture_time"] = evidence;
            let fallback = members
                .iter()
                .filter_map(|f| f["stamp"]["mtime_ns"].as_i64())
                .min()
                .unwrap_or_default();
            let created = manifest["createdAt"]
                .as_str()
                .and_then(|value| DateTime::fromisoformat(&value.replace('Z', "+00:00")))
                .unwrap_or_else(|| DateTime::fromtimestamp(fallback as f64 / 1e9))
                .astimezone();
            let seconds = manifest["duration"]["value"]
                .as_f64()
                .zip(manifest["duration"]["timescale"].as_f64())
                .filter(|(_, scale)| *scale != 0.0)
                .map(|(value, scale)| value / scale);
            let month = created.strftime("%Y/%m");
            let name = after_month(&claim(
                &mut taken,
                &format!(
                    "{month}/{}_STALKER_{}_{label}[].stalker",
                    stamp(&created),
                    duration(seconds)
                ),
            ));
            if first_capture
                .as_ref()
                .is_none_or(|first| earlier(&created, first))
            {
                first_capture = Some(created);
            }
            for file in members {
                let path = text(file, "path");
                let inner = path
                    .strip_prefix(&prefix)
                    .ok_or_else(|| Error::new("invalid_bundle", "Member outside Stalker bundle"))?;
                layout.insert(
                    path.to_owned(),
                    json!({"archive": format!("stalker/{month}/{name}/{inner}"),
                    "work": format!("{work_root}/{name}/{inner}")}),
                );
            }
            continue;
        }
        let media: Vec<&Value> = members
            .iter()
            .copied()
            .filter(|f| kind_of(text(f, "path")).is_some_and(|k| MEDIA_KINDS.contains(&k)))
            .collect();
        if media.is_empty() {
            support.push((index, members));
            continue;
        }
        let camera = text(&unit, "camera").to_owned();
        let mission = &unit["mission"];
        let mut primary: Option<(String, String)> = None;
        for file in media {
            let path = text(file, "path");
            let kind = kind_of(path).unwrap_or_default();
            let found = facts(source, file, truthy(&unit["device"]))?;
            let mut created = found.created.expect("facts always has a creation time");
            if units[index].get("capture_time").is_none()
                && let Some(evidence) = &found.capture_time
            {
                units[index]["capture_time"] = evidence.clone();
            }
            // Naive camera civil evidence remains unknown; naming uses the host display only.
            if created.offset.is_none() {
                created = created.astimezone();
            }
            if first_capture
                .as_ref()
                .is_none_or(|first| earlier(&created, first))
            {
                first_capture = Some(created);
            }
            let mut parts = vec![stamp(&created), model_for(&unit, file, &found)?];
            if matches!(kind, "video" | "audio") {
                parts.push(duration(found.seconds));
            }
            let ext = suffix(path).trim_start_matches('.').to_uppercase();
            let folder = archive_folder(kind, &found, &camera, &plan["category_folders"]);
            let month = created.strftime("%Y/%m");
            let claimed = claim(
                &mut taken,
                &format!("{month}/{}_{label}[].{ext}", parts.join("_")),
            );
            let name = after_month(&claimed);
            let mut entry = json!({"archive": format!("{folder}/{month}/{name}"), "work": format!("{work_root}/{name}")});
            if mission["state"] == "matched" {
                // A mission keeps its session together: <first photo of the session>_<PLAN>/ in archive and work.
                let rules = &plan["missions"][text(mission, "flow")]["layout"];
                let start_text = text(mission, "session_start");
                let start = DateTime::fromisoformat(start_text)
                    .ok_or_else(|| bad(format!("Invalid isoformat string: '{start_text}'")))?;
                let session = format!(
                    "{}/{}_{}",
                    start.strftime("%Y/%m"),
                    stamp(&start),
                    tag(text(mission, "plan_name"))?
                );
                let in_work = rules["work_kinds"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == kind));
                let work = if in_work {
                    json!(format!("{}/{session}/{name}", text(rules, "work_folder")))
                } else {
                    Value::Null
                };
                entry = json!({"archive": format!("{}/{session}/{name}", text(rules, "archive_folder")), "work": work});
            }
            let work_kinds = &plan["category_work"][camera.as_str()];
            let outside_kinds = !work_kinds.is_null()
                && !work_kinds
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == kind));
            if unit["work_existing"].get(path).is_some()
                || mission["state"] == "pending_export"
                || outside_kinds
            {
                // Already in the consumer's work root, waiting for the export, or archive-only (DNG/RAW).
                entry["work"] = Value::Null;
            }
            layout.insert(path.to_owned(), entry);
            if primary.is_none() {
                let base = name
                    .rsplit_once('.')
                    .map_or(name.as_str(), |(b, _)| b)
                    .to_owned();
                primary = Some((month, base));
            }
        }
        let family = if camera == "sony" {
            "sony".to_owned()
        } else {
            slug(&camera)?
        };
        let (month, base) = primary.expect("at least one media file");
        for file in members {
            let path = text(file, "path");
            if layout.contains_key(path) {
                continue;
            }
            // Sidecars (Sony XML, LRV/THM) keep the clip's name under metadata/, archive only.
            let ext = suffix(path).trim_start_matches('.').to_uppercase();
            let rel = claim(
                &mut taken,
                &format!("metadata/{family}/{month}/{base}.{ext}"),
            );
            layout.insert(path.to_owned(), json!({"archive": rel, "work": null}));
        }
    }
    if !support.is_empty() {
        support_layout(&mut layout, plan, units, support, first_capture)?;
    }
    Ok(layout)
}

/// The support part of `build_layout`: card indexes and MISC under `metadata/<family>/support/…`, original tree kept.
fn support_layout(
    layout: &mut Map<String, Value>,
    plan: &Value,
    units: &[Value],
    support: Vec<(usize, Vec<&Value>)>,
    first_capture: Option<DateTime>,
) -> Result<()> {
    let when = match first_capture {
        Some(first) => first,
        None => support
            .iter()
            .flat_map(|(_, m)| m.iter())
            .map(|f| {
                DateTime::fromtimestamp(f["stamp"]["mtime_ns"].as_i64().unwrap_or(0) as f64 / 1e9)
            })
            .max_by(|a, b| a.wall.cmp(&b.wall))
            .expect("support units have members"),
    };
    // Card support belongs to the device of the ingest when all its media agree on one.
    let media_units: Vec<&Value> = units.iter().filter(|u| !truthy(&u["support"])).collect();
    let cameras: HashSet<&str> = media_units.iter().map(|u| text(u, "camera")).collect();
    let sources: HashSet<Option<&str>> = media_units
        .iter()
        .map(|u| u["device"]["source"].as_str())
        .collect();
    let card_camera = if cameras.len() == 1 {
        cameras.iter().next().copied()
    } else {
        None
    };
    let card_source = if sources.len() == 1 {
        sources.iter().next().copied().flatten()
    } else {
        None
    };
    let family_source = nuun_recipe::template()["family_source"].clone();
    let country = tag(text(plan, "country"))?;
    for (index, members) in support {
        let unit = &units[index];
        let own = text(unit, "camera");
        let camera = match card_camera {
            Some(card) if own == "generic" => card,
            _ => own,
        };
        let sony_tree = members.iter().any(|f| {
            let lowered = text(f, "path").to_lowercase();
            format!("/{lowered}").contains("/private/")
                || lowered.starts_with("avf_info/")
                || lowered.starts_with("private/")
        });
        let family = if camera == "sony" || sony_tree {
            "sony".to_owned()
        } else if camera.starts_with("dji-") {
            "dji".to_owned()
        } else {
            slug(camera)?
        };
        let model = match (
            family_source[&family].as_str().filter(|s| !s.is_empty()),
            card_source.filter(|s| !s.is_empty()),
        ) {
            (Some(source), _) | (None, Some(source)) => source.to_owned(),
            (None, None) => tag(camera)?,
        };
        let label = if truthy(&unit["support"]) {
            format!("ingest-{}", text(unit, "id"))
        } else {
            tag(text(unit, "project"))?
        };
        let folder = format!(
            "metadata/{family}/support/{}/{}_{country}-{label}_{model}",
            when.strftime("%Y/%m"),
            when.strftime("%d%m-%H%M%S"),
        );
        for file in members {
            // Camera indexes/thumbnails keep their original tree so the card can be reconstructed.
            let path = text(file, "path");
            layout.insert(
                path.to_owned(),
                json!({"archive": format!("{folder}/{path}"), "work": null}),
            );
        }
    }
    Ok(())
}

/// Python truthiness of a JSON value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(root: &Path, rel: &str, bytes: &[u8], unit: &str) -> Value {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        let mut record = json!({"path": rel, "unit_id": unit,
                                "stamp": {"size": bytes.len(), "mtime_ns": 1_778_000_000_000_000_000i64}});
        let photo = crate::photo_meta::read(&path);
        if !photo.is_empty() {
            record["photo"] = Value::Object(photo);
        }
        record
    }

    fn drone_plan() -> Value {
        let categories = &nuun_recipe::template()["categories"]["dji-drone"];
        json!({"country": "mx", "category_folders": {"dji-drone": categories["archive"]},
               "category_work": {"dji-drone": categories["work_kinds"]}})
    }

    fn unit(id: &str, camera: &str, device: Value) -> Value {
        json!({"id": id, "project": "intro", "profile": "e64", "camera": camera, "kind": "camera_bundle",
               "device": device})
    }

    #[test]
    fn drone_stills_are_named_by_exif_and_the_recipe_and_raw_stays_in_the_archive() {
        let root = tempfile::tempdir().unwrap();
        let jpg = crate::photo_meta::sample::jpeg("DJI", "FC3170", "2026:11:04 16:32:12", None);
        let dng = crate::photo_meta::sample::tiff("DJI", "FC3170", "2026:11:04 16:32:12");
        let files = vec![
            file(root.path(), "DCIM/101MEDIA/DJI_0001.JPG", &jpg, "u1"),
            file(root.path(), "DCIM/101MEDIA/DJI_0001.DNG", &dng, "u1"),
            file(root.path(), "DCIM/101MEDIA/DJI_0002.JPG", &jpg, "u2"),
        ];
        let device = json!({"source": "MAVICAIR2", "category": "dji-drone"});
        let mut units = vec![
            unit("u1", "dji-drone", device.clone()),
            unit("u2", "dji-drone", device),
        ];
        let layout = build_layout(root.path(), &drone_plan(), &mut units, &files).unwrap();
        assert_eq!(
            layout["DCIM/101MEDIA/DJI_0001.JPG"],
            json!({"archive": "dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG",
                   "work": "e64_raw/intro/26NOV04-163212_MAVICAIR2_MX-INTRO[].JPG"})
        );
        assert_eq!(
            layout["DCIM/101MEDIA/DJI_0001.DNG"],
            json!({"archive": "dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO[].DNG", "work": null})
        );
        // Same second, same name: the next one takes -2.
        assert_eq!(
            layout["DCIM/101MEDIA/DJI_0002.JPG"]["archive"],
            "dron/fotos/2026/11/26NOV04-163212_MAVICAIR2_MX-INTRO-2[].JPG"
        );
        assert_eq!(
            units[0]["capture_time"]["provenance"],
            "exif.DateTimeOriginal"
        );
        // The sample carries OffsetTime -07:00.
        assert_eq!(
            units[0]["capture_time"]["original"],
            "2026-11-04T16:32:12-07:00"
        );
        assert_eq!(
            units[0]["capture_time"]["captured_at_absolute"],
            "2026-11-04T23:32:12Z"
        );
    }

    #[test]
    fn a_sony_clip_takes_its_xml_time_model_duration_and_width() {
        let root = tempfile::tempdir().unwrap();
        let xml = concat!(
            "<NonRealTimeMeta xmlns=\"urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.00\">",
            "<Duration value=\"300\"/><CreationDate value=\"2026-11-04T16:32:12-07:00\"/>",
            "<VideoFormat><VideoFrame captureFps=\"29.97p\" formatFps=\"29.97p\"/>",
            "<VideoLayout pixel=\"3840\" numOfVerticalLine=\"2160\"/></VideoFormat>",
            "<Device manufacturer=\"Sony\" modelName=\"ILCE-6400\"/></NonRealTimeMeta>"
        );
        let files = vec![
            file(root.path(), "PRIVATE/M4ROOT/CLIP/C0001.MP4", b"video", "s1"),
            file(
                root.path(),
                "PRIVATE/M4ROOT/CLIP/C0001M01.XML",
                xml.as_bytes(),
                "s1",
            ),
        ];
        let mut units = vec![unit("s1", "sony", Value::Null)];
        let layout =
            build_layout(root.path(), &json!({"country": "mx"}), &mut units, &files).unwrap();
        assert_eq!(
            layout["PRIVATE/M4ROOT/CLIP/C0001.MP4"],
            json!({"archive": "v4k/2026/11/26NOV04-163212_SONYA6400_00-10_MX-INTRO[].MP4",
                   "work": "e64_raw/intro/26NOV04-163212_SONYA6400_00-10_MX-INTRO[].MP4"})
        );
        assert_eq!(
            layout["PRIVATE/M4ROOT/CLIP/C0001M01.XML"],
            json!({"archive": "metadata/sony/2026/11/26NOV04-163212_SONYA6400_00-10_MX-INTRO[].XML", "work": null})
        );
        assert_eq!(units[0]["capture_time"]["offset_seconds"], -25200);
        // Broken XML gives no facts, as ElementTree's ParseError.
        let broken = root.path().join("bad.xml");
        std::fs::write(&broken, b"<a><b></a>").unwrap();
        assert!(sony_xml(&broken).unwrap().created.is_none());
        std::fs::write(&broken, b"<a/><Device modelName=\"X\"/>").unwrap();
        assert!(sony_xml(&broken).unwrap().model.is_none(), "a second root");
    }

    #[test]
    fn support_files_keep_their_tree_under_the_card_device() {
        let root = tempfile::tempdir().unwrap();
        let jpg = crate::photo_meta::sample::jpeg("DJI", "FC3170", "2026:11:04 16:32:12", None);
        let files = vec![
            file(root.path(), "DCIM/101MEDIA/DJI_0001.JPG", &jpg, "u1"),
            file(root.path(), "MISC/THM/101/x.THM", b"t", "sup"),
        ];
        let mut units = vec![
            unit("u1", "dji-drone", json!({"source": "MAVICAIR2"})),
            json!({"id": "sup", "project": "camera-support", "profile": "e64", "camera": "generic",
                   "kind": "file", "support": true}),
        ];
        let layout = build_layout(root.path(), &drone_plan(), &mut units, &files).unwrap();
        // The first capture keeps the still's own offset (-07:00), whatever the host zone.
        assert_eq!(
            layout["MISC/THM/101/x.THM"]["archive"],
            "metadata/dji/support/2026/11/0411-163212_MX-ingest-sup_MAVICAIR2/MISC/THM/101/x.THM"
        );
    }

    #[test]
    fn small_rules_follow_python() {
        assert_eq!(duration(Some(10.01)), "00-10");
        assert_eq!(duration(Some(62.5)), "01-02", "round half to even");
        assert_eq!(duration(None), "00-00");
        assert_eq!(h6_mic("ZOOM0001_TR12"), Some("TR12".into()));
        assert_eq!(h6_mic("ZOOM0001_LR"), Some("LR".into()));
        assert_eq!(h6_mic("ZOOM0001"), None);
        assert!(sony_index_member("PRIVATE/M4ROOT/MEDIAPRO.XML"));
        assert!(!sony_index_member("PRIVATE/M4ROOT/CLIP/C0001.MP4"));
        assert!(sony_card_path("AVF_INFO/AVIN0001.BNP"));
        let mut taken = HashSet::new();
        assert_eq!(claim(&mut taken, "2026/11/A[].JPG"), "2026/11/A[].JPG");
        assert_eq!(claim(&mut taken, "2026/11/a[].jpg"), "2026/11/a-2[].jpg");
        assert_eq!(
            capture_time(Some("2026-11-04T16:32:12"), "x")["captured_at_absolute"],
            Value::Null
        );
        let probed = probe_facts(&json!({
            "format": {"duration": "12.5", "tags": {"creation_time": "2026-11-04T23:32:12.000000Z"}},
            "streams": [{"codec_type": "video", "width": 1920}, {"codec_type": "audio"}]}));
        assert_eq!((probed.width, probed.seconds), (Some(1920), Some(12.5)));
        assert_eq!(probed.created.unwrap().timestamp(), Some(1_793_835_132));
    }

    fn wav(tag: u16, extra: &[u8], seconds: u32) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend(tag.to_le_bytes());
        fmt.extend(2u16.to_le_bytes());
        fmt.extend(48_000u32.to_le_bytes());
        fmt.extend((48_000u32 * 6).to_le_bytes());
        fmt.extend(6u16.to_le_bytes());
        fmt.extend(24u16.to_le_bytes());
        fmt.extend(extra);
        let data = vec![0u8; (48_000 * 6 * seconds) as usize];
        let mut body = b"WAVE".to_vec();
        body.extend(b"bext");
        body.extend(2u32.to_le_bytes());
        body.extend([0, 0]);
        body.extend(b"fmt ");
        body.extend((fmt.len() as u32).to_le_bytes());
        body.extend(fmt);
        body.extend(b"data");
        body.extend((data.len() as u32).to_le_bytes());
        body.extend(data);
        let mut out = b"RIFF".to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn wav_duration_reads_pcm_and_extensible_pcm() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.wav");
        std::fs::write(&path, wav(1, &[], 2)).unwrap();
        assert_eq!(wav_seconds(&path), Some(2.0));
        let mut extensible = vec![22, 0, 24, 0, 3, 0, 0, 0];
        extensible.extend(PCM_SUBFORMAT);
        std::fs::write(&path, wav(0xFFFE, &extensible, 1)).unwrap();
        assert_eq!(wav_seconds(&path), Some(1.0));
        std::fs::write(&path, wav(3, &[], 1)).unwrap();
        assert_eq!(wav_seconds(&path), None, "float WAV: wave.Error");
    }
}
