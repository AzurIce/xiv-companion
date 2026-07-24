use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;
use xiv_companion::inventory_collection::inventory_collection_sync;
use xiv_companion::{
    COLLECTION_CATEGORIES, CollectionItem, CollectionKind, ResourceOrigin, category_definition,
};

use crate::app::collection_index::{CollectionIndex, EquipmentSetGroup};
use crate::app::data::load_collection_catalog_with_metadata;
use crate::app::icons::{Icon, IconKind};
#[cfg(target_arch = "wasm32")]
use crate::app::inventory_state::{PersistedInventoryState, load_inventory_state};
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::format_integer;

use super::crafting::ItemIcon;

type ObtainedStore = Store<HashSet<u32>>;
type PendingWrites = Store<HashMap<u32, bool>>;
type CollectionIndexRef = Rc<CollectionIndex>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct JobFilterOption {
    label: &'static str,
    aliases: &'static [&'static str],
}

const BATTLE_JOB_OPTIONS: &[JobFilterOption] = &[
    job("骑士", &["战斗精英", "剑术师", "骑士"]),
    job("战士", &["战斗精英", "斧术师", "战士"]),
    job("暗黑骑士", &["战斗精英", "暗黑骑士"]),
    job("绝枪战士", &["战斗精英", "绝枪战士"]),
    job("武僧", &["战斗精英", "格斗家", "武僧"]),
    job("龙骑士", &["战斗精英", "枪术师", "龙骑士"]),
    job("忍者", &["战斗精英", "双剑师", "忍者"]),
    job("武士", &["战斗精英", "武士"]),
    job("钐镰客", &["战斗精英", "钐镰客"]),
    job("蝰蛇剑士", &["战斗精英", "蝰蛇剑士"]),
    job("吟游诗人", &["战斗精英", "弓箭手", "吟游诗人"]),
    job("机工士", &["战斗精英", "机工士"]),
    job("舞者", &["战斗精英", "舞者"]),
    job("白魔法师", &["魔法导师", "幻术师", "白魔法师"]),
    job("学者", &["魔法导师", "秘术师", "学者"]),
    job("占星术士", &["魔法导师", "占星术士"]),
    job("贤者", &["魔法导师", "贤者"]),
    job("黑魔法师", &["魔法导师", "咒术师", "黑魔法师"]),
    job("召唤师", &["魔法导师", "秘术师", "召唤师"]),
    job("赤魔法师", &["魔法导师", "赤魔法师"]),
    job("青魔法师", &["魔法导师", "青魔法师"]),
    job("绘灵法师", &["魔法导师", "绘灵法师"]),
];
const CRAFTER_JOB_OPTIONS: &[JobFilterOption] = &[
    job("刻木匠", &["能工巧匠", "刻木匠"]),
    job("锻铁匠", &["能工巧匠", "锻铁匠"]),
    job("铸甲匠", &["能工巧匠", "铸甲匠"]),
    job("雕金匠", &["能工巧匠", "雕金匠"]),
    job("制革匠", &["能工巧匠", "制革匠"]),
    job("裁衣匠", &["能工巧匠", "裁衣匠"]),
    job("炼金术士", &["能工巧匠", "炼金术士"]),
    job("烹调师", &["能工巧匠", "烹调师"]),
];
const GATHERER_JOB_OPTIONS: &[JobFilterOption] = &[
    job("采矿工", &["大地使者", "采矿工"]),
    job("园艺工", &["大地使者", "园艺工"]),
    job("捕鱼人", &["大地使者", "捕鱼人"]),
];
const BATTLE_JOB_TERMS: &[&str] = &[
    "战斗精英",
    "魔法导师",
    "剑术师",
    "斧术师",
    "格斗家",
    "枪术师",
    "弓箭手",
    "双剑师",
    "幻术师",
    "咒术师",
    "秘术师",
    "骑士",
    "战士",
    "暗黑骑士",
    "绝枪战士",
    "武僧",
    "龙骑士",
    "忍者",
    "武士",
    "钐镰客",
    "蝰蛇剑士",
    "吟游诗人",
    "机工士",
    "舞者",
    "白魔法师",
    "学者",
    "占星术士",
    "贤者",
    "黑魔法师",
    "召唤师",
    "赤魔法师",
    "青魔法师",
    "绘灵法师",
];
const CRAFTER_JOB_TERMS: &[&str] = &[
    "能工巧匠",
    "刻木匠",
    "锻铁匠",
    "铸甲匠",
    "雕金匠",
    "制革匠",
    "裁衣匠",
    "炼金术士",
    "烹调师",
];
const GATHERER_JOB_TERMS: &[&str] = &["大地使者", "采矿工", "园艺工", "捕鱼人"];

const fn job(label: &'static str, aliases: &'static [&'static str]) -> JobFilterOption {
    JobFilterOption { label, aliases }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquipmentCardEntry {
    Set(usize),
    Item(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObtainedFilter {
    All,
    Missing,
    Obtained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectionSection {
    Equipment,
    Unlocks,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NumericRangeFilter {
    min: Option<u16>,
    max: Option<u16>,
}

impl NumericRangeFilter {
    fn matches(self, value: u16) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CollectionSyncDialogState {
    Preview(CollectionSyncPreview),
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectionSyncPreview {
    replacement_ids: HashSet<u32>,
    detected_count: usize,
    added_items: Vec<CollectionItem>,
    removed_items: Vec<CollectionItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectionSyncApplication {
    replacement_ids: HashSet<u32>,
    added_count: usize,
    removed_count: usize,
}

async fn load_obtained_state() -> HashSet<u32> {
    #[cfg(target_arch = "wasm32")]
    {
        return crate::app::collection_state::load_collection_ids()
            .await
            .unwrap_or_default();
    }
    #[cfg(not(target_arch = "wasm32"))]
    HashSet::new()
}

async fn persist_obtained_state(obtained: HashSet<u32>) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return crate::app::collection_state::replace_collection_ids(&obtained).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = obtained;
        Ok(())
    }
}

async fn persist_obtained_entry(item_id: u32, obtained: bool) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return crate::app::collection_state::save_collection_entry(item_id, obtained).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (item_id, obtained);
        Ok(())
    }
}

#[component]
pub fn CollectionPage() -> Element {
    let catalog = use_resource(load_collection_catalog_with_metadata);
    let state = use_resource(load_obtained_state);
    #[cfg(target_arch = "wasm32")]
    let inventory_state = use_resource(load_inventory_state);
    let collection_index = use_memo(move || {
        catalog
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|loaded| Rc::new(CollectionIndex::new(loaded.data.clone())))
    });
    let mut active_kind = use_signal(|| CollectionKind::Equipment);
    let mut active_section = use_signal(|| CollectionSection::Equipment);
    let mut active_expansion = use_signal(String::new);
    let mut obtained_filter = use_signal(|| ObtainedFilter::All);
    let mut equipment_job_filter = use_signal(String::new);
    let mut equipment_slot_filter = use_signal(HashSet::<String>::new);
    let mut equipment_level_filter = use_signal(NumericRangeFilter::default);
    let mut equipment_item_level_filter = use_signal(NumericRangeFilter::default);
    let mut query = use_signal(String::new);
    let mut obtained = use_store(HashSet::<u32>::new);
    let pending_writes = use_store(HashMap::<u32, bool>::new);
    let mut hydrated = use_signal(|| false);
    let mut storage_message = use_signal(|| None::<Result<String, String>>);
    let mut import_input_version = use_signal(|| 0_u32);
    let mut bridge_dialog = use_signal(|| None::<CollectionSyncDialogState>);
    let mut bridge_applying = use_signal(|| false);

    use_effect(move || {
        if hydrated() {
            return;
        }
        let loaded_state = state.read().as_ref().cloned();
        let Some(loaded_state) = loaded_state else {
            return;
        };
        obtained.set(loaded_state);
        hydrated.set(true);
    });

    let catalog_snapshot = catalog.read().as_ref().cloned();
    let inventory_update_catalog = catalog_snapshot.clone();
    #[cfg(target_arch = "wasm32")]
    let inventory_snapshot = inventory_state.read().as_ref().cloned();
    #[cfg(target_arch = "wasm32")]
    let inventory_for_update = inventory_snapshot.clone();
    let index_snapshot = collection_index.read().clone();
    let query_snapshot = query();
    let hydrated_snapshot = hydrated();
    let kind_snapshot = active_kind();
    let section_snapshot = active_section();
    let active_expansion_snapshot = active_expansion();
    let obtained_filter_snapshot = obtained_filter();
    let equipment_job_snapshot = equipment_job_filter();
    let equipment_slot_snapshot = equipment_slot_filter();
    let equipment_level_snapshot = equipment_level_filter();
    let equipment_item_level_snapshot = equipment_item_level_filter();
    let storage_message_snapshot = storage_message();
    let bridge_dialog_snapshot = bridge_dialog();
    let bridge_applying_snapshot = bridge_applying();
    let has_pending_writes = !pending_writes.read().is_empty();
    let collection_transfer_ready = hydrated_snapshot && !has_pending_writes;
    #[cfg(target_arch = "wasm32")]
    let inventory_update_ready = matches!(&inventory_snapshot, Some(Ok(Some(_))));
    #[cfg(not(target_arch = "wasm32"))]
    let inventory_update_ready = false;

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-2 sm:px-5 lg:px-6",
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div { class: "min-w-0 space-y-0.5",
                        div { class: "text-xs text-muted-foreground", "数据" }
                        div { class: "flex flex-wrap items-center gap-x-2 gap-y-1",
                            h1 { class: "text-xl font-semibold leading-tight", "图鉴" }
                            crate::app::modules::ModuleCapabilityBadges { module_id: "collection" }
                        }
                    }
                    div { class: "flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2",
                        div { class: "relative w-full sm:w-72 lg:w-80",
                            Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                            input {
                                r#type: "text",
                                role: "searchbox",
                                placeholder: "搜索全部图鉴",
                                value: "{query_snapshot}",
                                class: input_class(if query_snapshot.is_empty() { "h-8 pl-9" } else { "h-8 pl-9 pr-9" }),
                                oninput: move |event| query.set(event.value()),
                                onchange: move |event| query.set(event.value()),
                            }
                            if !query_snapshot.is_empty() {
                                button {
                                    r#type: "button",
                                    title: "清空搜索",
                                    aria_label: "清空搜索",
                                    class: "absolute right-1 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                                    onclick: move |_| query.set(String::new()),
                                    Icon { kind: IconKind::X, class: "h-4 w-4" }
                                }
                            }
                        }
                        if section_snapshot == CollectionSection::Equipment {
                            Button {
                                variant: ButtonVariant::Outline,
                                size: ButtonSize::Sm,
                                disabled: !collection_transfer_ready || !inventory_update_ready || bridge_dialog_snapshot.is_some(),
                                title: if inventory_update_ready { "根据已保存的物品快照同步装备的新增和缺失状态" } else { "请先在物品页执行全量刷新" },
                                onclick: move |_| {
                                    let Some(Ok(loaded)) = inventory_update_catalog.as_ref() else { return; };
                                    #[cfg(target_arch = "wasm32")]
                                    let Some(Ok(Some(inventory))) = inventory_for_update.as_ref() else { return; };
                                    #[cfg(target_arch = "wasm32")]
                                    start_inventory_collection_update(
                                        inventory,
                                        loaded.data.items.clone(),
                                        bridge_dialog,
                                        obtained,
                                    );
                                },
                                Icon { kind: IconKind::RotateCcw, class: "h-4 w-4" }
                                "同步装备状态"
                            }
                        }
                        label {
                            class: if collection_transfer_ready { "inline-flex h-8 shrink-0 cursor-pointer items-center justify-center gap-2 rounded-md border border-input bg-background px-3 text-sm font-medium text-foreground transition-colors hover:bg-accent hover:text-accent-foreground" } else { "inline-flex h-8 shrink-0 cursor-not-allowed items-center justify-center gap-2 rounded-md border border-input bg-background px-3 text-sm font-medium text-foreground opacity-50" },
                            title: if collection_transfer_ready { "导入图鉴状态" } else { "正在加载或保存图鉴状态" },
                            Icon { kind: IconKind::Download, class: "h-4 w-4" }
                            "导入"
                            input {
                                key: "{import_input_version}",
                                r#type: "file",
                                accept: "application/json,.json",
                                class: "hidden",
                                disabled: !collection_transfer_ready,
                                onchange: move |event| {
                                    let Some(file) = event.files().into_iter().next() else { return; };
                                    spawn(async move {
                                        let result = async {
                                            let json = file.read_string().await.map_err(|error| format!("读取导入文件失败: {error}"))?;
                                            let item_ids = crate::app::collection_state::import_collection_json(&json)?;
                                            persist_obtained_state(item_ids.clone()).await?;
                                            let count = item_ids.len();
                                            obtained.set(item_ids);
                                            Ok(format!("已导入 {count} 项图鉴状态"))
                                        }.await;
                                        storage_message.set(Some(result));
                                        import_input_version += 1;
                                    });
                                },
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            disabled: !collection_transfer_ready,
                            onclick: move |_| {
                                let result = crate::app::collection_state::export_collection_json(&obtained.read())
                                    .and_then(|json| download_collection_json(&json))
                                    .map(|_| format!("已导出 {} 项图鉴状态", obtained.read().len()));
                                storage_message.set(Some(result));
                            },
                            Icon { kind: IconKind::Upload, class: "h-4 w-4" }
                            "导出"
                        }
                        if let Some(Ok(loaded)) = &catalog_snapshot {
                            div { class: "flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                            Badge {
                                variant: if loaded.metadata.origin == Some(ResourceOrigin::UserLocal) { BadgeVariant::Success } else { BadgeVariant::Outline },
                                {origin_label(loaded.metadata.origin)}
                            }
                            span { "{loaded.data.game_version}" }
                            span { "{format_integer(loaded.data.counts.items as f64)} 项" }
                            }
                        }
                    }
                }
                if let Some(Ok(message)) = &storage_message_snapshot {
                    div { class: "mt-2 text-xs text-emerald-700", "{message}" }
                }
                if let Some(Err(error)) = &storage_message_snapshot {
                    div { class: "mt-2 text-xs text-destructive", "{error}" }
                }
            }

            match catalog_snapshot {
                None => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        Icon { kind: IconKind::LoaderCircle, class: "h-5 w-5 animate-spin text-muted-foreground" }
                    }
                },
                Some(Err(error)) => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::Database, class: "h-6 w-6" } },
                            title: "图鉴数据未就绪".to_string(),
                            description: Some(error),
                            action: rsx! {
                                a { href: "#/",
                                    Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, "打开资源库" }
                                }
                            },
                        }
                    }
                },
                Some(Ok(loaded)) => {
                    let index = index_snapshot.expect("collection index follows loaded catalog");
                    let slot_options = index
                        .equipment_slot_options
                        .iter()
                        .filter(|label| label.as_str() != "复合部位")
                        .cloned()
                        .collect::<Vec<_>>();
                    let selected_expansion = index
                        .collection_expansions
                        .iter()
                        .find(|name| *name == &active_expansion_snapshot)
                        .or_else(|| index.collection_expansions.first())
                        .cloned()
                        .unwrap_or_default();
                    let normalized_global_query = query_snapshot.trim().to_lowercase();
                    let mut search_match_kinds = HashSet::<CollectionKind>::new();
                    let mut search_match_kind_expansions =
                        HashSet::<(CollectionKind, String)>::new();
                    if !normalized_global_query.is_empty() {
                        for item in &index.catalog.items {
                            if collection_item_matches_query(item, &normalized_global_query) {
                                search_match_kinds.insert(item.kind);
                                search_match_kind_expansions
                                    .insert((item.kind, item.expansion.clone()));
                            }
                        }
                        for set in &index.equipment_sets {
                            if set.search_text.contains(&normalized_global_query) {
                                search_match_kinds.insert(CollectionKind::Equipment);
                                search_match_kind_expansions
                                    .insert((CollectionKind::Equipment, set.expansion.clone()));
                            }
                        }
                    }
                    let mut collected_by_kind = HashMap::<CollectionKind, usize>::new();
                    let mut collected_by_kind_expansion =
                        HashMap::<(CollectionKind, String), usize>::new();
                    if hydrated_snapshot {
                        for item in &index.catalog.items {
                            if is_obtained(obtained, item.id) {
                                *collected_by_kind.entry(item.kind).or_default() += 1;
                                *collected_by_kind_expansion
                                    .entry((item.kind, item.expansion.clone()))
                                    .or_default() += 1;
                            }
                        }
                    }
                    let equipment_total = loaded.data.counts.equipment;
                    let unlock_total = loaded.data.counts.items.saturating_sub(equipment_total);
                    let equipment_collected = collected_by_kind
                        .get(&CollectionKind::Equipment)
                        .copied()
                        .unwrap_or_default();
                    let unlock_collected = collected_by_kind
                        .iter()
                        .filter(|(kind, _)| **kind != CollectionKind::Equipment)
                        .map(|(_, count)| *count)
                        .sum::<usize>();
                    let equipment_search_match =
                        search_match_kinds.contains(&CollectionKind::Equipment);
                    let unlock_search_match = search_match_kinds
                        .iter()
                        .any(|kind| *kind != CollectionKind::Equipment);
                    let unlock_kind = COLLECTION_CATEGORIES
                        .iter()
                        .map(|entry| entry.kind)
                        .find(|kind| *kind != CollectionKind::Equipment)
                        .unwrap_or(CollectionKind::Mount);
                    rsx! {
                        div { class: "flex min-h-0 flex-1 flex-col overflow-hidden",
                            div { class: "border-b px-4 sm:px-6 lg:px-8",
                                div { class: "flex gap-1 overflow-x-auto py-2",
                                    CollectionSectionTab {
                                        label: "装备",
                                        collected: equipment_collected,
                                        total: equipment_total,
                                        active: section_snapshot == CollectionSection::Equipment,
                                        search_match: equipment_search_match,
                                        onclick: move |_| {
                                            active_section.set(CollectionSection::Equipment);
                                            active_kind.set(CollectionKind::Equipment);
                                            active_expansion.set(String::new());
                                            obtained_filter.set(ObtainedFilter::All);
                                            equipment_job_filter.set(String::new());
                                            equipment_slot_filter.set(HashSet::new());
                                            equipment_level_filter.set(NumericRangeFilter::default());
                                            equipment_item_level_filter.set(NumericRangeFilter::default());
                                        },
                                    }
                                    CollectionSectionTab {
                                        label: "解锁",
                                        collected: unlock_collected,
                                        total: unlock_total,
                                        active: section_snapshot == CollectionSection::Unlocks,
                                        search_match: unlock_search_match,
                                        onclick: move |_| {
                                            active_section.set(CollectionSection::Unlocks);
                                            active_kind.set(unlock_kind);
                                            active_expansion.set(String::new());
                                            obtained_filter.set(ObtainedFilter::All);
                                            equipment_job_filter.set(String::new());
                                            equipment_slot_filter.set(HashSet::new());
                                            equipment_level_filter.set(NumericRangeFilter::default());
                                            equipment_item_level_filter.set(NumericRangeFilter::default());
                                        },
                                    }
                                }
                            }
                            if section_snapshot == CollectionSection::Unlocks {
                                div { class: "border-b px-4 sm:px-6 lg:px-8",
                                    div { class: "flex flex-wrap gap-2 py-3",
                                        for kind in COLLECTION_CATEGORIES.iter().map(|entry| entry.kind).filter(|kind| *kind != CollectionKind::Equipment) {
                                            CollectionKindTab {
                                                kind,
                                                collected: collected_by_kind.get(&kind).copied().unwrap_or_default(),
                                                total: loaded.data.counts.count_for(kind),
                                                active: kind == kind_snapshot,
                                                search_match: search_match_kinds.contains(&kind),
                                                onclick: move |_| {
                                                    active_kind.set(kind);
                                                    active_expansion.set(String::new());
                                                    obtained_filter.set(ObtainedFilter::All);
                                                    equipment_job_filter.set(String::new());
                                                    equipment_slot_filter.set(HashSet::new());
                                                    equipment_level_filter.set(NumericRangeFilter::default());
                                                    equipment_item_level_filter.set(NumericRangeFilter::default());
                                                },
                                            }
                                        }
                                    }
                                }
                            }

                            if section_snapshot == CollectionSection::Equipment {
                                div { class: "space-y-3 border-b px-4 py-3 sm:px-6 lg:px-8",
                                    JobFilterSelector {
                                        selected: equipment_job_snapshot.clone(),
                                        on_change: move |value| equipment_job_filter.set(value),
                                    }
                                    MultiSelectTagCloud {
                                        label: "部位",
                                        options: slot_options.clone(),
                                        selected: equipment_slot_snapshot.clone(),
                                        on_change: move |value: String| {
                                            let mut next = equipment_slot_filter.peek().clone();
                                            if value.is_empty() {
                                                next.clear();
                                            } else if !next.insert(value.clone()) {
                                                next.remove(&value);
                                            }
                                            equipment_slot_filter.set(next);
                                        },
                                    }
                                    div { class: "flex items-start gap-3",
                                        div { class: "w-10 shrink-0 pt-1 text-xs font-medium text-muted-foreground", "范围" }
                                        div { class: "flex min-w-0 flex-wrap items-center gap-x-5 gap-y-2",
                                            NumericRangeSelector {
                                                label: "装备等级",
                                                available: index.equipment_level_bounds,
                                                value: equipment_level_snapshot,
                                                on_change: move |value| equipment_level_filter.set(value),
                                            }
                                            NumericRangeSelector {
                                                label: "品级",
                                                available: index.equipment_item_level_bounds,
                                                value: equipment_item_level_snapshot,
                                                on_change: move |value| equipment_item_level_filter.set(value),
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "border-b px-4 sm:px-6 lg:px-8",
                                div { class: "flex gap-1 overflow-x-auto py-2",
                                    for expansion in &index.collection_expansions {
                                        ExpansionTab {
                                            key: "{expansion}",
                                            label: expansion.clone(),
                                            collected: collected_by_kind_expansion
                                                .get(&(kind_snapshot, expansion.clone()))
                                                .copied()
                                                .unwrap_or_default(),
                                            total: index.count_for_kind_expansion(kind_snapshot, expansion),
                                            active: expansion == &selected_expansion,
                                            search_match: search_match_kind_expansions
                                                .contains(&(kind_snapshot, expansion.clone())),
                                            onclick: {
                                                let expansion = expansion.clone();
                                                move |_| active_expansion.set(expansion.clone())
                                            },
                                        }
                                    }
                                }
                            }

                            div { class: "flex min-w-0 flex-wrap items-center gap-2 border-b px-4 py-2 sm:px-6 lg:px-8",
                                select {
                                    class: input_class("min-w-0 flex-1 basis-32 sm:max-w-40"),
                                    value: match obtained_filter_snapshot { ObtainedFilter::All => "all", ObtainedFilter::Missing => "missing", ObtainedFilter::Obtained => "obtained" },
                                    onchange: move |event| {
                                        obtained_filter.set(match event.value().as_str() {
                                            "missing" => ObtainedFilter::Missing,
                                            "obtained" => ObtainedFilter::Obtained,
                                            _ => ObtainedFilter::All,
                                        });
                                    },
                                    option { value: "all", "全部状态" }
                                    if section_snapshot == CollectionSection::Equipment {
                                        option { value: "missing", "未持有" }
                                        option { value: "obtained", "已持有" }
                                    } else {
                                        option { value: "missing", "未解锁" }
                                        option { value: "obtained", "已解锁" }
                                    }
                                }
                            }

                            div { class: "min-h-0 flex-1 overflow-y-auto px-4 py-3 sm:px-6 lg:px-8",
                                if !hydrated_snapshot {
                                    div { class: "flex min-h-48 items-center justify-center",
                                        Icon { kind: IconKind::LoaderCircle, class: "h-5 w-5 animate-spin text-muted-foreground" }
                                    }
                                } else if section_snapshot == CollectionSection::Equipment {
                                    EquipmentCollectionView {
                                        key: "{selected_expansion}|{query_snapshot}|{obtained_filter_snapshot:?}",
                                        index: index.clone(),
                                        expansion: selected_expansion,
                                        query: query_snapshot.clone(),
                                        filter: obtained_filter_snapshot,
                                        job_filter: equipment_job_snapshot.clone(),
                                        slot_filter: equipment_slot_snapshot.clone(),
                                        level_filter: equipment_level_snapshot,
                                        item_level_filter: equipment_item_level_snapshot,
                                        obtained,
                                        on_toggle: move |item_id| toggle_obtained(item_id, obtained, pending_writes, storage_message),
                                    }
                                } else {
                                    FlatCollectionView {
                                        index,
                                        kind: kind_snapshot,
                                        expansion: selected_expansion,
                                        query: query_snapshot.clone(),
                                        filter: obtained_filter_snapshot,
                                        obtained,
                                        on_toggle: move |item_id| toggle_obtained(item_id, obtained, pending_writes, storage_message),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(sync_state) = bridge_dialog_snapshot {
                CollectionSyncDialog {
                    state: sync_state,
                    applying: bridge_applying_snapshot,
                    on_close: move |_| {
                        bridge_dialog.set(None);
                        bridge_applying.set(false);
                    },
                    on_confirm: move |application: CollectionSyncApplication| {
                        bridge_applying.set(true);
                        spawn(async move {
                            let result = async {
                                #[cfg(target_arch = "wasm32")]
                                crate::app::collection_state::replace_collection_ids(
                                    &application.replacement_ids,
                                )
                                .await?;
                                #[cfg(not(target_arch = "wasm32"))]
                                let _ = &application.replacement_ids;
                                obtained.set(application.replacement_ids);
                                Ok::<(usize, usize), String>((
                                    application.added_count,
                                    application.removed_count,
                                ))
                            }.await;
                            match result {
                                Ok((added, removed)) => {
                                    storage_message.set(Some(Ok(format!(
                                        "已同步装备状态：新增 {added} 项，移除 {removed} 项"
                                    ))));
                                    bridge_dialog.set(None);
                                }
                                Err(error) => bridge_dialog.set(Some(CollectionSyncDialogState::Error(error))),
                            }
                            bridge_applying.set(false);
                        });
                    },
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn start_inventory_collection_update(
    inventory: &PersistedInventoryState,
    catalog: Vec<CollectionItem>,
    mut dialog: Signal<Option<CollectionSyncDialogState>>,
    obtained: ObtainedStore,
) {
    let owned_item_ids = inventory.item_ids();
    let sync = inventory_collection_sync(&owned_item_ids, &obtained.peek(), &catalog);
    let mut added_items = catalog
        .iter()
        .filter(|item| sync.added_ids.contains(&item.id))
        .cloned()
        .collect::<Vec<_>>();
    let mut removed_items = catalog
        .iter()
        .filter(|item| sync.removed_ids.contains(&item.id))
        .cloned()
        .collect::<Vec<_>>();
    added_items.sort_by(collection_sync_item_order);
    removed_items.sort_by(collection_sync_item_order);
    dialog.set(Some(CollectionSyncDialogState::Preview(
        CollectionSyncPreview {
            replacement_ids: sync.next_collection_ids,
            detected_count: sync.detected_equipment_ids.len(),
            added_items,
            removed_items,
        },
    )));
}

fn collection_sync_item_order(left: &CollectionItem, right: &CollectionItem) -> std::cmp::Ordering {
    left.name
        .cmp(&right.name)
        .then_with(|| left.id.cmp(&right.id))
}

#[component]
fn CollectionSyncDialog(
    state: CollectionSyncDialogState,
    applying: bool,
    on_confirm: EventHandler<CollectionSyncApplication>,
    on_close: EventHandler<()>,
) -> Element {
    let can_confirm = matches!(&state, CollectionSyncDialogState::Preview(preview) if !preview.added_items.is_empty() || !preview.removed_items.is_empty())
        && !applying;

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            onclick: move |_| {
                if !applying {
                    on_close.call(());
                }
            },
            div {
                class: "flex max-h-[min(760px,calc(100vh-2rem))] w-full max-w-2xl flex-col overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center gap-3 border-b p-4",
                    div { class: "flex h-9 w-9 items-center justify-center rounded-md border bg-background",
                        Icon { kind: IconKind::RotateCcw, class: "h-4 w-4" }
                    }
                    div { class: "min-w-0 flex-1",
                        div { class: "text-base font-semibold", "同步装备状态" }
                        div { class: "text-xs text-muted-foreground", "根据已保存的物品快照双向对齐" }
                    }
                    button {
                        r#type: "button",
                        disabled: applying,
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground disabled:pointer-events-none disabled:opacity-50",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }

                div { class: "min-h-0 flex-1 overflow-y-auto p-4",
                    match &state {
                        CollectionSyncDialogState::Error(error) => rsx! {
                            div { class: "flex min-h-48 items-center justify-center text-sm text-destructive", "{error}" }
                        },
                        CollectionSyncDialogState::Preview(preview) => rsx! {
                            div { class: "space-y-4",
                                div { class: "grid grid-cols-3 gap-3",
                                    SyncMetric { label: "快照持有", value: preview.detected_count }
                                    SyncMetric { label: "新增", value: preview.added_items.len() }
                                    SyncMetric { label: "移除", value: preview.removed_items.len() }
                                }
                                if preview.added_items.is_empty() && preview.removed_items.is_empty() {
                                    div { class: "flex min-h-32 items-center justify-center text-sm text-muted-foreground", "装备图鉴状态已与物品快照一致" }
                                } else {
                                    div { class: "space-y-4",
                                        if !preview.added_items.is_empty() {
                                            div {
                                                div { class: "mb-2 text-xs font-medium text-emerald-700", "新增持有" }
                                                div { class: "divide-y rounded-md border",
                                                    for item in &preview.added_items {
                                                        CollectionSyncItemRow { item: item.clone() }
                                                    }
                                                }
                                            }
                                        }
                                        if !preview.removed_items.is_empty() {
                                            div {
                                                div { class: "mb-2 text-xs font-medium text-destructive", "不再持有" }
                                                div { class: "divide-y rounded-md border",
                                                    for item in &preview.removed_items {
                                                        CollectionSyncItemRow { item: item.clone() }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }

                div { class: "flex justify-end gap-2 border-t bg-muted/30 p-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: applying,
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: !can_confirm,
                        onclick: move |_| {
                            if let CollectionSyncDialogState::Preview(preview) = &state {
                                on_confirm.call(CollectionSyncApplication {
                                    replacement_ids: preview.replacement_ids.clone(),
                                    added_count: preview.added_items.len(),
                                    removed_count: preview.removed_items.len(),
                                });
                            }
                        },
                        if applying {
                            Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                        }
                        "确定"
                    }
                }
            }
        }
    }
}

#[component]
fn CollectionSyncItemRow(item: CollectionItem) -> Element {
    rsx! {
        div { class: "flex items-center gap-3 px-3 py-2.5",
            ItemIcon { icon: item.icon, size: "sm" }
            div { class: "min-w-0 flex-1",
                div { class: "truncate text-sm font-medium", "{item.name}" }
                div { class: "text-xs text-muted-foreground", "装备 · {item.id}" }
            }
        }
    }
}

#[component]
fn SyncMetric(label: &'static str, value: usize) -> Element {
    rsx! {
        div { class: "rounded-md border bg-background p-3",
            div { class: "text-xs text-muted-foreground", "{label}" }
            div { class: "mt-1 text-xl font-semibold tabular-nums", "{value}" }
        }
    }
}

fn toggle_obtained(
    item_id: u32,
    mut store: ObtainedStore,
    mut pending_writes: PendingWrites,
    mut storage_message: Signal<Option<Result<String, String>>>,
) {
    storage_message.set(None);
    let next = !store.peek().contains(&item_id);
    if next {
        store.write().insert(item_id);
    } else {
        store.write().remove(&item_id);
    }
    let writer_active = pending_writes.peek().contains_key(&item_id);
    pending_writes.write().insert(item_id, next);
    if writer_active {
        return;
    }

    spawn(async move {
        loop {
            let Some(desired) = pending_writes.peek().get(&item_id).copied() else {
                break;
            };
            match persist_obtained_entry(item_id, desired).await {
                Ok(()) => {
                    if pending_writes.peek().get(&item_id).copied() == Some(desired) {
                        pending_writes.write().remove(&item_id);
                        break;
                    }
                }
                Err(error) => {
                    if pending_writes.peek().get(&item_id).copied() != Some(desired) {
                        continue;
                    }
                    pending_writes.write().remove(&item_id);
                    if store.peek().contains(&item_id) == desired {
                        if desired {
                            store.write().remove(&item_id);
                        } else {
                            store.write().insert(item_id);
                        }
                    }
                    storage_message.set(Some(Err(error.clone())));
                    crate::app::log::warn("collection", format!("保存图鉴状态失败: {error}"));
                    break;
                }
            }
        }
    });
}

fn is_obtained(store: ObtainedStore, item_id: u32) -> bool {
    store.read().contains(&item_id)
}

fn download_collection_json(json: &str) -> Result<(), String> {
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
        anchor.set_download("xiv-companion-collections.json");
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

fn origin_label(origin: Option<ResourceOrigin>) -> &'static str {
    match origin {
        Some(ResourceOrigin::UserLocal) => "本地数据",
        Some(ResourceOrigin::Network) => "网络数据",
        _ => "内置数据",
    }
}

#[component]
fn CollectionSectionTab(
    label: &'static str,
    collected: usize,
    total: usize,
    active: bool,
    search_match: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let progress = if total == 0 {
        0.0
    } else {
        (collected as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    };
    let interaction_style = if active {
        "border-color: #737373; box-shadow: 0 0 0 1px rgba(115, 115, 115, 0.12);"
    } else if search_match {
        "border-color: #f59e0b; box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.20);"
    } else {
        "border-color: #e5e5e5; box-shadow: none;"
    };
    let progress_label = format!("{label}：{collected}/{total}（{progress:.1}%）");

    rsx! {
        button {
            r#type: "button",
            class: if active {
                "relative isolate cursor-pointer overflow-hidden rounded-md border bg-primary/5 px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
            } else {
                "relative isolate cursor-pointer overflow-hidden rounded-md border bg-background px-4 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            },
            aria_pressed: active,
            aria_label: "{progress_label}",
            title: "{progress_label}",
            style: "{interaction_style}",
            onclick: move |event| onclick.call(event),
            if progress > 0.0 {
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute inset-y-0 left-0 z-0",
                    style: "width: max({progress:.2}%, 4px); background-color: rgba(16, 185, 129, 0.12);",
                }
            }
            span { class: "relative z-10 inline-flex items-center gap-2",
                "{label}"
                span { class: "text-xs tabular-nums text-muted-foreground", "{collected}/{total}" }
            }
        }
    }
}

#[component]
fn MultiSelectTagCloud(
    label: &'static str,
    options: Vec<String>,
    selected: HashSet<String>,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "flex gap-3",
            div { class: "w-10 shrink-0 pt-1 text-xs font-medium text-muted-foreground", "{label}" }
            div { class: "flex min-w-0 flex-wrap gap-1.5",
                button {
                    r#type: "button",
                    class: if selected.is_empty() {
                        "rounded border border-foreground bg-foreground px-2 py-1 text-xs font-medium text-background"
                    } else {
                        "rounded border border-input bg-background px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
                    },
                    aria_pressed: selected.is_empty(),
                    onclick: move |_| on_change.call(String::new()),
                    "全部"
                }
                for option in options {
                    {
                        let active = selected.contains(&option);
                        let value = option.clone();
                        rsx! {
                            button {
                                key: "{option}",
                                r#type: "button",
                                class: if active {
                                    "rounded border border-foreground bg-foreground px-2 py-1 text-xs font-medium text-background"
                                } else {
                                    "rounded border border-input bg-background px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
                                },
                                aria_pressed: active,
                                onclick: move |_| on_change.call(value.clone()),
                                "{option}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NumericRangeSelector(
    label: &'static str,
    available: (u16, u16),
    value: NumericRangeFilter,
    on_change: EventHandler<NumericRangeFilter>,
) -> Element {
    let available_title = match available {
        (0, 0) => format!("{label}范围"),
        (min, max) if min == max => format!("{label}可用值：{min}"),
        (min, max) => format!("{label}可用范围：{min}-{max}"),
    };
    rsx! {
        div { class: "flex shrink-0 items-center gap-1.5", title: "{available_title}",
            span { class: "mr-0.5 text-xs font-medium text-muted-foreground", "{label}" }
            div { class: "flex items-center gap-1",
                input {
                    r#type: "number",
                    min: "0",
                    max: "{available.1}",
                    step: "1",
                    inputmode: "numeric",
                    aria_label: "{label}最低值",
                    placeholder: "最低",
                    value: value.min.map(|value| value.to_string()).unwrap_or_default(),
                    class: input_class("h-7 w-[4.5rem] px-2 text-xs tabular-nums"),
                    oninput: move |event| {
                        on_change.call(NumericRangeFilter {
                            min: parse_optional_u16(&event.value()),
                            ..value
                        });
                    },
                }
                span { class: "text-xs text-muted-foreground", "-" }
                input {
                    r#type: "number",
                    min: "0",
                    max: "{available.1}",
                    step: "1",
                    inputmode: "numeric",
                    aria_label: "{label}最高值",
                    placeholder: "最高",
                    value: value.max.map(|value| value.to_string()).unwrap_or_default(),
                    class: input_class("h-7 w-[4.5rem] px-2 text-xs tabular-nums"),
                    oninput: move |event| {
                        on_change.call(NumericRangeFilter {
                            max: parse_optional_u16(&event.value()),
                            ..value
                        });
                    },
                }
                if value != NumericRangeFilter::default() {
                    button {
                        r#type: "button",
                        title: "清除{label}范围",
                        aria_label: "清除{label}范围",
                        class: "flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        onclick: move |_| on_change.call(NumericRangeFilter::default()),
                        Icon { kind: IconKind::X, class: "h-3.5 w-3.5" }
                    }
                }
            }
        }
    }
}

fn parse_optional_u16(value: &str) -> Option<u16> {
    let value = value.trim();
    (!value.is_empty())
        .then(|| value.parse::<u16>().ok())
        .flatten()
}

#[component]
fn JobFilterSelector(selected: String, on_change: EventHandler<String>) -> Element {
    rsx! {
        div { class: "flex gap-3",
            div { class: "w-10 shrink-0 pt-1 text-xs font-medium text-muted-foreground", "职业" }
            div { class: "min-w-0 flex-1 space-y-1.5",
                button {
                    r#type: "button",
                    class: job_filter_button_class(selected.is_empty()),
                    aria_pressed: selected.is_empty(),
                    onclick: move |_| on_change.call(String::new()),
                    "全部"
                }
                JobFilterGroup {
                    label: "战斗精英",
                    options: BATTLE_JOB_OPTIONS,
                    selected: selected.clone(),
                    on_change,
                }
                JobFilterGroup {
                    label: "能工巧匠",
                    options: CRAFTER_JOB_OPTIONS,
                    selected: selected.clone(),
                    on_change,
                }
                JobFilterGroup {
                    label: "大地使者",
                    options: GATHERER_JOB_OPTIONS,
                    selected: selected.clone(),
                    on_change,
                }
            }
        }
    }
}

#[component]
fn JobFilterGroup(
    label: &'static str,
    options: &'static [JobFilterOption],
    selected: String,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "flex gap-2",
            button {
                r#type: "button",
                class: format!("w-20 shrink-0 {}", job_filter_button_class(selected == label)),
                aria_pressed: selected == label,
                onclick: move |_| on_change.call(label.to_string()),
                "{label}"
            }
            div { class: "flex min-w-0 flex-wrap gap-1.5",
                for option in options {
                    {
                        let value = option.label.to_string();
                        let active = selected == option.label;
                        rsx! {
                            button {
                                key: "{option.label}",
                                r#type: "button",
                                class: job_filter_button_class(active),
                                aria_pressed: active,
                                onclick: move |_| on_change.call(value.clone()),
                                "{option.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn job_filter_button_class(active: bool) -> &'static str {
    if active {
        "rounded border border-foreground bg-foreground px-2 py-1 text-xs font-medium text-background"
    } else {
        "rounded border border-input bg-background px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
    }
}

#[component]
fn CollectionKindTab(
    kind: CollectionKind,
    collected: usize,
    total: usize,
    active: bool,
    search_match: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let progress = if total == 0 {
        0.0
    } else {
        (collected as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    };
    let complete = total > 0 && collected == total;
    let progress_label = format!("{}：{collected}/{total}（{progress:.1}%）", kind.label());
    let interaction_style = match (active, search_match, complete) {
        (true, true, _) => "border-color: #f59e0b; box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.22);",
        (true, false, _) => {
            "border-color: #a3a3a3; box-shadow: 0 0 0 1px rgba(115, 115, 115, 0.10);"
        }
        (false, true, _) => {
            "border-color: #f59e0b; box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.20);"
        }
        (false, false, true) => "border-color: #a7f3d0; box-shadow: none;",
        (false, false, false) => "border-color: #e5e5e5; box-shadow: none;",
    };
    rsx! {
        button {
            r#type: "button",
            class: if complete {
                "relative isolate cursor-pointer overflow-hidden rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1.5 text-sm font-medium text-emerald-700 transition-colors hover:bg-accent"
            } else if active {
                "relative isolate cursor-pointer overflow-hidden rounded-full border bg-primary/5 px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:bg-accent"
            } else {
                "relative isolate cursor-pointer overflow-hidden rounded-full border bg-background px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            },
            aria_pressed: active,
            aria_label: "{progress_label}",
            title: "{progress_label}",
            style: "{interaction_style}",
            onclick: move |event| onclick.call(event),
            if total == 0 {
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute inset-0 z-0",
                    style: "background-color: rgba(115, 115, 115, 0.08); background-image: repeating-linear-gradient(135deg, rgba(115, 115, 115, 0.13) 0 5px, rgba(115, 115, 115, 0.03) 5px 10px);",
                }
            }
            if !complete && progress > 0.0 {
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute inset-y-0 left-0 z-0",
                    style: "width: max({progress:.2}%, 4px); background-color: rgba(16, 185, 129, 0.14); background-image: repeating-linear-gradient(135deg, rgba(16, 185, 129, 0.17) 0 5px, rgba(16, 185, 129, 0.04) 5px 10px); box-shadow: inset -1px 0 rgba(5, 150, 105, 0.24);",
                }
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute left-0 z-0",
                    style: "bottom: 0; height: 2px; width: max({progress:.2}%, 4px); background-color: #059669;",
                }
            }
            span { class: "relative z-10 inline-flex items-center",
                "{kind.label()}"
                if category_definition(kind).experimental {
                    span { class: "ml-1 rounded border px-1 py-0.5 text-[10px] font-normal text-muted-foreground", "实验" }
                }
                span { class: if complete { "ml-1 text-xs tabular-nums text-emerald-700" } else { "ml-1 text-xs tabular-nums text-muted-foreground" }, "{collected}/{total}" }
            }
        }
    }
}

#[component]
fn ExpansionTab(
    label: String,
    collected: usize,
    total: usize,
    active: bool,
    search_match: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let display_label = expansion_display_label(&label);
    let progress = if total == 0 {
        0.0
    } else {
        (collected as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    };
    let complete = total > 0 && collected == total;
    let progress_label = format!("{display_label}：{collected}/{total}（{progress:.1}%）");
    let interaction_style = match (active, search_match, complete) {
        (true, true, _) => {
            "border-bottom-color: #737373; border-bottom-width: 2px; box-shadow: inset 0 0 0 1px rgba(245, 158, 11, 0.48);"
        }
        (true, false, _) => {
            "border-bottom-color: #737373; border-bottom-width: 2px; box-shadow: none;"
        }
        (false, true, _) => {
            "border-bottom-color: #f59e0b; border-bottom-width: 2px; box-shadow: inset 0 0 0 1px rgba(245, 158, 11, 0.38);"
        }
        (false, false, true) => {
            "border-bottom-color: #10b981; border-bottom-width: 2px; box-shadow: none;"
        }
        (false, false, false) => {
            "border-bottom-color: transparent; border-bottom-width: 2px; box-shadow: none;"
        }
    };
    rsx! {
        button {
            r#type: "button",
            class: if complete {
                "relative isolate shrink-0 cursor-pointer overflow-hidden border-b-2 border-emerald-500 bg-emerald-50 px-3 py-2 text-sm font-medium text-emerald-700 transition-colors hover:bg-accent"
            } else if active {
                "relative isolate shrink-0 cursor-pointer overflow-hidden border-b-2 bg-primary/5 px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
            } else {
                "relative isolate shrink-0 cursor-pointer overflow-hidden border-b-2 border-transparent px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            },
            aria_pressed: active,
            aria_label: "{progress_label}",
            title: "{progress_label}",
            style: "{interaction_style}",
            onclick: move |event| onclick.call(event),
            if total == 0 {
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute inset-0 z-0",
                    style: "background-color: rgba(115, 115, 115, 0.08); background-image: repeating-linear-gradient(135deg, rgba(115, 115, 115, 0.13) 0 5px, rgba(115, 115, 115, 0.03) 5px 10px);",
                }
            }
            if !complete && progress > 0.0 {
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute inset-y-0 left-0 z-0",
                    style: "width: max({progress:.2}%, 4px); background-color: rgba(16, 185, 129, 0.12); background-image: repeating-linear-gradient(135deg, rgba(16, 185, 129, 0.15) 0 5px, rgba(16, 185, 129, 0.03) 5px 10px); box-shadow: inset -1px 0 rgba(5, 150, 105, 0.22);",
                }
                span {
                    aria_hidden: "true",
                    class: "pointer-events-none absolute left-0 z-0",
                    style: "top: 0; height: 2px; width: max({progress:.2}%, 4px); background-color: #059669;",
                }
            }
            span { class: "relative z-10",
                "{display_label}"
                span { class: if complete { "ml-1 text-xs tabular-nums text-emerald-700" } else { "ml-1 text-xs tabular-nums text-muted-foreground" }, "{collected}/{total}" }
            }
        }
    }
}

fn expansion_display_label(label: &str) -> String {
    let series = match label {
        "旧版遗留" => "1.x",
        "重生之境" => "2.x",
        "苍穹之禁城" => "3.x",
        "红莲之狂潮" => "4.x",
        "暗影之逆焰" => "5.x",
        "晓月之终途" => "6.x",
        "金曦之遗辉" => "7.x",
        "历史版本" => return "历史版本 · 4.45 及以前".to_string(),
        _ => return label.to_string(),
    };
    format!("{label} · {series}")
}

#[component]
fn EquipmentCollectionView(
    index: CollectionIndexRef,
    expansion: String,
    query: String,
    filter: ObtainedFilter,
    job_filter: String,
    slot_filter: HashSet<String>,
    level_filter: NumericRangeFilter,
    item_level_filter: NumericRangeFilter,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let normalized_query = query.trim().to_lowercase();
    let mut patches: BTreeMap<String, Vec<EquipmentCardEntry>> = BTreeMap::new();
    for (set_index, set) in index.equipment_sets.iter().enumerate() {
        if set.expansion != expansion {
            continue;
        }
        if set.is_set {
            if equipment_set_matches(
                &index,
                set,
                &normalized_query,
                filter,
                &job_filter,
                &slot_filter,
                level_filter,
                item_level_filter,
                obtained,
            ) {
                patches
                    .entry(set.patch.clone())
                    .or_default()
                    .push(EquipmentCardEntry::Set(set_index));
            }
        } else {
            for &item_index in &set.item_indices {
                let item = &index.catalog.items[item_index];
                if equipment_item_matches(
                    item,
                    &normalized_query,
                    filter,
                    &job_filter,
                    &slot_filter,
                    level_filter,
                    item_level_filter,
                    obtained,
                ) {
                    patches
                        .entry(set.patch.clone())
                        .or_default()
                        .push(EquipmentCardEntry::Item(item_index));
                }
            }
        }
    }
    let mut patches = patches.into_iter().collect::<Vec<_>>();
    patches.sort_by(|left, right| compare_patch_labels(&left.0, &right.0));
    let total = patches.iter().map(|(_, sets)| sets.len()).sum::<usize>();

    rsx! {
        div { class: "divide-y border-y",
            if total == 0 {
                div { class: "py-10",
                    EmptyState { title: format!("{expansion}没有匹配的装备") }
                }
            }
            for (position, (patch, entries)) in patches.into_iter().enumerate() {
                EquipmentPatchSection {
                    key: "{patch}",
                    index: index.clone(),
                    patch,
                    entries,
                    obtained,
                    on_toggle,
                    default_open: position == 0,
                }
            }
        }
    }
}

#[component]
fn EquipmentPatchSection(
    index: CollectionIndexRef,
    patch: String,
    entries: Vec<EquipmentCardEntry>,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
    default_open: bool,
) -> Element {
    let mut expanded = use_signal(|| default_open);
    let total = entries.len();
    rsx! {
        section {
            button {
                r#type: "button",
                class: "flex w-full items-center gap-2 px-1 py-3 text-left hover:bg-muted/30 sm:px-2",
                aria_expanded: expanded(),
                onclick: move |_| expanded.toggle(),
                Icon { kind: if expanded() { IconKind::ChevronDown } else { IconKind::ChevronRight }, class: "h-4 w-4 shrink-0 text-muted-foreground" }
                h2 { class: "text-sm font-semibold", "{patch}" }
                span { class: "text-xs text-muted-foreground", "{total} 项" }
            }
            if expanded() {
                div { class: "pb-6 pt-1",
                    div { class: "grid items-start gap-3 lg:grid-cols-2",
                        for entry in entries.iter().copied() {
                            match entry {
                                EquipmentCardEntry::Set(set_index) => {
                                    let set = index.equipment_sets[set_index].clone();
                                    rsx! {
                                    EquipmentSetCard {
                                        key: "{set.patch}:{set.set_id}",
                                        index: index.clone(),
                                        set,
                                        obtained,
                                        on_toggle,
                                    }
                                }
                                },
                                EquipmentCardEntry::Item(item_index) => rsx! {
                                    EquipmentStandaloneCard {
                                        key: "{index.catalog.items[item_index].id}",
                                        item: index.catalog.items[item_index].clone(),
                                        obtained,
                                        on_toggle,
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

fn compare_patch_labels(left: &str, right: &str) -> std::cmp::Ordering {
    match (numeric_patch(left), numeric_patch(right)) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

fn numeric_patch(label: &str) -> Option<Vec<u32>> {
    let parts = label
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then_some(parts)
}

fn collection_item_matches_query(item: &CollectionItem, query: &str) -> bool {
    query.is_empty()
        || item.name.to_lowercase().contains(query)
        || item.description.to_lowercase().contains(query)
        || item.class_job_category_name.to_lowercase().contains(query)
        || item.id.to_string().contains(query)
}

fn equipment_set_matches(
    index: &CollectionIndex,
    set: &EquipmentSetGroup,
    query: &str,
    filter: ObtainedFilter,
    job_filter: &str,
    slot_filter: &HashSet<String>,
    level_filter: NumericRangeFilter,
    item_level_filter: NumericRangeFilter,
    obtained: ObtainedStore,
) -> bool {
    let query = query.trim();
    if !query.is_empty()
        && !set.search_text.contains(query)
        && !set.item_indices.iter().any(|&item_index| {
            collection_item_matches_query(&index.catalog.items[item_index], query)
        })
    {
        return false;
    }
    set.item_indices.iter().any(|&item_index| {
        let item = &index.catalog.items[item_index];
        let job_matches = equipment_job_filter_matches(job_filter, &item.class_job_category_name);
        let slot_matches = selected_filter_matches(slot_filter, &item.slot_name);
        job_matches
            && slot_matches
            && level_filter.matches(item.level_equip)
            && item_level_filter.matches(item.level_item)
            && (filter == ObtainedFilter::All
                || obtained_filter_matches(filter, is_obtained(obtained, item.id)))
    })
}

fn equipment_item_matches(
    item: &CollectionItem,
    query: &str,
    filter: ObtainedFilter,
    job_filter: &str,
    slot_filter: &HashSet<String>,
    level_filter: NumericRangeFilter,
    item_level_filter: NumericRangeFilter,
    obtained: ObtainedStore,
) -> bool {
    collection_item_matches_query(item, query)
        && equipment_job_filter_matches(job_filter, &item.class_job_category_name)
        && selected_filter_matches(slot_filter, &item.slot_name)
        && level_filter.matches(item.level_equip)
        && item_level_filter.matches(item.level_item)
        && (filter == ObtainedFilter::All
            || obtained_filter_matches(filter, is_obtained(obtained, item.id)))
}

fn selected_filter_matches(selected: &HashSet<String>, value: &str) -> bool {
    selected.is_empty() || selected.contains(value)
}

fn equipment_job_filter_matches(selected: &str, raw_category: &str) -> bool {
    if selected.is_empty() || raw_category == "所有职业" {
        return true;
    }
    if matches!(selected, "战斗精英" | "能工巧匠" | "大地使者") {
        return equipment_job_group_matches(selected, raw_category);
    }
    BATTLE_JOB_OPTIONS
        .iter()
        .chain(CRAFTER_JOB_OPTIONS)
        .chain(GATHERER_JOB_OPTIONS)
        .find(|option| option.label == selected)
        .is_some_and(|option| {
            option
                .aliases
                .iter()
                .any(|alias| raw_category.contains(alias))
        })
}

fn equipment_job_group_matches(group: &str, raw_category: &str) -> bool {
    if raw_category == "所有职业" {
        return true;
    }
    let terms = match group {
        "战斗精英" => BATTLE_JOB_TERMS,
        "能工巧匠" => CRAFTER_JOB_TERMS,
        "大地使者" => GATHERER_JOB_TERMS,
        _ => return false,
    };
    terms.iter().any(|term| raw_category.contains(term))
}

#[component]
fn EquipmentSetCard(
    index: CollectionIndexRef,
    set: EquipmentSetGroup,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let obtained_count = set
        .item_indices
        .iter()
        .filter(|&&item_index| is_obtained(obtained, index.catalog.items[item_index].id))
        .count();
    let set_complete = obtained_count == set.item_indices.len();
    let level_item_label = level_range_label("品级", set.min_item_level, set.max_item_level);
    let level_equip_label = level_range_label("装备等级", set.min_equip_level, set.max_equip_level);
    let representative = index.catalog.items[set.item_indices[0]].clone();
    let wiki_href = item_wiki_href(&set.set_name);
    rsx! {
        article { class: if set_complete { "overflow-hidden rounded-lg border border-emerald-200 bg-emerald-50/80 lg:col-span-2" } else { "overflow-hidden rounded-lg border bg-muted/10 lg:col-span-2" },
            header { class: if set_complete { "flex items-start gap-3 border-b border-emerald-200 bg-emerald-50 px-3 py-3" } else { "flex items-start gap-3 border-b bg-muted/25 px-3 py-3" },
                ItemIcon { icon: representative.icon, size: "lg" }
                div { class: "min-w-0 flex-1",
                    div { class: "flex min-w-0 items-center gap-2",
                        h3 { class: "min-w-0 break-words text-sm font-semibold", "{set.set_name}" }
                        Badge { variant: if set.is_set { BadgeVariant::Outline } else { BadgeVariant::Secondary },
                            if set.is_set { "{set.slot_count} 部位" } else { "单件" }
                        }
                    }
                    div { class: "mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground",
                        if !level_equip_label.is_empty() { span { "{level_equip_label}" } }
                        if !level_item_label.is_empty() { span { "{level_item_label}" } }
                        if !set.class_job_label.is_empty() { span { class: "max-w-full break-words", "{set.class_job_label}" } }
                    }
                }
                Badge { variant: if set_complete { BadgeVariant::Success } else { BadgeVariant::Secondary },
                    "{obtained_count}/{set.item_indices.len()}"
                }
                a {
                    href: wiki_href,
                    target: "_blank",
                    rel: "noreferrer",
                    title: "打开套装物品的灰机 Wiki",
                    class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                    Icon { kind: IconKind::ExternalLink, class: "h-4 w-4" }
                }
            }
            div { class: "grid grid-cols-1 gap-3 p-3 sm:grid-cols-2 xl:grid-cols-4",
                for item_index in set.item_indices {
                    EquipmentPiece {
                        key: "{index.catalog.items[item_index].id}",
                        item: index.catalog.items[item_index].clone(),
                        obtained,
                        on_toggle,
                    }
                }
            }
        }
    }
}

#[component]
fn EquipmentStandaloneCard(
    item: CollectionItem,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let item_id = item.id;
    let item_obtained = is_obtained(obtained, item_id);
    let wiki_href = item_wiki_href(&item.name);
    rsx! {
        article { class: if item_obtained { "flex min-h-20 items-center gap-3 rounded-lg border border-emerald-200 bg-emerald-50/80 p-3 shadow-sm transition-colors" } else { "flex min-h-20 items-center gap-3 rounded-lg border bg-card p-3 shadow-sm transition-colors hover:border-foreground/20" },
            label { class: "flex min-w-0 flex-1 cursor-pointer items-center gap-2",
                input {
                    r#type: "checkbox",
                    checked: item_obtained,
                    onchange: move |_| on_toggle.call(item_id),
                }
                ItemIcon { icon: item.icon, size: "lg" }
                div { class: "min-w-0 flex-1",
                    h3 { class: "break-words text-sm font-semibold", "{item.name}" }
                    div { class: "mt-1 flex flex-wrap gap-x-2 gap-y-1 text-xs text-muted-foreground",
                        span { class: "tabular-nums", "ID {item.id}" }
                        if !item.slot_name.is_empty() { span { "{item.slot_name}" } }
                        if item.level_equip > 0 { span { "等级 {item.level_equip}" } }
                        if item.level_item > 0 { span { "品级 {item.level_item}" } }
                        if !item.class_job_category_name.is_empty() { span { "{item.class_job_category_name}" } }
                    }
                }
            }
            a {
                href: wiki_href,
                target: "_blank",
                rel: "noreferrer",
                title: "打开灰机 Wiki",
                class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                Icon { kind: IconKind::ExternalLink, class: "h-4 w-4" }
            }
        }
    }
}

#[component]
fn EquipmentPiece(
    item: CollectionItem,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let item_id = item.id;
    let item_obtained = is_obtained(obtained, item_id);
    let wiki_href = item_wiki_href(&item.name);
    rsx! {
        article { class: if item_obtained { "flex min-h-20 items-center gap-3 rounded-md border border-emerald-200 bg-emerald-50/80 p-3 transition-colors" } else { "flex min-h-20 items-center gap-3 rounded-md border bg-card p-3 transition-colors hover:border-foreground/20 hover:bg-muted/30" },
            label { class: "flex min-w-0 flex-1 cursor-pointer items-center gap-2",
                input {
                    r#type: "checkbox",
                    checked: item_obtained,
                    onchange: move |_| on_toggle.call(item_id),
                }
                ItemIcon { icon: item.icon, size: "lg" }
                div { class: "min-w-0 flex-1",
                    div { class: "break-words text-sm font-medium", "{item.name}" }
                    div { class: "mt-1 flex flex-wrap gap-x-2 text-xs text-muted-foreground",
                        span { class: "tabular-nums", "ID {item.id}" }
                        span { "{item.slot_name}" }
                        if item.level_equip > 0 { span { "等级 {item.level_equip}" } }
                        if item.level_item > 0 { span { "品级 {item.level_item}" } }
                    }
                }
            }
            a {
                href: wiki_href,
                target: "_blank",
                rel: "noreferrer",
                title: "打开灰机 Wiki",
                class: "flex h-7 w-7 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                Icon { kind: IconKind::ExternalLink, class: "h-3.5 w-3.5" }
            }
        }
    }
}

fn level_range_label(prefix: &str, min: u16, max: u16) -> String {
    match (min, max) {
        (0, 0) => String::new(),
        (min, max) if min == max => format!("{prefix} {min}"),
        (min, max) => format!("{prefix} {min}-{max}"),
    }
}

fn item_wiki_href(name: &str) -> String {
    format!(
        "https://ff14.huijiwiki.com/wiki/%E7%89%A9%E5%93%81:{}",
        urlencoding::encode(name)
    )
}

#[component]
fn FlatCollectionView(
    index: CollectionIndexRef,
    kind: CollectionKind,
    expansion: String,
    query: String,
    filter: ObtainedFilter,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let query = query.trim().to_lowercase();
    let items = index
        .items_for_kind_expansion(kind, &expansion)
        .filter(|item| {
            collection_item_matches_query(item, &query)
                && (filter == ObtainedFilter::All
                    || obtained_filter_matches(filter, is_obtained(obtained, item.id)))
        })
        .cloned()
        .collect::<Vec<_>>();
    let total = items.len();
    let mut patches = BTreeMap::<String, Vec<CollectionItem>>::new();
    if category_definition(kind).group_by_patch {
        for item in items.iter().cloned() {
            patches.entry(item.patch.clone()).or_default().push(item);
        }
    }
    let mut patches = patches.into_iter().collect::<Vec<_>>();
    patches.sort_by(|left, right| compare_patch_labels(&left.0, &right.0));
    rsx! {
        div {
            if category_definition(kind).group_by_patch {
                div { class: "divide-y border-y",
                    for (patch, patch_items) in patches {
                        section { key: "{patch}", class: "py-4 first:pt-0",
                            header { class: "mb-3 flex items-baseline gap-2 px-1",
                                h2 { class: "text-sm font-semibold", "{patch}" }
                                span { class: "text-xs text-muted-foreground", "{patch_items.len()} 项" }
                            }
                            div { class: "grid gap-2 md:grid-cols-2 xl:grid-cols-3",
                                for item in patch_items {
                                    CollectionItemLine {
                                        key: "{item.id}",
                                        obtained,
                                        item,
                                        on_toggle,
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "grid gap-2 md:grid-cols-2 xl:grid-cols-3",
                    for item in items {
                        CollectionItemLine {
                            key: "{item.id}",
                            obtained,
                            item,
                            on_toggle,
                        }
                    }
                }
            }
            if total == 0 { EmptyState { title: format!("没有匹配的{}", kind.label()) } }
        }
    }
}

#[component]
fn CollectionItemLine(
    item: CollectionItem,
    obtained: ObtainedStore,
    on_toggle: EventHandler<u32>,
) -> Element {
    let item_id = item.id;
    let item_obtained = is_obtained(obtained, item_id);
    let wiki_href = item_wiki_href(&item.name);
    rsx! {
        article { class: if item_obtained { "flex min-h-16 items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50/80 p-2 transition-colors" } else { "flex min-h-16 items-center gap-2 rounded-md border bg-card p-2 transition-colors" },
            label { class: "flex min-w-0 flex-1 cursor-pointer items-center gap-2",
                input {
                    r#type: "checkbox",
                    checked: item_obtained,
                    onchange: move |_| on_toggle.call(item_id),
                }
                ItemIcon { icon: item.icon, size: "sm" }
                div { class: "min-w-0 flex-1",
                    div { class: "break-words text-sm font-medium", "{item.name}" }
                    div { class: "mt-1 flex flex-wrap gap-x-2 text-xs text-muted-foreground",
                        span { class: "tabular-nums", "ID {item.id}" }
                        if !item.patch.is_empty() { span { "{item.patch}" } }
                        if item.level_item > 1 { span { "品级 {item.level_item}" } }
                    }
                }
            }
            a {
                href: wiki_href,
                target: "_blank",
                rel: "noreferrer",
                title: "打开灰机 Wiki",
                class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground",
                Icon { kind: IconKind::ExternalLink, class: "h-4 w-4" }
            }
        }
    }
}

fn obtained_filter_matches(filter: ObtainedFilter, obtained: bool) -> bool {
    match filter {
        ObtainedFilter::All => true,
        ObtainedFilter::Missing => !obtained,
        ObtainedFilter::Obtained => obtained,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        NumericRangeFilter, equipment_job_filter_matches, item_wiki_href, parse_optional_u16,
        selected_filter_matches,
    };

    #[test]
    fn item_wiki_links_use_the_item_namespace() {
        assert_eq!(
            item_wiki_href("前魔导咏咒宽边帽"),
            "https://ff14.huijiwiki.com/wiki/%E7%89%A9%E5%93%81:%E5%89%8D%E9%AD%94%E5%AF%BC%E5%92%8F%E5%92%92%E5%AE%BD%E8%BE%B9%E5%B8%BD"
        );
        assert!(item_wiki_href("前魔导咏咒套装").contains("%E5%A5%97%E8%A3%85"));
    }

    #[test]
    fn empty_multi_select_matches_all_and_selected_values_use_or_semantics() {
        assert!(selected_filter_matches(&HashSet::new(), "头部"));

        let selected = HashSet::from(["头部".to_string(), "身体".to_string()]);
        assert!(selected_filter_matches(&selected, "头部"));
        assert!(selected_filter_matches(&selected, "身体"));
        assert!(!selected_filter_matches(&selected, "手部"));
    }

    #[test]
    fn equipment_job_filters_use_display_groups() {
        assert!(equipment_job_filter_matches(
            "战斗精英",
            "剑术师 斧术师 骑士 战士"
        ));
        assert!(equipment_job_filter_matches("骑士", "剑术师 骑士"));
        assert!(equipment_job_filter_matches("骑士", "战斗精英"));
        assert!(equipment_job_filter_matches("白魔法师", "魔法导师"));
        assert!(equipment_job_filter_matches("锻铁匠", "能工巧匠"));
        assert!(equipment_job_filter_matches("骑士", "所有职业"));
        assert!(!equipment_job_filter_matches("骑士", "能工巧匠"));
    }

    #[test]
    fn numeric_range_filters_are_inclusive_and_allow_open_ends() {
        assert!(NumericRangeFilter::default().matches(50));
        assert!(
            NumericRangeFilter {
                min: Some(50),
                max: Some(100),
            }
            .matches(50)
        );
        assert!(
            NumericRangeFilter {
                min: Some(50),
                max: Some(100),
            }
            .matches(100)
        );
        assert!(
            !NumericRangeFilter {
                min: Some(51),
                max: None,
            }
            .matches(50)
        );
        assert!(
            !NumericRangeFilter {
                min: None,
                max: Some(49),
            }
            .matches(50)
        );
    }

    #[test]
    fn optional_range_values_accept_empty_and_valid_u16_inputs() {
        assert_eq!(parse_optional_u16(""), None);
        assert_eq!(parse_optional_u16(" 100 "), Some(100));
        assert_eq!(parse_optional_u16("invalid"), None);
        assert_eq!(parse_optional_u16("70000"), None);
    }
}
