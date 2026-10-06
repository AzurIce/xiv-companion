use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WebCanvasWindowHandle, WebDisplayHandle,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlCanvasElement;
use xiv_companion::renderer::{
    ModelInstance, ModelRenderContext, ModelRenderOptions, WeaponVfxParticles,
};
use xiv_companion::{ModelRenderData, PreparedModelOptions, WeaponVfxAttachments};

/// 场景实例条目：稳定 key（如 "body" / "slot:4"）+ GPU 实例 + 可选的武器
/// 挂接规则。key 供逐件增量更新（换件只重建该件实例、染色只更新该件材质）
/// 定位；`attach` 为 Some 时该实例的 joint 矩阵按挂点骨规则（姿势世界 ×
/// 校正）逐帧驱动，不走 inverse bind。
struct SceneEntry {
    key: String,
    instance: ModelInstance,
    attach: Option<xiv_companion::WeaponAttachInfo>,
}

/// 单模型模式的场景 key（`set_model` 等单模型 API 的作用目标）。
const SINGLE_MODEL_SCENE_KEY: &str = "model";

pub struct WebModelCanvasRenderer {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    context: ModelRenderContext,
    scene: Vec<SceneEntry>,
    /// 场景结构修订号：实例增删/替换时递增（rAF 动画驱动据此刷新逐件 joint 表）。
    scene_revision: u64,
    /// 常驻 VFX 运行时与 GPU 批次（单模型预览）。
    vfx: Option<WeaponVfxParticles>,
    orbit: Rc<RefCell<OrbitState>>,
    msaa_samples: u32,
    _on_mouse_down: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_mouse_move: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_mouse_up: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_wheel: Closure<dyn FnMut(web_sys::WheelEvent)>,
    _on_context_menu: Closure<dyn FnMut(web_sys::Event)>,
}

impl WebModelCanvasRenderer {
    /// 一次性异步初始化：WebGPU instance/surface/adapter/device 与模型无关的
    /// 渲染 context（管线、后处理）。模型实例随后经 `set_model` 同步挂载。
    pub async fn from_canvas(canvas: HtmlCanvasElement) -> Result<Self, String> {
        Self::from_canvas_with_msaa(canvas, 1).await
    }

    /// MSAA 变体：`msaa_samples` 为场景目标/管线/深度纹理的采样数
    /// （1 = 关闭，4 = 4x）。切换需重建整个 canvas renderer。
    pub async fn from_canvas_with_msaa(
        canvas: HtmlCanvasElement,
        msaa_samples: u32,
    ) -> Result<Self, String> {
        let msaa_samples = if msaa_samples >= 3 { 4 } else { 1 };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let canvas_ptr: NonNull<core::ffi::c_void> = NonNull::from(&canvas).cast();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Web(WebDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::WebCanvas(WebCanvasWindowHandle::new(canvas_ptr)),
        };
        let surface = unsafe {
            instance
                .create_surface_unsafe(target)
                .map_err(|error| format!("create surface failed: {error:?}"))?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("request adapter failed: {error:?}"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: ModelRenderContext::required_limits(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await
            .map_err(|error| format!("request device failed: {error:?}"))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let (width, height) = canvas_pixel_size(&canvas);
        canvas.set_width(width);
        canvas.set_height(height);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth_texture = create_depth_texture(&device, width, height, msaa_samples);
        let initialization = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let context = ModelRenderContext::new_with_msaa(device, queue, config.format, msaa_samples);
        if let Some(error) = initialization.pop().await {
            return Err(format!("初始化渲染管线失败：{error}"));
        }
        let orbit = Rc::new(RefCell::new(OrbitState::default()));
        let (on_mouse_down, on_mouse_move, on_mouse_up, on_wheel, on_context_menu) =
            install_orbit_handlers(&canvas, orbit.clone())?;

        Ok(Self {
            canvas,
            surface,
            config,
            depth_texture,
            context,
            scene: Vec::new(),
            scene_revision: 0,
            vfx: None,
            orbit,
            msaa_samples,
            _on_mouse_down: on_mouse_down,
            _on_mouse_move: on_mouse_move,
            _on_mouse_up: on_mouse_up,
            _on_wheel: on_wheel,
            _on_context_menu: on_context_menu,
        })
    }

    /// Replace all mounted effects and their shared GPU resources.
    pub fn set_vfx(&mut self, vfx: Option<&WeaponVfxAttachments>, options: ModelRenderOptions) {
        let orbit = self.orbit.borrow();
        self.vfx = self
            .scene
            .iter()
            .find(|entry| entry.key == SINGLE_MODEL_SCENE_KEY)
            .and_then(|entry| {
                let model = &entry.instance;
                let data = vfx?;
                let camera = self.context.vfx_camera_view_snapshot(
                    model,
                    [self.config.width, self.config.height],
                    orbit.yaw,
                    orbit.pitch,
                    orbit.zoom,
                    [orbit.pan_x, orbit.pan_y],
                    options,
                );
                match self
                    .context
                    .create_weapon_vfx_particles_with_camera_view(model, data, camera)
                {
                    Ok(vfx) => Some(vfx),
                    Err(error) => {
                        web_sys::console::warn_1(&format!("VFX camera: {error}").into());
                        None
                    }
                }
            });
    }

    /// Forward an owning weapon's sampled local pose before rendering this
    /// frame. VFX targets are bone world matrices, independent of skinning.
    pub fn update_vfx_pose(
        &mut self,
        model_path: &str,
        pose: &xiv_companion_data::SkeletonPose,
    ) -> Result<(), String> {
        self.vfx
            .as_mut()
            .ok_or("VFX attachments unavailable")?
            .set_attachment_pose(model_path, pose)
    }

    /// 同步替换当前模型实例（单模型模式）：重建顶点/索引缓冲、绘制批次与
    /// 材质 bind group，不触碰设备、管线、surface 与轨道相机。等价于
    /// `set_scene(vec![(SINGLE_MODEL_SCENE_KEY, instance)])`。
    pub fn set_model<M: ModelRenderData + ?Sized>(
        &mut self,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion::ModelSkeleton>,
    ) {
        let instance = self
            .context
            .create_model_with_skeleton(model, prepared_options, skeleton);
        self.set_scene(vec![(SINGLE_MODEL_SCENE_KEY.to_string(), instance)]);
    }

    /// 为 `model` 创建 GPU 实例（骨架版）：`skeleton` 为 Some 时构建实例
    /// joint 表（网格 bone_table 名并集，按名映射到骨架）并上传 rest pose
    /// 关节矩阵；无骨架时 joint 数 0（静态 bind pose）。配合
    /// [`Self::set_scene`]/[`Self::upsert_instance_with_attach`] 组装多实例场景。
    pub fn create_instance<M: ModelRenderData + ?Sized>(
        &self,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion::ModelSkeleton>,
    ) -> ModelInstance {
        self.context
            .create_model_with_skeleton(model, prepared_options, skeleton)
    }

    /// 全量替换场景（身体 + 各槽位装备件等多实例）。传入顺序即绘制顺序；
    /// scene revision 递增。
    pub fn set_scene(&mut self, entries: Vec<(String, ModelInstance)>) {
        self.vfx = None;
        self.scene = entries
            .into_iter()
            .map(|(key, instance)| SceneEntry {
                key,
                instance,
                attach: None,
            })
            .collect();
        self.scene_revision += 1;
    }

    /// 增/替单件实例：key 已存在则原位替换（保持绘制顺序），不存在则尾部
    /// 追加。换件/改隐藏标签只重建对应实例，其余实例不动。`attach` 为 Some
    /// 时该实例是武器挂接件：动画驱动按挂点骨规则（姿势世界 × 校正）逐帧
    /// 出该实例的 joint 矩阵，不走 inverse bind。
    pub fn upsert_instance_with_attach(
        &mut self,
        key: &str,
        instance: ModelInstance,
        attach: Option<xiv_companion::WeaponAttachInfo>,
    ) {
        if key == SINGLE_MODEL_SCENE_KEY {
            self.vfx = None;
        }
        match self.scene.iter().position(|entry| entry.key == key) {
            Some(index) => {
                self.scene[index].instance = instance;
                self.scene[index].attach = attach;
            }
            None => self.scene.push(SceneEntry {
                key: key.to_string(),
                instance,
                attach,
            }),
        }
        self.scene_revision += 1;
    }

    /// 移除单件实例；key 不存在为空操作。返回是否发生移除。
    pub fn remove_instance(&mut self, key: &str) -> bool {
        let removed = self
            .scene
            .iter()
            .position(|entry| entry.key == key)
            .map(|index| self.scene.remove(index))
            .is_some();
        if removed {
            self.scene_revision += 1;
        }
        removed
    }

    /// 场景结构修订号（实例增删/替换递增；材质/joint 更新不变）。
    pub fn scene_revision(&self) -> u64 {
        self.scene_revision
    }

    /// 场景全部实例的 key（绘制顺序）。
    pub fn scene_keys(&self) -> Vec<String> {
        self.scene.iter().map(|entry| entry.key.clone()).collect()
    }

    /// 物品切换时对齐重建画布的旧行为，重置轨道相机视角。
    pub fn reset_orbit(&self) {
        *self.orbit.borrow_mut() = OrbitState::default();
    }

    pub fn render_with_options(&mut self, options: ModelRenderOptions) {
        self.resize_to_client();
        if self.scene.is_empty() {
            return;
        }
        let orbit = self.orbit.borrow();
        let mut vfx_ready = true;
        let instance = self
            .scene
            .iter()
            .find(|entry| entry.key == SINGLE_MODEL_SCENE_KEY)
            .map(|entry| &entry.instance);
        if let (Some(vfx), Some(instance)) = (&mut self.vfx, instance) {
            let camera = self.context.vfx_camera_view_snapshot(
                instance,
                [self.config.width, self.config.height],
                orbit.yaw,
                orbit.pitch,
                orbit.zoom,
                [orbit.pan_x, orbit.pan_y],
                options,
            );
            if let Err(error) = vfx.set_camera_view(instance, camera) {
                web_sys::console::warn_1(&format!("VFX camera: {error}").into());
                vfx_ready = false;
            } else {
                vfx.advance_to(options.vfx_time);
            }
        }
        let instances = self
            .scene
            .iter()
            .map(|entry| &entry.instance)
            .collect::<Vec<_>>();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            other => {
                web_sys::console::warn_1(&format!("model surface not ready: {other:?}").into());
                return;
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Read the already advanced VFX state; equal timestamps do not update twice.
        if options.vfx_enabled && vfx_ready {
            if let (Some(vfx), Some(instance)) = (&mut self.vfx, instance) {
                vfx.update(&self.context, instance, options.vfx_time);
            }
        }
        let vfx = (options.vfx_enabled && vfx_ready)
            .then_some(self.vfx.as_ref())
            .flatten()
            .map(WeaponVfxParticles::particles);
        let auras = (options.vfx_enabled && vfx_ready)
            .then_some(self.vfx.as_ref())
            .flatten()
            .map_or_else(Vec::new, |vfx| vfx.unambiguous_active_aura_resources());
        self.context.render_scene(
            &instances,
            &view,
            &depth_view,
            [self.config.width, self.config.height],
            orbit.yaw,
            orbit.pitch,
            orbit.zoom,
            [orbit.pan_x, orbit.pan_y],
            options,
            vfx,
            &auras,
        );
        output.present();
    }

    /// 单模型模式的材质增量更新（等价 `update_scene_materials` 对
    /// SINGLE_MODEL_SCENE_KEY）。
    pub fn update_materials<M: ModelRenderData + ?Sized>(&mut self, model: &M) {
        self.update_scene_materials(SINGLE_MODEL_SCENE_KEY, model);
    }

    /// 单件材质增量更新（染色编辑只重建该件材质 bind group，不动实例几何
    /// 与 joint 绑定）；key 不存在为空操作。
    pub fn update_scene_materials<M: ModelRenderData + ?Sized>(&mut self, key: &str, model: &M) {
        if let Some(entry) = self.scene.iter_mut().find(|entry| entry.key == key) {
            entry.instance.update_materials(&self.context, model);
        }
    }

    /// 实例 joint 名表（单模型模式；蒙皮实例为空表）；动画播放驱动按名计算
    /// 关节矩阵。多实例场景用 [`Self::scene_joint_tables`]。
    pub fn joint_names(&self) -> Vec<String> {
        self.scene
            .iter()
            .find(|entry| entry.key == SINGLE_MODEL_SCENE_KEY)
            .map(|entry| entry.instance.joint_names().to_vec())
            .unwrap_or_default()
    }

    /// 场景全部实例的 joint 表 + 武器挂接规则（key + 名表 + attach，绘制
    /// 顺序）。attach 为 Some 的实例按挂点骨规则驱动（姿势世界 × 校正），
    /// 不走 inverse bind；其余实例走世界 × inverse(bind world)。
    pub fn scene_joint_tables(
        &self,
    ) -> Vec<(String, Vec<String>, Option<xiv_companion::WeaponAttachInfo>)> {
        self.scene
            .iter()
            .map(|entry| {
                (
                    entry.key.clone(),
                    entry.instance.joint_names().to_vec(),
                    entry.attach,
                )
            })
            .collect()
    }

    /// 覆盖实例 joint 矩阵（单模型模式；动画采样结果增量上传，立即生效于
    /// 后续渲染）；无蒙皮实例为空操作。
    pub fn update_joint_matrices(&mut self, matrices: &[[f32; 16]]) {
        self.update_scene_joint_matrices(SINGLE_MODEL_SCENE_KEY, matrices);
    }

    /// 单件 joint 矩阵上传；key 不存在为空操作。
    pub fn update_scene_joint_matrices(&mut self, key: &str, matrices: &[[f32; 16]]) {
        if let Some(entry) = self.scene.iter_mut().find(|entry| entry.key == key) {
            entry
                .instance
                .update_joint_matrices(&self.context, matrices);
        }
    }

    pub fn canvas_connected(&self) -> bool {
        self.canvas.is_connected()
    }

    fn resize_to_client(&mut self) {
        let (width, height) = canvas_pixel_size(&self.canvas);
        if width == 0 || height == 0 {
            return;
        }
        if width != self.config.width || height != self.config.height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(self.context.device(), &self.config);
            self.depth_texture =
                create_depth_texture(self.context.device(), width, height, self.msaa_samples);
        }
    }
}

#[derive(Debug)]
struct OrbitState {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    dragging: bool,
    panning: bool,
    last_x: f32,
    last_y: f32,
}

impl Default for OrbitState {
    fn default() -> Self {
        Self {
            yaw: 0.65,
            pitch: 0.35,
            zoom: 3.2,
            pan_x: 0.0,
            pan_y: 0.0,
            dragging: false,
            panning: false,
            last_x: 0.0,
            last_y: 0.0,
        }
    }
}

fn install_orbit_handlers(
    canvas: &HtmlCanvasElement,
    orbit: Rc<RefCell<OrbitState>>,
) -> Result<
    (
        Closure<dyn FnMut(web_sys::MouseEvent)>,
        Closure<dyn FnMut(web_sys::MouseEvent)>,
        Closure<dyn FnMut(web_sys::MouseEvent)>,
        Closure<dyn FnMut(web_sys::WheelEvent)>,
        Closure<dyn FnMut(web_sys::Event)>,
    ),
    String,
> {
    let down_orbit = orbit.clone();
    let on_mouse_down = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        event.prevent_default();
        let mut orbit = down_orbit.borrow_mut();
        orbit.dragging = true;
        orbit.panning = event.button() != 0 || event.shift_key();
        orbit.last_x = event.client_x() as f32;
        orbit.last_y = event.client_y() as f32;
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref())
        .map_err(format_js_error)?;

    let move_orbit = orbit.clone();
    let on_mouse_move = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
        let mut orbit = move_orbit.borrow_mut();
        if !orbit.dragging {
            return;
        }
        event.prevent_default();
        let x = event.client_x() as f32;
        let y = event.client_y() as f32;
        let dx = x - orbit.last_x;
        let dy = y - orbit.last_y;
        orbit.last_x = x;
        orbit.last_y = y;
        if orbit.panning || event.shift_key() {
            let pan_scale = 0.0025 * orbit.zoom.max(1.0);
            orbit.pan_x -= dx * pan_scale;
            orbit.pan_y += dy * pan_scale;
        } else {
            orbit.yaw -= dx * 0.01;
            orbit.pitch = (orbit.pitch + dy * 0.01).clamp(-1.35, 1.35);
        }
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mousemove", on_mouse_move.as_ref().unchecked_ref())
        .map_err(format_js_error)?;

    let up_orbit = orbit.clone();
    let on_mouse_up = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
        let mut orbit = up_orbit.borrow_mut();
        orbit.dragging = false;
        orbit.panning = false;
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("mouseup", on_mouse_up.as_ref().unchecked_ref())
        .map_err(format_js_error)?;
    canvas
        .add_event_listener_with_callback("mouseleave", on_mouse_up.as_ref().unchecked_ref())
        .map_err(format_js_error)?;

    let wheel_orbit = orbit.clone();
    let on_wheel = Closure::wrap(Box::new(move |event: web_sys::WheelEvent| {
        event.prevent_default();
        let mut orbit = wheel_orbit.borrow_mut();
        orbit.zoom = (orbit.zoom + event.delta_y() as f32 * 0.002).clamp(1.35, 12.0);
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref())
        .map_err(format_js_error)?;

    let on_context_menu = Closure::wrap(Box::new(move |event: web_sys::Event| {
        event.prevent_default();
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("contextmenu", on_context_menu.as_ref().unchecked_ref())
        .map_err(format_js_error)?;

    Ok((
        on_mouse_down,
        on_mouse_move,
        on_mouse_up,
        on_wheel,
        on_context_menu,
    ))
}

fn canvas_pixel_size(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let rect = canvas.get_bounding_client_rect();
    let pixel_ratio = web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0)
        .clamp(1.0, 2.0);
    let width = (rect.width() * pixel_ratio).round().max(1.0) as u32;
    let height = (rect.height() * pixel_ratio).round().max(1.0) as u32;
    (width, height)
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("model depth texture"),
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

fn format_js_error(error: wasm_bindgen::JsValue) -> String {
    js_sys::Reflect::get(&error, &wasm_bindgen::JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "browser event call failed".to_string())
}
