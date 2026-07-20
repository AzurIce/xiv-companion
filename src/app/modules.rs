#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ModuleGroup {
    Tools,
    Preview,
    Data,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ModuleStatus {
    Ready,
    Planned,
    Experimental,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppCapability {
    Web,
    LocalGameData,
    ApiBridge,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRequirement {
    BuiltIn,
    Optional,
    Required,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ModuleCapability {
    pub capability: AppCapability,
    pub requirement: CapabilityRequirement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AppModule {
    pub id: &'static str,
    pub label: &'static str,
    pub href: &'static str,
    pub group: ModuleGroup,
    pub status: ModuleStatus,
    pub capabilities: &'static [ModuleCapability],
}

const WEB_ONLY: &[ModuleCapability] = &[ModuleCapability {
    capability: AppCapability::Web,
    requirement: CapabilityRequirement::BuiltIn,
}];

const WEB_WITH_LOCAL_DATA: &[ModuleCapability] = &[
    ModuleCapability {
        capability: AppCapability::Web,
        requirement: CapabilityRequirement::BuiltIn,
    },
    ModuleCapability {
        capability: AppCapability::LocalGameData,
        requirement: CapabilityRequirement::Optional,
    },
];

const LOCAL_DATA_REQUIRED: &[ModuleCapability] = &[ModuleCapability {
    capability: AppCapability::LocalGameData,
    requirement: CapabilityRequirement::Required,
}];

const API_BRIDGE_REQUIRED: &[ModuleCapability] = &[ModuleCapability {
    capability: AppCapability::ApiBridge,
    requirement: CapabilityRequirement::Required,
}];

const WEB_WITH_API_BRIDGE: &[ModuleCapability] = &[
    ModuleCapability {
        capability: AppCapability::Web,
        requirement: CapabilityRequirement::BuiltIn,
    },
    ModuleCapability {
        capability: AppCapability::ApiBridge,
        requirement: CapabilityRequirement::Optional,
    },
];

pub const APP_MODULES: &[AppModule] = &[
    AppModule {
        id: "crafting",
        label: "合成检索",
        href: "/crafting",
        group: ModuleGroup::Tools,
        status: ModuleStatus::Ready,
        capabilities: WEB_WITH_LOCAL_DATA,
    },
    AppModule {
        id: "notes",
        label: "笔记",
        href: "/notes",
        group: ModuleGroup::Tools,
        status: ModuleStatus::Ready,
        capabilities: WEB_ONLY,
    },
    AppModule {
        id: "weapon-models",
        label: "武器模型",
        href: "/weapon-models",
        group: ModuleGroup::Preview,
        status: ModuleStatus::Experimental,
        capabilities: LOCAL_DATA_REQUIRED,
    },
    AppModule {
        id: "inventory",
        label: "物品",
        href: "/inventory",
        group: ModuleGroup::Data,
        status: ModuleStatus::Ready,
        capabilities: API_BRIDGE_REQUIRED,
    },
    AppModule {
        id: "collection",
        label: "图鉴",
        href: "/collection",
        group: ModuleGroup::Data,
        status: ModuleStatus::Ready,
        capabilities: WEB_WITH_API_BRIDGE,
    },
];

pub fn module_group_label(group: ModuleGroup) -> &'static str {
    match group {
        ModuleGroup::Tools => "工具",
        ModuleGroup::Preview => "预览",
        ModuleGroup::Data => "数据",
    }
}

#[component]
pub fn ModuleCapabilityBadges(module_id: &'static str) -> Element {
    let Some(module) = APP_MODULES.iter().find(|module| module.id == module_id) else {
        return rsx! {};
    };

    rsx! {
        div { class: "flex flex-wrap items-center gap-1.5",
            for capability in module.capabilities {
                Badge {
                    variant: capability_badge_variant(*capability),
                    title: capability_description(*capability),
                    {capability_label(*capability)}
                }
            }
        }
    }
}

fn capability_label(capability: ModuleCapability) -> &'static str {
    match (capability.capability, capability.requirement) {
        (AppCapability::Web, CapabilityRequirement::BuiltIn) => "Web 可用",
        (AppCapability::LocalGameData, CapabilityRequirement::Optional) => "本地数据增强",
        (AppCapability::LocalGameData, CapabilityRequirement::Required) => "需要本地数据",
        (AppCapability::ApiBridge, CapabilityRequirement::Optional) => "API Bridge 增强",
        (AppCapability::ApiBridge, CapabilityRequirement::Required) => "需要 API Bridge",
        _ => "扩展能力",
    }
}

fn capability_description(capability: ModuleCapability) -> &'static str {
    match (capability.capability, capability.requirement) {
        (AppCapability::Web, CapabilityRequirement::BuiltIn) => {
            "无需安装扩展，打开 Web 页面即可使用"
        }
        (AppCapability::LocalGameData, CapabilityRequirement::Optional) => {
            "授权本地游戏目录后可刷新或补充数据"
        }
        (AppCapability::LocalGameData, CapabilityRequirement::Required) => {
            "此功能需要浏览器读取本地 FFXIV 游戏数据"
        }
        (AppCapability::ApiBridge, CapabilityRequirement::Optional) => {
            "连接 API Bridge 后可读取当前游戏客户端状态"
        }
        (AppCapability::ApiBridge, CapabilityRequirement::Required) => {
            "此功能需要 Dalamud 的 API Bridge 插件提供当前角色数据"
        }
        _ => "此功能使用额外运行能力",
    }
}

fn capability_badge_variant(capability: ModuleCapability) -> BadgeVariant {
    match capability.requirement {
        CapabilityRequirement::BuiltIn => BadgeVariant::Success,
        CapabilityRequirement::Optional => BadgeVariant::Outline,
        CapabilityRequirement::Required => BadgeVariant::Warning,
    }
}
use dioxus::prelude::*;

use crate::app::ui::{Badge, BadgeVariant};
