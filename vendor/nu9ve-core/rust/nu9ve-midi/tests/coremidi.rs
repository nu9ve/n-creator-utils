//! CoreMIDI end to end with a virtual controller: it appears and goes away by notifications (nothing
//! polls), its messages arrive with running status expanded, and the lights reach its destination.
#![cfg(target_os = "macos")]

use nu9ve_midi::coremidi::{CoreMidi, VirtualPort, now_ns};
use nu9ve_midi::{MidiSystem, PortEvent};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

fn next(rx: &mpsc::Receiver<PortEvent>, want: impl Fn(&PortEvent) -> bool) -> PortEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(event) if want(&event) => return event,
            Ok(_) => continue,
            Err(_) => panic!("no such event within 5 s"),
        }
    }
}

#[test]
fn a_virtual_controller_comes_plays_lights_and_goes() {
    let (tx, rx) = mpsc::channel();
    let sink = Arc::new(move |e: PortEvent| {
        let _ = tx.send(e);
    });
    let mut midi = CoreMidi::new("nu9ve-midi tests", sink).expect("CoreMIDI client");
    let name = format!("nu9ve-midi test {}", std::process::id());
    let mut pad = VirtualPort::new(&name).expect("virtual controller");

    let came = |e: &PortEvent| matches!(e, PortEvent::Connected { name: n } if *n == name);
    next(&rx, came);
    assert!(midi.sources().contains(&name));

    midi.open(&name).unwrap();
    let before = now_ns();
    // Two notes in one packet, the second with running status; a clock byte is dropped.
    pad.play(&[0x90, 0x0b, 0x7f, 0xf8, 0x0c, 0x01]);
    let message = |e: &PortEvent| matches!(e, PortEvent::Message { .. });
    let PortEvent::Message {
        name: from,
        bytes,
        host_ns,
    } = next(&rx, message)
    else {
        unreachable!()
    };
    assert_eq!((from.as_str(), bytes), (name.as_str(), [0x90, 0x0b, 0x7f]));
    assert!(host_ns >= before && host_ns <= now_ns(), "host clock in ns");
    let PortEvent::Message { bytes, .. } = next(&rx, message) else {
        unreachable!()
    };
    assert_eq!(bytes, [0x90, 0x0c, 0x01]);

    // Lights go to the destination of the same name; a SysEx stays whole.
    let lights = vec![
        vec![0x90, 0x51, 0x15],
        vec![0xf0, 0x00, 0x20, 0x29, 0x02, 0x10, 0x28, 0x51, 0x15, 0xf7],
    ];
    midi.send(&name, &lights).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while pad.received().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(pad.received(), lights);

    pad.unplug();
    let gone = |e: &PortEvent| matches!(e, PortEvent::Disconnected { name: n } if *n == name);
    next(&rx, gone);
    assert!(
        midi.send(&name, &lights).is_err(),
        "a port that went away is closed"
    );
    assert!(midi.open(&name).is_err());
    drop(midi);
}
