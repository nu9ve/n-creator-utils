//! Device opinion for a still and for a unit. Port of `media/recipes.py` (`device_key`, `resolve`, `unit_device`).
//!
//! Identity is Make + Model + Serial from the file (never a filename, DCIM folder or card name). A device the user
//! confirmed (`device-registry` in the catalog) wins over the recipe's rules; a new flying model is suggested as a
//! drone once; an ambiguous brand (`review_makes`, DJI) without evidence asks.

use crate::photo_meta::PHOTO_SUFFIXES;
use crate::rules::slug;
use nu9ve_json::Result;
use serde_json::{Value, json};
use std::path::Path;

fn clean(part: Option<&str>) -> String {
    part.unwrap_or_default().trim().to_uppercase()
}

pub fn device_key(make: Option<&str>, model: Option<&str>, serial: Option<&str>) -> String {
    [make, model, serial].map(clean).join("|")
}

fn same(a: Option<&str>, b: Option<&str>) -> bool {
    clean(a) == clean(b)
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value[key].as_str()
}

/// Python `resolve(meta, catalog, value)`: `None` without a Make. `registry` is the catalog's `device-registry`.
pub fn resolve(meta: &Value, registry: Option<&Value>, recipe: &Value) -> Result<Option<Value>> {
    let make = text(meta, "make").filter(|m| !m.is_empty());
    let Some(make) = make else { return Ok(None) };
    let (model, serial) = (text(meta, "model"), text(meta, "serial"));
    let key = device_key(Some(make), model, serial);
    let mut device = json!({"make": make, "model": model, "serial": serial, "key": key});
    let mut decide = |category: Value, source: Value, label: Value, rule: &str, ask: bool| {
        let d = device.as_object_mut().expect("object");
        d.insert("category".into(), category);
        d.insert("source".into(), source);
        d.insert("label".into(), label);
        d.insert("rule".into(), json!(rule));
        d.insert("requires_confirmation".into(), json!(ask));
    };
    if let Some(known) = registry.and_then(|r| r.get(&key)).filter(|k| is_truthy(k)) {
        decide(
            known["category"].clone(),
            known["source"].clone(),
            label(known),
            "registry",
            false,
        );
        return Ok(Some(device));
    }
    let rules = recipe["devices"].as_array().cloned().unwrap_or_default();
    if let Some(rule) = rules
        .iter()
        .find(|r| same(text(r, "make"), Some(make)) && same(text(r, "model"), model))
    {
        decide(
            rule["category"].clone(),
            rule["source"].clone(),
            label(rule),
            "template",
            false,
        );
        return Ok(Some(device));
    }
    let evidence = &recipe["flight_evidence"];
    let flying = meta["flight"].as_object().is_some_and(|flight| {
        evidence["xmp"].as_array().is_some_and(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .any(|k| flight.contains_key(k))
        })
    });
    // Python `slug(model or make)`: an empty model falls back to the make.
    let source = slug(model.filter(|m| !m.is_empty()).unwrap_or(make))?
        .to_uppercase()
        .replace('-', "");
    if flying {
        // A new flying model: suggest the drone category once; the user confirms and it is registered.
        decide(
            evidence["category"].clone(),
            json!(source),
            Value::Null,
            "flight_evidence",
            true,
        );
        return Ok(Some(device));
    }
    let review = recipe["review_makes"]
        .as_array()
        .is_some_and(|makes| makes.iter().any(|m| same(m.as_str(), Some(make))));
    decide(Value::Null, json!(source), Value::Null, "unknown", review);
    Ok(Some(device))
}

fn label(entry: &Value) -> Value {
    entry.get("label").cloned().unwrap_or(Value::Null)
}

/// Python truthiness of a registry entry (`if known:`).
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Object(o) => !o.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
    }
}

/// Python `unit_device(members, …)`: one device for a unit (JPG+DNG pair), or a conflict when members disagree.
pub fn unit_device(
    members: &[Value],
    registry: Option<&Value>,
    recipe: &Value,
) -> Result<Option<Value>> {
    let found: Vec<&Value> = members
        .iter()
        .map(|f| &f["photo"])
        .filter(|p| is_truthy(p))
        .collect();
    let Some(first) = found.first() else {
        return Ok(None);
    };
    let keys: std::collections::BTreeSet<String> = found
        .iter()
        .map(|m| device_key(text(m, "make"), text(m, "model"), text(m, "serial")))
        .collect();
    if keys.len() != 1 {
        return Ok(Some(
            json!({"conflict": true, "requires_confirmation": true, "category": null}),
        ));
    }
    resolve(first, registry, recipe)
}

/// Python `volumes.card_device`: the device of the first still under DCIM, with one bounded EXIF read (at most
/// `limit` entries looked at, no deep scan on insertion). `None` without DCIM, without a readable still or on an
/// I/O error.
pub fn card_device(
    root: &Path,
    registry: Option<&Value>,
    recipe: &Value,
    limit: usize,
) -> Result<Option<Value>> {
    let Some(dcim) = ["DCIM", "dcim"]
        .iter()
        .map(|n| root.join(n))
        .find(|p| p.is_dir())
    else {
        return Ok(None);
    };
    let entries = |folder: &Path| -> Option<Vec<std::fs::DirEntry>> {
        let mut list: Vec<_> = std::fs::read_dir(folder)
            .ok()?
            .collect::<std::io::Result<_>>()
            .ok()?;
        list.sort_by_key(|e| e.file_name());
        Some(list)
    };
    let Some(top) = entries(&dcim) else {
        return Ok(None);
    };
    let folders = top.iter().filter(|e| {
        e.file_type().is_ok_and(|t| t.is_dir()) && !e.file_name().to_string_lossy().starts_with('.')
    });
    let mut seen = 0;
    for folder in folders {
        let Some(files) = entries(&folder.path()) else {
            return Ok(None);
        };
        for entry in files {
            seen += 1;
            if seen > limit {
                return Ok(None);
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                return Ok(None);
            };
            if !kind.is_file() || !PHOTO_SUFFIXES.contains(&crate::scan::suffix(&name).as_str()) {
                continue;
            }
            let meta = Value::Object(crate::photo_meta::read(&entry.path()));
            let Some(found) = resolve(&meta, registry, recipe)? else {
                continue;
            };
            let keep = [
                "make",
                "model",
                "source",
                "label",
                "category",
                "requires_confirmation",
            ];
            let mut device: serde_json::Map<String, Value> = keep
                .iter()
                .map(|k| ((*k).to_owned(), found[*k].clone()))
                .collect();
            let flow = recipe["missions"].as_array().and_then(|flows| {
                flows.iter().find(|f| {
                    f["devices"]
                        .as_array()
                        .is_some_and(|d| d.contains(&found["category"]))
                })
            });
            if let Some(flow) = flow
                && found["requires_confirmation"] != true
            {
                device.insert("mission_flow".into(), flow["label"].clone());
            }
            return Ok(Some(Value::Object(device)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_shows_the_first_still_device_and_its_mission_flow() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("DCIM/100MEDIA");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("DJI_0001.MP4"), b"v").unwrap();
        std::fs::write(folder.join("DJI_0002.JPG"), b"not exif").unwrap();
        let jpg = crate::photo_meta::sample::jpeg("DJI", "FC3170", "2026:05:14 10:20:30", None);
        std::fs::write(folder.join("DJI_0003.JPG"), &jpg).unwrap();
        let device = card_device(root.path(), None, &recipe(), 64)
            .unwrap()
            .unwrap();
        assert_eq!(device["source"], "MAVICAIR2");
        assert_eq!(device["requires_confirmation"], false);
        assert!(
            device["mission_flow"].is_string(),
            "the template has a mission flow for dji-drone"
        );
        assert!(device.get("rule").is_none());
        // The bound counts every entry looked at; past it, nothing is read.
        assert!(
            card_device(root.path(), None, &recipe(), 2)
                .unwrap()
                .is_none()
        );
        assert!(
            card_device(&root.path().join("DCIM"), None, &recipe(), 64)
                .unwrap()
                .is_none()
        );
    }

    fn recipe() -> Value {
        nuun_recipe::template()
    }

    #[test]
    fn a_known_drone_model_is_classified_by_the_template() {
        let d = resolve(&json!({"make": "DJI", "model": "FC3170"}), None, &recipe())
            .unwrap()
            .unwrap();
        assert_eq!(d["category"], "dji-drone");
        assert_eq!(d["source"], "MAVICAIR2");
        assert_eq!(d["rule"], "template");
        assert_eq!(d["requires_confirmation"], false);
        assert_eq!(d["key"], "DJI|FC3170|");
        assert!(d["serial"].is_null());
    }

    #[test]
    fn dji_without_flight_evidence_asks_and_flight_suggests_a_drone() {
        let still = resolve(&json!({"make": "dji", "model": "FC-8282"}), None, &recipe())
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                still["rule"].as_str(),
                still["requires_confirmation"].as_bool()
            ),
            (Some("unknown"), Some(true))
        );
        assert!(still["category"].is_null());
        assert_eq!(still["source"], "FC8282");
        let flying = json!({"make": "DJI", "model": "FC9", "flight": {"gimbal_pitch": -90.0}});
        let d = resolve(&flying, None, &recipe()).unwrap().unwrap();
        assert_eq!(
            (d["rule"].as_str(), d["category"].as_str()),
            (Some("flight_evidence"), Some("dji-drone"))
        );
        assert_eq!(d["requires_confirmation"], true);
        // Another brand never asks by brand alone.
        let canon = resolve(
            &json!({"make": "Canon", "model": "EOS R6"}),
            None,
            &recipe(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                canon["source"].as_str(),
                canon["requires_confirmation"].as_bool()
            ),
            (Some("EOSR6"), Some(false))
        );
    }

    #[test]
    fn the_registry_wins_and_no_make_means_no_device() {
        let registry =
            json!({"DJI|FC3170|ABC": {"category": "dji-360", "source": "X", "label": "mine"}});
        let meta = json!({"make": "DJI", "model": "FC3170", "serial": " abc "});
        let d = resolve(&meta, Some(&registry), &recipe()).unwrap().unwrap();
        assert_eq!(
            (
                d["rule"].as_str(),
                d["category"].as_str(),
                d["label"].as_str()
            ),
            (Some("registry"), Some("dji-360"), Some("mine"))
        );
        assert!(
            resolve(
                &json!({"captured_at": "2026-01-01T00:00:00"}),
                None,
                &recipe()
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn a_pair_that_disagrees_is_a_conflict() {
        let member = |model: &str| json!({"path": "x", "photo": {"make": "DJI", "model": model}});
        let pair = [member("FC3170"), member("FC3170"), json!({"path": "y"})];
        assert_eq!(
            unit_device(&pair, None, &recipe()).unwrap().unwrap()["rule"],
            "template"
        );
        let conflict = unit_device(&[member("FC3170"), member("FC9")], None, &recipe())
            .unwrap()
            .unwrap();
        assert_eq!(
            conflict,
            json!({"conflict": true, "requires_confirmation": true, "category": null})
        );
        assert!(
            unit_device(&[json!({"path": "z"})], None, &recipe())
                .unwrap()
                .is_none()
        );
    }
}
