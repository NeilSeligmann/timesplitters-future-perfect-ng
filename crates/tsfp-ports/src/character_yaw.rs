// SPDX-License-Identifier: GPL-3.0-or-later
//! Horizontal heading comparison, a direct edge of the T2049 character tick.
//! Contract and evidence: docs/t2049-manual-audit.md (Remote10).
use crate::abi::{ExtRegisters, GuestCall, Xmm};
use crate::c2r::rt::RegsExt;
use crate::memory::GuestMemory;
use crate::views::generated_views::TsfpHeadingPointView;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizontalPoint {
    pub x: f32,
    pub z: f32,
}

impl HorizontalPoint {
    /// Borrow only the horizontal components. Keep dword reads and their order:
    /// the generated float getters read bytes and would change the guest ABI.
    fn from_guest<M: GuestMemory>(m: &M, point: TsfpHeadingPointView) -> Self {
        Self {
            x: f32::from_bits(m.read32(point.0.wrapping_add(TsfpHeadingPointView::OFFSET_X))),
            z: f32::from_bits(m.read32(point.0.wrapping_add(TsfpHeadingPointView::OFFSET_Z))),
        }
    }
}

impl HorizontalPoint {
    pub fn direction_to(self, target: Self) -> [f32; 3] {
        [target.x - self.x, 0.0, target.z - self.z]
    }
}

/// Preserve the angle representation at service edges, including arbitrary float bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AngleBits(pub u32);

/// Direction conversion and periodic difference remain neighbouring math services.
/// Read the reference after conversion: a service may update shared input state.
pub trait YawServices {
    fn direction_yaw(&mut self, direction: [f32; 3]) -> AngleBits;
    fn reference_angle(&mut self) -> AngleBits;
    fn periodic_difference(&mut self, reference: AngleBits, yaw: AngleBits, range: [f32; 2]);
}

pub fn compare_heading<S: YawServices>(
    origin: HorizontalPoint,
    target: HorizontalPoint,
    services: &mut S,
) {
    let yaw = services.direction_yaw(origin.direction_to(target));
    let reference = services.reference_angle();
    services.periodic_difference(reference, yaw, [-180.0, 180.0]);
}

struct GuestYaw<'a, M, C> {
    memory: &'a mut M,
    guest: &'a mut C,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> YawServices for GuestYaw<'_, M, C> {
    fn direction_yaw(&mut self, direction: [f32; 3]) -> AngleBits {
        let r = &mut self.regs;
        let m = &mut self.memory;
        // Local vector and output alias the original stack argument, not a new guest record.
        m.write32(r.esp, direction[0].to_bits());
        m.write32(r.esp.wrapping_add(4), direction[1].to_bits());
        r.xmm[0] = Xmm::scalar(direction[2].to_bits());
        r.eax = r.esp.wrapping_add(0x14);
        r.push(*m, r.eax);
        r.ecx = r.esp.wrapping_add(4);
        r.push(*m, 0);
        r.push(*m, r.ecx);
        m.write32(r.esp.wrapping_add(0x14), direction[2].to_bits());
        r.call(self.guest, 0x153cd0);
        r.edx = m.read32(r.esp.wrapping_add(0x20));
        AngleBits(r.edx)
    }

    fn reference_angle(&mut self) -> AngleBits {
        self.regs.eax = self.memory.read32(self.regs.esp.wrapping_add(0x24));
        AngleBits(self.regs.eax)
    }

    fn periodic_difference(&mut self, reference: AngleBits, yaw: AngleBits, range: [f32; 2]) {
        let r = &mut self.regs;
        r.push(self.memory, range[1].to_bits());
        r.push(self.memory, range[0].to_bits());
        r.push(self.memory, yaw.0);
        r.push(self.memory, reference.0);
        r.call(self.guest, 0x12570);
        r.esp = r.esp.wrapping_add(0x28);
    }
}

// portreg: va=0x002661f0
/// cdecl(origin vec3*, target vec3*, reference angle bits); result belongs to math callee.
pub fn compare_heading_guest<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(12);
    r.eax = m.read32(r.esp.wrapping_add(0x14));
    r.ecx = m.read32(r.esp.wrapping_add(0x10));
    let origin = HorizontalPoint::from_guest(m, TsfpHeadingPointView(r.ecx));
    let target = HorizontalPoint::from_guest(m, TsfpHeadingPointView(r.eax));
    compare_heading(
        origin,
        target,
        &mut GuestYaw {
            memory: m,
            guest,
            regs: r,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_point_reads_live_horizontal_dwords_only() {
        use std::cell::RefCell;
        struct Observed(RefCell<Vec<u32>>);
        impl GuestMemory for Observed {
            fn read8(&self, _: u32) -> u8 { panic!("byte read") }
            fn write8(&mut self, _: u32, _: u8) { panic!("byte write") }
            fn read32(&self, address: u32) -> u32 {
                self.0.borrow_mut().push(address);
                match address { 0xffff_fffc => 1.25f32.to_bits(), 4 => (-2.5f32).to_bits(), _ => panic!("unexpected read") }
            }
        }
        let memory = Observed(RefCell::new(Vec::new()));
        let point = HorizontalPoint::from_guest(&memory, TsfpHeadingPointView(0xffff_fffc));
        assert_eq!(point, HorizontalPoint { x: 1.25, z: -2.5 });
        assert_eq!(*memory.0.borrow(), [0xffff_fffc, 4]);
    }

    #[test]
    fn horizontal_delta_and_reference_reload_cross_named_services() {
        struct Services {
            converted: bool,
        }
        impl YawServices for Services {
            fn direction_yaw(&mut self, direction: [f32; 3]) -> AngleBits {
                assert_eq!(direction, [-4.0, 0.0, 6.0]);
                assert_eq!(direction[1].to_bits(), 0);
                self.converted = true;
                AngleBits(0x7fc01234)
            }
            fn reference_angle(&mut self) -> AngleBits {
                assert!(self.converted);
                AngleBits(0x80000000)
            }
            fn periodic_difference(
                &mut self,
                reference: AngleBits,
                yaw: AngleBits,
                range: [f32; 2],
            ) {
                assert_eq!(reference, AngleBits(0x80000000));
                assert_eq!(yaw, AngleBits(0x7fc01234));
                assert_eq!(range, [-180.0, 180.0]);
            }
        }
        compare_heading(
            HorizontalPoint { x: 5.0, z: -2.0 },
            HorizontalPoint { x: 1.0, z: 4.0 },
            &mut Services { converted: false },
        );
    }
}
