use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc as WasmRc};

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[cfg(target_arch = "wasm32")]
use xiv_companion::PreparedModelOptions;

use crate::app::icons::{Icon, IconKind};
#[cfg(target_arch = "wasm32")]
use crate::app::model_canvas_renderer::WebWeaponCanvasRenderer;
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::{cx, format_integer};
use xiv_companion::renderer::{ModelDebugMode, ModelGlassBlendMode, WeaponRenderOptions};

use xiv_companion::{
    PackedModelId, WeaponCatalogItem, WeaponCatalogPackage, WeaponModelData,
    WeaponModelTextureKind, WeaponStain, weapon_slot_label,
};

use super::crafting::ItemIcon;
use crate::app::data::{load_weapon_catalog, load_weapon_model_with_stains};

const RESULT_LIMIT: usize = 220;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WeaponSlotFilter {
    All,
    Main,
    Off,
    TwoHanded,
    Dual,
}

impl WeaponSlotFilter {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Main => "main",
            Self::Off => "off",
            Self::TwoHanded => "two",
            Self::Dual => "dual",
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn from_key(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "main" => Some(Self::Main),
            "off" => Some(Self::Off),
            "two" | "two-handed" | "twohanded" => Some(Self::TwoHanded),
            "dual" => Some(Self::Dual),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Main => "主手",
            Self::Off => "副手",
            Self::TwoHanded => "双手",
            Self::Dual => "双持",
        }
    }

    fn matches(self, item: &WeaponCatalogItem) -> bool {
        match self {
            Self::All => true,
            Self::Main => item.equip_slot_category == 1,
            Self::Off => item.equip_slot_category == 2,
            Self::TwoHanded => item.equip_slot_category == 13,
            Self::Dual => item.equip_slot_category == 14,
        }
    }
}

#[derive(Clone, PartialEq)]
struct WeaponSearchResult {
    total: usize,
    items: Vec<WeaponCatalogItem>,
}

#[derive(Clone, Debug)]
struct WeaponUrlState {
    query: String,
    filter: WeaponSlotFilter,
    item_id: Option<u32>,
    stain_ids: [u8; 2],
}

#[component]
pub fn WeaponModelsPage() -> Element {
    let initial_url_state = initial_weapon_url_state();
    let initial_query = initial_url_state.query;
    let initial_filter = initial_url_state.filter;
    let initial_item_id = initial_url_state.item_id;
    let initial_stain_ids = initial_url_state.stain_ids;

    let catalog = use_resource(load_weapon_catalog);
    let mut query = use_signal(move || initial_query.clone());
    let mut slot_filter = use_signal(move || initial_filter);
    let mut selected_id = use_signal(move || initial_item_id);
    let mut selected_item = use_signal(|| None::<WeaponCatalogItem>);
    let mut stain_ids = use_signal(move || initial_stain_ids);
    let model = use_resource(move || {
        let item = selected_item();
        let stain_ids = stain_ids();
        async move {
            match item {
                Some(item) => load_weapon_model_with_stains(item, stain_ids).await,
                None => Err("未选择武器".to_string()),
            }
        }
    });

    use_effect(move || {
        let id = selected_id();
        if selected_item().as_ref().map(|item| item.id) == id {
            return;
        }

        let Some(id) = id else {
            if selected_item().is_some() {
                selected_item.set(None);
            }
            return;
        };

        if let Some(Ok(catalog)) = catalog.read().as_ref() {
            if let Some(item) = catalog.items.iter().find(|item| item.id == id).cloned() {
                selected_item.set(Some(item));
            }
        }
    });

    use_effect(move || {
        sync_weapon_url_state(&query(), slot_filter(), selected_id(), stain_ids());
    });

    let catalog_snapshot = catalog.read().as_ref().cloned();
    let selected_snapshot = selected_item();
    let selected_id_snapshot = selected_id();
    let query_snapshot = query();
    let slot_filter_snapshot = slot_filter();
    let stain_ids_snapshot = stain_ids();

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-4 sm:px-6 lg:px-8",
                div { class: "flex flex-wrap items-end justify-between gap-3",
                    div { class: "min-w-0",
                        div { class: "text-sm text-muted-foreground", "预览" }
                        h1 { class: "text-2xl font-semibold", "武器模型" }
                    }
                    if let Some(Ok(catalog)) = &catalog_snapshot {
                        div { class: "flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                            Badge { variant: BadgeVariant::Outline, "UserLocal" }
                            span { "{catalog.game_version}" }
                            span { "{format_integer(catalog.counts.items as f64)} 件武器" }
                            span { "{format_integer(catalog.counts.stains as f64)} 种染剂" }
                        }
                    }
                }
            }

            match catalog_snapshot {
                None => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        div { class: "flex items-center gap-3 text-sm text-muted-foreground",
                            Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                            "正在读取本地武器目录"
                        }
                    }
                },
                Some(Err(error)) => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::Database, class: "h-6 w-6" } },
                            title: "UserLocal 未就绪".to_string(),
                            description: Some(error),
                            action: rsx! {
                                a { href: "#/",
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        size: ButtonSize::Sm,
                                        Icon { kind: IconKind::Database, class: "h-4 w-4" }
                                        "数据来源"
                                    }
                                }
                            },
                        }
                    }
                },
                Some(Ok(catalog)) => {
                    let search = search_weapons(&catalog, &query_snapshot, slot_filter_snapshot);
                    rsx! {
                        div { class: "grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[380px_minmax(0,1fr)]",
                            WeaponSearchPane {
                                catalog: catalog.clone(),
                                query: query_snapshot,
                                filter: slot_filter_snapshot,
                                result: search,
                                selected_id: selected_id_snapshot,
                                on_query_change: move |value| query.set(value),
                                on_filter_change: move |value| slot_filter.set(value),
                                on_select: move |item: WeaponCatalogItem| {
                                    selected_id.set(Some(item.id));
                                    selected_item.set(Some(item));
                                },
                            }
                            WeaponModelPane {
                                selected: selected_snapshot,
                                model,
                                stains: catalog.stains.clone(),
                                stain_ids: stain_ids_snapshot,
                                on_stain_change: move |(channel, stain_id): (usize, u8)| {
                                    let mut next = stain_ids();
                                    if let Some(value) = next.get_mut(channel) {
                                        *value = stain_id;
                                        stain_ids.set(next);
                                    }
                                },
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn WeaponSearchPane(
    catalog: Rc<WeaponCatalogPackage>,
    query: String,
    filter: WeaponSlotFilter,
    result: WeaponSearchResult,
    selected_id: Option<u32>,
    on_query_change: EventHandler<String>,
    on_filter_change: EventHandler<WeaponSlotFilter>,
    on_select: EventHandler<WeaponCatalogItem>,
) -> Element {
    rsx! {
        aside { class: "flex min-h-0 flex-col border-b bg-card lg:border-b-0 lg:border-r",
            div { class: "shrink-0 space-y-3 border-b p-4",
                div { class: "relative",
                    Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                    input {
                        class: input_class("pl-9"),
                        value: "{query}",
                        placeholder: "名称 / 物品 ID / 模型 ID",
                        oninput: move |event| on_query_change.call(event.value()),
                    }
                }

                div { class: "grid grid-cols-5 gap-1 rounded-md bg-muted p-1",
                    for option in [
                        WeaponSlotFilter::All,
                        WeaponSlotFilter::Main,
                        WeaponSlotFilter::Off,
                        WeaponSlotFilter::TwoHanded,
                        WeaponSlotFilter::Dual,
                    ] {
                        button {
                            r#type: "button",
                            class: segment_button_class(filter == option),
                            onclick: move |_| on_filter_change.call(option),
                            "{option.label()}"
                        }
                    }
                }

                div { class: "flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground",
                    span { "{format_integer(result.total as f64)} / {format_integer(catalog.counts.items as f64)}" }
                    if result.total > result.items.len() {
                        span { "显示前 {RESULT_LIMIT}" }
                    }
                }
            }

            div { class: "min-h-0 flex-1 overflow-y-auto p-2",
                if result.items.is_empty() {
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                        title: "没有匹配的武器".to_string(),
                    }
                } else {
                    div { class: "space-y-1",
                        for item in result.items {
                            WeaponListRow {
                                key: "{item.id}",
                                active: selected_id == Some(item.id),
                                item,
                                on_select,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn WeaponListRow(
    item: WeaponCatalogItem,
    active: bool,
    on_select: EventHandler<WeaponCatalogItem>,
) -> Element {
    let row_item = item.clone();
    rsx! {
        button {
            r#type: "button",
            class: cx([
                "flex w-full min-w-0 items-center gap-3 rounded-md border px-2.5 py-2 text-left transition-colors",
                if active {
                    "border-foreground/20 bg-background shadow-sm"
                } else {
                    "border-transparent hover:border-border hover:bg-background/70"
                },
            ]),
            onclick: move |_| on_select.call(row_item.clone()),
            ItemIcon { icon: item.icon, size: "sm" }
            div { class: "min-w-0 flex-1",
                div { class: "truncate text-sm font-medium", "{item.name}" }
                div { class: "mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground",
                    span { "#{item.id}" }
                    span { "{weapon_slot_label(item.equip_slot_category)}" }
                    span { "{format_packed_model(PackedModelId::from_raw(item.model_main))}" }
                }
            }
        }
    }
}

#[component]
fn WeaponModelPane(
    selected: Option<WeaponCatalogItem>,
    model: Resource<Result<Rc<WeaponModelData>, String>>,
    stains: Vec<WeaponStain>,
    stain_ids: [u8; 2],
    on_stain_change: EventHandler<(usize, u8)>,
) -> Element {
    let render_options = use_signal(WeaponRenderOptions::default);
    let mut shape_selection = use_signal(|| (None::<u32>, None::<u32>));

    rsx! {
        section { class: "flex min-h-0 min-w-0 flex-col overflow-hidden bg-background",
            if let Some(item) = selected.clone() {
                div { class: "shrink-0 border-b p-4",
                    div { class: "flex min-w-0 flex-wrap items-center gap-3",
                        ItemIcon { icon: item.icon }
                        div { class: "min-w-0 flex-1",
                            div { class: "truncate text-base font-semibold", "{item.name}" }
                            div { class: "mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                                Badge { variant: BadgeVariant::Outline, "#{item.id}" }
                                Badge { variant: BadgeVariant::Secondary, "{weapon_slot_label(item.equip_slot_category)}" }
                                span { "main {format_packed_model(item.primary_model())}" }
                                if let Some(sub) = item.secondary_model() {
                                    span { "sub {format_packed_model(sub)}" }
                                }
                            }
                        }
                    }
                    if !item.description.trim().is_empty() {
                        div { class: "mt-3 max-w-3xl text-sm leading-relaxed text-muted-foreground",
                            "{item.description}"
                        }
                    }
                    WeaponStainControls {
                        stains,
                        stain_ids,
                        on_stain_change,
                    }
                }

                div { class: "flex min-h-0 flex-1 flex-col overflow-hidden xl:flex-row",
                    div { class: "relative min-h-0 min-w-0 flex-1 overflow-hidden bg-[#0e1117]",
                        match model.read().as_ref() {
                            Some(Ok(data))
                                if data.item_id == item.id && data.stain_ids == stain_ids => {
                                let requested_shape = shape_selection();
                                let shape_mask = (requested_shape.0 == Some(item.id))
                                    .then_some(requested_shape.1)
                                    .flatten()
                                    .filter(|mask| model_has_shape_mask(data, *mask));
                                let key = model_canvas_key(data, shape_mask);
                                let item_id = item.id;
                                rsx! {
                                    div { key: "{key}", class: "absolute inset-0",
                                        WeaponModelCanvas {
                                            model: data.clone(),
                                            render_options,
                                            shape_mask,
                                        }
                                        WeaponRenderControls {
                                            options: render_options,
                                            model: data.clone(),
                                            shape_mask,
                                            on_shape_change: move |mask| {
                                                shape_selection.set((Some(item_id), mask));
                                            },
                                        }
                                    }
                                }
                            },
                            Some(Err(error)) => rsx! {
                                div { class: "absolute inset-0 flex items-center justify-center p-6",
                                    EmptyState {
                                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                        title: "模型读取失败".to_string(),
                                        description: Some(error.clone()),
                                    }
                                }
                            },
                            _ => rsx! {
                                div { class: "absolute inset-0 flex items-center justify-center",
                                    div { class: "flex items-center gap-3 text-sm text-muted-foreground",
                                        Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                                        "正在读取模型"
                                    }
                                }
                            },
                        }
                    }

                    aside { class: "h-56 shrink-0 overflow-y-auto border-t bg-card p-4 xl:h-auto xl:w-80 xl:border-l xl:border-t-0",
                        match model.read().as_ref() {
                            Some(Ok(data))
                                if data.item_id == item.id && data.stain_ids == stain_ids => rsx! {
                                WeaponModelStats { model: data.clone() }
                            },
                            _ => rsx! {
                                div { class: "space-y-3",
                                    SkeletonLine {}
                                    SkeletonLine {}
                                    SkeletonLine {}
                                }
                            },
                        }
                    }
                }
            } else {
                div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::Sword, class: "h-6 w-6" } },
                        title: "未选择武器".to_string(),
                    }
                }
            }
        }
    }
}

#[component]
fn WeaponModelStats(model: Rc<WeaponModelData>) -> Element {
    let mesh_count = model.meshes.len();
    let material_count = model.materials.len();
    let texture_count = model.textures.len();
    let texture_counts = TextureKindCounts::from_model(&model);
    let vertex_count: usize = model.meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let index_count: usize = model.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    let bounds = model.bounds;

    rsx! {
        div { class: "space-y-5",
            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "模型" }
                StatRow { label: "Mesh", value: format_integer(mesh_count as f64) }
                StatRow { label: "Material", value: format_integer(material_count as f64) }
                StatRow { label: "Texture", value: format_integer(texture_count as f64) }
                StatRow { label: "Vertex", value: format_integer(vertex_count as f64) }
                StatRow { label: "Index", value: format_integer(index_count as f64) }
                StatRow { label: "Radius", value: format!("{:.3}", bounds.radius) }
            }

            if texture_count > 0 {
                section { class: "space-y-2",
                    div { class: "text-sm font-semibold", "Textures" }
                    StatRow { label: "Base", value: texture_counts.base.to_string() }
                    StatRow { label: "Base Map 1", value: texture_counts.secondary_base.to_string() }
                    StatRow { label: "Normal", value: texture_counts.normal.to_string() }
                    StatRow { label: "Normal Map 1", value: texture_counts.secondary_normal.to_string() }
                    StatRow { label: "Mask", value: texture_counts.mask.to_string() }
                    StatRow { label: "Material Map", value: texture_counts.material_map.to_string() }
                    StatRow { label: "Multi Map", value: texture_counts.multi_map.to_string() }
                    StatRow { label: "Specular", value: texture_counts.specular.to_string() }
                    StatRow { label: "Specular Map 1", value: texture_counts.secondary_specular.to_string() }
                    StatRow { label: "Material Props", value: texture_counts.material_properties.to_string() }
                    StatRow { label: "Tile Props", value: texture_counts.tile_properties.to_string() }
                    StatRow { label: "Sheen Props", value: texture_counts.sheen_properties.to_string() }
                    StatRow { label: "Sphere Props", value: texture_counts.sphere_properties.to_string() }
                    StatRow { label: "Tile Matrix", value: texture_counts.tile_matrix.to_string() }
                    StatRow { label: "Emissive", value: texture_counts.emissive.to_string() }
                    StatRow { label: "Environment", value: texture_counts.environment.to_string() }
                    StatRow { label: "Index", value: texture_counts.index.to_string() }
                    StatRow { label: "Tile Normal Array", value: texture_counts.tile_normal_array.to_string() }
                    StatRow { label: "Tile ORB Array", value: texture_counts.tile_orb_array.to_string() }
                    StatRow { label: "Detail Diffuse Array", value: texture_counts.detail_diffuse_array.to_string() }
                    StatRow { label: "Detail Normal Array", value: texture_counts.detail_normal_array.to_string() }
                    StatRow { label: "Water Wave", value: texture_counts.water_wave.to_string() }
                    StatRow { label: "Water Wave 1", value: texture_counts.water_wave1.to_string() }
                    StatRow { label: "Water Whitecap", value: texture_counts.water_whitecap.to_string() }
                    StatRow { label: "Other", value: texture_counts.other.to_string() }
                }
            }

            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "Bounds" }
                StatRow { label: "Min", value: format_vec3(bounds.min) }
                StatRow { label: "Max", value: format_vec3(bounds.max) }
                StatRow { label: "Center", value: format_vec3(bounds.center) }
            }

            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "SqPack" }
                for path in model.loaded_paths.clone() {
                    div { class: "break-all rounded-md border bg-background px-2 py-1.5 font-mono text-[11px] text-muted-foreground",
                        "{path}"
                    }
                }
            }
        }
    }
}

#[component]
fn StatRow(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-3 border-b border-border/60 py-1.5 text-xs last:border-b-0",
            span { class: "text-muted-foreground", "{label}" }
            span { class: "min-w-0 truncate font-medium", "{value}" }
        }
    }
}

#[derive(Default)]
struct TextureKindCounts {
    base: usize,
    secondary_base: usize,
    normal: usize,
    secondary_normal: usize,
    mask: usize,
    material_map: usize,
    multi_map: usize,
    specular: usize,
    secondary_specular: usize,
    material_properties: usize,
    tile_properties: usize,
    sheen_properties: usize,
    sphere_properties: usize,
    tile_matrix: usize,
    emissive: usize,
    environment: usize,
    index: usize,
    tile_normal_array: usize,
    tile_orb_array: usize,
    detail_diffuse_array: usize,
    detail_normal_array: usize,
    water_wave: usize,
    water_wave1: usize,
    water_whitecap: usize,
    other: usize,
}

impl TextureKindCounts {
    fn from_model(model: &WeaponModelData) -> Self {
        let mut counts = Self::default();
        for texture in &model.textures {
            match texture.kind {
                WeaponModelTextureKind::BaseColor => counts.base += 1,
                WeaponModelTextureKind::SecondaryBaseColor => counts.secondary_base += 1,
                WeaponModelTextureKind::Normal => counts.normal += 1,
                WeaponModelTextureKind::SecondaryNormal => counts.secondary_normal += 1,
                WeaponModelTextureKind::Mask => counts.mask += 1,
                WeaponModelTextureKind::MaterialMap => counts.material_map += 1,
                WeaponModelTextureKind::MultiMap => counts.multi_map += 1,
                WeaponModelTextureKind::Specular => counts.specular += 1,
                WeaponModelTextureKind::SecondarySpecular => counts.secondary_specular += 1,
                WeaponModelTextureKind::MaterialProperties => counts.material_properties += 1,
                WeaponModelTextureKind::TileProperties => counts.tile_properties += 1,
                WeaponModelTextureKind::SheenProperties => counts.sheen_properties += 1,
                WeaponModelTextureKind::SphereProperties => counts.sphere_properties += 1,
                WeaponModelTextureKind::TileMatrixProperties => counts.tile_matrix += 1,
                WeaponModelTextureKind::Emissive => counts.emissive += 1,
                WeaponModelTextureKind::Environment => counts.environment += 1,
                WeaponModelTextureKind::Index => counts.index += 1,
                WeaponModelTextureKind::TileNormalArray => counts.tile_normal_array += 1,
                WeaponModelTextureKind::TileOrbArray => counts.tile_orb_array += 1,
                WeaponModelTextureKind::DetailDiffuseArray => counts.detail_diffuse_array += 1,
                WeaponModelTextureKind::DetailNormalArray => counts.detail_normal_array += 1,
                WeaponModelTextureKind::WaterWave => counts.water_wave += 1,
                WeaponModelTextureKind::WaterWaveSecondary => counts.water_wave1 += 1,
                WeaponModelTextureKind::WaterWhitecap => counts.water_whitecap += 1,
                WeaponModelTextureKind::Other => counts.other += 1,
            }
        }
        counts
    }
}

#[component]
fn SkeletonLine() -> Element {
    rsx! {
        div { class: "h-8 animate-pulse rounded-md bg-muted" }
    }
}

#[component]
fn WeaponRenderControls(
    options: Signal<WeaponRenderOptions>,
    model: Rc<WeaponModelData>,
    shape_mask: Option<u32>,
    on_shape_change: EventHandler<Option<u32>>,
) -> Element {
    let current = options();
    let bloom_percent = (current.bloom_strength * 100.0).round() as i32;
    let debug_select_class = input_class("h-8 cursor-pointer py-1 text-xs");
    let shape_select_class = input_class("h-8 w-24 cursor-pointer py-1 text-xs");
    let shape_options = model_shape_options(&model);

    rsx! {
        div {
            class: "absolute right-2 top-4 z-10 rounded-md border border-border bg-background/90 p-3 text-xs shadow-md backdrop-blur",
            style: "width: 14rem;",
            div { class: "mb-2 flex items-center justify-between gap-3",
                span { class: "font-medium", "渲染" }
                span { class: "text-[11px] text-muted-foreground", "{bloom_percent}%" }
            }
            div { class: "space-y-2",
                if !shape_options.is_empty() {
                    label { class: "flex items-center justify-between gap-3",
                        span { class: "text-muted-foreground", "Shape" }
                        select {
                            class: "{shape_select_class}",
                            value: "{shape_mask.unwrap_or(0)}",
                            onchange: move |event| {
                                let mask = event.value().parse::<u32>().ok().filter(|mask| *mask != 0);
                                on_shape_change.call(mask);
                            },
                            option { value: "0", "Base" }
                            for (_, mask, name) in shape_options.clone() {
                                option { value: "{mask}", "{name}" }
                            }
                        }
                    }
                }
                select {
                    class: "{debug_select_class}",
                    value: "{debug_mode_value(current.debug_mode)}",
                    onchange: move |event| {
                        let mut next = options();
                        next.debug_mode = parse_debug_mode(&event.value());
                        options.set(next);
                    },
                    option { value: "final", "Final" }
                    option { value: "base", "Base" }
                    option { value: "normal", "Normal" }
                    option { value: "mask", "Mask" }
                    option { value: "material", "Material" }
                    option { value: "specular", "Specular" }
                    option { value: "emissive", "Emissive" }
                    option { value: "alpha", "Alpha" }
                    option { value: "uv0", "UV0" }
                    option { value: "uv1", "UV1" }
                    option { value: "uv2", "UV2" }
                    option { value: "uv3", "UV3" }
                    option { value: "vertex", "Vertex" }
                    option { value: "vertex1", "Vertex 1" }
                    option { value: "normal1", "Normal 1" }
                    option { value: "flow0", "Flow 0" }
                    option { value: "flow1", "Flow 1" }
                    option { value: "mesh", "Mesh" }
                    option { value: "ct-index", "CT Index" }
                    option { value: "material-map", "Mat Map" }
                    option { value: "multi-map", "Multi" }
                    option { value: "tile-props", "Tile" }
                    option { value: "sheen-props", "Sheen" }
                    option { value: "sphere-props", "Sphere" }
                    option { value: "tile-matrix", "Tile Matrix" }
                    option { value: "tile-normal-array", "Tile Normal" }
                    option { value: "tile-orb-array", "Tile ORB" }
                    option { value: "detail-diffuse-array", "Detail Diffuse" }
                    option { value: "detail-normal-array", "Detail Normal" }
                }
                label { class: "flex items-center justify-between gap-3",
                    span { class: "text-muted-foreground", "Glass" }
                    select {
                        class: "{input_class(\"h-8 w-24 cursor-pointer py-1 text-xs\")}",
                        value: "{glass_blend_mode_value(current.glass_blend_mode)}",
                        onchange: move |event| {
                            let mut next = options();
                            next.glass_blend_mode = parse_glass_blend_mode(&event.value());
                            options.set(next);
                        },
                        option { value: "multiply", "Mul" }
                        option { value: "additive", "Add" }
                    }
                }
                RenderCheckbox {
                    label: "Normal",
                    checked: current.normal_mapping,
                    on_change: move |checked| {
                        let mut next = options();
                        next.normal_mapping = checked;
                        options.set(next);
                    },
                }
                RenderCheckbox {
                    label: "Bloom",
                    checked: current.bloom,
                    on_change: move |checked| {
                        let mut next = options();
                        next.bloom = checked;
                        options.set(next);
                    },
                }
                RenderCheckbox {
                    label: "Flip Y",
                    checked: current.normal_y_sign < 0.0,
                    on_change: move |checked| {
                        let mut next = options();
                        next.normal_y_sign = if checked { -1.0 } else { 1.0 };
                        options.set(next);
                    },
                }
                input {
                    class: "h-4 w-full cursor-pointer accent-foreground",
                    r#type: "range",
                    min: "0",
                    max: "160",
                    step: "5",
                    value: "{bloom_percent}",
                    disabled: !current.bloom,
                    oninput: move |event| {
                        let mut next = options();
                        next.bloom_strength = parse_render_slider_value(&event.value()) / 100.0;
                        options.set(next);
                    },
                }
            }
        }
    }
}

#[component]
fn RenderCheckbox(label: &'static str, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "flex items-center justify-between gap-3",
            span { class: "text-muted-foreground", "{label}" }
            input {
                class: "h-4 w-4 accent-foreground",
                r#type: "checkbox",
                checked,
                onchange: move |event| on_change.call(event.checked()),
            }
        }
    }
}

fn parse_render_slider_value(value: &str) -> f32 {
    value.parse::<f32>().unwrap_or(0.0).clamp(0.0, 160.0)
}

#[component]
fn WeaponStainControls(
    stains: Vec<WeaponStain>,
    stain_ids: [u8; 2],
    on_stain_change: EventHandler<(usize, u8)>,
) -> Element {
    rsx! {
        div { class: "mt-3 flex flex-wrap items-end gap-3 border-t pt-3",
            div { class: "pb-2 text-xs font-medium text-muted-foreground", "染色" }
            WeaponStainControl {
                label: "通道 1",
                stains: stains.clone(),
                value: stain_ids[0],
                onchange: move |value| on_stain_change.call((0, value)),
            }
            WeaponStainControl {
                label: "通道 2",
                stains,
                value: stain_ids[1],
                onchange: move |value| on_stain_change.call((1, value)),
            }
        }
    }
}

#[component]
fn WeaponStainControl(
    label: &'static str,
    stains: Vec<WeaponStain>,
    value: u8,
    onchange: EventHandler<u8>,
) -> Element {
    let selected = stains.iter().find(|stain| stain.id == value);
    let swatch_style = selected
        .map(|stain| {
            format!(
                "background-color: rgb({}, {}, {});",
                stain.ui_color[0], stain.ui_color[1], stain.ui_color[2]
            )
        })
        .unwrap_or_else(|| "background-color: transparent;".to_string());
    let swatch_title = selected
        .map(|stain| stain.name.clone())
        .unwrap_or_else(|| "无染色".to_string());
    let select_class = input_class("h-9 min-w-40 cursor-pointer py-1 text-xs");

    rsx! {
        label { class: "min-w-0 space-y-1",
            span { class: "block text-[11px] text-muted-foreground", "{label}" }
            div { class: "flex items-center gap-2",
                span {
                    class: "h-7 w-7 shrink-0 rounded border border-border shadow-sm",
                    style: "{swatch_style}",
                    title: "{swatch_title}",
                }
                select {
                    class: "{select_class}",
                    value: "{value}",
                    onchange: move |event| {
                        onchange.call(parse_stain_id(&event.value()));
                    },
                    option { value: "0", "无染色" }
                    for stain in stains {
                        option { value: "{stain.id}",
                            if stain.metallic {
                                "{stain.name} · 金属"
                            } else {
                                "{stain.name}"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn debug_mode_value(mode: ModelDebugMode) -> &'static str {
    match mode {
        ModelDebugMode::Final => "final",
        ModelDebugMode::BaseColor => "base",
        ModelDebugMode::Normal => "normal",
        ModelDebugMode::Mask => "mask",
        ModelDebugMode::MaterialProperties => "material",
        ModelDebugMode::Specular => "specular",
        ModelDebugMode::Emissive => "emissive",
        ModelDebugMode::Alpha => "alpha",
        ModelDebugMode::Uv0 => "uv0",
        ModelDebugMode::Uv1 => "uv1",
        ModelDebugMode::Uv2 => "uv2",
        ModelDebugMode::Uv3 => "uv3",
        ModelDebugMode::VertexColor => "vertex",
        ModelDebugMode::MeshRole => "mesh",
        ModelDebugMode::ColorTableIndex => "ct-index",
        ModelDebugMode::MaterialMap => "material-map",
        ModelDebugMode::MultiMap => "multi-map",
        ModelDebugMode::TileProperties => "tile-props",
        ModelDebugMode::SheenProperties => "sheen-props",
        ModelDebugMode::SphereProperties => "sphere-props",
        ModelDebugMode::TileMatrix => "tile-matrix",
        ModelDebugMode::TileNormalArray => "tile-normal-array",
        ModelDebugMode::TileOrbArray => "tile-orb-array",
        ModelDebugMode::DetailDiffuseArray => "detail-diffuse-array",
        ModelDebugMode::DetailNormalArray => "detail-normal-array",
        ModelDebugMode::VertexColor1 => "vertex1",
        ModelDebugMode::SecondaryNormal => "normal1",
        ModelDebugMode::Flow0 => "flow0",
        ModelDebugMode::Flow1 => "flow1",
    }
}

fn parse_debug_mode(value: &str) -> ModelDebugMode {
    match value {
        "base" => ModelDebugMode::BaseColor,
        "normal" => ModelDebugMode::Normal,
        "mask" => ModelDebugMode::Mask,
        "material" => ModelDebugMode::MaterialProperties,
        "specular" => ModelDebugMode::Specular,
        "emissive" => ModelDebugMode::Emissive,
        "alpha" => ModelDebugMode::Alpha,
        "uv0" => ModelDebugMode::Uv0,
        "uv1" => ModelDebugMode::Uv1,
        "uv2" => ModelDebugMode::Uv2,
        "uv3" => ModelDebugMode::Uv3,
        "vertex" => ModelDebugMode::VertexColor,
        "mesh" => ModelDebugMode::MeshRole,
        "ct-index" => ModelDebugMode::ColorTableIndex,
        "material-map" => ModelDebugMode::MaterialMap,
        "multi-map" => ModelDebugMode::MultiMap,
        "tile-props" => ModelDebugMode::TileProperties,
        "sheen-props" => ModelDebugMode::SheenProperties,
        "sphere-props" => ModelDebugMode::SphereProperties,
        "tile-matrix" => ModelDebugMode::TileMatrix,
        "tile-normal-array" => ModelDebugMode::TileNormalArray,
        "tile-orb-array" => ModelDebugMode::TileOrbArray,
        "detail-diffuse-array" => ModelDebugMode::DetailDiffuseArray,
        "detail-normal-array" => ModelDebugMode::DetailNormalArray,
        "vertex1" => ModelDebugMode::VertexColor1,
        "normal1" => ModelDebugMode::SecondaryNormal,
        "flow0" => ModelDebugMode::Flow0,
        "flow1" => ModelDebugMode::Flow1,
        _ => ModelDebugMode::Final,
    }
}

fn glass_blend_mode_value(mode: ModelGlassBlendMode) -> &'static str {
    match mode {
        ModelGlassBlendMode::Multiply => "multiply",
        ModelGlassBlendMode::Additive => "additive",
    }
}

fn parse_glass_blend_mode(value: &str) -> ModelGlassBlendMode {
    match value {
        "additive" => ModelGlassBlendMode::Additive,
        _ => ModelGlassBlendMode::Multiply,
    }
}

#[component]
fn WeaponModelCanvas(
    model: Rc<WeaponModelData>,
    render_options: Signal<WeaponRenderOptions>,
    shape_mask: Option<u32>,
) -> Element {
    let canvas_id = format!(
        "weapon-model-canvas-{}-{}-{}-{}-{}-{}",
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
        model.stain_ids[0],
        model.stain_ids[1],
        shape_mask.unwrap_or(0),
    );
    let init_error = use_signal(|| None::<String>);

    #[cfg(target_arch = "wasm32")]
    {
        let effect_canvas_id = canvas_id.clone();
        let effect_model = model.clone();
        let effect_shape_mask = shape_mask;
        let mut effect_error = init_error;
        use_effect(move || {
            let canvas_id = effect_canvas_id.clone();
            let model = effect_model.clone();
            let options = render_options;
            effect_error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let window =
                        web_sys::window().ok_or_else(|| "当前运行环境没有 window".to_string())?;
                    let document = window
                        .document()
                        .ok_or_else(|| "当前运行环境没有 document".to_string())?;
                    let canvas = document
                        .get_element_by_id(&canvas_id)
                        .ok_or_else(|| "canvas 未挂载".to_string())?
                        .dyn_into::<HtmlCanvasElement>()
                        .map_err(|_| "canvas 元素类型错误".to_string())?;
                    let prepared_options = effect_shape_mask
                        .map(|mask| PreparedModelOptions::default().with_enabled_shape_mask(mask))
                        .unwrap_or_default();
                    WebWeaponCanvasRenderer::from_canvas(canvas, &model, prepared_options).await
                }
                .await;

                match result {
                    Ok(renderer) => {
                        start_weapon_render_loop(WasmRc::new(RefCell::new(renderer)), options)
                    }
                    Err(error) => effect_error.set(Some(error)),
                }
            });
        });
    }

    rsx! {
        div { class: "absolute inset-0",
            canvas {
                id: "{canvas_id}",
                class: "h-full w-full cursor-grab touch-none select-none bg-[#0e1117] active:cursor-grabbing",
            }
            if let Some(error) = init_error() {
                div { class: "absolute inset-x-4 top-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-900 shadow-sm",
                    "{error}"
                }
            }
            if !cfg!(target_arch = "wasm32") {
                div { class: "absolute inset-0 flex items-center justify-center p-6 text-sm text-muted-foreground",
                    "WebGPU canvas 仅在 wasm32 web 构建中启用"
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn start_weapon_render_loop(
    renderer: WasmRc<RefCell<WebWeaponCanvasRenderer>>,
    render_options: Signal<WeaponRenderOptions>,
) {
    let callback_slot: WasmRc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
        WasmRc::new(RefCell::new(None));
    let callback_slot_for_loop = callback_slot.clone();
    let renderer_for_loop = renderer.clone();

    *callback_slot.borrow_mut() = Some(Closure::wrap(Box::new(move |time_ms: f64| {
        let connected = {
            let mut renderer = renderer_for_loop.borrow_mut();
            if renderer.canvas_connected() {
                let mut options = render_options();
                options.uv_scroll_time = (time_ms as f32) / 1000.0;
                renderer.render_with_options(options);
                true
            } else {
                false
            }
        };

        if connected {
            if let (Some(window), Some(callback)) =
                (web_sys::window(), callback_slot_for_loop.borrow().as_ref())
            {
                let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            }
        } else {
            let _ = callback_slot_for_loop.borrow_mut().take();
        }
    }) as Box<dyn FnMut(f64)>));

    if let (Some(window), Some(callback)) = (web_sys::window(), callback_slot.borrow().as_ref()) {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

fn search_weapons(
    catalog: &WeaponCatalogPackage,
    query: &str,
    filter: WeaponSlotFilter,
) -> WeaponSearchResult {
    let needle = query.trim().to_lowercase();
    let mut total = 0;
    let mut items = Vec::new();

    for item in &catalog.items {
        if !filter.matches(item) || !weapon_matches_query(item, &needle) {
            continue;
        }
        total += 1;
        if items.len() < RESULT_LIMIT {
            items.push(item.clone());
        }
    }

    WeaponSearchResult { total, items }
}

fn weapon_matches_query(item: &WeaponCatalogItem, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    let main = item.primary_model();
    let sub = item.secondary_model();
    item.name.to_lowercase().contains(needle)
        || item.id.to_string().contains(needle)
        || item.model_main.to_string().contains(needle)
        || item.model_sub.to_string().contains(needle)
        || format!("{:x}", item.model_main).contains(needle)
        || format!("{:x}", item.model_sub).contains(needle)
        || main.model_id.to_string().contains(needle)
        || main.body_id.to_string().contains(needle)
        || main.variant_id.to_string().contains(needle)
        || sub
            .map(|value| {
                value.model_id.to_string().contains(needle)
                    || value.body_id.to_string().contains(needle)
                    || value.variant_id.to_string().contains(needle)
            })
            .unwrap_or(false)
}

fn initial_weapon_url_state() -> WeaponUrlState {
    #[cfg(target_arch = "wasm32")]
    {
        weapon_url_state_from_hash(
            web_sys::window()
                .and_then(|window| window.location().hash().ok())
                .unwrap_or_default()
                .as_str(),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        WeaponUrlState {
            query: String::new(),
            filter: WeaponSlotFilter::All,
            item_id: None,
            stain_ids: [0, 0],
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn weapon_url_state_from_hash(hash: &str) -> WeaponUrlState {
    let mut state = WeaponUrlState {
        query: String::new(),
        filter: WeaponSlotFilter::All,
        item_id: None,
        stain_ids: [0, 0],
    };

    let route = hash.trim_start_matches('#');
    let Some((_, query)) = route.split_once('?') else {
        return state;
    };

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = decode_query_value(value);
        match key {
            "q" | "query" | "search" => state.query = value,
            "f" | "filter" | "slot" => {
                if let Some(filter) = WeaponSlotFilter::from_key(&value) {
                    state.filter = filter;
                }
            }
            "item" | "itemId" | "id" => {
                state.item_id = value.parse::<u32>().ok();
            }
            "stain0" | "dye0" => state.stain_ids[0] = parse_stain_id(&value),
            "stain1" | "dye1" => state.stain_ids[1] = parse_stain_id(&value),
            _ => {}
        }
    }

    state
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn decode_query_value(value: &str) -> String {
    urlencoding::decode(value)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| value.to_string())
}

fn parse_stain_id(value: &str) -> u8 {
    value
        .parse::<u8>()
        .ok()
        .filter(|stain_id| *stain_id <= 254)
        .unwrap_or(0)
}

#[allow(unused_variables)]
fn sync_weapon_url_state(
    query: &str,
    filter: WeaponSlotFilter,
    item_id: Option<u32>,
    stain_ids: [u8; 2],
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return;
        };

        let mut params = Vec::new();
        let trimmed_query = query.trim();
        if !trimmed_query.is_empty() {
            params.push(format!("q={}", urlencoding::encode(trimmed_query)));
        }
        if filter != WeaponSlotFilter::All {
            params.push(format!("f={}", filter.key()));
        }
        if let Some(item_id) = item_id {
            params.push(format!("item={item_id}"));
        }
        if stain_ids[0] != 0 {
            params.push(format!("stain0={}", stain_ids[0]));
        }
        if stain_ids[1] != 0 {
            params.push(format!("stain1={}", stain_ids[1]));
        }

        let hash = if params.is_empty() {
            "#/weapon-models".to_string()
        } else {
            format!("#/weapon-models?{}", params.join("&"))
        };

        if window.location().hash().ok().as_deref() == Some(hash.as_str()) {
            return;
        }

        match window.history() {
            Ok(history) => {
                let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&hash));
            }
            Err(_) => {
                let _ = window.location().set_hash(hash.trim_start_matches('#'));
            }
        }
    }
}

fn segment_button_class(active: bool) -> &'static str {
    if active {
        "flex h-8 items-center justify-center rounded bg-background text-xs font-medium text-foreground shadow-sm transition-colors"
    } else {
        "flex h-8 items-center justify-center rounded text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
    }
}

fn format_packed_model(model: PackedModelId) -> String {
    format!(
        "w{:04} b{:04} v{:04}",
        model.model_id, model.body_id, model.variant_id,
    )
}

fn format_vec3(value: [f32; 3]) -> String {
    format!("{:.3}, {:.3}, {:.3}", value[0], value[1], value[2])
}

fn model_canvas_key(model: &WeaponModelData, shape_mask: Option<u32>) -> String {
    format!(
        "{}-{}-{}-{}-{}-{}",
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
        model.stain_ids[0],
        model.stain_ids[1],
        shape_mask.unwrap_or(0),
    )
}

fn model_shape_options(model: &WeaponModelData) -> Vec<(usize, u32, String)> {
    let mut options = Vec::new();
    for shape in model
        .meshes
        .iter()
        .flat_map(|mesh| mesh.shape_influences.iter())
    {
        if shape.shape_index_mask == 0
            || options
                .iter()
                .any(|(_, mask, _)| *mask == shape.shape_index_mask)
        {
            continue;
        }
        options.push((
            shape.index,
            shape.shape_index_mask,
            shape
                .name
                .clone()
                .unwrap_or_else(|| format!("Shape {}", shape.index)),
        ));
    }
    options.sort_by_key(|(index, _, _)| *index);
    options
}

fn model_has_shape_mask(model: &WeaponModelData, shape_mask: u32) -> bool {
    model
        .meshes
        .iter()
        .flat_map(|mesh| mesh.shape_influences.iter())
        .any(|shape| shape.shape_index_mask == shape_mask)
}

#[cfg(test)]
mod weapon_url_tests {
    use super::*;

    #[test]
    fn parses_weapon_url_state_from_hash() {
        let state = weapon_url_state_from_hash(
            "#/weapon-models?q=%E6%B5%AA%E6%BC%AB&f=two&item=45058&stain0=17&stain1=93",
        );
        assert_eq!(state.query, "浪漫");
        assert_eq!(state.filter, WeaponSlotFilter::TwoHanded);
        assert_eq!(state.item_id, Some(45058));
        assert_eq!(state.stain_ids, [17, 93]);
    }

    #[test]
    fn stain_id_parser_rejects_reserved_and_invalid_values() {
        assert_eq!(parse_stain_id("254"), 254);
        assert_eq!(parse_stain_id("255"), 0);
        assert_eq!(parse_stain_id("invalid"), 0);
    }

    #[test]
    fn weapon_slot_filter_keys_round_trip() {
        for filter in [
            WeaponSlotFilter::All,
            WeaponSlotFilter::Main,
            WeaponSlotFilter::Off,
            WeaponSlotFilter::TwoHanded,
            WeaponSlotFilter::Dual,
        ] {
            assert_eq!(WeaponSlotFilter::from_key(filter.key()), Some(filter));
        }
    }

    #[test]
    fn glass_blend_mode_values_round_trip() {
        for mode in [ModelGlassBlendMode::Multiply, ModelGlassBlendMode::Additive] {
            assert_eq!(parse_glass_blend_mode(glass_blend_mode_value(mode)), mode);
        }
        assert_eq!(
            parse_glass_blend_mode("unknown"),
            ModelGlassBlendMode::Multiply
        );
    }

    #[test]
    fn secondary_vertex_debug_modes_round_trip() {
        for mode in [
            ModelDebugMode::VertexColor1,
            ModelDebugMode::SecondaryNormal,
            ModelDebugMode::Flow0,
            ModelDebugMode::Flow1,
        ] {
            assert_eq!(parse_debug_mode(debug_mode_value(mode)), mode);
        }
    }

    #[test]
    fn shape_options_are_unique_sorted_and_rebuild_the_canvas_key() {
        let model = test_shape_model();

        assert_eq!(
            model_shape_options(&model),
            vec![(0, 1, "shape_a".to_string()), (2, 4, "shape_c".to_string()),]
        );
        assert!(model_has_shape_mask(&model, 1));
        assert!(model_has_shape_mask(&model, 4));
        assert!(!model_has_shape_mask(&model, 2));
        assert_ne!(
            model_canvas_key(&model, None),
            model_canvas_key(&model, Some(1))
        );
    }

    fn test_shape_model() -> WeaponModelData {
        WeaponModelData {
            item_id: 42,
            item_name: "shape test".to_string(),
            model_main: PackedModelId::from_raw(1),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: xiv_companion::ModelBounds::default(),
            materials: Vec::new(),
            textures: Vec::new(),
            meshes: vec![xiv_companion::ModelMesh {
                path: "shape-test.mdl".to_string(),
                part_index: 0,
                mesh_category: Some("normal".to_string()),
                submesh: None,
                shape_influences: vec![
                    test_shape_info(2, "shape_c"),
                    test_shape_info(0, "shape_a"),
                    test_shape_info(0, "duplicate"),
                ],
                shape_targets: Vec::new(),
                material_index: 0,
                material_slot: 0,
                material_name: "shape test".to_string(),
                color: [1.0; 3],
                bone_table: None,
                vertices: Vec::new(),
                indices: Vec::new(),
            }],
        }
    }

    fn test_shape_info(index: usize, name: &str) -> xiv_companion::ModelShapeInfo {
        let shape_index_mask = 1_u32 << index;
        xiv_companion::ModelShapeInfo {
            index,
            name: Some(name.to_string()),
            shape_index_mask,
            shape_index_mask_hex: format!("0x{shape_index_mask:08X}"),
            shape_mesh_index: index,
            shape_value_count: 1,
        }
    }
}
