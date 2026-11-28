use serde::{Deserialize, Serialize};

/// How a relative encoder (jog, endless knob, browse) writes a step into 7 bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    /// 64 is still, 65 is +1, 63 is −1. Pioneer and Numark jogs.
    #[default]
    Offset64,
    /// 1 is +1, 127 is −1. Pioneer's browse knob, many endless knobs.
    TwosComplement,
    /// Bit 6 is the sign: 1 is +1, 65 is −1.
    SignBit,
}
impl Encoding {
    pub fn delta(self, value: u16) -> i32 {
        let v = (value & 0x7f) as i32;
        match self {
            Encoding::Offset64 => v - 64,
            Encoding::TwosComplement => {
                if v >= 64 {
                    v - 128
                } else {
                    v
                }
            }
            Encoding::SignBit => {
                if v & 0x40 != 0 {
                    -(v & 0x3f)
                } else {
                    v & 0x3f
                }
            }
        }
    }
}

/// How a bound control is read. Resolution comes from the binding: with an LSB it is 14-bit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Mode {
    /// Legacy bindings without an explicit mode. The consumer chooses how to read the input.
    #[default]
    Auto,
    /// Pressed and released, by note or by CC.
    Button,
    /// A fader or knob with ends: 7-bit, or 14-bit with an LSB.
    Absolute,
    /// An endless encoder sending steps.
    Relative { encoding: Encoding },
}

/// Join the two 7-bit values from an MSB/LSB pair. The caller tracks when the LSB arrives.
pub fn combine_cc14(msb: u16, lsb: u16) -> u16 {
    (msb & 0x7f) << 7 | (lsb & 0x7f)
}

/// 0–1 from a raw value, with the centre detent (64 of 127, 8192 of 16383) at exactly 0.5.
/// EQs, filters and tempo rest there, so unity has to be reachable.
pub fn normalize(raw: u16, bits: u8) -> f64 {
    let max = if bits >= 14 { 16383. } else { 127. };
    let centre = (max + 1.) / 2.;
    let raw = (raw as f64).min(max);
    if raw <= centre {
        raw / centre * 0.5
    } else {
        0.5 + (raw - centre) / (max - centre) * 0.5
    }
}

/// Messages closer than this belong to one movement.
pub const MOTION_MS: u64 = 40;

/// Keeps a worn fader still. A control that sends now and then, by itself, only gets through
/// when it has drifted past `deadband` (of 0–1); one moving by hand sends every few
/// milliseconds and gets through at full resolution. Measured on a DDJ-SB: an untouched tempo
/// fader steps 48–64 of 16383 about once a second; a hand sends every 2–10 ms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gate {
    anchor: Option<f64>,
    last: Option<u64>,
}
impl Gate {
    pub fn pass(&mut self, x: f64, now_ms: u64, deadband: f64) -> bool {
        let moving = self
            .last
            .is_some_and(|t| now_ms.saturating_sub(t) <= MOTION_MS);
        self.last = Some(now_ms);
        let far = self.anchor.is_none_or(|a| (x - a).abs() >= deadband);
        if moving || far {
            self.anchor = Some(x);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoders_read_both_directions() {
        assert_eq!(Encoding::Offset64.delta(0x3f), -1);
        assert_eq!(Encoding::Offset64.delta(0x43), 3);
        assert_eq!(Encoding::TwosComplement.delta(0x01), 1);
        assert_eq!(Encoding::TwosComplement.delta(0x7e), -2);
        assert_eq!(Encoding::SignBit.delta(0x02), 2);
        assert_eq!(Encoding::SignBit.delta(0x42), -2);
    }

    #[test]
    fn the_detent_is_exactly_the_middle() {
        assert_eq!(normalize(64, 7), 0.5);
        assert_eq!(normalize(8192, 14), 0.5);
        assert_eq!(normalize(0, 7), 0.);
        assert_eq!(normalize(127, 7), 1.);
        assert_eq!(normalize(16383, 14), 1.);
        assert!(normalize(63, 7) < 0.5 && normalize(65, 7) > 0.5);
    }

    #[test]
    fn modes_read_as_saved() {
        let relative: Mode =
            serde_json::from_str(r#"{"type":"relative","encoding":"twosComplement"}"#).unwrap();
        assert_eq!(
            relative,
            Mode::Relative {
                encoding: Encoding::TwosComplement
            }
        );
        assert_eq!(
            serde_json::to_string(&Mode::Button).unwrap(),
            r#"{"type":"button"}"#
        );
    }

    #[test]
    fn the_gate_holds_lone_steps_and_follows_a_hand() {
        let mut gate = Gate::default();
        let step = 64. / 16383.;
        assert!(gate.pass(0.5, 0, 0.006), "the first value sets the anchor");
        // A lone step a second later is noise.
        assert!(!gate.pass(0.5 + step, 1000, 0.006));
        assert!(!gate.pass(0.5, 2000, 0.006));
        // Drift past the dead band still arrives, once.
        assert!(gate.pass(0.5 + 2. * step, 3000, 0.006));
        // A hand: every few milliseconds, every value.
        for i in 1..20u64 {
            assert!(gate.pass(0.5 + i as f64 * 0.0001, 5000 + i * 5, 0.006));
        }
    }
}
