use dioxus::prelude::*;
use js_sys::JsString;
use physis::ReadableFile;
use wasm_bindgen::JsValue;
use xiv_companion::{
    BuiltinItemIconProvider, ColorTableRowColors, ItemIconResourceInfo, LocalItemIconImage,
    PackedModelId, ProviderRequest, ResourceBlob, ResourceError, ResourceErrorKind, ResourceFuture,
    ResourceHub, ResourceProvider, ResourceSource, WeaponMaterialRenderMode, WeaponModelData,
    WeaponModelMaterial, WeaponModelTexture, WeaponModelTextureKind, bake_color_table_maps,
    calculate_model_bounds, item_icon_tex_path, material_color, meshes_from_mdl_bytes,
    register_craft_data_resource, register_item_icon_resource, register_weapon_model_resources,
    resources::{
        craft_data::CraftDataKind,
        item_icon::ItemIconKind,
        weapon_model::{WeaponCatalogKind, WeaponModelKind, parse_weapon_model_request_key},
    },
    weapon_material_candidate_paths, weapon_model_candidate_paths,
};

use crate::app::browser_sqpack::BrowserSqPack;
use crate::app::load_progress::{
    CraftDataCacheStatus, CraftDataLoadProgress, report_craft_data_cache_status,
    report_craft_data_progress,
};
use crate::app::log;

const BUNDLED_CRAFT_DATA_ASSET: Asset = asset!("/assets/craft-data.json");
const LOCAL_RESOURCE_CACHE_DB: &str = "xiv-companion-resource-cache";
const LOCAL_RESOURCE_CACHE_STORE: &str = "resources";
const LOCAL_CRAFT_DATA_CACHE_KEY: &str = "user-local-craft-data";
const LOCAL_WEAPON_CATALOG_CACHE_KEY: &str = "user-local-weapon-catalog";
const ITEM_ICON_READ_WINDOW: u64 = 2 * 1024 * 1024;

pub fn default_web_resource_hub() -> ResourceHub {
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    register_item_icon_resource(&mut hub);
    register_weapon_model_resources(&mut hub);
    hub.add_provider(BundledProvider);
    hub.add_provider(BuiltinItemIconProvider);
    hub.add_provider(BrowserSqPackProvider);
    hub
}

pub struct BrowserSqPackProvider;

impl ResourceProvider for BrowserSqPackProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::UserLocal
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        (request.kind == CraftDataKind.into() && request.key == "default")
            || (request.kind == ItemIconKind.into() && request.key.parse::<u32>().is_ok())
            || (request.kind == WeaponCatalogKind.into() && request.key == "default")
            || (request.kind == WeaponModelKind.into()
                && parse_weapon_model_request_key(&request.key).is_ok())
    }

    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
        Box::pin(async move {
            let resource_kind = request.kind.clone();
            if !self.supports(&request) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(self.source()),
                    "Local SqPack source is not configured for this resource",
                ));
            }

            if request.kind == CraftDataKind.into() {
                log::info("resource", "loading CraftData from UserLocal SqPack");
                let start_ms = log::now_ms();
                let (bytes, fingerprint) = load_from_browser_sqpack_direct("craft-data", "default")
                    .await
                    .map_err(|error| {
                        report_craft_data_progress(Some(CraftDataLoadProgress {
                            stage: "本地 CraftData 失败".to_string(),
                            detail: error.clone(),
                            current: 1,
                            total: 1,
                            elapsed_ms: log::elapsed_ms(start_ms),
                            done: true,
                        }));
                        report_craft_data_cache_status(Some(CraftDataCacheStatus::Error {
                            message: error.clone(),
                        }));
                        ResourceError::new(
                            browser_sqpack_provider_error_kind(&error),
                            resource_kind,
                            Some(self.source()),
                            error,
                        )
                    })?;
                log::info(
                    "resource",
                    format!(
                        "loaded CraftData from UserLocal in {}",
                        log::format_elapsed(log::elapsed_ms(start_ms))
                    ),
                );
                return Ok(ResourceBlob { bytes, fingerprint });
            }

            if request.kind == WeaponCatalogKind.into() {
                log::info("resource", "loading WeaponCatalog from UserLocal SqPack");
                let start_ms = log::now_ms();
                let bytes = load_weapon_catalog_from_browser_sqpack()
                    .await
                    .map_err(|error| {
                        ResourceError::new(
                            browser_sqpack_provider_error_kind(&error),
                            resource_kind.clone(),
                            Some(self.source()),
                            error,
                        )
                    })?;
                log::info(
                    "resource",
                    format!(
                        "loaded WeaponCatalog from UserLocal in {}",
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                return Ok(ResourceBlob {
                    bytes,
                    fingerprint: None,
                });
            }

            if request.kind == WeaponModelKind.into() {
                log::info("resource", format!("loading WeaponModel {}", request.key));
                let start_ms = log::now_ms();
                let id = parse_weapon_model_request_key(&request.key).map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::Unsupported,
                        resource_kind.clone(),
                        Some(self.source()),
                        error,
                    )
                })?;
                let model = load_weapon_model_from_browser_sqpack(id)
                    .await
                    .map_err(|error| {
                        ResourceError::new(
                            browser_sqpack_provider_error_kind(&error),
                            resource_kind.clone(),
                            Some(self.source()),
                            error,
                        )
                    })?;
                log::info(
                    "resource",
                    format!(
                        "decoded WeaponModel in {}",
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                let bytes = serde_json::to_vec(&model).map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::ProviderFailed,
                        resource_kind,
                        Some(self.source()),
                        format!("failed to encode weapon model resource info: {error}"),
                    )
                })?;
                return Ok(ResourceBlob {
                    bytes,
                    fingerprint: None,
                });
            }

            log::info(
                "resource",
                format!("loading ItemIcon {} from UserLocal SqPack", request.key),
            );
            let start_ms = log::now_ms();
            let icon_id = request.key.parse::<u32>().map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("invalid item icon id {}: {error}", request.key),
                )
            })?;
            let path = item_icon_tex_path(icon_id);
            let (bytes, _) = load_from_browser_sqpack_direct("item-icon", &request.key)
                .await
                .map_err(|error| {
                    ResourceError::new(
                        browser_sqpack_not_found_or_access_error_kind(&error),
                        resource_kind.clone(),
                        Some(self.source()),
                        error,
                    )
                })?;
            let texture = physis::tex::Texture::from_existing(physis::Platform::Win32, &bytes)
                .ok_or_else(|| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        resource_kind.clone(),
                        Some(self.source()),
                        format!("failed to decode local icon texture {path}"),
                    )
                })?;
            let rgba = texture.to_rgba().ok_or_else(|| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("failed to convert local icon texture {path} to RGBA"),
                )
            })?;
            log::info(
                "resource",
                format!(
                    "decoded local ItemIcon {icon_id}: {}x{} in {}",
                    texture.width,
                    texture.height,
                    log::format_elapsed(log::elapsed_ms(start_ms)),
                ),
            );
            let info = ItemIconResourceInfo {
                icon_id,
                urls: Vec::new(),
                local_image: Some(LocalItemIconImage {
                    path,
                    width: texture.width,
                    height: texture.height,
                    rgba,
                }),
            };
            let bytes = serde_json::to_vec(&info).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind,
                    Some(self.source()),
                    format!("failed to encode local item icon resource info: {error}"),
                )
            })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint: None,
            })
        })
    }
}

fn browser_sqpack_provider_error_kind(error: &str) -> ResourceErrorKind {
    browser_sqpack_access_error_kind(error).unwrap_or(ResourceErrorKind::ProviderFailed)
}

fn browser_sqpack_not_found_or_access_error_kind(error: &str) -> ResourceErrorKind {
    browser_sqpack_access_error_kind(error).unwrap_or(ResourceErrorKind::NotFound)
}

fn browser_sqpack_access_error_kind(error: &str) -> Option<ResourceErrorKind> {
    if error.contains("尚未选择")
        || error.contains("权限")
        || error.contains("permission")
        || error.contains("denied")
        || error.contains("没有 window")
    {
        return Some(ResourceErrorKind::PermissionMissing);
    }

    if error.contains("没有 sqpack") {
        return Some(ResourceErrorKind::NotFound);
    }

    None
}

async fn load_weapon_catalog_from_browser_sqpack() -> Result<Vec<u8>, String> {
    let start_ms = log::now_ms();
    let open_start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    log::info(
        "resource",
        format!(
            "WeaponCatalog from_window_handle completed in {}",
            log::format_elapsed(log::elapsed_ms(open_start_ms)),
        ),
    );

    let fingerprint_start_ms = log::now_ms();
    let cache_fingerprint = match sqpack.weapon_catalog_cache_fingerprint().await {
        Ok(fingerprint) => {
            log::info(
                "resource",
                format!(
                    "WeaponCatalog cache fingerprint completed in {}",
                    log::format_elapsed(log::elapsed_ms(fingerprint_start_ms)),
                ),
            );
            Some(fingerprint)
        }
        Err(error) => {
            log::warn(
                "resource-cache",
                format!("WeaponCatalog cache fingerprint failed; skipping cache: {error}"),
            );
            None
        }
    };

    if let Some(cache_fingerprint) = cache_fingerprint.as_deref() {
        let cache_start_ms = log::now_ms();
        match load_cached_local_weapon_catalog(cache_fingerprint).await {
            Ok(Some((bytes, game_version))) => {
                let version_detail = game_version
                    .as_deref()
                    .map(|version| format!(" ({version})"))
                    .unwrap_or_default();
                log::info(
                    "resource",
                    format!(
                        "WeaponCatalog cache hit{version_detail}: {} bytes, cache lookup={}, total={}",
                        bytes.len(),
                        log::format_elapsed(log::elapsed_ms(cache_start_ms)),
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                return Ok(bytes);
            }
            Ok(None) => {
                log::info(
                    "resource",
                    format!(
                        "WeaponCatalog cache miss checked in {}; exporting from SqPack",
                        log::format_elapsed(log::elapsed_ms(cache_start_ms)),
                    ),
                );
            }
            Err(error) => {
                log::warn(
                    "resource-cache",
                    format!("WeaponCatalog cache lookup failed; exporting from SqPack: {error}"),
                );
            }
        }
    }

    let preload_start_ms = log::now_ms();
    let resource = sqpack.preload_weapon_catalog_resource().await?;
    let preload_elapsed_ms = log::elapsed_ms(preload_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog preload_weapon_catalog_resource completed in {}",
            log::format_elapsed(preload_elapsed_ms),
        ),
    );

    let generated_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    let export_start_ms = log::now_ms();
    let data = xiv_companion::game_data::export_weapon_catalog_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        "Local SqPack".to_string(),
        generated_at,
    )
    .map_err(|error| format!("failed to export WeaponCatalog from local SqPack: {error:#}"))?;
    let export_elapsed_ms = log::elapsed_ms(export_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog export_weapon_catalog_from_resource completed in {} ({} items)",
            log::format_elapsed(export_elapsed_ms),
            data.counts.items,
        ),
    );

    let encode_start_ms = log::now_ms();
    let bytes = serde_json::to_vec(&data)
        .map_err(|error| format!("failed to encode WeaponCatalog: {error}"))?;
    let encode_elapsed_ms = log::elapsed_ms(encode_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog serde_json encode completed in {} ({} bytes)",
            log::format_elapsed(encode_elapsed_ms),
            bytes.len(),
        ),
    );

    if let Some(cache_fingerprint) = cache_fingerprint.as_deref() {
        let save_start_ms = log::now_ms();
        match save_cached_local_weapon_catalog(cache_fingerprint, &data.game_version, &bytes).await
        {
            Ok(()) => {
                log::info(
                    "resource-cache",
                    format!(
                        "WeaponCatalog cache save completed in {}",
                        log::format_elapsed(log::elapsed_ms(save_start_ms)),
                    ),
                );
            }
            Err(error) => {
                log::warn(
                    "resource-cache",
                    format!("failed to save WeaponCatalog cache: {error}"),
                );
            }
        }
    }

    log::info(
        "resource",
        format!(
            "WeaponCatalog local pipeline completed: preload={}, export={}, encode={}, total={}",
            log::format_elapsed(preload_elapsed_ms),
            log::format_elapsed(export_elapsed_ms),
            log::format_elapsed(encode_elapsed_ms),
            log::format_elapsed(log::elapsed_ms(start_ms)),
        ),
    );
    Ok(bytes)
}

async fn load_weapon_model_from_browser_sqpack(
    id: xiv_companion::WeaponModelId,
) -> Result<WeaponModelData, String> {
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let model_main = PackedModelId::from_raw(id.model_main);
    let model_sub = (id.model_sub != 0).then(|| PackedModelId::from_raw(id.model_sub));
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();

    load_weapon_model_meshes(
        &mut sqpack,
        model_main,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
    )
    .await?;
    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            if let Err(error) = load_weapon_model_meshes(
                &mut sqpack,
                model_sub,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
            )
            .await
            {
                log::warn(
                    "resource",
                    format!("failed to load secondary weapon model: {error}"),
                );
            }
        }
    }

    if meshes.is_empty() {
        return Err(format!("{} 没有可渲染的模型网格", id.item_name));
    }

    Ok(WeaponModelData {
        item_id: id.item_id,
        item_name: id.item_name,
        model_main,
        model_sub,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

async fn load_weapon_model_meshes(
    sqpack: &mut BrowserSqPack,
    model: PackedModelId,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<xiv_companion::WeaponModelMesh>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for path in weapon_model_candidate_paths(model) {
        match sqpack.try_read_game_file(&path).await {
            Ok(bytes) => {
                let mut path_meshes =
                    meshes_from_mdl_bytes(&path, &bytes).map_err(|error| format!("{error:#}"))?;
                push_loaded_path(loaded_paths, path.clone());
                assign_weapon_materials(
                    sqpack,
                    model,
                    &path,
                    &mut path_meshes,
                    materials,
                    textures,
                    loaded_paths,
                )
                .await;
                meshes.append(&mut path_meshes);
                return Ok(());
            }
            Err(error) => errors.push(error),
        }
    }

    Err(format!(
        "无法读取 weapon model {} (tried: {})",
        model.model_id,
        errors.join("; ")
    ))
}

async fn assign_weapon_materials(
    sqpack: &mut BrowserSqPack,
    model: PackedModelId,
    model_path: &str,
    meshes: &mut [xiv_companion::WeaponModelMesh],
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) {
    let mut slots = Vec::<(u16, usize)>::new();
    let mut material_specs = Vec::<(u16, String)>::new();
    for mesh in meshes.iter() {
        if !material_specs
            .iter()
            .any(|(index, _)| *index == mesh.material_index)
        {
            material_specs.push((mesh.material_index, mesh.material_name.clone()));
        }
    }

    for (material_index, material_name) in material_specs {
        let slot = materials.len();
        let material = load_weapon_material(
            sqpack,
            model,
            model_path,
            material_index,
            material_name,
            slot,
            textures,
            loaded_paths,
        )
        .await;
        materials.push(material);
        slots.push((material_index, slot));
    }

    for mesh in meshes {
        if let Some((_, slot)) = slots
            .iter()
            .find(|(material_index, _)| *material_index == mesh.material_index)
        {
            mesh.material_slot = *slot;
        }
    }
}

async fn load_weapon_material(
    sqpack: &mut BrowserSqPack,
    model: PackedModelId,
    model_path: &str,
    material_index: u16,
    material_name: String,
    slot: usize,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponModelMaterial {
    let fallback = material_color(material_index);
    let candidates = weapon_material_candidate_paths(model, model_path, &material_name);
    for path in candidates {
        let Ok(bytes) = sqpack.try_read_game_file(&path).await else {
            continue;
        };
        let Some(material) = physis::mtrl::Material::from_existing(physis::Platform::Win32, &bytes)
        else {
            continue;
        };

        push_loaded_path(loaded_paths, path.clone());
        let summary = summarize_material_colors(material.color_table.as_ref(), fallback);
        let sampler_roles = parse_material_sampler_roles(&bytes);
        let shader_flags = parse_material_shader_flags(&bytes);
        let texture_set = load_weapon_material_textures(
            sqpack,
            &path,
            &material,
            &sampler_roles,
            textures,
            loaded_paths,
        )
        .await;

        let render_mode =
            weapon_material_render_mode(&material.shader_package_name, shader_flags, &texture_set);
        let opacity = weapon_material_opacity(render_mode);
        let diffuse_color = if texture_set.base_color.is_some() {
            [1.0, 1.0, 1.0]
        } else {
            summary.diffuse
        };
        let emissive_color = preview_emissive_color(summary.emissive, &texture_set);

        return WeaponModelMaterial {
            slot,
            material_index,
            name: material_name,
            path: Some(path),
            shader_package_name: Some(material.shader_package_name),
            render_mode,
            opacity,
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            normal_texture: texture_set.normal,
            mask_texture: texture_set.mask,
            emissive_texture: texture_set.emissive,
        };
    }

    fallback_weapon_material(slot, material_index, material_name, fallback)
}

async fn load_weapon_material_textures(
    sqpack: &mut BrowserSqPack,
    material_path: &str,
    material: &physis::mtrl::Material,
    sampler_roles: &[MaterialSamplerRole],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in material.texture_paths.iter().enumerate() {
        let kind = classify_weapon_texture(
            raw_texture_path,
            sampler_kind_for_texture(sampler_roles, texture_order),
        );
        let Some(texture_index) = load_weapon_texture(
            sqpack,
            material_path,
            raw_texture_path,
            kind,
            textures,
            loaded_paths,
        )
        .await
        else {
            continue;
        };
        if !set.indices.contains(&texture_index) {
            set.indices.push(texture_index);
        }
        match textures[texture_index].kind {
            WeaponModelTextureKind::BaseColor => {
                set.base_color.get_or_insert(texture_index);
                if texture_alpha_affects_material_transparency(&textures[texture_index]) {
                    set.has_alpha = true;
                }
            }
            WeaponModelTextureKind::Normal => {
                set.normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Mask | WeaponModelTextureKind::Specular => {
                set.mask.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Emissive => {
                set.emissive.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Index => {
                set.index.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Other => {}
        }
    }

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        material.color_table.as_ref(),
        set.index,
        set.emissive.is_none(),
        shader_opacity_override(&material.shader_package_name),
        textures,
    ) {
        if let Some(base_color) = set.base_color {
            if let Some(combined) = combine_base_with_colorset_texture(
                material_path,
                base_color,
                baked.base_color,
                textures,
            ) {
                set.base_color = Some(combined);
                add_unique_index(&mut set.indices, combined);
                if texture_alpha_affects_material_transparency(&textures[combined]) {
                    set.has_alpha = true;
                }
            }
        } else {
            set.base_color = Some(baked.base_color);
            add_unique_index(&mut set.indices, baked.base_color);
            if texture_alpha_affects_material_transparency(&textures[baked.base_color]) {
                set.has_alpha = true;
            }
        }

        if set.emissive.is_none() {
            if let Some(emissive) = baked.emissive {
                set.emissive = Some(emissive);
                add_unique_index(&mut set.indices, emissive);
            }
        }
    }

    if set.base_color.is_none() {
        set.base_color = choose_fallback_base_texture(&set.indices, textures);
    }

    set
}

async fn load_weapon_texture(
    sqpack: &mut BrowserSqPack,
    material_path: &str,
    raw_texture_path: &str,
    kind: WeaponModelTextureKind,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    for path in weapon_texture_candidate_paths(material_path, raw_texture_path) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
            textures[index].kind = merge_texture_kind(textures[index].kind, kind);
            return Some(index);
        }

        let Ok(bytes) = sqpack.try_read_game_file(&path).await else {
            continue;
        };
        let Some(texture) = physis::tex::Texture::from_existing(physis::Platform::Win32, &bytes)
        else {
            continue;
        };
        let Some(rgba) = texture.to_rgba() else {
            continue;
        };
        let index = textures.len();
        textures.push(WeaponModelTexture {
            path: path.clone(),
            kind,
            width: texture.width,
            height: texture.height,
            rgba,
        });
        push_loaded_path(loaded_paths, path);
        return Some(index);
    }

    None
}

#[derive(Default)]
struct WeaponTextureSet {
    indices: Vec<usize>,
    base_color: Option<usize>,
    normal: Option<usize>,
    mask: Option<usize>,
    emissive: Option<usize>,
    index: Option<usize>,
    has_alpha: bool,
}

struct BakedWeaponTextureIndices {
    base_color: usize,
    emissive: Option<usize>,
}

fn texture_has_alpha(texture: &WeaponModelTexture) -> bool {
    texture.rgba.chunks_exact(4).any(|pixel| pixel[3] < 250)
}

fn texture_alpha_affects_material_transparency(texture: &WeaponModelTexture) -> bool {
    texture.kind == WeaponModelTextureKind::BaseColor && texture_has_alpha(texture)
}

#[derive(Clone, Copy, Debug)]
struct MaterialSamplerRole {
    texture_index: usize,
    kind: WeaponModelTextureKind,
}

struct MaterialColorSummary {
    diffuse: [f32; 3],
    specular: [f32; 3],
    emissive: [f32; 3],
    roughness: f32,
    metalness: f32,
}

fn add_unique_index(indices: &mut Vec<usize>, index: usize) {
    if !indices.contains(&index) {
        indices.push(index);
    }
}

fn bake_weapon_color_table_textures(
    material_path: &str,
    color_table: Option<&physis::mtrl::ColorTable>,
    index_texture: Option<usize>,
    bake_emissive: bool,
    opacity_override: Option<f32>,
    textures: &mut Vec<WeaponModelTexture>,
) -> Option<BakedWeaponTextureIndices> {
    let rows = weapon_color_table_rows(color_table?)?;
    let index_texture = textures.get(index_texture?)?;
    let width = index_texture.width;
    let height = index_texture.height;
    let id_rgba = index_texture.rgba.clone();
    let mut baked = bake_color_table_maps(&rows, &id_rgba)?;
    if let Some(opacity) = opacity_override {
        apply_alpha_override(&mut baked.diffuse_rgba, opacity);
    }
    let material_key = normalize_game_resource_path(material_path);

    let base_path = format!("baked://{material_key}#colorset-diffuse");
    let base_color = push_or_replace_baked_texture(
        textures,
        base_path,
        WeaponModelTextureKind::BaseColor,
        width,
        height,
        baked.diffuse_rgba,
    );

    let emissive = if bake_emissive {
        baked.emissive_rgba.map(|rgba| {
            push_or_replace_baked_texture(
                textures,
                format!("baked://{material_key}#colorset-emissive"),
                WeaponModelTextureKind::Emissive,
                width,
                height,
                rgba,
            )
        })
    } else {
        None
    };

    Some(BakedWeaponTextureIndices {
        base_color,
        emissive,
    })
}

fn weapon_material_render_mode(
    shader_package_name: &str,
    shader_flags: u32,
    texture_set: &WeaponTextureSet,
) -> WeaponMaterialRenderMode {
    const ENABLE_TRANSLUCENCY: u32 = 0x10;
    let shader = shader_package_name.to_ascii_lowercase();
    if shader.contains("glass") {
        WeaponMaterialRenderMode::Glass
    } else if texture_set.has_alpha || shader_flags & ENABLE_TRANSLUCENCY != 0 {
        WeaponMaterialRenderMode::Transparent
    } else {
        WeaponMaterialRenderMode::Opaque
    }
}

fn weapon_material_opacity(mode: WeaponMaterialRenderMode) -> f32 {
    match mode {
        WeaponMaterialRenderMode::Opaque => 1.0,
        WeaponMaterialRenderMode::Transparent => 1.0,
        WeaponMaterialRenderMode::Glass => 0.28,
    }
}

fn shader_opacity_override(shader_package_name: &str) -> Option<f32> {
    let mode = weapon_material_render_mode(shader_package_name, 0, &WeaponTextureSet::default());
    (mode == WeaponMaterialRenderMode::Glass).then_some(weapon_material_opacity(mode))
}

fn apply_alpha_override(rgba: &mut [u8], opacity: f32) {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = pixel[3].min(alpha);
    }
}

fn combine_base_with_colorset_texture(
    material_path: &str,
    base_index: usize,
    colorset_index: usize,
    textures: &mut Vec<WeaponModelTexture>,
) -> Option<usize> {
    let base = textures.get(base_index)?.clone();
    let colorset = textures.get(colorset_index)?.clone();
    let width = colorset.width.max(1) as usize;
    let height = colorset.height.max(1) as usize;
    let base_width = base.width.max(1) as usize;
    let base_height = base.height.max(1) as usize;

    let mut rgba = Vec::with_capacity(colorset.rgba.len());
    for y in 0..height {
        let base_y = y * base_height / height;
        for x in 0..width {
            let base_x = x * base_width / width;
            let base_offset = (base_y * base_width + base_x) * 4;
            let colorset_offset = (y * width + x) * 4;
            let base = base.rgba.get(base_offset..base_offset + 4)?;
            let colorset = colorset.rgba.get(colorset_offset..colorset_offset + 4)?;
            rgba.push(multiply_srgb_channels(base[0], colorset[0]));
            rgba.push(multiply_srgb_channels(base[1], colorset[1]));
            rgba.push(multiply_srgb_channels(base[2], colorset[2]));
            rgba.push(((u16::from(base[3]) * u16::from(colorset[3])) / 255) as u8);
        }
    }

    Some(push_or_replace_baked_texture(
        textures,
        format!("baked://{material_path}#base-times-colorset"),
        WeaponModelTextureKind::BaseColor,
        colorset.width,
        colorset.height,
        rgba,
    ))
}

fn multiply_srgb_channels(a: u8, b: u8) -> u8 {
    linear_to_srgb_u8(srgb_u8_to_linear(a) * srgb_u8_to_linear(b))
}

fn srgb_u8_to_linear(value: u8) -> f32 {
    let srgb = f32::from(value) / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn push_or_replace_baked_texture(
    textures: &mut Vec<WeaponModelTexture>,
    path: String,
    kind: WeaponModelTextureKind,
    width: u16,
    height: u16,
    rgba: Vec<u8>,
) -> usize {
    if let Some(index) = textures.iter().position(|texture| texture.path == path) {
        textures[index] = WeaponModelTexture {
            path,
            kind,
            width,
            height,
            rgba,
        };
        return index;
    }

    let index = textures.len();
    textures.push(WeaponModelTexture {
        path,
        kind,
        width,
        height,
        rgba,
    });
    index
}

fn weapon_color_table_rows(
    color_table: &physis::mtrl::ColorTable,
) -> Option<Vec<ColorTableRowColors>> {
    match color_table {
        physis::mtrl::ColorTable::DawntrailColorTable(table) => Some(
            table
                .rows
                .iter()
                .map(|row| ColorTableRowColors {
                    diffuse: row.diffuse_color,
                    emissive: row.emissive_color,
                    alpha: row.tile_alpha,
                })
                .collect(),
        ),
        // 当前 `_id.tex` 烘焙实现针对 Dawntrail 32 行 ColorTable。
        // 旧 16 行格式的行索引编码不同，先继续使用平均色回退，避免误烘焙。
        physis::mtrl::ColorTable::LegacyColorTable(_)
        | physis::mtrl::ColorTable::OpaqueColorTable(_) => None,
    }
}

fn choose_fallback_base_texture(
    indices: &[usize],
    textures: &[WeaponModelTexture],
) -> Option<usize> {
    indices.iter().copied().find(|index| {
        textures
            .get(*index)
            .is_some_and(|texture| texture.kind == WeaponModelTextureKind::Other)
    })
}

fn preview_emissive_color(emissive: [f32; 3], texture_set: &WeaponTextureSet) -> [f32; 3] {
    let scale = if texture_set.emissive.is_some() {
        1.0
    } else if texture_set.mask.is_some() {
        0.25
    } else {
        0.0
    };
    [
        emissive[0].clamp(0.0, 4.0) * scale,
        emissive[1].clamp(0.0, 4.0) * scale,
        emissive[2].clamp(0.0, 4.0) * scale,
    ]
}

fn summarize_material_colors(
    color_table: Option<&physis::mtrl::ColorTable>,
    fallback: [f32; 3],
) -> MaterialColorSummary {
    let mut diffuse = ColorAccumulator::default();
    let mut specular = ColorAccumulator::default();
    let mut emissive = [0.0; 3];
    let mut roughness_total = 0.0;
    let mut metalness_total = 0.0;
    let mut physical_rows = 0_u32;

    match color_table {
        Some(physis::mtrl::ColorTable::LegacyColorTable(table)) => {
            for row in &table.rows {
                diffuse.add_nonzero(row.diffuse_color);
                specular.add_nonzero(row.specular_color);
                emissive = brighter_color(emissive, row.emissive_color);
            }
        }
        Some(physis::mtrl::ColorTable::DawntrailColorTable(table)) => {
            for row in &table.rows {
                diffuse.add_nonzero(row.diffuse_color);
                specular.add_nonzero(row.specular_color);
                emissive = brighter_color(emissive, row.emissive_color);
                if row.roughness.is_finite() && row.metalness.is_finite() {
                    roughness_total += row.roughness.clamp(0.0, 1.0);
                    metalness_total += row.metalness.clamp(0.0, 1.0);
                    physical_rows += 1;
                }
            }
        }
        Some(physis::mtrl::ColorTable::OpaqueColorTable(_)) | None => {}
    }

    MaterialColorSummary {
        diffuse: diffuse.average().unwrap_or(fallback),
        specular: specular.average().unwrap_or([0.45, 0.45, 0.45]),
        emissive,
        roughness: if physical_rows == 0 {
            0.5
        } else {
            roughness_total / physical_rows as f32
        },
        metalness: if physical_rows == 0 {
            0.0
        } else {
            metalness_total / physical_rows as f32
        },
    }
}

#[derive(Default)]
struct ColorAccumulator {
    total: [f32; 3],
    count: u32,
}

impl ColorAccumulator {
    fn add_nonzero(&mut self, color: [f32; 3]) {
        if color.iter().any(|value| value.abs() > 0.0001) {
            for (slot, value) in self.total.iter_mut().zip(color) {
                *slot += value;
            }
            self.count += 1;
        }
    }

    fn average(&self) -> Option<[f32; 3]> {
        (self.count != 0).then(|| {
            [
                self.total[0] / self.count as f32,
                self.total[1] / self.count as f32,
                self.total[2] / self.count as f32,
            ]
        })
    }
}

fn fallback_weapon_material(
    slot: usize,
    material_index: u16,
    name: String,
    fallback: [f32; 3],
) -> WeaponModelMaterial {
    WeaponModelMaterial {
        slot,
        material_index,
        name,
        path: None,
        shader_package_name: None,
        render_mode: WeaponMaterialRenderMode::Opaque,
        opacity: 1.0,
        fallback_color: fallback,
        diffuse_color: fallback,
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        normal_texture: None,
        mask_texture: None,
        emissive_texture: None,
    }
}

fn brighter_color(current: [f32; 3], candidate: [f32; 3]) -> [f32; 3] {
    let current_luma = current[0] * 0.2126 + current[1] * 0.7152 + current[2] * 0.0722;
    let candidate_luma = candidate[0] * 0.2126 + candidate[1] * 0.7152 + candidate[2] * 0.0722;
    if candidate_luma > current_luma {
        candidate
    } else {
        current
    }
}

fn parse_material_sampler_roles(bytes: &[u8]) -> Vec<MaterialSamplerRole> {
    let Some(texture_count) = bytes.get(12).copied().map(usize::from) else {
        return Vec::new();
    };
    let Some(uv_set_count) = bytes.get(13).copied().map(usize::from) else {
        return Vec::new();
    };
    let Some(color_set_count) = bytes.get(14).copied().map(usize::from) else {
        return Vec::new();
    };
    let Some(additional_data_size) = bytes.get(15).copied().map(usize::from) else {
        return Vec::new();
    };
    let Some(string_table_size) = read_u16_le(bytes, 8).map(usize::from) else {
        return Vec::new();
    };

    let mut offset = 16_usize;
    for byte_count in [
        texture_count.saturating_mul(4),
        uv_set_count.saturating_mul(4),
        color_set_count.saturating_mul(4),
        string_table_size,
    ] {
        let Some(next) = checked_advance(offset, byte_count, bytes.len()) else {
            return Vec::new();
        };
        offset = next;
    }

    let additional_data_offset = offset;
    let table_flags = if additional_data_size >= 4 {
        read_u32_le(bytes, additional_data_offset).unwrap_or(0)
    } else {
        0
    };
    let Some(next) = checked_advance(offset, additional_data_size, bytes.len()) else {
        return Vec::new();
    };
    offset = next;

    let table_dimension_logs = (table_flags >> 4) as u8;
    if table_flags & 0x4 != 0 {
        let Some(next) = checked_advance(
            offset,
            material_color_table_byte_len(table_dimension_logs),
            bytes.len(),
        ) else {
            return Vec::new();
        };
        offset = next;
    }
    if table_flags & 0x8 != 0 {
        let Some(next) = checked_advance(
            offset,
            material_color_dye_table_byte_len(table_dimension_logs),
            bytes.len(),
        ) else {
            return Vec::new();
        };
        offset = next;
    }

    let Some(shader_key_count) = read_u16_le(bytes, offset + 2).map(usize::from) else {
        return Vec::new();
    };
    let Some(constant_count) = read_u16_le(bytes, offset + 4).map(usize::from) else {
        return Vec::new();
    };
    let Some(sampler_count) = read_u16_le(bytes, offset + 6).map(usize::from) else {
        return Vec::new();
    };
    let Some(mut sampler_offset) = checked_advance(
        offset,
        12_usize
            .saturating_add(shader_key_count.saturating_mul(8))
            .saturating_add(constant_count.saturating_mul(8)),
        bytes.len(),
    ) else {
        return Vec::new();
    };

    let mut roles = Vec::new();
    for _ in 0..sampler_count {
        let Some(texture_usage) = read_u32_le(bytes, sampler_offset) else {
            return roles;
        };
        let Some(texture_index) = bytes.get(sampler_offset + 8).copied().map(usize::from) else {
            return roles;
        };
        if texture_index < texture_count {
            if let Some(kind) = classify_sampler_usage(texture_usage) {
                roles.push(MaterialSamplerRole {
                    texture_index,
                    kind,
                });
            }
        }
        let Some(next) = checked_advance(sampler_offset, 12, bytes.len()) else {
            return roles;
        };
        sampler_offset = next;
    }

    roles
}

fn parse_material_shader_flags(bytes: &[u8]) -> u32 {
    let Some(texture_count) = bytes.get(12).copied().map(usize::from) else {
        return 0;
    };
    let Some(uv_set_count) = bytes.get(13).copied().map(usize::from) else {
        return 0;
    };
    let Some(color_set_count) = bytes.get(14).copied().map(usize::from) else {
        return 0;
    };
    let Some(additional_data_size) = bytes.get(15).copied().map(usize::from) else {
        return 0;
    };
    let Some(string_table_size) = read_u16_le(bytes, 8).map(usize::from) else {
        return 0;
    };

    let mut offset = 16_usize;
    for byte_count in [
        texture_count.saturating_mul(4),
        uv_set_count.saturating_mul(4),
        color_set_count.saturating_mul(4),
        string_table_size,
    ] {
        let Some(next) = checked_advance(offset, byte_count, bytes.len()) else {
            return 0;
        };
        offset = next;
    }

    let additional_data_offset = offset;
    let table_flags = if additional_data_size >= 4 {
        read_u32_le(bytes, additional_data_offset).unwrap_or(0)
    } else {
        0
    };
    let Some(next) = checked_advance(offset, additional_data_size, bytes.len()) else {
        return 0;
    };
    offset = next;

    let table_dimension_logs = (table_flags >> 4) as u8;
    if table_flags & 0x4 != 0 {
        let Some(next) = checked_advance(
            offset,
            material_color_table_byte_len(table_dimension_logs),
            bytes.len(),
        ) else {
            return 0;
        };
        offset = next;
    }
    if table_flags & 0x8 != 0 {
        let Some(next) = checked_advance(
            offset,
            material_color_dye_table_byte_len(table_dimension_logs),
            bytes.len(),
        ) else {
            return 0;
        };
        offset = next;
    }

    read_u32_le(bytes, offset + 8).unwrap_or(0)
}

fn material_color_table_byte_len(table_dimension_logs: u8) -> usize {
    match table_dimension_logs {
        0 | 0x42 => 16 * 32,
        0x53 => 32 * 64,
        _ => 0,
    }
}

fn material_color_dye_table_byte_len(table_dimension_logs: u8) -> usize {
    match table_dimension_logs {
        0 => 16 * 2,
        0x50..=0x5f => 32 * 4,
        _ => 0,
    }
}

fn sampler_kind_for_texture(
    sampler_roles: &[MaterialSamplerRole],
    texture_index: usize,
) -> Option<WeaponModelTextureKind> {
    sampler_roles
        .iter()
        .find(|role| role.texture_index == texture_index)
        .map(|role| role.kind)
}

fn classify_sampler_usage(texture_usage: u32) -> Option<WeaponModelTextureKind> {
    if sampler_usage_matches(
        texture_usage,
        &[
            "g_SamplerNormal",
            "g_NormalSampler",
            "g_SamplerNormalMap",
            "g_NormalMapSampler",
        ],
    ) {
        Some(WeaponModelTextureKind::Normal)
    } else if sampler_usage_matches(
        texture_usage,
        &[
            "g_SamplerEmissive",
            "g_EmissiveSampler",
            "g_SamplerEmission",
            "g_EmissionSampler",
            "g_SamplerLight",
            "g_LightSampler",
        ],
    ) {
        Some(WeaponModelTextureKind::Emissive)
    } else if sampler_usage_matches(texture_usage, &["g_SamplerIndex", "g_IndexSampler"]) {
        Some(WeaponModelTextureKind::Index)
    } else if sampler_usage_matches(
        texture_usage,
        &[
            "g_SamplerMask",
            "g_MaskSampler",
            "g_SamplerMaterial",
            "g_MaterialSampler",
            "g_SamplerMulti",
            "g_MultiSampler",
        ],
    ) {
        Some(WeaponModelTextureKind::Mask)
    } else if sampler_usage_matches(
        texture_usage,
        &[
            "g_SamplerSpecular",
            "g_SpecularSampler",
            "g_SamplerSpecularMap",
            "g_SpecularMapSampler",
            "g_SamplerReflect",
            "g_ReflectSampler",
        ],
    ) {
        Some(WeaponModelTextureKind::Specular)
    } else if sampler_usage_matches(
        texture_usage,
        &[
            "g_SamplerDiffuse",
            "g_DiffuseSampler",
            "g_SamplerColor",
            "g_ColorSampler",
            "g_SamplerColorMap",
            "g_ColorMapSampler",
            "g_SamplerAlbedo",
            "g_AlbedoSampler",
            "g_SamplerBaseColor",
            "g_BaseColorSampler",
        ],
    ) {
        Some(WeaponModelTextureKind::BaseColor)
    } else {
        None
    }
}

fn sampler_usage_matches(texture_usage: u32, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| physis::shpk::ShaderPackage::crc(name) == texture_usage)
}

fn classify_weapon_texture(
    path: &str,
    sampler_kind: Option<WeaponModelTextureKind>,
) -> WeaponModelTextureKind {
    let path = path.to_ascii_lowercase();
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path.as_str())
        .trim_end_matches(".tex");

    if stem.ends_with("_id") || stem.contains("_id_") || stem.contains("index") {
        return WeaponModelTextureKind::Index;
    }

    if let Some(kind) = sampler_kind {
        return kind;
    }

    if stem.ends_with("_n") || stem.contains("_n_") || stem.contains("normal") {
        WeaponModelTextureKind::Normal
    } else if stem.ends_with("_s") || stem.contains("_s_") || stem.contains("spec") {
        WeaponModelTextureKind::Specular
    } else if stem.ends_with("_m") || stem.contains("_m_") || stem.contains("mask") {
        WeaponModelTextureKind::Mask
    } else if stem.ends_with("_e") || stem.contains("_e_") || stem.contains("emit") {
        WeaponModelTextureKind::Emissive
    } else if stem.ends_with("_a")
        || stem.contains("_a_")
        || stem.ends_with("_d")
        || stem.contains("_d_")
        || stem.contains("albedo")
        || stem.contains("diff")
        || stem.contains("base")
    {
        WeaponModelTextureKind::BaseColor
    } else {
        WeaponModelTextureKind::Other
    }
}

fn merge_texture_kind(
    existing: WeaponModelTextureKind,
    incoming: WeaponModelTextureKind,
) -> WeaponModelTextureKind {
    match (existing, incoming) {
        (WeaponModelTextureKind::Other, kind) => kind,
        (kind, WeaponModelTextureKind::Other) => kind,
        (WeaponModelTextureKind::Mask, WeaponModelTextureKind::Index) => {
            WeaponModelTextureKind::Index
        }
        (WeaponModelTextureKind::Index, WeaponModelTextureKind::Mask) => {
            WeaponModelTextureKind::Index
        }
        (kind, _) => kind,
    }
}

fn checked_advance(offset: usize, byte_count: usize, len: usize) -> Option<usize> {
    let next = offset.checked_add(byte_count)?;
    (next <= len).then_some(next)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod weapon_material_tests {
    use super::*;

    #[test]
    fn parse_material_sampler_roles_uses_sampler_texture_index() {
        let mut bytes = vec![0; 16];
        bytes[12] = 2;
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&physis::shpk::ShaderPackage::crc("g_SamplerNormal").to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&[0; 3]);

        let roles = parse_material_sampler_roles(&bytes);

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].texture_index, 1);
        assert_eq!(roles[0].kind, WeaponModelTextureKind::Normal);
    }

    #[test]
    fn classify_weapon_texture_recognizes_ffxiv_albedo_suffix() {
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w1758/obj/body/b0001/texture/w1758b0001_a.tex",
                None
            ),
            WeaponModelTextureKind::BaseColor
        );
    }

    #[test]
    fn classify_weapon_texture_recognizes_colorset_index_map() {
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0525/obj/body/b0001/texture/v01_w0525b0001_id.tex",
                None
            ),
            WeaponModelTextureKind::Index
        );
    }

    #[test]
    fn fallback_base_texture_ignores_specialized_maps() {
        let textures = vec![
            test_texture("emissive.tex", WeaponModelTextureKind::Emissive),
            test_texture("normal.tex", WeaponModelTextureKind::Normal),
            test_texture("mask.tex", WeaponModelTextureKind::Mask),
            test_texture("specular.tex", WeaponModelTextureKind::Specular),
            test_texture("id.tex", WeaponModelTextureKind::Index),
        ];
        assert_eq!(choose_fallback_base_texture(&[0, 1, 2, 3], &textures), None);
    }

    #[test]
    fn fallback_base_texture_allows_unknown_maps() {
        let textures = vec![
            test_texture("normal.tex", WeaponModelTextureKind::Normal),
            test_texture("unknown.tex", WeaponModelTextureKind::Other),
        ];
        assert_eq!(choose_fallback_base_texture(&[0, 1], &textures), Some(1));
    }

    #[test]
    fn only_base_texture_alpha_affects_material_transparency() {
        for kind in [
            WeaponModelTextureKind::Normal,
            WeaponModelTextureKind::Mask,
            WeaponModelTextureKind::Specular,
            WeaponModelTextureKind::Emissive,
            WeaponModelTextureKind::Index,
            WeaponModelTextureKind::Other,
        ] {
            let texture = test_texture_with_alpha("non-base-alpha.tex", kind, 0);
            assert!(!texture_alpha_affects_material_transparency(&texture));
        }

        let opaque_base =
            test_texture_with_alpha("base-opaque.tex", WeaponModelTextureKind::BaseColor, 255);
        assert!(!texture_alpha_affects_material_transparency(&opaque_base));

        let alpha_base =
            test_texture_with_alpha("base-alpha.tex", WeaponModelTextureKind::BaseColor, 128);
        assert!(texture_alpha_affects_material_transparency(&alpha_base));
    }

    #[test]
    fn srgb_multiply_uses_linear_space() {
        assert_eq!(multiply_srgb_channels(255, 128), 128);
        assert_ne!(multiply_srgb_channels(128, 128), 64);
    }

    fn test_texture(path: &str, kind: WeaponModelTextureKind) -> WeaponModelTexture {
        test_texture_with_alpha(path, kind, 255)
    }

    fn test_texture_with_alpha(
        path: &str,
        kind: WeaponModelTextureKind,
        alpha: u8,
    ) -> WeaponModelTexture {
        WeaponModelTexture {
            path: path.to_string(),
            kind,
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, alpha],
        }
    }
}

fn weapon_texture_candidate_paths(material_path: &str, texture_path: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let texture_path = normalize_game_resource_path(texture_path);
    if texture_path.is_empty() {
        return candidates;
    }

    push_unique_path(&mut candidates, texture_path.clone());
    if texture_path.starts_with("chara/")
        || texture_path.starts_with("bg/")
        || texture_path.starts_with("ui/")
        || texture_path.starts_with("common/")
    {
        return candidates;
    }

    let material_path = normalize_game_resource_path(material_path);
    let texture_file = texture_path
        .rsplit('/')
        .next()
        .unwrap_or(texture_path.as_str());
    if let Some((object_root, material_tail)) = material_path.split_once("/material/") {
        let texture_root = format!("{object_root}/texture");
        if let Some((version, _)) = material_tail.split_once('/') {
            if version.starts_with('v') {
                push_unique_path(
                    &mut candidates,
                    format!("{texture_root}/{version}/{texture_file}"),
                );
            }
        }
        push_unique_path(&mut candidates, format!("{texture_root}/{texture_file}"));
    }

    if let Some((material_dir, _)) = material_path.rsplit_once('/') {
        push_unique_path(&mut candidates, format!("{material_dir}/{texture_file}"));
    }

    candidates
}

fn normalize_game_resource_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/").to_ascii_lowercase()
}

fn push_loaded_path(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

async fn load_from_browser_sqpack_direct(
    kind: &str,
    key: &str,
) -> Result<(Vec<u8>, Option<String>), String> {
    log::debug("resource", format!("BrowserSqPack request: {kind}/{key}"));
    let start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    match kind {
        "craft-data" => {
            let cache_fingerprint = sqpack.craft_data_cache_fingerprint().await?;
            report_craft_data_cache_status(Some(CraftDataCacheStatus::Checking));
            report_craft_data_progress(Some(CraftDataLoadProgress {
                stage: "检查本地缓存".to_string(),
                detail: "CraftData JSON".to_string(),
                current: 0,
                total: 1,
                elapsed_ms: log::elapsed_ms(start_ms),
                done: false,
            }));
            if let Some((bytes, game_version)) =
                load_cached_local_craft_data(&cache_fingerprint).await?
            {
                report_craft_data_cache_status(Some(CraftDataCacheStatus::Hit {
                    bytes: bytes.len(),
                }));
                let elapsed_ms = log::elapsed_ms(start_ms);
                log::info(
                    "resource",
                    format!(
                        "loaded CraftData from IndexedDB cache in {}",
                        log::format_elapsed(elapsed_ms)
                    ),
                );
                report_craft_data_progress(Some(CraftDataLoadProgress {
                    stage: "使用本地缓存".to_string(),
                    detail: game_version
                        .clone()
                        .unwrap_or_else(|| "CraftData JSON".to_string()),
                    current: 1,
                    total: 1,
                    elapsed_ms,
                    done: true,
                }));
                return Ok((bytes, game_version));
            }

            report_craft_data_cache_status(Some(CraftDataCacheStatus::Miss {
                reason: "fingerprint mismatch or empty".to_string(),
            }));
            log::info(
                "resource",
                "local CraftData cache miss; exporting from SqPack",
            );
            let resource = sqpack.preload_craft_data_resource().await?;
            let after_preload_ms = log::elapsed_ms(start_ms);
            report_craft_data_progress(Some(CraftDataLoadProgress {
                stage: "导出 CraftData".to_string(),
                detail: "转换本地 EXD 为应用数据".to_string(),
                current: 1,
                total: 1,
                elapsed_ms: after_preload_ms,
                done: false,
            }));
            let export_start_ms = log::now_ms();
            let generated_at = js_sys::Date::new_0()
                .to_iso_string()
                .as_string()
                .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
            let data = xiv_companion::game_data::export_craft_data_from_resource(
                resource,
                "Browser Local SqPack".to_string(),
                "Local SqPack".to_string(),
                generated_at,
            )
            .map_err(|error| format!("failed to export CraftData from local SqPack: {error:#}"))?;
            let fingerprint = Some(data.game_version.clone());
            let export_elapsed_ms = log::elapsed_ms(export_start_ms);
            let bytes = serde_json::to_vec(&data)
                .map_err(|error| format!("failed to encode local CraftData: {error}"))?;
            let total_elapsed_ms = log::elapsed_ms(start_ms);
            log::info(
                "resource",
                format!(
                    "CraftData local pipeline completed: preload={}, export={}, total={}",
                    log::format_elapsed(after_preload_ms),
                    log::format_elapsed(export_elapsed_ms),
                    log::format_elapsed(total_elapsed_ms),
                ),
            );
            report_craft_data_cache_status(Some(CraftDataCacheStatus::Saving {
                bytes: bytes.len(),
            }));
            save_cached_local_craft_data(&cache_fingerprint, &data.game_version, &bytes).await?;
            report_craft_data_cache_status(Some(CraftDataCacheStatus::Saved {
                bytes: bytes.len(),
            }));
            report_craft_data_progress(Some(CraftDataLoadProgress {
                stage: "本地 CraftData 就绪".to_string(),
                detail: format!(
                    "{} / {} items / {} recipes",
                    data.game_version, data.counts.items, data.counts.recipes
                ),
                current: 1,
                total: 1,
                elapsed_ms: total_elapsed_ms,
                done: true,
            }));
            Ok((bytes, fingerprint))
        }
        "item-icon" => {
            let icon_id = key
                .parse::<u32>()
                .map_err(|error| format!("invalid icon id {key}: {error}"))?;
            let path = item_icon_tex_path(icon_id);
            let bytes = sqpack
                .read_game_file_with_window(&path, ITEM_ICON_READ_WINDOW)
                .await?;
            Ok((bytes, None))
        }
        other => Err(format!("unknown BrowserSqPack request {other}")),
    }
}

async fn local_resource_cache_db() -> Result<indexed_db::Database<String>, String> {
    let factory =
        indexed_db::Factory::get().map_err(|error| format!("打开 IndexedDB 缓存失败: {error}"))?;
    factory
        .open(LOCAL_RESOURCE_CACHE_DB, 1, |event| async move {
            let db = event.database();
            db.build_object_store(LOCAL_RESOURCE_CACHE_STORE).create()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("打开资源缓存数据库失败: {error}"))
}

async fn load_cached_local_craft_data(
    expected_fingerprint: &str,
) -> Result<Option<(Vec<u8>, Option<String>)>, String> {
    let db = local_resource_cache_db().await?;
    let record = db
        .transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .get(&JsString::from(LOCAL_CRAFT_DATA_CACHE_KEY))
                .await
        })
        .await
        .map_err(|error| format!("读取本地 CraftData 缓存失败: {error}"))?;

    let Some(record) = record else {
        log::info("resource-cache", "CraftData cache miss: empty");
        return Ok(None);
    };

    let fingerprint = js_string_field(&record, "fingerprint");
    if fingerprint.as_deref() != Some(expected_fingerprint) {
        log::info(
            "resource-cache",
            "CraftData cache miss: fingerprint changed",
        );
        return Ok(None);
    }

    let bytes =
        js_sys::Reflect::get(&record, &JsValue::from_str("bytes")).map_err(format_js_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        log::warn("resource-cache", "CraftData cache record has no bytes");
        return Ok(None);
    }

    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
    let game_version = js_string_field(&record, "gameVersion");
    log::info(
        "resource-cache",
        format!("CraftData cache hit: {} bytes", bytes.len()),
    );
    Ok(Some((bytes, game_version)))
}

async fn save_cached_local_craft_data(
    fingerprint: &str,
    game_version: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("fingerprint"),
        &JsValue::from_str(fingerprint),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("gameVersion"),
        &JsValue::from_str(game_version),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("savedAt"),
        &js_sys::Date::new_0().to_iso_string(),
    )
    .map_err(format_js_error)?;
    let array = js_sys::Uint8Array::from(bytes);
    js_sys::Reflect::set(&object, &JsValue::from_str("bytes"), &array).map_err(format_js_error)?;

    let db = local_resource_cache_db().await?;
    db.transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .put_kv(&JsString::from(LOCAL_CRAFT_DATA_CACHE_KEY), &object)
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存本地 CraftData 缓存失败: {error}"))?;
    log::info(
        "resource-cache",
        format!("saved CraftData cache: {} bytes", bytes.len()),
    );
    Ok(())
}

async fn load_cached_local_weapon_catalog(
    expected_fingerprint: &str,
) -> Result<Option<(Vec<u8>, Option<String>)>, String> {
    let db = local_resource_cache_db().await?;
    let record = db
        .transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .get(&JsString::from(LOCAL_WEAPON_CATALOG_CACHE_KEY))
                .await
        })
        .await
        .map_err(|error| format!("读取本地 WeaponCatalog 缓存失败: {error}"))?;

    let Some(record) = record else {
        log::info("resource-cache", "WeaponCatalog cache miss: empty");
        return Ok(None);
    };

    let fingerprint = js_string_field(&record, "fingerprint");
    if fingerprint.as_deref() != Some(expected_fingerprint) {
        log::info(
            "resource-cache",
            "WeaponCatalog cache miss: fingerprint changed",
        );
        return Ok(None);
    }

    let bytes =
        js_sys::Reflect::get(&record, &JsValue::from_str("bytes")).map_err(format_js_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        log::warn("resource-cache", "WeaponCatalog cache record has no bytes");
        return Ok(None);
    }

    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
    let game_version = js_string_field(&record, "gameVersion");
    log::info(
        "resource-cache",
        format!("WeaponCatalog cache hit: {} bytes", bytes.len()),
    );
    Ok(Some((bytes, game_version)))
}

async fn save_cached_local_weapon_catalog(
    fingerprint: &str,
    game_version: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("fingerprint"),
        &JsValue::from_str(fingerprint),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("gameVersion"),
        &JsValue::from_str(game_version),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("savedAt"),
        &js_sys::Date::new_0().to_iso_string(),
    )
    .map_err(format_js_error)?;
    let array = js_sys::Uint8Array::from(bytes);
    js_sys::Reflect::set(&object, &JsValue::from_str("bytes"), &array).map_err(format_js_error)?;

    let db = local_resource_cache_db().await?;
    db.transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .put_kv(&JsString::from(LOCAL_WEAPON_CATALOG_CACHE_KEY), &object)
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存本地 WeaponCatalog 缓存失败: {error}"))?;
    log::info(
        "resource-cache",
        format!("saved WeaponCatalog cache: {} bytes", bytes.len()),
    );
    Ok(())
}

fn js_string_field(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_string())
}

fn format_js_error(error: JsValue) -> String {
    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript 调用失败".to_string())
}

pub struct BundledProvider;

impl ResourceProvider for BundledProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::Builtin
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        request.kind == CraftDataKind.into() && request.key == "default"
    }

    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
        Box::pin(async move {
            let resource_kind = request.kind.clone();
            if !self.supports(&request) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(self.source()),
                    format!("bundled provider has no resource key {}", request.key),
                ));
            }

            let bytes = dioxus::asset_resolver::read_asset_bytes(BUNDLED_CRAFT_DATA_ASSET)
                .await
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::ProviderFailed,
                        resource_kind,
                        Some(self.source()),
                        format!("failed to read bundled asset: {error}"),
                    )
                })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint: None,
            })
        })
    }
}
