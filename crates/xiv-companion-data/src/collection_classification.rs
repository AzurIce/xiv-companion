use std::collections::BTreeMap;

use crate::CollectionKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectionCategoryDefinition {
    pub kind: CollectionKind,
    pub label: &'static str,
    pub experimental: bool,
    pub group_by_patch: bool,
}

pub const COLLECTION_CATEGORIES: [CollectionCategoryDefinition; 16] = [
    category(CollectionKind::Equipment, "装备", false, false),
    category(CollectionKind::Mount, "坐骑", false, true),
    category(CollectionKind::Minion, "宠物", false, true),
    category(CollectionKind::OrchestrionRoll, "乐谱", false, true),
    category(CollectionKind::Emote, "情感动作", false, true),
    category(CollectionKind::AestheticianStyle, "发型与面妆", false, true),
    category(CollectionKind::FashionAccessory, "时尚配饰", false, true),
    category(CollectionKind::Facewear, "面部配饰", false, true),
    category(CollectionKind::MasterRecipe, "生产秘籍", false, true),
    category(CollectionKind::FolkloreBook, "传习录", false, true),
    category(CollectionKind::RidingMap, "详细地图", false, true),
    category(CollectionKind::PortraitDesign, "肖像教材", false, true),
    category(CollectionKind::ChocoboBarding, "鸟甲", false, true),
    category(CollectionKind::TripleTriadCard, "九宫幻卡", false, true),
    category(CollectionKind::MahjongSupport, "方城声援", false, true),
    category(CollectionKind::OtherUnlock, "其他解锁", false, true),
];

const fn category(
    kind: CollectionKind,
    label: &'static str,
    experimental: bool,
    group_by_patch: bool,
) -> CollectionCategoryDefinition {
    CollectionCategoryDefinition {
        kind,
        label,
        experimental,
        group_by_patch,
    }
}

pub fn category_definition(kind: CollectionKind) -> &'static CollectionCategoryDefinition {
    COLLECTION_CATEGORIES
        .iter()
        .find(|definition| definition.kind == kind)
        .expect("every CollectionKind must have a category definition")
}

#[derive(Clone, Copy, Debug)]
pub struct CollectionClassificationInput<'a> {
    pub name: &'a str,
    pub equip_slot_category: u32,
    pub item_action_type: u32,
    pub item_ui_category: u32,
}

/// Classifies permanent collection candidates. Unlock candidates deliberately start in
/// `OtherUnlock`; ordered rules below may claim them for a more specific category.
pub fn classify_collection_item(
    input: CollectionClassificationInput<'_>,
) -> Option<CollectionKind> {
    if input.equip_slot_category != 0 {
        return Some(CollectionKind::Equipment);
    }
    if !is_permanent_unlock_candidate(input.item_action_type, input.item_ui_category) {
        return None;
    }

    let mut kind = CollectionKind::OtherUnlock;
    for rule in CLASSIFICATION_RULES {
        if rule.matcher.matches(input) {
            kind = rule.kind;
            break;
        }
    }
    Some(kind)
}

pub fn is_permanent_unlock_candidate(action_type: u32, item_ui_category: u32) -> bool {
    PERMANENT_UNLOCK_ACTION_TYPES.contains(&action_type) || matches!(item_ui_category, 81 | 94)
}

pub const PERMANENT_UNLOCK_ACTION_TYPES: &[u32] = &[
    853, 1_013, 1_322, 2_136, 2_633, 3_357, 4_107, 18_083, 19_743, 20_086, 25_183, 29_459, 37_312,
    43_141, 43_142,
];

struct ClassificationRule {
    kind: CollectionKind,
    matcher: RuleMatcher,
}

enum RuleMatcher {
    Action(u32),
    GenericUnlockName(fn(&str) -> bool),
    UiCategory(u32),
}

impl RuleMatcher {
    fn matches(&self, input: CollectionClassificationInput<'_>) -> bool {
        match self {
            Self::Action(action_type) => input.item_action_type == *action_type,
            Self::GenericUnlockName(predicate) => {
                input.item_action_type == 2_633 && predicate(input.name)
            }
            Self::UiCategory(category) => input.item_ui_category == *category,
        }
    }
}

const CLASSIFICATION_RULES: &[ClassificationRule] = &[
    action(CollectionKind::OrchestrionRoll, 25_183),
    action(CollectionKind::Mount, 1_322),
    action(CollectionKind::Minion, 853),
    action(CollectionKind::FashionAccessory, 20_086),
    named_unlock(CollectionKind::Emote, |name| name.starts_with("演技教材")),
    named_unlock(CollectionKind::AestheticianStyle, |name| {
        name.starts_with("发型样式") || name.starts_with("面妆样式")
    }),
    named_unlock(CollectionKind::RidingMap, |name| name.ends_with("详细地图")),
    named_unlock(CollectionKind::MahjongSupport, |name| {
        name.starts_with("方城金句集")
    }),
    named_unlock(CollectionKind::PortraitDesign, |name| {
        name.starts_with("肖像教材")
    }),
    action(CollectionKind::PortraitDesign, 29_459),
    action(CollectionKind::ChocoboBarding, 1_013),
    action(CollectionKind::MasterRecipe, 2_136),
    action(CollectionKind::TripleTriadCard, 3_357),
    action(CollectionKind::Facewear, 37_312),
    action(CollectionKind::FolkloreBook, 4_107),
    ui_category(CollectionKind::OrchestrionRoll, 94),
    ui_category(CollectionKind::Minion, 81),
];

const fn action(kind: CollectionKind, action_type: u32) -> ClassificationRule {
    ClassificationRule {
        kind,
        matcher: RuleMatcher::Action(action_type),
    }
}

const fn named_unlock(kind: CollectionKind, predicate: fn(&str) -> bool) -> ClassificationRule {
    ClassificationRule {
        kind,
        matcher: RuleMatcher::GenericUnlockName(predicate),
    }
}

const fn ui_category(kind: CollectionKind, ui_category: u32) -> ClassificationRule {
    ClassificationRule {
        kind,
        matcher: RuleMatcher::UiCategory(ui_category),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollectionClassificationAudit {
    pub candidate_count: usize,
    pub counts_by_kind: BTreeMap<CollectionKind, usize>,
    pub other_unlocks_by_action_type: BTreeMap<u32, usize>,
}

impl CollectionClassificationAudit {
    pub fn record(&mut self, kind: CollectionKind, action_type: u32) {
        self.candidate_count += 1;
        *self.counts_by_kind.entry(kind).or_default() += 1;
        if kind == CollectionKind::OtherUnlock {
            *self
                .other_unlocks_by_action_type
                .entry(action_type)
                .or_default() += 1;
        }
    }

    pub fn is_conserved(&self) -> bool {
        self.counts_by_kind.values().sum::<usize>() == self.candidate_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(name: &str, action: u32) -> Option<CollectionKind> {
        classify_collection_item(CollectionClassificationInput {
            name,
            equip_slot_category: 0,
            item_action_type: action,
            item_ui_category: 0,
        })
    }

    #[test]
    fn generic_unlock_rules_are_first_match_wins() {
        assert_eq!(
            classify("演技教材：挥手", 2_633),
            Some(CollectionKind::Emote)
        );
        assert_eq!(
            classify("面妆样式：星尘", 2_633),
            Some(CollectionKind::AestheticianStyle)
        );
        assert_eq!(
            classify("某地详细地图", 2_633),
            Some(CollectionKind::RidingMap)
        );
        assert_eq!(
            classify("未知的新解锁", 2_633),
            Some(CollectionKind::OtherUnlock)
        );
    }

    #[test]
    fn unknown_consumables_do_not_enter_other_unlock() {
        assert_eq!(classify("普通消耗品", 4_647), None);
        assert_eq!(classify("没有动作的物品", 0), None);
    }

    #[test]
    fn every_kind_has_exactly_one_definition() {
        assert_eq!(COLLECTION_CATEGORIES.len(), 16);
        for kind in COLLECTION_CATEGORIES.iter().map(|entry| entry.kind) {
            assert_eq!(
                COLLECTION_CATEGORIES
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .count(),
                1
            );
        }
    }
}
