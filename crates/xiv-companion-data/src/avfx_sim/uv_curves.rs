//! Authored UvSt compiler/First/read order, 1403a4870/1403e4070/1403a4da0.
//! Separate XY readers use zero defaults and byte ACT/ACTR, not XYZ flags.
//! Connected to ModelSkin's successful Numeric stage; other particle UV
//! consumers and the original UV matrix assembly remain separate work.
use super::{
    VfxClientRandomState, VfxClientScalarCurveState, VfxClientScalarPairState, scalar_random,
};
use crate::avfx::{AvfxCurve, AvfxCurve2Axis, AvfxUvSet};

#[derive(Clone, Copy, Debug)]
struct XyState {
    first: [VfxClientScalarCurveState; 2],
    random_enabled: bool,
}

fn random_type(curve: Option<&AvfxCurve>, empty: u32) -> u32 {
    curve
        .filter(|c| !c.keys.is_empty())
        .map_or(empty, |c| c.random_type)
}

fn constant_or_empty(curve: Option<&AvfxCurve>, value: f32) -> bool {
    curve.is_none_or(|c| c.keys.is_empty() || (c.keys.len() == 1 && c.keys[0].z == value))
}

impl XyState {
    fn construct(
        c: &AvfxCurve2Axis,
        empty: u32,
        random: &mut VfxClientRandomState,
    ) -> Result<Self, String> {
        if c.axis_connect as u8 > 2 {
            return Err("UvSt requires valid byte XY ACT".into());
        }
        Ok(Self {
            first: [&c.random_x, &c.random_y].map(|c| {
                VfxClientScalarCurveState::construct(random_type(c.as_ref(), empty), random)
            }),
            random_enabled: !constant_or_empty(c.random_x.as_ref(), 0.0)
                || !constant_or_empty(c.random_y.as_ref(), 0.0),
        })
    }
    fn evaluate(
        self,
        c: &AvfxCurve2Axis,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) -> [f32; 2] {
        let mut main = [&c.x, &c.y].map(|c| {
            c.as_ref()
                .map_or(0.0, |c| c.value_at(ages[0], ages[1], 0.0))
        });
        connect(c.axis_connect as u8, &mut main);
        if !self.random_enabled {
            return main;
        }
        let mut offset = [0.0; 2];
        for (i, c) in [&c.random_x, &c.random_y].into_iter().enumerate() {
            if let Some(c) = c.as_ref().filter(|c| !c.keys.is_empty()) {
                let mode = c.random_type & 7;
                if mode <= 5 {
                    let draw = if mode >= 3 { random.next_u16() } else { 0 };
                    offset[i] = scalar_random::cached_offset(
                        mode,
                        c.value_at(ages[0], ages[1], 0.0),
                        self.first[i].first_percentage(),
                        draw,
                    );
                }
            }
        }
        connect(c.axis_connect_random as u8, &mut offset);
        std::array::from_fn(|i| offset[i] + main[i])
    }
}

fn connect(mode: u8, axes: &mut [f32; 2]) {
    match mode {
        1 => axes[1] = axes[0],
        2 => axes[0] = axes[1],
        _ => {}
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientUvCurveCache {
    pub scale: [f32; 2],
    pub scroll: [f32; 2],
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientUvCurveState {
    scale: XyState,
    scroll: XyState,
    rotation: VfxClientScalarCurveState,
    enabled: bool,
    pub cache: Option<VfxClientUvCurveCache>,
}

impl VfxClientUvCurveState {
    #[cfg(test)]
    pub(super) fn first_percentages(self) -> [i8; 5] {
        [
            self.scale.first[0],
            self.scale.first[1],
            self.scroll.first[0],
            self.scroll.first[1],
            self.rotation,
        ]
        .map(VfxClientScalarCurveState::first_percentage)
    }
    pub(super) fn construct(
        set: &AvfxUvSet,
        empty: u32,
        random: &mut VfxClientRandomState,
    ) -> Result<Self, String> {
        // Validate both before any First draw, including inactive UV sets.
        if set.scale.axis_connect as u8 > 2 || set.scroll.axis_connect as u8 > 2 {
            return Err("UvSt requires valid byte XY ACT".into());
        }
        let scale = XyState::construct(&set.scale, empty, random)?;
        let scroll = XyState::construct(&set.scroll, empty, random)?;
        let rotation = VfxClientScalarCurveState::construct(
            random_type(Some(&set.rotation_random), empty),
            random,
        );
        // The original UV compiler tests BOTH Scl/Scr main channels and
        // Rot/RotR against one; Scl/Scr random channels against zero.
        // Preserve this unusual original optimization rather than editor defaults.
        let enabled = ![
            constant_or_empty(set.scale.x.as_ref(), 1.0),
            constant_or_empty(set.scale.y.as_ref(), 1.0),
            constant_or_empty(set.scale.random_x.as_ref(), 0.0),
            constant_or_empty(set.scale.random_y.as_ref(), 0.0),
            constant_or_empty(set.scroll.x.as_ref(), 1.0),
            constant_or_empty(set.scroll.y.as_ref(), 1.0),
            constant_or_empty(set.scroll.random_x.as_ref(), 0.0),
            constant_or_empty(set.scroll.random_y.as_ref(), 0.0),
            constant_or_empty(Some(&set.rotation), 1.0),
            constant_or_empty(Some(&set.rotation_random), 1.0),
        ]
        .into_iter()
        .all(|v| v);
        Ok(Self {
            scale,
            scroll,
            rotation,
            enabled,
            cache: None,
        })
    }
    pub(super) fn evaluate(
        &mut self,
        set: &AvfxUvSet,
        pair: &mut VfxClientScalarPairState,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) {
        self.cache = Some(if self.enabled {
            VfxClientUvCurveCache {
                scale: self.scale.evaluate(&set.scale, ages, random),
                scroll: self.scroll.evaluate(&set.scroll, ages, random),
                rotation: self.rotation.evaluate(
                    pair,
                    (!set.rotation.keys.is_empty())
                        .then(|| set.rotation.value_at(ages[0], ages[1], 0.0)),
                    (!set.rotation_random.keys.is_empty())
                        .then(|| set.rotation_random.value_at(ages[0], ages[1], 0.0)),
                    random,
                ),
            }
        } else {
            VfxClientUvCurveCache {
                scale: [1.0; 2],
                scroll: [0.0; 2],
                rotation: 0.0,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::AvfxCurveKey;

    fn scalar(value: f32, mode: u32) -> AvfxCurve {
        AvfxCurve {
            random_type: mode,
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

    #[test]
    fn uv_first_constructs_five_values_even_for_disabled_set_and_numeric_reads_do_not_draw() {
        let set = AvfxUvSet::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        let mut expected = random;
        for _ in 0..5 {
            expected.next_u16();
        }
        let mut state = VfxClientUvCurveState::construct(&set, 0, &mut random).unwrap();
        assert_eq!(random, expected);
        let mut pair = VfxClientScalarPairState::default();
        state.evaluate(&set, &mut pair, [2.0, 3.0], &mut random);
        let cache = state.cache.unwrap();
        assert_eq!(cache.scale, [1.0; 2]);
        assert_eq!(cache.scroll, [0.0; 2]);
        assert_eq!(cache.rotation, 0.0);
        assert_eq!(random, expected);
    }

    #[test]
    fn active_uv_missing_scale_axis_is_zero_and_byte_connections_copy_empty_source() {
        let set = AvfxUvSet {
            scale: AvfxCurve2Axis {
                x: Some(scalar(2.0, 0)),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let mut state = VfxClientUvCurveState::construct(&set, 0, &mut random).unwrap();
        state.evaluate(
            &set,
            &mut VfxClientScalarPairState::default(),
            [0.0; 2],
            &mut random,
        );
        assert_eq!(state.cache.unwrap().scale, [2.0, 0.0]);
        let mut connected = set.clone();
        connected.scale.axis_connect = 0x102;
        let mut state = VfxClientUvCurveState::construct(&connected, 0, &mut random).unwrap();
        state.evaluate(
            &connected,
            &mut VfxClientScalarPairState::default(),
            [0.0; 2],
            &mut random,
        );
        assert_eq!(state.cache.unwrap().scale, [0.0; 2]);
    }

    #[test]
    #[ignore = "compare original UvSt compilation, whole-set First and XY/scalar readers"]
    fn compare_original_modelskin_uv_compilation_first_and_readers() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("modelskin-uv-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut outputs = 0usize;
        let mut coverage = [[[0usize; 8]; 3]; 3];
        for (index, case) in input["cases"].as_array().unwrap().iter().enumerate() {
            let mode = case["mode"].as_u64().unwrap() as u32;
            let mask = case["mask"].as_u64().unwrap() as usize;
            let act = case["act"].as_u64().unwrap() as u32;
            let actr = case["actr"].as_u64().unwrap() as u32;
            coverage[act as usize][actr as usize][mode as usize] += 1;
            let curve = |i: usize| {
                (mask & (1 << i) != 0).then(|| {
                    scalar(
                        f32::from_bits(case["fields"][i].as_u64().unwrap() as u32),
                        if i == 8 { 0 } else { mode },
                    )
                })
            };
            let xy = |start| AvfxCurve2Axis {
                axis_connect: act,
                axis_connect_random: actr,
                x: curve(start),
                y: curve(start + 1),
                random_x: curve(start + 2),
                random_y: curve(start + 3),
            };
            let set = AvfxUvSet {
                calculate_uv: case["cuvt"].as_u64().unwrap() as u32,
                scale: xy(0),
                scroll: xy(4),
                rotation: curve(8).unwrap_or_default(),
                rotation_random: curve(9).unwrap_or_default(),
            };
            let mut random = VfxClientRandomState::from_words(std::array::from_fn(|i| {
                case["seed"][i].as_u64().unwrap() as u32
            }));
            let mut state = VfxClientUvCurveState::construct(&set, 0, &mut random).unwrap();
            assert_eq!(
                u64::from(state.enabled),
                case["enabled"].as_u64().unwrap(),
                "enabled case {index}"
            );
            let first = [
                state.scale.first[0],
                state.scale.first[1],
                state.scroll.first[0],
                state.scroll.first[1],
                state.rotation,
            ]
            .map(VfxClientScalarCurveState::first_percentage);
            assert_eq!(
                serde_json::json!(first),
                case["first"],
                "First case {index}"
            );
            assert_eq!(
                serde_json::json!(random.words()),
                case["afterFirst"],
                "First stream case {index}"
            );
            let mut pair = VfxClientScalarPairState::default();
            for (read, sample) in case["values"].as_array().unwrap().iter().enumerate() {
                state.evaluate(&set, &mut pair, [2.0, 3.0], &mut random);
                let cache = state.cache.unwrap();
                let actual = [
                    cache.scale[0],
                    cache.scale[1],
                    cache.scroll[0],
                    cache.scroll[1],
                    cache.rotation,
                ];
                for axis in 0..5 {
                    assert_eq!(
                        u64::from(actual[axis].to_bits()),
                        sample["output"][axis].as_u64().unwrap(),
                        "case {index} read {read} axis {axis}"
                    );
                    outputs += 1;
                }
                assert_eq!(
                    serde_json::json!(random.words()),
                    sample["state"],
                    "numeric stream case {index} read {read}"
                );
                assert_eq!(
                    u64::from(pair.dispatch_code()),
                    sample["dispatch"].as_u64().unwrap(),
                    "pair case {index} read {read}"
                );
            }
        }
        assert_eq!(coverage, [[[64; 8]; 3]; 3]);
        std::fs::write(folder.join("modelskin-uv-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({"cases":input["count"],"outputsCompared":outputs,"differences":0,"scope":"Original raw scalar/UvSt compilation, complete 3e4070 five-First initialization and 3a4ee0 UV dispatch versus production UV state; byte ACT/ACTR 0..2, RanT0..7, 32 constant/keyless/signed-zero/presence recipes, CUvT0/1, three repeated numeric reads. Controlled empty RanT0, incoming curve ages/TLS. No animated original reader, full ModelSkin constructor/phase, matrix assembly, GPU or client pixels."})).unwrap()+"\n").unwrap();
    }
}
