// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/leaf_wired
// portreg: mirrors=tsfp-ports/c2r/objchar_wired31
// portreg: mirrors=tsfp-ports/c2r/objchar_wired32
// portreg: mirrors=tsfp-ports/c2r/objchar_wired33
// portreg: mirrors=tsfp-ports/c2r/objchar_wired34
// portreg: mirrors=tsfp-port-gameplay-modes/c2r/modes_deferred
// portreg: mirrors=tsfp-ports/c2r/objchar_wired35
// portreg: mirrors=tsfp-ports/c2r/objchar_wired36
//! T2091 (S3 writer wiring 2): lane adapters for the small leaf writers of the Rust-owned SaveSlots, TextBank,
//! ModeSetup and player stats cells (tsfp-state `save_text.rs`, `modes.rs`, `player.rs`). The bodies are the c2r
//! translations in `c2r/leaf_wired.rs` (Tier 1 differential vs lifted); their cell stores go through `leaf_wire`
//! and so through the owner setters. Five roots are new lane roots (`ROOTS`). `0x238510` and `0x2385B0` are
//! already inventory pose-lane roots whose hand ports answered in `pose_leaf`, and `0x69D90` is a player-lane root:
//! this adapter answers them first (`REDIRECTED`), the player lane through `player_system::run`. Default behaviour
//! is unchanged: nothing is selected unless the spec names the VA. Registered through `lane_inv.rs`. T2114 adds the
//! PadState/AudioRamp, Missions and Inventory writers (batch `leaf2_wired`); `0x426A0`, `0x1B6B40` and `0x1BFAA0` are
//! existing inventory-lane roots redirected here.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::{
        leaf_wired, leaf2_wired, leaf3_wired, leaf4_wired, leaf5_wired, objchar_wired,
        objchar_wired2, objchar_wired3, objchar_wired4, objchar_wired5, objchar_wired6,
        objchar_wired7, objchar_wired8, objchar_wired9, objchar_wired10, objchar_wired11,
        objchar_wired12, objchar_wired14, objchar_wired17, objchar_wired24, objchar_wired27,
        objchar_wired31, objchar_wired32, objchar_wired33, objchar_wired34, objchar_wired35,
        objchar_wired36, player_wired, player2_wired, player3_wired,
    },
    memory::GuestMemory,
};

/// Existing lane roots now answered by the wired translation (the pose lane's hand ports are not used for them).
const REDIRECTED: [u32; 3] = [0x238510, 0x2385b0, 0x426a0];

/// T2118: player record writers that are already pose/inventory lane roots (wired by `c2r/player_wired.rs`).
const PLAYER_REDIRECTED: [u32; 9] = [
    0xd1e20, 0x25a080, 0x25b720, 0x259df0, 0x1c4400, 0x1c3100, 0x1bf410, 0x1bc890, 0x1c2e80,
];

/// T2122: new lane roots of the object pool block, character and object record writers (batch `objchar_wired`).
pub const OBJCHAR_ROOTS: [u32; 12] = [
    0x9e4b0, 0xa1ea0, 0xa0130, 0xa5c00, 0xa5af0, 0x15eae0, 0x1be870, 0x27ee70, 0x15ab80, 0x15d500,
    0xa7670, 0x9eab0,
];

/// T2126: the next new lane roots (batch `objchar_wired2`).
pub const OBJCHAR_ROOTS2: [u32; 18] = [
    0xa2b50, 0x9ff10, 0x167780, 0x15e830, 0x1260c0, 0x1099e0, 0x15c6c0, 0xd1cb0, 0x5c5b0, 0xe1200,
    0xa17c0, 0xa5f40, 0xa0860, 0xa7870, 0x15c9d0, 0x57880, 0x15bb50, 0x15cd00,
];

/// T2129: new lane roots of batch `objchar_wired3` (the writers T2126 skipped as untranslated). 0x21A120 is translated
/// in the batch but is not a lane root: it has no definition in the pinned lifted tree `generated/lifted/gen` (only in
/// the newer private lift pin), so the host adapter generator has no body to wrap.
pub const OBJCHAR_ROOTS3: [u32; 14] = [
    0x3c8960, 0xe8ed0, 0x87310, 0x5f040, 0xc8120, 0x867f0, 0x94a00, 0xa79b0, 0xe97a0, 0x1b6ff0,
    0x164c40, 0x158d40, 0xa2900, 0x261c70,
];

/// T2129: existing inventory, pose and render lane roots (already in `lane_inv::INV_ROOTS`) answered by `objchar_wired3`.
pub const OBJCHAR_REDIRECTED3: [u32; 10] = [
    0xa2740, 0xd0ca0, 0x1b66e0, 0x1b6ec0, 0xb2d20, 0x1b9760, 0x1ba130, 0x1bfc00, 0x1bfca0, 0x1ba220,
];

/// T2129: new lane roots of batch `objchar_wired4`: the matrix helpers that store rows into object and character
/// matrices and the duplicate translations.
pub const OBJCHAR_ROOTS4: [u32; 11] = [
    0x153440, 0x1531d0, 0x153270, 0x151650, 0x153240, 0x5cef0, 0x5f430, 0xc4700, 0xd8e30, 0x170500,
    0xc5310,
];

/// T2126 remote: object turn writers admitted through the per-function A/B selector.
pub const OBJCHAR_ROOTS5: [u32; 2] = [0x159300, 0x158f80];

/// T2126 remote continuation: character cursor and object spawn writers.
pub const OBJCHAR_ROOTS6: [u32; 2] = [0x57550, 0xa6880];

/// T2126 remote continuation: host-free object field writers.
pub const OBJCHAR_ROOTS7: [u32; 2] = [0x165a40, 0x266250];

/// T2126 remote continuation: small object record writers.
pub const OBJCHAR_ROOTS8: [u32; 2] = [0xa67c0, 0x5bc20];

/// T2126 remote continuation: post-step and object teardown writers.
pub const OBJCHAR_ROOTS9: [u32; 2] = [0x1719e0, 0xa7090];

/// T2126: host-free object and character cleanup writers.
pub const OBJCHAR_ROOTS10: [u32; 2] = [0xdc7b0, 0x1b7260];

/// T2126: object node allocation and character deactivation writers.
pub const OBJCHAR_ROOTS11: [u32; 2] = [0x9ef60, 0x15c700];

/// T2122 pass 4: next host-free character writers by T2057 weight (0x262B20 is a char_lane root, redirected there).
pub const OBJCHAR_ROOTS12: [u32; 5] = [0x15bd30, 0xe9380, 0x9bae0, 0xe91f0, 0x1596f0];

/// T2126 remote: object pool slot allocator.
pub const OBJCHAR_ROOTS14: [u32; 1] = [0x9ee00];

/// T2122 closing slice: character state-word writer routed through the character leaf setter.
pub const OBJCHAR_ROOTS17: [u32; 1] = [0x15b1b0];

/// T2122 pass 24: mixed weapon-slot writers with CharacterSet fields routed through the leaf setter.
pub const OBJCHAR_ROOTS24: [u32; 2] = [0x1b7680, 0x1c2090];

/// T2122 closing slice: the character tick writer routed through ObjectPool/CharacterSet leaf setters.

/// T2122 closing continuation: clear a pointer-valued CharacterSet target and handle through leaf setters.
pub const OBJCHAR_ROOTS27: [u32; 1] = [0x10e1d0];

/// T2122 closing continuation: object attach-mode fields routed through ObjectPool leaf setters.
pub const OBJCHAR_ROOTS31: [u32; 1] = [0xa2c00];

/// T2122 closing continuation: model-range object setup stores routed through ObjectPool leaf setters.
pub const OBJCHAR_ROOTS32: [u32; 1] = [0x21a710];

/// T2122 closing continuation: object creation record writes routed through ObjectPool leaf setters.
pub const OBJCHAR_ROOTS33: [u32; 1] = [0x234de0];

/// T2122 closing continuation: player/object table position writes routed through ObjectPool leaf setters.
pub const OBJCHAR_ROOTS34: [u32; 1] = [0xd6f60];

/// T2122 closing continuation: attached particle setup routes its spawned object record through leaf setters.
pub const OBJCHAR_ROOTS35: [u32; 1] = [0x111df0];

/// T2122 closing continuation: object teardown clears typed CharacterSet and ObjectPool fields.
pub const OBJCHAR_ROOTS36: [u32; 1] = [0x15dda0];

/// T2122: existing object-lane and character-lane roots whose wired translation answers first (`object_lane::run`,
/// `char_lane::run` call `objchar_redirected`).
pub const OBJCHAR_REDIRECTED: [u32; 18] = [
    0x9e430, 0x9fba0, 0xa7360, 0xa6530, 0x862c0, 0xa0c50, 0x2682a0, 0x15af30, 0x15f0d0, 0x27efb0,
    0xa2430, 0x9fbe0, 0xa00b0, 0x9fd60, 0xa1b80, 0xa14b0, 0x15d1f0, 0x2b9990,
];

/// True when the wired object/character translation answers `va` instead of the previous port.
pub fn objchar_redirected(va: u32) -> bool {
    OBJCHAR_REDIRECTED.contains(&va)
}

/// The new lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 45] = [
    0x1b7bc0, 0x1c0200, 0x699d0, 0x46030, 0x6a850, 0x69f30, 0x23f390, 0x42640, 0x450e0, 0x45160,
    0x9ce00, 0x9a140, 0x9a260, 0x9a630, 0x1c4400, 0x1c3100, 0x1bf410, 0x1bc890, 0x1c2e80,
    // T2137 (batch player2_wired)
    0x1b7680, 0x1bdfd0, 0x1bd820, 0x1bdaf0, 0x219ca0, // T2137 (batch leaf3_wired)
    0x425e0, 0x45970, 0x1bc650, // T2137 pass 3 (batch leaf4_wired)
    0x9a7a0, 0x9a5b0, 0x9af90, 0x43c50, // T2137 pass 3 (batch player3_wired)
    0x88950, 0xcf510, 0x151e80, 0x1baee0, 0x153cd0, 0x1b9dc0, 0x1b7c50, 0x10e1d0, 0xa2c00,
    0x21a710, 0x234de0, 0xd6f60, 0x111df0, 0x15dda0,
];

/// T2137: new lane roots answered by `leaf3_wired` (PadState and AudioRamp writers, the ammo word writer 0x1BC650).
pub const LEAF3_ROOTS: [u32; 3] = [0x425e0, 0x45970, 0x1bc650];

/// T2137: existing input lane roots whose pad table stores go through `PadState` (answered by `leaf3_wired`).
pub const LEAF3_REDIRECTED: [u32; 4] = [0x438b0, 0x42920, 0x42ca0, 0x42d00];

/// T2137: new lane roots answered by `player2_wired` (weapon slot state functions and the 0x219CA0 record writer).
pub const PLAYER2_ROOTS: [u32; 5] = [0x1b7680, 0x1bdfd0, 0x1bd820, 0x1bdaf0, 0x219ca0];

/// T2137 pass 3: new lane roots answered by `leaf4_wired` (mission point/link table writers, the audio key table parser).
pub const LEAF4_ROOTS: [u32; 4] = [0x9a7a0, 0x9a5b0, 0x9af90, 0x43c50];

/// T2137 pass 3: new lane roots answered by `player3_wired` (writers storing through a caller or computed pointer).
pub const PLAYER3_ROOTS: [u32; 7] = [
    0x88950, 0xcf510, 0x151e80, 0x1baee0, 0x153cd0, 0x1b9dc0, 0x1b7c50,
];

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if crate::save_slots::run(va, m, guest, regs) {
        return true;
    }
    if crate::player_wire::run_weapon(va, m, guest, regs) {
        return true;
    }
    // T2138: preserve the ObjectPool/CharacterSet setters and ABI of these
    // existing roots while routing their memory accesses through PlayerRetire.
    // Wrapping outside the retained body also covers helper stores and byte writes.
    if crate::player_wire::WRAPPER_ROOTS.contains(&va) {
        crate::player_wire::audit_entry(&*m, va);
        let selection_ext = (va == 0x1ba130).then(|| m.read32(regs.esp.wrapping_add(4)));
        let mut owned = crate::player_wire::Wired(m);
        let handled = match va {
            0xe1200 => objchar_wired2::run(va, &mut owned, guest, regs),
            0x1ba130 => objchar_wired3::run(va, &mut owned, guest, regs),
            _ => unreachable!(),
        };
        if handled && crate::weapon_hands_runtime::enabled() {
            if let Some(ext) = selection_ext {
                let _ = crate::weapon_hands_runtime::commit_normalized_selection_from_guest(
                    &owned, ext,
                );
            }
        }
        return handled;
    }
    // T2138: the two PlayerSet stats writers (T2091) are PlayerSet Retire roots too
    if matches!(va, 0x238510 | 0x2385b0) {
        crate::player_wire::audit_entry(&*m, va);
    }
    // T2114: the PadState/AudioRamp and Missions writers live in the second batch (0x426A0 is an input lane root, redirected here).
    if matches!(
        va,
        0x426a0
            | 0x42640
            | 0x450e0
            | 0x45160
            | 0x9ce00
            | 0x9a140
            | 0x9a260
            | 0x9a630
            | 0x1b6b40
            | 0x1bfaa0
    ) {
        return leaf2_wired::run(va, m, guest, regs);
    }
    if va == 0xa14b0 && crate::object_transform::selected() {
        crate::object_transform::run(m, regs);
        return true;
    }
    // T2122: the new object pool, character and object record roots
    if OBJCHAR_ROOTS.contains(&va) {
        return objchar_wired::run(va, m, guest, regs);
    }
    if crate::scene_lane::object_room_adapter::run(va, m, regs) {
        return true;
    }
    if OBJCHAR_ROOTS2.contains(&va) {
        return objchar_wired2::run(va, m, guest, regs);
    }
    // T2411 routes the reserve mutation through the session-owned WeaponInventory when enabled.
    // Invalid/missing owners decline to the retained typed writer below.
    if va == 0x1ba220
        && crate::weapon_hands_runtime::enabled()
        && crate::weapon_hands_runtime::dispatch_nested(va, m, guest, regs)
    {
        return true;
    }
    // T2114 pass 2: the ammo add 0x1BA220 (an OBJCHAR_REDIRECTED3 root) stores its reserve words through `Inventory`
    if va == 0x1ba220 {
        return leaf5_wired::run(va, m, guest, regs);
    }
    // T2129: untranslated writers retried (new roots and redirected inventory, pose and render roots), matrix helpers
    if OBJCHAR_ROOTS3.contains(&va) || OBJCHAR_REDIRECTED3.contains(&va) {
        return objchar_wired3::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS4.contains(&va) {
        return objchar_wired4::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS5.contains(&va) {
        return objchar_wired5::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS6.contains(&va) {
        return objchar_wired6::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS7.contains(&va) {
        return objchar_wired7::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS8.contains(&va) {
        return objchar_wired8::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS9.contains(&va) {
        let reference = (va == 0xa7090).then(|| m.read32(regs.esp.wrapping_add(4)));
        let pool_slot = reference.and_then(|object| {
            let base = m.read32(0x72f5e8);
            let count = (m.read32(0x7b4fac) as usize).min(0x226);
            let offset = object.checked_sub(base)?;
            (base != 0 && offset % 0x2e8 == 0)
                .then_some((offset / 0x2e8) as usize)
                .filter(|slot| *slot < count)
        });
        let handled = objchar_wired9::run(va, m, guest, regs);
        if handled {
            if let Some(reference) = reference {
                // A7090 removes this object from the scene list and tears down
                // its owned records. Retire all copied transform views only
                // after the selected teardown has completed.
                crate::object_transform::retire_transform(reference);
            }
            if let Some(slot) = pool_slot {
                // A9070's 9E330 callback dispatch has returned and A7090 has
                // completed before this generation is retired from the
                // optional scene bridge.
                #[cfg(not(any(test, tsfp_mmx_harness)))]
                unsafe {
                    tsfp_native_scene_bridge_retire_object(slot as u32);
                }
            }
        }
        return handled;
    }
    if OBJCHAR_ROOTS10.contains(&va) {
        return objchar_wired10::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS11.contains(&va) {
        let behavior_tick = if va == 0x15c700 {
            let extension = m.read32(regs.esp.wrapping_add(4));
            Some((
                extension,
                crate::rea_ai_runtime::actor_behavior_commit_started(m, extension),
            ))
        } else {
            None
        };
        let ran = objchar_wired11::run(va, m, guest, regs);
        if ran {
            if let Some((extension, token)) = behavior_tick {
                // T2407 TEMPORARY-SHIM: callbacks still execute in the lifted
                // body. Rust owns the behavior-pair commit and refreshes the
                // remaining measured actor snapshot at this boundary.
                crate::rea_ai_runtime::actor_behavior_tick_finished(m, extension, token);
            }
        }
        return ran;
    }
    if OBJCHAR_ROOTS12.contains(&va) {
        return objchar_wired12::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS14.contains(&va) {
        return objchar_wired14::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS17.contains(&va) {
        return objchar_wired17::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS24.contains(&va) {
        return objchar_wired24::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS27.contains(&va) {
        return objchar_wired27::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS31.contains(&va) {
        return objchar_wired31::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS32.contains(&va) {
        return objchar_wired32::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS33.contains(&va) {
        return objchar_wired33::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS34.contains(&va) {
        return objchar_wired34::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS35.contains(&va) {
        return objchar_wired35::run(va, m, guest, regs);
    }
    if OBJCHAR_ROOTS36.contains(&va) {
        // T2407: capture the extension identity before 0x15DDA0 clears
        // object+0x7c, then unregister its Rust-owned AI state. This is not
        // allocator reclamation of the keyed extension.
        let object = m.read32(regs.esp.wrapping_add(4));
        let extension = m.read32(object.wrapping_add(0x7c));
        let ran = objchar_wired36::run(va, m, guest, regs);
        if ran {
            crate::rea_ai_runtime::actor_unregistered(extension);
        }
        return ran;
    }
    // T2118: existing pose/inventory lane roots among the player record writers answer from the wired translation.
    if PLAYER_REDIRECTED.contains(&va) {
        return crate::objchar_player_writer::run(va, m, guest, regs)
            || player_wired::run(va, m, guest, regs);
    }
    if LEAF3_ROOTS.contains(&va) || LEAF3_REDIRECTED.contains(&va) {
        return leaf3_wired::run(va, m, guest, regs);
    }
    if LEAF4_ROOTS.contains(&va) {
        return leaf4_wired::run(va, m, guest, regs);
    }
    if PLAYER3_ROOTS.contains(&va) {
        return player3_wired::run(va, m, guest, regs);
    }
    if PLAYER2_ROOTS.contains(&va) {
        return player2_wired::run(va, m, guest, regs);
    }
    if REDIRECTED.contains(&va) {
        return leaf_wired::run(va, m, guest, regs);
    }
    match va {
        0x699d0 | 0x46030 | 0x6a850 | 0x69f30 | 0x23f390 => leaf_wired::run(va, m, guest, regs),
        _ => {
            use crate::c2r::*;
            t1937_math::run(va, m, guest, regs)
                || engine_anim::run(va, m, guest, regs)
                || t3398_math4::run(va, m, guest, regs)
                || t2049_exec_001::run(va, m, guest, regs)
                || t2049_exec_002::run(va, m, guest, regs)
                || t2049_exec_003::run(va, m, guest, regs)
                || t2049_nx_002::run(va, m, guest, regs)
                || t2049_nx_003::run(va, m, guest, regs)
                || t2049_nx_006::run(va, m, guest, regs)
                || t2125c_002::run(va, m, guest, regs)
                || t1966_sweep::run(va, m, guest, regs)
        }
    }
}

#[cfg(not(any(test, tsfp_mmx_harness)))]
unsafe extern "C" {
    fn tsfp_native_scene_bridge_retire_object(slot: u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T2126: every lane root answered by `objchar_wired2` is in its translation table, and the redirected and weapon-lane
    /// roots it serves are listed too (a root missing from the dispatch chain panics the host as an unregistered lane port).
    #[test]
    fn objchar_wired2_serves_its_roots() {
        for va in OBJCHAR_ROOTS2 {
            assert!(objchar_wired2::ROOTS.contains(&va), "{va:#x}");
        }
        for va in [0x15d1f0, 0x2b9990, 0xd8c80, 0xd19f0, 0xd84c0, 0x25fb70] {
            assert!(objchar_wired2::ROOTS.contains(&va), "{va:#x}");
        }
        assert_eq!(objchar_wired2::ROOTS.len(), OBJCHAR_ROOTS2.len() + 6);
    }

    /// T2129: every root answered by `objchar_wired3` / `objchar_wired4` is in its translation table, the lists do not
    /// overlap and the redirected roots are lane roots of `lane_inv` already.
    #[test]
    fn objchar_wired34_serve_their_roots() {
        for va in OBJCHAR_ROOTS3.iter().chain(OBJCHAR_REDIRECTED3.iter()) {
            assert!(objchar_wired3::ROOTS.contains(va), "{va:#x}");
        }
        // 0x21a120 is translated but not registered as a lane root (see OBJCHAR_ROOTS3)
        assert_eq!(
            objchar_wired3::ROOTS.len(),
            OBJCHAR_ROOTS3.len() + OBJCHAR_REDIRECTED3.len() + 1
        );
        assert!(
            objchar_wired3::ROOTS.contains(&0x21a120)
                && !crate::lane_inv::INV_ROOTS.contains(&0x21a120)
        );
        for va in OBJCHAR_ROOTS4 {
            assert!(objchar_wired4::ROOTS.contains(&va), "{va:#x}");
        }
        assert_eq!(objchar_wired4::ROOTS.len(), OBJCHAR_ROOTS4.len());
        for va in OBJCHAR_REDIRECTED3 {
            assert!(crate::lane_inv::INV_ROOTS.contains(&va), "{va:#x}");
            assert!(
                !OBJCHAR_ROOTS3.contains(&va) && !OBJCHAR_ROOTS4.contains(&va),
                "{va:#x}"
            );
        }
        for va in OBJCHAR_ROOTS3.iter().chain(OBJCHAR_ROOTS4.iter()) {
            assert!(crate::lane_inv::INV_ROOTS.contains(va), "{va:#x}");
        }
    }

    /// T2114 pass 2: 0x1BA220 (ext, type, amount) adds to the reserve word through `Inventory` and clamps it to the type
    /// maximum minus the loaded rounds (FABRICATED tables, the 0x1B67E0 callee stubbed to return the loaded rounds).
    #[test]
    fn ammo_add_writes_the_reserve_through_inventory() {
        struct Ram(std::collections::BTreeMap<u32, u8>);
        impl GuestMemory for Ram {
            fn read8(&self, a: u32) -> u8 {
                self.0.get(&a).copied().unwrap_or(0)
            }
            fn write8(&mut self, a: u32, v: u8) {
                self.0.insert(a, v);
            }
        }
        struct Loaded(u32);
        impl GuestCall for Loaded {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                assert_eq!(va, 0x1b67e0);
                r.eax = self.0;
            }
        }
        let ext = 0x2_0000;
        // (type, reserve before, amount, type maximum, loaded rounds) -> (reserve after, returned)
        for (kind, before, amount, max, loaded, after, added) in [
            (3u32, 10u16, 5u32, 100u32, 0u32, 15u16, 5u32),
            (3, 10, 5, 12, 0, 12, 2),
            (7, 4, 9, 10, 3, 7, 3),
        ] {
            let mut m = Ram(Default::default());
            m.write8(ext + 0x61e + 2 * kind, before as u8);
            m.write8(ext + 0x61f + 2 * kind, (before >> 8) as u8);
            m.write32(0x4ea3f8 + kind * 0xe0, max);
            // SAFETY: ExtRegisters is plain old data (integers and floats), all zero is a valid value.
            let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
            regs.esp = 0x7000;
            m.write32(0x7004, ext);
            m.write32(0x7008, kind);
            m.write32(0x700c, amount);
            assert!(run(0x1ba220, &mut m, &mut Loaded(loaded), &mut regs));
            let inventory = crate::leaf_wire::inventory::Inventory::load(&m, ext);
            assert_eq!(inventory.ammo[kind as usize], after, "type {kind}");
            assert_eq!(regs.eax, added, "type {kind}");
            assert_eq!(regs.esp, 0x7000);
        }
    }

    /// T2122 pass 4: every root of `objchar_wired12` is dispatched (leaf lane or the char_lane redirect).
    #[test]
    fn objchar_wired12_serves_its_roots() {
        for va in OBJCHAR_ROOTS12.iter().chain([0x262b20].iter()) {
            assert!(objchar_wired12::ROOTS.contains(va), "{va:#x}");
            assert!(crate::lane_inv::INV_ROOTS.contains(va), "{va:#x}");
        }
        assert_eq!(objchar_wired12::ROOTS.len(), OBJCHAR_ROOTS12.len() + 1);
    }

    /// T2122 closing slice: the selected character state-word root is registered for the leaf lane.
    #[test]
    fn objchar_wired17_serves_its_root() {
        assert!(objchar_wired17::ROOTS.contains(&0x15b1b0));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x15b1b0));
        assert!(OBJCHAR_ROOTS17.contains(&0x15b1b0));
    }

    /// T2122 closing continuation: the pointer-clear writer has a selectable leaf-lane entry.
    #[test]
    fn objchar_wired27_serves_its_root() {
        assert!(objchar_wired27::ROOTS.contains(&0x10e1d0));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x10e1d0));
        assert_eq!(OBJCHAR_ROOTS27, [0x10e1d0]);
    }

    /// T2122 closing continuation: object attach-mode stores have an opt-in leaf-lane entry.
    #[test]
    fn objchar_wired31_serves_its_root() {
        assert!(objchar_wired31::ROOTS.contains(&0xa2c00));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0xa2c00));
        assert_eq!(OBJCHAR_ROOTS31, [0xa2c00]);
    }

    /// T2122 closing continuation: model-range setup is an opt-in leaf-lane root.
    #[test]
    fn objchar_wired32_serves_its_root() {
        assert!(objchar_wired32::ROOTS.contains(&0x21a710));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x21a710));
        assert_eq!(OBJCHAR_ROOTS32, [0x21a710]);
    }

    /// T2122 closing continuation: object creation stores have an opt-in leaf-lane entry.
    #[test]
    fn objchar_wired33_serves_its_root() {
        assert!(objchar_wired33::ROOTS.contains(&0x234de0));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x234de0));
        assert_eq!(OBJCHAR_ROOTS33, [0x234de0]);
    }

    /// T2122 closing continuation: table-driven player/object writes have an opt-in leaf-lane entry.
    #[test]
    fn objchar_wired34_serves_its_root() {
        assert!(objchar_wired34::ROOTS.contains(&0xd6f60));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0xd6f60));
        assert_eq!(OBJCHAR_ROOTS34, [0xd6f60]);
    }

    /// T2122 closing continuation: attached particle setup has an opt-in leaf-lane entry.
    #[test]
    fn objchar_wired35_serves_its_root() {
        assert!(objchar_wired35::ROOTS.contains(&0x111df0));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x111df0));
        assert_eq!(OBJCHAR_ROOTS35, [0x111df0]);
    }

    /// T2122 closing continuation: object teardown runs as an opt-in leaf-lane root.
    #[test]
    fn objchar_wired36_serves_its_root() {
        assert!(objchar_wired36::ROOTS.contains(&0x15dda0));
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x15dda0));
        assert_eq!(OBJCHAR_ROOTS36, [0x15dda0]);
    }

    /// T2122 pass 4, spec test (INFERRED from the translation): 0x15BD30(a, b, flag) marks bit 0x1000 of the
    /// character with the other index when one index is below the character count, then stamps the pair cell
    /// (triangular index max*(max-1)/2+min, 12-byte cells): flag byte, frame at +4 and, when the flag changed, +8.
    #[test]
    fn pair_mark_sets_character_flag_and_stamps_the_pair_cell() {
        struct Ram(std::collections::BTreeMap<u32, u8>);
        impl GuestMemory for Ram {
            fn read8(&self, a: u32) -> u8 {
                self.0.get(&a).copied().unwrap_or(0)
            }
            fn write8(&mut self, a: u32, v: u8) {
                self.0.insert(a, v);
            }
        }
        struct NoCall;
        impl GuestCall for NoCall {
            fn call(&mut self, va: u32, _: &mut ExtRegisters) {
                panic!("unexpected call {va:#x}");
            }
        }
        let (chars, pairs) = (0x10_0000u32, 0x20_0000u32);
        for (old_flag, frame8) in [(0u8, 77u32), (1, 0)] {
            let mut m = Ram(Default::default());
            m.write32(0x7356d8, chars);
            m.write32(0x790950, 4);
            m.write32(0x74c368, pairs);
            m.write32(0x7de318, 77);
            let cell = pairs + 4 * 12;
            m.write8(cell, old_flag);
            // SAFETY: ExtRegisters is plain old data (integers and floats), all zero is a valid value.
            let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
            regs.esp = 0x7000;
            m.write32(0x7004, 1);
            m.write32(0x7008, 3);
            m.write32(0x700c, 1);
            assert!(run(0x15bd30, &mut m, &mut NoCall, &mut regs));
            assert_eq!(m.read32(chars + 3 * 0xea0 + 0x28), 0x1000);
            assert_eq!(m.read32(chars + 0xea0 + 0x28), 0);
            assert_eq!(m.read8(cell), 1);
            assert_eq!(m.read32(cell + 4), 77);
            assert_eq!(m.read32(cell + 8), frame8);
            assert_eq!(regs.esp, 0x7000);
        }
    }
}
