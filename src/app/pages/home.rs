use std::rc::Rc;

use dioxus::prelude::*;

use crate::app::collection_bridge::{
    BridgeUpdate, CollectionBridgeConnection, load_bridge_url, mark_bridge_verified,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::ui::{Badge, BadgeVariant};
use crate::app::user_local_directory::{AuthorizedDirectoryLayout, restore_user_local_directory};

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChangelogSection {
    title: String,
    items: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum IntegrationStatus {
    #[default]
    Checking,
    NotConfigured,
    Available(String),
    NeedsAttention(String),
}

#[component]
pub fn HomePage() -> Element {
    let changelog = recent_changelog(CHANGELOG, 3);
    let mut local_data_status = use_signal(IntegrationStatus::default);
    let bridge_status = use_signal(IntegrationStatus::default);
    let bridge_connection = use_signal(|| None::<Rc<CollectionBridgeConnection>>);
    let mut status_check_started = use_signal(|| false);

    use_effect(move || {
        if status_check_started() {
            return;
        }
        status_check_started.set(true);

        spawn(async move {
            match restore_user_local_directory().await {
                Ok(Some(directory))
                    if directory.layout != AuthorizedDirectoryLayout::MissingSqpack =>
                {
                    local_data_status.set(IntegrationStatus::Available(directory.name));
                }
                Ok(Some(_)) => local_data_status.set(IntegrationStatus::NeedsAttention(
                    "已保存的目录中没有找到 sqpack".to_string(),
                )),
                Ok(None) => local_data_status.set(IntegrationStatus::NotConfigured),
                Err(error) => {
                    local_data_status.set(IntegrationStatus::NeedsAttention(error));
                }
            }
        });

        check_api_bridge(bridge_status, bridge_connection);
    });

    rsx! {
        main { class: "min-h-screen bg-background",
            div { class: "mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8",
                header { class: "border-b pb-6",
                    div { class: "text-sm text-muted-foreground", "首页" }
                    h1 { class: "mt-1 text-2xl font-semibold", "XIV Companion" }
                    p { class: "mt-2 max-w-3xl text-sm leading-relaxed text-muted-foreground",
                        "基础功能打开 Web 页面即可使用，数据保存在当前浏览器。模型预览和本地数据刷新需要授权 FFXIV 游戏目录；物品与当前角色状态需要 Dalamud 的 API Bridge 插件。"
                    }
                }

                section { class: "grid gap-4 border-b py-6 md:grid-cols-[10rem_minmax(0,1fr)]",
                    div { class: "flex items-center gap-2 text-sm font-semibold",
                        Icon { kind: IconKind::Info, class: "h-4 w-4" }
                        "公告"
                    }
                    div { class: "flex min-h-20 items-center border-y text-sm text-muted-foreground",
                        "暂无公告"
                    }
                }

                section { class: "grid gap-5 border-b py-6 md:grid-cols-[10rem_minmax(0,1fr)]",
                    div {
                        div { class: "flex items-center gap-2 text-sm font-semibold",
                            Icon { kind: IconKind::PlugZap, class: "h-4 w-4" }
                            "扩展能力"
                        }
                        p { class: "mt-1 text-xs leading-relaxed text-muted-foreground",
                            "按需要配置，不影响其他 Web 功能。"
                        }
                    }
                    div { class: "divide-y border-y",
                        IntegrationRow {
                            icon: IconKind::Database,
                            title: "本地游戏数据",
                            description: "读取 SqPack 中的模型、材质和游戏表数据，不包含当前角色状态。",
                            features: "武器模型 · 合成数据刷新 · 图鉴资源刷新",
                            status: local_data_status(),
                            href: "#/settings?section=data",
                            action: "管理数据",
                        }
                        IntegrationRow {
                            icon: IconKind::PlugZap,
                            title: "API Bridge",
                            description: "通过 Dalamud 插件只读获取当前客户端的角色与物品状态。",
                            features: "物品容器 · 收藏柜与投影台 · 图鉴自动更新",
                            status: bridge_status(),
                            href: "#/settings",
                            action: "配置连接",
                        }
                    }
                }

                section { class: "grid gap-5 border-b py-6 md:grid-cols-[10rem_minmax(0,1fr)]",
                    div {
                        div { class: "flex items-center gap-2 text-sm font-semibold",
                            Icon { kind: IconKind::BookOpen, class: "h-4 w-4" }
                            "如何识别"
                        }
                        p { class: "mt-1 text-xs leading-relaxed text-muted-foreground",
                            "每个功能页标题下都会显示对应能力。"
                        }
                    }
                    div { class: "divide-y border-y text-sm",
                        CapabilityExplanation {
                            variant: BadgeVariant::Success,
                            label: "Web 可用",
                            description: "无需安装扩展，直接使用浏览、搜索、笔记或手动维护功能。",
                        }
                        CapabilityExplanation {
                            variant: BadgeVariant::Outline,
                            label: "本地数据增强",
                            description: "页面基础功能可用，授权游戏目录后可以刷新或补充数据。",
                        }
                        CapabilityExplanation {
                            variant: BadgeVariant::Warning,
                            label: "需要本地数据 / API Bridge",
                            description: "该页的核心功能依赖对应扩展；缺失时页面会给出设置入口。",
                        }
                    }
                }

                section { class: "grid gap-5 py-6 md:grid-cols-[10rem_minmax(0,1fr)]",
                    div {
                        div { class: "flex items-center gap-2 text-sm font-semibold",
                            Icon { kind: IconKind::Sparkles, class: "h-4 w-4" }
                            "最近更新"
                        }
                        p { class: "mt-1 text-xs text-muted-foreground", "来自 CHANGELOG.md" }
                    }
                    div { class: "divide-y border-y",
                        for section in changelog {
                            article { class: "grid gap-3 py-4 sm:grid-cols-[7rem_minmax(0,1fr)]",
                                div { class: "text-sm font-medium tabular-nums", "{section.title}" }
                                ul { class: "space-y-1.5 text-sm leading-relaxed text-muted-foreground",
                                    for item in section.items {
                                        li { class: "flex gap-2",
                                            span { class: "shrink-0 text-muted-foreground", "-" }
                                            span { "{item}" }
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

#[component]
fn IntegrationRow(
    icon: IconKind,
    title: &'static str,
    description: &'static str,
    features: &'static str,
    status: IntegrationStatus,
    href: &'static str,
    action: &'static str,
) -> Element {
    rsx! {
        article { class: "grid gap-3 py-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center",
            div { class: "flex min-w-0 gap-3",
                div { class: "mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-background",
                    Icon { kind: icon, class: "h-4 w-4" }
                }
                div { class: "min-w-0",
                    div { class: "flex flex-wrap items-center gap-2",
                        h2 { class: "text-sm font-semibold", "{title}" }
                        Badge { variant: integration_status_variant(&status),
                            {integration_status_label(&status)}
                        }
                    }
                    p { class: "mt-1 text-sm leading-relaxed text-muted-foreground", "{description}" }
                    div { class: "mt-1 text-xs text-muted-foreground", "支持：{features}" }
                    div { class: "mt-1 text-xs", "{integration_status_detail(&status)}" }
                }
            }
            a {
                href,
                class: "inline-flex items-center gap-1 text-sm font-medium text-foreground hover:underline",
                "{action}"
                Icon { kind: IconKind::ChevronRight, class: "h-4 w-4" }
            }
        }
    }
}

#[component]
fn CapabilityExplanation(
    variant: BadgeVariant,
    label: &'static str,
    description: &'static str,
) -> Element {
    rsx! {
        div { class: "grid gap-2 py-3 sm:grid-cols-[12rem_minmax(0,1fr)] sm:items-center",
            div { Badge { variant, "{label}" } }
            p { class: "leading-relaxed text-muted-foreground", "{description}" }
        }
    }
}

fn check_api_bridge(
    mut status: Signal<IntegrationStatus>,
    mut connection: Signal<Option<Rc<CollectionBridgeConnection>>>,
) {
    let url = load_bridge_url();
    if url.trim().is_empty() {
        status.set(IntegrationStatus::NotConfigured);
        return;
    }

    let verified_url = url.clone();
    match CollectionBridgeConnection::connect(&url, &[], move |update| match update {
        BridgeUpdate::Connected => {}
        BridgeUpdate::SnapshotReady(_) => {
            mark_bridge_verified(&verified_url);
            status.set(IntegrationStatus::Available(
                "已连接并可读取角色数据".to_string(),
            ));
        }
        BridgeUpdate::Disconnected => status.set(IntegrationStatus::NeedsAttention(
            "连接已断开，请确认插件已启用".to_string(),
        )),
        BridgeUpdate::Error(error) => status.set(IntegrationStatus::NeedsAttention(error)),
    }) {
        Ok(next) => {
            connection.set(Some(next));
            spawn(async move {
                gloo_timers::future::TimeoutFuture::new(8_000).await;
                if matches!(&*status.peek(), IntegrationStatus::Checking) {
                    connection.set(None);
                    status.set(IntegrationStatus::NeedsAttention(
                        "连接检测超时，请确认游戏与插件正在运行".to_string(),
                    ));
                }
            });
        }
        Err(error) => status.set(IntegrationStatus::NeedsAttention(error)),
    }
}

fn integration_status_label(status: &IntegrationStatus) -> &'static str {
    match status {
        IntegrationStatus::Checking => "检测中",
        IntegrationStatus::NotConfigured => "未配置",
        IntegrationStatus::Available(_) => "当前可用",
        IntegrationStatus::NeedsAttention(_) => "需要处理",
    }
}

fn integration_status_detail(status: &IntegrationStatus) -> &str {
    match status {
        IntegrationStatus::Checking => "正在检查当前浏览器中的配置",
        IntegrationStatus::NotConfigured => "尚未配置；仅影响上方列出的增强功能",
        IntegrationStatus::Available(detail) | IntegrationStatus::NeedsAttention(detail) => detail,
    }
}

fn integration_status_variant(status: &IntegrationStatus) -> BadgeVariant {
    match status {
        IntegrationStatus::Available(_) => BadgeVariant::Success,
        IntegrationStatus::NeedsAttention(_) => BadgeVariant::Warning,
        IntegrationStatus::Checking | IntegrationStatus::NotConfigured => BadgeVariant::Outline,
    }
}

fn recent_changelog(markdown: &str, limit: usize) -> Vec<ChangelogSection> {
    let mut sections = Vec::new();
    let mut current: Option<ChangelogSection> = None;

    for line in markdown.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if let Some(section) = current.take() {
                if !section.items.is_empty() {
                    sections.push(section);
                    if sections.len() == limit {
                        break;
                    }
                }
            }
            current = Some(ChangelogSection {
                title: title.trim().to_string(),
                items: Vec::new(),
            });
        } else if let Some(item) = line.strip_prefix("- ")
            && let Some(section) = current.as_mut()
        {
            section.items.push(item.trim().to_string());
        }
    }

    if sections.len() < limit
        && let Some(section) = current
        && !section.items.is_empty()
    {
        sections.push(section);
    }
    sections.truncate(limit);
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_recent_changelog_sections() {
        let sections = recent_changelog(
            "# Changelog\n\n## 开发中\n\n- First\n\n## 2026-07-20\n\n- Second\n",
            2,
        );
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "开发中");
        assert_eq!(sections[0].items, ["First"]);
        assert_eq!(sections[1].title, "2026-07-20");
    }

    #[test]
    fn skips_empty_changelog_sections() {
        let sections = recent_changelog(
            "# Changelog\n\n## 开发中\n\n## 2026-07-21\n\n- Homepage\n",
            3,
        );
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "2026-07-21");
    }
}
