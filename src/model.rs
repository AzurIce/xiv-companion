use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use tsify::Tsify;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "wasm",
    tsify(into_wasm_abi, from_wasm_abi, hashmap_as_object)
)]
pub struct CraftDataPackage {
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: CraftDataCounts,
    pub items: BTreeMap<String, CraftItem>,
    pub recipes: Vec<CraftRecipe>,
    pub recipe_levels: BTreeMap<String, RecipeLevelInfo>,
    pub secret_recipe_books: BTreeMap<String, String>,
    pub sources: BTreeMap<String, Vec<ItemSource>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CraftDataCounts {
    pub items: usize,
    pub recipes: usize,
    pub sources: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CraftItem {
    pub id: u32,
    pub name: String,
    pub icon: u32,
    pub item_ui_category: u32,
    pub item_search_category: u32,
    pub price_mid: u32,
    pub price_low: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CraftIngredient {
    pub item_id: u32,
    pub amount: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CraftRecipe {
    pub id: u32,
    pub result_item_id: u32,
    pub result_amount: u32,
    pub craft_type: u32,
    pub recipe_level_table_id: u32,
    #[serde(default)]
    pub max_level_scaling: u32,
    #[serde(default = "default_recipe_factor")]
    pub difficulty_factor: u32,
    #[serde(default = "default_recipe_factor")]
    pub quality_factor: u32,
    #[serde(default = "default_recipe_factor")]
    pub durability_factor: u32,
    #[serde(default)]
    pub required_craftsmanship: u32,
    #[serde(default)]
    pub required_control: u32,
    #[serde(default)]
    pub is_expert: bool,
    pub ingredients: Vec<CraftIngredient>,
    pub secret_recipe_book: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RecipeLevelInfo {
    pub class_job_level: u32,
    pub stars: u32,
    #[serde(default)]
    pub suggested_craftsmanship: u32,
    pub difficulty: u32,
    pub quality: u32,
    #[serde(default)]
    pub progress_divider: u32,
    #[serde(default)]
    pub quality_divider: u32,
    #[serde(default)]
    pub progress_modifier: u32,
    #[serde(default)]
    pub quality_modifier: u32,
    pub durability: u32,
    #[serde(default)]
    pub conditions_flag: u32,
}

fn default_recipe_factor() -> u32 {
    100
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ItemSource {
    GilShop {
        #[serde(rename = "shopName")]
        shop_name: String,
    },
    SpecialShop {
        #[serde(rename = "shopName")]
        shop_name: String,
        costs: Vec<SpecialShopCost>,
    },
    Fishing {
        #[serde(rename = "fishId")]
        fish_id: u32,
        #[serde(rename = "spotId")]
        spot_id: u32,
    },
    Gathering,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SpecialShopCost {
    pub item_id: u32,
    pub count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SourceChoice {
    Index { index: usize },
    Market,
    Ignore,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SourceChoiceEntry {
    pub item_id: u32,
    pub choice: SourceChoice,
}
