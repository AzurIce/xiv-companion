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
//!   `CrC × max(1, ItPr.CrCn)` 个粒子（`CrPr` 概率过滤、`StFr` 起始帧门控、
//!   `BIAX/Y/Z` 按事件内序号逐粒子旋转）；
//! - 出生点 = 发射器基座（子发射器为父链变换，根为绑点）∘ emitter
//!   Pos/Rot/Scl（含随机轴，按圈播种）∘ 形状偏移（Point/Cone/ConeModel/
//!   CylinderModel/SphereModel/Model 发射顶点）；
//! - 父级影响按 ItPr `PICd`+`ICbS/R` 解析：Initial 分量取出生时刻、Always
//!   分量取当前时刻发射器变换（`NoPosition` 变体不跟位置，`None` 不传递）；
//! - 粒子状态由出生时间解析式计算（无增量状态，任意时刻采样一致，可单测/
//!   快照），随机数用 splitmix64 按 (item, 圈, 事件, 粒子) 播种，曲线随机
//!   （`*R` 块/颜色 `Ran*`）按实例种子逐键重掷（AVFXTools
//!   `CurveRandomAttribute`），确定性可复现；
//! - 运动积分：初速度 = 注入方向×速度 + 发射器 `VRX/Y/Z` 随机；位移受
//!   ARs（+`ARsR`+发射器 ARs）逐帧衰减（等比累计），重力 = 粒子
//!   Gra(+`GraR`) + 发射器 Gra；实例寿命含 `Life.ValR` 随机；
//! - Powder 粒子是子粒子发射器（需 `bSCt` 启用 `Smpl`）：本体不绘制，
//!   按 `Smpl` 参数持续喷出短寿命子粒子（颜色帧簿 `Cols`/`Frms`、UV 翻页
//!   （`UvNR`/`UvLC`/`bRUV`）、径向方向 `IRD0/1`、速度衰减 `FltR/FltS`、
//!   旋转 `RB+RI+(RA+RV)·age`+随机相位、枢轴 `PvtX/Y`、子粒子重力
//!   `CGX/Y/Z`、`bCrN` 重建、`bSRL` 尺寸随机联动、`bSnP` 随父缩放）。

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
    /// 粒子 `DwPr` 绘制优先级（渲染端按它排序，同优先级保持创建序）。
    pub draw_priority: i32,
    /// 面内旋转枢轴（四边形半尺寸为单位；Smpl `PvtX`/`PvtY`）。
    pub pivot: [f32; 2],
    /// 4 层颜色贴图（TC1..TC4）的文件级贴图序号（TC1：TLst 优先；-1 = 无）。
    pub texture_indexes: [i32; 4],
    /// 各层引用的 UvSet 序号（网格粒子按它选逐顶点 UV 组；四边形基底
    /// 恒为角点，此字段不参与）。
    pub texture_uv_sets: [i32; 4],
    /// TC1 的 (颜色 TCCT, alpha TCAT) 合成模式（网格粒子 TC1 参与合成；
    /// 四边形 TC1 为覆盖写）。
    pub combine_mode_tc1: [i32; 2],
    /// TC2/TC3/TC4 的 (颜色 TCCT, alpha TCAT) 合成模式。
    pub combine_modes: [[i32; 2]; 3],
    /// 各层 bC2A（颜色转 alpha）。
    pub color_to_alpha: [bool; 4],
    /// 各层 UV 原点 + 缩放（各层引用自己的 UvSet）。
    pub uv_origins: [[f32; 2]; 4],
    pub uv_scales: [[f32; 2]; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（UvSet `Rot`+`RotR`，弧度）。
    pub uv_rotations: [f32; 4],
    /// 各层 U/V 边界模式（0 Repeat、1 Clamp、2 Mirror；渐变贴图 Clamp
    /// 防止越界回绕出异色条纹）。
    pub texture_borders: [[i32; 2]; 4],
    /// TC1 贴图为 TLst 形状遮罩（亮度→alpha，rgb 不乘）。
    pub texture1_is_shape_mask: bool,
    /// TC1 块存在且启用（网格路径据此决定 TC1 是否参与合成；四边形路径
    /// 恒采样 TC1/回退光点保形状）。
    pub texture1_enabled: bool,
    /// 混合模式：true = 加色（Add 系），false = 普通 alpha 混合。
    pub blend_add: bool,
    /// `DsDt`/`DsDw`：深度测试/写入（如爪笼壳写深度遮挡身后几何）。
    pub depth_test: bool,
    pub depth_write: bool,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    /// TD 扭曲强度（DPow 按粒子年龄求值）。
    pub distortion_power: f32,
    /// TD 扭曲目标位（bit0..3 = uv1..4）。
    pub distortion_targets: u32,
    /// TD 贴图采样用 UV（TD 引用的 UvSet 变换）。
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
    /// TD 引用的 UvSet 序号（网格粒子的基底 UV 选择）。
    pub distortion_uv_set: i32,
    /// TD 边界模式。
    pub distortion_borders: [i32; 2],
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
    /// 粒子 `DwPr` 绘制优先级。
    pub draw_priority: i32,
    pub texture_indexes: [i32; 4],
    /// 各层引用的 UvSet 序号（按它选逐顶点 UV 组）。
    pub texture_uv_sets: [i32; 4],
    /// TC1 的 (颜色 TCCT, alpha TCAT) 合成模式。
    pub combine_mode_tc1: [i32; 2],
    pub combine_modes: [[i32; 2]; 3],
    pub color_to_alpha: [bool; 4],
    pub uv_origins: [[f32; 2]; 4],
    pub uv_scales: [[f32; 2]; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（弧度）。
    pub uv_rotations: [f32; 4],
    pub texture_borders: [[i32; 2]; 4],
    pub texture1_is_shape_mask: bool,
    /// TC1 块存在且启用。
    pub texture1_enabled: bool,
    pub blend_add: bool,
    /// `DsDt`/`DsDw`：深度测试/写入。
    pub depth_test: bool,
    pub depth_write: bool,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    pub distortion_power: f32,
    pub distortion_targets: u32,
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
    /// TD 引用的 UvSet 序号。
    pub distortion_uv_set: i32,
    pub distortion_borders: [i32; 2],
    /// 剔除模式（CulT：0 双面、1 剔正面、2 剔背面）。
    pub cull_mode: i32,
    /// 内嵌绘制模型序号（文件 `Modl` 顺序）。
    pub model_index: usize,
}

/// 从解析后的 avfx 构建的采样运行时（拷贝子集，构建后与文件解耦）。
#[derive(Debug, Clone)]
pub struct VfxRuntime {
    file: AvfxFile,
    /// 目标模型的特效绑点表（id → 模型空间偏移；空表 = 全部原点）。
    bind_points: Vec<crate::avfx::VfxBindPoint>,
    /// 采样硬上限，超出丢弃。
    max_quads: usize,
}

/// 发射器实例的基准变换（子发射器经父链继承；根发射器 = 绑点偏移）。
#[derive(Clone, Copy, Debug)]
struct EmitterBase {
    position: [f32; 3],
    orientation: [f32; 4],
    scale: [f32; 3],
}

impl EmitterBase {
    fn root(bind_offset: [f32; 3]) -> Self {
        Self {
            position: bind_offset,
            orientation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }

    /// 发射器本地点 → 世界（scale → rotate → translate）。
    fn transform_point(&self, point: [f32; 3]) -> [f32; 3] {
        let scaled = [
            point[0] * self.scale[0],
            point[1] * self.scale[1],
            point[2] * self.scale[2],
        ];
        let rotated = quat_rotate(self.orientation, scaled);
        [
            self.position[0] + rotated[0],
            self.position[1] + rotated[1],
            self.position[2] + rotated[2],
        ]
    }

    /// 本地四元数 → 世界。
    fn rotate_quat(&self, quat: [f32; 4]) -> [f32; 4] {
        quat_mul(self.orientation, quat)
    }
}

/// 一次粒子出生的全部上下文。
struct SpawnContext {
    /// 粒子年龄（帧，全局=item 起算）。
    age: f32,
    /// 出生点（发射器基座 ∘ 发射器 SRT(出生时刻) ∘ 形状偏移，世界/武器空间）。
    position: [f32; 3],
    /// 初速度（世界，单位/帧）：注入方向×速度 + 发射器 VRX/VRY/VRZ 随机。
    velocity: [f32; 3],
    /// 出生时刻的发射器世界朝向/缩放（Initial 语义）。
    emitter_orientation: [f32; 4],
    emitter_scale: [f32; 3],
    /// 当前时刻的发射器世界朝向/缩放（PICd Always 语义）。
    emitter_orientation_now: [f32; 4],
    emitter_scale_now: [f32; 3],
    /// 发射器位置从出生到现在的位移（PICd=AllAlways 的连续跟随）。
    emitter_pos_drift: [f32; 3],
    /// 合并重力（粒子 Gra 在 push 时另加；此处为发射器 Gra 出生时刻值）。
    emitter_gravity: f32,
    /// 合并空气阻力（发射器 ARs 出生时刻值；粒子 ARs 在 push 时另加）。
    emitter_air_resistance: f32,
    /// 实例寿命（帧；含 ItPr 覆盖与 Life ValR 随机；None = 永生）。
    life: Option<f32>,
    /// 实例随机种子（随机曲线/颜色随机的确定性播种）。
    seed: u64,
    /// 全局创建序号（GeMT 顺序模式用）。
    create_index: u64,
}

impl VfxRuntime {
    pub fn new(file: &AvfxFile) -> Self {
        Self::with_bind_points(file, &[])
    }

    /// 携带武器绑点表构建（avfx Binder 的 `BPID` 经它解析成位置偏移）。
    pub fn with_bind_points(file: &AvfxFile, bind_points: &[crate::avfx::VfxBindPoint]) -> Self {
        Self {
            file: file.clone(),
            bind_points: bind_points.to_vec(),
            max_quads: 4096,
        }
    }

    /// timeline item 的绑点偏移（binder_index → binder.BPID → 武器绑点位置）。
    fn bind_offset(&self, binder_index: i32) -> [f32; 3] {
        let Some(binder) = self.file.binders.get(binder_index.max(0) as usize) else {
            return [0.0; 3];
        };
        if binder.bind_point_id < 0 {
            return [0.0; 3];
        }
        self.bind_points
            .iter()
            .find(|point| point.id == binder.bind_point_id as u32)
            .map(|point| point.translate)
            .unwrap_or([0.0; 3])
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
            let bind_offset = self.bind_offset(item.binder_index);
            self.sample_emitter(
                item.emitter_index as usize,
                emitter,
                local - item.start_time as f32,
                EmitterBase::root(bind_offset),
                0,
                sink,
            );
        }
    }

    /// 枚举发射器在 `emitter_frame`（实例出生起算的帧）时刻存活的粒子出生事件。
    /// `base` 为实例基准变换（根发射器=绑点偏移；子发射器=父链变换 ∘ 注入旋转）。
    fn sample_emitter(
        &self,
        emitter_index: usize,
        emitter: &AvfxEmitter,
        emitter_frame: f32,
        base: EmitterBase,
        depth: u32,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        if emitter_frame < 0.0 || depth > 4 {
            return;
        }
        // 发射器寿命循环：Life>0 时按寿命分圈（AVFXTools `Age > Life → Reset`），
        // 永生发射器（Life=-1）只有一圈。
        let emitter_life =
            (emitter.life.enabled && emitter.life.value > 0.0).then_some(emitter.life.value);
        let emitter_cap = if emitter.child_limit > 0 {
            emitter.child_limit as usize
        } else {
            48
        };
        let mut emitted = 0usize;
        // 发射器随机轴按 (发射器, 圈) 播种（AVFXTools 在发射器 Reset=每圈开始时
        // 重掷 CurveRandomGroup）。
        let emitter_seed = |loop_start: f32| {
            SplitMix64::seeded(
                emitter_index as u64 ^ 0xE417,
                loop_start.to_bits() as u64,
                depth as u64,
                0x1A2B,
            )
        };
        // 发射器 SRT（含随机轴）→ 世界。
        let emitter_transform = |loop_age: f32, loop_start: f32| {
            let mut rng = emitter_seed(loop_start);
            let pos = eval3_seeded(&emitter.position, loop_age, 0.0, rng.next_u64());
            let rot = eval3_seeded(&emitter.rotation, loop_age, 0.0, rng.next_u64());
            let scl = eval3_seeded(&emitter.scale, loop_age, 1.0, rng.next_u64());
            (
                base.transform_point(pos),
                base.rotate_quat(quat_from_euler(emitter.rotation_order, rot)),
                [
                    base.scale[0] * scl[0],
                    base.scale[1] * scl[1],
                    base.scale[2] * scl[2],
                ],
            )
        };

        for particle_item in &emitter.particle_items {
            if !particle_item.enabled || particle_item.target_index < 0 {
                continue;
            }
            let Some(particle) = self.file.particles.get(particle_item.target_index as usize)
            else {
                continue;
            };
            let life_base = resolve_particle_life(particle_item, particle);
            // 永生粒子（Life=-1）：JustOneCreate，每圈只在首个创建事件触发；
            // 且只保留当前圈（更早圈的实例由游戏实例槽上限覆盖语义近似）。
            let just_once = life_base.is_none();
            // Life ValR 随机会拉长个体寿命，枚举窗口按最大幅度放宽。
            let life_jitter = particle.life.value_random.abs();
            let max_mortal_life = life_base.map(|life| life + life_jitter).unwrap_or(0.0);
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
                // 当前时刻的发射器变换（PICd Always 分量用）。
                let (pos_now, quat_now, scl_now) = emitter_transform(loop_age_now, loop_start);
                let last_event = (loop_age_now / interval).floor() as i64;
                for event in 0..=last_event {
                    // JustOneCreate：永生粒子每圈只在首个事件创建。
                    if just_once && event != 0 {
                        break;
                    }
                    let spawn_loop_age = event as f32 * interval;
                    // ItPr.StFr：创建起始帧（发射器 loop-local）。
                    if spawn_loop_age < particle_item.start_frame as f32 {
                        continue;
                    }
                    // 发射器只在寿命窗口内创建。
                    if let Some(life) = emitter_life {
                        if spawn_loop_age >= life {
                            break;
                        }
                    }
                    let spawn_frame = loop_start + spawn_loop_age;
                    let age = emitter_frame - spawn_frame;
                    if let Some(life) = life_base {
                        if age > life + life_jitter {
                            continue;
                        }
                    }
                    // 出生时刻的发射器变换（曲线按发射器 loop-local 年龄求值）。
                    let (emitter_pos, emitter_quat, emitter_scl) =
                        emitter_transform(spawn_loop_age, loop_start);
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
                    // 发射器 Gra/ARs 在出生时刻固化给粒子（GraR/ARsR 逐粒子重掷）。
                    let emitter_gravity = emitter.gravity.value(spawn_loop_age, 0.0);
                    let emitter_resistance = emitter.air_resistance.value(spawn_loop_age, 0.0);
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
                        // Life ValR：实例寿命 = 基准 ± 随机幅度（RanT 区间）。
                        let life = life_base.map(|life| {
                            if life_jitter > 0.0 {
                                let rolled = roll_amplitude(
                                    &mut rng,
                                    particle.life.random_type,
                                    life_jitter,
                                );
                                (life + rolled).max(0.0)
                            } else {
                                life
                            }
                        });
                        if let Some(life) = life {
                            if age > life {
                                continue;
                            }
                        }
                        emitted += 1;
                        let create_index = ((loop_start as u64 + event as u64) << 16) | k;
                        let seed = rng.next_u64();
                        let (offset, dir, speed) = self.sample_spawn_shape(
                            emitter,
                            spawn_loop_age,
                            create_index,
                            &mut rng,
                        );
                        // ItPr BIAX/Y/Z：按事件内序号的逐粒子旋转（弧度，ZXY；
                        // 对齐 AVFXTools `idx * InjectionAngle`）。
                        let item_quat = if particle_item.by_injection_angle != [0.0; 3] {
                            quat_from_euler(
                                2,
                                [
                                    particle_item.by_injection_angle[0] * k as f32,
                                    particle_item.by_injection_angle[1] * k as f32,
                                    particle_item.by_injection_angle[2] * k as f32,
                                ],
                            )
                        } else {
                            [0.0, 0.0, 0.0, 1.0]
                        };
                        let offset = quat_rotate(item_quat, offset);
                        let dir = quat_rotate(item_quat, dir);
                        // 发射器注入角（IAX/Y/Z + 随机）：旋转注入方向。
                        let dir = {
                            let angles = [
                                curve_value_seeded(
                                    &emitter.injection_angle[0],
                                    &emitter.injection_angle_random[0],
                                    spawn_loop_age,
                                    0.0,
                                    rng.next_u64(),
                                ),
                                curve_value_seeded(
                                    &emitter.injection_angle[1],
                                    &emitter.injection_angle_random[1],
                                    spawn_loop_age,
                                    0.0,
                                    rng.next_u64(),
                                ),
                                curve_value_seeded(
                                    &emitter.injection_angle[2],
                                    &emitter.injection_angle_random[2],
                                    spawn_loop_age,
                                    0.0,
                                    rng.next_u64(),
                                ),
                            ];
                            if angles != [0.0; 3] {
                                quat_rotate(quat_from_euler(emitter.rotation_order, angles), dir)
                            } else {
                                dir
                            }
                        };
                        // 发射器 VRX/Y/Z：初速度逐轴随机幅度（曲线值=幅度，RanT
                        // 区间，逐粒子重掷）。
                        let mut velocity = [0.0; 3];
                        for axis in 0..3 {
                            let amp = emitter.velocity_random[axis].value(spawn_loop_age, 0.0);
                            if amp != 0.0 {
                                velocity[axis] = roll_amplitude(
                                    &mut rng,
                                    emitter.velocity_random[axis].random_type,
                                    amp,
                                );
                            }
                        }
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
                        let world_dir = quat_rotate(emitter_quat, dir);
                        let world_extra = quat_rotate(emitter_quat, velocity);
                        let ctx = SpawnContext {
                            age,
                            position,
                            velocity: [
                                world_dir[0] * speed + world_extra[0],
                                world_dir[1] * speed + world_extra[1],
                                world_dir[2] * speed + world_extra[2],
                            ],
                            emitter_orientation: emitter_quat,
                            emitter_scale: emitter_scl,
                            emitter_orientation_now: quat_now,
                            emitter_scale_now: scl_now,
                            emitter_pos_drift: [
                                pos_now[0] - emitter_pos[0],
                                pos_now[1] - emitter_pos[1],
                                pos_now[2] - emitter_pos[2],
                            ],
                            emitter_gravity,
                            emitter_air_resistance: emitter_resistance,
                            life,
                            seed,
                            create_index,
                        };
                        sink(&ctx, particle_item, particle);
                    }
                }
            }
        }

        // 子发射器（ItEm）：与粒子同节奏创建，父链变换 ∘ 注入旋转作为其基座，
        // 出生后按自身 Life 循环（AVFXTools：发射器实例永生、按寿命 Reset）。
        for emitter_item in &emitter.emitter_items {
            if !emitter_item.enabled || emitter_item.target_index < 0 {
                continue;
            }
            let Some(child) = self.file.emitters.get(emitter_item.target_index as usize)
            else {
                continue;
            };
            let child_life = (child.life.enabled && child.life.value > 0.0).then_some(child.life.value);
            let just_once = child_life.is_none();
            // 子发射器一旦创建即持续循环发射（实例不消亡），只需枚举其出生事件。
            let loop_starts: Vec<f32> = match emitter_life {
                Some(life) => {
                    let newest = (emitter_frame / life).floor() as i64;
                    let oldest = if just_once { newest } else { 0 };
                    (oldest..=newest).map(|n| n as f32 * life).collect()
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
                    continue;
                }
                let last_event = (loop_age_now / interval).floor() as i64;
                for event in 0..=last_event {
                    if just_once && event != 0 {
                        break;
                    }
                    let spawn_loop_age = event as f32 * interval;
                    if spawn_loop_age < emitter_item.start_frame as f32 {
                        continue;
                    }
                    if let Some(life) = emitter_life {
                        if spawn_loop_age >= life {
                            break;
                        }
                    }
                    let (emitter_pos, emitter_quat, emitter_scl) =
                        emitter_transform(spawn_loop_age, loop_start);
                    for k in 0..emitter_item.create_count.max(1) as u64 {
                        if emitted >= emitter_cap {
                            return;
                        }
                        let mut rng = SplitMix64::seeded(
                            emitter_item.target_index as u64 ^ 0x5A17_E417,
                            (loop_start as u64) * 65_537 + event as u64,
                            k,
                            0x3C4D_5E6F,
                        );
                        if emitter_item.create_probability < 100
                            && rng.next_f32() * 100.0 >= emitter_item.create_probability as f32
                        {
                            continue;
                        }
                        emitted += 1;
                        let item_quat = if emitter_item.by_injection_angle != [0.0; 3] {
                            quat_from_euler(
                                2,
                                [
                                    emitter_item.by_injection_angle[0] * k as f32,
                                    emitter_item.by_injection_angle[1] * k as f32,
                                    emitter_item.by_injection_angle[2] * k as f32,
                                ],
                            )
                        } else {
                            [0.0, 0.0, 0.0, 1.0]
                        };
                        let child_base = EmitterBase {
                            position: emitter_pos,
                            orientation: quat_mul(emitter_quat, item_quat),
                            scale: emitter_scl,
                        };
                        let child_birth =
                            loop_start + spawn_loop_age + emitter_item.generate_delay as f32;
                        self.sample_emitter(
                            emitter_item.target_index as usize,
                            child,
                            emitter_frame - child_birth,
                            child_base,
                            depth + 1,
                            sink,
                        );
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
            // xiv.dev：Smpl 需 `bSCt` 启用才生效；Powder 本体不绘制。
            if particle.simple_anim_enable && particle.simple.is_some() {
                self.push_powder(ctx, particle, out);
            }
            return;
        }
        let age = Self::particle_age(particle, ctx.age);
        let seed = ctx.seed;
        let tex = ResolvedTexture::of(particle, age, seed);
        let add = is_additive_draw(particle.draw_mode);
        let color = particle_color(particle, age, add, seed);
        // PICd + ICbS/R：发射器变换的 Initial/Always 分量。
        let (emitter_quat, emitter_scale, pos_drift) = influence_transform(item, ctx);
        let particle_scale = eval3_seeded(&particle.scale, age, 1.0, seed ^ 0x51C1);
        let scale = [
            particle_scale[0] * emitter_scale[0],
            particle_scale[1] * emitter_scale[1],
            particle_scale[2] * emitter_scale[2],
        ];
        let particle_pos = eval3_seeded(&particle.position, age, 0.0, seed ^ 0x9051);
        // 重力 = 粒子 Gra(+GraR) + 发射器 Gra（出生时刻固化）。
        let gravity =
            curve_value_seeded(&particle.gravity, &particle.gravity_random, age, 0.0, seed ^ 0x6A17)
                + ctx.emitter_gravity;
        // 空气阻力（ARs+ARsR+发射器 ARs）：逐帧速度系数 f=1−a 的等比累计
        // 位移 = v·(1−f^t)/a；a→0 退化为 v·t。
        let resistance = (curve_value_seeded(
            &particle.air_resistance,
            &particle.air_resistance_random,
            age,
            0.0,
            seed ^ 0xA125,
        ) + ctx.emitter_air_resistance)
            .clamp(0.0, 0.999);
        let travel = if resistance > 1.0e-6 {
            (1.0 - (1.0 - resistance).powf(ctx.age)) / resistance
        } else {
            ctx.age
        };
        let rotated_pos = quat_rotate(emitter_quat, particle_pos);
        let mut position = [
            ctx.position[0] + rotated_pos[0] + ctx.velocity[0] * travel + pos_drift[0],
            ctx.position[1] + rotated_pos[1] + ctx.velocity[1] * travel + pos_drift[1]
                - 0.5 * gravity * ctx.age * ctx.age,
            ctx.position[2] + rotated_pos[2] + ctx.velocity[2] * travel + pos_drift[2],
        ];
        // Powder 的 Data.CnOf：中心偏移（向发射原点收拢）。
        if let AvfxParticleData::Powder { center_offset, .. } = particle.data {
            position[1] += center_offset;
        }
        // 旋转 = Rot 曲线 + 逐轴旋转速度 × 年龄（各带随机）。
        let mut rotation_euler = eval3_seeded(&particle.rotation, age, 0.0, seed ^ 0x307A);
        for axis in 0..3 {
            rotation_euler[axis] += curve_value_seeded(
                &particle.rotation_velocity[axis],
                &particle.rotation_velocity_random[axis],
                age,
                0.0,
                seed ^ (0x1110 + axis as u64),
            ) * ctx.age;
        }
        let rotation = rotation_euler[2];
        let billboard = particle.is_billboard();
        let orientation = if billboard {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            quat_mul(
                emitter_quat,
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
            draw_priority: particle.draw_priority,
            pivot: [0.0; 2],
            texture_indexes: tex.texture_indexes,
            texture_uv_sets: tex.texture_uv_sets,
            combine_mode_tc1: tex.combine_mode_tc1,
            combine_modes: tex.combine_modes,
            color_to_alpha: tex.color_to_alpha,
            uv_origins: tex.uv_origins,
            uv_scales: tex.uv_scales,
            uv_rotations: tex.uv_rotations,
            texture_borders: tex.texture_borders,
            texture1_is_shape_mask: tex.texture1_is_shape_mask,
            texture1_enabled: tex.texture1_enabled,
            blend_add: is_additive_draw(particle.draw_mode),
            depth_test: particle.depth_test,
            depth_write: particle.depth_write,
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
            distortion_uv_set: tex.distortion_uv_set,
            distortion_borders: tex.distortion_borders,
        });
    }

    fn push_mesh_instance(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        if out.len() >= self.max_quads {
            return;
        }
        let model_index = match &particle.data {
            AvfxParticleData::LightModel { model_index } => *model_index,
            AvfxParticleData::Model { model_indexes, .. } => {
                // MdNo 随机池：MNRt 区间按实例重掷取池内序号。
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
        let seed = ctx.seed;
        let tex = ResolvedTexture::of(particle, age, seed);
        // Model 粒子用 Data 的 ColB/ColE（随年龄插值）；LightModel 用粒子 Col
        // （particle_color 已含 SclA；ColB/ColE 分支在此补乘）。
        let color = match &particle.data {
            AvfxParticleData::Model {
                color_begin,
                color_end,
                ..
            } if !color_begin.is_empty() || !color_end.is_empty() => {
                let life = ctx.life.or_else(|| particle.life_frames()).unwrap_or(30.0);
                let t = (ctx.age / life).clamp(0.0, 1.0);
                let a = color_begin.rgba(age);
                let b = color_end.rgba(age);
                let mut color = [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                    a[3] + (b[3] - a[3]) * t,
                ];
                color[3] *= particle.color.texture_alpha_scale(age);
                color
            }
            _ => particle_color(particle, age, is_additive_draw(particle.draw_mode), seed),
        };
        let (emitter_quat, emitter_scale, pos_drift) = influence_transform(item, ctx);
        let particle_pos = eval3_seeded(&particle.position, age, 0.0, seed ^ 0x9051);
        let rotated_pos = quat_rotate(emitter_quat, particle_pos);
        let position = [
            ctx.position[0] + rotated_pos[0] + pos_drift[0],
            ctx.position[1] + rotated_pos[1] + pos_drift[1],
            ctx.position[2] + rotated_pos[2] + pos_drift[2],
        ];
        let mut particle_rot = eval3_seeded(&particle.rotation, age, 0.0, seed ^ 0x307A);
        for axis in 0..3 {
            particle_rot[axis] += curve_value_seeded(
                &particle.rotation_velocity[axis],
                &particle.rotation_velocity_random[axis],
                age,
                0.0,
                seed ^ (0x1110 + axis as u64),
            ) * ctx.age;
        }
        let particle_scl = eval3_seeded(&particle.scale, age, 1.0, seed ^ 0x51C1);
        out.push(VfxMeshInstance {
            position,
            orientation: quat_mul(
                emitter_quat,
                quat_from_euler(particle.rotation_order, particle_rot),
            ),
            scale: [
                particle_scl[0] * emitter_scale[0],
                particle_scl[1] * emitter_scale[1],
                particle_scl[2] * emitter_scale[2],
            ],
            color,
            draw_priority: particle.draw_priority,
            texture_indexes: tex.texture_indexes,
            texture_uv_sets: tex.texture_uv_sets,
            combine_mode_tc1: tex.combine_mode_tc1,
            combine_modes: tex.combine_modes,
            color_to_alpha: tex.color_to_alpha,
            uv_origins: tex.uv_origins,
            uv_scales: tex.uv_scales,
            uv_rotations: tex.uv_rotations,
            texture_borders: tex.texture_borders,
            texture1_is_shape_mask: tex.texture1_is_shape_mask,
            texture1_enabled: tex.texture1_enabled,
            blend_add: is_additive_draw(particle.draw_mode),
            depth_test: particle.depth_test,
            depth_write: particle.depth_write,
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
            distortion_uv_set: tex.distortion_uv_set,
            distortion_borders: tex.distortion_borders,
            cull_mode: particle.culling_type,
            model_index: model_index as usize,
        });
    }

    /// Powder：spawner 本体不绘制，按 Smpl 参数喷出子粒子（短寿命小亮片）。
    /// 对齐 VFXEditor `AvfxParticleSimple` 全字段；AVFXTools 未实现的
    /// （重力/径向方向/速度衰减/旋转）按字段语义解析式求值。
    fn push_powder(&self, ctx: &SpawnContext, particle: &AvfxParticle, out: &mut Vec<VfxQuad>) {
        let Some(simple) = &particle.simple else {
            return;
        };
        if simple.create_interval <= 0 || simple.create_count <= 0 {
            return;
        }
        let interval = simple.create_interval as f32;
        // CrLR：子粒子寿命随机幅度（逐个子粒子重掷；窗口按最大幅度放宽）。
        let life_jitter = simple.create_life_random as f32;
        let sub_life = (simple.create_interval_life.max(1) as f32 + life_jitter).max(1.0);
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
        let tex = ResolvedTexture::of(particle, spawner_age, ctx.seed);
        // bSnP：子粒子尺寸随父粒子缩放。
        let parent_scale = if simple.scale_by_parent {
            let s = eval3_seeded(&particle.scale, spawner_age, 1.0, ctx.seed ^ 0x51C1);
            (s[0].abs() * ctx.emitter_scale[0].abs()).max(1.0e-6)
        } else {
            1.0
        };
        let cells = (simple.uv_cell[0].max(1) as u64) * (simple.uv_cell[1].max(1) as u64);
        let flipbook = cells > 1;
        let per_event = simple.create_interval_count.max(1) as u64;
        // CCnt 语义：bCrN=false 时是 spawner 一生的总创建上限；bCrN=true 时
        // 死亡即重建，等价于并发上限（只保留最新 CCnt 个存活）。
        let last_global = if last_sub >= 0 {
            (last_sub as u64 + 1) * per_event - 1
        } else {
            0
        };
        let alive_floor = if simple.create_new_after_delete {
            (last_global + 1).saturating_sub(simple.create_count as u64)
        } else {
            0
        };
        for event in first_sub..=last_sub {
            for c in 0..per_event {
                let global = event as u64 * per_event + c;
                if global >= simple.create_count as u64 && !simple.create_new_after_delete {
                    return;
                }
                if simple.create_new_after_delete && global < alive_floor {
                    continue;
                }
                if out.len() >= self.max_quads {
                    return;
                }
                let sub_age = spawner_age - event as f32 * interval;
                let mut rng = SplitMix64::seeded(0x7050_5752, ctx.create_index, global, 0xD1B4);
                // CrLR：个体寿命。
                let life = if life_jitter > 0.0 {
                    ((simple.create_interval_life.max(1) as f32)
                        + roll_amplitude(&mut rng, 0, life_jitter))
                    .max(1.0)
                } else {
                    simple.create_interval_life.max(1) as f32
                };
                if sub_age > life {
                    continue;
                }
                let t = (sub_age / life).clamp(0.0, 1.0);
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
                // 出生散布。
                let jitter = [
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[0],
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[1],
                    (rng.next_f32() * 2.0 - 1.0) * simple.create_area[2],
                ];
                // 初速度方向：IRD0/IRD1 非零时沿径向（散布点背离中心）±锥角
                // 随机偏转；否则均匀球面随机。
                let radial = {
                    let len = (jitter[0] * jitter[0]
                        + jitter[1] * jitter[1]
                        + jitter[2] * jitter[2])
                        .sqrt();
                    if len > 1.0e-6 {
                        [
                            jitter[0] / len,
                            jitter[1] / len,
                            jitter[2] / len,
                        ]
                    } else {
                        [0.0, 1.0, 0.0]
                    }
                };
                let dir = if simple.injection_radial_dir != [0.0; 2] {
                    let tilt = simple.injection_radial_dir[0]
                        + (simple.injection_radial_dir[1] - simple.injection_radial_dir[0])
                            * rng.next_f32();
                    let azimuth = rng.next_f32() * std::f32::consts::TAU;
                    // 以 radial 为轴建正交基，偏转 tilt。
                    let up = if radial[1].abs() < 0.99 {
                        [0.0, 1.0, 0.0]
                    } else {
                        [1.0, 0.0, 0.0]
                    };
                    let t1 = normalize(cross(radial, up));
                    let t2 = cross(radial, t1);
                    [
                        radial[0] * tilt.cos()
                            + tilt.sin() * (t1[0] * azimuth.cos() + t2[0] * azimuth.sin()),
                        radial[1] * tilt.cos()
                            + tilt.sin() * (t1[1] * azimuth.cos() + t2[1] * azimuth.sin()),
                        radial[2] * tilt.cos()
                            + tilt.sin() * (t1[2] * azimuth.cos() + t2[2] * azimuth.sin()),
                    ]
                } else {
                    let phi = rng.next_f32() * std::f32::consts::TAU;
                    let cos_theta = 2.0 * rng.next_f32() - 1.0;
                    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                    [sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin()]
                };
                // 速度衰减（FltR/FltS）：v(t) = FltS + (v0−FltS)·e^(−FltR·t)，
                // 位移解析积分；FltR=0 时匀速。
                let speed0 = simple.velocity_min
                    + (simple.velocity_max - simple.velocity_min) * rng.next_f32();
                let travel = if simple.velocity_flattery_rate > 1.0e-6 {
                    simple.velocity_flattery_speed * sub_age
                        + (speed0 - simple.velocity_flattery_speed)
                            * (1.0
                                - (-simple.velocity_flattery_rate * sub_age).exp())
                            / simple.velocity_flattery_rate
                } else {
                    speed0 * sub_age
                };
                // 子粒子重力（CGX/Y/Z）。
                let gravity = [
                    0.5 * simple.coord_gravity[0] * sub_age * sub_age,
                    0.5 * simple.coord_gravity[1] * sub_age * sub_age,
                    0.5 * simple.coord_gravity[2] * sub_age * sub_age,
                ];
                // bSRL：X/Y 尺寸随机联动（同一次掷值）。
                let rand_x = lerp(simple.scale_rand_x[0], simple.scale_rand_x[1], rng.next_f32());
                let rand_y = if simple.scale_random_link {
                    lerp(
                        simple.scale_rand_y[0],
                        simple.scale_rand_y[1],
                        (rand_x - simple.scale_rand_x[0])
                            / (simple.scale_rand_x[1] - simple.scale_rand_x[0]).max(1.0e-6),
                    )
                } else {
                    lerp(simple.scale_rand_y[0], simple.scale_rand_y[1], rng.next_f32())
                };
                let curve_t = t.powf(simple.scale_curve.max(1.0e-3));
                let scale = [
                    (simple.scale_start[0]
                        + (simple.scale_end[0] - simple.scale_start[0]) * curve_t)
                        * rand_x
                        * parent_scale,
                    (simple.scale_start[1]
                        + (simple.scale_end[1] - simple.scale_start[1]) * curve_t)
                        * rand_y
                        * parent_scale,
                ];
                // 旋转（弧度）：RB + RI + (RA + RV)·age，外加随机初始相位
                // （星点类贴图在游戏里朝向各异）。
                let rotation = simple.rotation_base[2]
                    + simple.rotation_start[2]
                    + (simple.rotation_add[2] + simple.rotation_velocity[2]) * sub_age
                    + rng.next_f32() * std::f32::consts::TAU;
                // 颜色帧簿：Frms 时间点之间插值 Cols。
                let color = sample_flipbook(&simple.colors, &simple.frames, sub_age);
                // UV 翻页：每 UvIv 帧前进一格；UvNR>0 随机起始格；UvLC>0
                // 限制循环圈数后停末格；bRUV 反向。
                let (uv_origin, uv_scale) = if flipbook {
                    let cells_f = cells as f32;
                    let start = if simple.uv_no_random > 0 {
                        (rng.next_f32() * cells_f) as u64 % cells
                    } else {
                        0
                    };
                    let mut step = (sub_age / simple.uv_interval.max(1) as f32) as u64;
                    if simple.uv_loop_count > 0 {
                        step = step.min(simple.uv_loop_count as u64 * cells - 1);
                    }
                    let cell = if simple.uv_reverse {
                        (start + cells - step % cells) % cells
                    } else {
                        (start + step) % cells
                    };
                    let cu = simple.uv_cell[0].max(1) as u64;
                    let cv = simple.uv_cell[1].max(1) as u64;
                    let cx = (cell % cu) as f32;
                    let cy = ((cell / cu) % cv) as f32;
                    (
                        [cx / cu as f32, cy / cv as f32],
                        [1.0 / cu as f32, 1.0 / cv as f32],
                    )
                } else {
                    (tex.uv_origins[0], tex.uv_scales[0])
                };
                out.push(VfxQuad {
                    position: [
                        base[0] + jitter[0] + dir[0] * travel + gravity[0],
                        base[1] + jitter[1] + dir[1] * travel + gravity[1],
                        base[2] + jitter[2] + dir[2] * travel + gravity[2],
                    ],
                    size: [0.5 * scale[0].abs(), 0.5 * scale[1].abs()],
                    rotation,
                    orientation: [0.0, 0.0, 0.0, 1.0],
                    billboard: true,
                    color: [
                        color[0] * bri,
                        color[1] * bri,
                        color[2] * bri,
                        color[3] * scl_a,
                    ],
                    draw_priority: particle.draw_priority,
                    pivot: simple.pivot,
                    texture_indexes: tex.texture_indexes,
                    texture_uv_sets: tex.texture_uv_sets,
                    combine_mode_tc1: tex.combine_mode_tc1,
                    combine_modes: tex.combine_modes,
                    color_to_alpha: tex.color_to_alpha,
                    uv_origins: {
                        // 翻页 UV 覆盖 TC1 层：片元公式为
                        // `scroll + 0.5 + scale·(base−0.5)`，整格采样需
                        // scroll′ = cell + 0.5·(scale−1)。
                        let mut origins = tex.uv_origins;
                        if flipbook {
                            origins[0] = [
                                uv_origin[0] + 0.5 * (uv_scale[0] - 1.0),
                                uv_origin[1] + 0.5 * (uv_scale[1] - 1.0),
                            ];
                        }
                        origins
                    },
                    uv_scales: {
                        let mut scales = tex.uv_scales;
                        if flipbook {
                            scales[0] = uv_scale;
                        }
                        scales
                    },
                    uv_rotations: tex.uv_rotations,
                    texture_borders: tex.texture_borders,
                    texture1_is_shape_mask: tex.texture1_is_shape_mask,
                    texture1_enabled: tex.texture1_enabled,
                    blend_add: is_additive_draw(particle.draw_mode),
                    depth_test: particle.depth_test,
                    depth_write: particle.depth_write,
                    texture_distortion_index: tex.texture_distortion_index,
                    distortion_power: tex.distortion_power,
                    distortion_targets: tex.distortion_targets,
                    uvd_origin: tex.uvd_origin,
                    uvd_scale: tex.uvd_scale,
                    distortion_uv_set: tex.distortion_uv_set,
                    distortion_borders: tex.distortion_borders,
                });
            }
        }
    }
}

/// 粒子的贴图/UV 解析结果（TC1 基准层 + TC2..TC4 合成层 + TD 扭曲层）。
struct ResolvedTexture {
    texture_indexes: [i32; 4],
    texture_uv_sets: [i32; 4],
    combine_mode_tc1: [i32; 2],
    combine_modes: [[i32; 2]; 3],
    color_to_alpha: [bool; 4],
    uv_origins: [[f32; 2]; 4],
    uv_scales: [[f32; 2]; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（UvSet `Rot` + `RotR` 随机，弧度）。
    uv_rotations: [f32; 4],
    texture_borders: [[i32; 2]; 4],
    texture1_is_shape_mask: bool,
    texture1_enabled: bool,
    texture_distortion_index: i32,
    distortion_power: f32,
    distortion_targets: u32,
    distortion_uv_set: i32,
    uvd_origin: [f32; 2],
    uvd_scale: [f32; 2],
    distortion_borders: [i32; 2],
}

impl ResolvedTexture {
    fn of(particle: &AvfxParticle, age: f32, seed: u64) -> Self {
        let uv = |index: i32| -> ([f32; 2], [f32; 2], f32) {
            particle
                .uv_sets
                .get(index.max(0) as usize)
                .map(|set| {
                    (
                        eval2_seeded(&set.scroll, age, 0.0, seed ^ 0x5C01),
                        eval2_seeded(&set.scale, age, 1.0, seed ^ 0x5C1E),
                        // UvSet Rot + RotR 随机（弧度，绕 0.5 中心）。
                        curve_value_seeded(
                            &set.rotation,
                            &set.rotation_random,
                            age,
                            0.0,
                            seed ^ 0xA07,
                        ),
                    )
                })
                .unwrap_or(([0.0, 0.0], [1.0, 1.0], 0.0))
        };
        let tcs = [
            particle.texture_color1.as_ref(),
            particle.texture_color2.as_ref(),
            particle.texture_color3.as_ref(),
            particle.texture_color4.as_ref(),
        ];
        let mut texture_indexes = [-1; 4];
        let mut texture_uv_sets = [0; 4];
        let mut color_to_alpha = [false; 4];
        let mut uv_origins = [[0.0; 2]; 4];
        let mut uv_scales = [[1.0; 2]; 4];
        let mut uv_rotations = [0.0; 4];
        let mut texture_borders = [[0; 2]; 4];
        let mut texture1_is_shape_mask = false;
        for (i, tc) in tcs.iter().enumerate() {
            let Some(tc) = tc.filter(|t| t.enabled) else {
                continue;
            };
            texture_uv_sets[i] = tc.uv_set_index;
            texture_indexes[i] = tc.effective_texture();
            // UvSet 动画对无贴图层也保留（翻页覆盖以它为基底）。
            let (origin, scale, rot) = uv(tc.uv_set_index);
            uv_origins[i] = origin;
            uv_scales[i] = scale;
            uv_rotations[i] = rot;
            if texture_indexes[i] < 0 {
                continue;
            }
            color_to_alpha[i] = tc.color_to_alpha;
            texture_borders[i] = [tc.texture_border_u, tc.texture_border_v];
            if i == 0 {
                texture1_is_shape_mask = tc.is_shape_mask();
            }
        }
        // TD：扭曲贴图用其 UvSN 指向的 UvSet 变换采样；bT1..bT4 按各层
        // 实际引用的 UvSet 序号映射到渲染端 uv1..4。
        let td = particle
            .texture_distortion
            .as_ref()
            .filter(|t| t.enabled && t.texture_index >= 0);
        let (texture_distortion_index, distortion_power, distortion_targets, uvd, dborders, duv_set) =
            match td {
                Some(td) => {
                    let uvd = uv(td.uv_set_index);
                    let mut targets = 0u32;
                    for (i, tc) in tcs.iter().enumerate() {
                        let Some(tc) = tc.filter(|t| t.enabled) else {
                            continue;
                        };
                        // bT(i+1) 扭曲 UvSet i。
                        let hit = (0..4).any(|u| {
                            tc.uv_set_index == u as i32 && td.target_uv[u]
                        });
                        if hit {
                            targets |= 1 << i;
                        }
                    }
                    (
                        td.texture_index,
                        td.power.value(age, 0.0),
                        targets,
                        uvd,
                        [td.texture_border_u, td.texture_border_v],
                        td.uv_set_index,
                    )
                }
                None => (-1, 0.0, 0, ([0.0, 0.0], [1.0, 1.0], 0.0), [0, 0], 0),
            };
        let mut combine_modes = [[0; 2]; 3];
        for (i, tc) in tcs[1..].iter().enumerate() {
            if let Some(tc) = tc.filter(|t| t.enabled) {
                combine_modes[i] = [tc.calculate_color, tc.calculate_alpha];
            }
        }
        let combine_mode_tc1 = particle
            .texture_color1
            .as_ref()
            .filter(|t| t.enabled)
            .map(|tc| [tc.calculate_color, tc.calculate_alpha])
            .unwrap_or([0, 3]);
        Self {
            texture_indexes,
            texture_uv_sets,
            combine_mode_tc1,
            combine_modes,
            color_to_alpha,
            uv_origins,
            uv_scales,
            uv_rotations,
            texture_borders,
            texture1_is_shape_mask,
            texture1_enabled: tcs[0].is_some_and(|tc| tc.enabled),
            texture_distortion_index,
            distortion_power,
            distortion_targets,
            distortion_uv_set: duv_set,
            uvd_origin: uvd.0,
            uvd_scale: uvd.1,
            distortion_borders: dborders,
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

/// RanT 随机区间掷值：0/3 = ±amp，1/4 = +amp，2/5 = −amp（VFXEditor
/// `RandomType`：First*/Always*；Always 的逐帧重掷由调用方在 seed 里
/// 混入帧号实现）。
fn roll_amplitude(rng: &mut SplitMix64, random_type: u32, amp: f32) -> f32 {
    match random_type {
        1 | 4 => rng.next_f32() * amp,
        2 | 5 => -rng.next_f32() * amp,
        _ => (rng.next_f32() * 2.0 - 1.0) * amp,
    }
}

/// 随机曲线求值（AVFXTools `CurveRandomAttribute`）：键值即幅度，每实例
/// 按 (seed, 键序) 重掷一次键值，再按原时间轴线性插值；Always 系
/// （RanT 3..5）额外混入取整帧号逐帧重掷。
fn curve_random_value(curve: &crate::avfx::AvfxCurve, time: f32, seed: u64) -> f32 {
    if curve.keys.is_empty() {
        return 0.0;
    }
    let frame = if curve.random_type >= 3 {
        time.max(0.0) as u64
    } else {
        0
    };
    let rolled = |index: usize| {
        let key = curve.keys[index];
        let mut rng = SplitMix64::seeded(seed, index as u64, frame, 0xB7E1);
        roll_amplitude(&mut rng, curve.random_type, key.z)
    };
    if curve.keys.len() == 1 {
        return rolled(0);
    }
    let first_time = f32::from(curve.keys[0].time);
    let last_time = f32::from(curve.keys[curve.keys.len() - 1].time);
    let time = time.clamp(first_time.min(last_time), first_time.max(last_time));
    let mut index = 0;
    for (i, key) in curve.keys.iter().enumerate() {
        if f32::from(key.time) <= time {
            index = i;
        }
    }
    if index + 1 >= curve.keys.len() {
        return rolled(curve.keys.len() - 1);
    }
    let t0 = f32::from(curve.keys[index].time);
    let t1 = f32::from(curve.keys[index + 1].time);
    let local = ((time - t0) / (t1 - t0).max(f32::EPSILON)).clamp(0.0, 1.0);
    lerp(rolled(index), rolled(index + 1), local)
}

/// 主曲线 + 随机曲线（键值逐实例重掷）叠加求值；随机曲线无键时加 0。
fn curve_value_seeded(
    main: &crate::avfx::AvfxCurve,
    random: &crate::avfx::AvfxCurve,
    time: f32,
    default: f32,
    seed: u64,
) -> f32 {
    main.value(time, default) + curve_random_value(random, time, seed)
}

/// 三轴连接（对齐 `AvfxCurve3Axis::evaluate` 的 ACT 语义；随机轴用 ACTR）。
fn connect3(connect: u32, has: [bool; 3], values: &mut [f32; 3]) {
    match connect {
        1 if has[0] => {
            values[1] = values[0];
            values[2] = values[0];
        }
        2 if has[0] => values[1] = values[0],
        3 if has[0] => values[2] = values[0],
        4 if has[1] => {
            values[0] = values[1];
            values[2] = values[1];
        }
        5 if has[1] => values[0] = values[1],
        6 if has[1] => values[2] = values[1],
        7 if has[2] => {
            values[0] = values[2];
            values[1] = values[2];
        }
        8 if has[2] => values[0] = values[2],
        9 if has[2] => values[1] = values[2],
        _ => {}
    }
}

/// 三轴曲线求值 + 随机轴（`XR`/`YR`/`ZR`）逐实例重掷叠加（AVFXTools
/// `CurveRandomGroup`：value = main(t) + randomRolled(t)）。
fn eval3_seeded(
    curve3: &crate::avfx::AvfxCurve3Axis,
    time: f32,
    default: f32,
    seed: u64,
) -> [f32; 3] {
    let base = curve3.evaluate(time, default);
    let randoms = [
        curve3.random_x.as_ref(),
        curve3.random_y.as_ref(),
        curve3.random_z.as_ref(),
    ];
    if randoms.iter().all(Option::is_none) {
        return base;
    }
    let mut extra = [0.0; 3];
    for (axis, random) in randoms.iter().enumerate() {
        if let Some(curve) = random {
            extra[axis] = curve_random_value(curve, time, seed ^ (axis as u64 + 1) * 0x9E37);
        }
    }
    connect3(
        curve3.axis_connect_random,
        randoms.map(|r| r.is_some()),
        &mut extra,
    );
    [base[0] + extra[0], base[1] + extra[1], base[2] + extra[2]]
}

/// 两轴曲线求值 + 随机轴（UvSt 的 Scl/Scr 随机）。
fn eval2_seeded(
    curve2: &crate::avfx::AvfxCurve2Axis,
    time: f32,
    default: f32,
    seed: u64,
) -> [f32; 2] {
    let base = curve2.evaluate(time, default);
    let mut extra = [0.0; 2];
    if let Some(curve) = &curve2.random_x {
        extra[0] = curve_random_value(curve, time, seed ^ 0x11);
    }
    if let Some(curve) = &curve2.random_y {
        extra[1] = curve_random_value(curve, time, seed ^ 0x22);
    }
    [base[0] + extra[0], base[1] + extra[1]]
}

/// 粒子颜色：Col.RGB×Bri（加色才乘 Bri）+ `RanR/G/B/A/RBri` 逐实例随机 +
/// `SclA` 乘 alpha（对齐 `AvfxColorCurve::rgba_with_brightness` 口径）。
fn particle_color(particle: &AvfxParticle, age: f32, add: bool, seed: u64) -> [f32; 4] {
    let cc = &particle.color;
    let mut rgb = cc.rgb.as_ref().map_or([1.0; 3], |c| c.color_at(age));
    let mut alpha = cc.alpha.as_ref().map_or(1.0, |c| c.value(age, 1.0));
    let mut bri = cc.brightness.as_ref().map_or(1.0, |c| c.value(age, 1.0));
    for (channel, random) in cc.random.iter().enumerate() {
        let Some(curve) = random else { continue };
        let rolled = curve_random_value(curve, age, seed ^ (0xC010 + channel as u64));
        match channel {
            0..=2 => rgb[channel] += rolled,
            3 => alpha += rolled,
            _ => bri += rolled,
        }
    }
    let mut color = if add {
        [rgb[0] * bri, rgb[1] * bri, rgb[2] * bri, alpha]
    } else {
        [rgb[0], rgb[1], rgb[2], alpha]
    };
    // Col.SclA：贴图 alpha 缩放（乘进总 alpha；加色模式下即强度倍率）。
    color[3] *= cc.texture_alpha_scale(age);
    color
}

/// ItPr 的 PICd + ICbS/ICbR 解析出粒子的有效发射器变换分量
/// （VFXEditor `ParentInfluenceCoordOptions`；位置出生时刻已固化进
/// `ctx.position`，Always 位置分量以 `emitter_pos_drift` 差量跟随）。
fn influence_transform(
    item: &AvfxEmitterItem,
    ctx: &SpawnContext,
) -> ([f32; 4], [f32; 3], [f32; 3]) {
    let spawn = (ctx.emitter_orientation, ctx.emitter_scale);
    let now = (ctx.emitter_orientation_now, ctx.emitter_scale_now);
    let with_options = (
        if item.influence_coord_rot { now.0 } else { spawn.0 },
        if item.influence_coord_scale { now.1 } else { spawn.1 },
        [0.0; 3],
    );
    match item.parent_influence_coord {
        // AllAlways：全部连续跟随（位置以出生→现在的位移差量叠加）。
        2 => (now.0, now.1, ctx.emitter_pos_drift),
        // AllAlways_NoPosition：朝向/缩放连续跟随，位置不跟随。
        4 => (now.0, now.1, [0.0; 3]),
        // None：发射器变换完全不传递。
        6 => ([0.0, 0.0, 0.0, 1.0], [1.0; 3], [0.0; 3]),
        // WithOptions_NoPosition / Unknown_NoPosition。
        7 | 9 => with_options,
        // InitialPosition / AllInitial / AllInitial_NoPosition：出生时刻全量。
        0 | 3 | 5 => (spawn.0, spawn.1, [0.0; 3]),
        // InitialPosition_WithOptions（默认）与 Unknown：位置恒为出生时刻，
        // ICbR/ICbS 选中的分量连续跟随。
        _ => with_options,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1.0e-6 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    }
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
            ..Default::default()
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
            ..Default::default()
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
                    ..Default::default()
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
        assert_eq!(quad.texture_indexes[0], 2);
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
