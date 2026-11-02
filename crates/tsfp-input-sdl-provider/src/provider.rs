// SPDX-License-Identifier: GPL-3.0-or-later
//! State and handle lifecycle for a presenter-thread controller backend.

use crate::{Device, PORTS, Selector};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnmappedJoystick {
    pub name: String,
    pub guid: String,
}

/// Format the existing C diagnostic into its 1024-byte, NUL-terminated buffer.
/// Truncation is byte based, matching snprintf even for non-ASCII device names.
pub fn unmapped_diagnostic(devices: &[UnmappedJoystick]) -> Vec<u8> {
    use std::fmt::Write;
    let mut text = String::new();
    for device in devices {
        let _ = writeln!(
            text,
            "unmapped joystick: {} [{}]; supply a local SDL mapping file",
            device.name, device.guid
        );
    }
    let mut bytes = text.into_bytes();
    bytes.truncate(1023);
    bytes.push(0);
    bytes
}

/// The platform adapter owns the SDL calls; handles never leave the provider thread.
pub trait Backend {
    type Handle;
    type State: Default;
    type Error;

    fn prepare(&mut self) {}
    fn devices(&mut self) -> Vec<Device>;
    fn unmapped(&mut self) -> Vec<UnmappedJoystick> {
        Vec::new()
    }
    fn connected(&mut self, handle: &Self::Handle) -> bool;
    fn name(&mut self, _handle: &Self::Handle) -> String {
        "Unnamed controller".into()
    }
    fn open(&mut self, id: u32, port: usize) -> Result<(Self::Handle, bool), Self::Error>;
    fn close(&mut self, handle: Self::Handle);
    fn sample(&mut self, handle: &Self::Handle, port: usize) -> Self::State;
    fn rumble(&mut self, handle: &Self::Handle, left: u16, right: u16) -> Result<(), Self::Error>;
}

struct Slot<H, S> {
    id: u32,
    handle: Option<H>,
    state: S,
    rumble: bool,
    feedback: Option<(u16, u16)>,
    name: String,
    guid: String,
}

impl<H, S: Default> Default for Slot<H, S> {
    fn default() -> Self {
        Self {
            id: 0,
            handle: None,
            state: S::default(),
            rumble: false,
            feedback: None,
            name: String::new(),
            guid: String::new(),
        }
    }
}

/// Device handles and published samples are owned together and dropped on disconnect.
pub struct Provider<B: Backend> {
    backend: B,
    selectors: [Selector; PORTS],
    slots: [Slot<B::Handle, B::State>; PORTS],
    unmapped: Vec<UnmappedJoystick>,
}

impl<B: Backend> Provider<B> {
    pub fn new(backend: B, selectors: [Selector; PORTS]) -> Self {
        Self {
            backend,
            selectors,
            slots: std::array::from_fn(|_| Slot::default()),
            unmapped: Vec::new(),
        }
    }

    fn drop_port(&mut self, port: usize) {
        let slot = &mut self.slots[port];
        if let Some(handle) = slot.handle.take() {
            let _ = self.backend.rumble(&handle, 0, 0);
            self.backend.close(handle);
        }
        *slot = Slot::default();
    }

    /// Reconcile hotplug first, then publish samples and apply pending feedback.
    /// Open errors are retained in encounter order; a later candidate may still open.
    pub fn pump(&mut self) -> Vec<B::Error> {
        let mut errors = Vec::new();
        self.backend.prepare();
        for port in 0..PORTS {
            if self.slots[port]
                .handle
                .as_ref()
                .is_some_and(|h| !self.backend.connected(h))
            {
                self.drop_port(port);
            }
        }
        let devices = self.backend.devices();
        self.unmapped = self.backend.unmapped();
        for automatic in [false, true] {
            for port in 0..PORTS {
                let selector = &self.selectors[port];
                if !selector.enabled
                    || self.slots[port].handle.is_some()
                    || selector.guid.is_empty() != automatic
                {
                    continue;
                }
                let mut occurrence = 0;
                for device in &devices {
                    if !selector.guid.is_empty() && selector.guid != device.guid {
                        continue;
                    }
                    let matches = selector.occurrence.is_none_or(|n| n == occurrence);
                    occurrence += 1;
                    if !matches || device.id == 0 || self.slots.iter().any(|s| s.id == device.id) {
                        continue;
                    }
                    match self.backend.open(device.id, port) {
                        Ok((handle, rumble)) => {
                            self.slots[port].name = self.backend.name(&handle);
                            self.slots[port].guid = device.guid.clone();
                            self.slots[port].id = device.id;
                            self.slots[port].handle = Some(handle);
                            self.slots[port].rumble = rumble;
                            break;
                        }
                        Err(error) => errors.push(error),
                    }
                }
            }
        }
        for (port, slot) in self.slots.iter_mut().enumerate() {
            if let Some(handle) = slot.handle.as_ref() {
                slot.state = self.backend.sample(handle, port);
                if let Some((left, right)) = slot.feedback.take() {
                    if slot.rumble {
                        if let Err(error) = self.backend.rumble(handle, left, right) {
                            errors.push(error);
                        }
                    }
                }
            }
        }
        errors
    }

    pub fn sample(&self, port: usize) -> Option<&B::State> {
        self.slots
            .get(port)
            .filter(|s| s.handle.is_some())
            .map(|s| &s.state)
    }

    pub fn unmapped(&self) -> &[UnmappedJoystick] {
        &self.unmapped
    }

    pub fn id(&self, port: usize) -> Option<u32> {
        self.slots
            .get(port)
            .and_then(|s| s.handle.as_ref().map(|_| s.id))
    }

    pub fn details(&self, port: usize) -> Option<(&str, &str, bool)> {
        self.slots
            .get(port)
            .filter(|s| s.handle.is_some())
            .map(|s| (s.name.as_str(), s.guid.as_str(), s.rumble))
    }

    pub fn feedback(&mut self, port: usize, left: u16, right: u16) -> bool {
        let Some(slot) = self.slots.get_mut(port) else {
            return false;
        };
        if slot.handle.is_none() || !slot.rumble {
            return false;
        }
        slot.feedback = Some((left, right));
        true
    }

    pub fn assign(&mut self, port: usize, selector: Selector) -> bool {
        if port >= PORTS {
            return false;
        }
        self.drop_port(port);
        self.selectors[port] = selector;
        true
    }
}

impl<B: Backend> Drop for Provider<B> {
    fn drop(&mut self) {
        for port in 0..PORTS {
            self.drop_port(port);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn unmapped_diagnostic_matches_c_buffer_and_truncation() {
        let device = UnmappedJoystick {
            name: "pad".into(),
            guid: "abc".into(),
        };
        assert_eq!(unmapped_diagnostic(&[]), b"\0");
        assert_eq!(
            unmapped_diagnostic(&[device]),
            b"unmapped joystick: pad [abc]; supply a local SDL mapping file\n\0"
        );
        let long = UnmappedJoystick {
            name: "x".repeat(2000),
            guid: "abc".into(),
        };
        let result = unmapped_diagnostic(&[long]);
        assert_eq!(result.len(), 1024);
        assert_eq!(result[1023], 0);
        assert!(result[..1023].iter().all(|b| *b != 0));
    }

    #[derive(Default)]
    struct World {
        devices: Vec<Device>,
        unmapped: Vec<UnmappedJoystick>,
        log: Vec<String>,
        fail_open: u32,
    }
    struct Fake(Rc<RefCell<World>>);
    impl Backend for Fake {
        type Handle = u32;
        type State = u32;
        type Error = u32;
        fn devices(&mut self) -> Vec<Device> {
            self.0.borrow().devices.clone()
        }
        fn unmapped(&mut self) -> Vec<UnmappedJoystick> {
            self.0.borrow().unmapped.clone()
        }
        fn connected(&mut self, id: &u32) -> bool {
            self.0.borrow().devices.iter().any(|d| d.id == *id)
        }
        fn open(&mut self, id: u32, port: usize) -> Result<(u32, bool), u32> {
            let mut w = self.0.borrow_mut();
            w.log.push(format!("open {id} {port}"));
            if w.fail_open == id {
                Err(id)
            } else {
                Ok((id, id != 2))
            }
        }
        fn close(&mut self, id: u32) {
            self.0.borrow_mut().log.push(format!("close {id}"));
        }
        fn sample(&mut self, id: &u32, _port: usize) -> u32 {
            *id * 10
        }
        fn rumble(&mut self, id: &u32, left: u16, right: u16) -> Result<(), u32> {
            self.0
                .borrow_mut()
                .log
                .push(format!("rumble {id} {left} {right}"));
            Ok(())
        }
    }
    fn device(id: u32, guid: &str) -> Device {
        Device {
            id,
            guid: guid.into(),
        }
    }
    fn enabled(guid: &str) -> Selector {
        Selector {
            enabled: true,
            guid: guid.into(),
            occurrence: None,
        }
    }

    #[test]
    fn unmapped_inventory_refreshes_on_each_pump() {
        let world = Rc::new(RefCell::new(World::default()));
        let mut provider = Provider::new(
            Fake(world.clone()),
            std::array::from_fn(|_| Selector::default()),
        );
        world.borrow_mut().unmapped.push(UnmappedJoystick {
            name: "unknown".into(),
            guid: "abc".into(),
        });
        provider.pump();
        assert_eq!(provider.unmapped()[0].guid, "abc");
        world.borrow_mut().unmapped.clear();
        provider.pump();
        assert!(provider.unmapped().is_empty());
    }

    #[test]
    fn hotplug_feedback_and_disconnect_clear_state_and_close_handle() {
        let world = Rc::new(RefCell::new(World::default()));
        world.borrow_mut().devices = vec![device(1, "a"), device(2, "b")];
        let mut provider = Provider::new(
            Fake(world.clone()),
            [
                enabled(""),
                enabled("a"),
                Selector::default(),
                Selector::default(),
            ],
        );
        assert!(provider.pump().is_empty());
        assert_eq!((provider.id(0), provider.id(1)), (Some(2), Some(1)));
        assert_eq!(
            provider.details(0),
            Some(("Unnamed controller", "b", false))
        );
        assert_eq!(provider.details(1), Some(("Unnamed controller", "a", true)));
        assert_eq!(provider.sample(1), Some(&10));
        assert!(!provider.feedback(0, 1, 2));
        assert!(provider.feedback(1, 3, 4));
        assert!(provider.pump().is_empty());
        world.borrow_mut().devices.retain(|d| d.id != 1);
        assert!(provider.pump().is_empty());
        assert_eq!(provider.sample(1), None);
        assert_eq!(provider.details(1), None);
        assert!(!provider.feedback(1, 3, 4));
        assert!(world.borrow().log.ends_with(&[
            "rumble 1 3 4".into(),
            "rumble 1 0 0".into(),
            "close 1".into()
        ]));
    }

    #[test]
    fn failed_open_tries_next_candidate_and_assignment_closes_previous() {
        let world = Rc::new(RefCell::new(World {
            devices: vec![device(1, "a"), device(2, "a")],
            fail_open: 1,
            ..World::default()
        }));
        let mut provider = Provider::new(
            Fake(world.clone()),
            [
                enabled("a"),
                Selector::default(),
                Selector::default(),
                Selector::default(),
            ],
        );
        assert_eq!(provider.pump(), vec![1]);
        assert_eq!(provider.id(0), Some(2));
        assert!(provider.assign(0, Selector::default()));
        assert_eq!(provider.id(0), None);
        assert!(
            world
                .borrow()
                .log
                .ends_with(&["rumble 2 0 0".into(), "close 2".into()])
        );
        assert!(!provider.assign(PORTS, enabled("")));
    }
}
