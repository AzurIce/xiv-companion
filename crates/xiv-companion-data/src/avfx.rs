//! `.avfx`（武器/技能特效）读取器：递归块 walker + 类型化子集 + 曲线求值。
//!
//! 布局对齐 VFXEditor `VFXEditor/Formats/AvfxFormat/`（社区字段级事实标准）：
//! 文件为一个外层块（盘上名字为反写 `AVFX`），子块为 `[反写4字节名][u32 size]
//! [负载]`，size 按 4 字节对齐推进；叶子字段负载为 4 字节（i32/f32/枚举，
//! 个别 1 字节布尔），`Tex` 为 NUL 结尾字符串，`TLst`/`MdNo` 等 IntList 为
//! 字节数组。`Schd`/`TmLn`/`Emit` 的 `Item`/`Trgr`/`ItPr`/`ItEm` 容器为累积式
//! （最后一个容器含全部条目），条目不带独立块包装，按 `bEna`/`bEnb`
//! 首字段切分，兼容不同字段数量；`ItEm` 末容器的前 N 条是粒子条目的重复
//! （N = 粒子条目数），解析时跳过。
//!
//! 曲线分三种容器形态：
//! - 单轴曲线（`CrC`/`Gra`/UvSt `Rot` 等）：块内直接是 `KeyC`/`BvPr`/`BvPo`/
//!   `RanT`/`Keys`；
//! - 多轴曲线（`Pos`/`Rot`/`Scl` 与 UvSt 的 `Scl`/`Scr`）：容器块，子块为
//!   `ACT`/`ACTR`（轴连接枚举）与按轴命名的 `X`/`Y`/`Z` 单轴曲线（可缺省，
//!   缺省轴取默认值）；
//! - 颜色曲线（`Col`/`ColB`/`ColE`）：容器块，子块为 `RGB`（键的 x/y/z 即
//!   r/g/b，线性或阶梯）、`A`（alpha）、`Bri`（HDR 亮度倍率）、`SclR`..`SclA`。
//!
//! 未知块按名计数进 `unknown_blocks`，永不 panic。

use half::f16;
use std::collections::{BTreeMap, HashMap};

#[cfg(test)]
#[path = "avfx_curve_tests.rs"]
pub(crate) mod curve_client_tests;

/// 一条曲线关键帧：16 字节 = 时间(i16，帧) + 插值类型(u16) + 三分量(f32)。
///
/// 非颜色曲线中 `x`/`y` 是样条切线权重、`z` 是数值；颜色曲线直接用
/// `x`/`y`/`z` 作 RGB。右键的 interpolation 控制其前一段；标量样条
/// 使用相邻键的量化切线权重，颜色的 Spline 与 Linear 均按线性求值。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxCurveKey {
    pub time: i16,
    pub interpolation: u16,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AvfxCurveKey {
    pub const INTERPOLATION_SPLINE: u16 = 0;
    pub const INTERPOLATION_LINEAR: u16 = 1;
    pub const INTERPOLATION_STEP: u16 = 2;

    pub(crate) fn scalar_time(self) -> u16 {
        self.time as u16 & 0x3fff
    }

    fn color_time(self) -> f32 {
        (i32::from(self.time) as u32 & 0x0fff_ffff) as f32
    }
}

/// 曲线首尾行为（VFXEditor `CurveBehavior`）。
pub const BEHAVIOR_CONST: u32 = 0;
pub const BEHAVIOR_REPEAT: u32 = 1;
pub const BEHAVIOR_ADD: u32 = 2;

/// 单轴曲线：`BvPr`/`BvPo`/`RanT` 标量 + `Keys` 裸 16 字节键数组。
/// `KeyC` 声明计数在文件加载时校验。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxCurve {
    pub pre_behavior: u32,
    pub post_behavior: u32,
    pub random_type: u32,
    pub keys: Vec<AvfxCurveKey>,
}

impl AvfxCurve {
    /// 在 `time`（帧）处求值，返回该键的 (x, y, z)。数值曲线取 z（样条），
    /// x/y 分量按线性近似返回。
    pub fn evaluate(&self, time: f32) -> [f32; 3] {
        self.evaluate_at(time, time)
    }

    /// Runtime evaluation with the instance's looped and accumulated ages.
    /// The client selects the accumulated clock for Add/unknown post modes.
    pub fn evaluate_at(&self, local_time: f32, total_time: f32) -> [f32; 3] {
        let time = self.runtime_time(local_time, total_time);
        let mut components = self.color_at(local_time);
        components[2] = self.value_with(time, &|index| self.keys[index].z);
        components
    }

    /// RGB Spline/Linear share the client's single-precision linear kernel.
    pub fn color_at(&self, time: f32) -> [f32; 3] {
        let Some(first) = self.keys.first() else {
            return [1.0; 3];
        };
        let last = self.keys.last().expect("non-empty");
        let color = |key: &AvfxCurveKey| [key.x, key.y, key.z];
        if self.keys.len() == 1 {
            return color(first);
        }
        let start = first.color_time();
        let end = last.color_time();
        // Both RGB endpoints dispatch inclusively. NaN bypasses the range
        // handlers, matching the client's unordered COMISS/JB branches.
        let time = if time <= start {
            match self.pre_behavior & 3 {
                BEHAVIOR_REPEAT | BEHAVIOR_ADD => {
                    repeat_scalar_curve_time(time, start, end, true).0
                }
                3 => return color(last), // adjacent post-Const table entry
                _ => return color(first),
            }
        } else if time >= end {
            match self.post_behavior & 3 {
                BEHAVIOR_REPEAT | BEHAVIOR_ADD => {
                    repeat_scalar_curve_time(time, start, end, false).0
                }
                _ => return color(last),
            }
        } else {
            time
        };
        if end < start {
            return color(first);
        }
        // An unsuccessful native search beyond the last key reads a next
        // time outside the authored array. Do not fabricate that memory.
        if time > end {
            return [f32::NAN; 3];
        }
        let index = self.color_segment_index(time);
        let left = self.keys[index];
        let Some(right) = self.keys.get(index + 1) else {
            return [f32::NAN; 3];
        };
        if right.interpolation & 15 == AvfxCurveKey::INTERPOLATION_STEP {
            return color(if time >= right.color_time() {
                right
            } else {
                &left
            });
        }
        // Native subtraction uses the packed integer times before converting
        // to f32; near 2^28, distinct key times can round to the same float.
        let packed_time = |key: &AvfxCurveKey| i32::from(key.time) as u32 & 0x0fff_ffff;
        let span = packed_time(right).wrapping_sub(packed_time(&left)) as f32;
        let u = (time - left.color_time()) / span;
        [
            (right.x - left.x) * u + left.x,
            (right.y - left.y) * u + left.y,
            (right.z - left.z) * u + left.z,
        ]
    }

    fn color_segment_index(&self, time: f32) -> usize {
        // RGB searches inclusive [left, right] intervals directly, without
        // ceilf. At duplicate times, the first interval visited wins.
        let mut left = 0i32;
        let mut right = self.keys.len() as i32 - 1;
        while left <= right {
            let middle = left + (right - left) / 2;
            let key = self.keys[middle as usize];
            if key.color_time() > time {
                right = middle - 1;
            } else {
                left = middle + 1;
                if let Some(next) = self.keys.get(middle as usize + 1) {
                    if time <= next.color_time() || time.is_nan() {
                        return middle as usize;
                    }
                }
            }
        }
        0
    }

    pub fn color_at_times(&self, local_time: f32, _total_time: f32) -> [f32; 3] {
        // Original Color RGB caller 0x14039a750 forwards XMM1 (local age).
        // Scalar A/Bri/scale/random readers select their own clocks.
        self.color_at(local_time)
    }

    /// 数值求值（z 分量）；无键时返回 `default`。
    pub fn value(&self, time: f32, default: f32) -> f32 {
        self.value_at(time, time, default)
    }

    /// Numeric runtime evaluation with separate looped and accumulated ages.
    pub fn value_at(&self, local_time: f32, total_time: f32, default: f32) -> f32 {
        if self.keys.is_empty() {
            default
        } else {
            self.value_with(self.runtime_time(local_time, total_time), &|index| {
                self.keys[index].z
            })
        }
    }

    fn runtime_time(&self, local_time: f32, total_time: f32) -> f32 {
        if self.post_behavior & 3 >= BEHAVIOR_ADD {
            total_time
        } else {
            local_time
        }
    }

    pub(crate) fn value_with(&self, time: f32, value: &impl Fn(usize) -> f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        let last_index = self.keys.len() - 1;
        let start = f32::from(first.scalar_time());
        let end = f32::from(self.keys[last_index].scalar_time());
        if last_index == 0 {
            return value(0);
        }
        let (time, addition) = if time < start || time > end {
            let before = time < start;
            let behavior = if before {
                self.pre_behavior
            } else {
                self.post_behavior
            } & 3;
            if !matches!(behavior, BEHAVIOR_REPEAT | BEHAVIOR_ADD) {
                // Pre code 3 indexes the adjacent post-Const table entry.
                return value(if before && behavior != 3 {
                    0
                } else {
                    last_index
                });
            }
            let (time, cycles) = repeat_scalar_curve_time(time, start, end, before);
            (time, (behavior == BEHAVIOR_ADD).then_some((before, cycles)))
        } else {
            (time, None)
        };
        // Malformed descending endpoints keep the preview fallback. Valid
        // duplicate keys execute the segment kernel, including zero spans.
        let sampled = if end < start {
            value(0)
        } else {
            let index = self.scalar_segment_index(time);
            if index + 1 < self.keys.len() {
                scalar_segment_value(&self.keys, index, time, value)
            } else {
                // The client can select an unauthored key at extreme ages.
                // Propagate an invalid sample instead of inventing a value
                // from memory following the compiled curve allocation.
                f32::NAN
            }
        };
        match addition {
            // Preserve the client's subtraction for pre-Add, rather than
            // adding a negated product (observable with signed zero).
            Some((true, cycles)) => sampled - (-cycles) * (value(last_index) - value(0)),
            Some((false, cycles)) => sampled + cycles * (value(last_index) - value(0)),
            None => sampled,
        }
    }

    fn scalar_segment_index(&self, time: f32) -> usize {
        // Original lower_bound compares the sign of a wrapping i32 subtraction,
        // rather than comparing the floats. ceilf then CVTTSS2SI also matters
        // when a Repeat/Add handler leaves the authored key range.
        let target = curve_cvtt_i32(time.ceil());
        let mut left = 0i32;
        let mut right = self.keys.len() as i32 - 1;
        while left <= right {
            let middle = (left + right) / 2;
            let key_time = i32::from(self.keys[middle as usize].scalar_time());
            if key_time.wrapping_sub(target) < 0 {
                left = middle + 1;
            } else {
                right = middle - 1;
            }
        }
        left.saturating_sub(1).max(0) as usize
    }
}

fn curve_cvtt_i32(value: f32) -> i32 {
    // Rust float-to-int casts saturate; SSE returns the integer-indefinite
    // value for NaN, infinity or overflow. The negative endpoint is valid.
    if (-2147483648.0..2147483648.0).contains(&value) {
        value as i32
    } else {
        i32::MIN
    }
}

fn repeat_scalar_curve_time(time: f32, start: f32, end: f32, before: bool) -> (f32, f32) {
    let period = end - start;
    if period <= 0.0 {
        // Zero-period Repeat/Add would fault in native integer division.
        // Retain the preview fallback for malformed resources.
        return (start, 0.0);
    }
    let distance = if before { start - time } else { time - end };
    let count = (curve_cvtt_i32(distance) / period as i32).wrapping_add(1) as f32;
    if before {
        (time + count * period, -count)
    } else {
        (time - count * period, count)
    }
}

// Original 0x1403968f0 / 0x140396ad0 / 0x140396b40 scalar kernels. Keep f32
// operations in instruction order; f64 polynomial evaluation changes rounding
// and hides the original overflow/NaN behavior, even at exact key times.
fn scalar_segment_value(
    keys: &[AvfxCurveKey],
    index: usize,
    time: f32,
    value: &impl Fn(usize) -> f32,
) -> f32 {
    let left = keys[index];
    let right = keys[index + 1];
    let left_time = f32::from(left.scalar_time());
    let right_time = f32::from(right.scalar_time());
    let interpolation = right.interpolation & 3;
    if interpolation == AvfxCurveKey::INTERPOLATION_STEP {
        return value(if time >= right_time { index + 1 } else { index });
    }
    let span = right_time - left_time;
    let delta = value(index + 1) - value(index);
    let u = (time - left_time) / span;
    if interpolation == AvfxCurveKey::INTERPOLATION_LINEAR {
        return u * delta + value(index);
    }
    let mut right_tangent = packed_tangent_weight(right.y) * delta;
    if index + 2 < keys.len() {
        let next_delta = value(index + 2) - value(index + 1);
        let neighboring = packed_tangent_weight(right.x) * next_delta;
        let ratio = span / (f32::from(keys[index + 2].scalar_time()) - left_time);
        right_tangent = (neighboring + right_tangent) * ratio;
    }
    let left_tangent = if index > 0 {
        let outgoing = delta * packed_tangent_weight(left.x);
        let ratio = span / (right_time - f32::from(keys[index - 1].scalar_time()));
        let previous_delta = value(index) - value(index - 1);
        let incoming = packed_tangent_weight(left.y) * previous_delta;
        ratio * (incoming + outgoing)
    } else {
        delta * packed_tangent_weight(left.x)
    };
    let u2 = u * u;
    let u3 = u2 * u;
    let three_u2 = u2 * 3.0;
    let two_u3 = u3 + u3;
    let h00 = (two_u3 - three_u2) + 1.0;
    let h01 = three_u2 - two_u3;
    let h11 = u3 - u2;
    let h10 = (u3 - (u2 + u2)) + u;
    ((h00 * value(index) + h01 * value(index + 1)) + h10 * left_tangent) + h11 * right_tangent
}

// Coefficients in normalized segment time for analytic preview integration.
// Client 0x1403968f0 uses weighted, neighboring Hermite tangents; the editor's
// GetDrawLine Bezier is only a graph approximation of this runtime curve.
pub(crate) fn curve_segment_polynomial(
    keys: &[AvfxCurveKey],
    index: usize,
    value: &impl Fn(usize) -> f64,
) -> [f64; 4] {
    let left = keys[index];
    let right = keys[index + 1];
    let y0 = value(index);
    let delta = value(index + 1) - y0;
    if right.interpolation & 3 == AvfxCurveKey::INTERPOLATION_STEP {
        return [y0, 0.0, 0.0, 0.0];
    }
    if right.interpolation & 3 == AvfxCurveKey::INTERPOLATION_LINEAR {
        return [y0, delta, 0.0, 0.0];
    }
    let span = f64::from(right.scalar_time()) - f64::from(left.scalar_time());
    let mut m0 = tangent_weight(left.x) * delta;
    let mut m1 = tangent_weight(right.y) * delta;
    if index > 0 {
        m0 += tangent_weight(left.y) * (y0 - value(index - 1));
        m0 *= span / (f64::from(right.scalar_time()) - f64::from(keys[index - 1].scalar_time()));
    }
    if index + 2 < keys.len() {
        m1 += tangent_weight(right.x) * (value(index + 2) - value(index + 1));
        m1 *= span / (f64::from(keys[index + 2].scalar_time()) - f64::from(left.scalar_time()));
    }
    [y0, m0, 3.0 * delta - 2.0 * m0 - m1, -2.0 * delta + m0 + m1]
}

fn tangent_weight(raw: f32) -> f64 {
    f64::from(packed_tangent_weight(raw))
}

fn packed_tangent_weight(raw: f32) -> f32 {
    let scaled = raw * 15.0;
    // CVTTSS2SI's invalid result is INT_MIN, whose low byte is zero.
    let packed = if (-2147483648.0..2147483648.0).contains(&scaled) {
        scaled as i32 as i8
    } else {
        0
    };
    f32::from(packed) * (1.0 / 15.0)
}

/// 三轴曲线容器（`Pos`/`Rot`/`Scl`）：子块 `ACT`（轴连接枚举）+ 按轴命名的
/// 单轴曲线 `X`/`Y`/`Z`（各自可缺省）。随机轴由采样运行时按实例求值。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxCurve3Axis {
    pub axis_connect: u32,
    pub axis_connect_random: u32,
    pub x: Option<AvfxCurve>,
    pub y: Option<AvfxCurve>,
    pub z: Option<AvfxCurve>,
    /// 随机轴曲线（`XR`/`YR`/`ZR`）：随机上下幅度，按粒子播种叠加。
    pub random_x: Option<AvfxCurve>,
    pub random_y: Option<AvfxCurve>,
    pub random_z: Option<AvfxCurve>,
}

impl AvfxCurve3Axis {
    /// 三轴求值；缺省轴（无子块或无键）取 `default`（位置/旋转 0，缩放 1）。
    /// `ACT` 轴连接（VFXEditor `AxisConnect3`）：命名序中首轴为源，其余轴
    /// 跟随源轴的值（如 X_Z = 3 时 z 取 x；真实文件用它保持圆环横截面等比）。
    pub fn evaluate(&self, time: f32, default: f32) -> [f32; 3] {
        self.evaluate_at(time, time, default)
    }

    pub fn evaluate_at(&self, local_time: f32, total_time: f32, default: f32) -> [f32; 3] {
        let x = self
            .x
            .as_ref()
            .map_or(default, |c| c.value_at(local_time, total_time, default));
        let y = self
            .y
            .as_ref()
            .map_or(default, |c| c.value_at(local_time, total_time, default));
        let z = self
            .z
            .as_ref()
            .map_or(default, |c| c.value_at(local_time, total_time, default));
        let mut value = [x, y, z];
        connect_axes3(self.axis_connect, &mut value);
        value
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_none() && self.y.is_none() && self.z.is_none()
    }
}

/// Native XYZ connections copy even an empty source's default value. The
/// compiler takes the signed low byte and stores only its low four bits.
pub(crate) fn connect_axes3(connect: u32, values: &mut [f32; 3]) {
    match connect & 15 {
        1 => {
            values[1] = values[0];
            values[2] = values[0];
        }
        2 => values[1] = values[0],
        3 => values[2] = values[0],
        4 => {
            values[0] = values[1];
            values[2] = values[1];
        }
        5 => values[0] = values[1],
        6 => values[2] = values[1],
        7 => {
            values[0] = values[2];
            values[1] = values[2];
        }
        8 => values[0] = values[2],
        9 => values[1] = values[2],
        _ => {}
    }
}

/// 两轴曲线容器（UvSt 的 `Scl`/`Scr`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxCurve2Axis {
    pub axis_connect: u32,
    pub axis_connect_random: u32,
    pub x: Option<AvfxCurve>,
    pub y: Option<AvfxCurve>,
    pub random_x: Option<AvfxCurve>,
    pub random_y: Option<AvfxCurve>,
}

impl AvfxCurve2Axis {
    /// 两轴求值；`ACT`（`AxisConnect2`：1 X_Y、2 Y_X）时目标轴跟随源轴。
    pub fn evaluate(&self, time: f32, default: f32) -> [f32; 2] {
        self.evaluate_at(time, time, default)
    }

    pub fn evaluate_at(&self, local_time: f32, total_time: f32, default: f32) -> [f32; 2] {
        let mut x = self
            .x
            .as_ref()
            .map_or(default, |c| c.value_at(local_time, total_time, default));
        let mut y = self
            .y
            .as_ref()
            .map_or(default, |c| c.value_at(local_time, total_time, default));
        match self.axis_connect {
            1 => {
                if self.x.is_some() {
                    y = x;
                }
            }
            2 => {
                if self.y.is_some() {
                    x = y;
                }
            }
            _ => {}
        }
        [x, y]
    }
}

/// 颜色曲线容器（`Col`/`ColB`/`ColE`）：`RGB` 颜色键 + `A` 透明度 +
/// `Bri` HDR 亮度倍率 + `SclR/G/B/A` 颜色通道缩放。缺省：白、不透明、倍率 1。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxColorCurve {
    pub rgb: Option<AvfxCurve>,
    pub alpha: Option<AvfxCurve>,
    pub brightness: Option<AvfxCurve>,
    pub scale_alpha: Option<AvfxCurve>,
    pub scale_rgb: Option<AvfxColorScaleRgb>,
    /// 随机通道曲线（`RanR`/`RanG`/`RanB`/`RanA`/`RBri`）。
    pub random: [Option<AvfxCurve>; 5],
}

/// `SclR`/`SclG`/`SclB`：颜色通道缩放曲线，在随机偏移之后乘算。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxColorScaleRgb {
    pub r: Option<AvfxCurve>,
    pub g: Option<AvfxCurve>,
    pub b: Option<AvfxCurve>,
}

impl AvfxColorCurve {
    /// 非随机颜色：各通道已乘 `SclR/G/B/A`，RGB 再乘 `Bri`（HDR，可 >1）。
    pub fn rgba(&self, time: f32) -> [f32; 4] {
        self.rgba_with_brightness(time, true)
    }

    pub fn rgba_at(&self, local_time: f32, total_time: f32) -> [f32; 4] {
        self.rgba_with_brightness_at(local_time, total_time, true)
    }

    /// 可选关闭亮度倍率供单独检查颜色；客户端所有 RMT 均求值 `Bri`。
    pub fn rgba_with_brightness(&self, time: f32, apply_brightness: bool) -> [f32; 4] {
        self.rgba_with_brightness_at(time, time, apply_brightness)
    }

    pub fn rgba_with_brightness_at(
        &self,
        local_time: f32,
        total_time: f32,
        apply_brightness: bool,
    ) -> [f32; 4] {
        self.compose_at(
            local_time,
            total_time,
            apply_brightness,
            [1.0; 4],
            |_, _| None,
        )
    }

    fn scale_curves(&self) -> [Option<&AvfxCurve>; 4] {
        let rgb = self.scale_rgb.as_ref();
        [
            rgb.and_then(|c| c.r.as_ref()),
            rgb.and_then(|c| c.g.as_ref()),
            rgb.and_then(|c| c.b.as_ref()),
            self.scale_alpha.as_ref(),
        ]
    }

    /// Original Color compiler (39b820): skip empty descriptors and single
    /// default keys. Multiple default keys remain active, including random
    /// zero amplitudes. RanT does not decide whether a channel is active.
    pub(crate) fn client_dispatch_flags(&self) -> u32 {
        fn scalar_active(curve: Option<&AvfxCurve>, default: f32) -> bool {
            curve.is_some_and(|c| match c.keys.as_slice() {
                [] => false,
                [key] => key.z != default,
                _ => true,
            })
        }
        let mut flags = 0;
        if self.rgb.as_ref().is_some_and(|c| match c.keys.as_slice() {
            [] => false,
            [key] => [key.x, key.y, key.z] != [1.0; 3],
            _ => true,
        }) {
            flags |= 2;
        }
        flags |= u32::from(scalar_active(self.alpha.as_ref(), 1.0)) << 2;
        for (i, curve) in self.random[..4].iter().enumerate() {
            flags |= u32::from(scalar_active(curve.as_ref(), 0.0)) << (i + 3);
        }
        for (i, curve) in self.scale_curves().into_iter().enumerate() {
            flags |= u32::from(scalar_active(curve, 1.0)) << (i + 7);
        }
        flags |= u32::from(scalar_active(self.brightness.as_ref(), 1.0)) << 11;
        flags |= u32::from(scalar_active(self.random[4].as_ref(), 0.0)) << 12;
        flags | u32::from(flags != 0)
    }

    /// Shared composition for nonrandom inspection, seeded preview and explicit
    /// client replay. None from a random callback means no addition (RanT 6/7
    /// or deliberately inspecting only the nonrandom channels).
    pub(crate) fn compose_at(
        &self,
        local: f32,
        total: f32,
        apply_brightness: bool,
        empty_rgba: [f32; 4],
        mut offset: impl FnMut(usize, &AvfxCurve) -> Option<f32>,
    ) -> [f32; 4] {
        let flags = self.client_dispatch_flags();
        if flags == 0 {
            return empty_rgba;
        }
        let mut rgba = [1.0; 4];
        if flags & 2 != 0 {
            rgba[..3].copy_from_slice(&self.rgb.as_ref().unwrap().color_at(local));
        }
        if flags & 4 != 0 {
            rgba[3] = self.alpha.as_ref().unwrap().value_at(local, total, 1.0);
        }
        for (i, channel) in rgba.iter_mut().enumerate() {
            if flags & (1 << (i + 3)) != 0
                && let Some(random) = offset(i, self.random[i].as_ref().unwrap())
            {
                *channel = random + *channel;
            }
        }
        for (i, curve) in self.scale_curves().into_iter().enumerate() {
            if flags & (1 << (i + 7)) != 0 {
                rgba[i] = curve.unwrap().value_at(local, total, 0.0) * rgba[i];
            }
        }
        if apply_brightness && flags & 0x1800 != 0 {
            let mut brightness = if flags & 0x800 != 0 {
                self.brightness
                    .as_ref()
                    .unwrap()
                    .value_at(local, total, 1.0)
            } else {
                1.0
            };
            if flags & 0x1000 != 0
                && let Some(random) = offset(4, self.random[4].as_ref().unwrap())
            {
                brightness += random;
            }
            for channel in &mut rgba[..3] {
                *channel = brightness * *channel;
            }
        }
        rgba
    }

    /// 颜色 alpha 缩放（`SclA`），缺省或无键时为 1。
    pub fn texture_alpha_scale(&self, time: f32) -> f32 {
        self.texture_alpha_scale_at(time, time)
    }

    pub fn texture_alpha_scale_at(&self, local_time: f32, total_time: f32) -> f32 {
        self.scale_alpha
            .as_ref()
            .map_or(1.0, |c| c.value_at(local_time, total_time, 1.0))
    }

    pub fn is_empty(&self) -> bool {
        self.rgb.is_none()
            && self.alpha.is_none()
            && self.brightness.is_none()
            && self.scale_alpha.is_none()
    }
}

/// Life 节点：4 字节哨兵（-1）= 未启用；否则嵌套 `Val`/`ValR`/`Type` 块。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxLife {
    pub enabled: bool,
    pub value: f32,
    pub value_random: f32,
    pub random_type: u32,
}

impl Default for AvfxLife {
    fn default() -> Self {
        Self {
            enabled: false,
            value: -1.0,
            value_random: 0.0,
            random_type: 0,
        }
    }
}

/// Scheduler/Timeline 条目。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxSchedulerItem {
    pub enabled: bool,
    pub start_time: i32,
    pub timeline_index: i32,
}

/// Keep raw i32 values; the client consumes these four reference indexes as signed bytes.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxTimelineItem {
    pub enabled: bool,
    pub start_time: i32,
    pub end_time: i32,
    pub binder_index: i32,
    pub effector_index: i32,
    pub emitter_index: i32,
    pub platform: i32,
    pub clip_index: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AvfxTimelineItemTarget {
    Effector(i32),
    Emitter(i32),
    Clip(i32),
    None,
}

impl AvfxTimelineItem {
    pub(crate) fn target(self) -> AvfxTimelineItemTarget {
        let effector = signed_byte_index(self.effector_index);
        if effector >= 0 {
            return AvfxTimelineItemTarget::Effector(effector);
        }
        let emitter = signed_byte_index(self.emitter_index);
        if emitter >= 0 {
            return AvfxTimelineItemTarget::Emitter(emitter);
        }
        let clip = signed_byte_index(self.clip_index);
        if clip >= 0 {
            return AvfxTimelineItemTarget::Clip(clip);
        }
        AvfxTimelineItemTarget::None
    }
}

/// Emitter 内的粒子/子发射器创建项（`ItPr`/`ItEm` 容器，累积式）。
/// 字段对齐 VFXEditor `AvfxEmitterItem`。
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEmitterItem {
    pub enabled: bool,
    pub target_index: i32,
    /// `LoDr`：局部方向模式。
    pub local_direction: i32,
    /// `CrTm`：0 周期创建，1 初始化创建，2 终止创建。
    pub create_time: i32,
    /// `CrCn`：CrTm=1/2 的数量；CrTm=0 使用发射器 CrC，二者不相乘。
    pub create_count: i32,
    pub create_probability: i32,
    /// `PICd`：父级坐标影响方式。
    pub parent_influence_coord: i32,
    /// `PICo`：父级颜色影响方式。
    pub parent_influence_color: i32,
    /// `ICbS`/`ICbR`/`ICbP`/`ICbB`：发射器缩放/旋转/位置/绑点位置是否传给子粒子。
    pub influence_coord_scale: bool,
    pub influence_coord_rot: bool,
    pub influence_coord_pos: bool,
    pub influence_coord_binder: bool,
    /// `ICSK`：位置影响的黏着系数。
    pub influence_coord_unstickiness: f32,
    /// `IPbV`/`IPbL`：继承父级速度/寿命。
    pub inherit_parent_velocity: bool,
    pub inherit_parent_life: bool,
    /// ItPr 覆盖粒子寿命（`bOvr`/`OvrV`/`OvrR`）。
    pub override_life: bool,
    pub override_life_value: i32,
    pub override_life_random: i32,
    /// `PrLk`：参数链接。
    pub parameter_link: i32,
    /// `StFr`：创建后预推进或直接设置的初始年龄，不是父级创建时间门槛。
    pub start_frame: i32,
    /// `bStN`：逐帧预推进；关闭时仅写入年龄，不执行中间更新。
    pub start_frame_null_update: bool,
    /// `BIAX`/`BIAY`/`BIAZ`：乘以本项累计 signed short 序号，添加到新实例的局部 Euler（弧度）。
    pub by_injection_angle: [f32; 3],
    /// `GenD`: initialization/finish delay helper; periodic creation ignores it.
    pub generate_delay: i32,
    pub generate_delay_by_one: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EmitterType {
    Point,
    Cone,
    ConeModel,
    SphereModel,
    CylinderModel,
    Model,
    Unknown(u32),
}

impl EmitterType {
    fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Point,
            1 => Self::Cone,
            2 => Self::ConeModel,
            3 => Self::SphereModel,
            4 => Self::CylinderModel,
            5 => Self::Model,
            other => Self::Unknown(other),
        }
    }
}

/// 粒子类型（VFXEditor `ParticleType`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleType {
    Parameter,
    Powder,
    Windmill,
    Line,
    Laser,
    Model,
    Polyline,
    Reserve0,
    Quad,
    Polygon,
    Decal,
    DecalRing,
    Disc,
    LightModel,
    ModelSkin,
    Dissolve,
    Unknown(u32),
}

impl ParticleType {
    fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Parameter,
            1 => Self::Powder,
            2 => Self::Windmill,
            3 => Self::Line,
            4 => Self::Laser,
            5 => Self::Model,
            6 => Self::Polyline,
            7 => Self::Reserve0,
            8 => Self::Quad,
            9 => Self::Polygon,
            10 => Self::Decal,
            11 => Self::DecalRing,
            12 => Self::Disc,
            13 => Self::LightModel,
            14 => Self::ModelSkin,
            15 => Self::Dissolve,
            other => Self::Unknown(other),
        }
    }
}

/// 绘制模式（`RMT`；VFXEditor `DrawMode`）：0 Blend、2 Add 等。
pub const DRAW_MODE_BLEND: i32 = 0;
pub const DRAW_MODE_MULTIPLY: i32 = 1;
pub const DRAW_MODE_ADD: i32 = 2;
pub const DRAW_MODE_SUBTRACT: i32 = 3;
pub const DRAW_MODE_SCREEN: i32 = 4;
pub const DRAW_MODE_REVERSE: i32 = 5;
pub const DRAW_MODE_MIN: i32 = 6;
pub const DRAW_MODE_MAX: i32 = 7;
pub const DRAW_MODE_OPACITY: i32 = 8;

/// 朝向基准（`RBDT`；VFXEditor `RotationDirectionBase`）。
pub mod rotation_direction_base {
    pub const X: i32 = 0;
    pub const Y: i32 = 1;
    pub const Z: i32 = 2;
    pub const MOVE_DIRECTION: i32 = 3;
    pub const BILLBOARD_AXIS_Y: i32 = 4;
    pub const SCREEN_BILLBOARD: i32 = 5;
    pub const CAMERA_BILLBOARD: i32 = 6;
    pub const MOVE_DIRECTION_BILLBOARD: i32 = 7;
    pub const CAMERA_BILLBOARD_AXIS_Y: i32 = 8;
    pub const TREE_BILLBOARD: i32 = 9;
    pub const NONE: i32 = 10;
}

/// 贴图颜色合成模式（`TCCT`；VFXEditor `TextureCalculateColor`）。
pub mod texture_calculate_color {
    pub const MULTIPLY: i32 = 0;
    pub const ADD: i32 = 1;
    pub const SUBTRACT: i32 = 2;
    pub const MAX: i32 = 3;
    pub const MIN: i32 = 4;
    pub const NONE: i32 = 5;
}

/// 贴图 alpha 合成模式（`TCAT`；VFXEditor `TextureCalculateAlpha`）。
pub mod texture_calculate_alpha {
    pub const MULTIPLY: i32 = 0;
    pub const MAX: i32 = 1;
    pub const MIN: i32 = 2;
    pub const NONE: i32 = 3;
}

/// 粒子 UV 动画集（`UvSt`）：缩放/滚动为两轴曲线容器（取 x/y），旋转为
/// 单轴曲线（取 z）。轴缺省时缩放 1、滚动/旋转 0。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxUvSet {
    /// `CUvT`: 0 uses the transformed vertex parameter; 1 samples by normalized
    /// fragment pixel position. Other values are preserved and diagnosed.
    pub calculate_uv: u32,
    pub scale: AvfxCurve2Axis,
    pub scroll: AvfxCurve2Axis,
    pub rotation: AvfxCurve,
    pub rotation_random: AvfxCurve,
}

/// 粒子贴图引用（`TC1`..`TC4`/`TN` 等同构子集）。真实武器文件里 TC1 常只
/// 写 `TLst`（贴图序号字节列表，VFXEditor 称 Mask Index）而不写 `TxNo`。
/// 客户端 TC1 忽略 `TxNo` 并按 `TLst`/`TxN` 选择；TC2..TC4 使用 `TxNo`。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTexture {
    pub enabled: bool,
    pub uv_set_index: i32,
    /// `TxNo`（TC2 及以后的主贴图序号；客户端 TC1 解析器不读取该字段）。
    pub texture_index: i32,
    /// `TLst[0]` 原值；运行时 128..255 为负值，无块为 -1。
    pub mask_texture_index: i32,
    /// `TCCT`：TC1 低 3 位非零时启用 RGB；TC2..TC4 使用颜色合成枚举。
    pub calculate_color: i32,
    /// `TCAT`：TC1 低 2 位非零时启用 alpha（bUSC 除外）；后续层使用 alpha 合成枚举。
    pub calculate_alpha: i32,
    /// `bC2A`：以 RGB 加权亮度替换 alpha，RGB 置白，忽略贴图自身 alpha。
    pub color_to_alpha: bool,
    /// `bUSC`/`bPFC`：屏幕拷贝/上一帧拷贝来源。
    pub use_screen_copy: bool,
    pub previous_frame_copy: bool,
    /// `bUOS`：角色肖像贴图（UI 场景）。
    pub use_chara_portrait: bool,
    /// `TFT`：Disable / Enable / High / VeryHigh / VeryVeryHigh（0..4）。
    /// 渲染端按 0 使用最近邻，1..4 使用线性过滤；高级质量档位尚未区分。
    pub texture_filter: i32,
    /// `TBUT`/`TBVT`：U/V 边界模式（0 Repeat、1 Clamp、2 Mirror）。
    pub texture_border_u: i32,
    pub texture_border_v: i32,
    /// `TxN`/`TxNR`：TC1 的贴图序号曲线及其随机项（按寿命选 TLst 里的贴图）。
    pub tex_n: Option<AvfxCurve>,
    pub tex_n_random: Option<AvfxCurve>,
    /// `TLst` 完整原始列表（运行时按 signed byte 消费；TxN 曲线索引整个池）。
    pub texture_list: Vec<i32>,
}

impl AvfxParticleTexture {
    pub(crate) fn tc1_has_animated_selection(&self) -> bool {
        self.tex_n
            .as_ref()
            .is_some_and(|curve| !curve.keys.is_empty())
            || self
                .tex_n_random
                .as_ref()
                .is_some_and(|curve| !curve.keys.is_empty())
    }

    /// 客户端 TC1 内建来源；优先级与 `0x1403a53e0` 一致。
    pub fn tc1_builtin_source(&self) -> Option<i32> {
        if self.use_screen_copy {
            Some(if self.previous_frame_copy { -3 } else { -2 })
        } else if self.use_chara_portrait {
            Some(-5)
        } else {
            None
        }
    }

    /// 客户端 TC1 的静态来源：内建源优先，否则读取首个 signed-byte
    /// `TLst` 项；客户端不会在列表为空或首项为 255 时回退 `TxNo`。
    pub fn tc1_static_texture(&self) -> i32 {
        self.tc1_builtin_source().unwrap_or_else(|| {
            self.texture_list
                .first()
                .map_or(-1, |index| i32::from(*index as u8 as i8))
        })
    }

    /// 格式层的静态引用便利值：有效 `TLst[0]` 优先，否则回退 `TxNo`。
    /// 客户端 TC1 运行时不使用这个回退；动态/内建源规则在采样器中处理。
    pub fn effective_texture(&self) -> i32 {
        if self.is_shape_mask() {
            self.mask_texture_index
        } else {
            self.texture_index
        }
    }

    /// 使用有效的 TLst 引用（VFXEditor 的 Mask Index）；这只决定资源选择，
    /// 不决定颜色方程，颜色转 alpha 由 `bC2A` 控制。
    pub fn is_shape_mask(&self) -> bool {
        (0..128).contains(&self.mask_texture_index)
    }
}

/// `TN` 法线贴图（Model / LightModel 预览已消费；完整客户端场景光照仍缺失）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTextureNormal {
    pub enabled: bool,
    pub uv_set_index: i32,
    pub texture_filter: i32,
    pub texture_border_u: i32,
    pub texture_border_v: i32,
    pub texture_index: i32,
    /// `NPow`：法线强度曲线。
    pub power: AvfxCurve,
}

/// `TR` 反射贴图（Model / LightModel 预览消费 Rate/RPow/TCCT 和 TEX cube；客户端场景内容仍缺失）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTextureReflection {
    pub enabled: bool,
    pub use_screen_copy: bool,
    pub texture_filter: i32,
    pub calculate_color: i32,
    pub texture_index: i32,
    /// `Rate`：反射混入比例曲线。
    pub rate: AvfxCurve,
    /// `RPow`：反射强度曲线。
    pub power: AvfxCurve,
}

/// `TP` 调色板贴图（按 `POff`/`POfR` 在调色板内取色）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTexturePalette {
    pub enabled: bool,
    pub texture_filter: i32,
    pub texture_border: i32,
    pub texture_index: i32,
    /// `POff`/`POfR`：调色板偏移曲线及其随机项。
    pub offset: AvfxCurve,
    pub offset_random: AvfxCurve,
}

/// 通用字段容器：Data 块里的叶子字段原样保留，保证类型未建模时数据也不丢。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxGenericData {
    /// 完整 Data 负载，包括未知字段、重复字段及其顺序。
    pub raw_payload: Vec<u8>,
    /// 4 字节标量叶子的原始位模式（i32 读值，浮点字段需按位还原）。
    pub scalars: BTreeMap<String, i32>,
    /// 单轴曲线容器（含 Keys 子块）。
    pub curves: BTreeMap<String, AvfxCurve>,
    /// 多轴曲线容器（含 X/Y/Z 子块）。
    pub curve3s: BTreeMap<String, AvfxCurve3Axis>,
    /// 颜色曲线容器（含 RGB 子块）。
    pub color_curves: BTreeMap<String, AvfxColorCurve>,
}

/// Line 粒子 Data（VFXEditor `AvfxParticleDataLine`）。`source` 保留未知字段。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataLine {
    pub line_count: i32,
    pub length: AvfxCurve,
    pub length_random: AvfxCurve,
    pub color_begin: AvfxColorCurve,
    pub color_end: AvfxColorCurve,
    pub source: AvfxGenericData,
}

/// Laser 粒子 Data（VFXEditor `AvfxParticleDataLaser`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataLaser {
    pub length: AvfxCurve,
    pub length_random: AvfxCurve,
    pub width: AvfxCurve,
    pub width_random: AvfxCurve,
    pub source: AvfxGenericData,
}

/// Polyline 粒子 Data（VFXEditor `AvfxParticleDataPolyline`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataPolyline {
    pub create_line_type: i32,
    pub not_billboard_base_axis_type: i32,
    pub bind_weapon_type: i32,
    pub point_count: i32,
    pub point_count_center: i32,
    pub point_count_end_distortion: i32,
    pub use_edge: bool,
    pub not_billboard: bool,
    pub bind_weapon: bool,
    pub connect_target: bool,
    pub connect_target_reverse: bool,
    pub tag_number: i32,
    pub is_spline: bool,
    pub is_local: bool,
    pub cf: AvfxCurve,
    pub cf_random: AvfxCurve,
    pub width: AvfxCurve,
    pub width_random: AvfxCurve,
    pub width_begin: AvfxCurve,
    pub width_begin_random: AvfxCurve,
    pub width_center: AvfxCurve,
    pub width_center_random: AvfxCurve,
    pub width_end: AvfxCurve,
    pub width_end_random: AvfxCurve,
    pub length: AvfxCurve,
    pub length_random: AvfxCurve,
    pub softness: AvfxCurve,
    pub softness_random: AvfxCurve,
    pub point_distortion: AvfxCurve,
    pub color_begin: AvfxColorCurve,
    pub color_center: AvfxColorCurve,
    pub color_end: AvfxColorCurve,
    pub color_edge_begin: AvfxColorCurve,
    pub color_edge_center: AvfxColorCurve,
    pub color_edge_end: AvfxColorCurve,
    pub source: AvfxGenericData,
}

/// Decal 粒子 Data（VFXEditor `AvfxParticleDataDecal`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataDecal {
    /// `SS`：格式层保存为 float；绘制端对 Decal 的局部 X/Z 或 Ring 宽度作距离补偿。
    pub scaling_scale: f32,
    /// `DDTT`：保留原始枚举；客户端按固定表分派普通/覆盖渲染状态组。
    pub ddtt: i32,
    pub source: AvfxGenericData,
}

/// DecalRing 粒子 Data（VFXEditor `AvfxParticleDataDecalRing`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataDecalRing {
    pub width: AvfxCurve,
    pub width_random: AvfxCurve,
    pub scaling_scale: f32,
    pub ring_fan: f32,
    pub ddtt: i32,
    pub source: AvfxGenericData,
}

/// Disc particle Data (VFXEditor `AvfxParticleDataDisc`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataDisc {
    pub parts_count: i32,
    pub parts_count_u: i32,
    pub parts_count_v: i32,
    pub point_interval_factor_v: f32,
    pub angle: AvfxCurve,
    pub angle_random: AvfxCurve,
    pub height_begin_inner: AvfxCurve,
    pub height_begin_inner_random: AvfxCurve,
    pub height_end_inner: AvfxCurve,
    pub height_end_inner_random: AvfxCurve,
    pub height_begin_outer: AvfxCurve,
    pub height_begin_outer_random: AvfxCurve,
    pub height_end_outer: AvfxCurve,
    pub height_end_outer_random: AvfxCurve,
    pub width_begin: AvfxCurve,
    pub width_begin_random: AvfxCurve,
    pub width_end: AvfxCurve,
    pub width_end_random: AvfxCurve,
    pub radius_begin: AvfxCurve,
    pub radius_begin_random: AvfxCurve,
    pub radius_end: AvfxCurve,
    pub radius_end_random: AvfxCurve,
    pub color_edge_inner: AvfxColorCurve,
    pub color_edge_outer: AvfxColorCurve,
    /// Integer percentage converted to a float by the client at runtime.
    pub scaling_scale: i32,
    pub source: AvfxGenericData,
}

/// Polygon particle Data (VFXEditor `AvfxParticleDataPolygon`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataPolygon {
    pub count: AvfxCurve,
    pub count_random: AvfxCurve,
    pub source: AvfxGenericData,
}

/// Dawntrail ModelSkin particle Data (VFXEditor `AvfxParticleDataModelSkin`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataModelSkin {
    pub fresnel_type: i32,
    pub aura_target: i32,
    pub cm: i32,
    pub fresnel_curve: AvfxCurve,
    pub fresnel_curve_random: AvfxCurve,
    pub fresnel_rotation: AvfxCurve3Axis,
    pub color_begin: AvfxColorCurve,
    pub color_end: AvfxColorCurve,
    pub sem: AvfxCurve,
    pub sem_random: AvfxCurve,
    pub eem: AvfxCurve,
    pub eem_random: AvfxCurve,
    pub uv_point_density: AvfxCurve3Axis,
    pub source: AvfxGenericData,
}

/// Nested `EdC` container used by Dawntrail Dissolve particles.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataDissolveColor {
    pub start: AvfxColorCurve,
    pub middle: AvfxColorCurve,
    pub end: AvfxColorCurve,
    pub scale_r: AvfxCurve,
    pub scale_g: AvfxCurve,
    pub scale_b: AvfxCurve,
    pub brightness: AvfxCurve,
}

/// Dawntrail Dissolve particle Data (VFXEditor `AvfxParticleDataDissolve`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDataDissolve {
    pub reverse: bool,
    pub bst: i32,
    pub npt: i32,
    pub dissolve_target: i32,
    pub erosion_rate: AvfxCurve,
    pub end_color_width: AvfxCurve,
    pub color: AvfxParticleDataDissolveColor,
    pub mid_color_width: AvfxCurve,
    pub start_color_width: AvfxCurve,
    pub intensity: AvfxCurve,
    pub source: AvfxGenericData,
}

/// 粒子 `Data` 块（按粒子类型解析）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvfxParticleData {
    /// 未写 Data 块。
    #[default]
    None,
    /// Model 粒子（字段对齐 VFXEditor `AvfxParticleDataModel`）。
    Model {
        /// `MNRv`/`MNRt`/`MNRi`：模型序号随机池参数。
        model_number_random_value: i32,
        model_number_random_type: i32,
        model_number_random_interval: i32,
        fresnel_type: i32,
        directional_light_type: i32,
        point_light_type: i32,
        is_lightning: bool,
        is_morph: bool,
        /// `MdNo` 模型序号字节列表（运行时按 signed byte 消费；`NoAn` 选择池项）。
        model_indexes: Vec<i32>,
        /// `NoAn` 动画序号曲线。
        animation_number: Option<AvfxCurve>,
        /// `Moph` 形变曲线。
        morph: Option<AvfxCurve>,
        /// `FrC`/`FrCR`/`FrRt`：菲涅尔曲线、其随机项与旋转。
        fresnel_curve: Option<AvfxCurve>,
        fresnel_curve_random: Option<AvfxCurve>,
        fresnel_rotation: Option<AvfxCurve3Axis>,
        color_begin: AvfxColorCurve,
        color_end: AvfxColorCurve,
    },
    /// LightModel 粒子：`MNO` 模型序号，支持无符号单字节及有符号短/整型标量。
    LightModel {
        model_index: i32,
    },
    /// Powder 粒子（子粒子发射器）：`bMV`/`bLoc`/`bLgt`/`LgtT`/`CnOf`。
    Powder {
        use_character_movement: bool,
        use_character_location: bool,
        is_lightning: bool,
        directional_light_type: i32,
        center_offset: f32,
    },
    /// Windmill: WUvT low byte selects Default (0) or Mirror (1).
    Windmill {
        uv_type: i32,
    },
    Line(AvfxParticleDataLine),
    Laser(AvfxParticleDataLaser),
    Polyline(AvfxParticleDataPolyline),
    Decal(AvfxParticleDataDecal),
    DecalRing(AvfxParticleDataDecalRing),
    Disc(AvfxParticleDataDisc),
    Polygon(AvfxParticleDataPolygon),
    ModelSkin(AvfxParticleDataModelSkin),
    Dissolve(AvfxParticleDataDissolve),
    /// Quad 粒子：`SS`/`bMP`。
    Quad {
        scaling_scale: i32,
        is_movement_particle: bool,
    },
    /// 其余类型（Disc/Polygon/Decal/…）：
    /// 全字段原样保留在通用容器里。
    Other(AvfxGenericData),
}

impl AvfxParticleData {
    /// 完整原始 Data 视图，供诊断与尚未建模字段继续使用。
    pub fn generic_data(&self) -> Option<&AvfxGenericData> {
        match self {
            Self::Line(data) => Some(&data.source),
            Self::Laser(data) => Some(&data.source),
            Self::Polyline(data) => Some(&data.source),
            Self::Decal(data) => Some(&data.source),
            Self::DecalRing(data) => Some(&data.source),
            Self::Disc(data) => Some(&data.source),
            Self::Polygon(data) => Some(&data.source),
            Self::ModelSkin(data) => Some(&data.source),
            Self::Dissolve(data) => Some(&data.source),
            Self::Other(data) => Some(data),
            _ => None,
        }
    }
}

/// Powder 粒子的 `Smpl`（简单动画/子粒子发射参数），字段对齐
/// VFXEditor `AvfxParticleSimple`。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleSimple {
    /// `SIPT`/`SIDT`/`SBDT`：出生位置、出生方向、基础方向类型。
    pub injection_position_type: i32,
    pub injection_direction_type: i32,
    pub base_direction_type: i32,
    /// `CCnt`：子粒子槽位数，死亡后可按 bCrN 复用。
    pub create_count: i32,
    /// `CrAX`/`CrAY`/`CrAZ`：出生点随机散布半径。
    pub create_area: [f32; 3],
    /// `CAX`/`CAY`/`CAZ`：Coord Accuracy；客户端每 0.1 帧乘到对应速度分量。
    pub coord_accuracy: [f32; 3],
    /// `CGX`/`CGY`/`CGZ`：子粒子重力。
    pub coord_gravity: [f32; 3],
    /// `SBX`/`SBY` 与 `SEX`/`SEY`：四角半尺寸的插值端点。
    pub scale_start: [f32; 2],
    pub scale_end: [f32; 2],
    /// `SC`：尺寸插值曲线指数。
    pub scale_curve: f32,
    /// `SRX0`/`SRX1`、`SRY0`/`SRY1`：尺寸随机倍率区间。
    pub scale_rand_x: [f32; 2],
    pub scale_rand_y: [f32; 2],
    /// `RIX`/`RIY`/`RIZ`：初相位，客户端叠加 RB 随机项后转为 1024/周的 short。
    pub rotation_start: [f32; 3],
    /// `RAX`/`RAY`/`RAZ`：每帧旋转增量。
    pub rotation_add: [f32; 3],
    /// `RBX`/`RBY`/`RBZ`：初相位随机幅度（正负区间）。
    pub rotation_base: [f32; 3],
    /// `RVX`/`RVY`/`RVZ`：旋转增量随机幅度（正负区间）。
    pub rotation_velocity: [f32; 3],
    /// `VMin`/`VMax`：子粒子初速度区间（单位/帧）。
    pub velocity_min: f32,
    pub velocity_max: f32,
    /// `FltR`/`FltS`：Flattery 参数，客户端用于顶点绑定插值，尚未完整消费。
    pub velocity_flattery_rate: f32,
    pub velocity_flattery_speed: f32,
    /// `UvCU`/`UvCV`/`UvIv`：UV 格子数与翻页间隔；间隔非正时停在起始格。
    pub uv_cell: [i32; 2],
    pub uv_interval: i32,
    /// `UvNR`：随机起点上限（含端点），取模总格数；客户端消费 signed byte。
    pub uv_no_random: i32,
    /// `UvLC`：客户端按 signed byte 解释；负值循环后停格，正值循环后死亡/重建。
    pub uv_loop_count: i32,
    /// `IJMN`：出生点引用的发射模型序号；Powder 按 signed byte 消费（-1 = 无模型点）。
    pub injection_model_index: i32,
    /// `VBMN`：出生点绑定顶点的模型序号；Powder 按 signed byte 消费（-1 = 不绑定）。
    pub injection_vertex_bind_model_index: i32,
    /// `IRD0`/`IRD1`：径向出生方向区间。
    pub injection_radial_dir: [f32; 2],
    /// `PvtX`/`PvtY`：旋转前四角偏移，以当前半尺寸为单位。
    pub pivot: [f32; 2],
    /// `BlkN`：每组槽位数，共享初始出生延迟、翻转与部分运动属性。
    pub block_num: i32,
    /// `LLin`/`LLax`：Smpl Line 每槽线段的最小/最大长度。
    pub line_length_min: f32,
    pub line_length_max: f32,
    /// `CrI`：子粒子创建间隔（帧）。
    pub create_interval: i32,
    /// `CIM`/`CIMR`：移动创建间隔与其随机项。
    pub create_interval_on_movement: f32,
    pub create_interval_on_movement_random: f32,
    /// `CrIR`：创建间隔随机项。
    pub create_interval_random: i32,
    /// `CrIC`：每次创建个数。
    pub create_interval_count: i32,
    /// `CrIL`：子粒子寿命（帧）。
    pub create_interval_life: i32,
    /// `CrLR`：各槽位的整数寿命偏移幅度，同时偏移颜色帧，重建时复用。
    pub create_life_random: i32,
    /// `bCrN`：子粒子死亡后重建。
    pub create_new_after_delete: bool,
    /// `bRUV`：随机选择是否翻转 U，同 block 的槽位共享翻转标志。
    pub uv_reverse: bool,
    /// `bSRL`：将 X 的量化随机倍率复制给 Y。
    pub scale_random_link: bool,
    /// `bBnP`：子粒子绑定父粒子位置。
    pub bind_parent: bool,
    /// `bSnP`：子粒子尺寸随父粒子缩放。
    pub scale_by_parent: bool,
    /// `PolT`：polyline 标签。
    pub polyline_tag: i32,
    /// `Cols`：4 段颜色（RGBA 字节），`Frms`：对应的 4 个帧号。
    pub colors: [[u8; 4]; 4],
    pub frames: [i16; 4],
}

/// 粒子扭曲贴图（`TD`）：采样扭曲贴图，把 (rg − 0.5) × `DPow` 加到目标
/// UV 坐标上（光罩/流光贴图的有机抖动来源）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleDistortion {
    pub enabled: bool,
    /// `bT1`..`bT4`：扭曲第 1..4 组 UV（UvSet 0..3）。
    pub target_uv: [bool; 4],
    /// 扭曲贴图采样用的 UvSet 序号（`UvSN`）。
    pub uv_set_index: i32,
    pub texture_index: i32,
    /// `DPow` 扭曲强度曲线。
    pub power: AvfxCurve,
    /// `TFT`：过滤模式。
    pub texture_filter: i32,
    /// `TBUT`/`TBVT`：扭曲贴图边界模式。
    pub texture_border_u: i32,
    pub texture_border_v: i32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxScheduler {
    /// Authored ItCn. The client reader stores its low byte, then ordinary
    /// dispatch sign-extends that byte. None lets synthetic definitions derive
    /// it from the mapping array; retain explicit counts independently.
    pub item_count: Option<i32>,
    pub items: Vec<AvfxSchedulerItem>,
    /// 固定 12 个调度触发器（拔刀/收刀等）；TRG/RTRG Clip 也可从此表创建 Timeline。
    pub triggers: Vec<AvfxSchedulerItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvfxTimelineClipType {
    Kill,
    Reset,
    End,
    FadeIn,
    UnlockLoopPoint,
    Trigger,
    RandomTrigger,
}

impl AvfxTimelineClipType {
    fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0x4B49_4C4C => Some(Self::Kill),
            0x5245_5354 => Some(Self::Reset),
            0x454E_4420 => Some(Self::End),
            0x4641_4449 => Some(Self::FadeIn),
            0x554C_4C50 => Some(Self::UnlockLoopPoint),
            0x5452_4720 => Some(Self::Trigger),
            0x5254_5247 => Some(Self::RandomTrigger),
            _ => None,
        }
    }
}

/// Signed one-based selectors used by KILL/FADI/ULLP callbacks. Positive
/// numbers select an ordinary Scheduler Item; negative numbers select a
/// Scheduler trigger slot identity. The slot is not the mapped Timeline
/// definition index or a live-child ordinal (factory 0x1403bc400 stores its
/// input slot at Timeline +0x23c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AvfxTimelineClipTarget {
    SchedulerItem { index: i32 },
    SchedulerTrigger { index: i32 },
}

impl AvfxTimelineClipTarget {
    pub fn from_signed_selector(selector: i32) -> Option<Self> {
        if selector == 0 {
            return None;
        }
        // Native neg/dec are wrapping, including INT_MIN -> INT_MAX.
        let index = selector.wrapping_abs().wrapping_sub(1);
        Some(if selector < 0 {
            Self::SchedulerTrigger { index }
        } else {
            Self::SchedulerItem { index }
        })
    }

    /// Compare the identities at native Timeline +0x238/+0x23c. Selection
    /// walks Scheduler children and the Timelines directly under type-9
    /// Binder children; this predicate does not perform that traversal.
    pub fn matches_identity(self, scheduler_item: i32, trigger_slot: i32) -> bool {
        match self {
            Self::SchedulerItem { index } => index == scheduler_item,
            Self::SchedulerTrigger { index } => index == trigger_slot,
        }
    }
}

/// Parameters consumed by the original factory/attachment callbacks. These
/// intentionally do not reinterpret VFXEditor's KILL UI hints as runtime
/// switches. Raw integer/float arrays and opaque bytes remain preserved.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AvfxTimelineClipParameters {
    Kill {
        fade_out_duration: i32,
        targets: [Option<AvfxTimelineClipTarget>; 3],
        /// Common fade-control mode, 2 for negative Float1, otherwise 3.
        fade_mode: u8,
        /// R9b of recursive Common fade control sets bit 21, inhibiting fade
        /// completion retirement (not life expiry). It is not the editor's
        /// differently mapped "Hide" checkbox.
        fade_flag: bool,
        end_document_on_retire: bool,
    },
    Reset {
        document: bool,
        /// Int1..3 minus one, without absolute value or zero suppression.
        scheduler_items: [i32; 3],
    },
    End,
    FadeIn {
        duration: i32,
        targets: [Option<AvfxTimelineClipTarget>; 3],
    },
    UnlockLoopPoint {
        all_scheduler: bool,
        targets: [Option<AvfxTimelineClipTarget>; 3],
    },
    Trigger {
        trigger: i32,
    },
    RandomTrigger {
        minimum_trigger: i32,
        maximum_trigger: i32,
    },
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxTimelineClip {
    pub clip_type: Option<AvfxTimelineClipType>,
    pub parameters: Option<AvfxTimelineClipParameters>,
    pub raw_type: Option<u32>,
    pub raw_ints: Option<[i32; 4]>,
    pub raw_floats: Option<[f32; 4]>,
    /// Includes the reserved bytes and any future extension bytes.
    pub raw_payload: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxTimeline {
    pub loop_start: i32,
    pub loop_end: i32,
    pub binder_index: i32,
    pub items: Vec<AvfxTimelineItem>,
    pub clip_count: usize,
    pub clips: Vec<AvfxTimelineClip>,
}

pub(crate) fn signed_byte_index(raw: i32) -> i32 {
    i32::from(raw as i8)
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEmitter {
    pub emitter_type: Option<EmitterType>,
    pub raw_emitter_type: u32,
    pub loop_start: i32,
    pub loop_end: i32,
    pub child_limit: i32,
    pub particle_count: i32,
    pub emitter_count: i32,
    /// `ROT`：旋转顺序（VFXEditor `RotationOrder`：0 XYZ、1 YZX、2 ZXY…）。
    pub rotation_order: i32,
    /// `RBDT`：发射器朝向基准。
    pub rotation_direction_base: i32,
    /// `CCOT`：坐标计算顺序（0 SRT、1 TSR、2 RTS、3 TRS、4 STR、5 RST）。
    pub coord_compute_order: i32,
    /// `bAD`：任意方向发射。
    pub any_direction: bool,
    /// `EfNo`：效果器序号（-1 无）。
    pub effector_index: i32,
    /// 子发射器的名义寿命；Timeline 根实例使用条目的持续时间。
    pub life: AvfxLife,
    pub create_count: AvfxCurve,
    pub create_count_random: AvfxCurve,
    pub create_interval: AvfxCurve,
    pub create_interval_random: AvfxCurve,
    /// `Gra`/`GraR`：发射器自身沿世界 Y 的重力，不是子粒子的加速度。
    pub gravity: AvfxCurve,
    pub gravity_random: AvfxCurve,
    /// `ARs`/`ARsR`：空气阻力。
    pub air_resistance: AvfxCurve,
    pub air_resistance_random: AvfxCurve,
    pub color: AvfxColorCurve,
    pub position: AvfxCurve3Axis,
    pub rotation: AvfxCurve3Axis,
    pub scale: AvfxCurve3Axis,
    /// `IAX`/`IAY`/`IAZ`（+`R` 随机）：注入角（弧度）。
    pub injection_angle: [AvfxCurve; 3],
    pub injection_angle_random: [AvfxCurve; 3],
    /// `VRX`/`VRY`/`VRZ`: direction angles for this emitter's own injected motion.
    pub rotation_velocity: [AvfxCurve; 3],
    /// `VRXR`/`VRYR`/`VRZR`: random counterparts of the emitter direction curves.
    pub rotation_velocity_random: [AvfxCurve; 3],
    pub particle_items: Vec<AvfxEmitterItem>,
    pub emitter_items: Vec<AvfxEmitterItem>,
    /// 形状/模型发射数据（按 `EVT` 解析 `Data` 块）。
    pub data: Option<AvfxEmitterData>,
    /// `Data` 原始负载，保留未知类型、未知字段与重复字段顺序。
    pub data_payload: Vec<u8>,
}

/// 发射器 `Data` 块（按类型）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvfxEmitterData {
    #[default]
    None,
    Cone(ConeEmitterData),
    ConeModel(ConeModelEmitterData),
    SphereModel(SphereModelEmitterData),
    CylinderModel(CylinderModelEmitterData),
    Model(ModelEmitterData),
}

/// 形状 Data 自身的旋转参数；独立于 Emit 的 Rot 和 IAX/IAY/IAZ。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitterDataRotation {
    /// `ROT`：旋转顺序。
    pub order: i32,
    /// `AnX`/`AnY`/`AnZ` 及对应的 `AnXR`/`AnYR`/`AnZR`。
    pub angles: [AvfxCurve; 3],
    pub angles_random: [AvfxCurve; 3],
}

/// 圆锥发射数据（类型 1）：`InS`/`OuS` 沿散射方向的出生距离、`IjS` 注入速度、
/// `IjA` 绕随机 XY 轴偏离 +Z 的最大正负角度；Data.ROT 随后旋转形状。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConeEmitterData {
    pub rotation: EmitterDataRotation,
    pub inner_size: AvfxCurve,
    pub inner_size_random: AvfxCurve,
    pub outer_size: AvfxCurve,
    pub outer_size_random: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_speed_random: AvfxCurve,
    pub injection_angle: AvfxCurve,
    pub injection_angle_random: AvfxCurve,
}

/// 圆锥模型发射数据（类型 2）：`GeMT` 生成方式、`DivX`/`DivY` 分割数、
/// `Rad` 半径、`IjS` 注入速度、`IjA` 注入角。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConeModelEmitterData {
    pub rotation: EmitterDataRotation,
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub radius: AvfxCurve,
    pub radius_random: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_speed_random: AvfxCurve,
    pub injection_angle: AvfxCurve,
    pub injection_angle_random: AvfxCurve,
}

/// 球模型发射数据（类型 3）：`Rads` 半径、`IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SphereModelEmitterData {
    pub rotation: EmitterDataRotation,
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_speed_random: AvfxCurve,
}

/// 圆柱模型发射数据（类型 4）：`Len` 长度、`Rad` 半径、`IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CylinderModelEmitterData {
    pub rotation: EmitterDataRotation,
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub length: AvfxCurve,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_speed_random: AvfxCurve,
}

/// 模型发射数据（类型 5）：`MdNo` 发射模型序号、`GeMT` 生成方式、
/// `IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEmitterData {
    pub rotation: EmitterDataRotation,
    pub model_index: i32,
    pub generate_method: i32,
    pub injection_speed: AvfxCurve,
    pub injection_speed_random: AvfxCurve,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticle {
    pub particle_type: Option<ParticleType>,
    pub raw_particle_type: u32,
    pub loop_start: i32,
    pub loop_end: i32,
    /// 绘制模式（`RMT`：0 Blend，2 Add……）。
    pub draw_mode: i32,
    /// 原始 `CulT`：0 双面、1 留正面、2 留背面、3 Double。
    /// 客户端 1/2 分别使用 CULL_BACK / CULL_FRONT；网格 Double 先背面后正面。
    pub culling_type: i32,
    pub depth_test: bool,
    pub depth_write: bool,
    /// `RBDT`：朝向基准（billboard 与否由它决定）。
    pub rotation_direction_base: i32,
    /// `RoOT`：旋转顺序。
    pub rotation_order: i32,
    /// `CCOT`：坐标计算顺序。
    pub coord_compute_order: i32,
    /// `EnvT`/`DirT`：环境光/平行光类型。
    pub env_light_type: i32,
    pub dir_light_type: i32,
    /// `UVPT`：UV 精度。
    pub uv_precision: i32,
    /// `DwPr`：绘制优先级（同发射器内排序，越小先画）。
    pub draw_priority: i32,
    /// `DsSp`：软粒子（深度衰减；由支持的几何在独立场景深度 pass 消费）。
    pub is_soft_particle: bool,
    /// `Coll`：碰撞类型。
    pub collision_type: i32,
    /// `bS11`/`ShUT`/`ShR`/`ShT`/`UniV`/`HybV`/`bE24`：阴影与 Dawntrail 新增参数。
    pub s11_enabled: bool,
    pub sh_u_t: i32,
    pub sh_r: i32,
    pub sh_t: i32,
    pub uni_v: i32,
    pub hyb_v: i32,
    pub e24_enabled: bool,
    /// `bATM`/`bAFg`：是否参与 tone map / 雾。
    pub is_apply_tone_map: bool,
    pub is_apply_fog: bool,
    /// `bNea`/`bFar` 与 `NeSt`/`NeEd`/`FaSt`/`FaEd`/`FaBP`：近/远距离裁剪。
    pub clip_near_enable: bool,
    pub clip_far_enable: bool,
    pub clip_near_start: f32,
    pub clip_near_end: f32,
    pub clip_far_start: f32,
    pub clip_far_end: f32,
    pub clip_base_point: i32,
    /// `EvAR`/`DlAR`/`LBAR`：环境光/平行光/光缓冲应用率。
    pub apply_rate_environment: i32,
    pub apply_rate_directional: i32,
    pub apply_rate_light_buffer: i32,
    /// `DOTy`/`DpOf`：深度偏移类型与值。
    pub depth_offset_type: i32,
    pub depth_offset: f32,
    /// `bSCt`：启用 Smpl 简单动画。
    pub simple_anim_enable: bool,
    pub life: AvfxLife,
    pub gravity: AvfxCurve,
    /// `GraR`：重力随机曲线。
    pub gravity_random: AvfxCurve,
    pub air_resistance: AvfxCurve,
    /// `ARsR`：空气阻力随机曲线。
    pub air_resistance_random: AvfxCurve,
    pub scale: AvfxCurve3Axis,
    pub rotation: AvfxCurve3Axis,
    pub position: AvfxCurve3Axis,
    pub color: AvfxColorCurve,
    /// Particle `VRX/Y/Z`: instantaneous injection-direction angles, not drawing angular velocity.
    pub rotation_velocity: [AvfxCurve; 3],
    /// `VRXR`/`VRYR`/`VRZR`: random counterparts of the injection-direction curves.
    pub rotation_velocity_random: [AvfxCurve; 3],
    pub texture_color1: Option<AvfxParticleTexture>,
    pub texture_color2: Option<AvfxParticleTexture>,
    pub texture_color3: Option<AvfxParticleTexture>,
    pub texture_color4: Option<AvfxParticleTexture>,
    /// `TN` 法线贴图。
    pub texture_normal: Option<AvfxParticleTextureNormal>,
    /// `TR` 反射贴图。
    pub texture_reflection: Option<AvfxParticleTextureReflection>,
    /// `TP` 调色板贴图。
    pub texture_palette: Option<AvfxParticleTexturePalette>,
    pub texture_distortion: Option<AvfxParticleDistortion>,
    pub uv_sets: Vec<AvfxUvSet>,
    /// `Data` 块（按粒子类型）。
    pub data: AvfxParticleData,
    /// `Smpl` 块（Powder 子粒子参数）。
    pub simple: Option<AvfxParticleSimple>,
}

impl AvfxParticle {
    /// 定义的名义寿命（帧）；未启用或负值表示没有寿命上限。
    /// 实例还会应用寿命随机/覆盖、时钟速率和局部循环；零不是永生。
    pub fn life_frames(&self) -> Option<f32> {
        if self.life.enabled && self.life.value >= 0.0 {
            Some(self.life.value)
        } else {
            None
        }
    }

    /// 是否相机朝向（billboard 系 RBDT 值）。
    pub fn is_billboard(&self) -> bool {
        matches!(
            self.rotation_direction_base,
            rotation_direction_base::BILLBOARD_AXIS_Y
                | rotation_direction_base::SCREEN_BILLBOARD
                | rotation_direction_base::CAMERA_BILLBOARD
                | rotation_direction_base::MOVE_DIRECTION_BILLBOARD
                | rotation_direction_base::CAMERA_BILLBOARD_AXIS_Y
                | rotation_direction_base::TREE_BILLBOARD
        )
    }
}

/// Binder 属性组（`PrpS`/`Prp1`/`Prp2`/`PrpG`，字段对齐 VFXEditor
/// `AvfxBinderProperties`）。S=起始、1/2=插值、G=目标，引擎在它们之间渐变。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxBinderProperties {
    /// `BPT`：绑定对象（0 Caster、1 Target）。
    pub bind_point_type: i32,
    /// `BPTP`：目标点（0 Origin、1 FitGround、2 DamageCircle、3 ByName）。
    pub bind_target_point_type: i32,
    /// `Name`：按名绑定目标（角色骨骼名等）。
    pub binder_name: String,
    /// `BPID`：目标模型上的绑点序号（武器 MDL 的 ElementId：
    /// 3=基部 / 4=中部 / 5=尖部……；-1 = 未指定，保持原点）。
    pub bind_point_id: i32,
    /// `GenD`：生成延迟（帧）。
    pub generate_delay: i32,
    /// `CoUF`：目标查询的截止帧；运行时按有符号低 16 位消费。
    /// 负值持续查询，非负值包含截止帧本身；初始化首查不受此限制。
    pub coord_update_frame: i32,
    /// `bRng` 与 `RnPT`/`RnPX`-`RnPZ`/`RnRd`：环形绑点参数。
    /// Spline 的 Prp1/Prp2 使用同组字段作为内控制点开关、参数和偏移。
    pub ring_enabled: bool,
    /// 缺失 RnPT 保留原属性构造器的 0，不采用编辑器 UI 的初值 1。
    pub ring_progress_time: i32,
    pub ring_position: [f32; 3],
    pub ring_radius: f32,
    pub bct: i32,
    /// `Pos`：绑点位置曲线。
    pub position: AvfxCurve3Axis,
}

/// Binder `Data` curves named by VFXEditor; only restricted static motion is sampled.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxBinderData {
    /// Point binder.
    pub spring_strength: Option<AvfxCurve>,
    pub spring_strength_random: Option<AvfxCurve>,
    /// Linear/Spline use both curves; LinearAdjust only defines COF.
    pub carry_over_factor: Option<AvfxCurve>,
    pub carry_over_factor_random: Option<AvfxCurve>,
    /// Camera binder.
    pub distance: Option<AvfxCurve>,
    pub distance_random: Option<AvfxCurve>,
    pub source: AvfxGenericData,
}

/// `Bind` 绑点块（字段对齐 VFXEditor `AvfxBinder`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxBinder {
    /// `BnVr`：0 = Point、1 = Linear、2 = Spline、3 = Camera、4 = LinearAdjust。
    pub binder_type: u32,
    /// `properties_start.bind_point_id` 的便捷镜像（渲染主路径消费）。
    pub bind_point_id: i32,
    /// `bStG`：Linear/LinearAdjust 按双端目标计算全局方向。
    /// 已核对的 Point 构造、更新与子工厂不消费此位。
    pub start_to_global_direction: bool,
    /// `bVSc`/`bVSb`/`bVSd`/`bVSi`：VFX 缩放开关、偏置、深度偏移、插值。
    pub vfx_scale_enabled: bool,
    pub vfx_scale_bias: f32,
    pub vfx_scale_depth_offset: bool,
    pub vfx_scale_interpolation: bool,
    /// `bTSc`/`bTSd`/`bTSi`：变换缩放及其深度偏移/插值。
    pub transform_scale: i32,
    pub transform_scale_depth_offset: bool,
    pub transform_scale_interpolation: bool,
    /// `bFTO`：跟随目标朝向。
    pub following_target_orientation: bool,
    /// `bDSE`/`bATS`/`bIFY`/`bBET`。
    pub document_scale_enabled: bool,
    pub adjust_to_screen_enabled: bool,
    pub ify: bool,
    pub bet: bool,
    /// `Life`：binder 寿命（帧）。
    pub life: i32,
    /// `RoTp`：binder 旋转类型。
    pub rotation_type: i32,
    pub properties_start: Option<AvfxBinderProperties>,
    pub properties_1: Option<AvfxBinderProperties>,
    pub properties_2: Option<AvfxBinderProperties>,
    pub properties_goal: Option<AvfxBinderProperties>,
    /// `Data` retains type-specific spring, carry-over, or camera curves.
    pub data: Option<AvfxBinderData>,
}

/// Effector `Data` fields follow VFXEditor's type-specific layouts. None of
/// these fields is sampled by the preview yet.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvfxEffectorData {
    PointLight {
        color: AvfxColorCurve,
        distance_scale: Option<AvfxCurve>,
        rotation: Option<AvfxCurve3Axis>,
        position: Option<AvfxCurve3Axis>,
        attenuation: Option<i32>,
        enable_shadow: bool,
        enable_character_shadow: bool,
        enable_map_shadow: bool,
        enable_move_shadow: bool,
        shadow_create_distance_near: Option<f32>,
        shadow_create_distance_far: Option<f32>,
    },
    DirectionalLight {
        ambient: AvfxColorCurve,
        color: AvfxColorCurve,
        power: Option<AvfxCurve>,
        power_random: Option<AvfxCurve>,
        rotation: Option<AvfxCurve3Axis>,
    },
    RadialBlur(AvfxEffectorBlurData),
    GaussianBlur(AvfxEffectorBlurData),
    DirectionalBlur(AvfxEffectorBlurData),
    ChromaticAberration {
        refraction_strength: Option<AvfxCurve>,
        separation_strength: Option<AvfxCurve>,
        effect_strength: Option<AvfxCurve>,
        strength: Option<AvfxCurve>,
        gradation: Option<AvfxCurve>,
        inner_radius: Option<AvfxCurve>,
        outer_radius: Option<AvfxCurve>,
        fade_start_distance: Option<f32>,
        fade_end_distance: Option<f32>,
        fade_base_point: Option<i32>,
    },
    CameraQuake {
        attenuation: Option<AvfxCurve>,
        attenuation_random: Option<AvfxCurve>,
        radius_out: Option<AvfxCurve>,
        radius_out_random: Option<AvfxCurve>,
        radius_in: Option<AvfxCurve>,
        radius_in_random: Option<AvfxCurve>,
        rotation: Option<AvfxCurve3Axis>,
        position: Option<AvfxCurve3Axis>,
    },
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEffectorBlurData {
    pub length: Option<AvfxCurve>,
    pub strength: Option<AvfxCurve>,
    pub gradation: Option<AvfxCurve>,
    pub inner_radius: Option<AvfxCurve>,
    pub outer_radius: Option<AvfxCurve>,
    pub angle_strength: Option<AvfxCurve>,
    pub angle: Option<AvfxCurve>,
    pub fade_start_distance: Option<f32>,
    pub fade_end_distance: Option<f32>,
    pub fade_base_point: Option<i32>,
    pub one_side: bool,
}

/// `Efct` effectors retain both type-specific fields and the exact Data payload.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEffector {
    /// `EfVT`：效果器类型。
    pub raw_effector_type: u32,
    pub rotation_order: i32,
    pub coord_compute_order: i32,
    /// `bAOV`/`bAGm`：作用于其他 VFX / 游戏本体。
    pub affect_other_vfx: bool,
    pub affect_game: bool,
    pub loop_start: i32,
    pub loop_end: i32,
    pub data: Option<AvfxEffectorData>,
    /// Complete `Data` payload, including unknown and repeated fields.
    pub data_payload: Vec<u8>,
}

/// 发射模型顶点（`VEmt`，28 字节/顶点）：位置 + 法线（注入方向）+ 颜色。
/// 坐标为 avfx 模型空间，与武器模型空间同向（不翻转）。
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxEmitVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}

/// 内嵌模型（根级 `Modl` 块，每块一个模型）：
/// - `emit_vertices` 来自 `VEmt`，是 Model 发射器/粉末注入的出生点；
/// - `draw` 来自 `VDrw`/`VIdx`，是 Model/LightModel 粒子的渲染网格
///   （龙形火舌、光罩壳等轮廓本体）。两者可共存于同一 `Modl`。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxModelGeometry {
    pub emit_vertices: Vec<VfxEmitVertex>,
    /// `VNum` 原始 short 列表；Model 发射器按其 u16 位模式间接索引 VEmt。
    pub emit_vertex_numbers: Vec<i16>,
    pub draw: Option<VfxDrawModel>,
}

/// 绘制网格顶点（`VDrw`，36 字节/顶点）：half4 位置 + 4B 法线 + 4B 切线 +
/// 4B 顶点色 + 4 组 half2 UV。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDrawVertex {
    pub position: [f32; 3],
    pub position_w: f32,
    /// Raw UNORM bytes. Apricot shaders decode xyz as byte / 255 - 0.5.
    pub normal: [u8; 4],
    pub tangent: [u8; 4],
    /// 四组原始 UV（half2 ×4，以零为中心）；渲染时加 0.5 转成采样坐标，
    /// 各贴图层按 TCn 的 `UvSN` 选用其中一组作基底。
    pub uvs: [[f32; 2]; 4],
    pub color: [u8; 4],
}

/// 三角形索引（u32 化的 i16 三元组）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDrawModel {
    pub vertices: Vec<VfxDrawVertex>,
    pub indices: Vec<u32>,
}

/// 解析后的 avfx 文件子集；未消费的根级块按名计数进 `unknown_blocks`。
/// 根级全局参数（VFXEditor `AvfxMain`）：绘制层、裁剪盒、距离淡出、
/// 修正值（位置/旋转/缩放/颜色）等。大多数对武器常驻特效无影响，全量保留。
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxGlobalParameters {
    /// `bDFP`/`bFG`/`bTS`/`bASH`：延迟快速粒子、贴地、变换跳过、隐藏即停。
    pub is_delay_fast_particle: bool,
    pub is_fit_ground: bool,
    pub is_transform_skip: bool,
    pub is_all_stop_on_hide: bool,
    /// `bCBC`/`bCul` 与 `CBPx-CBPz`/`CBSx-CBSz`：裁剪盒。
    pub can_be_clipped_out: bool,
    pub clip_box_enabled: bool,
    pub clip_box: [f32; 3],
    pub clip_box_size: [f32; 3],
    /// `ZBMs`/`ZBMd`：Z 偏置。
    pub bias_z_max_scale: f32,
    pub bias_z_max_distance: f32,
    /// `bCmS`/`bFEL`/`bOSE`/`bOSt`。
    pub is_camera_space: bool,
    pub is_full_env_light: bool,
    pub ose: bool,
    pub is_clip_own_setting: bool,
    /// `NCB`/`NCE`/`FCB`/`FCE`：近/远裁剪。
    pub near_clip_begin: f32,
    pub near_clip_end: f32,
    pub far_clip_begin: f32,
    pub far_clip_end: f32,
    /// `SPFR`/`SKO`：软粒子淡出范围、排序键偏移。
    pub soft_particle_fade_range: f32,
    pub soft_key_offset: f32,
    /// `DwLy`/`DwOT`：绘制层与绘制序。
    pub draw_layer: i32,
    pub draw_order: i32,
    /// `DLST`/`PL1S`/`PL2S`：平行光源/点光源 1/2。
    pub directional_light_source: i32,
    pub point_light_1: i32,
    pub point_light_2: i32,
    /// `RvPx-RvPz`/`RvRx-RvRz`/`RvSx-RvSz`/`RvR/G/B`：修正值。
    pub revised_position: [f32; 3],
    pub revised_rotation: [f32; 3],
    pub revised_scale: [f32; 3],
    pub revised_color: [f32; 3],
    /// `AFXe`/`AFXi`/`AFXo`（X/Y/Z 三条）：各轴距离淡出。
    pub fade_enabled: [bool; 3],
    pub fade_inner: [f32; 3],
    pub fade_outer: [f32; 3],
    /// `bGFE`/`GFIM`：全局雾。
    pub global_fog_enabled: bool,
    pub global_fog_influence: f32,
    /// `bLTS`。
    pub lts_enabled: bool,
    /// `bAGS`: Point initializes its auxiliary matrix from root revision
    /// and retains it during ordinary queries (client root +0xf4).
    pub ags_enabled: bool,
    /// `APri`/`DPri`/`bSAB`/`bSBV`/`SBVa`/`bSSV`/`SSVa`/`SPHP`。
    pub a_pri: i32,
    pub d_pri: i32,
    pub sab_enabled: bool,
    pub sbv_enabled: bool,
    pub sbv_a: f32,
    pub ssv_enabled: bool,
    pub ssv_a: f32,
    pub sphp: i32,
}

impl Default for AvfxGlobalParameters {
    fn default() -> Self {
        Self {
            is_delay_fast_particle: Default::default(),
            is_fit_ground: Default::default(),
            is_transform_skip: Default::default(),
            is_all_stop_on_hide: Default::default(),
            can_be_clipped_out: Default::default(),
            clip_box_enabled: Default::default(),
            clip_box: Default::default(),
            clip_box_size: Default::default(),
            bias_z_max_scale: Default::default(),
            bias_z_max_distance: Default::default(),
            is_camera_space: Default::default(),
            is_full_env_light: Default::default(),
            ose: Default::default(),
            is_clip_own_setting: Default::default(),
            near_clip_begin: Default::default(),
            near_clip_end: Default::default(),
            far_clip_begin: Default::default(),
            far_clip_end: Default::default(),
            soft_particle_fade_range: Default::default(),
            soft_key_offset: Default::default(),
            draw_layer: Default::default(),
            draw_order: Default::default(),
            directional_light_source: Default::default(),
            point_light_1: Default::default(),
            point_light_2: Default::default(),
            revised_position: Default::default(),
            revised_rotation: Default::default(),
            revised_scale: [1.0; 3],
            revised_color: Default::default(),
            fade_enabled: Default::default(),
            fade_inner: Default::default(),
            fade_outer: Default::default(),
            global_fog_enabled: Default::default(),
            global_fog_influence: Default::default(),
            lts_enabled: Default::default(),
            ags_enabled: Default::default(),
            a_pri: Default::default(),
            d_pri: Default::default(),
            sab_enabled: Default::default(),
            sbv_enabled: Default::default(),
            sbv_a: Default::default(),
            ssv_enabled: Default::default(),
            ssv_a: Default::default(),
            sphp: Default::default(),
        }
    }
}

impl AvfxGlobalParameters {
    /// Runtime upload value for the soft-particle range. The parsed field is
    /// retained verbatim for audit output, while non-finite data must not
    /// reach a GPU uniform and poison the alpha calculation.
    pub fn effective_soft_particle_fade_range(&self) -> f32 {
        self.soft_particle_fade_range
            .is_finite()
            .then_some(self.soft_particle_fade_range)
            .unwrap_or(1.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxFile {
    pub version: u32,
    /// 根级全局参数（`AvfxMain`）。
    pub global: AvfxGlobalParameters,
    pub schedulers: Vec<AvfxScheduler>,
    pub timelines: Vec<AvfxTimeline>,
    pub emitters: Vec<AvfxEmitter>,
    pub particles: Vec<AvfxParticle>,
    pub binders: Vec<AvfxBinder>,
    /// 文件级 `Efct` 效果器。
    pub effectors: Vec<AvfxEffector>,
    /// 文件级 `Tex` 块的贴图路径（通常指向 .atex）。
    pub texture_paths: Vec<String>,
    /// 根级 `Modl` 模型（按文件顺序；发射顶点与绘制网格共存于同一块）。
    pub models: Vec<VfxModelGeometry>,
    pub warnings: Vec<String>,
    pub unknown_blocks: BTreeMap<String, usize>,
}

impl AvfxFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, AvfxParseError> {
        let root = AvfxNodeView::parse_root(bytes)?;
        let mut warnings = Vec::new();
        validate_known_containers(&root, &mut warnings);
        let mut unknown_blocks = BTreeMap::new();
        let mut file = Self {
            version: root.scalar("Ver").unwrap_or(0),
            ..Default::default()
        };

        for node in root.children() {
            match node.name() {
                "Ver" | "ScCn" | "TlCn" | "EmCn" | "PrCn" | "EfCn" | "BdCn" | "TxCn" | "MdCn" => {}
                "Schd" => file.schedulers.push(parse_scheduler(&node, &mut warnings)),
                "TmLn" => {
                    let index = file.timelines.len();
                    file.timelines
                        .push(parse_timeline(&node, index, &mut warnings));
                }
                "Emit" => {
                    file.emitters
                        .push(parse_emitter(&node, file.emitters.len(), &mut warnings))
                }
                "Ptcl" => {
                    file.particles
                        .push(parse_particle(&node, file.particles.len(), &mut warnings))
                }
                "Bind" => {
                    let index = file.binders.len();
                    let binder = parse_binder(&node);
                    warn_unsupported_binder(&binder, index, &mut warnings);
                    file.binders.push(binder);
                }
                "Efct" => {
                    let index = file.effectors.len();
                    let effector = parse_effector(&node);
                    if !effector.data_payload.is_empty() {
                        warnings.push(format!(
                            "Efct[{index}].Data: effector data is retained but not sampled"
                        ));
                    }
                    file.effectors.push(effector);
                }
                "Modl" => file
                    .models
                    .push(parse_model(&node, file.models.len(), &mut warnings)),
                "Tex" => file
                    .texture_paths
                    .push(read_null_terminated(node.payload())),
                name => {
                    if !GLOBAL_BLOCK_NAMES.contains(&name) {
                        *unknown_blocks.entry(name.to_string()).or_default() += 1;
                    }
                }
            }
        }
        for (count_name, actual) in [
            ("ScCn", file.schedulers.len()),
            ("TlCn", file.timelines.len()),
            ("EmCn", file.emitters.len()),
            ("PrCn", file.particles.len()),
            ("EfCn", file.effectors.len()),
            ("BdCn", file.binders.len()),
            ("TxCn", file.texture_paths.len()),
            ("MdCn", file.models.len()),
        ] {
            warn_count_mismatch(&root, count_name, actual, &mut warnings);
        }
        file.global = parse_global_parameters(&root);
        warn_unsupported_global_parameters(&file.global, &mut warnings);
        file.validate_references(&mut warnings);
        file.warnings = warnings;
        file.unknown_blocks = unknown_blocks;
        Ok(file)
    }

    fn validate_references(&self, warnings: &mut Vec<String>) {
        let mut check = |path: String, index: i32, target: &str, count: usize| {
            // VFXEditor resolves only nonnegative node indices. Preserve negative
            // sentinels, including BdNo=-2 present in client resources.
            if index >= 0 && index as usize >= count {
                warnings.push(format!(
                    "AVFX reference {path}: index {index} outside {target} table ({count} entries)"
                ));
            }
        };
        for (scheduler_index, scheduler) in self.schedulers.iter().enumerate() {
            for (kind, items) in [("Item", &scheduler.items), ("Trgr", &scheduler.triggers)] {
                for (item_index, item) in items.iter().enumerate().filter(|(_, item)| item.enabled)
                {
                    check(
                        format!("Schd[{scheduler_index}].{kind}[{item_index}].TlNo"),
                        item.timeline_index,
                        "TmLn",
                        self.timelines.len(),
                    );
                }
            }
        }
        for (timeline_index, timeline) in self.timelines.iter().enumerate() {
            check(
                format!("TmLn[{timeline_index}].BnNo"),
                signed_byte_index(timeline.binder_index),
                "Bind",
                self.binders.len(),
            );
            for (item_index, item) in timeline
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.enabled)
            {
                check(
                    format!("TmLn[{timeline_index}].Item[{item_index}].BdNo"),
                    signed_byte_index(item.binder_index),
                    "Bind",
                    self.binders.len(),
                );
                let reference = match item.target() {
                    AvfxTimelineItemTarget::Effector(index) => {
                        Some(("EfNo", index, "Efct".to_owned(), self.effectors.len()))
                    }
                    AvfxTimelineItemTarget::Emitter(index) => {
                        Some(("EmNo", index, "Emit".to_owned(), self.emitters.len()))
                    }
                    AvfxTimelineItemTarget::Clip(index) => Some((
                        "ClNo",
                        index,
                        format!("TmLn[{timeline_index}].Clip"),
                        timeline.clip_count,
                    )),
                    AvfxTimelineItemTarget::None => None,
                };
                if let Some((field, index, target, count)) = reference {
                    check(
                        format!("TmLn[{timeline_index}].Item[{item_index}].{field}"),
                        index,
                        &target,
                        count,
                    );
                }
            }
        }
        for (emitter_index, emitter) in self.emitters.iter().enumerate() {
            if let Some(AvfxEmitterData::Model(data)) = &emitter.data {
                check(
                    format!("Emit[{emitter_index}].Data.MdNo"),
                    i32::from(data.model_index as i8),
                    "Modl",
                    self.models.len(),
                );
            }
            check(
                format!("Emit[{emitter_index}].EfNo"),
                emitter.effector_index,
                "Efct",
                self.effectors.len(),
            );
            for (kind, items, target, count) in [
                (
                    "ItPr",
                    &emitter.particle_items,
                    "Ptcl",
                    self.particles.len(),
                ),
                ("ItEm", &emitter.emitter_items, "Emit", self.emitters.len()),
            ] {
                for (item_index, item) in items.iter().enumerate().filter(|(_, item)| item.enabled)
                {
                    check(
                        format!("Emit[{emitter_index}].{kind}[{item_index}].TgtB"),
                        item.target_index,
                        target,
                        count,
                    );
                }
            }
        }
        for (particle_index, particle) in self.particles.iter().enumerate() {
            let owner = format!("Ptcl[{particle_index}]");
            let mesh_textures_used = matches!(
                particle.particle_type,
                Some(ParticleType::Model | ParticleType::LightModel)
            );
            // Four UV slots exist even when their optional UvSt animations do not.
            let uv_table = format!("{owner}.UV slots");
            for (layer, texture) in [
                ("TC1", &particle.texture_color1),
                ("TC2", &particle.texture_color2),
                ("TC3", &particle.texture_color3),
                ("TC4", &particle.texture_color4),
            ] {
                let Some(texture) = texture.as_ref().filter(|texture| texture.enabled) else {
                    continue;
                };
                if layer != "TC1" {
                    check(
                        format!("{owner}.{layer}.TxNo"),
                        texture.texture_index,
                        "Tex",
                        self.texture_paths.len(),
                    );
                }
                check(
                    format!("{owner}.{layer}.UvSN"),
                    texture.uv_set_index & 7,
                    &uv_table,
                    4,
                );
                if layer == "TC1" {
                    let referenced = if texture.tc1_builtin_source().is_some() {
                        0
                    } else if texture.tc1_has_animated_selection() {
                        texture.texture_list.len() & 0xff
                    } else {
                        texture.texture_list.len().min(1)
                    };
                    for (slot, &index) in texture.texture_list[..referenced].iter().enumerate() {
                        check(
                            format!("{owner}.{layer}.TLst[{slot}]"),
                            i32::from(index as i8),
                            "Tex",
                            self.texture_paths.len(),
                        );
                    }
                }
            }
            let texture_refs = [
                particle
                    .texture_normal
                    .as_ref()
                    .filter(|t| t.enabled && mesh_textures_used)
                    .map(|t| ("TN", t.texture_index, Some(t.uv_set_index))),
                particle
                    .texture_distortion
                    .as_ref()
                    .filter(|t| t.enabled)
                    .map(|t| ("TD", t.texture_index, Some(t.uv_set_index))),
                particle
                    .texture_reflection
                    .as_ref()
                    .filter(|t| t.enabled && mesh_textures_used && !t.use_screen_copy)
                    .map(|t| ("TR", t.texture_index, None)),
                particle
                    .texture_palette
                    .as_ref()
                    .filter(|t| t.enabled)
                    .map(|t| ("TP", t.texture_index, None)),
            ];
            for (layer, texture_index, uv_index) in texture_refs.into_iter().flatten() {
                check(
                    format!("{owner}.{layer}.TxNo"),
                    texture_index,
                    "Tex",
                    self.texture_paths.len(),
                );
                if let Some(uv_index) = uv_index {
                    check(format!("{owner}.{layer}.UvSN"), uv_index & 7, &uv_table, 4);
                }
            }
            match &particle.data {
                AvfxParticleData::Model { model_indexes, .. } => {
                    for (slot, &index) in model_indexes.iter().enumerate() {
                        check(
                            format!("{owner}.Data.MdNo[{slot}]"),
                            i32::from(index as i8),
                            "Modl",
                            self.models.len(),
                        );
                    }
                }
                AvfxParticleData::LightModel { model_index } => check(
                    format!("{owner}.Data.MNO"),
                    *model_index,
                    "Modl",
                    self.models.len(),
                ),
                _ => {}
            }
            if let Some(simple) = particle
                .simple
                .as_ref()
                .filter(|_| particle.simple_anim_enable)
            {
                for (field, index) in [
                    ("IJMN", simple.injection_model_index),
                    ("VBMN", simple.injection_vertex_bind_model_index),
                ] {
                    let index = if particle.particle_type == Some(ParticleType::Powder) {
                        i32::from(index as i8)
                    } else {
                        index
                    };
                    check(
                        format!("{owner}.Smpl.{field}"),
                        index,
                        "Modl",
                        self.models.len(),
                    );
                }
            }
        }
    }
}

fn warn_count_mismatch(node: &AvfxNodeView, name: &str, actual: usize, warnings: &mut Vec<String>) {
    if let Some(declared) = node.scalar_i32(name) {
        if usize::try_from(declared).ok() != Some(actual) {
            warnings.push(format!(
                "{}.{name}: declared {declared}, parsed {actual}",
                node.name()
            ));
        }
    }
}

fn curve_has_nonfinite_key(curve: &AvfxCurve) -> bool {
    curve
        .keys
        .iter()
        .any(|key| !key.x.is_finite() || !key.y.is_finite() || !key.z.is_finite())
}

fn curve3_has_nonfinite_key(curve: &AvfxCurve3Axis) -> bool {
    curve
        .x
        .iter()
        .chain(curve.y.iter())
        .chain(curve.z.iter())
        .any(curve_has_nonfinite_key)
}

fn curve2_has_nonfinite_key(curve: &AvfxCurve2Axis) -> bool {
    curve
        .x
        .iter()
        .chain(curve.y.iter())
        .any(curve_has_nonfinite_key)
}

fn color_curve_has_nonfinite_key(curve: &AvfxColorCurve) -> bool {
    curve
        .rgb
        .iter()
        .chain(curve.alpha.iter())
        .chain(curve.brightness.iter())
        .chain(curve.scale_alpha.iter())
        .chain(curve.random.iter().flatten())
        .any(curve_has_nonfinite_key)
        || curve.scale_rgb.as_ref().is_some_and(|scale| {
            scale
                .r
                .iter()
                .chain(scale.g.iter())
                .chain(scale.b.iter())
                .any(curve_has_nonfinite_key)
        })
}

fn push_nonfinite_aux_curve(fields: &mut Vec<String>, name: String, curve: Option<&AvfxCurve>) {
    if curve.is_some_and(curve_has_nonfinite_key) {
        fields.push(name);
    }
}

// Only descend into known containers. Clip, Keys, mesh arrays and unknown fields
// can contain arbitrary bytes and must not be interpreted as nested blocks.
fn nested_container_names(parent: &str) -> &'static [&'static str] {
    match parent {
        "AVFX" => &["Schd", "TmLn", "Emit", "Ptcl", "Bind", "Efct", "Modl"],
        "Schd" => &["Item", "Trgr"],
        "TmLn" => &["Item"],
        "Emit" => &[
            "ItPr", "ItEm", "Data", "Life", "CrC", "CrCR", "CrI", "CrIR", "Gra", "GraR", "ARs",
            "ARsR", "Col", "Pos", "Rot", "Scl", "IAX", "IAY", "IAZ", "IAXR", "IAYR", "IAZR", "VRX",
            "VRY", "VRZ", "VRXR", "VRYR", "VRZR",
        ],
        "Ptcl" => &[
            "Data", "Smpl", "Life", "Gra", "GraR", "ARs", "ARsR", "Pos", "Rot", "Scl", "Col",
            "VRX", "VRY", "VRZ", "VRXR", "VRYR", "VRZR", "TC1", "TC2", "TC3", "TC4", "TN", "TR",
            "TP", "TD", "UvSt",
        ],
        "Bind" => &["Data", "PrpS", "Prp1", "Prp2", "PrpG"],
        "Efct" => &["Data"],
        "PrpS" | "Prp1" | "Prp2" | "PrpG" => &["Pos"],
        "UvSt" => &["Scl", "Scr", "Rot", "RotR"],
        "Pos" | "Rot" | "Scl" | "Scr" | "FrRt" => &["X", "Y", "Z", "XR", "YR", "ZR"],
        "Amb" | "Col" | "ColB" | "ColC" | "ColE" | "CEI" | "CEO" | "CoEB" | "CoEC" | "CoEE"
        | "EdC" => &[
            "RGB", "A", "Bri", "SclR", "SclG", "SclB", "SclA", "RanR", "RanG", "RanB", "RanA",
            "RBri",
        ],
        "TC1" | "TC2" | "TC3" | "TC4" => &["TxN", "TxNR"],
        "TN" => &["NPow"],
        "TR" => &["Rate", "RPow"],
        "TP" => &["POff", "POfR"],
        "TD" => &["DPow"],
        "Data" => &[
            "AnX", "AnY", "AnZ", "AnXR", "AnYR", "AnZR", "InS", "InSR", "OuS", "OuSR", "IjS",
            "IjSR", "IjA", "IjAR", "Rad", "RadR", "Rads", "Len", "LenR", "NoAn", "Moph", "FrC",
            "FrCR", "FrRt", "ColB", "ColC", "ColE", "CEI", "CEO", "CoEB", "CoEC", "CoEE", "Ang",
            "AngR", "HBI", "HBIR", "HEI", "HEIR", "HBO", "HBOR", "HEO", "HEOR", "WB", "WBR", "WE",
            "WER", "RB", "RBR", "RE", "RER", "CF", "CFR", "Wd", "WdR", "WdB", "WdBR", "WdC",
            "WdCR", "WdE", "WdER", "Sft", "SftR", "PnDs", "Wdt", "WdtR", "Cnt", "CntR", "EroR",
            "EdW", "EdCW", "ECMP", "Int", "EdC", "SpS", "SpSR", "COF", "COFR", "Dst", "DstR",
        ],
        _ => &[],
    }
}

const EFFECTOR_DATA_CONTAINERS: &[&str] = &[
    "Amb", "Col", "Pos", "Rot", "DstS", "Pow", "PowR", "Att", "AttR", "RdO", "RdOR", "RdI", "RdIR",
    "CSR", "CSS", "CGE", "Str", "Gra", "IRad", "ORad", "AStr", "Ang", "Len",
];

fn validate_known_containers(root: &AvfxNodeView, warnings: &mut Vec<String>) {
    let mut pending = vec![(
        AvfxNodeView {
            name: std::borrow::Cow::Borrowed("AVFX"),
            payload: root.payload(),
            storage_payload: root.payload(),
        },
        8_usize,
        "AVFX".to_owned(),
    )];
    while let Some((node, base_offset, path)) = pending.pop() {
        let nested_names = if path == "AVFX.Efct.Data" {
            EFFECTOR_DATA_CONTAINERS
        } else {
            nested_container_names(node.name())
        };
        let mut offset = 0;
        while offset < node.payload().len() {
            let remaining = &node.payload()[offset..];
            let absolute_offset = base_offset + offset;
            let Some(header) = remaining.get(..8) else {
                warnings.push(format!(
                    "{path} at {absolute_offset:#x}: truncated block header ({} bytes)",
                    remaining.len()
                ));
                break;
            };
            let raw = header[..4].try_into().unwrap();
            let name = reverse_name_bytes(&raw);
            let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            let Some(payload) = remaining[8..].get(..size) else {
                warnings.push(format!(
                    "{path}.{name} at {absolute_offset:#x}: declares {size} bytes, only {} remain",
                    remaining.len() - 8
                ));
                break;
            };
            if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
                warnings.push(format!(
                    "{path} at {absolute_offset:#x}: invalid block name {raw:?}"
                ));
            } else if nested_names.contains(&name.as_str()) {
                // Life and texture attributes may use a four-byte disabled form.
                let disabled = size == 4
                    && matches!(
                        name.as_str(),
                        "Life" | "TC1" | "TC2" | "TC3" | "TC4" | "TN" | "TR" | "TP" | "TD"
                    );
                if !disabled {
                    let child_path = format!("{path}.{name}");
                    pending.push((
                        AvfxNodeView {
                            name: std::borrow::Cow::Owned(name),
                            payload,
                            storage_payload: payload,
                        },
                        absolute_offset + 8,
                        child_path,
                    ));
                }
            }
            offset += 8 + size;
            let padding = (4 - size % 4) % 4;
            if padding > node.payload().len() - offset {
                warnings.push(format!(
                    "{path} at {absolute_offset:#x}: missing block padding"
                ));
                break;
            }
            offset += padding;
        }
        let keys = node.child("Keys");
        let key_bytes = keys.as_ref().map_or(0, |keys| keys.payload().len());
        if key_bytes % 16 != 0 {
            warnings.push(format!(
                "{path}.Keys at {base_offset:#x}: {} trailing bytes outside 16-byte records",
                key_bytes % 16
            ));
        }
        if let Some(declared) = node.scalar_i32("KeyC") {
            if usize::try_from(declared).ok() != Some(key_bytes / 16) {
                warnings.push(format!(
                    "{path}.KeyC at {base_offset:#x}: declared {declared}, parsed {}",
                    key_bytes / 16
                ));
            }
        }
    }
}

/// 根级全局参数块名（VFXEditor `AvfxMain` 的全部叶子）。
const GLOBAL_BLOCK_NAMES: &[&str] = &[
    "bDFP", "bFG", "bTS", "bASH", "bCBC", "bCul", "CBPx", "CBPy", "CBPz", "CBSx", "CBSy", "CBSz",
    "ZBMs", "ZBMd", "bCmS", "bFEL", "bOSE", "bOSt", "NCB", "NCE", "FCB", "FCE", "SPFR", "SKO",
    "DwLy", "DwOT", "DLST", "PL1S", "PL2S", "RvPx", "RvPy", "RvPz", "RvRx", "RvRy", "RvRz", "RvSx",
    "RvSy", "RvSz", "RvR", "RvG", "RvB", "AFXe", "AFXi", "AFXo", "AFYe", "AFYi", "AFYo", "AFZe",
    "AFZi", "AFZo", "bGFE", "GFIM", "bLTS", "bAGS", "APri", "DPri", "bSAB", "bSBV", "SBVa", "bSSV",
    "SSVa", "SPHP",
];

fn parse_global_parameters(root: &AvfxNodeView) -> AvfxGlobalParameters {
    let bool_at = |name: &str| root.boolean(name).unwrap_or(false);
    let f32_at = |name: &str| root.f32(name).unwrap_or(0.0);
    let i32_at = |name: &str| root.scalar_i32(name).unwrap_or(0);
    AvfxGlobalParameters {
        is_delay_fast_particle: bool_at("bDFP"),
        is_fit_ground: bool_at("bFG"),
        is_transform_skip: bool_at("bTS"),
        is_all_stop_on_hide: bool_at("bASH"),
        can_be_clipped_out: bool_at("bCBC"),
        clip_box_enabled: bool_at("bCul"),
        clip_box: [f32_at("CBPx"), f32_at("CBPy"), f32_at("CBPz")],
        clip_box_size: [f32_at("CBSx"), f32_at("CBSy"), f32_at("CBSz")],
        bias_z_max_scale: f32_at("ZBMs"),
        bias_z_max_distance: f32_at("ZBMd"),
        is_camera_space: bool_at("bCmS"),
        is_full_env_light: bool_at("bFEL"),
        ose: bool_at("bOSE"),
        is_clip_own_setting: bool_at("bOSt"),
        near_clip_begin: f32_at("NCB"),
        near_clip_end: f32_at("NCE"),
        far_clip_begin: f32_at("FCB"),
        far_clip_end: f32_at("FCE"),
        soft_particle_fade_range: f32_at("SPFR"),
        soft_key_offset: f32_at("SKO"),
        // Client root constructor initializes +0x5c to 0x0a00: layer 2.
        draw_layer: root.scalar_i32("DwLy").unwrap_or(2),
        draw_order: i32_at("DwOT"),
        directional_light_source: i32_at("DLST"),
        point_light_1: i32_at("PL1S"),
        point_light_2: i32_at("PL2S"),
        revised_position: [f32_at("RvPx"), f32_at("RvPy"), f32_at("RvPz")],
        revised_rotation: [f32_at("RvRx"), f32_at("RvRy"), f32_at("RvRz")],
        // VFXEditor/AVFXTools initialize revised scale and color to one. A
        // missing leaf therefore means identity, not a zeroed transform.
        revised_scale: [
            root.f32("RvSx").unwrap_or(1.0),
            root.f32("RvSy").unwrap_or(1.0),
            root.f32("RvSz").unwrap_or(1.0),
        ],
        revised_color: [
            root.f32("RvR").unwrap_or(1.0),
            root.f32("RvG").unwrap_or(1.0),
            root.f32("RvB").unwrap_or(1.0),
        ],
        fade_enabled: [bool_at("AFXe"), bool_at("AFYe"), bool_at("AFZe")],
        fade_inner: [f32_at("AFXi"), f32_at("AFYi"), f32_at("AFZi")],
        fade_outer: [f32_at("AFXo"), f32_at("AFYo"), f32_at("AFZo")],
        global_fog_enabled: bool_at("bGFE"),
        global_fog_influence: f32_at("GFIM"),
        lts_enabled: bool_at("bLTS"),
        // Original parser 3ad804 reads the DWORD slot, including alignment
        // bytes for short Boolean payloads. Repeated records use the last write.
        ags_enabled: root
            .children_named("bAGS")
            .filter_map(|leaf| {
                if leaf.payload().is_empty() {
                    None
                } else {
                    read_u32(leaf.storage_payload)
                        .map(|raw| raw != 0)
                        // Retain tolerant decoding of malformed missing padding;
                        // structural validation already reports that boundary.
                        .or_else(|| read_bool(leaf.payload()))
                }
            })
            .last()
            .unwrap_or(false),
        a_pri: i32_at("APri"),
        d_pri: i32_at("DPri"),
        sab_enabled: bool_at("bSAB"),
        sbv_enabled: bool_at("bSBV"),
        sbv_a: f32_at("SBVa"),
        ssv_enabled: bool_at("bSSV"),
        ssv_a: f32_at("SSVa"),
        sphp: i32_at("SPHP"),
    }
}

fn warn_unsupported_global_parameters(global: &AvfxGlobalParameters, warnings: &mut Vec<String>) {
    let nonfinite = [
        ("NCB", global.near_clip_begin),
        ("NCE", global.near_clip_end),
        ("FCB", global.far_clip_begin),
        ("FCE", global.far_clip_end),
        ("SKO", global.soft_key_offset),
        ("GFIM", global.global_fog_influence),
        ("SBVa", global.sbv_a),
        ("SSVa", global.ssv_a),
    ]
    .into_iter()
    .filter_map(|(name, value)| (!value.is_finite()).then_some(name))
    .chain(
        global
            .revised_position
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "RvPx",
                    1 => "RvPy",
                    _ => "RvPz",
                })
            }),
    )
    .chain(
        global
            .revised_rotation
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "RvRx",
                    1 => "RvRy",
                    _ => "RvRz",
                })
            }),
    )
    .chain(
        global
            .revised_scale
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "RvSx",
                    1 => "RvSy",
                    _ => "RvSz",
                })
            }),
    )
    .chain(
        global
            .revised_color
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "RvR",
                    1 => "RvG",
                    _ => "RvB",
                })
            }),
    )
    .chain(
        global
            .fade_inner
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "AFXi",
                    1 => "AFYi",
                    _ => "AFZi",
                })
            }),
    )
    .chain(
        global
            .fade_outer
            .into_iter()
            .enumerate()
            .filter_map(|(axis, value)| {
                (!value.is_finite()).then_some(match axis {
                    0 => "AFXo",
                    1 => "AFYo",
                    _ => "AFZo",
                })
            }),
    )
    .collect::<Vec<_>>();
    if !nonfinite.is_empty() {
        warnings.push(format!(
            "AvfxMain.{} contains nonfinite scalar data; global clipping, lighting, fog, revised transform, or fade sampling may be skipped",
            nonfinite.join(", ")
        ));
    }
    if !global.soft_particle_fade_range.is_finite() {
        warnings.push(format!(
            "AvfxMain.SPFR={} is nonfinite; soft-particle depth fade range is invalid",
            global.soft_particle_fade_range
        ));
    }
    for (axis, enabled, inner, outer) in [
        (
            "X",
            global.fade_enabled[0],
            global.fade_inner[0],
            global.fade_outer[0],
        ),
        (
            "Y",
            global.fade_enabled[1],
            global.fade_inner[1],
            global.fade_outer[1],
        ),
        (
            "Z",
            global.fade_enabled[2],
            global.fade_inner[2],
            global.fade_outer[2],
        ),
    ] {
        if enabled {
            warnings.push(format!(
                "AvfxMain.AF{axis}e=1 distance fade ({inner}..{outer}) is parsed but renderer has no global distance-fade stage"
            ));
        }
    }
    if global.global_fog_enabled {
        warnings.push(format!(
            "AvfxMain.bGFE=1 global fog (GFIM={}) is parsed but renderer has no scene fog parameters",
            global.global_fog_influence
        ));
    }
    if global.near_clip_begin != 0.0 || global.near_clip_end != 0.0 {
        warnings.push(format!(
            "AvfxMain.NCB/NCE=({}, {}) are parsed but global near clipping is not connected",
            global.near_clip_begin, global.near_clip_end
        ));
    }
    if global.far_clip_begin != 0.0 || global.far_clip_end != 0.0 {
        warnings.push(format!(
            "AvfxMain.FCB/FCE=({}, {}) are parsed but global far clipping is not connected",
            global.far_clip_begin, global.far_clip_end
        ));
    }
    if global.is_camera_space {
        warnings.push(
            "AvfxMain.bCmS=1 camera-space transform is parsed but renderer has no camera-space root path"
                .to_owned(),
        );
    }
    if global.is_full_env_light {
        warnings.push(
            "AvfxMain.bFEL=1 full environment lighting is parsed but renderer has no VFX environment-light inputs"
                .to_owned(),
        );
    }
    if global.lts_enabled {
        warnings.push(
            "AvfxMain.bLTS=1 lighting switch is parsed but VFX lighting is not connected"
                .to_owned(),
        );
    }
    if global.ags_enabled {
        warnings.push("AvfxMain.bAGS=1 root revision uses the Binder auxiliary-matrix path; dynamic Document sources, exact client CRT trig and dynamic ancestor/Binder history remain incomplete".to_owned());
    }
    if global.directional_light_source != 0
        || global.point_light_1 != 0
        || global.point_light_2 != 0
    {
        warnings.push(format!(
            "AvfxMain.DLST/PL1S/PL2S=({}, {}, {}) light selections are parsed but VFX light sources are not connected",
            global.directional_light_source, global.point_light_1, global.point_light_2
        ));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvfxParseError {
    TooShort { len: usize },
    BadRootName { raw: [u8; 4] },
    TruncatedRoot { declared: usize, available: usize },
}

impl std::fmt::Display for AvfxParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(
                    f,
                    "avfx too short: {len} bytes (need the 8-byte root header)"
                )
            }
            Self::BadRootName { raw } => {
                let name = reverse_name_bytes(raw);
                write!(
                    f,
                    "avfx root block name mismatch: expected AVFX, got {name:?}"
                )
            }
            Self::TruncatedRoot {
                declared,
                available,
            } => {
                write!(
                    f,
                    "avfx root declares {declared} payload bytes, only {available} available"
                )
            }
        }
    }
}

impl std::error::Error for AvfxParseError {}

/// 块视图：名字（反写还原、去尾空白/NUL）+ 原始负载。
pub struct AvfxNodeView<'a> {
    name: std::borrow::Cow<'a, str>,
    payload: &'a [u8],
    /// Physical payload slot including available alignment bytes. Most fields
    /// use the declared payload; bAGS's original reader consumes a whole DWORD.
    storage_payload: &'a [u8],
}

impl<'a> AvfxNodeView<'a> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    fn parse_root(bytes: &'a [u8]) -> Result<Self, AvfxParseError> {
        if bytes.len() < 8 {
            return Err(AvfxParseError::TooShort { len: bytes.len() });
        }
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&bytes[0..4]);
        if reverse_name_bytes(&raw).trim() != "AVFX" {
            return Err(AvfxParseError::BadRootName { raw });
        }
        let size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let available = bytes.len() - 8;
        if size > available {
            return Err(AvfxParseError::TruncatedRoot {
                declared: size,
                available,
            });
        }
        let end = 8 + size;
        Ok(Self {
            name: std::borrow::Cow::Borrowed("AVFX"),
            payload: &bytes[8..end],
            storage_payload: &bytes[8..end],
        })
    }

    /// 迭代直接子块（非递归）；名字不可读或头部越界的尾块被静默跳过。
    pub fn children(&self) -> AvfxChildIter<'a> {
        AvfxChildIter {
            bytes: self.payload,
            offset: 0,
        }
    }

    /// 标量叶子查找（名为 `name` 的子块负载按 u32 读）。
    pub fn scalar(&self, name: &str) -> Option<u32> {
        self.children()
            .find(|child| child.name() == name)
            .and_then(|child| read_u32(child.payload()))
    }

    pub fn scalar_i32(&self, name: &str) -> Option<i32> {
        self.child(name).and_then(|child| read_i32(child.payload()))
    }

    /// Boolean leaf lookup, accepting byte payloads as well as padded words.
    /// Missing and empty leaves remain `None` so callers keep their defaults.
    pub fn boolean(&self, name: &str) -> Option<bool> {
        self.child(name)
            .and_then(|child| read_bool(child.payload()))
    }

    /// f32 叶子查找（名为 `name` 的子块负载按 f32 位模式读）。
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.scalar(name).map(f32::from_bits)
    }

    /// 名为 `name` 的第一个子块（容器或叶子）。
    pub fn child(&self, name: &str) -> Option<AvfxNodeView<'a>> {
        self.children().find(|child| child.name() == name)
    }

    /// 名为 `name` 的全部子块（累积式 Item 容器取末位、重复 UvSt 全收）。
    pub fn children_named<'s>(&self, name: &'s str) -> impl Iterator<Item = AvfxNodeView<'a>> {
        self.children().filter(move |child| child.name() == name)
    }
}

pub struct AvfxChildIter<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for AvfxChildIter<'a> {
    type Item = AvfxNodeView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let remaining = self.bytes.get(self.offset..)?;
            let header = remaining.get(..8)?;
            let raw = header[..4].try_into().unwrap();
            let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            let Some(payload) = remaining[8..].get(..size) else {
                self.offset = self.bytes.len();
                return None;
            };
            // Payload bounds are checked before arithmetic, including on wasm32.
            self.offset += 8 + size;
            let padding = (4 - size % 4) % 4;
            let available_padding = padding.min(self.bytes.len() - self.offset);
            self.offset += available_padding;
            let name = reverse_name_bytes(&raw);
            if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
                continue;
            }
            return Some(AvfxNodeView {
                name: std::borrow::Cow::Owned(name),
                payload,
                storage_payload: &remaining[8..8 + size + available_padding],
            });
        }
    }
}

fn reverse_name_bytes(raw: &[u8; 4]) -> String {
    let mut name = [0u8; 4];
    for (index, byte) in raw.iter().rev().take(4).enumerate() {
        name[index] = *byte;
    }
    // 块名不足 4 字节时在原名尾部补 NUL 再反写存储（VFXEditor 写序），
    // 读回反写后补位落在名字前部（如 "\0TC1"）；两侧 trim NUL/空格归一。
    String::from_utf8_lossy(&name)
        .trim_matches(['\0', ' '])
        .to_string()
}

fn read_u32(payload: &[u8]) -> Option<u32> {
    if payload.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ]))
}

fn read_i32(payload: &[u8]) -> Option<i32> {
    // VFXEditor ParsedInt: byte is unsigned; short and int are signed.
    match payload {
        [value] => Some(i32::from(*value)),
        [lo, hi] => Some(i32::from(i16::from_le_bytes([*lo, *hi]))),
        [a, b, c, d] => Some(i32::from_le_bytes([*a, *b, *c, *d])),
        _ => None,
    }
}

fn read_bool(payload: &[u8]) -> Option<bool> {
    match payload.len() {
        1..=3 => Some(payload[0] != 0),
        4.. => Some(read_u32(payload)? != 0),
        _ => None,
    }
}

fn read_null_terminated(payload: &[u8]) -> String {
    let end = payload
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(payload.len());
    String::from_utf8_lossy(&payload[..end]).trim().to_string()
}

/// 把块序列负载里的字段块整理为（名字→负载）；负载过短不能成块的视为内联值。
struct Fields<'a> {
    scalars: HashMap<String, &'a [u8]>,
}

impl<'a> Fields<'a> {
    /// 对一块（容器或切出的条目）负载直接遍历块收集叶子字段。
    fn walk(bytes: &'a [u8]) -> Self {
        let mut scalars = HashMap::new();
        for child in child_blocks(bytes) {
            scalars
                .entry(child.name().to_string())
                .or_insert(child.payload());
        }
        Self { scalars }
    }

    fn u32(&self, name: &str) -> Option<u32> {
        read_u32(*self.scalars.get(name)?)
    }

    fn i32(&self, name: &str) -> Option<i32> {
        read_i32(self.scalars.get(name)?)
    }

    fn f32(&self, name: &str) -> Option<f32> {
        self.u32(name).map(f32::from_bits)
    }

    fn boolean(&self, name: &str) -> Option<bool> {
        read_bool(self.scalars.get(name)?)
    }

    fn bytes(&self, name: &str) -> Option<&'a [u8]> {
        self.scalars.get(name).copied()
    }
}

/// 遍历一块负载中的块。
fn child_blocks(bytes: &[u8]) -> impl Iterator<Item = AvfxNodeView<'_>> {
    AvfxChildIter { bytes, offset: 0 }
}

/// 单轴曲线：块内直接是 `KeyC`/`BvPr`/`BvPo`/`RanT`/`Keys`。
fn parse_curve(node: &AvfxNodeView) -> AvfxCurve {
    let mut curve = AvfxCurve {
        pre_behavior: node.scalar("BvPr").unwrap_or(BEHAVIOR_CONST),
        post_behavior: node.scalar("BvPo").unwrap_or(BEHAVIOR_CONST),
        random_type: node.scalar("RanT").unwrap_or(0),
        keys: Vec::new(),
    };
    if let Some(keys) = node.child("Keys") {
        for chunk in keys.payload().chunks_exact(16) {
            let time = i16::from_le_bytes([chunk[0], chunk[1]]);
            let interpolation = u16::from_le_bytes([chunk[2], chunk[3]]);
            curve.keys.push(AvfxCurveKey {
                time,
                interpolation,
                x: f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
                y: f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]),
                z: f32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]),
            });
        }
    }
    curve
}

/// 三轴曲线容器：`ACT` + 按轴命名的 `X`/`Y`/`Z` 子曲线。
fn parse_curve3(node: &AvfxNodeView) -> AvfxCurve3Axis {
    AvfxCurve3Axis {
        axis_connect: node.scalar("ACT").unwrap_or(0),
        axis_connect_random: node.scalar("ACTR").unwrap_or(0),
        x: node.child("X").map(|child| parse_curve(&child)),
        y: node.child("Y").map(|child| parse_curve(&child)),
        z: node.child("Z").map(|child| parse_curve(&child)),
        random_x: node.child("XR").map(|child| parse_curve(&child)),
        random_y: node.child("YR").map(|child| parse_curve(&child)),
        random_z: node.child("ZR").map(|child| parse_curve(&child)),
    }
}

/// 两轴曲线容器（UvSt `Scl`/`Scr`）。
fn parse_curve2(node: &AvfxNodeView) -> AvfxCurve2Axis {
    AvfxCurve2Axis {
        axis_connect: node.scalar("ACT").unwrap_or(0),
        axis_connect_random: node.scalar("ACTR").unwrap_or(0),
        x: node.child("X").map(|child| parse_curve(&child)),
        y: node.child("Y").map(|child| parse_curve(&child)),
        random_x: node.child("XR").map(|child| parse_curve(&child)),
        random_y: node.child("YR").map(|child| parse_curve(&child)),
    }
}

/// 颜色曲线容器：`RGB`/`A`、通道缩放、亮度及随机子曲线。
fn parse_color_curve(node: &AvfxNodeView) -> AvfxColorCurve {
    AvfxColorCurve {
        rgb: node.child("RGB").map(|child| parse_curve(&child)),
        alpha: node.child("A").map(|child| parse_curve(&child)),
        brightness: node.child("Bri").map(|child| parse_curve(&child)),
        scale_alpha: node.child("SclA").map(|child| parse_curve(&child)),
        scale_rgb: {
            let r = node.child("SclR").map(|child| parse_curve(&child));
            let g = node.child("SclG").map(|child| parse_curve(&child));
            let b = node.child("SclB").map(|child| parse_curve(&child));
            if r.is_some() || g.is_some() || b.is_some() {
                Some(AvfxColorScaleRgb { r, g, b })
            } else {
                None
            }
        },
        random: [
            node.child("RanR").map(|child| parse_curve(&child)),
            node.child("RanG").map(|child| parse_curve(&child)),
            node.child("RanB").map(|child| parse_curve(&child)),
            node.child("RanA").map(|child| parse_curve(&child)),
            node.child("RBri").map(|child| parse_curve(&child)),
        ],
    }
}

fn parse_optional_life(node: &AvfxNodeView) -> AvfxLife {
    let Some(life) = node.child("Life") else {
        return AvfxLife::default();
    };
    if life.payload().len() <= 4 {
        return AvfxLife::default();
    }
    let fields = Fields::walk(life.payload());
    AvfxLife {
        enabled: true,
        value: fields.f32("Val").unwrap_or(-1.0),
        value_random: fields.f32("ValR").unwrap_or(0.0),
        random_type: fields.u32("Type").unwrap_or(0),
    }
}

/// Scheduler/Timeline 的累积式 Item/Trgr 容器：返回最后一个容器的负载。
fn last_container_payload<'a>(node: &AvfxNodeView<'a>, name: &str) -> Option<&'a [u8]> {
    node.children_named(name).last().map(|last| last.payload())
}

// Timeline entries may omit ClNo; emitter entries have several field counts.
// Split at the leading enabled field, never by a divisible total byte length.
fn item_payloads<'a>(payload: &'a [u8], first_field: &str) -> Vec<&'a [u8]> {
    let mut result = Vec::new();
    let mut start = None;
    let mut fields = AvfxChildIter {
        bytes: payload,
        offset: 0,
    };
    loop {
        let offset = fields.offset;
        let Some(field) = fields.next() else { break };
        if field.name() == first_field {
            if let Some(previous) = start.replace(offset) {
                result.push(&payload[previous..offset]);
            }
        }
    }
    if let Some(start) = start {
        result.push(&payload[start..]);
    }
    result
}

fn parse_scheduler(node: &AvfxNodeView, warnings: &mut Vec<String>) -> AvfxScheduler {
    // Item/Trgr 容器为累积式，每个条目以 bEna 开始。
    let items = last_container_payload(node, "Item")
        .map(|payload| {
            item_payloads(payload, "bEna")
                .into_iter()
                .map(|chunk| {
                    let fields = Fields::walk(chunk);
                    AvfxSchedulerItem {
                        enabled: fields.u32("bEna").is_none_or(|value| value & 3 != 0),
                        start_time: fields.i32("StTm").unwrap_or(0),
                        timeline_index: fields.i32("TlNo").unwrap_or(-1),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let triggers = last_container_payload(node, "Trgr")
        .map(|payload| {
            item_payloads(payload, "bEna")
                .into_iter()
                .map(|chunk| {
                    let fields = Fields::walk(chunk);
                    AvfxSchedulerItem {
                        enabled: fields.u32("bEna").is_none_or(|value| value & 3 != 0),
                        start_time: fields.i32("StTm").unwrap_or(0),
                        timeline_index: fields.i32("TlNo").unwrap_or(-1),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // 触发器固定 12 个，且写在全部条目之后（VFXEditor `AvfxScheduler`）。
    let triggers = if triggers.len() >= 12 {
        triggers[triggers.len() - 12..].to_vec()
    } else {
        if !triggers.is_empty() {
            warnings.push(format!(
                "scheduler has {} trigger-like entries (expected 12)",
                triggers.len()
            ));
        }
        triggers
    };
    let item_count = Fields::walk(node.payload()).i32("ItCn");
    if let Some(count) = item_count {
        let compiled = (count as u8 as i8).max(0) as usize;
        if compiled > items.len() {
            warnings.push(format!(
                "scheduler compiled Item count {compiled} exceeds {} mappings",
                items.len()
            ));
        }
    }
    AvfxScheduler {
        item_count,
        items,
        triggers,
    }
}

fn parse_timeline_clip(
    node: &AvfxNodeView,
    timeline_index: usize,
    clip_index: usize,
    warnings: &mut Vec<String>,
) -> AvfxTimelineClip {
    let payload = node.payload();
    if payload.len() < 164 {
        warnings.push(format!(
            "TmLn[{timeline_index}].Clip[{clip_index}]: expected 164 bytes, found {}",
            payload.len()
        ));
    }
    let raw_type = read_u32(payload);
    let raw_ints = payload.get(4..20).map(|bytes| {
        std::array::from_fn(|index| {
            i32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
        })
    });
    let raw_floats = payload.get(20..36).map(|bytes| {
        std::array::from_fn(|index| {
            f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
        })
    });
    let clip_type = raw_type.and_then(AvfxTimelineClipType::from_raw);
    let parameters = match (clip_type, raw_ints, raw_floats) {
        (Some(AvfxTimelineClipType::Kill), Some(ints), Some(floats)) => {
            Some(AvfxTimelineClipParameters::Kill {
                fade_out_duration: ints[0],
                targets: std::array::from_fn(|i| {
                    AvfxTimelineClipTarget::from_signed_selector(ints[i + 1])
                }),
                fade_mode: if floats[1] < 0.0 { 2 } else { 3 },
                fade_flag: floats[2] < 0.0,
                end_document_on_retire: floats[0] < 0.0,
            })
        }
        (Some(AvfxTimelineClipType::Reset), Some(ints), Some(floats)) => {
            Some(AvfxTimelineClipParameters::Reset {
                document: floats[0] >= 0.0,
                scheduler_items: std::array::from_fn(|i| ints[i + 1].wrapping_sub(1)),
            })
        }
        (Some(AvfxTimelineClipType::End), _, _) => Some(AvfxTimelineClipParameters::End),
        (Some(AvfxTimelineClipType::FadeIn), Some(ints), _) => {
            Some(AvfxTimelineClipParameters::FadeIn {
                duration: ints[0],
                targets: std::array::from_fn(|i| {
                    AvfxTimelineClipTarget::from_signed_selector(ints[i + 1])
                }),
            })
        }
        (Some(AvfxTimelineClipType::UnlockLoopPoint), Some(ints), Some(floats)) => {
            Some(AvfxTimelineClipParameters::UnlockLoopPoint {
                all_scheduler: floats[0] >= 0.0,
                targets: std::array::from_fn(|i| {
                    AvfxTimelineClipTarget::from_signed_selector(ints[i + 1])
                }),
            })
        }
        (Some(AvfxTimelineClipType::Trigger), Some(ints), _) => {
            Some(AvfxTimelineClipParameters::Trigger { trigger: ints[0] })
        }
        (Some(AvfxTimelineClipType::RandomTrigger), Some(ints), _) => {
            Some(AvfxTimelineClipParameters::RandomTrigger {
                minimum_trigger: ints[0],
                maximum_trigger: ints[1],
            })
        }
        _ => None,
    };
    AvfxTimelineClip {
        clip_type,
        parameters,
        raw_type,
        raw_ints,
        raw_floats,
        raw_payload: payload.to_vec(),
    }
}

fn parse_timeline(
    node: &AvfxNodeView,
    timeline_index: usize,
    warnings: &mut Vec<String>,
) -> AvfxTimeline {
    let items = last_container_payload(node, "Item")
        .map(|payload| {
            item_payloads(payload, "bEna")
                .into_iter()
                .map(|chunk| {
                    let fields = Fields::walk(chunk);
                    AvfxTimelineItem {
                        enabled: fields.boolean("bEna").unwrap_or(true),
                        start_time: fields.i32("StTm").unwrap_or(0),
                        end_time: fields.i32("EdTm").unwrap_or(0),
                        binder_index: fields.i32("BdNo").unwrap_or(-1),
                        effector_index: fields.i32("EfNo").unwrap_or(-1),
                        emitter_index: fields.i32("EmNo").unwrap_or(-1),
                        platform: fields.i32("Plfm").unwrap_or(0),
                        clip_index: fields.i32("ClNo").unwrap_or(-1),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let clips = node
        .children_named("Clip")
        .enumerate()
        .map(|(clip_index, clip)| parse_timeline_clip(&clip, timeline_index, clip_index, warnings))
        .collect::<Vec<_>>();
    let clip_count = clips.len();
    warn_count_mismatch(node, "TICn", items.len(), warnings);
    warn_count_mismatch(node, "CpCn", clip_count, warnings);
    AvfxTimeline {
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        binder_index: node.scalar_i32("BnNo").unwrap_or(-1),
        items,
        clip_count,
        clips,
    }
}

/// Emitter 的 ItPr/ItEm 条目以 `bEnb` 开始，字段数量可变。
fn parse_emitter_items(node: &AvfxNodeView, name: &str) -> Vec<AvfxEmitterItem> {
    let Some(payload) = last_container_payload(node, name) else {
        return Vec::new();
    };
    item_payloads(payload, "bEnb")
        .into_iter()
        .map(|chunk| {
            let fields = Fields::walk(chunk);
            AvfxEmitterItem {
                enabled: fields.boolean("bEnb").unwrap_or(false),
                target_index: fields.i32("TgtB").unwrap_or(-1),
                local_direction: fields.i32("LoDr").unwrap_or(0),
                create_time: fields.i32("CrTm").unwrap_or(1),
                create_count: fields.i32("CrCn").unwrap_or(1),
                create_probability: fields.i32("CrPr").unwrap_or(100),
                parent_influence_coord: fields.i32("PICd").unwrap_or(0),
                parent_influence_color: fields.i32("PICo").unwrap_or(0),
                influence_coord_scale: fields.boolean("ICbS").unwrap_or(false),
                influence_coord_rot: fields.boolean("ICbR").unwrap_or(false),
                influence_coord_pos: fields.boolean("ICbP").unwrap_or(true),
                influence_coord_binder: fields.boolean("ICbB").unwrap_or(false),
                influence_coord_unstickiness: fields.f32("ICSK").unwrap_or(0.0),
                inherit_parent_velocity: fields.boolean("IPbV").unwrap_or(false),
                inherit_parent_life: fields.boolean("IPbL").unwrap_or(false),
                override_life: fields.boolean("bOvr").unwrap_or(false),
                override_life_value: fields.i32("OvrV").unwrap_or(60),
                override_life_random: fields.i32("OvrR").unwrap_or(0),
                parameter_link: fields.i32("PrLk").unwrap_or(-1),
                start_frame: fields.i32("StFr").unwrap_or(0),
                start_frame_null_update: fields.boolean("bStN").unwrap_or(false),
                by_injection_angle: [
                    fields.f32("BIAX").unwrap_or(0.0),
                    fields.f32("BIAY").unwrap_or(0.0),
                    fields.f32("BIAZ").unwrap_or(0.0),
                ],
                generate_delay: fields.i32("GenD").unwrap_or(0),
                generate_delay_by_one: fields.boolean("bGD").unwrap_or(false),
            }
        })
        .collect()
}

fn parse_emitter(node: &AvfxNodeView, index: usize, warnings: &mut Vec<String>) -> AvfxEmitter {
    let mut emitter = AvfxEmitter {
        raw_emitter_type: node.scalar("EVT").unwrap_or(0),
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        child_limit: node.scalar_i32("ClCn").unwrap_or(0),
        particle_count: node.scalar_i32("PrCn").unwrap_or(0),
        emitter_count: node.scalar_i32("EmCn").unwrap_or(0),
        rotation_order: node.scalar_i32("ROT").unwrap_or(0),
        rotation_direction_base: node.scalar_i32("RBDT").unwrap_or(0),
        coord_compute_order: node.scalar_i32("CCOT").unwrap_or(0),
        any_direction: node
            .child("bAD")
            .and_then(|child| read_bool(child.payload()))
            .unwrap_or(false),
        effector_index: node.scalar_i32("EfNo").unwrap_or(-1),
        life: parse_optional_life(node),
        create_count: node
            .child("CrC")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        create_count_random: node
            .child("CrCR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        create_interval: node
            .child("CrI")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        create_interval_random: node
            .child("CrIR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        gravity: node
            .child("Gra")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        gravity_random: node
            .child("GraR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        air_resistance: node
            .child("ARs")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        air_resistance_random: node
            .child("ARsR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        color: node
            .child("Col")
            .map(|child| parse_color_curve(&child))
            .unwrap_or_default(),
        position: node
            .child("Pos")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        rotation: node
            .child("Rot")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        scale: node
            .child("Scl")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        injection_angle: [
            node.child("IAX")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("IAY")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("IAZ")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        ],
        injection_angle_random: [
            node.child("IAXR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("IAYR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("IAZR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        ],
        rotation_velocity: [
            node.child("VRX")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRY")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRZ")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        ],
        rotation_velocity_random: ["VRXR", "VRYR", "VRZR"].map(|name| {
            node.child(name)
                .map(|child| parse_curve(&child))
                .unwrap_or_default()
        }),
        ..Default::default()
    };
    emitter.emitter_type = Some(EmitterType::from_raw(emitter.raw_emitter_type));
    emitter.particle_items = parse_emitter_items(node, "ItPr");
    // ItEm 末容器前缀是全部粒子条目的重复（VFXEditor `AvfxEmitter` 读法），
    // 解析时跳过。
    let mut emitter_items = parse_emitter_items(node, "ItEm");
    if emitter_items.len() >= emitter.particle_items.len() {
        emitter_items.drain(..emitter.particle_items.len());
    }
    emitter.emitter_items = emitter_items;
    warn_count_mismatch(node, "PrCn", emitter.particle_items.len(), warnings);
    warn_count_mismatch(node, "EmCn", emitter.emitter_items.len(), warnings);
    if let Some(data) = node.child("Data") {
        emitter.data_payload = data.payload().to_vec();
        let curve = |name| {
            data.child(name)
                .map(|child| parse_curve(&child))
                .unwrap_or_default()
        };
        let rotation = EmitterDataRotation {
            order: data.scalar_i32("ROT").unwrap_or(0),
            angles: ["AnX", "AnY", "AnZ"].map(curve),
            angles_random: ["AnXR", "AnYR", "AnZR"].map(curve),
        };
        let injection_speed_random = curve("IjSR");
        emitter.data = match emitter.emitter_type {
            Some(EmitterType::Cone) => Some(AvfxEmitterData::Cone(ConeEmitterData {
                rotation,
                injection_speed_random,
                inner_size_random: curve("InSR"),
                outer_size_random: curve("OuSR"),
                injection_angle_random: curve("IjAR"),
                inner_size: data
                    .child("InS")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                outer_size: data
                    .child("OuS")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                injection_speed: data
                    .child("IjS")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                injection_angle: data
                    .child("IjA")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
            })),
            Some(EmitterType::ConeModel) => {
                Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                    rotation,
                    injection_speed_random,
                    radius_random: curve("RadR"),
                    injection_angle_random: curve("IjAR"),
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(3),
                    divide_y: data.scalar_i32("DivY").unwrap_or(3),
                    radius: data
                        .child("Rad")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                    injection_speed: data
                        .child("IjS")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                    injection_angle: data
                        .child("IjA")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                }))
            }
            Some(EmitterType::SphereModel) => {
                Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                    rotation,
                    injection_speed_random,
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(3),
                    divide_y: data.scalar_i32("DivY").unwrap_or(3),
                    radius: data
                        .child("Rads")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                    injection_speed: data
                        .child("IjS")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                }))
            }
            Some(EmitterType::CylinderModel) => {
                Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                    rotation,
                    injection_speed_random,
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(3),
                    divide_y: data.scalar_i32("DivY").unwrap_or(3),
                    length: data
                        .child("Len")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                    radius: data
                        .child("Rad")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                    injection_speed: data
                        .child("IjS")
                        .map(|child| parse_curve(&child))
                        .unwrap_or_default(),
                }))
            }
            Some(EmitterType::Model) => Some(AvfxEmitterData::Model(ModelEmitterData {
                rotation,
                injection_speed_random,
                model_index: data.scalar_i32("MdNo").unwrap_or(-1),
                generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                injection_speed: data
                    .child("IjS")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
            })),
            Some(EmitterType::Point | EmitterType::Unknown(_)) | None => None,
        };
        warn_unsupported_emitter_data(&emitter, index, warnings);
    }
    if let Some(EmitterType::Unknown(raw)) = emitter.emitter_type {
        warnings.push(format!(
            "emitter[{index}] EVT={raw} is unsupported; sampler uses point fallback"
        ));
    }
    let direction_fields: Vec<_> = ["VRX", "VRY", "VRZ"]
        .into_iter()
        .zip(&emitter.rotation_velocity)
        .chain(
            ["VRXR", "VRYR", "VRZR"]
                .into_iter()
                .zip(&emitter.rotation_velocity_random),
        )
        .filter_map(|(name, curve)| curve.keys.iter().any(|key| key.z != 0.0).then_some(name))
        .collect();
    if !direction_fields.is_empty() {
        warnings.push(format!(
            "Emit[{index}]: nonzero {} steer this emitter's injected motion via continuous integration; staged child-emitter history remains approximate, and zero-velocity roots are unaffected",
            direction_fields.join(", ")
        ));
    }
    for (kind, items) in [
        ("ItPr", &emitter.particle_items),
        ("ItEm", &emitter.emitter_items),
    ] {
        for (item_index, item) in items.iter().enumerate().filter(|(_, item)| item.enabled) {
            if item.influence_coord_unstickiness != 0.0 {
                warnings.push(format!(
                    "emitter[{index}].{kind}[{item_index}]: ICSK={} unstickiness is unsupported; PICd follow uses the exact parent position",
                    item.influence_coord_unstickiness
                ));
            }
            if matches!(item.parent_influence_coord, 1 | 7) && item.influence_coord_pos {
                let (shape, curves) = match &emitter.data {
                    Some(AvfxEmitterData::ConeModel(data)) => (
                        "ConeModel",
                        vec![
                            ("AnXR", &data.rotation.angles_random[0]),
                            ("AnYR", &data.rotation.angles_random[1]),
                            ("AnZR", &data.rotation.angles_random[2]),
                            ("RadR", &data.radius_random),
                            ("IjAR", &data.injection_angle_random),
                        ],
                    ),
                    Some(AvfxEmitterData::Model(data))
                        if matches!(data.generate_method as u8, 2 | 3 | 6 | 7) =>
                    {
                        (
                            "Model",
                            vec![
                                ("AnXR", &data.rotation.angles_random[0]),
                                ("AnYR", &data.rotation.angles_random[1]),
                                ("AnZR", &data.rotation.angles_random[2]),
                            ],
                        )
                    }
                    Some(AvfxEmitterData::CylinderModel(data)) => (
                        "CylinderModel",
                        vec![
                            ("AnXR", &data.rotation.angles_random[0]),
                            ("AnYR", &data.rotation.angles_random[1]),
                            ("AnZR", &data.rotation.angles_random[2]),
                        ],
                    ),
                    Some(AvfxEmitterData::SphereModel(data))
                        if matches!(data.generate_method as u8, 2 | 3 | 6 | 7) =>
                    {
                        (
                            "SphereModel",
                            vec![
                                ("AnXR", &data.rotation.angles_random[0]),
                                ("AnYR", &data.rotation.angles_random[1]),
                                ("AnZR", &data.rotation.angles_random[2]),
                            ],
                        )
                    }
                    _ => ("", Vec::new()),
                };
                let fields: Vec<_> = curves
                    .into_iter()
                    .filter_map(|(name, curve)| {
                        (matches!(curve.random_type & 7, 3..=5)
                            && curve.keys.iter().any(|key| key.z != 0.0))
                        .then_some(name)
                    })
                    .collect();
                if !fields.is_empty() {
                    warnings.push(format!("emitter[{index}].{kind}[{item_index}]: ICbP {shape} {} Always curves use integer-frame random samples; client callback caches are unsupported", fields.join(", ")));
                }
            }
            if matches!(item.parent_influence_coord, 4..=7 | 9) {
                warnings.push(format!(
                    "emitter[{index}].{kind}[{item_index}]: PICd={} uses normalized inheritance; ground height queries are unsupported",
                    item.parent_influence_coord
                ));
            }
        }
    }
    emitter
}

fn warn_unsupported_emitter_data(emitter: &AvfxEmitter, index: usize, warnings: &mut Vec<String>) {
    let nonfinite_emitter = [
        ("CrC", curve_has_nonfinite_key(&emitter.create_count)),
        (
            "CrCR",
            curve_has_nonfinite_key(&emitter.create_count_random),
        ),
        ("CrI", curve_has_nonfinite_key(&emitter.create_interval)),
        (
            "CrIR",
            curve_has_nonfinite_key(&emitter.create_interval_random),
        ),
        ("Gra", curve_has_nonfinite_key(&emitter.gravity)),
        ("GraR", curve_has_nonfinite_key(&emitter.gravity_random)),
        ("ARs", curve_has_nonfinite_key(&emitter.air_resistance)),
        (
            "ARsR",
            curve_has_nonfinite_key(&emitter.air_resistance_random),
        ),
        ("Col", color_curve_has_nonfinite_key(&emitter.color)),
        ("Pos", curve3_has_nonfinite_key(&emitter.position)),
        ("Rot", curve3_has_nonfinite_key(&emitter.rotation)),
        ("Scl", curve3_has_nonfinite_key(&emitter.scale)),
        (
            "Life",
            emitter.life.enabled
                && (!emitter.life.value.is_finite() || !emitter.life.value_random.is_finite()),
        ),
    ]
    .into_iter()
    .filter_map(|(name, invalid)| invalid.then_some(name))
    .collect::<Vec<_>>();
    if !nonfinite_emitter.is_empty() {
        warnings.push(format!(
            "emitter[{index}].{} contains nonfinite curve or life data; creation, motion, transform, or lifetime sampling may be skipped",
            nonfinite_emitter.join(", ")
        ));
    }
    let nonfinite_curves: Vec<&str> = match &emitter.data {
        Some(AvfxEmitterData::Cone(data)) => emitter_curve_fields(
            [
                ("AnX", &data.rotation.angles[0]),
                ("AnY", &data.rotation.angles[1]),
                ("AnZ", &data.rotation.angles[2]),
                ("AnXR", &data.rotation.angles_random[0]),
                ("AnYR", &data.rotation.angles_random[1]),
                ("AnZR", &data.rotation.angles_random[2]),
                ("InS", &data.inner_size),
                ("InSR", &data.inner_size_random),
                ("OuS", &data.outer_size),
                ("OuSR", &data.outer_size_random),
                ("IjS", &data.injection_speed),
                ("IjSR", &data.injection_speed_random),
                ("IjA", &data.injection_angle),
                ("IjAR", &data.injection_angle_random),
            ]
            .into_iter(),
        ),
        Some(AvfxEmitterData::ConeModel(data)) => emitter_curve_fields(
            [
                ("AnX", &data.rotation.angles[0]),
                ("AnY", &data.rotation.angles[1]),
                ("AnZ", &data.rotation.angles[2]),
                ("AnXR", &data.rotation.angles_random[0]),
                ("AnYR", &data.rotation.angles_random[1]),
                ("AnZR", &data.rotation.angles_random[2]),
                ("Rad", &data.radius),
                ("RadR", &data.radius_random),
                ("IjS", &data.injection_speed),
                ("IjSR", &data.injection_speed_random),
                ("IjA", &data.injection_angle),
                ("IjAR", &data.injection_angle_random),
            ]
            .into_iter(),
        ),
        Some(AvfxEmitterData::SphereModel(data)) => emitter_curve_fields(
            [
                ("AnX", &data.rotation.angles[0]),
                ("AnY", &data.rotation.angles[1]),
                ("AnZ", &data.rotation.angles[2]),
                ("AnXR", &data.rotation.angles_random[0]),
                ("AnYR", &data.rotation.angles_random[1]),
                ("AnZR", &data.rotation.angles_random[2]),
                ("Rads", &data.radius),
                ("IjS", &data.injection_speed),
                ("IjSR", &data.injection_speed_random),
            ]
            .into_iter(),
        ),
        Some(AvfxEmitterData::CylinderModel(data)) => emitter_curve_fields(
            [
                ("AnX", &data.rotation.angles[0]),
                ("AnY", &data.rotation.angles[1]),
                ("AnZ", &data.rotation.angles[2]),
                ("AnXR", &data.rotation.angles_random[0]),
                ("AnYR", &data.rotation.angles_random[1]),
                ("AnZR", &data.rotation.angles_random[2]),
                ("Len", &data.length),
                ("Rad", &data.radius),
                ("IjS", &data.injection_speed),
                ("IjSR", &data.injection_speed_random),
            ]
            .into_iter(),
        ),
        Some(AvfxEmitterData::Model(data)) => emitter_curve_fields(
            [
                ("AnX", &data.rotation.angles[0]),
                ("AnY", &data.rotation.angles[1]),
                ("AnZ", &data.rotation.angles[2]),
                ("AnXR", &data.rotation.angles_random[0]),
                ("AnYR", &data.rotation.angles_random[1]),
                ("AnZR", &data.rotation.angles_random[2]),
                ("IjS", &data.injection_speed),
                ("IjSR", &data.injection_speed_random),
            ]
            .into_iter(),
        ),
        _ => Vec::new(),
    };
    if !nonfinite_curves.is_empty() {
        warnings.push(format!(
            "emitter[{index}].Data.{} contains nonfinite curve key data; shape births may produce nonfinite position, direction, or speed",
            nonfinite_curves.join(", ")
        ));
    }
    match &emitter.data {
        Some(AvfxEmitterData::ConeModel(data)) => {
            if data.generate_method as u8 > 7 {
                warnings.push(format!(
                    "emitter[{index}].Data.GeMT={} is unsupported; cone-model births are skipped",
                    data.generate_method
                ));
            } else if data.divide_x as u8 == 0 || data.divide_y as u8 == 0 {
                warnings.push(format!("emitter[{index}].Data: zero low-byte DivX/DivY cannot provide finite cone-model directions; births are skipped"));
            }
        }
        Some(AvfxEmitterData::SphereModel(data)) => {
            if data.generate_method as u8 > 7 {
                warnings.push(format!(
                    "emitter[{index}].Data.GeMT={} is unsupported; sphere-model births are skipped",
                    data.generate_method
                ));
            } else if data.divide_x as i8 <= 0 {
                warnings.push(format!("emitter[{index}].Data: nonpositive signed-byte DivX leaves the client direction unwritten; sphere-model births are skipped"));
            } else if data.divide_y as i8 == 0 {
                warnings.push(format!("emitter[{index}].Data: zero signed-byte DivY produces nonfinite sphere-model directions; births are skipped"));
            }
        }
        Some(AvfxEmitterData::CylinderModel(data)) => {
            if data.generate_method as u8 > 7 {
                warnings.push(format!(
                    "emitter[{index}].Data.GeMT={} is unsupported; cylinder-model births are skipped",
                    data.generate_method
                ));
            } else if data.divide_x as u8 == 0 {
                warnings.push(format!("emitter[{index}].Data: zero low-byte DivX leaves the client direction unwritten; cylinder-model births are skipped"));
            }
        }
        Some(AvfxEmitterData::Model(data)) => {
            if data.generate_method as u8 > 7 {
                warnings.push(format!(
                    "emitter[{index}].Data.GeMT={} is unsupported; model births are skipped",
                    data.generate_method
                ));
            }
        }
        _ => {}
    }
}

fn emitter_curve_fields<'a>(
    fields: impl IntoIterator<Item = (&'static str, &'a AvfxCurve)>,
) -> Vec<&'static str> {
    fields
        .into_iter()
        .filter_map(|(name, curve)| curve_has_nonfinite_key(curve).then_some(name))
        .collect()
}

/// `TLst`/`MdNo` 等 IntList 叶子：保留原始字节，运行时按 signed byte 消费。
fn read_int_list(payload: &[u8]) -> Vec<i32> {
    payload.iter().map(|byte| i32::from(*byte)).collect()
}

fn parse_particle_texture(node: &AvfxNodeView) -> AvfxParticleTexture {
    let fields = Fields::walk(node.payload());
    let texture_list = fields.bytes("TLst").map(read_int_list).unwrap_or_default();
    AvfxParticleTexture {
        enabled: fields.boolean("bEna").unwrap_or(false),
        uv_set_index: fields.i32("UvSN").unwrap_or(0),
        texture_index: fields.i32("TxNo").unwrap_or(-1),
        mask_texture_index: texture_list.first().copied().unwrap_or(-1),
        calculate_color: fields.i32("TCCT").unwrap_or(0),
        calculate_alpha: fields.i32("TCAT").unwrap_or(0),
        color_to_alpha: fields.boolean("bC2A").unwrap_or(false),
        use_screen_copy: fields.boolean("bUSC").unwrap_or(false),
        previous_frame_copy: fields.boolean("bPFC").unwrap_or(false),
        use_chara_portrait: fields.boolean("bUOS").unwrap_or(false),
        texture_filter: fields.i32("TFT").unwrap_or(1),
        texture_border_u: fields.i32("TBUT").unwrap_or(0),
        texture_border_v: fields.i32("TBVT").unwrap_or(0),
        tex_n: node.child("TxN").map(|child| parse_curve(&child)),
        tex_n_random: node.child("TxNR").map(|child| parse_curve(&child)),
        texture_list,
    }
}

/// `TN` 法线贴图块。
fn parse_particle_texture_normal(node: &AvfxNodeView) -> AvfxParticleTextureNormal {
    let fields = Fields::walk(node.payload());
    AvfxParticleTextureNormal {
        enabled: fields.boolean("bEna").unwrap_or(false),
        uv_set_index: fields.i32("UvSN").unwrap_or(0),
        texture_filter: fields.i32("TFT").unwrap_or(1),
        texture_border_u: fields.i32("TBUT").unwrap_or(0),
        texture_border_v: fields.i32("TBVT").unwrap_or(0),
        texture_index: fields.i32("TxNo").unwrap_or(-1),
        power: node
            .child("NPow")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
    }
}

/// `TR` 反射贴图块。
fn parse_particle_texture_reflection(node: &AvfxNodeView) -> AvfxParticleTextureReflection {
    let fields = Fields::walk(node.payload());
    AvfxParticleTextureReflection {
        enabled: fields.boolean("bEna").unwrap_or(false),
        use_screen_copy: fields.boolean("bUSC").unwrap_or(false),
        texture_filter: fields.i32("TFT").unwrap_or(1),
        calculate_color: fields.i32("TCCT").unwrap_or(0),
        texture_index: fields.i32("TxNo").unwrap_or(-1),
        rate: node
            .child("Rate")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        power: node
            .child("RPow")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
    }
}

/// `TP` 调色板贴图块。
fn parse_particle_texture_palette(node: &AvfxNodeView) -> AvfxParticleTexturePalette {
    let fields = Fields::walk(node.payload());
    AvfxParticleTexturePalette {
        enabled: fields.boolean("bEna").unwrap_or(false),
        texture_filter: fields.i32("TFT").unwrap_or(1),
        texture_border: fields.i32("TBT").unwrap_or(0),
        texture_index: fields.i32("TxNo").unwrap_or(-1),
        offset: node
            .child("POff")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        offset_random: node
            .child("POfR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
    }
}

/// 粒子 `Data` 块：按粒子类型解析（Model 的 MdNo 列表 + ColB/ColE、
/// LightModel 的 MNO、Powder 的 CnOf、Quad 的 SS）。
fn parse_particle_data(
    node: &AvfxNodeView,
    particle_type: Option<ParticleType>,
) -> AvfxParticleData {
    let Some(data) = node.child("Data") else {
        return AvfxParticleData::None;
    };
    match particle_type {
        Some(ParticleType::Model) => {
            let fields = Fields::walk(data.payload());
            let curve = |name: &str| data.child(name).map(|child| parse_curve(&child));
            let curve3 = |name: &str| data.child(name).map(|child| parse_curve3(&child));
            AvfxParticleData::Model {
                model_number_random_value: fields.i32("MNRv").unwrap_or(0),
                model_number_random_type: fields.i32("MNRt").unwrap_or(0),
                model_number_random_interval: fields.i32("MNRi").unwrap_or(0),
                fresnel_type: fields.i32("FrsT").unwrap_or(0),
                directional_light_type: fields.i32("DLT").unwrap_or(0),
                point_light_type: fields.i32("PLT").unwrap_or(0),
                is_lightning: fields.boolean("bLgt").unwrap_or(false),
                is_morph: fields.boolean("bShp").unwrap_or(false),
                model_indexes: fields.bytes("MdNo").map(read_int_list).unwrap_or_default(),
                animation_number: curve("NoAn"),
                morph: curve("Moph"),
                fresnel_curve: curve("FrC"),
                fresnel_curve_random: curve("FrCR"),
                fresnel_rotation: curve3("FrRt"),
                color_begin: data
                    .child("ColB")
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default(),
                color_end: data
                    .child("ColE")
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default(),
            }
        }
        Some(ParticleType::LightModel) => AvfxParticleData::LightModel {
            model_index: data.scalar_i32("MNO").unwrap_or(-1),
        },
        Some(ParticleType::Powder) => {
            let fields = Fields::walk(data.payload());
            AvfxParticleData::Powder {
                use_character_movement: fields.boolean("bMV").unwrap_or(false),
                use_character_location: fields.boolean("bLoc").unwrap_or(false),
                is_lightning: fields.boolean("bLgt").unwrap_or(false),
                directional_light_type: fields.i32("LgtT").unwrap_or(0),
                center_offset: fields.f32("CnOf").unwrap_or(0.0),
            }
        }
        Some(ParticleType::Quad) => {
            let fields = Fields::walk(data.payload());
            AvfxParticleData::Quad {
                scaling_scale: fields.i32("SS").unwrap_or(1),
                is_movement_particle: fields.boolean("bMP").unwrap_or(false),
            }
        }
        Some(ParticleType::Windmill) => AvfxParticleData::Windmill {
            uv_type: data.scalar_i32("WUvT").unwrap_or(0),
        },
        Some(ParticleType::Line) => {
            let fields = Fields::walk(data.payload());
            AvfxParticleData::Line(AvfxParticleDataLine {
                line_count: fields.i32("LnCT").unwrap_or(0),
                length: data
                    .child("Len")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                length_random: data
                    .child("LenR")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                color_begin: data
                    .child("ColB")
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default(),
                color_end: data
                    .child("ColE")
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default(),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::Laser) => AvfxParticleData::Laser(AvfxParticleDataLaser {
            length: data
                .child("Len")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            length_random: data
                .child("LenR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            width: data
                .child("Wdt")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            width_random: data
                .child("WdtR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            source: parse_generic_data(&data),
        }),
        Some(ParticleType::Polyline) => {
            let fields = Fields::walk(data.payload());
            let curve = |name: &str| {
                data.child(name)
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default()
            };
            let color = |name: &str| {
                data.child(name)
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default()
            };
            AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                create_line_type: fields.i32("LnCT").unwrap_or(0),
                not_billboard_base_axis_type: fields.i32("NBBA").unwrap_or(0),
                bind_weapon_type: fields.i32("BWpT").unwrap_or(0),
                point_count: fields.i32("PnC").unwrap_or(0),
                point_count_center: fields.i32("PnCC").unwrap_or(0),
                point_count_end_distortion: fields.i32("PnED").unwrap_or(0),
                use_edge: fields.boolean("bEdg").unwrap_or(false),
                not_billboard: fields.boolean("bNtB").unwrap_or(false),
                bind_weapon: fields.boolean("BdWp").unwrap_or(false),
                connect_target: fields.boolean("bCtg").unwrap_or(false),
                connect_target_reverse: fields.boolean("bCtr").unwrap_or(false),
                tag_number: fields.i32("TagN").unwrap_or(0),
                is_spline: fields.boolean("bSpl").unwrap_or(false),
                is_local: fields.boolean("bLcl").unwrap_or(false),
                cf: curve("CF"),
                cf_random: curve("CFR"),
                width: curve("Wd"),
                width_random: curve("WdR"),
                width_begin: curve("WdB"),
                width_begin_random: curve("WdBR"),
                width_center: curve("WdC"),
                width_center_random: curve("WdCR"),
                width_end: curve("WdE"),
                width_end_random: curve("WdER"),
                length: curve("Len"),
                length_random: curve("LenR"),
                softness: curve("Sft"),
                softness_random: curve("SftR"),
                point_distortion: curve("PnDs"),
                color_begin: color("ColB"),
                color_center: color("ColC"),
                color_end: color("ColE"),
                color_edge_begin: color("CoEB"),
                color_edge_center: color("CoEC"),
                color_edge_end: color("CoEE"),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::Decal) => {
            let fields = Fields::walk(data.payload());
            AvfxParticleData::Decal(AvfxParticleDataDecal {
                scaling_scale: fields.f32("SS").unwrap_or(0.0),
                ddtt: fields.i32("DDTT").unwrap_or(0),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::DecalRing) => {
            let fields = Fields::walk(data.payload());
            AvfxParticleData::DecalRing(AvfxParticleDataDecalRing {
                width: data
                    .child("WID")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                width_random: data
                    .child("WIDR")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                scaling_scale: fields.f32("SS").unwrap_or(0.0),
                ring_fan: fields.f32("RF").unwrap_or(0.0),
                ddtt: fields.i32("DDTT").unwrap_or(0),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::Disc) => {
            let fields = Fields::walk(data.payload());
            let curve = |name: &str| {
                data.child(name)
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default()
            };
            let color = |name: &str| {
                data.child(name)
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default()
            };
            AvfxParticleData::Disc(AvfxParticleDataDisc {
                parts_count: fields.i32("PrtC").unwrap_or(1),
                parts_count_u: fields.i32("PCnU").unwrap_or(2),
                parts_count_v: fields.i32("PCnV").unwrap_or(2),
                point_interval_factor_v: fields.f32("PIFU").unwrap_or(0.0),
                angle: curve("Ang"),
                angle_random: curve("AngR"),
                height_begin_inner: curve("HBI"),
                height_begin_inner_random: curve("HBIR"),
                height_end_inner: curve("HEI"),
                height_end_inner_random: curve("HEIR"),
                height_begin_outer: curve("HBO"),
                height_begin_outer_random: curve("HBOR"),
                height_end_outer: curve("HEO"),
                height_end_outer_random: curve("HEOR"),
                width_begin: curve("WB"),
                width_begin_random: curve("WBR"),
                width_end: curve("WE"),
                width_end_random: curve("WER"),
                radius_begin: curve("RB"),
                radius_begin_random: curve("RBR"),
                radius_end: curve("RE"),
                radius_end_random: curve("RER"),
                color_edge_inner: color("CEI"),
                color_edge_outer: color("CEO"),
                scaling_scale: fields.i32("SS").unwrap_or(100),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::Polygon) => AvfxParticleData::Polygon(AvfxParticleDataPolygon {
            count: data
                .child("Cnt")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            count_random: data
                .child("CntR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            source: parse_generic_data(&data),
        }),
        Some(ParticleType::ModelSkin) => {
            let fields = Fields::walk(data.payload());
            let curve = |name: &str| {
                data.child(name)
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default()
            };
            let curve3 = |name: &str| {
                data.child(name)
                    .map(|child| parse_curve3(&child))
                    .unwrap_or_default()
            };
            let color = |name: &str| {
                data.child(name)
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default()
            };
            AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                fresnel_type: fields.i32("FrsT").unwrap_or(0),
                aura_target: fields.i32("AuTT").unwrap_or(0),
                cm: fields.i32("bCM").unwrap_or(0),
                fresnel_curve: curve("FrC"),
                fresnel_curve_random: curve("FrCR"),
                fresnel_rotation: curve3("FrRt"),
                color_begin: color("ColB"),
                color_end: color("ColE"),
                sem: curve("SEM"),
                sem_random: curve("SEMR"),
                eem: curve("EEM"),
                eem_random: curve("EEMR"),
                uv_point_density: curve3("UVPD"),
                source: parse_generic_data(&data),
            })
        }
        Some(ParticleType::Dissolve) => {
            let fields = Fields::walk(data.payload());
            let curve = |name: &str| {
                data.child(name)
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default()
            };
            let dissolve_color = data.child("EdC");
            let nested_curve = |name: &str| {
                dissolve_color
                    .as_ref()
                    .and_then(|color| color.child(name))
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default()
            };
            let nested_color = |name: &str| {
                dissolve_color
                    .as_ref()
                    .and_then(|color| color.child(name))
                    .map(|child| parse_color_curve(&child))
                    .unwrap_or_default()
            };
            AvfxParticleData::Dissolve(AvfxParticleDataDissolve {
                reverse: fields.boolean("bRev").unwrap_or(false),
                bst: fields.i32("BST").unwrap_or(0),
                npt: fields.i32("NPT").unwrap_or(0),
                dissolve_target: fields.i32("DTT").unwrap_or(0),
                erosion_rate: curve("EroR"),
                end_color_width: curve("EdW"),
                color: AvfxParticleDataDissolveColor {
                    start: nested_color("StrC"),
                    middle: nested_color("MidC"),
                    end: nested_color("EndC"),
                    scale_r: nested_curve("SclR"),
                    scale_g: nested_curve("SclG"),
                    scale_b: nested_curve("SclB"),
                    brightness: nested_curve("Bri"),
                },
                mid_color_width: curve("EdCW"),
                start_color_width: curve("ECMP"),
                intensity: curve("Int"),
                source: parse_generic_data(&data),
            })
        }
        _ => AvfxParticleData::Other(parse_generic_data(&data)),
    }
}

fn parse_generic_data(node: &AvfxNodeView) -> AvfxGenericData {
    let mut data = AvfxGenericData {
        raw_payload: node.payload().to_vec(),
        ..Default::default()
    };
    for child in node.children() {
        let name = child.name().to_owned();
        if child.payload().len() == 4 {
            data.scalars
                .insert(name, read_u32(child.payload()).unwrap() as i32);
        } else if child.child("Keys").is_some() || child.child("KeyC").is_some() {
            data.curves.insert(name, parse_curve(&child));
        } else if child.children().any(|field| {
            matches!(
                field.name(),
                "X" | "Y" | "Z" | "XR" | "YR" | "ZR" | "ACT" | "ACTR"
            )
        }) {
            data.curve3s.insert(name, parse_curve3(&child));
        } else if child.children().any(|field| {
            matches!(
                field.name(),
                "RGB"
                    | "A"
                    | "Bri"
                    | "SclR"
                    | "SclG"
                    | "SclB"
                    | "SclA"
                    | "RanR"
                    | "RanG"
                    | "RanB"
                    | "RanA"
                    | "RBri"
            )
        }) {
            data.color_curves.insert(name, parse_color_curve(&child));
        }
    }
    data
}

/// `Smpl` 块（Powder 子粒子参数）。
fn parse_particle_simple(node: &AvfxNodeView) -> Option<AvfxParticleSimple> {
    let simple = node.child("Smpl")?;
    let fields = Fields::walk(simple.payload());
    let f32_pair = |a: &str, b: &str| [fields.f32(a).unwrap_or(0.0), fields.f32(b).unwrap_or(0.0)];
    let colors = fields
        .bytes("Cols")
        .filter(|bytes| bytes.len() >= 16)
        .map(|bytes| {
            let mut colors = [[0u8; 4]; 4];
            for (index, chunk) in bytes.chunks_exact(4).take(4).enumerate() {
                colors[index] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            }
            colors
        })
        .unwrap_or_default();
    let frames = fields
        .bytes("Frms")
        .filter(|bytes| bytes.len() >= 8)
        .map(|bytes| {
            let mut frames = [0i16; 4];
            for (index, chunk) in bytes.chunks_exact(2).take(4).enumerate() {
                frames[index] = i16::from_le_bytes([chunk[0], chunk[1]]);
            }
            frames
        })
        .unwrap_or_default();
    let f32_triplet = |a: &str, b: &str, c: &str| {
        [
            fields.f32(a).unwrap_or(0.0),
            fields.f32(b).unwrap_or(0.0),
            fields.f32(c).unwrap_or(0.0),
        ]
    };
    Some(AvfxParticleSimple {
        injection_position_type: fields.i32("SIPT").unwrap_or(0),
        injection_direction_type: fields.i32("SIDT").unwrap_or(0),
        base_direction_type: fields.i32("SBDT").unwrap_or(0),
        create_count: fields.i32("CCnt").unwrap_or(0),
        create_area: f32_triplet("CrAX", "CrAY", "CrAZ"),
        coord_accuracy: f32_triplet("CAX", "CAY", "CAZ"),
        coord_gravity: f32_triplet("CGX", "CGY", "CGZ"),
        scale_start: f32_pair("SBX", "SBY"),
        scale_end: f32_pair("SEX", "SEY"),
        scale_curve: fields.f32("SC").unwrap_or(1.0),
        scale_rand_x: f32_pair("SRX0", "SRX1"),
        scale_rand_y: f32_pair("SRY0", "SRY1"),
        rotation_start: f32_triplet("RIX", "RIY", "RIZ"),
        rotation_add: f32_triplet("RAX", "RAY", "RAZ"),
        rotation_base: f32_triplet("RBX", "RBY", "RBZ"),
        rotation_velocity: f32_triplet("RVX", "RVY", "RVZ"),
        velocity_min: fields.f32("VMin").unwrap_or(0.0),
        velocity_max: fields.f32("VMax").unwrap_or(0.0),
        velocity_flattery_rate: fields.f32("FltR").unwrap_or(0.0),
        velocity_flattery_speed: fields.f32("FltS").unwrap_or(0.0),
        uv_cell: [
            fields.i32("UvCU").unwrap_or(1).max(1),
            fields.i32("UvCV").unwrap_or(1).max(1),
        ],
        uv_interval: fields.i32("UvIv").unwrap_or(1),
        uv_no_random: fields.i32("UvNR").unwrap_or(0),
        uv_loop_count: fields.i32("UvLC").unwrap_or(0),
        injection_model_index: fields.i32("IJMN").unwrap_or(-1),
        injection_vertex_bind_model_index: fields.i32("VBMN").unwrap_or(-1),
        injection_radial_dir: f32_pair("IRD0", "IRD1"),
        pivot: f32_pair("PvtX", "PvtY"),
        block_num: fields.i32("BlkN").unwrap_or(0),
        line_length_min: fields.f32("LLin").unwrap_or(0.0),
        line_length_max: fields.f32("LLax").unwrap_or(0.0),
        create_interval: fields.i32("CrI").unwrap_or(0),
        create_interval_on_movement: fields.f32("CIM").unwrap_or(0.0),
        create_interval_on_movement_random: fields.f32("CIMR").unwrap_or(0.0),
        create_interval_random: fields.i32("CrIR").unwrap_or(0),
        create_interval_count: fields.i32("CrIC").unwrap_or(1),
        create_interval_life: fields.i32("CrIL").unwrap_or(30),
        create_life_random: fields.i32("CrLR").unwrap_or(0),
        create_new_after_delete: fields.boolean("bCrN").unwrap_or(false),
        uv_reverse: fields.boolean("bRUV").unwrap_or(false),
        scale_random_link: fields.boolean("bSRL").unwrap_or(false),
        bind_parent: fields.boolean("bBnP").unwrap_or(false),
        scale_by_parent: fields.boolean("bSnP").unwrap_or(false),
        polyline_tag: fields.i32("PolT").unwrap_or(0),
        colors,
        frames,
    })
}

fn parse_particle(node: &AvfxNodeView, index: usize, warnings: &mut Vec<String>) -> AvfxParticle {
    let raw_particle_type = node.scalar("PrVT").unwrap_or(u32::MAX);
    let particle_type = Some(ParticleType::from_raw(raw_particle_type));
    let particle = AvfxParticle {
        particle_type,
        raw_particle_type,
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        draw_mode: node.scalar_i32("RMT").unwrap_or(0),
        culling_type: node.scalar_i32("CulT").unwrap_or(0),
        depth_test: node.boolean("DsDt").unwrap_or(true),
        depth_write: node.boolean("DsDw").unwrap_or(false),
        rotation_direction_base: node.scalar_i32("RBDT").unwrap_or(0),
        rotation_order: node.scalar_i32("RoOT").unwrap_or(0),
        coord_compute_order: node.scalar_i32("CCOT").unwrap_or(0),
        env_light_type: node.scalar_i32("EnvT").unwrap_or(0),
        dir_light_type: node.scalar_i32("DirT").unwrap_or(0),
        uv_precision: node.scalar_i32("UVPT").unwrap_or(0),
        draw_priority: node.scalar_i32("DwPr").unwrap_or(0),
        is_soft_particle: node.boolean("DsSp").unwrap_or(false),
        collision_type: node.scalar_i32("Coll").unwrap_or(0),
        s11_enabled: node.boolean("bS11").unwrap_or(false),
        sh_u_t: node.scalar_i32("ShUT").unwrap_or(0),
        sh_r: node.scalar_i32("ShR").unwrap_or(0),
        sh_t: node.scalar_i32("ShT").unwrap_or(0),
        uni_v: node.scalar_i32("UniV").unwrap_or(0),
        hyb_v: node.scalar_i32("HybV").unwrap_or(0),
        e24_enabled: node.boolean("bE24").unwrap_or(false),
        is_apply_tone_map: node.boolean("bATM").unwrap_or(false),
        is_apply_fog: node.boolean("bAFg").unwrap_or(false),
        clip_near_enable: node.boolean("bNea").unwrap_or(false),
        clip_far_enable: node.boolean("bFar").unwrap_or(false),
        clip_near_start: node.f32("NeSt").unwrap_or(0.0),
        clip_near_end: node.f32("NeEd").unwrap_or(0.0),
        clip_far_start: node.f32("FaSt").unwrap_or(0.0),
        clip_far_end: node.f32("FaEd").unwrap_or(0.0),
        clip_base_point: node.scalar_i32("FaBP").unwrap_or(0),
        apply_rate_environment: node.scalar_i32("EvAR").unwrap_or(0),
        apply_rate_directional: node.scalar_i32("DlAR").unwrap_or(0),
        apply_rate_light_buffer: node.scalar_i32("LBAR").unwrap_or(0),
        depth_offset_type: node.scalar_i32("DOTy").unwrap_or(0),
        depth_offset: node.f32("DpOf").unwrap_or(0.0),
        simple_anim_enable: node.boolean("bSCt").unwrap_or(false),
        life: parse_optional_life(node),
        gravity: node
            .child("Gra")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        gravity_random: node
            .child("GraR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        air_resistance: node
            .child("ARs")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        air_resistance_random: node
            .child("ARsR")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        scale: node
            .child("Scl")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        rotation: node
            .child("Rot")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        position: node
            .child("Pos")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
        color: node
            .child("Col")
            .map(|child| parse_color_curve(&child))
            .unwrap_or_default(),
        rotation_velocity: [
            node.child("VRX")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRY")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRZ")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        ],
        rotation_velocity_random: [
            node.child("VRXR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRYR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            node.child("VRZR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        ],
        texture_color1: node.child("TC1").map(|tc| parse_particle_texture(&tc)),
        texture_color2: node.child("TC2").map(|tc| parse_particle_texture(&tc)),
        texture_color3: node.child("TC3").map(|tc| parse_particle_texture(&tc)),
        texture_color4: node.child("TC4").map(|tc| parse_particle_texture(&tc)),
        texture_normal: node
            .child("TN")
            .map(|tn| parse_particle_texture_normal(&tn)),
        texture_reflection: node
            .child("TR")
            .map(|tr| parse_particle_texture_reflection(&tr)),
        texture_palette: node
            .child("TP")
            .map(|tp| parse_particle_texture_palette(&tp)),
        texture_distortion: node.child("TD").map(|td| {
            let fields = Fields::walk(td.payload());
            AvfxParticleDistortion {
                enabled: fields.boolean("bEna").unwrap_or(false),
                target_uv: [
                    fields.boolean("bT1").unwrap_or(false),
                    fields.boolean("bT2").unwrap_or(false),
                    fields.boolean("bT3").unwrap_or(false),
                    fields.boolean("bT4").unwrap_or(false),
                ],
                uv_set_index: fields.i32("UvSN").unwrap_or(0),
                texture_index: fields.i32("TxNo").unwrap_or(-1),
                power: td
                    .child("DPow")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                texture_filter: fields.i32("TFT").unwrap_or(1),
                texture_border_u: fields.i32("TBUT").unwrap_or(0),
                texture_border_v: fields.i32("TBVT").unwrap_or(0),
            }
        }),
        uv_sets: node
            .children_named("UvSt")
            .map(|uv_set| AvfxUvSet {
                calculate_uv: uv_set.scalar("CUvT").unwrap_or(0),
                scale: uv_set
                    .child("Scl")
                    .map(|child| parse_curve2(&child))
                    .unwrap_or_default(),
                scroll: uv_set
                    .child("Scr")
                    .map(|child| parse_curve2(&child))
                    .unwrap_or_default(),
                rotation: uv_set
                    .child("Rot")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
                rotation_random: uv_set
                    .child("RotR")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
            })
            .collect(),
        data: parse_particle_data(node, particle_type),
        simple: parse_particle_simple(node),
    };
    warn_count_mismatch(node, "UvSN", particle.uv_sets.len(), warnings);
    for (set_index, set) in particle.uv_sets.iter().enumerate() {
        if set.calculate_uv > 1 {
            warnings.push(format!(
                "Ptcl[{index}].UvSt[{set_index}].CUvT={} is unknown; the renderer preserves it and uses ByParameter UVs",
                set.calculate_uv
            ));
        }
    }
    let direction_fields: Vec<_> = ["VRX", "VRY", "VRZ"]
        .into_iter()
        .zip(&particle.rotation_velocity)
        .chain(
            ["VRXR", "VRYR", "VRZR"]
                .into_iter()
                .zip(&particle.rotation_velocity_random),
        )
        .filter_map(|(name, curve)| curve.keys.iter().any(|key| key.z != 0.0).then_some(name))
        .collect();
    if !direction_fields.is_empty() {
        warnings.push(format!(
            "Ptcl[{index}]: nonzero {} direction curves use continuous integration; client update clocks and full coordinate inheritance remain approximate",
            direction_fields.join(", ")
        ));
    }
    if let AvfxParticleData::Windmill { uv_type } = particle.data {
        if uv_type as u8 > 1 {
            warnings.push(format!(
                "Windmill WUvT={uv_type} has unsupported low byte {}; particle is skipped",
                uv_type as u8
            ));
        }
    }
    let soft_particle_supported = matches!(
        particle.particle_type,
        Some(
            ParticleType::Powder
                | ParticleType::Windmill
                | ParticleType::Line
                | ParticleType::Laser
                | ParticleType::Model
                | ParticleType::Quad
                | ParticleType::Polygon
                | ParticleType::Disc
                | ParticleType::LightModel
                | ParticleType::Polyline
        )
    );
    if particle.is_soft_particle && !soft_particle_supported {
        warnings.push(format!(
            "Ptcl[{index}].DsSp=1 requests soft-particle scene-depth fade; this particle type is not connected to scene depth/screen-copy sampling and is rendered without that fade"
        ));
    }
    if let Some(texture) = particle
        .texture_color1
        .as_ref()
        .filter(|texture| texture.enabled)
    {
        if !texture.use_screen_copy && texture.use_chara_portrait {
            warnings.push(format!(
                "Ptcl[{index}].TC1.bUOS=1 selects the client character-portrait (-5) source; rendering requires an injected UI portrait provider"
            ));
        }
        let animated = texture.tc1_has_animated_selection();
        if animated && texture.texture_list.len() > u8::MAX as usize {
            warnings.push(format!(
                "Ptcl[{index}].TC1.TLst has {} entries but the client stores its dynamic count in 8 bits; only the low-byte count is addressable",
                texture.texture_list.len()
            ));
        }
    }
    let mut diagnose_texture_filter = |layer: &str, enabled: bool, filter: i32| {
        if !enabled || (0..=4).contains(&filter) {
            return;
        }
        warnings.push(format!(
            "Ptcl[{index}].{layer}.TFT={filter} is unknown; the renderer preserves it and uses a linear sampler fallback"
        ));
    };
    for (layer, texture) in [
        ("TC1", particle.texture_color1.as_ref()),
        ("TC2", particle.texture_color2.as_ref()),
        ("TC3", particle.texture_color3.as_ref()),
        ("TC4", particle.texture_color4.as_ref()),
    ] {
        if let Some(texture) = texture {
            diagnose_texture_filter(layer, texture.enabled, texture.texture_filter);
        }
    }
    if let Some(texture) = &particle.texture_distortion {
        diagnose_texture_filter("TD", texture.enabled, texture.texture_filter);
    }
    if let Some(texture) = &particle.texture_palette {
        diagnose_texture_filter("TP", texture.enabled, texture.texture_filter);
    }
    if matches!(
        particle.particle_type,
        Some(ParticleType::Model | ParticleType::LightModel)
    ) {
        if let Some(texture) = &particle.texture_normal {
            diagnose_texture_filter("TN", texture.enabled, texture.texture_filter);
        }
        if let Some(texture) = &particle.texture_reflection {
            diagnose_texture_filter(
                "TR",
                texture.enabled && !texture.use_screen_copy,
                texture.texture_filter,
            );
        }
    }
    if let AvfxParticleData::Polyline(data) = &particle.data {
        let unsupported_static = !matches!(data.create_line_type, 0 | 1)
            || !(2..=64).contains(&data.point_count)
            || data.bind_weapon
            || data.bind_weapon_type != 0
            || data.connect_target
            || data.tag_number != 0
            || particle.simple_anim_enable
            || !matches!(particle.rotation_direction_base, 0..=2)
            || (data.not_billboard && !matches!(data.not_billboard_base_axis_type, 0..=2));
        let unsupported_continuous = data.create_line_type == 0 && data.is_spline;
        if unsupported_static || unsupported_continuous {
            warnings.push(format!(
                "Ptcl[{index}] Polyline configuration is unsupported for the available history/binding path; particle is skipped"
            ));
        }
    }
    let nonfinite_curves = match &particle.data {
        AvfxParticleData::Line(data) => [
            ("Len", curve_has_nonfinite_key(&data.length)),
            ("LenR", curve_has_nonfinite_key(&data.length_random)),
            ("ColB", color_curve_has_nonfinite_key(&data.color_begin)),
            ("ColE", color_curve_has_nonfinite_key(&data.color_end)),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        AvfxParticleData::Laser(data) => [
            ("Len", curve_has_nonfinite_key(&data.length)),
            ("LenR", curve_has_nonfinite_key(&data.length_random)),
            ("Wdt", curve_has_nonfinite_key(&data.width)),
            ("WdtR", curve_has_nonfinite_key(&data.width_random)),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        AvfxParticleData::Polyline(data) => [
            ("CF", curve_has_nonfinite_key(&data.cf)),
            ("CFR", curve_has_nonfinite_key(&data.cf_random)),
            ("Wd", curve_has_nonfinite_key(&data.width)),
            ("WdR", curve_has_nonfinite_key(&data.width_random)),
            ("WdB", curve_has_nonfinite_key(&data.width_begin)),
            ("WdBR", curve_has_nonfinite_key(&data.width_begin_random)),
            ("WdC", curve_has_nonfinite_key(&data.width_center)),
            ("WdCR", curve_has_nonfinite_key(&data.width_center_random)),
            ("WdE", curve_has_nonfinite_key(&data.width_end)),
            ("WdER", curve_has_nonfinite_key(&data.width_end_random)),
            ("Len", curve_has_nonfinite_key(&data.length)),
            ("LenR", curve_has_nonfinite_key(&data.length_random)),
            ("Sft", curve_has_nonfinite_key(&data.softness)),
            ("SftR", curve_has_nonfinite_key(&data.softness_random)),
            ("PnD", curve_has_nonfinite_key(&data.point_distortion)),
            ("ColB", color_curve_has_nonfinite_key(&data.color_begin)),
            ("ColC", color_curve_has_nonfinite_key(&data.color_center)),
            ("ColE", color_curve_has_nonfinite_key(&data.color_end)),
            (
                "CoEB",
                color_curve_has_nonfinite_key(&data.color_edge_begin),
            ),
            (
                "CoEC",
                color_curve_has_nonfinite_key(&data.color_edge_center),
            ),
            ("CoEE", color_curve_has_nonfinite_key(&data.color_edge_end)),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        AvfxParticleData::Disc(data) => [
            ("Ang", curve_has_nonfinite_key(&data.angle)),
            ("AngR", curve_has_nonfinite_key(&data.angle_random)),
            ("RdiB", curve_has_nonfinite_key(&data.radius_begin)),
            ("RdiBR", curve_has_nonfinite_key(&data.radius_begin_random)),
            ("RdiE", curve_has_nonfinite_key(&data.radius_end)),
            ("RdiER", curve_has_nonfinite_key(&data.radius_end_random)),
            ("WdB", curve_has_nonfinite_key(&data.width_begin)),
            ("WdBR", curve_has_nonfinite_key(&data.width_begin_random)),
            ("WdE", curve_has_nonfinite_key(&data.width_end)),
            ("WdER", curve_has_nonfinite_key(&data.width_end_random)),
            ("HtBI", curve_has_nonfinite_key(&data.height_begin_inner)),
            (
                "HtBIR",
                curve_has_nonfinite_key(&data.height_begin_inner_random),
            ),
            ("HtEI", curve_has_nonfinite_key(&data.height_end_inner)),
            (
                "HtEIR",
                curve_has_nonfinite_key(&data.height_end_inner_random),
            ),
            ("HtBO", curve_has_nonfinite_key(&data.height_begin_outer)),
            (
                "HtBOR",
                curve_has_nonfinite_key(&data.height_begin_outer_random),
            ),
            ("HtEO", curve_has_nonfinite_key(&data.height_end_outer)),
            (
                "HtEOR",
                curve_has_nonfinite_key(&data.height_end_outer_random),
            ),
            (
                "ColI",
                color_curve_has_nonfinite_key(&data.color_edge_inner),
            ),
            (
                "ColO",
                color_curve_has_nonfinite_key(&data.color_edge_outer),
            ),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        AvfxParticleData::Polygon(data) => [
            ("Cnt", curve_has_nonfinite_key(&data.count)),
            ("CntR", curve_has_nonfinite_key(&data.count_random)),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        AvfxParticleData::Decal(data) => [("SS", !data.scaling_scale.is_finite())]
            .into_iter()
            .filter_map(|(name, invalid)| invalid.then_some(name))
            .collect::<Vec<_>>(),
        AvfxParticleData::DecalRing(data) => [
            ("WID", curve_has_nonfinite_key(&data.width)),
            ("WIDR", curve_has_nonfinite_key(&data.width_random)),
            ("SS", !data.scaling_scale.is_finite()),
            ("RF", !data.ring_fan.is_finite()),
        ]
        .into_iter()
        .filter_map(|(name, invalid)| invalid.then_some(name))
        .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    if !nonfinite_curves.is_empty() {
        warnings.push(format!(
            "Ptcl[{index}].Data.{} contains nonfinite curve key data; sampler may skip nonfinite geometry or color",
            nonfinite_curves.join(", ")
        ));
    }
    let nonfinite_particle_curves = [
        ("Pos", curve3_has_nonfinite_key(&particle.position)),
        ("Rot", curve3_has_nonfinite_key(&particle.rotation)),
        ("Scl", curve3_has_nonfinite_key(&particle.scale)),
        ("Gra", curve_has_nonfinite_key(&particle.gravity)),
        ("GraR", curve_has_nonfinite_key(&particle.gravity_random)),
        ("ARs", curve_has_nonfinite_key(&particle.air_resistance)),
        (
            "ARsR",
            curve_has_nonfinite_key(&particle.air_resistance_random),
        ),
        ("Col", color_curve_has_nonfinite_key(&particle.color)),
    ]
    .into_iter()
    .filter_map(|(name, invalid)| invalid.then_some(name))
    .collect::<Vec<_>>();
    if !nonfinite_particle_curves.is_empty() {
        warnings.push(format!(
            "Ptcl[{index}].{} contains nonfinite curve key data; particle transform, motion, or color sampling may be skipped",
            nonfinite_particle_curves.join(", ")
        ));
    }
    let nonfinite_particle_scalars = [
        (
            "Life",
            particle.life.enabled
                && (!particle.life.value.is_finite() || !particle.life.value_random.is_finite()),
        ),
        ("NeSt", !particle.clip_near_start.is_finite()),
        ("NeEd", !particle.clip_near_end.is_finite()),
        ("FaSt", !particle.clip_far_start.is_finite()),
        ("FaEd", !particle.clip_far_end.is_finite()),
        ("DpOf", !particle.depth_offset.is_finite()),
    ]
    .into_iter()
    .filter_map(|(name, invalid)| invalid.then_some(name))
    .collect::<Vec<_>>();
    if !nonfinite_particle_scalars.is_empty() {
        warnings.push(format!(
            "Ptcl[{index}].{} contains nonfinite scalar data; lifetime, clipping, or depth-offset sampling may be skipped",
            nonfinite_particle_scalars.join(", ")
        ));
    }
    let mut nonfinite_aux_curves = Vec::new();
    for (layer, texture) in [
        ("TC1", particle.texture_color1.as_ref()),
        ("TC2", particle.texture_color2.as_ref()),
        ("TC3", particle.texture_color3.as_ref()),
        ("TC4", particle.texture_color4.as_ref()),
    ] {
        if let Some(texture) = texture {
            push_nonfinite_aux_curve(
                &mut nonfinite_aux_curves,
                format!("{layer}.TxN"),
                texture.tex_n.as_ref(),
            );
            push_nonfinite_aux_curve(
                &mut nonfinite_aux_curves,
                format!("{layer}.TxNR"),
                texture.tex_n_random.as_ref(),
            );
        }
    }
    if let Some(texture) = &particle.texture_normal {
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TN.NPow".to_owned(),
            Some(&texture.power),
        );
    }
    if let Some(texture) = &particle.texture_reflection {
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TR.Rate".to_owned(),
            Some(&texture.rate),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TR.RPow".to_owned(),
            Some(&texture.power),
        );
    }
    if let Some(texture) = &particle.texture_palette {
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TP.POff".to_owned(),
            Some(&texture.offset),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TP.POfR".to_owned(),
            Some(&texture.offset_random),
        );
    }
    if let Some(texture) = &particle.texture_distortion {
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "TD.DPow".to_owned(),
            Some(&texture.power),
        );
    }
    for (set_index, set) in particle.uv_sets.iter().enumerate() {
        let prefix = format!("UvSt[{set_index}]");
        if curve2_has_nonfinite_key(&set.scale) {
            nonfinite_aux_curves.push(format!("{prefix}.Scl"));
        }
        if curve2_has_nonfinite_key(&set.scroll) {
            nonfinite_aux_curves.push(format!("{prefix}.Scr"));
        }
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            format!("{prefix}.Rot"),
            Some(&set.rotation),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            format!("{prefix}.RotR"),
            Some(&set.rotation_random),
        );
    }
    if let AvfxParticleData::Model {
        animation_number,
        morph,
        fresnel_curve,
        fresnel_curve_random,
        fresnel_rotation,
        color_begin,
        color_end,
        ..
    } = &particle.data
    {
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "Data.NoAn".to_owned(),
            animation_number.as_ref(),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "Data.Moph".to_owned(),
            morph.as_ref(),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "Data.FrC".to_owned(),
            fresnel_curve.as_ref(),
        );
        push_nonfinite_aux_curve(
            &mut nonfinite_aux_curves,
            "Data.FrCR".to_owned(),
            fresnel_curve_random.as_ref(),
        );
        if fresnel_rotation
            .as_ref()
            .is_some_and(curve3_has_nonfinite_key)
        {
            nonfinite_aux_curves.push("Data.FrRt".to_owned());
        }
        if color_curve_has_nonfinite_key(color_begin) {
            nonfinite_aux_curves.push("Data.ColB".to_owned());
        }
        if color_curve_has_nonfinite_key(color_end) {
            nonfinite_aux_curves.push("Data.ColE".to_owned());
        }
    }
    if !nonfinite_aux_curves.is_empty() {
        warnings.push(format!(
            "Ptcl[{index}].{} contains nonfinite auxiliary curve key data; UV, texture, palette, distortion, or Fresnel sampling may be invalid",
            nonfinite_aux_curves.join(", ")
        ));
    }
    if particle.simple_anim_enable {
        if let Some(simple) = &particle.simple {
            let nonfinite_simple = [
                (
                    "CrAX/Y/Z",
                    simple.create_area.iter().any(|v| !v.is_finite()),
                ),
                (
                    "CAX/Y/Z",
                    simple.coord_accuracy.iter().any(|v| !v.is_finite()),
                ),
                (
                    "CGX/Y/Z",
                    simple.coord_gravity.iter().any(|v| !v.is_finite()),
                ),
                ("SBX/SBY", simple.scale_start.iter().any(|v| !v.is_finite())),
                ("SEX/SEY", simple.scale_end.iter().any(|v| !v.is_finite())),
                ("SC", !simple.scale_curve.is_finite()),
                ("SRX0/1", simple.scale_rand_x.iter().any(|v| !v.is_finite())),
                ("SRY0/1", simple.scale_rand_y.iter().any(|v| !v.is_finite())),
                (
                    "RIX/Y/Z",
                    simple.rotation_start.iter().any(|v| !v.is_finite()),
                ),
                (
                    "RAX/Y/Z",
                    simple.rotation_add.iter().any(|v| !v.is_finite()),
                ),
                (
                    "RBX/Y/Z",
                    simple.rotation_base.iter().any(|v| !v.is_finite()),
                ),
                (
                    "RVX/Y/Z",
                    simple.rotation_velocity.iter().any(|v| !v.is_finite()),
                ),
                (
                    "VMin/VMax",
                    !simple.velocity_min.is_finite() || !simple.velocity_max.is_finite(),
                ),
                (
                    "FltR/FltS",
                    !simple.velocity_flattery_rate.is_finite()
                        || !simple.velocity_flattery_speed.is_finite(),
                ),
                (
                    "IRD0/IRD1",
                    simple.injection_radial_dir.iter().any(|v| !v.is_finite()),
                ),
                ("PvtX/PvtY", simple.pivot.iter().any(|v| !v.is_finite())),
                (
                    "LLin/LLax",
                    !simple.line_length_min.is_finite() || !simple.line_length_max.is_finite(),
                ),
                (
                    "CIM/CIMR",
                    !simple.create_interval_on_movement.is_finite()
                        || !simple.create_interval_on_movement_random.is_finite(),
                ),
            ]
            .into_iter()
            .filter_map(|(name, invalid)| invalid.then_some(name))
            .collect::<Vec<_>>();
            if !nonfinite_simple.is_empty() {
                warnings.push(format!(
                    "Ptcl[{index}].Smpl.{} contains nonfinite values; simple child motion or geometry may be skipped",
                    nonfinite_simple.join(", ")
                ));
            }
        }
    }
    if particle.is_apply_fog {
        warnings.push(format!(
            "Ptcl[{index}].bAFg=1 requests particle fog, but scene fog parameters are not connected"
        ));
    }
    if particle.clip_near_enable {
        warnings.push(format!(
            "Ptcl[{index}].bNea=1 near clip ({}, {}) is parsed but not connected",
            particle.clip_near_start, particle.clip_near_end
        ));
    }
    if particle.clip_far_enable {
        warnings.push(format!(
            "Ptcl[{index}].bFar=1 far clip ({}, {}) is parsed but not connected",
            particle.clip_far_start, particle.clip_far_end
        ));
    }
    let depth_offset_supported = matches!(
        particle.particle_type,
        Some(
            ParticleType::Quad
                | ParticleType::Powder
                | ParticleType::Model
                | ParticleType::LightModel
        )
    );
    if !matches!(particle.depth_offset_type, 0 | 1)
        || (particle.depth_offset != 0.0 && !depth_offset_supported)
    {
        warnings.push(format!(
            "Ptcl[{index}].DOTy/DpOf=({}, {}) depth offset is not connected for this type and mode",
            particle.depth_offset_type, particle.depth_offset
        ));
    }
    if particle.is_apply_tone_map {
        warnings.push(format!(
            "Ptcl[{index}].bATM=1 tone-map application is parsed but the VFX forward target has no per-particle tone-map stage"
        ));
    }
    if particle.env_light_type != 0 || particle.dir_light_type != 0 {
        warnings.push(format!(
            "Ptcl[{index}].EnvT/DirT=({}, {}) lighting selections are parsed but VFX lighting is not connected",
            particle.env_light_type, particle.dir_light_type
        ));
    }
    if let AvfxParticleData::Disc(data) = &particle.data {
        for (name, raw, minimum) in [
            ("PrtC", data.parts_count, 1),
            ("PCnU", data.parts_count_u, 2),
            ("PCnV", data.parts_count_v, 2),
        ] {
            if (raw as u8) < minimum {
                warnings.push(format!(
                    "Ptcl[{index}].Data.{name}: low byte {} cannot form a Disc grid; particle is skipped",
                    raw as u8
                ));
            }
        }
        if !data.point_interval_factor_v.is_finite() {
            warnings.push(format!(
                "Ptcl[{index}].Data.PIFU: nonfinite Disc point interval; particle is skipped"
            ));
        }
    }
    let decal_depth_type = match &particle.data {
        AvfxParticleData::Decal(data) => Some(data.ddtt),
        AvfxParticleData::DecalRing(data) => Some(data.ddtt),
        _ => None,
    };
    if let Some(depth_type @ 3..=5) = decal_depth_type {
        warnings.push(format!(
            "Ptcl[{index}].Data.DDTT={depth_type} uses the deferred material GBuffer path; forward preview skips the particle"
        ));
    } else if let Some(depth_type) = decal_depth_type.filter(|value| !(0..=5).contains(value)) {
        warnings.push(format!(
            "Ptcl[{index}].Data.DDTT={depth_type} is unsupported; particle is skipped"
        ));
    }
    if matches!(
        particle.particle_type,
        Some(ParticleType::Powder | ParticleType::Line)
    ) && particle.simple_anim_enable
        && particle.simple.is_none()
    {
        warnings.push(format!(
            "{:?} bSCt is enabled but Smpl is missing; particle is skipped",
            particle.particle_type.unwrap()
        ));
    }
    if particle.particle_type == Some(ParticleType::Powder)
        && particle.simple_anim_enable
        && particle
            .simple
            .as_ref()
            .is_some_and(|simple| simple.injection_position_type as i8 == 2)
    {
        warnings.push(format!(
            "Ptcl[{index}].Smpl.SIPT=2 requires an external point source; preview has no source and skips child births"
        ));
    }
    let simple_line_supported = particle.particle_type == Some(ParticleType::Line)
        && particle.simple_anim_enable
        && particle.simple.as_ref().is_some_and(|simple| {
            simple.injection_position_type == 0
                && simple.injection_direction_type == 0
                && simple.injection_model_index == -1
                && simple.injection_vertex_bind_model_index == -1
                && !simple.bind_parent
        });
    if particle.particle_type == Some(ParticleType::Line)
        && particle.simple_anim_enable
        && particle.simple.is_some()
        && !simple_line_supported
    {
        warnings.push(
            "Line Smpl injection/binding configuration is unsupported; particle is skipped"
                .to_owned(),
        );
    }
    // RMT 9..12 share RGB equations with 1..4; their fog policy is scene-specific.
    if !(0..=12).contains(&particle.draw_mode) {
        warnings.push(format!(
            "particle RMT={} is unsupported; renderer uses Add fallback",
            particle.draw_mode
        ));
    }
    if node.scalar("RBDT").is_some()
        && particle.particle_type != Some(ParticleType::Windmill)
        && !(particle.particle_type == Some(ParticleType::Powder) && !particle.simple_anim_enable)
        && !(particle.particle_type == Some(ParticleType::Laser)
            && matches!(particle.rotation_direction_base, 0..=2))
        && !(particle.particle_type == Some(ParticleType::Line)
            && !particle.simple_anim_enable
            && matches!(particle.rotation_direction_base, 0..=2))
        && !(matches!(
            particle.particle_type,
            Some(ParticleType::Quad | ParticleType::Model | ParticleType::LightModel)
        ) && matches!(
            particle.rotation_direction_base,
            rotation_direction_base::MOVE_DIRECTION
                | rotation_direction_base::MOVE_DIRECTION_BILLBOARD
        ))
        && !simple_line_supported
        && !matches!(
            particle.rotation_direction_base,
            rotation_direction_base::X
                | rotation_direction_base::Y
                | rotation_direction_base::Z
                | rotation_direction_base::BILLBOARD_AXIS_Y
                | rotation_direction_base::SCREEN_BILLBOARD
                | rotation_direction_base::CAMERA_BILLBOARD
                | rotation_direction_base::CAMERA_BILLBOARD_AXIS_Y
                | rotation_direction_base::TREE_BILLBOARD
                | rotation_direction_base::NONE
        )
    {
        warnings.push(format!(
            "particle RBDT={} is unsupported; renderer uses {} fallback",
            particle.rotation_direction_base,
            if particle.is_billboard() {
                "camera billboard"
            } else {
                "fixed orientation"
            }
        ));
    }
    match particle.particle_type {
        Some(
            ParticleType::Quad
            | ParticleType::Powder
            | ParticleType::Windmill
            | ParticleType::Model
            | ParticleType::Disc
            | ParticleType::Polygon
            | ParticleType::Decal
            | ParticleType::DecalRing
            | ParticleType::Polyline
            | ParticleType::LightModel,
        ) => {}
        Some(ParticleType::Laser) if matches!(particle.rotation_direction_base, 0..=2) => {}
        Some(ParticleType::Laser) => warnings.push(format!(
            "Ptcl[{index}] Laser RBDT={} is unsupported; particle is skipped",
            particle.rotation_direction_base
        )),
        Some(ParticleType::Line)
            if !particle.simple_anim_enable
                && matches!(particle.rotation_direction_base, 0..=2) => {}
        Some(ParticleType::Line) if particle.simple_anim_enable => {}
        Some(ParticleType::Line) => warnings.push(format!(
            "Ptcl[{index}] Line RBDT={} is unsupported; particle is skipped",
            particle.rotation_direction_base
        )),
        Some(ParticleType::ModelSkin) => warnings.push(format!(
            "Ptcl[{index}] ModelSkin targets client model-surface render resources; the preview has no compatible Aura surface resource or shader and skips the particle"
        )),
        Some(ParticleType::Parameter) => warnings.push(format!(
            "Ptcl[{index}] Parameter has no client particle-object allocation; particle is skipped"
        )),
        Some(ParticleType::Reserve0) => warnings.push(format!(
            "Ptcl[{index}] Reserve0 has no client particle-object allocation; particle is skipped"
        )),
        Some(ParticleType::Dissolve) => warnings.push(format!(
            "Ptcl[{index}] Dissolve targets client model render resources; the preview has no compatible target path and skips the particle"
        )),
        Some(ParticleType::Unknown(raw)) if raw == u32::MAX => {}
        Some(ParticleType::Unknown(raw)) => warnings.push(format!(
            "Ptcl[{index}].PrVT={raw} is unknown; particle is skipped"
        )),
        None => {}
    }
    particle
}

/// 根级 `Modl` 块：每块一个模型（VFXEditor `AvfxModel`），`VEmt` 发射顶点与
/// `VDrw`/`VIdx` 绘制网格可共存于同一块。坐标保持 avfx 模型空间原样
/// （与武器模型同向，不翻转）。
fn parse_model(
    node: &AvfxNodeView,
    model_index: usize,
    warnings: &mut Vec<String>,
) -> VfxModelGeometry {
    let mut model = VfxModelGeometry::default();
    for (name, stride) in [("VEmt", 28), ("VNum", 2), ("VDrw", 36), ("VIdx", 6)] {
        if let Some(data) = node.child(name) {
            let remainder = data.payload().len() % stride;
            if remainder != 0 {
                warnings.push(format!(
                    "Modl[{model_index}].{name}: {remainder} trailing bytes outside {stride}-byte records"
                ));
            }
        }
    }
    if let Some(emit_vertexes) = node.child("VEmt") {
        for chunk in emit_vertexes.payload().chunks_exact(28) {
            model.emit_vertices.push(VfxEmitVertex {
                position: [
                    f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                    f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
                    f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]),
                ],
                normal: [
                    f32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]),
                    f32::from_le_bytes([chunk[16], chunk[17], chunk[18], chunk[19]]),
                    f32::from_le_bytes([chunk[20], chunk[21], chunk[22], chunk[23]]),
                ],
                color: [chunk[24], chunk[25], chunk[26], chunk[27]],
            });
        }
    }
    if let Some(numbers) = node.child("VNum") {
        model.emit_vertex_numbers = numbers
            .payload()
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
    }
    if model.emit_vertex_numbers.len() != model.emit_vertices.len() {
        warnings.push(format!(
            "Modl[{model_index}].VNum: {} entries for {} VEmt vertices",
            model.emit_vertex_numbers.len(),
            model.emit_vertices.len()
        ));
    }
    let invalid = model
        .emit_vertex_numbers
        .iter()
        .filter(|&&index| index as u16 as usize >= model.emit_vertices.len())
        .count();
    if invalid > 0 {
        warnings.push(format!("Modl[{model_index}].VNum: {invalid} indexes outside VEmt; model births selecting them are skipped"));
    }
    if let Some(draw_vertexes) = node.child("VDrw") {
        let mut vertices = Vec::with_capacity(draw_vertexes.payload().len() / 36);
        for chunk in draw_vertexes.payload().chunks_exact(36) {
            vertices.push(VfxDrawVertex {
                position: [
                    f16_to_f32([chunk[0], chunk[1]]),
                    f16_to_f32([chunk[2], chunk[3]]),
                    f16_to_f32([chunk[4], chunk[5]]),
                ],
                position_w: f16_to_f32([chunk[6], chunk[7]]),
                normal: [chunk[8], chunk[9], chunk[10], chunk[11]],
                tangent: [chunk[12], chunk[13], chunk[14], chunk[15]],
                // 四组 UV（half2 ×4，偏移 20/24/28/32；VFXEditor `AvfxVertex`）。
                uvs: [
                    [
                        f16_to_f32([chunk[20], chunk[21]]),
                        f16_to_f32([chunk[22], chunk[23]]),
                    ],
                    [
                        f16_to_f32([chunk[24], chunk[25]]),
                        f16_to_f32([chunk[26], chunk[27]]),
                    ],
                    [
                        f16_to_f32([chunk[28], chunk[29]]),
                        f16_to_f32([chunk[30], chunk[31]]),
                    ],
                    [
                        f16_to_f32([chunk[32], chunk[33]]),
                        f16_to_f32([chunk[34], chunk[35]]),
                    ],
                ],
                color: [chunk[16], chunk[17], chunk[18], chunk[19]],
            });
        }
        let mut indices = Vec::new();
        if let Some(indexes) = node.child("VIdx") {
            let mut invalid_triangles = 0;
            for chunk in indexes.payload().chunks_exact(6) {
                let triangle =
                    [0, 1, 2].map(|i| i16::from_le_bytes([chunk[i * 2], chunk[i * 2 + 1]]));
                if triangle
                    .iter()
                    .any(|&index| index < 0 || index as usize >= vertices.len())
                {
                    invalid_triangles += 1;
                    continue;
                }
                indices.extend(triangle.map(|index| index as u32));
            }
            if invalid_triangles > 0 {
                warnings.push(format!(
                    "Modl[{model_index}].VIdx: skipped {invalid_triangles} triangles with invalid vertex indexes"
                ));
            }
        }
        model.draw = Some(VfxDrawModel { vertices, indices });
    } else if node
        .child("VIdx")
        .is_some_and(|indexes| !indexes.payload().is_empty())
    {
        warnings.push(format!(
            "Modl[{model_index}].VIdx: indexes without VDrw vertices"
        ));
    }
    model
}

/// IEEE 754 half（f16）→ f32。
fn f16_to_f32(bits: [u8; 2]) -> f32 {
    f16::from_le_bytes(bits).to_f32()
}

/// `Name` 叶子：字符串 + 3 字节尾 + 4 字节对齐填充（VFXEditor
/// `AvfxBinderPropertiesName`），按 NUL 截断。
fn read_binder_name(payload: &[u8]) -> String {
    let end = payload
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(payload.len());
    String::from_utf8_lossy(&payload[..end]).into_owned()
}

fn parse_binder_properties(node: &AvfxNodeView) -> AvfxBinderProperties {
    let fields = Fields::walk(node.payload());
    AvfxBinderProperties {
        bind_point_type: fields.i32("BPT").unwrap_or(0),
        bind_target_point_type: fields.i32("BPTP").unwrap_or(0),
        binder_name: fields
            .bytes("Name")
            .map(read_binder_name)
            .unwrap_or_default(),
        bind_point_id: fields.i32("BPID").unwrap_or(-1),
        generate_delay: fields.i32("GenD").unwrap_or(0),
        coord_update_frame: fields.i32("CoUF").unwrap_or(-1),
        ring_enabled: fields.boolean("bRng").unwrap_or(false),
        ring_progress_time: fields.i32("RnPT").unwrap_or(0),
        ring_position: [
            fields.f32("RnPX").unwrap_or(0.0),
            fields.f32("RnPY").unwrap_or(0.0),
            fields.f32("RnPZ").unwrap_or(0.0),
        ],
        ring_radius: fields.f32("RnRd").unwrap_or(0.0),
        bct: fields.i32("BCT").unwrap_or(0),
        position: node
            .child("Pos")
            .map(|child| parse_curve3(&child))
            .unwrap_or_default(),
    }
}

fn parse_binder(node: &AvfxNodeView) -> AvfxBinder {
    // ParsedBool is commonly a single byte (0/255) in installed AVFX.
    // scalar() only reads four-byte payloads and silently loses these flags.
    let boolean = |name| {
        node.child(name)
            .and_then(|child| read_bool(child.payload()))
            .unwrap_or(false)
    };
    let properties_start = node
        .child("PrpS")
        .map(|props| parse_binder_properties(&props));
    let bind_point_id = properties_start
        .as_ref()
        .map(|props| props.bind_point_id)
        .unwrap_or(-1);
    AvfxBinder {
        binder_type: node.scalar("BnVr").unwrap_or(0),
        bind_point_id,
        start_to_global_direction: boolean("bStG"),
        vfx_scale_enabled: boolean("bVSc"),
        vfx_scale_bias: node.f32("bVSb").unwrap_or(0.0),
        vfx_scale_depth_offset: boolean("bVSd"),
        vfx_scale_interpolation: boolean("bVSi"),
        transform_scale: node.scalar_i32("bTSc").unwrap_or(0),
        transform_scale_depth_offset: boolean("bTSd"),
        transform_scale_interpolation: boolean("bTSi"),
        following_target_orientation: boolean("bFTO"),
        document_scale_enabled: boolean("bDSE"),
        adjust_to_screen_enabled: boolean("bATS"),
        ify: boolean("bIFY"),
        bet: boolean("bBET"),
        life: node.scalar_i32("Life").unwrap_or(0),
        rotation_type: node.scalar_i32("RoTp").unwrap_or(0),
        properties_start,
        properties_1: node
            .child("Prp1")
            .map(|props| parse_binder_properties(&props)),
        properties_2: node
            .child("Prp2")
            .map(|props| parse_binder_properties(&props)),
        properties_goal: node
            .child("PrpG")
            .map(|props| parse_binder_properties(&props)),
        data: node.child("Data").map(|data| AvfxBinderData {
            spring_strength: data.child("SpS").map(|curve| parse_curve(&curve)),
            spring_strength_random: data.child("SpSR").map(|curve| parse_curve(&curve)),
            carry_over_factor: data.child("COF").map(|curve| parse_curve(&curve)),
            carry_over_factor_random: data.child("COFR").map(|curve| parse_curve(&curve)),
            distance: data.child("Dst").map(|curve| parse_curve(&curve)),
            distance_random: data.child("DstR").map(|curve| parse_curve(&curve)),
            source: parse_generic_data(&data),
        }),
    }
}

fn warn_unsupported_binder(binder: &AvfxBinder, index: usize, warnings: &mut Vec<String>) {
    if !binder.vfx_scale_bias.is_finite() {
        warnings.push(format!(
            "Bind[{index}].bVSb={} is nonfinite; binder scale sampling may be skipped",
            binder.vfx_scale_bias
        ));
    }
    if matches!(binder.binder_type, 1 | 4) {
        let kind = if binder.binder_type == 4 {
            "LinearAdjust"
        } else {
            "Linear"
        };
        warnings.push(format!(
            "Bind[{index}]: {kind} binder has limited local matrix preview support; external targets and full curve motion remain incomplete"
        ));
    } else if binder.binder_type != 0 {
        warnings.push(format!(
            "Bind[{index}]: BnVr={} binder type is parsed but unsupported",
            binder.binder_type
        ));
    }
    if binder
        .data
        .as_ref()
        .is_some_and(|data| !data.source.raw_payload.is_empty())
    {
        warnings.push(format!(
            "Bind[{index}].Data: full binder motion is retained but not sampled"
        ));
    }
    let mut fields = Vec::new();
    if binder.following_target_orientation {
        fields.push("bFTO");
    }
    if binder.document_scale_enabled {
        fields.push("bDSE");
    }
    if binder.adjust_to_screen_enabled {
        fields.push("bATS");
    }
    if binder.ify {
        fields.push("bIFY");
    }
    if binder.bet {
        fields.push("bBET");
    }
    if binder.transform_scale != 0 {
        fields.push("bTSc");
    }
    if binder.vfx_scale_depth_offset {
        fields.push("bVSd");
    }
    if binder.transform_scale_depth_offset {
        fields.push("bTSd");
    }
    if !fields.is_empty() {
        warnings.push(format!(
            "Bind[{index}]: {} binder transform options have incomplete runtime support",
            fields.join(", ")
        ));
    }
    if binder.rotation_type != 0 {
        warnings.push(format!(
            "Bind[{index}]: RoTp={} binder rotation type is parsed but unsupported",
            binder.rotation_type
        ));
    }
    for (name, properties) in [
        ("PrpS", binder.properties_start.as_ref()),
        ("Prp1", binder.properties_1.as_ref()),
        ("Prp2", binder.properties_2.as_ref()),
        ("PrpG", binder.properties_goal.as_ref()),
    ] {
        let Some(properties) = properties else {
            continue;
        };
        if properties.ring_enabled {
            warnings.push(format!(
                "Bind[{index}].{name}: ring bind-point motion is parsed but unsupported"
            ));
        }
        let nonfinite_ring = properties.ring_enabled
            && (properties
                .ring_position
                .iter()
                .any(|value| !value.is_finite())
                || !properties.ring_radius.is_finite());
        if nonfinite_ring {
            warnings.push(format!(
                "Bind[{index}].{name}: ring position or radius is nonfinite; binder motion may be skipped"
            ));
        }
        if curve3_has_nonfinite_key(&properties.position) {
            warnings.push(format!(
                "Bind[{index}].{name}.Pos contains nonfinite curve key data; binder position sampling may be skipped"
            ));
        }
        if properties
            .position
            .x
            .as_ref()
            .is_some_and(|curve| !curve.keys.is_empty())
            || properties
                .position
                .y
                .as_ref()
                .is_some_and(|curve| !curve.keys.is_empty())
            || properties
                .position
                .z
                .as_ref()
                .is_some_and(|curve| !curve.keys.is_empty())
        {
            warnings.push(format!(
                "Bind[{index}].{name}.Pos: binder position animation is parsed but unsupported"
            ));
        }
    }
}

fn parse_effector_data(raw_type: u32, data: &AvfxNodeView) -> AvfxEffectorData {
    let curve = |name| data.child(name).map(|node| parse_curve(&node));
    let curve3 = |name| data.child(name).map(|node| parse_curve3(&node));
    let color = |name| {
        data.child(name)
            .map(|node| parse_color_curve(&node))
            .unwrap_or_default()
    };
    let flag = |name| data.boolean(name).unwrap_or(false);
    match raw_type {
        0 => AvfxEffectorData::PointLight {
            color: color("Col"),
            distance_scale: curve("DstS"),
            rotation: curve3("Rot"),
            position: curve3("Pos"),
            attenuation: data.scalar_i32("Attn"),
            enable_shadow: flag("bSdw"),
            enable_character_shadow: flag("bChS"),
            enable_map_shadow: flag("bMpS"),
            enable_move_shadow: flag("bMvS"),
            shadow_create_distance_near: data.f32("SCDN"),
            shadow_create_distance_far: data.f32("SCDF"),
        },
        1 => AvfxEffectorData::DirectionalLight {
            ambient: color("Amb"),
            color: color("Col"),
            power: curve("Pow"),
            power_random: curve("PowR"),
            rotation: curve3("Rot"),
        },
        3 => AvfxEffectorData::ChromaticAberration {
            refraction_strength: curve("CSR"),
            separation_strength: curve("CSS"),
            effect_strength: curve("CGE"),
            strength: curve("Str"),
            gradation: curve("Gra"),
            inner_radius: curve("IRad"),
            outer_radius: curve("ORad"),
            fade_start_distance: data.f32("FSDc"),
            fade_end_distance: data.f32("FEDc"),
            fade_base_point: data.scalar_i32("FaBP"),
        },
        4 | 5 | 7 => {
            let directional = raw_type == 5;
            let blur = AvfxEffectorBlurData {
                length: curve("Len"),
                strength: curve("Str"),
                gradation: if directional { None } else { curve("Gra") },
                inner_radius: if directional { None } else { curve("IRad") },
                outer_radius: if directional { None } else { curve("ORad") },
                angle_strength: if directional { curve("AStr") } else { None },
                angle: if directional { curve("Ang") } else { None },
                fade_start_distance: data.f32("FSDc"),
                fade_end_distance: data.f32("FEDc"),
                fade_base_point: data.scalar_i32("FaBP"),
                one_side: directional && flag("bOS"),
            };
            match raw_type {
                4 => AvfxEffectorData::GaussianBlur(blur),
                5 => AvfxEffectorData::DirectionalBlur(blur),
                _ => AvfxEffectorData::RadialBlur(blur),
            }
        }
        6 | 9 => AvfxEffectorData::CameraQuake {
            attenuation: curve("Att"),
            attenuation_random: curve("AttR"),
            radius_out: curve("RdO"),
            radius_out_random: curve("RdOR"),
            radius_in: curve("RdI"),
            radius_in_random: curve("RdIR"),
            rotation: curve3("Rot"),
            position: curve3("Pos"),
        },
        _ => AvfxEffectorData::Other,
    }
}

/// `Efct` parameters and type-specific `Data`; runtime effects are not sampled.
fn parse_effector(node: &AvfxNodeView) -> AvfxEffector {
    let raw_effector_type = node.scalar("EfVT").unwrap_or(u32::MAX);
    let data_node = node.child("Data");
    AvfxEffector {
        raw_effector_type,
        rotation_order: node.scalar_i32("RoOT").unwrap_or(0),
        coord_compute_order: node.scalar_i32("CCOT").unwrap_or(0),
        affect_other_vfx: node.boolean("bAOV").unwrap_or(false),
        affect_game: node.boolean("bAGm").unwrap_or(false),
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        data: data_node
            .as_ref()
            .map(|data| parse_effector_data(raw_effector_type, data)),
        data_payload: data_node
            .map(|data| data.payload().to_vec())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_skin_combined_target_flags_select_each_loaded_weapon_slot() {
        let mut mounts = WeaponVfxAttachments {
            model_skin_targets: WeaponVfxModelTargets {
                weapon: Some("main.mdl".into()),
                off_hand: Some("sub.mdl".into()),
            },
            ..Default::default()
        };
        for (flags, expected) in [
            (2, vec!["main.mdl"]),
            (4, vec!["sub.mdl"]),
            (6, vec!["main.mdl", "sub.mdl"]),
            (7, vec!["main.mdl", "sub.mdl"]),
            (0x406, vec!["main.mdl", "sub.mdl"]),
            (16, vec![]),
        ] {
            assert_eq!(
                mounts.model_skin_target_paths(flags).collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(mounts.model_skin_target_path(6), None);
        mounts.model_skin_targets.off_hand = None;
        assert_eq!(
            mounts.model_skin_target_paths(6).collect::<Vec<_>>(),
            ["main.mdl"]
        );
        assert_eq!(mounts.model_skin_target_path(6), Some("main.mdl"));
    }

    #[test]
    fn element_target_matrix_applies_each_euler_axis_before_local_translation() {
        let point = VfxBindPoint {
            id: 3,
            parent_bone: None,
            translate: [4.0, -5.0, 6.0],
            rotate: [0.7, -0.4, 1.2],
        };
        let (sx, cx) = point.rotate[0].sin_cos();
        let (sy, cy) = point.rotate[1].sin_cos();
        let (sz, cz) = point.rotate[2].sin_cos();
        let input = [1.2, -0.4, 0.8];
        let x = [
            input[0],
            cx * input[1] - sx * input[2],
            sx * input[1] + cx * input[2],
        ];
        let y = [cy * x[0] + sy * x[2], x[1], -sy * x[0] + cy * x[2]];
        let rotated = [cz * y[0] - sz * y[1], sz * y[0] + cz * y[1], y[2]];
        let matrix = point.local_matrix();
        for row in 0..3 {
            let actual = matrix[row] * input[0]
                + matrix[4 + row] * input[1]
                + matrix[8 + row] * input[2]
                + matrix[12 + row];
            assert!((actual - rotated[row] - point.translate[row]).abs() < 1e-5);
        }
    }

    #[test]
    fn element_target_pose_keeps_parent_basis_and_uses_root_for_unknown_bones() {
        use crate::skeleton::{BoneTransform, ModelSkeleton, SkeletonPose, quat_from_axis_angle};
        let skeleton = ModelSkeleton {
            bone_names: vec!["root".into(), "tip".into()],
            parent_indices: vec![-1, 0],
            rest_pose: vec![
                BoneTransform {
                    translation: [10.0, 20.0, 30.0],
                    scale: [2.0, 3.0, 4.0],
                    ..BoneTransform::IDENTITY
                },
                BoneTransform {
                    translation: [1.0, 2.0, 3.0],
                    rotation: quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2),
                    ..BoneTransform::IDENTITY
                },
            ],
        };
        let point = VfxBindPoint {
            id: 3,
            parent_bone: Some("tip".into()),
            translate: [1.0, 2.0, 3.0],
            rotate: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
        };
        let points = [
            point.clone(),
            VfxBindPoint {
                parent_bone: Some("absent".into()),
                ..point.clone()
            },
            VfxBindPoint {
                parent_bone: None,
                ..point
            },
        ];
        let matrices =
            vfx_bind_point_matrices(&points, &skeleton, &SkeletonPose::rest_pose(&skeleton));
        let expected = [
            [
                0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 2.0, 0.0, 0.0, 0.0, 8.0, 29.0, 54.0, 1.0,
            ],
            [
                2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, -3.0, 0.0, 0.0, 12.0, 26.0, 42.0, 1.0,
            ],
        ];
        for (actual, expected) in matrices.iter().zip([expected[0], expected[1], expected[1]]) {
            for (a, e) in actual.iter().zip(expected) {
                assert!((a - e).abs() < 1e-5, "{actual:?}");
            }
        }
        // Skinning matrices at rest would be identity and lose both parents.
        assert_ne!(matrices[0], points[0].local_matrix());
        let empty = ModelSkeleton {
            bone_names: vec![],
            parent_indices: vec![],
            rest_pose: vec![],
        };
        assert_eq!(
            vfx_bind_point_matrices(&points, &empty, &SkeletonPose::new(0)),
            points
                .iter()
                .map(VfxBindPoint::local_matrix)
                .collect::<Vec<_>>()
        );
    }

    /// 构造块：名字按格式反写存储，不足 4 字节补空格。
    #[test]
    fn bone_pose_provider_preserves_shear_mirrors_and_rejects_invalid_target_identity() {
        use crate::{BoneTransform, ModelSkeleton, SkeletonPose, VfxRuntime, quat_from_axis_angle};
        let skeleton = ModelSkeleton {
            bone_names: vec!["root".into(), "tip".into()],
            parent_indices: vec![-1, 0],
            rest_pose: vec![
                BoneTransform {
                    translation: [10.0, 20.0, 30.0],
                    scale: [2.0, 3.0, -4.0],
                    ..BoneTransform::IDENTITY
                },
                BoneTransform {
                    translation: [1.0, 2.0, 0.0],
                    rotation: quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_4),
                    ..BoneTransform::IDENTITY
                },
            ],
        };
        let point = VfxBindPoint {
            id: 3,
            parent_bone: Some("tip".into()),
            translate: [2.0, 0.0, 0.0],
            rotate: [0.0; 3],
        };
        let pose = SkeletonPose::rest_pose(&skeleton);
        let data = WeaponVfxData {
            skeleton: Some(skeleton.clone()),
            bind_points: vec![point.clone()],
            ..Default::default()
        };
        let target = data.binder_targets_for_pose(&pose).unwrap()[0];
        let expected = [
            12.0 + 2.0_f32.sqrt() * 2.0,
            26.0 + 2.0_f32.sqrt() * 3.0,
            30.0,
        ];
        for (a, b) in target.matrix.position.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-5);
        }
        let [x, y, z] = target.matrix.basis;
        let dot: f32 = x.into_iter().zip(y).map(|(a, b)| a * b).sum();
        assert!(
            (dot - 2.5).abs() < 1e-5,
            "raw shear must not be orthogonalized"
        );
        assert_eq!(z, [0.0, 0.0, -4.0]);
        assert!(
            VfxRuntime::bind_point_pose_targets(&[point.clone(), point.clone()], &skeleton, &pose)
                .unwrap_err()
                .contains("duplicate")
        );
        let invalid = VfxBindPoint {
            translate: [f32::INFINITY, 0.0, 0.0],
            ..point
        };
        assert!(
            VfxRuntime::bind_point_pose_targets(&[invalid], &skeleton, &pose)
                .unwrap_err()
                .contains("finite")
        );
        let serialized = serde_json::to_value(&data).unwrap();
        assert!(
            serialized.get("skeleton").is_none(),
            "transient skeleton must not enter asset snapshots"
        );
    }

    fn block(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut reversed: Vec<u8> = name.bytes().collect();
        while reversed.len() < 4 {
            reversed.push(b' ');
        }
        reversed.reverse();
        out.extend_from_slice(&reversed[..4]);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out
    }

    fn u32_block(name: &str, value: u32) -> Vec<u8> {
        block(name, &value.to_le_bytes())
    }

    fn i32_block(name: &str, value: i32) -> Vec<u8> {
        u32_block(name, value as u32)
    }

    fn container(name: &str, children: Vec<Vec<u8>>) -> Vec<u8> {
        block(name, &children.concat())
    }

    fn key_bytes(time: i16, interpolation: u16, x: f32, y: f32, z: f32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&interpolation.to_le_bytes());
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
        out
    }

    /// 单轴曲线容器（KeyC/BvPr/BvPo/RanT/Keys）。
    fn curve(name: &str, keys: Vec<Vec<u8>>, post_behavior: u32) -> Vec<u8> {
        container(
            name,
            vec![
                u32_block("KeyC", keys.len() as u32),
                u32_block("BvPr", BEHAVIOR_CONST),
                u32_block("BvPo", post_behavior),
                u32_block("RanT", 0),
                block("Keys", &keys.concat()),
            ],
        )
    }

    /// 三轴曲线容器：x/y/z 各自的键列表（None = 轴缺省）。
    fn curve3(
        name: &str,
        x: Option<Vec<Vec<u8>>>,
        y: Option<Vec<Vec<u8>>>,
        z: Option<Vec<Vec<u8>>>,
    ) -> Vec<u8> {
        let mut children = vec![u32_block("ACT", 0), u32_block("ACTR", 0)];
        for (axis, keys) in [("X", x), ("Y", y), ("Z", z)] {
            if let Some(keys) = keys {
                children.push(curve(axis, keys, 0));
            }
        }
        container(name, children)
    }

    /// 颜色曲线容器（RGB/A/Bri 子曲线）。
    fn color_curve(
        name: &str,
        rgb: Vec<Vec<u8>>,
        alpha: Option<Vec<Vec<u8>>>,
        brightness: Option<Vec<Vec<u8>>>,
    ) -> Vec<u8> {
        let mut children = vec![curve("RGB", rgb, 0)];
        if let Some(alpha) = alpha {
            children.push(curve("A", alpha, 0));
        }
        if let Some(brightness) = brightness {
            children.push(curve("Bri", brightness, 0));
        }
        container(name, children)
    }

    fn scheduler_item(enabled: u32, start: i32, timeline: i32) -> Vec<u8> {
        vec![
            u32_block("bEna", enabled),
            i32_block("StTm", start),
            i32_block("TlNo", timeline),
        ]
        .concat()
    }

    #[test]
    fn scheduler_enable_uses_compiled_low_two_bits() {
        for (raw, enabled) in [
            (0, false),
            (1, true),
            (2, true),
            (3, true),
            (4, false),
            (5, true),
            (65536, false),
            (u32::MAX, true),
        ] {
            let bytes = container(
                "AVFX",
                vec![container(
                    "Schd",
                    vec![
                        u32_block("ItCn", 1),
                        container("Item", vec![scheduler_item(raw, 0, 0)]),
                        u32_block("TrCn", 1),
                        container("Trgr", vec![scheduler_item(raw, 0, 0)]),
                    ],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(
                file.schedulers[0].items[0].enabled, enabled,
                "ordinary raw {raw}"
            );
            assert_eq!(
                file.schedulers[0].triggers[0].enabled, enabled,
                "trigger raw {raw}"
            );
        }
    }

    #[test]
    #[ignore = "CPU: generate original Scheduler mapping/count reader observations first"]
    fn compare_original_scheduler_mapping_parser() {
        use serde_json::{Value, json};
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(output.join("scheduler-ordinary-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["packingCases"].as_array().unwrap();
        for (ordinal, case) in cases.iter().enumerate() {
            let enabled = case["enabledRaw"].as_i64().unwrap() as u32;
            let start = case["startRaw"].as_i64().unwrap() as i32;
            let timeline = case["timelineRaw"].as_i64().unwrap() as i32;
            let bytes = container(
                "AVFX",
                vec![container(
                    "Schd",
                    vec![
                        u32_block("ItCn", 1),
                        container("Item", vec![scheduler_item(enabled, start, timeline)]),
                        u32_block("TrCn", 12),
                        container("Trgr", vec![scheduler_item(enabled, start, timeline); 12]),
                    ],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.schedulers[0].item_count, Some(1));
            for (item, word) in [
                (&file.schedulers[0].items[0], "word"),
                (&file.schedulers[0].triggers[0], "triggerWord"),
            ] {
                assert_eq!(
                    item.enabled,
                    case[word].as_u64().unwrap() & 3 != 0,
                    "enabled {ordinal}"
                );
                assert_eq!(item.start_time, start, "raw start {ordinal}");
                assert_eq!(item.timeline_index, timeline, "raw timeline {ordinal}");
                assert_eq!(
                    i32::from(item.timeline_index as i16),
                    case["timeline"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    (item.start_time as u16).wrapping_shl(2),
                    case[word].as_u64().unwrap() as u16 & !3
                );
            }
        }
        for case in original["countCases"].as_array().unwrap() {
            let raw = case["raw"].as_i64().unwrap() as i32;
            let bytes = container(
                "AVFX",
                vec![container("Schd", vec![i32_block("ItCn", raw)])],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.schedulers[0].item_count, Some(raw));
            assert_eq!(raw as u8, case["byte"].as_u64().unwrap() as u8);
        }
        std::fs::write(output.join("scheduler-ordinary-parser-comparison.json"),serde_json::to_vec_pretty(&json!({"mappingCases":cases.len(),"mappingReaders":2,"countCases":13,"differences":0,"scope":"Actual public AvfxFile parser enabled low bits and preservation of raw StTm/TlNo/ItCn, with compiled word/index cast checked against both original mapping readers. No malformed cumulative container memory behavior, host or GPU."})).unwrap()).unwrap();
    }

    fn timeline_item(emitter: i32, end: i32) -> Vec<u8> {
        vec![
            u32_block("bEna", 1),
            i32_block("StTm", 0),
            i32_block("EdTm", end),
            i32_block("BdNo", -1),
            i32_block("EfNo", -1),
            i32_block("EmNo", emitter),
            i32_block("Plfm", 0),
            i32_block("ClNo", -1),
        ]
        .concat()
    }

    fn emitter_item() -> Vec<u8> {
        vec![
            u32_block("bEnb", 1),
            i32_block("TgtB", 0),
            i32_block("LoDr", 0),
            i32_block("CrTm", 1),
            i32_block("CrCn", 4),
            i32_block("CrPr", 100),
            i32_block("PICd", 0),
            i32_block("PICo", 0),
            u32_block("ICbS", 0),
            u32_block("ICbR", 0),
            u32_block("ICbP", 1),
            u32_block("ICbB", 0),
            block("ICSK", &0.0_f32.to_le_bytes()),
            u32_block("IPbV", 0),
            u32_block("IPbL", 0),
            u32_block("bOvr", 0),
            i32_block("OvrV", 60),
            i32_block("OvrR", 0),
            i32_block("PrLk", -1),
            i32_block("StFr", 0),
            u32_block("bStN", 0),
            block("BIAX", &0.0_f32.to_le_bytes()),
            block("BIAY", &0.0_f32.to_le_bytes()),
            block("BIAZ", &0.0_f32.to_le_bytes()),
            i32_block("GenD", 0),
            u32_block("bGD", 0),
        ]
        .concat()
    }

    /// 最小合成武器特效：1 scheduler（1 常驻 item + 12 trigger）→
    /// 1 timeline（1 item）→ 1 cone emitter → 1 quad particle（1 uvset +
    /// TC1 贴图）+ 1 binder + 1 模型（发射顶点 + 绘制网格共存）+ 1 tex 路径。
    fn synthetic_avfx() -> Vec<u8> {
        // 累积式写序：前面是空容器，最后一个容器含全部条目。
        let scheduler = container(
            "Schd",
            vec![
                u32_block("ItCn", 1),
                u32_block("TrCn", 12),
                container("Item", Vec::new()),
                container("Item", vec![scheduler_item(1, 0, 0)]),
                container("Trgr", Vec::new()),
                container(
                    "Trgr",
                    (0..12)
                        .map(|_| scheduler_item(0, 0, -1))
                        .collect::<Vec<_>>(),
                ),
            ],
        );

        let timeline = container(
            "TmLn",
            vec![
                u32_block("LpSt", 0),
                u32_block("LpEd", 120),
                i32_block("BnNo", 0),
                u32_block("TICn", 1),
                u32_block("CpCn", 0),
                container("Item", Vec::new()),
                container("Item", vec![timeline_item(0, 120)]),
            ],
        );

        let cone_data = container(
            "Data",
            vec![
                u32_block("ROT", 0),
                curve("AnX", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0),
                curve("InS", vec![key_bytes(0, 1, 0.0, 0.0, 0.1)], 0),
                curve("OuS", vec![key_bytes(0, 1, 0.0, 0.0, 0.6)], 0),
                curve("IjS", vec![key_bytes(0, 1, 0.0, 0.0, 0.02)], 0),
                curve("IjA", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
            ],
        );
        // Life：size>4 = 启用（嵌套 Val/ValR/Type），否则 4 字节 -1 哨兵。
        let life = container(
            "Life",
            vec![
                block("Val", &30.0_f32.to_le_bytes()),
                block("ValR", &0.0_f32.to_le_bytes()),
                u32_block("Type", 0),
            ],
        );
        let emitter = container(
            "Emit",
            vec![
                block("SdNm", &[0]),
                u32_block("SdNo", 0),
                u32_block("LpSt", 0),
                u32_block("LpEd", 120),
                i32_block("ClCn", 128),
                i32_block("EfNo", -1),
                u32_block("EVT", 1),
                u32_block("ROT", 2),
                u32_block("PrCn", 1),
                u32_block("EmCn", 0),
                life,
                curve("CrC", vec![key_bytes(0, 1, 0.0, 0.0, 1.0)], 0),
                curve(
                    "CrI",
                    vec![
                        key_bytes(0, 1, 0.0, 0.0, 15.0),
                        key_bytes(60, 1, 0.0, 0.0, 30.0),
                    ],
                    0,
                ),
                color_curve("Col", vec![key_bytes(0, 1, 1.0, 0.5, 0.25)], None, None),
                curve3(
                    "Pos",
                    None,
                    Some(vec![key_bytes(0, 1, 0.0, 0.0, 0.5)]),
                    None,
                ),
                curve3("Rot", None, None, None),
                curve3(
                    "Scl",
                    Some(vec![key_bytes(0, 1, 0.0, 0.0, 2.0)]),
                    None,
                    None,
                ),
                cone_data,
                container("ItPr", Vec::new()),
                container("ItPr", vec![emitter_item()]),
            ],
        );

        let uv_set = container(
            "UvSt",
            vec![
                u32_block("CUvT", 0),
                container(
                    "Scl",
                    vec![
                        u32_block("ACT", 0),
                        u32_block("ACTR", 0),
                        curve("X", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0),
                    ],
                ),
                container(
                    "Scr",
                    vec![
                        u32_block("ACT", 0),
                        u32_block("ACTR", 0),
                        curve("Y", vec![key_bytes(0, 1, 0.0, 0.0, 0.25)], 0),
                    ],
                ),
                curve("Rot", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
            ],
        );
        let tc1 = container(
            "TC1",
            vec![
                u32_block("bEna", 1),
                u32_block("bC2A", 0),
                i32_block("UvSN", 0),
                block("TLst", &[0u8]),
                u32_block("TCCT", 0),
                u32_block("TCAT", 0),
            ],
        );
        let quad_data = container("Data", vec![u32_block("SS", 1), u32_block("bMP", 0)]);
        let particle = container(
            "Ptcl",
            vec![
                u32_block("LpSt", 0),
                u32_block("LpEd", 30),
                u32_block("PrVT", 8),
                i32_block("RMT", 2),
                i32_block("RBDT", 6),
                u32_block("DsDt", 1),
                u32_block("DsDw", 0),
                block("Life", &(-1_i32).to_le_bytes()),
                curve3(
                    "Scl",
                    Some(vec![key_bytes(0, 1, 0.0, 0.0, 0.2)]),
                    None,
                    None,
                ),
                curve3("Rot", None, None, None),
                curve3("Pos", None, None, None),
                color_curve(
                    "Col",
                    vec![
                        key_bytes(0, 1, 1.0, 0.8, 0.4),
                        key_bytes(30, 1, 0.2, 0.1, 0.0),
                    ],
                    Some(vec![
                        key_bytes(0, 1, 0.0, 0.0, 0.0),
                        key_bytes(30, 1, 0.0, 0.0, 1.0),
                    ]),
                    Some(vec![key_bytes(0, 1, 0.0, 0.0, 2.0)]),
                ),
                tc1,
                uv_set,
                quad_data,
            ],
        );

        let binder = container(
            "Bind",
            vec![u32_block("BnVr", 0), container("Data", vec![])],
        );

        // 单 Modl 块同时携带发射顶点与绘制网格。
        let mut emit_vertex = Vec::new();
        emit_vertex.extend_from_slice(&0.1_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&0.2_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&0.3_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&0.0_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&1.0_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&0.0_f32.to_le_bytes());
        emit_vertex.extend_from_slice(&[255, 128, 64, 255]);
        let mut draw_vertex = Vec::new();
        for value in [0.5_f32, -0.5, 0.25, 1.0] {
            draw_vertex.extend_from_slice(&half::f16::from_f32(value).to_le_bytes());
        }
        draw_vertex.extend_from_slice(&[0u8; 8]);
        draw_vertex.extend_from_slice(&[255, 255, 255, 255]);
        for value in [0.25_f32, 0.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
            draw_vertex.extend_from_slice(&half::f16::from_f32(value).to_le_bytes());
        }
        let mut index_triple = Vec::new();
        for value in [0_i16, 1, 2] {
            index_triple.extend_from_slice(&value.to_le_bytes());
        }
        let model = container(
            "Modl",
            vec![
                block("VEmt", &emit_vertex),
                block("VNum", &[0, 0]),
                block("VDrw", &draw_vertex.repeat(3)),
                block("VIdx", &index_triple),
            ],
        );

        let root_children = vec![
            u32_block("Ver", 0x2011_0913),
            u32_block("ScCn", 1),
            u32_block("TlCn", 1),
            u32_block("EmCn", 1),
            u32_block("PrCn", 1),
            u32_block("EfCn", 0),
            u32_block("BdCn", 1),
            u32_block("TxCn", 1),
            u32_block("MdCn", 1),
            block("Tex", b"vfx/eff/vws_test.atex\0"),
            scheduler,
            timeline,
            emitter,
            particle,
            binder,
            model,
        ];
        container("AVFX", root_children)
    }

    #[test]
    fn integer_literals_use_declared_width_without_reinterpreting_floats() {
        for (payload, expected) in [
            (vec![0], 0),
            (vec![128], 128),
            (vec![255], 255),
            ((-2_i16).to_le_bytes().to_vec(), -2),
            (i16::MIN.to_le_bytes().to_vec(), i32::from(i16::MIN)),
            ((-1_i32).to_le_bytes().to_vec(), -1),
            (i32::MAX.to_le_bytes().to_vec(), i32::MAX),
        ] {
            let bytes = container("AVFX", vec![block("Val", &payload)]);
            let root = AvfxNodeView::parse_root(&bytes).unwrap();
            let fields = Fields::walk(root.payload());
            assert_eq!(root.scalar_i32("Val"), Some(expected), "{payload:?}");
            assert_eq!(fields.i32("Val"), Some(expected), "{payload:?}");
            if payload.len() < 4 {
                assert_eq!(root.f32("Val"), None);
                assert_eq!(fields.f32("Val"), None);
            }
        }
    }

    #[test]
    fn special_sampler_curve_diagnostics_detect_nonfinite_keys_without_rewriting_data() {
        let finite = AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }],
            ..Default::default()
        };
        assert!(!curve_has_nonfinite_key(&finite));
        let mut invalid = finite.clone();
        invalid.keys[0].z = f32::NAN;
        assert!(curve_has_nonfinite_key(&invalid));
        let color = AvfxColorCurve {
            rgb: Some(invalid.clone()),
            ..Default::default()
        };
        assert!(color_curve_has_nonfinite_key(&color));
        let axes = AvfxCurve3Axis {
            x: Some(invalid.clone()),
            ..Default::default()
        };
        assert!(curve3_has_nonfinite_key(&axes));
        assert!(invalid.keys[0].z.is_nan());
    }

    #[test]
    fn decal_sampler_scalar_diagnostics_preserve_nonfinite_values() {
        let decal = AvfxParticleData::Decal(AvfxParticleDataDecal {
            scaling_scale: f32::NAN,
            ..Default::default()
        });
        let AvfxParticleData::Decal(data) = &decal else {
            unreachable!()
        };
        assert!(!data.scaling_scale.is_finite());
        let ring = AvfxParticleData::DecalRing(AvfxParticleDataDecalRing {
            ring_fan: f32::INFINITY,
            ..Default::default()
        });
        let AvfxParticleData::DecalRing(data) = &ring else {
            unreachable!()
        };
        assert!(!data.ring_fan.is_finite());
    }

    #[test]
    fn emitter_curve_diagnostics_identify_nonfinite_shape_fields() {
        let mut curve = AvfxCurve::default();
        curve.keys.push(AvfxCurveKey {
            time: 0,
            interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
            x: 0.0,
            y: 0.0,
            z: f32::INFINITY,
        });
        assert_eq!(
            emitter_curve_fields([("Rad", &curve), ("IjS", &AvfxCurve::default())]),
            ["Rad"]
        );
    }

    #[test]
    fn byte_light_model_index_survives_parsing_and_mesh_sampling() {
        for index in [0_u8, 1, 127, 128, 254, 255] {
            let mut children = vec![
                container("TmLn", vec![block("Item", &timeline_item(0, -1))]),
                container(
                    "Emit",
                    vec![
                        curve("CrI", vec![key_bytes(0, 1, 0.0, 0.0, 1.0)], 0),
                        block("ItPr", &emitter_item()),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 13),
                        container("Data", vec![block("MNO", &[index])]),
                    ],
                ),
            ];
            for model_index in 0..=usize::from(index) {
                let geometry = if model_index == usize::from(index) {
                    vec![
                        block("VDrw", &[0; 36 * 3]),
                        block("VIdx", &[0, 0, 1, 0, 2, 0]),
                    ]
                } else {
                    Vec::new()
                };
                children.push(container("Modl", geometry));
            }
            let file = AvfxFile::parse(&container("AVFX", children)).unwrap();
            assert!(file.warnings.is_empty(), "{:?}", file.warnings);
            assert_eq!(
                file.particles[0].data,
                AvfxParticleData::LightModel {
                    model_index: i32::from(index),
                }
            );
            let runtime = crate::avfx_sim::VfxRuntime::new(&file);
            let mut meshes = Vec::new();
            runtime.sample_mesh(0.0, &mut meshes);
            assert!(!meshes.is_empty(), "MNO={index} produced no mesh");
            assert!(
                meshes
                    .iter()
                    .all(|mesh| mesh.model_index == usize::from(index))
            );
            let mut quads = Vec::new();
            runtime.sample(0.0, &mut quads);
            assert!(quads.is_empty());
        }
    }

    #[test]
    fn soft_particle_flag_is_preserved_and_diagnosed_by_particle_index() {
        let enabled = AvfxFile::parse(&container(
            "AVFX",
            vec![container("Ptcl", vec![u32_block("DsSp", 1)])],
        ))
        .unwrap();
        assert!(enabled.particles[0].is_soft_particle);
        assert_eq!(
            enabled.warnings,
            vec![
                "Ptcl[0].DsSp=1 requests soft-particle scene-depth fade; this particle type is not connected to scene depth/screen-copy sampling and is rendered without that fade"
                    .to_owned()
            ]
        );

        for (raw_type, supported) in [(8, true), (5, true), (6, true)] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![u32_block("PrVT", raw_type), u32_block("DsSp", 1)],
                )],
            ))
            .unwrap();
            let has_soft_warning = file
                .warnings
                .iter()
                .any(|warning| warning.contains("DsSp=1 requests soft-particle"));
            assert_eq!(
                has_soft_warning, !supported,
                "PrVT={raw_type} warnings: {:?}",
                file.warnings
            );
        }

        let disabled = AvfxFile::parse(&container(
            "AVFX",
            vec![container("Ptcl", vec![u32_block("DsSp", 0)])],
        ))
        .unwrap();
        assert!(!disabled.particles[0].is_soft_particle);
        assert!(disabled.warnings.is_empty(), "{:?}", disabled.warnings);
    }

    #[test]
    fn tc1_builtin_texture_sources_only_diagnose_missing_portrait_resource() {
        for (fields, expected_warning) in [
            (vec![u32_block("bUSC", 1)], false),
            (vec![u32_block("bUSC", 1), u32_block("bPFC", 1)], false),
            (vec![u32_block("bUOS", 1)], true),
        ] {
            let mut texture_fields = vec![u32_block("bEna", 1)];
            texture_fields.extend(fields);
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![u32_block("PrVT", 8), container("TC1", texture_fields)],
                )],
            ))
            .unwrap();
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("character-portrait (-5) source")),
                expected_warning,
                "unexpected built-in texture diagnostics: {:?}",
                file.warnings
            );
        }
    }

    #[test]
    fn uv_calculation_modes_are_preserved_and_unknown_values_are_diagnosed() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    container("UvSt", vec![u32_block("CUvT", 1)]),
                    container("UvSt", vec![u32_block("CUvT", 7)]),
                ],
            )],
        ))
        .unwrap();

        assert_eq!(file.particles[0].uv_sets[0].calculate_uv, 1);
        assert_eq!(file.particles[0].uv_sets[1].calculate_uv, 7);
        assert!(
            file.warnings
                .iter()
                .any(|warning| { warning.contains("Ptcl[0].UvSt[1].CUvT=7 is unknown") })
        );
        assert!(
            !file
                .warnings
                .iter()
                .any(|warning| { warning.contains("Ptcl[0].UvSt[0]") })
        );
    }

    #[test]
    fn polyline_unsupported_paths_are_diagnosed_without_rejecting_typed_data() {
        let parse = |fields: Vec<Vec<u8>>| {
            AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![u32_block("PrVT", 6), container("Data", fields)],
                )],
            ))
            .unwrap()
        };
        let continuous_spline = parse(vec![u32_block("bSpl", 1)]);
        assert!(matches!(
            &continuous_spline.particles[0].data,
            AvfxParticleData::Polyline(_)
        ));
        assert!(
            continuous_spline
                .warnings
                .iter()
                .any(|warning| warning.contains("Polyline configuration is unsupported"))
        );

        let bound = parse(vec![u32_block("BdWp", 1)]);
        assert!(
            bound
                .warnings
                .iter()
                .any(|warning| warning.contains("Polyline configuration is unsupported"))
        );

        for fields in [
            vec![i32_block("LnCT", -1)],
            vec![i32_block("PnC", 1)],
            vec![u32_block("bEdg", 0)],
        ] {
            let file = parse(fields);
            assert!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("Polyline configuration is unsupported"))
            );
        }

        let staged_spline = parse(vec![
            u32_block("LnCT", 1),
            u32_block("bSpl", 1),
            u32_block("bLcl", 1),
            u32_block("PnC", 2),
        ]);
        assert!(
            !staged_spline
                .warnings
                .iter()
                .any(|warning| warning.contains("Polyline configuration is unsupported"))
        );

        let static_non_edge = parse(vec![
            i32_block("LnCT", 0),
            i32_block("PnC", 6),
            u32_block("bEdg", 0),
            u32_block("bLcl", 1),
            i32_block("PnED", 1),
        ]);
        assert!(
            !static_non_edge
                .warnings
                .iter()
                .any(|warning| warning.contains("Polyline configuration is unsupported"))
        );
    }

    #[test]
    fn explicit_zero_revised_scale_does_not_become_default_identity() {
        assert_eq!(AvfxGlobalParameters::default().revised_scale, [1.0; 3]);
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                block("RvSx", &0.0_f32.to_le_bytes()),
                block("RvSy", &0.0_f32.to_le_bytes()),
                block("RvSz", &0.0_f32.to_le_bytes()),
            ],
        ))
        .unwrap();
        assert_eq!(file.global.revised_scale, [0.0; 3]);
        let document = crate::VfxDocumentTransform::new(
            file.global.revised_rotation,
            file.global.revised_scale,
            file.global.revised_position,
            crate::VfxBinderMatrix::IDENTITY,
            None,
        );
        assert_eq!(document.scale, [0.0; 3]);
        assert_eq!(document.auxiliary_matrix.basis, [[0.0; 3]; 3]);
    }

    #[test]
    fn revised_scale_and_color_default_to_identity_and_preserve_explicit_values() {
        let identity = AvfxFile::parse(&container("AVFX", Vec::new())).unwrap();
        assert_eq!(identity.global.revised_position, [0.0; 3]);
        assert_eq!(identity.global.revised_rotation, [0.0; 3]);
        assert_eq!(identity.global.revised_scale, [1.0; 3]);
        assert_eq!(identity.global.revised_color, [1.0; 3]);

        let explicit = AvfxFile::parse(&container(
            "AVFX",
            vec![
                block("RvPx", &2.0_f32.to_le_bytes()),
                block("RvRy", &0.25_f32.to_le_bytes()),
                block("RvSz", &3.0_f32.to_le_bytes()),
                block("RvR", &0.5_f32.to_le_bytes()),
            ],
        ))
        .unwrap();
        assert_eq!(explicit.global.revised_position, [2.0, 0.0, 0.0]);
        assert_eq!(explicit.global.revised_rotation, [0.0, 0.25, 0.0]);
        assert_eq!(explicit.global.revised_scale, [1.0, 1.0, 3.0]);
        assert_eq!(explicit.global.revised_color, [0.5, 1.0, 1.0]);
    }

    #[test]
    fn draw_layer_defaults_to_base_and_preserves_authored_values() {
        let absent = AvfxFile::parse(&container("AVFX", vec![])).unwrap();
        assert_eq!(absent.global.draw_layer, 2);
        for value in [0, 2, 10, 11, 31, 32, 257, -1] {
            let explicit =
                AvfxFile::parse(&container("AVFX", vec![i32_block("DwLy", value)])).unwrap();
            assert_eq!(explicit.global.draw_layer, value);
        }
    }

    #[test]
    fn aura_priority_preserves_root_apri_independently_of_dpri() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![i32_block("APri", 0x102), i32_block("DPri", -3)],
        ))
        .unwrap();
        assert_eq!(file.global.a_pri, 0x102);
        assert_eq!(file.global.d_pri, -3);
        assert_eq!(file.global.a_pri as u8, 2);
    }

    #[test]
    fn unsupported_scene_parameters_are_diagnosed_without_dropping_fields() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                u32_block("AFXe", 1),
                block("AFXi", &1.0_f32.to_le_bytes()),
                block("AFXo", &2.0_f32.to_le_bytes()),
                u32_block("bGFE", 1),
                block("GFIM", &0.25_f32.to_le_bytes()),
                u32_block("bCmS", 1),
                u32_block("bFEL", 1),
                u32_block("bLTS", 1),
                u32_block("DLST", 2),
                block("NCB", &1.0_f32.to_le_bytes()),
                block("NCE", &2.0_f32.to_le_bytes()),
                container(
                    "Ptcl",
                    vec![
                        u32_block("bAFg", 1),
                        u32_block("bNea", 1),
                        block("NeSt", &3.0_f32.to_le_bytes()),
                        block("NeEd", &4.0_f32.to_le_bytes()),
                        u32_block("bFar", 1),
                        block("FaSt", &5.0_f32.to_le_bytes()),
                        block("FaEd", &6.0_f32.to_le_bytes()),
                        i32_block("DOTy", 2),
                        block("DpOf", &0.125_f32.to_le_bytes()),
                        u32_block("bATM", 1),
                        i32_block("EnvT", 1),
                        i32_block("DirT", 2),
                    ],
                ),
            ],
        ))
        .unwrap();
        assert!(file.particles[0].is_apply_fog);
        assert!(file.particles[0].clip_near_enable);
        assert!(file.particles[0].clip_far_enable);
        assert_eq!(file.particles[0].depth_offset_type, 2);
        for needle in [
            "AvfxMain.AFXe=1 distance fade",
            "AvfxMain.bGFE=1 global fog",
            "AvfxMain.NCB/NCE",
            "AvfxMain.bCmS=1",
            "AvfxMain.bFEL=1",
            "AvfxMain.bLTS=1",
            "AvfxMain.DLST/PL1S/PL2S",
            "Ptcl[0].bAFg=1",
            "Ptcl[0].bNea=1",
            "Ptcl[0].bFar=1",
            "Ptcl[0].DOTy/DpOf",
            "Ptcl[0].bATM=1",
            "Ptcl[0].EnvT/DirT",
        ] {
            assert!(
                file.warnings.iter().any(|warning| warning.contains(needle)),
                "missing {needle:?} in {:?}",
                file.warnings
            );
        }
    }

    #[test]
    fn root_ags_defaults_false_and_uses_last_full_dword_write() {
        assert!(
            !AvfxFile::parse(&container("AVFX", vec![]))
                .unwrap()
                .global
                .ags_enabled
        );
        for (words, expected) in [
            (vec![0, 0x8000_0000], true),
            (vec![0x8000_0000, 0], false),
            (vec![0, 0x100, 0x10000], true),
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                words
                    .into_iter()
                    .map(|word| u32_block("bAGS", word))
                    .collect(),
            ))
            .unwrap();
            assert_eq!(file.global.ags_enabled, expected);
        }
    }

    #[test]
    fn root_ags_short_records_consume_alignment_slot_and_last_write() {
        for width in 1_u32..=4 {
            for word in [0_u32, 1, 0x100, 0x10000, 0x80000000] {
                let mut leaf = u32_block("bAGS", word);
                leaf[4..8].copy_from_slice(&width.to_le_bytes());
                let file =
                    AvfxFile::parse(&container("AVFX", vec![leaf, block("bAGS", &[0])])).unwrap();
                assert!(!file.global.ags_enabled, "last byte write clears the slot");
                let mut leaf = u32_block("bAGS", word);
                leaf[4..8].copy_from_slice(&width.to_le_bytes());
                let file =
                    AvfxFile::parse(&container("AVFX", vec![block("bAGS", &[0]), leaf])).unwrap();
                assert_eq!(
                    file.global.ags_enabled,
                    word != 0,
                    "width {width}, word {word:#x}"
                );
            }
        }
    }

    #[test]
    fn root_ags_diagnostic_identifies_auxiliary_transform_without_lighting() {
        let file = AvfxFile::parse(&container("AVFX", vec![u32_block("bAGS", 1)])).unwrap();
        assert!(file.global.ags_enabled);
        assert!(!file.global.lts_enabled);
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("AvfxMain.bAGS=1 root revision")
                    && warning.contains("auxiliary-matrix"))
        );
        assert!(
            !file
                .warnings
                .iter()
                .any(|warning| warning.contains("lighting"))
        );
    }

    #[test]
    fn depth_offset_diagnostics_follow_supported_particle_types_and_modes() {
        for (particle_type, offset_type, warned) in [
            (8, 0, false),
            (8, 1, false),
            (5, 1, false),
            (1, 1, false),
            (8, 2, true),
            (12, 0, true),
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", particle_type),
                        i32_block("DOTy", offset_type),
                        block("DpOf", &0.125_f32.to_le_bytes()),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(file.particles[0].depth_offset_type, offset_type);
            assert_eq!(file.particles[0].depth_offset, 0.125);
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("DOTy/DpOf")),
                warned,
                "PrVT={particle_type}, DOTy={offset_type}: {:?}",
                file.warnings
            );
        }
    }

    #[test]
    fn nonfinite_soft_particle_range_is_diagnosed_without_rewriting_value() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![block("SPFR", &f32::NAN.to_le_bytes())],
        ))
        .unwrap();
        assert!(file.global.soft_particle_fade_range.is_nan());
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("AvfxMain.SPFR=NaN is nonfinite"))
        );
        assert_eq!(file.global.effective_soft_particle_fade_range(), 1.0);
    }

    #[test]
    fn finite_negative_soft_particle_range_is_preserved_for_upload() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![block("SPFR", &(-0.8_f32).to_le_bytes())],
        ))
        .unwrap();
        assert_eq!(file.global.soft_particle_fade_range, -0.8);
        assert_eq!(file.global.effective_soft_particle_fade_range(), -0.8);
        assert!(!file.warnings.iter().any(|warning| warning.contains("SPFR")));
    }

    #[test]
    fn resource_references_report_all_slots_and_use_local_clip_tables() {
        let mut particle = vec![
            u32_block("PrVT", 5),
            container("Data", vec![block("MdNo", &[255, 0, 1, 254])]),
            u32_block("bSCt", 1),
            container("Smpl", vec![i32_block("IJMN", 1), i32_block("VBMN", 2)]),
        ];
        let mut expected = vec![
            ("TmLn[0].Item[0].ClNo".to_owned(), 0, "TmLn[0].Clip", 0),
            ("Emit[0].Data.MdNo".to_owned(), 1, "Modl", 1),
            ("Ptcl[0].Data.MdNo[2]".to_owned(), 1, "Modl", 1),
            ("Ptcl[0].Smpl.IJMN".to_owned(), 1, "Modl", 1),
            ("Ptcl[0].Smpl.VBMN".to_owned(), 2, "Modl", 1),
            ("Ptcl[0].TC1.TLst[2]".to_owned(), 1, "Tex", 1),
            ("Ptcl[1].Data.MNO".to_owned(), 255, "Modl", 1),
        ];
        for layer in ["TC1", "TC2", "TC3", "TC4", "TN", "TD", "TR", "TP"] {
            let mut fields = vec![u32_block("bEna", 1), i32_block("TxNo", 255)];
            if layer != "TC1" {
                expected.push((format!("Ptcl[0].{layer}.TxNo"), 255, "Tex", 1));
            }
            if !matches!(layer, "TR" | "TP") {
                fields.push(i32_block("UvSN", 4));
                expected.push((format!("Ptcl[0].{layer}.UvSN"), 4, "Ptcl[0].UV slots", 4));
            }
            if layer == "TC1" {
                fields.push(block("TLst", &[255, 0, 1]));
                fields.push(curve("TxN", vec![key_bytes(0, 1, 0.0, 0.0, 2.0)], 0));
            }
            particle.push(container(layer, fields));
        }
        let bytes = container(
            "AVFX",
            vec![
                container(
                    "TmLn",
                    vec![container(
                        "Item",
                        vec![u32_block("bEna", 1), i32_block("ClNo", 0)],
                    )],
                ),
                container("TmLn", vec![block("Clip", &[0; 164])]),
                container(
                    "Emit",
                    vec![
                        u32_block("EVT", 5),
                        container("Data", vec![i32_block("MdNo", 1)]),
                    ],
                ),
                container("Ptcl", particle),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 13),
                        container("Data", vec![block("MNO", &[255])]),
                        container("UvSt", Vec::new()),
                    ],
                ),
                block("Tex", b"test.atex\0"),
                container("Modl", Vec::new()),
            ],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        let mut expected: Vec<_> = expected
            .into_iter()
            .map(|(path, index, target, count)| {
                format!(
                    "AVFX reference {path}: index {index} outside {target} table ({count} entries)"
                )
            })
            .collect();
        let mut actual = file.warnings.clone();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
        assert_eq!(
            file.particles[0]
                .texture_color1
                .as_ref()
                .unwrap()
                .texture_list,
            [255, 0, 1]
        );
        assert!(
            matches!(&file.particles[0].data, AvfxParticleData::Model { model_indexes, .. } if model_indexes == &[255, 0, 1, 254])
        );
    }

    #[test]
    fn uv_set_reference_diagnostics_use_client_low_three_bits() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 5),
                    container("TC1", vec![u32_block("bEna", 1), i32_block("UvSN", 9)]),
                    container("TC2", vec![u32_block("bEna", 1), i32_block("UvSN", 12)]),
                    container("TN", vec![u32_block("bEna", 1), i32_block("UvSN", 8)]),
                    container("TD", vec![u32_block("bEna", 1), i32_block("UvSN", 15)]),
                ],
            )],
        ))
        .unwrap();
        let particle = &file.particles[0];
        assert_eq!(particle.texture_color1.as_ref().unwrap().uv_set_index, 9);
        assert_eq!(particle.texture_color2.as_ref().unwrap().uv_set_index, 12);
        assert_eq!(particle.texture_normal.as_ref().unwrap().uv_set_index, 8);
        assert_eq!(
            particle.texture_distortion.as_ref().unwrap().uv_set_index,
            15
        );
        assert_eq!(
            file.warnings,
            [
                "AVFX reference Ptcl[0].TC2.UvSN: index 4 outside Ptcl[0].UV slots table (4 entries)",
                "AVFX reference Ptcl[0].TD.UvSN: index 7 outside Ptcl[0].UV slots table (4 entries)",
            ]
        );
    }

    #[test]
    fn resource_reference_validation_accepts_forward_targets_and_skips_disabled_fields() {
        let mut active = vec![
            u32_block("PrVT", 5),
            container("Data", vec![block("MdNo", &[255, 0])]),
            container("UvSt", Vec::new()),
            u32_block("bSCt", 1),
            container("Smpl", vec![i32_block("IJMN", 0), i32_block("VBMN", -1)]),
        ];
        let mut disabled = vec![
            u32_block("bSCt", 0),
            container(
                "Smpl",
                vec![i32_block("IJMN", i32::MAX), i32_block("VBMN", i32::MAX)],
            ),
        ];
        for layer in ["TC1", "TC2", "TC3", "TC4", "TN", "TD", "TR", "TP"] {
            let mut fields = vec![
                u32_block("bEna", 1),
                i32_block("TxNo", 0),
                i32_block("UvSN", 3),
            ];
            if layer == "TC1" {
                fields.push(block("TLst", &[255, 0]));
            }
            active.push(container(layer, fields));
            disabled.push(container(
                layer,
                vec![
                    u32_block("bEna", 0),
                    i32_block("TxNo", i32::MAX),
                    i32_block("UvSN", i32::MAX),
                    block("TLst", &[254]),
                ],
            ));
        }
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container("Ptcl", active),
                container("Ptcl", disabled),
                container(
                    "Emit",
                    vec![
                        u32_block("EVT", 5),
                        container("Data", vec![i32_block("MdNo", 0)]),
                    ],
                ),
                container(
                    "TmLn",
                    vec![
                        container(
                            "Item",
                            vec![
                                u32_block("bEna", 1),
                                i32_block("ClNo", 0),
                                u32_block("bEna", 0),
                                i32_block("ClNo", i32::MAX),
                            ],
                        ),
                        block("Clip", &[0; 164]),
                    ],
                ),
                block("Tex", b"test.atex\0"),
                container("Modl", Vec::new()),
            ],
        ))
        .unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
    }

    #[test]
    fn powder_model_reference_diagnostics_use_signed_low_byte() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 1),
                        u32_block("bSCt", 1),
                        container("Smpl", vec![i32_block("IJMN", 256), i32_block("VBMN", 257)]),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 1),
                        u32_block("bSCt", 1),
                        container("Smpl", vec![i32_block("IJMN", 255), i32_block("VBMN", 258)]),
                    ],
                ),
                container("Modl", Vec::new()),
                container("Modl", Vec::new()),
            ],
        ))
        .unwrap();
        let first = file.particles[0].simple.as_ref().unwrap();
        let second = file.particles[1].simple.as_ref().unwrap();
        assert_eq!(first.injection_model_index, 256);
        assert_eq!(second.injection_model_index, 255);
        assert_eq!(
            file.warnings,
            ["AVFX reference Ptcl[1].Smpl.VBMN: index 2 outside Modl table (2 entries)"]
        );
    }

    #[test]
    fn texture_and_model_byte_lists_only_diagnose_runtime_references() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 5),
                        container("Data", vec![block("MdNo", &[0, 127, 128, 254, 255])]),
                        container(
                            "TC1",
                            vec![
                                u32_block("bEna", 1),
                                i32_block("TxNo", 99),
                                block("TLst", &[0, 127, 128, 253, 255]),
                                curve("TxN", vec![key_bytes(0, 1, 0.0, 0.0, 1.0)], 0),
                            ],
                        ),
                        container(
                            "TC2",
                            vec![
                                u32_block("bEna", 1),
                                i32_block("TxNo", 1),
                                block("TLst", &[99]),
                            ],
                        ),
                    ],
                ),
                block("Tex", b"test.atex\0"),
                container("Modl", Vec::new()),
            ],
        ))
        .unwrap();
        assert_eq!(
            file.warnings,
            [
                "AVFX reference Ptcl[0].TC1.TLst[1]: index 127 outside Tex table (1 entries)",
                "AVFX reference Ptcl[0].TC2.TxNo: index 1 outside Tex table (1 entries)",
                "AVFX reference Ptcl[0].Data.MdNo[1]: index 127 outside Modl table (1 entries)",
            ]
        );
        assert_eq!(
            file.particles[0]
                .texture_color1
                .as_ref()
                .unwrap()
                .texture_list,
            [0, 127, 128, 253, 255]
        );
        let mut mask = file.particles[0].texture_color1.as_ref().unwrap().clone();
        mask.mask_texture_index = 128;
        assert!(!mask.is_shape_mask());
    }

    #[test]
    fn tc1_reference_diagnostics_only_check_addressable_list_entries() {
        let parse = |list: Vec<u8>, extra: Vec<Vec<u8>>| {
            let mut tc1 = vec![u32_block("bEna", 1), block("TLst", &list)];
            tc1.extend(extra);
            AvfxFile::parse(&container(
                "AVFX",
                vec![
                    container("Ptcl", vec![container("TC1", tc1)]),
                    block("Tex", b"test.atex\0"),
                ],
            ))
            .unwrap()
        };
        let dynamic = || curve("TxN", vec![key_bytes(0, 1, 0.0, 0.0, 1.0)], 0);
        let refs = |file: &AvfxFile| {
            file.warnings
                .iter()
                .filter(|warning| warning.contains("AVFX reference Ptcl[0].TC1.TLst"))
                .cloned()
                .collect::<Vec<_>>()
        };

        assert!(refs(&parse(vec![0, 4], vec![])).is_empty());
        assert_eq!(
            refs(&parse(vec![0, 4], vec![dynamic()])),
            ["AVFX reference Ptcl[0].TC1.TLst[1]: index 4 outside Tex table (1 entries)"]
        );
        for source_flag in ["bUSC", "bUOS"] {
            assert!(
                refs(&parse(vec![4], vec![u32_block(source_flag, 1), dynamic()])).is_empty(),
                "{source_flag} overrides TLst"
            );
        }

        let mut wrapped = vec![0; 258];
        wrapped[1] = 4;
        wrapped[257] = 4;
        assert!(refs(&parse(wrapped[..256].to_vec(), vec![dynamic()])).is_empty());
        assert_eq!(
            refs(&parse(wrapped, vec![dynamic()])),
            ["AVFX reference Ptcl[0].TC1.TLst[1]: index 4 outside Tex table (1 entries)"]
        );
    }

    #[test]
    fn reflection_screen_source_does_not_diagnose_unused_file_index() {
        let reflection = |enabled, screen_copy| {
            container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 5),
                    container(
                        "TR",
                        vec![
                            u32_block("bEna", enabled),
                            u32_block("bUSC", screen_copy),
                            i32_block("TxNo", 7),
                        ],
                    ),
                ],
            )
        };
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                reflection(1, 1),
                reflection(1, 0),
                reflection(0, 0),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 8),
                        container("TR", vec![u32_block("bEna", 1), i32_block("TxNo", 7)]),
                        container("TN", vec![u32_block("bEna", 1), i32_block("TxNo", 7)]),
                    ],
                ),
                block("Tex", b"test.atex\0"),
            ],
        ))
        .unwrap();
        let references = file
            .warnings
            .iter()
            .filter(|warning| warning.contains(".TR.TxNo"))
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            references,
            ["AVFX reference Ptcl[1].TR.TxNo: index 7 outside Tex table (1 entries)"]
        );
        assert!(
            !file
                .warnings
                .iter()
                .any(|warning| warning.contains("Ptcl[3].TN.TxNo"))
        );
        assert_eq!(
            file.particles[0]
                .texture_reflection
                .as_ref()
                .unwrap()
                .texture_index,
            7
        );
    }

    #[test]
    fn timeline_and_uv_counts_are_checked_against_parsed_tables() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "TmLn",
                    vec![
                        i32_block("TICn", 2),
                        i32_block("CpCn", -1),
                        block("Item", &timeline_item(-1, 30)),
                        block("Clip", &[0; 164]),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![i32_block("UvSN", i32::MAX), container("UvSt", Vec::new())],
                ),
            ],
        ))
        .unwrap();
        assert_eq!(
            file.warnings,
            [
                "TmLn.TICn: declared 2, parsed 1",
                "TmLn.CpCn: declared -1, parsed 1",
                "Ptcl.UvSN: declared 2147483647, parsed 1",
            ]
        );
    }

    #[test]
    fn timeline_clip_control_switches_follow_signed_comparisons_and_wrapping_selectors() {
        assert_eq!(
            AvfxTimelineClipTarget::from_signed_selector(i32::MIN),
            Some(AvfxTimelineClipTarget::SchedulerTrigger { index: i32::MAX })
        );
        assert_eq!(AvfxTimelineClipTarget::from_signed_selector(0), None);
        let trigger = AvfxTimelineClipTarget::from_signed_selector(-10).unwrap();
        assert!(trigger.matches_identity(-1, 9));
        assert!(
            !trigger.matches_identity(9, 4),
            "mapped Timeline4 is not trigger slot9"
        );
        for value in [-2.0, -f32::MIN_POSITIVE, -0.0, 0.0, 2.0, f32::NAN] {
            let mut payload = Vec::new();
            payload.extend_from_slice(&0x4b494c4cu32.to_le_bytes());
            for int in [7i32, -1, 0, 1] {
                payload.extend_from_slice(&int.to_le_bytes());
            }
            for float in [value, value, value, f32::NAN] {
                payload.extend_from_slice(&float.to_le_bytes());
            }
            payload.resize(164, 0);
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container("TmLn", vec![block("Clip", &payload)])],
            ))
            .unwrap();
            assert_eq!(
                file.timelines[0].clips[0].parameters,
                Some(AvfxTimelineClipParameters::Kill {
                    fade_out_duration: 7,
                    targets: [
                        Some(AvfxTimelineClipTarget::SchedulerTrigger { index: 0 }),
                        None,
                        Some(AvfxTimelineClipTarget::SchedulerItem { index: 0 })
                    ],
                    fade_mode: if value < 0.0 { 2 } else { 3 },
                    fade_flag: value < 0.0,
                    end_document_on_retire: value < 0.0,
                })
            );
            assert_eq!(file.timelines[0].clips[0].raw_payload, payload);
        }
    }

    #[test]
    fn timeline_clips_decode_fixed_fields_and_preserve_opaque_bytes() {
        let clip_payload = |raw_type: u32| {
            let mut payload = Vec::new();
            payload.extend_from_slice(&raw_type.to_le_bytes());
            for value in [-3_i32, 5, 0, 7] {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            for value in [1.25_f32, -2.5, 0.0, 3.75] {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            payload.extend_from_slice(&[0xa5; 128]);
            payload
        };
        let tags = [
            (0x4B49_4C4C, Some(AvfxTimelineClipType::Kill)),
            (0x5245_5354, Some(AvfxTimelineClipType::Reset)),
            (0x454E_4420, Some(AvfxTimelineClipType::End)),
            (0x4641_4449, Some(AvfxTimelineClipType::FadeIn)),
            (0x554C_4C50, Some(AvfxTimelineClipType::UnlockLoopPoint)),
            (0x5452_4720, Some(AvfxTimelineClipType::Trigger)),
            (0x5254_5247, Some(AvfxTimelineClipType::RandomTrigger)),
            (0xDEAD_BEEF, None),
        ];
        let mut fields = vec![u32_block("CpCn", tags.len() as u32)];
        fields.extend(
            tags.iter()
                .map(|(raw_type, _)| block("Clip", &clip_payload(*raw_type))),
        );
        let file = AvfxFile::parse(&container("AVFX", vec![container("TmLn", fields)])).unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.timelines[0].clip_count, tags.len());
        for (clip, &(raw_type, clip_type)) in file.timelines[0].clips.iter().zip(&tags) {
            assert_eq!(clip.raw_type, Some(raw_type));
            assert_eq!(clip.clip_type, clip_type);
            assert_eq!(clip.raw_ints, Some([-3, 5, 0, 7]));
            assert_eq!(clip.raw_floats, Some([1.25, -2.5, 0.0, 3.75]));
            assert_eq!(clip.raw_payload, clip_payload(raw_type));
            let expected = match clip_type {
                Some(AvfxTimelineClipType::Kill) => Some(AvfxTimelineClipParameters::Kill {
                    fade_out_duration: -3,
                    targets: [
                        Some(AvfxTimelineClipTarget::SchedulerItem { index: 4 }),
                        None,
                        Some(AvfxTimelineClipTarget::SchedulerItem { index: 6 }),
                    ],
                    fade_mode: 2,
                    fade_flag: false,
                    end_document_on_retire: false,
                }),
                Some(AvfxTimelineClipType::Reset) => Some(AvfxTimelineClipParameters::Reset {
                    document: true,
                    scheduler_items: [4, -1, 6],
                }),
                Some(AvfxTimelineClipType::End) => Some(AvfxTimelineClipParameters::End),
                Some(AvfxTimelineClipType::FadeIn) => Some(AvfxTimelineClipParameters::FadeIn {
                    duration: -3,
                    targets: [
                        Some(AvfxTimelineClipTarget::SchedulerItem { index: 4 }),
                        None,
                        Some(AvfxTimelineClipTarget::SchedulerItem { index: 6 }),
                    ],
                }),
                Some(AvfxTimelineClipType::UnlockLoopPoint) => {
                    Some(AvfxTimelineClipParameters::UnlockLoopPoint {
                        all_scheduler: true,
                        targets: [
                            Some(AvfxTimelineClipTarget::SchedulerItem { index: 4 }),
                            None,
                            Some(AvfxTimelineClipTarget::SchedulerItem { index: 6 }),
                        ],
                    })
                }
                Some(AvfxTimelineClipType::Trigger) => {
                    Some(AvfxTimelineClipParameters::Trigger { trigger: -3 })
                }
                Some(AvfxTimelineClipType::RandomTrigger) => {
                    Some(AvfxTimelineClipParameters::RandomTrigger {
                        minimum_trigger: -3,
                        maximum_trigger: 5,
                    })
                }
                _ => None,
            };
            assert_eq!(clip.parameters, expected);
        }
        let serialized = serde_json::to_value(&file).unwrap();
        assert_eq!(serialized["timelines"][0]["clipCount"], tags.len());
        assert_eq!(
            serialized["timelines"][0]["clips"]
                .as_array()
                .unwrap()
                .len(),
            tags.len()
        );
        assert_eq!(
            serialized["timelines"][0]["clips"][5]["parameters"]["trigger"]["trigger"],
            -3
        );
        assert_eq!(
            serialized["timelines"][0]["clips"][6]["parameters"]["randomTrigger"]["minimumTrigger"],
            -3
        );

        let mut kill_payload = clip_payload(0x4B49_4C4C);
        kill_payload[16..20].copy_from_slice(&1_i32.to_le_bytes());
        kill_payload[20..24].copy_from_slice(&(-1.0_f32).to_le_bytes());
        kill_payload[24..28].copy_from_slice(&(-1.0_f32).to_le_bytes());
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("TmLn", vec![block("Clip", &kill_payload)])],
        ))
        .unwrap();
        assert_eq!(
            file.timelines[0].clips[0].parameters,
            Some(AvfxTimelineClipParameters::Kill {
                fade_out_duration: -3,
                targets: [
                    Some(AvfxTimelineClipTarget::SchedulerItem { index: 4 }),
                    None,
                    Some(AvfxTimelineClipTarget::SchedulerItem { index: 0 })
                ],
                fade_mode: 2,
                fade_flag: false,
                end_document_on_retire: true,
            })
        );

        let mut extended = clip_payload(0xDEAD_BEEF);
        extended.extend_from_slice(&[1, 2, 3]);
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("TmLn", vec![block("Clip", &extended)])],
        ))
        .unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.timelines[0].clips[0].raw_payload, extended);

        let mut truncated = clip_payload(0x4B49_4C4C);
        truncated.truncate(28);
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("TmLn", vec![block("Clip", &truncated)])],
        ))
        .unwrap();
        let clip = &file.timelines[0].clips[0];
        assert_eq!(clip.clip_type, Some(AvfxTimelineClipType::Kill));
        assert_eq!(clip.parameters, None);
        assert_eq!(clip.raw_ints, Some([-3, 5, 0, 7]));
        assert_eq!(clip.raw_floats, None);
        assert_eq!(clip.raw_payload, truncated);
        assert_eq!(file.warnings.len(), 1);
        assert_eq!(
            file.warnings[0],
            "TmLn[0].Clip[0]: expected 164 bytes, found 28"
        );

        let mut truncated_trigger = clip_payload(0x5254_5247);
        truncated_trigger.truncate(20);
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("TmLn", vec![block("Clip", &truncated_trigger)])],
        ))
        .unwrap();
        assert_eq!(
            file.timelines[0].clips[0].parameters,
            Some(AvfxTimelineClipParameters::RandomTrigger {
                minimum_trigger: -3,
                maximum_trigger: 5,
            })
        );
        assert_eq!(
            file.warnings,
            ["TmLn[0].Clip[0]: expected 164 bytes, found 20"]
        );
    }

    #[test]
    fn coordinate_inheritance_diagnostics_preserve_raw_modes_and_options() {
        for shape in [3, 4] {
            for kind in ["ItPr", "ItEm"] {
                for mode in 0..10 {
                    let file = AvfxFile::parse(&container(
                        "AVFX",
                        vec![container(
                            "Emit",
                            vec![
                                u32_block("EVT", shape),
                                container("Data", vec![i32_block("GeMT", 2)]),
                                container(
                                    kind,
                                    vec![
                                        u32_block("bEnb", 1),
                                        i32_block("PICd", mode),
                                        u32_block("ICbP", 1),
                                    ],
                                ),
                            ],
                        )],
                    ))
                    .unwrap();
                    let item = match kind {
                        "ItPr" => file.emitters[0].particle_items[0],
                        _ => file.emitters[0].emitter_items[0],
                    };
                    assert_eq!(item.parent_influence_coord, mode);
                    assert!(item.influence_coord_pos);
                    assert!(!item.influence_coord_rot);
                    assert_eq!(
                        file.warnings
                            .iter()
                            .any(|warning| warning.contains("ground height queries")),
                        matches!(mode, 4..=7 | 9)
                    );
                    assert!(!file.warnings.iter().any(|warning| {
                        warning.starts_with(&format!("emitter[0].{kind}[0]: ICbP shape binding"))
                    }));
                }
            }
        }
    }

    #[test]
    fn coordinate_inheritance_diagnostics_report_unimplemented_motion_options() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Emit",
                vec![container(
                    "ItPr",
                    vec![
                        u32_block("bEnb", 1),
                        i32_block("PICd", 1),
                        u32_block("IPbV", 1),
                        u32_block("ICSK", 0x3f00_0000),
                    ],
                )],
            )],
        ))
        .unwrap();
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("ICSK=0.5"))
        );
        assert!(!file.warnings.iter().any(|warning| warning.contains("IPbV")));
    }

    #[test]
    fn validates_node_references_after_reading_forward_targets() {
        let file = AvfxFile::parse(&synthetic_avfx()).unwrap();
        assert!(
            file.warnings
                .iter()
                .all(|warning| !warning.starts_with("AVFX reference")),
            "{:?}",
            file.warnings
        );
    }

    #[test]
    fn invalid_node_references_report_their_owner_without_reindexing() {
        let scheduler_item = |index| [u32_block("bEna", 1), u32_block("TlNo", index)].concat();
        let mut triggers = scheduler_item(u32::MAX).repeat(11);
        triggers.extend(scheduler_item(4));
        let bytes = container(
            "AVFX",
            vec![
                container(
                    "Schd",
                    vec![block("Item", &scheduler_item(1)), block("Trgr", &triggers)],
                ),
                container(
                    "TmLn",
                    vec![
                        u32_block("BnNo", 2),
                        container(
                            "Item",
                            vec![
                                u32_block("bEna", 1),
                                u32_block("EmNo", 1),
                                u32_block("BdNo", 3),
                                u32_block("EfNo", i32::MAX as u32),
                            ],
                        ),
                    ],
                ),
                container(
                    "Emit",
                    vec![
                        u32_block("EfNo", 5),
                        container("ItPr", vec![u32_block("bEnb", 1), u32_block("TgtB", 7)]),
                        container(
                            "ItEm",
                            vec![
                                u32_block("bEnb", 1),
                                u32_block("TgtB", 7),
                                u32_block("bEnb", 1),
                                u32_block("TgtB", 1),
                            ],
                        ),
                    ],
                ),
            ],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        let references: Vec<_> = file
            .warnings
            .iter()
            .filter(|warning| warning.starts_with("AVFX reference"))
            .collect();
        assert_eq!(references.len(), 8, "{references:?}");
        for (path, index, target, count) in [
            ("Schd[0].Item[0].TlNo", 1, "TmLn", 1),
            ("Schd[0].Trgr[11].TlNo", 4, "TmLn", 1),
            ("TmLn[0].BnNo", 2, "Bind", 0),
            ("TmLn[0].Item[0].EmNo", 1, "Emit", 1),
            ("TmLn[0].Item[0].BdNo", 3, "Bind", 0),
            ("Emit[0].EfNo", 5, "Efct", 0),
            ("Emit[0].ItPr[0].TgtB", 7, "Ptcl", 0),
            ("Emit[0].ItEm[0].TgtB", 1, "Emit", 1),
        ] {
            assert!(file.warnings.contains(&format!(
                "AVFX reference {path}: index {index} outside {target} table ({count} entries)"
            )));
        }
        assert_eq!(file.schedulers[0].items[0].timeline_index, 1);
        assert_eq!(file.timelines[0].items[0].emitter_index, 1);
        assert_eq!(file.timelines[0].items[0].effector_index, i32::MAX);
        assert_eq!(file.emitters[0].particle_items[0].target_index, 7);
        let mut quads = Vec::new();
        crate::avfx_sim::VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());
    }

    #[test]
    fn unassigned_and_disabled_node_references_do_not_warn() {
        let bytes = container(
            "AVFX",
            vec![
                container(
                    "Schd",
                    vec![container(
                        "Item",
                        vec![
                            u32_block("bEna", 1),
                            u32_block("TlNo", u32::MAX),
                            u32_block("bEna", 0),
                            u32_block("TlNo", 42),
                        ],
                    )],
                ),
                container(
                    "TmLn",
                    vec![container(
                        "Item",
                        vec![
                            u32_block("bEna", 1),
                            u32_block("BdNo", (-2_i32) as u32),
                            u32_block("bEna", 0),
                            u32_block("EmNo", 42),
                            u32_block("BdNo", 42),
                            u32_block("EfNo", 42),
                        ],
                    )],
                ),
                container(
                    "Emit",
                    vec![container(
                        "ItPr",
                        vec![
                            u32_block("bEnb", 1),
                            u32_block("TgtB", u32::MAX),
                            u32_block("bEnb", 0),
                            u32_block("TgtB", 42),
                        ],
                    )],
                ),
            ],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.timelines[0].items[0].binder_index, -2);
    }

    #[test]
    fn timeline_binder_reference_diagnostics_use_signed_low_byte() {
        for (raw_timeline, raw_item, expected) in [
            (255, 254, Vec::<String>::new()),
            (
                256,
                257,
                vec![
                    "AVFX reference TmLn[0].BnNo: index 0 outside Bind table (0 entries)"
                        .to_owned(),
                    "AVFX reference TmLn[0].Item[0].BdNo: index 1 outside Bind table (0 entries)"
                        .to_owned(),
                ],
            ),
        ] {
            let bytes = container(
                "AVFX",
                vec![container(
                    "TmLn",
                    vec![
                        u32_block("BnNo", raw_timeline),
                        container(
                            "Item",
                            vec![u32_block("bEna", 1), u32_block("BdNo", raw_item)],
                        ),
                    ],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.timelines[0].binder_index, raw_timeline as i32);
            assert_eq!(file.timelines[0].items[0].binder_index, raw_item as i32);
            assert_eq!(file.warnings, expected);
        }
    }

    #[test]
    fn timeline_item_target_diagnostics_use_signed_low_byte() {
        for (raw_emitter, raw_effector, raw_clip, target, expected) in [
            (255, 254, 253, AvfxTimelineItemTarget::None, Vec::<String>::new()),
            (
                256,
                257,
                258,
                AvfxTimelineItemTarget::Effector(1),
                vec![
                    "AVFX reference TmLn[0].Item[0].EfNo: index 1 outside Efct table (0 entries)"
                        .to_owned(),
                ],
            ),
            (
                256,
                254,
                258,
                AvfxTimelineItemTarget::Emitter(0),
                vec![
                    "AVFX reference TmLn[0].Item[0].EmNo: index 0 outside Emit table (0 entries)"
                        .to_owned(),
                ],
            ),
            (
                255,
                254,
                258,
                AvfxTimelineItemTarget::Clip(2),
                vec![
                    "AVFX reference TmLn[0].Item[0].ClNo: index 2 outside TmLn[0].Clip table (0 entries)"
                        .to_owned(),
                ],
            ),
        ] {
            let bytes = container(
                "AVFX",
                vec![container(
                    "TmLn",
                    vec![container(
                        "Item",
                        vec![
                            u32_block("bEna", 1),
                            u32_block("EmNo", raw_emitter),
                            u32_block("EfNo", raw_effector),
                            u32_block("ClNo", raw_clip),
                        ],
                    )],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            let item = file.timelines[0].items[0];
            assert_eq!(item.emitter_index, raw_emitter as i32);
            assert_eq!(item.effector_index, raw_effector as i32);
            assert_eq!(item.clip_index, raw_clip as i32);
            assert_eq!(item.target(), target);
            assert_eq!(file.warnings, expected);
        }
    }

    #[test]
    fn preserves_unsigned_list_indexes_and_resolves_null_mask() {
        for scalar_texture in [-1, 3, 255] {
            let bytes = container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 5),
                        container(
                            "TC1",
                            vec![
                                u32_block("bEna", 1),
                                i32_block("TxNo", scalar_texture),
                                block("TLst", &[255, 0, 127, 128, 254]),
                            ],
                        ),
                        container("Data", vec![block("MdNo", &[255, 0, 127, 128, 254])]),
                    ],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            let particle = &file.particles[0];
            let texture = particle.texture_color1.as_ref().unwrap();
            assert_eq!(texture.texture_list, [255, 0, 127, 128, 254]);
            assert_eq!(texture.mask_texture_index, 255);
            assert_eq!(texture.effective_texture(), scalar_texture);
            assert_eq!(texture.tc1_static_texture(), -1);
            assert!(!texture.is_shape_mask());
            let AvfxParticleData::Model { model_indexes, .. } = &particle.data else {
                panic!("expected model data");
            };
            assert_eq!(model_indexes, &[255, 0, 127, 128, 254]);
        }
    }

    #[test]
    fn parses_synthetic_file() {
        let file = AvfxFile::parse(&synthetic_avfx()).expect("synthetic avfx should parse");
        assert_eq!(file.version, 0x2011_0913);
        assert_eq!(file.schedulers.len(), 1);
        assert_eq!(file.timelines.len(), 1);
        assert_eq!(file.emitters.len(), 1);
        assert_eq!(file.particles.len(), 1);
        assert_eq!(file.binders.len(), 1);
        assert_eq!(
            file.texture_paths,
            vec!["vfx/eff/vws_test.atex".to_string()]
        );

        let scheduler = &file.schedulers[0];
        assert_eq!(scheduler.items.len(), 1);
        assert_eq!(scheduler.items[0].timeline_index, 0);
        assert_eq!(scheduler.triggers.len(), 12);

        let timeline = &file.timelines[0];
        assert_eq!(timeline.loop_end, 120);
        assert_eq!(timeline.items.len(), 1);
        assert_eq!(timeline.items[0].emitter_index, 0);

        let emitter = &file.emitters[0];
        assert_eq!(emitter.emitter_type, Some(EmitterType::Cone));
        assert_eq!(emitter.particle_items.len(), 1);
        assert_eq!(emitter.particle_items[0].create_count, 4);
        assert_eq!(emitter.life.value, 30.0);
        assert_eq!(emitter.rotation_order, 2);
        // 容器曲线：发射器 Pos.Y = 0.5、Scl.X = 2，缺省轴取默认。
        assert!((emitter.position.evaluate(0.0, 0.0)[1] - 0.5).abs() < 1.0e-6);
        assert!((emitter.position.evaluate(0.0, 0.0)[0]).abs() < 1.0e-6);
        assert!((emitter.scale.evaluate(0.0, 1.0)[0] - 2.0).abs() < 1.0e-6);
        assert!((emitter.scale.evaluate(0.0, 1.0)[1] - 1.0).abs() < 1.0e-6);
        match emitter.data.as_ref() {
            Some(AvfxEmitterData::Cone(cone)) => {
                assert!((cone.inner_size.evaluate(0.0)[2] - 0.1).abs() < 1.0e-6);
                assert_eq!(cone.rotation.angles[0].value(0.0, 0.0), 0.5);
            }
            other => panic!("expected cone emitter data, got {other:?}"),
        }

        let particle = &file.particles[0];
        assert_eq!(particle.particle_type, Some(ParticleType::Quad));
        assert_eq!(particle.draw_mode, 2);
        assert!(particle.is_billboard());
        // 颜色容器：RGB × Bri，alpha 包络。
        let rgba = particle.color.rgba(0.0);
        assert!((rgba[0] - 2.0).abs() < 1.0e-6); // 1.0 × Bri 2.0
        assert!((rgba[1] - 1.6).abs() < 1.0e-6); // 0.8 × 2.0
        assert!((rgba[2] - 0.8).abs() < 1.0e-6); // 0.4 × 2.0
        assert!((rgba[3] - 0.0).abs() < 1.0e-6);
        assert!((particle.color.rgba(30.0)[3] - 1.0).abs() < 1.0e-6);
        // 三轴曲线容器：Scl.X = 0.2，缺省轴默认。
        assert!((particle.scale.evaluate(0.0, 1.0)[0] - 0.2).abs() < 1.0e-6);
        assert!((particle.scale.evaluate(0.0, 1.0)[1] - 1.0).abs() < 1.0e-6);
        // 格式层便利值仍让 TLst 优先；TC1 客户端运行时不会回退 TxNo。
        let tc1 = particle.texture_color1.as_ref().expect("TC1");
        assert!(tc1.enabled);
        assert_eq!(tc1.mask_texture_index, 0);
        assert_eq!(tc1.effective_texture(), 0);
        assert_eq!(tc1.tc1_static_texture(), 0);
        // UvSt 两轴容器：Scl.X=0.5、Y 缺省 1；Scr.Y=0.25、X 缺省 0。
        assert_eq!(particle.uv_sets.len(), 1);
        let uv = &particle.uv_sets[0];
        assert!((uv.scale.evaluate(0.0, 1.0)[0] - 0.5).abs() < 1.0e-6);
        assert!((uv.scale.evaluate(0.0, 1.0)[1] - 1.0).abs() < 1.0e-6);
        assert!((uv.scroll.evaluate(0.0, 0.0)[1] - 0.25).abs() < 1.0e-6);
        assert!((uv.scroll.evaluate(0.0, 0.0)[0]).abs() < 1.0e-6);
        // Quad Data。
        assert_eq!(
            particle.data,
            AvfxParticleData::Quad {
                scaling_scale: 1,
                is_movement_particle: false,
            }
        );
        // 单 Modl 共存的发射顶点与绘制网格。
        assert_eq!(file.models.len(), 1);
        let model = &file.models[0];
        assert_eq!(model.emit_vertices.len(), 1);
        assert_eq!(model.emit_vertices[0].position, [0.1, 0.2, 0.3]);
        assert_eq!(model.emit_vertices[0].normal, [0.0, 1.0, 0.0]);
        assert_eq!(model.emit_vertices[0].color, [255, 128, 64, 255]);
        let draw = model.draw.as_ref().expect("draw model");
        assert_eq!(draw.vertices.len(), 3);
        assert!((draw.vertices[0].position[0] - 0.5).abs() < 1.0e-3);
        assert!((draw.vertices[0].uvs[0][0] - 0.25).abs() < 1.0e-3);
        assert_eq!(draw.indices, vec![0, 1, 2]);

        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.unknown_blocks.len(), 0);
    }

    #[test]
    fn emit_vertex_numbers_preserve_signed_values_and_file_order() {
        let values = [2_i16, 0, 1, i16::MIN, i16::MAX, -1, 2];
        let numbers: Vec<_> = values.into_iter().flat_map(i16::to_le_bytes).collect();
        let mut vertices = vec![0; 28 * values.len()];
        for (index, vertex) in vertices.chunks_exact_mut(28).enumerate() {
            vertex[..4].copy_from_slice(&(index as f32).to_le_bytes());
        }
        for numbers_first in [false, true] {
            let mut fields = vec![block("VEmt", &vertices), block("VNum", &numbers)];
            if numbers_first {
                fields.reverse();
            }
            let file =
                AvfxFile::parse(&container("AVFX", vec![container("Modl", fields)])).unwrap();
            let geometry = &file.models[0];
            assert_eq!(geometry.emit_vertex_numbers, values);
            let positions: Vec<_> = geometry
                .emit_vertices
                .iter()
                .map(|v| v.position[0])
                .collect();
            assert_eq!(positions, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
            assert_eq!(
                file.warnings,
                ["Modl[0].VNum: 3 indexes outside VEmt; model births selecting them are skipped"]
            );
        }
    }

    #[test]
    fn emit_vertex_number_count_mismatches_are_diagnosed_without_dropping_values() {
        for (vertex_count, number_count) in [(0, 1), (2, 1), (1, 2), (1, 0)] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Modl",
                    vec![
                        block("VEmt", &vec![0; 28 * vertex_count]),
                        block("VNum", &vec![0; 2 * number_count]),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(file.models[0].emit_vertices.len(), vertex_count);
            assert_eq!(file.models[0].emit_vertex_numbers.len(), number_count);
            assert!(file.warnings.contains(&format!(
                "Modl[0].VNum: {number_count} entries for {vertex_count} VEmt vertices"
            )));
        }
    }

    #[test]
    fn missing_emit_vertex_numbers_are_diagnosed() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("Modl", vec![block("VEmt", &[0; 28])])],
        ))
        .unwrap();
        assert_eq!(
            file.warnings,
            ["Modl[0].VNum: 0 entries for 1 VEmt vertices"]
        );
        assert_eq!(file.models[0].emit_vertices.len(), 1);
        assert!(file.models[0].emit_vertex_numbers.is_empty());
    }

    #[test]
    fn model_shape_rotation_and_optional_position_no_longer_warn_as_unsupported() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Emit",
                vec![
                    u32_block("EVT", 5),
                    container(
                        "Data",
                        vec![
                            i32_block("GeMT", 3),
                            curve("AnX", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0),
                            curve("AnZR", vec![key_bytes(0, 1, 0.0, 0.0, 0.3)], 0),
                        ],
                    ),
                    container(
                        "ItPr",
                        vec![
                            u32_block("bEnb", 1),
                            i32_block("PICd", 1),
                            u32_block("ICbP", 1),
                        ],
                    ),
                ],
            )],
        ))
        .unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
    }

    #[test]
    fn draw_vertices_preserve_homogeneous_position_and_raw_directions() {
        let mut bytes = Vec::new();
        for value in [0.5_f32, -0.5, 0.25, 2.0] {
            bytes.extend_from_slice(&half::f16::from_f32(value).to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 127, 128, 255, 254, 129, 64, 1]);
        bytes.extend_from_slice(&[32, 96, 160, 224]);
        for value in [0.25_f32, 0.75, -1.0, 2.0, 0.5, -0.5, 3.0, 4.0] {
            bytes.extend_from_slice(&half::f16::from_f32(value).to_le_bytes());
        }
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container("Modl", vec![block("VDrw", &bytes)])],
        ))
        .unwrap();
        let vertex = &file.models[0].draw.as_ref().unwrap().vertices[0];
        assert_eq!(vertex.position, [0.5, -0.5, 0.25]);
        assert_eq!(vertex.position_w, 2.0);
        assert_eq!(vertex.normal, [0, 127, 128, 255]);
        assert_eq!(vertex.tangent, [254, 129, 64, 1]);
        assert_eq!(vertex.color, [32, 96, 160, 224]);
        assert_eq!(
            vertex.uvs,
            [[0.25, 0.75], [-1.0, 2.0], [0.5, -0.5], [3.0, 4.0]]
        );
    }

    #[test]
    fn nested_truncation_and_curve_record_counts_are_diagnosed() {
        let truncated_header = container(
            "AVFX",
            vec![container("Emit", vec![block("CrI", &[0, 1, 2])])],
        );
        let file = AvfxFile::parse(&truncated_header).unwrap();
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.starts_with("AVFX.Emit.CrI at")
                    && warning.contains("truncated block header"))
        );

        let mut truncated_keys = block("Keys", &[]);
        truncated_keys[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let bytes = container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![container(
                    "Pos",
                    vec![container("Y", vec![u32_block("KeyC", 1), truncated_keys])],
                )],
            )],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.starts_with("AVFX.Ptcl.Pos.Y.Keys at")
                    && warning.contains("declares 4294967295 bytes"))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("KeyC") && warning.contains("parsed 0"))
        );

        for remainder in 1..16 {
            let bytes = container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![container(
                        "UvSt",
                        vec![container(
                            "Rot",
                            vec![
                                i32_block("KeyC", 2),
                                block("Keys", &vec![0; 16 + remainder]),
                            ],
                        )],
                    )],
                )],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.particles[0].uv_sets[0].rotation.keys.len(), 1);
            assert!(
                file.warnings
                    .iter()
                    .any(|warning| warning.starts_with("AVFX.Ptcl.UvSt.Rot.Keys")
                        && warning.contains("trailing bytes"))
            );
            assert!(
                file.warnings
                    .iter()
                    .any(|warning| warning.starts_with("AVFX.Ptcl.UvSt.Rot.KeyC")
                        && warning.contains("declared 2, parsed 1"))
            );
        }
    }

    #[test]
    fn validation_distinguishes_containers_from_opaque_and_disabled_fields() {
        let bytes = container(
            "AVFX",
            vec![
                block("Futr", &[0xff; 13]),
                container("TmLn", vec![block("Clip", &[0xff; 164])]),
                container("Ptcl", vec![i32_block("Life", -1), i32_block("TC1", -1)]),
                container("Bind", vec![i32_block("Life", 60)]),
            ],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.unknown_blocks["Futr"], 1);

        let mut missing_padding = block("bEnb", &[1]);
        missing_padding.truncate(9);
        let bytes = container(
            "AVFX",
            vec![container("Emit", vec![block("ItPr", &missing_padding)])],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.starts_with("AVFX.Emit.ItPr")
                    && warning.contains("missing block padding"))
        );
    }

    #[test]
    fn global_boolean_options_keep_byte_payloads_and_missing_defaults() {
        let names = [
            "bDFP", "bFG", "bTS", "bASH", "bCBC", "bCul", "bCmS", "bFEL", "bOSE", "bOSt", "AFXe",
            "AFYe", "AFZe", "bGFE", "bLTS", "bAGS", "bSAB", "bSBV", "bSSV",
        ];
        for payload in [vec![0], vec![1], vec![1, 0, 0, 0], vec![]] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                names.iter().map(|name| block(name, &payload)).collect(),
            ))
            .unwrap();
            let g = &file.global;
            let actual = [
                g.is_delay_fast_particle,
                g.is_fit_ground,
                g.is_transform_skip,
                g.is_all_stop_on_hide,
                g.can_be_clipped_out,
                g.clip_box_enabled,
                g.is_camera_space,
                g.is_full_env_light,
                g.ose,
                g.is_clip_own_setting,
                g.fade_enabled[0],
                g.fade_enabled[1],
                g.fade_enabled[2],
                g.global_fog_enabled,
                g.lts_enabled,
                g.ags_enabled,
                g.sab_enabled,
                g.sbv_enabled,
                g.ssv_enabled,
            ];
            assert_eq!(
                actual,
                [payload.first() == Some(&1); 19],
                "payload={payload:?}"
            );
        }
        assert_eq!(
            AvfxFile::parse(&container("AVFX", vec![])).unwrap().global,
            AvfxFile::parse(&container(
                "AVFX",
                names.iter().map(|name| block(name, &[0])).collect()
            ))
            .unwrap()
            .global
        );
    }

    #[test]
    fn particle_boolean_options_keep_explicit_byte_zero_and_one() {
        let names = [
            "DsDt", "DsDw", "DsSp", "bS11", "bE24", "bATM", "bAFg", "bNea", "bFar", "bSCt",
        ];
        for payload in [vec![0], vec![1], vec![1, 0, 0, 0]] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    names.iter().map(|name| block(name, &payload)).collect(),
                )],
            ))
            .unwrap();
            let p = &file.particles[0];
            assert_eq!(
                [
                    p.depth_test,
                    p.depth_write,
                    p.is_soft_particle,
                    p.s11_enabled,
                    p.e24_enabled,
                    p.is_apply_tone_map,
                    p.is_apply_fog,
                    p.clip_near_enable,
                    p.clip_far_enable,
                    p.simple_anim_enable,
                ],
                [payload[0] != 0; 10],
                "payload={payload:?}"
            );
        }
        for fields in [vec![], names.iter().map(|name| block(name, &[])).collect()] {
            let file =
                AvfxFile::parse(&container("AVFX", vec![container("Ptcl", fields)])).unwrap();
            let p = &file.particles[0];
            assert!(p.depth_test);
            assert!(!p.depth_write && !p.is_soft_particle);
        }
    }

    #[test]
    fn effector_boolean_options_keep_byte_payloads() {
        for payload in [vec![0], vec![1], vec![1, 0, 0, 0]] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![
                    container(
                        "Efct",
                        vec![
                            u32_block("EfVT", 0),
                            block("bAOV", &payload),
                            block("bAGm", &payload),
                            container(
                                "Data",
                                ["bSdw", "bChS", "bMpS", "bMvS"]
                                    .iter()
                                    .map(|name| block(name, &payload))
                                    .collect(),
                            ),
                        ],
                    ),
                    container(
                        "Efct",
                        vec![
                            u32_block("EfVT", 5),
                            container("Data", vec![block("bOS", &payload)]),
                        ],
                    ),
                ],
            ))
            .unwrap();
            let expected = payload[0] != 0;
            let effect = &file.effectors[0];
            assert_eq!([effect.affect_other_vfx, effect.affect_game], [expected; 2]);
            let Some(AvfxEffectorData::PointLight {
                enable_shadow,
                enable_character_shadow,
                enable_map_shadow,
                enable_move_shadow,
                ..
            }) = &effect.data
            else {
                panic!("missing PointLight");
            };
            assert_eq!(
                [
                    *enable_shadow,
                    *enable_character_shadow,
                    *enable_map_shadow,
                    *enable_move_shadow
                ],
                [expected; 4]
            );
            let Some(AvfxEffectorData::DirectionalBlur(blur)) = &file.effectors[1].data else {
                panic!("missing DirectionalBlur");
            };
            assert_eq!(blur.one_side, expected);
        }
    }

    #[test]
    fn binder_missing_ring_progress_retains_native_zero_while_authored_one_is_preserved() {
        for value in [None, Some(1), Some(-1), Some(65535)] {
            let mut fields = vec![u32_block("bRng", 1)];
            if let Some(value) = value {
                fields.push(i32_block("RnPT", value));
            }
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Bind",
                    vec![u32_block("BnVr", 2), container("Prp1", fields)],
                )],
            ))
            .unwrap();
            let properties = file.binders[0].properties_1.as_ref().unwrap();
            assert!(properties.ring_enabled);
            assert_eq!(properties.ring_progress_time, value.unwrap_or(0));
        }
    }

    #[test]
    fn binder_boolean_options_keep_byte_and_wide_payloads() {
        let names = [
            "bStG", "bVSc", "bVSd", "bVSi", "bTSd", "bTSi", "bFTO", "bDSE", "bATS", "bIFY", "bBET",
        ];
        for payload in [
            vec![0],
            vec![255],
            vec![1, 0],
            vec![0, 0, 0],
            vec![0, 1, 0, 0],
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Bind",
                    names.iter().map(|name| block(name, &payload)).collect(),
                )],
            ))
            .unwrap();
            let binder = &file.binders[0];
            let actual = [
                binder.start_to_global_direction,
                binder.vfx_scale_enabled,
                binder.vfx_scale_depth_offset,
                binder.vfx_scale_interpolation,
                binder.transform_scale_depth_offset,
                binder.transform_scale_interpolation,
                binder.following_target_orientation,
                binder.document_scale_enabled,
                binder.adjust_to_screen_enabled,
                binder.ify,
                binder.bet,
            ];
            let expected = payload.iter().any(|value| *value != 0);
            assert_eq!(actual, [expected; 11], "payload={payload:?}");
            if expected {
                assert!(file.warnings.iter().any(|warning| warning.contains("bFTO")));
            }
        }
    }

    #[test]
    fn binder_transform_options_are_not_silent_when_enabled() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Bind",
                vec![
                    i32_block("RoTp", 2),
                    u32_block("bFTO", 1),
                    u32_block("bDSE", 1),
                    i32_block("bTSc", 1),
                ],
            )],
        ))
        .unwrap();
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("bFTO, bDSE, bTSc"))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("RoTp=2"))
        );
    }

    #[test]
    fn binder_depth_offset_scale_flags_are_preserved_and_diagnosed() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Bind",
                vec![u32_block("bVSd", 1), u32_block("bTSd", 1)],
            )],
        ))
        .unwrap();
        assert!(file.binders[0].vfx_scale_depth_offset);
        assert!(file.binders[0].transform_scale_depth_offset);
        assert!(
            file.warnings
                .iter()
                .any(|warning| { warning.contains("Bind[0]") && warning.contains("bVSd, bTSd") })
        );

        let disabled = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Bind",
                vec![u32_block("bVSd", 0), u32_block("bTSd", 0)],
            )],
        ))
        .unwrap();
        assert!(!disabled.binders[0].vfx_scale_depth_offset);
        assert!(!disabled.binders[0].transform_scale_depth_offset);
        assert!(
            !disabled
                .warnings
                .iter()
                .any(|warning| { warning.contains("bVSd") || warning.contains("bTSd") })
        );
    }

    #[test]
    fn binder_data_preserves_known_curves_and_opaque_fields() {
        let point_payload = [
            curve("SpS", vec![key_bytes(0, 1, 0.0, 0.0, 2.0)], 0),
            curve("SpSR", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0),
            block("Xtra", &[1, 2, 3, 4, 5]),
        ]
        .concat();
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Bind",
                    vec![u32_block("BnVr", 0), block("Data", &point_payload)],
                ),
                container(
                    "Bind",
                    vec![
                        u32_block("BnVr", 1),
                        container(
                            "Data",
                            vec![curve("COF", vec![key_bytes(0, 1, 0.0, 0.0, 0.75)], 0)],
                        ),
                    ],
                ),
                container(
                    "Bind",
                    vec![
                        u32_block("BnVr", 3),
                        container(
                            "Data",
                            vec![curve("DstR", vec![key_bytes(0, 1, 0.0, 0.0, 4.0)], 0)],
                        ),
                    ],
                ),
            ],
        ))
        .unwrap();
        let point = file.binders[0].data.as_ref().unwrap();
        assert_eq!(point.spring_strength.as_ref().unwrap().value(0.0, 0.0), 2.0);
        assert_eq!(
            point
                .spring_strength_random
                .as_ref()
                .unwrap()
                .value(0.0, 0.0),
            0.5
        );
        assert_eq!(point.source.raw_payload, point_payload);
        assert!(point.source.curves.contains_key("SpS"));
        assert_eq!(
            file.binders[1]
                .data
                .as_ref()
                .unwrap()
                .carry_over_factor
                .as_ref()
                .unwrap()
                .value(0.0, 0.0),
            0.75
        );
        assert_eq!(
            file.binders[2]
                .data
                .as_ref()
                .unwrap()
                .distance_random
                .as_ref()
                .unwrap()
                .value(0.0, 0.0),
            4.0
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("Bind[0].Data"))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("Bind[1].Data"))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("Bind[2].Data"))
        );
    }

    #[test]
    fn binder_data_curves_receive_nested_structure_diagnostics() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Bind",
                vec![
                    u32_block("BnVr", 0),
                    container(
                        "Data",
                        vec![container(
                            "SpS",
                            vec![u32_block("KeyC", 1), block("Keys", &[0; 3])],
                        )],
                    ),
                ],
            )],
        ))
        .unwrap();
        assert!(file.warnings.iter().any(|warning| {
            warning.contains("AVFX.Bind.Data.SpS.Keys") && warning.contains("trailing bytes")
        }));
    }

    #[test]
    fn effector_data_decodes_type_specific_fields_and_retains_payload() {
        let light_data = vec![
            color_curve("Col", vec![key_bytes(0, 1, 0.25, 0.5, 0.75)], None, None),
            curve("DstS", vec![key_bytes(0, 1, 0.0, 0.0, 2.0)], 0),
            curve3(
                "Pos",
                Some(vec![key_bytes(0, 1, 0.0, 0.0, 3.0)]),
                None,
                None,
            ),
            i32_block("Attn", 2),
            u32_block("bSdw", 1),
            u32_block("SCDN", 4.5_f32.to_bits()),
            block("Blob", &[0xff; 5]),
            i32_block("Dupe", 1),
            i32_block("Dupe", 2),
        ];
        let quake_data = vec![
            curve("Att", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0),
            curve("RdOR", vec![key_bytes(0, 1, 0.0, 0.0, 8.0)], 0),
            curve3(
                "Rot",
                None,
                Some(vec![key_bytes(0, 1, 0.0, 0.0, 30.0)]),
                None,
            ),
        ];
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Efct",
                    vec![u32_block("EfVT", 0), container("Data", light_data.clone())],
                ),
                container(
                    "Efct",
                    vec![u32_block("EfVT", 6), container("Data", quake_data.clone())],
                ),
                container(
                    "Efct",
                    vec![u32_block("EfVT", 9), container("Data", quake_data)],
                ),
            ],
        ))
        .unwrap();
        let light = &file.effectors[0];
        assert_eq!(light.data_payload, light_data.concat());
        let Some(AvfxEffectorData::PointLight {
            color,
            distance_scale,
            position,
            attenuation,
            enable_shadow,
            shadow_create_distance_near,
            ..
        }) = &light.data
        else {
            panic!("expected point light data");
        };
        assert_eq!(color.rgb.as_ref().unwrap().keys[0].x, 0.25);
        assert_eq!(distance_scale.as_ref().unwrap().value(0.0, 0.0), 2.0);
        assert_eq!(
            position
                .as_ref()
                .unwrap()
                .x
                .as_ref()
                .unwrap()
                .value(0.0, 0.0),
            3.0
        );
        assert_eq!(*attenuation, Some(2));
        assert!(*enable_shadow);
        assert_eq!(*shadow_create_distance_near, Some(4.5));
        for effector in &file.effectors[1..] {
            let Some(AvfxEffectorData::CameraQuake {
                attenuation,
                radius_out_random,
                rotation,
                ..
            }) = &effector.data
            else {
                panic!("expected camera quake data");
            };
            assert_eq!(attenuation.as_ref().unwrap().value(0.0, 0.0), 0.5);
            assert_eq!(radius_out_random.as_ref().unwrap().value(0.0, 0.0), 8.0);
            assert_eq!(
                rotation
                    .as_ref()
                    .unwrap()
                    .y
                    .as_ref()
                    .unwrap()
                    .value(0.0, 0.0),
                30.0
            );
        }
        assert_eq!(
            file.warnings
                .iter()
                .filter(|warning| warning.contains(".Data: effector data"))
                .count(),
            3
        );
    }

    #[test]
    fn effector_data_maps_blur_light_and_unknown_types() {
        let payload = vec![
            curve("Len", vec![key_bytes(0, 1, 0.0, 0.0, 2.0)], 0),
            curve("AStr", vec![key_bytes(0, 1, 0.0, 0.0, 3.0)], 0),
            curve("Ang", vec![key_bytes(0, 1, 0.0, 0.0, 4.0)], 0),
            curve("Gra", vec![key_bytes(0, 1, 0.0, 0.0, 5.0)], 0),
            curve("IRad", vec![key_bytes(0, 1, 0.0, 0.0, 6.0)], 0),
            curve("ORad", vec![key_bytes(0, 1, 0.0, 0.0, 7.0)], 0),
            u32_block("FSDc", 1.5_f32.to_bits()),
            u32_block("bOS", 1),
        ];
        let fields = [1, 3, 4, 5, 7, 8, 99]
            .into_iter()
            .map(|raw_type| {
                container(
                    "Efct",
                    vec![
                        u32_block("EfVT", raw_type),
                        container("Data", payload.clone()),
                    ],
                )
            })
            .collect();
        let file = AvfxFile::parse(&container("AVFX", fields)).unwrap();
        assert!(matches!(
            file.effectors[0].data,
            Some(AvfxEffectorData::DirectionalLight { .. })
        ));
        assert!(matches!(
            file.effectors[1].data,
            Some(AvfxEffectorData::ChromaticAberration { .. })
        ));
        assert!(matches!(
            file.effectors[2].data,
            Some(AvfxEffectorData::GaussianBlur(_))
        ));
        assert!(matches!(
            file.effectors[3].data,
            Some(AvfxEffectorData::DirectionalBlur(_))
        ));
        assert!(matches!(
            file.effectors[4].data,
            Some(AvfxEffectorData::RadialBlur(_))
        ));
        assert!(matches!(
            file.effectors[5].data,
            Some(AvfxEffectorData::Other)
        ));
        assert!(matches!(
            file.effectors[6].data,
            Some(AvfxEffectorData::Other)
        ));
        let Some(AvfxEffectorData::DirectionalBlur(blur)) = &file.effectors[3].data else {
            unreachable!();
        };
        assert_eq!(blur.angle_strength.as_ref().unwrap().value(0.0, 0.0), 3.0);
        assert_eq!(blur.angle.as_ref().unwrap().value(0.0, 0.0), 4.0);
        assert_eq!(blur.fade_start_distance, Some(1.5));
        assert!(blur.one_side);
        assert!(blur.gradation.is_none());
        assert!(blur.inner_radius.is_none());
        assert!(blur.outer_radius.is_none());
        for effector in [&file.effectors[2], &file.effectors[4]] {
            let blur = match effector.data.as_ref().unwrap() {
                AvfxEffectorData::GaussianBlur(blur) | AvfxEffectorData::RadialBlur(blur) => blur,
                _ => unreachable!(),
            };
            assert_eq!(blur.gradation.as_ref().unwrap().value(0.0, 0.0), 5.0);
            assert_eq!(blur.inner_radius.as_ref().unwrap().value(0.0, 0.0), 6.0);
            assert_eq!(blur.outer_radius.as_ref().unwrap().value(0.0, 0.0), 7.0);
            assert!(blur.angle_strength.is_none());
            assert!(blur.angle.is_none());
            assert!(!blur.one_side);
        }
        assert!(
            file.effectors
                .iter()
                .all(|effector| effector.data_payload == payload.concat())
        );
    }

    #[test]
    fn effector_data_curves_receive_nested_structure_diagnostics() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Efct",
                    vec![
                        u32_block("EfVT", 1),
                        container(
                            "Data",
                            vec![container(
                                "Amb",
                                vec![container(
                                    "RGB",
                                    vec![
                                        i32_block("KeyC", 2),
                                        block("Keys", &key_bytes(0, 1, 1.0, 1.0, 1.0)),
                                    ],
                                )],
                            )],
                        ),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 0),
                        container("Data", vec![u32_block("Str", 1)]),
                    ],
                ),
            ],
        ))
        .unwrap();
        assert!(file.warnings.iter().any(|warning| {
            warning.contains("AVFX.Efct.Data.Amb.RGB.KeyC")
                && warning.contains("declared 2, parsed 1")
        }));
        assert!(
            file.warnings
                .iter()
                .all(|warning| !warning.contains("AVFX.Ptcl.Data.Str"))
        );
    }

    #[test]
    fn emitter_shape_data_preserves_rotation_random_curves_and_raw_fields() {
        let names = [
            "AnX", "AnY", "AnZ", "AnXR", "AnYR", "AnZR", "IjSR", "InS", "InSR", "OuS", "OuSR",
            "Rad", "RadR", "Rads", "IjS", "IjA", "IjAR", "Len",
        ];
        let mut expected = BTreeMap::new();
        let mut fields = vec![
            block("ROT", &[5]),
            i32_block("GeMT", 7),
            i32_block("DivX", 8),
            i32_block("DivY", 9),
            i32_block("MdNo", -1),
        ];
        for (index, name) in names.into_iter().enumerate() {
            let value = index as f32 + 0.25;
            let keys = vec![
                key_bytes(-5, 0, 0.2, 0.7, value),
                key_bytes(13, 2, 0.3, 0.8, -value),
            ];
            fields.push(container(
                name,
                vec![
                    i32_block("KeyC", 2),
                    u32_block("BvPr", BEHAVIOR_REPEAT),
                    u32_block("BvPo", BEHAVIOR_ADD),
                    u32_block("RanT", 5),
                    block("Keys", &keys.concat()),
                ],
            ));
            expected.insert(
                name,
                AvfxCurve {
                    pre_behavior: BEHAVIOR_REPEAT,
                    post_behavior: BEHAVIOR_ADD,
                    random_type: 5,
                    keys: vec![
                        AvfxCurveKey {
                            time: -5,
                            interpolation: 0,
                            x: 0.2,
                            y: 0.7,
                            z: value,
                        },
                        AvfxCurveKey {
                            time: 13,
                            interpolation: 2,
                            x: 0.3,
                            y: 0.8,
                            z: -value,
                        },
                    ],
                },
            );
        }
        fields.extend([
            block("Blob", &[0xff; 13]),
            i32_block("Dupe", 1),
            i32_block("Dupe", 2),
        ]);
        for raw_type in [0, 1, 2, 3, 4, 5, 999] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Emit",
                    vec![
                        u32_block("EVT", raw_type),
                        container("Data", fields.clone()),
                    ],
                )],
            ))
            .unwrap();
            let emitter = &file.emitters[0];
            assert_eq!(emitter.data_payload, fields.concat(), "type {raw_type}");
            let (rotation, speed, speed_random) = match &emitter.data {
                Some(AvfxEmitterData::Cone(data)) => {
                    assert_eq!(data.inner_size, expected["InS"]);
                    assert_eq!(data.inner_size_random, expected["InSR"]);
                    assert_eq!(data.outer_size, expected["OuS"]);
                    assert_eq!(data.outer_size_random, expected["OuSR"]);
                    assert_eq!(data.injection_angle, expected["IjA"]);
                    assert_eq!(data.injection_angle_random, expected["IjAR"]);
                    (
                        &data.rotation,
                        &data.injection_speed,
                        &data.injection_speed_random,
                    )
                }
                Some(AvfxEmitterData::ConeModel(data)) => {
                    assert_eq!(
                        (data.generate_method, data.divide_x, data.divide_y),
                        (7, 8, 9)
                    );
                    assert_eq!(data.radius, expected["Rad"]);
                    assert_eq!(data.radius_random, expected["RadR"]);
                    assert_eq!(data.injection_angle, expected["IjA"]);
                    assert_eq!(data.injection_angle_random, expected["IjAR"]);
                    (
                        &data.rotation,
                        &data.injection_speed,
                        &data.injection_speed_random,
                    )
                }
                Some(AvfxEmitterData::SphereModel(data)) => {
                    assert_eq!(
                        (data.generate_method, data.divide_x, data.divide_y),
                        (7, 8, 9)
                    );
                    assert_eq!(data.radius, expected["Rads"]);
                    (
                        &data.rotation,
                        &data.injection_speed,
                        &data.injection_speed_random,
                    )
                }
                Some(AvfxEmitterData::CylinderModel(data)) => {
                    assert_eq!(
                        (data.generate_method, data.divide_x, data.divide_y),
                        (7, 8, 9)
                    );
                    assert_eq!(data.radius, expected["Rad"]);
                    assert_eq!(data.length, expected["Len"]);
                    (
                        &data.rotation,
                        &data.injection_speed,
                        &data.injection_speed_random,
                    )
                }
                Some(AvfxEmitterData::Model(data)) => {
                    assert_eq!((data.model_index, data.generate_method), (-1, 7));
                    (
                        &data.rotation,
                        &data.injection_speed,
                        &data.injection_speed_random,
                    )
                }
                None if raw_type == 0 || raw_type == 999 => continue,
                other => panic!("unexpected type {raw_type}: {other:?}"),
            };
            assert_eq!(rotation.order, 5);
            for (axis, name) in ["AnX", "AnY", "AnZ"].into_iter().enumerate() {
                assert_eq!(rotation.angles[axis], expected[name]);
            }
            for (axis, name) in ["AnXR", "AnYR", "AnZR"].into_iter().enumerate() {
                assert_eq!(rotation.angles_random[axis], expected[name]);
            }
            assert_eq!(*speed, expected["IjS"]);
            assert_eq!(*speed_random, expected["IjSR"]);
        }
    }

    #[test]
    fn emitter_vr_preserves_base_and_random_curves_and_reports_motion_scope() {
        for (index, field) in ["VRX", "VRY", "VRZ", "VRXR", "VRYR", "VRZR"]
            .into_iter()
            .enumerate()
        {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Emit",
                    vec![container(
                        field,
                        vec![
                            i32_block("KeyC", 1),
                            u32_block("BvPr", BEHAVIOR_REPEAT),
                            u32_block("BvPo", BEHAVIOR_ADD),
                            u32_block("RanT", 5),
                            block("Keys", &key_bytes(7, 2, 0.2, 0.8, -0.5)),
                        ],
                    )],
                )],
            ))
            .unwrap();
            let emitter = &file.emitters[0];
            let curves: Vec<_> = emitter
                .rotation_velocity
                .iter()
                .chain(&emitter.rotation_velocity_random)
                .collect();
            for (axis, curve) in curves.iter().enumerate() {
                assert_eq!(curve.keys.len(), usize::from(axis == index), "{field}");
            }
            assert_eq!(curves[index].keys[0].time, 7);
            assert_eq!(curves[index].keys[0].z, -0.5);
            assert_eq!(curves[index].keys[0].interpolation, 2);
            assert_eq!(curves[index].keys[0].x, 0.2);
            assert_eq!(curves[index].keys[0].y, 0.8);
            assert_eq!(curves[index].random_type, 5);
            assert_eq!(curves[index].pre_behavior, BEHAVIOR_REPEAT);
            assert_eq!(curves[index].post_behavior, BEHAVIOR_ADD);
            assert_eq!(
                file.warnings,
                [format!(
                    "Emit[0]: nonzero {field} steer this emitter's injected motion via continuous integration; staged child-emitter history remains approximate, and zero-velocity roots are unaffected"
                )]
            );

            let malformed = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Emit",
                    vec![container(
                        field,
                        vec![i32_block("KeyC", 1), block("Keys", &[])],
                    )],
                )],
            ))
            .unwrap();
            assert!(
                malformed
                    .warnings
                    .iter()
                    .any(|warning| { warning.contains(field) && warning.contains("KeyC") }),
                "{field}: {:?}",
                malformed.warnings
            );
        }
    }

    #[test]
    fn particle_vr_diagnostics_identify_approximate_direction_curves() {
        for field in ["VRX", "VRY", "VRZ", "VRXR", "VRYR", "VRZR"] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![
                    container("Ptcl", vec![u32_block("PrVT", 8)]),
                    container(
                        "Ptcl",
                        vec![
                            u32_block("PrVT", 8),
                            curve(field, vec![key_bytes(0, 1, 1.0, 1.0, -0.5)], 0),
                        ],
                    ),
                ],
            ))
            .unwrap();
            assert_eq!(
                file.warnings,
                [format!(
                    "Ptcl[1]: nonzero {field} direction curves use continuous integration; client update clocks and full coordinate inheritance remain approximate"
                )]
            );
            let particle = &file.particles[1];
            assert_eq!(
                particle
                    .rotation_velocity
                    .iter()
                    .chain(&particle.rotation_velocity_random)
                    .flat_map(|curve| &curve.keys)
                    .map(|key| key.z)
                    .collect::<Vec<_>>(),
                [-0.5]
            );
        }
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 8),
                    curve("VRX", vec![key_bytes(0, 0, 1.0, 1.0, 0.0)], 0),
                    curve("VRY", vec![], 0),
                    curve("VRZR", vec![key_bytes(0, 1, 1.0, 1.0, 0.0)], 0),
                ],
            )],
        ))
        .unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
    }

    #[test]
    fn cone_model_binding_diagnostics_only_report_active_always_curves() {
        for kind in ["ItPr", "ItEm"] {
            for mode in [1, 4, 12, 6] {
                for follow in [false, true] {
                    let file = AvfxFile::parse(&container(
                        "AVFX",
                        vec![container(
                            "Emit",
                            vec![
                                u32_block("EVT", 2),
                                container(
                                    "Data",
                                    vec![
                                        i32_block("GeMT", 3),
                                        container(
                                            "AnXR",
                                            vec![
                                                u32_block("RanT", mode),
                                                u32_block("KeyC", 1),
                                                block("Keys", &key_bytes(0, 1, 0.0, 0.0, 0.5)),
                                            ],
                                        ),
                                    ],
                                ),
                                container(
                                    kind,
                                    vec![
                                        u32_block("bEnb", 1),
                                        i32_block("PICd", 1),
                                        u32_block("ICbP", u32::from(follow)),
                                    ],
                                ),
                            ],
                        )],
                    ))
                    .unwrap();
                    if follow && matches!(mode, 4 | 12) {
                        assert_eq!(
                            file.warnings,
                            [format!(
                                "emitter[0].{kind}[0]: ICbP ConeModel AnXR Always curves use integer-frame random samples; client callback caches are unsupported"
                            )]
                        );
                    } else {
                        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
                    }
                }
            }
        }
    }

    #[test]
    fn sphere_model_defaults_signed_divisions_and_binding_diagnostics() {
        for kind in ["ItPr", "ItEm"] {
            for (fields, expected) in [
                (vec![], None),
                (vec![i32_block("DivY", 254)], None),
                (vec![i32_block("DivX", 260), i32_block("GeMT", 263)], None),
                (
                    vec![i32_block("DivX", 128)],
                    Some("nonpositive signed-byte DivX"),
                ),
                (
                    vec![i32_block("DivX", 256)],
                    Some("nonpositive signed-byte DivX"),
                ),
                (vec![i32_block("DivY", 256)], Some("zero signed-byte DivY")),
                (
                    vec![i32_block("GeMT", 8)],
                    Some("unsupported; sphere-model births are skipped"),
                ),
            ] {
                let file = AvfxFile::parse(&container(
                    "AVFX",
                    vec![container(
                        "Emit",
                        vec![
                            u32_block("EVT", 3),
                            container("Data", fields.clone()),
                            container(
                                kind,
                                vec![
                                    u32_block("bEnb", 1),
                                    i32_block("PICd", 1),
                                    u32_block("ICbP", 1),
                                ],
                            ),
                        ],
                    )],
                ))
                .unwrap();
                let Some(AvfxEmitterData::SphereModel(data)) = &file.emitters[0].data else {
                    unreachable!()
                };
                if fields.is_empty() {
                    assert_eq!((data.divide_x, data.divide_y), (3, 3));
                }
                match expected {
                    Some(text) => {
                        assert_eq!(file.warnings.iter().filter(|v| v.contains(text)).count(), 1)
                    }
                    None => assert!(file.warnings.is_empty(), "{:?}", file.warnings),
                }
            }
            for method in 0..8 {
                for mode in [0, 4, 12, 6] {
                    for follow in [false, true] {
                        let file = AvfxFile::parse(&container(
                            "AVFX",
                            vec![container(
                                "Emit",
                                vec![
                                    u32_block("EVT", 3),
                                    container(
                                        "Data",
                                        vec![
                                            i32_block("GeMT", method),
                                            container(
                                                "AnXR",
                                                vec![
                                                    u32_block("RanT", mode),
                                                    u32_block("KeyC", 1),
                                                    block("Keys", &key_bytes(0, 1, 0.0, 0.0, 0.5)),
                                                ],
                                            ),
                                        ],
                                    ),
                                    container(
                                        kind,
                                        vec![
                                            u32_block("bEnb", 1),
                                            i32_block("PICd", 1),
                                            u32_block("ICbP", u32::from(follow)),
                                        ],
                                    ),
                                ],
                            )],
                        ))
                        .unwrap();
                        assert_eq!(
                            file.warnings.len(),
                            usize::from(follow && method & 2 != 0 && matches!(mode, 4 | 12)),
                            "{:?}",
                            file.warnings
                        );
                        assert!(
                            file.warnings
                                .iter()
                                .all(|v| v.contains("ICbP SphereModel AnXR Always curves"))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cylinder_model_defaults_zero_y_and_binding_diagnostics_follow_client_fields() {
        for kind in ["ItPr", "ItEm"] {
            for (fields, expected) in [
                (vec![], None),
                (vec![i32_block("DivY", 0)], None),
                (vec![i32_block("DivX", 256)], Some("zero low-byte DivX")),
                (
                    vec![i32_block("GeMT", 8)],
                    Some("unsupported; cylinder-model births are skipped"),
                ),
                (vec![i32_block("GeMT", 259)], None),
                (
                    vec![container(
                        "AnXR",
                        vec![
                            u32_block("RanT", 12),
                            u32_block("KeyC", 1),
                            block("Keys", &key_bytes(0, 1, 0.0, 0.0, 0.5)),
                        ],
                    )],
                    Some("ICbP CylinderModel AnXR Always curves"),
                ),
            ] {
                let file = AvfxFile::parse(&container(
                    "AVFX",
                    vec![container(
                        "Emit",
                        vec![
                            u32_block("EVT", 4),
                            container("Data", fields.clone()),
                            container(
                                kind,
                                vec![
                                    u32_block("bEnb", 1),
                                    i32_block("PICd", 1),
                                    u32_block("ICbP", 1),
                                ],
                            ),
                        ],
                    )],
                ))
                .unwrap();
                let Some(AvfxEmitterData::CylinderModel(data)) = &file.emitters[0].data else {
                    panic!("CylinderModel")
                };
                if fields.is_empty() {
                    assert_eq!((data.divide_x, data.divide_y), (3, 3));
                }
                match expected {
                    Some(message) => assert!(
                        file.warnings.iter().any(|v| v.contains(message)),
                        "{:?}",
                        file.warnings
                    ),
                    None => assert!(file.warnings.is_empty(), "{:?}", file.warnings),
                }
            }
        }
    }

    #[test]
    fn cone_model_defaults_and_degenerate_divisions_are_explicit() {
        for (fields, diagnostic) in [
            (vec![], None),
            (vec![i32_block("GeMT", 8)], Some("unsupported")),
            (vec![i32_block("DivX", 256)], Some("zero low-byte")),
            (vec![i32_block("DivY", 0)], Some("zero low-byte")),
            (vec![i32_block("GeMT", 260)], None),
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Emit",
                    vec![
                        u32_block("EVT", 2),
                        container("Data", fields.clone()),
                        container(
                            "ItPr",
                            vec![
                                u32_block("bEnb", 1),
                                i32_block("PICd", 1),
                                u32_block("ICbP", 1),
                            ],
                        ),
                    ],
                )],
            ))
            .unwrap();
            let Some(AvfxEmitterData::ConeModel(data)) = &file.emitters[0].data else {
                panic!("ConeModel")
            };
            if fields.is_empty() {
                assert_eq!((data.divide_x, data.divide_y), (3, 3));
            }
            if let Some(diagnostic) = diagnostic {
                assert!(
                    file.warnings
                        .iter()
                        .any(|warning| warning.contains(diagnostic)),
                    "{:?}",
                    file.warnings
                );
            } else {
                assert!(file.warnings.is_empty(), "{:?}", file.warnings);
            }
        }
    }

    #[test]
    fn model_shape_byte_fields_preserve_raw_values_and_validate_effective_references() {
        for (index, effective) in [(256, 0), (-256, 0), (257, 1), (255, -1), (128, -128)] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![
                    container("Modl", vec![]),
                    container(
                        "Emit",
                        vec![
                            u32_block("EVT", 5),
                            container(
                                "Data",
                                vec![
                                    i32_block("MdNo", index),
                                    i32_block("GeMT", 259),
                                    i32_block("ROT", 256),
                                ],
                            ),
                        ],
                    ),
                ],
            ))
            .unwrap();
            let Some(AvfxEmitterData::Model(data)) = &file.emitters[0].data else {
                unreachable!()
            };
            assert_eq!(
                (data.model_index, data.generate_method, data.rotation.order),
                (index, 259, 256)
            );
            if effective == 1 {
                assert_eq!(file.warnings.len(), 1, "{:?}", file.warnings);
                assert_eq!(
                    file.warnings[0],
                    "AVFX reference Emit[0].Data.MdNo: index 1 outside Modl table (1 entries)"
                );
            } else {
                assert!(file.warnings.is_empty(), "{:?}", file.warnings);
            }
        }
    }

    #[test]
    fn model_binding_diagnostics_only_report_on_vertex_always_angles() {
        for kind in ["ItPr", "ItEm"] {
            for method in 0..8 {
                for mode in [1, 4, 12, 6] {
                    let file = AvfxFile::parse(&container(
                        "AVFX",
                        vec![container(
                            "Emit",
                            vec![
                                u32_block("EVT", 5),
                                container(
                                    "Data",
                                    vec![
                                        i32_block("GeMT", method + 256),
                                        container(
                                            "AnYR",
                                            vec![
                                                u32_block("RanT", mode),
                                                u32_block("KeyC", 1),
                                                block("Keys", &key_bytes(0, 1, 0.0, 0.0, 0.5)),
                                            ],
                                        ),
                                    ],
                                ),
                                container(
                                    kind,
                                    vec![
                                        u32_block("bEnb", 1),
                                        i32_block("PICd", 1),
                                        u32_block("ICbP", 1),
                                    ],
                                ),
                            ],
                        )],
                    ))
                    .unwrap();
                    if method & 2 != 0 && matches!(mode, 4 | 12) {
                        assert_eq!(
                            file.warnings,
                            [format!(
                                "emitter[0].{kind}[0]: ICbP Model AnYR Always curves use integer-frame random samples; client callback caches are unsupported"
                            )]
                        );
                    } else {
                        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
                    }
                }
            }
        }
    }

    #[test]
    fn emitter_shape_diagnostics_accept_consumed_curves_and_report_unknown_types() {
        let zero = curve("AnX", vec![key_bytes(0, 0, 1.0, 1.0, 0.0)], 0);
        let nonzero = |name| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0);
        for (raw_type, shape_field, supported) in [
            (1, "IjA", "InS"),
            (2, "RadR", "IjA"),
            (3, "AnZR", "Rads"),
            (4, "Len", "Rad"),
            (5, "AnYR", "IjS"),
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![
                    container("Emit", vec![u32_block("EVT", 0)]),
                    container(
                        "Emit",
                        vec![
                            u32_block("EVT", raw_type),
                            container(
                                "Data",
                                vec![
                                    zero.clone(),
                                    nonzero(shape_field),
                                    nonzero("IjSR"),
                                    nonzero(supported),
                                ],
                            ),
                        ],
                    ),
                ],
            ))
            .unwrap();
            assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        }
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Emit",
                vec![
                    u32_block("EVT", 1),
                    container("Data", vec![zero, curve("IjSR", vec![], 0)]),
                ],
            )],
        ))
        .unwrap();
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);

        for with_data in [false, true] {
            let mut fields = vec![u32_block("EVT", 999)];
            if with_data {
                fields.push(container("Data", vec![block("Blob", &[0xff; 13])]));
            }
            let file =
                AvfxFile::parse(&container("AVFX", vec![container("Emit", fields)])).unwrap();
            assert_eq!(
                file.warnings,
                ["emitter[0] EVT=999 is unsupported; sampler uses point fallback"]
            );
        }
    }

    #[test]
    fn emitter_shape_rotation_and_random_curve_truncation_is_diagnosed() {
        for name in [
            "AnX", "AnY", "AnZ", "AnXR", "AnYR", "AnZR", "InSR", "OuSR", "IjSR", "IjAR", "RadR",
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Emit",
                    vec![
                        u32_block("EVT", 1),
                        container(
                            "Data",
                            vec![container(
                                name,
                                vec![u32_block("KeyC", 1), block("Keys", &[0; 15])],
                            )],
                        ),
                    ],
                )],
            ))
            .unwrap();
            assert!(
                file.warnings.iter().any(|warning| {
                    warning.starts_with(&format!("AVFX.Emit.Data.{name}.Keys at 0x"))
                        && warning.contains("15 trailing bytes")
                }),
                "{name}: {:?}",
                file.warnings
            );
            assert!(
                file.warnings.iter().any(|warning| {
                    warning.starts_with(&format!("AVFX.Emit.Data.{name}.KeyC at 0x"))
                        && warning.contains("declared 1, parsed 0")
                }),
                "{name}: {:?}",
                file.warnings
            );
        }
    }

    #[test]
    fn windmill_preserves_uv_type_and_diagnoses_unsupported_low_byte() {
        for raw in [None, Some(0), Some(1), Some(257), Some(2), Some(-1)] {
            let fields = raw
                .map(|value| i32_block("WUvT", value))
                .into_iter()
                .collect();
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 2),
                        i32_block("RBDT", 0),
                        container("Data", fields),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(
                file.particles[0].data,
                AvfxParticleData::Windmill {
                    uv_type: raw.unwrap_or(0)
                }
            );
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|w| w.contains("quad approximation") || w.contains("RBDT"))
            );
            assert_eq!(
                file.warnings.iter().any(|w| w.contains("Windmill WUvT")),
                raw.unwrap_or(0) as u8 > 1
            );
        }
    }

    #[test]
    fn texture_filters_preserve_known_advanced_modes_and_diagnose_unknown_values() {
        let texture = |name, enabled, filter| {
            container(
                name,
                vec![u32_block("bEna", enabled), i32_block("TFT", filter)],
            )
        };
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 5),
                        texture("TC1", 1, 2),
                        texture("TC2", 1, 3),
                        texture("TC3", 1, 9),
                        texture("TC4", 0, 4),
                        texture("TD", 1, 4),
                        texture("TP", 1, 2),
                        texture("TN", 1, 7),
                        texture("TR", 1, 8),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 5),
                        texture("TC1", 1, -2),
                        texture("TN", 1, 4),
                        container(
                            "TR",
                            vec![
                                u32_block("bEna", 1),
                                u32_block("bUSC", 1),
                                i32_block("TFT", 9),
                            ],
                        ),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 8),
                        texture("TN", 1, 7),
                        texture("TR", 1, 8),
                    ],
                ),
                container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 5),
                        texture("TN", 0, 7),
                        texture("TR", 0, 8),
                    ],
                ),
            ],
        ))
        .unwrap();
        let particle = &file.particles[0];
        assert_eq!(particle.texture_color1.as_ref().unwrap().texture_filter, 2);
        assert_eq!(particle.texture_color2.as_ref().unwrap().texture_filter, 3);
        assert_eq!(particle.texture_color3.as_ref().unwrap().texture_filter, 9);
        assert_eq!(particle.texture_color4.as_ref().unwrap().texture_filter, 4);
        assert_eq!(
            particle.texture_distortion.as_ref().unwrap().texture_filter,
            4
        );
        assert_eq!(particle.texture_palette.as_ref().unwrap().texture_filter, 2);
        for needle in [
            "Ptcl[0].TC3.TFT=9 is unknown",
            "Ptcl[0].TN.TFT=7 is unknown",
            "Ptcl[0].TR.TFT=8 is unknown",
            "Ptcl[1].TC1.TFT=-2 is unknown",
        ] {
            assert!(
                file.warnings.iter().any(|warning| warning.contains(needle)),
                "missing {needle:?} in {:?}",
                file.warnings
            );
        }
        for layer in ["TC1", "TC2", "TC4", "TD", "TP"] {
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(&format!("Ptcl[0].{layer}.TFT")))
            );
        }
        for index in [1, 2, 3] {
            for layer in ["TN", "TR"] {
                assert!(
                    !file
                        .warnings
                        .iter()
                        .any(|warning| warning.contains(&format!("Ptcl[{index}].{layer}.TFT")))
                );
            }
        }
    }

    #[test]
    fn special_particle_data_preserves_typed_fields_and_opaque_bytes() {
        let fields = vec![
            i32_block("PrtC", 32),
            block("PIFU", &0.75_f32.to_le_bytes()),
            curve("Ang", vec![key_bytes(0, 1, 0.0, 0.0, 1.5)], 0),
            curve("Len", vec![key_bytes(0, 1, 0.0, 0.0, 3.0)], 0),
            curve3(
                "FrRt",
                Some(vec![key_bytes(0, 1, 0.0, 0.0, 0.25)]),
                None,
                None,
            ),
            color_curve("CEI", vec![key_bytes(0, 1, 0.1, 0.2, 0.3)], None, None),
            container(
                "CEO",
                vec![curve("A", vec![key_bytes(0, 1, 0.0, 0.0, 0.5)], 0)],
            ),
            block("Blob", &[0xff; 13]),
            i32_block("Dupe", 1),
            i32_block("Dupe", 2),
        ];
        for raw_type in [0, 3, 4, 6, 7, 9, 10, 11, 12, 14, 15, 999] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", raw_type),
                        container("Data", fields.clone()),
                    ],
                )],
            ))
            .unwrap();
            let Some(data) = file.particles[0].data.generic_data() else {
                panic!("type {raw_type} must retain its Data block");
            };
            assert_eq!(data.raw_payload, fields.concat());
            assert_eq!(data.scalars["PrtC"], 32);
            assert_eq!(f32::from_bits(data.scalars["PIFU"] as u32), 0.75);
            assert_eq!(data.curves["Ang"].value(0.0, 0.0), 1.5);
            assert_eq!(data.curves["Len"].value(0.0, 0.0), 3.0);
            assert_eq!(data.curve3s["FrRt"].evaluate(0.0, 0.0), [0.25, 0.0, 0.0]);
            assert_eq!(data.color_curves["CEI"].rgba(0.0), [0.1, 0.2, 0.3, 1.0]);
            assert_eq!(data.color_curves["CEO"].rgba(0.0)[3], 0.5);
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("quad approximation"))
            );
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("model-surface render resources")),
                raw_type == 14
            );
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("no client particle-object allocation")),
                matches!(raw_type, 0 | 7)
            );
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning
                        .contains("Dissolve targets client model render resources")),
                raw_type == 15
            );
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("PrVT=999 is unknown")),
                raw_type == 999
            );
        }
    }

    #[test]
    fn line_laser_and_polyline_data_have_typed_fields_without_losing_source() {
        let parse = |raw_type, fields: Vec<Vec<u8>>| {
            AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![u32_block("PrVT", raw_type), container("Data", fields)],
                )],
            ))
            .unwrap()
        };
        let scalar_curve = |name, value| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, value)], 0);
        let rgba = |name, value| {
            color_curve(
                name,
                vec![key_bytes(0, 1, value, value + 0.1, value + 0.2)],
                Some(vec![key_bytes(0, 1, 0.0, 0.0, value + 0.3)]),
                None,
            )
        };

        let line = parse(
            3,
            vec![
                i32_block("LnCT", 7),
                scalar_curve("Len", 2.0),
                scalar_curve("LenR", 0.25),
                rgba("ColB", 0.1),
                rgba("ColE", 0.5),
                block("Blob", &[0xaa; 5]),
            ],
        );
        let AvfxParticleData::Line(line) = &line.particles[0].data else {
            panic!("Line Data must be typed");
        };
        assert_eq!(line.line_count, 7);
        assert_eq!(line.length.value(0.0, 0.0), 2.0);
        assert_eq!(line.length_random.value(0.0, 0.0), 0.25);
        assert_eq!(line.color_begin.rgba(0.0), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(line.color_end.rgba(0.0), [0.5, 0.6, 0.7, 0.8]);
        assert!(
            line.source
                .raw_payload
                .ends_with(&block("Blob", &[0xaa; 5]))
        );

        let laser = parse(
            4,
            vec![
                scalar_curve("Len", 3.0),
                scalar_curve("LenR", 0.5),
                scalar_curve("Wdt", 1.5),
                scalar_curve("WdtR", 0.75),
                i32_block("Unkn", 91),
            ],
        );
        let AvfxParticleData::Laser(laser) = &laser.particles[0].data else {
            panic!("Laser Data must be typed");
        };
        assert_eq!(
            [
                laser.length.value(0.0, 0.0),
                laser.length_random.value(0.0, 0.0),
                laser.width.value(0.0, 0.0),
                laser.width_random.value(0.0, 0.0),
            ],
            [3.0, 0.5, 1.5, 0.75]
        );
        assert_eq!(laser.source.scalars["Unkn"], 91);

        let curve_names = [
            "CF", "CFR", "Wd", "WdR", "WdB", "WdBR", "WdC", "WdCR", "WdE", "WdER", "Len", "LenR",
            "Sft", "SftR", "PnDs",
        ];
        let mut fields = vec![
            i32_block("LnCT", 2),
            i32_block("NBBA", 3),
            i32_block("BWpT", 4),
            i32_block("PnC", 5),
            i32_block("PnCC", 6),
            i32_block("PnED", 7),
            i32_block("bEdg", 1),
            i32_block("bNtB", 1),
            i32_block("BdWp", 1),
            i32_block("bCtg", 1),
            i32_block("bCtr", 1),
            i32_block("TagN", 8),
            i32_block("bSpl", 1),
            i32_block("bLcl", 1),
        ];
        fields.extend(
            curve_names
                .iter()
                .enumerate()
                .map(|(index, name)| scalar_curve(name, index as f32 + 1.0)),
        );
        for (index, name) in ["ColB", "ColC", "ColE", "CoEB", "CoEC", "CoEE"]
            .into_iter()
            .enumerate()
        {
            fields.push(rgba(name, index as f32 * 0.1));
        }
        fields.push(block("Opaque", &[1, 2, 3]));
        let polyline_file = parse(6, fields);
        assert!(
            !polyline_file
                .warnings
                .iter()
                .any(|warning| warning.contains("quad approximation"))
        );
        let AvfxParticleData::Polyline(polyline) = &polyline_file.particles[0].data else {
            panic!("Polyline Data must be typed");
        };
        assert_eq!(
            [
                polyline.create_line_type,
                polyline.not_billboard_base_axis_type,
                polyline.bind_weapon_type,
                polyline.point_count,
                polyline.point_count_center,
                polyline.point_count_end_distortion,
                polyline.tag_number,
            ],
            [2, 3, 4, 5, 6, 7, 8]
        );
        assert!(
            [
                polyline.use_edge,
                polyline.not_billboard,
                polyline.bind_weapon,
                polyline.connect_target,
                polyline.connect_target_reverse,
                polyline.is_spline,
                polyline.is_local,
            ]
            .into_iter()
            .all(|value| value)
        );
        assert_eq!(
            [
                &polyline.cf,
                &polyline.cf_random,
                &polyline.width,
                &polyline.width_random,
                &polyline.width_begin,
                &polyline.width_begin_random,
                &polyline.width_center,
                &polyline.width_center_random,
                &polyline.width_end,
                &polyline.width_end_random,
                &polyline.length,
                &polyline.length_random,
                &polyline.softness,
                &polyline.softness_random,
                &polyline.point_distortion,
            ]
            .map(|curve| curve.value(0.0, 0.0)),
            std::array::from_fn::<_, 15, _>(|index| index as f32 + 1.0)
        );
        assert_eq!(polyline.color_center.rgba(0.0), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(polyline.color_edge_end.rgba(0.0), [0.5, 0.6, 0.7, 0.8]);
        assert!(
            polyline
                .source
                .raw_payload
                .ends_with(&block("Opaque", &[1, 2, 3]))
        );
    }

    #[test]
    fn decal_data_has_typed_fields_without_losing_unknown_payload() {
        let parse = |raw_type, fields: Vec<Vec<u8>>| {
            AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![u32_block("PrVT", raw_type), container("Data", fields)],
                )],
            ))
            .unwrap()
        };
        let scalar_curve = |name, value| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, value)], 0);

        let decal = parse(
            10,
            vec![
                block("SS", &1.25_f32.to_le_bytes()),
                i32_block("DDTT", 3),
                block("Opaque", &[0xaa; 5]),
            ],
        );
        let AvfxParticleData::Decal(data) = &decal.particles[0].data else {
            panic!("Decal Data must be typed");
        };
        assert_eq!((data.scaling_scale, data.ddtt), (1.25, 3));
        assert!(
            data.source
                .raw_payload
                .ends_with(&block("Opaque", &[0xaa; 5]))
        );
        assert_eq!(f32::from_bits(data.source.scalars["SS"] as u32), 1.25);
        assert!(decal.warnings.iter().any(|warning| {
            warning
                == "Ptcl[0].Data.DDTT=3 uses the deferred material GBuffer path; forward preview skips the particle"
        }));

        let ring = parse(
            11,
            vec![
                scalar_curve("WID", 2.0),
                scalar_curve("WIDR", 0.5),
                block("SS", &0.75_f32.to_le_bytes()),
                block("RF", &0.25_f32.to_le_bytes()),
                i32_block("DDTT", 7),
                i32_block("Unkn", 91),
            ],
        );
        let AvfxParticleData::DecalRing(data) = &ring.particles[0].data else {
            panic!("DecalRing Data must be typed");
        };
        assert_eq!(data.width.value(0.0, 0.0), 2.0);
        assert_eq!(data.width_random.value(0.0, 0.0), 0.5);
        assert_eq!(
            (data.scaling_scale, data.ring_fan, data.ddtt),
            (0.75, 0.25, 7)
        );
        assert_eq!(data.source.scalars["Unkn"], 91);
        assert!(ring.warnings.iter().any(|warning| {
            warning == "Ptcl[0].Data.DDTT=7 is unsupported; particle is skipped"
        }));
    }

    #[test]
    fn disc_data_has_typed_fields_without_losing_unknown_payload() {
        let scalar_curve = |name, value| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, value)], 0);
        let rgba = |name, value| {
            color_curve(
                name,
                vec![key_bytes(0, 1, value, value + 0.1, value + 0.2)],
                Some(vec![key_bytes(0, 1, 0.0, 0.0, value + 0.3)]),
                None,
            )
        };
        let curve_names = [
            "Ang", "AngR", "HBI", "HBIR", "HEI", "HEIR", "HBO", "HBOR", "HEO", "HEOR", "WB", "WBR",
            "WE", "WER", "RB", "RBR", "RE", "RER",
        ];
        let mut fields = vec![
            i32_block("PrtC", 3),
            i32_block("PCnU", 4),
            i32_block("PCnV", 5),
            block("PIFU", &0.75_f32.to_le_bytes()),
            i32_block("SS", 25),
        ];
        fields.extend(
            curve_names
                .iter()
                .enumerate()
                .map(|(index, name)| scalar_curve(name, index as f32 + 1.0)),
        );
        fields.push(rgba("CEI", 0.1));
        fields.push(rgba("CEO", 0.5));
        fields.push(block("Opaque", &[0xaa; 5]));
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![u32_block("PrVT", 12), container("Data", fields)],
            )],
        ))
        .unwrap();
        let AvfxParticleData::Disc(data) = &file.particles[0].data else {
            panic!("Disc Data must be typed");
        };
        assert_eq!(
            (
                data.parts_count,
                data.parts_count_u,
                data.parts_count_v,
                data.point_interval_factor_v,
                data.scaling_scale,
            ),
            (3, 4, 5, 0.75, 25)
        );
        assert_eq!(
            [
                &data.angle,
                &data.angle_random,
                &data.height_begin_inner,
                &data.height_begin_inner_random,
                &data.height_end_inner,
                &data.height_end_inner_random,
                &data.height_begin_outer,
                &data.height_begin_outer_random,
                &data.height_end_outer,
                &data.height_end_outer_random,
                &data.width_begin,
                &data.width_begin_random,
                &data.width_end,
                &data.width_end_random,
                &data.radius_begin,
                &data.radius_begin_random,
                &data.radius_end,
                &data.radius_end_random,
            ]
            .map(|curve| curve.value(0.0, 0.0)),
            std::array::from_fn::<_, 18, _>(|index| index as f32 + 1.0)
        );
        assert_eq!(data.color_edge_inner.rgba(0.0), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(data.color_edge_outer.rgba(0.0), [0.5, 0.6, 0.7, 0.8]);
        assert!(
            data.source
                .raw_payload
                .ends_with(&block("Opaque", &[0xaa; 5]))
        );
    }

    #[test]
    fn polygon_data_has_typed_fields_without_losing_unknown_payload() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 9),
                    container(
                        "Data",
                        vec![
                            curve("Cnt", vec![key_bytes(0, 1, 0.0, 0.0, 6.0)], 0),
                            curve("CntR", vec![key_bytes(0, 1, 0.0, 0.0, 2.0)], 0),
                            block("Opaque", &[1, 2, 3]),
                        ],
                    ),
                ],
            )],
        ))
        .unwrap();
        let AvfxParticleData::Polygon(data) = &file.particles[0].data else {
            panic!("Polygon Data must be typed");
        };
        assert_eq!(data.count.value(0.0, 0.0), 6.0);
        assert_eq!(data.count_random.value(0.0, 0.0), 2.0);
        assert!(
            data.source
                .raw_payload
                .ends_with(&block("Opaque", &[1, 2, 3]))
        );
        assert!(
            !file
                .warnings
                .iter()
                .any(|warning| warning.contains("Polygon uses a dedicated client particle path"))
        );
    }

    #[test]
    fn model_skin_data_has_typed_fields_without_losing_unknown_payload() {
        let scalar_curve = |name, value| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, value)], 0);
        let rgba = |name, value| {
            color_curve(
                name,
                vec![key_bytes(0, 1, value, value + 0.1, value + 0.2)],
                Some(vec![key_bytes(0, 1, 0.0, 0.0, value + 0.3)]),
                None,
            )
        };
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 14),
                    container(
                        "Data",
                        vec![
                            i32_block("FrsT", 2),
                            i32_block("AuTT", 3),
                            i32_block("bCM", 4),
                            scalar_curve("FrC", 1.0),
                            scalar_curve("FrCR", 2.0),
                            curve3(
                                "FrRt",
                                Some(vec![key_bytes(0, 1, 0.0, 0.0, 3.0)]),
                                None,
                                None,
                            ),
                            rgba("ColB", 0.1),
                            rgba("ColE", 0.5),
                            scalar_curve("SEM", 4.0),
                            scalar_curve("SEMR", 5.0),
                            scalar_curve("EEM", 6.0),
                            scalar_curve("EEMR", 7.0),
                            curve3(
                                "UVPD",
                                None,
                                Some(vec![key_bytes(0, 1, 0.0, 0.0, 8.0)]),
                                None,
                            ),
                            block("Opaque", &[4, 5, 6]),
                        ],
                    ),
                ],
            )],
        ))
        .unwrap();
        let AvfxParticleData::ModelSkin(data) = &file.particles[0].data else {
            panic!("ModelSkin Data must be typed");
        };
        assert_eq!((data.fresnel_type, data.aura_target, data.cm), (2, 3, 4));
        assert_eq!(
            [
                &data.fresnel_curve,
                &data.fresnel_curve_random,
                &data.sem,
                &data.sem_random,
                &data.eem,
                &data.eem_random,
            ]
            .map(|curve| curve.value(0.0, 0.0)),
            [1.0, 2.0, 4.0, 5.0, 6.0, 7.0]
        );
        assert_eq!(data.fresnel_rotation.evaluate(0.0, 0.0), [3.0, 0.0, 0.0]);
        assert_eq!(data.uv_point_density.evaluate(0.0, 0.0), [0.0, 8.0, 0.0]);
        assert_eq!(data.color_begin.rgba(0.0), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(data.color_end.rgba(0.0), [0.5, 0.6, 0.7, 0.8]);
        assert!(
            data.source
                .raw_payload
                .ends_with(&block("Opaque", &[4, 5, 6]))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("model-surface render resources"))
        );
        assert!(
            !file
                .warnings
                .iter()
                .any(|warning| warning.contains("quad approximation"))
        );
    }

    #[test]
    fn dissolve_data_has_typed_nested_color_without_losing_unknown_payload() {
        let scalar_curve = |name, value| curve(name, vec![key_bytes(0, 1, 0.0, 0.0, value)], 0);
        let rgba = |name, value| {
            color_curve(
                name,
                vec![key_bytes(0, 1, value, value + 0.1, value + 0.2)],
                Some(vec![key_bytes(0, 1, 0.0, 0.0, value + 0.3)]),
                None,
            )
        };
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![container(
                "Ptcl",
                vec![
                    u32_block("PrVT", 15),
                    container(
                        "Data",
                        vec![
                            i32_block("bRev", 1),
                            i32_block("BST", 2),
                            i32_block("NPT", 3),
                            i32_block("DTT", 4),
                            scalar_curve("EroR", 1.0),
                            scalar_curve("EdW", 2.0),
                            container(
                                "EdC",
                                vec![
                                    rgba("StrC", 0.1),
                                    rgba("MidC", 0.3),
                                    rgba("EndC", 0.5),
                                    scalar_curve("SclR", 3.0),
                                    scalar_curve("SclG", 4.0),
                                    scalar_curve("SclB", 5.0),
                                    scalar_curve("Bri", 6.0),
                                ],
                            ),
                            scalar_curve("EdCW", 7.0),
                            scalar_curve("ECMP", 8.0),
                            scalar_curve("Int", 9.0),
                            block("Opaque", &[7, 8, 9]),
                        ],
                    ),
                ],
            )],
        ))
        .unwrap();
        let AvfxParticleData::Dissolve(data) = &file.particles[0].data else {
            panic!("Dissolve Data must be typed");
        };
        assert!(data.reverse);
        assert_eq!((data.bst, data.npt, data.dissolve_target), (2, 3, 4));
        assert_eq!(
            [
                &data.erosion_rate,
                &data.end_color_width,
                &data.color.scale_r,
                &data.color.scale_g,
                &data.color.scale_b,
                &data.color.brightness,
                &data.mid_color_width,
                &data.start_color_width,
                &data.intensity,
            ]
            .map(|curve| curve.value(0.0, 0.0)),
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        );
        assert_eq!(data.color.start.rgba(0.0), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(data.color.middle.rgba(0.0), [0.3, 0.4, 0.5, 0.6]);
        assert_eq!(data.color.end.rgba(0.0), [0.5, 0.6, 0.7, 0.8]);
        assert!(
            data.source
                .raw_payload
                .ends_with(&block("Opaque", &[7, 8, 9]))
        );
        assert!(
            file.warnings
                .iter()
                .any(|warning| warning.contains("Dissolve targets client model render resources"))
        );
    }

    #[test]
    fn disc_degenerate_grids_and_nonfinite_interval_are_diagnosed() {
        for (name, raw, invalid) in [
            ("PrtC", 256, true),
            ("PCnU", 257, true),
            ("PCnV", 0, true),
            ("PIFU", f32::NAN.to_bits() as i32, true),
            ("PIFU", 0.75_f32.to_bits() as i32, false),
            ("PCnU", 258, false),
            ("PCnV", 255, false),
        ] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 12),
                        container("Data", vec![i32_block(name, raw)]),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(
                file.warnings.iter().any(|warning| {
                    warning.starts_with(&format!("Ptcl[0].Data.{name}:"))
                        && warning.ends_with("particle is skipped")
                }),
                invalid,
                "{name}={raw}: {:?}",
                file.warnings
            );
        }
    }

    #[test]
    fn invalid_model_triangles_are_skipped_without_changing_model_slots() {
        let indexes: Vec<_> = [0_i16, 1, 2, -1, 1, 2, 0, 3, 2, 2, 1, 0]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect();
        let valid_model = container(
            "Modl",
            vec![block("VDrw", &[0; 36 * 3]), block("VIdx", &indexes)],
        );
        let bytes = container("AVFX", vec![container("Modl", vec![]), valid_model]);
        let file = AvfxFile::parse(&bytes).unwrap();
        assert_eq!(file.models.len(), 2);
        assert!(file.models[0].draw.is_none());
        assert_eq!(
            file.models[1].draw.as_ref().unwrap().indices,
            [0, 1, 2, 2, 1, 0]
        );
        assert_eq!(
            file.warnings,
            ["Modl[1].VIdx: skipped 2 triangles with invalid vertex indexes"]
        );
    }

    #[test]
    fn incomplete_model_records_and_missing_vertices_are_diagnosed() {
        for (name, stride) in [("VEmt", 28), ("VNum", 2), ("VDrw", 36), ("VIdx", 6)] {
            for remainder in 1..stride {
                let bytes = container(
                    "AVFX",
                    vec![container(
                        "Modl",
                        vec![block(name, &vec![0; stride + remainder])],
                    )],
                );
                let file = AvfxFile::parse(&bytes).unwrap();
                assert!(
                    file.warnings
                        .iter()
                        .any(|warning| warning.contains("trailing bytes"))
                );
                if name == "VIdx" {
                    assert!(
                        file.warnings
                            .iter()
                            .any(|warning| warning.contains("without VDrw"))
                    );
                }
            }
        }
    }

    #[test]
    fn declared_counts_are_checked_without_allocating_from_them() {
        let file = AvfxFile::parse(&container(
            "AVFX",
            vec![
                i32_block("MdCn", -1),
                i32_block("PrCn", i32::MAX),
                container("Emit", vec![i32_block("PrCn", 1), i32_block("EmCn", 2)]),
            ],
        ))
        .unwrap();
        assert!(file.models.is_empty());
        assert!(file.particles.is_empty());
        for field in ["AVFX.MdCn", "AVFX.PrCn", "Emit.PrCn", "Emit.EmCn"] {
            assert!(
                file.warnings
                    .iter()
                    .any(|warning| warning.starts_with(field)),
                "{field}"
            );
        }
    }

    #[test]
    fn evaluates_linear_step_and_repeat_curves() {
        let curve = AvfxCurve {
            pre_behavior: BEHAVIOR_CONST,
            post_behavior: BEHAVIOR_CONST,
            random_type: 0,
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                AvfxCurveKey {
                    time: 30,
                    interpolation: 2,
                    x: 0.0,
                    y: 0.0,
                    z: 3.0,
                },
                AvfxCurveKey {
                    time: 60,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 5.0,
                },
            ],
        };
        assert!((curve.evaluate(0.0)[2] - 1.0).abs() < 1.0e-6);
        // The right key selects the segment: 0..30 is Step, 30..60 is Linear.
        assert!((curve.evaluate(15.0)[2] - 1.0).abs() < 1.0e-6);
        assert!((curve.evaluate(30.0)[2] - 3.0).abs() < 1.0e-6);
        assert!((curve.evaluate(45.0)[2] - 4.0).abs() < 1.0e-6);
        assert_eq!(curve.color_at(15.0)[2], 1.0);
        assert_eq!(curve.color_at(30.0)[2], 3.0);
        assert_eq!(curve.color_at(45.0)[2], 4.0);
        assert!((curve.evaluate(70.0)[2] - 5.0).abs() < 1.0e-6);

        let mut repeat = curve.clone();
        repeat.post_behavior = BEHAVIOR_REPEAT;
        // 90 帧回绕到 30，取右键值 3。
        assert!((repeat.evaluate(90.0)[2] - 3.0).abs() < 1.0e-6);
    }

    #[test]
    fn evaluates_color_curve_linearly() {
        let curve = AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 1.0,
                    y: 0.5,
                    z: 0.25,
                },
                AvfxCurveKey {
                    time: 30,
                    interpolation: 0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            ..Default::default()
        };
        // 颜色键不做样条：插值类型 0 也按线性。
        let mid = curve.color_at(15.0);
        assert!((mid[0] - 0.5).abs() < 1.0e-6);
        assert!((mid[1] - 0.25).abs() < 1.0e-6);
        assert!((mid[2] - 0.125).abs() < 1.0e-6);
    }

    #[test]
    fn spline_curve_hits_keys_and_midpoints() {
        // Zero tangent weights give smoothstep, including a linear midpoint.
        let curve = AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                AvfxCurveKey {
                    time: 30,
                    interpolation: 0,
                    x: 0.0,
                    y: 0.0,
                    z: 6.0,
                },
            ],
            ..Default::default()
        };
        assert!((curve.evaluate(15.0)[2] - 3.0).abs() < 1.0e-4);
        assert!((curve.evaluate(7.5)[2] - 0.9375).abs() < 1.0e-6);
        assert!((curve.evaluate(30.0)[2] - 6.0).abs() < 1.0e-6);
    }

    fn spline_keys(points: &[(i16, f32)]) -> AvfxCurve {
        AvfxCurve {
            keys: points
                .iter()
                .map(|&(time, z)| AvfxCurveKey {
                    time,
                    z,
                    x: 0.0,
                    y: 0.0,
                    interpolation: AvfxCurveKey::INTERPOLATION_SPLINE,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn spline_uses_left_outgoing_and_right_incoming_tangents() {
        let mut curve = spline_keys(&[(0, 0.0), (8, 8.0)]);
        curve.keys[0].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        curve.keys[0].x = 1.0;
        curve.keys[0].y = 7.0;
        curve.keys[1].x = -3.0;
        assert_eq!(curve.value(2.0, 0.0), 2.375);
        assert_eq!(curve.value(4.0, 0.0), 5.0);
        curve.keys[1].y = 1.0;
        assert_eq!(curve.value(2.0, 0.0), 2.0);
        assert_eq!(curve.value(4.0, 0.0), 4.0);
    }

    #[test]
    fn spline_tangents_include_neighbor_values_and_uneven_key_spacing() {
        let mut curve = spline_keys(&[(0, 1.0), (2, 4.0), (5, 2.0), (9, 8.0)]);
        curve.keys[1].x = 2.0;
        curve.keys[1].y = 1.0;
        curve.keys[2].x = 1.0;
        // Segment tangents are -3/5 and 18/7, not just the endpoint delta.
        assert!((curve.value(3.5, 0.0) - 729.0 / 280.0).abs() < 1e-6);
    }

    #[test]
    fn spline_tangents_use_signed_byte_quantization() {
        let mut curve = spline_keys(&[(0, 0.0), (8, 8.0)]);
        curve.keys[0].x = 0.1;
        curve.keys[1].y = -0.1;
        assert!((curve.value(4.0, 0.0) - (4.0 + 2.0 / 15.0)).abs() < 1e-6);
        curve.keys[0].x = 9.0; // 135 wraps to signed byte -121.
        curve.keys[1].y = 0.0;
        assert!((curve.value(4.0, 0.0) - (4.0 - 121.0 / 15.0)).abs() < 1e-6);
        for invalid in [f32::NAN, f32::INFINITY, -f32::INFINITY, f32::MAX] {
            curve.keys[0].x = invalid;
            assert_eq!(curve.value(2.0, 0.0), 1.25);
        }
    }

    #[test]
    fn duplicate_curve_times_select_first_equal_then_last_preceding_key() {
        let mut curve = spline_keys(&[(0, 0.0), (2, 2.0), (2, 6.0), (4, 8.0)]);
        for key in &mut curve.keys {
            key.interpolation = AvfxCurveKey::INTERPOLATION_LINEAR;
        }
        for (time, expected) in [(1.5, 1.5), (2.0, 2.0), (2.5, 6.5), (4.0, 8.0)] {
            assert_eq!(curve.value(time, 0.0), expected);
            if time == 2.0 {
                // Native RGB binary search visits the zero-span middle pair.
                assert!(curve.color_at(time)[2].is_nan());
            } else {
                assert_eq!(curve.color_at(time)[2], expected);
            }
        }
    }

    #[test]
    fn scalar_and_rgb_key_times_use_distinct_packed_widths() {
        let mut curve = spline_keys(&[(16384, 1.0), (16392, 3.0)]);
        for key in &mut curve.keys {
            key.interpolation = AvfxCurveKey::INTERPOLATION_LINEAR;
        }
        assert_eq!(curve.value(4.0, 0.0), 2.0);
        assert_eq!(curve.color_at(4.0)[2], 1.0);
        assert_eq!(curve.keys[0].time, 16384);
        curve.keys[0].time = -1;
        curve.keys[0].z = 0.3;
        curve.keys[1].time = 0;
        curve.keys[1].z = 1.0;
        for time in [0.0, 20.0, 16382.0] {
            assert_eq!(curve.value(time, 0.0), 0.3);
        }
        assert_eq!(curve.value(16383.0, 0.0), 1.0);
        assert_eq!(curve.color_at(0.0)[2], 0.3);
        assert_eq!(curve.color_at(268_435_456.0)[2], 0.3);
        assert_eq!(curve.color_at(268_435_488.0)[2], 1.0);
        assert_eq!(curve.keys[0].time, -1);
    }

    #[test]
    fn scalar_add_repeats_the_curve_and_accumulates_endpoint_delta() {
        let mut curve = spline_keys(&[(10, 1.0), (20, 5.0), (30, 3.0)]);
        for behavior in [BEHAVIOR_ADD, BEHAVIOR_ADD + 4] {
            curve.pre_behavior = behavior;
            curve.post_behavior = behavior;
            for (time, expected) in [
                (-10.0, -1.0),
                (-7.5, -0.375),
                (5.0, 2.0),
                (10.0, 1.0),
                (12.5, 1.625),
                (30.0, 3.0),
                (32.5, 3.625),
                (45.0, 6.0),
                (50.0, 5.0),
                (72.5, 7.625),
            ] {
                assert_eq!(curve.value(time, 0.0), expected, "{behavior} at {time}");
            }
        }
        for key in &mut curve.keys {
            key.interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        }
        for (time, expected) in [(30.0, 3.0), (35.0, 3.0), (45.0, 7.0), (50.0, 5.0)] {
            assert_eq!(curve.value(time, 0.0), expected, "step at {time}");
        }
        curve.pre_behavior = BEHAVIOR_CONST;
        assert_eq!(curve.value(-7.5, 0.0), 1.0);
        curve.post_behavior = BEHAVIOR_CONST;
        assert_eq!(curve.value(45.0, 0.0), 3.0);
    }

    #[test]
    fn runtime_scalar_clock_uses_total_age_only_for_add_modes() {
        let mut curve = spline_keys(&[(0, 0.0), (10, 10.0)]);
        for key in &mut curve.keys {
            key.interpolation = AvfxCurveKey::INTERPOLATION_LINEAR;
        }
        curve.post_behavior = BEHAVIOR_REPEAT;
        assert_eq!(curve.value_at(5.0, 25.0, 0.0), 5.0);
        curve.post_behavior = BEHAVIOR_ADD;
        assert_eq!(curve.value_at(5.0, 25.0, 0.0), 25.0);
        curve.post_behavior = 3;
        assert_eq!(curve.value_at(5.0, 25.0, 0.0), 10.0);
    }

    #[test]
    fn repeat_uses_client_cycle_boundaries_for_scalar_and_rgb() {
        let mut curve = spline_keys(&[(10, 1.0), (30, 5.0)]);
        for key in &mut curve.keys {
            key.interpolation = AvfxCurveKey::INTERPOLATION_LINEAR;
            key.x = key.z;
            key.y = -key.z;
        }
        curve.pre_behavior = BEHAVIOR_REPEAT;
        curve.post_behavior = BEHAVIOR_REPEAT;
        for (time, expected) in [
            (-30.0, 5.0),
            (-10.0, 5.0),
            (10.0, 1.0),
            (30.0, 5.0),
            (50.0, 1.0),
        ] {
            assert_eq!(curve.value(time, 0.0), expected, "scalar at {time}");
        }
        for behavior in [BEHAVIOR_REPEAT, BEHAVIOR_ADD, BEHAVIOR_ADD + 4] {
            curve.pre_behavior = behavior;
            curve.post_behavior = behavior;
            for (time, expected) in [
                (-30.0, 5.0),
                (-10.0, 5.0),
                (0.0, 3.0),
                (10.0, 5.0),
                (30.0, 1.0),
                (40.0, 3.0),
                (50.0, 1.0),
            ] {
                assert_eq!(
                    curve.color_at(time),
                    [expected, -expected, expected],
                    "RGB {behavior} at {time}"
                );
            }
        }
        curve.keys.truncate(1);
        assert_eq!(curve.color_at(30.0), [1.0, -1.0, 1.0]);
        assert_eq!(curve.value(30.0, 0.0), 1.0);
    }

    #[test]
    fn entry_boundaries_do_not_depend_on_total_payload_divisibility() {
        let old = emitter_item()[..288].to_vec();
        let entries = vec![old; 26]; // 7488 is divisible by both 288 and 312.
        let bytes = container(
            "AVFX",
            vec![container(
                "Emit",
                vec![
                    container("ItPr", entries.clone()),
                    container("ItEm", [entries, vec![emitter_item()]].concat()),
                ],
            )],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        assert_eq!(file.emitters[0].particle_items.len(), 26);
        assert_eq!(file.emitters[0].emitter_items.len(), 1);
        assert!(
            file.emitters[0]
                .particle_items
                .iter()
                .all(|item| { item.enabled && item.target_index == 0 && item.create_count == 4 })
        );
    }

    #[test]
    fn timeline_entries_allow_optional_clip_index() {
        let bytes = container(
            "AVFX",
            vec![container(
                "TmLn",
                vec![container(
                    "Item",
                    vec![timeline_item(2, 30)[..84].to_vec(), timeline_item(7, 60)],
                )],
            )],
        );
        let file = AvfxFile::parse(&bytes).unwrap();
        let items = &file.timelines[0].items;
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].emitter_index, items[0].clip_index), (2, -1));
        assert_eq!((items[1].emitter_index, items[1].end_time), (7, 60));
    }

    #[test]
    fn curve_extrapolation_selects_each_side_with_distinct_scalar_and_rgb_endpoints() {
        let mut curve = AvfxCurve {
            pre_behavior: BEHAVIOR_CONST,
            post_behavior: BEHAVIOR_REPEAT,
            keys: vec![
                AvfxCurveKey {
                    time: 10,
                    interpolation: 1,
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                AvfxCurveKey {
                    time: 30,
                    interpolation: 1,
                    x: 5.0,
                    y: 6.0,
                    z: 7.0,
                },
            ],
            ..Default::default()
        };
        for (time, expected) in [(0.0, [1.0, 2.0, 3.0]), (40.0, [3.0, 4.0, 5.0])] {
            assert_eq!(curve.evaluate(time), expected);
            assert_eq!(curve.color_at(time), expected);
        }
        assert_eq!(curve.evaluate(30.0), [1.0, 2.0, 7.0]);
        assert_eq!(curve.color_at(30.0), [1.0, 2.0, 3.0]);
        curve.pre_behavior = BEHAVIOR_REPEAT;
        curve.post_behavior = BEHAVIOR_CONST;
        assert_eq!(curve.evaluate(0.0), [3.0, 4.0, 5.0]);
        assert_eq!(curve.color_at(0.0), [3.0, 4.0, 5.0]);
        assert_eq!(curve.evaluate(40.0), [5.0, 6.0, 7.0]);
    }

    #[test]
    fn rejects_bad_roots() {
        assert_eq!(
            AvfxFile::parse(&[]),
            Err(AvfxParseError::TooShort { len: 0 })
        );
        let mut bad = Vec::new();
        bad.extend_from_slice(b"XXXX");
        bad.extend_from_slice(&4_u32.to_le_bytes());
        assert!(matches!(
            AvfxFile::parse(&bad),
            Err(AvfxParseError::BadRootName { .. })
        ));
        let mut truncated = container("AVFX", vec![i32_block("Ver", 1)]);
        truncated.pop();
        assert!(matches!(
            AvfxFile::parse(&truncated),
            Err(AvfxParseError::TruncatedRoot { .. })
        ));
    }

    #[test]
    fn powder_external_point_source_is_reported_without_rewriting_the_type() {
        for raw_type in [0, 1, 2, 258] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 1),
                        u32_block("bSCt", 1),
                        container("Smpl", vec![i32_block("SIPT", raw_type)]),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(
                file.particles[0]
                    .simple
                    .as_ref()
                    .unwrap()
                    .injection_position_type,
                raw_type
            );
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("external point source")),
                raw_type as i8 == 2
            );
        }
    }

    #[test]
    fn powder_simple_preserves_nonpositive_uv_interval() {
        for interval in [0, -1, 2] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 1),
                        u32_block("bSCt", 1),
                        container("Smpl", vec![i32_block("UvIv", interval)]),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(
                file.particles[0].simple.as_ref().unwrap().uv_interval,
                interval
            );
        }
    }

    #[test]
    fn powder_missing_enabled_simple_payload_is_diagnosed() {
        for (enabled, payload, warn) in [
            (true, false, true),
            (false, false, false),
            (true, true, false),
        ] {
            let mut fields = vec![u32_block("PrVT", 1), u32_block("bSCt", u32::from(enabled))];
            if payload {
                fields.push(container("Smpl", vec![]));
            }
            let file =
                AvfxFile::parse(&container("AVFX", vec![container("Ptcl", fields)])).unwrap();
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("Smpl is missing")),
                warn
            );
        }
    }

    #[test]
    fn line_simple_diagnostics_match_the_supported_unbound_path() {
        for (simple_fields, expected) in [
            (Some(vec![]), None),
            (
                Some(vec![i32_block("IJMN", 0)]),
                Some("injection/binding configuration is unsupported"),
            ),
            (
                Some(vec![u32_block("bBnP", 1)]),
                Some("injection/binding configuration is unsupported"),
            ),
            (None, Some("Smpl is missing")),
        ] {
            let mut fields = vec![
                u32_block("PrVT", 3),
                u32_block("bSCt", 1),
                container("Data", vec![]),
            ];
            if let Some(simple_fields) = simple_fields {
                fields.push(container("Smpl", simple_fields));
            }
            let file =
                AvfxFile::parse(&container("AVFX", vec![container("Ptcl", fields)])).unwrap();
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| expected.is_some_and(|text| warning.contains(text))),
                expected.is_some()
            );
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("quad approximation"))
            );
        }
    }

    #[test]
    fn powder_single_ignores_rotation_base_without_fallback_warning() {
        for mode in [0, 1, 4, 7, 8, 99] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 1),
                        u32_block("bSCt", 0),
                        i32_block("RBDT", mode),
                    ],
                )],
            ))
            .unwrap();
            assert_eq!(file.particles[0].rotation_direction_base, mode);
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("RBDT="))
            );
        }
    }

    #[test]
    fn preserves_and_diagnoses_unsupported_draw_modes() {
        for draw_mode in (0..=12).chain([-1, 13, 1000]) {
            let bytes = container(
                "AVFX",
                vec![container("Ptcl", vec![i32_block("RMT", draw_mode)])],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.particles[0].draw_mode, draw_mode);
            assert_eq!(
                file.warnings.iter().any(|warning| warning.contains("RMT=")),
                matches!(draw_mode, -1 | 13 | 1000),
            );
        }
    }

    #[test]
    fn preserves_and_diagnoses_unsupported_facing_modes() {
        for mode in [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1000] {
            let bytes = container(
                "AVFX",
                vec![container("Ptcl", vec![i32_block("RBDT", mode)])],
            );
            let file = AvfxFile::parse(&bytes).unwrap();
            assert_eq!(file.particles[0].rotation_direction_base, mode);
            assert_eq!(
                file.warnings
                    .iter()
                    .any(|warning| warning.contains("RBDT=")),
                !matches!(mode, 0 | 1 | 2 | 4 | 5 | 6 | 8 | 9 | 10),
            );
        }
    }

    #[test]
    fn ordinary_particles_accept_movement_direction_facing_modes() {
        for particle_type in [8, 5, 13] {
            for mode in [
                rotation_direction_base::MOVE_DIRECTION,
                rotation_direction_base::MOVE_DIRECTION_BILLBOARD,
            ] {
                let file = AvfxFile::parse(&container(
                    "AVFX",
                    vec![container(
                        "Ptcl",
                        vec![u32_block("PrVT", particle_type), i32_block("RBDT", mode)],
                    )],
                ))
                .unwrap();
                assert!(
                    !file
                        .warnings
                        .iter()
                        .any(|warning| warning.contains("RBDT=")),
                    "PrVT={particle_type} mode={mode}: {:?}",
                    file.warnings
                );
            }
        }
    }

    #[test]
    fn laser_skips_unsupported_axis_modes_without_quad_fallback() {
        for mode in [-1, 0, 1, 2, 3, 10] {
            let file = AvfxFile::parse(&container(
                "AVFX",
                vec![container(
                    "Ptcl",
                    vec![
                        u32_block("PrVT", 4),
                        i32_block("RBDT", mode),
                        container("Data", vec![]),
                    ],
                )],
            ))
            .unwrap();
            let skipped = file
                .warnings
                .iter()
                .any(|warning| warning.contains("Laser RBDT="));
            let facing = file
                .warnings
                .iter()
                .any(|warning| warning.starts_with("particle RBDT="));
            assert_eq!(skipped, !matches!(mode, 0..=2), "mode {mode}");
            assert_eq!(facing, matches!(mode, -1 | 3), "mode {mode}");
            assert!(
                !file
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("quad approximation"))
            );
        }
    }

    #[test]
    fn child_walker_handles_invalid_names_and_oversized_lengths() {
        let mut payload = vec![0; 8 * 100_000];
        payload.extend(i32_block("Ver", 7));
        let root = block("AVFX", &payload);
        assert_eq!(AvfxFile::parse(&root).unwrap().version, 7);

        let mut payload = block("Ver", &[]);
        payload[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(child_blocks(&payload).next().is_none());
        for size in 0..8 {
            assert!(child_blocks(&payload[..size]).next().is_none());
        }
    }

    fn aura_test_texture(base: u8, mip: u8) -> VfxTextureRgba {
        VfxTextureRgba {
            width: 2,
            height: 2,
            rgba: vec![base; 16],
            mips: vec![VfxTextureMipRgba {
                width: 1,
                height: 1,
                rgba: vec![mip; 4],
            }],
            source_mip_count: 2,
            ..Default::default()
        }
    }

    #[test]
    fn aura_array_packs_only_enabled_slots_in_mip_major_order() {
        let tc2 = aura_test_texture(10, 11);
        let td = aura_test_texture(30, 31);
        let packed = VfxAuraTextureArrayRgba::pack([Some(&tc2), None, Some(&td)]).unwrap();
        assert_eq!(
            (packed.width, packed.height, packed.layer_count()),
            (2, 2, 2)
        );
        assert_eq!(packed.slot_layers, [Some(0), None, Some(1)]);
        assert_eq!(packed.mips.len(), 2);
        assert_eq!((packed.mips[0].width, packed.mips[0].height), (2, 2));
        assert_eq!(packed.mips[0].rgba, [[10; 16], [30; 16]].concat());
        assert_eq!((packed.mips[1].width, packed.mips[1].height), (1, 1));
        assert_eq!(packed.mips[1].rgba, [[11; 4], [31; 4]].concat());
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([None, None, None]),
            Err(VfxAuraTextureArrayError::NoLayers)
        );
    }

    #[test]
    fn aura_array_rejects_incomplete_and_malformed_mips() {
        let valid = aura_test_texture(10, 11);
        let mut invalid = valid.clone();
        invalid.source_mip_count = 3;
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&invalid), None, None]),
            Err(VfxAuraTextureArrayError::IncompleteSourceMips { slot: 0 })
        );
        invalid = valid.clone();
        invalid.rgba.pop();
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&valid), Some(&invalid), None]),
            Err(VfxAuraTextureArrayError::InvalidMip { slot: 1, level: 0 })
        );
        invalid = valid.clone();
        invalid.mips[0].width = 2;
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&invalid), None, None]),
            Err(VfxAuraTextureArrayError::InvalidMip { slot: 0, level: 1 })
        );
    }

    #[test]
    fn aura_array_rejects_incompatible_or_non_rgba8_inputs() {
        let valid = aura_test_texture(10, 11);
        let mut incompatible = valid.clone();
        incompatible.mips.clear();
        incompatible.source_mip_count = 1;
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&valid), Some(&incompatible), None]),
            Err(VfxAuraTextureArrayError::IncompatibleLayout { slot: 1 })
        );
        incompatible = valid.clone();
        incompatible.width = 4;
        incompatible.rgba.resize(32, 12);
        incompatible.mips[0].width = 2;
        incompatible.mips[0].rgba.resize(8, 13);
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&valid), Some(&incompatible), None]),
            Err(VfxAuraTextureArrayError::IncompatibleLayout { slot: 1 })
        );
        let mut cube = valid.clone();
        cube.is_cube = true;
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&cube), None, None]),
            Err(VfxAuraTextureArrayError::UnsupportedFormat { slot: 0 })
        );
        let mut hdr = valid;
        hdr.rgba16f_mips.push(VfxTextureMipRgba16f::default());
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&hdr), None, None]),
            Err(VfxAuraTextureArrayError::UnsupportedFormat { slot: 0 })
        );
        let mut array = aura_test_texture(10, 11);
        array.source_type = VfxTextureSourceType::D2Array;
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&array), None, None]),
            Err(VfxAuraTextureArrayError::UnsupportedFormat { slot: 0 })
        );
    }

    #[test]
    fn aura_array_selects_model_skin_texture_slots_by_file_index() {
        let textures = vec![
            Some(aura_test_texture(30, 31)),
            None,
            Some(aura_test_texture(10, 11)),
        ];
        let mut particle = AvfxParticle {
            particle_type: Some(ParticleType::ModelSkin),
            data: AvfxParticleData::ModelSkin(Default::default()),
            texture_color2: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 2,
                ..Default::default()
            }),
            texture_color3: Some(AvfxParticleTexture {
                enabled: false,
                texture_index: 99,
                ..Default::default()
            }),
            texture_distortion: Some(AvfxParticleDistortion {
                enabled: true,
                texture_index: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let packed = VfxAuraTextureArrayRgba::from_model_skin_particle(&particle, &textures)
            .expect("file-indexed Aura sources");
        assert_eq!(packed.slot_layers, [Some(0), None, Some(1)]);
        assert_eq!(packed.mips[0].rgba, [[10; 16], [30; 16]].concat());

        particle.texture_color2.as_mut().unwrap().texture_index = 1;
        assert_eq!(
            VfxAuraTextureArrayRgba::from_model_skin_particle(&particle, &textures),
            Err(VfxAuraTextureArrayError::MissingTexture { slot: 0, index: 1 })
        );
        particle.texture_color2.as_mut().unwrap().texture_index = 2;
        particle.texture_color2.as_mut().unwrap().use_screen_copy = true;
        assert_eq!(
            VfxAuraTextureArrayRgba::from_model_skin_particle(&particle, &textures),
            Err(VfxAuraTextureArrayError::UnsupportedSource { slot: 0 })
        );
        particle.data = AvfxParticleData::None;
        assert_eq!(
            VfxAuraTextureArrayRgba::from_model_skin_particle(&particle, &textures),
            Err(VfxAuraTextureArrayError::NotModelSkin)
        );
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn truncated_atex_wrapper_is_rejected() {
        for len in 4..8 {
            let mut bytes = b"atex".to_vec();
            bytes.resize(len, 0);
            assert!(decode_atex_rgba(&bytes).is_none());
        }
    }

    #[cfg(feature = "game-data")]
    fn synthetic_tex(
        format: u32,
        width: u16,
        height: u16,
        array_size: u8,
        levels: &[Vec<Vec<u8>>],
    ) -> Vec<u8> {
        assert!(!levels.is_empty() && levels.len() <= 13);
        let mut tex = vec![0_u8; 80];
        let attribute = if array_size > 1 {
            0x1000_0000_u32
        } else {
            0x0080_0000
        };
        tex[0..4].copy_from_slice(&attribute.to_le_bytes());
        tex[4..8].copy_from_slice(&format.to_le_bytes());
        tex[8..10].copy_from_slice(&width.to_le_bytes());
        tex[10..12].copy_from_slice(&height.to_le_bytes());
        tex[12..14].copy_from_slice(&1_u16.to_le_bytes());
        tex[14] = levels.len() as u8;
        tex[15] = array_size;
        let mut offset = 80_u32;
        for (index, slices) in levels.iter().enumerate() {
            assert_eq!(slices.len(), usize::from(array_size.max(1)));
            tex[28 + index * 4..32 + index * 4].copy_from_slice(&offset.to_le_bytes());
            for slice in slices {
                offset += slice.len() as u32;
            }
        }
        for slices in levels {
            for slice in slices {
                tex.extend_from_slice(slice);
            }
        }
        let mut atex = b"atex\0\0\0\0".to_vec();
        atex.extend_from_slice(&tex);
        atex
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_source_authored_mips() {
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x1131,
            4,
            4,
            1,
            &[vec![vec![10; 16]], vec![vec![20; 4]], vec![vec![30; 1]]],
        ))
        .expect("decode A8 mip chain");
        assert_eq!((decoded.width, decoded.height), (4, 4));
        assert_eq!(decoded.source_mip_count, 3);
        assert_eq!(decoded.rgba, [10; 64]);
        assert_eq!(decoded.mips.len(), 2);
        assert_eq!((decoded.mips[0].width, decoded.mips[0].height), (2, 2));
        assert_eq!(decoded.mips[0].rgba, [20; 16]);
        assert_eq!((decoded.mips[1].width, decoded.mips[1].height), (1, 1));
        assert_eq!(decoded.mips[1].rgba, [30; 4]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_takes_array_slice_zero_at_each_mip() {
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x1131,
            2,
            2,
            2,
            &[vec![vec![1; 4], vec![9; 4]], vec![vec![2; 1], vec![8; 1]]],
        ))
        .expect("decode A8 array mip chain");
        assert_eq!(decoded.source_type, VfxTextureSourceType::D2Array);
        assert_eq!(decoded.rgba, [1; 16]);
        assert_eq!(decoded.mips.len(), 1);
        assert_eq!(decoded.mips[0].rgba, [2; 4]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn aura_array_rejects_atex_volume_sources() {
        let mut bytes = synthetic_tex(0x1131, 2, 2, 1, &[vec![vec![10; 4]]]);
        bytes[8..12].copy_from_slice(&0x0100_0000_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("decode first volume slice");
        assert_eq!(decoded.source_type, VfxTextureSourceType::Other);
        assert_eq!(
            VfxAuraTextureArrayRgba::pack([Some(&decoded), None, None]),
            Err(VfxAuraTextureArrayError::UnsupportedFormat { slot: 0 })
        );
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_all_cube_faces_at_each_mip() {
        let mut bytes = synthetic_tex(
            0x1131,
            2,
            2,
            6,
            &[
                (1..=6).map(|value| vec![value; 4]).collect(),
                (11..=16).map(|value| vec![value]).collect(),
            ],
        );
        bytes[8..12].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("decode A8 cube mip chain");
        assert!(decoded.is_cube);
        assert_eq!(decoded.source_type, VfxTextureSourceType::Cube);
        assert!(decoded.has_complete_source_mips());
        assert_eq!(decoded.decoded_mip_count(), 2);
        assert_eq!(decoded.rgba, [1; 16]);
        assert_eq!(decoded.mips[0].rgba, [11; 4]);
        for (index, face) in decoded.cube_mips[0].faces.iter().enumerate() {
            assert_eq!(face, &vec![(index + 1) as u8; 16]);
        }
        for (index, face) in decoded.cube_mips[1].faces.iter().enumerate() {
            assert_eq!(face, &vec![(index + 11) as u8; 4]);
        }
        bytes[8..12].copy_from_slice(&0x1200_0000_u32.to_le_bytes());
        let mixed_flags = decode_atex_rgba(&bytes).expect("cube with array flag");
        assert_eq!(mixed_flags.source_type, VfxTextureSourceType::Cube);
        assert_eq!(mixed_flags.cube_mips.len(), 2);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_block_compressed_cube_faces() {
        let bc4 = |value| vec![value, value, 0, 0, 0, 0, 0, 0];
        let mut bytes = synthetic_tex(
            0x6120,
            4,
            4,
            6,
            &[
                (1..=6).map(|value| bc4(value * 10)).collect(),
                (1..=6).map(|value| bc4(value * 10 + 5)).collect(),
            ],
        );
        bytes[8..12].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("decode BC4 cube mip chain");
        assert!(decoded.has_complete_source_mips());
        for (index, face) in decoded.cube_mips[0].faces.iter().enumerate() {
            assert_eq!(face, &vec![((index + 1) * 10) as u8; 64]);
        }
        for (index, face) in decoded.cube_mips[1].faces.iter().enumerate() {
            assert_eq!(face, &vec![((index + 1) * 10 + 5) as u8; 16]);
        }
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_bc6h_cube_as_half_float_faces() {
        let mut bytes = synthetic_tex(
            0x6330,
            4,
            4,
            6,
            &[
                (0..6).map(|_| vec![0; 16]).collect(),
                (0..6).map(|_| vec![0; 16]).collect(),
            ],
        );
        bytes[8..12].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("decode BC6H cube mip chain");
        assert_eq!(decoded.cube_format, VfxTextureCubeFormat::Rgba16Float);
        assert!(decoded.has_complete_source_mips());
        assert_eq!(decoded.decoded_mip_count(), 2);
        assert_eq!(decoded.rgba.len(), 4 * 4 * 4);
        assert_eq!(decoded.rgba16f_mips.len(), 2);
        assert_eq!(decoded.cube_mips[0].width, 4);
        assert_eq!(decoded.cube_mips[1].width, 2);
        for ((level, face_zero), size) in decoded
            .cube_mips
            .iter()
            .zip(&decoded.rgba16f_mips)
            .zip([4, 2])
        {
            assert_eq!(face_zero.rgba16f, level.faces[0]);
            for face in &level.faces {
                assert_eq!(face.len(), size * size * 8);
                assert!(face.chunks_exact(8).all(|pixel| pixel[6..8] == [0, 0x3c]));
            }
        }

        bytes.pop();
        let truncated = decode_atex_rgba(&bytes).expect("retain complete BC6H base cube");
        assert_eq!(truncated.decoded_mip_count(), 1);
        assert!(!truncated.has_complete_source_mips());
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_bc6h_2d_mips_as_half_float() {
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x6330,
            4,
            4,
            1,
            &[vec![vec![0; 16]], vec![vec![0; 16]]],
        ))
        .expect("decode BC6H 2D mip chain");
        assert!(!decoded.is_cube);
        assert!(decoded.cube_mips.is_empty());
        assert!(decoded.has_complete_source_mips());
        assert_eq!(decoded.rgba16f_mips.len(), 2);
        assert_eq!(decoded.rgba16f_mips[0].rgba16f.len(), 4 * 4 * 8);
        assert_eq!(decoded.rgba16f_mips[1].rgba16f.len(), 2 * 2 * 8);
        assert_eq!(decoded.rgba16f_mips[1].rgba16f[6..8], [0, 0x3c]);
        assert_eq!(decoded.rgba.len(), 4 * 4 * 4);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_uncompressed_half_float_2d_mips() {
        let pixel = [0x4000_u16, 0xb800, 0x3800, 0x3c00]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x2460,
            2,
            2,
            1,
            &[vec![pixel.repeat(4)], vec![pixel.clone()]],
        ))
        .expect("decode RGBA16F 2D mip chain");
        assert!(!decoded.is_cube);
        assert!(decoded.has_complete_source_mips());
        assert_eq!(decoded.rgba16f_mips.len(), 2);
        assert_eq!(decoded.rgba16f_mips[0].rgba16f, pixel.repeat(4));
        assert_eq!(decoded.rgba16f_mips[1].rgba16f, pixel);
        assert_eq!(decoded.rgba[0..4], [255, 0, 128, 255]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_uncompressed_half_float_cube_faces() {
        let faces = [0x3800_u16, 0x3c00, 0x4000, 0x4200, 0xb800, 0]
            .into_iter()
            .map(|red| {
                [red, 0, 0, 0x3c00]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut bytes = synthetic_tex(0x2460, 1, 1, 6, &[faces.clone()]);
        bytes[8..12].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("decode RGBA16F cube");
        assert_eq!(decoded.cube_format, VfxTextureCubeFormat::Rgba16Float);
        assert!(decoded.has_complete_source_mips());
        assert_eq!(decoded.cube_mips[0].faces.as_slice(), faces);
        assert_eq!(decoded.rgba16f_mips[0].rgba16f, faces[0]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_does_not_expose_incomplete_cube_mips() {
        let mut bytes = synthetic_tex(
            0x1131,
            2,
            2,
            6,
            &[
                (1..=6).map(|value| vec![value; 4]).collect(),
                (11..=16).map(|value| vec![value]).collect(),
            ],
        );
        bytes[8..12].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        bytes.pop();
        let decoded = decode_atex_rgba(&bytes).expect("retain complete base cube");
        assert_eq!(decoded.decoded_mip_count(), 1);
        assert!(!decoded.has_complete_source_mips());
        assert_eq!(decoded.cube_mips[0].faces[5], [6; 16]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_preserves_block_compressed_mips() {
        let bc4 = |value| vec![value, value, 0, 0, 0, 0, 0, 0];
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x6120,
            4,
            4,
            1,
            &[vec![bc4(40)], vec![bc4(90)]],
        ))
        .expect("decode BC4 mip chain");
        assert_eq!(decoded.rgba, [40; 64]);
        assert_eq!(decoded.mips.len(), 1);
        assert_eq!((decoded.mips[0].width, decoded.mips[0].height), (2, 2));
        assert_eq!(decoded.mips[0].rgba, [90; 16]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_reports_truncated_source_mip_chain() {
        let mut bytes = synthetic_tex(0x1131, 2, 2, 1, &[vec![vec![1; 4]], vec![vec![2; 1]]]);
        bytes.pop();
        let decoded = decode_atex_rgba(&bytes).expect("base level remains decodable");
        assert_eq!(decoded.source_mip_count, 2);
        assert!(decoded.mips.is_empty());
        assert_eq!(decoded.rgba, [1; 16]);
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_reports_levels_beyond_the_dimension_limit() {
        let decoded = decode_atex_rgba(&synthetic_tex(
            0x1131,
            1,
            1,
            1,
            &[vec![vec![1]], vec![vec![2]]],
        ))
        .expect("base level remains decodable");
        assert_eq!(decoded.source_mip_count, 2);
        assert_eq!(decoded.decoded_mip_count(), 1);
        assert!(!decoded.has_complete_source_mips());
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn atex_decoder_keeps_base_when_a_later_surface_offset_is_invalid() {
        let mut bytes = synthetic_tex(0x1131, 2, 2, 1, &[vec![vec![3; 4]], vec![vec![4]]]);
        bytes[40..44].copy_from_slice(&79_u32.to_le_bytes());
        let decoded = decode_atex_rgba(&bytes).expect("base level remains decodable");
        assert_eq!(decoded.rgba, [3; 16]);
        assert_eq!(decoded.source_mip_count, 2);
        assert_eq!(decoded.decoded_mip_count(), 1);
    }
}

/// 武器/配件 MDL 的特效绑点（ElementId 表项）：avfx Binder 的 `BPID`
/// 引用 `id`（如武器的 3=基部 / 4=中部 / 5=尖部）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBindPoint {
    pub id: u32,
    /// MDL ElementId parent bone resolved from its string-table offset.
    pub parent_bone: Option<String>,
    pub translate: [f32; 3],
    pub rotate: [f32; 3],
}

impl VfxBindPoint {
    /// ElementId offset in its parent bone's coordinates. The client creates
    /// Rx, then applies Ry and Rz (column-vector matrix Rz * Ry * Rx).
    pub fn local_matrix(&self) -> [f32; 16] {
        let (sx, cx) = self.rotate[0].sin_cos();
        let (sy, cy) = self.rotate[1].sin_cos();
        let (sz, cz) = self.rotate[2].sin_cos();
        [
            cz * cy,
            sz * cy,
            -sy,
            0.0,
            cz * sy * sx - sz * cx,
            sz * sy * sx + cz * cx,
            cy * sx,
            0.0,
            cz * sy * cx + sz * sx,
            sz * sy * cx - cz * sx,
            cy * cx,
            0.0,
            self.translate[0],
            self.translate[1],
            self.translate[2],
            1.0,
        ]
    }
}

/// Resolve ElementId targets for a single skeleton pose. Results match `points`
/// order and use the skeleton's model coordinates; preview mount offsets are
/// applied separately. These are bone world matrices, not skinning matrices
/// (world * inverse bind). Preserve the complete affine basis, including shear.
/// An unresolved parent uses bone 0, as in the client's attach cache; without
/// any skeleton bones the local matrix remains the preview fallback.
pub fn vfx_bind_point_matrices(
    points: &[VfxBindPoint],
    skeleton: &crate::skeleton::ModelSkeleton,
    pose: &crate::skeleton::SkeletonPose,
) -> Vec<[f32; 16]> {
    let world = crate::skeleton::world_matrices(skeleton, pose);
    points
        .iter()
        .map(|point| {
            let parent = point
                .parent_bone
                .as_deref()
                .and_then(|name| skeleton.bone_index(name))
                .unwrap_or(0);
            world.get(parent).map_or_else(
                || point.local_matrix(),
                |matrix| crate::skeleton::mat4_mul(*matrix, point.local_matrix()),
            )
        })
        .collect()
}

/// 武器挂载的常驻 VFX 数据：解析后的 avfx 子集 + 解码好的颜色贴图。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponVfxData {
    /// 命中的 avfx 资源路径（`chara/weapon/.../vfx/eff/vw####.avfx`）。
    pub avfx_path: String,
    /// IMC 里的 VfxId。
    pub vfx_id: u8,
    /// 目标武器 MDL 的绑点表（ElementId id → 模型空间偏移）。
    /// avfx Binder 的 `BPID` 经它解析成位置；空表时全部按原点。
    pub bind_points: Vec<VfxBindPoint>,
    /// This MDL owner's base skeleton, loaded from game resources. Transient
    /// pose data is not part of serialized asset snapshots.
    #[serde(skip)]
    pub skeleton: Option<crate::skeleton::ModelSkeleton>,
    pub file: AvfxFile,
    /// 按文件 `Tex` 块顺序解码的贴图（RGBA8，数组贴图取第 0 层，cube 保留六面）；解码失败的项以
    /// `diagnostics` 记录、此处缺位（渲染端按索引回退程序化贴图）。
    pub textures: Vec<Option<VfxTextureRgba>>,
    pub diagnostics: Vec<String>,
}

impl WeaponVfxData {
    /// Resolve raw ElementId matrices in the owning model's coordinates. The
    /// Binder applies bFTO/query scaling later; do not normalize here or use
    /// skinning matrices. Preview layout translation is applied when drawing.
    pub fn binder_targets_for_pose(
        &self,
        pose: &crate::skeleton::SkeletonPose,
    ) -> Result<Vec<crate::avfx_sim::VfxBinderTargetSnapshot>, String> {
        let skeleton = self
            .skeleton
            .as_ref()
            .ok_or("VFX owner skeleton unavailable")?;
        crate::avfx_sim::VfxRuntime::bind_point_pose_targets(&self.bind_points, skeleton, pose)
    }

    /// 构建常驻采样运行时（携带武器绑点表）。
    pub fn runtime(&self) -> crate::avfx_sim::VfxRuntime {
        crate::avfx_sim::VfxRuntime::with_bind_points(&self.file, &self.bind_points)
    }
}

/// A mounted effect and the successfully loaded MDL that owns its bind points.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponVfxAttachment {
    pub model_path: String,
    pub data: WeaponVfxData,
}

/// Effects on distinct loaded primary/secondary weapon models. Each AVFX keeps
/// its own node, texture and model index spaces.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponVfxAttachments {
    pub attachments: Vec<WeaponVfxAttachment>,
    /// Successfully loaded primary/secondary preview MDLs used for the
    /// weapon-preview subset of the client's Aura target slots. A target stays
    /// available even when its MDL has no AVFX mount.
    pub model_skin_targets: WeaponVfxModelTargets,
    /// Mount failures retained when another attachment loaded successfully.
    pub diagnostics: Vec<String>,
}

/// One limitation shared by multiple particles or mounts. Keep the original
/// messages with their resource paths for diagnosis; only the summary merges
/// particle indices. Distinct parameter values remain distinct limitations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponVfxDiagnosticGroup {
    pub message: String,
    pub details: Vec<String>,
}

fn diagnostic_without_particle_index(message: &str) -> String {
    let Some(rest) = message.strip_prefix("Ptcl[") else {
        return message.to_owned();
    };
    let Some((index, rest)) = rest.split_once(']') else {
        return message.to_owned();
    };
    if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return message.to_owned();
    }
    format!("Ptcl[]{rest}")
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponVfxModelTargets {
    pub weapon: Option<String>,
    pub off_hand: Option<String>,
}

impl WeaponVfxAttachments {
    /// Loaded paths selected by the weapon/off-hand bits of AuraFilter, in
    /// client slot order. Character and summon need other host resources.
    pub fn model_skin_target_paths(&self, aura_target: i32) -> impl Iterator<Item = &str> {
        let flags = aura_target as u32;
        [
            (flags & 2 != 0)
                .then_some(self.model_skin_targets.weapon.as_deref())
                .flatten(),
            (flags & 4 != 0)
                .then_some(self.model_skin_targets.off_hand.as_deref())
                .flatten(),
        ]
        .into_iter()
        .flatten()
    }

    /// A single loaded target, when unambiguous. Use the plural iterator to
    /// route a combined AuraFilter to all of its loaded target slots.
    pub fn model_skin_target_path(&self, aura_target: i32) -> Option<&str> {
        let mut paths = self.model_skin_target_paths(aura_target);
        let first = paths.next()?;
        paths.next().is_none().then_some(first)
    }

    pub fn all_diagnostics(&self) -> impl Iterator<Item = String> + '_ {
        self.diagnostics
            .iter()
            .cloned()
            .chain(self.attachments.iter().flat_map(|attachment| {
                attachment
                    .data
                    .diagnostics
                    .iter()
                    .chain(&attachment.data.file.warnings)
                    .map(move |message| {
                        format!(
                            "{} ({}): {message}",
                            attachment.model_path, attachment.data.avfx_path,
                        )
                    })
            }))
    }

    pub fn diagnostic_groups(&self) -> Vec<WeaponVfxDiagnosticGroup> {
        let mut groups: Vec<WeaponVfxDiagnosticGroup> = Vec::new();
        let mut add = |message: &str, detail: String| {
            let key = diagnostic_without_particle_index(message);
            if let Some(group) = groups.iter_mut().find(|group| group.message == key) {
                group.details.push(detail);
            } else {
                groups.push(WeaponVfxDiagnosticGroup {
                    message: key,
                    details: vec![detail],
                });
            }
        };
        for message in &self.diagnostics {
            add(message, message.clone());
        }
        for attachment in &self.attachments {
            for message in attachment
                .data
                .diagnostics
                .iter()
                .chain(&attachment.data.file.warnings)
            {
                add(
                    message,
                    format!(
                        "{} ({}): {message}",
                        attachment.model_path, attachment.data.avfx_path
                    ),
                );
            }
        }
        groups
    }
}

#[cfg(test)]
mod diagnostic_group_tests {
    use super::*;

    #[test]
    fn repeated_particle_limitations_merge_across_mounts_without_losing_sources() {
        let mut effects = WeaponVfxAttachments::default();
        for path in ["main.mdl", "offhand.mdl"] {
            let mut attachment = WeaponVfxAttachment::default();
            attachment.model_path = path.to_owned();
            attachment.data.avfx_path = format!("{path}.avfx");
            attachment.data.file.warnings = vec![
                "Ptcl[0].bATM=1 tone-map stage not connected".to_owned(),
                "Ptcl[17].bATM=1 tone-map stage not connected".to_owned(),
                "Ptcl[1].DOTy/DpOf=(2, 0.1) unsupported".to_owned(),
                "Ptcl[2].DOTy/DpOf=(2, 0.2) unsupported".to_owned(),
            ];
            effects.attachments.push(attachment);
        }
        let groups = effects.diagnostic_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].details.len(), 4);
        assert!(groups[0].details[0].contains("main.mdl (main.mdl.avfx): Ptcl[0]"));
        assert!(groups[0].details[3].contains("offhand.mdl (offhand.mdl.avfx): Ptcl[17]"));
        assert_eq!(
            groups
                .iter()
                .flat_map(|group| group.details.iter())
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            effects.all_diagnostics().collect()
        );
    }

    #[test]
    fn mount_failures_and_malformed_particle_indices_are_preserved() {
        let effects = WeaponVfxAttachments {
            diagnostics: vec![
                "main.mdl: missing resource".into(),
                "offhand.mdl: missing resource".into(),
                "Ptcl[x].unknown".into(),
            ],
            ..Default::default()
        };
        let groups = effects.diagnostic_groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[2].message, "Ptcl[x].unknown");
    }
}

/// 解码到 RGBA8 的单个 VFX mip level。
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxTextureMipRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// BC6H_SF16 / R16G16B16A16F 的 source-authored 2D mip，每像素为 little-endian RGBA16F。
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxTextureMipRgba16f {
    pub width: u32,
    pub height: u32,
    pub rgba16f: Vec<u8>,
}

/// TEX cube 的一个 source-authored mip，面顺序与 TEX slice 顺序一致。
/// `faces` 的像素编码由所属贴图的 `cube_format` 指定。
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxTextureCubeMipRgba {
    pub width: u32,
    pub height: u32,
    pub faces: [Vec<u8>; 6],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxTextureCubeFormat {
    #[default]
    Rgba8Unorm,
    Rgba16Float,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxTextureSourceType {
    #[default]
    D2,
    D2Array,
    Cube,
    Other,
}

/// 解码到 RGBA8 的 VFX 贴图。顶层字段保持 base level 的既有接口，`mips`
/// 保存其后的 source-authored levels；cube 的顶层字段对应第 0 面。
/// HDR TEX 的顶层 RGBA8 字段仅用于兼容；`rgba16f_mips` 保留二维 HDR 采样，
/// cube 另由 `cube_mips` 保留全部六面。
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxTextureRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub mips: Vec<VfxTextureMipRgba>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rgba16f_mips: Vec<VfxTextureMipRgba16f>,
    pub is_cube: bool,
    pub source_type: VfxTextureSourceType,
    pub cube_format: VfxTextureCubeFormat,
    pub cube_mips: Vec<VfxTextureCubeMipRgba>,
    /// TEX 头中从首项开始连续非零的 surface offset 数；若大于当前资源类型的
    /// `decoded_mip_count()`，文件声明的后续 mip 未能完整解码。
    pub source_mip_count: u32,
}

impl VfxTextureRgba {
    pub fn decoded_mip_count(&self) -> u32 {
        if self.is_cube {
            self.cube_mips.len() as u32
        } else {
            1_u32.saturating_add(self.mips.len() as u32)
        }
    }

    pub fn has_complete_source_mips(&self) -> bool {
        self.source_mip_count == 0 || self.decoded_mip_count() == self.source_mip_count
    }
}

/// ModelSkin Aura's TC2/TC3/TD inputs packed as one RGBA8 2D array.
/// Each mip stores its enabled layers contiguously in slot order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxAuraTextureArrayRgba {
    pub width: u32,
    pub height: u32,
    pub slot_layers: [Option<u32>; 3],
    pub mips: Vec<VfxTextureMipRgba>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxAuraTextureArrayError {
    NotModelSkin,
    NoLayers,
    UnsupportedSource { slot: usize },
    MissingTexture { slot: usize, index: i32 },
    UnsupportedFormat { slot: usize },
    IncompleteSourceMips { slot: usize },
    InvalidMip { slot: usize, level: usize },
    IncompatibleLayout { slot: usize },
    OversizedMip { level: usize },
}

impl VfxAuraTextureArrayRgba {
    pub fn from_model_skin_particle(
        particle: &AvfxParticle,
        textures: &[Option<VfxTextureRgba>],
    ) -> Result<Self, VfxAuraTextureArrayError> {
        if !matches!(particle.data, AvfxParticleData::ModelSkin(_)) {
            return Err(VfxAuraTextureArrayError::NotModelSkin);
        }
        let sources = [
            particle.texture_color2.as_ref().map(|texture| {
                (
                    texture.enabled,
                    texture.texture_index,
                    texture.use_screen_copy || texture.use_chara_portrait,
                )
            }),
            particle.texture_color3.as_ref().map(|texture| {
                (
                    texture.enabled,
                    texture.texture_index,
                    texture.use_screen_copy || texture.use_chara_portrait,
                )
            }),
            particle
                .texture_distortion
                .as_ref()
                .map(|texture| (texture.enabled, texture.texture_index, false)),
        ];
        let mut slots = [None; 3];
        for (slot, source) in sources.into_iter().enumerate() {
            let Some((true, index, builtin)) = source else {
                continue;
            };
            if builtin {
                return Err(VfxAuraTextureArrayError::UnsupportedSource { slot });
            }
            slots[slot] = Some(
                usize::try_from(index)
                    .ok()
                    .and_then(|index| textures.get(index))
                    .and_then(Option::as_ref)
                    .ok_or(VfxAuraTextureArrayError::MissingTexture { slot, index })?,
            );
        }
        Self::pack(slots)
    }

    pub fn pack(slots: [Option<&VfxTextureRgba>; 3]) -> Result<Self, VfxAuraTextureArrayError> {
        let mut slot_layers = [None; 3];
        let mut enabled: Vec<&VfxTextureRgba> = Vec::new();
        for (slot, texture) in slots.into_iter().enumerate() {
            let Some(texture) = texture else { continue };
            if texture.source_type != VfxTextureSourceType::D2
                || texture.is_cube
                || !texture.rgba16f_mips.is_empty()
                || !texture.cube_mips.is_empty()
            {
                return Err(VfxAuraTextureArrayError::UnsupportedFormat { slot });
            }
            if texture.source_mip_count == 0
                || texture.decoded_mip_count() != texture.source_mip_count
            {
                return Err(VfxAuraTextureArrayError::IncompleteSourceMips { slot });
            }
            let max_mips = u32::BITS - texture.width.max(texture.height).leading_zeros();
            if texture.width == 0 || texture.height == 0 || texture.source_mip_count > max_mips {
                return Err(VfxAuraTextureArrayError::InvalidMip { slot, level: 0 });
            }
            for level in 0..texture.source_mip_count as usize {
                let (width, height, rgba) = if level == 0 {
                    (texture.width, texture.height, &texture.rgba)
                } else {
                    let mip = &texture.mips[level - 1];
                    (mip.width, mip.height, &mip.rgba)
                };
                let expected_len = (width as usize)
                    .checked_mul(height as usize)
                    .and_then(|pixels| pixels.checked_mul(4));
                if width != (texture.width >> level).max(1)
                    || height != (texture.height >> level).max(1)
                    || expected_len != Some(rgba.len())
                {
                    return Err(VfxAuraTextureArrayError::InvalidMip { slot, level });
                }
            }
            if let Some(reference) = enabled.first().copied() {
                if texture.width != reference.width
                    || texture.height != reference.height
                    || texture.source_mip_count != reference.source_mip_count
                {
                    return Err(VfxAuraTextureArrayError::IncompatibleLayout { slot });
                }
            }
            slot_layers[slot] = Some(enabled.len() as u32);
            enabled.push(texture);
        }
        let Some(reference) = enabled.first() else {
            return Err(VfxAuraTextureArrayError::NoLayers);
        };
        let mut mips = Vec::with_capacity(reference.source_mip_count as usize);
        for level in 0..reference.source_mip_count as usize {
            let width = (reference.width >> level).max(1);
            let height = (reference.height >> level).max(1);
            let layer_size = (width as usize) * (height as usize) * 4;
            let total_size = layer_size
                .checked_mul(enabled.len())
                .ok_or(VfxAuraTextureArrayError::OversizedMip { level })?;
            let mut rgba = Vec::new();
            rgba.try_reserve_exact(total_size)
                .map_err(|_| VfxAuraTextureArrayError::OversizedMip { level })?;
            for texture in &enabled {
                if level == 0 {
                    rgba.extend_from_slice(&texture.rgba);
                } else {
                    rgba.extend_from_slice(&texture.mips[level - 1].rgba);
                }
            }
            mips.push(VfxTextureMipRgba {
                width,
                height,
                rgba,
            });
        }
        Ok(Self {
            width: reference.width,
            height: reference.height,
            slot_layers,
            mips,
        })
    }

    pub fn layer_count(&self) -> u32 {
        self.slot_layers.iter().flatten().count() as u32
    }
}

/// 解码 `.atex` 贴图（avfx 内 `Tex` 块引用）。atex 在 `.tex` 负载前有
/// 8 字节头（`atex` 魔数 + 版本），tolerant：无魔数时按裸 tex 解析；
/// 数组贴图逐 mip 取第 0 层，cube 逐 mip 保留六面；BC6H_SF16 / R16G16B16A16F 保留 HDR 半浮点。
/// TEX 头与 surface span 语义对齐 Meddle
/// `TexFile.TexHeader` / `SliceSize` / `SliceSpan`。
#[cfg(feature = "game-data")]
pub fn decode_atex_rgba(bytes: &[u8]) -> Option<VfxTextureRgba> {
    let payload = if bytes.starts_with(b"atex") {
        bytes.get(8..)?
    } else {
        bytes
    };
    let levels = decode_tex_rgba_mips(payload)?;
    let source_mip_count = levels.source_mip_count;
    let is_cube = levels.is_cube;
    let source_type = levels.source_type;
    let cube_format = levels.cube_format;
    let cube_mips = levels.cube_mips;
    let rgba16f_mips = levels.rgba16f_mips;
    let mut levels = levels.levels.into_iter();
    let base = levels.next()?;
    Some(VfxTextureRgba {
        width: base.width,
        height: base.height,
        rgba: base.rgba,
        mips: levels.collect(),
        rgba16f_mips,
        is_cube,
        source_type,
        cube_format,
        cube_mips,
        source_mip_count,
    })
}

#[cfg(feature = "game-data")]
struct DecodedTexMipChain {
    levels: Vec<VfxTextureMipRgba>,
    rgba16f_mips: Vec<VfxTextureMipRgba16f>,
    is_cube: bool,
    source_type: VfxTextureSourceType,
    cube_format: VfxTextureCubeFormat,
    cube_mips: Vec<VfxTextureCubeMipRgba>,
    source_mip_count: u32,
}

#[cfg(feature = "game-data")]
fn decode_tex_rgba_mips(payload: &[u8]) -> Option<DecodedTexMipChain> {
    const HEADER_SIZE: usize = 80;
    const SURFACE_OFFSET_START: usize = 28;
    const MAX_SURFACE_OFFSETS: usize = 13;

    let header = payload.get(..HEADER_SIZE)?;
    let format = u32::from_le_bytes(header.get(4..8)?.try_into().ok()?);
    let attribute = u32::from_le_bytes(header.get(0..4)?.try_into().ok()?);
    let source_type = match attribute & 0x03c0_0000 {
        0x0200_0000 => VfxTextureSourceType::Cube,
        0x0080_0000 if attribute & 0x1000_0000 == 0 => VfxTextureSourceType::D2,
        0 | 0x0080_0000 if attribute & 0x1000_0000 != 0 => VfxTextureSourceType::D2Array,
        _ => VfxTextureSourceType::Other,
    };
    let is_cube = source_type == VfxTextureSourceType::Cube;
    let is_hdr = matches!(format, 0x6330 | 0x2460);
    let cube_format = if is_cube && is_hdr {
        VfxTextureCubeFormat::Rgba16Float
    } else {
        VfxTextureCubeFormat::Rgba8Unorm
    };
    let base_width = u16::from_le_bytes(header.get(8..10)?.try_into().ok()?) as u32;
    let base_height = u16::from_le_bytes(header.get(10..12)?.try_into().ok()?) as u32;
    if base_width == 0 || base_height == 0 {
        return None;
    }
    let declared_mips = usize::from(header[14].max(1)).min(MAX_SURFACE_OFFSETS);
    let mut offsets = Vec::with_capacity(declared_mips);
    for index in 0..declared_mips {
        let start = SURFACE_OFFSET_START + index * 4;
        let offset = u32::from_le_bytes(header.get(start..start + 4)?.try_into().ok()?) as usize;
        if offset == 0 {
            break;
        }
        offsets.push(offset);
    }
    if offsets.first().is_none_or(|offset| *offset < HEADER_SIZE) {
        return None;
    }

    let source_mip_count = offsets.len() as u32;
    let max_mip_count = (u32::BITS - base_width.max(base_height).leading_zeros()) as usize;
    let mut levels = Vec::with_capacity(offsets.len().min(max_mip_count));
    let mut rgba16f_mips = Vec::new();
    let mut cube_mips = Vec::new();
    for (index, &offset) in offsets.iter().take(max_mip_count).enumerate() {
        if offset < HEADER_SIZE || index > 0 && offset <= offsets[index - 1] {
            break;
        }
        let width = (base_width >> index).max(1);
        let height = (base_height >> index).max(1);
        let slice_size = tex_mip_slice_size(format, width, height)?;
        let Some(slice) = payload.get(offset..offset.checked_add(slice_size)?) else {
            break;
        };
        let decoded = if is_hdr {
            decode_hdr_slice_rgba16f(format, slice, width, height)
                .map(|hdr| (rgba16f_to_rgba8(&hdr), hdr))
        } else {
            decode_tex_slice_rgba(header, slice, width, height).map(|rgba| {
                let face = if is_cube { rgba.clone() } else { Vec::new() };
                (rgba, face)
            })
        };
        let Some((rgba, cube_face_zero)) = decoded else {
            break;
        };
        levels.push(VfxTextureMipRgba {
            width,
            height,
            rgba,
        });
        if is_hdr {
            rgba16f_mips.push(VfxTextureMipRgba16f {
                width,
                height,
                rgba16f: cube_face_zero.clone(),
            });
        }
        if is_cube {
            if width != height {
                break;
            }
            let next_offset = offsets.get(index + 1).copied().unwrap_or(payload.len());
            let Some(end) = slice_size
                .checked_mul(6)
                .and_then(|size| offset.checked_add(size))
            else {
                break;
            };
            if end > next_offset || end > payload.len() {
                break;
            }
            let mut faces = std::array::from_fn(|_| Vec::new());
            faces[0] = cube_face_zero;
            let mut complete = true;
            for (face, output) in faces.iter_mut().enumerate().skip(1) {
                let start = offset + face * slice_size;
                let decoded = if is_hdr {
                    decode_hdr_slice_rgba16f(
                        format,
                        &payload[start..start + slice_size],
                        width,
                        height,
                    )
                } else {
                    decode_tex_slice_rgba(
                        header,
                        &payload[start..start + slice_size],
                        width,
                        height,
                    )
                };
                let Some(decoded) = decoded else {
                    complete = false;
                    break;
                };
                *output = decoded;
            }
            if !complete {
                break;
            }
            cube_mips.push(VfxTextureCubeMipRgba {
                width,
                height,
                faces,
            });
        }
    }
    if levels.is_empty() {
        return None;
    }
    Some(DecodedTexMipChain {
        levels,
        rgba16f_mips,
        is_cube,
        source_type,
        cube_format,
        cube_mips,
        source_mip_count,
    })
}

#[cfg(feature = "game-data")]
fn tex_mip_slice_size(format: u32, width: u32, height: u32) -> Option<usize> {
    let bits_per_pixel = 1_usize.checked_shl((format & 0xf0) >> 4)?;
    match (format & 0xf000) >> 12 {
        3 | 6 => usize::try_from(width.div_ceil(4).max(1))
            .ok()?
            .checked_mul(usize::try_from(height.div_ceil(4).max(1)).ok()?)?
            .checked_mul(bits_per_pixel)?
            .checked_mul(2),
        1 | 2 | 4 | 5 => usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(bits_per_pixel)?
            .checked_div(8),
        _ => None,
    }
}

#[cfg(feature = "game-data")]
fn decode_hdr_slice_rgba16f(format: u32, slice: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    match format {
        0x6330 => decode_bc6h_slice_rgba16f(slice, width, height),
        0x2460 => {
            let expected = usize::try_from(width)
                .ok()?
                .checked_mul(usize::try_from(height).ok()?)?
                .checked_mul(8)?;
            Some(slice.get(..expected)?.to_vec())
        }
        _ => None,
    }
}

#[cfg(feature = "game-data")]
fn decode_bc6h_slice_rgba16f(slice: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let required = blocks_x.checked_mul(blocks_y)?.checked_mul(16)?;
    let slice = slice.get(..required)?;
    let mut rgba = vec![0; width.checked_mul(height)?.checked_mul(8)?];
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let offset = (block_y * blocks_x + block_x) * 16;
            let mut rgb = [0_u16; 4 * 4 * 3];
            bcdec_rs::bc6h_half(&slice[offset..offset + 16], &mut rgb, 4 * 3, true);
            for y in 0..4.min(height - block_y * 4) {
                for x in 0..4.min(width - block_x * 4) {
                    let pixel = ((block_y * 4 + y) * width + block_x * 4 + x) * 8;
                    let source = (y * 4 + x) * 3;
                    for channel in 0..3 {
                        rgba[pixel + channel * 2..pixel + channel * 2 + 2]
                            .copy_from_slice(&rgb[source + channel].to_le_bytes());
                    }
                    rgba[pixel + 6..pixel + 8].copy_from_slice(&0x3c00_u16.to_le_bytes());
                }
            }
        }
    }
    Some(rgba)
}

#[cfg(feature = "game-data")]
fn rgba16f_to_rgba8(rgba16f: &[u8]) -> Vec<u8> {
    rgba16f
        .chunks_exact(2)
        .map(|bits| {
            let value = f16::from_bits(u16::from_le_bytes([bits[0], bits[1]])).to_f32();
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        })
        .collect()
}

#[cfg(feature = "game-data")]
fn decode_tex_slice_rgba(
    source_header: &[u8],
    slice: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    const HEADER_SIZE: usize = 80;
    let mut single = source_header.to_vec();
    // The standalone slice is a 2D texture even when its source was a cube.
    single[0..4].copy_from_slice(&0x0080_0000_u32.to_le_bytes());
    single[8..10].copy_from_slice(&u16::try_from(width).ok()?.to_le_bytes());
    single[10..12].copy_from_slice(&u16::try_from(height).ok()?.to_le_bytes());
    single[12..14].copy_from_slice(&1_u16.to_le_bytes());
    single[14] = 1;
    single[15] = 1;
    single[28..32].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    single[32..80].fill(0);
    single.extend_from_slice(slice);
    let texture = <physis::tex::Texture as physis::ReadableFile>::from_existing(
        physis::Platform::Win32,
        &single,
    )?;
    let rgba = crate::texture_decode::decode_texture_rgba(&texture)?;
    let expected = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    (rgba.len() >= expected).then(|| rgba[..expected].to_vec())
}
