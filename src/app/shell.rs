use dioxus::prelude::*;

use crate::app::icons::{Icon, IconKind};
use crate::app::modules::{APP_MODULES, ModuleGroup, module_group_label};
use crate::app::pages::{CraftingPage, NotesPage, WorkspacePage};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Workspace,
    Crafting,
    Notes,
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
            _ => Route::Workspace,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Route::Workspace => "/",
            Route::Crafting => "/crafting",
            Route::Notes => "/notes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Route::Workspace => "工作台",
            Route::Crafting => "合成检索",
            Route::Notes => "笔记",
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
) -> Element {
    let button_class = match (compact, collapsed, active) {
        (true, _, true) => {
            "flex h-10 min-w-36 items-center gap-3 rounded-md bg-accent px-3 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (true, _, false) => {
            "flex h-10 min-w-36 items-center gap-3 rounded-md px-3 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out"
        }
        (false, true, true) => {
            "flex h-10 items-center justify-center rounded-md bg-accent px-0 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (false, true, false) => {
            "flex h-10 items-center justify-center rounded-md px-0 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out"
        }
        (false, false, true) => {
            "flex h-9 items-center gap-3 rounded-md bg-accent px-3 text-sm font-medium text-foreground transition-all duration-300 ease-out"
        }
        (false, false, false) => {
            "flex h-9 items-center gap-3 rounded-md px-3 text-sm font-medium text-muted-foreground transition-all duration-300 ease-out"
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
        "min-h-screen bg-background text-foreground lg:grid lg:grid-cols-[72px_minmax(0,1fr)] lg:transition-[grid-template-columns] lg:duration-300 lg:ease-out"
    } else {
        "min-h-screen bg-background text-foreground lg:grid lg:grid-cols-[260px_minmax(0,1fr)] lg:transition-[grid-template-columns] lg:duration-300 lg:ease-out"
    };

    rsx! {
        div {
            class: shell_class,
            DesktopSidebar { current, collapsed }
            div { class: "flex min-w-0 flex-col",
                MobileHeader { current }
                main { class: "min-w-0 flex-1",
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
        aside { class: "hidden min-h-screen min-w-0 overflow-visible border-r bg-card transition-all duration-300 ease-out lg:flex lg:flex-col",
            div { class: brand_class,
                div { class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground",
                    Icon { kind: IconKind::LayoutDashboard, class: "h-4 w-4" }
                }
                div { class: brand_text_class,
                    div { class: "text-sm font-semibold", "XIV Companion" }
                    div { class: "text-xs text-muted-foreground", "Eorzea workspace" }
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
                    label: "工作台",
                    route: Route::Workspace,
                    active: current == Route::Workspace,
                    icon: IconKind::Home,
                    collapsed: collapsed(),
                }

                section { class: "mb-5 mt-4",
                    if !collapsed() {
                        div { class: "mb-2 px-3 text-xs font-medium text-muted-foreground",
                            "{module_group_label(ModuleGroup::Tools)}"
                        }
                    }
                    nav { class: "space-y-1", aria_label: module_group_label(ModuleGroup::Tools),
                        for module in APP_MODULES {
                            NavButton {
                                label: module.label,
                                route: Route::from_path(module.href),
                                active: current.path() == module.href,
                                icon: module_icon(module.id),
                                collapsed: collapsed(),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MobileHeader(current: Route) -> Element {
    let home_class = if current == Route::Workspace {
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
                    onclick: move |_| navigate(Route::Workspace),
                    Icon { kind: IconKind::Home, class: "h-4 w-4" }
                    "工作台"
                }
                for module in APP_MODULES {
                    div { class: "rounded-md border bg-card",
                        NavButton {
                            label: module.label,
                            route: Route::from_path(module.href),
                            active: current.path() == module.href,
                            icon: module_icon(module.id),
                            compact: true,
                        }
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
            Route::Workspace => rsx! { WorkspacePage {} },
            Route::Crafting => rsx! { CraftingPage {} },
            Route::Notes => rsx! { NotesPage {} },
        }
    }
}
