use nu9ve_midi::profile::{NOVATION_LAUNCHPAD_PRO_V2, PIONEER_DDJ_SB_V1, Profile};
use nu9ve_midi::{Encoding, Key, Kind, Mode};

#[test]
fn the_ddj_sb_profile_keeps_the_address_inventory_and_evidence() {
    let profile = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    assert_eq!(profile.id, "pioneer-ddj-sb");
    assert_eq!(profile.controls.len(), 125);
    assert_eq!(profile.controls.iter().filter(|c| c.verified).count(), 89);
    assert!(profile.matches("PIONEER DDJ-SB MIDI"));
    assert!(profile.matches("pioneer ddj-sb"));
    assert!(!profile.matches("Some Keyboard"));

    for (id, number) in [
        ("browse-press", 0x41),
        ("center-unidentified-note-65", 0x65),
    ] {
        let control = profile.control(id).unwrap();
        assert_eq!(
            control.key(),
            Key {
                kind: Kind::Note,
                channel: 6,
                number
            }
        );
        assert!(control.verified);
        assert_eq!(control.identified, id == "browse-press");
    }
    assert_eq!(
        profile.control("browse-turn").unwrap().mode,
        Mode::Relative {
            encoding: Encoding::TwosComplement
        }
    );
    for deck in ["deck-1", "deck-2"] {
        let tempo = profile.control(&format!("{deck}-tempo")).unwrap();
        assert_eq!(
            (tempo.number, tempo.lsb, tempo.deadband),
            (0x00, Some(0x20), Some(0.008))
        );
    }
    for unit in ["fx1", "fx2"] {
        for suffix in ["copy", "second-copy"] {
            let id = format!("{unit}-level-{suffix}");
            let primary = format!("{unit}-level");
            assert_eq!(
                profile.control(&id).unwrap().mirror_of.as_deref(),
                Some(primary.as_str())
            );
        }
    }
}

#[test]
fn rejects_duplicate_ids_and_overlapping_lsb_addresses() {
    let mut profile = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    profile.controls[1].id = profile.controls[0].id.clone();
    assert!(
        profile
            .validate()
            .unwrap_err()
            .contains("duplicate control ID")
    );

    let mut profile = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    let mut also_uses_lsb = profile.control("deck-1-tempo").unwrap().clone();
    also_uses_lsb.id = "duplicate-tempo-lsb".into();
    also_uses_lsb.number = also_uses_lsb.lsb.take().unwrap();
    profile.controls.push(also_uses_lsb);
    assert!(
        profile
            .validate()
            .unwrap_err()
            .contains("duplicate primary address")
    );
}

#[test]
fn validates_schema_and_accepts_decimal_or_hex_midi_numbers() {
    // Version 2 adds lights; a version 1 profile must not bring them.
    let text = PIONEER_DDJ_SB_V1.replace("\"schemaVersion\": 1", "\"schemaVersion\": 3");
    assert!(
        Profile::from_json(&text)
            .unwrap_err()
            .contains("unsupported MIDI profile version")
    );
    let text = PIONEER_DDJ_SB_V1.replace("\"0x33\"", "51");
    assert_eq!(
        Profile::from_json(&text)
            .unwrap()
            .control("deck-1-channel-fader")
            .unwrap()
            .lsb,
        Some(0x33)
    );
    let text = PIONEER_DDJ_SB_V1.replace("\"0x33\"", "\"0x80\"");
    assert!(Profile::from_json(&text).is_err());
    let lit = PIONEER_DDJ_SB_V1.replacen(
        "\"controls\":",
        "\"feedback\": {\"kind\": \"onOff\", \"allControls\": true}, \"controls\":",
        1,
    );
    assert!(
        Profile::from_json(&lit)
            .unwrap_err()
            .contains("feedback needs profile version 2")
    );
    let v2 = lit.replace("\"schemaVersion\": 1", "\"schemaVersion\": 2");
    let profile = Profile::from_json(&v2).unwrap();
    let mirror = profile.control("fx1-level-copy").unwrap();
    assert!(!profile.is_lit(mirror), "a mirror has no light of its own");
    assert_eq!(profile.primary(mirror).id, "fx1-level");
}

#[test]
fn the_launchpad_profile_has_the_programmer_grid_and_lights() {
    let pad = Profile::from_json(NOVATION_LAUNCHPAD_PRO_V2).unwrap();
    assert_eq!((pad.schema_version, pad.controls.len()), (2, 96));
    assert_eq!(pad.layout, Some([10, 10]));
    assert!(pad.matches("Launchpad Pro Standalone Port"));
    assert!(!pad.matches("Launchpad Pro Live Port"));
    let key = |kind, number| Key {
        kind,
        channel: 0,
        number,
    };
    assert_eq!(
        pad.control_by_key(key(Kind::Note, 11)).unwrap().id,
        "pad-8-1"
    );
    assert_eq!(
        pad.control_by_key(key(Kind::Note, 88)).unwrap().id,
        "pad-1-8"
    );
    assert_eq!(pad.control_by_key(key(Kind::Cc, 91)).unwrap().id, "top-1");
    assert_eq!(pad.control("top-1").unwrap().grid, Some([1, 0]));
    assert_eq!(
        pad.page_control.as_deref(),
        Some("top-5"),
        "Session changes page"
    );
    let wrong = NOVATION_LAUNCHPAD_PRO_V2
        .replace("\"pageControl\": \"top-5\"", "\"pageControl\": \"top-9\"");
    assert!(
        Profile::from_json(&wrong)
            .unwrap_err()
            .contains("pageControl")
    );
    assert!(pad.controls.iter().all(|c| pad.is_lit(c)));
    let ddj = Profile::from_json(PIONEER_DDJ_SB_V1).unwrap();
    assert!(
        !ddj.controls.iter().any(|c| ddj.is_lit(c)),
        "v1 has no output"
    );
}
