use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::app::resource_settings::{
    ResourceSettings, configured_web_resource_hub, configured_web_resource_hub_for,
    load_resource_settings,
};
use xiv_companion::{
    CraftDataId, CraftDataIndex, CraftDataPackage, CraftDataResource, CraftItem, CraftRecipe,
    CraftTreeNode, ItemIconResourceInfo, ItemSource, MaterialSummary, ResourceSource, SourceChoice,
    build_craft_tree, builtin_icon_urls, craftable_recipes as planner_craftable_recipes,
    create_craft_data_index, default_source_index as planner_default_source_index,
    get_item as planner_get_item, get_item_name as planner_get_item_name,
    resolve_source as planner_resolve_source, source_label as planner_source_label,
    source_priority as planner_source_priority, summarize_materials as planner_summarize_materials,
};

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
    static ITEM_ICON_CACHE: RefCell<HashMap<String, ItemIconResourceInfo>> = RefCell::new(HashMap::new());
}

#[derive(Clone)]
pub struct CraftDataEngine {
    pub data: Rc<CraftDataPackage>,
    pub index: Rc<CraftDataIndex>,
}

#[derive(Clone, PartialEq)]
pub struct LoadedCraftData {
    pub source: ResourceSource,
    pub data: Rc<CraftDataPackage>,
}

impl PartialEq for CraftDataEngine {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

pub async fn load_craft_data_with_source(
    settings: ResourceSettings,
) -> Result<LoadedCraftData, String> {
    let loaded = configured_web_resource_hub_for(&settings)
        .load_with_source::<CraftDataResource>(CraftDataId::Default)
        .await
        .map_err(|error| error.to_string())?;
    Ok(LoadedCraftData {
        source: loaded.source,
        data: Rc::new(loaded.value),
    })
}

pub async fn load_craft_data() -> Result<Rc<CraftDataPackage>, String> {
    let data = configured_web_resource_hub()
        .load::<CraftDataResource>(CraftDataId::Default)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Rc::new(data))
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

pub async fn load_item_icon(icon_id: u32) -> Result<ItemIconResourceInfo, String> {
    let settings = load_resource_settings();
    let settings_key = serde_json::to_string(&settings).unwrap_or_default();
    let cache_key = format!("{}|{}", icon_id, settings_key);
    if let Some(info) = ITEM_ICON_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned()) {
        return Ok(info);
    }

    let info = ItemIconResourceInfo {
        icon_id,
        urls: builtin_icon_urls(icon_id),
        local_image: None,
    };
    ITEM_ICON_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, info.clone());
    });
    Ok(info)
}

pub fn clear_item_icon_cache() {
    ITEM_ICON_CACHE.with(|cache| cache.borrow_mut().clear());
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
