//! Opening the controller: a [`MidiSystem`] trait that the owner of a controller uses, so it can be tested
//! with a fake one, and the byte stream of a native port split into channel messages. The CoreMIDI adapter
//! is [`crate::coremidi`] (macOS). Everything arrives by events: a port that appears or goes away, and each
//! message, never by polling.

use std::io;
use std::sync::Arc;

/// What a [`MidiSystem`] tells its owner, from its own thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortEvent {
    /// A source with this name appeared (also once for each one present when the system opens).
    Connected { name: String },
    /// A source went away. If it was open, it is closed.
    Disconnected { name: String },
    /// A channel message from an open source. `host_ns` is the host clock in nanoseconds
    /// (`mach_absolute_time` on macOS, the clock of nu9ve-recording-v1 §5).
    Message {
        name: String,
        bytes: [u8; 3],
        host_ns: u64,
    },
}

/// Receives the events of a [`MidiSystem`]. Called from the system's threads: keep it short.
pub type Sink = Arc<dyn Fn(PortEvent) + Send + Sync>;

/// The native MIDI system, by port name: the source and, if there is one, the destination of the same
/// name (where the lights go).
pub trait MidiSystem: Send {
    /// Names of the sources present now.
    fn sources(&self) -> Vec<String>;
    /// Starts receiving from the source called `name` and sending to its destination.
    fn open(&mut self, name: &str) -> io::Result<()>;
    /// Stops receiving from `name`. Closing a port that is not open does nothing.
    fn close(&mut self, name: &str);
    /// Sends short messages and SysEx, in order, to the destination of an open port.
    fn send(&mut self, name: &str, messages: &[Vec<u8>]) -> io::Result<()>;
}

/// Splits the bytes of a native packet into 3-byte channel messages: running status is expanded, and
/// SysEx, real time, system common and the two-byte families (program change, channel pressure) are dropped,
/// as nu9ve-controller-map-v1 §4 says to ignore them. `status` carries running status between packets.
pub fn split_channel_messages(bytes: &[u8], status: &mut Option<u8>) -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    let mut data: Vec<u8> = Vec::with_capacity(2);
    let mut sysex = false;
    for &b in bytes {
        if b >= 0xf8 {
            // Real time may come anywhere, even inside another message.
            continue;
        }
        if b & 0x80 != 0 {
            data.clear();
            sysex = b == 0xf0;
            *status = (b < 0xf0).then_some(b);
            continue;
        }
        if sysex {
            continue;
        }
        let Some(s) = *status else { continue };
        data.push(b);
        let needed = match s & 0xf0 {
            0xc0 | 0xd0 => 1,
            _ => 2,
        };
        if data.len() == needed {
            if needed == 2 {
                out.push([s, data[0], data[1]]);
            }
            data.clear();
        }
    }
    out
}

/// A system with no ports, for owners that must run where there is no MIDI support.
#[derive(Debug, Default)]
pub struct NoMidi;

impl MidiSystem for NoMidi {
    fn sources(&self) -> Vec<String> {
        Vec::new()
    }
    fn open(&mut self, name: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no MIDI on this system: {name}"),
        ))
    }
    fn close(&mut self, _: &str) {}
    fn send(&mut self, name: &str, _: &[Vec<u8>]) -> io::Result<()> {
        self.open(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_status_sysex_and_real_time_split_cleanly() {
        let mut status = None;
        // Two notes in one packet, the second with running status, a clock byte in the middle.
        let out = split_channel_messages(&[0x90, 0x0b, 0x7f, 0xf8, 0x0c, 0x01], &mut status);
        assert_eq!(out, vec![[0x90, 0x0b, 0x7f], [0x90, 0x0c, 0x01]]);
        // Running status continues in the next packet.
        assert_eq!(
            split_channel_messages(&[0x0b, 0x00], &mut status),
            vec![[0x90, 0x0b, 0x00]]
        );
        // SysEx (the Launchpad's reply) and its data are dropped; channel pressure too.
        let out = split_channel_messages(
            &[0xf0, 0x00, 0x20, 0x29, 0xf7, 0xd0, 0x28, 0xb0, 0x5b, 0x7f],
            &mut status,
        );
        assert_eq!(out, vec![[0xb0, 0x5b, 0x7f]]);
        // Data with no status, after a reset, is dropped.
        let mut fresh = None;
        assert!(split_channel_messages(&[0x01, 0x02], &mut fresh).is_empty());
    }
}
