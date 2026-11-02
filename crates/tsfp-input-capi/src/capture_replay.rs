// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline selector-only replay. Handler execution and lifecycle history are
//! outside this comparison; its result never removes input_unverified.

use crate::dispatch::CaptureSelection;
use tsfp_input::dispatch::{Registry, SURFACE};
use tsfp_input::dispatch_capture::HandlerId;

const INCOMPLETE: u32 = 0;
const EQUAL: u32 = 1;
const DIFFERENT: u32 = 2;

macro_rules! markers {
    ($(($name:ident, $id:ident)),+ $(,)?) => {
        $(fn $name(_: &mut ()) -> u32 { HandlerId::$id as u32 })+
        fn marker_for(id: u32) -> Option<fn(&mut ()) -> u32> {
            match HandlerId::from_wire(id) { $(HandlerId::$id => Some($name),)+ _ => None }
        }
    };
}
markers!(
    (m1, MuMount),
    (m2, MuUnmount),
    (m3, InputOpen),
    (m4, InputClose),
    (m5, InputCapabilities),
    (m6, InputState),
    (m7, InputFeedback),
    (m8, DevicesInit),
    (m9, DevicesGet),
    (m10, DevicesPeek),
    (m11, DevicesChanges),
    (m12, DevicesStatus),
    (m13, VoiceCreateEx)
);

fn replay(snapshot: &CaptureSelection) -> u32 {
    replay_selected(snapshot, snapshot.address)
}

fn replay_selected(snapshot: &CaptureSelection, selector_address: u32) -> u32 {
    if snapshot.complete != 1
        || snapshot.generation == 0
        || snapshot.selection_sequence == 0
        || snapshot.selection_sequence > snapshot.lifecycle_sequence
        || snapshot.entry_count as usize != SURFACE.len()
        || snapshot.entry_count as usize > snapshot.entries.len()
        || !matches!(snapshot.route, 1..=3)
    {
        return INCOMPLETE;
    }
    let mut registry = Registry::<()>::default();
    let mut defaults = [0u32; SURFACE.len()];
    let mut seen = [false; SURFACE.len()];
    for entry in &snapshot.entries[..snapshot.entry_count as usize] {
        let Some(index) = SURFACE
            .iter()
            .position(|item| item.address == entry.address)
        else {
            return INCOMPLETE;
        };
        if seen[index]
            || (entry.handler != 0 && HandlerId::from_wire(entry.handler) == HandlerId::Unknown)
        {
            return INCOMPLETE;
        }
        seen[index] = true;
        defaults[index] = entry.default_return;
        if !registry.set_default_return(entry.address, entry.default_return) {
            return INCOMPLETE;
        }
        if entry.handler != 0 {
            let Some(marker) = marker_for(entry.handler) else {
                return INCOMPLETE;
            };
            if !registry.register(entry.address, marker) {
                return INCOMPLETE;
            }
        }
    }
    if seen.iter().any(|item| !item) {
        return INCOMPLETE;
    }
    let Some(index) = SURFACE
        .iter()
        .position(|item| item.address == snapshot.address)
    else {
        return INCOMPLETE;
    };
    let selected = registry.prepare_call(selector_address);
    let has_handler = selected.has_handler();
    // Markers are pure Rust and return stable IDs. The actual C handler is
    // never invoked, including on a nonlocal-exit path.
    let value = selected.finish(&mut ()).value;
    let handler = if has_handler { value } else { 0 };
    // A stub's returned default comes from the real Registry. For a registered
    // handler the default is inactive, so only its captured state is compared.
    let default = if has_handler { defaults[index] } else { value };
    if handler != snapshot.handler
        || default != snapshot.default_return
        || (snapshot.route == 3) != (handler == 0)
    {
        DIFFERENT
    } else {
        EQUAL
    }
}

/// Returns 0=incomplete, 1=equal selector pre-state, 2=different. Only pure
/// Rust identity markers run; this is not a Tier 2 or handler-behavior verdict.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_replay_selection(
    snapshot: *const CaptureSelection,
) -> u32 {
    match unsafe { snapshot.as_ref() } {
        Some(snapshot) => replay(snapshot),
        None => INCOMPLETE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::CaptureEntry;

    fn valid() -> CaptureSelection {
        let mut entries = [CaptureEntry::default(); 64];
        for (entry, surface) in entries.iter_mut().zip(SURFACE) {
            entry.address = surface.address;
        }
        entries[0].handler = 3;
        entries[0].default_return = 9;
        entries[1].handler = 4;
        CaptureSelection {
            generation: 1,
            lifecycle_sequence: 4,
            selection_sequence: 1,
            address: SURFACE[0].address,
            route: 2,
            handler: 3,
            default_return: 9,
            complete: 1,
            entry_count: SURFACE.len() as u32,
            entries,
        }
    }

    #[test]
    fn replay_selects_using_real_registry_and_rejects_changed_outcomes() {
        let mut snapshot = valid();
        assert_eq!(replay(&snapshot), EQUAL);
        // Both rows are populated; selecting the adjacent handler must diverge.
        assert_eq!(replay_selected(&snapshot, SURFACE[1].address), DIFFERENT);
        snapshot.handler = 4;
        assert_eq!(replay(&snapshot), DIFFERENT);
        snapshot.handler = 3;
        snapshot.default_return = 8;
        assert_eq!(replay(&snapshot), DIFFERENT);
        snapshot = valid();
        snapshot.entries[0].handler = 0;
        snapshot.handler = 0;
        snapshot.route = 3;
        assert_eq!(replay(&snapshot), EQUAL);
        snapshot.default_return = 8;
        assert_eq!(replay(&snapshot), DIFFERENT);
    }

    #[test]
    fn replay_rejects_incomplete_unknown_duplicate_and_invalid_sequence() {
        let mut snapshot = valid();
        snapshot.complete = 0;
        assert_eq!(replay(&snapshot), INCOMPLETE);
        snapshot.complete = 1;
        snapshot.entries[0].handler = 99;
        assert_eq!(replay(&snapshot), INCOMPLETE);
        snapshot = valid();
        snapshot.entries[1].address = snapshot.entries[0].address;
        assert_eq!(replay(&snapshot), INCOMPLETE);
        snapshot = valid();
        snapshot.selection_sequence = 5;
        assert_eq!(replay(&snapshot), INCOMPLETE);
    }
}
