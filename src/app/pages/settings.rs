use std::rc::Rc;

use dioxus::prelude::*;

use crate::app::collection_bridge::{
    BridgeUpdate, CollectionBridgeConnection, load_bridge_url, mark_bridge_verified,
    save_bridge_url,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{Badge, BadgeVariant, Button, ButtonVariant, input_class};

use super::settings_resources::ResourceSettingsSection;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsSection {
    #[default]
    Connection,
    Data,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum BridgeTestStatus {
    #[default]
    Idle,
    Testing,
    Available,
    Error(String),
}

#[component]
pub fn SettingsPage() -> Element {
    let mut bridge_url = use_signal(load_bridge_url);
    let mut persisted_bridge_url = use_signal(load_bridge_url);
    let bridge_connection = use_signal(|| None::<Rc<CollectionBridgeConnection>>);
    let bridge_generation = use_signal(|| 0_u64);
    let mut bridge_status = use_signal(BridgeTestStatus::default);
    let mut saved = use_signal(|| false);
    let mut show_bridge_url = use_signal(|| false);
    let mut active_section = use_signal(initial_settings_section);

    let status_snapshot = bridge_status();
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
                        div { class: "flex flex-wrap items-center justify-between gap-3",
                            div { class: "flex items-center gap-3",
                                div { class: "flex h-9 w-9 items-center justify-center rounded-md border bg-background",
                                    Icon { kind: IconKind::PlugZap, class: "h-4 w-4" }
                                }
                                div {
                                    div { class: "text-sm font-medium", "API Bridge" }
                                    div { class: "text-xs text-muted-foreground", "Dalamud 插件 · 本机只读 WebSocket" }
                                }
                            }
                            Badge {
                                variant: match &status_snapshot {
                                    BridgeTestStatus::Available => BadgeVariant::Success,
                                    BridgeTestStatus::Testing => BadgeVariant::Warning,
                                    _ => BadgeVariant::Outline,
                                },
                                {bridge_test_status_label(&status_snapshot)}
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
                                        bridge_status.set(BridgeTestStatus::Idle);
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
                                    persisted_bridge_url.set(url.trim().to_string());
                                    if matches!(bridge_status(), BridgeTestStatus::Available) {
                                        mark_bridge_verified(&url);
                                    }
                                    saved.set(true);
                                },
                                "保存"
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: bridge_url().trim().is_empty()
                                    || matches!(bridge_status(), BridgeTestStatus::Testing),
                                onclick: move |_| test_bridge_connection(
                                    bridge_url(),
                                    bridge_connection,
                                    bridge_generation,
                                    bridge_status,
                                ),
                                Icon { kind: IconKind::PlugZap, class: "h-4 w-4" }
                                "测试连接"
                            }
                            if saved() {
                                span { class: "text-xs text-emerald-700", "已保存" }
                            }
                        }

                        if let BridgeTestStatus::Error(error) = &status_snapshot {
                            div { class: "text-sm text-destructive", "{error}" }
                        }

                        div { class: "border-t pt-5 text-sm text-muted-foreground",
                            "物品页面通过此连接读取当前角色的容器数据，图鉴可以从已保存的物品快照更新进度。连接地址和验证结果只保存在当前浏览器。"
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

fn test_bridge_connection(
    url: String,
    mut connection: Signal<Option<Rc<CollectionBridgeConnection>>>,
    mut generation: Signal<u64>,
    mut status: Signal<BridgeTestStatus>,
) {
    let active_generation = generation.peek().wrapping_add(1);
    generation.set(active_generation);
    status.set(BridgeTestStatus::Testing);
    let verified_url = url.clone();

    match CollectionBridgeConnection::connect(&url, &[], move |update| {
        if *generation.peek() != active_generation {
            return;
        }
        match update {
            BridgeUpdate::Connected => {}
            BridgeUpdate::SnapshotReady(_) => {
                if load_bridge_url().trim() == verified_url {
                    mark_bridge_verified(&verified_url);
                }
                status.set(BridgeTestStatus::Available);
            }
            BridgeUpdate::Disconnected => {
                status.set(BridgeTestStatus::Error("API Bridge 连接已断开".to_string()))
            }
            BridgeUpdate::Error(error) => status.set(BridgeTestStatus::Error(error)),
        }
    }) {
        Ok(next) => {
            connection.set(Some(next));
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(15_000).await;
                let still_testing = *generation.peek() == active_generation
                    && matches!(&*status.peek(), BridgeTestStatus::Testing);
                if still_testing {
                    generation += 1;
                    connection.set(None);
                    status.set(BridgeTestStatus::Error(
                        "API Bridge 连接测试超时".to_string(),
                    ));
                }
            });
        }
        Err(error) => {
            connection.set(None);
            status.set(BridgeTestStatus::Error(error));
        }
    }
}

fn bridge_test_status_label(status: &BridgeTestStatus) -> &'static str {
    match status {
        BridgeTestStatus::Idle => "未测试",
        BridgeTestStatus::Testing => "测试中",
        BridgeTestStatus::Available => "可用",
        BridgeTestStatus::Error(_) => "不可用",
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
