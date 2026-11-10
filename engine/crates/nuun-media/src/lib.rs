//! nüün's media domain in Rust, ported method by method from `media/` (rust-engine.md §7).
//! Reads open the catalog read only. The only write so far is `register_volume`, which changes configuration
//! (the catalog), never media, and takes the same writer lock as Python.

pub mod catalog;
pub mod devices;
pub mod photo_meta;
pub mod rules;
pub mod scan;
pub mod snapshot;
pub mod volumes;
