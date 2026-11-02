//! Read completion contract (T2210). Timing reproduces the C host model, not hardware.
use std::collections::HashMap;

pub const FREQUENCY: u64 = 733_333_333;
pub const INVALID_PARAMETER: u32 = 0xc000_000d;
const MAX_REUSABLE_BUFFERS: usize = 8;
const MAX_REUSABLE_BUFFER_CAPACITY: usize = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Volume {
    Disc,
    Hdd,
}
impl Volume {
    pub fn of_path(path: &[u8]) -> Self {
        let path = path.to_ascii_uppercase();
        if path.starts_with(b"\\DEVICE\\HARDDISK") {
            return Self::Hdd;
        }
        for root in [b"\\??\\".as_slice(), b"\\DOSDEVICES\\", b""] {
            if let Some(rest) = path.strip_prefix(root) {
                if rest.len() >= 2 && rest[1] == b':' && b"CEFXYZ".contains(&rest[0]) {
                    return Self::Hdd;
                }
            }
        }
        Self::Disc
    }
    pub fn service_ticks(self, bytes: u32) -> u64 {
        let (access, rate) = match self {
            Self::Disc => (115, 1_000_000_000),
            Self::Hdd => (85, 1_200_000_000),
        };
        (access * FREQUENCY).div_ceil(1_000_000) + (u64::from(bytes) * FREQUENCY).div_ceil(rate)
    }
}
#[derive(Clone, Debug)]
pub struct Read {
    pub file: u32,
    pub event: u32,
    pub iosb: u32,
    pub buffer: u32,
    pub data: Vec<u8>,
    pub requested: u32,
    pub volume: Volume,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Stats {
    pub submitted: u32,
    pub completed: u32,
    pub pending: u32,
    pub refused: u32,
    pub failed: u32,
    pub bytes: u64,
    pub file_object_reads: u32,
    pub waits: u32,
    pub wait_timeouts: u32,
    pub apc_refused: u32,
    pub spin_completions: u32,
    pub elapsed_completions: u32,
    pub elapsed_calls: u32,
    pub elapsed_backwards: u32,
    pub elapsed_rebases: u32,
}
/// Operations crossing the file/clock/object/memory boundary. Publication methods
/// are separate so their observable order can be tested and retained by a new host.
pub trait CompletionHost {
    fn now(&self) -> u64;
    fn advance_to(&mut self, ticks: u64);
    fn identity(&self, file: u32) -> u32;
    fn identity_live(&self, identity: u32) -> bool;
    fn write_buffer(&mut self, address: u32, bytes: &[u8]) -> bool;
    fn write_iosb(&mut self, address: u32, status: u32, information: u32);
    fn signal_event(&mut self, event: u32);
    fn event_signalled(&self, event: u32) -> bool;
}
#[derive(Clone, Debug)]
struct Pending {
    read: Read,
    identity: u32,
    due: u64,
}
#[derive(Clone, Copy, Debug)]
struct Epoch {
    start_wall: u64,
    last_wall: u64,
    start_model: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Wait {
    None = 0,
    Satisfied = 1,
    TimedOut = 2,
}
#[derive(Default)]
pub struct AsyncIo {
    pub enabled: bool,
    pub file_object_enabled: bool,
    queue: Vec<Pending>,
    reuse_owned_buffers: bool,
    reusable_buffers: Vec<Vec<u8>>,
    files: HashMap<u32, bool>,
    // Keep slot order: the C oracle picks the first dead identity for reclamation.
    file_order: Vec<u32>,
    drive_free: u64,
    epoch: Option<Epoch>,
    stats: Stats,
}
impl AsyncIo {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.epoch = None;
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Enable reuse for callers that explicitly opt into the Rust-owned asset path.
    /// The default C-host completion preset keeps its existing allocation behavior.
    pub fn enable_owned_buffer_reuse(&mut self) {
        self.reuse_owned_buffers = true;
    }
    /// Return an empty buffer with at least `capacity` bytes of storage.
    pub fn take_owned_buffer(&mut self, capacity: usize) -> Vec<u8> {
        if let Some(index) = self
            .reusable_buffers
            .iter()
            .position(|buffer| buffer.capacity() >= capacity)
        {
            let mut buffer = self.reusable_buffers.swap_remove(index);
            buffer.clear();
            buffer
        } else {
            Vec::with_capacity(capacity)
        }
    }
    fn recycle_owned_buffer(&mut self, mut buffer: Vec<u8>) {
        if self.reuse_owned_buffers
            && self.reusable_buffers.len() < MAX_REUSABLE_BUFFERS
            && buffer.capacity() <= MAX_REUSABLE_BUFFER_CAPACITY
        {
            buffer.clear();
            self.reusable_buffers.push(buffer);
        }
    }
    pub fn eligible(&self, options: u32, event: u32) -> bool {
        options & 0x30 == 0
            && options & 8 != 0
            && self.enabled
            && (event != 0 || self.file_object_enabled)
    }
    pub fn apc_refused(&mut self, options: u32, apc: u32) -> bool {
        let refused = apc != 0 && options & 0x30 == 0 && self.enabled;
        if refused {
            self.stats.apc_refused = self.stats.apc_refused.wrapping_add(1);
        }
        refused
    }
    /// On refusal return the owned request intact, like the C caller retaining its allocation.
    pub fn submit(&mut self, host: &impl CompletionHost, read: Read) -> Result<(), Read> {
        if self.queue.len() == 32 {
            self.stats.refused = self.stats.refused.wrapping_add(1);
            return Err(read);
        }
        let identity = if read.event == 0 {
            host.identity(read.file)
        } else {
            0
        };
        if read.event == 0 {
            if !self.files.contains_key(&identity) {
                if self.files.len() == 64 {
                    let reusable = self.file_order.iter().copied().find(|id| {
                        !host.identity_live(*id) && !self.queue.iter().any(|p| p.identity == *id)
                    });
                    let Some(old) = reusable else {
                        self.stats.refused = self.stats.refused.wrapping_add(1);
                        return Err(read);
                    };
                    self.files.remove(&old);
                    let slot = self.file_order.iter().position(|id| *id == old).unwrap();
                    self.file_order[slot] = identity;
                } else {
                    self.file_order.push(identity);
                }
            }
            self.files.insert(identity, false);
            self.stats.file_object_reads = self.stats.file_object_reads.wrapping_add(1);
        }
        if self.queue.is_empty() {
            self.epoch = None;
        }
        let due = self
            .drive_free
            .max(host.now())
            .wrapping_add(read.volume.service_ticks(read.data.len() as u32));
        self.drive_free = due;
        self.queue.push(Pending {
            read,
            identity,
            due,
        });
        self.stats.submitted = self.stats.submitted.wrapping_add(1);
        self.stats.pending += 1;
        Ok(())
    }
    fn earliest(&self) -> Option<usize> {
        self.queue
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.due)
            .map(|(i, _)| i)
    }
    pub fn service(&mut self, host: &mut impl CompletionHost) -> u32 {
        let now = host.now();
        let mut completed = 0;
        while let Some(index) = self.earliest() {
            if self.queue[index].due > now {
                break;
            }
            let pending = self.queue.remove(index);
            let read = pending.read;
            let mut status = 0;
            let mut information = read.data.len() as u32;
            if !read.data.is_empty() {
                if host.write_buffer(read.buffer, &read.data) {
                    self.stats.bytes = self.stats.bytes.wrapping_add(u64::from(information));
                } else {
                    status = INVALID_PARAMETER;
                    information = 0;
                    self.stats.failed = self.stats.failed.wrapping_add(1);
                }
            }
            if read.iosb != 0 {
                host.write_iosb(read.iosb, status, information);
            }
            if read.event == 0 {
                if let Some(signal) = self.files.get_mut(&pending.identity) {
                    *signal = true;
                }
            } else {
                host.signal_event(read.event);
            }
            self.stats.completed = self.stats.completed.wrapping_add(1);
            self.stats.pending -= 1;
            completed += 1;
            self.recycle_owned_buffer(read.data);
        }
        completed
    }
    pub fn service_elapsed(&mut self, host: &mut impl CompletionHost, wall: u64) -> u32 {
        if !self.enabled || self.queue.is_empty() {
            self.epoch = None;
            return 0;
        }
        self.stats.elapsed_calls = self.stats.elapsed_calls.wrapping_add(1);
        let now = host.now();
        let epoch = self.epoch.get_or_insert(Epoch {
            start_wall: wall,
            last_wall: wall,
            start_model: now,
        });
        if wall < epoch.last_wall {
            self.stats.elapsed_backwards = self.stats.elapsed_backwards.wrapping_add(1);
            return 0;
        }
        epoch.last_wall = wall;
        let delta = wall - epoch.start_wall;
        let ticks = (u128::from(delta) * u128::from(FREQUENCY) / 1_000_000_000)
            .min(u128::from(u64::MAX)) as u64;
        let mut target = epoch.start_model.saturating_add(ticks);
        if now > target {
            target = now;
            epoch.start_model = now;
            epoch.start_wall = wall;
            self.stats.elapsed_rebases = self.stats.elapsed_rebases.wrapping_add(1);
        }
        host.advance_to(target);
        let count = self.service(host);
        self.stats.elapsed_completions = self.stats.elapsed_completions.wrapping_add(count);
        count
    }
    pub fn wait(
        &mut self,
        host: &mut impl CompletionHost,
        event: bool,
        key: u32,
        timeout: Option<u64>,
    ) -> Wait {
        if !self.enabled {
            return Wait::None;
        }
        let span = timeout.map_or(u64::MAX, |value| {
            if value > u64::MAX / FREQUENCY {
                u64::MAX
            } else {
                value.wrapping_mul(FREQUENCY).wrapping_add(9_999_999) / 10_000_000
            }
        });
        let deadline = host.now().saturating_add(span);
        let mut advanced = false;
        let result = loop {
            self.service(host);
            let signal = if event {
                host.event_signalled(key)
            } else {
                self.files.get(&key).copied().unwrap_or(false)
            };
            if signal {
                break Wait::Satisfied;
            }
            let due = self
                .queue
                .iter()
                .filter(|p| {
                    if event {
                        p.read.event == key
                    } else {
                        p.read.event == 0 && p.identity == key
                    }
                })
                .map(|p| p.due)
                .min();
            let Some(due) = due else {
                break Wait::None;
            };
            if due > deadline {
                host.advance_to(deadline);
                self.service(host);
                break Wait::TimedOut;
            }
            host.advance_to(due);
            advanced = true;
        };
        if result == Wait::TimedOut {
            self.stats.wait_timeouts = self.stats.wait_timeouts.wrapping_add(1);
        } else if result == Wait::Satisfied && advanced {
            self.stats.waits = self.stats.waits.wrapping_add(1);
        }
        result
    }
    pub fn complete_next(&mut self, host: &mut impl CompletionHost) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(index) = self.earliest() else {
            return false;
        };
        host.advance_to(self.queue[index].due);
        let count = self.service(host);
        self.stats.spin_completions = self.stats.spin_completions.wrapping_add(count);
        count != 0
    }
    pub fn reset(&mut self) {
        let (enabled, file_object_enabled) = (self.enabled, self.file_object_enabled);
        *self = Self {
            enabled,
            file_object_enabled,
            ..Self::default()
        };
    }
}
