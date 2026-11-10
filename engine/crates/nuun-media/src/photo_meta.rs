//! Bounded, read-only EXIF/XMP facts of a still (JPEG or TIFF/DNG). Port of `media/photo_meta.py`, no dependencies.
//!
//! Only what identifies the device and the capture: Make, Model, serial, DateTimeOriginal (+ offset), GPS and the DJI
//! flight XMP (`drone-dji:*`). Never fails for a damaged or foreign file: returns an empty object. The XMP
//! `tiff:Model` of DJI files is unreliable (observed `Test_Pro`), so Model always comes from EXIF.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const PHOTO_SUFFIXES: [&str; 3] = [".jpg", ".jpeg", ".dng"];
const JPEG_HEAD: u64 = 1024 * 1024; // never walk segments past this offset
const MAX_ENTRIES: usize = 512;
const MAX_VALUE: usize = 256 * 1024;
const XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\x00";
const FLIGHT_KEYS: [(&str, &str); 8] = [
    ("GimbalPitchDegree", "gimbal_pitch"),
    ("GimbalYawDegree", "gimbal_yaw"),
    ("GimbalRollDegree", "gimbal_roll"),
    ("FlightYawDegree", "flight_yaw"),
    ("FlightPitchDegree", "flight_pitch"),
    ("FlightRollDegree", "flight_roll"),
    ("RelativeAltitude", "relative_altitude"),
    ("AbsoluteAltitude", "absolute_altitude"),
];

/// Any failure while walking a file: the caller returns `{}` as Python does.
struct Bad;
type R<T> = std::result::Result<T, Bad>;
/// EXIF facts and the XMP packet found on the way.
type Facts = (Map<String, Value>, Option<Vec<u8>>);

/// A decoded TIFF value (Python returns str, bytes or a tuple).
#[derive(Debug, Clone)]
enum Val {
    Text(String),
    Bytes(Vec<u8>),
    Ints(Vec<i64>),
    Rationals(Vec<Option<f64>>),
}

fn type_size(kind: u16) -> usize {
    match kind {
        1 | 2 | 7 => 1,
        3 => 2,
        4 | 9 => 4,
        5 | 10 => 8,
        _ => 0,
    }
}

struct Tiff<'a> {
    read: &'a mut dyn FnMut(u64, usize) -> R<Vec<u8>>,
    base: u64,
    little: bool,
    first: u32,
}

/// One IFD entry: type, count and the 4 inline bytes.
type Entry = (u16, u32, [u8; 4]);

impl<'a> Tiff<'a> {
    fn new(read: &'a mut dyn FnMut(u64, usize) -> R<Vec<u8>>, base: u64) -> R<Self> {
        let order = read(base, 2)?;
        let little = match order.as_slice() {
            b"II" => true,
            b"MM" => false,
            _ => return Err(Bad),
        };
        let head = read(base + 2, 6)?;
        if head.len() < 6 {
            return Err(Bad);
        }
        let mut tiff = Tiff {
            read,
            base,
            little,
            first: 0,
        };
        if tiff.u16(&head[0..2]) != 42 {
            return Err(Bad);
        }
        tiff.first = tiff.u32(&head[2..6]);
        Ok(tiff)
    }

    fn u16(&self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    }

    fn u32(&self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    }

    fn ifd(&mut self, offset: u64) -> R<BTreeMap<u16, Entry>> {
        let head = (self.read)(self.base + offset, 2)?;
        if head.len() < 2 {
            return Err(Bad);
        }
        let count = self.u16(&head) as usize;
        if count > MAX_ENTRIES {
            return Err(Bad);
        }
        let raw = (self.read)(self.base + offset + 2, 12 * count)?;
        if raw.len() < 12 * count {
            return Err(Bad); // Python's struct.unpack on a short slice fails the whole read
        }
        let mut entries = BTreeMap::new();
        for i in 0..count {
            let e = &raw[12 * i..12 * i + 12];
            entries.insert(
                self.u16(&e[0..2]),
                (
                    self.u16(&e[2..4]),
                    self.u32(&e[4..8]),
                    [e[8], e[9], e[10], e[11]],
                ),
            );
        }
        Ok(entries)
    }

    fn value(&mut self, entry: &Entry) -> R<Option<Val>> {
        let (kind, n, inline) = *entry;
        let size = type_size(kind) * n as usize;
        if size == 0 || size > MAX_VALUE {
            return Ok(None);
        }
        let data = if size <= 4 {
            inline[..size].to_vec()
        } else {
            let at = self.u32(&inline);
            (self.read)(self.base + u64::from(at), size)?
        };
        if data.len() < size {
            return Ok(None);
        }
        let n = n as usize;
        Ok(Some(match kind {
            2 => {
                let text = data.split(|b| *b == 0).next().unwrap_or(&[]);
                let text = String::from_utf8_lossy(text).trim().to_owned();
                if text.is_empty() {
                    return Ok(None);
                }
                Val::Text(text)
            }
            1 | 7 => Val::Bytes(data),
            3 => Val::Ints(
                (0..n)
                    .map(|i| i64::from(self.u16(&data[2 * i..])))
                    .collect(),
            ),
            4 => Val::Ints(
                (0..n)
                    .map(|i| i64::from(self.u32(&data[4 * i..])))
                    .collect(),
            ),
            9 => Val::Ints(
                (0..n)
                    .map(|i| i64::from(self.u32(&data[4 * i..]) as i32))
                    .collect(),
            ),
            _ => {
                let signed = kind == 10;
                let num = |i: usize| -> f64 {
                    let v = self.u32(&data[4 * i..]);
                    if signed {
                        f64::from(v as i32)
                    } else {
                        f64::from(v)
                    }
                };
                Val::Rationals(
                    (0..n)
                        .map(|i| {
                            let (a, b) = (num(2 * i), num(2 * i + 1));
                            if b == 0.0 { None } else { Some(a / b) }
                        })
                        .collect(),
                )
            }
        }))
    }

    fn text(&mut self, entries: &BTreeMap<u16, Entry>, tag: u16) -> R<Option<String>> {
        Ok(match entries.get(&tag) {
            Some(e) => match self.value(e)? {
                Some(Val::Text(t)) => Some(t),
                _ => None,
            },
            None => None,
        })
    }

    /// Python `offset`: the first number of an entry (an IFD pointer).
    fn offset(&mut self, entries: &BTreeMap<u16, Entry>, tag: u16) -> R<Option<u64>> {
        Ok(match entries.get(&tag) {
            Some(e) => match self.value(e)? {
                Some(Val::Ints(v)) => v.first().map(|x| *x as u64).filter(|x| *x != 0),
                Some(Val::Bytes(b)) => b.first().map(|x| u64::from(*x)).filter(|x| *x != 0),
                _ => None,
            },
            None => None,
        })
    }
}

/// Python `round(x, n)`: correctly rounded to n decimals (Rust's `{:.n}` rounds the exact value, ties to even).
fn round(x: f64, digits: usize) -> f64 {
    format!("{x:.digits$}").parse().unwrap_or(x)
}

fn degrees(values: Option<Val>, reference: Option<String>) -> Option<f64> {
    let Some(Val::Rationals(v)) = values else {
        return None;
    };
    if v.len() != 3 || v.iter().any(Option::is_none) {
        return None;
    }
    let result = v[0]? + v[1]? / 60.0 + v[2]? / 3600.0;
    let south_or_west = matches!(reference.as_deref(), Some("S") | Some("W"));
    Some(round(if south_or_west { -result } else { result }, 7))
}

fn digits(s: &str, pattern: &str) -> bool {
    // `d` is a digit, any other character must match itself.
    s.len() == pattern.len()
        && s.chars()
            .zip(pattern.chars())
            .all(|(c, p)| if p == 'd' { c.is_ascii_digit() } else { c == p })
}

fn tiff_facts(read: &mut dyn FnMut(u64, usize) -> R<Vec<u8>>, base: u64) -> R<Facts> {
    let mut tiff = Tiff::new(read, base)?;
    let ifd0 = tiff.ifd(u64::from(tiff.first))?;
    let mut found = Map::new();
    found.insert("make".into(), json!(tiff.text(&ifd0, 0x010F)?));
    found.insert("model".into(), json!(tiff.text(&ifd0, 0x0110)?));
    let mut serial = tiff.text(&ifd0, 0xC62F)?;
    let xmp = match ifd0.get(&0x02BC) {
        Some(e) => match tiff.value(e)? {
            Some(Val::Bytes(b)) => Some(b),
            Some(Val::Text(t)) => Some(t.into_bytes()),
            _ => None,
        },
        None => None,
    };
    if let Some(at) = tiff.offset(&ifd0, 0x8769)? {
        let exif = tiff.ifd(at)?;
        if serial.is_none() {
            serial = tiff.text(&exif, 0xA431)?;
        }
        if let Some(when) = tiff.text(&exif, 0x9003)?
            && digits(&when, "dddd:dd:dd dd:dd:dd")
        {
            found.insert(
                "captured_at".into(),
                json!(format!("{}T{}", when[..10].replace(':', "-"), &when[11..])),
            );
        }
        if let Some(offset) = tiff.text(&exif, 0x9011)?
            && (offset.starts_with('+') || offset.starts_with('-'))
            && digits(&offset[1..], "dd:dd")
        {
            found.insert("offset".into(), json!(offset));
        }
    }
    found.insert("serial".into(), json!(serial));
    if let Some(at) = tiff.offset(&ifd0, 0x8825)? {
        let gps = tiff.ifd(at)?;
        let mut get = |tag: u16| -> R<Option<Val>> {
            match gps.get(&tag) {
                Some(e) => tiff.value(e),
                None => Ok(None),
            }
        };
        let text = |v: Option<Val>| match v {
            Some(Val::Text(t)) => Some(t),
            _ => None,
        };
        let lat = degrees(get(2)?, text(get(1)?));
        let lon = degrees(get(4)?, text(get(3)?));
        if let (Some(lat), Some(lon)) = (lat, lon) {
            let mut place = Map::new();
            place.insert("lat".into(), json!(lat));
            place.insert("lon".into(), json!(lon));
            if let Some(Val::Rationals(alt)) = get(6)?
                && let Some(Some(meters)) = alt.first().copied()
            {
                let below = matches!(get(5)?, Some(Val::Bytes(b)) if b.first() == Some(&1));
                place.insert(
                    "alt".into(),
                    json!(round(if below { -meters } else { meters }, 2)),
                );
            }
            found.insert("gps".into(), Value::Object(place));
        }
    }
    Ok((found, xmp))
}

/// The DJI flight values of an XMP packet, as attributes or elements.
fn xmp_facts(xmp: &[u8]) -> Map<String, Value> {
    let text = String::from_utf8_lossy(xmp);
    let number = |s: &str| -> Option<f64> {
        let t = s.trim();
        let ok = !t.is_empty()
            && t.chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '+' || c == '-')));
        if ok { t.parse::<f64>().ok() } else { None }
    };
    let mut flight = Map::new();
    for (key, name) in FLIGHT_KEYS {
        let attribute = format!("drone-dji:{key}=\"");
        let element = format!("<drone-dji:{key}>");
        let found = text
            .find(&attribute)
            .and_then(|i| {
                let rest = &text[i + attribute.len()..];
                rest.find('"').map(|end| &rest[..end])
            })
            .and_then(number)
            .or_else(|| {
                text.find(&element)
                    .and_then(|i| {
                        let rest = &text[i + element.len()..];
                        rest.find('<').map(|end| &rest[..end])
                    })
                    .and_then(number)
            });
        if let Some(value) = found {
            flight.insert(name.into(), json!(value));
        }
    }
    flight
}

/// Walk JPEG segment headers with seeks; read only APP1 bodies (EXIF, XMP). Stops at the image data.
fn jpeg(file: &mut std::fs::File) -> R<Facts> {
    let mut soi = [0u8; 2];
    if file.read_exact(&mut soi).is_err() || soi != [0xFF, 0xD8] {
        return Ok((Map::new(), None));
    }
    let (mut exif, mut xmp): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    let mut position = 2u64;
    while position < JPEG_HEAD {
        let mut head = [0u8; 4];
        if file.read_exact(&mut head).is_err()
            || head[0] != 0xFF
            || head[1] == 0xD9
            || head[1] == 0xDA
        {
            break;
        }
        let length = u64::from(u16::from_be_bytes([head[2], head[3]]));
        if length < 2 {
            break;
        }
        if head[1] == 0xE1 && (exif.is_none() || xmp.is_none()) {
            let mut body = Vec::new();
            file.by_ref()
                .take(length - 2)
                .read_to_end(&mut body)
                .map_err(|_| Bad)?;
            if body.starts_with(b"Exif\x00\x00") && exif.is_none() {
                exif = Some(body[6..].to_vec());
            } else if body.starts_with(XMP_HEADER) && xmp.is_none() {
                xmp = Some(body[XMP_HEADER.len()..].to_vec());
            }
        } else {
            file.seek(SeekFrom::Current((length - 2) as i64))
                .map_err(|_| Bad)?;
        }
        position += 2 + length;
        if exif.is_some() && xmp.is_some() {
            break;
        }
    }
    let Some(exif) = exif else {
        return Ok((Map::new(), xmp));
    };
    let mut read = |at: u64, size: usize| -> R<Vec<u8>> {
        let start = (at as usize).min(exif.len());
        Ok(exif[start..(start + size).min(exif.len())].to_vec())
    };
    let (found, embedded) = tiff_facts(&mut read, 0)?;
    Ok((found, xmp.or(embedded)))
}

/// `{make, model, serial, captured_at, offset, gps, flight}` for a JPEG/DNG; `{}` when absent or unreadable.
pub fn read(path: &Path) -> Map<String, Value> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Map::new();
    };
    let dng = path.to_string_lossy().to_lowercase().ends_with(".dng");
    let result = if dng {
        let mut read = |at: u64, size: usize| -> R<Vec<u8>> {
            file.seek(SeekFrom::Start(at)).map_err(|_| Bad)?;
            let mut data = Vec::new();
            file.by_ref()
                .take(size as u64)
                .read_to_end(&mut data)
                .map_err(|_| Bad)?;
            Ok(data)
        };
        tiff_facts(&mut read, 0)
    } else {
        jpeg(&mut file)
    };
    let Ok((found, xmp)) = result else {
        return Map::new();
    };
    let mut found: Map<String, Value> = found.into_iter().filter(|(_, v)| !v.is_null()).collect();
    if let Some(xmp) = xmp {
        let flight = xmp_facts(&xmp);
        if !flight.is_empty() {
            found.insert("flight".into(), Value::Object(flight));
        }
    }
    if found.contains_key("make") || found.contains_key("captured_at") {
        found
    } else {
        Map::new()
    }
}

/// Test support: a little-endian TIFF/EXIF stream with Make, Model, DateTimeOriginal, offset and GPS, wrapped
/// in a JPEG with an XMP packet. Built by hand so the tests do not need sample photos.
#[cfg(test)]
pub mod sample {
    fn ascii(text: &str) -> Vec<u8> {
        let mut v = text.as_bytes().to_vec();
        v.push(0);
        v
    }

    /// (tag, type, count, payload); payloads over 4 bytes go after the IFDs.
    fn ifd(
        entries: &[(u16, u16, u32, Vec<u8>)],
        start: u32,
        data: &mut Vec<u8>,
        next_data: &mut u32,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend((entries.len() as u16).to_le_bytes());
        for (tag, kind, count, payload) in entries {
            out.extend(tag.to_le_bytes());
            out.extend(kind.to_le_bytes());
            out.extend(count.to_le_bytes());
            if payload.len() <= 4 {
                let mut inline = payload.clone();
                inline.resize(4, 0);
                out.extend(inline);
            } else {
                out.extend(next_data.to_le_bytes());
                data.extend(payload);
                *next_data += payload.len() as u32;
            }
        }
        out.extend(0u32.to_le_bytes());
        let _ = start;
        out
    }

    fn rationals(values: &[(u32, u32)]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes()))
            .collect()
    }

    /// The TIFF stream (starting at "II").
    pub fn tiff(make: &str, model: &str, when: &str) -> Vec<u8> {
        // Layout: header(8) | IFD0 (4 entries) | EXIF IFD (2) | GPS IFD (6) | data.
        let ifd0_at = 8u32;
        let ifd0_len = 2 + 12 * 4 + 4;
        let exif_at = ifd0_at + ifd0_len;
        let exif_len = 2 + 12 * 2 + 4;
        let gps_at = exif_at + exif_len;
        let gps_len = 2 + 12 * 6 + 4;
        let mut next = gps_at + gps_len;
        let mut data = Vec::new();
        let ifd0 = ifd(
            &[
                (0x010F, 2, (make.len() + 1) as u32, ascii(make)),
                (0x0110, 2, (model.len() + 1) as u32, ascii(model)),
                (0x8769, 4, 1, exif_at.to_le_bytes().to_vec()),
                (0x8825, 4, 1, gps_at.to_le_bytes().to_vec()),
            ],
            ifd0_at,
            &mut data,
            &mut next,
        );
        let exif = ifd(
            &[
                (0x9003, 2, 20, ascii(when)),
                (0x9011, 2, 7, ascii("-07:00")),
            ],
            exif_at,
            &mut data,
            &mut next,
        );
        let gps = ifd(
            &[
                (1, 2, 2, ascii("N")),
                (2, 5, 3, rationals(&[(29, 1), (5, 1), (2400, 100)])),
                (3, 2, 2, ascii("W")),
                (4, 5, 3, rationals(&[(110, 1), (57, 1), (3000, 100)])),
                (5, 1, 1, vec![0]),
                (6, 5, 1, rationals(&[(21050, 100)])),
            ],
            gps_at,
            &mut data,
            &mut next,
        );
        let mut out = b"II".to_vec();
        out.extend(42u16.to_le_bytes());
        out.extend(ifd0_at.to_le_bytes());
        out.extend(ifd0);
        out.extend(exif);
        out.extend(gps);
        out.extend(data);
        out
    }

    pub fn jpeg(make: &str, model: &str, when: &str, xmp: Option<&str>) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        let mut app1 = |body: Vec<u8>| {
            out.extend([0xFF, 0xE1]);
            out.extend(((body.len() + 2) as u16).to_be_bytes());
            out.extend(body);
        };
        let mut exif = b"Exif\x00\x00".to_vec();
        exif.extend(tiff(make, model, when));
        app1(exif);
        if let Some(packet) = xmp {
            let mut body = super::XMP_HEADER.to_vec();
            body.extend(packet.as_bytes());
            app1(body);
        }
        out.extend([0xFF, 0xDA, 0x00, 0x02]); // start of scan: the walk stops here
        out.extend([0x00; 16]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn a_drone_jpeg_gives_device_time_place_and_flight() {
        let xmp = r#"<x:xmpmeta><rdf:Description drone-dji:GimbalPitchDegree="-90.0" drone-dji:RelativeAltitude="+60.20"
            tiff:Model="Test_Pro"><drone-dji:FlightYawDegree> 12.5 </drone-dji:FlightYawDegree></rdf:Description></x:xmpmeta>"#;
        let (_d, path) = write(
            "DJI_0001.JPG",
            &sample::jpeg("DJI", "FC3170", "2026:05:14 10:20:30", Some(xmp)),
        );
        let facts = read(&path);
        assert_eq!(facts["make"], "DJI");
        assert_eq!(
            facts["model"], "FC3170",
            "Model comes from EXIF, never from tiff:Model in XMP"
        );
        assert_eq!(facts["captured_at"], "2026-05-14T10:20:30");
        assert_eq!(facts["offset"], "-07:00");
        assert_eq!(
            facts["gps"],
            json!({"lat": 29.09, "lon": -110.9583333, "alt": 210.5})
        );
        assert_eq!(
            facts["flight"],
            json!({"gimbal_pitch": -90.0, "relative_altitude": 60.2, "flight_yaw": 12.5})
        );
        assert!(facts.get("serial").is_none());
    }

    #[test]
    fn a_dng_is_read_as_tiff() {
        let (_d, path) = write(
            "DJI_0001.DNG",
            &sample::tiff("DJI", "FC3170", "2026:05:14 10:20:30"),
        );
        let facts = read(&path);
        assert_eq!(facts["model"], "FC3170");
        assert_eq!(facts["captured_at"], "2026-05-14T10:20:30");
    }

    #[test]
    fn damaged_or_foreign_files_give_nothing() {
        for bytes in [
            &b"not a photo"[..],
            &[0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x08, b'E', b'x'][..],
            &b"II\x2a\x00\xff\xff\xff\xff"[..],
        ] {
            let (_d, path) = write("x.jpg", bytes);
            assert!(read(&path).is_empty());
        }
        // A bad date is ignored, and without Make nor date there is nothing to say.
        let (_d, path) = write("y.jpg", &sample::jpeg("", "", "yesterday", None));
        assert!(read(&path).is_empty());
    }

    #[test]
    fn rounding_follows_python() {
        assert_eq!(round(29.09, 7), 29.09);
        assert_eq!(round(-110.958333333333, 7), -110.9583333);
        assert_eq!(
            round(2.675, 2),
            2.67,
            "binary 2.675 is below the tie, as in Python"
        );
    }
}
