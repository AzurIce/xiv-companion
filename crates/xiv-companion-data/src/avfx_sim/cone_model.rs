use super::{
    CurveAges, EmitterAnimation, EmitterBase, ShapeBinding, SplitMix64, cone, quat_from_euler,
    quat_rotate,
};
use crate::avfx::ConeModelEmitterData;

pub(super) fn sample(
    animation: &EmitterAnimation,
    data: &ConeModelEmitterData,
    frame: f32,
    create_index: u64,
    rng: &mut SplitMix64,
) -> Option<([f32; 3], [f32; 3], f32, ShapeBinding)> {
    let method = data.generate_method as u8;
    let x = i32::from(data.divide_x as u8);
    let y = i32::from(data.divide_y as u8);
    // Degenerate divisions cannot provide a finite direction. Skipping them is
    // a preview guard, not a claim about the client's uninitialized outputs.
    if method > 7 || x == 0 || y == 0 {
        return None;
    }
    let on_vertex = method & 2 != 0;
    let ages = animation.clock.ages(frame);
    let first = first_draws(animation);
    let spread = cone::curve_value_at(
        &data.injection_angle,
        &data.injection_angle_random,
        ages,
        first[4],
        rng,
    );
    let radius = if on_vertex {
        cone::curve_value_at(&data.radius, &data.radius_random, ages, first[3], rng)
    } else {
        0.0
    };
    let speed = cone::curve_value_at(
        &data.injection_speed,
        &data.injection_speed_random,
        ages,
        first[5],
        rng,
    );
    let rotation = rotation(data, ages, &first, rng);
    // The client includes one pole at x-1 and x vertices in each of y rings.
    // Ordered selection receives the emitter instance's shared shape ordinal.
    let count = x * y + 1;
    let ordinal = if method & 1 != 0 {
        (create_index % count as u64) as i32 + x - 1
    } else {
        count.wrapping_mul(i32::from(rng.next_u64() as u16)) / 65536 + x - 1
    };
    let direction = quat_rotate(rotation, vertex_direction(ordinal, x, y, spread));
    let offset = direction.map(|v| v * radius);
    let direction = if on_vertex && animation.emitter.any_direction {
        let mut angles = [0.0; 3];
        for axis in (0..3).rev() {
            angles[axis] = cone::curve_value_at(
                &animation.emitter.injection_angle[axis],
                &animation.emitter.injection_angle_random[axis],
                ages,
                first[6 + axis],
                rng,
            );
        }
        // 0x1403cf650 replaces the vector with R_XYZ(IAX/Y/Z) * +Z.
        quat_rotate(
            rotation,
            quat_rotate(quat_from_euler(0, angles), [0.0, 0.0, 1.0]),
        )
    } else {
        direction
    };
    Some((
        offset,
        direction,
        speed,
        ShapeBinding::ConeModel {
            index: if on_vertex { ordinal as i16 } else { -1 },
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
    let super::AvfxEmitterData::ConeModel(data) = animation.emitter.data.as_ref()? else {
        return None;
    };
    let x = i32::from(data.divide_x as u8);
    let y = i32::from(data.divide_y as u8);
    if x == 0 || y == 0 {
        return None;
    }
    let ages = animation.clock.ages(frame);
    let first = first_draws(animation);
    // Absolute sampling has no client callback history. Keep per-child Always
    // draws stable within an integer frame, as in the other bound-shape path.
    let mut rng = SplitMix64::seeded(random_seed, frame.floor() as u64, 0, 0x4249_4E44);
    let spread = cone::curve_value_at(
        &data.injection_angle,
        &data.injection_angle_random,
        ages,
        first[4],
        &mut rng,
    );
    let radius = cone::curve_value_at(&data.radius, &data.radius_random, ages, first[3], &mut rng);
    let rotation = rotation(data, ages, &first, &mut rng);
    // The getter consumes signed short indices, including ToVertex's -1.
    let vertex = quat_rotate(rotation, vertex_direction(i32::from(index), x, y, spread));
    Some(world.transform_point(vertex.map(|v| v * radius)))
}

fn first_draws(animation: &EmitterAnimation) -> [u16; 9] {
    let mut rng = SplitMix64::seeded(animation.random.clone().next_u64(), 0, 0, 0x434D_4F44);
    std::array::from_fn(|_| rng.next_u64() as u16)
}

fn rotation(
    data: &ConeModelEmitterData,
    ages: CurveAges,
    first: &[u16; 9],
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
    quat_from_euler(i32::from(data.rotation.order as u8), angles)
}

fn vertex_direction(ordinal: i32, x: i32, y: i32, spread: f32) -> [f32; 3] {
    let azimuth = (ordinal % x) as f32 * std::f32::consts::TAU / x as f32;
    let angle = (ordinal / x) as f32 * spread / y as f32;
    let (sin_azimuth, cos_azimuth) = azimuth.sin_cos();
    let (sin_angle, cos_angle) = angle.sin_cos();
    [sin_azimuth * sin_angle, -cos_azimuth * sin_angle, cos_angle]
}
