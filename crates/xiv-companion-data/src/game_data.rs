use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use physis::{
    Language,
    excel::{Field, Row},
    resource::{Resource, SqPackResource},
};

use crate::{
    COLLECTION_CATALOG_SCHEMA_VERSION, CollectionCatalogCounts, CollectionCatalogPackage,
    CollectionClassificationAudit, CollectionClassificationInput, CollectionItem, CollectionKind,
    CraftDataCounts, CraftDataPackage, CraftIngredient, CraftItem, CraftRecipe, ItemSource,
    MACRO_ACTION_DEFINITIONS, MacroActionNameSource, RecipeLevelInfo, SpecialShopCost,
    WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage, WeaponStain,
    classify_collection_item, is_weapon_equip_slot_category,
};

pub struct GameExcel<R: Resource> {
    source_label: String,
    game_version: String,
    resource: R,
}

pub fn export_craft_data(game_dir: &Path, generated_at: String) -> Result<CraftDataPackage> {
    let game_dir = normalize_game_dir(game_dir)?;
    let game_version = game_version(&game_dir);
    let source_label = game_dir.display().to_string();
    let resource = SqPackResource::from_existing(
        game_dir
            .to_str()
            .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?,
    );
    export_craft_data_from_resource(resource, source_label, game_version, generated_at)
}

pub fn export_weapon_catalog(
    game_dir: &Path,
    generated_at: String,
) -> Result<WeaponCatalogPackage> {
    let game_dir = normalize_game_dir(game_dir)?;
    let game_version = game_version(&game_dir);
    let source_label = game_dir.display().to_string();
    let resource = SqPackResource::from_existing(
        game_dir
            .to_str()
            .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?,
    );
    export_weapon_catalog_from_resource(resource, source_label, game_version, generated_at)
}

pub fn export_collection_catalog(
    game_dir: &Path,
    generated_at: String,
) -> Result<CollectionCatalogPackage> {
    let game_dir = normalize_game_dir(game_dir)?;
    let game_version = game_version(&game_dir);
    let source_label = game_dir.display().to_string();
    let resource = SqPackResource::from_existing(
        game_dir
            .to_str()
            .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?,
    );
    export_collection_catalog_from_resource(resource, source_label, game_version, generated_at)
}

pub fn export_craft_data_from_resource<R: Resource>(
    resource: R,
    source_label: String,
    game_version: String,
    generated_at: String,
) -> Result<CraftDataPackage> {
    let mut game = GameExcel::new(resource, source_label, game_version);
    let items = game.load_items()?;
    let recipes = game.load_recipes()?;
    let recipe_levels = game.load_recipe_levels()?;
    let secret_recipe_books = game.load_secret_recipe_books()?;
    let macro_action_names = game.load_macro_action_names()?;
    let sources = game.load_sources(&items)?;
    let source_count = sources.values().map(Vec::len).sum();

    Ok(CraftDataPackage {
        generated_at,
        game_version: game.game_version.clone(),
        source: game.source_label.clone(),
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

pub fn export_weapon_catalog_from_resource<R: Resource>(
    resource: R,
    source_label: String,
    game_version: String,
    generated_at: String,
) -> Result<WeaponCatalogPackage> {
    let mut game = GameExcel::new(resource, source_label, game_version);
    let items = game.load_weapon_catalog_items()?;
    let stains = game.load_weapon_stains()?;

    Ok(WeaponCatalogPackage {
        generated_at,
        game_version: game.game_version.clone(),
        source: game.source_label.clone(),
        counts: WeaponCatalogCounts {
            items: items.len(),
            stains: stains.len(),
        },
        stains,
        items,
    })
}

pub fn export_collection_catalog_from_resource<R: Resource>(
    resource: R,
    source_label: String,
    game_version: String,
    generated_at: String,
) -> Result<CollectionCatalogPackage> {
    let mut game = GameExcel::new(resource, source_label, game_version);
    let class_job_categories = game.load_named_rows("ClassJobCategory")?;
    let item_action_types = game.load_item_action_types()?;
    let explicit_equipment_sets = game.load_explicit_equipment_sets()?;
    let mut classification_audit = CollectionClassificationAudit::default();
    let mut items = game.load_collection_items(
        &class_job_categories,
        &item_action_types,
        &mut classification_audit,
    )?;
    if !classification_audit.is_conserved() || classification_audit.candidate_count != items.len() {
        bail!(
            "collection classification is not conserved: candidates={}, classified={}, items={}",
            classification_audit.candidate_count,
            classification_audit.counts_by_kind.values().sum::<usize>(),
            items.len()
        );
    }
    eprintln!(
        "collection classification audit: {} candidates; OtherUnlock by ItemAction.Type: {:?}",
        classification_audit.candidate_count, classification_audit.other_unlocks_by_action_type
    );
    assign_equipment_sets(&mut items, &explicit_equipment_sets);
    sort_collection_items(&mut items);
    let mut counts = CollectionCatalogCounts::default();
    counts.items = items.len();
    for item in &items {
        match item.kind {
            CollectionKind::Equipment => counts.equipment += 1,
            CollectionKind::OrchestrionRoll => counts.orchestrion_rolls += 1,
            CollectionKind::Mount => counts.mounts += 1,
            CollectionKind::Minion => counts.minions += 1,
            CollectionKind::FashionAccessory => counts.fashion_accessories += 1,
            CollectionKind::Emote => counts.emotes += 1,
            CollectionKind::AestheticianStyle => counts.aesthetician_styles += 1,
            CollectionKind::RidingMap => counts.riding_maps += 1,
            CollectionKind::MahjongSupport => counts.mahjong_supports += 1,
            CollectionKind::PortraitDesign => counts.portrait_designs += 1,
            CollectionKind::TripleTriadCard => counts.triple_triad_cards += 1,
            CollectionKind::ChocoboBarding => counts.chocobo_bardings += 1,
            CollectionKind::Facewear => counts.facewear += 1,
            CollectionKind::MasterRecipe => counts.master_recipes += 1,
            CollectionKind::OtherUnlock => counts.other_unlocks += 1,
            CollectionKind::FolkloreBook => counts.folklore_books += 1,
        }
    }

    Ok(CollectionCatalogPackage {
        schema_version: COLLECTION_CATALOG_SCHEMA_VERSION,
        generated_at,
        game_version: game.game_version.clone(),
        source: game.source_label.clone(),
        counts,
        items,
    })
}

impl<R: Resource> GameExcel<R> {
    pub fn new(resource: R, source_label: String, game_version: String) -> Self {
        Self {
            source_label,
            game_version,
            resource,
        }
    }

    fn sheet(&mut self, name: &str, language: Language) -> Result<physis::excel::Sheet> {
        let header = physis::resource::generic_read_excel_sheet_header(&mut self.resource, name)
            .with_context(|| format!("failed to read {name} sheet header"))?;
        physis::resource::generic_read_excel_sheet(&mut self.resource, &header, name, language)
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

    pub fn load_weapon_catalog_items(&mut self) -> Result<Vec<WeaponCatalogItem>> {
        let sheet = self.sheet("Item", Language::ChineseSimplified)?;
        let mut items = Vec::new();

        for_each_row(&sheet, |row_id, row| {
            let Some(name) = string_value(row, 0) else {
                return;
            };
            if name.is_empty() {
                return;
            }

            let equip_slot_category = number_value(row, 17);
            let model_main = model_id_value(row, 47);
            if model_main == 0 || !is_weapon_equip_slot_category(equip_slot_category) {
                return;
            }

            items.push(WeaponCatalogItem {
                id: row_id,
                name: name.to_owned(),
                description: string_value(row, 8).unwrap_or_default().to_owned(),
                icon: number_value(row, 10),
                item_ui_category: number_value(row, 15),
                item_search_category: number_value(row, 16),
                equip_slot_category,
                price_mid: number_value(row, 25),
                price_low: number_value(row, 26),
                model_main,
                model_sub: model_id_value(row, 48),
            });
        });

        items.sort_by(|a, b| {
            a.item_ui_category
                .cmp(&b.item_ui_category)
                .then(a.id.cmp(&b.id))
        });
        Ok(items)
    }

    pub fn load_weapon_stains(&mut self) -> Result<Vec<WeaponStain>> {
        let sheet = self.sheet("Stain", Language::ChineseSimplified)?;
        let mut stains = Vec::new();

        for_each_row(&sheet, |row_id, row| {
            let Ok(id) = u8::try_from(row_id) else {
                return;
            };
            if id == 0 {
                return;
            }
            let Some(name) = string_value(row, 3).filter(|name| !name.is_empty()) else {
                return;
            };
            let se_color = number_value(row, 0);
            stains.push(WeaponStain {
                id,
                name: name.to_owned(),
                se_color,
                ui_color: se_color_to_rgba(se_color),
                shade: number_value(row, 1) as u8,
                sub_order: number_value(row, 2) as u8,
                metallic: bool_value(row, 5),
            });
        });

        stains.sort_by_key(|stain| (stain.shade, stain.sub_order, stain.id));
        Ok(stains)
    }

    pub fn load_named_rows(&mut self, sheet_name: &str) -> Result<HashMap<u32, String>> {
        let sheet = self.sheet(sheet_name, Language::ChineseSimplified)?;
        let mut names = HashMap::new();
        for_each_row(&sheet, |row_id, row| {
            if let Some(name) = string_value(row, 0).filter(|name| !name.is_empty()) {
                names.insert(row_id, name.to_owned());
            }
        });
        Ok(names)
    }

    pub fn load_item_action_types(&mut self) -> Result<HashMap<u32, u32>> {
        let sheet = self.sheet("ItemAction", Language::None)?;
        let mut actions = HashMap::new();
        for_each_row(&sheet, |row_id, row| {
            let action_type = number_value(row, 4);
            if action_type != 0 {
                actions.insert(row_id, action_type);
            }
        });
        Ok(actions)
    }

    fn load_explicit_equipment_sets(&mut self) -> Result<Vec<EquipmentSetDefinition>> {
        let fitting_sheet = self.sheet("FittingShopItemSet", Language::ChineseSimplified)?;
        let mut sets = Vec::new();
        for_each_row(&fitting_sheet, |row_id, row| {
            let item_ids = (0..6)
                .map(|column| number_value(row, column))
                .filter(|item_id| *item_id != 0)
                .collect::<Vec<_>>();
            let name = string_value(row, 6)
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned);
            if item_ids.len() >= 2 {
                sets.push(EquipmentSetDefinition {
                    id: format!("fitting:{row_id}"),
                    name,
                    item_ids,
                });
            }
        });

        let mirage_sheet = self.sheet("MirageStoreSetItem", Language::None)?;
        for_each_row(&mirage_sheet, |row_id, row| {
            let item_ids = (2..=10)
                .map(|column| number_value(row, column))
                .filter(|item_id| *item_id != 0)
                .collect::<Vec<_>>();
            if item_ids.len() >= 2 {
                sets.push(EquipmentSetDefinition {
                    id: format!("mirage:{row_id}"),
                    name: None,
                    item_ids,
                });
            }
        });
        Ok(sets)
    }

    pub fn load_collection_items(
        &mut self,
        class_job_category_names: &HashMap<u32, String>,
        item_action_types: &HashMap<u32, u32>,
        classification_audit: &mut CollectionClassificationAudit,
    ) -> Result<Vec<CollectionItem>> {
        let sheet = self.sheet("Item", Language::ChineseSimplified)?;
        let mut items = Vec::new();

        for_each_row(&sheet, |row_id, row| {
            let Some(name) = string_value(row, 0) else {
                return;
            };
            if name.is_empty() {
                return;
            }
            if is_obsolete_legacy_item_name(name) {
                return;
            }

            let item_search_category = number_value(row, 16);
            let equip_slot_category = number_value(row, 17);
            let item_action = number_value(row, 30);
            let action_type = item_action_types
                .get(&item_action)
                .copied()
                .unwrap_or_default();
            let item_ui_category = number_value(row, 15);
            let Some(kind) = classify_collection_item(CollectionClassificationInput {
                name,
                equip_slot_category,
                item_action_type: action_type,
                item_ui_category,
            }) else {
                return;
            };
            if kind == CollectionKind::Equipment && equip_slot_category == 17 {
                return;
            }
            classification_audit.record(kind, action_type);
            let model_main = model_id_value(row, 47);
            let item_series = number_value(row, 45);
            let class_job_category = number_value(row, 43);
            let (slot_name, slot_order) = equipment_slot(equip_slot_category);
            let (set_id, set_name) = if kind == CollectionKind::Equipment {
                (format!("item:{row_id}"), name.to_owned())
            } else {
                (format!("{}:{row_id}", kind.id()), kind.label().to_string())
            };

            items.push(CollectionItem {
                id: row_id,
                kind,
                name: name.to_owned(),
                description: string_value(row, 8).unwrap_or_default().to_owned(),
                icon: number_value(row, 10),
                item_ui_category,
                item_search_category,
                item_action,
                equip_slot_category,
                slot_name: slot_name.to_string(),
                slot_order,
                level_item: number_value(row, 11) as u16,
                level_equip: number_value(row, 40) as u16,
                rarity: number_value(row, 12) as u8,
                class_job_category,
                class_job_category_name: class_job_category_names
                    .get(&class_job_category)
                    .cloned()
                    .unwrap_or_default(),
                item_series,
                set_id,
                set_name,
                expansion: "未归档".to_string(),
                patch: "未归档版本".to_string(),
                model_main,
                model_sub: model_id_value(row, 48),
                appearance_key: (model_main != 0)
                    .then(|| {
                        format!(
                            "equipment:{}:{model_main}",
                            equipment_model_domain(equip_slot_category)
                        )
                    })
                    .unwrap_or_default(),
            });
        });

        sort_collection_items(&mut items);
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

fn is_obsolete_legacy_item_name(name: &str) -> bool {
    name.starts_with("过期")
}

fn equipment_slot(equip_slot_category: u32) -> (&'static str, u8) {
    match equip_slot_category {
        1 | 13 | 14 => ("武器", 0),
        2 => ("副手", 1),
        3 => ("头部", 2),
        4 => ("身体", 3),
        5 => ("手部", 4),
        6 => ("腰部", 5),
        7 => ("腿部", 6),
        8 => ("脚部", 7),
        9 => ("耳饰", 8),
        10 => ("项链", 9),
        11 => ("手镯", 10),
        12 => ("戒指", 11),
        _ => ("复合部位", 12),
    }
}

#[derive(Clone, Debug)]
struct EquipmentSetDefinition {
    id: String,
    name: Option<String>,
    item_ids: Vec<u32>,
}

fn assign_equipment_sets(items: &mut [CollectionItem], explicit_sets: &[EquipmentSetDefinition]) {
    let indices_by_id = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id, index))
        .collect::<HashMap<_, _>>();
    let mut assigned = HashSet::new();

    // FittingShopItemSet has canonical display names and is intentionally loaded first.
    // MirageStoreSetItem supplies additional game-defined multi-piece combinations.
    for definition in explicit_sets {
        let indices = definition
            .item_ids
            .iter()
            .filter_map(|item_id| indices_by_id.get(item_id).copied())
            .filter(|index| items[*index].kind == CollectionKind::Equipment)
            .collect::<Vec<_>>();
        if indices.iter().any(|index| assigned.contains(index))
            || distinct_equipment_slots(items, &indices) < 2
        {
            continue;
        }
        let name = definition
            .name
            .clone()
            .unwrap_or_else(|| inferred_set_name(items, &indices));
        for index in indices {
            items[index].set_id.clone_from(&definition.id);
            items[index].set_name.clone_from(&name);
            assigned.insert(index);
        }
    }

    // The game has no single sheet covering ordinary dungeon, raid, tomestone, and crafted
    // armor sets. Derive those families from stable item metadata plus a shared localized name
    // prefix. Model ids are deliberately excluded: model reuse is appearance data, not set data.
    let mut buckets: HashMap<(u16, u16, u32, u8, String), Vec<usize>> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if item.kind == CollectionKind::Equipment
            && !assigned.contains(&index)
            && (2..=11).contains(&item.slot_order)
        {
            buckets
                .entry((
                    item.level_item,
                    item.level_equip,
                    item.class_job_category,
                    item.rarity,
                    equipment_variant_marker(&item.name).to_string(),
                ))
                .or_default()
                .push(index);
        }
    }

    for indices in buckets.values() {
        let mut prefixes = HashSet::new();
        for (position, &left_index) in indices.iter().enumerate() {
            for &right_index in &indices[position + 1..] {
                if items[left_index].slot_order == items[right_index].slot_order {
                    continue;
                }
                let prefix = common_name_prefix(&items[left_index].name, &items[right_index].name);
                if is_meaningful_set_prefix(prefix) {
                    prefixes.insert(prefix);
                }
            }
        }

        let mut candidates = prefixes
            .into_iter()
            .filter_map(|prefix| {
                let members = indices
                    .iter()
                    .copied()
                    .filter(|index| items[*index].name.starts_with(&prefix))
                    .collect::<Vec<_>>();
                let slot_count = distinct_equipment_slots(items, &members);
                (slot_count >= 2 && members.len() <= 12).then_some((prefix, members, slot_count))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then(right.0.chars().count().cmp(&left.0.chars().count()))
                .then(right.1.len().cmp(&left.1.len()))
        });

        for (prefix, members, _) in candidates {
            let members = members
                .into_iter()
                .filter(|index| !assigned.contains(index))
                .collect::<Vec<_>>();
            if distinct_equipment_slots(items, &members) < 2 || members.len() > 12 {
                continue;
            }
            let min_item_id = members
                .iter()
                .map(|index| items[*index].id)
                .min()
                .unwrap_or_default();
            let set_id = format!("family:{min_item_id}");
            let set_name = format!("{prefix}套装");
            for index in members {
                items[index].set_id.clone_from(&set_id);
                items[index].set_name.clone_from(&set_name);
                assigned.insert(index);
            }
        }
    }
}

fn distinct_equipment_slots(items: &[CollectionItem], indices: &[usize]) -> usize {
    indices
        .iter()
        .map(|index| items[*index].slot_order)
        .collect::<HashSet<_>>()
        .len()
}

fn inferred_set_name(items: &[CollectionItem], indices: &[usize]) -> String {
    let prefix = indices
        .iter()
        .map(|index| items[*index].name.as_str())
        .reduce(common_name_prefix)
        .unwrap_or_default();
    if prefix.chars().count() >= 2 {
        format!("{prefix}套装")
    } else {
        let first = indices
            .first()
            .map(|index| items[*index].name.as_str())
            .unwrap_or("装备");
        format!("{first}等 {} 件", indices.len())
    }
}

fn common_name_prefix<'a>(left: &'a str, right: &str) -> &'a str {
    let mut end = 0;
    for ((left_offset, left_char), right_char) in left.char_indices().zip(right.chars()) {
        if left_char != right_char {
            break;
        }
        end = left_offset + left_char.len_utf8();
    }
    left[..end].trim_end_matches([' ', '-', '·', '・', '（', '('])
}

fn is_meaningful_set_prefix(prefix: &str) -> bool {
    prefix.chars().count() >= 2
        && !matches!(
            prefix,
            "过期" | "风化" | "陈旧" | "旧化" | "旧化的" | "改良型" | "复制品"
        )
}

fn equipment_variant_marker(name: &str) -> &str {
    if name.ends_with('）') {
        if let Some(start) = name.rfind('（') {
            return &name[start..];
        }
    }
    if let Some(start) = name.rfind('+') {
        if name[start + 1..]
            .chars()
            .all(|character| character.is_ascii_digit())
        {
            return &name[start..];
        }
    }
    for suffix in ["·改", "·阳", "·阴"] {
        if name.ends_with(suffix) {
            return suffix;
        }
    }
    ""
}

fn sort_collection_items(items: &mut [CollectionItem]) {
    items.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.patch.cmp(&right.patch))
            .then(left.set_name.cmp(&right.set_name))
            .then(left.slot_order.cmp(&right.slot_order))
            .then(left.id.cmp(&right.id))
    });
}

fn equipment_model_domain(equip_slot_category: u32) -> &'static str {
    if matches!(equip_slot_category, 1 | 2 | 13 | 14) {
        "weapon"
    } else if matches!(equip_slot_category, 9..=12) {
        "accessory"
    } else {
        "gear"
    }
}

fn for_each_row(sheet: &physis::excel::Sheet, mut f: impl FnMut(u32, &Row)) {
    for page in &sheet.pages {
        for (row_id, row) in page.into_iter().flatten_subrows() {
            f(row_id, row);
        }
    }
}

#[cfg(test)]
mod collection_tests {
    use super::*;

    fn collection_kind(
        name: &str,
        equip_slot_category: u32,
        item_action_type: u32,
        item_ui_category: u32,
    ) -> Option<CollectionKind> {
        classify_collection_item(CollectionClassificationInput {
            name,
            equip_slot_category,
            item_action_type,
            item_ui_category,
        })
    }

    fn unlock_collection_kind(name: &str) -> CollectionKind {
        collection_kind(name, 0, 2_633, 0).expect("2633 is a permanent unlock action")
    }

    #[test]
    fn classifies_item_action_collection_types() {
        assert_eq!(
            collection_kind("宠物", 0, 853, 0),
            Some(CollectionKind::Minion)
        );
        assert_eq!(
            collection_kind("坐骑", 0, 1_322, 0),
            Some(CollectionKind::Mount)
        );
        assert_eq!(
            collection_kind("演技教材·挥手", 0, 2_633, 0),
            Some(CollectionKind::Emote)
        );
        assert_eq!(
            collection_kind("肖像教材：骑士", 0, 29_459, 61),
            Some(CollectionKind::PortraitDesign)
        );
        assert_eq!(
            collection_kind("九宫幻卡：渡渡鸟", 0, 3_357, 86),
            Some(CollectionKind::TripleTriadCard)
        );
        assert_eq!(
            collection_kind("陆行鸟黑魔装甲", 0, 1_013, 63),
            Some(CollectionKind::ChocoboBarding)
        );
        assert_eq!(
            collection_kind("面部配饰：椭圆眼镜", 0, 37_312, 61),
            Some(CollectionKind::Facewear)
        );
        assert_eq!(
            collection_kind("木工秘籍第一卷", 0, 2_136, 63),
            Some(CollectionKind::MasterRecipe)
        );
        assert_eq!(
            collection_kind("第1赛季福者之证", 0, 18_083, 61),
            Some(CollectionKind::OtherUnlock)
        );
        assert_eq!(
            collection_kind("装备", 4, 0, 0),
            Some(CollectionKind::Equipment)
        );
        assert_eq!(collection_kind("普通物品", 0, 0, 0), None);
    }

    #[test]
    fn classifies_generic_unlock_items_by_collection_semantics() {
        assert_eq!(
            unlock_collection_kind("发型样式：马尾辫"),
            CollectionKind::AestheticianStyle
        );
        assert_eq!(
            unlock_collection_kind("雷克兰德详细地图"),
            CollectionKind::RidingMap
        );
        assert_eq!(
            unlock_collection_kind("天青图腾·白风"),
            CollectionKind::OtherUnlock
        );
        assert_eq!(
            unlock_collection_kind("方城金句集：阿尔菲诺"),
            CollectionKind::MahjongSupport
        );
        assert_eq!(
            unlock_collection_kind("肖像教材：随身神典石1"),
            CollectionKind::PortraitDesign
        );
        assert_eq!(
            unlock_collection_kind("魔法树建造许可证书"),
            CollectionKind::OtherUnlock
        );
        assert_eq!(
            unlock_collection_kind("2018年度群狼盛宴区域锦标赛冠军之证"),
            CollectionKind::OtherUnlock
        );
        assert_eq!(
            unlock_collection_kind("以太摆锤"),
            CollectionKind::OtherUnlock
        );
    }

    #[test]
    fn excludes_obsolete_legacy_items_from_collection_catalog() {
        assert!(is_obsolete_legacy_item_name("过期亚麻无檐帽"));
        assert!(!is_obsolete_legacy_item_name("亚麻无檐帽"));
    }

    #[test]
    fn groups_named_equipment_families_across_slots() {
        let mut items = vec![
            equipment_item(10, "伊甸之恩御敌战盔", 2),
            equipment_item(11, "伊甸之恩御敌战铠", 3),
            equipment_item(12, "伊甸之恩御敌手铠", 4),
        ];
        assign_equipment_sets(&mut items, &[]);
        assert_eq!(items[0].set_id, "family:10");
        assert_eq!(items[0].set_name, "伊甸之恩御敌套装");
        assert!(items.iter().all(|item| item.set_id == items[0].set_id));
    }

    #[test]
    fn model_reuse_does_not_create_an_equipment_set() {
        let mut items = vec![
            equipment_item(20, "红铜头环", 2),
            equipment_item(21, "旅行者长衣", 3),
        ];
        items[0].model_main = 777;
        items[1].model_main = 777;
        assign_equipment_sets(&mut items, &[]);
        assert_eq!(items[0].set_id, "item:20");
        assert_eq!(items[1].set_id, "item:21");
    }

    #[test]
    fn explicit_game_set_wins_over_inferred_family() {
        let mut items = vec![
            equipment_item(30, "东方公子长衫", 3),
            equipment_item(31, "东方公子长裤", 6),
        ];
        let definitions = vec![EquipmentSetDefinition {
            id: "fitting:1".to_string(),
            name: Some("东方公子套装".to_string()),
            item_ids: vec![30, 31],
        }];
        assign_equipment_sets(&mut items, &definitions);
        assert!(items.iter().all(|item| item.set_id == "fitting:1"));
        assert!(items.iter().all(|item| item.set_name == "东方公子套装"));
    }

    #[test]
    fn inferred_sets_keep_upgrade_variants_separate() {
        let mut items = vec![
            equipment_item(40, "元素御敌头盔+1", 2),
            equipment_item(41, "元素御敌战甲+1", 3),
            equipment_item(42, "元素御敌头盔+2", 2),
            equipment_item(43, "元素御敌战甲+2", 3),
        ];
        assign_equipment_sets(&mut items, &[]);
        assert_eq!(items[0].set_id, items[1].set_id);
        assert_eq!(items[2].set_id, items[3].set_id);
        assert_ne!(items[0].set_id, items[2].set_id);
    }

    fn equipment_item(id: u32, name: &str, slot_order: u8) -> CollectionItem {
        CollectionItem {
            id,
            kind: CollectionKind::Equipment,
            name: name.to_string(),
            description: String::new(),
            icon: 0,
            item_ui_category: 0,
            item_search_category: 0,
            item_action: 0,
            equip_slot_category: slot_order as u32,
            slot_name: format!("部位 {slot_order}"),
            slot_order,
            level_item: 470,
            level_equip: 80,
            rarity: 3,
            class_job_category: 2,
            class_job_category_name: "防护职业".to_string(),
            item_series: 0,
            set_id: format!("item:{id}"),
            set_name: name.to_string(),
            expansion: String::new(),
            patch: String::new(),
            model_main: 0,
            model_sub: 0,
            appearance_key: String::new(),
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

fn model_id_value(row: &Row, col: usize) -> u64 {
    match row.columns.get(col) {
        Some(Field::UInt64(value)) => *value,
        Some(field) => field_number_value(field) as u64,
        None => 0,
    }
}

fn se_color_to_rgba(color: u32) -> [u8; 4] {
    [
        (color & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        ((color >> 16) & 0xff) as u8,
        0xff,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_color_to_rgba_matches_meddle_bgr_conversion() {
        assert_eq!(se_color_to_rgba(0x12_34_56), [0x56, 0x34, 0x12, 0xff]);
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn loads_installed_game_stains() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let resource = SqPackResource::from_existing(&game_dir);
        let catalog = export_weapon_catalog_from_resource(
            resource,
            game_dir,
            "installed".to_string(),
            "test".to_string(),
        )
        .expect("installed weapon catalog");
        let stains = &catalog.stains;

        eprintln!(
            "stains: count={}, first={:#?}, last={:#?}",
            stains.len(),
            stains.first(),
            stains.last()
        );
        assert_eq!(catalog.counts.items, catalog.items.len());
        assert_eq!(catalog.counts.stains, stains.len());
        assert!(!catalog.items.is_empty());
        assert!(stains.len() >= 100);
        assert!(stains.iter().all(|stain| !stain.name.is_empty()));
        assert!(stains.iter().all(|stain| stain.id <= 254));
        assert!(stains.windows(2).all(|pair| {
            (pair[0].shade, pair[0].sub_order, pair[0].id)
                <= (pair[1].shade, pair[1].sub_order, pair[1].id)
        }));
        assert!(stains.iter().any(|stain| stain.metallic));
        let metallic_gold = stains
            .iter()
            .find(|stain| stain.id == 113)
            .expect("metallic gold stain");
        assert_eq!(metallic_gold.name, "闪耀金");
        assert!(metallic_gold.metallic);
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
