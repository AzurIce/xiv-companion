use super::{VFX_IDENTITY_BASIS, VfxBinderMatrix};

/// Resolved Document getters (0x1403b2d20, ground input already resolved).
/// Main position, auxiliary basis and scalar/Euler caches are independent.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDocumentTransform {
    pub main_matrix: VfxBinderMatrix,
    pub auxiliary_matrix: VfxBinderMatrix,
    pub scale: [f32; 3],
    pub euler: [f32; 3],
}

impl VfxDocumentTransform {
    pub fn new(
        revised_rotation: [f32; 3],
        revised_scale: [f32; 3],
        revised_position: [f32; 3],
        host: VfxBinderMatrix,
        ground_y: Option<f32>,
    ) -> Self {
        // Document negates all three root angles before 37b700. Point bAGS
        // uses the original signs, so the two revision paths are distinct.
        let revision = VfxBinderMatrix::root_revision(
            revised_rotation.map(|value| -value),
            revised_scale,
            revised_position,
        );
        Self::from_revision(revision, host, ground_y)
    }

    /// Accept resolved trig/matrix input to compare the native source core
    /// separately from platform/client CRT differences.
    pub fn from_revision(
        revision: VfxBinderMatrix,
        host: VfxBinderMatrix,
        ground_y: Option<f32>,
    ) -> Self {
        let mut auxiliary = host.transform_matrix(revision);
        if let Some(y) = ground_y {
            auxiliary.position[1] = y;
        }
        let scale = auxiliary
            .basis
            .map(|[x, y, z]| ((x * x + y * y) + z * z).sqrt());
        // Native decomposition reads the raw composed basis. It does not
        // normalize columns, clamp asin or discard mirror/shear/NaN inputs.
        let basis = auxiliary.basis;
        let mut z = basis[0][1].atan2(basis[0][0]);
        let y = (-basis[0][2]).asin();
        let cosine = y.cos();
        let x = if cosine.abs() < f32::from_bits(0x38d1b717) {
            z = (-basis[1][0]).atan2(basis[1][1]);
            0.0
        } else {
            let x = (basis[1][2] / cosine).asin();
            if basis[2][2] < 0.0 {
                std::f32::consts::PI - x
            } else {
                x
            }
        };
        let main_matrix = VfxBinderMatrix {
            basis: VFX_IDENTITY_BASIS,
            position: auxiliary.position,
        };
        auxiliary.position = [0.0; 3];
        Self {
            main_matrix,
            auxiliary_matrix: auxiliary,
            scale,
            euler: [x, y, z],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_separates_world_position_sheared_basis_and_unsigned_lengths() {
        let host = VfxBinderMatrix {
            basis: [[1.0, 0.0, 0.0], [0.5, 1.0, 0.0], [0.0, 0.0, 1.0]],
            position: [0.25, -0.5, 1.0],
        };
        let value =
            VfxDocumentTransform::new([0.0; 3], [-2.0, 3.0, 0.5], [1.0, 2.0, 3.0], host, None);
        assert_eq!(value.main_matrix.position, [2.25, 1.5, 4.0]);
        assert_eq!(value.main_matrix.basis, VFX_IDENTITY_BASIS);
        assert_eq!(value.auxiliary_matrix.position, [0.0; 3]);
        assert_eq!(
            value.auxiliary_matrix.basis,
            [[-2.0, 0.0, 0.0], [1.5, 3.0, 0.0], [0.0, 0.0, 0.5]]
        );
        assert_eq!(value.scale, [2.0, 11.25_f32.sqrt(), 0.5]);
        assert_eq!(value.euler[2], std::f32::consts::PI);
    }
}
