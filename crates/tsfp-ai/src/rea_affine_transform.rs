// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable affine point transform reconstructed from original 0x151E80.

/// A 3D point represented by three single-precision components.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Column-major 4x4 matrix used by the original angle-matrix helper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4x4 {
    pub columns: [[f32; 4]; 4],
}

impl Matrix4x4 {
    /// Apply 0x1533E0's sine/cosine slots to a caller-supplied template.
    ///
    /// The original copies its template from a global data table before these
    /// writes. Passing that template keeps original data outside this code.
    pub fn with_angle_template(mut self, radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        self.columns[1][1] = cos;
        self.columns[1][2] = sin;
        self.columns[2][1] = -sin;
        self.columns[2][2] = cos;
        self
    }

    /// View the first three rows of the four matrix columns as the layout
    /// consumed by 0x152090 and 0x151E80.
    pub fn affine_3x4(self) -> Affine3x4 {
        Affine3x4 {
            columns: self.columns.map(|column| [column[0], column[1], column[2]]),
        }
    }
}

/// Column-major 3x4 affine transform matching the original offsets.
///
/// `columns[0..3]` are the X, Y and Z basis columns; `columns[3]` is the
/// translation. In the original record their component offsets are 0/4/8,
/// 0x10/0x14/0x18, 0x20/0x24/0x28 and 0x30/0x34/0x38 respectively.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine3x4 {
    pub columns: [[f32; 3]; 4],
}

impl Affine3x4 {
    /// Apply the original 0x152090 3x3 basis transform, without translation.
    ///
    /// The original reads all three input components before writing them and
    /// accumulates Z, Y, then X basis contributions.
    pub fn transform_vector(self, vector: &mut Point3) {
        let Point3 { x, y, z } = *vector;
        // The original helper writes matrix rows 1, 2, then 0 to the
        // destination's x, y, then z fields, respectively.
        let mut out_x = self.columns[2][1] * z;
        out_x += self.columns[1][1] * y;
        out_x += self.columns[0][1] * x;

        let mut out_y = self.columns[2][2] * z;
        out_y += self.columns[1][2] * y;
        out_y += self.columns[0][2] * x;

        let mut out_z = self.columns[2][0] * z;
        out_z += self.columns[1][0] * y;
        out_z += self.columns[0][0] * x;

        *vector = Point3 {
            x: out_x,
            y: out_y,
            z: out_z,
        };
    }

    /// Apply the original 0x151E80 transform to a point in place.
    ///
    /// The explicit accumulation order follows the observed scalar SSE
    /// operations: Z basis, Y basis, X basis, then translation.
    pub fn transform_point(self, point: &mut Point3) {
        let Point3 { x, y, z } = *point;
        let mut out_x = self.columns[2][0] * z;
        out_x += self.columns[1][0] * y;
        out_x += self.columns[0][0] * x;
        out_x += self.columns[3][0];

        let mut out_y = self.columns[2][1] * z;
        out_y += self.columns[1][1] * y;
        out_y += self.columns[0][1] * x;
        out_y += self.columns[3][1];

        let mut out_z = self.columns[2][2] * z;
        out_z += self.columns[1][2] * y;
        out_z += self.columns[0][2] * x;
        out_z += self.columns[3][2];

        *point = Point3 {
            x: out_x,
            y: out_y,
            z: out_z,
        };
    }
}

/// Compose the observed `0x1533E0` template-angle setup with the following
/// in-place `0x152090` vector transform.
pub fn transform_vector_with_angle_template(
    template: Matrix4x4,
    angle_radians: f32,
    vector: &mut Point3,
) -> Matrix4x4 {
    let matrix = template.with_angle_template(angle_radians);
    matrix.affine_3x4().transform_vector(vector);
    matrix
}

#[cfg(test)]
mod tests {
    use super::{Affine3x4, Matrix4x4, Point3, transform_vector_with_angle_template};

    #[test]
    fn angle_template_changes_only_observed_slots() {
        let template = Matrix4x4 {
            columns: [[1.0; 4], [2.0; 4], [3.0; 4], [4.0; 4]],
        };

        let matrix = template.with_angle_template(0.0);

        assert_eq!(matrix.columns[1][1], 1.0);
        assert_eq!(matrix.columns[1][2], 0.0);
        assert_eq!(matrix.columns[2][1], 0.0);
        assert_eq!(matrix.columns[2][2], 1.0);
        assert_eq!(matrix.columns[0], template.columns[0]);
        assert_eq!(matrix.columns[3], template.columns[3]);
        assert_eq!(matrix.columns[1][0], 2.0);
        assert_eq!(matrix.columns[2][0], 3.0);
    }

    #[test]
    fn identity_preserves_point() {
        let transform = Affine3x4 {
            columns: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
            ],
        };
        let mut point = Point3 {
            x: 2.0,
            y: -3.0,
            z: 4.5,
        };

        transform.transform_point(&mut point);

        assert_eq!(
            point,
            Point3 {
                x: 2.0,
                y: -3.0,
                z: 4.5
            }
        );
    }

    #[test]
    fn combines_basis_columns_then_translation() {
        let transform = Affine3x4 {
            columns: [
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [7.0, 8.0, 9.0],
                [10.0, 11.0, 12.0],
            ],
        };
        let mut point = Point3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };

        transform.transform_point(&mut point);

        assert_eq!(
            point,
            Point3 {
                x: 40.0,
                y: 47.0,
                z: 54.0
            }
        );
    }

    #[test]
    fn vector_transform_ignores_translation() {
        let transform = Affine3x4 {
            columns: [
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [7.0, 8.0, 9.0],
                [100.0, 200.0, 300.0],
            ],
        };
        let mut vector = Point3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };

        transform.transform_vector(&mut vector);

        assert_eq!(
            vector,
            Point3 {
                x: 36.0,
                y: 42.0,
                z: 30.0
            }
        );
    }

    #[test]
    fn angle_template_composition_returns_matrix_and_transforms_in_place() {
        let template = Matrix4x4 {
            columns: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let mut vector = Point3 {
            x: 3.0,
            y: 4.0,
            z: 5.0,
        };

        let matrix = transform_vector_with_angle_template(template, 0.0, &mut vector);

        assert_eq!(matrix, template);
        assert_eq!(
            vector,
            Point3 {
                x: 4.0,
                y: 5.0,
                z: 3.0
            }
        );
    }
}
