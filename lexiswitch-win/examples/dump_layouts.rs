//! Print every layout table derived from the layouts Windows currently has installed.
//!
//! Task 5.7 exists to confirm that production probing agrees with the frozen fixtures the
//! spike generated. Output is deterministic and sorted so it can be diffed mechanically
//! rather than eyeballed:
//!
//! ```text
//! # de-DE hkl=0x04070407 langid=0x0407
//! 2,0,1
//! 2,1,!
//! 13,0,DEAD
//! ```
//!
//! Fields are `scancode,shift,value`, with `DEAD` for dead-key positions.
//!
//!   cargo run -p lexiswitch-win --example dump_layouts

use lexiswitch_core::layout::KeyValue;

fn main() {
    let layouts = match lexiswitch_win::layouts::build_all() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not build layout tables: {e}");
            std::process::exit(1);
        }
    };

    for layout in &layouts {
        println!(
            "# {} hkl={:#010x} langid={:#06x} positions={}",
            layout.name,
            layout.hkl,
            layout.langid,
            layout.table.len()
        );

        let mut rows: Vec<(u16, bool, String)> = layout
            .table
            .positions()
            .map(|(pos, value)| {
                let text = match value {
                    KeyValue::Dead => "DEAD".to_string(),
                    KeyValue::Char(c) => c.to_string(),
                };
                (pos.scancode, pos.shift, text)
            })
            .collect();
        rows.sort();

        for (scancode, shift, value) in rows {
            println!("{scancode},{},{value}", u8::from(shift));
        }
    }
}
