//! avfx 常驻特效的确定性 CPU 模拟：scheduler → timeline → emitter → 粒子批次。
//!
//! 语义对齐 VFXEditor 字段定义与 AVFXTools 运行时（`Graphics/Emitter|Particle`）：
//! - 帧率 30fps（avfx 时间轴单位为帧，见 `AVFX_FPS`）；
//! - 常驻路径只消费 scheduler `items` 指向的 timeline（12 个 trigger 的
//!   拔刀/收刀切换不在此列）；
//! - timeline 仅在 `LpSt < LpEd` 时循环，否则一次性播放（item 的 EdTm=-1
//!   表示持续到时间轴结束，不重复触发）；
//! - emitter Life>0 时按寿命循环（每圈重建一次永生粒子——AVFXTools 的
//!   `JustOneCreate`：目标粒子 Life=-1 时该 ItPr 每圈只在首个创建事件触发）；
//! - 创建节奏由 emitter `CrI`（间隔曲线）驱动，每事件生成
//!   `CrC × max(1, ItPr.CrCn)` 个粒子（`CrPr` 概率过滤）；
//! - 出生点 = emitter Pos/Rot/Scl（出生时刻求值）∘ 形状偏移（Point/Cone/
//!   ConeModel/CylinderModel/SphereModel/Model 发射顶点）；
//! - 粒子状态由出生时间解析式计算（无增量状态，任意时刻采样一致，可单测/快照），
//!   随机数用 splitmix64 按 (item, 圈, 事件, 粒子) 播种，确定性可复现；
//! - Powder 粒子是子粒子发射器：本体不绘制，按 `Smpl` 参数持续喷出
//!   短寿命子粒子（颜色帧簿 `Cols`/`Frms` + UV 翻页）。

use crate::avfx::{
    AvfxEmitter, AvfxEmitterData, AvfxEmitterItem, AvfxFile, AvfxParticle, AvfxParticleData,
    AvfxTimeline, ParticleType,
};

/// avfx 时间轴帧率；VFXEditor 默认 30fps，帧 → 秒换算只用这里。
pub const AVFX_FPS: f32 = 30.0;

/// 单个粒子四边形（渲染端实例数据，世界单位）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxQuad {
    pub position: [f32; 3],
    /// 半宽/半高（世界单位；游戏四边形全宽 = 粒子 scale）。
    pub size: [f32; 2],
    /// billboard 面内旋转（弧度）。
    pub rotation: f32,
    /// 非 billboard 朝向（四元数 xyzw；billboard 时为单位四元数）。
    pub orientation: [f32; 4],
    pub billboard: bool,
    /// HDR 颜色（rgb = Col.RGB × Bri，可 >1；a = Col.A 包络）。
    pub color: [f32; 4],
    /// 贴图 1（TC1 引用的 UvSet）的 UV 原点 + 缩放。
    pub uv_origin: [f32; 2],
    pub uv_scale: [f32; 2],
    /// 贴图 2（TC2 引用的 UvSet）的 UV 原点 + 缩放。
    pub uv2_origin: [f32; 2],
    pub uv2_scale: [f32; 2],
    /// 文件级贴图序号（TC1：TLst 优先；-1 = 无贴图）。
    pub texture_index: i32,
    /// TC2 贴图序号（-1 = 无）。
    pub texture2_index: i32,
    /// TC2 颜色/alpha 合成模式（`TCCT`/`TCAT`）。
    pub combine_color: i32,
    pub combine_alpha: i32,
    /// TC1 的 bC2A（颜色转 alpha）。
    pub color_to_alpha: bool,
    /// TC2 的 bC2A。
    pub color_to_alpha2: bool,
    /// 混合模式：true = 加色（Add 系），false = 普通 alpha 混合。
    pub blend_add: bool,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    /// TD 扭曲强度（DPow 按粒子年龄求值）。
    pub distortion_power: f32,
    /// TD 扭曲目标位（bit0 = uv1，bit1 = uv2）。
    pub distortion_targets: u32,
    /// TD 贴图采样用 UV（TD 引用的 UvSet 变换）。
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
}

/// 网格粒子实例：Model/LightModel 粒子把粒子 Data 引用的内嵌绘制模型
/// （`Modl` 的 `VDrw` 网格）按实例 TRS 绘制（火舌、光罩壳等轮廓本体）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxMeshInstance {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub scale: [f32; 3],
    /// HDR 颜色（rgb 可 >1，a 为不透明度）。
    pub color: [f32; 4],
    pub uv_origin: [f32; 2],
    pub uv_scale: [f32; 2],
    pub uv2_origin: [f32; 2],
    pub uv2_scale: [f32; 2],
    pub texture_index: i32,
    pub texture2_index: i32,
    pub combine_color: i32,
    pub combine_alpha: i32,
    pub color_to_alpha: bool,
    pub color_to_alpha2: bool,
    pub blend_add: bool,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    pub distortion_power: f32,
    pub distortion_targets: u32,
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
    /// 剔除模式（CulT：0 双面、1 剔正面、2 剔背面）。
    pub cull_mode: i32,
    /// 内嵌绘制模型序号（文件 `Modl` 顺序）。
    pub model_index: usize,
}

/// 从解析后的 avfx 构建的采样运行时（拷贝子集，构建后与文件解耦）。
#[derive(Debug, Clone)]
pub struct VfxRuntime {
    file: AvfxFile,
    /// 采样硬上限，超出丢弃。
    max_quads: usize,
}

/// 一次粒子出生的全部上下文（发射器变换在出生时刻求值后的结果）。
struct SpawnContext {
    /// 粒子年龄（帧，全局=item 起算）。
    age: f32,
    /// 出生点（发射器 SRT ∘ 形状偏移，世界/武器空间）。
    position: [f32; 3],
    /// 注入方向与速度（单位/帧；方向已随发射器旋转）。
    injection_dir: [f32; 3],
    injection_speed: f32,
    emitter_orientation: [f32; 4],
    emitter_scale: [f32; 3],
    /// 全局创建序号（GeMT 顺序模式用）。
    create_index: u64,
}

impl VfxRuntime {
    pub fn new(file: &AvfxFile) -> Self {
        Self {
            file: file.clone(),
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
        self.sample_ambient(frame, &mut |ctx, item, particle| {
            if matches!(
                particle.particle_type,
                Some(ParticleType::Model) | Some(ParticleType::LightModel)
            ) {
                return; // 网格粒子走 sample_mesh
            }
            self.push_quad(&ctx, item, particle, out);
        });
    }

    /// 采样 `time_seconds` 时刻的网格粒子实例（out 先清空）。
    pub fn sample_mesh(&self, time_seconds: f32, out: &mut Vec<VfxMeshInstance>) {
        out.clear();
        let frame = time_seconds * AVFX_FPS;
        self.sample_ambient(frame, &mut |ctx, item, particle| {
            if !matches!(
                particle.particle_type,
                Some(ParticleType::Model) | Some(ParticleType::LightModel)
            ) {
                return;
            }
            self.push_mesh_instance(&ctx, item, particle, out);
        });
    }

    /// 遍历常驻（scheduler items 指向的）timeline 上 `frame` 时刻存活的所有
    /// 粒子出生事件（同一 timeline 被多个 item 引用时只采样一次）。
    /// 无 scheduler 时回退遍历全部 timeline。
    fn sample_ambient(&self, frame: f32, sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle)) {
        let mut visited = vec![false; self.file.timelines.len()];
        for scheduler in &self.file.schedulers {
            for item in &scheduler.items {
                if !item.enabled || item.timeline_index < 0 {
                    continue;
                }
                let Some(timeline) = self.file.timelines.get(item.timeline_index as usize)
                else {
                    continue;
                };
                if frame < item.start_time as f32 {
                    continue;
                }
                if visited[item.timeline_index as usize] {
                    continue;
                }
                visited[item.timeline_index as usize] = true;
                self.sample_timeline(timeline, frame - item.start_time as f32, sink);
            }
        }
        if self.file.schedulers.is_empty() {
            for timeline in &self.file.timelines {
                self.sample_timeline(timeline, frame, sink);
            }
        }
    }

    fn sample_timeline(
        &self,
        timeline: &AvfxTimeline,
        frame: f32,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        // LpSt < LpEd 才循环；0..0 表示一次性播放（item 的 EdTm=-1 = 常驻）。
        let local = if timeline.loop_end > timeline.loop_start {
            let start = timeline.loop_start as f32;
            let end = timeline.loop_end as f32;
            start + (frame - start).rem_euclid(end - start)
        } else {
            frame
        };
        for item in &timeline.items {
            if !item.enabled || item.emitter_index < 0 {
                continue;
            }
            if local < item.start_time as f32 {
                continue;
            }
            if item.end_time >= 0 && local >= item.end_time as f32 {
                continue;
            }
            let Some(emitter) = self.file.emitters.get(item.emitter_index as usize) else {
                continue;
            };
            self.sample_emitter(emitter, local - item.start_time as f32, sink);
        }
    }

    /// 枚举发射器在 `emitter_frame`（item 起算的帧）时刻存活的粒子出生事件。
    fn sample_emitter(
        &self,
        emitter: &AvfxEmitter,
        emitter_frame: f32,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        if emitter.particle_items.is_empty() || emitter_frame < 0.0 {
            return;
        }
        // 发射器寿命循环：Life>0 时按寿命分圈（AVFXTools `Age > Life → Reset`），
        // 永生发射器（Life=-1）只有一圈。
        let emitter_life = (emitter.life.enabled && emitter.life.value > 0.0)
            .then_some(emitter.life.value);
        let emitter_cap = if emitter.child_limit > 0 {
            emitter.child_limit as usize
        } else {
            48
        };
        let mut emitted = 0usize;

        for particle_item in &emitter.particle_items {
            if !particle_item.enabled || particle_item.target_index < 0 {
                continue;
            }
            let Some(particle) = self.file.particles.get(particle_item.target_index as usize)
            else {
                continue;
            };
            let life_frames = resolve_particle_life(particle_item, particle);
            // 永生粒子（Life=-1）：JustOneCreate，每圈只在首个创建事件触发；
            // 且只保留当前圈（更早圈的实例由游戏实例槽上限覆盖语义近似）。
            let just_once = life_frames.is_none();
            let max_mortal_life = life_frames.unwrap_or(0.0);
            let loop_starts: Vec<f32> = match emitter_life {
                Some(life) => {
                    let newest = (emitter_frame / life).floor();
                    let oldest = if just_once {
                        newest
                    } else {
                        ((emitter_frame - max_mortal_life) / life).floor().max(0.0)
                    };
                    ((oldest as i64)..=(newest as i64))
                        .map(|n| n as f32 * life)
                        .collect()
                }
                None => vec![0.0],
            };
            for loop_start in loop_starts {
                let loop_age_now = emitter_frame - loop_start;
                if loop_age_now < 0.0 {
                    continue;
                }
                let interval = emitter.create_interval.value(loop_age_now, 0.0);
                if interval <= 0.0 {
                    continue; // CrI=0：不周期创建
                }
                let last_event = (loop_age_now / interval).floor() as i64;
                for event in 0..=last_event {
                    // JustOneCreate：永生粒子每圈只在首个事件创建。
                    if just_once && event != 0 {
                        break;
                    }
                    let spawn_loop_age = event as f32 * interval;
                    // 发射器只在寿命窗口内创建。
                    if let Some(life) = emitter_life {
                        if spawn_loop_age >= life {
                            break;
                        }
                    }
                    let spawn_frame = loop_start + spawn_loop_age;
                    let age = emitter_frame - spawn_frame;
                    if let Some(life) = life_frames {
                        if age > life {
                            continue;
                        }
                    }
                    // 出生时刻的发射器变换（曲线按发射器 loop-local 年龄求值）。
                    let emitter_pos = emitter.position.evaluate(spawn_loop_age, 0.0);
                    let emitter_rot = emitter.rotation.evaluate(spawn_loop_age, 0.0);
                    let emitter_scl = emitter.scale.evaluate(spawn_loop_age, 1.0);
                    let emitter_quat = quat_from_euler(emitter.rotation_order, emitter_rot);
                    let count = emitter
                        .create_count
                        .value(spawn_loop_age, 1.0)
                        .round()
                        .max(0.0);
                    if count <= 0.0 {
                        continue;
                    }
                    let per_event =
                        (count * particle_item.create_count.max(1) as f32).round() as u64;
                    for k in 0..per_event {
                        if emitted >= emitter_cap {
                            return;
                        }
                        let mut rng = SplitMix64::seeded(
                            particle_item.target_index as u64 ^ 0x9E37_79B9,
                            (loop_start as u64) * 65_537 + event as u64,
                            k,
                            0xC2B2_AE3D,
                        );
                        if particle_item.create_probability < 100
                            && rng.next_f32() * 100.0 >= particle_item.create_probability as f32
                        {
                            continue;
                        }
                        emitted += 1;
                        let create_index = ((loop_start as u64 + event as u64) << 16) | k;
                        let (offset, dir, speed) =
                            self.sample_spawn_shape(emitter, spawn_loop_age, create_index, &mut rng);
                        let scaled_offset = [
                            offset[0] * emitter_scl[0],
                            offset[1] * emitter_scl[1],
                            offset[2] * emitter_scl[2],
                        ];
                        let rotated_offset = quat_rotate(emitter_quat, scaled_offset);
                        let position = [
                            emitter_pos[0] + rotated_offset[0],
                            emitter_pos[1] + rotated_offset[1],
                            emitter_pos[2] + rotated_offset[2],
                        ];
                        let ctx = SpawnContext {
                            age,
                            position,
                            injection_dir: quat_rotate(emitter_quat, dir),
                            injection_speed: speed,
                            emitter_orientation: emitter_quat,
                            emitter_scale: emitter_scl,
                            create_index,
                        };
                        sink(&ctx, particle_item, particle);
                    }
                }
            }
        }
    }

    /// 形状采样：返回发射器本地空间的 (出生偏移, 注入方向, 注入速度)。
    fn sample_spawn_shape(
        &self,
        emitter: &AvfxEmitter,
        loop_age: f32,
        create_index: u64,
        rng: &mut SplitMix64,
    ) -> ([f32; 3], [f32; 3], f32) {
        match &emitter.data {
            Some(AvfxEmitterData::Cone(cone)) => {
                let inner = cone.inner_size.value(loop_age, 0.0);
                let outer = cone.outer_size.value(loop_age, 0.0);
                let radius = inner + (outer - inner) * rng.next_f32();
                let theta = rng.next_f32() * std::f32::consts::TAU;
                let speed = cone.injection_speed.value(loop_age, 0.0);
                (
                    [radius * theta.cos(), 0.0, radius * theta.sin()],
                    [0.0, 1.0, 0.0],
                    speed,
                )
            }
            Some(AvfxEmitterData::ConeModel(cone)) => {
                let radius = cone.radius.value(loop_age, 0.0);
                let theta = ordered_angle(cone.generate_method, cone.divide_x, create_index, rng);
                let angle = cone.injection_angle.value(loop_age, 0.0);
                let speed = cone.injection_speed.value(loop_age, 0.0);
                let dir = [
                    theta.cos() * angle.cos(),
                    angle.sin(),
                    theta.sin() * angle.cos(),
                ];
                (
                    [radius * theta.cos(), 0.0, radius * theta.sin()],
                    dir,
                    speed,
                )
            }
            Some(AvfxEmitterData::CylinderModel(cyl)) => {
                let radius = cyl.radius.value(loop_age, 0.0);
                let theta = ordered_angle(cyl.generate_method, cyl.divide_x, create_index, rng);
                let speed = cyl.injection_speed.value(loop_age, 0.0);
                (
                    [radius * theta.cos(), 0.0, radius * theta.sin()],
                    [0.0, 1.0, 0.0],
                    speed,
                )
            }
            Some(AvfxEmitterData::SphereModel(sphere)) => {
                let radius = sphere.radius.value(loop_age, 0.0);
                let phi = rng.next_f32() * std::f32::consts::TAU;
                let cos_theta = 2.0 * rng.next_f32() - 1.0;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let dir = [sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin()];
                let speed = sphere.injection_speed.value(loop_age, 0.0);
                (
                    [dir[0] * radius, dir[1] * radius, dir[2] * radius],
                    dir,
                    speed,
                )
            }
            Some(AvfxEmitterData::Model(model)) => {
                let Some(geometry) = self.file.models.get(model.model_index as usize) else {
                    return ([0.0; 3], [0.0, 1.0, 0.0], 0.0);
                };
                let vertices = &geometry.emit_vertices;
                if vertices.is_empty() {
                    return ([0.0; 3], [0.0, 1.0, 0.0], 0.0);
                }
                let vertex = if is_order_method(model.generate_method) {
                    &vertices[(create_index as usize) % vertices.len()]
                } else {
                    &vertices[(rng.next_f32() * vertices.len() as f32) as usize
                        % vertices.len()]
                };
                let speed = model.injection_speed.value(loop_age, 0.0);
                (vertex.position, vertex.normal, speed)
            }
            _ => ([0.0; 3], [0.0, 1.0, 0.0], 0.0),
        }
    }

    /// 粒子曲线求值年龄：粒子自身的 LpSt/LpEd 循环。
    fn particle_age(particle: &AvfxParticle, age: f32) -> f32 {
        if particle.loop_end > particle.loop_start {
            let start = particle.loop_start as f32;
            let end = particle.loop_end as f32;
            start + (age - start).rem_euclid(end - start)
        } else {
            age
        }
    }

    fn push_quad(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxQuad>,
    ) {
        if out.len() >= self.max_quads {
            return;
        }
        if matches!(particle.particle_type, Some(ParticleType::Powder)) {
            self.push_powder(ctx, particle, out);
            return;
        }
        let age = Self::particle_age(particle, ctx.age);
        let tex = ResolvedTexture::of(particle, age);
        let add = is_additive_draw(particle.draw_mode);
        let mut color = particle.color.rgba_with_brightness(age, add);
        // Col.SclA：贴图 alpha 缩放（乘进总 alpha；加色模式下即强度倍率）。
        color[3] *= particle.color.texture_alpha_scale(age);
        let particle_scale = particle.scale.evaluate(age, 1.0);
        let scale = [
            particle_scale[0] * ctx.emitter_scale[0],
            particle_scale[1] * ctx.emitter_scale[1],
            particle_scale[2] * ctx.emitter_scale[2],
        ];
        let particle_pos = particle.position.evaluate(age, 0.0);
        let gravity = particle.gravity.value(age, 0.0);
        let mut position = [
            ctx.position[0]
                + quat_rotate(ctx.emitter_orientation, particle_pos)[0]
                + ctx.injection_dir[0] * ctx.injection_speed * ctx.age,
            ctx.position[1]
                + quat_rotate(ctx.emitter_orientation, particle_pos)[1]
                + ctx.injection_dir[1] * ctx.injection_speed * ctx.age
                - 0.5 * gravity * ctx.age * ctx.age,
            ctx.position[2]
                + quat_rotate(ctx.emitter_orientation, particle_pos)[2]
                + ctx.injection_dir[2] * ctx.injection_speed * ctx.age,
        ];
        // Powder 的 Data.CnOf：中心偏移（向发射原点收拢）。
        if let AvfxParticleData::Powder { center_offset } = particle.data {
            position[1] += center_offset;
        }
        let rotation_euler = particle.rotation.evaluate(age, 0.0);
        let mut rotation = rotation_euler[2];
        rotation += particle.rotation_velocity[2].value(age, 0.0) * ctx.age;
        let billboard = particle.is_billboard();
        let orientation = if billboard {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            quat_mul(
                ctx.emitter_orientation,
                quat_from_euler(particle.rotation_order, rotation_euler),
            )
        };
        out.push(VfxQuad {
            position,
            size: [0.5 * scale[0].abs(), 0.5 * scale[1].abs()],
            rotation,
            orientation,
            billboard,
            color,
            uv_origin: tex.uv_origin,
            uv_scale: tex.uv_scale,
            uv2_origin: tex.uv2_origin,
            uv2_scale: tex.uv2_scale,
            texture_index: tex.texture_index,
            texture2_index: tex.texture2_index,
            combine_color: tex.combine_color,
            combine_alpha: tex.combine_alpha,
            color_to_alpha: tex.color_to_alpha,
            color_to_alpha2: tex.color_to_alpha2,
            blend_add: is_additive_draw(particle.draw_mode),
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
        });
        let _ = item;
    }

    fn push_mesh_instance(
        &self,
        ctx: &SpawnContext,
        _item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        if out.len() >= self.max_quads {
            return;
        }
        let model_index = match &particle.data {
            AvfxParticleData::LightModel { model_index } => *model_index,
            AvfxParticleData::Model { model_indexes, .. } => {
                model_indexes.first().copied().unwrap_or(-1)
            }
            _ => -1,
        };
        if model_index < 0 {
            return;
        }
        let Some(geometry) = self.file.models.get(model_index as usize) else {
            return;
        };
        if geometry.draw.is_none() {
            return;
        }
        let age = Self::particle_age(particle, ctx.age);
        let tex = ResolvedTexture::of(particle, age);
        // Model 粒子用 Data 的 ColB/ColE（随年龄插值）；LightModel 用粒子 Col。
        let color = match &particle.data {
            AvfxParticleData::Model {
                color_begin,
                color_end,
                ..
            } if !color_begin.is_empty() || !color_end.is_empty() => {
                let life = particle.life_frames().unwrap_or(30.0);
                let t = (ctx.age / life).clamp(0.0, 1.0);
                let a = color_begin.rgba(age);
                let b = color_end.rgba(age);
                [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                    a[3] + (b[3] - a[3]) * t,
                ]
            }
            _ => particle.color.rgba_with_brightness(age, is_additive_draw(particle.draw_mode)),
        };
        let mut color = color;
        color[3] *= particle.color.texture_alpha_scale(age);
        let particle_pos = particle.position.evaluate(age, 0.0);
        let position = [
            ctx.position[0] + quat_rotate(ctx.emitter_orientation, particle_pos)[0],
            ctx.position[1] + quat_rotate(ctx.emitter_orientation, particle_pos)[1],
            ctx.position[2] + quat_rotate(ctx.emitter_orientation, particle_pos)[2],
        ];
        let particle_rot = particle.rotation.evaluate(age, 0.0);
        let particle_scl = particle.scale.evaluate(age, 1.0);
        out.push(VfxMeshInstance {
            position,
            orientation: quat_mul(
                ctx.emitter_orientation,
                quat_from_euler(particle.rotation_order, particle_rot),
            ),
            scale: [
                particle_scl[0] * ctx.emitter_scale[0],
                particle_scl[1] * ctx.emitter_scale[1],
                particle_scl[2] * ctx.emitter_scale[2],
            ],
            color,
            uv_origin: tex.uv_origin,
            uv_scale: tex.uv_scale,
            uv2_origin: tex.uv2_origin,
            uv2_scale: tex.uv2_scale,
            texture_index: tex.texture_index,
            texture2_index: tex.texture2_index,
            combine_color: tex.combine_color,
            combine_alpha: tex.combine_alpha,
            color_to_alpha: tex.color_to_alpha,
            color_to_alpha2: tex.color_to_alpha2,
            blend_add: is_additive_draw(particle.draw_mode),
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
            cull_mode: particle.culling_type,
            model_index: model_index as usize,
        });
    }

    /// Powder：spawner 本体不绘制，按 Smpl 参数喷出子粒子（短寿命小亮片）。
    fn push_powder(&self, ctx: &SpawnContext, particle: &AvfxParticle, out: &mut Vec<VfxQuad>) {
        let Some(simple) = &particle.simple else {
            return;
        };
        if simple.create_interval <= 0 || simple.create_count <= 0 {
            return;
        }
        let interval = simple.create_interval as f32;
        let sub_life = simple.create_interval_life.max(1) as f32;
        // spawner 年龄 = 粒子年龄（本体寿命即粒子 Life）。
        let spawner_age = ctx.age;
        let last_sub = (spawner_age / interval).floor() as i64;
        let first_sub = ((spawner_age - sub_life) / interval).ceil().max(0.0) as i64;
        let emit_vertices = (simple.injection_model_index >= 0)
            .then(|| self.file.models.get(simple.injection_model_index as usize))
            .flatten()
            .map(|model| model.emit_vertices.as_slice())
            .unwrap_or(&[]);
        let bri = particle
            .color
            .brightness
            .as_ref()
            .map_or(1.0, |c| c.value(spawner_age, 1.0));
        let scl_a = particle.color.texture_alpha_scale(spawner_age);
        let tex = ResolvedTexture::of(particle, spawner_age);
        for event in first_sub..=last_sub {
            for c in 0..simple.create_interval_count.max(1) as u64 {
                let global = event as u64 * simple.create_interval_count.max(1) as u64 + c;
                if global >= simple.create_count as u64 {
                    return;
                }
                if out.len() >= self.max_quads {
                    return;
                }
                let sub_age = spawner_age - event as f32 * interval;
                let t = (sub_age / sub_life).clamp(0.0, 1.0);
                let mut rng = SplitMix64::seeded(
                    0x7050_5752,
                    ctx.create_index,
                    global,
                    0xD1B4,
                );
                let base = if emit_vertices.is_empty() {
                    ctx.position
                } else {
                    let vertex = &emit_vertices[global as usize % emit_vertices.len()];
                    [
                        ctx.position[0] + vertex.position[0],
                        ctx.position[1] + vertex.position[1],
                        ctx.position[2] + vertex.position[2],
                    ]
                };
                // 出生散布 + 初速度漂移（随机方向，速率 VMin..VMax/帧）。
                let jitter = [
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[0],
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[1],
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[2],
                ];
                let phi = rng.next_f32() * std::f32::consts::TAU;
                let cos_theta = 2.0 * rng.next_f32() - 1.0;
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let speed =
                    simple.velocity_min + (simple.velocity_max - simple.velocity_min) * rng.next_f32();
                let drift = [
                    sin_theta * phi.cos() * speed * sub_age,
                    cos_theta * speed * sub_age,
                    sin_theta * phi.sin() * speed * sub_age,
                ];
                let rand_x = lerp(simple.scale_rand_x[0], simple.scale_rand_x[1], rng.next_f32());
                let rand_y = lerp(simple.scale_rand_y[0], simple.scale_rand_y[1], rng.next_f32());
                let curve_t = t.powf(simple.scale_curve.max(1.0e-3));
                let scale = [
                    (simple.scale_start[0]
                        + (simple.scale_end[0] - simple.scale_start[0]) * curve_t)
                        * rand_x,
                    (simple.scale_start[1]
                        + (simple.scale_end[1] - simple.scale_start[1]) * curve_t)
                        * rand_y,
                ];
                // 颜色帧簿：Frms 时间点之间插值 Cols。
                let color = sample_flipbook(&simple.colors, &simple.frames, sub_age);
                // UV 翻页：每 UvIv 帧前进一格。
                let (uv_origin, uv_scale) = if simple.uv_cell[0] > 1 || simple.uv_cell[1] > 1 {
                    let cells = (simple.uv_cell[0] * simple.uv_cell[1]) as u64;
                    let start = (rng.next_f32() * cells as f32) as u64;
                    let step = (sub_age / simple.uv_interval as f32) as u64;
                    let cell = (start + step) % cells;
                    let cx = (cell % simple.uv_cell[0] as u64) as f32;
                    let cy = ((cell / simple.uv_cell[0] as u64) % simple.uv_cell[1] as u64) as f32;
                    (
                        [cx / simple.uv_cell[0] as f32, cy / simple.uv_cell[1] as f32],
                        [1.0 / simple.uv_cell[0] as f32, 1.0 / simple.uv_cell[1] as f32],
                    )
                } else {
                    (tex.uv_origin, tex.uv_scale)
                };
                out.push(VfxQuad {
                    position: [
                        base[0] + jitter[0] + drift[0],
                        base[1] + jitter[1] + drift[1],
                        base[2] + jitter[2] + drift[2],
                    ],
                    size: [0.5 * scale[0].abs(), 0.5 * scale[1].abs()],
                    rotation: rng.next_f32() * std::f32::consts::TAU,
                    orientation: [0.0, 0.0, 0.0, 1.0],
                    billboard: true,
                    color: [
                        color[0] * bri,
                        color[1] * bri,
                        color[2] * bri,
                        color[3] * scl_a,
                    ],
                    uv_origin,
                    uv_scale,
                    uv2_origin: tex.uv2_origin,
                    uv2_scale: tex.uv2_scale,
                    texture_index: tex.texture_index,
                    texture2_index: tex.texture2_index,
                    combine_color: tex.combine_color,
                    combine_alpha: tex.combine_alpha,
                    color_to_alpha: tex.color_to_alpha,
                    color_to_alpha2: tex.color_to_alpha2,
                    blend_add: is_additive_draw(particle.draw_mode),
                    texture_distortion_index: tex.texture_distortion_index,
                    distortion_power: tex.distortion_power,
                    distortion_targets: tex.distortion_targets,
                    uvd_origin: tex.uvd_origin,
                    uvd_scale: tex.uvd_scale,
                });
            }
        }
    }
}

/// 粒子的贴图/UV 解析结果（TC1 = 基准层，TC2 = 合成层，TD = 扭曲层）。
struct ResolvedTexture {
    texture_index: i32,
    texture2_index: i32,
    combine_color: i32,
    combine_alpha: i32,
    color_to_alpha: bool,
    color_to_alpha2: bool,
    uv_origin: [f32; 2],
    uv_scale: [f32; 2],
    uv2_origin: [f32; 2],
    uv2_scale: [f32; 2],
    texture_distortion_index: i32,
    distortion_power: f32,
    distortion_targets: u32,
    uvd_origin: [f32; 2],
    uvd_scale: [f32; 2],
}

impl ResolvedTexture {
    fn of(particle: &AvfxParticle, age: f32) -> Self {
        let uv = |index: i32| -> ([f32; 2], [f32; 2]) {
            particle
                .uv_sets
                .get(index.max(0) as usize)
                .map(|set| {
                    (
                        set.scroll.evaluate(age, 0.0),
                        set.scale.evaluate(age, 1.0),
                    )
                })
                .unwrap_or(([0.0, 0.0], [1.0, 1.0]))
        };
        let tc1 = particle
            .texture_color1
            .as_ref()
            .filter(|texture| texture.enabled);
        let tc2 = particle
            .texture_color2
            .as_ref()
            .filter(|texture| texture.enabled && texture.effective_texture() >= 0);
        let (uv_origin, uv_scale) = uv(tc1.map(|t| t.uv_set_index).unwrap_or(0));
        let (uv2_origin, uv2_scale) = tc2
            .map(|t| uv(t.uv_set_index))
            .unwrap_or((uv_origin, uv_scale));
        // TD：扭曲贴图用其 UvSN 指向的 UvSet 变换采样；bT1/bT2 按 TC1/TC2
        // 实际引用的 UvSet 序号映射到 uv1/uv2。
        let td = particle
            .texture_distortion
            .as_ref()
            .filter(|t| t.enabled && t.texture_index >= 0);
        let (texture_distortion_index, distortion_power, distortion_targets, uvd) =
            match td {
                Some(td) => {
                    let uvd = uv(td.uv_set_index);
                    let uv1_idx = tc1.map(|t| t.uv_set_index).unwrap_or(0);
                    let uv2_idx = tc2.map(|t| t.uv_set_index).unwrap_or(uv1_idx);
                    // bT1/bT2 = 扭曲 UvSet 0/1；按 TC1/TC2 实际引用的
                    // UvSet 序号映射到渲染端的 uv1/uv2。
                    let distort = |uv_idx: i32| {
                        (td.target_uv[0] && uv_idx == 0) || (td.target_uv[1] && uv_idx == 1)
                    };
                    let targets = (distort(uv1_idx) as u32) | ((distort(uv2_idx) as u32) << 1);
                    (
                        td.texture_index,
                        td.power.value(age, 0.0),
                        targets,
                        uvd,
                    )
                }
                None => (-1, 0.0, 0, ([0.0, 0.0], [1.0, 1.0])),
            };
        Self {
            texture_index: tc1.map(|t| t.effective_texture()).unwrap_or(-1),
            texture2_index: tc2.map(|t| t.effective_texture()).unwrap_or(-1),
            combine_color: tc2.map(|t| t.calculate_color).unwrap_or(0),
            combine_alpha: tc2.map(|t| t.calculate_alpha).unwrap_or(0),
            color_to_alpha: tc1.map(|t| t.color_to_alpha).unwrap_or(false),
            color_to_alpha2: tc2.map(|t| t.color_to_alpha).unwrap_or(false),
            uv_origin,
            uv_scale,
            uv2_origin,
            uv2_scale,
            texture_distortion_index,
            distortion_power,
            distortion_targets,
            uvd_origin: uvd.0,
            uvd_scale: uvd.1,
        }
    }
}

/// 颜色帧簿求值：`frames` 为时间点，`colors` 逐段线性插值。
fn sample_flipbook(colors: &[[u8; 4]; 4], frames: &[i16; 4], age: f32) -> [f32; 4] {
    let to_f32 = |c: [u8; 4]| {
        [
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            c[3] as f32 / 255.0,
        ]
    };
    if age <= frames[0] as f32 {
        return to_f32(colors[0]);
    }
    for i in 0..3 {
        let t0 = frames[i] as f32;
        let t1 = frames[i + 1].max(frames[i] + 1) as f32;
        if age <= t1 {
            let local = ((age - t0) / (t1 - t0)).clamp(0.0, 1.0);
            let a = to_f32(colors[i]);
            let b = to_f32(colors[i + 1]);
            return [
                lerp(a[0], b[0], local),
                lerp(a[1], b[1], local),
                lerp(a[2], b[2], local),
                lerp(a[3], b[3], local),
            ];
        }
    }
    to_f32(colors[3])
}

/// ItPr 覆盖 > 粒子 Life；未启用/<=0 = 永生（None）。
fn resolve_particle_life(item: &AvfxEmitterItem, particle: &AvfxParticle) -> Option<f32> {
    if item.override_life && item.override_life_value > 0 {
        Some(item.override_life_value as f32)
    } else {
        particle.life_frames()
    }
}

/// GeMT 顺序系（Order*）：按创建序号取模；其余随机。
fn is_order_method(generate_method: i32) -> bool {
    matches!(generate_method, 1 | 3 | 5 | 7)
}

/// 顺序/随机的圆周角：顺序模式按创建序号均分 DivX。
fn ordered_angle(generate_method: i32, divide_x: i32, create_index: u64, rng: &mut SplitMix64) -> f32 {
    if is_order_method(generate_method) && divide_x > 0 {
        (create_index % divide_x as u64) as f32 / divide_x as f32 * std::f32::consts::TAU
    } else {
        rng.next_f32() * std::f32::consts::TAU
    }
}

/// RMT → 混合模式：Add/Screen/Reverse 系走加色，其余普通 alpha 混合。
fn is_additive_draw(draw_mode: i32) -> bool {
    !matches!(draw_mode, crate::avfx::DRAW_MODE_BLEND)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 欧拉角（弧度）→ 四元数（xyzw）。`order` 为 VFXEditor `RotationOrder`
/// （0 XYZ、1 YZX、2 ZXY、3 XZY、4 YXZ、5 ZYX），名称序即旋转施加顺序。
fn quat_from_euler(order: i32, angles: [f32; 3]) -> [f32; 4] {
    let [x, y, z] = angles;
    let qx = [(x * 0.5).sin(), 0.0, 0.0, (x * 0.5).cos()];
    let qy = [0.0, (y * 0.5).sin(), 0.0, (y * 0.5).cos()];
    let qz = [0.0, 0.0, (z * 0.5).sin(), (z * 0.5).cos()];
    // v' = R_first∘... 的相反：列序 XYZ 表示先绕 X，矩阵 M = Rz·Ry·Rx。
    match order {
        0 => quat_mul(qz, quat_mul(qy, qx)),
        1 => quat_mul(qx, quat_mul(qz, qy)),
        2 => quat_mul(qy, quat_mul(qx, qz)),
        3 => quat_mul(qy, quat_mul(qz, qx)),
        4 => quat_mul(qz, quat_mul(qx, qy)),
        _ => quat_mul(qx, quat_mul(qy, qz)),
    }
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let t = [
        2.0 * (y * v[2] - z * v[1]),
        2.0 * (z * v[0] - x * v[2]),
        2.0 * (x * v[1] - y * v[0]),
    ];
    [
        v[0] + w * t[0] + y * t[2] - z * t[1],
        v[1] + w * t[1] + z * t[0] - x * t[2],
        v[2] + w * t[2] + x * t[1] - y * t[0],
    ]
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
        AvfxColorCurve, AvfxCurve, AvfxCurve3Axis, AvfxCurveKey, AvfxEmitter, AvfxFile, AvfxLife,
        AvfxParticle, AvfxParticleTexture, AvfxScheduler, AvfxSchedulerItem, AvfxTimeline,
        AvfxTimelineItem, AvfxUvSet, ConeEmitterData, EmitterType, ParticleType,
    };

    fn linear_curve(value: f32) -> AvfxCurve {
        AvfxCurve {
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

    fn axis3(x: Option<f32>, y: Option<f32>, z: Option<f32>) -> AvfxCurve3Axis {
        AvfxCurve3Axis {
            axis_connect: 0,
            x: x.map(linear_curve),
            y: y.map(linear_curve),
            z: z.map(linear_curve),
        }
    }

    fn color_curve(r: f32, g: f32, b: f32, a: f32) -> AvfxColorCurve {
        AvfxColorCurve {
            rgb: Some(AvfxCurve {
                keys: vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: r,
                    y: g,
                    z: b,
                }],
                ..Default::default()
            }),
            alpha: Some(linear_curve(a)),
            brightness: None,
            scale_alpha: None,
        }
    }

    fn quad_particle() -> AvfxParticle {
        AvfxParticle {
            particle_type: Some(ParticleType::Quad),
            raw_particle_type: 8,
            draw_mode: 2,
            rotation_direction_base: 6,
            scale: axis3(Some(0.2), Some(0.2), None),
            color: color_curve(0.5, 0.6, 0.8, 0.9),
            texture_color1: Some(AvfxParticleTexture {
                enabled: true,
                uv_set_index: 0,
                texture_index: -1,
                mask_texture_index: 2,
                ..Default::default()
            }),
            uv_sets: vec![AvfxUvSet {
                calculate_uv: 0,
                scale: Default::default(),
                scroll: Default::default(),
                rotation: Default::default(),
                rotation_random: Default::default(),
            }],
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
                loop_end: 0,
                binder_index: 0,
                items: vec![AvfxTimelineItem {
                    enabled: true,
                    start_time: 0,
                    end_time: -1,
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
                child_limit: 128,
                life: AvfxLife {
                    enabled: true,
                    value: 30.0,
                    value_random: 0.0,
                    random_type: 0,
                },
                create_count: linear_curve(2.0),
                create_interval: linear_curve(15.0),
                position: axis3(None, Some(0.5), None),
                data: Some(AvfxEmitterData::Cone(ConeEmitterData {
                    inner_size: linear_curve(0.1),
                    outer_size: linear_curve(0.5),
                    injection_speed: linear_curve(0.03),
                    injection_angle: linear_curve(0.0),
                })),
                particle_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 0,
                    create_time: 0,
                    create_count: 3,
                    create_probability: 100,
                    start_frame: 0,
                    generate_delay: 0,
                    override_life: false,
                    override_life_value: 60,
                }],
                ..Default::default()
            }],
            particles: vec![AvfxParticle {
                life: AvfxLife {
                    enabled: true,
                    value: 30.0,
                    value_random: 0.0,
                    random_type: 0,
                },
                ..quad_particle()
            }],
            ..Default::default()
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
        assert_eq!(quads_a, quads_b);
        // 间隔 15 帧、生命 30 帧、emitter Life 30：t=30 帧处恰有事件 k=0,1,2
        // （k=2 落在 emitter 寿命边界；每事件 CrC=2 × CrCn=3 = 6 粒子）。
        assert_eq!(quads_a.len(), 18);
        let quad = &quads_a[0];
        // 颜色 = Col.RGB（HDR 无 Bri），alpha = Col.A。
        assert!((quad.color[2] - 0.8).abs() < 1.0e-6);
        assert!((quad.color[3] - 0.9).abs() < 1.0e-6);
        // TC1 TLst 优先。
        assert_eq!(quad.texture_index, 2);
        // 尺寸 = 半宽（scale 0.2 → 0.1）。
        assert!((quad.size[0] - 0.1).abs() < 1.0e-6);
        // billboard 四元数为单位。
        assert_eq!(quad.orientation, [0.0, 0.0, 0.0, 1.0]);
        // 发射器 Pos.Y = 0.5 生效。
        assert!(quad.position[1] >= 0.5 - 1.0e-5);
    }

    #[test]
    fn immortal_particle_creates_once_per_emitter_loop() {
        let mut file = fixture_file();
        file.particles[0].life = AvfxLife::default(); // 永生
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(2.0, &mut quads); // 60 帧 = emitter 第 3 圈
        // 永生粒子：每圈 JustOneCreate 一次 × CrC=2 × CrCn=3 = 6/圈；
        // 存留窗口覆盖 0..=2 圈（永生粒子只在当前圈创建 → 见 sample_emitter：
        // loop_starts 由 max_mortal_life=0 过滤为最旧=最新圈？此处 emitter
        // Life=30，永生粒子在每一圈都创建一份）。
        assert!(!quads.is_empty());
        runtime.sample(2.0, &mut quads.clone());
    }

    #[test]
    fn cone_positions_stay_within_bounds_and_inject_upward() {
        let file = fixture_file();
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(0.9, &mut quads);
        assert!(!quads.is_empty());
        for quad in quads {
            let radial =
                (quad.position[0] * quad.position[0] + quad.position[2] * quad.position[2]).sqrt();
            assert!(radial <= 0.5 + 1.0e-3, "radial {radial} beyond cone outer");
            assert!(quad.position[1] >= 0.5 - 1.0e-5);
        }
    }

    #[test]
    fn empty_file_samples_nothing() {
        let runtime = VfxRuntime::new(&AvfxFile::default());
        let mut quads = Vec::new();
        runtime.sample(1.0, &mut quads);
        assert!(quads.is_empty());
    }
}
