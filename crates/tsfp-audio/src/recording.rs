//! Recording host: captures the command stream (and renders through an inner host) so conformance
//! streams can be written offline and replayed against any [`SoundHost`].

use crate::contract::{Command, SoundHost};

/// Timed log entry: an applied command or a render request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Apply(Command, bool),
    Render { now_ticks: u64, capacity: usize, frames: usize },
}

pub struct Recording<H: SoundHost> {
    pub inner: H,
    pub log: Vec<Event>,
}

impl<H: SoundHost> Recording<H> {
    pub fn new(inner: H) -> Self {
        Self { inner, log: Vec::new() }
    }
    /// The commands only, for replay against another host.
    pub fn commands(&self) -> Vec<Command> {
        self.log.iter().filter_map(|e| if let Event::Apply(c, _) = e { Some(c.clone()) } else { None }).collect()
    }
}

impl<H: SoundHost> SoundHost for Recording<H> {
    fn apply(&mut self, command: &Command) -> bool {
        let ok = self.inner.apply(command);
        self.log.push(Event::Apply(command.clone(), ok));
        ok
    }
    fn render_until(&mut self, now_ticks: u64, capacity: usize) -> Vec<i16> {
        let out = self.inner.render_until(now_ticks, capacity);
        self.log.push(Event::Render { now_ticks, capacity, frames: out.len() / 2 });
        out
    }
}

/// A host that only records (renders silence of the requested length), for capture without a mixer.
#[derive(Default)]
pub struct Silent;
impl SoundHost for Silent {
    fn apply(&mut self, _: &Command) -> bool {
        true
    }
    fn render_until(&mut self, _: u64, capacity: usize) -> Vec<i16> {
        vec![0; capacity * 2]
    }
}
