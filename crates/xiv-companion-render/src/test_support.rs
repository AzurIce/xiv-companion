use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use half::f16;

use crate::renderer::{ModelRenderOptions, ModelRenderer};
use crate::{ModelRenderData, PreparedModelOptions};

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
}

#[derive(Debug)]
pub enum WeaponModelSnapshotError {
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

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: options.power_preference,
            compatible_surface: None,
            force_fallback_adapter: options.force_fallback_adapter,
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestAdapter(format!("{error:?}")))?;
    let adapter_info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
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

    let context =
        crate::renderer::ModelRenderContext::new_with_msaa(device, queue, format, msaa_samples);
    let mut renderer = ModelRenderer::from_context_with_skeleton(
        context,
        model,
        options.prepared_model_options,
        skeleton,
    );
    if let (Some(skeleton), Some(pose)) = (skeleton, pose) {
        let joint_names = renderer.joint_names().to_vec();
        let mut cache = xiv_companion_data::SkeletonInverseBindCache::new();
        let matrices = cache.joint_matrices(skeleton, pose, &joint_names);
        renderer.update_joint_matrices(&matrices);
    }
    renderer.render_to(
        &target_view,
        &depth_view,
        [options.width, options.height],
        options.yaw,
        options.pitch,
        options.zoom,
        options.pan,
        options.render_options,
    );

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

    Ok(WeaponModelSnapshot {
        png_path,
        width: options.width,
        height: options.height,
        adapter_name: adapter_info.name,
        adapter_backend: adapter_info.backend,
        hdr_scene_rgba,
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

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: options.power_preference,
            compatible_surface: None,
            force_fallback_adapter: options.force_fallback_adapter,
        })
        .await
        .map_err(|error| WeaponModelSnapshotError::RequestAdapter(format!("{error:?}")))?;
    let adapter_info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
