use std::collections::HashMap;

use half::f16;
use wgpu::util::DeviceExt;
use xiv_companion_data::MaterialSpecularType;

use crate::{
    MaterialAlphaMode, MaterialDrawDepthMode, MaterialFlowMode, MaterialLightingMode,
    MaterialRenderMode, MaterialShaderFamily, MaterialSkinValueMode, MaterialValueMode,
    ModelMaterial, ModelMeshDrawRole, ModelRenderData, ModelTexture, ModelTextureKind,
    PreparedAlphaSource, PreparedMaterial, PreparedMaterialUnsupportedInputs, PreparedModelOptions,
    PreparedRenderPass, PreparedTextureAddressMode, PreparedTextureColorSpace,
    PreparedTextureFilter, PreparedTextureSampling, PreparedUvSource, material_shader_family,
    model_mesh_vertices_with_shape_mask, prepare_model_for_render_with_options,
};

mod context;
mod instance;
mod material_params;
mod materials;
mod options;
mod pipelines;
mod postprocess;
mod textures;
mod uniforms;

#[cfg(test)]
mod tests;

pub use context::{ModelInstance, ModelRenderContext, ModelRenderer};
pub use options::{
    ModelDebugMode, ModelGlassBlendMode, ModelRenderOptions, WeaponRenderOptions, WeaponRenderer,
};

use context::*;
use instance::*;
use material_params::*;
use materials::*;
use options::*;
use pipelines::*;
use postprocess::*;
use textures::*;
use uniforms::*;
