// SPDX-License-Identifier: GPL-3.0-or-later
//! T2197 real-data check: decode the name-field specs and key grids from the owner's
//! `default.xbe` at run time (TSFP_XBE, else build/default.xbe of the checkout). Skips with a
//! notice when no image exists. Asserts structure only, no original bytes are stored here.
use std::path::{Path, PathBuf};
use tsfp_text_field::field_spec::{self, NameField};
use tsfp_text_field::keyboard::{FieldFlags, KeyGrid, all_tables, select_table};
use tsfp_text_field::name_entry::{NameEntry, ScaledWidth};
use tsfp_text_field::{Event, Outcome};
use tsfp_ui_model::xbe::XbeImage;

fn image() -> Option<XbeImage> {
    let path = std::env::var("TSFP_XBE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/default.xbe"));
    XbeImage::load(&path)
}

#[test]
fn name_fields_and_grids_from_the_owner_image() {
    let Some(xbe) = image() else {
        eprintln!("SKIP: no default.xbe (set TSFP_XBE)");
        return;
    };
    let read = |a| xbe.byte(a);
    let limits: Vec<(NameField, u32, bool)> = NameField::ALL
        .into_iter()
        .map(|f| {
            let s = field_spec::load(f, read).expect("constructor call decodes");
            (f, s.limit, s.width_scale > 0.0)
        })
        .collect();
    assert_eq!(
        limits,
        vec![
            (NameField::ProfileName, 12, false),
            (NameField::MapName, 32, true),
            (NameField::MapDescription, 224, true),
            (NameField::SaveName, 31, true),
        ]
    );
    for table in all_tables() {
        let grid = KeyGrid::load(table, read).expect("grid inside the image");
        assert_eq!(grid.keys.len(), table.key_count());
        assert!(!grid.charset().is_empty(), "{table:?} has characters");
    }
    // Base and shifted tables differ (case).
    let base = KeyGrid::load(select_table(FieldFlags(0)), read).unwrap();
    let shifted = KeyGrid::load(select_table(FieldFlags(FieldFlags::SHIFT)), read).unwrap();
    assert_ne!(base.keys, shifted.keys);
    // Every real name field builds, accepts its first base key and routes done/cancel.
    for f in NameField::ALL {
        let spec = field_spec::load(f, read).unwrap();
        let mut entry = NameEntry::new(spec, b"", false, read).unwrap();
        let slot = (0..entry.grid().keys.len())
            .find(|i| entry.grid().key(*i).is_some())
            .unwrap();
        let mut width = ScaledWidth {
            scale: spec.width_scale,
            viewport_x: 1.0,
            measure: |_: &[u8]| 0.0,
        };
        entry.press_key(slot, &mut width).unwrap();
        entry.apply(Event::Confirm).unwrap();
        assert_eq!(entry.field.outcome(), Outcome::Confirmed);
        assert_eq!(entry.outcome_handler(), Some(spec.done));
        assert_ne!(spec.done, spec.cancel);
    }
}
