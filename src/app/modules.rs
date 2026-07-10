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
pub struct AppModule {
    pub id: &'static str,
    pub label: &'static str,
    pub href: &'static str,
    pub group: ModuleGroup,
    pub status: ModuleStatus,
}

pub const APP_MODULES: &[AppModule] = &[
    AppModule {
        id: "crafting",
        label: "合成检索",
        href: "/crafting",
        group: ModuleGroup::Tools,
        status: ModuleStatus::Ready,
    },
    AppModule {
        id: "notes",
        label: "笔记",
        href: "/notes",
        group: ModuleGroup::Tools,
        status: ModuleStatus::Ready,
    },
    AppModule {
        id: "weapon-models",
        label: "武器模型",
        href: "/weapon-models",
        group: ModuleGroup::Preview,
        status: ModuleStatus::Experimental,
    },
    AppModule {
        id: "collection",
        label: "图鉴",
        href: "/collection",
        group: ModuleGroup::Data,
        status: ModuleStatus::Experimental,
    },
];

pub fn module_group_label(group: ModuleGroup) -> &'static str {
    match group {
        ModuleGroup::Tools => "工具",
        ModuleGroup::Preview => "预览",
        ModuleGroup::Data => "数据",
    }
}
