use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc as WasmRc};

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

use crate::app::icons::{Icon, IconKind};
#[cfg(target_arch = "wasm32")]
use crate::app::model_canvas_renderer::WebWeaponCanvasRenderer;
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::{cx, format_integer};
use xiv_companion::renderer::WeaponRenderOptions;

use xiv_companion::{
    PackedModelId, WeaponCatalogItem, WeaponCatalogPackage, WeaponModelData,
    WeaponModelTextureKind, weapon_slot_label,
};

use super::crafting::ItemIcon;
use crate::app::data::{load_weapon_catalog, load_weapon_model};

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
}

#[component]
pub fn WeaponModelsPage() -> Element {
    let initial_url_state = initial_weapon_url_state();
    let initial_query = initial_url_state.query;
    let initial_filter = initial_url_state.filter;
    let initial_item_id = initial_url_state.item_id;

    let catalog = use_resource(load_weapon_catalog);
    let mut query = use_signal(move || initial_query.clone());
    let mut slot_filter = use_signal(move || initial_filter);
    let mut selected_id = use_signal(move || initial_item_id);
    let mut selected_item = use_signal(|| None::<WeaponCatalogItem>);
    let model = use_resource(move || {
        let item = selected_item();
        async move {
            match item {
                Some(item) => load_weapon_model(item).await,
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
        sync_weapon_url_state(&query(), slot_filter(), selected_id());
    });

    let catalog_snapshot = catalog.read().as_ref().cloned();
    let selected_snapshot = selected_item();
    let selected_id_snapshot = selected_id();
    let query_snapshot = query();
    let slot_filter_snapshot = slot_filter();

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
                                catalog,
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
) -> Element {
    let render_options = use_signal(WeaponRenderOptions::default);

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
                }

                div { class: "flex min-h-0 flex-1 flex-col overflow-hidden xl:flex-row",
                    div { class: "relative min-h-0 min-w-0 flex-1 overflow-hidden bg-[#0e1117]",
                        match model.read().as_ref() {
                            Some(Ok(data)) if data.item_id == item.id => {
                                let key = model_canvas_key(data);
                                rsx! {
                                    div { key: "{key}", class: "absolute inset-0",
                                        WeaponModelCanvas { model: data.clone(), render_options }
                                        WeaponRenderControls { options: render_options }
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
                            Some(Ok(data)) if data.item_id == item.id => rsx! {
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
                    StatRow { label: "Normal", value: texture_counts.normal.to_string() }
                    StatRow { label: "Mask", value: texture_counts.mask.to_string() }
                    StatRow { label: "Specular", value: texture_counts.specular.to_string() }
                    StatRow { label: "Material Props", value: texture_counts.material_properties.to_string() }
                    StatRow { label: "Tile Props", value: texture_counts.tile_properties.to_string() }
                    StatRow { label: "Sheen Props", value: texture_counts.sheen_properties.to_string() }
                    StatRow { label: "Sphere Props", value: texture_counts.sphere_properties.to_string() }
                    StatRow { label: "Tile Matrix", value: texture_counts.tile_matrix.to_string() }
                    StatRow { label: "Emissive", value: texture_counts.emissive.to_string() }
                    StatRow { label: "Index", value: texture_counts.index.to_string() }
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
    normal: usize,
    mask: usize,
    specular: usize,
    material_properties: usize,
    tile_properties: usize,
    sheen_properties: usize,
    sphere_properties: usize,
    tile_matrix: usize,
    emissive: usize,
    index: usize,
    other: usize,
}

impl TextureKindCounts {
    fn from_model(model: &WeaponModelData) -> Self {
        let mut counts = Self::default();
        for texture in &model.textures {
            match texture.kind {
                WeaponModelTextureKind::BaseColor => counts.base += 1,
                WeaponModelTextureKind::Normal => counts.normal += 1,
                WeaponModelTextureKind::Mask => counts.mask += 1,
                WeaponModelTextureKind::Specular => counts.specular += 1,
                WeaponModelTextureKind::MaterialProperties => counts.material_properties += 1,
                WeaponModelTextureKind::TileProperties => counts.tile_properties += 1,
                WeaponModelTextureKind::SheenProperties => counts.sheen_properties += 1,
                WeaponModelTextureKind::SphereProperties => counts.sphere_properties += 1,
                WeaponModelTextureKind::TileMatrixProperties => counts.tile_matrix += 1,
                WeaponModelTextureKind::Emissive => counts.emissive += 1,
                WeaponModelTextureKind::Index => counts.index += 1,
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
fn WeaponRenderControls(options: Signal<WeaponRenderOptions>) -> Element {
    let current = options();
    let bloom_percent = (current.bloom_strength * 100.0).round() as i32;

    rsx! {
        div {
            class: "absolute right-2 top-4 z-10 rounded-md border border-border bg-background/90 p-3 text-xs shadow-md backdrop-blur",
            style: "width: 14rem;",
            div { class: "mb-2 flex items-center justify-between gap-3",
                span { class: "font-medium", "渲染" }
                span { class: "text-[11px] text-muted-foreground", "{bloom_percent}%" }
            }
            div { class: "space-y-2",
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
fn WeaponModelCanvas(
    model: Rc<WeaponModelData>,
    render_options: Signal<WeaponRenderOptions>,
) -> Element {
    let canvas_id = format!(
        "weapon-model-canvas-{}-{}-{}",
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
    );
    let init_error = use_signal(|| None::<String>);

    #[cfg(target_arch = "wasm32")]
    {
        let effect_canvas_id = canvas_id.clone();
        let effect_model = model.clone();
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
                    WebWeaponCanvasRenderer::from_canvas(canvas, &model).await
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

    *callback_slot.borrow_mut() = Some(Closure::wrap(Box::new(move |_time_ms: f64| {
        let connected = {
            let mut renderer = renderer_for_loop.borrow_mut();
            if renderer.canvas_connected() {
                renderer.render_with_options(render_options());
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
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn weapon_url_state_from_hash(hash: &str) -> WeaponUrlState {
    let mut state = WeaponUrlState {
        query: String::new(),
        filter: WeaponSlotFilter::All,
        item_id: None,
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

#[allow(unused_variables)]
fn sync_weapon_url_state(query: &str, filter: WeaponSlotFilter, item_id: Option<u32>) {
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

fn model_canvas_key(model: &WeaponModelData) -> String {
    format!(
        "{}-{}-{}",
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
    )
}

#[cfg(test)]
mod weapon_url_tests {
    use super::*;

    #[test]
    fn parses_weapon_url_state_from_hash() {
        let state =
            weapon_url_state_from_hash("#/weapon-models?q=%E6%B5%AA%E6%BC%AB&f=two&item=45058");
        assert_eq!(state.query, "浪漫");
        assert_eq!(state.filter, WeaponSlotFilter::TwoHanded);
        assert_eq!(state.item_id, Some(45058));
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
}
