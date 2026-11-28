//! MIDI channel messages and physical controller input and output, independent of the native port backend.
//!
//! Feed one channel message at a time to [`Message::parse`]. A live learner can feed
//! timestamped messages to [`Explorer`]; a capture can use [`parse_log`] and [`replay`].
//! [`Profile`] describes hardware addresses and encodings, and from profile v2 its lights
//! ([`Feedback`]): [`LightBoard`] sends only what changed. [`Reader`] turns live messages into
//! presses, values and steps. Applications own binding policy and the actions triggered by the
//! decoded values.

#[cfg(target_os = "macos")]
pub mod coremidi;
pub mod detect;
pub mod feedback;
pub mod input;
pub mod map;
pub mod message;
pub mod port;
pub mod profile;
pub mod reader;

pub use detect::{Detected, Event, Explorer, Seen, Want, classify, parse_log, replay};
pub use feedback::{
    ActionState, Color, Feedback, FeedbackKind, Light, LightBoard, Motion, hex_bytes,
};
pub use input::{Encoding, Gate, Mode, combine_cc14, normalize};
pub use map::{
    Address, DeviceMap, MapFile, Mapping, ModeMappings, Modes, Zone, ZoneWhen, bundled_profiles,
};
pub use message::{Key, Kind, Message};
pub use port::{MidiSystem, NoMidi, PortEvent, Sink, split_channel_messages};
pub use profile::{Control, Profile};
pub use reader::{Gesture, Reader, Reading, default_deadband};
