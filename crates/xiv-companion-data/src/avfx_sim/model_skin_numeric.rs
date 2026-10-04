//! ModelSkin 140400a80 axis arithmetic. Keep the original scalar operation
//! order: reducing its zero products changes signed zero and finite rounding.
//! General CRT large-argument reduction is still outside the verified domain.

use super::{VfxClientTrigMode, client_trig};

/// 14040182a..140401997: S * R * T through two complete 37be80 affine
/// multiplies. Keep zero products and the column-specific add order.
pub(super) fn uv_rows(
    scale: [f32; 2],
    scroll: [f32; 2],
    angle: f32,
    trig: Option<VfxClientTrigMode>,
) -> [[f32; 4]; 2] {
    let [cos, sin] = match trig.filter(|_| angle.is_finite() && angle.abs() <= std::f32::consts::PI)
    {
        Some(trig) => client_trig::spline_transverse(angle, trig),
        None => {
            let (sin, cos) = angle.sin_cos();
            [cos, sin]
        }
    };
    let scale = [
        scale[0], 0.0, 0.0, 0.0, scale[1], 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
    ];
    let rotation = [cos, sin, 0.0, -sin, cos, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    let translation = [
        1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, scroll[0], scroll[1], 0.0,
    ];
    let matrix = affine_multiply(affine_multiply(scale, rotation), translation);
    [
        [matrix[0], matrix[3], matrix[9], 0.0],
        [matrix[1], matrix[4], matrix[10], 0.0],
    ]
}

fn affine_multiply(a: [f32; 12], b: [f32; 12]) -> [f32; 12] {
    [
        (a[0] * b[0] + b[3] * a[1]) + b[6] * a[2],
        (b[4] * a[1] + b[1] * a[0]) + b[7] * a[2],
        (b[5] * a[1] + b[2] * a[0]) + b[8] * a[2],
        (a[3] * b[0] + a[4] * b[3]) + a[5] * b[6],
        (a[4] * b[4] + a[3] * b[1]) + a[5] * b[7],
        (a[4] * b[5] + a[3] * b[2]) + a[5] * b[8],
        (b[3] * a[7] + b[0] * a[6]) + b[6] * a[8],
        (b[4] * a[7] + b[1] * a[6]) + b[7] * a[8],
        (b[5] * a[7] + b[2] * a[6]) + b[8] * a[8],
        ((a[9] * b[0] + a[10] * b[3]) + a[11] * b[6]) + b[9],
        ((a[10] * b[4] + a[9] * b[1]) + a[11] * b[7]) + b[10],
        ((a[10] * b[5] + a[9] * b[2]) + a[11] * b[8]) + b[11],
    ]
}

pub(super) fn fresnel_axis(
    mode: i32,
    rotation: [f32; 3],
    trig: Option<VfxClientTrigMode>,
) -> [f32; 3] {
    let [x, y, z] = rotation.map(|angle| {
        match trig.filter(|_| angle.is_finite() && angle.abs() <= std::f32::consts::PI) {
            Some(trig) => client_trig::spline_transverse(angle, trig),
            None => {
                let (sin, cos) = angle.sin_cos();
                [cos, sin]
            }
        }
    });
    let (initial_y, initial_x) = match mode {
        2 => (-x[0], -x[1]),
        3 => (x[1], -x[0]),
        _ => return [0.0; 3],
    };
    // The original reference vector's remaining lane is negative zero.
    let zero = 0.0;
    let negative_zero = -0.0;
    let yz = (initial_y * zero - y[1] * negative_zero) + initial_x * y[0];
    let yy = (negative_zero * zero + initial_y) + initial_x * zero;
    let yx = (y[0] * negative_zero + initial_y * zero) + initial_x * y[1];
    let axis = [
        (yx * z[0] - yy * z[1]) + yz * zero,
        (yy * z[0] + yx * z[1]) + yz * zero,
        (yx * zero + yy * zero) + yz,
    ];
    if mode == 2 {
        normalize_axis(axis)
    } else {
        axis
    }
}

/// 140400c9b/140400e65: accumulate X², then Y², then Z²; multiply by
/// one reciprocal. A zero vector yields NaN in the client, not a unit axis.
pub(super) fn normalize_axis([x, y, z]: [f32; 3]) -> [f32; 3] {
    let length = ((x * x + y * y) + z * z).sqrt();
    let inverse = 1.0 / length;
    [x * inverse, y * inverse, z * inverse]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modelskin_uv_matrix_retains_original_zero_terms() {
        for trig in [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma] {
            // Original 140401490 mode 2/profile 0/variant 1: the two affine
            // multiplies turn these signed-zero scale/scroll inputs into +0.
            let rows = uv_rows([-0.0, 0.0], [-0.0, -0.0], -0.0, Some(trig));
            assert_eq!(
                std::array::from_fn::<_, 8, _>(|i| rows.as_flattened()[i].to_bits()),
                [0; 8]
            );
        }
    }

    #[test]
    #[ignore = "original complete Aura Numeric UV selection/matrices, both actual CRT paths"]
    fn compare_original_modelskin_uv_matrix_pack() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("modelskin-aura-pack-client-probe.json")).unwrap(),
        )
        .unwrap();
        let number = |v: &serde_json::Value| f32::from_bits(v.as_u64().unwrap() as u32);
        let mut coverage = [0usize; 2];
        let mut components = 0usize;
        for (ordinal, case) in input["cases"].as_array().unwrap().iter().enumerate() {
            let crt = case["crt"].as_u64().unwrap() as usize;
            let trig = [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma][crt];
            for (slot, field) in ["tc2", "tc3", "td"].into_iter().enumerate() {
                let flags = case[field].as_u64().unwrap() as u32;
                let enabled =
                    flags & 1 != 0 && (slot != 1 || case["tc3Present"].as_bool().unwrap());
                let actual = if enabled {
                    let index = ((flags >> if slot == 2 { 5 } else { 4 }) & 7) as usize;
                    let uv = &case["uv"][index];
                    uv_rows(
                        [number(&uv[0]), number(&uv[1])],
                        [number(&uv[2]), number(&uv[3])],
                        number(&uv[4]),
                        Some(trig),
                    )
                } else {
                    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]]
                };
                for (lane, value) in actual.as_flattened().iter().enumerate() {
                    assert_eq!(
                        u64::from(value.to_bits()),
                        case["output"][32 + slot * 8 + lane].as_u64().unwrap(),
                        "case {ordinal} slot {slot} lane {lane}"
                    );
                    components += 1;
                }
            }
            coverage[crt] += 1;
        }
        assert_eq!(coverage, [2048, 2048]);
        assert_eq!(components, 98304);
        std::fs::write(folder.join("modelskin-aura-pack-uv-comparison.json"), serde_json::to_string_pretty(&serde_json::json!({"cases":4096,"coverageByCrt":coverage,"componentsCompared":components,"differences":0,"scope":"Original full Numeric matrix construction and UV routing versus production uv_rows; controlled cached UV inputs, bounded angles, no GPU/provider/constructor."})).unwrap()+"\n").unwrap();
    }

    #[test]
    fn modelskin_axis_rotation_keeps_reference_modes_and_normalizes_mode_two() {
        for trig in [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma] {
            assert_eq!(fresnel_axis(2, [0.0; 3], Some(trig)), [0.0, -1.0, 0.0]);
            assert_eq!(fresnel_axis(3, [0.0; 3], Some(trig)), [0.0, 0.0, -1.0]);
            assert_eq!(fresnel_axis(1, [0.3; 3], Some(trig)), [0.0; 3]);
        }
        // Mode two normalizes even without a model transform; mode three
        // must wait until after the entire model linear transform is applied.
        let rotation = [0.31415927, -0.739, 1.2323];
        let axis = fresnel_axis(2, rotation, Some(VfxClientTrigMode::Sse2));
        assert!((axis.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 2.0e-7);
    }

    #[test]
    fn modelskin_axis_normalization_preserves_client_reciprocal_and_degenerate_result() {
        let axis = normalize_axis([3.0, -4.0, 12.0]);
        assert_eq!(
            axis,
            [3.0 * (1.0 / 13.0), -4.0 * (1.0 / 13.0), 12.0 * (1.0 / 13.0)]
        );
        assert!(normalize_axis([0.0; 3]).iter().all(|v| v.is_nan()));
        assert_eq!(normalize_axis([1.0e20, -1.0e20, 1.0e20]), [0.0, -0.0, 0.0]);
    }

    #[test]
    #[ignore = "compare the complete original Fresnel helper and actual CRT against production axis arithmetic"]
    fn compare_original_modelskin_fresnel_axis_and_model_transform() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("modelskin-fresnel-client-probe.json")).unwrap(),
        )
        .unwrap();
        let number = |v: &serde_json::Value| f32::from_bits(v.as_u64().unwrap() as u32);
        let mut outputs = 0usize;
        let mut degenerate = 0usize;
        let mut coverage = [[[0usize; 5]; 2]; 2];
        for (index, case) in input["cases"].as_array().unwrap().iter().enumerate() {
            let mode = case["mode"].as_i64().unwrap() as i32;
            let trig = [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma]
                [case["crt"].as_u64().unwrap() as usize];
            coverage[case["crt"].as_u64().unwrap() as usize][mode as usize - 2]
                [case["matrixIndex"].as_u64().unwrap() as usize] += 1;
            let rotation = std::array::from_fn(|axis| number(&case["rotation"][axis]));
            let matrix = std::array::from_fn(|column| {
                std::array::from_fn(|row| number(&case["matrix"][column * 3 + row]))
            });
            let mut actual = fresnel_axis(mode, rotation, Some(trig));
            if mode == 3 {
                actual = normalize_axis(super::super::basis_transform(matrix, actual));
            }
            for axis in 0..3 {
                let expected = number(&case["output"][axis]);
                if expected.is_nan() {
                    assert!(actual[axis].is_nan(), "case {index} axis {axis}");
                    degenerate += 1;
                } else {
                    assert_eq!(
                        actual[axis].to_bits(),
                        expected.to_bits(),
                        "case {index} {trig:?} mode {mode} axis {axis} rotation {rotation:?}"
                    );
                    outputs += 1;
                }
            }
            assert_eq!(number(&case["output"][3]), 0.375);
        }
        assert_eq!(coverage, [[[1029; 5]; 2]; 2]);
        std::fs::write(folder.join("modelskin-fresnel-rust-comparison.json"), serde_json::to_string_pretty(&serde_json::json!({
            "cases":input["casesCount"],"finiteOutputsCompared":outputs,"nanClassificationsCompared":degenerate,"differences":0,
            "scope":"Complete original 140400a80 with actual SSE2/AVX-FMA CRT versus production ModelSkin axis arithmetic for modes2/3; angles within floatPI, identity/nonuniform/mirrored/sheared/collapsed/overflowing matrices. Exponent curve and incoming model matrix are controlled; no full constructor/provider/Aura packing/GPU. Production preview still sanitizes collapsed mode3 axes; NaN payload/sign not compared."
        })).unwrap()+"\n").unwrap();
    }
}
