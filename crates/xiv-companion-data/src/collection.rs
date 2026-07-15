use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const COLLECTION_CATALOG_SCHEMA_VERSION: u32 = 14;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCatalogPackage {
    pub schema_version: u32,
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: CollectionCatalogCounts,
    pub items: Vec<CollectionItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCatalogCounts {
    pub items: usize,
    pub equipment: usize,
    pub orchestrion_rolls: usize,
    pub mounts: usize,
    pub minions: usize,
    pub fashion_accessories: usize,
    pub emotes: usize,
    pub aesthetician_styles: usize,
    pub riding_maps: usize,
    pub mahjong_supports: usize,
    pub portrait_designs: usize,
    pub triple_triad_cards: usize,
    pub chocobo_bardings: usize,
    pub facewear: usize,
    pub master_recipes: usize,
    pub other_unlocks: usize,
    pub folklore_books: usize,
}

impl CollectionCatalogCounts {
    pub fn count_for(&self, kind: CollectionKind) -> usize {
        match kind {
            CollectionKind::Equipment => self.equipment,
            CollectionKind::OrchestrionRoll => self.orchestrion_rolls,
            CollectionKind::Mount => self.mounts,
            CollectionKind::Minion => self.minions,
            CollectionKind::FashionAccessory => self.fashion_accessories,
            CollectionKind::Emote => self.emotes,
            CollectionKind::AestheticianStyle => self.aesthetician_styles,
            CollectionKind::RidingMap => self.riding_maps,
            CollectionKind::MahjongSupport => self.mahjong_supports,
            CollectionKind::PortraitDesign => self.portrait_designs,
            CollectionKind::TripleTriadCard => self.triple_triad_cards,
            CollectionKind::ChocoboBarding => self.chocobo_bardings,
            CollectionKind::Facewear => self.facewear,
            CollectionKind::MasterRecipe => self.master_recipes,
            CollectionKind::OtherUnlock => self.other_unlocks,
            CollectionKind::FolkloreBook => self.folklore_books,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollectionKind {
    Equipment,
    OrchestrionRoll,
    Mount,
    Minion,
    FashionAccessory,
    Emote,
    AestheticianStyle,
    RidingMap,
    MahjongSupport,
    PortraitDesign,
    TripleTriadCard,
    ChocoboBarding,
    Facewear,
    MasterRecipe,
    OtherUnlock,
    FolkloreBook,
}

impl CollectionKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Equipment => "equipment",
            Self::OrchestrionRoll => "orchestrion-roll",
            Self::Mount => "mount",
            Self::Minion => "minion",
            Self::FashionAccessory => "fashion-accessory",
            Self::Emote => "emote",
            Self::AestheticianStyle => "aesthetician-style",
            Self::RidingMap => "riding-map",
            Self::MahjongSupport => "mahjong-support",
            Self::PortraitDesign => "portrait-design",
            Self::TripleTriadCard => "triple-triad-card",
            Self::ChocoboBarding => "chocobo-barding",
            Self::Facewear => "facewear",
            Self::MasterRecipe => "master-recipe",
            Self::OtherUnlock => "other-unlock",
            Self::FolkloreBook => "folklore-book",
        }
    }

    pub fn label(self) -> &'static str {
        crate::category_definition(self).label
    }
}

impl fmt::Display for CollectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for CollectionKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        crate::COLLECTION_CATEGORIES
            .iter()
            .map(|definition| definition.kind)
            .find(|kind| kind.id() == value)
            .ok_or_else(|| format!("unknown collection kind: {value}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItem {
    pub id: u32,
    pub kind: CollectionKind,
    pub name: String,
    pub description: String,
    pub icon: u32,
    pub item_ui_category: u32,
    pub item_search_category: u32,
    pub item_action: u32,
    pub equip_slot_category: u32,
    pub slot_name: String,
    pub slot_order: u8,
    pub level_item: u16,
    pub level_equip: u16,
    pub rarity: u8,
    pub class_job_category: u32,
    pub class_job_category_name: String,
    pub item_series: u32,
    pub set_id: String,
    pub set_name: String,
    pub expansion: String,
    pub patch: String,
    pub model_main: u64,
    pub model_sub: u64,
    pub appearance_key: String,
}

impl CollectionItem {
    pub fn is_equipment(&self) -> bool {
        self.kind == CollectionKind::Equipment
    }
}
