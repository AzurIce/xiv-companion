use std::collections::{BTreeMap, HashSet};

use dioxus::prelude::*;
use xiv_companion::{CollectionEntryKey, CollectionItem, CollectionKind, ResourceOrigin};

use crate::app::collection_index::{CollectionIndex, EquipmentSetGroup};
use crate::app::data::load_collection_catalog_with_metadata;
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::format_integer;

use super::crafting::ItemIcon;

const PAGE_SIZE: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquipmentGrouping {
    VersionAndSet,
    Appearance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObtainedFilter {
    All,
    Missing,
    Obtained,
}

#[derive(Clone, Debug, Default)]
struct LoadedObtainedState {
    obtained: HashSet<CollectionEntryKey>,
    legacy_item_ids: HashSet<u32>,
}

async fn load_obtained_state() -> LoadedObtainedState {
    #[cfg(target_arch = "wasm32")]
    {
        let state = crate::app::collection_state::load_collection_state()
            .await
            .unwrap_or_default();
        return LoadedObtainedState {
            obtained: state.obtained,
            legacy_item_ids: state.legacy_item_ids,
        };
    }
    #[cfg(not(target_arch = "wasm32"))]
    LoadedObtainedState::default()
}

async fn persist_obtained_state(obtained: HashSet<CollectionEntryKey>) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let state = crate::app::collection_state::CollectionState {
            obtained,
            legacy_item_ids: HashSet::new(),
            updated_at: js_sys::Date::new_0()
                .to_iso_string()
                .as_string()
                .unwrap_or_default(),
        };
        return crate::app::collection_state::save_collection_state(&state).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = obtained;
        Ok(())
    }
}

#[component]
pub fn CollectionPage() -> Element {
    let catalog = use_resource(load_collection_catalog_with_metadata);
    let state = use_resource(load_obtained_state);
    let collection_index = use_memo(move || {
        catalog
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|loaded| CollectionIndex::new(loaded.data.clone()))
    });
    let mut active_kind = use_signal(|| CollectionKind::Equipment);
    let mut equipment_grouping = use_signal(|| EquipmentGrouping::VersionAndSet);
    let mut obtained_filter = use_signal(|| ObtainedFilter::All);
    let mut equipment_job_filter = use_signal(String::new);
    let mut equipment_slot_filter = use_signal(String::new);
    let mut query = use_signal(String::new);
    let mut visible_pages = use_signal(|| 1usize);
    let mut obtained = use_signal(HashSet::<CollectionEntryKey>::new);
    let mut hydrated = use_signal(|| false);

    use_effect(move || {
        if hydrated() {
            return;
        }
        let state_read = state.read();
        let Some(loaded_state) = state_read.as_ref() else {
            return;
        };
        let catalog_read = catalog.read();
        let Some(Ok(loaded_catalog)) = catalog_read.as_ref() else {
            return;
        };
        let mut next = loaded_state.obtained.clone();
        for legacy_id in &loaded_state.legacy_item_ids {
            if let Some(item) = loaded_catalog
                .data
                .items
                .iter()
                .find(|item| item.id == *legacy_id)
            {
                next.insert(item.key());
            }
        }
        obtained.set(next.clone());
        hydrated.set(true);
        if !loaded_state.legacy_item_ids.is_empty() {
            spawn(async move {
                let _ = persist_obtained_state(next).await;
            });
        }
    });

    let catalog_snapshot = catalog.read().as_ref().cloned();
    let index_snapshot = collection_index.read().clone();
    let query_snapshot = query();
    let obtained_snapshot = obtained();
    let kind_snapshot = active_kind();
    let grouping_snapshot = equipment_grouping();
    let obtained_filter_snapshot = obtained_filter();
    let equipment_job_snapshot = equipment_job_filter();
    let equipment_slot_snapshot = equipment_slot_filter();
    let visible_limit = visible_pages() * PAGE_SIZE;

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-3 sm:px-6 lg:px-8",
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    div { class: "min-w-0",
                        div { class: "text-sm text-muted-foreground", "数据" }
                        h1 { class: "text-2xl font-semibold", "图鉴" }
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
                    let job_options = equipment_filter_options(&index, |item| &item.class_job_category_name);
                    let slot_options = equipment_filter_options(&index, |item| &item.slot_name);
                    rsx! {
                        div { class: "flex min-h-0 flex-1 flex-col overflow-hidden",
                            div { class: "border-b px-4 sm:px-6 lg:px-8",
                                div { class: "flex gap-1 overflow-x-auto py-2",
                                    for kind in CollectionKind::ALL {
                                        CollectionKindTab {
                                            kind,
                                            count: loaded.data.counts.count_for(kind),
                                            active: kind == kind_snapshot,
                                            onclick: move |_| {
                                                active_kind.set(kind);
                                                query.set(String::new());
                                                obtained_filter.set(ObtainedFilter::All);
                                                equipment_job_filter.set(String::new());
                                                equipment_slot_filter.set(String::new());
                                                visible_pages.set(1);
                                            },
                                        }
                                    }
                                }
                            }

                            div { class: "flex flex-wrap items-center gap-2 border-b px-4 py-3 sm:px-6 lg:px-8",
                                div { class: "relative min-w-56 flex-1 lg:max-w-sm",
                                    Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" }
                                    input {
                                        r#type: "search",
                                        placeholder: "搜索名称或套装",
                                        value: "{query_snapshot}",
                                        class: input_class("pl-9"),
                                        oninput: move |event| {
                                            query.set(event.value());
                                            visible_pages.set(1);
                                        },
                                    }
                                }
                                select {
                                    class: input_class("w-auto min-w-28"),
                                    value: match obtained_filter_snapshot { ObtainedFilter::All => "all", ObtainedFilter::Missing => "missing", ObtainedFilter::Obtained => "obtained" },
                                    onchange: move |event| {
                                        obtained_filter.set(match event.value().as_str() {
                                            "missing" => ObtainedFilter::Missing,
                                            "obtained" => ObtainedFilter::Obtained,
                                            _ => ObtainedFilter::All,
                                        });
                                        visible_pages.set(1);
                                    },
                                    option { value: "all", "全部状态" }
                                    option { value: "missing", "未获得" }
                                    option { value: "obtained", "已获得" }
                                }
                                if kind_snapshot == CollectionKind::Equipment {
                                    select {
                                        class: input_class("w-auto min-w-32"),
                                        value: "{equipment_job_snapshot}",
                                        onchange: move |event| { equipment_job_filter.set(event.value()); visible_pages.set(1); },
                                        option { value: "", "全部职业" }
                                        for option in job_options { option { value: "{option}", "{option}" } }
                                    }
                                    select {
                                        class: input_class("w-auto min-w-28"),
                                        value: "{equipment_slot_snapshot}",
                                        onchange: move |event| { equipment_slot_filter.set(event.value()); visible_pages.set(1); },
                                        option { value: "", "全部部位" }
                                        for option in slot_options { option { value: "{option}", "{option}" } }
                                    }
                                    div { class: "flex h-9 items-center rounded-md border bg-muted/30 p-1",
                                        GroupingButton {
                                            label: "版本·套装",
                                            active: grouping_snapshot == EquipmentGrouping::VersionAndSet,
                                            onclick: move |_| { equipment_grouping.set(EquipmentGrouping::VersionAndSet); visible_pages.set(1); },
                                        }
                                        GroupingButton {
                                            label: "同模",
                                            active: grouping_snapshot == EquipmentGrouping::Appearance,
                                            onclick: move |_| { equipment_grouping.set(EquipmentGrouping::Appearance); visible_pages.set(1); },
                                        }
                                    }
                                }
                            }

                            div { class: "min-h-0 flex-1 overflow-y-auto px-4 py-4 sm:px-6 lg:px-8",
                                if kind_snapshot == CollectionKind::Equipment {
                                    EquipmentCollectionView {
                                        index: index.clone(),
                                        grouping: grouping_snapshot,
                                        query: query_snapshot.clone(),
                                        filter: obtained_filter_snapshot,
                                        job_filter: equipment_job_snapshot,
                                        slot_filter: equipment_slot_snapshot,
                                        obtained: obtained_snapshot.clone(),
                                        visible_limit,
                                        on_toggle: move |key| toggle_obtained(key, obtained, &obtained_snapshot),
                                        on_load_more: move |_| visible_pages.set(visible_pages() + 1),
                                    }
                                } else {
                                    FlatCollectionView {
                                        index,
                                        kind: kind_snapshot,
                                        query: query_snapshot.clone(),
                                        filter: obtained_filter_snapshot,
                                        obtained: obtained_snapshot.clone(),
                                        visible_limit,
                                        on_toggle: move |key| toggle_obtained(key, obtained, &obtained_snapshot),
                                        on_load_more: move |_| visible_pages.set(visible_pages() + 1),
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

fn toggle_obtained(
    key: CollectionEntryKey,
    mut signal: Signal<HashSet<CollectionEntryKey>>,
    snapshot: &HashSet<CollectionEntryKey>,
) {
    let mut next = snapshot.clone();
    if !next.remove(&key) {
        next.insert(key);
    }
    signal.set(next.clone());
    spawn(async move {
        if let Err(error) = persist_obtained_state(next).await {
            crate::app::log::warn("collection", format!("保存图鉴状态失败: {error}"));
        }
    });
}

fn origin_label(origin: Option<ResourceOrigin>) -> &'static str {
    match origin {
        Some(ResourceOrigin::UserLocal) => "本地数据",
        Some(ResourceOrigin::Network) => "网络数据",
        _ => "内置数据",
    }
}

#[component]
fn CollectionKindTab(
    kind: CollectionKind,
    count: usize,
    active: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if active {
                "shrink-0 border-b-2 border-primary px-3 py-2 text-sm font-medium text-foreground"
            } else {
                "shrink-0 border-b-2 border-transparent px-3 py-2 text-sm text-muted-foreground hover:text-foreground"
            },
            onclick: move |event| onclick.call(event),
            "{kind.label()}"
            span { class: "ml-1 text-xs text-muted-foreground", "{count}" }
        }
    }
}

#[component]
fn GroupingButton(label: &'static str, active: bool, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if active { "h-7 rounded bg-background px-3 text-xs font-medium shadow" } else { "h-7 rounded px-3 text-xs text-muted-foreground" },
            onclick: move |event| onclick.call(event),
            "{label}"
        }
    }
}

#[component]
fn EquipmentCollectionView(
    index: CollectionIndex,
    grouping: EquipmentGrouping,
    query: String,
    filter: ObtainedFilter,
    job_filter: String,
    slot_filter: String,
    obtained: HashSet<CollectionEntryKey>,
    visible_limit: usize,
    on_toggle: EventHandler<CollectionEntryKey>,
    on_load_more: EventHandler<MouseEvent>,
) -> Element {
    if grouping == EquipmentGrouping::Appearance {
        return rsx! {
            AppearanceGroups {
                index,
                query,
                filter,
                job_filter,
                slot_filter,
                obtained,
                visible_limit,
                on_toggle,
                on_load_more,
            }
        };
    }

    let matching = index
        .equipment_sets
        .iter()
        .filter(|set| {
            equipment_set_matches(
                &index,
                set,
                &query,
                filter,
                &job_filter,
                &slot_filter,
                &obtained,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let total = matching.len();
    let tree = equipment_tree(matching.into_iter().take(visible_limit));

    rsx! {
        div { class: "space-y-4",
            if total == 0 {
                EmptyState { title: "没有匹配的装备套装".to_string() }
            }
            for (expansion, patches) in tree {
                details { class: "border-b pb-4", open: true,
                    summary { class: "cursor-pointer select-none py-2 text-base font-semibold", "{expansion}" }
                    div { class: "space-y-4 pt-2",
                        for (patch, sets) in patches {
                            details { open: true,
                                summary { class: "cursor-pointer select-none py-2 text-sm font-medium text-muted-foreground", "{patch} · {sets.len()} 套" }
                                div { class: "grid gap-3 pt-2 md:grid-cols-2 xl:grid-cols-3",
                                    for set in sets {
                                        EquipmentSetCard {
                                            index: index.clone(),
                                            set,
                                            obtained: obtained.clone(),
                                            on_toggle,
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if total > visible_limit {
                LoadMoreButton { remaining: total - visible_limit, onclick: on_load_more }
            }
        }
    }
}

fn equipment_tree(
    sets: impl Iterator<Item = EquipmentSetGroup>,
) -> Vec<(String, Vec<(String, Vec<EquipmentSetGroup>)>)> {
    let mut tree: BTreeMap<String, BTreeMap<String, Vec<EquipmentSetGroup>>> = BTreeMap::new();
    for set in sets {
        tree.entry(set.expansion.clone())
            .or_default()
            .entry(set.patch.clone())
            .or_default()
            .push(set);
    }
    let mut expansions = tree.into_iter().collect::<Vec<_>>();
    expansions.sort_by_key(|(name, _)| std::cmp::Reverse(expansion_order(name)));
    expansions
        .into_iter()
        .map(|(expansion, patches)| {
            let mut patches = patches.into_iter().collect::<Vec<_>>();
            patches.sort_by(|left, right| right.0.cmp(&left.0));
            (expansion, patches)
        })
        .collect()
}

fn expansion_order(name: &str) -> u8 {
    match name {
        "重生之境" => 1,
        "苍穹之禁城" => 2,
        "红莲之狂潮" => 3,
        "暗影之逆焰" => 4,
        "晓月之终途" => 5,
        "金曦之遗辉" => 6,
        "本地版本" => 7,
        _ => 0,
    }
}

fn equipment_set_matches(
    index: &CollectionIndex,
    set: &EquipmentSetGroup,
    query: &str,
    filter: ObtainedFilter,
    job_filter: &str,
    slot_filter: &str,
    obtained: &HashSet<CollectionEntryKey>,
) -> bool {
    let query = query.trim().to_lowercase();
    set.item_indices.iter().any(|&item_index| {
        let item = &index.catalog.items[item_index];
        let text_matches = query.is_empty()
            || set.set_name.to_lowercase().contains(&query)
            || item.name.to_lowercase().contains(&query)
            || item.class_job_category_name.to_lowercase().contains(&query);
        let job_matches = job_filter.is_empty() || item.class_job_category_name == job_filter;
        let slot_matches = slot_filter.is_empty() || item.slot_name == slot_filter;
        text_matches
            && job_matches
            && slot_matches
            && obtained_filter_matches(filter, obtained.contains(&item.key()))
    })
}

#[component]
fn EquipmentSetCard(
    index: CollectionIndex,
    set: EquipmentSetGroup,
    obtained: HashSet<CollectionEntryKey>,
    on_toggle: EventHandler<CollectionEntryKey>,
) -> Element {
    let obtained_count = set
        .item_indices
        .iter()
        .filter(|&&item_index| obtained.contains(&index.catalog.items[item_index].key()))
        .count();
    rsx! {
        article { class: "overflow-hidden rounded-lg border bg-card",
            div { class: "flex items-start justify-between gap-3 border-b px-3 py-3",
                div { class: "min-w-0",
                    h3 { class: "truncate text-sm font-semibold", "{set.set_name}" }
                    div { class: "mt-1 flex flex-wrap gap-x-3 text-xs text-muted-foreground",
                        if set.max_item_level > 0 { span { "品级 {set.max_item_level}" } }
                        if !set.class_job_label.is_empty() { span { "{set.class_job_label}" } }
                    }
                }
                Badge { variant: if obtained_count == set.item_indices.len() { BadgeVariant::Success } else { BadgeVariant::Secondary },
                    "{obtained_count}/{set.item_indices.len()}"
                }
            }
            div { class: "grid grid-cols-2 divide-x divide-y sm:grid-cols-3",
                for item_index in set.item_indices {
                    EquipmentPiece {
                        item: index.catalog.items[item_index].clone(),
                        sibling_obtained: index.has_obtained_sibling_model(&index.catalog.items[item_index], &obtained),
                        obtained: obtained.contains(&index.catalog.items[item_index].key()),
                        on_toggle,
                    }
                }
            }
        }
    }
}

#[component]
fn EquipmentPiece(
    item: CollectionItem,
    obtained: bool,
    sibling_obtained: bool,
    on_toggle: EventHandler<CollectionEntryKey>,
) -> Element {
    let key = item.key();
    let wiki_href = format!(
        "https://ff14.huijiwiki.com/wiki/{}",
        urlencoding::encode(&item.name)
    );
    rsx! {
        div { class: if sibling_obtained && !obtained { "flex min-h-16 items-center gap-1 bg-yellow-500/10 p-2" } else { "flex min-h-16 items-center gap-1 p-2 hover:bg-muted/50" },
            label { class: "flex min-w-0 flex-1 cursor-pointer items-center gap-2",
                input {
                    r#type: "checkbox",
                    checked: obtained,
                    onchange: move |_| on_toggle.call(key.clone()),
                }
                ItemIcon { icon: item.icon, size: "sm" }
                div { class: "min-w-0 flex-1",
                    div { class: "truncate text-xs font-medium", "{item.name}" }
                    div { class: "mt-0.5 text-[11px] text-muted-foreground", "{item.slot_name}" }
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

#[component]
fn AppearanceGroups(
    index: CollectionIndex,
    query: String,
    filter: ObtainedFilter,
    job_filter: String,
    slot_filter: String,
    obtained: HashSet<CollectionEntryKey>,
    visible_limit: usize,
    on_toggle: EventHandler<CollectionEntryKey>,
    on_load_more: EventHandler<MouseEvent>,
) -> Element {
    let query = query.trim().to_lowercase();
    let mut groups = index
        .items_by_appearance
        .iter()
        .filter_map(|(key, indices)| {
            let matches = indices.iter().any(|&item_index| {
                let item = &index.catalog.items[item_index];
                (query.is_empty() || item.name.to_lowercase().contains(&query))
                    && (job_filter.is_empty() || item.class_job_category_name == job_filter)
                    && (slot_filter.is_empty() || item.slot_name == slot_filter)
                    && obtained_filter_matches(filter, obtained.contains(&item.key()))
            });
            matches.then(|| (key.clone(), indices.clone()))
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0));
    let total = groups.len();
    rsx! {
        div { class: "space-y-4",
            div { class: "grid gap-3 md:grid-cols-2 xl:grid-cols-3",
                for (appearance_key, indices) in groups.into_iter().take(visible_limit) {
                    article { class: "overflow-hidden rounded-lg border bg-card",
                        div { class: "border-b px-3 py-2 text-xs font-medium text-muted-foreground", "{appearance_key}" }
                        div { class: "divide-y",
                            for item_index in indices {
                                CollectionItemLine {
                                    item: index.catalog.items[item_index].clone(),
                                    obtained: obtained.contains(&index.catalog.items[item_index].key()),
                                    sibling_obtained: index.has_obtained_sibling_model(&index.catalog.items[item_index], &obtained),
                                    on_toggle,
                                }
                            }
                        }
                    }
                }
            }
            if total == 0 { EmptyState { title: "没有匹配的同模装备".to_string() } }
            if total > visible_limit { LoadMoreButton { remaining: total - visible_limit, onclick: on_load_more } }
        }
    }
}

fn equipment_filter_options(
    index: &CollectionIndex,
    value: impl Fn(&CollectionItem) -> &str,
) -> Vec<String> {
    let mut values = index
        .items_for_kind(CollectionKind::Equipment)
        .map(value)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[component]
fn FlatCollectionView(
    index: CollectionIndex,
    kind: CollectionKind,
    query: String,
    filter: ObtainedFilter,
    obtained: HashSet<CollectionEntryKey>,
    visible_limit: usize,
    on_toggle: EventHandler<CollectionEntryKey>,
    on_load_more: EventHandler<MouseEvent>,
) -> Element {
    let query = query.trim().to_lowercase();
    let items = index
        .items_for_kind(kind)
        .filter(|item| {
            (query.is_empty() || item.name.to_lowercase().contains(&query))
                && obtained_filter_matches(filter, obtained.contains(&item.key()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let total = items.len();
    rsx! {
        div { class: "space-y-4",
            div { class: "grid gap-2 md:grid-cols-2 xl:grid-cols-3",
                for item in items.into_iter().take(visible_limit) {
                    CollectionItemLine {
                        obtained: obtained.contains(&item.key()),
                        sibling_obtained: false,
                        item,
                        on_toggle,
                    }
                }
            }
            if total == 0 { EmptyState { title: format!("没有匹配的{}", kind.label()) } }
            if total > visible_limit { LoadMoreButton { remaining: total - visible_limit, onclick: on_load_more } }
        }
    }
}

#[component]
fn CollectionItemLine(
    item: CollectionItem,
    obtained: bool,
    sibling_obtained: bool,
    on_toggle: EventHandler<CollectionEntryKey>,
) -> Element {
    let key = item.key();
    let wiki_href = format!(
        "https://ff14.huijiwiki.com/wiki/{}",
        urlencoding::encode(&item.name)
    );
    rsx! {
        article { class: if sibling_obtained && !obtained { "flex min-h-18 items-center gap-3 rounded-lg border border-yellow-500/40 bg-yellow-500/10 p-3" } else { "flex min-h-18 items-center gap-3 rounded-lg border bg-card p-3" },
            label { class: "flex min-w-0 flex-1 cursor-pointer items-center gap-3",
                input {
                    r#type: "checkbox",
                    checked: obtained,
                    onchange: move |_| on_toggle.call(key.clone()),
                }
                ItemIcon { icon: item.icon, size: "sm" }
                div { class: "min-w-0 flex-1",
                    div { class: "truncate text-sm font-medium", "{item.name}" }
                    div { class: "mt-1 flex flex-wrap gap-x-2 text-xs text-muted-foreground",
                        if !item.patch.is_empty() { span { "{item.patch}" } }
                        if item.level_item > 1 { span { "品级 {item.level_item}" } }
                        if sibling_obtained { span { class: "text-yellow-600 dark:text-yellow-400", "同模已获得" } }
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

#[component]
fn LoadMoreButton(remaining: usize, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        div { class: "flex justify-center py-2",
            Button { variant: ButtonVariant::Outline, onclick: move |event| onclick.call(event),
                "继续加载 {remaining.min(PAGE_SIZE)} 项"
            }
        }
    }
}
