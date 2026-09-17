use super::*;

pub(crate) fn material_array_params(prepared_material: PreparedMaterial) -> [f32; 4] {
    let tile = prepared_material.resource_availability.tile_array;
    let detail = prepared_material.resource_availability.detail_array;
    let tile_ready = matches!(tile.status, crate::PreparedTextureArrayStatus::Ready);
    let detail_ready = matches!(detail.status, crate::PreparedTextureArrayStatus::Ready);
    [
        tile.layer_count.map(f32::from).unwrap_or(1.0),
        detail.layer_count.map(f32::from).unwrap_or(1.0),
        if tile_ready { 1.0 } else { 0.0 },
        if detail_ready { 1.0 } else { 0.0 },
    ]
}

pub(crate) fn material_tile_lod_params<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    let tile_ready = matches!(
        prepared_material.resource_availability.tile_array.status,
        crate::PreparedTextureArrayStatus::Ready
    );
    let has_packed_color_table = material_baked_color_table_ramps_are_ab(material, model);
    let has_packed_tile = material_baked_color_table_tile_ramps_are_ab(material, model);
    [
        if tile_ready {
            finite_or(material.tile_mip_bias_offset, 0.0).clamp(-16.0, 15.99)
        } else {
            0.0
        },
        if has_packed_color_table { 1.0 } else { 0.0 },
        if has_packed_color_table
            && material
                .shader_package_name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("character.shpk"))
        {
            1.0
        } else {
            0.0
        },
        if has_packed_tile { 1.0 } else { 0.0 },
    ]
}

pub(crate) fn material_baked_color_table_ramps_are_ab<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> bool {
    let has_layout = |index: Option<usize>, expected_kind, expected_layout| {
        index
            .and_then(|index| model.textures().get(index))
            .is_some_and(|texture| {
                texture.kind == expected_kind && texture.texel_layout == expected_layout
            })
    };
    let generic = crate::ModelTextureTexelLayout::ColorTableRampAb;
    // The base ramp lives in `colorset_diffuse_texture` when the shader composes
    // the full-resolution diffuse; otherwise the base texture itself is the ramp.
    let base_ramp = material
        .colorset_diffuse_texture
        .or(material.base_color_texture);
    has_layout(base_ramp, crate::ModelTextureKind::BaseColor, generic)
        && has_layout(
            material.specular_texture,
            crate::ModelTextureKind::Specular,
            generic,
        )
        && has_layout(
            material.material_properties_texture,
            crate::ModelTextureKind::MaterialProperties,
            generic,
        )
        && has_layout(
            material.sheen_properties_texture,
            crate::ModelTextureKind::SheenProperties,
            generic,
        )
        && has_layout(
            material.sphere_properties_texture,
            crate::ModelTextureKind::SphereProperties,
            generic,
        )
}

pub(crate) fn material_colorset_params<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> [f32; 4] {
    let composes_in_shader = material.colorset_diffuse_texture.is_some_and(|index| {
        model.textures().get(index).is_some_and(|texture| {
            texture.kind == crate::ModelTextureKind::BaseColor
                && texture.texel_layout == crate::ModelTextureTexelLayout::ColorTableRampAb
        })
    });
    [if composes_in_shader { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]
}

pub(crate) fn material_baked_color_table_tile_ramps_are_ab<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> bool {
    let has_layout = |index: Option<usize>, expected_kind| {
        index
            .and_then(|index| model.textures().get(index))
            .is_some_and(|texture| {
                texture.kind == expected_kind
                    && texture.texel_layout == crate::ModelTextureTexelLayout::ColorTableTileRampAb
            })
    };
    has_layout(
        material.tile_properties_texture,
        crate::ModelTextureKind::TileProperties,
    ) && has_layout(
        material.tile_matrix_texture,
        crate::ModelTextureKind::TileMatrixProperties,
    )
}

pub(crate) fn model_texture_pair_for_kinds<M: ModelRenderData + ?Sized>(
    model: &M,
    first_kind: ModelTextureKind,
    second_kind: ModelTextureKind,
) -> Option<(&ModelTexture, &ModelTexture)> {
    let first = model
        .textures()
        .iter()
        .find(|texture| texture.kind == first_kind)?;
    let second = model
        .textures()
        .iter()
        .find(|texture| texture.kind == second_kind)?;
    texture_array_pair_is_compatible(first, second).then_some((first, second))
}

pub(crate) fn texture_array_pair_is_compatible(
    first: &ModelTexture,
    second: &ModelTexture,
) -> bool {
    texture_array_layout_is_valid(first)
        && texture_array_layout_is_valid(second)
        && first.width == second.width
        && first.height == second.height
        && first.array_size == second.array_size
        && first.array_layer_height == second.array_layer_height
}

pub(crate) fn texture_array_layout_is_valid(texture: &ModelTexture) -> bool {
    texture.width != 0
        && texture.array_size > 1
        && texture.array_layer_height != 0
        && u32::from(texture.height)
            == u32::from(texture.array_size) * u32::from(texture.array_layer_height)
        && texture.rgba.len() == usize::from(texture.width) * usize::from(texture.height) * 4
}

pub(crate) fn fallback_material() -> ModelMaterial {
    ModelMaterial {
        slot: 0,
        material_index: 0,
        name: "fallback".to_string(),
        path: None,
        reference_fallback: None,
        shader_package_name: None,
        render_mode: MaterialRenderMode::Opaque,
        alpha_mode: MaterialAlphaMode::Opaque,
        alpha_threshold: 0.0,
        draw_depth_mode: MaterialDrawDepthMode::None,
        lighting_mode: MaterialLightingMode::Default,
        specular_type: MaterialSpecularType::Default,
        specular_type_raw: None,
        flow_mode: MaterialFlowMode::Standard,
        value_mode: crate::MaterialValueMode::Single,
        value_mode_raw: None,
        sub_color_mode: crate::MaterialSubColorMode::None,
        decal_color_mode: crate::MaterialDecalColorMode::Off,
        decal_color_mode_raw: None,
        skin_value_mode: crate::MaterialSkinValueMode::None,
        character_scroll_variant: crate::MaterialCharacterScrollVariant::None,
        character_scroll_variant_raw: None,
        transparency: 0.0,
        water_deep_color: [0.3529, 0.372_549, 0.3921, 1.0],
        water_refraction_color: [0.4117, 0.4313, 0.4509, 1.0],
        water_whitecap_color: [0.4509, 0.4705, 0.4901, 0.3],
        alpha_aperture: 2.0,
        alpha_offset: 0.0,
        vertex_alpha_to_one: 0.0,
        shadow_alpha_threshold: 0.5,
        glass_ior: 1.0,
        glass_thickness_max: 0.01,
        normal_scale: 1.0,
        multi_normal_scale: 1.0,
        detail_normal_scale: 1.0,
        multi_detail_normal_scale: 1.0,
        tile_index: 0.0,
        tile_alpha: 1.0,
        tile_scale: [16.0, 16.0],
        toon_index: 0.0,
        toon_light_scale: 2.0,
        toon_light_spec_aperture: 50.0,
        toon_reflection_scale: 2.5,
        toon_spec_index: 4.0e-45,
        sheen_rate: 0.0,
        sheen_tint_rate: 0.0,
        sheen_aperture: 1.0,
        sphere_map_index: 0.0,
        detail_id: 0.0,
        multi_detail_id: 0.0,
        detail_color: [0.5, 0.5, 0.5, 1.0],
        multi_detail_color: [0.5, 0.5, 0.5, 1.0],
        shader_diffuse_color: [1.0, 1.0, 1.0, 1.0],
        shader_multi_diffuse_color: [1.0, 1.0, 1.0, 1.0],
        shader_emissive_color: [0.0, 0.0, 0.0, 1.0],
        shader_multi_emissive_color: [0.0, 0.0, 0.0, 1.0],
        outline_color: [0.0, 0.0, 0.0, 1.0],
        outline_width: 0.0,
        specular_color_mask: [1.0, 1.0, 1.0, 1.0],
        ssao_mask: 1.0,
        ambient_occlusion_mask: None,
        texture_mip_bias: 0.0,
        tile_mip_bias_offset: 0.0,
        shadow_pos_offset: 0.0,
        vertex_movement_scale: 1.0,
        vertex_movement_max_length: 1.0,
        detail_color_uv_scale: [4.0, 4.0, 4.0, 4.0],
        detail_normal_uv_scale: [4.0, 4.0, 4.0, 4.0],
        uv_scroll: [0.0, 0.0, 0.0, 0.0],
        lightshaft_color: [1.0, 1.0, 1.0, 1.0],
        lightshaft_tex_anim: [0.0, 0.0, 0.0, 0.0],
        lightshaft_tex_u: [1.0, 0.0, 0.0, 0.0],
        lightshaft_tex_v: [0.0, 1.0, 0.0, 0.0],
        lightshaft_ray: [0.0, 0.0, 0.0, 0.0],
        lightshaft_type: crate::MaterialLightShaftType::None,
        lightshaft_type_raw: None,
        lightshaft_angle_clip: 0.0,
        lightshaft_near_clip: 0.25,
        opacity: 1.0,
        render_backfaces: true,
        apply_vertex_color: false,
        has_color_dye_table: false,
        color_dye_table: None,
        color_table_rows: None,
        staining_application: None,
        character_colors: None,
        texture_arrays: crate::ModelMaterialTextureArrays::default(),
        fallback_color: [0.78, 0.72, 0.64],
        diffuse_color: [0.78, 0.72, 0.64],
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        colorset_diffuse_texture: None,
        secondary_base_color_texture: None,
        normal_texture: None,
        secondary_normal_texture: None,
        mask_texture: None,
        skin_diffuse_texture: None,
        skin_normal_texture: None,
        skin_mask_texture: None,
        material_map_texture: None,
        multi_map_texture: None,
        specular_texture: None,
        secondary_specular_texture: None,
        emissive_texture: None,
        environment_texture: None,
        material_properties_texture: None,
        tile_properties_texture: None,
        sheen_properties_texture: None,
        sphere_properties_texture: None,
        tile_matrix_texture: None,
        index_texture: None,
        water_wave_texture: None,
        water_wave1_texture: None,
        water_whitecap_texture: None,
    }
}

pub(crate) fn effective_mask_texture(material: &ModelMaterial) -> Option<usize> {
    material.mask_texture
}

pub(crate) fn effective_normal_texture(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> Option<usize> {
    if matches!(prepared_material.shader_family, MaterialShaderFamily::Water) {
        material.water_wave_texture.or(material.normal_texture)
    } else {
        material.normal_texture
    }
}

pub(crate) fn material_extra_texture_flags<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    if prepared_material.feature_flags.uses_secondary_maps {
        return [0.0; 4];
    }
    [
        texture_presence_flag(model, material.tile_properties_texture),
        texture_presence_flag(model, material.sheen_properties_texture),
        texture_presence_flag(model, material.sphere_properties_texture),
        texture_presence_flag(model, material.tile_matrix_texture),
    ]
}

pub(crate) fn texture_presence_flag<M: ModelRenderData + ?Sized>(
    model: &M,
    texture_index: Option<usize>,
) -> f32 {
    texture_index
        .and_then(|index| model.textures().get(index))
        .map(|_| 1.0)
        .unwrap_or(0.0)
}

pub(crate) fn material_water_deep_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.water_deep_color, [0.3529, 0.372_549, 0.3921, 1.0])
}

pub(crate) fn material_water_refraction_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(
        material.water_refraction_color,
        [0.4117, 0.4313, 0.4509, 1.0],
    )
}

pub(crate) fn material_water_whitecap_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.water_whitecap_color, [0.4509, 0.4705, 0.4901, 0.3])
}

pub(crate) fn material_alpha_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.alpha_aperture, 2.0),
        finite_or(material.alpha_offset, 0.0),
        finite_or(material.shadow_alpha_threshold, 0.5).clamp(0.0, 1.0),
        finite_or(material.transparency, 0.0).clamp(0.0, 1.0),
    ]
}

pub(crate) fn material_alpha_policy_params(prepared_material: PreparedMaterial) -> [f32; 4] {
    let source = match prepared_material.alpha_policy.source {
        PreparedAlphaSource::Opaque => 0.0,
        PreparedAlphaSource::BaseColorAlpha => 1.0,
        PreparedAlphaSource::NormalBlue => 2.0,
        PreparedAlphaSource::MaterialTransparency => 3.0,
        PreparedAlphaSource::NormalAlpha => 4.0,
    };
    let pass = match prepared_material.render_pass {
        PreparedRenderPass::Transparent => 1.0,
        PreparedRenderPass::Glass => 2.0,
        _ => 0.0,
    };
    [
        source,
        if prepared_material.alpha_policy.lighting_enabled {
            1.0
        } else {
            0.0
        },
        if matches!(
            prepared_material.alpha_policy.draw_depth_mode,
            MaterialDrawDepthMode::Dither
        ) {
            1.0
        } else {
            0.0
        },
        pass,
    ]
}

pub(crate) fn material_alpha_composition_params(material: &ModelMaterial) -> [f32; 4] {
    let package_consumes_vertex_alpha =
        material.shader_package_name.as_deref().is_some_and(|name| {
            name.eq_ignore_ascii_case("character.shpk")
                || name.eq_ignore_ascii_case("characterlegacy.shpk")
                || name.eq_ignore_ascii_case("characterglass.shpk")
        });
    [
        finite_or(material.vertex_alpha_to_one, 0.0),
        if package_consumes_vertex_alpha {
            1.0
        } else {
            0.0
        },
        0.0,
        0.0,
    ]
}

pub(crate) fn material_shader_params(material: &ModelMaterial) -> [f32; 4] {
    [
        material.normal_scale.clamp(0.0, 4.0),
        material.multi_normal_scale.clamp(0.0, 4.0),
        material.detail_normal_scale.clamp(0.0, 4.0),
        material.multi_detail_normal_scale.clamp(0.0, 4.0),
    ]
}

pub(crate) fn material_tile_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.tile_index, 0.0),
        finite_or(material.tile_alpha, 1.0).clamp(0.0, 1.0),
        finite_or(material.tile_scale[0], 16.0),
        finite_or(material.tile_scale[1], 16.0),
    ]
}

pub(crate) fn material_toon_sheen_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.toon_index, 0.0),
        finite_or(material.toon_light_scale, 2.0),
        finite_or(material.sheen_rate, 0.0),
        finite_or(material.sheen_tint_rate, 0.0),
    ]
}

pub(crate) fn material_toon_params(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    [
        finite_or(material.toon_light_spec_aperture, 50.0),
        finite_or(material.toon_reflection_scale, 2.5),
        finite_or(material.toon_spec_index, 4.0e-45),
        if prepared_material.feature_flags.uses_toon {
            1.0
        } else {
            0.0
        },
    ]
}

pub(crate) fn material_sheen_sphere_params(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    [
        finite_or(material.sheen_aperture, 1.0),
        finite_or(material.sphere_map_index, 0.0),
        if matches!(
            prepared_material.texture_sampling.specular.color_space,
            PreparedTextureColorSpace::Srgb
        ) {
            1.0
        } else {
            0.0
        },
        0.0,
    ]
}

pub(crate) fn material_detail_params(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    let uses_multi_blend = matches!(prepared_material.value_mode, MaterialValueMode::Multi)
        && matches!(
            prepared_material.shader_family,
            MaterialShaderFamily::Bg | MaterialShaderFamily::BgUvScroll
        );
    [
        finite_or(material.detail_id, 0.0),
        finite_or(material.multi_detail_id, 0.0),
        if uses_multi_blend { 1.0 } else { 0.0 },
        0.0,
    ]
}

pub(crate) fn material_detail_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_color, [0.5, 0.5, 0.5, 1.0])
}

pub(crate) fn material_multi_detail_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.multi_detail_color, [0.5, 0.5, 0.5, 1.0])
}

pub(crate) fn material_shader_diffuse_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_diffuse_color, [1.0; 4])
}

pub(crate) fn material_shader_multi_diffuse_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_multi_diffuse_color, [1.0; 4])
}

pub(crate) fn material_shader_emissive_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_emissive_color, [0.0, 0.0, 0.0, 1.0])
}

pub(crate) fn material_shader_multi_emissive_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_multi_emissive_color, [0.0, 0.0, 0.0, 1.0])
}

pub(crate) fn material_outline_params(material: &ModelMaterial) -> [f32; 4] {
    let color = finite_vec4_or(material.outline_color, [0.0, 0.0, 0.0, 1.0]);
    [
        color[0],
        color[1],
        color[2],
        finite_or(material.outline_width, 0.0),
    ]
}

pub(crate) fn material_specular_color_mask(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.specular_color_mask, [1.0; 4])
}

pub(crate) fn material_legacy_specular_mode(material: &ModelMaterial) -> f32 {
    let uses_legacy_compatibility = material_is_character_legacy(material)
        && matches!(material.value_mode, crate::MaterialValueMode::Compatibility);
    if !uses_legacy_compatibility {
        return 0.0;
    }
    match material.specular_type {
        MaterialSpecularType::Default => 1.0,
        MaterialSpecularType::Mask => 2.0,
        MaterialSpecularType::Unknown => 0.0,
    }
}

pub(crate) fn material_is_character_legacy(material: &ModelMaterial) -> bool {
    material
        .shader_package_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("characterlegacy.shpk"))
}

/// 皮肤族材质（skin/hair/iris/charactertattoo/characterocclusion）：皮肤与毛发
/// 是介电质，mask 通道语义为族别数据（皮肤 B≈次表面强度、毛发 R=明暗渐变/
/// G=挑染区域），不能把 mask.b 当金属度（敖龙 mask.b≈0.61 渲染成铜金属实证）。
pub(crate) fn material_is_skin_family(material: &ModelMaterial) -> bool {
    matches!(
        material_shader_family(material.shader_package_name.as_deref()),
        MaterialShaderFamily::Skin
            | MaterialShaderFamily::Hair
            | MaterialShaderFamily::Iris
            | MaterialShaderFamily::CharacterTattoo
            | MaterialShaderFamily::CharacterOcclusion
    )
}

/// hair.shpk：family_params.w 标记。头发 alpha clip 不走固定阈值硬裁（发丝
/// 边缘渐变带会被整片削碎），WGSL 按 4x4 screen-door 抖动近似
/// alpha-to-coverage（软边缘、不透头皮）。
pub(crate) fn material_is_hair_family(material: &ModelMaterial) -> bool {
    matches!(
        material_shader_family(material.shader_package_name.as_deref()),
        MaterialShaderFamily::Hair
    )
}

pub(crate) fn material_has_character_colortable_final(material: &ModelMaterial) -> bool {
    material.color_table_rows.is_some()
        && material.shader_package_name.as_deref().is_some_and(|name| {
            name.eq_ignore_ascii_case("character.shpk")
                || name.eq_ignore_ascii_case("characterlegacy.shpk")
        })
}

pub(crate) fn material_surface_params(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    let uses_texture_mip_bias = matches!(
        prepared_material.shader_family,
        MaterialShaderFamily::Character
            | MaterialShaderFamily::CharacterStockings
            | MaterialShaderFamily::CharacterGlass
            | MaterialShaderFamily::CharacterReflection
            | MaterialShaderFamily::CharacterTransparency
            | MaterialShaderFamily::CharacterScroll
            | MaterialShaderFamily::CharacterTattoo
            | MaterialShaderFamily::CharacterOcclusion
    );
    [
        finite_or(material.ssao_mask, 1.0),
        finite_or(material.texture_mip_bias, 0.0),
        finite_or(material.shadow_pos_offset, 0.0),
        if uses_texture_mip_bias { 1.0 } else { 0.0 },
    ]
}

pub(crate) fn material_detail_color_uv_scale(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_color_uv_scale, [4.0; 4])
}

pub(crate) fn material_detail_normal_uv_scale(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_normal_uv_scale, [4.0; 4])
}

pub(crate) fn material_uv_scroll(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.uv_scroll, [0.0; 4])
}

pub(crate) fn material_lightshaft_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_color, [1.0; 4])
}

pub(crate) fn material_lightshaft_tex_anim(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_anim, [0.0; 4])
}

pub(crate) fn material_lightshaft_tex_u(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_u, [1.0, 0.0, 0.0, 0.0])
}

pub(crate) fn material_lightshaft_tex_v(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_v, [0.0, 1.0, 0.0, 0.0])
}

pub(crate) fn material_lightshaft_ray(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_ray, [0.0; 4])
}

pub(crate) fn material_uv_source_params(
    prepared_material: PreparedMaterial,
) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    let uv_sources = prepared_material.uv_sources.textures;
    (
        [
            prepared_uv_source_value(uv_sources.base_color),
            prepared_uv_source_value(uv_sources.normal),
            prepared_uv_source_value(uv_sources.mask),
            prepared_uv_source_value(uv_sources.material_map),
        ],
        [
            prepared_uv_source_value(uv_sources.multi_map),
            prepared_uv_source_value(uv_sources.specular),
            prepared_uv_source_value(uv_sources.emissive),
            prepared_uv_source_value(uv_sources.material_properties),
        ],
        if prepared_material.feature_flags.uses_secondary_maps {
            [
                prepared_uv_source_value(uv_sources.secondary_base_color),
                prepared_uv_source_value(uv_sources.secondary_normal),
                prepared_uv_source_value(uv_sources.secondary_specular),
                prepared_uv_source_value(uv_sources.tile_matrix),
            ]
        } else {
            [
                prepared_uv_source_value(uv_sources.tile_properties),
                prepared_uv_source_value(uv_sources.sheen_properties),
                prepared_uv_source_value(uv_sources.sphere_properties),
                prepared_uv_source_value(uv_sources.tile_matrix),
            ]
        },
        [
            prepared_uv_source_value(uv_sources.index),
            prepared_uv_source_value(uv_sources.other),
            0.0,
            0.0,
        ],
    )
}

pub(crate) fn material_uv_scroll_mask_params(
    prepared_material: PreparedMaterial,
) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    let scroll = prepared_material.uv_sources.scroll;
    let value = |enabled| if enabled { 1.0 } else { 0.0 };
    (
        [
            value(scroll.base_color),
            value(scroll.normal),
            value(scroll.mask),
            value(scroll.material_map),
        ],
        [
            value(scroll.multi_map),
            value(scroll.specular),
            value(scroll.emissive),
            value(scroll.material_properties),
        ],
        if prepared_material.feature_flags.uses_secondary_maps {
            [
                value(scroll.secondary_base_color),
                value(scroll.secondary_normal),
                value(scroll.secondary_specular),
                value(scroll.tile_matrix),
            ]
        } else {
            [
                value(scroll.tile_properties),
                value(scroll.sheen_properties),
                value(scroll.sphere_properties),
                value(scroll.tile_matrix),
            ]
        },
        [value(scroll.index), value(scroll.other), 0.0, 0.0],
    )
}

pub(crate) fn material_feature_params(prepared_material: PreparedMaterial) -> [f32; 4] {
    [
        if prepared_material.feature_flags.uses_flow {
            1.0
        } else {
            0.0
        },
        if matches!(prepared_material.shader_family, MaterialShaderFamily::Water) {
            1.0
        } else {
            0.0
        },
        if prepared_material.feature_flags.uses_secondary_maps {
            1.0
        } else {
            0.0
        },
        if matches!(
            prepared_material.shader_family,
            MaterialShaderFamily::Bg | MaterialShaderFamily::BgUvScroll
        ) {
            1.0
        } else {
            0.0
        },
    ]
}

pub(crate) fn material_family_params(material: &ModelMaterial) -> [f32; 4] {
    [
        if material_is_character_legacy(material) {
            1.0
        } else {
            0.0
        },
        if material_has_character_colortable_final(material) {
            1.0
        } else {
            0.0
        },
        if material_is_skin_family(material) {
            1.0
        } else {
            0.0
        },
        if material_is_hair_family(material) {
            1.0
        } else {
            0.0
        },
    ]
}

/// 角色颜色 uniform 通道（MaterialUniform 尾部 9 个 vec4）。逐材质按 shader
/// family 从 `ModelMaterial::character_colors` 填充：skin 全收（肤色恒激活；
/// 唇妆按 lipstick 开关；面妆按 decal 贴图存在性），hair 收发色/挑染（挑染按
/// highlights 开关），iris 收左右眼色，charactertattoo 收特征色；其余 family
/// 或无 character_colors 时全零（WGSL 分支不激活，武器/装备渲染不受影响）。
/// RGB 为 squared RGB 线性色（数据侧已定），W 为激活/强度标记。
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct CharacterColorChannels {
    pub(crate) skin: [f32; 4],
    pub(crate) lip: [f32; 4],
    pub(crate) main: [f32; 4],
    pub(crate) mesh: [f32; 4],
    pub(crate) left_iris: [f32; 4],
    pub(crate) right_iris: [f32; 4],
    pub(crate) option: [f32; 4],
    pub(crate) decal: [f32; 4],
    pub(crate) decal_uv: [f32; 4],
    pub(crate) params: [f32; 4],
}

pub(crate) fn material_character_color_channels(
    material: &ModelMaterial,
) -> CharacterColorChannels {
    let mut channels = CharacterColorChannels::default();
    let Some(character) = material.character_colors.as_ref() else {
        return channels;
    };
    let colors = &character.colors;
    let with_flag = |color: [f32; 4], active: f32| [color[0], color[1], color[2], active];
    match material_shader_family(material.shader_package_name.as_deref()) {
        MaterialShaderFamily::Skin => {
            channels.skin = with_flag(colors.skin, 1.0);
            // LipColor 只落在脸材质（唇形遮罩只在脸漫反射 alpha 上有意义）；
            // GetMaterialValue=Face 或路径在 obj/face 下的 skin 材质视为脸变体。
            let is_face_material = material.skin_value_mode == MaterialSkinValueMode::Face
                || material
                    .path
                    .as_deref()
                    .is_some_and(|path| path.contains("/obj/face/"));
            if colors.lipstick && is_face_material {
                channels.lip = colors.lip;
            }
            if character.decal_texture.is_some() {
                channels.decal = colors.decal;
                // FacePaintUVMultiplier/Offset 是运行态 cbuffer 值（无离线来源），
                // 按恒等 UV 处理；reversed 仅透传标记。
                channels.decal_uv = [
                    1.0,
                    0.0,
                    if colors.face_paint_reversed { 1.0 } else { 0.0 },
                    1.0,
                ];
            }
        }
        MaterialShaderFamily::Hair => {
            channels.main = with_flag(colors.main, 1.0);
            if colors.highlights {
                channels.mesh = with_flag(colors.mesh, 1.0);
            }
            // 头发/尾/兔耳的 mask R 通道是发丝明暗渐变（AO）；脸部 hair 材质
            // （眉毛/睫毛，obj/face 下）的 mask R 同样是明暗细节图（眉发丝纹理
            // + 睫毛区中灰压暗，真实纹理解码），一并作明暗细节——脸部毛发只取
            // 发色平色会亮成白粉点（敖龙女睫毛实证）。
            channels.params = [1.0, 0.0, 0.0, 0.0];
        }
        MaterialShaderFamily::Iris => {
            channels.left_iris = colors.left_iris;
            channels.right_iris = colors.right_iris;
        }
        MaterialShaderFamily::CharacterTattoo => {
            channels.option = with_flag(colors.option, 1.0);
        }
        _ => {}
    }
    channels
}

pub(crate) fn material_secondary_map_params<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
    prepared_material: PreparedMaterial,
) -> [f32; 4] {
    if matches!(
        prepared_material.shader_family,
        MaterialShaderFamily::LightShaft
    ) {
        return [
            texture_presence_flag(model, material.secondary_base_color_texture),
            0.0,
            0.0,
            0.0,
        ];
    }
    if !prepared_material.feature_flags.uses_secondary_maps {
        return [0.0; 4];
    }
    [
        texture_presence_flag(model, material.secondary_base_color_texture),
        texture_presence_flag(model, material.secondary_normal_texture),
        texture_presence_flag(model, material.secondary_specular_texture),
        1.0,
    ]
}

pub(crate) fn prepared_uv_source_value(source: PreparedUvSource) -> f32 {
    match source {
        PreparedUvSource::Uv0 => 0.0,
        PreparedUvSource::Uv1 => 1.0,
        PreparedUvSource::Uv2 => 2.0,
        PreparedUvSource::Uv3 => 3.0,
    }
}

pub(crate) fn draw_role_debug_color(draw_role: ModelMeshDrawRole) -> [f32; 4] {
    match draw_role {
        ModelMeshDrawRole::Normal => [0.16, 0.72, 1.0, 1.0],
        ModelMeshDrawRole::Glass => [0.66, 0.92, 1.0, 1.0],
        ModelMeshDrawRole::LightShaft => [1.0, 0.82, 0.22, 1.0],
        ModelMeshDrawRole::ShadowOnly => [0.18, 0.18, 0.22, 1.0],
        ModelMeshDrawRole::Ignored => [0.55, 0.55, 0.55, 1.0],
        ModelMeshDrawRole::MaterialChange => [1.0, 0.34, 0.76, 1.0],
        ModelMeshDrawRole::CrestChange => [1.0, 0.62, 0.2, 1.0],
    }
}

/// Diagnostic color for the `UnsupportedInputs` debug view. Known-incomplete
/// shader families and other unsupported inputs get distinct hues so silently
/// approximated materials become visible instead of passing as fully
/// supported. Priority goes to the visible weapon families (lightshaft,
/// crystal environment maps, character reflection/scroll/glass), then any
/// other known-incomplete family, then material-semantics gaps with installed
/// coverage (AlphaMulti, unverified vertex-color/Toon/Sheen/Sphere lighting,
/// AO/SSAO, legacy specular, tile LOD, vertex movement), then any remaining
/// runtime-only input.
pub(crate) fn unsupported_inputs_diagnostic_color(prepared_material: PreparedMaterial) -> [f32; 4] {
    let unsupported = prepared_material.unsupported_inputs;
    let rgb = if unsupported.lightshaft_clip {
        [1.0, 0.82, 0.2]
    } else if unsupported.environment_mapping {
        [0.25, 0.5, 1.0]
    } else if unsupported.character_reflection {
        [1.0, 0.3, 0.75]
    } else if unsupported.character_scroll_variant {
        [1.0, 0.55, 0.1]
    } else if unsupported.glass_shader_parameters {
        [0.15, 0.85, 0.9]
    } else if unsupported.incomplete_shader_family_logic {
        [0.95, 0.2, 0.2]
    } else if unsupported.alpha_multi_values {
        [0.68, 0.28, 1.0]
    } else if unsupported.multi_color_composition {
        [0.92, 0.36, 0.16]
    } else if unsupported.detail_composition {
        [0.72, 0.52, 0.96]
    } else if unsupported.alpha_shaping {
        [0.94, 0.44, 0.78]
    } else if unsupported.vertex_color_composition {
        [0.18, 0.76, 0.54]
    } else if unsupported.specular_color_mask_composition {
        [0.84, 0.28, 0.62]
    } else if unsupported.outline_composition {
        [0.46, 0.82, 0.94]
    } else if unsupported.toon_lighting {
        [0.88, 0.64, 0.22]
    } else if unsupported.sheen_lighting {
        [0.95, 0.48, 0.62]
    } else if unsupported.sphere_lighting {
        [0.34, 0.7, 0.92]
    } else if unsupported.ambient_occlusion_mask || unsupported.ssao_mask {
        [0.58, 0.72, 0.16]
    } else if unsupported.legacy_gloss_composition {
        [0.76, 0.3, 0.9]
    } else if unsupported.legacy_specular_type {
        [0.58, 0.38, 0.88]
    } else if unsupported.tile_mip_bias_offset {
        [0.78, 0.4, 0.12]
    } else if unsupported.vertex_movement_parameters {
        [0.32, 0.88, 0.28]
    } else if has_any_unsupported_inputs(unsupported) {
        [0.6, 0.6, 0.6]
    } else {
        [0.05, 0.22, 0.1]
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

pub(crate) fn has_any_unsupported_inputs(unsupported: PreparedMaterialUnsupportedInputs) -> bool {
    unsupported.dye_application
        || unsupported.runtime_color_table
        || unsupported.decal_or_crest
        || unsupported.runtime_material_change
        || unsupported.runtime_option_color
        || unsupported.runtime_decal_color
        || unsupported.runtime_decal_texture
        || unsupported.runtime_skin_color
        || unsupported.runtime_skin_material
        || unsupported.skin_sampler_composition
        || unsupported.runtime_sub_color
        || unsupported.tile_array
        || unsupported.detail_array
        || unsupported.detail_composition
        || unsupported.secondary_map_blend
        || unsupported.multi_map_interpretation
        || unsupported.multi_color_composition
        || unsupported.ambient_occlusion_mask
        || unsupported.ssao_mask
        || unsupported.sheen_lighting
        || unsupported.sphere_lighting
        || unsupported.toon_lighting
        || unsupported.decal_color_mode
        || unsupported.alpha_multi_values
        || unsupported.alpha_shaping
        || unsupported.vertex_color_composition
        || unsupported.specular_color_mask_composition
        || unsupported.outline_composition
        || unsupported.legacy_gloss_composition
        || unsupported.legacy_specular_type
        || unsupported.tile_mip_bias_offset
        || unsupported.vertex_movement_parameters
}

pub(crate) fn draw_role_params(draw_role: ModelMeshDrawRole) -> [f32; 4] {
    [
        if matches!(draw_role, ModelMeshDrawRole::LightShaft) {
            1.0
        } else {
            0.0
        },
        if matches!(draw_role, ModelMeshDrawRole::CrestChange) {
            1.0
        } else {
            0.0
        },
        if matches!(draw_role, ModelMeshDrawRole::MaterialChange) {
            1.0
        } else {
            0.0
        },
        0.0,
    ]
}

pub(crate) fn finite_vec4_or(values: [f32; 4], default: [f32; 4]) -> [f32; 4] {
    let mut resolved = default;
    for (target, value) in resolved.iter_mut().zip(values) {
        if value.is_finite() {
            *target = value;
        }
    }
    resolved
}

pub(crate) fn finite_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

pub(crate) fn render_mode_value(mode: MaterialRenderMode) -> f32 {
    match mode {
        MaterialRenderMode::Opaque => 0.0,
        MaterialRenderMode::Transparent => 1.0,
        MaterialRenderMode::Glass => 2.0,
    }
}

pub(crate) fn alpha_mode_value(mode: MaterialAlphaMode) -> f32 {
    match mode {
        MaterialAlphaMode::Opaque => 0.0,
        MaterialAlphaMode::Mask => 1.0,
        MaterialAlphaMode::Blend => 2.0,
        MaterialAlphaMode::Glass => 3.0,
    }
}

/// render.z 编码：hair.shpk 的 Blend 按 Mask（alpha clip）编码——配合数据层
/// 同规则（hair Blend → Cutout pass），使 WGSL 按 MTRL alpha_threshold 丢弃
/// 发丝间隙（true blend 会透出头皮鳞片/皮肤纹理，敖龙前额鳞片透出实证）。
pub(crate) fn alpha_mode_value_for_pass(
    material: &ModelMaterial,
    prepared_material: PreparedMaterial,
) -> f32 {
    if matches!(prepared_material.shader_family, MaterialShaderFamily::Hair)
        && matches!(material.alpha_mode, MaterialAlphaMode::Blend)
    {
        return alpha_mode_value(MaterialAlphaMode::Mask);
    }
    alpha_mode_value(material.alpha_mode)
}
