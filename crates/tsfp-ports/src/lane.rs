// SPDX-License-Identifier: GPL-3.0-or-later
// T1854: original-instruction lane bridge for the 44 wave 4 and 4b roots (docs/t1854-rust-admission.md).
// It is separate from `bridge.rs` and the host runtime: the lane hands the port the whole register
// file (callee-saved registers included, they are real `g_*` globals in the harness) and routes
// nested guest calls back to the lifted callee through `tsfp_lane_guest_call`, defined by the
// generated C shim. Harness-only, exactly like the T1835 roots: the host selector stays at 24.
use crate::{
    callee::{Callee, Cpu},
    gprs::Gprs,
    memory::{GuestMemory, MappedGuest},
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-audio/src/leaves.rs"]
mod audio;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-network/src/fesl.rs"]
mod fesl;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-input/src/leaves.rs"]
mod input;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-input/src/calls.rs"]
mod input_calls;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-network/src/leaves.rs"]
mod network;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-network/src/calls.rs"]
mod network_calls;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-object/src/lib.rs"]
mod object;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/leaves.rs"]
mod render;

/// Register file in the order of the C `rust_cpu` struct emitted by `tools/replace/rust_lane.py`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneCpu {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
}
const _: () = assert!(core::mem::size_of::<LaneCpu>() == 32);

unsafe extern "C" {
    /// Runs the lifted callee for `target`: `regs` is eax, ecx, edx, ebx, esi, edi, ebp, esp in
    /// and out. Defined once by the generated shim; aborts when the target has no function.
    fn tsfp_lane_guest_call(target: u32, regs: *mut u32);
}

struct LaneCallee;
impl Callee<MappedGuest> for LaneCallee {
    fn call(&mut self, _m: &mut MappedGuest, target: u32, cpu: &mut Cpu) {
        let mut r = [
            cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp, cpu.esp,
        ];
        unsafe { tsfp_lane_guest_call(target, r.as_mut_ptr()) };
        *cpu = Cpu {
            eax: r[0],
            ecx: r[1],
            edx: r[2],
            ebx: r[3],
            esi: r[4],
            edi: r[5],
            ebp: r[6],
            esp: r[7],
        };
    }
}

fn arg<M: GuestMemory>(m: &M, cpu: &LaneCpu, index: u32) -> u32 {
    m.read32(cpu.esp.wrapping_add(4).wrapping_add(index.wrapping_mul(4)))
}
fn entry(cpu: &LaneCpu) -> Gprs {
    Gprs {
        eax: cpu.eax,
        ecx: cpu.ecx,
        edx: cpu.edx,
    }
}
fn set(cpu: &mut LaneCpu, g: Gprs) {
    cpu.eax = g.eax;
    cpu.ecx = g.ecx;
    cpu.edx = g.edx;
}
fn to_cpu(c: &LaneCpu) -> Cpu {
    Cpu {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
        ebx: c.ebx,
        esi: c.esi,
        edi: c.edi,
        ebp: c.ebp,
        esp: c.esp,
    }
}
fn from_cpu(c: &mut LaneCpu, v: Cpu) {
    c.eax = v.eax;
    c.ecx = v.ecx;
    c.edx = v.edx;
    c.ebx = v.ebx;
    c.esi = v.esi;
    c.edi = v.edi;
    c.ebp = v.ebp;
    c.esp = v.esp;
}

/// T1835 wave 2b: object-tree leaves share the ObjectMemory trait; this adapter maps it onto the mapped guest.
struct ObjectGuest<'a>(&'a mut MappedGuest);
impl object::ObjectMemory for ObjectGuest<'_> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}
fn og(c: &LaneCpu) -> object::gprs::Gprs {
    object::gprs::Gprs {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
    }
}
fn set_og(c: &mut LaneCpu, g: object::gprs::Gprs) {
    c.eax = g.eax;
    c.ecx = g.ecx;
    c.edx = g.edx;
}

fn execute_object(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) -> bool {
    use object::object_leaves as o;
    let (esp, e) = (c.esp, og(c));
    let mut g = ObjectGuest(m);
    let r = match va {
        0x174e0 => o::mode_selected_flag_pair_intersects(&g, esp, e),
        0x71740 => o::set_field_0x24_and_flag_0x2c_bit0(&mut g, esp, e),
        0x264720 => o::ext_set_9d0_if_state_3(&mut g, esp, e),
        0x10ebd0 => o::is_field_0x50_equal_2(&g, esp, e),
        0x234270 => o::is_field_0x34_nonzero_null_safe(&g, esp, e),
        0x36cc10 => o::store_three_args_into_fields_0x24_0x28_0x2c(&mut g, esp, e),
        0xd7550 => o::set_field_0x84_and_0xa58_from_table(&mut g, esp, e),
        0x15bca0 => o::array_7a3580_append_field_0x7c(&mut g, esp, e),
        0x177e90 => o::child_fields_all_nonnegative(&g, esp, e),
        0x1b5840 => o::flag_0x40_or_kind_1_record_flag_20000_clear(&g, esp, e),
        0x1b6540 => o::stash_field_0x94_to_0x9c(&mut g, esp, e),
        0x1b6a10 => o::field_0x80_set_and_not_masked(&g, esp, e),
        0x242ee0 => o::type_entry_flag_mask_hit_or_kind_9_or_12(&g, esp, e),
        0x350d40 => o::type_table_dword_by_word_0x48(&g, esp, e),
        0x350e70 => o::type_table_flag_mask_test(&g, esp, e),
        0x37a6f0 => o::type_8_copy_vec3_from_ext_child(&mut g, esp, e),
        0x87840 => o::field_0x1f0_or_room_entry_field_0x7c(&g, esp, e),
        0x12ba0 => {
            let mut r = object::exact::Registers {
                eax: c.eax,
                ecx: c.ecx,
                edx: c.edx,
            };
            object::exact::indexed_field_pointer(&g, esp, &mut r);
            c.eax = r.eax;
            c.ecx = r.ecx;
            c.edx = r.edx;
            return true;
        }
        _ => return false,
    };
    set_og(c, r);
    if va == 0x15bca0 {
        // T2407: observe the explicit actor-list append after the typed leaf
        // has installed the extension and incremented the list length.
        let object = m.read32(esp.wrapping_add(4));
        if tsfp_ai_spawn_binding::published_binding_matches(m, object) {
            crate::rea_ai_runtime::actor_spawned(m, object);
        }
    }
    true
}

pub fn execute(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) {
    if execute_object(va, m, c) {
        mutate(va, m, c);
        return;
    }
    let e = entry(c);
    match va {
        0x45d60 => set(c, audio::global_mask_bit_test(m, arg(m, c, 0), e)),
        0x63bf0 => set(c, audio::pair_cursor_equal(m, arg(m, c, 0), e)),
        0xd76a0 => set(
            c,
            audio::table_special_fallback(m, arg(m, c, 0), arg(m, c, 1), e),
        ),
        0x26b70 => set(c, audio::clear_matching_key(m, e)),
        0x427b0 => set(c, input::pad_current_button_state(m, arg(m, c, 0), e)),
        0x42c30 => set(c, input::pad_get_field_0x10_direct(m, arg(m, c, 0), e)),
        0x431c0 => set(c, input::pad_map_is_bit_set(m, arg(m, c, 0))),
        0x246d10 => set(c, input::driven_object_is_null(m, e)),
        0x190d60 => set(c, input::count_local_players_with_pad_mapping(m, e)),
        0x43a90 => set(
            c,
            input::export_record(m, arg(m, c, 0), arg(m, c, 1), arg(m, c, 2), e),
        ),
        0x43af0 => set(
            c,
            input::import_record(m, arg(m, c, 0), arg(m, c, 1), arg(m, c, 2)),
        ),
        0x3443f0 => set(c, render::is_weather_pass_flag_set(m, e)),
        0x1e8d0 => set(c, render::vertex_format_size(arg(m, c, 0), e)),
        0x8e3e0 => set(
            c,
            render::relocate_flagged_records(m, arg(m, c, 0), arg(m, c, 1), e),
        ),
        0x387d0 => set(c, network::state_000387d0(m, e)),
        0x3b20f0 => set(c, network::flag_and_level_at_least(m, c.ecx, 0x34, 3, e)),
        0x3b2110 => set(c, network::flag_and_level_at_least(m, c.ecx, 0x34, 5, e)),
        0x3b2130 => set(c, network::flag_and_level_at_least(m, c.ecx, 0x35, 3, e)),
        0x3b2150 => set(c, network::flag_and_level_at_least(m, c.ecx, 0x35, 5, e)),
        0x35e590 => set(
            c,
            network::is_slot_limit_reached(m, arg(m, c, 0), arg(m, c, 1), e),
        ),
        0x356290 => set(c, network::count_free_record_slots(m, e)),
        0x356020 => set(
            c,
            network::player_slot_claim(m, arg(m, c, 0), arg(m, c, 1), arg(m, c, 2), e),
        ),
        0x3bc270 => set(c, network::md5_context_init(m, arg(m, c, 0), e)),
        _ => execute_cpu(va, m, c),
    }
    mutate(va, m, c);
}

fn execute_cpu(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) {
    let mut cpu = to_cpu(c);
    let mut callee = LaneCallee;
    match va {
        0x3c6730 => fesl::rc4_key_setup(m, &mut cpu),
        0x3ba270 => fesl::base64_encode(m, &mut cpu),
        0x3b5fe0 => network_calls::state_003b5fe0(m, &mut cpu),
        0x3b3120 => network_calls::find_record_by_key_pair(m, &mut cpu),
        0x33db0 => network_calls::header_store_magic_and_copy_text(&mut callee, m, &mut cpu),
        0x35e630 => network_calls::net_total_slot_count(&mut callee, m, &mut cpu),
        0x356320 => network_calls::net_count_free_player_slots(&mut callee, m, &mut cpu),
        0x3568c0 => network_calls::net_is_player_slot_connected(&mut callee, m, &mut cpu),
        0x35e6a0 => network_calls::net_set_slot_counts(&mut callee, m, &mut cpu),
        0x35eaf0 => network_calls::net_advance_session_key(&mut callee, m, &mut cpu),
        0x356990 => network_calls::net_is_role_2_in_state_0xa(&mut callee, m, &mut cpu),
        0x358130 => network_calls::net_take_pickup_flag(&mut callee, m, &mut cpu),
        0x358160 => network_calls::net_next_pickup_sequence_id(&mut callee, m, &mut cpu),
        0x356080 => network_calls::net_player_index_in_session(&mut callee, m, &mut cpu),
        0x356870 => network_calls::net_player_slot_field_3c(&mut callee, m, &mut cpu),
        0x358560 => network_calls::net_object_replicates_locally(&mut callee, m, &mut cpu),
        0x3560e0 => network_calls::net_record_124_init_at_index(&mut callee, m, &mut cpu),
        0x359910 => network_calls::net_min_word_over_connected_slots(&mut callee, m, &mut cpu),
        0x15c4e0 => network_calls::net_slot_release_each_active(&mut callee, m, &mut cpu),
        0x235600 => network_calls::net_slot_object_set_flag(&mut callee, m, &mut cpu),
        0x2d74b0 => input_calls::any_pad_has_button_mask(&mut callee, m, &mut cpu),
        _ => panic!("unregistered Rust lane port"),
    }
    from_cpu(c, cpu);
}

/// Output-bit mutant for the control matrix. 003BA270 is void with scratch EAX, so its mutant
/// corrupts the destination terminator byte instead (the output the harness compares).
#[cfg(rust_lane_mutation)]
fn mutate(va: u32, m: &mut MappedGuest, c: &mut LaneCpu) {
    // T1874: a non-zero target restricts the mutation to one root (TSFP_LANE_MUTATE_VA), so a
    // live callee that is also a lane root is not corrupted by the mutant of its caller.
    unsafe extern "C" {
        fn tsfp_lane_mutation_target() -> u32;
    }
    let target = unsafe { tsfp_lane_mutation_target() };
    if target != 0 && target != va {
        return;
    }
    if va == 0x3ba270 {
        let destination = arg(m, c, 2);
        let byte = m.read8(destination);
        m.write8(destination, byte ^ 1);
    } else {
        c.eax ^= 1;
    }
}
#[cfg(not(rust_lane_mutation))]
fn mutate(_: u32, _: &mut MappedGuest, _: &mut LaneCpu) {}

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
export!(tsfp_lane_00045d60, 0x45d60);
export!(tsfp_lane_00063bf0, 0x63bf0);
export!(tsfp_lane_000d76a0, 0xd76a0);
export!(tsfp_lane_00026b70, 0x26b70);
export!(tsfp_lane_000427b0, 0x427b0);
export!(tsfp_lane_00042c30, 0x42c30);
export!(tsfp_lane_000431c0, 0x431c0);
export!(tsfp_lane_00246d10, 0x246d10);
export!(tsfp_lane_00190d60, 0x190d60);
export!(tsfp_lane_00043a90, 0x43a90);
export!(tsfp_lane_00043af0, 0x43af0);
export!(tsfp_lane_003443f0, 0x3443f0);
export!(tsfp_lane_0001e8d0, 0x1e8d0);
export!(tsfp_lane_0008e3e0, 0x8e3e0);
export!(tsfp_lane_000387d0, 0x387d0);
export!(tsfp_lane_003b20f0, 0x3b20f0);
export!(tsfp_lane_003b2110, 0x3b2110);
export!(tsfp_lane_003b2130, 0x3b2130);
export!(tsfp_lane_003b2150, 0x3b2150);
export!(tsfp_lane_0035e590, 0x35e590);
export!(tsfp_lane_00356290, 0x356290);
export!(tsfp_lane_00356020, 0x356020);
export!(tsfp_lane_003bc270, 0x3bc270);
export!(tsfp_lane_003c6730, 0x3c6730);
export!(tsfp_lane_003ba270, 0x3ba270);
export!(tsfp_lane_003b5fe0, 0x3b5fe0);
export!(tsfp_lane_003b3120, 0x3b3120);
export!(tsfp_lane_00033db0, 0x33db0);
export!(tsfp_lane_0035e630, 0x35e630);
export!(tsfp_lane_00356320, 0x356320);
export!(tsfp_lane_003568c0, 0x3568c0);
export!(tsfp_lane_0035e6a0, 0x35e6a0);
export!(tsfp_lane_0035eaf0, 0x35eaf0);
export!(tsfp_lane_00356990, 0x356990);
export!(tsfp_lane_00358130, 0x358130);
export!(tsfp_lane_00358160, 0x358160);
export!(tsfp_lane_00356080, 0x356080);
export!(tsfp_lane_00356870, 0x356870);
export!(tsfp_lane_00358560, 0x358560);
export!(tsfp_lane_003560e0, 0x3560e0);
export!(tsfp_lane_00359910, 0x359910);
export!(tsfp_lane_0015c4e0, 0x15c4e0);
export!(tsfp_lane_00235600, 0x235600);
export!(tsfp_lane_002d74b0, 0x2d74b0);
export!(tsfp_lane_000174e0, 0x174e0);
export!(tsfp_lane_00071740, 0x71740);
export!(tsfp_lane_00264720, 0x264720);
export!(tsfp_lane_0010ebd0, 0x10ebd0);
export!(tsfp_lane_00234270, 0x234270);
export!(tsfp_lane_0036cc10, 0x36cc10);
export!(tsfp_lane_000d7550, 0xd7550);
export!(tsfp_lane_0015bca0, 0x15bca0);
export!(tsfp_lane_00177e90, 0x177e90);
export!(tsfp_lane_001b5840, 0x1b5840);
export!(tsfp_lane_001b6540, 0x1b6540);
export!(tsfp_lane_001b6a10, 0x1b6a10);
export!(tsfp_lane_00242ee0, 0x242ee0);
export!(tsfp_lane_00350d40, 0x350d40);
export!(tsfp_lane_00350e70, 0x350e70);
export!(tsfp_lane_0037a6f0, 0x37a6f0);
export!(tsfp_lane_00087840, 0x87840);
export!(tsfp_lane_00012ba0, 0x12ba0);
