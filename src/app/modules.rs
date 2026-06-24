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
];

pub fn module_group_label(group: ModuleGroup) -> &'static str {
    match group {
        ModuleGroup::Tools => "工具",
        ModuleGroup::Preview => "预览",
        ModuleGroup::Data => "数据",
    }
}
