//! Loading frozen layout fixtures. Test-only by design.
//!
//! The fixtures in `tests/fixtures/` were generated once by
//! `spikes/spike3_layout_tables` from the layouts installed on the development machine, and
//! are committed frozen so these tests do not depend on the machine they run on — or on
//! Windows existing at all (ADR-0001).
//!
//! This loader lives in the test tree rather than the library because JSON is not part of
//! how the product builds tables. At runtime `lexiswitch-win` probes `ToUnicodeEx` and
//! passes plain data to `LayoutTable::new`.

use std::path::PathBuf;

use lexiswitch_core::layout::{KeyPos, KeyValue, LayoutTable};

/// Load `tests/fixtures/<name>.json` into a `LayoutTable`.
///
/// Panics loudly on anything unexpected: a fixture that cannot be read or parsed is a
/// broken test setup, not a condition to handle.
pub fn load(name: &str) -> LayoutTable {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.json"));

    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading fixture {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("parsing fixture {}: {e}", path.display()));

    let layout = doc["layout"]
        .as_str()
        .unwrap_or_else(|| panic!("fixture {name} has no \"layout\" field"))
        .to_string();

    let entries: Vec<(KeyPos, KeyValue)> = doc["positions"]
        .as_array()
        .unwrap_or_else(|| panic!("fixture {name} has no \"positions\" array"))
        .iter()
        .map(|p| {
            let pos = KeyPos::new(
                p["scancode"].as_u64().expect("scancode") as u16,
                p["shift"].as_bool().expect("shift"),
            );
            let value = if p.get("dead").and_then(|d| d.as_bool()).unwrap_or(false) {
                KeyValue::Dead
            } else {
                let s = p["char"].as_str().expect("char");
                let mut chars = s.chars();
                let c = chars.next().unwrap_or_else(|| panic!("empty char at {pos:?}"));
                // ToUnicodeEx can in principle return more than one UTF-16 unit for a
                // position. None of the three target layouts does, and a multi-character
                // position would break the character-to-position inverse that conversion
                // depends on — so fail rather than silently keep the first.
                assert!(
                    chars.next().is_none(),
                    "fixture {name} has a multi-character position at {pos:?}: {s:?}"
                );
                KeyValue::Char(c)
            };
            (pos, value)
        })
        .collect();

    LayoutTable::new(layout, entries)
}
