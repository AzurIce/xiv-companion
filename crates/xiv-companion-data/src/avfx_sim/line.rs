use super::{
    AvfxParticle, AvfxParticleData, CurveAges, color_curve_seeded_at, curve_value_seeded_at,
};

pub(super) fn supports_simple(particle: &AvfxParticle) -> bool {
    let Some(simple) = particle
        .simple
        .as_ref()
        .filter(|_| particle.simple_anim_enable)
    else {
        return false;
    };
    particle.particle_type == Some(crate::avfx::ParticleType::Line)
        && simple.create_count as i16 > 0
        && simple.injection_position_type == 0
        && simple.injection_direction_type == 0
        && simple.injection_model_index == -1
        && simple.injection_vertex_bind_model_index == -1
        && !simple.bind_parent
}

/// Evaluated Line endpoints. Non-Smpl Line uses `length` along RBDT; the
/// supported Smpl path uses a world-space `endpoint_offset` reconstructed from
/// per-slot motion.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxLine {
    /// Cached local scale; ignored for world-space Smpl endpoint offsets.
    pub scale: [f32; 3],
    pub length: f32,
    /// Smpl Line's world-space displacement from the first endpoint to the
    /// second. Non-Smpl Line leaves this unset and uses `length` along RBDT.
    pub endpoint_offset: Option<[f32; 3]>,
    pub color_begin: [f32; 4],
    pub color_end: [f32; 4],
}

impl VfxLine {
    /// Convert a Smpl Line slot's position delta into the rendered endpoint
    /// displacement. The client leaves a zero delta collapsed, otherwise
    /// applies LLin first and LLax second to the delta length.
    pub fn simple_endpoint_offset(
        delta: [f32; 3],
        length_min: f32,
        length_max: f32,
    ) -> Option<[f32; 3]> {
        if !delta.into_iter().all(f32::is_finite)
            || !length_min.is_finite()
            || !length_max.is_finite()
        {
            return None;
        }
        let length = delta.iter().map(|value| value * value).sum::<f32>().sqrt();
        if length == 0.0 {
            return Some([0.0; 3]);
        }
        let rendered_length = length.max(length_min).min(length_max);
        Some(delta.map(|value| value * rendered_length / length))
    }
}

#[cfg(test)]
pub(super) fn sample_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
) -> Option<VfxLine> {
    sample_at_times(particle, ages, ages, seed, particle_color)
}

/// Ordinary Line evaluates Len at draw time, but ColB/ColE in normal +f0.
#[cfg(test)]
pub(super) fn sample_at_times(
    particle: &AvfxParticle,
    draw_ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
) -> Option<VfxLine> {
    sample_at_times_with_client(particle, draw_ages, cache_ages, seed, particle_color, None)
}

pub(super) fn sample_at_times_with_client(
    particle: &AvfxParticle,
    draw_ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    client: Option<super::particle_curves::VfxClientLineDrawValues>,
) -> Option<VfxLine> {
    let AvfxParticleData::Line(data) = &particle.data else {
        return None;
    };
    let length = client.map_or_else(
        || {
            curve_value_seeded_at(
                &data.length,
                &data.length_random,
                draw_ages,
                0.0,
                seed ^ 0x4c49_4e45,
            )
        },
        |value| value.length.unwrap_or(0.0),
    );
    let color_begin = client.map_or_else(
        || color_curve_seeded_at(&data.color_begin, cache_ages, seed ^ 0x4342),
        |value| value.colors[0],
    );
    let color_end = client.map_or_else(
        || color_curve_seeded_at(&data.color_end, cache_ages, seed ^ 0x4345),
        |value| value.colors[1],
    );
    let result = VfxLine {
        scale: [1.0; 3],
        length,
        endpoint_offset: None,
        color_begin: std::array::from_fn(|axis| particle_color[axis] * color_begin[axis]),
        color_end: std::array::from_fn(|axis| particle_color[axis] * color_end[axis]),
    };
    (result.length.is_finite()
        && result.color_begin.iter().all(|value| value.is_finite())
        && result.color_end.iter().all(|value| value.is_finite()))
    .then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{
        AvfxColorCurve, AvfxCurve, AvfxCurveKey, AvfxParticleDataLine, BEHAVIOR_ADD,
        BEHAVIOR_REPEAT, ParticleType,
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

    fn color(value: f32, post_behavior: u32) -> AvfxColorCurve {
        AvfxColorCurve {
            rgb: Some(AvfxCurve {
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
                        x: value,
                        y: value * 2.0,
                        z: value * 3.0,
                    },
                ],
                post_behavior,
                ..Default::default()
            }),
            alpha: Some(ramp(post_behavior)),
            ..Default::default()
        }
    }

    fn particle() -> AvfxParticle {
        AvfxParticle {
            particle_type: Some(ParticleType::Line),
            data: AvfxParticleData::Line(AvfxParticleDataLine {
                length: ramp(BEHAVIOR_ADD),
                color_begin: color(1.0, BEHAVIOR_ADD),
                color_end: color(2.0, BEHAVIOR_REPEAT),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn line_keeps_dual_clocks_and_multiplies_endpoint_colors() {
        let sampled = sample_at(
            &particle(),
            CurveAges {
                local: 5.0,
                total: 25.0,
            },
            7,
            [2.0, 3.0, 4.0, 0.5],
        )
        .unwrap();
        assert_eq!(sampled.length, 25.0);
        assert_eq!(sampled.color_begin, [1.0, 3.0, 6.0, 12.5]);
        assert_eq!(sampled.color_end, [2.0, 6.0, 12.0, 2.5]);
    }

    #[test]
    fn line_rejects_nonfinite_dimensions_and_colors() {
        let ages = CurveAges {
            local: 0.0,
            total: 0.0,
        };
        let mut particle = particle();
        if let AvfxParticleData::Line(data) = &mut particle.data {
            data.length.keys[0].z = f32::INFINITY;
        }
        assert!(sample_at(&particle, ages, 1, [1.0; 4]).is_none());
        if let AvfxParticleData::Line(data) = &mut particle.data {
            data.length.keys[0].z = 0.0;
        }
        assert!(sample_at(&particle, ages, 1, [1.0, f32::NAN, 1.0, 1.0]).is_none());
    }

    #[test]
    fn simple_line_clamps_nonzero_delta_min_then_max() {
        assert_eq!(
            VfxLine::simple_endpoint_offset([0.0, 0.0, 0.1], 0.25, 0.5),
            Some([0.0, 0.0, 0.25])
        );
        assert_eq!(
            VfxLine::simple_endpoint_offset([0.3, 0.4, 0.0], 0.25, 0.75),
            Some([0.3, 0.4, 0.0])
        );
        assert_eq!(
            VfxLine::simple_endpoint_offset([0.0, -0.8, 0.0], 0.25, 0.5),
            Some([0.0, -0.5, 0.0])
        );
        assert_eq!(
            VfxLine::simple_endpoint_offset([1.0, 0.0, 0.0], 2.0, 1.0),
            Some([1.0, 0.0, 0.0])
        );
    }

    #[test]
    fn simple_line_keeps_zero_delta_collapsed_and_rejects_nonfinite_input() {
        assert_eq!(
            VfxLine::simple_endpoint_offset([0.0; 3], 0.25, 0.5),
            Some([0.0; 3])
        );
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                VfxLine::simple_endpoint_offset([value, 0.0, 0.0], 0.25, 0.5),
                None
            );
            assert_eq!(
                VfxLine::simple_endpoint_offset([1.0, 0.0, 0.0], value, 0.5),
                None
            );
            assert_eq!(
                VfxLine::simple_endpoint_offset([1.0, 0.0, 0.0], 0.25, value),
                None
            );
        }
    }

    #[test]
    fn line_endpoint_random_colors_are_seeded_independently() {
        let mut particle = particle();
        let AvfxParticleData::Line(data) = &mut particle.data else {
            unreachable!()
        };
        for color in [&mut data.color_begin, &mut data.color_end] {
            color.random[0] = Some(AvfxCurve {
                keys: vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                }],
                random_type: 3,
                ..Default::default()
            });
        }
        let ages = CurveAges {
            local: 3.0,
            total: 3.0,
        };
        let first = sample_at(&particle, ages, 9, [1.0; 4]).unwrap();
        assert_eq!(first, sample_at(&particle, ages, 9, [1.0; 4]).unwrap());
        assert_ne!(first, sample_at(&particle, ages, 10, [1.0; 4]).unwrap());
        assert_ne!(first.color_begin[0], first.color_end[0]);
    }
}
