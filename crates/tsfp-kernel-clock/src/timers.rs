//! Timer/DPC slice. Shared guest object capacity and ABI writes belong to the K-1 adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId(pub u32);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    pub id: ObjectId,
    pub kind: u32,
    pub armed: bool,
    pub due: i64,
    pub deadline: u64,
    pub period_ms: i32,
    pub dpc: Option<ObjectId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub id: ObjectId,
    pub routine: u32,
    pub context: u32,
    pub arguments: [u32; 2],
    pub sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dpc {
    pub delivery: Delivery,
    pub queued: bool,
}
#[derive(Debug, Default)]
pub struct Timers {
    pub timers: Vec<Timer>,
    pub dpcs: Vec<Dpc>,
    sequence: u64,
    boundary: Option<u64>,
    in_flight: Option<Delivery>,
}
impl Timers {
    fn sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }
    fn timer(&mut self, id: ObjectId) -> &mut Timer {
        if let Some(i) = self.timers.iter().position(|t| t.id == id) {
            return &mut self.timers[i];
        }
        self.timers.push(Timer {
            id,
            kind: 0,
            armed: false,
            due: 0,
            deadline: 0,
            period_ms: 0,
            dpc: None,
        });
        self.timers.last_mut().unwrap()
    }
    pub fn initialize_timer(&mut self, id: ObjectId, kind: u32) {
        let t = self.timer(id);
        t.kind = kind;
        t.armed = false;
        t.due = 0;
        t.dpc = None;
        t.period_ms = 0;
    }
    pub fn set(
        &mut self,
        id: ObjectId,
        due: i64,
        period_ms: i32,
        dpc: Option<ObjectId>,
        elapsed: u64,
        calendar: u64,
    ) -> bool {
        let t = self.timer(id);
        let previous = t.armed;
        let delay = if due < 0 {
            due.unsigned_abs()
        } else {
            (due as u64).saturating_sub(calendar)
        };
        t.armed = true;
        t.due = due;
        t.period_ms = period_ms;
        t.dpc = dpc;
        t.deadline = elapsed.saturating_add(delay);
        previous
    }
    pub fn cancel(&mut self, id: ObjectId) -> bool {
        let Some(t) = self.timers.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        let previous = t.armed;
        t.armed = false;
        t.due = 0;
        t.dpc = None;
        t.period_ms = 0;
        previous
    }
    pub fn initialize_dpc(&mut self, id: ObjectId, routine: u32, context: u32) {
        let sequence = self.sequence();
        let fresh = Dpc {
            delivery: Delivery {
                id,
                routine,
                context,
                arguments: [0; 2],
                sequence,
            },
            queued: false,
        };
        if let Some(d) = self.dpcs.iter_mut().find(|d| d.delivery.id == id) {
            *d = fresh;
        } else {
            self.dpcs.push(fresh);
        }
    }
    pub fn enqueue(&mut self, id: ObjectId, arguments: [u32; 2]) -> bool {
        if !self.dpcs.iter().any(|d| d.delivery.id == id) {
            self.dpcs.push(Dpc {
                delivery: Delivery {
                    id,
                    routine: 0,
                    context: 0,
                    arguments: [0; 2],
                    sequence: 0,
                },
                queued: false,
            });
        }
        let index = self.dpcs.iter().position(|d| d.delivery.id == id).unwrap();
        if self.dpcs[index].queued {
            return false;
        }
        let sequence = self.sequence();
        let d = &mut self.dpcs[index];
        d.queued = true;
        d.delivery.arguments = arguments;
        d.delivery.sequence = sequence;
        true
    }
    pub fn remove(&mut self, id: ObjectId) -> bool {
        let Some(index) = self.dpcs.iter().position(|d| d.delivery.id == id) else {
            return false;
        };
        let sequence = self.sequence();
        let d = &mut self.dpcs[index];
        let previous = d.queued;
        d.queued = false;
        d.delivery.sequence = sequence;
        previous
    }
    /// False means refused or already servicing; adapters defer recursive service.
    pub fn begin_service(&mut self, elapsed: u64, irql: u32, dispatcher: bool) -> bool {
        if self.boundary.is_some() || irql > 2 || !dispatcher {
            return false;
        }
        for i in 0..self.timers.len() {
            if !self.timers[i].armed || self.timers[i].deadline > elapsed {
                continue;
            }
            if let Some(id) = self.timers[i].dpc {
                if self.dpcs.iter().any(|d| d.delivery.id == id && !d.queued) {
                    self.enqueue(id, [elapsed as u32, (elapsed >> 32) as u32]);
                }
            }
            let timer = &mut self.timers[i];
            if timer.period_ms > 0 {
                let period = timer.period_ms as u64 * 10_000;
                timer.deadline =
                    elapsed.saturating_add(period - (elapsed - timer.deadline) % period);
            } else {
                timer.armed = false;
            }
        }
        self.boundary = Some(self.sequence);
        true
    }
    /// Release the outer lock before executing this delivery at IRQL 2.
    pub fn next_delivery(&mut self) -> Option<Delivery> {
        if self.in_flight.is_some() {
            return None;
        }
        let boundary = self.boundary?;
        let next = self
            .dpcs
            .iter_mut()
            .filter(|d| d.queued && d.delivery.sequence <= boundary)
            .min_by_key(|d| d.delivery.sequence)?;
        next.queued = false;
        let pending = next.delivery.clone();
        self.in_flight = Some(pending.clone());
        Some(pending)
    }
    /// Refusal ends this pass. A callback's newer queue state takes precedence.
    pub fn finish_delivery(&mut self, executed: bool) {
        let pending = self.in_flight.take().expect("no delivery in flight");
        if !executed {
            if let Some(d) = self.dpcs.iter_mut().find(|d| d.delivery.id == pending.id) {
                if !d.queued && d.delivery.sequence == pending.sequence {
                    d.queued = true;
                }
            }
            self.boundary = None;
        }
    }
    pub fn end_service(&mut self) {
        assert!(self.in_flight.is_none());
        self.boundary = None;
    }
}
