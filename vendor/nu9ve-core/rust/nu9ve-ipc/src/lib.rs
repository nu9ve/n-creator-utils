//! Engine registry (nu9ve-registry-v1) and a local client for engines that serve
//! nu9ve-engine-v1 frames on a socket.
//!
//! An engine announces itself with [`Registration::register`]; a client lists the
//! folder with [`scan`] and talks to a live engine with [`request`].

mod client;
mod process;
mod registry;
mod time;

pub use client::{DEFAULT_TIMEOUT, MAX_FRAME_BYTES, call, request};
pub use process::{ProcessInfo, current_uid, process};
pub use registry::{
    CONTRACT, ENV_DIR, EngineRef, Entry, MAX_RECORD_BYTES, Record, Registration, State, Transport,
    default_dir, is_live, parse, read_entry, scan, valid_file_name, valid_profile_id,
};
