// SPDX-License-Identifier: GPL-3.0-or-later
//! T1907 launch configuration slice. Each listed VA is Tier 0 (smoke only).
//! Call-bearing ports retain the guest Callee boundary; only EAX is a return contract.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// Explicit byte view of the shared launch record at 007EB300. Guest aliases stay live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchBlock(pub u32);
impl LaunchBlock {
    pub const ORIGINAL: Self = Self(0x007E_B300);
    pub fn flags<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.0.wrapping_add(0x58))
    }
    pub fn add_flags<M: GuestMemory>(self, m: &mut M, bits: u32) {
        m.write32(self.0.wrapping_add(0x58), self.flags(m) | bits);
    }
}
/// The argument array contains guest string pointers; index zero is reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchArguments {
    pub count_address: u32,
    pub array_address: u32,
    pub cursor_address: u32,
}
impl LaunchArguments {
    pub const ORIGINAL: Self = Self {
        count_address: 0x0074_CF68,
        array_address: 0x0074_CF6C,
        cursor_address: 0x006B_7A20,
    };
}
fn equal_string<M: GuestMemory>(m: &M, mut a: u32, mut b: u32) -> bool {
    loop {
        let x = m.read8(a);
        if x != m.read8(b) {
            return false;
        }
        if x == 0 {
            return true;
        }
        a = a.wrapping_add(1);
        b = b.wrapping_add(1);
    }
}
fn equal_bytes<M: GuestMemory>(m: &M, a: u32, b: u32, count: u32) -> bool {
    (0..count).all(|i| m.read8(a.wrapping_add(i)) == m.read8(b.wrapping_add(i)))
}
/// 0003C230 cdecl(name): case-sensitive lookup; writes the global scan cursor, returns zero on miss.
/// Empty names clear the cursor without reading argc or argv. Counts compare as signed integers.
/// The original scans in registers: argv is read once, argc on every step, and the cursor is written
/// only on exit (index on a hit, zero otherwise), so a name or argv entry stored on the cursor word
/// is compared unmodified (T2409 differential against the lifted original).
pub fn find_argument<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let name = stack_arg(m, cpu, 0);
    let view = LaunchArguments::ORIGINAL;
    if m.read8(name) != 0 && (m.read32(view.count_address) as i32) > 1 {
        let argv = m.read32(view.array_address);
        let mut index = 1u32;
        loop {
            if equal_string(m, name, m.read32(argv.wrapping_add(index.wrapping_mul(4)))) {
                m.write32(view.cursor_address, index);
                cpu.eax = index;
                return;
            }
            index = index.wrapping_add(1);
            if (index as i32) >= (m.read32(view.count_address) as i32) {
                break;
            }
        }
    }
    m.write32(view.cursor_address, 0);
    cpu.eax = 0;
}
fn parse_value<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    target: u32,
    pc: u32,
) -> u32 {
    let text = stack_arg(m, cpu, 0);
    guest_call(c, m, cpu, target, pc, 0, &[text]);
    cpu.eax
}

/// 0003C920 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_map_byte<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003C92A);
    m.write8(0x007EB354, value as u8);
    cpu.eax = 1;
}

/// 0003C970 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_player_counts<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003C97A);
    m.write8(0x007EB300, value as u8);
    m.write8(0x007EB301, value as u8);
    cpu.eax = 1;
}

/// 0003C990 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_mode_byte<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003C99A);
    m.write8(0x007EB302, value as u8);
    cpu.eax = 1;
}

/// 0003D1A0 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_limit<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003D1AA);
    m.write32(0x007EB374, value);
    cpu.eax = 1;
}

/// 0003D1C0 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_score<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003D1CA);
    m.write32(0x007EB368, value);
    cpu.eax = 1;
}

/// 0003D1E0 cdecl(text): numeric launch handler; returns one. Tier 0.
pub fn set_time<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003D1EA);
    m.write32(0x007EB364, value);
    cpu.eax = 1;
}

/// 0003C940: set flag before calling the player-name resolver 0006F450. Tier 0.
pub fn set_named_player<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    LaunchBlock::ORIGINAL.add_flags(m, 0x2000_0000);
    let value = parse_value(c, m, cpu, 0x0006_F450, 0x0003_C954);
    m.write8(0x007E_B39B, value as u8);
    cpu.eax = 1;
}
/// 0003D140: store signed count; call 000389B0(-1) if below one. Tier 0.
pub fn set_global_count<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003_D14A);
    m.write32(0x006B_7A88, value);
    if (value as i32) < 1 {
        guest_call(c, m, cpu, 0x0003_89B0, 0x0003_D15E, 0, &[u32::MAX]);
    }
    cpu.eax = 1;
}
/// 0003D170: multiply parsed count by 004E8CEC, wrapping at 32 bits. Tier 0.
pub fn set_scaled_time<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003_D17A);
    m.write32(0x007E_B360, value.wrapping_mul(m.read32(0x004E_8CEC)));
    cpu.eax = 1;
}
/// 0003D200: parsed byte at +9F; +57 receives ONE after EAX is overwritten. Tier 0.
pub fn set_special_mode<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    LaunchBlock::ORIGINAL.add_flags(m, 0x0080_0000);
    let value = parse_value(c, m, cpu, 0x003C_8FD0, 0x0003_D214);
    m.write8(0x007E_B39F, value as u8);
    cpu.eax = 1;
    m.write8(0x007E_B357, 1);
}
/// 0003CEE0: fixed-length comparisons include terminators in the guest constants. Tier 0.
pub fn set_variant_name<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let text = stack_arg(m, cpu, 0);
    for (constant, count, value) in [
        (0x0047_80A8, 5, 0),
        (0x0047_80A0, 7, 1),
        (0x0047_8098, 5, 2),
    ] {
        if equal_bytes(m, text, constant, count) {
            m.write8(0x007E_B357, value);
            break;
        }
    }
    cpu.eax = 1;
}
/// 0003D2B0: exact seven-byte comparison, preserving guest constant data. Tier 0.
pub fn set_global_switch<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let text = stack_arg(m, cpu, 0);
    let value = u32::from(equal_bytes(m, text, 0x0047_80B0, 7));
    m.write32(0x006B_7A8C, value);
    cpu.eax = 1;
}
fn launch_image<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    kind: u32,
    pc: u32,
) {
    // Original allocates 0xC00 bytes but initialises only its first four dwords.
    let data = cpu.esp.wrapping_sub(0xC00);
    for (i, value) in [kind, 0, 0, 0].iter().enumerate() {
        m.write32(data.wrapping_add(i as u32 * 4), *value);
    }
    cpu.eax = 0;
    cpu.ecx = data;
    guest_call(c, m, cpu, 0x0037_C9F6, pc, 0xC00, &[0, data]);
}
/// 00031AB0 launches image kind 8. Tier 0; untouched launch payload bytes remain untouched.
pub fn launch_kind_8<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    launch_image(c, m, cpu, 8, 0x0003_1AD6);
}
/// 00031AE0 launches image kind 6. Tier 0.
pub fn launch_kind_6<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    launch_image(c, m, cpu, 6, 0x0003_1B06);
}
/// 00031B10 launches image kind 7. Tier 0.
pub fn launch_kind_7<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    launch_image(c, m, cpu, 7, 0x0003_1B36);
}
/// 000372A0: guest tail jump; original frame is passed unchanged. Tier 0.
pub fn thunk_launch_kind_8<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    c.call(m, 0x0003_1AB0, cpu);
}
/// 000372B0: guest tail jump. Tier 0.
pub fn thunk_launch_kind_6<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    c.call(m, 0x0003_1AE0, cpu);
}
/// 00037300: guest tail jump. Tier 0.
pub fn thunk_launch_kind_7<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    c.call(m, 0x0003_1B10, cpu);
}
/// 0002FF60: join active launch worker, close its handle, then restore priority boost. Tier 0.
pub fn join_launch_worker<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.eax = m.read32(0x0068_7E24);
    if cpu.eax == 0 {
        return;
    }
    let handle = m.read32(0x0068_7DDC);
    m.write32(0x0068_7E20, 1);
    guest_call(c, m, cpu, 0x0038_00BF, 0x0002_FF80, 0, &[handle, u32::MAX]);
    let handle = m.read32(0x0068_7DDC);
    guest_call(c, m, cpu, 0x0037_CBC4, 0x0002_FF8C, 0, &[handle]);
    let thread = m.read32(0x0068_7DE0);
    guest_call(c, m, cpu, 0x0037_FC76, 0x0002_FF9A, 0, &[thread, 0]);
}

/// Per-function opt-in selector: false means the caller retains its lifted implementation.
/// No production registration is changed by this module; host adapters are verification debt.
pub fn run_if_enabled<M: GuestMemory, C: Callee<M>>(
    address: u32,
    enabled: bool,
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) -> bool {
    if !enabled {
        return false;
    }
    match address {
        0x0003C230 => find_argument(m, cpu),
        0x0003C2D0 => parse_launch_and_enter_state(c, m, cpu),
        0x0003C920 => set_map_byte(c, m, cpu),
        0x0003C940 => set_named_player(c, m, cpu),
        0x0003C970 => set_player_counts(c, m, cpu),
        0x0003C990 => set_mode_byte(c, m, cpu),
        0x0003D140 => set_global_count(c, m, cpu),
        0x0003D170 => set_scaled_time(c, m, cpu),
        0x0003D1A0 => set_limit(c, m, cpu),
        0x0003D1C0 => set_score(c, m, cpu),
        0x0003D1E0 => set_time(c, m, cpu),
        0x0003D200 => set_special_mode(c, m, cpu),
        0x0003CEE0 => set_variant_name(m, cpu),
        0x0003D2B0 => set_global_switch(m, cpu),
        0x00031AB0 => launch_kind_8(c, m, cpu),
        0x00031AE0 => launch_kind_6(c, m, cpu),
        0x00031B10 => launch_kind_7(c, m, cpu),
        0x000372A0 => thunk_launch_kind_8(c, m, cpu),
        0x000372B0 => thunk_launch_kind_6(c, m, cpu),
        0x00037300 => thunk_launch_kind_7(c, m, cpu),
        0x0002FF60 => join_launch_worker(c, m, cpu),
        0x00227100 => super::initializers::init_slot_arrays(m, cpu),
        0x00032550 => super::initializers::init_flag_addresses(m, cpu),
        0x0002A200 => super::initializers::init_threshold_masks(m, cpu),
        _ => return false,
    }
    true
}

/// 0003C2D0: zero the launch block, parse defaults, initialise state/text bank, enter state 1.
/// Tier 0. The original final callee never returns: a returning implementation is a contract error.
pub fn parse_launch_and_enter_state<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    for i in 0..0x28 {
        m.write32(0x007E_B300 + 4 * i, 0);
    }
    let argv = m.read32(0x0074_CF6C);
    let argc = m.read32(0x0074_CF68);
    guest_call(c, m, cpu, 0x001B_5300, 0x0003_C2F1, 4, &[argc, argv]);
    guest_call(c, m, cpu, 0x0006_58D0, 0x0003_C2FB, 12, &[0x007E_B300]);
    if m.read8(0x007E_B354) != 0x67 {
        m.write32(0x004C_0454, 0);
        guest_call(c, m, cpu, 0x0003_DD20, 0x0003_C317, 0, &[]);
    }
    guest_call(c, m, cpu, 0x0003_89B0, 0x0003_C31C, 0, &[]);
    guest_call(c, m, cpu, 0x0003_D6A0, 0x0003_C323, 0, &[1]);
    panic!("nonreturning launch state entry 0003D6A0 returned");
}
