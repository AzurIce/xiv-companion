//! avfx 常驻特效的确定性 CPU 模拟：scheduler → timeline → emitter → 粒子批次。
//!
//! 实现参考 VFXEditor 字段定义与 AVFXTools 运行时的子集，已知缺口见
//! `docs/vfx-audit.md`。当前行为：
//! - 帧率 30fps（avfx 时间轴单位为帧，见 `AVFX_FPS`）；
//! - 常驻路径只消费 scheduler `items` 指向的 timeline（12 个 trigger 的
//!   拔刀/收刀切换不在此列）；
//! - timeline 仅在 `LpSt < LpEd` 时循环，否则一次性播放（item 的 EdTm=-1
//!   表示持续到时间轴结束，不重复触发）；
//! - 根实例寿命取 Timeline 条目；子发射器和粒子用名义/所选寿命之比缩放
//!   时钟。局部循环先于 Life 终止，不重新创建实例或重掷随机状态；
//! - 创建数量：CrTm=0 向零截断 CrC，CrTm=1/2 使用 CrCn，非正数不创建；
//!   初始化先触发 1 再触发 0，终止触发 2，后代可继续存活。StFr 区分预推进
//!   和仅设置年龄；两者都不推迟创建，外部父级不随预推进前进；
//!   同一发射器内按事件时间合并创建项，同一事件先 ItPr 后 ItEm；周期项
//!   不使用 GenD。BIAX/Y/Z 按每项 signed short 累计序号加到局部 Euler；
//!   延迟队列、跨发射器时序及客户端随机序列仍待对应；
//! - 出生点 = 发射器基座（子发射器为父链变换，根为 Item Binder 或继承的
//!   Timeline Binder 绑点）∘ emitter
//!   Pos/Rot/Scl（含实例随机轴）∘ 形状偏移（Point/Cone/ConeModel/
//!   CylinderModel/SphereModel/Model 发射顶点）；
//! - PICd 2/3 选择当前/出生父矩阵；PICd 1 按 ICbS/R 沿父链累乘缩放、
//!   累加 Euler，关闭则停止继承。Point/Cone 的 ICbP 可选当前中心，Model
//!   可跟随当前中心或形状旋转后的绑定顶点；其他形状、Binder 和地面查询待补全；
//! - ItPr / ItEm 的 PICo 选择不继承、出生快照或持续继承父级 RGBA；
//!   完整颜色求值后相乘，Powder / Windmill 在继承后量化；
//! - 连续路径由出生时间重建状态，随机数按 scheduler/timeline/父发射器实例用
//!   splitmix64 播种。标量随机曲线先插值幅度，First 共用实例整数百分比；
//!   除 Cone 按创建调用重掷外，Always 仍按整数帧近似，缺少完整更新缓存；
//!   简单 Timeline 循环中有限直接粒子的旧轮尾部可按寿命上界重放，复杂子树
//!   和无界历史仍只采样当前轮；
//! - 连续运动：形状初速度来自注入方向与速度，发射器 VR 不叠加到子粒子。
//!   重力与 ARs/ARsR 位移倍率按历史积分，保留实例循环和随机幅度；子发射器
//!   使用自己的 ARs。粒子 VR 方向与 ARs 联合积分，IPbV 将父级注入速度按
//!   PICd/LoDr 基底转换后继承；非零发射器 VR 与客户端增量时钟仍待补全；
//!   实例寿命含构造期 `Life.ValR` 与条目 `OvrR` 随机；
//! - Powder 未启用 `bSCt` 时绘制单四边形，使用固定 ZXY 旋转、父级 Z 旋转
//!   与逐轴缩放、千分之一颜色量化；GPU 使用相机基底和固定 UV。
//!   启用时按固定 Smpl 槽位生成子粒子，寿命或正 UV 循环终点触发 bCrN
//!   重建并复用随机状态；负 UV 循环停格，bRUV 随机翻转 U。
//!   颜色帧簿使用槽位整数寿命偏移，尺寸指数使用名义寿命；随机尺寸按 byte
//!   量化，三轴旋转按 short 量化并应用 ZXY。SBDT=0 使用相机基底，1..4
//!   使用当前发射器矩阵；绑定基准缩放取 1。CAX/Y/Z 按 0.1 帧先乘速再
//!   位移的递推累计，子步间线性插值；绑定与移动触发仍不完整，未复现客户端
//!   增量更新时钟。
//! - Line 未启用 `bSCt` 时按 RBDT 轴输出单条两顶点 LineList；启用时，已覆盖
//!   安装武器语料使用的无模型、无顶点绑定、无父位置绑定配置。每个 Smpl 槽
//!   独立输出 LineList，按 0.1 帧速度差与 LLin/LLax 生成端点并乘槽位颜色；
//!   任意绘制间隔的历史缓存、客户端随机压缩和其他注入/绑定配置仍未实现。
//! - Polyline 的无 Binder 单中心严格子集已输出专用几何：edge 使用三列，
//!   non-edge 使用两列；`LnCT=0` 生成静态中心线，`LnCT=1` 在受限逐步播放中
//!   按每次正更新保存实例历史并应用 Sft，非 local edge spline 另做 Hermite
//!   重采样。Point 与固定形状参数的 SphereModel 根可生产历史；SphereModel
//!   的径向 CF 使用当前父发射器世界原点。默认预览已保存六组颜色、七条
//!   标量的 First，按原回调顺序求几何、纹理及条件中心宽度；属性和预热
//!   使用各自颜色缓存。完整原轨迹、LOD、绑定来源与 GPU 画面仍未验收。

use crate::avfx::{
    AvfxBinder, AvfxCurve, AvfxEmitter, AvfxEmitterData, AvfxEmitterItem, AvfxFile, AvfxParticle,
    AvfxParticleData, AvfxTimeline, AvfxTimelineItemTarget, ParticleType, signed_byte_index,
};

mod binder;
mod binder_curves;
pub use binder_curves::{VfxClientBinderCurveDefaults, VfxClientDualBinderCurveState};
mod camera_binder;
mod client_random;
mod color_curves;
mod model_skin_curves;
mod model_skin_numeric;
mod model_skin_target;
mod particle_curves;
mod uv_curves;
pub use color_curves::{VfxClientColorCurveDefaults, VfxClientColorCurveState};
pub use model_skin_curves::VfxClientModelSkinCurveDefaults;
pub use model_skin_target::{
    VfxModelSkinCharacterTargets, VfxModelSkinCreationOrder, VfxModelSkinRegistration,
    VfxModelSkinSurface, VfxModelSkinTargetInput, VfxModelSkinTargetList, VfxModelSkinTargetQuery,
    VfxModelSkinTargetSnapshot, VfxModelSkinTargetState, VfxModelSkinTargetStatus,
};
mod client_trig;
pub use client_random::{VfxClientRandomState, VfxSharedRandomStream};
mod clock;
pub use client_trig::VfxClientTrigMode;
mod common_fade;
mod cone;
mod cone_model;
mod cylinder_model;
mod decal;
mod depth_offset;
mod disc;
mod document;
mod draw_order;
pub use draw_order::{depth_sort_camera_row, depth_sort_key, document_sort_key, draw_layer_group};
mod inheritance;
mod integration;
mod laser;
mod line;
mod motion;
mod ownership;
mod playback;
mod point_factory;
mod polygon;
#[allow(dead_code)]
mod polyline;
mod powder;
mod scalar_random;
pub use scalar_random::{VfxClientScalarCurveState, VfxClientScalarPairState};
mod scheduler_input;
mod scheduler_ordinary;
mod scheduling;
mod sphere_model;
mod spline_binder;
mod timeline;
mod trace;
mod vector_curves;
pub use vector_curves::VfxClientVectorCurveState;

pub use camera_binder::{
    VfxCameraBinderConstruction, VfxCameraBinderFrame, VfxCameraBinderInstance,
    VfxCameraBinderOwner, VfxCameraBinderSample, VfxCameraBinderScaleNode, VfxCameraBinderSnapshot,
    VfxCameraBinderState, VfxCameraBinderTimelineSnapshot, VfxCameraViewSnapshot,
};
pub use common_fade::{VfxCommonFadeState, VfxCommonFadeStep, VfxCommonNumericState};
pub use scheduler_input::VfxSchedulerTrigger;
pub use spline_binder::{
    VfxSplineBinderAdvance, VfxSplineBinderConstruction, VfxSplineBinderControl,
    VfxSplineBinderInstance, VfxSplineBinderKnot, VfxSplineBinderPath, VfxSplineBinderState,
};
pub use timeline::{VfxTimelineBirth, VfxTimelineLifecycle};

pub use binder::{
    VfxBinderBirthClock, VfxBinderCameraSnapshot, VfxBinderLifecycle, VfxBinderMatrix,
    VfxBinderObjectSnapshot, VfxBinderQueryScale, VfxBinderQueryStatus, VfxBinderStep,
    VfxBinderTarget, VfxBinderTargetObjectSnapshot, VfxBinderTargetSnapshot,
    VfxLinearBinderAdvance, VfxLinearBinderConstruction, VfxLinearBinderCurves,
    VfxLinearBinderFrame, VfxLinearBinderInitialization, VfxLinearBinderInstance,
    VfxLinearBinderState, VfxPointBinderAdvance, VfxPointBinderConstruction, VfxPointBinderCurves,
    VfxPointBinderFrame, VfxPointBinderInitialization, VfxPointBinderInstance, VfxPointBinderState,
};
pub use decal::VfxDecal;
pub use depth_offset::VfxDepthOffsetParameters;
pub use disc::VfxDisc;
pub use document::VfxDocumentTransform;
pub use laser::VfxLaser;
pub use line::VfxLine;
pub use playback::{VfxModelSkinCameraFacing, VfxModelSkinInstance, VfxPlayback};
pub use polygon::VfxPolygon;
pub use polyline::VfxPolyline;

pub use trace::{
    VfxCreationEvent, VfxCreationTrace, VfxTraceAction, VfxTracePhase, VfxTraceTarget,
};

/// Common +88 is idempotent per object; callers recurse only on its first call.
pub(super) fn unlock_loop_flags(flags: &mut u32) -> bool {
    if *flags & 0x0080_0000 != 0 {
        return false;
    }
    *flags = (*flags & 0x7f3f_ffff) | 0x0080_0000;
    true
}

use clock::{CurveAges, InstanceClock};
use inheritance::{EmitterComponents, ParentComponents, coordinate_mode};
use scheduling::{CreationSchedule, CreationTarget, InjectionCounter};

/// 客户端把秒乘 30 后量化到 0.1 帧；此处只对应单位换算，尚无增量量化。
/// 时间推进和创建阶段的证据见 `docs/vfx-client-shaders.md`。
pub const AVFX_FPS: f32 = 30.0;

/// Column-major identity for particle parent transforms.
pub const VFX_IDENTITY_BASIS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// 单个粒子四边形（渲染端实例数据，世界单位）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxQuad {
    /// 来源 Ptcl 类型；特殊几何的四边形回退保留原类型。
    pub particle_type: Option<ParticleType>,
    /// 来源 Ptcl 序号；合并多个 AVFX 时须重映射，供 Double 绘制分批。
    pub particle_index: usize,
    /// `DsSp`: this instance uses the scene-depth soft-particle variant.
    pub soft_particle: bool,
    /// `AvfxMain.SPFR`, carried with the instance for the depth-fade pass.
    pub soft_particle_fade_range: f32,
    /// `DOTy/DpOf`: Legacy (0) or FixedIntervalNDC (1) depth offset.
    pub depth_offset_type: i32,
    pub depth_offset: f32,
    /// Powder with bSCt disabled: client single-quad geometry, camera basis and UVs.
    pub powder_single: bool,
    /// Windmill quadrant UV arrangement: 0 Default, 1 Mirror.
    pub windmill_uv_type: u8,
    /// Disc's evaluated ring grid; one particle still occupies one output slot.
    pub disc: Option<VfxDisc>,
    /// Polygon's evaluated closed triangle fan.
    pub polygon: Option<VfxPolygon>,
    /// Evaluated Laser dimensions; `Some` selects the dedicated 18-vertex strip.
    pub laser: Option<VfxLaser>,
    /// Evaluated Line; `Some` selects the two-vertex line-list path.
    pub line: Option<VfxLine>,
    /// Evaluated edge Polyline geometry; history production remains staged-only.
    pub polyline: Option<VfxPolyline>,
    /// Evaluated Decal / DecalRing projection parameters.
    pub decal: Option<VfxDecal>,
    pub position: [f32; 3],
    /// 带符号的局部半宽/半高；旋转后缩放存于 parent_basis，此处取 0.5。
    pub size: [f32; 2],
    /// 粒子局部旋转，四元数 xyzw。
    pub orientation: [f32; 4],
    /// 前乘线性基底（列向量），包含父级、LoDr 及旋转后缩放，保留剪切。
    pub parent_basis: [[f32; 3]; 3],
    /// RBDT 3/7 使用的世界空间注入方向（客户端实例 +0x174）。
    pub movement_direction: [f32; 3],
    /// RBDT 3/7 构造方向基底时使用的、尚未应用粒子局部旋转的父基底。
    pub facing_parent_basis: [[f32; 3]; 3],
    /// 普通粒子的 RBDT；Smpl 的 SBDT=0 映射为 5，非 Smpl 忽略此字段。
    pub rotation_direction_base: i32,
    /// HDR 颜色，含 Col 的随机偏移、通道缩放及亮度；Powder 已量化。
    pub color: [f32; 4],
    /// Source root `DwLy`, retained verbatim for scene pass scheduling.
    /// This is separate from the particle's priority within an ordinary bucket.
    pub draw_layer: i32,
    /// Source root `SKO`; client Document ordering consumes this before
    /// allocating each document's ordinary and independent draw-key ranges.
    pub soft_key_offset: f32,
    /// 粒子 `DwPr` 绘制优先级。
    pub draw_priority: i32,
    /// Sampling order within the ordinary list (shared with mesh output), or
    /// within the independent Decal list. The lists require separate GPU passes.
    #[serde(skip)]
    pub draw_order: Option<u64>,
    /// 局部枢轴参数；Smpl `PvtX/Y` 按半尺寸平移四角，再施加旋转。
    pub pivot: [f32; 2],
    /// 4 层颜色贴图（TC1..TC4）的文件级贴图序号（TC1：TLst 优先；-1 = 无）。
    pub texture_indexes: [i32; 4],
    /// 各层引用的 UvSet 序号（网格粒子按它选逐顶点 UV 组；四边形基底
    /// 恒为角点，此字段不参与）。
    pub texture_uv_sets: [i32; 4],
    /// TC1 原始 (TCCT, TCAT)：低 3 / 2 位非零时启用对应 RGB / alpha 通道。
    pub combine_mode_tc1: [i32; 2],
    /// TC2/TC3/TC4 的 (颜色 TCCT, alpha TCAT) 合成模式。
    pub combine_modes: [[i32; 2]; 3],
    /// 各层 bC2A（颜色转 alpha）。
    pub color_to_alpha: [bool; 4],
    /// 各层 UV 原点 + 缩放（各层引用自己的 UvSet）。
    pub uv_origins: [[f32; 2]; 4],
    pub uv_scales: [[f32; 2]; 4],
    /// Each color layer whose referenced UvSet uses `CUvT=ByPixelPosition`.
    pub uv_by_pixel_position: [bool; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（UvSet `Rot`+`RotR`，弧度）。
    pub uv_rotations: [f32; 4],
    /// 各层 U/V 边界模式（0 Repeat、1 Clamp、2 Mirror；渐变贴图 Clamp
    /// 防止越界回绕出异色条纹）。
    pub texture_borders: [[i32; 2]; 4],
    /// 各层 TFT：0 禁用过滤，1 启用，2..4 为更高质量档位。
    pub texture_filters: [i32; 4],
    /// TC1 使用 TLst 引用；只记录资源来源，不改变颜色方程。
    pub texture1_is_shape_mask: bool,
    /// TC1 块存在且启用；四边形和网格禁用时均跳过该层。
    pub texture1_enabled: bool,
    /// TC1 `bUSC`；屏幕拷贝的 alpha 不参与首层颜色。
    pub texture1_use_screen_copy: bool,
    /// 原始 `RMT`，包括渲染器暂不支持的模式。
    pub draw_mode: i32,
    /// `DsDt`/`DsDw`：深度测试/写入（如爪笼壳写深度遮挡身后几何）。
    pub depth_test: bool,
    pub depth_write: bool,
    /// 原始 CulT：0 不剔除、1 留正面、2 留背面；Quad 的 3 按批次先背后正。
    pub cull_mode: i32,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    /// TD 扭曲强度（DPow 按当前绘制年龄求值，Quad 保留客户端字节量化）。
    pub distortion_power: f32,
    /// TD 扭曲目标位（bit0..3 = uv1..4）。
    pub distortion_targets: u32,
    /// TD 贴图采样用 UV（TD 引用的 UvSet 变换）。
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
    /// TD 引用的 UvSet 旋转（弧度）。
    pub uvd_rotation: f32,
    /// TD's referenced UvSet uses `CUvT=ByPixelPosition`.
    pub uvd_by_pixel_position: bool,
    /// TD 引用的 UvSet 序号（网格粒子的基底 UV 选择）。
    pub distortion_uv_set: i32,
    /// TD 边界模式。
    pub distortion_borders: [i32; 2],
    pub distortion_filter: i32,
    /// TP 调色板贴图序号（-1 = 禁用）。
    pub texture_palette_index: i32,
    /// POff + POfR 经客户端 UNORM8 顶点字节量化后的偏移。
    pub palette_offset: f32,
    /// TP 的单一 TBT 同时作用于 U/V。
    pub palette_border: i32,
    pub palette_filter: i32,
}

/// Model's normal-dependent color multiplier; evaluated per fragment, not by age.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxFresnel {
    /// FrsT: 1 camera, 2 fixed world axis, 3 axis transformed with the model.
    pub kind: i32,
    pub direction: [f32; 3],
    pub exponent: f32,
    pub color_begin: [f32; 4],
    pub color_end: [f32; 4],
}

/// 网格粒子实例：Model/LightModel 粒子把粒子 Data 引用的内嵌绘制模型
/// （`Modl` 的 `VDrw` 网格）按实例仿射变换绘制（火舌、光罩壳等轮廓本体）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxMeshInstance {
    pub position: [f32; 3],
    /// 与 VfxQuad 相同，粒子局部旋转。
    pub orientation: [f32; 4],
    /// 与 VfxQuad 相同，包含父级、LoDr 及旋转后缩放的前乘基底。
    pub parent_basis: [[f32; 3]; 3],
    pub movement_direction: [f32; 3],
    pub facing_parent_basis: [[f32; 3]; 3],
    pub rotation_direction_base: i32,
    pub scale: [f32; 3],
    /// HDR 颜色（rgb 可 >1，a 为不透明度）。
    pub color: [f32; 4],
    pub fresnel: Option<VfxFresnel>,
    /// Source root `DwLy`, retained verbatim for scene pass scheduling.
    /// This is separate from the particle's priority within an ordinary bucket.
    pub draw_layer: i32,
    /// Source root `SKO`, retained for camera-dependent Document ordering.
    pub soft_key_offset: f32,
    /// 粒子 `DwPr` 绘制优先级。
    pub draw_priority: i32,
    /// Sampling traversal order within a DwPr bucket, shared with quad output.
    #[serde(skip)]
    pub draw_order: Option<u64>,
    pub texture_indexes: [i32; 4],
    /// 各层引用的 UvSet 序号（按它选逐顶点 UV 组）。
    pub texture_uv_sets: [i32; 4],
    /// TC1 原始 (TCCT, TCAT)：低 3 / 2 位非零时启用对应 RGB / alpha 通道。
    pub combine_mode_tc1: [i32; 2],
    pub combine_modes: [[i32; 2]; 3],
    pub color_to_alpha: [bool; 4],
    pub uv_origins: [[f32; 2]; 4],
    pub uv_scales: [[f32; 2]; 4],
    pub uv_by_pixel_position: [bool; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（弧度）。
    pub uv_rotations: [f32; 4],
    pub texture_borders: [[i32; 2]; 4],
    pub texture_filters: [i32; 4],
    /// TN normal texture and its independently selected UvSet (-1 disables it).
    pub texture_normal_index: i32,
    pub normal_uv_set: i32,
    pub normal_uv_origin: [f32; 2],
    pub normal_uv_scale: [f32; 2],
    pub normal_uv_rotation: f32,
    pub normal_uv_by_pixel_position: bool,
    pub normal_texture_borders: [i32; 2],
    pub normal_texture_filter: i32,
    /// TN NPow: client scales tangent-space X/Y by this value before normalize.
    pub normal_power: f32,
    /// TR reflection controls; the shader expects a cube resource.
    pub reflection_enabled: bool,
    pub reflection_use_screen_copy: bool,
    /// TR `TxNo`; VFXEditor maps this to the AVFX `Tex` table. A cube TEX is
    /// usable by the preview; the client's provider precedence is unverified.
    pub reflection_texture_index: i32,
    /// TR `TFT` filter mode.
    pub reflection_texture_filter: i32,
    pub reflection_calculate_color: i32,
    pub reflection_rate: f32,
    pub reflection_power: f32,
    /// TC1 使用 TLst 引用；只记录资源来源，不改变颜色方程。
    pub texture1_is_shape_mask: bool,
    /// TC1 块存在且启用。
    pub texture1_enabled: bool,
    pub texture1_use_screen_copy: bool,
    pub draw_mode: i32,
    /// `DsDt`/`DsDw`：深度测试/写入。
    pub depth_test: bool,
    pub depth_write: bool,
    /// TD 扭曲贴图序号（-1 = 无扭曲）。
    pub texture_distortion_index: i32,
    pub distortion_power: f32,
    pub distortion_targets: u32,
    pub uvd_origin: [f32; 2],
    pub uvd_scale: [f32; 2],
    pub uvd_rotation: f32,
    pub uvd_by_pixel_position: bool,
    /// TD 引用的 UvSet 序号。
    pub distortion_uv_set: i32,
    pub distortion_borders: [i32; 2],
    pub distortion_filter: i32,
    pub texture_palette_index: i32,
    pub palette_offset: f32,
    pub palette_border: i32,
    pub palette_filter: i32,
    /// 原始 CulT：0 双面、1 留正面、2 留背面、3 每实例先背面后正面。
    pub cull_mode: i32,
    /// 内嵌绘制模型序号（文件 `Modl` 顺序）。
    pub model_index: usize,
    /// `DsSp` and file-level `SPFR` used by the post-depth soft pass.
    pub soft_particle: bool,
    pub soft_particle_fade_range: f32,
    /// `DOTy/DpOf`: Legacy (0) or FixedIntervalNDC (1) depth offset.
    pub depth_offset_type: i32,
    pub depth_offset: f32,
}

/// 从解析后的 avfx 构建的采样运行时（拷贝子集，构建后与文件解耦）。
#[derive(Debug, Clone)]
pub struct VfxRuntime {
    file: AvfxFile,
    /// 目标模型的特效绑点表（id → 模型空间偏移；空表 = 全部原点）。
    bind_points: Vec<crate::avfx::VfxBindPoint>,
    /// Explicit target provider; None keeps static preview admission.
    binder_targets: Option<Vec<VfxBinderTargetSnapshot>>,
    /// Independent resolved Character object source (BTPT=0).
    binder_object: Option<VfxBinderObjectSnapshot>,
    /// Ordered host target objects, independent of the caster's providers.
    binder_target_objects: Option<Vec<VfxBinderTargetObjectSnapshot>>,
    /// Fixed listener VFX scale for this preview; independent of target bases.
    vfx_scale: f32,
    /// Fixed resolved Document +0x38 getter, including host-side revision.
    document_scale: Option<[f32; 3]>,
    /// Fixed Document update result; host provider/history remain external.
    document_transform: VfxDocumentTransform,
    /// Fixed camera snapshot in the target/particle coordinate space.
    camera_position: Option<[f32; 3]>,
    binder_camera: Option<VfxBinderCameraSnapshot>,
    camera_binder: Option<VfxCameraBinderSnapshot>,
    /// Explicit replay input, shared across all Spline constructors in this runtime.
    client_binder_random: Option<(
        client_random::VfxClientRandomCell,
        VfxClientRandomState,
        VfxClientTrigMode,
    )>,
    client_random_reset: client_random::RandomResetPolicy,
    client_binder_curves: Option<binder_curves::VfxClientBinderCurveEnvironment>,
    client_color_curves: Option<VfxClientColorCurveDefaults>,
    client_particle_curves: Option<particle_curves::VfxClientParticleCurveEnvironment>,
    client_model_skin_curves: Option<model_skin_curves::VfxClientModelSkinCurveEnvironment>,
    model_skin_host: Option<model_skin_target::TargetEnvironment>,
    model_skin_creation_order: Option<model_skin_target::CreationOrderEnvironment>,
    /// Explicit preview owner's raw Euler rotation, independent of the view.
    preview_camera_host_rotation: Option<VfxBinderMatrix>,
    /// Resolved Document depth override (+0x94), separate from scale.
    document_depth_offset: f32,
    /// 采样硬上限，超出丢弃。
    max_quads: usize,
}

/// 发射器实例的基准变换（子发射器经父链继承；根发射器 = 绑点偏移）。
#[derive(Clone, Copy, Debug, PartialEq)]
struct EmitterBase {
    position: [f32; 3],
    orientation: [f32; 4],
    /// Sum of inherited Euler Z angles; Powder does not inherit parent X/Y.
    rotation_z: f32,
    scale: [f32; 3],
    linear: [[f32; 3]; 3],
    /// Nearest Point Binder or Document's separate +0x30 getter. It must not be folded
    /// into the main matrix: ordinary PICd 2/3/8 children skip this getter.
    auxiliary_matrix: VfxBinderMatrix,
    /// Ancestor Point Binder's independent cached +0x138 output.
    depth_offset_multiplier: f32,
}

impl EmitterBase {
    fn root(bind_offset: [f32; 3]) -> Self {
        Self {
            position: bind_offset,
            orientation: [0.0, 0.0, 0.0, 1.0],
            rotation_z: 0.0,
            scale: [1.0; 3],
            linear: VFX_IDENTITY_BASIS,
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            depth_offset_multiplier: 1.0,
        }
    }

    /// 发射器本地点 → 世界，完整保留父链线性变换。
    fn transform_point(&self, point: [f32; 3]) -> [f32; 3] {
        let rotated = basis_transform(self.linear, point);
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

fn timeline_root_binder_index(timeline: &AvfxTimeline, item_binder_index: i32) -> i32 {
    let item_binder_index = signed_byte_index(item_binder_index);
    if item_binder_index < 0 {
        signed_byte_index(timeline.binder_index)
    } else {
        item_binder_index
    }
}

/// Point applies the property translation before the target matrix (client
/// 0x1403c1ca3); only this constant, independent-axis subset is modeled.
fn static_point_binder_position(binder: &AvfxBinder) -> Option<[f32; 3]> {
    let properties = binder.properties_start.as_ref()?;
    let data = binder.data.as_ref()?;
    let spring = data.spring_strength.as_ref()?;
    let position = &properties.position;
    // The client stores bTSc as a byte and Point tests only zero vs nonzero.
    if binder.binder_type != 0
        || binder.life >= 0
        || binder.rotation_type != 0
        || binder.transform_scale as u8 == 0
        || binder.start_to_global_direction
        || binder.vfx_scale_enabled
        || binder.vfx_scale_depth_offset
        || binder.vfx_scale_interpolation
        || binder.transform_scale_depth_offset
        || binder.transform_scale_interpolation
        || binder.following_target_orientation
        || binder.document_scale_enabled
        || binder.adjust_to_screen_enabled
        || binder.ify
        || binder.bet
        || binder.properties_1.is_some()
        || binder.properties_2.is_some()
        || binder.properties_goal.is_some()
        || properties.bind_point_type != 0
        || properties.bind_target_point_type != 3
        || !matches!(properties.binder_name.as_str(), "" | "null")
        || properties.bind_point_id != binder.bind_point_id
        || properties.generate_delay != 0
        || properties.coord_update_frame != -1
        || properties.ring_enabled
        || position.axis_connect != 0
        || position.axis_connect_random != 0
        || position.random_x.is_some()
        || position.random_y.is_some()
        || position.random_z.is_some()
        || spring.pre_behavior != 0
        || spring.post_behavior != 0
        || spring.random_type != 0
        || spring.keys.len() != 1
        || spring.keys[0].time != 0
        || spring.keys[0].z != 1.0
        || data.spring_strength_random.is_some()
        || data.carry_over_factor.is_some()
        || data.carry_over_factor_random.is_some()
        || data.distance.is_some()
        || data.distance_random.is_some()
        || !data.source.scalars.is_empty()
        || !data.source.curves.keys().all(|name| name == "SpS")
        || !data.source.curve3s.is_empty()
        || !data.source.color_curves.is_empty()
    {
        return None;
    }
    let axis = |curve: Option<&AvfxCurve>| match curve {
        None => Some(0.0),
        Some(curve)
            if curve.pre_behavior == 0
                && curve.post_behavior == 0
                && curve.random_type == 0
                && curve.keys.len() == 1
                && curve.keys[0].time == 0
                && curve.keys[0].z.is_finite() =>
        {
            Some(curve.keys[0].z)
        }
        _ => None,
    };
    Some([
        axis(position.x.as_ref())?,
        axis(position.y.as_ref())?,
        axis(position.z.as_ref())?,
    ])
}

/// Local Point inputs for the explicit matrix source. bStG belongs to Linear's
/// endpoint direction and is inert in the verified Point functions. Random
/// curves, external identities and full child factories remain separate.
fn local_point_binder_curves(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
) -> Option<VfxPointBinderCurves> {
    let properties = binder.properties_start.as_ref()?;
    let data = binder.data.as_ref()?;
    let spring = data.spring_strength.as_ref()?;
    let plain_curve = |curve: &AvfxCurve| {
        curve.random_type == 0
            && curve.keys.iter().all(|key| {
                key.interpolation <= 2 && [key.x, key.y, key.z].into_iter().all(f32::is_finite)
            })
    };
    let position = &properties.position;
    if binder.binder_type != 0
        || !matches!(binder.rotation_type as i8, 0..=3)
        || binder.document_scale_enabled
        || binder.adjust_to_screen_enabled
        || binder.ify
        || binder.properties_1.is_some()
        || binder.properties_2.is_some()
        || binder.properties_goal.is_some()
        || !matches!(properties.bind_point_type as u8 & 3, 0 | 1)
        || !matches!(properties.bind_target_point_type, 0 | 3)
        || (properties.bind_target_point_type == 3
            && (!matches!(properties.binder_name.as_str(), "" | "null")
                || properties.bind_point_id < 0
                || properties.bind_point_id != binder.bind_point_id
                || properties.bct != 0))
        || properties.ring_enabled
        || position.axis_connect != 0
        || position.axis_connect_random != 0
        || position.random_x.is_some()
        || position.random_y.is_some()
        || position.random_z.is_some()
        || !plain_curve(spring)
        || [&position.x, &position.y, &position.z]
            .into_iter()
            .flatten()
            .any(|curve| !plain_curve(curve))
        || data.spring_strength_random.is_some()
        || data.carry_over_factor.is_some()
        || data.carry_over_factor_random.is_some()
        || data.distance.is_some()
        || data.distance_random.is_some()
        || !data.source.scalars.is_empty()
        || !data.source.curves.keys().all(|name| name == "SpS")
        || !data.source.curve3s.is_empty()
        || !data.source.color_curves.is_empty()
    {
        return None;
    }
    Some(VfxPointBinderCurves {
        spring: spring.value_at(age, total_age, 1.0),
        position: position.evaluate_at(age, total_age, 0.0),
    })
}

/// Admission for the existing static staged path, independent of explicit sources.
fn static_linear_binder_factor(binder: &AvfxBinder) -> Option<f32> {
    if binder.life >= 0
        || binder
            .properties_start
            .as_ref()
            .is_some_and(|point| point.generate_delay != 0)
        || binder.transform_scale as u8 == 0
        || binder.start_to_global_direction
        || binder.vfx_scale_enabled
        || binder.vfx_scale_depth_offset
        || binder.vfx_scale_interpolation
        || binder.transform_scale_depth_offset
        || binder.transform_scale_interpolation
        || binder.following_target_orientation
        || binder.bet
    {
        return None;
    }
    local_linear_binder_factor(binder)
}

/// Deterministic local Linear curve inputs evaluated on the Binder's own clocks.
/// Random readers remain excluded: the native start Pos reader runs twice.
fn local_linear_binder_curves(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
) -> Option<VfxLinearBinderCurves> {
    matches!(binder.binder_type, 1 | 4)
        .then(|| local_dual_binder_curves(binder, age, total_age))
        .flatten()
}

fn local_spline_binder_curves(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
) -> Option<VfxLinearBinderCurves> {
    local_spline_binder_curves_mode(binder, age, total_age, false)
}

fn local_spline_binder_curves_mode(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
    client_random: bool,
) -> Option<VfxLinearBinderCurves> {
    if binder.binder_type != 2
        || [binder.properties_1.as_ref(), binder.properties_2.as_ref()]
            .into_iter()
            .flatten()
            .any(|p| {
                !p.ring_position
                    .into_iter()
                    .chain([p.ring_radius])
                    .all(f32::is_finite)
            })
    {
        return None;
    }
    local_dual_binder_curves_mode(binder, age, total_age, client_random)
}

fn local_dual_binder_curves(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
) -> Option<VfxLinearBinderCurves> {
    local_dual_binder_curves_mode(binder, age, total_age, false)
}

fn local_dual_binder_curves_mode(
    binder: &AvfxBinder,
    age: f32,
    total_age: f32,
    client_random: bool,
) -> Option<VfxLinearBinderCurves> {
    let start = binder.properties_start.as_ref()?;
    let goal = binder.properties_goal.as_ref()?;
    let data = binder.data.as_ref()?;
    let factor = data.carry_over_factor.as_ref();
    if factor.is_none() && !client_random {
        return None;
    }
    let plain_curve = |curve: &AvfxCurve| {
        (client_random || curve.random_type == 0)
            && curve.pre_behavior <= 3
            && curve.post_behavior <= 3
            && (client_random || !curve.keys.is_empty())
            && curve.keys.iter().all(|key| {
                key.interpolation <= 2 && [key.x, key.y, key.z].into_iter().all(f32::is_finite)
            })
    };
    let supported_point = |point: &crate::avfx::AvfxBinderProperties| {
        matches!(point.bind_point_type as u8 & 3, 0 | 1)
            && matches!(point.bind_target_point_type, 0 | 3)
            && (point.bind_target_point_type == 0
                || (matches!(point.binder_name.as_str(), "" | "null")
                    && point.bind_point_id >= 0
                    && point.bct == 0))
            && !point.ring_enabled
            && (if client_random {
                point.position.axis_connect & 15 <= 9
                    && point.position.axis_connect_random & 15 <= 9
            } else {
                point.position.axis_connect == 0 && point.position.axis_connect_random == 0
            })
            && (client_random
                || (point.position.random_x.is_none()
                    && point.position.random_y.is_none()
                    && point.position.random_z.is_none()))
            && [
                &point.position.x,
                &point.position.y,
                &point.position.z,
                &point.position.random_x,
                &point.position.random_y,
                &point.position.random_z,
            ]
            .into_iter()
            .flatten()
            .all(plain_curve)
    };
    if !matches!(binder.binder_type, 1 | 2 | 4)
        || !matches!(binder.rotation_type as i8, 0..=3)
        || binder.document_scale_enabled
        || binder.adjust_to_screen_enabled
        || binder.ify
        || (binder.binder_type != 2
            && (binder.properties_1.is_some() || binder.properties_2.is_some()))
        || (start.bind_target_point_type == 3 && binder.bind_point_id != start.bind_point_id)
        || !supported_point(start)
        || !supported_point(goal)
        || factor.is_some_and(|factor| !plain_curve(factor))
        || data.spring_strength.is_some()
        || data.spring_strength_random.is_some()
        || data
            .carry_over_factor_random
            .as_ref()
            .is_some_and(|curve| !client_random || !plain_curve(curve))
        || data.distance.is_some()
        || data.distance_random.is_some()
        || !data.source.scalars.is_empty()
        || !data
            .source
            .curves
            .keys()
            .all(|name| name == "COF" || (client_random && name == "COFR"))
        || !data.source.curve3s.is_empty()
        || !data.source.color_curves.is_empty()
    {
        return None;
    }
    let start_position = start.position.evaluate_at(age, total_age, 0.0);
    Some(VfxLinearBinderCurves {
        factor: factor.map_or(0.0, |factor| factor.value_at(age, total_age, 0.0)),
        start_position,
        goal_position: goal.position.evaluate_at(age, total_age, 0.0),
        // Deterministic readers return the same result on the second call.
        start_position_after: start.position.evaluate_at(age, total_age, 0.0),
    })
}

/// Fixed preview admission stays constant; dynamic curves require persistent
/// playback and an explicitly supplied target snapshot.
fn local_linear_binder_factor(binder: &AvfxBinder) -> Option<f32> {
    if [
        binder.properties_start.as_ref()?,
        binder.properties_goal.as_ref()?,
    ]
    .iter()
    .any(|point| point.bind_target_point_type != 3 || point.bind_point_type as u8 & 3 != 0)
    {
        return None;
    }
    let factor = binder.data.as_ref()?.carry_over_factor.as_ref()?;
    if factor.pre_behavior != 0
        || factor.post_behavior != 0
        || factor.keys.len() != 1
        || factor.keys[0].time != 0
        || [
            binder.properties_start.as_ref()?,
            binder.properties_goal.as_ref()?,
        ]
        .into_iter()
        .any(|point| point.coord_update_frame != -1 || point.position != Default::default())
    {
        return None;
    }
    local_linear_binder_curves(binder, 0.0, 0.0).map(|curves| curves.factor)
}

/// Preserve local components and the complete parent matrix without decomposing
/// shear or zero scales. Optional inheritance uses separate scalar/Euler getters.
#[derive(Clone, Copy, Debug)]
struct EmitterTransform {
    base: EmitterBase,
    position: [f32; 3],
    orientation: [f32; 4],
    rotation_z: f32,
    scale: [f32; 3],
    coord_compute_order: i32,
}

impl EmitterTransform {
    fn world(self) -> EmitterBase {
        EmitterBase {
            position: self.position,
            orientation: self.base.rotate_quat(self.orientation),
            rotation_z: self.base.rotation_z + self.rotation_z,
            scale: std::array::from_fn(|axis| self.base.scale[axis] * self.scale[axis]),
            linear: basis_mul(
                self.base.linear,
                coordinate_basis(self.coord_compute_order, self.orientation, self.scale),
            ),
            depth_offset_multiplier: self.base.depth_offset_multiplier,
            auxiliary_matrix: self.base.auxiliary_matrix,
        }
    }
}

#[derive(Clone)]
struct EmitterAnimation<'a> {
    emitter: &'a AvfxEmitter,
    clock: InstanceClock,
    base: EmitterBase,
    binder_base: EmitterBase,
    direction: [f32; 3],
    parent: Option<EmitterParent<'a>>,
    /// Staged snapshots resolve the same parent inputs without borrowing a
    /// temporary ancestor animation. They still use the child transform path.
    sampled_parent: Option<EmitterParentFrame>,
    parent_components: ParentComponents<'a>,
    creation_angle: [f32; 3],
    random: SplitMix64,
}

#[derive(Clone, Copy)]
struct EmitterParent<'a> {
    animation: &'a EmitterAnimation<'a>,
    birth: f32,
    transform: EmitterBase,
    motion: motion::InjectionMotion,
    binding: ShapeBinding,
}

#[derive(Clone, Copy)]
struct EmitterParentFrame {
    birth: EmitterBase,
    now: EmitterBase,
    motion: motion::InjectionMotion,
    bound: Option<[f32; 3]>,
}

impl EmitterAnimation<'_> {
    fn model_injection_direction(&self, frame: f32, rng: &mut SplitMix64) -> [f32; 3] {
        let ages = self.clock.ages(frame);
        let mut first_rng = SplitMix64::seeded(self.random.clone().next_u64(), 0, 0, 0x4941_4E47);
        let first: [u16; 3] = std::array::from_fn(|_| first_rng.next_u64() as u16);
        let mut angles = [0.0; 3];
        for axis in (0..3).rev() {
            angles[axis] = cone::curve_value_at(
                &self.emitter.injection_angle[axis],
                &self.emitter.injection_angle_random[axis],
                ages,
                first[axis],
                rng,
            );
        }
        // 0x1403cf650 uses fixed XYZ and +Z, independent of Emit.ROT.
        quat_rotate(quat_from_euler(0, angles), [0.0, 0.0, 1.0])
    }

    fn model_shape_rotation(&self, age: f32, rng: &mut SplitMix64) -> [f32; 4] {
        let ages = self.clock.ages(age);
        let Some(AvfxEmitterData::Model(data)) = &self.emitter.data else {
            return [0.0, 0.0, 0.0, 1.0];
        };
        // Shape angles share emitter-instance random state at birth and while bound.
        let seed = self.random.clone().next_u64() ^ 0x5348_4150;
        let mut angles = [0.0; 3];
        for axis in (0..3).rev() {
            angles[axis] = if matches!(data.rotation.angles_random[axis].random_type & 7, 3..=5) {
                cone::curve_value_at(
                    &data.rotation.angles[axis],
                    &data.rotation.angles_random[axis],
                    ages,
                    0,
                    rng,
                )
            } else {
                curve_value_seeded_at(
                    &data.rotation.angles[axis],
                    &data.rotation.angles_random[axis],
                    ages,
                    0.0,
                    seed ^ axis as u64,
                )
            };
        }
        quat_from_euler(i32::from(data.rotation.order as u8), angles)
    }

    fn at(&self, age: f32) -> EmitterTransform {
        self.at_with_gravity(age, None)
    }

    fn at_with_gravity(&self, age: f32, gravity_offset: Option<f32>) -> EmitterTransform {
        self.at_with_gravity_at(age, self.clock.ages(age), gravity_offset)
    }

    fn at_with_gravity_at(
        &self,
        age: f32,
        ages: CurveAges,
        gravity_offset: Option<f32>,
    ) -> EmitterTransform {
        let frame = age;
        let range = self.clock.motion_range(age);
        let mut rng = self.random.clone();
        let mut pos = eval3_seeded_at(&self.emitter.position, ages, 0.0, rng.next_u64());
        let curve_rot = eval3_seeded_at(&self.emitter.rotation, ages, 0.0, rng.next_u64());
        let mut rot = std::array::from_fn(|axis| curve_rot[axis] + self.creation_angle[axis]);
        let mut scale = eval3_seeded_at(&self.emitter.scale, ages, 1.0, rng.next_u64());
        // Construction prewarm freezes the external parent. A finished emitter
        // retains its last transform while its descendants retire.
        let parent_age = self
            .clock
            .finish_frame()
            .map_or(frame, |end| frame.min(end))
            .max(0.0);
        if self
            .parent
            .is_some_and(|parent| parent.motion.has_optional_components())
            || self
                .sampled_parent
                .is_some_and(|parent| parent.motion.has_optional_components())
        {
            let inherited = self.parent_components.at(parent_age);
            for axis in 0..3 {
                rot[axis] += inherited.rotation[axis];
                scale[axis] *= inherited.scale[axis];
            }
        }
        let orientation = quat_from_euler(self.emitter.rotation_order, rot);
        let parent_frame = self.sampled_parent.or_else(|| {
            self.parent.map(|parent| {
                let now_frame = parent.birth + parent_age;
                let now = parent.animation.at(now_frame).world();
                EmitterParentFrame {
                    birth: parent.transform,
                    now,
                    motion: parent.motion,
                    bound: parent.binding.position(parent.animation, now_frame, now),
                }
            })
        });
        let (base, mut position) = if let Some(parent) = parent_frame {
            let displacement = parent.motion.emitter_displacement(
                self.emitter,
                if self.sampled_parent.is_some() {
                    self.clock.frame_for_curve_total(ages.total)
                } else {
                    frame
                },
                self.clock,
                self.random.clone().next_u64() ^ 0x454D_4F54,
            );
            for axis in 0..3 {
                pos[axis] += displacement[axis];
            }
            let position = parent.motion.translate(
                coordinate_translation(self.emitter.coord_compute_order, pos, orientation, scale),
                parent.birth,
                parent.now,
                parent.bound,
                self.binder_base,
            );
            let mut base = parent.motion.parent(parent.birth, parent.now);
            base.auxiliary_matrix = self.binder_base.auxiliary_matrix;
            base.linear = parent
                .motion
                .drawing_parent_with_auxiliary(base.linear, base.auxiliary_matrix.basis);
            (base, position)
        } else {
            let mut base = self.base;
            base.linear = basis_mul(base.linear, base.auxiliary_matrix.basis);
            (
                base,
                self.base
                    .transform_point(self.base.auxiliary_matrix.transform_point(
                        coordinate_translation(
                            self.emitter.coord_compute_order,
                            pos,
                            orientation,
                            scale,
                        ),
                    )),
            )
        };
        position[1] += gravity_offset.unwrap_or_else(|| {
            integration::looped_curve_range(
                &self.emitter.gravity,
                &self.emitter.gravity_random,
                range,
                f64::from(self.clock.loop_start),
                f64::from(self.clock.loop_end),
                rng.next_u64(),
            )
            .displacement as f32
        });
        EmitterTransform {
            base,
            position,
            orientation,
            rotation_z: rot[2],
            scale,
            coord_compute_order: self.emitter.coord_compute_order,
        }
    }
}

#[derive(Clone, Copy)]
enum ShapeBinding {
    Center,
    ModelVertex { vertex: [f32; 3], random_seed: u64 },
    ConeModel { index: i16, random_seed: u64 },
    CylinderModel { index: i16, random_seed: u64 },
    SphereModel { index: i16, random_seed: u64 },
}

impl ShapeBinding {
    fn position(
        self,
        animation: &EmitterAnimation,
        frame: f32,
        world: EmitterBase,
    ) -> Option<[f32; 3]> {
        match self {
            Self::Center => Some(world.position),
            Self::ModelVertex {
                vertex,
                random_seed,
            } => {
                // Bound Always curves still lack client callback history.
                let mut rng = SplitMix64::seeded(random_seed, frame.floor() as u64, 0, 0x4249_4E44);
                Some(world.transform_point(quat_rotate(
                    animation.model_shape_rotation(frame, &mut rng),
                    vertex,
                )))
            }
            Self::ConeModel { index, random_seed } => {
                cone_model::position(animation, frame, world, index, random_seed)
            }
            Self::CylinderModel { index, random_seed } => {
                cylinder_model::position(animation, frame, world, index, random_seed)
            }
            Self::SphereModel { index, random_seed } => {
                sphere_model::position(animation, frame, world, index, random_seed)
            }
        }
    }
}

struct SpawnInjection {
    position: [f32; 3],
    direction: motion::InjectionDirection,
    velocity: [f32; 3],
    binding: ShapeBinding,
}

/// Color has its own parent clock: a child's curve loop does not rewind its
/// parent, and Initial snapshots survive the child's local curve loops.
struct EmitterColorAnimation<'a> {
    emitter: &'a AvfxEmitter,
    emitter_index: usize,
    instance_seed: u64,
    clock: InstanceClock,
    parent: ParentColor<'a>,
}

impl EmitterColorAnimation<'_> {
    fn at(&self, frame: f32) -> [f32; 4] {
        self.at_ages(frame, self.clock.ages(frame))
    }

    fn at_ages(&self, frame: f32, ages: CurveAges) -> [f32; 4] {
        let seed = SplitMix64::seeded(
            self.emitter_index as u64 ^ 0xE417,
            0,
            self.instance_seed,
            0xC010,
        )
        .next_u64();
        let color = color_curve_seeded_at(&self.emitter.color, ages, seed);
        self.parent.apply(color, frame)
    }
}

#[derive(Clone, Copy)]
enum ParentColor<'a> {
    None,
    Initial([f32; 4]),
    Always {
        animation: &'a EmitterColorAnimation<'a>,
        birth: f32,
    },
}

impl<'a> ParentColor<'a> {
    fn new(mode: i32, animation: &'a EmitterColorAnimation<'a>, birth: f32) -> Self {
        match mode {
            1 => Self::Initial(animation.at(birth)),
            // The client constructor maps both 2 and 8 to the live-parent branch.
            2 | 8 => Self::Always { animation, birth },
            _ => Self::None,
        }
    }

    fn apply(self, color: [f32; 4], age: f32) -> [f32; 4] {
        let parent = match self {
            Self::None => return color,
            Self::Initial(color) => color,
            Self::Always { animation, birth } => animation.at(birth + age.max(0.0)),
        };
        std::array::from_fn(|axis| parent[axis] * color[axis])
    }
}

/// A complete external root cache, possibly shared by consecutive inputs.
#[derive(Clone, Copy, Debug)]
struct RootTransformSample {
    /// Last input end frame covered by this cache value.
    frame: f32,
    /// First input generation covered by this value; the next sample ends it.
    input_update: usize,
    base: EmitterBase,
}

#[derive(Clone, Copy)]
struct RootTransformHistory<'a> {
    samples: &'a [RootTransformSample],
    sample_frame: f32,
    current_base: EmitterBase,
    runtime: &'a VfxRuntime,
    path: &'a [trace::EmitterSnapshot],
    direction: [f32; 3],
    seed: u64,
}

impl RootTransformHistory<'_> {
    fn sample_at(self, frame: f32) -> Option<RootTransformSample> {
        self.samples
            .iter()
            .find(|sample| sample.frame + 1e-4 >= frame)
            .or_else(|| self.samples.last())
            .copied()
    }
}

/// 一次粒子出生的全部上下文。
#[derive(Clone, Copy)]
struct SpawnContext<'a> {
    /// 粒子年龄（帧，全局=item 起算）。
    age: f32,
    /// Instance age before local-loop writeback.
    total_age: f32,
    clock: InstanceClock,
    /// Birth point, velocity and direction basis in the PICd motion space.
    motion: motion::InjectionMotion,
    /// Stored world-Y motion for staged playback; None uses continuous integration.
    gravity_offset: Option<f32>,
    shape_binding: ShapeBinding,
    /// 出生和当前时刻的发射器变换，供 Initial/Always 选择。
    emitter: EmitterTransform,
    emitter_now: EmitterTransform,
    /// Ancestor Binder linear transform used by PICd=1/ICbB.
    binder_base: EmitterBase,
    /// Re-evaluate emitter inheritance at a Powder child's birth with the same seed.
    emitter_animation: &'a EmitterAnimation<'a>,
    root_transform_history: Option<RootTransformHistory<'a>>,
    spawn_loop_age: f32,
    parent_color: ParentColor<'a>,
    /// Staged +d0..dc cache, including nonempty prewarm inheritance.
    /// Continuous sampling uses its independent curve evaluator.
    cached_color: Option<[f32; 4]>,
    /// Scale, Euler rotation and position latched by the last property callback.
    /// Drawing and current-input history updates read these without sampling.
    cached_xyz: Option<[[f32; 3]; 3]>,
    cached_textures: Option<particle_curves::VfxClientParticleTextureValues>,
    client_injection: Option<[f32; 3]>,
    /// AvfxMain Revised RGB, applied once after parent-color inheritance.
    revised_color: [f32; 3],
    parent_components: ParentComponents<'a>,
    creation_angle: [f32; 3],
    /// 实例随机种子（随机曲线/颜色随机的确定性播种）。
    seed: u64,
    /// 出生事件与副本的位编码，用于 Powder 子粒子的随机播种。
    create_index: u64,
}

impl SpawnContext<'_> {
    fn curve_ages(&self) -> CurveAges {
        CurveAges {
            local: self.clock.age(self.age),
            total: self.total_age,
        }
    }

    fn bound_position(&self) -> Option<[f32; 3]> {
        self.shape_binding.position(
            self.emitter_animation,
            self.spawn_loop_age + self.age.max(0.0),
            self.emitter_now.world(),
        )
    }

    fn color(&self, particle: &AvfxParticle) -> [f32; 4] {
        let local = self.cached_color.unwrap_or_else(|| {
            color_curve_seeded_at(&particle.color, self.curve_ages(), self.seed)
        });
        let mut color = self.parent_color.apply(local, self.age);
        color[0] *= self.revised_color[0];
        color[1] *= self.revised_color[1];
        color[2] *= self.revised_color[2];
        color
    }

    fn at_age(&self, age: f32) -> Self {
        let frame = self.spawn_loop_age + age.max(0.0);
        let mut now = self.emitter_animation.at(frame);
        let mut binder_base = self.binder_base;
        if let Some(history) = self.root_transform_history {
            let sample_frame = history.sample_frame - (self.age - age);
            if let Some(sample) = history.sample_at(sample_frame) {
                // Only the external cache generation changes here. Instance
                // clocks and immutable ancestor birth paths retain their own
                // ages; prewarm freezes the external source within one input.
                let mut path = history.path.to_vec();
                for snapshot in &mut path {
                    snapshot.input_update = sample.input_update;
                }
                if let Some(animation) = playback::staged_emitter_animation(
                    history.runtime,
                    history.current_base,
                    history.samples,
                    history.direction,
                    history.seed,
                    &path,
                ) {
                    now = animation.at(frame);
                    binder_base = animation.binder_base;
                }
            }
        }
        Self {
            // This helper synthesizes a different historical birth, not an
            // original property callback. Never reuse a current XYZ cache there.
            cached_xyz: if age.to_bits() == self.age.to_bits() {
                self.cached_xyz
            } else {
                None
            },
            cached_textures: if age.to_bits() == self.age.to_bits() {
                self.cached_textures
            } else {
                None
            },
            client_injection: if age.to_bits() == self.age.to_bits() {
                self.client_injection
            } else {
                None
            },
            age,
            total_age: self.clock.ages(age).total,
            emitter_now: now,
            binder_base,
            ..*self
        }
    }
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
            binder_targets: None,
            binder_object: None,
            binder_target_objects: None,
            vfx_scale: 1.0,
            document_scale: None,
            document_transform: VfxDocumentTransform::new(
                file.global.revised_rotation,
                file.global.revised_scale,
                file.global.revised_position,
                VfxBinderMatrix::IDENTITY,
                None,
            ),
            camera_position: None,
            binder_camera: None,
            camera_binder: None,
            client_binder_random: None,
            client_random_reset: client_random::RandomResetPolicy::Owned,
            client_binder_curves: None,
            client_color_curves: None,
            client_particle_curves: None,
            client_model_skin_curves: None,
            model_skin_host: None,
            model_skin_creation_order: None,
            preview_camera_host_rotation: None,
            document_depth_offset: 0.0,
            max_quads: 4096,
        }
    }

    /// Supply the client TLS random snapshot and CRT dispatch for Binder replay.
    /// Reset/seek restores this snapshot; Document REST keeps consuming it.
    /// Polyline/model/Trigger draws share this input. Other consumers still
    /// require complete client stream integration.
    pub fn with_client_binder_random_state(
        mut self,
        state: VfxClientRandomState,
        mode: VfxClientTrigMode,
    ) -> Self {
        self.client_binder_random =
            Some((client_random::VfxClientRandomCell::new(state), state, mode));
        self.client_random_reset = client_random::RandomResetPolicy::Owned;
        self
    }

    /// Attach to an explicitly ordered multi-document preview stream. The
    /// host coordinates replay reset; new playback and individual reset keep
    /// consuming the stream, while per-resource curve caches still reset.
    /// Runtime Clone isolates its cell and restores normal owned reset behavior.
    pub fn with_shared_client_random_stream(
        mut self,
        stream: &VfxSharedRandomStream,
        mode: VfxClientTrigMode,
    ) -> Self {
        self.client_binder_random = Some((stream.cell.share(), stream.snapshot(), mode));
        self.client_random_reset = client_random::RandomResetPolicy::HostManaged;
        self
    }

    pub fn client_binder_random_state(&self) -> Option<VfxClientRandomState> {
        self.client_binder_random
            .as_ref()
            .map(|(state, _, _)| state.get())
    }

    /// Retain Col First bytes and evaluate active Always channels on each
    /// staged property callback using the explicit shared stream. Requires
    /// with_client_binder_random_state. Other constructor/curve consumers
    /// still need complete replay; ModelSkin curves have a separate explicit
    /// environment below. This is not full TLS/provider replay.
    pub fn with_client_color_curve_defaults(
        mut self,
        defaults: VfxClientColorCurveDefaults,
    ) -> Self {
        self.client_color_curves = Some(defaults);
        self
    }

    /// Include common particle TC1/TP and conditional Gra/ARs/VR First states,
    /// then Col and Scl/Rot/Pos First/property reads in the shared stream.
    /// Current drawing reads the retained XYZ property cache; staged particle
    /// gravity and VR/ARs use saved First and shared definition dispatch.
    /// Production factories resolve birth velocity/basis before prewarm;
    /// injection displacement retains each input step. Requires the explicit
    /// Color environment. Full shape/Common factories, large-angle CRT,
    /// texture and emitter constructors still need replay.
    pub fn with_client_particle_constructor(mut self) -> Self {
        self.client_particle_curves =
            Some(particle_curves::VfxClientParticleCurveEnvironment::default());
        self
    }

    /// Enable ModelSkin-owned First and property/numeric curve caches. Requires
    /// the explicit Color environment and shared stream. Numeric replay assumes
    /// the admitted model receiver succeeds unless an explicit target snapshot
    /// gates it. Full target/provider replay remains.
    pub fn with_client_model_skin_curve_defaults(
        mut self,
        defaults: VfxClientModelSkinCurveDefaults,
    ) -> Self {
        self.client_model_skin_curves =
            Some(model_skin_curves::VfxClientModelSkinCurveEnvironment::new(
                defaults,
                self.file.particles.len(),
            ));
        self
    }

    /// Compare ModelSkin resource creation across documents of the same host group.
    /// Reset the coordinator once before resetting all documents. Independent
    /// Runtime clones preserve their own constructor prefix and isolate the counter.
    pub fn with_shared_model_skin_creation_order(
        mut self,
        order: &VfxModelSkinCreationOrder,
    ) -> Self {
        self.model_skin_creation_order =
            Some(model_skin_target::CreationOrderEnvironment::new(order));
        self
    }

    /// Gate retained ModelSkin Numeric using explicit host targets. Failed
    /// queries preserve the last complete Aura packet; generation changes
    /// retire the original object's callbacks. Reset accepts the current host.
    /// Omit this input to retain legacy static admission behavior.
    pub fn with_model_skin_target_snapshot(mut self, snapshot: VfxModelSkinTargetSnapshot) -> Self {
        self.model_skin_host = Some(model_skin_target::TargetEnvironment::new(snapshot));
        self
    }

    fn color_curve_environment(
        &self,
    ) -> Result<Option<color_curves::VfxClientColorCurveEnvironment>, String> {
        if (self.client_model_skin_curves.is_some() || self.client_particle_curves.is_some())
            && self.client_color_curves.is_none()
        {
            return Err("ModelSkin curve replay requires an explicit Color environment".into());
        }
        if self.client_model_skin_curves.is_some() {
            for particle in &self.file.particles {
                if let AvfxParticleData::ModelSkin(data) = &particle.data {
                    for axes in [&data.fresnel_rotation, &data.uv_point_density] {
                        if axes.axis_connect & 15 > 9 || axes.axis_connect_random & 15 > 9 {
                            return Err(
                                "ModelSkin curve replay requires valid XYZ connection codes".into(),
                            );
                        }
                    }
                }
            }
        }
        self.client_color_curves
            .map(|defaults| {
                self.client_binder_random
                    .as_ref()
                    .map(|(random, _, mode)| {
                        color_curves::VfxClientColorCurveEnvironment::new(defaults, random)
                            .with_model_skin(self.client_model_skin_curves.as_ref())
                            .with_particle_constructor(self.client_particle_curves.as_ref())
                            .with_particle_math(*mode)
                    })
                    .ok_or_else(|| {
                        "client Color replay requires an explicit shared random snapshot".into()
                    })
            })
            .transpose()
    }

    /// Enable owner-local First bytes and shared COF reader caching for Spline
    /// replay. Requires an explicit client random snapshot and resolved compiled
    /// defaults; no live global empty descriptor/vector values are inferred.
    /// Starts with fresh lazy COF descriptors. Reset/seek restores that cache;
    /// Document REST preserves it. Current target snapshots never consume RNG.
    pub fn with_client_binder_curve_defaults(
        mut self,
        defaults: VfxClientBinderCurveDefaults,
    ) -> Self {
        self.client_binder_curves = Some(binder_curves::VfxClientBinderCurveEnvironment::new(
            defaults,
            self.file.binders.len(),
        ));
        self
    }

    fn spline_binder_curves(
        &self,
        binder: &AvfxBinder,
        age: f32,
        total: f32,
    ) -> Option<VfxLinearBinderCurves> {
        local_spline_binder_curves_mode(binder, age, total, self.client_binder_curves.is_some())
    }

    fn evaluate_spline_binder_curves(
        &self,
        index: usize,
        state: Option<VfxClientDualBinderCurveState>,
        age: f32,
        total: f32,
    ) -> VfxLinearBinderCurves {
        let binder = &self.file.binders[index];
        match state {
            Some(state) => self
                .client_binder_random
                .as_ref()
                .expect("validated client Binder RNG")
                .0
                .with_mut(|random| {
                    self.client_binder_curves
                        .as_ref()
                        .expect("validated compiled Binder curve environment")
                        .evaluate(index, state, binder, [age, total], random)
                }),
            None => local_spline_binder_curves(binder, age, total)
                .expect("validated deterministic Spline curves"),
        }
    }

    fn reset_client_binder_random_state(&mut self) {
        if let Some(order) = &mut self.model_skin_creation_order {
            order.reset();
        }
        if let Some((state, initial, _)) = &mut self.client_binder_random {
            if self.client_random_reset == client_random::RandomResetPolicy::Owned {
                state.set(*initial);
            } else {
                // Remember this document's actual reconstruction boundary for
                // an independent Clone, without rewinding its peers.
                *initial = state.get();
            }
        }
        if let Some(curves) = &self.client_binder_curves {
            curves.reset();
        }
        if let Some(curves) = &self.client_model_skin_curves {
            curves.reset();
        }
        if let Some(curves) = &self.client_particle_curves {
            curves.reset();
        }
    }

    fn next_client_binder_random_word(&self) -> u16 {
        let (state, _, _) = self
            .client_binder_random
            .as_ref()
            .expect("validated client Binder random snapshot");
        state.next_u16()
    }

    /// Supply an initial resolved ElementId provider. Missing IDs are query
    /// failures, with no static-point fallback. Playback accepts later snapshots.
    pub fn with_binder_target_snapshot(
        mut self,
        targets: &[VfxBinderTargetSnapshot],
    ) -> Result<Self, String> {
        Self::validate_binder_target_snapshot(targets)?;
        self.binder_targets = Some(targets.to_vec());
        Ok(self)
    }

    /// Convert bone world * ElementId local matrices into an explicit target
    /// provider. Preserve all affine columns, including mirrored/sheared axes.
    pub fn bind_point_pose_targets(
        points: &[crate::avfx::VfxBindPoint],
        skeleton: &crate::skeleton::ModelSkeleton,
        pose: &crate::skeleton::SkeletonPose,
    ) -> Result<Vec<VfxBinderTargetSnapshot>, String> {
        let targets = points
            .iter()
            .zip(crate::avfx::vfx_bind_point_matrices(points, skeleton, pose))
            .map(|(point, matrix)| VfxBinderTargetSnapshot {
                id: point.id,
                matrix: VfxBinderMatrix {
                    basis: std::array::from_fn(|column| {
                        std::array::from_fn(|row| matrix[column * 4 + row])
                    }),
                    position: [matrix[12], matrix[13], matrix[14]],
                },
            })
            .collect::<Vec<_>>();
        Self::validate_binder_target_snapshot(&targets)?;
        Ok(targets)
    }

    fn validate_binder_target_snapshot(targets: &[VfxBinderTargetSnapshot]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for target in targets {
            if !ids.insert(target.id) {
                return Err("duplicate Binder target ID".into());
            }
            if !target
                .matrix
                .basis
                .into_iter()
                .flatten()
                .chain(target.matrix.position)
                .all(f32::is_finite)
            {
                return Err("Binder target matrix must be finite".into());
            }
        }
        Ok(())
    }

    /// Supply BTPT=0 object fields independently of the ElementId provider.
    /// This does not infer an object from ID=0, a skeleton root or Document.
    pub fn with_binder_object_snapshot(
        mut self,
        object: VfxBinderObjectSnapshot,
    ) -> Result<Self, String> {
        object.validate()?;
        self.binder_object = Some(object);
        Ok(self)
    }

    fn has_resolved_binder_sources(&self) -> bool {
        self.binder_targets.is_some()
            || self.binder_object.is_some()
            || self.binder_target_objects.is_some()
            || self.camera_binder.is_some()
    }

    /// Install the ordered target list used by Point/Linear Scheduler and Item
    /// factories. Empty lists use the caster only in the Point Scheduler branch;
    /// Linear Scheduler and target-list Items consume without creating an object.
    pub fn with_binder_target_objects(
        mut self,
        objects: &[VfxBinderTargetObjectSnapshot],
    ) -> Result<Self, String> {
        Self::validate_binder_target_objects(objects)?;
        self.binder_target_objects = Some(objects.to_vec());
        Ok(self)
    }

    fn validate_binder_target_objects(
        objects: &[VfxBinderTargetObjectSnapshot],
    ) -> Result<(), String> {
        if objects.len() > 256 {
            return Err("Binder target object limit".into());
        }
        for source in objects {
            Self::validate_binder_target_snapshot(&source.targets)?;
            if let Some(object) = source.object {
                object.validate()?;
            }
            if !source.vfx_scale.is_finite() {
                return Err("Binder target listener scale must be finite".into());
            }
        }
        Ok(())
    }

    fn supports_binder_target_sources(&self, binder: &AvfxBinder) -> bool {
        [
            binder.properties_start.as_ref(),
            binder.properties_goal.as_ref(),
        ]
        .into_iter()
        .flatten()
        .all(|point| {
            if point.bind_point_type as u8 & 3 == 1 && self.binder_target_objects.is_none() {
                return false;
            }
            match point.bind_target_point_type {
                0 => {
                    self.binder_object.is_some()
                        || (point.bind_point_type as u8 & 3 == 1
                            && self.binder_target_objects.is_some())
                }
                3 => true, // Missing ElementIds are query failures, not admission failures.
                _ => false,
            }
        })
    }

    fn resolved_binder_target(
        &self,
        point: &crate::avfx::AvfxBinderProperties,
        following: bool,
    ) -> Option<VfxBinderTarget> {
        let matrix = match point.bind_target_point_type {
            0 => self.binder_object?.matrix(),
            3 => {
                let id = point.bind_point_id as u32;
                if let Some(targets) = &self.binder_targets {
                    targets.iter().find(|target| target.id == id)?.matrix
                } else if !self.has_resolved_binder_sources() {
                    return self
                        .bind_points
                        .iter()
                        .find(|point| point.id == id)
                        .map(|point| {
                            VfxBinderTarget::from_matrix(point.local_matrix(), following)
                        });
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        let [x, y, z] = matrix.basis;
        let p = matrix.position;
        Some(VfxBinderTarget::from_matrix(
            [
                x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, p[0], p[1],
                p[2], 1.0,
            ],
            following,
        ))
    }

    fn resolved_point_target(
        &self,
        point: &crate::avfx::AvfxBinderProperties,
        following: bool,
        target_index: i32,
    ) -> Option<VfxBinderTarget> {
        if target_index < 0 {
            return self.resolved_binder_target(point, following);
        }
        let source = self
            .binder_target_objects
            .as_ref()?
            .get(target_index as usize)?;
        let matrix = match point.bind_target_point_type {
            0 => source.object?.matrix(),
            3 => {
                source
                    .targets
                    .iter()
                    .find(|target| target.id == point.bind_point_id as u32)?
                    .matrix
            }
            _ => return None,
        };
        let [x, y, z] = matrix.basis;
        let p = matrix.position;
        Some(VfxBinderTarget::from_matrix(
            [
                x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, p[0], p[1],
                p[2], 1.0,
            ],
            following,
        ))
    }

    fn point_listener_scale(&self, target_index: i32, scale: &mut f32) -> bool {
        if target_index < 0 {
            *scale = self.vfx_scale;
            return true;
        }
        let Some(source) = self
            .binder_target_objects
            .as_ref()
            .and_then(|sources| sources.get(target_index as usize))
        else {
            return false;
        };
        *scale = source.vfx_scale;
        true
    }

    /// Supply a fixed listener VFX scale. Per-update changes require Binder
    /// history inputs, which are not yet modeled by this runtime constructor.
    pub fn with_vfx_scale(mut self, scale: f32) -> Result<Self, String> {
        if !scale.is_finite() {
            return Err("VFX listener scale must be finite".into());
        }
        self.vfx_scale = scale;
        Ok(self)
    }

    /// Fixed resolved Document scale getter, separate from the Document
    /// matrix/revised transform and listener VFX scale. Dynamic scale and
    /// camera compensation still require their own runtime inputs.
    pub fn with_document_scale(mut self, scale: [f32; 3]) -> Result<Self, String> {
        if !scale.into_iter().all(f32::is_finite) {
            return Err("Document scale must be finite".into());
        }
        self.document_scale = Some(scale);
        Ok(self)
    }

    /// Supply the fixed host affine matrix consumed by Document +0x20.
    /// Root revision is composed before this host matrix. Binder query
    /// targets must already be resolved in the same coordinate space.
    pub fn with_document_matrix(mut self, host: VfxBinderMatrix) -> Result<Self, String> {
        self.document_transform = self.document_transform_for_host(host)?;
        Ok(self)
    }

    fn document_transform_for_host(
        &self,
        host: VfxBinderMatrix,
    ) -> Result<VfxDocumentTransform, String> {
        if !host
            .basis
            .into_iter()
            .flatten()
            .chain(host.position)
            .all(f32::is_finite)
        {
            return Err("Document host matrix must be finite".into());
        }
        Ok(VfxDocumentTransform::new(
            self.file.global.revised_rotation,
            self.file.global.revised_scale,
            self.file.global.revised_position,
            host,
            None,
        ))
    }

    fn document_scale(&self) -> [f32; 3] {
        self.document_scale.unwrap_or(self.document_transform.scale)
    }

    /// Supply a fixed camera in the same space as targets and particles.
    /// This enables bTSd and ZBMs/ZBMd in continuous previews. Dynamic camera
    /// snapshots and Binder query-age caching need per-update host inputs.
    pub fn with_camera_position(mut self, position: [f32; 3]) -> Result<Self, String> {
        if !position.into_iter().all(f32::is_finite) {
            return Err("VFX camera position must be finite".into());
        }
        self.camera_position = Some(position);
        if let Some(camera) = self.binder_camera.as_mut() {
            camera.position = position;
        }
        Ok(self)
    }

    /// Supply all three resolved Binder camera fields. RoTp=1 copies basis,
    /// RoTp=2 uses the parallel vector, and RoTp=3 uses position. A position
    /// alone cannot authorize camera-facing Binder playback.
    pub fn with_binder_camera_snapshot(
        mut self,
        camera: VfxBinderCameraSnapshot,
    ) -> Result<Self, String> {
        camera.validate()?;
        if self
            .camera_binder
            .as_ref()
            .is_some_and(|input| input.camera != camera)
        {
            return Err("Camera Binder requires a complete Camera snapshot".into());
        }
        self.camera_position = Some(camera.position);
        self.binder_camera = Some(camera);
        Ok(self)
    }

    fn supports_binder_camera(&self, binder: &AvfxBinder) -> bool {
        match binder.rotation_type as i8 {
            0 => true,
            1..=3 => self.binder_camera.is_some(),
            _ => false,
        }
    }

    /// Supply complete resolved Camera inputs. Ordinary RoTp snapshots do not
    /// supply inverse view, constructor storage or query partial writes.
    pub fn with_camera_binder_snapshot(
        mut self,
        snapshot: VfxCameraBinderSnapshot,
    ) -> Result<Self, String> {
        self.validate_camera_binder_snapshot(&snapshot)?;
        self.install_camera_binder_snapshot(snapshot);
        Ok(self)
    }

    fn validate_camera_binder_snapshot(
        &self,
        snapshot: &VfxCameraBinderSnapshot,
    ) -> Result<(), String> {
        snapshot.validate()?;
        if snapshot.root_flag_f4 != self.file.global.ags_enabled {
            return Err("Camera root+f4 must match the file's compiled bAGS value".into());
        }
        if snapshot.samples.iter().any(|sample| {
            self.file
                .binders
                .get(sample.binder_index)
                .is_none_or(|binder| binder.binder_type != 3)
        }) {
            return Err("Camera sample references an invalid Camera Binder".into());
        }
        Ok(())
    }

    /// Supply the preview owner's raw Euler angles in radians before the
    /// initial Camera view. A new Camera uses the original Common storage
    /// defaults; these angles are independent of camera, bones and revised
    /// root rotation. Generic resolved snapshots keep their explicit sources.
    pub fn with_preview_camera_host_rotation(mut self, angles: [f32; 3]) -> Result<Self, String> {
        if !angles.into_iter().all(f32::is_finite) {
            return Err("preview Camera host rotation must be finite".into());
        }
        if self.camera_binder.is_some() {
            return Err("preview host rotation must be supplied before initial Camera view".into());
        }
        self.preview_camera_host_rotation =
            Some(VfxBinderMatrix::root_revision(angles, [1.0; 3], [0.0; 3]));
        Ok(self)
    }

    /// Use the renderer's actual view and authored compiled root fields.
    /// bAGS-off Camera does not read query/constructor storage sources. For
    /// bAGS-on uses explicit object/ElementId providers for caster queries;
    /// unknown point sources or host rotation remain capability failures. An
    /// explicit preview owner supplies fresh Common storage for Timeline birth.
    pub fn with_preview_camera_view(mut self, view: VfxCameraViewSnapshot) -> Result<Self, String> {
        view.validate()?;
        let snapshot = self.preview_camera_binder_snapshot(view);
        if let Some(snapshot) = &snapshot {
            self.validate_camera_binder_snapshot(snapshot)?;
        }
        self.camera_position = Some(view.camera.position);
        self.binder_camera = Some(view.camera);
        if let Some(snapshot) = snapshot {
            self.install_camera_binder_snapshot(snapshot);
        }
        Ok(self)
    }

    fn preview_camera_binder_snapshot(
        &self,
        view: VfxCameraViewSnapshot,
    ) -> Option<VfxCameraBinderSnapshot> {
        self.file
            .binders
            .iter()
            .any(|binder| binder.binder_type == 3)
            .then(|| VfxCameraBinderSnapshot {
                inverse_view: view.inverse_view,
                camera: view.camera,
                screen_height: view.screen_height,
                root_flag_f4: self.file.global.ags_enabled,
                ify_scale: None,
                // Emitter-owner 3bfeff..3bff66 reads compiled root a0..a8/94..9c.
                document_rotation: if self.file.global.ags_enabled {
                    VfxBinderMatrix::root_revision(
                        self.file.global.revised_rotation,
                        [1.0; 3],
                        [0.0; 3],
                    )
                } else {
                    VfxBinderMatrix::IDENTITY
                },
                root_position: if self.file.global.ags_enabled {
                    self.file.global.revised_position
                } else {
                    [0.0; 3]
                },
                timeline: self
                    .preview_camera_host_rotation
                    .map(VfxCameraBinderTimelineSnapshot::for_new_object),
                samples: Vec::new(),
            })
    }

    fn install_camera_binder_snapshot(&mut self, snapshot: VfxCameraBinderSnapshot) {
        self.camera_position = Some(snapshot.camera.position);
        self.binder_camera = Some(snapshot.camera);
        self.camera_binder = Some(snapshot);
    }

    fn camera_binder_frame(&self, age: f32) -> Option<VfxCameraBinderFrame> {
        let input = self.camera_binder.as_ref()?;
        Some(VfxCameraBinderFrame {
            age,
            root_flag_f4: input.root_flag_f4,
            inverse_view: input.inverse_view,
            camera: input.camera,
            document_scale: self.document_scale(),
            screen_height: input.screen_height,
            ify_scale: input.ify_scale,
        })
    }

    fn camera_binder_sample(&self, index: usize) -> Option<&VfxCameraBinderSample> {
        self.camera_binder
            .as_ref()?
            .samples
            .iter()
            .find(|sample| sample.binder_index == index)
    }

    fn supports_camera_caster_sources(&self, index: usize) -> bool {
        let Some(point) = self
            .file
            .binders
            .get(index)
            .and_then(|binder| binder.properties_start.as_ref())
        else {
            return false;
        };
        // 3bef20's caster branch uses BPTP's packed three bits and BPID's
        // byte; it never chooses a target-list entry based on BPT.
        match point.bind_target_point_type as u8 & 7 {
            0 => self.binder_object.is_some(),
            3 => {
                self.binder_targets.is_some()
                    && matches!(point.binder_name.as_str(), "" | "null")
                    && point.bct as u8 & 3 == 0
                    && !point.ring_enabled
            }
            _ => false,
        }
    }

    fn camera_binder_query(
        &self,
        index: usize,
        scale: &mut [f32; 3],
        vfx: &mut f32,
        depth: &mut f32,
    ) -> bool {
        if let Some(sample) = self.camera_binder_sample(index) {
            return sample.apply_query(scale, vfx, depth);
        }
        if !self.supports_camera_caster_sources(index) {
            return false;
        }
        let binder = &self.file.binders[index];
        let mut point = binder.properties_start.as_ref().unwrap().clone();
        point.bind_target_point_type = (point.bind_target_point_type as u8 & 7) as i32;
        point.bind_point_id = point.bind_point_id as u8 as i32;
        VfxCameraBinderState::query_caster(
            binder,
            self.camera_binder.as_ref().unwrap().camera.position,
            scale,
            vfx,
            depth,
            || self.resolved_binder_target(&point, binder.following_target_orientation),
            |value| self.point_listener_scale(-1, value),
        )
    }

    fn camera_binder_distance(&self, index: usize, age: f32, total: f32) -> Option<f32> {
        self.camera_binder_distance_from(index, age, total, self.camera_binder.as_ref()?)
    }

    fn camera_binder_distance_from(
        &self,
        index: usize,
        age: f32,
        total: f32,
        input: &VfxCameraBinderSnapshot,
    ) -> Option<f32> {
        if let Some(value) = input
            .samples
            .iter()
            .find(|sample| sample.binder_index == index)
            .and_then(|sample| sample.distance)
        {
            return Some(value);
        }
        let data = self.file.binders.get(index)?.data.as_ref()?;
        let curve = data.distance.as_ref()?;
        if data.distance_random.as_ref().is_some_and(|random| {
            random.random_type != 0
                || random.pre_behavior > 3
                || random.post_behavior > 3
                || random.keys.iter().any(|key| {
                    key.interpolation > 2 || !key.x.is_finite() || key.y != 0.0 || key.z != 0.0
                })
        }) || curve.random_type != 0
            || curve.keys.is_empty()
            || curve.pre_behavior > 3
            || curve.post_behavior > 3
            || curve.keys.iter().any(|key| {
                key.interpolation > 2 || ![key.x, key.y, key.z].into_iter().all(f32::is_finite)
            })
        {
            return None;
        }
        let value = curve.value_at(age, total, 0.0);
        value.is_finite().then_some(value)
    }

    fn supports_camera_binder(&self, index: usize, timeline: bool) -> bool {
        let Some(input) = &self.camera_binder else {
            return false;
        };
        self.supports_camera_binder_input(index, timeline, input)
    }

    fn supports_camera_binder_input(
        &self,
        index: usize,
        timeline: bool,
        input: &VfxCameraBinderSnapshot,
    ) -> bool {
        let Some(binder) = self.file.binders.get(index) else {
            return false;
        };
        binder.binder_type == 3
            && self
                .camera_binder_distance_from(index, 0.0, 0.0, input)
                .is_some()
            && (!input.root_flag_f4 || !timeline || input.timeline.is_some())
            && (!input.root_flag_f4
                || binder.properties_start.is_none()
                || input
                    .samples
                    .iter()
                    .any(|sample| sample.binder_index == index)
                || self.supports_camera_caster_sources(index))
    }

    fn camera_binder_construction(
        &self,
        index: usize,
        age: f32,
        timeline: bool,
    ) -> Option<VfxCameraBinderConstruction> {
        if !self.supports_camera_binder(index, timeline) {
            return None;
        }
        let input = self.camera_binder.as_ref()?;
        let owner = if timeline && input.root_flag_f4 {
            let source = input.timeline.as_ref()?;
            VfxCameraBinderOwner::Timeline {
                host_rotation: source.rotation,
                storage_scale: source.storage_scale,
                local: source.local,
                ancestors: &source.ancestors,
            }
        } else if timeline {
            // +f4 off never reads constructor storage/rotation; the update
            // creates its diagonal auxiliary. These ignored fields are inert.
            VfxCameraBinderOwner::Timeline {
                host_rotation: VfxBinderMatrix::IDENTITY,
                storage_scale: [1.0; 3],
                local: VfxCameraBinderScaleNode {
                    inherits_scale: false,
                    is_timeline: false,
                    scale: [1.0; 3],
                    timeline_scale: [1.0; 3],
                },
                ancestors: &[],
            }
        } else {
            VfxCameraBinderOwner::Emitter {
                document_rotation: input.document_rotation,
            }
        };
        let birth = VfxCameraBinderState::construct(
            &self.file.binders[index],
            self.camera_binder_frame(age)?,
            self.camera_binder_distance(index, age, age)?,
            owner,
            input.root_position,
            |scale, vfx, depth| self.camera_binder_query(index, scale, vfx, depth),
        );
        birth.state.is_finite().then_some(birth)
    }

    /// Supply the resolved Document depth override. Zero retains authored
    /// DpOf; this scalar is independent of Document's matrix and scale getter.
    pub fn with_document_depth_offset(mut self, offset: f32) -> Result<Self, String> {
        if !offset.is_finite() {
            return Err("Document depth offset must be finite".into());
        }
        self.document_depth_offset = offset;
        Ok(self)
    }

    fn particle_depth_offset(
        &self,
        ctx: &SpawnContext,
        particle: &AvfxParticle,
        position: [f32; 3],
    ) -> f32 {
        VfxDepthOffsetParameters {
            document_override: self.document_depth_offset,
            // Without a host camera snapshot, retain the existing continuous
            // approximation. Unsupported flags still block staged admission.
            bias_z_max_scale: self
                .camera_position
                .map_or(1.0, |_| self.file.global.bias_z_max_scale),
            bias_z_max_distance: self.file.global.bias_z_max_distance,
        }
        .evaluate(
            particle.depth_offset,
            ctx.binder_base.depth_offset_multiplier,
            position,
            self.camera_position.unwrap_or(position),
        )
    }

    fn spawner_depth_offset(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
    ) -> f32 {
        let position = if self.camera_position.is_some()
            && self.file.global.bias_z_max_scale > 1.0
            && self.file.global.bias_z_max_distance > 0.0
        {
            let (parent, drift) = influence_transform(item, ctx);
            particle_position_at(ctx, particle, ctx.curve_ages(), parent.linear, drift)
        } else {
            // The disabled distance branch never reads particle/camera position.
            [0.0; 3]
        };
        self.particle_depth_offset(ctx, particle, position)
    }

    fn revised_color(&self) -> [f32; 3] {
        if self.file.global.revised_color == [0.0; 3] {
            [1.0; 3]
        } else {
            self.file.global.revised_color
        }
    }

    /// timeline item 的绑点偏移（binder_index → binder.BPID → 武器绑点位置）。
    fn bind_offset(&self, binder_index: i32) -> [f32; 3] {
        let Ok(binder_index) = usize::try_from(signed_byte_index(binder_index)) else {
            return [0.0; 3];
        };
        let Some(binder) = self.file.binders.get(binder_index) else {
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

    /// Resolve a fixed local Linear birth through the native query/update
    /// order. Dynamic provider identities and lifetime belong to playback.
    fn fixed_linear_binder_initialization(
        &self,
        binder: &AvfxBinder,
    ) -> Option<(VfxLinearBinderState, VfxLinearBinderInitialization)> {
        let birth = self.local_linear_binder_construction(binder)?;
        Some((birth.state, birth.initialization?))
    }

    fn local_linear_binder_construction(
        &self,
        binder: &AvfxBinder,
    ) -> Option<VfxLinearBinderConstruction> {
        self.local_linear_binder_construction_at_age(binder, 0.0)
    }

    fn local_linear_binder_construction_at_age(
        &self,
        binder: &AvfxBinder,
        age: f32,
    ) -> Option<VfxLinearBinderConstruction> {
        self.local_linear_binder_construction_for_targets(
            binder,
            age,
            point_factory::LinearTargetBirth {
                target_indices: [-1, -1],
                delay: 0.0,
            },
        )
    }

    fn local_linear_binder_construction_for_targets(
        &self,
        binder: &AvfxBinder,
        age: f32,
        birth: point_factory::LinearTargetBirth,
    ) -> Option<VfxLinearBinderConstruction> {
        if !self.has_resolved_binder_sources()
            && (binder.life >= 0 || binder.properties_start.as_ref()?.generate_delay != 0)
        {
            return None;
        }
        let curves = local_linear_binder_curves(binder, age, age).filter(|_| {
            (self.has_resolved_binder_sources() || local_linear_binder_factor(binder).is_some())
                && (!binder.transform_scale_depth_offset || self.camera_position.is_some())
                && self.supports_binder_camera(binder)
                && self.supports_binder_target_sources(binder)
        })?;
        let endpoints = [
            binder.properties_start.as_ref()?,
            binder.properties_goal.as_ref()?,
        ];
        let document_scale = self.document_scale();
        let birth = VfxLinearBinderState::construct(
            binder,
            VfxLinearBinderFrame {
                age,
                query_deadlines: VfxLinearBinderFrame::authored_query_deadlines(binder),
                // Disabled depth never reads this value. Enabled depth must
                // have the explicit camera required above.
                camera_position: self.camera_position.unwrap_or([0.0; 3]),
                camera: self.binder_camera,
                document_scale,
                self_targets: birth.target_indices.map(|index| index < 0),
                root_ags: self.file.global.ags_enabled,
            },
            birth.delay,
            |endpoint| {
                self.resolved_point_target(
                    endpoints[endpoint],
                    binder.following_target_orientation,
                    birth.target_indices[endpoint],
                )
            },
            |endpoint, value| self.point_listener_scale(birth.target_indices[endpoint], value),
            || curves,
            if self.file.global.ags_enabled {
                VfxBinderMatrix::root_revision(
                    self.file.global.revised_rotation,
                    document_scale,
                    self.file.global.revised_position,
                )
            } else {
                VfxBinderMatrix::IDENTITY
            },
        );
        Some(birth)
    }

    fn local_spline_binder_construction_for_targets(
        &self,
        index: usize,
        age: f32,
        birth: point_factory::LinearTargetBirth,
    ) -> Option<(
        VfxSplineBinderConstruction,
        Option<VfxClientDualBinderCurveState>,
    )> {
        let binder = &self.file.binders[index];
        let mode = self.client_binder_random.as_ref()?.2;
        self.spline_binder_curves(binder, age, age).filter(|_| {
            self.has_resolved_binder_sources()
                && (!binder.transform_scale_depth_offset || self.camera_position.is_some())
                && self.supports_binder_camera(binder)
                && self.supports_binder_target_sources(binder)
        })?;
        let endpoints = [
            binder.properties_start.as_ref()?,
            binder.properties_goal.as_ref()?,
        ];
        let document_scale = self.document_scale();
        let curves = match &self.client_binder_curves {
            Some(environment) => {
                Some(self.client_binder_random.as_ref()?.0.with_mut(|random| {
                    VfxClientDualBinderCurveState::construct_from_authored(
                        binder,
                        environment.defaults,
                        random,
                    )
                })?)
            }
            None => None,
        };
        let birth = VfxSplineBinderState::construct_with_client_trig(
            binder,
            VfxLinearBinderFrame {
                age,
                query_deadlines: VfxLinearBinderFrame::authored_query_deadlines(binder),
                // Disabled depth never reads this value. Enabled depth must
                // have the explicit camera required above.
                camera_position: self.camera_position.unwrap_or([0.0; 3]),
                camera: self.binder_camera,
                document_scale,
                self_targets: birth.target_indices.map(|index| index < 0),
                root_ags: self.file.global.ags_enabled,
            },
            birth.delay,
            |endpoint| {
                self.resolved_point_target(
                    endpoints[endpoint],
                    binder.following_target_orientation,
                    birth.target_indices[endpoint],
                )
            },
            |endpoint, value| self.point_listener_scale(birth.target_indices[endpoint], value),
            || self.next_client_binder_random_word(),
            mode,
            || self.evaluate_spline_binder_curves(index, curves, age, age),
            if self.file.global.ags_enabled {
                VfxBinderMatrix::root_revision(
                    self.file.global.revised_rotation,
                    document_scale,
                    self.file.global.revised_position,
                )
            } else {
                VfxBinderMatrix::IDENTITY
            },
        );
        Some((birth, curves))
    }

    fn local_point_binder_construction(
        &self,
        binder: &AvfxBinder,
    ) -> Option<VfxPointBinderConstruction> {
        self.local_point_binder_construction_at_age(binder, 0.0)
    }

    fn local_point_binder_construction_at_age(
        &self,
        binder: &AvfxBinder,
        age: f32,
    ) -> Option<VfxPointBinderConstruction> {
        self.local_point_binder_construction_for_target(
            binder,
            age,
            point_factory::PointTargetBirth {
                target_index: -1,
                delay: binder.properties_start.as_ref()?.generate_delay as i16 as f32,
            },
        )
    }

    fn local_point_binder_construction_for_target(
        &self,
        binder: &AvfxBinder,
        age: f32,
        birth: point_factory::PointTargetBirth,
    ) -> Option<VfxPointBinderConstruction> {
        if !self.has_resolved_binder_sources()
            && (binder.life >= 0 || binder.properties_start.as_ref()?.generate_delay != 0)
        {
            return None;
        }
        let curves = local_point_binder_curves(binder, age, age).filter(|_| {
            (!binder.transform_scale_depth_offset || self.camera_position.is_some())
                && self.supports_binder_camera(binder)
                && self.supports_binder_target_sources(binder)
        })?;
        let document_scale = self.document_scale();
        Some(VfxPointBinderState::construct(
            binder,
            VfxPointBinderFrame {
                age,
                query_deadline: VfxPointBinderFrame::authored_query_deadline(binder),
                camera_position: self.camera_position.unwrap_or([0.0; 3]),
                camera: self.binder_camera,
                document_scale,
                self_target: birth.target_index < 0,
                root_revision: self.file.global.ags_enabled.then(|| {
                    VfxBinderMatrix::root_revision(
                        self.file.global.revised_rotation,
                        document_scale,
                        self.file.global.revised_position,
                    )
                }),
            },
            birth.delay,
            || {
                self.resolved_point_target(
                    binder.properties_start.as_ref()?,
                    binder.following_target_orientation,
                    birth.target_index,
                )
            },
            |scale| self.point_listener_scale(birth.target_index, scale),
            || curves,
        ))
    }

    /// A fixed Linear query failure bypasses the root child factory. Missing
    /// targets must not turn into a generic root at the start point or origin.
    fn fixed_linear_child_ready(&self, binder_index: i32) -> bool {
        let binder = usize::try_from(signed_byte_index(binder_index))
            .ok()
            .and_then(|index| self.file.binders.get(index));
        if self.has_resolved_binder_sources()
            && let Some(birth) =
                binder.and_then(|binder| self.local_point_binder_construction(binder))
        {
            return birth
                .initialization
                .is_some_and(VfxPointBinderInitialization::child_ready);
        }
        binder
            .and_then(|binder| self.fixed_linear_binder_initialization(binder))
            .is_none_or(|(_, initialization)| initialization.child_direction.is_some())
    }

    /// Root transform for the evidence-backed ElementId subset. The target
    /// query drops its orientation unless bFTO is enabled (0x1403bf498).
    /// Independent scale/depth getters consume the fixed listener and Document
    /// inputs. Dynamic bone/provider history and full Binder lifetime remain external.
    fn bind_base(&self, binder_index: i32) -> EmitterBase {
        let binder_index = signed_byte_index(binder_index);
        if binder_index < 0 {
            let document = self.document_transform;
            return EmitterBase {
                position: document.main_matrix.position,
                orientation: [0.0, 0.0, 0.0, 1.0],
                rotation_z: 0.0,
                scale: self.document_scale(),
                linear: document.main_matrix.basis,
                auxiliary_matrix: document.auxiliary_matrix,
                depth_offset_multiplier: 1.0,
            };
        }
        let mut base = EmitterBase::root(self.bind_offset(binder_index));
        let Some(binder) = usize::try_from(binder_index)
            .ok()
            .and_then(|index| self.file.binders.get(index))
        else {
            return base;
        };
        if let Some((state, _)) = self.fixed_linear_binder_initialization(binder) {
            return EmitterBase {
                position: state.matrix.position,
                orientation: [0.0, 0.0, 0.0, 1.0],
                rotation_z: 0.0,
                scale: state.scale,
                linear: state.matrix.basis,
                auxiliary_matrix: state.auxiliary_matrix,
                depth_offset_multiplier: state.depth_offset_multiplier(binder),
            };
        }
        if self.has_resolved_binder_sources()
            && let Some(birth) = self.local_point_binder_construction(binder)
        {
            let state = birth.state;
            return EmitterBase {
                position: state.matrix.position,
                orientation: [0.0, 0.0, 0.0, 1.0],
                rotation_z: 0.0,
                scale: state.scale,
                linear: state.matrix.basis,
                auxiliary_matrix: state.auxiliary_matrix,
                depth_offset_multiplier: state.depth_offset_multiplier(binder),
            };
        }
        if let Some(point) = (binder.bind_point_id >= 0)
            .then(|| {
                self.bind_points
                    .iter()
                    .find(|point| point.id == binder.bind_point_id as u32)
            })
            .flatten()
        {
            let target = VfxBinderTarget::from_matrix(
                point.local_matrix(),
                binder.following_target_orientation,
            );
            if binder.following_target_orientation {
                base.orientation = quat_from_euler(0, point.rotate);
            }
            base.linear = target.basis;
            // Point's independent +0x38 getter uses the target-query scale.
            // Linear/Spline need both queried endpoints and their own update.
            if binder.binder_type == 0 {
                let query = target.query_scale(binder, self.vfx_scale, true);
                let query_scale = query.target_scale;
                let document_scale = self.document_scale();
                base.scale = std::array::from_fn(|axis| query_scale[axis] * document_scale[axis]);
                base.auxiliary_matrix = if self.file.global.ags_enabled {
                    VfxBinderMatrix::root_revision(
                        self.file.global.revised_rotation,
                        document_scale,
                        self.file.global.revised_position,
                    )
                } else {
                    VfxBinderMatrix::scale_matrix(base.scale)
                };
                let depth = self
                    .camera_position
                    .map_or(1.0, |camera| target.query_depth_scale(binder, camera));
                base.depth_offset_multiplier = query.depth_offset_multiplier(binder, depth);
            }
        }
        if let Some(position) = static_point_binder_position(binder) {
            base.position = base.transform_point(position);
        }
        base
    }

    fn static_loop_point_binder(&self, binder_index: i32) -> bool {
        let Some(binder) = usize::try_from(signed_byte_index(binder_index))
            .ok()
            .and_then(|index| self.file.binders.get(index))
        else {
            return false;
        };
        static_point_binder_position(binder).is_some()
    }

    fn root_direction(&self, binder_index: i32) -> [f32; 3] {
        let Some(binder) = usize::try_from(signed_byte_index(binder_index))
            .ok()
            .and_then(|index| self.file.binders.get(index))
        else {
            return [0.0; 3];
        };
        if binder.binder_type == 0 {
            // Point emitter factory params +0x18 is the zero vector; target
            // orientation belongs to the Binder matrix, not injection.
            return [0.0; 3];
        }
        if let Some((_, result)) = self.fixed_linear_binder_initialization(binder) {
            return result.child_direction.unwrap_or([0.0; 3]);
        }
        let Some(point) = (binder.bind_point_id >= 0)
            .then(|| {
                self.bind_points
                    .iter()
                    .find(|point| point.id == binder.bind_point_id as u32)
            })
            .flatten()
        else {
            // Binder direction still requires the external parent-bone basis;
            // retain the preview's +Z approximation when no element rotation
            // is available.
            return [0.0, 0.0, 1.0];
        };
        // MDL ElementId stores the local Euler offset in radians. Applying it
        // to the default emitter axis is the portion that is independent of
        // the missing parent-bone/document transform.
        quat_rotate(quat_from_euler(0, point.rotate), [0.0, 0.0, 1.0])
    }

    pub fn max_quads(&self) -> usize {
        self.max_quads
    }

    /// 采样 `time_seconds` 时刻的全部存活粒子四边形（out 先清空）。
    pub fn sample(&self, time_seconds: f32, out: &mut Vec<VfxQuad>) {
        out.clear();
        let frame = time_seconds * AVFX_FPS;
        let mut draw_order = 0_u64;
        let mut independent_order = 0_u64;
        self.sample_ambient(frame, &mut |ctx, item, particle| {
            if particle.particle_type == Some(ParticleType::ModelSkin) {
                return;
            }
            let counter = if matches!(
                particle.particle_type,
                Some(ParticleType::Decal | ParticleType::DecalRing)
            ) {
                &mut independent_order
            } else {
                &mut draw_order
            };
            let order = *counter;
            *counter += 1;
            if matches!(
                particle.particle_type,
                Some(ParticleType::Model) | Some(ParticleType::LightModel)
            ) {
                return; // 网格粒子走 sample_mesh
            }
            let start = out.len();
            self.push_quad(&ctx, item, particle, out);
            for quad in &mut out[start..] {
                quad.draw_order = Some(order);
            }
        });
        if self.file.global.draw_order == 1 {
            for quad in out.iter_mut() {
                if quad.decal.is_none() {
                    quad.draw_order = quad.draw_order.map(|order| draw_order - 1 - order);
                }
            }
            out.sort_by_key(|quad| {
                (
                    quad.decal.is_some(),
                    if quad.decal.is_some() {
                        0
                    } else {
                        quad.draw_priority
                    },
                    quad.draw_order,
                )
            });
        }
    }

    /// 采样 `time_seconds` 时刻的网格粒子实例（out 先清空）。
    pub fn sample_mesh(&self, time_seconds: f32, out: &mut Vec<VfxMeshInstance>) {
        out.clear();
        let frame = time_seconds * AVFX_FPS;
        let mut draw_order = 0_u64;
        self.sample_ambient(frame, &mut |ctx, item, particle| {
            if matches!(
                particle.particle_type,
                Some(ParticleType::ModelSkin | ParticleType::Decal | ParticleType::DecalRing)
            ) {
                return;
            }
            let order = draw_order;
            draw_order += 1;
            if !matches!(
                particle.particle_type,
                Some(ParticleType::Model) | Some(ParticleType::LightModel)
            ) {
                return;
            }
            let start = out.len();
            self.push_mesh_instance(&ctx, item, particle, out);
            for mesh in &mut out[start..] {
                mesh.draw_order = Some(order);
            }
        });
        if self.file.global.draw_order == 1 {
            for mesh in out.iter_mut() {
                mesh.draw_order = mesh.draw_order.map(|order| draw_order - 1 - order);
            }
            out.sort_by_key(|mesh| (mesh.draw_priority, mesh.draw_order));
        }
    }

    /// 遍历常驻（scheduler items 指向的）timeline 上 `frame` 时刻存活的所有
    /// 粒子出生事件。每个 scheduler item 都创建独立的 timeline 实例。
    /// 无 scheduler 时回退遍历全部 timeline。
    fn sample_ambient(
        &self,
        frame: f32,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        if !frame.is_finite() || frame < 0.0 {
            return;
        }
        for (scheduler_index, scheduler) in self.file.schedulers.iter().enumerate() {
            for (item_index, item) in scheduler.items.iter().enumerate() {
                if !item.enabled || item.timeline_index < 0 {
                    continue;
                }
                let Some(timeline) = self.file.timelines.get(item.timeline_index as usize) else {
                    continue;
                };
                if frame < item.start_time as f32 {
                    continue;
                }
                let seed = SplitMix64::seeded(
                    scheduler_index as u64,
                    item_index as u64,
                    item.timeline_index as u64,
                    0x5C4D,
                )
                .next_u64();
                self.sample_timeline(timeline, frame - item.start_time as f32, seed, sink);
            }
        }
        if self.file.schedulers.is_empty() {
            for (index, timeline) in self.file.timelines.iter().enumerate() {
                let seed = SplitMix64::seeded(index as u64, 0, 0, 0x714E).next_u64();
                self.sample_timeline(timeline, frame, seed, sink);
            }
        }
    }

    fn sample_timeline(
        &self,
        timeline: &AvfxTimeline,
        frame: f32,
        instance_seed: u64,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        if !self.fixed_linear_child_ready(timeline.binder_index) {
            return;
        }
        // LpSt < LpEd 才循环；0..0 表示一次性播放（item 的 EdTm=-1 = 常驻）。
        let local = loop_age(frame, timeline.loop_start, timeline.loop_end);
        let period = f64::from(timeline.loop_end) - f64::from(timeline.loop_start);
        let period_f32 = timeline.loop_end as f32 - timeline.loop_start as f32;
        let loop_index =
            if timeline.loop_end > timeline.loop_start && frame >= timeline.loop_end as f32 {
                ((frame - timeline.loop_start as f32) / period_f32).floor() as u64
            } else {
                0
            };
        let mut sample_cycle = |local: f32, cycle: u64| {
            for (item_index, item) in timeline.items.iter().enumerate() {
                let AvfxTimelineItemTarget::Emitter(emitter_index) = item.target() else {
                    continue;
                };
                if !item.enabled {
                    continue;
                }
                if !self.fixed_linear_child_ready(item.binder_index) {
                    continue;
                }
                if timeline.loop_end > timeline.loop_start && item.start_time >= timeline.loop_end {
                    continue;
                }
                if local < item.start_time as f32 {
                    continue;
                }
                let Some(emitter) = self.file.emitters.get(emitter_index as usize) else {
                    continue;
                };
                self.sample_emitter(
                    emitter_index as usize,
                    emitter,
                    InstanceClock::root(
                        emitter,
                        if item.end_time < 0 {
                            -1.0
                        } else {
                            (item.end_time - item.start_time) as f32
                        },
                    ),
                    local - item.start_time as f32,
                    self.bind_base(timeline_root_binder_index(timeline, item.binder_index)),
                    self.bind_base(timeline_root_binder_index(timeline, item.binder_index)),
                    self.root_direction(item.binder_index),
                    None,
                    ParentColor::None,
                    ParentComponents::None,
                    [0.0; 3],
                    SplitMix64::seeded(instance_seed, item_index as u64, cycle, 0xE417).next_u64(),
                    0,
                    sink,
                );
            }
        };
        if let Some(tail) = self.finite_timeline_tail_bound(timeline) {
            const MAX_REPLAYED_TIMELINE_CYCLES: u64 = 256;
            let oldest = ((f64::from(frame) - f64::from(tail)) / period)
                .floor()
                .max(0.0) as u64;
            let oldest = oldest.saturating_sub(1).min(loop_index);
            if loop_index - oldest < MAX_REPLAYED_TIMELINE_CYCLES {
                for cycle in oldest..=loop_index {
                    let cycle_age = if cycle == loop_index {
                        local
                    } else {
                        frame - (cycle as f32 * period_f32)
                    };
                    sample_cycle(cycle_age, cycle);
                }
                return;
            }
        }
        sample_cycle(local, loop_index);
    }

    /// A finite subtree bound lets continuous sampling keep old loop roots
    /// without guessing how many generations can still be visible.
    fn finite_timeline_tail_bound(&self, timeline: &AvfxTimeline) -> Option<f32> {
        if timeline.loop_start < 0
            || timeline.loop_end <= timeline.loop_start
            || (signed_byte_index(timeline.binder_index) >= 0
                && !self.static_loop_point_binder(timeline.binder_index))
        {
            return None;
        }
        let mut tail = 0.0_f32;
        for item in timeline.items.iter().filter(|item| item.enabled) {
            if item.start_time >= timeline.loop_end {
                continue;
            }
            let emitter_index = match item.target() {
                AvfxTimelineItemTarget::Emitter(index) => index,
                AvfxTimelineItemTarget::None | AvfxTimelineItemTarget::Effector(_) => continue,
                AvfxTimelineItemTarget::Clip(_) => return None,
            };
            // Intro items may not restart with the repeated segment.
            if item.start_time < timeline.loop_start || signed_byte_index(item.binder_index) >= 0 {
                return None;
            }
            let emitter = self.file.emitters.get(emitter_index as usize)?;
            let duration = if item.end_time < 0 {
                -1.0
            } else {
                (item.end_time - item.start_time) as f32
            };
            let clock = InstanceClock::root(emitter, duration);
            let subtree = self.finite_emitter_tail_bound(
                emitter_index as usize,
                clock,
                clock.finish_frame(),
                0,
            )?;
            let last_visible = item.start_time as f32 + subtree;
            if !last_visible.is_finite() {
                return None;
            }
            tail = tail.max(last_visible);
        }
        Some(tail.next_up().next_up())
    }

    fn finite_emitter_tail_bound(
        &self,
        emitter_index: usize,
        clock: InstanceClock,
        max_alive: Option<f32>,
        depth: u32,
    ) -> Option<f32> {
        let emitter = self.file.emitters.get(emitter_index)?;
        if emitter.effector_index >= 0 {
            return None;
        }
        let mut tail = 0.0_f32;
        for creation in &emitter.particle_items {
            if !creation.enabled
                || creation.target_index < 0
                || creation.create_probability <= 0
                || !emitter_can_create(emitter, creation)
            {
                continue;
            }
            let particle = self.file.particles.get(creation.target_index as usize)?;
            if creation.parameter_link >= 0 || particle.collision_type != -1 {
                return None;
            }
            let life = clock.max_child_duration(
                particle.life,
                [particle.loop_start, particle.loop_end],
                creation,
            )?;
            let last_visible = last_creation_age(creation, max_alive)? + life;
            if !last_visible.is_finite() {
                return None;
            }
            tail = tail.max(last_visible.next_up().next_up());
        }
        if depth < 4 {
            for creation in &emitter.emitter_items {
                if !creation.enabled
                    || creation.target_index < 0
                    || creation.create_probability <= 0
                    || !emitter_can_create(emitter, creation)
                {
                    continue;
                }
                if creation.parameter_link >= 0 {
                    return None;
                }
                let child = self.file.emitters.get(creation.target_index as usize)?;
                let child_alive = clock.max_child_duration(
                    child.life,
                    [child.loop_start, child.loop_end],
                    creation,
                );
                let nominal_life = if child.life.enabled {
                    child.life.value
                } else {
                    -1.0
                };
                let child_tail = self.finite_emitter_tail_bound(
                    creation.target_index as usize,
                    InstanceClock::root(child, nominal_life),
                    child_alive,
                    depth + 1,
                )?;
                let last_visible = last_creation_age(creation, max_alive)? + child_tail;
                if !last_visible.is_finite() {
                    return None;
                }
                tail = tail.max(last_visible.next_up().next_up());
            }
        }
        Some(tail)
    }

    /// 枚举发射器在 `emitter_frame`（实例出生起算的帧）时刻存活的粒子出生事件。
    /// `base` 为父链基准变换；创建角加到本地 Euler，按本实例的 ROT/CCOT 求值。
    fn sample_emitter(
        &self,
        emitter_index: usize,
        emitter: &AvfxEmitter,
        clock: InstanceClock,
        emitter_frame: f32,
        base: EmitterBase,
        binder_base: EmitterBase,
        direction: [f32; 3],
        parent: Option<EmitterParent<'_>>,
        parent_color: ParentColor<'_>,
        parent_components: ParentComponents<'_>,
        creation_angle: [f32; 3],
        instance_seed: u64,
        depth: u32,
        sink: &mut dyn FnMut(&SpawnContext, &AvfxEmitterItem, &AvfxParticle),
    ) {
        if emitter_frame < 0.0 || depth > 4 || !clock.rate.is_finite() || clock.rate <= 0.0 {
            return;
        }
        let color_animation = EmitterColorAnimation {
            emitter,
            emitter_index,
            instance_seed,
            clock,
            parent: parent_color,
        };
        let component_animation = EmitterComponents {
            emitter,
            emitter_index,
            instance_seed,
            clock,
            parent: parent_components,
            creation_angle,
        };
        let emitter_cap = if emitter.child_limit > 0 {
            emitter.child_limit as usize
        } else {
            // ClCn <= 0 disables the definition limit in both client factories.
            // Retain the preview's work bound independently of that field.
            self.max_quads
        };
        let mut emitted = 0usize;
        let emitter_animation = EmitterAnimation {
            emitter,
            clock,
            base,
            binder_base,
            direction,
            parent,
            sampled_parent: None,
            parent_components,
            creation_angle,
            random: SplitMix64::seeded(emitter_index as u64 ^ 0xE417, 0, instance_seed, 0x1A2B),
        };
        let transform_now = emitter_animation.at(emitter_frame);
        let mut particle_counters = vec![InjectionCounter::default(); emitter.particle_items.len()];
        let mut emitter_counters = vec![InjectionCounter::default(); emitter.emitter_items.len()];
        // Ordered GeMT modes use one counter owned by the emitter instance,
        // shared by every ItPr/ItEm item. Recover calls omitted before the
        // common live-history window without replaying constant tails.
        let schedule =
            CreationSchedule::new(&self.file, emitter, clock, emitter_frame, instance_seed);
        let mut shape_ordinal = schedule.skipped_creations(instance_seed);
        for birth in schedule {
            let event = birth.event;
            let spawn_loop_age = birth.frame;
            let item_index = birth.item_index;
            match birth.target {
                CreationTarget::Particle(particle) => {
                    let particle_item = birth.item;
                    let item_seed =
                        SplitMix64::seeded(instance_seed, item_index as u64, 0, 0x17_5052)
                            .next_u64();
                    let spawn_frame = spawn_loop_age;
                    let emitter_ages = clock.ages(spawn_frame);
                    let age = emitter_frame - spawn_frame;
                    // 出生时刻的发射器变换（曲线按发射器 loop-local 年龄求值）。
                    let transform = emitter_animation.at(spawn_loop_age);
                    let world = transform.world();
                    let per_event = emitter_create_count_at_seeded(
                        emitter,
                        particle_item,
                        emitter_ages.local,
                        emitter_ages.total,
                        event,
                        instance_seed,
                    );
                    let random =
                        |event, copy| SplitMix64::seeded(item_seed, event, copy, 0xC2B2_AE3D);
                    let counter = &mut particle_counters[item_index];
                    counter.begin_event(
                        emitter,
                        clock,
                        particle_item,
                        event,
                        spawn_loop_age,
                        instance_seed,
                        random,
                    );
                    for k in 0..per_event {
                        if emitted >= emitter_cap {
                            return;
                        }
                        let mut rng = random(event, k);
                        if particle_item.create_probability < 100
                            && rng.next_f32() * 100.0 >= particle_item.create_probability as f32
                        {
                            continue;
                        }
                        let creation_angle = counter.take_angle(particle_item);
                        let particle_clock = clock.child(
                            particle.life,
                            [particle.loop_start, particle.loop_end],
                            particle_item,
                            &mut rng,
                        );
                        if !particle_clock.rate.is_finite() || particle_clock.rate <= 0.0 {
                            continue;
                        }
                        let create_index = shape_ordinal;
                        shape_ordinal = shape_ordinal.wrapping_add(1);
                        if let Some(life) = particle_clock.finish_frame() {
                            if age > life {
                                continue;
                            }
                        }
                        // Invalid geometry still consumes the bounded creation budget.
                        emitted += 1;
                        let seed = rng.next_u64();
                        let Some(injection) = self.sample_injection(
                            particle_item,
                            &emitter_animation,
                            spawn_loop_age,
                            create_index,
                            world,
                            &mut rng,
                        ) else {
                            continue;
                        };
                        let ctx = SpawnContext {
                            cached_color: None,
                            cached_xyz: None,
                            cached_textures: None,
                            client_injection: None,
                            age,
                            total_age: particle_clock.ages(age).total,
                            clock: particle_clock,
                            gravity_offset: None,
                            shape_binding: injection.binding,
                            motion: motion::InjectionMotion::new(
                                particle_item,
                                world,
                                injection.position,
                                injection.direction,
                                injection.velocity,
                            ),
                            emitter: transform,
                            emitter_now: transform_now,
                            binder_base: emitter_animation.binder_base,
                            emitter_animation: &emitter_animation,
                            root_transform_history: None,
                            spawn_loop_age,
                            parent_color: ParentColor::new(
                                particle_item.parent_influence_color,
                                &color_animation,
                                spawn_frame,
                            ),
                            revised_color: self.revised_color(),
                            parent_components: ParentComponents::new(
                                particle_item,
                                &component_animation,
                                spawn_frame,
                            ),
                            creation_angle,
                            seed,
                            create_index,
                        };
                        sink(&ctx, particle_item, particle);
                    }
                }
                CreationTarget::Emitter(child) => {
                    let emitter_item = birth.item;
                    let item_seed =
                        SplitMix64::seeded(instance_seed, item_index as u64, 0, 0x17_454D)
                            .next_u64();
                    let world = emitter_animation.at(spawn_loop_age).world();
                    let random =
                        |event, copy| SplitMix64::seeded(item_seed, event, copy, 0x3C4D_5E6F);
                    let counter = &mut emitter_counters[item_index];
                    counter.begin_event(
                        emitter,
                        clock,
                        emitter_item,
                        event,
                        spawn_loop_age,
                        instance_seed,
                        random,
                    );
                    let emitter_ages = clock.ages(spawn_loop_age);
                    for k in 0..emitter_create_count_at_seeded(
                        emitter,
                        emitter_item,
                        emitter_ages.local,
                        emitter_ages.total,
                        event,
                        instance_seed,
                    ) {
                        if emitted >= emitter_cap {
                            return;
                        }
                        let mut rng = random(event, k);
                        if emitter_item.create_probability < 100
                            && rng.next_f32() * 100.0 >= emitter_item.create_probability as f32
                        {
                            continue;
                        }
                        emitted += 1;
                        let creation_angle = counter.take_angle(emitter_item);
                        // Only initialization/finish allocate delay helpers.
                        // Those queues still need their own update clock.
                        let child_birth = spawn_loop_age
                            + if emitter_item.create_time == 0 {
                                0.0
                            } else {
                                emitter_item.generate_delay as f32
                            };
                        let child_clock = clock.child(
                            child.life,
                            [child.loop_start, child.loop_end],
                            emitter_item,
                            &mut rng,
                        );
                        let child_seed = rng.next_u64();
                        let create_index = shape_ordinal;
                        shape_ordinal = shape_ordinal.wrapping_add(1);
                        let Some(injection) = self.sample_injection(
                            emitter_item,
                            &emitter_animation,
                            spawn_loop_age,
                            create_index,
                            world,
                            &mut rng,
                        ) else {
                            continue;
                        };
                        let motion = motion::InjectionMotion::new(
                            emitter_item,
                            world,
                            injection.position,
                            injection.direction,
                            injection.velocity,
                        );
                        self.sample_emitter(
                            emitter_item.target_index as usize,
                            child,
                            child_clock,
                            emitter_frame - child_birth,
                            world,
                            emitter_animation.binder_base,
                            motion.direction,
                            Some(EmitterParent {
                                animation: &emitter_animation,
                                birth: child_birth,
                                transform: world,
                                motion,
                                binding: injection.binding,
                            }),
                            ParentColor::new(
                                emitter_item.parent_influence_color,
                                &color_animation,
                                child_birth,
                            ),
                            ParentComponents::new(emitter_item, &component_animation, child_birth),
                            creation_angle,
                            child_seed,
                            depth + 1,
                            sink,
                        );
                    }
                }
            }
        }
    }

    fn sample_injection(
        &self,
        item: &AvfxEmitterItem,
        animation: &EmitterAnimation,
        frame: f32,
        create_index: u64,
        world: EmitterBase,
        rng: &mut SplitMix64,
    ) -> Option<SpawnInjection> {
        Self::sample_injection_from_file(
            &self.file,
            item,
            animation,
            frame,
            create_index,
            world,
            rng,
        )
    }

    fn sample_injection_from_file(
        file: &AvfxFile,
        item: &AvfxEmitterItem,
        animation: &EmitterAnimation,
        frame: f32,
        create_index: u64,
        world: EmitterBase,
        rng: &mut SplitMix64,
    ) -> Option<SpawnInjection> {
        let (offset, direction, speed, binding) =
            Self::sample_spawn_shape_from_file(file, animation, frame, create_index, rng)?;
        let emitter = animation.emitter;
        let position = world.transform_point(offset);
        let inherited_velocity = if item.inherit_parent_velocity {
            animation
                .parent
                .map(|parent| parent.motion.world_velocity(parent.transform.linear))
                .unwrap_or([0.0; 3])
        } else {
            [0.0; 3]
        };
        if matches!(emitter.data, None | Some(AvfxEmitterData::None)) {
            return Some(SpawnInjection {
                position,
                direction: motion::InjectionDirection::Inherited(animation.direction),
                velocity: inherited_velocity,
                binding,
            });
        }
        let ages = animation.clock.ages(frame);
        let angles = std::array::from_fn(|axis| {
            if matches!(
                emitter.data,
                Some(
                    AvfxEmitterData::Cone(_)
                        | AvfxEmitterData::ConeModel(_)
                        | AvfxEmitterData::CylinderModel(_)
                        | AvfxEmitterData::SphereModel(_)
                        | AvfxEmitterData::Model(_)
                )
            ) {
                return 0.0;
            }
            curve_value_seeded_at(
                &emitter.injection_angle[axis],
                &emitter.injection_angle_random[axis],
                ages,
                0.0,
                rng.next_u64(),
            )
        });
        let direction = if angles == [0.0; 3] {
            direction
        } else {
            quat_rotate(quat_from_euler(emitter.rotation_order, angles), direction)
        };
        let direction = motion::normalized(basis_transform(world.linear, direction));
        Some(SpawnInjection {
            position,
            direction: motion::InjectionDirection::World(direction),
            velocity: std::array::from_fn(|axis| {
                direction[axis] * speed + inherited_velocity[axis]
            }),
            binding,
        })
    }

    /// 形状采样：返回发射器本地空间的 (出生偏移, 注入方向, 注入速度)。
    #[cfg(test)]
    fn sample_spawn_shape(
        &self,
        animation: &EmitterAnimation,
        loop_age: f32,
        create_index: u64,
        rng: &mut SplitMix64,
    ) -> Option<([f32; 3], [f32; 3], f32, ShapeBinding)> {
        Self::sample_spawn_shape_from_file(&self.file, animation, loop_age, create_index, rng)
    }

    fn sample_spawn_shape_from_file(
        file: &AvfxFile,
        animation: &EmitterAnimation,
        loop_age: f32,
        create_index: u64,
        rng: &mut SplitMix64,
    ) -> Option<([f32; 3], [f32; 3], f32, ShapeBinding)> {
        let binding;
        let emitter_frame = loop_age;
        let sample = match &animation.emitter.data {
            Some(AvfxEmitterData::Cone(cone)) => {
                binding = ShapeBinding::Center;
                cone::sample(animation, cone, emitter_frame, rng)
            }
            Some(AvfxEmitterData::ConeModel(cone)) => {
                return cone_model::sample(animation, cone, emitter_frame, create_index, rng);
            }
            Some(AvfxEmitterData::CylinderModel(cyl)) => {
                return cylinder_model::sample(animation, cyl, emitter_frame, create_index, rng);
            }
            Some(AvfxEmitterData::SphereModel(sphere)) => {
                return sphere_model::sample(animation, sphere, emitter_frame, create_index, rng);
            }
            Some(AvfxEmitterData::Model(model)) => {
                let method = model.generate_method as u8;
                if method > 7 {
                    return None;
                }
                let geometry = file
                    .models
                    .get(usize::try_from(model.model_index as i8).ok()?)?;
                let numbers = &geometry.emit_vertex_numbers;
                let count = u16::try_from(numbers.len()).ok()?;
                if count == 0 {
                    return None;
                }
                // Unlike shape angles, Model's speed First byte stays zero
                // in 0x1403d9a70. Only Always adds a random speed offset.
                let speed = if matches!(model.injection_speed_random.random_type & 7, 3..=5) {
                    cone::curve_value_at(
                        &model.injection_speed,
                        &model.injection_speed_random,
                        animation.clock.ages(emitter_frame),
                        0,
                        rng,
                    )
                } else {
                    let ages = animation.clock.ages(emitter_frame);
                    model.injection_speed.value_at(ages.local, ages.total, 0.0)
                };
                let rotation = animation.model_shape_rotation(emitter_frame, rng);
                // The client reads VEmt[VNum[ordinal]], without inverting VNum.
                // The preview's event/copy ordinal is still a scheduling approximation.
                let ordinal = if method & 1 != 0 {
                    (create_index % u64::from(count)) as usize
                } else {
                    let product = i32::from(count).wrapping_mul(i32::from(rng.next_u64() as u16));
                    // Overflowing client indices are not safe vertex selections.
                    usize::try_from(product / 65536).ok()?
                };
                let vertex = geometry
                    .emit_vertices
                    .get(numbers[ordinal] as u16 as usize)?;
                if method & 2 == 0 {
                    binding = ShapeBinding::Center;
                    ([0.0; 3], quat_rotate(rotation, vertex.position), speed)
                } else {
                    // Keep the selected vertex before shape rotation so ICbP can
                    // evaluate it with the current shape angles and emitter matrix.
                    let direction = if animation.emitter.any_direction {
                        animation.model_injection_direction(emitter_frame, rng)
                    } else {
                        vertex.normal
                    };
                    binding = ShapeBinding::ModelVertex {
                        vertex: vertex.position,
                        random_seed: rng.clone().next_u64(),
                    };
                    (
                        quat_rotate(rotation, vertex.position),
                        quat_rotate(rotation, direction),
                        speed,
                    )
                }
            }
            _ => {
                binding = ShapeBinding::Center;
                ([0.0; 3], [0.0, 1.0, 0.0], 0.0)
            }
        };
        Some((sample.0, sample.1, sample.2, binding))
    }

    /// 粒子曲线求值年龄：粒子自身的 LpSt/LpEd 循环。
    #[cfg(test)]
    fn particle_age(particle: &AvfxParticle, age: f32) -> f32 {
        loop_age(age, particle.loop_start, particle.loop_end)
    }

    fn push_quad(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxQuad>,
    ) {
        self.push_quad_at(ctx, item, particle, ctx.curve_ages(), ctx.curve_ages(), out);
    }

    fn push_quad_at(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        cache_ages: CurveAges,
        draw_ages: CurveAges,
        out: &mut Vec<VfxQuad>,
    ) {
        self.push_quad_at_with_overrides(
            ctx, item, particle, cache_ages, draw_ages, None, None, out,
        );
    }

    fn push_quad_at_with_polyline(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        cache_ages: CurveAges,
        draw_ages: CurveAges,
        polyline: VfxPolyline,
        out: &mut Vec<VfxQuad>,
    ) {
        self.push_quad_at_with_overrides(
            ctx,
            item,
            particle,
            cache_ages,
            draw_ages,
            None,
            Some(polyline),
            out,
        );
    }

    fn push_quad_at_with_overrides(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        cache_ages: CurveAges,
        draw_ages: CurveAges,
        simple_line_delta: Option<&mut dyn FnMut(u64, [f32; 3], [f32; 3]) -> [f32; 3]>,
        polyline_override: Option<VfxPolyline>,
        out: &mut Vec<VfxQuad>,
    ) {
        if out.len() >= self.max_quads {
            return;
        }
        // These types either allocate no client draw object or modify existing
        // model render resources. None of them use generic Shape geometry.
        if matches!(
            particle.particle_type,
            None | Some(
                ParticleType::Parameter
                    | ParticleType::Reserve0
                    | ParticleType::ModelSkin
                    | ParticleType::Dissolve
                    | ParticleType::Unknown(_)
            )
        ) {
            return;
        }
        if particle.particle_type == Some(ParticleType::Laser)
            && !matches!(particle.rotation_direction_base, 0..=2)
        {
            return;
        }
        if particle.particle_type == Some(ParticleType::Line)
            && !particle.simple_anim_enable
            && !matches!(particle.rotation_direction_base, 0..=2)
        {
            return;
        }
        let powder_single = matches!(particle.particle_type, Some(ParticleType::Powder));
        if powder_single && particle.simple_anim_enable {
            self.push_powder(ctx, item, particle, out);
            return;
        }
        if particle.particle_type == Some(ParticleType::Line) && particle.simple_anim_enable {
            self.push_simple_line(
                ctx,
                item,
                particle,
                None,
                cache_ages,
                simple_line_delta,
                out,
            );
            return;
        }
        let windmill = particle.particle_type == Some(ParticleType::Windmill);
        let windmill_uv_type = match particle.data {
            AvfxParticleData::Windmill { uv_type } => uv_type as u8,
            _ => 0,
        };
        if windmill && windmill_uv_type > 1 {
            return;
        }
        let ages = ctx.curve_ages();
        let seed = ctx.seed;
        let tex = ResolvedTexture::at_times_with_client(
            particle,
            cache_ages,
            draw_ages,
            seed,
            ctx.cached_textures,
        );
        let mut color = ctx.color(particle);
        let (parent, pos_drift) = influence_transform(item, ctx);
        let (scale, parent_basis, orientation, position) = if powder_single || windmill {
            // Powder/Windmill share 0x1403b08a0, 0x1403b0af0 and apricot_powder VS1.
            let local = particle_xyz_at(ctx, particle, ages, 0);
            let mut euler = particle_euler_at(ctx, particle, ages);
            let inherited = ctx.parent_components.at(ctx.age);
            euler[2] += inherited.rotation[2];
            color = quantize_powder_color(color);
            let scale = std::array::from_fn(|axis| local[axis] * inherited.scale[axis]);
            let orientation = quat_from_euler(2, euler);
            let position = particle_position_at(ctx, particle, ages, parent.linear, pos_drift);
            (scale, VFX_IDENTITY_BASIS, orientation, position)
        } else {
            let transform = particle_draw_transform_at(ctx, item, particle, ages);
            (
                transform.scale,
                transform.parent_basis,
                transform.orientation,
                transform.position,
            )
        };
        let disc = if particle.particle_type == Some(ParticleType::Disc) {
            let Some(disc) = disc::sample_at_times_with_client(
                particle,
                draw_ages,
                cache_ages,
                seed,
                scale[2],
                ctx.cached_textures.and_then(|value| value.disc_geometry),
            ) else {
                return;
            };
            Some(disc)
        } else {
            None
        };
        let polygon = if particle.particle_type == Some(ParticleType::Polygon) {
            // Polygon +108 evaluates Cnt/CntR from the current instance ages
            // (0x1403f0291); its transform still comes from the property cache.
            let Some(polygon) = polygon::sample_at_with_client(
                particle,
                draw_ages,
                seed,
                scale[2],
                ctx.cached_textures.and_then(|value| value.polygon_count),
            ) else {
                return;
            };
            Some(polygon)
        } else {
            None
        };
        let laser = if particle.particle_type == Some(ParticleType::Laser) {
            if matches!(particle.rotation_direction_base, 0..=2) {
                let Some(laser) = laser::sample_at_with_client(
                    particle,
                    draw_ages,
                    seed,
                    scale,
                    ctx.cached_textures.and_then(|value| value.laser_dimensions),
                ) else {
                    return;
                };
                Some(laser)
            } else {
                None
            }
        } else {
            None
        };
        let line = if particle.particle_type == Some(ParticleType::Line)
            && !particle.simple_anim_enable
            && matches!(particle.rotation_direction_base, 0..=2)
        {
            let Some(mut line) = line::sample_at_times_with_client(
                particle,
                draw_ages,
                cache_ages,
                seed,
                color,
                ctx.cached_textures.and_then(|value| value.line_geometry),
            ) else {
                return;
            };
            line.scale = scale;
            Some(line)
        } else {
            None
        };
        let polyline = if particle.particle_type == Some(ParticleType::Polyline) {
            let basis = basis_mul(parent_basis, rotation_scale_basis(orientation, scale));
            let Some(polyline) = polyline_override.or_else(|| {
                polyline::sample_at_times_with_client(
                    particle,
                    draw_ages,
                    cache_ages,
                    seed,
                    color,
                    position,
                    basis,
                    ctx.cached_textures
                        .and_then(|value| value.polyline_geometry),
                )
            }) else {
                return;
            };
            Some(polyline)
        } else {
            None
        };
        let decal = if matches!(
            particle.particle_type,
            Some(ParticleType::Decal | ParticleType::DecalRing)
        ) {
            // Ring +108 evaluates WID/WIDR at draw time (0x140404c78),
            // independently of the cached transform's third scale axis.
            let Some(decal) = decal::sample_at(particle, draw_ages, seed, scale[2]) else {
                return;
            };
            Some(decal)
        } else {
            None
        };
        out.push(VfxQuad {
            particle_type: particle.particle_type,
            particle_index: item.target_index as usize,
            soft_particle: particle.is_soft_particle,
            soft_particle_fade_range: self.file.global.effective_soft_particle_fade_range(),
            depth_offset_type: particle.depth_offset_type,
            depth_offset: self.particle_depth_offset(ctx, particle, position),
            powder_single,
            windmill_uv_type,
            disc,
            polygon,
            laser,
            line,
            polyline,
            decal,
            position,
            size: [0.5 * scale[0], 0.5 * scale[1]],
            orientation,
            parent_basis,
            movement_direction: ctx.motion.drawing_direction(parent.linear),
            facing_parent_basis: ctx.motion.drawing_parent(parent.linear),
            rotation_direction_base: particle.rotation_direction_base,
            color,
            draw_layer: self.file.global.draw_layer,
            soft_key_offset: self.file.global.soft_key_offset,
            draw_priority: i32::from(particle.draw_priority as i8),
            draw_order: None,
            pivot: [0.0; 2],
            texture_indexes: tex.texture_indexes,
            texture_uv_sets: tex.texture_uv_sets,
            combine_mode_tc1: tex.combine_mode_tc1,
            combine_modes: tex.combine_modes,
            color_to_alpha: tex.color_to_alpha,
            uv_origins: tex.uv_origins,
            uv_scales: tex.uv_scales,
            uv_by_pixel_position: tex.uv_by_pixel_position,
            uv_rotations: tex.uv_rotations,
            texture_borders: tex.texture_borders,
            texture_filters: tex.texture_filters,
            texture1_is_shape_mask: tex.texture1_is_shape_mask,
            texture1_enabled: tex.texture1_enabled,
            texture1_use_screen_copy: tex.texture1_use_screen_copy,
            draw_mode: particle.draw_mode,
            depth_test: particle.depth_test,
            depth_write: particle.depth_write,
            cull_mode: particle.culling_type,
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
            uvd_rotation: tex.uvd_rotation,
            uvd_by_pixel_position: tex.uvd_by_pixel_position,
            distortion_uv_set: tex.distortion_uv_set,
            distortion_borders: tex.distortion_borders,
            distortion_filter: tex.distortion_filter,
            texture_palette_index: tex.texture_palette_index,
            palette_offset: tex.palette_offset,
            palette_border: tex.palette_border,
            palette_filter: tex.palette_filter,
        });
    }

    fn push_mesh_instance(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        let ages = ctx.curve_ages();
        self.push_mesh_instance_at(ctx, item, particle, ages, ages, out);
    }

    fn push_mesh_instance_at(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        render_curve_ages: CurveAges,
        draw_ages: CurveAges,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        self.push_mesh_instance_at_selected(
            ctx,
            item,
            particle,
            render_curve_ages,
            draw_ages,
            None,
            out,
        );
    }

    fn push_mesh_instance_at_selected(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        render_curve_ages: CurveAges,
        draw_ages: CurveAges,
        selected_model_index: Option<i32>,
        out: &mut Vec<VfxMeshInstance>,
    ) {
        if out.len() >= self.max_quads {
            return;
        }
        let model_index = selected_model_index.unwrap_or_else(|| match &particle.data {
            AvfxParticleData::LightModel { model_index } => *model_index,
            AvfxParticleData::Model { .. } => model_particle_index(particle, draw_ages, ctx.seed),
            _ => -1,
        });
        if model_index < 0 {
            return;
        }
        let Some(geometry) = self.file.models.get(model_index as usize) else {
            return;
        };
        if geometry.draw.is_none() {
            return;
        }
        let ages = ctx.curve_ages();
        let seed = ctx.seed;
        let tex = ResolvedTexture::at_times_with_client(
            particle,
            render_curve_ages,
            draw_ages,
            seed,
            ctx.cached_textures,
        );
        let color = ctx.color(particle);
        let (parent, pos_drift) = influence_transform(item, ctx);
        let (scale, parent_basis) = particle_scale_and_parent_at(ctx, particle, ages, parent);
        let position = particle_position_at(ctx, particle, ages, parent.linear, pos_drift);
        out.push(VfxMeshInstance {
            position,
            orientation: particle_orientation_at(ctx, particle, ages),
            parent_basis,
            movement_direction: ctx.motion.drawing_direction(parent.linear),
            facing_parent_basis: ctx.motion.drawing_parent(parent.linear),
            rotation_direction_base: particle.rotation_direction_base,
            scale,
            color,
            fresnel: model_fresnel(&particle.data, render_curve_ages, draw_ages, seed),
            draw_layer: self.file.global.draw_layer,
            soft_key_offset: self.file.global.soft_key_offset,
            draw_priority: i32::from(particle.draw_priority as i8),
            draw_order: None,
            texture_indexes: tex.texture_indexes,
            texture_uv_sets: tex.texture_uv_sets,
            combine_mode_tc1: tex.combine_mode_tc1,
            combine_modes: tex.combine_modes,
            color_to_alpha: tex.color_to_alpha,
            uv_origins: tex.uv_origins,
            uv_scales: tex.uv_scales,
            uv_by_pixel_position: tex.uv_by_pixel_position,
            uv_rotations: tex.uv_rotations,
            texture_borders: tex.texture_borders,
            texture_filters: tex.texture_filters,
            texture_normal_index: tex.texture_normal_index,
            normal_uv_set: tex.normal_uv_set,
            normal_uv_origin: tex.normal_uv_origin,
            normal_uv_scale: tex.normal_uv_scale,
            normal_uv_rotation: tex.normal_uv_rotation,
            normal_uv_by_pixel_position: tex.normal_uv_by_pixel_position,
            normal_texture_borders: tex.normal_texture_borders,
            normal_texture_filter: tex.normal_texture_filter,
            normal_power: tex.normal_power,
            reflection_enabled: tex.reflection_enabled,
            reflection_use_screen_copy: tex.reflection_use_screen_copy,
            reflection_texture_index: tex.reflection_texture_index,
            reflection_texture_filter: tex.reflection_texture_filter,
            reflection_calculate_color: tex.reflection_calculate_color,
            reflection_rate: tex.reflection_rate,
            reflection_power: tex.reflection_power,
            texture1_is_shape_mask: tex.texture1_is_shape_mask,
            texture1_enabled: tex.texture1_enabled,
            texture1_use_screen_copy: tex.texture1_use_screen_copy,
            draw_mode: particle.draw_mode,
            depth_test: particle.depth_test,
            depth_write: particle.depth_write,
            texture_distortion_index: tex.texture_distortion_index,
            distortion_power: tex.distortion_power,
            distortion_targets: tex.distortion_targets,
            uvd_origin: tex.uvd_origin,
            uvd_scale: tex.uvd_scale,
            uvd_rotation: tex.uvd_rotation,
            uvd_by_pixel_position: tex.uvd_by_pixel_position,
            distortion_uv_set: tex.distortion_uv_set,
            distortion_borders: tex.distortion_borders,
            distortion_filter: tex.distortion_filter,
            texture_palette_index: tex.texture_palette_index,
            palette_offset: tex.palette_offset,
            palette_border: tex.palette_border,
            palette_filter: tex.palette_filter,
            cull_mode: particle.culling_type,
            model_index: model_index as usize,
            soft_particle: particle.is_soft_particle,
            soft_particle_fade_range: self.file.global.effective_soft_particle_fade_range(),
            depth_offset_type: particle.depth_offset_type,
            depth_offset: self.particle_depth_offset(ctx, particle, position),
        });
    }

    /// Smpl Line reconstruction for the unbound Point configuration used by
    /// all installed weapon Line definitions. The client retains draw-to-draw
    /// history; the stateless sampler uses the preceding 0.1-frame substep.
    fn push_simple_line(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        state: Option<&powder::SpawnerState>,
        cache_ages: CurveAges,
        mut endpoint_delta: Option<&mut dyn FnMut(u64, [f32; 3], [f32; 3]) -> [f32; 3]>,
        out: &mut Vec<VfxQuad>,
    ) {
        if !line::supports_simple(particle) {
            return;
        }
        let simple = particle.simple.as_ref().unwrap();
        let spawner_age = ctx.clock.elapsed(ctx.age) * ctx.clock.rate;
        let Some(base_line) = line::sample_at_times_with_client(
            particle,
            ctx.curve_ages(),
            cache_ages,
            ctx.seed,
            ctx.color(particle),
            ctx.cached_textures.and_then(|value| value.line_geometry),
        ) else {
            return;
        };
        let tex = ResolvedTexture::of(particle, spawner_age, ctx.seed);
        let length_scale = if simple.scale_by_parent {
            let (parent, _) = influence_transform(item, ctx);
            let particle_age = ctx.clock.age(ctx.age);
            let (scale, parent_basis) =
                particle_scale_and_parent(ctx, particle, particle_age, parent);
            let basis = basis_mul(
                parent_basis,
                rotation_scale_basis(particle_orientation(ctx, particle, particle_age), scale),
            );
            basis
                .into_iter()
                .map(|axis| {
                    axis.into_iter()
                        .map(|value| value * value)
                        .sum::<f32>()
                        .sqrt()
                })
                .sum::<f32>()
                / 3.0
        } else {
            1.0
        };
        if !length_scale.is_finite() {
            return;
        }
        let depth_offset = self.spawner_depth_offset(ctx, item, particle);
        for slot_index in 0..simple.create_count as i16 as u64 {
            if out.len() >= self.max_quads {
                return;
            }
            let slot = powder::SimpleSlot::new_line(simple, ctx.seed, ctx.create_index, slot_index);
            let active = state.and_then(|state| state.slots.get(slot_index as usize));
            if active.is_some_and(|slot| slot.phase != powder::SlotPhase::Active) {
                continue;
            }
            let Some((birth, sub_age)) = active
                .map(|slot| (0.0, slot.age))
                .or_else(|| slot.sample(spawner_age, simple.create_new_after_delete))
            else {
                continue;
            };
            let at_birth = ctx.at_age(birth / ctx.clock.rate - ctx.clock.warmup);
            let age = ctx.clock.age(at_birth.age);
            let (parent, drift) = influence_transform(item, &at_birth);
            let origin = particle_position(&at_birth, particle, age, parent.linear, drift);
            // Line birth 0x1403ec14a transforms both packed position and
            // velocity before the world-axis decay in 0x1403ebd43.
            let (scale, parent_basis) = particle_scale_and_parent(&at_birth, particle, age, parent);
            let basis = basis_mul(
                parent_basis,
                rotation_scale_basis(particle_orientation(&at_birth, particle, age), scale),
            );
            let birth_offset =
                basis_transform(basis, powder::packed_position(slot.birth_offset, false));
            let velocity = basis_transform(basis, slot.velocity);
            let previous_age = (sub_age - 0.1).max(0.0);
            let travel: [f32; 3] = std::array::from_fn(|axis| {
                powder::velocity_displacement(velocity[axis], simple.coord_accuracy[axis], sub_age)
            });
            let previous_travel: [f32; 3] = std::array::from_fn(|axis| {
                powder::velocity_displacement(
                    velocity[axis],
                    simple.coord_accuracy[axis],
                    previous_age,
                )
            });
            let stateless_delta = std::array::from_fn(|axis| travel[axis] - previous_travel[axis]);
            let slot_color =
                sample_flipbook_offset(&simple.colors, &simple.frames, sub_age, slot.life_offset);
            let color_begin = quantize_powder_color(std::array::from_fn(|axis| {
                base_line.color_begin[axis] * slot_color[axis]
            }));
            let color_end = quantize_powder_color(std::array::from_fn(|axis| {
                base_line.color_end[axis] * slot_color[axis]
            }));
            let position = active.map_or_else(
                || std::array::from_fn(|axis| origin[axis] + birth_offset[axis] + travel[axis]),
                |slot| slot.position,
            );
            let stateless_delta = active.map_or(stateless_delta, |slot| {
                std::array::from_fn(|axis| slot.position[axis] - slot.birth_position[axis])
            });
            if !position.into_iter().all(f32::is_finite) {
                continue;
            }
            let delta = endpoint_delta.as_mut().map_or(stateless_delta, |resolve| {
                resolve(slot_index, position, stateless_delta)
            });
            let Some(endpoint_offset) = VfxLine::simple_endpoint_offset(
                delta,
                simple.line_length_min * length_scale,
                simple.line_length_max * length_scale,
            ) else {
                continue;
            };
            out.push(VfxQuad {
                particle_type: particle.particle_type,
                particle_index: item.target_index as usize,
                soft_particle: particle.is_soft_particle,
                soft_particle_fade_range: self.file.global.effective_soft_particle_fade_range(),
                depth_offset_type: particle.depth_offset_type,
                depth_offset,
                powder_single: false,
                windmill_uv_type: 0,
                disc: None,
                polygon: None,
                laser: None,
                line: Some(VfxLine {
                    scale: [1.0; 3],
                    length: 0.0,
                    endpoint_offset: Some(endpoint_offset),
                    color_begin,
                    color_end,
                }),
                polyline: None,
                decal: None,
                position,
                size: [0.0; 2],
                orientation: [0.0, 0.0, 0.0, 1.0],
                parent_basis: VFX_IDENTITY_BASIS,
                movement_direction: [0.0; 3],
                facing_parent_basis: VFX_IDENTITY_BASIS,
                rotation_direction_base: 0,
                color: [1.0; 4],
                draw_layer: self.file.global.draw_layer,
                soft_key_offset: self.file.global.soft_key_offset,
                draw_priority: i32::from(particle.draw_priority as i8),
                draw_order: None,
                pivot: [0.0; 2],
                texture_indexes: tex.texture_indexes,
                texture_uv_sets: tex.texture_uv_sets,
                combine_mode_tc1: tex.combine_mode_tc1,
                combine_modes: tex.combine_modes,
                color_to_alpha: tex.color_to_alpha,
                uv_origins: tex.uv_origins,
                uv_scales: tex.uv_scales,
                uv_by_pixel_position: tex.uv_by_pixel_position,
                uv_rotations: tex.uv_rotations,
                texture_borders: tex.texture_borders,
                texture_filters: tex.texture_filters,
                texture1_is_shape_mask: tex.texture1_is_shape_mask,
                texture1_enabled: tex.texture1_enabled,
                texture1_use_screen_copy: tex.texture1_use_screen_copy,
                draw_mode: particle.draw_mode,
                depth_test: particle.depth_test,
                depth_write: particle.depth_write,
                cull_mode: particle.culling_type,
                texture_distortion_index: tex.texture_distortion_index,
                distortion_power: tex.distortion_power,
                distortion_targets: tex.distortion_targets,
                uvd_origin: tex.uvd_origin,
                uvd_scale: tex.uvd_scale,
                uvd_rotation: tex.uvd_rotation,
                uvd_by_pixel_position: tex.uvd_by_pixel_position,
                distortion_uv_set: tex.distortion_uv_set,
                distortion_borders: tex.distortion_borders,
                distortion_filter: tex.distortion_filter,
                texture_palette_index: tex.texture_palette_index,
                palette_offset: tex.palette_offset,
                palette_border: tex.palette_border,
                palette_filter: tex.palette_filter,
            });
        }
    }

    fn new_powder_state(
        &self,
        ctx: &SpawnContext,
        particle: &AvfxParticle,
    ) -> powder::SpawnerState {
        let simple = particle.simple.as_ref().expect("Smpl Powder definition");
        if particle.particle_type == Some(ParticleType::Line) {
            return powder::SpawnerState::new_line(
                (0..(simple.create_count as i16).max(0) as u64).map(|index| {
                    powder::SimpleSlot::new_line(simple, ctx.seed, ctx.create_index, index)
                }),
                ctx.seed,
            );
        }

        let model = (simple.injection_model_index as i8 >= 0)
            .then(|| {
                self.file
                    .models
                    .get(simple.injection_model_index as i8 as usize)
            })
            .flatten();
        powder::SpawnerState::new(
            (0..(simple.create_count as i16).max(0) as u64).map(|index| {
                let normal = (simple.injection_direction_type as i8 == 5)
                    .then(|| {
                        model
                            .and_then(|model| {
                                powder_emit_vertex(
                                    model,
                                    powder::birth_model_ordinal(simple, index),
                                )
                            })
                            .map(|vertex| vertex.normal)
                    })
                    .flatten();
                powder::SimpleSlot::new_with_direction(
                    simple,
                    ctx.seed,
                    ctx.create_index,
                    index,
                    normal,
                )
            }),
        )
    }

    fn advance_powder_state(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        frames: f32,
        state: &mut powder::SpawnerState,
    ) {
        let simple = particle.simple.as_ref().expect("Smpl Powder definition");
        let age = ctx.clock.age(ctx.age);
        let (parent, drift) = influence_transform(item, ctx);
        let origin = particle_position(ctx, particle, age, parent.linear, drift);
        let (scale, parent_basis) = particle_scale_and_parent(ctx, particle, age, parent);
        let basis = basis_mul(
            parent_basis,
            rotation_scale_basis(particle_orientation(ctx, particle, age), scale),
        );
        let model = (simple.injection_position_type as i8 == 1
            && simple.injection_model_index as i8 >= 0)
            .then(|| {
                self.file
                    .models
                    .get(simple.injection_model_index as i8 as usize)
            })
            .flatten();
        // The public client update refreshes the birth matrix once before its
        // substep loop. Successful births latch it; later reads cannot change it.
        state.advance(
            simple,
            frames,
            simple.injection_position_type as i8 != 2,
            |index, slot| {
                let vertex = model.and_then(|model| {
                    powder_emit_vertex(model, powder::birth_model_ordinal(simple, index as u64))
                });
                let local = std::array::from_fn(|axis| {
                    slot.birth_offset[axis] + vertex.map_or(0.0, |v| v.position[axis])
                });
                let position = powder::packed_position(
                    local,
                    simple.injection_vertex_bind_model_index as i8 >= 0,
                );
                if simple.bind_parent {
                    (position, slot.velocity)
                } else {
                    let offset = basis_transform(basis, position);
                    (
                        std::array::from_fn(|axis| origin[axis] + offset[axis]),
                        basis_transform(basis, slot.velocity),
                    )
                }
            },
        );
    }

    /// Powder with bSCt enabled: Smpl emits children instead of the single quad.
    /// Smpl 字段布局参考 VFXEditor，尺寸与旋转打包参考客户端指令。
    /// 重力、发射方向、运动与绑定仍使用近似，尚无客户端轨迹验证。
    fn push_powder(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        out: &mut Vec<VfxQuad>,
    ) {
        self.push_powder_with_state(ctx, item, particle, None, out);
    }

    fn push_powder_with_state(
        &self,
        ctx: &SpawnContext,
        item: &AvfxEmitterItem,
        particle: &AvfxParticle,
        state: Option<&powder::SpawnerState>,
        out: &mut Vec<VfxQuad>,
    ) {
        let Some(simple) = &particle.simple else {
            return;
        };
        if simple.create_count as i16 <= 0 {
            return;
        }
        // spawner 年龄 = 粒子年龄（本体寿命即粒子 Life）。
        let spawner_age = ctx.clock.elapsed(ctx.age) * ctx.clock.rate;
        let particle_age = ctx.clock.age(ctx.age);
        let position_type = simple.injection_position_type as i8;
        let direction_type = simple.injection_direction_type as i8;
        // SIPT=2 is fed by an external point source, not a zero origin.
        // 0x1403e806d -> 0x1403e74f0 clears the birth gate and only restores
        // it after obtaining a valid source and filling its points. No such
        // source is attached to this runtime yet, so no children can start.
        if position_type == 2 {
            return;
        }
        // The client reads IJMN/VBMN as signed bytes. SIPT=0 may prefill a
        // normal for SIDT=5, but only SIPT=1 pre-fills a model position.
        let emit_model = ((position_type == 1 || (position_type != 2 && direction_type == 5))
            && (simple.injection_model_index as i8) >= 0)
            .then(|| {
                self.file
                    .models
                    .get(simple.injection_model_index as i8 as usize)
            })
            .flatten();
        let bind_model = (position_type != 2
            && (simple.injection_vertex_bind_model_index as i8) >= 0)
            .then(|| {
                self.file
                    .models
                    .get(simple.injection_vertex_bind_model_index as i8 as usize)
            })
            .flatten();
        let spawner_color = ctx.color(particle);
        let tex = ResolvedTexture::of(particle, spawner_age, ctx.seed);
        // Client 0x1403f9969 measures the current spawner matrix's X/Y columns.
        let parent_scale = if simple.scale_by_parent {
            let (parent, _) = influence_transform(item, ctx);
            let (scale, parent_basis) =
                particle_scale_and_parent(ctx, particle, particle_age, parent);
            let basis = basis_mul(
                parent_basis,
                rotation_scale_basis(particle_orientation(ctx, particle, particle_age), scale),
            );
            std::array::from_fn(|axis| {
                let [x, y, z] = basis[axis];
                (x * x + y * y + z * z).sqrt()
            })
        } else {
            // 0x1403fc5df captures the +0x38 Binder/Document getter;
            // 0x1403fb383 multiplies the two half extents by these values.
            [ctx.binder_base.scale[0], ctx.binder_base.scale[1]]
        };
        // SBDT reads the current emitter directly, independently of ItPr PICd.
        // Modes 3/4 divide each original column by the corresponding +0x38
        // base scale before mode 3 permutes it (0x1403fc7c3..7f7).
        let emitter_basis = ctx.emitter_now.world().linear;
        let direction_basis = if matches!(simple.base_direction_type as i8, 3 | 4) {
            std::array::from_fn(|column| {
                emitter_basis[column].map(|value| value / ctx.binder_base.scale[column])
            })
        } else {
            emitter_basis
        };
        if !direction_basis.into_iter().flatten().all(f32::is_finite) {
            // The native division is undefined at zero base scale. Keep the
            // numerical operation, but do not upload its nonfinite geometry.
            return;
        }
        let (facing_basis, facing) = match simple.base_direction_type as i8 {
            0 => (
                VFX_IDENTITY_BASIS,
                crate::avfx::rotation_direction_base::SCREEN_BILLBOARD,
            ),
            1 | 3 => (
                [
                    direction_basis[0],
                    direction_basis[2].map(|v| -v),
                    direction_basis[1],
                ],
                crate::avfx::rotation_direction_base::NONE,
            ),
            2 | 4 => (direction_basis, crate::avfx::rotation_direction_base::NONE),
            _ => (
                VFX_IDENTITY_BASIS,
                crate::avfx::rotation_direction_base::CAMERA_BILLBOARD,
            ),
        };
        let depth_offset = self.spawner_depth_offset(ctx, item, particle);
        for slot_index in 0..simple.create_count as i16 as u64 {
            if out.len() >= self.max_quads {
                return;
            }
            let model_vertex = emit_model.and_then(|model| {
                powder_emit_vertex(model, powder::birth_model_ordinal(simple, slot_index))
            });
            let model_normal = (direction_type == 5)
                .then(|| model_vertex.map(|vertex| vertex.normal))
                .flatten();
            let staged_slot = state.and_then(|state| state.slots.get(slot_index as usize));
            let stateless_slot;
            let slot = if let Some(staged) = staged_slot {
                &staged.definition
            } else {
                stateless_slot = powder::SimpleSlot::new_with_direction(
                    simple,
                    ctx.seed,
                    ctx.create_index,
                    slot_index,
                    model_normal,
                );
                &stateless_slot
            };
            let timing = if let Some(staged) = staged_slot {
                (staged.phase == powder::SlotPhase::Active).then_some((spawner_age, staged.age))
            } else {
                slot.sample(spawner_age, simple.create_new_after_delete)
            };
            let Some((birth, sub_age)) = timing else {
                continue;
            };
            // AVFXTools PowderSpawner latches the transformed source point when
            // each child is born. Replaying a later query must keep that point.
            let at_birth = ctx.at_age(birth / ctx.clock.rate - ctx.clock.warmup);
            // bBnP keeps the slot in the parent's binding path. The client
            // does not bake the birth-time parent matrix into that slot, so
            // a moving parent is sampled at the current frame instead.
            let transform_ctx = if simple.bind_parent { ctx } else { &at_birth };
            let age = ctx.clock.age(transform_ctx.age);
            let (parent, drift) = influence_transform(item, transform_ctx);
            let origin = particle_position(transform_ctx, particle, age, parent.linear, drift);
            let (scale, parent_basis) =
                particle_scale_and_parent(transform_ctx, particle, age, parent);
            let basis = basis_mul(
                parent_basis,
                rotation_scale_basis(particle_orientation(transform_ctx, particle, age), scale),
            );
            // Camera-facing source geometry still lacks the GPU facing basis.
            let local_birth = std::array::from_fn(|axis| {
                (if position_type == 1 {
                    model_vertex.map_or(0.0, |vertex| vertex.position[axis])
                } else {
                    0.0
                }) + slot.birth_offset[axis]
            });
            let offset = basis_transform(
                basis,
                powder::packed_position(local_birth, bind_model.is_some()),
            );
            // SIPT=2 bypasses the birth matrix and the draw-time bBnP matrix.
            let base: [f32; 3] = if simple.injection_position_type as i8 == 2 {
                [0.0; 3]
            } else {
                std::array::from_fn(|axis| origin[axis] + offset[axis])
            };
            // 0x1403fbe43 transforms the initial velocity at birth unless
            // bBnP or SIPT=2 keeps the slot local. Per-axis decay happens
            // afterwards, so it cannot commute with a rotated basis.
            let velocity = if !simple.bind_parent && position_type != 2 {
                basis_transform(basis, slot.velocity)
            } else {
                slot.velocity
            };
            // Client 0x1403f9351/0x1403f9cc9 skips FltR/FltS without VBMN.
            let travel: [f32; 3] = std::array::from_fn(|axis| {
                if staged_slot.is_some() {
                    return 0.0;
                }
                if (simple.injection_vertex_bind_model_index as i8) >= 0 {
                    powder::bound_velocity_displacement(
                        velocity[axis],
                        simple.coord_accuracy[axis],
                        sub_age,
                        simple.velocity_flattery_rate,
                        simple.velocity_flattery_speed,
                    )
                } else {
                    powder::velocity_displacement(
                        velocity[axis],
                        simple.coord_accuracy[axis],
                        sub_age,
                    )
                }
            });
            if !travel.into_iter().all(f32::is_finite) {
                continue;
            }
            // Parent-bound slots accumulate local motion; 0x1403f9e52
            // transforms the complete draw position using the current basis.
            let travel = if simple.bind_parent && position_type != 2 {
                basis_transform(basis, travel)
            } else {
                travel
            };
            // 0x1403f9c80 computes CG * age² / 2, but the VBMN branch
            // at 0x1403f9dfe replaces it with interpolation from the raw
            // slot position. SIPT=2 bypasses that branch entirely.
            let gravity = if bind_model.is_some() {
                [0.0; 3]
            } else {
                let gravity = [
                    0.5 * simple.coord_gravity[0] * sub_age * sub_age,
                    0.5 * simple.coord_gravity[1] * sub_age * sub_age,
                    0.5 * simple.coord_gravity[2] * sub_age * sub_age,
                ];
                // bBnP transforms the complete local draw position at
                // 0x1403f9e52, including gravity; SIPT=2 skips it as well.
                if simple.bind_parent && position_type != 2 {
                    basis_transform(basis, gravity)
                } else {
                    gravity
                }
            };
            let moving = if let Some(staged) = staged_slot {
                let position = if simple.bind_parent {
                    let offset = basis_transform(basis, staged.position);
                    std::array::from_fn(|axis| origin[axis] + offset[axis])
                } else {
                    staged.position
                };
                std::array::from_fn(|axis| position[axis] + gravity[axis])
            } else {
                std::array::from_fn(|axis| base[axis] + travel[axis] + gravity[axis])
            };
            let position = if let Some(vertex) = bind_model.and_then(|model| {
                // VBMN advances its cursor for every slot (0x1403e495c),
                // but siblings copy the head's point (0x1403e4762).
                powder_emit_vertex(model, powder::block_first_slot(simple, slot_index))
            }) {
                // VBMN follows the current spawner matrix even when the
                // child's unbound birth position remains latched.
                let (bind_origin, bind_basis) = if simple.bind_parent {
                    (origin, basis)
                } else {
                    let age = ctx.clock.age(ctx.age);
                    let (parent, drift) = influence_transform(item, ctx);
                    let origin = particle_position(ctx, particle, age, parent.linear, drift);
                    let (scale, parent_basis) =
                        particle_scale_and_parent(ctx, particle, age, parent);
                    let basis = basis_mul(
                        parent_basis,
                        rotation_scale_basis(particle_orientation(ctx, particle, age), scale),
                    );
                    (origin, basis)
                };
                let local_bind =
                    std::array::from_fn(|axis| vertex.position[axis] + slot.binding_offset[axis]);
                let bind = basis_transform(bind_basis, powder::packed_position(local_bind, true));
                let progress = (simple.velocity_flattery_rate
                    + sub_age * simple.velocity_flattery_speed)
                    .clamp(0.0, 1.0);
                let factor = 0.5 - 0.5 * (std::f32::consts::PI * progress).cos();
                std::array::from_fn(|axis| {
                    moving[axis] + (bind_origin[axis] + bind[axis] - moving[axis]) * factor
                })
            } else {
                moving
            };
            let nominal_life = simple.create_interval_life as i16;
            let curve_t = if nominal_life > 0 {
                (sub_age / nominal_life as f32).powf(simple.scale_curve)
            } else {
                0.0
            };
            if !curve_t.is_finite() {
                continue;
            }
            let scale = [
                (simple.scale_start[0] + (simple.scale_end[0] - simple.scale_start[0]) * curve_t)
                    * slot.scale[0] as f32
                    / 50.0
                    * parent_scale[0],
                (simple.scale_start[1] + (simple.scale_end[1] - simple.scale_start[1]) * curve_t)
                    * slot.scale[1] as f32
                    / 50.0
                    * parent_scale[1],
            ];
            // 颜色帧簿：Frms 时间点之间插值 Cols。
            let color =
                sample_flipbook_offset(&simple.colors, &simple.frames, sub_age, slot.life_offset);
            // Client 0x1403fa0df multiplies by evaluated Col, then packs
            // through 0x1403af230. Quantizing either input first loses HDR.
            let color = quantize_powder_color(std::array::from_fn(|axis| {
                color[axis] * spawner_color[axis]
            }));
            let (uv_origin, uv_scale) = slot.uv(simple, sub_age);
            out.push(VfxQuad {
                particle_type: particle.particle_type,
                particle_index: item.target_index as usize,
                soft_particle: particle.is_soft_particle,
                soft_particle_fade_range: self.file.global.effective_soft_particle_fade_range(),
                depth_offset_type: particle.depth_offset_type,
                depth_offset,
                powder_single: false,
                windmill_uv_type: 0,
                disc: None,
                polygon: None,
                laser: None,
                line: None,
                polyline: None,
                decal: None,
                parent_basis: facing_basis,
                movement_direction: [0.0; 3],
                facing_parent_basis: emitter_basis,
                position,
                size: scale,
                orientation: quat_from_euler(2, slot.angles(sub_age)),
                rotation_direction_base: facing,
                color,
                draw_layer: self.file.global.draw_layer,
                soft_key_offset: self.file.global.soft_key_offset,
                draw_priority: i32::from(particle.draw_priority as i8),
                draw_order: None,
                pivot: simple.pivot,
                texture_indexes: tex.texture_indexes,
                texture_uv_sets: tex.texture_uv_sets,
                combine_mode_tc1: tex.combine_mode_tc1,
                combine_modes: tex.combine_modes,
                color_to_alpha: tex.color_to_alpha,
                uv_origins: {
                    let mut origins = tex.uv_origins;
                    origins[0] = uv_origin;
                    origins
                },
                uv_scales: {
                    let mut scales = tex.uv_scales;
                    scales[0] = uv_scale;
                    scales
                },
                // Powder/Windmill use their dedicated raw-UV shader path and
                // configure no Apricot UvSet records.
                uv_by_pixel_position: [false; 4],
                uv_rotations: [0.0; 4],
                texture_borders: tex.texture_borders,
                texture_filters: tex.texture_filters,
                texture1_is_shape_mask: tex.texture1_is_shape_mask,
                texture1_enabled: tex.texture1_enabled,
                texture1_use_screen_copy: tex.texture1_use_screen_copy,
                draw_mode: particle.draw_mode,
                depth_test: particle.depth_test,
                depth_write: particle.depth_write,
                cull_mode: particle.culling_type,
                texture_distortion_index: tex.texture_distortion_index,
                distortion_power: tex.distortion_power,
                distortion_targets: tex.distortion_targets,
                uvd_origin: tex.uvd_origin,
                uvd_scale: tex.uvd_scale,
                uvd_rotation: tex.uvd_rotation,
                uvd_by_pixel_position: false,
                distortion_uv_set: tex.distortion_uv_set,
                distortion_borders: tex.distortion_borders,
                distortion_filter: tex.distortion_filter,
                texture_palette_index: tex.texture_palette_index,
                palette_offset: tex.palette_offset,
                palette_border: tex.palette_border,
                palette_filter: tex.palette_filter,
            });
        }
    }
}

fn powder_emit_vertex(
    model: &crate::avfx::VfxModelGeometry,
    ordinal: u64,
) -> Option<&crate::avfx::VfxEmitVertex> {
    if model.emit_vertex_numbers.is_empty() {
        return None;
    }
    let number = model
        .emit_vertex_numbers
        .get((ordinal % model.emit_vertex_numbers.len() as u64) as usize)?;
    model.emit_vertices.get(*number as u16 as usize)
}

fn model_number_random_base(value: i32, random_type: i32, seed: u64) -> i32 {
    if value <= 0 || !matches!(random_type, 0..=2) {
        return 0;
    }
    let mut random = SplitMix64::seeded(seed, 0, 0, 0x4d4e_5256);
    model_number_random_base_from_unit(value, random_type, random.next_u64() as u16)
}

fn model_number_random_base_from_unit(value: i32, random_type: i32, unit: u16) -> i32 {
    if value <= 0 || !matches!(random_type, 0..=2) {
        return 0;
    }
    let unit = i32::from(unit);
    let unsigned = |maximum: i32| maximum.wrapping_add(1).wrapping_mul(unit) / 65_536;
    match random_type {
        0 => unsigned(value.wrapping_mul(2)).wrapping_sub(value),
        1 => unsigned(value),
        2 => unsigned(value).wrapping_sub(value),
        _ => 0,
    }
}

fn cvttss2si(value: f32) -> i32 {
    if (-2147483648.0..2147483648.0).contains(&value) {
        value as i32
    } else {
        i32::MIN
    }
}

fn model_particle_value(particle: &AvfxParticle, ages: CurveAges, seed: u64) -> i32 {
    let AvfxParticleData::Model {
        model_number_random_value,
        model_number_random_type,
        ..
    } = &particle.data
    else {
        return 0;
    };
    let random_base =
        model_number_random_base(*model_number_random_value, *model_number_random_type, seed);
    model_particle_value_with_base(particle, ages, random_base)
}

fn model_particle_value_with_base(
    particle: &AvfxParticle,
    ages: CurveAges,
    random_base: i32,
) -> i32 {
    let AvfxParticleData::Model {
        animation_number, ..
    } = &particle.data
    else {
        return 0;
    };
    let number = animation_number
        .as_ref()
        .map(|curve| curve.value_at(ages.local, ages.total, 0.0))
        .unwrap_or(0.0);
    cvttss2si(number).wrapping_add(random_base)
}

fn model_index_from_value(particle: &AvfxParticle, value: i32) -> i32 {
    let AvfxParticleData::Model { model_indexes, .. } = &particle.data else {
        return -1;
    };
    if model_indexes.is_empty() {
        return -1;
    }
    let selected = model_indexes[value.unsigned_abs() as usize % model_indexes.len()];
    i32::from(selected as i8)
}

fn model_particle_index(particle: &AvfxParticle, ages: CurveAges, seed: u64) -> i32 {
    let AvfxParticleData::Model {
        model_number_random_interval,
        model_indexes,
        ..
    } = &particle.data
    else {
        return -1;
    };
    let Some(&fallback) = model_indexes.first() else {
        return -1;
    };

    // Model constructor 0x1403edb17 draws the First random base once. Draw
    // 0x1403ee505 then truncates NoAn, adds that base and maps its unsigned
    // absolute value through MdNo. Our stable instance seed preserves the
    // range and lifetime but not the client's process-wide TLS draw order.
    let selected = if *model_number_random_interval <= 0 {
        return model_index_from_value(particle, model_particle_value(particle, ages, seed));
    } else {
        fallback
    };
    i32::from(selected as i8)
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
    uv_by_pixel_position: [bool; 4],
    /// 各层基底 UV 绕 (0.5, 0.5) 的旋转（UvSet `Rot` + `RotR` 随机，弧度）。
    uv_rotations: [f32; 4],
    texture_borders: [[i32; 2]; 4],
    texture_filters: [i32; 4],
    texture_normal_index: i32,
    normal_uv_set: i32,
    normal_uv_origin: [f32; 2],
    normal_uv_scale: [f32; 2],
    normal_uv_rotation: f32,
    normal_uv_by_pixel_position: bool,
    normal_texture_borders: [i32; 2],
    normal_texture_filter: i32,
    normal_power: f32,
    reflection_enabled: bool,
    reflection_use_screen_copy: bool,
    reflection_texture_index: i32,
    reflection_texture_filter: i32,
    reflection_calculate_color: i32,
    reflection_rate: f32,
    reflection_power: f32,
    texture1_is_shape_mask: bool,
    texture1_enabled: bool,
    texture1_use_screen_copy: bool,
    texture_distortion_index: i32,
    distortion_power: f32,
    distortion_targets: u32,
    distortion_uv_set: i32,
    uvd_origin: [f32; 2],
    uvd_scale: [f32; 2],
    uvd_rotation: f32,
    uvd_by_pixel_position: bool,
    distortion_borders: [i32; 2],
    distortion_filter: i32,
    texture_palette_index: i32,
    palette_offset: f32,
    palette_border: i32,
    palette_filter: i32,
}

impl ResolvedTexture {
    fn of(particle: &AvfxParticle, age: f32, seed: u64) -> Self {
        Self::at(particle, age, age, seed)
    }

    fn at(particle: &AvfxParticle, age: f32, draw_age: f32, seed: u64) -> Self {
        Self::at_times(
            particle,
            CurveAges {
                local: age,
                total: age,
            },
            CurveAges {
                local: draw_age,
                total: draw_age,
            },
            seed,
        )
    }

    fn at_times(particle: &AvfxParticle, ages: CurveAges, draw_ages: CurveAges, seed: u64) -> Self {
        Self::at_times_with_client(particle, ages, draw_ages, seed, None)
    }

    fn at_times_with_client(
        particle: &AvfxParticle,
        ages: CurveAges,
        draw_ages: CurveAges,
        seed: u64,
        client: Option<particle_curves::VfxClientParticleTextureValues>,
    ) -> Self {
        let uv = |index: i32| -> ([f32; 2], [f32; 2], f32, bool) {
            let index = (index & 7) as usize;
            particle
                .uv_sets
                .get(index)
                .map(|set| {
                    // Shared references to one UvSt reuse its values; different
                    // sets have independent random curves (AVFXTools UV1..UV4).
                    let seed = SplitMix64::seeded(seed, index as u64, 0x5556, 0).next_u64();
                    (
                        eval2_seeded_at(&set.scroll, ages, 0.0, seed ^ 0x5C01),
                        eval2_seeded_at(&set.scale, ages, 1.0, seed ^ 0x5C1E),
                        // UvSet Rot + RotR 随机（弧度，绕 0.5 中心）。
                        curve_value_seeded_at(
                            &set.rotation,
                            &set.rotation_random,
                            ages,
                            0.0,
                            seed ^ 0xA07,
                        ),
                        set.calculate_uv == 1,
                    )
                })
                .unwrap_or(([0.0, 0.0], [1.0, 1.0], 0.0, false))
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
        let mut uv_by_pixel_position = [false; 4];
        let mut uv_rotations = [0.0; 4];
        let mut texture_borders = [[0; 2]; 4];
        let mut texture_filters = [1; 4];
        let mut texture1_is_shape_mask = false;
        for (i, tc) in tcs.iter().enumerate() {
            let Some(tc) = tc.filter(|t| t.enabled) else {
                continue;
            };
            let uv_set_index = tc.uv_set_index & 7;
            texture_uv_sets[i] = uv_set_index;
            texture_filters[i] = tc.texture_filter;
            texture_indexes[i] = if i == 0 {
                client
                    .and_then(|c| c.texture_index)
                    .unwrap_or_else(|| tc1_texture_index(tc, ages, seed))
            } else {
                tc.texture_index
            };
            // UvSet 动画对无贴图层也保留（翻页覆盖以它为基底）。
            let (origin, scale, rot, by_pixel_position) = uv(uv_set_index);
            uv_origins[i] = origin;
            uv_scales[i] = scale;
            uv_rotations[i] = rot;
            uv_by_pixel_position[i] = by_pixel_position;
            if texture_indexes[i] < 0 && !(i == 0 && matches!(texture_indexes[i], -2 | -3 | -5)) {
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
        let (
            texture_distortion_index,
            distortion_power,
            distortion_targets,
            uvd,
            dborders,
            duv_set,
        ) = match td {
            Some(td) => {
                let uv_set_index = td.uv_set_index & 7;
                let uvd = uv(uv_set_index);
                let mut targets = 0u32;
                for (i, tc) in tcs.iter().enumerate() {
                    let Some(tc) = tc.filter(|t| t.enabled) else {
                        continue;
                    };
                    // bT(i+1) 扭曲 UvSet i。
                    let hit = (0..4).any(|u| tc.uv_set_index & 7 == u as i32 && td.target_uv[u]);
                    if hit {
                        targets |= 1 << i;
                    }
                }
                (
                    td.texture_index,
                    {
                        let power = td.power.value_at(draw_ages.local, draw_ages.total, 0.0);
                        if particle.particle_type == Some(ParticleType::Quad) {
                            quantize_quad_distortion(power)
                        } else {
                            power
                        }
                    },
                    targets,
                    uvd,
                    [td.texture_border_u, td.texture_border_v],
                    uv_set_index,
                )
            }
            None => (-1, 0.0, 0, ([0.0, 0.0], [1.0, 1.0], 0.0, false), [0, 0], 0),
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
        let tp = particle
            .texture_palette
            .as_ref()
            .filter(|tp| tp.enabled && tp.texture_index >= 0);
        let (texture_palette_index, palette_offset, palette_border, palette_filter) = tp
            .map(|tp| {
                (
                    tp.texture_index,
                    client.and_then(|c| c.palette_offset).map_or_else(
                        || {
                            particle_palette_offset(
                                particle,
                                curve_value_seeded_at(
                                    &tp.offset,
                                    &tp.offset_random,
                                    draw_ages,
                                    0.0,
                                    seed ^ 0x5041_4C45,
                                ),
                            )
                        },
                        |value| value,
                    ),
                    tp.texture_border,
                    tp.texture_filter,
                )
            })
            .unwrap_or((-1, 0.0, 0, 1));
        let tn = particle
            .texture_normal
            .as_ref()
            .filter(|tn| tn.enabled && tn.texture_index >= 0);
        let (
            texture_normal_index,
            normal_uv_set,
            normal_uv,
            normal_texture_borders,
            normal_texture_filter,
            normal_power,
        ) = tn
            .map(|tn| {
                let uv_set_index = tn.uv_set_index & 7;
                (
                    tn.texture_index,
                    uv_set_index,
                    uv(uv_set_index),
                    [tn.texture_border_u, tn.texture_border_v],
                    tn.texture_filter,
                    tn.power.value_at(draw_ages.local, draw_ages.total, 0.0),
                )
            })
            .unwrap_or((-1, 0, ([0.0; 2], [1.0; 2], 0.0, false), [0; 2], 1, 0.0));
        let tr = particle.texture_reflection.as_ref().filter(|tr| tr.enabled);
        let (
            reflection_enabled,
            reflection_use_screen_copy,
            reflection_texture_index,
            reflection_texture_filter,
            reflection_calculate_color,
            reflection_rate,
            reflection_power,
        ) = tr
            .map(|tr| {
                (
                    true,
                    tr.use_screen_copy,
                    tr.texture_index,
                    tr.texture_filter,
                    tr.calculate_color,
                    tr.rate.value_at(draw_ages.local, draw_ages.total, 0.0),
                    tr.power.value_at(draw_ages.local, draw_ages.total, 0.0),
                )
            })
            .unwrap_or((false, false, -1, 1, 0, 0.0, 0.0));
        Self {
            texture_indexes,
            texture_uv_sets,
            combine_mode_tc1,
            combine_modes,
            color_to_alpha,
            uv_origins,
            uv_scales,
            uv_by_pixel_position,
            uv_rotations,
            texture_borders,
            texture_filters,
            texture_normal_index,
            normal_uv_set,
            normal_uv_origin: normal_uv.0,
            normal_uv_scale: normal_uv.1,
            normal_uv_rotation: normal_uv.2,
            normal_uv_by_pixel_position: normal_uv.3,
            normal_texture_borders,
            normal_texture_filter,
            normal_power,
            reflection_enabled,
            reflection_use_screen_copy,
            reflection_texture_index,
            reflection_texture_filter,
            reflection_calculate_color,
            reflection_rate,
            reflection_power,
            texture1_is_shape_mask,
            texture1_enabled: tcs[0].is_some_and(|tc| tc.enabled),
            texture1_use_screen_copy: tcs[0].is_some_and(|tc| tc.use_screen_copy),
            texture_distortion_index,
            distortion_power,
            distortion_targets,
            distortion_uv_set: duv_set,
            uvd_origin: uvd.0,
            uvd_scale: uvd.1,
            uvd_rotation: uvd.2,
            uvd_by_pixel_position: uvd.3,
            distortion_borders: dborders,
            distortion_filter: td.map_or(1, |td| td.texture_filter),
            texture_palette_index,
            palette_offset,
            palette_border,
            palette_filter,
        }
    }
}

fn tc1_texture_index(
    texture: &crate::avfx::AvfxParticleTexture,
    ages: CurveAges,
    seed: u64,
) -> i32 {
    if let Some(source) = texture.tc1_builtin_source() {
        return source;
    }
    let animated = texture.tc1_has_animated_selection();
    if !animated || texture.texture_list.is_empty() {
        return texture.tc1_static_texture();
    }
    let list_len = i32::try_from(texture.texture_list.len() & 0xff).unwrap_or(0);
    if list_len == 0 {
        return -1;
    }
    let value = texture
        .tex_n
        .as_ref()
        .map_or(0.0, |curve| curve.value_at(ages.local, ages.total, 0.0))
        + texture.tex_n_random.as_ref().map_or(0.0, |curve| {
            curve_random_value_at(curve, ages, seed ^ 0x5458_4E52)
        });
    // The client stores the TLst count in 8 bits, initializes this positive
    // bias while parsing, then truncates the combined TxN/TxNR result once.
    let biased = list_len.wrapping_shl(16).wrapping_add(cvttss2si(value));
    let slot = biased % list_len;
    let Some(&selected) = usize::try_from(slot)
        .ok()
        .and_then(|slot| texture.texture_list.get(slot))
    else {
        return -1;
    };
    i32::from(selected as u8 as i8)
}

fn quantize_powder_color(color: [f32; 4]) -> [f32; 4] {
    color.map(|value| (value * 1000.0).clamp(0.0, 10000.0).trunc() * 0.001)
}

fn quantize_quad_distortion(power: f32) -> f32 {
    // Client 0x1403e7223: f32 * 255, CVTTSS2SI, low byte, UNORM at input.
    // CVTTSS2SI returns INT_MIN on overflow/NaN, whose low byte is zero.
    let scaled = power * 255.0;
    let integer = if (-2147483648.0..2147483648.0).contains(&scaled) {
        scaled as i32
    } else {
        i32::MIN
    };
    f32::from(integer as u8) / 255.0
}

fn particle_palette_offset(particle: &AvfxParticle, value: f32) -> f32 {
    // Model/LightModel +3e0 writes a float uniform (3e73c0); vertex-backed
    // shapes use +3c8 and its UNORM8 byte (3e7250).
    if matches!(
        particle.particle_type,
        Some(ParticleType::Model | ParticleType::LightModel)
    ) {
        value
    } else {
        quantize_palette_offset(value)
    }
}

fn quantize_palette_offset(offset: f32) -> f32 {
    // Client 0x1403e7288: (POff + POfR) * 255, CVTTSS2SI, low byte, UNORM input.
    let scaled = offset * 255.0;
    let integer = if (-2147483648.0..2147483648.0).contains(&scaled) {
        scaled.trunc() as i32
    } else {
        i32::MIN
    };
    (integer as u8) as f32 / 255.0
}

/// 颜色帧簿求值：`frames` 为时间点，`colors` 逐段线性插值。
#[cfg(test)]
fn sample_flipbook(colors: &[[u8; 4]; 4], frames: &[i16; 4], age: f32) -> [f32; 4] {
    sample_flipbook_offset(colors, frames, age, 0)
}

fn sample_flipbook_offset(
    colors: &[[u8; 4]; 4],
    frames: &[i16; 4],
    age: f32,
    offset: i32,
) -> [f32; 4] {
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
    if age >= (frames[3] as i32 + offset) as f32 {
        return to_f32(colors[3]);
    }
    for i in 0..3 {
        let t0 = (frames[i] as i32 + offset) as f32;
        let t1 = (frames[i + 1] as i32 + offset) as f32;
        if age < t1 {
            // Client selects the first strictly later frame, skipping repeated
            // keys, then interpolates byte values before dividing by 255.
            let local = (age - t0) / (t1 - t0);
            return std::array::from_fn(|channel| {
                lerp(
                    colors[i][channel] as f32,
                    colors[i + 1][channel] as f32,
                    local,
                ) / 255.0
            });
        }
    }
    to_f32(colors[3])
}

/// Count selection is independent of event routing and target lifetime.
/// Client CVTTSS2SI returns INT_MIN for nonfinite/out-of-range CrC, then skips it.
fn emitter_create_count(emitter: &AvfxEmitter, item: &AvfxEmitterItem, age: f32) -> u64 {
    emitter_create_count_at(emitter, item, age, age)
}

fn emitter_create_count_at(
    emitter: &AvfxEmitter,
    item: &AvfxEmitterItem,
    local_age: f32,
    total_age: f32,
) -> u64 {
    let count = match item.create_time {
        0 => {
            let value = emitter.create_count.value_at(local_age, total_age, 0.0);
            if !(0.0..2147483648.0).contains(&value) {
                return 0;
            }
            value as i32
        }
        1 | 2 => item.create_count,
        _ => 0,
    };
    count.max(0) as u64
}

fn emitter_create_count_at_seeded(
    emitter: &AvfxEmitter,
    item: &AvfxEmitterItem,
    local_age: f32,
    total_age: f32,
    event: u64,
    seed: u64,
) -> u64 {
    if item.create_time != 0 {
        return emitter_create_count_at(emitter, item, local_age, total_age);
    }
    let random = &emitter.create_count_random;
    let random_seed = if matches!(random.random_type & 7, 0..=2) {
        seed
    } else {
        seed ^ event
    };
    let value = emitter.create_count.value_at(local_age, total_age, 0.0)
        + curve_random_value_at(
            random,
            CurveAges {
                local: local_age,
                total: total_age,
            },
            random_seed,
        );
    if !(0.0..2147483648.0).contains(&value) {
        return 0;
    }
    (value as i32).max(0) as u64
}

fn emitter_can_create(emitter: &AvfxEmitter, item: &AvfxEmitterItem) -> bool {
    if item.generate_delay > 0 {
        // Positive delay suppresses the immediate initialization/finish batch.
        // Initialization needs a helper; finish helpers cannot attach directly
        // to the already dead emitter when PrLk is absent.
        let no_helper = match item.create_time {
            1 => item.create_count <= 1 && item.generate_delay_by_one,
            2 => item.create_count <= 1 || item.parameter_link == -1,
            _ => false,
        };
        if no_helper {
            return false;
        }
    }
    if item.create_time == 0 {
        let keys = &emitter.create_count.keys;
        if keys
            .first()
            .is_some_and(|first| keys.iter().any(|key| key.z.to_bits() != first.z.to_bits()))
        {
            return true;
        }
        // A zero CrC baseline can still produce creations through a nonzero
        // CrCR offset. Do not prune the event stream before seeded count
        // selection has had a chance to evaluate that random curve.
        if emitter
            .create_count_random
            .keys
            .iter()
            .any(|key| key.z != 0.0)
        {
            return true;
        }
    }
    // Avoid replaying empty child histories, even for queries far in the future.
    emitter_create_count(emitter, item, 0.0) != 0
}

fn last_creation_age(item: &AvfxEmitterItem, max_alive: Option<f32>) -> Option<f32> {
    let delay = item.generate_delay.max(0) as f32;
    match item.create_time {
        0 => max_alive.map(|age| age.max(0.0)),
        1 => Some(delay),
        2 => max_alive.map(|age| age.max(0.0) + delay),
        _ => None,
    }
}

/// Latch the next interval at each birth, so querying a later frame cannot rewrite
/// earlier events. The client's exact sampling rule for animated CrI is unverified.
#[cfg(test)]
fn emitter_events(
    emitter: &AvfxEmitter,
    oldest_birth: f32,
    until: f32,
) -> impl Iterator<Item = (u64, f32)> + '_ {
    clock_emitter_events(
        emitter,
        InstanceClock::root(emitter, -1.0),
        oldest_birth,
        until,
        0,
    )
}

fn item_events(
    emitter: &AvfxEmitter,
    clock: InstanceClock,
    create_time: i32,
    oldest_birth: f32,
    until: f32,
    interval_seed: u64,
) -> impl Iterator<Item = (u64, f32)> + '_ {
    let once = match create_time {
        1 => Some((0, -clock.warmup)),
        2 => clock
            .finish_frame()
            .filter(|&finish| until > finish)
            .map(|finish| (0, finish)),
        _ => None,
    };
    let periodic = (create_time == 0)
        .then(|| clock_emitter_events(emitter, clock, oldest_birth, until, interval_seed));
    once.into_iter().chain(periodic.into_iter().flatten())
}

fn clock_emitter_events(
    emitter: &AvfxEmitter,
    clock: InstanceClock,
    oldest_birth: f32,
    until: f32,
    interval_seed: u64,
) -> impl Iterator<Item = (u64, f32)> + '_ {
    let until = f64::from(clock.elapsed(until)) * f64::from(clock.rate);
    let interval_at = move |elapsed: f64, event: u64| {
        let frame = (elapsed / f64::from(clock.rate)) as f32 - clock.warmup;
        let ages = clock.ages(frame);
        let random = &emitter.create_interval_random;
        let seed = if matches!(random.random_type & 7, 0..=2) {
            interval_seed
        } else {
            interval_seed ^ event
        };
        emitter
            .create_interval
            .value_at(ages.local, ages.total, 0.0)
            + curve_random_value_at(random, ages, seed)
    };
    let initial_interval = interval_at(0.0, 0);
    let first = || {
        let constant = emitter
            .create_interval
            .keys
            .iter()
            .all(|key| key.z == emitter.create_interval.value(0.0, 0.0))
            && emitter
                .create_interval_random
                .keys
                .iter()
                .all(|key| key.z == 0.0);
        let oldest = f64::from((oldest_birth + clock.warmup).max(0.0)) * f64::from(clock.rate);
        if constant {
            return seek_constant_event((0, 0.0), initial_interval, oldest);
        }
        let curve = &emitter.create_interval;
        let random = &emitter.create_interval_random;
        let random_is_zero = random.keys.is_empty()
            || random.random_type & 7 >= 6
            || random.keys.iter().all(|key| key.z == 0.0);
        let random_tail = if random_is_zero {
            Some(0.0)
        } else if matches!(random.random_type & 7, 0..=2)
            && random.post_behavior == crate::avfx::BEHAVIOR_CONST
            && random
                .keys
                .windows(2)
                .all(|keys| keys[0].scalar_time() <= keys[1].scalar_time())
        {
            random.keys.last().map(|key| f64::from(key.scalar_time()))
        } else {
            None
        };
        let constant_tail = if !clock.loops()
            && curve.post_behavior == crate::avfx::BEHAVIOR_CONST
            && curve
                .keys
                .windows(2)
                .all(|keys| keys[0].scalar_time() <= keys[1].scalar_time())
        {
            curve.keys.last().zip(random_tail).map(|(key, random_end)| {
                // Initialization reads CrI before an assigned StFr age.
                // Consume that first interval before seeking the tail.
                (f64::from(key.scalar_time()).max(random_end)
                    - f64::from(clock.motion_range(-clock.warmup)[0]))
                .max(0.0)
                .max(f64::from(initial_interval))
            })
        } else {
            None
        };
        let mut first = (0, 0.0);
        if let Some(tail_start) = constant_tail {
            // Replay only the animated prefix to establish the actual event
            // phase. A keyframe need not itself be a birth event.
            while first.1 < oldest.min(tail_start) {
                let interval = interval_at(first.1, first.0);
                let Some(next) = next_emitter_event(first, interval) else {
                    return first;
                };
                if next.1 > oldest {
                    return first;
                }
                first = next;
            }
            if first.1 >= tail_start {
                first = seek_constant_event(first, interval_at(first.1, first.0), oldest);
            }
        }
        first
    };
    std::iter::successors(Some(first()), move |&(index, birth)| {
        let interval = interval_at(birth, index);
        next_emitter_event((index, birth), interval)
    })
    .take_while(move |&(_, birth)| birth <= until)
    .map(move |(index, birth)| (index, (birth / f64::from(clock.rate)) as f32 - clock.warmup))
}

fn next_emitter_event((index, birth): (u64, f64), interval: f32) -> Option<(u64, f64)> {
    let next_birth = birth + f64::from(interval);
    if !interval.is_finite() || interval <= 0.0 || !next_birth.is_finite() || next_birth <= birth {
        return None;
    }
    Some((index.checked_add(1)?, next_birth))
}

/// Retain the last event at or before `oldest`, with the same f64 additions as
/// full history enumeration. Multiplying the interval by the event number can
/// drift once additions round, changing both birth boundaries and random seeds.
fn seek_constant_event(mut event: (u64, f64), interval: f32, oldest: f64) -> (u64, f64) {
    while event.1 < oldest {
        let Some(next) = next_emitter_event(event, interval) else {
            break;
        };
        if next.1 > oldest {
            break;
        }
        event = next;

        // Within a binary exponent range, f64 has fixed spacing. After a
        // possible ties-to-even transition, repeated addition has a fixed
        // stride. Never jump across the next power of two, where spacing changes.
        let end = f64::from_bits(((event.1.to_bits() >> 52) + 1) << 52);
        let next = event.1 + f64::from(interval);
        let following = next + f64::from(interval);
        let stride = next - event.1;
        if stride <= 0.0 {
            break;
        }
        if next >= end || following >= end || following - next != stride {
            continue;
        }
        let limit = oldest.min(end.next_down());
        let mut jumps = (((limit - event.1) / stride).floor() as u64).min(u64::MAX - event.0);
        let mut birth = event.1 + jumps as f64 * stride;
        if birth > limit && jumps > 0 {
            jumps -= 1;
            birth = event.1 + jumps as f64 * stride;
        }
        event = (event.0 + jumps, birth);
    }
    event
}

/// Life 随机幅度的预览规则；其存储和量化不能套用标量曲线规则。
fn roll_amplitude(rng: &mut SplitMix64, random_type: u32, amp: f32) -> f32 {
    match random_type {
        1 | 4 => rng.next_f32() * amp,
        2 | 5 => -rng.next_f32() * amp,
        _ => (rng.next_f32() * 2.0 - 1.0) * amp,
    }
}

/// 幅度先插值，First 共用实例系数；Always 的完整更新时序仍按整数帧近似。
#[cfg(test)]
fn curve_random_value(curve: &crate::avfx::AvfxCurve, time: f32, seed: u64) -> f32 {
    curve_random_value_at(
        curve,
        CurveAges {
            local: time,
            total: time,
        },
        seed,
    )
}

fn curve_random_value_at(curve: &crate::avfx::AvfxCurve, ages: CurveAges, seed: u64) -> f32 {
    if curve.keys.is_empty() || curve.random_type & 7 >= 6 {
        return 0.0;
    }
    let time = if curve.post_behavior & 3 >= crate::avfx::BEHAVIOR_ADD {
        ages.total
    } else {
        ages.local
    };
    // Continuous sampling has no callback history; integer age approximates
    // the draw index for Always curves outside staged gravity.
    random_curve_offset(
        curve.random_type,
        curve.value_at(ages.local, ages.total, 0.0),
        time.max(0.0) as u64,
        seed,
    )
}

fn random_curve_offset(random_type: u32, amplitude: f32, draw_index: u64, seed: u64) -> f32 {
    let draw_index = if matches!(random_type & 7, 3..=5) {
        draw_index
    } else {
        0
    };
    let draw = SplitMix64::seeded(seed, 0, draw_index, 0xB7E1).next_u64() as u16;
    scalar_random::offset(random_type, amplitude, draw)
}

/// 主曲线 + 随机偏移；无随机键或禁用模式时直接返回主值。
fn curve_value_seeded_at(
    main: &crate::avfx::AvfxCurve,
    random: &crate::avfx::AvfxCurve,
    ages: CurveAges,
    default: f32,
    seed: u64,
) -> f32 {
    let base = main.value_at(ages.local, ages.total, default);
    if random.keys.is_empty() || random.random_type & 7 >= 6 {
        return base;
    }
    let offset = curve_random_value_at(random, ages, seed);
    if main.keys.is_empty() && default.to_bits() == 0 {
        offset
    } else {
        offset + base
    }
}

/// 三轴主/随机曲线分别按 ACT/ACTR 连接，再叠加随机偏移。
fn eval3_seeded_at(
    curve3: &crate::avfx::AvfxCurve3Axis,
    ages: CurveAges,
    default: f32,
    seed: u64,
) -> [f32; 3] {
    let base = curve3.evaluate_at(ages.local, ages.total, default);
    let randoms = [
        curve3.random_x.as_ref(),
        curve3.random_y.as_ref(),
        curve3.random_z.as_ref(),
    ];
    if !vector_curves::random_reader_enabled(curve3) {
        return base;
    }
    let mut extra = [0.0; 3];
    for (axis, random) in randoms.iter().enumerate() {
        if let Some(curve) = random {
            extra[axis] = curve_random_value_at(curve, ages, seed ^ (axis as u64 + 1) * 0x9E37);
        }
    }
    crate::avfx::connect_axes3(curve3.axis_connect_random, &mut extra);
    [base[0] + extra[0], base[1] + extra[1], base[2] + extra[2]]
}

/// 两轴曲线求值 + 随机轴（UvSt 的 Scl/Scr 随机）。
#[cfg(test)]
fn eval2_seeded(
    curve2: &crate::avfx::AvfxCurve2Axis,
    time: f32,
    default: f32,
    seed: u64,
) -> [f32; 2] {
    eval2_seeded_at(
        curve2,
        CurveAges {
            local: time,
            total: time,
        },
        default,
        seed,
    )
}

fn eval2_seeded_at(
    curve2: &crate::avfx::AvfxCurve2Axis,
    ages: CurveAges,
    default: f32,
    seed: u64,
) -> [f32; 2] {
    let base = curve2.evaluate_at(ages.local, ages.total, default);
    let mut extra = [0.0; 2];
    if let Some(curve) = &curve2.random_x {
        extra[0] = curve_random_value_at(curve, ages, seed ^ 0x11);
    }
    if let Some(curve) = &curve2.random_y {
        extra[1] = curve_random_value_at(curve, ages, seed ^ 0x22);
    }
    match curve2.axis_connect_random {
        1 if curve2.random_x.is_some() => extra[1] = extra[0],
        2 if curve2.random_y.is_some() => extra[0] = extra[1],
        _ => {}
    }
    [base[0] + extra[0], base[1] + extra[1]]
}

fn loop_age(age: f32, start: i32, end: i32) -> f32 {
    if end > start && age >= end as f32 {
        start as f32 + (age - start as f32).rem_euclid((end as f32) - start as f32)
    } else {
        age
    }
}

/// Client color order: base + random, channel scales, then RGB brightness.
#[cfg(test)]
fn color_curve_seeded(cc: &crate::avfx::AvfxColorCurve, age: f32, seed: u64) -> [f32; 4] {
    color_curve_seeded_at(
        cc,
        CurveAges {
            local: age,
            total: age,
        },
        seed,
    )
}

fn color_curve_seeded_at(cc: &crate::avfx::AvfxColorCurve, ages: CurveAges, seed: u64) -> [f32; 4] {
    cc.compose_at(ages.local, ages.total, true, [1.0; 4], |channel, curve| {
        (curve.random_type & 7 < 6)
            .then(|| curve_random_value_at(curve, ages, seed ^ (0xC010 + channel as u64)))
    })
}

fn model_fresnel(
    data: &AvfxParticleData,
    cached_ages: CurveAges,
    draw_ages: CurveAges,
    seed: u64,
) -> Option<VfxFresnel> {
    let AvfxParticleData::Model {
        fresnel_type: kind @ 1..=3,
        fresnel_curve,
        fresnel_curve_random,
        fresnel_rotation,
        color_begin,
        color_end,
        ..
    } = data
    else {
        return None;
    };
    // FrC is evaluated during draw; FrRt and both colors are cached by +f0.
    let exponent = fresnel_curve
        .as_ref()
        .map_or(0.0, |c| c.value_at(draw_ages.local, draw_ages.total, 0.0))
        + fresnel_curve_random
            .as_ref()
            .map_or(0.0, |c| curve_random_value_at(c, draw_ages, seed ^ 0xF2C0));
    let rotation = fresnel_rotation.as_ref().map_or([0.0; 3], |c| {
        eval3_seeded_at(c, cached_ages, 0.0, seed ^ 0xF207)
    });
    Some(VfxFresnel {
        kind: *kind,
        direction: fresnel_axis(*kind, rotation),
        exponent,
        color_begin: color_curve_seeded_at(color_begin, cached_ages, seed ^ 0xF2B0),
        color_end: color_curve_seeded_at(color_end, cached_ages, seed ^ 0xF2E0),
    })
}

fn fresnel_axis(kind: i32, [x, y, z]: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    // Client 2026.09.15: the two axis modes use different reference axes.
    // Mode 3 is subsequently multiplied by the model's linear transform and normalized.
    match kind {
        2 => [cx * sz - sx * sy * cz, -cx * cz - sx * sy * sz, -sx * cy],
        3 => [-cx * sy * cz - sx * sz, sx * cz - cx * sy * sz, -cx * cy],
        _ => [0.0; 3],
    }
}

fn particle_orientation(ctx: &SpawnContext, particle: &AvfxParticle, age: f32) -> [f32; 4] {
    particle_orientation_at(
        ctx,
        particle,
        CurveAges {
            local: age,
            total: age,
        },
    )
}

fn particle_orientation_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
) -> [f32; 4] {
    if particle.rotation_direction_base == crate::avfx::rotation_direction_base::MOVE_DIRECTION {
        let mut z = particle_euler_at(ctx, particle, ages)[2];
        if ctx.motion.has_optional_components() {
            z += ctx.parent_components.at(ctx.age).rotation[2];
        }
        return quat_from_euler(particle.rotation_order, [0.0, 0.0, z]);
    }
    if particle.is_billboard() {
        let mut euler = particle_euler_at(ctx, particle, ages);
        // RBDT 5/6/9 rebuild the drawing basis using the parent-Z getter.
        euler[2] += ctx.parent_components.at(ctx.age).rotation[2];
        quat_from_euler(particle.rotation_order, euler)
    } else {
        particle_coordinate_orientation_at(ctx, particle, ages)
    }
}

fn particle_coordinate_orientation(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    age: f32,
) -> [f32; 4] {
    particle_coordinate_orientation_at(
        ctx,
        particle,
        CurveAges {
            local: age,
            total: age,
        },
    )
}

fn particle_coordinate_orientation_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
) -> [f32; 4] {
    let mut euler = particle_euler_at(ctx, particle, ages);
    if ctx.motion.has_optional_components() {
        let inherited = ctx.parent_components.at(ctx.age);
        euler = std::array::from_fn(|axis| euler[axis] + inherited.rotation[axis]);
    }
    quat_from_euler(particle.rotation_order, euler)
}

fn particle_coordinate_scale(ctx: &SpawnContext, particle: &AvfxParticle, age: f32) -> [f32; 3] {
    particle_coordinate_scale_at(
        ctx,
        particle,
        CurveAges {
            local: age,
            total: age,
        },
    )
}

fn particle_xyz_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
    channel: usize,
) -> [f32; 3] {
    ctx.cached_xyz.map_or_else(
        || {
            let curves = [&particle.scale, &particle.rotation, &particle.position];
            eval3_seeded_at(
                curves[channel],
                ages,
                if channel == 0 { 1.0 } else { 0.0 },
                ctx.seed ^ [0x51C1, 0x307A, 0x9051][channel],
            )
        },
        |cache| cache[channel],
    )
}

fn particle_coordinate_scale_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
) -> [f32; 3] {
    let local = particle_xyz_at(ctx, particle, ages, 0);
    if ctx.motion.has_optional_components() {
        let inherited = ctx.parent_components.at(ctx.age);
        std::array::from_fn(|axis| local[axis] * inherited.scale[axis])
    } else {
        local
    }
}

fn particle_euler_at(ctx: &SpawnContext, particle: &AvfxParticle, ages: CurveAges) -> [f32; 3] {
    // Client VR curves redirect injection motion; they do not add drawing angles.
    let rotation = particle_xyz_at(ctx, particle, ages, 1);
    std::array::from_fn(|axis| rotation[axis] + ctx.creation_angle[axis])
}

fn particle_scale_and_parent(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    age: f32,
    parent: EmitterBase,
) -> ([f32; 3], [[f32; 3]; 3]) {
    particle_scale_and_parent_at(
        ctx,
        particle,
        CurveAges {
            local: age,
            total: age,
        },
        parent,
    )
}

fn particle_scale_and_parent_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
    parent: EmitterBase,
) -> ([f32; 3], [[f32; 3]; 3]) {
    let (scale, basis) = if particle.is_billboard() {
        let local = particle_xyz_at(ctx, particle, ages, 0);
        let inherited = ctx.parent_components.at(ctx.age);
        (
            std::array::from_fn(|axis| local[axis] * inherited.scale[axis]),
            VFX_IDENTITY_BASIS,
        )
    } else {
        (
            particle_coordinate_scale_at(ctx, particle, ages),
            ctx.motion.drawing_parent_with_auxiliary(
                parent.linear,
                ctx.binder_base.auxiliary_matrix.basis,
            ),
        )
    };
    if matches!(particle.coord_compute_order, 2 | 3 | 5) {
        // Factor S*R as a left-hand scale and unit local scale. Never divide by
        // an axis: zero/mirrored scales and parent shear must survive GPU upload.
        (
            [1.0; 3],
            basis_mul(basis, rotation_scale_basis([0.0, 0.0, 0.0, 1.0], scale)),
        )
    } else {
        (scale, basis)
    }
}

#[derive(Clone, Copy)]
struct ParticleDrawTransform {
    position: [f32; 3],
    scale: [f32; 3],
    parent_basis: [[f32; 3]; 3],
    orientation: [f32; 4],
}

/// Final ordinary-particle transform shared by staged history updates and draw
/// submission. Powder and Windmill have a separate client transform path.
fn particle_draw_transform_at(
    ctx: &SpawnContext,
    item: &AvfxEmitterItem,
    particle: &AvfxParticle,
    ages: CurveAges,
) -> ParticleDrawTransform {
    let (parent, pos_drift) = influence_transform(item, ctx);
    let (scale, parent_basis) = particle_scale_and_parent_at(ctx, particle, ages, parent);
    ParticleDrawTransform {
        position: particle_position_at(ctx, particle, ages, parent.linear, pos_drift),
        scale,
        parent_basis,
        orientation: particle_orientation_at(ctx, particle, ages),
    }
}

/// Shared translation for quads, models and Powder spawners. Integrate the
/// instantaneous ARs multiplier without feeding the scaled delta back into velocity.
fn particle_position(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    age: f32,
    parent_basis: [[f32; 3]; 3],
    pos_drift: [f32; 3],
) -> [f32; 3] {
    particle_position_at(
        ctx,
        particle,
        CurveAges {
            local: age,
            total: age,
        },
        parent_basis,
        pos_drift,
    )
}

fn particle_position_at(
    ctx: &SpawnContext,
    particle: &AvfxParticle,
    ages: CurveAges,
    parent_basis: [[f32; 3]; 3],
    pos_drift: [f32; 3],
) -> [f32; 3] {
    let local = particle_xyz_at(ctx, particle, ages, 2);
    let gravity = ctx.gravity_offset.unwrap_or_else(|| {
        integration::looped_curve_range(
            &particle.gravity,
            &particle.gravity_random,
            ctx.clock.motion_range(ctx.age),
            f64::from(ctx.clock.loop_start),
            f64::from(ctx.clock.loop_end),
            ctx.seed ^ 0x6A17,
        )
        .displacement as f32
    });
    let mut position = ctx
        .motion
        .position(ctx, particle, ages.local, local, parent_basis);
    for axis in 0..3 {
        position[axis] += pos_drift[axis];
    }
    position[1] += gravity;
    position
}

/// ItPr 的 PICd + ICbS/ICbR 解析出粒子的有效发射器变换分量
/// （VFXEditor `ParentInfluenceCoordOptions`；出生点和注入运动由
/// `InjectionMotion` 单独按 PICd 选择当前或出生矩阵）。
fn influence_transform(item: &AvfxEmitterItem, ctx: &SpawnContext) -> (EmitterBase, [f32; 3]) {
    let mut parent = match coordinate_mode(item.parent_influence_coord) {
        2 | 8 => ctx.emitter_now.world(),
        3 => ctx.emitter.world(),
        // PICd=1 inherits scalar scale/Euler through ParentComponents, not a
        // composed emitter matrix. PICd=0 does not inherit drawing components.
        _ => EmitterBase::root([0.0; 3]),
    };
    if coordinate_mode(item.parent_influence_coord) == 1 && item.influence_coord_binder {
        // Client 0x1403d0c90 obtains the ancestor type=9 Binder matrix and
        // 0x14040d340 copies only its linear part, clearing translation.
        parent = ctx.binder_base;
        parent.position = [0.0; 3];
    }
    (parent, [0.0; 3])
}

fn basis_transform(basis: [[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|row| {
        basis[0][row] * vector[0] + basis[1][row] * vector[1] + basis[2][row] * vector[2]
    })
}

fn basis_mul(left: [[f32; 3]; 3], right: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    right.map(|column| basis_transform(left, column))
}

fn rotation_scale_basis(rotation: [f32; 4], scale: [f32; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|axis| {
        let mut column = [0.0; 3];
        column[axis] = scale[axis];
        quat_rotate(rotation, column)
    })
}

/// Client 0x14040c600, using column vectors and VFXEditor CCOT values.
fn coordinate_basis(order: i32, rotation: [f32; 4], scale: [f32; 3]) -> [[f32; 3]; 3] {
    if matches!(order, 2 | 3 | 5) {
        rotation_scale_basis(rotation, [1.0; 3])
            .map(|column| std::array::from_fn(|row| scale[row] * column[row]))
    } else {
        rotation_scale_basis(rotation, scale)
    }
}

fn coordinate_translation(
    order: i32,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
) -> [f32; 3] {
    let scaled = |v: [f32; 3]| std::array::from_fn(|axis| scale[axis] * v[axis]);
    match order {
        1 => quat_rotate(rotation, scaled(position)),
        2 => scaled(position),
        3 => scaled(quat_rotate(rotation, position)),
        4 => quat_rotate(rotation, position),
        // Invalid values retain the preview's SRT fallback, not client semantics.
        _ => position,
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

/// 欧拉角（弧度）→ 四元数（xyzw）。`order` 为 VFXEditor `RotationOrder`
/// （0 XYZ、1 YZX、2 ZXY、3 XZY、4 YXZ、5 ZYX），名称序即旋转施加顺序。
fn quat_from_euler(order: i32, angles: [f32; 3]) -> [f32; 4] {
    // ROT/RoOT reach the client matrix helper through byte fields. Unknown
    // orders return identity only when at least two axes are nonzero.
    let order = order as u8;
    if order > 5 && angles.iter().filter(|&&angle| angle != 0.0).count() > 1 {
        return [0.0, 0.0, 0.0, 1.0];
    }
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
        AvfxColorCurve, AvfxCurve, AvfxCurve3Axis, AvfxCurveKey, AvfxEffector, AvfxEmitter,
        AvfxFile, AvfxLife, AvfxParticle, AvfxParticleDataLaser, AvfxParticleTexture,
        AvfxScheduler, AvfxSchedulerItem, AvfxTimeline, AvfxTimelineItem, AvfxUvSet,
        ConeEmitterData, EmitterType, ParticleType,
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

    #[test]
    fn unbound_root_consumes_revised_transform_while_bound_root_stays_at_bind_point() {
        let mut file = AvfxFile::default();
        file.global.revised_position = [2.0, 3.0, 4.0];
        file.global.revised_rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        file.global.revised_scale = [2.0, 3.0, 4.0];
        let runtime = VfxRuntime::new(&file);

        let root = runtime.bind_base(-1);
        assert_eq!(root.position, [2.0, 3.0, 4.0]);
        // Document +28 has identity basis; +30 carries root revision.
        // +38 computes rounded column lengths, not the authored scale.
        assert_eq!(root.linear, VFX_IDENTITY_BASIS);
        assert_eq!(root.transform_point([1.0, 0.0, 0.0]), [3.0, 3.0, 4.0]);
        for (actual, expected) in root.scale.into_iter().zip([2.0, 3.0, 4.0]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        let position = root.transform_point(root.auxiliary_matrix.transform_point([1.0, 0.0, 0.0]));
        for (actual, expected) in position.into_iter().zip([2.0, 5.0, 4.0]) {
            assert!((actual - expected).abs() < 1e-6);
        }

        let bound = runtime.bind_base(0);
        assert_eq!(bound.position, [0.0, 0.0, 0.0]);
        assert_eq!(bound.scale, [1.0, 1.0, 1.0]);

        file.global.revised_scale = [0.0; 3];
        let zero = VfxRuntime::new(&file).bind_base(-1);
        assert_eq!(zero.scale, [0.0; 3]);
        assert_eq!(zero.auxiliary_matrix.basis, [[0.0; 3]; 3]);
    }

    #[test]
    fn root_revised_color_is_applied_once_to_emitter_color() {
        let emitter = AvfxEmitter::default();
        let animation = EmitterColorAnimation {
            emitter: &emitter,
            emitter_index: 0,
            instance_seed: 0,
            clock: InstanceClock::new(-1.0, 1.0, 0, 0),
            parent: ParentColor::None,
        };
        assert_eq!(animation.at(0.0), [1.0; 4]);
        let mut color = animation.at(0.0);
        color[0] *= 0.5;
        color[1] *= 2.0;
        color[2] *= 3.0;
        assert_eq!(color, [0.5, 2.0, 3.0, 1.0]);
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

    fn frame_ramp(start: f32, end: f32, first: i16, last: i16) -> AvfxCurve {
        let mut curve = linear_curve(start);
        curve.keys[0].time = first;
        curve.keys.push(AvfxCurveKey {
            time: last,
            z: end,
            ..curve.keys[0]
        });
        curve
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

    #[test]
    fn runtime_property_evaluators_keep_local_and_total_curve_ages_separate() {
        let mut add = frame_ramp(0.0, 10.0, 0, 10);
        add.post_behavior = crate::avfx::BEHAVIOR_ADD;
        let mut repeat = add.clone();
        repeat.post_behavior = crate::avfx::BEHAVIOR_REPEAT;
        let ages = CurveAges {
            local: 5.0,
            total: 25.0,
        };
        let curve = AvfxCurve3Axis {
            x: Some(add),
            y: Some(repeat),
            ..Default::default()
        };
        assert_eq!(eval3_seeded_at(&curve, ages, 1.0, 7), [25.0, 5.0, 1.0]);
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
                texture_list: vec![2],
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

    // Motion tests using +Y explicitly rotate the client's default +Z cone.
    fn upward_cone() -> ConeEmitterData {
        ConeEmitterData {
            rotation: crate::avfx::EmitterDataRotation {
                angles: [
                    linear_curve(-std::f32::consts::FRAC_PI_2),
                    Default::default(),
                    Default::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn fixture_file() -> AvfxFile {
        AvfxFile {
            version: 0x2011_0913,
            schedulers: vec![AvfxScheduler {
                item_count: None,
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
                binder_index: -1,
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
                clips: Vec::new(),
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
                    ..Default::default()
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
    fn parsed_byte_depth_flags_reach_continuous_and_staged_outputs() {
        fn block(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = name.to_vec();
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            bytes.resize(bytes.len().next_multiple_of(4), 0);
            bytes
        }
        for test in [false, true] {
            for write in [false, true] {
                for soft in [false, true] {
                    let bytes = block(
                        b"XFVA",
                        &block(
                            b"lctP",
                            &[
                                block(b"tDsD", &[u8::from(test)]),
                                block(b"wDsD", &[u8::from(write)]),
                                block(b"pSsD", &[u8::from(soft)]),
                            ]
                            .concat(),
                        ),
                    );
                    let parsed = AvfxFile::parse(&bytes).unwrap();
                    let flags = &parsed.particles[0];
                    let mut file = fixture_file();
                    file.emitters[0].effector_index = -1;
                    file.emitters[0].particle_items[0].parameter_link = -1;
                    let p = &mut file.particles[0];
                    p.collision_type = -1;
                    p.depth_test = flags.depth_test;
                    p.depth_write = flags.depth_write;
                    p.is_soft_particle = flags.is_soft_particle;
                    let mut quads = Vec::new();
                    let runtime = VfxRuntime::new(&file);
                    runtime.sample(0.0, &mut quads);
                    assert!(!quads.is_empty());
                    assert!(
                        quads
                            .iter()
                            .all(|q| (q.depth_test, q.depth_write, q.soft_particle)
                                == (test, write, soft))
                    );
                    let mut playback = VfxPlayback::new(runtime);
                    assert_eq!(playback.fallback_reason(), None);
                    playback.advance(1.0 / 60.0).unwrap();
                    playback.sample(&mut quads, &mut Vec::new());
                    assert!(!quads.is_empty());
                    assert!(
                        quads
                            .iter()
                            .all(|q| (q.depth_test, q.depth_write, q.soft_particle)
                                == (test, write, soft))
                    );
                }
            }
        }
    }

    #[test]
    fn laser_sampling_uses_dedicated_geometry_only_for_supported_finite_data() {
        let mut file = fixture_file();
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Laser);
        particle.rotation_direction_base = 1;
        particle.data = AvfxParticleData::Laser(AvfxParticleDataLaser {
            length: linear_curve(0.75),
            width: linear_curve(0.2),
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(quads.iter().all(|quad| quad.laser.is_some()));

        file.particles[0].rotation_direction_base = 3;
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());

        file.particles[0].rotation_direction_base = 1;
        let AvfxParticleData::Laser(data) = &mut file.particles[0].data else {
            unreachable!()
        };
        data.width.keys[0].z = f32::INFINITY;
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());
    }

    #[test]
    fn static_non_edge_polyline_reaches_continuous_geometry() {
        let mut file = fixture_file();
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Polyline);
        particle.rotation_direction_base = 1;
        particle.data = AvfxParticleData::Polyline(crate::avfx::AvfxParticleDataPolyline {
            create_line_type: 0,
            point_count: 6,
            use_edge: false,
            is_local: true,
            point_count_end_distortion: 1,
            length: linear_curve(5.0),
            width: linear_curve(1.0),
            ..Default::default()
        });

        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        for quad in &quads {
            let polyline = quad.polyline.expect("Polyline must use dedicated geometry");
            assert!(!polyline.use_edge);
            assert!(!polyline.reverse_points);
            assert_eq!(polyline.point_count, 6);
            assert_eq!(polyline.point_end_distortion(), [0, 255, 255, 255, 255, 0]);
        }

        if let AvfxParticleData::Polyline(data) = &mut file.particles[0].data {
            data.is_spline = true;
        }
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());

        if let AvfxParticleData::Polyline(data) = &mut file.particles[0].data {
            data.is_spline = false;
            data.create_line_type = 1;
        }
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());
    }

    #[test]
    fn line_sampling_uses_dedicated_normal_and_supported_simple_paths() {
        let mut file = fixture_file();
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Line);
        particle.rotation_direction_base = 1;
        particle.data = AvfxParticleData::Line(crate::avfx::AvfxParticleDataLine {
            length: linear_curve(0.75),
            color_begin: color_curve(1.0, 0.5, 0.25, 1.0),
            color_end: color_curve(0.25, 0.5, 1.0, 0.5),
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(quads.iter().all(|quad| quad.line.is_some()));
        let mut normal_at_two = Vec::new();
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut normal_at_two);
        let expected_colors: Vec<_> = normal_at_two
            .iter()
            .map(|quad| {
                let line = quad.line.unwrap();
                let quantize = |color: [f32; 4]| {
                    color.map(|value| (value * 1000.0).clamp(0.0, 10000.0).trunc() * 0.001)
                };
                (quantize(line.color_begin), quantize(line.color_end))
            })
            .collect();

        let particle = &mut file.particles[0];
        particle.simple_anim_enable = true;
        particle.simple = Some(crate::avfx::AvfxParticleSimple {
            create_count: 2,
            create_interval: 1,
            create_interval_count: 1,
            create_interval_life: 10,
            block_num: 1,
            injection_model_index: -1,
            injection_vertex_bind_model_index: -1,
            coord_accuracy: [1.0; 3],
            velocity_min: 0.1,
            velocity_max: 0.1,
            line_length_min: 0.25,
            line_length_max: 0.5,
            colors: [[255; 4]; 4],
            frames: [0, 3, 6, 10],
            ..Default::default()
        });
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
        assert!(!quads.is_empty());
        assert_eq!(quads.len() % 2, 0);
        assert!(quads.iter().all(|quad| quad.line.is_some()));
        for quad in &quads {
            let line = quad.line.unwrap();
            let offset = line.endpoint_offset.unwrap();
            let length = offset.iter().map(|value| value * value).sum::<f32>().sqrt();
            assert!((length - 0.25).abs() < 1.0e-6);
            assert!(expected_colors.contains(&(line.color_begin, line.color_end)));
        }
        let baseline = quads.clone();
        file.particles[0].simple.as_mut().unwrap().coord_gravity = [100.0, -200.0, 300.0];
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads, baseline, "Smpl Line does not consume CGX/Y/Z");

        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .injection_model_index = 0;
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
        assert!(quads.is_empty());

        file.particles[0].simple_anim_enable = false;
        file.particles[0].rotation_direction_base = 6;
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());

        file.particles[0].rotation_direction_base = 1;
        let AvfxParticleData::Line(data) = &mut file.particles[0].data else {
            unreachable!()
        };
        data.length.keys[0].z = f32::NAN;
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());
    }

    fn creation_count_fixture(
        children: bool,
        create_time: i32,
        count: i32,
        curve: f32,
    ) -> AvfxFile {
        let mut file = fixture_file();
        file.emitters[0].life.enabled = false;
        file.emitters[0].create_count = linear_curve(curve);
        file.emitters[0].particle_items[0].create_time = create_time;
        file.emitters[0].particle_items[0].create_count = count;
        if children {
            let mut child = file.emitters[0].clone();
            child.create_count = linear_curve(1.0);
            child.create_interval = linear_curve(1000.0);
            child.particle_items[0].create_time = 1;
            child.particle_items[0].create_count = 1;
            file.emitters.push(child);
            let mut item = file.emitters[0].particle_items.remove(0);
            item.target_index = 1;
            file.emitters[0].emitter_items.push(item);
        }
        file
    }

    #[test]
    fn non_shape_particle_paths_do_not_emit_quad_or_model_geometry() {
        for (raw, particle_type) in [
            (0, Some(ParticleType::Parameter)),
            (7, Some(ParticleType::Reserve0)),
            (14, Some(ParticleType::ModelSkin)),
            (15, Some(ParticleType::Dissolve)),
            (999, Some(ParticleType::Unknown(999))),
            (u32::MAX, Some(ParticleType::Unknown(u32::MAX))),
            (u32::MAX, None),
        ] {
            let mut file = creation_count_fixture(false, 0, 1, 1.0);
            file.particles[0].particle_type = particle_type;
            file.particles[0].raw_particle_type = raw;

            let runtime = VfxRuntime::new(&file);
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            runtime.sample(0.0, &mut quads);
            runtime.sample_mesh(0.0, &mut meshes);
            assert!(quads.is_empty(), "{particle_type:?}");
            assert!(meshes.is_empty(), "{particle_type:?}");
        }
    }

    fn assert_rotation(actual: [f32; 4], expected: [f32; 4]) {
        for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            assert_coordinate(quat_rotate(actual, axis), quat_rotate(expected, axis));
        }
    }

    fn rotate_axes(mut vector: [f32; 3], angles: [f32; 3], axes: [usize; 3]) -> [f32; 3] {
        for axis in axes {
            let (s, c) = angles[axis].sin_cos();
            let [x, y, z] = vector;
            vector = match axis {
                0 => [x, y * c - z * s, y * s + z * c],
                1 => [x * c + z * s, y, z * c - x * s],
                2 => [x * c - y * s, x * s + y * c, z],
                _ => unreachable!(),
            };
        }
        vector
    }

    #[test]
    fn euler_rotation_orders_match_client_axis_sequences_and_byte_aliases() {
        // 0x14040c1a8 and the three two-axis tables select these products.
        let orders = [
            [0, 1, 2],
            [1, 2, 0],
            [2, 0, 1],
            [0, 2, 1],
            [1, 0, 2],
            [2, 1, 0],
        ];
        for (order, axes) in orders.into_iter().enumerate() {
            for mask in 0..8 {
                let angles = std::array::from_fn(|axis| {
                    if mask & (1 << axis) != 0 {
                        [0.43, -0.71, 1.09][axis]
                    } else {
                        -0.0
                    }
                });
                for vector in [[0.37, -0.52, 0.81], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                    let expected = rotate_axes(vector, angles, axes);
                    for alias in [-512, -256, 0, 256, 512] {
                        assert_coordinate(
                            quat_rotate(quat_from_euler(order as i32 + alias, angles), vector),
                            expected,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn euler_unknown_order_keeps_single_axis_but_clears_combined_rotation() {
        for order in [6, 7, 127, 128, 255, -1, -2, i32::MAX] {
            for mask in 0_u32..8 {
                let angles = std::array::from_fn(|axis| {
                    if mask & (1 << axis) != 0 {
                        [0.43, -0.71, 1.09][axis]
                    } else {
                        -0.0
                    }
                });
                for vector in VFX_IDENTITY_BASIS {
                    let expected = if mask.count_ones() < 2 {
                        rotate_axes(vector, angles, [0, 1, 2])
                    } else {
                        vector
                    };
                    assert_coordinate(
                        quat_rotate(quat_from_euler(order, angles), vector),
                        expected,
                    );
                }
            }
        }
    }

    #[test]
    fn euler_field_width_and_unknown_orders_reach_shape_emitter_and_particle_sampling() {
        for target in 0..3 {
            let mut file = model_emission_fixture(3);
            file.models[0].emit_vertex_numbers = vec![0];
            let configure = |file: &mut AvfxFile, order: i32, angles: [f32; 3]| match target {
                0 => {
                    file.emitters[0].rotation_order = order;
                    file.emitters[0].rotation =
                        axis3(Some(angles[0]), Some(angles[1]), Some(angles[2]));
                }
                1 => {
                    file.particles[0].rotation_order = order;
                    file.particles[0].rotation =
                        axis3(Some(angles[0]), Some(angles[1]), Some(angles[2]));
                }
                _ => {
                    let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
                        unreachable!()
                    };
                    data.rotation.order = order;
                    data.rotation.angles = angles.map(linear_curve);
                }
            };
            for (actual_order, expected_order, expected_angles) in [
                (257, 1, [0.43, -0.71, 1.09]),
                (-254, 2, [0.43, -0.71, 1.09]),
                (6, 0, [0.0; 3]),
                (-1, 0, [0.0; 3]),
            ] {
                configure(&mut file, actual_order, [0.43, -0.71, 1.09]);
                let mut reference = file.clone();
                configure(&mut reference, expected_order, expected_angles);
                for time in [0.0, 0.4] {
                    let mut actual = Vec::new();
                    let mut expected = Vec::new();
                    VfxRuntime::new(&file).sample(time, &mut actual);
                    VfxRuntime::new(&reference).sample(time, &mut expected);
                    assert!(!actual.is_empty());
                    assert_eq!(
                        actual, expected,
                        "target {target}, order {actual_order}, time {time}"
                    );
                }
            }
        }
    }

    #[test]
    fn creation_angles_add_to_particle_euler_without_rotating_shape() {
        let mut file = creation_count_fixture(false, 1, 3, 0.0);
        let angles = [0.17, -0.28, 0.39];
        file.emitters[0].particle_items[0].by_injection_angle = angles;
        file.particles[0].rotation = axis3(Some(0.4), Some(-0.6), Some(0.8));
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        for kind in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::Windmill,
            ParticleType::Model,
            ParticleType::LightModel,
        ] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = match kind {
                ParticleType::Model => AvfxParticleData::Model {
                    model_number_random_value: 0,
                    model_number_random_type: 0,
                    model_number_random_interval: 0,
                    fresnel_type: 0,
                    directional_light_type: 0,
                    point_light_type: 0,
                    is_lightning: false,
                    is_morph: false,
                    model_indexes: vec![0],
                    animation_number: None,
                    morph: None,
                    fresnel_curve: None,
                    fresnel_curve_random: None,
                    fresnel_rotation: None,
                    color_begin: Default::default(),
                    color_end: Default::default(),
                },
                ParticleType::LightModel => AvfxParticleData::LightModel { model_index: 0 },
                _ => AvfxParticleData::default(),
            };
            for order in 0..6 {
                file.particles[0].rotation_order = order;
                for facing in [crate::avfx::rotation_direction_base::NONE, 5] {
                    file.particles[0].rotation_direction_base = facing;
                    let mut baseline = file.clone();
                    baseline.emitters[0].particle_items[0].by_injection_angle = [0.0; 3];
                    for frame in [0.0, 2.0] {
                        let sample = |file: &AvfxFile| {
                            let runtime = VfxRuntime::new(file);
                            let mut quads = Vec::new();
                            let mut meshes = Vec::new();
                            runtime.sample(frame / AVFX_FPS, &mut quads);
                            runtime.sample_mesh(frame / AVFX_FPS, &mut meshes);
                            quads
                                .iter()
                                .map(|q| (q.position, q.orientation))
                                .chain(meshes.iter().map(|m| (m.position, m.orientation)))
                                .collect::<Vec<_>>()
                        };
                        let actual = sample(&file);
                        let expected = sample(&baseline);
                        assert_eq!(actual.len(), 3);
                        for (index, ((position, rotation), (birth, _))) in
                            actual.into_iter().zip(expected).enumerate()
                        {
                            assert_coordinate(position, birth);
                            let euler = std::array::from_fn(|axis| {
                                [0.4, -0.6, 0.8][axis] + angles[axis] * index as f32
                            });
                            let rotation_order =
                                if matches!(kind, ParticleType::Powder | ParticleType::Windmill) {
                                    2
                                } else {
                                    order
                                };
                            assert_rotation(rotation, quat_from_euler(rotation_order, euler));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn light_model_depth_offset_reaches_mesh_instances() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::LightModel);
        particle.data = AvfxParticleData::LightModel { model_index: 0 };
        particle.depth_offset = -0.125;

        for offset_type in [0, 1] {
            file.particles[0].depth_offset_type = offset_type;
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert_eq!(meshes.len(), 1);
            assert_eq!(meshes[0].depth_offset_type, offset_type);
            assert_eq!(meshes[0].depth_offset, -0.125);
        }
    }

    #[test]
    fn creation_angles_keep_signed_counters_across_dead_history_and_loops() {
        let mut file = creation_count_fixture(false, 0, 1, 3.0);
        file.emitters[0].create_interval = linear_curve(1.0);
        file.emitters[0].loop_start = 2;
        file.emitters[0].loop_end = 7;
        file.emitters[0].particle_items[0].by_injection_angle = [0.0, 0.0, 0.001];
        let mut second = file.emitters[0].particle_items[0];
        second.by_injection_angle[2] = -0.002;
        file.emitters[0].particle_items.push(second);
        file.particles[0].life.value = 0.0;
        let runtime = VfxRuntime::new(&file);
        for frame in [0.0, 1.0, 10923.0, 65536.0, 3_000_000.0, 1.0] {
            let mut quads = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 6, "frame={frame}");
            for (index, quad) in quads.iter().enumerate() {
                let ordinal = (frame as u64 * 3 + (index % 3) as u64) as i16;
                let angle = if index < 3 { 0.001 } else { -0.002 };
                assert_rotation(
                    quad.orientation,
                    quat_from_euler(0, [0.0, 0.0, angle * f32::from(ordinal)]),
                );
            }
        }
    }

    #[test]
    fn creation_angles_count_probability_passes_and_varying_batches() {
        for probability in [37, 100] {
            let mut file = creation_count_fixture(false, 0, 1, 2.0);
            file.emitters[0].child_limit = 0;
            file.emitters[0].create_interval = linear_curve(1.0);
            file.emitters[0].create_count = frame_ramp(2.0, 7.0, 0, 20);
            file.emitters[0].particle_items[0].by_injection_angle = [0.0, 0.0, 0.04];
            file.emitters[0].particle_items[0].create_probability = probability;
            file.particles[0].life.enabled = false;
            let mut births = Vec::new();
            VfxRuntime::new(&file).sample_ambient(24.0, &mut |ctx, item, particle| {
                let mut quads = Vec::new();
                VfxRuntime::new(&file).push_quad(ctx, item, particle, &mut quads);
                births.push((ctx.age, quads[0]));
            });
            assert!(births.len() > 10);
            for (ordinal, (_, quad)) in births.iter().enumerate() {
                assert_rotation(
                    quad.orientation,
                    quat_from_euler(0, [0.0, 0.0, 0.04 * ordinal as f32]),
                );
            }
            file.particles[0].life.enabled = true;
            file.particles[0].life.value = 1.0;
            let mut actual = Vec::new();
            VfxRuntime::new(&file).sample(24.0 / AVFX_FPS, &mut actual);
            let expected: Vec<_> = births
                .into_iter()
                .filter_map(|(age, q)| (age <= 1.0).then_some(q))
                .enumerate()
                .map(|(order, mut quad)| {
                    quad.draw_order = Some(order as u64);
                    quad
                })
                .collect();
            assert!(!expected.is_empty());
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn creation_angles_skip_constant_count_tail_with_its_prefix_counter() {
        let mut file = creation_count_fixture(false, 0, 1, 2.0);
        file.emitters[0].create_interval = linear_curve(1.0);
        file.emitters[0].create_count = frame_ramp(2.0, 5.0, 0, 10);
        file.emitters[0].create_count.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        file.emitters[0].particle_items[0].by_injection_angle = [0.0, 0.0, 0.001];
        file.particles[0].life.value = 0.0;
        let runtime = VfxRuntime::new(&file);
        for frame in [10.0, 3_000_000.0, 20.0] {
            let mut quads = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 5);
            for (index, quad) in quads.iter().enumerate() {
                let ordinal = (20 + (frame as u64 - 10) * 5 + index as u64) as i16;
                assert_rotation(
                    quad.orientation,
                    quat_from_euler(0, [0.0, 0.0, 0.001 * f32::from(ordinal)]),
                );
            }
        }
    }

    #[test]
    fn random_count_live_window_keeps_full_history_injection_angles() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        file.emitters[0].child_limit = 0;
        file.emitters[0].create_interval = linear_curve(1.0);
        file.emitters[0].create_count = frame_ramp(1.0, 2.0, 0, 4);
        file.emitters[0].create_count_random = linear_curve(100.0);
        file.emitters[0].create_count_random.random_type = 4;
        file.emitters[0].particle_items[0].by_injection_angle = [0.0, 0.0, 0.001];
        file.particles[0].life.enabled = false;

        let mut history = Vec::new();
        let runtime = VfxRuntime::new(&file);
        runtime.sample_ambient(19.0, &mut |ctx, item, particle| {
            let mut quads = Vec::new();
            runtime.push_quad(ctx, item, particle, &mut quads);
            if ctx.age == 0.0 {
                history.push(quads[0].orientation);
            }
        });
        assert!(history.len() > 2);

        file.particles[0].life.enabled = true;
        file.particles[0].life.value = 0.0;
        let mut visible = Vec::new();
        VfxRuntime::new(&file).sample(19.0 / AVFX_FPS, &mut visible);
        assert_eq!(
            visible
                .iter()
                .map(|quad| quad.orientation)
                .collect::<Vec<_>>(),
            history
        );
    }

    #[test]
    fn creation_angles_are_child_local_and_enter_scalar_inheritance() {
        let mut file = creation_count_fixture(true, 1, 2, 0.0);
        file.emitters[0].data = None;
        let angles = [0.2, -0.3, 0.5];
        file.emitters[0].emitter_items[0].by_injection_angle = angles;
        file.emitters[0].position = Default::default();
        let child = &mut file.emitters[1];
        child.data = None;
        child.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        child.rotation = axis3(Some(0.4), Some(0.6), Some(-0.8));
        child.scale = axis3(Some(2.0), Some(3.0), Some(-1.0));
        child.particle_items[0].parent_influence_coord = 1;
        child.particle_items[0].influence_coord_rot = true;
        file.particles[0].rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        for rotation_order in 0..6 {
            file.emitters[1].rotation_order = rotation_order;
            for order in 0..6 {
                file.emitters[1].coord_compute_order = order;
                for inherit in [false, true] {
                    file.emitters[1].particle_items[0].influence_coord_rot = inherit;
                    let mut quads = Vec::new();
                    VfxRuntime::new(&file).sample(0.0, &mut quads);
                    assert_eq!(quads.len(), 2);
                    for (index, quad) in quads.iter().enumerate() {
                        let euler = std::array::from_fn(|axis| {
                            [0.4, 0.6, -0.8][axis] + angles[axis] * index as f32
                        });
                        assert_coordinate(
                            quad.position,
                            coordinate_translation(
                                order,
                                [1.0, 2.0, 3.0],
                                quat_from_euler(rotation_order, euler),
                                [2.0, 3.0, -1.0],
                            ),
                        );
                        assert_rotation(
                            quad.orientation,
                            quat_from_euler(
                                file.particles[0].rotation_order,
                                if inherit { euler } else { [0.0; 3] },
                            ),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn timeline_root_ignores_definition_life_for_particles_and_child_births() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 0, 1, 1.0);
            file.timelines[0].items[0].end_time = 100;
            file.emitters[0].create_interval = linear_curve(7.0);
            file.emitters[0].position.x = Some(frame_ramp(0.0, 100.0, 0, 100));
            if children {
                file.emitters[1].life.enabled = true;
                file.emitters[1].life.value = 1000.0;
            }
            let baseline = VfxRuntime::new(&file);
            for duration in [5.0, 10.0, 30.0] {
                file.emitters[0].life.enabled = true;
                file.emitters[0].life.value = duration;
                let runtime = VfxRuntime::new(&file);
                for frame in [0.0, 4.0, 7.0, 24.0, 90.0, 7.0] {
                    let mut expected = Vec::new();
                    let mut actual = Vec::new();
                    baseline.sample(frame / AVFX_FPS, &mut expected);
                    runtime.sample(frame / AVFX_FPS, &mut actual);
                    assert!(!expected.is_empty());
                    assert_eq!(
                        actual, expected,
                        "children={children}, Life={duration}, frame={frame}"
                    );
                }
            }
        }
    }

    #[test]
    fn timeline_root_keeps_color_components_and_random_state_past_definition_life() {
        let mut file = fixture_file();
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].life.enabled = false;
        file.emitters[0].scale.x = Some(frame_ramp(1.0, 4.0, 0, 60));
        file.emitters[0].rotation.z = Some(frame_ramp(0.0, 1.0, 0, 60));
        file.emitters[0].color = animated_parent_color();
        file.emitters[0].color.random = [Some(linear_curve(0.4)), None, None, None, None];
        let item = &mut file.emitters[0].particle_items[0];
        item.create_time = 1;
        item.create_count = 1;
        item.parent_influence_color = 2;
        item.parent_influence_coord = 1;
        item.influence_coord_scale = true;
        item.influence_coord_rot = true;
        file.particles[0].life.enabled = false;
        let baseline = VfxRuntime::new(&file);
        file.emitters[0].life.enabled = true;
        file.emitters[0].life.value = 10.0;
        let runtime = VfxRuntime::new(&file);
        for frame in [0.0, 9.0, 10.0, 24.0, 90.0, 9.0] {
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            baseline.sample(frame / AVFX_FPS, &mut expected);
            runtime.sample(frame / AVFX_FPS, &mut actual);
            assert_eq!(expected.len(), 1);
            assert_eq!(actual, expected, "frame={frame}");
        }
    }

    #[test]
    fn creation_trigger_does_not_depend_on_target_lifetime() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 1, 1, 1.0);
            file.emitters[0].create_interval = linear_curve(5.0);
            file.particles[0].life.value = 20.0;
            if children {
                file.emitters[1].life.enabled = true;
                file.emitters[1].life.value = 1000.0;
            }
            for (frame, expected) in [(0.0, 1), (15.0, 1), (21.0, 0), (15.0, 1)] {
                let mut count = 0;
                VfxRuntime::new(&file).sample_ambient(frame, &mut |_, _, _| count += 1);
                assert_eq!(
                    count, expected,
                    "initial: children={children}, frame={frame}"
                );
            }
            file.particles[0].life.enabled = false;
            if children {
                file.emitters[0].emitter_items[0].create_time = 0;
                file.emitters[1].life.enabled = false;
            } else {
                file.emitters[0].particle_items[0].create_time = 0;
            }
            let mut count = 0;
            VfxRuntime::new(&file).sample_ambient(15.0, &mut |_, _, _| count += 1);
            assert_eq!(count, 4, "periodic: children={children}");
        }
    }

    #[test]
    fn creation_trigger_startup_does_not_require_a_positive_interval() {
        for children in [false, true] {
            for create_time in [0, 1] {
                for interval in [0.0, -1.0] {
                    let mut file = creation_count_fixture(children, create_time, 1, 1.0);
                    file.emitters[0].create_interval = linear_curve(interval);
                    for frame in [0.0, 1.0] {
                        let mut count = 0;
                        VfxRuntime::new(&file).sample_ambient(frame, &mut |_, _, _| count += 1);
                        assert_eq!(
                            count, 1,
                            "children={children}, CrTm={create_time}, CrI={interval}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn creation_trigger_finish_waits_for_emitter_death_and_preserves_particle_tail() {
        let mut file = creation_count_fixture(false, 2, 1, 1.0);
        file.timelines[0].items[0].end_time = 10;
        file.emitters[0].create_interval = linear_curve(5.0);
        file.particles[0].life.value = 20.0;
        for (frame, expected) in [
            (0.0, 0),
            (10.0, 0),
            (11.0, 1),
            (30.0, 1),
            (31.0, 0),
            (11.0, 1),
        ] {
            let mut births = Vec::new();
            VfxRuntime::new(&file)
                .sample_ambient(frame, &mut |ctx, _, _| births.push(frame - ctx.age));
            assert_eq!(births, vec![10.0; expected], "frame={frame}");
        }
        file.timelines[0].items[0].end_time = -1;
        let mut count = 0;
        VfxRuntime::new(&file).sample_ambient(100.0, &mut |_, _, _| count += 1);
        assert_eq!(
            count, 0,
            "an infinite emitter never invokes its finish items"
        );
    }

    #[test]
    fn finite_child_emitter_stops_without_restarting_and_keeps_descendants() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[1].life.enabled = true;
        file.emitters[1].life.value = 10.0;
        file.emitters[1].create_interval = linear_curve(5.0);
        file.emitters[1].particle_items[0].create_time = 0;
        file.particles[0].life.value = 6.0;
        for (frame, expected) in [
            (6.0, vec![0.0, 5.0]),
            (11.0, vec![5.0, 10.0]),
            (16.0, vec![10.0]),
            (17.0, vec![]),
            (6.0, vec![0.0, 5.0]),
        ] {
            let mut births = Vec::new();
            VfxRuntime::new(&file)
                .sample_ambient(frame, &mut |ctx, _, _| births.push(frame - ctx.age));
            assert_eq!(births, expected, "frame={frame}");
        }
    }

    #[test]
    fn child_lifetime_override_scales_creation_animation_and_freezes_at_finish() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[0].data = None;
        let child = &mut file.emitters[1];
        child.life.enabled = true;
        child.life.value = 10.0;
        child.create_interval = linear_curve(5.0);
        child.position.x = Some(frame_ramp(0.0, 10.0, 0, 10));
        child.color = animated_parent_color();
        child.particle_items[0].create_time = 0;
        child.particle_items[0].parent_influence_color = 2;
        let item = &mut file.emitters[0].emitter_items[0];
        item.override_life = true;
        item.override_life_value = 5;
        for (frame, expected) in [
            (4.0, vec![0.0, 2.5]),
            (6.0, vec![0.0, 2.5, 5.0]),
            (4.0, vec![0.0, 2.5]),
        ] {
            let mut births = Vec::new();
            VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                births.push(frame - ctx.age);
                let age = (frame * 2.0).min(10.0);
                assert!((ctx.emitter_now.position[0] - age).abs() < 1e-5);
                assert_color(
                    ctx.parent_color.apply([1.0; 4], ctx.age),
                    [1.0 + age / 10.0, 0.25, 0.5, 0.8],
                );
            });
            assert_eq!(births, expected);
        }
    }

    #[test]
    fn child_curve_loop_preserves_initial_creation_randomness_and_integrated_gravity() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[0].data = None;
        file.particles[0].life.value = 100.0;
        let child = &mut file.emitters[1];
        child.life.enabled = true;
        child.life.value = 10.0;
        child.loop_start = 2;
        child.loop_end = 6;
        child.position.x = Some(frame_ramp(0.0, 12.0, 0, 12));
        child.gravity = linear_curve(2.0);
        child.color = color_curve(1.0, 1.0, 1.0, 1.0);
        child.color.random = [Some(linear_curve(0.4)), None, None, None, None];
        child.particle_items[0].parent_influence_color = 2;
        let mut first_color = None;
        for (frame, age) in [
            (2.0, 2.0),
            (5.0, 5.0),
            (6.0, 2.0),
            (10.0, 2.0),
            (11.0, 3.0),
            (2.0, 2.0),
        ] {
            let mut count = 0;
            VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                count += 1;
                assert!((ctx.emitter_now.position[0] - age).abs() < 1e-5);
                assert!((ctx.emitter_now.position[1] - 1.0 - frame * frame).abs() < 1e-5);
                let color = ctx.parent_color.apply([1.0; 4], ctx.age);
                assert_eq!(color, *first_color.get_or_insert(color));
            });
            assert_eq!(count, 1, "frame={frame}");
        }
    }

    #[test]
    fn particle_loop_outlives_nominal_life_without_resetting_motion_or_randomness() {
        let mut file = creation_count_fixture(false, 1, 1, 1.0);
        file.emitters[0].data = None;
        let particle = &mut file.particles[0];
        particle.life.value = 10.0;
        particle.loop_start = 2;
        particle.loop_end = 6;
        particle.position.x = Some(frame_ramp(0.0, 20.0, 0, 20));
        particle.gravity = linear_curve(2.0);
        particle.color.random = [Some(linear_curve(0.4)), None, None, None, None];
        let runtime = VfxRuntime::new(&file);
        let mut first_color = None;
        for (frame, curve_age) in [
            (0.0, 0.0),
            (5.0, 5.0),
            (6.0, 2.0),
            (10.0, 2.0),
            (15.0, 3.0),
            (100.0, 4.0),
            (5.0, 5.0),
        ] {
            let mut quads = Vec::new();
            runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                runtime.push_quad(ctx, item, particle, &mut quads);
            });
            assert_eq!(quads.len(), 1, "frame={frame}");
            assert_eq!(quads[0].position, [curve_age, 0.5 + frame * frame, 0.0]);
            assert_eq!(quads[0].color, *first_color.get_or_insert(quads[0].color));
        }
    }

    #[test]
    fn particle_overrides_have_independent_rates_scaled_loops_and_lifetime_boundaries() {
        let mut file = creation_count_fixture(false, 1, 1, 1.0);
        file.emitters[0].data = None;
        let item = file.emitters[0].particle_items[0];
        file.emitters[0].particle_items = [5, 10, 20]
            .map(|duration| AvfxEmitterItem {
                override_life: true,
                override_life_value: duration,
                ..item
            })
            .to_vec();
        let particle = &mut file.particles[0];
        particle.life.value = 10.0;
        particle.loop_start = 2;
        particle.loop_end = 6;
        particle.position.x = Some(frame_ramp(0.0, 20.0, 0, 20));
        let runtime = VfxRuntime::new(&file);
        // The fast instance reaches Life=10 before its scaled loop end=12.
        // The other instances wrap at real frame 6 and remain alive.
        for (frame, expected) in [
            (4.0, vec![(5, 8.0), (10, 4.0), (20, 2.0)]),
            (5.0, vec![(5, 10.0), (10, 5.0), (20, 2.5)]),
            (5.25, vec![(10, 5.25), (20, 2.625)]),
            (6.0, vec![(10, 2.0), (20, 1.0)]),
            (20.0, vec![(10, 4.0), (20, 2.0)]),
            (4.0, vec![(5, 8.0), (10, 4.0), (20, 2.0)]),
        ] {
            let mut actual = Vec::new();
            runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                let mut quads = Vec::new();
                runtime.push_quad(ctx, item, particle, &mut quads);
                actual.push((item.override_life_value, quads[0].position[0]));
            });
            assert_eq!(actual, expected, "frame={frame}");
        }
    }

    #[test]
    fn particle_inherited_life_uses_parent_nominal_life_before_its_rate() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[0].data = None;
        file.emitters[0].emitter_items[0].override_life = true;
        file.emitters[0].emitter_items[0].override_life_value = 5;
        let child = &mut file.emitters[1];
        child.data = None;
        child.life.enabled = true;
        child.life.value = 10.0;
        child.particle_items[0].inherit_parent_life = true;
        file.particles[0].life.value = 40.0;
        file.particles[0].position.x = Some(frame_ramp(0.0, 40.0, 0, 40));
        let runtime = VfxRuntime::new(&file);
        for (frame, expected) in [
            (0.0, vec![0.0]),
            (4.0, vec![16.0]),
            (6.0, vec![24.0]),
            (10.0, vec![40.0]),
            (10.25, vec![]),
            (4.0, vec![16.0]),
        ] {
            let mut quads = Vec::new();
            runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                runtime.push_quad(ctx, item, particle, &mut quads);
            });
            assert_eq!(
                quads
                    .iter()
                    .map(|quad| quad.position[0])
                    .collect::<Vec<_>>(),
                expected,
                "frame={frame}"
            );
        }
    }

    #[test]
    fn particle_zero_life_is_finite_even_with_positive_override() {
        for (enabled, life, survives) in
            [(true, 0.0, false), (true, -1.0, true), (false, 0.0, true)]
        {
            let mut file = creation_count_fixture(false, 1, 1, 1.0);
            file.particles[0].life.enabled = enabled;
            file.particles[0].life.value = life;
            file.emitters[0].particle_items[0].override_life = true;
            file.emitters[0].particle_items[0].override_life_value = 60;
            assert_eq!(file.particles[0].life_frames(), (!survives).then_some(0.0));
            for frame in [0.0, 0.25, 60.0, 0.0] {
                let mut count = 0;
                VfxRuntime::new(&file).sample_ambient(frame, &mut |_, _, _| count += 1);
                assert_eq!(count, usize::from(frame == 0.0 || survives));
            }
        }
    }

    #[test]
    fn particle_start_frame_separates_animation_motion_and_parent_time() {
        for prewarm in [false, true] {
            for duration in [100, 200] {
                let mut file = creation_count_fixture(false, 1, 1, 1.0);
                file.emitters[0].data = None;
                file.emitters[0].color = animated_parent_color();
                let item = &mut file.emitters[0].particle_items[0];
                item.start_frame = 10;
                item.start_frame_null_update = prewarm;
                item.override_life = true;
                item.override_life_value = duration;
                item.parent_influence_color = 2;
                let particle = &mut file.particles[0];
                particle.life.value = 100.0;
                particle.position.x = Some(frame_ramp(0.0, 100.0, 0, 100));
                particle.gravity = frame_ramp(0.0, 100.0, 0, 100);
                particle.color = color_curve(1.0, 1.0, 1.0, 1.0);
                let runtime = VfxRuntime::new(&file);
                for frame in [0.0_f32, 2.0, 5.0, 2.0] {
                    let mut quads = Vec::new();
                    runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                        runtime.push_quad(ctx, item, particle, &mut quads);
                    });
                    assert_eq!(quads.len(), 1, "prewarm={prewarm}, frame={frame}");
                    let rate = 100.0 / duration as f32;
                    let age = (10.0 + frame) * rate;
                    let elapsed = frame * rate;
                    let y = if prewarm {
                        age.powi(3) / 6.0
                    } else {
                        10.0 * rate * elapsed.powi(2) / 2.0 + elapsed.powi(3) / 6.0
                    };
                    let x = if !prewarm && frame == 0.0 { 0.0 } else { age };
                    for (actual, expected) in quads[0].position.into_iter().zip([x, 0.5 + y, 0.0]) {
                        assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
                    }
                    assert_color(quads[0].color, [1.0 + frame / 10.0, 0.25, 0.5, 0.8]);
                }
            }
        }
    }

    #[test]
    fn emitter_start_frame_prewarms_descendants_only_when_requested() {
        for prewarm in [false, true] {
            let mut file = creation_count_fixture(true, 1, 1, 1.0);
            file.particles[0].life.enabled = false;
            file.emitters[0].data = None;
            let item = &mut file.emitters[0].emitter_items[0];
            item.start_frame = 12;
            item.start_frame_null_update = prewarm;
            let child = &mut file.emitters[1];
            child.data = None;
            child.create_interval = linear_curve(5.0);
            child.particle_items[0].create_time = 0;
            child.position.x = Some(frame_ramp(0.0, 100.0, 0, 100));
            for frame in [0.0, 2.0, 5.0, 0.0] {
                let mut births = Vec::new();
                VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                    births.push((frame - ctx.age, ctx.emitter.position[0]));
                });
                let expected = if prewarm {
                    let mut values = vec![(-12.0, 0.0), (-7.0, 5.0), (-2.0, 10.0)];
                    if frame >= 3.0 {
                        // Original linear reader: (15 / 100) * 100 rounds
                        // to 0x41700001; the old f64 shortcut produced 15.
                        values.push((3.0, f32::from_bits(0x41700001)));
                    }
                    values
                } else if frame >= 5.0 {
                    vec![(0.0, 0.0), (5.0, 17.0)]
                } else {
                    vec![(0.0, 0.0)]
                };
                assert_eq!(births, expected, "prewarm={prewarm}, frame={frame}");
            }
        }
    }

    #[test]
    fn start_frame_prewarm_freezes_external_parent_and_keeps_descendant_tail() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[0].color = animated_parent_color();
        let item = &mut file.emitters[0].emitter_items[0];
        item.start_frame = 12;
        item.start_frame_null_update = true;
        item.parent_influence_color = 2;
        let child = &mut file.emitters[1];
        child.life.enabled = true;
        child.life.value = 10.0;
        child.color = color_curve(1.0, 1.0, 1.0, 1.0);
        child.particle_items[0].parent_influence_color = 1;
        file.particles[0].color = color_curve(1.0, 1.0, 1.0, 1.0);
        let runtime = VfxRuntime::new(&file);
        for frame in [0.0, 18.0, 18.25, 0.0] {
            let mut quads = Vec::new();
            runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                runtime.push_quad(ctx, item, particle, &mut quads);
            });
            assert_eq!(quads.len(), usize::from(frame <= 18.0));
            for quad in quads {
                assert_color(quad.color, [1.0, 0.25, 0.5, 0.8]);
            }
        }
    }

    #[test]
    fn powder_start_frame_distinguishes_age_assignment_from_child_prewarm() {
        for prewarm in [false, true] {
            let mut file = powder_emission_fixture();
            file.models.clear();
            file.emitters[0].position.x = Some(frame_ramp(0.0, 100.0, 0, 100));
            let item = &mut file.emitters[0].particle_items[0];
            item.start_frame = 12;
            item.start_frame_null_update = prewarm;
            item.parent_influence_coord = 2;
            file.particles[0].position.y = Some(frame_ramp(0.0, 100.0, 0, 100));
            let runtime = VfxRuntime::new(&file);
            for frame in [0.0, 10.0, 0.0] {
                let mut quads = Vec::new();
                runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                    runtime.push_quad(ctx, item, particle, &mut quads);
                });
                let expected = match (prewarm, frame == 0.0) {
                    (false, true) => vec![[0.0; 3]],
                    (false, false) => vec![[0.0; 3], [10.0, 22.0, 0.0]],
                    (true, true) => vec![[0.0; 3], [0.0, 10.0, 0.0]],
                    (true, false) => vec![[0.0; 3], [0.0, 10.0, 0.0], [8.0, 20.0, 0.0]],
                };
                assert_eq!(
                    quads.iter().map(|q| q.position).collect::<Vec<_>>(),
                    expected
                );
            }
        }
    }

    #[test]
    fn start_frame_negative_loop_phase_keeps_the_birth_window_conservative() {
        let mut file = creation_count_fixture(false, 1, 1, 1.0);
        file.emitters[0].particle_items[0].start_frame = 12;
        file.particles[0].life.value = 10.0;
        file.particles[0].loop_start = -5;
        file.particles[0].loop_end = 12;
        for (frame, expected) in [(0.0, 1), (12.0, 1), (15.0, 1), (15.25, 0)] {
            let mut count = 0;
            VfxRuntime::new(&file).sample_ambient(frame, &mut |_, _, _| count += 1);
            assert_eq!(count, expected, "frame={frame}");
        }
    }

    #[test]
    fn particle_birth_window_preserves_the_rounded_lifetime_boundary() {
        let mut file = creation_count_fixture(false, 1, 1, 1.0);
        file.particles[0].life.value = 1.0;
        file.emitters[0].particle_items[0].override_life = true;
        file.emitters[0].particle_items[0].override_life_value = 61;
        let finish = 61.0_f32.next_up();
        for (frame, expected) in [(61.0, 1), (finish, 1), (finish.next_up(), 0)] {
            let mut count = 0;
            VfxRuntime::new(&file).sample_ambient(frame, &mut |_, _, _| count += 1);
            assert_eq!(count, expected, "frame={frame}");
        }
    }

    #[test]
    fn start_frame_fast_forward_retains_the_initial_creation_interval() {
        let mut file = creation_count_fixture(true, 1, 1, 1.0);
        file.emitters[0].emitter_items[0].start_frame = 30;
        file.emitters[1].create_interval = frame_ramp(2.0, 5.0, 0, 10);
        file.emitters[1].particle_items[0].create_time = 0;
        file.particles[0].life.value = 4.0;
        // Initialization latches interval 2 before assigning age 30. Later
        // intervals are 5, so creation times are 0, 2, 7, ..., 97.
        for (frame, expected) in [(100.0, vec![97.0]), (6.0, vec![2.0]), (7.0, vec![7.0])] {
            let mut births = Vec::new();
            VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                births.push(frame - ctx.age);
            });
            assert_eq!(births, expected, "frame={frame}");
        }
    }

    #[test]
    fn creation_trigger_finish_can_create_child_emitters() {
        let mut file = creation_count_fixture(true, 2, 1, 1.0);
        file.timelines[0].items[0].end_time = 10;
        for (frame, expected) in [(0.0, 0), (10.0, 0), (11.0, 1), (24.0, 1)] {
            let mut count = 0;
            VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                count += 1;
                assert_eq!(frame - ctx.age, 10.0);
            });
            assert_eq!(count, expected);
        }
    }

    #[test]
    fn initialization_creation_precedes_periodic_creation_at_startup() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        let mut initial = file.emitters[0].particle_items[0];
        initial.create_time = 1;
        initial.target_index = 1;
        file.particles.push(file.particles[0].clone());
        file.emitters[0].particle_items.push(initial);
        file.emitters[0].child_limit = 1;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].particle_index, 1);
    }

    #[test]
    fn creation_history_interleaves_items_at_each_periodic_event() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 0, 1, 1.0);
            file.particles.push(file.particles[0].clone());
            let mut item = if children {
                file.emitters[1].particle_items[0].target_index = 1;
                file.emitters[0].emitter_items[0]
            } else {
                file.emitters[0].particle_items[0]
            };
            item.target_index = if children { 0 } else { 1 };
            file.emitters[0].particle_items.push(item);
            file.emitters[0].create_interval = linear_curve(10.0);
            let runtime = VfxRuntime::new(&file);
            for frame in [20.0, 10.0, 0.0, 20.0] {
                let mut actual = Vec::new();
                runtime.sample_ambient(frame, &mut |ctx, item, _| {
                    actual.push((frame - ctx.age, item.target_index));
                });
                let expected: Vec<_> = (0..=frame as u32 / 10)
                    .flat_map(|event| [(event as f32 * 10.0, 0), (event as f32 * 10.0, 1)])
                    .collect();
                assert_eq!(actual, expected, "children={children}, frame={frame}");
            }
        }
    }

    #[test]
    fn creation_capacity_respects_event_time_before_item_order() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        file.particles[0].life.enabled = false;
        file.particles.push(file.particles[0].clone());
        let emitter = &mut file.emitters[0];
        emitter.create_interval = linear_curve(10.0);
        emitter.child_limit = 3;
        emitter.particle_items.push(AvfxEmitterItem {
            target_index: 1,
            ..emitter.particle_items[0]
        });
        for frame in [20.0, 1000.0, 20.0] {
            let mut actual = Vec::new();
            VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, item, _| {
                actual.push((frame - ctx.age, item.target_index));
            });
            assert_eq!(actual, [(0.0, 0), (0.0, 1), (10.0, 0)]);
        }
    }

    #[test]
    fn nonpositive_child_limit_does_not_impose_a_48_instance_limit() {
        for children in [false, true] {
            for limit in [-1, 0, 48, 60] {
                let mut file = creation_count_fixture(children, 1, 60, 1.0);
                file.emitters[0].child_limit = limit;
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(0.0, &mut quads);
                assert_eq!(quads.len(), if limit == 48 { 48 } else { 60 });
            }
        }
    }

    #[test]
    fn periodic_creation_ignores_generate_delay_for_both_target_kinds() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 0, 1, 1.0);
            file.emitters[0].create_interval = linear_curve(5.0);
            for delay in [-7, 0, 5] {
                for by_one in [false, true] {
                    let item = if children {
                        &mut file.emitters[0].emitter_items[0]
                    } else {
                        &mut file.emitters[0].particle_items[0]
                    };
                    item.generate_delay = delay;
                    item.generate_delay_by_one = by_one;
                    for frame in [0.0, 4.0, 10.0, 4.0] {
                        let mut births = Vec::new();
                        VfxRuntime::new(&file).sample_ambient(frame, &mut |ctx, _, _| {
                            births.push(frame - ctx.age);
                        });
                        let expected: Vec<_> = (0..=frame as u32 / 5)
                            .map(|event| event as f32 * 5.0)
                            .collect();
                        assert_eq!(
                            births, expected,
                            "children={children}, GenD={delay}, bGD={by_one}, frame={frame}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn positive_initial_delay_with_one_by_one_copy_has_no_creation_path() {
        for children in [false, true] {
            for parameter_link in [-1, 0] {
                let mut file = creation_count_fixture(children, 1, 1, 1.0);
                file.particles[0].life.enabled = false;
                let item = if children {
                    &mut file.emitters[0].emitter_items[0]
                } else {
                    &mut file.emitters[0].particle_items[0]
                };
                item.generate_delay = 5;
                item.generate_delay_by_one = true;
                item.parameter_link = parameter_link;
                let runtime = VfxRuntime::new(&file);
                for frame in [0.0, 4.0, 5.0, 100.0, 0.0] {
                    let mut count = 0;
                    runtime.sample_ambient(frame, &mut |_, _, _| count += 1);
                    assert_eq!(
                        count, 0,
                        "children={children}, PrLk={parameter_link}, frame={frame}"
                    );
                }
            }
        }
    }

    #[test]
    fn positive_finish_delay_requires_an_attachable_helper() {
        for children in [false, true] {
            for (count, parameter_link) in [(1, -1), (1, 0), (3, -1)] {
                for by_one in [false, true] {
                    let mut file = creation_count_fixture(children, 2, count, 1.0);
                    file.timelines[0].items[0].end_time = 10;
                    file.particles[0].life.enabled = false;
                    let item = if children {
                        &mut file.emitters[0].emitter_items[0]
                    } else {
                        &mut file.emitters[0].particle_items[0]
                    };
                    item.generate_delay = 5;
                    item.generate_delay_by_one = by_one;
                    item.parameter_link = parameter_link;
                    let runtime = VfxRuntime::new(&file);
                    for frame in [0.0, 10.0, 11.0, 15.0, 100.0, 11.0] {
                        let mut actual = 0;
                        runtime.sample_ambient(frame, &mut |_, _, _| actual += 1);
                        assert_eq!(
                            actual, 0,
                            "children={children}, CrCn={count}, PrLk={parameter_link}, bGD={by_one}, frame={frame}"
                        );
                    }

                    // Immediate finish creation is allowed on the dead parent.
                    if children {
                        file.emitters[0].emitter_items[0].generate_delay = 0;
                    } else {
                        file.emitters[0].particle_items[0].generate_delay = 0;
                    }
                    let mut actual = 0;
                    VfxRuntime::new(&file).sample_ambient(11.0, &mut |_, _, _| actual += 1);
                    assert_eq!(actual, count);
                }
            }
        }
    }

    #[test]
    fn rejected_delay_items_do_not_consume_creation_capacity() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 1, 1, 1.0);
            file.particles[0].life.enabled = false;
            file.particles.push(file.particles[0].clone());
            let mut accepted = if children {
                file.emitters[1].particle_items[0].target_index = 1;
                file.emitters[0].emitter_items[0]
            } else {
                file.emitters[0].particle_items[0]
            };
            accepted.create_time = 0;
            accepted.target_index = 0;
            let emitter = &mut file.emitters[0];
            emitter.child_limit = 1;
            emitter.create_interval = linear_curve(1000.0);
            let rejected = if children {
                &mut emitter.emitter_items[0]
            } else {
                emitter.particle_items[0].target_index = 1;
                &mut emitter.particle_items[0]
            };
            rejected.generate_delay = 5;
            rejected.generate_delay_by_one = true;
            emitter.particle_items.push(accepted);
            let runtime = VfxRuntime::new(&file);
            for frame in [0.0, 5.0, 100.0, 0.0] {
                let mut targets = Vec::new();
                runtime.sample_ambient(frame, &mut |_, item, _| targets.push(item.target_index));
                assert_eq!(targets, [0], "children={children}, frame={frame}");
            }
        }
    }

    #[test]
    fn merged_creation_history_preserves_each_items_seed_and_lifetime_window() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        file.emitters[0].create_interval = linear_curve(3.0);
        file.particles[0].life.value = 10.0;
        file.particles[0].life.value_random = 3.0;
        file.particles[0].color.random = [Some(linear_curve(0.3)), None, None, None, None];
        file.particles.push(file.particles[0].clone());
        file.particles[1].life.value = 1.0;
        file.particles[1].life.value_random = 0.5;
        let emitter = &mut file.emitters[0];
        emitter.particle_items.push(AvfxEmitterItem {
            target_index: 1,
            ..emitter.particle_items[0]
        });
        for frame in [0.0, 6.25, 15.0, 100_000.0, 6.25] {
            let sample = |file: &AvfxFile| {
                let runtime = VfxRuntime::new(file);
                let mut quads = Vec::new();
                runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                    runtime.push_quad(ctx, item, particle, &mut quads);
                });
                quads
            };
            let combined = sample(&file);
            assert!(!combined.is_empty());
            for target in 0..2 {
                let mut isolated = file.clone();
                isolated.emitters[0].particle_items[1 - target].enabled = false;
                let actual: Vec<_> = combined
                    .iter()
                    .filter(|q| q.particle_index == target)
                    .copied()
                    .collect();
                assert_eq!(actual, sample(&isolated), "target={target}, frame={frame}");
            }
        }
    }

    #[test]
    fn creation_count_selects_curve_or_item_without_multiplication() {
        for children in [false, true] {
            for (mode, curve, count, expected) in [
                (0, 2.9, 3, 2),
                (0, 2.9, 0, 2),
                (0, 2.9, -5, 2),
                (1, 9.0, 3, 3),
                (1, 0.0, 3, 3),
                (1, -1.0, 3, 3),
            ] {
                let file = creation_count_fixture(children, mode, count, curve);
                let mut births = 0;
                VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
                assert_eq!(
                    births, expected,
                    "children={children}, mode={mode}, curve={curve}, count={count}"
                );
            }
        }
    }

    #[test]
    fn creation_count_preserves_zero_and_rejects_invalid_float_counts() {
        for children in [false, true] {
            for curve in [
                0.0,
                0.99,
                -0.99,
                -1.0,
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                2147483648.0,
            ] {
                let file = creation_count_fixture(children, 0, 7, curve);
                let mut births = 0;
                VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
                assert_eq!(births, 0, "children={children}, curve={curve}");
            }
            for count in [0, -1, i32::MIN] {
                let file = creation_count_fixture(children, 1, count, 5.0);
                let mut births = 0;
                VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
                assert_eq!(births, 0, "children={children}, count={count}");
            }
        }
    }

    #[test]
    fn random_count_offsets_keep_zero_baseline_items_in_the_event_stream() {
        let mut file = creation_count_fixture(false, 0, 1, 0.0);
        file.emitters[0].create_count_random = linear_curve(2.0);
        file.emitters[0].create_count_random.random_type = 4;
        let emitter = &file.emitters[0];
        let item = &emitter.particle_items[0];
        assert!(emitter_can_create(emitter, item));
        assert!((0..32).any(|event| {
            emitter_create_count_at_seeded(emitter, item, 0.0, 0.0, event, 0xC2B2) > 0
        }));
        let mut births = 0;
        VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
        assert!(births > 0);
    }

    #[test]
    fn first_random_creation_count_uses_one_emitter_coefficient() {
        let mut file = creation_count_fixture(false, 0, 1, 0.0);
        file.emitters[0].create_count_random = linear_curve(100.0);
        file.emitters[0].create_count_random.random_type = 1;
        let emitter = &file.emitters[0];
        let item = &emitter.particle_items[0];
        let counts = (0..8)
            .map(|event| emitter_create_count_at_seeded(emitter, item, 0.0, 0.0, event, 0xC2B2))
            .collect::<Vec<_>>();
        assert!(counts.iter().all(|&count| count == counts[0]), "{counts:?}");
        assert_ne!(
            counts[0],
            emitter_create_count_at_seeded(emitter, item, 0.0, 0.0, 0, 0x1234)
        );

        file.emitters[0].create_count_random.random_type = 4;
        let emitter = &file.emitters[0];
        let item = &emitter.particle_items[0];
        let always_counts = (0..8)
            .map(|event| emitter_create_count_at_seeded(emitter, item, 0.0, 0.0, event, 0xC2B2))
            .collect::<Vec<_>>();
        assert!(always_counts.windows(2).any(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn creation_count_zero_histories_are_skipped_before_loop_enumeration() {
        for children in [false, true] {
            for mode in [0, 1, 2, -1, 3] {
                let mut file = creation_count_fixture(children, mode, 0, 0.99);
                file.emitters[0].life.enabled = true;
                file.emitters[0].life.value = 1.0;
                if children {
                    file.emitters[1].life.enabled = true;
                    file.emitters[1].life.value = 1.0;
                }
                let mut births = 0;
                VfxRuntime::new(&file).sample_ambient(2.0_f32.powi(40), &mut |_, _, _| births += 1);
                assert_eq!(births, 0, "children={children}, mode={mode}");
                file.emitters[0].create_count.keys.clear();
                VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
                assert_eq!(births, 0);
            }
        }
    }

    #[test]
    fn creation_count_finish_and_unknown_modes_do_not_fall_back_to_curve() {
        let mut file = fixture_file();
        let item = &mut file.emitters[0].particle_items[0];
        item.create_time = 2;
        item.create_count = 4;
        assert_eq!(
            emitter_create_count(&file.emitters[0], &file.emitters[0].particle_items[0], 0.0),
            4
        );
        file.emitters[0].particle_items[0].create_time = 99;
        let mut births = 0;
        VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births += 1);
        assert_eq!(births, 0);
    }

    #[test]
    fn creation_count_animated_curve_uses_each_birth_and_keeps_child_limit() {
        for children in [false, true] {
            let mut file = creation_count_fixture(children, 0, 7, 1.99);
            let first = file.emitters[0].create_count.keys[0];
            file.emitters[0].create_count.keys.push(AvfxCurveKey {
                time: 15,
                z: 2.99,
                ..first
            });
            // A finite child currently permits periodic parent creation. The
            // lifecycle rule is audited separately from count selection.
            if children {
                file.emitters[1].life.enabled = true;
                file.emitters[1].life.value = 1000.0;
            }
            let mut births = Vec::new();
            for frame in [15.0, 0.0, 15.0] {
                births.clear();
                VfxRuntime::new(&file)
                    .sample_ambient(frame, &mut |ctx, _, _| births.push(frame - ctx.age));
                assert_eq!(
                    births,
                    if frame == 0.0 {
                        vec![0.0]
                    } else {
                        vec![0.0, 15.0, 15.0]
                    }
                );
            }
            file.emitters[0].create_count = linear_curve(2147483520.0);
            file.emitters[0].child_limit = 2;
            births.clear();
            VfxRuntime::new(&file).sample_ambient(0.0, &mut |_, _, _| births.push(0.0));
            assert_eq!(births.len(), 2);
        }
    }

    fn assert_coordinate(actual: [f32; 3], expected: [f32; 3]) {
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 2e-5, "{actual:?} != {expected:?}");
        }
    }

    fn model_emission_fixture(method: i32) -> AvfxFile {
        let mut file = powder_emission_fixture();
        file.emitters[0].emitter_type = Some(EmitterType::Model);
        file.emitters[0].data = Some(AvfxEmitterData::Model(crate::avfx::ModelEmitterData {
            model_index: 0,
            generate_method: method,
            injection_speed: linear_curve(0.5),
            ..Default::default()
        }));
        file.emitters[0].particle_items[0].parent_influence_coord = 0;
        file.models[0].emit_vertices = vec![
            crate::avfx::VfxEmitVertex {
                position: [2.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                color: [255; 4],
            },
            crate::avfx::VfxEmitVertex {
                position: [0.0, 0.0, 3.0],
                normal: [1.0, 0.0, 0.0],
                color: [255; 4],
            },
            crate::avfx::VfxEmitVertex {
                position: [0.0, 4.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [255; 4],
            },
        ];
        file.models[0].emit_vertex_numbers = vec![2, 0, 1];
        file.particles[0].particle_type = Some(ParticleType::Quad);
        file.particles[0].simple_anim_enable = false;
        file
    }

    fn linear_keyframes(keys: &[(i16, f32)]) -> AvfxCurve {
        AvfxCurve {
            keys: keys
                .iter()
                .map(|&(time, z)| AvfxCurveKey {
                    time,
                    z,
                    interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                    x: 0.0,
                    y: 0.0,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn model_shape_births_distinguish_to_vertex_and_on_vertex_and_follow_vnum() {
        for method in 0..8 {
            let mut file = model_emission_fixture(method);
            // A repeated index proves that random selection also goes through VNum.
            if method % 2 == 0 {
                file.models[0].emit_vertex_numbers = vec![2, 2, 2];
            } else {
                file.emitters[0].create_count = linear_curve(3.0);
            }
            let runtime = VfxRuntime::new(&file);
            let mut birth = Vec::new();
            let mut later = Vec::new();
            runtime.sample(0.0, &mut birth);
            runtime.sample(2.0 / AVFX_FPS, &mut later);
            let indices = if method % 2 == 0 {
                vec![2]
            } else {
                vec![2, 0, 1]
            };
            assert_eq!(birth.len(), indices.len());
            assert_eq!(later.len(), indices.len());
            for ((birth, later), index) in birth.iter().zip(&later).zip(indices) {
                let vertex = file.models[0].emit_vertices[index];
                let on_vertex = method % 4 >= 2;
                let origin = if on_vertex { vertex.position } else { [0.0; 3] };
                let direction = if on_vertex {
                    vertex.normal
                } else {
                    motion::normalized(vertex.position)
                };
                assert_coordinate(birth.position, origin);
                assert_coordinate(
                    later.position,
                    std::array::from_fn(|i| origin[i] + direction[i]),
                );
            }
        }
    }

    #[test]
    fn ordered_model_shape_counter_is_shared_across_creation_items() {
        let mut file = model_emission_fixture(3);
        let second = file.emitters[0].particle_items[0];
        file.emitters[0].particle_items.push(second);

        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(quads.len(), 2);
        assert_coordinate(quads[0].position, [0.0, 4.0, 0.0]);
        assert_coordinate(quads[1].position, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn ordered_model_shape_counter_includes_retired_births() {
        let mut file = model_emission_fixture(3);
        file.emitters[0].create_interval = linear_curve(1.0);
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };

        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_coordinate(quads[0].position, [0.0, 0.0, 3.0]);
    }

    #[test]
    fn ordered_model_shape_counter_recovers_probability_accepted_prefix() {
        let mut full = model_emission_fixture(3);
        full.emitters[0].create_interval = linear_curve(1.0);
        full.emitters[0].particle_items[0].create_probability = 50;
        if let Some(AvfxEmitterData::Model(data)) = &mut full.emitters[0].data {
            data.injection_speed = linear_curve(0.0);
        }
        full.particles[0].life = AvfxLife {
            enabled: true,
            value: 100.0,
            ..Default::default()
        };
        let mut current = full.clone();
        current.particles[0].life.value = 0.0;

        let full = VfxRuntime::new(&full);
        let current = VfxRuntime::new(&current);
        let mut checked = false;
        for frame in 1..30 {
            let time = frame as f32 / AVFX_FPS;
            let mut history = Vec::new();
            let mut visible = Vec::new();
            full.sample(time, &mut history);
            current.sample(time, &mut visible);
            if visible.is_empty() || history.len() < 2 || visible[0].position == [0.0, 4.0, 0.0] {
                continue;
            }
            assert_eq!(visible.len(), 1);
            assert_coordinate(visible[0].position, history.last().unwrap().position);
            checked = true;
            break;
        }
        assert!(checked, "fixture did not expose an accepted nonzero prefix");
    }

    #[test]
    fn model_injection_angles_replace_normals_only_for_enabled_on_vertex() {
        let angles = [0.31_f32, -0.47, 0.59];
        // Independent XYZ rotation of +Z, followed by shape Rz(90),
        // parent scale (-2, 3, 0.5), then parent Ry(90).
        let (sx, cx) = angles[0].sin_cos();
        let (sy, cy) = angles[1].sin_cos();
        let (sz, cz) = angles[2].sin_cos();
        let custom = [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy];
        let transformed = [0.5 * custom[2], 3.0 * custom[0], -2.0 * custom[1]];
        let norm = transformed.iter().map(|v| v * v).sum::<f32>().sqrt();
        for children in [false, true] {
            for method in 0..8 {
                for enabled in [false, true] {
                    let mut file = model_emission_fixture(method);
                    file.models[0].emit_vertex_numbers = vec![0];
                    let emitter = &mut file.emitters[0];
                    emitter.any_direction = enabled;
                    emitter.rotation_order = 5;
                    emitter.injection_angle = angles.map(linear_curve);
                    emitter.scale = axis3(Some(-2.0), Some(3.0), Some(0.5));
                    emitter.rotation = axis3(None, Some(std::f32::consts::FRAC_PI_2), None);
                    let Some(AvfxEmitterData::Model(data)) = &mut emitter.data else {
                        unreachable!()
                    };
                    data.rotation.angles[2] = linear_curve(std::f32::consts::FRAC_PI_2);
                    if children {
                        let mut item = emitter.particle_items.remove(0);
                        item.target_index = 1;
                        emitter.emitter_items.push(item);
                        file.emitters.push(AvfxEmitter {
                            emitter_type: Some(EmitterType::Point),
                            particle_items: vec![AvfxEmitterItem {
                                enabled: true,
                                target_index: 0,
                                create_time: 1,
                                create_count: 1,
                                create_probability: 100,
                                parent_influence_coord: 2,
                                parameter_link: -1,
                                ..Default::default()
                            }],
                            ..Default::default()
                        });
                    }
                    let on_vertex = method % 4 >= 2;
                    let origin = if on_vertex { [0.0, 6.0, 0.0] } else { [0.0; 3] };
                    let direction = if on_vertex && enabled {
                        transformed.map(|v| v / norm)
                    } else if on_vertex {
                        [0.0, 0.0, -1.0]
                    } else {
                        [0.0, 1.0, 0.0]
                    };
                    for frame in [2.0, 0.0, 2.0] {
                        let mut out = Vec::new();
                        VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                        assert_eq!(out.len(), 1);
                        assert_coordinate(
                            out[0].position,
                            std::array::from_fn(|i| origin[i] + frame * 0.5 * direction[i]),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn model_injection_random_angles_share_first_and_redraw_always_at_birth() {
        for mode in [1, 4] {
            let mut file = model_emission_fixture(3);
            file.models[0].emit_vertex_numbers = vec![0];
            let emitter = &mut file.emitters[0];
            emitter.any_direction = true;
            emitter.create_count = linear_curve(8.0);
            emitter.create_interval = linear_curve(10.0);
            emitter.injection_angle_random[0] = AvfxCurve {
                random_type: mode,
                ..linear_keyframes(&[(0, 0.4), (10, 0.8)])
            };
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(12.0 / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 16);
            let coefficients: Vec<_> = out
                .iter()
                .enumerate()
                .map(|(i, q)| {
                    assert!((q.position[0] - 2.0).abs() < 1e-5);
                    let amplitude = if i < 8 { 0.4 } else { 0.8 };
                    (-q.position[1]).atan2(q.position[2]) / amplitude
                })
                .collect();
            assert!(coefficients.iter().all(|c| (0.0..=1.00001).contains(c)));
            if mode == 1 {
                assert!(
                    coefficients
                        .iter()
                        .all(|c| (c - coefficients[0]).abs() < 1e-5)
                );
            } else {
                assert!(
                    coefficients[..8]
                        .windows(2)
                        .any(|c| (c[0] - c[1]).abs() > 1e-4)
                );
            }
        }
    }

    #[test]
    fn model_random_speed_uses_client_modes_and_birth_amplitudes() {
        for children in [false, true] {
            for mode in 0..8 {
                for amplitude_sign in [-1.0, 1.0] {
                    let mut file = model_emission_fixture(3);
                    file.models[0].emit_vertex_numbers = vec![0];
                    let emitter = &mut file.emitters[0];
                    emitter.create_count = linear_curve(8.0);
                    emitter.create_interval = linear_curve(10.0);
                    let Some(AvfxEmitterData::Model(data)) = &mut emitter.data else {
                        unreachable!()
                    };
                    data.injection_speed = linear_keyframes(&[(0, 0.5), (10, -0.5)]);
                    data.injection_speed_random = AvfxCurve {
                        random_type: mode | 8,
                        ..linear_keyframes(&[(0, amplitude_sign), (10, 2.0 * amplitude_sign)])
                    };
                    if children {
                        let mut item = emitter.particle_items.remove(0);
                        item.target_index = 1;
                        emitter.emitter_items.push(item);
                        file.emitters.push(AvfxEmitter {
                            emitter_type: Some(EmitterType::Point),
                            particle_items: vec![AvfxEmitterItem {
                                enabled: true,
                                target_index: 0,
                                create_time: 1,
                                create_count: 1,
                                create_probability: 100,
                                parent_influence_coord: 2,
                                parameter_link: -1,
                                ..Default::default()
                            }],
                            ..Default::default()
                        });
                    }
                    let mut out = Vec::new();
                    VfxRuntime::new(&file).sample(12.0 / AVFX_FPS, &mut out);
                    assert_eq!(out.len(), 16);
                    let coefficients: Vec<_> = out
                        .iter()
                        .enumerate()
                        .map(|(i, quad)| {
                            assert!((quad.position[0] - 2.0).abs() < 1e-5);
                            assert!(quad.position[2].abs() < 1e-5);
                            let (age, base, amplitude) = if i < 8 {
                                (12.0, 0.5, amplitude_sign)
                            } else {
                                (2.0, -0.5, 2.0 * amplitude_sign)
                            };
                            (quad.position[1] / age - base) / amplitude
                        })
                        .collect();
                    if matches!(mode, 3..=5) {
                        let (low, high) = match mode {
                            3 => (-1.0, 1.0),
                            4 => (0.0, 1.0),
                            _ => (-1.0, 0.0),
                        };
                        assert!(
                            coefficients
                                .iter()
                                .all(|v| *v >= low - 1e-5 && *v <= high + 1e-5)
                        );
                        assert!(
                            coefficients[..8]
                                .windows(2)
                                .any(|v| (v[0] - v[1]).abs() > 1e-4)
                        );
                    } else {
                        // Model's constructor initializes the speed First byte to zero.
                        assert!(coefficients.iter().all(|v| v.abs() < 1e-5));
                    }
                }
            }
        }
    }

    #[test]
    fn model_birth_rotation_redraws_always_and_keeps_position_normal_coherent() {
        for mode in [1, 4] {
            let mut file = model_emission_fixture(3);
            file.models[0].emit_vertex_numbers = vec![0];
            file.emitters[0].create_count = linear_curve(8.0);
            let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
                unreachable!()
            };
            data.rotation.angles_random[2] = AvfxCurve {
                random_type: mode,
                ..linear_curve(0.8)
            };
            let runtime = VfxRuntime::new(&file);
            let mut birth = Vec::new();
            let mut later = Vec::new();
            runtime.sample(0.0, &mut birth);
            runtime.sample(2.0 / AVFX_FPS, &mut later);
            assert_eq!(birth.len(), 8);
            assert_eq!(later.len(), 8);
            for (a, b) in birth.iter().zip(&later) {
                let [x, y, z] = a.position;
                assert!((x.hypot(y) - 2.0).abs() < 1e-5);
                assert!(z.abs() < 1e-5);
                assert_coordinate(b.position, [x - y * 0.5, y + x * 0.5, 0.0]);
            }
            let varies = birth
                .windows(2)
                .any(|v| (v[0].position[1] - v[1].position[1]).abs() > 1e-4);
            assert_eq!(varies, mode == 4);
        }
    }

    #[test]
    fn model_shape_fields_use_client_byte_widths() {
        for method in 0..8 {
            let mut file = model_emission_fixture(method);
            file.models[0].emit_vertex_numbers = vec![0];
            let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
                unreachable!()
            };
            data.rotation.angles = [0.3, -0.7, 1.1].map(linear_curve);
            let mut reference = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut reference);
            assert_eq!(reference.len(), 1);
            for high_bits in [256, -256] {
                let mut alias = file.clone();
                let Some(AvfxEmitterData::Model(data)) = &mut alias.emitters[0].data else {
                    unreachable!()
                };
                data.model_index += high_bits;
                data.generate_method += high_bits;
                data.rotation.order += high_bits;
                let mut actual = Vec::new();
                VfxRuntime::new(&alias).sample(2.0 / AVFX_FPS, &mut actual);
                assert_eq!(actual, reference);
            }
        }
    }

    #[test]
    fn model_random_births_evaluate_speed_then_angles_then_low16_vertex_selection() {
        for method in [0, 2, 4, 6] {
            let mut file = model_emission_fixture(method);
            let Some(AvfxEmitterData::Model(data)) = &mut file.emitters[0].data else {
                unreachable!()
            };
            data.injection_speed_random = AvfxCurve {
                random_type: 4,
                ..linear_curve(1.0)
            };
            data.rotation.angles_random[2] = AvfxCurve {
                random_type: 4,
                ..linear_curve(0.8)
            };
            let runtime = VfxRuntime::new(&file);
            let mut calls = 0;
            runtime.sample_ambient(0.0, &mut |ctx, _, _| {
                calls += 1;
                for seed in 0..16 {
                    let mut draws = SplitMix64::seeded(seed, 0, 0, 0);
                    let speed = 0.5 + f32::from(draws.next_u64() as u16) / 65535.0;
                    let angle = 0.8 * f32::from(draws.next_u64() as u16) / 65535.0;
                    let ordinal = 3 * usize::from(draws.next_u64() as u16) / 65536;
                    let vertex = file.models[0].emit_vertices[[2, 0, 1][ordinal]];
                    let (s, c) = angle.sin_cos();
                    let rotate = |[x, y, z]: [f32; 3]| [c * x - s * y, s * x + c * y, z];
                    let mut rng = SplitMix64::seeded(seed, 0, 0, 0);
                    let (position, direction, actual_speed, _) = runtime
                        .sample_spawn_shape(ctx.emitter_animation, 0.0, 0, &mut rng)
                        .unwrap();
                    assert!((actual_speed - speed).abs() < 1e-6);
                    if method & 2 == 0 {
                        assert_coordinate(position, [0.0; 3]);
                        assert_coordinate(direction, rotate(vertex.position));
                    } else {
                        assert_coordinate(position, rotate(vertex.position));
                        assert_coordinate(direction, rotate(vertex.normal));
                    }
                }
            });
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn model_shape_optional_binding_uses_current_shape_and_parent_without_rewinding() {
        for method in [1, 3, 5, 7] {
            for follow in [false, true] {
                for scale in [[2.0, 3.0, 4.0], [-2.0, 3.0, 4.0], [0.0, 3.0, 4.0]] {
                    let mut file = model_emission_fixture(method);
                    file.models[0].emit_vertex_numbers = vec![0];
                    let emitter = &mut file.emitters[0];
                    emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                    emitter.position.x = Some(linear_keyframes(&[(0, 1.0), (20, 5.0)]));
                    emitter.scale = axis3(Some(scale[0]), Some(scale[1]), Some(scale[2]));
                    let AvfxEmitterData::Model(data) = emitter.data.as_mut().unwrap() else {
                        unreachable!()
                    };
                    data.injection_speed = linear_curve(0.0);
                    data.rotation.angles[2] =
                        linear_keyframes(&[(0, 0.0), (20, std::f32::consts::FRAC_PI_2)]);
                    let item = &mut emitter.particle_items[0];
                    item.parent_influence_coord = 1;
                    item.influence_coord_pos = follow;
                    file.particles[0].loop_start = 0;
                    file.particles[0].loop_end = 4;
                    let runtime = VfxRuntime::new(&file);
                    for frame in [20.0, 0.0, 10.0, 20.0] {
                        let mut quads = Vec::new();
                        runtime.sample(frame / AVFX_FPS, &mut quads);
                        assert_eq!(quads.len(), 1);
                        let age = if follow { frame } else { 0.0 };
                        let angle = age / 20.0 * std::f32::consts::FRAC_PI_2;
                        let on_vertex = method % 4 == 3;
                        let expected = [
                            1.0 + age / 5.0
                                + if on_vertex {
                                    2.0 * angle.cos() * scale[0]
                                } else {
                                    0.0
                                },
                            2.0 + if on_vertex {
                                2.0 * angle.sin() * scale[1]
                            } else {
                                0.0
                            },
                            3.0,
                        ];
                        assert_coordinate(quads[0].position, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn model_shape_invalid_vertex_selection_does_not_fabricate_births() {
        for numbers in [vec![], vec![-1], vec![3], vec![i16::MIN]] {
            let mut file = model_emission_fixture(3);
            file.models[0].emit_vertex_numbers = numbers;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert!(quads.is_empty());
        }
    }

    #[test]
    fn model_shape_rotation_precedes_nonuniform_parent_for_positions_and_normals() {
        let angles = [0.3_f32, -0.7, 1.1];
        let orders = [
            [0, 1, 2],
            [1, 2, 0],
            [2, 0, 1],
            [0, 2, 1],
            [1, 0, 2],
            [2, 1, 0],
        ];
        for (order, axes) in orders.into_iter().enumerate() {
            let mut file = model_emission_fixture(3);
            file.models[0].emit_vertex_numbers = vec![0];
            file.models[0].emit_vertices[0].position = [2.0, 3.0, 4.0];
            file.models[0].emit_vertices[0].normal = [1.0, 2.0, 3.0];
            file.models[0].draw = Some(Default::default());
            let emitter = &mut file.emitters[0];
            emitter.scale = axis3(Some(-2.0), Some(3.0), Some(0.5));
            emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
            let AvfxEmitterData::Model(data) = emitter.data.as_mut().unwrap() else {
                unreachable!()
            };
            data.rotation.order = order as i32;
            data.rotation.angles = angles.map(linear_curve);
            let reference = |mut point: [f32; 3]| {
                for axis in axes {
                    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                    let (sin, cos) = angles[axis].sin_cos();
                    let (x, y) = (point[a], point[b]);
                    point[a] = cos * x - sin * y;
                    point[b] = sin * x + cos * y;
                }
                [-3.0 * point[1], -2.0 * point[0], 0.5 * point[2]]
            };
            let origin = reference([2.0, 3.0, 4.0]);
            let normal = reference([1.0, 2.0, 3.0]);
            let length = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
            let expected = std::array::from_fn(|i| origin[i] + normal[i] / length);
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 1);
            assert_coordinate(quads[0].position, expected);
            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(2.0 / AVFX_FPS, &mut meshes);
            assert_eq!(meshes.len(), 1);
            assert_coordinate(meshes[0].position, expected);
        }
    }

    #[test]
    fn model_shape_random_rotation_is_shared_and_stable_between_birth_and_binding() {
        let mut file = model_emission_fixture(3);
        file.models[0].emit_vertex_numbers = vec![0];
        let emitter = &mut file.emitters[0];
        emitter.create_count = linear_curve(3.0);
        emitter.particle_items[0].parent_influence_coord = 1;
        let AvfxEmitterData::Model(data) = emitter.data.as_mut().unwrap() else {
            unreachable!()
        };
        data.injection_speed = linear_curve(0.0);
        data.rotation.angles = [0.3, 0.5, 0.8].map(linear_curve);
        data.rotation.angles_random = [0.6, 0.9, 0.4].map(linear_curve);
        file.particles[0].loop_end = 4;
        let mut reference = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut reference);
        assert_eq!(reference.len(), 3);
        assert_ne!(reference[0].position, [2.0, 0.0, 0.0]);
        for follow in [true, false] {
            file.emitters[0].particle_items[0].influence_coord_pos = follow;
            let runtime = VfxRuntime::new(&file);
            for frame in [20.0, 0.0, 5.0, 20.0] {
                let mut quads = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut quads);
                assert_eq!(quads.len(), 3);
                for quad in quads {
                    assert_coordinate(quad.position, reference[0].position);
                }
            }
        }
    }

    #[test]
    fn model_shape_binding_for_powder_uses_each_child_birth_time() {
        let mut file = model_emission_fixture(3);
        file.models[0].emit_vertex_numbers = vec![0];
        let emitter = &mut file.emitters[0];
        emitter.position.x = Some(linear_keyframes(&[(0, 0.0), (20, 4.0)]));
        emitter.particle_items[0].parent_influence_coord = 1;
        emitter.particle_items[0].influence_coord_pos = true;
        let AvfxEmitterData::Model(data) = emitter.data.as_mut().unwrap() else {
            unreachable!()
        };
        data.injection_speed = linear_curve(0.0);
        data.rotation.angles[2] = linear_keyframes(&[(0, 0.0), (20, std::f32::consts::FRAC_PI_2)]);
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Powder);
        particle.simple_anim_enable = true;
        particle.loop_end = 4;
        particle.simple.as_mut().unwrap().injection_model_index = -1;
        let runtime = VfxRuntime::new(&file);
        for frame in [30.0, 20.0, 0.0, 30.0] {
            let mut quads = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), if frame == 0.0 { 1 } else { 3 });
            for (i, quad) in quads.into_iter().enumerate() {
                let age = i as f32 * 10.0;
                let angle = age / 20.0 * std::f32::consts::FRAC_PI_2;
                assert_coordinate(
                    quad.position,
                    [age / 5.0 + 2.0 * angle.cos(), 2.0 * angle.sin(), 0.0],
                );
            }
        }
    }

    #[test]
    fn normalized_coordinate_modes_match_without_scene_ground_queries() {
        let mut file = powder_emission_fixture();
        file.emitters[0].rotation = axis3(Some(0.7), Some(-0.4), Some(0.9));
        file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        file.emitters[0].position = axis3(Some(2.0), Some(-3.0), Some(4.0));
        file.emitters[0].particle_items[0].influence_coord_rot = true;
        file.emitters[0].particle_items[0].influence_coord_pos = true;
        file.particles[0].simple_anim_enable = false;
        file.particles[0].position = axis3(Some(0.2), Some(0.3), Some(0.4));
        for kind in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::Windmill,
        ] {
            file.particles[0].particle_type = Some(kind);
            for (alias, mode) in [(4, 2), (5, 3), (6, 0), (7, 1), (9, 8)] {
                file.emitters[0].particle_items[0].parent_influence_coord = alias;
                let mut actual = Vec::new();
                VfxRuntime::new(&file).sample(0.5, &mut actual);
                file.emitters[0].particle_items[0].parent_influence_coord = mode;
                let mut expected = Vec::new();
                VfxRuntime::new(&file).sample(0.5, &mut expected);
                assert!(!actual.is_empty());
                assert_eq!(actual, expected, "{kind:?}: {alias} -> {mode}");
            }
        }
    }

    #[test]
    fn optional_inheritance_billboards_use_parent_z_and_scalar_scale() {
        let mut file = powder_emission_fixture();
        file.emitters[0].rotation = axis3(Some(0.7), Some(-0.4), Some(0.9));
        file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        let item = &mut file.emitters[0].particle_items[0];
        item.parent_influence_coord = 1;
        item.influence_coord_rot = true;
        item.influence_coord_scale = true;
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.particle_type = Some(ParticleType::Quad);
        particle.rotation = axis3(Some(0.2), Some(0.3), Some(0.4));
        for facing in [5, 6, 9] {
            file.particles[0].rotation_direction_base = facing;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            assert_eq!(quads[0].size, [-1.0, 1.5]);
            assert_eq!(quads[0].parent_basis, VFX_IDENTITY_BASIS);
            for axis in VFX_IDENTITY_BASIS {
                assert_coordinate(
                    quat_rotate(quads[0].orientation, axis),
                    quat_rotate(quat_from_euler(0, [0.2, 0.3, 1.3]), axis),
                );
            }
        }
    }

    #[test]
    fn optional_inheritance_disables_components_and_adds_euler_before_rotation() {
        let mut file = powder_emission_fixture();
        file.models[0].draw = Some(Default::default());
        file.emitters[0].rotation = axis3(Some(0.7), Some(-0.4), Some(0.9));
        file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        file.emitters[0].particle_items[0].parent_influence_coord = 1;
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.particle_type = Some(ParticleType::LightModel);
        particle.data = AvfxParticleData::LightModel { model_index: 0 };
        particle.rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        particle.rotation = axis3(Some(-0.2), Some(0.6), Some(0.3));
        particle.scale = axis3(Some(0.5), Some(2.0), Some(4.0));
        for rotate in [false, true] {
            for scale in [false, true] {
                for order in 0..6 {
                    file.emitters[0].particle_items[0].influence_coord_rot = rotate;
                    file.emitters[0].particle_items[0].influence_coord_scale = scale;
                    file.particles[0].rotation_order = order;
                    let mut meshes = Vec::new();
                    VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
                    assert_eq!(meshes.len(), 1);
                    let mesh = &meshes[0];
                    let expected_rotation = quat_from_euler(
                        order,
                        if rotate {
                            [0.5, 0.2, 1.2]
                        } else {
                            [-0.2, 0.6, 0.3]
                        },
                    );
                    let expected_scale = if scale {
                        [-1.0, 6.0, 0.0]
                    } else {
                        [0.5, 2.0, 4.0]
                    };
                    for axis in 0..3 {
                        let mut vertex = [0.0; 3];
                        vertex[axis] = mesh.scale[axis];
                        let actual = basis_transform(
                            mesh.parent_basis,
                            quat_rotate(mesh.orientation, vertex),
                        );
                        vertex[axis] = expected_scale[axis];
                        assert_coordinate(actual, quat_rotate(expected_rotation, vertex));
                    }
                }
            }
        }
    }

    #[test]
    fn optional_inheritance_position_switch_uses_current_point_or_cone_origin() {
        for cone in [false, true] {
            let mut file = powder_emission_fixture();
            let emitter = &mut file.emitters[0];
            emitter.position = axis3(Some(3.0), None, None);
            let curve = emitter.position.x.as_mut().unwrap();
            curve.keys.push(AvfxCurveKey {
                time: 10,
                z: 13.0,
                ..curve.keys[0]
            });
            emitter.particle_items[0].parent_influence_coord = 1;
            emitter.data = cone.then(|| {
                AvfxEmitterData::Cone(ConeEmitterData {
                    outer_size: linear_curve(2.0),
                    inner_size: linear_curve(2.0),
                    ..Default::default()
                })
            });
            file.particles[0].simple_anim_enable = false;
            file.particles[0].particle_type = Some(ParticleType::Quad);
            file.particles[0].position = axis3(Some(1.0), None, None);
            let mut birth = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut birth);
            for follows in [false, true] {
                file.emitters[0].particle_items[0].influence_coord_pos = follows;
                let runtime = VfxRuntime::new(&file);
                for frame in [10.0, 2.0, 0.0] {
                    let mut quads = Vec::new();
                    runtime.sample(frame / AVFX_FPS, &mut quads);
                    assert_eq!(quads.len(), 1);
                    // Cone's +0xc8 getter returns its center, not its birth point.
                    let expected = if follows {
                        [4.0 + frame, 0.0, 0.0]
                    } else {
                        birth[0].position
                    };
                    assert_coordinate(quads[0].position, expected);
                }
            }
        }
    }

    #[test]
    fn optional_inheritance_ccot_transforms_combined_pos_and_motion() {
        let mut file = powder_emission_fixture();
        let emitter = &mut file.emitters[0];
        emitter.position = axis3(Some(10.0), Some(20.0), Some(30.0));
        emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        emitter.scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(1.0),
            ..upward_cone()
        }));
        emitter.particle_items[0].parent_influence_coord = 1;
        emitter.particle_items[0].influence_coord_scale = true;
        emitter.particle_items[0].influence_coord_rot = true;
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.particle_type = Some(ParticleType::Quad);
        particle.rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        particle.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        particle.scale = axis3(Some(0.5), Some(2.0), Some(4.0));
        // World injection is -X. After two frames Pos + motion = (-1,2,3).
        let translations = [
            [-1.0, 2.0, 3.0],
            [-12.0, 1.0, 0.0],
            [1.0, 12.0, 0.0],
            [2.0, -6.0, 0.0],
            [-2.0, -1.0, 3.0],
            [-1.0, 2.0, 3.0],
        ];
        for order in 0..6 {
            file.particles[0].coord_compute_order = order;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut quads);
            let [x, y, z] = translations[order as usize];
            assert_coordinate(quads[0].position, [10.0 + x, 20.0 + y, 30.0 + z]);
        }
    }

    #[test]
    fn ccot_emitter_orders_transform_translation_and_geometry_separately() {
        // Rz(90), S(-2,3,0), P(1,2,3), vertex(2,1,4).
        let translations = [
            [1.0, 2.0, 3.0],
            [-6.0, -2.0, 0.0],
            [-2.0, 6.0, 0.0],
            [4.0, 3.0, 0.0],
            [-2.0, 1.0, 3.0],
            [1.0, 2.0, 3.0],
        ];
        let base = EmitterBase {
            linear: [[2.0, 0.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, -1.0]],
            ..EmitterBase::root([10.0, 20.0, 30.0])
        };
        for order in 0..6 {
            let emitter = AvfxEmitter {
                coord_compute_order: order,
                position: axis3(Some(1.0), Some(2.0), Some(3.0)),
                rotation: axis3(None, None, Some(std::f32::consts::FRAC_PI_2)),
                scale: axis3(Some(-2.0), Some(3.0), Some(0.0)),
                gravity: linear_curve(2.0),
                ..Default::default()
            };
            let animation = EmitterAnimation {
                emitter: &emitter,
                clock: InstanceClock::root(&emitter, -1.0),
                base,
                binder_base: base,
                direction: [0.0; 3],
                parent: None,
                sampled_parent: None,
                parent_components: ParentComponents::None,
                creation_angle: [0.0; 3],
                random: SplitMix64::seeded(0, 0, 0, 0),
            };
            let world = animation.at(2.0).world();
            let [x, y, z] = translations[order as usize];
            assert_coordinate(
                world.position,
                [10.0 + 2.0 * x + y, 24.0 + 3.0 * y, 30.0 - z],
            );
            let [vx, vy, vz] = if matches!(order, 2 | 3 | 5) {
                [2.0, 6.0, 0.0]
            } else {
                [-3.0, -4.0, 0.0]
            };
            assert_coordinate(
                world.transform_point([2.0, 1.0, 4.0]),
                [
                    10.0 + 2.0 * (x + vx) + y + vy,
                    24.0 + 3.0 * (y + vy),
                    30.0 - z - vz,
                ],
            );
        }
    }

    #[test]
    fn ccot_particles_apply_order_to_pos_plus_motion_before_birth_origin() {
        let mut file = powder_emission_fixture();
        file.models[0].draw = Some(Default::default());
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        particle.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        particle.scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        particle.gravity = linear_curve(2.0);
        let runtime = VfxRuntime::new(&file);
        let animation = EmitterAnimation {
            emitter: &file.emitters[0],
            clock: InstanceClock::root(&file.emitters[0], -1.0),
            base: EmitterBase::root([0.0; 3]),
            binder_base: EmitterBase::root([0.0; 3]),
            direction: [0.0; 3],
            parent: None,
            sampled_parent: None,
            parent_components: ParentComponents::None,
            creation_angle: [0.0; 3],
            random: SplitMix64::seeded(0, 0, 0, 0),
        };
        // Pos + 2 frames of +Y motion is (1,4,3). Origin and gravity are outside CCOT.
        let translations = [
            [1.0, 4.0, 3.0],
            [-12.0, -2.0, 0.0],
            [-2.0, 12.0, 0.0],
            [8.0, 3.0, 0.0],
            [-4.0, 1.0, 3.0],
            [1.0, 4.0, 3.0],
        ];
        let mut saved_ctx = None;
        for mode in [0, 1, 2, 3] {
            let item = AvfxEmitterItem {
                parent_influence_coord: mode,
                ..Default::default()
            };
            let ctx = SpawnContext {
                cached_color: None,
                cached_xyz: None,
                cached_textures: None,
                age: 2.0,
                total_age: 2.0,
                clock: InstanceClock::new(-1.0, 1.0, 0, 0),
                gravity_offset: None,
                client_injection: None,
                shape_binding: ShapeBinding::Center,
                motion: motion::InjectionMotion::new(
                    &item,
                    animation.at(0.0).world(),
                    [7.0, 8.0, 9.0],
                    motion::InjectionDirection::World([0.0, 1.0, 0.0]),
                    [0.0, 1.0, 0.0],
                ),
                emitter: animation.at(0.0),
                emitter_now: animation.at(2.0),
                binder_base: animation.binder_base,
                emitter_animation: &animation,
                root_transform_history: None,
                spawn_loop_age: 0.0,
                parent_color: ParentColor::None,
                revised_color: [1.0; 3],
                parent_components: ParentComponents::None,
                creation_angle: [0.0; 3],
                seed: 0,
                create_index: 0,
            };
            saved_ctx = Some(ctx);
            for order in 0..6 {
                let mut particle = file.particles[0].clone();
                particle.coord_compute_order = order;
                particle.particle_type = Some(ParticleType::Quad);
                let mut quads = Vec::new();
                runtime.push_quad(&ctx, &item, &particle, &mut quads);
                let q = quads[0];
                let [x, y, z] = translations[order as usize];
                assert_coordinate(q.position, [7.0 + x, 12.0 + y, 9.0 + z]);
                let expected_x = if matches!(order, 2 | 3 | 5) {
                    [0.0, 1.5, 0.0]
                } else {
                    [0.0, -1.0, 0.0]
                };
                assert_coordinate(
                    basis_transform(
                        q.parent_basis,
                        quat_rotate(q.orientation, [q.size[0], 0.0, 0.0]),
                    ),
                    expected_x,
                );
                for kind in [ParticleType::Model, ParticleType::LightModel] {
                    particle.particle_type = Some(kind);
                    particle.data = if kind == ParticleType::Model {
                        AvfxParticleData::Model {
                            model_number_random_value: 0,
                            model_number_random_type: 0,
                            model_number_random_interval: 0,
                            fresnel_type: 0,
                            directional_light_type: 0,
                            point_light_type: 0,
                            is_lightning: false,
                            is_morph: false,
                            model_indexes: vec![0],
                            animation_number: None,
                            morph: None,
                            fresnel_curve: None,
                            fresnel_curve_random: None,
                            fresnel_rotation: None,
                            color_begin: Default::default(),
                            color_end: Default::default(),
                        }
                    } else {
                        AvfxParticleData::LightModel { model_index: 0 }
                    };
                    let mut meshes = Vec::new();
                    runtime.push_mesh_instance(&ctx, &item, &particle, &mut meshes);
                    assert_eq!(meshes[0].position, q.position);
                    assert_coordinate(
                        basis_transform(
                            meshes[0].parent_basis,
                            quat_rotate(
                                meshes[0].orientation,
                                [0.5 * meshes[0].scale[0], 0.0, 0.0],
                            ),
                        ),
                        expected_x,
                    );
                }
            }
        }
        let binder_rotation = quat_from_euler(0, [std::f32::consts::FRAC_PI_2, 0.0, 0.0]);
        let mut binder_ctx = saved_ctx.expect("inheritance fixture context");
        binder_ctx.binder_base = EmitterBase {
            orientation: binder_rotation,
            linear: rotation_scale_basis(binder_rotation, [1.0; 3]),
            ..EmitterBase::root([99.0, 99.0, 99.0])
        };
        let binder_item = AvfxEmitterItem {
            parent_influence_coord: 1,
            influence_coord_binder: true,
            ..Default::default()
        };
        let (binder_parent, _) = influence_transform(&binder_item, &binder_ctx);
        assert_eq!(binder_parent.position, [0.0; 3]);
        assert_coordinate(
            basis_transform(binder_parent.linear, [0.0, 0.0, 1.0]),
            [0.0, -1.0, 0.0],
        );
    }

    #[test]
    fn ccot_powder_birth_geometry_and_current_axis_lengths_use_same_order() {
        let mut file = powder_emission_fixture();
        let particle = &mut file.particles[0];
        particle.coord_compute_order = 3;
        particle.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        particle.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        particle.scale = axis3(Some(-2.0), Some(3.0), Some(0.0));
        particle.simple.as_mut().unwrap().scale_by_parent = true;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(20.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 3);
        let packed = powder::packed_position([1.0, 1.0, 3.0], false);
        for quad in quads {
            // S * R * (packed source + Pos(1,2,3)).
            assert_coordinate(
                quad.position,
                [4.0 + 2.0 * packed[0], 3.0 + 3.0 * packed[1], 0.0],
            );
            assert!((quad.size[0] - 3.0).abs() < 1e-5);
            assert!((quad.size[1] - 2.0).abs() < 1e-5);
        }
    }

    #[test]
    fn ccot_local_direction_transforms_pos_motion_and_fixed_geometry() {
        let mut file = powder_emission_fixture();
        file.models[0].emit_vertices[0].normal = [1.0, 0.0, 0.0];
        file.models[0].emit_vertices[0].position = [1.0, 2.0, 3.0];
        file.emitters[0].data = Some(AvfxEmitterData::Model(crate::avfx::ModelEmitterData {
            model_index: 0,
            injection_speed: linear_curve(1.0),
            generate_method: 3,
            ..Default::default()
        }));
        file.emitters[0].position = axis3(Some(10.0), Some(20.0), Some(30.0));
        file.emitters[0].rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        file.emitters[0].scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Quad);
        particle.simple_anim_enable = false;
        particle.coord_compute_order = 1;
        particle.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        particle.scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        particle.gravity = linear_curve(2.0);
        for (mode, position, x, y) in [
            (0, [-2.0, 46.0, 40.0], [0.0, 0.0, -1.0], [-1.5, 0.0, 0.0]),
            (1, [-2.0, 46.0, 40.0], [0.0, 0.0, -1.0], [-1.5, 0.0, 0.0]),
            (2, [-14.0, 58.0, 34.0], [0.0, 0.0, -4.0], [-4.5, 0.0, 0.0]),
            (3, [-14.0, 58.0, 34.0], [0.0, 0.0, -4.0], [-4.5, 0.0, 0.0]),
        ] {
            file.emitters[0].particle_items[0].parent_influence_coord = mode;
            file.emitters[0].particle_items[0].local_direction = 1;
            let runtime = VfxRuntime::new(&file);
            let mut quads = Vec::new();
            runtime.sample(2.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 1);
            let q = quads[0];
            assert_coordinate(q.position, position);
            assert_coordinate(
                basis_transform(
                    q.parent_basis,
                    quat_rotate(q.orientation, [q.size[0], 0.0, 0.0]),
                ),
                x,
            );
            assert_coordinate(
                basis_transform(
                    q.parent_basis,
                    quat_rotate(q.orientation, [0.0, q.size[1], 0.0]),
                ),
                y,
            );
            runtime.sample(20.0 / AVFX_FPS, &mut quads);
            runtime.sample(2.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads[0], q);
        }
    }

    #[test]
    fn runtime_spline_drives_quad_and_mesh_scale_and_alpha() {
        let mut file = fixture_file();
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].create_interval = linear_curve(1000.0);
        file.emitters[0].particle_items[0].create_count = 1;
        let mut curve = linear_curve(0.0);
        curve.keys[0].x = 1.0;
        curve.keys.push(AvfxCurveKey {
            time: 8,
            interpolation: AvfxCurveKey::INTERPOLATION_SPLINE,
            x: 0.0,
            y: 0.0,
            z: 1.0,
        });
        file.particles[0].scale.x = Some(curve.clone());
        file.particles[0].color.alpha = Some(curve);
        let quad_runtime = VfxRuntime::new(&file);
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        file.particles[0].particle_type = Some(ParticleType::LightModel);
        file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
        let mesh_runtime = VfxRuntime::new(&file);
        for (age, expected) in [(4.0, 0.625), (2.0, 0.296875), (8.0, 1.0), (2.0, 0.296875)] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            quad_runtime.sample(age / AVFX_FPS, &mut quads);
            mesh_runtime.sample_mesh(age / AVFX_FPS, &mut meshes);
            assert_eq!((quads.len(), meshes.len()), (1, 1));
            assert_eq!(quads[0].size[0], expected * 0.5);
            assert_eq!(meshes[0].scale[0], expected);
            assert_eq!(quads[0].color[3], expected);
            assert_eq!(meshes[0].color[3], expected);
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
        // （k=2 落在 emitter 寿命边界；CrTm=0 每事件只取 CrC=2）。
        assert_eq!(quads_a.len(), 6);
        let quad = &quads_a[0];
        // 颜色 = Col.RGB（HDR 无 Bri），alpha = Col.A。
        assert!((quad.color[2] - 0.8).abs() < 1.0e-6);
        assert!((quad.color[3] - 0.9).abs() < 1.0e-6);
        // TC1 TLst 优先。
        assert_eq!(quad.texture_indexes[0], 2);
        // 尺寸 = 半宽（scale 0.2 → 0.1）。
        assert!((quad.size[0] - 0.1).abs() < 1.0e-6);
        // 未设置粒子局部旋转时，四元数为单位。
        assert_eq!(quad.orientation, [0.0, 0.0, 0.0, 1.0]);
        // 发射器 Pos.Y = 0.5 生效。
        assert!(quad.position[1] >= 0.5 - 1.0e-5);
    }

    #[test]
    fn quad_and_powder_preserve_particle_source_and_culling_mode() {
        for particle_type in [ParticleType::Quad, ParticleType::Powder] {
            for cull_mode in [0, 1, 2, 3, 4, 99] {
                let mut file = fixture_file();
                let particle = &mut file.particles[0];
                particle.particle_type = Some(particle_type);
                particle.culling_type = cull_mode;
                particle.simple_anim_enable = true;
                particle.simple = Some(crate::avfx::AvfxParticleSimple {
                    create_interval: 1,
                    create_count: 1,
                    create_interval_life: 30,
                    scale_start: [1.0; 2],
                    scale_end: [1.0; 2],
                    scale_rand_x: [1.0; 2],
                    scale_rand_y: [1.0; 2],
                    colors: [[255; 4]; 4],
                    ..Default::default()
                });
                file.particles.push(file.particles[0].clone());
                file.emitters[0].particle_items[0].target_index = 1;
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(0.0, &mut quads);
                assert!(!quads.is_empty(), "{particle_type:?}");
                assert!(quads.iter().all(|quad| quad.cull_mode == cull_mode));
                assert!(quads.iter().all(|quad| {
                    quad.particle_type == Some(particle_type) && quad.particle_index == 1
                }));
            }
        }
    }

    #[test]
    fn tree_billboard_and_nonzero_simple_pivot_are_separate_particle_paths() {
        let mut file = fixture_file();
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Quad);
        particle.rotation_direction_base = crate::avfx::rotation_direction_base::TREE_BILLBOARD;
        particle.simple_anim_enable = true;
        particle.simple = Some(crate::avfx::AvfxParticleSimple {
            pivot: [0.5, -0.25],
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(quads.iter().all(|quad| quad.pivot == [0.0; 2]));

        let mut file = powder_emission_fixture();
        file.particles[0].simple.as_mut().unwrap().pivot = [0.5, -0.25];
        for base_direction_type in [0, 1, 2, 3, 4, 9] {
            file.particles[0]
                .simple
                .as_mut()
                .unwrap()
                .base_direction_type = base_direction_type;
            quads.clear();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert!(!quads.is_empty(), "SBDT={base_direction_type}");
            assert!(quads.iter().all(|quad| {
                quad.pivot == [0.5, -0.25]
                    && quad.rotation_direction_base
                        != crate::avfx::rotation_direction_base::TREE_BILLBOARD
            }));
        }
    }

    #[test]
    fn powder_linked_scale_copies_the_quantized_x_multiplier_to_y() {
        let sample = |range_x, range_y, linked| {
            let mut file = powder_emission_fixture();
            let simple = file.particles[0].simple.as_mut().unwrap();
            simple.create_count = 64;
            simple.create_interval_count = 64;
            simple.scale_start = [2.0, 3.0];
            simple.scale_end = [2.0, 3.0];
            simple.scale_rand_x = range_x;
            simple.scale_rand_y = range_y;
            simple.scale_random_link = linked;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 64);
            quads
        };
        for range_x in [[-2.0, 3.0], [3.0, -2.0], [2.0, 2.0], [0.0, 1.0e-8]] {
            let baseline = sample(range_x, [0.0; 2], true);
            let changed_y = sample(range_x, [-100.0, 100.0], true);
            assert_eq!(baseline, changed_y, "bSRL ignores SRY0/1");
            for quad in baseline {
                assert!((quad.size[0] * 1.5 - quad.size[1]).abs() < 1.0e-6);
                let packed = quad.size[0] * 25.0;
                assert!((packed - packed.round()).abs() < 1.0e-4);
                assert!((-128.0001..=127.0001).contains(&packed));
            }
        }
        assert!(
            sample([0.0, 1.0], [0.0, 1.0], false)
                .iter()
                .any(|q| (q.size[0] * 1.5 - q.size[1]).abs() > 0.1)
        );
    }

    #[test]
    fn powder_simple_rotation_uses_packed_angles_and_fixed_zxy_order() {
        let unit = std::f32::consts::TAU / 1024.0;
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.rotation_start = [256.9 * unit, -512.9 * unit, 768.9 * unit];
        simple.rotation_add = [64.9 * unit, -32.9 * unit, 16.9 * unit];
        for order in 0..6 {
            file.particles[0].rotation_order = order;
            for age in [0.0_f32, 0.75, 17.5] {
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(age / AVFX_FPS, &mut quads);
                assert_eq!(quads.len(), 1);
                let angles: [f32; 3] = std::array::from_fn(|axis| {
                    let initial = [256, -512, 768][axis];
                    let velocity = [64.0, -32.0, 16.0][axis];
                    ((initial + (velocity * age).trunc() as i32) & 1023) as f32 * unit
                });
                let [x, y, z] = angles;
                for [px, py, pz] in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                    let after_z = [px * z.cos() - py * z.sin(), px * z.sin() + py * z.cos(), pz];
                    let after_x = [
                        after_z[0],
                        after_z[1] * x.cos() - pz * x.sin(),
                        after_z[1] * x.sin() + pz * x.cos(),
                    ];
                    let expected = [
                        after_x[0] * y.cos() + after_x[2] * y.sin(),
                        after_x[1],
                        -after_x[0] * y.sin() + after_x[2] * y.cos(),
                    ];
                    let actual = quat_rotate(quads[0].orientation, [px, py, pz]);
                    for axis in 0..3 {
                        assert!(
                            (actual[axis] - expected[axis]).abs() < 1.0e-5,
                            "order={order}, age={age}: {actual:?} != {expected:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn powder_simple_block_rotations_share_initial_state_and_advance_z() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 4;
        simple.block_num = 4;
        simple.create_interval = 0;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(quads.len(), 4);
        for (quad, expected) in quads.iter().zip([
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ]) {
            let actual = quat_rotate(quad.orientation, [1.0, 0.0, 0.0]);
            for axis in 0..3 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 1.0e-6,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }

    #[test]
    fn powder_simple_scale_uses_signed_bytes_and_full_corner_extents() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.scale_start = [2.0, 3.0];
        simple.scale_end = [2.0, 3.0];
        for (x, y, expected) in [
            (1.039, -0.039, [2.04, -0.06]),
            (2.56, -2.58, [-5.12, 7.62]),
            (5.12, 5.14, [0.0, 0.06]),
        ] {
            let simple = file.particles[0].simple.as_mut().unwrap();
            simple.scale_rand_x = [x; 2];
            simple.scale_rand_y = [y; 2];
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            for (actual, expected) in quads[0].size.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
            }
        }
    }

    #[test]
    fn powder_simple_parent_scale_uses_spawner_matrix_axis_lengths() {
        let mut file = powder_emission_fixture();
        file.emitters[0].scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        let particle = &mut file.particles[0];
        particle.rotation.z = Some(linear_curve(std::f32::consts::FRAC_PI_4));
        let simple = particle.simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.scale_by_parent = true;
        simple.scale_start = [2.0, 3.0];
        simple.scale_end = simple.scale_start;
        // S(2,3,4) * Rz(45 degrees) * S(x,7,11) contains shear.
        for x in [0.0_f32, -5.0, 5.0] {
            file.particles[0].scale = axis3(Some(x), Some(7.0), Some(11.0));
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            let expected = [2.0 * x.abs() * 6.5_f32.sqrt(), 21.0 * 6.5_f32.sqrt()];
            for (actual, expected) in quads[0].size.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
            }
            if x == 0.0 {
                assert_eq!(quads[0].size[0], 0.0);
            }
        }
    }

    #[test]
    fn powder_simple_parent_scale_respects_particle_loop_and_inheritance() {
        let mut file = powder_emission_fixture();
        let mut scale = linear_curve(2.0);
        scale.keys.push(AvfxCurveKey {
            time: 30,
            z: 8.0,
            ..scale.keys[0]
        });
        file.emitters[0].scale.x = Some(scale.clone());
        scale.keys[0].z = 3.0;
        scale.keys[1].z = 9.0;
        file.emitters[0].scale.y = Some(scale);
        let particle = &mut file.particles[0];
        let mut scale = linear_curve(1.0);
        scale.keys.push(AvfxCurveKey {
            time: 20,
            z: 5.0,
            ..scale.keys[0]
        });
        particle.scale.x = Some(scale);
        particle.scale.y = Some(linear_curve(4.0));
        particle.loop_start = 0;
        particle.loop_end = 10;
        particle.simple.as_mut().unwrap().create_count = 1;
        for (mode, expected) in [(3, [4.0, 12.0]), (2, [10.0, 24.0]), (6, [2.0, 4.0])] {
            file.emitters[0].particle_items[0].parent_influence_coord = mode;
            for enabled in [true, false] {
                file.particles[0].simple.as_mut().unwrap().scale_by_parent = enabled;
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(0.5, &mut quads);
                assert_eq!(quads.len(), 1);
                assert_eq!(quads[0].size, if enabled { expected } else { [1.0; 2] });
            }
        }
    }

    #[test]
    fn point_depth_inputs_reach_quad_powder_and_mesh_without_geometric_scale() {
        let mut file = powder_emission_fixture();
        file.models[0].draw = Some(Default::default());
        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.timelines[0].items[0].binder_index = 0;
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            vfx_scale_depth_offset: true,
            transform_scale_depth_offset: true,
            vfx_scale_bias: 9.0,
            ..Default::default()
        });
        file.global.bias_z_max_scale = 4.0;
        file.global.bias_z_max_distance = 8.0;
        file.particles[0].depth_offset = 0.2;
        let points = [crate::avfx::VfxBindPoint {
            id: 3,
            parent_bone: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
        }];
        for kind in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::LightModel,
        ] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].simple_anim_enable = false;
            if kind == ParticleType::LightModel {
                file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            }
            for scale_flag in [false, true] {
                file.binders[0].vfx_scale_enabled = scale_flag;
                for offset_type in [0, 1] {
                    file.particles[0].depth_offset_type = offset_type;
                    let runtime = VfxRuntime::with_bind_points(&file, &points)
                        .with_vfx_scale(2.0)
                        .unwrap()
                        .with_document_scale([9.0, -2.0, 0.5])
                        .unwrap()
                        .with_document_depth_offset(-0.1)
                        .unwrap()
                        .with_camera_position([0.0, 0.0, 4.0])
                        .unwrap();
                    let mut quads = Vec::new();
                    let mut meshes = Vec::new();
                    runtime.sample(0.0, &mut quads);
                    runtime.sample_mesh(0.0, &mut meshes);
                    let offsets: Vec<_> = quads
                        .iter()
                        .map(|q| (q.depth_offset_type, q.depth_offset))
                        .chain(meshes.iter().map(|m| (m.depth_offset_type, m.depth_offset)))
                        .collect();
                    assert_eq!(offsets.len(), 1, "{kind:?}");
                    assert_eq!(
                        offsets[0],
                        (offset_type, -0.4),
                        "{kind:?} bVSc={scale_flag}"
                    );
                }
            }
        }
        assert!(
            VfxRuntime::new(&file)
                .with_camera_position([f32::NAN; 3])
                .is_err()
        );
        assert!(
            VfxRuntime::new(&file)
                .with_document_depth_offset(f32::INFINITY)
                .is_err()
        );
    }

    #[test]
    fn simple_powder_depth_uses_current_spawner_not_child_or_birth_positions() {
        let mut file = powder_emission_fixture();
        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.global.bias_z_max_scale = 4.0;
        file.global.bias_z_max_distance = 8.0;
        file.particles[0].depth_offset = -0.1;
        let mut movement = linear_curve(0.0);
        movement.keys.push(AvfxCurveKey {
            time: 30,
            z: 2.0,
            ..movement.keys[0]
        });
        file.particles[0].position.z = Some(movement);
        for bound in [false, true] {
            file.particles[0].simple.as_mut().unwrap().bind_parent = bound;
            let mut quads = Vec::new();
            VfxRuntime::new(&file)
                .with_camera_position([0.0, 0.0, 4.0])
                .unwrap()
                .sample(15.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 2);
            assert!(quads.iter().all(|q| (q.depth_offset - -0.15).abs() < 1e-6));
            assert!(quads.iter().all(|q| q.position != [0.0, 0.0, 1.0]));
        }
    }

    #[test]
    fn point_document_scale_multiplies_base_getter_independently_of_bdse_and_basis() {
        let mut file = powder_emission_fixture();
        file.timelines[0].items[0].binder_index = 0;
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            transform_scale: 255,
            vfx_scale_enabled: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        });
        let points = [crate::avfx::VfxBindPoint {
            id: 3,
            parent_bone: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
        }];
        let document_scale = [-3.0, 0.5, 0.0];
        for following in [false, true] {
            file.binders[0].following_target_orientation = following;
            for bdse in [false, true] {
                file.binders[0].document_scale_enabled = bdse;
                let reference = VfxRuntime::with_bind_points(&file, &points)
                    .with_vfx_scale(2.0)
                    .unwrap();
                let runtime = reference
                    .clone()
                    .with_document_scale(document_scale)
                    .unwrap();
                assert_eq!(reference.bind_base(0).linear, runtime.bind_base(0).linear);
                let mut before = Vec::new();
                let mut after = Vec::new();
                reference.sample(0.5, &mut before);
                runtime.sample(0.5, &mut after);
                assert!(!before.is_empty());
                assert_eq!(before.len(), after.len());
                for (before, after) in before.into_iter().zip(after) {
                    // The direct Binder root consumes +0x30 before +0x28.
                    // Document scale changes offsets through the auxiliary
                    // matrix, while the target's main matrix stays unchanged.
                    for axis in 0..3 {
                        assert!(
                            (after.position[axis] - before.position[axis] * document_scale[axis])
                                .abs()
                                < 1e-5
                        );
                    }
                    assert_eq!(
                        after.size,
                        [
                            before.size[0] * document_scale[0],
                            before.size[1] * document_scale[1]
                        ]
                    );
                }
            }
        }
        assert!(
            VfxRuntime::new(&file)
                .with_document_scale([1.0, f32::NAN, 1.0])
                .is_err()
        );
        assert!(
            VfxRuntime::new(&file)
                .with_document_scale([f32::INFINITY; 3])
                .is_err()
        );
    }

    #[test]
    fn resolved_document_scale_is_not_applied_twice_to_revised_root_matrix() {
        let mut file = AvfxFile::default();
        file.global.revised_scale = [2.0, 3.0, 4.0];
        let reference = VfxRuntime::new(&file).bind_base(-1);
        let runtime = VfxRuntime::new(&file)
            .with_document_scale([-5.0, 0.0, 7.0])
            .unwrap();
        let actual = runtime.bind_base(-1);
        assert_eq!(actual.scale, [-5.0, 0.0, 7.0]);
        assert_eq!(actual.linear, reference.linear);
        assert_eq!(actual.position, reference.position);
    }

    #[test]
    fn binder_query_scale_reaches_powder_size_without_scaling_target_basis() {
        let mut file = powder_emission_fixture();
        file.timelines[0].items[0].binder_index = 0;
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            transform_scale: 255,
            vfx_scale_enabled: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        });
        let points = [crate::avfx::VfxBindPoint {
            parent_bone: None,
            id: 3,
            translate: [2.0, -3.0, 4.0],
            rotate: [0.0; 3],
        }];
        for following in [false, true] {
            file.binders[0].following_target_orientation = following;
            for by_parent in [false, true] {
                file.particles[0].simple.as_mut().unwrap().scale_by_parent = by_parent;
                let reference = VfxRuntime::with_bind_points(&file, &points);
                let scaled = reference.clone().with_vfx_scale(2.0).unwrap();
                assert_eq!(reference.bind_base(0).linear, scaled.bind_base(0).linear);
                let mut before = Vec::new();
                let mut after = Vec::new();
                reference.sample(0.5, &mut before);
                scaled.sample(0.5, &mut after);
                assert!(!before.is_empty());
                assert_eq!(before.len(), after.len());
                for (before, after) in before.into_iter().zip(after) {
                    for axis in 0..3 {
                        let origin = points[0].translate[axis];
                        let expected = origin + 2.0 * (before.position[axis] - origin);
                        assert!((after.position[axis] - expected).abs() < 1e-5);
                    }
                    assert_eq!(before.parent_basis, after.parent_basis);
                    assert_eq!(after.size, before.size.map(|v| v * 2.0));
                }
            }
        }
        for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(VfxRuntime::new(&file).with_vfx_scale(scale).is_err());
        }
    }

    #[test]
    fn powder_compensated_direction_divides_binder_scale_and_retains_emitter_scale() {
        let mut file = powder_emission_fixture();
        file.timelines[0].items[0].binder_index = 0;
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            vfx_scale_enabled: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        });
        let points = [crate::avfx::VfxBindPoint {
            parent_bone: None,
            id: 3,
            translate: [0.0; 3],
            rotate: [0.0; 3],
        }];
        file.emitters[0].scale = axis3(Some(2.0), Some(-3.0), Some(4.0));
        for vfx_scale in [2.0, -2.0, 0.0] {
            for mode in 1..=4 {
                file.particles[0]
                    .simple
                    .as_mut()
                    .unwrap()
                    .base_direction_type = mode;
                let runtime = VfxRuntime::with_bind_points(&file, &points)
                    .with_vfx_scale(vfx_scale)
                    .unwrap();
                let mut quads = Vec::new();
                runtime.sample(0.5, &mut quads);
                if vfx_scale == 0.0 && mode >= 3 {
                    assert!(
                        quads.is_empty(),
                        "undefined zero-scale division must not reach GPU"
                    );
                    continue;
                }
                assert!(!quads.is_empty());
                // The root auxiliary matrix first contributes Q to the
                // emitter basis. SBDT 3/4 then cancels Q, retaining local S.
                let factor = if mode >= 3 { 1.0 } else { vfx_scale };
                let expected = if mode == 1 || mode == 3 {
                    [
                        [2.0 * factor, 0.0, 0.0],
                        [0.0, 0.0, -4.0 * factor],
                        [0.0, -3.0 * factor, 0.0],
                    ]
                } else {
                    [
                        [2.0 * factor, 0.0, 0.0],
                        [0.0, -3.0 * factor, 0.0],
                        [0.0, 0.0, 4.0 * factor],
                    ]
                };
                assert!(quads.iter().all(|q| q.parent_basis == expected));
                assert!(quads.iter().all(|q| q.size == [vfx_scale; 2]));
            }
        }
    }

    #[test]
    fn powder_simple_direction_uses_current_emitter_matrix() {
        let mut file = powder_emission_fixture();
        let mut rotation = linear_curve(0.0);
        rotation.keys.push(AvfxCurveKey {
            time: 30,
            z: std::f32::consts::PI,
            ..rotation.keys[0]
        });
        file.emitters[0].rotation.z = Some(rotation);
        // These particle curves must not replace the emitter's SBDT basis.
        file.particles[0].scale = axis3(Some(5.0), Some(7.0), Some(11.0));
        file.particles[0].rotation = axis3(Some(0.3), Some(0.7), Some(0.9));
        file.particles[0].simple.as_mut().unwrap().create_count = 1;
        for x in [2.0, 0.0] {
            file.emitters[0].scale = axis3(Some(x), Some(-3.0), Some(4.0));
            for influence in [3, 6] {
                file.emitters[0].particle_items[0].parent_influence_coord = influence;
                for mode in 0..=4 {
                    file.particles[0]
                        .simple
                        .as_mut()
                        .unwrap()
                        .base_direction_type = mode;
                    let mut quads = Vec::new();
                    VfxRuntime::new(&file).sample(0.5, &mut quads);
                    assert_eq!(quads.len(), 1);
                    let expected = match mode {
                        1 | 3 => [[0.0, x, 0.0], [0.0, 0.0, -4.0], [3.0, 0.0, 0.0]],
                        2 | 4 => [[0.0, x, 0.0], [3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
                        _ => VFX_IDENTITY_BASIS,
                    };
                    assert_eq!(
                        quads[0].rotation_direction_base,
                        if mode == 0 { 5 } else { 10 }
                    );
                    for (actual, expected) in quads[0]
                        .parent_basis
                        .into_iter()
                        .flatten()
                        .zip(expected.into_iter().flatten())
                    {
                        assert!(
                            (actual - expected).abs() < 1.0e-6,
                            "SBDT={mode}, PICd={influence}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }

    fn powder_emission_fixture() -> AvfxFile {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.data = None;
        emitter.position = Default::default();
        emitter.particle_items[0].create_count = 1;
        emitter.particle_items[0].parent_influence_coord = 3;
        let particle = &mut file.particles[0];
        particle.particle_type = Some(ParticleType::Powder);
        particle.rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        particle.life.enabled = false;
        particle.scale = Default::default();
        particle.simple_anim_enable = true;
        particle.simple = Some(crate::avfx::AvfxParticleSimple {
            create_interval: 10,
            create_count: 3,
            create_interval_count: 1,
            create_interval_life: 100,
            injection_position_type: 1,
            injection_model_index: 0,
            injection_vertex_bind_model_index: -1,
            scale_start: [1.0; 2],
            scale_end: [1.0; 2],
            scale_rand_x: [1.0; 2],
            scale_rand_y: [1.0; 2],
            colors: [[255; 4]; 4],
            ..Default::default()
        });
        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertex_numbers: vec![0],
            emit_vertices: vec![crate::avfx::VfxEmitVertex {
                position: [1.0, 1.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                color: [255; 4],
            }],
            draw: None,
            ..Default::default()
        });
        file
    }

    #[test]
    fn powder_simple_coord_accuracy_controls_each_velocity_axis() {
        let mut file = powder_emission_fixture();
        file.models.clear();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.injection_model_index = -1;
        simple.injection_vertex_bind_model_index = -1;
        simple.coord_accuracy = [1.0; 3];
        simple.coord_gravity = [0.0, -0.04, 0.02];
        simple.velocity_min = 2.0;
        simple.velocity_max = 2.0;
        simple.create_new_after_delete = true;
        let sample = |file: &AvfxFile, frames| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(frames / AVFX_FPS, &mut quads);
            quads
        };
        for frames in [0.0_f32, 0.05, 0.1, 0.15, 1.25, 12.5, 102.5, 5.0] {
            let baseline = sample(&file, frames);
            assert!(!baseline.is_empty());
            for accuracy in [[0.0, 0.5, 1.002], [-0.5, -1.0, -1.002], [0.9999; 3]] {
                let mut variant = file.clone();
                variant.particles[0].simple.as_mut().unwrap().coord_accuracy = accuracy;
                let quads = sample(&variant, frames);
                assert_eq!(quads.len(), baseline.len());
                for (index, (quad, original)) in quads.iter().zip(&baseline).enumerate() {
                    let mut geometry = *quad;
                    geometry.position = original.position;
                    assert_eq!(geometry, *original, "only motion may change");
                    let age = (frames / AVFX_FPS * AVFX_FPS - index as f32 * 10.0) % 100.0;
                    if age == 0.0 {
                        assert_eq!(quad.position, original.position);
                        continue;
                    }
                    for axis in 0..3 {
                        let gravity = 0.5
                            * file.particles[0].simple.as_ref().unwrap().coord_gravity[axis]
                            * age
                            * age;
                        let mut velocity = (original.position[axis] - gravity) as f64 / age as f64;
                        let mut expected = gravity as f64;
                        let steps = (age as f64 * 10.0).floor() as usize;
                        for _ in 0..steps {
                            velocity *= accuracy[axis] as f64;
                            expected += velocity * 0.1;
                        }
                        velocity *= accuracy[axis] as f64;
                        expected += velocity * (age as f64 - steps as f64 * 0.1);
                        assert!(
                            (quad.position[axis] as f64 - expected).abs()
                                < 2.0e-5 * expected.abs().max(1.0),
                            "CAX/Y/Z={accuracy:?}, frame={frames}, slot={index}, axis={axis}: {} != {expected}",
                            quad.position[axis]
                        );
                    }
                }
            }
            assert_eq!(
                sample(&file, frames),
                baseline,
                "reverse queries must agree"
            );
        }
    }

    #[test]
    fn powder_simple_expanding_motion_omits_nonfinite_displacement() {
        for speed in [0.0, 2.0] {
            let mut file = powder_emission_fixture();
            let simple = file.particles[0].simple.as_mut().unwrap();
            simple.velocity_min = speed;
            simple.velocity_max = speed;
            simple.coord_accuracy = [2.0; 3];
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(50.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), if speed == 0.0 { 3 } else { 0 });
            assert!(quads.iter().flat_map(|q| q.position).all(f32::is_finite));
        }
    }

    #[test]
    fn powder_simple_unbound_motion_ignores_flattery_parameters() {
        let mut file = powder_emission_fixture();
        file.models.clear();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.injection_model_index = -1;
        simple.injection_vertex_bind_model_index = -1;
        // Use an exactly representable direction so the magnitude assertion
        // isolates flattery rather than signed-byte direction quantization.
        simple.injection_direction_type = 2;
        simple.coord_accuracy = [1.0; 3];
        simple.coord_gravity = [0.0, -0.04, 0.02];
        simple.create_new_after_delete = true;
        let sample = |file: &AvfxFile, frames| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(frames / AVFX_FPS, &mut quads);
            quads
        };
        for initial_speed in [0.0, 2.0] {
            let simple = file.particles[0].simple.as_mut().unwrap();
            simple.velocity_min = initial_speed;
            simple.velocity_max = initial_speed;
            for frames in [0.0, 0.1, 2.0, 12.0, 102.0, 5.0] {
                let baseline = sample(&file, frames);
                assert!(!baseline.is_empty());
                for (slot, quad) in baseline.iter().enumerate() {
                    let age = (frames - slot as f32 * 10.0) % 100.0;
                    let displacement = [
                        quad.position[0],
                        quad.position[1] + 0.02 * age * age,
                        quad.position[2] - 0.01 * age * age,
                    ];
                    let distance = displacement.iter().map(|v| v * v).sum::<f32>().sqrt();
                    assert!((distance - initial_speed * age).abs() < 1.0e-3);
                }
                for (rate, speed) in [(0.2, 1.0), (-0.2, 4.0), (2.0, -8.0)] {
                    let mut variant = file.clone();
                    let simple = variant.particles[0].simple.as_mut().unwrap();
                    simple.velocity_flattery_rate = rate;
                    simple.velocity_flattery_speed = speed;
                    assert_eq!(
                        sample(&variant, frames),
                        baseline,
                        "unbound FltR={rate}, FltS={speed}, age={frames}"
                    );
                }
            }
        }
    }

    #[test]
    fn powder_simple_vertex_binding_uses_cosine_flattery_gate() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.create_interval_life = 100;
        simple.injection_vertex_bind_model_index = 1;
        simple.velocity_min = 0.0;
        simple.velocity_max = 0.0;
        simple.coord_gravity = [0.0; 3];
        simple.velocity_flattery_rate = 0.0;
        simple.velocity_flattery_speed = 0.0;
        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertices: vec![
                crate::avfx::VfxEmitVertex {
                    position: [99.0, 0.0, 0.0],
                    ..Default::default()
                },
                crate::avfx::VfxEmitVertex {
                    position: [5.0, 0.0, 0.0],
                    ..Default::default()
                },
            ],
            emit_vertex_numbers: vec![1, 0],
            ..Default::default()
        });
        let sample = |file: &AvfxFile, frames| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(frames / AVFX_FPS, &mut quads);
            quads
        };
        let moving = sample(&file, 0.0);
        assert_eq!(moving.len(), 1);
        assert_eq!(
            moving[0].position,
            powder::packed_position([1.0, 1.0, 3.0], true)
        );

        let mut bound = file.clone();
        bound.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .velocity_flattery_rate = 1.0;
        let attached = sample(&bound, 0.0);
        assert_eq!(attached.len(), 1);
        assert_eq!(attached[0].position, [5.0, 0.0, 0.0]);
    }

    #[test]
    fn powder_bound_motion_does_not_recover_steps_skipped_by_flattery_gate() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.create_interval_life = 100;
        simple.injection_direction_type = 2;
        simple.injection_vertex_bind_model_index = 1;
        simple.velocity_min = 2.0;
        simple.velocity_max = 2.0;
        simple.coord_accuracy = [1.0; 3];
        simple.coord_gravity = [0.0; 3];
        simple.velocity_flattery_rate = -0.5;
        simple.velocity_flattery_speed = 0.5;
        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertices: vec![crate::avfx::VfxEmitVertex {
                position: [5.0, 0.0, 0.0],
                ..Default::default()
            }],
            emit_vertex_numbers: vec![0],
            ..Default::default()
        });

        let mut quads = Vec::new();
        let runtime = VfxRuntime::new(&file);
        runtime.sample(1.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 1);
        let birth_x = powder::packed_position([1.0, 1.0, 3.0], true)[0];
        assert!((quads[0].position[0] - birth_x).abs() < 1.0e-5);

        runtime.sample(2.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 1);
        // The first ten 0.1-frame steps only decay velocity. The next ten
        // move by 2, then the half-strength cosine gate blends toward VBMN.
        assert!((quads[0].position[0] - ((birth_x + 2.0) * 0.5 + 2.5)).abs() < 1.0e-5);

        // SIPT=2 requires an external source before its birth gate opens.
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .injection_position_type = 2;
        let runtime = VfxRuntime::new(&file);
        runtime.sample(1.0 / AVFX_FPS, &mut quads);
        assert!(quads.is_empty());
        runtime.sample(2.0 / AVFX_FPS, &mut quads);
        assert!(quads.is_empty());
    }

    #[test]
    fn powder_position_packing_uses_binding_dependent_width() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.injection_position_type = 1;
        simple.injection_vertex_bind_model_index = -1;
        simple.velocity_min = 0.0;
        simple.velocity_max = 0.0;
        let sample = |file: &AvfxFile| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            quads[0].position
        };
        let expected = |point: [f32; 3], resolution: f32| {
            let length = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
            std::array::from_fn(|axis| {
                ((point[axis] / length * resolution).trunc() / resolution) * length
            })
        };
        assert_coordinate(sample(&file), expected([1.0, 1.0, 3.0], 32000.0));

        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertex_numbers: vec![0],
            emit_vertices: vec![crate::avfx::VfxEmitVertex {
                position: [-5.0, 2.0, 1.0],
                ..Default::default()
            }],
            ..Default::default()
        });
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .injection_vertex_bind_model_index = 1;
        assert_coordinate(sample(&file), expected([1.0, 1.0, 3.0], 100.0));
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .velocity_flattery_rate = 1.0;
        assert_coordinate(sample(&file), expected([-5.0, 2.0, 1.0], 100.0));
    }

    #[test]
    fn powder_birth_scatter_uses_the_birth_spawner_matrix() {
        for (position_type, model_index) in [(0, -1), (0, 0), (1, 0)] {
            let mut file = powder_emission_fixture();
            let simple = file.particles[0].simple.as_mut().unwrap();
            simple.create_count = 1;
            simple.create_area = [0.75, 1.25, 0.5];
            simple.injection_position_type = position_type;
            simple.injection_model_index = model_index;
            simple.injection_vertex_bind_model_index = -1;
            let sample = |file: &AvfxFile| {
                let mut quads = Vec::new();
                VfxRuntime::new(file).sample(0.0, &mut quads);
                assert_eq!(quads.len(), 1);
                quads[0].position
            };
            let local = sample(&file);
            file.emitters[0].position = axis3(Some(10.0), Some(20.0), Some(30.0));
            file.emitters[0].rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
            file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
            assert_coordinate(
                sample(&file),
                [
                    10.0 - 3.0 * local[1],
                    20.0 - 2.0 * local[0],
                    30.0 + 4.0 * local[2],
                ],
            );
        }
    }

    #[test]
    fn powder_binding_scatter_uses_an_independent_current_spawner_target() {
        let mut file = powder_emission_fixture();
        let particle = &mut file.particles[0];
        particle.position = axis3(Some(0.0), Some(0.0), Some(0.0));
        particle.rotation = axis3(None, None, Some(0.0));
        particle.scale = axis3(Some(1.0), Some(1.0), Some(1.0));
        for (axis, end) in [
            (&mut particle.position.x, 10.0),
            (&mut particle.position.y, 20.0),
            (&mut particle.position.z, 30.0),
            (&mut particle.rotation.z, std::f32::consts::FRAC_PI_2),
            (&mut particle.scale.x, 2.0),
            (&mut particle.scale.y, 3.0),
            (&mut particle.scale.z, 4.0),
        ] {
            let curve = axis.as_mut().unwrap();
            curve.keys.push(AvfxCurveKey {
                time: 20,
                z: end,
                ..curve.keys[0]
            });
        }
        let simple = particle.simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.create_area = [0.75, 1.25, 0.5];
        simple.injection_position_type = 1;
        simple.injection_vertex_bind_model_index = 1;
        simple.velocity_min = 0.0;
        simple.velocity_max = 0.0;
        simple.velocity_flattery_rate = 0.0;
        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertex_numbers: vec![0],
            emit_vertices: vec![crate::avfx::VfxEmitVertex {
                position: [5.0, -2.0, 1.0],
                ..Default::default()
            }],
            ..Default::default()
        });
        let sample = |file: &AvfxFile, frame| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(frame / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 1);
            quads[0].position
        };
        let birth = sample(&file, 0.0);
        assert_coordinate(sample(&file, 20.0), birth);
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .velocity_flattery_rate = 1.0;
        let bind_local = sample(&file, 0.0);
        let source_scatter: [f32; 3] =
            std::array::from_fn(|axis| birth[axis] - [1.0, 1.0, 3.0][axis]);
        let bind_scatter: [f32; 3] =
            std::array::from_fn(|axis| bind_local[axis] - [5.0, -2.0, 1.0][axis]);
        assert!(
            source_scatter
                .iter()
                .zip(bind_scatter)
                .any(|(a, b)| (a - b).abs() > 1e-4)
        );
        let bind_world = [
            10.0 - 3.0 * bind_local[1],
            20.0 + 2.0 * bind_local[0],
            30.0 + 4.0 * bind_local[2],
        ];
        assert_coordinate(sample(&file, 20.0), bind_world);
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .velocity_flattery_rate = 0.5;
        assert_coordinate(
            sample(&file, 20.0),
            std::array::from_fn(|axis| 0.5 * (birth[axis] + bind_world[axis])),
        );
    }

    #[test]
    fn powder_simple_reuses_slots_only_after_death() {
        let mut file = powder_emission_fixture();
        file.models.clear();
        let particle = &mut file.particles[0];
        particle.position = axis3(Some(0.0), None, None);
        let curve = particle.position.x.as_mut().unwrap();
        curve.keys.push(AvfxCurveKey {
            time: 100,
            z: 100.0,
            ..curve.keys[0]
        });
        let simple = particle.simple.as_mut().unwrap();
        simple.create_count = 2;
        simple.create_interval = 1;
        simple.create_interval_life = 10;
        simple.create_new_after_delete = true;
        let sample = |frames| {
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(frames / AVFX_FPS, &mut quads);
            quads
        };
        let initial = sample(2.0);
        assert_eq!(initial.len(), 2);
        assert_eq!(initial, sample(3.0), "living slots must not be evicted");
        let reborn = sample(10.5);
        assert_eq!(reborn.len(), 2);
        assert!((reborn[0].position[0] - 10.0).abs() < 1.0e-5);
        assert!((reborn[1].position[0] - 1.0).abs() < 1.0e-5);
        assert_eq!(initial[0].orientation, reborn[0].orientation);
    }

    #[test]
    fn powder_simple_uv_loop_sign_controls_death_rebirth_and_hold() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.create_interval_life = 0;
        simple.uv_cell = [3, 1];
        simple.uv_interval = 2;
        let sample = |file: &AvfxFile, frames| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(frames / AVFX_FPS, &mut quads);
            quads
        };
        let step = (32767 / 3) as f32 / 32767.0;
        for (loops, frames, cell) in [(-1, 8.0, 2), (0, 8.0, 1), (-2, 8.0, 1), (-2, 14.0, 2)] {
            file.particles[0].simple.as_mut().unwrap().uv_loop_count = loops;
            let quads = sample(&file, frames);
            assert_eq!(quads.len(), 1, "UvLC={loops}, age={frames}");
            assert!((quads[0].uv_scales[0][0] - step).abs() < 1.0e-7);
            let left = quads[0].uv_origins[0][0] + 0.5 * (1.0 - step);
            assert!((left - cell as f32 * step).abs() < 1.0e-7);
        }
        file.particles[0].simple.as_mut().unwrap().uv_loop_count = 1;
        assert_eq!(sample(&file, 5.5).len(), 1);
        assert!(sample(&file, 6.0).is_empty());
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .create_new_after_delete = true;
        let reborn = sample(&file, 8.0);
        assert_eq!(reborn.len(), 1);
        assert!((reborn[0].uv_origins[0][0] - (step + 0.5 * (step - 1.0))).abs() < 1.0e-7);
    }

    #[test]
    fn powder_simple_uv_random_range_and_horizontal_flip_are_independent() {
        let mut file = powder_emission_fixture();
        file.particles[0].uv_sets[0].scroll.x = Some(linear_curve(0.3));
        file.particles[0].uv_sets[0].scale.y = Some(linear_curve(2.0));
        file.particles[0].uv_sets[0].rotation = linear_curve(0.75);
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 64;
        simple.create_interval_count = 64;
        simple.create_interval_life = 0;
        simple.uv_cell = [8, 1];
        simple.uv_interval = 0;
        simple.uv_no_random = 1;
        simple.uv_reverse = true;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(1.0, &mut quads);
        assert_eq!(quads.len(), 64);
        let step = (32767 / 8) as f32 / 32767.0;
        let mut cells = [false; 2];
        let mut flips = [false; 2];
        for quad in &quads {
            assert_eq!(quad.uv_rotations[0], 0.0);
            assert_eq!(quad.uv_scales[0][1], 1.0);
            let scale = quad.uv_scales[0][0];
            assert!((scale.abs() - step).abs() < 1.0e-7);
            let left = quad.uv_origins[0][0] + 0.5 * (1.0 - scale.abs());
            let cell = (left / step).round() as usize;
            assert!(cell < 2, "UvNR=1 yielded cell {cell}");
            cells[cell] = true;
            flips[usize::from(scale < 0.0)] = true;
        }
        assert_eq!(cells, [true; 2]);
        assert_eq!(flips, [true; 2]);
        let first = quads.clone();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(first, quads, "UvIv=0 must hold the initial UV cell");
    }

    #[test]
    fn powder_simple_lifetime_randomness_shifts_color_and_survives_rebirth() {
        let mut file = powder_emission_fixture();
        file.particles[0].color = color_curve(1.0, 1.0, 1.0, 1.0);
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 64;
        simple.create_interval_count = 64;
        simple.create_interval_life = 10;
        simple.create_life_random = 3;
        simple.create_new_after_delete = true;
        simple.colors = [[0, 0, 0, 255], [255; 4], [255; 4], [255; 4]];
        simple.frames = [0, 10, 10, 10];
        simple.scale_curve = 1.0;
        simple.scale_start = [1.0; 2];
        simple.scale_end = [3.0; 2];
        let mut early = Vec::new();
        let mut later = Vec::new();
        VfxRuntime::new(&file).sample(0.5 / AVFX_FPS, &mut early);
        VfxRuntime::new(&file).sample(14.5 / AVFX_FPS, &mut later);
        assert_eq!(early.len(), 64);
        assert_eq!(later.len(), 64);
        let mut offsets = [false; 7];
        for (a, b) in early.iter().zip(&later) {
            // (0.5 - offset) / 10, followed by the client color packing.
            let possible: Vec<_> = (-3..=3)
                .filter(|offset| {
                    let expected = quantize_powder_color([(0.5 - *offset as f32) / 10.0; 4])[0];
                    (a.color[0] - expected).abs() < 1.0e-6
                })
                .collect();
            assert!(
                !possible.is_empty(),
                "non-integer palette offset: {:?}",
                a.color
            );
            assert!((a.size[0] - 1.1).abs() < 1.0e-6, "SC uses nominal CrIL");
            assert!(
                possible.into_iter().any(|offset| {
                    let life = 10 + offset;
                    let age = 14.5 % life as f32;
                    let expected =
                        quantize_powder_color([((age - offset as f32) / 10.0).min(1.0); 4])[0];
                    let matches = (b.color[0] - expected).abs() < 1.0e-6
                        && (b.size[0] - (1.0 + age / 5.0)).abs() < 1.0e-6;
                    if matches {
                        offsets[(offset + 3) as usize] = true;
                    }
                    matches
                }),
                "rebirth rerolled lifetime/color: {a:?} -> {b:?}"
            );
        }
        assert_eq!(offsets, [true; 7]);
    }

    #[test]
    fn powder_simple_scale_exponent_is_not_clamped() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 1;
        simple.create_interval_life = 10;
        simple.scale_start = [2.0; 2];
        simple.scale_end = [4.0; 2];
        for (exponent, expected) in [(0.0, 4.0), (-1.0, 6.0), (2.0, 2.5)] {
            file.particles[0].simple.as_mut().unwrap().scale_curve = exponent;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(5.0 / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 1);
            assert_eq!(quads[0].size, [expected; 2]);
        }
        file.particles[0].simple.as_mut().unwrap().scale_curve = -1.0;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty(), "singular scale must not reach the GPU");
    }

    #[test]
    fn powder_simple_zero_creation_interval_births_all_slots() {
        let mut file = powder_emission_fixture();
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_interval = 0;
        simple.create_interval_life = -1;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(10.0, &mut quads);
        assert_eq!(quads.len(), 3);
    }

    #[test]
    fn powder_without_simple_emits_one_quad_per_particle() {
        for keep_simple_payload in [false, true] {
            let mut file = powder_emission_fixture();
            file.particles[0].simple_anim_enable = false;
            if !keep_simple_payload {
                file.particles[0].simple = None;
            }
            file.particles[0].scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
            file.emitters[0].scale = axis3(Some(5.0), Some(7.0), Some(11.0));
            let runtime = VfxRuntime::new(&file);
            let mut quads = Vec::new();
            for time in [0.0, 0.5, 1.0] {
                runtime.sample(time, &mut quads);
                assert_eq!(quads.len(), 1, "time={time}, Smpl={keep_simple_payload}");
                assert!(quads[0].powder_single);
                assert_eq!(quads[0].size, [-5.0, 10.5]);
                assert_eq!(quads[0].parent_basis, VFX_IDENTITY_BASIS);
            }
        }
    }

    #[test]
    fn powder_missing_enabled_simple_payload_does_not_emit_single_quad() {
        let mut file = powder_emission_fixture();
        file.particles[0].simple = None;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.5, &mut quads);
        assert!(quads.is_empty());
    }

    #[test]
    fn disc_sampling_retains_one_instance_and_full_scale() {
        let mut file = powder_emission_fixture();
        file.particles[0].particle_type = Some(ParticleType::Disc);
        file.particles[0].scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        file.particles[0].data = AvfxParticleData::Disc(crate::avfx::AvfxParticleDataDisc {
            parts_count: 1,
            parts_count_u: 2,
            parts_count_v: 64,
            angle: linear_curve(std::f32::consts::TAU),
            radius_begin: linear_curve(0.26),
            radius_end: linear_curve(0.26),
            width_begin: linear_curve(0.03),
            width_end: linear_curve(0.03),
            scaling_scale: 100,
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.5, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].particle_type, Some(ParticleType::Disc));
        let disc = quads[0].disc.unwrap();
        assert_eq!(disc.counts, [1, 2, 64]);
        assert_eq!(disc.vertex_count(), 378);
        assert_eq!(disc.radius, [0.26; 2]);
        assert_eq!(disc.width, [0.03; 2]);
        assert_eq!(disc.scale_z, 4.0);
        assert_eq!(quads[0].size, [1.0, 1.5]);
    }

    #[test]
    fn polygon_sampling_retains_truncated_count_and_full_scale() {
        let mut file = powder_emission_fixture();
        file.particles[0].particle_type = Some(ParticleType::Polygon);
        file.particles[0].scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        file.particles[0].data = AvfxParticleData::Polygon(crate::avfx::AvfxParticleDataPolygon {
            count: linear_curve(5.9),
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.5, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].particle_type, Some(ParticleType::Polygon));
        assert_eq!(quads[0].polygon.unwrap().count, 5);
        assert_eq!(quads[0].polygon.unwrap().scale_z, 4.0);
        assert_eq!(quads[0].size, [1.0, 1.5]);
    }

    #[test]
    fn windmill_consumes_uv_type_low_byte_and_keeps_one_instance() {
        let mut file = powder_emission_fixture();
        file.particles[0].particle_type = Some(ParticleType::Windmill);
        for raw in [0, 1, 256, 257, 2, -1] {
            file.particles[0].data = AvfxParticleData::Windmill { uv_type: raw };
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.5, &mut quads);
            if raw as u8 > 1 {
                assert!(quads.is_empty());
            } else {
                assert_eq!(quads.len(), 1);
                assert_eq!(quads[0].windmill_uv_type, raw as u8);
                assert_eq!(quads[0].particle_index, 0);
                assert_eq!(quads[0].particle_type, Some(ParticleType::Windmill));
            }
        }
    }

    #[test]
    fn windmill_uses_single_powder_scale_rotation_and_color_rules() {
        let mut file = powder_emission_fixture();
        file.emitters[0].rotation = axis3(Some(0.8), Some(-0.9), Some(0.7));
        file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(0.5));
        file.emitters[0].particle_items[0].parent_influence_coord = 2;
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.rotation_order = 5;
        particle.rotation = axis3(Some(0.2), Some(-0.3), Some(0.4));
        particle.rotation_velocity[2] = linear_curve(0.02);
        particle.scale = axis3(Some(1.2), Some(-0.7), Some(2.0));
        particle.color = color_curve(-0.2, 0.1239, 12.0, 0.00399);
        let mut expected = Vec::new();
        VfxRuntime::new(&file).sample(0.5, &mut expected);
        file.particles[0].particle_type = Some(ParticleType::Windmill);
        // Smpl belongs to Powder and must not change Windmill's geometry.
        file.particles[0].simple_anim_enable = true;
        let mut actual = Vec::new();
        VfxRuntime::new(&file).sample(0.5, &mut actual);
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].color, expected[0].color);
        assert_eq!(actual[0].size, expected[0].size);
        assert_eq!(actual[0].orientation, expected[0].orientation);
        assert_eq!(actual[0].parent_basis, expected[0].parent_basis);
        assert_eq!(actual[0].position, expected[0].position);
        assert!(!actual[0].powder_single);
    }

    #[test]
    fn powder_single_color_uses_client_fixed_point_range_and_truncation() {
        let mut file = powder_emission_fixture();
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.color = color_curve(-0.2, 0.1239, 12.0, 0.00399);
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        for (actual, expected) in quads[0].color.into_iter().zip([0.0, 0.123, 10.0, 0.003]) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn powder_simple_color_multiplies_full_particle_color_before_quantization() {
        let mut file = powder_emission_fixture();
        file.particles[0].color = AvfxColorCurve {
            brightness: Some(linear_curve(2.0)),
            scale_rgb: Some(crate::avfx::AvfxColorScaleRgb {
                r: Some(linear_curve(0.5)),
                g: Some(linear_curve(2.0)),
                b: Some(linear_curve(-1.0)),
            }),
            scale_alpha: Some(linear_curve(0.25)),
            ..color_curve(0.25, 0.5, 0.75, 0.8)
        };
        file.particles[0].simple.as_mut().unwrap().colors = [[128, 64, 255, 128]; 4];
        for draw_mode in 0..=12 {
            file.particles[0].draw_mode = draw_mode;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            for (actual, expected) in quads[0].color.into_iter().zip([0.125, 0.501, 0.0, 0.100]) {
                assert!(
                    (actual - expected).abs() < 1.0e-6,
                    "RMT={draw_mode}: {actual} != {expected}"
                );
            }
        }

        file.particles[0].color = color_curve(30.0, 0.1239, 0.0, 0.00399);
        file.particles[0].simple.as_mut().unwrap().colors = [[128, 255, 255, 255]; 4];
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        for (actual, expected) in quads[0].color.into_iter().zip([10.0, 0.123, 0.0, 0.003]) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn powder_simple_color_uses_looped_particle_age_and_each_childs_age() {
        let mut file = powder_emission_fixture();
        let particle = &mut file.particles[0];
        particle.loop_start = 0;
        particle.loop_end = 10;
        particle.color = color_curve(1.0, 0.5, 0.25, 1.0);
        let mut alpha = linear_curve(0.0);
        let mut last = alpha.keys[0].clone();
        last.time = 10;
        last.z = 1.0;
        alpha.keys.push(last);
        particle.color.alpha = Some(alpha);
        let simple = particle.simple.as_mut().unwrap();
        simple.colors = [[0; 4], [255; 4], [0; 4], [255; 4]];
        simple.frames = [0, 10, 20, 30];
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(0.5, &mut quads);
        assert_eq!(quads.len(), 2);
        // Col is at frame 5; the children are at frames 15 and 5.
        for quad in &quads {
            for (actual, expected) in quad.color.into_iter().zip([0.5, 0.25, 0.125, 0.25]) {
                assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
            }
        }
        runtime.sample(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].color, [0.0; 4]);
    }

    #[test]
    fn powder_simple_color_keeps_particle_random_channels() {
        let mut file = powder_emission_fixture();
        file.particles[0].color = color_curve(0.4, 0.3, 0.2, 0.6);
        file.particles[0].color.random =
            [0.2, 0.3, 0.1, 0.2, 0.5].map(|value| Some(linear_curve(value)));
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(quads.len(), 1);
        let actual = quads[0].color;
        // A white Smpl palette must preserve the ordinary Col evaluator's random
        // channels. Non-Smpl Powder uses the same fixed-point vertex format.
        file.particles[0].simple_anim_enable = false;
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert_eq!(actual, quads[0].color);
        assert_ne!(actual, [0.4, 0.3, 0.2, 0.6]);
    }

    #[test]
    fn powder_single_rotates_zxy_and_inherits_only_parent_euler_z() {
        for order in 0..6 {
            for (mode, always, inherited_z) in [
                (6, false, 0.0),
                (1, false, 0.0),
                (3, false, 1.0),
                (2, false, 1.5),
                (1, true, 1.5),
            ] {
                let mut file = powder_emission_fixture();
                let emitter = &mut file.emitters[0];
                emitter.rotation = axis3(Some(0.8), Some(-0.9), None);
                let mut parent_z = linear_curve(1.0);
                parent_z.keys.push(AvfxCurveKey {
                    time: 30,
                    z: 2.0,
                    ..parent_z.keys[0]
                });
                emitter.rotation.z = Some(parent_z);
                emitter.particle_items[0].parent_influence_coord = mode;
                emitter.particle_items[0].influence_coord_rot = always;
                let particle = &mut file.particles[0];
                particle.simple_anim_enable = false;
                particle.rotation_order = order;
                particle.rotation = axis3(Some(0.2), Some(-0.3), Some(0.4));
                particle.rotation_velocity[2] = linear_curve(0.02);
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(0.5, &mut quads);
                assert_eq!(quads.len(), 1);
                // Powder VS1 uses Rot plus parent Z; VR does not add drawing angles.
                let (sx, cx) = 0.2_f32.sin_cos();
                let (sy, cy) = (-0.3_f32).sin_cos();
                let (sz, cz) = (0.4_f32 + inherited_z).sin_cos();
                for [x, y, _] in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                    let a = x * cz - y * sz;
                    let b = x * sz + y * cz;
                    let expected = [a * cy + b * sx * sy, b * cx, b * sx * cy - a * sy];
                    let actual = quat_rotate(quads[0].orientation, [x, y, 0.0]);
                    for axis in 0..3 {
                        assert!(
                            (actual[axis] - expected[axis]).abs() < 1.0e-5,
                            "ROT={order}, PICd={mode}: {actual:?} != {expected:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn powder_emission_uses_spawner_transform_at_each_birth() {
        let a = std::f32::consts::FRAC_1_SQRT_2;
        let packed = powder::packed_position([1.0, 1.0, 3.0], false);
        for sx in [2.0, -2.0, 0.0] {
            for model_index in [-1, 0] {
                let mut file = powder_emission_fixture();
                let emitter = &mut file.emitters[0];
                emitter.position = axis3(Some(10.0), Some(20.0), Some(30.0));
                emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
                emitter.scale = axis3(Some(sx), Some(3.0), Some(4.0));
                let particle = &mut file.particles[0];
                particle.position = axis3(Some(1.0), Some(2.0), Some(-1.0));
                let curve = particle.position.x.as_mut().unwrap();
                curve.keys.push(AvfxCurveKey {
                    time: 20,
                    z: 5.0,
                    ..curve.keys[0]
                });
                particle.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_4));
                particle.scale = axis3(Some(0.5), Some(2.0), Some(0.0));
                particle.simple.as_mut().unwrap().injection_model_index = model_index;
                let runtime = VfxRuntime::new(&file);
                for frame in [0.0, 25.0, 10.0, 20.0, 35.0] {
                    let mut quads = Vec::new();
                    runtime.sample(frame / AVFX_FPS, &mut quads);
                    assert_eq!(quads.len(), ((frame / 10.0) as usize + 1).min(3));
                    for (event, quad) in quads.iter().enumerate() {
                        // T(10,20,30) Rz(90) S(sx,3,4) applied to the
                        // animated position plus Rz(45) S(.5,2,0) packed model point.
                        let px = 1.0 + event as f32 * 2.0;
                        let offset = if model_index < 0 {
                            [0.0; 3]
                        } else {
                            [
                                -3.0 * a * (0.5 * packed[0] + 2.0 * packed[1]),
                                sx * a * (0.5 * packed[0] - 2.0 * packed[1]),
                                0.0,
                            ]
                        };
                        let expected = [4.0 + offset[0], 20.0 + sx * px + offset[1], 26.0];
                        for (actual, expected) in quad.position.into_iter().zip(expected) {
                            assert!(
                                (actual - expected).abs() < 1e-5,
                                "sx={sx}, model={model_index}, frame={frame}, event={event}: {:?} != {expected}",
                                quad.position
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn powder_births_capture_emitter_following_and_spawner_motion() {
        let a = std::f32::consts::FRAC_1_SQRT_2;
        for (influence, expected) in [
            (
                2,
                [[2.0, 0.0, 0.0], [5.0 - a, 9.0 * a, 0.0], [30.0, 6.0, 0.0]],
            ),
            (3, [[2.0, 0.0, 0.0], [2.0, 5.0, 0.0], [2.0, 10.0, 0.0]]),
            (
                4,
                [[2.0, 0.0, 0.0], [5.0 - a, 9.0 * a, 0.0], [30.0, 6.0, 0.0]],
            ),
            (
                1,
                [[2.0, 0.0, 0.0], [1.0 + a, 5.0 + a, 0.0], [1.0, 11.0, 0.0]],
            ),
        ] {
            let mut file = powder_emission_fixture();
            file.models[0].emit_vertices[0].position = [1.0, 0.0, 0.0];
            let emitter = &mut file.emitters[0];
            emitter.particle_items[0].parent_influence_coord = influence;
            emitter.particle_items[0].influence_coord_rot = true;
            emitter.particle_items[0].influence_coord_scale = false;
            emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
                injection_speed: linear_curve(0.5),
                ..upward_cone()
            }));
            emitter.position = axis3(Some(0.0), None, None);
            let curve = emitter.position.x.as_mut().unwrap();
            curve.keys.push(AvfxCurveKey {
                time: 10,
                z: 5.0,
                ..curve.keys[0]
            });
            curve.keys.push(AvfxCurveKey {
                time: 20,
                z: 40.0,
                ..curve.keys[0]
            });
            emitter.scale = axis3(Some(1.0), Some(1.0), Some(1.0));
            let curve = emitter.scale.x.as_mut().unwrap();
            curve.keys.push(AvfxCurveKey {
                time: 20,
                z: 3.0,
                ..curve.keys[0]
            });
            emitter.rotation = axis3(None, None, Some(0.0));
            let curve = emitter.rotation.z.as_mut().unwrap();
            curve.keys.push(AvfxCurveKey {
                time: 20,
                z: std::f32::consts::FRAC_PI_2,
                ..curve.keys[0]
            });
            file.particles[0].position = axis3(Some(1.0), None, None);
            let runtime = VfxRuntime::new(&file);
            for frame in [20.0, 35.0, 10.0, 0.0] {
                let mut quads = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut quads);
                assert_eq!(quads.len(), ((frame / 10.0) as usize + 1).min(3));
                for (quad, expected) in quads.iter().zip(expected) {
                    for (actual, expected) in quad.position.into_iter().zip(expected) {
                        assert!(
                            (actual - expected).abs() < 1e-5,
                            "influence={influence}, frame={frame}: {:?} != {expected}",
                            quad.position
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn powder_births_keep_emitter_and_looping_particle_clocks_separate() {
        let mut file = powder_emission_fixture();
        file.models[0].emit_vertices[0].position = [1.0, 0.0, 0.0];
        let emitter = &mut file.emitters[0];
        emitter.create_interval = linear_curve(10.0);
        emitter.create_count = frame_ramp(0.0, 1.0, 0, 10);
        emitter.create_count.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        emitter.particle_items[0].parent_influence_coord = 2;
        emitter.position = axis3(Some(0.0), None, None);
        let curve = emitter.position.x.as_mut().unwrap();
        curve.keys.push(AvfxCurveKey {
            time: 100,
            z: 100.0,
            ..curve.keys[0]
        });
        let particle = &mut file.particles[0];
        particle.life = AvfxLife {
            enabled: true,
            value: 50.0,
            ..Default::default()
        };
        particle.loop_end = 15;
        particle.position = axis3(None, Some(0.0), None);
        let curve = particle.position.y.as_mut().unwrap();
        curve.keys.push(AvfxCurveKey {
            time: 30,
            z: 30.0,
            ..curve.keys[0]
        });
        let runtime = VfxRuntime::new(&file);
        for frame in [35.0, 15.0, 25.0] {
            let mut quads = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut quads);
            let expected: Vec<_> = (1..=frame as usize / 10)
                .flat_map(|parent_event| {
                    (0..3).filter_map(move |child_event| {
                        let birth = (parent_event + child_event) as f32 * 10.0;
                        (birth <= frame).then_some([
                            birth + 1.0,
                            (child_event * 10 % 15) as f32,
                            0.0,
                        ])
                    })
                })
                .collect();
            assert_eq!(quads.len(), expected.len());
            for (quad, expected) in quads.iter().zip(expected) {
                for (actual, expected) in quad.position.into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "frame={frame}: {:?} != {expected}",
                        quad.position
                    );
                }
            }
        }
    }

    #[test]
    fn tc1_null_texture_list_entry_does_not_use_ignored_scalar_texture() {
        let mut file = fixture_file();
        for index in [-1, 2, 255] {
            let texture = file.particles[0].texture_color1.as_mut().unwrap();
            texture.texture_list = vec![255, 1];
            texture.mask_texture_index = 255;
            texture.texture_index = index;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert!(!quads.is_empty());
            assert!(
                quads
                    .iter()
                    .all(|quad| quad.texture_indexes[0] == -1 && !quad.texture1_is_shape_mask)
            );
        }
    }

    #[test]
    fn tc1_animated_texture_selection_uses_client_integer_and_signed_byte_rules() {
        let mut texture = crate::avfx::AvfxParticleTexture {
            enabled: true,
            texture_index: 7,
            mask_texture_index: -1,
            texture_list: vec![2, 3, 255, 128],
            tex_n: Some(crate::avfx::AvfxCurve {
                keys: vec![crate::avfx::AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 2.9,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            -1
        );
        texture.tex_n.as_mut().unwrap().keys[0].z = 3.9;
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            -128
        );
        texture.texture_list.push(253);
        texture.tex_n.as_mut().unwrap().keys[0].z = 4.9;
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            -3
        );
        texture.tex_n.as_mut().unwrap().keys[0].z = -1.9;
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            -3
        );
        texture.texture_list = vec![10, 11];
        texture.tex_n.as_mut().unwrap().keys[0].z = 0.75;
        texture.tex_n_random = Some(crate::avfx::AvfxCurve {
            random_type: 1,
            keys: vec![crate::avfx::AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }],
            ..Default::default()
        });
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            11
        );
        texture.tex_n = None;
        texture.tex_n_random = None;
        texture.texture_index = 1;
        assert_eq!(
            tc1_texture_index(
                &texture,
                CurveAges {
                    local: 0.0,
                    total: 0.0
                },
                1
            ),
            10
        );
    }

    #[test]
    fn tc1_builtin_texture_sources_precede_list_selection() {
        let mut texture = crate::avfx::AvfxParticleTexture {
            enabled: true,
            texture_index: 7,
            texture_list: vec![2],
            use_screen_copy: true,
            ..Default::default()
        };
        let ages = CurveAges {
            local: 0.0,
            total: 0.0,
        };
        assert_eq!(tc1_texture_index(&texture, ages, 1), -2);
        texture.previous_frame_copy = true;
        assert_eq!(tc1_texture_index(&texture, ages, 1), -3);
        texture.use_screen_copy = false;
        texture.use_chara_portrait = true;
        assert_eq!(tc1_texture_index(&texture, ages, 1), -5);
        texture.use_chara_portrait = false;
        texture.texture_list.clear();
        assert_eq!(tc1_texture_index(&texture, ages, 1), -1);
    }

    #[test]
    fn tc1_builtin_list_entries_keep_color_conversion_and_sampler_state() {
        let mut particle = quad_particle();
        let tc1 = particle.texture_color1.as_mut().unwrap();
        tc1.color_to_alpha = true;
        tc1.texture_border_u = 2;
        tc1.texture_border_v = 1;
        tc1.calculate_color = 1;
        tc1.calculate_alpha = 1;

        for (entry, source) in [(254, -2), (253, -3), (251, -5)] {
            let tc1 = particle.texture_color1.as_mut().unwrap();
            tc1.texture_list = vec![entry];
            tc1.mask_texture_index = entry;
            let resolved = ResolvedTexture::of(&particle, 0.0, 1);
            assert_eq!(resolved.texture_indexes[0], source);
            assert!(resolved.color_to_alpha[0]);
            assert_eq!(resolved.texture_borders[0], [2, 1]);
            assert!(!resolved.texture1_use_screen_copy);
        }

        let tc1 = particle.texture_color1.as_mut().unwrap();
        tc1.use_screen_copy = true;
        tc1.previous_frame_copy = true;
        let resolved = ResolvedTexture::of(&particle, 0.0, 1);
        assert_eq!(resolved.texture_indexes[0], -3);
        assert!(resolved.color_to_alpha[0]);
        assert!(resolved.texture1_use_screen_copy);
    }

    #[test]
    fn signed_model_list_entries_do_not_select_models_128_through_255() {
        let mut file = fixture_file();
        file.models = vec![
            crate::avfx::VfxModelGeometry {
                draw: Some(crate::avfx::VfxDrawModel::default()),
                ..Default::default()
            };
            256
        ];
        file.particles[0].particle_type = Some(ParticleType::Model);
        file.particles[0].data = AvfxParticleData::Model {
            model_number_random_value: 0,
            model_number_random_type: 0,
            model_number_random_interval: 0,
            fresnel_type: 0,
            directional_light_type: 0,
            point_light_type: 0,
            is_lightning: false,
            is_morph: false,
            model_indexes: vec![255, 254],
            animation_number: None,
            morph: None,
            fresnel_curve: None,
            fresnel_curve_random: None,
            fresnel_rotation: None,
            color_begin: Default::default(),
            color_end: Default::default(),
        };
        let mut meshes = Vec::new();
        VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
        assert!(meshes.is_empty());

        for (index, drawn) in [(254, false), (127, true), (128, false)] {
            let AvfxParticleData::Model { model_indexes, .. } = &mut file.particles[0].data else {
                unreachable!();
            };
            model_indexes[0] = index;
            meshes.clear();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert_eq!(!meshes.is_empty(), drawn, "MdNo={index}");
            if drawn {
                assert!(meshes.iter().all(|mesh| mesh.model_index == index as usize));
            }
        }

        // Scalar selectors do not use the IntList sentinel convention.
        file.particles[0].particle_type = Some(ParticleType::LightModel);
        file.particles[0].data = AvfxParticleData::LightModel { model_index: 255 };
        VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
        assert!(!meshes.is_empty());
        assert!(meshes.iter().all(|mesh| mesh.model_index == 255));
    }

    #[test]
    fn later_color_layers_use_txno_even_when_tlst_is_present() {
        let mut particle = quad_particle();
        for (layer, texture_index) in [
            (&mut particle.texture_color2, 3),
            (&mut particle.texture_color3, 4),
            (&mut particle.texture_color4, 5),
        ] {
            *layer = Some(AvfxParticleTexture {
                enabled: true,
                texture_index,
                mask_texture_index: 0,
                texture_list: vec![0],
                ..Default::default()
            });
        }
        let texture = ResolvedTexture::of(&particle, 0.0, 1);
        assert_eq!(texture.texture_indexes, [2, 3, 4, 5]);
        assert!(texture.texture1_is_shape_mask);
    }

    #[test]
    fn model_number_curve_selects_the_static_model_pool_subset() {
        let mut particle = AvfxParticle {
            data: AvfxParticleData::Model {
                model_number_random_value: 0,
                model_number_random_type: 5,
                model_number_random_interval: 0,
                fresnel_type: 0,
                directional_light_type: 0,
                point_light_type: 0,
                is_lightning: false,
                is_morph: false,
                model_indexes: vec![10, 11, 255],
                animation_number: Some(linear_curve(-1.9)),
                morph: None,
                fresnel_curve: None,
                fresnel_curve_random: None,
                fresnel_rotation: None,
                color_begin: Default::default(),
                color_end: Default::default(),
            },
            ..Default::default()
        };
        let ages = CurveAges {
            local: 0.0,
            total: 0.0,
        };
        assert_eq!(model_particle_index(&particle, ages, 0), 11);

        let set_model_number = |particle: &mut AvfxParticle, value| {
            let AvfxParticleData::Model {
                animation_number, ..
            } = &mut particle.data
            else {
                unreachable!();
            };
            *animation_number = Some(linear_curve(value));
        };
        set_model_number(&mut particle, 2.9);
        assert_eq!(model_particle_index(&particle, ages, 0), -1);
        set_model_number(&mut particle, f32::NAN);
        assert_eq!(model_particle_index(&particle, ages, 0), -1);
        set_model_number(&mut particle, 2_147_483_648.0);
        assert_eq!(model_particle_index(&particle, ages, 0), -1);

        set_model_number(&mut particle, 1.0);
        let AvfxParticleData::Model {
            model_number_random_value,
            ..
        } = &mut particle.data
        else {
            unreachable!();
        };
        *model_number_random_value = 1;
        // Always variants do not initialize the Model-number birth cache.
        assert_eq!(model_particle_index(&particle, ages, 0), 11);
        let AvfxParticleData::Model {
            model_number_random_value,
            model_number_random_interval,
            ..
        } = &mut particle.data
        else {
            unreachable!();
        };
        *model_number_random_value = 0;
        *model_number_random_interval = 1;
        assert_eq!(model_particle_index(&particle, ages, 0), 10);
    }

    #[test]
    fn model_number_first_random_types_use_client_integer_ranges() {
        assert_eq!(model_number_random_base_from_unit(4, 0, 0), -4);
        assert_eq!(model_number_random_base_from_unit(4, 0, u16::MAX), 4);
        assert_eq!(model_number_random_base_from_unit(4, 1, 0), 0);
        assert_eq!(model_number_random_base_from_unit(4, 1, u16::MAX), 4);
        assert_eq!(model_number_random_base_from_unit(4, 2, 0), -4);
        assert_eq!(model_number_random_base_from_unit(4, 2, u16::MAX), 0);
        for random_type in 3..=5 {
            assert_eq!(
                model_number_random_base_from_unit(4, random_type, u16::MAX),
                0
            );
        }
        assert_eq!(model_number_random_base_from_unit(0, 0, u16::MAX), 0);
        assert_eq!(model_number_random_base_from_unit(-4, 0, u16::MAX), 0);
        assert_eq!(model_number_random_base_from_unit(4, 99, u16::MAX), 0);

        for random_type in 0..=5 {
            let first = model_number_random_base(9, random_type, 1234);
            let repeated = model_number_random_base(9, random_type, 1234);
            assert_eq!(first, repeated);
        }
    }

    #[test]
    fn particle_color_uses_channel_scales_and_brightness_for_every_draw_mode() {
        let mut file = fixture_file();
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        file.particles[0].color = AvfxColorCurve {
            brightness: Some(linear_curve(2.0)),
            scale_rgb: Some(crate::avfx::AvfxColorScaleRgb {
                r: Some(linear_curve(0.5)),
                g: Some(linear_curve(2.0)),
                b: Some(linear_curve(-1.0)),
            }),
            scale_alpha: Some(linear_curve(0.25)),
            ..color_curve(0.25, 0.5, 0.75, 0.8)
        };
        let expected = [0.25, 2.0, -1.5, 0.2];
        for draw_mode in 0..=12 {
            file.particles[0].draw_mode = draw_mode;
            file.particles[0].particle_type = Some(ParticleType::Quad);
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert!(!quads.is_empty());
            for quad in quads {
                assert_eq!(quad.color, expected, "RMT={draw_mode}");
            }

            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert!(!meshes.is_empty());
            for mesh in meshes {
                assert_eq!(mesh.color, expected, "RMT={draw_mode}");
            }
        }
    }

    #[test]
    fn color_scales_include_random_offsets_and_preserve_empty_defaults() {
        let mut cc = color_curve(0.25, 0.5, 0.75, 0.8);
        cc.brightness = Some(linear_curve(2.0));
        cc.random = [0.2, 0.3, 0.4, 0.1, 0.5].map(|value| Some(linear_curve(value)));
        for seed in [0, 17, 999] {
            let base = color_curve_seeded(&cc, 5.0, seed);
            let mut scaled = cc.clone();
            scaled.scale_rgb = Some(crate::avfx::AvfxColorScaleRgb {
                r: Some(linear_curve(0.5)),
                g: Some(AvfxCurve::default()),
                b: Some(linear_curve(-2.0)),
            });
            scaled.scale_alpha = Some(linear_curve(0.25));
            let actual = color_curve_seeded(&scaled, 5.0, seed);
            for (actual, expected) in
                actual
                    .into_iter()
                    .zip([base[0] * 0.5, base[1], base[2] * -2.0, base[3] * 0.25])
            {
                assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
            }
        }
    }

    #[test]
    fn preview_color_skips_single_zero_and_disabled_random_additions() {
        let mut color = color_curve(-0.0, -0.0, -0.0, -0.0);
        for mode in [0, 1, 2, 3, 4, 5, 6, 7, 14, 15] {
            color.random = std::array::from_fn(|_| {
                Some(AvfxCurve {
                    random_type: mode,
                    ..linear_curve(-0.0)
                })
            });
            for seed in 0..8 {
                assert_eq!(
                    color_curve_seeded(&color, 0.0, seed).map(f32::to_bits),
                    [0x8000_0000; 4]
                );
            }
            if mode & 7 >= 6 {
                color.random = std::array::from_fn(|_| {
                    Some(AvfxCurve {
                        random_type: mode,
                        ..linear_curve(2.0)
                    })
                });
                assert_eq!(
                    color_curve_seeded(&color, 0.0, 17).map(f32::to_bits),
                    [0x8000_0000; 4]
                );
            }
        }
        let mut file = fixture_file();
        file.particles[0].color = color;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(
            quads
                .iter()
                .all(|quad| quad.color.map(f32::to_bits) == [0x8000_0000; 4])
        );
    }

    fn animated_parent_color() -> AvfxColorCurve {
        let mut color = color_curve(1.0, 0.25, 0.5, 0.8);
        let rgb = color.rgb.as_mut().unwrap();
        rgb.keys.push(AvfxCurveKey {
            time: 30,
            x: 4.0,
            ..rgb.keys[0]
        });
        color
    }

    fn assert_color(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
        }
    }

    #[test]
    fn parent_color_modes_apply_to_quad_mesh_powder_and_windmill() {
        let mut file = fixture_file();
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        file.emitters[0].life.value = 120.0;
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].create_interval = linear_curve(10.0);
        file.emitters[0].particle_items[0].create_count = 1;
        file.emitters[0].color = animated_parent_color();
        file.emitters[0].color.brightness = Some(linear_curve(2.0));
        file.emitters[0].color.scale_alpha = Some(linear_curve(0.5));
        file.particles[0].color = color_curve(2.0, -0.5, 12.0, 0.6);
        // A particle's local curve loop must not rewind its parent color clock.
        file.particles[0].loop_start = 3;
        file.particles[0].loop_end = 7;
        for kind in [
            ParticleType::Quad,
            ParticleType::LightModel,
            ParticleType::Powder,
            ParticleType::Windmill,
        ] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = if kind == ParticleType::LightModel {
                AvfxParticleData::LightModel { model_index: 0 }
            } else {
                AvfxParticleData::None
            };
            for mode in [0, 1, 2, 8] {
                file.emitters[0].particle_items[0].parent_influence_color = mode;
                let runtime = VfxRuntime::new(&file);
                for frame in [12.0, 18.0, 12.0] {
                    let colors: Vec<_> = if kind == ParticleType::LightModel {
                        let mut meshes = Vec::new();
                        runtime.sample_mesh(frame / AVFX_FPS, &mut meshes);
                        meshes.into_iter().map(|m| m.color).collect()
                    } else {
                        let mut quads = Vec::new();
                        runtime.sample(frame / AVFX_FPS, &mut quads);
                        quads.into_iter().map(|q| q.color).collect()
                    };
                    assert_eq!(colors.len(), 2);
                    for (birth, actual) in [0.0, 10.0].into_iter().zip(colors) {
                        let mut expected = if mode == 0 {
                            [2.0, -0.5, 12.0, 0.6]
                        } else {
                            let parent_age = if mode == 1 { birth } else { frame };
                            [4.0 * (1.0 + parent_age / 10.0), -0.25, 12.0, 0.24]
                        };
                        if matches!(kind, ParticleType::Powder | ParticleType::Windmill) {
                            expected = expected
                                .map(|v: f32| (v * 1000.0).clamp(0.0, 10000.0).trunc() * 0.001);
                        }
                        assert_color(actual, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn parent_color_follows_nested_emitter_clocks_and_snapshots() {
        let mut file = fixture_file();
        let mut child = file.emitters[0].clone();
        child.life.value = 100.0;
        child.child_limit = 1;
        child.create_count = frame_ramp(0.0, 1.0, 0, 4);
        child.create_count.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        child.create_interval = linear_curve(4.0);
        child.particle_items[0].create_count = 1;
        child.color = animated_parent_color();
        let root = &mut file.emitters[0];
        root.life.value = 100.0;
        root.child_limit = 1;
        root.create_interval = linear_curve(5.0);
        root.create_count = frame_ramp(0.0, 1.0, 0, 5);
        root.create_count.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        root.particle_items.clear();
        root.color = animated_parent_color();
        root.emitter_items = vec![AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_count: 1,
            create_probability: 100,
            ..Default::default()
        }];
        file.emitters.push(child);
        file.particles[0].color = color_curve(2.0, 2.0, 2.0, 0.5);
        for emitter_mode in 0..=2 {
            file.emitters[0].emitter_items[0].parent_influence_color = emitter_mode;
            for particle_mode in 0..=2 {
                file.emitters[1].particle_items[0].parent_influence_color = particle_mode;
                let mut quads = Vec::new();
                VfxRuntime::new(&file).sample(0.5, &mut quads);
                assert_eq!(quads.len(), 1);
                let mut expected = [2.0, 2.0, 2.0, 0.5];
                if particle_mode != 0 {
                    let frame = if particle_mode == 1 { 9.0 } else { 15.0 };
                    let child_color = [1.0 + (frame - 5.0) / 10.0, 0.25, 0.5, 0.8];
                    expected = std::array::from_fn(|i| expected[i] * child_color[i]);
                    if emitter_mode != 0 {
                        let parent_frame = if emitter_mode == 1 { 5.0 } else { frame };
                        let root_color = [1.0 + parent_frame / 10.0, 0.25, 0.5, 0.8];
                        expected = std::array::from_fn(|i| expected[i] * root_color[i]);
                    }
                }
                assert_color(quads[0].color, expected);
            }
        }
    }

    #[test]
    fn parent_color_always_uses_continuous_timeline_root_age() {
        let mut file = fixture_file();
        file.emitters[0].life.value = 10.0;
        file.emitters[0].create_interval = linear_curve(20.0);
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].particle_items[0].create_count = 1;
        file.emitters[0].color = animated_parent_color();
        file.particles[0].color = color_curve(1.0, 1.0, 1.0, 1.0);
        for mode in [1, 2] {
            file.emitters[0].particle_items[0].parent_influence_color = mode;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.5, &mut quads);
            assert_eq!(quads.len(), 1);
            for quad in quads {
                assert_color(
                    quad.color,
                    [if mode == 1 { 1.0 } else { 2.5 }, 0.25, 0.5, 0.8],
                );
            }
        }
    }

    #[test]
    fn parent_color_randomness_belongs_to_emitter_instance() {
        let mut file = fixture_file();
        file.emitters[0].color = color_curve(0.5, 0.5, 0.5, 0.5);
        file.emitters[0].color.random = [Some(linear_curve(0.4)), None, None, None, None];
        file.emitters[0].particle_items[0].parent_influence_color = 2;
        file.particles[0].color = color_curve(1.0, 1.0, 1.0, 1.0);
        let runtime = VfxRuntime::new(&file);
        let mut first = Vec::new();
        runtime.sample(0.5, &mut first);
        assert_eq!(first.len(), 4);
        assert_eq!(first[0].color[1..], [0.5; 3]);
        assert!(first[0].color[0] != 0.5);
        assert!(first.iter().all(|q| q.color == first[0].color));
        let mut later = Vec::new();
        runtime.sample(1.5, &mut later);
        assert!(later.iter().all(|q| q.color == later[0].color));
        assert_eq!(first[0].color, later[0].color);
        runtime.sample(0.5, &mut later);
        assert_eq!(first, later);
    }

    #[test]
    fn parent_color_powder_children_use_spawner_color_before_palette_quantization() {
        let mut file = powder_emission_fixture();
        file.emitters[0].color = animated_parent_color();
        file.particles[0].color = color_curve(12.0, 0.2, 0.3, 0.1);
        file.particles[0].loop_start = 0;
        file.particles[0].loop_end = 4;
        let simple = file.particles[0].simple.as_mut().unwrap();
        simple.create_count = 3;
        simple.create_interval = 5;
        simple.create_interval_life = 0;
        simple.colors = [[51, 128, 255, 128]; 4];
        for mode in [1, 2] {
            file.emitters[0].particle_items[0].parent_influence_color = mode;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.5, &mut quads);
            assert_eq!(quads.len(), 3);
            for quad in quads {
                // All slots, including delayed births, use the current spawner's
                // inherited color. PICo Initial belongs to the spawner's birth.
                assert_color(
                    quad.color,
                    [if mode == 1 { 2.4 } else { 6.0 }, 0.025, 0.15, 0.040],
                );
            }
        }
    }

    #[test]
    fn client_fresnel_axes_use_distinct_reference_directions() {
        let quarter = std::f32::consts::FRAC_PI_2;
        for (kind, rotation, expected) in [
            (2, [0.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            (2, [quarter, 0.0, 0.0], [0.0, 0.0, -1.0]),
            (2, [quarter, quarter, 0.0], [-1.0, 0.0, 0.0]),
            (2, [0.0, 0.0, quarter], [1.0, 0.0, 0.0]),
            (3, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            (3, [quarter, 0.0, 0.0], [0.0, 1.0, 0.0]),
            (3, [0.0, quarter, 0.0], [-1.0, 0.0, 0.0]),
            (3, [quarter, 0.0, quarter], [-1.0, 0.0, 0.0]),
        ] {
            for (actual, expected) in fresnel_axis(kind, rotation).into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-6, "{kind}: {rotation:?}");
            }
        }
    }

    #[test]
    fn model_colors_are_particle_color_times_fresnel_not_lifetime_lerp() {
        let mut file = fixture_file();
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        let color = |rgb: [f32; 3], alpha| crate::avfx::AvfxColorCurve {
            rgb: Some(AvfxCurve {
                keys: vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: rgb[0],
                    y: rgb[1],
                    z: rgb[2],
                }],
                ..Default::default()
            }),
            alpha: Some(linear_curve(alpha)),
            ..Default::default()
        };
        file.particles[0].color = color([0.2, 0.4, 0.8], 0.5);
        file.particles[0].particle_type = Some(ParticleType::Model);
        for kind in 0..=3 {
            file.particles[0].data = AvfxParticleData::Model {
                model_number_random_value: 0,
                model_number_random_type: 0,
                model_number_random_interval: 0,
                fresnel_type: kind,
                directional_light_type: 0,
                point_light_type: 0,
                is_lightning: false,
                is_morph: false,
                model_indexes: vec![0],
                animation_number: None,
                morph: None,
                fresnel_curve: Some(linear_curve(2.0)),
                fresnel_curve_random: None,
                fresnel_rotation: None,
                color_begin: AvfxColorCurve {
                    scale_rgb: Some(crate::avfx::AvfxColorScaleRgb {
                        r: Some(linear_curve(2.0)),
                        ..Default::default()
                    }),
                    scale_alpha: Some(linear_curve(0.5)),
                    ..color([1.0, 0.0, 0.0], 0.25)
                },
                color_end: AvfxColorCurve {
                    scale_rgb: Some(crate::avfx::AvfxColorScaleRgb {
                        b: Some(linear_curve(3.0)),
                        ..Default::default()
                    }),
                    scale_alpha: Some(linear_curve(0.5)),
                    ..color([0.0, 0.0, 1.0], 0.75)
                },
            };
            for time in [0.0, 0.25, 0.125] {
                let mut meshes = Vec::new();
                VfxRuntime::new(&file).sample_mesh(time, &mut meshes);
                assert!(!meshes.is_empty());
                for mesh in meshes {
                    assert_eq!(mesh.color, [0.2, 0.4, 0.8, 0.5], "FrsT={kind}, t={time}");
                    if kind == 0 {
                        assert_eq!(mesh.fresnel, None);
                    } else {
                        let fresnel = mesh.fresnel.expect("enabled Fresnel");
                        assert_eq!(fresnel.kind, kind);
                        assert_eq!(fresnel.exponent, 2.0);
                        assert_eq!(fresnel.color_begin, [2.0, 0.0, 0.0, 0.125]);
                        assert_eq!(fresnel.color_end, [0.0, 0.0, 3.0, 0.375]);
                        if kind == 2 {
                            assert_eq!(fresnel.direction, [0.0, -1.0, 0.0]);
                        } else if kind == 3 {
                            assert_eq!(fresnel.direction, [0.0, 0.0, -1.0]);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn quads_and_models_share_injection_gravity_air_resistance_and_parent_scale() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.particle_items[0].parent_influence_coord = 3;
        emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        emitter.scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        emitter.gravity = linear_curve(0.02);
        emitter.air_resistance = linear_curve(0.1);
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(0.25),
            ..upward_cone()
        }));
        let particle = &mut file.particles[0];
        particle.position = axis3(Some(2.0), Some(1.0), Some(0.5));
        particle.gravity = linear_curve(0.03);
        particle.air_resistance = linear_curve(0.15);
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(4.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 1);
        // Rz(90) * S(2,3,4) * local(2,1,0.5) = (-3,4,2).
        // Injection v=(-0.25,0,0), travel=0.15*4; emitter ARs is independent.
        // Particle gravity adds to world Y; Initial captures emitter Y at birth.
        let expected = [-2.15, 6.24, 5.0];
        for (actual, expected) in quads[0].position.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
        }
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        for (kind, data) in [
            (
                ParticleType::LightModel,
                AvfxParticleData::LightModel { model_index: 0 },
            ),
            (
                ParticleType::Model,
                AvfxParticleData::Model {
                    model_number_random_value: 0,
                    model_number_random_type: 0,
                    model_number_random_interval: 0,
                    fresnel_type: 0,
                    directional_light_type: 0,
                    point_light_type: 0,
                    is_lightning: false,
                    is_morph: false,
                    model_indexes: vec![0],
                    animation_number: None,
                    morph: None,
                    fresnel_curve: None,
                    fresnel_curve_random: None,
                    fresnel_rotation: None,
                    color_begin: Default::default(),
                    color_end: Default::default(),
                },
            ),
        ] {
            file.particles[0].particle_type = Some(kind);
            file.particles[0].data = data;
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(4.0 / AVFX_FPS, &mut meshes);
            assert_eq!(meshes.len(), 1);
            assert_eq!(meshes[0].position, quads[0].position);
        }
    }

    #[test]
    fn emitter_gravity_moves_births_and_following_particles_in_world_y() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(2.0);
        emitter.particle_items[0].create_count = 1;
        emitter.data = None;
        emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        emitter.scale = axis3(Some(2.0), Some(-3.0), Some(0.0));
        emitter.gravity = linear_curve(0.0);
        emitter.gravity.keys.push(AvfxCurveKey {
            time: 6,
            z: 6.0,
            ..emitter.gravity.keys[0]
        });
        file.particles[0].position = axis3(Some(1.0), None, None);
        file.particles[0].gravity = linear_curve(-0.5);
        file.particles[0].rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        file.particles[0].loop_end = 2;
        for mode in [2, 3, 4, 5] {
            file.emitters[0].particle_items[0].parent_influence_coord = mode;
            let runtime = VfxRuntime::new(&file);
            for frame in [6.0_f32, 2.0, 4.0, 0.0, 6.0] {
                let mut quads = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut quads);
                assert_eq!(quads.len(), (frame / 2.0) as usize + 1);
                for (event, quad) in quads.iter().enumerate() {
                    let birth = event as f32 * 2.0;
                    let age = frame - birth;
                    let emitter_age = if matches!(mode, 2 | 4) { frame } else { birth };
                    // Continuous preview: emitter g(t)=t, particle g=-0.5.
                    let expected = [1.0, 4.0 + emitter_age.powi(3) / 6.0 - 0.25 * age * age, 3.0];
                    for (actual, expected) in quad.position.into_iter().zip(expected) {
                        assert!(
                            (actual - expected).abs() < 1e-5,
                            "PICd={mode}, frame={frame}, birth={birth}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn emitter_gravity_random_is_shared_by_births_and_stable_when_resampling() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(2.0);
        emitter.particle_items[0].create_count = 1;
        emitter.particle_items[0].parent_influence_coord = 2;
        emitter.position = Default::default();
        emitter.data = None;
        emitter.gravity_random = AvfxCurve {
            random_type: 1,
            ..linear_curve(1.0)
        };
        let mut quads = Vec::new();
        let runtime = VfxRuntime::new(&file);
        runtime.sample(4.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 3);
        let displacement = quads[0].position[1];
        assert!(displacement > 0.0 && displacement < 8.0);
        for frame in [6.0_f32, 2.0, 4.0, 0.0] {
            runtime.sample(frame / AVFX_FPS, &mut quads);
            for quad in &quads {
                assert!((quad.position[1] - displacement * (frame / 4.0).powi(2)).abs() < 1e-5);
            }
        }
        file.emitters[0].particle_items[0].parent_influence_coord = 3;
        let runtime = VfxRuntime::new(&file);
        for frame in [6.0_f32, 2.0, 4.0] {
            runtime.sample(frame / AVFX_FPS, &mut quads);
            for (event, quad) in quads.iter().enumerate() {
                let birth = event as f32 * 2.0;
                assert!((quad.position[1] - displacement * (birth / 4.0).powi(2)).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn nested_emitter_gravity_keeps_parent_birth_offset_and_world_axis() {
        let mut file = fixture_file();
        let child = &mut file.emitters[0];
        child.life.value = 100.0;
        child.create_count = linear_curve(1.0);
        child.create_interval = linear_curve(2.0);
        child.particle_items[0].create_count = 1;
        child.particle_items[0].parent_influence_coord = 3;
        child.data = None;
        child.position = axis3(Some(1.0), None, None);
        child.gravity = linear_curve(1.0);
        let child = child.clone();
        file.emitters.push(child);
        let parent = &mut file.emitters[0];
        parent.life.enabled = false;
        parent.child_limit = 1;
        parent.particle_items.clear();
        parent.position = Default::default();
        parent.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        parent.scale = axis3(Some(2.0), Some(3.0), Some(0.0));
        parent.gravity = linear_curve(0.5);
        parent.create_count = frame_ramp(0.0, 1.0, 0, 2);
        parent.create_count.keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        parent.emitter_items = vec![AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_count: 1,
            create_probability: 100,
            parent_influence_coord: 3,
            ..Default::default()
        }];
        let runtime = VfxRuntime::new(&file);
        for frame in [6.0_f32, 2.0, 4.0] {
            let mut quads = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), (frame / 2.0) as usize);
            for (quad, expected_y) in quads.iter().zip([3.0, 5.0, 11.0]) {
                assert!((quad.position[1] - expected_y).abs() < 1e-5);
                assert!(quad.position[0].abs() < 1e-5);
                assert_eq!(quad.position[2], 0.0);
            }
        }
    }

    #[test]
    fn powder_births_capture_emitter_and_particle_gravity_separately() {
        let mut file = powder_emission_fixture();
        file.emitters[0].gravity = linear_curve(0.02);
        file.particles[0].gravity = linear_curve(-0.01);
        let packed = powder::packed_position([1.0, 1.0, 3.0], false);
        for model in [0, -1] {
            file.particles[0]
                .simple
                .as_mut()
                .unwrap()
                .injection_model_index = model;
            for mode in [2, 3] {
                file.emitters[0].particle_items[0].parent_influence_coord = mode;
                let runtime = VfxRuntime::new(&file);
                for frame in [25.0, 10.0, 20.0, 0.0] {
                    let mut quads = Vec::new();
                    runtime.sample(frame / AVFX_FPS, &mut quads);
                    assert_eq!(quads.len(), ((frame / 10.0) as usize + 1).min(3));
                    for (event, quad) in quads.iter().enumerate() {
                        let birth = event as f32 * 10.0;
                        let mut expected = if model == 0 { packed } else { [0.0; 3] };
                        expected[1] += if mode == 2 { 0.005 } else { -0.005 } * birth * birth;
                        for (actual, expected) in quad.position.into_iter().zip(expected) {
                            assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn particle_air_resistance_scales_injection_without_clamping_or_emitter_sum() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.position = Default::default();
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(1.0),
            ..upward_cone()
        }));
        file.particles[0].position = axis3(Some(1.0), Some(2.0), Some(3.0));
        file.particles[0].gravity = linear_curve(0.25);
        for emitter_resistance in [0.0, 0.5, 1.0, 2.0, -1.0] {
            file.emitters[0].air_resistance = linear_curve(emitter_resistance);
            for multiplier in [None, Some(0.0), Some(0.5), Some(1.0), Some(2.0), Some(-0.5)] {
                file.particles[0].air_resistance =
                    multiplier.map_or_else(Default::default, linear_curve);
                let runtime = VfxRuntime::new(&file);
                for age in [4.0, 0.0, 1.5, 2.0] {
                    let mut quads = Vec::new();
                    runtime.sample(age / AVFX_FPS, &mut quads);
                    assert_eq!(quads.len(), 1);
                    let expected_y = 2.0 + age * multiplier.unwrap_or(1.0) + 0.125 * age * age;
                    assert_coordinate(quads[0].position, [1.0, expected_y, 3.0]);
                }
            }
        }
    }

    #[test]
    fn particle_air_resistance_integrates_history_and_particle_loops() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.position = Default::default();
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(1.0),
            ..upward_cone()
        }));
        let curve = &mut file.particles[0].air_resistance;
        *curve = linear_curve(1.0);
        curve.keys.push(AvfxCurveKey {
            time: 4,
            z: 0.0,
            ..curve.keys[0]
        });
        for looping in [false, true] {
            file.particles[0].loop_end = if looping { 4 } else { 0 };
            let runtime = VfxRuntime::new(&file);
            for (age, travel) in [
                (9.5, if looping { 5.21875 } else { 2.0 }),
                (1.0, 0.875),
                (4.0, 2.0),
                (0.0, 0.0),
                (5.0, if looping { 2.875 } else { 2.0 }),
                (2.0, 1.5),
            ] {
                let mut quads = Vec::new();
                runtime.sample(age / AVFX_FPS, &mut quads);
                assert_eq!(quads.len(), 1);
                assert!((quads[0].position[1] - travel).abs() < 1.0e-6);
            }
        }
    }

    #[test]
    fn particle_air_resistance_random_only_uses_zero_main_when_active() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.position = Default::default();
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(1.0),
            ..upward_cone()
        }));
        let sample = |file: &AvfxFile, age| {
            let mut quads = Vec::new();
            VfxRuntime::new(file).sample(age / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), 1);
            quads[0].position[1]
        };
        for random_type in [1, 4] {
            file.particles[0].air_resistance_random = AvfxCurve {
                random_type,
                ..linear_curve(0.5)
            };
            let mut cumulative = Vec::new();
            for age in [1.0, 2.0, 3.0] {
                file.particles[0].air_resistance = Default::default();
                let empty = sample(&file, age);
                file.particles[0].air_resistance = linear_curve(0.0);
                assert_eq!(empty, sample(&file, age));
                file.particles[0].air_resistance = linear_curve(1.0);
                assert!((sample(&file, age) - empty - age).abs() < 1.0e-6);
                assert!(empty > 0.0 && empty < 0.5 * age);
                cumulative.push(empty);
            }
            let increments = [
                cumulative[0],
                cumulative[1] - cumulative[0],
                cumulative[2] - cumulative[1],
            ];
            assert!(increments.iter().all(|&v| v > 0.0 && v < 0.5));
            if random_type == 1 {
                assert!((increments[0] - increments[1]).abs() < 1.0e-6);
                assert!((increments[0] - increments[2]).abs() < 1.0e-6);
            } else {
                assert!((increments[0] - increments[1]).abs() > 1.0e-4);
            }
        }
        file.particles[0].air_resistance = Default::default();
        file.particles[0].air_resistance_random = linear_curve(0.0);
        assert_eq!(sample(&file, 3.0), 3.0);
        // The client bypass tests key count, not whether an animated curve is constant.
        let random = &mut file.particles[0].air_resistance_random;
        random.keys.push(AvfxCurveKey {
            time: 4,
            ..random.keys[0]
        });
        assert_eq!(sample(&file, 3.0), 0.0);
    }

    #[test]
    fn powder_births_capture_air_resistance_history() {
        let mut file = powder_emission_fixture();
        file.emitters[0].data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(1.0),
            ..upward_cone()
        }));
        let curve = &mut file.particles[0].air_resistance;
        *curve = linear_curve(1.0);
        curve.keys.push(AvfxCurveKey {
            time: 20,
            z: 0.0,
            ..curve.keys[0]
        });
        let runtime = VfxRuntime::new(&file);
        let packed = powder::packed_position([1.0, 1.0, 3.0], false);
        for age in [25.0, 10.0, 20.0, 0.0] {
            let mut quads = Vec::new();
            runtime.sample(age / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), ((age / 10.0) as usize + 1).min(3));
            for (quad, travel) in quads.iter().zip([0.0, 7.5, 10.0]) {
                assert_coordinate(quad.position, [packed[0], packed[1] + travel, packed[2]]);
            }
        }
    }

    #[test]
    fn particles_accumulate_animated_gravity_across_keys_and_loops() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.life.enabled = false;
        emitter.position = Default::default();
        emitter.data = None;
        let particle = &mut file.particles[0];
        particle.gravity = linear_curve(0.0);
        particle.gravity.keys[0].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        particle.gravity.keys.push(AvfxCurveKey {
            time: 2,
            z: 0.5,
            ..particle.gravity.keys[0]
        });
        particle.loop_end = 4;
        let runtime = VfxRuntime::new(&file);
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        file.particles[0].particle_type = Some(ParticleType::LightModel);
        file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
        let mesh_runtime = VfxRuntime::new(&file);
        for (age, displacement) in [
            (7.0, 4.25),
            (2.0, 0.0),
            (4.0, 1.0),
            (0.0, 0.0),
            (5.0, 2.0),
            (3.0, 0.25),
        ] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            runtime.sample(age / AVFX_FPS, &mut quads);
            mesh_runtime.sample_mesh(age / AVFX_FPS, &mut meshes);
            assert_eq!(quads.len(), 1);
            assert_eq!(meshes.len(), 1);
            assert!((quads[0].position[1] - displacement).abs() < 1e-5);
            assert_eq!(quads[0].orientation, [0.0, 0.0, 0.0, 1.0]);
            assert_eq!(meshes[0].position, quads[0].position);
            assert_eq!(meshes[0].orientation, quads[0].orientation);
        }
    }

    #[test]
    fn vr_direction_integrates_turning_with_air_resistance() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.position = Default::default();
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.particle_items[0].parent_influence_coord = 1;
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(2.0),
            ..upward_cone()
        }));
        let particle = &mut file.particles[0];
        particle.life.enabled = false;
        particle.air_resistance = linear_curve(0.0);
        particle.air_resistance.keys.push(AvfxCurveKey {
            time: 4,
            z: 1.0,
            ..particle.air_resistance.keys[0]
        });
        particle.rotation_velocity[1] = linear_curve(0.0);
        particle.rotation_velocity[1].keys.push(AvfxCurveKey {
            time: 4,
            z: std::f32::consts::FRAC_PI_2,
            ..particle.rotation_velocity[1].keys[0]
        });
        // Injection +Y, parent up +Y: the alternate up +Z makes basis X=-X.
        // Integrate 2*(t/4)*(-sin(w*t), cos(w*t), 0), w=pi/8.
        let w = std::f32::consts::PI / 8.0;
        for lodr in [0, 1] {
            file.emitters[0].particle_items[0].local_direction = lodr;
            let runtime = VfxRuntime::new(&file);
            for frame in [4.0, 1.0, 0.0, 2.5, 3.0] {
                let mut out = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut out);
                let (s, c) = (w * frame).sin_cos();
                let expected = [
                    0.5 * (frame * c / w - s / (w * w)),
                    0.5 * (frame * s / w + (c - 1.0) / (w * w)),
                    0.0,
                ];
                for (actual, expected) in out[0].position.into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() < 2e-5,
                        "LoDr={lodr}: {actual} != {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn injection_follows_current_parent_linear_transform_only_in_always_mode() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.position = Default::default();
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            inner_size: linear_curve(1.0),
            outer_size: linear_curve(1.0),
            injection_speed: linear_curve(0.5),
            ..upward_cone()
        }));
        emitter.rotation = axis3(None, None, Some(0.0));
        emitter
            .rotation
            .z
            .as_mut()
            .unwrap()
            .keys
            .push(AvfxCurveKey {
                time: 4,
                z: std::f32::consts::FRAC_PI_2,
                ..linear_curve(0.0).keys[0]
            });
        emitter.scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        emitter.scale.y.as_mut().unwrap().keys.push(AvfxCurveKey {
            time: 4,
            z: 6.0,
            ..linear_curve(0.0).keys[0]
        });
        for mode in [0, 1, 2, 3] {
            file.emitters[0].particle_items[0].parent_influence_coord = mode;
            let runtime = VfxRuntime::new(&file);
            let mut birth = Vec::new();
            let mut out = Vec::new();
            runtime.sample(0.0, &mut birth);
            runtime.sample(4.0 / AVFX_FPS, &mut out);
            let [x, y, z] = birth[0].position;
            let expected = if mode == 2 {
                [-2.0 * (y + 2.0), x, z]
            } else {
                [x, y + 2.0, z]
            };
            for (actual, expected) in out[0].position.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 2e-5,
                    "PICd={mode}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn sphere_model_births_use_discrete_latitudes_and_method_origin() {
        use crate::avfx::{EmitterDataRotation, SphereModelEmitterData};
        for children in [false, true] {
            for method in [1, 3, 5, 7, 257, 259] {
                let indices: &[i32] = if method & 4 == 0 {
                    &[0, 4, 5, 6, 7, 8, 0, 4]
                } else {
                    &[4, 5, 6, 7, 4, 5, 6, 7]
                };
                let mut file = creation_count_fixture(children, 1, 8, 0.0);
                let emitter = &mut file.emitters[0];
                emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                emitter.data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                    generate_method: method,
                    divide_x: 260,
                    divide_y: 258,
                    radius: linear_curve(-2.0),
                    injection_speed: linear_curve(-0.25),
                    rotation: EmitterDataRotation {
                        angles: [
                            Default::default(),
                            Default::default(),
                            linear_curve(std::f32::consts::FRAC_PI_2),
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                }));
                if children {
                    file.emitters[1].data = None;
                    file.emitters[1].position = Default::default();
                    file.emitters[1].particle_items[0].parent_influence_coord = 2;
                }
                for frame in [0.0, 1.0] {
                    let mut out = Vec::new();
                    VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                    assert_eq!(out.len(), 8);
                    for (quad, &index) in out.iter().zip(indices) {
                        let normal = match index {
                            0 => [0.0, 1.0, 0.0],
                            4 => [0.0, 0.0, 1.0],
                            5 => [1.0, 0.0, 0.0],
                            6 => [0.0, 0.0, -1.0],
                            7 => [-1.0, 0.0, 0.0],
                            8 => [0.0, -1.0, 0.0],
                            _ => unreachable!(),
                        };
                        let dir = [2.0 * normal[1], 3.0 * normal[0], 4.0 * normal[2]];
                        let norm = dir.iter().map(|v| v * v).sum::<f32>().sqrt();
                        assert_coordinate(
                            quad.position,
                            std::array::from_fn(|axis| {
                                [1.0, 2.0, 3.0][axis]
                                    + if method & 2 != 0 {
                                        -2.0 * dir[axis]
                                    } else {
                                        0.0
                                    }
                                    - 0.25 * frame * dir[axis] / norm
                            }),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sphere_model_random_births_keep_the_client_pole_ranges() {
        use crate::avfx::SphereModelEmitterData;
        for method in [0, 2, 4, 6] {
            let mut file = creation_count_fixture(false, 1, 128, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                generate_method: method,
                divide_x: 4,
                divide_y: 2,
                radius: linear_curve(2.0),
                injection_speed: linear_curve(1.0),
                ..Default::default()
            }));
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 128);
            let distance = if method & 2 != 0 { 3.0 } else { 1.0 };
            let axes = [
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ];
            let mut seen = [false; 6];
            for quad in out {
                let selected = axes
                    .iter()
                    .position(|axis| {
                        quad.position
                            .iter()
                            .zip(axis)
                            .all(|(a, b)| (a - b * distance).abs() < 1e-5)
                    })
                    .expect("sphere birth must use a discrete latitude vertex");
                seen[selected] = true;
            }
            assert_eq!(seen[0], method < 4);
            // RandomOnVertexWithoutSingularPoint still includes the south pole.
            assert_eq!(seen[1], method != 4);
            assert!(seen[2..].iter().all(|v| *v));
        }
    }

    #[test]
    fn sphere_model_binding_tracks_radius_rotation_and_center_sentinel() {
        use crate::avfx::{EmitterDataRotation, SphereModelEmitterData};
        for children in [false, true] {
            for method in [1, 3, 5, 7] {
                for follow in [false, true] {
                    let mut file = creation_count_fixture(children, 1, 1, 0.0);
                    let emitter = &mut file.emitters[0];
                    emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                    emitter.position.x = Some(frame_ramp(1.0, 5.0, 0, 20));
                    emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                    emitter.data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                        generate_method: method,
                        divide_x: 4,
                        divide_y: 2,
                        radius: frame_ramp(2.0, 3.0, 0, 20),
                        rotation: EmitterDataRotation {
                            angles: [
                                frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 20),
                                Default::default(),
                                Default::default(),
                            ],
                            ..Default::default()
                        },
                        ..Default::default()
                    }));
                    let item = if children {
                        &mut emitter.emitter_items[0]
                    } else {
                        &mut emitter.particle_items[0]
                    };
                    item.parent_influence_coord = 1;
                    item.influence_coord_pos = follow;
                    if children {
                        file.emitters[1].data = None;
                        file.emitters[1].position = Default::default();
                        file.emitters[1].particle_items[0].parent_influence_coord = 2;
                    }
                    file.particles[0].life.enabled = false;
                    file.particles[0].loop_end = 4;
                    for frame in [20.0, 0.0, 10.0, 20.0] {
                        let age = if follow { frame } else { 0.0 };
                        let radius = 2.0 + age / 20.0;
                        let (s, c) = (age / 20.0 * std::f32::consts::FRAC_PI_2).sin_cos();
                        let local = match method {
                            3 => [0.0, radius * c, radius * s],
                            7 => [0.0, -radius * s, radius * c],
                            _ => [0.0; 3],
                        };
                        let mut out = Vec::new();
                        VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                        assert_eq!(out.len(), 1);
                        assert_coordinate(
                            out[0].position,
                            [1.0 + age / 5.0, 2.0 + 3.0 * local[1], 3.0 + 4.0 * local[2]],
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sphere_model_direction_override_is_on_vertex_only_and_precedes_shape_rotation() {
        use crate::avfx::{EmitterDataRotation, SphereModelEmitterData};
        for method in [1, 3, 5, 7] {
            for enabled in [false, true] {
                let mut file = creation_count_fixture(false, 1, 1, 0.0);
                let emitter = &mut file.emitters[0];
                emitter.position = Default::default();
                emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                emitter.rotation_order = 5;
                emitter.any_direction = enabled;
                let angles = [0.2, -0.4, 0.7];
                emitter.injection_angle = angles.map(linear_curve);
                emitter.data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                    generate_method: method,
                    divide_x: 4,
                    divide_y: 2,
                    radius: linear_curve(2.0),
                    injection_speed: linear_curve(-0.25),
                    rotation: EmitterDataRotation {
                        angles: [
                            Default::default(),
                            Default::default(),
                            linear_curve(std::f32::consts::FRAC_PI_2),
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                }));
                let normal = if method < 4 {
                    [0.0, 1.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0]
                };
                let dir = if enabled && method & 2 != 0 {
                    rotate_axes([0.0, 0.0, 1.0], angles, [0, 1, 2])
                } else {
                    normal
                };
                let dir = [2.0 * dir[1], 3.0 * dir[0], 4.0 * dir[2]];
                let norm = dir.iter().map(|v| v * v).sum::<f32>().sqrt();
                let point = [4.0 * normal[1], 6.0 * normal[0], 8.0 * normal[2]];
                let mut out = Vec::new();
                VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 1);
                assert_coordinate(
                    out[0].position,
                    std::array::from_fn(|axis| {
                        (if method & 2 != 0 { point[axis] } else { 0.0 }) - 0.25 * dir[axis] / norm
                    }),
                );
            }
        }
    }

    #[test]
    fn sphere_model_random_angles_and_speed_share_first_and_redraw_always() {
        use crate::avfx::{EmitterDataRotation, SphereModelEmitterData};
        for mode in [1, 4, 5] {
            let mut file = creation_count_fixture(false, 1, 8, 0.0);
            file.emitters[0].position = Default::default();
            let random_curve = |z| AvfxCurve {
                random_type: mode,
                ..linear_curve(z)
            };
            file.emitters[0].data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                generate_method: 7,
                divide_x: 1,
                divide_y: 1,
                radius: linear_curve(2.0),
                injection_speed_random: random_curve(0.2),
                rotation: EmitterDataRotation {
                    angles_random: [random_curve(0.7), Default::default(), Default::default()],
                    ..Default::default()
                },
                ..Default::default()
            }));
            let mut birth = Vec::new();
            let mut moved = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut birth);
            VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut moved);
            assert_eq!(birth.len(), 8);
            assert_eq!(moved.len(), 8);
            let mut angles = Vec::new();
            let mut speeds = Vec::new();
            for (birth, moved) in birth.iter().zip(&moved) {
                let [x, y, z] = birth.position;
                assert!(x.abs() < 1e-6);
                assert!((y.hypot(z) - 2.0).abs() < 1e-5);
                let angle = (-z).atan2(-y);
                let speed = moved.position[1].hypot(moved.position[2]) - 2.0;
                for coefficient in [angle / 0.7, speed / 0.2] {
                    assert!(coefficient.abs() <= 1.0001);
                    if mode == 1 {
                        assert!((coefficient * 100.0 - (coefficient * 100.0).round()).abs() < 1e-3);
                    }
                    assert!(if mode == 5 {
                        coefficient <= 1e-5
                    } else {
                        coefficient >= -1e-5
                    });
                }
                assert_coordinate(
                    moved.position,
                    birth.position.map(|v| v * (1.0 + speed / 2.0)),
                );
                angles.push(angle);
                speeds.push(speed);
            }
            assert_eq!(
                angles.windows(2).any(|a| (a[0] - a[1]).abs() > 1e-4),
                mode != 1
            );
            assert_eq!(
                speeds.windows(2).any(|a| (a[0] - a[1]).abs() > 1e-4),
                mode != 1
            );
            if mode == 1 {
                file.emitters[0].particle_items[0].influence_coord_pos = true;
                let mut bound = Vec::new();
                VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut bound);
                assert_eq!(bound, moved);
            }
        }
    }

    #[test]
    fn sphere_model_signed_fields_preserve_negative_y_and_skip_unwritten_directions() {
        use crate::avfx::SphereModelEmitterData;
        for (x, y, method, valid) in [
            (260, 254, 259, true),
            (4, 0, 3, false),
            (128, 2, 3, false),
            (256, 2, 3, false),
            (4, 2, 8, false),
        ] {
            let mut file = creation_count_fixture(false, 1, 2, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                generate_method: method,
                divide_x: x,
                divide_y: y,
                radius: linear_curve(2.0),
                ..Default::default()
            }));
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut out);
            assert_eq!(out.len(), if valid { 2 } else { 0 });
            if valid {
                assert_coordinate(out[0].position, [0.0, 2.0, 0.0]);
                assert_coordinate(out[1].position, [0.0, 0.0, -2.0]);
            }
        }
    }

    #[test]
    fn cylinder_model_births_use_height_side_normals_and_method_origin() {
        use crate::avfx::{CylinderModelEmitterData, EmitterDataRotation};
        for children in [false, true] {
            for method in [1, 3, 5, 7, 257, 259] {
                for y in [0, 2] {
                    let count = 4 * (y + 1);
                    let mut file = creation_count_fixture(children, 1, count, 0.0);
                    let emitter = &mut file.emitters[0];
                    emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                    emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                    emitter.data = Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                        generate_method: method,
                        divide_x: 260,
                        divide_y: y + 256,
                        length: linear_curve(4.0),
                        radius: linear_curve(-2.0),
                        injection_speed: linear_curve(-0.25),
                        rotation: EmitterDataRotation {
                            angles: [
                                Default::default(),
                                Default::default(),
                                linear_curve(std::f32::consts::FRAC_PI_2),
                            ],
                            ..Default::default()
                        },
                        ..Default::default()
                    }));
                    if children {
                        file.emitters[1].data = None;
                        file.emitters[1].position = Default::default();
                        file.emitters[1].particle_items[0].parent_influence_coord = 2;
                    }
                    for frame in [0.0, 1.0] {
                        let mut out = Vec::new();
                        VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                        assert_eq!(out.len(), count as usize);
                        for (index, quad) in out.iter().enumerate() {
                            let angle = (index % 4) as f32 * std::f32::consts::FRAC_PI_2;
                            let (sin, cos) = angle.sin_cos();
                            let height = if y == 0 {
                                0.0
                            } else {
                                (index / 4) as f32 * 2.0 - 2.0
                            };
                            let point = [2.0 * height, -6.0 * sin, -8.0 * cos];
                            let on_vertex = method & 2 != 0;
                            let dir = if on_vertex {
                                [0.0, 3.0 * sin, 4.0 * cos]
                            } else {
                                point
                            };
                            let length = dir.iter().map(|x| x * x).sum::<f32>().sqrt();
                            let expected = std::array::from_fn(|axis| {
                                [1.0, 2.0, 3.0][axis] + if on_vertex { point[axis] } else { 0.0 }
                                    - 0.25 * frame * dir[axis] / length
                            });
                            assert_coordinate(quad.position, expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cylinder_model_random_births_select_discrete_vertices() {
        use crate::avfx::CylinderModelEmitterData;
        for method in [0, 2, 4, 6] {
            let mut file = creation_count_fixture(false, 1, 64, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data =
                Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                    generate_method: method,
                    divide_x: 4,
                    divide_y: 2,
                    length: linear_curve(4.0),
                    radius: linear_curve(2.0),
                    injection_speed: linear_curve(1.0),
                    ..Default::default()
                }));
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 64);
            let mut seen = [false; 12];
            for particle in out {
                let index = (0..12)
                    .find(|&index| {
                        let angle = (index % 4) as f32 * std::f32::consts::FRAC_PI_2;
                        let (sin, cos) = angle.sin_cos();
                        let point = [2.0 * sin, (index / 4) as f32 * 2.0 - 2.0, 2.0 * cos];
                        let expected = if method & 2 != 0 {
                            [3.0 * sin, point[1], 3.0 * cos]
                        } else {
                            let length = point.iter().map(|x| x * x).sum::<f32>().sqrt();
                            point.map(|x| x / length)
                        };
                        particle
                            .position
                            .into_iter()
                            .zip(expected)
                            .all(|(a, b)| (a - b).abs() < 1e-5)
                    })
                    .expect("cylinder birth must select a discrete vertex");
                seen[index] = true;
            }
            assert!(seen[..4].iter().any(|v| *v));
            assert!(seen[8..].iter().any(|v| *v));
        }
    }

    #[test]
    fn cylinder_model_binding_tracks_height_radius_rotation_and_signed_index() {
        use crate::avfx::{CylinderModelEmitterData, EmitterDataRotation};
        for children in [false, true] {
            for method in [1, 3] {
                for x in [1, 4] {
                    for follow in [false, true] {
                        let mut file = creation_count_fixture(children, 1, 3 * x, 0.0);
                        let emitter = &mut file.emitters[0];
                        emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                        emitter.position.x = Some(frame_ramp(1.0, 5.0, 0, 20));
                        emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                        emitter.data =
                            Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                                generate_method: method,
                                divide_x: x,
                                divide_y: 2,
                                radius: frame_ramp(2.0, 3.0, 0, 20),
                                length: frame_ramp(4.0, 8.0, 0, 20),
                                rotation: EmitterDataRotation {
                                    angles: [
                                        Default::default(),
                                        Default::default(),
                                        frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 20),
                                    ],
                                    ..Default::default()
                                },
                                ..Default::default()
                            }));
                        let item = if children {
                            &mut emitter.emitter_items[0]
                        } else {
                            &mut emitter.particle_items[0]
                        };
                        item.parent_influence_coord = 1;
                        item.influence_coord_pos = follow;
                        if children {
                            file.emitters[1].data = None;
                            file.emitters[1].position = Default::default();
                            file.emitters[1].particle_items[0].parent_influence_coord = 2;
                        }
                        file.particles[0].life.enabled = false;
                        file.particles[0].loop_end = 4;
                        for frame in [20.0, 0.0, 10.0, 20.0] {
                            let age = if follow { frame } else { 0.0 };
                            let (s, c) = (age / 20.0 * std::f32::consts::FRAC_PI_2).sin_cos();
                            let radius = 2.0 + age / 20.0;
                            let length = 4.0 + age / 5.0;
                            let mut out = Vec::new();
                            VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                            assert_eq!(out.len(), (3 * x) as usize);
                            for (index, particle) in out.iter().enumerate() {
                                let index = if method == 1 { -1 } else { index as i32 };
                                let angle = (index % x) as f32 * std::f32::consts::TAU / x as f32;
                                let local = if method == 1 && !follow {
                                    [0.0; 3]
                                } else {
                                    [
                                        radius * angle.sin(),
                                        (index / x) as f32 * length / 2.0 - length / 2.0,
                                        radius * angle.cos(),
                                    ]
                                };
                                assert_coordinate(
                                    particle.position,
                                    [
                                        1.0 + age / 5.0 - 2.0 * (c * local[0] - s * local[1]),
                                        2.0 + 3.0 * (s * local[0] + c * local[1]),
                                        3.0 + 4.0 * local[2],
                                    ],
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cylinder_model_direction_override_is_on_vertex_only_and_precedes_shape_rotation() {
        use crate::avfx::{CylinderModelEmitterData, EmitterDataRotation};
        for method in [1, 3, 5, 7] {
            for enabled in [false, true] {
                let mut file = creation_count_fixture(false, 1, 1, 0.0);
                let emitter = &mut file.emitters[0];
                emitter.position = Default::default();
                emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                emitter.rotation_order = 5;
                emitter.any_direction = enabled;
                let angles = [0.2, -0.4, 0.7];
                emitter.injection_angle = angles.map(linear_curve);
                emitter.data = Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                    generate_method: method,
                    divide_x: 4,
                    divide_y: 2,
                    length: linear_curve(4.0),
                    radius: linear_curve(2.0),
                    injection_speed: linear_curve(-0.25),
                    rotation: EmitterDataRotation {
                        angles: [
                            Default::default(),
                            Default::default(),
                            linear_curve(std::f32::consts::FRAC_PI_2),
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                }));
                let on_vertex = method & 2 != 0;
                let dir = if !on_vertex {
                    [0.0, -2.0, 2.0]
                } else if enabled {
                    rotate_axes([0.0, 0.0, 1.0], angles, [0, 1, 2])
                } else {
                    [0.0, 0.0, 1.0]
                };
                let dir = [2.0 * dir[1], 3.0 * dir[0], 4.0 * dir[2]];
                let norm = dir.iter().map(|v| v * v).sum::<f32>().sqrt();
                let mut out = Vec::new();
                VfxRuntime::new(&file).sample(1.0 / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 1);
                assert_coordinate(
                    out[0].position,
                    std::array::from_fn(|axis| {
                        (if on_vertex {
                            [-4.0, 0.0, 8.0][axis]
                        } else {
                            0.0
                        }) - 0.25 * dir[axis] / norm
                    }),
                );
            }
        }
    }

    #[test]
    fn cylinder_model_random_angles_and_speed_share_first_and_redraw_always() {
        use crate::avfx::{CylinderModelEmitterData, EmitterDataRotation};
        for mode in [1, 4, 5] {
            let mut file = creation_count_fixture(false, 1, 8, 0.0);
            file.emitters[0].position = Default::default();
            let random_curve = |z| AvfxCurve {
                random_type: mode,
                ..linear_curve(z)
            };
            file.emitters[0].data =
                Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
                    generate_method: 3,
                    divide_x: 1,
                    divide_y: 0,
                    radius: linear_curve(2.0),
                    injection_speed_random: random_curve(0.2),
                    rotation: EmitterDataRotation {
                        angles_random: [random_curve(0.7), Default::default(), Default::default()],
                        ..Default::default()
                    },
                    ..Default::default()
                }));
            let runtime = VfxRuntime::new(&file);
            let mut birth = Vec::new();
            let mut moved = Vec::new();
            runtime.sample(0.0, &mut birth);
            runtime.sample(1.0 / AVFX_FPS, &mut moved);
            assert_eq!(birth.len(), 8);
            assert_eq!(moved.len(), 8);
            let mut angles = Vec::new();
            let mut speeds = Vec::new();
            for (birth, moved) in birth.iter().zip(&moved) {
                let [x, y, z] = birth.position;
                assert!(x.abs() < 1e-6);
                assert!((y.hypot(z) - 2.0).abs() < 1e-5);
                let angle = (-y).atan2(z);
                let speed = moved.position[1].hypot(moved.position[2]) - 2.0;
                for coefficient in [angle / 0.7, speed / 0.2] {
                    assert!(coefficient.abs() <= 1.0001);
                    if mode == 1 {
                        assert!((coefficient * 100.0 - (coefficient * 100.0).round()).abs() < 1e-3);
                    }
                    assert!(if mode == 5 {
                        coefficient <= 1e-5
                    } else {
                        coefficient >= -1e-5
                    });
                }
                assert_coordinate(
                    moved.position,
                    birth.position.map(|v| v * (1.0 + speed / 2.0)),
                );
                angles.push(angle);
                speeds.push(speed);
            }
            assert_eq!(
                angles.windows(2).any(|a| (a[0] - a[1]).abs() > 1e-4),
                mode != 1
            );
            assert_eq!(
                speeds.windows(2).any(|a| (a[0] - a[1]).abs() > 1e-4),
                mode != 1
            );
            let mut again = Vec::new();
            runtime.sample(0.0, &mut again);
            assert_eq!(again, birth);
        }
    }

    #[test]
    fn cylinder_model_binding_for_powder_uses_each_child_birth_time() {
        use crate::avfx::{CylinderModelEmitterData, EmitterDataRotation};
        let mut file = powder_emission_fixture();
        let emitter = &mut file.emitters[0];
        emitter.data = Some(AvfxEmitterData::CylinderModel(CylinderModelEmitterData {
            generate_method: 3,
            divide_x: 4,
            divide_y: 2,
            radius: frame_ramp(2.0, 4.0, 0, 20),
            length: frame_ramp(4.0, 8.0, 0, 20),
            rotation: EmitterDataRotation {
                angles: [
                    Default::default(),
                    frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 20),
                    Default::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        }));
        emitter.position.x = Some(frame_ramp(0.0, 4.0, 0, 20));
        emitter.particle_items[0].parent_influence_coord = 1;
        emitter.particle_items[0].influence_coord_pos = true;
        file.particles[0].loop_end = 4;
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .injection_model_index = -1;
        for frame in [30.0, 20.0, 0.0, 30.0] {
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
            assert_eq!(out.len(), if frame == 0.0 { 1 } else { 3 });
            for (i, particle) in out.iter().enumerate() {
                let birth = i as f32 * 10.0;
                let angle = birth / 20.0 * std::f32::consts::FRAC_PI_2;
                let radius = 2.0 + birth / 10.0;
                assert_coordinate(
                    particle.position,
                    [
                        birth / 5.0 + radius * angle.sin(),
                        -radius,
                        radius * angle.cos(),
                    ],
                );
            }
        }
    }

    #[test]
    fn cone_model_binding_tracks_shape_and_parent_for_particles_and_child_emitters() {
        use crate::avfx::{ConeModelEmitterData, EmitterDataRotation};
        for children in [false, true] {
            for follow in [false, true] {
                let mut file = creation_count_fixture(children, 1, 9, 0.0);
                let emitter = &mut file.emitters[0];
                emitter.position = axis3(Some(1.0), Some(2.0), Some(3.0));
                emitter.position.x = Some(frame_ramp(1.0, 5.0, 0, 20));
                emitter.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
                emitter.data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                    generate_method: 3,
                    divide_x: 4,
                    divide_y: 2,
                    radius: frame_ramp(2.0, 3.0, 0, 20),
                    injection_angle: frame_ramp(
                        std::f32::consts::FRAC_PI_4,
                        std::f32::consts::FRAC_PI_2,
                        0,
                        20,
                    ),
                    rotation: EmitterDataRotation {
                        angles: [
                            Default::default(),
                            Default::default(),
                            frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 20),
                        ],
                        ..Default::default()
                    },
                    ..Default::default()
                }));
                let item = if children {
                    &mut emitter.emitter_items[0]
                } else {
                    &mut emitter.particle_items[0]
                };
                item.parent_influence_coord = 1;
                item.influence_coord_pos = follow;
                if children {
                    file.emitters[1].data = None;
                    file.emitters[1].position = Default::default();
                    file.emitters[1].particle_items[0].parent_influence_coord = 2;
                }
                file.particles[0].life.enabled = false;
                file.particles[0].loop_end = 4;
                let runtime = VfxRuntime::new(&file);
                for frame in [20.0, 0.0, 10.0, 20.0] {
                    let age = if follow { frame } else { 0.0 };
                    let rotation = age / 20.0 * std::f32::consts::FRAC_PI_2;
                    let radius = 2.0 + age / 20.0;
                    let spread = std::f32::consts::FRAC_PI_4 * (1.0 + age / 20.0);
                    let mut out = Vec::new();
                    runtime.sample(frame / AVFX_FPS, &mut out);
                    assert_eq!(out.len(), 9);
                    for (index, particle) in out.iter().enumerate() {
                        let (ring, sector) = if index == 0 {
                            (0, 0)
                        } else {
                            ((index - 1) / 4 + 1, (index - 1) % 4)
                        };
                        let azimuth = sector as f32 * std::f32::consts::FRAC_PI_2 + rotation;
                        let polar = ring as f32 * spread / 2.0;
                        let expected = [
                            1.0 + age / 5.0 - 2.0 * radius * polar.sin() * azimuth.sin(),
                            2.0 - 3.0 * radius * polar.sin() * azimuth.cos(),
                            3.0 + 4.0 * radius * polar.cos(),
                        ];
                        assert_coordinate(particle.position, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn cone_model_to_vertex_binding_keeps_negative_index_arithmetic() {
        use crate::avfx::ConeModelEmitterData;
        for x in [1, 4] {
            let mut file = creation_count_fixture(false, 1, 1, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                generate_method: 1,
                divide_x: x,
                divide_y: 2,
                radius: frame_ramp(1.0, 3.0, 0, 8),
                injection_angle: linear_curve(std::f32::consts::FRAC_PI_2),
                ..Default::default()
            }));
            file.emitters[0].particle_items[0].parent_influence_coord = 1;
            for follow in [false, true] {
                file.emitters[0].particle_items[0].influence_coord_pos = follow;
                for frame in [0.0, 8.0] {
                    let mut out = Vec::new();
                    VfxRuntime::new(&file).sample(frame / AVFX_FPS, &mut out);
                    assert_eq!(out.len(), 1);
                    let radius = if follow { 1.0 + frame / 4.0 } else { 0.0 };
                    let angle: f32 = if x == 1 {
                        std::f32::consts::FRAC_PI_4
                    } else {
                        0.0
                    };
                    assert_coordinate(
                        out[0].position,
                        [0.0, radius * angle.sin(), radius * angle.cos()],
                    );
                }
            }
        }
    }

    #[test]
    fn cone_model_binding_preserves_first_coefficients_across_shape_animation() {
        use crate::avfx::{ConeModelEmitterData, EmitterDataRotation};
        let mut file = creation_count_fixture(false, 1, 9, 0.0);
        file.emitters[0].position = Default::default();
        file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
            generate_method: 3,
            divide_x: 4,
            divide_y: 2,
            radius_random: AvfxCurve {
                random_type: 1,
                ..frame_ramp(1.0, 3.0, 0, 10)
            },
            rotation: EmitterDataRotation {
                angles_random: [
                    AvfxCurve {
                        random_type: 1,
                        ..frame_ramp(0.25, 0.5, 0, 10)
                    },
                    Default::default(),
                    Default::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        }));
        file.emitters[0].particle_items[0].parent_influence_coord = 1;
        file.emitters[0].particle_items[0].influence_coord_pos = true;
        file.particles[0].loop_end = 4;
        let runtime = VfxRuntime::new(&file);
        let mut first = None;
        for frame in [0.0, 10.0, 2.0, 10.0] {
            let mut out = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 9);
            for particle in out {
                let [x, y, z] = particle.position;
                assert!(x.abs() < 1e-6);
                let coefficients = [
                    y.hypot(z) / (1.0 + 0.2 * frame),
                    (-y).atan2(z) / (0.25 + 0.025 * frame),
                ];
                let expected = *first.get_or_insert(coefficients);
                for (a, b) in coefficients.into_iter().zip(expected) {
                    assert!(a > 0.0 && a <= 1.0);
                    assert!((a - b).abs() < 1e-5, "{coefficients:?} != {expected:?}");
                }
            }
        }
    }

    #[test]
    fn cone_model_binding_for_powder_uses_each_child_birth_time() {
        use crate::avfx::{ConeModelEmitterData, EmitterDataRotation};
        let mut file = powder_emission_fixture();
        let emitter = &mut file.emitters[0];
        emitter.data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
            generate_method: 3,
            divide_x: 4,
            divide_y: 2,
            radius: frame_ramp(2.0, 4.0, 0, 20),
            rotation: EmitterDataRotation {
                angles: [
                    Default::default(),
                    frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 20),
                    Default::default(),
                ],
                ..Default::default()
            },
            ..Default::default()
        }));
        emitter.position.x = Some(frame_ramp(0.0, 4.0, 0, 20));
        emitter.particle_items[0].parent_influence_coord = 1;
        emitter.particle_items[0].influence_coord_pos = true;
        file.particles[0].loop_end = 4;
        file.particles[0]
            .simple
            .as_mut()
            .unwrap()
            .injection_model_index = -1;
        let runtime = VfxRuntime::new(&file);
        for frame in [30.0, 20.0, 0.0, 30.0] {
            let mut out = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut out);
            assert_eq!(out.len(), if frame == 0.0 { 1 } else { 3 });
            for (i, particle) in out.iter().enumerate() {
                let birth = i as f32 * 10.0;
                let angle = birth / 20.0 * std::f32::consts::FRAC_PI_2;
                let radius = 2.0 + birth / 10.0;
                assert_coordinate(
                    particle.position,
                    [
                        birth / 5.0 + radius * angle.sin(),
                        0.0,
                        radius * angle.cos(),
                    ],
                );
            }
        }
    }

    #[test]
    fn cone_model_ordered_births_use_two_dimensional_vertices_and_method_origin() {
        use crate::avfx::ConeModelEmitterData;
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let directions = [
            [0.0, 0.0, 1.0],
            [0.0, -h, h],
            [h, 0.0, h],
            [0.0, h, h],
            [-h, 0.0, h],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
        ];
        for method in [1, 3, 5, 7, 259] {
            let mut file = creation_count_fixture(false, 1, 18, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                generate_method: method,
                divide_x: 260,
                divide_y: 258,
                radius: linear_curve(2.0),
                injection_angle: linear_curve(std::f32::consts::FRAC_PI_2),
                injection_speed: linear_curve(-0.25),
                ..Default::default()
            }));
            let runtime = VfxRuntime::new(&file);
            for frame in [0.0, 2.0, 0.0] {
                let mut out = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 18);
                let distance = if method % 4 < 2 { 0.0 } else { 2.0 } - 0.25 * frame;
                for (index, quad) in out.iter().enumerate() {
                    assert_coordinate(quad.position, directions[index % 9].map(|v| v * distance));
                }
            }
        }
    }

    #[test]
    fn cone_model_random_births_stay_on_discrete_vertices() {
        use crate::avfx::ConeModelEmitterData;
        let mut file = creation_count_fixture(false, 1, 100, 0.0);
        file.emitters[0].position = Default::default();
        file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
            generate_method: 2,
            divide_x: 4,
            divide_y: 1,
            radius: linear_curve(1.0),
            injection_angle: linear_curve(std::f32::consts::FRAC_PI_2),
            ..Default::default()
        }));
        let mut out = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut out);
        assert_eq!(out.len(), 100);
        let vertices = [
            [0., 0., 1.],
            [0., -1., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [-1., 0., 0.],
        ];
        for particle in &out {
            assert!(
                vertices.iter().any(|vertex| {
                    vertex
                        .iter()
                        .zip(particle.position)
                        .all(|(a, b)| (a - b).abs() < 1e-5)
                }),
                "{:?}",
                particle.position
            );
        }
        for vertex in vertices {
            assert!(out.iter().any(|p| {
                p.position
                    .iter()
                    .zip(vertex)
                    .all(|(a, b)| (a - b).abs() < 1e-5)
            }));
        }
    }

    #[test]
    fn cone_model_direction_override_uses_xyz_and_preserves_birth_point() {
        use crate::avfx::{ConeModelEmitterData, EmitterDataRotation};
        for method in [1, 3] {
            let mut file = creation_count_fixture(true, 1, 1, 0.0);
            file.emitters[0].position = axis3(Some(1.0), Some(2.0), Some(3.0));
            file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
            file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                generate_method: method,
                divide_x: 4,
                divide_y: 2,
                radius: linear_curve(0.5),
                injection_speed: linear_curve(0.5),
                rotation: EmitterDataRotation {
                    angles: [
                        Default::default(),
                        linear_curve(std::f32::consts::FRAC_PI_2),
                        Default::default(),
                    ],
                    ..Default::default()
                },
                ..Default::default()
            }));
            file.emitters[0].emitter_items[0].parent_influence_coord = 0;
            file.emitters[1].position = Default::default();
            file.emitters[1].data = None;
            file.emitters[1].particle_items[0].parent_influence_coord = 2;
            let mut before = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut before);
            assert_eq!(before.len(), 1);
            let birth_x = if method == 3 { 0.0 } else { 1.0 };
            assert_coordinate(before[0].position, [birth_x - 1.0, 2.0, 3.0]);
            file.emitters[0].injection_angle = [
                linear_curve(std::f32::consts::FRAC_PI_2),
                linear_curve(std::f32::consts::FRAC_PI_2),
                linear_curve(std::f32::consts::FRAC_PI_2),
            ];
            file.emitters[0].rotation_order = 5;
            let mut disabled = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut disabled);
            assert_eq!(disabled, before);
            file.emitters[0].any_direction = true;
            let mut enabled = Vec::new();
            VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut enabled);
            if method == 1 {
                assert_eq!(enabled, before);
            } else {
                assert_coordinate(enabled[0].position, [birth_x, 2.0, 2.0]);
            }
        }
    }

    #[test]
    fn cone_model_first_radius_and_speed_randomness_belong_to_emitter() {
        use crate::avfx::ConeModelEmitterData;
        for mode in [1, 4] {
            let mut file = creation_count_fixture(false, 1, 16, 0.0);
            file.emitters[0].position = Default::default();
            file.emitters[0].data = Some(AvfxEmitterData::ConeModel(ConeModelEmitterData {
                generate_method: 3,
                divide_x: 4,
                divide_y: 2,
                radius: linear_curve(1.0),
                radius_random: AvfxCurve {
                    random_type: mode,
                    ..linear_curve(1.0)
                },
                injection_speed: linear_curve(2.0),
                injection_speed_random: AvfxCurve {
                    random_type: mode,
                    ..linear_curve(1.0)
                },
                ..Default::default()
            }));
            let runtime = VfxRuntime::new(&file);
            let mut birth = Vec::new();
            let mut later = Vec::new();
            runtime.sample(0.0, &mut birth);
            runtime.sample(1.0 / AVFX_FPS, &mut later);
            let values: Vec<_> = birth
                .iter()
                .zip(&later)
                .map(|(a, b)| [a.position[2], b.position[2] - a.position[2]])
                .collect();
            assert_eq!(values.len(), 16);
            assert!(
                values
                    .iter()
                    .all(|v| (1.0..=2.0).contains(&v[0]) && (2.0..=3.0).contains(&v[1]))
            );
            assert!(values.iter().any(|v| v[0] > 1.01 && v[1] > 2.01));
            if mode == 1 {
                assert!(values.iter().all(|v| *v == values[0]));
            } else {
                assert!(values.iter().any(|v| *v != values[0]));
            }
        }
    }

    #[test]
    fn cone_birth_distance_and_velocity_share_the_scattered_direction() {
        let mut file = creation_count_fixture(false, 1, 32, 0.0);
        let emitter = &mut file.emitters[0];
        emitter.position = Default::default();
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            inner_size: linear_curve(2.0),
            outer_size: linear_curve(2.0),
            injection_speed: linear_curve(-0.5),
            injection_angle: linear_curve(0.7),
            ..Default::default()
        }));
        file.particles[0].life.enabled = false;
        let runtime = VfxRuntime::new(&file);
        let mut birth = Vec::new();
        let mut later = Vec::new();
        runtime.sample(0.0, &mut birth);
        runtime.sample(2.0 / AVFX_FPS, &mut later);
        assert_eq!(birth.len(), 32);
        assert_eq!(later.len(), 32);
        for (start, end) in birth.iter().zip(&later) {
            assert!((start.position.iter().map(|x| x * x).sum::<f32>() - 4.0).abs() < 2e-5);
            assert!(start.position[2] >= 2.0 * 0.7f32.cos() - 2e-5);
            assert_coordinate(end.position, start.position.map(|x| x * 0.5));
        }
        assert!(birth.iter().any(|p| p.position[0].abs() > 0.1));
        assert!(birth.iter().any(|p| p.position[1].abs() > 0.1));
        let mut repeated = Vec::new();
        runtime.sample(0.0, &mut repeated);
        assert_eq!(repeated, birth);
    }

    #[test]
    fn cone_shape_rotation_precedes_parent_matrix_and_ignores_emitter_iax() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        let root = &mut file.emitters[0];
        root.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        root.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
        root.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            rotation: crate::avfx::EmitterDataRotation {
                angles: [
                    linear_curve(std::f32::consts::FRAC_PI_2),
                    Default::default(),
                    Default::default(),
                ],
                ..Default::default()
            },
            inner_size: linear_curve(2.0),
            outer_size: linear_curve(2.0),
            injection_speed: linear_curve(0.5),
            ..Default::default()
        }));
        root.emitter_items[0].parent_influence_coord = 0;
        file.emitters[1].data = None;
        file.emitters[1].position = Default::default();
        file.emitters[1].particle_items[0].parent_influence_coord = 2;
        file.particles[0].life.enabled = false;
        let mut expected = Vec::new();
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut expected);
        assert_eq!(expected.len(), 1);
        assert_coordinate(expected[0].position, [1.0, -5.0, 3.0]);
        file.emitters[0].injection_angle =
            [linear_curve(0.4), linear_curve(-0.7), linear_curve(1.2)];
        file.emitters[0].injection_angle_random =
            [linear_curve(0.1), linear_curve(0.2), linear_curve(0.3)];
        file.emitters[0].any_direction = true;
        let mut actual = Vec::new();
        VfxRuntime::new(&file).sample(2.0 / AVFX_FPS, &mut actual);
        assert_eq!(actual, expected);
    }

    #[test]
    fn cone_random_speed_is_shared_for_first_and_redrawn_for_always() {
        let mut file = creation_count_fixture(false, 1, 16, 0.0);
        file.emitters[0].position = Default::default();
        file.particles[0].life.enabled = false;
        for mode in 0..6 {
            file.emitters[0].data = Some(AvfxEmitterData::Cone(ConeEmitterData {
                injection_speed: linear_curve(2.0),
                injection_speed_random: AvfxCurve {
                    random_type: mode,
                    ..linear_curve(1.0)
                },
                ..Default::default()
            }));
            let runtime = VfxRuntime::new(&file);
            let mut out = Vec::new();
            runtime.sample(1.0 / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 16);
            let speeds: Vec<_> = out.iter().map(|p| p.position[2]).collect();
            let (min, max) = match mode % 3 {
                1 => (2.0, 3.0),
                2 => (1.0, 2.0),
                _ => (1.0, 3.0),
            };
            assert!(
                speeds.iter().all(|s| (min..=max).contains(s)),
                "mode={mode}: {speeds:?}"
            );
            assert!(
                speeds.iter().any(|s| (*s - 2.0).abs() > 0.01),
                "mode={mode}: random speed ignored"
            );
            if mode < 3 {
                assert!(speeds.iter().all(|s| *s == speeds[0]));
                let percent = (speeds[0] - 2.0) * 100.0;
                assert!((percent - percent.round()).abs() < 2e-5);
            } else {
                assert!(speeds.iter().any(|s| *s != speeds[0]));
            }
        }
    }

    #[test]
    fn cone_first_rotation_and_speed_share_coefficients_across_birth_ages() {
        let mut file = creation_count_fixture(false, 0, 1, 1.0);
        let emitter = &mut file.emitters[0];
        emitter.position = Default::default();
        emitter.create_interval = linear_curve(2.0);
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            rotation: crate::avfx::EmitterDataRotation {
                angles_random: [
                    AvfxCurve {
                        random_type: 1,
                        ..frame_ramp(0.1, 0.6, 0, 10)
                    },
                    Default::default(),
                    Default::default(),
                ],
                ..Default::default()
            },
            inner_size: linear_curve(1.0),
            outer_size: linear_curve(1.0),
            injection_speed: linear_curve(2.0),
            injection_speed_random: AvfxCurve {
                random_type: 1,
                ..frame_ramp(1.0, 3.5, 0, 10)
            },
            ..Default::default()
        }));
        file.particles[0].life.enabled = false;
        let runtime = VfxRuntime::new(&file);
        let mut coefficients = None::<[f32; 2]>;
        let mut first_sample = Vec::new();
        for frame in [5.0, 9.0, 5.0] {
            let mut out = Vec::new();
            runtime.sample(frame / AVFX_FPS, &mut out);
            assert_eq!(out.len(), (frame / 2.0) as usize + 1);
            for (index, particle) in out.iter().enumerate() {
                let birth = index as f32 * 2.0;
                let age = frame - birth;
                let [x, y, z] = particle.position;
                assert!(x.abs() < 1e-6);
                // Rotation around X tilts +Z toward -Y. Birth radius is 1,
                // then the particle moves at its fixed birth speed.
                let length = (y * y + z * z).sqrt();
                let angle = (-y).atan2(z);
                let speed = (length - 1.0) / age;
                let actual = [
                    angle / (0.1 + 0.05 * birth),
                    (speed - 2.0) / (1.0 + 0.25 * birth),
                ];
                let expected = coefficients.get_or_insert(actual);
                for (actual, expected) in actual.into_iter().zip(*expected) {
                    assert!(actual > 0.0 && actual <= 1.0);
                    assert!((actual - expected).abs() < 2e-6);
                    assert!((actual * 100.0 - (actual * 100.0).round()).abs() < 2e-4);
                }
            }
            if frame == 5.0 {
                if first_sample.is_empty() {
                    first_sample = out;
                } else {
                    assert_eq!(out, first_sample);
                }
            }
        }
    }

    #[test]
    fn child_emitter_shape_birth_uses_current_initial_and_optional_parent_spaces() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        file.models.push(crate::avfx::VfxModelGeometry {
            emit_vertices: vec![crate::avfx::VfxEmitVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                color: [255; 4],
            }],
            emit_vertex_numbers: vec![0],
            ..Default::default()
        });
        let parent = &mut file.emitters[0];
        parent.data = Some(AvfxEmitterData::Model(crate::avfx::ModelEmitterData {
            model_index: 0,
            generate_method: 3,
            ..Default::default()
        }));
        parent.position = axis3(Some(10.0), None, None);
        parent.position.x = Some(frame_ramp(10.0, 14.0, 0, 4));
        parent.rotation.z = Some(frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 4));
        parent.scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        parent.scale.x = Some(frame_ramp(2.0, 4.0, 0, 4));
        file.emitters[1].data = None;
        file.emitters[1].position = axis3(Some(0.5), None, None);
        file.emitters[1].particle_items[0].parent_influence_coord = 2;
        file.particles[0].position = Default::default();
        for (mode, follow, birth, later) in [
            (0, false, [12.5, 0.0, 0.0], [12.5, 0.0, 0.0]),
            (1, false, [12.5, 0.0, 0.0], [12.5, 0.0, 0.0]),
            (1, true, [12.5, 0.0, 0.0], [14.5, 4.0, 0.0]),
            (2, false, [13.0, 0.0, 0.0], [14.0, 6.0, 0.0]),
            (3, false, [13.0, 0.0, 0.0], [13.0, 0.0, 0.0]),
        ] {
            let item = &mut file.emitters[0].emitter_items[0];
            item.parent_influence_coord = mode;
            item.influence_coord_pos = follow;
            item.influence_coord_rot = false;
            item.influence_coord_scale = false;
            let runtime = VfxRuntime::new(&file);
            for (frame, expected) in [(0.0, birth), (4.0, later), (0.0, birth)] {
                let mut out = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 1, "PICd={mode} follow={follow} frame={frame}");
                assert_coordinate(out[0].position, expected);
            }
        }
    }

    #[test]
    fn child_emitter_local_direction_applies_after_ccot_and_before_birth_point() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        file.emitters[0].position = Default::default();
        file.emitters[0].data = Some(AvfxEmitterData::Cone(upward_cone()));
        file.emitters[0].emitter_items[0].local_direction = 1;
        let child = &mut file.emitters[1];
        child.data = None;
        child.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        child.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        child.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
        child.particle_items[0].parent_influence_coord = 2;
        file.particles[0].position = Default::default();
        let positions = [
            [1.0, 2.0, 3.0],
            [-6.0, -2.0, 12.0],
            [-2.0, 6.0, 12.0],
            [4.0, 3.0, 12.0],
            [-2.0, 1.0, 3.0],
            [1.0, 2.0, 3.0],
        ];
        for mode in [0, 1, 2, 3] {
            file.emitters[0].emitter_items[0].parent_influence_coord = mode;
            for order in 0..6 {
                file.emitters[1].coord_compute_order = order;
                let runtime = VfxRuntime::new(&file);
                let mut count = 0;
                runtime.sample_ambient(0.0, &mut |ctx, _, _| {
                    count += 1;
                    let world = ctx.emitter.world();
                    let [x, y, z] = positions[order as usize];
                    assert_coordinate(world.position, [-x, z, y]);
                    let basis = if matches!(order, 2 | 3 | 5) {
                        [[0.0, 0.0, 3.0], [-2.0, 0.0, 0.0], [0.0, 4.0, 0.0]]
                    } else {
                        [[0.0, 0.0, -2.0], [3.0, 0.0, 0.0], [0.0, 4.0, 0.0]]
                    };
                    for (actual, expected) in world.linear.into_iter().zip(basis) {
                        assert_coordinate(actual, expected);
                    }
                });
                assert_eq!(count, 1);
            }
        }
    }

    #[test]
    fn child_emitter_injected_motion_uses_own_resistance_and_parent_following() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        let parent = &mut file.emitters[0];
        parent.position = Default::default();
        parent.rotation.z = Some(frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 4));
        parent.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(2.0),
            ..upward_cone()
        }));
        parent.air_resistance = linear_curve(50.0);
        let child = &mut file.emitters[1];
        child.data = None;
        child.position = Default::default();
        child.air_resistance = frame_ramp(1.0, 3.0, 0, 4);
        child.gravity = linear_curve(0.5);
        child.particle_items[0].parent_influence_coord = 2;
        for mode in [0, 2, 3] {
            for local in [0, 1] {
                let item = &mut file.emitters[0].emitter_items[0];
                item.parent_influence_coord = mode;
                item.local_direction = local;
                let runtime = VfxRuntime::new(&file);
                let mut out = Vec::new();
                runtime.sample(4.0 / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 1);
                assert_coordinate(
                    out[0].position,
                    if mode == 2 {
                        [-16.0, 4.0, 0.0]
                    } else {
                        [0.0, 20.0, 0.0]
                    },
                );
            }
        }
    }

    #[test]
    fn child_particle_ipbv_inherits_parent_motion_in_world_space() {
        fn sample(inherit: bool) -> [f32; 3] {
            let mut file = creation_count_fixture(true, 1, 1, 0.0);
            let parent = &mut file.emitters[0];
            parent.position = Default::default();
            parent.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
                injection_speed: linear_curve(2.0),
                ..upward_cone()
            }));
            parent.air_resistance = linear_curve(50.0);
            let child = &mut file.emitters[1];
            child.data = None;
            child.position = Default::default();
            child.particle_items[0].inherit_parent_velocity = inherit;
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(4.0 / AVFX_FPS, &mut out);
            assert_eq!(out.len(), 1);
            out[0].position
        }

        let without = sample(false);
        let with = sample(true);
        assert!(with[1] > without[1]);
        assert!((with[0] - without[0]).abs() < 1.0e-5);
        assert!((with[2] - without[2]).abs() < 1.0e-5);
    }

    #[test]
    fn child_emitter_optional_components_are_separate_from_position_following() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        let parent = &mut file.emitters[0];
        parent.data = None;
        parent.position = Default::default();
        parent.position.x = Some(frame_ramp(0.0, 4.0, 0, 4));
        parent.rotation.z = Some(frame_ramp(0.0, std::f32::consts::FRAC_PI_2, 0, 4));
        parent.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
        let child = &mut file.emitters[1];
        child.data = None;
        child.position = axis3(Some(1.0), Some(2.0), Some(3.0));
        child.rotation.z = Some(linear_curve(std::f32::consts::FRAC_PI_2));
        child.scale = axis3(Some(0.5), Some(2.0), Some(1.0));
        child.particle_items[0].parent_influence_coord = 2;
        for bits in 0..8 {
            let item = &mut file.emitters[0].emitter_items[0];
            item.parent_influence_coord = 1;
            item.influence_coord_scale = bits & 1 != 0;
            item.influence_coord_rot = bits & 2 != 0;
            item.influence_coord_pos = bits & 4 != 0;
            let runtime = VfxRuntime::new(&file);
            let mut count = 0;
            runtime.sample_ambient(4.0, &mut |ctx, _, _| {
                count += 1;
                let world = ctx.emitter_now.world();
                assert_coordinate(
                    world.position,
                    [if bits & 4 != 0 { 5.0 } else { 1.0 }, 2.0, 3.0],
                );
                let [sx, sy, sz] = if bits & 1 != 0 {
                    [-1.0, 6.0, 4.0]
                } else {
                    [0.5, 2.0, 1.0]
                };
                let basis = if bits & 2 != 0 {
                    [[-sx, 0.0, 0.0], [0.0, -sy, 0.0], [0.0, 0.0, sz]]
                } else {
                    [[0.0, sx, 0.0], [-sy, 0.0, 0.0], [0.0, 0.0, sz]]
                };
                for (actual, expected) in world.linear.into_iter().zip(basis) {
                    assert_coordinate(actual, expected);
                }
            });
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn child_emitter_prewarm_and_finish_freeze_external_parent_without_resetting_motion() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        let parent = &mut file.emitters[0];
        parent.position = Default::default();
        parent.position.x = Some(frame_ramp(0.0, 10.0, 0, 10));
        parent.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(2.0),
            ..upward_cone()
        }));
        parent.emitter_items[0].parent_influence_coord = 2;
        parent.emitter_items[0].start_frame = 2;
        let child = &mut file.emitters[1];
        child.data = None;
        child.life.enabled = true;
        child.life.value = 6.0;
        child.position = Default::default();
        child.position.y = Some(frame_ramp(0.0, 10.0, 0, 10));
        child.particle_items[0].parent_influence_coord = 2;
        for prewarm in [false, true] {
            file.emitters[0].emitter_items[0].start_frame_null_update = prewarm;
            let runtime = VfxRuntime::new(&file);
            for (frame, expected) in if prewarm {
                [
                    (0.0, [0.0, 6.0, 0.0]),
                    (1.0, [1.0, 9.0, 0.0]),
                    (8.0, [4.0, 18.0, 0.0]),
                    (0.0, [0.0, 6.0, 0.0]),
                ]
            } else {
                [
                    (0.0, [0.0, 0.0, 0.0]),
                    (1.0, [1.0, 5.0, 0.0]),
                    (8.0, [4.0, 14.0, 0.0]),
                    (0.0, [0.0, 0.0, 0.0]),
                ]
            } {
                let mut out = Vec::new();
                runtime.sample(frame / AVFX_FPS, &mut out);
                assert_eq!(out.len(), 1);
                assert_coordinate(out[0].position, expected);
            }
        }
    }

    #[test]
    fn point_root_preserves_zero_direction_in_all_coordinate_modes() {
        let mut file = creation_count_fixture(false, 1, 1, 0.0);
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.emitters[0].rotation = axis3(Some(0.7), Some(-0.4), Some(0.9));
        file.emitters[0].scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
        file.particles[0].rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        file.particles[0].position = axis3(Some(0.2), Some(0.3), Some(0.4));
        for data in [None, Some(AvfxEmitterData::None)] {
            file.emitters[0].data = data;
            for mode in [0, 1, 2, 3, 8] {
                file.emitters[0].particle_items[0].parent_influence_coord = mode;
                for kind in [ParticleType::Quad, ParticleType::LightModel] {
                    file.particles[0].particle_type = Some(kind);
                    file.particles[0].data = if kind == ParticleType::LightModel {
                        AvfxParticleData::LightModel { model_index: 0 }
                    } else {
                        AvfxParticleData::None
                    };
                    let sample = |local| {
                        let mut file = file.clone();
                        file.emitters[0].particle_items[0].local_direction = local;
                        let runtime = VfxRuntime::new(&file);
                        let mut result = (Vec::new(), Vec::new());
                        runtime.sample(0.1, &mut result.0);
                        runtime.sample_mesh(0.1, &mut result.1);
                        assert_eq!(result.0.len() + result.1.len(), 1);
                        result
                    };
                    assert_eq!(sample(1), sample(0), "{kind:?} PICd={mode}");
                }
            }
        }
    }

    #[test]
    fn point_chain_forwards_shape_direction_without_reapplying_inverse() {
        let mut file = creation_count_fixture(true, 1, 1, 0.0);
        file.emitters[0].data = Some(AvfxEmitterData::Cone(upward_cone()));
        file.emitters[0].position = Default::default();
        file.emitters[0].emitter_items[0].parent_influence_coord = 2;
        let point = &mut file.emitters[1];
        point.emitter_type = Some(EmitterType::Point);
        point.data = None;
        point.position = Default::default();
        point.rotation = axis3(Some(0.6), Some(-0.5), Some(0.7));
        point.scale = axis3(Some(-2.0), Some(3.0), Some(4.0));
        point.particle_items[0].local_direction = 1;
        point.particle_items[0].parent_influence_coord = 2;
        let leaf = point.clone();
        let mut item = file.emitters[1].particle_items.remove(0);
        item.local_direction = 0;
        item.target_index = 2;
        file.emitters[1].emitter_items.push(item);
        file.emitters.push(leaf);
        file.particles[0].position = axis3(Some(0.2), Some(0.3), Some(0.4));
        file.particles[0].rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        let expected = [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
        for frame in [0.0, 4.0, 0.0] {
            let runtime = VfxRuntime::new(&file);
            let mut count = 0;
            runtime.sample_ambient(frame, &mut |ctx, item, particle| {
                count += 1;
                for (actual, expected) in ctx
                    .motion
                    .drawing_parent(VFX_IDENTITY_BASIS)
                    .into_iter()
                    .zip(expected)
                {
                    assert_coordinate(actual, expected);
                }
                let mut quads = Vec::new();
                runtime.push_quad(ctx, item, particle, &mut quads);
                assert_coordinate(
                    quads[0].position,
                    ctx.emitter.world().transform_point([-0.2, 0.4, 0.3]),
                );
            });
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn emitter_vr_does_not_inject_random_velocity_into_particles() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.position = Default::default();
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        emitter.data = None;
        emitter.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(Default::default()),
            ..Default::default()
        });
        file.particles[0].rotation_velocity[0] = linear_curve(0.7);
        for shape in [
            None,
            Some(AvfxEmitterData::Cone(ConeEmitterData {
                injection_speed: linear_curve(0.5),
                ..Default::default()
            })),
        ] {
            file.emitters[0].data = shape;
            for kind in [ParticleType::Quad, ParticleType::LightModel] {
                file.particles[0].particle_type = Some(kind);
                file.particles[0].data = if kind == ParticleType::LightModel {
                    AvfxParticleData::LightModel { model_index: 0 }
                } else {
                    AvfxParticleData::None
                };
                let baseline = VfxRuntime::new(&file);
                let mut steered = file.clone();
                for axis in 0..3 {
                    steered.emitters[0].rotation_velocity[axis] = linear_curve(0.2 + axis as f32);
                    steered.emitters[0].rotation_velocity_random[axis] = linear_curve(0.8);
                    steered.emitters[0].rotation_velocity_random[axis].random_type = 1;
                }
                let steered = VfxRuntime::new(&steered);
                for frame in [0.0, 4.0, 12.0, 4.0] {
                    let (mut expected_quads, mut expected_meshes) = (Vec::new(), Vec::new());
                    let (mut actual_quads, mut actual_meshes) = (Vec::new(), Vec::new());
                    baseline.sample(frame / AVFX_FPS, &mut expected_quads);
                    baseline.sample_mesh(frame / AVFX_FPS, &mut expected_meshes);
                    steered.sample(frame / AVFX_FPS, &mut actual_quads);
                    steered.sample_mesh(frame / AVFX_FPS, &mut actual_meshes);
                    assert_eq!(expected_quads.len() + expected_meshes.len(), 1);
                    assert_eq!(actual_quads, expected_quads, "{kind:?} frame={frame}");
                    assert_eq!(actual_meshes, expected_meshes, "{kind:?} frame={frame}");
                }
            }
        }
    }

    #[test]
    fn injection_direction_uses_normalized_full_parent_matrix() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.life.enabled = false;
        emitter.position = Default::default();
        emitter.create_count = linear_curve(1.0);
        emitter.create_interval = linear_curve(1000.0);
        emitter.particle_items[0].create_count = 1;
        let mut shape = upward_cone();
        shape.rotation.angles[2] = linear_curve(-std::f32::consts::FRAC_PI_4);
        emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(0.5),
            ..shape
        }));
        for scale in [[2.0_f32, 3.0, 4.0], [-2.0, 3.0, 4.0], [0.0; 3]] {
            file.emitters[0].scale = axis3(Some(scale[0]), Some(scale[1]), Some(scale[2]));
            let mut out = Vec::new();
            VfxRuntime::new(&file).sample(4.0 / AVFX_FPS, &mut out);
            let expected = if scale == [0.0; 3] {
                [0.0; 3]
            } else {
                [2.0 * scale[0] / 13.0_f32.sqrt(), 6.0 / 13.0_f32.sqrt(), 0.0]
            };
            for (actual, expected) in out[0].position.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 1e-5,
                    "{scale:?}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn vr_motion_reaches_meshes_and_each_powder_child_birth() {
        let mut file = powder_emission_fixture();
        file.models[0].emit_vertices[0].position = [0.0; 3];
        file.models[0].draw = Some(Default::default());
        file.emitters[0].data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            injection_speed: linear_curve(0.5),
            ..upward_cone()
        }));
        // The +Y injection basis maps a +90 degree X angle to world -Z.
        file.particles[0].rotation_velocity[0] = linear_curve(std::f32::consts::FRAC_PI_2);
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(25.0 / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 3);
        for (quad, z) in quads.iter().zip([0.0, -5.0, -10.0]) {
            assert!(quad.position[0].abs() < 1e-5 && quad.position[1].abs() < 1e-5);
            assert!((quad.position[2] - z).abs() < 1e-5);
        }
        for kind in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::Windmill,
            ParticleType::LightModel,
        ] {
            let particle = &mut file.particles[0];
            particle.particle_type = Some(kind);
            particle.simple_anim_enable = false;
            particle.data = match kind {
                ParticleType::LightModel => AvfxParticleData::LightModel { model_index: 0 },
                ParticleType::Windmill => AvfxParticleData::Windmill { uv_type: 0 },
                _ => Default::default(),
            };
            let runtime = VfxRuntime::new(&file);
            let mut meshes = Vec::new();
            runtime.sample(20.0 / AVFX_FPS, &mut quads);
            runtime.sample_mesh(20.0 / AVFX_FPS, &mut meshes);
            let position = quads
                .first()
                .map(|q| q.position)
                .or_else(|| meshes.first().map(|m| m.position))
                .unwrap();
            assert!(position[0].abs() < 1e-5 && position[1].abs() < 1e-5);
            assert!((position[2] + 10.0).abs() < 1e-5, "{kind:?}: {position:?}");
        }
    }

    #[test]
    fn vr_curves_do_not_add_drawing_rotation() {
        for kind in [
            ParticleType::Quad,
            ParticleType::Powder,
            ParticleType::Windmill,
            ParticleType::LightModel,
        ] {
            let mut baseline = powder_emission_fixture();
            baseline.models[0].draw = Some(Default::default());
            let particle = &mut baseline.particles[0];
            particle.particle_type = Some(kind);
            particle.simple_anim_enable = false;
            particle.data = match kind {
                ParticleType::LightModel => AvfxParticleData::LightModel { model_index: 0 },
                ParticleType::Windmill => AvfxParticleData::Windmill { uv_type: 0 },
                _ => Default::default(),
            };
            particle.rotation = axis3(Some(0.2), Some(-0.3), Some(0.4));
            particle.rotation.random_z = Some(linear_curve(0.2));
            let rotation_y = particle.rotation.y.as_mut().unwrap();
            rotation_y.keys.push(AvfxCurveKey {
                time: 4,
                z: 0.6,
                ..rotation_y.keys[0]
            });
            particle.loop_end = 4;
            let baseline = VfxRuntime::new(&baseline);
            for random in [false, true] {
                for axis in 0..3 {
                    let mut changed = baseline.file.clone();
                    let particle = &mut changed.particles[0];
                    let curve = if random {
                        &mut particle.rotation_velocity_random[axis]
                    } else {
                        &mut particle.rotation_velocity[axis]
                    };
                    *curve = linear_curve(0.2);
                    curve.keys.push(AvfxCurveKey {
                        time: 4,
                        z: 0.8,
                        ..curve.keys[0]
                    });
                    let changed = VfxRuntime::new(&changed);
                    for frame in [7.0, 2.0, 4.0, 0.0, 5.0, 3.0] {
                        let mut expected_quads = Vec::new();
                        let mut actual_quads = Vec::new();
                        let mut expected_meshes = Vec::new();
                        let mut actual_meshes = Vec::new();
                        baseline.sample(frame / AVFX_FPS, &mut expected_quads);
                        changed.sample(frame / AVFX_FPS, &mut actual_quads);
                        baseline.sample_mesh(frame / AVFX_FPS, &mut expected_meshes);
                        changed.sample_mesh(frame / AVFX_FPS, &mut actual_meshes);
                        assert_eq!(expected_quads.len() + expected_meshes.len(), 1);
                        assert_eq!(actual_quads.len(), expected_quads.len());
                        assert_eq!(actual_meshes.len(), expected_meshes.len());
                        for (actual, expected) in actual_quads.iter().zip(&expected_quads) {
                            assert_eq!(
                                actual.orientation, expected.orientation,
                                "{kind:?}, random={random}, axis={axis}, frame={frame}"
                            );
                        }
                        for (actual, expected) in actual_meshes.iter().zip(&expected_meshes) {
                            assert_eq!(
                                actual.orientation, expected.orientation,
                                "{kind:?}, random={random}, axis={axis}, frame={frame}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn powder_births_capture_integrated_spawner_motion() {
        let mut file = powder_emission_fixture();
        file.models[0].emit_vertices[0].position = [1.0, 0.0, 0.0];
        let particle = &mut file.particles[0];
        particle.gravity = linear_curve(0.0);
        particle.gravity.keys.push(AvfxCurveKey {
            time: 20,
            z: 0.06,
            ..particle.gravity.keys[0]
        });
        particle.rotation_velocity[1] = linear_curve(0.0);
        let rotation = &mut particle.rotation_velocity[1];
        rotation.keys.push(AvfxCurveKey {
            time: 20,
            z: 0.02,
            ..rotation.keys[0]
        });
        particle.rotation.y = Some(rotation.clone());
        let runtime = VfxRuntime::new(&file);
        for age in [25.0, 10.0, 20.0, 0.0] {
            let mut quads = Vec::new();
            runtime.sample(age / AVFX_FPS, &mut quads);
            assert_eq!(quads.len(), ((age / 10.0) as usize + 1).min(3));
            for (birth, quad) in quads.iter().enumerate() {
                let t = birth as f32 * 10.0;
                let angle = 0.001 * t;
                let expected = [angle.cos(), 0.0005 * t.powi(3), -angle.sin()];
                for (actual, expected) in quad.position.into_iter().zip(expected) {
                    assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
                }
            }
        }
    }

    #[test]
    fn sampled_instances_preserve_raw_draw_modes() {
        for draw_mode in [0, 1, 2, 4, 12, 1000] {
            let mut file = fixture_file();
            file.particles[0].draw_mode = draw_mode;
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert!(!quads.is_empty());
            assert!(quads.iter().all(|quad| quad.draw_mode == draw_mode));

            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            file.models.push(crate::avfx::VfxModelGeometry {
                draw: Some(crate::avfx::VfxDrawModel::default()),
                ..Default::default()
            });
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert!(!meshes.is_empty());
            assert!(meshes.iter().all(|mesh| mesh.draw_mode == draw_mode));
        }
    }

    #[test]
    fn facing_preserves_local_rotation_and_signed_scale_for_quads_and_meshes() {
        let mut file = fixture_file();
        let emitter = &mut file.emitters[0];
        emitter.create_count = linear_curve(1.0);
        emitter.particle_items[0].create_count = 1;
        emitter.particle_items[0].parent_influence_coord = 3;
        emitter.rotation = axis3(None, Some(std::f32::consts::FRAC_PI_2), None);
        emitter.scale = axis3(Some(2.0), Some(3.0), Some(4.0));
        file.particles[0].rotation = axis3(Some(std::f32::consts::FRAC_PI_2), None, None);
        file.particles[0].scale = axis3(Some(-0.4), Some(0.2), Some(0.3));
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        for mode in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1000] {
            let particle = &mut file.particles[0];
            particle.rotation_direction_base = mode;
            particle.particle_type = Some(ParticleType::Quad);
            let billboard = particle.is_billboard();
            let expected = if mode == crate::avfx::rotation_direction_base::MOVE_DIRECTION {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ]
            };
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            assert_eq!(quads[0].rotation_direction_base, mode);
            for (actual, expected) in quads[0].facing_parent_basis.into_iter().flatten().zip(
                [[0.0, 0.0, -2.0], [0.0, 3.0, 0.0], [4.0, 0.0, 0.0]]
                    .into_iter()
                    .flatten(),
            ) {
                assert!((actual - expected).abs() < 1e-6, "mode={mode}");
            }
            let expected_size = if billboard { [-0.4, 0.3] } else { [-0.2, 0.1] };
            for (actual, expected) in quads[0].size.into_iter().zip(expected_size) {
                assert!((actual - expected).abs() < 1e-6, "mode={mode}");
            }
            let expected_parent = if billboard {
                VFX_IDENTITY_BASIS
            } else {
                [[0.0, 0.0, -2.0], [0.0, 3.0, 0.0], [4.0, 0.0, 0.0]]
            };
            for (actual, expected) in quads[0]
                .parent_basis
                .into_iter()
                .flatten()
                .zip(expected_parent.into_iter().flatten())
            {
                assert!((actual - expected).abs() < 1e-6, "mode={mode}");
            }
            for (actual, expected) in quads[0].orientation.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-6, "mode={mode}");
            }

            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert_eq!(meshes.len(), 1);
            assert_eq!(meshes[0].rotation_direction_base, mode);
            assert_eq!(meshes[0].orientation, quads[0].orientation);
            assert_eq!(meshes[0].parent_basis, quads[0].parent_basis);
            assert_eq!(meshes[0].position, quads[0].position);
            assert_eq!(meshes[0].scale[0], quads[0].size[0] * 2.0);
            assert_eq!(meshes[0].scale[1], quads[0].size[1] * 2.0);
        }
    }

    #[test]
    fn nested_emitters_preserve_shear_for_spawn_points_and_particle_transforms() {
        let a = std::f32::consts::FRAC_1_SQRT_2;
        for sx in [2.0, -2.0, 0.0] {
            let mut file = fixture_file();
            let mut child = file.emitters[0].clone();
            child.life.enabled = false;
            child.create_count = linear_curve(1.0);
            child.create_interval = linear_curve(1000.0);
            child.particle_items[0].create_count = 1;
            child.particle_items[0].parent_influence_coord = 3;
            child.position = axis3(Some(1.0), Some(2.0), None);
            child.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_4));
            child.scale = axis3(Some(0.5), Some(2.0), Some(1.0));
            child.emitter_type = Some(EmitterType::Model);
            child.data = Some(AvfxEmitterData::Model(crate::avfx::ModelEmitterData {
                model_index: 0,
                generate_method: 3,
                ..Default::default()
            }));
            file.emitters.push(child);
            let root = &mut file.emitters[0];
            root.life.enabled = false;
            root.create_count = linear_curve(1.0);
            root.create_interval = linear_curve(1000.0);
            root.position = axis3(Some(1.0), Some(2.0), Some(3.0));
            root.rotation = axis3(None, None, Some(std::f32::consts::FRAC_PI_2));
            root.scale = axis3(Some(sx), Some(3.0), Some(4.0));
            root.data = None;
            let mut item = root.particle_items.remove(0);
            item.target_index = 1;
            item.create_count = 1;
            item.parent_influence_coord = 3;
            root.emitter_items.push(item);
            file.models.push(crate::avfx::VfxModelGeometry {
                emit_vertex_numbers: vec![0],
                emit_vertices: vec![crate::avfx::VfxEmitVertex {
                    position: [1.0, 1.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    color: [255; 4],
                }],
                draw: Some(crate::avfx::VfxDrawModel::default()),
                ..Default::default()
            });
            file.particles[0].rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
            file.particles[0].position = axis3(Some(0.5), Some(-0.25), Some(0.75));
            let mut quads = Vec::new();
            VfxRuntime::new(&file).sample(0.0, &mut quads);
            assert_eq!(quads.len(), 1);
            // T(1,2,3) Rz(90) S(sx,3,4) T(1,2,0) Rz(45) S(.5,2,1)
            // applied to the emission point (1,1,0) plus the particle offset.
            let expected_position = if sx == 0.0 {
                // Singular birth inverse uses identity linear: M * M * emission
                // offset, followed by M * particle offset and parent translation.
                [0.625 + 0.75 * a, 2.0, 6.0]
            } else {
                [-5.0 - 6.75 * a, 2.0 + sx - 0.75 * a * sx, 6.0]
            };
            let expected_parent = [
                [-1.5 * a, 0.5 * sx * a, 0.0],
                [-6.0 * a, -2.0 * sx * a, 0.0],
                [0.0, 0.0, 4.0],
            ];
            for (actual, expected) in quads[0].position.into_iter().zip(expected_position).chain(
                quads[0]
                    .parent_basis
                    .into_iter()
                    .flatten()
                    .zip(expected_parent.into_iter().flatten()),
            ) {
                assert!(
                    (actual - expected).abs() < 1e-5,
                    "sx={sx}: {actual} != {expected}"
                );
            }
            file.particles[0].particle_type = Some(ParticleType::LightModel);
            file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
            let mut meshes = Vec::new();
            VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
            assert_eq!(meshes.len(), 1);
            assert_eq!(meshes[0].position, quads[0].position);
            assert_eq!(meshes[0].parent_basis, quads[0].parent_basis);
        }
    }

    #[test]
    fn optional_inheritance_parent_links_stop_independently_and_snapshot_initial() {
        let mut file = powder_emission_fixture();
        let mut child = file.emitters[0].clone();
        child.rotation = axis3(Some(0.2), Some(-0.3), Some(0.4));
        child.scale = axis3(Some(3.0), Some(2.0), Some(1.0));
        child.particle_items[0].parent_influence_coord = 1;
        child.particle_items[0].influence_coord_scale = true;
        child.particle_items[0].influence_coord_rot = true;
        file.emitters.push(child);
        let root = &mut file.emitters[0];
        root.particle_items.clear();
        root.rotation = axis3(Some(0.7), Some(-0.6), Some(0.9));
        let curve = root.rotation.z.as_mut().unwrap();
        curve.keys.push(AvfxCurveKey {
            time: 10,
            z: 1.9,
            ..curve.keys[0]
        });
        root.scale = axis3(Some(2.0), Some(-3.0), Some(0.0));
        let curve = root.scale.x.as_mut().unwrap();
        curve.keys.push(AvfxCurveKey {
            time: 10,
            z: 4.0,
            ..curve.keys[0]
        });
        root.emitter_items = vec![AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_count: 1,
            create_probability: 100,
            influence_coord_rot: true,
            influence_coord_scale: false,
            ..Default::default()
        }];
        file.models[0].draw = Some(Default::default());
        let particle = &mut file.particles[0];
        particle.simple_anim_enable = false;
        particle.particle_type = Some(ParticleType::LightModel);
        particle.data = AvfxParticleData::LightModel { model_index: 0 };
        particle.rotation_direction_base = crate::avfx::rotation_direction_base::NONE;
        particle.loop_end = 4;
        for mode in [0, 1, 2, 3] {
            file.emitters[0].emitter_items[0].parent_influence_coord = mode;
            let runtime = VfxRuntime::new(&file);
            for frame in [10.0, 2.0, 0.0] {
                let mut meshes = Vec::new();
                runtime.sample_mesh(frame / AVFX_FPS, &mut meshes);
                assert_eq!(meshes.len(), 1);
                let time = if mode == 3 { 0.0 } else { frame };
                let scale = if matches!(mode, 2 | 3) {
                    [6.0 + 0.6 * time, -6.0, 0.0]
                } else {
                    [3.0, 2.0, 1.0]
                };
                let euler = if mode == 0 {
                    [0.2, -0.3, 0.4]
                } else {
                    [0.9, -0.9, 1.3 + 0.1 * time]
                };
                let mesh = &meshes[0];
                assert_coordinate(mesh.scale, scale);
                assert_eq!(mesh.parent_basis, VFX_IDENTITY_BASIS);
                for axis in VFX_IDENTITY_BASIS {
                    assert_coordinate(
                        quat_rotate(mesh.orientation, axis),
                        quat_rotate(quat_from_euler(0, euler), axis),
                    );
                }
            }
        }
    }

    #[test]
    fn immortal_periodic_particles_keep_all_creation_events() {
        let mut file = fixture_file();
        file.particles[0].life = AvfxLife::default(); // 永生
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(2.0, &mut quads);
        // Five CrTm=0 events at 0/15/30/45/60, each using CrC=2.
        assert_eq!(quads.len(), 10);
        let mut repeated = Vec::new();
        runtime.sample(2.0, &mut repeated);
        assert_eq!(quads, repeated);
    }

    #[test]
    fn cone_zero_spread_birth_distance_and_motion_stay_on_positive_z() {
        let file = fixture_file();
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(0.9, &mut quads);
        assert!(!quads.is_empty());
        for (index, quad) in quads.into_iter().enumerate() {
            let age = if index < 2 { 27.0 } else { 12.0 };
            assert_eq!(quad.position[0], 0.0);
            assert_eq!(quad.position[1], 0.5);
            assert!((0.1 - 1e-5..=0.5 + 1e-5).contains(&(quad.position[2] - 0.03 * age)));
        }
    }

    #[test]
    fn repeated_timeline_references_start_separate_instances() {
        let mut file = fixture_file();
        file.schedulers[0].items.push(AvfxSchedulerItem {
            enabled: true,
            start_time: 15,
            timeline_index: 0,
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.75, &mut quads);
        assert_eq!(quads.len(), 6); // Ages 22.5 and 7.5: two events plus one.
    }

    #[test]
    fn animated_interval_does_not_rewrite_particle_or_child_births() {
        for (initial, later, expected) in [
            (30.0, 10.0, vec![0.0, 30.0, 40.0, 50.0, 60.0]),
            (10.0, 30.0, vec![0.0, 10.0, 20.0, 30.0, 60.0]),
        ] {
            let mut file = fixture_file();
            file.emitters[0].life.enabled = false;
            file.emitters[0].create_count = linear_curve(1.0);
            file.emitters[0].particle_items[0].create_count = 1;
            file.emitters[0].create_interval = AvfxCurve {
                keys: vec![
                    AvfxCurveKey {
                        time: 0,
                        interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                        z: initial,
                        x: 0.0,
                        y: 0.0,
                    },
                    AvfxCurveKey {
                        time: 30,
                        z: later,
                        interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                        x: 0.0,
                        y: 0.0,
                    },
                ],
                ..Default::default()
            };
            file.particles[0].life.value = 120.0;
            for children in [false, true] {
                let mut fixture = file.clone();
                if children {
                    let mut child = fixture.emitters[0].clone();
                    child.create_interval = linear_curve(1000.0);
                    child.life.enabled = true;
                    child.life.value = 1000.0;
                    fixture.emitters.push(child);
                    let mut item = fixture.emitters[0].particle_items.remove(0);
                    item.target_index = 1;
                    fixture.emitters[0].emitter_items.push(item);
                }
                let runtime = VfxRuntime::new(&fixture);
                for frame in [29.0, 30.0, 31.0, 60.0, 30.0] {
                    let mut births = Vec::new();
                    runtime.sample_ambient(frame, &mut |ctx, _, _| births.push(frame - ctx.age));
                    assert_eq!(
                        births,
                        expected
                            .iter()
                            .copied()
                            .filter(|birth| *birth <= frame)
                            .collect::<Vec<_>>(),
                        "children={children}, frame={frame}, interval={initial}->{later}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_intervals_terminate_without_erasing_prior_events() {
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut emitter = fixture_file().emitters.remove(0);
            emitter.create_interval = linear_curve(invalid);
            assert_eq!(
                emitter_events(&emitter, 0.0, 60.0).collect::<Vec<_>>(),
                vec![(0, 0.0)]
            );
            emitter.create_interval.keys[0].z = 30.0;
            emitter.create_interval.keys[0].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
            emitter.create_interval.keys.push(AvfxCurveKey {
                time: 30,
                z: invalid,
                ..emitter.create_interval.keys[0]
            });
            assert_eq!(
                emitter_events(&emitter, 0.0, 60.0)
                    .map(|(_, age)| age)
                    .collect::<Vec<_>>(),
                vec![0.0, 30.0]
            );
        }
    }

    #[test]
    fn random_intervals_are_latched_per_event_and_window_stable() {
        let mut emitter = fixture_file().emitters.remove(0);
        emitter.create_interval = linear_curve(10.0);
        emitter.create_interval_random = linear_curve(2.0);
        emitter.create_interval_random.random_type = 3;
        let clock = InstanceClock::root(&emitter, -1.0);
        let full: Vec<_> = clock_emitter_events(&emitter, clock, 0.0, 60.0, 0x1234).collect();
        assert!(full.len() > 4);
        assert!(full.windows(2).all(|events| events[1].1 > events[0].1));
        assert!(
            full.windows(2)
                .any(|events| (events[1].1 - events[0].1 - 10.0).abs() > 1e-4)
        );
        let window: Vec<_> = clock_emitter_events(&emitter, clock, 25.0, 60.0, 0x1234)
            .filter(|(_, birth)| *birth >= 25.0)
            .collect();
        assert_eq!(
            window,
            full.iter()
                .copied()
                .filter(|(_, birth)| *birth >= 25.0)
                .collect::<Vec<_>>()
        );
        let other_seed: Vec<_> = clock_emitter_events(&emitter, clock, 0.0, 60.0, 0x5678).collect();
        assert_ne!(other_seed, full);
    }

    #[test]
    fn first_random_interval_keeps_one_coefficient_across_events() {
        let mut emitter = fixture_file().emitters.remove(0);
        emitter.create_interval = linear_curve(10.0);
        emitter.create_interval_random = linear_curve(2.0);
        emitter.create_interval_random.random_type = 1;
        let events: Vec<_> = emitter_events(&emitter, 0.0, 80.0).take(6).collect();
        let intervals = events
            .windows(2)
            .map(|pair| pair[1].1 - pair[0].1)
            .collect::<Vec<_>>();
        assert_eq!(intervals.len(), 5);
        assert!(
            intervals
                .iter()
                .all(|interval| (*interval - intervals[0]).abs() < 1.0e-4),
            "intervals={intervals:?}"
        );
    }

    #[test]
    fn always_random_intervals_keep_long_window_event_history() {
        let mut emitter = fixture_file().emitters.remove(0);
        emitter.create_interval = linear_curve(10.0);
        emitter.create_interval_random = linear_curve(3.0);
        emitter.create_interval_random.random_type = 3;
        let clock = InstanceClock::root(&emitter, -1.0);
        let full: Vec<_> = clock_emitter_events(&emitter, clock, 0.0, 180.0, 0x1234).collect();
        assert!(full.len() > 12);
        for oldest in [40.0, 90.0, 130.0] {
            let window: Vec<_> =
                clock_emitter_events(&emitter, clock, oldest, 180.0, 0x1234).collect();
            let expected = full
                .iter()
                .copied()
                .skip_while(|(_, birth)| *birth < oldest)
                .collect::<Vec<_>>();
            assert_eq!(
                window
                    .into_iter()
                    .filter(|(_, birth)| *birth >= oldest)
                    .collect::<Vec<_>>(),
                expected,
                "oldest={oldest}"
            );
        }
    }

    #[test]
    fn first_random_interval_const_tail_keeps_window_phase() {
        let mut emitter = fixture_file().emitters.remove(0);
        emitter.create_interval = linear_curve(10.0);
        emitter.create_interval_random = linear_curve(1.0);
        emitter.create_interval_random.random_type = 1;
        emitter.create_interval_random.keys.push(AvfxCurveKey {
            time: 30,
            z: 3.0,
            ..emitter.create_interval_random.keys[0]
        });
        let clock = InstanceClock::root(&emitter, -1.0);
        let full: Vec<_> = clock_emitter_events(&emitter, clock, 0.0, 300.0, 0x1234).collect();
        let oldest = 180.0;
        let window: Vec<_> = clock_emitter_events(&emitter, clock, oldest, 300.0, 0x1234)
            .filter(|(_, birth)| *birth >= oldest)
            .collect();
        assert_eq!(
            window,
            full.into_iter()
                .filter(|(_, birth)| *birth >= oldest)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn constant_interval_windows_match_history_enumeration() {
        for interval in [0.1, 0.25, 1.0, 15.0, 31.3] {
            let mut emitter = fixture_file().emitters.remove(0);
            emitter.create_interval = linear_curve(interval);
            // The fast path also covers redundant keys at the same value.
            emitter.create_interval.keys.push(AvfxCurveKey {
                time: 30,
                ..emitter.create_interval.keys[0]
            });
            for oldest in [0.0, 40.75, 90.0] {
                let window: Vec<_> = emitter_events(&emitter, oldest, 100.0)
                    .filter(|(_, birth)| *birth >= oldest)
                    .collect();
                let history: Vec<_> = emitter_events(&emitter, 0.0, 100.0)
                    .filter(|(_, birth)| *birth >= oldest)
                    .collect();
                assert_eq!(window, history, "interval={interval}, oldest={oldest}");
            }
        }
    }

    #[test]
    fn constant_interval_seeks_directly_to_live_events_after_a_day() {
        let mut file = fixture_file();
        file.emitters[0].life.enabled = false;
        let frame = AVFX_FPS * 60.0 * 60.0 * 24.0;
        let events: Vec<_> = emitter_events(&file.emitters[0], frame - 30.0, frame).collect();
        assert_eq!(
            events,
            vec![
                (172_798, frame - 30.0),
                (172_799, frame - 15.0),
                (172_800, frame)
            ]
        );
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        runtime.sample(frame / AVFX_FPS, &mut quads);
        assert_eq!(quads.len(), 6);
        let expected = quads.clone();
        runtime.sample(0.8, &mut quads);
        runtime.sample(frame / AVFX_FPS, &mut quads);
        assert_eq!(quads, expected);
    }

    #[test]
    fn animated_interval_with_constant_tail_seeks_to_live_events_after_a_day() {
        for interpolation in [
            AvfxCurveKey::INTERPOLATION_STEP,
            AvfxCurveKey::INTERPOLATION_LINEAR,
            AvfxCurveKey::INTERPOLATION_SPLINE,
        ] {
            let mut emitter = fixture_file().emitters.remove(0);
            emitter.create_interval = linear_curve(2.5);
            emitter.create_interval.keys[0].interpolation = interpolation;
            emitter.create_interval.keys[0].x = 0.3;
            emitter.create_interval.keys[0].y = 1.2;
            emitter.create_interval.keys.push(AvfxCurveKey {
                time: 30,
                z: 0.1,
                ..emitter.create_interval.keys[0]
            });
            let until = AVFX_FPS * 86_400.0;
            let oldest = until - 30.0;
            let (_, first) = emitter_events(&emitter, oldest, until).next().unwrap();
            assert!(
                (oldest - 0.5..=oldest).contains(&first),
                "interpolation={interpolation}, first={first}, oldest={oldest}"
            );
            for oldest in [0.0, 20.0, 30.0, 100.0, 1_000.0] {
                let until = oldest + 30.0;
                let actual: Vec<_> = emitter_events(&emitter, oldest, until)
                    .filter(|(_, birth)| *birth >= oldest)
                    .collect();
                let expected: Vec<_> = emitter_events(&emitter, 0.0, until)
                    .filter(|(_, birth)| *birth >= oldest)
                    .collect();
                assert_eq!(
                    actual, expected,
                    "interpolation={interpolation}, oldest={oldest}"
                );
            }
        }
    }

    #[test]
    fn constant_event_seek_matches_rounded_additions_at_exponent_boundaries() {
        for exponent in [-140, -30, -5, 0, 10, 40, 52, 53, 80, 120] {
            let boundary = 2.0_f64.powi(exponent);
            for start in [
                boundary.next_down(),
                boundary,
                boundary.next_up(),
                boundary + (boundary.next_up() - boundary) * 3.0,
            ] {
                for interval in [
                    f32::from_bits(1),
                    0.01,
                    0.1,
                    1.0,
                    1.5,
                    2.5,
                    31.3,
                    (boundary * f64::EPSILON * 1.5) as f32,
                ] {
                    if interval == 0.0 {
                        continue;
                    }
                    let mut target = start;
                    for _ in 0..257 {
                        target += f64::from(interval);
                    }
                    for oldest in [target.next_down(), target, target.next_up()] {
                        let mut expected = (123, start);
                        loop {
                            let next = expected.1 + f64::from(interval);
                            if next <= expected.1 || next > oldest {
                                break;
                            }
                            expected = (expected.0 + 1, next);
                            assert!(expected.0 < 1_000);
                        }
                        assert_eq!(
                            seek_constant_event((123, start), interval, oldest),
                            expected,
                            "start={start:?}, interval={interval:?}, oldest={oldest:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(
            seek_constant_event((u64::MAX - 3, 0.0), 1.0, 100.0),
            (u64::MAX, 3.0)
        );
        // The first addition establishes even parity; subsequent increments
        // round to 2, so interval * event_number would produce the wrong ID.
        let start = 2.0_f64.powi(52) + 1.0;
        assert_eq!(
            seek_constant_event((0, start), 1.5, start + 1_000_000_001.0),
            (500_000_001, start + 1_000_000_001.0)
        );
    }

    #[test]
    fn constant_tail_seek_preserves_terminal_events_and_repeat_behavior() {
        let mut emitter = fixture_file().emitters.remove(0);
        for post_behavior in [crate::avfx::BEHAVIOR_CONST, crate::avfx::BEHAVIOR_REPEAT] {
            for last_value in [0.0, -1.0, f32::NAN, f32::INFINITY, 7.3] {
                emitter.create_interval = AvfxCurve {
                    post_behavior,
                    keys: vec![
                        AvfxCurveKey {
                            time: 5,
                            z: 4.0,
                            interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                            x: 0.0,
                            y: 0.0,
                        },
                        AvfxCurveKey {
                            time: 30,
                            z: last_value,
                            interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                            x: 0.0,
                            y: 0.0,
                        },
                    ],
                    ..Default::default()
                };
                for oldest in [29.0, 30.0, 31.0, 32.0, 100.0] {
                    let until = oldest + 100.0;
                    let actual: Vec<_> = emitter_events(&emitter, oldest, until)
                        .filter(|(_, birth)| *birth >= oldest)
                        .collect();
                    let expected: Vec<_> = emitter_events(&emitter, 0.0, until)
                        .filter(|(_, birth)| *birth >= oldest)
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "behavior={post_behavior}, last={last_value}, oldest={oldest}"
                    );
                }
            }
        }
    }

    #[test]
    fn child_limit_does_not_allocate_all_historical_emitter_loops() {
        let mut file = fixture_file();
        let mut child = file.emitters[0].clone();
        child.life.value = 5.0;
        child.create_count = linear_curve(1.0);
        child.particle_items[0].create_count = 1;
        file.emitters.push(child);
        let parent = &mut file.emitters[0];
        parent.life.value = 30.0;
        parent.child_limit = 2;
        let mut item = parent.particle_items.remove(0);
        item.target_index = 1;
        item.create_count = 1;
        parent.emitter_items.push(item);
        file.particles[0].life.value = -1.0;
        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        // Collecting the parent loop range here would require > 4 GB, despite
        // the child cap admitting only its first two creation events.
        runtime.sample(2.0_f32.powi(30), &mut quads);
        assert_eq!(quads.len(), 2);
        let expected = quads.clone();
        runtime.sample(0.8, &mut quads);
        runtime.sample(2.0_f32.powi(30), &mut quads);
        assert_eq!(quads, expected);
        for probability in [0, -1] {
            file.emitters[0].emitter_items[0].create_probability = probability;
            VfxRuntime::new(&file).sample(2.0_f32.powi(30), &mut quads);
            assert!(quads.is_empty());
        }
    }

    #[test]
    fn animated_interval_tail_preserves_complete_instances_and_random_lifetimes() {
        for interpolation in [
            AvfxCurveKey::INTERPOLATION_LINEAR,
            AvfxCurveKey::INTERPOLATION_SPLINE,
        ] {
            let mut file = random_position_fixture();
            let emitter = &mut file.emitters[0];
            emitter.life.enabled = false;
            emitter.create_interval = linear_curve(3.0);
            emitter.create_interval.keys[0].interpolation = interpolation;
            emitter.create_interval.keys[0].x = 1.2;
            emitter.create_interval.keys[0].y = 0.3;
            emitter.create_interval.keys.push(AvfxCurveKey {
                time: 31,
                z: 0.3,
                interpolation,
                x: 0.0,
                y: 0.0,
                ..emitter.create_interval.keys[0]
            });
            emitter.particle_items[0].create_probability = 63;
            file.particles[0].life.value = 5.0;
            file.particles[0].life.value_random = 2.0;
            let particle = AvfxParticle {
                particle_type: Some(ParticleType::LightModel),
                data: AvfxParticleData::LightModel { model_index: 0 },
                ..file.particles[0].clone()
            };
            file.particles.push(particle);
            let mesh_item = AvfxEmitterItem {
                target_index: 1,
                ..file.emitters[0].particle_items[0].clone()
            };
            file.emitters[0].particle_items.push(mesh_item);
            file.models.push(crate::avfx::VfxModelGeometry {
                draw: Some(crate::avfx::VfxDrawModel::default()),
                ..Default::default()
            });
            let mut history = file.clone();
            // Keep the same animated prefix and constant values at every tested
            // event, but postpone the tail so the reference enumerates history.
            let last = *history.emitters[0].create_interval.keys.last().unwrap();
            history.emitters[0].create_interval.keys.push(AvfxCurveKey {
                time: 16_000,
                z: 10.0,
                interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                ..last
            });
            let runtime = VfxRuntime::new(&file);
            let reference = VfxRuntime::new(&history);
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            let mut actual_meshes = Vec::new();
            let mut expected_meshes = Vec::new();
            let mut saw_quad = false;
            let mut saw_mesh = false;
            for time in [0.0, 0.8, 1.0, 1.1, 3.7, 100.0, 500.0, 3.7] {
                runtime.sample(time, &mut actual);
                reference.sample(time, &mut expected);
                assert_eq!(
                    actual, expected,
                    "interpolation={interpolation}, time={time}"
                );
                runtime.sample_mesh(time, &mut actual_meshes);
                reference.sample_mesh(time, &mut expected_meshes);
                assert_eq!(
                    actual_meshes, expected_meshes,
                    "interpolation={interpolation}, time={time}"
                );
                saw_quad |= !actual.is_empty();
                saw_mesh |= !actual_meshes.is_empty();
            }
            assert!(saw_quad && saw_mesh);
        }
    }

    #[test]
    fn event_fast_forward_preserves_random_lifetimes_and_spawn_identity() {
        let mut file = random_position_fixture();
        file.emitters[0].life.enabled = false;
        file.emitters[0].particle_items[0].create_probability = 63;
        file.particles[0].life.value = 30.0;
        file.particles[0].life.value_random = 12.0;
        let mut history = file.clone();
        // A future change prevents fast-forward but leaves all sampled intervals
        // identical, providing the full-history reference for complete instances.
        history.emitters[0].create_interval.keys[0].interpolation =
            AvfxCurveKey::INTERPOLATION_STEP;
        let first_key = history.emitters[0].create_interval.keys[0];
        history.emitters[0].create_interval.keys.push(AvfxCurveKey {
            time: 16_000,
            z: 100.0,
            ..first_key
        });
        let runtime = VfxRuntime::new(&file);
        let reference = VfxRuntime::new(&history);
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        for time in [0.0, 0.8, 3.7, 10.0, 100.0, 3.7] {
            runtime.sample(time, &mut actual);
            reference.sample(time, &mut expected);
            assert_eq!(actual, expected, "time={time}");
        }
    }

    #[test]
    fn event_fast_forward_keeps_lifetime_boundaries_with_scaled_child_clock() {
        let mut file = random_position_fixture();
        file.emitters[0].life.value = 10_000.0;
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].particle_items[0].create_count = 1;
        file.particles[0].life.value = 2000.0;
        // The child's slow clock keeps the reference's full replay bounded,
        // while its real birth times exercise large f32 lifetime boundaries.
        file.emitters.insert(
            0,
            AvfxEmitter {
                create_count: linear_curve(1.0),
                create_interval: linear_curve(10_000_000.0),
                emitter_items: vec![AvfxEmitterItem {
                    enabled: true,
                    target_index: 1,
                    create_time: 1,
                    create_count: 1,
                    create_probability: 100,
                    override_life: true,
                    override_life_value: 10_000_000,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        for interval in [0.01, 0.03, 0.1, 0.25, 1.0] {
            file.emitters[1].create_interval = linear_curve(interval);
            let mut history = file.clone();
            history.emitters[1].create_interval.keys[0].interpolation =
                AvfxCurveKey::INTERPOLATION_STEP;
            let first_key = history.emitters[1].create_interval.keys[0];
            history.emitters[1].create_interval.keys.push(AvfxCurveKey {
                time: 16_000,
                z: 100.0,
                ..first_key
            });
            let runtime = VfxRuntime::new(&file);
            let reference = VfxRuntime::new(&history);
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            for time in [86_400.0, 86_400.04, 86_400.125] {
                runtime.sample(time, &mut actual);
                reference.sample(time, &mut expected);
                assert!(!expected.is_empty());
                assert!(
                    actual == expected,
                    "interval={interval}, time={time}: {} actual, {} expected",
                    actual.len(),
                    expected.len()
                );
            }
        }
    }

    fn random_position_fixture() -> AvfxFile {
        let mut file = fixture_file();
        file.emitters[0].create_count = linear_curve(1.0);
        file.emitters[0].particle_items[0].create_count = 1;
        file.emitters[0].data = None;
        file.emitters[0].position = AvfxCurve3Axis::default();
        file.emitters[0].position.random_x = Some(linear_curve(1.0));
        file.particles[0].position.random_y = Some(linear_curve(1.0));
        file
    }

    #[test]
    fn repeated_instance_paths_have_independent_random_positions() {
        for level in [
            "scheduler",
            "scheduler_item",
            "timeline_item",
            "particle_item",
            "child_item",
            "child_copy",
        ] {
            let mut file = random_position_fixture();
            match level {
                "scheduler" => file.schedulers.push(file.schedulers[0].clone()),
                "scheduler_item" => {
                    let item = file.schedulers[0].items[0];
                    file.schedulers[0].items.push(item);
                }
                "timeline_item" => {
                    let item = file.timelines[0].items[0];
                    file.timelines[0].items.push(item);
                }
                "particle_item" => {
                    let item = file.emitters[0].particle_items[0];
                    file.emitters[0].particle_items.push(item);
                }
                _ => {
                    let child = file.emitters[0].clone();
                    file.emitters.push(child);
                    let mut item = file.emitters[0].particle_items.remove(0);
                    item.target_index = 1;
                    if level == "child_copy" {
                        item.create_time = 1;
                        item.create_count = 2;
                    }
                    file.emitters[0].emitter_items.push(item);
                    if level == "child_item" {
                        file.emitters[0].emitter_items.push(item);
                    }
                }
            }
            let runtime = VfxRuntime::new(&file);
            let mut quads = Vec::new();
            runtime.sample(0.0, &mut quads);
            assert_eq!(quads.len(), 2, "{level}");
            assert_ne!(quads[0].position[1], quads[1].position[1], "{level}");
            if level != "particle_item" {
                assert_ne!(quads[0].position[0], quads[1].position[0], "{level}");
            }
            let mut repeated = Vec::new();
            runtime.sample(1.0, &mut repeated);
            runtime.sample(0.0, &mut repeated);
            assert_eq!(
                quads, repeated,
                "random access must be deterministic: {level}"
            );
        }
    }

    #[test]
    fn timeline_loops_get_distinct_random_instances() {
        let mut file = random_position_fixture();
        file.timelines[0].loop_end = 30;
        let runtime = VfxRuntime::new(&file);
        let mut first = Vec::new();
        let mut second = Vec::new();
        runtime.sample(0.0, &mut first);
        runtime.sample(1.0, &mut second);
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].position[0], second[0].position[0]);
        assert_ne!(first[0].position[1], second[0].position[1]);
    }

    #[test]
    fn finite_direct_timeline_loops_keep_particles_from_previous_roots() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.timelines[0].items[0].end_time = 15;
        file.emitters[0].effector_index = -1;
        file.emitters[0].create_interval = linear_curve(100.0);
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.particles[0].collision_type = -1;

        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        for (frame, count) in [(0.0, 2), (10.0, 4), (20.0, 6), (30.0, 8), (40.0, 8)] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }

        file.timelines[0].binder_index = 255;
        file.timelines[0].items[0].binder_index = 255;
        VfxRuntime::new(&file).sample(20.0 / 30.0, &mut quads);
        assert_eq!(quads.len(), 6);
        file.timelines[0].items[0].emitter_index = 256;
        file.timelines[0].items[0].effector_index = 254;
        file.timelines[0].items[0].clip_index = 253;
        VfxRuntime::new(&file).sample(20.0 / 30.0, &mut quads);
        assert_eq!(quads.len(), 6);
        file.timelines[0].items[0].emitter_index = 0;
        file.timelines[0].items[0].effector_index = -1;
        file.timelines[0].items[0].clip_index = -1;
        file.timelines[0].binder_index = -1;
        file.timelines[0].items[0].binder_index = -1;

        runtime.sample(20.0 / 30.0, &mut quads);
        let previous_roots = quads.clone();
        file.timelines[0].loop_end = 0;
        let mut first_root = Vec::new();
        VfxRuntime::new(&file).sample(20.0 / 30.0, &mut first_root);
        assert_eq!(&previous_roots[..first_root.len()], first_root);
        runtime.sample(20.0 / 30.0, &mut quads);
        assert_eq!(quads, previous_roots);

        file.timelines[0].loop_end = 10;
        let automatic_item = file.timelines[0].items[0];
        file.timelines[0].items.push(AvfxTimelineItem {
            start_time: 15,
            end_time: 30,
            ..automatic_item
        });
        VfxRuntime::new(&file).sample(20.0 / 30.0, &mut quads);
        assert_eq!(quads, previous_roots);
    }

    #[test]
    fn finite_timeline_replay_does_not_cross_clip_only_items() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.timelines[0].items[0].end_time = 15;
        file.emitters[0].effector_index = -1;
        file.emitters[0].create_interval = linear_curve(100.0);
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.particles[0].collision_type = -1;
        let automatic_item = file.timelines[0].items[0];
        file.timelines[0].items.push(AvfxTimelineItem {
            emitter_index: -1,
            clip_index: 0,
            ..automatic_item
        });

        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(10.0 / 30.0, &mut quads);
        assert_eq!(quads.len(), 2);
    }

    #[test]
    fn finite_timeline_replay_ignores_effector_only_items() {
        let mut baseline = fixture_file();
        baseline.timelines[0].loop_end = 10;
        baseline.timelines[0].items[0].end_time = 15;
        baseline.emitters[0].effector_index = -1;
        baseline.emitters[0].create_interval = linear_curve(100.0);
        baseline.emitters[0].particle_items[0].parameter_link = -1;
        baseline.particles[0].collision_type = -1;

        let mut with_effector = baseline.clone();
        with_effector.effectors.push(AvfxEffector::default());
        with_effector.timelines[0].items.push(AvfxTimelineItem {
            emitter_index: 0,
            effector_index: 0,
            ..baseline.timelines[0].items[0]
        });
        let mut expected = Vec::new();
        let mut actual = Vec::new();
        for frame in [0.0, 10.0, 20.0] {
            VfxRuntime::new(&baseline).sample(frame / 30.0, &mut expected);
            VfxRuntime::new(&with_effector).sample(frame / 30.0, &mut actual);
            assert_eq!(actual, expected, "at frame {frame}");
        }
    }

    #[test]
    fn initialization_only_timeline_loop_has_finite_tails_with_unlimited_root() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.emitters[0].effector_index = -1;
        file.emitters[0].particle_items[0].create_time = 1;
        file.emitters[0].particle_items[0].create_count = 2;
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.particles[0].collision_type = -1;

        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        for (frame, count) in [(0.0, 2), (10.0, 4), (20.0, 6), (30.0, 8), (40.0, 8)] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }
    }

    #[test]
    fn finite_child_emitter_timeline_loop_retains_grandchild_tails() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.particles[0].collision_type = -1;

        let mut child = file.emitters[0].clone();
        child.emitter_type = Some(EmitterType::Point);
        child.data = None;
        child.effector_index = -1;
        child.life.value = 15.0;
        child.create_interval = linear_curve(100.0);
        child.particle_items[0].parameter_link = -1;
        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.emitters[0].data = None;
        file.emitters[0].effector_index = -1;
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        });
        file.emitters.push(child);

        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        for (frame, count) in [(0.0, 2), (10.0, 4), (20.0, 6), (30.0, 8), (40.0, 8)] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }

        runtime.sample(20.0 / 30.0, &mut quads);
        let previous_roots = quads.clone();
        file.timelines[0].loop_end = 0;
        let mut first_root = Vec::new();
        VfxRuntime::new(&file).sample(20.0 / 30.0, &mut first_root);
        assert_eq!(&previous_roots[..first_root.len()], first_root);
    }

    #[test]
    fn finite_repeat_segment_timeline_loop_retains_previous_roots() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_start = 30;
        file.timelines[0].loop_end = 60;
        file.timelines[0].items[0].start_time = 35;
        file.timelines[0].items[0].end_time = 50;
        file.emitters[0].effector_index = -1;
        file.emitters[0].create_interval = linear_curve(100.0);
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.particles[0].collision_type = -1;
        file.particles[0].life.value = 40.0;

        let runtime = VfxRuntime::new(&file);
        let mut quads = Vec::new();
        for (frame, count) in [
            (34.0, 0),
            (35.0, 2),
            (59.0, 2),
            (60.0, 2),
            (64.0, 2),
            (65.0, 4),
            (75.0, 4),
            (76.0, 2),
            (95.0, 4),
        ] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }
        runtime.sample(65.0 / 30.0, &mut quads);
        let retained = quads.clone();
        let mut single = file.clone();
        single.timelines[0].loop_start = 0;
        single.timelines[0].loop_end = 0;
        let mut first_root = Vec::new();
        VfxRuntime::new(&single).sample(65.0 / 30.0, &mut first_root);
        assert_eq!(&retained[..first_root.len()], first_root);
        runtime.sample(95.0 / 30.0, &mut quads);
        runtime.sample(65.0 / 30.0, &mut quads);
        assert_eq!(quads, retained);

        let item = file.timelines[0].items[0];
        file.timelines[0].items.push(AvfxTimelineItem {
            start_time: 10,
            emitter_index: -1,
            ..item
        });
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_some()
        );

        file.timelines[0].items[0].start_time = 10;
        file.timelines[0].items[0].end_time = 25;
        file.particles[0].life.value = 100.0;
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
        VfxRuntime::new(&file).sample(60.0 / 30.0, &mut quads);
        assert_eq!(quads.len(), 2, "intro items retain the single-cycle path");
    }

    #[test]
    fn finite_multilevel_timeline_loop_retains_tails_after_emitter_deaths() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.timelines[0].items[0].end_time = 12;
        file.particles[0].life.value = 20.0;
        file.particles[0].collision_type = -1;

        let mut grandchild = file.emitters[0].clone();
        grandchild.emitter_type = Some(EmitterType::Point);
        grandchild.data = None;
        grandchild.life.value = 10.0;
        grandchild.effector_index = -1;
        grandchild.particle_items[0].create_time = 2;
        grandchild.particle_items[0].create_count = 2;
        grandchild.particle_items[0].parameter_link = -1;

        let mut child = grandchild.clone();
        child.particle_items.clear();
        child.emitter_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: 2,
            create_time: 2,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        });

        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.emitters[0].data = None;
        file.emitters[0].effector_index = -1;
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        });
        file.emitters.extend([child, grandchild]);

        let runtime = VfxRuntime::new(&file);
        assert!(
            runtime
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_some()
        );
        let mut quads = Vec::new();
        for (frame, count) in [(19.0, 0), (21.0, 2), (31.0, 4), (40.0, 4), (41.0, 4)] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }
    }

    #[test]
    fn initialization_only_infinite_child_has_finite_timeline_tail() {
        let mut file = fixture_file();
        file.timelines[0].binder_index = -1;
        file.timelines[0].loop_end = 10;
        file.particles[0].collision_type = -1;

        let mut child = file.emitters[0].clone();
        child.emitter_type = Some(EmitterType::Point);
        child.data = None;
        child.effector_index = -1;
        child.life.enabled = false;
        child.particle_items[0].create_time = 1;
        child.particle_items[0].create_count = 2;
        child.particle_items[0].parameter_link = -1;
        file.emitters[0].emitter_type = Some(EmitterType::Point);
        file.emitters[0].data = None;
        file.emitters[0].effector_index = -1;
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items.push(AvfxEmitterItem {
            enabled: true,
            target_index: 1,
            create_time: 1,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        });
        file.emitters.push(child);

        let runtime = VfxRuntime::new(&file);
        assert!(
            runtime
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_some()
        );
        let mut quads = Vec::new();
        for (frame, count) in [(0.0, 2), (10.0, 4), (20.0, 6), (30.0, 8), (40.0, 8)] {
            runtime.sample(frame / 30.0, &mut quads);
            assert_eq!(quads.len(), count, "at frame {frame}");
        }

        file.emitters[1].particle_items[0].create_time = 0;
        file.emitters[1].create_interval = linear_curve(10.0);
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
        file.emitters[1].particle_items[0].create_time = 1;
        file.emitters[1].effector_index = 0;
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
        file.emitters[1].effector_index = -1;
        file.particles[0].collision_type = 0;
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
        file.particles[0].collision_type = -1;
        file.emitters[1].particle_items[0].parameter_link = 0;
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
        file.emitters[1].particle_items[0].parameter_link = -1;
        file.particles[0].life.enabled = false;
        assert!(
            VfxRuntime::new(&file)
                .finite_timeline_tail_bound(&file.timelines[0])
                .is_none()
        );
    }

    #[test]
    fn loops_preserve_the_intro_before_loop_start() {
        let mut file = fixture_file();
        file.timelines[0].loop_start = 30;
        file.timelines[0].loop_end = 60;
        file.timelines[0].items[0].start_time = 30;
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(quads.is_empty());
        VfxRuntime::new(&file).sample(2.0, &mut quads);
        assert_eq!(quads.len(), 2);
        file.particles[0].loop_start = 30;
        file.particles[0].loop_end = 60;
        for (age, expected) in [
            (0.0, 0.0),
            (29.0, 29.0),
            (59.0, 59.0),
            (60.0, 30.0),
            (91.0, 31.0),
        ] {
            assert_eq!(VfxRuntime::particle_age(&file.particles[0], age), expected);
        }
    }

    #[test]
    fn missing_binder_does_not_select_binder_zero() {
        let mut file = fixture_file();
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            ..Default::default()
        });
        let runtime = VfxRuntime::with_bind_points(
            &file,
            &[crate::avfx::VfxBindPoint {
                parent_bone: None,
                id: 3,
                translate: [1.0, 2.0, 3.0],
                rotate: [0.0; 3],
            }],
        );
        assert_eq!(runtime.bind_offset(-1), [0.0; 3]);
        assert_eq!(runtime.bind_offset(0), [1.0, 2.0, 3.0]);
        assert_eq!(runtime.bind_offset(1), [0.0; 3]);
    }

    #[test]
    fn binder_without_follow_orientation_keeps_offsets_in_unrotated_target_axes() {
        use crate::avfx::{AvfxBinder, AvfxBinderData, AvfxBinderProperties, VfxBindPoint};
        let mut file = fixture_file();
        file.timelines[0].items[0].binder_index = 0;
        file.binders.push(AvfxBinder {
            bind_point_id: 3,
            life: -1,
            transform_scale: 1,
            properties_start: Some(AvfxBinderProperties {
                bind_target_point_type: 3,
                bind_point_id: 3,
                coord_update_frame: -1,
                position: axis3(Some(2.0), Some(-3.0), Some(4.0)),
                ..Default::default()
            }),
            data: Some(AvfxBinderData {
                spring_strength: Some(linear_curve(1.0)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let points = [VfxBindPoint {
            parent_bone: None,
            id: 3,
            translate: [10.0, 20.0, 30.0],
            rotate: [0.7, -0.4, 1.2],
        }];
        let runtime = VfxRuntime::with_bind_points(&file, &points);
        let base = runtime.bind_base(0);
        assert_eq!(base.linear, VFX_IDENTITY_BASIS);
        assert_eq!(base.position, [12.0, 17.0, 34.0]);
        assert_eq!(base.transform_point([1.0, 2.0, 3.0]), [13.0, 19.0, 37.0]);

        let mut unrotated = points.clone();
        unrotated[0].rotate = [0.0; 3];
        let mut actual = Vec::new();
        let mut reference = Vec::new();
        runtime.sample(0.0, &mut actual);
        VfxRuntime::with_bind_points(&file, &unrotated).sample(0.0, &mut reference);
        assert!(!actual.is_empty());
        assert_eq!(actual, reference);
    }

    #[test]
    fn following_binder_uses_client_normalization_in_sampled_parent_basis() {
        let mut file = fixture_file();
        file.timelines[0].items[0].binder_index = 0;
        file.emitters[0].particle_items[0].parent_influence_coord = 2;
        file.emitters[0].effector_index = -1;
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.particles[0].collision_type = -1;
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            life: -1,
            transform_scale: 1,
            following_target_orientation: true,
            properties_start: Some(crate::avfx::AvfxBinderProperties {
                bind_point_id: 3,
                bind_target_point_type: 3,
                coord_update_frame: -1,
                ..Default::default()
            }),
            data: Some(crate::avfx::AvfxBinderData {
                spring_strength: Some(linear_curve(1.0)),
                ..Default::default()
            }),
            ..Default::default()
        });
        file.emitters[0].position = Default::default();
        file.emitters[0].rotation = Default::default();
        file.emitters[0].scale = Default::default();
        file.particles[0].rotation_direction_base = 0;
        let runtime = VfxRuntime::with_bind_points(
            &file,
            &[crate::avfx::VfxBindPoint {
                parent_bone: None,
                id: 3,
                translate: [0.0; 3],
                rotate: [0.0; 3],
            }],
        );
        let inverse = 1.0 / (1.0 + f32::EPSILON);
        let expected = [
            [inverse, 0.0, 0.0],
            [0.0, inverse, 0.0],
            [0.0, 0.0, inverse],
        ];
        assert_eq!(runtime.bind_base(0).linear, expected);
        let mut quads = Vec::new();
        runtime.sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(quads.iter().all(|q| q.parent_basis == expected));
        // Other Binder channels have not been implemented; do not silently
        // admit this continuous fallback into staged playback.
        assert_eq!(
            VfxPlayback::new(runtime.clone()).fallback_reason(),
            Some("staged playback: unsupported Item Binder")
        );
        file.binders[0].following_target_orientation = false;
        assert_eq!(
            VfxPlayback::new(VfxRuntime::with_bind_points(&file, &runtime.bind_points))
                .fallback_reason(),
            None
        );
    }

    #[test]
    fn point_factory_direction_is_zero_independently_of_target_orientation() {
        let mut file = fixture_file();
        file.binders.push(crate::avfx::AvfxBinder {
            bind_point_id: 3,
            ..Default::default()
        });
        let runtime = VfxRuntime::with_bind_points(
            &file,
            &[crate::avfx::VfxBindPoint {
                parent_bone: None,
                id: 3,
                translate: [0.0; 3],
                rotate: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            }],
        );
        let direction = runtime.root_direction(0);
        assert_eq!(direction, [0.0; 3]);
        assert_eq!(runtime.bind_base(0).linear, VFX_IDENTITY_BASIS);
        file.binders[0].following_target_orientation = true;
        let runtime = VfxRuntime::with_bind_points(&file, &runtime.bind_points);
        let transformed = runtime.bind_base(0).transform_point([0.0, 0.0, 1.0]);
        assert_eq!(runtime.root_direction(0), [0.0; 3]);
        assert!((transformed[1] + 1.0).abs() < 1e-5);
        assert!(transformed[0].abs() < 1e-5);
        assert!(transformed[2].abs() < 1e-5);
    }

    #[test]
    fn static_linear_without_follow_orientation_blends_only_target_translations() {
        use crate::avfx::{AvfxBinder, AvfxBinderData, AvfxBinderProperties, VfxBindPoint};

        let point = |id| AvfxBinderProperties {
            bind_target_point_type: 3,
            binder_name: "null".into(),
            bind_point_id: id,
            coord_update_frame: -1,
            ..Default::default()
        };
        let mut file = fixture_file();
        file.binders.push(AvfxBinder {
            binder_type: 1,
            bind_point_id: 3,
            life: -1,
            transform_scale: 255,
            properties_start: Some(point(3)),
            properties_goal: Some(point(4)),
            data: Some(AvfxBinderData {
                carry_over_factor: Some(linear_curve(0.5)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let points = [
            VfxBindPoint {
                parent_bone: None,
                id: 3,
                translate: [2.0, 4.0, 6.0],
                rotate: [0.0; 3],
            },
            VfxBindPoint {
                parent_bone: None,
                id: 4,
                translate: [6.0, 8.0, 10.0],
                rotate: [0.0, 0.0, std::f32::consts::FRAC_PI_2],
            },
        ];
        let runtime = VfxRuntime::with_bind_points(&file, &points);
        let base = runtime.bind_base(0);
        assert_eq!(base.position, [4.0, 6.0, 8.0]);
        assert_eq!(runtime.root_direction(0), [4.0, 4.0, 4.0]);
        assert_eq!(base.linear, VFX_IDENTITY_BASIS);

        file.binders[0]
            .data
            .as_mut()
            .unwrap()
            .carry_over_factor
            .as_mut()
            .unwrap()
            .keys[0]
            .z = 0.0;
        assert_eq!(
            VfxRuntime::with_bind_points(&file, &points).root_direction(0),
            [4.0, 4.0, 4.0]
        );

        file.binders[0]
            .data
            .as_mut()
            .unwrap()
            .carry_over_factor_random = Some(linear_curve(0.1));
        assert!(static_linear_binder_factor(&file.binders[0]).is_none());
        assert_eq!(
            VfxRuntime::with_bind_points(&file, &points).root_direction(0),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            VfxRuntime::with_bind_points(&file, &points)
                .bind_base(0)
                .position,
            points[0].translate
        );
    }

    #[test]
    fn local_linear_preview_consumes_native_direction_scale_and_depth_without_admitting_staged() {
        use crate::avfx::{AvfxBinder, AvfxBinderData, AvfxBinderProperties, VfxBindPoint};
        let point = |id| AvfxBinderProperties {
            bind_target_point_type: 3,
            binder_name: "null".into(),
            bind_point_id: id,
            coord_update_frame: -1,
            ..Default::default()
        };
        let mut file = fixture_file();
        file.binders.push(AvfxBinder {
            binder_type: 1,
            bind_point_id: 3,
            life: -1,
            transform_scale: 255,
            start_to_global_direction: true,
            vfx_scale_enabled: true,
            vfx_scale_bias: 0.5,
            vfx_scale_depth_offset: true,
            vfx_scale_interpolation: true,
            transform_scale_depth_offset: true,
            transform_scale_interpolation: true,
            properties_start: Some(point(3)),
            properties_goal: Some(point(4)),
            data: Some(AvfxBinderData {
                carry_over_factor: Some(linear_curve(1.25)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let points = [
            VfxBindPoint {
                id: 3,
                parent_bone: None,
                translate: [0.0, 0.0, 1.0],
                rotate: [0.0; 3],
            },
            VfxBindPoint {
                id: 4,
                parent_bone: None,
                translate: [0.0, 0.0, -1.0],
                rotate: [0.0; 3],
            },
        ];
        let runtime = VfxRuntime::with_bind_points(&file, &points)
            .with_document_scale([2.0, 3.0, 4.0])
            .unwrap()
            .with_vfx_scale(2.0)
            .unwrap()
            .with_camera_position([0.0, 0.0, 3.0])
            .unwrap();
        let base = runtime.bind_base(0);
        assert_eq!(base.position, [0.0, 0.0, -1.5]);
        assert_eq!(
            base.linear,
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]
        );
        assert_eq!(base.orientation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(base.scale, [3.0, 4.5, 6.0]);
        assert_eq!(
            base.auxiliary_matrix,
            VfxBinderMatrix::scale_matrix([3.0, 4.5, 6.0])
        );
        assert_eq!(base.depth_offset_multiplier, 2.0);
        assert_eq!(runtime.root_direction(0), [0.0, 0.0, -2.0]);
        assert!(local_linear_binder_factor(&file.binders[0]).is_some());
        assert!(static_linear_binder_factor(&file.binders[0]).is_none());

        file.global.ags_enabled = true;
        file.global.revised_rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        file.global.revised_position = [10.0, 20.0, 30.0];
        let runtime = VfxRuntime::with_bind_points(&file, &points)
            .with_document_scale([2.0, 3.0, 4.0])
            .unwrap()
            .with_vfx_scale(2.0)
            .unwrap()
            .with_camera_position([0.0, 0.0, 3.0])
            .unwrap();
        let (state, initialized) = runtime
            .fixed_linear_binder_initialization(&file.binders[0])
            .unwrap();
        assert_eq!(
            initialized.queries,
            [Some(VfxBinderQueryStatus::Refreshed); 2]
        );
        assert_eq!(
            initialized.update,
            Some([VfxBinderQueryStatus::Refreshed; 2])
        );
        assert_eq!(initialized.child_direction, Some([0.0, 0.0, -2.0]));
        let revised = runtime.bind_base(0);
        assert_eq!(revised.position, base.position);
        assert_eq!(revised.linear, base.linear);
        assert_eq!(revised.scale, [3.0, 4.5, 6.0]);
        assert_eq!(revised.auxiliary_matrix, state.auxiliary_matrix);
        // Root revision consumes raw Document scale, without query/VFX bias.
        let literal = [[0.0, -2.0, 0.0], [3.0, 0.0, 0.0], [0.0, 0.0, 4.0]];
        for (actual, expected) in revised
            .auxiliary_matrix
            .basis
            .iter()
            .flatten()
            .zip(literal.iter().flatten())
        {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert_eq!(revised.auxiliary_matrix.position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn static_linear_root_direction_uses_start_axis_for_near_coincident_targets() {
        use crate::avfx::{AvfxBinder, AvfxBinderData, AvfxBinderProperties, VfxBindPoint};

        let point = |id| AvfxBinderProperties {
            bind_target_point_type: 3,
            binder_name: "null".into(),
            bind_point_id: id,
            coord_update_frame: -1,
            ..Default::default()
        };
        let mut file = fixture_file();
        file.binders.push(AvfxBinder {
            binder_type: 1,
            bind_point_id: 3,
            life: -1,
            transform_scale: 1,
            properties_start: Some(point(3)),
            properties_goal: Some(point(4)),
            data: Some(AvfxBinderData {
                carry_over_factor: Some(linear_curve(0.5)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut points = [
            VfxBindPoint {
                parent_bone: None,
                id: 3,
                translate: [0.0; 3],
                rotate: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            },
            VfxBindPoint {
                parent_bone: None,
                id: 4,
                translate: [0.009, 0.0, 0.0],
                rotate: [0.0; 3],
            },
        ];
        let direction = VfxRuntime::with_bind_points(&file, &points).root_direction(0);
        assert!(direction[0].abs() < 1e-6);
        assert_eq!(direction, [0.0, 0.0, 1.0]);

        points[1].translate[0] = 0.01;
        assert_eq!(
            VfxRuntime::with_bind_points(&file, &points).root_direction(0),
            [0.01, 0.0, 0.0]
        );

        file.emitters[0].data = None;
        file.timelines[0].items[0].binder_index = 0;
        let mut quads = Vec::new();
        VfxRuntime::with_bind_points(&file, &points).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        assert!(
            quads
                .iter()
                .all(|quad| quad.movement_direction == [0.01, 0.0, 0.0])
        );
    }

    #[test]
    fn uv_random_axes_follow_their_own_connection() {
        let mut curve = crate::avfx::AvfxCurve2Axis {
            axis_connect_random: 1,
            random_x: Some(linear_curve(1.0)),
            ..Default::default()
        };
        let values = eval2_seeded(&curve, 0.0, 0.0, 19);
        assert_ne!(values[0], 0.0);
        assert_eq!(values[0], values[1]);
        curve.axis_connect_random = 2;
        curve.random_y = curve.random_x.take();
        let values = eval2_seeded(&curve, 0.0, 0.0, 19);
        assert_ne!(values[1], 0.0);
        assert_eq!(values[0], values[1]);
    }

    #[test]
    fn distortion_keeps_the_selected_uv_rotation() {
        let mut particle = quad_particle();
        particle.uv_sets[0].rotation = linear_curve(0.75);
        particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            uv_set_index: 0,
            ..Default::default()
        });
        assert_eq!(ResolvedTexture::of(&particle, 0.0, 0).uvd_rotation, 0.75);
    }

    #[test]
    fn texture_uv_high_bit_aliases_use_client_low_three_bits() {
        let mut particle = quad_particle();
        particle.uv_sets.push(AvfxUvSet {
            calculate_uv: 1,
            scroll: crate::avfx::AvfxCurve2Axis {
                x: Some(linear_curve(0.25)),
                ..Default::default()
            },
            scale: crate::avfx::AvfxCurve2Axis {
                x: Some(linear_curve(2.0)),
                ..Default::default()
            },
            rotation: linear_curve(0.75),
            ..Default::default()
        });
        particle.texture_color1.as_mut().unwrap().uv_set_index = 9;
        particle.texture_color2 = Some(AvfxParticleTexture {
            enabled: true,
            texture_index: 0,
            uv_set_index: 9,
            ..Default::default()
        });
        particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            uv_set_index: 9,
            target_uv: [false, true, false, false],
            ..Default::default()
        });
        particle.texture_normal = Some(crate::avfx::AvfxParticleTextureNormal {
            enabled: true,
            texture_index: 0,
            uv_set_index: 9,
            ..Default::default()
        });

        let texture = ResolvedTexture::of(&particle, 0.0, 0);
        assert_eq!(texture.texture_uv_sets[..2], [1, 1]);
        assert_eq!(texture.uv_origins[..2], [[0.25, 0.0]; 2]);
        assert_eq!(texture.uv_scales[..2], [[2.0, 1.0]; 2]);
        assert_eq!(texture.uv_rotations[..2], [0.75; 2]);
        assert_eq!(texture.uv_by_pixel_position[..2], [true; 2]);
        assert_eq!(texture.distortion_uv_set, 1);
        assert_eq!(texture.distortion_targets, 0b11);
        assert_eq!(texture.uvd_origin, [0.25, 0.0]);
        assert_eq!(texture.normal_uv_set, 1);
        assert_eq!(texture.normal_uv_origin, [0.25, 0.0]);
    }

    #[test]
    fn pixel_position_uv_mode_follows_each_referenced_set() {
        let mut particle = quad_particle();
        particle.uv_sets.push(AvfxUvSet {
            calculate_uv: 1,
            ..Default::default()
        });
        particle.texture_color2 = particle.texture_color1.clone();
        particle.texture_color2.as_mut().unwrap().uv_set_index = 1;
        particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            uv_set_index: 1,
            ..Default::default()
        });

        let texture = ResolvedTexture::of(&particle, 0.0, 0);
        assert_eq!(texture.uv_by_pixel_position, [false, true, false, false]);
        assert!(texture.uvd_by_pixel_position);

        particle.texture_color2.as_mut().unwrap().uv_set_index = 99;
        particle.texture_distortion.as_mut().unwrap().uv_set_index = -1;
        let texture = ResolvedTexture::of(&particle, 0.0, 0);
        assert_eq!(texture.uv_by_pixel_position, [false; 4]);
        assert!(!texture.uvd_by_pixel_position);
    }

    #[test]
    fn normal_texture_keeps_its_uv_sampler_and_power_curve() {
        let mut particle = quad_particle();
        particle.uv_sets.push(AvfxUvSet {
            calculate_uv: 1,
            scroll: crate::avfx::AvfxCurve2Axis {
                x: Some(linear_curve(0.25)),
                y: Some(linear_curve(-0.5)),
                ..Default::default()
            },
            scale: crate::avfx::AvfxCurve2Axis {
                x: Some(linear_curve(2.0)),
                y: Some(linear_curve(3.0)),
                ..Default::default()
            },
            rotation: linear_curve(0.75),
            ..Default::default()
        });
        particle.texture_normal = Some(crate::avfx::AvfxParticleTextureNormal {
            enabled: true,
            uv_set_index: 1,
            texture_filter: 4,
            texture_border_u: 2,
            texture_border_v: 1,
            texture_index: 7,
            power: linear_curve(3.5),
        });

        let texture = ResolvedTexture::of(&particle, 0.0, 19);
        assert_eq!(texture.texture_normal_index, 7);
        assert_eq!(texture.normal_uv_set, 1);
        assert_eq!(texture.normal_uv_origin, [0.25, -0.5]);
        assert_eq!(texture.normal_uv_scale, [2.0, 3.0]);
        assert_eq!(texture.normal_uv_rotation, 0.75);
        assert!(texture.normal_uv_by_pixel_position);
        assert_eq!(texture.normal_texture_borders, [2, 1]);
        assert_eq!(texture.normal_texture_filter, 4);
        assert_eq!(texture.normal_power, 3.5);

        particle.texture_normal.as_mut().unwrap().enabled = false;
        let texture = ResolvedTexture::of(&particle, 0.0, 19);
        assert_eq!(texture.texture_normal_index, -1);
        assert_eq!(texture.normal_power, 0.0);
    }

    #[test]
    fn reflection_texture_evaluates_rate_power_and_screen_source() {
        let mut particle = quad_particle();
        particle.texture_reflection = Some(crate::avfx::AvfxParticleTextureReflection {
            enabled: true,
            use_screen_copy: true,
            texture_filter: 4,
            calculate_color: 1,
            texture_index: 23,
            rate: linear_curve(0.75),
            power: linear_curve(2.0),
        });
        let texture = ResolvedTexture::of(&particle, 0.0, 0);
        assert!(texture.reflection_enabled);
        assert!(texture.reflection_use_screen_copy);
        assert_eq!(texture.reflection_texture_index, 23);
        assert_eq!(texture.reflection_texture_filter, 4);
        assert_eq!(texture.reflection_calculate_color, 1);
        assert_eq!(texture.reflection_rate, 0.75);
        assert_eq!(texture.reflection_power, 2.0);

        particle.texture_reflection.as_mut().unwrap().enabled = false;
        let texture = ResolvedTexture::of(&particle, 0.0, 0);
        assert!(!texture.reflection_enabled);
        assert_eq!(texture.reflection_texture_index, -1);
        assert_eq!(texture.reflection_texture_filter, 1);
        assert_eq!(texture.reflection_rate, 0.0);
        assert_eq!(texture.reflection_power, 0.0);
    }

    #[test]
    fn uv_sets_have_independent_random_curves_but_shared_references_match() {
        let mut particle = quad_particle();
        let set = &mut particle.uv_sets[0];
        set.scroll.random_x = Some(linear_curve(1.0));
        set.scale.random_y = Some(linear_curve(0.5));
        set.rotation_random = linear_curve(1.0);
        particle.uv_sets.push(particle.uv_sets[0].clone());
        particle.texture_color2 = particle.texture_color1.clone();
        particle.texture_color2.as_mut().unwrap().uv_set_index = 1;
        particle.texture_color3 = particle.texture_color1.clone();
        particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            uv_set_index: 1,
            ..Default::default()
        });

        let texture = ResolvedTexture::of(&particle, 3.0, 19);
        assert_ne!(texture.uv_origins[0], texture.uv_origins[1]);
        assert_ne!(texture.uv_scales[0], texture.uv_scales[1]);
        assert_ne!(texture.uv_rotations[0], texture.uv_rotations[1]);
        assert_eq!(texture.uv_origins[0], texture.uv_origins[2]);
        assert_eq!(texture.uv_scales[0], texture.uv_scales[2]);
        assert_eq!(texture.uv_rotations[0], texture.uv_rotations[2]);
        assert_eq!(texture.uvd_origin, texture.uv_origins[1]);
        assert_eq!(texture.uvd_scale, texture.uv_scales[1]);
        assert_eq!(texture.uvd_rotation, texture.uv_rotations[1]);

        let again = ResolvedTexture::of(&particle, 3.0, 19);
        assert_eq!(texture.uv_origins, again.uv_origins);
        assert_eq!(texture.uv_scales, again.uv_scales);
        assert_eq!(texture.uv_rotations, again.uv_rotations);
    }

    #[test]
    fn invalid_uv_set_does_not_borrow_set_zero_animation() {
        let mut particle = quad_particle();
        particle.uv_sets[0].scroll.x = Some(linear_curve(0.5));
        particle.uv_sets[0].scale.y = Some(linear_curve(2.0));
        particle.uv_sets[0].rotation = linear_curve(0.75);
        for index in [-1, 1, 4, i32::MAX] {
            particle.texture_color1.as_mut().unwrap().uv_set_index = index;
            particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
                enabled: true,
                texture_index: 0,
                uv_set_index: index,
                ..Default::default()
            });
            let texture = ResolvedTexture::of(&particle, 0.0, 0);
            assert_eq!(texture.texture_uv_sets[0], index & 7);
            assert_eq!(texture.distortion_uv_set, index & 7);
            assert_eq!(texture.uv_origins[0], [0.0; 2]);
            assert_eq!(texture.uv_scales[0], [1.0; 2]);
            assert_eq!(texture.uv_rotations[0], 0.0);
            assert_eq!(texture.uvd_origin, [0.0; 2]);
            assert_eq!(texture.uvd_scale, [1.0; 2]);
            assert_eq!(texture.uvd_rotation, 0.0);
        }
    }

    #[test]
    fn quad_distortion_uses_client_byte_conversion_while_meshes_keep_floats() {
        for kind in [
            ParticleType::Quad,
            ParticleType::Model,
            ParticleType::LightModel,
        ] {
            for (raw, byte) in [
                (0.0, 0),
                (0.25, 63),
                (0.5, 127),
                (1.0, 255),
                (2.0, 254),
                (-0.5, 129),
                (-1.0, 1),
                (f32::MAX, 0),
                (f32::INFINITY, 0),
                (f32::NEG_INFINITY, 0),
            ] {
                let particle = AvfxParticle {
                    particle_type: Some(kind),
                    texture_distortion: Some(crate::avfx::AvfxParticleDistortion {
                        enabled: true,
                        texture_index: 0,
                        power: linear_curve(raw),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let actual = ResolvedTexture::of(&particle, 0.0, 0).distortion_power;
                assert_eq!(
                    actual,
                    if kind == ParticleType::Quad {
                        byte as f32 / 255.0
                    } else {
                        raw
                    },
                    "{kind:?}: {raw}"
                );
            }
        }
    }

    #[test]
    fn texture_palette_uses_seeded_offset_and_client_byte_conversion() {
        let ages = CurveAges {
            local: 3.0,
            total: 13.0,
        };
        let seed = 19;
        let mut random = linear_curve(0.5);
        random.random_type = 4;
        let mut particle = quad_particle();
        particle.texture_palette = Some(crate::avfx::AvfxParticleTexturePalette {
            enabled: true,
            texture_filter: 0,
            texture_border: 2,
            texture_index: 7,
            offset: linear_curve(0.25),
            offset_random: random.clone(),
        });
        let texture = ResolvedTexture::at_times(&particle, ages, ages, seed);
        let expected = quantize_palette_offset(
            0.25 + curve_random_value_at(&random, ages, seed ^ 0x5041_4C45),
        );
        assert_eq!(texture.texture_palette_index, 7);
        assert_eq!(texture.palette_offset, expected);
        assert_eq!(texture.palette_border, 2);
        assert_eq!(texture.palette_filter, 0);

        particle.texture_palette.as_mut().unwrap().enabled = false;
        let texture = ResolvedTexture::at_times(&particle, ages, ages, seed);
        assert_eq!(texture.texture_palette_index, -1);
        assert_eq!(texture.palette_offset, 0.0);
    }

    #[test]
    fn palette_offset_matches_client_low_byte_conversion() {
        for (raw, byte) in [
            (0.0, 0),
            (0.25, 63),
            (0.5, 127),
            (1.0, 255),
            (2.0, 254),
            (-0.5, 129),
            (-1.0, 1),
            (f32::NAN, 0),
            (f32::INFINITY, 0),
            (f32::MAX, 0),
        ] {
            assert_eq!(quantize_palette_offset(raw), byte as f32 / 255.0, "{raw}");
        }
    }

    #[test]
    fn texture_filters_survive_particle_sampling() {
        let mut file = fixture_file();
        let particle = &mut file.particles[0];
        particle.texture_color1.as_mut().unwrap().use_screen_copy = true;
        particle.texture_color1.as_mut().unwrap().texture_filter = 0;
        particle.texture_color2 = particle.texture_color1.clone();
        particle.texture_color2.as_mut().unwrap().texture_filter = 1;
        particle.texture_color3 = particle.texture_color1.clone();
        particle.texture_color3.as_mut().unwrap().texture_filter = 3;
        particle.texture_color4 = particle.texture_color1.clone();
        particle.texture_color4.as_mut().unwrap().texture_filter = 4;
        particle.texture_distortion = Some(crate::avfx::AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            texture_filter: 0,
            ..Default::default()
        });
        let mut quads = Vec::new();
        VfxRuntime::new(&file).sample(0.0, &mut quads);
        assert!(!quads.is_empty());
        for quad in quads {
            assert_eq!(quad.texture_filters, [0, 1, 3, 4]);
            assert_eq!(quad.distortion_filter, 0);
            assert!(quad.texture1_use_screen_copy);
        }
        file.models.push(crate::avfx::VfxModelGeometry {
            draw: Some(crate::avfx::VfxDrawModel::default()),
            ..Default::default()
        });
        file.particles[0].particle_type = Some(ParticleType::LightModel);
        file.particles[0].data = AvfxParticleData::LightModel { model_index: 0 };
        let mut meshes = Vec::new();
        VfxRuntime::new(&file).sample_mesh(0.0, &mut meshes);
        assert!(!meshes.is_empty());
        assert!(meshes.iter().all(|mesh| mesh.texture1_use_screen_copy));
    }

    #[test]
    fn color_flipbook_handles_full_i16_frame_range() {
        let colors = [[0; 4], [255; 4], [0; 4], [255; 4]];
        assert_eq!(
            sample_flipbook(&colors, &[i16::MAX; 4], i16::MAX as f32 + 0.5),
            [1.0; 4]
        );
        assert_eq!(
            sample_flipbook(&colors, &[i16::MAX; 4], i16::MAX as f32 + 2.0),
            [1.0; 4]
        );
        assert_eq!(
            sample_flipbook(&colors, &[i16::MIN, i16::MAX, i16::MAX, i16::MAX], -0.5),
            [0.5; 4]
        );
        assert_eq!(sample_flipbook(&colors, &[0, 10, 20, 30], 5.0), [0.5; 4]);
        assert_eq!(sample_flipbook(&colors, &[0, 10, 10, 20], 10.0), [0.0; 4]);
        assert_eq!(sample_flipbook(&colors, &[0, 10, 10, 20], 15.0), [0.5; 4]);
        assert_eq!(sample_flipbook(&colors, &[0; 4], 0.0), [0.0; 4]);
        assert_eq!(sample_flipbook(&colors, &[0; 4], 0.5), [1.0; 4]);
    }

    #[test]
    fn scalar_random_constant_key_count_does_not_change_value() {
        for mode in 0..16 {
            let single = AvfxCurve {
                random_type: mode,
                ..linear_curve(1.0)
            };
            let mut multiple = single.clone();
            multiple.keys.push(AvfxCurveKey {
                time: 10,
                ..single.keys[0]
            });
            for seed in 0..16 {
                for age in [0.0, 0.25, 5.0, 10.0, 1000.0] {
                    assert_eq!(
                        curve_random_value(&single, age, seed),
                        curve_random_value(&multiple, age, seed),
                        "mode={mode}, seed={seed}, age={age}"
                    );
                }
            }
        }
    }

    #[test]
    fn scalar_random_first_offsets_use_integer_percentages() {
        for mode in 0..3 {
            let curve = AvfxCurve {
                random_type: mode,
                ..linear_curve(1.0)
            };
            for seed in 0..256 {
                let value = curve_random_value(&curve, 2.5, seed);
                let percent = value * 100.0;
                assert!(
                    (percent - percent.round()).abs() < 1e-5,
                    "{mode}: {percent}"
                );
                let range = match mode {
                    1 => 0.0..=1.0,
                    2 => -1.0..=0.0,
                    _ => -1.0..=1.0,
                };
                assert!(range.contains(&value));
            }
        }
    }

    #[test]
    fn scalar_random_unknown_modes_have_no_offset() {
        for mode in [6, 7, 14, 15] {
            let curve = AvfxCurve {
                random_type: mode,
                ..linear_curve(2.0)
            };
            for age in [0.0, 3.5, 100.0] {
                assert_eq!(curve_random_value(&curve, age, 4), 0.0);
            }
        }
    }

    #[test]
    fn seeded_scalar_pair_keeps_main_zero_sign_when_random_is_absent_or_disabled() {
        let ages = CurveAges {
            local: 2.0,
            total: 2.0,
        };
        let main = linear_curve(-0.0);
        for random in [
            AvfxCurve::default(),
            AvfxCurve {
                random_type: 7,
                ..linear_curve(f32::NAN)
            },
        ] {
            assert_eq!(
                curve_value_seeded_at(&main, &random, ages, 0.0, 4).to_bits(),
                0x8000_0000
            );
        }
    }

    #[test]
    fn random_curve_preserves_repeat_and_step() {
        let mut curve = linear_curve(1.0);
        curve.keys[0].interpolation = crate::avfx::AvfxCurveKey::INTERPOLATION_STEP;
        curve.keys.push(crate::avfx::AvfxCurveKey {
            time: 10,
            z: 4.0,
            ..curve.keys[0]
        });
        curve.post_behavior = crate::avfx::BEHAVIOR_REPEAT;
        assert_eq!(
            curve_random_value(&curve, 0.0, 4),
            curve_random_value(&curve, 5.0, 4)
        );
        assert_eq!(
            curve_random_value(&curve, 5.0, 4),
            curve_random_value(&curve, 15.0, 4)
        );
        assert_ne!(
            curve_random_value(&curve, 0.0, 4),
            curve_random_value(&curve, 10.0, 4)
        );
    }

    #[test]
    fn random_curve_scales_spline_after_interpolating_amplitude() {
        let mut curve = linear_curve(1.0);
        curve.keys = [(0, 1.0), (2, 4.0), (5, 2.0), (9, 8.0)]
            .map(|(time, z)| crate::avfx::AvfxCurveKey {
                time,
                z,
                x: 0.0,
                y: 0.0,
                interpolation: 0,
            })
            .to_vec();
        curve.keys[1].x = 2.0;
        curve.keys[1].y = 1.0;
        curve.keys[2].x = 1.0;
        for random_type in 0..6 {
            curve.random_type = random_type;
            let unit = AvfxCurve {
                random_type,
                ..linear_curve(1.0)
            };
            let coefficient = curve_random_value(&unit, 3.5, 4);
            // Hermite amplitude at u=0.5: 3 + (-3/5 - 18/7)/8.
            let expected = coefficient * (729.0 / 280.0);
            assert!((curve_random_value(&curve, 3.5, 4) - expected).abs() < 1e-6);
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
