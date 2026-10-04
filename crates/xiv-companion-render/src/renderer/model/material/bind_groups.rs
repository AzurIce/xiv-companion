use super::*;

/// GPU texture cache shared by every draw batch of one model. Batches sharing a
/// material would otherwise each upload their own copy of every bound texture
/// (including full mip chains), multiplying VRAM usage by the batch count and
/// exhausting GPU memory on models with many submeshes. The key carries the
/// `model.textures()` index plus every parameter that affects the created
/// texture, so the same source bound under a different semantic stays a
/// distinct entry; per batch only the cheap `TextureView` is created.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MaterialTextureKey {
    /// `create_mipped_rgba_texture` uploaded from `model.textures()[index]`.
    Mipped {
        index: usize,
        semantic: RgbaMipSemantic,
    },
    /// 1×1 constant `create_mipped_rgba_texture` fallback.
    MippedFallback {
        rgba: [u8; 4],
        semantic: RgbaMipSemantic,
    },
    /// `create_rgba_texture` uploaded from `model.textures()[index]`.
    Rgba {
        index: usize,
        format: wgpu::TextureFormat,
    },
    /// 1×1 constant `create_rgba_texture` fallback.
    RgbaFallback {
        rgba: [u8; 4],
        format: wgpu::TextureFormat,
    },
    /// `create_float_ramp_texture` sourced from `model.textures()[index]`;
    /// `neutral` is the per-channel fallback color as `f32::to_bits`.
    FloatRamp { index: usize, neutral: [u32; 4] },
    /// Neutral-only `create_float_ramp_texture` without a usable source.
    FloatRampFallback { neutral: [u32; 4] },
    /// `create_tile_matrix_texture` sourced from `model.textures()[index]`;
    /// `None` creates the neutral identity matrix.
    TileMatrix { index: Option<usize> },
}

pub(crate) type MaterialTextureCache = HashMap<MaterialTextureKey, wgpu::Texture>;

pub(crate) fn cached_material_texture<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    textures: &[ModelTexture],
    cache: &'a mut MaterialTextureCache,
    key: MaterialTextureKey,
    label: &str,
) -> &'a wgpu::Texture {
    cache.entry(key).or_insert_with(|| match key {
        MaterialTextureKey::Mipped { index, semantic } => {
            let texture = &textures[index];
            create_mipped_rgba_texture(
                device,
                queue,
                label,
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                semantic,
            )
        }
        MaterialTextureKey::MippedFallback { rgba, semantic } => {
            create_mipped_rgba_texture(device, queue, label, 1, 1, &rgba, semantic)
        }
        MaterialTextureKey::Rgba { index, format } => {
            let texture = &textures[index];
            create_rgba_texture(
                device,
                queue,
                label,
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                format,
            )
        }
        MaterialTextureKey::RgbaFallback { rgba, format } => {
            create_rgba_texture(device, queue, label, 1, 1, &rgba, format)
        }
        MaterialTextureKey::FloatRamp { index, neutral } => create_float_ramp_texture(
            device,
            queue,
            label,
            Some(&textures[index]),
            neutral.map(f32::from_bits),
        ),
        MaterialTextureKey::FloatRampFallback { neutral } => {
            create_float_ramp_texture(device, queue, label, None, neutral.map(f32::from_bits))
        }
        MaterialTextureKey::TileMatrix { index } => {
            create_tile_matrix_texture(device, queue, label, index.map(|index| &textures[index]))
        }
    })
}

pub(crate) fn create_material_bind_groups<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    overlay_layout: &wgpu::BindGroupLayout,
    model: &M,
    draw_batches: &[DrawBatch],
) -> (Vec<wgpu::BindGroup>, Vec<wgpu::BindGroup>) {
    // Pair related atlases horizontally to stay below common WebGPU per-stage texture limits.
    let tile_array_pair_texture = create_array_pair_texture(
        device,
        queue,
        "weapon tile array pair",
        model_texture_pair_for_kinds(
            model,
            ModelTextureKind::TileNormalArray,
            ModelTextureKind::TileOrbArray,
        ),
        RgbaMipSemantic::PackedNormalRg,
        RgbaMipSemantic::LinearData,
        [128, 128, 255, 255],
        [255, 128, 255, 255],
    );
    let tile_array_pair_view =
        tile_array_pair_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let detail_array_pair_texture = create_array_pair_texture(
        device,
        queue,
        "weapon detail array pair",
        model_texture_pair_for_kinds(
            model,
            ModelTextureKind::DetailDiffuseArray,
            ModelTextureKind::DetailNormalArray,
        ),
        RgbaMipSemantic::LinearData,
        RgbaMipSemantic::PackedNormalRg,
        [128, 128, 128, 255],
        [128, 128, 255, 255],
    );
    let detail_array_pair_view =
        detail_array_pair_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Batches typically share materials; cache the uploaded GPU textures across
    // the whole batch loop so each distinct source/semantic combination is
    // created (and uploaded) only once per model.
    let mut texture_cache = MaterialTextureCache::new();
    let overlay_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("weapon default surface overlay uniform"),
        contents: bytemuck::bytes_of(&SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    draw_batches
        .iter()
        .map(|batch| {
            let fallback = fallback_material();
            let material = model
                .materials()
                .get(batch.material_slot)
                .unwrap_or(&fallback);
            create_material_bind_group(
                device,
                queue,
                layout,
                overlay_layout,
                &overlay_uniform,
                material,
                model,
                batch.prepared_material,
                batch.draw_role,
                &tile_array_pair_view,
                &detail_array_pair_view,
                &mut texture_cache,
            )
        })
        .collect()
}

pub(crate) fn create_material_bind_group<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    overlay_layout: &wgpu::BindGroupLayout,
    overlay_uniform: &wgpu::Buffer,
    material: &ModelMaterial,
    model: &M,
    prepared_material: PreparedMaterial,
    draw_role: ModelMeshDrawRole,
    tile_array_pair_view: &wgpu::TextureView,
    detail_array_pair_view: &wgpu::TextureView,
    texture_cache: &mut MaterialTextureCache,
) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let effective_mask_texture = effective_mask_texture(material);
    let effective_normal_texture = effective_normal_texture(material, prepared_material);
    let uv_sources = material_uv_source_params(prepared_material);
    let uv_scroll_masks = material_uv_scroll_mask_params(prepared_material);
    let sheen_sphere_params = material_sheen_sphere_params(material, prepared_material);
    let character_channels = material_character_color_channels(material);
    let iris_params = material_iris_params(material, prepared_material);
    let uv_scales = material_uv_scales(material, prepared_material);
    let uniform = MaterialUniform {
        diffuse_color: [
            material.diffuse_color[0],
            material.diffuse_color[1],
            material.diffuse_color[2],
            material.opacity.clamp(0.0, 1.0),
        ],
        emissive_color: [
            material.emissive_color[0],
            material.emissive_color[1],
            material.emissive_color[2],
            material
                .emissive_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        specular_color: [
            material.specular_color[0],
            material.specular_color[1],
            material.specular_color[2],
            material.roughness,
        ],
        params: [
            material
                .base_color_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material.metalness,
            effective_normal_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            effective_mask_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        properties: [
            material
                .material_properties_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material
                .specular_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            if material.apply_vertex_color {
                1.0
            } else {
                0.0
            },
            material_legacy_specular_mode(material),
        ],
        render: [
            render_mode_value(material.render_mode),
            material.opacity,
            alpha_mode_value_for_pass(material, prepared_material),
            material.alpha_threshold.clamp(0.0, 1.0),
        ],
        alpha_params: material_alpha_params(material),
        alpha_policy_params: material_alpha_policy_params(prepared_material),
        alpha_composition_params: material_alpha_composition_params(material),
        water_deep_color: material_water_deep_color(material),
        water_refraction_color: material_water_refraction_color(material),
        water_whitecap_color: material_water_whitecap_color(material),
        extra_properties: material_extra_texture_flags(material, model, prepared_material),
        shader_params: material_shader_params(material),
        tile_params: material_tile_params(material),
        toon_sheen_params: material_toon_sheen_params(material),
        toon_params: material_toon_params(material, prepared_material),
        sheen_sphere_params,
        detail_params: material_detail_params(material, prepared_material),
        array_params: material_array_params(prepared_material),
        tile_lod_params: material_tile_lod_params(material, model, prepared_material),
        colorset_params: material_colorset_params(material, model),
        detail_color: material_detail_color(material),
        multi_detail_color: material_multi_detail_color(material),
        shader_diffuse_color: material_shader_diffuse_color(material),
        shader_multi_diffuse_color: material_shader_multi_diffuse_color(material),
        shader_emissive_color: material_shader_emissive_color(material),
        shader_multi_emissive_color: material_shader_multi_emissive_color(material),
        outline_params: material_outline_params(material),
        specular_color_mask: material_specular_color_mask(material),
        surface_params: material_surface_params(material, prepared_material),
        detail_color_uv_scale: material_detail_color_uv_scale(material),
        detail_normal_uv_scale: material_detail_normal_uv_scale(material),
        uv_scroll: material_uv_scroll(material),
        lightshaft_color: material_lightshaft_color(material),
        lightshaft_tex_anim: material_lightshaft_tex_anim(material),
        lightshaft_tex_u: material_lightshaft_tex_u(material),
        lightshaft_tex_v: material_lightshaft_tex_v(material),
        lightshaft_ray: material_lightshaft_ray(material),
        uv_sources0: uv_sources.0,
        uv_sources1: uv_sources.1,
        uv_sources2: uv_sources.2,
        uv_sources3: uv_sources.3,
        uv_scroll_masks0: uv_scroll_masks.0,
        uv_scroll_masks1: uv_scroll_masks.1,
        uv_scroll_masks2: uv_scroll_masks.2,
        uv_scroll_masks3: uv_scroll_masks.3,
        feature_params: material_feature_params(prepared_material),
        family_params: material_family_params(material),
        secondary_map_params: material_secondary_map_params(material, model, prepared_material),
        draw_role_params: draw_role_params(draw_role),
        debug_color: draw_role_debug_color(draw_role),
        unsupported_color: unsupported_inputs_diagnostic_color(prepared_material),
        character_skin: character_channels.skin,
        character_lip: character_channels.lip,
        character_main: character_channels.main,
        character_mesh: character_channels.mesh,
        character_left_iris: character_channels.left_iris,
        character_right_iris: character_channels.right_iris,
        character_option: character_channels.option,
        character_decal: character_channels.decal,
        character_decal_uv: character_channels.decal_uv,
        character_params: character_channels.params,
        uv_scale_a: uv_scales.0,
        uv_scale_b: uv_scales.1,
        uv_scale_c: uv_scales.2,
        iris_white_eye: iris_params.0,
        iris_ring_color: iris_params.1,
        iris_ring_a: iris_params.2,
        iris_ring_b: iris_params.3,
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("weapon material uniform"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let (base_color_key, base_color_label) = material
        .base_color_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            let label = format!("weapon texture {}", texture.path);
            if texture.rgba_f32.is_some() {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [1.0; 4].map(f32::to_bits),
                    },
                    label,
                )
            } else {
                (
                    MaterialTextureKey::Mipped {
                        index,
                        semantic: mip_semantic_for_color_space(
                            prepared_material.texture_sampling.base_color.color_space,
                        ),
                    },
                    label,
                )
            }
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [255, 255, 255, 255],
                    semantic: RgbaMipSemantic::SrgbColor,
                },
                "weapon white texture".to_string(),
            )
        });
    let texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        base_color_key,
        &base_color_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (mask_key, mask_label) = effective_mask_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Mipped {
                    index,
                    semantic: RgbaMipSemantic::LinearData,
                },
                format!("weapon mask texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [255, 128, 0, 255],
                    semantic: RgbaMipSemantic::LinearData,
                },
                "weapon neutral mask texture".to_string(),
            )
        });
    let mask_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        mask_key,
        &mask_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (emissive_key, emissive_label) = material
        .emissive_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            let label = format!("weapon emissive texture {}", texture.path);
            if texture.rgba_f32.is_some() {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
                    },
                    label,
                )
            } else {
                (
                    MaterialTextureKey::Mipped {
                        index,
                        semantic: mip_semantic_for_color_space(
                            prepared_material.texture_sampling.emissive.color_space,
                        ),
                    },
                    label,
                )
            }
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [0, 0, 0, 255],
                    semantic: RgbaMipSemantic::SrgbColor,
                },
                "weapon black emissive texture".to_string(),
            )
        });
    let emissive_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        emissive_key,
        &emissive_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (normal_key, normal_label) = effective_normal_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Mipped {
                    index,
                    semantic: RgbaMipSemantic::PackedNormalRg,
                },
                format!("weapon normal texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [128, 128, 255, 255],
                    semantic: RgbaMipSemantic::PackedNormalRg,
                },
                "weapon flat normal texture".to_string(),
            )
        });
    let normal_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        normal_key,
        &normal_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (material_properties_key, material_properties_label) = material
        .material_properties_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            let label = format!("weapon material properties texture {}", texture.path);
            if texture.rgba_f32.is_some() {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [0.0, 1.0, 1.0, 1.0].map(f32::to_bits),
                    },
                    label,
                )
            } else {
                (
                    MaterialTextureKey::Mipped {
                        index,
                        semantic: RgbaMipSemantic::LinearData,
                    },
                    label,
                )
            }
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [
                        unorm_byte(material.metalness),
                        unorm_byte(material.roughness),
                        255,
                        255,
                    ],
                    semantic: RgbaMipSemantic::LinearData,
                },
                "weapon neutral material properties texture".to_string(),
            )
        });
    let material_properties_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        material_properties_key,
        &material_properties_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (specular_key, specular_label) = material
        .specular_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            let label = format!("weapon specular texture {}", texture.path);
            if texture.rgba_f32.is_some() {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [1.0; 4].map(f32::to_bits),
                    },
                    label,
                )
            } else {
                (
                    MaterialTextureKey::Mipped {
                        index,
                        semantic: mip_semantic_for_color_space(
                            prepared_material.texture_sampling.specular.color_space,
                        ),
                    },
                    label,
                )
            }
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [
                        unorm_byte(material.specular_color[0]),
                        unorm_byte(material.specular_color[1]),
                        unorm_byte(material.specular_color[2]),
                        255,
                    ],
                    semantic: RgbaMipSemantic::LinearData,
                },
                "weapon neutral specular texture".to_string(),
            )
        });
    let specular_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        specular_key,
        &specular_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let uses_secondary_maps = prepared_material.feature_flags.uses_secondary_maps;
    let tile_binding_texture = if uses_secondary_maps {
        material.secondary_base_color_texture
    } else {
        material.tile_properties_texture
    };
    let tile_binding_format = texture_format_for_color_space(if uses_secondary_maps {
        prepared_material
            .texture_sampling
            .secondary_base_color
            .color_space
    } else {
        prepared_material
            .texture_sampling
            .tile_properties
            .color_space
    });
    let (tile_key, tile_label) = tile_binding_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Rgba {
                    index,
                    format: tile_binding_format,
                },
                format!("weapon tile/secondary color texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::RgbaFallback {
                    rgba: if uses_secondary_maps {
                        [255, 255, 255, 255]
                    } else {
                        [0, 255, 255, 255]
                    },
                    format: tile_binding_format,
                },
                "weapon neutral tile/secondary color texture".to_string(),
            )
        });
    let tile_properties_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        tile_key,
        &tile_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let sheen_binding_texture = if uses_secondary_maps {
        material.secondary_normal_texture
    } else {
        material.sheen_properties_texture
    };
    let sheen_binding_index =
        sheen_binding_texture.filter(|&index| model.textures().get(index).is_some());
    let (sheen_key, sheen_label) = if uses_secondary_maps {
        sheen_binding_index.map_or_else(
            || {
                (
                    MaterialTextureKey::RgbaFallback {
                        rgba: [128, 128, 255, 255],
                        format: wgpu::TextureFormat::Rgba8Unorm,
                    },
                    "weapon neutral secondary normal texture".to_string(),
                )
            },
            |index| {
                (
                    MaterialTextureKey::Rgba {
                        index,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                    },
                    format!(
                        "weapon secondary normal texture {}",
                        model.textures()[index].path
                    ),
                )
            },
        )
    } else {
        sheen_binding_index.map_or_else(
            || {
                (
                    MaterialTextureKey::FloatRampFallback {
                        neutral: [0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
                    },
                    "weapon neutral sheen texture".to_string(),
                )
            },
            |index| {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
                    },
                    format!("weapon sheen texture {}", model.textures()[index].path),
                )
            },
        )
    };
    let sheen_properties_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        sheen_key,
        &sheen_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let sphere_binding_texture = if uses_secondary_maps {
        material.secondary_specular_texture
    } else {
        material.sphere_properties_texture
    };
    let sphere_binding_index =
        sphere_binding_texture.filter(|&index| model.textures().get(index).is_some());
    let (sphere_key, sphere_label) = if uses_secondary_maps {
        sphere_binding_index.map_or_else(
            || {
                (
                    MaterialTextureKey::RgbaFallback {
                        rgba: [
                            unorm_byte(material.specular_color[0]),
                            unorm_byte(material.specular_color[1]),
                            unorm_byte(material.specular_color[2]),
                            255,
                        ],
                        format: wgpu::TextureFormat::Rgba8Unorm,
                    },
                    "weapon neutral secondary specular texture".to_string(),
                )
            },
            |index| {
                (
                    MaterialTextureKey::Rgba {
                        index,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                    },
                    format!(
                        "weapon secondary specular texture {}",
                        model.textures()[index].path
                    ),
                )
            },
        )
    } else {
        sphere_binding_index.map_or_else(
            || {
                (
                    MaterialTextureKey::FloatRampFallback {
                        neutral: [0.0, 0.0, 1.0, 1.0].map(f32::to_bits),
                    },
                    "weapon neutral sphere texture".to_string(),
                )
            },
            |index| {
                (
                    MaterialTextureKey::FloatRamp {
                        index,
                        neutral: [0.0, 0.0, 1.0, 1.0].map(f32::to_bits),
                    },
                    format!("weapon sphere texture {}", model.textures()[index].path),
                )
            },
        )
    };
    let sphere_properties_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        sphere_key,
        &sphere_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let tile_matrix_index = material
        .tile_matrix_texture
        .filter(|&index| model.textures().get(index).is_some());
    let tile_matrix_label = tile_matrix_index
        .map(|index| {
            format!(
                "weapon tile matrix texture {}",
                model.textures()[index].path
            )
        })
        .unwrap_or_else(|| "weapon neutral tile matrix texture".to_string());
    let tile_matrix_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        MaterialTextureKey::TileMatrix {
            index: tile_matrix_index,
        },
        &tile_matrix_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (index_key, index_label) = material
        .index_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Rgba {
                    index,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
                format!("weapon ColorTable index texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::RgbaFallback {
                    rgba: [0, 0, 0, 255],
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
                "weapon neutral ColorTable index texture".to_string(),
            )
        });
    let index_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        index_key,
        &index_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (colorset_diffuse_key, colorset_diffuse_label) = material
        .colorset_diffuse_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::FloatRamp {
                    index,
                    neutral: [1.0; 4].map(f32::to_bits),
                },
                format!("weapon colorset diffuse ramp texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::FloatRampFallback {
                    neutral: [1.0; 4].map(f32::to_bits),
                },
                "weapon neutral colorset diffuse ramp texture".to_string(),
            )
        });
    let colorset_diffuse_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        colorset_diffuse_key,
        &colorset_diffuse_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());
    let (multi_map_key, multi_map_label) = material
        .multi_map_texture
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Rgba {
                    index,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
                format!("weapon multi map texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::RgbaFallback {
                    rgba: [0, 0, 0, 255],
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
                "weapon neutral multi map texture".to_string(),
            )
        });
    let multi_map_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        multi_map_key,
        &multi_map_label,
    )
    .create_view(&wgpu::TextureViewDescriptor::default());

    let base_color_sampler = create_sampler_for_sampling(
        device,
        "weapon base color sampler",
        prepared_material.texture_sampling.base_color,
    );
    let normal_sampler = create_sampler_for_sampling(
        device,
        "weapon normal sampler",
        prepared_material.texture_sampling.normal,
    );
    let tile_matrix_sampler = create_sampler_for_sampling(
        device,
        "weapon tile matrix sampler",
        prepared_material.texture_sampling.tile_matrix,
    );
    let mask_sampler = create_sampler_for_sampling(
        device,
        "weapon mask sampler",
        prepared_material.texture_sampling.mask,
    );
    let emissive_sampler = create_sampler_for_sampling(
        device,
        "weapon emissive sampler",
        prepared_material.texture_sampling.emissive,
    );
    let material_properties_sampler = create_sampler_for_sampling(
        device,
        "weapon material properties sampler",
        prepared_material.texture_sampling.material_properties,
    );
    let specular_sampler = create_sampler_for_sampling(
        device,
        "weapon specular sampler",
        prepared_material.texture_sampling.specular,
    );
    let tile_sampler = create_sampler_for_sampling(
        device,
        "weapon tile or secondary color sampler",
        if uses_secondary_maps {
            prepared_material.texture_sampling.secondary_base_color
        } else {
            prepared_material.texture_sampling.tile_properties
        },
    );
    let sheen_sampler = create_sampler_for_sampling(
        device,
        "weapon sheen or secondary normal sampler",
        if uses_secondary_maps {
            prepared_material.texture_sampling.secondary_normal
        } else {
            prepared_material.texture_sampling.sheen_properties
        },
    );
    let sphere_sampler = create_sampler_for_sampling(
        device,
        "weapon sphere or secondary specular sampler",
        if uses_secondary_maps {
            prepared_material.texture_sampling.secondary_specular
        } else {
            prepared_material.texture_sampling.sphere_properties
        },
    );
    let index_sampler = create_sampler_for_sampling(
        device,
        "weapon ColorTable index sampler",
        prepared_material.texture_sampling.index,
    );
    let multi_map_sampler = create_sampler_for_sampling(
        device,
        "weapon multi map sampler",
        prepared_material.texture_sampling.multi_map,
    );
    let tile_array_sampler = create_sampler_for_sampling(
        device,
        "weapon tile array pair sampler",
        prepared_material.texture_sampling.tile_normal_array,
    );
    let detail_array_sampler = create_sampler_for_sampling(
        device,
        "weapon detail array pair sampler",
        prepared_material.texture_sampling.detail_diffuse_array,
    );
    // 面妆 decal：仅 skin family 且数据层挂接了 decal 贴图时非退化；其余材质
    // 绑定 1×1 全零透明占位（has-texture 标记为 0，WGSL 不采样其内容）。
    let (decal_key, decal_label) = material
        .character_colors
        .as_ref()
        .and_then(|character| character.decal_texture)
        .and_then(|index| model.textures().get(index).map(|texture| (index, texture)))
        .map(|(index, texture)| {
            (
                MaterialTextureKey::Mipped {
                    index,
                    semantic: RgbaMipSemantic::SrgbColor,
                },
                format!("weapon face decal texture {}", texture.path),
            )
        })
        .unwrap_or_else(|| {
            (
                MaterialTextureKey::MippedFallback {
                    rgba: [255, 255, 255, 0],
                    semantic: RgbaMipSemantic::SrgbColor,
                },
                "weapon transparent face decal texture".to_string(),
            )
        });
    let decal_texture_view = cached_material_texture(
        device,
        queue,
        model.textures(),
        texture_cache,
        decal_key,
        &decal_label,
    )
    // One-layer face decals share this array-compatible slot with target Aura textures.
    .create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let decal_sampler = create_sampler_for_sampling(
        device,
        "weapon face decal sampler",
        PreparedTextureSampling {
            color_space: PreparedTextureColorSpace::Srgb,
            filter: PreparedTextureFilter::Linear,
            address_mode: PreparedTextureAddressMode::ClampToEdge,
        },
    );

    let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("weapon material bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&base_color_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&normal_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&mask_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&emissive_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&material_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&specular_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&normal_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&tile_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::TextureView(&sheen_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(&sphere_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(&tile_matrix_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(&tile_matrix_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: wgpu::BindingResource::TextureView(&index_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: wgpu::BindingResource::TextureView(&colorset_diffuse_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 16,
                resource: wgpu::BindingResource::TextureView(&multi_map_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(tile_array_pair_view),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::TextureView(detail_array_pair_view),
            },
            wgpu::BindGroupEntry {
                binding: 19,
                resource: wgpu::BindingResource::Sampler(&mask_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 20,
                resource: wgpu::BindingResource::Sampler(&emissive_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 21,
                resource: wgpu::BindingResource::Sampler(&material_properties_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 22,
                resource: wgpu::BindingResource::Sampler(&specular_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 23,
                resource: wgpu::BindingResource::Sampler(&tile_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 24,
                resource: wgpu::BindingResource::Sampler(&sheen_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 25,
                resource: wgpu::BindingResource::Sampler(&sphere_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 26,
                resource: wgpu::BindingResource::Sampler(&index_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 28,
                resource: wgpu::BindingResource::Sampler(&multi_map_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 29,
                resource: wgpu::BindingResource::Sampler(&tile_array_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 30,
                resource: wgpu::BindingResource::Sampler(&detail_array_sampler),
            },
        ],
    });
    let overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("weapon surface overlay bind group"),
        layout: overlay_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: overlay_uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&decal_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&decal_sampler),
            },
        ],
    });
    (material_bind_group, overlay_bind_group)
}

pub(crate) fn create_sampler_for_sampling(
    device: &wgpu::Device,
    label: &'static str,
    sampling: PreparedTextureSampling,
) -> wgpu::Sampler {
    device.create_sampler(&sampler_descriptor_for_sampling(label, sampling))
}

pub(crate) fn sampler_descriptor_for_sampling(
    label: &'static str,
    sampling: PreparedTextureSampling,
) -> wgpu::SamplerDescriptor<'static> {
    let address_mode = sampler_address_mode(sampling.address_mode);
    let filter_mode = sampler_filter_mode(sampling.filter);
    wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: filter_mode,
        min_filter: filter_mode,
        mipmap_filter: match sampling.filter {
            PreparedTextureFilter::Linear => wgpu::MipmapFilterMode::Linear,
            PreparedTextureFilter::Nearest => wgpu::MipmapFilterMode::Nearest,
        },
        ..Default::default()
    }
}

pub(crate) fn sampler_address_mode(address_mode: PreparedTextureAddressMode) -> wgpu::AddressMode {
    match address_mode {
        PreparedTextureAddressMode::Repeat => wgpu::AddressMode::Repeat,
        PreparedTextureAddressMode::ClampToEdge | PreparedTextureAddressMode::Clip => {
            wgpu::AddressMode::ClampToEdge
        }
    }
}

pub(crate) fn sampler_filter_mode(filter: PreparedTextureFilter) -> wgpu::FilterMode {
    match filter {
        PreparedTextureFilter::Linear => wgpu::FilterMode::Linear,
        PreparedTextureFilter::Nearest => wgpu::FilterMode::Nearest,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MaterialUniform {
    pub(crate) diffuse_color: [f32; 4],
    pub(crate) emissive_color: [f32; 4],
    pub(crate) specular_color: [f32; 4],
    pub(crate) params: [f32; 4],
    pub(crate) properties: [f32; 4],
    pub(crate) render: [f32; 4],
    pub(crate) alpha_params: [f32; 4],
    pub(crate) alpha_policy_params: [f32; 4],
    pub(crate) alpha_composition_params: [f32; 4],
    pub(crate) water_deep_color: [f32; 4],
    pub(crate) water_refraction_color: [f32; 4],
    pub(crate) water_whitecap_color: [f32; 4],
    pub(crate) extra_properties: [f32; 4],
    pub(crate) shader_params: [f32; 4],
    pub(crate) tile_params: [f32; 4],
    pub(crate) toon_sheen_params: [f32; 4],
    pub(crate) toon_params: [f32; 4],
    pub(crate) sheen_sphere_params: [f32; 4],
    pub(crate) detail_params: [f32; 4],
    pub(crate) array_params: [f32; 4],
    pub(crate) tile_lod_params: [f32; 4],
    // x: the shader composes the full-resolution base diffuse with the
    // colorset_diffuse_texture ramp row color (Compatibility base × colorset).
    pub(crate) colorset_params: [f32; 4],
    pub(crate) detail_color: [f32; 4],
    pub(crate) multi_detail_color: [f32; 4],
    pub(crate) shader_diffuse_color: [f32; 4],
    pub(crate) shader_multi_diffuse_color: [f32; 4],
    pub(crate) shader_emissive_color: [f32; 4],
    pub(crate) shader_multi_emissive_color: [f32; 4],
    pub(crate) outline_params: [f32; 4],
    pub(crate) specular_color_mask: [f32; 4],
    pub(crate) surface_params: [f32; 4],
    pub(crate) detail_color_uv_scale: [f32; 4],
    pub(crate) detail_normal_uv_scale: [f32; 4],
    pub(crate) uv_scroll: [f32; 4],
    pub(crate) lightshaft_color: [f32; 4],
    pub(crate) lightshaft_tex_anim: [f32; 4],
    pub(crate) lightshaft_tex_u: [f32; 4],
    pub(crate) lightshaft_tex_v: [f32; 4],
    pub(crate) lightshaft_ray: [f32; 4],
    pub(crate) uv_sources0: [f32; 4],
    pub(crate) uv_sources1: [f32; 4],
    pub(crate) uv_sources2: [f32; 4],
    pub(crate) uv_sources3: [f32; 4],
    pub(crate) uv_scroll_masks0: [f32; 4],
    pub(crate) uv_scroll_masks1: [f32; 4],
    pub(crate) uv_scroll_masks2: [f32; 4],
    pub(crate) uv_scroll_masks3: [f32; 4],
    pub(crate) feature_params: [f32; 4],
    pub(crate) family_params: [f32; 4],
    pub(crate) secondary_map_params: [f32; 4],
    pub(crate) draw_role_params: [f32; 4],
    pub(crate) debug_color: [f32; 4],
    pub(crate) unsupported_color: [f32; 4],
    // 角色颜色通道（尾部 16 字节对齐区）：RGB 为 squared RGB 线性色，W 为
    // 激活/强度标记；无 character_colors 的材质全零，WGSL 分支不激活，
    // 对武器/装备渲染零影响。面妆贴图在 group(3) 与 Aura 共用采样槽。
    pub(crate) character_skin: [f32; 4],
    pub(crate) character_lip: [f32; 4],
    pub(crate) character_main: [f32; 4],
    pub(crate) character_mesh: [f32; 4],
    pub(crate) character_left_iris: [f32; 4],
    pub(crate) character_right_iris: [f32; 4],
    pub(crate) character_option: [f32; 4],
    pub(crate) character_decal: [f32; 4],
    pub(crate) character_decal_uv: [f32; 4],
    pub(crate) character_params: [f32; 4],
    // bg 域逐贴图 UV 缩放（仅 Bg 家族非 1）：color/normal/specular 各 map0.xy+map1.xy。
    pub(crate) uv_scale_a: [f32; 4],
    pub(crate) uv_scale_b: [f32; 4],
    pub(crate) uv_scale_c: [f32; 4],
    // iris.shpk 专用（仅 Iris 家族非零）：眼白/环色/环带参数与每侧 limbal 强度。
    pub(crate) iris_white_eye: [f32; 4],
    pub(crate) iris_ring_color: [f32; 4],
    pub(crate) iris_ring_a: [f32; 4],
    pub(crate) iris_ring_b: [f32; 4],
}
