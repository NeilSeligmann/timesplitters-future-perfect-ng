//! T2207: original C clock policy in typed Rust. See docs/t-contract-kernel-clock.md.
//! Tier 1 synthetic differential for Clock; timers/calendar/pacing are spec-tested.

pub mod guest;
pub mod timers;
/// Default-off A/B selection marker (T2207), mirrors `rust-kernel-sync`.
pub const RUST_KERNEL_CLOCK_ENABLED: bool = cfg!(feature = "rust-kernel-clock");
pub const FREQUENCY: u64 = 733_333_333;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub total: u64,
    pub floor: u64,
    pub reads: u64,
    pub reads_above_floor: u64,
    pub frames: u64,
    pub stall_ticks: u64,
    pub advance_ticks: u64,
    pub advance_calls: u64,
}
/// Counter policy shared by frame scheduling and guest time reads.
///
/// A frame raises the minimum visible counter value; a read advances by 1000
/// ticks and catches up to that minimum when necessary.
///
/// ```
/// use tsfp_kernel_clock::{Clock, FREQUENCY};
/// let mut clock = Clock::default();
/// assert_eq!(clock.frame(60), Some(FREQUENCY / 60));
/// assert_eq!(clock.read(), FREQUENCY / 60);
/// assert_eq!(clock.stats().frames, 1);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Clock {
    now: u64,
    floor: u64,
    frame_carry: u64,
    rate: u32,
    stall_carry: u64,
    stats: Stats,
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Read,
    Peek,
    Frame(u32),
    Stall(u32),
    Advance(u64),
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub result: u64,
    pub stats: Stats,
    pub milliseconds: u32,
}
pub trait ClockBackend {
    fn apply(&mut self, operation: Operation) -> Snapshot;
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    #[default]
    C,
    Rust,
}
/// Default-off A/B boundary. The caller supplies its C oracle adapter.
pub struct Selection<C> {
    pub backend: Backend,
    pub c: C,
    pub rust: Clock,
}
impl<C: ClockBackend> Selection<C> {
    pub fn new(c: C) -> Self {
        Self {
            backend: Backend::default(),
            c,
            rust: Clock::default(),
        }
    }
    pub fn apply(&mut self, op: Operation) -> Snapshot {
        match self.backend {
            Backend::C => self.c.apply(op),
            Backend::Rust => self.rust.apply(op),
        }
    }
}
impl Clock {
    pub fn peek(&self) -> u64 {
        self.now.max(self.floor)
    }
    pub fn read(&mut self) -> u64 {
        self.stats.reads = self.stats.reads.wrapping_add(1);
        self.now = self.now.wrapping_add(1000);
        if self.now < self.floor {
            self.now = self.floor;
        } else {
            self.stats.reads_above_floor = self.stats.reads_above_floor.wrapping_add(1);
        }
        self.now
    }
    pub fn frame(&mut self, rate: u32) -> Option<u64> {
        if !(1..=1000).contains(&rate) {
            return None;
        }
        if self.rate != rate {
            self.rate = rate;
            self.frame_carry = 0;
        }
        self.stats.frames = self.stats.frames.wrapping_add(1);
        self.floor = self.floor.wrapping_add(FREQUENCY / u64::from(rate));
        self.frame_carry += FREQUENCY % u64::from(rate);
        if self.frame_carry >= u64::from(rate) {
            self.frame_carry -= u64::from(rate);
            self.floor = self.floor.wrapping_add(1);
        }
        Some(self.floor)
    }
    pub fn stall(&mut self, us: u32) -> u64 {
        if us > 1_000_000 {
            return 0;
        }
        self.now = self.peek();
        let scaled = u64::from(us) * FREQUENCY + self.stall_carry;
        let ticks = scaled / 1_000_000;
        self.stall_carry = scaled % 1_000_000;
        self.stats.stall_ticks = self.stats.stall_ticks.wrapping_add(ticks);
        self.now = self.now.wrapping_add(ticks);
        ticks
    }
    pub fn advance(&mut self, ticks: u64) -> u64 {
        self.now = self.peek();
        if self.now < ticks {
            self.stats.advance_ticks = self.stats.advance_ticks.wrapping_add(ticks - self.now);
            self.stats.advance_calls = self.stats.advance_calls.wrapping_add(1);
            self.now = ticks;
        }
        self.now
    }
    pub fn milliseconds(&self) -> u32 {
        let ticks = self.peek();
        ((ticks / FREQUENCY)
            .wrapping_mul(1000)
            .wrapping_add((ticks % FREQUENCY) * 1000 / FREQUENCY)) as u32
    }
    pub fn elapsed_units(&self) -> u64 {
        let ticks = self.peek();
        (ticks / FREQUENCY)
            .wrapping_mul(10_000_000)
            .wrapping_add((ticks % FREQUENCY) * 10_000_000 / FREQUENCY)
    }
    pub fn stats(&self) -> Stats {
        Stats {
            total: self.peek(),
            floor: self.floor,
            ..self.stats
        }
    }
}
impl ClockBackend for Clock {
    fn apply(&mut self, op: Operation) -> Snapshot {
        let result = match op {
            Operation::Read => self.read(),
            Operation::Peek => self.peek(),
            Operation::Frame(rate) => self.frame(rate).unwrap_or(u64::MAX),
            Operation::Stall(us) => self.stall(us),
            Operation::Advance(ticks) => self.advance(ticks),
            Operation::Reset => {
                *self = Self::default();
                0
            }
        };
        Snapshot {
            result,
            stats: self.stats(),
            milliseconds: self.milliseconds(),
        }
    }
}
/// Host samples are injected (100ns since 1601); None represents clock_gettime failure.
#[derive(Debug, Default)]
pub struct Calendar {
    pub offset: i64,
    pub last: u64,
    pub sets: u32,
}
impl Calendar {
    pub fn query(&mut self, sample: Option<u64>) -> u64 {
        let mut value = sample.map_or(self.last.wrapping_add(1), |v| {
            v.wrapping_add(self.offset as u64)
        });
        if value <= self.last {
            value = self.last.wrapping_add(1);
        }
        self.last = value;
        value
    }
    pub fn set(&mut self, desired: u64, sample: Option<u64>) -> bool {
        let Some(sample) = sample else {
            return false;
        };
        self.offset = desired.wrapping_sub(sample) as i64;
        self.last = 0;
        self.sets = self.sets.wrapping_add(1);
        true
    }
}
/// FABRICATED interactive pacing, deliberately separate from guest Clock.
#[derive(Debug, Default)]
pub struct Presenter {
    frames: u64,
}
impl Presenter {
    pub fn next_deadline_ns(&mut self) -> u128 {
        self.frames += 1;
        u128::from(self.frames) * 1_001_000_000_000 / 60_000
    }
}
