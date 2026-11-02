// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared location contract for private HUD oracle cases.
use std::path::PathBuf;

pub fn oracle_dir() -> PathBuf {
    match std::env::var("HUD_ORACLE_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/hud/oracle"),
    }
}
