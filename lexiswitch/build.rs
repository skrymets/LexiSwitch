//! Compiles the icons under `media/` and the version resource into the executable.
//!
//! The version resource is what Explorer's Properties → Details and Task Manager read. It is
//! written from the workspace manifest's version (ADR-0016) — the one place a version is
//! declared — so it cannot disagree with the build it describes.
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

/// `RT_VERSION` — the version resource. There is one, and by convention it is id 1.
const RT_VERSION: u16 = 16;

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

    entry(&mut res, RT_VERSION, 1, &version_info(), MOVEABLE | PURE, LANG);

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

/// The `VS_VERSIONINFO` block, built from the version cargo hands every build script.
///
/// The numeric version is four 16-bit fields and has no room for a pre-release identifier, so
/// `0.2.0-rc.1` is numerically `0.2.0.0`, flagged as a pre-release, and spelled out in full in
/// the strings — which is what Explorer shows. Folding the identifier into the fourth field
/// would invent an ordering SemVer does not define.
fn version_info() -> Vec<u8> {
    let full = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let part = |name: &str| -> u32 {
        let var = format!("CARGO_PKG_VERSION_{name}");
        let text = env::var(&var).unwrap_or_else(|_| panic!("{var} is not set"));
        // Refused rather than truncated: a field that wrapped would publish a wrong version.
        let n: u16 = text
            .parse()
            .unwrap_or_else(|_| panic!("{var} = {text} does not fit a version resource field"));
        u32::from(n)
    };
    let (major, minor, patch) = (part("MAJOR"), part("MINOR"), part("PATCH"));
    let prerelease = !env::var("CARGO_PKG_VERSION_PRE").unwrap_or_default().is_empty();

    let ms = (major << 16) | minor;
    let ls = patch << 16;
    let fixed: Vec<u8> = [
        0xFEEF_04BD,                           // signature
        0x0001_0000,                           // structure version 1.0
        ms,                                    // file version, high
        ls,                                    // file version, low
        ms,                                    // product version, high
        ls,                                    // product version, low
        0x3F,                                  // which flag bits are meaningful
        if prerelease { 0x2 } else { 0 },      // VS_FF_PRERELEASE
        0x0004_0004,                           // VOS_NT_WINDOWS32
        0x1,                                   // VFT_APP
        0,                                     // no subtype
        0,                                     // no date, high
        0,                                     // no date, low
    ]
    .iter()
    .flat_map(|v: &u32| v.to_le_bytes())
    .collect();

    let description = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();
    // `FileDescription` is the name Task Manager gives the process, so it is the product's name
    // rather than the crate's one-line description, which goes in `Comments` instead.
    let strings = [
        ("ProductName", "LexiSwitch"),
        ("FileDescription", "LexiSwitch"),
        ("Comments", description.as_str()),
        ("ProductVersion", full.as_str()),
        ("FileVersion", full.as_str()),
        ("InternalName", "lexiswitch"),
        ("OriginalFilename", "lexiswitch.exe"),
    ]
    .map(|(key, value)| node(key, Value::Text(value), &[]));

    // US English, Unicode — the same pair the string table is keyed by.
    let translation: Vec<u8> = [0x0409u16, 0x04B0].iter().flat_map(|v| v.to_le_bytes()).collect();

    node(
        "VS_VERSION_INFO",
        Value::Binary(&fixed),
        &[
            node("StringFileInfo", Value::None, &[node("040904B0", Value::None, &strings)]),
            node(
                "VarFileInfo",
                Value::None,
                &[node("Translation", Value::Binary(&translation), &[])],
            ),
        ],
    )
}

/// What one version-resource block holds besides its key and children.
enum Value<'a> {
    None,
    Binary(&'a [u8]),
    Text(&'a str),
}

/// One block of the version resource.
///
/// Every block in the format has the same shape — length, value length, type, a NUL-terminated
/// UTF-16 key, padding, the value, padding, the children — so this is the only place the layout
/// rules are written down. Each block is built as if it started at offset zero, which is
/// equivalent because each one is placed at a four-byte boundary in its parent.
///
/// The value length counts bytes for binary values and UTF-16 units (terminator included) for
/// text, which is the format's own inconsistency rather than this function's.
fn node(key: &str, value: Value, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; 6];
    out.extend(utf16z(key));
    pad(&mut out);
    let (value_length, kind) = match value {
        Value::None => (0, 1u16),
        Value::Binary(bytes) => {
            out.extend_from_slice(bytes);
            (bytes.len(), 0)
        }
        Value::Text(text) => {
            let units = utf16z(text);
            let count = units.len() / 2;
            out.extend(units);
            (count, 1)
        }
    };
    for child in children {
        pad(&mut out);
        out.extend_from_slice(child);
    }
    let length = out.len() as u16;
    out[0..2].copy_from_slice(&length.to_le_bytes());
    out[2..4].copy_from_slice(&(value_length as u16).to_le_bytes());
    out[4..6].copy_from_slice(&kind.to_le_bytes());
    out
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn pad(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
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
