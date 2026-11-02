// SPDX-License-Identifier: GPL-3.0-or-later
//! Multiport host adapter: per-connection opaque handles, independent raw report counters and
//! pending feedback. Models guest contracts, not USB timing or interrupts.

use crate::pad::GAMEPAD_TYPE;

pub const HOST_HANDLES: usize = 64;
pub const PORTS: usize = 4;
const TOKEN_BASE: u32 = 0x5860_0000;
const TOKEN_SEQUENCE_LIMIT: u32 = 0x7_FFFF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Handle {
    token: u32,
    port: u32,
    removed: bool,
    report_base: u32,
    pending_feedback: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    GenerationExhausted,
    TooManyHandles,
}

#[derive(Clone, Debug)]
pub struct HostTable {
    handles: [Handle; HOST_HANDLES],
    token_sequence: u32,
    active: [u32; PORTS],
}

impl Default for HostTable {
    fn default() -> Self {
        Self {
            handles: [Handle::default(); HOST_HANDLES],
            token_sequence: 0,
            active: [0; PORTS],
        }
    }
}

impl HostTable {
    /// Fresh guest address space: no handle survives, the token sequence continues.
    pub fn reset(&mut self) {
        self.handles = [Handle::default(); HOST_HANDLES];
        self.active = [0; PORTS];
    }

    fn index(&self, token: u32) -> Option<usize> {
        (token != 0)
            .then(|| self.handles.iter().position(|h| h.token == token))
            .flatten()
    }

    pub fn contains(&self, token: u32) -> bool {
        self.index(token).is_some()
    }

    /// Whether an Open of this shape yields a handle (the C adapter then calls [`open`](Self::open)).
    pub fn open_allowed(
        &self,
        device_type: u32,
        port: u32,
        slot: u32,
        port_is_synthetic: bool,
    ) -> bool {
        device_type == GAMEPAD_TYPE
            && (port as usize) < PORTS
            && slot != 1
            && port_is_synthetic
            && self.active[port as usize] == 0
    }

    pub fn open(&mut self, port: u32, report_changes: u32) -> Result<u32, OpenError> {
        let free = self
            .handles
            .iter()
            .position(|h| h.token == 0)
            .ok_or(OpenError::TooManyHandles)?;
        if self.token_sequence >= TOKEN_SEQUENCE_LIMIT {
            return Err(OpenError::GenerationExhausted);
        }
        self.token_sequence += 1;
        let token = TOKEN_BASE | (self.token_sequence << 2) | port;
        self.handles[free] = Handle {
            token,
            port,
            removed: false,
            report_base: report_changes,
            pending_feedback: 0,
        };
        self.active[port as usize] = token;
        Ok(token)
    }

    /// Close: returns the pending feedback address to cancel and the port to silence, if the
    /// handle was the active one.
    pub fn close(&mut self, token: u32) -> Option<(u32, Option<u32>)> {
        let at = self.index(token)?;
        let handle = self.handles[at];
        let silence = (self.active[handle.port as usize] == token).then(|| {
            self.active[handle.port as usize] = 0;
            handle.port
        });
        self.handles[at] = Handle::default();
        Some((handle.pending_feedback, silence))
    }

    pub fn is_removed(&self, token: u32) -> bool {
        self.index(token).is_some_and(|at| self.handles[at].removed)
    }

    pub fn port_of(&self, token: u32) -> Option<u32> {
        self.index(token).map(|at| self.handles[at].port)
    }

    /// Packet number of an open present handle: 1 plus the changes since its Open.
    pub fn packet(&self, token: u32, report_changes: u32) -> u32 {
        self.index(token).map_or(0, |at| {
            1u32.wrapping_add(report_changes)
                .wrapping_sub(self.handles[at].report_base)
        })
    }

    pub fn report_base(&self, token: u32) -> Option<u32> {
        self.index(token).map(|at| self.handles[at].report_base)
    }

    /// Pending feedback address of the handle to complete (0 when none); clears it.
    pub fn take_pending(&mut self, token: u32) -> u32 {
        self.index(token).map_or(0, |at| {
            std::mem::take(&mut self.handles[at].pending_feedback)
        })
    }

    pub fn set_pending(&mut self, token: u32, address: u32) {
        if let Some(at) = self.index(token) {
            self.handles[at].pending_feedback = address;
        }
    }

    /// Hot unplug of `port`: every live handle on it becomes removed. Returns the pending
    /// feedback addresses to cancel (zero entries are skipped).
    pub fn disconnect(&mut self, port: u32) -> Vec<u32> {
        let mut cancelled = Vec::new();
        for handle in self
            .handles
            .iter_mut()
            .filter(|h| h.token != 0 && h.port == port && !h.removed)
        {
            if handle.pending_feedback != 0 {
                cancelled.push(std::mem::take(&mut handle.pending_feedback));
            }
            handle.removed = true;
        }
        if let Some(active) = self.active.get_mut(port as usize) {
            *active = 0;
        }
        cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_and_unplug() {
        let mut table = HostTable::default();
        assert!(!table.open_allowed(GAMEPAD_TYPE, 4, 0, true));
        assert!(!table.open_allowed(GAMEPAD_TYPE, 0, 1, true));
        assert!(!table.open_allowed(GAMEPAD_TYPE, 0, 0, false));
        assert!(table.open_allowed(GAMEPAD_TYPE, 2, 0, true));
        let token = table.open(2, 10).unwrap();
        assert_eq!(token, 0x5860_0000 | (1 << 2) | 2);
        assert!(!table.open_allowed(GAMEPAD_TYPE, 2, 0, true));
        assert_eq!(table.packet(token, 13), 4);
        table.set_pending(token, 0x200);
        assert_eq!(table.disconnect(2), vec![0x200]);
        assert!(table.is_removed(token));
        assert!(table.open_allowed(GAMEPAD_TYPE, 2, 0, true));
        assert_eq!(table.close(token), Some((0, None)));
        assert!(!table.contains(token));
        assert_eq!(table.close(token), None);
    }

    #[test]
    fn close_of_active_handle_silences_port() {
        let mut table = HostTable::default();
        let token = table.open(1, 0).unwrap();
        table.set_pending(token, 0x300);
        assert_eq!(table.close(token), Some((0x300, Some(1))));
    }

    #[test]
    fn handle_exhaustion() {
        let mut table = HostTable::default();
        for port in 0..4u32 {
            table.open(port, 0).unwrap();
        }
        // ports are exclusive in the C adapter; the raw table still fills at 64 handles
        for _ in 4..HOST_HANDLES {
            table.open(0, 0).unwrap();
        }
        assert_eq!(table.open(0, 0), Err(OpenError::TooManyHandles));
        table.reset();
        assert!(table.open(0, 0).is_ok());
    }
}
