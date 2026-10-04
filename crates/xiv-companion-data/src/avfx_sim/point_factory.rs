use crate::avfx::AvfxBinder;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PointTargetBirth {
    pub target_index: i32,
    pub delay: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LinearTargetBirth {
    pub target_indices: [i32; 2],
    pub delay: f32,
}

/// Scheduler/Item Linear, LinearAdjust and Spline factories 3bb490/3bd3f0.
/// Both target endpoints expand
/// start-major Cartesian pairs, omitting the same ordinal. Only start GenD
/// contributes delay, multiplied by the goal ordinal in the two-list branch.
/// Empty target lists never fall back to the caster; two caster ends ignore GenD.
pub(super) fn dual_target_births(binder: &AvfxBinder, count: i32) -> Vec<LinearTargetBirth> {
    let (Some(start), Some(goal)) = (&binder.properties_start, &binder.properties_goal) else {
        return Vec::new();
    };
    let delay = start.generate_delay as i16 as i32;
    let birth = |i, j, ordinal| LinearTargetBirth {
        target_indices: [i, j],
        delay: delay.wrapping_mul(ordinal) as f32,
    };
    match (
        start.bind_point_type as u8 & 3,
        goal.bind_point_type as u8 & 3,
    ) {
        (0, 0) => vec![birth(-1, -1, 0)],
        (0, 1) => (0..count).map(|j| birth(-1, j, j)).collect(),
        (1, 0) => (0..count).map(|i| birth(i, -1, i)).collect(),
        (1, 1) => (0..count)
            .flat_map(|i| {
                (0..count)
                    .filter(move |&j| i != j)
                    .map(move |j| birth(i, j, j))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Scheduler Point factory 3bbd2e: bind mode is two bits. Only target mode
/// asks for count; zero/negative count takes the caster single-object branch.
/// GenD is sign-extended before integer multiplication and conversion to f32.
pub(super) fn births(binder: &AvfxBinder, target_count: i32) -> Vec<PointTargetBirth> {
    let Some(properties) = &binder.properties_start else {
        return Vec::new();
    };
    let delay = properties.generate_delay as i16 as i32;
    match properties.bind_point_type as u8 & 3 {
        0 => vec![PointTargetBirth {
            target_index: -1,
            delay: delay as f32,
        }],
        1 if target_count <= 0 => vec![PointTargetBirth {
            target_index: -1,
            delay: delay as f32,
        }],
        1 => (0..target_count)
            .map(|index| PointTargetBirth {
                target_index: index,
                delay: delay.wrapping_mul(index) as f32,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Timeline Item Point factory 3bdc2b has no caster fallback for an empty
/// target list, unlike the Scheduler factory. Other branches use the same
/// signed integer delay and target ordinals.
pub(super) fn item_births(binder: &AvfxBinder, target_count: i32) -> Vec<PointTargetBirth> {
    if binder
        .properties_start
        .as_ref()
        .is_some_and(|p| p.bind_point_type as u8 & 3 == 1)
        && target_count <= 0
    {
        Vec::new()
    } else {
        births(binder, target_count)
    }
}
