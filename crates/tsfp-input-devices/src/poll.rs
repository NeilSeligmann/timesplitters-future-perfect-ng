// SPDX-License-Identifier: GPL-3.0-or-later
//! Poll counters of the pad source (`polls` and `port_polls[]` of `src/input/xinput_source.c`).
//! The clock of record and replay is the port 0 count, so these are the guest-visible poll indices.

pub const PORTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PollCounters {
    total: u64,
    ports: [u64; PORTS],
}

impl PollCounters {
    /// For statics.
    pub const fn default_const() -> Self {
        Self {
            total: 0,
            ports: [0; PORTS],
        }
    }

    /// Count one poll of `port`; the zero based index it had, or `None` for a port out of range.
    pub fn begin(&mut self, port: usize) -> Option<u64> {
        let slot = self.ports.get_mut(port)?;
        let index = *slot;
        *slot += 1;
        self.total += 1;
        Some(index)
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    /// Polls of `port` so far; 0 for a port out of range.
    pub fn port(&self, port: usize) -> u64 {
        self.ports.get(port).copied().unwrap_or(0)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_per_port_and_total() {
        let mut counters = PollCounters::default();
        assert_eq!(counters.begin(0), Some(0));
        assert_eq!(counters.begin(0), Some(1));
        assert_eq!(counters.begin(2), Some(0));
        assert_eq!(counters.begin(4), None);
        assert_eq!(
            (
                counters.total(),
                counters.port(0),
                counters.port(2),
                counters.port(9)
            ),
            (3, 2, 1, 0)
        );
        counters.reset();
        assert_eq!(counters.total(), 0);
    }
}
