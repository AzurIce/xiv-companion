//! `.avfx`（武器/技能特效）读取器：递归块 walker + 类型化子集 + 曲线求值。
//!
//! 布局对齐 VFXEditor `VFXEditor/Formats/AvfxFormat/`（社区字段级事实标准）：
//! 文件为一个外层块（盘上名字为反写 `AVFX`），子块为 `[反写4字节名][u32 size]
//! [负载]`，size 按 4 字节对齐推进；叶子字段负载为 4 字节（i32/f32/枚举，
//! 个别 1 字节布尔），`Tex` 为 NUL 结尾字符串。`Schd`/`TmLn`/`Emit` 的
//! `Item`/`Trgr`/`ItPr`/`ItEm` 容器为累积式（最后一个容器含全部条目），且
//! 条目不带独立块包装，按固定步长（36/96 字节、312/300/288/276 字节）切分。
//! 未知块按名计数进 `unknown_blocks`，永不 panic。

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

/// 命名曲线：`BvPr`/`BvPo`/`RanT` 标量 + `Keys` 裸 16 字节键数组
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
    /// 在 `time`（帧）处求值，返回该键的 (x, y, z)。
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
}

/// 三次贝塞尔按 x 反解参数（8 次二分，单调段足够）后取 y。
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxEmitterItem {
    pub enabled: bool,
    pub target_index: i32,
    pub create_time: i32,
    pub create_count: i32,
    pub create_probability: i32,
    pub start_frame: i32,
    pub generate_delay: i32,
    /// ItPr 覆盖粒子寿命（`bOvr`/`OvrV`）。
    pub override_life: bool,
    pub override_life_value: i32,
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

/// 粒子 UV 动画集（`UvSt`）：缩放/滚动各为 2 轴曲线（取 x/y），旋转 1 轴（取 z）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxUvSet {
    pub calculate_uv: u32,
    pub scale: AvfxCurve,
    pub scroll: AvfxCurve,
    pub rotation: AvfxCurve,
    pub rotation_random: AvfxCurve,
}

/// 粒子颜色贴图引用（`TC1`，四张同构取其一的子集）。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticleTexture {
    pub enabled: bool,
    pub uv_set_index: i32,
    pub texture_index: i32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxScheduler {
    pub items: Vec<AvfxSchedulerItem>,
    /// 固定 12 个调度触发器（拔刀/收刀等）；语义社区未完全逆向，
    /// 常驻特效只消费 `items`。
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
    pub life: AvfxLife,
    pub create_count: AvfxCurve,
    pub create_interval: AvfxCurve,
    pub color: AvfxCurve,
    pub position: AvfxCurve,
    pub rotation: AvfxCurve,
    pub scale: AvfxCurve,
    pub particle_items: Vec<AvfxEmitterItem>,
    pub emitter_items: Vec<AvfxEmitterItem>,
    /// 圆锥发射数据（类型 1）。
    pub cone: Option<ConeEmitterData>,
    /// 球形模型发射数据（类型 3）。
    pub sphere_model: Option<SphereModelEmitterData>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConeEmitterData {
    pub inner_size: AvfxCurve,
    pub outer_size: AvfxCurve,
    pub injection_speed: AvfxCurve,
    pub injection_angle: AvfxCurve,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SphereModelEmitterData {
    pub generate_method: i32,
    pub divide_x: i32,
    pub divide_y: i32,
    pub radius: AvfxCurve,
    pub injection_speed: AvfxCurve,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxParticle {
    pub particle_type: Option<ParticleType>,
    pub raw_particle_type: u32,
    pub loop_start: i32,
    pub loop_end: i32,
    /// 绘制模式（VFXEditor `DrawMode`：0 Blend，2 Add……）。
    pub draw_mode: i32,
    pub depth_test: bool,
    pub depth_write: bool,
    pub life: AvfxLife,
    pub gravity: AvfxCurve,
    pub air_resistance: AvfxCurve,
    pub scale: AvfxCurve,
    pub rotation: AvfxCurve,
    pub position: AvfxCurve,
    pub color: AvfxCurve,
    pub rotation_velocity: [AvfxCurve; 3],
    pub texture_color1: Option<AvfxParticleTexture>,
    pub uv_sets: Vec<AvfxUvSet>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxBinder {
    /// 0 = Point（武器挂点绑定，其余类型原样记录）。
    pub binder_type: u32,
}

/// 解析后的 avfx 文件子集；未消费的根级块按名计数进 `unknown_blocks`。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvfxFile {
    pub version: u32,
    pub schedulers: Vec<AvfxScheduler>,
    pub timelines: Vec<AvfxTimeline>,
    pub emitters: Vec<AvfxEmitter>,
    pub particles: Vec<AvfxParticle>,
    pub binders: Vec<AvfxBinder>,
    /// 文件级 `Tex` 块的贴图路径（通常指向 .atex）。
    pub texture_paths: Vec<String>,
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
                "Tex" => file
                    .texture_paths
                    .push(read_null_terminated(node.payload())),
                _ => *unknown_blocks.entry(node.name().to_string()).or_default() += 1,
            }
        }
        file.warnings = warnings;
        file.unknown_blocks = unknown_blocks;
        Ok(file)
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
        let mut raw = [0_u8; 4];
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
        let mut raw = [0_u8; 4];
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
    let mut name = [0_u8; 4];
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
}

/// 遍历一块负载中的块。
fn child_blocks(bytes: &[u8]) -> impl Iterator<Item = AvfxNodeView<'_>> {
    AvfxChildIter { bytes, offset: 0 }
}

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
                create_time: fields.i32("CrTm").unwrap_or(1),
                create_count: fields.i32("CrCn").unwrap_or(1),
                create_probability: fields.i32("CrPr").unwrap_or(100),
                start_frame: fields.i32("StFr").unwrap_or(0),
                generate_delay: fields.i32("GenD").unwrap_or(0),
                override_life: fields.boolean("bOvr").unwrap_or(false),
                override_life_value: fields.i32("OvrV").unwrap_or(60),
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
        life: parse_optional_life(node),
        create_count: node
            .child("CrC")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        create_interval: node
            .child("CrI")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        color: node
            .child("Col")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        position: node
            .child("Pos")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        rotation: node
            .child("Rot")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        scale: node
            .child("Scl")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        ..Default::default()
    };
    emitter.emitter_type = Some(EmitterType::from_raw(emitter.raw_emitter_type));
    emitter.particle_items = parse_emitter_items(node, "ItPr");
    emitter.emitter_items = parse_emitter_items(node, "ItEm");
    if let Some(data) = node.child("Data") {
        match emitter.emitter_type {
            Some(EmitterType::Cone) => {
                emitter.cone = Some(ConeEmitterData {
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
                });
            }
            Some(EmitterType::SphereModel) => {
                emitter.sphere_model = Some(SphereModelEmitterData {
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
                });
            }
            // Point 本就无形状数据；其余类型（ConeModel/CylinderModel/Model）
            // 未建模时提示一次。
            Some(EmitterType::Point) | None => {}
            other => warnings.push(format!("emitter data for type {other:?} not modeled")),
        }
    }
    emitter
}

fn parse_particle(node: &AvfxNodeView, warnings: &mut Vec<String>) -> AvfxParticle {
    let mut particle = AvfxParticle {
        particle_type: None,
        raw_particle_type: node.scalar("PrVT").unwrap_or(u32::MAX),
        loop_start: node.scalar_i32("LpSt").unwrap_or(0),
        loop_end: node.scalar_i32("LpEd").unwrap_or(0),
        draw_mode: node.scalar_i32("RMT").unwrap_or(0),
        depth_test: node.scalar("DsDt").map(|value| value != 0).unwrap_or(true),
        depth_write: node.scalar("DsDw").map(|value| value != 0).unwrap_or(false),
        life: parse_optional_life(node),
        gravity: node
            .child("Gra")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        air_resistance: node
            .child("ARs")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        scale: node
            .child("Scl")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        rotation: node
            .child("Rot")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        position: node
            .child("Pos")
            .map(|child| parse_curve(&child))
            .unwrap_or_default(),
        color: node
            .child("Col")
            .map(|child| parse_curve(&child))
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
        texture_color1: node.child("TC1").map(|tc1| {
            let fields = Fields::walk(tc1.payload());
            AvfxParticleTexture {
                enabled: fields.boolean("bEna").unwrap_or(false),
                uv_set_index: fields.i32("UvSN").unwrap_or(0),
                texture_index: fields.i32("TxNo").unwrap_or(-1),
            }
        }),
        uv_sets: Vec::new(),
    };
    particle.particle_type = Some(ParticleType::from_raw(particle.raw_particle_type));
    for uv_set in node.children_named("UvSt") {
        particle.uv_sets.push(AvfxUvSet {
            calculate_uv: uv_set.scalar("CUvT").unwrap_or(0),
            scale: uv_set
                .child("Scl")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            scroll: uv_set
                .child("Scr")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            rotation: uv_set
                .child("Rot")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
            rotation_random: uv_set
                .child("RotR")
                .map(|child| parse_curve(&child))
                .unwrap_or_default(),
        });
    }
    match particle.particle_type {
        Some(
            ParticleType::Quad
            | ParticleType::Parameter
            | ParticleType::Powder
            | ParticleType::Line
            | ParticleType::Laser
            | ParticleType::Polyline,
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

fn parse_binder(node: &AvfxNodeView) -> AvfxBinder {
    AvfxBinder {
        binder_type: node.scalar("BnVr").unwrap_or(0),
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
    /// TC1 贴图 0）+ 1 binder + 1 tex 路径。
    fn synthetic_avfx() -> Vec<u8> {
        // 累积式写序：前面是空容器，最后一个容器含全部条目。
        let scheduler = container(
            "Schd",
            vec![
                u32_block("ItCn", 1),
                u32_block("TrCn", 12),
                container("Item", Vec::new()),
                container("Item", vec![scheduler_item(1, 0, -1)]),
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
                curve(
                    "Col",
                    vec![
                        key_bytes(0, 1, 1.0, 0.5, 0.25),
                        key_bytes(30, 1, 0.0, 0.0, 0.0),
                    ],
                    0,
                ),
                curve("Pos", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
                curve("Rot", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
                curve("Scl", vec![key_bytes(0, 1, 0.0, 0.0, 1.0)], 0),
                cone_data,
                container("ItPr", Vec::new()),
                container("ItPr", vec![emitter_item()]),
            ],
        );

        let uv_set = container(
            "UvSt",
            vec![
                u32_block("CUvT", 0),
                curve("Scl", vec![key_bytes(0, 1, 0.0, 0.5, 0.0)], 0),
                curve("Scr", vec![key_bytes(0, 1, 0.0, 0.25, 0.0)], 0),
                curve("Rot", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
            ],
        );
        let tc1 = container(
            "TC1",
            vec![
                u32_block("bEna", 1),
                u32_block("bC2A", 0),
                i32_block("UvSN", 0),
                i32_block("TxNo", 0),
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
                u32_block("DsDt", 1),
                u32_block("DsDw", 0),
                block("Life", &(-1_i32).to_le_bytes()),
                curve("Scl", vec![key_bytes(0, 1, 0.0, 0.0, 0.1)], 0),
                curve("Rot", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
                curve("Pos", vec![key_bytes(0, 1, 0.0, 0.0, 0.0)], 0),
                curve(
                    "Col",
                    vec![
                        key_bytes(0, 1, 1.0, 0.8, 0.4),
                        key_bytes(30, 1, 0.2, 0.1, 0.0),
                    ],
                    0,
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

        let root_children = vec![
            u32_block("Ver", 0x2011_0913),
            u32_block("ScCn", 1),
            u32_block("TlCn", 1),
            u32_block("EmCn", 1),
            u32_block("PrCn", 1),
            u32_block("EfCn", 0),
            u32_block("BdCn", 1),
            u32_block("TxCn", 1),
            block("Tex", b"vfx/eff/vws_test.atex\0"),
            scheduler,
            timeline,
            emitter,
            particle,
            binder,
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
        assert_eq!(scheduler.items[0].timeline_index, -1);
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
        assert!(emitter.cone.is_some());
        let cone = emitter.cone.as_ref().unwrap();
        assert!((cone.inner_size.evaluate(0.0)[2] - 0.1).abs() < 1.0e-6);

        let particle = &file.particles[0];
        assert_eq!(particle.particle_type, Some(ParticleType::Quad));
        assert_eq!(particle.draw_mode, 2);
        assert!(particle.texture_color1.as_ref().unwrap().enabled);
        assert_eq!(particle.uv_sets.len(), 1);
        assert!(particle.uv_sets[0].scroll.evaluate(0.0)[1] > 0.0);
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
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            ],
            ..Default::default()
        };
        let mid = curve.evaluate(15.0);
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

/// 武器挂载的常驻 VFX 数据：解析后的 avfx 子集 + 解码好的颜色贴图。
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponVfxData {
    /// 命中的 avfx 资源路径（`chara/weapon/.../vfx/eff/vw####.avfx`）。
    pub avfx_path: String,
    /// IMC 里的 VfxId。
    pub vfx_id: u8,
    pub file: AvfxFile,
    /// 按文件 `Tex` 块顺序解码的贴图（RGBA8，取首层）；解码失败的项以
    /// `diagnostics` 记录、此处缺位（渲染端按索引回退程序化贴图）。
    pub textures: Vec<Option<VfxTextureRgba>>,
    pub diagnostics: Vec<String>,
}

impl WeaponVfxData {
    /// 构建常驻采样运行时。
    pub fn runtime(&self) -> crate::avfx_sim::VfxRuntime {
        crate::avfx_sim::VfxRuntime::new(&self.file)
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
