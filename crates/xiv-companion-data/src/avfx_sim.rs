//! avfx 常驻特效的确定性 CPU 模拟：timeline items → emitter → 粒子四边形批次。
//!
//! v1 语义（对游戏运行时的近似，字段语义对齐 VFXEditor/社区逆向）：
//! - 帧率 30fps（avfx 时间轴单位为帧，见 `AVFX_FPS`）；
//! - 常驻路径直接遍历全部 timeline（scheduler `items` 即常驻 timeline 的
//!   起始表；12 个 trigger 的拔刀/收刀切换不在 v1 范围）；
//! - timeline 在 `[LpSt, LpEd]` 内循环，item 按启用窗口驱动 emitter；
//! - emitter 的创建事件按 `CrI`（间隔曲线）整数倍对齐，一次事件生成
//!   `CrC`（数量曲线）× `ItPr.CrCn` 个粒子，粒子状态由出生时间解析式
//!   计算（无增量状态，任意时刻采样一致，可单测/快照）；
//! - 形状支持 Point / Cone（XZ 圆盘 + 沿 Y 注入）/ SphereModel（球面 +
//!   径向注入），其余回退为点；
//! - 随机数用 splitmix64 按 (timeline, item, emitter, 事件, 粒子) 播种，
//!   确定性可复现。

use crate::avfx::{AvfxCurve, AvfxEmitter, AvfxEmitterItem, AvfxFile, AvfxParticle, AvfxTimeline};

/// avfx 时间轴帧率；VFXEditor 默认 30fps，帧 → 秒换算只用这里。
pub const AVFX_FPS: f32 = 30.0;

/// 单个粒子四边形（渲染端实例数据，世界单位）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxQuad {
    pub position: [f32; 3],
    /// 宽/高（世界单位，粒子 scale 的 x/y）。
    pub size: [f32; 2],
    /// 面内旋转（弧度）。
    pub rotation: f32,
    /// HDR 颜色（rgb 可 >1，a 为不透明度）。
    pub color: [f32; 4],
    /// 贴图帧基点 + 缩放（UVSet scroll/scale 求值结果）。
    pub uv_origin: [f32; 2],
    pub uv_scale: [f32; 2],
    /// 文件级贴图索引（TC1.TxNo；渲染端据此选纹理，-1 = 程序化回退）。
    pub texture_index: i32,
}

/// 网格粒子实例：一次生成 = 整个 DrawModel 网格按实例变换绘制
/// （Model/LightModel 粒子的本体渲染路径，龙形火舌等轮廓来源）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxMeshInstance {
    /// 网格实例原点（世界单位；网格顶点为武器本地坐标）。
    pub position: [f32; 3],
    pub scale: f32,
    /// HDR 颜色（rgb 可 >1，a 为不透明度）。
    pub color: [f32; 4],
    /// 文件级贴图索引（TC1.TxNo；渲染端据此选纹理组）。
    pub texture_index: i32,
    /// 内嵌绘制模型序号（文件 `Modl` Draw 半对顺序）。
    pub model_index: usize,
}

/// 从解析后的 avfx 构建的采样运行时（拷贝子集，构建后与文件解耦）。
#[derive(Debug, Clone)]
pub struct VfxRuntime {
    timelines: Vec<AvfxTimeline>,
    emitters: Vec<AvfxEmitter>,
    particles: Vec<AvfxParticle>,
    /// 内嵌发射模型顶点（Model/LightModel 粒子的可见几何）。
    models: Vec<crate::avfx::VfxModelGeometry>,
    /// 采样硬上限，超出丢弃（child_limit 语义的保守版）。
    max_quads: usize,
}

impl VfxRuntime {
    pub fn new(file: &AvfxFile) -> Self {
        Self {
            timelines: file.timelines.clone(),
            emitters: file.emitters.clone(),
            particles: file.particles.clone(),
            models: file.models.clone(),
            max_quads: 4096,
        }
    }

    pub fn max_quads(&self) -> usize {
        self.max_quads
    }

    /// 采样 `time_seconds` 时刻的全部存活粒子四边形（out 先清空）。
    pub fn sample(&self, time_seconds: f32, out: &mut Vec<VfxQuad>) {
        out.clear();
        let frame = time_seconds * AVFX_FPS;
        for timeline in &self.timelines {
            let (loop_start, loop_end) = timeline_span(timeline);
            let local = loop_start + (frame - loop_start).rem_euclid(loop_end - loop_start);
            for (item_index, item) in timeline.items.iter().enumerate() {
                if !item.enabled || item.emitter_index < 0 {
                    continue;
                }
                if local < item.start_time as f32 {
                    continue;
                }
                if item.end_time >= 0 && local > item.end_time as f32 {
                    continue;
                }
                let Some(emitter) = self.emitters.get(item.emitter_index as usize) else {
                    continue;
                };
                self.sample_emitter(
                    item_index,
                    item,
                    emitter,
                    local - item.start_time as f32,
                    out,
                );
                if out.len() >= self.max_quads {
                    out.truncate(self.max_quads);
                    return;
                }
            }
        }
    }

    /// 采样 `time_seconds` 时刻的网格粒子实例（Model/LightModel 且发射器
    /// 带绘制网格时走此路径；`out` 先清空）。
    pub fn sample_mesh(&self, time_seconds: f32, out: &mut Vec<VfxMeshInstance>) {
        out.clear();
        let frame = time_seconds * AVFX_FPS;
        for timeline in &self.timelines {
            let (loop_start, loop_end) = timeline_span(timeline);
            let local = loop_start + (frame - loop_start).rem_euclid(loop_end - loop_start);
            for (item_index, item) in timeline.items.iter().enumerate() {
                if !item.enabled || item.emitter_index < 0 {
                    continue;
                }
                if local < item.start_time as f32 {
                    continue;
                }
                if item.end_time >= 0 && local > item.end_time as f32 {
                    continue;
                }
                let Some(emitter) = self.emitters.get(item.emitter_index as usize) else {
                    continue;
                };
                self.sample_mesh_emitter(item_index, emitter, local - item.start_time as f32, out);
                if out.len() >= self.max_quads {
                    out.truncate(self.max_quads);
                    return;
                }
            }
        }
    }

    fn sample_mesh_emitter(
        &self,
        item_index: usize,
        emitter: &AvfxEmitter,
        emitter_frame: f32,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        // 仅 Model 型发射器且引用的绘制网格存在时。
        let Some(model_index) = emitter.model_index.filter(|index| *index >= 0) else {
            return;
        };
        let Some(geometry) = self.models.get(model_index as usize) else {
            return;
        };
        let Some(draw) = geometry
            .draw
            .as_ref()
            .filter(|draw| !draw.indices.is_empty())
        else {
            return;
        };
        let emitter_offset = emitter.position.evaluate(emitter_frame);
        let emitter_cap = out.len() + emitter.child_limit.clamp(1, 48) as usize;

        for particle_item in &emitter.particle_items {
            if !particle_item.enabled || particle_item.target_index < 0 {
                continue;
            }
            let Some(particle) = self.particles.get(particle_item.target_index as usize) else {
                continue;
            };
            let life_frames = if particle.life.enabled && particle.life.value > 0.0 {
                particle.life.value
            } else if emitter.life.enabled && emitter.life.value > 0.0 {
                emitter.life.value
            } else {
                MAX_PARTICLE_LIFE
            }
            .clamp(1.0, MAX_PARTICLE_LIFE);
            let interval = if particle_item.create_time > 0 {
                particle_item.create_time as f32
            } else {
                emitter.create_interval.evaluate(emitter_frame)[2].max(1.0)
            };
            let first_event = ((emitter_frame - life_frames) / interval).ceil().max(0.0);
            let last_event = (emitter_frame / interval).floor();
            if last_event < first_event {
                continue;
            }
            let per_event = emitter.create_count.evaluate(emitter_frame)[2].round()
                * particle_item.create_count.max(0) as f32;
            let per_event = per_event.max(1.0).round().clamp(1.0, 16.0) as u32;
            for event in (first_event as u64)..=(last_event as u64) {
                let spawn_frame = event as f32 * interval;
                if spawn_frame > emitter_frame {
                    continue;
                }
                let age = emitter_frame - spawn_frame;
                for particle_index in 0..per_event {
                    if out.len() >= emitter_cap {
                        return;
                    }
                    let mut rng = SplitMix64::seeded(
                        item_index as u64,
                        event,
                        particle_item.target_index as u64,
                        particle_index as u64,
                    );
                    let alpha = (1.0 - age / life_frames) * APPROX_ALPHA_SCALE * 5.0;
                    let rgb = if particle.color.keys.is_empty() {
                        [1.0, 1.0, 1.0]
                    } else {
                        particle.color.evaluate(age)
                    };
                    // 网格保持授权坐标（Y 翻转后自发射点向 +Y 延伸，恰好
                    // 覆盖刃部并伸出锋外，与游戏内观感一致）。
                    let spawn = particle.position.evaluate(age);
                    let position = [
                        emitter_offset[0] + spawn[0],
                        emitter_offset[1] + spawn[1],
                        emitter_offset[2] + spawn[2],
                    ];
                    let texture_index = particle
                        .texture_color1
                        .as_ref()
                        .filter(|texture| texture.enabled)
                        .map(|texture| texture.texture_index)
                        .unwrap_or(-1);
                    out.push(VfxMeshInstance {
                        position,
                        scale: 1.0,
                        color: [rgb[0], rgb[1], rgb[2], alpha],
                        texture_index,
                        model_index: model_index as usize,
                    });
                }
            }
        }
    }

    fn sample_emitter(
        &self,
        item_index: usize,
        item: &crate::avfx::AvfxTimelineItem,
        emitter: &AvfxEmitter,
        emitter_frame: f32,
        out: &mut Vec<VfxQuad>,
    ) {
        if emitter.particle_items.is_empty() {
            return;
        }
        let emitter_offset = emitter.position.evaluate(emitter_frame);
        let has_draw_mesh = emitter
            .model_index
            .and_then(|index| self.models.get(index as usize))
            .and_then(|geometry| geometry.draw.as_ref())
            .is_some();
        let model_vertices = emitter
            .model_index
            .and_then(|index| self.models.get(index as usize))
            .map(|geometry| geometry.positions.as_slice())
            .unwrap_or(&[]);
        // child_limit：单 emitter 存活粒子数上限（缺省 0 → 48）。
        let emitter_cap = out.len() + emitter.child_limit.clamp(1, 12) as usize;

        for particle_item in &emitter.particle_items {
            if !particle_item.enabled || particle_item.target_index < 0 {
                continue;
            }
            let Some(particle) = self.particles.get(particle_item.target_index as usize) else {
                continue;
            };
            // 带绘制网格的 Model/LightModel 粒子走 sample_mesh 路径，四边形流跳过。
            if has_draw_mesh
                && matches!(
                    particle.particle_type,
                    Some(crate::avfx::ParticleType::Model)
                        | Some(crate::avfx::ParticleType::LightModel)
                )
            {
                continue;
            }
            // 生命周期（真实文件里 Life 常写 -1 = 跟随 timeline）：ItPr 的
            // Override 优先，其次 particle Life、emitter Life；无有效值或
            // <=0 视为跟随循环，clamp 到上限防止无穷累积。
            let life_frames = resolve_life_frames(particle_item, particle, emitter);
            // 发射节奏（真实文件普遍用 ItPr.CrTm/CrCn；CrI/CrC 是 emitter
            // 级连续发射，缺省时回退到它）：出生帧 = 起始 + k × 间隔，k 覆盖
            // 整个生命窗口，任意时刻解析出同一批存活粒子。
            let interval = if particle_item.create_time > 0 {
                particle_item.create_time as f32
            } else {
                emitter.create_interval.evaluate(emitter_frame)[2].max(1.0)
            };
            let first_event = ((emitter_frame - life_frames) / interval).ceil().max(0.0);
            let last_event = (emitter_frame / interval).floor();
            if last_event < first_event {
                continue;
            }
            let per_event = emitter.create_count.evaluate(emitter_frame)[2].round()
                * particle_item.create_count.max(0) as f32;
            let per_event = per_event.max(1.0).round().clamp(1.0, 64.0) as u32;
            for event in (first_event as u64)..=(last_event as u64) {
                let spawn_frame = event as f32 * interval;
                if spawn_frame > emitter_frame {
                    continue;
                }
                let age = emitter_frame - spawn_frame;
                for particle_index in 0..per_event {
                    if out.len() >= emitter_cap {
                        break;
                    }
                    let mut rng = SplitMix64::seeded(
                        item_index as u64,
                        event,
                        particle_item.target_index as u64,
                        particle_index as u64,
                    );
                    // EmitterType.Model/…Model 系：发射点在内嵌模型顶点上
                    // （与粒子类型无关），覆盖整个贴片几何；其余走形状采样。
                    let (shape_offset, injection_dir, injection_speed) = if !model_vertices
                        .is_empty()
                        && matches!(
                            emitter.emitter_type,
                            Some(
                                crate::avfx::EmitterType::Model
                                    | crate::avfx::EmitterType::ConeModel
                                    | crate::avfx::EmitterType::CylinderModel
                                    | crate::avfx::EmitterType::SphereModel
                            )
                        ) {
                        let vertex = model_vertices
                            [(particle_index as usize + event as usize) % model_vertices.len()];
                        (vertex, [0.0, 1.0, 0.0], 0.0)
                    } else {
                        sample_shape(emitter, &mut rng)
                    };
                    self.sample_particle(
                        particle,
                        age,
                        life_frames,
                        &emitter_offset,
                        &shape_offset,
                        injection_dir,
                        injection_speed * age,
                        &mut rng,
                        out,
                    );
                    if out.len() >= self.max_quads {
                        out.truncate(self.max_quads);
                        return;
                    }
                }
            }
        }
    }

    fn sample_particle(
        &self,
        particle: &AvfxParticle,
        age: f32,
        life_frames: f32,
        emitter_offset: &[f32; 3],
        shape_offset: &[f32; 3],
        injection_dir: [f32; 3],
        injection_distance: f32,
        rng: &mut SplitMix64,
        out: &mut Vec<VfxQuad>,
    ) {
        if age > life_frames {
            return;
        }
        let alpha = 1.0 - age / life_frames;

        // 位置 = 发射器曲线 + 形状偏移 + 注入位移 + 重力半积分（-Y）。
        let curve_position = particle.position.evaluate(age);
        let gravity = particle.gravity.evaluate(age)[2];
        let position = [
            curve_position[0]
                + emitter_offset[0]
                + shape_offset[0]
                + injection_dir[0] * injection_distance,
            curve_position[1]
                + emitter_offset[1]
                + shape_offset[1]
                + injection_dir[1] * injection_distance
                - 0.5 * gravity * age * age,
            curve_position[2]
                + emitter_offset[2]
                + shape_offset[2]
                + injection_dir[2] * injection_distance,
        ];

        // 缺省曲线语义（真实武器特效普遍不写这些块）：color 缺省为白。
        // 尺寸：Quad 类用 scale 曲线原值（缺省小亮片）；Model/LightModel 等
        // 贴图层近似的粒子本体尺寸来自内嵌几何，游戏内约指关节大小，此处以
        // APPROXimated 半径 × scale 系数（缺省 1）近似，避免把 1.0 系数当成
        // 1 米边长。
        let uses_raw_scale = matches!(
            particle.particle_type,
            Some(crate::avfx::ParticleType::Quad)
                | Some(crate::avfx::ParticleType::Powder)
                | Some(crate::avfx::ParticleType::Disc)
                | Some(crate::avfx::ParticleType::Polygon)
        );
        // 尺寸：游戏缺省粒子 scale = 1（1 米级软贴片，灵光/烟雾类特效的
        // 主要观感）；贴图层近似（模型顶点落位）取中尺寸避免顶点间大 overlap。
        let scale = if particle.scale.keys.is_empty() {
            // 点发射的 Quad 在灵光类特效里是小星光；大软贴片由
            // 有 Scl 曲线的粒子或网格本体承担。
            [0.08, 0.08, 1.0]
        } else {
            particle.scale.evaluate(age)
        };
        let size = if uses_raw_scale {
            [scale[0].abs().max(1.0e-4), scale[1].abs().max(1.0e-4)]
        } else {
            [
                (DEFAULT_QUAD_SIZE * 6.0) * scale[0].abs().max(1.0e-4),
                (DEFAULT_QUAD_SIZE * 6.0) * scale[1].abs().max(1.0e-4),
            ]
        };

        // 颜色 = 粒子 Color 曲线（缺省白；HDR 不截断，饱和交给 bloom）。
        let rgb = if particle.color.keys.is_empty() {
            [1.0, 1.0, 1.0]
        } else {
            particle.color.evaluate(age)
        };
        let color = [rgb[0], rgb[1], rgb[2], alpha * APPROX_ALPHA_SCALE];

        // UV：首个 UVSet 的 scroll（x/y，sampler Repeat 寻址环绕）与 scale。
        // 真实文件里 Scale 常量为 0（= 不缩放 = 单帧全贴图），按 1 处理，
        // 否则 UV 坍缩到单点、加色输出恒 0（粒子隐形）。
        let (uv_origin, uv_scale) = match particle.uv_sets.first() {
            Some(uv_set) => {
                let scroll = uv_set.scroll.evaluate(age);
                let scale = uv_set.scale.evaluate(age);
                let scale_x = if scale[0].abs() > 1.0e-4 {
                    scale[0]
                } else {
                    1.0
                };
                let scale_y = if scale[1].abs() > 1.0e-4 {
                    scale[1]
                } else {
                    1.0
                };
                ([scroll[0], scroll[1]], [scale_x, scale_y])
            }
            None => ([0.0, 0.0], [1.0, 1.0]),
        };

        // 面内旋转：Rot 曲线 z + z 角速度积分近似；随机抖动叠加 ±ε。
        let mut rotation = particle.rotation.evaluate(age)[2];
        rotation += particle.rotation_velocity[2].evaluate(age)[2] * age;
        rotation += (rng.next_f32() - 0.5) * 1.0e-3;

        let texture_index = particle
            .texture_color1
            .as_ref()
            .filter(|texture| texture.enabled)
            .map(|texture| texture.texture_index)
            .unwrap_or(-1);

        out.push(VfxQuad {
            position,
            size,
            rotation,
            color,
            uv_origin,
            uv_scale,
            texture_index,
        });
    }
}

/// 无界 item（end_time=-1）与空循环区间（LpSt=LpEd=0，武器常驻特效普遍
/// 如此）的近似循环长度（帧）；游戏内由 scheduler 触发结束，此处取 4 秒。
const DEFAULT_TIMELINE_SPAN: f32 = 120.0;

/// timeline 循环区间；item 的 end_time=-1 视为无界（到循环尾）。
fn timeline_span(timeline: &AvfxTimeline) -> (f32, f32) {
    let mut start = timeline.loop_start as f32;
    let mut end = if timeline.loop_end > 0 {
        timeline.loop_end as f32
    } else {
        0.0
    };
    for item in &timeline.items {
        if !item.enabled || item.end_time < 0 {
            continue;
        }
        start = start.min(item.start_time as f32);
        end = end.max(item.end_time as f32);
    }
    if end <= start {
        end = start + DEFAULT_TIMELINE_SPAN;
    }
    (start, end)
}

/// 无有效 Life 时的粒子寿命上限（帧）：跟随 timeline 的粒子 clamp 到
/// 3 秒，防止无界累积（配合加色 + bloom 的密度上限）。
const MAX_PARTICLE_LIFE: f32 = 45.0;
/// 缺省粒子四边形尺寸（世界单位；Quad 类无 scale 曲线时的亮片大小）。
const DEFAULT_QUAD_SIZE: f32 = 0.05;
/// Point 发射器近似散布半径（世界单位）：贴图层近似粒子的体积感。
const POINT_SCATTER_RADIUS: f32 = 0.26;
/// 加色 + bloom 下的全局透明度折减（近似 HDR 曝光，避免叠成过曝白团）。
const APPROX_ALPHA_SCALE: f32 = 0.22;

/// ItPr.Override > particle Life > emitter Life；<=0（-1 = 跟随 timeline）
/// 视为无界并 clamp。
fn resolve_life_frames(
    item: &AvfxEmitterItem,
    particle: &AvfxParticle,
    emitter: &AvfxEmitter,
) -> f32 {
    let value = if item.override_life && item.override_life_value > 0 {
        item.override_life_value as f32
    } else if particle.life.enabled && particle.life.value > 0.0 {
        particle.life.value
    } else if emitter.life.enabled && emitter.life.value > 0.0 {
        emitter.life.value
    } else {
        MAX_PARTICLE_LIFE
    };
    value.clamp(1.0, MAX_PARTICLE_LIFE)
}

/// 发射器形状采样：返回 (形状偏移, 注入方向, 注入速度)。
fn sample_shape(emitter: &AvfxEmitter, rng: &mut SplitMix64) -> ([f32; 3], [f32; 3], f32) {
    let frame = 0.0; // 形状曲线随 emitter 时间变化时在此传参（v1 取常量键）。
    let _ = frame;
    match (&emitter.emitter_type, &emitter.cone, &emitter.sphere_model) {
        (Some(crate::avfx::EmitterType::Cone), Some(cone), _) => {
            let inner = cone.inner_size.evaluate(0.0)[2];
            let outer = cone.outer_size.evaluate(0.0)[2];
            let radius = inner + (outer - inner) * rng.next_f32();
            let theta = rng.next_f32() * std::f32::consts::TAU;
            let speed = cone.injection_speed.evaluate(0.0)[2];
            (
                [radius * theta.cos(), 0.0, radius * theta.sin()],
                [0.0, 1.0, 0.0],
                speed,
            )
        }
        (Some(crate::avfx::EmitterType::SphereModel), _, Some(sphere)) => {
            let radius = sphere.radius.evaluate(0.0)[2];
            let phi = rng.next_f32() * std::f32::consts::TAU;
            let cos_theta = 2.0 * rng.next_f32() - 1.0;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let dir = [sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin()];
            let speed = sphere.injection_speed.evaluate(0.0)[2];
            (
                [dir[0] * radius, dir[1] * radius, dir[2] * radius],
                dir,
                speed,
            )
        }
        // Point 与未建模形状：贴点发射（近似给小半径球面散布，避免
        // Model/LightModel 近似粒子全部重叠在原点）。
        _ => {
            let phi = rng.next_f32() * std::f32::consts::TAU;
            let cos_theta = 2.0 * rng.next_f32() - 1.0;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let radius = POINT_SCATTER_RADIUS * rng.next_f32();
            (
                [
                    sin_theta * phi.cos() * radius,
                    cos_theta * radius,
                    sin_theta * phi.sin() * radius,
                ],
                [0.0, 1.0, 0.0],
                0.0,
            )
        }
    }
}

/// 确定性随机（splitmix64）：同种子序列恒同输出。
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// 四个索引混合成种子（黄金比例常数旋转混合，避免结构性零）。
    fn seeded(a: u64, b: u64, c: u64, d: u64) -> Self {
        let mut state = a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ b.rotate_left(17)
            ^ c.rotate_left(33)
            ^ d.rotate_left(49);
        if state == 0 {
            state = 0x9E37_79B9_7F4A_7C15;
        }
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// [0, 1) 均匀。
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1_u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{
        AvfxBinder, AvfxCurveKey, AvfxLife, AvfxParticleTexture, AvfxScheduler, AvfxSchedulerItem,
        AvfxTimelineItem, AvfxUvSet, ConeEmitterData, EmitterType, ParticleType,
    };

    fn linear_curve(values: &[(i16, f32)]) -> AvfxCurve {
        axis_curve(values, |v| [0.0, 0.0, v])
    }

    /// 2 轴曲线（UVSet scale/scroll）：数值在键的 x/y。
    fn uv_axis_curve(values: &[(i16, f32)]) -> AvfxCurve {
        axis_curve(values, |v| [v, v, 0.0])
    }

    fn axis_curve(values: &[(i16, f32)], axes: impl Fn(f32) -> [f32; 3]) -> AvfxCurve {
        AvfxCurve {
            keys: values
                .iter()
                .map(|&(time, value)| {
                    let [x, y, z] = axes(value);
                    AvfxCurveKey {
                        time,
                        interpolation: 1,
                        x,
                        y,
                        z,
                    }
                })
                .collect(),
            ..Default::default()
        }
    }

    fn fixture_file() -> AvfxFile {
        AvfxFile {
            version: 0x2011_0913,
            schedulers: vec![AvfxScheduler {
                items: vec![AvfxSchedulerItem {
                    enabled: true,
                    start_time: 0,
                    timeline_index: 0,
                }],
                triggers: Vec::new(),
            }],
            timelines: vec![AvfxTimeline {
                loop_start: 0,
                loop_end: 120,
                binder_index: 0,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: 120,
                    binder_index: -1,
                    effector_index: -1,
                    emitter_index: 0,
                    platform: 0,
                    clip_index: -1,
                }],
                clip_count: 0,
            }],
            emitters: vec![AvfxEmitter {
                emitter_type: Some(EmitterType::Cone),
                raw_emitter_type: 1,
                model_index: None,
                loop_start: 0,
                loop_end: 120,
                child_limit: 128,
                particle_count: 0,
                emitter_count: 0,
                life: AvfxLife {
                    enabled: true,
                    value: 30.0,
                    value_random: 0.0,
                    random_type: 0,
                },
                create_count: linear_curve(&[(0, 2.0)]),
                create_interval: linear_curve(&[(0, 15.0)]),
                color: linear_curve(&[(0, 1.0)]),
                position: linear_curve(&[(0, 0.0)]),
                rotation: linear_curve(&[(0, 0.0)]),
                scale: linear_curve(&[(0, 1.0)]),
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    create_time: 15,
                    create_count: 3,
                    create_probability: 100,
                    start_frame: 0,
                    generate_delay: 0,
                    override_life: false,
                    override_life_value: 60,
                }],
                emitter_items: Vec::new(),
                cone: Some(ConeEmitterData {
                    inner_size: linear_curve(&[(0, 0.1)]),
                    outer_size: linear_curve(&[(0, 0.5)]),
                    injection_speed: linear_curve(&[(0, 0.03)]),
                    injection_angle: linear_curve(&[(0, 0.0)]),
                }),
                sphere_model: None,
            }],
            particles: vec![AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                raw_particle_type: 8,
                loop_start: 0,
                loop_end: 30,
                draw_mode: 2,
                depth_test: true,
                depth_write: false,
                life: AvfxLife::default(),
                gravity: linear_curve(&[(0, 0.0)]),
                air_resistance: linear_curve(&[(0, 0.0)]),
                scale: axis_curve(&[(0, 0.2)], |v| [v, v, 0.0]),
                rotation: linear_curve(&[(0, 0.0)]),
                position: linear_curve(&[(0, 0.0)]),
                color: linear_curve(&[(0, 0.8)]),
                rotation_velocity: Default::default(),
                texture_color1: Some(AvfxParticleTexture {
                    enabled: true,
                    uv_set_index: 0,
                    texture_index: 2,
                }),
                uv_sets: vec![AvfxUvSet {
                    calculate_uv: 0,
                    scale: uv_axis_curve(&[(0, 1.0)]),
                    scroll: uv_axis_curve(&[(0, 0.25)]),
                    rotation: linear_curve(&[(0, 0.0)]),
                    rotation_random: linear_curve(&[(0, 0.0)]),
                }],
            }],
            binders: vec![AvfxBinder { binder_type: 0 }],
            models: Vec::new(),
            texture_paths: vec!["vfx/eff/test.atex".to_string()],
            warnings: Vec::new(),
            unknown_blocks: Default::default(),
        }
    }

    #[test]
    fn samples_deterministic_particle_batches() {
        let file = fixture_file();
        let runtime = VfxRuntime::new(&file);
        let mut quads_a = Vec::new();
        let mut quads_b = Vec::new();
        runtime.sample(1.0, &mut quads_a);
        runtime.sample(1.0, &mut quads_b);
        // 确定性：同时刻两次采样完全一致。
        assert_eq!(quads_a, quads_b);
        // 间隔 15 帧、生命 30 帧：30 帧处恰有 3 个事件（k=0,1,2，边界
        // 粒子视为存活）；每事件 CrC=2 × ItPr.CrCn=3 = 6 粒子。
        // child_limit 钳制上限（fixture emitter 128 → 采样上限 12）。
        assert_eq!(quads_a.len(), 12);

        // 颜色/贴图/UV/尺寸语义落位。
        let quad = &quads_a[0];
        assert!((quad.color[2] - 0.8).abs() < 1.0e-6);
        assert_eq!(quad.texture_index, 2);
        assert!((quad.uv_scale[0] - 1.0).abs() < 1.0e-6);
        assert!((quad.size[0] - 0.2).abs() < 1.0e-6);
    }

    #[test]
    fn fades_with_age_and_loops_timeline() {
        let file = fixture_file();
        let runtime = VfxRuntime::new(&file);
        let mut at_zero = Vec::new();
        let mut mid = Vec::new();
        let mut looped = Vec::new();
        runtime.sample(0.0, &mut at_zero);
        runtime.sample(0.5, &mut mid);
        // t=0 只有一个事件批，全部新生（alpha = 1 × 全局折减）。
        assert_eq!(at_zero.len(), 6);
        assert!(
            at_zero
                .iter()
                .all(|quad| (quad.color[3] - APPROX_ALPHA_SCALE).abs() < 1.0e-6)
        );
        // t=0.5s（15 帧）出现老一批（alpha≈0.5）。
        assert!(
            mid.iter()
                .any(|quad| (quad.color[3] - 0.5 * APPROX_ALPHA_SCALE).abs() < 0.01)
        );

        // 时间轴 120 帧 = 4 秒循环：t=0 与 t=4s 的粒子集合一致。
        runtime.sample(4.0, &mut looped);
        assert_eq!(at_zero, looped);
    }

    #[test]
    fn cone_positions_stay_within_bounds_and_inject_upward() {
        let file = fixture_file();
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(0.9, &mut quads);
        assert!(!quads.is_empty());
        for quad in &quads {
            let radial =
                (quad.position[0] * quad.position[0] + quad.position[2] * quad.position[2]).sqrt();
            // 圆盘半径 ≤ 外径 0.5。
            assert!(radial <= 0.5 + 1.0e-3, "radial {radial} beyond cone outer");
            // 注入沿 +Y：老粒子更高（同批内至少存在非零 Y）。
            assert!(quad.position[1] >= -1.0e-5);
        }
        let max_y = quads
            .iter()
            .map(|quad| quad.position[1])
            .fold(f32::MIN, f32::max);
        assert!(
            max_y > 0.1,
            "no injected particles above origin (max {max_y})"
        );
    }

    #[test]
    fn empty_file_samples_nothing() {
        let runtime = VfxRuntime::new(&AvfxFile::default());
        let mut quads = Vec::new();
        runtime.sample(1.0, &mut quads);
        assert!(quads.is_empty());
    }
}
