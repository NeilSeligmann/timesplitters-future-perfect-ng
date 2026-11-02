//! Proposed pure-game host interfaces. No adapter or backend is selected here.
use tsfp_types::TsfpVec3;

/// Run a host entry point, then publish its shutdown reports exactly once.
pub fn run_then<T>(run: impl FnOnce() -> T, report: impl FnOnce()) -> T {
    let result = run();
    report();
    result
}

#[cfg(test)]
mod tests {
    use super::run_then;

    #[test]
    fn shutdown_report_runs_after_host_and_preserves_result() {
        let events = std::cell::RefCell::new(Vec::new());
        let result = run_then(
            || {
                events.borrow_mut().push("host");
                7
            },
            || events.borrow_mut().push("report"),
        );
        assert_eq!(result, 7);
        assert_eq!(*events.borrow(), ["host", "report"]);
    }
}

#[derive(Debug)]
pub enum HostError {
    Unavailable,
    InvalidResource,
    Io,
    DeviceLost,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceId(pub u64);
#[derive(Clone, Copy, Debug, Default)]
pub struct PadState {
    pub buttons: u32,
    pub axes: [f32; 4],
    pub triggers: [f32; 2],
}

/// XDK clock, content and persistence services, with explicit failures.
pub trait Platform {
    fn monotonic_ticks(&self) -> u64;
    fn ticks_per_second(&self) -> u64;
    fn read_content(&mut self, logical_path: &str) -> Result<Vec<u8>, HostError>;
    fn write_save(&mut self, slot: &str, bytes: &[u8]) -> Result<(), HostError>;
}
/// D3D concepts translated to game-facing resources; guest calls stay in the bridge.
pub trait Render {
    fn create_mesh(
        &mut self,
        positions: &[TsfpVec3],
        indices: &[u32],
    ) -> Result<ResourceId, HostError>;
    fn release(&mut self, resource: ResourceId) -> Result<(), HostError>;
    fn present(&mut self) -> Result<(), HostError>;
}
/// DSOUND reference supplies lifetime, format, scheduling and completion semantics.
pub trait Audio {
    fn submit_pcm(
        &mut self,
        channels: u16,
        sample_rate: u32,
        samples: &[i16],
    ) -> Result<ResourceId, HostError>;
    fn stop(&mut self, voice: ResourceId) -> Result<(), HostError>;
    fn completed(&mut self, voice: ResourceId) -> Result<bool, HostError>;
}
/// XNET transport is separate from gameplay packet and session state.
pub trait Network {
    fn send(&mut self, peer: ResourceId, packet: &[u8]) -> Result<(), HostError>;
    fn receive(&mut self, packet: &mut [u8]) -> Result<Option<(ResourceId, usize)>, HostError>;
}
pub trait Input {
    fn sample(&mut self, port: usize) -> Result<PadState, HostError>;
    fn rumble(&mut self, port: usize, motors: [f32; 2]) -> Result<(), HostError>;
}
