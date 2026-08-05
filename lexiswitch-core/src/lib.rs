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

pub mod convert;
pub mod layout;

// Modules arrive with the tasks that implement them, test-first:
//   buffer.rs     task 3.2   KeyBuffer, the 64-entry ring
//   cycle.rs      task 3.10  candidate selection and multi-tap cycling
//   config.rs     task 4.2   config types, defaults, validation
//   exclusion.rs  task 4.5   process-name and window-class matching
