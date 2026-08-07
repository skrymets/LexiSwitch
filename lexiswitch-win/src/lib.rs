//! Every Win32 call and every `unsafe` block in LexiSwitch.
//!
//! Per [ADR-0001], `unsafe` is confined here so it can be audited as a set rather than
//! hunted for. The security argument this project rests on is one a reader can check, which
//! only holds if the surface to check is small and in one place.
//!
//! **Nothing here observes the keyboard.** There is no Raw Input registration and no
//! `SetWindowsHookEx` call, and no abstraction is provided that would make adding either a
//! small change ([ADR-0011]). Keys reach this program only as an ordinary focused window
//! receives them, and only while a window it owns has focus.
//!
//! That prohibition is not only a privacy property. A chooser that avoided taking focus would
//! need exactly the hook forbidden here, which is why the interface has the shape it does.
//!
//! [ADR-0001]: ../../docs/adr/0001-windows-first-with-os-free-core.md
//! [ADR-0011]: ../../docs/adr/0011-no-keyboard-observation.md

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
pub mod chooser;
pub mod clipboard;
pub mod focus;
pub mod hotkey;
pub mod inject;
pub mod langswitch;
pub mod layouts;
pub mod selection;
pub mod transaction;
pub mod tray;
pub mod window;

pub use error::WinError;
