//! Every Win32 call and every `unsafe` block in LexiSwitch.
//!
//! Per [ADR-0001], `unsafe` is confined here so it can be audited as a set rather than
//! hunted for. The security argument this project rests on is one a reader can check, which
//! only holds if the surface to check is small and in one place.
//!
//! Keystrokes are observed through Raw Input (`RIDEV_INPUTSINK`), an API with no capability
//! to block or alter a keystroke. `SetWindowsHookEx` is not called anywhere in this crate,
//! and no abstraction is provided that would make adding it a small change ([ADR-0003]).
//!
//! [ADR-0001]: ../../docs/adr/0001-windows-first-with-os-free-core.md
//! [ADR-0003]: ../../docs/adr/0003-raw-input-over-low-level-hook.md

// Requires the `unsafe` keyword on operations inside an `unsafe fn` rather than letting the
// function signature blanket them. Without this, an unsafe operation can be added to an
// existing unsafe fn with nothing at the call site to notice.
#![deny(unsafe_op_in_unsafe_fn)]
// "Every unsafe block carries a comment stating the invariant it upholds" is a stated
// convention (openspec/config.yaml) and task 10.9 verifies it by hand. This lint makes it
// mechanical: clippy requires a `// SAFETY:` comment on every unsafe block. Surfaced by
// `cargo clippy`; plain `cargo build` ignores tool lints.
#![warn(clippy::undocumented_unsafe_blocks)]

mod error;
pub mod clipboard;
pub mod focus;
pub mod hotkey;
pub mod inject;
pub mod langswitch;
pub mod layouts;
pub mod pagelock;
pub mod rawinput;
pub mod selection;
pub mod tray;
pub mod window;

pub use error::WinError;
