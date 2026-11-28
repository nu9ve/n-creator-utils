//! Live reading: one message at a time into what the hand did (contracts/nu9ve-controller-map-v1.md §4).
//! Port of `MIDIReader` in `Nu9veMIDI`; both pass `data/midi/controller-vectors-v1.json`.

use crate::{Gate, Key, Kind, Message, Mode, Profile, combine_cc14, normalize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    Press,
    Release,
    /// An absolute control, 0–1, already through the dead band.
    Value(f64),
    /// A relative control, in steps with sign.
    Step(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The primary address of the control (the MSB of a 14-bit pair, the primary of a mirror).
    pub key: Key,
    pub control: Option<String>,
    pub gesture: Gesture,
}

/// The dead band when neither the mapping nor the profile gives one.
pub fn default_deadband(bits: u8) -> f64 {
    if bits >= 14 { 0.002 } else { 0. }
}

/// Keeps the last MSB of each 14-bit control, a gate per control and which buttons are down.
#[derive(Clone, Debug, Default)]
pub struct Reader {
    profile: Option<Profile>,
    msb: BTreeMap<Key, u16>,
    gates: BTreeMap<Key, Gate>,
    down: BTreeSet<Key>,
}

impl Reader {
    pub fn new(profile: Option<Profile>) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }

    pub fn profile(&self) -> Option<&Profile> {
        self.profile.as_ref()
    }

    pub fn read(&mut self, message: Message, at_ms: u64) -> Vec<Reading> {
        let key = message.key;
        // The low half of a 14-bit control completes the last MSB; one without an MSB is dropped.
        if key.kind == Kind::Cc
            && let Some(profile) = &self.profile
            && let Some(control) = profile.controls.iter().find(|c| {
                c.kind == Kind::Cc && c.lsb == Some(key.number) && c.key().channel == key.channel
            })
        {
            let primary = profile.primary(control);
            let Some(high) = self.msb.get(&control.key()).copied() else {
                return Vec::new();
            };
            let x = normalize(combine_cc14(high, message.value), 14);
            let deadband = control.deadband.unwrap_or(default_deadband(14));
            let (pkey, pid) = (primary.key(), primary.id.clone());
            return self.gated(pkey, Some(pid), x, at_ms, deadband);
        }
        let found = self.profile.as_ref().and_then(|p| {
            p.control_by_key(key).map(|c| {
                let primary = p.primary(c);
                (
                    c.mode,
                    c.lsb.is_some(),
                    c.deadband,
                    primary.key(),
                    primary.id.clone(),
                )
            })
        });
        let Some((mode, has_lsb, deadband, pkey, id)) = found else {
            return match key.kind {
                Kind::PitchBend => self.gated(
                    key,
                    None,
                    normalize(message.value, 14),
                    at_ms,
                    default_deadband(14),
                ),
                // Without a profile a CC may be a button or a knob: report both readings.
                Kind::Cc => {
                    let mut out = self.edge(key, None, message.pressed());
                    out.extend(self.gated(key, None, normalize(message.value, 7), at_ms, 0.));
                    out
                }
                Kind::Note => self.edge(key, None, message.pressed()),
            };
        };
        let id = Some(id);
        match mode {
            Mode::Relative { encoding } => {
                let delta = encoding.delta(message.value);
                if delta == 0 {
                    Vec::new()
                } else {
                    vec![Reading {
                        key: pkey,
                        control: id,
                        gesture: Gesture::Step(delta),
                    }]
                }
            }
            Mode::Absolute if has_lsb => {
                self.msb.insert(key, message.value);
                Vec::new()
            }
            Mode::Absolute if key.kind == Kind::PitchBend => {
                let x = normalize(message.value, 14);
                self.gated(pkey, id, x, at_ms, deadband.unwrap_or(default_deadband(14)))
            }
            Mode::Absolute => {
                let x = normalize(message.value, 7);
                self.gated(pkey, id, x, at_ms, deadband.unwrap_or(0.))
            }
            _ => self.edge(pkey, id, message.pressed()),
        }
    }

    fn edge(&mut self, key: Key, control: Option<String>, pressed: bool) -> Vec<Reading> {
        let gesture = if pressed && self.down.insert(key) {
            Gesture::Press
        } else if !pressed && self.down.remove(&key) {
            Gesture::Release
        } else {
            return Vec::new();
        };
        vec![Reading {
            key,
            control,
            gesture,
        }]
    }

    fn gated(
        &mut self,
        key: Key,
        control: Option<String>,
        x: f64,
        at_ms: u64,
        deadband: f64,
    ) -> Vec<Reading> {
        let gate = self.gates.entry(key).or_default();
        if gate.pass(x, at_ms, deadband) {
            vec![Reading {
                key,
                control,
                gesture: Gesture::Value(x),
            }]
        } else {
            Vec::new()
        }
    }
}
