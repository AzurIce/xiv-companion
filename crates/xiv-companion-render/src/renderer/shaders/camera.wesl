// Shared CPU CameraUniform layout for model, VFX and decal shaders.
struct Camera {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    view_dir: vec4<f32>,
    camera_position: vec4<f32>,
    options: vec4<f32>, // x: normal mapping, y: normal y sign, z: uv scroll time, w: debug mode
    dynamic_emissive_color: vec4<f32>, // runtime material dynamic emissive multiplier
    instance_env_parameter: vec4<f32>, // x: runtime InstanceParameter.m_EnvParameter.w
    viewport: vec4<f32>, // x/y framebuffer dimensions in pixels
    view_right: vec4<f32>, // view rotation axes, before projection
    view_up: vec4<f32>,
};
