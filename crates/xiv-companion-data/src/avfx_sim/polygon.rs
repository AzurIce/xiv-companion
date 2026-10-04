use super::{AvfxParticle, AvfxParticleData, CurveAges, curve_random_value_at};

/// Evaluated Polygon data. The client streams one center plus a closed rim.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxPolygon {
    pub count: u32,
    /// The other two local scale axes are retained in `VfxQuad::size`.
    pub scale_z: f32,
}

impl VfxPolygon {
    pub fn vertex_count(self) -> u32 {
        self.count * 3
    }
}

#[cfg(test)]
pub(super) fn sample(
    particle: &AvfxParticle,
    age: f32,
    seed: u64,
    scale_z: f32,
) -> Option<VfxPolygon> {
    sample_at(
        particle,
        CurveAges {
            local: age,
            total: age,
        },
        seed,
        scale_z,
    )
}

#[cfg(test)]
pub(super) fn sample_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    scale_z: f32,
) -> Option<VfxPolygon> {
    sample_at_with_client(particle, ages, seed, scale_z, None)
}

pub(super) fn sample_at_with_client(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    scale_z: f32,
    count: Option<f32>,
) -> Option<VfxPolygon> {
    let data = match &particle.data {
        AvfxParticleData::Polygon(data) => data,
        _ => return None,
    };
    let count = count.unwrap_or_else(|| {
        data.count.value_at(ages.local, ages.total, 0.0)
            + curve_random_value_at(&data.count_random, ages, seed ^ 0x504F_4C59)
    });
    if !count.is_finite() || !scale_z.is_finite() {
        return None;
    }
    let count = count.trunc();
    // The client stream reserves 48 bytes per center/rim vertex and rejects a
    // write past 0xc0000 bytes. Counts below three cannot form a polygon.
    if !(3.0..=16_383.0).contains(&count) {
        return None;
    }
    Some(VfxPolygon {
        count: count as u32,
        scale_z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey, AvfxParticleDataPolygon, ParticleType};

    fn curve(value: f32) -> AvfxCurve {
        AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                x: 0.0,
                y: 0.0,
                z: value,
            }],
            ..Default::default()
        }
    }

    fn particle(count: f32) -> AvfxParticle {
        AvfxParticle {
            particle_type: Some(ParticleType::Polygon),
            data: AvfxParticleData::Polygon(AvfxParticleDataPolygon {
                count: curve(count),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn count_uses_client_truncation_and_stream_limit() {
        for (value, expected) in [
            (2.999, None),
            (3.0, Some(3)),
            (7.999, Some(7)),
            (16_383.0, Some(16_383)),
            (16_384.0, None),
            (f32::NAN, None),
        ] {
            assert_eq!(
                sample(&particle(value), 0.0, 1, 2.0).map(|polygon| polygon.count),
                expected
            );
        }
    }

    #[test]
    fn sample_retains_third_scale_and_rejects_nonfinite_values() {
        let polygon = sample(&particle(5.0), 0.0, 1, -3.0).unwrap();
        assert_eq!(polygon.scale_z, -3.0);
        assert_eq!(polygon.vertex_count(), 15);
        assert!(sample(&particle(5.0), 0.0, 1, f32::INFINITY).is_none());
    }
}
