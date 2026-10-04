use super::{
    CurveAges, EmitterAnimation, EmitterBase, ShapeBinding, SplitMix64, cone, quat_from_euler,
    quat_rotate,
};
use crate::avfx::CylinderModelEmitterData;

pub(super) fn sample(
    animation: &EmitterAnimation,
    data: &CylinderModelEmitterData,
    frame: f32,
    create_index: u64,
    rng: &mut SplitMix64,
) -> Option<([f32; 3], [f32; 3], f32, ShapeBinding)> {
    let method = data.generate_method as u8;
    let x = i32::from(data.divide_x as u8);
    let y = i32::from(data.divide_y as u8);
    // The zero-X client branch leaves its direction output unwritten.
    // Skip these births rather than inventing a direction for the preview.
    if method > 7 || x == 0 {
        return None;
    }
    let ages = animation.clock.ages(frame);
    let length = data.length.value_at(ages.local, ages.total, 0.0);
    let radius = data.radius.value_at(ages.local, ages.total, 0.0);
    let first = first_draws(animation);
    let speed = cone::curve_value_at(
        &data.injection_speed,
        &data.injection_speed_random,
        ages,
        first[3],
        rng,
    );
    let rotation = rotation(data, ages, &first, rng);
    let count = x * (y + 1);
    // The client increments one emitter-owned counter; the preview still
    // supplies the emitter instance's shared shape ordinal for ordered births.
    let index = if method & 1 != 0 {
        (create_index % count as u64) as i32
    } else {
        count.wrapping_mul(i32::from(rng.next_u64() as u16)) / 65536
    };
    let (point, normal) = vertex(index, x, y, length, radius);
    if point.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let on_vertex = method & 2 != 0;
    let direction = if !on_vertex {
        point
    } else if animation.emitter.any_direction {
        animation.model_injection_direction(frame, rng)
    } else {
        normal
    };
    Some((
        if on_vertex {
            quat_rotate(rotation, point)
        } else {
            [0.0; 3]
        },
        quat_rotate(rotation, direction),
        speed,
        ShapeBinding::CylinderModel {
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
    let super::AvfxEmitterData::CylinderModel(data) = animation.emitter.data.as_ref()? else {
        return None;
    };
    let x = i32::from(data.divide_x as u8);
    let y = i32::from(data.divide_y as u8);
    if x == 0 {
        return Some(world.position);
    }
    let ages = animation.clock.ages(frame);
    let length = data.length.value_at(ages.local, ages.total, 0.0);
    let radius = data.radius.value_at(ages.local, ages.total, 0.0);
    // Bound Always draws remain a per-child integer-frame approximation.
    let mut rng = SplitMix64::seeded(random_seed, frame.floor() as u64, 0, 0x4249_4E44);
    let rotation = rotation(data, ages, &first_draws(animation), &mut rng);
    let (point, _) = vertex(i32::from(index), x, y, length, radius);
    Some(world.transform_point(quat_rotate(rotation, point)))
}

fn first_draws(animation: &EmitterAnimation) -> [u16; 4] {
    let mut rng = SplitMix64::seeded(animation.random.clone().next_u64(), 0, 0, 0x4359_4C4D);
    std::array::from_fn(|_| rng.next_u64() as u16)
}

fn rotation(
    data: &CylinderModelEmitterData,
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

fn vertex(index: i32, x: i32, y: i32, length: f32, radius: f32) -> ([f32; 3], [f32; 3]) {
    let angle = (index % x) as f32 * std::f32::consts::TAU / x as f32;
    let (sin, cos) = angle.sin_cos();
    // Retain the client's divide-then-multiply, including radius=0, and
    // signed index arithmetic: ToVertex's -1 is not a center sentinel here.
    let height = if y == 0 {
        0.0
    } else {
        ((index / x) as f32 * length / y as f32 - length * 0.5) / radius
    };
    (
        [sin * radius, height * radius, cos * radius],
        [sin, 0.0, cos],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_vertical_divisions_bypass_radius_division_but_other_rows_do_not() {
        assert_eq!(vertex(0, 4, 0, 4.0, 0.0), ([0.0; 3], [0.0, 0.0, 1.0]));
        for index in [0, 4, 8, -1] {
            assert!(!vertex(index, 4, 2, 4.0, 0.0).0[1].is_finite());
        }
        assert_eq!(vertex(-1, 1, 2, 4.0, 2.0).0[1], -4.0);
        assert_eq!(vertex(-1, 4, 2, 4.0, 2.0).0[1], -2.0);
        let index = 32768_u16 as i16;
        let point = vertex(i32::from(index), 255, 255, 510.0, 2.0).0;
        assert_eq!(point[1], -511.0);
    }
}
