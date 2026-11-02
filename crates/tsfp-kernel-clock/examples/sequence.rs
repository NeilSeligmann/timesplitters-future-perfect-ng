use std::io::{self, BufRead};
use tsfp_kernel_clock::{Clock, ClockBackend, Operation};
fn main() {
    let mut clock = Clock::default();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let mut words = line.split_whitespace();
        let name = words.next().unwrap();
        let value: u64 = words.next().unwrap_or("0").parse().unwrap();
        let op = match name {
            "read" => Operation::Read,
            "peek" => Operation::Peek,
            "frame" => Operation::Frame(value as u32),
            "stall" => Operation::Stall(value as u32),
            "advance" => Operation::Advance(value),
            "reset" => Operation::Reset,
            _ => panic!("bad operation"),
        };
        let s = clock.apply(op);
        let t = s.stats;
        println!(
            "{} {} {} {} {} {} {} {} {} {}",
            s.result,
            t.total,
            t.floor,
            t.reads,
            t.reads_above_floor,
            t.frames,
            t.stall_ticks,
            t.advance_ticks,
            t.advance_calls,
            s.milliseconds
        );
    }
}
