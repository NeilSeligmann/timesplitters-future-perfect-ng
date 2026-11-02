// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 network/syslink, batch 1 (Tier 0: translated, compiles, smoke tested).
//! System Link state lives in guest globals (0x665908.., 0x686644, peer table 0x666040/0x66648C);
//! this module reads and writes them through [`SyslinkHost`]. Functions carry their original VA.
//! Nothing here was observed running: the recorded sessions never reach the System Link layer.

/// Guest memory plus the one platform call the batch needs.
pub trait SyslinkHost {
    fn read32(&mut self, address: u32) -> u32;
    fn read8(&mut self, address: u32) -> u8;
    fn write32(&mut self, address: u32, value: u32);
    /// Original 0x38488E: XNet link status query (nonzero when the ethernet link is up).
    fn xnet_link_status(&mut self) -> u32;
    /// Records or performs a platform call (XNet/Winsock) with stdcall arguments in order;
    /// returns EAX. The System Link functions below only issue the calls, never inspect the
    /// platform state.
    fn platform_call(&mut self, target: u32, args: &[u32]) -> u32;
    /// Original 0x156DF0 / 0x156DB0 / 0x156E30 / 0x156E70 pool helpers (opaque callees), by VA.
    fn pool_call(&mut self, target: u32, args: &[u32]);
    /// Address of a 4-byte scratch local in guest stack memory (the original passes the address
    /// of a stack variable).
    fn stack_scratch(&mut self) -> u32;
}

const SOCKET_STATE: u32 = 0x0066_5904; // 2 = socket usable
const SOCKET_HANDLE: u32 = 0x0066_57F0;
const SEND_ADDRESS: u32 = 0x0066_58A4; // sockaddr passed to every send (16 bytes)
const SEND_NODE_POOL: u32 = 0x0066_5894;
const SEND_NODES: u32 = 0x0066_5538; // 8 nodes of 0x54 bytes
const SEND_HEADER: u32 = 0x0066_5520; // 0x18-byte header written before each send
const SEND_NODE_SIZE: u32 = 0x54;
const SEND_NODE_COUNT: u32 = 8;
const SEND_BUSY: u32 = 0x0066_5910;
const SENDTO: u32 = 0x0043_188F; // platform sendto(socket, buf, len, flags, to, tolen)

pub const PEER_SLOT_FLAGS: u32 = 0x0066_6040; // dword per slot, 0 = unused
pub const PEER_ADDRESSES: u32 = 0x0066_648C; // 16 bytes per slot, the first 4 are the address
pub const PEER_SLOT_COUNT: u32 = 16;
pub const CURRENT_SLOT_INDEX: u32 = 0x0077_4040;

const GLOBAL_665908: u32 = 0x0066_5908;
const GLOBAL_66591C: u32 = 0x0066_591C;
const GLOBAL_665920: u32 = 0x0066_5920;
const GLOBAL_686644: u32 = 0x0068_6644;

/// 0002DC50: sets 665908 to 1 (returns nothing).
pub fn set_global_665908_to_1<H: SyslinkHost>(h: &mut H) {
    h.write32(GLOBAL_665908, 1);
}
/// 0002DC60: sets 665908 to 2, returns true.
pub fn set_global_665908_to_2_return_true<H: SyslinkHost>(h: &mut H) -> u32 {
    h.write32(GLOBAL_665908, 2);
    1
}
/// 0002E200 / 0002E210: the same pair for 66591C.
pub fn set_global_66591c_to_1<H: SyslinkHost>(h: &mut H) {
    h.write32(GLOBAL_66591C, 1);
}
pub fn set_global_66591c_to_2_return_true<H: SyslinkHost>(h: &mut H) -> u32 {
    h.write32(GLOBAL_66591C, 2);
    1
}
/// 0002E220 / 0002E230: the same pair for 665920.
pub fn set_global_665920_to_1<H: SyslinkHost>(h: &mut H) {
    h.write32(GLOBAL_665920, 1);
}
pub fn set_global_665920_to_2_return_true<H: SyslinkHost>(h: &mut H) -> u32 {
    h.write32(GLOBAL_665920, 2);
    1
}
/// 0002E240: clears 665920 and 66591C.
pub fn clear_globals_665920_and_66591c<H: SyslinkHost>(h: &mut H) {
    h.write32(GLOBAL_665920, 0);
    h.write32(GLOBAL_66591C, 0);
}
/// 0002E510: sets 686644 to 1. 0002DF20 is a tail-jump wrapper with the same effect.
pub fn global_686644_set_one<H: SyslinkHost>(h: &mut H) {
    h.write32(GLOBAL_686644, 1);
}
pub fn global_686644_set_one_wrapper<H: SyslinkHost>(h: &mut H) {
    global_686644_set_one(h);
}
/// 0002E520: returns the constant 0x24.
pub fn return_constant_0x24() -> u32 {
    0x24
}
/// 0002E500: ethernet link status as 0 or 1 (normalised from the XNet query).
pub fn ethernet_link_status_nonzero<H: SyslinkHost>(h: &mut H) -> u32 {
    u32::from(h.xnet_link_status() != 0)
}
/// 0002E4A0: peer-slot flag of the current slot (index in 774040).
pub fn current_peer_slot_flags<H: SyslinkHost>(h: &mut H) -> u32 {
    let index = h.read32(CURRENT_SLOT_INDEX);
    h.read32(PEER_SLOT_FLAGS.wrapping_add(index.wrapping_mul(4)))
}
/// 0002E4B0 `net_peer_find_by_addr(address)`: index of the first used slot whose 4-byte address
/// equals the 4 bytes at `address`, or u32::MAX (-1) when none.
pub fn peer_find_by_addr<H: SyslinkHost>(h: &mut H, address: u32) -> u32 {
    for slot in 0..PEER_SLOT_COUNT {
        if h.read32(PEER_SLOT_FLAGS + 4 * slot) == 0 {
            continue;
        }
        let entry = PEER_ADDRESSES + 0x10 * slot;
        if (0..4).all(|i| h.read8(entry + i) == h.read8(address + i)) {
            return slot;
        }
    }
    u32::MAX
}

/// 0002DCC0 `net_send_to_peer`: when the socket is usable, sendto(socket, buffer = register ECX,
/// length = stack arg0) towards the fixed send address; returns 0. `ecx` is the register argument the
/// original receives (the buffer pointer); `arg0` is the stack argument (the length).
pub fn send_to_peer<H: SyslinkHost>(h: &mut H, ecx: u32, arg0: u32) -> u32 {
    if h.read32(SOCKET_STATE) == 2 {
        let socket = h.read32(SOCKET_HANDLE);
        h.platform_call(SENDTO, &[socket, ecx, arg0, 0, SEND_ADDRESS, 0x10]);
    }
    0
}

/// 0002DCF0 `net_send_packet_init_node_pool_sendto`: initialises the send node pool (8 nodes of
/// 0x54 bytes, zeroed first), then, when the socket is usable, sends the 4-byte value -5 and marks
/// the sender busy.
pub fn send_packet_init_node_pool<H: SyslinkHost>(h: &mut H) {
    let local = h.stack_scratch();
    h.write32(local, 0xFFFF_FFFB);
    h.pool_call(0x0015_6DF0, &[SEND_NODE_POOL]);
    for offset in (0..SEND_NODE_SIZE * SEND_NODE_COUNT).step_by(4) {
        h.write32(SEND_NODES + offset, 0);
    }
    h.pool_call(0x0015_6DB0, &[SEND_NODE_POOL, SEND_NODE_SIZE, SEND_NODES, SEND_NODE_COUNT]);
    if h.read32(SOCKET_STATE) == 2 {
        let socket = h.read32(SOCKET_HANDLE);
        h.platform_call(SENDTO, &[socket, local, 4, 0, SEND_ADDRESS, 0x10]);
        h.write32(SEND_BUSY, 1);
    }
}

/// 0002DEC0 `net_sendto_peer_helper(index)`: loads the header from node `index`, clears the busy
/// flag, marks the header valid and sends the 0x18-byte header when the socket is usable.
pub fn sendto_peer_helper<H: SyslinkHost>(h: &mut H, index: u32) {
    let node = SEND_NODES.wrapping_add(index.wrapping_mul(SEND_NODE_SIZE));
    let first = h.read32(node);
    let second = h.read32(node.wrapping_add(4));
    h.write32(SEND_BUSY, 0);
    let flags = h.read32(SEND_HEADER + 0x14);
    h.write32(SEND_HEADER + 0x14, (flags & !0xFF) | 1); // 00665534 byte = 1
    h.write32(SEND_HEADER + 0xC, first); // 0066552C
    h.write32(SEND_HEADER + 0x10, second); // 00665530
    if h.read32(SOCKET_STATE) == 2 {
        let socket = h.read32(SOCKET_HANDLE);
        h.platform_call(SENDTO, &[socket, SEND_HEADER, 0x18, 0, SEND_ADDRESS, 0x10]);
    }
}
