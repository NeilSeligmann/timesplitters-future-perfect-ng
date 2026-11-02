//! Rust-owned attempt and observer state for the original file-I/O preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptIndex(pub usize);
#[derive(Clone, Debug)]
pub struct Attempt {
    pub path: Vec<u8>,
    pub access: u32,
    pub share: u32,
    pub options: u32,
    pub attempts: u32,
    pub opened: bool,
}
pub struct Attempts {
    records: Vec<Attempt>,
    capacity: usize,
}
impl Attempts {
    pub const fn new(capacity: usize) -> Self {
        Self {
            records: Vec::new(),
            capacity,
        }
    }
    pub fn reset(&mut self) {
        self.records.clear();
    }
    pub fn count(&self) -> usize {
        self.records.len()
    }
    pub fn get(&self, index: AttemptIndex) -> Option<&Attempt> {
        self.records.get(index.0)
    }
    pub fn opened(&mut self, index: AttemptIndex) {
        if let Some(record) = self.records.get_mut(index.0) {
            record.opened = true;
        }
    }
    pub fn record(
        &mut self,
        path: &[u8],
        access: u32,
        share: u32,
        options: u32,
    ) -> Option<AttemptIndex> {
        for (index, record) in self.records.iter_mut().enumerate() {
            if record.path.eq_ignore_ascii_case(path) {
                record.attempts = record.attempts.wrapping_add(1);
                return Some(AttemptIndex(index));
            }
        }
        if self.records.len() >= self.capacity {
            return None;
        }
        let index = AttemptIndex(self.records.len());
        self.records.push(Attempt {
            path: path.to_vec(),
            access,
            share,
            options,
            attempts: 1,
            opened: false,
        });
        Some(index)
    }
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum EventKind {
    Open = 1,
    Read = 2,
    Write = 3,
}
pub struct Event<'a> {
    pub kind: EventKind,
    pub path: &'a [u8],
    pub bytes: u64,
}
pub struct Observer<T> {
    callback: Option<T>,
}
impl<T: Copy> Observer<T> {
    pub const fn new() -> Self {
        Self { callback: None }
    }
    pub fn set(&mut self, callback: Option<T>) {
        self.callback = callback;
    }
    pub fn callback(&self) -> Option<T> {
        self.callback
    }
}
impl<T: Copy> Default for Observer<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// What an open of an undeclared, unbacked name does (`kernel_file_missing_policy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum MissingPolicy {
    /// Not-found status: the default.
    Fail = 0,
    /// A zero-length file is issued anyway (FABRICATED, opt-in).
    Empty = 1,
}

impl MissingPolicy {
    /// Unknown raw values keep the original's "anything but FAIL is lenient" reading.
    pub fn from_raw(raw: u32) -> Self {
        if raw == 0 { Self::Fail } else { Self::Empty }
    }
}

/// Names declared openable by the host, matched case-insensitively over ASCII.
/// Entries are never removed singly, only by `reset`, so a Vec keeps the C slot order.
pub struct Openables {
    names: Vec<Vec<u8>>,
    capacity: usize,
    path_max: usize,
    policy: MissingPolicy,
}

impl Openables {
    pub const fn new(capacity: usize, path_max: usize) -> Self {
        Self {
            names: Vec::new(),
            capacity,
            path_max,
            policy: MissingPolicy::Fail,
        }
    }
    /// Clears the names and restores the FAIL policy, as `kernel_file_reset` does.
    pub fn reset(&mut self) {
        self.names.clear();
        self.policy = MissingPolicy::Fail;
    }
    /// Declares `path`. A repeat (any case) overwrites the stored spelling and takes no
    /// new slot. Empty, overlong or table-full declarations are refused.
    pub fn add(&mut self, path: &[u8]) -> bool {
        if path.is_empty() || path.len() >= self.path_max {
            return false;
        }
        if let Some(existing) = self
            .names
            .iter_mut()
            .find(|name| name.eq_ignore_ascii_case(path))
        {
            *existing = path.to_vec();
            return true;
        }
        if self.names.len() >= self.capacity {
            return false;
        }
        self.names.push(path.to_vec());
        true
    }
    pub fn count(&self) -> usize {
        self.names.len()
    }
    pub fn contains(&self, path: &[u8]) -> bool {
        self.names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(path))
    }
    pub fn policy(&self) -> MissingPolicy {
        self.policy
    }
    pub fn set_policy(&mut self, policy: MissingPolicy) {
        self.policy = policy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openables_match_case_insensitively_and_bound() {
        let mut table = Openables::new(2, 8);
        assert!(!table.add(b""));
        assert!(!table.add(b"12345678"));
        assert!(table.add(b"D:\\A"));
        assert!(table.add(b"d:\\a"));
        assert_eq!(table.count(), 1);
        assert!(table.contains(b"D:\\a"));
        assert!(!table.contains(b"D:\\a\xc4"));
        assert!(table.add(b"D:\\B"));
        assert!(!table.add(b"D:\\C"));
        assert!(table.add(b"d:\\B"));
        assert_eq!(table.count(), 2);
        table.set_policy(MissingPolicy::from_raw(7));
        assert_eq!(table.policy(), MissingPolicy::Empty);
        table.reset();
        assert_eq!((table.count(), table.policy()), (0, MissingPolicy::Fail));
    }
}
