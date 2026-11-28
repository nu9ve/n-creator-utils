//! The controller map of contracts/nu9ve-controller-map-v1.md §3 (control → an action id of the app's
//! catalog), with the two optional fields of nu9ve-surface-v1 §6: `zones` and `modes`. Same shape as
//! `MIDIMapFile` in `Nu9veMIDI`. Unknown fields are ignored, so a newer minor version still reads.

use crate::{Color, Key, Kind, Profile};
use serde::{Deserialize, Deserializer, Serialize};

/// A raw address with the channel from 1, as in profiles and manuals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address {
    pub kind: Kind,
    pub channel: u8,
    #[serde(deserialize_with = "map_number")]
    pub number: u8,
}

impl Address {
    pub fn key(&self) -> Key {
        Key {
            kind: self.kind,
            channel: self.channel.saturating_sub(1),
            number: self.number,
        }
    }
}

fn map_number<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum N {
        Int(u8),
        Text(String),
    }
    let n = match N::deserialize(d)? {
        N::Int(n) => n,
        N::Text(t) => {
            let t = t.trim();
            match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(hex) => u8::from_str_radix(hex, 16),
                None => t.parse(),
            }
            .map_err(|_| serde::de::Error::custom(format!("not a MIDI number: {t}")))?
        }
    };
    if n > 0x7f {
        return Err(serde::de::Error::custom("MIDI numbers end at 0x7F"));
    }
    Ok(n)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    /// The control's id in the profile (the primary one for a mirror).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// A raw address, for a controller without a profile or a control the profile lacks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Address>,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    /// Its light's color, instead of the action's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

/// Controls that belong to the app even without the page (nu9ve-surface-v1 §5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    pub when: ZoneWhen,
    pub controls: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneWhen {
    Always,
    /// While this app records (its file of nu9ve-recording-v1 is live in `recording`).
    Recording,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Modes {
    /// Replaces the mappings of those controls while the suite records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<ModeMappings>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModeMappings {
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceMap {
    /// Substring of the port name, ignoring case.
    #[serde(rename = "match")]
    pub matches: String,
    pub profile: Option<String>,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<Zone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modes: Option<Modes>,
}

impl DeviceMap {
    pub fn matches_port(&self, port: &str) -> bool {
        port.to_lowercase().contains(&self.matches.to_lowercase())
    }

    /// The mappings in effect: in recording mode, `modes.recording` replaces those of its controls.
    pub fn effective(&self, recording: bool) -> Vec<&Mapping> {
        let replaced = self
            .modes
            .as_ref()
            .and_then(|m| m.recording.as_ref())
            .filter(|_| recording);
        let Some(replaced) = replaced else {
            return self.mappings.iter().collect();
        };
        let same = |a: &Mapping, b: &Mapping| {
            (a.control.is_some() && a.control == b.control) || (a.key.is_some() && a.key == b.key)
        };
        self.mappings
            .iter()
            .filter(|m| !replaced.mappings.iter().any(|r| same(m, r)))
            .chain(replaced.mappings.iter())
            .collect()
    }

    /// The mapping a reading belongs to: by the profile's control (the primary), then by raw address.
    /// A repeated control or address keeps the first (controller-map §3).
    pub fn mapping<'a>(
        &self,
        mappings: &[&'a Mapping],
        control: Option<&str>,
        key: Key,
    ) -> Option<&'a Mapping> {
        control
            .and_then(|id| mappings.iter().find(|m| m.control.as_deref() == Some(id)))
            .or_else(|| {
                mappings
                    .iter()
                    .find(|m| m.key.is_some_and(|a| a.key() == key))
            })
            .copied()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapFile {
    pub schema_version: u32,
    /// Off: the app does not want the controller at all.
    pub enabled: bool,
    #[serde(default)]
    pub devices: Vec<DeviceMap>,
}

impl MapFile {
    pub const VERSION: u32 = 1;

    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if file.schema_version != Self::VERSION {
            return Err(format!(
                "unsupported controller map version {}",
                file.schema_version
            ));
        }
        Ok(file)
    }

    pub fn device(&self, port: &str) -> Option<&DeviceMap> {
        self.devices.iter().find(|d| d.matches_port(port))
    }
}

/// The profiles that ship with the crate (data/midi/), valid ones only.
pub fn bundled_profiles() -> Vec<Profile> {
    [
        crate::profile::NOVATION_LAUNCHPAD_PRO_V2,
        crate::profile::PIONEER_DDJ_SB_V1,
    ]
    .iter()
    .filter_map(|t| Profile::from_json(t).ok())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contract_example_reads_against_the_profiles() {
        let text = include_str!("../../../data/midi/controller-map-example-v1.json");
        let file = MapFile::from_json(text).unwrap();
        assert!(file.enabled && file.devices.len() == 3);
        let profiles = bundled_profiles();
        for device in &file.devices {
            let profile = device.profile.as_ref().map(|id| {
                profiles
                    .iter()
                    .find(|p| &p.id == id)
                    .expect("known profile")
            });
            for m in &device.mappings {
                assert!(m.control.is_some() || m.key.is_some(), "{}", m.action);
                if let Some(c) = &m.control {
                    assert!(profile.and_then(|p| p.control(c)).is_some(), "{c}");
                }
            }
        }
        let pad = file.device("Launchpad Pro Standalone Port").unwrap();
        let all = pad.effective(false);
        let key = Key {
            kind: Kind::Cc,
            channel: 0,
            number: 0x0a,
        };
        assert_eq!(
            pad.mapping(&all, Some("left-8"), key).unwrap().action,
            "recording.toggle"
        );
        assert_eq!(pad.mappings.last().unwrap().color, Some(Color::Cyan));
        let generic = file.device("Generic Keys 1").unwrap();
        let key = Key {
            kind: Kind::Cc,
            channel: 15,
            number: 7,
        };
        assert_eq!(
            generic
                .mapping(&generic.effective(false), None, key)
                .unwrap()
                .action,
            "mix.volume"
        );
    }

    #[test]
    fn recording_mode_replaces_only_its_controls() {
        let text = r#"{"schemaVersion":1,"enabled":true,"devices":[{"match":"Launchpad","profile":"novation-launchpad-pro",
          "mappings":[{"control":"pad-1-1","action":"a"},{"control":"pad-1-2","action":"b"}],
          "zones":[{"when":"recording","controls":["pad-1-1"]}],
          "modes":{"recording":{"mappings":[{"control":"pad-1-2","action":"c"}]}},"later":1}]}"#;
        let file = MapFile::from_json(text).unwrap();
        let d = &file.devices[0];
        assert_eq!(d.zones[0].when, ZoneWhen::Recording);
        let actions = |r| {
            d.effective(r)
                .iter()
                .map(|m| m.action.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(actions(false), ["a", "b"]);
        assert_eq!(actions(true), ["a", "c"]);
        assert!(
            MapFile::from_json(&text.replace("\"schemaVersion\":1", "\"schemaVersion\":2"))
                .is_err()
        );
    }
}
