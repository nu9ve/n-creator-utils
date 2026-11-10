//! Validation rules from `media/common.py` that writes depend on.

use nu9ve_json::{Result, require};

/// Python `slug(value)`: at most 160 characters; runs of non-word characters become `-`, then `-`/`_` are
/// stripped at both ends, and something must remain. Returns the cleaned form. Python normalizes to NFC first;
/// that never changes whether a letter or digit is present, which is all a validation needs.
pub fn slug(value: &str) -> Result<String> {
    require(
        value.chars().count() <= 160,
        "invalid_tag",
        "Invalid project/profile tag",
    )?;
    let mut cleaned = String::new();
    let mut in_gap = false;
    for c in value.trim().chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            if in_gap {
                cleaned.push('-');
                in_gap = false;
            }
            cleaned.push(c);
        } else {
            in_gap = true;
        }
    }
    if in_gap {
        cleaned.push('-');
    }
    let cleaned = cleaned.trim_matches(|c| c == '-' || c == '_').to_owned();
    require(
        !cleaned.is_empty() && cleaned != "." && cleaned != "..",
        "invalid_tag",
        "Project/profile cannot be empty",
    )?;
    Ok(cleaned)
}

/// Python `relative(value)`: a path below a root, with `/`, no `\`, `:`, NUL, absolute start or `.`/`..` parts.
pub fn relative(value: &str) -> Result<&str> {
    require(
        !value.is_empty() && !value.contains('\\') && !value.contains(':') && !value.contains('\0'),
        "unsafe_path",
        "Invalid relative path",
    )?;
    require(
        !value.starts_with('/')
            && value
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."),
        "unsafe_path",
        "Path must remain within its root",
    )?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_cleans_like_python() {
        assert_eq!(slug("  Viaje a Sonora!  ").unwrap(), "Viaje-a-Sonora");
        assert_eq!(slug("_a_").unwrap(), "a");
        assert_eq!(slug("MEDIA502").unwrap(), "MEDIA502");
        assert_eq!(slug("ñandú").unwrap(), "ñandú");
        for bad in ["", "-_-", "...", "  "] {
            assert_eq!(slug(bad).unwrap_err().code, "invalid_tag", "{bad:?}");
        }
        assert!(slug(&"a".repeat(161)).is_err());
    }

    #[test]
    fn relative_paths_stay_inside_their_root() {
        assert!(relative("001_raw").is_ok() && relative("001_proyectos/e64_raw").is_ok());
        for bad in ["", "/abs", "a/../b", "a//b", "./a", "a\\b", "C:x", "a\0"] {
            assert_eq!(relative(bad).unwrap_err().code, "unsafe_path", "{bad:?}");
        }
    }
}
