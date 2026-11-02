// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct-IP datagram boundary for the T3891 session work.
//!
//! This transports the existing session frame unchanged. It does not perform
//! discovery, admission, reliability, simulation, or establish a game session.

use std::io::{self, ErrorKind};
use std::net::{SocketAddr, UdpSocket};
use tsfp_network::syslink_codec::vdp::{Frame, MAX_DATAGRAM};

pub mod session;
pub mod socket;

/// A nonblocking datagram endpoint; injectable so contract tests need no ports.
pub trait Datagram {
    fn send_to(&self, data: &[u8], destination: SocketAddr) -> io::Result<usize>;
    fn recv_from(&self, data: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
}

impl Datagram for UdpSocket {
    fn send_to(&self, data: &[u8], destination: SocketAddr) -> io::Result<usize> {
        UdpSocket::send_to(self, data, destination)
    }

    fn recv_from(&self, data: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, data)
    }
}

/// Endpoint addresses are explicit, allowing two local instances to use
/// distinct ports. The caller owns peer admission and lifecycle.
pub struct Transport<D> {
    socket: D,
}

impl Transport<UdpSocket> {
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self::new(socket))
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl<D: Datagram> Transport<D> {
    pub fn new(socket: D) -> Self {
        Self { socket }
    }

    pub fn send(&self, destination: SocketAddr, frame: &Frame) -> io::Result<()> {
        let bytes = frame.encode().map_err(|_| invalid_frame())?;
        // Decode applies the original receiver's datagram bounds and checks
        // the declared extent before anything is sent.
        Frame::decode(&bytes).map_err(|_| invalid_frame())?;
        match self.socket.send_to(&bytes, destination)? {
            n if n == bytes.len() => Ok(()),
            _ => Err(io::Error::new(ErrorKind::WriteZero, "partial UDP datagram")),
        }
    }

    /// `None` means the nonblocking socket is empty. Malformed and oversized
    /// input is reported to the caller instead of entering session state.
    pub fn receive(&self) -> io::Result<Option<(SocketAddr, Frame)>> {
        let mut bytes = [0; MAX_DATAGRAM + 1];
        let (len, source) = match self.socket.recv_from(&mut bytes) {
            Ok(value) => value,
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(None),
            Err(error) => return Err(error),
        };
        if len > MAX_DATAGRAM {
            return Err(invalid_frame());
        }
        let frame = Frame::decode(&bytes[..len]).map_err(|_| invalid_frame())?;
        Ok(Some((source, frame)))
    }
}

fn invalid_frame() -> io::Error {
    io::Error::new(ErrorKind::InvalidData, "invalid session datagram")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        received: RefCell<Option<(Vec<u8>, SocketAddr)>>,
        sent: RefCell<Vec<(Vec<u8>, SocketAddr)>>,
        short_write: bool,
    }

    impl Datagram for Fake {
        fn send_to(&self, bytes: &[u8], dest: SocketAddr) -> io::Result<usize> {
            self.sent.borrow_mut().push((bytes.to_vec(), dest));
            Ok(bytes.len() - usize::from(self.short_write))
        }
        fn recv_from(&self, bytes: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            let (data, source) = self.received.borrow_mut().take()
                .ok_or_else(|| io::Error::from(ErrorKind::WouldBlock))?;
            let len = data.len().min(bytes.len());
            bytes[..len].copy_from_slice(&data[..len]);
            Ok((len, source))
        }
    }

    fn addr() -> SocketAddr { "127.0.0.1:32123".parse().unwrap() }

    #[test]
    fn preserves_original_frame_and_source() {
        let io = Transport::new(Fake::default());
        let frame = Frame { header: vec![0xf8, 0xff, 0xff, 0xff], tail: vec![7] };
        io.send(addr(), &frame).unwrap();
        let (bytes, dest) = io.socket.sent.borrow()[0].clone();
        assert_eq!(dest, addr());
        assert_eq!(bytes, [4, 0, 0xf8, 0xff, 0xff, 0xff, 7]);
        *io.socket.received.borrow_mut() = Some((bytes, addr()));
        assert_eq!(io.receive().unwrap(), Some((addr(), frame)));
        assert_eq!(io.receive().unwrap(), None);
    }

    #[test]
    fn rejects_truncated_and_oversized_frames_and_short_sends() {
        let io = Transport::new(Fake::default());
        for bytes in [vec![9, 0, 1, 2], vec![0; MAX_DATAGRAM + 1]] {
            *io.socket.received.borrow_mut() = Some((bytes, addr()));
            assert_eq!(io.receive().unwrap_err().kind(), ErrorKind::InvalidData);
        }
        let oversized = Frame { header: vec![0; MAX_DATAGRAM], tail: vec![] };
        assert_eq!(io.send(addr(), &oversized).unwrap_err().kind(), ErrorKind::InvalidData);
        assert!(io.socket.sent.borrow().is_empty());
        let io = Transport::new(Fake { short_write: true, ..Fake::default() });
        let valid = Frame { header: vec![1, 2, 3, 4], tail: vec![] };
        assert_eq!(io.send(addr(), &valid).unwrap_err().kind(), ErrorKind::WriteZero);
    }
}
