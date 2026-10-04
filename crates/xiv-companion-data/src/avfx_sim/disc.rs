use super::{
    AvfxParticle, AvfxParticleData, CurveAges, color_curve_seeded_at, curve_random_value_at,
};
use crate::avfx::AvfxCurve;

/// Evaluated Disc data. Counts include both endpoints of each grid axis.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDisc {
    /// PrtC, PCnU, PCnV after the client's byte conversion and limits.
    pub counts: [u8; 3],
    pub angle: f32,
    pub radius: [f32; 2],
    /// Half-width at the two angular endpoints.
    pub width: [f32; 2],
    pub height_inner: [f32; 2],
    pub height_outer: [f32; 2],
    pub color_inner: [f32; 4],
    pub color_outer: [f32; 4],
    pub point_interval_factor: f32,
    /// SS / 100. Values below one enable width compensation.
    pub scaling_scale: f32,
    /// The other two local scale axes are retained in VfxQuad.size.
    pub scale_z: f32,
}

impl VfxDisc {
    pub fn vertex_count(&self) -> u32 {
        u32::from(self.counts[0])
            * u32::from(self.counts[1].saturating_sub(1))
            * u32::from(self.counts[2].saturating_sub(1))
            * 6
    }
}

#[cfg(test)]
pub(super) fn sample(
    particle: &AvfxParticle,
    age: f32,
    seed: u64,
    scale_z: f32,
) -> Option<VfxDisc> {
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
) -> Option<VfxDisc> {
    sample_at_times(particle, ages, ages, seed, scale_z)
}

/// Geometry is evaluated by +108; CEI/CEO are cached by normal +f0.
#[cfg(test)]
pub(super) fn sample_at_times(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    scale_z: f32,
) -> Option<VfxDisc> {
    sample_at_times_with_client(particle, ages, cache_ages, seed, scale_z, None)
}

/// Compiled pair order at Data +8,+20,...,+c8: Ang, HBI, HEI, HBO, HEO,
/// WB, WE, RB, RE (3a2400 parser, 3e88f0 First and 3e9de0 Draw).
pub(super) fn scalar_curves(
    data: &crate::avfx::AvfxParticleDataDisc,
) -> [(&AvfxCurve, &AvfxCurve); 9] {
    [
        (&data.angle, &data.angle_random),
        (&data.height_begin_inner, &data.height_begin_inner_random),
        (&data.height_end_inner, &data.height_end_inner_random),
        (&data.height_begin_outer, &data.height_begin_outer_random),
        (&data.height_end_outer, &data.height_end_outer_random),
        (&data.width_begin, &data.width_begin_random),
        (&data.width_end, &data.width_end_random),
        (&data.radius_begin, &data.radius_begin_random),
        (&data.radius_end, &data.radius_end_random),
    ]
}

pub(super) fn sample_at_times_with_client(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    scale_z: f32,
    values: Option<super::particle_curves::VfxClientDiscDrawValues>,
) -> Option<VfxDisc> {
    let data = match &particle.data {
        AvfxParticleData::Disc(data) => data,
        _ => return None,
    };
    let parts = (data.parts_count as u8).min(16);
    let count_u = (data.parts_count_u as u8).min(16);
    let count_v = (data.parts_count_v as u8).min(128);
    // Degenerate grids cannot form triangles. Avoid the client's divisions by zero.
    if parts == 0 || count_u < 2 || count_v < 2 {
        return None;
    }
    let count_v = u16::from(count_v).min(1024 / u16::from(count_u)) as u8;
    let curve = |base: &AvfxCurve, random: &AvfxCurve, salt| {
        base.value_at(ages.local, ages.total, 0.0)
            + curve_random_value_at(random, ages, seed ^ salt)
    };
    let scalars = values.map(|values| values.scalars).unwrap_or_else(|| {
        let salts = [
            0xD150, 0xD155, 0xD156, 0xD157, 0xD158, 0xD153, 0xD154, 0xD151, 0xD152,
        ];
        let curves = scalar_curves(data);
        std::array::from_fn(|i| curve(curves[i].0, curves[i].1, salts[i]))
    });
    let colors = values.map(|values| values.colors).unwrap_or_else(|| {
        [
            color_curve_seeded_at(&data.color_edge_inner, cache_ages, seed ^ 0xD159),
            color_curve_seeded_at(&data.color_edge_outer, cache_ages, seed ^ 0xD15A),
        ]
    });
    let result = VfxDisc {
        counts: [parts, count_u, count_v],
        angle: scalars[0],
        radius: [scalars[7], scalars[8]],
        width: [scalars[5], scalars[6]],
        height_inner: [scalars[1], scalars[2]],
        height_outer: [scalars[3], scalars[4]],
        color_inner: colors[0],
        color_outer: colors[1],
        point_interval_factor: data.point_interval_factor_v,
        scaling_scale: data.scaling_scale as f32 / 100.0,
        scale_z,
    };
    result
        .radius
        .iter()
        .chain(&result.width)
        .chain(&result.height_inner)
        .chain(&result.height_outer)
        .chain(&result.color_inner)
        .chain(&result.color_outer)
        .chain([
            &result.angle,
            &result.point_interval_factor,
            &result.scaling_scale,
            &result.scale_z,
        ])
        .all(|value| value.is_finite())
        .then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey, AvfxParticleDataDisc, ParticleType};

    fn particle() -> AvfxParticle {
        AvfxParticle {
            particle_type: Some(ParticleType::Disc),
            data: AvfxParticleData::Disc(AvfxParticleDataDisc {
                parts_count: 1,
                parts_count_u: 2,
                parts_count_v: 2,
                scaling_scale: 100,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn counts_use_client_bytes_caps_and_joint_vertex_limit() {
        let mut particle = particle();
        for (raw, expected) in [
            ([1, 2, 64], Some([1, 2, 64])),
            ([257, 258, 320], Some([1, 2, 64])),
            ([255, 255, 255], Some([16, 16, 64])),
            ([1, 8, 255], Some([1, 8, 128])),
            ([1, 3, 1], None),
            ([0, 2, 64], None),
            ([1, 256, 64], None),
        ] {
            let AvfxParticleData::Disc(data) = &mut particle.data else {
                unreachable!()
            };
            [data.parts_count, data.parts_count_u, data.parts_count_v] = raw;
            assert_eq!(sample(&particle, 0.0, 1, 1.0).map(|d| d.counts), expected);
        }
    }

    #[test]
    fn missing_disc_fields_use_client_defaults_and_float_scalars_keep_bits() {
        let mut particle = particle();
        let sampled = sample(&particle, 0.0, 1, -2.0).unwrap();
        assert_eq!(sampled.counts, [1, 2, 2]);
        assert_eq!(sampled.scaling_scale, 1.0);
        assert_eq!(sampled.radius, [0.0; 2]);
        assert_eq!(sampled.scale_z, -2.0);
        assert_eq!(sampled.vertex_count(), 6);
        let AvfxParticleData::Disc(data) = &mut particle.data else {
            unreachable!()
        };
        data.point_interval_factor_v = 0.75;
        data.scaling_scale = 25;
        let sampled = sample(&particle, 0.0, 1, 1.0).unwrap();
        assert_eq!(sampled.point_interval_factor, 0.75);
        assert_eq!(sampled.scaling_scale, 0.25);
    }

    #[test]
    fn disc_geometry_curves_animate_and_nonfinite_data_is_not_submitted() {
        let mut particle = particle();
        let linear = AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 1.0,
                    y: 1.0,
                    z: 0.2,
                },
                AvfxCurveKey {
                    time: 10,
                    interpolation: 1,
                    x: 1.0,
                    y: 1.0,
                    z: 0.6,
                },
            ],
            ..Default::default()
        };
        let AvfxParticleData::Disc(data) = &mut particle.data else {
            unreachable!()
        };
        data.radius_begin = linear.clone();
        data.radius_end = linear.clone();
        data.width_begin = linear.clone();
        data.width_end = linear.clone();
        data.height_begin_inner = linear.clone();
        data.height_end_inner = linear.clone();
        data.height_begin_outer = linear.clone();
        data.height_end_outer = linear.clone();
        data.angle = linear;
        let sampled = sample(&particle, 5.0, 1, 1.0).unwrap();
        for value in sampled
            .radius
            .into_iter()
            .chain(sampled.width)
            .chain(sampled.height_inner)
            .chain(sampled.height_outer)
            .chain([sampled.angle])
        {
            assert!((value - 0.4).abs() < 1e-6);
        }
        let AvfxParticleData::Disc(data) = &mut particle.data else {
            unreachable!()
        };
        data.point_interval_factor_v = f32::INFINITY;
        assert!(sample(&particle, 0.0, 1, 1.0).is_none());
    }
}
