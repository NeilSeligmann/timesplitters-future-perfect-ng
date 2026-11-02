// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/t2049_nx_007
// portreg: replaces=tsfp-ports/c2r/ui_menu
// portreg: wraps=tsfp-gameplay/modes/story_sequencer
// portreg: wraps=tsfp-gameplay/modes/story_menu
//! T2431: lane adapter for the Story progression sequencer `0x65940` (cdecl, one stack argument:
//! the mode block, `0x78AD58` in the original callers). The readable port is
//! `tsfp-gameplay/src/modes/story_sequencer.rs` (compiled here through `modes_lane::modes`); it
//! builds a typed mode block that the existing Rust `0x658D0` copy (`story_flow::copy_setup`)
//! stages. Tier 1 against the lifted original used FABRICATED inputs (weak); the full-route
//! owner Story rule-24 gate passed at Tier 2. The Rust path is the default;
//! `TSFP_STORY_SEQUENCER=off` retains the lifted implementation for A/B checks.
//!
//! The guest services stay calls through the hybrid bridge: `0x64D10`, `0x64DD0`, `0x1CD590`,
//! `0x35DE70`, `0x389B0`. Tables are read from the running image, nothing is embedded.
use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::HostBridge,
    memory::GuestMemory,
    modes_lane::modes,
};
use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Synchronous host receiver for a completed, typed Story gameplay setup.
/// `setup` points to exactly `BLOCK_BYTES` readable bytes for the duration of the call.
pub type StorySequencerOutputCallback = unsafe extern "C" fn(ordinal: u32, setup: *const u8) -> u32;

static OUTPUT_CALLBACK: AtomicUsize = AtomicUsize::new(0);

/// Register or clear the Bevy-owned receiver for typed sequencer outputs.
///
/// # Safety
/// The function pointer must remain callable until replaced or cleared. The callback must copy
/// the setup bytes before returning; the pointer refers to the sequencer's local typed block.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_ports_register_story_sequencer_output(
    callback: Option<StorySequencerOutputCallback>,
) {
    OUTPUT_CALLBACK.store(
        callback.map_or(0, |callback| callback as usize),
        Ordering::Release,
    );
}

fn dispatch_output(
    callback: Option<StorySequencerOutputCallback>,
    output: Option<&modes::story_sequencer::StorySequencerOutput>,
) -> bool {
    let (Some(callback), Some(output)) = (callback, output) else {
        return false;
    };
    // SAFETY: the callback registration contract requires synchronous copying of this block.
    unsafe {
        callback(
            u32::from(output.campaign_ordinal().get()),
            output.setup().bytes().as_ptr(),
        ) != 0
    }
}

fn dispatch_registered_output(output: Option<&modes::story_sequencer::StorySequencerOutput>) {
    let address = OUTPUT_CALLBACK.load(Ordering::Acquire);
    let callback = (address != 0).then(|| {
        // SAFETY: registration documents the function pointer lifetime and ABI.
        unsafe { core::mem::transmute::<usize, StorySequencerOutputCallback>(address) }
    });
    let _ = dispatch_output(callback, output);
}

/// `0x65940` the sequencer and `0x2D3BF0` the Story/credits page tick. The mode 4 branch of `0x2D2C70`
/// is routed from `modes_lane` through [`run_mode_4`] (that VA is owned by the modes lane).
pub const ROOTS: [u32; 2] = [0x65940, 0x2d3bf0];
// `0x2D2C70` is dispatched and registered by `modes_lane`; this switch still gates its Story
// mode-4 branch without adding a duplicate host root.
const OPT_IN_ROOTS: [u32; 3] = [0x65940, 0x2d3bf0, 0x2d2c70];
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_STORY_SEQUENCER",
    on: c"rust",
    roots: &OPT_IN_ROOTS,
};

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// Rust is default-on; setting `TSFP_STORY_SEQUENCER` to a value other than `rust`
/// explicitly selects the lifted path for A/B checks.
#[cfg(not(test))]
pub(crate) fn enabled() -> bool {
    if !crate::retired::lifted_selectable("story-sequencer") {
        return true;
    }
    unsafe {
        let value = getenv(OPT_IN.env.as_ptr());
        value.is_null() || core::ffi::CStr::from_ptr(value).to_bytes() == OPT_IN.on.to_bytes()
    }
}

#[cfg(test)]
pub(crate) fn enabled() -> bool {
    true
}

/// Selector hook: a root is declined when the A/B switch selects the lifted implementation.
pub fn declined(va: u32) -> bool {
    OPT_IN_ROOTS.contains(&va) && !enabled()
}

/// Dispatches the root; `false` for other VAs or while the switch is off.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) || !enabled() {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        let entry = *regs;
        let argument = view.read32(entry.esp.wrapping_add(4));
        let cell = RefCell::new(&mut *view);
        let mut bridge = HostBridge::new(&cell, guest, &entry);
        match va {
            0x2d3bf0 => {
                crate::ui_menu_capture::run_page_tick(
                    &mut bridge,
                    argument,
                    regs,
                    |bridge, page| crate::modes_lane::story_page_tick(bridge, page, entry.ecx),
                    HostBridge::return_regs,
                );
                // The lifted branch's last EAX assignment is the global menu mode. The tick itself
                // already ran the match-state hand-off (`0x1553E0`, `0x2D38C0`): repeating it here
                // started the Story session twice per tick (T3784 gate divergence at poll 960).
                let mode = bridge.memory().read32(modes::story_menu::MENU_MODE);
                if mode != 1 {
                    regs.eax = mode;
                }
            }
            _ => {
                let (output, exit_eax) =
                    modes::story_sequencer::story_progression_sequencer_with_exit(
                        &mut bridge,
                        argument,
                    );
                dispatch_registered_output(output.as_ref());
                // The lifted sequencer returns the level word it stored last when it ends in a
                // level block, else the EAX left by its final nested cdecl call; if it made no
                // call, the entry EAX remains live. Publish the same value from the bridge.
                regs.eax = exit_eax.unwrap_or_else(|| bridge.return_regs().0);
            }
        }
        true
    })
}

/// The Story branch (menu mode 4) of `0x2D2C70`, entered from `modes_lane`. `false` leaves the original
/// running (switch off, or the menu mode is not Story).
pub fn run_mode_4<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled() || m.read32(modes::story_menu::MENU_MODE) != modes::story_menu::MODE_STORY {
        return false;
    }
    crate::anim_lane::guarded(0x2d2c70, m, guest, regs, |view, guest, regs| {
        let entry = *regs;
        let cell = RefCell::new(&mut *view);
        let mut bridge = HostBridge::new(&cell, guest, &entry);
        // Always handled: a re-run of the original after the Rust prologue would repeat its services.
        if modes::story_menu::setup_mode_4_prologue(&mut bridge)
            == modes::story_menu::Mode4Step::Sequencer
        {
            // A guest call, as in the original: the sequencer stays a visible `0x65940` root call
            // (call-argument stream, Rust call counters) and publishes its own typed output.
            // The original's frame keeps the callee-saved EDI, EBX and ESI it pushed above the
            // argument (the Tier 2 stream records those stack words), so the call repeats them.
            bridge.call_cdecl(
                0x65940,
                &[
                    modes::story_menu::MODE_BLOCK,
                    entry.edi,
                    entry.ebx,
                    entry.esi,
                ],
            );
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicU32;
    use std::collections::BTreeMap;

    static CAPTURED_ORDINAL: AtomicU32 = AtomicU32::new(u32::MAX);
    static CAPTURED_CHECKSUM: AtomicU32 = AtomicU32::new(0);

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct ReturnTwelve(Vec<u32>);
    impl GuestCall for ReturnTwelve {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.0.push(va);
            regs.eax = 0x0c;
        }
    }

    unsafe extern "C" fn capture(ordinal: u32, setup: *const u8) -> u32 {
        let bytes =
            unsafe { core::slice::from_raw_parts(setup, modes::story_sequencer::BLOCK_BYTES) };
        let checksum = bytes.iter().fold(0x811c_9dc5u32, |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u32::from(*byte)
        });
        CAPTURED_ORDINAL.store(ordinal, Ordering::Relaxed);
        CAPTURED_CHECKSUM.store(checksum, Ordering::Relaxed);
        1
    }

    #[test]
    fn output_callback_receives_the_bounded_setup_and_campaign_ordinal() {
        use modes::story_sequencer::{ModeBlock, Phase, SequencerState, StorySequencerOutput};

        let mut bytes = [0u8; modes::story_sequencer::BLOCK_BYTES];
        bytes[0x54] = 2;
        bytes[0x55] = crate::story_flow::MODE_STORY;
        bytes[0x58] = 0x35;
        let setup = ModeBlock::from_bytes(bytes);
        let output = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 3,
                phase: Phase::Level,
            },
            setup,
        )
        .unwrap();
        let expected_checksum = bytes.iter().fold(0x811c_9dc5u32, |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u32::from(*byte)
        });

        assert!(dispatch_output(Some(capture), Some(&output)));
        assert_eq!(CAPTURED_ORDINAL.load(Ordering::Relaxed), 3);
        assert_eq!(CAPTURED_CHECKSUM.load(Ordering::Relaxed), expected_checksum);
    }

    #[test]
    fn opt_in_selector_covers_mode_root_owned_by_modes_lane() {
        assert!(OPT_IN_ROOTS.contains(&0x2d2c70));
        assert!(!ROOTS.contains(&0x2d2c70));
        assert!(!declined(0x2d2c70));
    }

    #[test]
    fn missing_output_or_receiver_is_a_noop() {
        assert!(!dispatch_output(None, None));
        assert!(!dispatch_output(Some(capture), None));
    }

    /// Records every guest call; `0x70d90` (page not finished) answers nonzero.
    #[derive(Default)]
    struct RecordCalls(Vec<u32>);
    impl GuestCall for RecordCalls {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.0.push(va);
            regs.eax = 1;
        }
    }

    #[test]
    fn story_page_tick_starts_the_session_once_when_the_match_state_is_one() {
        let mut memory = Memory::default();
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x20000;
        memory.write32(regs.esp.wrapping_add(4), 0x5236dc);
        memory.write32(modes::story_menu::MENU_MODE, 4);
        memory.write32(modes::story_menu::MATCH_STATE, 1);
        let mut guest = RecordCalls::default();

        assert!(run(0x2d3bf0, &mut memory, &mut guest, &mut regs));
        let started = |va: u32| guest.0.iter().filter(|call| **call == va).count();
        assert_eq!(started(0x2d38c0), 1, "calls: {:x?}", guest.0);
        assert_eq!(started(0x1553e0), 1);
        assert_eq!(regs.eax, 4);
    }

    #[test]
    fn sequencer_root_returns_eax_from_its_last_nested_call() {
        let mut memory = Memory::default();
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.eax = 0x4016_fea8;
        // HostBridge's nested cdecl argument frame sits 0x800 bytes below ESP; keep it in the
        // guarded guest range so the fake guest call is not skipped as an invalid low address.
        regs.esp = 0x20000;
        memory.write32(regs.esp.wrapping_add(4), 0x78ad58);
        memory.write32(modes::story_sequencer::CURSOR, 0);
        memory.write32(modes::story_sequencer::PHASE, 0);
        let mut guest = ReturnTwelve::default();

        assert!(run(0x65940, &mut memory, &mut guest, &mut regs));
        assert!(!guest.0.is_empty());
        assert_eq!(regs.eax, 0x0c);
    }
}
