use dioxus::prelude::*;

use crate::app::collection_bridge::{load_bridge_url, mark_bridge_verified, save_bridge_url};
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{Button, ButtonVariant, input_class};

use super::settings_resources::ResourceSettingsSection;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsSection {
    #[default]
    Connection,
    Data,
}

#[component]
pub fn SettingsPage() -> Element {
    let mut bridge_url = use_signal(load_bridge_url);
    let mut persisted_bridge_url = use_signal(load_bridge_url);
    let mut saved = use_signal(|| false);
    let mut show_bridge_url = use_signal(|| false);
    let mut active_section = use_signal(initial_settings_section);

    let url_snapshot = bridge_url();
    let bridge_settings_dirty = bridge_url().trim() != persisted_bridge_url().trim();
    let active_section_snapshot = active_section();

    rsx! {
        div { class: "min-h-screen bg-background px-4 py-4 sm:px-5 lg:px-6",
            div { class: "mx-auto max-w-6xl space-y-4",
                header { class: "border-b pb-2",
                    div { class: "text-xs text-muted-foreground", "系统" }
                    h1 { class: "text-xl font-semibold leading-tight", "设置" }
                }

                nav { class: "flex gap-1 border-b", aria_label: "设置分类",
                    SettingsTab {
                        label: "连接",
                        icon: IconKind::PlugZap,
                        active: active_section_snapshot == SettingsSection::Connection,
                        onclick: move |_| {
                            active_section.set(SettingsSection::Connection);
                            write_settings_section(SettingsSection::Connection);
                        },
                    }
                    SettingsTab {
                        label: "数据与存储",
                        icon: IconKind::Database,
                        active: active_section_snapshot == SettingsSection::Data,
                        onclick: move |_| {
                            active_section.set(SettingsSection::Data);
                            write_settings_section(SettingsSection::Data);
                        },
                    }
                }

                if active_section_snapshot == SettingsSection::Connection {
                    section { class: "max-w-3xl space-y-5",
                        div { class: "flex items-center gap-3",
                            div { class: "flex items-center gap-3",
                                div { class: "flex h-9 w-9 items-center justify-center rounded-md border bg-background",
                                    Icon { kind: IconKind::PlugZap, class: "h-4 w-4" }
                                }
                                div {
                                    div { class: "text-sm font-medium", "API Bridge" }
                                    div { class: "text-xs text-muted-foreground", "Dalamud 插件 · WebSocket" }
                                }
                            }
                        }

                        div { class: "grid gap-2",
                            label { class: "text-sm font-medium", r#for: "xiv-local-bridge-url", "连接地址" }
                            div { class: "relative",
                                input {
                                    id: "xiv-local-bridge-url",
                                    r#type: if show_bridge_url() { "text" } else { "password" },
                                    value: "{url_snapshot}",
                                    placeholder: "ws://127.0.0.1:51398/v1/events?token=...",
                                    class: input_class("pr-10 font-mono text-xs"),
                                    oninput: move |event| {
                                        bridge_url.set(event.value());
                                        saved.set(false);
                                    },
                                }
                                button {
                                    r#type: "button",
                                    class: "absolute right-1 top-1/2 flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground",
                                    aria_label: if show_bridge_url() { "隐藏连接地址" } else { "显示连接地址" },
                                    title: if show_bridge_url() { "隐藏连接地址" } else { "显示连接地址" },
                                    onclick: move |_| show_bridge_url.set(!show_bridge_url()),
                                    Icon {
                                        kind: if show_bridge_url() { IconKind::EyeOff } else { IconKind::Eye },
                                        class: "h-4 w-4"
                                    }
                                }
                            }
                        }

                        div { class: "flex flex-wrap items-center gap-2",
                            Button {
                                variant: ButtonVariant::Primary,
                                disabled: !bridge_settings_dirty,
                                onclick: move |_| {
                                    let url = bridge_url();
                                    save_bridge_url(&url);
                                    mark_bridge_verified(&url);
                                    persisted_bridge_url.set(url.trim().to_string());
                                    saved.set(true);
                                },
                                "保存"
                            }
                            if saved() {
                                span { class: "text-xs text-emerald-700", "已保存" }
                            }
                        }

                        div { class: "border-t pt-5 text-sm text-muted-foreground",
                            "物品页面通过此连接读取当前角色的容器数据，图鉴可以从已保存的物品快照更新进度。连接地址只保存在当前浏览器。"
                        }
                    }
                } else {
                    ResourceSettingsSection {}
                }
            }
        }
    }
}

#[component]
fn SettingsTab(
    label: &'static str,
    icon: IconKind,
    active: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: if active {
                "flex h-10 items-center gap-2 border-b-2 border-foreground px-3 text-sm font-medium text-foreground"
            } else {
                "flex h-10 items-center gap-2 border-b-2 border-transparent px-3 text-sm text-muted-foreground hover:text-foreground"
            },
            aria_pressed: active,
            onclick: move |event| onclick.call(event),
            Icon { kind: icon, class: "h-4 w-4" }
            "{label}"
        }
    }
}

fn initial_settings_section() -> SettingsSection {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    if hash
        .split_once('?')
        .is_some_and(|(_, query)| query.split('&').any(|part| part == "section=data"))
    {
        SettingsSection::Data
    } else {
        SettingsSection::Connection
    }
}

fn write_settings_section(section: SettingsSection) {
    let hash = match section {
        SettingsSection::Connection => "/settings",
        SettingsSection::Data => "/settings?section=data",
    };
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(hash);
    }
}
