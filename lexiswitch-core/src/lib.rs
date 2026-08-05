//! Layout conversion and everything that does not need an operating system.
//!
//! Per [ADR-0001], this crate holds the logic and the tests: layout tables, position-based
//! conversion, the bounded keystroke buffer, multi-tap cycling, config types, and exclusion
//! matching. It must compile and pass its tests on any host — no Win32, no GUI, no keyboard
//! in the loop. Tests read frozen layout fixtures from `tests/fixtures/` rather than probing
//! the running system, because probing is exactly what this crate is forbidden to do.
//!
//! [ADR-0001]: ../../docs/adr/0001-windows-first-with-os-free-core.md

// The boundary ADR-0001 draws, stated to the compiler rather than to the reader.
#![forbid(unsafe_code)]

pub mod buffer;
pub mod config;
pub mod convert;
pub mod cycle;
pub mod layout;
