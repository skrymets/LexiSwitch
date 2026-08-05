//! Spike 3 — can layout tables be derived at runtime?
//!
//! Question: can `GetKeyboardLayoutList` + `ToUnicodeEx` produce complete
//! `(scancode, shift) -> char` tables for uk, en-US and de-QWERTZ, including sane handling
//! of German dead keys? ADR-0004 rests on the answer.
//!
//! Throwaway. `unwrap` is fine here, duplication is fine here, and nothing in this file is
//! meant to survive into the product. The artifact that survives is the JSON.
//!
//!   cargo run                 print tables and the flush proof
//!   cargo run -- --out DIR    also write {uk,en_us,de}.json into DIR

use std::collections::BTreeMap;

use windows::Win32::Globalization::LCIDToLocaleName;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayoutList, MapVirtualKeyExW, ToUnicodeEx, HKL, MAPVK_VK_TO_VSC,
    MAPVK_VSC_TO_VK_EX,
};

/// Scancodes for the alphanumeric and punctuation block, in physical rows. Set 1 scancodes,
/// which is what Raw Input reports and what the buffer will store.
const SCANCODES: &[u16] = &[
    // number row: 1 2 3 4 5 6 7 8 9 0 - =
    0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, //
    // top row: Q W E R T Y U I O P [ ]
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, //
    // home row: A S D F G H J K L ; '
    0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, //
    0x29, // the key left of 1: ` on US, ^ (dead) on German
    0x2B, // backslash
    // bottom row: Z X C V B N M , . /
    0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, //
    0x56, // ISO extra key, present on German as <>|
];

const VK_SHIFT: usize = 0x10;
const VK_SPACE: u32 = 0x20;

/// What `ToUnicodeEx` reported for one physical position in one shift state.
#[derive(Debug, Clone, PartialEq)]
enum Probe {
    Char(String),
    Dead,
    None,
}

fn main() {
    let layouts = installed_layouts();

    println!("== installed layouts ==");
    for &hkl in &layouts {
        let raw = hkl_raw(hkl);
        println!(
            "  HKL 0x{:08x}  langid 0x{:04x}  {}",
            raw,
            raw & 0xFFFF,
            locale_name(raw & 0xFFFF)
        );
    }
    println!();

    // Task 2.5 runs before the tables are built: if the flush does not work, the tables are
    // not worth printing.
    prove_flush(&layouts);

    let mut tables: BTreeMap<String, (u32, String, Vec<(u16, bool, Probe)>)> = BTreeMap::new();
    for &hkl in &layouts {
        let raw = hkl_raw(hkl);
        let langid = raw & 0xFFFF;
        let name = match langid {
            0x0409 => "en_us",
            0x0422 => "uk",
            0x0407 => "de",
            _ => continue,
        };
        tables.insert(name.to_string(), (raw, locale_name(langid), probe_layout(hkl)));
    }

    for (name, (raw, locale, rows)) in &tables {
        println!("== {name} (HKL 0x{raw:08x}, {locale}) ==");
        print_human(rows);
        println!();
    }

    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--out") {
        let dir = &args[i + 1];
        std::fs::create_dir_all(dir).unwrap();
        for (name, (raw, locale, rows)) in &tables {
            let path = format!("{dir}/{name}.json");
            std::fs::write(&path, to_json(name, *raw, locale, rows)).unwrap();
            println!("wrote {path}");
        }
    } else {
        for (name, (raw, locale, rows)) in &tables {
            println!("== {name}.json ==");
            println!("{}", to_json(name, *raw, locale, rows));
        }
    }
}

fn installed_layouts() -> Vec<HKL> {
    // SAFETY: first call with an empty buffer asks for the count; the second is given a
    // buffer of exactly that length.
    unsafe {
        let n = GetKeyboardLayoutList(None);
        assert!(n > 0, "GetKeyboardLayoutList reported no layouts");
        let mut list = vec![HKL::default(); n as usize];
        let got = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(got as usize);
        list
    }
}

fn hkl_raw(hkl: HKL) -> u32 {
    hkl.0 as usize as u32
}

fn locale_name(langid: u32) -> String {
    let mut buf = [0u16; 85];
    // SAFETY: buf is a fixed 85-wide array, the maximum LOCALE_NAME_MAX_LENGTH.
    let n = unsafe { LCIDToLocaleName(langid, Some(&mut buf), 0) };
    if n <= 0 {
        return format!("<unresolved 0x{langid:04x}>");
    }
    String::from_utf16_lossy(&buf[..(n as usize - 1)])
}

/// Clear any pending dead-key state so the next probe starts clean.
///
/// This is the whole hazard of the spike. `ToUnicodeEx` is stateful: probing a dead key
/// leaves the accent pending inside the layout, and the *next* probe silently returns a
/// combined character with no error. Pressing a known non-dead key consumes the pending
/// state.
fn flush(hkl: HKL) {
    let state = [0u8; 256];
    let mut buf = [0u16; 16];
    // SAFETY: state is the required 256 bytes; buf is a real buffer with its length passed.
    unsafe {
        let sc = MapVirtualKeyExW(VK_SPACE, MAPVK_VK_TO_VSC, Some(hkl));
        for _ in 0..8 {
            if ToUnicodeEx(VK_SPACE, sc, &state, &mut buf, 0, Some(hkl)) >= 0 {
                break;
            }
        }
    }
}

fn probe(hkl: HKL, scancode: u16, shift: bool) -> Probe {
    let mut state = [0u8; 256];
    if shift {
        state[VK_SHIFT] = 0x80;
    }
    let mut buf = [0u16; 16];

    // SAFETY: state is 256 bytes as the API requires; buf's length is passed with it. The
    // scancode is mapped to a virtual key under this specific layout rather than the
    // active one, which is the entire point of the Ex variants.
    unsafe {
        let vk = MapVirtualKeyExW(scancode as u32, MAPVK_VSC_TO_VK_EX, Some(hkl));
        if vk == 0 {
            return Probe::None;
        }
        let n = ToUnicodeEx(vk, scancode as u32, &state, &mut buf, 0, Some(hkl));
        match n {
            // A dead key. Record it as dead — inventing a resolved character here would put
            // a wrong answer into the fixture that every later test treats as ground truth.
            -1 => Probe::Dead,
            0 => Probe::None,
            n => Probe::Char(String::from_utf16_lossy(&buf[..n as usize])),
        }
    }
}

/// Probe every position in both shift states, flushing before each one.
fn probe_layout(hkl: HKL) -> Vec<(u16, bool, Probe)> {
    let mut out = Vec::new();
    for &sc in SCANCODES {
        for shift in [false, true] {
            flush(hkl);
            let p = probe(hkl, sc, shift);
            if p == Probe::Dead {
                // Consume the state this probe just created, so it cannot leak forward.
                flush(hkl);
            }
            out.push((sc, shift, p));
        }
    }
    out
}

/// Task 2.5 — a spike that does not test its own flush proves nothing.
///
/// Probes the German dead key `^` (scancode 0x29), then probes `e` (0x12) twice: once
/// without flushing, to show the hazard is real, and once with, to show the fix works.
fn prove_flush(layouts: &[HKL]) {
    let Some(&de) = layouts.iter().find(|h| hkl_raw(**h) & 0xFFFF == 0x0407) else {
        println!("== flush proof: SKIPPED, German layout not installed ==\n");
        return;
    };

    println!("== flush proof (German) ==");
    flush(de);

    let dead = probe(de, 0x29, false);
    println!("  probe 0x29 (^)            -> {dead:?}");

    // No flush: this is the failure mode the flush exists to prevent.
    let unflushed = probe(de, 0x12, false);
    println!("  probe 0x12 (e), NO flush  -> {unflushed:?}   <- contaminated if not \"e\"");

    // Same probe, this time with the state cleared first.
    flush(de);
    let flushed = probe(de, 0x12, false);
    println!("  probe 0x12 (e), flushed   -> {flushed:?}");

    let verdict = match (&dead, &unflushed, &flushed) {
        (Probe::Dead, u, Probe::Char(f)) if f == "e" && u != &Probe::Char("e".into()) => {
            "PASS - hazard reproduced, flush clears it"
        }
        (Probe::Dead, _, Probe::Char(f)) if f == "e" => {
            "PASS (weak) - flush works, but the unflushed probe was not contaminated"
        }
        (Probe::Dead, _, _) => "FAIL - flush did not restore a clean probe",
        _ => "INCONCLUSIVE - 0x29 did not report as a dead key",
    };
    println!("  verdict: {verdict}\n");
    flush(de);
}

fn print_human(rows: &[(u16, bool, Probe)]) {
    for (sc, shift, p) in rows {
        let s = match p {
            Probe::Char(c) => format!("{c:?}"),
            Probe::Dead => "DEAD".to_string(),
            Probe::None => "-".to_string(),
        };
        let tag = if *shift { "shift" } else { "     " };
        println!("  0x{sc:02x} {tag} -> {s}");
    }
}

fn to_json(name: &str, raw: u32, locale: &str, rows: &[(u16, bool, Probe)]) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"layout\": \"{name}\",\n"));
    s.push_str(&format!("  \"hkl\": \"0x{raw:08x}\",\n"));
    s.push_str(&format!("  \"locale\": \"{locale}\",\n"));
    s.push_str("  \"positions\": [\n");
    let body: Vec<String> = rows
        .iter()
        .filter(|(_, _, p)| *p != Probe::None)
        .map(|(sc, shift, p)| match p {
            Probe::Dead => format!(
                "    {{ \"scancode\": {sc}, \"shift\": {shift}, \"dead\": true }}"
            ),
            Probe::Char(c) => format!(
                "    {{ \"scancode\": {sc}, \"shift\": {shift}, \"char\": \"{}\" }}",
                escape(c)
            ),
            Probe::None => unreachable!(),
        })
        .collect();
    s.push_str(&body.join(",\n"));
    s.push_str("\n  ]\n}\n");
    s
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
