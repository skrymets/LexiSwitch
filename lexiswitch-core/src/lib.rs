//! Layout conversion and everything that does not need an operating system.
//!
//! Per [ADR-0001], this crate holds the logic and the tests: layout tables, position-based
//! conversion, and config types. It must compile and pass its tests on any host — no Win32,
//! no GUI, no keyboard in the loop. Tests read frozen layout fixtures from `tests/fixtures/`
//! rather than probing the running system, because probing is exactly what this crate is
//! forbidden to do.
//!
//! The keystroke buffer and multi-tap cycling lived here until
//! [ADR-0011](../../docs/adr/0011-no-keyboard-observation.md) and
//! [ADR-0012](../../docs/adr/0012-single-chooser-transaction.md) retired the features they
//! served. Candidate generation will join this crate for the same reason the rest of it is
//! here: it is a pure function of the layout tables.
//!
//! [ADR-0001]: ../../docs/adr/0001-windows-first-with-os-free-core.md

// The boundary ADR-0001 draws, stated to the compiler rather than to the reader.
#![forbid(unsafe_code)]

pub mod candidates;
pub mod casing;
pub mod config;
pub mod convert;
pub mod layout;
pub mod preview;
pub mod state;
pub mod usage;
