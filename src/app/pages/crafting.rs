use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::app::data::{
    CRAFT_TYPE_ABBRS, CRAFT_TYPE_NAMES, build_tree, collapse_key, craftable_recipes,
    create_craft_data_engine, default_source_index, get_icon_urls, get_item, get_item_name,
    load_craft_data, resolve_source, source_label, source_priority, summarize_materials,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::{cx, format_integer};
use dioxus::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use xiv_companion::{
    CraftDataPackage, CraftItem, CraftRecipe, CraftTreeNode, CrafterAttributes, ItemSource,
    MacroAction, MacroSolveResult, MaterialSummary, RaphaelSolveOptions, SourceChoice,
    solve_raphael_macro,
};

const MARKET_WORLD_DC_REGION: &str = "中国";
const UNIVERSALIS_BASE_URL: &str = "https://universalis.app";

const DEFAULT_CRAFTER_ATTRIBUTES: CrafterAttributes = CrafterAttributes {
    level: 100,
    craftsmanship: 4900,
    control: 4800,
    craft_points: 620,
};

const DEFAULT_SOLVE_OPTIONS: RaphaelSolveOptions = RaphaelSolveOptions {
    target_quality: None,
    use_manipulation: true,
    use_heart_and_soul: false,
    use_quick_innovation: false,
    use_trained_eye: true,
    backload_progress: false,
    adversarial: false,
    stellar_steady_hand_charges: 0,
};

#[derive(Clone, PartialEq)]
pub(super) struct SourceDisplayGroup {
    key: String,
    source: ItemSource,
    indices: Vec<usize>,
    cost_label: Option<String>,
    details: Vec<String>,
}

#[derive(Clone, PartialEq)]
pub(super) struct MaterialPlanEntry {
    pub(super) item_id: u32,
    pub(super) amount: u32,
    pub(super) sources: Vec<ItemSource>,
    pub(super) choice: Option<SourceChoice>,
    pub(super) source: Option<ItemSource>,
    pub(super) marketable: bool,
    pub(super) shop_name: Option<String>,
    pub(super) gil: Option<u32>,
    pub(super) costs: Option<Vec<MaterialCost>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct MaterialCost {
    pub(super) item_id: u32,
    pub(super) amount: u32,
}

#[derive(Clone, PartialEq)]
pub(super) struct ExchangePlanGroup {
    pub(super) key: String,
    pub(super) shop_name: String,
    pub(super) costs: Vec<MaterialCost>,
    pub(super) entries: Vec<MaterialPlanEntry>,
}

#[derive(Clone, PartialEq)]
pub(super) struct MaterialPlan {
    pub(super) gathering: Vec<MaterialPlanEntry>,
    pub(super) shops: Vec<MaterialPlanEntry>,
    pub(super) market: Vec<MaterialPlanEntry>,
    pub(super) exchange_groups: Vec<ExchangePlanGroup>,
    pub(super) owned: Vec<MaterialPlanEntry>,
    pub(super) unknown: Vec<MaterialPlanEntry>,
    pub(super) gil_total: u32,
}

impl MaterialPlan {
    pub(super) fn empty() -> Self {
        Self {
            gathering: Vec::new(),
            shops: Vec::new(),
            market: Vec::new(),
            exchange_groups: Vec::new(),
            owned: Vec::new(),
            unknown: Vec::new(),
            gil_total: 0,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.gathering.is_empty()
            && self.shops.is_empty()
            && self.market.is_empty()
            && self.exchange_groups.is_empty()
            && self.owned.is_empty()
            && self.unknown.is_empty()
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct DetailTarget {
    pub(super) item_id: u32,
    pub(super) amount_needed: u32,
    pub(super) recipe: Option<CraftRecipe>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MarketQuote {
    item_id: u32,
    unit_price: u32,
    basis: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MarketCost {
    pub(super) total: u32,
    pub(super) priced: usize,
    pub(super) missing: usize,
}

#[derive(Clone, Deserialize)]
struct UniversalisAggregatedResponse {
    #[serde(default)]
    results: Vec<UniversalisResult>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniversalisResult {
    item_id: u32,
    #[serde(default)]
    nq: UniversalisNq,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniversalisNq {
    min_listing: Option<UniversalisPriceScope>,
    recent_purchase: Option<UniversalisPriceScope>,
    average_sale_price: Option<UniversalisPriceScope>,
}

#[derive(Clone, Deserialize)]
struct UniversalisPriceScope {
    region: Option<UniversalisPrice>,
}

#[derive(Clone, Deserialize)]
struct UniversalisPrice {
    price: Option<f64>,
}

pub(super) fn recipe_level_label(data: &CraftDataPackage, recipe: &CraftRecipe) -> String {
    if recipe.secret_recipe_book > 0 {
        return data
            .secret_recipe_books
            .get(&recipe.secret_recipe_book.to_string())
            .cloned()
            .unwrap_or_else(|| "秘籍".to_string());
    }
    let level = data
        .recipe_levels
        .get(&recipe.recipe_level_table_id.to_string())
        .map(|level| level.class_job_level)
        .unwrap_or(1);
    format!("Lv.{level}")
}

pub(super) fn source_icon_kind(source: &ItemSource) -> IconKind {
    match source {
        ItemSource::GilShop { .. } => IconKind::Coins,
        ItemSource::SpecialShop { .. } => IconKind::Shuffle,
        ItemSource::Fishing { .. } => IconKind::Fish,
        ItemSource::Gathering => IconKind::Leaf,
    }
}

fn source_gil_cost(data: &CraftDataPackage, item_id: u32, amount: u32) -> Option<u32> {
    let unit_price = get_item(data, item_id)
        .map(|item| item.price_mid)
        .unwrap_or(0);
    (unit_price > 0).then_some(unit_price.saturating_mul(amount))
}

fn source_cost_label(
    data: &CraftDataPackage,
    source: &ItemSource,
    amount: u32,
    item_id: Option<u32>,
) -> Option<String> {
    match source {
        ItemSource::GilShop { .. } => item_id
            .and_then(|item_id| source_gil_cost(data, item_id, amount))
            .map(|gil| format!("{}G", format_integer(gil as f64))),
        ItemSource::SpecialShop { costs, .. } => Some(cost_list_label(
            data,
            &costs
                .iter()
                .map(|cost| MaterialCost {
                    item_id: cost.item_id,
                    amount: cost.count.saturating_mul(amount),
                })
                .collect::<Vec<_>>(),
        )),
        _ => None,
    }
}

pub(super) fn cost_list_label(data: &CraftDataPackage, costs: &[MaterialCost]) -> String {
    costs
        .iter()
        .map(|cost| {
            format!(
                "{} x{}",
                get_item_name(data, cost.item_id),
                format_integer(cost.amount as f64)
            )
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn source_info_label(source: &ItemSource) -> String {
    match source {
        ItemSource::GilShop { shop_name } | ItemSource::SpecialShop { shop_name, .. } => {
            shop_name.clone()
        }
        ItemSource::Fishing { spot_id, .. } => format!("钓场 #{spot_id}"),
        ItemSource::Gathering => "采矿 / 园艺".to_string(),
    }
}

fn source_consumption_key(
    data: &CraftDataPackage,
    source: &ItemSource,
    amount: u32,
    item_id: u32,
) -> String {
    match source {
        ItemSource::GilShop { .. } => {
            format!(
                "gilShop|{}",
                source_gil_cost(data, item_id, amount).unwrap_or(0)
            )
        }
        ItemSource::SpecialShop { costs, .. } => {
            let mut parts = costs
                .iter()
                .map(|cost| format!("{}:{}", cost.item_id, cost.count.saturating_mul(amount)))
                .collect::<Vec<_>>();
            parts.sort();
            format!("specialShop|{}", parts.join(","))
        }
        ItemSource::Fishing { fish_id, spot_id } => format!("fishing|{fish_id}|{spot_id}"),
        ItemSource::Gathering => "gathering".to_string(),
    }
}

pub(super) fn source_display_groups(
    data: &CraftDataPackage,
    item_id: u32,
    sources: &[ItemSource],
    amount: u32,
) -> Vec<SourceDisplayGroup> {
    let mut ordered_keys = Vec::new();
    let mut groups: HashMap<String, SourceDisplayGroup> = HashMap::new();

    for (index, source) in sources.iter().enumerate() {
        let key = source_consumption_key(data, source, amount, item_id);
        if !groups.contains_key(&key) {
            ordered_keys.push(key.clone());
            groups.insert(
                key.clone(),
                SourceDisplayGroup {
                    key: key.clone(),
                    source: source.clone(),
                    indices: Vec::new(),
                    cost_label: source_cost_label(data, source, amount, Some(item_id)),
                    details: Vec::new(),
                },
            );
        }
        if let Some(group) = groups.get_mut(&key) {
            group.indices.push(index);
            group.details.push(source_info_label(source));
        }
    }

    let mut result = ordered_keys
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .collect::<Vec<_>>();
    result.sort_by_key(|group| source_priority(&group.source));
    result
}

pub(super) fn source_tone_class(source: Option<&ItemSource>, ignored: bool) -> &'static str {
    if ignored {
        return "border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground";
    }
    match source {
        Some(ItemSource::Gathering) => "border-l-emerald-200 bg-emerald-50/80",
        Some(ItemSource::Fishing { .. }) => "border-l-cyan-200 bg-cyan-50/80",
        Some(ItemSource::SpecialShop { .. }) => "border-l-[#d7c7ff] bg-[#f5efff]",
        Some(ItemSource::GilShop { .. }) => "border-l-amber-200 bg-amber-50/80",
        None => "border-l-border bg-background",
    }
}

pub(super) fn source_button_class(source: &ItemSource, active: bool) -> &'static str {
    if !active {
        return "border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground";
    }
    match source {
        ItemSource::Gathering => "border-emerald-200 bg-[#dff5e5] text-[#166534]",
        ItemSource::Fishing { .. } => "border-cyan-200 bg-[#d8f3fb] text-[#155e75]",
        ItemSource::SpecialShop { .. } => "border-[#bfa7ff] bg-[#e4d9ff] text-[#3b2778]",
        ItemSource::GilShop { .. } => "border-amber-200 bg-[#fff0bf] text-[#854d0e]",
    }
}

pub(super) fn market_button_class(active: bool) -> &'static str {
    if active {
        "border-[#93c5fd] bg-[#dbeafe] text-[#1d4ed8]"
    } else {
        "border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground"
    }
}

fn huiji_item_url(item_name: &str) -> String {
    format!(
        "https://ff14.huijiwiki.com/wiki/{}",
        urlencoding::encode(&format!("物品:{item_name}"))
    )
}

fn lodestone_item_search_url(item_name: &str) -> String {
    format!(
        "https://na.finalfantasyxiv.com/lodestone/playguide/db/search/?q={}",
        urlencoding::encode(item_name)
    )
}

fn xiv_market_item_url(item_id: u32) -> String {
    format!("https://azurice.github.io/xiv-market/#/item/{item_id}")
}

fn fish_cake_url(spot_id: u32, fish_id: u32) -> String {
    format!("https://fish.ffmomola.com/#/wiki/fishing/spot/{spot_id}/fish/{fish_id}")
}

pub(super) fn is_marketable(item: Option<&CraftItem>) -> bool {
    item.map(|item| item.item_search_category > 0)
        .unwrap_or(false)
}

fn exchange_group_key(source: &ItemSource) -> String {
    match source {
        ItemSource::SpecialShop { shop_name, costs } => format!(
            "{}|{}",
            shop_name,
            costs
                .iter()
                .map(|cost| cost.item_id.to_string())
                .collect::<Vec<_>>()
                .join("+")
        ),
        _ => String::new(),
    }
}

pub(super) fn leaf_tone_class(
    data: &CraftDataPackage,
    node: &CraftTreeNode,
    choices: &HashMap<u32, SourceChoice>,
) -> &'static str {
    let sources = data
        .sources
        .get(&node.item_id.to_string())
        .cloned()
        .unwrap_or_default();
    let choice = choices.get(&node.item_id);
    let marketable = is_marketable(get_item(data, node.item_id));
    let source = resolve_source(node.item_id, &sources, choices);

    if matches!(choice, Some(SourceChoice::Ignore)) {
        "border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground hover:bg-[#e7e5e4]"
    } else if marketable && matches!(choice, Some(SourceChoice::Market)) {
        "border-l-[#93c5fd] bg-[#eff6ff] hover:bg-[#dbeafe]"
    } else {
        match source {
            Some(ItemSource::GilShop { .. }) => {
                "border-l-amber-200 bg-amber-50/80 hover:bg-amber-100/70"
            }
            Some(ItemSource::SpecialShop { .. }) => {
                "border-l-[#d7c7ff] bg-[#f7f2ff] hover:bg-[#efe7ff]"
            }
            Some(ItemSource::Gathering | ItemSource::Fishing { .. }) => {
                "border-l-emerald-200 bg-emerald-50/80 hover:bg-emerald-100/70"
            }
            None => "border-l-border bg-background hover:bg-muted/35",
        }
    }
}

pub(super) fn build_material_plan(
    data: &CraftDataPackage,
    materials: &[MaterialSummary],
    choices: &HashMap<u32, SourceChoice>,
) -> MaterialPlan {
    let mut plan = MaterialPlan::empty();
    let mut exchange_groups: HashMap<String, ExchangePlanGroup> = HashMap::new();
    let mut exchange_order = Vec::new();
    let mut exchange_cost_maps: HashMap<String, HashMap<u32, u32>> = HashMap::new();

    for material in materials {
        let sources = data
            .sources
            .get(&material.item_id.to_string())
            .cloned()
            .unwrap_or_default();
        let item = get_item(data, material.item_id);
        let marketable = is_marketable(item);
        let choice = choices.get(&material.item_id).cloned();
        let source = resolve_source(material.item_id, &sources, choices).cloned();
        let base_entry = MaterialPlanEntry {
            item_id: material.item_id,
            amount: material.amount,
            sources,
            choice: choice.clone(),
            source: source.clone(),
            marketable,
            shop_name: None,
            gil: None,
            costs: None,
        };

        if matches!(choice, Some(SourceChoice::Ignore)) {
            plan.owned.push(base_entry);
            continue;
        }

        if marketable && matches!(choice, Some(SourceChoice::Market)) {
            plan.market.push(MaterialPlanEntry {
                source: None,
                ..base_entry
            });
            continue;
        }

        match source {
            Some(ItemSource::GilShop { shop_name }) => {
                let gil = get_item(data, material.item_id)
                    .map(|item| item.price_mid.saturating_mul(material.amount))
                    .unwrap_or(0);
                plan.gil_total = plan.gil_total.saturating_add(gil);
                plan.shops.push(MaterialPlanEntry {
                    shop_name: Some(shop_name),
                    gil: Some(gil),
                    ..base_entry
                });
            }
            Some(ItemSource::SpecialShop {
                ref shop_name,
                ref costs,
            }) => {
                let source = ItemSource::SpecialShop {
                    shop_name: shop_name.clone(),
                    costs: costs.clone(),
                };
                let key = exchange_group_key(&source);
                if !exchange_groups.contains_key(&key) {
                    exchange_order.push(key.clone());
                    exchange_groups.insert(
                        key.clone(),
                        ExchangePlanGroup {
                            key: key.clone(),
                            shop_name: shop_name.clone(),
                            costs: Vec::new(),
                            entries: Vec::new(),
                        },
                    );
                    exchange_cost_maps.insert(key.clone(), HashMap::new());
                }

                let entry_costs = costs
                    .iter()
                    .map(|cost| MaterialCost {
                        item_id: cost.item_id,
                        amount: cost.count.saturating_mul(material.amount),
                    })
                    .collect::<Vec<_>>();
                if let Some(cost_map) = exchange_cost_maps.get_mut(&key) {
                    for cost in &entry_costs {
                        let current = cost_map.entry(cost.item_id).or_default();
                        *current = current.saturating_add(cost.amount);
                    }
                }
                if let Some(group) = exchange_groups.get_mut(&key) {
                    group.entries.push(MaterialPlanEntry {
                        shop_name: Some(shop_name.clone()),
                        costs: Some(entry_costs),
                        ..base_entry
                    });
                }
            }
            Some(ItemSource::Gathering | ItemSource::Fishing { .. }) => {
                plan.gathering.push(base_entry);
            }
            None => plan.unknown.push(base_entry),
        }
    }

    plan.exchange_groups = exchange_order
        .into_iter()
        .filter_map(|key| {
            let mut group = exchange_groups.remove(&key)?;
            let mut costs = exchange_cost_maps
                .remove(&key)
                .unwrap_or_default()
                .into_iter()
                .map(|(item_id, amount)| MaterialCost { item_id, amount })
                .collect::<Vec<_>>();
            costs.sort_by_key(|cost| cost.item_id);
            group.costs = costs;
            Some(group)
        })
        .collect();
    plan
}

fn current_source_index(choice: Option<&SourceChoice>, sources: &[ItemSource]) -> Option<usize> {
    match choice {
        Some(SourceChoice::Index { index }) => Some(*index),
        Some(SourceChoice::Ignore | SourceChoice::Market) => None,
        None => default_source_index(sources),
    }
}

pub(super) fn market_item_ids_key(plan: &MaterialPlan) -> Option<String> {
    let mut ids = plan
        .market
        .iter()
        .map(|entry| entry.item_id)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    (!ids.is_empty()).then(|| ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
}

fn first_positive(values: &[Option<f64>]) -> Option<u32> {
    values
        .iter()
        .flatten()
        .find(|value| value.is_finite() && **value > 0.0)
        .map(|value| value.round() as u32)
}

fn normalize_market_quote(raw: UniversalisResult) -> Option<MarketQuote> {
    let min_listing_price = raw
        .nq
        .min_listing
        .as_ref()
        .and_then(|scope| scope.region.as_ref())
        .and_then(|region| region.price);
    let recent_purchase_price = raw
        .nq
        .recent_purchase
        .as_ref()
        .and_then(|scope| scope.region.as_ref())
        .and_then(|region| region.price);
    let average_sale_price = raw
        .nq
        .average_sale_price
        .as_ref()
        .and_then(|scope| scope.region.as_ref())
        .and_then(|region| region.price);
    let unit_price =
        first_positive(&[min_listing_price, recent_purchase_price, average_sale_price])?;
    let basis = if min_listing_price.is_some() {
        "最低挂单"
    } else if recent_purchase_price.is_some() {
        "近期成交"
    } else {
        "平均成交"
    };
    Some(MarketQuote {
        item_id: raw.item_id,
        unit_price,
        basis: basis.to_string(),
    })
}

pub(super) async fn fetch_market_quotes(
    item_ids_key: String,
) -> Result<HashMap<u32, MarketQuote>, String> {
    let item_ids = item_ids_key
        .split(',')
        .filter_map(|item_id| item_id.parse::<u32>().ok())
        .filter(|item_id| *item_id > 0)
        .collect::<Vec<_>>();
    let mut result = HashMap::new();

    for chunk in item_ids.chunks(100) {
        let ids = chunk
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "{UNIVERSALIS_BASE_URL}/api/v2/aggregated/{}/{}",
            urlencoding::encode(MARKET_WORLD_DC_REGION),
            ids
        );
        let response = Request::get(&url)
            .send()
            .await
            .map_err(|error| format!("Universalis {error}"))?;
        if !response.ok() {
            return Err(format!("Universalis {}", response.status()));
        }
        let json = response
            .json::<UniversalisAggregatedResponse>()
            .await
            .map_err(|error| format!("Universalis {error}"))?;
        for raw in json.results {
            if let Some(quote) = normalize_market_quote(raw) {
                result.insert(quote.item_id, quote);
            }
        }
    }

    Ok(result)
}

pub(super) fn market_cost(
    plan: &MaterialPlan,
    quotes: Option<&HashMap<u32, MarketQuote>>,
) -> MarketCost {
    let mut total = 0u32;
    let mut priced = 0usize;
    let mut missing = 0usize;
    for entry in &plan.market {
        if let Some(unit_price) = quotes
            .and_then(|quotes| quotes.get(&entry.item_id))
            .map(|q| q.unit_price)
        {
            total = total.saturating_add(unit_price.saturating_mul(entry.amount));
            priced += 1;
        } else {
            missing += 1;
        }
    }
    MarketCost {
        total,
        priced,
        missing,
    }
}

pub(super) fn market_meta(
    entry: &MaterialPlanEntry,
    quotes_state: Option<&Result<HashMap<u32, MarketQuote>, String>>,
    loading: bool,
) -> String {
    if loading {
        return format!("估价载入中 · {MARKET_WORLD_DC_REGION}");
    }
    match quotes_state {
        Some(Err(_)) => "估价失败".to_string(),
        Some(Ok(quotes)) => quotes
            .get(&entry.item_id)
            .map(|quote| {
                format!(
                    "{}G / 个 · {}G · {}",
                    format_integer(quote.unit_price as f64),
                    format_integer(quote.unit_price.saturating_mul(entry.amount) as f64),
                    quote.basis
                )
            })
            .unwrap_or_else(|| "暂无市场价格".to_string()),
        None => "暂无市场价格".to_string(),
    }
}

fn macro_action_name(data: &CraftDataPackage, action: &MacroAction) -> String {
    data.macro_action_names
        .get(&action.id)
        .cloned()
        .unwrap_or_else(|| action.id.clone())
}

fn format_macro_line(data: &CraftDataPackage, action: &MacroAction) -> String {
    format!(
        "/ac \"{}\" <wait.{}>",
        macro_action_name(data, action),
        action.wait_seconds
    )
}

fn macro_blocks(
    data: &CraftDataPackage,
    actions: &[MacroAction],
    with_notify: bool,
) -> Vec<Vec<String>> {
    let max_action_lines = if with_notify { 14 } else { 15 };
    let mut result = Vec::new();
    let mut index = 0;
    while index < actions.len() {
        let mut block = actions[index..actions.len().min(index + max_action_lines)]
            .iter()
            .map(|action| format_macro_line(data, action))
            .collect::<Vec<_>>();
        if with_notify {
            block.push(format!("/echo 宏 #{} 完成 <se.1>", result.len() + 1));
        }
        result.push(block);
        index += max_action_lines;
    }
    result
}

fn percent_label(value: u32, max: u32) -> String {
    if max == 0 {
        return "0%".to_string();
    }
    format!(
        "{}%",
        ((value as f64 / max as f64) * 100.0).round().min(100.0)
    )
}

fn copy_to_clipboard(text: String) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(&text);
    }
}

fn clamp_number(value: String, min: u32, max: Option<u32>) -> u32 {
    let mut parsed = value.parse::<u32>().unwrap_or(min).max(min);
    if let Some(max) = max {
        parsed = parsed.min(max);
    }
    parsed
}

#[component]
pub(super) fn ItemIcon(icon: u32, #[props(default = "md")] size: &'static str) -> Element {
    let urls = get_icon_urls(icon);
    let mut failed_urls = use_signal(HashSet::<String>::new);
    let src = urls
        .iter()
        .find(|url| !failed_urls().contains(*url))
        .cloned();
    let size_class = if size == "sm" { "h-5 w-5" } else { "h-7 w-7" };

    if let Some(src) = src {
        let current_src = src.clone();
        rsx! {
            img {
                src,
                alt: "",
                loading: "lazy",
                class: cx([size_class, "shrink-0 rounded border bg-muted object-cover"]),
                onerror: move |_| {
                    let mut failed = failed_urls();
                    failed.insert(current_src.clone());
                    failed_urls.set(failed);
                },
            }
        }
    } else {
        rsx! {
            div { class: cx([size_class, "rounded border bg-muted"]) }
        }
    }
}

#[component]
pub(super) fn ExternalItemLink(href: String, label: &'static str) -> Element {
    rsx! {
        a {
            href,
            target: "_blank",
            rel: "noopener noreferrer",
            class: "inline-flex h-7 items-center gap-1 rounded border bg-background px-2 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground",
            Icon { kind: IconKind::ExternalLink, class: "h-3 w-3 shrink-0" }
            "{label}"
        }
    }
}

#[component]
fn NumericField(
    label: &'static str,
    value: u32,
    #[props(default = 1)] min: u32,
    #[props(default = None)] max: Option<u32>,
    on_input: EventHandler<u32>,
) -> Element {
    rsx! {
        label { class: "grid gap-1 text-xs font-medium text-muted-foreground",
            "{label}"
            input {
                r#type: "number",
                min: "{min}",
                max: max.map(|value| value.to_string()),
                value: "{value}",
                class: input_class("h-8"),
                oninput: move |event| on_input.call(clamp_number(event.value(), min, max)),
            }
        }
    }
}

#[component]
fn ToggleField(label: &'static str, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "flex items-center gap-2 rounded-sm border bg-background/70 px-2 py-1.5 text-xs font-medium text-muted-foreground",
            input {
                r#type: "checkbox",
                checked,
                class: "h-3.5 w-3.5 accent-foreground",
                onchange: move |event| on_change.call(event.checked()),
            }
            "{label}"
        }
    }
}

#[component]
pub(super) fn ItemExternalLinks(
    item: Option<CraftItem>,
    item_id: u32,
    item_name: String,
    sources: Vec<ItemSource>,
) -> Element {
    let fishing = sources.iter().find_map(|source| match source {
        ItemSource::Fishing { fish_id, spot_id } => Some((*fish_id, *spot_id)),
        _ => None,
    });
    let marketable = item
        .as_ref()
        .map(|item| item.item_search_category > 0)
        .unwrap_or(false);

    rsx! {
        div { class: "flex flex-wrap gap-1.5",
            ExternalItemLink { href: huiji_item_url(&item_name), label: "灰机 Wiki" }
            ExternalItemLink { href: lodestone_item_search_url(&item_name), label: "Lodestone" }
            if marketable {
                ExternalItemLink { href: xiv_market_item_url(item_id), label: "xiv-market" }
            }
            if let Some((fish_id, spot_id)) = fishing {
                ExternalItemLink { href: fish_cake_url(spot_id, fish_id), label: "鱼糕" }
            }
        }
    }
}

#[component]
pub(super) fn SourceChoiceControls(
    data: Rc<CraftDataPackage>,
    item_id: u32,
    amount: u32,
    sources: Vec<ItemSource>,
    marketable: bool,
    choice: Option<SourceChoice>,
    on_choose: EventHandler<(u32, Option<SourceChoice>)>,
) -> Element {
    let ignored = matches!(choice, Some(SourceChoice::Ignore));
    let market = matches!(choice, Some(SourceChoice::Market));
    let source_index = current_source_index(choice.as_ref(), &sources);
    let groups = source_display_groups(&data, item_id, &sources, amount);

    rsx! {
        div { class: "flex flex-wrap gap-1.5",
            onclick: move |event| event.stop_propagation(),
            if marketable {
                button {
                    r#type: "button",
                    class: cx([
                        "inline-flex min-h-7 max-w-full items-center gap-1 rounded border px-2 py-1 text-left text-[11px] font-medium leading-snug transition-colors",
                        market_button_class(market),
                    ]),
                    title: format!("从市场购买（{MARKET_WORLD_DC_REGION}）"),
                    aria_label: "市场购买",
                    onclick: {
                        let on_choose = on_choose;
                        move |_| on_choose.call((item_id, if market { None } else { Some(SourceChoice::Market) }))
                    },
                    Icon { kind: IconKind::Coins, class: "h-3 w-3 shrink-0" }
                    "市场"
                }
            }

            for group in groups {
                {
                    let active = !ignored
                        && !market
                        && source_index
                            .map(|index| group.indices.contains(&index))
                            .unwrap_or(false);
                    let selected_index = group.indices.first().copied().unwrap_or(0);
                    rsx! {
                        button {
                            key: "{group.key}",
                            r#type: "button",
                            class: cx([
                                "inline-flex min-h-7 max-w-full items-center gap-1 rounded border px-2 py-1 text-left text-[11px] font-medium leading-snug transition-colors",
                                source_button_class(&group.source, active),
                            ]),
                            title: group.details.join("\n"),
                            onclick: {
                                let on_choose = on_choose;
                                move |_| on_choose.call((item_id, Some(SourceChoice::Index { index: selected_index })))
                            },
                            Icon { kind: source_icon_kind(&group.source), class: "h-3.5 w-3.5 shrink-0" }
                            span { "{source_label(&group.source)}" }
                            if let Some(cost_label) = &group.cost_label {
                                span {
                                    class: if active {
                                        "min-w-0 whitespace-normal break-words opacity-90"
                                    } else {
                                        "min-w-0 whitespace-normal break-words opacity-70"
                                    },
                                    "{cost_label}"
                                }
                            }
                        }
                    }
                }
            }

            if !marketable && sources.is_empty() {
                span { class: "inline-flex min-h-7 items-center rounded border bg-background/80 px-2 py-1 text-[11px] text-muted-foreground",
                    "无来源"
                }
            }
        }
    }
}

#[component]
pub(super) fn MaterialPlanRow(
    data: Rc<CraftDataPackage>,
    entry: MaterialPlanEntry,
    row_class: &'static str,
    #[props(default = None)] meta: Option<String>,
    #[props(default = false)] subdued: bool,
    on_choose: EventHandler<(u32, Option<SourceChoice>)>,
    on_inspect: EventHandler<MaterialPlanEntry>,
) -> Element {
    let item = get_item(&data, entry.item_id).cloned();
    let owned = matches!(entry.choice, Some(SourceChoice::Ignore));
    let container_class = cx([
        "rounded-sm border-l-2 px-2 py-2 text-sm cursor-pointer transition-[box-shadow,filter] hover:shadow-sm hover:brightness-[0.98]",
        row_class,
        if subdued { "opacity-75" } else { "" },
    ]);
    let own_button_class = if owned {
        "border-[#a8a29e] bg-[#e7e5e4] text-[#44403c]"
    } else {
        "border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground"
    };

    rsx! {
        div {
            class: container_class,
            onclick: {
                let entry = entry.clone();
                move |_| on_inspect.call(entry.clone())
            },
            div {
                class: "grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                role: "button",
                tabindex: "0",
                ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0), size: "sm" }
                div { class: "min-w-0",
                    div { class: "truncate font-medium", "{get_item_name(&data, entry.item_id)}" }
                    if let Some(meta) = meta {
                        div { class: "whitespace-normal break-words text-xs leading-snug text-muted-foreground", "{meta}" }
                    }
                }
                div { class: "flex items-center gap-1.5",
                    Badge { variant: BadgeVariant::Outline, class: "bg-background/70".to_string(),
                        "x{format_integer(entry.amount as f64)}"
                    }
                    button {
                        r#type: "button",
                        class: cx([
                            "flex h-7 w-7 items-center justify-center rounded-full border transition-colors",
                            own_button_class,
                        ]),
                        title: if owned { "取消已拥有" } else { "标记为已拥有" },
                        aria_label: if owned { "取消已拥有" } else { "标记为已拥有" },
                        onclick: {
                            let on_choose = on_choose;
                            move |event| {
                                event.stop_propagation();
                                on_choose.call((entry.item_id, if owned { None } else { Some(SourceChoice::Ignore) }));
                            }
                        },
                        Icon { kind: IconKind::CircleCheck, class: "h-3.5 w-3.5" }
                    }
                }
            }
            div { class: "mt-2 pl-7",
                SourceChoiceControls {
                    data,
                    item_id: entry.item_id,
                    amount: entry.amount,
                    sources: entry.sources.clone(),
                    marketable: entry.marketable,
                    choice: entry.choice.clone(),
                    on_choose,
                }
            }
        }
    }
}

#[component]
pub(super) fn SummaryItemRow(
    data: Rc<CraftDataPackage>,
    item_id: u32,
    amount: u32,
    row_class: &'static str,
    #[props(default = None)] meta: Option<String>,
    on_inspect: EventHandler<(u32, u32)>,
) -> Element {
    let item = get_item(&data, item_id).cloned();

    rsx! {
        div {
            class: cx([
                "mb-1 grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm border-l-2 px-2 py-1.5 text-sm cursor-pointer transition-shadow hover:shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                row_class,
            ]),
            role: "button",
            tabindex: "0",
            onclick: move |_| on_inspect.call((item_id, amount)),
            ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0), size: "sm" }
            div { class: "min-w-0",
                div { class: "truncate font-medium", "{get_item_name(&data, item_id)}" }
                if let Some(meta) = meta {
                    div { class: "whitespace-normal break-words text-xs leading-snug text-muted-foreground", "{meta}" }
                }
            }
            Badge { variant: BadgeVariant::Outline, class: "bg-background/70".to_string(),
                "x{format_integer(amount as f64)}"
            }
        }
    }
}

#[component]
pub(super) fn ExchangeGroupPanel(
    data: Rc<CraftDataPackage>,
    group: ExchangePlanGroup,
    on_choose: EventHandler<(u32, Option<SourceChoice>)>,
    on_inspect: EventHandler<MaterialPlanEntry>,
    on_inspect_item: EventHandler<(u32, u32)>,
) -> Element {
    rsx! {
        div { class: "overflow-hidden rounded-md border bg-[#fbf9ff]",
            div { class: "grid grid-cols-1 lg:grid-cols-[150px_minmax(0,1fr)] xl:grid-cols-1 2xl:grid-cols-[160px_minmax(0,1fr)]",
                div { class: "border-b bg-[#f0ebff] p-3 lg:border-b-0 lg:border-r xl:border-b xl:border-r-0 2xl:border-b-0 2xl:border-r",
                    div { class: "mb-2 min-w-0 break-words text-xs font-medium text-[#4c3290]", "{group.shop_name}" }
                    div { class: "space-y-1.5",
                        for cost in group.costs.clone() {
                            SummaryItemRow {
                                data: data.clone(),
                                item_id: cost.item_id,
                                amount: cost.amount,
                                row_class: "border-l-[#bfa7ff] bg-background/75",
                                on_inspect: on_inspect_item,
                            }
                        }
                    }
                }
                div { class: "divide-y",
                    for entry in group.entries.clone() {
                        {
                            let meta = entry.costs.as_ref().map(|costs| cost_list_label(&data, costs));
                            rsx! {
                                div { class: "p-2",
                                    MaterialPlanRow {
                                        data: data.clone(),
                                        entry,
                                        row_class: "border-l-[#d7c7ff] bg-[#f7f2ff]",
                                        meta,
                                        subdued: true,
                                        on_choose,
                                        on_inspect,
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
pub(super) fn NodeDetailDialog(
    data: Rc<CraftDataPackage>,
    target: DetailTarget,
    recipe: Option<CraftRecipe>,
    on_close: EventHandler<()>,
) -> Element {
    let item = get_item(&data, target.item_id).cloned();
    let sources = data
        .sources
        .get(&target.item_id.to_string())
        .cloned()
        .unwrap_or_default();
    let source_groups =
        source_display_groups(&data, target.item_id, &sources, target.amount_needed);
    let item_name = get_item_name(&data, target.item_id);

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            onclick: move |_| on_close.call(()),
            div {
                class: "max-h-[min(680px,calc(100vh-2rem))] w-full max-w-md overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-start gap-3 border-b p-4",
                    ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0) }
                    div { class: "min-w-0 flex-1",
                        div { class: "truncate text-base font-semibold", "{item_name}" }
                        div { class: "text-xs text-muted-foreground", "#{target.item_id}" }
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
                div { class: "max-h-[calc(100vh-8rem)] overflow-y-auto p-4",
                    div { class: "space-y-4",
                        ItemExternalLinks {
                            item,
                            item_id: target.item_id,
                            item_name: item_name.clone(),
                            sources: sources.clone(),
                        }

                        div { class: "grid grid-cols-2 gap-x-4 gap-y-2 text-sm",
                            div { class: "text-muted-foreground", "需求" }
                            div { class: "text-right font-medium", "x{format_integer(target.amount_needed as f64)}" }
                            if let Some(item) = get_item(&data, target.item_id) {
                                if item.price_low > 0 {
                                    div { class: "text-muted-foreground", "收购" }
                                    div { class: "text-right font-medium", "{format_integer(item.price_low as f64)}G" }
                                }
                            }
                            if let Some(recipe) = recipe.as_ref() {
                                div { class: "text-muted-foreground", "职业" }
                                div { class: "text-right font-medium", "{CRAFT_TYPE_NAMES[recipe.craft_type.min(7) as usize]}" }
                                div { class: "text-muted-foreground", "等级" }
                                div { class: "text-right font-medium", "{recipe_level_label(&data, recipe)}" }
                                div { class: "text-muted-foreground", "产出" }
                                div { class: "text-right font-medium", "x{recipe.result_amount}" }
                            }
                        }

                        if !source_groups.is_empty() {
                            div { class: "space-y-2",
                                div { class: "text-sm font-medium", "获取来源" }
                                for group in source_groups {
                                    div { class: cx(["flex items-start gap-2 rounded-sm border-l-2 p-2 text-sm", source_tone_class(Some(&group.source), false)]),
                                        div { class: "mt-0.5 text-muted-foreground",
                                            Icon { kind: source_icon_kind(&group.source), class: "h-3.5 w-3.5 shrink-0" }
                                        }
                                        div { class: "min-w-0 flex-1",
                                            div { class: "font-medium",
                                                "{source_label(&group.source)}"
                                                if let Some(cost_label) = group.cost_label.as_ref() {
                                                    span { class: "ml-1 text-muted-foreground", "{cost_label}" }
                                                }
                                            }
                                            div { class: "whitespace-normal break-words text-xs leading-snug text-muted-foreground",
                                                if group.details.len() > 1 {
                                                    ul { class: "list-disc space-y-0.5 pl-4",
                                                        for detail in group.details {
                                                            li { "{detail}" }
                                                        }
                                                    }
                                                } else if let Some(detail) = group.details.first() {
                                                    "{detail}"
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
        }
    }
}

#[component]
fn MacroSolverPanel(data: Rc<CraftDataPackage>, recipe: Option<CraftRecipe>) -> Element {
    let mut attrs = use_signal(|| DEFAULT_CRAFTER_ATTRIBUTES);
    let mut options = use_signal(|| DEFAULT_SOLVE_OPTIONS);
    let mut result = use_signal(|| None::<MacroSolveResult>);
    let mut error = use_signal(|| None::<String>);
    let mut solving = use_signal(|| false);
    let mut with_notify = use_signal(|| true);
    let mut copied_index = use_signal(|| None::<usize>);
    let mut last_recipe_id = use_signal(|| None::<u32>);

    let recipe_id = recipe.as_ref().map(|recipe| recipe.id);
    if last_recipe_id() != recipe_id {
        last_recipe_id.set(recipe_id);
        result.set(None);
        error.set(None);
        copied_index.set(None);
        solving.set(false);
        let mut next_options = options();
        next_options.target_quality = None;
        options.set(next_options);
    }

    let recipe_level = recipe.as_ref().and_then(|recipe| {
        data.recipe_levels
            .get(&recipe.recipe_level_table_id.to_string())
            .cloned()
    });
    let can_solve = recipe.is_some()
        && recipe_level
            .as_ref()
            .map(|level| level.progress_divider > 0 && level.quality_divider > 0)
            .unwrap_or(false);
    let blocks = result()
        .as_ref()
        .map(|result| macro_blocks(&data, &result.actions, with_notify()))
        .unwrap_or_default();

    rsx! {
        div { class: "min-h-0 flex-1 overflow-y-auto p-3",
            if let (Some(recipe), Some(level)) = (recipe.clone(), recipe_level.clone()) {
                div { class: "space-y-4",
                    div { class: "rounded-md border bg-background/70 p-3",
                        div { class: "mb-3 flex items-start justify-between gap-3",
                            div {
                                div { class: "text-sm font-semibold", "Raphael 宏求解" }
                                div { class: "mt-1 text-xs text-muted-foreground",
                                    "Lv.{level.class_job_level} · 耐久 {level.durability} · 难度 {format_integer(level.difficulty as f64)} · 品质 {format_integer(level.quality as f64)}"
                                }
                            }
                            Badge { variant: BadgeVariant::Outline, "实验" }
                        }

                        div { class: "grid grid-cols-2 gap-2",
                            NumericField {
                                label: "等级",
                                value: attrs().level,
                                min: 1,
                                max: Some(100),
                                on_input: move |value: u32| {
                                    let mut next = attrs();
                                    next.level = value;
                                    attrs.set(next);
                                },
                            }
                            NumericField {
                                label: "作业精度",
                                value: attrs().craftsmanship,
                                min: 1,
                                on_input: move |value: u32| {
                                    let mut next = attrs();
                                    next.craftsmanship = value;
                                    attrs.set(next);
                                },
                            }
                            NumericField {
                                label: "加工精度",
                                value: attrs().control,
                                min: 1,
                                on_input: move |value: u32| {
                                    let mut next = attrs();
                                    next.control = value;
                                    attrs.set(next);
                                },
                            }
                            NumericField {
                                label: "制作力",
                                value: attrs().craft_points,
                                min: 1,
                                on_input: move |value: u32| {
                                    let mut next = attrs();
                                    next.craft_points = value;
                                    attrs.set(next);
                                },
                            }
                            NumericField {
                                label: "目标品质",
                                value: options().target_quality.unwrap_or(level.quality),
                                min: 1,
                                max: Some(level.quality),
                                on_input: move |value: u32| {
                                    let mut next = options();
                                    next.target_quality = Some(value.clamp(1, level.quality));
                                    options.set(next);
                                },
                            }
                            NumericField {
                                label: "宇宙稳手次数",
                                value: u32::from(options().stellar_steady_hand_charges),
                                min: 0,
                                max: Some(9),
                                on_input: move |value: u32| {
                                    let mut next = options();
                                    next.stellar_steady_hand_charges = value.min(9) as u8;
                                    options.set(next);
                                },
                            }
                        }

                        div { class: "mt-3 grid grid-cols-1 gap-2 2xl:grid-cols-2",
                            ToggleField {
                                label: "使用掌握",
                                checked: options().use_manipulation,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.use_manipulation = value;
                                    options.set(next);
                                },
                            }
                            ToggleField {
                                label: "允许工匠的神速技巧",
                                checked: options().use_trained_eye,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.use_trained_eye = value;
                                    options.set(next);
                                },
                            }
                            ToggleField {
                                label: "允许专心致志",
                                checked: options().use_heart_and_soul,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.use_heart_and_soul = value;
                                    options.set(next);
                                },
                            }
                            ToggleField {
                                label: "允许高速改革",
                                checked: options().use_quick_innovation,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.use_quick_innovation = value;
                                    options.set(next);
                                },
                            }
                            ToggleField {
                                label: "后置作业技能",
                                checked: options().backload_progress,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.backload_progress = value;
                                    options.set(next);
                                },
                            }
                            ToggleField {
                                label: "防黑球",
                                checked: options().adversarial,
                                on_change: move |value| {
                                    let mut next = options();
                                    next.adversarial = value;
                                    options.set(next);
                                },
                            }
                        }

                        div { class: "mt-3 flex items-center justify-between gap-2",
                            label { class: "flex items-center gap-2 text-xs text-muted-foreground",
                                input {
                                    r#type: "checkbox",
                                    checked: with_notify(),
                                    class: "h-3.5 w-3.5 accent-foreground",
                                    onchange: move |event| with_notify.set(event.checked()),
                                }
                                "宏结束提示"
                            }
                            Button {
                                size: ButtonSize::Sm,
                                variant: ButtonVariant::Primary,
                                disabled: !can_solve || solving(),
                                onclick: {
                                    let recipe = recipe.clone();
                                    let level = level.clone();
                                    move |_| {
                                        let recipe = recipe.clone();
                                        let level = level.clone();
                                        let next_attrs = attrs();
                                        let next_options = options();
                                        solving.set(true);
                                        error.set(None);
                                        result.set(None);
                                        copied_index.set(None);
                                        spawn_local(async move {
                                            let solve_result = solve_raphael_macro(&recipe, &level, &next_attrs, &next_options);
                                            match solve_result {
                                                Ok(next) => result.set(Some(next)),
                                                Err(message) => error.set(Some(message)),
                                            }
                                            solving.set(false);
                                        });
                                    }
                                },
                                if solving() {
                                    Icon { kind: IconKind::LoaderCircle, class: "h-3.5 w-3.5 animate-spin" }
                                    "求解中"
                                } else {
                                    Icon { kind: IconKind::Sparkles, class: "h-3.5 w-3.5" }
                                    "开始求解"
                                }
                            }
                        }

                        if !can_solve {
                            div { class: "mt-3 rounded-sm border border-amber-200 bg-amber-50 px-2 py-1.5 text-xs text-amber-800",
                                "当前数据缺少求解公式字段，请重新运行数据导出后再求解。"
                            }
                        }
                        if let Some(message) = error() {
                            div { class: "mt-3 rounded-sm border border-red-200 bg-red-50 px-2 py-1.5 text-xs text-red-700",
                                "{message}"
                            }
                        }
                    }

                    if let Some(value) = result() {
                        div { class: "space-y-3",
                            div { class: "flex flex-wrap gap-2",
                                Badge { variant: BadgeVariant::Outline, "步骤 {value.steps}" }
                                Badge { variant: BadgeVariant::Outline, "耗时 {value.duration_seconds}s" }
                                Badge { variant: BadgeVariant::Outline, "宏 {blocks.len()}" }
                            }
                            div { class: "grid grid-cols-2 gap-2",
                                div { class: "rounded-md border bg-background p-2",
                                    div { class: "mb-1 flex items-center justify-between gap-2 text-xs",
                                        span { class: "text-muted-foreground", "进度" }
                                        span { class: "font-medium", "{format_integer(value.final_progress as f64)} / {format_integer(value.max_progress as f64)}" }
                                    }
                                    div { class: "h-1.5 overflow-hidden rounded-full bg-muted",
                                        div { class: "h-full bg-emerald-500", style: "width: {percent_label(value.final_progress, value.max_progress)};" }
                                    }
                                }
                                div { class: "rounded-md border bg-background p-2",
                                    div { class: "mb-1 flex items-center justify-between gap-2 text-xs",
                                        span { class: "text-muted-foreground", "品质" }
                                        span { class: "font-medium", "{format_integer(value.final_quality as f64)} / {format_integer(value.max_quality as f64)}" }
                                    }
                                    div { class: "h-1.5 overflow-hidden rounded-full bg-muted",
                                        div { class: "h-full bg-cyan-500", style: "width: {percent_label(value.final_quality, value.max_quality)};" }
                                    }
                                    if value.target_quality < value.max_quality {
                                        div { class: "mt-1 text-[11px] text-muted-foreground",
                                            "目标 {format_integer(value.target_quality as f64)}"
                                        }
                                    }
                                }
                                div { class: "rounded-md border bg-background p-2 text-xs",
                                    div { class: "text-muted-foreground", "剩余耐久" }
                                    div { class: "mt-1 font-medium", "{format_integer(value.final_durability as f64)}" }
                                }
                                div { class: "rounded-md border bg-background p-2 text-xs",
                                    div { class: "text-muted-foreground", "剩余 CP" }
                                    div { class: "mt-1 font-medium", "{format_integer(value.final_cp as f64)}" }
                                }
                            }
                            for (index, lines) in blocks.iter().enumerate() {
                                div { class: "overflow-hidden rounded-md border bg-background",
                                    div { class: "flex items-center justify-between border-b px-3 py-2",
                                        div { class: "text-sm font-semibold", "宏 #{index + 1}" }
                                        Button {
                                            size: ButtonSize::Sm,
                                            variant: ButtonVariant::Outline,
                                            title: Some(format!("复制宏 #{}", index + 1)),
                                            onclick: {
                                                let lines = lines.clone();
                                                move |_| {
                                                    copy_to_clipboard(lines.join("\r\n"));
                                                    copied_index.set(Some(index));
                                                }
                                            },
                                            Icon { kind: IconKind::Copy, class: "h-3.5 w-3.5" }
                                            if copied_index() == Some(index) {
                                                "已复制"
                                            } else {
                                                "复制"
                                            }
                                        }
                                    }
                                    pre { class: "max-h-64 overflow-auto whitespace-pre-wrap p-3 font-mono text-xs leading-relaxed",
                                        "{lines.join(\"\\n\")}"
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                EmptyState {
                    icon: rsx! { Icon { kind: IconKind::Hammer, class: "h-6 w-6" } },
                    title: "暂无配方".to_string(),
                    description: "选择一个配方后可以尝试生成生产宏".to_string(),
                }
            }
        }
    }
}

#[component]
fn TreeNodeView(
    data: Rc<CraftDataPackage>,
    node: CraftTreeNode,
    depth: u32,
    collapsed: HashSet<String>,
    source_choices: HashMap<u32, SourceChoice>,
    on_toggle: EventHandler<String>,
    on_select: EventHandler<CraftTreeNode>,
) -> Element {
    let item = get_item(&data, node.item_id).cloned();
    let key = collapse_key(node.item_id, depth);
    let is_collapsed = collapsed.contains(&key);
    let is_craftable = !node.children.is_empty();
    let counts_as_leaf = !is_craftable || is_collapsed;
    let tone_class = if counts_as_leaf {
        leaf_tone_class(&data, &node, &source_choices)
    } else {
        "border-l-transparent bg-background hover:bg-accent/70"
    };
    let padding_left = format!("{}px", 8 + depth * 18);

    rsx! {
        div {
            div {
                class: cx([
                    "group relative cursor-pointer rounded-sm border-l-2 px-2 py-1 text-sm transition-colors",
                    tone_class,
                ]),
                style: "padding-left: {padding_left};",
                onclick: {
                    let node = node.clone();
                    move |_| on_select.call(node.clone())
                },
                div { class: "grid grid-cols-[1.25rem_1.5rem_minmax(0,1fr)_auto] items-center gap-2",
                    button {
                        r#type: "button",
                        class: "flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-background",
                        aria_label: if is_collapsed { "展开" } else { "折叠" },
                        onclick: {
                            let key = key.clone();
                            move |event| {
                                event.stop_propagation();
                                if is_craftable {
                                    on_toggle.call(key.clone());
                                }
                            }
                        },
                        if is_craftable {
                            Icon {
                                kind: if is_collapsed { IconKind::ChevronRight } else { IconKind::ChevronDown },
                                class: "h-4 w-4"
                            }
                        } else {
                            span { class: "h-1 w-1 rounded-full bg-muted-foreground/40" }
                        }
                    }

                    ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0), size: "sm" }

                    div { class: "min-w-0",
                        div { class: "truncate font-medium", "{get_item_name(&data, node.item_id)}" }
                        if let Some(recipe) = node.recipe.as_ref() {
                            div { class: "truncate text-xs text-muted-foreground",
                                "{CRAFT_TYPE_ABBRS[recipe.craft_type.min(7) as usize]} · {recipe_level_label(&data, recipe)}"
                            }
                        }
                    }

                    div { class: "flex items-center gap-1",
                        if counts_as_leaf {
                            Badge { variant: BadgeVariant::Outline, class: "bg-background/70".to_string(), "叶" }
                        }
                        Badge { variant: BadgeVariant::Outline, class: "bg-background/70".to_string(),
                            "x{format_integer(node.amount_needed as f64)}"
                        }
                    }
                }
            }

            if is_craftable && !is_collapsed {
                for child in node.children.clone() {
                    TreeNodeView {
                        data: data.clone(),
                        node: child,
                        depth: depth + 1,
                        collapsed: collapsed.clone(),
                        source_choices: source_choices.clone(),
                        on_toggle,
                        on_select,
                    }
                }
            }
        }
    }
}

fn read_search_param(name: &str) -> Option<String> {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    let query = hash.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == name {
            return urlencoding::decode(value)
                .ok()
                .map(|value| value.into_owned());
        }
    }
    None
}

fn parse_craft_type_param(value: Option<String>) -> Option<u32> {
    let parsed = value?.parse::<u32>().ok()?;
    (parsed < CRAFT_TYPE_ABBRS.len() as u32).then_some(parsed)
}

fn parse_recipe_param(value: Option<String>) -> Option<u32> {
    let parsed = value?.parse::<u32>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn write_search_params(query: &str, craft_type: Option<u32>, recipe: Option<u32>) {
    let mut pairs = Vec::new();
    if !query.is_empty() {
        pairs.push(format!("q={}", urlencoding::encode(query)));
    }
    if let Some(craft_type) = craft_type {
        pairs.push(format!("type={craft_type}"));
    }
    if let Some(recipe) = recipe {
        pairs.push(format!("recipe={recipe}"));
    }
    let next_hash = if pairs.is_empty() {
        "/crafting".to_string()
    } else {
        format!("/crafting?{}", pairs.join("&"))
    };
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&next_hash);
    }
}

#[component]
pub fn CraftingPage() -> Element {
    let craft_data = use_resource(load_craft_data);
    let mut query = use_signal(|| read_search_param("q").unwrap_or_default());
    let mut craft_type = use_signal(|| parse_craft_type_param(read_search_param("type")));
    let mut selected_recipe_id = use_signal(|| parse_recipe_param(read_search_param("recipe")));
    let mut recipe_linked = use_signal(|| selected_recipe_id().is_some());
    let mut detail_target = use_signal(|| None::<DetailTarget>);
    let mut collapsed = use_signal(HashSet::<String>::new);
    let mut source_choices = use_signal(HashMap::<u32, SourceChoice>::new);
    let mut side_tab = use_signal(|| "materials");
    let mut market_quotes_key = use_signal(|| None::<String>);
    let mut market_quotes =
        use_signal(|| None::<(String, Result<HashMap<u32, MarketQuote>, String>)>);
    let mut market_loading = use_signal(|| false);

    let data = craft_data
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let engine = data
        .as_ref()
        .map(|data| create_craft_data_engine(data.clone()));
    let recipes = engine
        .as_ref()
        .map(|engine| craftable_recipes(engine, craft_type(), &query(), 300))
        .unwrap_or_default();
    let selected_recipe = selected_recipe_id()
        .and_then(|id| recipes.iter().find(|recipe| recipe.id == id).cloned())
        .or_else(|| recipes.first().cloned());

    if let Some(recipe) = selected_recipe.as_ref() {
        if selected_recipe_id() != Some(recipe.id) {
            selected_recipe_id.set(Some(recipe.id));
        }
    } else if selected_recipe_id().is_some() {
        selected_recipe_id.set(None);
    }

    let tree = selected_recipe.as_ref().and_then(|recipe| {
        engine
            .as_ref()
            .map(|engine| build_tree(engine, recipe.result_item_id, 1))
    });
    let materials = tree
        .as_ref()
        .map(|tree| summarize_materials(tree, &collapsed()))
        .unwrap_or_default();
    let material_plan = data
        .as_ref()
        .map(|data| build_material_plan(data, &materials, &source_choices()))
        .unwrap_or_else(MaterialPlan::empty);
    let next_market_key = market_item_ids_key(&material_plan);
    if market_quotes_key() != next_market_key {
        market_quotes_key.set(next_market_key.clone());
        market_quotes.set(None);
        if let Some(key) = next_market_key {
            market_loading.set(true);
            spawn_local(async move {
                let result = fetch_market_quotes(key.clone()).await;
                market_quotes.set(Some((key, result)));
                market_loading.set(false);
            });
        } else {
            market_loading.set(false);
        }
    }
    let quotes_result = market_quotes()
        .and_then(|(key, result)| (Some(key) == market_quotes_key()).then_some(result));
    let market_cost_value = market_cost(
        &material_plan,
        quotes_result
            .as_ref()
            .and_then(|result| result.as_ref().ok()),
    );

    let mut select_recipe = move |recipe: CraftRecipe| {
        selected_recipe_id.set(Some(recipe.id));
        recipe_linked.set(true);
        detail_target.set(None);
        collapsed.set(HashSet::new());
        source_choices.set(HashMap::new());
        write_search_params(&query(), craft_type(), Some(recipe.id));
    };
    let toggle_collapsed = move |key: String| {
        let mut next = collapsed();
        if next.contains(&key) {
            next.remove(&key);
        } else {
            next.insert(key);
        }
        collapsed.set(next);
    };
    let choose_source = move |(item_id, choice): (u32, Option<SourceChoice>)| {
        let mut next = source_choices();
        if let Some(choice) = choice {
            next.insert(item_id, choice);
        } else {
            next.remove(&item_id);
        }
        source_choices.set(next);
    };
    let inspect_node = move |node: CraftTreeNode| {
        detail_target.set(Some(DetailTarget {
            item_id: node.item_id,
            amount_needed: node.amount_needed,
            recipe: node.recipe,
        }));
    };
    let inspect_entry = move |entry: MaterialPlanEntry| {
        detail_target.set(Some(DetailTarget {
            item_id: entry.item_id,
            amount_needed: entry.amount,
            recipe: None,
        }));
    };
    let inspect_item = move |(item_id, amount_needed): (u32, u32)| {
        detail_target.set(Some(DetailTarget {
            item_id,
            amount_needed,
            recipe: None,
        }));
    };
    let detail_recipe = detail_target().and_then(|target| {
        target.recipe.or_else(|| {
            engine
                .as_ref()
                .map(|engine| build_tree(engine, target.item_id, target.amount_needed).recipe)
                .unwrap_or(None)
        })
    });

    rsx! {
        div { class: "flex min-h-screen flex-col lg:h-screen lg:min-h-0 lg:overflow-hidden",
            div { class: "shrink-0 border-b bg-background px-4 py-4 sm:px-6 lg:px-8",
                div { class: "mx-auto flex max-w-[1600px] flex-col gap-3 xl:flex-row xl:items-end xl:justify-between",
                    div {
                        div { class: "text-sm text-muted-foreground", "工具 / 合成检索" }
                        h1 { class: "text-2xl font-semibold", "合成检索" }
                    }
                    if let Some(data) = data.as_ref() {
                        div { class: "flex flex-wrap gap-2 text-xs text-muted-foreground",
                            Badge { variant: BadgeVariant::Outline, "配方 {format_integer(data.counts.recipes as f64)}" }
                            Badge { variant: BadgeVariant::Outline, "物品 {format_integer(data.counts.items as f64)}" }
                            Badge { variant: BadgeVariant::Outline, "来源 {format_integer(data.counts.sources as f64)}" }
                        }
                    }
                }
            }

            div { class: "grid w-full flex-1 lg:min-h-0 lg:grid-cols-[300px_minmax(0,1fr)] lg:grid-rows-[minmax(0,1fr)_320px] xl:grid-cols-[300px_minmax(0,1fr)_400px] xl:grid-rows-1 2xl:grid-cols-[320px_minmax(0,1fr)_460px]",
                aside { class: "flex h-[340px] flex-col overflow-hidden border-b bg-card sm:h-[380px] lg:row-span-2 lg:h-auto lg:min-h-0 lg:border-b-0 lg:border-r xl:row-span-1",
                    div { class: "border-b p-3",
                        div { class: "relative",
                            Icon { kind: IconKind::Search, class: "absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                            input {
                                class: input_class("pl-9 pr-9"),
                                value: "{query}",
                                placeholder: "搜索物品或 ID",
                                oninput: move |event| {
                                    let value = event.value();
                                    query.set(value.clone());
                                    recipe_linked.set(false);
                                    write_search_params(&value, craft_type(), None);
                                },
                            }
                            if !query().is_empty() {
                                button {
                                    r#type: "button",
                                    class: "absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent",
                                    aria_label: "清除搜索",
                                    title: "清除搜索",
                                    onclick: move |_| {
                                        query.set(String::new());
                                        recipe_linked.set(false);
                                        write_search_params("", craft_type(), None);
                                    },
                                    Icon { kind: IconKind::X, class: "h-4 w-4" }
                                }
                            }
                        }

                        div { class: "mt-3 flex flex-wrap gap-1.5",
                            Button {
                                size: ButtonSize::Sm,
                                variant: if craft_type().is_none() { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                onclick: move |_| {
                                    craft_type.set(None);
                                    recipe_linked.set(false);
                                    write_search_params(&query(), None, None);
                                },
                                "全部"
                            }
                            for (index, label) in CRAFT_TYPE_ABBRS.iter().enumerate() {
                                Button {
                                    size: ButtonSize::Sm,
                                    variant: if craft_type() == Some(index as u32) { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                    onclick: move |_| {
                                        craft_type.set(Some(index as u32));
                                        recipe_linked.set(false);
                                        write_search_params(&query(), Some(index as u32), None);
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }

                    div { class: "min-h-0 flex-1 overflow-y-auto p-2",
                        match data.as_ref() {
                            Some(data) => {
                                if recipes.is_empty() {
                                    rsx! {
                                        EmptyState {
                                            icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                            title: "没有匹配的配方".to_string(),
                                        }
                                    }
                                } else {
                                    rsx! {
                                        for recipe in recipes.clone() {
                                            {
                                                let item = get_item(data, recipe.result_item_id).cloned();
                                                let active = selected_recipe.as_ref().map(|selected| selected.id) == Some(recipe.id);
                                                let button_class = if active {
                                                    "mb-1 grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md bg-accent px-2 py-2 text-left text-sm text-foreground transition-colors"
                                                } else {
                                                    "mb-1 grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors hover:bg-accent/70"
                                                };
                                                rsx! {
                                                    button {
                                                        key: "{recipe.id}",
                                                        r#type: "button",
                                                        class: button_class,
                                                        onclick: {
                                                            let recipe = recipe.clone();
                                                            move |_| select_recipe(recipe.clone())
                                                        },
                                                        ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0) }
                                                        div { class: "min-w-0",
                                                            div { class: "truncate font-medium", "{get_item_name(data, recipe.result_item_id)}" }
                                                            div { class: "truncate text-xs text-muted-foreground",
                                                                "{CRAFT_TYPE_NAMES[recipe.craft_type.min(7) as usize]} · {recipe_level_label(data, &recipe)}"
                                                            }
                                                        }
                                                        Badge { variant: BadgeVariant::Outline, "#{recipe.result_item_id}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            None => rsx! {
                                div { class: "space-y-2 p-2",
                                    for _ in 0..12 {
                                        div { class: "h-12 rounded-md bg-muted" }
                                    }
                                }
                            },
                        }
                    }
                }

                section { class: "min-h-[520px] overflow-hidden bg-background lg:min-h-0",
                    if let (Some(data), Some(root), Some(recipe)) = (data.as_ref(), tree.as_ref(), selected_recipe.as_ref()) {
                        {
                            let item = get_item(data, recipe.result_item_id).cloned();
                            rsx! {
                                div { class: "flex h-full min-h-[520px] flex-col lg:min-h-0",
                                    div { class: "flex items-center gap-3 border-b p-4",
                                        ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0) }
                                        div { class: "min-w-0 flex-1",
                                            div { class: "truncate text-base font-semibold", "{get_item_name(data, recipe.result_item_id)}" }
                                            div { class: "text-sm text-muted-foreground",
                                                "{CRAFT_TYPE_NAMES[recipe.craft_type.min(7) as usize]} · {recipe_level_label(data, recipe)}"
                                            }
                                        }
                                        Badge { variant: BadgeVariant::Secondary, "x{recipe.result_amount}" }
                                    }

                                    div { class: "min-h-0 flex-1 overflow-y-auto p-3",
                                        TreeNodeView {
                                            data: data.clone(),
                                            node: root.clone(),
                                            depth: 0,
                                            collapsed: collapsed(),
                                            source_choices: source_choices(),
                                            on_toggle: EventHandler::new(toggle_collapsed),
                                            on_select: EventHandler::new(inspect_node),
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                            title: "合成数据未载入".to_string(),
                            description: "运行 update-craft-data 后刷新页面".to_string(),
                        }
                    }
                }

                aside { class: "flex min-h-[420px] flex-col overflow-hidden border-t bg-card lg:col-start-2 lg:row-start-2 lg:min-h-0 xl:col-start-auto xl:row-start-auto xl:border-l xl:border-t-0",
                    div { class: "shrink-0 border-b p-3",
                        div { class: "grid grid-cols-2 gap-1 rounded-md bg-muted p-1",
                            button {
                                r#type: "button",
                                class: if side_tab() == "materials" {
                                    "flex h-8 items-center justify-center gap-1.5 rounded bg-background text-sm font-medium text-foreground shadow-sm transition-colors"
                                } else {
                                    "flex h-8 items-center justify-center gap-1.5 rounded text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
                                },
                                onclick: move |_| side_tab.set("materials"),
                                Icon { kind: IconKind::Info, class: "h-3.5 w-3.5" }
                                "材料"
                            }
                            button {
                                r#type: "button",
                                class: if side_tab() == "macro" {
                                    "flex h-8 items-center justify-center gap-1.5 rounded bg-background text-sm font-medium text-foreground shadow-sm transition-colors"
                                } else {
                                    "flex h-8 items-center justify-center gap-1.5 rounded text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
                                },
                                onclick: move |_| side_tab.set("macro"),
                                Icon { kind: IconKind::Hammer, class: "h-3.5 w-3.5" }
                                "宏求解"
                            }
                        }

                        if side_tab() == "materials" {
                            div { class: "mt-3 flex flex-wrap gap-2",
                                Badge { variant: BadgeVariant::Outline, "叶子 {materials.len()}" }
                                if !material_plan.gathering.is_empty() {
                                    Badge { variant: BadgeVariant::Success,
                                        Icon { kind: IconKind::Leaf, class: "mr-1 h-3 w-3" }
                                        "采集 {material_plan.gathering.len()}"
                                    }
                                }
                                if !material_plan.exchange_groups.is_empty() {
                                    Badge { variant: BadgeVariant::Outline,
                                        Icon { kind: IconKind::Shuffle, class: "mr-1 h-3 w-3" }
                                        "兑换 {material_plan.exchange_groups.len()}"
                                    }
                                }
                                if material_plan.gil_total > 0 {
                                    Badge { variant: BadgeVariant::Warning,
                                        Icon { kind: IconKind::Coins, class: "mr-1 h-3 w-3" }
                                        "商店 {format_integer(material_plan.gil_total as f64)}G"
                                    }
                                }
                                if !material_plan.market.is_empty() {
                                    Badge { variant: BadgeVariant::Outline,
                                        Icon { kind: IconKind::Coins, class: "mr-1 h-3 w-3" }
                                        "市场 {material_plan.market.len()}"
                                    }
                                }
                                if market_cost_value.total > 0 {
                                    Badge { variant: BadgeVariant::Outline, "估 {format_integer(market_cost_value.total as f64)}G" }
                                }
                                if !material_plan.owned.is_empty() {
                                    Badge { variant: BadgeVariant::Outline,
                                        Icon { kind: IconKind::CircleCheck, class: "mr-1 h-3 w-3" }
                                        "已拥有 {material_plan.owned.len()}"
                                    }
                                }
                            }
                        }
                    }

                    if side_tab() == "macro" {
                        if let Some(data) = data.as_ref() {
                            MacroSolverPanel {
                                data: data.clone(),
                                recipe: selected_recipe.clone(),
                            }
                        }
                    } else {
                        section { class: "flex min-h-0 flex-1 flex-col overflow-hidden",
                            div { class: "min-h-0 flex-1 overflow-y-auto p-3",
                                if let Some(data) = data.as_ref() {
                                    div { class: "space-y-4",
                                        if !material_plan.exchange_groups.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                                    Icon { kind: IconKind::Shuffle, class: "h-4 w-4 text-[#3b2778]" }
                                                    "兑换"
                                                }
                                                div { class: "space-y-2",
                                                    for group in material_plan.exchange_groups.clone() {
                                                        ExchangeGroupPanel {
                                                            data: data.clone(),
                                                            group,
                                                            on_choose: EventHandler::new(choose_source),
                                                            on_inspect: EventHandler::new(inspect_entry),
                                                            on_inspect_item: EventHandler::new(inspect_item),
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !material_plan.gathering.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                                    Icon { kind: IconKind::Leaf, class: "h-4 w-4 text-emerald-700" }
                                                    "采集清单"
                                                }
                                                div { class: "space-y-2",
                                                    for entry in material_plan.gathering.clone() {
                                                        MaterialPlanRow {
                                                            data: data.clone(),
                                                            entry,
                                                            row_class: "border-l-emerald-200 bg-emerald-50/80",
                                                            on_choose: EventHandler::new(choose_source),
                                                            on_inspect: EventHandler::new(inspect_entry),
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !material_plan.shops.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                                    Icon { kind: IconKind::Coins, class: "h-4 w-4 text-amber-700" }
                                                    "商店购买"
                                                }
                                                div { class: "space-y-2",
                                                    for entry in material_plan.shops.clone() {
                                                        {
                                                            let meta = format!(
                                                                "{}{}",
                                                                entry.shop_name.clone().unwrap_or_default(),
                                                                entry.gil.map(|gil| format!(" · {}G", format_integer(gil as f64))).unwrap_or_default()
                                                            );
                                                            rsx! {
                                                                MaterialPlanRow {
                                                                    data: data.clone(),
                                                                    entry,
                                                                    row_class: "border-l-amber-200 bg-amber-50/80",
                                                                    meta,
                                                                    on_choose: EventHandler::new(choose_source),
                                                                    on_inspect: EventHandler::new(inspect_entry),
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !material_plan.market.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center justify-between gap-2 text-sm font-semibold",
                                                    div { class: "flex items-center gap-2",
                                                        Icon { kind: IconKind::Coins, class: "h-4 w-4 text-[#1d4ed8]" }
                                                        "市场购买"
                                                    }
                                                    div { class: "text-xs font-medium text-muted-foreground",
                                                        if market_loading() {
                                                            "估价载入中"
                                                        } else if market_cost_value.total > 0 {
                                                            "估 {format_integer(market_cost_value.total as f64)}G"
                                                        } else {
                                                            "区域 {MARKET_WORLD_DC_REGION}"
                                                        }
                                                    }
                                                }
                                                div { class: "space-y-2",
                                                    for entry in material_plan.market.clone() {
                                                        MaterialPlanRow {
                                                            data: data.clone(),
                                                            meta: market_meta(&entry, quotes_result.as_ref(), market_loading()),
                                                            entry,
                                                            row_class: "border-l-[#93c5fd] bg-[#eff6ff]",
                                                            on_choose: EventHandler::new(choose_source),
                                                            on_inspect: EventHandler::new(inspect_entry),
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !material_plan.unknown.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                                    Icon { kind: IconKind::PackageSearch, class: "h-4 w-4 text-muted-foreground" }
                                                    "未安排"
                                                }
                                                div { class: "space-y-2",
                                                    for entry in material_plan.unknown.clone() {
                                                        MaterialPlanRow {
                                                            data: data.clone(),
                                                            entry,
                                                            row_class: "border-l-border bg-background",
                                                            on_choose: EventHandler::new(choose_source),
                                                            on_inspect: EventHandler::new(inspect_entry),
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !material_plan.owned.is_empty() {
                                            section {
                                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold text-muted-foreground",
                                                    Icon { kind: IconKind::CircleCheck, class: "h-4 w-4" }
                                                    "已拥有"
                                                }
                                                div { class: "space-y-2",
                                                    for entry in material_plan.owned.clone() {
                                                        MaterialPlanRow {
                                                            data: data.clone(),
                                                            entry,
                                                            row_class: "border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground",
                                                            on_choose: EventHandler::new(choose_source),
                                                            on_inspect: EventHandler::new(inspect_entry),
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if material_plan.is_empty() {
                                            EmptyState {
                                                icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                                title: "暂无材料".to_string(),
                                                description: "选择一个配方后会在这里生成叶子材料清单".to_string(),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let (Some(data), Some(target)) = (data.as_ref(), detail_target()) {
                NodeDetailDialog {
                    data: data.clone(),
                    target,
                    recipe: detail_recipe,
                    on_close: EventHandler::new(move |_| detail_target.set(None)),
                }
            }
        }
    }
}
