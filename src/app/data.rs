use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use xiv_companion::{
    CraftDataIndex, CraftDataPackage, CraftItem, CraftRecipe, CraftTreeNode, ItemSource,
    MaterialSummary, SourceChoice, build_craft_tree,
    craftable_recipes as planner_craftable_recipes, create_craft_data_index,
    default_source_index as planner_default_source_index, get_item as planner_get_item,
    get_item_name as planner_get_item_name, resolve_source as planner_resolve_source,
    source_label as planner_source_label, source_priority as planner_source_priority,
    summarize_materials as planner_summarize_materials,
};

const CRAFT_DATA_ASSET: Asset = asset!("/assets/craft-data.json");

pub const CRAFT_TYPE_NAMES: [&str; 8] = [
    "刻木匠",
    "锻铁匠",
    "铸甲匠",
    "雕金匠",
    "制革匠",
    "裁衣匠",
    "炼金术士",
    "烹调师",
];

pub const CRAFT_TYPE_ABBRS: [&str; 8] = [
    "木工", "锻冶", "甲胄", "雕金", "皮革", "裁缝", "炼金", "烹调",
];

thread_local! {
    static CRAFT_DATA_CACHE: RefCell<Option<Rc<CraftDataPackage>>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct CraftDataEngine {
    pub data: Rc<CraftDataPackage>,
    pub index: Rc<CraftDataIndex>,
}

impl PartialEq for CraftDataEngine {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

pub async fn load_craft_data() -> Result<Rc<CraftDataPackage>, String> {
    if let Some(data) = CRAFT_DATA_CACHE.with(|cache| cache.borrow().clone()) {
        return Ok(data);
    }

    let bytes = dioxus::asset_resolver::read_asset_bytes(CRAFT_DATA_ASSET)
        .await
        .map_err(|error| format!("craft-data.json {error}"))?;

    let data = serde_json::from_slice::<CraftDataPackage>(&bytes)
        .map_err(|error| format!("craft-data.json {error}"))?;
    let data = Rc::new(data);
    CRAFT_DATA_CACHE.with(|cache| cache.replace(Some(data.clone())));
    Ok(data)
}

pub fn create_craft_data_engine(data: Rc<CraftDataPackage>) -> CraftDataEngine {
    CraftDataEngine {
        index: Rc::new(create_craft_data_index(&data)),
        data,
    }
}

pub fn craftable_recipes(
    engine: &CraftDataEngine,
    craft_type: Option<u32>,
    query: &str,
    limit: usize,
) -> Vec<CraftRecipe> {
    planner_craftable_recipes(&engine.data, &engine.index, craft_type, query, limit)
}

pub fn get_item(data: &CraftDataPackage, item_id: u32) -> Option<&CraftItem> {
    planner_get_item(data, item_id)
}

pub fn get_item_name(data: &CraftDataPackage, item_id: u32) -> String {
    planner_get_item_name(data, item_id)
}

pub fn get_icon_urls(icon_id: u32) -> Vec<String> {
    if icon_id == 0 {
        return Vec::new();
    }
    let folder = icon_id / 1000 * 1000;
    vec![
        format!("https://xivapi.com/i/{folder:06}/{icon_id:06}.png"),
        format!("https://www.garlandtools.org/files/icons/item/t/{icon_id}.png"),
        format!("https://garlandtools.org/files/icons/item/t/{icon_id}.png"),
    ]
}

pub fn build_tree(engine: &CraftDataEngine, item_id: u32, amount: u32) -> CraftTreeNode {
    build_craft_tree(item_id, amount, &engine.index)
}

pub fn summarize_materials(
    node: &CraftTreeNode,
    collapsed: &std::collections::HashSet<String>,
) -> Vec<MaterialSummary> {
    planner_summarize_materials(node, collapsed)
}

pub fn collapse_key(item_id: u32, depth: u32) -> String {
    xiv_companion::collapse_key(item_id, depth)
}

pub fn default_source_index(sources: &[ItemSource]) -> Option<usize> {
    planner_default_source_index(sources)
}

pub fn resolve_source<'a>(
    item_id: u32,
    sources: &'a [ItemSource],
    choices: &HashMap<u32, SourceChoice>,
) -> Option<&'a ItemSource> {
    planner_resolve_source(item_id, sources, choices)
}

pub fn source_label(source: &ItemSource) -> &'static str {
    planner_source_label(source)
}

pub fn source_priority(source: &ItemSource) -> u16 {
    planner_source_priority(source)
}
