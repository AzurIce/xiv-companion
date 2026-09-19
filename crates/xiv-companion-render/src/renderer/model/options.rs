use super::*;

pub(crate) const DEFAULT_BLOOM_STRENGTH: f32 = 0.68;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelDebugMode {
    #[default]
    Final,
    BaseColor,
    Normal,
    Mask,
    MaterialProperties,
    Specular,
    Emissive,
    Alpha,
    Uv0,
    Uv1,
    Uv2,
    Uv3,
    VertexColor,
    MeshRole,
    ColorTableIndex,
    MultiMap,
    TileProperties,
    SheenProperties,
    SphereProperties,
    TileMatrix,
    TileNormalArray,
    TileOrbArray,
    DetailDiffuseArray,
    DetailNormalArray,
    VertexColor1,
    SecondaryNormal,
    Flow0,
    Flow1,
    /// Paints each material with a diagnostic color derived from
    /// `PreparedMaterialUnsupportedInputs`, so known-incomplete shader
    /// families stop passing silently as fully supported renders.
    UnsupportedInputs,
    /// Encodes the per-fragment world-space direction to the camera as
    /// `view * 0.5 + 0.5`, useful for auditing perspective Fresnel/normals.
    ViewDirection,
}

impl ModelDebugMode {
    pub(crate) fn shader_value(self) -> f32 {
        match self {
            ModelDebugMode::Final => 0.0,
            ModelDebugMode::BaseColor => 1.0,
            ModelDebugMode::Normal => 2.0,
            ModelDebugMode::Mask => 3.0,
            ModelDebugMode::MaterialProperties => 4.0,
            ModelDebugMode::Specular => 5.0,
            ModelDebugMode::Emissive => 6.0,
            ModelDebugMode::Alpha => 7.0,
            ModelDebugMode::Uv0 => 8.0,
            ModelDebugMode::Uv1 => 9.0,
            ModelDebugMode::Uv2 => 10.0,
            ModelDebugMode::Uv3 => 11.0,
            ModelDebugMode::VertexColor => 12.0,
            ModelDebugMode::MeshRole => 13.0,
            ModelDebugMode::ColorTableIndex => 14.0,
            ModelDebugMode::MultiMap => 15.0,
            ModelDebugMode::TileProperties => 16.0,
            ModelDebugMode::SheenProperties => 17.0,
            ModelDebugMode::SphereProperties => 18.0,
            ModelDebugMode::TileMatrix => 19.0,
            ModelDebugMode::TileNormalArray => 20.0,
            ModelDebugMode::TileOrbArray => 21.0,
            ModelDebugMode::DetailDiffuseArray => 22.0,
            ModelDebugMode::DetailNormalArray => 23.0,
            ModelDebugMode::VertexColor1 => 24.0,
            ModelDebugMode::SecondaryNormal => 25.0,
            ModelDebugMode::Flow0 => 26.0,
            ModelDebugMode::Flow1 => 27.0,
            ModelDebugMode::UnsupportedInputs => 28.0,
            ModelDebugMode::ViewDirection => 29.0,
        }
    }
}

pub(crate) fn lightshaft_uses_dedicated_pipeline(debug_mode: ModelDebugMode) -> bool {
    matches!(debug_mode, ModelDebugMode::Final)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelGlassBlendMode {
    #[default]
    Alpha,
    Additive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelRenderOptions {
    pub normal_mapping: bool,
    pub normal_y_sign: f32,
    pub bloom: bool,
    pub bloom_strength: f32,
    pub uv_scroll_time: f32,
    /// VFX 粒子采样时钟（秒）；web 循环每帧写入，快照路径固定值保证确定性。
    pub vfx_time: f32,
    /// 是否绘制常驻 VFX 粒子批次（页面「武器特效」开关）。
    pub vfx_enabled: bool,
    /// Runtime material dynamic emissive multiplier. The preview default is
    /// the identity; the shader consumes it only for ColorTable emissive.
    pub dynamic_emissive_color: [f32; 3],
    pub debug_mode: ModelDebugMode,
    pub glass_blend_mode: ModelGlassBlendMode,
    /// 场景 HDR 目标的 MSAA 采样数：1 = 关闭，4 = 4x 多重采样（resolve 后进
    /// bloom/compose）。属于渲染上下文创建参数，切换需要重建管线与场景纹理。
    pub msaa_samples: u32,
}

impl Default for ModelRenderOptions {
    fn default() -> Self {
        Self {
            normal_mapping: true,
            normal_y_sign: -1.0,
            bloom: true,
            bloom_strength: DEFAULT_BLOOM_STRENGTH,
            uv_scroll_time: 0.0,
            vfx_time: 0.0,
            vfx_enabled: true,
            dynamic_emissive_color: [1.0; 3],
            debug_mode: ModelDebugMode::Final,
            glass_blend_mode: ModelGlassBlendMode::Alpha,
            msaa_samples: 1,
        }
    }
}

impl ModelRenderOptions {
    pub(crate) fn normalized(self) -> Self {
        Self {
            normal_mapping: self.normal_mapping,
            normal_y_sign: if self.normal_y_sign < 0.0 { -1.0 } else { 1.0 },
            bloom: self.bloom,
            bloom_strength: self.bloom_strength.clamp(0.0, 2.0),
            uv_scroll_time: if self.uv_scroll_time.is_finite() {
                self.uv_scroll_time
            } else {
                0.0
            },
            vfx_time: if self.vfx_time.is_finite() {
                self.vfx_time
            } else {
                0.0
            },
            vfx_enabled: self.vfx_enabled,
            dynamic_emissive_color: self
                .dynamic_emissive_color
                .map(|value| if value.is_finite() { value } else { 1.0 }),
            debug_mode: self.debug_mode,
            glass_blend_mode: self.glass_blend_mode,
            msaa_samples: self.msaa_samples,
        }
    }

    /// MSAA 采样数归一：仅支持 1（关闭）与 4（4x），其余值就近归入。
    pub fn msaa_samples(self) -> u32 {
        if self.msaa_samples >= 3 { 4 } else { 1 }
    }

    pub(crate) fn bloom_strength(self) -> f32 {
        let normalized = self.normalized();
        if normalized.bloom {
            normalized.bloom_strength
        } else {
            0.0
        }
    }
}

pub type WeaponRenderOptions = ModelRenderOptions;
pub type WeaponRenderer = ModelRenderer;
