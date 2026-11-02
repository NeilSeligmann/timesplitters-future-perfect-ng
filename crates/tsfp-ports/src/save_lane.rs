// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-object/synth_leaves
// portreg: wraps=tsfp-gameplay/save_unlock
//! T1923: lane adapters (Tier 2 through `tools.rust_trace`) for the T1899 data/save ports that the Story route reaches (profile
//! slot table accessors 0x69630..0x69930 and the unlock bit tests 0x1CD2B0 / 0x1CD330 / 0x1CD380). The pure ports live in
//! `tsfp-gameplay` (`save_profile.rs`, `save_unlock.rs`, compiled here by `#[path]`, Tier 1 against the lifted original in
//! `crates/tsfp-c-diff` `save_t1.rs`). `ProfileHost` / `UnlockHost` are implemented over guest memory plus the retained lifted
//! originals (`gp_bridge::HostBridge`). Registers other than EAX are left as entered (the originals clobber ECX / EDX scratch).
//! Not adapted: `save_device.rs` (uses `Vec`, not `no_std`), the mapmaker storage / content ports (their `tsfp_gameplay::memory`
//! import is not includable here) and the Tier 0 unlock ports.
use core::cell::RefCell;

use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::HostBridge,
    memory::{GuestMemory, MappedGuest},
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/save_profile.rs"]
mod save_profile;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/save_unlock.rs"]
mod save_unlock;

use save_profile::ProfileHost;
use save_unlock::UnlockHost;

impl<M: GuestMemory, G: GuestCall> ProfileHost for HostBridge<'_, M, G> {
    fn net_is_session_active(&mut self) -> bool {
        self.call_cdecl(0x3533f0, &[]) != 0
    }
    fn local_slot_index(&mut self, player: u32) -> u32 {
        self.call_cdecl(0x356800, &[player])
    }
    fn local_time_encode(&mut self) -> u32 {
        self.call_cdecl(0x150230, &[])
    }
    fn string_copy(&mut self, _mem: &mut dyn GuestMemory, dst: u32, src: u32, len: u32) {
        self.call_cdecl(0x3c8960, &[dst, src, len]);
    }
}

impl<M: GuestMemory, G: GuestCall> UnlockHost for HostBridge<'_, M, G> {
    fn fn_00069630_profile_block(&mut self) -> u32 {
        self.call_cdecl(0x69630, &[])
    }
    fn fn_00069900_stats_block(&mut self, index: u32) -> u32 {
        self.call_cdecl(0x69900, &[index])
    }
    fn fn_000696d0_local_slot_active(&mut self) -> u32 {
        let slot = self.call_cdecl(0x356870, &[0]);
        self.call_cdecl(0x696d0, &[slot])
    }
    fn fn_001cceb0_override_byte(&mut self, category: u32) -> u32 {
        self.call_cdecl(0x1cceb0, &[category])
    }
    fn fn_0022e070_record38(&mut self, index: u32) -> u32 {
        self.call_cdecl(0x22e070, &[index])
    }
    fn fn_0022e050_record_table(&mut self, index: u32) -> u32 {
        self.call_cdecl(0x22e050, &[index])
    }
    fn fn_002be590_find_record30(&mut self, key_a: u32, key_b: u32) -> u32 {
        self.call_cdecl(0x2be590, &[key_a, key_b])
    }
    fn fn_0003d7b0_text_by_id(&mut self, id: u32) -> u32 {
        self.call_cdecl(0x3d7b0, &[id])
    }
    fn fn_0003fb90_string_copy_case_convert(&mut self, source: u32) -> u32 {
        self.call_cdecl(0x3fb90, &[source])
    }
    fn fn_0003d7f0_sprintf(&mut self, dest: u32, format: u32, arg_a: u32, arg_b: u32) -> u32 {
        self.call_cdecl(0x3d7f0, &[dest, format, arg_a, arg_b])
    }
    fn fn_00045390_text_context_init(&mut self, buffer: u32) {
        self.call_cdecl(0x45390, &[buffer]);
    }
    fn fn_00045820_text_show(&mut self, context: u32) {
        self.call_cdecl(0x45820, &[context]);
    }
    fn fn_00151030_format_text(&mut self, a: u32, v: u32) -> u32 {
        self.call_cdecl(0x151030, &[a, v])
    }
    fn fn_003c8960_widget_set_text(&mut self, widget: u32, text: u32, len: u32) {
        self.call_cdecl(0x3c8960, &[widget, text, len]);
    }
}

fn arg<M: GuestMemory>(m: &M, regs: &ExtRegisters, index: u32) -> u32 {
    m.read32(regs.esp.wrapping_add(4).wrapping_add(4 * index))
}

/// Dispatches a root; `false` when `va` is not one of the save adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if UNLOCK_ROOTS.contains(&va) {
        // T2131: a null or wild descriptor is refused, the footprint rolled back and the retained original runs instead.
        return crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
            run_unlock(va, view, guest, regs)
        });
    }
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry = *regs;
    let args = [
        arg(m, &entry, 0),
        arg(m, &entry, 1),
        arg(m, &entry, 2),
        arg(m, &entry, 3),
    ];
    let cell = RefCell::new(&mut *m);
    let mut host = HostBridge::new(&cell, guest, &entry);
    let mut mem = host.memory();
    let eax = match va {
        0x69630 => Some(save_profile::profile_ptr(&mem)),
        0x69640 => Some(save_profile::record_base_for_local_slot_zero(&mut host)),
        0x696a0 => {
            // The seconds value is a global, not an argument.
            let seconds = host.read32(0x007497b0) as i32;
            Some(save_profile::pack_local_time(seconds, &mut host))
        }
        0x696d0 => Some(u32::from(save_profile::slot_is_active(&mem, args[0]))),
        0x69720 => Some(save_profile::slot_entry_ptr_plus_4(&mem, args[0] as i32)),
        // A null slot record faults in the original; the port reports the fault and the lane leaves memory unchanged.
        0x697d0 => {
            let _ = save_profile::slot_set_fields(&mut mem, args[0] as i32, args[1], args[2]);
            None
        }
        0x69900 => Some(save_profile::stats_record_ptr(&mem, args[0])),
        0x69930 => Some(save_profile::stats_entry_ptr(&mem, args[0], args[1])),
        0x1cd2b0 => Some(save_unlock::profile_bit_set_for_difficulty(
            &mem,
            &mut host,
            args[0] as i32,
            args[1] as i32,
        )),
        0x1cd330 => Some(save_unlock::profile_all_bits_set_for_record38(
            &mem,
            &mut host,
            args[0],
            args[1] as i32,
        )),
        0x1cd380 => Some(save_unlock::profile_bit_set_selector_block(
            &mem, &mut host, args[0], args[1], args[2], args[3],
        )),
        _ => return false,
    };
    if let Some(value) = eax {
        regs.eax = value;
    }
    true
}

/// The data/save lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 20] = [
    0x69630, 0x69640, 0x696a0, 0x696d0, 0x69720, 0x697d0, 0x69900, 0x69930, 0x1cd2b0, 0x1cd330,
    0x1cd380,
    // T2131: the nine unlock requirement roots, dispatched by `run_unlock` (their entry points are exported below)
    0x1cd3e0, 0x1cd940, 0x1cd040, 0x1cd980, 0x2be5e0, 0x1cd9d0, 0x1cd9f0, 0x1cda20, 0x1cd890,
];

/// T2131: the nine unlock requirement roots (their entry points are exported below; `lane_inv::INV_ROOTS` lists them after the cutscene roots).
pub const UNLOCK_ROOTS: [u32; 9] = [
    0x1cd3e0, 0x1cd940, 0x1cd040, 0x1cd980, 0x2be5e0, 0x1cd9d0, 0x1cd9f0, 0x1cda20, 0x1cd890,
];

fn run_unlock<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry = *regs;
    let args = [arg(m, &entry, 0), arg(m, &entry, 1), arg(m, &entry, 2)];
    let cell = RefCell::new(&mut *m);
    let mut host = HostBridge::new(&cell, guest, &entry);
    let mut mem = host.memory();
    let eax = match va {
        0x1cd3e0 => save_unlock::unlock_requirement_met(&mem, &mut host, args[0]),
        0x1cd940 => save_unlock::requirement_met_table_4fcc88(&mem, &mut host, args[0]),
        0x1cd040 => save_unlock::unlock_bitmask_of_met_entries(&mem, &mut host),
        0x1cd980 => {
            save_unlock::requirement_met_record30_payload(&mem, &mut host, args[0], args[1])
        }
        0x2be5e0 => save_unlock::find_record30_payload(&mut host, args[0], args[1]),
        0x1cd9d0 => save_unlock::requirement_met_weapon_slot(&mem, &mut host, args[0]),
        0x1cd9f0 => save_unlock::requirement_met_table_4d23ac(&mem, &mut host, args[0]),
        0x1cda20 => save_unlock::requirement_met_record38_entry(&mem, &mut host, args[0]),
        // The original builds the type 2 descriptor in its own frame (20 bytes below the entry ESP).
        0x1cd890 => save_unlock::unlock_requirement_met_type2_override_1(
            &mut mem,
            &mut host,
            args[0],
            args[2],
            entry.esp.wrapping_sub(0x100),
        ),
        _ => return false,
    };
    regs.eax = eax;
    true
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut crate::lane::LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            crate::lane_inv::execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_001cd3e0, 0x1cd3e0);
export!(tsfp_lane_001cd940, 0x1cd940);
export!(tsfp_lane_001cd040, 0x1cd040);
export!(tsfp_lane_001cd980, 0x1cd980);
export!(tsfp_lane_002be5e0, 0x2be5e0);
export!(tsfp_lane_001cd9d0, 0x1cd9d0);
export!(tsfp_lane_001cd9f0, 0x1cd9f0);
export!(tsfp_lane_001cda20, 0x1cda20);
export!(tsfp_lane_001cd890, 0x1cd890);
