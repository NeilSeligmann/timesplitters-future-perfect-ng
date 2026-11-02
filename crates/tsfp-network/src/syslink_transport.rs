// SPDX-License-Identifier: GPL-3.0-or-later
//! T1911 socket and packet slice. Each original-address function is Tier 0.
//! Guest addresses preserve shared state; platform behavior is supplied by traits.
use crate::w4n_syslink::{peer_find_by_addr, SyslinkHost};

pub trait DatagramHost: SyslinkHost {
    fn socket(&mut self, family: u32, kind: u32, protocol: u32) -> u32;
    fn bind(&mut self, socket: u32, address: u32, length: u32);
    fn nonblocking(&mut self, socket: u32, enabled: u32);
    fn close(&mut self, socket: u32);
    fn register_key(&mut self, id: u32, key: u32);
    fn unregister_key(&mut self, id: u32);
    fn stop_qos(&mut self, id: u32);
    fn resolve_peer(&mut self, xn_address: u32, id: u32, output: u32);
    fn live_mode(&mut self) -> bool;
    fn send(&mut self, socket: u32, buffer: u32, length: u32, address: u32) -> i32;
    /// Write bytes and the source sockaddr into guest memory, return received length.
    fn receive(&mut self, socket: u32, buffer: u32, capacity: u32, address: u32) -> i32;
    fn last_error(&mut self) -> u32;
    fn disconnect_peer(&mut self, index: u32);
    fn receive_tail(&mut self, buffer: u32, peer: u32, length: u32);
    fn write8(&mut self, address: u32, value: u8);
}
const SOCKET: u32 = 0x665900;
const OPEN: u32 = 0x686640;
const KEY: u32 = 0x666588;
const REGISTERED: u32 = 0x686648;
fn fill_queue<H: DatagramHost>(h: &mut H) {
    for i in 0..256 {
        h.write32(0x666088 + i * 4, u32::MAX);
    }
}
fn copy<H: DatagramHost>(h: &mut H, dst: u32, src: u32, n: u32) {
    // Match the original forward dword copy followed by its byte remainder.
    for i in 0..n / 4 {
        let value = h.read32(src.wrapping_add(i * 4));
        h.write32(dst.wrapping_add(i * 4), value);
    }
    for i in n & !3..n {
        let value = h.read8(src.wrapping_add(i));
        h.write8(dst.wrapping_add(i), value);
    }
}
/// 0x2E280: unregister QoS/key, invalidate queue, clear peers, close socket.
pub fn close_session<H: DatagramHost>(h: &mut H) {
    if h.read32(REGISTERED) != 0 {
        h.stop_qos(KEY);
        h.unregister_key(KEY);
        h.write32(REGISTERED, 0);
    }
    fill_queue(h);
    for i in 0..16 {
        h.write32(0x666040 + i * 4, 0);
        h.write32(0x66648c + i * 16, 0);
    }
    let socket = h.read32(SOCKET);
    h.close(socket);
    for address in [0x68664c, 0x686650, OPEN, 0x686644] {
        h.write32(address, 0);
    }
}
/// 0x2E810: optional 21-dword session descriptor, duplicated key material.
pub fn install_descriptor<H: DatagramHost>(h: &mut H, source: u32) {
    if source == 0 {
        return;
    }
    copy(h, 0x666590, source, 84);
    for (index, dst) in [
        (15, KEY),
        (16, KEY + 4),
        (17, 0x6865fc),
        (18, 0x686600),
        (19, 0x686604),
        (20, 0x686608),
    ] {
        let value = h.read32(source.wrapping_add(index * 4));
        h.write32(dst, value);
    }
}
/// 0x2E870: register LAN key and resolve active peers except the local host slot.
pub fn register_session_key<H: DatagramHost>(h: &mut H) {
    if h.live_mode() {
        return;
    }
    let host = h.read32(0x774028);
    if host == 0 {
        return;
    }
    h.register_key(KEY, 0x6865fc);
    h.write32(REGISTERED, 1);
    for slot in 0..16 {
        if slot != h.read32(host) && h.read32(0x666040 + slot * 4) != 0 {
            h.resolve_peer(0x665930 + slot * 36, KEY, 0x66648c + slot * 16);
        }
    }
}
/// 0x2E8E0: UDP-like protocol 254 socket, port 1000, nonblocking; errors ignored originally.
pub fn open_session<H: DatagramHost>(h: &mut H) {
    if h.read32(OPEN) != 0 {
        close_session(h);
    }
    let socket = h.socket(2, 2, 254);
    h.write32(SOCKET, socket);
    h.write32(0x665fb0, 2 | (1000_u16.swap_bytes() as u32) << 16);
    h.write32(0x665fb4, 0);
    h.bind(socket, 0x665fb0, 16);
    h.nonblocking(socket, 1);
    fill_queue(h);
    h.write32(OPEN, 1);
}
/// 0x2E980: prefix payload length (u16), optional tail, send and disconnect on hard error.
pub fn send_packet<H: DatagramHost>(
    h: &mut H,
    payload: u32,
    length: u32,
    address: u32,
    tail: u32,
    tail_length: u32,
) -> i32 {
    copy(h, 0x686682, payload, length);
    h.write8(0x686680, length as u8);
    h.write8(0x686681, (length >> 8) as u8);
    let extra = if tail == 0 {
        0
    } else {
        copy(h, 0x686682_u32.wrapping_add(length), tail, tail_length);
        tail_length
    };
    let total = length.wrapping_add(2).wrapping_add(extra);
    let socket = h.read32(SOCKET);
    let result = h.send(socket, 0x686680, total, address);
    if result as u32 != total && h.last_error() != 10035 && length as i32 > 0 {
        let peer = peer_find_by_addr(h, address.wrapping_add(4));
        h.disconnect_peer(peer);
    }
    result
}
/// 0x2EA20: original signed MAX(copy length, header length), including short/malformed packets.
pub fn receive_packet<H: DatagramHost>(
    h: &mut H,
    output: u32,
    copy_length: u32,
    address: u32,
) -> u32 {
    let socket = h.read32(SOCKET);
    let received = h.receive(socket, 0x686a80, 512, address);
    if received < 0 {
        if h.last_error() != 10035 {
            let peer = peer_find_by_addr(h, address.wrapping_add(4));
            h.disconnect_peer(peer);
        }
        return 0;
    }
    let length = h.read8(0x686a80) as u32 | (h.read8(0x686a81) as u32) << 8;
    let count = if (copy_length as i32) < length as i32 {
        length
    } else {
        copy_length
    };
    copy(h, output, 0x686a82, count);
    let remainder = received.wrapping_sub(2);
    if (length as i32) < remainder {
        let peer = peer_find_by_addr(h, address.wrapping_add(4));
        h.receive_tail(
            0x686a82 + length,
            peer,
            (remainder as u32).wrapping_sub(length),
        );
        if length < 5 {
            return u32::MAX;
        }
    }
    length
}

/// Messenger's temporary endpoint object is a scoped host resource (constructor/destructor).
/// Handler return uses its low byte; all addresses are guest pointers.
pub trait MessengerHost: DatagramHost {
    fn endpoint_begin(&mut self) -> u32;
    fn endpoint_set_source(&mut self, endpoint: u32, sockaddr: u32);
    fn endpoint_end(&mut self, endpoint: u32);
    fn handler_accept(&mut self, handler: u32, endpoint: u32, buffer: u32, length: u32) -> u8;
    fn handler_dispatch(&mut self, handler: u32, endpoint: u32, buffer: u32, length: u32);
    fn special_dispatch(&mut self, object: u32, endpoint: u32, buffer: u32, length: u32);
}
/// 0x313D0: receive once, route accepted/ordinary vs four-FF special message, always return 0.
pub fn messenger_receive<H: MessengerHost>(h: &mut H, object: u32, source_scratch: u32) -> u32 {
    let endpoint = h.endpoint_begin();
    let buffer = object.wrapping_add(8);
    let socket = h.read32(object.wrapping_add(0x102c));
    let length = h.receive(socket, buffer, 4096, source_scratch);
    if length < 0 {
        h.last_error();
    } else {
        h.endpoint_set_source(endpoint, source_scratch);
        let handler = h.read32(object.wrapping_add(4));
        let accepted = h.handler_accept(handler, endpoint, buffer, length as u32);
        if accepted == 0 && length >= 5 && (0..4).all(|i| h.read8(buffer.wrapping_add(i)) == 255) {
            h.special_dispatch(object, endpoint, buffer, length as u32);
        } else {
            h.handler_dispatch(handler, endpoint, buffer, length as u32);
        }
    }
    h.endpoint_end(endpoint);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Host {
        bytes: BTreeMap<u32, u8>,
        calls: Vec<String>,
        received: i32,
        sent: i32,
        error: u32,
    }
    impl SyslinkHost for Host {
        fn read32(&mut self, a: u32) -> u32 {
            u32::from_le_bytes(std::array::from_fn(|i| self.read8(a + i as u32)))
        }
        fn read8(&mut self, a: u32) -> u8 {
            *self.bytes.get(&a).unwrap_or(&0)
        }
        fn write32(&mut self, a: u32, v: u32) {
            for (i, b) in v.to_le_bytes().iter().enumerate() {
                self.write8(a + i as u32, *b);
            }
        }
        fn xnet_link_status(&mut self) -> u32 {
            0
        }
        fn platform_call(&mut self, _: u32, _: &[u32]) -> u32 {
            0
        }
        fn pool_call(&mut self, _: u32, _: &[u32]) {}
        fn stack_scratch(&mut self) -> u32 {
            0
        }
    }
    impl DatagramHost for Host {
        fn socket(&mut self, f: u32, k: u32, p: u32) -> u32 {
            assert_eq!((f, k, p), (2, 2, 254));
            self.calls.push("socket".into());
            7
        }
        fn bind(&mut self, s: u32, a: u32, n: u32) {
            assert_eq!((s, a, n), (7, 0x665fb0, 16));
            self.calls.push("bind".into());
        }
        fn nonblocking(&mut self, s: u32, e: u32) {
            assert_eq!((s, e), (7, 1));
            self.calls.push("nonblocking".into());
        }
        fn close(&mut self, _: u32) {
            self.calls.push("close".into());
        }
        fn register_key(&mut self, _: u32, _: u32) {
            self.calls.push("register".into());
        }
        fn unregister_key(&mut self, _: u32) {
            self.calls.push("unregister".into());
        }
        fn stop_qos(&mut self, _: u32) {
            self.calls.push("qos".into());
        }
        fn resolve_peer(&mut self, a: u32, _: u32, o: u32) {
            self.calls.push(format!("resolve:{a:x}:{o:x}"));
        }
        fn live_mode(&mut self) -> bool {
            false
        }
        fn send(&mut self, _: u32, _: u32, n: u32, _: u32) -> i32 {
            self.calls.push(format!("send:{n}"));
            self.sent
        }
        fn receive(&mut self, _: u32, _: u32, _: u32, _: u32) -> i32 {
            self.received
        }
        fn last_error(&mut self) -> u32 {
            self.calls.push("error".into());
            self.error
        }
        fn disconnect_peer(&mut self, i: u32) {
            self.calls.push(format!("disconnect:{i}"));
        }
        fn receive_tail(&mut self, _: u32, p: u32, n: u32) {
            self.calls.push(format!("tail:{p}:{n}"));
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.bytes.insert(a, v);
        }
    }
    impl MessengerHost for Host {
        fn endpoint_begin(&mut self) -> u32 {
            self.calls.push("begin".into());
            99
        }
        fn endpoint_set_source(&mut self, _: u32, _: u32) {
            self.calls.push("source".into());
        }
        fn endpoint_end(&mut self, _: u32) {
            self.calls.push("end".into());
        }
        fn handler_accept(&mut self, _: u32, _: u32, _: u32, _: u32) -> u8 {
            0
        }
        fn handler_dispatch(&mut self, _: u32, _: u32, _: u32, _: u32) {
            self.calls.push("ordinary".into());
        }
        fn special_dispatch(&mut self, _: u32, _: u32, _: u32, _: u32) {
            self.calls.push("special".into());
        }
    }
    #[test]
    fn lifecycle_and_reopen() {
        let mut h = Host::default();
        open_session(&mut h);
        assert_eq!(h.read32(0x665fb0), 0xe8030002);
        assert_eq!(h.read32(0x666484), u32::MAX);
        h.write32(REGISTERED, 1);
        open_session(&mut h);
        assert_eq!(
            h.calls,
            [
                "socket",
                "bind",
                "nonblocking",
                "qos",
                "unregister",
                "close",
                "socket",
                "bind",
                "nonblocking"
            ]
        );
        assert_eq!(h.read32(OPEN), 1);
    }
    #[test]
    fn descriptor_and_register() {
        let mut h = Host::default();
        install_descriptor(&mut h, 0);
        for i in 0..21 {
            h.write32(0x100 + i * 4, i + 10);
        }
        install_descriptor(&mut h, 0x100);
        assert_eq!(h.read32(KEY), 25);
        assert_eq!(h.read32(0x686608), 30);
        h.write32(0x774028, 0x200);
        h.write32(0x200, 1);
        h.write32(0x666044, 1);
        h.write32(0x666048, 1);
        register_session_key(&mut h);
        assert_eq!(h.calls, ["register", "resolve:665978:6664ac"]);
    }
    #[test]
    fn send_tail_and_hard_error() {
        let mut h = Host {
            sent: -1,
            error: 5,
            ..Host::default()
        };
        h.write32(0x100, 0x12345678);
        h.write8(0x200, 9);
        assert_eq!(send_packet(&mut h, 0x100, 3, 0x300, 0x200, 1), -1);
        assert_eq!(h.read32(0x686682), 0x09345678);
        assert_eq!(h.calls, ["send:6", "error", "disconnect:4294967295"]);
    }
    #[test]
    fn receive_preserves_max_copy_and_short_tail() {
        let mut h = Host {
            received: 8,
            ..Host::default()
        };
        h.write8(0x686a80, 4);
        for i in 0..8 {
            h.write8(0x686a82 + i, i as u8 + 1);
        }
        assert_eq!(receive_packet(&mut h, 0x100, 8, 0x200), u32::MAX);
        assert_eq!(h.read32(0x104), 0x08070605);
        assert_eq!(h.calls, ["tail:4294967295:2"]);
    }
    #[test]
    fn would_block_does_not_disconnect() {
        let mut h = Host {
            received: -1,
            error: 10035,
            ..Host::default()
        };
        assert_eq!(receive_packet(&mut h, 0x100, 8, 0x200), 0);
        assert_eq!(h.calls, ["error"]);
    }
    #[test]
    fn messenger_special_and_error_cleanup() {
        let mut h = Host {
            received: 5,
            ..Host::default()
        };
        h.write32(0x108, u32::MAX);
        messenger_receive(&mut h, 0x100, 0x200);
        assert_eq!(h.calls, ["begin", "source", "special", "end"]);
        h.calls.clear();
        h.received = -1;
        messenger_receive(&mut h, 0x100, 0x200);
        assert_eq!(h.calls, ["begin", "error", "end"]);
    }
}
