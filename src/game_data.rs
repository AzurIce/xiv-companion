use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use physis::{
    Language,
    excel::{Field, Row},
    resource::SqPackResource,
};

use crate::{
    CraftDataCounts, CraftDataPackage, CraftIngredient, CraftItem, CraftRecipe, ItemSource,
    MACRO_ACTION_DEFINITIONS, MacroActionNameSource, RecipeLevelInfo, SpecialShopCost,
};

pub struct GameExcel {
    game_dir: PathBuf,
    resource: SqPackResource,
}

pub fn export_craft_data(game_dir: &Path, generated_at: String) -> Result<CraftDataPackage> {
    let game_dir = normalize_game_dir(game_dir)?;
    let mut game = GameExcel::new(game_dir)?;
    let game_version = game_version(&game.game_dir);
    let items = game.load_items()?;
    let recipes = game.load_recipes()?;
    let recipe_levels = game.load_recipe_levels()?;
    let secret_recipe_books = game.load_secret_recipe_books()?;
    let macro_action_names = game.load_macro_action_names()?;
    let sources = game.load_sources(&items)?;
    let source_count = sources.values().map(Vec::len).sum();

    Ok(CraftDataPackage {
        generated_at,
        game_version,
        source: game.game_dir.display().to_string(),
        counts: CraftDataCounts {
            items: items.len(),
            recipes: recipes.len(),
            sources: source_count,
        },
        items,
        recipes,
        recipe_levels,
        secret_recipe_books,
        macro_action_names,
        sources,
    })
}

impl GameExcel {
    pub fn new(game_dir: PathBuf) -> Result<Self> {
        let game_dir_string = game_dir
            .to_str()
            .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
        Ok(Self {
            resource: SqPackResource::from_existing(game_dir_string),
            game_dir,
        })
    }

    fn sheet(&mut self, name: &str, language: Language) -> Result<physis::excel::Sheet> {
        let header = self
            .resource
            .read_excel_sheet_header(name)
            .with_context(|| format!("failed to read {name} sheet header"))?;
        self.resource
            .read_excel_sheet(&header, name, language)
            .with_context(|| format!("failed to read {name} sheet"))
    }

    pub fn load_items(&mut self) -> Result<BTreeMap<String, CraftItem>> {
        let sheet = self.sheet("Item", Language::ChineseSimplified)?;
        let mut items = BTreeMap::new();

        for_each_row(&sheet, |row_id, row| {
            let Some(name) = string_value(row, 0) else {
                return;
            };
            if name.is_empty() {
                return;
            }
            items.insert(
                row_id.to_string(),
                CraftItem {
                    id: row_id,
                    name: name.to_owned(),
                    icon: number_value(row, 10),
                    item_ui_category: number_value(row, 15),
                    item_search_category: number_value(row, 16),
                    price_mid: number_value(row, 25),
                    price_low: number_value(row, 26),
                },
            );
        });

        Ok(items)
    }

    pub fn load_recipes(&mut self) -> Result<Vec<CraftRecipe>> {
        let sheet = self.sheet("Recipe", Language::None)?;
        let mut recipes = Vec::new();

        for_each_row(&sheet, |row_id, row| {
            let result_item_id = number_value(row, 4);
            if result_item_id == 0 {
                return;
            }

            let ingredients = (0..8)
                .filter_map(|i| {
                    let item_id = number_value(row, 6 + i * 2);
                    let amount = number_value(row, 7 + i * 2);
                    (item_id != 0 && amount != 0).then_some(CraftIngredient { item_id, amount })
                })
                .collect::<Vec<_>>();

            if ingredients.is_empty() {
                return;
            }

            recipes.push(CraftRecipe {
                id: row_id,
                result_item_id,
                result_amount: number_value(row, 5).max(1),
                craft_type: number_value(row, 1),
                recipe_level_table_id: number_value(row, 2),
                max_level_scaling: number_value(row, 3),
                difficulty_factor: defaulted_number_value(row, 26, 100),
                quality_factor: defaulted_number_value(row, 27, 100),
                durability_factor: defaulted_number_value(row, 28, 100),
                required_craftsmanship: number_value(row, 30),
                required_control: number_value(row, 31),
                is_expert: bool_value(row, 43),
                ingredients,
                secret_recipe_book: number_value(row, 40),
            });
        });

        Ok(recipes)
    }

    pub fn load_recipe_levels(&mut self) -> Result<BTreeMap<String, RecipeLevelInfo>> {
        let sheet = self.sheet("RecipeLevelTable", Language::None)?;
        let mut levels = BTreeMap::new();

        for_each_row(&sheet, |row_id, row| {
            levels.insert(
                row_id.to_string(),
                RecipeLevelInfo {
                    class_job_level: number_value(row, 0),
                    stars: number_value(row, 1),
                    suggested_craftsmanship: number_value(row, 2),
                    difficulty: number_value(row, 3),
                    quality: number_value(row, 4),
                    progress_divider: number_value(row, 5),
                    quality_divider: number_value(row, 6),
                    progress_modifier: number_value(row, 7),
                    quality_modifier: number_value(row, 8),
                    durability: number_value(row, 9),
                    conditions_flag: number_value(row, 10),
                },
            );
        });

        Ok(levels)
    }

    pub fn load_secret_recipe_books(&mut self) -> Result<BTreeMap<String, String>> {
        let sheet = self.sheet("SecretRecipeBook", Language::ChineseSimplified)?;
        let mut books = BTreeMap::new();

        for_each_row(&sheet, |row_id, row| {
            let item_id = number_value(row, 0);
            let Some(name) = string_value(row, 1) else {
                return;
            };
            if item_id == 0 || name.is_empty() {
                return;
            }
            books.insert(row_id.to_string(), name.to_owned());
            books.insert(item_id.to_string(), name.to_owned());
            books.insert((row_id + 546).to_string(), name.to_owned());
        });

        Ok(books)
    }

    pub fn load_macro_action_names(&mut self) -> Result<BTreeMap<String, String>> {
        let action_names = self.load_row_names("Action")?;
        let craft_action_names = self.load_row_names("CraftAction")?;
        let general_action_names = self.load_row_names("GeneralAction")?;
        let mut names = BTreeMap::new();

        for definition in MACRO_ACTION_DEFINITIONS {
            let name = match definition.macro_name_source {
                MacroActionNameSource::Action(row_id) => action_names.get(&row_id),
                MacroActionNameSource::CraftAction(row_id) => craft_action_names.get(&row_id),
                MacroActionNameSource::GeneralAction(row_id) => general_action_names.get(&row_id),
            };
            if let Some(name) = name.filter(|name| !name.is_empty()) {
                names.insert(definition.key.to_string(), name.to_owned());
            }
        }

        Ok(names)
    }

    fn load_row_names(&mut self, sheet_name: &str) -> Result<HashMap<u32, String>> {
        let sheet = self.sheet(sheet_name, Language::ChineseSimplified)?;
        let mut names = HashMap::new();
        for_each_row(&sheet, |row_id, row| {
            if let Some(name) = string_value(row, 0).filter(|name| !name.is_empty()) {
                names.insert(row_id, name.to_owned());
            }
        });
        Ok(names)
    }

    pub fn load_sources(
        &mut self,
        items: &BTreeMap<String, CraftItem>,
    ) -> Result<BTreeMap<String, Vec<ItemSource>>> {
        let mut sources = BTreeMap::new();
        self.load_gathering_sources(&mut sources)?;
        self.load_fishing_sources(&mut sources, items)?;
        self.load_gil_shop_sources(&mut sources)?;
        self.load_special_shop_sources(&mut sources, items)?;
        Ok(sources)
    }

    fn load_gathering_sources(
        &mut self,
        sources: &mut BTreeMap<String, Vec<ItemSource>>,
    ) -> Result<()> {
        let sheet = self.sheet("GatheringItem", Language::None)?;
        for_each_row(&sheet, |_row_id, row| {
            add_source(sources, number_value(row, 0), ItemSource::Gathering);
        });
        Ok(())
    }

    fn load_fishing_sources(
        &mut self,
        sources: &mut BTreeMap<String, Vec<ItemSource>>,
        items: &BTreeMap<String, CraftItem>,
    ) -> Result<()> {
        let fish_item_ids = items
            .values()
            .filter(|item| item.item_ui_category == 47)
            .map(|item| item.id)
            .collect::<HashSet<_>>();

        let fishing_spot = self.sheet("FishingSpot", Language::ChineseSimplified);
        if let Ok(sheet) = fishing_spot.or_else(|_| self.sheet("FishingSpot", Language::None)) {
            for_each_row(&sheet, |spot_id, row| {
                let mut row_items = HashSet::new();
                for field in &row.columns {
                    let item_id = field_number_value(field);
                    if fish_item_ids.contains(&item_id) {
                        row_items.insert(item_id);
                    }
                }
                for item_id in row_items {
                    add_source(
                        sources,
                        item_id,
                        ItemSource::Fishing {
                            fish_id: item_id,
                            spot_id,
                        },
                    );
                }
            });
        }

        Ok(())
    }

    fn load_gil_shop_sources(
        &mut self,
        sources: &mut BTreeMap<String, Vec<ItemSource>>,
    ) -> Result<()> {
        let gil_shop = self.sheet("GilShop", Language::ChineseSimplified)?;
        let mut shop_names = HashMap::new();
        for_each_row(&gil_shop, |row_id, row| {
            if let Some(name) = string_value(row, 0) {
                shop_names.insert(row_id, name.to_owned());
            }
        });

        let gil_shop_item = self.sheet("GilShopItem", Language::None)?;
        for_each_row(&gil_shop_item, |shop_id, row| {
            let item_id = number_value(row, 0);
            let shop_name = shop_names
                .get(&shop_id)
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| "金币商店".to_string());
            add_source(sources, item_id, ItemSource::GilShop { shop_name });
        });

        Ok(())
    }

    fn load_special_shop_sources(
        &mut self,
        sources: &mut BTreeMap<String, Vec<ItemSource>>,
        items: &BTreeMap<String, CraftItem>,
    ) -> Result<()> {
        let sheet = self.sheet("SpecialShop", Language::ChineseSimplified)?;
        let names = item_ids_by_name(items);

        for_each_row(&sheet, |_row_id, row| {
            let shop_name = string_value(row, 0).unwrap_or("兑换");
            if shop_name.contains("测试") {
                return;
            }

            let use_currency_type = number_value(row, 2041);
            let cost_groups = [(481, 541), (721, 781), (961, 1021)];

            for i in 0..60 {
                let receive_item_id = number_value(row, 1 + i);
                if receive_item_id == 0 {
                    continue;
                }

                let costs = cost_groups
                    .iter()
                    .filter_map(|(item_base, count_base)| {
                        let item_id = number_value(row, item_base + i);
                        let count = number_value(row, count_base + i);
                        (item_id != 0 && count != 0).then(|| SpecialShopCost {
                            item_id: resolve_special_shop_cost_item_id(
                                shop_name,
                                use_currency_type,
                                item_id,
                                &names,
                            ),
                            count,
                        })
                    })
                    .collect::<Vec<_>>();

                if !costs.is_empty() {
                    add_source(
                        sources,
                        receive_item_id,
                        ItemSource::SpecialShop {
                            shop_name: shop_name.to_owned(),
                            costs,
                        },
                    );
                }
            }
        });

        Ok(())
    }
}

fn for_each_row(sheet: &physis::excel::Sheet, mut f: impl FnMut(u32, &Row)) {
    for page in &sheet.pages {
        for (row_id, row) in page.into_iter().flatten_subrows() {
            f(row_id, row);
        }
    }
}

fn string_value(row: &Row, col: usize) -> Option<&str> {
    match row.columns.get(col) {
        Some(Field::String(value)) => Some(value.as_str()),
        _ => None,
    }
}

fn number_value(row: &Row, col: usize) -> u32 {
    row.columns
        .get(col)
        .map(field_number_value)
        .unwrap_or_default()
}

fn defaulted_number_value(row: &Row, col: usize, default: u32) -> u32 {
    match number_value(row, col) {
        0 => default,
        value => value,
    }
}

fn bool_value(row: &Row, col: usize) -> bool {
    matches!(row.columns.get(col), Some(Field::Bool(true)))
}

fn field_number_value(field: &Field) -> u32 {
    match field {
        Field::UInt8(value) => *value as u32,
        Field::UInt16(value) => *value as u32,
        Field::UInt32(value) => *value,
        Field::Int8(value) if *value > 0 => *value as u32,
        Field::Int16(value) if *value > 0 => *value as u32,
        Field::Int32(value) if *value > 0 => *value as u32,
        _ => 0,
    }
}

fn add_source(sources: &mut BTreeMap<String, Vec<ItemSource>>, item_id: u32, source: ItemSource) {
    if item_id == 0 {
        return;
    }
    let entry = sources.entry(item_id.to_string()).or_default();
    if !entry.contains(&source) {
        entry.push(source);
    }
}

fn item_ids_by_name(items: &BTreeMap<String, CraftItem>) -> HashMap<&str, u32> {
    items
        .values()
        .map(|item| (item.name.as_str(), item.id))
        .collect()
}

fn resolve_name_id(names: &HashMap<&str, u32>, name: &str) -> Option<u32> {
    names.get(name.replace('"', "").as_str()).copied()
}

fn resolve_special_shop_cost_item_id(
    shop_name: &str,
    use_currency_type: u32,
    cost_item_id: u32,
    names: &HashMap<&str, u32>,
) -> u32 {
    let clean_name = shop_name.replace('"', "");

    if clean_name.contains("巧手白票") {
        return resolve_name_id(names, "巧手白票").unwrap_or(cost_item_id);
    }
    if clean_name.contains("大地白票") {
        return resolve_name_id(names, "大地白票").unwrap_or(cost_item_id);
    }
    if clean_name.contains("巧手工票") {
        return resolve_name_id(names, "制作蓝票的票据").unwrap_or(cost_item_id);
    }
    if clean_name.contains("大地工票") {
        return resolve_name_id(names, "采集蓝票的票据").unwrap_or(cost_item_id);
    }

    if let Some(tomestone) = clean_name
        .split("亚拉戈")
        .nth(1)
        .and_then(|tail| tail.split('神').next())
    {
        let name = format!("亚拉戈{tomestone}神典石");
        if let Some(item_id) = resolve_name_id(names, &name) {
            return item_id;
        }
    }

    if use_currency_type == 4 || use_currency_type == 2 {
        return match cost_item_id {
            1 => resolve_name_id(names, "亚拉戈诗学神典石").unwrap_or(28),
            2 => resolve_name_id(names, "亚拉戈数理神典石").unwrap_or(cost_item_id),
            3 => resolve_name_id(names, "亚拉戈记忆神典石").unwrap_or(cost_item_id),
            _ => cost_item_id,
        };
    }

    if use_currency_type != 16 {
        return cost_item_id;
    }

    match cost_item_id {
        1 => 28,
        2 => 33913,
        4 => 33914,
        6 => 41784,
        7 => 41785,
        _ => cost_item_id,
    }
}

pub fn normalize_game_dir(path: &Path) -> Result<PathBuf> {
    let path = expand_tilde(path);
    if path.join("sqpack").is_dir() {
        return path
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    if path.join("game").join("sqpack").is_dir() {
        return path
            .join("game")
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.join("game").display()));
    }
    bail!(
        "failed to find sqpack under {} or {}/game",
        path.display(),
        path.display()
    )
}

fn expand_tilde(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if text == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_path_buf()
}

pub fn game_version(game_dir: &Path) -> String {
    fs::read_to_string(game_dir.join("ffxivgame.ver"))
        .ok()
        .map(|value| format!("game-{}", value.trim()))
        .filter(|value| value != "game-")
        .unwrap_or_else(|| "game-local".to_string())
}
