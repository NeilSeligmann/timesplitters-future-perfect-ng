// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 network/syslink, batch 3 (Tier 0: translated, compiles, smoke tested).
//! First drafted by the cheap model (gpt-6-luna) from the private lifted C, then reviewed by hand
//! against that text. Review fixes: 2E530 shifted the index by 2 instead of 4, 2E650 passed the
//! frame offsets of the later ESP instead of the array bases, 2E390 passed a spurious argument to
//! the last-error query. Every call goes through [`SyslinkHost::platform_call`] (the callee may be
//! a lifted game function or a platform routine). Nothing here was observed running.
use crate::w4n_syslink::SyslinkHost;

/// 0002DC70
pub fn net_socket_cleanup_xnet_unregister_close<H: SyslinkHost>(h: &mut H) -> u32 {
    const C_0002DC70_STATE_A: u32 = 0x00665904;
    const C_0002DC70_STATE_B: u32 = 0x00665908;
    const C_0002DC70_BUFFER: u32 = 0x006657D8;
    const C_0002DC70_VALUE: u32 = 0x006657F0;

    if h.read32(C_0002DC70_STATE_A) == 2 || h.read32(C_0002DC70_STATE_B) == 2 {
        h.platform_call(0x00431721, &[C_0002DC70_BUFFER]);
        let value = h.read32(C_0002DC70_VALUE);
        h.platform_call(0x004317BC, &[value]);
        h.write32(C_0002DC70_STATE_A, 0);
        h.write32(C_0002DC70_STATE_B, 0);
        h.platform_call(0x00431CB6, &[]);
        h.platform_call(0x00431CC3, &[]);
        h.platform_call(0x0002E280, &[]);
    }

    1
}

/// 0002E1D0: Retry link operations until they succeed or the retry count expires.
pub fn net_recv_discovery_wrapper<H: SyslinkHost>(h: &mut H) {
    let mut remaining = 0x14u32;
    loop {
        if h.platform_call(0x0002DFC0, &[]) == 0 {
            break;
        }
        if remaining == 0 {
            break;
        }
        remaining = remaining.wrapping_sub(1);
    }

    while h.platform_call(0x0002EBC0, &[]) != 0 {}
}

/// 0002E250
pub fn net_socket_close_unregister_wrapper<H: SyslinkHost>(h: &mut H) -> u32 {
    const C_0002E250_STATUS_A: u32 = 0x00665920;
    const C_0002E250_STATUS_B: u32 = 0x0066591C;
    const C_0002E250_INIT: u32 = 0x0002E280;

    if h.read32(C_0002E250_STATUS_A) != 0 || h.read32(C_0002E250_STATUS_B) != 0 {
        return 0;
    }

    h.platform_call(C_0002E250_INIT, &[]);
    1
}

/// Xbox VA 0002E390.
pub fn net_error_assert<H: SyslinkHost>(h: &mut H, arg0: u32) -> u32 {
    const C_0002E390_COUNTER_BASE: u32 = 0x0077_4484;
    const C_0002E390_ENTRY_STRIDE: u32 = 0x30C;
    const C_0002E390_BUFFER_OFFSET: u32 = 0x0077_4180;
    const C_0002E390_GLOBAL_STATUS: u32 = 0x0068_6644;

    let status = h.platform_call(0x0043_1D53, &[]);
    if status != 0x2751 {
        return 0;
    }

    let entry_offset = (arg0 as i32 as u32).wrapping_mul(C_0002E390_ENTRY_STRIDE);
    let counter_address = entry_offset.wrapping_add(C_0002E390_COUNTER_BASE);
    let word = h.read32(counter_address);
    let counter = (word as u16).wrapping_add(1);
    h.write32(counter_address, (word & 0xFFFF_0000) | u32::from(counter));

    if counter > 0x32 {
        let allocation = h.platform_call(0x0035_5EC0, &[]);
        let buffer = entry_offset.wrapping_add(C_0002E390_BUFFER_OFFSET);
        if allocation != 0 {
            h.platform_call(0x0035_BEF0, &[buffer, 0]);
            return 1;
        }

        if h.read32(C_0002E390_GLOBAL_STATUS) != 0 {
            h.platform_call(0x0035_BEF0, &[buffer, 0]);
            return 1;
        }

        h.platform_call(0x0035_6BB0, &[0x0047_6638, 0x1CE, 1]);
    }

    1
}

/// 0002E530: Look up and process a non-null indexed entry.
pub fn net_inaddr_to_xnaddr_wrapper<H: SyslinkHost>(h: &mut H, index: u32) -> u32 {
    let entry = h.read32(index.wrapping_mul(4).wrapping_add(0x0066_6040));
    if entry == 0 {
        return 0x0068_6658;
    }

    let esi = index.wrapping_add(index.wrapping_mul(8))
        .wrapping_mul(4)
        .wrapping_add(0x0066_5930);
    let value = h.read32(index.wrapping_shl(4).wrapping_add(0x0066_648C));
    let _ = h.platform_call(0x0043_1968, &[value, esi, 0]);
    esi
}

/// Lifted from VA 0002E650.
pub fn net_peer_qos_lookup_start<H: SyslinkHost>(h: &mut H) -> u32 {
    const C_0002E650_FLAGS: u32 = 0x0066_5FC4;
    const C_0002E650_BASE: u32 = 0x0066_5954;
    const C_0002E650_DEST: u32 = 0x0066_6588;
    const C_0002E650_SOURCE: u32 = 0x0068_65FC;
    const C_0002E650_RESULT: u32 = 0x0068_667C;
    const C_0002E650_CALL: u32 = 0x0043_177F;
    const C_0002E650_FINAL: u32 = 0x0066_6080;

    // The scratch pointer corresponds to ESP after the local allocation and saved registers.
    let frame = h.stack_scratch();
    let mut count = 0u32;
    let mut flags = C_0002E650_FLAGS;
    let mut source = C_0002E650_BASE;

    while flags < 0x0066_6004 {
        if h.read32(flags.wrapping_sub(4)) != 0 {
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x8C), source.wrapping_sub(36));
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x0C), C_0002E650_DEST);
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x4C), C_0002E650_SOURCE);
            count = count.wrapping_add(1);
        }
        if h.read32(flags) != 0 {
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x8C), source);
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x0C), C_0002E650_DEST);
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x4C), C_0002E650_SOURCE);
            count = count.wrapping_add(1);
        }
        if h.read32(flags.wrapping_add(4)) != 0 {
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x8C), source.wrapping_add(0x24));
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x0C), C_0002E650_DEST);
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x4C), C_0002E650_SOURCE);
            count = count.wrapping_add(1);
        }
        if h.read32(flags.wrapping_add(8)) != 0 {
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x8C), source.wrapping_add(0x48));
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x0C), C_0002E650_DEST);
            h.write32(frame.wrapping_add(count.wrapping_mul(4)).wrapping_add(0x4C), C_0002E650_SOURCE);
            count = count.wrapping_add(1);
        }
        flags = flags.wrapping_add(0x10);
        source = source.wrapping_add(0x90);
    }

    h.write32(C_0002E650_RESULT, count);
    h.platform_call(
        C_0002E650_CALL,
        &[
            count,
            frame.wrapping_add(0x8C),
            frame.wrapping_add(0x0C),
            frame.wrapping_add(0x4C),
            0,
            0,
            0,
            8,
            0,
            0,
            0,
            C_0002E650_FINAL,
        ],
    )
}
