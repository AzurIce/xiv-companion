use crate::avfx::{AvfxCurve, BEHAVIOR_ADD, BEHAVIOR_REPEAT, curve_segment_polynomial};

use super::CurveAges;

// The second integral carries displacement from acceleration; concatenation
// preserves velocity accumulated before the next interval.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Integral {
    pub duration: f64,
    pub value: f64,
    pub displacement: f64,
}

impl Integral {
    fn constant(value: f64, duration: f64) -> Self {
        Self {
            duration,
            value: value * duration,
            displacement: 0.5 * value * duration * duration,
        }
    }

    fn then(self, next: Self) -> Self {
        Self {
            duration: self.duration + next.duration,
            value: self.value + next.value,
            displacement: self.displacement + self.value * next.duration + next.displacement,
        }
    }

    fn plus(self, other: Self) -> Self {
        debug_assert!((self.duration - other.duration).abs() < 1.0e-6);
        Self {
            duration: self.duration.max(other.duration),
            value: self.value + other.value,
            displacement: self.displacement + other.displacement,
        }
    }

    fn offset(self, value: f64) -> Self {
        Self {
            duration: self.duration,
            value: self.value + value * self.duration,
            displacement: self.displacement + value * self.duration * self.duration * 0.5,
        }
    }

    fn repeated(self, count: f64, drift: f64) -> Self {
        // Cycle i adds the constant i*drift. Sum its area and time-weighted
        // area so Add remains bounded even across long motion histories.
        let pairs = count * (count - 1.0) * 0.5;
        Self {
            duration: self.duration * count,
            value: self.value * count + drift * self.duration * pairs,
            displacement: self.displacement * count
                + self.value * self.duration * pairs
                + drift * self.duration * self.duration * pairs * (2.0 * count - 1.0) / 6.0,
        }
    }
}

fn periodic(
    from: f64,
    to: f64,
    start: f64,
    end: f64,
    drift: f64,
    integrate: &impl Fn(f64, f64) -> Integral,
) -> Integral {
    let period = end - start;
    let phase = (from - start).rem_euclid(period);
    let cycle = ((from - start) / period).floor();
    let first_duration = (to - from).min(period - phase);
    let mut result = integrate(start + phase, start + phase + first_duration).offset(cycle * drift);
    let remaining = (to - from - first_duration).max(0.0);
    let cycles = (remaining / period).floor();
    if cycles > 0.0 {
        result = result.then(
            integrate(start, end)
                .offset((cycle + 1.0) * drift)
                .repeated(cycles, drift),
        );
    }
    let tail = remaining - cycles * period;
    if tail > 0.0 {
        result = result.then(integrate(start, start + tail).offset((cycle + 1.0 + cycles) * drift));
    }
    result
}

#[cfg(test)]
pub(super) fn particle_curve(
    main: &AvfxCurve,
    random: &AvfxCurve,
    age: f32,
    loop_start: i32,
    loop_end: i32,
    seed: u64,
) -> Integral {
    looped_curve(
        main,
        random,
        age,
        f64::from(loop_start),
        f64::from(loop_end),
        seed,
    )
}

#[cfg(test)]
pub(super) fn looped_curve(
    main: &AvfxCurve,
    random: &AvfxCurve,
    age: f32,
    loop_start: f64,
    loop_end: f64,
    seed: u64,
) -> Integral {
    looped_curve_range(main, random, [0.0, age], loop_start, loop_end, seed)
}

pub(super) fn looped_curve_range(
    main: &AvfxCurve,
    random: &AvfxCurve,
    range: [f32; 2],
    loop_start: f64,
    loop_end: f64,
    seed: u64,
) -> Integral {
    let [from, to] = range.map(f64::from);
    if to <= from {
        return Integral::default();
    }
    let main_integrate = |from, to| curve(main, from, to, &|index| f64::from(main.keys[index].z));
    let random_integrate = |from, to| random_curve(random, from, to, seed);
    looped_integral(
        from,
        to,
        loop_start,
        loop_end,
        main.post_behavior & 3 >= BEHAVIOR_ADD,
        &main_integrate,
    )
    .plus(looped_integral(
        from,
        to,
        loop_start,
        loop_end,
        random.post_behavior & 3 >= BEHAVIOR_ADD,
        &random_integrate,
    ))
}

fn looped_integral(
    from: f64,
    to: f64,
    loop_start: f64,
    loop_end: f64,
    use_total_age: bool,
    integrate: &impl Fn(f64, f64) -> Integral,
) -> Integral {
    if use_total_age || loop_end <= loop_start || to <= loop_end {
        return integrate(from, to);
    }
    if from >= loop_end {
        return periodic(from, to, loop_start, loop_end, 0.0, integrate);
    }
    integrate(from, loop_end).then(periodic(loop_end, to, loop_start, loop_end, 0.0, integrate))
}

/// Continuous preview integration for the nonlinear VR direction times ARs.
/// Each one-frame panel uses eight-point Gauss-Legendre quadrature. The panel
/// budget bounds unusual long, nonperiodic histories; those are less accurate.
#[cfg(test)]
pub(super) fn particle_vector(
    age: f32,
    loop_start: i32,
    loop_end: i32,
    curves: &[(&AvfxCurve, bool)],
    at: &impl Fn(CurveAges) -> [f64; 3],
) -> [f64; 3] {
    looped_vector(age, f64::from(loop_start), f64::from(loop_end), curves, at)
}

#[cfg(test)]
pub(super) fn looped_vector(
    age: f32,
    loop_start: f64,
    loop_end: f64,
    curves: &[(&AvfxCurve, bool)],
    at: &impl Fn(CurveAges) -> [f64; 3],
) -> [f64; 3] {
    looped_vector_range([0.0, age], loop_start, loop_end, curves, at)
}

pub(super) fn looped_vector_range(
    range: [f32; 2],
    loop_start: f64,
    loop_end: f64,
    curves: &[(&AvfxCurve, bool)],
    at: &impl Fn(CurveAges) -> [f64; 3],
) -> [f64; 3] {
    let add = |left: [f64; 3], right: [f64; 3]| std::array::from_fn(|i| left[i] + right[i]);
    let ages = |total: f64| {
        let total = total as f32;
        let local = if loop_end > loop_start && f64::from(total) >= loop_end {
            (loop_start + (f64::from(total) - loop_start).rem_euclid(loop_end - loop_start)) as f32
        } else {
            total
        };
        CurveAges { local, total }
    };
    let integrate = |from: f64, to: f64| {
        const NODES: [(f64, f64); 4] = [
            (0.1834346424956498, 0.3626837833783620),
            (0.5255324099163290, 0.3137066458778873),
            (0.7966664774136267, 0.2223810344533745),
            (0.9602898564975363, 0.1012285362903763),
        ];
        let span = to.ceil() - from.floor();
        let panels = span.clamp(1.0, 4096.0);
        // Integer panels also split step keys, Repeat seams and Always random
        // changes until the budget is reached (scalar key times are integers).
        let step = (span / panels).ceil().max(1.0);
        let mut result = [0.0; 3];
        for panel in 0..panels as usize {
            let start = (from.floor() + panel as f64 * step).max(from);
            if start >= to {
                break;
            }
            let end = (start.floor() + step).min(to);
            let half = (end - start) * 0.5;
            let midpoint = start + half;
            for (node, weight) in NODES {
                let pair = add(
                    at(ages(midpoint - half * node)),
                    at(ages(midpoint + half * node)),
                );
                result = add(result, pair.map(|value| value * half * weight));
            }
        }
        result
    };
    let repeat = |from: f64, to: f64, start: f64, period: f64| {
        let phase = (from - start).rem_euclid(period);
        let first = (to - from).min(period - phase);
        let mut result = integrate(start + phase, start + phase + first);
        let remaining = (to - from - first).max(0.0);
        let cycles = (remaining / period).floor();
        if cycles > 0.0 {
            result = add(
                result,
                integrate(start, start + period).map(|value| value * cycles),
            );
        }
        let tail = remaining - cycles * period;
        add(result, integrate(start, start + tail))
    };
    let [from, to] = range.map(f64::from);
    if to <= from {
        return [0.0; 3];
    }
    let uses_total_age = curves.iter().any(|(curve, _)| {
        !curve.keys.is_empty()
            && curve.keys.iter().any(|key| key.z != 0.0)
            && curve.post_behavior & 3 >= BEHAVIOR_ADD
    });
    if loop_end > loop_start && to > loop_end && !uses_total_age {
        let end = loop_end.max(from);
        return add(
            integrate(from, end),
            repeat(end, to, loop_start, loop_end - loop_start),
        );
    }

    // Outside all keys the joint function is constant, or periodic with an
    // integer common period. Always-random curves have no repeating tail.
    let mut tail_start = 0.0_f64;
    let mut period = 1_u64;
    let mut periodic = false;
    for &(curve, random) in curves {
        if curve.keys.is_empty() || curve.keys.iter().all(|key| key.z == 0.0) {
            continue;
        }
        if random && matches!(curve.random_type & 7, 3..=5) {
            return integrate(from, to);
        }
        let start = i32::from(curve.keys[0].scalar_time());
        let end = i32::from(curve.keys.last().unwrap().scalar_time());
        tail_start = tail_start.max(f64::from(start.max(end)));
        let behavior = curve.post_behavior & 3;
        if behavior == BEHAVIOR_ADD && curve.keys.last().unwrap().z != curve.keys[0].z {
            return integrate(from, to);
        }
        if matches!(behavior, BEHAVIOR_REPEAT | BEHAVIOR_ADD) && end > start {
            periodic = true;
            let span = (end - start) as u64;
            let (mut a, mut b) = (period, span);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            let Some(common) = (period / a).checked_mul(span) else {
                return integrate(from, to);
            };
            period = common;
        }
    }
    if to <= tail_start {
        return integrate(from, to);
    }
    let end = tail_start.max(from);
    let prefix = integrate(from, end);
    let tail = if periodic {
        repeat(end, to, tail_start, period as f64)
    } else {
        // At duplicate end keys the exact endpoint may select an earlier key;
        // the open tail uses the final key, as does the scalar analytic integral.
        at(ages(f64::from((tail_start as f32).next_up()))).map(|value| value * (to - end))
    };
    add(prefix, tail)
}

fn random_curve(curve_data: &AvfxCurve, from: f64, to: f64, seed: u64) -> Integral {
    if curve_data.random_type & 7 >= 6 || curve_data.keys.iter().all(|key| key.z == 0.0) {
        return Integral::constant(0.0, to - from);
    }
    let at_frame = |start, end, frame| {
        let coefficient = f64::from(super::random_curve_offset(
            curve_data.random_type,
            1.0,
            frame,
            seed,
        ));
        let amplitude = curve(curve_data, start, end, &|index| {
            f64::from(curve_data.keys[index].z)
        });
        Integral {
            duration: amplitude.duration,
            value: amplitude.value * coefficient,
            displacement: amplitude.displacement * coefficient,
        }
    };
    if curve_data.random_type & 7 < 3 {
        return at_frame(from, to, 0);
    }
    let mut result = Integral::default();
    let mut start = from;
    while start < to {
        let end = (start.max(0.0).floor() + 1.0).min(to);
        result = result.then(at_frame(start, end, start.max(0.0) as u64));
        start = end;
    }
    result
}

fn curve(curve: &AvfxCurve, from: f64, to: f64, value: &impl Fn(usize) -> f64) -> Integral {
    let Some(first) = curve.keys.first() else {
        return Integral::constant(0.0, to - from);
    };
    let last = curve.keys.last().unwrap();
    let start = f64::from(first.scalar_time());
    let end = f64::from(last.scalar_time());
    if end <= start {
        // Packed negative times can put the first key after the last. The
        // client's pre-Const branch wins until the first key, then post-Const.
        let prefix = (start - from).clamp(0.0, to - from);
        return Integral::constant(value(0), prefix).then(Integral::constant(
            value(curve.keys.len() - 1),
            to - from - prefix,
        ));
    }
    let within_keys = |from, to| {
        let mut result = Integral::default();
        let mut from = from;
        while from < to {
            let next = curve
                .keys
                .iter()
                .map(|key| f64::from(key.scalar_time()))
                .filter(|&time| time > from)
                .fold(to, f64::min);
            let index = curve
                .keys
                .iter()
                .rposition(|key| f64::from(key.scalar_time()) <= from)
                .unwrap_or(0);
            let part = if index + 1 == curve.keys.len() {
                Integral::constant(value(index), next - from)
            } else {
                segment(curve, index, value, from, next)
            };
            result = result.then(part);
            from = next;
        }
        result
    };
    let mut result = Integral::default();
    if from < start {
        let to = to.min(start);
        let behavior = curve.pre_behavior & 3;
        result = result.then(if matches!(behavior, BEHAVIOR_REPEAT | BEHAVIOR_ADD) {
            let drift = if behavior == BEHAVIOR_ADD {
                value(curve.keys.len() - 1) - value(0)
            } else {
                0.0
            };
            periodic(from, to, start, end, drift, &within_keys)
        } else {
            Integral::constant(value(0), to - from)
        });
    }
    if from < end && to > start {
        result = result.then(within_keys(from.max(start), to.min(end)));
    }
    if to > end {
        let from = from.max(end);
        let behavior = curve.post_behavior & 3;
        result = result.then(if matches!(behavior, BEHAVIOR_REPEAT | BEHAVIOR_ADD) {
            let drift = if behavior == BEHAVIOR_ADD {
                value(curve.keys.len() - 1) - value(0)
            } else {
                0.0
            };
            periodic(from, to, start, end, drift, &within_keys)
        } else {
            Integral::constant(value(curve.keys.len() - 1), to - from)
        });
    }
    result
}

fn segment(
    curve: &AvfxCurve,
    index: usize,
    value: &impl Fn(usize) -> f64,
    from: f64,
    to: f64,
) -> Integral {
    let duration = to - from;
    let start = f64::from(curve.keys[index].scalar_time());
    let span = f64::from(curve.keys[index + 1].scalar_time()) - start;
    let u = (from - start) / span;
    let [a, b, c, d] = curve_segment_polynomial(&curve.keys, index, value);
    // Translate to the integration interval's origin, avoiding subtraction of
    // nearly equal antiderivatives for short intervals near a key boundary.
    let coefficients = [
        ((d * u + c) * u + b) * u + a,
        (3.0 * d * u + 2.0 * c) * u + b,
        c + 3.0 * d * u,
        d,
    ];
    let mut result = Integral {
        duration,
        ..Default::default()
    };
    let mut power = 1.0;
    for (degree, coefficient) in coefficients.into_iter().enumerate() {
        let area = coefficient * power * duration / (degree + 1) as f64;
        result.value += area;
        result.displacement += area * duration / (degree + 2) as f64;
        power *= duration / span;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::AvfxCurveKey;

    fn linear(keys: &[(i16, f32)]) -> AvfxCurve {
        AvfxCurve {
            keys: keys
                .iter()
                .map(|&(time, z)| AvfxCurveKey {
                    time,
                    z,
                    interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                    x: 0.0,
                    y: 0.0,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn vector_integral_combines_periods_and_bounds_nonperiodic_work() {
        let mut two = linear(&[(0, 1.0), (1, 2.0), (2, 1.0)]);
        let mut three = linear(&[(0, 1.0), (1, 3.0), (3, 1.0)]);
        for curve in [&mut two, &mut three] {
            curve.post_behavior = BEHAVIOR_REPEAT;
            for key in &mut curve.keys {
                key.interpolation = AvfxCurveKey::INTERPOLATION_STEP;
            }
        }
        let expected: f64 = (0..6)
            .map(|frame| {
                f64::from(two.value(frame as f32 + 0.5, 0.0) * three.value(frame as f32 + 0.5, 0.0))
            })
            .sum();
        let calls = std::cell::Cell::new(0);
        let actual = particle_vector(
            600_000.0,
            0,
            0,
            &[(&two, false), (&three, false)],
            &|ages| {
                calls.set(calls.get() + 1);
                [
                    f64::from(two.value(ages.local, 0.0) * three.value(ages.local, 0.0)),
                    0.0,
                    0.0,
                ]
            },
        );
        assert!((actual[0] - expected * 100_000.0).abs() < 1e-6);
        assert!(calls.get() < 200);

        two.random_type = 3;
        calls.set(0);
        let actual = particle_vector(1e9, 0, 0, &[(&two, true)], &|_| {
            calls.set(calls.get() + 1);
            [1.0, -2.0, 3.0]
        });
        assert!(calls.get() <= 4096 * 8);
        for (value, expected) in actual.into_iter().zip([1e9, -2e9, 3e9]) {
            assert!((value - expected).abs() < 1e-2);
        }
    }

    #[test]
    fn vector_constant_tail_uses_post_endpoint_values() {
        for (curve, age, expected) in [
            (linear(&[(0, 1.0), (2, 1.0), (2, 3.0)]), 6.0, 14.0),
            (
                linear(&[(-1, 0.25), (0, 1.0)]),
                20_000.0,
                16_383.0 * 0.25 + 3_617.0,
            ),
        ] {
            let value = particle_vector(age, 0, 0, &[(&curve, false)], &|ages| {
                [f64::from(curve.value(ages.local, 0.0)), 0.0, 0.0]
            });
            assert!(
                (value[0] - expected).abs() < 1e-7,
                "{value:?} != {expected}"
            );
        }
    }

    fn main(curve: &AvfxCurve, age: f32, start: i32, end: i32) -> Integral {
        particle_curve(curve, &AvfxCurve::default(), age, start, end, 0)
    }

    fn assert_integral(actual: Integral, duration: f64, value: f64, displacement: f64) {
        for (actual, expected) in [
            (actual.duration, duration),
            (actual.value, value),
            (actual.displacement, displacement),
        ] {
            assert!(
                (actual - expected).abs() <= 1.0e-7 * expected.abs().max(1.0),
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn integrates_linear_rates_and_constant_tails() {
        let curve = linear(&[(0, 0.0), (4, 4.0)]);
        assert_integral(main(&curve, 4.0, 0, 0), 4.0, 8.0, 32.0 / 3.0);
        assert_integral(main(&curve, 6.0, 0, 0), 6.0, 16.0, 104.0 / 3.0);
        let delayed = linear(&[(2, 2.0), (6, 6.0)]);
        assert_integral(main(&delayed, 4.0, 0, 0), 4.0, 10.0, 52.0 / 3.0);
        assert_integral(main(&linear(&[]), 7.0, 0, 0), 7.0, 0.0, 0.0);
        assert_integral(main(&linear(&[(2, -3.0)]), 4.0, 0, 0), 4.0, -12.0, -24.0);
    }

    #[test]
    fn add_integrals_keep_cycle_drift_random_amplitude_and_partial_ranges() {
        let mut data = linear(&[(2, 2.0), (6, 6.0)]);
        data.pre_behavior = crate::avfx::BEHAVIOR_ADD;
        data.post_behavior = crate::avfx::BEHAVIOR_ADD;
        // This Add curve is f(t)=t on both sides of the authored interval.
        for (from, to) in [
            (-17.5, -2.5),
            (-3.5, 19.5),
            (0.0, 12.0),
            (25.5, 43.0),
            (0.0, 1e9),
        ] {
            let duration: f64 = to - from;
            let expected_value = from * duration + duration * duration * 0.5;
            let expected_displacement = from * duration * duration * 0.5 + duration.powi(3) / 6.0;
            assert_integral(
                curve(&data, from, to, &|i| f64::from(data.keys[i].z)),
                duration,
                expected_value,
                expected_displacement,
            );
            for mode in 0..3 {
                data.random_type = mode;
                let coefficient = f64::from(super::super::random_curve_offset(mode, 1.0, 0, 123));
                assert_integral(
                    random_curve(&data, from, to, 123),
                    duration,
                    expected_value * coefficient,
                    expected_displacement * coefficient,
                );
            }
        }
        let mut step = linear(&[(0, 1.0), (2, 3.0)]);
        step.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        step.post_behavior = crate::avfx::BEHAVIOR_ADD;
        // Rates 1,3,5 over successive two-frame intervals.
        assert_integral(main(&step, 6.0, 0, 0), 6.0, 18.0, 38.0);
    }

    #[test]
    fn vector_add_tail_is_not_treated_as_constant_or_periodic() {
        let mut data = linear(&[(0, 0.0), (4, 4.0)]);
        data.post_behavior = crate::avfx::BEHAVIOR_ADD;
        let actual = particle_vector(12.0, 0, 0, &[(&data, false)], &|ages| {
            [f64::from(data.value(ages.total, 0.0)), 0.0, 0.0]
        });
        assert!((actual[0] - 72.0).abs() < 1e-5, "{actual:?}");
        data.keys.push(AvfxCurveKey {
            time: 8,
            z: 0.0,
            ..data.keys[0]
        });
        let calls = std::cell::Cell::new(0);
        let actual = particle_vector(800_000.0, 0, 0, &[(&data, false)], &|ages| {
            calls.set(calls.get() + 1);
            [f64::from(data.value(ages.total, 0.0)), 0.0, 0.0]
        });
        assert!((actual[0] - 1_600_000.0).abs() < 0.1, "{actual:?}");
        assert!(calls.get() < 200);
    }

    #[test]
    fn step_rates_preserve_prior_velocity_at_the_boundary() {
        let mut curve = linear(&[(0, 1.0), (2, 3.0)]);
        curve.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        assert_integral(main(&curve, 2.0, 0, 0), 2.0, 2.0, 2.0);
        assert_integral(main(&curve, 2.5, 0, 0), 2.5, 3.5, 3.375);
        assert_integral(main(&curve, 4.0, 0, 0), 4.0, 8.0, 12.0);
    }

    #[test]
    fn spline_integrals_use_runtime_hermite_tangents() {
        let mut curve = linear(&[(0, 0.0), (10, 1.0)]);
        curve.keys[1].interpolation = 0;
        // x=10u, y=3u^2-2u^3.
        assert_integral(main(&curve, 10.0, 0, 0), 10.0, 5.0, 15.0);
        assert_integral(main(&curve, 5.0, 0, 0), 5.0, 0.9375, 1.25);
        curve.keys[1].time = 8;
        curve.keys[1].z = 8.0;
        curve.keys[0].x = 1.0;
        // t=8u, value=8u+8u^2-8u^3.
        assert_integral(main(&curve, 8.0, 0, 0), 8.0, 112.0 / 3.0, 102.4);
        assert_integral(main(&curve, 4.0, 0, 0), 4.0, 29.0 / 3.0, 188.0 / 15.0);
    }

    #[test]
    fn scalar_random_integral_scales_amplitude_before_accumulation() {
        let mut data = linear(&[(0, 1.0), (2, 4.0), (5, 2.0), (9, 8.0)]);
        data.keys[1].x = 2.0;
        data.keys[1].y = 1.0;
        data.keys[2].interpolation = AvfxCurveKey::INTERPOLATION_SPLINE;
        data.keys[2].x = 1.0;
        for mode in 0..3 {
            data.random_type = mode;
            let unit = AvfxCurve {
                random_type: mode,
                ..linear(&[(0, 1.0)])
            };
            let coefficient = f64::from(super::super::curve_random_value(&unit, 0.0, 123));
            assert_integral(
                random_curve(&data, 2.0, 5.0, 123),
                3.0,
                coefficient * 1149.0 / 140.0,
                coefficient * 9981.0 / 700.0,
            );
        }
    }

    #[test]
    fn spline_integrals_include_neighboring_amplitudes() {
        let mut data = linear(&[(0, 1.0), (2, 4.0), (5, 2.0), (9, 8.0)]);
        data.keys[1].x = 2.0;
        data.keys[1].y = 1.0;
        data.keys[2].interpolation = AvfxCurveKey::INTERPOLATION_SPLINE;
        data.keys[2].x = 1.0;
        let integral = curve(&data, 2.0, 5.0, &|i| f64::from(data.keys[i].z));
        assert_integral(integral, 3.0, 1149.0 / 140.0, 9981.0 / 700.0);
    }

    #[test]
    fn curve_repeat_and_particle_loop_accumulate_instead_of_resetting() {
        let mut curve = linear(&[(2, 0.0), (4, 2.0)]);
        curve.post_behavior = BEHAVIOR_REPEAT;
        assert_integral(main(&curve, 8.0, 0, 0), 8.0, 6.0, 16.0);
        curve.pre_behavior = BEHAVIOR_REPEAT;
        assert_integral(main(&curve, 8.0, 0, 0), 8.0, 8.0, 88.0 / 3.0);

        let curve = linear(&[(0, 0.0), (4, 4.0)]);
        assert_integral(main(&curve, 8.0, 2, 4), 8.0, 20.0, 196.0 / 3.0);
        assert_integral(main(&curve, 4.5, 2, 4), 4.5, 9.125, 14.9375);
        let mut curve = linear(&[(0, 0.0), (4, 4.0)]);
        curve.pre_behavior = BEHAVIOR_REPEAT;
        assert_integral(main(&curve, 2.0, -4, 0), 2.0, 2.0, 4.0 / 3.0);
    }

    #[test]
    fn long_running_repeated_rates_have_closed_form_results() {
        let curve = linear(&[(0, 0.0), (2, 2.0)]);
        let age = 1_000_000_000.0;
        let cycles = f64::from(age) / 2.0;
        assert_integral(
            main(&curve, age, 0, 2),
            f64::from(age),
            2.0 * cycles,
            (4.0 / 3.0) * cycles + 2.0 * cycles * (cycles - 1.0),
        );
    }

    #[test]
    fn seeded_random_rates_integrate_each_frame_and_preserve_loop_phase() {
        let seed = 12345;
        let empty = AvfxCurve::default();
        let mut random = linear(&[(0, 0.8)]);
        for random_type in 0..6 {
            random.random_type = random_type;
            let r = |frame| f64::from(super::super::curve_random_value(&random, frame, seed));
            let (r0, r1, r2) = (r(0.0), r(1.0), r(2.0));
            assert_integral(
                particle_curve(&empty, &random, 2.5, 0, 0, seed),
                2.5,
                r0 + r1 + 0.5 * r2,
                2.0 * r0 + r1 + 0.125 * r2,
            );
            assert_integral(
                particle_curve(&empty, &random, 3.5, 1, 3, seed),
                3.5,
                r0 + 1.5 * r1 + r2,
                3.0 * r0 + 2.125 * r1 + r2,
            );
        }
    }

    #[test]
    fn first_random_amplitude_integrates_its_interpolated_values() {
        let seed = 67890;
        let random = linear(&[(0, 1.0), (4, 2.0)]);
        let a = f64::from(super::super::curve_random_value(&random, 0.0, seed));
        let b = f64::from(super::super::curve_random_value(&random, 4.0, seed));
        assert_integral(
            particle_curve(&AvfxCurve::default(), &random, 4.0, 0, 0, seed),
            4.0,
            2.0 * (a + b),
            16.0 * (2.0 * a + b) / 6.0,
        );
    }

    #[test]
    fn duplicate_time_random_keys_match_instantaneous_sampling() {
        let random = linear(&[(0, 1.0), (0, 2.0)]);
        let expected = f64::from(super::super::curve_random_value(&random, 1.0, 91));
        assert_integral(
            particle_curve(&AvfxCurve::default(), &random, 3.0, 0, 0, 91),
            3.0,
            3.0 * expected,
            4.5 * expected,
        );
    }

    #[test]
    fn packed_key_times_preserve_client_endpoint_order_in_integrals() {
        let wrapped = linear(&[(16384, 0.0), (16388, 4.0)]);
        assert_integral(main(&wrapped, 4.0, 0, 0), 4.0, 8.0, 32.0 / 3.0);
        let descending = linear(&[(-1, 0.25), (0, 1.0)]);
        assert_integral(main(&descending, 8.0, 0, 0), 8.0, 2.0, 8.0);
        assert_integral(
            curve(&descending, 16382.0, 16384.0, &|i| {
                f64::from(descending.keys[i].z)
            }),
            2.0,
            1.25,
            0.875,
        );
    }

    #[test]
    fn scalar_random_unknown_modes_skip_history_without_motion() {
        for mode in [6, 7, 14, 15] {
            let random = AvfxCurve {
                random_type: mode,
                ..linear(&[(0, 1.0), (10, 2.0)])
            };
            assert_integral(random_curve(&random, 0.0, 1e9, 91), 1e9, 0.0, 0.0);
        }
        let first = AvfxCurve {
            random_type: 8,
            ..linear(&[(0, 1.0)])
        };
        let coefficient = f64::from(super::super::curve_random_value(&first, 0.0, 91));
        assert_integral(
            random_curve(&first, 0.0, 1e9, 91),
            1e9,
            coefficient * 1e9,
            coefficient * 5e17,
        );
    }

    #[test]
    fn zero_always_random_rates_skip_empty_history() {
        let mut random = linear(&[(0, 0.0), (10, 0.0)]);
        random.random_type = 3;
        assert_integral(
            particle_curve(&linear(&[(0, 1.0)]), &random, 1e9, 0, 0, 91),
            1e9,
            1e9,
            5e17,
        );
    }

    #[test]
    fn offset_motion_integrates_only_updates_after_the_assigned_age() {
        let data = linear(&[(0, 0.0), (20, 20.0)]);
        assert_integral(
            looped_curve_range(&data, &AvfxCurve::default(), [10.0, 12.0], 0.0, 0.0, 0),
            2.0,
            22.0,
            64.0 / 3.0,
        );
        // Cross the local loop without integrating the skipped prefix.
        assert_integral(
            looped_curve_range(&data, &AvfxCurve::default(), [3.0, 5.0], 2.0, 4.0, 0),
            2.0,
            6.0,
            19.0 / 3.0,
        );
    }

    #[test]
    fn add_integral_uses_total_age_across_particle_local_loops() {
        let mut add = linear(&[(0, 0.0), (4, 4.0)]);
        add.post_behavior = BEHAVIOR_ADD;
        assert_integral(
            looped_curve_range(&add, &AvfxCurve::default(), [0.0, 10.0], 1.0, 5.0, 0),
            10.0,
            50.0,
            500.0 / 3.0,
        );
    }

    #[test]
    fn offset_vector_panels_keep_integer_step_boundaries() {
        let mut data = linear(&[(0, 0.0), (1, 2.0), (2, 2.0)]);
        data.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        let result = looped_vector_range([0.25, 1.5], 0.0, 0.0, &[(&data, false)], &|ages| {
            [f64::from(data.value(ages.local, 0.0)), 0.0, 0.0]
        });
        assert!((result[0] - 1.0).abs() < 1e-12, "{result:?}");
        assert_eq!([result[1], result[2]], [0.0; 2]);
    }
}
