//! Platform-independent data types for the event-timers addon.
//!
//! This crate has no nexus/windows dependencies so its tests can run
//! natively on any host. The addon crate re-exports these types; file I/O
//! and runtime state stay on the addon side.

pub mod config;
pub mod tracks;
