//! Compiles the icons under `media/` into the executable.
//!
//! The first of them is the application icon — what Explorer, the taskbar, Alt-Tab and the
//! window menu read. Nothing at runtime can supply that one, because the shell asks the *file*
//! for an icon before the process exists. Group id 1 is not arbitrary: the shell shows the
//! numerically lowest `RT_GROUP_ICON`, so the icon meant to represent the program has to be the
//! first one. The other two are tray states, reachable only from inside the process.
//!
//! `lexiswitch-win` loads the same ids (see `icon.rs`), and [`ICONS`] is the contract between
//! them: same order, same numbering, one list each.
//!
//! **It writes the `.res` itself rather than calling a resource compiler.** `rc.exe` ships with
//! the Windows SDK and `llvm-rc` with some LLVM installations; depending on either would make a
//! build that otherwise needs only cargo fail on a machine that has neither, and pulling in a
//! crate to paper over that trades a hard dependency for a tree of soft ones. The RES format is
//! a header and a length — the whole writer is `entry` below. The icon's own bytes are copied
//! through untouched, PNG-compressed 256px frame included, which is what `RT_ICON` holds
//! anyway.

use std::env;
use std::fs;
use std::path::PathBuf;

/// The icons to embed, in group-id order: the first gets group 1, the second group 2, and so
/// on. `lexiswitch-win/src/icon.rs` numbers its `Kind` the same way, and the application icon
/// has to stay first because the shell reads the lowest group id and nothing else.
const ICONS: [&str; 3] = [
    "../media/lexiswitch_main.ico",
    "../media/lexiswitch_error.ico",
    "../media/lexiswitch_disabled.ico",
];

/// `RT_ICON` — one per frame inside each `.ico`.
const RT_ICON: u16 = 3;

/// `RT_GROUP_ICON` — the directory that names them, and the thing `LoadIcon` actually asks for.
const RT_GROUP_ICON: u16 = 14;

fn main() {
    for icon in ICONS {
        println!("cargo:rerun-if-changed={icon}");
    }
    println!("cargo:rerun-if-changed=build.rs");

    // Windows only, and only under the MSVC linker: `link.exe` takes a `.res` on its command
    // line directly. Anywhere else there is nothing to embed into, so the build stays quiet
    // rather than failing over an icon.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let mut res = Vec::new();
    // The RES format opens with an empty entry whose type and name are both ordinal zero. It
    // carries nothing; its presence is how the linker recognises the file.
    entry(&mut res, 0, 0, &[], 0, 0);

    // Frame ids run on across the whole file rather than restarting per icon: they share one id
    // space under `RT_ICON`, so two groups both numbering their frames from 1 would each be
    // pointing at the other's images.
    let mut next_frame = 1u16;
    for (i, path) in ICONS.iter().enumerate() {
        let ico = fs::read(path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
        next_frame = add_icon(&mut res, path, &ico, i as u16 + 1, next_frame);
    }

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("lexiswitch.res");
    fs::write(&out, res).unwrap_or_else(|e| panic!("writing {}: {e}", out.display()));
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// Append one `.ico` as `RT_ICON` per frame plus the `RT_GROUP_ICON` naming them.
///
/// Returns the next free frame id.
fn add_icon(res: &mut Vec<u8>, path: &str, ico: &[u8], group_id: u16, first_frame: u16) -> u16 {
    assert!(ico.len() >= 6, "{path} is too short to be an icon");
    assert_eq!(read_u16(ico, 0), 0, "{path}: reserved field is not zero");
    assert_eq!(read_u16(ico, 2), 1, "{path}: not an icon (type is not 1)");
    let count = read_u16(ico, 4) as usize;
    assert!(count > 0, "{path} holds no images");

    // The directory the group resource is built from. The `.ico` on disk points at each frame
    // by file offset; the resource points at it by id, and everything else is copied across.
    let mut group = Vec::with_capacity(6 + count * 14);
    group.extend_from_slice(&0u16.to_le_bytes()); // reserved
    group.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    group.extend_from_slice(&(count as u16).to_le_bytes());

    for i in 0..count {
        let e = 6 + i * 16;
        assert!(ico.len() >= e + 16, "{path}: directory entry {i} is truncated");
        let bytes = read_u32(ico, e + 8) as usize;
        let offset = read_u32(ico, e + 12) as usize;
        let end = offset
            .checked_add(bytes)
            .filter(|end| *end <= ico.len())
            .unwrap_or_else(|| panic!("{path}: image {i} runs past the end of the file"));

        let id = first_frame + i as u16;
        entry(res, RT_ICON, id, &ico[offset..end], MOVEABLE | DISCARDABLE, LANG);

        group.extend_from_slice(&ico[e..e + 8]); // width, height, colours, reserved, planes, bpp
        group.extend_from_slice(&(bytes as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes()); // 14 bytes, where the file has 16
    }

    entry(
        res,
        RT_GROUP_ICON,
        group_id,
        &group,
        MOVEABLE | PURE | DISCARDABLE,
        LANG,
    );
    first_frame + count as u16
}

const MOVEABLE: u16 = 0x0010;
const PURE: u16 = 0x0020;
const DISCARDABLE: u16 = 0x1000;

/// US English. Resources are language-tagged whether or not anything varies by language, and
/// an icon that claims a language nobody has is one the loader can decline to find.
const LANG: u16 = 0x0409;

/// One resource, header then data, each padded out to a four-byte boundary.
///
/// The header is exactly 32 bytes because both the type and the name are ordinals, written as
/// the marker `0xFFFF` followed by the number. A named resource would make the header longer
/// and nothing here needs one.
fn entry(res: &mut Vec<u8>, type_id: u16, name_id: u16, data: &[u8], memory: u16, lang: u16) {
    res.extend_from_slice(&(data.len() as u32).to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    res.extend_from_slice(&0xFFFFu16.to_le_bytes());
    res.extend_from_slice(&type_id.to_le_bytes());
    res.extend_from_slice(&0xFFFFu16.to_le_bytes());
    res.extend_from_slice(&name_id.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes()); // data version
    res.extend_from_slice(&memory.to_le_bytes());
    res.extend_from_slice(&lang.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes()); // version
    res.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    res.extend_from_slice(data);
    while !res.len().is_multiple_of(4) {
        res.push(0);
    }
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
