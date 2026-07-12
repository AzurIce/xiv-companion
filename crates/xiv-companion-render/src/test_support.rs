use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

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
    pollster::block_on(render_model_snapshot_async(options, model))
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
    let target = create_target_texture(&device, options.width, options.height, format);
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = create_depth_texture(&device, options.width, options.height);
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer = ModelRenderer::new_with_prepared_options(
        device,
        queue,
        format,
        model,
        options.prepared_model_options,
    );
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

    let rgba = read_texture_rgba(&renderer, &target, options.width, options.height)?;
    write_png(&png_path, options.width, options.height, &rgba)?;

    Ok(WeaponModelSnapshot {
        png_path,
        width: options.width,
        height: options.height,
        adapter_name: adapter_info.name,
        adapter_backend: adapter_info.backend,
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

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native weapon snapshot depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
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
    let bytes_per_pixel = 4;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let output_buffer_size = padded_bytes_per_row as u64 * height as u64;
    let output_buffer = renderer.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("native weapon snapshot readback"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = renderer
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
    let submission = renderer.queue().submit(std::iter::once(encoder.finish()));
    let buffer_slice = output_buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    renderer
        .device()
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
