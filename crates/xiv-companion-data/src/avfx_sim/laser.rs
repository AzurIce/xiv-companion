use super::{AvfxParticle, AvfxParticleData, CurveAges, curve_value_seeded_at};

/// Evaluated Laser dimensions. The renderer uses a separate procedural strip
/// path; zero dimensions are retained so the caller can apply client-like
/// degeneracy checks without confusing Laser with a Quad.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxLaser {
    pub length: f32,
    pub width: f32,
    pub scale: [f32; 3],
}

#[cfg(test)]
pub(super) fn sample_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    scale: [f32; 3],
) -> Option<VfxLaser> {
    sample_at_with_client(particle, ages, seed, scale, None)
}

pub(super) fn sample_at_with_client(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    scale: [f32; 3],
    dimensions: Option<[f32; 2]>,
) -> Option<VfxLaser> {
    let AvfxParticleData::Laser(data) = &particle.data else {
        return None;
    };
    let [length, width] = dimensions.unwrap_or_else(|| {
        [
            curve_value_seeded_at(&data.length, &data.length_random, ages, 0.0, seed ^ 0x4c45),
            curve_value_seeded_at(&data.width, &data.width_random, ages, 0.0, seed ^ 0x5744),
        ]
    });
    let result = VfxLaser {
        length,
        width,
        scale,
    };
    result.length.is_finite().then_some(result).filter(|laser| {
        laser.width.is_finite() && laser.scale.iter().all(|value| value.is_finite())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{
        AvfxCurve, AvfxCurveKey, AvfxParticleDataLaser, BEHAVIOR_ADD, BEHAVIOR_REPEAT, ParticleType,
    };

    fn ramp(post_behavior: u32) -> AvfxCurve {
        AvfxCurve {
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
            post_behavior,
            ..Default::default()
        }
    }

    fn particle() -> AvfxParticle {
        AvfxParticle {
            particle_type: Some(ParticleType::Laser),
            data: AvfxParticleData::Laser(AvfxParticleDataLaser {
                length: ramp(BEHAVIOR_ADD),
                width: ramp(BEHAVIOR_REPEAT),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn dimensions_keep_dual_curve_clocks_and_scale() {
        let sampled = sample_at(
            &particle(),
            CurveAges {
                local: 5.0,
                total: 25.0,
            },
            7,
            [-2.0, 3.0, 4.0],
        )
        .unwrap();
        assert_eq!(sampled.length, 25.0);
        assert_eq!(sampled.width, 5.0);
        assert_eq!(sampled.scale, [-2.0, 3.0, 4.0]);
    }

    #[test]
    fn random_curves_are_seeded_and_nonfinite_values_are_rejected() {
        let mut particle = particle();
        if let AvfxParticleData::Laser(data) = &mut particle.data {
            data.length_random = AvfxCurve {
                keys: vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 2.0,
                }],
                random_type: 3,
                ..Default::default()
            };
        }
        let ages = CurveAges {
            local: 4.0,
            total: 4.0,
        };
        let first = sample_at(&particle, ages, 11, [1.0; 3]).unwrap();
        assert_eq!(first, sample_at(&particle, ages, 11, [1.0; 3]).unwrap());
        assert_ne!(
            first.length,
            sample_at(&particle, ages, 12, [1.0; 3]).unwrap().length
        );

        let AvfxParticleData::Laser(data) = &mut particle.data else {
            unreachable!()
        };
        data.width.keys[0].z = f32::INFINITY;
        assert!(sample_at(&particle, ages, 11, [1.0; 3]).is_none());
        let AvfxParticleData::Laser(data) = &mut particle.data else {
            unreachable!()
        };
        data.width.keys[0].z = 0.0;
        assert!(sample_at(&particle, ages, 11, [1.0, f32::NAN, 1.0]).is_none());
    }
}
