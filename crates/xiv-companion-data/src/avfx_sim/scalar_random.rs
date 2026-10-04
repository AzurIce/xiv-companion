use super::VfxClientRandomState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum PairDispatch {
    #[default]
    Lazy,
    Main,
    Random,
    Pair,
    Empty,
}

/// Dispatch cache owned by the compiled curve pair, shared by its instances.
/// The first call fills the cache but still executes the full pair evaluator;
/// later calls enter the selected main/random/pair/empty reader directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VfxClientScalarPairState {
    dispatch: PairDispatch,
}

impl VfxClientScalarPairState {
    pub fn dispatch_code(self) -> u8 {
        match self.dispatch {
            PairDispatch::Lazy => 0,
            PairDispatch::Main => 1,
            PairDispatch::Random => 2,
            PairDispatch::Pair => 3,
            PairDispatch::Empty => 4,
        }
    }

    fn values(&mut self, main: Option<f32>, amplitude: Option<f32>) -> (Option<f32>, Option<f32>) {
        match self.dispatch {
            PairDispatch::Lazy => {
                self.dispatch = match (main.is_some(), amplitude.is_some()) {
                    (true, true) => PairDispatch::Pair,
                    (true, false) => PairDispatch::Main,
                    (false, true) => PairDispatch::Random,
                    (false, false) => PairDispatch::Empty,
                };
                (Some(main.unwrap_or(0.0)), Some(amplitude.unwrap_or(0.0)))
            }
            PairDispatch::Main => (main, None),
            PairDispatch::Random => (None, amplitude),
            PairDispatch::Pair => (main, amplitude),
            PairDispatch::Empty => (None, None),
        }
    }
}

/// A compiled scalar pair's random mode and constructor-owned First byte.
/// Share the caller's explicit stream with other client consumers. This core
/// does not infer the live TLS seed or the order of all Binder/particle calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VfxClientScalarCurveState {
    random_type: u8,
    first_percentage: i8,
}

impl VfxClientScalarCurveState {
    /// Original 396fe0 consumes one draw for First (modes 0..2), even when
    /// the compiled random descriptor has no keys or its amplitude is zero.
    /// Always (3..5) and disabled modes (6/7) consume no constructor draw.
    pub fn construct(random_type: u32, random: &mut VfxClientRandomState) -> Self {
        Self::construct_with(random_type, &mut || random.next_u16())
    }

    fn construct_with(random_type: u32, draw: &mut impl FnMut() -> u16) -> Self {
        let mode = (random_type & 7) as u8;
        Self {
            random_type: mode,
            first_percentage: if mode <= 2 {
                first_percentage(u32::from(mode), draw())
            } else {
                0
            },
        }
    }

    pub fn first_percentage(self) -> i8 {
        self.first_percentage
    }

    /// Evaluate already-read main/amplitude values using the compiled pair's
    /// shared dispatch cache. None means no keys; Some(0) is active. The
    /// lazy first call always runs both readers with +0 defaults. Subsequent
    /// Always calls draw only when the cached random reader is active, even
    /// at zero amplitude or the same age. Keep key presence stable for the
    /// lifetime of this compiled pair, as in the original client.
    pub fn evaluate(
        self,
        pair: &mut VfxClientScalarPairState,
        main: Option<f32>,
        amplitude: Option<f32>,
        random: &mut VfxClientRandomState,
    ) -> f32 {
        self.evaluate_with(pair, main, amplitude, &mut || random.next_u16())
    }

    fn evaluate_with(
        self,
        pair: &mut VfxClientScalarPairState,
        main: Option<f32>,
        amplitude: Option<f32>,
        draw: &mut impl FnMut() -> u16,
    ) -> f32 {
        let (main, amplitude) = pair.values(main, amplitude);
        let mode = u32::from(self.random_type);
        let Some(amplitude) = amplitude.filter(|_| mode < 6) else {
            return main.unwrap_or(0.0);
        };
        let draw = if mode >= 3 { draw() } else { 0 };
        let offset = cached_offset(mode, amplitude, self.first_percentage, draw);
        // Native random-only dispatch does not add a synthetic +0 main.
        main.map_or(offset, |base| offset + base)
    }
}

// Client 396fe0 stores First as a signed percentage. Keep the instruction
// order; factoring division/multiplication changes boundary rounding.
fn first_percentage(random_type: u32, draw: u16) -> i8 {
    let draw = f32::from(draw);
    let fraction = match random_type & 7 {
        0 => (draw + draw) / 65535.0 - 1.0,
        1 => draw / 65535.0 + 0.0,
        2 => draw / 65535.0 - 1.0,
        _ => return 0,
    };
    (fraction * 100.0) as i8
}

pub(super) fn cached_offset(random_type: u32, amplitude: f32, first: i8, draw: u16) -> f32 {
    let draw = f32::from(draw);
    match random_type & 7 {
        0..=2 => (f32::from(first) * 0.01) * amplitude,
        3 => (amplitude - -amplitude) * draw / 65535.0 + -amplitude,
        4 => (amplitude - 0.0) * draw / 65535.0 + 0.0,
        5 => (0.0 - -amplitude) * draw / 65535.0 + -amplitude,
        _ => 0.0,
    }
}

pub(super) fn offset(random_type: u32, amplitude: f32, draw: u16) -> f32 {
    cached_offset(
        random_type,
        amplitude,
        first_percentage(random_type, draw),
        draw,
    )
}

/// Cached numerical pair dispatch for the legacy seeded preview. That
/// stateless preview has no compiled lazy-call history or client TLS stream.
pub(super) fn pair(random_type: u32, main: Option<f32>, amplitude: Option<f32>, draw: u16) -> f32 {
    let Some(amplitude) = amplitude.filter(|_| random_type & 7 < 6) else {
        return main.unwrap_or(0.0);
    };
    let offset = offset(random_type, amplitude, draw);
    main.map_or(offset, |base| offset + base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_first_constructor_consumes_empty_descriptor_and_reuses_byte() {
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let initial = random;
        let first = VfxClientScalarCurveState::construct(0, &mut random);
        assert_eq!(first.first_percentage(), -93);
        assert_ne!(random, initial);
        let constructed = random;
        let mut empty = VfxClientScalarPairState::default();
        assert_eq!(
            first
                .evaluate(&mut empty, None, None, &mut random)
                .to_bits(),
            0
        );
        assert_eq!(empty.dispatch_code(), 4);
        let mut active = VfxClientScalarPairState::default();
        assert_eq!(
            first.evaluate(&mut active, None, Some(2.0), &mut random),
            -0.93 * 2.0
        );
        assert_eq!(
            first.evaluate(&mut active, None, Some(4.0), &mut random),
            -0.93 * 4.0
        );
        assert_eq!(random, constructed);
    }

    #[test]
    fn client_always_empty_draws_once_then_cached_zero_amplitude_draws_each_call() {
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let initial = random;
        let always = VfxClientScalarCurveState::construct(4, &mut random);
        assert_eq!(random, initial);
        let mut expected = initial;
        let mut empty_random = VfxClientScalarPairState::default();
        always.evaluate(&mut empty_random, Some(-0.0), None, &mut random);
        expected.next_u16();
        assert_eq!(random, expected);
        assert_eq!(
            always
                .evaluate(&mut empty_random, Some(-0.0), None, &mut random)
                .to_bits(),
            0x8000_0000
        );
        assert_eq!(random, expected);
        // A second owner shares the compiled cache and must not run the
        // lazy draw again. Its First byte is a separate per-owner field.
        let second_owner = VfxClientScalarCurveState::construct(4, &mut random);
        second_owner.evaluate(&mut empty_random, Some(-0.0), None, &mut random);
        assert_eq!(random, expected);
        let mut active = VfxClientScalarPairState::default();
        for _ in 0..3 {
            assert_eq!(
                always
                    .evaluate(&mut active, None, Some(-0.0), &mut random)
                    .to_bits(),
                0
            );
            expected.next_u16();
            assert_eq!(random, expected);
        }
        let disabled = VfxClientScalarCurveState::construct(14, &mut random);
        let mut disabled_pair = VfxClientScalarPairState::default();
        assert_eq!(
            disabled
                .evaluate(&mut disabled_pair, Some(-0.0), Some(1.0), &mut random)
                .to_bits(),
            0x8000_0000
        );
        assert_eq!(random, expected);
    }

    #[test]
    fn client_pair_preserves_dispatch_and_explicit_zero_arithmetic() {
        assert_eq!(offset(4, -0.0, 1).to_bits(), 0);
        assert_eq!(pair(0, None, Some(0.0), 0).to_bits(), 0x8000_0000);
        assert_eq!(pair(0, Some(0.0), Some(0.0), 0).to_bits(), 0);
        assert_eq!(pair(4, Some(-0.0), None, 1).to_bits(), 0x8000_0000);
        assert_eq!(
            pair(7, Some(-0.0), Some(f32::NAN), 1).to_bits(),
            0x8000_0000
        );
        let mut random = VfxClientRandomState::from_words([0; 4]);
        let first = VfxClientScalarCurveState::construct(0, &mut random);
        let mut compiled = VfxClientScalarPairState::default();
        assert_eq!(
            first
                .evaluate(&mut compiled, None, Some(0.0), &mut random)
                .to_bits(),
            0
        );
        assert_eq!(compiled.dispatch_code(), 2);
        assert_eq!(
            first
                .evaluate(&mut compiled, None, Some(0.0), &mut random)
                .to_bits(),
            0x8000_0000
        );
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        cases: Vec<Case>,
        first_draws: Vec<[i32; 4]>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        mode: u32,
        main_present: bool,
        random_present: bool,
        main: u32,
        amplitude: u32,
        seed: [u32; 4],
        first: i8,
        after_first: [u32; 4],
        first_tls_calls: usize,
        values: Vec<Sample>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Sample {
        value: u32,
        dispatch: u8,
        tls_calls: usize,
        state: [u32; 4],
    }

    fn same_value(actual: f32, expected: u32) -> bool {
        if f32::from_bits(expected).is_nan() {
            actual.is_nan()
        } else {
            actual.to_bits() == expected
        }
    }

    #[test]
    #[ignore = "CPU: original scalar constructor/dispatch/readers/TLS draws, including every First WORD input"]
    fn compare_original_client_scalar_random_with_shared_state() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let probe: Probe = serde_json::from_slice(
            &std::fs::read(folder.join("client-scalar-random-client-probe.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(probe.cases.len(), 18432);
        assert_eq!(probe.first_draws.len(), 3 * 65536);
        for (ordinal, &[mode, draw, expected, calls]) in probe.first_draws.iter().enumerate() {
            assert_eq!(ordinal, mode as usize * 65536 + draw as usize);
            let mut random = VfxClientRandomState::from_words([0, 0, 0, draw as u32]);
            let first = VfxClientScalarCurveState::construct(mode as u32, &mut random);
            assert_eq!(
                first.first_percentage(),
                expected as i8,
                "mode={mode} draw={draw}"
            );
            assert_eq!(random.words(), [0, 0, draw as u32, draw as u32]);
            assert_eq!(calls, 1);
        }
        let mut outputs = 0;
        let mut nan_outputs = 0;
        let mut first_calls = 0;
        let mut always_calls = 0;
        let mut legacy_differences = 0;
        for (ordinal, case) in probe.cases.iter().enumerate() {
            let main = case.main_present.then(|| f32::from_bits(case.main));
            let amplitude = case.random_present.then(|| f32::from_bits(case.amplitude));
            let mut random = VfxClientRandomState::from_words(case.seed);
            let first = VfxClientScalarCurveState::construct(case.mode, &mut random);
            assert_eq!(first.first_percentage(), case.first, "case {ordinal}");
            assert_eq!(random.words(), case.after_first, "case {ordinal}");
            let mut counted_random = VfxClientRandomState::from_words(case.seed);
            let mut calls = 0;
            let counted = VfxClientScalarCurveState::construct_with(case.mode, &mut || {
                calls += 1;
                counted_random.next_u16()
            });
            assert_eq!(counted, first);
            assert_eq!(
                calls, case.first_tls_calls,
                "case {ordinal} constructor calls"
            );
            assert_eq!(counted_random, random);
            first_calls += calls;
            assert_eq!(case.values.len(), 3);
            let mut compiled = VfxClientScalarPairState::default();
            let mut counted_compiled = VfxClientScalarPairState::default();
            for (tick, sample) in case.values.iter().enumerate() {
                let actual = first.evaluate(&mut compiled, main, amplitude, &mut random);
                assert!(
                    same_value(actual, sample.value),
                    "case {ordinal} tick {tick} mode {} main {:08x} amp {:08x}: {:08x} != {:08x}",
                    case.mode,
                    case.main,
                    case.amplitude,
                    actual.to_bits(),
                    sample.value
                );
                assert_eq!(random.words(), sample.state, "case {ordinal} tick {tick}");
                calls = 0;
                assert_eq!(compiled.dispatch_code(), sample.dispatch);
                let counted_value =
                    counted.evaluate_with(&mut counted_compiled, main, amplitude, &mut || {
                        calls += 1;
                        counted_random.next_u16()
                    });
                assert!(same_value(counted_value, sample.value));
                assert_eq!(counted_random, random);
                assert_eq!(counted_compiled, compiled);
                assert_eq!(
                    calls, sample.tls_calls,
                    "case {ordinal} tick {tick} evaluator calls"
                );
                always_calls += calls;

                // Feed the captured producer value to the same numerical core
                // used by legacy preview/Cone, without claiming its SplitMix
                // source is the client's stream.
                let draw = match case.mode & 7 {
                    0..=2 => case.after_first[3] as u16,
                    3..=5 => sample.state[3] as u16,
                    _ => 0,
                };
                let (kernel_main, kernel_amplitude) = if tick == 0 {
                    (Some(main.unwrap_or(0.0)), Some(amplitude.unwrap_or(0.0)))
                } else {
                    (main, amplitude)
                };
                assert!(same_value(
                    pair(case.mode, kernel_main, kernel_amplitude, draw),
                    sample.value
                ));

                // Previous implementation always added a +0 main and skipped
                // the explicit +0 in positive Always. Count observed errors,
                // excluding NaN payload/sign which is not the oracle's claim.
                let old_offset = match (case.mode & 7, amplitude) {
                    (0..=2, Some(amp)) => (f32::from(case.first) * 0.01) * amp,
                    (3, Some(amp)) => (amp - -amp) * f32::from(draw) / 65535.0 - amp,
                    (4, Some(amp)) => amp * f32::from(draw) / 65535.0,
                    (5, Some(amp)) => amp * f32::from(draw) / 65535.0 - amp,
                    _ => 0.0,
                };
                if !same_value(main.unwrap_or(0.0) + old_offset, sample.value) {
                    legacy_differences += 1;
                }
                outputs += 1;
                nan_outputs += usize::from(actual.is_nan());
            }
        }
        assert_eq!(outputs, 55296);
        assert!(legacy_differences > 0);
        std::fs::write(folder.join("client-scalar-random-rust-comparison.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "cases": probe.cases.len(), "outputsCompared": outputs,
                "nanClassOutputs": nan_outputs, "firstProducerInputsCompared": probe.first_draws.len(),
                "constructorDrawsInScenes": first_calls, "evaluationDrawsInScenes": always_calls,
                "previousNumericalDifferences": legacy_differences, "differences": 0,
                "scope": "Original signed-byte constructor, lazy/shared cached pair dispatch, constant/empty readers and inline TLS xorshift versus production explicit-state scalar cache and legacy numerical core with captured draws. Raw modes 0..15, all four presence shapes, four TLS snapshots, signed zero/finite extrema/Inf/NaN amplitudes, three same-age calls. All 65536 WORD inputs for each First mode. Finite/Inf/signed-zero bits, NaN classification, First byte, dispatch code, draw counts and four RNG words compared. Controlled compiled descriptors and TLS getter; no full parsing, Binder constructor call order/default BSS descriptor initialization, animated readers, global consumers, live host or GPU."
            })).unwrap() + "\n").unwrap();
    }

    #[test]
    fn client_percentage_quantization_and_low_mode_bits() {
        for (mode, draw, expected) in [
            (0, 49151, 49.0 * 0.01),
            (1, 32767, 49.0 * 0.01),
            (2, 32768, -49.0 * 0.01),
            (0, 0, -1.0),
            (0, u16::MAX, 1.0),
            (1, u16::MAX, 1.0),
            (2, u16::MAX, 0.0),
            (3, 0, -1.0),
            (4, u16::MAX, 1.0),
            (5, 0, -1.0),
            (6, 12345, 0.0),
            (7, 12345, 0.0),
        ] {
            for high_bits in [0, 8, 0xffff_fff8] {
                assert_eq!(offset(mode | high_bits, 1.0, draw), expected);
                assert_eq!(offset(mode | high_bits, -2.0, draw), -2.0 * expected);
            }
        }
    }
}
