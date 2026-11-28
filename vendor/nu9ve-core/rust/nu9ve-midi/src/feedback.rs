//! Output to a controller's lights: profile v2 `feedback` (contracts/nu9ve-controller-map-v1.md §1 and §2).
//! An app asks for a light by meaning (a color, dim or bright, still or moving) and the profile says which
//! bytes draw it. [`LightBoard`] sends only what changed. Same cases as `Nu9veMIDI` in Swift
//! (`data/midi/controller-vectors-v1.json`).

use crate::{Control, Kind, Profile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Colors an app can ask for. A profile maps each to the closest value it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Color {
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
    White,
}

impl Color {
    pub const ALL: [Color; 9] = [
        Color::Red,
        Color::Orange,
        Color::Yellow,
        Color::Green,
        Color::Cyan,
        Color::Blue,
        Color::Purple,
        Color::Pink,
        Color::White,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Orange => "orange",
            Color::Yellow => "yellow",
            Color::Green => "green",
            Color::Cyan => "cyan",
            Color::Blue => "blue",
            Color::Purple => "purple",
            Color::Pink => "pink",
            Color::White => "white",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    #[default]
    Solid,
    Flash,
    Pulse,
}

/// A light by its meaning. `color: None` is off, and an off light never moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Light {
    pub color: Option<Color>,
    pub dim: bool,
    pub motion: Motion,
}

impl Light {
    pub const OFF: Light = Light {
        color: None,
        dim: false,
        motion: Motion::Solid,
    };

    pub fn new(color: Option<Color>, dim: bool, motion: Motion) -> Self {
        Self {
            color,
            dim: color.is_some() && dim,
            motion: if color.is_some() {
                motion
            } else {
                Motion::Solid
            },
        }
    }

    pub fn solid(color: Color) -> Self {
        Self::new(Some(color), false, Motion::Solid)
    }

    pub fn is_off(&self) -> bool {
        self.color.is_none()
    }

    /// The default light of an action's state (nu9ve-surface-v1 §3): off dark, available dim,
    /// active bright and warning flashing orange. `color` is the action's or the mapping's.
    pub fn for_state(state: ActionState, color: Color) -> Self {
        match state {
            ActionState::Off => Self::OFF,
            ActionState::Available => Self::new(Some(color), true, Motion::Solid),
            ActionState::Active => Self::solid(color),
            ActionState::Warning => Self::new(Some(Color::Orange), false, Motion::Flash),
        }
    }
}

/// The state of an action of an app's catalog (nu9ve-surface-v1 §3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionState {
    #[default]
    Off,
    Available,
    Active,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedbackKind {
    /// The value is a color from `palette` (Launchpad, APC).
    Palette,
    /// One color: any light is `on`, off is 0 (DJ controllers).
    OnOff,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feedback {
    pub kind: FeedbackKind,
    /// Every control lights unless it says `"lit": false`; otherwise only those with `"lit": true`.
    pub all_controls: bool,
    /// Hex messages sent when the device connects, for example the layout the addresses assume.
    #[serde(default, rename = "init", skip_serializing_if = "Vec::is_empty")]
    pub initialize: Vec<String>,
    /// `"red"` and `"red.dim"`; a missing dim falls back to the bright one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palette: BTreeMap<String, u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<u8>,
    /// Templates in hex with `{number}` and `{value}`. Without them the owner blinks in software.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Hex bytes separated by spaces: `"F0 00 20 29 F7"`.
pub fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for part in text.split_whitespace() {
        if part.len() > 2 {
            return None;
        }
        out.push(u8::from_str_radix(part, 16).ok()?);
    }
    (!out.is_empty()).then_some(out)
}

impl Feedback {
    pub fn validate(&self, profile: &str) -> Result<(), String> {
        if self.kind == FeedbackKind::Palette && self.palette.is_empty() {
            return Err(format!("{profile}: a palette feedback needs colors"));
        }
        for text in self
            .initialize
            .iter()
            .chain(self.flash.iter())
            .chain(self.pulse.iter())
        {
            let filled = text.replace("{number}", "00").replace("{value}", "00");
            if hex_bytes(&filled).is_none() {
                return Err(format!("{profile}: not hex bytes: {text}"));
            }
        }
        if self.palette.values().any(|v| *v > 0x7f) || self.on.is_some_and(|v| v > 0x7f) {
            return Err(format!("{profile}: light values end at 0x7F"));
        }
        Ok(())
    }

    pub fn init_messages(&self) -> Vec<Vec<u8>> {
        self.initialize
            .iter()
            .filter_map(|t| hex_bytes(t))
            .collect()
    }

    /// Whether this light needs its owner to switch it on and off over time.
    pub fn blinks_in_software(&self, light: &Light) -> bool {
        match light.motion {
            Motion::Solid => false,
            Motion::Flash => self.flash.is_none(),
            Motion::Pulse => self.pulse.is_none(),
        }
    }

    /// The value that draws a still light.
    pub fn value(&self, light: &Light) -> u8 {
        let Some(color) = light.color else { return 0 };
        match self.kind {
            FeedbackKind::OnOff => self.on.unwrap_or(0x7f),
            FeedbackKind::Palette => {
                let dim = light
                    .dim
                    .then(|| self.palette.get(&format!("{}.dim", color.name())))
                    .flatten();
                dim.or_else(|| self.palette.get(color.name()))
                    .or_else(|| self.palette.get("white"))
                    .copied()
                    .unwrap_or(0x7f)
            }
        }
    }

    /// The messages that draw `light` on `control`. `phase` is the software blink: on or off.
    pub fn messages(&self, control: &Control, light: &Light, phase: bool) -> Vec<Vec<u8>> {
        let key = control.key();
        let status = if key.kind == Kind::Cc { 0xb0 } else { 0x90 } | key.channel;
        let value = self.value(light);
        let still = vec![status, key.number, value];
        let template = match light.motion {
            Motion::Solid => return vec![still],
            Motion::Flash => &self.flash,
            Motion::Pulse => &self.pulse,
        };
        let Some(template) = template else {
            return vec![if phase {
                still
            } else {
                vec![status, key.number, 0]
            }];
        };
        let filled = template
            .replace("{number}", &format!("{:02X}", key.number))
            .replace("{value}", &format!("{value:02X}"));
        match hex_bytes(&filled) {
            Some(animated) => vec![still, animated],
            None => vec![still],
        }
    }
}

/// What the controller shows now. It sends only what changed, everything after a reconnect, and the
/// software blink for profiles that cannot blink by themselves. Control ids → lights.
#[derive(Clone, Debug)]
pub struct LightBoard {
    profile: Profile,
    sent: BTreeMap<String, Light>,
    wanted: BTreeMap<String, Light>,
}

impl LightBoard {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            sent: BTreeMap::new(),
            wanted: BTreeMap::new(),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Messages that take the controller from what was sent to `lights`. Lit controls missing from
    /// `lights` are switched off.
    pub fn update(&mut self, lights: &BTreeMap<String, Light>) -> Vec<Vec<u8>> {
        let Some(feedback) = &self.profile.feedback else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for control in &self.profile.controls {
            if !self.profile.is_lit(control) {
                continue;
            }
            let light = lights.get(&control.id).copied().unwrap_or(Light::OFF);
            self.wanted.insert(control.id.clone(), light);
            let before = self.sent.get(&control.id).copied();
            if before == Some(light) {
                continue;
            }
            // Off never needs to be sent to a light that was never lit.
            if !(light.is_off() && before.is_none()) {
                out.extend(feedback.messages(control, &light, true));
            }
            self.sent.insert(control.id.clone(), light);
        }
        out
    }

    /// After a (re)connect the device may show anything, from another app or from before: `init`,
    /// every light off, then what is wanted.
    pub fn forget(&mut self) -> Vec<Vec<u8>> {
        let Some(feedback) = &self.profile.feedback else {
            return Vec::new();
        };
        let mut out = feedback.init_messages();
        self.sent.clear();
        for control in &self.profile.controls {
            if self.profile.is_lit(control) {
                out.extend(feedback.messages(control, &Light::OFF, true));
                self.sent.insert(control.id.clone(), Light::OFF);
            }
        }
        let wanted = std::mem::take(&mut self.wanted);
        out.extend(self.update(&wanted));
        out
    }

    /// The next software blink step, only for the lights that need it.
    pub fn blink(&self, phase: bool) -> Vec<Vec<u8>> {
        let Some(feedback) = &self.profile.feedback else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for control in &self.profile.controls {
            if let Some(light) = self.sent.get(&control.id)
                && feedback.blinks_in_software(light)
            {
                out.extend(feedback.messages(control, light, phase));
            }
        }
        out
    }

    /// Whether a timer has to run: only while something blinks in software.
    pub fn needs_blink(&self) -> bool {
        self.profile
            .feedback
            .as_ref()
            .is_some_and(|f| self.sent.values().any(|l| f.blinks_in_software(l)))
    }

    /// Everything that was lit goes off, when the owner lets go of the controller.
    pub fn darken(&mut self) -> Vec<Vec<u8>> {
        let Some(feedback) = &self.profile.feedback else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for control in &self.profile.controls {
            if self.sent.get(&control.id).is_some_and(|l| !l.is_off()) {
                out.extend(feedback.messages(control, &Light::OFF, true));
            }
        }
        self.sent.clear();
        self.wanted.clear();
        out
    }
}
