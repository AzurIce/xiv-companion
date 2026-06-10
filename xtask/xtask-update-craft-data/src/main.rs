use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use physis::{
    Language,
    excel::{Field, Row},
    resource::SqPackResource,
};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Export XIV Companion crafting data from a game install"
)]
struct Args {
    /// FFXIV game directory. Accepts either the install root or the inner game directory.
    #[arg(long, value_name = "DIR")]
    game_dir: Option<PathBuf>,

    /// Output directory for craft-data.json and version.json.
    #[arg(long, value_name = "DIR", default_value = "app/public")]
    out_dir: PathBuf,

    /// Only audit an existing craft-data.json without exporting.
    #[arg(long)]
    audit_only: bool,

    /// Skip the generated JSON audit after export.
    #[arg(long)]
    skip_audit: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CraftDataPackage {
    generated_at: String,
    game_version: String,
    source: String,
    counts: CraftDataCounts,
    items: BTreeMap<String, CraftItem>,
    recipes: Vec<CraftRecipe>,
    recipe_levels: BTreeMap<String, RecipeLevelInfo>,
    secret_recipe_books: BTreeMap<String, String>,
    sources: BTreeMap<String, Vec<ItemSource>>,
}

#[derive(Serialize)]
struct CraftDataCounts {
    items: usize,
    recipes: usize,
    sources: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CraftItem {
    id: u32,
    name: String,
    icon: u32,
    item_ui_category: u32,
    item_search_category: u32,
    price_mid: u32,
    price_low: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CraftIngredient {
    item_id: u32,
    amount: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CraftRecipe {
    id: u32,
    result_item_id: u32,
    result_amount: u32,
    craft_type: u32,
    recipe_level_table_id: u32,
    ingredients: Vec<CraftIngredient>,
    secret_recipe_book: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecipeLevelInfo {
    class_job_level: u32,
    stars: u32,
    difficulty: u32,
    quality: u32,
    durability: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ItemSource {
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

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecialShopCost {
    item_id: u32,
    count: u32,
}

#[derive(Serialize)]
struct VersionInfo {
    commit: String,
    date: String,
}

struct GameExcel {
    game_dir: PathBuf,
    resource: SqPackResource,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = workspace_root()?;
    let out_dir = absolutize(&root, &args.out_dir);
    let data_path = out_dir.join("craft-data.json");

    if args.audit_only {
        audit_craft_data(&data_path)?;
        return Ok(());
    }

    let game_dir = args
        .game_dir
        .as_ref()
        .ok_or_else(|| anyhow!("--game-dir is required unless --audit-only is set"))?;
    let game_dir = normalize_game_dir(game_dir)?;
    let generated_at = chrono_like_timestamp();

    let mut game = GameExcel::new(game_dir)?;
    let game_version = game_version(&game.game_dir);
    let items = game.load_items()?;
    let recipes = game.load_recipes()?;
    let recipe_levels = game.load_recipe_levels()?;
    let secret_recipe_books = game.load_secret_recipe_books()?;
    let sources = game.load_sources(&items)?;
    let source_count = sources.values().map(Vec::len).sum();

    let data = CraftDataPackage {
        generated_at: generated_at.clone(),
        game_version: game_version.clone(),
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
        sources,
    };

    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    fs::write(&data_path, serde_json::to_string(&data)?)
        .with_context(|| format!("failed to write {}", data_path.display()))?;
    fs::write(
        out_dir.join("version.json"),
        serde_json::to_string(&VersionInfo {
            commit: game_version,
            date: generated_at,
        })?,
    )
    .with_context(|| format!("failed to write {}", out_dir.join("version.json").display()))?;

    println!("Items: {}", data.counts.items);
    println!("Recipes: {}", data.counts.recipes);
    println!("Sources: {}", data.counts.sources);
    println!("Output: {}", data_path.display());

    if !args.skip_audit {
        audit_craft_data(&data_path)?;
    }

    Ok(())
}

impl GameExcel {
    fn new(game_dir: PathBuf) -> Result<Self> {
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

    fn load_items(&mut self) -> Result<BTreeMap<String, CraftItem>> {
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

    fn load_recipes(&mut self) -> Result<Vec<CraftRecipe>> {
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
                ingredients,
                secret_recipe_book: number_value(row, 40),
            });
        });

        Ok(recipes)
    }

    fn load_recipe_levels(&mut self) -> Result<BTreeMap<String, RecipeLevelInfo>> {
        let sheet = self.sheet("RecipeLevelTable", Language::None)?;
        let mut levels = BTreeMap::new();

        for_each_row(&sheet, |row_id, row| {
            levels.insert(
                row_id.to_string(),
                RecipeLevelInfo {
                    class_job_level: number_value(row, 0),
                    stars: number_value(row, 1),
                    difficulty: number_value(row, 3),
                    quality: number_value(row, 4),
                    durability: number_value(row, 9),
                },
            );
        });

        Ok(levels)
    }

    fn load_secret_recipe_books(&mut self) -> Result<BTreeMap<String, String>> {
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

    fn load_sources(
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

fn workspace_root() -> Result<PathBuf> {
    let xtask_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask_root
        .join("../..")
        .canonicalize()
        .context("failed to resolve workspace root")
}

fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn normalize_game_dir(path: &Path) -> Result<PathBuf> {
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

fn game_version(game_dir: &Path) -> String {
    fs::read_to_string(game_dir.join("ffxivgame.ver"))
        .ok()
        .map(|value| format!("game-{}", value.trim()))
        .filter(|value| value != "game-")
        .unwrap_or_else(|| "game-local".to_string())
}

fn chrono_like_timestamp() -> String {
    let output = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output();

    output
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn audit_craft_data(data_path: &Path) -> Result<()> {
    let data = fs::read_to_string(data_path)
        .with_context(|| format!("failed to read {}", data_path.display()))?;
    let data: Value = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", data_path.display()))?;

    let items = data
        .get("items")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("craft-data.json is missing items"))?;
    let sources = data
        .get("sources")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("craft-data.json is missing sources"))?;

    assert_known_exchange_costs(items, sources)?;

    let mut low_id_costs = Vec::new();
    let mut missing_cost_items = Vec::new();
    for (receive_item_id, source_list) in sources {
        let Some(source_list) = source_list.as_array() else {
            continue;
        };
        for source in source_list {
            if source.get("kind").and_then(Value::as_str) != Some("specialShop") {
                continue;
            }
            let shop_name = source
                .get("shopName")
                .and_then(Value::as_str)
                .unwrap_or("兑换");
            let Some(costs) = source.get("costs").and_then(Value::as_array) else {
                continue;
            };
            for cost in costs {
                let Some(cost_item_id) = cost.get("itemId").and_then(Value::as_u64) else {
                    continue;
                };
                if (2..=17).contains(&cost_item_id) {
                    low_id_costs.push(format!(
                        "{} {} -> {} x{} @ {}",
                        receive_item_id,
                        item_name(items, receive_item_id.parse().unwrap_or_default()),
                        item_name(items, cost_item_id),
                        cost.get("count")
                            .and_then(Value::as_u64)
                            .unwrap_or_default(),
                        shop_name,
                    ));
                }
                if !items.contains_key(&cost_item_id.to_string()) {
                    missing_cost_items.push(format!(
                        "{} has missing cost item {} @ {}",
                        receive_item_id, cost_item_id, shop_name
                    ));
                }
            }
        }
    }

    if !low_id_costs.is_empty() {
        bail!(
            "found suspicious shard/crystal SpecialShop costs:\n{}",
            low_id_costs
                .into_iter()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    if !missing_cost_items.is_empty() {
        bail!(
            "found SpecialShop costs with missing item rows:\n{}",
            missing_cost_items
                .into_iter()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    println!("craft data audit passed: no suspicious low-id SpecialShop costs");
    Ok(())
}

fn assert_known_exchange_costs(
    items: &Map<String, Value>,
    sources: &Map<String, Value>,
) -> Result<()> {
    let checks = [
        ("克罗诺兽的粗皮", "亚拉戈天道神典石", 20),
        ("克罗诺兽的粗皮", "亚拉戈数理神典石", 10),
        ("高浓缩炼金药", "巧手橙票", 125),
        ("高浓缩炼金药", "宇宙信用点", 250),
    ];

    let ids_by_name = audit_item_ids_by_name(items);
    for (item_name, cost_name, cost_count) in checks {
        let item_id = ids_by_name
            .get(item_name)
            .copied()
            .ok_or_else(|| anyhow!("missing known item {item_name}"))?;
        let cost_item_id = ids_by_name
            .get(cost_name)
            .copied()
            .ok_or_else(|| anyhow!("missing known cost item {cost_name}"))?;

        if !has_special_shop_cost(sources, item_id, cost_item_id, cost_count) {
            bail!("{item_name} is missing expected exchange cost {cost_name} x{cost_count}");
        }
    }

    Ok(())
}

fn audit_item_ids_by_name(items: &Map<String, Value>) -> BTreeMap<String, u64> {
    items
        .iter()
        .filter_map(|(id, item)| {
            let id = id.parse().ok()?;
            let name = item.get("name")?.as_str()?.to_string();
            Some((name, id))
        })
        .collect()
}

fn item_name(items: &Map<String, Value>, item_id: u64) -> String {
    items
        .get(&item_id.to_string())
        .and_then(|item| item.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("#{item_id}"))
}

fn has_special_shop_cost(
    sources: &Map<String, Value>,
    item_id: u64,
    cost_item_id: u64,
    cost_count: u64,
) -> bool {
    sources
        .get(&item_id.to_string())
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|source| source.get("kind").and_then(Value::as_str) == Some("specialShop"))
        .flat_map(|source| {
            source
                .get("costs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .any(|cost| {
            cost.get("itemId").and_then(Value::as_u64) == Some(cost_item_id)
                && cost.get("count").and_then(Value::as_u64) == Some(cost_count)
        })
}
