// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered nested-call producer. This only frames supplied observations; the
//! caller is responsible for instrumenting every boundary and proving boot state.
use crate::{Writer, events};
use std::io::{self, Write};

struct Call {
    id: u64,
    thread: u64,
    reads: u32,
    writes: u32,
    placements: u32,
    outputs: usize,
}

pub struct Producer<W> {
    writer: Writer<W>,
    ops: Vec<(u32, usize, usize)>,
    stack: Vec<Call>,
    next_id: u64,
    completed: usize,
}
fn bad() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "unsupported or incomplete capture",
    )
}
fn push32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}
fn push64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

impl<W: Write> Producer<W> {
    /// The caller must have checked the *entire* registry, generations and
    /// placement cursor are in their initial state; a count of zero is not enough.
    pub fn new(
        mut writer: Writer<W>,
        identity: &[u8],
        boot_state_verified: bool,
        ops: &[(u32, usize, usize)],
    ) -> io::Result<Self> {
        let invalid_ops = ops.is_empty()
            || ops.len() > 128
            || ops
                .iter()
                .enumerate()
                .any(|(index, (id, inputs, outputs))| {
                    *inputs > 16
                        || *outputs > 16
                        || ops[..index].iter().any(|(earlier, _, _)| earlier == id)
                });
        if identity.is_empty() || identity.len() > 4096 || !boot_state_verified || invalid_ops {
            writer.incomplete();
            return Err(bad());
        }
        writer.record(events::IDENTITY, identity)?;
        writer.record(events::EMPTY_REGISTRY, &[])?;
        Ok(Self {
            writer,
            ops: ops.to_vec(),
            stack: Vec::new(),
            next_id: 1,
            completed: 0,
        })
    }
    fn reject<T>(&mut self) -> io::Result<T> {
        self.writer.stop();
        Err(bad())
    }
    /// Stream position: the number of events recorded so far.
    pub fn epoch(&self) -> u64 {
        self.writer.count()
    }
    /// Begin a call for a mutation made *before* it was recorded (host-helper writes). The mutation is
    /// provably adjacent to this record only if `epoch` (taken just before the mutation) is still the
    /// stream position, i.e. no other event slipped in; otherwise the order is unprovable and the stream
    /// is poisoned. Never reorders or serialises after the fact.
    pub fn begin_after(
        &mut self,
        thread: u64,
        epoch: u64,
        op: u32,
        args: &[u32],
    ) -> io::Result<u64> {
        if epoch != self.epoch() {
            return self.reject();
        }
        self.begin(thread, op, args)
    }
    pub fn begin(&mut self, thread: u64, op: u32, args: &[u32]) -> io::Result<u64> {
        let Some((_, arity, outputs)) = self
            .ops
            .iter()
            .find(|(candidate, _, _)| *candidate == op)
            .copied()
        else {
            return self.reject();
        };
        if thread == 0
            || args.len() != arity
            || arity > 16
            || outputs > 16
            || self.stack.len() >= 64
            || self.stack.last().is_some_and(|c| c.thread != thread)
        {
            return self.reject();
        }
        let Some(next) = self.next_id.checked_add(1) else {
            return self.reject();
        };
        let id = self.next_id;
        let mut data = Vec::with_capacity(32 + args.len() * 4);
        push64(&mut data, id);
        push64(&mut data, self.stack.last().map_or(0, |c| c.id));
        push64(&mut data, thread);
        push32(&mut data, op);
        push32(&mut data, args.len() as u32);
        for arg in args {
            push32(&mut data, *arg);
        }
        self.writer.record(events::CALL_BEGIN, &data)?;
        self.next_id = next;
        self.stack.push(Call {
            id,
            thread,
            reads: 0,
            writes: 0,
            placements: 0,
            outputs,
        });
        Ok(id)
    }
    pub fn memory(
        &mut self,
        thread: u64,
        write: bool,
        address: u32,
        bytes: &[u8],
    ) -> io::Result<()> {
        let Some(call) = self.stack.last_mut() else {
            return self.reject();
        };
        if call.thread != thread
            || bytes.len() > super::MAX_RECORD - 16
            || u64::from(address) + bytes.len() as u64 > 1u64 << 32
        {
            return self.reject();
        }
        let mut data = Vec::with_capacity(16 + bytes.len());
        push64(&mut data, call.id);
        push32(&mut data, address);
        push32(&mut data, bytes.len() as u32);
        data.extend_from_slice(bytes);
        let counter = if write {
            &mut call.writes
        } else {
            &mut call.reads
        };
        let Some(next) = counter.checked_add(1) else {
            return self.reject();
        };
        self.writer.record(
            if write {
                events::MEMORY_WRITE
            } else {
                events::MEMORY_READ
            },
            &data,
        )?;
        *counter = next;
        Ok(())
    }
    pub fn placement(
        &mut self,
        thread: u64,
        kind: u32,
        length: u64,
        address: u32,
        result: u32,
    ) -> io::Result<()> {
        let Some(call) = self.stack.last_mut() else {
            return self.reject();
        };
        if call.thread != thread || kind > 1 {
            return self.reject();
        }
        let Some(next) = call.placements.checked_add(1) else {
            return self.reject();
        };
        let mut data = Vec::with_capacity(28);
        push64(&mut data, call.id);
        push32(&mut data, kind);
        push64(&mut data, length);
        push32(&mut data, address);
        push32(&mut data, result);
        self.writer.record(events::PLACEMENT, &data)?;
        call.placements = next;
        Ok(())
    }
    pub fn end(&mut self, thread: u64, id: u64, result: u32, outputs: &[u32]) -> io::Result<()> {
        let Some(call) = self.stack.last() else {
            return self.reject();
        };
        if call.thread != thread || call.id != id || outputs.len() != call.outputs {
            return self.reject();
        }
        let mut data = Vec::with_capacity(28 + outputs.len() * 4);
        push64(&mut data, id);
        push32(&mut data, result);
        push32(&mut data, call.reads);
        push32(&mut data, call.writes);
        push32(&mut data, call.placements);
        push32(&mut data, outputs.len() as u32);
        for output in outputs {
            push32(&mut data, *output);
        }
        self.writer.record(events::CALL_END, &data)?;
        self.stack.pop();
        self.completed += 1;
        Ok(())
    }
    pub fn incomplete(&mut self) {
        self.writer.incomplete();
    }
    pub fn stop(&mut self) {
        self.writer.stop();
    }
    pub fn seal(mut self) -> io::Result<W> {
        if !self.stack.is_empty() || self.completed == 0 {
            self.writer.incomplete();
        }
        self.writer.seal()
    }
}
