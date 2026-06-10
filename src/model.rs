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
    #[serde(default)]
    pub macro_action_names: BTreeMap<String, String>,
    pub sources: BTreeMap<String, Vec<ItemSource>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacroActionDefinition {
    pub key: &'static str,
    pub game_action_id: u32,
    pub macro_name_source: MacroActionNameSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroActionNameSource {
    Action(u32),
    CraftAction(u32),
    GeneralAction(u32),
}

pub const MACRO_ACTION_DEFINITIONS: &[MacroActionDefinition] = &[
    MacroActionDefinition {
        key: "basic_synthesis",
        game_action_id: 100001,
        macro_name_source: MacroActionNameSource::CraftAction(100001),
    },
    MacroActionDefinition {
        key: "basic_touch",
        game_action_id: 100002,
        macro_name_source: MacroActionNameSource::CraftAction(100002),
    },
    MacroActionDefinition {
        key: "masters_mend",
        game_action_id: 100003,
        macro_name_source: MacroActionNameSource::CraftAction(100003),
    },
    MacroActionDefinition {
        key: "observe",
        game_action_id: 100010,
        macro_name_source: MacroActionNameSource::CraftAction(100010),
    },
    MacroActionDefinition {
        key: "tricks_of_the_trade",
        game_action_id: 100371,
        macro_name_source: MacroActionNameSource::CraftAction(100371),
    },
    MacroActionDefinition {
        key: "waste_not",
        game_action_id: 4631,
        macro_name_source: MacroActionNameSource::Action(4631),
    },
    MacroActionDefinition {
        key: "veneration",
        game_action_id: 19297,
        macro_name_source: MacroActionNameSource::Action(19297),
    },
    MacroActionDefinition {
        key: "standard_touch",
        game_action_id: 100004,
        macro_name_source: MacroActionNameSource::CraftAction(100004),
    },
    MacroActionDefinition {
        key: "great_strides",
        game_action_id: 260,
        macro_name_source: MacroActionNameSource::Action(260),
    },
    MacroActionDefinition {
        key: "innovation",
        game_action_id: 19004,
        macro_name_source: MacroActionNameSource::Action(19004),
    },
    MacroActionDefinition {
        key: "waste_not_ii",
        game_action_id: 4639,
        macro_name_source: MacroActionNameSource::Action(4639),
    },
    MacroActionDefinition {
        key: "byregots_blessing",
        game_action_id: 100339,
        macro_name_source: MacroActionNameSource::CraftAction(100339),
    },
    MacroActionDefinition {
        key: "precise_touch",
        game_action_id: 100128,
        macro_name_source: MacroActionNameSource::CraftAction(100128),
    },
    MacroActionDefinition {
        key: "muscle_memory",
        game_action_id: 100379,
        macro_name_source: MacroActionNameSource::CraftAction(100379),
    },
    MacroActionDefinition {
        key: "careful_synthesis",
        game_action_id: 100203,
        macro_name_source: MacroActionNameSource::CraftAction(100203),
    },
    MacroActionDefinition {
        key: "manipulation",
        game_action_id: 4574,
        macro_name_source: MacroActionNameSource::Action(4574),
    },
    MacroActionDefinition {
        key: "prudent_touch",
        game_action_id: 100227,
        macro_name_source: MacroActionNameSource::CraftAction(100227),
    },
    MacroActionDefinition {
        key: "advanced_touch",
        game_action_id: 100411,
        macro_name_source: MacroActionNameSource::CraftAction(100411),
    },
    MacroActionDefinition {
        key: "reflect",
        game_action_id: 100387,
        macro_name_source: MacroActionNameSource::CraftAction(100387),
    },
    MacroActionDefinition {
        key: "preparatory_touch",
        game_action_id: 100299,
        macro_name_source: MacroActionNameSource::CraftAction(100299),
    },
    MacroActionDefinition {
        key: "groundwork",
        game_action_id: 100403,
        macro_name_source: MacroActionNameSource::CraftAction(100403),
    },
    MacroActionDefinition {
        key: "delicate_synthesis",
        game_action_id: 100323,
        macro_name_source: MacroActionNameSource::CraftAction(100323),
    },
    MacroActionDefinition {
        key: "intensive_synthesis",
        game_action_id: 100315,
        macro_name_source: MacroActionNameSource::CraftAction(100315),
    },
    MacroActionDefinition {
        key: "trained_eye",
        game_action_id: 100283,
        macro_name_source: MacroActionNameSource::CraftAction(100283),
    },
    MacroActionDefinition {
        key: "heart_and_soul",
        game_action_id: 100419,
        macro_name_source: MacroActionNameSource::CraftAction(100419),
    },
    MacroActionDefinition {
        key: "prudent_synthesis",
        game_action_id: 100427,
        macro_name_source: MacroActionNameSource::CraftAction(100427),
    },
    MacroActionDefinition {
        key: "trained_finesse",
        game_action_id: 100435,
        macro_name_source: MacroActionNameSource::CraftAction(100435),
    },
    MacroActionDefinition {
        key: "refined_touch",
        game_action_id: 100443,
        macro_name_source: MacroActionNameSource::CraftAction(100443),
    },
    MacroActionDefinition {
        key: "quick_innovation",
        game_action_id: 100459,
        macro_name_source: MacroActionNameSource::CraftAction(100459),
    },
    MacroActionDefinition {
        key: "immaculate_mend",
        game_action_id: 100467,
        macro_name_source: MacroActionNameSource::CraftAction(100467),
    },
    MacroActionDefinition {
        key: "trained_perfection",
        game_action_id: 100475,
        macro_name_source: MacroActionNameSource::CraftAction(100475),
    },
    MacroActionDefinition {
        key: "stellar_steady_hand",
        game_action_id: 46843,
        macro_name_source: MacroActionNameSource::GeneralAction(27),
    },
    MacroActionDefinition {
        key: "rapid_synthesis",
        game_action_id: 100363,
        macro_name_source: MacroActionNameSource::CraftAction(100363),
    },
    MacroActionDefinition {
        key: "hasty_touch",
        game_action_id: 100355,
        macro_name_source: MacroActionNameSource::CraftAction(100355),
    },
    MacroActionDefinition {
        key: "daring_touch",
        game_action_id: 100451,
        macro_name_source: MacroActionNameSource::CraftAction(100355),
    },
];

pub fn macro_action_key_by_game_action_id(game_action_id: u32) -> Option<&'static str> {
    MACRO_ACTION_DEFINITIONS
        .iter()
        .find(|definition| definition.game_action_id == game_action_id)
        .map(|definition| definition.key)
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
