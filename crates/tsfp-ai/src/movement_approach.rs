// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed owner for the measured XZ approach helper at 0x266250.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// If either XZ delta exceeds the 0.01 dead zone, move by the supplied fraction.
/// Y is unchanged. The unordered float comparisons also take the move path.
pub fn approach_xz(position: &mut Position3, target: Position3, fraction: f32) -> bool {
    let dx = position.x - target.x;
    let dz = position.z - target.z;
    if dx.abs() <= 0.01 && dz.abs() <= 0.01 {
        return false;
    }
    position.x -= dx * fraction;
    position.z -= dz * fraction;
    true
}
