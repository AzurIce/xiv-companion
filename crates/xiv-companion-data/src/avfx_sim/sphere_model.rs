use super::{
    CurveAges, EmitterAnimation, EmitterBase, ShapeBinding, SplitMix64, cone, quat_from_euler,
    quat_rotate,
};
use crate::avfx::SphereModelEmitterData;

pub(super) fn sample(
    animation: &EmitterAnimation,
    data: &SphereModelEmitterData,
    frame: f32,
    create_index: u64,
    rng: &mut SplitMix64,
) -> Option<([f32; 3], [f32; 3], f32, ShapeBinding)> {
    let method = data.generate_method as u8;
    let x = i32::from(data.divide_x as i8);
    let y = i32::from(data.divide_y as i8);
    // The nonpositive-X client branch leaves direction unwritten; zero Y
    // produces nonfinite angles. Neither can supply a finite preview birth.
    if method > 7 || x <= 0 || y == 0 {
        return None;
    }
    let ages = animation.clock.ages(frame);
    let on_vertex = method & 2 != 0;
    let radius = if on_vertex {
        data.radius.value_at(ages.local, ages.total, 0.0)
    } else {
        0.0
    };
    let first = first_draws(animation);
    let speed = cone::curve_value_at(
        &data.injection_speed,
        &data.injection_speed_random,
        ages,
        first[3],
        rng,
    );
    let rotation = rotation(data, ages, &first, rng);
    let draw = if method & 1 == 0 {
        rng.next_u64() as u16
    } else {
        0
    };
    let index = vertex_index(method, x, y, create_index, draw);
    let normal = normal(index, x, y);
    let direction = if on_vertex && animation.emitter.any_direction {
        animation.model_injection_direction(frame, rng)
    } else {
        normal
    };
    Some((
        quat_rotate(rotation, normal).map(|v| v * radius),
        quat_rotate(rotation, direction),
        speed,
        ShapeBinding::SphereModel {
            index: if on_vertex { index as i16 } else { -1 },
            random_seed: rng.clone().next_u64(),
        },
    ))
}

pub(super) fn position(
    animation: &EmitterAnimation,
    frame: f32,
    world: EmitterBase,
    index: i16,
    random_seed: u64,
) -> Option<[f32; 3]> {
    let super::AvfxEmitterData::SphereModel(data) = animation.emitter.data.as_ref()? else {
        return None;
    };
    let x = i32::from(data.divide_x as i8);
    let y = i32::from(data.divide_y as i8);
    if index < 0 || x <= 0 {
        return Some(world.position);
    }
    let ages = animation.clock.ages(frame);
    let radius = data.radius.value_at(ages.local, ages.total, 0.0);
    // Bound Always draws remain a per-child integer-frame approximation.
    let mut rng = SplitMix64::seeded(random_seed, frame.floor() as u64, 0, 0x4249_4E44);
    let rotation = rotation(data, ages, &first_draws(animation), &mut rng);
    let point = quat_rotate(rotation, normal(i32::from(index), x, y));
    Some(world.transform_point(point.map(|v| v * radius)))
}

fn first_draws(animation: &EmitterAnimation) -> [u16; 4] {
    let mut rng = SplitMix64::seeded(animation.random.clone().next_u64(), 0, 0, 0x5350_484D);
    std::array::from_fn(|_| rng.next_u64() as u16)
}

fn rotation(
    data: &SphereModelEmitterData,
    ages: CurveAges,
    first: &[u16; 4],
    rng: &mut SplitMix64,
) -> [f32; 4] {
    let mut angles = [0.0; 3];
    for axis in (0..3).rev() {
        angles[axis] = cone::curve_value_at(
            &data.rotation.angles[axis],
            &data.rotation.angles_random[axis],
            ages,
            first[axis],
            rng,
        );
    }
    quat_from_euler(data.rotation.order, angles)
}

fn vertex_index(method: u8, x: i32, y: i32, ordinal: u64, draw: u16) -> i32 {
    match method {
        0 | 2 => x - 1 + (x * (y - 1) + 2) * i32::from(draw) / 65536,
        4 => x + x * (y - 1) * i32::from(draw) / 65536,
        // This client range includes the south pole, unlike method 4.
        6 => x + (x * (y - 1) + 1) * i32::from(draw) / 65536,
        // Ordered modes receive the emitter instance's shared shape ordinal.
        1 | 3 => {
            let count = if y > 0 { x * (y - 1) + 2 } else { 2 };
            let index = (ordinal % count as u64) as i32;
            if index == 0 { 0 } else { x + index - 1 }
        }
        5 | 7 => {
            let count = if y > 1 { x * (y - 1) } else { 1 };
            x + (ordinal % count as u64) as i32
        }
        _ => unreachable!(),
    }
}

fn normal(index: i32, x: i32, y: i32) -> [f32; 3] {
    let latitude = (index / x) as f32 * std::f32::consts::PI / y as f32;
    let azimuth = (index % x) as f32 * std::f32::consts::TAU / x as f32;
    let (sin_lat, cos_lat) = latitude.sin_cos();
    let (sin_az, cos_az) = azimuth.sin_cos();
    [sin_lat * sin_az, cos_lat, sin_lat * cos_az]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_endpoints_and_signed_vertical_divisions() {
        for (method, ends) in [(0, [3, 8]), (2, [3, 8]), (4, [4, 7]), (6, [4, 8])] {
            assert_eq!(vertex_index(method, 4, 2, 0, 0), ends[0]);
            assert_eq!(vertex_index(method, 4, 2, 0, u16::MAX), ends[1]);
        }
        assert_eq!(vertex_index(0, 127, 127, 0, u16::MAX), 16129);
        assert_eq!(vertex_index(4, 4, -2, 0, u16::MAX), -7);
        for method in [1, 3] {
            assert_eq!(
                (0..6)
                    .map(|i| vertex_index(method, 4, -2, i, 0))
                    .collect::<Vec<_>>(),
                [0, 4, 0, 4, 0, 4]
            );
        }
        for method in 4..8 {
            assert_eq!(vertex_index(method, 4, 1, 10, u16::MAX), 4);
        }
        let point = normal(4, 4, -2);
        assert!(point[0].abs() < 1e-6 && point[1].abs() < 1e-6);
        assert!((point[2] + 1.0).abs() < 1e-6);
    }
}
