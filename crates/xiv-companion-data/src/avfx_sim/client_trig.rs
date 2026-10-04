//! Current client's two CRT paths in the finite [-floatPI, floatPI] domain.
//! This is not a general sin/cos implementation: large argument reduction,
//! nonfinite handling/errno and runtime CRT dispatch discovery remain separate.

/// The client's runtime CRT dispatch word at 142c1b230 selects these two
/// different rounding paths. Choose from a resolved client/host snapshot;
/// do not infer the client's mode from the machine running this simulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxClientTrigMode {
    Sse2,
    AvxFma,
}

const QUARTER_PI: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
const INV_HALF_PI: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
const HALF_PI_HIGH: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
const HALF_PI_LOW: f64 = f64::from_bits(0x3dd0_b461_1a62_6331);
const HALF_PI_HIGH_2: f64 = f64::from_bits(0x3dd0_b461_1a60_0000);
const HALF_PI_LOW_2: f64 = f64::from_bits(0x3ba3_198a_2e03_7073);
const ONE_SIXTH: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
const SIN: [f64; 4] = [
    f64::from_bits(0xbfc5_5555_5555_5555),
    f64::from_bits(0x3f81_1111_1111_1111),
    f64::from_bits(0xbf2a_01a0_1a01_a01a),
    f64::from_bits(0x3ec7_1de3_a556_c734),
];
const COS: [f64; 5] = [
    -0.5,
    f64::from_bits(0x3fa5_5555_5555_5555),
    f64::from_bits(0xbf56_c16c_16c1_6c16),
    f64::from_bits(0x3efa_01a0_1a01_a019),
    f64::from_bits(0xbe92_7e4f_b778_9f5c),
];

fn sine_kernel(x: f64, mode: VfxClientTrigMode) -> f64 {
    let square = x * x;
    let cube = x * square;
    match mode {
        VfxClientTrigMode::Sse2 => {
            let fourth = square * square;
            let a = (SIN[3] * square + SIN[2]) * fourth;
            let b = SIN[1] * square + SIN[0];
            x + (a + b) * cube
        }
        VfxClientTrigMode::AvxFma => {
            let a = square.mul_add(SIN[3], SIN[2]);
            let b = a.mul_add(square, SIN[1]);
            let c = b.mul_add(square, SIN[0]);
            c.mul_add(cube, x)
        }
    }
}

fn cosine_kernel(x: f64, mode: VfxClientTrigMode, sine_branch: bool) -> f64 {
    let square = x * x;
    let fourth = square * square;
    match mode {
        VfxClientTrigMode::Sse2 => {
            let a = (COS[4] * square + COS[3]) * fourth;
            let b = COS[2] * square + COS[1];
            let base = COS[0] * square + 1.0;
            base + (a + b) * fourth
        }
        VfxClientTrigMode::AvxFma => {
            // Original cosf computes 1 - .5*x² separately. The cosine
            // branch inside sinf instead fuses -.5*x² + 1 before the tail.
            let base = if sine_branch {
                square.mul_add(COS[0], 1.0)
            } else {
                1.0 - square * 0.5
            };
            let a = square.mul_add(COS[4], COS[3]);
            let b = a.mul_add(square, COS[2]);
            let c = b.mul_add(square, COS[1]);
            c.mul_add(fourth, base)
        }
    }
}

fn reduced(x: f64, mode: VfxClientTrigMode) -> (f64, i32, u64) {
    let n = match mode {
        VfxClientTrigMode::Sse2 => (x * INV_HALF_PI + 0.5) as i32,
        VfxClientTrigMode::AvxFma => INV_HALF_PI.mul_add(x, 0.5) as i32,
    };
    let nf = f64::from(n);
    let mut high = match mode {
        VfxClientTrigMode::Sse2 => x - nf * HALF_PI_HIGH,
        VfxClientTrigMode::AvxFma => (-nf).mul_add(HALF_PI_HIGH, x),
    };
    let mut tail = nf * HALF_PI_LOW;
    let mut remainder = high - tail;
    // The original SSE2 branch uses the exponent before its second correction
    // for both the cancellation gate and its tiny-argument polynomial choice.
    let exponent = (remainder.to_bits() << 1) >> 53;
    if mode == VfxClientTrigMode::Sse2 && (x.to_bits() >> 52) as i64 - exponent as i64 > 15 {
        let old = high;
        let product = nf * HALF_PI_HIGH_2;
        high -= product;
        tail = nf * HALF_PI_LOW_2 - ((old - high) - product);
        remainder = high - tail;
    }
    (remainder, n, exponent)
}

/// 141ddf500/141de1260 for every finite angle produced by the Spline's
/// unsigned-WORD random draw. Returns [cosf, sinf] in original call order.
/// Spline passes draw*floatTAU/65535-floatPI. ModelSkin checks this same
/// domain before calling; its larger angles still use the host fallback.
pub(super) fn spline_transverse(angle: f32, mode: VfxClientTrigMode) -> [f32; 2] {
    assert!(
        angle.is_finite() && angle.abs() <= std::f32::consts::PI,
        "client angle must stay in the verified finite domain"
    );
    let x = f64::from(angle);
    let absolute = x.abs();
    let cosine = if absolute <= QUARTER_PI {
        if mode == VfxClientTrigMode::AvxFma && absolute < 0.0078125 {
            if absolute < 0.0001220703125 {
                1.0
            } else {
                (-(x * 0.5)).mul_add(x, 1.0)
            }
        } else {
            cosine_kernel(x, mode, false)
        }
    } else {
        let (r, n, exponent) = reduced(absolute, mode);
        let mut result = if mode == VfxClientTrigMode::Sse2 && exponent < 0x3f2 {
            if n & 1 == 0 {
                if exponent <= 0x3de {
                    1.0
                } else {
                    1.0 - (r * r) * 0.5
                }
            } else if exponent <= 0x3de {
                r
            } else {
                r - ((ONE_SIXTH * r) * (r * r))
            }
        } else if n & 1 == 0 {
            cosine_kernel(r, mode, false)
        } else {
            sine_kernel(r, mode)
        };
        if (n + 1) & 2 != 0 {
            result = if mode == VfxClientTrigMode::Sse2 {
                0.0 - result
            } else {
                f64::from_bits(result.to_bits() ^ (1u64 << 63))
            };
        }
        result
    } as f32;
    let sine = if absolute <= QUARTER_PI {
        if absolute < 0.0001220703125 {
            // The CRT also raises inexact using f32 operations; its return
            // stays equal to the input, including signed zero/subnormals.
            return [cosine, angle];
        } else if absolute < 0.0078125 {
            let cube = (x * x) * x;
            match mode {
                VfxClientTrigMode::Sse2 => x - cube * ONE_SIXTH,
                VfxClientTrigMode::AvxFma => (-cube).mul_add(ONE_SIXTH, x),
            }
        } else {
            sine_kernel(x, mode)
        }
    } else {
        let (r, n, exponent) = reduced(absolute, mode);
        let mut result = if mode == VfxClientTrigMode::Sse2 && exponent < 0x3f2 {
            if n & 1 != 0 {
                if exponent <= 0x3de {
                    1.0
                } else {
                    1.0 - (r * r) * 0.5
                }
            } else if exponent <= 0x3de {
                r
            } else {
                r - ((ONE_SIXTH * r) * (r * r))
            }
        } else if n & 1 == 0 {
            sine_kernel(r, mode)
        } else {
            cosine_kernel(r, mode, true)
        };
        if ((n >> 1) & 1 != 0) ^ angle.is_sign_negative() {
            result = if mode == VfxClientTrigMode::Sse2 {
                0.0 - result
            } else {
                f64::from_bits(result.to_bits() ^ (1u64 << 63))
            };
        }
        result
    } as f32;
    [cosine, sine]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_spline_trig_retains_signed_zero_and_quadrant_signs() {
        for mode in [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma] {
            let positive = spline_transverse(0.0, mode);
            let negative = spline_transverse(-0.0, mode);
            assert_eq!(positive, [1.0, 0.0]);
            assert_eq!(negative[0], 1.0);
            assert_eq!(negative[1].to_bits(), (-0.0f32).to_bits());
            assert!(spline_transverse(std::f32::consts::PI, mode)[0] < 0.0);
            assert!(spline_transverse(std::f32::consts::PI, mode)[1] < 0.0);
            assert!(spline_transverse(-std::f32::consts::PI, mode)[1] > 0.0);
        }
    }

    #[test]
    #[ignore = "compare both original CRT paths for all Spline random angles and finite branch stress inputs"]
    fn compare_original_client_spline_trig_two_dispatch_paths() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("client-spline-trig-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = input["cases"].as_array().unwrap();
        let mut finite = 0usize;
        let mut draws = [0usize; 2];
        let mut samples = [0usize; 2];
        let mut seen = [vec![false; 65536], vec![false; 65536]];
        for (ordinal, case) in cases.iter().enumerate() {
            let index = case[0].as_u64().unwrap() as usize;
            let mode = [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma][index];
            let angle = f32::from_bits(case[2].as_u64().unwrap() as u32);
            let actual = spline_transverse(angle, mode);
            samples[index] += 1;
            let draw = case[1].as_i64().unwrap();
            if draw >= 0 {
                let draw = usize::try_from(draw).unwrap();
                assert!(!seen[index][draw], "duplicate draw {draw} mode {mode:?}");
                seen[index][draw] = true;
                let produced = draw as f32 * std::f32::consts::TAU / 65535.0 - std::f32::consts::PI;
                assert_eq!(produced.to_bits(), angle.to_bits());
                draws[index] += 1;
            }
            for axis in 0..2 {
                let expected = case[3 + axis].as_u64().unwrap() as u32;
                assert_eq!(
                    actual[axis].to_bits(),
                    expected,
                    "case {ordinal} mode {mode:?} input {:08x} axis {axis}: {} != {}",
                    angle.to_bits(),
                    actual[axis],
                    f32::from_bits(expected)
                );
                finite += 1;
            }
        }
        assert_eq!(draws, [65536; 2]);
        std::fs::write(folder.join("client-spline-trig-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({
            "cases":cases.len(),"samplesPerMode":samples,"allDrawAnglesPerMode":draws,
            "finiteOutputsCompared":finite,"differences":0,
            "scope":"Original cosf/sinf with SSE2 and AVX/FMA dispatch vs production finite Spline angle kernel. Exhaustive u16 draw angles plus finite branch neighborhoods/random float bits within [-floatPI,floatPI]. No large/nonfinite/errno, live CRT mode discovery, GPU or full playback. Normal MXCSR, finite output bits including signed zero compared."
        })).unwrap()+"\n").unwrap();
    }
}
