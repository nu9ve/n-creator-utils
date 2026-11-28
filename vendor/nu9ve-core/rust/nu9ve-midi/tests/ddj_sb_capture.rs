//! Selected CoreMIDI messages from the DDJ-SB on 2026-11-22. The fixture retains the
//! original timestamps and values for fader, jog, pad, FX and browse checks.

use nu9ve_midi::detect::{classify, parse_log, replay};
use nu9ve_midi::input::{Encoding, Mode};
use nu9ve_midi::message::{Key, Kind};
use nu9ve_midi::profile::{PIONEER_DDJ_SB_V1, Profile};
use std::collections::BTreeSet;

const CAPTURE: &str = include_str!("fixtures/ddj-sb-2026-11-22.txt");

fn key(kind: Kind, channel: u8, number: u8) -> Key {
    Key {
        kind,
        channel,
        number,
    }
}

#[test]
fn coremidi_capture_covers_fader_jogs_and_buttons() {
    let events = parse_log(CAPTURE);
    let find = |kind, channel, number, lsb: Option<u8>| {
        let control = key(kind, channel, number);
        let selected: Vec<_> = events
            .iter()
            .filter(|e| {
                e.message.key == control
                    || lsb.is_some_and(|lsb| e.message.key == key(Kind::Cc, channel, lsb))
            })
            .copied()
            .collect();
        classify(&selected, &BTreeSet::new(), None)
            .unwrap_or_else(|| panic!("{} not found", control.label()))
    };

    let fader = find(Kind::Cc, 0, 0x13, Some(0x33));
    assert_eq!(
        (fader.mode, fader.lsb, fader.bits),
        (Mode::Absolute, Some(0x33), 14)
    );
    for deck in 0..2 {
        for encoder in [0x21, 0x23] {
            assert_eq!(
                find(Kind::Cc, deck, encoder, None).mode,
                Mode::Relative {
                    encoding: Encoding::Offset64
                }
            );
        }
        assert_eq!(find(Kind::Note, deck, 0x35, None).mode, Mode::Button);
    }
    assert_eq!(find(Kind::Note, 1, 0x0b, None).mode, Mode::Button);
    assert_eq!(find(Kind::Note, 7, 0x02, None).mode, Mode::Button);
}

#[test]
fn profile_agrees_with_coremidi_gestures_and_browse_press() {
    let profile = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    let events = parse_log(CAPTURE);
    let found = replay(&events, 10_000, 400);
    for (_, d) in &found {
        let control = profile
            .control_by_key(d.key)
            .unwrap_or_else(|| panic!("{} is missing from the profile", d.summary()));
        assert_eq!(control.mode, d.mode, "{}", control.label);
        assert_eq!(control.lsb, d.lsb, "{}", control.label);
        assert!(control.verified, "{} was captured", control.label);
    }

    let find = |kind, channel, number| {
        let control = key(kind, channel, number);
        found
            .iter()
            .map(|(_, d)| d)
            .find(|d| d.key == control)
            .unwrap_or_else(|| panic!("{} not found", control.label()))
    };
    assert_eq!(find(Kind::Note, 1, 0x0b).mode, Mode::Button);
    assert_eq!(find(Kind::Note, 6, 0x41).mode, Mode::Button);
    assert_eq!(find(Kind::Note, 6, 0x46).mode, Mode::Button);
    assert_eq!(
        find(Kind::Cc, 6, 0x40).mode,
        Mode::Relative {
            encoding: Encoding::TwosComplement
        }
    );
    let fx = find(Kind::Cc, 5, 0x02);
    assert_eq!(fx.lsb, Some(0x22));
    assert_eq!(
        fx.mirrors,
        vec![key(Kind::Cc, 5, 0x04), key(Kind::Cc, 5, 0x06)]
    );
}

#[test]
fn replay_matches_the_original_reader_including_initial_silence() {
    let events = parse_log(CAPTURE);
    assert_eq!(events.len(), 1165);
    assert_eq!(events[0].at_ms, 21_356);
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ddj-sb-2026-11-22-replay.json")).unwrap();
    let actual = serde_json::to_value(replay(&events, 10_000, 400)).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn captured_tempo_fader_keeps_the_normalization_and_noise_gate_results() {
    use nu9ve_midi::{Gate, combine_cc14, normalize};

    let profile = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    let tempo = profile.control("deck-2-tempo").unwrap();
    let key = tempo.key();
    let lsb = Key {
        number: tempo.lsb.unwrap(),
        ..key
    };
    let deadband = tempo.deadband.unwrap();
    let mut gate = Gate::default();
    let mut msb = 0;
    let mut count = 0;
    let mut accepted = 0;
    let mut accepted_raw_sum = 0;
    for event in parse_log(CAPTURE) {
        if event.message.key == key {
            msb = event.message.value;
        } else if event.message.key == lsb {
            let raw = combine_cc14(msb, event.message.value);
            count += 1;
            if gate.pass(normalize(raw, 14), event.at_ms, deadband) {
                accepted += 1;
                accepted_raw_sum += u64::from(raw);
            }
        }
    }
    assert_eq!((count, accepted, accepted_raw_sum), (208, 206, 2_362_047));
}
