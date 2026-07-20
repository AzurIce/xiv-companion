use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;
use xiv_companion::builtin_icon_urls;
use xiv_companion::inventory_bridge_protocol::{
    InventoryContainerAvailability, InventoryContainerDescriptor, InventoryContainerDirectory,
    InventoryContainerKind, InventoryContainerSnapshot, InventoryItemEntry, should_apply_container,
    should_apply_directory,
};

use crate::app::collection_bridge::load_verified_bridge_url;
use crate::app::data::load_craft_data;
use crate::app::icons::{Icon, IconKind};
use crate::app::inventory_bridge::{InventoryBridgeConnection, InventoryBridgeUpdate};
#[cfg(target_arch = "wasm32")]
use crate::app::inventory_state::{
    PersistedInventoryState, load_inventory_state, save_inventory_state,
};
use crate::app::ui::{Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, input_class};
use crate::app::utils::format_integer;

#[derive(Clone, Debug, PartialEq, Eq)]
enum InventoryConnectionState {
    Connecting,
    Connected,
    Unconfigured,
    WaitingForLogin,
    Disconnected,
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InventorySearchMatches {
    active: bool,
    item_ids: HashSet<u32>,
    container_ids: HashSet<String>,
    groups: HashSet<ContainerGroup>,
}

impl InventorySearchMatches {
    fn item_matches(&self, item: &InventoryItemEntry) -> bool {
        !self.active || self.item_ids.contains(&item.item_id)
    }

    fn container_matches(&self, container_id: &str) -> bool {
        self.active && self.container_ids.contains(container_id)
    }

    fn group_matches(&self, group: ContainerGroup) -> bool {
        self.active && self.groups.contains(&group)
    }
}

#[component]
pub fn InventoryPage() -> Element {
    let item_data = use_resource(load_craft_data);
    #[cfg(target_arch = "wasm32")]
    let persisted = use_resource(load_inventory_state);
    let connection = use_signal(|| None::<Rc<InventoryBridgeConnection>>);
    let generation = use_signal(|| 0_u64);
    let mut started = use_signal(|| false);
    let mut hydrated = use_signal(|| false);
    let status = use_signal(|| InventoryConnectionState::Connecting);
    let mut directory_revision = use_signal(|| 0_u64);
    let mut containers = use_signal(Vec::<InventoryContainerDescriptor>::new);
    let mut snapshots = use_signal(HashMap::<String, InventoryContainerSnapshot>::new);
    let persist_generation = use_hook(|| Rc::new(Cell::new(0_u64)));
    let page_alive = use_hook(|| Rc::new(Cell::new(true)));
    let mut saved_at = use_signal(|| None::<String>);
    let mut storage_error = use_signal(|| None::<String>);
    let refreshing = use_signal(|| false);
    let mut selected = use_signal(|| None::<String>);
    let mut query = use_signal(String::new);
    let mut active_group = use_signal(|| ContainerGroup::Character);

    let drop_alive = page_alive.clone();
    use_drop(move || drop_alive.set(false));

    let effect_persist_generation = persist_generation.clone();
    let effect_page_alive = page_alive.clone();
    use_effect(move || {
        if hydrated() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let Some(result) = persisted.read().as_ref().cloned() else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        let result: Result<Option<PersistedInventoryState>, String> = Ok(None);

        match result {
            Ok(Some(state)) => {
                let mut restored_containers = state.containers;
                restored_containers.sort_by_key(container_sort_key);
                selected.set(
                    restored_containers
                        .first()
                        .map(|container| container.container_id.clone()),
                );
                directory_revision.set(state.directory_revision);
                containers.set(restored_containers);
                snapshots.set(state.snapshots);
                saved_at.set(Some(state.saved_at));
            }
            Ok(None) => {}
            Err(error) => storage_error.set(Some(error)),
        }
        hydrated.set(true);
    });

    use_effect(move || {
        if started() || !hydrated() {
            return;
        }
        started.set(true);
        start_inventory_connection(
            connection,
            generation,
            status,
            directory_revision,
            containers,
            snapshots,
            selected,
            effect_persist_generation.clone(),
            effect_page_alive.clone(),
            saved_at,
            storage_error,
            refreshing,
        );
    });

    let status_snapshot = status();
    let mut container_list = containers();
    container_list.sort_by_key(container_sort_key);
    let query_snapshot = query();
    let data_snapshot = item_data.read().as_ref().cloned();
    let snapshot_map = snapshots();
    let active_group_snapshot = active_group();
    let search_matches = Rc::new(build_inventory_search_matches(
        &query_snapshot,
        &container_list,
        &snapshot_map,
        &data_snapshot,
    ));

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3 sm:px-6 lg:px-8",
                div { class: "min-w-0",
                    div { class: "text-sm text-muted-foreground", "本地角色数据" }
                    h1 { class: "text-2xl font-semibold", "物品" }
                }
                div { class: "flex flex-wrap items-center justify-end gap-2",
                    InventorySearchInput {
                        query: query_snapshot.clone(),
                        on_query_change: move |value| query.set(value),
                    }
                    ConnectionBadge { status: status_snapshot.clone() }
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        disabled: refreshing() || matches!(status_snapshot, InventoryConnectionState::Connecting),
                        onclick: move |_| start_inventory_connection(
                            connection,
                            generation,
                            status,
                            directory_revision,
                            containers,
                            snapshots,
                            selected,
                            persist_generation.clone(),
                            page_alive.clone(),
                            saved_at,
                            storage_error,
                            refreshing,
                        ),
                        Icon { kind: IconKind::RotateCcw, class: "h-4 w-4" }
                        if refreshing() { "刷新中" } else { "全量刷新" }
                    }
                }
            }

            match status_snapshot {
                InventoryConnectionState::Unconfigured if container_list.is_empty() => rsx! {
                    div { class: "flex flex-1 items-center justify-center p-6",
                        div { class: "max-w-sm text-center",
                            div { class: "mx-auto mb-3 flex h-10 w-10 items-center justify-center rounded-md border bg-card",
                                Icon { kind: IconKind::PlugZap, class: "h-5 w-5" }
                            }
                            div { class: "font-medium", "尚未配置本地桥接" }
                            a {
                                href: "#/settings",
                                class: "mt-4 inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground",
                                "打开设置"
                            }
                        }
                    }
                },
                InventoryConnectionState::Error(ref error) if container_list.is_empty() => rsx! {
                    InventoryUnavailable {
                        title: "无法读取本地物品",
                        detail: error.clone(),
                    }
                },
                InventoryConnectionState::Disconnected if container_list.is_empty() => rsx! {
                    InventoryUnavailable {
                        title: "本地桥接已断开",
                        detail: "重新连接以获取当前角色的物品状态。".to_string(),
                    }
                },
                InventoryConnectionState::WaitingForLogin if container_list.is_empty() => rsx! {
                    InventoryUnavailable {
                        title: "等待角色登录",
                        detail: "连接保持开启，角色登录后会自动刷新。".to_string(),
                    }
                },
                _ if container_list.is_empty() => rsx! {
                    div { class: "flex flex-1 items-center justify-center text-sm text-muted-foreground",
                        Icon { kind: IconKind::LoaderCircle, class: "mr-2 h-4 w-4 animate-spin" }
                        "正在读取容器"
                    }
                },
                _ => rsx! {
                    main { class: "min-h-0 flex-1 overflow-y-auto",
                        if !matches!(status_snapshot, InventoryConnectionState::Connected) {
                            div { class: "border-b bg-amber-50 px-4 py-2 text-xs text-amber-900 sm:px-6 lg:px-8",
                                "当前显示持久化的上次物品快照；连接后点击全量刷新可替换当前数据。"
                            }
                        }
                        if let Some(error) = storage_error() {
                            div { class: "border-b bg-destructive/10 px-4 py-2 text-xs text-destructive sm:px-6 lg:px-8", "{error}" }
                        }
                        div { class: "border-b px-4 py-3 text-xs text-muted-foreground sm:px-6 lg:px-8",
                            "目录修订 {directory_revision()} · {container_list.len()} 个容器"
                            if let Some(saved_at) = saved_at() { span { class: "ml-3", "已保存 {saved_at}" } }
                        }
                        div { class: "px-4 py-4 sm:px-6 lg:px-8",
                            div { class: "mb-5 flex max-w-full gap-1 overflow-x-auto border-b pb-2",
                                for group in ContainerGroup::all() {
                                    button {
                                        key: "{group.label()}",
                                        r#type: "button",
                                        class: group_tab_class(
                                            group == active_group_snapshot,
                                            search_matches.group_matches(group),
                                        ),
                                        style: search_highlight_style(search_matches.group_matches(group)),
                                        onclick: move |_| active_group.set(group),
                                        "{group.label()}"
                                    }
                                }
                            }
                            section {
                                div { class: "mb-3 flex items-baseline gap-3",
                                    h2 { class: "text-base font-semibold", "{active_group_snapshot.label()}" }
                                    span { class: "text-xs text-muted-foreground",
                                        "{container_list.iter().filter(|container| active_group_snapshot.matches(container.kind)).count()} 个区块"
                                    }
                                }
                                div { class: "flex items-start gap-4 overflow-x-auto pb-2 lg:flex-wrap lg:overflow-visible",
                                    for descriptor in container_list.iter().filter(|container| active_group_snapshot.matches(container.kind)) {
                                        InventoryContainerBlock {
                                            key: "{descriptor.container_id}",
                                            descriptor: descriptor.clone(),
                                            snapshot: snapshot_map.get(&descriptor.container_id).cloned(),
                                            data: data_snapshot.clone(),
                                            search: search_matches.clone(),
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn InventoryContainerBlock(
    descriptor: InventoryContainerDescriptor,
    snapshot: Option<InventoryContainerSnapshot>,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    let search_match = search.container_matches(&descriptor.container_id);
    rsx! {
        article {
            class: container_block_class(descriptor.kind),
            style: if search_match { "border-color: #f59e0b; box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.22);" } else { "" },
            header { class: if search_match { "flex min-h-12 items-center justify-between gap-3 border-b border-amber-200 bg-amber-50/70 px-3 py-2" } else { "flex min-h-12 items-center justify-between gap-3 border-b px-3 py-2" },
                div { class: "min-w-0",
                    div { class: "flex items-center gap-2",
                        h3 { class: if search_match { "truncate text-sm font-semibold text-amber-900" } else { "truncate text-sm font-semibold" }, "{container_label(&descriptor)}" }
                        AvailabilityBadge { availability: descriptor.availability }
                    }
                    div { class: "mt-0.5 text-[11px] text-muted-foreground",
                        "{descriptor.occupied_slots} / {capacity_label(descriptor.capacity)} 槽 · {format_integer(descriptor.total_quantity)} 件"
                    }
                }
            }
            div { class: "p-3",
                if descriptor.availability == InventoryContainerAvailability::NotLoaded {
                    div { class: "flex h-36 items-center justify-center text-xs text-muted-foreground", "该容器尚未加载" }
                } else if let Some(snapshot) = snapshot {
                    match descriptor.kind {
                        InventoryContainerKind::Equipped | InventoryContainerKind::RetainerEquipped => rsx! {
                            EquipmentLayout { snapshot, data, search }
                        },
                        InventoryContainerKind::Crystals | InventoryContainerKind::RetainerCrystals => rsx! {
                            CrystalLayout { snapshot, data, search }
                        },
                        InventoryContainerKind::RetainerInventory => rsx! {
                            RetainerInventoryLayout { snapshot, data, search }
                        },
                        InventoryContainerKind::Cabinet | InventoryContainerKind::GlamourDresser => rsx! {
                            DenseItemGrid { snapshot, data, search }
                        },
                        _ => rsx! { FixedSlotGrid { snapshot, data, search } },
                    }
                } else {
                    div { class: "flex h-36 items-center justify-center text-xs text-muted-foreground",
                        Icon { kind: IconKind::LoaderCircle, class: "mr-2 h-4 w-4 animate-spin" }
                        "正在读取内容"
                    }
                }
            }
        }
    }
}

#[component]
fn FixedSlotGrid(
    snapshot: InventoryContainerSnapshot,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    let capacity = snapshot.capacity.unwrap_or(snapshot.items.len() as u32) as usize;
    let slots = items_by_slot(&snapshot, capacity);
    rsx! {
        div { class: "grid grid-cols-5 gap-1",
            for (slot, item) in slots.into_iter().enumerate() {
                InventorySlot { key: "{slot}", item, data: data.clone(), search: search.clone(), label: None }
            }
        }
    }
}

#[component]
fn RetainerInventoryLayout(
    snapshot: InventoryContainerSnapshot,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    let capacity = snapshot.capacity.unwrap_or(175) as usize;
    let pages = capacity.div_ceil(35);
    let slots = items_by_slot(&snapshot, capacity);
    let mut active_page = use_signal(|| 0_usize);
    let page = active_page().min(pages.saturating_sub(1));
    rsx! {
        div {
            div { class: "mb-2 flex gap-1",
                for page_index in 0..pages {
                    button {
                        key: "{page_index}",
                        r#type: "button",
                        title: "第 {page_index + 1} 页",
                        class: inventory_page_tab_class(
                            page_index == page,
                            search.active && slots
                                .iter()
                                .skip(page_index * 35)
                                .take(35)
                                .flatten()
                                .any(|item| search.item_matches(item)),
                        ),
                        style: search_highlight_style(
                            search.active && slots
                                .iter()
                                .skip(page_index * 35)
                                .take(35)
                                .flatten()
                                .any(|item| search.item_matches(item)),
                        ),
                        onclick: move |_| active_page.set(page_index),
                        "{page_index + 1}"
                    }
                }
            }
            div { class: "grid grid-cols-5 gap-1",
                for offset in 0..35_usize {
                    InventorySlot {
                        key: "{offset}",
                        item: slots.get(page * 35 + offset).cloned().flatten(),
                        data: data.clone(),
                        search: search.clone(),
                        label: None,
                    }
                }
            }
        }
    }
}

#[component]
fn DenseItemGrid(
    snapshot: InventoryContainerSnapshot,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    rsx! {
        if snapshot.items.is_empty() {
            div { class: "flex h-24 items-center justify-center text-xs text-muted-foreground", "暂无物品" }
        } else {
            div { class: "grid max-h-[28rem] grid-cols-5 gap-1 overflow-y-auto pr-1",
                for item in snapshot.items {
                    InventorySlot { key: "{item.slot}-{item.item_id}", item: Some(item), data: data.clone(), search: search.clone(), label: None }
                }
            }
        }
    }
}

#[component]
fn EquipmentLayout(
    snapshot: InventoryContainerSnapshot,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    const LEFT: [(u32, &str); 6] = [
        (0, "主手"),
        (2, "头部"),
        (3, "身体"),
        (4, "手部"),
        (6, "腿部"),
        (7, "脚部"),
    ];
    const RIGHT: [(u32, &str); 6] = [
        (1, "副手"),
        (8, "耳饰"),
        (9, "项链"),
        (10, "手镯"),
        (11, "右戒"),
        (12, "左戒"),
    ];
    let slots = items_by_slot(&snapshot, 14);
    rsx! {
        div { class: "grid grid-cols-[5rem_5rem] gap-x-5 gap-y-1",
            div { class: "grid gap-1",
                for (slot, label) in LEFT {
                    EquipmentSlot { key: "{slot}", item: slots.get(slot as usize).cloned().flatten(), data: data.clone(), search: search.clone(), label }
                }
            }
            div { class: "grid gap-1",
                for (slot, label) in RIGHT {
                    EquipmentSlot { key: "{slot}", item: slots.get(slot as usize).cloned().flatten(), data: data.clone(), search: search.clone(), label }
                }
            }
            div { class: "col-span-2 mt-2 flex justify-center",
                EquipmentSlot { item: slots.get(13).cloned().flatten(), data, search, label: "灵魂水晶" }
            }
        }
    }
}

#[component]
fn EquipmentSlot(
    item: Option<InventoryItemEntry>,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
    label: &'static str,
) -> Element {
    rsx! {
        div { class: "grid grid-cols-[3rem_1.75rem] items-center gap-1",
            InventorySlot { item, data, search, label: Some(label.to_string()) }
            span { class: "text-[10px] leading-tight text-muted-foreground", "{label}" }
        }
    }
}

#[component]
fn CrystalLayout(
    snapshot: InventoryContainerSnapshot,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    const ELEMENTS: [&str; 6] = ["火", "冰", "风", "土", "雷", "水"];
    const GRADES: [&str; 3] = ["碎晶", "水晶", "晶簇"];
    let slots = items_by_slot(&snapshot, 18);
    rsx! {
        div { class: "grid grid-cols-[1.5rem_repeat(3,4rem)] items-center gap-1",
            div {}
            for grade in GRADES { div { class: "text-center text-[10px] text-muted-foreground", "{grade}" } }
            for (element_index, element) in ELEMENTS.iter().enumerate() {
                div { class: crystal_element_class(element_index), "{element}" }
                for grade_index in 0..3_usize {
                    CrystalSlot {
                        key: "{element_index}-{grade_index}",
                        item: slots.get(grade_index * 6 + element_index).cloned().flatten(),
                        data: data.clone(),
                        search: search.clone(),
                    }
                }
            }
        }
    }
}

#[component]
fn CrystalSlot(
    item: Option<InventoryItemEntry>,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
) -> Element {
    let quantity = item.as_ref().map(|item| item.quantity).unwrap_or(0);
    let matches = item.as_ref().is_some_and(|item| search.item_matches(item));
    let class = search_slot_class(matches, search.active, true);
    rsx! {
        div { class,
            if let Some(item) = item {
                InventoryItemIcon { icon: item_icon(&data, item.item_id), size: "sm" }
            } else {
                div { class: "h-5 w-5 rounded-sm bg-muted" }
            }
            span { class: "text-right text-xs tabular-nums", "{format_integer(quantity)}" }
        }
    }
}

#[component]
fn InventorySlot(
    item: Option<InventoryItemEntry>,
    data: Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    search: Rc<InventorySearchMatches>,
    label: Option<String>,
) -> Element {
    let matches = item.as_ref().is_some_and(|item| search.item_matches(item));
    let title = item
        .as_ref()
        .map(|item| item_title(item, &data, label.as_deref()))
        .unwrap_or_else(|| label.unwrap_or_else(|| "空槽位".to_string()));
    rsx! {
        div {
            title,
            class: search_slot_class(matches, search.active, false),
            if let Some(item) = item {
                InventoryItemIcon { icon: item_icon(&data, item.item_id), size: "lg" }
                if item.quantity > 1 {
                    span { class: "absolute bottom-0.5 right-1 rounded-sm bg-black/70 px-0.5 text-[10px] font-medium leading-4 text-white tabular-nums", "{format_integer(item.quantity)}" }
                }
                if item.hq {
                    span { class: "absolute left-0.5 top-0.5 h-2 w-2 rounded-full border border-white bg-amber-400" }
                }
            } else {
                div { class: "h-8 w-8 rounded bg-muted/50" }
            }
        }
    }
}

fn items_by_slot(
    snapshot: &InventoryContainerSnapshot,
    capacity: usize,
) -> Vec<Option<InventoryItemEntry>> {
    let mut slots = vec![None; capacity];
    for item in &snapshot.items {
        if let Some(slot) = slots.get_mut(item.slot as usize) {
            *slot = Some(item.clone());
        }
    }
    slots
}

#[component]
fn InventoryItemIcon(icon: u32, #[props(default = "lg")] size: &'static str) -> Element {
    let size_class = if size == "sm" { "h-5 w-5" } else { "h-10 w-10" };
    let src = builtin_icon_urls(icon).into_iter().next();
    if let Some(src) = src {
        rsx! {
            img {
                src,
                alt: "",
                loading: "lazy",
                decoding: "async",
                class: "{size_class} shrink-0 rounded object-cover",
            }
        }
    } else {
        rsx! { div { class: "{size_class} shrink-0 rounded bg-muted" } }
    }
}

#[component]
fn InventorySearchInput(query: String, on_query_change: EventHandler<String>) -> Element {
    let mut draft = use_signal(|| query);
    let generation = use_hook(|| Rc::new(Cell::new(0_u64)));
    let input_generation = generation.clone();
    let clear_generation = generation.clone();
    rsx! {
        div { class: "relative w-48 sm:w-64",
            Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
            input {
                r#type: "search",
                value: "{draft}",
                placeholder: "搜索全部物品",
                class: input_class(if draft().is_empty() { "h-8 pl-9" } else { "h-8 pl-9 pr-9" }),
                oninput: move |event| {
                    let value = event.value();
                    draft.set(value.clone());
                    let active_generation = input_generation.get().wrapping_add(1);
                    input_generation.set(active_generation);
                    let generation = input_generation.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(120).await;
                        if generation.get() == active_generation {
                            on_query_change.call(value);
                        }
                    });
                },
            }
            if !draft().is_empty() {
                button {
                    r#type: "button",
                    title: "清空搜索",
                    aria_label: "清空搜索",
                    class: "absolute right-0.5 top-0.5 flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                    onclick: move |_| {
                        clear_generation.set(clear_generation.get().wrapping_add(1));
                        draft.set(String::new());
                        on_query_change.call(String::new());
                    },
                    Icon { kind: IconKind::X, class: "h-4 w-4" }
                }
            }
        }
    }
}

fn item_icon(
    data: &Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    item_id: u32,
) -> u32 {
    data.as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|data| data.items.get(&item_id.to_string()))
        .map(|item| item.icon)
        .unwrap_or(0)
}

fn item_name<'a>(
    data: &'a Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    item_id: u32,
) -> &'a str {
    data.as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|data| data.items.get(&item_id.to_string()))
        .map(|item| item.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("未知物品")
}

fn build_inventory_search_matches(
    query: &str,
    containers: &[InventoryContainerDescriptor],
    snapshots: &HashMap<String, InventoryContainerSnapshot>,
    data: &Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
) -> InventorySearchMatches {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return InventorySearchMatches::default();
    }

    let mut matches = InventorySearchMatches {
        active: true,
        ..InventorySearchMatches::default()
    };
    for item_id in snapshots
        .values()
        .flat_map(|snapshot| snapshot.items.iter().map(|item| item.item_id))
        .collect::<HashSet<_>>()
    {
        if item_id.to_string().contains(&query)
            || item_name(data, item_id).to_lowercase().contains(&query)
        {
            matches.item_ids.insert(item_id);
        }
    }

    for container in containers {
        let label_matches = container_label(container).to_lowercase().contains(&query);
        let item_matches = snapshots
            .get(&container.container_id)
            .is_some_and(|snapshot| {
                snapshot
                    .items
                    .iter()
                    .any(|item| matches.item_ids.contains(&item.item_id))
            });
        if label_matches || item_matches {
            matches.container_ids.insert(container.container_id.clone());
        }
    }

    for group in ContainerGroup::all() {
        let label_matches = group.label().to_lowercase().contains(&query);
        let container_matches = containers.iter().any(|container| {
            group.matches(container.kind) && matches.container_ids.contains(&container.container_id)
        });
        if label_matches || container_matches {
            matches.groups.insert(group);
        }
    }

    matches
}

fn item_title(
    item: &InventoryItemEntry,
    data: &Option<Result<Rc<xiv_companion::CraftDataPackage>, String>>,
    label: Option<&str>,
) -> String {
    let prefix = label.map(|label| format!("{label} · ")).unwrap_or_default();
    let hq = if item.hq { " · HQ" } else { "" };
    format!(
        "{prefix}{} · {} 个 · Item {}{hq}",
        item_name(data, item.item_id),
        item.quantity,
        item.item_id
    )
}

fn container_block_class(kind: InventoryContainerKind) -> &'static str {
    match kind {
        InventoryContainerKind::Equipped | InventoryContainerKind::RetainerEquipped => {
            "w-[15rem] shrink-0 overflow-hidden rounded-md border bg-card"
        }
        InventoryContainerKind::Crystals | InventoryContainerKind::RetainerCrystals => {
            "w-[16rem] shrink-0 overflow-hidden rounded-md border bg-card"
        }
        InventoryContainerKind::RetainerInventory => {
            "max-w-[calc(100vw-3rem)] shrink-0 overflow-hidden rounded-md border bg-card lg:max-w-[calc(100vw-8rem)]"
        }
        _ => "w-[17.5rem] shrink-0 overflow-hidden rounded-md border bg-card",
    }
}

fn group_tab_class(active: bool, search_match: bool) -> &'static str {
    match (active, search_match) {
        (true, true) => {
            "h-8 shrink-0 rounded border bg-foreground px-3 text-sm font-medium text-background shadow-sm"
        }
        (true, false) => {
            "h-8 shrink-0 rounded border border-transparent bg-foreground px-3 text-sm font-medium text-background"
        }
        (false, true) => {
            "h-8 shrink-0 rounded border bg-amber-50 px-3 text-sm font-medium text-amber-900 shadow-sm hover:bg-amber-100"
        }
        (false, false) => {
            "h-8 shrink-0 rounded border border-transparent px-3 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
        }
    }
}

fn inventory_page_tab_class(active: bool, search_match: bool) -> &'static str {
    match (active, search_match) {
        (true, true) => {
            "flex h-6 min-w-6 items-center justify-center rounded border bg-foreground px-1.5 text-[11px] font-medium text-background shadow-sm"
        }
        (true, false) => {
            "flex h-6 min-w-6 items-center justify-center rounded border border-transparent bg-foreground px-1.5 text-[11px] font-medium text-background"
        }
        (false, true) => {
            "flex h-6 min-w-6 items-center justify-center rounded border bg-amber-50 px-1.5 text-[11px] font-medium text-amber-900 shadow-sm hover:bg-amber-100"
        }
        (false, false) => {
            "flex h-6 min-w-6 items-center justify-center rounded border bg-background px-1.5 text-[11px] text-muted-foreground hover:bg-accent"
        }
    }
}

fn search_slot_class(matches: bool, search_active: bool, compact: bool) -> &'static str {
    match (compact, search_active, matches) {
        (true, true, true) => {
            "grid h-9 grid-cols-[1.75rem_minmax(0,1fr)] items-center rounded border border-amber-200 bg-amber-50 px-1 shadow-sm"
        }
        (true, true, false) => {
            "grid h-9 grid-cols-[1.75rem_minmax(0,1fr)] items-center rounded border bg-background px-1 opacity-20"
        }
        (true, false, _) => {
            "grid h-9 grid-cols-[1.75rem_minmax(0,1fr)] items-center rounded border bg-background px-1"
        }
        (false, true, true) => {
            "relative flex h-12 w-12 items-center justify-center overflow-hidden rounded border border-amber-200 bg-amber-50 shadow-sm"
        }
        (false, true, false) => {
            "relative flex h-12 w-12 items-center justify-center overflow-hidden rounded border bg-background opacity-20"
        }
        (false, false, _) => {
            "relative flex h-12 w-12 items-center justify-center overflow-hidden rounded border bg-background shadow-sm"
        }
    }
}

fn search_highlight_style(search_match: bool) -> &'static str {
    if search_match {
        "border-color: #f59e0b; box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.22);"
    } else {
        ""
    }
}

fn crystal_element_class(index: usize) -> &'static str {
    match index {
        0 => "text-center text-xs font-semibold text-red-500",
        1 => "text-center text-xs font-semibold text-sky-500",
        2 => "text-center text-xs font-semibold text-lime-600",
        3 => "text-center text-xs font-semibold text-amber-600",
        4 => "text-center text-xs font-semibold text-fuchsia-500",
        _ => "text-center text-xs font-semibold text-cyan-600",
    }
}

#[component]
fn AvailabilityBadge(availability: InventoryContainerAvailability) -> Element {
    let (variant, label) = match availability {
        InventoryContainerAvailability::Live => (BadgeVariant::Success, "实时"),
        InventoryContainerAvailability::Cached => (BadgeVariant::Warning, "缓存"),
        InventoryContainerAvailability::NotLoaded => (BadgeVariant::Outline, "未加载"),
    };
    rsx! { Badge { variant, "{label}" } }
}

#[component]
fn ConnectionBadge(status: InventoryConnectionState) -> Element {
    let (variant, label) = match status {
        InventoryConnectionState::Connecting => (BadgeVariant::Warning, "连接中"),
        InventoryConnectionState::Connected => (BadgeVariant::Success, "已连接"),
        InventoryConnectionState::Unconfigured => (BadgeVariant::Outline, "未配置"),
        InventoryConnectionState::WaitingForLogin => (BadgeVariant::Outline, "等待登录"),
        InventoryConnectionState::Disconnected => (BadgeVariant::Outline, "已断开"),
        InventoryConnectionState::Error(_) => (BadgeVariant::Warning, "错误"),
    };
    rsx! { Badge { variant, "{label}" } }
}

#[component]
fn InventoryUnavailable(title: &'static str, detail: String) -> Element {
    rsx! {
        div { class: "flex flex-1 items-center justify-center p-6",
            div { class: "max-w-md text-center",
                div { class: "mx-auto mb-3 flex h-10 w-10 items-center justify-center rounded-md border bg-card",
                    Icon { kind: IconKind::PackageSearch, class: "h-5 w-5" }
                }
                div { class: "font-medium", "{title}" }
                div { class: "mt-1 text-sm text-muted-foreground", "{detail}" }
            }
        }
    }
}

fn start_inventory_connection(
    mut connection: Signal<Option<Rc<InventoryBridgeConnection>>>,
    mut generation: Signal<u64>,
    mut status: Signal<InventoryConnectionState>,
    mut directory_revision: Signal<u64>,
    mut containers: Signal<Vec<InventoryContainerDescriptor>>,
    mut snapshots: Signal<HashMap<String, InventoryContainerSnapshot>>,
    mut selected: Signal<Option<String>>,
    persist_generation: Rc<Cell<u64>>,
    page_alive: Rc<Cell<bool>>,
    saved_at: Signal<Option<String>>,
    storage_error: Signal<Option<String>>,
    mut refreshing: Signal<bool>,
) {
    let active_generation = generation.peek().wrapping_add(1);
    generation.set(active_generation);
    connection.set(None);
    status.set(InventoryConnectionState::Connecting);
    refreshing.set(true);

    let Some(url) = load_verified_bridge_url() else {
        status.set(InventoryConnectionState::Unconfigured);
        refreshing.set(false);
        return;
    };

    let staged_directory = Rc::new(RefCell::new(None::<InventoryContainerDirectory>));
    let staged_snapshots = Rc::new(RefCell::new(
        HashMap::<String, InventoryContainerSnapshot>::new(),
    ));
    match InventoryBridgeConnection::connect(&url, move |update| {
        if *generation.peek() != active_generation {
            return;
        }
        match update {
            InventoryBridgeUpdate::Connected => status.set(InventoryConnectionState::Connected),
            InventoryBridgeUpdate::RefreshStarted => {
                refreshing.set(true);
                staged_directory.borrow_mut().take();
                staged_snapshots.borrow_mut().clear();
            }
            InventoryBridgeUpdate::RefreshComplete => {
                refreshing.set(false);
                if let Some(directory) = staged_directory.borrow_mut().take() {
                    let incoming = std::mem::take(&mut *staged_snapshots.borrow_mut());
                    apply_refresh(
                        directory,
                        incoming,
                        &mut directory_revision,
                        &mut containers,
                        &mut snapshots,
                        &mut selected,
                    );
                    schedule_inventory_persist(
                        directory_revision,
                        containers,
                        snapshots,
                        persist_generation.clone(),
                        page_alive.clone(),
                        saved_at,
                        storage_error,
                    );
                }
            }
            InventoryBridgeUpdate::Directory(directory) => {
                if !should_apply_directory(*directory_revision.peek(), directory.revision) {
                    return;
                }
                *staged_directory.borrow_mut() = Some(directory);
                status.set(InventoryConnectionState::Connected);
            }
            InventoryBridgeUpdate::Container(container) => {
                if *refreshing.peek() {
                    let should_stage = {
                        let staged = staged_snapshots.borrow();
                        let current = staged
                            .get(&container.container_id)
                            .cloned()
                            .or_else(|| snapshots.peek().get(&container.container_id).cloned());
                        should_apply_container(current.as_ref(), &container)
                    };
                    if should_stage {
                        staged_snapshots
                            .borrow_mut()
                            .insert(container.container_id.clone(), container);
                    }
                    return;
                }
                let should_apply = should_apply_container(
                    snapshots.peek().get(&container.container_id),
                    &container,
                );
                if !should_apply {
                    return;
                }
                let descriptor = container.descriptor();
                let incoming_revision = container.revision;
                let mut next_snapshots = snapshots.peek().clone();
                next_snapshots.insert(container.container_id.clone(), container);
                snapshots.set(next_snapshots);
                let mut next_containers = containers.peek().clone();
                if let Some(existing) = next_containers
                    .iter_mut()
                    .find(|existing| existing.container_id == descriptor.container_id)
                {
                    *existing = descriptor;
                } else {
                    next_containers.push(descriptor);
                }
                containers.set(next_containers);
                let current_revision = *directory_revision.peek();
                directory_revision.set(current_revision.max(incoming_revision));
                if !*refreshing.peek() {
                    schedule_inventory_persist(
                        directory_revision,
                        containers,
                        snapshots,
                        persist_generation.clone(),
                        page_alive.clone(),
                        saved_at,
                        storage_error,
                    );
                }
            }
            InventoryBridgeUpdate::WaitingForLogin => {
                refreshing.set(false);
                status.set(InventoryConnectionState::WaitingForLogin);
            }
            InventoryBridgeUpdate::Disconnected => {
                refreshing.set(false);
                status.set(InventoryConnectionState::Disconnected)
            }
            InventoryBridgeUpdate::Error(error) => {
                refreshing.set(false);
                status.set(InventoryConnectionState::Error(error))
            }
        }
    }) {
        Ok(next) => connection.set(Some(next)),
        Err(error) => {
            refreshing.set(false);
            status.set(InventoryConnectionState::Error(error));
        }
    }
}

fn schedule_inventory_persist(
    directory_revision: Signal<u64>,
    containers: Signal<Vec<InventoryContainerDescriptor>>,
    snapshots: Signal<HashMap<String, InventoryContainerSnapshot>>,
    persist_generation: Rc<Cell<u64>>,
    page_alive: Rc<Cell<bool>>,
    mut saved_at: Signal<Option<String>>,
    mut storage_error: Signal<Option<String>>,
) {
    let active_generation = persist_generation.get().wrapping_add(1);
    persist_generation.set(active_generation);
    wasm_bindgen_futures::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(150).await;
        if !page_alive.get() || persist_generation.get() != active_generation {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let revision = *directory_revision.peek();
            let containers = containers.peek().clone();
            let snapshots = snapshots.peek().clone();
            let state = PersistedInventoryState::new(revision, containers, snapshots);
            let result = save_inventory_state(&state).await;
            if !page_alive.get() || persist_generation.get() != active_generation {
                return;
            }
            match result {
                Ok(()) => {
                    saved_at.set(Some(state.saved_at));
                    storage_error.set(None);
                }
                Err(error) => storage_error.set(Some(error)),
            }
        }
    });
}

fn apply_refresh(
    directory: InventoryContainerDirectory,
    incoming: HashMap<String, InventoryContainerSnapshot>,
    directory_revision: &mut Signal<u64>,
    containers: &mut Signal<Vec<InventoryContainerDescriptor>>,
    snapshots: &mut Signal<HashMap<String, InventoryContainerSnapshot>>,
    selected: &mut Signal<Option<String>>,
) {
    let known_ids = directory
        .containers
        .iter()
        .map(|container| container.container_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut next_snapshots = snapshots.peek().clone();
    next_snapshots.retain(|id, _| known_ids.contains(id));
    let mut next_containers = directory.containers;
    let mut next_revision = directory.revision;

    for container in incoming.into_values() {
        if !should_apply_container(next_snapshots.get(&container.container_id), &container) {
            continue;
        }
        let descriptor = container.descriptor();
        next_revision = next_revision.max(container.revision);
        next_snapshots.insert(container.container_id.clone(), container);
        if let Some(existing) = next_containers
            .iter_mut()
            .find(|existing| existing.container_id == descriptor.container_id)
        {
            *existing = descriptor;
        }
    }

    let selected_still_exists = selected
        .peek()
        .as_ref()
        .is_some_and(|id| known_ids.contains(id));
    let next_selected = if selected_still_exists {
        selected.peek().clone()
    } else {
        let mut sorted = next_containers.clone();
        sorted.sort_by_key(container_sort_key);
        sorted
            .first()
            .map(|container| container.container_id.clone())
    };

    directory_revision.set(next_revision);
    containers.set(next_containers);
    snapshots.set(next_snapshots);
    selected.set(next_selected);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ContainerGroup {
    Character,
    Armoury,
    Saddlebag,
    Retainer,
    Collection,
}

impl ContainerGroup {
    fn all() -> [Self; 5] {
        [
            Self::Character,
            Self::Armoury,
            Self::Saddlebag,
            Self::Retainer,
            Self::Collection,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Character => "角色",
            Self::Armoury => "兵装库",
            Self::Saddlebag => "陆行鸟鞍囊",
            Self::Retainer => "雇员",
            Self::Collection => "收藏设施",
        }
    }

    fn matches(self, kind: InventoryContainerKind) -> bool {
        match self {
            Self::Character => matches!(
                kind,
                InventoryContainerKind::Inventory
                    | InventoryContainerKind::Equipped
                    | InventoryContainerKind::Currency
                    | InventoryContainerKind::Crystals
                    | InventoryContainerKind::KeyItems
            ),
            Self::Armoury => kind == InventoryContainerKind::Armoury,
            Self::Saddlebag => matches!(
                kind,
                InventoryContainerKind::Saddlebag | InventoryContainerKind::PremiumSaddlebag
            ),
            Self::Retainer => matches!(
                kind,
                InventoryContainerKind::RetainerInventory
                    | InventoryContainerKind::RetainerEquipped
                    | InventoryContainerKind::RetainerMarket
                    | InventoryContainerKind::RetainerCrystals
            ),
            Self::Collection => matches!(
                kind,
                InventoryContainerKind::Cabinet | InventoryContainerKind::GlamourDresser
            ),
        }
    }
}

fn container_label(container: &InventoryContainerDescriptor) -> String {
    let index = container.index.unwrap_or(1);
    let owner = container
        .owner_id
        .as_deref()
        .and_then(|owner| owner.rsplit(':').next())
        .unwrap_or("?");
    match container.kind {
        InventoryContainerKind::Inventory => format!("背包 {index}"),
        InventoryContainerKind::Equipped => "当前装备".to_string(),
        InventoryContainerKind::Armoury => armoury_label(container.category.as_deref()).to_string(),
        InventoryContainerKind::Currency => "货币".to_string(),
        InventoryContainerKind::Crystals => "水晶".to_string(),
        InventoryContainerKind::KeyItems => "关键道具".to_string(),
        InventoryContainerKind::Saddlebag => format!("鞍囊 {index}"),
        InventoryContainerKind::PremiumSaddlebag => format!("高级鞍囊 {index}"),
        InventoryContainerKind::RetainerInventory => format!("雇员 {owner} · 背包"),
        InventoryContainerKind::RetainerEquipped => format!("雇员 {owner} · 装备"),
        InventoryContainerKind::RetainerMarket => format!("雇员 {owner} · 出售栏"),
        InventoryContainerKind::RetainerCrystals => format!("雇员 {owner} · 水晶"),
        InventoryContainerKind::Cabinet => "收藏柜".to_string(),
        InventoryContainerKind::GlamourDresser => "投影台".to_string(),
    }
}

fn armoury_label(category: Option<&str>) -> &'static str {
    match category {
        Some("mainHand") => "主手",
        Some("offHand") => "副手",
        Some("head") => "头部",
        Some("body") => "身体",
        Some("hands") => "手部",
        Some("waist") => "腰带",
        Some("legs") => "腿部",
        Some("feet") => "脚部",
        Some("earrings") => "耳饰",
        Some("necklace") => "项链",
        Some("bracelets") => "手镯",
        Some("rings") => "戒指",
        Some("soulCrystal") => "灵魂水晶",
        _ => "兵装库",
    }
}

fn container_sort_key(container: &InventoryContainerDescriptor) -> (u8, u32, String) {
    let group = match container.kind {
        InventoryContainerKind::Inventory => 0,
        InventoryContainerKind::Equipped => 1,
        InventoryContainerKind::Currency => 2,
        InventoryContainerKind::Crystals => 3,
        InventoryContainerKind::KeyItems => 4,
        InventoryContainerKind::Armoury => 10,
        InventoryContainerKind::Saddlebag => 20,
        InventoryContainerKind::PremiumSaddlebag => 21,
        InventoryContainerKind::RetainerInventory => 30,
        InventoryContainerKind::RetainerEquipped => 31,
        InventoryContainerKind::RetainerMarket => 32,
        InventoryContainerKind::RetainerCrystals => 33,
        InventoryContainerKind::Cabinet => 40,
        InventoryContainerKind::GlamourDresser => 41,
    };
    (
        group,
        container.index.unwrap_or(0),
        container.container_id.clone(),
    )
}

fn capacity_label(capacity: Option<u32>) -> String {
    capacity.map_or_else(|| "?".to_string(), |value| value.to_string())
}
