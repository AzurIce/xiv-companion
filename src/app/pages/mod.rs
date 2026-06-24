pub mod crafting;
pub mod notes;

use dioxus::prelude::*;

use crate::app::data::load_craft_data;
use crate::app::icons::{Icon, IconKind};
use crate::app::modules::APP_MODULES;
use crate::app::ui::{Card, CardContent, CardHeader, CardTitle};
use crate::app::utils::{cx, format_integer};

pub use crafting::CraftingPage;
pub use notes::NotesPage;

fn format_data_time(value: &str) -> String {
    value.to_string()
}

#[component]
pub fn WorkspacePage() -> Element {
    let craft_data = use_resource(load_craft_data);

    rsx! {
        div { class: "mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8",
            div { class: "flex flex-col gap-2",
                div { class: "text-sm text-muted-foreground", "工作台" }
                h1 { class: "text-2xl font-semibold", "XIV Companion" }
            }

            section { class: "rounded-md border bg-muted/30 px-3 py-2.5",
                match craft_data.read().as_ref() {
                    Some(Ok(data)) => rsx! {
                        div { class: "flex flex-wrap items-center gap-x-4 gap-y-2 text-sm",
                            div { class: "flex items-center gap-2 font-medium",
                                Icon { kind: IconKind::Database, class: "h-4 w-4 text-muted-foreground" }
                                "数据"
                            }
                            div { class: "min-w-0",
                                span { class: "text-muted-foreground", "游戏版本 " }
                                span { class: "font-medium", "{data.game_version}" }
                            }
                            div {
                                span { class: "text-muted-foreground", "生成 " }
                                span { class: "font-medium", "{format_data_time(&data.generated_at)}" }
                            }
                            div {
                                span { class: "text-muted-foreground", "物品 " }
                                span { class: "font-medium", "{format_integer(data.counts.items as f64)}" }
                            }
                            div {
                                span { class: "text-muted-foreground", "配方 " }
                                span { class: "font-medium", "{format_integer(data.counts.recipes as f64)}" }
                            }
                            div {
                                span { class: "text-muted-foreground", "来源 " }
                                span { class: "font-medium", "{format_integer(data.counts.sources as f64)}" }
                            }
                        }
                    },
                    Some(Err(error)) => rsx! { div { class: "text-sm text-destructive", "{error}" } },
                    None => rsx! { div { class: "h-8 rounded bg-muted" } },
                }
            }

            section { class: "space-y-3",
                div {
                    div { class: "text-sm font-medium", "工具" }
                    div { class: "mt-1 text-sm text-muted-foreground", "当前可用的工作区" }
                }

                div { class: "grid gap-4 md:grid-cols-2 xl:grid-cols-4",
                    for module in APP_MODULES {
                        a { href: format!("#{}", module.href), class: "block",
                            Card { class: cx(["h-full transition-colors hover:border-foreground/20"]),
                                CardHeader {
                                    div { class: "flex h-10 w-10 items-center justify-center rounded-lg border bg-background text-muted-foreground",
                                        Icon {
                                            kind: if module.id == "notes" { IconKind::BookOpen } else { IconKind::Wrench },
                                            class: "h-5 w-5"
                                        }
                                    }
                                }
                                CardContent { class: "space-y-2".to_string(),
                                    CardTitle { "{module.label}" }
                                    div { class: "text-sm text-muted-foreground",
                                        if module.id == "crafting" {
                                            "配方树、素材汇总、来源选择"
                                        } else {
                                            "目录页面、分栏卡片、材料汇总"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
