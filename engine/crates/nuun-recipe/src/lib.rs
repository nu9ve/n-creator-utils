//! nüün's recipe: device and flow rules kept as data, not code.
//!
//! Port of `media/recipes.py` (TEMPLATE, `card_family`, `card_parents`, `recipe_ref`, `summary`) and of the
//! family part of `media/volumes.py` `card_hint`. `template.json` is a transcription of Python's `TEMPLATE`; the
//! golden test (`tests/golden.rs`) proves it by comparing its digest with the one Python computes.
//! Safety invariants (Sony indexes never deleted, cleanup only after verification) stay in code, not here.

use nu9ve_json::{Error, Result, digest, require};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const TEMPLATE: &str = include_str!("../template.json");

/// Camera categories the planner knows (media/planner.py `CAMERAS`).
pub const CAMERAS: [&str; 10] = [
    "sony",
    "canon",
    "nikon",
    "gopro",
    "h6",
    "dji-drone",
    "dji-360",
    "dji-action",
    "insta360",
    "generic",
];

/// The initial template (`nu9ve-default`).
pub fn template() -> Value {
    serde_json::from_str(TEMPLATE).expect("template.json is valid JSON")
}

/// Effective recipe: a saved one (same schema) inherits any section it lacks from the template; anything else is
/// the template. Python: `recipe(catalog)`.
pub fn effective(saved: Option<&Value>) -> Value {
    let mut value = template();
    if let (Some(Value::Object(saved)), Value::Object(base)) = (saved, &mut value)
        && saved.get("schema") == base.get("schema")
    {
        for (key, item) in saved {
            base.insert(key.clone(), item.clone());
        }
    }
    value
}

/// `{id, digest}` frozen into each plan.
pub fn recipe_ref(value: &Value) -> Value {
    json!({"id": value["id"], "digest": digest(value)})
}

type Second = BTreeMap<String, BTreeSet<String>>;

fn names_of(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

const CONDITIONS: [&str; 7] = [
    "top",
    "top_prefix",
    "top_suffix",
    "in",
    "dcim_prefix",
    "dcim_suffix",
    "dcim_regex",
];

fn clause_holds(
    clause: &Map<String, Value>,
    top: &BTreeSet<String>,
    second: &Second,
) -> Result<bool> {
    let unknown: BTreeSet<&str> = clause
        .keys()
        .map(String::as_str)
        .filter(|k| !CONDITIONS.contains(k))
        .collect();
    require(
        !clause.is_empty() && unknown.is_empty(),
        "invalid_recipe",
        format!(
            "Unknown card condition: {}",
            unknown.into_iter().collect::<Vec<_>>().join(", ")
        ),
    )?;
    let empty = BTreeSet::new();
    let dcim = second.get("dcim").unwrap_or(&empty);
    for (key, value) in clause {
        let holds = match key.as_str() {
            "top" => names_of(value).iter().any(|n| top.contains(*n)),
            "top_prefix" => top
                .iter()
                .any(|t| names_of(value).iter().any(|n| t.starts_with(n))),
            "top_suffix" => top
                .iter()
                .any(|t| names_of(value).iter().any(|n| t.ends_with(n))),
            "in" => value
                .as_object()
                .map(|folders| {
                    folders.iter().all(|(folder, names)| {
                        second.get(folder).is_some_and(|inside| {
                            names_of(names).iter().any(|n| inside.contains(*n))
                        })
                    })
                })
                .unwrap_or(false),
            "dcim_prefix" => dcim
                .iter()
                .any(|d| names_of(value).iter().any(|n| d.starts_with(n))),
            "dcim_suffix" => dcim
                .iter()
                .any(|d| names_of(value).iter().any(|n| d.ends_with(n))),
            "dcim_regex" => {
                let pattern = value.as_str().unwrap_or("");
                // Python re.fullmatch: anchored at both ends.
                let regex = regex::Regex::new(&format!("^(?:{pattern})$")).map_err(|e| {
                    Error::new("invalid_recipe", format!("Invalid dcim_regex: {e}"))
                })?;
                dcim.iter().any(|d| regex.is_match(d))
            }
            _ => unreachable!("checked above"),
        };
        if !holds {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Top-level folders whose children the card rules look at (always `dcim`).
pub fn card_parents(value: &Value) -> BTreeSet<String> {
    let mut parents: BTreeSet<String> = ["dcim".to_owned()].into();
    for card in value["cards"].as_array().into_iter().flatten() {
        for clause in card["any"].as_array().into_iter().flatten() {
            if let Some(folders) = clause["in"].as_object() {
                parents.extend(folders.keys().cloned());
            }
        }
    }
    parents
}

/// First card rule matching these lowercase names, or the default family. Lazy like Python: a broken rule after
/// the first match is not evaluated.
pub fn card_family<'a>(
    value: &'a Value,
    top: &BTreeSet<String>,
    second: &Second,
) -> Result<&'a Value> {
    for card in value["cards"].as_array().into_iter().flatten() {
        for clause in card["any"].as_array().into_iter().flatten() {
            let clause = clause
                .as_object()
                .ok_or_else(|| Error::new("invalid_recipe", "A card clause must be an object"))?;
            if clause_holds(clause, top, second)? {
                return Ok(card);
            }
        }
    }
    Ok(&value["card_default"])
}

/// Lowercase names in a folder; dot files are skipped except `.stalker` bundles. Unreadable folders are empty.
fn names(path: &Path) -> BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return BTreeSet::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .filter(|n| !n.starts_with('.') || n.ends_with(".stalker"))
        .collect()
}

/// The family rule for a mounted card, from its top two directory levels (volumes.py `card_hint`).
/// Folders with content decide first: an empty residue (100CANON left by another camera) must not win over
/// 101MEDIA full of photos; a freshly formatted card (only empty folders) still gets its family.
/// Assumes a case-insensitive filesystem, as Python does (it joins lowercase names): true for exFAT, FAT32, NTFS
/// and APFS/HFS+ default volumes.
pub fn card_rule<'a>(root: &Path, value: &'a Value) -> Result<&'a Value> {
    let parents = card_parents(value);
    let top = names(root);
    let mut second = Second::new();
    if top.iter().any(|t| parents.contains(t))
        && let Ok(entries) = std::fs::read_dir(root)
    {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if parents.contains(&name) && is_dir {
                second.insert(name, names(&entry.path()));
            }
        }
    }
    let filled: Second = second
        .iter()
        .map(|(key, values)| {
            let kept = values
                .iter()
                .filter(|n| {
                    let path = root.join(key).join(n.as_str());
                    !names(&path).is_empty() || !path.is_dir()
                })
                .cloned()
                .collect();
            (key.clone(), kept)
        })
        .collect();
    let rule = card_family(value, &top, &filled)?;
    if std::ptr::eq(rule, &value["card_default"]) {
        card_family(value, &top, &second)
    } else {
        Ok(rule)
    }
}

/// Read-only view of the effective recipe for Flujos (Python `summary`). `confirmed_devices` comes from the
/// catalog's device registry; the Rust engine has no catalog yet, so callers pass what they know.
pub fn summary(value: &Value, confirmed_devices: usize) -> Value {
    let pick = |item: &Value, keys: &[&str]| -> Value {
        Value::Object(
            keys.iter()
                .map(|k| {
                    (
                        (*k).to_owned(),
                        item.get(*k).cloned().unwrap_or(Value::Null),
                    )
                })
                .collect(),
        )
    };
    let mut categories: BTreeSet<String> = CAMERAS.iter().map(|c| (*c).to_owned()).collect();
    if let Some(extra) = value["categories"].as_object() {
        categories.extend(extra.keys().cloned());
    }
    let list = |key: &str, keys: &[&str]| -> Value {
        Value::Array(
            value[key]
                .as_array()
                .into_iter()
                .flatten()
                .map(|i| pick(i, keys))
                .collect(),
        )
    };
    json!({
        "recipe": recipe_ref(value),
        "name": value["name"],
        "cards": list("cards", &["family", "label", "recommendation", "cleanup"]),
        "devices": list("devices", &["make", "model", "label", "category", "source"]),
        "confirmed_devices": confirmed_devices,
        "categories": categories.into_iter().collect::<Vec<_>>(),
        "missions": list("missions", &["id", "label", "project", "devices", "archive_folder", "work_folder"]),
    })
}

/// FUENTE tag for a camera model read from its own metadata.
pub fn model_source<'a>(value: &'a Value, model: &str) -> Option<&'a str> {
    value["models"][model].as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port of tests/test_recipe_cards.py `family_of`: folders with one file each, then the card's family.
    fn family_of(folders: &[&str], value: Option<&Value>) -> String {
        let root = tempfile::tempdir().unwrap();
        for name in folders {
            let path = root.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("x"), b"1").unwrap();
        }
        let owned;
        let value = match value {
            Some(v) => v,
            None => {
                owned = template();
                &owned
            }
        };
        card_rule(root.path(), value).unwrap()["family"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn template_reproduces_builtin_families() {
        let cases: [(&str, &[&str]); 8] = [
            ("stalker", &["S_1.stalker"]),
            ("sony", &["PRIVATE/M4ROOT/CLIP"]),
            ("h6", &["ZOOM0001"]),
            ("gopro", &["DCIM/100GOPRO"]),
            ("insta360", &["DCIM/Camera01", "MISC"]),
            ("dji", &["DCIM/101MEDIA", "MISC"]),
            ("canon", &["DCIM/100CANON"]),
            ("generic", &["DCIM/100MEDIA"]),
        ];
        for (family, folders) in cases {
            assert_eq!(family_of(folders, None), family, "{folders:?}");
        }
        assert_eq!(family_of(&["AVF_INFO"], None), "sony");
        assert_eq!(family_of(&["DJI_001"], None), "dji");
    }

    #[test]
    fn order_and_recommendations_are_data() {
        let value = template();
        let top: BTreeSet<String> = ["private".to_owned()].into();
        let second: Second = [("private".to_owned(), ["m4root".to_owned()].into())].into();
        let rule = card_family(&value, &top, &second).unwrap();
        assert_eq!(
            (
                rule["family"].as_str(),
                rule["recommendation"].as_str(),
                rule["cleanup"].as_str()
            ),
            (
                Some("sony"),
                Some("sony_format_in_camera"),
                Some("sony_clips_then_format")
            )
        );
        assert!(std::ptr::eq(
            card_family(&value, &BTreeSet::new(), &Second::new()).unwrap(),
            &value["card_default"]
        ));
    }

    #[test]
    fn a_new_camera_is_a_recipe_entry_not_code() {
        let mut value = template();
        value["cards"].as_array_mut().unwrap().insert(0, json!({"family": "blackmagic", "label": "Blackmagic", "recommendation": "archive_and_work", "any": [{"top_prefix": ["blackmagic"]}]}));
        assert_eq!(family_of(&["BlackmagicPocket"], Some(&value)), "blackmagic");
        assert_eq!(family_of(&["BlackmagicPocket"], None), "generic");
    }

    #[test]
    fn unknown_condition_is_rejected() {
        let mut value = template();
        value["cards"] = json!([{"family": "x", "recommendation": "archive_and_work", "any": [{"top_glob": ["*"]}]}]);
        let error = card_family(&value, &["a".to_owned()].into(), &Second::new()).unwrap_err();
        assert_eq!(error.code, "invalid_recipe");
        assert_eq!(error.message, "Unknown card condition: top_glob");
    }

    #[test]
    fn saved_recipe_without_cards_inherits_template_sections() {
        let mut saved = template();
        for key in ["cards", "card_default", "models", "family_source"] {
            saved.as_object_mut().unwrap().remove(key);
        }
        let effective = effective(Some(&saved));
        assert_eq!(effective["cards"], template()["cards"]);
        assert_eq!(model_source(&effective, "ILCE-6400"), Some("SONYA6400"));
    }

    #[test]
    fn empty_residue_does_not_win_over_filled_folders() {
        // An empty 100GOPRO left by another camera must not win over an Insta360's full Camera01: GoPro comes
        // first in the recipe, so without looking at content first the card would be called GoPro.
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("DCIM/100GOPRO")).unwrap();
        std::fs::create_dir_all(root.path().join("DCIM/Camera01")).unwrap();
        std::fs::create_dir_all(root.path().join("MISC")).unwrap();
        std::fs::write(root.path().join("DCIM/Camera01/a.insv"), b"1").unwrap();
        assert_eq!(
            card_rule(root.path(), &template()).unwrap()["family"],
            "insta360"
        );
        // When no filled folder matches, every folder counts again (Python's second pass): an empty residue then
        // names the card, as on a freshly formatted one.
        let residue = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(residue.path().join("DCIM/100CANON")).unwrap();
        std::fs::create_dir_all(residue.path().join("DCIM/101MEDIA")).unwrap();
        std::fs::write(residue.path().join("DCIM/101MEDIA/a.jpg"), b"1").unwrap();
        assert_eq!(
            card_rule(residue.path(), &template()).unwrap()["family"],
            "canon"
        );
        // A freshly formatted card with only empty folders still gets its family.
        let fresh = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fresh.path().join("DCIM/100CANON")).unwrap();
        assert_eq!(
            card_rule(fresh.path(), &template()).unwrap()["family"],
            "canon"
        );
    }

    #[test]
    fn summary_has_the_rpc_shape() {
        let data = summary(&template(), 0);
        assert_eq!(data["recipe"]["id"], "nu9ve-default");
        let families: Vec<&str> = data["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["family"].as_str().unwrap())
            .collect();
        assert_eq!(
            families,
            ["stalker", "sony", "h6", "gopro", "insta360", "dji", "canon"]
        );
        assert_eq!(data["missions"][0]["id"], "subaereo");
        assert!(
            data["categories"]
                .as_array()
                .unwrap()
                .contains(&json!("dji-drone"))
        );
        assert_eq!(data["cards"][0]["cleanup"], Value::Null);
    }
}
