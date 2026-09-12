use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc as WasmRc};

use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[cfg(target_arch = "wasm32")]
use xiv_companion::PreparedModelOptions;

use crate::app::icons::{Icon, IconKind};
#[cfg(target_arch = "wasm32")]
use crate::app::model_canvas_renderer::WebWeaponCanvasRenderer;
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, GitHubRepoButton,
    input_class,
};
use crate::app::utils::{cx, format_integer};
use xiv_companion::renderer::{ModelDebugMode, ModelGlassBlendMode, WeaponRenderOptions};

use xiv_companion::{
    CharaCatalogItem, CharaCatalogPackage, CharaModelKind, CharaModelType,
    CollectionCatalogPackage, CollectionItem, EQUIPMENT_MODEL_FALLBACK_RACE_ID,
    FurnitureCatalogItem, FurnitureCatalogPackage, FurnitureModelKind, ModelAnimationSet,
    ModelAttributeOption, ModelSkeleton, PackedCharaModelId, PackedEquipmentModelId, PackedModelId,
    WeaponModelData, WeaponModelTextureKind, WeaponStain, equipment_slot_info,
    is_weapon_equip_slot_category, model_attribute_options, weapon_slot_label,
};

use super::crafting::ItemIcon;
use crate::app::data::{
    load_chara_catalog, load_chara_model_with_animation_assets, load_collection_catalog,
    load_equipment_model, load_furniture_catalog, load_furniture_model, load_weapon_catalog,
    load_weapon_model, load_weapon_staining_templates, stain_weapon_model,
};
use crate::app::load_progress::{self, WeaponModelLoadProgress};

/// 动画播放状态：模型 + 骨架 + 动画集齐备时由页面派生传入画布；画布 rAF
/// 循环每帧读取（选中动画时 `sample(t) → update_joint_matrices`）。
/// 身份比较按 Rc 指针——状态随模型资源/选中项变化整体替换。
#[derive(Clone)]
pub(crate) struct AnimationPlaybackState {
    pub set: std::rc::Rc<ModelAnimationSet>,
    pub skeleton: std::rc::Rc<ModelSkeleton>,
    pub selected: Option<usize>,
}

impl PartialEq for AnimationPlaybackState {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.set, &other.set)
            && std::rc::Rc::ptr_eq(&self.skeleton, &other.skeleton)
            && self.selected == other.selected
    }
}

/// `Rc<ModelAnimationSet>` 的组件 props 包装：相等按 Rc 身份比较（动画集随
/// 加载整体替换，语义深比较无意义且昂贵）。
#[derive(Clone, Debug)]
pub(crate) struct AnimationSetHandle(pub Rc<ModelAnimationSet>);

impl PartialEq for AnimationSetHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

const RESULT_LIMIT: usize = 220;
const WEAPON_MODELS_ROUTE_PATH: &str = "/weapon-models";
const EQUIPMENT_MODELS_ROUTE_PATH: &str = "/equipment-models";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelItemFilter {
    All,
    Weapons,
    Main,
    Off,
    TwoHanded,
    Dual,
    Armor,
    Head,
    Body,
    Hands,
    Legs,
    Feet,
    Accessories,
    Ears,
    Neck,
    Wrists,
    Rings,
    Furniture,
    Yard,
    Minions,
    Mounts,
}

impl ModelItemFilter {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Weapons => "weapons",
            Self::Main => "main",
            Self::Off => "off",
            Self::TwoHanded => "two",
            Self::Dual => "dual",
            Self::Armor => "armor",
            Self::Head => "head",
            Self::Body => "body",
            Self::Hands => "hands",
            Self::Legs => "legs",
            Self::Feet => "feet",
            Self::Accessories => "accessories",
            Self::Ears => "ears",
            Self::Neck => "neck",
            Self::Wrists => "wrists",
            Self::Rings => "rings",
            Self::Furniture => "furniture",
            Self::Yard => "yard",
            Self::Minions => "minions",
            Self::Mounts => "mounts",
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn from_key(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "weapons" | "weapon" => Some(Self::Weapons),
            "main" => Some(Self::Main),
            "off" => Some(Self::Off),
            "two" | "two-handed" | "twohanded" => Some(Self::TwoHanded),
            "dual" => Some(Self::Dual),
            "armor" => Some(Self::Armor),
            "head" => Some(Self::Head),
            "body" => Some(Self::Body),
            "hands" => Some(Self::Hands),
            "legs" => Some(Self::Legs),
            "feet" => Some(Self::Feet),
            "accessories" | "accessory" => Some(Self::Accessories),
            "ears" => Some(Self::Ears),
            "neck" => Some(Self::Neck),
            "wrists" => Some(Self::Wrists),
            "rings" | "ring" => Some(Self::Rings),
            "furniture" => Some(Self::Furniture),
            "yard" => Some(Self::Yard),
            "minions" | "minion" => Some(Self::Minions),
            "mounts" | "mount" => Some(Self::Mounts),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Weapons => "武器",
            Self::Main => "主手",
            Self::Off => "副手",
            Self::TwoHanded => "双手",
            Self::Dual => "双持",
            Self::Armor => "防具",
            Self::Head => "头部",
            Self::Body => "身体",
            Self::Hands => "手部",
            Self::Legs => "腿部",
            Self::Feet => "脚部",
            Self::Accessories => "饰品",
            Self::Ears => "耳饰",
            Self::Neck => "项链",
            Self::Wrists => "手镯",
            Self::Rings => "戒指",
            Self::Furniture => "家具",
            Self::Yard => "庭具",
            Self::Minions => "宠物",
            Self::Mounts => "坐骑",
        }
    }

    /// 组级过滤器（武器/防具/饰品）在二级槽位行里显示为“全部”。
    fn chip_label(self) -> &'static str {
        match self {
            Self::Weapons | Self::Armor | Self::Accessories => "全部",
            _ => self.label(),
        }
    }

    /// 当前过滤器所属分组的二级槽位选项（grid 列 class + 选项）；
    /// `全部` 与家具/庭具/宠物/坐骑没有二级行。
    fn sub_filters(self) -> Option<(&'static str, &'static [ModelItemFilter])> {
        const WEAPON_SUB_FILTERS: &[ModelItemFilter] = &[
            ModelItemFilter::Weapons,
            ModelItemFilter::Main,
            ModelItemFilter::Off,
            ModelItemFilter::TwoHanded,
            ModelItemFilter::Dual,
        ];
        const ARMOR_SUB_FILTERS: &[ModelItemFilter] = &[
            ModelItemFilter::Armor,
            ModelItemFilter::Head,
            ModelItemFilter::Body,
            ModelItemFilter::Hands,
            ModelItemFilter::Legs,
            ModelItemFilter::Feet,
        ];
        const ACCESSORY_SUB_FILTERS: &[ModelItemFilter] = &[
            ModelItemFilter::Accessories,
            ModelItemFilter::Ears,
            ModelItemFilter::Neck,
            ModelItemFilter::Wrists,
            ModelItemFilter::Rings,
        ];
        match self {
            Self::Weapons | Self::Main | Self::Off | Self::TwoHanded | Self::Dual => {
                Some(("grid-cols-5", WEAPON_SUB_FILTERS))
            }
            Self::Armor | Self::Head | Self::Body | Self::Hands | Self::Legs | Self::Feet => {
                Some(("grid-cols-6", ARMOR_SUB_FILTERS))
            }
            Self::Accessories | Self::Ears | Self::Neck | Self::Wrists | Self::Rings => {
                Some(("grid-cols-5", ACCESSORY_SUB_FILTERS))
            }
            Self::All | Self::Furniture | Self::Yard | Self::Minions | Self::Mounts => None,
        }
    }

    /// 该过滤器是否覆盖装备（含武器）条目。
    fn includes_equipment(self) -> bool {
        !matches!(
            self,
            Self::Furniture | Self::Yard | Self::Minions | Self::Mounts
        )
    }

    /// 该过滤器是否覆盖家具/庭具条目。
    fn includes_furniture(self) -> bool {
        matches!(self, Self::All | Self::Furniture | Self::Yard)
    }

    /// 该过滤器是否覆盖宠物/坐骑条目。
    fn includes_chara(self) -> bool {
        matches!(self, Self::All | Self::Minions | Self::Mounts)
    }

    fn matches_equipment(self, item: &CollectionItem) -> bool {
        let category = item.equip_slot_category;
        match self {
            Self::All => true,
            Self::Weapons => is_weapon_equip_slot_category(category),
            Self::Main => category == 1,
            Self::Off => category == 2,
            Self::TwoHanded => category == 13,
            Self::Dual => category == 14,
            Self::Armor => matches!(category, 3 | 4 | 5 | 7 | 8),
            Self::Head => category == 3,
            Self::Body => category == 4,
            Self::Hands => category == 5,
            Self::Legs => category == 7,
            Self::Feet => category == 8,
            Self::Accessories => matches!(category, 9..=12),
            Self::Ears => category == 9,
            Self::Neck => category == 10,
            Self::Wrists => category == 11,
            Self::Rings => category == 12,
            Self::Furniture | Self::Yard | Self::Minions | Self::Mounts => false,
        }
    }

    fn matches_furniture(self, item: &FurnitureCatalogItem) -> bool {
        match self {
            Self::All => true,
            Self::Furniture => item.kind == FurnitureModelKind::Indoor,
            Self::Yard => item.kind == FurnitureModelKind::Outdoor,
            _ => false,
        }
    }

    fn matches_chara(self, item: &CharaCatalogItem) -> bool {
        match self {
            Self::All => true,
            Self::Minions => item.kind == CharaModelKind::Minion,
            Self::Mounts => item.kind == CharaModelKind::Mount,
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq)]
struct ModelSearchResult {
    total: usize,
    items: Vec<ModelCatalogItem>,
}

#[derive(Clone, Debug)]
struct ModelPreviewUrlState {
    query: String,
    filter: ModelItemFilter,
    item_id: Option<u32>,
    stain_ids: [u8; 2],
    race_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModelRequestKey {
    item_id: u32,
    race_id: u16,
    stain_ids: [u8; 2],
}

#[derive(Clone)]
struct ModelResourceResult {
    item_id: u32,
    race_id: u16,
    result: Result<Rc<WeaponModelData>, String>,
    /// 宠物/坐骑的 rest pose 骨架（sklb 缺失为 None）。
    skeleton: Option<Rc<ModelSkeleton>>,
    /// 宠物/坐骑的动画集（pap 候选全缺为 None）。
    animations: Option<Rc<ModelAnimationSet>>,
}

#[derive(Clone)]
struct ModelPreviewResult {
    key: ModelRequestKey,
    result: Result<Rc<WeaponModelData>, String>,
    skeleton: Option<Rc<ModelSkeleton>>,
    animations: Option<Rc<ModelAnimationSet>>,
}

impl PartialEq for ModelPreviewResult {
    fn eq(&self, other: &Self) -> bool {
        if self.key != other.key {
            return false;
        }
        let result_eq = match (&self.result, &other.result) {
            (Ok(left), Ok(right)) => Rc::ptr_eq(left, right),
            (Err(left), Err(right)) => left == right,
            _ => false,
        };
        result_eq
            && optional_rc_eq(&self.skeleton, &other.skeleton)
            && optional_rc_eq(&self.animations, &other.animations)
    }
}

fn optional_rc_eq<T>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
        _ => false,
    }
}

/// 可预览条目的统一视图：装备（含武器，来自图鉴目录）、家具/庭具（家具目录）
/// 或宠物/坐骑（chara 目录）。
#[derive(Clone, PartialEq)]
enum ModelCatalogItem {
    Equipment(CollectionItem),
    Furniture(FurnitureCatalogItem),
    Chara(CharaCatalogItem),
}

impl ModelCatalogItem {
    fn id(&self) -> u32 {
        match self {
            Self::Equipment(item) => item.id,
            Self::Furniture(item) => item.id,
            Self::Chara(item) => item.id,
        }
    }

    fn icon(&self) -> u32 {
        match self {
            Self::Equipment(item) => item.icon,
            Self::Furniture(item) => item.icon,
            Self::Chara(item) => item.icon,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Equipment(item) => &item.name,
            Self::Furniture(item) => &item.name,
            Self::Chara(item) => &item.name,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Equipment(item) => &item.description,
            Self::Furniture(_) | Self::Chara(_) => "",
        }
    }

    /// 部位/类别标签：装备用槽位名，家具/宠物用各自 kind 的 `label()`。
    fn kind_label(&self) -> &'static str {
        match self {
            Self::Equipment(item) => equipment_slot_label(item.equip_slot_category),
            Self::Furniture(item) => item.kind.label(),
            Self::Chara(item) => item.kind.label(),
        }
    }

    fn model_label(&self) -> Option<String> {
        match self {
            Self::Equipment(item) => format_item_model(item),
            Self::Furniture(item) => Some(format_furniture_model(item.kind, item.model_key)),
            Self::Chara(item) => Some(format_chara_model(&item.model)),
        }
    }

    fn sub_model_label(&self) -> Option<String> {
        match self {
            Self::Equipment(item) => format_item_sub_model(item),
            Self::Furniture(_) | Self::Chara(_) => None,
        }
    }

    fn support(&self) -> ModelPreviewSupport {
        match self {
            Self::Equipment(item) => model_preview_support(item),
            Self::Furniture(_) => ModelPreviewSupport::Furniture,
            Self::Chara(_) => ModelPreviewSupport::Chara,
        }
    }
}

/// 可选种族模型（race code 依据 xivModdingFramework `XivRace`）。
#[derive(Clone, Copy)]
struct EquipmentRace {
    id: u16,
    label: &'static str,
}

const EQUIPMENT_RACES: &[EquipmentRace] = &[
    EquipmentRace {
        id: 101,
        label: "中原人男",
    },
    EquipmentRace {
        id: 201,
        label: "中原人女",
    },
    EquipmentRace {
        id: 301,
        label: "高地人男",
    },
    EquipmentRace {
        id: 401,
        label: "高地人女",
    },
    EquipmentRace {
        id: 501,
        label: "精灵男",
    },
    EquipmentRace {
        id: 601,
        label: "精灵女",
    },
    EquipmentRace {
        id: 701,
        label: "猫魅族男",
    },
    EquipmentRace {
        id: 801,
        label: "猫魅族女",
    },
    EquipmentRace {
        id: 901,
        label: "鲁加族男",
    },
    EquipmentRace {
        id: 1001,
        label: "鲁加族女",
    },
    EquipmentRace {
        id: 1101,
        label: "拉拉菲尔族男",
    },
    EquipmentRace {
        id: 1201,
        label: "拉拉菲尔族女",
    },
    EquipmentRace {
        id: 1301,
        label: "敖龙族男",
    },
    EquipmentRace {
        id: 1401,
        label: "敖龙族女",
    },
    EquipmentRace {
        id: 1501,
        label: "硌狮族男",
    },
    EquipmentRace {
        id: 1601,
        label: "硌狮族女",
    },
    EquipmentRace {
        id: 1701,
        label: "维埃拉族男",
    },
    EquipmentRace {
        id: 1801,
        label: "维埃拉族女",
    },
];

fn parse_equipment_race_id(value: &str) -> Option<u16> {
    let digits = value.trim().trim_start_matches(['c', 'C']);
    let race_id = digits.parse::<u16>().ok()?;
    EQUIPMENT_RACES
        .iter()
        .any(|race| race.id == race_id)
        .then_some(race_id)
}

/// 条目的模型预览支持度，决定加载链路与空态展示。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelPreviewSupport {
    /// 武器槽位：走武器模型加载链路。
    Weapon,
    /// 有独立模型的防具/饰品槽位：走装备模型加载链路，按种族解析模型文件。
    Equipment,
    /// 家具/庭具：走 housing SGB → 多 MDL 加载链路，无染色与种族维度。
    Furniture,
    /// 宠物/坐骑：monster 单 MDL / demihuman 多槽位 MDL 链路，无染色与种族维度。
    Chara,
    /// 条目本身没有模型数据（ModelMain 为 0）。
    NoModel,
    /// 腰带与复合部位：没有可单独预览的模型。
    UnsupportedSlot,
}

fn model_preview_support(item: &CollectionItem) -> ModelPreviewSupport {
    if item.model_main == 0 {
        return ModelPreviewSupport::NoModel;
    }
    if is_weapon_equip_slot_category(item.equip_slot_category) {
        return ModelPreviewSupport::Weapon;
    }
    if equipment_slot_info(item.equip_slot_category).is_some() {
        ModelPreviewSupport::Equipment
    } else {
        ModelPreviewSupport::UnsupportedSlot
    }
}

#[component]
pub fn ModelPreviewPage() -> Element {
    let initial_url_state = initial_model_preview_url_state();
    let initial_query = initial_url_state.query;
    let initial_filter = initial_url_state.filter;
    let initial_item_id = initial_url_state.item_id;
    let initial_stain_ids = initial_url_state.stain_ids;
    let initial_race_id = initial_url_state.race_id;

    let collection_catalog = use_resource(load_collection_catalog);
    let furniture_catalog = use_resource(load_furniture_catalog);
    let chara_catalog = use_resource(load_chara_catalog);
    let weapon_catalog = use_resource(load_weapon_catalog);
    let mut query = use_signal(move || initial_query.clone());
    let mut slot_filter = use_signal(move || initial_filter);
    let mut selected_id = use_signal(move || initial_item_id);
    let mut selected_item = use_signal(|| None::<ModelCatalogItem>);
    let mut stain_ids = use_signal(move || initial_stain_ids);
    let mut race_id = use_signal(move || initial_race_id);
    let mut model_progress = use_signal(|| None::<WeaponModelLoadProgress>);
    let model = use_resource(move || {
        let item = selected_item();
        let race_id = race_id();
        async move {
            let item = item?;
            let (result, skeleton, animations) = match &item {
                ModelCatalogItem::Equipment(equipment) => match model_preview_support(equipment) {
                    ModelPreviewSupport::Weapon => (load_weapon_model(equipment).await, None, None),
                    ModelPreviewSupport::Equipment => {
                        (load_equipment_model(equipment, race_id).await, None, None)
                    }
                    _ => return None,
                },
                ModelCatalogItem::Furniture(furniture) => {
                    (load_furniture_model(furniture).await, None, None)
                }
                ModelCatalogItem::Chara(chara) => {
                    match load_chara_model_with_animation_assets(chara).await {
                        Ok((data, skeleton, animations)) => (Ok(data), skeleton, animations),
                        Err(error) => (Err(error), None, None),
                    }
                }
            };
            Some(ModelResourceResult {
                item_id: item.id(),
                race_id,
                result,
                skeleton,
                animations,
            })
        }
    });
    let staining_templates = use_resource(load_weapon_staining_templates);
    let preview_model = use_memo(move || {
        let item = selected_item()?;
        let race_id = race_id();
        let stain_ids = stain_ids();
        let key = ModelRequestKey {
            item_id: item.id(),
            race_id,
            stain_ids,
        };
        let loaded = model.read().as_ref().cloned().flatten()?;
        if loaded.item_id != item.id() || loaded.race_id != race_id {
            return None;
        }
        // 家具与宠物/坐骑没有染色通道，始终展示基线模型；骨架/动画集与染色无关。
        let stainable = matches!(item, ModelCatalogItem::Equipment(_));
        let result = match loaded.result {
            Err(error) => Err(error),
            Ok(base) if stain_ids == [0, 0] || !stainable => Ok(base),
            Ok(base) => match staining_templates.read().as_ref().cloned() {
                Some(Ok(templates)) => Ok(stain_weapon_model(&base, stain_ids, &templates)),
                Some(Err(error)) => Err(error),
                None => return None,
            },
        };
        Some(ModelPreviewResult {
            key,
            result,
            skeleton: loaded.skeleton,
            animations: loaded.animations,
        })
    });

    use_effect(move || {
        load_progress::set_weapon_model_progress_sink(move |progress| {
            if let Ok(mut slot) = model_progress.try_write() {
                *slot = progress;
            }
        });
    });

    use_drop(move || {
        load_progress::clear_weapon_model_progress();
    });

    use_effect(move || {
        let id = selected_id();
        if selected_item().as_ref().map(|item| item.id()) == id {
            return;
        }

        let Some(id) = id else {
            if selected_item().is_some() {
                selected_item.set(None);
            }
            return;
        };

        let resolved = {
            let collection = collection_catalog.read();
            let furniture = furniture_catalog.read();
            let chara = chara_catalog.read();
            resolve_model_catalog_item(
                collection
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .map(Rc::as_ref),
                furniture
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .map(Rc::as_ref),
                chara
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .map(Rc::as_ref),
                id,
            )
        };
        if let Some(item) = resolved {
            selected_item.set(Some(item));
        }
    });

    use_effect(move || {
        sync_model_preview_url_state(
            &query(),
            slot_filter(),
            selected_id(),
            stain_ids(),
            race_id(),
        );
    });

    let collection_catalog_snapshot = collection_catalog.read().as_ref().cloned();
    let furniture_catalog_snapshot = furniture_catalog.read().as_ref().cloned();
    let chara_catalog_snapshot = chara_catalog.read().as_ref().cloned();
    let weapon_catalog_snapshot = weapon_catalog.read().as_ref().cloned();
    let selected_snapshot = selected_item();
    let selected_id_snapshot = selected_id();
    let query_snapshot = query();
    let slot_filter_snapshot = slot_filter();
    let stain_ids_snapshot = stain_ids();
    let race_id_snapshot = race_id();
    let model_progress_snapshot = model_progress();

    rsx! {
        div { class: "flex h-[calc(100dvh-3.5rem)] min-w-0 flex-col overflow-hidden bg-background lg:h-screen",
            div { class: "border-b px-4 py-2 sm:px-5 lg:px-6",
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div { class: "flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1",
                        h1 { class: "text-xl font-semibold leading-tight", "模型预览" }
                        crate::app::modules::ModuleCapabilityBadges { module_id: "equipment-models" }
                    }
                    div { class: "flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                        if let Some(Ok(catalog)) = &collection_catalog_snapshot {
                            span { "{catalog.game_version}" }
                            span { "{format_integer(catalog.counts.equipment as f64)} 件装备" }
                        }
                        if let Some(Ok(catalog)) = &furniture_catalog_snapshot {
                            span { "{format_integer(catalog.counts.indoor as f64)} 件家具" }
                            span { "{format_integer(catalog.counts.outdoor as f64)} 件庭具" }
                        }
                        if let Some(Ok(catalog)) = &chara_catalog_snapshot {
                            span { "{format_integer(catalog.counts.minions as f64)} 只宠物" }
                            span { "{format_integer(catalog.counts.mounts as f64)} 个坐骑" }
                        }
                        if let Some(Ok(catalog)) = &weapon_catalog_snapshot {
                            span { "{format_integer(catalog.counts.stains as f64)} 种染剂" }
                        }
                        GitHubRepoButton {}
                    }
                }
            }

            match collection_catalog_snapshot {
                None => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        div { class: "flex items-center gap-3 text-sm text-muted-foreground",
                            Icon { kind: IconKind::LoaderCircle, class: "h-4 w-4 animate-spin" }
                            "正在读取本地图鉴目录"
                        }
                    }
                },
                Some(Err(error)) => rsx! {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        EmptyState {
                            icon: rsx! { Icon { kind: IconKind::Database, class: "h-6 w-6" } },
                            title: "图鉴目录不可用".to_string(),
                            description: Some(error),
                            action: rsx! {
                                a { href: "#/",
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        size: ButtonSize::Sm,
                                        Icon { kind: IconKind::Database, class: "h-4 w-4" }
                                        "数据来源"
                                    }
                                }
                            },
                        }
                    }
                },
                Some(Ok(catalog)) => {
                    let furniture_package = furniture_catalog_snapshot
                        .as_ref()
                        .and_then(|result| result.as_ref().ok());
                    let chara_package = chara_catalog_snapshot
                        .as_ref()
                        .and_then(|result| result.as_ref().ok());
                    let search = search_model_items(
                        &catalog,
                        furniture_package.map(Rc::as_ref),
                        chara_package.map(Rc::as_ref),
                        &query_snapshot,
                        slot_filter_snapshot,
                    );
                    let total_items = match slot_filter_snapshot {
                        ModelItemFilter::All => {
                            catalog.counts.equipment
                                + furniture_package
                                    .map(|package| package.counts.items)
                                    .unwrap_or(0)
                                + chara_package
                                    .map(|package| package.counts.items)
                                    .unwrap_or(0)
                        }
                        ModelItemFilter::Furniture => {
                            furniture_package.map(|package| package.counts.indoor).unwrap_or(0)
                        }
                        ModelItemFilter::Yard => {
                            furniture_package.map(|package| package.counts.outdoor).unwrap_or(0)
                        }
                        ModelItemFilter::Minions => {
                            chara_package.map(|package| package.counts.minions).unwrap_or(0)
                        }
                        ModelItemFilter::Mounts => {
                            chara_package.map(|package| package.counts.mounts).unwrap_or(0)
                        }
                        _ => catalog.counts.equipment,
                    };
                    let stains = weapon_catalog_snapshot
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .map(|catalog| catalog.stains.clone())
                        .unwrap_or_default();
                    rsx! {
                        div { class: "grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[280px_minmax(0,1fr)]",
                            ModelSearchPane {
                                total_items,
                                query: query_snapshot,
                                filter: slot_filter_snapshot,
                                result: search,
                                selected_id: selected_id_snapshot,
                                on_query_change: move |value| query.set(value),
                                on_filter_change: move |value| slot_filter.set(value),
                                on_select: move |item: ModelCatalogItem| {
                                    selected_id.set(Some(item.id()));
                                    selected_item.set(Some(item));
                                },
                            }
                            ModelPreviewPane {
                                selected: selected_snapshot,
                                selection_pending: selected_id_snapshot.is_some(),
                                model: preview_model(),
                                progress: model_progress_snapshot,
                                stains,
                                stain_ids: stain_ids_snapshot,
                                race_id: race_id_snapshot,
                                on_race_change: move |value| race_id.set(value),
                                on_stain_change: move |(channel, stain_id): (usize, u8)| {
                                    let mut next = stain_ids();
                                    if let Some(value) = next.get_mut(channel) {
                                        *value = stain_id;
                                        stain_ids.set(next);
                                    }
                                },
                            }
                        }
                    }
                },
            }
        }
    }
}

/// 按物品 id 解析预览条目：装备/家具/宠物坐骑共用物品 id 空间但互不重叠，
/// 依次按装备（图鉴）目录、家具目录、宠物/坐骑目录解析。
fn resolve_model_catalog_item(
    collection: Option<&CollectionCatalogPackage>,
    furniture: Option<&FurnitureCatalogPackage>,
    chara: Option<&CharaCatalogPackage>,
    item_id: u32,
) -> Option<ModelCatalogItem> {
    if let Some(item) = collection.and_then(|catalog| {
        catalog
            .items
            .iter()
            .find(|item| item.is_equipment() && item.id == item_id)
    }) {
        return Some(ModelCatalogItem::Equipment(item.clone()));
    }
    if let Some(item) =
        furniture.and_then(|catalog| catalog.items.iter().find(|item| item.id == item_id))
    {
        return Some(ModelCatalogItem::Furniture(item.clone()));
    }
    chara
        .and_then(|catalog| catalog.items.iter().find(|item| item.id == item_id))
        .cloned()
        .map(ModelCatalogItem::Chara)
}

#[component]
fn ModelSearchPane(
    total_items: usize,
    query: String,
    filter: ModelItemFilter,
    result: ModelSearchResult,
    selected_id: Option<u32>,
    on_query_change: EventHandler<String>,
    on_filter_change: EventHandler<ModelItemFilter>,
    on_select: EventHandler<ModelCatalogItem>,
) -> Element {
    rsx! {
        aside { class: "flex min-h-0 flex-col border-b bg-card lg:border-b-0 lg:border-r",
            div { class: "shrink-0 space-y-2 border-b p-3",
                div { class: "relative",
                    Icon { kind: IconKind::Search, class: "pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" }
                    input {
                        class: "flex h-8 w-full rounded-md border border-input bg-background py-1 pl-9 pr-3 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
                        value: "{query}",
                        placeholder: "名称 / 物品 ID / 模型 ID",
                        oninput: move |event| on_query_change.call(event.value()),
                    }
                }

                div { class: "grid grid-cols-8 gap-1 rounded-md bg-muted p-1",
                    for option in [
                        ModelItemFilter::All,
                        ModelItemFilter::Weapons,
                        ModelItemFilter::Armor,
                        ModelItemFilter::Accessories,
                        ModelItemFilter::Furniture,
                        ModelItemFilter::Yard,
                        ModelItemFilter::Minions,
                        ModelItemFilter::Mounts,
                    ] {
                        button {
                            r#type: "button",
                            class: segment_button_class(filter == option),
                            onclick: move |_| on_filter_change.call(option),
                            "{option.label()}"
                        }
                    }
                }

                if let Some((grid_class, sub_filters)) = filter.sub_filters() {
                    div { class: "grid {grid_class} gap-1 rounded-md bg-muted p-1",
                        for option in sub_filters {
                            button {
                                r#type: "button",
                                class: segment_button_class(filter == *option),
                                onclick: move |_| on_filter_change.call(*option),
                                "{option.chip_label()}"
                            }
                        }
                    }
                }

                div { class: "flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground",
                    span { "{format_integer(result.total as f64)} / {format_integer(total_items as f64)}" }
                    if result.total > result.items.len() {
                        span { "显示前 {RESULT_LIMIT}" }
                    }
                }
            }

            div { class: "min-h-0 flex-1 overflow-y-auto p-1.5",
                if result.items.is_empty() {
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                        title: "没有匹配的物品".to_string(),
                    }
                } else {
                    div { class: "space-y-1",
                        for item in result.items {
                            ModelListRow {
                                key: "{item.id()}",
                                active: selected_id == Some(item.id()),
                                item,
                                on_select,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ModelListRow(
    item: ModelCatalogItem,
    active: bool,
    on_select: EventHandler<ModelCatalogItem>,
) -> Element {
    let row_item = item.clone();
    let model_label = item.model_label();
    rsx! {
        button {
            r#type: "button",
            class: cx([
                "flex w-full min-w-0 items-center gap-2.5 rounded-md border px-2 py-1.5 text-left transition-colors",
                if active {
                    "border-foreground/20 bg-background shadow-sm"
                } else {
                    "border-transparent hover:border-border hover:bg-background/70"
                },
            ]),
            onclick: move |_| on_select.call(row_item.clone()),
            ItemIcon { icon: item.icon(), size: "sm" }
            div { class: "min-w-0 flex-1",
                div { class: "flex min-w-0 items-baseline gap-1.5",
                    span { class: "truncate text-sm font-medium", "{item.name()}" }
                    span { class: "shrink-0 text-[11px] text-muted-foreground", "#{item.id()}" }
                }
                div { class: "mt-0 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground",
                    span { "{item.kind_label()}" }
                    if let Some(model_label) = model_label {
                        span { "{model_label}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ModelPreviewPane(
    selected: Option<ModelCatalogItem>,
    selection_pending: bool,
    model: Option<ModelPreviewResult>,
    progress: Option<WeaponModelLoadProgress>,
    stains: Vec<WeaponStain>,
    stain_ids: [u8; 2],
    race_id: u16,
    on_race_change: EventHandler<u16>,
    on_stain_change: EventHandler<(usize, u8)>,
) -> Element {
    let render_options = use_signal(WeaponRenderOptions::default);
    let mut shape_selection = use_signal(|| (None::<u32>, None::<u32>));
    // 部件变体选择按物品 id 作用域存储（与 shape 选择同款模式）：切换物品
    // 后读回 0=全部关闭（对应游戏默认状态）。
    let mut attribute_selection = use_signal(|| (None::<u32>, 0_u32));
    // 隔离预览开关同样按物品 id 作用域存储，切换物品后回到关闭。
    let mut parts_only_selection = use_signal(|| (None::<u32>, false));
    // 动画选择按物品 id 作用域存储：切换物品后回到 None（rest）。
    let mut animation_selection = use_signal(|| (None::<u32>, None::<usize>));
    let support = selected.as_ref().map(ModelCatalogItem::support);
    let stainable = matches!(
        support,
        Some(ModelPreviewSupport::Weapon | ModelPreviewSupport::Equipment)
    );
    let previewable = stainable
        || matches!(
            support,
            Some(ModelPreviewSupport::Furniture | ModelPreviewSupport::Chara)
        );
    let race_selection = (support == Some(ModelPreviewSupport::Equipment)).then_some(race_id);
    let requested_key = selected.as_ref().map(|item| ModelRequestKey {
        item_id: item.id(),
        race_id,
        stain_ids,
    });
    let current_snapshot = requested_key.and_then(|key| {
        model
            .filter(|snapshot| {
                snapshot.key.item_id == key.item_id && snapshot.key.race_id == key.race_id
            })
            .clone()
    });
    let current_model_result = current_snapshot.as_ref().map(|snapshot| snapshot.result.clone());
    let current_skeleton = current_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.skeleton.clone());
    let current_animations = current_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.animations.clone());
    let current_progress =
        requested_key.and_then(|key| progress.filter(|progress| progress.item_id == key.item_id));

    rsx! {
        section { class: "flex min-h-0 min-w-0 flex-col overflow-hidden bg-background",
            if let Some(item) = selected.clone() {
                div { class: "shrink-0 border-b p-4",
                    div { class: "flex min-w-0 flex-wrap items-center gap-3",
                        ItemIcon { icon: item.icon() }
                        div { class: "min-w-0 flex-1",
                            div { class: "truncate text-base font-semibold", "{item.name()}" }
                            div { class: "mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground",
                                Badge { variant: BadgeVariant::Outline, "#{item.id()}" }
                                Badge { variant: BadgeVariant::Secondary, "{item.kind_label()}" }
                                if let Some(model_label) = item.model_label() {
                                    span { "main {model_label}" }
                                }
                                if let Some(sub_label) = item.sub_model_label() {
                                    span { "sub {sub_label}" }
                                }
                            }
                        }
                    }
                    if !item.description().trim().is_empty() {
                        div { class: "mt-3 max-w-3xl text-sm leading-relaxed text-muted-foreground",
                            "{item.description()}"
                        }
                    }
                    if stainable {
                        WeaponStainControls {
                            stains,
                            stain_ids,
                            race_id: race_selection,
                            on_race_change,
                            on_stain_change,
                        }
                    }
                }

                if previewable {
                    div { class: "flex min-h-0 flex-1 flex-col overflow-hidden xl:flex-row",
                        div { class: "relative min-h-0 min-w-0 flex-1 overflow-hidden bg-[#0e1117]",
                            {
                                // 画布在物品切换、加载中与出错期间始终挂载，加载与错误
                                // 状态用浮层覆盖，避免切换物品时重复初始化 WebGPU。
                                let canvas_model = current_model_result
                                    .as_ref()
                                    .and_then(|result| result.as_ref().ok())
                                    .cloned();
                                let requested_shape = shape_selection();
                                let shape_mask = canvas_model.as_ref().and_then(|data| {
                                    (requested_shape.0 == Some(item.id()))
                                        .then_some(requested_shape.1)
                                        .flatten()
                                        .filter(|mask| model_has_shape_mask(data, *mask))
                                });
                                let attribute_mask =
                                    scoped_attribute_mask(attribute_selection(), item.id());
                                let attribute_parts_only =
                                    scoped_attribute_parts_only(parts_only_selection(), item.id());
                                let animation_selected =
                                    scoped_animation_selection(animation_selection(), item.id());
                                let playback = animation_playback(
                                    &current_skeleton,
                                    &current_animations,
                                    animation_selected,
                                );
                                rsx! {
                                    WeaponModelCanvas {
                                        model: canvas_model,
                                        render_options,
                                        shape_mask,
                                        race_id,
                                        attribute_mask,
                                        attribute_parts_only,
                                        // 有动画集时强制原位布局：平铺偏移量烘
                                        // 在蒙皮前顶点位置里，动画驱动 joint 后
                                        // 偏移随关节旋转（rest 下无此问题）。
                                        component_preview_layout: playback.is_none(),
                                        animation: playback,
                                    }
                                    match current_model_result.as_ref() {
                                        Some(Ok(_)) => rsx! {},
                                        Some(Err(error)) => rsx! {
                                            div { class: "absolute inset-0 flex items-center justify-center bg-[#0e1117] p-6",
                                                EmptyState {
                                                    icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                                    title: "模型读取失败".to_string(),
                                                    description: Some(error.clone()),
                                                }
                                            }
                                        },
                                        _ => rsx! {
                                            div { class: "absolute inset-0 bg-[#0e1117]",
                                                WeaponModelLoadingView { progress: current_progress.clone() }
                                            }
                                        },
                                    }
                                }
                            }
                        }

                        aside { class: "h-64 shrink-0 overflow-y-auto border-t bg-card p-3 xl:h-auto xl:w-64 xl:border-l xl:border-t-0",
                            match current_model_result.as_ref() {
                                Some(Ok(data)) => {
                                    let requested_shape = shape_selection();
                                    let shape_mask = (requested_shape.0 == Some(item.id()))
                                        .then_some(requested_shape.1)
                                        .flatten()
                                        .filter(|mask| model_has_shape_mask(data, *mask));
                                    let item_id = item.id();
                                    let attribute_mask =
                                        scoped_attribute_mask(attribute_selection(), item_id);
                                    let attribute_parts_only = scoped_attribute_parts_only(
                                        parts_only_selection(),
                                        item_id,
                                    );
                                    let animation_selected =
                                        scoped_animation_selection(animation_selection(), item_id);
                                    let animations = current_animations.clone();
                                    rsx! {
                                        div { class: "space-y-4",
                                            WeaponRenderControls {
                                                options: render_options,
                                                model: data.clone(),
                                                shape_mask,
                                                on_shape_change: move |mask| {
                                                    shape_selection.set((Some(item_id), mask));
                                                },
                                            }
                                            if let Some(animations) = animations {
                                                AnimationControls {
                                                    animations: AnimationSetHandle(animations),
                                                    selected: animation_selected,
                                                    on_select: move |selected| {
                                                        animation_selection
                                                            .set((Some(item_id), selected));
                                                    },
                                                }
                                            }
                                            WeaponAttributeControls {
                                                model: data.clone(),
                                                attribute_mask,
                                                attribute_parts_only,
                                                on_attribute_change: move |mask| {
                                                    attribute_selection.set((Some(item_id), mask));
                                                },
                                                on_parts_only_change: move |parts_only| {
                                                    parts_only_selection
                                                        .set((Some(item_id), parts_only));
                                                },
                                            }
                                            WeaponModelStats { model: data.clone() }
                                        }
                                    }
                                },
                                _ => rsx! {
                                    div { class: "space-y-3",
                                        SkeletonLine {}
                                        SkeletonLine {}
                                        SkeletonLine {}
                                    }
                                },
                            }
                        }
                    }
                } else {
                    div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                        if support == Some(ModelPreviewSupport::NoModel) {
                            EmptyState {
                                icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                title: "该物品没有可预览的模型".to_string(),
                                description: Some("该物品没有关联的模型数据。".to_string()),
                            }
                        } else {
                            EmptyState {
                                icon: rsx! { Icon { kind: IconKind::PackageSearch, class: "h-6 w-6" } },
                                title: "该部位暂不支持预览".to_string(),
                                description: Some("腰带与复合部位的装备没有可单独预览的模型。".to_string()),
                            }
                        }
                    }
                }
            } else if selection_pending {
                div { class: "relative min-h-0 flex-1 bg-[#0e1117]",
                    WeaponModelLoadingView {
                        progress: None,
                        stage: Some("正在定位物品".to_string()),
                    }
                }
            } else {
                div { class: "flex min-h-0 flex-1 items-center justify-center p-6",
                    EmptyState {
                        icon: rsx! { Icon { kind: IconKind::Sword, class: "h-6 w-6" } },
                        title: "未选择物品".to_string(),
                    }
                }
            }
        }
    }
}

#[component]
pub(crate) fn WeaponModelLoadingView(
    progress: Option<WeaponModelLoadProgress>,
    stage: Option<String>,
) -> Element {
    let stage = stage
        .or_else(|| progress.as_ref().map(|progress| progress.stage.clone()))
        .unwrap_or_else(|| "准备读取模型".to_string());
    let detail = progress
        .as_ref()
        .map(|progress| progress.detail.clone())
        .filter(|detail| !detail.is_empty());
    let stats = progress.as_ref().map(|progress| {
        format!(
            "已载入 {} / 已检查 {} 个资源 · {} · {}",
            progress.loaded_resources,
            progress.checked_resources,
            format_byte_size(progress.loaded_bytes),
            format_elapsed(progress.elapsed_ms),
        )
    });

    rsx! {
        div { class: "absolute inset-0 flex items-center justify-center p-6",
            div { class: "w-full max-w-md space-y-4 text-center",
                div {
                    class: "flex items-center justify-center gap-3 text-sm font-medium",
                    style: "color: rgba(255, 255, 255, 0.9);",
                    Icon { kind: IconKind::LoaderCircle, class: "h-5 w-5 animate-spin" }
                    span { "{stage}" }
                }
                if let Some(detail) = detail {
                    div {
                        class: "truncate text-xs",
                        style: "color: rgba(255, 255, 255, 0.55);",
                        title: "{detail}",
                        "{detail}"
                    }
                }
                div {
                    class: "mx-auto h-1.5 w-full max-w-sm overflow-hidden rounded",
                    style: "background-color: rgba(255, 255, 255, 0.1);",
                    div {
                        class: "h-full animate-pulse rounded",
                        style: "width: 50%; background-color: rgba(255, 255, 255, 0.55);",
                    }
                }
                if let Some(stats) = stats {
                    div {
                        class: "text-[11px]",
                        style: "color: rgba(255, 255, 255, 0.45);",
                        "{stats}"
                    }
                }
            }
        }
    }
}

fn format_byte_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    let bytes = bytes as f64;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn format_elapsed(elapsed_ms: f64) -> String {
    if elapsed_ms >= 1_000.0 {
        format!("{:.1} 秒", elapsed_ms / 1_000.0)
    } else {
        format!("{elapsed_ms:.0} 毫秒")
    }
}

#[component]
fn WeaponModelStats(model: Rc<WeaponModelData>) -> Element {
    let mesh_count = model.meshes.len();
    let material_count = model.materials.len();
    let texture_count = model.textures.len();
    let texture_counts = TextureKindCounts::from_model(&model);
    let vertex_count: usize = model.meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let index_count: usize = model.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    let bounds = model.bounds;

    rsx! {
        div { class: "space-y-4",
            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "模型" }
                StatRow { label: "Mesh", value: format_integer(mesh_count as f64) }
                StatRow { label: "Material", value: format_integer(material_count as f64) }
                StatRow { label: "Texture", value: format_integer(texture_count as f64) }
                StatRow { label: "Vertex", value: format_integer(vertex_count as f64) }
                StatRow { label: "Index", value: format_integer(index_count as f64) }
                StatRow { label: "Radius", value: format!("{:.3}", bounds.radius) }
            }

            if texture_count > 0 {
                section { class: "space-y-2",
                    div { class: "text-sm font-semibold", "Textures" }
                    StatRow { label: "Base", value: texture_counts.base.to_string() }
                    StatRow { label: "Base Map 1", value: texture_counts.secondary_base.to_string() }
                    StatRow { label: "Normal", value: texture_counts.normal.to_string() }
                    StatRow { label: "Normal Map 1", value: texture_counts.secondary_normal.to_string() }
                    StatRow { label: "Mask", value: texture_counts.mask.to_string() }
                    StatRow { label: "Material Map", value: texture_counts.material_map.to_string() }
                    StatRow { label: "Multi Map", value: texture_counts.multi_map.to_string() }
                    StatRow { label: "Specular", value: texture_counts.specular.to_string() }
                    StatRow { label: "Specular Map 1", value: texture_counts.secondary_specular.to_string() }
                    StatRow { label: "Material Props", value: texture_counts.material_properties.to_string() }
                    StatRow { label: "Tile Props", value: texture_counts.tile_properties.to_string() }
                    StatRow { label: "Sheen Props", value: texture_counts.sheen_properties.to_string() }
                    StatRow { label: "Sphere Props", value: texture_counts.sphere_properties.to_string() }
                    StatRow { label: "Tile Matrix", value: texture_counts.tile_matrix.to_string() }
                    StatRow { label: "Emissive", value: texture_counts.emissive.to_string() }
                    StatRow { label: "Environment", value: texture_counts.environment.to_string() }
                    StatRow { label: "Index", value: texture_counts.index.to_string() }
                    StatRow { label: "Tile Normal Array", value: texture_counts.tile_normal_array.to_string() }
                    StatRow { label: "Tile ORB Array", value: texture_counts.tile_orb_array.to_string() }
                    StatRow { label: "Detail Diffuse Array", value: texture_counts.detail_diffuse_array.to_string() }
                    StatRow { label: "Detail Normal Array", value: texture_counts.detail_normal_array.to_string() }
                    StatRow { label: "Water Wave", value: texture_counts.water_wave.to_string() }
                    StatRow { label: "Water Wave 1", value: texture_counts.water_wave1.to_string() }
                    StatRow { label: "Water Whitecap", value: texture_counts.water_whitecap.to_string() }
                    StatRow { label: "Other", value: texture_counts.other.to_string() }
                }
            }

            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "Bounds" }
                StatRow { label: "Min", value: format_vec3(bounds.min) }
                StatRow { label: "Max", value: format_vec3(bounds.max) }
                StatRow { label: "Center", value: format_vec3(bounds.center) }
            }

            section { class: "space-y-2",
                div { class: "text-sm font-semibold", "SqPack" }
                for path in model.loaded_paths.clone() {
                    div { class: "break-all rounded-md border bg-background px-2 py-1.5 font-mono text-[11px] text-muted-foreground",
                        "{path}"
                    }
                }
            }
        }
    }
}

#[component]
fn StatRow(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-2 border-b border-border/60 py-1 text-xs last:border-b-0",
            span { class: "text-muted-foreground", "{label}" }
            span { class: "min-w-0 truncate font-medium", "{value}" }
        }
    }
}

#[derive(Default)]
struct TextureKindCounts {
    base: usize,
    secondary_base: usize,
    normal: usize,
    secondary_normal: usize,
    mask: usize,
    material_map: usize,
    multi_map: usize,
    specular: usize,
    secondary_specular: usize,
    material_properties: usize,
    tile_properties: usize,
    sheen_properties: usize,
    sphere_properties: usize,
    tile_matrix: usize,
    emissive: usize,
    environment: usize,
    index: usize,
    tile_normal_array: usize,
    tile_orb_array: usize,
    detail_diffuse_array: usize,
    detail_normal_array: usize,
    water_wave: usize,
    water_wave1: usize,
    water_whitecap: usize,
    other: usize,
}

impl TextureKindCounts {
    fn from_model(model: &WeaponModelData) -> Self {
        let mut counts = Self::default();
        for texture in &model.textures {
            match texture.kind {
                WeaponModelTextureKind::BaseColor => counts.base += 1,
                WeaponModelTextureKind::SecondaryBaseColor => counts.secondary_base += 1,
                WeaponModelTextureKind::Normal => counts.normal += 1,
                WeaponModelTextureKind::SecondaryNormal => counts.secondary_normal += 1,
                WeaponModelTextureKind::Mask => counts.mask += 1,
                WeaponModelTextureKind::MaterialMap => counts.material_map += 1,
                WeaponModelTextureKind::MultiMap => counts.multi_map += 1,
                WeaponModelTextureKind::Specular => counts.specular += 1,
                WeaponModelTextureKind::SecondarySpecular => counts.secondary_specular += 1,
                WeaponModelTextureKind::MaterialProperties => counts.material_properties += 1,
                WeaponModelTextureKind::TileProperties => counts.tile_properties += 1,
                WeaponModelTextureKind::SheenProperties => counts.sheen_properties += 1,
                WeaponModelTextureKind::SphereProperties => counts.sphere_properties += 1,
                WeaponModelTextureKind::TileMatrixProperties => counts.tile_matrix += 1,
                WeaponModelTextureKind::Emissive => counts.emissive += 1,
                WeaponModelTextureKind::Environment => counts.environment += 1,
                WeaponModelTextureKind::Index => counts.index += 1,
                WeaponModelTextureKind::TileNormalArray => counts.tile_normal_array += 1,
                WeaponModelTextureKind::TileOrbArray => counts.tile_orb_array += 1,
                WeaponModelTextureKind::DetailDiffuseArray => counts.detail_diffuse_array += 1,
                WeaponModelTextureKind::DetailNormalArray => counts.detail_normal_array += 1,
                WeaponModelTextureKind::WaterWave => counts.water_wave += 1,
                WeaponModelTextureKind::WaterWaveSecondary => counts.water_wave1 += 1,
                WeaponModelTextureKind::WaterWhitecap => counts.water_whitecap += 1,
                WeaponModelTextureKind::Other => counts.other += 1,
            }
        }
        counts
    }
}

#[component]
fn SkeletonLine() -> Element {
    rsx! {
        div { class: "h-8 animate-pulse rounded-md bg-muted" }
    }
}

#[component]
fn WeaponRenderControls(
    options: Signal<WeaponRenderOptions>,
    model: Rc<WeaponModelData>,
    shape_mask: Option<u32>,
    on_shape_change: EventHandler<Option<u32>>,
) -> Element {
    let current = options();
    let bloom_percent = (current.bloom_strength * 100.0).round() as i32;
    let debug_select_class = input_class("h-8 cursor-pointer py-1 text-xs");
    let shape_select_class = input_class("h-8 w-24 cursor-pointer py-1 text-xs");
    let shape_options = model_shape_options(&model);

    rsx! {
        section { class: "space-y-2 text-xs",
            div { class: "flex items-center justify-between gap-3",
                span { class: "text-sm font-semibold", "渲染" }
                span { class: "text-[11px] text-muted-foreground", "{bloom_percent}%" }
            }
            div { class: "space-y-1.5",
                if !shape_options.is_empty() {
                    label { class: "flex items-center justify-between gap-3",
                        span { class: "text-muted-foreground", "Shape" }
                        select {
                            class: "{shape_select_class}",
                            value: "{shape_mask.unwrap_or(0)}",
                            onchange: move |event| {
                                let mask = event.value().parse::<u32>().ok().filter(|mask| *mask != 0);
                                on_shape_change.call(mask);
                            },
                            option { value: "0", "Base" }
                            for (_, mask, name) in shape_options.clone() {
                                option { value: "{mask}", "{name}" }
                            }
                        }
                    }
                }
                select {
                    class: "{debug_select_class}",
                    value: "{debug_mode_value(current.debug_mode)}",
                    onchange: move |event| {
                        let mut next = options();
                        next.debug_mode = parse_debug_mode(&event.value());
                        options.set(next);
                    },
                    option { value: "final", "Final" }
                    option { value: "base", "Base" }
                    option { value: "normal", "Normal" }
                    option { value: "mask", "Mask" }
                    option { value: "material", "Material" }
                    option { value: "specular", "Specular" }
                    option { value: "emissive", "Emissive" }
                    option { value: "alpha", "Alpha" }
                    option { value: "uv0", "UV0" }
                    option { value: "uv1", "UV1" }
                    option { value: "uv2", "UV2" }
                    option { value: "uv3", "UV3" }
                    option { value: "vertex", "Vertex" }
                    option { value: "vertex1", "Vertex 1" }
                    option { value: "normal1", "Normal 1" }
                    option { value: "flow0", "Flow 0" }
                    option { value: "flow1", "Flow 1" }
                    option { value: "mesh", "Mesh" }
                    option { value: "ct-index", "CT Index" }
                    option { value: "material-map", "Mat Map" }
                    option { value: "multi-map", "Multi" }
                    option { value: "tile-props", "Tile" }
                    option { value: "sheen-props", "Sheen" }
                    option { value: "sphere-props", "Sphere" }
                    option { value: "tile-matrix", "Tile Matrix" }
                    option { value: "tile-normal-array", "Tile Normal" }
                    option { value: "tile-orb-array", "Tile ORB" }
                    option { value: "detail-diffuse-array", "Detail Diffuse" }
                    option { value: "detail-normal-array", "Detail Normal" }
                    option { value: "unsupported", "Unsupported" }
                    option { value: "view-direction", "View Direction" }
                }
                label { class: "flex items-center justify-between gap-3",
                    span { class: "text-muted-foreground", "Glass" }
                    select {
                        class: "{input_class(\"h-8 w-24 cursor-pointer py-1 text-xs\")}",
                        value: "{glass_blend_mode_value(current.glass_blend_mode)}",
                        onchange: move |event| {
                            let mut next = options();
                            next.glass_blend_mode = parse_glass_blend_mode(&event.value());
                            options.set(next);
                        },
                        option { value: "alpha", "Alpha" }
                        option { value: "additive", "Add" }
                    }
                }
                RenderCheckbox {
                    label: "Normal",
                    checked: current.normal_mapping,
                    on_change: move |checked| {
                        let mut next = options();
                        next.normal_mapping = checked;
                        options.set(next);
                    },
                }
                RenderCheckbox {
                    label: "Bloom",
                    checked: current.bloom,
                    on_change: move |checked| {
                        let mut next = options();
                        next.bloom = checked;
                        options.set(next);
                    },
                }
                RenderCheckbox {
                    label: "Flip Y",
                    checked: current.normal_y_sign < 0.0,
                    on_change: move |checked| {
                        let mut next = options();
                        next.normal_y_sign = if checked { -1.0 } else { 1.0 };
                        options.set(next);
                    },
                }
                input {
                    class: "h-4 w-full cursor-pointer accent-foreground",
                    r#type: "range",
                    min: "0",
                    max: "160",
                    step: "5",
                    value: "{bloom_percent}",
                    disabled: !current.bloom,
                    oninput: move |event| {
                        let mut next = options();
                        next.bloom_strength = parse_render_slider_value(&event.value()) / 100.0;
                        options.set(next);
                    },
                }
            }
        }
    }
}

#[component]
fn RenderCheckbox(label: &'static str, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "flex items-center justify-between gap-3",
            span { class: "text-muted-foreground", "{label}" }
            input {
                class: "h-4 w-4 accent-foreground",
                r#type: "checkbox",
                checked,
                onchange: move |event| on_change.call(event.checked()),
            }
        }
    }
}

/// 骨骼动画下拉：有动画集时出现。None=rest（绑定姿势），其余为动画名列表。
#[component]
pub(crate) fn AnimationControls(
    animations: AnimationSetHandle,
    selected: Option<usize>,
    on_select: EventHandler<Option<usize>>,
) -> Element {
    let animations = animations.0;
    let select_class = input_class("h-8 w-full cursor-pointer py-1 text-xs");
    rsx! {
        section { class: "space-y-2 text-xs",
            div { class: "flex items-center justify-between gap-3",
                span { class: "text-sm font-semibold", "动画" }
                span { class: "text-[11px] text-muted-foreground",
                    "{animations.animations.len()} 个"
                }
            }
            select {
                class: "{select_class}",
                value: "{selected.map(|index| index.to_string()).unwrap_or_default()}",
                onchange: move |event| {
                    let value = event.value().parse::<usize>().ok();
                    on_select.call(value);
                },
                option { value: "", "Rest（绑定姿势）" }
                for (index, animation) in animations.animations.iter().enumerate() {
                    option { value: "{index}", "{animation.name}" }
                }
            }
        }
    }
}

/// 部件变体（attribute submesh）勾选列表：模型无 attribute 选项时不渲染。
/// 默认全部关闭，对应游戏默认状态。
#[component]
fn WeaponAttributeControls(
    model: Rc<WeaponModelData>,
    attribute_mask: u32,
    attribute_parts_only: bool,
    on_attribute_change: EventHandler<u32>,
    on_parts_only_change: EventHandler<bool>,
) -> Element {
    let options = model_attribute_options(model.as_ref());
    if options.is_empty() {
        return rsx! {};
    }

    rsx! {
        section { class: "space-y-2 text-xs",
            div { class: "flex items-center justify-between gap-3",
                span { class: "text-sm font-semibold", "部件变体" }
                span { class: "text-[11px] text-muted-foreground", "默认全关" }
            }
            div { class: "space-y-1.5",
                for option in options {
                    AttributeCheckbox {
                        key: "{option.bit}",
                        label: attribute_option_label(&option),
                        checked: attribute_mask & option.bit != 0,
                        on_change: move |checked| {
                            on_attribute_change
                                .call(toggle_attribute_mask(attribute_mask, option.bit, checked));
                        },
                    }
                }
            }
            // 变体部件按骨骼绑定姿势存放，与本体重叠是数据的真实状态；
            // 隔离开关让用户只看选中的部件本身。
            div { class: "border-t border-border/60 pt-1.5",
                RenderCheckbox {
                    label: "仅显示选中部件",
                    checked: attribute_parts_only,
                    on_change: move |checked| on_parts_only_change.call(checked),
                }
            }
        }
    }
}

#[component]
fn AttributeCheckbox(label: String, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "flex items-center justify-between gap-3",
            span { class: "truncate font-mono text-[11px] text-muted-foreground", title: "{label}", "{label}" }
            input {
                class: "h-4 w-4 shrink-0 accent-foreground",
                r#type: "checkbox",
                checked,
                onchange: move |event| on_change.call(event.checked()),
            }
        }
    }
}

fn parse_render_slider_value(value: &str) -> f32 {
    value.parse::<f32>().unwrap_or(0.0).clamp(0.0, 160.0)
}

#[component]
fn WeaponStainControls(
    stains: Vec<WeaponStain>,
    stain_ids: [u8; 2],
    race_id: Option<u16>,
    on_race_change: EventHandler<u16>,
    on_stain_change: EventHandler<(usize, u8)>,
) -> Element {
    let stains_available = !stains.is_empty();
    rsx! {
        div { class: "mt-3 flex flex-wrap items-end gap-3 border-t pt-3",
            if let Some(race_id) = race_id {
                EquipmentRaceControl {
                    race_id,
                    onchange: move |value| on_race_change.call(value),
                }
            }
            div { class: "pb-2 text-xs font-medium text-muted-foreground", "染色" }
            WeaponStainControl {
                label: "通道 1",
                stains: stains.clone(),
                value: stain_ids[0],
                onchange: move |value| on_stain_change.call((0, value)),
            }
            WeaponStainControl {
                label: "通道 2",
                stains,
                value: stain_ids[1],
                onchange: move |value| on_stain_change.call((1, value)),
            }
            if !stains_available {
                div { class: "pb-2 text-xs text-amber-700",
                    "当前武器索引缺少染剂数据，请在数据来源中更新武器索引"
                }
            }
        }
    }
}

#[component]
fn EquipmentRaceControl(race_id: u16, onchange: EventHandler<u16>) -> Element {
    let select_class = input_class("h-9 min-w-40 cursor-pointer py-1 text-xs");
    rsx! {
        label { class: "min-w-0 space-y-1",
            span { class: "block text-[11px] text-muted-foreground", "种族模型" }
            select {
                class: "{select_class}",
                value: "{race_id}",
                onchange: move |event| {
                    if let Some(race_id) = parse_equipment_race_id(&event.value()) {
                        onchange.call(race_id);
                    }
                },
                for race in EQUIPMENT_RACES {
                    option { value: "{race.id}", "{race.label}" }
                }
            }
        }
    }
}

#[component]
fn WeaponStainControl(
    label: &'static str,
    stains: Vec<WeaponStain>,
    value: u8,
    onchange: EventHandler<u8>,
) -> Element {
    let selected = stains.iter().find(|stain| stain.id == value);
    let swatch_style = selected
        .map(|stain| {
            format!(
                "background-color: rgb({}, {}, {});",
                stain.ui_color[0], stain.ui_color[1], stain.ui_color[2]
            )
        })
        .unwrap_or_else(|| "background-color: transparent;".to_string());
    let swatch_title = selected
        .map(|stain| stain.name.clone())
        .unwrap_or_else(|| "无染色".to_string());
    let select_class = input_class("h-9 min-w-40 cursor-pointer py-1 text-xs");
    let disabled = stains.is_empty();

    rsx! {
        label { class: "min-w-0 space-y-1",
            span { class: "block text-[11px] text-muted-foreground", "{label}" }
            div { class: "flex items-center gap-2",
                span {
                    class: "h-7 w-7 shrink-0 rounded border border-border shadow-sm",
                    style: "{swatch_style}",
                    title: "{swatch_title}",
                }
                select {
                    class: "{select_class}",
                    value: "{value}",
                    disabled,
                    onchange: move |event| {
                        onchange.call(parse_stain_id(&event.value()));
                    },
                    option { value: "0", "无染色" }
                    for stain in stains {
                        option { value: "{stain.id}",
                            if stain.metallic {
                                "{stain.name} · 金属"
                            } else {
                                "{stain.name}"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn debug_mode_value(mode: ModelDebugMode) -> &'static str {
    match mode {
        ModelDebugMode::Final => "final",
        ModelDebugMode::BaseColor => "base",
        ModelDebugMode::Normal => "normal",
        ModelDebugMode::Mask => "mask",
        ModelDebugMode::MaterialProperties => "material",
        ModelDebugMode::Specular => "specular",
        ModelDebugMode::Emissive => "emissive",
        ModelDebugMode::Alpha => "alpha",
        ModelDebugMode::Uv0 => "uv0",
        ModelDebugMode::Uv1 => "uv1",
        ModelDebugMode::Uv2 => "uv2",
        ModelDebugMode::Uv3 => "uv3",
        ModelDebugMode::VertexColor => "vertex",
        ModelDebugMode::MeshRole => "mesh",
        ModelDebugMode::ColorTableIndex => "ct-index",
        ModelDebugMode::MaterialMap => "material-map",
        ModelDebugMode::MultiMap => "multi-map",
        ModelDebugMode::TileProperties => "tile-props",
        ModelDebugMode::SheenProperties => "sheen-props",
        ModelDebugMode::SphereProperties => "sphere-props",
        ModelDebugMode::TileMatrix => "tile-matrix",
        ModelDebugMode::TileNormalArray => "tile-normal-array",
        ModelDebugMode::TileOrbArray => "tile-orb-array",
        ModelDebugMode::DetailDiffuseArray => "detail-diffuse-array",
        ModelDebugMode::DetailNormalArray => "detail-normal-array",
        ModelDebugMode::VertexColor1 => "vertex1",
        ModelDebugMode::SecondaryNormal => "normal1",
        ModelDebugMode::Flow0 => "flow0",
        ModelDebugMode::Flow1 => "flow1",
        ModelDebugMode::UnsupportedInputs => "unsupported",
        ModelDebugMode::ViewDirection => "view-direction",
    }
}

fn parse_debug_mode(value: &str) -> ModelDebugMode {
    match value {
        "base" => ModelDebugMode::BaseColor,
        "normal" => ModelDebugMode::Normal,
        "mask" => ModelDebugMode::Mask,
        "material" => ModelDebugMode::MaterialProperties,
        "specular" => ModelDebugMode::Specular,
        "emissive" => ModelDebugMode::Emissive,
        "alpha" => ModelDebugMode::Alpha,
        "uv0" => ModelDebugMode::Uv0,
        "uv1" => ModelDebugMode::Uv1,
        "uv2" => ModelDebugMode::Uv2,
        "uv3" => ModelDebugMode::Uv3,
        "vertex" => ModelDebugMode::VertexColor,
        "mesh" => ModelDebugMode::MeshRole,
        "ct-index" => ModelDebugMode::ColorTableIndex,
        "material-map" => ModelDebugMode::MaterialMap,
        "multi-map" => ModelDebugMode::MultiMap,
        "tile-props" => ModelDebugMode::TileProperties,
        "sheen-props" => ModelDebugMode::SheenProperties,
        "sphere-props" => ModelDebugMode::SphereProperties,
        "tile-matrix" => ModelDebugMode::TileMatrix,
        "tile-normal-array" => ModelDebugMode::TileNormalArray,
        "tile-orb-array" => ModelDebugMode::TileOrbArray,
        "detail-diffuse-array" => ModelDebugMode::DetailDiffuseArray,
        "detail-normal-array" => ModelDebugMode::DetailNormalArray,
        "vertex1" => ModelDebugMode::VertexColor1,
        "normal1" => ModelDebugMode::SecondaryNormal,
        "flow0" => ModelDebugMode::Flow0,
        "flow1" => ModelDebugMode::Flow1,
        "unsupported" => ModelDebugMode::UnsupportedInputs,
        "view-direction" => ModelDebugMode::ViewDirection,
        _ => ModelDebugMode::Final,
    }
}

fn glass_blend_mode_value(mode: ModelGlassBlendMode) -> &'static str {
    match mode {
        ModelGlassBlendMode::Alpha => "alpha",
        ModelGlassBlendMode::Additive => "additive",
    }
}

fn parse_glass_blend_mode(value: &str) -> ModelGlassBlendMode {
    match value {
        "additive" => ModelGlassBlendMode::Additive,
        "alpha" | "multiply" => ModelGlassBlendMode::Alpha,
        _ => ModelGlassBlendMode::Alpha,
    }
}

/// 模型画布的 DOM id。画布在物品切换、加载中与出错期间始终挂载，id 保持稳定；
/// WebGPU context 只在画布首次挂载后初始化一次，之后切换模型走 `set_model`。
const WEAPON_MODEL_CANVAS_ID: &str = "weapon-model-canvas";

#[component]
pub(crate) fn WeaponModelCanvas(
    model: Option<Rc<WeaponModelData>>,
    render_options: Signal<WeaponRenderOptions>,
    shape_mask: Option<u32>,
    race_id: u16,
    attribute_mask: u32,
    attribute_parts_only: bool,
    /// 按 attribute 名启用（角色拼装）：Some 时优先于 `attribute_mask`，
    /// 位号映射回各 MDL 本地 attribute 名逐名核对。
    #[props(default)]
    enabled_attribute_names: Option<Vec<String>>,
    /// 多 component 预览布局：true（默认）沿 X 轴平铺拆开（武器/家具），false
    /// 部件按游戏坐标原位重叠（角色拼装）。调用方按页面语义固定传入。
    #[props(default = true)]
    component_preview_layout: bool,
    /// 模型 Reload 修订号：同一 instance key 下重新加载出新模型（如角色装配换
    /// 发型/颜色）时递增，驱动画布重新 set_model；恒为 0（默认）时行为不变。
    #[props(default = 0_u64)]
    model_revision: u64,
    /// 骨骼动画播放状态（有动画集时 Some）。None 时渲染行为与无动画一致。
    #[props(default)]
    animation: Option<AnimationPlaybackState>,
) -> Element {
    let init_error = use_signal(|| None::<String>);
    let ready = use_signal(|| false);

    #[cfg(target_arch = "wasm32")]
    {
        let renderer = use_signal(|| None::<WasmRc<RefCell<WebWeaponCanvasRenderer>>>);
        let init_generation = use_signal(|| 0_u64);
        let init_in_flight = use_signal(|| false);
        // 动画播放状态镜像：prop 在渲染期快照，rAF 循环经 Signal 读取最新值。
        let animation_signal = use_signal(|| animation.clone());
        // 实例重建计数：set_model 后递增，rAF 循环据此刷新 joint 名表缓存。
        let joint_epoch = use_signal(|| 0_u64);
        let instance_key = model.as_ref().map(|model| {
            model_instance_key(
                model,
                shape_mask,
                race_id,
                attribute_mask,
                attribute_parts_only,
            )
        });
        let renderer_ready = renderer.read().is_some();

        // WebGPU context 一次性初始化：不依赖模型数据，与模型加载并行进行。
        // 失败时下一次实例 key 变化重试，对齐画布重建时的重试行为。
        let mut effect_error = init_error;
        let mut effect_ready = ready;
        let mut effect_renderer = renderer;
        let mut effect_generation = init_generation;
        let mut effect_in_flight = init_in_flight;
        let effect_animation = animation_signal;
        let mut effect_joint_epoch = joint_epoch;
        use_effect(use_reactive((&instance_key,), move |_| {
            if effect_renderer.peek().is_some() || *effect_in_flight.peek() {
                return;
            }
            let generation = *effect_generation.peek() + 1;
            effect_generation.set(generation);
            effect_in_flight.set(true);
            let options = render_options;
            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let window =
                        web_sys::window().ok_or_else(|| "当前运行环境没有 window".to_string())?;
                    let document = window
                        .document()
                        .ok_or_else(|| "当前运行环境没有 document".to_string())?;
                    let canvas = document
                        .get_element_by_id(WEAPON_MODEL_CANVAS_ID)
                        .ok_or_else(|| "canvas 未挂载".to_string())?
                        .dyn_into::<HtmlCanvasElement>()
                        .map_err(|_| "canvas 元素类型错误".to_string())?;
                    WebWeaponCanvasRenderer::from_canvas(canvas).await
                }
                .await;

                effect_in_flight.set(false);
                match result {
                    Ok(renderer) => {
                        if *effect_generation.peek() != generation {
                            return;
                        }
                        let renderer = WasmRc::new(RefCell::new(renderer));
                        effect_renderer.set(Some(renderer.clone()));
                        effect_error.set(None);
                        effect_ready.set(true);
                        start_weapon_render_loop(
                            renderer,
                            options,
                            effect_animation,
                            effect_joint_epoch,
                            effect_generation,
                            generation,
                        )
                    }
                    Err(error) if *effect_generation.peek() == generation => {
                        effect_error.set(Some(error))
                    }
                    Err(_) => {}
                }
            });
        }));

        // 动画播放状态 prop → Signal 镜像（rAF 循环每帧读取最新状态）。
        let mut effect_animation = animation_signal;
        use_effect(use_reactive((&animation,), move |(next,)| {
            if effect_animation() != next {
                effect_animation.set(next);
            }
        }));

        // 物品/模型/shape/种族/部件变体变化时同步重建 GPU 实例：复用常驻
        // context，不再重新初始化设备与管线。物品/模型/种族变化重置轨道相机
        // 视角（对齐画布重建的旧行为）；shape 与部件变体变化保持视角。
        let set_model_key = (instance_key, model_revision, renderer_ready);
        let instance_model = model.clone();
        let instance_attribute_names = enabled_attribute_names.clone();
        let mut last_orbit_key = use_signal(|| None::<(u32, u64, u64, u16)>);
        use_effect(use_reactive((&set_model_key,), move |_| {
            let (Some(model), Some(_)) = (instance_model.clone(), instance_key) else {
                return;
            };
            let Some(renderer) = renderer.peek().clone() else {
                return;
            };
            let mut prepared_options = PreparedModelOptions::default()
                .with_component_preview_layout(component_preview_layout);
            if let Some(mask) = shape_mask {
                prepared_options = prepared_options.with_enabled_shape_mask(mask);
            }
            // 模型无 attribute submesh 时不设置 attribute 相关选项（保持
            // None/false），由准备管线按游戏默认处理。角色拼装按名启用
            // （位是 MDL 本地表序，跨模型数值不可比），优先于数值掩码。
            if model_has_attribute_submeshes(&model) {
                if let Some(names) = &instance_attribute_names {
                    prepared_options =
                        prepared_options.with_enabled_attribute_names(names.clone());
                } else {
                    prepared_options = prepared_options
                        .with_enabled_attribute_mask(attribute_mask)
                        .with_attribute_parts_only(attribute_parts_only);
                }
            }
            let orbit_key = model_orbit_reset_key(&model, race_id);
            let mut renderer = renderer.borrow_mut();
            renderer.set_model(&model, prepared_options);
            // 实例重建后 joint 名表/缓冲均重置（rest）：rAF 循环据此刷新动画运行时。
            let next_epoch = *effect_joint_epoch.peek() + 1;
            effect_joint_epoch.set(next_epoch);
            if *last_orbit_key.peek() != Some(orbit_key) {
                renderer.reset_orbit();
                last_orbit_key.set(Some(orbit_key));
            }
        }));

        // 染色变化走增量材质更新，不重建实例。
        let update_key = model.as_ref().map(|model| model.stain_ids);
        let update_model = model.clone();
        use_effect(use_reactive((&update_key,), move |_| {
            let (Some(model), Some(renderer)) = (update_model.clone(), renderer.peek().clone())
            else {
                return;
            };
            renderer.borrow_mut().update_materials(&model);
        }));
    }

    rsx! {
        div { class: "absolute inset-0",
            canvas {
                id: WEAPON_MODEL_CANVAS_ID,
                class: "h-full w-full cursor-grab touch-none select-none bg-[#0e1117] active:cursor-grabbing",
            }
            if let Some(error) = init_error() {
                div { class: "absolute inset-x-4 top-4 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-900 shadow-sm",
                    "{error}"
                }
            }
            if cfg!(target_arch = "wasm32") && !ready() && init_error().is_none() {
                WeaponModelLoadingView {
                    progress: None,
                    stage: Some("正在初始化 WebGPU 渲染".to_string()),
                }
            }
            if !cfg!(target_arch = "wasm32") {
                div { class: "absolute inset-0 flex items-center justify-center p-6 text-sm text-muted-foreground",
                    "WebGPU canvas 仅在 wasm32 web 构建中启用"
                }
            }
        }
    }
}

/// rAF 动画播放运行时：joint 名表 + inverse bind 缓存随实例重建（joint_epoch）
/// 刷新；`playing` 记录当前 joint buffer 对应的动画（None = 已是 rest）。
#[cfg(target_arch = "wasm32")]
struct AnimationLoopRuntime {
    joint_names: Vec<String>,
    inverse_bind: xiv_companion::SkeletonInverseBindCache,
    playing: Option<usize>,
    started_at_ms: f64,
    epoch: u64,
}

#[cfg(target_arch = "wasm32")]
fn start_weapon_render_loop(
    renderer: WasmRc<RefCell<WebWeaponCanvasRenderer>>,
    render_options: Signal<WeaponRenderOptions>,
    animation: Signal<Option<AnimationPlaybackState>>,
    joint_epoch: Signal<u64>,
    generation: Signal<u64>,
    expected_generation: u64,
) {
    let callback_slot: WasmRc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
        WasmRc::new(RefCell::new(None));
    let callback_slot_for_loop = callback_slot.clone();
    let renderer_for_loop = renderer.clone();
    let mut animation_runtime: Option<AnimationLoopRuntime> = None;

    *callback_slot.borrow_mut() = Some(Closure::wrap(Box::new(move |time_ms: f64| {
        let connected = {
            let mut renderer = renderer_for_loop.borrow_mut();
            if renderer.canvas_connected() && *generation.peek() == expected_generation {
                drive_animation_playback(
                    &mut renderer,
                    &animation(),
                    *joint_epoch.peek(),
                    &mut animation_runtime,
                    time_ms,
                );
                let mut options = render_options();
                options.uv_scroll_time = (time_ms as f32) / 1000.0;
                renderer.render_with_options(options);
                true
            } else {
                false
            }
        };

        if connected {
            if let (Some(window), Some(callback)) =
                (web_sys::window(), callback_slot_for_loop.borrow().as_ref())
            {
                let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            }
        } else {
            let _ = callback_slot_for_loop.borrow_mut().take();
        }
    }) as Box<dyn FnMut(f64)>));

    if let (Some(window), Some(callback)) = (web_sys::window(), callback_slot.borrow().as_ref()) {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

/// 每帧动画驱动：实例重建后刷新 joint 名表缓存；选中动画时
/// `time = (now - start) % duration` 采样并上传关节矩阵（切换动画/模型
/// 时重置计时）；切回 Rest 时上传一次 rest 关节矩阵。
#[cfg(target_arch = "wasm32")]
fn drive_animation_playback(
    renderer: &mut WebWeaponCanvasRenderer,
    playback: &Option<AnimationPlaybackState>,
    epoch: u64,
    runtime: &mut Option<AnimationLoopRuntime>,
    time_ms: f64,
) {
    if runtime.as_ref().map(|runtime| runtime.epoch) != Some(epoch) {
        *runtime = Some(AnimationLoopRuntime {
            joint_names: renderer.joint_names(),
            inverse_bind: xiv_companion::SkeletonInverseBindCache::new(),
            playing: None,
            started_at_ms: time_ms,
            epoch,
        });
    }
    let Some(runtime) = runtime.as_mut() else {
        return;
    };
    let Some(playback) = playback else {
        // 动画状态消失（切到无动画模型）：joint 缓冲已被新实例重置为 rest。
        runtime.playing = None;
        return;
    };
    match playback.selected {
        Some(index) => {
            let Some(animation) = playback.set.animations.get(index) else {
                return;
            };
            if runtime.playing != Some(index) {
                runtime.playing = Some(index);
                runtime.started_at_ms = time_ms;
            }
            let duration_ms = animation.duration_ms.max(1.0);
            let time = ((time_ms - runtime.started_at_ms) as f32).rem_euclid(duration_ms);
            let matrices = xiv_companion::animation_joint_matrices(
                &playback.set,
                index,
                time,
                &playback.skeleton,
                &runtime.joint_names,
                &mut runtime.inverse_bind,
            );
            renderer.update_joint_matrices(&matrices);
        }
        None => {
            if runtime.playing.take().is_some() {
                let rest = xiv_companion::SkeletonPose::rest_pose(&playback.skeleton);
                let matrices = runtime.inverse_bind.joint_matrices(
                    &playback.skeleton,
                    &rest,
                    &runtime.joint_names,
                );
                renderer.update_joint_matrices(&matrices);
            }
        }
    }
}

fn search_model_items(
    collection: &CollectionCatalogPackage,
    furniture: Option<&FurnitureCatalogPackage>,
    chara: Option<&CharaCatalogPackage>,
    query: &str,
    filter: ModelItemFilter,
) -> ModelSearchResult {
    let needle = query.trim().to_lowercase();
    let mut total = 0;
    let mut items = Vec::new();

    if filter.includes_equipment() {
        for item in &collection.items {
            if !item.is_equipment()
                || !filter.matches_equipment(item)
                || !equipment_matches_query(item, &needle)
            {
                continue;
            }
            total += 1;
            if items.len() < RESULT_LIMIT {
                items.push(ModelCatalogItem::Equipment(item.clone()));
            }
        }
    }

    let furniture = furniture.filter(|_| filter.includes_furniture());
    if let Some(furniture) = furniture {
        for item in &furniture.items {
            if !filter.matches_furniture(item) || !furniture_matches_query(item, &needle) {
                continue;
            }
            total += 1;
            if items.len() < RESULT_LIMIT {
                items.push(ModelCatalogItem::Furniture(item.clone()));
            }
        }
    }

    let chara = chara.filter(|_| filter.includes_chara());
    if let Some(chara) = chara {
        for item in &chara.items {
            if !filter.matches_chara(item) || !chara_matches_query(item, &needle) {
                continue;
            }
            total += 1;
            if items.len() < RESULT_LIMIT {
                items.push(ModelCatalogItem::Chara(item.clone()));
            }
        }
    }

    ModelSearchResult { total, items }
}

fn equipment_matches_query(item: &CollectionItem, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if item.name.to_lowercase().contains(needle)
        || item.id.to_string().contains(needle)
        || item.model_main.to_string().contains(needle)
        || item.model_sub.to_string().contains(needle)
        || format!("{:x}", item.model_main).contains(needle)
        || format!("{:x}", item.model_sub).contains(needle)
    {
        return true;
    }

    let segments: [u16; 6] = if is_weapon_equip_slot_category(item.equip_slot_category) {
        let main = PackedModelId::from_raw(item.model_main);
        let sub = PackedModelId::from_raw(item.model_sub);
        [
            main.model_id,
            main.body_id,
            main.variant_id,
            sub.model_id,
            sub.body_id,
            sub.variant_id,
        ]
    } else {
        let main = PackedEquipmentModelId::from_raw(item.model_main);
        let sub = PackedEquipmentModelId::from_raw(item.model_sub);
        [
            main.set_id,
            main.variant_id,
            sub.set_id,
            sub.variant_id,
            0,
            0,
        ]
    };
    segments
        .iter()
        .any(|value| value.to_string().contains(needle))
}

fn furniture_matches_query(item: &FurnitureCatalogItem, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    item.name.to_lowercase().contains(needle)
        || item.id.to_string().contains(needle)
        || item.model_key.to_string().contains(needle)
        || format!("{:04}", item.model_key).contains(needle)
        || format_furniture_model(item.kind, item.model_key).contains(needle)
}

fn chara_matches_query(item: &CharaCatalogItem, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    item.name.to_lowercase().contains(needle)
        || item.id.to_string().contains(needle)
        || item.model.model_id.to_string().contains(needle)
        || item.model.base_id.to_string().contains(needle)
        || item.model.variant_id.to_string().contains(needle)
        || format_chara_model(&item.model).contains(needle)
}

fn initial_model_preview_url_state() -> ModelPreviewUrlState {
    #[cfg(target_arch = "wasm32")]
    {
        model_preview_url_state_from_hash(
            web_sys::window()
                .and_then(|window| window.location().hash().ok())
                .unwrap_or_default()
                .as_str(),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        ModelPreviewUrlState {
            query: String::new(),
            filter: ModelItemFilter::All,
            item_id: None,
            stain_ids: [0, 0],
            race_id: EQUIPMENT_MODEL_FALLBACK_RACE_ID,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn model_preview_url_state_from_hash(hash: &str) -> ModelPreviewUrlState {
    let route = hash.trim_start_matches('#');
    let (path, query) = route.split_once('?').unwrap_or((route, ""));
    let mut state = ModelPreviewUrlState {
        query: String::new(),
        filter: default_filter_for_path(path),
        item_id: None,
        stain_ids: [0, 0],
        race_id: EQUIPMENT_MODEL_FALLBACK_RACE_ID,
    };

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = decode_query_value(value);
        match key {
            "q" | "query" | "search" => state.query = value,
            "f" | "filter" | "slot" => {
                if let Some(filter) = ModelItemFilter::from_key(&value) {
                    state.filter = filter;
                }
            }
            "item" | "itemId" | "id" => {
                state.item_id = value.parse::<u32>().ok();
            }
            "stain0" | "dye0" => state.stain_ids[0] = parse_stain_id(&value),
            "stain1" | "dye1" => state.stain_ids[1] = parse_stain_id(&value),
            "race" => {
                if let Some(race_id) = parse_equipment_race_id(&value) {
                    state.race_id = race_id;
                }
            }
            _ => {}
        }
    }

    state
}

/// 旧 `/weapon-models` 路由进入时默认只看武器，保持原武器页的浏览习惯。
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn default_filter_for_path(path: &str) -> ModelItemFilter {
    if path == WEAPON_MODELS_ROUTE_PATH {
        ModelItemFilter::Weapons
    } else {
        ModelItemFilter::All
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn decode_query_value(value: &str) -> String {
    urlencoding::decode(value)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| value.to_string())
}

fn parse_stain_id(value: &str) -> u8 {
    value
        .parse::<u8>()
        .ok()
        .filter(|stain_id| *stain_id <= 254)
        .unwrap_or(0)
}

#[allow(unused_variables)]
fn sync_model_preview_url_state(
    query: &str,
    filter: ModelItemFilter,
    item_id: Option<u32>,
    stain_ids: [u8; 2],
    race_id: u16,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return;
        };
        let current_path = window
            .location()
            .hash()
            .ok()
            .map(|hash| {
                hash.trim_start_matches('#')
                    .split('?')
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_default();
        let base = if current_path == WEAPON_MODELS_ROUTE_PATH {
            WEAPON_MODELS_ROUTE_PATH
        } else {
            EQUIPMENT_MODELS_ROUTE_PATH
        };

        let mut params = Vec::new();
        let trimmed_query = query.trim();
        if !trimmed_query.is_empty() {
            params.push(format!("q={}", urlencoding::encode(trimmed_query)));
        }
        if filter != default_filter_for_path(base) {
            params.push(format!("f={}", filter.key()));
        }
        if let Some(item_id) = item_id {
            params.push(format!("item={item_id}"));
        }
        if stain_ids[0] != 0 {
            params.push(format!("stain0={}", stain_ids[0]));
        }
        if stain_ids[1] != 0 {
            params.push(format!("stain1={}", stain_ids[1]));
        }
        if race_id != EQUIPMENT_MODEL_FALLBACK_RACE_ID {
            params.push(format!("race={race_id}"));
        }

        let hash = if params.is_empty() {
            format!("#{base}")
        } else {
            format!("#{base}?{}", params.join("&"))
        };

        if window.location().hash().ok().as_deref() == Some(hash.as_str()) {
            return;
        }

        match window.history() {
            Ok(history) => {
                let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&hash));
            }
            Err(_) => {
                let _ = window.location().set_hash(hash.trim_start_matches('#'));
            }
        }
    }
}

fn segment_button_class(active: bool) -> &'static str {
    if active {
        "flex h-7 items-center justify-center rounded bg-background text-xs font-medium text-foreground shadow-sm transition-colors"
    } else {
        "flex h-7 items-center justify-center rounded text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
    }
}

fn equipment_slot_label(category: u32) -> &'static str {
    if is_weapon_equip_slot_category(category) {
        return weapon_slot_label(category);
    }
    match category {
        3 => "头部",
        4 => "身体",
        5 => "手部",
        6 => "腰部",
        7 => "腿部",
        8 => "脚部",
        9 => "耳饰",
        10 => "项链",
        11 => "手镯",
        12 => "戒指",
        _ => "复合部位",
    }
}

fn format_item_model(item: &CollectionItem) -> Option<String> {
    (item.model_main != 0).then(|| format_model_raw(item.equip_slot_category, item.model_main))
}

fn format_item_sub_model(item: &CollectionItem) -> Option<String> {
    (item.model_sub != 0).then(|| format_model_raw(item.equip_slot_category, item.model_sub))
}

fn format_model_raw(equip_slot_category: u32, raw: u64) -> String {
    if is_weapon_equip_slot_category(equip_slot_category) {
        return format_packed_model(PackedModelId::from_raw(raw));
    }
    let model = PackedEquipmentModelId::from_raw(raw);
    let prefix = match equipment_slot_info(equip_slot_category) {
        Some(slot) if slot.is_accessory => "a",
        _ => "e",
    };
    format!("{prefix}{:04} v{:04}", model.set_id, model.variant_id)
}

fn format_packed_model(model: PackedModelId) -> String {
    format!(
        "w{:04} b{:04} v{:04}",
        model.model_id, model.body_id, model.variant_id,
    )
}

/// 家具/庭具模型标签：SGB 资源文件名（`fun_b0_m####` / `gar_b0_m####`），
/// 与 SqPack 加载路径直接对应。
fn format_furniture_model(kind: FurnitureModelKind, model_key: u16) -> String {
    let prefix = match kind {
        FurnitureModelKind::Indoor => "fun_b0_m",
        FurnitureModelKind::Outdoor => "gar_b0_m",
    };
    format!("{prefix}{model_key:04}")
}

/// 宠物/坐骑模型标签：模型种类 + 模型文件主名（monster `m####b####` /
/// demihuman `d####e####`），与 SqPack 加载路径直接对应。
fn format_chara_model(model: &PackedCharaModelId) -> String {
    match model.chara_type {
        CharaModelType::Monster => {
            format!("monster m{:04}b{:04}", model.model_id, model.base_id)
        }
        CharaModelType::Demihuman => {
            format!("demihuman d{:04}e{:04}", model.model_id, model.base_id)
        }
    }
}

fn format_vec3(value: [f32; 3]) -> String {
    format!("{:.3}, {:.3}, {:.3}", value[0], value[1], value[2])
}

/// GPU 实例 key：物品/模型/shape/种族/部件变体（含隔离预览）任一变化都需要经
/// `set_model` 同步重建模型实例（复用常驻渲染 context）。染色不在其中——
/// 染色走 `update_materials` 增量路径。
fn model_instance_key(
    model: &WeaponModelData,
    shape_mask: Option<u32>,
    race_id: u16,
    attribute_mask: u32,
    attribute_parts_only: bool,
) -> (u32, u64, u64, Option<u32>, u16, u32, bool) {
    (
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
        shape_mask,
        race_id,
        attribute_mask,
        attribute_parts_only,
    )
}

/// 轨道相机重置 key：物品/模型/种族变化时重置视角（对齐画布重建的旧行为），
/// shape、部件变体与染色变化保持当前视角。
fn model_orbit_reset_key(model: &WeaponModelData, race_id: u16) -> (u32, u64, u64, u16) {
    (
        model.item_id,
        model.model_main.raw,
        model.model_sub.map(|value| value.raw).unwrap_or(0),
        race_id,
    )
}

fn model_shape_options(model: &WeaponModelData) -> Vec<(usize, u32, String)> {
    let mut options = Vec::new();
    for shape in model
        .meshes
        .iter()
        .flat_map(|mesh| mesh.shape_influences.iter())
    {
        if shape.shape_index_mask == 0
            || options
                .iter()
                .any(|(_, mask, _)| *mask == shape.shape_index_mask)
        {
            continue;
        }
        options.push((
            shape.index,
            shape.shape_index_mask,
            shape
                .name
                .clone()
                .unwrap_or_else(|| format!("Shape {}", shape.index)),
        ));
    }
    options.sort_by_key(|(index, _, _)| *index);
    options
}

fn model_has_shape_mask(model: &WeaponModelData, shape_mask: u32) -> bool {
    model
        .meshes
        .iter()
        .flat_map(|mesh| mesh.shape_influences.iter())
        .any(|shape| shape.shape_index_mask == shape_mask)
}

fn model_has_attribute_submeshes(model: &WeaponModelData) -> bool {
    model.meshes.iter().any(|mesh| {
        mesh.submesh
            .as_ref()
            .is_some_and(|submesh| submesh.attribute_index_mask != 0)
    })
}

/// 部件变体选择按物品 id 作用域存储；切换到其他物品时读回 0（全部关闭，
/// 对应游戏默认状态）。
fn scoped_attribute_mask(selection: (Option<u32>, u32), item_id: u32) -> u32 {
    if selection.0 == Some(item_id) {
        selection.1
    } else {
        0
    }
}

/// 隔离预览开关同样按物品 id 作用域存储；切换物品后回到关闭。
fn scoped_attribute_parts_only(selection: (Option<u32>, bool), item_id: u32) -> bool {
    selection.0 == Some(item_id) && selection.1
}

/// 动画选择按物品 id 作用域存储；切换物品后回到 None（rest）。
fn scoped_animation_selection(selection: (Option<u32>, Option<usize>), item_id: u32) -> Option<usize> {
    (selection.0 == Some(item_id)).then_some(selection.1).flatten()
}

/// 动画播放状态：骨架 + 非空动画集齐备时可用（UI 出现「动画」下拉）。
pub(crate) fn animation_playback(
    skeleton: &Option<Rc<ModelSkeleton>>,
    animations: &Option<Rc<ModelAnimationSet>>,
    selected: Option<usize>,
) -> Option<AnimationPlaybackState> {
    let (skeleton, animations) = skeleton.clone().zip(animations.clone())?;
    if animations.is_empty() {
        return None;
    }
    Some(AnimationPlaybackState {
        set: animations,
        skeleton,
        selected,
    })
}

fn toggle_attribute_mask(mask: u32, bit: u32, enabled: bool) -> u32 {
    if enabled { mask | bit } else { mask & !bit }
}

/// 变体勾选项的显示名：属性名并列；无名字的 bit 回退为位数编号。
fn attribute_option_label(option: &ModelAttributeOption) -> String {
    if option.names.is_empty() {
        format!("Attribute {}", option.bit.trailing_zeros())
    } else {
        option.names.join(" / ")
    }
}

#[cfg(test)]
mod model_preview_tests {
    use super::*;

    #[test]
    fn parses_model_preview_url_state_from_hash() {
        let state = model_preview_url_state_from_hash(
            "#/equipment-models?q=%E6%B5%AA%E6%BC%AB&f=head&item=45058&stain0=17&stain1=93&race=701",
        );
        assert_eq!(state.query, "浪漫");
        assert_eq!(state.filter, ModelItemFilter::Head);
        assert_eq!(state.item_id, Some(45058));
        assert_eq!(state.stain_ids, [17, 93]);
        assert_eq!(state.race_id, 701);

        let state = model_preview_url_state_from_hash("#/equipment-models?f=furniture&item=43570");
        assert_eq!(state.filter, ModelItemFilter::Furniture);
        assert_eq!(state.item_id, Some(43570));

        let state = model_preview_url_state_from_hash("#/equipment-models?f=yard");
        assert_eq!(state.filter, ModelItemFilter::Yard);

        let state = model_preview_url_state_from_hash("#/equipment-models?f=minions&item=42722");
        assert_eq!(state.filter, ModelItemFilter::Minions);
        assert_eq!(state.item_id, Some(42722));

        let state = model_preview_url_state_from_hash("#/equipment-models?f=mounts");
        assert_eq!(state.filter, ModelItemFilter::Mounts);
    }

    #[test]
    fn weapon_models_route_defaults_to_weapon_filter() {
        let state = model_preview_url_state_from_hash("#/weapon-models?item=45058");
        assert_eq!(state.filter, ModelItemFilter::Weapons);
        assert_eq!(state.item_id, Some(45058));

        let state = model_preview_url_state_from_hash(
            "#/weapon-models?q=%E6%B5%AA%E6%BC%AB&f=two&item=45058&stain0=17&stain1=93",
        );
        assert_eq!(state.query, "浪漫");
        assert_eq!(state.filter, ModelItemFilter::TwoHanded);
        assert_eq!(state.stain_ids, [17, 93]);

        let state = model_preview_url_state_from_hash("#/equipment-models");
        assert_eq!(state.filter, ModelItemFilter::All);
    }

    #[test]
    fn race_param_accepts_plain_and_c_prefixed_codes() {
        assert_eq!(parse_equipment_race_id("701"), Some(701));
        assert_eq!(parse_equipment_race_id("c0701"), Some(701));
        assert_eq!(parse_equipment_race_id("C1801"), Some(1801));
        assert_eq!(parse_equipment_race_id("999"), None);
        assert_eq!(parse_equipment_race_id("invalid"), None);

        let state = model_preview_url_state_from_hash("#/equipment-models?race=c1301");
        assert_eq!(state.race_id, 1301);

        let state = model_preview_url_state_from_hash("#/equipment-models?race=999");
        assert_eq!(state.race_id, EQUIPMENT_MODEL_FALLBACK_RACE_ID);
    }

    #[test]
    fn stain_id_parser_rejects_reserved_and_invalid_values() {
        assert_eq!(parse_stain_id("254"), 254);
        assert_eq!(parse_stain_id("255"), 0);
        assert_eq!(parse_stain_id("invalid"), 0);
    }

    #[test]
    fn model_item_filter_keys_round_trip() {
        for filter in [
            ModelItemFilter::All,
            ModelItemFilter::Weapons,
            ModelItemFilter::Main,
            ModelItemFilter::Off,
            ModelItemFilter::TwoHanded,
            ModelItemFilter::Dual,
            ModelItemFilter::Armor,
            ModelItemFilter::Head,
            ModelItemFilter::Body,
            ModelItemFilter::Hands,
            ModelItemFilter::Legs,
            ModelItemFilter::Feet,
            ModelItemFilter::Accessories,
            ModelItemFilter::Ears,
            ModelItemFilter::Neck,
            ModelItemFilter::Wrists,
            ModelItemFilter::Rings,
            ModelItemFilter::Furniture,
            ModelItemFilter::Yard,
            ModelItemFilter::Minions,
            ModelItemFilter::Mounts,
        ] {
            assert_eq!(ModelItemFilter::from_key(filter.key()), Some(filter));
        }
    }

    #[test]
    fn equipment_filters_match_their_equip_slot_categories() {
        let weapon = test_collection_item(13, 100);
        for filter in [
            ModelItemFilter::All,
            ModelItemFilter::Weapons,
            ModelItemFilter::TwoHanded,
        ] {
            assert!(
                filter.matches_equipment(&weapon),
                "{filter:?} should match {weapon:?}"
            );
        }
        assert!(!ModelItemFilter::Main.matches_equipment(&weapon));
        assert!(!ModelItemFilter::Armor.matches_equipment(&weapon));

        let armor = test_collection_item(4, 100);
        assert!(ModelItemFilter::Armor.matches_equipment(&armor));
        assert!(ModelItemFilter::Body.matches_equipment(&armor));
        assert!(!ModelItemFilter::Weapons.matches_equipment(&armor));

        let accessory = test_collection_item(12, 100);
        assert!(ModelItemFilter::Accessories.matches_equipment(&accessory));
        assert!(ModelItemFilter::Rings.matches_equipment(&accessory));
        assert!(!ModelItemFilter::Armor.matches_equipment(&accessory));

        // 腰带（6）与复合部位（15+）只在“全部”下出现。
        for category in [6, 15, 16, 18, 21] {
            let item = test_collection_item(category, 100);
            assert!(ModelItemFilter::All.matches_equipment(&item));
            assert!(!ModelItemFilter::Weapons.matches_equipment(&item));
            assert!(!ModelItemFilter::Armor.matches_equipment(&item));
            assert!(!ModelItemFilter::Accessories.matches_equipment(&item));
        }
    }

    #[test]
    fn furniture_filters_match_their_kinds() {
        let indoor = test_furniture_item(1, FurnitureModelKind::Indoor, 7);
        let outdoor = test_furniture_item(2, FurnitureModelKind::Outdoor, 9);

        assert!(ModelItemFilter::All.matches_furniture(&indoor));
        assert!(ModelItemFilter::Furniture.matches_furniture(&indoor));
        assert!(!ModelItemFilter::Yard.matches_furniture(&indoor));
        assert!(!ModelItemFilter::Weapons.matches_furniture(&indoor));
        assert!(ModelItemFilter::Yard.matches_furniture(&outdoor));
        assert!(!ModelItemFilter::Furniture.matches_furniture(&outdoor));

        // 家具/庭具过滤器只覆盖家具目录，装备过滤器只覆盖装备目录；“全部”两者兼有。
        assert!(ModelItemFilter::Furniture.includes_furniture());
        assert!(!ModelItemFilter::Furniture.includes_equipment());
        assert!(ModelItemFilter::Yard.includes_furniture());
        assert!(!ModelItemFilter::Head.includes_furniture());
        assert!(ModelItemFilter::All.includes_equipment());
        assert!(ModelItemFilter::All.includes_furniture());
        assert!(!ModelItemFilter::Furniture.matches_equipment(&test_collection_item(4, 100)));
        assert!(!ModelItemFilter::Yard.matches_equipment(&test_collection_item(9, 100)));
    }

    #[test]
    fn chara_filters_match_their_kinds() {
        let minion = test_chara_item(1, CharaModelKind::Minion, 8003, CharaModelType::Monster);
        let mount = test_chara_item(2, CharaModelKind::Mount, 1, CharaModelType::Demihuman);

        assert!(ModelItemFilter::All.matches_chara(&minion));
        assert!(ModelItemFilter::Minions.matches_chara(&minion));
        assert!(!ModelItemFilter::Mounts.matches_chara(&minion));
        assert!(!ModelItemFilter::Weapons.matches_chara(&minion));
        assert!(ModelItemFilter::Mounts.matches_chara(&mount));
        assert!(!ModelItemFilter::Minions.matches_chara(&mount));

        // 宠物/坐骑过滤器只覆盖 chara 目录，其余目录过滤器不覆盖 chara；“全部”三者兼有。
        assert!(ModelItemFilter::Minions.includes_chara());
        assert!(!ModelItemFilter::Minions.includes_equipment());
        assert!(!ModelItemFilter::Minions.includes_furniture());
        assert!(ModelItemFilter::Mounts.includes_chara());
        assert!(!ModelItemFilter::Furniture.includes_chara());
        assert!(!ModelItemFilter::Head.includes_chara());
        assert!(ModelItemFilter::All.includes_chara());
        assert!(!ModelItemFilter::Minions.matches_equipment(&test_collection_item(4, 100)));
        assert!(
            !ModelItemFilter::Mounts.matches_furniture(&test_furniture_item(
                3,
                FurnitureModelKind::Indoor,
                1
            ))
        );
    }

    #[test]
    fn model_preview_support_classifies_items() {
        assert_eq!(
            model_preview_support(&test_collection_item(1, 100)),
            ModelPreviewSupport::Weapon
        );
        assert_eq!(
            model_preview_support(&test_collection_item(14, 100)),
            ModelPreviewSupport::Weapon
        );
        assert_eq!(
            model_preview_support(&test_collection_item(4, 100)),
            ModelPreviewSupport::Equipment
        );
        assert_eq!(
            model_preview_support(&test_collection_item(9, 100)),
            ModelPreviewSupport::Equipment
        );
        assert_eq!(
            model_preview_support(&test_collection_item(4, 0)),
            ModelPreviewSupport::NoModel
        );
        assert_eq!(
            model_preview_support(&test_collection_item(6, 100)),
            ModelPreviewSupport::UnsupportedSlot
        );
        assert_eq!(
            model_preview_support(&test_collection_item(16, 100)),
            ModelPreviewSupport::UnsupportedSlot
        );

        for kind in [FurnitureModelKind::Indoor, FurnitureModelKind::Outdoor] {
            let item = ModelCatalogItem::Furniture(test_furniture_item(3, kind, 1));
            assert_eq!(item.support(), ModelPreviewSupport::Furniture);
        }
        assert_eq!(
            ModelCatalogItem::Equipment(test_collection_item(4, 100)).support(),
            ModelPreviewSupport::Equipment
        );

        for (kind, chara_type) in [
            (CharaModelKind::Minion, CharaModelType::Monster),
            (CharaModelKind::Mount, CharaModelType::Demihuman),
        ] {
            let item = ModelCatalogItem::Chara(test_chara_item(5, kind, 1, chara_type));
            assert_eq!(item.support(), ModelPreviewSupport::Chara);
        }
    }

    #[test]
    fn equipment_slot_label_covers_weapon_and_armor_slots() {
        assert_eq!(equipment_slot_label(1), "主手");
        assert_eq!(equipment_slot_label(13), "双手主手");
        assert_eq!(equipment_slot_label(4), "身体");
        assert_eq!(equipment_slot_label(12), "戒指");
        assert_eq!(equipment_slot_label(6), "腰部");
        assert_eq!(equipment_slot_label(21), "复合部位");
    }

    #[test]
    fn item_model_label_uses_weapon_or_equipment_packing() {
        // model_id=2001, body_id=102, variant_id=1
        let weapon = test_collection_item(1, 0x0000_0001_0066_07D1);
        assert_eq!(
            format_item_model(&weapon),
            Some("w2001 b0102 v0001".to_string())
        );

        let armor = test_collection_item(4, 0x0000_0000_0001_2276);
        assert_eq!(format_item_model(&armor), Some("e8822 v0001".to_string()));

        let accessory = test_collection_item(9, 0x0000_0000_0000_0010);
        assert_eq!(
            format_item_model(&accessory),
            Some("a0016 v0000".to_string())
        );

        let no_model = test_collection_item(4, 0);
        assert_eq!(format_item_model(&no_model), None);
    }

    #[test]
    fn furniture_labels_use_sgb_asset_stem_and_kind_label() {
        let indoor =
            ModelCatalogItem::Furniture(test_furniture_item(3, FurnitureModelKind::Indoor, 1));
        assert_eq!(indoor.model_label(), Some("fun_b0_m0001".to_string()));
        assert_eq!(indoor.kind_label(), "室内家具");
        assert_eq!(indoor.sub_model_label(), None);

        let outdoor =
            ModelCatalogItem::Furniture(test_furniture_item(4, FurnitureModelKind::Outdoor, 1234));
        assert_eq!(outdoor.model_label(), Some("gar_b0_m1234".to_string()));
        assert_eq!(outdoor.kind_label(), "庭具");
    }

    #[test]
    fn chara_labels_use_type_and_model_file_stem() {
        let minion = ModelCatalogItem::Chara(test_chara_item(
            5,
            CharaModelKind::Minion,
            8003,
            CharaModelType::Monster,
        ));
        assert_eq!(minion.model_label(), Some("monster m8003b0001".to_string()));
        assert_eq!(minion.kind_label(), "宠物");
        assert_eq!(minion.sub_model_label(), None);

        let mount = ModelCatalogItem::Chara(test_chara_item(
            6,
            CharaModelKind::Mount,
            1,
            CharaModelType::Demihuman,
        ));
        assert_eq!(
            mount.model_label(),
            Some("demihuman d0001e0001".to_string())
        );
        assert_eq!(mount.kind_label(), "坐骑");
    }

    #[test]
    fn chara_matches_query_covers_name_id_and_model() {
        let item = CharaCatalogItem {
            id: 42722,
            kind: CharaModelKind::Minion,
            name: "爆弹仔".to_string(),
            icon: 0,
            model: PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 2,
                chara_type: CharaModelType::Monster,
            },
        };
        assert!(chara_matches_query(&item, ""));
        assert!(chara_matches_query(&item, "爆弹"));
        assert!(chara_matches_query(&item, "42722"));
        assert!(chara_matches_query(&item, "8003"));
        assert!(chara_matches_query(&item, "m8003b0001"));
        assert!(chara_matches_query(&item, "monster"));
        assert!(!chara_matches_query(&item, "demihuman"));
        assert!(!chara_matches_query(&item, "古菩"));
    }

    #[test]
    fn furniture_matches_query_covers_name_id_and_model_key() {
        let item = FurnitureCatalogItem {
            id: 43570,
            kind: FurnitureModelKind::Indoor,
            name: "春意衣柜".to_string(),
            icon: 0,
            model_key: 369,
        };
        assert!(furniture_matches_query(&item, ""));
        assert!(furniture_matches_query(&item, "衣柜"));
        assert!(furniture_matches_query(&item, "43570"));
        assert!(furniture_matches_query(&item, "369"));
        assert!(furniture_matches_query(&item, "0369"));
        assert!(furniture_matches_query(&item, "fun_b0_m0369"));
        assert!(!furniture_matches_query(&item, "gar_b0"));
        assert!(!furniture_matches_query(&item, "书桌"));
    }

    #[test]
    fn search_merges_all_catalogs_under_all() {
        let collection = test_collection_catalog(vec![test_collection_item(4, 100)]);
        let furniture = test_furniture_catalog(vec![
            test_furniture_item(7, FurnitureModelKind::Indoor, 1),
            test_furniture_item(8, FurnitureModelKind::Outdoor, 2),
        ]);
        let chara = test_chara_catalog(vec![
            test_chara_item(11, CharaModelKind::Minion, 8003, CharaModelType::Monster),
            test_chara_item(12, CharaModelKind::Mount, 1, CharaModelType::Demihuman),
        ]);

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::All,
        );
        assert_eq!(result.total, 5);
        assert_eq!(result.items.len(), 5);
        assert!(matches!(result.items[0], ModelCatalogItem::Equipment(_)));
        assert!(matches!(result.items[1], ModelCatalogItem::Furniture(_)));
        assert!(matches!(result.items[3], ModelCatalogItem::Chara(_)));

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::Furniture,
        );
        assert_eq!(result.total, 1);
        assert!(matches!(result.items[0], ModelCatalogItem::Furniture(_)));

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::Yard,
        );
        assert_eq!(result.total, 1);

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::Minions,
        );
        assert_eq!(result.total, 1);
        assert!(matches!(result.items[0], ModelCatalogItem::Chara(_)));

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::Mounts,
        );
        assert_eq!(result.total, 1);

        let result = search_model_items(
            &collection,
            Some(&furniture),
            Some(&chara),
            "",
            ModelItemFilter::Body,
        );
        assert_eq!(result.total, 1);

        // 家具/chara 目录不可用（未加载/加载失败）时装备检索不受影响，对应过滤器为空。
        let result = search_model_items(&collection, None, None, "", ModelItemFilter::All);
        assert_eq!(result.total, 1);
        let result = search_model_items(&collection, None, None, "", ModelItemFilter::Furniture);
        assert_eq!(result.total, 0);
        let result = search_model_items(&collection, None, None, "", ModelItemFilter::Minions);
        assert_eq!(result.total, 0);
    }

    #[test]
    fn resolve_model_catalog_item_prefers_equipment_then_furniture_then_chara() {
        let collection = test_collection_catalog(vec![test_collection_item(4, 100)]);
        let furniture =
            test_furniture_catalog(vec![test_furniture_item(7, FurnitureModelKind::Indoor, 1)]);
        let chara = test_chara_catalog(vec![test_chara_item(
            11,
            CharaModelKind::Minion,
            8003,
            CharaModelType::Monster,
        )]);

        let resolved =
            resolve_model_catalog_item(Some(&collection), Some(&furniture), Some(&chara), 42);
        assert!(matches!(resolved, Some(ModelCatalogItem::Equipment(_))));

        let resolved =
            resolve_model_catalog_item(Some(&collection), Some(&furniture), Some(&chara), 7);
        assert!(matches!(resolved, Some(ModelCatalogItem::Furniture(_))));

        let resolved =
            resolve_model_catalog_item(Some(&collection), Some(&furniture), Some(&chara), 11);
        assert!(matches!(resolved, Some(ModelCatalogItem::Chara(_))));

        let resolved = resolve_model_catalog_item(None, None, Some(&chara), 11);
        assert!(matches!(resolved, Some(ModelCatalogItem::Chara(_))));

        assert!(
            resolve_model_catalog_item(Some(&collection), Some(&furniture), Some(&chara), 999)
                .is_none()
        );
    }

    #[test]
    fn glass_blend_mode_values_round_trip() {
        for mode in [ModelGlassBlendMode::Alpha, ModelGlassBlendMode::Additive] {
            assert_eq!(parse_glass_blend_mode(glass_blend_mode_value(mode)), mode);
        }
        assert_eq!(
            parse_glass_blend_mode("unknown"),
            ModelGlassBlendMode::Alpha
        );
    }

    #[test]
    fn secondary_vertex_debug_modes_round_trip() {
        for mode in [
            ModelDebugMode::VertexColor1,
            ModelDebugMode::SecondaryNormal,
            ModelDebugMode::Flow0,
            ModelDebugMode::Flow1,
            ModelDebugMode::UnsupportedInputs,
            ModelDebugMode::ViewDirection,
        ] {
            assert_eq!(parse_debug_mode(debug_mode_value(mode)), mode);
        }
    }

    #[test]
    fn shape_options_are_unique_sorted_and_change_the_instance_key() {
        let model = test_shape_model();

        assert_eq!(
            model_shape_options(&model),
            vec![(0, 1, "shape_a".to_string()), (2, 4, "shape_c".to_string()),]
        );
        assert!(model_has_shape_mask(&model, 1));
        assert!(model_has_shape_mask(&model, 4));
        assert!(!model_has_shape_mask(&model, 2));
        assert_ne!(
            model_instance_key(&model, None, 101, 0, false),
            model_instance_key(&model, Some(1), 101, 0, false)
        );
        // 种族维度在实例 key 内：装备按种族加载不同的模型文件。
        assert_ne!(
            model_instance_key(&model, None, 101, 0, false),
            model_instance_key(&model, None, 701, 0, false)
        );
    }

    #[test]
    fn stain_changes_keep_the_existing_instance_key() {
        let base = test_shape_model();
        let mut stained = base.clone();
        stained.stain_ids = [17, 93];

        assert_eq!(
            model_instance_key(&base, None, 101, 0, false),
            model_instance_key(&stained, None, 101, 0, false)
        );
    }

    #[test]
    fn orbit_reset_key_ignores_stain_but_tracks_item_and_race() {
        let model = test_shape_model();
        let mut stained = model.clone();
        stained.stain_ids = [17, 93];

        assert_eq!(
            model_orbit_reset_key(&model, 101),
            model_orbit_reset_key(&stained, 101)
        );
        assert_ne!(
            model_orbit_reset_key(&model, 101),
            model_orbit_reset_key(&model, 701)
        );
        let mut other = model.clone();
        other.item_id = 43;
        assert_ne!(
            model_orbit_reset_key(&model, 101),
            model_orbit_reset_key(&other, 101)
        );
    }

    #[test]
    fn attribute_mask_changes_the_instance_key_but_not_the_orbit_key() {
        let model = test_shape_model();

        assert_ne!(
            model_instance_key(&model, None, 101, 0, false),
            model_instance_key(&model, None, 101, 0x0000_0001, false)
        );
        // 部件变体切换保持视角：orbit key 不含 attribute mask 维度（按签名保证）。
        assert_eq!(
            model_orbit_reset_key(&model, 101),
            (42, model.model_main.raw, 0, 101)
        );
    }

    #[test]
    fn parts_only_changes_the_instance_key_but_not_the_orbit_key() {
        let model = test_shape_model();

        assert_ne!(
            model_instance_key(&model, None, 101, 0, false),
            model_instance_key(&model, None, 101, 0, true)
        );
        assert_eq!(
            model_orbit_reset_key(&model, 101),
            (42, model.model_main.raw, 0, 101)
        );
    }

    #[test]
    fn attribute_options_pass_through_and_detect_submeshes() {
        let mut model = test_shape_model();
        assert!(!model_has_attribute_submeshes(&model));
        assert!(model_attribute_options(&model).is_empty());

        model.meshes[0].submesh = Some(test_attribute_submesh(
            0x0000_0003,
            &["atr_bv_a", "atr_lod"],
        ));
        assert!(model_has_attribute_submeshes(&model));
        assert_eq!(
            model_attribute_options(&model),
            [
                ModelAttributeOption {
                    bit: 0x0000_0001,
                    names: vec!["atr_bv_a".to_string()],
                },
                ModelAttributeOption {
                    bit: 0x0000_0002,
                    names: vec!["atr_lod".to_string()],
                },
            ]
        );
        assert_eq!(
            attribute_option_label(&model_attribute_options(&model)[0]),
            "atr_bv_a"
        );
        assert_eq!(
            attribute_option_label(&ModelAttributeOption {
                bit: 0x0000_0004,
                names: Vec::new(),
            }),
            "Attribute 2"
        );
    }

    #[test]
    fn attribute_mask_toggle_and_item_scope_reset() {
        let mut mask = 0_u32;
        mask = toggle_attribute_mask(mask, 0x0000_0001, true);
        mask = toggle_attribute_mask(mask, 0x0000_0004, true);
        assert_eq!(mask, 0x0000_0005);
        assert_eq!(toggle_attribute_mask(mask, 0x0000_0001, false), 0x0000_0004);

        // 选择按物品 id 作用域：其他物品读回 0（全部关闭）。
        let selection = (Some(42), 0x0000_0005);
        assert_eq!(scoped_attribute_mask(selection, 42), 0x0000_0005);
        assert_eq!(scoped_attribute_mask(selection, 43), 0);
        assert_eq!(scoped_attribute_mask((None, 0x0000_0005), 42), 0);
    }

    #[test]
    fn parts_only_selection_resets_on_item_switch() {
        let selection = (Some(42), true);
        assert!(scoped_attribute_parts_only(selection, 42));
        assert!(!scoped_attribute_parts_only(selection, 43));
        assert!(!scoped_attribute_parts_only((None, true), 42));
    }

    fn test_collection_item(equip_slot_category: u32, model_main: u64) -> CollectionItem {
        CollectionItem {
            id: 42,
            kind: xiv_companion::CollectionKind::Equipment,
            name: "test item".to_string(),
            description: String::new(),
            icon: 0,
            item_ui_category: 0,
            item_search_category: 0,
            item_action: 0,
            equip_slot_category,
            slot_name: String::new(),
            slot_order: 0,
            level_item: 0,
            level_equip: 0,
            rarity: 0,
            class_job_category: 0,
            class_job_category_name: String::new(),
            item_series: 0,
            set_id: String::new(),
            set_name: String::new(),
            set_item_ids: Vec::new(),
            expansion: String::new(),
            patch: String::new(),
            model_main,
            model_sub: 0,
            appearance_key: String::new(),
        }
    }

    fn test_collection_catalog(items: Vec<CollectionItem>) -> CollectionCatalogPackage {
        CollectionCatalogPackage {
            schema_version: xiv_companion::COLLECTION_CATALOG_SCHEMA_VERSION,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: xiv_companion::CollectionCatalogCounts {
                items: items.len(),
                equipment: items.len(),
                ..Default::default()
            },
            items,
        }
    }

    fn test_furniture_item(
        id: u32,
        kind: FurnitureModelKind,
        model_key: u16,
    ) -> FurnitureCatalogItem {
        FurnitureCatalogItem {
            id,
            kind,
            name: format!("furniture {id}"),
            icon: 0,
            model_key,
        }
    }

    fn test_furniture_catalog(items: Vec<FurnitureCatalogItem>) -> FurnitureCatalogPackage {
        let indoor = items
            .iter()
            .filter(|item| item.kind == FurnitureModelKind::Indoor)
            .count();
        FurnitureCatalogPackage {
            schema_version: 1,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: xiv_companion::FurnitureCatalogCounts {
                items: items.len(),
                indoor,
                outdoor: items.len() - indoor,
                skipped_missing_items: 0,
            },
            items,
        }
    }

    fn test_chara_item(
        id: u32,
        kind: CharaModelKind,
        model_id: u16,
        chara_type: CharaModelType,
    ) -> CharaCatalogItem {
        CharaCatalogItem {
            id,
            kind,
            name: format!("chara {id}"),
            icon: 0,
            model: PackedCharaModelId {
                model_id,
                base_id: 1,
                variant_id: 1,
                chara_type,
            },
        }
    }

    fn test_chara_catalog(items: Vec<CharaCatalogItem>) -> CharaCatalogPackage {
        let minions = items
            .iter()
            .filter(|item| item.kind == CharaModelKind::Minion)
            .count();
        CharaCatalogPackage {
            schema_version: 1,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: xiv_companion::CharaCatalogCounts {
                items: items.len(),
                minions,
                mounts: items.len() - minions,
                skipped_empty_targets: 0,
                skipped_unsupported_models: 0,
                skipped_missing_items: 0,
            },
            items,
        }
    }

    fn test_shape_model() -> WeaponModelData {
        WeaponModelData {
            item_id: 42,
            item_name: "shape test".to_string(),
            model_main: PackedModelId::from_raw(1),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: xiv_companion::ModelBounds::default(),
            materials: Vec::new(),
            textures: Vec::new(),
            meshes: vec![xiv_companion::ModelMesh {
                path: "shape-test.mdl".to_string(),
                part_index: 0,
                mesh_category: Some("normal".to_string()),
                submesh: None,
                shape_influences: vec![
                    test_shape_info(2, "shape_c"),
                    test_shape_info(0, "shape_a"),
                    test_shape_info(0, "duplicate"),
                ],
                shape_targets: Vec::new(),
                material_index: 0,
                material_slot: 0,
                material_name: "shape test".to_string(),
                color: [1.0; 3],
                bone_table: None,
                vertices: Vec::new(),
                indices: Vec::new(),
            }],
        }
    }

    fn test_shape_info(index: usize, name: &str) -> xiv_companion::ModelShapeInfo {
        let shape_index_mask = 1_u32 << index;
        xiv_companion::ModelShapeInfo {
            index,
            name: Some(name.to_string()),
            shape_index_mask,
            shape_index_mask_hex: format!("0x{shape_index_mask:08X}"),
            shape_mesh_index: index,
            shape_value_count: 1,
        }
    }

    fn test_attribute_submesh(mask: u32, names: &[&str]) -> xiv_companion::ModelSubmeshInfo {
        xiv_companion::ModelSubmeshInfo {
            index: 0,
            table_index: 0,
            attribute_index_mask: mask,
            attribute_index_mask_hex: format!("0x{mask:08X}"),
            attribute_names: names.iter().map(|name| name.to_string()).collect(),
            bone_start_index: 0,
            bone_count: 0,
        }
    }
}
