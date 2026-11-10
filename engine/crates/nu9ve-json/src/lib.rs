//! Canonical JSON, digests and errors shared by the suite's Rust engines.
//!
//! nu9ve-mcp-v1 / rust-engine.md §6 · origin: n-creator-utils `media/common.py` (`canonical`, `digest`,
//! `MediaError`). A plan's digest is SHA-256 of its canonical JSON, so the Rust engine must produce the
//! **same bytes** as Python's
//! `json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False)`:
//! keys sorted by code point, no spaces, non-ASCII kept as UTF-8, Python's escapes, and Python's `repr`
//! for floats (`1e-07`, `1e+16`, `1.0`). serde_json alone writes `1e-7` and `1e16`.
//!
//! Limit: integers must fit in i64/u64 (Python allows any size; serde_json would read a larger one as a float).

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

/// An error with a stable code, as Python's `MediaError` and nove's engine errors (`{code, message}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Python's `require(condition, code, message)`.
pub fn require(condition: bool, code: &str, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::new(code, message))
    }
}

/// Canonical bytes of a JSON value, identical to nüün's Python `canonical`.
pub fn canonical(value: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_value(&mut out, value);
    out.into_bytes()
}

/// SHA-256 hex of the canonical bytes (Python's `digest`).
pub fn digest(value: &Value) -> String {
    hex(&Sha256::digest(canonical(value)))
}

pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        text.push(DIGITS[(b >> 4) as usize] as char);
        text.push(DIGITS[(b & 15) as usize] as char);
    }
    text
}

/// The one-line envelope of `rpc-serve` (media/cli.py `serve_reads`): `{id, ok: true, data}`.
pub fn ok_envelope(id: &Value, data: Value) -> Value {
    serde_json::json!({"id": id, "ok": true, "data": data})
}

/// `{id, ok: false, error: {code, message}}`.
pub fn error_envelope(id: &Value, error: &Error) -> Value {
    serde_json::json!({"id": id, "ok": false, "error": {"code": error.code, "message": error.message}})
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                out.push_str(&python_float_repr(n.as_f64().unwrap_or(f64::NAN)));
            }
        }
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // Python sorts str keys by code point; UTF-8 byte order is the same order.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, &map[key.as_str()]);
            }
            out.push('}');
        }
    }
}

/// Python json's escapes with ensure_ascii=False: `"`, `\`, `\n`, `\r`, `\t`, `\b`, `\f`, and `\u00xx`
/// (lowercase) for the remaining control characters below 0x20. Everything else is written as is.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// CPython's `float.__repr__` (`float_repr_style == 'short'`): the shortest digits that round-trip, written
/// in fixed notation unless the decimal point position is ≤ -4 or > 16, with a signed exponent of at least two
/// digits (`1e-07`, `1.5e+300`) and `.0` on integral values in fixed notation.
pub fn python_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    // Rust's `{:e}` gives the shortest round-trip digits: "1.2345e-7", "-0e0", "1e16".
    let sci = format!("{x:e}");
    let (sign, sci) = match sci.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", sci.as_str()),
    };
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let decpt = exponent + 1;
    let n = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        let (first, rest) = digits.split_at(1);
        let mantissa = if rest.is_empty() {
            first.to_owned()
        } else {
            format!("{first}.{rest}")
        };
        let exp = decpt - 1;
        let exp_sign = if exp < 0 { '-' } else { '+' };
        format!("{sign}{mantissa}e{exp_sign}{:02}", exp.abs())
    } else if decpt <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt >= n {
        format!("{sign}{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let (int, frac) = digits.split_at(decpt as usize);
        format!("{sign}{int}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn floats_follow_python_repr() {
        // Values and their `repr` in CPython 3.9+.
        let cases = [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (0.6, "0.6"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (1e-7, "1e-07"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (1234567890123456.0, "1234567890123456.0"),
            (12345678901234567.0, "1.2345678901234568e+16"),
            (0.12345678901234568, "0.12345678901234568"),
            (1.5e300, "1.5e+300"),
            (-2.5e-310, "-2.5e-310"),
            (123.456, "123.456"),
            (100.0, "100.0"),
            (1e22, "1e+22"),
        ];
        for (value, expected) in cases {
            assert_eq!(python_float_repr(value), expected, "{value:e}");
        }
    }

    #[test]
    fn canonical_sorts_keys_without_spaces_and_keeps_utf8() {
        let value = json!({"b": [1, 2.0, "ñ"], "a": {"z": null, "y": true}, "Á": "x"});
        assert_eq!(
            String::from_utf8(canonical(&value)).unwrap(),
            r#"{"a":{"y":true,"z":null},"b":[1,2.0,"ñ"],"Á":"x"}"#
        );
    }

    #[test]
    fn strings_use_python_escapes() {
        let value = json!("a\"b\\c\n\r\t\u{08}\u{0c}\u{01}\u{1f}/\u{7f}\u{2028}é");
        assert_eq!(
            String::from_utf8(canonical(&value)).unwrap(),
            "\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001\\u001f/\u{7f}\u{2028}é\""
        );
    }

    #[test]
    fn large_unsigned_integers_stay_integers() {
        // The native transport fixture (tests/fixtures/native_plan_backend.py) uses 2**64 - 1.
        let value: Value = serde_json::from_str(
            "{\"unsigned\": 18446744073709551615, \"integral_float\": 1.0, \"small\": 1e-7}",
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(canonical(&value)).unwrap(),
            r#"{"integral_float":1.0,"small":1e-07,"unsigned":18446744073709551615}"#
        );
    }

    #[test]
    fn digest_is_sha256_of_canonical_bytes() {
        // sha256(b'{"a":1}') — independent of this crate's writer.
        assert_eq!(
            digest(&json!({"a": 1})),
            "015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"
        );
    }

    #[test]
    fn envelopes_match_rpc_serve() {
        let ok = ok_envelope(&json!(3), json!([1]));
        assert_eq!(ok["ok"], json!(true));
        let error = error_envelope(&json!(null), &Error::new("read_only", "no"));
        assert_eq!(error["error"]["code"], json!("read_only"));
    }
}

/// Random bytes from the operating system's generator.
pub fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
        };
        // SAFETY: valid buffer and length; the system-preferred generator needs no algorithm handle.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                N as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        require(
            status == 0,
            "random_unavailable",
            format!("BCryptGenRandom failed: {status:#x}"),
        )?;
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|e| Error::new("random_unavailable", e.to_string()))?;
    }
    Ok(bytes)
}

/// A random UUID version 4 in Python's `str(uuid.uuid4())` form (`media/common.py` `uid`).
pub fn uid() -> Result<String> {
    let mut b = random_bytes::<16>()?;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    let h = hex(&b);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn uids_are_version_4_and_differ() {
        let a = uid().unwrap();
        let b = uid().unwrap();
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(&parts[3][..1], "8" | "9" | "a" | "b"));
        assert!(
            a.chars()
                .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}

/// SHA-1 (FIPS 180-4), only for name-based UUIDs: Python's `uuid.uuid5` hashes with it. Never for integrity.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend(((data.len() as u64) * 8).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(v);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// `uuid.NAMESPACE_URL`.
pub const NAMESPACE_URL: [u8; 16] = [
    0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
];

/// Python `str(uuid.uuid5(namespace, name))`.
pub fn uuid5(namespace: &[u8; 16], name: &str) -> String {
    let mut data = namespace.to_vec();
    data.extend(name.as_bytes());
    let hash = sha1(&data);
    let mut b = [0u8; 16];
    b.copy_from_slice(&hash[..16]);
    b[6] = (b[6] & 0x0F) | 0x50;
    b[8] = (b[8] & 0x3F) | 0x80;
    let h = hex(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

#[cfg(test)]
mod uuid5_tests {
    use super::*;

    #[test]
    fn sha1_and_uuid5_match_known_vectors() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        let long = vec![b'a'; 1000];
        assert_eq!(
            hex(&sha1(&long)),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
        // Python: str(uuid.uuid5(uuid.NAMESPACE_URL, 'http://python.org/'))
        assert_eq!(
            uuid5(&NAMESPACE_URL, "http://python.org/"),
            "4c565f0d-3f5a-5890-b41b-20cf47701c5e"
        );
        // Python docs: uuid.uuid5(uuid.NAMESPACE_DNS, 'python.org')
        let dns = [
            0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4,
            0x30, 0xc8,
        ];
        assert_eq!(
            uuid5(&dns, "python.org"),
            "886313e1-3b8a-5372-9b90-0c9aee199e5d"
        );
    }
}
