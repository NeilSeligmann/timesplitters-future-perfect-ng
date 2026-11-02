// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned model of the XInput HLE registry, used by offline checks and the
//! optional live dispatcher path. The C surface remains the production oracle and
//! default until the platform retirement gate is complete.

/// A function on the measured XInput surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceEntry {
    pub address: u32,
    pub name: &'static str,
    pub sites: u32,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Input,
    MemoryUnit,
    Voice,
}

use Kind::{Input, MemoryUnit, Voice};

/// The order and site counts match `XINPUT_SURFACE` in `xinput_hle.c`.
pub const SURFACE: [SurfaceEntry; 15] = [
    SurfaceEntry {
        address: 0x46dbd2,
        name: "XGetDevices",
        sites: 4,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46dbf4,
        name: "XGetDeviceChanges",
        sites: 3,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46db91,
        name: "XPeekDevices",
        sites: 2,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46e189,
        name: "XInputClose",
        sites: 2,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x4754fd,
        name: "XVoiceCreateMediaObjectEx",
        sites: 2,
        kind: Voice,
    },
    SurfaceEntry {
        address: 0x46d7c4,
        name: "XMountMUA",
        sites: 1,
        kind: MemoryUnit,
    },
    SurfaceEntry {
        address: 0x46d8f6,
        name: "XUnmountMU",
        sites: 1,
        kind: MemoryUnit,
    },
    SurfaceEntry {
        address: 0x46dbcd,
        name: "XInitDevices",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46e133,
        name: "XInputOpen",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46e195,
        name: "XInputGetCapabilities",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46e36d,
        name: "XInputGetState",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46e3e0,
        name: "XInputSetState",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46f3a8,
        name: "XGetDeviceEnumerationStatus",
        sites: 1,
        kind: Input,
    },
    SurfaceEntry {
        address: 0x46da04,
        name: "XReadMUMetaData",
        sites: 0,
        kind: MemoryUnit,
    },
    SurfaceEntry {
        address: 0x4752c6,
        name: "XVoiceCreateMediaObject",
        sites: 0,
        kind: Voice,
    },
];

const DATA_SYMBOLS: [u32; 6] = [0x46c6e0, 0x46c75c, 0x46c7c0, 0x46c894, 0x46c8a0, 0x46c8ac];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    StubFirstCall,
    DataSymbol,
    UnknownInSection,
    UnknownOutsideSection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallResult {
    pub value: u32,
    pub diagnostic: Option<Diagnostic>,
}

/// A state update produced while the registry is borrowed. The caller invokes it
/// only after releasing that borrow, so callbacks may safely update registry state.
pub struct PreparedCall<C> {
    handler: Option<fn(&mut C) -> u32>,
    result: CallResult,
}

impl<C> PreparedCall<C> {
    pub fn has_handler(&self) -> bool {
        self.handler.is_some()
    }

    pub fn finish(self, context: &mut C) -> CallResult {
        match self.handler {
            Some(handler) => CallResult {
                value: handler(context),
                diagnostic: None,
            },
            None => self.result,
        }
    }
}

struct Entry<C> {
    handler: Option<fn(&mut C) -> u32>,
    default_return: u32,
    calls: u64,
    reported: bool,
}

impl<C> Default for Entry<C> {
    fn default() -> Self {
        Self {
            handler: None,
            default_return: 0,
            calls: 0,
            reported: false,
        }
    }
}

/// Owns dispatch state; the caller controls serialization and reset.
pub struct Registry<C> {
    entries: [Entry<C>; SURFACE.len()],
    unknown_calls: u64,
}

impl<C> Default for Registry<C> {
    fn default() -> Self {
        Self {
            entries: std::array::from_fn(|_| Entry::default()),
            unknown_calls: 0,
        }
    }
}

impl<C> Registry<C> {
    fn entry_mut(&mut self, address: u32) -> Option<&mut Entry<C>> {
        SURFACE
            .iter()
            .position(|entry| entry.address == address)
            .map(|i| &mut self.entries[i])
    }

    /// Replacing a registered handler is allowed; unknown addresses are refused.
    pub fn register(&mut self, address: u32, handler: fn(&mut C) -> u32) -> bool {
        let Some(entry) = self.entry_mut(address) else {
            return false;
        };
        entry.handler = Some(handler);
        true
    }

    pub fn set_default_return(&mut self, address: u32, value: u32) -> bool {
        let Some(entry) = self.entry_mut(address) else {
            return false;
        };
        entry.default_return = value;
        true
    }

    pub fn call_count(&self, address: u32) -> Option<u64> {
        SURFACE
            .iter()
            .position(|entry| entry.address == address)
            .map(|i| self.entries[i].calls)
    }

    pub fn unknown_calls(&self) -> u64 {
        self.unknown_calls
    }

    pub fn prepare_call(&mut self, address: u32) -> PreparedCall<C> {
        let Some(entry) = self.entry_mut(address) else {
            self.unknown_calls = self.unknown_calls.wrapping_add(1);
            let diagnostic = if DATA_SYMBOLS.contains(&address) {
                Diagnostic::DataSymbol
            } else if (0x46c680..0x47576c).contains(&address) {
                Diagnostic::UnknownInSection
            } else {
                Diagnostic::UnknownOutsideSection
            };
            return PreparedCall {
                handler: None,
                result: CallResult {
                    value: 0,
                    diagnostic: Some(diagnostic),
                },
            };
        };
        entry.calls = entry.calls.wrapping_add(1);
        let handler = entry.handler;
        let diagnostic = if entry.reported {
            None
        } else {
            Some(Diagnostic::StubFirstCall)
        };
        entry.reported = true;
        PreparedCall {
            handler,
            result: CallResult {
                value: entry.default_return,
                diagnostic,
            },
        }
    }

    pub fn call(&mut self, address: u32, context: &mut C) -> CallResult {
        self.prepare_call(address).finish(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_matches_c_registry() {
        let source = include_str!("../../../src/input/xinput_hle.c");
        let table = source
            .split("} XINPUT_SURFACE[XINPUT_FUNCTION_COUNT] = {")
            .nth(1)
            .expect("C surface declaration")
            .split("\n};")
            .next()
            .expect("C surface end");
        assert_eq!(
            table
                .lines()
                .filter(|line| line.trim_start().starts_with("{0x"))
                .count(),
            SURFACE.len()
        );
        for entry in SURFACE {
            let kind = match entry.kind {
                Kind::Input => "INPUT",
                Kind::MemoryUnit => "MEMORY_UNIT",
                Kind::Voice => "VOICE",
            };
            let row = format!(
                "{{0x{:08x}, \"{}\", {}, XINPUT_KIND_{kind}}}",
                entry.address, entry.name, entry.sites
            );
            assert!(table.contains(&row), "C surface missing {row}");
        }
    }

    #[test]
    fn registry_dispatches_exact_addresses_and_resets() {
        let mut registry = Registry::<u32>::default();
        let mut context = 4;
        let address = 0x46dbcd;
        assert!(registry.set_default_return(address, 9));
        assert_eq!(
            registry.call(address, &mut context),
            CallResult {
                value: 9,
                diagnostic: Some(Diagnostic::StubFirstCall)
            }
        );
        assert_eq!(
            registry.call(address, &mut context),
            CallResult {
                value: 9,
                diagnostic: None
            }
        );
        assert!(!registry.register(address + 1, |v| *v));
        assert!(registry.register(address, |v| {
            *v += 1;
            *v
        }));
        assert_eq!(registry.call(address, &mut context).value, 5);
        assert!(registry.register(address, |_| 42));
        assert_eq!(registry.call(address, &mut context).value, 42);
        assert_eq!(registry.call_count(address), Some(4));
        registry = Registry::default();
        assert_eq!(
            registry.call(address, &mut context),
            CallResult {
                value: 0,
                diagnostic: Some(Diagnostic::StubFirstCall)
            }
        );
    }

    #[test]
    fn unknown_calls_classify_data_and_section_boundaries() {
        let mut registry = Registry::<()>::default();
        for (address, expected) in [
            (0x46c75c, Diagnostic::DataSymbol),
            (0x46c680, Diagnostic::UnknownInSection),
            (0x47576b, Diagnostic::UnknownInSection),
            (0x46c67f, Diagnostic::UnknownOutsideSection),
            (0x47576c, Diagnostic::UnknownOutsideSection),
        ] {
            assert_eq!(registry.call(address, &mut ()).diagnostic, Some(expected));
            assert_eq!(registry.call(address, &mut ()).diagnostic, Some(expected));
        }
        assert_eq!(registry.unknown_calls(), 10);
        assert_eq!(registry.call_count(0x46c75c), None);
    }
}
