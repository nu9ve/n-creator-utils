//! From a gesture to a control. A gesture is everything a controller sent while one control was
//! moved by hand, ending when it goes quiet. The person never has to say what kind of control it
//! was: the messages do.

use crate::input::{Encoding, Mode, combine_cc14};
use crate::message::{Key, Kind, Message};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub at_ms: u64,
    pub message: Message,
}

/// What one gesture was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub key: Key,
    /// The CC carrying the low 7 bits of a 14-bit control (by convention MSB + 0x20).
    pub lsb: Option<u8>,
    pub mode: Mode,
    /// 7 or 14.
    pub bits: u8,
    /// Range seen during the gesture, at `bits` resolution.
    pub min: u16,
    pub max: u16,
    /// Messages from the control (MSBs only for a 14-bit one).
    pub count: usize,
    /// Other CCs that sent exactly the same values at the same time (a DDJ-SB FX knob sends three).
    pub mirrors: Vec<Key>,
}
impl Detected {
    /// One line for a terminal, in the terms of a MIDI manual.
    pub fn summary(&self) -> String {
        let what = match self.mode {
            Mode::Button => "button".to_string(),
            Mode::Absolute if self.bits == 14 => match self.lsb {
                Some(lsb) => format!("fader/knob 14-bit (LSB 0x{lsb:02X})"),
                None => "fader/knob 14-bit".into(),
            },
            Mode::Absolute | Mode::Auto => "fader/knob 7-bit".into(),
            Mode::Relative { encoding } => format!("relative encoder ({encoding:?})"),
        };
        let mut line = format!(
            "{} · {what} · {}..{} · {} msgs",
            self.key.label(),
            self.min,
            self.max,
            self.count
        );
        if !self.mirrors.is_empty() {
            let mirrors: Vec<String> = self.mirrors.iter().map(Key::label).collect();
            line.push_str(&format!(" · same as {}", mirrors.join(", ")));
        }
        line
    }
}

/// Which kind of control the person was asked for, when they were asked for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    Button,
    Continuous,
}

/// Reads one gesture. `noisy` holds controls that send by themselves (found with nothing
/// touched); they are only chosen when nothing else moved. `want` breaks ties when two
/// controls moved together, like touching a jog while turning it.
pub fn classify(events: &[Event], noisy: &BTreeSet<Key>, want: Option<Want>) -> Option<Detected> {
    let mut by_key: BTreeMap<Key, Vec<(u64, u16)>> = BTreeMap::new();
    for event in events {
        by_key
            .entry(event.message.key)
            .or_default()
            .push((event.at_ms, event.message.value));
    }
    // 14-bit pairs: CC n below 0x20 and CC n + 0x20 arriving right after it.
    let mut lsb_of: BTreeMap<Key, u8> = BTreeMap::new();
    let mut lsbs: BTreeSet<Key> = BTreeSet::new();
    for (key, msbs) in &by_key {
        if key.kind != Kind::Cc || key.number >= 0x20 {
            continue;
        }
        let low = Key {
            number: key.number + 0x20,
            ..*key
        };
        let Some(low_values) = by_key.get(&low) else {
            continue;
        };
        let paired = low_values
            .iter()
            .filter(|(t, _)| msbs.iter().any(|(m, _)| *t >= *m && t - m <= 2))
            .count();
        if paired > 0 && paired * 2 >= low_values.len() {
            lsb_of.insert(*key, low.number);
            lsbs.insert(low);
        }
    }
    let mut candidates: Vec<Detected> = by_key
        .iter()
        .filter(|(key, _)| !lsbs.contains(key))
        .map(|(key, values)| describe(*key, values, lsb_of.get(key).copied(), events))
        .collect();
    // Mirrors: CCs with exactly the same values as a lower-numbered one.
    let mut mirrored: BTreeSet<Key> = BTreeSet::new();
    for i in 0..candidates.len() {
        if mirrored.contains(&candidates[i].key) {
            continue;
        }
        let mine = &by_key[&candidates[i].key];
        let twins: Vec<Key> = candidates
            .iter()
            .skip(i + 1)
            .filter(|other| {
                other.key.kind == Kind::Cc
                    && candidates[i].key.kind == Kind::Cc
                    && other.key.channel == candidates[i].key.channel
                    && same_values(mine, &by_key[&other.key])
            })
            .map(|other| other.key)
            .collect();
        mirrored.extend(twins.iter().copied());
        candidates[i].mirrors = twins;
    }
    candidates.retain(|c| !mirrored.contains(&c.key) && !too_little(c));
    if candidates.iter().any(|c| !noisy.contains(&c.key)) {
        candidates.retain(|c| !noisy.contains(&c.key));
    }
    if let Some(want) = want {
        let fits = |c: &Detected| (c.mode == Mode::Button) == (want == Want::Button);
        if candidates.iter().any(fits) {
            candidates.retain(fits);
        }
    }
    // The control that sent most; on a tie, the one touched last (a modifier is held first).
    let start = |c: &Detected| by_key[&c.key][0].0;
    candidates
        .into_iter()
        .max_by(|a, b| a.count.cmp(&b.count).then(start(a).cmp(&start(b))))
}

/// Too little to tell what a control is, or to tell it from a worn fader drifting: one or two
/// messages that are not a button, or a few that barely move. On the DDJ-SB an untouched tempo
/// fader sent 6 messages over 1.5% of its travel while pads were being pressed.
fn too_little(d: &Detected) -> bool {
    if d.mode == Mode::Button {
        // Only a release: it was pressed before anyone was listening.
        return d.max == 0;
    }
    let full = if d.bits == 14 { 16383. } else { 127. };
    d.count < 3
        || (d.mode == Mode::Absolute && d.count < 10 && (d.max - d.min) as f64 / full < 0.02)
}

fn same_values(a: &[(u64, u16)], b: &[(u64, u16)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|((ta, va), (tb, vb))| va == vb && ta.abs_diff(*tb) <= 5)
}

fn describe(key: Key, values: &[(u64, u16)], lsb: Option<u8>, events: &[Event]) -> Detected {
    let raw: Vec<u16> = values.iter().map(|(_, v)| *v).collect();
    let (mode, bits, seen) = match (key.kind, lsb) {
        (Kind::Note, _) => (Mode::Button, 7, raw.clone()),
        (Kind::PitchBend, _) => (Mode::Absolute, 14, raw.clone()),
        (Kind::Cc, Some(lsb)) => (Mode::Absolute, 14, combined(key, lsb, events)),
        (Kind::Cc, None) => (cc_mode(&raw), 7, raw.clone()),
    };
    Detected {
        key,
        lsb,
        mode,
        bits,
        min: seen.iter().copied().min().unwrap_or(0),
        max: seen.iter().copied().max().unwrap_or(0),
        count: values.len(),
        mirrors: Vec::new(),
    }
}

/// 14-bit values in order: each LSB completes the MSB before it.
fn combined(key: Key, lsb: u8, events: &[Event]) -> Vec<u16> {
    let mut msb = None;
    let mut values = Vec::new();
    for event in events {
        let k = event.message.key;
        if k.kind != Kind::Cc || k.channel != key.channel {
            continue;
        }
        if k.number == key.number {
            msb = Some(event.message.value);
        } else if k.number == lsb
            && let Some(m) = msb
        {
            values.push(combine_cc14(m, event.message.value));
        }
    }
    values
}

fn cc_mode(values: &[u16]) -> Mode {
    let distinct: BTreeSet<u16> = values.iter().copied().collect();
    // A button over CC sends its two states, usually 127 and 0. Always a 0 on release: an encoder
    // turned down one step at a time also repeats 127, but never sends 0.
    if distinct.contains(&0)
        && (distinct.iter().all(|v| *v == 0 || *v == 127)
            || (distinct.len() <= 2 && values.len() <= 8))
    {
        return Mode::Button;
    }
    match relative(values) {
        Some(encoding) => Mode::Relative { encoding },
        None => Mode::Absolute,
    }
}

/// Absolute controls only send when their value changes, so they never repeat a value back to
/// back, and never jump between the two ends. Encoders do both.
fn relative(values: &[u16]) -> Option<Encoding> {
    if values.len() < 3 {
        return None;
    }
    let repeats =
        values.windows(2).filter(|w| w[0] == w[1]).count() as f64 / (values.len() - 1) as f64;
    let all = |f: &dyn Fn(u16) -> bool| values.iter().all(|v| f(*v));
    let any = |f: &dyn Fn(u16) -> bool| values.iter().any(|v| f(*v));
    let low = |v: u16| (1..=20).contains(&v);
    let high = |v: u16| (108..=127).contains(&v);
    if all(&|v| low(v) || high(v)) && ((any(&low) && any(&high)) || repeats >= 0.2) {
        return Some(Encoding::TwosComplement);
    }
    let around = |v: u16| (44..=84).contains(&v);
    if all(&|v| low(v) || (65..=84).contains(&v)) && any(&low) && any(&|v| v >= 65) {
        return Some(Encoding::SignBit);
    }
    let sides = any(&|v| v < 64) && any(&|v| v > 64);
    if all(&around) && (repeats >= 0.2 || (sides && repeats > 0.)) {
        return Some(Encoding::Offset64);
    }
    None
}

/// What the explorer knows about a control it has seen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Seen {
    pub key: Key,
    pub lsb: Option<u8>,
    /// From the last gesture that chose it; `None` while it has only been seen.
    pub mode: Option<Mode>,
    pub count: u64,
    pub min: u16,
    pub max: u16,
    pub last: u16,
    /// Sent while nothing was touched.
    pub noisy: bool,
}

/// Bounds for a live session: a controller has at most 16 × 128 × 2 + 16 keys.
const MAX_EVENTS: usize = 8192;
const MAX_SEEN: usize = 512;
/// The longest a held note keeps a gesture open with nothing else happening.
const HELD_MS: u64 = 5000;

/// Watches a controller live: finds noisy controls while nothing is touched, then turns each
/// gesture into a [`Detected`] once the controller goes quiet.
#[derive(Debug)]
pub struct Explorer {
    events: Vec<Event>,
    last_at: Option<u64>,
    noisy: BTreeSet<Key>,
    baseline_until: Option<u64>,
    seen: BTreeMap<Key, Seen>,
    lsbs: BTreeSet<Key>,
    /// Notes held now. Letting go of the last one ends a gesture of buttons at once.
    held: BTreeSet<Key>,
    closed: bool,
    /// Silence that ends a gesture.
    pub quiet_ms: u64,
}
impl Default for Explorer {
    fn default() -> Self {
        Self::new(400)
    }
}
impl Explorer {
    pub fn new(quiet_ms: u64) -> Self {
        Self {
            events: Vec::new(),
            last_at: None,
            noisy: BTreeSet::new(),
            baseline_until: None,
            seen: BTreeMap::new(),
            lsbs: BTreeSet::new(),
            held: BTreeSet::new(),
            closed: false,
            quiet_ms,
        }
    }
    /// Starts listening for controls that move by themselves. Nothing should be touched.
    pub fn start_baseline(&mut self, now_ms: u64, length_ms: u64) {
        self.noisy.clear();
        self.reset();
        self.baseline_until = Some(now_ms + length_ms);
        for seen in self.seen.values_mut() {
            seen.noisy = false;
        }
    }
    pub fn baseline_left_ms(&self, now_ms: u64) -> Option<u64> {
        self.baseline_until
            .filter(|until| *until > now_ms)
            .map(|until| until - now_ms)
    }
    pub fn feed(&mut self, at_ms: u64, message: Message) {
        let key = message.key;
        if self.seen.len() < MAX_SEEN || self.seen.contains_key(&key) {
            let seen = self.seen.entry(key).or_insert(Seen {
                key,
                lsb: None,
                mode: None,
                count: 0,
                min: message.value,
                max: message.value,
                last: message.value,
                noisy: false,
            });
            seen.count += 1;
            seen.min = seen.min.min(message.value);
            seen.max = seen.max.max(message.value);
            seen.last = message.value;
        }
        if self.baseline_left_ms(at_ms).is_some() {
            self.noisy.insert(key);
            if let Some(seen) = self.seen.get_mut(&key) {
                seen.noisy = true;
            }
            return;
        }
        let quiet = self.noisy.contains(&key);
        if quiet && self.last_at.is_none() {
            return;
        }
        if !quiet {
            self.last_at = Some(at_ms);
        }
        if self.events.len() < MAX_EVENTS {
            self.events.push(Event { at_ms, message });
        }
        if key.kind == Kind::Note {
            if message.pressed() {
                self.held.insert(key);
            } else {
                self.held.remove(&key);
                // Only buttons in this gesture, all let go: it is over.
                self.closed = self.held.is_empty()
                    && self.events.iter().all(|e| {
                        e.message.key.kind == Kind::Note || self.noisy.contains(&e.message.key)
                    });
            }
        } else if !quiet {
            self.closed = false;
        }
    }
    /// A finished gesture: buttons all let go, or the controller quiet for `quiet_ms`.
    pub fn poll(&mut self, now_ms: u64, want: Option<Want>) -> Option<Detected> {
        let last = self.last_at?;
        let quiet = now_ms.saturating_sub(last);
        // A held modifier waits for what it modifies, though not for a note that never ends.
        let wait = if self.held.is_empty() {
            self.quiet_ms
        } else {
            HELD_MS
        };
        if !self.closed && quiet < wait {
            return None;
        }
        let detected = classify(&self.events, &self.noisy, want);
        self.reset();
        if let Some(d) = &detected {
            if let Some(lsb) = d.lsb {
                self.lsbs.insert(Key {
                    number: lsb,
                    ..d.key
                });
            }
            if let Some(seen) = self.seen.get_mut(&d.key) {
                seen.mode = Some(d.mode);
                seen.lsb = d.lsb;
                if d.bits == 14 {
                    seen.min = d.min;
                    seen.max = d.max;
                }
            }
        }
        detected
    }
    /// Drops a gesture in progress, so the next one starts clean.
    pub fn reset(&mut self) {
        self.events.clear();
        self.last_at = None;
        self.held.clear();
        self.closed = false;
    }
    pub fn gesture_in_progress(&self) -> bool {
        self.last_at.is_some()
    }
    pub fn noisy(&self) -> &BTreeSet<Key> {
        &self.noisy
    }
    /// Controls seen so far, without the LSB half of 14-bit pairs.
    pub fn seen(&self) -> Vec<Seen> {
        self.seen
            .values()
            .filter(|s| !self.lsbs.contains(&s.key))
            .cloned()
            .collect()
    }
    pub fn clear(&mut self) {
        *self = Self::new(self.quiet_ms);
    }
}

/// Reads a capture as the probe writes it: milliseconds, a tab, then hex bytes.
pub fn parse_log(text: &str) -> Vec<Event> {
    text.lines()
        .filter_map(|line| {
            let (at, bytes) = line.split_once('\t')?;
            let bytes: Vec<u8> = bytes
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16))
                .collect::<Result<_, _>>()
                .ok()?;
            Some(Event {
                at_ms: at.trim().parse().ok()?,
                message: Message::parse(&bytes)?,
            })
        })
        .collect()
}

/// Runs a whole capture through an explorer, as if it were live: the first `baseline_ms` find
/// noisy controls, then every gesture is reported with the time it ended.
pub fn replay(events: &[Event], baseline_ms: u64, quiet_ms: u64) -> Vec<(u64, Detected)> {
    let mut explorer = Explorer::new(quiet_ms);
    // The probe logs elapsed time from connection, including any silence before the first event.
    explorer.start_baseline(0, baseline_ms);
    let mut found = Vec::new();
    for event in events {
        // Time passes between messages: a gap long enough closes the gesture before it.
        if let Some(d) = explorer.poll(event.at_ms, None) {
            found.push((event.at_ms, d));
        }
        explorer.feed(event.at_ms, event.message);
    }
    if let Some(d) = explorer.poll(u64::MAX, None) {
        found.push((events.last().map_or(0, |e| e.at_ms), d));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(lines: &[(u64, [u8; 3])]) -> Vec<Event> {
        lines
            .iter()
            .map(|(at, bytes)| Event {
                at_ms: *at,
                message: Message::parse(bytes).unwrap(),
            })
            .collect()
    }

    #[test]
    fn a_note_pressed_and_released_is_a_button() {
        let d = classify(
            &events(&[(0, [0x90, 0x0b, 0x7f]), (90, [0x90, 0x0b, 0x00])]),
            &BTreeSet::new(),
            None,
        )
        .unwrap();
        assert_eq!(d.mode, Mode::Button);
        assert_eq!(d.key.number, 0x0b);
    }

    #[test]
    fn a_modifier_held_first_loses_to_the_pad() {
        // SHIFT, then the pad, then both let go.
        let d = classify(
            &events(&[
                (0, [0x90, 0x3f, 0x7f]),
                (700, [0x97, 0x0a, 0x7f]),
                (850, [0x97, 0x0a, 0x00]),
                (900, [0x90, 0x3f, 0x00]),
            ]),
            &BTreeSet::new(),
            None,
        )
        .unwrap();
        assert_eq!((d.key.channel, d.key.number), (7, 0x0a));
    }

    #[test]
    fn pairs_make_fourteen_bits() {
        let d = classify(
            &events(&[
                (0, [0xb0, 0x13, 0x7f]),
                (0, [0xb0, 0x33, 0x40]),
                (3, [0xb0, 0x13, 0x7e]),
                (3, [0xb0, 0x33, 0x50]),
                (5, [0xb0, 0x13, 0x00]),
                (5, [0xb0, 0x33, 0x00]),
            ]),
            &BTreeSet::new(),
            None,
        )
        .unwrap();
        assert_eq!((d.key.number, d.lsb, d.bits), (0x13, Some(0x33), 14));
        assert_eq!((d.min, d.max), (0, 0x7f << 7 | 0x40));
    }

    #[test]
    fn encoders_are_told_from_knobs() {
        let jog: Vec<_> = (0..12).map(|i| (i * 5, [0xb0, 0x21, 0x3f])).collect();
        let d = classify(&events(&jog), &BTreeSet::new(), None).unwrap();
        assert_eq!(
            d.mode,
            Mode::Relative {
                encoding: Encoding::Offset64
            }
        );
        let browse = [0x7e, 0x7e, 0x7f, 0x01, 0x04, 0x03, 0x01];
        let browse: Vec<_> = browse
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u64 * 40, [0xb6, 0x40, *v]))
            .collect();
        let d = classify(&events(&browse), &BTreeSet::new(), None).unwrap();
        assert_eq!(
            d.mode,
            Mode::Relative {
                encoding: Encoding::TwosComplement
            }
        );
        let knob: Vec<_> = (60..70).map(|v| (v as u64 * 8, [0xb0, 0x05, v])).collect();
        let d = classify(&events(&knob), &BTreeSet::new(), None).unwrap();
        assert_eq!(d.mode, Mode::Absolute);
    }

    #[test]
    fn noisy_controls_and_mirrors_step_aside() {
        let mut lines = vec![(0, [0xb1, 0x00, 0x45]), (0, [0xb1, 0x20, 0x10])];
        for i in 0..4u8 {
            let at = 10 + i as u64 * 3;
            lines.push((at, [0xb5, 0x02, 0x70 - i]));
            lines.push((at, [0xb5, 0x22, 0x00]));
            lines.push((at + 1, [0xb5, 0x04, 0x70 - i]));
            lines.push((at + 1, [0xb5, 0x24, 0x00]));
        }
        for i in 0..6u8 {
            lines.push((30 + i as u64, [0xb1, 0x00, 0x45 + i % 2]));
        }
        let noisy = BTreeSet::from([Key {
            kind: Kind::Cc,
            channel: 1,
            number: 0,
        }]);
        let d = classify(&events(&lines), &noisy, None).unwrap();
        assert_eq!((d.key.channel, d.key.number, d.lsb), (5, 0x02, Some(0x22)));
        assert_eq!(d.mirrors.len(), 1);
        assert_eq!(d.mirrors[0].number, 0x04);
    }

    #[test]
    fn asking_for_a_button_finds_the_touch_under_the_turn() {
        let mut lines = vec![(0, [0x90, 0x35, 0x7f])];
        lines.extend((1..20).map(|i| (i * 5, [0xb0, 0x23, 0x3e])));
        lines.push((120, [0x90, 0x35, 0x00]));
        let all = events(&lines);
        let none = BTreeSet::new();
        assert_eq!(classify(&all, &none, None).unwrap().key.number, 0x23);
        let touch = classify(&all, &none, Some(Want::Button)).unwrap();
        assert_eq!((touch.key.number, touch.mode), (0x35, Mode::Button));
    }

    #[test]
    fn the_explorer_learns_noise_then_closes_gestures() {
        let mut explorer = Explorer::new(400);
        explorer.start_baseline(0, 3000);
        let noise = Message::parse(&[0xb1, 0x00, 0x45]).unwrap();
        explorer.feed(1000, noise);
        assert!(explorer.noisy().contains(&noise.key));
        // Noise alone after the baseline starts nothing.
        explorer.feed(4000, noise);
        assert!(!explorer.gesture_in_progress());
        explorer.feed(5000, Message::parse(&[0x90, 0x0b, 0x7f]).unwrap());
        assert_eq!(
            explorer.poll(5600, None),
            None,
            "a held button waits for what it modifies"
        );
        explorer.feed(5700, Message::parse(&[0x90, 0x0b, 0x00]).unwrap());
        // Let go: no need to wait for silence.
        let d = explorer.poll(5700, None).unwrap();
        assert_eq!(d.key.number, 0x0b);
        // A turn waits for silence.
        for i in 0..5 {
            explorer.feed(6000 + i * 5, Message::parse(&[0xb0, 0x21, 0x3f]).unwrap());
        }
        assert_eq!(explorer.poll(6200, None), None, "not quiet long enough");
        assert_eq!(explorer.poll(6500, None).unwrap().key.number, 0x21);
        // A release with no press seen is nothing.
        explorer.feed(7000, Message::parse(&[0x91, 0x17, 0x00]).unwrap());
        assert_eq!(explorer.poll(7000, None), None);
        assert!(
            explorer
                .seen()
                .iter()
                .any(|s| s.noisy && s.key == noise.key)
        );
    }
}
