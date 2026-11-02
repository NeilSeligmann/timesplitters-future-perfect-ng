// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned math values for native systems. Guest-address adapters stay in `math_slice`.
//! Matrix words follow the established row-major 16-f32 layout, with translation at elements 12..14.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }

    pub fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    pub fn scale(self, scale: f32) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }

    /// The left-associated order is retained for the common three-term dot product.
    pub fn dot(self, rhs: Self) -> f32 {
        (self.x * rhs.x + self.y * rhs.y) + self.z * rhs.z
    }

    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn lerp(self, end: Self, amount: f32) -> Self {
        self.add(end.sub(self).scale(amount))
    }
}

/// Row-major 16-f32 matrix words. Translation occupies bytes +48..+59 (elements 12..14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub elements: [f32; 16],
}

impl Mat4 {
    pub const fn from_elements(elements: [f32; 16]) -> Self {
        Self { elements }
    }

    pub fn translation(self) -> Vec3 {
        Vec3::new(self.elements[12], self.elements[13], self.elements[14])
    }

    /// Transforms a point using the original X, Y, Z product and sum order.
    pub fn transform_point(self, point: Vec3) -> Vec3 {
        let m = self.elements;
        let x = ((m[8] * point.z + m[4] * point.y) + m[0] * point.x) + m[12];
        let y = ((m[9] * point.z + m[5] * point.y) + m[1] * point.x) + m[13];
        let z = ((m[10] * point.z + m[6] * point.y) + m[2] * point.x) + m[14];
        Vec3::new(x, y, z)
    }

    /// Transforms a direction with the same linear part and no translation.
    pub fn transform_direction(self, direction: Vec3) -> Vec3 {
        let m = self.elements;
        let x = (m[8] * direction.z + m[4] * direction.y) + m[0] * direction.x;
        let y = (m[9] * direction.z + m[5] * direction.y) + m[1] * direction.x;
        let z = (m[10] * direction.z + m[6] * direction.y) + m[2] * direction.x;
        Vec3::new(x, y, z)
    }

    pub fn transform_world_point(self, point: WorldPoint) -> ViewPoint {
        ViewPoint(self.transform_point(point.0))
    }

    pub fn transform_world_direction(self, direction: WorldDirection) -> ViewDirection {
        ViewDirection(self.transform_direction(direction.0))
    }

    /// Matrix product with the source's four ordered multiply-add terms per output word.
    pub fn multiply(self, rhs: Self) -> Self {
        let mut output = [0.0; 16];
        let a = self.elements;
        let b = rhs.elements;
        for row in 0..4 {
            for column in 0..4 {
                output[row * 4 + column] = (((a[row * 4] * b[column])
                    + (a[row * 4 + 1] * b[4 + column]))
                    + (a[row * 4 + 2] * b[8 + column]))
                    + (a[row * 4 + 3] * b[12 + column]);
            }
        }
        Self::from_elements(output)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Degrees(pub f32);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Radians(pub f32);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AspectRatio(pub f32);

impl Degrees {
    /// The conversion factor is supplied by the owning runtime policy.
    pub fn to_radians(self, radians_per_degree: f32) -> Radians {
        Radians(self.0 * radians_per_degree)
    }

    /// Interpolates already-unwrapped angles; wrapping limits belong to the caller policy.
    pub fn lerp_unwrapped(self, end: Self, amount: f32) -> Self {
        Self(self.0 + (end.0 - self.0) * amount)
    }
}

impl Radians {
    /// The conversion factor is supplied by the owning runtime policy.
    pub fn to_degrees(self, degrees_per_radian: f32) -> Degrees {
        Degrees(self.0 * degrees_per_radian)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldPoint(pub Vec3);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldDirection(pub Vec3);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewPoint(pub Vec3);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewDirection(pub Vec3);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClipPoint4(pub [f32; 4]);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint4(pub [f32; 4]);

/// Runtime coefficients remain explicit because the original reads them from data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CatmullRomPolicy {
    pub half: f32,
    /// Runtime coefficient used by the first weight's t² term and the second weight's constant.
    pub shared_first_quadratic_and_second_constant: f32,
    pub second_t_cubed: f32,
    pub second_t_squared: f32,
    pub third_t_cubed: f32,
    pub third_t_squared: f32,
}

/// Vector Catmull-Rom form used by the original math root, preserving f32 operation order.
pub fn catmull_rom_vec3(points: [Vec3; 4], t: f32, policy: CatmullRomPolicy) -> Vec3 {
    fn component(p: [f32; 4], t: f32, policy: CatmullRomPolicy) -> f32 {
        let t_squared = t * t;
        let t_cubed = t_squared * t;
        let first = ((t_squared * policy.shared_first_quadratic_and_second_constant - t_cubed) - t)
            * policy.half;
        let second = ((t_cubed * policy.second_t_cubed - t_squared * policy.second_t_squared)
            + policy.shared_first_quadratic_and_second_constant)
            * policy.half;
        let third = ((t_cubed * policy.third_t_cubed + t_squared * policy.third_t_squared) + t)
            * policy.half;
        let fourth = (t_cubed - t_squared) * policy.half;
        ((p[0] * first + p[1] * second) + p[2] * third) + p[3] * fourth
    }
    Vec3::new(
        component(
            [points[0].x, points[1].x, points[2].x, points[3].x],
            t,
            policy,
        ),
        component(
            [points[0].y, points[1].y, points[2].y, points[3].y],
            t,
            policy,
        ),
        component(
            [points[0].z, points[1].z, points[2].z, points[3].z],
            t,
            policy,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{ExtRegisters, Fpu, Xmm};
    use crate::memory::GuestMemory;
    use std::collections::BTreeMap;

    #[derive(Default, Clone, PartialEq)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            esp: 0x8000,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
    fn put_vec3(memory: &mut Memory, address: u32, value: Vec3) {
        for (offset, value) in [value.x, value.y, value.z].into_iter().enumerate() {
            memory.write32(address + offset as u32 * 4, value.to_bits());
        }
    }
    fn get_vec3(memory: &Memory, address: u32) -> Vec3 {
        Vec3::new(
            f32::from_bits(memory.read32(address)),
            f32::from_bits(memory.read32(address + 4)),
            f32::from_bits(memory.read32(address + 8)),
        )
    }
    fn same_float(actual: f32, expected: f32) {
        assert!(
            actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan()),
            "actual {:#010x}, expected {:#010x}",
            actual.to_bits(),
            expected.to_bits()
        );
    }
    fn same_vec3(actual: Vec3, expected: Vec3) {
        same_float(actual.x, expected.x);
        same_float(actual.y, expected.y);
        same_float(actual.z, expected.z);
    }

    #[test]
    fn point_and_direction_transforms_match_tier1_lifted_math_references_on_float_edges() {
        let edge = [
            0.0,
            -0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            1.0,
            -2.0,
            f32::MAX,
            f32::MIN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(0x7fc0_0123),
            f32::from_bits(0x7f80_0001),
        ];
        for value in edge {
            let mut matrix = [0.0f32; 16];
            matrix[0] = value;
            matrix[5] = 2.0;
            matrix[10] = -3.0;
            matrix[15] = 1.0;
            matrix[12] = 11.0;
            matrix[13] = -7.0;
            matrix[14] = 0.25;
            matrix[1] = 0.5;
            matrix[4] = -0.25;
            matrix[8] = 4.0;
            let point = Vec3::new(1.5, -2.0, value);
            let typed = Mat4::from_elements(matrix);
            for (va, native) in [
                (0x151e80, typed.transform_point(point)),
                (0x152090, typed.transform_direction(point)),
            ] {
                let mut memory = Memory::default();
                let mut regs = registers();
                let matrix_address = 0x1000;
                let vector_address = 0x2000;
                for (i, word) in matrix.iter().enumerate() {
                    memory.write32(matrix_address + i as u32 * 4, word.to_bits());
                }
                put_vec3(&mut memory, vector_address, point);
                memory.write32(regs.esp + 4, matrix_address);
                memory.write32(regs.esp + 8, vector_address);
                if va == 0x151e80 {
                    crate::c2r::t1937_math::game_matrix4x4_transform_point_vec3_inplace(
                        &mut memory,
                        &mut regs,
                    );
                } else {
                    crate::c2r::t1937_math::game_matrix4x4_transform_direction_vec3_inplace(
                        &mut memory,
                        &mut regs,
                    );
                }
                same_vec3(native, get_vec3(&memory, vector_address));
            }
        }
    }

    #[test]
    fn matrix_product_matches_tier1_lifted_math_reference() {
        let lhs = Mat4::from_elements([
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ]);
        let rhs = Mat4::from_elements([
            -1.0, 0.5, 2.0, -3.0, 4.0, 1.5, -2.0, 0.25, 3.0, -4.0, 0.125, 5.0, -6.0, 7.0, 8.0, -9.0,
        ]);
        let expected = lhs.multiply(rhs);
        let mut memory = Memory::default();
        let mut regs = registers();
        let out = 0x3000;
        let left = 0x1000;
        let right = 0x2000;
        for (base, matrix) in [(left, lhs), (right, rhs)] {
            for (i, word) in matrix.elements.iter().enumerate() {
                memory.write32(base + i as u32 * 4, word.to_bits());
            }
        }
        memory.write32(regs.esp + 4, out);
        memory.write32(regs.esp + 8, left);
        memory.write32(regs.esp + 12, right);
        crate::c2r::t1937_math::game_matrix4x4_multiply(&mut memory, &mut regs);
        for i in 0..16 {
            same_float(
                expected.elements[i],
                f32::from_bits(memory.read32(out + i as u32 * 4)),
            );
        }
    }

    #[test]
    fn vector_catmull_rom_matches_tier1_lifted_reference_for_ieee_edges() {
        let policy = CatmullRomPolicy {
            half: 0.5,
            shared_first_quadratic_and_second_constant: 2.0,
            second_t_cubed: 3.0,
            second_t_squared: 5.0,
            third_t_cubed: 3.0,
            third_t_squared: -5.0,
        };
        let edge = [
            0.0,
            -0.0,
            f32::from_bits(1),
            1.0,
            -2.0,
            f32::MAX,
            f32::MIN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(0x7fc0_0123),
            f32::from_bits(0x7f80_0001),
        ];
        for (case, edge_value) in edge.into_iter().enumerate() {
            let points = [
                Vec3::new(edge_value, 1.0, -2.0),
                Vec3::new(2.0, edge_value, 1.5),
                Vec3::new(-3.0, 4.0, edge_value),
                Vec3::new(0.25, -0.5, 5.0),
            ];
            let t = if case % 2 == 0 { 0.25 } else { edge_value };
            let native = catmull_rom_vec3(points, t, policy);
            let mut memory = Memory::default();
            let mut regs = registers();
            for (address, value) in [
                (0x475cd4, policy.half),
                (0x475d10, policy.shared_first_quadratic_and_second_constant),
                (0x475d18, policy.second_t_cubed),
                (0x4760cc, policy.second_t_squared),
                (0x4765ec, policy.third_t_cubed),
                (0x475db4, policy.third_t_squared),
            ] {
                memory.write32(address, value.to_bits());
            }
            let out = 0x3000;
            let addresses = [0x4000, 0x4100, 0x4200, 0x4300];
            for (address, point) in addresses.into_iter().zip(points) {
                put_vec3(&mut memory, address, point);
            }
            for (offset, value) in [
                (4, out),
                (8, t.to_bits()),
                (12, addresses[0]),
                (16, addresses[1]),
                (20, addresses[2]),
                (24, addresses[3]),
            ] {
                memory.write32(regs.esp + offset, value);
            }
            crate::c2r::t1937_math::game_math_catmull_rom_vec3(&mut memory, &mut regs);
            same_vec3(native, get_vec3(&memory, out));
        }
    }

    #[test]
    fn angle_units_require_runtime_conversion_and_unwrapped_interpolation() {
        assert_eq!(
            Degrees(180.0).to_radians(core::f32::consts::PI / 180.0),
            Radians(core::f32::consts::PI)
        );
        assert_eq!(
            Radians(core::f32::consts::PI).to_degrees(180.0 / core::f32::consts::PI),
            Degrees(180.0)
        );
        assert_eq!(
            Degrees(350.0).lerp_unwrapped(Degrees(370.0), 0.5),
            Degrees(360.0)
        );
    }
}
