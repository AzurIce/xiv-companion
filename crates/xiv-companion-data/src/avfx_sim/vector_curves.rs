use super::{VfxClientRandomState, VfxClientScalarCurveState, scalar_random};
use crate::avfx::{AvfxCurve3Axis, connect_axes3};

pub(super) fn random_reader_enabled(curve: &AvfxCurve3Axis) -> bool {
    [&curve.random_x, &curve.random_y, &curve.random_z]
        .iter()
        .any(|c| {
            c.as_ref()
                .is_some_and(|c| !c.keys.is_empty() && (c.keys.len() != 1 || c.keys[0].z != 0.0))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey};
    use serde_json::{Value, json};

    fn scalar(mode: u32, present: bool, z: f32) -> AvfxCurve {
        AvfxCurve {
            random_type: mode,
            keys: if present {
                vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z,
                }]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }

    fn curve(
        act: u32,
        actr: u32,
        main: u32,
        random: u32,
        mode: u32,
        recipe: usize,
    ) -> AvfxCurve3Axis {
        let main_values = [-0.375, 0.5, -0.125];
        let amplitudes = [
            [0, 0, 0],
            [0x80000000, 0, 0x80000000],
            [0x3e800000, 0xbf000000, 0x3f400000],
            [0x7f7fffff, 0x7fc00000, 0xff800000],
        ];
        let m = |i: usize| Some(scalar(0, main & (1 << i) != 0, main_values[i]));
        let r = |i: usize| {
            Some(scalar(
                mode,
                random & (1 << i) != 0,
                f32::from_bits(amplitudes[recipe][i]),
            ))
        };
        AvfxCurve3Axis {
            axis_connect: act,
            axis_connect_random: actr,
            x: m(0),
            y: m(1),
            z: m(2),
            random_x: r(0),
            random_y: r(1),
            random_z: r(2),
        }
    }

    #[test]
    fn xyz_single_zero_random_keys_disable_reader_but_still_construct_first() {
        let c = curve(0, 0, 7, 7, 0, 1);
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let mut expected = random;
        for _ in 0..3 {
            expected.next_u16();
        }
        let state = VfxClientVectorCurveState::construct(&c, 4, &mut random).unwrap();
        assert_eq!(state.compiled_header(), 0);
        assert_eq!(random, expected);
        assert_eq!(
            state.evaluate(&c, [0.0; 2], 0.0, [0.75; 3], &mut random),
            [-0.375, 0.5, -0.125]
        );
        assert_eq!(random, expected);
    }

    #[test]
    fn xyz_keyless_authored_rant_collapses_to_resolved_empty_descriptor() {
        let c = curve(0, 0, 7, 0, 4, 2);
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let mut expected = random;
        for _ in 0..3 {
            expected.next_u16();
        }
        VfxClientVectorCurveState::construct(&c, 0, &mut random).unwrap();
        assert_eq!(
            random, expected,
            "authored Always does not survive empty compilation"
        );
    }

    #[test]
    fn xyz_random_connection_copies_empty_fallback_after_other_axes_draw() {
        let c = curve(1, 1, 6, 6, 4, 2);
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let state = VfxClientVectorCurveState::construct(&c, 0, &mut random).unwrap();
        let mut expected = random;
        expected.next_u16();
        expected.next_u16();
        assert_eq!(
            state.evaluate(&c, [0.0; 2], 0.0, [0.75, -0.25, 0.125], &mut random),
            [0.75; 3]
        );
        assert_eq!(
            random, expected,
            "connected-away Y/Z Always draws remain consumed"
        );
        assert_eq!(
            c.evaluate_at(0.0, 0.0, 1.0),
            [1.0; 3],
            "empty main source copies its default"
        );
    }

    #[test]
    fn xyz_connection_aliases_use_low_four_bits_and_reject_invalid_codes_before_drawing() {
        let mut c = curve(17, 0xffffff04, 7, 7, 0, 2);
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let state = VfxClientVectorCurveState::construct(&c, 0, &mut random).unwrap();
        assert_eq!(state.compiled_header(), 2 | 128 | 1);
        assert_eq!(c.evaluate(0.0, 0.0), [-0.375; 3]);
        c.axis_connect_random = 10;
        let before = random;
        assert!(VfxClientVectorCurveState::construct(&c, 0, &mut random).is_none());
        assert_eq!(random, before);
    }

    fn canonical(bits: u32) -> Value {
        if f32::from_bits(bits).is_nan() {
            json!("NaN")
        } else {
            json!(bits)
        }
    }

    #[test]
    #[ignore = "CPU: requires original scalar/XYZ compilers and connected readers"]
    fn compare_original_vector_compilation_and_connected_curves() {
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(folder.join("client-vector-curves-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 204800);
        let mut outputs = 0usize;
        for (ordinal, c) in cases.iter().enumerate() {
            let get = |name: &str| c[name].as_u64().unwrap() as u32;
            let recipe = get("recipe") as usize;
            let curve = curve(
                get("act"),
                get("actr"),
                get("mainMask"),
                get("randomMask"),
                get("mode"),
                recipe,
            );
            let seed = if recipe == 0 {
                [0; 4]
            } else {
                [
                    123456789 + recipe as u32,
                    362436069,
                    521288629 + get("mode"),
                    88675123,
                ]
            };
            let mut random = VfxClientRandomState::from_words(seed);
            let state = VfxClientVectorCurveState::construct(&curve, 0, &mut random).unwrap();
            assert_eq!(
                state.compiled_header(),
                get("header"),
                "header case {ordinal}"
            );
            assert_eq!(
                json!(state.first_percentages()),
                c["first"],
                "First case {ordinal}"
            );
            assert_eq!(
                json!(random.words()),
                c["afterFirst"],
                "First stream case {ordinal}"
            );
            let first_draws = (0..3)
                .filter(|axis| get("randomMask") & (1 << axis) == 0 || get("mode") <= 2)
                .count();
            assert_eq!(first_draws, c["firstDraws"].as_u64().unwrap() as usize);
            for axis in 0..6 {
                let present = if axis < 3 {
                    get("mainMask") & (1 << axis) != 0
                } else {
                    get("randomMask") & (1 << (axis - 3)) != 0
                };
                let header = if present {
                    0x280 | if axis >= 3 { get("mode") << 4 } else { 0 }
                } else {
                    0
                };
                assert_eq!(
                    header,
                    c["scalarHeaders"][axis].as_u64().unwrap() as u32,
                    "scalar header case {ordinal} axis {axis}"
                );
            }
            let fallback = if recipe & 1 == 0 {
                [0.0; 3]
            } else {
                [0.75, -0.25, 0.125]
            };
            for (input, sample) in c["values"].as_array().unwrap().iter().enumerate() {
                let output = state.evaluate(&curve, [2.0; 2], 0.0, fallback, &mut random);
                let expected = sample["output"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| canonical(x.as_u64().unwrap() as u32))
                    .collect::<Vec<_>>();
                assert_eq!(
                    output.map(|f| canonical(f.to_bits())).as_slice(),
                    expected,
                    "case {ordinal} input {input}"
                );
                assert_eq!(
                    json!(random.words()),
                    sample["state"],
                    "stream case {ordinal} input {input}"
                );
                let draws = if state.compiled_header() & 1 != 0 && (3..=5).contains(&get("mode")) {
                    get("randomMask").count_ones()
                } else {
                    0
                };
                assert_eq!(draws, sample["draws"].as_u64().unwrap() as u32);
                if state.compiled_header() & 1 == 0 {
                    assert_eq!(
                        curve
                            .evaluate_at(2.0, 2.0, 0.0)
                            .map(|f| canonical(f.to_bits()))
                            .as_slice(),
                        expected,
                        "general main reader case {ordinal}"
                    );
                }
                outputs += 1;
            }
        }
        for alias in original["aliases"].as_array().unwrap() {
            let curve = curve(
                alias[0].as_u64().unwrap() as u32,
                alias[1].as_u64().unwrap() as u32,
                7,
                7,
                4,
                2,
            );
            let state = VfxClientVectorCurveState::construct(
                &curve,
                0,
                &mut VfxClientRandomState::from_words([0; 4]),
            )
            .unwrap();
            assert_eq!(state.compiled_header(), alias[2].as_u64().unwrap() as u32);
        }
        assert_eq!(outputs, 614400);
        std::fs::write(folder.join("client-vector-curves-rust-comparison.json"),serde_json::to_vec_pretty(&json!({
            "cases":cases.len(),"outputs":outputs,"aliases":25,"differences":0,
            "scope":"Original scalar and XYZ AVFX block compilers, original First and all 10 main/10 random connection readers. All main/random presence masks and eight RanT modes, four recipes with zero/signed zero/nonzero/max finite/Inf/NaN, three repeated reads. Header/scalar flags, First bytes and draw counts, outputs and four RNG words compared; general authored main curve evaluation compared when original compiler disables random. Arena, shared empty descriptor header0, fallback vector, MXCSR/TLS controlled. Raw per-curve blocks compiled; no animated readers, full file/resource loader, live compiled globals, Binder/particle tree or GPU. Finite/Inf/signed zero bits; NaN classification."
        })).unwrap()).unwrap();
    }
}

/// Three owner-local First bytes plus the compiled vector dispatch header.
/// The AVFX compiler collapses keyless scalar blocks to its shared empty
/// descriptor and disables random reads when every random axis is keyless or
/// a single zero-valued key. ACT/ACTR remain independent low-four-bit fields.
#[derive(Clone, Copy, Debug)]
pub struct VfxClientVectorCurveState {
    random: [VfxClientScalarCurveState; 3],
    header: u32,
}

impl VfxClientVectorCurveState {
    /// Compile the known authored vector semantics before constructing First.
    /// Supply the resolved shared empty descriptor's RanT; do not infer it.
    pub fn construct(
        curve: &AvfxCurve3Axis,
        empty_random_type: u32,
        random: &mut VfxClientRandomState,
    ) -> Option<Self> {
        Self::construct_inner(curve, empty_random_type, random, false)
    }

    /// Adapter for already-compiled controlled descriptors used by the older
    /// Binder probes. Those probes explicitly retain keyless descriptor RanT
    /// and set random-present on any key, bypassing the authored compiler.
    pub(super) fn construct_compiled(
        curve: &AvfxCurve3Axis,
        empty_random_type: u32,
        random: &mut VfxClientRandomState,
    ) -> Option<Self> {
        Self::construct_inner(curve, empty_random_type, random, true)
    }

    fn construct_inner(
        curve: &AvfxCurve3Axis,
        empty_random_type: u32,
        random: &mut VfxClientRandomState,
        compiled: bool,
    ) -> Option<Self> {
        if curve.axis_connect & 15 > 9 || curve.axis_connect_random & 15 > 9 {
            return None;
        }
        let curves = [&curve.random_x, &curve.random_y, &curve.random_z];
        let random_present = if compiled {
            curves
                .iter()
                .any(|c| c.as_ref().is_some_and(|c| !c.keys.is_empty()))
        } else {
            random_reader_enabled(curve)
        };
        Some(Self {
            header: (curve.axis_connect & 15) << 1
                | (curve.axis_connect_random & 15) << 5
                | u32::from(random_present),
            random: curves.map(|c| {
                VfxClientScalarCurveState::construct(
                    c.as_ref()
                        .filter(|c| compiled || !c.keys.is_empty())
                        .map_or(empty_random_type, |c| c.random_type),
                    random,
                )
            }),
        })
    }

    pub fn compiled_header(self) -> u32 {
        self.header
    }

    pub fn first_percentages(self) -> [i8; 3] {
        self.random.map(VfxClientScalarCurveState::first_percentage)
    }

    /// All random axes read/draw before ACTR copies a source axis. Even axes
    /// subsequently overwritten consume Always draws. A missing/disabled
    /// source copies its resolved fallback; connections do not require keys.
    pub fn evaluate(
        self,
        curve: &AvfxCurve3Axis,
        ages: [f32; 2],
        main_default: f32,
        random_fallback: [f32; 3],
        random: &mut VfxClientRandomState,
    ) -> [f32; 3] {
        let mut base = [&curve.x, &curve.y, &curve.z].map(|c| {
            c.as_ref()
                .map_or(main_default, |c| c.value_at(ages[0], ages[1], main_default))
        });
        connect_axes3((self.header >> 1) & 15, &mut base);
        if self.header & 1 == 0 {
            return base;
        }
        let mut offset = random_fallback;
        for (axis, curve) in [&curve.random_x, &curve.random_y, &curve.random_z]
            .into_iter()
            .enumerate()
        {
            let Some(curve) = curve.as_ref().filter(|c| !c.keys.is_empty()) else {
                continue;
            };
            let mode = curve.random_type & 7;
            if mode > 5 {
                continue;
            }
            let draw = if mode >= 3 { random.next_u16() } else { 0 };
            offset[axis] = scalar_random::cached_offset(
                mode,
                curve.value_at(ages[0], ages[1], 0.0),
                self.random[axis].first_percentage(),
                draw,
            );
        }
        connect_axes3((self.header >> 5) & 15, &mut offset);
        std::array::from_fn(|axis| offset[axis] + base[axis])
    }
}
