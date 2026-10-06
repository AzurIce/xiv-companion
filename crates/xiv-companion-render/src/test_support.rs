use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use half::f16;

use crate::renderer::{ModelRenderOptions, ModelRenderer};
use crate::{ModelRenderData, PreparedModelOptions};

const GPU_TESTS_DISABLED: &str =
    "native render tests require the explicit test-session switch XIV_ALLOW_GPU_TESTS=1";
static SNAPSHOT_INSTANCE: std::sync::OnceLock<wgpu::Instance> = std::sync::OnceLock::new();

fn check_gpu_test_opt_in(value: Option<&std::ffi::OsStr>) -> Result<(), &'static str> {
    if value == Some(std::ffi::OsStr::new("1")) {
        Ok(())
    } else {
        Err(GPU_TESTS_DISABLED)
    }
}

/// Keep driver initialization opt-in for each test process. The user has
/// authorized native validation during development; this switch avoids accidental
/// driver loading when ordinary tests are run with broad filters.
pub fn require_gpu_test_opt_in() -> Result<(), &'static str> {
    check_gpu_test_opt_in(std::env::var_os("XIV_ALLOW_GPU_TESTS").as_deref())
}

fn snapshot_instance() -> Result<wgpu::Instance, &'static str> {
    snapshot_instance_with_opt_in(std::env::var_os("XIV_ALLOW_GPU_TESTS").as_deref())
}

fn snapshot_instance_with_opt_in(
    value: Option<&std::ffi::OsStr>,
) -> Result<wgpu::Instance, &'static str> {
    check_gpu_test_opt_in(value)?;
    // Reuse the loader only after authorization. Driver enumeration itself can
    // affect the desktop, so the gate must precede Instance::new.
    Ok(SNAPSHOT_INSTANCE
        .get_or_init(|| {
            wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: if software_render_tests_requested() {
                    wgpu::Backends::VULKAN
                } else {
                    wgpu::Backends::PRIMARY
                },
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            })
        })
        .clone())
}

fn software_render_tests_requested() -> bool {
    std::env::var_os("XIV_RENDER_TEST_SOFTWARE_ONLY").as_deref() == Some(std::ffi::OsStr::new("1"))
}

fn validate_software_adapter(
    backend: wgpu::Backend,
    device_type: wgpu::DeviceType,
) -> Result<(), &'static str> {
    if backend == wgpu::Backend::Vulkan && device_type == wgpu::DeviceType::Cpu {
        Ok(())
    } else {
        Err("software render tests require a CPU Vulkan adapter; hardware fallback is forbidden")
    }
}

/// An orbit-camera input to mounted playback before the captured frame.
#[derive(Clone, Copy, Debug)]
pub struct WeaponVfxCameraInput {
    pub time: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub roll: f32,
}

#[derive(Clone, Debug)]
pub struct WeaponModelSnapshotOptions {
    pub name: String,
    pub output_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub prepared_model_options: PreparedModelOptions,
    pub render_options: ModelRenderOptions,
    pub power_preference: wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
    pub capture_hdr_scene: bool,
    /// Render the same prepared scene repeatedly while retaining temporal GPU resources.
    pub render_repetitions: usize,
    /// 固定的 VFX 粒子批次（合成特效/确定性快照用）。
    pub vfx_quads: Vec<xiv_companion_data::VfxQuad>,
    /// 固定的 VFX 网格实例（Model/LightModel 本体路径，确定性快照用）。
    pub vfx_mesh_instances: Vec<xiv_companion_data::VfxMeshInstance>,
    /// 网格粒子引用的绘制模型集合（顺序即实例的 model_index）。
    pub vfx_meshes: Vec<xiv_companion_data::VfxDrawModel>,
    /// VFX 贴图（文件 `Tex` 顺序；None = 解码失败用回退贴图）。
    pub vfx_textures: Vec<Option<crate::renderer::VfxTextureInput>>,
    /// Test-only solid color for an injected VFX reflection cube provider.
    pub vfx_reflection_cube_color: Option<[u8; 4]>,
    /// Test-only solid color for an injected TC1 character portrait provider.
    pub vfx_portrait_color: Option<[u8; 4]>,
    /// 完整武器主/副模型挂载；用于按模型预览偏移采样真实整件 VFX。
    pub weapon_vfx: Option<xiv_companion_data::WeaponVfxAttachments>,
    pub weapon_vfx_time: f32,
    /// Submit local owner poses and input timestamps through the production
    /// attachment API before capturing the final frame.
    pub weapon_vfx_pose_updates: Vec<(f32, String, xiv_companion_data::SkeletonPose)>,
    /// Fail the capture if any mounted runtime uses continuous fallback.
    pub require_staged_weapon_vfx: bool,
    /// First camera supplies birth state; later cameras are explicit inputs.
    pub weapon_vfx_camera_inputs: Vec<WeaponVfxCameraInput>,
    /// Explicit definition selection for isolated Aura bind-group render tests.
    pub aura_definition: Option<(usize, usize)>,
    /// Exercise the preview's independent per-model Aura selection.
    pub all_unambiguous_auras: bool,
}

impl WeaponModelSnapshotOptions {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn with_output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = output_dir.into();
        self
    }

    pub fn with_viewport(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_camera(mut self, yaw: f32, pitch: f32, zoom: f32, pan: [f32; 2]) -> Self {
        self.yaw = yaw;
        self.pitch = pitch;
        self.zoom = zoom;
        self.pan = pan;
        self
    }

    pub fn with_render_options(mut self, render_options: ModelRenderOptions) -> Self {
        self.render_options = render_options;
        self
    }

    pub fn with_prepared_model_options(
        mut self,
        prepared_model_options: PreparedModelOptions,
    ) -> Self {
        self.prepared_model_options = prepared_model_options;
        self
    }

    pub fn with_enabled_shape_mask(mut self, enabled_shape_mask: u32) -> Self {
        self.prepared_model_options.enabled_shape_mask = Some(enabled_shape_mask);
        self
    }

    pub fn with_hdr_scene_capture(mut self) -> Self {
        self.capture_hdr_scene = true;
        self
    }

    pub fn with_render_repetitions(mut self, repetitions: usize) -> Self {
        self.render_repetitions = repetitions.max(1);
        self
    }

    /// 固定 VFX 粒子批次（时间已由调用方采样，保证快照确定性）。
    pub fn with_vfx_quads(
        mut self,
        quads: impl IntoIterator<Item = xiv_companion_data::VfxQuad>,
    ) -> Self {
        self.vfx_quads = quads.into_iter().collect();
        self
    }

    /// 固定 VFX 网格实例（时间已由调用方采样，保证快照确定性）。
    pub fn with_vfx_mesh_instances(
        mut self,
        instances: impl IntoIterator<Item = xiv_companion_data::VfxMeshInstance>,
    ) -> Self {
        self.vfx_mesh_instances = instances.into_iter().collect();
        self
    }

    /// 网格粒子引用的绘制模型集合。
    pub fn with_vfx_meshes(
        mut self,
        meshes: impl IntoIterator<Item = xiv_companion_data::VfxDrawModel>,
    ) -> Self {
        self.vfx_meshes = meshes.into_iter().collect();
        self
    }

    /// VFX 贴图（文件 `Tex` 顺序）。
    pub fn with_vfx_textures(
        mut self,
        textures: impl IntoIterator<Item = Option<crate::renderer::VfxTextureInput>>,
    ) -> Self {
        self.vfx_textures = textures.into_iter().collect();
        self
    }

    pub fn with_vfx_reflection_cube_color(mut self, rgba: [u8; 4]) -> Self {
        self.vfx_reflection_cube_color = Some(rgba);
        self
    }

    pub fn with_vfx_portrait_color(mut self, rgba: [u8; 4]) -> Self {
        self.vfx_portrait_color = Some(rgba);
        self
    }

    /// 使用与 Web 预览相同的多 MDL 挂载运行时采样真实 VFX。
    pub fn with_weapon_vfx(
        mut self,
        vfx: xiv_companion_data::WeaponVfxAttachments,
        time: f32,
    ) -> Self {
        self.weapon_vfx = Some(vfx);
        self.weapon_vfx_time = time;
        self
    }

    pub fn with_weapon_vfx_pose_update(
        mut self,
        time: f32,
        model_path: impl Into<String>,
        pose: xiv_companion_data::SkeletonPose,
    ) -> Self {
        self.weapon_vfx_pose_updates
            .push((time, model_path.into(), pose));
        self
    }

    pub fn with_aura_definition(mut self, attachment_index: usize, particle_index: usize) -> Self {
        self.aura_definition = Some((attachment_index, particle_index));
        self
    }

    pub fn with_all_unambiguous_auras(mut self) -> Self {
        self.all_unambiguous_auras = true;
        self
    }
}

impl Default for WeaponModelSnapshotOptions {
    fn default() -> Self {
        let output_dir = std::env::var_os("XIV_WEAPON_RENDER_SNAPSHOT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("target")
                    .join("weapon-render-snapshots")
            });

        Self {
            name: "weapon-model".to_string(),
            output_dir,
            width: 1280,
            height: 900,
            yaw: 0.65,
            pitch: 0.35,
            zoom: 3.2,
            pan: [0.0, 0.0],
            prepared_model_options: PreparedModelOptions::default(),
            render_options: ModelRenderOptions::default(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            capture_hdr_scene: false,
            render_repetitions: 1,
            vfx_quads: Vec::new(),
            vfx_mesh_instances: Vec::new(),
            vfx_meshes: Vec::new(),
            vfx_textures: Vec::new(),
            vfx_reflection_cube_color: None,
            vfx_portrait_color: None,
            weapon_vfx: None,
            weapon_vfx_time: 0.0,
            weapon_vfx_pose_updates: Vec::new(),
            weapon_vfx_camera_inputs: Vec::new(),
            require_staged_weapon_vfx: false,
            aura_definition: None,
            all_unambiguous_auras: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WeaponModelSnapshot {
    pub png_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub adapter_name: String,
    pub adapter_backend: wgpu::Backend,
    pub hdr_scene_rgba: Option<Vec<[f32; 4]>>,
    /// CPU geometry actually submitted by the mounted production adapter.
    pub weapon_vfx_quads: Vec<xiv_companion_data::VfxQuad>,
}

#[derive(Debug)]
pub enum WeaponModelSnapshotError {
    GpuTestsDisabled,
    InvalidVfxPose(String),
    InvalidViewport {
        width: u32,
        height: u32,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    RequestAdapter(String),
    RequestDevice(String),
    Poll(String),
    Map(String),
    MapCallbackDropped,
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
}

impl fmt::Display for WeaponModelSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GpuTestsDisabled => f.write_str(GPU_TESTS_DISABLED),
            Self::InvalidVfxPose(error) => write!(f, "invalid VFX pose: {error}"),
            Self::InvalidViewport { width, height } => {
                write!(f, "invalid weapon snapshot viewport {width}x{height}")
            }
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} {}: {source}", path.display()),
            Self::RequestAdapter(error) => {
                write!(f, "failed to request native wgpu adapter: {error}")
            }
            Self::RequestDevice(error) => {
                write!(f, "failed to request native wgpu device: {error}")
            }
            Self::Poll(error) => write!(f, "failed to poll native wgpu device: {error}"),
            Self::Map(error) => write!(f, "failed to map weapon snapshot buffer: {error}"),
            Self::MapCallbackDropped => {
                write!(f, "weapon snapshot buffer map callback was dropped")
            }
            Self::Image { path, source } => {
                write!(f, "failed to write PNG {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for WeaponModelSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Image { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type ModelSnapshotOptions = WeaponModelSnapshotOptions;
pub type ModelSnapshot = WeaponModelSnapshot;
pub type ModelSnapshotError = WeaponModelSnapshotError;

/// 多实例场景快照的单条目：模型 + 准备选项 + 可选的关节矩阵覆盖。
/// `joint_matrices` 为 None 时按 `skeleton` + `pose` 计算（rest/世界 ×
/// inverse bind）；Some 时创建实例后直接上传（武器挂接等自定义关节规则，
/// 长度按实例 joint 表截断——调用方应与实例 joint 名表对齐）。
pub struct SceneSnapshotEntry<'a> {
    pub model: &'a (dyn crate::ModelRenderData + 'a),
    pub prepared_options: PreparedModelOptions,
    pub joint_matrices: Option<Vec<[f32; 16]>>,
}

impl<'a> SceneSnapshotEntry<'a> {
    pub fn new(model: &'a (dyn crate::ModelRenderData + 'a)) -> Self {
        Self {
            model,
            prepared_options: PreparedModelOptions::default(),
            joint_matrices: None,
        }
    }

    pub fn with_prepared_options(mut self, prepared_options: PreparedModelOptions) -> Self {
        self.prepared_options = prepared_options;
        self
    }

    pub fn with_joint_matrices(mut self, joint_matrices: Vec<[f32; 16]>) -> Self {
        self.joint_matrices = Some(joint_matrices);
        self
    }
}

/// 多实例场景快照（`render_scene` 路径）：各条目一个 GPU 实例（骨架蒙皮同
/// 单模型路径），逐条上传关节矩阵后共享相机/光照/后处理一帧渲染。
/// `pose` 为 None 时各实例保持创建时的 rest pose 关节矩阵。
pub fn render_scene_snapshot(
    options: ModelSnapshotOptions,
    entries: &[SceneSnapshotEntry],
    skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    pose: Option<&xiv_companion_data::SkeletonPose>,
) -> Result<ModelSnapshot, ModelSnapshotError> {
    require_gpu_test_opt_in().map_err(|_| WeaponModelSnapshotError::GpuTestsDisabled)?;
    let _render_guard = SNAPSHOT_RENDER_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pollster::block_on(render_scene_snapshot_async(
        options, entries, skeleton, pose,
    ))
}

/// libvulkan 的 ICD 扫描/加载在多个线程并发 vkCreateInstance 时存在已知竞态
/// （loader_icd_scan 空函数指针，NVIDIA 等 dlopen 重 ICD 环境下随机 SIGSEGV）。
/// 只串行化 Instance::new/request_adapter 不够：wgpu 的 Instance/Adapter 句柄
/// 都存活到渲染结束，vkDestroyInstance 仍与其他线程的实例创建/销毁并发。
/// 这里串行化整个快照渲染，任意时刻只有一个 Vulkan 实例生命周期在跑，
/// ignored 快照套件可以多线程跑（--test-threads=N）而不再触发 ICD 竞态。
static SNAPSHOT_RENDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn render_model_snapshot<M: ModelRenderData + ?Sized>(
    name: impl Into<String>,
    model: &M,
) -> Result<ModelSnapshot, ModelSnapshotError> {
    render_model_snapshot_with_options(ModelSnapshotOptions::new(name), model)
}

pub fn render_model_snapshot_with_options<M: ModelRenderData + ?Sized>(
    options: ModelSnapshotOptions,
    model: &M,
) -> Result<ModelSnapshot, ModelSnapshotError> {
    render_model_snapshot_with_skeleton_and_pose(options, model, None, None)
}

/// [`render_model_snapshot_with_options`] 的骨架/姿势版：`skeleton` 为 Some 时
/// 走蒙皮路径（实例 joint 表 + rest pose 关节矩阵上传）；`pose` 为 Some 时按
/// 实例 joint 名表重算关节矩阵覆盖（姿势更新冒烟用）。`pose` 需提供 `skeleton`。
pub fn render_model_snapshot_with_skeleton_and_pose<M: ModelRenderData + ?Sized>(
    options: ModelSnapshotOptions,
    model: &M,
    skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    pose: Option<&xiv_companion_data::SkeletonPose>,
) -> Result<ModelSnapshot, ModelSnapshotError> {
    require_gpu_test_opt_in().map_err(|_| WeaponModelSnapshotError::GpuTestsDisabled)?;
    let _render_guard = SNAPSHOT_RENDER_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pollster::block_on(render_model_snapshot_async(options, model, skeleton, pose))
}

pub fn render_weapon_model_snapshot(
    name: impl Into<String>,
    model: &impl ModelRenderData,
) -> Result<WeaponModelSnapshot, WeaponModelSnapshotError> {
    render_model_snapshot(name, model)
}

pub fn render_weapon_model_snapshot_with_options(
    options: WeaponModelSnapshotOptions,
    model: &impl ModelRenderData,
) -> Result<WeaponModelSnapshot, WeaponModelSnapshotError> {
    render_model_snapshot_with_options(options, model)
}

async fn render_model_snapshot_async<M: ModelRenderData + ?Sized>(
    options: WeaponModelSnapshotOptions,
    model: &M,
    skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    pose: Option<&xiv_companion_data::SkeletonPose>,
) -> Result<WeaponModelSnapshot, WeaponModelSnapshotError> {
    let snapshot_started = std::time::Instant::now();
    if options.width == 0 || options.height == 0 {
        return Err(WeaponModelSnapshotError::InvalidViewport {
            width: options.width,
            height: options.height,
        });
    }

    fs::create_dir_all(&options.output_dir).map_err(|source| WeaponModelSnapshotError::Io {
        action: "create weapon snapshot output directory",
        path: options.output_dir.clone(),
        source,
    })?;
    let png_path = options
        .output_dir
        .join(format!("{}.png", sanitize_file_stem(&options.name)));

    let instance = snapshot_instance().map_err(|_| WeaponModelSnapshotError::GpuTestsDisabled)?;
    let software_only = software_render_tests_requested();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: options.power_preference,
            compatible_surface: None,
            force_fallback_adapter: software_only || options.force_fallback_adapter,
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestAdapter(format!("{error:?}")))?;
    let adapter_info = adapter.get_info();
    eprintln!(
        "render snapshot {}: adapter={} backend={:?} device_type={:?}",
        options.name, adapter_info.name, adapter_info.backend, adapter_info.device_type
    );
    if software_only {
        validate_software_adapter(adapter_info.backend, adapter_info.device_type)
            .map_err(|error| WeaponModelSnapshotError::RequestAdapter(error.to_owned()))?;
        eprintln!(
            "software render adapter: {} ({:?}, {:?})",
            adapter_info.name, adapter_info.backend, adapter_info.device_type
        );
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            // Exercise the same requested limits as the browser canvas path.
            required_limits: crate::renderer::ModelRenderContext::required_limits(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestDevice(error.to_string()))?;
    let device_ready_elapsed = snapshot_started.elapsed();

    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let msaa_samples = options.render_options.msaa_samples();
    let target = create_target_texture(&device, options.width, options.height, format);
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = create_depth_texture(&device, options.width, options.height, msaa_samples);
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let context =
        crate::renderer::ModelRenderContext::new_with_msaa(device, queue, format, msaa_samples);
    let mut renderer = ModelRenderer::from_context_with_skeleton(
        context,
        model,
        options.prepared_model_options,
        skeleton,
    );
    if let Some(rgba) = options.vfx_reflection_cube_color {
        let texture = renderer.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("native snapshot VFX reflection cube"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for layer in 0..6 {
            renderer.queue().write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("native snapshot VFX reflection cube view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = renderer.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("native snapshot VFX reflection cube sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        renderer.set_vfx_reflection_provider(view, sampler);
    }
    if let Some(rgba) = options.vfx_portrait_color {
        let texture = renderer.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("native snapshot VFX portrait"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        renderer.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = renderer.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("native snapshot VFX portrait sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        renderer.set_vfx_portrait_provider(view, sampler);
    }
    if let (Some(skeleton), Some(pose)) = (skeleton, pose) {
        let joint_names = renderer.joint_names().to_vec();
        let mut cache = xiv_companion_data::SkeletonInverseBindCache::new();
        let matrices = cache.joint_matrices(skeleton, pose, &joint_names);
        renderer.update_joint_matrices(&matrices);
    }
    let vfx_particles = (!options.vfx_quads.is_empty() || !options.vfx_mesh_instances.is_empty())
        .then(|| {
            let mut batch = renderer
                .context()
                .create_vfx_particles(&options.vfx_textures, &options.vfx_meshes);
            batch.update(renderer.context(), &options.vfx_quads);
            batch.update_mesh(renderer.context(), &options.vfx_mesh_instances);
            eprintln!(
                "[vfx batch] quads={} mesh_instances={} mesh_draw_ranges={:?} meshes_gpu={}",
                batch.count,
                batch.mesh_instance_count,
                batch.mesh_draw_ranges,
                batch.meshes.len()
            );
            batch
        });
    let weapon_vfx_particles = options
        .weapon_vfx
        .as_ref()
        .map(|vfx| {
            let camera_for = |input: WeaponVfxCameraInput| {
                renderer.vfx_camera_view_snapshot(
                    [options.width, options.height],
                    input.yaw,
                    input.pitch,
                    input.zoom,
                    input.pan,
                    ModelRenderOptions {
                        camera_roll: input.roll,
                        ..options.render_options
                    },
                )
            };
            let camera = options
                .weapon_vfx_camera_inputs
                .first()
                .copied()
                .map(camera_for)
                .unwrap_or_else(|| {
                    renderer.vfx_camera_view_snapshot(
                        [options.width, options.height],
                        options.yaw,
                        options.pitch,
                        options.zoom,
                        options.pan,
                        options.render_options,
                    )
                });
            let mut particles = renderer
                .create_weapon_vfx_particles_with_camera_view(vfx, camera)
                .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
            let mut camera_inputs = options.weapon_vfx_camera_inputs.iter().peekable();
            for updates in options.weapon_vfx_pose_updates.chunk_by(|a, b| a.0 == b.0) {
                let time = updates[0].0;
                while camera_inputs.peek().is_some_and(|input| input.time < time) {
                    let input = camera_inputs.next().unwrap();
                    renderer
                        .set_weapon_vfx_camera_view(&mut particles, camera_for(*input))
                        .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
                    particles.advance_to(input.time);
                }
                if camera_inputs.peek().is_some_and(|input| input.time == time) {
                    let input = camera_inputs.next().unwrap();
                    renderer
                        .set_weapon_vfx_camera_view(&mut particles, camera_for(*input))
                        .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
                }
                // Commit this camera and every owner's pose in the same input.
                for (_, path, pose) in updates {
                    particles
                        .set_attachment_pose(path, pose)
                        .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
                }
                particles.advance_to(time);
            }
            for input in camera_inputs {
                renderer
                    .set_weapon_vfx_camera_view(&mut particles, camera_for(*input))
                    .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
                particles.advance_to(input.time);
            }
            let final_camera = renderer.vfx_camera_view_snapshot(
                [options.width, options.height],
                options.yaw,
                options.pitch,
                options.zoom,
                options.pan,
                options.render_options,
            );
            renderer
                .set_weapon_vfx_camera_view(&mut particles, final_camera)
                .map_err(WeaponModelSnapshotError::InvalidVfxPose)?;
            renderer.update_weapon_vfx_particles(&mut particles, options.weapon_vfx_time);
            if options.require_staged_weapon_vfx {
                let diagnostics = particles.binding_diagnostics();
                if !diagnostics.is_empty() {
                    return Err(WeaponModelSnapshotError::InvalidVfxPose(format!(
                        "mounted playback did not remain staged: {diagnostics:?}"
                    )));
                }
            }
            Ok::<_, WeaponModelSnapshotError>(particles)
        })
        .transpose()?;
    let rendered_vfx = weapon_vfx_particles
        .as_ref()
        .map(|particles| particles.particles())
        .or(vfx_particles.as_ref());
    let auras = if options.all_unambiguous_auras {
        weapon_vfx_particles
            .as_ref()
            .map_or_else(Vec::new, |particles| {
                particles.unambiguous_active_aura_resources()
            })
    } else {
        options
            .aura_definition
            .and_then(|(attachment_index, particle_index)| {
                let particles = weapon_vfx_particles.as_ref()?;
                let active = particles.active_aura_instances();
                let [instance] = active else { return None };
                particles
                    .aura_resources()
                    .iter()
                    .enumerate()
                    .find(|(index, resource)| {
                        instance.resource_index == *index
                            && resource.attachment_index == attachment_index
                            && resource.particle_index == particle_index
                    })
                    .map(|(_, resource)| resource)
            })
            .into_iter()
            .collect()
    };
    for _ in 0..options.render_repetitions.max(1) {
        renderer.render_to_with_auras(
            &target_view,
            &depth_view,
            [options.width, options.height],
            options.yaw,
            options.pitch,
            options.zoom,
            options.pan,
            options.render_options,
            rendered_vfx,
            &auras,
        );
    }

    let hdr_scene_rgba = if options.capture_hdr_scene {
        let scene_texture = renderer
            .hdr_scene_texture()
            .expect("post-process scene texture is initialized after render");
        Some(read_texture_rgba16f(
            &renderer,
            scene_texture,
            options.width,
            options.height,
        )?)
    } else {
        None
    };
    let rgba = read_texture_rgba(&renderer, &target, options.width, options.height)?;
    write_png(&png_path, options.width, options.height, &rgba)?;
    eprintln!(
        "render snapshot {}: complete; adapter/device={:.3}s total={:.3}s",
        options.name,
        device_ready_elapsed.as_secs_f64(),
        snapshot_started.elapsed().as_secs_f64()
    );

    Ok(WeaponModelSnapshot {
        png_path,
        width: options.width,
        height: options.height,
        adapter_name: adapter_info.name,
        adapter_backend: adapter_info.backend,
        hdr_scene_rgba,
        weapon_vfx_quads: weapon_vfx_particles
            .as_ref()
            .map_or_else(Vec::new, |p| p.sampled_quads().to_vec()),
    })
}

async fn render_scene_snapshot_async(
    options: WeaponModelSnapshotOptions,
    entries: &[SceneSnapshotEntry<'_>],
    skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    pose: Option<&xiv_companion_data::SkeletonPose>,
) -> Result<WeaponModelSnapshot, WeaponModelSnapshotError> {
    if options.width == 0 || options.height == 0 {
        return Err(WeaponModelSnapshotError::InvalidViewport {
            width: options.width,
            height: options.height,
        });
    }

    fs::create_dir_all(&options.output_dir).map_err(|source| WeaponModelSnapshotError::Io {
        action: "create weapon snapshot output directory",
        path: options.output_dir.clone(),
        source,
    })?;
    let png_path = options
        .output_dir
        .join(format!("{}.png", sanitize_file_stem(&options.name)));

    let instance = snapshot_instance().map_err(|_| WeaponModelSnapshotError::GpuTestsDisabled)?;
    let software_only = software_render_tests_requested();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: options.power_preference,
            compatible_surface: None,
            force_fallback_adapter: software_only || options.force_fallback_adapter,
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestAdapter(format!("{error:?}")))?;
    let adapter_info = adapter.get_info();
    eprintln!(
        "render scene snapshot {}: adapter={} backend={:?} device_type={:?}",
        options.name, adapter_info.name, adapter_info.backend, adapter_info.device_type
    );
    if software_only {
        validate_software_adapter(adapter_info.backend, adapter_info.device_type)
            .map_err(|error| WeaponModelSnapshotError::RequestAdapter(error.to_owned()))?;
    }

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: crate::renderer::ModelRenderContext::required_limits(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestDevice(error.to_string()))?;

    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let msaa_samples = options.render_options.msaa_samples();
    let target = create_target_texture(&device, options.width, options.height, format);
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = create_depth_texture(&device, options.width, options.height, msaa_samples);
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let mut context =
        crate::renderer::ModelRenderContext::new_with_msaa(device, queue, format, msaa_samples);
    let mut instances: Vec<crate::renderer::ModelInstance> = entries
        .iter()
        .map(|entry| {
            context.create_model_with_skeleton(
                entry.model,
                entry.prepared_options.clone(),
                skeleton,
            )
        })
        .collect();
    let mut inverse_bind = xiv_companion_data::SkeletonInverseBindCache::new();
    for (entry, instance) in entries.iter().zip(instances.iter_mut()) {
        if let Some(matrices) = &entry.joint_matrices {
            instance.update_joint_matrices(&context, matrices);
        } else if let (Some(skeleton), Some(pose)) = (skeleton, pose) {
            let matrices = inverse_bind.joint_matrices(skeleton, pose, instance.joint_names());
            instance.update_joint_matrices(&context, &matrices);
        }
    }
    let instance_refs: Vec<&crate::renderer::ModelInstance> = instances.iter().collect();
    let vfx_particles = (!options.vfx_quads.is_empty() || !options.vfx_mesh_instances.is_empty())
        .then(|| {
            let mut batch =
                context.create_vfx_particles(&options.vfx_textures, &options.vfx_meshes);
            batch.update(&context, &options.vfx_quads);
            batch.update_mesh(&context, &options.vfx_mesh_instances);
            batch
        });
    context.render_scene(
        &instance_refs,
        &target_view,
        &depth_view,
        [options.width, options.height],
        options.yaw,
        options.pitch,
        options.zoom,
        options.pan,
        options.render_options,
        vfx_particles.as_ref(),
        &[],
    );

    let rgba = read_texture_bytes(
        context.device(),
        context.queue(),
        &target,
        options.width,
        options.height,
        4,
    )?;
    write_png(&png_path, options.width, options.height, &rgba)?;

    Ok(WeaponModelSnapshot {
        png_path,
        width: options.width,
        height: options.height,
        adapter_name: adapter_info.name,
        adapter_backend: adapter_info.backend,
        hdr_scene_rgba: None,
        weapon_vfx_quads: Vec::new(),
    })
}

fn create_target_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native weapon snapshot target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native weapon snapshot depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn read_texture_rgba(
    renderer: &ModelRenderer,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, WeaponModelSnapshotError> {
    read_texture_bytes(
        renderer.device(),
        renderer.queue(),
        texture,
        width,
        height,
        4,
    )
}

fn read_texture_rgba16f(
    renderer: &ModelRenderer,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<[f32; 4]>, WeaponModelSnapshotError> {
    let bytes = read_texture_bytes(
        renderer.device(),
        renderer.queue(),
        texture,
        width,
        height,
        8,
    )?;
    Ok(bytes
        .chunks_exact(8)
        .map(|pixel| {
            std::array::from_fn(|channel| {
                let offset = channel * 2;
                f16::from_bits(u16::from_le_bytes([pixel[offset], pixel[offset + 1]])).to_f32()
            })
        })
        .collect())
}

fn read_texture_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> Result<Vec<u8>, WeaponModelSnapshotError> {
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let output_buffer_size = padded_bytes_per_row as u64 * height as u64;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("native weapon snapshot readback"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("native weapon snapshot readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit(std::iter::once(encoder.finish()));
    let buffer_slice = output_buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .map_err(|error| WeaponModelSnapshotError::Poll(error.to_string()))?;
    receiver
        .recv()
        .map_err(|_| WeaponModelSnapshotError::MapCallbackDropped)?
        .map_err(|error| WeaponModelSnapshotError::Map(error.to_string()))?;

    let mapped = buffer_slice.get_mapped_range();
    let mut rgba = vec![0; unpadded_bytes_per_row as usize * height as usize];
    for row in 0..height as usize {
        let src_start = row * padded_bytes_per_row as usize;
        let src_end = src_start + unpadded_bytes_per_row as usize;
        let dst_start = row * unpadded_bytes_per_row as usize;
        rgba[dst_start..dst_start + unpadded_bytes_per_row as usize]
            .copy_from_slice(&mapped[src_start..src_end]);
    }
    drop(mapped);
    output_buffer.unmap();
    Ok(rgba)
}

fn write_png(
    path: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), WeaponModelSnapshotError> {
    image::save_buffer_with_format(
        path,
        rgba,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|source| WeaponModelSnapshotError::Image {
        path: path.to_path_buf(),
        source,
    })
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn sanitize_file_stem(name: &str) -> String {
    let mut stem = String::with_capacity(name.len().max(1));
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            stem.push(ch);
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }

    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "weapon-model".to_string()
    } else {
        stem.to_string()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod native_resource_tests {
    #[test]
    #[ignore = "requires native GPU drivers and /proc"]
    fn snapshot_initialization_does_not_accumulate_device_descriptors() {
        let initialize = || {
            let instance = super::snapshot_instance().expect("explicit native GPU test opt-in");
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    power_preference: wgpu::PowerPreference::HighPerformance,
                }))
                .unwrap();
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .unwrap();
            drop(queue);
            drop(device);
            drop(adapter);
            drop(instance);
        };
        let count = || std::fs::read_dir("/proc/self/fd").unwrap().count();
        initialize();
        let baseline = count();
        for _ in 0..12 {
            initialize();
        }
        let after = count();
        assert!(
            after <= baseline + 4,
            "GPU initialization retained descriptors: {baseline} -> {after}"
        );
    }
}

#[cfg(test)]
mod gpu_test_policy_tests {
    #[test]
    fn software_render_tests_reject_hardware_adapters() {
        use wgpu::{Backend, DeviceType};
        assert!(super::validate_software_adapter(Backend::Vulkan, DeviceType::Cpu).is_ok());
        for kind in [
            DeviceType::DiscreteGpu,
            DeviceType::IntegratedGpu,
            DeviceType::VirtualGpu,
            DeviceType::Other,
        ] {
            assert!(super::validate_software_adapter(Backend::Vulkan, kind).is_err());
        }
        assert!(super::validate_software_adapter(Backend::Gl, DeviceType::Cpu).is_err());
    }

    #[test]
    fn denied_snapshot_does_not_initialize_native_drivers() {
        let before = super::SNAPSHOT_INSTANCE.get().is_some();
        assert!(super::snapshot_instance_with_opt_in(None).is_err());
        assert_eq!(super::SNAPSHOT_INSTANCE.get().is_some(), before);
    }

    #[test]
    fn native_gpu_tests_require_exact_explicit_opt_in() {
        use std::ffi::OsStr;
        assert!(super::check_gpu_test_opt_in(None).is_err());
        for value in ["", "0", "true", "yes", " 1", "1 "] {
            assert!(super::check_gpu_test_opt_in(Some(OsStr::new(value))).is_err());
        }
        assert!(super::check_gpu_test_opt_in(Some(OsStr::new("1"))).is_ok());
    }
}
