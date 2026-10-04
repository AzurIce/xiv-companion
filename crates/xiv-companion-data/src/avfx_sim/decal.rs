use super::{AvfxParticle, AvfxParticleData, CurveAges, curve_value_seeded_at};

/// Evaluated inputs for the client's dedicated Decal / DecalRing projection path.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDecal {
    pub ring: bool,
    /// Raw SS. Camera-dependent distance compensation is applied by the draw path.
    pub scaling_scale: f32,
    /// Authored DDTT. The client can override 0 to 2 using external scene state.
    pub depth_type: i32,
    /// Current WID + WIDR value for DecalRing; zero for Decal.
    pub ring_width: f32,
    /// RF for DecalRing; zero for Decal.
    pub ring_fan: f32,
    /// The other two local scale axes are retained in [`super::VfxQuad::size`].
    pub scale_z: f32,
}

impl VfxDecal {
    /// Client table 0x142159ca8, used by Document's independent Decal list.
    /// Bits 0/1 select ordinary passes; bits 2/3 select GBuffer passes.
    /// Combined values select both passes, not a third independent target.
    /// This uses authored DDTT; the external scene override is not applied here.
    pub fn draw_pass_mask(self) -> Option<u8> {
        match self.depth_type {
            0 => Some(1),
            1 => Some(2),
            2 => Some(3),
            3 => Some(4),
            4 => Some(8),
            5 => Some(12),
            _ => None,
        }
    }

    /// Match the mask passed to the client's independent Decal draw traversal.
    pub fn participates_in_draw_pass(self, mask: u8) -> bool {
        self.draw_pass_mask()
            .is_some_and(|passes| passes & mask != 0)
    }

    /// Target-family eligibility only: the preview does not yet reproduce the
    /// separate client passes or their scene-dependent gates.
    pub fn uses_forward_target(self) -> bool {
        self.participates_in_draw_pass(3)
    }

    pub fn uses_deferred_gbuffer(self) -> bool {
        self.participates_in_draw_pass(12)
    }
}

pub(super) fn sample_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    scale_z: f32,
) -> Option<VfxDecal> {
    let result = match &particle.data {
        AvfxParticleData::Decal(data) => VfxDecal {
            ring: false,
            scaling_scale: data.scaling_scale,
            depth_type: data.ddtt,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z,
        },
        AvfxParticleData::DecalRing(data) => VfxDecal {
            ring: true,
            scaling_scale: data.scaling_scale,
            depth_type: data.ddtt,
            ring_width: curve_value_seeded_at(
                &data.width,
                &data.width_random,
                ages,
                0.0,
                seed ^ 0xDEC4_1A11,
            ),
            ring_fan: data.ring_fan,
            scale_z,
        },
        _ => return None,
    };
    [
        result.scaling_scale,
        result.ring_width,
        result.ring_fan,
        result.scale_z,
    ]
    .into_iter()
    .all(f32::is_finite)
    .then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{
        AvfxCurve, AvfxCurveKey, AvfxParticleDataDecal, AvfxParticleDataDecalRing, BEHAVIOR_ADD,
        ParticleType,
    };

    #[test]
    fn decal_keeps_projection_parameters_and_third_scale_axis() {
        let particle = AvfxParticle {
            particle_type: Some(ParticleType::Decal),
            data: AvfxParticleData::Decal(AvfxParticleDataDecal {
                scaling_scale: 0.75,
                ddtt: 3,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            sample_at(
                &particle,
                CurveAges {
                    local: 4.0,
                    total: 14.0,
                },
                9,
                -2.5,
            ),
            Some(VfxDecal {
                ring: false,
                scaling_scale: 0.75,
                depth_type: 3,
                ring_width: 0.0,
                ring_fan: 0.0,
                scale_z: -2.5,
            })
        );
    }

    #[test]
    fn ring_width_keeps_curve_clocks_and_seeded_randomness() {
        let particle = AvfxParticle {
            particle_type: Some(ParticleType::DecalRing),
            data: AvfxParticleData::DecalRing(AvfxParticleDataDecalRing {
                width: AvfxCurve {
                    keys: vec![
                        AvfxCurveKey {
                            time: 0,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        AvfxCurveKey {
                            time: 10,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: 10.0,
                        },
                    ],
                    post_behavior: BEHAVIOR_ADD,
                    ..Default::default()
                },
                width_random: AvfxCurve {
                    keys: vec![AvfxCurveKey {
                        time: 0,
                        interpolation: 1,
                        x: 0.0,
                        y: 0.0,
                        z: 2.0,
                    }],
                    random_type: 3,
                    ..Default::default()
                },
                scaling_scale: 0.5,
                ring_fan: 0.25,
                ddtt: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let ages = CurveAges {
            local: 5.0,
            total: 25.0,
        };
        let first = sample_at(&particle, ages, 11, 3.0).unwrap();
        assert!(first.ring);
        assert!((23.0..=27.0).contains(&first.ring_width));
        assert_eq!(first, sample_at(&particle, ages, 11, 3.0).unwrap());
        assert_ne!(first, sample_at(&particle, ages, 12, 3.0).unwrap());
    }

    #[test]
    fn nonfinite_projection_inputs_are_rejected() {
        let mut particle = AvfxParticle {
            particle_type: Some(ParticleType::Decal),
            data: AvfxParticleData::Decal(AvfxParticleDataDecal {
                scaling_scale: f32::NAN,
                ..Default::default()
            }),
            ..Default::default()
        };
        let ages = CurveAges {
            local: 0.0,
            total: 0.0,
        };
        assert!(sample_at(&particle, ages, 1, 1.0).is_none());
        if let AvfxParticleData::Decal(data) = &mut particle.data {
            data.scaling_scale = 1.0;
        }
        assert!(sample_at(&particle, ages, 1, f32::INFINITY).is_none());
    }

    #[test]
    fn ddtt_target_groups_match_the_client_dispatch_table() {
        for depth_type in 0..=5 {
            let decal = VfxDecal {
                ring: false,
                scaling_scale: 1.0,
                depth_type,
                ring_width: 0.0,
                ring_fan: 0.0,
                scale_z: 1.0,
            };
            let expected_passes: &[u8] = match depth_type {
                0 => &[1],
                1 => &[2],
                2 => &[1, 2],
                3 => &[4],
                4 => &[8],
                5 => &[4, 8],
                _ => unreachable!(),
            };
            let selected: Vec<_> = [1, 2, 4, 8]
                .into_iter()
                .filter(|mask| decal.participates_in_draw_pass(*mask))
                .collect();
            assert_eq!(selected, expected_passes);
            assert!(!decal.participates_in_draw_pass(0));
            assert!(!decal.participates_in_draw_pass(0xf0));
            assert_eq!(decal.uses_forward_target(), depth_type <= 2);
            assert_eq!(decal.uses_deferred_gbuffer(), depth_type >= 3);
        }
        let unknown = VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 6,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 1.0,
        };
        for depth_type in [-1, 6, 256, i32::MAX] {
            let unknown = VfxDecal {
                depth_type,
                ..unknown
            };
            assert_eq!(unknown.draw_pass_mask(), None);
            assert!(!unknown.participates_in_draw_pass(u8::MAX));
        }
        assert!(!unknown.uses_forward_target());
        assert!(!unknown.uses_deferred_gbuffer());
    }
}
