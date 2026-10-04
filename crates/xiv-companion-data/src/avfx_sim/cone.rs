use super::{CurveAges, EmitterAnimation, SplitMix64, quat_from_euler, quat_rotate, scalar_random};
use crate::avfx::{AvfxCurve, ConeEmitterData};

pub(super) fn sample(
    animation: &EmitterAnimation,
    data: &ConeEmitterData,
    frame: f32,
    rng: &mut SplitMix64,
) -> ([f32; 3], [f32; 3], f32) {
    let ages = animation.clock.ages(frame);
    // First coefficients belong to the emitter, not to individual births.
    // The seed source remains the preview's deterministic random sequence.
    let mut first = SplitMix64::seeded(animation.random.clone().next_u64(), 0, 0, 0x434F_4E45);
    let first: [u16; 7] = std::array::from_fn(|_| first.next_u64() as u16);
    let mut value = |main: &AvfxCurve, random: &AvfxCurve, index: usize| {
        curve_value_at(main, random, ages, first[index], rng)
    };
    let inner = value(&data.inner_size, &data.inner_size_random, 3);
    let outer = value(&data.outer_size, &data.outer_size_random, 4);
    let speed = value(&data.injection_speed, &data.injection_speed_random, 5);
    let spread = value(&data.injection_angle, &data.injection_angle_random, 6);
    let mut angles = [0.0; 3];
    for axis in (0..3).rev() {
        angles[axis] = value(
            &data.rotation.angles[axis],
            &data.rotation.angles_random[axis],
            axis,
        );
    }

    // 0x1403d1980 rotates +Z about a random axis in XY, then applies Data.ROT.
    // The same direction sets the shell birth point and the injection velocity.
    let azimuth = unit(rng) * std::f32::consts::TAU - std::f32::consts::PI;
    let angle = (spread - -spread) * unit(rng) - spread;
    let (sin_azimuth, cos_azimuth) = azimuth.sin_cos();
    let (sin_angle, cos_angle) = angle.sin_cos();
    let direction = quat_rotate(
        quat_from_euler(data.rotation.order, angles),
        [sin_azimuth * sin_angle, -cos_azimuth * sin_angle, cos_angle],
    );
    let distance = (outer - inner) * unit(rng) + inner;
    (direction.map(|v| v * distance), direction, speed)
}

fn unit(rng: &mut SplitMix64) -> f32 {
    f32::from(rng.next_u64() as u16) / 65535.0
}

#[cfg(test)]
pub(super) fn curve_value(
    main: &AvfxCurve,
    random: &AvfxCurve,
    age: f32,
    first: u16,
    rng: &mut SplitMix64,
) -> f32 {
    curve_value_at(
        main,
        random,
        CurveAges {
            local: age,
            total: age,
        },
        first,
        rng,
    )
}

pub(super) fn curve_value_at(
    main: &AvfxCurve,
    random: &AvfxCurve,
    ages: CurveAges,
    first: u16,
    rng: &mut SplitMix64,
) -> f32 {
    let base = main.value_at(ages.local, ages.total, 0.0);
    if random.keys.is_empty() || random.random_type & 7 >= 6 {
        return base;
    }
    let draw = if matches!(random.random_type & 7, 3..=5) {
        rng.next_u64() as u16
    } else {
        first
    };
    scalar_random::pair(
        random.random_type,
        (!main.keys.is_empty()).then_some(base),
        Some(random.value_at(ages.local, ages.total, 0.0)),
        draw,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::AvfxCurveKey;

    #[test]
    fn cone_scalar_dispatch_preserves_random_only_and_disabled_signed_zero() {
        let mut rng = SplitMix64::seeded(1, 2, 3, 4);
        let curve = |z| AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                z,
                x: 0.0,
                y: 0.0,
                interpolation: 1,
            }],
            ..Default::default()
        };
        assert_eq!(
            curve_value(&AvfxCurve::default(), &curve(0.0), 2.0, 0, &mut rng).to_bits(),
            0x8000_0000
        );
        assert_eq!(
            curve_value(&curve(-0.0), &AvfxCurve::default(), 2.0, 0, &mut rng).to_bits(),
            0x8000_0000
        );
        let disabled = AvfxCurve {
            random_type: 6,
            ..curve(f32::NAN)
        };
        assert_eq!(
            curve_value(&curve(-0.0), &disabled, 2.0, 0, &mut rng).to_bits(),
            0x8000_0000
        );
    }

    #[test]
    fn first_coefficient_scales_interpolated_amplitude_and_keeps_percent_quantization() {
        let mut random = AvfxCurve {
            keys: [(0, 2.0), (4, 6.0)]
                .map(|(time, z)| AvfxCurveKey {
                    time,
                    z,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                })
                .to_vec(),
            ..Default::default()
        };
        let mut rng = SplitMix64::seeded(1, 2, 3, 4);
        for (mode, first, coefficient) in [(0, 49151, 0.49), (1, 32767, 0.49), (2, 32768, -0.49)] {
            random.random_type = mode;
            for (age, amplitude) in [(0.0, 2.0), (2.0, 4.0), (4.0, 6.0), (0.0, 2.0)] {
                let actual = curve_value(&AvfxCurve::default(), &random, age, first, &mut rng);
                assert!((actual - amplitude * coefficient).abs() < 1e-6);
            }
        }
    }
}
