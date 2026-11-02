# SPDX-License-Identifier: GPL-3.0-or-later
"""Opt-in Rust pilot. Existing registries, fixtures, audits and gates stay unchanged.

An isolated shim tree hashes every Rust input and the compiler into a C header,
so the existing replacement/manifest fingerprints cover the actual linked logic.
"""

from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path
from typing import Any

from .build import BuildError, run
from .scan import Registration, scan_directory

# T1854: 44 wave 4 and 4b roots (audio, input, render, network). They use the full register
# file (`rust_cpu`, callee-saved registers are real harness globals) and the guest-call callback
# of crates/tsfp-ports/src/lane.rs, so they need no change to the 4-register bridge or abi.rs.
# T2221: all root families share the declarative lane manifest.
_LANE_MANIFEST = json.loads(
    (Path(__file__).resolve().parents[2] / "crates/tsfp-ports/lanes.json").read_text()
)


def _legacy_roots(name: str) -> tuple[int, ...]:
    return tuple(int(row["va"], 16) for row in _LANE_MANIFEST["legacy"][name]["roots"])


LANE_ROOTS = _legacy_roots("LANE_ROOTS")
# T1835 wave 2b: 18 object-tree roots (17 object_leaves plus exact 12BA0) through the same lane.
OBJECT_LANE_ROOTS = _legacy_roots("OBJECT_LANE_ROOTS")
ALL_LANE_ROOTS = (*LANE_ROOTS, *OBJECT_LANE_ROOTS)
# T1874: inventory and pose/control roots of T1850. The metadata (cleanup, stack arguments) is
# derived from the lifted bodies by tools/replace/invpose_meta.py. Seven T1850 VAs are already
# admitted through an earlier implementation (59D70, 246D10, 427B0, 42C30, 431C0, 43A90, 43AF0).
_INV_ALREADY = frozenset(
    int(row["va"], 16)
    for lane in _LANE_MANIFEST["lanes"]
    for row in lane["roots"]
    if not row.get("selectable", True)
)
# Roots whose caller resource audit is ineligible (unproven return slot / live flags at a caller),
# measured by the T1874 exploratory run; kept out of the harness matrix (the audit gate is not
# weakened), evidence in docs/data/t1874-audit-rejections.json. They are still host-selectable.
INV_AUDIT_REJECTED = frozenset(
    {
        0x426D0,
        0x42870,
        0x42D00,
        0x5BA00,
        0xC9BE0,
        0xC9C00,
        0xF8AE0,
        0x126260,
        0x1B6BB0,
        0x1B9AB0,
        0x25EC70,
    }
)
# T1905: the T1668 animation adapters (crates/tsfp-ports/src/anim_lane.rs). Host-selectable
# (Tier 2 through tools.rust_trace), kept out of the original-instruction harness matrix: their
# ports are the pure tsfp-engine-anim modules, validated at Tier 1 by their own goldens
# (docs/t1668-anim-tiers.md).
INV_ANIM_LANE = frozenset(
    {
        # T2126 remote: selectable object turn writers through ObjectPool.
        0x159300, 0x158F80,
        0x57550, 0xA6880,
        # T2126 remote continuation: selectable object field writers.
        0x165A40, 0x266250,
        # T2137: owner-boundary tail writers, Tier 1 only; route evidence is debt.
        0x1CB8A0, 0x1642C0, 0xDAC30, 0xA6EC0, 0x58A40, 0x172180,
        # missions_lane.rs (T2230): data/missions roots with a Tier 1 translation
        0x17730, 0x83910, 0x85920, 0x86240, 0x87880, 0x89700, 0x162060, 0x162080,
        0x215790, 0x216230, 0x218910, 0x227430, 0x227650, 0x227710, 0x227750, 0x227920,
        0x228180, 0x228870, 0x234110, 0x234190, 0x235630, 0x237480, 0x237770, 0x237920,
        0x59F90, 0x61570, 0x5A070, 0x5A2B0, 0x59D50, 0x59EC0, 0x5A260, 0x5A2F0, 0x614B0, 0x61450,
        # anim_lane2.rs (T1918 wave 2)
        0x5ADC0, 0x5A360, 0x60670, 0x60920, 0x585E0, 0x59A40, 0x59B00,
        # anim_lane3.rs (T1918 wave B, T1986)
        0x60540, 0x605D0, 0x604D0, 0x60B30, 0x60F80, 0x5AFB0, 0x58690, 0x5A220,
        # T1923: gameplay lane adapters (mp_lane.rs, modes_lane.rs, save_lane.rs, mapmaker_lane.rs).
        # Same status as the animation adapters: host-selectable, Tier 1 against the lifted
        # original in crates/tsfp-c-diff.
        0xC0C50, 0xCB240, 0x13DCF0, 0x80F40, 0x158C80, 0x1CD1E0, 0x69E70, 0x68610,
        0x67280, 0x431F0, 0x2B0AF0, 0x233EF0,
        0x69630, 0x69640, 0x696A0, 0x696D0, 0x69720, 0x697D0,
        0x69900, 0x69930, 0x1CD2B0, 0x1CD330, 0x1CD380, 0x312AD0,
        # modes_lane2.rs (T1923): c2r modes_deferred translations 0x68830, 0x681C0
        0x68830, 0x681C0,
        # npc_lane.rs (T1971)
        0x265CC0, 0x283730, 0x2621A0,
        # hud_lane.rs (T1974): c2r ui_hud translations of the heaviest ui/hud functions
        0x40260, 0x23F2E0, 0x2D7240, 0x12CB40, 0x12CBB0, 0x336570, 0xFC9F0, 0x42F90,
        # hud_lane.rs (T2409w): c2r ui_framework translations of the heaviest ui/framework roots
        0x3FCF0, 0x3F8D0, 0x74760, 0x40740, 0x722B0, 0x40FA0, 0x77470, 0x774B0, 0x40F00,
        0x76400, 0x757E0,
        # audio_lane.rs (T1990): audio translations of T1913/T1924, 31 heaviest roots
        0x28180, 0x29F30, 0x29CC0, 0x45670, 0x29D10, 0x28A60, 0x29BB0, 0x457B0,
        0x26F00, 0x462F0, 0x459F0, 0x458B0, 0x28760, 0x28980, 0x45B60, 0x29AF0,
        0x46350, 0x45A50, 0x45790, 0x26BB0, 0x46610, 0x28610, 0x44AC0, 0x164E00,
        0x1651C0, 0x45540, 0x27BB0, 0x26D20, 0x29DD0, 0x45610, 0x27C20,
        # input_lane.rs (T1988): c2r engine_input and ai_pathfinding translations
        0x18FF70, 0x42970, 0x1CCF80, 0x438B0, 0x1CCE70,
        0x42B60, 0x426A0, 0x190520, 0x2AEFD0, 0x2A5C30,
        # render_lane.rs and render_lane2.rs (T1976): 62 render roots of T1898 (Tier 1),
        # the 0x19240 stdcall arm and the c2r render neighbours
        0x21440, 0x21450, 0x214C0, 0x21740, 0x22560, 0x22790, 0x2A630, 0x22950, 0x226E0,
        0x22720, 0x204B0, 0x22FF0, 0x1A010, 0x19640, 0x19D50, 0x19D60, 0x19D40, 0x19650,
        0x19D70, 0x8F470, 0x8F490, 0x8F4E0, 0x1E770, 0x1E7D0, 0x1E7F0, 0x1E970, 0x1EB00,
        0x18DE0, 0x18EA0, 0x1B7B0, 0x1D5F0, 0x1ED50, 0x8E710, 0x8F5E0, 0xA2740,
        0x10C390, 0x10C400, 0x10C550, 0x153BD0, 0x10D070, 0x22BE0, 0x2A2D0, 0x2A310,
        0x2A320, 0x2A340, 0x2A360, 0x1D380, 0x389B0, 0x459D0, 0x1C720, 0x19CC0, 0x1ED60,
        0x1A2E0, 0x1AFA0, 0x1B090, 0x1EFB0, 0x2A900, 0x2ABE0, 0x2AD60, 0x18E910,
        0xFFE50, 0x10E410, 0x19240, 0x8F6B0, 0x14D60, 0x20C40, 0x1F790, 0x1C620,
        0x1E6B0, 0x21060, 0x22740, 0x1C600, 0x8F950,
        # object_lane.rs (T2026): c2r engine/object translations, 30 heaviest roots
        0xA14B0, 0x1518F0, 0x9E840, 0xA1AB0, 0x9E430, 0xA0C50, 0xA7360, 0xBE860,
        0xA00B0, 0x9E330, 0x9FBA0, 0x74720, 0x9FD60, 0x9FBE0, 0xA1B80, 0x10E210,
        0x162200, 0x2776F0, 0xBDD50, 0x1622C0, 0xA1FA0, 0xA2430, 0x862C0, 0x48FE0,
        0xA6530, 0x9A330, 0x153190, 0x9EB60, 0x23FF20, 0x80430,
        # char_lane.rs (T1996): c2r ai/characters, ai/behavior and pathfinding class ticks
        0x2B8F90, 0x27EFB0, 0x27FE30, 0x1B6A00, 0x27D590, 0x15AF30, 0x15F0D0, 0x261990,
        0x276B60, 0x2777E0, 0x276A10, 0x2B9990, 0x27D490, 0x276540, 0x2771C0, 0x2772B0,
        0x277E00, 0x27CBD0, 0x27CEF0, 0x27CB10, 0x27B8B0, 0x2663D0, 0x27F8E0, 0x261CD0,
        0x15A540, 0x26C5E0, 0x2778A0, 0x27CC20, 0x27F100, 0x267CA0, 0x276F80, 0x2774D0,
        0x2B05A0, 0xA4B40, 0x1594E0, 0x15A2D0, 0x15E450, 0x160F80, 0x189040, 0x18ACC0,
        0x18E580, 0x1B9340, 0x244F30, 0x260760, 0x260940, 0x263C80, 0x264030, 0x265460,
        0x265960, 0x26B5E0, 0x26C7E0, 0x26C9A0, 0x26CB30, 0x26CBE0, 0x26CE20, 0x26D1B0,
        0x26D2F0, 0x26D6D0, 0x26D930, 0x26DBE0, 0x26DD80, 0x26DFB0, 0x26EA00, 0x271BB0,
        0x271CB0, 0x272060, 0x272660, 0x275530, 0x275BB0, 0x276250, 0x2768C0, 0x2768F0,
        0x276A00, 0x276CA0, 0x276CB0, 0x276CE0, 0x276D10, 0x276EF0, 0x2770D0, 0x277530,
        0x277600, 0x277690, 0x277BC0, 0x277D30, 0x277D90, 0x278070, 0x27BA40, 0x27C8C0,
        0x27C9A0, 0x27CCD0, 0x27CDA0, 0x27DF60, 0x27E6C0, 0x27FE90, 0x281620, 0x281F30,
        0x2836D0, 0x283ED0, 0x283F80, 0x283FF0, 0x284070, 0x284430, 0x2847C0, 0x28D0A0,
        0x292430, 0x294520, 0x29CBF0, 0x2A1A90, 0x2A1AE0, 0x2B0320, 0x2B0500, 0x2B0770,
        0x2B0B10, 0x2B1240, 0x2B12F0, 0x2B14C0, 0x2B15A0, 0x2B1B60, 0x2B2330, 0x2B25E0,
        0x2B3E90, 0x2B4870, 0x2B4C70, 0x2B53F0, 0x2B7750, 0x2B7CC0, 0x2B7F30, 0x2B8BD0,
        0x2B8EA0, 0x2B8F40, 0x2B9E90, 0x2BA090, 0x2BA100, 0x2BA160, 0x2BA1C0, 0x37B720,
        0x15E120, 0x15FE00, 0x2682A0, 0x59DB0, 0x161730, 0x15D1F0, 0x276660, 0x262B20,
        0x26A500, 0x267FF0, 0x276160, 0x5BA40, 0x15B240, 0x15BE50, 0x15BE60, 0x161430,
        0x188D40, 0x2603F0, 0x261750, 0x262720, 0x262C20, 0x262D40, 0x263260, 0x263710,
        0x263870, 0x263D00, 0x263E20, 0x2648C0, 0x264A10, 0x2651A0, 0x265820, 0x266620,
        0x266B10, 0x266DD0, 0x267450, 0x267710, 0x267790, 0x267D50, 0x269240, 0x26A7A0,
        0x26B7A0, 0x26C0E0, 0x26C630, 0x26D390, 0x272800, 0x273240, 0x273A20, 0x274690,
        0x274BA0, 0x275050, 0x27BEC0, 0x27C0C0, 0x27E4E0, 0x27F160, 0x280070, 0x2806B0,
        0x280FA0, 0x2816D0, 0x281FE0, 0x282300, 0x2834B0, 0x284A00, 0x2861A0, 0x286380,
        0x286D30, 0x286F10, 0x2875F0, 0x287860, 0x287E00, 0x287E20, 0x288230, 0x288540,
        0x288AB0, 0x288BD0, 0x288E20, 0x289120, 0x289290, 0x289410, 0x2895A0, 0x289F30,
        0x28A0A0, 0x28A290, 0x28A650, 0x28A9D0, 0x28B610, 0x28B730, 0x28B780, 0x28BA60,
        0x28C1C0, 0x28C640, 0x28CA40, 0x28DD50, 0x293730, 0x2998E0, 0x29CE00, 0x29FE00,
        0x29FF40, 0x2A0D10, 0x2A0E20, 0x2A0ED0, 0x2A1010, 0x2A14B0, 0x2A16D0, 0x2A17B0,
        0x2A29B0, 0x2B1120, 0x2B11B0, 0x2B18E0, 0x2B1A90, 0x2B1E90, 0x2B2050, 0x2B2750,
        0x2B4060, 0x2B4260, 0x2B4510, 0x2B55D0, 0x2B5B90, 0x2B6170, 0x2B66A0, 0x2B68B0,
        0x2BA300, 0x2A1F30, 0x2A4120, 0x2A0500,
        # state_lane.rs (T2082): c2r gamestate_wired writers of the GameState cells
        0x65230, 0x653B0, 0x65420, 0x65470, 0x658D0, 0x65C70, 0x65F40, 0x667F0,
        0x668A0, 0x66FC0, 0x670C0, 0x674C0, 0x67450, 0x67E60, 0x67F00, 0x68720,
        0x15DF20, 0x15E4F0, 0x22CE60, 0x23EFE0, 0x25F830,
        # leaf_lane.rs (T2091): c2r leaf_wired writers of SaveSlots, TextBank, ModeSetup
        0x699D0, 0x46030, 0x6A850, 0x69F30, 0x23F390,
        # leaf_lane.rs (T2114): PadState/AudioRamp writers
        0x42640, 0x450E0, 0x45160,
        # leaf_lane.rs (T2114): Missions writers
        0x9CE00, 0x9A140, 0x9A260, 0x9A630,
        # leaf_lane.rs (T2118): weapon slot and controller writers of the player record
        0x1C4400, 0x1C3100, 0x1BF410, 0x1BC890, 0x1C2E80,
        # leaf_lane.rs (T2137): player2_wired roots
        0x1B7680, 0x1BDFD0, 0x1BD820, 0x1BDAF0, 0x219CA0,
        # leaf_lane.rs (T2137): leaf3_wired roots
        0x425E0, 0x45970, 0x1BC650,
        # leaf_lane.rs (T2137 pass 3): leaf4_wired and player3_wired roots
        0x9A7A0, 0x9A5B0, 0x9AF90, 0x43C50, 0x88950, 0xCF510, 0x151E80,
        0x1BAEE0, 0x153CD0, 0x1B9DC0, 0x1B7C50, 0x1B7BC0, 0x1C0200,
        # leaf_lane.rs (T2122): object pool, character and object record writers
        0x9E4B0, 0xA1EA0, 0xA0130, 0xA5C00, 0xA5AF0, 0x15EAE0, 0x1BE870, 0x27EE70,
        0x15AB80, 0x15D500, 0xA7670, 0x9EAB0,
        # leaf_lane.rs (T2126): the next object and character writers
        0xA2B50, 0x9FF10, 0x167780, 0x15E830, 0x1260C0, 0x1099E0, 0x15C6C0, 0xD1CB0,
        0x5C5B0, 0xE1200, 0xA17C0, 0xA5F40, 0xA0860, 0xA7870, 0x15C9D0, 0x57880,
        0x15BB50, 0x15CD00,
        # leaf_lane.rs (T2129): untranslated writers retried, matrix helpers, duplicate translations
        0x3C8960, 0xE8ED0, 0x87310, 0x5F040, 0xC8120, 0x867F0, 0x94A00, 0xA79B0, 0xE97A0, 0x1B6FF0,
        0x164C40, 0x158D40, 0xA2900, 0x261C70, 0x153440, 0x1531D0, 0x153270, 0x151650, 0x153240,
        0x5CEF0, 0x5F430, 0xC4700, 0xD8E30, 0x170500, 0xC5310,
        # physics_lane.rs (T2099): c2r engine/physics translations
        0x919D0, 0xA2CD0, 0x47D50, 0x91960, 0xC38F0, 0xA7A00, 0xC3B10, 0xA46A0,
        0x475C0, 0x91390, 0x48590, 0x49370, 0xD6A70, 0xC3C50, 0x1665E0, 0x1665F0,
        0xC3E60, 0x167CA0, 0x16B8C0, 0x477B0, 0x48350, 0x484D0, 0x49C60, 0x4A270,
        0x4A800, 0x4A9D0, 0x4AE00, 0x4B310, 0x4B3B0, 0x4FE00, 0x52550, 0x913B0,
        0x913D0, 0x92360, 0xA3050, 0xA3140, 0xA3500, 0x128760, 0x128C60, 0x128EF0,
        0x129CA0, 0x129F50, 0x166B60, 0x16B130, 0x16B550, 0x16B590, 0x16B800, 0x16B850,
        0x16B870, 0x16B8A0, 0x16B900,
        # physics_lane2.rs (T2117): the heavy mesh ray leaves
        0x47B30, 0x47BD0, 0x913F0,
        # cutscene_lane.rs (T2131): cutscene and save_unlock roots
        0x3EE20, 0x58240, 0x60390, 0x60410, 0x61680, 0x64D90, 0x64DD0, 0x665B0, 0x6EE00,
        0x6F430, 0x6F4B0, 0x8E970, 0x94220, 0x98DA0, 0x98DB0, 0x23EFC0, 0x1CD3E0, 0x1CD940,
        0x1CD040, 0x1CD980, 0x2BE5E0, 0x1CD9D0, 0x1CD9F0, 0x1CDA20, 0x1CD890,
        # mapmaker_ui_lane.rs (T2142): mapmaker/ui roots
        0x70E30, 0x81480, 0x81C10, 0x81C40, 0x2FA030, 0x2FE450, 0x308B10, 0x308C00,
        0x3091C0, 0x3092E0, 0x3096A0, 0x309A80, 0x30AEA0, 0x30B150, 0x30B280, 0x30B3A0,
        0x30B480, 0x30B560, 0x30B7C0, 0x30B9B0, 0x30BA90, 0x30BC50, 0x30BE60, 0x30C210,
        0x30C3C0, 0x30C440, 0x30CB30, 0x30CB90, 0x30CCD0, 0x30CF60, 0x30D120, 0x30D300,
        0x30D410, 0x30D7B0, 0x30DD40, 0x310100, 0x310310, 0x3108D0, 0x310A20, 0x312270,
        0x314420, 0x314F80, 0x314FA0, 0x315D20, 0x315D60, 0x315DB0, 0x315E10, 0x316110,
        0x316B50, 0x3179D0, 0x317D70, 0x318350, 0x3188C0, 0x318BE0, 0x318EA0, 0x3190C0,
        0x3191F0, 0x3192E0, 0x319300, 0x319BD0, 0x319DA0, 0x319E60, 0x31AB80, 0x31AD90,
        0x31B6B0, 0x31B7D0, 0x31B8B0, 0x31C7C0, 0x31CB90, 0x31D140, 0x31D160, 0x31D180,
        0x31D3E0, 0x31D580, 0x31DC10, 0x31EAB0, 0x31EC90, 0x31EE30, 0x31FDC0, 0x320250,
        0x320580, 0x321F20, 0x3223A0, 0x322BB0, 0x3236C0, 0x323C30, 0x3244E0, 0x3246A0,
        0x324C00, 0x3266B0, 0x3267C0, 0x3268C0, 0x3276F0, 0x327A70, 0x327F90, 0x328D20,
        0x329DE0, 0x32CE00, 0x32D9B0, 0x32DBA0, 0x331C70, 0x334730, 0x3368B0, 0x3368E0,
        0x336910, 0x336950, 0x336D30, 0x336D60, 0x336D90, 0x336DC0, 0x336E60, 0x336F60,
        0x336FB0, 0x337ED0,
        # weapons_lane2.rs (T2148): gameplay/weapons roots with a T1925 Tier 1 translation
        0x12F30, 0x12F50, 0x12F70, 0x6BEC0, 0x9E750, 0xAE790, 0xAE830, 0xB3E40,
        0xB4470, 0xB4680, 0xB4B60, 0xB4DF0, 0xB4F90, 0xB6090, 0xB6140, 0xB6430,
        0xB74C0, 0xB9270, 0xB9340, 0xB93E0, 0xB9430, 0xB9510, 0xB9720, 0xB9790,
        0xB9920, 0xB9960, 0xB99D0, 0xB9A30, 0xB9A90, 0xB9DC0, 0xBA310, 0xBA8A0,
        0xBB090, 0xBB6C0, 0xBC140, 0xBC1B0, 0xBC210, 0xBC310, 0xBC4D0, 0xBC520,
        0xBCB00, 0xBCD80, 0xBD190, 0xBD650, 0xBDBD0, 0xBE180, 0xBE990, 0xBEE60,
        0xBEF00, 0xBF080, 0xBF360, 0xBF520, 0xBF620, 0xBF740, 0xBFA30, 0xBFDF0,
        0xC0450, 0xC0540, 0xC1360, 0xC1950, 0xC2090, 0xC2D40, 0xDC270, 0xE0CE0,
        0xE28B0, 0xE9690, 0xEA9C0, 0xEAAF0, 0xF7070, 0xF88C0, 0x10E9A0, 0x142880,
        0x142900, 0x142990, 0x15DB20, 0x17BB10, 0x183040, 0x18B530, 0x1B56A0, 0x1B5C70,
        0x1B6350, 0x1B7E80, 0x1B8160, 0x1B81B0, 0x1B8C70, 0x1B90A0, 0x1B9F90, 0x1BA020,
        0x1BA300, 0x1BB030, 0x1BB2F0, 0x1BB480, 0x1BC990, 0x1BD020, 0x1BD390, 0x1BD440,
        0x1BD4F0, 0x1BD600, 0x1BDD20, 0x1BDE40, 0x1BF000, 0x1BF140, 0x1BF320, 0x1C0110,
        0x1C1C00, 0x1C2C00, 0x1C5E10, 0x216190, 0x34FF30,
        # text_lane.rs (T2157): data/text roots
        0x3D7B0, 0x3FB90, 0x461C0, 0x150F60, 0x27250, 0x151160, 0x40650, 0x63C80,
        0x63E70, 0x3DD20, 0x3DB80, 0x3DE30, 0x46170, 0x150FE0, 0x31C50, 0x32860,
        0x37330, 0x38200, 0x3FC10, 0x406A0, 0x66470, 0x7C300, 0x7C390, 0x82860,
        0x828C0, 0x828F0, 0x82920, 0x195810, 0x1B2B80, 0x2BE6F0, 0x2C7750, 0x2C7780,
        0x2D1370, 0x2D14C0, 0x2D1770, 0x311820,
        # input_lane2.rs (T1995): engine/input roots with a Tier 1 translation
        0x190D50, 0x43220, 0x42720, 0x42BF0, 0x2586C0, 0x2588B0, 0x2FB00, 0x31FC0,
        0x373F0, 0x42A70, 0x42AE0, 0x196460, 0x196570, 0x2E7DF0, 0x32B400
    }
)  # fmt: skip
INV_ALL_META: dict[int, dict[str, Any]] = {
    int(row["va"], 16): row
    for row in json.loads(Path(__file__).with_name("invpose_roots.json").read_text())["roots"]
    if int(row["va"], 16) not in _INV_ALREADY
}
# the 305 host-selectable roots (129 T1874 + 25 T1905/T1918 animation adapters + 24 T1923
# gameplay adapters + 3 T1971 npc ticks + later lanes + 73 T1976 render roots), in the order of
# crates/tsfp-ports/src/lane_inv.rs
INV_HOST_ROOTS = tuple(INV_ALL_META)
INV_META = {
    va: row
    for va, row in INV_ALL_META.items()
    if va not in INV_AUDIT_REJECTED and va not in INV_ANIM_LANE
}
INV_ROOTS = tuple(INV_META)
ROOTS = (
    *_legacy_roots("HOST_ROOTS"),
    *_legacy_roots("HARNESS_EXTRA_ROOTS"),
    # T1854: wave 4 and 4b roots, admitted through the register-file ("lane") bridge.
    *ALL_LANE_ROOTS,
    # T1874: inventory and pose/control roots (register-file lane over the ExtRegisters ports).
    *INV_ROOTS,
)
ALL_LANE = (*ALL_LANE_ROOTS, *INV_ROOTS)
INV_SOURCE = "rust_invpose.c"
# Callback of the lane ports' nested guest calls: the same lifted callee lookup as the inline
# `game_guest_call`; the Rust port has already written the call frame (return PC, arguments).
LANE_GUEST_CALL = """
void tsfp_lane_guest_call(uint32_t target, uint32_t *r)
{
    game_guest_function function = recomp_lookup ? recomp_lookup(target) : NULL;
    if (function == NULL)
        __builtin_trap();
    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3];
    g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];
    function();
    r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx;
    r[4] = g_esi; r[5] = g_edi; r[6] = g_ebp; r[7] = g_esp;
}
"""
# T1874: x87/XMM marshalling and the guest call of the ExtRegisters ports (same behaviour as
# src/host/rust_ext_bridge.c `rust_ext_load/store/guest_call`, over the harness globals).
INV_C_TYPES = """
#include <string.h>
typedef struct { double stack[8]; uint32_t top; uint16_t control; } rust_x_fpu;
typedef struct { uint32_t lanes[4]; } rust_x_xmm;
typedef struct {
    uint32_t eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags;
    rust_x_fpu fpu;
    rust_x_xmm xmm[5];
    rust_x_xmm xmm_hi[3];
} rust_x_registers;
_Static_assert(sizeof(rust_x_registers) == 240u, "ExtRegisters layout");
extern __thread double g_fp_stack[8];
extern __thread int g_fp_top;
extern __thread int g_df;
extern __thread uint16_t g_fp_control_word;
extern __thread rust_x_xmm g_xmm0, g_xmm1, g_xmm2, g_xmm3, g_xmm4, g_xmm5, g_xmm6, g_xmm7;
void tsfp_lane_x_fp_load(rust_x_registers *r);
void tsfp_lane_x_fp_store(const rust_x_registers *r);
void tsfp_lane_x_call(uint32_t va, rust_x_registers *r);
"""
INV_C_BODY = """
#include <stdlib.h>
uint32_t tsfp_lane_mutation_target(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_VA");
    return text ? (uint32_t)strtoul(text, NULL, 16) : 0u;
}
void tsfp_lane_x_fp_load(rust_x_registers *r)
{
    memcpy(r->fpu.stack, g_fp_stack, sizeof g_fp_stack);
    r->eflags = g_df ? 0x400u : 0u; /* DF steers the rep string forms */
    r->fpu.top = (uint32_t)g_fp_top & 7u; r->fpu.control = g_fp_control_word;
    memcpy(&r->xmm[0], &g_xmm0, 16); memcpy(&r->xmm[1], &g_xmm1, 16);
    memcpy(&r->xmm[2], &g_xmm2, 16); memcpy(&r->xmm[3], &g_xmm3, 16);
    memcpy(&r->xmm[4], &g_xmm4, 16);
    memcpy(&r->xmm_hi[0], &g_xmm5, 16); memcpy(&r->xmm_hi[1], &g_xmm6, 16);
    memcpy(&r->xmm_hi[2], &g_xmm7, 16);
}
void tsfp_lane_x_fp_store(const rust_x_registers *r)
{
    memcpy(g_fp_stack, r->fpu.stack, sizeof g_fp_stack);
    g_fp_top = (int)r->fpu.top; g_fp_control_word = r->fpu.control;
    memcpy(&g_xmm0, &r->xmm[0], 16); memcpy(&g_xmm1, &r->xmm[1], 16);
    memcpy(&g_xmm2, &r->xmm[2], 16); memcpy(&g_xmm3, &r->xmm[3], 16);
    memcpy(&g_xmm4, &r->xmm[4], 16);
    memcpy(&g_xmm5, &r->xmm_hi[0], 16); memcpy(&g_xmm6, &r->xmm_hi[1], 16);
    memcpy(&g_xmm7, &r->xmm_hi[2], 16);
}
void tsfp_lane_x_call(uint32_t va, rust_x_registers *r)
{
    game_guest_function function = recomp_lookup ? recomp_lookup(va) : NULL;
    if (function == NULL)
        __builtin_trap();
    g_eax = r->eax; g_ecx = r->ecx; g_edx = r->edx; g_esp = r->esp;
    g_ebx = r->ebx; g_ebp = r->ebp; g_esi = r->esi; g_edi = r->edi;
    tsfp_lane_x_fp_store(r);
    g_esp -= 4u; /* return slot, the lifted RET does esp += 4 */
    *(uint32_t *)((uintptr_t)g_xbox_mem_offset + g_esp) = 0u;
    const uint32_t entry_esp = g_esp;
    function();
    if (g_esp != entry_esp + 4u)
        __builtin_trap();
    r->eax = g_eax; r->ecx = g_ecx; r->edx = g_edx; r->esp = g_esp;
    r->ebx = g_ebx; r->ebp = g_ebp; r->esi = g_esi; r->edi = g_edi;
    tsfp_lane_x_fp_load(r);
}
"""
REPO_ROOT = Path(__file__).resolve().parents[2]


def pinned_channel(root: Path = REPO_ROOT) -> str:
    """The one Rust toolchain pin (T3472): `channel` in the repo's rust-toolchain.toml.

    Every rustc check resolves its version here instead of hardcoding one; a stale
    hardcoded 1.99.0 segfaulted on tsfp-ports while the pin had moved to 1.99.1 (sup-segv).
    """
    import tomllib

    with (root / "rust-toolchain.toml").open("rb") as handle:
        channel = tomllib.load(handle).get("toolchain", {}).get("channel")
    if not isinstance(channel, str) or not channel:
        raise BuildError(f"{root / 'rust-toolchain.toml'} has no toolchain.channel")
    return channel


def matches_pin(version_output: str, channel: str) -> bool:
    """True when `rustc --version` output is exactly the pinned channel's release."""
    return version_output.strip().startswith(f"rustc {channel} ")


TARGET = "x86_64-unknown-linux-gnu"
MARKER = "rust-lane.json"


def compiler() -> Path:
    candidate = shutil.which("rustc")
    if candidate is None:
        raise BuildError("Rust lane requires pinned rustc on PATH; see docs/t-rust-lane.md")
    # Resolve rustup proxy to the installed compiler; no automatic download path.
    executable = Path(run(["rustup", "which", "rustc"]).strip()).resolve()
    channel = pinned_channel()
    if not matches_pin(run([str(executable), "--version"]), channel):
        raise BuildError(f"Rust lane requires rustc {channel} (rust-toolchain.toml)")
    return executable


def identity(crate: Path, rustc: Path, mutation: bool) -> dict[str, Any]:
    files = [
        crate / "Cargo.toml",
        *sorted((crate / "src").rglob("*.rs")),
        crate.parent / "tsfp-engine-core/src/leaf.rs",
        *sorted((crate.parent / "tsfp-engine-memory/src").rglob("*.rs")),
        *sorted((crate.parent / "tsfp-engine-object/src").rglob("*.rs")),
        crate.parent / "tsfp-engine-anim/src/leaf.rs",
        crate.parent / "tsfp-engine-physics/src/leaf.rs",
        crate.parent / "tsfp-engine-audio/src/leaves.rs",
        crate.parent / "tsfp-engine-input/src/leaves.rs",
        crate.parent / "tsfp-engine-input/src/calls.rs",
        crate.parent / "tsfp-engine-render/src/leaves.rs",
        crate.parent / "tsfp-network/src/leaves.rs",
        crate.parent / "tsfp-network/src/calls.rs",
        crate.parent / "tsfp-network/src/fesl.rs",
        crate.parent / "tsfp-network/src/callee.rs",
    ]
    return {
        "schema": 1,
        "crate": str(crate.resolve()),
        "rustc": str(rustc),
        "compiler_sha256": hashlib.sha256(rustc.read_bytes()).hexdigest(),
        "version": f"rustc {pinned_channel()}",
        "target": TARGET,
        "mutation": mutation,
        "flags": [
            "--edition=2024",
            "-Cpanic=abort",
            "-Coverflow-checks=off",
            "-Crelocation-model=pic",
        ],
        "sources": {
            str(p.relative_to(crate.parent)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in files
        },
    }


def inv_registration(va: int) -> Registration:
    """Synthetic exact registration of a T1850 root that has no C replacement."""
    meta = INV_ALL_META[va]
    pops = meta["ret_pops"]
    stack_args = min(max(int(meta["stack_args"]), max(pops, default=0) // 4), 8)
    convention = "stdcall" if pops and pops[0] > 0 else "cdecl"
    return Registration(
        va=va,
        convention=convention,
        stack_args=stack_args,
        returns="u32",
        function=f"tsfp_inv_{va:08x}",
        source=INV_SOURCE,
        exact_registers=True,
    )


def prepare(game: Path, destination: Path, *, mutation: bool = False) -> Path:
    crate = Path(__file__).resolve().parents[2] / "crates/tsfp-ports"
    record = identity(crate, compiler(), mutation)
    selected = [r for r in scan_directory(game) if r.va in ROOTS and r.va not in INV_META]
    selected += [inv_registration(va) for va in INV_ROOTS]
    if {r.va for r in selected} != set(ROOTS):
        raise BuildError("Rust pilot requires every original admitted registry entry")
    destination.mkdir(parents=True, exist_ok=True)
    # Preserve originals' headers, registries, convention, returns and scratch policy.
    for p in [*game.glob("*.h"), game / "game_registry.c", game / "game_manifest.c"]:
        shutil.copyfile(p, destination / p.name)
    header = "/* SPDX-License-Identifier: GPL-3.0-or-later */\n"
    header += "/* Rust implementation provenance: " + json.dumps(record, sort_keys=True) + " */\n"
    header += "typedef struct { uint32_t eax, ecx, edx, esp; } rust_registers;\n"
    header += '_Static_assert(sizeof(rust_registers) == 16u, "Rust Registers ABI");\n'
    header += (
        "typedef struct { uint32_t eax, ecx, edx, ebx, esi, edi, ebp, esp; } rust_cpu;\n"
        '_Static_assert(sizeof(rust_cpu) == 32u, "Rust lane register file ABI");\n'
        "extern __thread uint32_t g_ebx, g_edi, g_ebp;\n"
        "void tsfp_lane_guest_call(uint32_t target, uint32_t *regs);\n"
    )
    header += INV_C_TYPES
    for r in selected:
        if r.va in ALL_LANE:
            header += f"extern void tsfp_lane_{r.va:08x}(rust_cpu *, uintptr_t);\n"
        else:
            header += f"extern void tsfp_port_{r.va:08x}(rust_registers *, uintptr_t);\n"
    (destination / "rust_lane.h").write_text(header)
    by_source: dict[str, str] = {}
    for r in selected:
        text = by_source.setdefault(r.source, '#include "game_replace.h"\n#include "rust_lane.h"\n')
        if r.va in ALL_LANE:
            call = "rust_cpu state = {g_eax, g_ecx, g_edx, g_ebx, g_esi, g_edi, g_ebp, g_esp};\n"
            call += f"    tsfp_lane_{r.va:08x}(&state, (uintptr_t)g_xbox_mem_offset);\n"
            call += "    g_eax = state.eax; g_ecx = state.ecx; g_edx = state.edx;\n"
            call += (
                "    g_ebx = state.ebx; g_esi = state.esi; g_edi = state.edi; g_ebp = state.ebp;\n"
            )
            call += "    g_esp = state.esp;\n"
        else:
            call = "rust_registers state = {g_eax, g_ecx, g_edx, g_esp};\n"
            call += f"    tsfp_port_{r.va:08x}(&state, (uintptr_t)g_xbox_mem_offset);\n"
            call += "    g_eax = state.eax; g_ecx = state.ecx; g_edx = state.edx;\n"
        if r.exact_registers:
            macro = "GAME_REPLACE_EXACT_INPUTS" if r.register_inputs else "GAME_REPLACE_EXACT"
            inputs = f"{'_'.join(r.register_inputs)}, " if r.register_inputs else ""
            text += (
                f"{macro}({r.va:08X}, {r.convention}, {r.stack_args}, "
                f"{r.returns}, {inputs}{r.function})\n{{\n    {call}}}\n"
            )
        else:
            params = ", ".join(f"uint32_t a{i}" for i in range(r.stack_args)) or "void"
            unused = " ".join(f"(void)a{i};" for i in range(r.stack_args))
            result = "" if r.returns == "void" else "    return state.eax;\n"
            kind = "void" if r.returns == "void" else "uint32_t"
            text += (
                f"static {kind} {r.function}({params})\n{{\n    {unused}\n    {call}{result}}}\n"
            )
            text += (
                f"GAME_REPLACE({r.va:08X}, {r.convention}, {r.stack_args}, "
                f"{r.returns}, {r.function})\n"
            )
        by_source[r.source] = text
    guest_call_host = next(iter(by_source), None)
    if any(r.va in ALL_LANE for r in selected) and guest_call_host is not None:
        by_source[guest_call_host] += LANE_GUEST_CALL + INV_C_BODY
    for name, content in by_source.items():
        (destination / name).write_text(
            "/* SPDX-License-Identifier: GPL-3.0-or-later */\n" + content
        )
    (destination / MARKER).write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    return destination


def compile_library(game: Path, output: Path, opt_level: str) -> Path | None:
    marker = game / MARKER
    if not marker.exists():
        return None
    record = json.loads(marker.read_text())
    crate = Path(record["crate"])
    rustc = compiler()
    if identity(crate, rustc, record["mutation"]) != record:
        raise BuildError("Rust source/compiler drift: regenerate the opt-in shim tree")
    # Fingerprint header must match the build input marker as well.
    if json.dumps(record, sort_keys=True) not in (game / "rust_lane.h").read_text():
        raise BuildError("Rust lane provenance header drift")
    if opt_level not in ("-O0", "-O3"):
        raise BuildError("Rust lane supports only the fixed O0/O3 matrix")
    output.mkdir(parents=True, exist_ok=True)
    library = output / "libtsfp_ports.a"
    command = [
        str(rustc),
        str(crate / "src/lib.rs"),
        "--crate-name",
        "tsfp_ports",
        "--crate-type",
        "staticlib",
        "--target",
        record["target"],
        *record["flags"],
        f"-Copt-level={opt_level[-1]}",
        "-o",
        str(library),
    ]
    if record["mutation"]:
        command += ["--cfg", "rust_lane_mutation"]
    run(command)
    (output / "rust-build.json").write_text(
        json.dumps(
            {
                **record,
                "command": command,
                "archive_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
            },
            indent=2,
        )
        + "\n"
    )
    return library


def prepare_control(game: Path, destination: Path) -> Path:
    """Retain the same selected C registrations for an original-vs-C control.

    Unselected exact bodies are removed with balanced braces from comment-stripped
    source. Readable static helpers remain unchanged. Production sources stay intact.
    """
    from .scan import REGISTRATION, strip_comments

    destination.mkdir(parents=True, exist_ok=True)
    for p in [*game.glob("*.h"), game / "game_registry.c", game / "game_manifest.c"]:
        shutil.copyfile(p, destination / p.name)
    sources = {r.source for r in scan_directory(game) if r.va in ROOTS}
    for source in sorted(sources):
        text = (game / source).read_text()
        clean = strip_comments(text)
        removals = []
        for match in REGISTRATION.finditer(clean):
            if int(match["va"], 16) in ROOTS:
                continue
            end = match.end()
            if match["mode"]:
                start = clean.index("{", end)
                depth = 1
                end = start + 1
                while depth:
                    depth += (clean[end] == "{") - (clean[end] == "}")
                    end += 1
            removals.append((match.start(), end))
        for start, end in reversed(removals):
            text = text[:start] + text[end:]
        (destination / source).write_text(
            '#pragma GCC diagnostic ignored "-Wunused-function"\n' + text
        )
    # the T1874 inventory and pose roots have no C replacement unless src/game registers one
    synthetic = {va for va in INV_ROOTS if va not in {r.va for r in scan_directory(game)}}
    if {r.va for r in scan_directory(destination)} != set(ROOTS) - synthetic:
        raise BuildError("C control does not contain exactly the Rust roots")
    return destination
