/// Document override and root distance settings consumed by the particle
/// depth getter (0x1403e3640), independently of geometric scale and DOTy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxDepthOffsetParameters {
    /// Resolved Document state +0x94. Zero (including -0) selects authored DpOf.
    pub document_override: f32,
    /// Root ZBMs: maximum multiplier; values <= 1 disable distance scaling.
    pub bias_z_max_scale: f32,
    /// Root ZBMd: distance at which the maximum multiplier is reached.
    pub bias_z_max_distance: f32,
}

impl VfxDepthOffsetParameters {
    /// Evaluate with the current particle object's position and camera in the
    /// same coordinate space. Smpl children share their spawner object's
    /// result; their individual positions must not be substituted here.
    pub fn evaluate(
        &self,
        authored_offset: f32,
        binder_multiplier: f32,
        position: [f32; 3],
        camera_position: [f32; 3],
    ) -> f32 {
        let offset = if self.document_override == 0.0 {
            authored_offset
        } else {
            self.document_override
        };
        let mut offset = offset * binder_multiplier;
        if self.bias_z_max_scale > 1.0 && self.bias_z_max_distance > 0.0 {
            let ray: [f32; 3] = std::array::from_fn(|axis| camera_position[axis] - position[axis]);
            let distance = ((ray[1] * ray[1] + ray[0] * ray[0]) + ray[2] * ray[2]).sqrt();
            let scaled = (distance / self.bias_z_max_distance) * self.bias_z_max_scale;
            // Explicit comparisons match COMISS/MINSS, including NaN. Rust's
            // min/clamp would suppress NaN or reject disabled root settings.
            let factor = if scaled < 1.0 {
                1.0
            } else if self.bias_z_max_scale < scaled {
                self.bias_z_max_scale
            } else {
                scaled
            };
            offset *= factor;
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_override_precedes_binder_and_distance_multipliers() {
        let mut parameters = VfxDepthOffsetParameters {
            document_override: -0.125,
            bias_z_max_scale: 4.0,
            bias_z_max_distance: 8.0,
        };
        for (distance, expected) in [(0.0, -0.25), (2.0, -0.25), (4.0, -0.5), (9.0, -1.0)] {
            assert_eq!(
                parameters.evaluate(0.2, 2.0, [0.0; 3], [distance, 0.0, 0.0]),
                expected
            );
        }
        parameters.document_override = -0.0;
        assert_eq!(
            parameters.evaluate(0.2, 2.0, [0.0; 3], [4.0, 0.0, 0.0]),
            0.8
        );
    }

    #[test]
    fn disabled_distance_settings_do_not_read_camera_and_nan_is_not_hidden() {
        for (scale, distance) in [(0.0, 2.0), (1.0, 2.0), (4.0, 0.0), (4.0, -2.0)] {
            let parameters = VfxDepthOffsetParameters {
                document_override: 0.0,
                bias_z_max_scale: scale,
                bias_z_max_distance: distance,
            };
            assert_eq!(
                parameters.evaluate(-0.2, 3.0, [0.0; 3], [f32::NAN; 3]),
                -0.6
            );
        }
        let parameters = VfxDepthOffsetParameters {
            document_override: 0.0,
            bias_z_max_scale: 4.0,
            bias_z_max_distance: 2.0,
        };
        assert!(
            parameters
                .evaluate(0.0, f32::NAN, [0.0; 3], [0.0; 3])
                .is_nan()
        );
        assert!(
            parameters
                .evaluate(0.2, 1.0, [0.0; 3], [f32::NAN; 3])
                .is_nan()
        );
    }
}
