use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::crafting::{
    DetailTarget, ExchangeGroupPanel, ItemIcon, MaterialPlanEntry, MaterialPlanRow,
    NodeDetailDialog, build_material_plan, fetch_market_quotes, leaf_tone_class, market_cost,
    market_item_ids_key, market_meta, recipe_level_label,
};
use crate::app::data::{
    CRAFT_TYPE_ABBRS, CRAFT_TYPE_NAMES, CraftDataEngine, build_tree, collapse_key,
    craftable_recipes, create_craft_data_engine, get_item, get_item_name, load_craft_data,
    summarize_materials,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, input_class,
};
use crate::app::utils::{cx, format_integer};
use xiv_companion::{CraftDataPackage, CraftRecipe, CraftTreeNode, MaterialSummary, SourceChoice};

const NOTES_STORAGE_KEY: &str = "xiv-companion-notes-v1";
const MARKET_WORLD_DC_REGION: &str = "中国";
const GRAPH_EDGE_COLOR: &str = "#94a3b8";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoteTreeNode {
    id: String,
    kind: String,
    title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<NoteTreeNode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CraftSummaryTarget {
    id: String,
    recipe_id: u32,
    item_id: u32,
    amount: u32,
    #[serde(default)]
    collapsed: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CraftSummaryCard {
    id: String,
    kind: String,
    title: String,
    targets: Vec<CraftSummaryTarget>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    source_choices: BTreeMap<String, SourceChoice>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotePage {
    id: String,
    #[serde(default)]
    cards: Vec<CraftSummaryCard>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotesState {
    tree: Vec<NoteTreeNode>,
    pages: BTreeMap<String, NotePage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_page_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_card_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NameDialogKind {
    AddFolder {
        parent_id: Option<String>,
    },
    AddPage {
        parent_id: Option<String>,
    },
    RenameNode {
        node_id: String,
        current_title: String,
    },
}

impl NameDialogKind {
    fn title(&self) -> &'static str {
        match self {
            NameDialogKind::AddFolder { .. } => "新建目录",
            NameDialogKind::AddPage { .. } => "新建页面",
            NameDialogKind::RenameNode { .. } => "重命名",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            NameDialogKind::AddFolder { .. } => "目录名称",
            NameDialogKind::AddPage { .. } => "页面名称",
            NameDialogKind::RenameNode { .. } => "名称",
        }
    }

    fn initial_value(&self) -> String {
        match self {
            NameDialogKind::AddFolder { .. } => "新目录".to_string(),
            NameDialogKind::AddPage { .. } => "新页面".to_string(),
            NameDialogKind::RenameNode { current_title, .. } => current_title.clone(),
        }
    }

    fn confirm_label(&self) -> &'static str {
        match self {
            NameDialogKind::RenameNode { .. } => "保存",
            _ => "创建",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConfirmDialogKind {
    DeleteNode { node_id: String },
    DeleteCard { card_id: String },
}

#[derive(Clone, Debug, PartialEq)]
struct CraftGraphRoot {
    target: CraftSummaryTarget,
    tree: CraftTreeNode,
    collapsed: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct MergedCraftGraphNode {
    item_id: u32,
    amount: u32,
    depth: u32,
    order: usize,
    recipe: Option<CraftRecipe>,
    craftable: bool,
    collapsed: bool,
    root: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct MergedCraftGraphEdge {
    key: String,
    from: u32,
    to: u32,
    amount: u32,
    order: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct MergedCraftGraph {
    nodes: Vec<MergedCraftGraphNode>,
    edges: Vec<MergedCraftGraphEdge>,
}

#[derive(Clone, Debug, PartialEq)]
struct PositionedCraftGraphNode {
    node: MergedCraftGraphNode,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PositionedCraftGraphEdge {
    edge: MergedCraftGraphEdge,
    from_node: PositionedCraftGraphNode,
    to_node: PositionedCraftGraphNode,
    from_offset: f64,
    to_offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct CraftGraphLayout {
    nodes: Vec<PositionedCraftGraphNode>,
    edges: Vec<PositionedCraftGraphEdge>,
    width: f64,
    height: f64,
    node_width: f64,
    node_height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GraphLayoutRelation {
    item_id: u32,
    order: usize,
}

fn id() -> String {
    let now = js_sys::Date::now().round() as u64;
    let random = (js_sys::Math::random() * 1_000_000_000.0).round() as u32;
    format!("{now:x}-{random:x}")
}

fn create_default_page(page_id: String) -> NotePage {
    NotePage {
        id: page_id,
        cards: Vec::new(),
    }
}

fn create_default_state() -> NotesState {
    let page_id = id();
    let mut pages = BTreeMap::new();
    pages.insert(page_id.clone(), create_default_page(page_id.clone()));
    NotesState {
        tree: vec![NoteTreeNode {
            id: page_id.clone(),
            kind: "page".to_string(),
            title: "新笔记".to_string(),
            children: Vec::new(),
        }],
        pages,
        active_page_id: Some(page_id),
        active_card_id: None,
    }
}

fn collect_page_ids(node: &NoteTreeNode, result: &mut Vec<String>) {
    if node.kind == "page" {
        result.push(node.id.clone());
    }
    for child in &node.children {
        collect_page_ids(child, result);
    }
}

fn first_page_id(nodes: &[NoteTreeNode]) -> Option<String> {
    for node in nodes {
        if node.kind == "page" {
            return Some(node.id.clone());
        }
        if let Some(id) = first_page_id(&node.children) {
            return Some(id);
        }
    }
    None
}

fn find_tree_node<'a>(
    nodes: &'a [NoteTreeNode],
    node_id: Option<&str>,
) -> Option<&'a NoteTreeNode> {
    let node_id = node_id?;
    for node in nodes {
        if node.id == node_id {
            return Some(node);
        }
        if let Some(child) = find_tree_node(&node.children, Some(node_id)) {
            return Some(child);
        }
    }
    None
}

fn append_tree_node(
    nodes: &[NoteTreeNode],
    parent_id: Option<&str>,
    node_to_add: NoteTreeNode,
) -> Vec<NoteTreeNode> {
    if parent_id.is_none() {
        let mut next = nodes.to_vec();
        next.push(node_to_add);
        return next;
    }

    nodes
        .iter()
        .map(|node| {
            if node.kind != "folder" {
                return node.clone();
            }
            if Some(node.id.as_str()) == parent_id {
                let mut next = node.clone();
                next.children.push(node_to_add.clone());
                return next;
            }
            let mut next = node.clone();
            next.children = append_tree_node(&node.children, parent_id, node_to_add.clone());
            next
        })
        .collect()
}

fn rename_tree_node(nodes: &[NoteTreeNode], node_id: &str, title: &str) -> Vec<NoteTreeNode> {
    nodes
        .iter()
        .map(|node| {
            let mut next = node.clone();
            if node.id == node_id {
                next.title = title.to_string();
            } else if node.kind == "folder" {
                next.children = rename_tree_node(&node.children, node_id, title);
            }
            next
        })
        .collect()
}

fn delete_tree_node(nodes: &[NoteTreeNode], node_id: &str) -> (Vec<NoteTreeNode>, Vec<String>) {
    let mut removed_pages = Vec::new();
    let mut next_nodes = Vec::new();

    for node in nodes {
        if node.id == node_id {
            collect_page_ids(node, &mut removed_pages);
            continue;
        }
        if node.kind == "folder" {
            let (children, mut child_pages) = delete_tree_node(&node.children, node_id);
            removed_pages.append(&mut child_pages);
            let mut next = node.clone();
            next.children = children;
            next_nodes.push(next);
        } else {
            next_nodes.push(node.clone());
        }
    }

    (next_nodes, removed_pages)
}

fn normalize_target(raw: &Value) -> Option<CraftSummaryTarget> {
    let recipe_id = raw.get("recipeId")?.as_u64()? as u32;
    let item_id = raw.get("itemId")?.as_u64()? as u32;
    if recipe_id == 0 || item_id == 0 {
        return None;
    }
    let amount = raw
        .get("amount")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as u32;
    let collapsed = raw
        .get("collapsed")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(CraftSummaryTarget {
        id: raw
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(id),
        recipe_id,
        item_id,
        amount,
        collapsed,
    })
}

fn normalize_card(raw: &Value, fallback_title: &str) -> Option<CraftSummaryCard> {
    if raw.get("kind").and_then(Value::as_str) == Some("craftSummary") {
        let targets = raw
            .get("targets")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(normalize_target)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let source_choices = raw
            .get("sourceChoices")
            .cloned()
            .and_then(|value| serde_json::from_value::<BTreeMap<String, SourceChoice>>(value).ok())
            .unwrap_or_default();
        return Some(CraftSummaryCard {
            id: raw
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(id),
            kind: "craftSummary".to_string(),
            title: raw
                .get("title")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(fallback_title)
                .to_string(),
            targets,
            source_choices,
        });
    }

    normalize_target(raw).map(|target| CraftSummaryCard {
        id: id(),
        kind: "craftSummary".to_string(),
        title: fallback_title.to_string(),
        targets: vec![target],
        source_choices: BTreeMap::new(),
    })
}

fn normalize_page(raw: Option<&Value>, page_id: &str) -> NotePage {
    let Some(raw) = raw else {
        return create_default_page(page_id.to_string());
    };

    if let Some(cards) = raw.get("cards").and_then(Value::as_array) {
        return NotePage {
            id: page_id.to_string(),
            cards: cards
                .iter()
                .enumerate()
                .filter_map(|(index, card)| {
                    normalize_card(card, &format!("合成汇总 {}", index + 1))
                })
                .collect(),
        };
    }

    if let Some(sections) = raw.get("sections").and_then(Value::as_array) {
        let cards = sections
            .iter()
            .enumerate()
            .filter_map(|(index, section)| {
                let targets = section
                    .get("cards")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(normalize_target)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if targets.is_empty() {
                    return None;
                }
                let source_choices = section
                    .get("sourceChoices")
                    .cloned()
                    .and_then(|value| {
                        serde_json::from_value::<BTreeMap<String, SourceChoice>>(value).ok()
                    })
                    .unwrap_or_default();
                Some(CraftSummaryCard {
                    id: section
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(id),
                    kind: "craftSummary".to_string(),
                    title: section
                        .get("title")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("合成汇总 {}", index + 1)),
                    targets,
                    source_choices,
                })
            })
            .collect::<Vec<_>>();
        return NotePage {
            id: page_id.to_string(),
            cards,
        };
    }

    create_default_page(page_id.to_string())
}

fn normalize_tree_node(raw: &Value) -> Option<NoteTreeNode> {
    let id = raw.get("id")?.as_str()?.to_string();
    let kind = raw.get("kind")?.as_str()?.to_string();
    if kind != "folder" && kind != "page" {
        return None;
    }
    let title = raw
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(if kind == "folder" {
            "新目录"
        } else {
            "新笔记"
        })
        .to_string();
    let children = raw
        .get("children")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(normalize_tree_node).collect())
        .unwrap_or_default();
    Some(NoteTreeNode {
        id,
        kind,
        title,
        children,
    })
}

fn normalize_state(raw: Value) -> NotesState {
    let fallback = create_default_state();
    let tree = raw
        .get("tree")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_tree_node)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if tree.is_empty() {
        return fallback;
    }

    let mut page_ids = Vec::new();
    for node in &tree {
        collect_page_ids(node, &mut page_ids);
    }
    if page_ids.is_empty() {
        return fallback;
    }

    let raw_pages = raw.get("pages").and_then(Value::as_object);
    let mut pages = BTreeMap::new();
    for page_id in &page_ids {
        let raw_page = raw_pages.and_then(|pages| pages.get(page_id));
        pages.insert(page_id.clone(), normalize_page(raw_page, page_id));
    }

    let active_page_id = raw
        .get("activePageId")
        .and_then(Value::as_str)
        .filter(|id| page_ids.iter().any(|page_id| page_id == id))
        .map(str::to_string)
        .or_else(|| first_page_id(&tree));
    let active_card_id = active_page_id.as_ref().and_then(|page_id| {
        let active_page = pages.get(page_id)?;
        let raw_active = raw
            .get("activeCardId")
            .or_else(|| raw.get("activeSectionId"))
            .and_then(Value::as_str);
        raw_active
            .filter(|id| active_page.cards.iter().any(|card| card.id == *id))
            .map(str::to_string)
            .or_else(|| active_page.cards.first().map(|card| card.id.clone()))
    });

    NotesState {
        tree,
        pages,
        active_page_id,
        active_card_id,
    }
}

fn load_notes_state() -> NotesState {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return create_default_state();
    };
    let Ok(Some(raw)) = storage.get_item(NOTES_STORAGE_KEY) else {
        return create_default_state();
    };
    serde_json::from_str::<Value>(&raw)
        .map(normalize_state)
        .unwrap_or_else(|_| create_default_state())
}

fn save_notes_state(state: &NotesState) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        if let Ok(value) = serde_json::to_string(state) {
            let _ = storage.set_item(NOTES_STORAGE_KEY, &value);
        }
    }
}

fn choice_record_to_map(record: &BTreeMap<String, SourceChoice>) -> HashMap<u32, SourceChoice> {
    record
        .iter()
        .filter_map(|(item_id, choice)| item_id.parse::<u32>().ok().map(|id| (id, choice.clone())))
        .collect()
}

fn summarize_card_materials(
    engine: &CraftDataEngine,
    card: &CraftSummaryCard,
) -> Vec<MaterialSummary> {
    let mut totals = BTreeMap::<u32, u32>::new();
    for target in &card.targets {
        let tree = build_tree(engine, target.item_id, target.amount.max(1));
        let collapsed = collapsed_keys_for_tree(&tree, &target.collapsed.iter().cloned().collect());
        for material in summarize_materials(&tree, &collapsed) {
            *totals.entry(material.item_id).or_default() += material.amount;
        }
    }
    totals
        .into_iter()
        .map(|(item_id, amount)| MaterialSummary { item_id, amount })
        .collect()
}

fn graph_collapse_key(item_id: u32) -> String {
    format!("graph:{item_id}")
}

fn collapsed_keys_for_tree(tree: &CraftTreeNode, collapsed: &HashSet<String>) -> HashSet<String> {
    let mut result = collapsed
        .iter()
        .filter(|key| !key.starts_with("graph:"))
        .cloned()
        .collect::<HashSet<_>>();

    fn visit(
        node: &CraftTreeNode,
        depth: u32,
        collapsed: &HashSet<String>,
        result: &mut HashSet<String>,
    ) {
        if collapsed.contains(&graph_collapse_key(node.item_id)) {
            result.insert(collapse_key(node.item_id, depth));
        }
        for child in &node.children {
            visit(child, depth + 1, collapsed, result);
        }
    }

    visit(tree, 0, collapsed, &mut result);
    result
}

fn target_with_collapsed_item(
    target: &CraftSummaryTarget,
    item_id: u32,
    collapsed: bool,
) -> CraftSummaryTarget {
    let graph_key = graph_collapse_key(item_id);
    let legacy_prefix = format!("{item_id}:");
    let mut next = target
        .collapsed
        .iter()
        .filter(|key| *key != &graph_key && !key.starts_with(&legacy_prefix))
        .cloned()
        .collect::<Vec<_>>();
    if collapsed {
        next.push(graph_key);
    }
    CraftSummaryTarget {
        collapsed: next,
        ..target.clone()
    }
}

fn parse_crystal_resource_name(name: &str) -> Option<(&'static str, &'static str)> {
    const ELEMENTS: [&str; 6] = ["火", "冰", "风", "土", "雷", "水"];
    const TIERS: [&str; 3] = ["碎晶", "水晶", "晶簇"];
    for tier in TIERS {
        for element in ELEMENTS {
            if name == format!("{element}之{tier}") {
                return Some((element, tier));
            }
        }
    }
    None
}

fn is_crystal_resource(data: &CraftDataPackage, item_id: u32) -> bool {
    parse_crystal_resource_name(&get_item_name(data, item_id)).is_some()
}

fn crystal_total(data: &CraftDataPackage, materials: &[MaterialSummary]) -> u32 {
    materials
        .iter()
        .filter(|material| is_crystal_resource(data, material.item_id))
        .map(|material| material.amount)
        .sum()
}

fn is_crystal_resource_leaf(
    data: &CraftDataPackage,
    node: &CraftTreeNode,
    parent_item_id: Option<u32>,
) -> bool {
    parent_item_id.is_some() && node.children.is_empty() && is_crystal_resource(data, node.item_id)
}

fn build_merged_craft_graph(data: &CraftDataPackage, roots: &[CraftGraphRoot]) -> MergedCraftGraph {
    let root_ids = roots
        .iter()
        .map(|root| root.tree.item_id)
        .collect::<HashSet<_>>();
    let mut nodes = HashMap::<u32, MergedCraftGraphNode>::new();
    let mut edges = HashMap::<String, MergedCraftGraphEdge>::new();
    let mut order = 0usize;

    fn visit(
        data: &CraftDataPackage,
        node: &CraftTreeNode,
        depth: u32,
        collapsed: &HashSet<String>,
        root_ids: &HashSet<u32>,
        nodes: &mut HashMap<u32, MergedCraftGraphNode>,
        edges: &mut HashMap<String, MergedCraftGraphEdge>,
        order: &mut usize,
        parent_item_id: Option<u32>,
        child_order: usize,
    ) {
        let graph_collapsed = collapsed.contains(&graph_collapse_key(node.item_id));
        let legacy_collapsed = collapsed.contains(&collapse_key(node.item_id, depth));
        let is_collapsed = graph_collapsed || legacy_collapsed;
        if is_crystal_resource_leaf(data, node, parent_item_id) {
            return;
        }

        if let Some(existing) = nodes.get_mut(&node.item_id) {
            existing.amount = existing.amount.saturating_add(node.amount_needed);
            existing.depth = existing.depth.max(depth);
            existing.craftable = existing.craftable || !node.children.is_empty();
            existing.collapsed = existing.collapsed || is_collapsed;
            existing.root = existing.root || root_ids.contains(&node.item_id);
            if existing.recipe.is_none() {
                existing.recipe.clone_from(&node.recipe);
            }
        } else {
            nodes.insert(
                node.item_id,
                MergedCraftGraphNode {
                    item_id: node.item_id,
                    amount: node.amount_needed,
                    depth,
                    order: *order,
                    recipe: node.recipe.clone(),
                    craftable: !node.children.is_empty(),
                    collapsed: is_collapsed,
                    root: root_ids.contains(&node.item_id),
                },
            );
            *order += 1;
        }

        if let Some(parent_item_id) = parent_item_id {
            let edge_key = format!("{parent_item_id}->{}", node.item_id);
            if let Some(edge) = edges.get_mut(&edge_key) {
                edge.amount = edge.amount.saturating_add(node.amount_needed);
                edge.order = edge.order.min(child_order);
            } else {
                edges.insert(
                    edge_key.clone(),
                    MergedCraftGraphEdge {
                        key: edge_key,
                        from: parent_item_id,
                        to: node.item_id,
                        amount: node.amount_needed,
                        order: child_order,
                    },
                );
            }
        }

        if node.children.is_empty() || is_collapsed {
            return;
        }
        for (index, child) in node.children.iter().enumerate() {
            visit(
                data,
                child,
                depth + 1,
                collapsed,
                root_ids,
                nodes,
                edges,
                order,
                Some(node.item_id),
                index,
            );
        }
    }

    for root in roots {
        visit(
            data,
            &root.tree,
            0,
            &root.collapsed,
            &root_ids,
            &mut nodes,
            &mut edges,
            &mut order,
            None,
            0,
        );
    }

    let mut depth_by_item = nodes
        .keys()
        .map(|item_id| (*item_id, 0u32))
        .collect::<HashMap<_, _>>();
    for _ in 0..nodes.len() {
        let mut changed = false;
        for edge in edges.values() {
            if root_ids.contains(&edge.to) {
                continue;
            }
            let next_depth = depth_by_item
                .get(&edge.from)
                .copied()
                .unwrap_or_default()
                .saturating_add(1);
            if next_depth > depth_by_item.get(&edge.to).copied().unwrap_or_default() {
                depth_by_item.insert(edge.to, next_depth);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for node in nodes.values_mut() {
        node.depth = depth_by_item
            .get(&node.item_id)
            .copied()
            .unwrap_or(node.depth);
    }

    let mut sorted_nodes = nodes.into_values().collect::<Vec<_>>();
    sorted_nodes.sort_by_key(|node| (node.depth, node.order));
    MergedCraftGraph {
        nodes: sorted_nodes,
        edges: edges.into_values().collect(),
    }
}

fn graph_lane_indexes(lanes: &BTreeMap<u32, Vec<MergedCraftGraphNode>>) -> HashMap<u32, usize> {
    let mut indexes = HashMap::new();
    for lane in lanes.values() {
        for (index, node) in lane.iter().enumerate() {
            indexes.insert(node.item_id, index);
        }
    }
    indexes
}

fn resolved_graph_relations(
    relations: Option<&Vec<GraphLayoutRelation>>,
    indexes: &HashMap<u32, usize>,
) -> Vec<(usize, usize)> {
    relations
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|relation| {
            indexes
                .get(&relation.item_id)
                .copied()
                .map(|index| (index, relation.order))
        })
        .collect()
}

fn average_resolved_index(relations: &[(usize, usize)]) -> f64 {
    relations
        .iter()
        .map(|(index, _)| *index as f64)
        .sum::<f64>()
        / relations.len().max(1) as f64
}

fn primary_resolved_order(relations: &[(usize, usize)]) -> usize {
    relations
        .iter()
        .min_by_key(|(index, order)| (*index, *order))
        .map(|(_, order)| *order)
        .unwrap_or_default()
}

fn related_node_average(
    relations: Option<&Vec<GraphLayoutRelation>>,
    indexes: &HashMap<u32, usize>,
) -> Option<f64> {
    let resolved = resolved_graph_relations(relations, indexes);
    (!resolved.is_empty()).then(|| average_resolved_index(&resolved))
}

fn related_node_sort_value(
    relations: Option<&Vec<GraphLayoutRelation>>,
    indexes: &HashMap<u32, usize>,
    fallback_index: usize,
) -> f64 {
    let resolved = resolved_graph_relations(relations, indexes);
    if resolved.is_empty() {
        return fallback_index as f64 * 1000.0;
    }
    let anchor = average_resolved_index(&resolved);
    let parent_count = resolved
        .iter()
        .map(|(index, _)| *index)
        .collect::<HashSet<_>>()
        .len();
    let shared_offset = if parent_count > 1 { 500.0 } else { 0.0 };
    anchor * 1000.0 + shared_offset + primary_resolved_order(&resolved) as f64
}

fn sort_graph_lane(
    lane: &mut [MergedCraftGraphNode],
    related: &HashMap<u32, Vec<GraphLayoutRelation>>,
    indexes: &HashMap<u32, usize>,
) {
    let current_indexes = lane
        .iter()
        .enumerate()
        .map(|(index, node)| (node.item_id, index))
        .collect::<HashMap<_, _>>();
    let has_related_score = lane
        .iter()
        .any(|node| related_node_average(related.get(&node.item_id), indexes).is_some());
    if !has_related_score {
        return;
    }

    lane.sort_by(|a, b| {
        let a_current = current_indexes.get(&a.item_id).copied().unwrap_or(a.order);
        let b_current = current_indexes.get(&b.item_id).copied().unwrap_or(b.order);
        related_node_sort_value(related.get(&a.item_id), indexes, a_current)
            .partial_cmp(&related_node_sort_value(
                related.get(&b.item_id),
                indexes,
                b_current,
            ))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a_current.cmp(&b_current))
            .then_with(|| a.order.cmp(&b.order))
    });
}

fn ordered_graph_lanes(
    nodes: &[MergedCraftGraphNode],
    edges: &[MergedCraftGraphEdge],
) -> BTreeMap<u32, Vec<MergedCraftGraphNode>> {
    let mut lanes = BTreeMap::<u32, Vec<MergedCraftGraphNode>>::new();
    let node_ids = nodes
        .iter()
        .map(|node| node.item_id)
        .collect::<HashSet<_>>();
    let mut incoming = HashMap::<u32, Vec<GraphLayoutRelation>>::new();
    let mut outgoing = HashMap::<u32, Vec<GraphLayoutRelation>>::new();

    for node in nodes {
        lanes.entry(node.depth).or_default().push(node.clone());
    }
    for lane in lanes.values_mut() {
        lane.sort_by_key(|node| node.order);
    }

    for edge in edges {
        if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
            continue;
        }
        incoming
            .entry(edge.to)
            .or_default()
            .push(GraphLayoutRelation {
                item_id: edge.from,
                order: edge.order,
            });
        outgoing
            .entry(edge.from)
            .or_default()
            .push(GraphLayoutRelation {
                item_id: edge.to,
                order: edge.order,
            });
    }

    let depths = lanes.keys().copied().collect::<Vec<_>>();
    for _ in 0..6 {
        let mut indexes = graph_lane_indexes(&lanes);
        for depth in depths.iter().copied().skip(1) {
            if let Some(lane) = lanes.get_mut(&depth) {
                sort_graph_lane(lane, &incoming, &indexes);
                indexes = graph_lane_indexes(&lanes);
            }
        }

        indexes = graph_lane_indexes(&lanes);
        for depth in depths.iter().rev().copied().skip(1) {
            if let Some(lane) = lanes.get_mut(&depth) {
                sort_graph_lane(lane, &outgoing, &indexes);
                indexes = graph_lane_indexes(&lanes);
            }
        }
    }

    let mut indexes = graph_lane_indexes(&lanes);
    for depth in depths.into_iter().skip(1) {
        if let Some(lane) = lanes.get_mut(&depth) {
            sort_graph_lane(lane, &incoming, &indexes);
            indexes = graph_lane_indexes(&lanes);
        }
    }

    lanes
}

fn connected_graph_items(
    item_id: u32,
    incoming: &HashMap<u32, Vec<GraphLayoutRelation>>,
    outgoing: &HashMap<u32, Vec<GraphLayoutRelation>>,
) -> Vec<GraphLayoutRelation> {
    incoming
        .get(&item_id)
        .into_iter()
        .flat_map(|items| items.iter().copied())
        .chain(
            outgoing
                .get(&item_id)
                .into_iter()
                .flat_map(|items| items.iter().copied()),
        )
        .collect()
}

fn average_connected_center(
    item_id: u32,
    incoming: &HashMap<u32, Vec<GraphLayoutRelation>>,
    outgoing: &HashMap<u32, Vec<GraphLayoutRelation>>,
    centers: &HashMap<u32, f64>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for relation in connected_graph_items(item_id, incoming, outgoing) {
        if let Some(center) = centers.get(&relation.item_id) {
            total += center;
            count += 1;
        }
    }
    (count > 0).then(|| total / count as f64)
}

fn clamp_graph_position(value: f64, min: f64, max: f64) -> f64 {
    value.min(max).max(min)
}

fn lane_ideal_gap(count: usize) -> f64 {
    if count >= 11 {
        12.0
    } else if count >= 8 {
        16.0
    } else if count >= 5 {
        22.0
    } else if count >= 3 {
        34.0
    } else {
        48.0
    }
}

fn lane_max_gap(count: usize) -> f64 {
    if count >= 11 {
        34.0
    } else if count >= 8 {
        46.0
    } else if count >= 5 {
        62.0
    } else if count >= 3 {
        82.0
    } else {
        112.0
    }
}

fn graph_content_height(lanes: &BTreeMap<u32, Vec<MergedCraftGraphNode>>, node_height: f64) -> f64 {
    let mut height = 260.0;
    for lane in lanes.values() {
        let count = lane.len();
        let lane_height =
            count as f64 * node_height + count.saturating_sub(1) as f64 * lane_ideal_gap(count);
        height = f64::max(height, lane_height);
    }
    height
}

fn initial_lane_center(
    lane_length: usize,
    index: usize,
    content_height: f64,
    node_height: f64,
) -> f64 {
    if lane_length <= 1 {
        return content_height / 2.0;
    }
    let min_gap = lane_ideal_gap(lane_length);
    let available_gap =
        (content_height - lane_length as f64 * node_height) / lane_length.saturating_sub(1) as f64;
    let gap = clamp_graph_position(available_gap, min_gap, 96.0);
    let lane_height = lane_length as f64 * node_height + lane_length.saturating_sub(1) as f64 * gap;
    let start = (content_height - lane_height) / 2.0;
    start + node_height / 2.0 + index as f64 * (node_height + gap)
}

fn shift_lane_into_bounds(centers: &mut [f64], min_center: f64, max_center: f64) {
    let Some(last) = centers.last().copied() else {
        return;
    };
    let overflow = last - max_center;
    if overflow > 0.0 {
        for center in centers.iter_mut() {
            *center -= overflow;
        }
    }

    let Some(first) = centers.first().copied() else {
        return;
    };
    let underflow = min_center - first;
    if underflow > 0.0 {
        for center in centers.iter_mut() {
            *center += underflow;
        }
    }
}

fn enforce_lane_min_step(centers: &mut [f64], min_step: f64, min_center: f64, max_center: f64) {
    for index in 1..centers.len() {
        centers[index] = centers[index].max(centers[index - 1] + min_step);
    }
    shift_lane_into_bounds(centers, min_center, max_center);
    for index in (0..centers.len().saturating_sub(1)).rev() {
        centers[index] = centers[index].min(centers[index + 1] - min_step);
    }
    shift_lane_into_bounds(centers, min_center, max_center);
}

fn enforce_lane_max_step(centers: &mut [f64], max_step: f64, min_center: f64, max_center: f64) {
    for index in 1..centers.len() {
        if centers[index] - centers[index - 1] > max_step {
            centers[index] = centers[index - 1] + max_step;
        }
    }
    for index in (0..centers.len().saturating_sub(1)).rev() {
        if centers[index + 1] - centers[index] > max_step {
            centers[index] = centers[index + 1] - max_step;
        }
    }
    shift_lane_into_bounds(centers, min_center, max_center);
}

fn average_graph_centers(centers: &[f64]) -> f64 {
    centers.iter().sum::<f64>() / centers.len().max(1) as f64
}

fn resolve_lane_centers(
    lane: &[MergedCraftGraphNode],
    centers: &mut HashMap<u32, f64>,
    content_height: f64,
    node_height: f64,
) {
    if lane.is_empty() {
        return;
    }
    let min_center = node_height / 2.0;
    let max_center = content_height - node_height / 2.0;
    let min_step = node_height + lane_ideal_gap(lane.len());
    let max_step = node_height + lane_max_gap(lane.len());
    let desired_centers = lane
        .iter()
        .map(|node| {
            clamp_graph_position(
                centers
                    .get(&node.item_id)
                    .copied()
                    .unwrap_or(content_height / 2.0),
                min_center,
                max_center,
            )
        })
        .collect::<Vec<_>>();
    let mut next_centers = desired_centers.clone();
    let desired_average = average_graph_centers(&desired_centers);

    enforce_lane_min_step(&mut next_centers, min_step, min_center, max_center);
    for _ in 0..3 {
        enforce_lane_max_step(&mut next_centers, max_step, min_center, max_center);
        enforce_lane_min_step(&mut next_centers, min_step, min_center, max_center);
    }

    let current_average = average_graph_centers(&next_centers);
    let average_shift = clamp_graph_position(
        desired_average - current_average,
        min_center - next_centers[0],
        max_center - next_centers[next_centers.len() - 1],
    );
    if average_shift != 0.0 {
        for center in &mut next_centers {
            *center += average_shift;
        }
        enforce_lane_min_step(&mut next_centers, min_step, min_center, max_center);
        enforce_lane_max_step(&mut next_centers, max_step, min_center, max_center);
    }

    for (node, center) in lane.iter().zip(next_centers) {
        centers.insert(node.item_id, center);
    }
}

fn adaptive_graph_centers(
    lanes: &BTreeMap<u32, Vec<MergedCraftGraphNode>>,
    edges: &[MergedCraftGraphEdge],
    node_height: f64,
) -> (HashMap<u32, f64>, f64) {
    let node_ids = lanes
        .values()
        .flat_map(|lane| lane.iter().map(|node| node.item_id))
        .collect::<HashSet<_>>();
    let mut incoming = HashMap::<u32, Vec<GraphLayoutRelation>>::new();
    let mut outgoing = HashMap::<u32, Vec<GraphLayoutRelation>>::new();
    let mut centers = HashMap::<u32, f64>::new();
    let content_height = graph_content_height(lanes, node_height);
    let depths = lanes.keys().copied().collect::<Vec<_>>();

    for edge in edges {
        if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
            continue;
        }
        incoming
            .entry(edge.to)
            .or_default()
            .push(GraphLayoutRelation {
                item_id: edge.from,
                order: edge.order,
            });
        outgoing
            .entry(edge.from)
            .or_default()
            .push(GraphLayoutRelation {
                item_id: edge.to,
                order: edge.order,
            });
    }

    for depth in &depths {
        let lane = lanes.get(depth).cloned().unwrap_or_default();
        for (index, node) in lane.iter().enumerate() {
            centers.insert(
                node.item_id,
                initial_lane_center(lane.len(), index, content_height, node_height),
            );
        }
    }

    for _ in 0..8 {
        for depth in &depths {
            let lane = lanes.get(depth).cloned().unwrap_or_default();
            for node in &lane {
                if let Some(connected_center) =
                    average_connected_center(node.item_id, &incoming, &outgoing, &centers)
                {
                    let current = centers
                        .get(&node.item_id)
                        .copied()
                        .unwrap_or(connected_center);
                    centers.insert(node.item_id, current * 0.38 + connected_center * 0.62);
                }
            }
            resolve_lane_centers(&lane, &mut centers, content_height, node_height);
        }

        for depth in depths.iter().rev() {
            let lane = lanes.get(depth).cloned().unwrap_or_default();
            for node in &lane {
                if let Some(connected_center) =
                    average_connected_center(node.item_id, &incoming, &outgoing, &centers)
                {
                    let current = centers
                        .get(&node.item_id)
                        .copied()
                        .unwrap_or(connected_center);
                    centers.insert(node.item_id, current * 0.45 + connected_center * 0.55);
                }
            }
            resolve_lane_centers(&lane, &mut centers, content_height, node_height);
        }
    }

    (centers, content_height)
}

fn graph_edge_port_offset(index: usize, count: usize, node_height: f64) -> f64 {
    if count <= 1 {
        return 0.0;
    }
    let span = (node_height * 0.62).min(((count - 1) * 7).max(18) as f64);
    -span / 2.0 + (span * index as f64) / count.saturating_sub(1) as f64
}

fn apply_graph_edge_ports(edges: &mut [PositionedCraftGraphEdge], node_height: f64) {
    let mut by_from = HashMap::<u32, Vec<usize>>::new();
    let mut by_to = HashMap::<u32, Vec<usize>>::new();

    for (index, edge) in edges.iter().enumerate() {
        by_from.entry(edge.edge.from).or_default().push(index);
        by_to.entry(edge.edge.to).or_default().push(index);
    }

    for group in by_from.values_mut() {
        group.sort_by(|a, b| {
            edges[*a]
                .to_node
                .y
                .partial_cmp(&edges[*b].to_node.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| edges[*a].edge.order.cmp(&edges[*b].edge.order))
                .then_with(|| edges[*a].edge.to.cmp(&edges[*b].edge.to))
        });
        let len = group.len();
        for (index, edge_index) in group.iter().copied().enumerate() {
            edges[edge_index].from_offset = graph_edge_port_offset(index, len, node_height);
        }
    }

    for group in by_to.values_mut() {
        group.sort_by(|a, b| {
            edges[*a]
                .from_node
                .y
                .partial_cmp(&edges[*b].from_node.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| edges[*a].edge.order.cmp(&edges[*b].edge.order))
                .then_with(|| edges[*a].edge.from.cmp(&edges[*b].edge.from))
        });
        let len = group.len();
        for (index, edge_index) in group.iter().copied().enumerate() {
            edges[edge_index].to_offset = graph_edge_port_offset(index, len, node_height);
        }
    }
}

fn build_graph_layout(graph: &MergedCraftGraph) -> CraftGraphLayout {
    let node_width = 184.0;
    let node_height = 58.0;
    let x_gap = 116.0;
    let padding = 18.0;
    let lanes = ordered_graph_lanes(&graph.nodes, &graph.edges);
    let depths = lanes.keys().copied().collect::<Vec<_>>();
    let (centers, content_height) = adaptive_graph_centers(&lanes, &graph.edges, node_height);

    let mut positioned = Vec::new();
    for (column, depth) in depths.iter().copied().enumerate() {
        let lane = lanes.get(&depth).cloned().unwrap_or_default();
        for (index, node) in lane.iter().cloned().enumerate() {
            let center = centers.get(&node.item_id).copied().unwrap_or_else(|| {
                initial_lane_center(lane.len(), index, content_height, node_height)
            });
            positioned.push(PositionedCraftGraphNode {
                node,
                x: padding + column as f64 * (node_width + x_gap),
                y: padding + center - node_height / 2.0,
            });
        }
    }

    let by_item_id = positioned
        .iter()
        .cloned()
        .map(|node| (node.node.item_id, node))
        .collect::<HashMap<_, _>>();
    let mut positioned_edges = Vec::new();
    for edge in &graph.edges {
        let Some(from_node) = by_item_id.get(&edge.from).cloned() else {
            continue;
        };
        let Some(to_node) = by_item_id.get(&edge.to).cloned() else {
            continue;
        };
        positioned_edges.push(PositionedCraftGraphEdge {
            edge: edge.clone(),
            from_node,
            to_node,
            from_offset: 0.0,
            to_offset: 0.0,
        });
    }
    apply_graph_edge_ports(&mut positioned_edges, node_height);

    let max_x = positioned
        .iter()
        .fold(node_width, |value, node| value.max(node.x + node_width));
    let max_y = positioned
        .iter()
        .fold(node_height, |value, node| value.max(node.y + node_height));

    CraftGraphLayout {
        nodes: positioned,
        edges: positioned_edges,
        width: max_x + padding,
        height: f64::max(max_y + padding, content_height + padding * 2.0),
        node_width,
        node_height,
    }
}

#[component]
fn MaterialSummaryPanel(
    data: Rc<CraftDataPackage>,
    title: String,
    materials: Vec<MaterialSummary>,
    source_choices: HashMap<u32, SourceChoice>,
    on_choose: EventHandler<(u32, Option<SourceChoice>)>,
    on_inspect_item: EventHandler<(u32, u32)>,
) -> Element {
    let ordinary_materials = materials
        .iter()
        .filter(|material| !is_crystal_resource(&data, material.item_id))
        .cloned()
        .collect::<Vec<_>>();
    let plan = build_material_plan(&data, &ordinary_materials, &source_choices);
    let mut market_quotes_key = use_signal(|| None::<String>);
    let mut market_quotes = use_signal(|| {
        None::<(
            String,
            Result<HashMap<u32, super::crafting::MarketQuote>, String>,
        )>
    });
    let mut market_loading = use_signal(|| false);
    let next_market_key = market_item_ids_key(&plan);
    if market_quotes_key() != next_market_key {
        market_quotes_key.set(next_market_key.clone());
        market_quotes.set(None);
        if let Some(key) = next_market_key {
            market_loading.set(true);
            wasm_bindgen_futures::spawn_local(async move {
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
        &plan,
        quotes_result
            .as_ref()
            .and_then(|result| result.as_ref().ok()),
    );
    let crystal_amount = crystal_total(&data, &materials);

    rsx! {
        section { class: "overflow-hidden rounded-md border bg-background",
            div { class: "shrink-0 border-b p-3",
                div { class: "flex items-start justify-between gap-3",
                    div { class: "min-w-0",
                        div { class: "flex items-center gap-2 text-sm font-semibold",
                            Icon { kind: IconKind::Info, class: "h-4 w-4" }
                            "材料清单"
                        }
                        div { class: "mt-1 truncate text-xs text-muted-foreground", "{title}" }
                    }
                    div { class: "flex shrink-0 flex-wrap items-center justify-end gap-1.5",
                        Badge { variant: BadgeVariant::Outline, "材料 {ordinary_materials.len()}" }
                        if crystal_amount > 0 {
                            Badge { variant: BadgeVariant::Outline, "晶石 {format_integer(crystal_amount as f64)}" }
                        }
                    }
                }

                div { class: "mt-3 flex flex-wrap gap-2",
                    if crystal_amount > 0 {
                        Badge { variant: BadgeVariant::Outline, "晶石汇总 {format_integer(crystal_amount as f64)}" }
                    }
                    if !plan.gathering.is_empty() {
                        Badge { variant: BadgeVariant::Success,
                            Icon { kind: IconKind::Leaf, class: "mr-1 h-3 w-3" }
                            "采集 {plan.gathering.len()}"
                        }
                    }
                    if !plan.exchange_groups.is_empty() {
                        Badge { variant: BadgeVariant::Outline,
                            Icon { kind: IconKind::Shuffle, class: "mr-1 h-3 w-3" }
                            "兑换 {plan.exchange_groups.len()}"
                        }
                    }
                    if plan.gil_total > 0 {
                        Badge { variant: BadgeVariant::Warning,
                            Icon { kind: IconKind::Coins, class: "mr-1 h-3 w-3" }
                            "商店 {format_integer(plan.gil_total as f64)}G"
                        }
                    }
                    if !plan.market.is_empty() {
                        Badge { variant: BadgeVariant::Outline,
                            Icon { kind: IconKind::Coins, class: "mr-1 h-3 w-3" }
                            "市场 {plan.market.len()}"
                        }
                    }
                    if market_cost_value.total > 0 {
                        Badge { variant: BadgeVariant::Outline, "估 {format_integer(market_cost_value.total as f64)}G" }
                    }
                    if !plan.owned.is_empty() {
                        Badge { variant: BadgeVariant::Outline,
                            Icon { kind: IconKind::CircleCheck, class: "mr-1 h-3 w-3" }
                            "已拥有 {plan.owned.len()}"
                        }
                    }
                }
            }

            div { class: "p-3",
                if plan.is_empty() && crystal_amount == 0 {
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                        title: "暂无材料".to_string(),
                        description: "卡片里选择物品后会汇总叶子材料".to_string(),
                    }
                } else {
                    div { class: "space-y-4",
                        if crystal_amount > 0 {
                            section {
                                div { class: "overflow-hidden rounded-md border bg-[#fafaf9]",
                                    div { class: "flex items-center justify-between gap-2 p-3",
                                        div {
                                            div { class: "text-xs font-semibold text-foreground", "晶石消耗" }
                                            div { class: "mt-0.5 text-[11px] text-muted-foreground", "碎晶 / 水晶 / 晶簇按元素汇总" }
                                        }
                                        Badge { variant: BadgeVariant::Secondary, "总 {format_integer(crystal_amount as f64)}" }
                                    }
                                }
                            }
                        }

                        if !plan.exchange_groups.is_empty() {
                            section {
                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                    Icon { kind: IconKind::Shuffle, class: "h-4 w-4 text-[#3b2778]" }
                                    "兑换"
                                }
                                div { class: "space-y-2",
                                    for group in plan.exchange_groups.clone() {
                                        ExchangeGroupPanel {
                                            data: data.clone(),
                                            group,
                                            on_choose,
                                            on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
                                            on_inspect_item,
                                        }
                                    }
                                }
                            }
                        }

                        if !plan.gathering.is_empty() {
                            section {
                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                    Icon { kind: IconKind::Leaf, class: "h-4 w-4 text-emerald-700" }
                                    "采集清单"
                                }
                                div { class: "space-y-2",
                                    for entry in plan.gathering.clone() {
                                        MaterialPlanRow {
                                            data: data.clone(),
                                            entry,
                                            row_class: "border-l-emerald-200 bg-emerald-50/80",
                                            on_choose,
                                            on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
                                        }
                                    }
                                }
                            }
                        }

                        if !plan.shops.is_empty() {
                            section {
                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                    Icon { kind: IconKind::Coins, class: "h-4 w-4 text-amber-700" }
                                    "商店购买"
                                }
                                div { class: "space-y-2",
                                    for entry in plan.shops.clone() {
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
                                                    on_choose,
                                                    on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if !plan.market.is_empty() {
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
                                    for entry in plan.market.clone() {
                                        MaterialPlanRow {
                                            data: data.clone(),
                                            meta: market_meta(&entry, quotes_result.as_ref(), market_loading()),
                                            entry,
                                            row_class: "border-l-[#93c5fd] bg-[#eff6ff]",
                                            on_choose,
                                            on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
                                        }
                                    }
                                }
                            }
                        }

                        if !plan.unknown.is_empty() {
                            section {
                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold",
                                    Icon { kind: IconKind::PackageSearch, class: "h-4 w-4 text-muted-foreground" }
                                    "未安排"
                                }
                                div { class: "space-y-2",
                                    for entry in plan.unknown.clone() {
                                        MaterialPlanRow {
                                            data: data.clone(),
                                            entry,
                                            row_class: "border-l-border bg-background",
                                            on_choose,
                                            on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
                                        }
                                    }
                                }
                            }
                        }

                        if !plan.owned.is_empty() {
                            section {
                                div { class: "mb-2 flex items-center gap-2 text-sm font-semibold text-muted-foreground",
                                    Icon { kind: IconKind::CircleCheck, class: "h-4 w-4" }
                                    "已拥有"
                                }
                                div { class: "space-y-2",
                                    for entry in plan.owned.clone() {
                                        MaterialPlanRow {
                                            data: data.clone(),
                                            entry,
                                            row_class: "border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground",
                                            on_choose,
                                            on_inspect: move |entry: MaterialPlanEntry| on_inspect_item.call((entry.item_id, entry.amount)),
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
fn MergedCraftGraphNodeCard(
    data: Rc<CraftDataPackage>,
    node: PositionedCraftGraphNode,
    width: f64,
    height: f64,
    source_choices: HashMap<u32, SourceChoice>,
    on_toggle: EventHandler<(u32, bool)>,
    on_select: EventHandler<DetailTarget>,
) -> Element {
    let graph_node = node.node.clone();
    let counts_as_leaf = !graph_node.craftable || graph_node.collapsed;
    let tone_class = if counts_as_leaf {
        leaf_tone_class(
            &data,
            &CraftTreeNode {
                item_id: graph_node.item_id,
                amount_needed: graph_node.amount,
                recipe: graph_node.recipe.clone(),
                children: Vec::new(),
            },
            &source_choices,
        )
    } else {
        "border-l-transparent bg-background hover:bg-accent/70"
    };
    let item = get_item(&data, graph_node.item_id).cloned();
    let subtitle = graph_node.recipe.as_ref().map(|recipe| {
        format!(
            "{} · {}",
            CRAFT_TYPE_ABBRS[recipe.craft_type.min(7) as usize],
            recipe_level_label(&data, recipe)
        )
    });

    rsx! {
        div {
            class: "absolute",
            style: "left: {node.x}px; top: {node.y}px; width: {width}px; height: {height}px;",
            button {
                r#type: "button",
                class: cx([
                    "grid h-full w-full grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-1.5 rounded border border-border border-l-2 px-2 py-1.5 text-left text-xs shadow-sm transition-colors",
                    tone_class,
                ]),
                onclick: {
                    let graph_node = graph_node.clone();
                    move |_| on_select.call(DetailTarget {
                        item_id: graph_node.item_id,
                        amount_needed: graph_node.amount,
                        recipe: graph_node.recipe.clone(),
                    })
                },
                ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0), size: "sm" }
                div { class: "min-w-0",
                    div { class: "truncate font-medium", "{get_item_name(&data, graph_node.item_id)}" }
                    if let Some(subtitle) = subtitle.as_ref() {
                        div {
                            class: "truncate text-[11px] text-muted-foreground",
                            title: subtitle.as_str(),
                            "{subtitle}"
                        }
                    }
                }
                div { class: "flex flex-col items-end gap-1",
                    Badge {
                        variant: BadgeVariant::Secondary,
                        class: "h-4 px-1 text-[10px]".to_string(),
                        "总 {format_integer(graph_node.amount as f64)}"
                    }
                    if graph_node.root {
                        Badge {
                            variant: BadgeVariant::Outline,
                            class: "h-4 bg-background/80 px-1 text-[10px]".to_string(),
                            "目标"
                        }
                    }
                    if counts_as_leaf && !graph_node.root {
                        Badge {
                            variant: BadgeVariant::Outline,
                            class: "h-4 bg-background/80 px-1 text-[10px]".to_string(),
                            "叶"
                        }
                    }
                }
            }

            if graph_node.craftable {
                button {
                    r#type: "button",
                    class: "absolute right-[-12px] top-1/2 z-10 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-full border bg-background text-muted-foreground shadow-sm transition-colors hover:bg-accent hover:text-foreground",
                    aria_label: if graph_node.collapsed { "继续分解" } else { "停止分解" },
                    title: if graph_node.collapsed { "继续分解" } else { "停止分解" },
                    onclick: {
                        let graph_node = graph_node.clone();
                        move |event| {
                            event.stop_propagation();
                            on_toggle.call((graph_node.item_id, !graph_node.collapsed));
                        }
                    },
                    Icon {
                        kind: if graph_node.collapsed { IconKind::Plus } else { IconKind::ChevronRight },
                        class: "h-3.5 w-3.5"
                    }
                }
            }
        }
    }
}

#[component]
fn CraftSummaryGraph(
    data: Rc<CraftDataPackage>,
    roots: Vec<CraftGraphRoot>,
    source_choices: HashMap<u32, SourceChoice>,
    on_toggle_collapsed_item: EventHandler<(u32, bool)>,
    on_select: EventHandler<DetailTarget>,
) -> Element {
    let mut zoom = use_signal(|| 1.0f64);
    let mut update_zoom = move |next: f64| {
        let next = (next.clamp(0.5, 1.4) * 100.0).round() / 100.0;
        zoom.set(next);
    };
    let graph = build_merged_craft_graph(&data, &roots);
    let layout = build_graph_layout(&graph);
    let outer_width = layout.width * zoom();
    let outer_height = layout.height * zoom();
    let zoom_percent = (zoom() * 100.0).round() as u32;

    rsx! {
        section { class: "overflow-hidden rounded-md border bg-background",
            div { class: "flex flex-wrap items-center justify-between gap-3 border-b p-3",
                div { class: "min-w-0",
                    div { class: "text-sm font-semibold", "合成图" }
                    div { class: "mt-0.5 text-xs text-muted-foreground", "全部目标合并为一张图，相同物品共用节点" }
                }
                div { class: "flex flex-wrap items-center gap-2",
                    Badge { variant: BadgeVariant::Outline, "目标 {roots.len()}" }
                    Badge { variant: BadgeVariant::Outline, "节点 {layout.nodes.len()}" }
                    Badge { variant: BadgeVariant::Outline, "边 {layout.edges.len()}" }
                    div { class: "flex items-center gap-1 rounded-md border bg-background p-1",
                        button {
                            r#type: "button",
                            class: "flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                            title: "缩小",
                            aria_label: "缩小",
                            onclick: move |_| update_zoom(zoom() - 0.1),
                            Icon { kind: IconKind::ZoomOut, class: "h-4 w-4" }
                        }
                        div { class: "w-12 text-center text-xs font-medium text-muted-foreground",
                            "{zoom_percent}%"
                        }
                        button {
                            r#type: "button",
                            class: "flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                            title: "放大",
                            aria_label: "放大",
                            onclick: move |_| update_zoom(zoom() + 0.1),
                            Icon { kind: IconKind::ZoomIn, class: "h-4 w-4" }
                        }
                        button {
                            r#type: "button",
                            class: "flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                            title: "重置缩放",
                            aria_label: "重置缩放",
                            onclick: move |_| update_zoom(1.0),
                            Icon { kind: IconKind::RotateCcw, class: "h-4 w-4" }
                        }
                    }
                }
            }

            div { class: "overflow-x-auto overflow-y-visible bg-muted/10 p-3",
                div {
                    class: "relative min-w-max rounded-md border bg-background",
                    style: "width: {outer_width}px; height: {outer_height}px;",
                    div {
                        class: "absolute left-0 top-0 origin-top-left",
                        style: "width: {layout.width}px; height: {layout.height}px; transform: scale({zoom});",
                        svg {
                            class: "pointer-events-none absolute inset-0",
                            width: "{layout.width}",
                            height: "{layout.height}",
                            view_box: "0 0 {layout.width} {layout.height}",
                            defs {
                                marker {
                                    id: "craft-graph-arrow",
                                    marker_width: "10",
                                    marker_height: "10",
                                    ref_x: "9",
                                    ref_y: "5",
                                    orient: "auto",
                                    path { d: "M0,0 L10,5 L0,10 Z", fill: GRAPH_EDGE_COLOR }
                                }
                            }
                            for edge in layout.edges.clone() {
                                {
                                    let x1 = edge.from_node.x + layout.node_width;
                                    let y1 = edge.from_node.y + layout.node_height / 2.0 + edge.from_offset;
                                    let x2 = edge.to_node.x;
                                    let y2 = edge.to_node.y + layout.node_height / 2.0 + edge.to_offset;
                                    let mid_x = (x1 + x2) / 2.0;
                                    let path_value = format!(
                                        "M {x1} {y1} C {mid_x} {y1}, {mid_x} {y2}, {x2} {y2}"
                                    );
                                    let label_x = x1 + (x2 - x1) * 0.68 - 22.0;
                                    let label_y = y1 + (y2 - y1) * 0.68 - 10.0;
                                    rsx! {
                                        g {
                                            path {
                                                d: "{path_value}",
                                                fill: "none",
                                                stroke: GRAPH_EDGE_COLOR,
                                                stroke_opacity: "0.68",
                                                stroke_width: "1.75",
                                                marker_end: "url(#craft-graph-arrow)",
                                            }
                                            foreignObject {
                                                x: "{label_x}",
                                                y: "{label_y}",
                                                width: "44",
                                                height: "20",
                                                div { class: "flex h-5 items-center justify-center",
                                                    span { class: "rounded border bg-background/90 px-1 text-[9px] font-medium leading-none text-muted-foreground shadow-sm",
                                                        "x{format_integer(edge.edge.amount as f64)}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        for node in layout.nodes.clone() {
                            MergedCraftGraphNodeCard {
                                data: data.clone(),
                                node,
                                width: layout.node_width,
                                height: layout.node_height,
                                source_choices: source_choices.clone(),
                                on_toggle: on_toggle_collapsed_item,
                                on_select,
                            }
                        }

                        if layout.nodes.is_empty() {
                            div { class: "absolute inset-0 flex items-center justify-center",
                                EmptyState {
                                    icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                    title: "暂无图节点".to_string(),
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
fn CraftSummaryCardView(
    data: Rc<CraftDataPackage>,
    engine: CraftDataEngine,
    card: CraftSummaryCard,
    active: bool,
    source_choices: HashMap<u32, SourceChoice>,
    on_select: EventHandler<()>,
    on_edit: EventHandler<()>,
    on_remove: EventHandler<()>,
    on_toggle_collapsed_item: EventHandler<(u32, bool)>,
    on_choose_source: EventHandler<(u32, Option<SourceChoice>)>,
    on_inspect: EventHandler<DetailTarget>,
) -> Element {
    let materials = summarize_card_materials(&engine, &card);
    let roots = card
        .targets
        .iter()
        .map(|target| {
            let tree = build_tree(&engine, target.item_id, target.amount.max(1));
            CraftGraphRoot {
                target: target.clone(),
                collapsed: target.collapsed.iter().cloned().collect::<HashSet<_>>(),
                tree,
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: cx([
                "w-full min-w-0 overflow-hidden rounded-md border bg-card transition-colors",
                if active { "border-foreground/30 ring-1 ring-foreground/10" } else { "" },
            ]),
            onclick: move |_| on_select.call(()),
            div { class: "flex items-start gap-3 border-b p-3",
                div { class: "flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-background text-muted-foreground",
                    Icon { kind: IconKind::PackageSearch, class: "h-4 w-4" }
                }
                div { class: "min-w-0 flex-1",
                    div { class: "truncate text-sm font-semibold", "{card.title}" }
                    div { class: "mt-0.5 flex flex-wrap gap-1.5 text-xs text-muted-foreground",
                        Badge { variant: BadgeVariant::Outline, "物品 {card.targets.len()}" }
                        Badge { variant: BadgeVariant::Outline, "叶子 {materials.len()}" }
                    }
                }
                div { class: "flex shrink-0 items-center gap-1",
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        title: "编辑卡片",
                        aria_label: "编辑卡片",
                        onclick: move |event| {
                            event.stop_propagation();
                            on_edit.call(());
                        },
                        Icon { kind: IconKind::Pencil, class: "h-4 w-4" }
                    }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        title: "删除卡片",
                        aria_label: "删除卡片",
                        onclick: move |event| {
                            event.stop_propagation();
                            on_remove.call(());
                        },
                        Icon { kind: IconKind::Trash2, class: "h-4 w-4" }
                    }
                }
            }

            div { class: "space-y-3 p-3",
                if roots.is_empty() {
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                        title: "暂无物品".to_string(),
                        action: rsx! {
                            Button {
                                size: ButtonSize::Sm,
                                variant: ButtonVariant::Outline,
                                onclick: move |_| on_edit.call(()),
                                Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                                "选择物品"
                            }
                        },
                    }
                } else {
                    CraftSummaryGraph {
                        data: data.clone(),
                        roots,
                        source_choices: source_choices.clone(),
                        on_toggle_collapsed_item,
                        on_select: on_inspect,
                    }
                }
                MaterialSummaryPanel {
                    data: data.clone(),
                    title: card.title.clone(),
                    materials,
                    source_choices,
                    on_choose: on_choose_source,
                    on_inspect_item: move |(item_id, amount_needed)| on_inspect.call(DetailTarget {
                        item_id,
                        amount_needed,
                        recipe: None,
                    }),
                }
            }
        }
    }
}

#[component]
fn TreeNodeRow(
    node: NoteTreeNode,
    depth: u32,
    active_page_id: Option<String>,
    expanded: HashSet<String>,
    on_toggle: EventHandler<String>,
    on_select_page: EventHandler<String>,
    on_add_folder: EventHandler<String>,
    on_add_page: EventHandler<String>,
    on_rename: EventHandler<(String, String)>,
    on_delete: EventHandler<String>,
) -> Element {
    let is_folder = node.kind == "folder";
    let is_expanded = expanded.contains(&node.id);
    let active = node.kind == "page" && active_page_id.as_deref() == Some(node.id.as_str());
    let padding_left = format!("{}px", 8 + depth * 16);

    rsx! {
        div {
            div {
                class: cx([
                    "group flex h-9 items-center gap-1 rounded-md px-2 text-sm transition-colors",
                    if active {
                        "bg-accent text-foreground"
                    } else {
                        "text-muted-foreground hover:bg-accent/70 hover:text-foreground"
                    },
                ]),
                style: "padding-left: {padding_left};",
                button {
                    r#type: "button",
                    class: "flex h-6 w-6 shrink-0 items-center justify-center rounded hover:bg-background/80",
                    aria_label: if is_folder {
                        if is_expanded { "折叠目录" } else { "展开目录" }
                    } else {
                        "打开页面"
                    },
                    onclick: {
                        let node_id = node.id.clone();
                        move |_| {
                            if is_folder {
                                on_toggle.call(node_id.clone());
                            } else {
                                on_select_page.call(node_id.clone());
                            }
                        }
                    },
                    if is_folder {
                        Icon {
                            kind: if is_expanded { IconKind::ChevronDown } else { IconKind::ChevronRight },
                            class: "h-4 w-4"
                        }
                    } else {
                        Icon { kind: IconKind::BookOpen, class: "h-4 w-4" }
                    }
                }
                button {
                    r#type: "button",
                    class: "min-w-0 flex-1 truncate text-left",
                    onclick: {
                        let node_id = node.id.clone();
                        move |_| {
                            if is_folder {
                                on_toggle.call(node_id.clone());
                            } else {
                                on_select_page.call(node_id.clone());
                            }
                        }
                    },
                    "{node.title}"
                }
                div { class: "flex shrink-0 items-center gap-0.5 opacity-70 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100",
                    if is_folder {
                        button {
                            r#type: "button",
                            class: "flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background",
                            title: "添加页面",
                            aria_label: "添加页面",
                            onclick: {
                                let node_id = node.id.clone();
                                move |event| {
                                    event.stop_propagation();
                                    on_add_page.call(node_id.clone());
                                }
                            },
                            Icon { kind: IconKind::FilePlus2, class: "h-3.5 w-3.5" }
                        }
                        button {
                            r#type: "button",
                            class: "flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background",
                            title: "添加目录",
                            aria_label: "添加目录",
                            onclick: {
                                let node_id = node.id.clone();
                                move |event| {
                                    event.stop_propagation();
                                    on_add_folder.call(node_id.clone());
                                }
                            },
                            Icon { kind: IconKind::FolderPlus, class: "h-3.5 w-3.5" }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background",
                        title: "重命名",
                        aria_label: "重命名",
                        onclick: {
                            let node_id = node.id.clone();
                            let title = node.title.clone();
                            move |event| {
                                event.stop_propagation();
                                on_rename.call((node_id.clone(), title.clone()));
                            }
                        },
                        Icon { kind: IconKind::MoreHorizontal, class: "h-3.5 w-3.5" }
                    }
                    button {
                        r#type: "button",
                        class: "flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background",
                        title: "删除",
                        aria_label: "删除",
                        onclick: {
                            let node_id = node.id.clone();
                            move |event| {
                                event.stop_propagation();
                                on_delete.call(node_id.clone());
                            }
                        },
                        Icon { kind: IconKind::Trash2, class: "h-3.5 w-3.5" }
                    }
                }
            }

            if is_folder && is_expanded {
                for child in node.children.clone() {
                    TreeNodeRow {
                        node: child,
                        depth: depth + 1,
                        active_page_id: active_page_id.clone(),
                        expanded: expanded.clone(),
                        on_toggle,
                        on_select_page,
                        on_add_folder,
                        on_add_page,
                        on_rename,
                        on_delete,
                    }
                }
            }
        }
    }
}

#[component]
fn NameDialog(
    kind: NameDialogKind,
    on_confirm: EventHandler<(NameDialogKind, String)>,
    on_close: EventHandler<()>,
) -> Element {
    let mut value = use_signal(|| kind.initial_value());
    let trimmed = value().trim().to_string();

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-full max-w-sm overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center justify-between gap-3 border-b p-4",
                    div { class: "min-w-0 text-base font-semibold", "{kind.title()}" }
                    button {
                        r#type: "button",
                        class: "flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                        aria_label: "关闭",
                        title: "关闭",
                        onclick: move |_| on_close.call(()),
                        Icon { kind: IconKind::X, class: "h-4 w-4" }
                    }
                }

                div { class: "space-y-3 p-4",
                    label { class: "grid gap-1.5 text-sm font-medium",
                        "{kind.label()}"
                        input {
                            class: input_class(""),
                            value: "{value}",
                            autofocus: true,
                            oninput: move |event| value.set(event.value()),
                        }
                    }
                }

                div { class: "flex justify-end gap-2 border-t bg-muted/30 p-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: trimmed.is_empty(),
                        onclick: {
                            let kind = kind.clone();
                            move |_| {
                                let value = value().trim().to_string();
                                if !value.is_empty() {
                                    on_confirm.call((kind.clone(), value));
                                    on_close.call(());
                                }
                            }
                        },
                        "{kind.confirm_label()}"
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
fn CraftSummaryEditorDialog(
    data: Rc<CraftDataPackage>,
    engine: CraftDataEngine,
    card: Option<CraftSummaryCard>,
    on_save: EventHandler<CraftSummaryCard>,
    on_close: EventHandler<()>,
) -> Element {
    let mut query = use_signal(String::new);
    let mut craft_type = use_signal(|| None::<u32>);
    let mut title = use_signal(|| {
        card.as_ref()
            .map(|card| card.title.clone())
            .unwrap_or_else(|| "合成汇总".to_string())
    });
    let mut targets = use_signal(|| {
        card.as_ref()
            .map(|card| card.targets.clone())
            .unwrap_or_default()
    });
    let recipes = craftable_recipes(&engine, craft_type(), &query(), 120);
    let selected_recipe_ids = targets()
        .iter()
        .map(|target| target.recipe_id)
        .collect::<HashSet<_>>();

    let mut add_recipe = move |recipe: CraftRecipe| {
        let mut next = targets();
        if next.iter().all(|target| target.recipe_id != recipe.id) {
            next.push(CraftSummaryTarget {
                id: id(),
                recipe_id: recipe.id,
                item_id: recipe.result_item_id,
                amount: 1,
                collapsed: Vec::new(),
            });
            targets.set(next);
        }
    };
    let mut remove_target = move |target_id: String| {
        targets.set(
            targets()
                .into_iter()
                .filter(|target| target.id != target_id)
                .collect(),
        );
    };
    let mut update_target_amount = move |target_id: String, amount: u32| {
        targets.set(
            targets()
                .into_iter()
                .map(|target| {
                    if target.id == target_id {
                        CraftSummaryTarget {
                            amount: amount.max(1),
                            ..target
                        }
                    } else {
                        target
                    }
                })
                .collect(),
        );
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4",
            role: "dialog",
            aria_modal: "true",
            onclick: move |_| on_close.call(()),
            div {
                class: "flex max-h-[min(820px,calc(100vh-2rem))] w-full max-w-5xl flex-col overflow-hidden rounded-md border bg-card shadow-xl",
                onclick: move |event| event.stop_propagation(),
                div { class: "flex items-center gap-3 border-b p-4",
                    div { class: "flex h-9 w-9 items-center justify-center rounded-md border bg-background",
                        Icon { kind: IconKind::PackageSearch, class: "h-4 w-4" }
                    }
                    div { class: "min-w-0 flex-1",
                        div { class: "text-base font-semibold",
                            if card.is_some() { "编辑合成汇总卡片" } else { "新建合成汇总卡片" }
                        }
                        div { class: "text-xs text-muted-foreground", "搜索并多选需要制作的物品" }
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

                div { class: "grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_360px]",
                    section { class: "flex min-h-0 flex-col border-b lg:border-b-0 lg:border-r",
                        div { class: "space-y-3 border-b p-3",
                            label { class: "grid gap-1.5 text-sm font-medium",
                                "卡片名称"
                                input {
                                    class: input_class(""),
                                    value: "{title}",
                                    oninput: move |event| title.set(event.value()),
                                }
                            }
                            div { class: "relative",
                                Icon { kind: IconKind::Search, class: "absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                                input {
                                    class: input_class("pl-9 pr-9"),
                                    value: "{query}",
                                    placeholder: "搜索物品或 ID",
                                    autofocus: true,
                                    oninput: move |event| query.set(event.value()),
                                }
                                if !query().is_empty() {
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

                            div { class: "flex flex-wrap gap-1.5",
                                Button {
                                    size: ButtonSize::Sm,
                                    variant: if craft_type().is_none() { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                    onclick: move |_| craft_type.set(None),
                                    "全部"
                                }
                                for (index, label) in CRAFT_TYPE_ABBRS.iter().enumerate() {
                                    Button {
                                        size: ButtonSize::Sm,
                                        variant: if craft_type() == Some(index as u32) { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                        onclick: move |_| craft_type.set(Some(index as u32)),
                                        "{label}"
                                    }
                                }
                            }
                        }

                        div { class: "min-h-0 flex-1 overflow-y-auto p-2",
                            if recipes.is_empty() {
                                EmptyState {
                                    icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                    title: "没有匹配的配方".to_string(),
                                }
                            } else {
                                for recipe in recipes.clone() {
                                    {
                                        let item = get_item(&data, recipe.result_item_id).cloned();
                                        let selected = selected_recipe_ids.contains(&recipe.id);
                                        rsx! {
                                            button {
                                                key: "{recipe.id}",
                                                r#type: "button",
                                                class: cx([
                                                    "mb-1 grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors",
                                                    if selected { "bg-accent text-foreground" } else { "hover:bg-accent/70" },
                                                ]),
                                                onclick: {
                                                    let recipe = recipe.clone();
                                                    move |_| add_recipe(recipe.clone())
                                                },
                                                ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0) }
                                                div { class: "min-w-0",
                                                    div { class: "truncate font-medium", "{get_item_name(&data, recipe.result_item_id)}" }
                                                    div { class: "truncate text-xs text-muted-foreground",
                                                        "{CRAFT_TYPE_NAMES[recipe.craft_type.min(7) as usize]} · {recipe_level_label(&data, &recipe)}"
                                                    }
                                                }
                                                Badge { variant: if selected { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                    if selected { "已选" } else { "#{recipe.result_item_id}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    aside { class: "flex min-h-0 flex-col",
                        div { class: "flex items-center justify-between gap-3 border-b p-3",
                            div { class: "text-sm font-semibold", "已选物品" }
                            Badge { variant: BadgeVariant::Outline, "{targets().len()}" }
                        }
                        div { class: "min-h-0 flex-1 overflow-y-auto p-3",
                            if targets().is_empty() {
                                EmptyState {
                                    icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                    title: "还没有选择物品".to_string(),
                                }
                            } else {
                                div { class: "space-y-2",
                                    for target in targets() {
                                        {
                                            let item = get_item(&data, target.item_id).cloned();
                                            let recipe = data.recipes.iter().find(|recipe| recipe.id == target.recipe_id).cloned();
                                            rsx! {
                                                div { class: "rounded-md border bg-background p-2",
                                                    div { class: "grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-2",
                                                        ItemIcon { icon: item.as_ref().map(|item| item.icon).unwrap_or(0), size: "sm" }
                                                        div { class: "min-w-0",
                                                            div { class: "truncate text-sm font-medium", "{get_item_name(&data, target.item_id)}" }
                                                            if let Some(recipe) = recipe.as_ref() {
                                                                div { class: "truncate text-xs text-muted-foreground",
                                                                    "{CRAFT_TYPE_NAMES[recipe.craft_type.min(7) as usize]}"
                                                                }
                                                            }
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                                                            title: "移除",
                                                            aria_label: "移除",
                                                            onclick: {
                                                                let target_id = target.id.clone();
                                                                move |_| remove_target(target_id.clone())
                                                            },
                                                            Icon { kind: IconKind::X, class: "h-4 w-4" }
                                                        }
                                                    }
                                                    label { class: "mt-2 grid gap-1 text-xs font-medium text-muted-foreground",
                                                        "数量"
                                                        input {
                                                            r#type: "number",
                                                            min: "1",
                                                            value: "{target.amount}",
                                                            class: input_class("h-8"),
                                                            oninput: {
                                                                let target_id = target.id.clone();
                                                                move |event| {
                                                                    let amount = event.value().parse::<u32>().unwrap_or(1).max(1);
                                                                    update_target_amount(target_id.clone(), amount);
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
                }

                div { class: "flex justify-end gap-2 border-t bg-muted/30 p-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: targets().is_empty(),
                        onclick: {
                            let card = card.clone();
                            move |_| {
                                let trimmed_title = title().trim().to_string();
                                on_save.call(CraftSummaryCard {
                                    id: card.as_ref().map(|card| card.id.clone()).unwrap_or_else(id),
                                    kind: "craftSummary".to_string(),
                                    title: if trimmed_title.is_empty() { "合成汇总".to_string() } else { trimmed_title },
                                    targets: targets(),
                                    source_choices: card.as_ref().map(|card| card.source_choices.clone()).unwrap_or_default(),
                                });
                                on_close.call(());
                            }
                        },
                        "保存卡片"
                    }
                }
            }
        }
    }
}

#[component]
pub fn NotesPage() -> Element {
    let craft_data = use_resource(load_craft_data);
    let mut state = use_signal(load_notes_state);
    let mut expanded_folders = use_signal(HashSet::<String>::new);
    let mut editing_card_id = use_signal(|| None::<String>);
    let mut detail_target = use_signal(|| None::<DetailTarget>);
    let mut name_dialog = use_signal(|| None::<NameDialogKind>);
    let mut confirm_dialog = use_signal(|| None::<ConfirmDialogKind>);

    use_effect(move || {
        let current = state();
        save_notes_state(&current);
    });

    let data = craft_data
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let engine = data
        .as_ref()
        .map(|data| create_craft_data_engine(data.clone()));
    let current_state = state();
    let active_page_id = current_state.active_page_id.clone();
    let active_page = active_page_id
        .as_ref()
        .and_then(|page_id| current_state.pages.get(page_id))
        .cloned();
    let active_page_node = find_tree_node(&current_state.tree, active_page_id.as_deref()).cloned();
    let active_card_id = current_state.active_card_id.clone();
    let active_card = active_page
        .as_ref()
        .and_then(|page| {
            active_card_id
                .as_ref()
                .and_then(|card_id| page.cards.iter().find(|card| &card.id == card_id))
                .or_else(|| page.cards.first())
        })
        .cloned();
    let editing_card = editing_card_id().and_then(|card_id| {
        if card_id == "new" {
            None
        } else {
            active_page
                .as_ref()
                .and_then(|page| page.cards.iter().find(|card| card.id == card_id).cloned())
        }
    });
    let detail_recipe = detail_target().and_then(|target| {
        target.recipe.or_else(|| {
            engine
                .as_ref()
                .map(|engine| build_tree(engine, target.item_id, target.amount_needed).recipe)
                .unwrap_or(None)
        })
    });

    let mut create_folder = move |title: String, parent_id: Option<String>| {
        let folder_id = id();
        let node = NoteTreeNode {
            id: folder_id.clone(),
            kind: "folder".to_string(),
            title,
            children: Vec::new(),
        };
        let mut next = state();
        next.tree = append_tree_node(&next.tree, parent_id.as_deref(), node);
        state.set(next);
        let mut expanded = expanded_folders();
        if let Some(parent_id) = parent_id {
            expanded.insert(parent_id);
        }
        expanded.insert(folder_id);
        expanded_folders.set(expanded);
    };
    let mut create_page = move |title: String, parent_id: Option<String>| {
        let page_id = id();
        let node = NoteTreeNode {
            id: page_id.clone(),
            kind: "page".to_string(),
            title,
            children: Vec::new(),
        };
        let mut next = state();
        next.tree = append_tree_node(&next.tree, parent_id.as_deref(), node);
        next.pages
            .insert(page_id.clone(), create_default_page(page_id.clone()));
        next.active_page_id = Some(page_id.clone());
        next.active_card_id = None;
        state.set(next);
        if let Some(parent_id) = parent_id {
            let mut expanded = expanded_folders();
            expanded.insert(parent_id);
            expanded_folders.set(expanded);
        }
    };
    let mut delete_node = move |node_id: String| {
        let mut next = state();
        let (tree, removed_pages) = delete_tree_node(&next.tree, &node_id);
        for page_id in &removed_pages {
            next.pages.remove(page_id);
        }
        let active_removed = next
            .active_page_id
            .as_ref()
            .map(|page_id| removed_pages.contains(page_id))
            .unwrap_or(false);
        next.tree = tree;
        if active_removed {
            next.active_page_id = first_page_id(&next.tree);
            next.active_card_id = next
                .active_page_id
                .as_ref()
                .and_then(|page_id| next.pages.get(page_id))
                .and_then(|page| page.cards.first())
                .map(|card| card.id.clone());
        }
        state.set(next);
    };
    let mut save_card = move |card: CraftSummaryCard| {
        let mut next = state();
        let Some(page_id) = next.active_page_id.clone() else {
            return;
        };
        if let Some(page) = next.pages.get_mut(&page_id) {
            if let Some(existing) = page.cards.iter_mut().find(|item| item.id == card.id) {
                *existing = card.clone();
            } else {
                page.cards.push(card.clone());
            }
            next.active_card_id = Some(card.id);
            state.set(next);
        }
    };
    let mut delete_card = move |card_id: String| {
        let mut next = state();
        let Some(page_id) = next.active_page_id.clone() else {
            return;
        };
        if let Some(page) = next.pages.get_mut(&page_id) {
            page.cards.retain(|card| card.id != card_id);
            if next.active_card_id.as_deref() == Some(card_id.as_str()) {
                next.active_card_id = page.cards.first().map(|card| card.id.clone());
            }
            state.set(next);
        }
    };
    rsx! {
        div { class: "flex min-h-screen flex-col lg:h-screen lg:min-h-0 lg:overflow-hidden",
            div { class: "shrink-0 border-b bg-background px-4 py-4 sm:px-6 lg:px-8",
                div { class: "mx-auto flex max-w-[1720px] flex-col gap-3 xl:flex-row xl:items-end xl:justify-between",
                    div {
                        div { class: "text-sm text-muted-foreground", "工具 / 笔记" }
                        h1 { class: "text-2xl font-semibold", "笔记" }
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

            div { class: "grid w-full flex-1 lg:min-h-0 xl:grid-cols-[280px_minmax(0,1fr)] 2xl:grid-cols-[300px_minmax(0,1fr)]",
                aside { class: "flex h-[320px] flex-col overflow-hidden border-b bg-card xl:h-auto xl:min-h-0 xl:border-b-0 xl:border-r",
                    div { class: "flex items-center justify-between gap-2 border-b p-3",
                        div { class: "flex items-center gap-2 text-sm font-semibold",
                            Icon { kind: IconKind::Folder, class: "h-4 w-4" }
                            "笔记树"
                        }
                        div { class: "flex gap-1",
                            Button {
                                size: ButtonSize::Icon,
                                variant: ButtonVariant::Ghost,
                                title: Some("添加页面".to_string()),
                                onclick: move |_| name_dialog.set(Some(NameDialogKind::AddPage { parent_id: None })),
                                Icon { kind: IconKind::FilePlus2, class: "h-4 w-4" }
                            }
                            Button {
                                size: ButtonSize::Icon,
                                variant: ButtonVariant::Ghost,
                                title: Some("添加目录".to_string()),
                                onclick: move |_| name_dialog.set(Some(NameDialogKind::AddFolder { parent_id: None })),
                                Icon { kind: IconKind::FolderPlus, class: "h-4 w-4" }
                            }
                        }
                    }
                    div { class: "min-h-0 flex-1 overflow-y-auto p-2",
                        for node in current_state.tree.clone() {
                            TreeNodeRow {
                                node,
                                depth: 0,
                                active_page_id: current_state.active_page_id.clone(),
                                expanded: expanded_folders(),
                                on_toggle: move |folder_id| {
                                    let mut expanded = expanded_folders();
                                    if expanded.contains(&folder_id) {
                                        expanded.remove(&folder_id);
                                    } else {
                                        expanded.insert(folder_id);
                                    }
                                    expanded_folders.set(expanded);
                                },
                                on_select_page: move |page_id| {
                                    let mut next = state();
                                    let card_id = next.pages.get(&page_id).and_then(|page| page.cards.first()).map(|card| card.id.clone());
                                    next.active_page_id = Some(page_id);
                                    next.active_card_id = card_id;
                                    state.set(next);
                                },
                                on_add_folder: move |parent_id| name_dialog.set(Some(NameDialogKind::AddFolder { parent_id: Some(parent_id) })),
                                on_add_page: move |parent_id| name_dialog.set(Some(NameDialogKind::AddPage { parent_id: Some(parent_id) })),
                                on_rename: move |(node_id, current_title)| name_dialog.set(Some(NameDialogKind::RenameNode { node_id, current_title })),
                                on_delete: move |node_id| confirm_dialog.set(Some(ConfirmDialogKind::DeleteNode { node_id })),
                            }
                        }
                    }
                }

                main { class: "min-h-[560px] overflow-hidden bg-background lg:min-h-0",
                    if let (Some(data), Some(engine), Some(page)) = (data.as_ref(), engine.clone(), active_page.clone()) {
                        div { class: "flex h-full min-h-[560px] flex-col lg:min-h-0",
                            div { class: "flex flex-wrap items-center justify-between gap-3 border-b p-4",
                                div { class: "min-w-0",
                                    div { class: "truncate text-base font-semibold",
                                        "{active_page_node.as_ref().map(|node| node.title.as_str()).unwrap_or(\"未命名页面\")}"
                                    }
                                    div { class: "mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground",
                                        Badge { variant: BadgeVariant::Outline,
                                            Icon { kind: IconKind::Hammer, class: "mr-1 h-3 w-3" }
                                            "汇总卡片 {page.cards.len()}"
                                        }
                                    }
                                }
                                Button {
                                    variant: ButtonVariant::Primary,
                                    onclick: move |_| editing_card_id.set(Some("new".to_string())),
                                    Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                                    "添加汇总卡片"
                                }
                            }

                            div { class: "min-h-0 flex-1 overflow-y-auto p-3",
                                if page.cards.is_empty() {
                                    EmptyState {
                                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                        title: "还没有合成汇总卡片".to_string(),
                                        description: "创建一张卡片后，可以在里面搜索并多选要制作的物品。".to_string(),
                                        action: rsx! {
                                            Button {
                                                variant: ButtonVariant::Primary,
                                                onclick: move |_| editing_card_id.set(Some("new".to_string())),
                                                Icon { kind: IconKind::Plus, class: "h-4 w-4" }
                                                "添加汇总卡片"
                                            }
                                        },
                                    }
                                } else {
                                    div { class: "grid w-full gap-3",
                                        for card in page.cards.clone() {
                                            {
                                                let card_id = card.id.clone();
                                                let source_choices = choice_record_to_map(&card.source_choices);
                                                rsx! {
                                                    CraftSummaryCardView {
                                                        key: "{card.id}",
                                                        data: data.clone(),
                                                        engine: engine.clone(),
                                                        active: active_card.as_ref().map(|active| active.id.as_str()) == Some(card.id.as_str()),
                                                        source_choices,
                                                        card,
                                                        on_select: {
                                                            let card_id = card_id.clone();
                                                            move |_| {
                                                                let mut next = state();
                                                                next.active_card_id = Some(card_id.clone());
                                                                state.set(next);
                                                            }
                                                        },
                                                        on_edit: {
                                                            let card_id = card_id.clone();
                                                            move |_| editing_card_id.set(Some(card_id.clone()))
                                                        },
                                                        on_remove: {
                                                            let card_id = card_id.clone();
                                                            move |_| confirm_dialog.set(Some(ConfirmDialogKind::DeleteCard { card_id: card_id.clone() }))
                                                        },
                                                        on_toggle_collapsed_item: {
                                                            let card_id = card_id.clone();
                                                            move |(item_id, collapsed)| {
                                                                let mut next = state();
                                                                if let Some(page_id) = next.active_page_id.clone() {
                                                                    if let Some(page) = next.pages.get_mut(&page_id) {
                                                                        page.cards = page.cards.clone().into_iter().map(|card| {
                                                                            if card.id == card_id {
                                                                                CraftSummaryCard {
                                                                                    targets: card.targets.iter().map(|target| target_with_collapsed_item(target, item_id, collapsed)).collect(),
                                                                                    ..card
                                                                                }
                                                                            } else {
                                                                                card
                                                                            }
                                                                        }).collect();
                                                                    }
                                                                }
                                                                state.set(next);
                                                            }
                                                        },
                                                        on_choose_source: {
                                                            let card_id = card_id.clone();
                                                            move |(item_id, choice): (u32, Option<SourceChoice>)| {
                                                                let mut next = state();
                                                                if let Some(page_id) = next.active_page_id.clone() {
                                                                    if let Some(page) = next.pages.get_mut(&page_id) {
                                                                        if let Some(card) = page.cards.iter_mut().find(|card| card.id == card_id) {
                                                                            if let Some(choice) = choice {
                                                                                card.source_choices.insert(item_id.to_string(), choice);
                                                                            } else {
                                                                                card.source_choices.remove(&item_id.to_string());
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                state.set(next);
                                                            }
                                                        },
                                                        on_inspect: move |target| detail_target.set(Some(target)),
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        div { class: "p-4",
                            EmptyState {
                                icon: rsx! { Icon { kind: IconKind::BookOpen, class: "h-6 w-6" } },
                                title: "笔记未载入".to_string(),
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
                    on_close: move |_| detail_target.set(None),
                }
            }

            if let (Some(data), Some(engine), Some(card_id)) = (data.as_ref(), engine.clone(), editing_card_id()) {
                CraftSummaryEditorDialog {
                    data: data.clone(),
                    engine,
                    card: if card_id == "new" { None } else { editing_card.clone() },
                    on_save: move |card| save_card(card),
                    on_close: move |_| editing_card_id.set(None),
                }
            }

            if let Some(kind) = name_dialog() {
                NameDialog {
                    kind,
                    on_confirm: move |(kind, value)| {
                        match kind {
                            NameDialogKind::AddFolder { parent_id } => create_folder(value, parent_id),
                            NameDialogKind::AddPage { parent_id } => create_page(value, parent_id),
                            NameDialogKind::RenameNode { node_id, .. } => {
                                let mut next = state();
                                next.tree = rename_tree_node(&next.tree, &node_id, &value);
                                state.set(next);
                            }
                        }
                    },
                    on_close: move |_| name_dialog.set(None),
                }
            }

            if let Some(kind) = confirm_dialog() {
                {
                    let (title, description, confirm_label) = match &kind {
                        ConfirmDialogKind::DeleteNode { node_id } => {
                            let node = find_tree_node(&current_state.tree, Some(node_id));
                            let is_folder = node.map(|node| node.kind.as_str()) == Some("folder");
                            let title = if is_folder { "删除目录" } else { "删除页面" }.to_string();
                            let name = node.map(|node| node.title.clone()).unwrap_or_default();
                            let description = if is_folder {
                                format!("将删除“{name}”以及其中所有页面。")
                            } else {
                                format!("将删除页面“{name}”。")
                            };
                            (title, description, "删除")
                        }
                        ConfirmDialogKind::DeleteCard { card_id } => {
                            let name = active_page
                                .as_ref()
                                .and_then(|page| page.cards.iter().find(|card| card.id == *card_id))
                                .map(|card| card.title.clone())
                                .unwrap_or_else(|| "合成汇总".to_string());
                            ("删除合成汇总卡片".to_string(), format!("将删除“{name}”。"), "删除")
                        }
                    };
                    rsx! {
                        ConfirmDialog {
                            title,
                            description,
                            confirm_label,
                            on_confirm: {
                                let kind = kind.clone();
                                move |_| match kind.clone() {
                                    ConfirmDialogKind::DeleteNode { node_id } => delete_node(node_id),
                                    ConfirmDialogKind::DeleteCard { card_id } => delete_card(card_id),
                                }
                            },
                            on_close: move |_| confirm_dialog.set(None),
                        }
                    }
                }
            }
        }
    }
}
