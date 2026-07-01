use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WebCanvasWindowHandle, WebDisplayHandle,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlCanvasElement;

use crate::WeaponModelData;

use super::WeaponRenderer;

pub struct WebWeaponCanvasRenderer {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    renderer: WeaponRenderer,
    orbit: Rc<RefCell<OrbitState>>,
    _on_mouse_down: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_mouse_move: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_mouse_up: Closure<dyn FnMut(web_sys::MouseEvent)>,
    _on_wheel: Closure<dyn FnMut(web_sys::WheelEvent)>,
    _on_context_menu: Closure<dyn FnMut(web_sys::Event)>,
}

impl WebWeaponCanvasRenderer {
    pub async fn from_canvas(
        canvas: HtmlCanvasElement,
        model: &WeaponModelData,
    ) -> Result<Self, String> {
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
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
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
        let depth_texture = create_depth_texture(&device, width, height);
        let renderer = WeaponRenderer::new(device, queue, config.format, model);
        let orbit = Rc::new(RefCell::new(OrbitState::default()));
        let (on_mouse_down, on_mouse_move, on_mouse_up, on_wheel, on_context_menu) =
            install_orbit_handlers(&canvas, orbit.clone())?;

        Ok(Self {
            canvas,
            surface,
            config,
            depth_texture,
            renderer,
            orbit,
            _on_mouse_down: on_mouse_down,
            _on_mouse_move: on_mouse_move,
            _on_mouse_up: on_mouse_up,
            _on_wheel: on_wheel,
            _on_context_menu: on_context_menu,
        })
    }

    pub fn render(&mut self) {
        self.resize_to_client();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            other => {
                web_sys::console::warn_1(&format!("weapon surface not ready: {other:?}").into());
                return;
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let orbit = self.orbit.borrow();
        self.renderer.render_to(
            &view,
            &depth_view,
            [self.config.width, self.config.height],
            orbit.yaw,
            orbit.pitch,
            orbit.zoom,
            [orbit.pan_x, orbit.pan_y],
        );
        output.present();
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
            self.surface.configure(self.renderer.device(), &self.config);
            self.depth_texture = create_depth_texture(self.renderer.device(), width, height);
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

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("weapon depth texture"),
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

fn format_js_error(error: wasm_bindgen::JsValue) -> String {
    js_sys::Reflect::get(&error, &wasm_bindgen::JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "browser event call failed".to_string())
}
