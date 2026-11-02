// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/t1926_characters
// portreg: mirrors=tsfp-ports/c2r/t1941_characters
// portreg: mirrors=tsfp-port-ai-characters/c2r/t1954_characters
// portreg: mirrors=tsfp-port-ai-behavior/c2r/ai_behavior
// portreg: mirrors=tsfp-port-ai-pathfinding/c2r/ai_pathfinding
// portreg: mirrors=tsfp-ports/c2r/t1892_pathfinding
//! T1996: lane adapters (Tier 2 through `tools.rust_trace`) for the 260 `ai/characters` roots that have a Tier 1 machine
//! translation (T1878, differential against the lifted original): `c2r/t1926_characters`, `t1941_characters`,
//! `t1954_characters`, `ai_behavior` (the class ticks 0x2682A0, 0x262B20, 0x262720, 0x2648C0, 0x2806B0 and the other
//! dispatcher arms, the awareness check 0x15E120) and the three class ticks 0x2A1F30, 0x2A4120, 0x2A0500 of the
//! pathfinding modules. These include the class dispatchers 0x27D590 (story mode A) and 0x27F8E0/0x27DF60/0x27FE30,
//! the scripted actor ticks 0x2B9990 and 0x2B9E90, the combat state machine 0x2B7F30 and the railbot tick 0x37B720.
//! The translations already use the `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which
//! of the modules' roots the host may select, and normalises ESP. Registered through `lane_inv.rs` (the `export!`
//! symbols are defined here). The hero tick 0x2784D0 (T1951) and 0x2621A0 (`npc_lane.rs`) are not part of this list.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r,
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// Dispatches a root; `false` when `va` is not one of the character adapters.
///
/// The c2r bodies end with ESP at the entry value (`ret N` is modelled as `esp += N - 4`, the return slot is not popped), the
/// host adapter pops the return slot and the callee-cleaned arguments itself, so ESP is restored to the entry value here.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry_esp = regs.esp;
    let ran = match va {
        // T2122: character body writers answer from the wired translation (stores go through CharacterSet::classify)
        0x2682a0 | 0x15af30 | 0x15f0d0 | 0x27efb0 => c2r::objchar_wired::run(va, m, guest, regs),
        0x15d1f0 | 0x2b9990 => c2r::objchar_wired2::run(va, m, guest, regs),
        // T2122 pass 4: the class tick writer answers from objchar_wired12 (stores through the leaf setters).
        0x262b20 => c2r::objchar_wired12::run(va, m, guest, regs),
        0x27b8b0 => c2r::objchar_wired15::run(va, m, guest, regs),
        0x2b8f90 | 0x27efb0 | 0x27fe30 | 0x1b6a00 | 0x27d590 | 0x15af30 | 0x15f0d0 | 0x261990
        | 0x276b60 | 0x2777e0 | 0x276a10 | 0x2b9990 | 0x27d490 | 0x276540 | 0x2771c0 | 0x2772b0
        | 0x277e00 | 0x27cbd0 | 0x27cef0 | 0x27cb10 | 0x27b8b0 | 0x2663d0 | 0x27f8e0 | 0x261cd0
        | 0x15a540 | 0x26c5e0 | 0x2778a0 | 0x27cc20 | 0x27f100 | 0x267ca0 | 0x276f80 | 0x2774d0
        | 0x2b05a0 | 0xa4b40 | 0x1594e0 | 0x15a2d0 | 0x15e450 | 0x160f80 | 0x189040 | 0x18acc0
        | 0x18e580 | 0x1b9340 | 0x244f30 | 0x260760 | 0x260940 | 0x263c80 | 0x264030 | 0x265460
        | 0x265960 | 0x26b5e0 | 0x26c7e0 | 0x26c9a0 | 0x26cb30 | 0x26cbe0 | 0x26ce20 | 0x26d1b0
        | 0x26d2f0 | 0x26d6d0 | 0x26d930 | 0x26dbe0 | 0x26dd80 | 0x26dfb0 | 0x26ea00 | 0x271bb0
        | 0x271cb0 | 0x272060 | 0x272660 | 0x275530 | 0x275bb0 | 0x276250 | 0x2768c0 | 0x2768f0
        | 0x276a00 | 0x276ca0 | 0x276cb0 | 0x276ce0 | 0x276d10 | 0x276ef0 | 0x2770d0 | 0x277530
        | 0x277600 | 0x277690 | 0x277bc0 | 0x277d30 | 0x277d90 | 0x278070 | 0x27ba40 | 0x27c8c0
        | 0x27c9a0 | 0x27ccd0 | 0x27cda0 | 0x27df60 | 0x27e6c0 | 0x27fe90 | 0x281620 | 0x281f30
        | 0x2836d0 | 0x283ed0 | 0x283f80 | 0x283ff0 | 0x284070 | 0x284430 | 0x2847c0 | 0x28d0a0
        | 0x292430 | 0x294520 | 0x29cbf0 | 0x2a1a90 | 0x2a1ae0 | 0x2b0320 | 0x2b0500 | 0x2b0770 => {
            c2r::t1926_characters::run(va, m, guest, regs)
        }
        0x2b0b10 | 0x2b1240 | 0x2b12f0 | 0x2b14c0 | 0x2b15a0 | 0x2b1b60 | 0x2b2330 | 0x2b25e0
        | 0x2b3e90 | 0x2b4870 | 0x2b4c70 | 0x2b53f0 | 0x2b7750 | 0x2b7cc0 | 0x2b7f30 | 0x2b8bd0
        | 0x2b8ea0 | 0x2b8f40 | 0x2b9e90 | 0x2ba090 | 0x2ba100 | 0x2ba160 | 0x2ba1c0 => {
            c2r::t1941_characters::run(va, m, guest, regs)
        }
        0x37b720 => c2r::t1954_characters::run(va, m, guest, regs),
        0x15e120 | 0x15fe00 | 0x2682a0 | 0x59db0 | 0x161730 | 0x15d1f0 | 0x276660 | 0x262b20
        | 0x26a500 | 0x267ff0 | 0x276160 | 0x5ba40 | 0x15b240 | 0x15be50 | 0x15be60 | 0x161430
        | 0x188d40 | 0x2603f0 | 0x261750 | 0x262720 | 0x262c20 | 0x262d40 | 0x263260 | 0x263710
        | 0x263870 | 0x263d00 | 0x263e20 | 0x2648c0 | 0x264a10 | 0x2651a0 | 0x265820 | 0x266620
        | 0x266b10 | 0x266dd0 | 0x267450 | 0x267710 | 0x267790 | 0x267d50 | 0x269240 | 0x26a7a0
        | 0x26b7a0 | 0x26c0e0 | 0x26c630 | 0x26d390 | 0x272800 | 0x273240 | 0x273a20 | 0x274690
        | 0x274ba0 | 0x275050 | 0x27bec0 | 0x27c0c0 | 0x27e4e0 | 0x27f160 | 0x280070 | 0x2806b0
        | 0x280fa0 | 0x2816d0 | 0x281fe0 | 0x282300 | 0x2834b0 | 0x284a00 | 0x2861a0 | 0x286380
        | 0x286d30 | 0x286f10 | 0x2875f0 | 0x287860 | 0x287e00 | 0x287e20 | 0x288230 | 0x288540
        | 0x288ab0 | 0x288bd0 | 0x288e20 | 0x289120 | 0x289290 | 0x289410 | 0x2895a0 | 0x289f30
        | 0x28a0a0 | 0x28a290 | 0x28a650 | 0x28a9d0 | 0x28b610 | 0x28b730 | 0x28b780 | 0x28ba60
        | 0x28c1c0 | 0x28c640 | 0x28ca40 | 0x28dd50 | 0x293730 | 0x2998e0 | 0x29ce00 | 0x29fe00
        | 0x29ff40 | 0x2a0d10 | 0x2a0e20 | 0x2a0ed0 | 0x2a1010 | 0x2a14b0 | 0x2a16d0 | 0x2a17b0
        | 0x2a29b0 | 0x2b1120 | 0x2b11b0 | 0x2b18e0 | 0x2b1a90 | 0x2b1e90 | 0x2b2050 | 0x2b2750
        | 0x2b4060 | 0x2b4260 | 0x2b4510 | 0x2b55d0 | 0x2b5b90 | 0x2b6170 | 0x2b66a0 | 0x2b68b0
        | 0x2ba300 => c2r::ai_behavior::run(va, m, guest, regs),
        0x2a1f30 | 0x2a4120 => c2r::ai_pathfinding::run(va, m, guest, regs),
        0x2a0500 => c2r::t1892_pathfinding::run(va, m, guest, regs),
        _ => return false,
    };
    regs.esp = entry_esp;
    ran
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 260] = [
    0x2b8f90, 0x27efb0, 0x27fe30, 0x1b6a00, 0x27d590, 0x15af30, 0x15f0d0, 0x261990, 0x276b60,
    0x2777e0, 0x276a10, 0x2b9990, 0x27d490, 0x276540, 0x2771c0, 0x2772b0, 0x277e00, 0x27cbd0,
    0x27cef0, 0x27cb10, 0x27b8b0, 0x2663d0, 0x27f8e0, 0x261cd0, 0x15a540, 0x26c5e0, 0x2778a0,
    0x27cc20, 0x27f100, 0x267ca0, 0x276f80, 0x2774d0, 0x2b05a0, 0xa4b40, 0x1594e0, 0x15a2d0,
    0x15e450, 0x160f80, 0x189040, 0x18acc0, 0x18e580, 0x1b9340, 0x244f30, 0x260760, 0x260940,
    0x263c80, 0x264030, 0x265460, 0x265960, 0x26b5e0, 0x26c7e0, 0x26c9a0, 0x26cb30, 0x26cbe0,
    0x26ce20, 0x26d1b0, 0x26d2f0, 0x26d6d0, 0x26d930, 0x26dbe0, 0x26dd80, 0x26dfb0, 0x26ea00,
    0x271bb0, 0x271cb0, 0x272060, 0x272660, 0x275530, 0x275bb0, 0x276250, 0x2768c0, 0x2768f0,
    0x276a00, 0x276ca0, 0x276cb0, 0x276ce0, 0x276d10, 0x276ef0, 0x2770d0, 0x277530, 0x277600,
    0x277690, 0x277bc0, 0x277d30, 0x277d90, 0x278070, 0x27ba40, 0x27c8c0, 0x27c9a0, 0x27ccd0,
    0x27cda0, 0x27df60, 0x27e6c0, 0x27fe90, 0x281620, 0x281f30, 0x2836d0, 0x283ed0, 0x283f80,
    0x283ff0, 0x284070, 0x284430, 0x2847c0, 0x28d0a0, 0x292430, 0x294520, 0x29cbf0, 0x2a1a90,
    0x2a1ae0, 0x2b0320, 0x2b0500, 0x2b0770, 0x2b0b10, 0x2b1240, 0x2b12f0, 0x2b14c0, 0x2b15a0,
    0x2b1b60, 0x2b2330, 0x2b25e0, 0x2b3e90, 0x2b4870, 0x2b4c70, 0x2b53f0, 0x2b7750, 0x2b7cc0,
    0x2b7f30, 0x2b8bd0, 0x2b8ea0, 0x2b8f40, 0x2b9e90, 0x2ba090, 0x2ba100, 0x2ba160, 0x2ba1c0,
    0x37b720, 0x15e120, 0x15fe00, 0x2682a0, 0x59db0, 0x161730, 0x15d1f0, 0x276660, 0x262b20,
    0x26a500, 0x267ff0, 0x276160, 0x5ba40, 0x15b240, 0x15be50, 0x15be60, 0x161430, 0x188d40,
    0x2603f0, 0x261750, 0x262720, 0x262c20, 0x262d40, 0x263260, 0x263710, 0x263870, 0x263d00,
    0x263e20, 0x2648c0, 0x264a10, 0x2651a0, 0x265820, 0x266620, 0x266b10, 0x266dd0, 0x267450,
    0x267710, 0x267790, 0x267d50, 0x269240, 0x26a7a0, 0x26b7a0, 0x26c0e0, 0x26c630, 0x26d390,
    0x272800, 0x273240, 0x273a20, 0x274690, 0x274ba0, 0x275050, 0x27bec0, 0x27c0c0, 0x27e4e0,
    0x27f160, 0x280070, 0x2806b0, 0x280fa0, 0x2816d0, 0x281fe0, 0x282300, 0x2834b0, 0x284a00,
    0x2861a0, 0x286380, 0x286d30, 0x286f10, 0x2875f0, 0x287860, 0x287e00, 0x287e20, 0x288230,
    0x288540, 0x288ab0, 0x288bd0, 0x288e20, 0x289120, 0x289290, 0x289410, 0x2895a0, 0x289f30,
    0x28a0a0, 0x28a290, 0x28a650, 0x28a9d0, 0x28b610, 0x28b730, 0x28b780, 0x28ba60, 0x28c1c0,
    0x28c640, 0x28ca40, 0x28dd50, 0x293730, 0x2998e0, 0x29ce00, 0x29fe00, 0x29ff40, 0x2a0d10,
    0x2a0e20, 0x2a0ed0, 0x2a1010, 0x2a14b0, 0x2a16d0, 0x2a17b0, 0x2a29b0, 0x2b1120, 0x2b11b0,
    0x2b18e0, 0x2b1a90, 0x2b1e90, 0x2b2050, 0x2b2750, 0x2b4060, 0x2b4260, 0x2b4510, 0x2b55d0,
    0x2b5b90, 0x2b6170, 0x2b66a0, 0x2b68b0, 0x2ba300, 0x2a1f30, 0x2a4120, 0x2a0500,
];

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
export!(tsfp_lane_002b8f90, 0x2b8f90);
export!(tsfp_lane_0027efb0, 0x27efb0);
export!(tsfp_lane_0027fe30, 0x27fe30);
export!(tsfp_lane_001b6a00, 0x1b6a00);
export!(tsfp_lane_0027d590, 0x27d590);
export!(tsfp_lane_0015af30, 0x15af30);
export!(tsfp_lane_0015f0d0, 0x15f0d0);
export!(tsfp_lane_00261990, 0x261990);
export!(tsfp_lane_00276b60, 0x276b60);
export!(tsfp_lane_002777e0, 0x2777e0);
export!(tsfp_lane_00276a10, 0x276a10);
export!(tsfp_lane_002b9990, 0x2b9990);
export!(tsfp_lane_0027d490, 0x27d490);
export!(tsfp_lane_00276540, 0x276540);
export!(tsfp_lane_002771c0, 0x2771c0);
export!(tsfp_lane_002772b0, 0x2772b0);
export!(tsfp_lane_00277e00, 0x277e00);
export!(tsfp_lane_0027cbd0, 0x27cbd0);
export!(tsfp_lane_0027cef0, 0x27cef0);
export!(tsfp_lane_0027cb10, 0x27cb10);
export!(tsfp_lane_0027b8b0, 0x27b8b0);
export!(tsfp_lane_002663d0, 0x2663d0);
export!(tsfp_lane_0027f8e0, 0x27f8e0);
export!(tsfp_lane_00261cd0, 0x261cd0);
export!(tsfp_lane_0015a540, 0x15a540);
export!(tsfp_lane_0026c5e0, 0x26c5e0);
export!(tsfp_lane_002778a0, 0x2778a0);
export!(tsfp_lane_0027cc20, 0x27cc20);
export!(tsfp_lane_0027f100, 0x27f100);
export!(tsfp_lane_00267ca0, 0x267ca0);
export!(tsfp_lane_00276f80, 0x276f80);
export!(tsfp_lane_002774d0, 0x2774d0);
export!(tsfp_lane_002b05a0, 0x2b05a0);
export!(tsfp_lane_000a4b40, 0xa4b40);
export!(tsfp_lane_001594e0, 0x1594e0);
export!(tsfp_lane_0015a2d0, 0x15a2d0);
export!(tsfp_lane_0015e450, 0x15e450);
export!(tsfp_lane_00160f80, 0x160f80);
export!(tsfp_lane_00189040, 0x189040);
export!(tsfp_lane_0018acc0, 0x18acc0);
export!(tsfp_lane_0018e580, 0x18e580);
export!(tsfp_lane_001b9340, 0x1b9340);
export!(tsfp_lane_00244f30, 0x244f30);
export!(tsfp_lane_00260760, 0x260760);
export!(tsfp_lane_00260940, 0x260940);
export!(tsfp_lane_00263c80, 0x263c80);
export!(tsfp_lane_00264030, 0x264030);
export!(tsfp_lane_00265460, 0x265460);
export!(tsfp_lane_00265960, 0x265960);
export!(tsfp_lane_0026b5e0, 0x26b5e0);
export!(tsfp_lane_0026c7e0, 0x26c7e0);
export!(tsfp_lane_0026c9a0, 0x26c9a0);
export!(tsfp_lane_0026cb30, 0x26cb30);
export!(tsfp_lane_0026cbe0, 0x26cbe0);
export!(tsfp_lane_0026ce20, 0x26ce20);
export!(tsfp_lane_0026d1b0, 0x26d1b0);
export!(tsfp_lane_0026d2f0, 0x26d2f0);
export!(tsfp_lane_0026d6d0, 0x26d6d0);
export!(tsfp_lane_0026d930, 0x26d930);
export!(tsfp_lane_0026dbe0, 0x26dbe0);
export!(tsfp_lane_0026dd80, 0x26dd80);
export!(tsfp_lane_0026dfb0, 0x26dfb0);
export!(tsfp_lane_0026ea00, 0x26ea00);
export!(tsfp_lane_00271bb0, 0x271bb0);
export!(tsfp_lane_00271cb0, 0x271cb0);
export!(tsfp_lane_00272060, 0x272060);
export!(tsfp_lane_00272660, 0x272660);
export!(tsfp_lane_00275530, 0x275530);
export!(tsfp_lane_00275bb0, 0x275bb0);
export!(tsfp_lane_00276250, 0x276250);
export!(tsfp_lane_002768c0, 0x2768c0);
export!(tsfp_lane_002768f0, 0x2768f0);
export!(tsfp_lane_00276a00, 0x276a00);
export!(tsfp_lane_00276ca0, 0x276ca0);
export!(tsfp_lane_00276cb0, 0x276cb0);
export!(tsfp_lane_00276ce0, 0x276ce0);
export!(tsfp_lane_00276d10, 0x276d10);
export!(tsfp_lane_00276ef0, 0x276ef0);
export!(tsfp_lane_002770d0, 0x2770d0);
export!(tsfp_lane_00277530, 0x277530);
export!(tsfp_lane_00277600, 0x277600);
export!(tsfp_lane_00277690, 0x277690);
export!(tsfp_lane_00277bc0, 0x277bc0);
export!(tsfp_lane_00277d30, 0x277d30);
export!(tsfp_lane_00277d90, 0x277d90);
export!(tsfp_lane_00278070, 0x278070);
export!(tsfp_lane_0027ba40, 0x27ba40);
export!(tsfp_lane_0027c8c0, 0x27c8c0);
export!(tsfp_lane_0027c9a0, 0x27c9a0);
export!(tsfp_lane_0027ccd0, 0x27ccd0);
export!(tsfp_lane_0027cda0, 0x27cda0);
export!(tsfp_lane_0027df60, 0x27df60);
export!(tsfp_lane_0027e6c0, 0x27e6c0);
export!(tsfp_lane_0027fe90, 0x27fe90);
export!(tsfp_lane_00281620, 0x281620);
export!(tsfp_lane_00281f30, 0x281f30);
export!(tsfp_lane_002836d0, 0x2836d0);
export!(tsfp_lane_00283ed0, 0x283ed0);
export!(tsfp_lane_00283f80, 0x283f80);
export!(tsfp_lane_00283ff0, 0x283ff0);
export!(tsfp_lane_00284070, 0x284070);
export!(tsfp_lane_00284430, 0x284430);
export!(tsfp_lane_002847c0, 0x2847c0);
export!(tsfp_lane_0028d0a0, 0x28d0a0);
export!(tsfp_lane_00292430, 0x292430);
export!(tsfp_lane_00294520, 0x294520);
export!(tsfp_lane_0029cbf0, 0x29cbf0);
export!(tsfp_lane_002a1a90, 0x2a1a90);
export!(tsfp_lane_002a1ae0, 0x2a1ae0);
export!(tsfp_lane_002b0320, 0x2b0320);
export!(tsfp_lane_002b0500, 0x2b0500);
export!(tsfp_lane_002b0770, 0x2b0770);
export!(tsfp_lane_002b0b10, 0x2b0b10);
export!(tsfp_lane_002b1240, 0x2b1240);
export!(tsfp_lane_002b12f0, 0x2b12f0);
export!(tsfp_lane_002b14c0, 0x2b14c0);
export!(tsfp_lane_002b15a0, 0x2b15a0);
export!(tsfp_lane_002b1b60, 0x2b1b60);
export!(tsfp_lane_002b2330, 0x2b2330);
export!(tsfp_lane_002b25e0, 0x2b25e0);
export!(tsfp_lane_002b3e90, 0x2b3e90);
export!(tsfp_lane_002b4870, 0x2b4870);
export!(tsfp_lane_002b4c70, 0x2b4c70);
export!(tsfp_lane_002b53f0, 0x2b53f0);
export!(tsfp_lane_002b7750, 0x2b7750);
export!(tsfp_lane_002b7cc0, 0x2b7cc0);
export!(tsfp_lane_002b7f30, 0x2b7f30);
export!(tsfp_lane_002b8bd0, 0x2b8bd0);
export!(tsfp_lane_002b8ea0, 0x2b8ea0);
export!(tsfp_lane_002b8f40, 0x2b8f40);
export!(tsfp_lane_002b9e90, 0x2b9e90);
export!(tsfp_lane_002ba090, 0x2ba090);
export!(tsfp_lane_002ba100, 0x2ba100);
export!(tsfp_lane_002ba160, 0x2ba160);
export!(tsfp_lane_002ba1c0, 0x2ba1c0);
export!(tsfp_lane_0037b720, 0x37b720);
export!(tsfp_lane_0015e120, 0x15e120);
export!(tsfp_lane_0015fe00, 0x15fe00);
export!(tsfp_lane_002682a0, 0x2682a0);
export!(tsfp_lane_00059db0, 0x59db0);
export!(tsfp_lane_00161730, 0x161730);
export!(tsfp_lane_0015d1f0, 0x15d1f0);
export!(tsfp_lane_00276660, 0x276660);
export!(tsfp_lane_00262b20, 0x262b20);
export!(tsfp_lane_0026a500, 0x26a500);
export!(tsfp_lane_00267ff0, 0x267ff0);
export!(tsfp_lane_00276160, 0x276160);
export!(tsfp_lane_0005ba40, 0x5ba40);
export!(tsfp_lane_0015b240, 0x15b240);
export!(tsfp_lane_0015be50, 0x15be50);
export!(tsfp_lane_0015be60, 0x15be60);
export!(tsfp_lane_00161430, 0x161430);
export!(tsfp_lane_00188d40, 0x188d40);
export!(tsfp_lane_002603f0, 0x2603f0);
export!(tsfp_lane_00261750, 0x261750);
export!(tsfp_lane_00262720, 0x262720);
export!(tsfp_lane_00262c20, 0x262c20);
export!(tsfp_lane_00262d40, 0x262d40);
export!(tsfp_lane_00263260, 0x263260);
export!(tsfp_lane_00263710, 0x263710);
export!(tsfp_lane_00263870, 0x263870);
export!(tsfp_lane_00263d00, 0x263d00);
export!(tsfp_lane_00263e20, 0x263e20);
export!(tsfp_lane_002648c0, 0x2648c0);
export!(tsfp_lane_00264a10, 0x264a10);
export!(tsfp_lane_002651a0, 0x2651a0);
export!(tsfp_lane_00265820, 0x265820);
export!(tsfp_lane_00266620, 0x266620);
export!(tsfp_lane_00266b10, 0x266b10);
export!(tsfp_lane_00266dd0, 0x266dd0);
export!(tsfp_lane_00267450, 0x267450);
export!(tsfp_lane_00267710, 0x267710);
export!(tsfp_lane_00267790, 0x267790);
export!(tsfp_lane_00267d50, 0x267d50);
export!(tsfp_lane_00269240, 0x269240);
export!(tsfp_lane_0026a7a0, 0x26a7a0);
export!(tsfp_lane_0026b7a0, 0x26b7a0);
export!(tsfp_lane_0026c0e0, 0x26c0e0);
export!(tsfp_lane_0026c630, 0x26c630);
export!(tsfp_lane_0026d390, 0x26d390);
export!(tsfp_lane_00272800, 0x272800);
export!(tsfp_lane_00273240, 0x273240);
export!(tsfp_lane_00273a20, 0x273a20);
export!(tsfp_lane_00274690, 0x274690);
export!(tsfp_lane_00274ba0, 0x274ba0);
export!(tsfp_lane_00275050, 0x275050);
export!(tsfp_lane_0027bec0, 0x27bec0);
export!(tsfp_lane_0027c0c0, 0x27c0c0);
export!(tsfp_lane_0027e4e0, 0x27e4e0);
export!(tsfp_lane_0027f160, 0x27f160);
export!(tsfp_lane_00280070, 0x280070);
export!(tsfp_lane_002806b0, 0x2806b0);
export!(tsfp_lane_00280fa0, 0x280fa0);
export!(tsfp_lane_002816d0, 0x2816d0);
export!(tsfp_lane_00281fe0, 0x281fe0);
export!(tsfp_lane_00282300, 0x282300);
export!(tsfp_lane_002834b0, 0x2834b0);
export!(tsfp_lane_00284a00, 0x284a00);
export!(tsfp_lane_002861a0, 0x2861a0);
export!(tsfp_lane_00286380, 0x286380);
export!(tsfp_lane_00286d30, 0x286d30);
export!(tsfp_lane_00286f10, 0x286f10);
export!(tsfp_lane_002875f0, 0x2875f0);
export!(tsfp_lane_00287860, 0x287860);
export!(tsfp_lane_00287e00, 0x287e00);
export!(tsfp_lane_00287e20, 0x287e20);
export!(tsfp_lane_00288230, 0x288230);
export!(tsfp_lane_00288540, 0x288540);
export!(tsfp_lane_00288ab0, 0x288ab0);
export!(tsfp_lane_00288bd0, 0x288bd0);
export!(tsfp_lane_00288e20, 0x288e20);
export!(tsfp_lane_00289120, 0x289120);
export!(tsfp_lane_00289290, 0x289290);
export!(tsfp_lane_00289410, 0x289410);
export!(tsfp_lane_002895a0, 0x2895a0);
export!(tsfp_lane_00289f30, 0x289f30);
export!(tsfp_lane_0028a0a0, 0x28a0a0);
export!(tsfp_lane_0028a290, 0x28a290);
export!(tsfp_lane_0028a650, 0x28a650);
export!(tsfp_lane_0028a9d0, 0x28a9d0);
export!(tsfp_lane_0028b610, 0x28b610);
export!(tsfp_lane_0028b730, 0x28b730);
export!(tsfp_lane_0028b780, 0x28b780);
export!(tsfp_lane_0028ba60, 0x28ba60);
export!(tsfp_lane_0028c1c0, 0x28c1c0);
export!(tsfp_lane_0028c640, 0x28c640);
export!(tsfp_lane_0028ca40, 0x28ca40);
export!(tsfp_lane_0028dd50, 0x28dd50);
export!(tsfp_lane_00293730, 0x293730);
export!(tsfp_lane_002998e0, 0x2998e0);
export!(tsfp_lane_0029ce00, 0x29ce00);
export!(tsfp_lane_0029fe00, 0x29fe00);
export!(tsfp_lane_0029ff40, 0x29ff40);
export!(tsfp_lane_002a0d10, 0x2a0d10);
export!(tsfp_lane_002a0e20, 0x2a0e20);
export!(tsfp_lane_002a0ed0, 0x2a0ed0);
export!(tsfp_lane_002a1010, 0x2a1010);
export!(tsfp_lane_002a14b0, 0x2a14b0);
export!(tsfp_lane_002a16d0, 0x2a16d0);
export!(tsfp_lane_002a17b0, 0x2a17b0);
export!(tsfp_lane_002a29b0, 0x2a29b0);
export!(tsfp_lane_002b1120, 0x2b1120);
export!(tsfp_lane_002b11b0, 0x2b11b0);
export!(tsfp_lane_002b18e0, 0x2b18e0);
export!(tsfp_lane_002b1a90, 0x2b1a90);
export!(tsfp_lane_002b1e90, 0x2b1e90);
export!(tsfp_lane_002b2050, 0x2b2050);
export!(tsfp_lane_002b2750, 0x2b2750);
export!(tsfp_lane_002b4060, 0x2b4060);
export!(tsfp_lane_002b4260, 0x2b4260);
export!(tsfp_lane_002b4510, 0x2b4510);
export!(tsfp_lane_002b55d0, 0x2b55d0);
export!(tsfp_lane_002b5b90, 0x2b5b90);
export!(tsfp_lane_002b6170, 0x2b6170);
export!(tsfp_lane_002b66a0, 0x2b66a0);
export!(tsfp_lane_002b68b0, 0x2b68b0);
export!(tsfp_lane_002ba300, 0x2ba300);
export!(tsfp_lane_002a1f30, 0x2a1f30);
export!(tsfp_lane_002a4120, 0x2a4120);
export!(tsfp_lane_002a0500, 0x2a0500);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_unique_and_match_the_dispatch_table() {
        let mut sorted = ROOTS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ROOTS.len());
        let text = include_str!("char_lane.rs");
        let table = text
            .split("match va {")
            .nth(1)
            .expect("dispatch table")
            .split("_ => return false")
            .next()
            .expect("end");
        // redirect arms (T2122, T2126) answer roots from `objchar_wired`; they are not table entries
        let table: String = table
            .lines()
            .filter(|line| !line.contains("objchar_wired"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(table.matches("0x").count(), ROOTS.len());
    }
}
