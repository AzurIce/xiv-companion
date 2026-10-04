const MIN_SEGMENT_LENGTH: f32 = 0.0001;
const MIN_WIDTH_DIRECTION_LENGTH: f32 = 0.00001;

use super::{
    AvfxParticle, AvfxParticleData, CurveAges, color_curve_seeded_at, curve_value_seeded_at,
};

pub(super) fn scalar_curves(
    data: &crate::avfx::AvfxParticleDataPolyline,
) -> [(&crate::avfx::AvfxCurve, &crate::avfx::AvfxCurve); 7] {
    [
        (&data.cf, &data.cf_random),
        (&data.width, &data.width_random),
        (&data.width_begin, &data.width_begin_random),
        (&data.width_center, &data.width_center_random),
        (&data.width_end, &data.width_end_random),
        (&data.length, &data.length_random),
        (&data.softness, &data.softness_random),
    ]
}

pub(super) fn color_curves(
    data: &crate::avfx::AvfxParticleDataPolyline,
) -> [&crate::avfx::AvfxColorCurve; 6] {
    [
        &data.color_begin,
        &data.color_center,
        &data.color_end,
        &data.color_edge_begin,
        &data.color_edge_center,
        &data.color_edge_end,
    ]
}

pub(super) fn has_center(data: &crate::avfx::AvfxParticleDataPolyline) -> bool {
    let center = i32::from(data.point_count_center as i8);
    center > 0 && center < i32::from(data.point_count as i8)
}

/// Evaluated Polyline data. Camera-facing vertex expansion remains a
/// render-time operation; history state must be produced incrementally.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxPolyline {
    pub point_count: u8,
    pub point_count_center: i32,
    pub uv_precision: i32,
    pub use_edge: bool,
    /// Instance axis selected by NBBA when `bNtB` disables camera facing.
    pub not_billboard_axis: Option<[f32; 3]>,
    /// Visit the evaluated point attributes from tail to head at render time.
    pub reverse_points: bool,
    /// Longitudinal UV range covered by the active point sequence.
    pub uv_span: f32,
    /// Evaluated center-line points. Only `point_count` entries are active.
    pub positions: [[f32; 3]; 64],
    /// Per-point UNORM8 multiplier for TD distortion (`PnED`).
    pub end_distortion: [u8; 64],
    /// Evaluated WdB/WdC/WdE after multiplying Wd and RBDT scale.
    pub widths: [f32; 3],
    /// Evaluated ColB/ColC/ColE, including the particle color.
    pub colors: [[f32; 4]; 3],
    /// Evaluated CoEB/CoEC/CoEE, including the particle color.
    pub edge_colors: [[f32; 4]; 3],
}

impl serde::Serialize for VfxPolyline {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Repr<'a> {
            point_count: u8,
            point_count_center: i32,
            uv_precision: i32,
            use_edge: bool,
            not_billboard_axis: Option<[f32; 3]>,
            reverse_points: bool,
            uv_span: f32,
            positions: &'a [[f32; 3]],
            end_distortion: &'a [u8],
            widths: [f32; 3],
            colors: [[f32; 4]; 3],
            edge_colors: [[f32; 4]; 3],
        }

        serde::Serialize::serialize(
            &Repr {
                point_count: self.point_count,
                point_count_center: self.point_count_center,
                uv_precision: self.uv_precision,
                use_edge: self.use_edge,
                not_billboard_axis: self.not_billboard_axis,
                reverse_points: self.reverse_points,
                uv_span: self.uv_span,
                positions: self.points(),
                end_distortion: self.point_end_distortion(),
                widths: self.widths,
                colors: self.colors,
                edge_colors: self.edge_colors,
            },
            serializer,
        )
    }
}

impl VfxPolyline {
    pub fn points(&self) -> &[[f32; 3]] {
        &self.positions[..usize::from(self.point_count)]
    }

    pub fn point_end_distortion(&self) -> &[u8] {
        &self.end_distortion[..usize::from(self.point_count)]
    }

    pub fn point_widths(&self) -> Vec<f32> {
        position_history_widths(
            1.0,
            self.widths[0],
            self.widths[1],
            self.widths[2],
            usize::from(self.point_count),
            self.point_count_center,
        )
    }

    pub fn point_colors(&self) -> Vec<[[f32; 4]; 3]> {
        position_history_edge_vertex_colors(
            self.colors,
            self.edge_colors,
            usize::from(self.point_count),
            self.point_count_center,
        )
    }
}

#[cfg(test)]
pub(super) fn sample_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    position: [f32; 3],
    basis: [[f32; 3]; 3],
) -> Option<VfxPolyline> {
    sample_at_times(particle, ages, ages, seed, particle_color, position, basis)
}

#[cfg(test)]
pub(super) fn sample_at_times(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    position: [f32; 3],
    basis: [[f32; 3]; 3],
) -> Option<VfxPolyline> {
    sample_at_times_with_client(
        particle,
        ages,
        cache_ages,
        seed,
        particle_color,
        position,
        basis,
        None,
    )
}

pub(super) fn sample_at_times_with_client(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    position: [f32; 3],
    basis: [[f32; 3]; 3],
    client: Option<super::particle_curves::VfxClientPolylineDrawValues>,
) -> Option<VfxPolyline> {
    let AvfxParticleData::Polyline(data) = &particle.data else {
        return None;
    };
    let point_count = u8::try_from(data.point_count)
        .ok()
        .filter(|count| (2..=64).contains(count))?;
    if data.create_line_type != 0
        || data.is_spline
        || data.bind_weapon
        || data.bind_weapon_type != 0
        || data.connect_target
        || data.tag_number != 0
        || particle.simple_anim_enable
        || !matches!(particle.rotation_direction_base, 0..=2)
        || (data.not_billboard && !matches!(data.not_billboard_base_axis_type, 0..=2))
    {
        return None;
    }

    let value = |curve, random, salt| curve_value_seeded_at(curve, random, ages, 0.0, seed ^ salt);
    let cf = client.map_or_else(
        || value(&data.cf, &data.cf_random, 0x504c_4346),
        |v| v.values[1],
    );
    let softness = value(&data.softness, &data.softness_random, 0x504c_5346);
    if !cf.is_finite()
        || !softness.is_finite()
        || softness != 0.0
        || (particle.rotation_direction_base == 2 && cf != 0.0)
    {
        return None;
    }
    let length = client.map_or_else(
        || value(&data.length, &data.length_random, 0x504c_4c45),
        |v| v.values[0],
    );
    let direction = basis[particle.rotation_direction_base as usize];
    let end = std::array::from_fn(|axis| position[axis] + direction[axis] * length);
    let points = linear_polyline_points(position, end, usize::from(point_count));
    evaluated_from_points(
        particle,
        data,
        ages,
        cache_ages,
        seed,
        particle_color,
        &points,
        basis,
        false,
        1.0,
        client,
    )
}

/// Evaluate the final attributes for an incrementally produced `LnCT=1`
/// center line. The caller owns history advancement, PnDs, CF, Sft and any
/// resampling; this function never reconstructs missing updates.
#[cfg(test)]
pub(super) fn sample_history_at(
    particle: &AvfxParticle,
    ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    points: &[[f32; 3]],
    basis: [[f32; 3]; 3],
    uv_span: f32,
) -> Option<VfxPolyline> {
    sample_history_at_times(
        particle,
        ages,
        ages,
        seed,
        particle_color,
        points,
        basis,
        uv_span,
    )
}

#[cfg(test)]
pub(super) fn sample_history_at_times(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    points: &[[f32; 3]],
    basis: [[f32; 3]; 3],
    uv_span: f32,
) -> Option<VfxPolyline> {
    sample_history_at_times_with_client(
        particle,
        ages,
        cache_ages,
        seed,
        particle_color,
        points,
        basis,
        uv_span,
        None,
    )
}

pub(super) fn sample_history_at_times_with_client(
    particle: &AvfxParticle,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    points: &[[f32; 3]],
    basis: [[f32; 3]; 3],
    uv_span: f32,
    client: Option<super::particle_curves::VfxClientPolylineDrawValues>,
) -> Option<VfxPolyline> {
    let AvfxParticleData::Polyline(data) = &particle.data else {
        return None;
    };
    let point_count = usize::try_from(data.point_count)
        .ok()
        .filter(|count| (2..=64).contains(count))?;
    if data.create_line_type != 1
        || points.len() != point_count
        || data.bind_weapon
        || data.bind_weapon_type != 0
        || data.connect_target
        || data.tag_number != 0
        || particle.simple_anim_enable
        || !matches!(particle.rotation_direction_base, 0..=2)
        || (data.not_billboard && !matches!(data.not_billboard_base_axis_type, 0..=2))
    {
        return None;
    }
    let reverse_points = data.is_spline && data.is_local;
    let uv_span = if reverse_points { -uv_span } else { uv_span };
    evaluated_from_points(
        particle,
        data,
        ages,
        cache_ages,
        seed,
        particle_color,
        points,
        basis,
        reverse_points,
        uv_span,
        client,
    )
}

fn evaluated_from_points(
    particle: &AvfxParticle,
    data: &crate::avfx::AvfxParticleDataPolyline,
    ages: CurveAges,
    cache_ages: CurveAges,
    seed: u64,
    particle_color: [f32; 4],
    points: &[[f32; 3]],
    basis: [[f32; 3]; 3],
    reverse_points: bool,
    uv_span: f32,
    client: Option<super::particle_curves::VfxClientPolylineDrawValues>,
) -> Option<VfxPolyline> {
    let point_count = u8::try_from(points.len())
        .ok()
        .filter(|count| (2..=64).contains(count))?;
    let value = |curve, random, salt| curve_value_seeded_at(curve, random, ages, 0.0, seed ^ salt);
    let base_width = client.map_or_else(
        || value(&data.width, &data.width_random, 0x504c_5744),
        |v| v.values[4],
    );
    let axis_lengths = basis.map(|axis| axis.into_iter().map(|v| v * v).sum::<f32>().sqrt());
    let base_width =
        scale_width_for_direction(base_width, axis_lengths, particle.rotation_direction_base);
    let widths = client
        .map_or_else(
            || {
                [
                    value(&data.width_begin, &data.width_begin_random, 0x5742),
                    value(&data.width_center, &data.width_center_random, 0x5743),
                    value(&data.width_end, &data.width_end_random, 0x5745),
                ]
            },
            |v| [v.values[5], v.values[6], v.values[7]],
        )
        .map(|width| width * base_width);
    let colors = [
        (&data.color_begin, 0x4342, 0),
        (&data.color_center, 0x4343, 1),
        (&data.color_end, 0x4345, 2),
    ]
    .map(|(curve, salt, i)| {
        let color = client.map_or_else(
            || color_curve_seeded_at(curve, cache_ages, seed ^ salt),
            |v| v.colors[i],
        );
        std::array::from_fn(|axis| color[axis] * particle_color[axis])
    });
    let edge_colors = [
        (&data.color_edge_begin, 0x4542, 3),
        (&data.color_edge_center, 0x4543, 4),
        (&data.color_edge_end, 0x4545, 5),
    ]
    .map(|(curve, salt, i)| {
        let color = client.map_or_else(
            || color_curve_seeded_at(curve, cache_ages, seed ^ salt),
            |v| v.colors[i],
        );
        std::array::from_fn(|axis| color[axis] * particle_color[axis])
    });
    let mut positions = [[0.0; 3]; 64];
    positions[..points.len()].copy_from_slice(&points);
    let end_distortion_values =
        position_history_end_distortion(points.len(), data.point_count_end_distortion);
    let mut end_distortion = [u8::MAX; 64];
    end_distortion[..points.len()].copy_from_slice(&end_distortion_values);
    let result = VfxPolyline {
        point_count,
        point_count_center: data.point_count_center,
        uv_precision: particle.uv_precision,
        use_edge: data.use_edge,
        not_billboard_axis: data
            .not_billboard
            .then(|| basis[data.not_billboard_base_axis_type as usize]),
        reverse_points,
        uv_span,
        positions,
        end_distortion,
        widths,
        colors,
        edge_colors,
    };
    (result
        .points()
        .iter()
        .flatten()
        .copied()
        .all(f32::is_finite)
        && result.uv_span.is_finite()
        && result.widths.into_iter().all(f32::is_finite)
        && result
            .colors
            .into_iter()
            .chain(result.edge_colors)
            .flatten()
            .all(f32::is_finite))
    .then_some(result)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HistoryPoint {
    pub position: [f32; 3],
    pub distance: f32,
}

/// Scale the base polyline width for the three axis-aligned RBDT modes.
/// Client function `0x1403f7720` uses the mean scale of the two axes
/// perpendicular to the selected direction. Other modes leave `Wd` alone.
pub(super) fn scale_width_for_direction(width: f32, scale: [f32; 3], rbdt: i32) -> f32 {
    let perpendicular_mean = match rbdt {
        0 => (scale[1] + scale[2]) * 0.5,
        1 => (scale[2] + scale[0]) * 0.5,
        2 => (scale[0] + scale[1]) * 0.5,
        _ => 1.0,
    };
    width * perpendicular_mean
}

/// Build the per-point width sequence used by both PositionHistory vertex
/// writers. The normal client path first multiplies `WdB`, `WdC`, and `WdE`
/// by the base `Wd`, then uses the same `PnCC` split as its color gradients.
pub(super) fn position_history_widths(
    base: f32,
    begin: f32,
    center: f32,
    end: f32,
    point_count: usize,
    point_count_center: i32,
) -> Vec<f32> {
    let begin = begin * base;
    let center = center * base;
    let end = end * base;
    let center_index = usize::try_from(point_count_center)
        .ok()
        .filter(|&index| index > 0 && index < point_count);

    (0..point_count)
        .map(|index| match center_index {
            Some(center_index) if index < center_index => {
                begin + (center - begin) * (index as f32 / center_index as f32)
            }
            Some(center_index) => {
                let denominator = point_count - center_index - 1;
                let t = if denominator == 0 {
                    0.0
                } else {
                    (index - center_index) as f32 / denominator as f32
                };
                center + (end - center) * t
            }
            None => {
                let denominator = point_count.saturating_sub(1);
                let t = if denominator == 0 {
                    0.0
                } else {
                    index as f32 / denominator as f32
                };
                begin + (end - begin) * t
            }
        })
        .collect()
}

/// Build the normalized direction multiplied by each point's width in the
/// PositionHistory writers. Billboard ribbons use the cross product of the
/// clamped neighboring tangent and the camera ray. `bNtB` instead supplies
/// one NBBA-selected instance axis, which the client normalizes once.
pub(super) fn position_history_width_directions(
    points: &[[f32; 3]],
    camera_origin: [f32; 3],
    not_billboard_axis: Option<[f32; 3]>,
) -> Vec<[f32; 3]> {
    if let Some(axis) = not_billboard_axis {
        let direction = normalize_unchecked(axis);
        return vec![direction; points.len()];
    }

    points
        .iter()
        .enumerate()
        .map(|(index, &point)| {
            let previous = points[index.saturating_sub(1)];
            let next = points[(index + 1).min(points.len() - 1)];
            let tangent = sub(previous, next);
            let camera_ray = sub(point, camera_origin);
            let direction = cross(tangent, camera_ray);
            let length = dot(direction, direction).sqrt();
            if length < MIN_WIDTH_DIRECTION_LENGTH {
                [0.0; 3]
            } else {
                scale(direction, 1.0 / length)
            }
        })
        .collect()
}

/// Expand each center-line point to the three positions emitted by the edge
/// PositionHistory writer (`0x1403f1b56..0x1403f1c34`). The regular vertex
/// stays on the center line and the two edge vertices use the same evaluated
/// width in opposite directions.
pub(super) fn position_history_edge_vertex_positions(
    points: &[[f32; 3]],
    widths: &[f32],
    camera_origin: [f32; 3],
    not_billboard_axis: Option<[f32; 3]>,
) -> Option<Vec<[[f32; 3]; 3]>> {
    if points.len() != widths.len() {
        return None;
    }

    Some(
        points
            .iter()
            .copied()
            .zip(widths.iter().copied())
            .zip(position_history_width_directions(
                points,
                camera_origin,
                not_billboard_axis,
            ))
            .map(|((point, width), direction)| {
                let offset = scale(direction, width);
                [
                    sub(point, offset),
                    point,
                    std::array::from_fn(|axis| point[axis] + offset[axis]),
                ]
            })
            .collect(),
    )
}

/// Update one of the client's allocated ribbon histories (`0x1403f6a30`).
/// Normal updates insert the current outer/center point at the head. The
/// initialization path fills every active history slot with that point so
/// the first frame cannot retain uninitialized positions.
pub(super) fn advance_ribbon_history(
    previous: &[[f32; 3]],
    current: [f32; 3],
    initialize: bool,
) -> Vec<[f32; 3]> {
    if initialize {
        return vec![current; previous.len()];
    }
    if previous.is_empty() {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(previous.len());
    points.push(current);
    points.extend_from_slice(&previous[..previous.len() - 1]);
    points
}

/// Map the three low-word RNG draws used by the client to the symmetric
/// `PnDs` interval. The multiply and divide remain separate to preserve the
/// client's float operation order and inclusive 65535 endpoint.
pub(super) fn position_history_point_distortion(amplitude: f32, draws: [u16; 3]) -> [f32; 3] {
    let minimum = f32::from_bits(amplitude.to_bits() ^ 0x8000_0000);
    draws.map(|draw| (amplitude - minimum) * f32::from(draw) / 65535.0 + minimum)
}

/// Advance the common `LnCT=1` PositionHistory current-point buffer.
/// Client function `0x1403f51b0` shifts older points toward the tail, inserts
/// the current particle position at index zero, and applies `PnDs` only to
/// that new point. Its RBDT=2 branch then applies `CF` as an index-weighted
/// radial displacement from the current parent-emitter transform origin.
pub(super) fn advance_position_history(
    previous: &[[f32; 3]],
    current_position: [f32; 3],
    point_distortion: [f32; 3],
    emitter_origin: [f32; 3],
    cf: f32,
    apply_radial_cf: bool,
) -> Vec<[f32; 3]> {
    if previous.is_empty() {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(previous.len());
    points.push(std::array::from_fn(|axis| {
        current_position[axis] + point_distortion[axis]
    }));
    points.extend_from_slice(&previous[..previous.len() - 1]);

    apply_polyline_radial_cf(&mut points, emitter_origin, cf, apply_radial_cf);

    points
}

/// The radial pass also runs on the first PositionHistory fill. Keep the
/// native Y-squared + X-squared + Z-squared order, including zero-length NaNs.
pub(super) fn apply_polyline_radial_cf(
    points: &mut [[f32; 3]],
    emitter_origin: [f32; 3],
    cf: f32,
    apply_radial_cf: bool,
) {
    if apply_radial_cf && cf != 0.0 {
        let point_count = points.len() as f32;
        for (index, point) in points.iter_mut().enumerate() {
            let direction = sub(*point, emitter_origin);
            let length = ((direction[1] * direction[1] + direction[0] * direction[0])
                + direction[2] * direction[2])
                .sqrt();
            let displacement = (1.0 / length) * index as f32 / point_count * cf;
            for axis in 0..3 {
                point[axis] += direction[axis] * displacement;
            }
        }
    }
}

/// Expand the two endpoints used by the non-PositionHistory and spline
/// branches of client function `0x1403f51b0`. The client divides every
/// endpoint difference times the point index by `point_count - 1`, without
/// a single-point guard. Dividing the index first changes float rounding.
pub(super) fn linear_polyline_points(
    begin: [f32; 3],
    end: [f32; 3],
    point_count: usize,
) -> Vec<[f32; 3]> {
    let denominator = point_count as f32 - 1.0;
    (0..point_count)
        .map(|index| {
            std::array::from_fn(|axis| {
                begin[axis] + (end[axis] - begin[axis]) * index as f32 / denominator
            })
        })
        .collect()
}

/// Blend the newly generated point sequence back toward the snapshot taken
/// at the start of client function `0x1403f51b0`. Point zero stays current;
/// the final point uses the full `Sft` value. The client does not clamp the
/// factor, and a one-point sequence retains its `0 / 0` non-finite edge.
pub(super) fn soften_polyline_points(
    current: &[[f32; 3]],
    previous: &[[f32; 3]],
    softness: f32,
) -> Option<Vec<[f32; 3]>> {
    if current.len() != previous.len() {
        return None;
    }
    if softness == 0.0 {
        return Some(current.to_vec());
    }

    let denominator = current.len().saturating_sub(1) as f32;
    Some(
        current
            .iter()
            .zip(previous)
            .enumerate()
            .map(|(index, (&current, &previous))| {
                let factor = index as f32 * softness / denominator;
                std::array::from_fn(|axis| {
                    current[axis] + (previous[axis] - current[axis]) * factor
                })
            })
            .collect(),
    )
}

/// Return the longitudinal UV step and point order used by the PositionHistory
/// writers. `bLcl` only changes spline UV traversal: the client negates the
/// arc-length-scaled step and visits points from the tail to the head.
pub(super) fn position_history_uv_traversal(
    point_count: usize,
    base_step: f32,
    arc_length: f32,
    configured_length: f32,
    is_spline: bool,
    is_local: bool,
) -> (f32, Vec<usize>) {
    if !is_spline {
        return (base_step, (0..point_count).collect());
    }

    let step = base_step * arc_length / configured_length;
    if is_local {
        (-step, (0..point_count).rev().collect())
    } else {
        (step, (0..point_count).collect())
    }
}

/// Build the per-point byte written for `PnED` by both PositionHistory
/// writers. The client enables the ramp only below half the point count and
/// uses the nearer endpoint distance with truncating integer division.
pub(super) fn position_history_end_distortion(
    point_count: usize,
    point_count_end_distortion: i32,
) -> Vec<u8> {
    let half_point_count = i32::try_from(point_count).unwrap_or(i32::MAX) / 2;
    let enabled = point_count_end_distortion > 0 && point_count_end_distortion < half_point_count;

    (0..point_count)
        .map(|index| {
            if !enabled {
                return u8::MAX;
            }

            let endpoint_distance =
                i32::try_from(index.min(point_count - 1 - index)).unwrap_or(i32::MAX);
            if endpoint_distance >= point_count_end_distortion {
                u8::MAX
            } else {
                ((endpoint_distance * i32::from(u8::MAX)) / point_count_end_distortion) as u8
            }
        })
        .collect()
}

/// Rebuild the client's 16-byte history records with cumulative arc length.
/// Client function `0x140407770` gives coincident points a small positive
/// segment length so later tangent divisions remain defined.
pub(super) fn cumulative_history(positions: &[[f32; 3]]) -> Vec<HistoryPoint> {
    let mut distance = 0.0;
    positions
        .iter()
        .enumerate()
        .map(|(index, &position)| {
            if index > 0 {
                let previous = positions[index - 1];
                let squared = std::array::from_fn::<_, 3, _>(|axis| {
                    let delta = position[axis] - previous[axis];
                    delta * delta
                })
                .into_iter()
                .sum::<f32>();
                distance += squared.sqrt().max(MIN_SEGMENT_LENGTH);
            }
            HistoryPoint { position, distance }
        })
        .collect()
}

/// Sample one located arc-length segment using the client's non-uniform
/// cubic Hermite rule (`0x14040c210`). The caller supplies the segment index,
/// matching the client writer's monotonic segment search.
pub(super) fn sample_history_segment(
    points: &[HistoryPoint],
    target_distance: f32,
    segment: usize,
) -> Option<[f32; 3]> {
    let start = *points.get(segment)?;
    let end = *points.get(segment + 1)?;
    let segment_length = end.distance - start.distance;
    if !segment_length.is_finite() || segment_length <= 0.0 {
        return None;
    }
    let t = (target_distance - start.distance) / segment_length;
    if !t.is_finite() {
        return None;
    }

    let chord = sub(end.position, start.position);
    let tangent_start = if segment > 0 {
        let previous = points[segment - 1];
        scale(
            sub(end.position, previous.position),
            segment_length / (end.distance - previous.distance),
        )
    } else {
        chord
    };
    let tangent_end = if let Some(next) = points.get(segment + 2) {
        scale(
            sub(next.position, start.position),
            segment_length / (next.distance - start.distance),
        )
    } else {
        chord
    };

    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = 3.0 * t2 - 2.0 * t3;
    let h11 = t3 - t2;
    Some(std::array::from_fn(|axis| {
        start.position[axis] * h00
            + end.position[axis] * h01
            + tangent_start[axis] * h10
            + tangent_end[axis] * h11
    }))
}

/// Resample one PositionHistory center line at uniform arc-length intervals
/// using the client's cumulative-distance and non-uniform Hermite helpers.
/// The single-history writer clamps the visible span to `Len`.
pub(super) fn resample_position_history(
    positions: &[[f32; 3]],
    point_count: usize,
    configured_length: f32,
) -> Option<Vec<[f32; 3]>> {
    if positions.len() < 2
        || point_count < 2
        || !configured_length.is_finite()
        || configured_length <= 0.0
    {
        return None;
    }

    let history = cumulative_history(positions);
    let arc_length = visible_position_history_arc_length(&history, configured_length)?;

    let denominator = (point_count - 1) as f32;
    let mut segment = 0;
    (0..point_count)
        .map(|index| {
            let target = index as f32 / denominator * arc_length;
            while segment + 2 < history.len() && target > history[segment + 1].distance {
                segment += 1;
            }
            sample_history_segment(&history, target, segment)
        })
        .collect()
}

/// Return the cumulative history span visible through the configured `Len`
/// clamp. This is also the numerator used by the spline writer's UV step.
pub(super) fn position_history_uv_span(
    positions: &[[f32; 3]],
    configured_length: f32,
) -> Option<f32> {
    let history = cumulative_history(positions);
    let arc_length = visible_position_history_arc_length(&history, configured_length)?;
    Some(arc_length / configured_length)
}

fn visible_position_history_arc_length(
    history: &[HistoryPoint],
    configured_length: f32,
) -> Option<f32> {
    if history.len() < 2 || !configured_length.is_finite() || configured_length <= 0.0 {
        return None;
    }
    let arc_length = history.last()?.distance.min(configured_length);
    (arc_length.is_finite() && arc_length >= MIN_SEGMENT_LENGTH).then_some(arc_length)
}

/// Build the per-point color sequence used by the PositionHistory writers.
/// A valid PnCC gives the center key its own point; the preceding span
/// approaches but does not reach it. Invalid center counts select the direct
/// begin-to-end path.
pub(super) fn position_history_colors(
    begin: [f32; 4],
    center: [f32; 4],
    end: [f32; 4],
    point_count: usize,
    point_count_center: i32,
) -> Vec<[f32; 4]> {
    let center_index = usize::try_from(point_count_center)
        .ok()
        .filter(|&index| index > 0 && index < point_count);
    (0..point_count)
        .map(|index| {
            let color = match center_index {
                Some(center_index) if index < center_index => {
                    mix(begin, center, index as f32 / center_index as f32)
                }
                Some(center_index) => {
                    let denominator = point_count - center_index - 1;
                    let t = if denominator == 0 {
                        0.0
                    } else {
                        (index - center_index) as f32 / denominator as f32
                    };
                    mix(center, end, t)
                }
                None => {
                    let denominator = point_count.saturating_sub(1);
                    let t = if denominator == 0 {
                        0.0
                    } else {
                        index as f32 / denominator as f32
                    };
                    mix(begin, end, t)
                }
            };
            quantize_color(color)
        })
        .collect()
}

/// Expand the regular and edge gradients to the three vertices emitted for
/// each point by the edge PositionHistory writer (`0x1403f2ef0`).
pub(super) fn position_history_edge_vertex_colors(
    regular: [[f32; 4]; 3],
    edge: [[f32; 4]; 3],
    point_count: usize,
    point_count_center: i32,
) -> Vec<[[f32; 4]; 3]> {
    let regular = position_history_colors(
        regular[0],
        regular[1],
        regular[2],
        point_count,
        point_count_center,
    );
    let edge = position_history_colors(edge[0], edge[1], edge[2], point_count, point_count_center);
    regular
        .into_iter()
        .zip(edge)
        .map(|(regular, edge)| [edge, regular, edge])
        .collect()
}

/// Pack one edge-writer UV cross-section. Each UV set has an origin, a base
/// span subtracted from all vertices, and a cross span that places the two
/// outer vertices around the regular center vertex (`0x1403f3863`).
pub(super) fn position_history_edge_uvs(
    origin: [f32; 2],
    base_span: [f32; 2],
    cross_span: [f32; 2],
    uv_precision: i32,
) -> [[i16; 2]; 3] {
    [-0.5, 0.0, 0.5].map(|cross_factor| {
        std::array::from_fn(|axis| {
            pack_uv(
                origin[axis] - 0.5 * base_span[axis] + cross_factor * cross_span[axis],
                uv_precision,
            )
        })
    })
}

/// Apply the V-scroll adjustment used by the common UV record producer when
/// `CUvT` is ByParameter and the sampled Y-scroll curve has flag bits 2..3 set
/// to `2`. The client uses truncating signed integer arithmetic here rather
/// than a floating-point remainder.
pub(super) fn adjust_position_history_uv_scroll_v(
    scroll_v: f32,
    calculate_uv: u32,
    scroll_y_flags: u32,
) -> f32 {
    if calculate_uv != 0 || scroll_y_flags & 0xc != 0x8 || scroll_v == 0.0 {
        return scroll_v;
    }

    let truncated =
        if scroll_v.is_finite() && scroll_v >= i32::MIN as f32 && scroll_v < 2_147_483_648.0 {
            scroll_v.trunc() as i32
        } else {
            i32::MIN
        };
    if scroll_v > 0.0 {
        scroll_v - ((truncated & !1).wrapping_sub(2) as f32)
    } else {
        scroll_v + ((truncated & 2).wrapping_add(2) as f32)
    }
}

/// Build the common 32-byte UV record consumed by the PositionHistory
/// writers. Client function `0x1403e3c00` multiplies a clockwise 2D rotation
/// by the sampled non-uniform scale, stores two three-component matrix rows,
/// and places the scroll origin (offset by 0.5) in each row's fourth slot.
pub(super) fn position_history_uv_record(
    scale: [f32; 2],
    scroll: [f32; 2],
    rotation: f32,
) -> [f32; 8] {
    let (sin, cos) = rotation.sin_cos();
    [
        cos * scale[0],
        -sin * scale[0],
        0.0,
        scroll[0] + 0.5,
        sin * scale[1],
        cos * scale[1],
        0.0,
        scroll[1] + 0.5,
    ]
}

fn pack_uv(value: f32, uv_precision: i32) -> i16 {
    const SCALE: [f32; 4] = [1000.0, 200.0, 100.0, 0.0];
    let scaled = value * SCALE[(uv_precision as usize) & 3];
    let truncated = if scaled.is_finite() && scaled >= i32::MIN as f32 && scaled < 2_147_483_648.0 {
        scaled.trunc() as i32
    } else {
        i32::MIN
    };
    truncated as i16
}

fn mix(start: [f32; 4], end: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|axis| start[axis] + (end[axis] - start[axis]) * t)
}

fn quantize_color(color: [f32; 4]) -> [f32; 4] {
    color.map(|value| (value * 1000.0).clamp(0.0, 10000.0).trunc() * 0.001)
}

fn sub(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|axis| left[axis] - right[axis])
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    value.map(|axis| axis * factor)
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left.into_iter().zip(right).map(|(a, b)| a * b).sum()
}

fn normalize_unchecked(value: [f32; 3]) -> [f32; 3] {
    scale(value, 1.0 / dot(value, value).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey, AvfxParticleDataPolyline, ParticleType};

    fn captured_float(value: &serde_json::Value) -> f32 {
        f32::from_bits(value.as_u64().unwrap() as u32)
    }

    fn captured_point(value: &serde_json::Value) -> [f32; 3] {
        std::array::from_fn(|axis| captured_float(&value[axis]))
    }

    fn compare_original_point_history(cases: &[serde_json::Value]) -> serde_json::Value {
        use super::super::VfxClientRandomState;
        let mut components = 0_usize;
        let mut rng_draws = 0_usize;
        let mut skipped = 0_usize;
        let mut old_first_fill_mismatches = 0_usize;
        let mut old_linear_rounding_mismatches = 0_usize;
        for (case_index, case) in cases.iter().enumerate() {
            let count = case["count"].as_u64().unwrap() as usize;
            let history = case["type"] == 1;
            let length = captured_float(&case["length"]);
            let cf = captured_float(&case["cf"]);
            let softness = captured_float(&case["softness"]);
            let distortion = captured_float(&case["distortion"]);
            let origin = captured_point(&case["origin"]);
            let radial = case["axis"] == 2;
            let mut points = vec![[0.0; 3]; count];
            let mut random =
                VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
            let mut counter = 1;
            for (step_index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
                let first = step["first"].as_bool().unwrap();
                if first {
                    counter = 1;
                }
                assert_eq!(counter, step["beforeCounter"].as_i64().unwrap());
                let cache_only = !first && step["cacheOnly"] == true;
                let lod_skip = !cache_only && history && counter > 1 && length > 1.0;
                let mut draws = 0;
                if lod_skip {
                    counter -= 1;
                } else if !cache_only {
                    if history {
                        counter = case["lod"].as_i64().unwrap();
                    }
                    let previous = points.clone();
                    let position = captured_point(&step["position"]);
                    if history && first {
                        points.fill(position);
                        apply_polyline_radial_cf(&mut points, origin, cf, radial);
                        old_first_fill_mismatches += points
                            .iter()
                            .flatten()
                            .zip(std::iter::repeat(position).take(count).flatten())
                            .filter(|(a, b)| a.to_bits() != b.to_bits())
                            .count();
                    } else if history {
                        let perturbation = if distortion == 0.0 {
                            [0.0; 3]
                        } else {
                            draws += 3;
                            position_history_point_distortion(
                                distortion,
                                std::array::from_fn(|_| random.next_u16()),
                            )
                        };
                        points = advance_position_history(
                            &points,
                            position,
                            perturbation,
                            origin,
                            cf,
                            radial,
                        );
                    } else {
                        let basis = captured_point(&case["basis"]);
                        let end = std::array::from_fn(|a| position[a] + length * basis[a]);
                        points = linear_polyline_points(position, end, count);
                        for (i, point) in points.iter().enumerate() {
                            let t = i as f32 / (count as f32 - 1.0);
                            for a in 0..3 {
                                let old = position[a] + (end[a] - position[a]) * t;
                                if old.is_finite() && old.to_bits() != point[a].to_bits() {
                                    old_linear_rounding_mismatches += 1;
                                }
                            }
                        }
                        if distortion != 0.0 {
                            for point in points.iter_mut().take(count.saturating_sub(1)).skip(1) {
                                draws += 3;
                                let perturbation = position_history_point_distortion(
                                    distortion,
                                    std::array::from_fn(|_| random.next_u16()),
                                );
                                for a in 0..3 {
                                    point[a] += perturbation[a];
                                }
                            }
                        }
                        apply_polyline_radial_cf(&mut points, origin, cf, radial);
                    }
                    if !first {
                        points = soften_polyline_points(&points, &previous, softness).unwrap();
                    }
                } else {
                    skipped += 1;
                }
                assert_eq!(step["written"], !cache_only && !lod_skip);
                assert_eq!(step["counter"].as_i64().unwrap(), counter);
                assert_eq!(step["draws"].as_u64().unwrap(), draws);
                rng_draws += draws as usize;
                let expected_state: [u32; 4] =
                    std::array::from_fn(|i| step["state"][i].as_u64().unwrap() as u32);
                assert_eq!(
                    random.words(),
                    expected_state,
                    "case {case_index} step {step_index}"
                );
                for (index, point) in points.iter().enumerate() {
                    let expected = captured_point(&step["points"][index]);
                    for a in 0..3 {
                        assert!(
                            (point[a].is_nan() && expected[a].is_nan())
                                || point[a].to_bits() == expected[a].to_bits(),
                            "case {case_index} step {step_index} point {index} axis {a}: {:?} != {:?}",
                            point[a],
                            expected[a],
                        );
                        components += 1;
                    }
                }
            }
        }
        serde_json::json!({
            "cases": cases.len(), "steps": cases.len() * 7, "components": components,
            "rngDraws": rng_draws, "cacheOnlySkips": skipped,
            "oldFirstFillComponentMismatches": old_first_fill_mismatches,
            "oldLinearRoundingComponentMismatches": old_linear_rounding_mismatches,
            "differences": 0, "nanComparison": "classification; all other float bits exact",
            "lodProductionIntegrated": false, "axisOrderDistortionProductionIntegrated": false,
            "fullDrawCompared": false, "gpuCompared": false,
        })
    }

    #[test]
    fn original_polyline_point_history_regressions() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/polyline-history.json")).unwrap();
        let comparison = compare_original_point_history(&cases);
        assert!(
            comparison["oldFirstFillComponentMismatches"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            comparison["oldLinearRoundingComponentMismatches"]
                .as_u64()
                .unwrap()
                > 0
        );
    }

    #[test]
    #[ignore = "requires installed-client probe-polyline-history.py capture"]
    fn original_polyline_point_history_full_matrix() {
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let capture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(output.join("polyline-history-client-probe.json")).unwrap(),
        )
        .unwrap();
        let comparison = compare_original_point_history(capture["cases"].as_array().unwrap());
        std::fs::write(
            output.join("polyline-history-rust-comparison.json"),
            serde_json::to_string_pretty(&comparison).unwrap() + "\n",
        )
        .unwrap();
    }

    fn constant(value: f32) -> AvfxCurve {
        AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: value,
            }],
            ..Default::default()
        }
    }

    fn assert_colors_close(actual: &[[f32; 4]], expected: &[[f32; 4]]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            for axis in 0..4 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 1e-5,
                    "actual={actual:?}, expected={expected:?}"
                );
            }
        }
    }

    fn assert_vertex_colors_close(actual: &[[[f32; 4]; 3]], expected: &[[[f32; 4]; 3]]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_colors_close(actual, expected);
        }
    }

    #[test]
    fn axis_width_uses_the_two_perpendicular_instance_scales() {
        let scale = [2.0, 4.0, 8.0];
        assert_eq!(scale_width_for_direction(3.0, scale, 0), 18.0);
        assert_eq!(scale_width_for_direction(3.0, scale, 1), 15.0);
        assert_eq!(scale_width_for_direction(3.0, scale, 2), 9.0);
    }

    #[test]
    fn non_axis_width_modes_do_not_apply_instance_scale() {
        for rbdt in [-1, 3, 6, 10] {
            assert_eq!(scale_width_for_direction(3.0, [2.0, 4.0, 8.0], rbdt), 3.0);
        }
    }

    #[test]
    fn verified_linear_edge_subset_evaluates_compact_render_data() {
        let mut particle = AvfxParticle {
            particle_type: Some(ParticleType::Polyline),
            rotation_direction_base: 1,
            ..Default::default()
        };
        particle.data = AvfxParticleData::Polyline(AvfxParticleDataPolyline {
            create_line_type: 0,
            point_count: 3,
            point_count_center: 1,
            use_edge: true,
            connect_target_reverse: true,
            length: constant(2.0),
            width: constant(0.5),
            width_begin: constant(1.0),
            width_center: constant(2.0),
            width_end: constant(3.0),
            ..Default::default()
        });
        let sampled = sample_at(
            &particle,
            CurveAges {
                local: 0.0,
                total: 0.0,
            },
            7,
            [0.5; 4],
            [1.0, 2.0, 3.0],
            [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 6.0]],
        )
        .unwrap();
        assert_eq!(sampled.points().last(), Some(&[1.0, 10.0, 3.0]));
        // RBDT Y uses mean X/Z scale: (2 + 6) / 2.
        assert_eq!(sampled.widths, [2.0, 4.0, 6.0]);
        assert_eq!(
            sampled.points(),
            [[1.0, 2.0, 3.0], [1.0, 6.0, 3.0], [1.0, 10.0, 3.0]]
        );
        assert_eq!(sampled.point_widths(), [2.0, 4.0, 6.0]);
        assert_eq!(sampled.point_colors()[0], [[0.5; 4]; 3]);
    }

    #[test]
    fn static_non_edge_polyline_ignores_local_flag_and_keeps_end_distortion() {
        let particle = AvfxParticle {
            particle_type: Some(ParticleType::Polyline),
            rotation_direction_base: 1,
            data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                create_line_type: 0,
                point_count: 6,
                use_edge: false,
                is_local: true,
                point_count_end_distortion: 1,
                length: constant(5.0),
                width: constant(1.0),
                ..Default::default()
            }),
            ..Default::default()
        };

        let sampled = sample_at(
            &particle,
            CurveAges {
                local: 0.0,
                total: 0.0,
            },
            7,
            [1.0; 4],
            [1.0, 2.0, 3.0],
            super::super::VFX_IDENTITY_BASIS,
        )
        .expect("non-edge static Polyline should use the existing two-column writer");
        assert!(!sampled.use_edge);
        assert!(!sampled.reverse_points);
        assert_eq!(sampled.uv_span, 1.0);
        assert_eq!(
            sampled.points(),
            [
                [1.0, 2.0, 3.0],
                [1.0, 3.0, 3.0],
                [1.0, 4.0, 3.0],
                [1.0, 5.0, 3.0],
                [1.0, 6.0, 3.0],
                [1.0, 7.0, 3.0]
            ]
        );
        assert_eq!(sampled.point_end_distortion(), [0, 255, 255, 255, 255, 0]);
    }

    #[test]
    fn history_entry_preserves_supplied_points_and_evaluates_vertex_attributes() {
        let particle = AvfxParticle {
            particle_type: Some(ParticleType::Polyline),
            rotation_direction_base: 2,
            data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                create_line_type: 1,
                point_count: 6,
                point_count_center: 3,
                point_count_end_distortion: 2,
                use_edge: true,
                width: constant(0.5),
                width_begin: constant(1.0),
                width_center: constant(2.0),
                width_end: constant(3.0),
                cf: constant(4.0),
                softness: constant(0.75),
                point_distortion: constant(8.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 3.0, 1.0],
            [4.0, 3.0, 2.0],
            [7.0, 2.0, 3.0],
            [11.0, 0.0, 5.0],
        ];
        let sampled = sample_history_at(
            &particle,
            CurveAges {
                local: 0.0,
                total: 0.0,
            },
            9,
            [0.5; 4],
            &points,
            [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 6.0]],
            1.0,
        )
        .unwrap();

        assert_eq!(sampled.points(), points);
        assert_eq!(sampled.point_end_distortion(), [0, 127, 255, 255, 127, 0]);
        // RBDT Z uses mean X/Y scale: (2 + 4) / 2.
        assert_eq!(sampled.widths, [1.5, 3.0, 4.5]);
        assert_eq!(sampled.point_colors()[0], [[0.5; 4]; 3]);
    }

    #[test]
    fn history_spline_and_binding_variants_stay_out_of_dedicated_geometry() {
        let base = AvfxParticleDataPolyline {
            point_count: 6,
            use_edge: true,
            ..Default::default()
        };
        for data in [
            AvfxParticleDataPolyline {
                create_line_type: 1,
                ..base.clone()
            },
            AvfxParticleDataPolyline {
                is_spline: true,
                ..base.clone()
            },
            AvfxParticleDataPolyline {
                bind_weapon: true,
                ..base.clone()
            },
            AvfxParticleDataPolyline {
                bind_weapon_type: 1,
                ..base.clone()
            },
            AvfxParticleDataPolyline {
                connect_target: true,
                ..base.clone()
            },
            AvfxParticleDataPolyline {
                softness: constant(0.5),
                ..base.clone()
            },
        ] {
            let particle = AvfxParticle {
                particle_type: Some(ParticleType::Polyline),
                data: AvfxParticleData::Polyline(data),
                ..Default::default()
            };
            assert!(
                sample_at(
                    &particle,
                    CurveAges {
                        local: 0.0,
                        total: 0.0,
                    },
                    0,
                    [1.0; 4],
                    [0.0; 3],
                    super::super::VFX_IDENTITY_BASIS,
                )
                .is_none()
            );
        }

        let particle = AvfxParticle {
            particle_type: Some(ParticleType::Polyline),
            rotation_direction_base: 2,
            data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                cf: constant(0.25),
                ..base
            }),
            ..Default::default()
        };
        assert!(
            sample_at(
                &particle,
                CurveAges {
                    local: 0.0,
                    total: 0.0,
                },
                0,
                [1.0; 4],
                [0.0; 3],
                super::super::VFX_IDENTITY_BASIS,
            )
            .is_none()
        );
    }

    #[test]
    fn widths_scale_all_keys_before_using_the_center_split() {
        assert_eq!(
            position_history_widths(2.0, 1.0, 3.0, 5.0, 5, 2),
            [2.0, 4.0, 6.0, 8.0, 10.0]
        );
    }

    #[test]
    fn first_width_span_approaches_but_does_not_include_the_center_key() {
        assert_eq!(
            position_history_widths(1.0, 0.0, 8.0, 12.0, 6, 4),
            [0.0, 2.0, 4.0, 6.0, 8.0, 12.0]
        );
    }

    #[test]
    fn invalid_width_center_uses_the_direct_begin_to_end_span() {
        for center_count in [-1, 0, 5, 6] {
            assert_eq!(
                position_history_widths(2.0, 1.0, 99.0, 5.0, 5, center_count),
                [2.0, 4.0, 6.0, 8.0, 10.0]
            );
        }
    }

    #[test]
    fn width_sequence_handles_empty_and_single_point_inputs() {
        assert!(position_history_widths(2.0, 1.0, 3.0, 5.0, 0, 0).is_empty());
        assert_eq!(position_history_widths(2.0, 1.0, 3.0, 5.0, 1, 0), [2.0]);
    }

    #[test]
    fn billboard_width_direction_uses_clamped_neighbors_and_camera_ray() {
        let directions = position_history_width_directions(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [0.0, 0.0, 1.0],
            None,
        );
        assert_eq!(directions, [[0.0, -1.0, 0.0]; 3]);
    }

    #[test]
    fn degenerate_billboard_width_direction_uses_the_client_zero_fallback() {
        assert_eq!(
            position_history_width_directions(
                &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                [2.0, 0.0, 0.0],
                None,
            ),
            [[0.0; 3]; 2]
        );
        assert!(position_history_width_directions(&[], [0.0; 3], None).is_empty());
    }

    #[test]
    fn not_billboard_width_direction_normalizes_the_selected_axis_once() {
        assert_eq!(
            position_history_width_directions(&[[0.0; 3]; 2], [9.0; 3], Some([0.0, 3.0, 4.0])),
            [[0.0, 0.6, 0.8]; 2]
        );
    }

    #[test]
    fn zero_not_billboard_axis_preserves_client_non_finite_normalization() {
        let directions = position_history_width_directions(&[[0.0; 3]], [0.0; 3], Some([0.0; 3]));
        assert!(directions[0].into_iter().all(f32::is_nan));
    }

    #[test]
    fn edge_writer_expands_center_points_in_edge_regular_edge_order() {
        assert_eq!(
            position_history_edge_vertex_positions(
                &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                &[0.5, 1.0, 1.5],
                [0.0, 0.0, 1.0],
                None,
            ),
            Some(vec![
                [[0.0, 0.5, 0.0], [0.0, 0.0, 0.0], [0.0, -0.5, 0.0]],
                [[1.0, 1.0, 0.0], [1.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                [[2.0, 1.5, 0.0], [2.0, 0.0, 0.0], [2.0, -1.5, 0.0]],
            ])
        );
    }

    #[test]
    fn edge_writer_rejects_mismatched_widths_and_collapses_degenerate_directions() {
        assert_eq!(
            position_history_edge_vertex_positions(&[[0.0; 3]], &[], [0.0; 3], None),
            None
        );
        assert_eq!(
            position_history_edge_vertex_positions(
                &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                &[2.0, 3.0],
                [2.0, 0.0, 0.0],
                None,
            ),
            Some(vec![[[0.0; 3]; 3], [[1.0, 0.0, 0.0]; 3]])
        );
    }

    #[test]
    fn ribbon_history_initialization_fills_and_later_updates_shift() {
        let uninitialized = [[9.0; 3], [8.0; 3], [7.0; 3]];
        let initialized = advance_ribbon_history(&uninitialized, [1.0, 2.0, 3.0], true);
        assert_eq!(initialized, [[1.0, 2.0, 3.0]; 3]);
        assert_eq!(
            advance_ribbon_history(&initialized, [4.0, 5.0, 6.0], false),
            [[4.0, 5.0, 6.0], [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]
        );
        assert!(advance_ribbon_history(&[], [1.0; 3], false).is_empty());
    }

    #[test]
    fn position_history_shifts_and_distorts_only_the_new_point() {
        let points = advance_position_history(
            &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            [10.0, 20.0, 30.0],
            [0.5, -1.0, 2.0],
            [0.0; 3],
            0.0,
            true,
        );
        assert_eq!(
            points,
            [[10.5, 19.0, 32.0], [1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
        );
    }

    #[test]
    fn point_distortion_uses_inclusive_symmetric_low_word_range() {
        assert_eq!(
            position_history_point_distortion(2.0, [0, 32768, u16::MAX]),
            [-2.0, 0.000_030_517_578, 2.0]
        );
        assert_eq!(
            position_history_point_distortion(-2.0, [0, u16::MAX, 0]),
            [2.0, -2.0, 2.0]
        );
        assert_eq!(
            position_history_point_distortion(0.0, [0, 123, u16::MAX]),
            [0.0; 3]
        );
    }

    #[test]
    fn non_history_endpoints_expand_with_client_point_count_denominator() {
        assert_eq!(
            linear_polyline_points([1.0, 2.0, 3.0], [5.0, 8.0, -1.0], 3),
            [[1.0, 2.0, 3.0], [3.0, 5.0, 1.0], [5.0, 8.0, -1.0]]
        );
        assert!(linear_polyline_points([0.0; 3], [1.0; 3], 0).is_empty());
    }

    #[test]
    fn single_non_history_point_preserves_client_zero_denominator() {
        let points = linear_polyline_points([1.0; 3], [2.0; 3], 1);
        assert!(points[0].into_iter().all(f32::is_nan));
    }

    #[test]
    fn softness_blends_each_point_toward_the_pre_update_snapshot() {
        assert_eq!(
            soften_polyline_points(
                &[[10.0; 3], [20.0; 3], [30.0; 3]],
                &[[0.0; 3], [10.0; 3], [20.0; 3]],
                0.5,
            ),
            Some(vec![[10.0; 3], [17.5; 3], [25.0; 3]])
        );
    }

    #[test]
    fn softness_factor_is_not_clamped() {
        assert_eq!(
            soften_polyline_points(&[[0.0; 3]; 3], &[[10.0; 3]; 3], 2.0,),
            Some(vec![[0.0; 3], [10.0; 3], [20.0; 3]])
        );
        assert_eq!(
            soften_polyline_points(&[[0.0; 3]; 3], &[[10.0; 3]; 3], -1.0,),
            Some(vec![[0.0; 3], [-5.0; 3], [-10.0; 3]])
        );
    }

    #[test]
    fn softness_zero_bypasses_blending_and_mismatched_snapshots_are_rejected() {
        let bypassed = soften_polyline_points(&[[f32::NAN, 2.0, 3.0]], &[[9.0; 3]], 0.0).unwrap();
        assert!(bypassed[0][0].is_nan());
        assert_eq!(&bypassed[0][1..], &[2.0, 3.0]);
        assert_eq!(
            soften_polyline_points(&[[0.0; 3]], &[[0.0; 3]; 2], 1.0),
            None
        );
    }

    #[test]
    fn single_point_softness_preserves_client_non_finite_division() {
        let points = soften_polyline_points(&[[1.0; 3]], &[[2.0; 3]], 1.0).unwrap();
        assert!(points[0].into_iter().all(f32::is_nan));
        assert_eq!(soften_polyline_points(&[], &[], 1.0), Some(Vec::new()));
    }

    #[test]
    fn local_spline_reverses_arc_length_uv_traversal() {
        let (step, order) = position_history_uv_traversal(4, 0.25, 6.0, 3.0, true, true);
        assert_eq!(step, -0.5);
        assert_eq!(order, [3, 2, 1, 0]);
    }

    #[test]
    fn local_flag_does_not_change_non_spline_uv_traversal() {
        let (step, order) = position_history_uv_traversal(3, 0.5, 99.0, 0.0, false, true);
        assert_eq!(step, 0.5);
        assert_eq!(order, [0, 1, 2]);
    }

    #[test]
    fn history_entry_marks_only_local_splines_for_reverse_negative_uv_output() {
        let evaluate = |is_spline, is_local, uv_span| {
            let particle = AvfxParticle {
                particle_type: Some(ParticleType::Polyline),
                data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                    create_line_type: 1,
                    point_count: 3,
                    use_edge: true,
                    is_spline,
                    is_local,
                    ..Default::default()
                }),
                ..Default::default()
            };
            sample_history_at(
                &particle,
                CurveAges {
                    local: 0.0,
                    total: 0.0,
                },
                0,
                [1.0; 4],
                &[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                super::super::VFX_IDENTITY_BASIS,
                uv_span,
            )
            .unwrap()
        };

        let local_spline = evaluate(true, true, 0.4);
        assert!(local_spline.reverse_points);
        assert_eq!(local_spline.uv_span, -0.4);
        assert_eq!(local_spline.points()[0], [0.0; 3]);

        let local_unsmoothed = evaluate(false, true, 1.0);
        assert!(!local_unsmoothed.reverse_points);
        assert_eq!(local_unsmoothed.uv_span, 1.0);
    }

    #[test]
    fn point_end_distortion_ramps_from_both_endpoints_with_integer_truncation() {
        assert_eq!(
            position_history_end_distortion(12, 5),
            [0, 51, 102, 153, 204, 255, 255, 204, 153, 102, 51, 0]
        );
    }

    #[test]
    fn point_end_distortion_must_be_positive_and_below_half_the_point_count() {
        for point_count_end_distortion in [-1, 0, 5, 6] {
            assert_eq!(
                position_history_end_distortion(10, point_count_end_distortion),
                [u8::MAX; 10]
            );
        }
    }

    #[test]
    fn point_end_distortion_handles_empty_and_odd_point_sequences() {
        assert!(position_history_end_distortion(0, 1).is_empty());
        assert_eq!(
            position_history_end_distortion(9, 3),
            [0, 85, 170, 255, 255, 255, 170, 85, 0]
        );
    }

    #[test]
    fn uv_record_maps_rotation_scale_and_scroll_to_all_eight_components() {
        assert_eq!(
            position_history_uv_record([2.0, 3.0], [0.25, -0.75], 0.0),
            [2.0, -0.0, 0.0, 0.75, 0.0, 3.0, 0.0, -0.25]
        );

        let record =
            position_history_uv_record([2.0, 3.0], [0.25, -0.75], std::f32::consts::FRAC_PI_2);
        let expected = [0.0, -2.0, 0.0, 0.75, 3.0, 0.0, 0.0, -0.25];
        for (actual, expected) in record.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6, "{record:?}");
        }
    }

    #[test]
    fn uv_scroll_v_uses_client_signed_truncation_for_the_flagged_parameter_mode() {
        for (scroll, expected) in [
            (0.25, 2.25),
            (2.25, 2.25),
            (4.25, 2.25),
            (-0.25, 1.75),
            (-2.25, 1.75),
            (-4.25, -2.25),
        ] {
            assert_eq!(
                adjust_position_history_uv_scroll_v(scroll, 0, 0x8),
                expected
            );
        }
    }

    #[test]
    fn uv_scroll_v_adjustment_requires_parameter_mode_and_matching_curve_flags() {
        for (scroll, calculate_uv, flags) in [
            (0.0, 0, 0x8),
            (4.25, 1, 0x8),
            (4.25, 0, 0x0),
            (4.25, 0, 0xc),
        ] {
            assert_eq!(
                adjust_position_history_uv_scroll_v(scroll, calculate_uv, flags),
                scroll
            );
        }
    }

    #[test]
    fn rbdt_two_cf_pushes_points_radially_by_index_over_point_count() {
        let points = advance_position_history(
            &[[0.0, 2.0, 0.0], [0.0, 0.0, 3.0], [-4.0, 0.0, 0.0], [9.0; 3]],
            [1.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            4.0,
            true,
        );
        assert_eq!(
            points,
            [
                [1.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [0.0, 0.0, 5.0],
                [-7.0, 0.0, 0.0]
            ]
        );
    }

    #[test]
    fn radial_cf_branch_preserves_client_zero_length_non_finite_result() {
        let points = advance_position_history(
            &[[0.0; 3], [1.0, 0.0, 0.0]],
            [2.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            1.0,
            true,
        );
        assert_eq!(points[0], [2.0, 0.0, 0.0]);
        assert!(points[1].into_iter().all(f32::is_nan));
    }

    #[test]
    fn cumulative_history_clamps_coincident_segments_to_client_epsilon() {
        let points = cumulative_history(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [1.0, 2.003, 3.004]]);
        assert_eq!(points[0].distance, 0.0);
        assert_eq!(points[1].distance, MIN_SEGMENT_LENGTH);
        assert!((points[2].distance - 0.0051).abs() < 1e-7);
    }

    #[test]
    fn two_point_history_reduces_to_linear_interpolation() {
        let points = cumulative_history(&[[1.0, -2.0, 4.0], [5.0, 6.0, -8.0]]);
        let sampled = sample_history_segment(&points, points[1].distance * 0.25, 0).unwrap();
        assert_eq!(sampled, [2.0, 0.0, 1.0]);
    }

    #[test]
    fn interior_tangents_are_scaled_by_neighboring_arc_lengths() {
        let points = cumulative_history(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [5.0, 2.0, 0.0],
        ]);
        let target = (points[1].distance + points[2].distance) * 0.5;
        let sampled = sample_history_segment(&points, target, 1).unwrap();
        assert!((sampled[0] - 1.459_220_2).abs() < 1e-6, "{sampled:?}");
        assert!((sampled[1] - 1.065_982_8).abs() < 1e-6, "{sampled:?}");
        assert_eq!(sampled[2], 0.0);
    }

    #[test]
    fn invalid_segment_is_rejected() {
        let points = cumulative_history(&[[0.0; 3], [1.0, 0.0, 0.0]]);
        assert!(sample_history_segment(&points, 0.0, 1).is_none());
        assert!(sample_history_segment(&points, f32::NAN, 0).is_none());
    }

    #[test]
    fn spline_resampling_uses_uniform_arc_length_and_configured_length_clamp() {
        let sampled = resample_position_history(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            4,
            1.5,
        )
        .unwrap();
        assert_eq!(
            sampled,
            [
                [0.0, 0.0, 0.0],
                [0.5, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.5, 0.0, 0.0],
            ]
        );
    }

    #[test]
    fn spline_uv_span_uses_clamped_pre_resampling_history_length() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert_eq!(position_history_uv_span(&positions, 4.0), Some(0.5));
        assert_eq!(position_history_uv_span(&positions, 1.5), Some(1.0));
    }

    #[test]
    fn spline_resampling_keeps_coincident_history_finite() {
        let sampled = resample_position_history(
            &[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [2.0, 3.0, 4.0]],
            7,
            10.0,
        )
        .unwrap();
        assert_eq!(sampled.len(), 7);
        assert!(sampled.iter().flatten().all(|value| value.is_finite()));
        assert_eq!(sampled[0], [1.0, 2.0, 3.0]);
        assert_eq!(sampled[6], [2.0, 3.0, 4.0]);
    }

    #[test]
    fn center_count_splits_begin_center_and_center_end_spans() {
        let colors = position_history_colors(
            [0.0, 1.0, 2.0, 3.0],
            [2.0, 3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0, 9.0],
            5,
            2,
        );
        assert_colors_close(
            &colors,
            &[
                [0.0, 1.0, 2.0, 3.0],
                [1.0, 2.0, 3.0, 4.0],
                [2.0, 3.0, 4.0, 5.0],
                [4.0, 5.0, 6.0, 7.0],
                [6.0, 7.0, 8.0, 9.0],
            ],
        );
    }

    #[test]
    fn invalid_center_count_uses_direct_begin_to_end_gradient() {
        for center_count in [-1, 0, 4, 5] {
            let actual = position_history_colors([0.0; 4], [9.0; 4], [3.0; 4], 4, center_count);
            assert_colors_close(&actual, &[[0.0; 4], [1.0; 4], [2.0; 4], [3.0; 4]]);
        }
    }

    #[test]
    fn colors_use_client_fixed_point_range_and_single_point_fallback() {
        assert_eq!(
            position_history_colors([-1.0, 0.1239, 10.5, 1.0], [0.0; 4], [4.0; 4], 1, 0),
            vec![[0.0, 0.123, 10.0, 1.0]]
        );
        assert!(position_history_colors([0.0; 4], [1.0; 4], [2.0; 4], 0, 0).is_empty());
    }

    #[test]
    fn edge_writer_places_regular_color_between_matching_edge_colors() {
        let colors = position_history_edge_vertex_colors(
            [[0.0; 4], [2.0; 4], [6.0; 4]],
            [[1.0; 4], [5.0; 4], [9.0; 4]],
            3,
            1,
        );
        assert_vertex_colors_close(
            &colors,
            &[
                [[1.0; 4], [0.0; 4], [1.0; 4]],
                [[5.0; 4], [2.0; 4], [5.0; 4]],
                [[9.0; 4], [6.0; 4], [9.0; 4]],
            ],
        );
    }

    #[test]
    fn edge_writer_direct_path_ignores_both_center_colors() {
        let colors = position_history_edge_vertex_colors(
            [[0.0; 4], [8.0; 4], [2.0; 4]],
            [[2.0; 4], [9.0; 4], [4.0; 4]],
            3,
            0,
        );
        assert_vertex_colors_close(
            &colors,
            &[
                [[2.0; 4], [0.0; 4], [2.0; 4]],
                [[3.0; 4], [1.0; 4], [3.0; 4]],
                [[4.0; 4], [2.0; 4], [4.0; 4]],
            ],
        );
    }

    #[test]
    fn edge_uvs_place_outer_vertices_around_the_regular_center() {
        assert_eq!(
            position_history_edge_uvs([0.75, 1.0], [0.5, 1.0], [0.2, 0.4], 0),
            [[400, 300], [500, 500], [600, 700]]
        );
    }

    #[test]
    fn edge_uvs_use_client_precision_scales_and_truncate_toward_zero() {
        let expected = [123, 24, 12, 0];
        for (precision, expected) in expected.into_iter().enumerate() {
            let actual =
                position_history_edge_uvs([0.1239, -0.1239], [0.0; 2], [0.0; 2], precision as i32);
            assert_eq!(actual, [[expected, -expected]; 3]);
        }
    }

    #[test]
    fn edge_uv_packing_keeps_client_low_word_and_invalid_conversion() {
        assert_eq!(pack_uv(40.0, 0), -25_536);
        assert_eq!(pack_uv(f32::INFINITY, 0), 0);
        assert_eq!(pack_uv(f32::NAN, 0), 0);
    }
}
