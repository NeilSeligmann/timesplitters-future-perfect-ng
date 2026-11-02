// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed preset-start orchestration; weather/sky purpose remains INFERRED.
//! Contract and evidence: docs/t2049-manual-audit.md (Remote10).
use crate::abi::{ExtRegisters, GuestCall};
use crate::c2r::rt::RegsExt;
use crate::memory::GuestMemory;
use crate::views::generated_views::TsfpWeatherPresetView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetIndex(pub u32);

/// Opaque parameters retain their measured width; units and meanings are unresolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnParameters {
    pub kind: u32,
    pub parameter_14: u32,
    pub parameter_18: u32,
    pub parameter_28: u32,
}

/// Services used to allocate a preset slot and start its fade.
///
/// `fade_parameter` runs after `associate`: an implementation may update the
/// preset definition while associating the new slot. The guest adapter keeps
/// pointers and register effects at this boundary.
pub trait PresetServices {
    type Slot: Copy;
    fn spawn(&mut self, index: PresetIndex) -> Self::Slot;
    fn associate(&mut self, slot: Self::Slot, index: PresetIndex);
    fn fade_parameter(&mut self, index: PresetIndex) -> u32;
    fn start_fade(&mut self, slot: Self::Slot, parameter: u32);
}

/// Spawn and associate one preset, then read its current fade parameter.
///
/// The returned slot is the value from `spawn`, even if `start_fade` mutates
/// service state. The order is part of the contract.
///
/// ```
/// use tsfp_ports::weather_presets::{start_preset, PresetIndex, PresetServices};
/// struct Services { parameter: u32 }
/// impl PresetServices for Services {
///     type Slot = u32;
///     fn spawn(&mut self, _: PresetIndex) -> u32 { 7 }
///     fn associate(&mut self, _: u32, _: PresetIndex) { self.parameter = 12; }
///     fn fade_parameter(&mut self, _: PresetIndex) -> u32 { self.parameter }
///     fn start_fade(&mut self, slot: u32, parameter: u32) {
///         assert_eq!((slot, parameter), (7, 12));
///     }
/// }
/// assert_eq!(start_preset(PresetIndex(3), &mut Services { parameter: 0 }), 7);
/// ```
pub fn start_preset<S: PresetServices>(index: PresetIndex, services: &mut S) -> S::Slot {
    let slot = services.spawn(index);
    services.associate(slot, index);
    // Reload after spawning and association, which may mutate shared definitions.
    let parameter = services.fade_parameter(index);
    services.start_fade(slot, parameter);
    slot
}

const PRESET_TABLE: u32 = 0x535238;

/// Live guest definition; each field is read at its original width and time.
#[derive(Clone, Copy)]
struct PresetDefinition(TsfpWeatherPresetView);

impl PresetDefinition {
    fn at(index: PresetIndex) -> Self {
        Self(TsfpWeatherPresetView(PRESET_TABLE.wrapping_add(
            index.0.wrapping_mul(TsfpWeatherPresetView::SIZE),
        )))
    }

    fn spawn_parameters<M: GuestMemory>(self, m: &M) -> SpawnParameters {
        SpawnParameters {
            parameter_28: m.read32(
                self.0
                    .0
                    .wrapping_add(TsfpWeatherPresetView::OFFSET_SPAWN_PARAMETER_28),
            ),
            parameter_18: m.read32(
                self.0
                    .0
                    .wrapping_add(TsfpWeatherPresetView::OFFSET_SPAWN_PARAMETER_18),
            ),
            parameter_14: m.read32(
                self.0
                    .0
                    .wrapping_add(TsfpWeatherPresetView::OFFSET_SPAWN_PARAMETER_14),
            ),
            kind: m.read32(self.0.0.wrapping_add(TsfpWeatherPresetView::OFFSET_KIND)),
        }
    }

    fn fade_parameter<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(
            self.0
                .0
                .wrapping_add(TsfpWeatherPresetView::OFFSET_FADE_PARAMETER),
        )
    }

    fn payload_address(self) -> u32 {
        self.0.0.wrapping_add(8)
    }
}

struct GuestPresets<'a, M, C> {
    memory: &'a mut M,
    guest: &'a mut C,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> PresetServices for GuestPresets<'_, M, C> {
    type Slot = u32;

    fn spawn(&mut self, index: PresetIndex) -> u32 {
        let r = &mut self.regs;
        let m = &mut self.memory;
        r.ebx = index.0;
        r.push(*m, r.esi);
        let definition = PresetDefinition::at(index);
        r.esi = definition.0.0;
        let parameters = definition.spawn_parameters(&**m);
        r.eax = parameters.parameter_28;
        r.ecx = parameters.parameter_18;
        r.edx = parameters.parameter_14;
        r.push(*m, r.edi);
        r.push(*m, r.eax);
        r.push(*m, r.ecx);
        r.ecx = parameters.kind;
        r.push(*m, r.edx);
        r.eax = definition.payload_address();
        r.push(*m, r.eax);
        r.push(*m, r.ecx);
        r.call(self.guest, 0x341850);
        r.edi = r.eax;
        r.edi
    }

    fn associate(&mut self, slot: u32, _index: PresetIndex) {
        // EBX is preserved across the neighbouring callee by the production ABI.
        self.memory.write32(slot.wrapping_add(0x1d8), self.regs.ebx);
    }

    fn fade_parameter(&mut self, _index: PresetIndex) -> u32 {
        self.regs.edx =
            PresetDefinition(TsfpWeatherPresetView(self.regs.esi)).fade_parameter(&*self.memory);
        self.regs.edx
    }

    fn start_fade(&mut self, slot: u32, parameter: u32) {
        self.regs.push(self.memory, parameter);
        self.regs.push(self.memory, slot);
        self.regs.call(self.guest, 0x3445e0);
    }
}

// portreg: va=0x00344ac0
/// cdecl(preset index); returns the allocated slot, even after the fade callee returns.
pub fn start_preset_guest<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, r.ebx);
    let index = PresetIndex(m.read32(r.esp.wrapping_add(8)));
    start_preset(
        index,
        &mut GuestPresets {
            memory: m,
            guest,
            regs: r,
        },
    );
    r.esp = r.esp.wrapping_add(0x1c);
    r.eax = r.edi;
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_definition_uses_live_dword_reads_in_original_order() {
        use std::cell::RefCell;
        struct Reads(RefCell<std::vec::Vec<u32>>);
        impl GuestMemory for Reads {
            fn read8(&self, _: u32) -> u8 {
                panic!("unexpected byte read")
            }
            fn write8(&mut self, _: u32, _: u8) {
                panic!("unexpected write")
            }
            fn read32(&self, address: u32) -> u32 {
                self.0.borrow_mut().push(address);
                address
            }
        }
        let memory = Reads(RefCell::new(std::vec::Vec::new()));
        let view = PresetDefinition::at(PresetIndex(2));
        assert_eq!(view.0.0, PRESET_TABLE + 2 * TsfpWeatherPresetView::SIZE);
        assert_eq!(
            view.spawn_parameters(&memory),
            SpawnParameters {
                parameter_28: view.0.0 + 0x28,
                parameter_18: view.0.0 + 0x18,
                parameter_14: view.0.0 + 0x14,
                kind: view.0.0,
            }
        );
        assert_eq!(view.fade_parameter(&memory), view.0.0 + 0x1c);
        assert_eq!(view.payload_address(), view.0.0 + 8);
        assert_eq!(
            *memory.0.borrow(),
            [
                view.0.0 + 0x28,
                view.0.0 + 0x18,
                view.0.0 + 0x14,
                view.0.0,
                view.0.0 + 0x1c,
            ]
        );
    }

    #[test]
    fn association_precedes_fade_read_and_allocated_slot_is_returned() {
        struct Services {
            stage: u8,
        }
        impl PresetServices for Services {
            type Slot = u32;
            fn spawn(&mut self, index: PresetIndex) -> u32 {
                assert_eq!(index, PresetIndex(7));
                assert_eq!(self.stage, 0);
                self.stage = 1;
                42
            }
            fn associate(&mut self, slot: u32, index: PresetIndex) {
                assert_eq!((slot, index), (42, PresetIndex(7)));
                assert_eq!(self.stage, 1);
                self.stage = 2;
            }
            fn fade_parameter(&mut self, _: PresetIndex) -> u32 {
                assert_eq!(self.stage, 2);
                self.stage = 3;
                0x7fc01234
            }
            fn start_fade(&mut self, slot: u32, parameter: u32) {
                assert_eq!((slot, parameter), (42, 0x7fc01234));
                assert_eq!(self.stage, 3);
                self.stage = 4;
            }
        }
        let mut services = Services { stage: 0 };
        assert_eq!(start_preset(PresetIndex(7), &mut services), 42);
        assert_eq!(services.stage, 4);
    }

    #[test]
    fn preset_service_order_and_post_association_value_hold_for_seed_grid() {
        struct Services {
            index: PresetIndex,
            slot: u32,
            parameter: u32,
            calls: [u8; 4],
            count: usize,
        }
        impl Services {
            fn record(&mut self, call: u8) {
                self.calls[self.count] = call;
                self.count += 1;
            }
        }
        impl PresetServices for Services {
            type Slot = u32;
            fn spawn(&mut self, index: PresetIndex) -> u32 {
                assert_eq!(index, self.index);
                self.record(1);
                self.slot
            }
            fn associate(&mut self, slot: u32, index: PresetIndex) {
                assert_eq!((slot, index), (self.slot, self.index));
                self.record(2);
                self.parameter = self.parameter.wrapping_add(index.0);
            }
            fn fade_parameter(&mut self, index: PresetIndex) -> u32 {
                assert_eq!(index, self.index);
                self.record(3);
                self.parameter
            }
            fn start_fade(&mut self, slot: u32, parameter: u32) {
                assert_eq!((slot, parameter), (self.slot, self.parameter));
                self.record(4);
            }
        }
        for seed in 0u32..256 {
            let index = PresetIndex(seed.wrapping_mul(0x0101_0101));
            let slot = seed.rotate_left(13) ^ 0x8000_0001;
            let mut services = Services {
                index,
                slot,
                parameter: u32::MAX - seed,
                calls: [0; 4],
                count: 0,
            };
            assert_eq!(start_preset(index, &mut services), slot);
            assert_eq!(services.calls, [1, 2, 3, 4]);
            assert_eq!(services.parameter, (u32::MAX - seed).wrapping_add(index.0));
        }
    }
}
