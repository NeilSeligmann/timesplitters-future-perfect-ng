// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit-address join adapter. This is not yet connected to game state.

use std::collections::HashMap;
use std::net::SocketAddr;
use tsfp_network::syslink_codec::vdp::{Control, Frame};
use tsfp_network::syslink_session::Lobby;

/// An outbound message; the caller sends it through `Transport`.
#[derive(Debug, PartialEq, Eq)]
pub struct Outbound {
    pub destination: SocketAddr,
    pub frame: Frame,
}

fn message(destination: SocketAddr, control: Control) -> Outbound {
    Outbound {
        destination,
        frame: Frame {
            header: control.encode_header(),
            tail: Vec::new(),
        },
    }
}

/// Host owns slot assignment. Repeated requests from an address receive the
/// same answer so lost replies cannot exhaust the lobby.
pub struct Host {
    pub lobby: Lobby,
    peers: HashMap<SocketAddr, Control>,
}

impl Host {
    pub fn new(players: u32) -> Self {
        Self {
            lobby: Lobby::new_host(players),
            peers: HashMap::new(),
        }
    }

    pub fn receive(&mut self, source: SocketAddr, frame: &Frame) -> Option<Outbound> {
        let Control::JoinRequest { players, .. } = Control::decode_header(&frame.header).ok()?
        else {
            return None;
        };
        let reply = if let Some(reply) = self.peers.get(&source) {
            reply.clone()
        } else {
            let reply = self.lobby.host_on_join_request(players);
            if matches!(reply, Control::JoinAccept { .. }) {
                self.peers.insert(source, reply.clone());
            }
            reply
        };
        Some(message(source, reply))
    }

    pub fn remove(&mut self, source: SocketAddr) {
        if let Some(Control::JoinAccept { assigned_slot, .. }) = self.peers.remove(&source) {
            self.lobby.release(assigned_slot as usize);
        }
    }
}

/// Client retries on a caller-supplied monotonic timebase; no wall clock is used.
pub struct Client {
    pub lobby: Lobby,
    host: SocketAddr,
    players: u32,
    user_tag: i32,
    user_name: Vec<u8>,
    next_retry: u64,
    retry_interval: u64,
    denied: bool,
}

impl Client {
    pub fn new(
        host: SocketAddr,
        players: u32,
        user_tag: i32,
        user_name: Vec<u8>,
        retry_interval: u64,
    ) -> Self {
        assert!(retry_interval > 0);
        Self {
            lobby: Lobby::new_client(),
            host,
            players,
            user_tag,
            user_name,
            next_retry: 0,
            retry_interval,
            denied: false,
        }
    }

    pub fn poll(&mut self, now: u64) -> Option<Outbound> {
        if self.lobby.join_complete || self.denied || now < self.next_retry {
            return None;
        }
        self.next_retry = now.saturating_add(self.retry_interval);
        Some(message(
            self.host,
            Control::JoinRequest {
                players: self.players,
                user_tag: self.user_tag,
                user_name: self.user_name.clone(),
            },
        ))
    }

    pub fn receive(&mut self, source: SocketAddr, frame: &Frame) {
        if source != self.host || self.lobby.join_complete || self.denied {
            return;
        }
        match Control::decode_header(&frame.header) {
            Ok(Control::JoinAccept {
                assigned_slot,
                host_slot,
                host_players,
            }) if assigned_slot != host_slot => {
                let _ = self.lobby.client_on_join_accept(
                    assigned_slot,
                    host_slot,
                    host_players,
                    self.players,
                );
            }
            Ok(Control::JoinDeny) => {
                self.lobby.client_on_join_deny();
                self.denied = true;
            }
            _ => {}
        }
    }

    pub fn denied(&self) -> bool {
        self.denied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_are_idempotent_and_accept_only_the_selected_host() {
        let host_addr = "127.0.0.1:1000".parse().unwrap();
        let client_addr = "127.0.0.1:1001".parse().unwrap();
        let impostor = "127.0.0.1:1002".parse().unwrap();
        let mut host = Host::new(1);
        let mut client = Client::new(host_addr, 1, -1, b"player".to_vec(), 2);
        let request = client.poll(0).unwrap();
        let reply = host.receive(client_addr, &request.frame).unwrap();
        assert!(client.poll(1).is_none());
        let retry = client.poll(2).unwrap();
        assert_eq!(host.receive(client_addr, &retry.frame).unwrap(), reply);
        assert_eq!(host.lobby.connected_count(), 2);
        client.receive(impostor, &reply.frame);
        assert!(!client.lobby.join_complete);
        client.receive(host_addr, &reply.frame);
        assert!(client.lobby.join_complete);
        assert!(client.poll(4).is_none());
        host.remove(client_addr);
        assert_eq!(host.lobby.connected_count(), 1);
    }

    #[test]
    fn invalid_accept_does_not_complete_join_and_denial_stops_retries() {
        let host = "127.0.0.1:1000".parse().unwrap();
        let mut client = Client::new(host, 1, -1, Vec::new(), 2);
        let same_slot = message(
            host,
            Control::JoinAccept {
                assigned_slot: 0,
                host_slot: 0,
                host_players: 1,
            },
        );
        client.receive(host, &same_slot.frame);
        assert!(!client.lobby.join_complete);
        let invalid_slot = message(
            host,
            Control::JoinAccept {
                assigned_slot: u32::MAX,
                host_slot: 0,
                host_players: 1,
            },
        );
        client.receive(host, &invalid_slot.frame);
        assert!(!client.lobby.join_complete);
        client.receive(host, &message(host, Control::JoinDeny).frame);
        assert!(client.denied());
        assert!(client.poll(100).is_none());
    }
}
