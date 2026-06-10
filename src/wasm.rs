use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use crate::{
    CraftDataIndex, CraftDataPackage, CraftRecipe, CraftTreeNode, CrafterAttributes, ItemSource,
    MacroSolveResult, MaterialSummary, RaphaelSolveOptions, RecipeLevelInfo, SourceChoiceEntry,
    build_craft_tree as planner_build_craft_tree, craftable_recipes as planner_craftable_recipes,
    create_craft_data_index,
    default_source_index as planner_default_source_index, resolve_source as planner_resolve_source,
    solve_raphael_macro as solver_solve_raphael_macro, source_label as planner_source_label,
    source_priority as planner_source_priority,
    summarize_materials as planner_summarize_materials,
};

#[wasm_bindgen]
pub struct CraftDataEngine {
    data: CraftDataPackage,
    index: CraftDataIndex,
}

#[wasm_bindgen]
impl CraftDataEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(data: CraftDataPackage) -> CraftDataEngine {
        let index = create_craft_data_index(&data);
        Self { data, index }
    }

    #[wasm_bindgen(js_name = craftableRecipes)]
    pub fn craftable_recipes(
        &self,
        craft_type: Option<u32>,
        query: &str,
        limit: usize,
    ) -> Vec<CraftRecipe> {
        planner_craftable_recipes(&self.data, &self.index, craft_type, query, limit)
    }

    #[wasm_bindgen(js_name = buildCraftTree)]
    pub fn build_craft_tree(&self, item_id: u32, amount: u32) -> CraftTreeNode {
        planner_build_craft_tree(item_id, amount, &self.index)
    }
}

#[wasm_bindgen(js_name = summarizeMaterials)]
pub fn summarize_materials(tree: CraftTreeNode, collapsed: Vec<String>) -> Vec<MaterialSummary> {
    let collapsed = collapsed.into_iter().collect();
    planner_summarize_materials(&tree, &collapsed)
}

#[wasm_bindgen(js_name = defaultSourceIndex)]
pub fn default_source_index(sources: Vec<ItemSource>) -> Option<u32> {
    planner_default_source_index(&sources).map(|index| index as u32)
}

#[wasm_bindgen(js_name = resolveSource)]
pub fn resolve_source(
    item_id: u32,
    sources: Vec<ItemSource>,
    choices: Vec<SourceChoiceEntry>,
) -> Option<ItemSource> {
    let choices = choices
        .into_iter()
        .map(|entry| (entry.item_id, entry.choice))
        .collect::<HashMap<_, _>>();
    planner_resolve_source(item_id, &sources, &choices).cloned()
}

#[wasm_bindgen(js_name = sourceLabel)]
pub fn source_label(source: ItemSource) -> String {
    planner_source_label(&source).to_string()
}

#[wasm_bindgen(js_name = sourcePriority)]
pub fn source_priority(source: ItemSource) -> u8 {
    planner_source_priority(&source)
}

#[wasm_bindgen(js_name = solveRaphaelMacro)]
pub fn solve_raphael_macro(
    recipe: CraftRecipe,
    recipe_level: RecipeLevelInfo,
    attrs: CrafterAttributes,
    options: RaphaelSolveOptions,
) -> Result<MacroSolveResult, JsValue> {
    solver_solve_raphael_macro(&recipe, &recipe_level, &attrs, &options)
        .map_err(|message| JsValue::from_str(&message))
}
