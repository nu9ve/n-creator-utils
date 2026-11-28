use serde::{Deserialize, Serialize};

/// The supported channel message families. Everything else (clock, SysEx, aftertouch) is ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Note On and Note Off. A Note On with velocity 0 is a release.
    Note,
    /// Control Change, 7 bits, or the MSB of a 14-bit pair.
    Cc,
    /// Pitch bend: one 14-bit value per channel, as keyboards' wheels and some faders send.
    PitchBend,
}
impl Kind {
    /// The status nibble stored with a binding.
    pub fn status(self) -> u8 {
        match self {
            Kind::Note => 0x90,
            Kind::Cc => 0xb0,
            Kind::PitchBend => 0xe0,
        }
    }
    pub fn from_status(status: u8) -> Option<Self> {
        match status & 0xf0 {
            0x80 | 0x90 => Some(Kind::Note),
            0xb0 => Some(Kind::Cc),
            0xe0 => Some(Kind::PitchBend),
            _ => None,
        }
    }
}

/// One physical control: what it sends, on which channel (0–15) and which number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub kind: Kind,
    pub channel: u8,
    /// Note or CC number; 0 for pitch bend.
    pub number: u8,
}
impl Key {
    /// How people read it in a manual: channels from 1, numbers in hex.
    pub fn label(&self) -> String {
        match self.kind {
            Kind::Note => format!("CH {} · NOTE 0x{:02X}", self.channel + 1, self.number),
            Kind::Cc => format!("CH {} · CC 0x{:02X}", self.channel + 1, self.number),
            Kind::PitchBend => format!("CH {} · PITCH BEND", self.channel + 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub key: Key,
    /// Velocity (0 when released), CC value 0–127, or pitch bend 0–16383.
    pub value: u16,
}
impl Message {
    /// Reads a channel message; anything else is `None`.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let [status, a, b, ..] = *bytes else {
            return None;
        };
        if a > 0x7f || b > 0x7f {
            return None;
        }
        let kind = Kind::from_status(status)?;
        let channel = status & 0x0f;
        let (number, value) = match status & 0xf0 {
            0x80 => (a, 0),
            0xe0 => (0, (b as u16) << 7 | a as u16),
            _ => (a, b as u16),
        };
        Some(Self {
            key: Key {
                kind,
                channel,
                number,
            },
            value,
        })
    }
    /// A note held or a CC at or above the middle: how a button reads either.
    pub fn pressed(&self) -> bool {
        match self.key.kind {
            Kind::Note => self.value > 0,
            Kind::Cc => self.value >= 64,
            Kind::PitchBend => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_notes_ccs_and_pitch_bend() {
        let on = Message::parse(&[0x91, 0x0b, 0x7f]).unwrap();
        assert_eq!(on.key.kind, Kind::Note);
        assert_eq!(on.key.channel, 1);
        assert!(on.pressed());
        // Both ways a controller says "released".
        assert!(!Message::parse(&[0x91, 0x0b, 0x00]).unwrap().pressed());
        let off = Message::parse(&[0x81, 0x0b, 0x40]).unwrap();
        assert_eq!((off.key.kind, off.value), (Kind::Note, 0));
        let cc = Message::parse(&[0xb6, 0x1f, 0x40]).unwrap();
        assert_eq!(
            (cc.key.kind, cc.key.channel, cc.key.number),
            (Kind::Cc, 6, 0x1f)
        );
        let bend = Message::parse(&[0xe0, 0x00, 0x40]).unwrap();
        assert_eq!((bend.key.kind, bend.value), (Kind::PitchBend, 8192));
        assert_eq!(Message::parse(&[0xf8]), None);
        assert_eq!(Message::parse(&[0xa0, 1, 2]), None);
        assert_eq!(Message::parse(&[0x90, 0x80, 1]), None);
        assert_eq!(
            Message::parse(&[0x90, 0x0c, 1]).unwrap().key.label(),
            "CH 1 · NOTE 0x0C"
        );
    }
}
