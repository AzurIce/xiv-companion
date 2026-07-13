use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const COLLECTION_CATALOG_SCHEMA_VERSION: u32 = 8;

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
    FolkloreBook,
}

impl CollectionKind {
    pub const ALL: [Self; 7] = [
        Self::Equipment,
        Self::OrchestrionRoll,
        Self::Mount,
        Self::Minion,
        Self::FashionAccessory,
        Self::Emote,
        Self::FolkloreBook,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Equipment => "equipment",
            Self::OrchestrionRoll => "orchestrion-roll",
            Self::Mount => "mount",
            Self::Minion => "minion",
            Self::FashionAccessory => "fashion-accessory",
            Self::Emote => "emote",
            Self::FolkloreBook => "folklore-book",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Equipment => "装备",
            Self::OrchestrionRoll => "乐谱",
            Self::Mount => "坐骑",
            Self::Minion => "宠物",
            Self::FashionAccessory => "时尚配饰",
            Self::Emote => "情感动作",
            Self::FolkloreBook => "传习录",
        }
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
        Self::ALL
            .into_iter()
            .find(|kind| kind.id() == value)
            .ok_or_else(|| format!("unknown collection kind: {value}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionEntryKey {
    pub kind: CollectionKind,
    pub row_id: u32,
}

impl CollectionEntryKey {
    pub fn new(kind: CollectionKind, row_id: u32) -> Self {
        Self { kind, row_id }
    }

    pub fn storage_key(&self) -> String {
        format!("{}:{}", self.kind.id(), self.row_id)
    }
}

impl FromStr for CollectionEntryKey {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (kind, row_id) = value
            .split_once(':')
            .ok_or_else(|| format!("invalid collection entry key: {value}"))?;
        Ok(Self {
            kind: kind.parse()?,
            row_id: row_id
                .parse()
                .map_err(|error| format!("invalid collection row id {row_id}: {error}"))?,
        })
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
    pub fn key(&self) -> CollectionEntryKey {
        CollectionEntryKey::new(self.kind, self.id)
    }

    pub fn is_equipment(&self) -> bool {
        self.kind == CollectionKind::Equipment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_entry_keys_round_trip() {
        let key = CollectionEntryKey::new(CollectionKind::FashionAccessory, 33041);
        assert_eq!(key.storage_key().parse::<CollectionEntryKey>(), Ok(key));
    }
}
