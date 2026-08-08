//! The application's icons, out of the executable's own resources.
//!
//! `lexiswitch/build.rs` compiles the three files under `media/` in as group icons 1, 2 and 3;
//! this reads them back. [`Kind`] is numbered to match that order and the two lists have to
//! stay in step. The fallback below is what happens when there is no such resource — the
//! `examples/` probes are separate binaries with no build script, and they still have to draw
//! something.
//!
//! **`LR_SHARED` is deliberately not used.** It caches by resource name and ignores the size
//! asked for, so the first load would decide the size of every later one — the tray would get
//! whatever the window class asked for first, or the reverse. Instead each kind and size is
//! loaded once into [`CACHE`] and kept for the life of the process. Nothing here is ever
//! destroyed and nothing here leaks: there are six handles at most, each made at most once, and
//! all of them are still in use when the process ends.

use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON,
    SM_CYICON, SM_CYSMICON,
};

/// Which icon. The discriminants are the resource group ids, shared with `lexiswitch/build.rs`.
///
/// [`Kind::Main`] is 1 because the shell shows the numerically lowest group icon as the
/// executable's own; the other two are only ever asked for from inside the process.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Running and able to convert. Also the executable's icon, everywhere the shell shows one.
    Main = 1,
    /// A condition the user has to act on — no shortcut, a layout gone, a failed reload.
    Error = 2,
    /// Switched off deliberately. Not a fault, and drawn as its own thing rather than as the
    /// main icon dimmed, because dimming is what a disabled *control* looks like and this is a
    /// state the user chose.
    Disabled = 3,
}

/// Which of the two sizes Windows defines is wanted.
///
/// Not a pixel count. The shell picks the frame it wants out of the group, and the sizes it
/// asks for follow the display's scaling — asking for 16 by name is how a 16-pixel icon ends
/// up stretched across 24 pixels on a 150% display.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    /// `SM_CXSMICON` — the tray, the title bar, the window menu.
    Small = 0,
    /// `SM_CXICON` — Alt-Tab, the large-icon views, and the balloon.
    Large = 1,
}

/// One slot per kind and size, filled on first use.
static CACHE: [OnceLock<Option<usize>>; 6] = [const { OnceLock::new() }; 6];

/// `kind` at `size`, or `None` when this binary carries no icon resources.
pub fn app_icon(kind: Kind, size: Size) -> Option<HICON> {
    let slot = &CACHE[(kind as usize - 1) * 2 + size as usize];
    let handle = *slot.get_or_init(|| load(kind, size));
    handle.map(|h| HICON(h as *mut std::ffi::c_void))
}

fn load(kind: Kind, size: Size) -> Option<usize> {
    let (cx, cy) = match size {
        Size::Small => (SM_CXSMICON, SM_CYSMICON),
        Size::Large => (SM_CXICON, SM_CYICON),
    };
    // SAFETY: `None` asks for this process's own executable, which always has a module handle,
    // and reading system metrics has no preconditions at all.
    let (module, cx, cy) = unsafe {
        (
            GetModuleHandleW(None).ok()?,
            GetSystemMetrics(cx),
            GetSystemMetrics(cy),
        )
    };
    // SAFETY: the name is an integer resource id rather than a pointer to a string, which is
    // what `PCWSTR` holding a value below 0x10000 means to this API — the same MAKEINTRESOURCE
    // convention the C headers use. A missing resource is reported as an error, not undefined
    // behaviour, and is handled as the ordinary case it is for the `examples/` binaries.
    let handle = unsafe {
        LoadImageW(
            Some(module.into()),
            PCWSTR(kind as usize as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR,
        )
    }
    .ok()?;
    Some(handle.0 as usize)
}
