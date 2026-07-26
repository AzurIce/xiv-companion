use dioxus::prelude::*;

use crate::app::icons::{Icon, IconKind};
use crate::app::modules::{APP_MODULES, ModuleGroup, ModuleStatus, module_group_label};
use crate::app::pages::{
    CollectionPage, CraftingPage, HomePage, InventoryPage, NotesPage, SettingsPage,
    WeaponModelsPage,
};
use crate::app::ui::{Badge, BadgeVariant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    Crafting,
    Notes,
    WeaponModels,
    Inventory,
    Collection,
    Settings,
}

impl Route {
    pub fn from_hash() -> Self {
        let hash = web_sys::window()
            .and_then(|window| window.location().hash().ok())
            .unwrap_or_default();
        Self::from_path(
            hash.trim_start_matches('#')
                .split('?')
                .next()
                .unwrap_or("/"),
        )
    }

    pub fn from_path(path: &str) -> Self {
        match path {
            "/crafting" => Route::Crafting,
            "/notes" => Route::Notes,
            "/weapon-models" => Route::WeaponModels,
            "/inventory" => Route::Inventory,
            "/collection" => Route::Collection,
            "/settings" => Route::Settings,
            _ => Route::Home,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Route::Home => "/",
            Route::Crafting => "/crafting",
            Route::Notes => "/notes",
            Route::WeaponModels => "/weapon-models",
            Route::Inventory => "/inventory",
            Route::Collection => "/collection",
            Route::Settings => "/settings",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Route::Home => "首页",
            Route::Crafting => "合成检索",
            Route::Notes => "制作清单",
            Route::WeaponModels => "武器模型",
            Route::Inventory => "物品",
            Route::Collection => "图鉴",
            Route::Settings => "设置",
        }
    }
}

fn local_storage_value(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

fn set_local_storage_value(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

fn navigate(route: Route) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(route.path());
    }
}

fn module_icon(id: &str) -> IconKind {
    match id {
        "notes" => IconKind::BookOpen,
        "weapon-models" => IconKind::Sword,
        "inventory" => IconKind::PackageSearch,
        "collection" => IconKind::BookOpen,
        "crafting" => IconKind::Wrench,
        _ => IconKind::Wrench,
    }
}

#[component]
fn IconTooltip(
    label: &'static str,
    #[props(default = true)] enabled: bool,
    #[props(default = "relative".to_string())] class: String,
    children: Element,
) -> Element {
    let wrapper_class = if class == "absolute -right-3 top-1/2 z-20 -translate-y-1/2" {
        "group absolute -right-3 top-1/2 z-20 -translate-y-1/2"
    } else {
        "group relative"
    };

    rsx! {
        div { class: wrapper_class,
            {children}
            if enabled {
                div { class: "pointer-events-none absolute left-full top-1/2 z-50 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border bg-popover px-2 py-1 text-xs text-popover-foreground shadow-md group-hover:block",
                    "{label}"
                }
            }
        }
    }
}

#[component]
fn NavButton(
    label: &'static str,
    route: Route,
    active: bool,
    icon: IconKind,
    #[props(default = false)] compact: bool,
    #[props(default = false)] collapsed: bool,
    #[props(default = false)] experimental: bool,
) -> Element {
    let button_class = match (compact, collapsed, active) {
        (true, _, true) => {
            "flex h-10 min-w-36 items-center gap-3 rounded-md bg-accent px-3 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (true, _, false) => {
            "flex h-10 min-w-36 items-center gap-3 rounded-md px-3 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out hover:bg-accent hover:text-foreground"
        }
        (false, true, true) => {
            "flex h-10 w-full items-center justify-center rounded-md bg-accent px-0 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (false, true, false) => {
            "flex h-10 w-full items-center justify-center rounded-md px-0 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out hover:bg-accent hover:text-foreground"
        }
        (false, false, true) => {
            "flex h-9 w-full items-center gap-3 rounded-md bg-accent px-3 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (false, false, false) => {
            "flex h-9 w-full items-center gap-3 rounded-md px-3 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out hover:bg-accent hover:text-foreground"
        }
    };
    let label_class = if collapsed {
        "min-w-0 truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out max-w-0 -translate-x-1 opacity-0"
    } else {
        "min-w-0 truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out max-w-40 translate-x-0 opacity-100"
    };

    let link = rsx! {
        button {
            r#type: "button",
            class: button_class,
            title: if collapsed { label } else { "" },
            onclick: move |_| navigate(route),
            Icon { kind: icon, class: "h-4 w-4" }
            span {
                class: label_class,
                "{label}"
            }
            if experimental && !collapsed {
                Badge { variant: BadgeVariant::Warning, "实验" }
            }
        }
    };

    if compact {
        link
    } else {
        rsx! {
            IconTooltip { label, enabled: collapsed, {link} }
        }
    }
}

#[component]
pub fn AppShell(route: Signal<Route>) -> Element {
    let collapsed =
        use_signal(|| local_storage_value("xiv-companion-sidebar").as_deref() == Some("collapsed"));

    use_effect(move || {
        set_local_storage_value(
            "xiv-companion-sidebar",
            if collapsed() { "collapsed" } else { "expanded" },
        );
    });

    let current = route();
    let shell_class = if collapsed() {
        "grid h-dvh min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden bg-background text-foreground lg:grid-cols-[72px_minmax(0,1fr)] lg:transition-[grid-template-columns] lg:duration-300 lg:ease-out"
    } else {
        "grid h-dvh min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden bg-background text-foreground lg:grid-cols-[240px_minmax(0,1fr)] lg:transition-[grid-template-columns] lg:duration-300 lg:ease-out"
    };

    rsx! {
        div {
            class: shell_class,
            DesktopSidebar { current, collapsed }
            div { class: "flex min-h-0 min-w-0 flex-col overflow-hidden",
                MobileHeader { current }
                main { class: "min-h-0 min-w-0 flex-1 overflow-y-auto",
                    PageContent { current }
                }
            }
        }
    }
}

#[component]
fn DesktopSidebar(current: Route, collapsed: Signal<bool>) -> Element {
    let brand_class = if collapsed() {
        "relative flex h-16 items-center justify-center border-b px-0 transition-all duration-300 ease-out"
    } else {
        "relative flex h-16 items-center gap-3 border-b px-3 pr-8 transition-all duration-300 ease-out"
    };
    let brand_text_class = if collapsed() {
        "min-w-0 overflow-hidden whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out max-w-0 -translate-x-1 opacity-0"
    } else {
        "min-w-0 overflow-hidden whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out max-w-40 translate-x-0 opacity-100"
    };

    rsx! {
        aside { class: "relative z-50 hidden h-dvh min-h-0 min-w-0 overflow-visible border-r bg-card transition-all duration-300 ease-out lg:flex lg:flex-col",
            div { class: brand_class,
                div { class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground",
                    Icon { kind: IconKind::LayoutDashboard, class: "h-4 w-4" }
                }
                div { class: brand_text_class,
                    div { class: "text-sm font-semibold", "XIV Companion" }
                    div { class: "text-xs text-muted-foreground", "Eorzea toolkit" }
                }
                button {
                    r#type: "button",
                    class: "absolute -right-3 top-1/2 z-20 flex h-7 w-7 -translate-y-1/2 shrink-0 items-center justify-center rounded-full border bg-card text-muted-foreground shadow-sm transition-colors duration-200 hover:border-foreground/20 hover:bg-accent hover:text-foreground",
                    aria_label: if collapsed() { "展开侧边栏" } else { "折叠侧边栏" },
                    title: if collapsed() { "展开侧边栏" } else { "折叠侧边栏" },
                    onclick: move |_| collapsed.set(!collapsed()),
                    Icon {
                        kind: if collapsed() { IconKind::PanelLeftOpen } else { IconKind::PanelLeftClose },
                        class: "h-3.5 w-3.5"
                    }
                }
            }

            div { class: "flex-1 overflow-y-auto px-3 py-4",
                NavButton {
                    label: "首页",
                    route: Route::Home,
                    active: current == Route::Home,
                    icon: IconKind::Home,
                    collapsed: collapsed(),
                }

                for group in [ModuleGroup::Tools, ModuleGroup::Preview, ModuleGroup::Data] {
                    section { class: "mb-5 mt-4",
                        if !collapsed() {
                            div { class: "mb-2 px-3 text-xs font-medium text-muted-foreground",
                                "{module_group_label(group)}"
                            }
                        }
                        nav { class: "space-y-1", aria_label: module_group_label(group),
                            for module in APP_MODULES.iter().filter(move |module| module.group == group) {
                                NavButton {
                                    label: module.label,
                                    route: Route::from_path(module.href),
                                    active: current.path() == module.href,
                                    icon: module_icon(module.id),
                                    collapsed: collapsed(),
                                    experimental: module.status == ModuleStatus::Experimental,
                                }
                            }
                        }
                    }
                }
            }
            div { class: "border-t p-3",
                div { class: "flex items-center gap-1.5",
                    div { class: "min-w-0 flex-1",
                        NavButton {
                            label: "设置",
                            route: Route::Settings,
                            active: current == Route::Settings,
                            icon: IconKind::Settings,
                            collapsed: collapsed(),
                        }
                    }
                    a {
                        href: "https://github.com/AzurIce/xiv-companion",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        title: "GitHub 仓库",
                        aria_label: "GitHub 仓库",
                        class: "flex h-9 w-9 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
                        Icon { kind: IconKind::Github, class: "h-4 w-4" }
                    }
                }
            }
        }
    }
}

#[component]
fn MobileHeader(current: Route) -> Element {
    let home_class = if current == Route::Home {
        "flex h-10 min-w-28 items-center gap-2 rounded-md border border-foreground/20 bg-card px-3 text-sm font-medium text-foreground"
    } else {
        "flex h-10 min-w-28 items-center gap-2 rounded-md border bg-card px-3 text-sm font-medium text-muted-foreground"
    };

    rsx! {
        header { class: "sticky top-0 z-40 border-b bg-background/95 backdrop-blur lg:hidden",
            div { class: "flex h-14 items-center gap-3 px-4",
                div { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground",
                    Icon { kind: IconKind::LayoutDashboard, class: "h-4 w-4" }
                }
                div { class: "min-w-0 flex-1",
                    div { class: "text-sm font-semibold", "XIV Companion" }
                    div { class: "truncate text-xs text-muted-foreground", "{current.label()}" }
                }
            }
            nav { class: "flex gap-2 overflow-x-auto px-4 pb-3", aria_label: "模块",
                button {
                    r#type: "button",
                    class: home_class,
                    onclick: move |_| navigate(Route::Home),
                    Icon { kind: IconKind::Home, class: "h-4 w-4" }
                    "首页"
                }
                for module in APP_MODULES {
                    div { class: "rounded-md border bg-card",
                        NavButton {
                            label: module.label,
                            route: Route::from_path(module.href),
                            active: current.path() == module.href,
                            icon: module_icon(module.id),
                            compact: true,
                            experimental: module.status == ModuleStatus::Experimental,
                        }
                    }
                }
                div { class: "rounded-md border bg-card",
                    NavButton {
                        label: "设置",
                        route: Route::Settings,
                        active: current == Route::Settings,
                        icon: IconKind::Settings,
                        compact: true,
                    }
                }
            }
        }
    }
}

#[component]
fn PageContent(current: Route) -> Element {
    rsx! {
        match current {
            Route::Home => rsx! { HomePage {} },
            Route::Crafting => rsx! { CraftingPage {} },
            Route::Notes => rsx! { NotesPage {} },
            Route::WeaponModels => rsx! { WeaponModelsPage {} },
            Route::Inventory => rsx! { InventoryPage {} },
            Route::Collection => rsx! { CollectionPage {} },
            Route::Settings => rsx! { SettingsPage {} },
        }
    }
}
