use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;
use xiv_companion::renderer::WeaponRenderOptions;
use xiv_companion::{
    CharacterAssemblyLoadRequest, CharacterCustomize, CharacterMakePackage,
    CollectionCatalogPackage, CollectionItem, DressedConcealmentPlan, DressedEquipmentPiece,
    DressedPieceModel, ModelAnimationSet, ModelSkeleton, PreparedModelOptions, WeaponModelData,
    WeaponStain, appearance_colors_from_palette, apply_weapon_model_stains,
    character_enabled_attribute_names, plan_dressed_concealment,
};

use crate::app::character_customize::{
    RaceSelect, character_display_name, customize_from_hex, customize_to_hex, fallback_customize,
    find_make_group,
};
use crate::app::data::{
    load_character_make, load_character_palette, load_collection_catalog, load_weapon_catalog,
    load_weapon_staining_templates,
};
use crate::app::glamour_state::{
    GlamourPiece, GlamourSet, GlamourSlot, GlamourState, export_glamour_json, import_glamour_json,
    load_glamour_state, new_glamour_id, now_timestamp, save_glamour_state,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::load_progress::{self, WeaponModelLoadProgress};
use crate::app::resources::{
    load_character_assembly_with_skeleton_from_local, load_dressed_piece_from_local,
};
use crate::app::ui::{
    Badge, Button, ButtonSize, ButtonVariant, Card, DialogKeyAction, EmptyState, dialog_key_action,
    input_class,
};
use crate::app::utils::cx;

use super::crafting::ItemIcon;
use super::weapon_models::{
    AnimationControls, AnimationPlaybackState, AnimationSetHandle, CharacterSceneCanvas,
    SceneCanvasModel, WeaponModelLoadingView, animation_playback, model_has_attribute_submeshes,
};

const PICKER_ROW_LIMIT: usize = 100;

#[derive(Clone, PartialEq)]
enum GlamourView {
    List,
    Editor(String),
}

#[derive(Clone, PartialEq)]
enum GlamourDialog {
    DeleteSet(String),
    ImportError(String),
}

fn piece_from_item(item: &CollectionItem, stains: [u8; 2]) -> GlamourPiece {
    GlamourPiece {
        item_id: item.id,
        name: item.name.clone(),
        icon: item.icon,
        equip_slot_category: u8::try_from(item.equip_slot_category).unwrap_or(u8::MAX),
        model_main: item.model_main,
        model_sub: item.model_sub,
        stains,
    }
}

fn mutate_glamour_set(
    mut glamour_state: Signal<GlamourState>,
    set_id: &str,
    mutate: impl FnOnce(&mut GlamourSet),
) {
    let mut state = glamour_state.write();
    if let Some(set) = state.sets.iter_mut().find(|set| set.id == set_id) {
        mutate(set);
        set.updated_at = now_timestamp();
    }
}

fn updated_label(updated_at: u64) -> String {
    if updated_at == 0 {
        return "尚未记录更新时间".to_string();
    }
    let elapsed_seconds = ((js_sys::Date::now() - updated_at as f64) / 1_000.0)
        .max(0.0)
        .floor() as u64;
    if elapsed_seconds < 60 {
        "刚刚更新".to_string()
    } else if elapsed_seconds < 3_600 {
        format!("{} 分钟前更新", elapsed_seconds / 60)
    } else if elapsed_seconds < 86_400 {
        format!("{} 小时前更新", elapsed_seconds / 3_600)
    } else {
        format!("{} 天前更新", elapsed_seconds / 86_400)
    }
}

fn find_stain(stains: &[WeaponStain], stain_id: u8) -> Option<&WeaponStain> {
    stains.iter().find(|stain| stain.id == stain_id)
}

fn stain_display_name(stains: &[WeaponStain], stain_id: u8) -> String {
    if stain_id == 0 {
        return "无染色".to_string();
    }
    find_stain(stains, stain_id)
        .map(|stain| stain.name.clone())
        .unwrap_or_else(|| "未知染剂".to_string())
}

fn stain_color_style(stain: &WeaponStain) -> String {
    format!(
        "background-color: rgb({}, {}, {});",
        stain.ui_color[0], stain.ui_color[1], stain.ui_color[2]
    )
}

fn download_glamour_json(json: &str) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::{JsCast, JsValue};

        let parts = js_sys::Array::new();
        parts.push(&JsValue::from_str(json));
        let blob = web_sys::Blob::new_with_str_sequence(&parts)
            .map_err(|error| format!("创建导出文件失败: {error:?}"))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|error| format!("创建导出链接失败: {error:?}"))?;
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| "当前页面无法创建下载链接".to_string())?;
        let anchor = document
            .create_element("a")
            .map_err(|error| format!("创建导出链接失败: {error:?}"))?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|_| "创建导出链接失败".to_string())?;
        anchor.set_href(&url);
        anchor.set_download("xiv-companion-glamour.json");
        anchor.click();
        web_sys::Url::revoke_object_url(&url)
            .map_err(|error| format!("释放导出链接失败: {error:?}"))?;
        return Ok(());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = json;
        Ok(())
    }
}

/// 预览身体资产：裸装装配（外观色已落地）+ 骨架 + 动画集。装备件独立缓存
/// （见 [`DRESSED_PIECE_CACHE`]），遮蔽标签按当前件组合经
/// `plan_dressed_concealment` 现算。
#[derive(Clone)]
struct GlamourPreviewAssets {
    /// 资产对应的捏脸 hex（换捏脸重载期间据此判过期）。
    customize_hex: String,
    body: Rc<WeaponModelData>,
    skeleton: Option<Rc<ModelSkeleton>>,
    animations: Option<Rc<ModelAnimationSet>>,
}

/// 免染基准件的缓存键：基准件对 (model_main, model_sub, 槽位, race) 恒定，
/// 与染色无关。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PieceCacheKey {
    model_main: u64,
    model_sub: u64,
    equip_slot_category: u32,
    race_code: u16,
}

thread_local! {
    /// 免染基准件缓存：换装/回切不重读游戏文件（命中为同步 Rc 克隆）。
    /// 插入序 FIFO，超上限逐出最旧。
    static DRESSED_PIECE_CACHE: RefCell<(HashMap<PieceCacheKey, Rc<DressedPieceModel>>, Vec<PieceCacheKey>)> =
        RefCell::new((HashMap::new(), Vec::new()));
}

/// 件缓存容量上限（约 4-5 个全身套装的件数）。
const DRESSED_PIECE_CACHE_CAP: usize = 48;

fn cached_dressed_piece(key: PieceCacheKey) -> Option<Rc<DressedPieceModel>> {
    DRESSED_PIECE_CACHE.with(|cache| cache.borrow().0.get(&key).cloned())
}

fn cache_dressed_piece(key: PieceCacheKey, piece: Rc<DressedPieceModel>) {
    DRESSED_PIECE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.0.contains_key(&key) {
            cache.1.push(key);
        }
        cache.0.insert(key, piece);
        while cache.1.len() > DRESSED_PIECE_CACHE_CAP {
            let oldest = cache.1.remove(0);
            cache.0.remove(&oldest);
        }
    });
}

/// 稳定内容修订号：用哈希当修订（内容变 → 修订变，驱动画布按件重建）。
fn content_revision<T: std::hash::Hash>(value: &T) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// 预览的装备件身份：染色变化不改变它（不重载模型，走增量染色路径）。
#[derive(Clone, PartialEq)]
struct PreviewPieceIdentity {
    item_id: u32,
    item_name: String,
    model_main: u64,
    model_sub: u64,
    equip_slot_category: u8,
}

#[component]
pub fn GlamourPage() -> Element {
    let glamour_state = use_signal(load_glamour_state);
    use_effect(move || save_glamour_state(&glamour_state()));

    let catalog = use_resource(load_collection_catalog);
    let stain_catalog = use_resource(load_weapon_catalog);
    let stains = use_memo(move || {
        stain_catalog
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|package| Rc::new(package.stains.clone()))
            .unwrap_or_else(|| Rc::new(Vec::new()))
    });

    let mut view = use_signal(|| GlamourView::List);
    let mut dialog = use_signal(|| None::<GlamourDialog>);
    let mut list_query = use_signal(String::new);
    let mut transfer_message = use_signal(|| None::<Result<String, String>>);
    let mut import_input_version = use_signal(|| 0_u32);

    // 预览捏脸（页面级，持久化为 hex）：持久化值经完整校验优先采用，否则用
    // 兜底基底；捏脸菜单资产到达后采用当前（种族, 部族, 性别）组默认捏脸
    // （已有持久化捏脸时跳过采用；资产加载失败同样落定，用兜底基底继续）。
    let (initial_customize, initial_adopted) = match glamour_state
        .peek()
        .preview_customize
        .as_deref()
        .and_then(customize_from_hex)
    {
        Some(customize) => (customize, true),
        None => (fallback_customize(), false),
    };
    let mut preview_customize = use_signal(move || initial_customize);
    let mut preview_default_adopted = use_signal(move || initial_adopted);
    let character_make = use_resource(load_character_make);
    use_effect(move || {
        if preview_default_adopted() {
            return;
        }
        let package_snapshot = character_make.read();
        let Some(result) = package_snapshot.as_ref() else {
            return;
        };
        if let Ok(package) = result {
            let current = preview_customize();
            if let Some(group) =
                find_make_group(package, current.race, current.tribe, current.gender)
            {
                preview_customize.set(group.default_customize);
            }
        }
        preview_default_adopted.set(true);
    });

    let mut create_set = move || {
        let set = GlamourSet {
            id: new_glamour_id(),
            name: "未命名套装".to_string(),
            customize: None,
            pieces: BTreeMap::new(),
            updated_at: now_timestamp(),
        };
        let set_id = set.id.clone();
        let mut state = glamour_state;
        state.write().upsert_set(set);
        view.set(GlamourView::Editor(set_id));
    };

    let state_snapshot = glamour_state.read().clone();
    let view_snapshot = view.read().clone();
    let dialog_snapshot = dialog.read().clone();
    let list_query_snapshot = list_query();
    let transfer_message_snapshot = transfer_message.read().clone();
    let catalog_snapshot = catalog.read().as_ref().cloned();
    let catalog_data = catalog_snapshot
        .as_ref()
        .and_then(|result| result.as_ref().ok().cloned());
    let catalog_error = catalog_snapshot
        .as_ref()
        .and_then(|result| result.as_ref().err().cloned());
    let catalog_loading = catalog_snapshot.is_none();
    let set_total = state_snapshot.sets.len();
    let stains_snapshot = stains.read().clone();
    let character_make_snapshot = character_make
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-2 sm:px-5 lg:px-6",
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div { class: "min-w-0 space-y-0.5",
                        div { class: "text-xs text-muted-foreground", "工具" }
                        div { class: "flex flex-wrap items-center gap-x-2 gap-y-1",
                            h1 { class: "text-xl font-semibold leading-tight", "幻化套装" }
                            crate::app::modules::ModuleCapabilityBadges { module_id: "glamour" }
                        }
                    }
                    div { class: "flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2",
                        match &view_snapshot {
                            GlamourView::List => rsx! {
                                div { class: "relative w-full sm:w-56",
                                    Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                                    input {
                                        r#type: "text",
                                        role: "searchbox",
                                        placeholder: "搜索套装名称",
                                        value: "{list_query_snapshot}",
                                        class: input_class("h-8 pl-9"),
                                        oninput: move |event| list_query.set(event.value()),
                                    }
                                }
                                Button {
                                    variant: ButtonVariant::Primary,
                                    size: ButtonSize::Sm,
                                    onclick: move |_| create_set(),
                                    Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                                    "新建套装"
                                }
                                label {
                                    class: "inline-flex h-8 shrink-0 cursor-pointer items-center justify-center gap-2 rounded-md border border-input bg-background px-3 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
                                    title: "导入幻化套装 JSON",
                                    Icon { kind: IconKind::Download, class: "h-4 w-4" }
                                    "导入"
                                    input {
                                        key: "{import_input_version}",
                                        r#type: "file",
                                        accept: "application/json,.json",
                                        class: "hidden",
                                        onchange: move |event| {
                                            let Some(file) = event.files().into_iter().next() else { return; };
                                            spawn(async move {
                                                let outcome = async {
                                                    let json = file
                                                        .read_string()
                                                        .await
                                                        .map_err(|error| format!("读取导入文件失败: {error}"))?;
                                                    import_glamour_json(&json)
                                                }
                                                .await;
                                                match outcome {
                                                    Ok(imported) => {
                                                        let count = imported.len();
                                                        let mut state = glamour_state;
                                                        let mut state = state.write();
                                                        let mut known_ids = state
                                                            .sets
                                                            .iter()
                                                            .map(|set| set.id.clone())
                                                            .collect::<HashSet<_>>();
                                                        for mut set in imported {
                                                            if !known_ids.insert(set.id.clone()) {
                                                                set.id = new_glamour_id();
                                                                known_ids.insert(set.id.clone());
                                                            }
                                                            state.upsert_set(set);
                                                        }
                                                        drop(state);
                                                        transfer_message
                                                            .set(Some(Ok(format!("已导入 {count} 个套装"))));
                                                    }
                                                    Err(error) => {
                                                        dialog.set(Some(GlamourDialog::ImportError(error)));
                                                    }
                                                }
                                                import_input_version += 1;
                                            });
                                        },
                                    }
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    size: ButtonSize::Sm,
                                    disabled: set_total == 0,
                                    onclick: move |_| {
                                        let snapshot = glamour_state.read();
                                        let result = export_glamour_json(&snapshot)
                                            .and_then(|json| download_glamour_json(&json))
                                            .map(|_| format!("已导出 {} 个套装", snapshot.sets.len()));
                                        drop(snapshot);
                                        transfer_message.set(Some(result));
                                    },
                                    Icon { kind: IconKind::Upload, class: "h-4 w-4" }
                                    "导出"
                                }
                            },
                            GlamourView::Editor(_) => rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    size: ButtonSize::Sm,
                                    onclick: move |_| view.set(GlamourView::List),
                                    "← 返回"
                                }
                            },
                        }
                    }
                }
                if let Some(Ok(message)) = &transfer_message_snapshot {
                    div { class: "mt-2 text-xs text-emerald-700", "{message}" }
                }
                if let Some(Err(error)) = &transfer_message_snapshot {
                    div { class: "mt-2 text-xs text-destructive", "{error}" }
                }
            }

            match &view_snapshot {
                GlamourView::List => rsx! {
                    GlamourSetList {
                        glamour_state,
                        query: list_query_snapshot.clone(),
                        on_create: move |_| create_set(),
                        on_edit: move |set_id: String| view.set(GlamourView::Editor(set_id)),
                        on_request_delete: move |set_id: String| {
                            dialog.set(Some(GlamourDialog::DeleteSet(set_id)));
                        },
                    }
                },
                GlamourView::Editor(set_id) => rsx! {
                    GlamourSetEditor {
                        set_id: set_id.clone(),
                        glamour_state,
                        catalog: catalog_data.clone(),
                        catalog_error: catalog_error.clone(),
                        catalog_loading,
                        stains: stains_snapshot.clone(),
                        preview_customize,
                        preview_default_adopted,
                        character_make: character_make_snapshot.clone(),
                    }
                },
            }

            match &dialog_snapshot {
                Some(GlamourDialog::DeleteSet(set_id)) => {
                    let name = state_snapshot
                        .find_set(set_id)
                        .map(|set| set.name.clone())
                        .unwrap_or_else(|| "未命名套装".to_string());
                    let set_id = set_id.clone();
                    rsx! {
                        ConfirmDialog {
                            title: "删除套装".to_string(),
                            description: format!("确定删除套装「{name}」吗？此操作无法撤销。"),
                            confirm_label: "删除",
                            on_confirm: move |_| {
                                let mut glamour_state = glamour_state;
                                glamour_state.write().remove_set(&set_id);
                            },
                            on_close: move |_| dialog.set(None),
                        }
                    }
                },
                Some(GlamourDialog::ImportError(error)) => rsx! {
                    MessageDialog {
                        title: "导入失败".to_string(),
                        message: error.clone(),
                        on_close: move |_| dialog.set(None),
                    }
                },
                None => rsx! {},
            }
        }
    }
}

#[component]
fn GlamourSetList(
    glamour_state: Signal<GlamourState>,
    query: String,
    on_create: EventHandler<()>,
    on_edit: EventHandler<String>,
    on_request_delete: EventHandler<String>,
) -> Element {
    let needle = query.trim().to_lowercase();
    let total = glamour_state.read().sets.len();
    let mut sets = glamour_state.read().sets.clone();
    sets.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    sets.retain(|set| needle.is_empty() || set.name.to_lowercase().contains(&needle));

    let duplicate_set = move |set_id: String| {
        let mut glamour_state = glamour_state;
        let Some(original) = glamour_state.read().find_set(&set_id).cloned() else {
            return;
        };
        glamour_state
            .write()
            .upsert_set(original.duplicate(new_glamour_id(), now_timestamp()));
    };

    rsx! {
        div { class: "min-h-0 flex-1 overflow-y-auto p-4 sm:px-5 lg:px-6",
            if total == 0 {
                EmptyState {
                    icon: rsx! { Icon { kind: IconKind::Shirt, class: "h-6 w-6" } },
                    title: "还没有幻化套装".to_string(),
                    description: Some("新建套装后，可以为每个部位挑选装备并记录双通道染色。".to_string()),
                    action: rsx! {
                        Button {
                            variant: ButtonVariant::Primary,
                            size: ButtonSize::Sm,
                            onclick: move |_| on_create.call(()),
                            Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                            "新建套装"
                        }
                    },
                }
            } else if sets.is_empty() {
                div { class: "py-10 text-center text-sm text-muted-foreground",
                    "没有名称包含「{query}」的套装"
                }
            } else {
                div { class: "grid gap-3 sm:grid-cols-2 xl:grid-cols-3",
                    for set in sets {
                        GlamourSetCard {
                            key: "{set.id}",
                            set,
                            on_edit,
                            on_duplicate: move |set_id: String| duplicate_set(set_id),
                            on_request_delete,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GlamourSetCard(
    set: GlamourSet,
    on_edit: EventHandler<String>,
    on_duplicate: EventHandler<String>,
    on_request_delete: EventHandler<String>,
) -> Element {
    let edit_id = set.id.clone();
    let duplicate_id = set.id.clone();
    let delete_id = set.id.clone();

    rsx! {
        Card {
            div { class: "space-y-3 p-4",
                div { class: "flex items-center justify-between gap-2",
                    div {
                        class: "min-w-0 truncate text-sm font-semibold",
                        title: "{set.name}",
                        "{set.name}"
                    }
                    div { class: "flex shrink-0 items-center gap-1.5",
                        if set.customize.is_some() {
                            Badge { title: "本套装使用独立角色形象", "独立形象" }
                        }
                        Badge { "{set.piece_count()}/12" }
                    }
                }
                if set.pieces.is_empty() {
                    div { class: "text-xs text-muted-foreground", "尚未选择装备" }
                } else {
                    div { class: "flex flex-wrap items-center gap-1.5",
                        for slot in GlamourSlot::ALL {
                            if let Some(piece) = set.pieces.get(&slot) {
                                div {
                                    key: "{slot.label()}",
                                    title: "{piece.name}",
                                    ItemIcon { icon: piece.icon, size: "sm" }
                                }
                            }
                        }
                    }
                }
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div { class: "text-xs text-muted-foreground", "{updated_label(set.updated_at)}" }
                    div { class: "flex items-center gap-1",
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            onclick: move |_| on_edit.call(edit_id.clone()),
                            Icon { kind: IconKind::Pencil, class: "h-3.5 w-3.5" }
                            "编辑"
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            onclick: move |_| on_duplicate.call(duplicate_id.clone()),
                            Icon { kind: IconKind::Copy, class: "h-3.5 w-3.5" }
                            "复制"
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            class: "text-destructive",
                            onclick: move |_| on_request_delete.call(delete_id.clone()),
                            Icon { kind: IconKind::Trash2, class: "h-3.5 w-3.5" }
                            "删除"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GlamourSetEditor(
    set_id: String,
    glamour_state: Signal<GlamourState>,
    catalog: Option<Rc<CollectionCatalogPackage>>,
    catalog_error: Option<String>,
    catalog_loading: bool,
    stains: Rc<Vec<WeaponStain>>,
    preview_customize: Signal<CharacterCustomize>,
    preview_default_adopted: Signal<bool>,
    character_make: Option<Rc<CharacterMakePackage>>,
) -> Element {
    let mut picker_slot = use_signal(|| None::<GlamourSlot>);
    let mut dye_slot = use_signal(|| None::<GlamourSlot>);
    let mut model_progress = use_signal(|| None::<WeaponModelLoadProgress>);
    // 动画选择：None=rest；action.pap 动画表在 race code 间一致，换装保留选择。
    let mut animation_selection = use_signal(|| None::<usize>);
    let render_options = use_signal(|| WeaponRenderOptions {
        msaa_samples: 4,
        ..Default::default()
    });
    let palette_package = use_resource(load_character_palette);
    let staining_templates = use_resource(load_weapon_staining_templates);

    let set_id_for_identity = set_id.clone();
    // 预览重载键：全部件（含主手/副手武器）按槽位序的免染色身份；增删/换装
    // 改变它，染色改变不改变它（染色走下方增量染色路径）。
    let preview_identity = use_memo(move || {
        let state = glamour_state.read();
        let Some(set) = state.find_set(&set_id_for_identity) else {
            return Vec::new();
        };
        set.pieces
            .values()
            .map(|piece| PreviewPieceIdentity {
                item_id: piece.item_id,
                item_name: piece.name.clone(),
                model_main: piece.model_main,
                model_sub: piece.model_sub,
                equip_slot_category: piece.equip_slot_category,
            })
            .collect::<Vec<_>>()
    });
    // 生效捏脸：套装有独立形象（hex 通过完整校验）时优先，否则跟随页面级预览
    // 捏脸。预览加载键、遮蔽计划与形象控件全部以生效值为准。
    let set_id_for_customize = set_id.clone();
    let effective_customize = use_memo(move || {
        let state = glamour_state.read();
        let set_override = state
            .find_set(&set_id_for_customize)
            .and_then(|set| set.customize.as_deref())
            .and_then(customize_from_hex);
        match set_override {
            Some(customize) => (customize, true),
            None => (preview_customize(), false),
        }
    });
    // 身体加载（捏脸/调色板变化时重载；换装/染色不触发）：裸装装配 + 骨架 +
    // 动画集。装备件走下方独立资源，不再整身合并重读。
    let body_load = use_resource(move || {
        let adopted = preview_default_adopted();
        let (customize, from_set) = effective_customize();
        let palette = palette_package.read().as_ref().cloned();
        async move {
            if (!from_set && !adopted) || customize.validate().is_err() {
                return None;
            }
            let palette = match palette {
                None => return None,
                Some(Err(error)) => return Some(Err(error)),
                Some(Ok(package)) => package,
            };
            let appearance = appearance_colors_from_palette(&customize, &palette.palette);
            let request =
                CharacterAssemblyLoadRequest::new(customize, character_display_name(&customize))
                    .with_appearance(appearance);
            let customize_hex = customize_to_hex(&request.customize);
            Some(
                load_character_assembly_with_skeleton_from_local(request)
                    .await
                    .map(|(body, skeleton, animations)| GlamourPreviewAssets {
                        customize_hex,
                        body: Rc::new(body),
                        skeleton: skeleton.map(Rc::new),
                        animations: animations.map(Rc::new),
                    }),
            )
        }
    });

    // 装备件加载（身份列表/身体资产变化时重跑）：逐件先查免染基准缓存，未命中
    // 才读该件的 MDL/IMC/EQP 链——换一件只加载那一件，其余件同步 Rc 直传。
    let preview_pieces = use_resource(move || {
        let identity = preview_identity();
        let (customize, _) = effective_customize();
        let body = body_load
            .read()
            .clone()
            .flatten()
            .and_then(|result| result.ok());
        let race_code = customize.race_code();
        async move {
            let Some(body) = body else {
                return Vec::new();
            };
            let skeleton = body.skeleton.as_ref().map(|skeleton| (**skeleton).clone());
            let mut loaded: Vec<(u32, Rc<DressedPieceModel>)> = Vec::new();
            for piece in &identity {
                let category = u32::from(piece.equip_slot_category);
                let key = PieceCacheKey {
                    model_main: piece.model_main,
                    model_sub: piece.model_sub,
                    equip_slot_category: category,
                    race_code,
                };
                let model = match cached_dressed_piece(key) {
                    Some(model) => model,
                    None => {
                        let request = DressedEquipmentPiece {
                            item_id: piece.item_id,
                            item_name: piece.item_name.clone(),
                            model_main: piece.model_main,
                            model_sub: piece.model_sub,
                            equip_slot_category: category,
                            stain_ids: [0, 0],
                        };
                        match load_dressed_piece_from_local(request, race_code, skeleton.clone())
                            .await
                        {
                            Ok(Some(model)) => {
                                let model = Rc::new(model);
                                cache_dressed_piece(key, model.clone());
                                model
                            }
                            // 无模型/读取失败：单件跳过，不阻断其余件。
                            Ok(None) | Err(_) => continue,
                        }
                    }
                };
                loaded.push((category, model));
            }
            loaded
        }
    });

    // 场景快照（hold-last）：件资源 Pending 期间保持上一次完整列表（换件不闪
    // 空白，新件到达即增量上屏）；身体按捏脸 hex 判过期，过期/失败时整体清空。
    let mut scene_assets = use_signal(|| None::<GlamourPreviewAssets>);
    let mut scene_pieces = use_signal(|| Vec::<(u32, Rc<DressedPieceModel>)>::new());
    let mut preview_error = use_signal(|| None::<String>);
    {
        let body_signal = body_load;
        use_effect(move || {
            let customize_hex = customize_to_hex(&effective_customize().0);
            let body = body_signal.read().cloned().flatten();
            match body {
                // 完成：接受新身体（同捏脸的重跑 Pending 不动旧值）。
                Some(Ok(assets)) if assets.customize_hex == customize_hex => {
                    scene_assets.set(Some(assets));
                    preview_error.set(None);
                }
                Some(Err(error)) => {
                    scene_assets.set(None);
                    scene_pieces.set(Vec::new());
                    preview_error.set(Some(error));
                }
                // 未就绪：仅当旧快照已过期（捏脸已变）或此前在报错时清空。
                _ => {
                    let stale = scene_assets
                        .peek()
                        .as_ref()
                        .map(|assets| assets.customize_hex != customize_hex)
                        .unwrap_or(false);
                    if stale || preview_error.peek().is_some() {
                        scene_assets.set(None);
                        scene_pieces.set(Vec::new());
                        preview_error.set(None);
                    }
                }
            }
        });
    }
    use_effect(move || {
        if let Some(pieces) = preview_pieces.read().clone() {
            scene_pieces.set(pieces);
        }
    });

    use_effect(move || {
        load_progress::set_weapon_model_progress_sink(move |progress| {
            if let Ok(mut slot) = model_progress.try_write() {
                *slot = progress;
            }
        });
    });

    use_drop(move || {
        load_progress::clear_weapon_model_progress();
    });

    let set = glamour_state.read().find_set(&set_id).cloned();
    let Some(set) = set else {
        return rsx! {
            div { class: "min-h-0 flex-1 overflow-y-auto p-4 sm:px-5 lg:px-6",
                EmptyState {
                    icon: rsx! { Icon { kind: IconKind::Shirt, class: "h-6 w-6" } },
                    title: "套装不存在或已被删除".to_string(),
                    description: Some("返回列表选择其他套装。".to_string()),
                }
            }
        };
    };

    let set_id_for_name = set_id.clone();
    let set_id_for_blur = set_id.clone();
    let set_id_for_clear = set_id.clone();
    let set_id_for_pick = set_id.clone();
    let set_id_for_dye = set_id.clone();
    let picker_slot_snapshot = picker_slot();
    let dye_slot_snapshot = dye_slot();

    let (customize_snapshot, has_set_override) = effective_customize();
    let race_code = customize_snapshot.race_code();
    let has_pieces = !set.pieces.is_empty();

    let assets_snapshot = scene_assets();
    let error_snapshot = preview_error();
    let pieces_snapshot = scene_pieces();
    let playback: Option<AnimationPlaybackState> = assets_snapshot.as_ref().and_then(|assets| {
        animation_playback(&assets.skeleton, &assets.animations, animation_selection())
    });
    let piece_item_ids = set
        .pieces
        .values()
        .map(|piece| piece.item_id)
        .collect::<Vec<_>>();
    let current_progress = model_progress().filter(|progress| {
        progress.item_id == u32::from(race_code) || piece_item_ids.contains(&progress.item_id)
    });
    let preview_loading = has_pieces && assets_snapshot.is_none() && error_snapshot.is_none();

    // 场景组装（渲染期纯计算）：身体 + 各件 → 多实例画布条目。染色在此按件
    // 落地（漂移件克隆重染，未染件 Rc 直传零拷贝）；遮蔽隐藏标签按当前件组合
    // 现算（`plan_dressed_concealment`，不动缓存件数据本体）。
    let scene_entries = {
        let templates = staining_templates
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned();
        let mut entries: Vec<SceneCanvasModel> = Vec::new();
        let mut concealment: Option<DressedConcealmentPlan> = None;
        if let Some(assets) = &assets_snapshot {
            // DressedPieceModel 内部 Rc，按值克隆零拷贝。
            let piece_models: Vec<DressedPieceModel> = pieces_snapshot
                .iter()
                .map(|(_, piece)| (**piece).clone())
                .collect();
            let plan = plan_dressed_concealment(&customize_snapshot, &assets.body, &piece_models);
            // 身体启用名 = 捏脸默认集合（脸部特征件等）− 遮蔽名。
            let mut body_names =
                character_enabled_attribute_names(&customize_snapshot, &assets.body);
            for hidden in &plan.body_hidden_attributes {
                body_names.retain(|name| name != hidden);
            }
            let mut body_options = PreparedModelOptions::default()
                .with_component_preview_layout(false)
                .with_hidden_mesh_indices(plan.body_hidden_meshes.clone());
            if model_has_attribute_submeshes(&assets.body) {
                body_options = body_options.with_enabled_attribute_names(body_names.clone());
            }
            let body_ptr = Rc::as_ptr(&assets.body) as usize;
            entries.push(SceneCanvasModel {
                key: "body".to_string(),
                model: assets.body.clone(),
                prepared_options: body_options,
                revision: content_revision(&(body_ptr, &plan.body_hidden_meshes, &body_names)),
                materials_revision: content_revision(&(body_ptr,)),
                attach: None,
            });
            concealment = Some(plan);
        }
        for (category, piece) in &pieces_snapshot {
            // 整件隐藏的件（如头部 EQP 耳饰位门控的耳饰件）不建实例。
            if concealment
                .as_ref()
                .is_some_and(|plan| plan.hidden_pieces.contains(&(*category, piece.item_id)))
            {
                continue;
            }
            let stains = set
                .pieces
                .values()
                .find(|set_piece| {
                    u32::from(set_piece.equip_slot_category) == *category
                        && set_piece.item_id == piece.item_id
                })
                .map(|set_piece| set_piece.stains)
                .unwrap_or([0, 0]);
            let stained = if stains == [0, 0] {
                piece.model.clone()
            } else {
                match &templates {
                    Some(templates) => {
                        Rc::new(apply_weapon_model_stains(&piece.model, stains, templates))
                    }
                    // 染色模板未就绪：先显示免染基准，模板到达后本组件重跑补齐。
                    None => piece.model.clone(),
                }
            };
            let enabled_names = concealment.as_ref().and_then(|plan| {
                plan.piece_enabled_attributes
                    .iter()
                    .find(|((slot, item_id), _)| *slot == *category && *item_id == piece.item_id)
                    .map(|(_, names)| names.clone())
            });
            let mut piece_options =
                PreparedModelOptions::default().with_component_preview_layout(false);
            if model_has_attribute_submeshes(&stained) {
                if let Some(names) = &enabled_names {
                    piece_options = piece_options.with_enabled_attribute_names(names.clone());
                }
            }
            entries.push(SceneCanvasModel {
                key: format!("slot:{category}:{}", piece.item_id),
                model: stained.clone(),
                prepared_options: piece_options,
                revision: content_revision(&(Rc::as_ptr(&piece.model) as usize, &enabled_names)),
                materials_revision: content_revision(&(Rc::as_ptr(&stained) as usize, stains)),
                attach: piece.attach,
            });
        }
        entries
    };
    let orbit_reset_revision = content_revision(&customize_to_hex(&customize_snapshot));

    // 捏脸变更：套装有独立形象时写入套装（走 mutate/persist，刷新 updated_at）；
    // 否则持久化 hex 到 GlamourState 页面级 preview_customize（页面级保存
    // effect 落盘），并阻止待进行的默认捏脸采用覆盖用户选择。
    let set_id_for_apply = set_id.clone();
    let mut apply_customize = move |next: CharacterCustomize| {
        if effective_customize.peek().1 {
            mutate_glamour_set(glamour_state, &set_id_for_apply, move |set| {
                set.customize = Some(customize_to_hex(&next));
            });
        } else {
            preview_default_adopted.set(true);
            glamour_state.write().preview_customize = Some(customize_to_hex(&next));
            preview_customize.set(next);
        }
    };
    let mut apply_customize_for_hex = apply_customize.clone();
    // 独立形象开关：绑定 = 把当前生效捏脸固化进套装；清除 = 回退跟随页面级。
    let set_id_for_pin = set_id.clone();
    let pin_set_customize = move |_| {
        let hex = customize_to_hex(&effective_customize.peek().0);
        mutate_glamour_set(glamour_state, &set_id_for_pin, move |set| {
            set.customize = Some(hex);
        });
    };
    let set_id_for_unpin = set_id.clone();
    let clear_set_customize = move |_| {
        mutate_glamour_set(glamour_state, &set_id_for_unpin, |set| {
            set.customize = None;
        });
    };

    rsx! {
        div { class: "min-h-0 flex-1 overflow-y-auto",
            div { class: "flex flex-col gap-4 p-4 sm:px-5 lg:flex-row lg:items-start lg:gap-5 lg:px-6",
                div { class: "min-w-0 flex-1",
                    div { class: "mb-4 flex flex-wrap items-center gap-3",
                        input {
                            r#type: "text",
                            value: "{set.name}",
                            placeholder: "套装名称",
                            aria_label: "套装名称",
                            class: input_class("h-9 max-w-xs font-medium"),
                            oninput: move |event| {
                                let name = event.value();
                                mutate_glamour_set(glamour_state, &set_id_for_name, move |set| {
                                    set.name = name;
                                });
                            },
                            onblur: move |_| {
                                mutate_glamour_set(glamour_state, &set_id_for_blur, |set| {
                                    if set.name.trim().is_empty() {
                                        set.name = "未命名套装".to_string();
                                    }
                                });
                            },
                        }
                        Badge { "{set.piece_count()}/12 部位" }
                        div { class: "text-xs text-muted-foreground", "{updated_label(set.updated_at)}" }
                    }

                    if catalog_loading {
                        div { class: "flex items-center justify-center gap-2 py-16 text-sm text-muted-foreground",
                            Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                            "正在加载装备目录…"
                        }
                    } else if let Some(error) = &catalog_error {
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::Database, class: "h-6 w-6" } },
                            title: "装备目录未就绪".to_string(),
                            description: Some(error.clone()),
                        }
                    } else if catalog.is_some() {
                        div { class: "grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4",
                            for slot in GlamourSlot::ALL {
                                GlamourSlotCard {
                                    key: "{slot.label()}",
                                    slot,
                                    piece: set.pieces.get(&slot).cloned(),
                                    stains: stains.clone(),
                                    on_pick: move |slot| picker_slot.set(Some(slot)),
                                    on_dye: move |slot| dye_slot.set(Some(slot)),
                                    on_clear: {
                                        let set_id = set_id_for_clear.clone();
                                        move |slot| {
                                            mutate_glamour_set(glamour_state, &set_id, move |set| {
                                                set.pieces.remove(&slot);
                                            });
                                        }
                                    },
                                }
                            }
                        }
                    }
                }

                aside { class: "flex w-full shrink-0 flex-col overflow-hidden rounded-lg border bg-card lg:sticky lg:top-4 lg:w-[22rem] xl:w-[24rem]",
                    div { class: "relative h-72 shrink-0 overflow-hidden bg-[#0e1117] sm:h-80",
                        CharacterSceneCanvas {
                            entries: scene_entries,
                            render_options,
                            animation: playback,
                            skeleton: assets_snapshot
                                .as_ref()
                                .and_then(|assets| assets.skeleton.clone()),
                            orbit_reset_revision,
                        }
                        if !has_pieces {
                            div { class: "absolute inset-0 flex items-center justify-center bg-[#0e1117] p-4",
                                EmptyState {
                                    icon: rsx! { Icon { kind: IconKind::PersonStanding, class: "h-6 w-6" } },
                                    title: "暂无可预览的装备".to_string(),
                                    description: Some("为套装选择装备后，在此预览着装效果。".to_string()),
                                }
                            }
                        } else if let Some(error) = &error_snapshot {
                            div { class: "absolute inset-0 flex items-center justify-center bg-[#0e1117] p-4",
                                EmptyState {
                                    icon: rsx! { Icon { kind: IconKind::PersonStanding, class: "h-6 w-6" } },
                                    title: "着装预览加载失败".to_string(),
                                    description: Some(error.clone()),
                                    action: rsx! {
                                        a { href: "#/settings",
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                size: ButtonSize::Sm,
                                                Icon { kind: IconKind::Database, class: "h-4 w-4" }
                                                "设置页授权游戏目录"
                                            }
                                        }
                                    },
                                }
                            }
                        } else if preview_loading {
                            div { class: "absolute inset-0 bg-[#0e1117]",
                                WeaponModelLoadingView { progress: current_progress }
                            }
                        }
                    }
                    div { class: "space-y-3 border-t p-3 lg:max-h-96 lg:overflow-y-auto",
                        if has_pieces {
                            div { class: "flex items-center justify-between gap-2",
                                div { class: "text-xs text-muted-foreground",
                                    if has_set_override {
                                        "本套装使用独立形象"
                                    } else {
                                        "当前使用页面预览形象"
                                    }
                                }
                                if has_set_override {
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        size: ButtonSize::Sm,
                                        onclick: clear_set_customize,
                                        "清除独立形象"
                                    }
                                } else {
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        size: ButtonSize::Sm,
                                        onclick: pin_set_customize,
                                        "为本套装单独设置"
                                    }
                                }
                            }
                            RaceSelect {
                                customize: customize_snapshot,
                                on_change: move |(race, tribe, gender)| {
                                    let Some(package) = character_make.as_ref() else {
                                        return;
                                    };
                                    let next = find_make_group(package, race, tribe, gender)
                                        .map(|group| group.default_customize)
                                        .unwrap_or(CharacterCustomize {
                                            race,
                                            tribe,
                                            gender,
                                            age: 1,
                                            height: 50,
                                            ..Default::default()
                                        });
                                    apply_customize(next);
                                },
                            }
                            GlamourCustomizeHexInput {
                                customize: customize_snapshot,
                                on_apply: move |next| apply_customize_for_hex(next),
                            }
                            if let Some(animations) = assets_snapshot
                                .as_ref()
                                .and_then(|assets| assets.animations.clone())
                            {
                                AnimationControls {
                                    animations: AnimationSetHandle(animations),
                                    selected: animation_selection(),
                                    on_select: move |selected| animation_selection.set(selected),
                                }
                            }
                        }
                    }
                }
            }

            if let Some(slot) = picker_slot_snapshot {
                if let Some(catalog) = &catalog {
                    GlamourItemPickerDialog {
                        slot,
                        catalog: catalog.clone(),
                        on_pick: move |item: CollectionItem| {
                            mutate_glamour_set(glamour_state, &set_id_for_pick, move |set| {
                                let stains = set
                                    .pieces
                                    .get(&slot)
                                    .filter(|piece| piece.item_id == item.id)
                                    .map(|piece| piece.stains)
                                    .unwrap_or([0, 0]);
                                set.pieces.insert(slot, piece_from_item(&item, stains));
                            });
                        },
                        on_close: move |_| picker_slot.set(None),
                    }
                }
            }
            if let Some(slot) = dye_slot_snapshot {
                if let Some(piece) = set.pieces.get(&slot).cloned() {
                    GlamourDyeDialog {
                        slot,
                        piece,
                        stains: stains.clone(),
                        on_stain: move |(channel, stain_id): (usize, u8)| {
                            mutate_glamour_set(glamour_state, &set_id_for_dye, move |set| {
                                if let Some(piece) = set.pieces.get(&slot).cloned() {
                                    set.pieces.insert(slot, piece.stained(channel, stain_id));
                                }
                            });
                        },
                        on_close: move |_| dye_slot.set(None),
                    }
                }
            }
        }
    }
}

/// 捏脸代码粘贴框：接受角色页 52 字符 hex（Enter 或「应用」提交），非法输入
/// 仅提示不改状态；底部展示当前预览捏脸的代码。
#[component]
fn GlamourCustomizeHexInput(
    customize: CharacterCustomize,
    on_apply: EventHandler<CharacterCustomize>,
) -> Element {
    let mut input = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let current_hex = customize_to_hex(&customize);
    let input_snapshot = input();
    let error_snapshot = error();

    let mut apply = move || match customize_from_hex(&input()) {
        Some(next) => {
            error.set(None);
            input.set(String::new());
            on_apply.call(next);
        }
        None => {
            error.set(Some(
                "无效的捏脸代码：需为角色页复制的 52 位十六进制字符串".to_string(),
            ));
        }
    };
    let mut apply_on_key = apply.clone();

    rsx! {
        section { class: "space-y-2 border-t pt-3",
            div { class: "text-sm font-semibold", "捏脸代码" }
            div { class: "flex items-center gap-1.5",
                input {
                    r#type: "text",
                    value: "{input_snapshot}",
                    placeholder: "粘贴角色页 52 位代码",
                    aria_label: "捏脸代码",
                    class: input_class("h-8 flex-1 font-mono text-xs"),
                    oninput: move |event| {
                        input.set(event.value());
                        error.set(None);
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            apply_on_key();
                        }
                    },
                }
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    disabled: input_snapshot.trim().is_empty(),
                    onclick: move |_| apply(),
                    "应用"
                }
            }
            if let Some(message) = &error_snapshot {
                div { class: "text-xs text-destructive", "{message}" }
            }
            div {
                class: "break-all font-mono text-[11px] text-muted-foreground",
                title: "当前预览捏脸代码",
                "{current_hex}"
            }
        }
    }
}

#[component]
fn GlamourSlotCard(
    slot: GlamourSlot,
    piece: Option<GlamourPiece>,
    stains: Rc<Vec<WeaponStain>>,
    on_pick: EventHandler<GlamourSlot>,
    on_dye: EventHandler<GlamourSlot>,
    on_clear: EventHandler<GlamourSlot>,
) -> Element {
    rsx! {
        Card {
            div { class: "space-y-2 p-3",
                div { class: "text-xs font-medium text-muted-foreground", "{slot.label()}" }
                if let Some(piece) = &piece {
                    div { class: "flex items-center gap-2",
                        ItemIcon { icon: piece.icon, size: "md" }
                        div { class: "min-w-0 flex-1",
                            div {
                                class: "truncate text-sm font-medium",
                                title: "{piece.name}",
                                "{piece.name}"
                            }
                            div { class: "flex items-center gap-1.5 text-xs text-muted-foreground",
                                span { "染色" }
                                StainDot { stains: stains.clone(), stain_id: piece.stains[0] }
                                StainDot { stains: stains.clone(), stain_id: piece.stains[1] }
                            }
                        }
                    }
                    div { class: "flex items-center gap-1",
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            onclick: move |_| on_pick.call(slot),
                            "更换"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            onclick: move |_| on_dye.call(slot),
                            "染色"
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Sm,
                            class: "text-destructive",
                            onclick: move |_| on_clear.call(slot),
                            "清除"
                        }
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "flex w-full items-center justify-center gap-2 rounded-md border border-dashed px-3 py-4 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
                        onclick: move |_| on_pick.call(slot),
                        Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                        "选择装备"
                    }
                }
            }
        }
    }
}

#[component]
fn StainDot(stains: Rc<Vec<WeaponStain>>, stain_id: u8) -> Element {
    let (style, title) = if stain_id == 0 {
        (String::new(), "无染色".to_string())
    } else {
        match find_stain(&stains[..], stain_id) {
            Some(stain) => (stain_color_style(stain), stain.name.clone()),
            None => (String::new(), "未知染剂".to_string()),
        }
    };
    rsx! {
        span {
            class: "h-3.5 w-3.5 shrink-0 rounded-full border border-border bg-muted shadow-sm",
            style: "{style}",
            title: "{title}",
        }
    }
}

#[component]
fn GlamourItemPickerDialog(
    slot: GlamourSlot,
    catalog: Rc<CollectionCatalogPackage>,
    on_pick: EventHandler<CollectionItem>,
    on_close: EventHandler<()>,
) -> Element {
    let mut query = use_signal(String::new);
    let slot_items = use_memo(use_reactive!(|(slot, catalog)| {
        let mut items = catalog
            .items
            .iter()
            .filter(|item| {
                item.is_equipment()
                    && u8::try_from(item.equip_slot_category)
                        .is_ok_and(|category| slot.accepts(category))
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .level_item
                .cmp(&left.level_item)
                .then_with(|| left.name.cmp(&right.name))
        });
        items
    }));

    let query_snapshot = query();
    let needle = query_snapshot.trim().to_lowercase();
    let mut total = 0_usize;
    let mut visible: Vec<CollectionItem> = Vec::new();
    {
        let items = slot_items.read();
        for item in items.iter() {
            if needle.is_empty()
                || item.name.to_lowercase().contains(&needle)
                || item
                    .class_job_category_name
                    .to_lowercase()
                    .contains(&needle)
            {
                total += 1;
                if visible.len() < PICKER_ROW_LIMIT {
                    visible.push(item.clone());
                }
            }
        }
    }
    let slot_total = slot_items.read().len();
    let first_visible = visible.first().cloned();

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            tabindex: "-1",
            autofocus: true,
            onkeydown: move |event| match dialog_key_action(&event, first_visible.is_some()) {
                Some(DialogKeyAction::Confirm) => {
                    if let Some(item) = first_visible.clone() {
                        on_pick.call(item);
                        on_close.call(());
                    }
                }
                Some(DialogKeyAction::Close) => on_close.call(()),
                None => {}
            },
            onclick: move |_| on_close.call(()),
            div {
                class: "flex max-h-[min(720px,calc(100vh-2rem))] w-full max-w-xl flex-col overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center gap-3 border-b p-4",
                    div { class: "min-w-0 flex-1",
                        div { class: "text-base font-semibold", "选择装备 · {slot.label()}" }
                        div { class: "text-xs text-muted-foreground", "共 {slot_total} 件可选装备" }
                    }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }
                div { class: "border-b p-3",
                    div { class: "relative",
                        Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                        input {
                            class: input_class("pl-9 pr-9"),
                            value: "{query_snapshot}",
                            placeholder: "搜索装备名称或职业",
                            autofocus: true,
                            oninput: move |event| query.set(event.value()),
                        }
                        if !query_snapshot.is_empty() {
                            button {
                                r#type: "button",
                                class: "absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent",
                                aria_label: "清除搜索",
                                title: "清除搜索",
                                onclick: move |_| query.set(String::new()),
                                Icon { kind: IconKind::X, class: "h-4 w-4" }
                            }
                        }
                    }
                }
                div { class: "min-h-0 flex-1 overflow-y-auto p-2",
                    if visible.is_empty() {
                        div { class: "py-10 text-center text-sm text-muted-foreground", "没有匹配的装备" }
                    } else {
                        for item in visible {
                            button {
                                key: "{item.id}",
                                r#type: "button",
                                class: "flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent",
                                onclick: {
                                    let picked = item.clone();
                                    move |_| {
                                        on_pick.call(picked.clone());
                                        on_close.call(());
                                    }
                                },
                                ItemIcon { icon: item.icon, size: "md" }
                                div { class: "min-w-0 flex-1",
                                    div { class: "truncate text-sm font-medium", "{item.name}" }
                                    div { class: "truncate text-xs text-muted-foreground",
                                        "品级 {item.level_item}"
                                        if !item.class_job_category_name.is_empty() {
                                            " · {item.class_job_category_name}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if total > PICKER_ROW_LIMIT {
                    div { class: "border-t p-2 text-center text-xs text-muted-foreground",
                        "还有 {total - PICKER_ROW_LIMIT} 条，请继续搜索"
                    }
                }
            }
        }
    }
}

#[component]
fn GlamourDyeDialog(
    slot: GlamourSlot,
    piece: GlamourPiece,
    stains: Rc<Vec<WeaponStain>>,
    on_stain: EventHandler<(usize, u8)>,
    on_close: EventHandler<()>,
) -> Element {
    let channels = [0_usize, 1_usize].map(|channel| {
        let current = piece.stains[channel];
        let name = stain_display_name(&stains[..], current);
        (channel, current, name)
    });

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            tabindex: "-1",
            autofocus: true,
            onkeydown: move |event| match dialog_key_action(&event, true) {
                Some(DialogKeyAction::Confirm) | Some(DialogKeyAction::Close) => on_close.call(()),
                None => {}
            },
            onclick: move |_| on_close.call(()),
            div {
                class: "flex max-h-[min(720px,calc(100vh-2rem))] w-full max-w-xl flex-col overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center gap-3 border-b p-4",
                    div { class: "min-w-0 flex-1",
                        div { class: "text-base font-semibold", "染色 · {slot.label()}" }
                        div { class: "truncate text-xs text-muted-foreground", "{piece.name}" }
                    }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }
                div { class: "min-h-0 flex-1 space-y-4 overflow-y-auto p-4",
                    if stains.is_empty() {
                        div { class: "rounded-md border border-amber-200 bg-amber-50 p-2 text-xs text-amber-700",
                            "染剂数据未加载，暂时只能记录为无染色。"
                        }
                    }
                    for (channel, current, name) in channels {
                        div { key: "channel-{channel}",
                            div { class: "mb-2 flex items-baseline gap-2 text-sm font-medium",
                                "通道 {channel + 1}"
                                span { class: "text-xs font-normal text-muted-foreground", "当前：{name}" }
                            }
                            div { class: "grid grid-cols-6 gap-1.5 sm:grid-cols-8",
                                button {
                                    r#type: "button",
                                    class: cx([
                                        "flex h-7 w-7 items-center justify-center rounded border text-[10px] text-muted-foreground transition-transform hover:scale-110",
                                        if current == 0 { "border-foreground/60 ring-2 ring-ring" } else { "border-dashed border-border" },
                                    ]),
                                    title: "无染色",
                                    aria_label: "无染色",
                                    onclick: move |_| {
                                        on_stain.call((channel, 0));
                                        on_close.call(());
                                    },
                                    "无"
                                }
                                for stain in stains.iter() {
                                    button {
                                        key: "{stain.id}",
                                        r#type: "button",
                                        class: cx([
                                            "h-7 w-7 rounded border shadow-sm transition-transform hover:scale-110",
                                            if stain.id == current { "border-foreground/60 ring-2 ring-ring" } else { "border-border" },
                                        ]),
                                        style: "{stain_color_style(stain)}",
                                        title: "{stain.name}",
                                        aria_label: "{stain.name}",
                                        onclick: {
                                            let stain_id = stain.id;
                                            move |_| {
                                                on_stain.call((channel, stain_id));
                                                on_close.call(());
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ConfirmDialog(
    title: String,
    description: String,
    confirm_label: &'static str,
    on_confirm: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            tabindex: "-1",
            autofocus: true,
            onkeydown: move |event| match dialog_key_action(&event, true) {
                Some(DialogKeyAction::Confirm) => {
                    on_confirm.call(());
                    on_close.call(());
                }
                Some(DialogKeyAction::Close) => on_close.call(()),
                None => {}
            },
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-sm overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center justify-between gap-3 border-b p-4",
                    div { class: "min-w-0 text-base font-semibold", "{title}" }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }
                div { class: "p-4 text-sm text-muted-foreground", "{description}" }
                div { class: "flex justify-end gap-2 border-t bg-muted/30 p-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    button {
                        r#type: "button",
                        class: "inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md bg-destructive px-3 text-sm font-medium text-destructive-foreground transition-colors hover:bg-destructive/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                        onclick: move |_| {
                            on_confirm.call(());
                            on_close.call(());
                        },
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}

#[component]
fn MessageDialog(title: String, message: String, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            tabindex: "-1",
            autofocus: true,
            onkeydown: move |event| match dialog_key_action(&event, true) {
                Some(DialogKeyAction::Confirm) | Some(DialogKeyAction::Close) => on_close.call(()),
                None => {}
            },
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-sm overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center justify-between gap-3 border-b p-4",
                    div { class: "min-w-0 text-base font-semibold", "{title}" }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }
                div { class: "p-4 text-sm text-muted-foreground", "{message}" }
                div { class: "flex justify-end gap-2 border-t bg-muted/30 p-3",
                    Button {
                        variant: ButtonVariant::Primary,
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                }
            }
        }
    }
}
