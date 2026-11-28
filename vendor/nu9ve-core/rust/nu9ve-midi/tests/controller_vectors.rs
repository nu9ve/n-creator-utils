//! data/midi/controller-vectors-v1.json: written with Nu9veMIDI (Swift) as the reference, passed here too.

use nu9ve_midi::{
    ActionState, Color, Gesture, Light, LightBoard, Message, Motion, Profile, Reader, Reading,
    hex_bytes,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/midi")
}

fn hex(messages: &[Vec<u8>]) -> Value {
    json!(
        messages
            .iter()
            .map(|m| m
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" "))
            .collect::<Vec<_>>()
    )
}

fn light(v: &Value) -> Light {
    let color = v["color"].as_str().map(|c| Color::from_name(c).unwrap());
    let motion: Motion = serde_json::from_value(v["motion"].clone()).unwrap_or_default();
    Light::new(color, v["dim"].as_bool().unwrap_or(false), motion)
}

fn light_json(l: Light) -> Value {
    json!({"color": l.color.map(Color::name), "dim": l.dim, "motion": l.motion})
}

fn lights(v: &Value) -> BTreeMap<String, Light> {
    v.as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), light(v)))
        .collect()
}

/// A profile of data/midi/ with the case's patch: top-level fields replaced and `lit` set per control.
fn profile(spec: &Value) -> Profile {
    let text = std::fs::read_to_string(data().join(spec["file"].as_str().unwrap())).unwrap();
    let mut json: Value = serde_json::from_str(&text).unwrap();
    if let Some(patch) = spec["patch"].as_object() {
        for (key, value) in patch {
            if key != "lit" {
                json[key] = value.clone();
            }
        }
        if let Some(lit) = patch.get("lit").and_then(Value::as_object) {
            for control in json["controls"].as_array_mut().unwrap() {
                if let Some(on) = lit.get(control["id"].as_str().unwrap()) {
                    control["lit"] = on.clone();
                }
            }
        }
    }
    Profile::from_json(&json.to_string()).unwrap()
}

fn reading_json(r: &Reading) -> Value {
    let gesture = match r.gesture {
        Gesture::Press => json!({"kind": "press"}),
        Gesture::Release => json!({"kind": "release"}),
        Gesture::Value(x) => json!({"kind": "value", "value": x}),
        Gesture::Step(d) => json!({"kind": "step", "delta": d}),
    };
    json!({"key": {"kind": r.key.kind, "channel": r.key.channel, "number": r.key.number},
           "control": r.control, "gesture": gesture})
}

/// JSON values with numbers compared by value: Swift writes `1` where Rust writes `1.0`.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn check(what: &str, expect: &Value, ours: Value) {
    assert!(
        same(expect, &ours),
        "{what}: expected {expect}, nu9ve-midi gives {ours}"
    );
}

#[test]
fn nu9ve_midi_gives_every_expectation_of_the_swift_reference() {
    let text = std::fs::read_to_string(data().join("controller-vectors-v1.json")).unwrap();
    let root: Value = serde_json::from_str(&text).unwrap();
    let profiles: BTreeMap<&str, Profile> = root["profiles"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.as_str(), profile(v)))
        .collect();
    let of = |c: &Value| &profiles[c["profile"].as_str().unwrap()];
    let mut count = 0;

    for c in root["lit"].as_array().unwrap() {
        let p = of(c);
        let ours = p.is_lit(p.control(c["control"].as_str().unwrap()).unwrap());
        check(&format!("lit {c}"), &c["expect"], json!(ours));
        count += 1;
    }
    for c in root["lights"].as_array().unwrap() {
        let p = of(c);
        let control = p.control(c["control"].as_str().unwrap()).unwrap();
        let ours = p.feedback.as_ref().unwrap().messages(
            control,
            &light(&c["light"]),
            c["phase"].as_bool().unwrap(),
        );
        check(&format!("light {c}"), &c["expect"], hex(&ours));
        count += 1;
    }
    for c in root["softwareBlink"].as_array().unwrap() {
        let ours = of(c)
            .feedback
            .as_ref()
            .unwrap()
            .blinks_in_software(&light(&c["light"]));
        check(&format!("blink {c}"), &c["expect"], json!(ours));
        count += 1;
    }
    for c in root["states"].as_array().unwrap() {
        let state: ActionState = serde_json::from_value(c["state"].clone()).unwrap();
        let color = Color::from_name(c["color"].as_str().unwrap()).unwrap();
        check(
            &format!("state {c}"),
            &c["expect"],
            light_json(Light::for_state(state, color)),
        );
        count += 1;
    }
    for b in root["boards"].as_array().unwrap() {
        let mut board = LightBoard::new(of(b).clone());
        for s in b["steps"].as_array().unwrap() {
            let ours = if let Some(update) = s.get("update") {
                hex(&board.update(&lights(update)))
            } else if s.get("forget").is_some() {
                hex(&board.forget())
            } else if s.get("darken").is_some() {
                hex(&board.darken())
            } else if let Some(phase) = s.get("blink").and_then(Value::as_bool) {
                hex(&board.blink(phase))
            } else {
                json!(board.needs_blink())
            };
            check(&format!("{} · {s}", b["name"]), &s["expect"], ours);
            count += 1;
        }
    }
    for r in root["readings"].as_array().unwrap() {
        let mut reader = Reader::new(r["profile"].as_str().map(|p| profiles[p].clone()));
        for s in r["steps"].as_array().unwrap() {
            let bytes = hex_bytes(s["bytes"].as_str().unwrap()).unwrap();
            let ours: Vec<Value> = Message::parse(&bytes)
                .map(|m| reader.read(m, s["at"].as_u64().unwrap()))
                .unwrap_or_default()
                .iter()
                .map(reading_json)
                .collect();
            check(&format!("{} · {s}", r["name"]), &s["expect"], json!(ours));
            count += 1;
        }
    }
    assert!(count >= 70, "only {count} cases");
}
