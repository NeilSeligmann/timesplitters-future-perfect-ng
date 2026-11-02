// SPDX-License-Identifier: GPL-3.0-or-later
// T1874: original-instruction lane bridge for the inventory and pose/control roots of T1850
// (docs/t1874-rust-admission.md). The ports use the `ExtRegisters` contract (x87, XMM0-7, callees
// through `GuestCall`); this module hands them the harness/host register file (`LaneCpu`) and
// marshals the x87/XMM globals through the C side. Seven T1850 VAs are not exported here: they are
// already admitted by an earlier implementation (59D70 by T1834, 246D10 427B0 42C30 431C0 43A90
// 43AF0 by T1854) and a selector has one slot per address. `abi.rs` and `ext.rs` are untouched.
use crate::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    lane::LaneCpu,
    memory::{GuestMemory, MappedGuest},
};

unsafe extern "C" {
    /// Appends the selected nested call to the existing opt-in call-args trace.
    fn host_rust_ports_nested_call_args(va: u32, phase: u32, arg0: u32, eax: u32);
    /// Fills `regs.fpu`, `regs.xmm` and `regs.xmm_hi` from the guest globals (g_fp_stack, g_xmm0..7).
    fn tsfp_lane_x_fp_load(regs: *mut ExtRegisters);
    /// Writes `regs.fpu`, `regs.xmm` and `regs.xmm_hi` to the guest globals.
    fn tsfp_lane_x_fp_store(regs: *const ExtRegisters);
    /// Runs the lifted callee of `va` like `rust_ext_guest_call`: loads the whole record, pushes the
    /// return slot, runs, requires `esp == entry + 4` and stores the whole record back.
    fn tsfp_lane_x_call(va: u32, regs: *mut ExtRegisters);
}

struct LaneGuest {
    memory_offset: usize,
}

/// Apply the selected REA inventory/interaction owner at Rust-to-guest boundaries.
/// These nested calls do not necessarily re-enter the exported root dispatcher.
pub(crate) fn dispatch_inventory_owner<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    crate::rea_inventory_runtime::run(va, memory, guest, regs)
}

impl GuestCall for LaneGuest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        // T2161: a nested call of the player update (Rust translation of 0xD5EB0 calling 0x25D520 and the targeting roots)
        // goes through the raw look hooks when a Mouse+Keyboard slot is enabled; otherwise this is the plain call.
        let mut memory = unsafe { MappedGuest::new(crate::raw_look::offset()) };
        // Selected inventory and interaction roots can be nested under this lane just as they
        // can under the weapons lane. Route them before the lifted guest-call fallback.
        if dispatch_inventory_owner(va, &mut memory, self, regs) {
            return;
        }
        // T3421: 0x12F00 is selected inside a translated caller through this
        // nested boundary, so the host root trace hook cannot see its inputs.
        let handle_trace = va == 0x12f00;
        let handle_arg = if handle_trace {
            // GuestCall::call is entered before the x86 return slot is pushed;
            // ESP therefore points at argument one here. run_nested adds the
            // return slot before rea_ai_runtime::run reads [ESP + 4].
            memory.read32(regs.esp)
        } else {
            0
        };
        if handle_trace {
            unsafe { host_rust_ports_nested_call_args(va, 0, handle_arg, 0) };
        }
        if va == 0x000c_ff20
            && crate::player_interaction_runtime::run_nested_targeting_gate(
                va,
                &mut memory,
                self,
                regs,
            )
        {
            return;
        }
        // T3398: D0730 reaches D01C0 as a nested GuestCall, which bypasses the exported
        // player-root selector. Give the default-off typed attachment the same seam here;
        // off/reference runs keep the lifted callee and snapshot its two ABI outputs.
        if va == 0x000d_01c0 {
            let args_esp = regs.esp;
            if crate::player_interaction_runtime::run_nested(va, &mut memory, self, regs) {
                return;
            }
            unsafe { tsfp_lane_x_call(va, regs) };
            crate::player_interaction_runtime::record_lifted_reference(&memory, args_esp, regs.eax);
            return;
        }
        if va == 0x000d_00f0
            && crate::player_interaction_runtime::run_nested_candidate_filter(
                va,
                &mut memory,
                self,
                regs,
            )
        {
            return;
        }
        // The REA executor calls 0x15c700 from lifted code as well as through
        // the exported leaf root. Capture the typed transition before either
        // nested or direct lifted dispatch so callback requests cannot replace
        // the pair committed by this invocation. Snapshot at this narrow
        // boundary so retained lifted producers can still feed the commit.
        let behavior_commit = if va == 0x15c700 {
            let extension = memory.read32(regs.esp);
            crate::rea_ai_runtime::actor_behavior_commit_started(&memory, extension)
        } else {
            None
        };
        // T1662 measures the actor on the stack at 0x26EFC0. Keep its typed
        // identity token across the lifted call and publish state on return.
        let actor_controller = if va == 0x26efc0 {
            crate::rea_ai_runtime::actor_controller_started_at_nested_call(&mut memory, regs.esp)
        } else {
            None
        };
        if crate::weapon_hands_runtime::dispatch_nested_guest_call(va, &mut memory, self, regs) {
            if let Some(token) = behavior_commit {
                crate::rea_ai_runtime::actor_behavior_committed(&memory, token);
            }
            crate::rea_ai_runtime::actor_controller_finished(&memory, actor_controller);
            return;
        }
        if crate::story_event_callback_lane::enabled()
            && crate::story_event_callback_lane::run_nested(va, &mut memory, self, regs)
        {
            return;
        }
        if crate::rea_ai_runtime::enabled()
            && crate::rea_ai_runtime::run_nested(va, &mut memory, self, regs)
        {
            if handle_trace {
                unsafe { host_rust_ports_nested_call_args(va, 1, handle_arg, regs.eax) };
            }
            if let Some(token) = behavior_commit {
                crate::rea_ai_runtime::actor_behavior_committed(&memory, token);
            }
            crate::rea_ai_runtime::actor_controller_finished(&memory, actor_controller);
            return;
        }
        if crate::raw_look::watches(va) {
            crate::raw_look::around(va, false, &mut memory, self, regs, |_, _, regs| {
                unsafe { tsfp_lane_x_call(va, regs) };
                true
            });
        } else {
            unsafe { tsfp_lane_x_call(va, regs) }
        }
        if handle_trace {
            unsafe { host_rust_ports_nested_call_args(va, 1, handle_arg, regs.eax) };
        }
        if let Some(token) = behavior_commit {
            crate::rea_ai_runtime::actor_behavior_committed(&memory, token);
        }
        crate::rea_ai_runtime::actor_controller_finished(&memory, actor_controller);
    }
}

pub const INV_ROOTS: [u32; 1392] = [
    0x426b0, 0x426d0, 0x426e0, 0x42710, 0x42760, 0x42830, 0x42870, 0x428d0, 0x42920, 0x42a30,
    0x42c50, 0x42ca0, 0x42ce0, 0x42d00, 0x585b0, 0x59cf0, 0x59d90, 0x59dd0, 0x59e00, 0x59f10,
    0x59f50, 0x5a1e0, 0x5ba00, 0x606e0, 0x60740, 0x61430, 0x61610, 0x61630, 0x64650, 0x646a0,
    0x65340, 0x660c0, 0x66100, 0x66220, 0x66280, 0x8ea20, 0x949c0, 0xb2d20, 0xbe090, 0xbfb40,
    0xbff70, 0xc0310, 0xc4560, 0xc6140, 0xc63f0, 0xc7950, 0xc9bb0, 0xc9be0, 0xc9c00, 0xc9cc0,
    0xc9f50, 0xcef30, 0xcfec0, 0xd0ca0, 0xd1e20, 0xd2d20, 0xd6920, 0xd7bc0, 0xf8ad0, 0xf8ae0,
    0x115f10, 0x126260, 0x147080, 0x190e80, 0x1b66e0, 0x1b67e0, 0x1b6890, 0x1b68f0, 0x1b6950,
    0x1b6b40, 0x1b6bb0, 0x1b6bf0, 0x1b6c90, 0x1b6d10, 0x1b6e20, 0x1b6ec0, 0x1b9760, 0x1b99b0,
    0x1b9ab0, 0x1b9ad0, 0x1b9cf0, 0x1b9d30, 0x1ba130, 0x1ba1b0, 0x1ba220, 0x1ba2b0, 0x1ba410,
    0x1ba7b0, 0x1ba8f0, 0x1ba9b0, 0x1baa70, 0x1baaf0, 0x1baba0, 0x1bac50, 0x1baca0, 0x1badd0,
    0x1bae40, 0x1baeb0, 0x1bfa60, 0x1bfaa0, 0x1bfc00, 0x1bfca0, 0x1bfe60, 0x1bfec0, 0x1c0080,
    0x1c1a90, 0x1c1d90, 0x22dd90, 0x238460, 0x238510, 0x2385b0, 0x238610, 0x246dc0, 0x2474e0,
    0x258870, 0x259800, 0x259df0, 0x25a080, 0x25b720, 0x25ead0, 0x25ec70, 0x25ef30, 0x2cbce0,
    0x2ce120, 0x2e86a0, 0x2e8c00, 0x3381d0, 0x357160, 0x357320, 0x59f90, 0x61570, 0x5a070, 0x5a2b0,
    0x59d50, 0x59ec0, 0x5a260, 0x5a2f0, 0x614b0, 0x61450, 0x5adc0, 0x5a360, 0x60670, 0x60920,
    0x585e0, 0x59a40, 0x59b00, 0x60540, 0x605d0, 0x604d0, 0x60b30, 0x60f80, 0x5afb0, 0x58690,
    0x5a220, 0xc0c50, 0xcb240, 0x13dcf0, 0x80f40, 0x158c80, 0x1cd1e0, 0x69e70, 0x68610, 0x67280,
    0x431f0, 0x2b0af0, 0x233ef0, 0x69630, 0x69640, 0x696a0, 0x696d0, 0x69720, 0x697d0, 0x69900,
    0x69930, 0x1cd2b0, 0x1cd330, 0x1cd380, 0x312ad0, 0x68830, 0x681c0, 0x265cc0, 0x283730,
    0x2621a0, 0x40260, 0x23f2e0, 0x2d7240, 0x12cb40, 0x12cbb0, 0x336570, 0xfc9f0, 0x42f90, 0x28180,
    0x29f30, 0x29cc0, 0x45670, 0x29d10, 0x28a60, 0x29bb0, 0x457b0, 0x26f00, 0x462f0, 0x459f0,
    0x458b0, 0x28760, 0x28980, 0x45b60, 0x29af0, 0x46350, 0x45a50, 0x45790, 0x26bb0, 0x46610,
    0x28610, 0x44ac0, 0x164e00, 0x1651c0, 0x45540, 0x27bb0, 0x26d20, 0x29dd0, 0x45610, 0x27c20,
    0x18ff70, 0x42970, 0x1ccf80, 0x438b0, 0x1cce70, 0x42b60, 0x426a0, 0x190520, 0x2aefd0, 0x2a5c30,
    // render_lane.rs and render_lane2.rs (T1976): the 62 render roots of T1898 and the render neighbours
    0x21440, 0x21450, 0x214c0, 0x21740, 0x22560, 0x22790, 0x2a630, 0x22950, 0x226e0, 0x22720,
    0x204b0, 0x22ff0, 0x1a010, 0x19640, 0x19d50, 0x19d60, 0x19d40, 0x19650, 0x19d70, 0x8f470,
    0x8f490, 0x8f4e0, 0x1e770, 0x1e7d0, 0x1e7f0, 0x1e970, 0x1eb00, 0x18de0, 0x18ea0, 0x1b7b0,
    0x1d5f0, 0x1ed50, 0x8e710, 0x8f5e0, 0xa2740, 0x10c390, 0x10c400, 0x10c550, 0x153bd0, 0x10d070,
    0x22be0, 0x2a2d0, 0x2a310, 0x2a320, 0x2a340, 0x2a360, 0x1d380, 0x389b0, 0x459d0, 0x1c720,
    0x19cc0, 0x1ed60, 0x1a2e0, 0x1afa0, 0x1b090, 0x1efb0, 0x2a900, 0x2abe0, 0x2ad60, 0x18e910,
    0xffe50, 0x10e410, 0x19240, 0x8f6b0, 0x14d60, 0x20c40, 0x1f790, 0x1c620, 0x1e6b0, 0x21060,
    0x22740, 0x1c600, 0x8f950,
    // object_lane.rs (T2026): the 30 heaviest c2r engine/object roots
    0xa14b0, 0x1518f0, 0x9e840, 0xa1ab0, 0x9e430, 0xa0c50, 0xa7360, 0xbe860, 0xa00b0, 0x9e330,
    0x9fba0, 0x74720, 0x9fd60, 0x9fbe0, 0xa1b80, 0x10e210, 0x162200, 0x2776f0, 0xbdd50, 0x1622c0,
    0xa1fa0, 0xa2430, 0x862c0, 0x48fe0, 0xa6530, 0x9a330, 0x153190, 0x9eb60, 0x23ff20, 0x80430,
    // char_lane.rs (T1996): c2r ai/characters, ai/behavior class tick and pathfinding class tick translations
    0x65230, 0x653b0, 0x65420, 0x65470, 0x658d0, 0x65c70, 0x65f40, 0x667f0, 0x668a0, 0x66fc0,
    0x670c0, 0x674c0, 0x67450, 0x67e60, 0x67f00, 0x68720, 0x15df20, 0x15e4f0, 0x22ce60, 0x23efe0,
    0x25f830,
    // leaf_lane.rs (T2091): leaf writers of SaveSlots, TextBank, ModeSetup, ModeRecord (the stats writers are roots above)
    0x699d0, 0x46030, 0x6a850, 0x69f30, 0x23f390,
    // T2114: PadState/AudioRamp writers (the gate exchange is a root above, redirected by leaf_lane)
    0x42640, 0x450e0, 0x45160,
    // T2114: Missions writers (PickupCells resets, loader, pointer clear, visited marks)
    0x9ce00, 0x9a140, 0x9a260, 0x9a630,
    // T2118: weapon slot and controller writers of the player record (wired by leaf_lane)
    0x1c4400, 0x1c3100, 0x1bf410, 0x1bc890, 0x1c2e80,
    // T2137: weapon slot state functions and a record writer (wired by leaf_lane, batch player2_wired)
    0x1b7680, 0x1bdfd0, 0x1bd820, 0x1bdaf0, 0x219ca0,
    // T2137: PadState/AudioRamp writers and the ammo word writer (wired by leaf_lane, batch leaf3_wired)
    0x425e0, 0x45970, 0x1bc650,
    // T2137 pass 3: leaf4_wired and player3_wired roots (wired by leaf_lane)
    0x9a7a0, 0x9a5b0, 0x9af90, 0x43c50, 0x88950, 0xcf510, 0x151e80, 0x1baee0, 0x153cd0, 0x1b9dc0,
    0x1b7c50,
    // T2122: object pool, character and object record writers (wired by leaf_lane)
    0x9e4b0, 0xa1ea0, 0xa0130, 0xa5c00, 0xa5af0, 0x15eae0, 0x1be870, 0x27ee70, 0x15ab80, 0x15d500,
    0xa7670, 0x9eab0,
    // T2126: the next object and character writers (wired by leaf_lane)
    0xa2b50, 0x9ff10, 0x167780, 0x15e830, 0x1260c0, 0x1099e0, 0x15c6c0, 0xd1cb0, 0x5c5b0, 0xe1200,
    0xa17c0, 0xa5f40, 0xa0860, 0xa7870, 0x15c9d0, 0x57880, 0x15bb50, 0x15cd00,
    // T2129: the writers T2126 skipped as untranslated, the matrix helpers and the duplicate translations (leaf_lane)
    0x3c8960, 0xe8ed0, 0x87310, 0x5f040, 0xc8120, 0x867f0, 0x94a00, 0xa79b0, 0xe97a0, 0x1b6ff0,
    0x164c40, 0x158d40, 0xa2900, 0x261c70, 0x153440, 0x1531d0, 0x153270, 0x151650, 0x153240,
    0x5cef0, 0x5f430, 0xc4700, 0xd8e30, 0x170500, 0xc5310, 0x2b8f90, 0x27efb0, 0x27fe30, 0x1b6a00,
    0x27d590, 0x15af30, 0x15f0d0, 0x261990, 0x276b60, 0x2777e0, 0x276a10, 0x2b9990, 0x27d490,
    0x276540, 0x2771c0, 0x2772b0, 0x277e00, 0x27cbd0, 0x27cef0, 0x27cb10, 0x27b8b0, 0x2663d0,
    0x27f8e0, 0x261cd0, 0x15a540, 0x26c5e0, 0x2778a0, 0x27cc20, 0x27f100, 0x267ca0, 0x276f80,
    0x2774d0, 0x2b05a0, 0xa4b40, 0x1594e0, 0x15a2d0, 0x15e450, 0x160f80, 0x189040, 0x18acc0,
    0x18e580, 0x1b9340, 0x244f30, 0x260760, 0x260940, 0x263c80, 0x264030, 0x265460, 0x265960,
    0x26b5e0, 0x26c7e0, 0x26c9a0, 0x26cb30, 0x26cbe0, 0x26ce20, 0x26d1b0, 0x26d2f0, 0x26d6d0,
    0x26d930, 0x26dbe0, 0x26dd80, 0x26dfb0, 0x26ea00, 0x271bb0, 0x271cb0, 0x272060, 0x272660,
    0x275530, 0x275bb0, 0x276250, 0x2768c0, 0x2768f0, 0x276a00, 0x276ca0, 0x276cb0, 0x276ce0,
    0x276d10, 0x276ef0, 0x2770d0, 0x277530, 0x277600, 0x277690, 0x277bc0, 0x277d30, 0x277d90,
    0x278070, 0x27ba40, 0x27c8c0, 0x27c9a0, 0x27ccd0, 0x27cda0, 0x27df60, 0x27e6c0, 0x27fe90,
    0x281620, 0x281f30, 0x2836d0, 0x283ed0, 0x283f80, 0x283ff0, 0x284070, 0x284430, 0x2847c0,
    0x28d0a0, 0x292430, 0x294520, 0x29cbf0, 0x2a1a90, 0x2a1ae0, 0x2b0320, 0x2b0500, 0x2b0770,
    0x2b0b10, 0x2b1240, 0x2b12f0, 0x2b14c0, 0x2b15a0, 0x2b1b60, 0x2b2330, 0x2b25e0, 0x2b3e90,
    0x2b4870, 0x2b4c70, 0x2b53f0, 0x2b7750, 0x2b7cc0, 0x2b7f30, 0x2b8bd0, 0x2b8ea0, 0x2b8f40,
    0x2b9e90, 0x2ba090, 0x2ba100, 0x2ba160, 0x2ba1c0, 0x37b720, 0x15e120, 0x15fe00, 0x2682a0,
    0x59db0, 0x161730, 0x15d1f0, 0x276660, 0x262b20, 0x26a500, 0x267ff0, 0x276160, 0x5ba40,
    0x15b240, 0x15be50, 0x15be60, 0x161430, 0x188d40, 0x2603f0, 0x261750, 0x262720, 0x262c20,
    0x262d40, 0x263260, 0x263710, 0x263870, 0x263d00, 0x263e20, 0x2648c0, 0x264a10, 0x2651a0,
    0x265820, 0x266620, 0x266b10, 0x266dd0, 0x267450, 0x267710, 0x267790, 0x267d50, 0x269240,
    0x26a7a0, 0x26b7a0, 0x26c0e0, 0x26c630, 0x26d390, 0x272800, 0x273240, 0x273a20, 0x274690,
    0x274ba0, 0x275050, 0x27bec0, 0x27c0c0, 0x27e4e0, 0x27f160, 0x280070, 0x2806b0, 0x280fa0,
    0x2816d0, 0x281fe0, 0x282300, 0x2834b0, 0x284a00, 0x2861a0, 0x286380, 0x286d30, 0x286f10,
    0x2875f0, 0x287860, 0x287e00, 0x287e20, 0x288230, 0x288540, 0x288ab0, 0x288bd0, 0x288e20,
    0x289120, 0x289290, 0x289410, 0x2895a0, 0x289f30, 0x28a0a0, 0x28a290, 0x28a650, 0x28a9d0,
    0x28b610, 0x28b730, 0x28b780, 0x28ba60, 0x28c1c0, 0x28c640, 0x28ca40, 0x28dd50, 0x293730,
    0x2998e0, 0x29ce00, 0x29fe00, 0x29ff40, 0x2a0d10, 0x2a0e20, 0x2a0ed0, 0x2a1010, 0x2a14b0,
    0x2a16d0, 0x2a17b0, 0x2a29b0, 0x2b1120, 0x2b11b0, 0x2b18e0, 0x2b1a90, 0x2b1e90, 0x2b2050,
    0x2b2750, 0x2b4060, 0x2b4260, 0x2b4510, 0x2b55d0, 0x2b5b90, 0x2b6170, 0x2b66a0, 0x2b68b0,
    0x2ba300, 0x2a1f30, 0x2a4120, 0x2a0500,
    // physics_lane.rs (T2099): c2r engine/physics translations
    0x919d0, 0xa2cd0, 0x47d50, 0x91960, 0xc38f0, 0xa7a00, 0xc3b10, 0xa46a0, 0x475c0, 0x91390,
    0x48590, 0x49370, 0xd6a70, 0xc3c50, 0x1665e0, 0x1665f0, 0xc3e60, 0x167ca0, 0x16b8c0, 0x477b0,
    0x48350, 0x484d0, 0x49c60, 0x4a270, 0x4a800, 0x4a9d0, 0x4ae00, 0x4b310, 0x4b3b0, 0x4fe00,
    0x52550, 0x913b0, 0x913d0, 0x92360, 0xa3050, 0xa3140, 0xa3500, 0x128760, 0x128c60, 0x128ef0,
    0x129ca0, 0x129f50, 0x166b60, 0x16b130, 0x16b550, 0x16b590, 0x16b800, 0x16b850, 0x16b870,
    0x16b8a0, 0x16b900, // physics_lane2.rs (T2117): the heavy mesh ray leaves
    0x47b30, 0x47bd0, 0x913f0,
    // cutscene_lane.rs (T2131): the sixteen cutscene roots, then the nine save_unlock roots (dispatched by save_lane.rs)
    0x3ee20, 0x58240, 0x60390, 0x60410, 0x61680, 0x64d90, 0x64dd0, 0x665b0, 0x6ee00, 0x6f430,
    0x6f4b0, 0x8e970, 0x94220, 0x98da0, 0x98db0, 0x23efc0, 0x1cd3e0, 0x1cd940, 0x1cd040, 0x1cd980,
    0x2be5e0, 0x1cd9d0, 0x1cd9f0, 0x1cda20, 0x1cd890,
    // mapmaker_ui_lane.rs (T2142): the mapmaker/ui roots
    0x70e30, 0x81480, 0x81c10, 0x81c40, 0x2fa030, 0x2fe450, 0x308b10, 0x308c00, 0x3091c0, 0x3092e0,
    0x3096a0, 0x309a80, 0x30aea0, 0x30b150, 0x30b280, 0x30b3a0, 0x30b480, 0x30b560, 0x30b7c0,
    0x30b9b0, 0x30ba90, 0x30bc50, 0x30be60, 0x30c210, 0x30c3c0, 0x30c440, 0x30cb30, 0x30cb90,
    0x30ccd0, 0x30cf60, 0x30d120, 0x30d300, 0x30d410, 0x30d7b0, 0x30dd40, 0x310100, 0x310310,
    0x3108d0, 0x310a20, 0x312270, 0x314420, 0x314f80, 0x314fa0, 0x315d20, 0x315d60, 0x315db0,
    0x315e10, 0x316110, 0x316b50, 0x3179d0, 0x317d70, 0x318350, 0x3188c0, 0x318be0, 0x318ea0,
    0x3190c0, 0x3191f0, 0x3192e0, 0x319300, 0x319bd0, 0x319da0, 0x319e60, 0x31ab80, 0x31ad90,
    0x31b6b0, 0x31b7d0, 0x31b8b0, 0x31c7c0, 0x31cb90, 0x31d140, 0x31d160, 0x31d180, 0x31d3e0,
    0x31d580, 0x31dc10, 0x31eab0, 0x31ec90, 0x31ee30, 0x31fdc0, 0x320250, 0x320580, 0x321f20,
    0x3223a0, 0x322bb0, 0x3236c0, 0x323c30, 0x3244e0, 0x3246a0, 0x324c00, 0x3266b0, 0x3267c0,
    0x3268c0, 0x3276f0, 0x327a70, 0x327f90, 0x328d20, 0x329de0, 0x32ce00, 0x32d9b0, 0x32dba0,
    0x331c70, 0x334730, 0x3368b0, 0x3368e0, 0x336910, 0x336950, 0x336d30, 0x336d60, 0x336d90,
    0x336dc0, 0x336e60, 0x336f60, 0x336fb0, 0x337ed0,
    // weapons_lane2.rs (T2148): the gameplay/weapons roots with a T1925 Tier 1 translation
    0x12f30, 0x12f50, 0x12f70, 0x6bec0, 0x9e750, 0xae790, 0xae830, 0xb3e40, 0xb4470, 0xb4680,
    0xb4b60, 0xb4df0, 0xb4f90, 0xb6090, 0xb6140, 0xb6430, 0xb74c0, 0xb9270, 0xb9340, 0xb93e0,
    0xb9430, 0xb9510, 0xb9720, 0xb9790, 0xb9920, 0xb9960, 0xb99d0, 0xb9a30, 0xb9a90, 0xb9dc0,
    0xba310, 0xba8a0, 0xbb090, 0xbb6c0, 0xbc140, 0xbc1b0, 0xbc210, 0xbc310, 0xbc4d0, 0xbc520,
    0xbcb00, 0xbcd80, 0xbd190, 0xbd650, 0xbdbd0, 0xbe180, 0xbe990, 0xbee60, 0xbef00, 0xbf080,
    0xbf360, 0xbf520, 0xbf620, 0xbf740, 0xbfa30, 0xbfdf0, 0xc0450, 0xc0540, 0xc1360, 0xc1950,
    0xc2090, 0xc2d40, 0xdc270, 0xe0ce0, 0xe28b0, 0xe9690, 0xea9c0, 0xeaaf0, 0xf7070, 0xf88c0,
    0x10e9a0, 0x142880, 0x142900, 0x142990, 0x15db20, 0x17bb10, 0x183040, 0x18b530, 0x1b56a0,
    0x1b5c70, 0x1b6350, 0x1b7e80, 0x1b8160, 0x1b81b0, 0x1b8c70, 0x1b90a0, 0x1b9f90, 0x1ba020,
    0x1ba300, 0x1bb030, 0x1bb2f0, 0x1bb480, 0x1bc990, 0x1bd020, 0x1bd390, 0x1bd440, 0x1bd4f0,
    0x1bd600, 0x1bdd20, 0x1bde40, 0x1bf000, 0x1bf140, 0x1bf320, 0x1c0110, 0x1c1c00, 0x1c2c00,
    0x1c5e10, 0x216190, 0x34ff30,
    // text_lane.rs (T2157): the data/text roots with a T1878 Tier 1 translation
    0x3d7b0, 0x3fb90, 0x461c0, 0x150f60, 0x27250, 0x151160, 0x40650, 0x63c80, 0x63e70, 0x3dd20,
    0x3db80, 0x3de30, 0x46170, 0x150fe0, 0x31c50, 0x32860, 0x37330, 0x38200, 0x3fc10, 0x406a0,
    0x66470, 0x7c300, 0x7c390, 0x82860, 0x828c0, 0x828f0, 0x82920, 0x195810, 0x1b2b80, 0x2be6f0,
    0x2c7750, 0x2c7780, 0x2d1370, 0x2d14c0, 0x2d1770, 0x311820,
    // input_lane2.rs (T1995): the remaining engine/input roots with a Tier 1 translation
    0x190d50, 0x43220, 0x42720, 0x42bf0, 0x2586c0, 0x2588b0, 0x2fb00, 0x31fc0, 0x373f0, 0x42a70,
    0x42ae0, 0x196460, 0x196570, 0x2e7df0, 0x32b400, 0x30210, 0x6f150, 0xc95e0, 0x1b7bc0, 0x1c0200,
    0x159300, 0x158f80, 0x57550, 0xa6880, 0xa67c0, 0x5bc20, 0x1719e0, 0xa7090, 0xdc7b0, 0x1b7260,
    0x9ef60, 0x15c700, 0x15bd30, 0xe9380, 0x9bae0, 0xe91f0, 0x1596f0, 0x9ee00, 0x69d40, 0x165a40,
    0x266250, 0x1cb8a0, 0x1642c0, 0xdac30, 0xa6ec0, 0x58a40, 0x172180, 0x17730, 0x83910, 0x85920,
    0x86240, 0x87880, 0x89700, 0x162060, 0x162080, 0x215790, 0x216230, 0x218910, 0x227430,
    0x227650, 0x227710, 0x227750, 0x227920, 0x228180, 0x228870, 0x234110, 0x234190, 0x235630,
    0x237480, 0x237770, 0x237920, 0x1571c0, 0x83790, 0x157440, 0x873c0, 0x96c20, 0x88430, 0x87590,
    0x88c60, 0x87720, 0xc77c0, 0x87bb0, 0x86e10, 0xc84a0, 0xccab0, 0xc9de0, 0x3f050, 0x8c690,
    0x1cebc0, 0x8c8e0, 0xc79c0, 0x87f80, 0x87630, 0xcb3b0, 0x87a70, 0x89170, 0x94620, 0x888a0,
    0x8cc20, 0x157320, 0x8eac0, 0x89af0, 0x88ea0, 0x83980, 0x878a0, 0xf8a30, 0x12c690, 0xc9c20,
    0x1b90f0, 0x2e4c80, 0x13a110, 0xf89e0, 0x810c0, 0x63b40, 0x637e0, 0x63770, 0x18b90, 0x12910,
    0x18380, 0x8faa0, 0x903d0, 0x8bdd0, 0x8dd10, 0x96860, 0x96220, 0x8c910, 0x18fb10, 0x8df60,
    0x96410, 0x8dc70, 0x8e420, 0x157490, 0x62fb0, 0x157600, 0x1574d0, 0x18ce0, 0x89280, 0x8e590,
    0x8e4b0, 0x59cd0, 0x1575b0, 0x12c6e0, 0x963f0, 0x96330, 0x8ff30, 0x16b990, 0xcc720, 0xdca70,
    0x9f490, 0x812f0, 0x157970, 0x8c8c0, 0x81420, 0x63900, 0x63390, 0x2a370, 0x9e610, 0x3f420,
    0x8e3d0, 0x86130, 0x240580, 0x12c370, 0x86fc0, 0x835b0, 0x18fdc0, 0xcb450, 0x812c0, 0x36c9d0,
    0x2be260, 0x148310, 0xcb2e0, 0x85cc0, 0x17600, 0x23ef10, 0xcede0, 0xcbd10, 0xcb3e0, 0x63730,
    0x7c1b0, 0x7c410, 0x7c4d0, 0x7db20, 0x86c40, 0x15b1b0, 0x2784d0, 0x10e1d0, 0xa2c00, 0x21a710,
    0xd6f60, 0x111df0, 0x2d2c70, 0x1bf9e0, 0x15dda0, 0x151fc0, 0x59320, 0x152090, 0x150960,
    0x582a0, 0x583e0, 0x58340, 0x1506b0, 0x1530e0, 0x591d0, 0x59210, 0x152540, 0x1526c0, 0x22520,
    0x1c6f0, 0x8e630, 0x12c1b0, 0x1512b0, 0x153650, 0x584a0, 0x153600, 0x153550, 0x1536a0,
    0x1515e0, 0x58df0, 0x152770, 0x5a5b0, 0x1536e0, 0x153490, 0x153af0, 0x151cf0, 0x1534e0,
    0x1c5d0, 0x151640, 0x1519e0, 0x156f30, 0x151790, 0x152260, 0x153b00, 0x153c00, 0x56fc0,
    0x151c90, 0x1533e0, 0x60880, 0x1570b0, 0x150d70, 0x150580, 0x12430, 0x94660, 0x153ac0, 0x47500,
    0x153da0, 0x111310, 0x12010, 0x1b73f0, 0x59110, 0x152b50, 0x152a10, 0x154030, 0x5a0e0, 0x565d0,
    0x151da0, 0x150830, 0x154300, 0x111370, 0x150cc0, 0x12570, 0x1521d0, 0x123d0, 0x124f0, 0x126c0,
    0x12660, 0x150920, 0x1510f0, 0x1528a0, 0xc9f80, 0x8fe30, 0x59bf0, 0x59c00, 0x59cb0, 0x976e0,
    0x5b980, 0x5c2b0, 0x5c420, 0xb9be0, 0x1cb240, 0x1c8520, 0x1c97f0, 0x12d80, 0x1c8540, 0x1ca2f0,
    0x153c20, 0x965f0, 0xb9bc0, 0x153200, 0x1c9510, 0x9deb0, 0x29e40, 0x29e60, 0x164bf0, 0x165390,
    0x27fa0, 0x234de0, 0x80f20, 0x2e5900, 0x315e50, 0x2d8100, 0x81ec0, 0x56120, 0x560e0, 0x42fd0,
    0x1ccec0, 0x1cef70, 0x1ce9a0, 0xfe000, 0x14d480, 0x81ce0, 0x2e4b80, 0xf9250, 0x14af00, 0x3fcf0,
    0x3f8d0, 0x74760, 0x40740, 0x722b0, 0x40fa0, 0x77470, 0x774b0, 0x40f00, 0x76400, 0x757e0,
    0x335500, 0x3356a0, 0x3356f0, 0x335790, 0x335840, 0x2fb4c0, 0x30ad30, 0x30acb0, 0x3111f0,
    0x306460, 0x65940, 0x2d3bf0, 0xb0770, 0x160f50, 0x162520, 0x2273c0, 0x25f370, 0x227e90,
    0x2274c0, 0x3c8a84, 0x3c9800, 0x298b0, 0x220e40,
];

/// T2143: the module that handled each root last time. The old code walked a cascade of 30 `run` functions (each a
/// `match va`) until one accepted, so a root late in the list paid for every earlier miss on every call. A root's handler
/// does not change, so the first success is remembered per root and tried first (a miss still falls back to the cascade).
static HANDLER: [core::sync::atomic::AtomicU8; crate::runtime::TOTAL] =
    [const { core::sync::atomic::AtomicU8::new(0) }; crate::runtime::TOTAL];
const MODULES: u8 = 51;

fn run_module(
    id: u8,
    va: u32,
    m: &mut MappedGuest,
    guest: &mut LaneGuest,
    regs: &mut ExtRegisters,
) -> bool {
    match id {
        0 => crate::leaf_lane::run(va, m, guest, regs),
        1 => crate::pose::run(va, m, guest, regs),
        2 => crate::pose_anim::run(va, m, guest, regs),
        3 => crate::pose_leaf::run(va, m, guest, regs),
        4 => crate::inventory::run(va, m, guest, regs),
        5 => crate::inventory_select::run(va, m, guest, regs),
        6 => crate::inventory_grant::run(va, m, guest, regs),
        7 => crate::inventory_ammo::run(va, m, guest, regs),
        8 => crate::inventory_slots::run(va, m, guest, regs),
        9 => crate::inventory_pickup::run(va, m, guest, regs),
        10 => crate::anim_lane::run(va, m, guest, regs),
        11 => crate::anim_lane2::run(va, m, guest, regs),
        12 => crate::anim_lane3::run(va, m, guest, regs),
        13 => crate::mp_lane::run(va, m, guest, regs),
        14 => crate::modes_lane::run(va, m, guest, regs),
        15 => crate::save_lane::run(va, m, guest, regs),
        16 => crate::mapmaker_lane::run(va, m, guest, regs),
        17 => crate::npc_lane::run(va, m, guest, regs),
        18 => crate::modes_lane2::run(va, m, guest, regs),
        19 => crate::hud_lane::run(va, m, guest, regs),
        20 => crate::state_lane::run(va, m, guest, regs),
        21 => crate::audio_lane::run(va, m, guest, regs),
        22 => crate::input_lane::run(va, m, guest, regs),
        23 => crate::render_lane::run(va, m, guest, regs),
        24 => crate::render_lane2::run(va, m, guest, regs),
        25 => crate::object_lane::run(va, m, guest, regs),
        26 => crate::char_lane::run(va, m, guest, regs),
        27 => crate::physics_lane::run(va, m, guest, regs),
        28 => crate::physics_lane2::run(va, m, guest, regs),
        29 => crate::cutscene_lane::run(va, m, guest, regs),
        30 => crate::mapmaker_ui_lane::run(va, m, guest, regs),
        31 => crate::weapons_lane2::run(va, m, guest, regs),
        32 => crate::text_lane::run(va, m, guest, regs),
        33 => crate::input_lane2::run(va, m, guest, regs),
        34 => crate::objchar_tail_writer::run(va, m, guest, regs),
        35 => crate::missions_lane::run(va, m, guest, regs),
        36 => crate::scene_lane::run(va, m, guest, regs),
        37 => crate::text_field_lane::run(va, m, guest, regs),
        38 => crate::cutscene_presentation_lane::run(va, m, guest, regs),
        39 => crate::mapmaker_hud_panels::run(va, m, guest, regs),
        40 => crate::mapmaker_overlay::run(va, m, guest, regs),
        41 => crate::story_sequencer_lane::run(va, m, guest, regs),
        42 => crate::rea_object_runtime::run(va, m, guest, regs),
        43 => crate::story_event_link_lane::run(va, m, guest, regs),
        44 => crate::story_eligible_count_lane::run(va, m, guest, regs),
        45 => crate::story_objective_admission_lane::run(va, m, guest, regs),
        46 => crate::story_objective_lifecycle::run(va, m, guest, regs),
        47 => crate::memory_atexit::run(va, m, guest, regs),
        48 => crate::memory_alloc_wrapper_lane::run(va, m, guest, regs),
        49 => crate::missions_writer::run(va, m, guest, regs),
        50 => tsfp_lane_c2r::run(va, m, guest, regs),
        _ => false,
    }
}

fn run(va: u32, m: &mut MappedGuest, guest: &mut LaneGuest, regs: &mut ExtRegisters) -> bool {
    // T2161: lane roots entered from lifted code get the same raw look hooks as the nested calls (inert without a Mouse+Keyboard slot).
    if crate::raw_look::watches(va) {
        return crate::raw_look::around(va, true, m, guest, regs, |m, guest, regs| {
            run_modules(va, m, guest, regs)
        });
    }
    run_modules(va, m, guest, regs)
}

fn run_modules(
    va: u32,
    m: &mut MappedGuest,
    guest: &mut LaneGuest,
    regs: &mut ExtRegisters,
) -> bool {
    // T2376: selected hand-state roots use the same typed owner as nested calls. This must run
    // before the inventory/player batches claim their translated reference entrypoints.
    if crate::weapon_hands_runtime::dispatch_nested(va, m, guest, regs) {
        return true;
    }
    // T2385: selectively dispatch admitted REA leaves to their shared typed
    // owners when the default-off runtime adoption switch is enabled.
    if crate::rea_ai_runtime::run(va, m, guest, regs) {
        return true;
    }
    // T2377: let the readable, typed REA owner win for the bounded interaction and inventory
    // lookup roots when explicitly enabled. The environment switch defaults to the normal lane.
    if crate::rea_inventory_runtime::run(va, m, guest, regs) {
        return true;
    }
    use core::sync::atomic::Ordering::Relaxed;
    let slot = crate::runtime::ROOT_MAP.find(va);
    if let Some(slot) = slot {
        let cached = HANDLER[slot].load(Relaxed);
        if cached != 0 && run_module(cached - 1, va, m, guest, regs) {
            return true;
        }
    }
    for id in 0..MODULES {
        if run_module(id, va, m, guest, regs) {
            if let Some(slot) = slot {
                HANDLER[slot].store(id + 1, Relaxed);
            }
            return true;
        }
    }
    false
}

pub fn execute(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) -> bool {
    let mut regs = ExtRegisters {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
        esp: c.esp,
        ebx: c.ebx,
        ebp: c.ebp,
        esi: c.esi,
        edi: c.edi,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm { lanes: [0; 4] }; 5],
        xmm_hi: [Xmm { lanes: [0; 4] }; 3],
    };
    unsafe { tsfp_lane_x_fp_load(&mut regs) };
    let mut guest = LaneGuest {
        memory_offset: m.offset(),
    };
    crate::raw_look::note_offset(m.offset());
    note_first_argument(m, c.esp);
    if !run(va, m, &mut guest, &mut regs) {
        // No handler accepted the root. Leave the caller registers and FP globals intact;
        // the host selector runs the retained original when we return false.
        warn_unregistered(va);
        return false;
    }
    let _ = tsfp_port_engine_render_effects_live_return::repair_return(va, m, &mut regs);
    unsafe { tsfp_lane_x_fp_store(&regs) };
    c.eax = regs.eax;
    c.ecx = regs.ecx;
    c.edx = regs.edx;
    c.ebx = regs.ebx;
    c.esi = regs.esi;
    c.edi = regs.edi;
    c.ebp = regs.ebp;
    c.esp = regs.esp;
    mutate(va, m, c);
    true
}

fn warn_unregistered(va: u32) {
    use core::sync::atomic::{AtomicBool, Ordering};
    const UNKNOWN: usize = crate::runtime::ROOTS.len();
    static WARNED: [AtomicBool; UNKNOWN + 1] = [const { AtomicBool::new(false) }; UNKNOWN + 1];
    let slot = crate::runtime::ROOT_MAP.find(va).unwrap_or(UNKNOWN);
    if !WARNED.get(slot).unwrap().swap(true, Ordering::Relaxed) {
        unsafe extern "C" {
            fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
        }
        let mut line =
            *b"unregistered T1874 lane port at VA 0x00000000; falling back to lifted original\n";
        let start = b"unregistered T1874 lane port at VA 0x".len();
        for digit in 0..8 {
            line[start + digit] = b"0123456789abcdef"[((va >> (28 - 4 * digit)) & 15) as usize];
        }
        unsafe { write(2, line.as_ptr(), line.len()) };
    }
}

/// Ring of the latest guest byte writes, for the memory mutant (T1923; single guest thread).
#[cfg(rust_lane_mutation)]
static mut WRITES: [u32; 64] = [0; 64];
#[cfg(rust_lane_mutation)]
static mut WRITE_COUNT: usize = 0;
#[cfg(rust_lane_mutation)]
pub fn note_write(address: u32) {
    unsafe {
        let index = WRITE_COUNT % 64;
        WRITES[index] = address;
        WRITE_COUNT += 1;
    }
}

/// Output-bit mutant for the control matrix (EAX ^ 1), same as the T1854 lane. T1923: with
/// `TSFP_LANE_MUTATE_MEM` set it instead flips bit 0 of the newest non-stack byte the root wrote and
/// logs the address (`tsfp_lane_mutation_log`), so the sampled regions can be sized to what the root writes.
#[cfg(rust_lane_mutation)]
pub(crate) fn mutate(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) {
    // A non-zero target (TSFP_LANE_MUTATE_VA) restricts the mutation to that one root.
    unsafe extern "C" {
        fn tsfp_lane_mutation_target() -> u32;
        fn tsfp_lane_mutation_mem() -> u32;
        fn tsfp_lane_mutation_log(va: u32, address: u32);
        fn tsfp_lane_mutation_every() -> u32;
        fn tsfp_lane_mutation_bit() -> u32;
    }
    let target = unsafe { tsfp_lane_mutation_target() };
    let memory_level = unsafe { tsfp_lane_mutation_mem() };
    let memory_mode = memory_level != 0;
    if target != 0 && target != va {
        unsafe { WRITE_COUNT = 0 };
        return;
    }
    // T2256: TSFP_LANE_MUTATE_EVERY=<n> mutates only every n-th call of the target (a sparse, non-crashing perturbation).
    static mut CALLS: u32 = 0;
    let every = unsafe { tsfp_lane_mutation_every() };
    let call = unsafe {
        CALLS = CALLS.wrapping_add(1);
        CALLS
    };
    if every > 1 && call % every != 0 {
        unsafe { WRITE_COUNT = 0 };
        return;
    }
    if !memory_mode {
        unsafe { WRITE_COUNT = 0 };
        c.eax ^= 1;
        return;
    }
    // T2264 level 5: flip a byte of the record the first argument points to (offset TSFP_LANE_MUTATE_OFF, default 3, the
    // top byte of its first float). For roots whose outputs are written by a retained original callee through that
    // pointer (0x9E840 transforms its first argument in place via 0x151E80), which the Rust write ring never sees.
    // T2271 level 6: the same for the record argument TSFP_LANE_MUTATE_ARG (1-based, default 3) points to. 0x9E840 is
    // (matrix, half_extents, center): level 5 flipped the input matrix, levels 6 with ARG=2 or 3 flip the real outputs.
    if memory_level == 5 || memory_level == 6 {
        unsafe extern "C" {
            fn tsfp_lane_mutation_offset() -> u32;
            fn tsfp_lane_mutation_arg() -> u32;
        }
        use crate::memory::GuestMemory as _;
        let index = if memory_level == 5 {
            0
        } else {
            (unsafe { tsfp_lane_mutation_arg() }.clamp(1, 4) - 1) as usize
        };
        let address =
            unsafe { ARGUMENTS[index] }.wrapping_add(unsafe { tsfp_lane_mutation_offset() });
        let value = m.read8(address);
        m.write8(
            address,
            value ^ (1u8 << (unsafe { tsfp_lane_mutation_bit() } & 7)),
        );
        unsafe {
            tsfp_lane_mutation_log(va, address);
            WRITE_COUNT = 0
        };
        return;
    }
    let count = unsafe { WRITE_COUNT };
    for back in 1..=count.min(64) {
        let address = unsafe { WRITES[(count - back) % 64] };
        let near_stack =
            address >= c.esp.wrapping_sub(0x8000) && address <= c.esp.wrapping_add(0x8000);
        // T2117 level 2: the caller's frame (output buffers and argument slots above the return slot) is a legitimate
        // output of the root, only the callee's own locals below the entry ESP are dead.
        let caller_frame =
            address >= c.esp.wrapping_add(4) && address <= c.esp.wrapping_add(0x8000);
        // T2256 level 3: any write, stack-adjacent included (object roots whose outputs sit within 0x8000 of ESP).
        // T2264 level 4: like level 2 but skips the first eight argument slots (MSVC reuses a float argument slot as a
        // local, so the newest caller-frame write of 0x9E840 is its own dead argument, MEASURED esp+7 after return).
        let past_arguments =
            address >= c.esp.wrapping_add(0x20) && address <= c.esp.wrapping_add(0x8000);
        let outside_frame =
            address < c.esp.wrapping_sub(0x8000) || address > c.esp.wrapping_add(0x8000);
        if memory_level == 4 && !(past_arguments || outside_frame) {
            continue;
        }
        if (memory_level == 2 && !caller_frame) || (memory_level == 1 && near_stack) {
            continue;
        }
        use crate::memory::GuestMemory as _;
        let value = m.read8(address);
        // T2264: TSFP_LANE_MUTATE_BIT picks the flipped bit (default 0).
        m.write8(
            address,
            value ^ (1u8 << (unsafe { tsfp_lane_mutation_bit() } & 7)),
        );
        unsafe { tsfp_lane_mutation_log(va, address) };
        break;
    }
    unsafe { WRITE_COUNT = 0 };
}
/// T2264/T2271: the first four stack arguments at entry (the root may reuse their slots as scratch before it returns).
#[cfg(rust_lane_mutation)]
static mut ARGUMENTS: [u32; 4] = [0; 4];
#[cfg(rust_lane_mutation)]
fn note_first_argument(m: &mut MappedGuest, esp: u32) {
    use crate::memory::GuestMemory as _;
    for index in 0..4 {
        unsafe { ARGUMENTS[index] = m.read32(esp.wrapping_add(4 + 4 * index as u32)) };
    }
}
#[cfg(not(rust_lane_mutation))]
fn note_first_argument(_: &mut MappedGuest, _: u32) {}
#[cfg(not(rust_lane_mutation))]
pub(crate) fn mutate(_: u32, _: &mut MappedGuest, _: &mut LaneCpu) {}

/// T3765: the `tsfp_lane_<va>` exports of tsfp-lane-c2r re-enter the executor here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_lane_c2r_execute(va: u32, cpu: *mut LaneCpu, offset: usize) {
    execute(va, &mut unsafe { MappedGuest::new(offset) }, unsafe {
        &mut *cpu
    });
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_000426b0, 0x426b0);
export!(tsfp_lane_000426d0, 0x426d0);
export!(tsfp_lane_000426e0, 0x426e0);
export!(tsfp_lane_00042710, 0x42710);
export!(tsfp_lane_00042760, 0x42760);
export!(tsfp_lane_00042830, 0x42830);
export!(tsfp_lane_00042870, 0x42870);
export!(tsfp_lane_000428d0, 0x428d0);
export!(tsfp_lane_00042920, 0x42920);
export!(tsfp_lane_00042a30, 0x42a30);
export!(tsfp_lane_00042c50, 0x42c50);
export!(tsfp_lane_00042ca0, 0x42ca0);
export!(tsfp_lane_00042ce0, 0x42ce0);
export!(tsfp_lane_00042d00, 0x42d00);
export!(tsfp_lane_000585b0, 0x585b0);
export!(tsfp_lane_00059cf0, 0x59cf0);
export!(tsfp_lane_00059d90, 0x59d90);
export!(tsfp_lane_00059dd0, 0x59dd0);
export!(tsfp_lane_00059e00, 0x59e00);
export!(tsfp_lane_00059f10, 0x59f10);
export!(tsfp_lane_00059f50, 0x59f50);
export!(tsfp_lane_0005a1e0, 0x5a1e0);
export!(tsfp_lane_0005ba00, 0x5ba00);
export!(tsfp_lane_000606e0, 0x606e0);
export!(tsfp_lane_00060740, 0x60740);
export!(tsfp_lane_00061430, 0x61430);
export!(tsfp_lane_00061610, 0x61610);
export!(tsfp_lane_00061630, 0x61630);
export!(tsfp_lane_00064650, 0x64650);
export!(tsfp_lane_000646a0, 0x646a0);
export!(tsfp_lane_00065340, 0x65340);
export!(tsfp_lane_000660c0, 0x660c0);
export!(tsfp_lane_00066100, 0x66100);
export!(tsfp_lane_00066220, 0x66220);
export!(tsfp_lane_00066280, 0x66280);
export!(tsfp_lane_0008ea20, 0x8ea20);
export!(tsfp_lane_000949c0, 0x949c0);
export!(tsfp_lane_000b2d20, 0xb2d20);
export!(tsfp_lane_000be090, 0xbe090);
export!(tsfp_lane_000bfb40, 0xbfb40);
export!(tsfp_lane_000bff70, 0xbff70);
export!(tsfp_lane_000c0310, 0xc0310);
export!(tsfp_lane_000c4560, 0xc4560);
export!(tsfp_lane_000c6140, 0xc6140);
export!(tsfp_lane_000c63f0, 0xc63f0);
export!(tsfp_lane_000c7950, 0xc7950);
export!(tsfp_lane_000c9bb0, 0xc9bb0);
export!(tsfp_lane_000c9be0, 0xc9be0);
export!(tsfp_lane_000c9c00, 0xc9c00);
export!(tsfp_lane_000c9cc0, 0xc9cc0);
export!(tsfp_lane_000c9f50, 0xc9f50);
export!(tsfp_lane_000cef30, 0xcef30);
export!(tsfp_lane_000cfec0, 0xcfec0);
export!(tsfp_lane_000d0ca0, 0xd0ca0);
export!(tsfp_lane_000d1e20, 0xd1e20);
export!(tsfp_lane_000d2d20, 0xd2d20);
export!(tsfp_lane_000d6920, 0xd6920);
export!(tsfp_lane_000d7bc0, 0xd7bc0);
export!(tsfp_lane_000f8ad0, 0xf8ad0);
export!(tsfp_lane_000f8ae0, 0xf8ae0);
export!(tsfp_lane_00115f10, 0x115f10);
export!(tsfp_lane_00126260, 0x126260);
export!(tsfp_lane_00147080, 0x147080);
export!(tsfp_lane_00190e80, 0x190e80);
export!(tsfp_lane_001b66e0, 0x1b66e0);
export!(tsfp_lane_001b67e0, 0x1b67e0);
export!(tsfp_lane_001b6890, 0x1b6890);
export!(tsfp_lane_001b68f0, 0x1b68f0);
export!(tsfp_lane_001b6950, 0x1b6950);
export!(tsfp_lane_001b6b40, 0x1b6b40);
export!(tsfp_lane_001b6bb0, 0x1b6bb0);
export!(tsfp_lane_001b6bf0, 0x1b6bf0);
export!(tsfp_lane_001b6c90, 0x1b6c90);
export!(tsfp_lane_001b6d10, 0x1b6d10);
export!(tsfp_lane_001b6e20, 0x1b6e20);
export!(tsfp_lane_001b6ec0, 0x1b6ec0);
export!(tsfp_lane_001b9760, 0x1b9760);
export!(tsfp_lane_001b99b0, 0x1b99b0);
export!(tsfp_lane_001b9ab0, 0x1b9ab0);
export!(tsfp_lane_001b9ad0, 0x1b9ad0);
export!(tsfp_lane_001b9cf0, 0x1b9cf0);
export!(tsfp_lane_001b9d30, 0x1b9d30);
export!(tsfp_lane_001ba130, 0x1ba130);
export!(tsfp_lane_001ba1b0, 0x1ba1b0);
export!(tsfp_lane_001ba220, 0x1ba220);
export!(tsfp_lane_001ba2b0, 0x1ba2b0);
export!(tsfp_lane_001ba410, 0x1ba410);
export!(tsfp_lane_001ba7b0, 0x1ba7b0);
export!(tsfp_lane_001ba8f0, 0x1ba8f0);
export!(tsfp_lane_001ba9b0, 0x1ba9b0);
export!(tsfp_lane_001baa70, 0x1baa70);
export!(tsfp_lane_001baaf0, 0x1baaf0);
export!(tsfp_lane_001baba0, 0x1baba0);
export!(tsfp_lane_001bac50, 0x1bac50);
export!(tsfp_lane_001baca0, 0x1baca0);
export!(tsfp_lane_001badd0, 0x1badd0);
export!(tsfp_lane_001bae40, 0x1bae40);
export!(tsfp_lane_001baeb0, 0x1baeb0);
export!(tsfp_lane_001bfa60, 0x1bfa60);
export!(tsfp_lane_001bfaa0, 0x1bfaa0);
export!(tsfp_lane_001bfc00, 0x1bfc00);
export!(tsfp_lane_001bfca0, 0x1bfca0);
export!(tsfp_lane_001bfe60, 0x1bfe60);
export!(tsfp_lane_001bfec0, 0x1bfec0);
export!(tsfp_lane_001c0080, 0x1c0080);
export!(tsfp_lane_001c1a90, 0x1c1a90);
export!(tsfp_lane_001c1d90, 0x1c1d90);
export!(tsfp_lane_0022dd90, 0x22dd90);
export!(tsfp_lane_00238460, 0x238460);
export!(tsfp_lane_00238510, 0x238510);
export!(tsfp_lane_002385b0, 0x2385b0);
export!(tsfp_lane_00238610, 0x238610);
export!(tsfp_lane_00246dc0, 0x246dc0);
export!(tsfp_lane_002474e0, 0x2474e0);
export!(tsfp_lane_00258870, 0x258870);
export!(tsfp_lane_00259800, 0x259800);
export!(tsfp_lane_00259df0, 0x259df0);
export!(tsfp_lane_0025a080, 0x25a080);
export!(tsfp_lane_0025b720, 0x25b720);
export!(tsfp_lane_0025ead0, 0x25ead0);
export!(tsfp_lane_0025ec70, 0x25ec70);
export!(tsfp_lane_0025ef30, 0x25ef30);
export!(tsfp_lane_002cbce0, 0x2cbce0);
export!(tsfp_lane_002ce120, 0x2ce120);
export!(tsfp_lane_002e86a0, 0x2e86a0);
export!(tsfp_lane_002e8c00, 0x2e8c00);
export!(tsfp_lane_003381d0, 0x3381d0);
export!(tsfp_lane_00357160, 0x357160);
export!(tsfp_lane_00357320, 0x357320);
export!(tsfp_lane_00059f90, 0x59f90);
export!(tsfp_lane_00061570, 0x61570);
export!(tsfp_lane_0005a070, 0x5a070);
export!(tsfp_lane_0005a2b0, 0x5a2b0);
export!(tsfp_lane_00059d50, 0x59d50);
export!(tsfp_lane_00059ec0, 0x59ec0);
export!(tsfp_lane_0005a260, 0x5a260);
export!(tsfp_lane_0005a2f0, 0x5a2f0);
export!(tsfp_lane_000614b0, 0x614b0);
export!(tsfp_lane_00061450, 0x61450);
export!(tsfp_lane_0005adc0, 0x5adc0);
export!(tsfp_lane_0005a360, 0x5a360);
export!(tsfp_lane_00060670, 0x60670);
export!(tsfp_lane_00060920, 0x60920);
export!(tsfp_lane_000585e0, 0x585e0);
export!(tsfp_lane_00059a40, 0x59a40);
export!(tsfp_lane_00059b00, 0x59b00);
export!(tsfp_lane_00060540, 0x60540);
export!(tsfp_lane_000605d0, 0x605d0);
export!(tsfp_lane_000604d0, 0x604d0);
export!(tsfp_lane_00060b30, 0x60b30);
export!(tsfp_lane_00060f80, 0x60f80);
export!(tsfp_lane_0005afb0, 0x5afb0);
export!(tsfp_lane_00058690, 0x58690);
export!(tsfp_lane_0005a220, 0x5a220);
export!(tsfp_lane_000c0c50, 0xc0c50);
export!(tsfp_lane_000cb240, 0xcb240);
export!(tsfp_lane_0013dcf0, 0x13dcf0);
export!(tsfp_lane_00080f40, 0x80f40);
export!(tsfp_lane_00158c80, 0x158c80);
export!(tsfp_lane_001cd1e0, 0x1cd1e0);
export!(tsfp_lane_00069e70, 0x69e70);
export!(tsfp_lane_00068610, 0x68610);
export!(tsfp_lane_00067280, 0x67280);
export!(tsfp_lane_000431f0, 0x431f0);
export!(tsfp_lane_002b0af0, 0x2b0af0);
export!(tsfp_lane_00233ef0, 0x233ef0);
export!(tsfp_lane_00069630, 0x69630);
export!(tsfp_lane_00069640, 0x69640);
export!(tsfp_lane_000696a0, 0x696a0);
export!(tsfp_lane_000696d0, 0x696d0);
export!(tsfp_lane_00069720, 0x69720);
export!(tsfp_lane_000697d0, 0x697d0);
export!(tsfp_lane_00069900, 0x69900);
export!(tsfp_lane_00069930, 0x69930);
export!(tsfp_lane_001cd2b0, 0x1cd2b0);
export!(tsfp_lane_001cd330, 0x1cd330);
export!(tsfp_lane_001cd380, 0x1cd380);
export!(tsfp_lane_00312ad0, 0x312ad0);
export!(tsfp_lane_00068830, 0x68830);
export!(tsfp_lane_000681c0, 0x681c0);
export!(tsfp_lane_00265cc0, 0x265cc0);
export!(tsfp_lane_00283730, 0x283730);
export!(tsfp_lane_002621a0, 0x2621a0);
export!(tsfp_lane_00040260, 0x40260);
export!(tsfp_lane_0023f2e0, 0x23f2e0);
export!(tsfp_lane_002d7240, 0x2d7240);
export!(tsfp_lane_0012cb40, 0x12cb40);
export!(tsfp_lane_0012cbb0, 0x12cbb0);
export!(tsfp_lane_00336570, 0x336570);
export!(tsfp_lane_000fc9f0, 0xfc9f0);
export!(tsfp_lane_00042f90, 0x42f90);
export!(tsfp_lane_00028180, 0x28180);
export!(tsfp_lane_00029f30, 0x29f30);
export!(tsfp_lane_00029cc0, 0x29cc0);
export!(tsfp_lane_00045670, 0x45670);
export!(tsfp_lane_00029d10, 0x29d10);
export!(tsfp_lane_00028a60, 0x28a60);
export!(tsfp_lane_00029bb0, 0x29bb0);
export!(tsfp_lane_000457b0, 0x457b0);
export!(tsfp_lane_00026f00, 0x26f00);
export!(tsfp_lane_000462f0, 0x462f0);
export!(tsfp_lane_000459f0, 0x459f0);
export!(tsfp_lane_000458b0, 0x458b0);
export!(tsfp_lane_00028760, 0x28760);
export!(tsfp_lane_00028980, 0x28980);
export!(tsfp_lane_00045b60, 0x45b60);
export!(tsfp_lane_00029af0, 0x29af0);
export!(tsfp_lane_00046350, 0x46350);
export!(tsfp_lane_00045a50, 0x45a50);
export!(tsfp_lane_00045790, 0x45790);
export!(tsfp_lane_00026bb0, 0x26bb0);
export!(tsfp_lane_00046610, 0x46610);
export!(tsfp_lane_00028610, 0x28610);
export!(tsfp_lane_00044ac0, 0x44ac0);
export!(tsfp_lane_00164e00, 0x164e00);
export!(tsfp_lane_001651c0, 0x1651c0);
export!(tsfp_lane_00045540, 0x45540);
export!(tsfp_lane_00027bb0, 0x27bb0);
export!(tsfp_lane_00026d20, 0x26d20);
export!(tsfp_lane_00029dd0, 0x29dd0);
export!(tsfp_lane_00045610, 0x45610);
export!(tsfp_lane_00027c20, 0x27c20);
export!(tsfp_lane_0018ff70, 0x18ff70);
export!(tsfp_lane_00042970, 0x42970);
export!(tsfp_lane_001ccf80, 0x1ccf80);
export!(tsfp_lane_000438b0, 0x438b0);
export!(tsfp_lane_001cce70, 0x1cce70);
export!(tsfp_lane_00042b60, 0x42b60);
export!(tsfp_lane_000426a0, 0x426a0);
export!(tsfp_lane_00190520, 0x190520);
export!(tsfp_lane_002aefd0, 0x2aefd0);
export!(tsfp_lane_002a5c30, 0x2a5c30);
export!(tsfp_lane_00021440, 0x21440);
export!(tsfp_lane_00021450, 0x21450);
export!(tsfp_lane_000214c0, 0x214c0);
export!(tsfp_lane_00021740, 0x21740);
export!(tsfp_lane_00022560, 0x22560);
export!(tsfp_lane_00022790, 0x22790);
export!(tsfp_lane_0002a630, 0x2a630);
export!(tsfp_lane_00022950, 0x22950);
export!(tsfp_lane_000226e0, 0x226e0);
export!(tsfp_lane_00022720, 0x22720);
export!(tsfp_lane_000204b0, 0x204b0);
export!(tsfp_lane_00022ff0, 0x22ff0);
export!(tsfp_lane_0001a010, 0x1a010);
export!(tsfp_lane_00019640, 0x19640);
export!(tsfp_lane_00019d50, 0x19d50);
export!(tsfp_lane_00019d60, 0x19d60);
export!(tsfp_lane_00019d40, 0x19d40);
export!(tsfp_lane_00019650, 0x19650);
export!(tsfp_lane_00019d70, 0x19d70);
export!(tsfp_lane_0008f470, 0x8f470);
export!(tsfp_lane_0008f490, 0x8f490);
export!(tsfp_lane_0008f4e0, 0x8f4e0);
export!(tsfp_lane_0001e770, 0x1e770);
export!(tsfp_lane_0001e7d0, 0x1e7d0);
export!(tsfp_lane_0001e7f0, 0x1e7f0);
export!(tsfp_lane_0001e970, 0x1e970);
export!(tsfp_lane_0001eb00, 0x1eb00);
export!(tsfp_lane_00018de0, 0x18de0);
export!(tsfp_lane_00018ea0, 0x18ea0);
export!(tsfp_lane_0001b7b0, 0x1b7b0);
export!(tsfp_lane_0001d5f0, 0x1d5f0);
export!(tsfp_lane_0001ed50, 0x1ed50);
export!(tsfp_lane_0008e710, 0x8e710);
export!(tsfp_lane_0008f5e0, 0x8f5e0);
export!(tsfp_lane_000a2740, 0xa2740);
export!(tsfp_lane_0010c390, 0x10c390);
export!(tsfp_lane_0010c400, 0x10c400);
export!(tsfp_lane_0010c550, 0x10c550);
export!(tsfp_lane_00153bd0, 0x153bd0);
export!(tsfp_lane_0010d070, 0x10d070);
export!(tsfp_lane_00022be0, 0x22be0);
export!(tsfp_lane_0002a2d0, 0x2a2d0);
export!(tsfp_lane_0002a310, 0x2a310);
export!(tsfp_lane_0002a320, 0x2a320);
export!(tsfp_lane_0002a340, 0x2a340);
export!(tsfp_lane_0002a360, 0x2a360);
export!(tsfp_lane_0001d380, 0x1d380);
export!(tsfp_lane_000389b0, 0x389b0);
export!(tsfp_lane_000459d0, 0x459d0);
export!(tsfp_lane_0001c720, 0x1c720);
export!(tsfp_lane_00019cc0, 0x19cc0);
export!(tsfp_lane_0001ed60, 0x1ed60);
export!(tsfp_lane_0001a2e0, 0x1a2e0);
export!(tsfp_lane_0001afa0, 0x1afa0);
export!(tsfp_lane_0001b090, 0x1b090);
export!(tsfp_lane_0001efb0, 0x1efb0);
export!(tsfp_lane_0002a900, 0x2a900);
export!(tsfp_lane_0002abe0, 0x2abe0);
export!(tsfp_lane_0002ad60, 0x2ad60);
export!(tsfp_lane_0018e910, 0x18e910);
export!(tsfp_lane_000ffe50, 0xffe50);
export!(tsfp_lane_0010e410, 0x10e410);
export!(tsfp_lane_00019240, 0x19240);
export!(tsfp_lane_0008f6b0, 0x8f6b0);
export!(tsfp_lane_00014d60, 0x14d60);
export!(tsfp_lane_00020c40, 0x20c40);
export!(tsfp_lane_0001f790, 0x1f790);
export!(tsfp_lane_0001c620, 0x1c620);
export!(tsfp_lane_0001e6b0, 0x1e6b0);
export!(tsfp_lane_00021060, 0x21060);
export!(tsfp_lane_00022740, 0x22740);
export!(tsfp_lane_0001c600, 0x1c600);
export!(tsfp_lane_0008f950, 0x8f950);
export!(tsfp_lane_000a14b0, 0xa14b0);
export!(tsfp_lane_001518f0, 0x1518f0);
export!(tsfp_lane_0009e840, 0x9e840);
export!(tsfp_lane_000a1ab0, 0xa1ab0);
export!(tsfp_lane_0009e430, 0x9e430);
export!(tsfp_lane_000a0c50, 0xa0c50);
export!(tsfp_lane_000a7360, 0xa7360);
export!(tsfp_lane_000be860, 0xbe860);
export!(tsfp_lane_000a00b0, 0xa00b0);
export!(tsfp_lane_0009e330, 0x9e330);
export!(tsfp_lane_0009fba0, 0x9fba0);
export!(tsfp_lane_00074720, 0x74720);
export!(tsfp_lane_0009fd60, 0x9fd60);
export!(tsfp_lane_0009fbe0, 0x9fbe0);
export!(tsfp_lane_000a1b80, 0xa1b80);
export!(tsfp_lane_0010e210, 0x10e210);
export!(tsfp_lane_00162200, 0x162200);
export!(tsfp_lane_002776f0, 0x2776f0);
export!(tsfp_lane_000bdd50, 0xbdd50);
export!(tsfp_lane_001622c0, 0x1622c0);
export!(tsfp_lane_000a1fa0, 0xa1fa0);
export!(tsfp_lane_000a2430, 0xa2430);
export!(tsfp_lane_000862c0, 0x862c0);
export!(tsfp_lane_00048fe0, 0x48fe0);
export!(tsfp_lane_000a6530, 0xa6530);
export!(tsfp_lane_0009a330, 0x9a330);
export!(tsfp_lane_00153190, 0x153190);
export!(tsfp_lane_0009eb60, 0x9eb60);
export!(tsfp_lane_0023ff20, 0x23ff20);
export!(tsfp_lane_00080430, 0x80430);
export!(tsfp_lane_00065230, 0x65230);
export!(tsfp_lane_000653b0, 0x653b0);
export!(tsfp_lane_00065420, 0x65420);
export!(tsfp_lane_00065470, 0x65470);
export!(tsfp_lane_000658d0, 0x658d0);
export!(tsfp_lane_00065c70, 0x65c70);
export!(tsfp_lane_00065f40, 0x65f40);
export!(tsfp_lane_000667f0, 0x667f0);
export!(tsfp_lane_000668a0, 0x668a0);
export!(tsfp_lane_00066fc0, 0x66fc0);
export!(tsfp_lane_000670c0, 0x670c0);
export!(tsfp_lane_000674c0, 0x674c0);
export!(tsfp_lane_00067450, 0x67450);
export!(tsfp_lane_00067e60, 0x67e60);
export!(tsfp_lane_00067f00, 0x67f00);
export!(tsfp_lane_00068720, 0x68720);
export!(tsfp_lane_0015df20, 0x15df20);
export!(tsfp_lane_0015e4f0, 0x15e4f0);
export!(tsfp_lane_0022ce60, 0x22ce60);
export!(tsfp_lane_0023efe0, 0x23efe0);
export!(tsfp_lane_0025f830, 0x25f830);
export!(tsfp_lane_000699d0, 0x699d0);
export!(tsfp_lane_00046030, 0x46030);
export!(tsfp_lane_0006a850, 0x6a850);
export!(tsfp_lane_00069f30, 0x69f30);
export!(tsfp_lane_0023f390, 0x23f390);
export!(tsfp_lane_00042640, 0x42640);
export!(tsfp_lane_000450e0, 0x450e0);
export!(tsfp_lane_00045160, 0x45160);
export!(tsfp_lane_0009ce00, 0x9ce00);
export!(tsfp_lane_0009a140, 0x9a140);
export!(tsfp_lane_0009a260, 0x9a260);
export!(tsfp_lane_0009a630, 0x9a630);
export!(tsfp_lane_001c4400, 0x1c4400);
export!(tsfp_lane_001c3100, 0x1c3100);
export!(tsfp_lane_001bf410, 0x1bf410);
export!(tsfp_lane_001bc890, 0x1bc890);
export!(tsfp_lane_001c2e80, 0x1c2e80);
export!(tsfp_lane_001b7680, 0x1b7680);
export!(tsfp_lane_001bdfd0, 0x1bdfd0);
export!(tsfp_lane_001bd820, 0x1bd820);
export!(tsfp_lane_001bdaf0, 0x1bdaf0);
export!(tsfp_lane_00219ca0, 0x219ca0);
export!(tsfp_lane_000425e0, 0x425e0);
export!(tsfp_lane_00045970, 0x45970);
export!(tsfp_lane_001bc650, 0x1bc650);
export!(tsfp_lane_0009a7a0, 0x9a7a0);
export!(tsfp_lane_0009a5b0, 0x9a5b0);
export!(tsfp_lane_0009af90, 0x9af90);
export!(tsfp_lane_00043c50, 0x43c50);
export!(tsfp_lane_00088950, 0x88950);
export!(tsfp_lane_000cf510, 0xcf510);
export!(tsfp_lane_00151e80, 0x151e80);
export!(tsfp_lane_001baee0, 0x1baee0);
export!(tsfp_lane_00153cd0, 0x153cd0);
export!(tsfp_lane_001b9dc0, 0x1b9dc0);
export!(tsfp_lane_001b7c50, 0x1b7c50);
export!(tsfp_lane_0009e4b0, 0x9e4b0);
export!(tsfp_lane_000a1ea0, 0xa1ea0);
export!(tsfp_lane_000a0130, 0xa0130);
export!(tsfp_lane_000a5c00, 0xa5c00);
export!(tsfp_lane_000a5af0, 0xa5af0);
export!(tsfp_lane_0015eae0, 0x15eae0);
export!(tsfp_lane_001be870, 0x1be870);
export!(tsfp_lane_0027ee70, 0x27ee70);
export!(tsfp_lane_0015ab80, 0x15ab80);
export!(tsfp_lane_0015d500, 0x15d500);
export!(tsfp_lane_000a7670, 0xa7670);
export!(tsfp_lane_0009eab0, 0x9eab0);
export!(tsfp_lane_000a2b50, 0xa2b50);
export!(tsfp_lane_0009ff10, 0x9ff10);
export!(tsfp_lane_00167780, 0x167780);
export!(tsfp_lane_0015e830, 0x15e830);
export!(tsfp_lane_001260c0, 0x1260c0);
export!(tsfp_lane_001099e0, 0x1099e0);
export!(tsfp_lane_0015c6c0, 0x15c6c0);
export!(tsfp_lane_000d1cb0, 0xd1cb0);
export!(tsfp_lane_0005c5b0, 0x5c5b0);
export!(tsfp_lane_000e1200, 0xe1200);
export!(tsfp_lane_000a17c0, 0xa17c0);
export!(tsfp_lane_000a5f40, 0xa5f40);
export!(tsfp_lane_000a0860, 0xa0860);
export!(tsfp_lane_000a7870, 0xa7870);
export!(tsfp_lane_0015c9d0, 0x15c9d0);
export!(tsfp_lane_00057880, 0x57880);
export!(tsfp_lane_0015bb50, 0x15bb50);
export!(tsfp_lane_0015cd00, 0x15cd00);
export!(tsfp_lane_003c8960, 0x3c8960);
export!(tsfp_lane_000e8ed0, 0xe8ed0);
export!(tsfp_lane_00087310, 0x87310);
export!(tsfp_lane_0005f040, 0x5f040);
export!(tsfp_lane_000c8120, 0xc8120);
export!(tsfp_lane_000867f0, 0x867f0);
export!(tsfp_lane_00094a00, 0x94a00);
export!(tsfp_lane_000a79b0, 0xa79b0);
export!(tsfp_lane_000e97a0, 0xe97a0);
export!(tsfp_lane_001b6ff0, 0x1b6ff0);
export!(tsfp_lane_00164c40, 0x164c40);
export!(tsfp_lane_00158d40, 0x158d40);
export!(tsfp_lane_000a2900, 0xa2900);
export!(tsfp_lane_00261c70, 0x261c70);
export!(tsfp_lane_00153440, 0x153440);
export!(tsfp_lane_001531d0, 0x1531d0);
export!(tsfp_lane_00153270, 0x153270);
export!(tsfp_lane_00151650, 0x151650);
export!(tsfp_lane_00153240, 0x153240);
export!(tsfp_lane_0005cef0, 0x5cef0);
export!(tsfp_lane_0005f430, 0x5f430);
export!(tsfp_lane_000c4700, 0xc4700);
export!(tsfp_lane_000d8e30, 0xd8e30);
export!(tsfp_lane_00170500, 0x170500);
export!(tsfp_lane_000c5310, 0xc5310);

// T2118 remote: existing weapon writers behind PlayerSet authority.
export!(tsfp_lane_001b7bc0, 0x1b7bc0);
export!(tsfp_lane_001c0200, 0x1c0200);

// T2137: six typed owner-boundary tail adapters, Tier 1 only; default off.
export!(tsfp_lane_001cb8a0, 0x1cb8a0);
export!(tsfp_lane_001642c0, 0x1642c0);
export!(tsfp_lane_000dac30, 0xdac30);
export!(tsfp_lane_000a6ec0, 0xa6ec0);
export!(tsfp_lane_00058a40, 0x58a40);
export!(tsfp_lane_00172180, 0x172180);
