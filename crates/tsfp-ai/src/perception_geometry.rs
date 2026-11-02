// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned geometry boundary for REA's angular admission predicate (`0x161730`).
//!
//! This is an angle test, not a ray or occlusion query. Direction delegates remain
//! behind [`DirectionProvider`] until their own contracts are ported. Verification:
//! Tier 1 lifted differential on fabricated typed inputs, including float edges.

use crate::navigation_scoring::ActorId;

#[inline]
pub(crate) fn sqrt_f64(value: f64) -> f64 {
    if value == 0.0 || value == f64::INFINITY || value.is_nan() {
        return value;
    }
    if value < 0.0 {
        return f64::NAN;
    }
    // A bit-level estimate followed by Newton steps keeps this pure core
    // operation available to both std and no_std owners.
    let mut estimate = f64::from_bits((value.to_bits() >> 1) + 0x1ff8_0000_0000_0000);
    for _ in 0..8 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

#[inline]
fn sqrt_f32(value: f32) -> f32 {
    if value == 0.0 || value == f32::INFINITY || value.is_nan() {
        return value;
    }
    if value < 0.0 {
        return f32::NAN;
    }
    let mut estimate = f32::from_bits((value.to_bits() >> 1) + 0x1fc0_0000);
    for _ in 0..8 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixDirectionRoute {
    /// Original `0xD9050`: model-derived direction rotated around the actor pivot.
    ModelYaw,
    /// Original `0xD9080`: actor/model direction without the local yaw argument.
    ActorBone,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorGeometry {
    pub id: ActorId,
    pub position: Vec3,
    pub yaw: f32,
    pub kind: u16,
    pub flags: u32,
    /// Signed identifier read from the actor extension at `+0x4A`.
    pub direction_selector: i16,
    /// Resolved vertical extent selected by the caller from the documented table.
    pub vertical_extent: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngularAdmissionQuery {
    pub actor: ActorGeometry,
    pub target: Vec3,
    pub threshold: f32,
    /// The lifted x87 helper rounds intermediates to 24 significant bits when
    /// `(control & 0x300) == 0`; other control settings retain f64 intermediates.
    pub x87_control: u16,
}

/// Opaque direction operations kept at the edges of the measured predicate.
/// Implementations own their model data and may not infer occlusion from this API.
pub trait DirectionProvider {
    /// Result of the opaque original `0xD6EA0(selector)` lookup. It is called
    /// before every route choice, including when `selector` is zero.
    fn has_delegated_direction(&mut self, selector: i16) -> bool;

    /// Original `0xD6EA0` selected the `0x3797B0` vector source. No local
    /// normalization is applied to this returned vector.
    fn delegated_direction(&mut self, actor: ActorId, selector: i16) -> Vec3;

    /// Supply the matrix X/Z values written by the selected opaque matrix helper.
    /// `yaw_radians` is present only for the `0xD9050` route.
    fn matrix_direction_xz(
        &mut self,
        actor: ActorId,
        route: MatrixDirectionRoute,
        origin: Vec3,
        yaw_radians: Option<f32>,
    ) -> Vec2;

    /// Read one value from the original 2048-entry yaw table.
    fn yaw_table_value(&mut self, index: u16) -> f32;
}

/// Evaluate the represented `0x161730` angular admission predicate.
///
/// The vertical origin is the actor position clamped toward the target using
/// the resolved extent. The displacement length and strict normalized dot
/// comparison preserve the lifted x87/SSE operation order. A true result says
/// only that the angle exceeds `threshold`; it says nothing about a clear ray.
pub fn angular_admitted(
    query: AngularAdmissionQuery,
    directions: &mut impl DirectionProvider,
) -> bool {
    let origin = adjusted_origin(
        query.actor.position,
        query.target,
        query.actor.vertical_extent,
    );
    let direction = select_direction(query.actor, origin, directions);

    let dx = x87_pc(
        f64::from(query.target.x) - f64::from(origin.x),
        query.x87_control,
    ) as f32;
    let dy = x87_pc(
        f64::from(query.target.y) - f64::from(origin.y),
        query.x87_control,
    ) as f32;
    let dz = x87_pc(
        f64::from(query.target.z) - f64::from(origin.z),
        query.x87_control,
    ) as f32;

    // Lifted x87 sequence squares and accumulates Z, Y, then X before sqrt/store.
    let z2 = x87_pc(f64::from(dz) * f64::from(dz), query.x87_control);
    let y2 = x87_pc(f64::from(dy) * f64::from(dy), query.x87_control);
    let x2 = x87_pc(f64::from(dx) * f64::from(dx), query.x87_control);
    let zy = x87_pc(z2 + y2, query.x87_control);
    let squared_length = x87_pc(zy + x2, query.x87_control);
    let length = x87_pc(sqrt_f64(squared_length), query.x87_control) as f32;

    // The source selects zero for zero or unordered length; it does not sanitize
    // non-finite displacement components before their later multiplications.
    let inverse_length = if length > 0.0 { 1.0f32 / length } else { 0.0 };
    let scaled_x = inverse_length * dx;
    let scaled_y = inverse_length * dy;
    let scaled_z = inverse_length * dz;

    // Preserve scalar SSE accumulation order and an ordered, strict comparison.
    let z_dot = scaled_z * direction.z;
    let y_dot = scaled_y * direction.y;
    let mut dot = z_dot + y_dot;
    let x_dot = scaled_x * direction.x;
    dot += x_dot;
    dot > query.threshold
}

fn adjusted_origin(actor: Vec3, target: Vec3, extent: f32) -> Vec3 {
    let upper = actor.y + extent;
    let target_y = target.y;
    let origin_y = if !(target_y < actor.y || target_y.is_nan())
        && !(upper < target_y || upper.is_nan() || target_y.is_nan())
    {
        target_y
    } else if target_y <= upper || target_y.is_nan() || upper.is_nan() {
        actor.y
    } else {
        upper
    };
    Vec3 {
        y: origin_y,
        ..actor
    }
}

fn select_direction(
    actor: ActorGeometry,
    origin: Vec3,
    directions: &mut impl DirectionProvider,
) -> Vec3 {
    if directions.has_delegated_direction(actor.direction_selector) {
        return directions.delegated_direction(actor.id, actor.direction_selector);
    }

    if actor.kind == 0x607 {
        return normalized_matrix_direction(directions.matrix_direction_xz(
            actor.id,
            MatrixDirectionRoute::ModelYaw,
            origin,
            Some(actor.yaw * 0.017_453_292),
        ));
    }

    if actor.flags & 0xffff != 0 {
        return normalized_matrix_direction(directions.matrix_direction_xz(
            actor.id,
            MatrixDirectionRoute::ActorBone,
            origin,
            None,
        ));
    }

    let yaw_radians = actor.yaw * 0.017_453_292;
    let yaw_scaled = yaw_radians * 325.949_3;
    let yaw_index = cvttss2si(yaw_scaled + 0.5);
    let index = (yaw_index as u32 & 0x7ff) as u16;
    let quarter_turn = (index.wrapping_add(0x200) & 0x7ff) as u16;
    Vec3 {
        x: directions.yaw_table_value(index),
        y: 0.0,
        z: directions.yaw_table_value(quarter_turn),
    }
}

fn normalized_matrix_direction(xz: Vec2) -> Vec3 {
    let squared_length = xz.z * xz.z + xz.x * xz.x;
    let (x, z) = if squared_length > 1.0e-7 {
        let inverse = 1.0f32 / sqrt_f32(squared_length);
        (xz.x * inverse, xz.z * inverse)
    } else {
        (xz.x, xz.z)
    };
    Vec3 { x, y: 0.0, z }
}

/// Model x87's extended precision-control helper for operations whose values
/// are subsequently stored as f32 by the lifted body.
fn x87_pc(value: f64, control: u16) -> f64 {
    if control & 0x300 != 0 {
        return value;
    }
    round_to_24_significand_bits(value)
}

fn round_to_24_significand_bits(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    // All products in this path originate from f32 operands and remain normal
    // f64 values, including f32 subnormals squared.
    if exponent == 0 {
        return value;
    }
    let significand = (1u64 << 52) | (bits & ((1u64 << 52) - 1));
    let retained = significand >> 29;
    let remainder = significand & ((1u64 << 29) - 1);
    let halfway = 1u64 << 28;
    let rounded =
        retained + u64::from(remainder > halfway || (remainder == halfway && retained & 1 != 0));
    let (rounded, exponent) = if rounded == (1u64 << 24) {
        (1u64 << 23, exponent + 1)
    } else {
        (rounded, exponent)
    };
    let sign = bits & (1u64 << 63);
    let fraction = (rounded & ((1u64 << 23) - 1)) << 29;
    f64::from_bits(sign | ((exponent as u64) << 52) | fraction)
}

/// x86 `cvttss2si`: invalid or out-of-range values produce `INT_MIN`.
fn cvttss2si(value: f32) -> i32 {
    if !value.is_finite() || value < i32::MIN as f32 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directions {
        direction: Vec3,
        matrix: Vec2,
        table: [f32; 2],
        delegated: usize,
        matrix_route: Option<MatrixDirectionRoute>,
        table_calls: Vec<u16>,
    }

    impl DirectionProvider for Directions {
        fn has_delegated_direction(&mut self, selector: i16) -> bool {
            selector != 0
        }

        fn delegated_direction(&mut self, _: ActorId, _: i16) -> Vec3 {
            self.delegated += 1;
            self.direction
        }

        fn matrix_direction_xz(
            &mut self,
            _: ActorId,
            route: MatrixDirectionRoute,
            _: Vec3,
            _: Option<f32>,
        ) -> Vec2 {
            self.matrix_route = Some(route);
            self.matrix
        }

        fn yaw_table_value(&mut self, index: u16) -> f32 {
            self.table_calls.push(index);
            if self.table_calls.len() == 1 {
                self.table[0]
            } else {
                self.table[1]
            }
        }
    }

    fn actor() -> ActorGeometry {
        ActorGeometry {
            id: ActorId(1),
            position: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            yaw: 90.0,
            kind: 0,
            flags: 0,
            direction_selector: 1,
            vertical_extent: 4.0,
        }
    }

    #[test]
    fn route_selection_keeps_delegate_and_matrix_boundaries_separate() {
        let mut query_actor = actor();
        let mut directions = Directions {
            direction: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            matrix: Vec2 { x: 3.0, z: 4.0 },
            table: [0.25, 0.75],
            delegated: 0,
            matrix_route: None,
            table_calls: Vec::new(),
        };
        assert_eq!(
            select_direction(query_actor, query_actor.position, &mut directions),
            directions.direction
        );
        assert_eq!(directions.delegated, 1);

        query_actor.direction_selector = 0;
        query_actor.kind = 0x607;
        assert_eq!(
            select_direction(query_actor, query_actor.position, &mut directions),
            Vec3 {
                x: 0.6,
                y: 0.0,
                z: 0.8
            }
        );
        assert_eq!(
            directions.matrix_route,
            Some(MatrixDirectionRoute::ModelYaw)
        );

        query_actor.kind = 0;
        query_actor.flags = 1;
        select_direction(query_actor, query_actor.position, &mut directions);
        assert_eq!(
            directions.matrix_route,
            Some(MatrixDirectionRoute::ActorBone)
        );

        query_actor.flags = 0;
        select_direction(query_actor, query_actor.position, &mut directions);
        assert_eq!(directions.table_calls, [0x200, 0x400]);
    }

    #[test]
    fn vertical_origin_clamp_preserves_nan_and_negative_extent_branches() {
        let origin = adjusted_origin(
            Vec3 {
                x: 1.0,
                y: 5.0,
                z: 2.0,
            },
            Vec3 {
                x: 9.0,
                y: 8.0,
                z: 7.0,
            },
            2.0,
        );
        assert_eq!(
            origin,
            Vec3 {
                x: 1.0,
                y: 7.0,
                z: 2.0
            }
        );
        assert_eq!(
            adjusted_origin(
                Vec3 {
                    x: 1.0,
                    y: 5.0,
                    z: 2.0
                },
                Vec3 {
                    x: 9.0,
                    y: f32::NAN,
                    z: 7.0
                },
                2.0,
            )
            .y,
            5.0
        );
        assert_eq!(
            adjusted_origin(
                Vec3 {
                    x: 1.0,
                    y: 5.0,
                    z: 2.0
                },
                Vec3 {
                    x: 9.0,
                    y: 6.0,
                    z: 7.0
                },
                -2.0,
            )
            .y,
            3.0
        );
    }

    #[test]
    fn invalid_yaw_conversion_uses_x86_int_min_wrapping_index() {
        assert_eq!(cvttss2si(f32::NAN), i32::MIN);
        assert_eq!(cvttss2si(f32::INFINITY), i32::MIN);
        assert_eq!(cvttss2si(1.9), 1);
        assert_eq!((cvttss2si(f32::NAN) as u32 & 0x7ff) as u16, 0);
    }
}
