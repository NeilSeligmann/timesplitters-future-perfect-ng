// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw datagrams for a game socket boundary. The game owns packet framing.

use crate::Datagram;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, UdpSocket};
use std::collections::HashMap;

/// Owns host sockets for guest socket handles. A failed bind does not install
/// an entry, and removing an entry closes its host socket when dropped.
pub struct SocketTable<D> {
    sockets: HashMap<u32, Socket<D>>,
}

impl<D> Default for SocketTable<D> {
    fn default() -> Self {
        Self { sockets: HashMap::new() }
    }
}

impl<D> SocketTable<D> {
    pub fn bind_with(
        &mut self,
        handle: u32,
        address: SocketAddr,
        bind: impl FnOnce(SocketAddr) -> io::Result<Socket<D>>,
    ) -> io::Result<()> {
        if self.sockets.contains_key(&handle) {
            return Err(io::Error::new(ErrorKind::AlreadyExists, "guest socket already bound"));
        }
        let socket = bind(address)?;
        self.sockets.insert(handle, socket);
        Ok(())
    }

    pub fn get(&self, handle: u32) -> Option<&Socket<D>> {
        self.sockets.get(&handle)
    }

    pub fn close(&mut self, handle: u32) -> bool {
        self.sockets.remove(&handle).is_some()
    }
}

/// A nonblocking UDP socket whose payload is not interpreted as a VDP frame.
pub struct Socket<D> {
    io: D,
}

impl Socket<UdpSocket> {
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let io = UdpSocket::bind(address)?;
        io.set_nonblocking(true)?;
        Ok(Self::new(io))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }
}

impl<D: Datagram> Socket<D> {
    pub fn new(io: D) -> Self {
        Self { io }
    }

    pub fn send(&self, bytes: &[u8], destination: SocketAddr) -> io::Result<()> {
        match self.io.send_to(bytes, destination)? {
            n if n == bytes.len() => Ok(()),
            _ => Err(io::Error::new(ErrorKind::WriteZero, "partial UDP datagram")),
        }
    }

    /// Returns the number of bytes and sender, or None if no packet is ready.
    /// The caller provides one extra byte when it must detect truncation.
    pub fn receive(&self, bytes: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self.io.recv_from(bytes) {
            Ok(packet) => Ok(Some(packet)),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake {
        packet: RefCell<Option<(Vec<u8>, SocketAddr)>>,
        short: bool,
    }

    impl Datagram for Fake {
        fn send_to(&self, bytes: &[u8], dest: SocketAddr) -> io::Result<usize> {
            *self.packet.borrow_mut() = Some((bytes.to_vec(), dest));
            Ok(bytes.len() - usize::from(self.short && !bytes.is_empty()))
        }
        fn recv_from(&self, bytes: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            let (packet, source) = self.packet.borrow_mut().take()
                .ok_or_else(|| io::Error::from(ErrorKind::WouldBlock))?;
            let len = bytes.len().min(packet.len());
            bytes[..len].copy_from_slice(&packet[..len]);
            Ok((len, source))
        }
    }

    #[test]
    fn raw_packets_preserve_bytes_and_address_and_report_empty() {
        let address = "127.0.0.1:3010".parse().unwrap();
        let socket = Socket::new(Fake { packet: RefCell::new(None), short: false });
        let payload = [0xff, 0, 7, 0, 2];
        socket.send(&payload, address).unwrap();
        let mut received = [0; 8];
        assert_eq!(socket.receive(&mut received).unwrap(), Some((payload.len(), address)));
        assert_eq!(&received[..payload.len()], payload);
        assert_eq!(socket.receive(&mut received).unwrap(), None);
    }

    #[test]
    fn short_sends_fail() {
        let socket = Socket::new(Fake { packet: RefCell::new(None), short: true });
        assert_eq!(socket.send(&[1], "127.0.0.1:3010".parse().unwrap()).unwrap_err().kind(), ErrorKind::WriteZero);
    }

    #[test]
    fn lifecycle_does_not_replace_or_leak_failed_bindings() {
        let address = "127.0.0.1:3010".parse().unwrap();
        let mut table = SocketTable::default();
        let failed: io::Result<()> = table.bind_with(1, address, |_| Err(io::Error::from(ErrorKind::AddrInUse)));
        assert_eq!(failed.unwrap_err().kind(), ErrorKind::AddrInUse);
        assert!(table.get(1).is_none());
        table.bind_with(1, address, |_| Ok(Socket::new(Fake { packet: RefCell::new(None), short: false }))).unwrap();
        let duplicate = table.bind_with(1, address, |_| panic!("duplicate must not bind"));
        assert_eq!(duplicate.unwrap_err().kind(), ErrorKind::AlreadyExists);
        assert!(table.close(1));
        assert!(!table.close(1));
        assert!(table.get(1).is_none());
    }
}
