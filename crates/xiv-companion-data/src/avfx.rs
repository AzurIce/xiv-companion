//! `.avfx`（武器/技能特效）读取器：递归块 walker + 类型化子集 + 曲线求值。
//!
//! 布局对齐 VFXEditor `VFXEditor/Formats/AvfxFormat/`（社区字段级事实标准）：
//! 文件为一个外层块（盘上名字为反写 `AVFX`），子块为 `[反写4字节名][u32 size]
//! [负载]`，size 按 4 字节对齐推进；叶子字段负载为 4 字节（i32/f32/枚举，
//! 个别 1 字节布尔），`Tex` 为 NUL 结尾字符串，`TLst`/`MdNo` 等 IntList 为
//! 字节数组。`Schd`/`TmLn`/`Emit` 的 `Item`/`Trgr`/`ItPr`/`ItEm` 容器为累积式
//! （最后一个容器含全部条目），条目不带独立块包装，按固定步长（36/96 字节、
//! 312/300/288/276 字节）切分；`ItEm` 末容器的前 N 条是粒子条目的重复
//! （N = 粒子条目数），解析时跳过。
//!
//! 曲线分三种容器形态：
//! - 单轴曲线（`CrC`/`Gra`/UvSt `Rot` 等）：块内直接是 `KeyC`/`BvPr`/`BvPo`/
//!   `RanT`/`Keys`；
//! - 多轴曲线（`Pos`/`Rot`/`Scl` 与 UvSt 的 `Scl`/`Scr`）：容器块，子块为
//!   `ACT`/`ACTR`（轴连接枚举）与按轴命名的 `X`/`Y`/`Z` 单轴曲线（可缺省，
//!   缺省轴取默认值）；
//! - 颜色曲线（`Col`/`ColB`/`ColE`）：容器块，子块为 `RGB`（键的 x/y/z 即
//!   r/g/b，恒线性）、`A`（alpha）、`Bri`（HDR 亮度倍率）、`SclR`..`SclA`。
//!
//! 未知块按名计数进 `unknown_blocks`，永不 panic。

use half::f16;
use std::collections::{BTreeMap, HashMap};

/// 一条曲线关键帧：16 字节 = 时间(i16，帧) + 插值类型(u16) + 三分量(f32)。
///
/// 非颜色曲线中 `x`/`y` 是样条切线权重、`z` 是数值；颜色曲线直接用
/// `x`/`y`/`z` 作 RGB 且按线性插值（VFXEditor `AvfxCurveKey`）。
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
}

/// 曲线首尾行为（VFXEditor `CurveBehavior`）。
pub const BEHAVIOR_CONST: u32 = 0;
pub const BEHAVIOR_REPEAT: u32 = 1;
pub const BEHAVIOR_ADD: u32 = 2;

/// 单轴曲线：`BvPr`/`BvPo`/`RanT` 标量 + `Keys` 裸 16 字节键数组
/// （`KeyC` 计数块存在时仅用于校验）。
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
        let Some(first) = self.keys.first() else {
            return [0.0; 3];
        };
        let Some(last) = self.keys.last() else {
            return [0.0; 3];
        };
        let mut time = time;
        let first_time = f32::from(first.time);
        let last_time = f32::from(last.time);
        if last_time <= first_time {
            return [first.x, first.y, first.z];
        }
        match self.post_behavior {
            BEHAVIOR_REPEAT => {
                time = first_time + (time - first_time).rem_euclid(last_time - first_time);
            }
            // Const 与罕见的 Add（无累计上下文）都钳制到端点外延。
            _ => time = time.clamp(first_time, last_time),
        }
        let mut index = 0;
        for (i, key) in self.keys.iter().enumerate() {
            if f32::from(key.time) <= time {
                index = i;
            }
        }
        if index + 1 >= self.keys.len() {
            return [last.x, last.y, last.z];
        }
        let left = self.keys[index];
        let right = self.keys[index + 1];
        let t0 = f32::from(left.time);
        let t1 = f32::from(right.time);
        let span = (t1 - t0).max(f32::EPSILON);
        let local = ((time - t0) / span).clamp(0.0, 1.0);
        if left.interpolation == AvfxCurveKey::INTERPOLATION_STEP {
            return [left.x, left.y, left.z];
        }
        if left.interpolation == AvfxCurveKey::INTERPOLATION_LINEAR {
            return [
                left.x + (right.x - left.x) * local,
                left.y + (right.y - left.y) * local,
                left.z + (right.z - left.z) * local,
            ];
        }
        // Spline：VFXEditor `GetDrawLine` 的三次贝塞尔——两控制点都由左键的
        // x（出切线）/ y（入切线）权重给出，数值取 z；x/y 分量按线性近似。
        let mid = span * 0.5;
        let p0 = (t0, left.z);
        let p1 = (t0 + left.x * mid, left.z);
        let p2 = (t1 - left.y * mid, right.z);
        let p3 = (t1, right.z);
        let z = bezier_y_at_x(p0, p1, p2, p3, time);
        [
            left.x + (right.x - left.x) * local,
            left.y + (right.y - left.y) * local,
            z,
        ]
    }

    /// 颜色键求值：x/y/z 即 r/g/b，VFXEditor 对颜色曲线恒用线性（或阶梯）
    /// 插值，不做样条。
    pub fn color_at(&self, time: f32) -> [f32; 3] {
        let Some(first) = self.keys.first() else {
            return [0.0; 3];
        };
        let last = self.keys.last().expect("non-empty");
        let mut time = time;
        let first_time = f32::from(first.time);
        let last_time = f32::from(last.time);
        if last_time <= first_time {
            return [first.x, first.y, first.z];
        }
        match self.post_behavior {
            BEHAVIOR_REPEAT => {
                time = first_time + (time - first_time).rem_euclid(last_time - first_time);
            }
            _ => time = time.clamp(first_time, last_time),
        }
        let mut index = 0;
        for (i, key) in self.keys.iter().enumerate() {
            if f32::from(key.time) <= time {
                index = i;
            }
        }
        if index + 1 >= self.keys.len() {
            return [last.x, last.y, last.z];
        }
        let left = self.keys[index];
        let right = self.keys[index + 1];
        let t0 = f32::from(left.time);
        let t1 = f32::from(right.time);
        let span = (t1 - t0).max(f32::EPSILON);
        let local = ((time - t0) / span).clamp(0.0, 1.0);
        if left.interpolation == AvfxCurveKey::INTERPOLATION_STEP {
            return [left.x, left.y, left.z];
        }
        [
            left.x + (right.x - left.x) * local,
            left.y + (right.y - left.y) * local,
            left.z + (right.z - left.z) * local,
        ]
    }

    /// 数值求值（z 分量）；无键时返回 `default`。
    pub fn value(&self, time: f32, default: f32) -> f32 {
        if self.keys.is_empty() {
            default
        } else {
            self.evaluate(time)[2]
        }
    }
}

/// 三次贝塞尔按 x 反解参数（二分，单调段足够）后取 y。
fn bezier_y_at_x(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), x: f32) -> f32 {
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    let mut s = 0.5_f32;
    for _ in 0..20 {
        s = (low + high) * 0.5;
        if cubic(p0.0, p1.0, p2.0, p3.0, s) < x {
            low = s;
        } else {
            high = s;
        }
    }
    cubic(p0.1, p1.1, p2.1, p3.1, s)
}

fn cubic(a: f32, b: f32, c: f32, d: f32, s: f32) -> f32 {
    let t = 1.0 - s;
    t * t * t * a + 3.0 * t * t * s * b + 3.0 * t * s * s * c + s * s * s * d
}

/// 三轴曲线容器（`Pos`/`Rot`/`Scl`）：子块 `ACT`（轴连接枚举）+ 按轴命名的
/// 单轴曲线 `X`/`Y`/`Z`（各自可缺省）。随机轴 `XR`/`YR`/`ZR` 暂不采样。
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
        let mut x = self.x.as_ref().map_or(default, |c| c.value(time, default));
        let mut y = self.y.as_ref().map_or(default, |c| c.value(time, default));
        let mut z = self.z.as_ref().map_or(default, |c| c.value(time, default));
        match self.axis_connect {
            1 => {
                // X_YZ
                if self.x.is_some() {
                    y = x;
                    z = x;
                }
            }
            2 => {
                if self.x.is_some() {
                    y = x;
                }
            }
            3 => {
                if self.x.is_some() {
                    z = x;
                }
            }
            4 => {
                // Y_XZ
                if self.y.is_some() {
                    x = y;
                    z = y;
                }
            }
            5 => {
                if self.y.is_some() {
                    x = y;
                }
            }
            6 => {
                if self.y.is_some() {
                    z = y;
                }
            }
            7 => {
                // Z_XY
                if self.z.is_some() {
                    x = z;
                    y = z;
                }
            }
            8 => {
                if self.z.is_some() {
                    x = z;
                }
            }
            9 => {
                if self.z.is_some() {
                    y = z;
                }
            }
            _ => {}
        }
        [x, y, z]
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_none() && self.y.is_none() && self.z.is_none()
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
        let mut x = self.x.as_ref().map_or(default, |c| c.value(time, default));
        let mut y = self.y.as_ref().map_or(default, |c| c.value(time, default));
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
/// `Bri` HDR 亮度倍率 + `SclA` 贴图 alpha 缩放。缺省：白、不透明、倍率 1。
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

/// `SclR`/`SclG`/`SclB`：贴图 rgb 缩放曲线。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxColorScaleRgb {
    pub r: Option<AvfxCurve>,
    pub g: Option<AvfxCurve>,
    pub b: Option<AvfxCurve>,
}

impl AvfxColorCurve {
    /// `time` 处的 (r, g, b, a)：rgb 已乘 `Bri`（HDR，可 >1）。
    pub fn rgba(&self, time: f32) -> [f32; 4] {
        self.rgba_with_brightness(time, true)
    }

    /// `Bri` 亮度倍率可选关闭：Blend（非加色）粒子不是发光体，Bri 不参与
    /// 乘算（AVFXTools 合成里 Bri 恒未生效；加色 HDR 粒子照旧）。
    pub fn rgba_with_brightness(&self, time: f32, apply_brightness: bool) -> [f32; 4] {
        let rgb = self.rgb.as_ref().map_or([1.0; 3], |c| c.color_at(time));
        let alpha = self.alpha.as_ref().map_or(1.0, |c| c.value(time, 1.0));
        let bri = if apply_brightness {
            self.brightness.as_ref().map_or(1.0, |c| c.value(time, 1.0))
        } else {
            1.0
        };
        [rgb[0] * bri, rgb[1] * bri, rgb[2] * bri, alpha]
    }

    /// 贴图 alpha 缩放（`SclA`），缺省 1。
    pub fn texture_alpha_scale(&self, time: f32) -> f32 {
        self.scale_alpha.as_ref().map_or(1.0, |c| c.value(time, 1.0))
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

/// Emitter 内的粒子/子发射器创建项（`ItPr`/`ItEm` 容器，累积式）。
/// 字段对齐 VFXEditor `AvfxEmitterItem`。
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEmitterItem {
    pub enabled: bool,
    pub target_index: i32,
    /// `LoDr`：局部方向模式。
    pub local_direction: i32,
    pub create_time: i32,
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
    pub start_frame: i32,
    pub start_frame_null_update: bool,
    /// `BIAX`/`BIAY`/`BIAZ`：按注入角逐粒子分布的旋转（弧度）。
    pub by_injection_angle: [f32; 3],
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
pub const DRAW_MODE_ADD: i32 = 2;

/// 朝向基准（`RBDT`；VFXEditor `RotationDirectionBase`）。
pub mod rotation_direction_base {
    pub const X: i32 = 0;
    pub const Y: i32 = 1;
    pub const Z: i32 = 2;
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
    pub calculate_uv: u32,
    pub scale: AvfxCurve2Axis,
    pub scroll: AvfxCurve2Axis,
    pub rotation: AvfxCurve,
    pub rotation_random: AvfxCurve,
}

/// 粒子贴图引用（`TC1`..`TC4`/`TN` 等同构子集）。真实武器文件里 TC1 常只
/// 写 `TLst`（贴图序号字节列表，VFXEditor 称 Mask Index）而不写 `TxNo`；
/// 有效贴图序号的解析见 [`AvfxParticleTexture::effective_texture`]。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTexture {
    pub enabled: bool,
    pub uv_set_index: i32,
    /// `TxNo`（TC2 及以后的主贴图序号；TC1 里常缺省 -1）。
    pub texture_index: i32,
    /// `TLst[0]`（TC1 的有效贴图序号；无块为 -1）。
    pub mask_texture_index: i32,
    /// `TCCT`（颜色合成模式；仅 TC2+ 有意义，TC1 恒为覆盖写）。
    pub calculate_color: i32,
    /// `TCAT`（alpha 合成模式）。
    pub calculate_alpha: i32,
    /// `bC2A`（颜色转 alpha：alpha 取颜色 x 分量）。
    pub color_to_alpha: bool,
    /// `bUSC`/`bPFC`：屏幕拷贝/上一帧拷贝来源（渲染端不支持时按普通贴图处理）。
    pub use_screen_copy: bool,
    pub previous_frame_copy: bool,
    /// `bUOS`：角色肖像贴图（UI 场景）。
    pub use_chara_portrait: bool,
    /// `TFT`：过滤模式（0 近邻、1 线性……）。
    pub texture_filter: i32,
    /// `TBUT`/`TBVT`：U/V 边界模式（0 Repeat、1 Clamp、2 Mirror）。
    pub texture_border_u: i32,
    pub texture_border_v: i32,
    /// `TxN`/`TxNR`：TC1 的贴图序号曲线及其随机项（按寿命选 TLst 里的贴图）。
    pub tex_n: Option<AvfxCurve>,
    pub tex_n_random: Option<AvfxCurve>,
    /// `TLst` 完整列表（`mask_texture_index` 是首项；TxN 曲线索引整个池）。
    pub texture_list: Vec<i32>,
}

impl AvfxParticleTexture {
    /// 实际用于采样的文件级贴图序号：`TLst` 优先（TC1 的真实数据口径），
    /// 回退 `TxNo`；-1 表示无贴图。
    pub fn effective_texture(&self) -> i32 {
        if self.mask_texture_index >= 0 {
            self.mask_texture_index
        } else {
            self.texture_index
        }
    }

    /// TLst 来源的贴图是形状遮罩（白形黑底）：只把亮度写进 alpha、不改
    /// rgb——颜色由 Col/TC2 承担（火舌的亮青白 = tone 渐变，纹路 = 遮罩）。
    pub fn is_shape_mask(&self) -> bool {
        self.mask_texture_index >= 0
    }
}

/// `TN` 法线贴图（需场景光照模型；渲染端暂未消费）。
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

/// `TR` 反射贴图（需场景反射模型；渲染端暂未消费）。
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

/// `TP` 调色板贴图（按 `POff` 在调色板内取色；渲染端暂未消费）。
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
    /// 标量叶子（i32 读值）。
    pub scalars: BTreeMap<String, i32>,
    /// 单轴曲线容器（含 Keys 子块）。
    pub curves: BTreeMap<String, AvfxCurve>,
    /// 多轴曲线容器（含 X/Y/Z 子块）。
    pub curve3s: BTreeMap<String, AvfxCurve3Axis>,
    /// 颜色曲线容器（含 RGB 子块）。
    pub color_curves: BTreeMap<String, AvfxColorCurve>,
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
        /// `MdNo` 模型序号字节列表（随机池，取首个）。
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
    /// LightModel 粒子：`MNO` 模型序号。
    LightModel { model_index: i32 },
    /// Powder 粒子（子粒子发射器）：`bMV`/`bLoc`/`bLgt`/`LgtT`/`CnOf`。
    Powder {
        use_character_movement: bool,
        use_character_location: bool,
        is_lightning: bool,
        directional_light_type: i32,
        center_offset: f32,
    },
    /// Quad 粒子：`SS`/`bMP`。
    Quad { scaling_scale: i32, is_movement_particle: bool },
    /// 其余类型（Disc/Polygon/Line/Polyline/Laser/Windmill/Decal/…）：
    /// 全字段原样保留在通用容器里。
    Other(AvfxGenericData),
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
    /// `CCnt`：单 spawner 生命周期内子粒子总数上限。
    pub create_count: i32,
    /// `CrAX`/`CrAY`/`CrAZ`：出生点随机散布半径。
    pub create_area: [f32; 3],
    /// `CAX`/`CAY`/`CAZ`：出生点坐标精度。
    pub coord_accuracy: [f32; 3],
    /// `CGX`/`CGY`/`CGZ`：子粒子重力。
    pub coord_gravity: [f32; 3],
    /// `SBX`/`SBY` 与 `SEX`/`SEY`：出生/消亡尺寸。
    pub scale_start: [f32; 2],
    pub scale_end: [f32; 2],
    /// `SC`：尺寸插值曲线指数。
    pub scale_curve: f32,
    /// `SRX0`/`SRX1`、`SRY0`/`SRY1`：尺寸随机倍率区间。
    pub scale_rand_x: [f32; 2],
    pub scale_rand_y: [f32; 2],
    /// `RIX`/`RIY`/`RIZ`：出生旋转（度）。
    pub rotation_start: [f32; 3],
    /// `RAX`/`RAY`/`RAZ`：每帧旋转增量。
    pub rotation_add: [f32; 3],
    /// `RBX`/`RBY`/`RBZ`：旋转基准。
    pub rotation_base: [f32; 3],
    /// `RVX`/`RVY`/`RVZ`：旋转速度。
    pub rotation_velocity: [f32; 3],
    /// `VMin`/`VMax`：子粒子初速度区间（单位/帧）。
    pub velocity_min: f32,
    pub velocity_max: f32,
    /// `FltR`/`FltS`：速度衰减率与目标速度。
    pub velocity_flattery_rate: f32,
    pub velocity_flattery_speed: f32,
    /// `UvCU`/`UvCV`/`UvIv`：UV 翻页贴图的格子数与翻页间隔（帧）。
    pub uv_cell: [i32; 2],
    pub uv_interval: i32,
    /// `UvNR`：UV 起始格随机个数；`UvLC`：翻页循环次数。
    pub uv_no_random: i32,
    pub uv_loop_count: i32,
    /// `IJMN`：出生点引用的发射模型序号（-1 = 发射器原点）。
    pub injection_model_index: i32,
    /// `VBMN`：出生点绑定顶点的模型序号（-1 = 不绑定）。
    pub injection_vertex_bind_model_index: i32,
    /// `IRD0`/`IRD1`：径向出生方向区间。
    pub injection_radial_dir: [f32; 2],
    /// `PvtX`/`PvtY`：旋转枢轴。
    pub pivot: [f32; 2],
    /// `BlkN`：block 编号。
    pub block_num: i32,
    /// `LLin`/`LLax`：线长区间（Polyline 子粒子）。
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
    /// `CrLR`：寿命随机项。
    pub create_life_random: i32,
    /// `bCrN`：子粒子死亡后重建。
    pub create_new_after_delete: bool,
    /// `bRUV`：UV 翻页反向。
    pub uv_reverse: bool,
    /// `bSRL`：X/Y 尺寸随机联动。
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
    pub items: Vec<AvfxSchedulerItem>,
    /// 固定 12 个调度触发器（拔刀/收刀等）；常驻特效只消费 `items`。
    pub triggers: Vec<AvfxSchedulerItem>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxTimeline {
    pub loop_start: i32,
    pub loop_end: i32,
    pub binder_index: i32,
    pub items: Vec<AvfxTimelineItem>,
    pub clip_count: usize,
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
    /// `CCOT`：坐标计算顺序（0 Scale_Rot_Translate、1 Translate_Scale_Rot）。
    pub coord_compute_order: i32,
    /// `bAD`：任意方向发射。
    pub any_direction: bool,
    /// `EfNo`：效果器序号（-1 无）。
    pub effector_index: i32,
    pub life: AvfxLife,
    pub create_count: AvfxCurve,
    pub create_count_random: AvfxCurve,
    pub create_interval: AvfxCurve,
    pub create_interval_random: AvfxCurve,
    /// `Gra`/`GraR`：发射器重力（传子粒子）。
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
    /// `VRX`/`VRY`/`VRZ`：初速度随机幅度。
    pub velocity_random: [AvfxCurve; 3],
    pub particle_items: Vec<AvfxEmitterItem>,
    pub emitter_items: Vec<AvfxEmitterItem>,
    /// 形状/模型发射数据（按 `EVT` 解析 `Data` 块）。
    pub data: Option<AvfxEmitterData>,
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

/// 圆锥发射数据（类型 1）：`InS`/`OuS` 内外径、`IjS` 注入速度、`IjA` 注入角。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConeEmitterData {
    pub inner_size: AvfxCurve,
    pub outer_size: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_angle: AvfxCurve,
}

/// 圆锥模型发射数据（类型 2）：`GeMT` 生成方式、`DivX`/`DivY` 分割数、
/// `Rad` 半径、`IjS` 注入速度、`IjA` 注入角。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConeModelEmitterData {
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_angle: AvfxCurve,
}

/// 球模型发射数据（类型 3）：`Rads` 半径、`IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SphereModelEmitterData {
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
}

/// 圆柱模型发射数据（类型 4）：`Len` 长度、`Rad` 半径、`IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CylinderModelEmitterData {
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub length: AvfxCurve,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
}

/// 模型发射数据（类型 5）：`MdNo` 发射模型序号、`GeMT` 生成方式、
/// `IjS` 注入速度。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEmitterData {
    pub model_index: i32,
    pub generate_method: i32,
    pub injection_speed: AvfxCurve,
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
    /// 剔除模式（`CulT`：VFXEditor `CullingType` 0 双面、1 剔正面、2 剔背面、
    /// 3 Double；壳体膜（如屠龙戟灵光的光罩）靠剔正面实现「透过正面看内侧」
    /// 的薄纱观感）。
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
    /// `DsSp`：软粒子（深度衰减；需场景深度，渲染端暂未消费）。
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
    pub rotation_velocity: [AvfxCurve; 3],
    /// `VRXR`/`VRYR`/`VRZR`：旋转速度随机曲线。
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
    /// 粒子寿命（帧）；未启用或 <=0 表示永生（跟随发射器/时间轴，
    /// VFXEditor/AVFXTools 的 JustOneCreate 语义来源）。
    pub fn life_frames(&self) -> Option<f32> {
        if self.life.enabled && self.life.value > 0.0 {
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
    /// `BPT`：绑点类型（0 自由、1 按 ID……）。
    pub bind_point_type: i32,
    /// `BPTP`：绑定目标点类型（默认 0 = 按名）。
    pub bind_target_point_type: i32,
    /// `Name`：按名绑定目标（角色骨骼名等）。
    pub binder_name: String,
    /// `BPID`：目标模型上的绑点序号（武器 MDL 的 ElementId：
    /// 3=基部 / 4=中部 / 5=尖部……；-1 = 未指定，保持原点）。
    pub bind_point_id: i32,
    /// `GenD`：生成延迟（帧）。
    pub generate_delay: i32,
    /// `CoUF`：坐标更新帧。
    pub coord_update_frame: i32,
    /// `bRng` 与 `RnPT`/`RnPX`-`RnPZ`/`RnRd`：环形绑点参数。
    pub ring_enabled: bool,
    pub ring_progress_time: i32,
    pub ring_position: [f32; 3],
    pub ring_radius: f32,
    pub bct: i32,
    /// `Pos`：绑点位置曲线。
    pub position: AvfxCurve3Axis,
}

/// `Bind` 绑点块（字段对齐 VFXEditor `AvfxBinder`）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxBinder {
    /// `BnVr`：0 = Point（武器挂点绑定，其余类型原样记录）。
    pub binder_type: u32,
    /// `properties_start.bind_point_id` 的便捷镜像（渲染主路径消费）。
    pub bind_point_id: i32,
    /// `bStG`：起始即全局方向。
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
}

/// `Efct` 效果器（相机震动/画面扭曲等；武器特效几乎不用，保留参数与
/// 原始 `Data` 负载）。
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
    /// `Data` 块原始负载（类型相关，未建模）。
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
    pub draw: Option<VfxDrawModel>,
}

/// 绘制网格顶点（`VDrw`，36 字节/顶点）：half4 位置 + 4B 法线 + 4B 切线 +
/// 4B 顶点色 + 4 组 half2 UV（只取首组）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxDrawVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
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
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
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
    /// `bLTS`/`bAGS`。
    pub lts_enabled: bool,
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
        let mut unknown_blocks = BTreeMap::new();
        let mut file = Self {
            version: root.scalar("Ver").unwrap_or(0),
            ..Default::default()
        };

        for node in root.children() {
            match node.name() {
                "Ver" | "ScCn" | "TlCn" | "EmCn" | "PrCn" | "EfCn" | "BdCn" | "TxCn" | "MdCn" => {}
                "Schd" => file.schedulers.push(parse_scheduler(&node, &mut warnings)),
                "TmLn" => file.timelines.push(parse_timeline(&node)),
                "Emit" => file.emitters.push(parse_emitter(&node, &mut warnings)),
                "Ptcl" => file.particles.push(parse_particle(&node, &mut warnings)),
                "Bind" => file.binders.push(parse_binder(&node)),
                "Efct" => file.effectors.push(parse_effector(&node)),
                "Modl" => file.models.push(parse_model(&node)),
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
        file.global = parse_global_parameters(&root);
        file.warnings = warnings;
        file.unknown_blocks = unknown_blocks;
        Ok(file)
    }
}

/// 根级全局参数块名（VFXEditor `AvfxMain` 的全部叶子）。
const GLOBAL_BLOCK_NAMES: &[&str] = &[
    "bDFP", "bFG", "bTS", "bASH", "bCBC", "bCul", "CBPx", "CBPy", "CBPz", "CBSx", "CBSy",
    "CBSz", "ZBMs", "ZBMd", "bCmS", "bFEL", "bOSE", "bOSt", "NCB", "NCE", "FCB", "FCE",
    "SPFR", "SKO", "DwLy", "DwOT", "DLST", "PL1S", "PL2S", "RvPx", "RvPy", "RvPz", "RvRx",
    "RvRy", "RvRz", "RvSx", "RvSy", "RvSz", "RvR", "RvG", "RvB", "AFXe", "AFXi", "AFXo",
    "AFYe", "AFYi", "AFYo", "AFZe", "AFZi", "AFZo", "bGFE", "GFIM", "bLTS", "bAGS", "APri",
    "DPri", "bSAB", "bSBV", "SBVa", "bSSV", "SSVa", "SPHP",
];

fn parse_global_parameters(root: &AvfxNodeView) -> AvfxGlobalParameters {
    let bool_at = |name: &str| root.scalar(name).map(|value| value != 0).unwrap_or(false);
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
        draw_layer: i32_at("DwLy"),
        draw_order: i32_at("DwOT"),
        directional_light_source: i32_at("DLST"),
        point_light_1: i32_at("PL1S"),
        point_light_2: i32_at("PL2S"),
        revised_position: [f32_at("RvPx"), f32_at("RvPy"), f32_at("RvPz")],
        revised_rotation: [f32_at("RvRx"), f32_at("RvRy"), f32_at("RvRz")],
        revised_scale: [f32_at("RvSx"), f32_at("RvSy"), f32_at("RvSz")],
        revised_color: [f32_at("RvR"), f32_at("RvG"), f32_at("RvB")],
        fade_enabled: [bool_at("AFXe"), bool_at("AFYe"), bool_at("AFZe")],
        fade_inner: [f32_at("AFXi"), f32_at("AFYi"), f32_at("AFZi")],
        fade_outer: [f32_at("AFXo"), f32_at("AFYo"), f32_at("AFZo")],
        global_fog_enabled: bool_at("bGFE"),
        global_fog_influence: f32_at("GFIM"),
        lts_enabled: bool_at("bLTS"),
        ags_enabled: bool_at("bAGS"),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvfxParseError {
    TooShort { len: usize },
    BadRootName { raw: [u8; 4] },
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
        }
    }
}

impl std::error::Error for AvfxParseError {}

/// 块视图：名字（反写还原、去尾空白/NUL）+ 原始负载。
pub struct AvfxNodeView<'a> {
    name: std::borrow::Cow<'a, str>,
    payload: &'a [u8],
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
        let end = 8usize.saturating_add(size).min(bytes.len());
        Ok(Self {
            name: std::borrow::Cow::Borrowed("AVFX"),
            payload: &bytes[8..end],
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
        self.scalar(name).map(|value| value as i32)
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
        if self.offset + 8 > self.bytes.len() {
            return None;
        }
        let start = self.offset;
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&self.bytes[start..start + 4]);
        let size = u32::from_le_bytes([
            self.bytes[start + 4],
            self.bytes[start + 5],
            self.bytes[start + 6],
            self.bytes[start + 7],
        ]) as usize;
        if start + 8 + size > self.bytes.len() {
            self.offset = self.bytes.len();
            return None;
        }
        let aligned = size.div_ceil(4) * 4;
        self.offset = start + 8 + aligned;
        let name = reverse_name_bytes(&raw);
        if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_graphic()) {
            return self.next();
        }
        Some(AvfxNodeView {
            name: std::borrow::Cow::Owned(name),
            payload: &self.bytes[start + 8..start + 8 + size],
        })
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
        self.u32(name).map(|value| value as i32)
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

/// 颜色曲线容器：`RGB`/`A`/`Bri`/`SclA` 子曲线。
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

fn parse_scheduler(node: &AvfxNodeView, warnings: &mut Vec<String>) -> AvfxScheduler {
    // Item/Trgr 容器为累积式：每个条目 36 字节（3 个 12 字节块）。
    let items = last_container_payload(node, "Item")
        .map(|payload| {
            payload
                .chunks_exact(36)
                .map(|chunk| {
                    let fields = Fields::walk(chunk);
                    AvfxSchedulerItem {
                        enabled: fields.boolean("bEna").unwrap_or(true),
                        start_time: fields.i32("StTm").unwrap_or(0),
                        timeline_index: fields.i32("TlNo").unwrap_or(-1),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let triggers = last_container_payload(node, "Trgr")
        .map(|payload| {
            payload
                .chunks_exact(36)
                .map(|chunk| {
                    let fields = Fields::walk(chunk);
                    AvfxSchedulerItem {
                        enabled: fields.boolean("bEna").unwrap_or(true),
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
    AvfxScheduler { items, triggers }
}

fn parse_timeline(node: &AvfxNodeView) -> AvfxTimeline {
    // 每个条目 96 字节（8 个 12 字节块）。
    let items = last_container_payload(node, "Item")
        .map(|payload| {
            payload
                .chunks_exact(96)
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
    AvfxTimeline {
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        binder_index: node.scalar_i32("BnNo").unwrap_or(-1),
        items,
        clip_count: node.children_named("Clip").count(),
    }
}

/// Emitter 的 ItPr/ItEm 条目：23–26 个 12 字节块（版本差异），步长取
/// VFXEditor 同款候选 [312, 300, 288, 276] 里第一个能整除的。
fn parse_emitter_items(node: &AvfxNodeView, name: &str) -> Vec<AvfxEmitterItem> {
    let Some(payload) = last_container_payload(node, name) else {
        return Vec::new();
    };
    let stride = [312usize, 300, 288, 276]
        .into_iter()
        .find(|stride| payload.len() % stride == 0 && payload.len() / stride > 0);
    let Some(stride) = stride else {
        return Vec::new();
    };
    payload
        .chunks_exact(stride)
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

fn parse_emitter(node: &AvfxNodeView, warnings: &mut Vec<String>) -> AvfxEmitter {
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
        velocity_random: [
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
    if let Some(data) = node.child("Data") {
        emitter.data = match emitter.emitter_type {
            Some(EmitterType::Cone) => Some(AvfxEmitterData::Cone(ConeEmitterData {
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
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(1),
                    divide_y: data.scalar_i32("DivY").unwrap_or(1),
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
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(1),
                    divide_y: data.scalar_i32("DivY").unwrap_or(1),
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
                    generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                    divide_x: data.scalar_i32("DivX").unwrap_or(1),
                    divide_y: data.scalar_i32("DivY").unwrap_or(1),
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
                model_index: data.scalar_i32("MdNo").unwrap_or(-1),
                generate_method: data.scalar_i32("GeMT").unwrap_or(0),
                injection_speed: data
                    .child("IjS")
                    .map(|child| parse_curve(&child))
                    .unwrap_or_default(),
            })),
            // Point 无形状数据；其余未建模类型提示一次。
            Some(EmitterType::Point) | None => None,
            Some(EmitterType::Unknown(raw)) => {
                warnings.push(format!("emitter data for type {raw} not modeled"));
                None
            }
        };
    }
    emitter
}

/// `TLst`/`MdNo` 等 IntList 叶子：负载为字节数组，每字节一个序号。
fn read_int_list(payload: &[u8]) -> Vec<i32> {
    payload.iter().map(|byte| i32::from(*byte)).collect()
}

fn parse_particle_texture(node: &AvfxNodeView) -> AvfxParticleTexture {
    let fields = Fields::walk(node.payload());
    let texture_list = fields
        .bytes("TLst")
        .map(read_int_list)
        .unwrap_or_default();
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
        _ => AvfxParticleData::None,
    }
}

/// `Smpl` 块（Powder 子粒子参数）。
fn parse_particle_simple(node: &AvfxNodeView) -> Option<AvfxParticleSimple> {
    let simple = node.child("Smpl")?;
    let fields = Fields::walk(simple.payload());
    let f32_pair = |a: &str, b: &str| {
        [
            fields.f32(a).unwrap_or(0.0),
            fields.f32(b).unwrap_or(0.0),
        ]
    };
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
        uv_interval: fields.i32("UvIv").unwrap_or(1).max(1),
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

fn parse_particle(node: &AvfxNodeView, warnings: &mut Vec<String>) -> AvfxParticle {
    let raw_particle_type = node.scalar("PrVT").unwrap_or(u32::MAX);
    let particle_type = Some(ParticleType::from_raw(raw_particle_type));
    let particle = AvfxParticle {
        particle_type,
        raw_particle_type,
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        draw_mode: node.scalar_i32("RMT").unwrap_or(0),
        culling_type: node.scalar_i32("CulT").unwrap_or(0),
        depth_test: node.scalar("DsDt").map(|value| value != 0).unwrap_or(true),
        depth_write: node.scalar("DsDw").map(|value| value != 0).unwrap_or(false),
        rotation_direction_base: node.scalar_i32("RBDT").unwrap_or(0),
        rotation_order: node.scalar_i32("RoOT").unwrap_or(0),
        coord_compute_order: node.scalar_i32("CCOT").unwrap_or(0),
        env_light_type: node.scalar_i32("EnvT").unwrap_or(0),
        dir_light_type: node.scalar_i32("DirT").unwrap_or(0),
        uv_precision: node.scalar_i32("UVPT").unwrap_or(0),
        draw_priority: node.scalar_i32("DwPr").unwrap_or(0),
        is_soft_particle: node.scalar("DsSp").map(|value| value != 0).unwrap_or(false),
        collision_type: node.scalar_i32("Coll").unwrap_or(0),
        s11_enabled: node.scalar("bS11").map(|value| value != 0).unwrap_or(false),
        sh_u_t: node.scalar_i32("ShUT").unwrap_or(0),
        sh_r: node.scalar_i32("ShR").unwrap_or(0),
        sh_t: node.scalar_i32("ShT").unwrap_or(0),
        uni_v: node.scalar_i32("UniV").unwrap_or(0),
        hyb_v: node.scalar_i32("HybV").unwrap_or(0),
        e24_enabled: node.scalar("bE24").map(|value| value != 0).unwrap_or(false),
        is_apply_tone_map: node.scalar("bATM").map(|value| value != 0).unwrap_or(false),
        is_apply_fog: node.scalar("bAFg").map(|value| value != 0).unwrap_or(false),
        clip_near_enable: node.scalar("bNea").map(|value| value != 0).unwrap_or(false),
        clip_far_enable: node.scalar("bFar").map(|value| value != 0).unwrap_or(false),
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
        simple_anim_enable: node.scalar("bSCt").map(|value| value != 0).unwrap_or(false),
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
    match particle.particle_type {
        Some(
            ParticleType::Quad
            | ParticleType::Parameter
            | ParticleType::Powder
            | ParticleType::Line
            | ParticleType::Laser
            | ParticleType::Polyline
            | ParticleType::Model
            | ParticleType::LightModel,
        ) => {}
        Some(ParticleType::Unknown(raw)) if raw == u32::MAX => {}
        other => warnings.push(format!(
            "particle type {:?} renders as quad approximation",
            other
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|| "unset".to_string())
        )),
    }
    particle
}

/// 根级 `Modl` 块：每块一个模型（VFXEditor `AvfxModel`），`VEmt` 发射顶点与
/// `VDrw`/`VIdx` 绘制网格可共存于同一块。坐标保持 avfx 模型空间原样
/// （与武器模型同向，不翻转）。
fn parse_model(node: &AvfxNodeView) -> VfxModelGeometry {
    let mut model = VfxModelGeometry::default();
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
    if let Some(draw_vertexes) = node.child("VDrw") {
        let mut vertices = Vec::with_capacity(draw_vertexes.payload().len() / 36);
        for chunk in draw_vertexes.payload().chunks_exact(36) {
            vertices.push(VfxDrawVertex {
                position: [
                    f16_to_f32([chunk[0], chunk[1]]),
                    f16_to_f32([chunk[2], chunk[3]]),
                    f16_to_f32([chunk[4], chunk[5]]),
                ],
                // 首组 UV（half2），其余三组不消费。
                uv: [
                    f16_to_f32([chunk[20], chunk[21]]),
                    f16_to_f32([chunk[22], chunk[23]]),
                ],
                color: [chunk[16], chunk[17], chunk[18], chunk[19]],
            });
        }
        let mut indices = Vec::new();
        if let Some(indexes) = node.child("VIdx") {
            for chunk in indexes.payload().chunks_exact(6) {
                for i in 0..3 {
                    let index = i16::from_le_bytes([chunk[i * 2], chunk[i * 2 + 1]]);
                    indices.push(index.max(0) as u32);
                }
            }
        }
        model.draw = Some(VfxDrawModel { vertices, indices });
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
        binder_name: fields.bytes("Name").map(read_binder_name).unwrap_or_default(),
        bind_point_id: fields.i32("BPID").unwrap_or(-1),
        generate_delay: fields.i32("GenD").unwrap_or(0),
        coord_update_frame: fields.i32("CoUF").unwrap_or(-1),
        ring_enabled: fields.boolean("bRng").unwrap_or(false),
        ring_progress_time: fields.i32("RnPT").unwrap_or(1),
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
    let properties_start = node.child("PrpS").map(|props| parse_binder_properties(&props));
    let bind_point_id = properties_start
        .as_ref()
        .map(|props| props.bind_point_id)
        .unwrap_or(-1);
    AvfxBinder {
        binder_type: node.scalar("BnVr").unwrap_or(0),
        bind_point_id,
        start_to_global_direction: node.scalar("bStG").map(|value| value != 0).unwrap_or(false),
        vfx_scale_enabled: node.scalar("bVSc").map(|value| value != 0).unwrap_or(false),
        vfx_scale_bias: node.f32("bVSb").unwrap_or(0.0),
        vfx_scale_depth_offset: node.scalar("bVSd").map(|value| value != 0).unwrap_or(false),
        vfx_scale_interpolation: node.scalar("bVSi").map(|value| value != 0).unwrap_or(false),
        transform_scale: node.scalar_i32("bTSc").unwrap_or(0),
        transform_scale_depth_offset: node.scalar("bTSd").map(|value| value != 0).unwrap_or(false),
        transform_scale_interpolation: node.scalar("bTSi").map(|value| value != 0).unwrap_or(false),
        following_target_orientation: node.scalar("bFTO").map(|value| value != 0).unwrap_or(false),
        document_scale_enabled: node.scalar("bDSE").map(|value| value != 0).unwrap_or(false),
        adjust_to_screen_enabled: node.scalar("bATS").map(|value| value != 0).unwrap_or(false),
        ify: node.scalar("bIFY").map(|value| value != 0).unwrap_or(false),
        bet: node.scalar("bBET").map(|value| value != 0).unwrap_or(false),
        life: node.scalar_i32("Life").unwrap_or(0),
        rotation_type: node.scalar_i32("RoTp").unwrap_or(0),
        properties_start,
        properties_1: node.child("Prp1").map(|props| parse_binder_properties(&props)),
        properties_2: node.child("Prp2").map(|props| parse_binder_properties(&props)),
        properties_goal: node.child("PrpG").map(|props| parse_binder_properties(&props)),
    }
}

/// `Efct` 效果器最小解析：参数 + `Data` 原始负载。
fn parse_effector(node: &AvfxNodeView) -> AvfxEffector {
    AvfxEffector {
        raw_effector_type: node.scalar("EfVT").unwrap_or(u32::MAX),
        rotation_order: node.scalar_i32("RoOT").unwrap_or(0),
        coord_compute_order: node.scalar_i32("CCOT").unwrap_or(0),
        affect_other_vfx: node.scalar("bAOV").map(|value| value != 0).unwrap_or(false),
        affect_game: node.scalar("bAGm").map(|value| value != 0).unwrap_or(false),
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        data_payload: node
            .child("Data")
            .map(|data| data.payload().to_vec())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造块：名字按格式反写存储，不足 4 字节补空格。
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
                u32_block("PrCn", 0),
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
                color_curve(
                    "Col",
                    vec![key_bytes(0, 1, 1.0, 0.5, 0.25)],
                    None,
                    None,
                ),
                curve3("Pos", None, Some(vec![key_bytes(0, 1, 0.0, 0.0, 0.5)]), None),
                curve3("Rot", None, None, None),
                curve3("Scl", Some(vec![key_bytes(0, 1, 0.0, 0.0, 2.0)]), None, None),
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
                block("TLst", &[3u8]),
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
                curve3("Scl", Some(vec![key_bytes(0, 1, 0.0, 0.0, 0.2)]), None, None),
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
                block("VDrw", &draw_vertex),
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
        // TC1：TLst 优先于 TxNo。
        let tc1 = particle.texture_color1.as_ref().expect("TC1");
        assert!(tc1.enabled);
        assert_eq!(tc1.mask_texture_index, 3);
        assert_eq!(tc1.effective_texture(), 3);
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
        assert_eq!(draw.vertices.len(), 1);
        assert!((draw.vertices[0].position[0] - 0.5).abs() < 1.0e-3);
        assert!((draw.vertices[0].uv[0] - 0.25).abs() < 1.0e-3);
        assert_eq!(draw.indices, vec![0, 1, 2]);

        assert!(file.warnings.is_empty(), "{:?}", file.warnings);
        assert_eq!(file.unknown_blocks.len(), 0);
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
        assert!((curve.evaluate(15.0)[2] - 2.0).abs() < 1.0e-6);
        // 30..60 是 Step 段：保持左键值 3。
        assert!((curve.evaluate(45.0)[2] - 3.0).abs() < 1.0e-6);
        assert!((curve.evaluate(70.0)[2] - 5.0).abs() < 1.0e-6);

        let mut repeat = curve.clone();
        repeat.post_behavior = BEHAVIOR_REPEAT;
        // 90 帧回绕到 30：落在 Step 段保持 3。
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
        // 切线权重为 0 的样条退化为线性。
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
        assert!((curve.evaluate(30.0)[2] - 6.0).abs() < 1.0e-6);
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
    }
}

/// 武器/配件 MDL 的特效绑点（ElementId 表项）：avfx Binder 的 `BPID`
/// 引用 `id`（如武器的 3=基部 / 4=中部 / 5=尖部）。
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBindPoint {
    pub id: u32,
    pub translate: [f32; 3],
    pub rotate: [f32; 3],
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
    pub file: AvfxFile,
    /// 按文件 `Tex` 块顺序解码的贴图（RGBA8，取首层）；解码失败的项以
    /// `diagnostics` 记录、此处缺位（渲染端按索引回退程序化贴图）。
    pub textures: Vec<Option<VfxTextureRgba>>,
    pub diagnostics: Vec<String>,
}

impl WeaponVfxData {
    /// 构建常驻采样运行时（携带武器绑点表）。
    pub fn runtime(&self) -> crate::avfx_sim::VfxRuntime {
        crate::avfx_sim::VfxRuntime::with_bind_points(&self.file, &self.bind_points)
    }
}

/// 解码到 RGBA8 的 VFX 贴图（单层）。
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxTextureRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// 解码 `.atex` 贴图（avfx 内 `Tex` 块引用）。atex 在 `.tex` 负载前有
/// 8 字节头（`atex` 魔数 + 版本），tolerant：无魔数时按裸 tex 解析；
/// 数组贴图取第 0 层。此头布局假设待 Windows 真实数据审计复核。
#[cfg(feature = "game-data")]
pub fn decode_atex_rgba(bytes: &[u8]) -> Option<VfxTextureRgba> {
    let payload = bytes.strip_prefix(b"atex").map(|rest| &rest[4..]);
    let payload: &[u8] = match payload {
        Some(rest) if rest.len() >= 16 => rest,
        None => bytes,
        Some(_) => bytes,
    };
    let texture = <physis::tex::Texture as physis::ReadableFile>::from_existing(
        physis::Platform::Win32,
        payload,
    )?;
    let decoded = crate::texture_decode::decode_texture_rgba(&texture)?;
    let width = texture.width as usize;
    let layer_height = usize::from(texture.height).max(1);
    let layer_bytes = width.checked_mul(layer_height)?.checked_mul(4)?;
    let rgba = if decoded.len() >= layer_bytes {
        decoded[..layer_bytes].to_vec()
    } else {
        return None;
    };
    Some(VfxTextureRgba {
        width: texture.width as u32,
        height: layer_height as u32,
        rgba,
    })
}
