pub mod crafting;
pub mod notes;
pub mod weapon_models;

use std::rc::Rc;

use dioxus::prelude::*;
use xiv_companion::{
    CraftDataId, CraftDataResource, ItemIconId, ItemIconResource, ResourceError, ResourceErrorKind,
    ResourceSource, ResourceSpec, WeaponCatalogId, WeaponCatalogResource,
};

use crate::app::data::{
    LoadedCraftData, clear_item_icon_cache, load_craft_data_with_source, load_weapon_catalog,
};
use crate::app::icons::{Icon, IconKind};
use crate::app::load_progress::{self, CraftDataLoadProgress};
use crate::app::log;
use crate::app::modules::APP_MODULES;
use crate::app::resource_settings::{
    ResourceSettings, SourcePreference, configured_web_resource_hub_for, is_user_local_path_usable,
    load_resource_settings, path_user_local_provider_available_for_runtime, save_resource_settings,
};
use crate::app::resources::IndexedDbCachedProvider;
use crate::app::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardContent, CardHeader,
    CardTitle, input_class,
};
use crate::app::user_local_directory::{
    AuthorizedDirectoryLayout, AuthorizedUserLocalDirectory, authorize_user_local_directory,
    restore_user_local_directory, save_current_user_local_directory_handle,
};
use crate::app::utils::{cx, format_integer};

pub use crafting::CraftingPage;
pub use notes::NotesPage;
pub use weapon_models::WeaponModelsPage;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ResourceTestResult {
    Ok(String),
    Err(ResourceDiagnostic),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SaveFeedback {
    Saved,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UserLocalStatus {
    PathProviderUnavailable,
    MissingPath,
    IncompletePath,
    Configured,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResourceDiagnostic {
    title: String,
    summary: String,
    action: String,
    details: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SettingsValidation {
    valid: bool,
    user_local_status: UserLocalStatus,
    message: String,
    details: Vec<String>,
}

fn source_name(source: ResourceSource) -> &'static str {
    match source {
        ResourceSource::Builtin => "Builtin",
        ResourceSource::IndexedDb => "IndexedDB",
        ResourceSource::UserLocal => "UserLocal",
    }
}

fn preference_sources(value: SourcePreference) -> &'static str {
    match value {
        SourcePreference::BuiltinFirst => "Builtin -> UserLocal",
        SourcePreference::UserLocalFirst => "UserLocal -> Builtin",
        SourcePreference::BuiltinOnly => "Builtin",
        SourcePreference::UserLocalOnly => "UserLocal",
    }
}

fn user_local_status(
    settings: &ResourceSettings,
    authorized_directory: Option<&AuthorizedUserLocalDirectory>,
) -> UserLocalStatus {
    if authorized_directory
        .is_some_and(|directory| directory.layout != AuthorizedDirectoryLayout::MissingSqpack)
    {
        return UserLocalStatus::Configured;
    }
    if !path_user_local_provider_available_for_runtime() {
        return UserLocalStatus::MissingPath;
    }

    let path = settings.user_local_path.trim();
    if path.is_empty() {
        UserLocalStatus::MissingPath
    } else if !is_user_local_path_usable(path) {
        UserLocalStatus::IncompletePath
    } else {
        UserLocalStatus::Configured
    }
}

fn user_local_status_label(status: UserLocalStatus) -> &'static str {
    match status {
        UserLocalStatus::PathProviderUnavailable => "需接入 provider",
        UserLocalStatus::MissingPath => "未配置",
        UserLocalStatus::IncompletePath => "路径不完整",
        UserLocalStatus::Configured => "已配置",
    }
}

fn user_local_status_summary(status: UserLocalStatus) -> &'static str {
    match status {
        UserLocalStatus::PathProviderUnavailable => "当前运行时还没有可用的本地 provider",
        UserLocalStatus::MissingPath => "选择并保存游戏目录后即可作为 Local 数据源使用",
        UserLocalStatus::IncompletePath => {
            "Native 路径模式预留字段不完整；Web 下请优先选择游戏目录"
        }
        UserLocalStatus::Configured => "Local 数据源已可用",
    }
}

fn user_local_next_action(status: UserLocalStatus) -> &'static str {
    match status {
        UserLocalStatus::PathProviderUnavailable => {
            "Web 版本通过浏览器授权读取本地文件；完整 SqPack 直接解包将按 EXDViewer 的 DirectoryHandle + VFS 方案接入。"
        }
        UserLocalStatus::MissingPath => {
            "点击“选择游戏目录”，授权 FF14 安装根目录或 game 目录；确认无误后保存。"
        }
        UserLocalStatus::IncompletePath => {
            "如果使用 Native 路径模式，请输入完整 game 目录；Web 模式无需手动路径。"
        }
        UserLocalStatus::Configured => {
            "保存并应用后，CraftData 等游戏数据会从本地 SqPack 读取；ItemIcon 固定使用 Builtin/API。"
        }
    }
}

fn authorized_directory_layout_label(layout: AuthorizedDirectoryLayout) -> &'static str {
    match layout {
        AuthorizedDirectoryLayout::GameDir => "已找到 sqpack",
        AuthorizedDirectoryLayout::InstallRoot => "已找到 game\\sqpack",
        AuthorizedDirectoryLayout::MissingSqpack => "未找到 sqpack",
    }
}

fn authorized_directory_layout_summary(layout: AuthorizedDirectoryLayout) -> &'static str {
    match layout {
        AuthorizedDirectoryLayout::GameDir => "这个授权目录看起来就是 FF14 game 目录。",
        AuthorizedDirectoryLayout::InstallRoot => "这个授权目录看起来是 FF14 安装根目录。",
        AuthorizedDirectoryLayout::MissingSqpack => {
            "这个授权目录下没有 sqpack，也没有 game\\sqpack；请重新授权 FF14 安装根目录或 game 目录。"
        }
    }
}

fn preference_uses_user_local(value: SourcePreference) -> bool {
    matches!(
        value,
        SourcePreference::UserLocalFirst | SourcePreference::UserLocalOnly
    )
}

fn validate_resource_settings(
    settings: &ResourceSettings,
    authorized_directory: Option<&AuthorizedUserLocalDirectory>,
) -> SettingsValidation {
    let user_local_status = user_local_status(settings, authorized_directory);
    if let Some(directory) = authorized_directory {
        if directory.layout == AuthorizedDirectoryLayout::MissingSqpack {
            return SettingsValidation {
                valid: false,
                user_local_status,
                message: "所选目录不是可用的 FF14 游戏目录。".to_string(),
                details: vec![
                    format!("目录: {}", directory.name),
                    "没有找到 sqpack 或 game\\sqpack，不能保存为 UserLocal 来源。".to_string(),
                ],
            };
        }
    }

    let craft_preference = settings.craft_data_preference();
    let icon_preference = settings.item_icon_preference();
    // On Web, CraftData and WeaponCatalog are always served from IndexedDB and ItemIcon from
    // Builtin/API, so resource preferences do not require a UserLocal directory.
    let needs_user_local = if cfg!(target_arch = "wasm32") {
        false
    } else {
        preference_uses_user_local(craft_preference) || preference_uses_user_local(icon_preference)
    };

    if needs_user_local && user_local_status == UserLocalStatus::PathProviderUnavailable {
        return SettingsValidation {
            valid: false,
            user_local_status,
            message: "当前运行时还没有可应用的 UserLocal provider。".to_string(),
            details: vec![
                "Web 版需要先通过“选择游戏目录”保存 FileSystemDirectoryHandle。".to_string(),
                "保存后 BrowserSqPackProvider 会使用该 handle 读取 CraftData 等游戏数据；ItemIcon 仍固定使用 Builtin/API。".to_string(),
            ],
        };
    }

    if user_local_status == UserLocalStatus::IncompletePath {
        return SettingsValidation {
            valid: false,
            user_local_status,
            message: "UserLocal 路径不完整，不能保存为可用配置。".to_string(),
            details: vec![
                format!("当前路径: {}", settings.user_local_path.trim()),
                "请输入完整本地路径；浏览器授权目录的目录名不能作为路径使用。".to_string(),
            ],
        };
    }

    if needs_user_local && user_local_status == UserLocalStatus::MissingPath {
        return SettingsValidation {
            valid: false,
            user_local_status,
            message: "当前草稿需要 UserLocal，但还没有选择可用的游戏目录。".to_string(),
            details: vec![
                format!("CraftData: {}", preference_sources(craft_preference)),
                format!("ItemIcon: {}", preference_sources(icon_preference)),
                "请先选择 FF14 安装根目录或 game 目录，或把策略切回 Builtin。".to_string(),
            ],
        };
    }

    SettingsValidation {
        valid: true,
        user_local_status,
        message: if user_local_status == UserLocalStatus::Configured {
            "草稿可保存；保存后会用 Local 数据源重新加载资源。".to_string()
        } else {
            "草稿可保存；当前配置只使用 Builtin 来源。".to_string()
        },
        details: Vec::new(),
    }
}

fn error_kind_label(kind: ResourceErrorKind) -> &'static str {
    match kind {
        ResourceErrorKind::DecodeFailed => "解码失败",
        ResourceErrorKind::NotFound => "文件未找到",
        ResourceErrorKind::NoSourceAvailable => "无可用来源",
        ResourceErrorKind::PermissionMissing => "缺少权限",
        ResourceErrorKind::ProviderFailed => "来源读取失败",
        ResourceErrorKind::Unsupported => "来源不支持",
        ResourceErrorKind::VersionMismatch => "版本不匹配",
    }
}

fn diagnose_resource_error(
    resource_label: &'static str,
    source: ResourceSource,
    settings: &ResourceSettings,
    error: ResourceError,
) -> ResourceDiagnostic {
    let mut details = vec![
        format!("资源类型: {resource_label}"),
        format!("请求来源: {}", source_name(source)),
        format!("错误类别: {}", error_kind_label(error.kind)),
        format!("内部错误: {error}"),
    ];

    if source == ResourceSource::UserLocal {
        details.push(format!(
            "UserLocal 状态: {}",
            user_local_status_label(user_local_status(settings, None))
        ));
        details.push(format!(
            "UserLocal 路径: {}",
            settings.user_local_path.trim()
        ));
    }

    if source == ResourceSource::UserLocal && error.kind == ResourceErrorKind::Unsupported {
        let status = user_local_status(settings, None);

        let (title, summary) = match status {
            UserLocalStatus::PathProviderUnavailable => (
                "UserLocal provider 未接入",
                "当前没有可用的浏览器目录 handle，UserLocal provider 不会匹配资源。",
            ),
            UserLocalStatus::MissingPath => (
                "UserLocal 路径未配置",
                "没有选择游戏目录时，浏览器本地 provider 不会注册为可用来源。",
            ),
            UserLocalStatus::IncompletePath => (
                "UserLocal 路径不完整",
                "当前 Native 路径像目录名或相对路径，不能注册为可用本地来源。",
            ),
            UserLocalStatus::Configured => (
                "UserLocal provider 未匹配该资源",
                "当前已配置路径，但没有 provider 声明可以提供这个资源类型。",
            ),
        };

        return ResourceDiagnostic {
            title: title.to_string(),
            summary: summary.to_string(),
            action: user_local_next_action(status).to_string(),
            details,
        };
    }

    if source == ResourceSource::UserLocal {
        let error_text = error.to_string();
        if error.kind == ResourceErrorKind::ProviderFailed
            && error_text.contains("failed to find sqpack")
        {
            return ResourceDiagnostic {
                title: "没有找到 sqpack".to_string(),
                summary: "UserLocal provider 已启动，但当前路径下没有 sqpack，也没有 game\\sqpack。"
                    .to_string(),
                action: "如果路径来自“授权浏览器目录”，它只是目录名，不是实际路径；请手动输入完整 FF14 game 目录或安装根目录路径。"
                    .to_string(),
                details,
            };
        }

        let (title, summary, action) = match error.kind {
            ResourceErrorKind::NotFound => (
                "本地资源文件未找到",
                "ResourceHub 已调用 UserLocal provider，但没有在本地游戏目录中找到对应文件。",
                "确认路径指向 FF14 的 game 目录，并检查游戏资源是否完整。",
            ),
            ResourceErrorKind::ProviderFailed => (
                "本地资源读取失败",
                "UserLocal provider 已启动，但读取或转换本地游戏数据时失败。",
                "确认路径有效，并检查路径下是否存在 sqpack 或 game\\sqpack。",
            ),
            ResourceErrorKind::DecodeFailed => (
                "本地资源解码失败",
                "已读取到本地资源，但转换成应用数据结构时失败。",
                "保留错误详情并检查资源版本或解码流程。",
            ),
            ResourceErrorKind::PermissionMissing => (
                "缺少本地资源权限",
                "运行环境没有读取本地游戏目录所需的权限。",
                "重新选择目录授权，或在具备本地文件权限的运行环境中启动。",
            ),
            _ => (
                "UserLocal 读取失败",
                "本地资源来源返回错误。",
                "查看详情中的内部错误，再决定是修路径、修 provider，还是切回 Builtin。",
            ),
        };

        return ResourceDiagnostic {
            title: title.to_string(),
            summary: summary.to_string(),
            action: action.to_string(),
            details,
        };
    }

    ResourceDiagnostic {
        title: format!("{} 读取失败", source_name(source)),
        summary: "资源来源返回错误。".to_string(),
        action:
            "查看详情中的内部错误；如果 Builtin 失败，通常需要检查 bundled asset 或网络可用性。"
                .to_string(),
        details,
    }
}

async fn test_craft_data(settings: ResourceSettings, source: ResourceSource) -> ResourceTestResult {
    let hub = configured_web_resource_hub_for(&settings);
    match hub
        .load_from::<CraftDataResource>(source, CraftDataId::Default)
        .await
    {
        Ok(data) => ResourceTestResult::Ok(format!(
            "{} / 物品 {} / 配方 {}",
            data.game_version,
            format_integer(data.counts.items as f64),
            format_integer(data.counts.recipes as f64)
        )),
        Err(error) => ResourceTestResult::Err(diagnose_resource_error(
            "CraftData",
            source,
            &settings,
            error,
        )),
    }
}

async fn test_item_icon(settings: ResourceSettings, source: ResourceSource) -> ResourceTestResult {
    if source == ResourceSource::UserLocal {
        return ResourceTestResult::Err(ResourceDiagnostic {
            title: "ItemIcon 固定使用 Builtin/API".to_string(),
            summary: "Web 版不再从本地 SqPack 解码 .tex 图标。".to_string(),
            action: "图标会使用浏览器原生 <img> 加载 API/Builtin URL；本地 UserLocal 保留给 CraftData 和未来模型等游戏数据。".to_string(),
            details: vec![
                "避免批量 TEX 解码造成主线程卡顿和 WASM 内存峰值。".to_string(),
                "如需验证图标，请测试 Builtin 来源。".to_string(),
            ],
        });
    }

    let hub = configured_web_resource_hub_for(&settings);
    match hub
        .load_from::<ItemIconResource>(source, ItemIconId { icon_id: 65000 })
        .await
    {
        Ok(info) => ResourceTestResult::Ok(format!("{} builtin URLs", info.urls.len())),
        Err(error) => ResourceTestResult::Err(diagnose_resource_error(
            "ItemIcon", source, &settings, error,
        )),
    }
}

#[component]
pub fn WorkspacePage() -> Element {
    let mut settings = use_signal(load_resource_settings);
    let mut applied_settings = use_signal(load_resource_settings);
    let mut settings_revision = use_signal(|| 0_u64);
    let mut craft_test = use_signal(|| None::<(ResourceSource, ResourceTestResult)>);
    let mut icon_test = use_signal(|| None::<(ResourceSource, ResourceTestResult)>);
    let mut directory_pick_error = use_signal(|| None::<String>);
    let mut authorized_user_local_directory = use_signal(|| None::<AuthorizedUserLocalDirectory>);
    let mut directory_dirty = use_signal(|| false);
    let mut restore_started = use_signal(|| false);
    let mut craft_data_progress = use_signal(|| None::<CraftDataLoadProgress>);
    let mut save_feedback = use_signal(|| None::<SaveFeedback>);
    let mut craft_data_source_info = use_signal(|| None::<(String, String)>);
    let mut weapon_catalog_source_info = use_signal(|| None::<(String, String)>);
    let mut craft_data = use_resource(move || {
        let _ = settings_revision();
        load_craft_data_with_source(applied_settings())
    });
    let mut weapon_catalog = use_resource(move || {
        let _ = settings_revision();
        load_weapon_catalog()
    });

    use_effect(move || {
        load_progress::set_craft_data_progress_sink(move |progress| {
            if let Ok(mut slot) = craft_data_progress.try_write() {
                *slot = progress;
            }
        });
    });

    use_drop(move || {
        load_progress::clear_craft_data_progress();
    });

    use_effect(move || {
        spawn(async move {
            let request = CraftDataResource::request(&CraftDataId::Default);
            match IndexedDbCachedProvider::current_cache_info(request).await {
                Ok(info) => craft_data_source_info.set(info),
                Err(error) => log::warn("resource", format!("craft-data cache info failed: {error}")),
            }
        });
        spawn(async move {
            let request = WeaponCatalogResource::request(&WeaponCatalogId::Default);
            match IndexedDbCachedProvider::current_cache_info(request).await {
                Ok(info) => weapon_catalog_source_info.set(info),
                Err(error) => log::warn("resource", format!("weapon-catalog cache info failed: {error}")),
            }
        });
    });

    use_effect(move || {
        if restore_started() {
            return;
        }
        restore_started.set(true);
        spawn(async move {
            match restore_user_local_directory().await {
                Ok(Some(directory)) => {
                    clear_item_icon_cache();
                    authorized_user_local_directory.set(Some(directory));
                    directory_dirty.set(false);
                    settings_revision.set(settings_revision() + 1);
                    craft_data.restart();
                    weapon_catalog.restart();
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn("local-dir", format!("restore failed: {error}"));
                    directory_pick_error.set(Some(error));
                }
            }
        });
    });

    let current_settings = settings();
    let applied_settings_snapshot = applied_settings();
    let settings_dirty = current_settings != applied_settings_snapshot || directory_dirty();
    let authorized_directory_snapshot = authorized_user_local_directory();
    let validation =
        validate_resource_settings(&current_settings, authorized_directory_snapshot.as_ref());

    rsx! {
        div { class: "mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8",
            div { class: "flex flex-col gap-2",
                div { class: "text-sm text-muted-foreground", "工作台" }
                h1 { class: "text-2xl font-semibold", "XIV Companion" }
            }

            ResourcePanel {
                settings: current_settings,
                craft_data,
                weapon_catalog,
                craft_test: craft_test(),
                icon_test: icon_test(),
                directory_pick_error: directory_pick_error(),
                authorized_user_local_directory: authorized_directory_snapshot,
                craft_progress: craft_data_progress(),
                craft_data_source_info: craft_data_source_info(),
                weapon_catalog_source_info: weapon_catalog_source_info(),
                settings_dirty,
                save_feedback: save_feedback(),
                validation,
                on_settings_change: move |next| {
                    settings.set(next);
                    load_progress::clear_craft_data_progress();

                    craft_test.set(None);
                    icon_test.set(None);
                    directory_pick_error.set(None);
                    save_feedback.set(None);
                },
                on_save: move |_| {
                    let next = settings();
                    if !validate_resource_settings(&next, authorized_user_local_directory().as_ref()).valid {
                        return;
                    }
                    directory_pick_error.set(None);
                    load_progress::clear_craft_data_progress();

                    spawn(async move {
                        if directory_dirty() {
                            if let Err(error) = save_current_user_local_directory_handle().await {
                                log::error("local-dir", format!("save failed: {error}"));
                                directory_pick_error.set(Some(error));
                                return;
                            }
                        }
                        save_resource_settings(&next);
                        applied_settings.set(next);
                        directory_dirty.set(false);
                        save_feedback.set(Some(SaveFeedback::Saved));
                        settings_revision.set(settings_revision() + 1);
                        craft_data.restart();
                        weapon_catalog.restart();
                    });
                },
                on_cancel: move |_| {
                    settings.set(applied_settings());
                    load_progress::clear_craft_data_progress();

                    craft_test.set(None);
                    icon_test.set(None);
                    directory_pick_error.set(None);
                    spawn(async move {
                        match restore_user_local_directory().await {
                            Ok(directory) => {
                                clear_item_icon_cache();
                                authorized_user_local_directory.set(directory)
                            }
                            Err(error) => directory_pick_error.set(Some(error)),
                        }
                        directory_dirty.set(false);
                        save_feedback.set(Some(SaveFeedback::Cancelled));
                        settings_revision.set(settings_revision() + 1);
                        craft_data.restart();
                        weapon_catalog.restart();
                    });
                },
                on_test_craft: move |source| {
                    let snapshot = settings();
                    load_progress::clear_craft_data_progress();

                    spawn(async move {
                        let result = test_craft_data(snapshot, source).await;
                        craft_test.set(Some((source, result)));
                    });
                },
                on_test_icon: move |source| {
                    let snapshot = settings();
                    load_progress::clear_craft_data_progress();

                    spawn(async move {
                        let result = test_item_icon(snapshot, source).await;
                        icon_test.set(Some((source, result)));
                    });
                },
                on_choose_user_local_dir: move |_| {
                    directory_pick_error.set(None);
                    load_progress::clear_craft_data_progress();

                    spawn(async move {
                        match authorize_user_local_directory().await {
                            Ok(directory) => {
                                clear_item_icon_cache();
                                authorized_user_local_directory.set(Some(directory));
                                craft_test.set(None);
                                icon_test.set(None);
                                save_feedback.set(None);
                                directory_dirty.set(true);
                                settings_revision.set(settings_revision() + 1);
                                craft_data.restart();
                                weapon_catalog.restart();
                            }
                            Err(error) => {
                                log::warn("local-dir", format!("directory pick failed: {error}"));
                                directory_pick_error.set(Some(error));
                            }
                        }
                    });
                },
                on_update_craft_data_from_local: move |_| {
                    spawn(async move {
                        let request = CraftDataResource::request(&CraftDataId::Default);
                        match IndexedDbCachedProvider::update_from_local(request.clone()).await {
                            Ok(()) => {
                                match IndexedDbCachedProvider::current_cache_info(request).await {
                                    Ok(info) => craft_data_source_info.set(info),
                                    Err(error) => log::warn("resource", format!("refresh CraftData cache info failed: {error}")),
                                }
                                craft_data.restart();
                            }
                            Err(error) => {
                                log::warn("resource", format!("update CraftData from local failed: {error}"));
                            }
                        }
                    });
                },
                on_reset_craft_data_to_builtin: move |_| {
                    spawn(async move {
                        let request = CraftDataResource::request(&CraftDataId::Default);
                        match IndexedDbCachedProvider::reset_to_builtin(request.clone()).await {
                            Ok(()) => {
                                match IndexedDbCachedProvider::current_cache_info(request).await {
                                    Ok(info) => craft_data_source_info.set(info),
                                    Err(error) => log::warn("resource", format!("refresh CraftData cache info failed: {error}")),
                                }
                                craft_data.restart();
                            }
                            Err(error) => {
                                log::warn("resource", format!("reset CraftData to builtin failed: {error}"));
                            }
                        }
                    });
                },
                on_update_weapon_catalog_from_local: move |_| {
                    spawn(async move {
                        let request = WeaponCatalogResource::request(&WeaponCatalogId::Default);
                        match IndexedDbCachedProvider::update_from_local(request.clone()).await {
                            Ok(()) => {
                                match IndexedDbCachedProvider::current_cache_info(request).await {
                                    Ok(info) => weapon_catalog_source_info.set(info),
                                    Err(error) => log::warn("resource", format!("refresh WeaponCatalog cache info failed: {error}")),
                                }
                                weapon_catalog.restart();
                            }
                            Err(error) => {
                                log::warn("resource", format!("update WeaponCatalog from local failed: {error}"));
                            }
                        }
                    });
                },
                on_reset_weapon_catalog_to_builtin: move |_| {
                    spawn(async move {
                        let request = WeaponCatalogResource::request(&WeaponCatalogId::Default);
                        match IndexedDbCachedProvider::reset_to_builtin(request.clone()).await {
                            Ok(()) => {
                                match IndexedDbCachedProvider::current_cache_info(request).await {
                                    Ok(info) => weapon_catalog_source_info.set(info),
                                    Err(error) => log::warn("resource", format!("refresh WeaponCatalog cache info failed: {error}")),
                                }
                                weapon_catalog.restart();
                            }
                            Err(error) => {
                                log::warn("resource", format!("reset WeaponCatalog to builtin failed: {error}"));
                            }
                        }
                    });
                },
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
                                            kind: match module.id {
                                                "notes" => IconKind::BookOpen,
                                                "weapon-models" => IconKind::Sword,
                                                _ => IconKind::Wrench,
                                            },
                                            class: "h-5 w-5"
                                        }
                                    }
                                }
                                CardContent { class: "space-y-2".to_string(),
                                    CardTitle { "{module.label}" }
                                    div { class: "text-sm text-muted-foreground",
                                        {module_description(module.id)}
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

fn module_description(id: &str) -> &'static str {
    match id {
        "crafting" => "配方树、素材汇总、来源选择",
        "notes" => "目录页面、分栏卡片、材料汇总",
        "weapon-models" => "本地 SqPack 武器检索和模型预览",
        _ => "工作区模块",
    }
}

#[component]
fn ResourcePanel(
    settings: ResourceSettings,
    craft_data: Resource<Result<LoadedCraftData, String>>,
    weapon_catalog: Resource<Result<Rc<xiv_companion::WeaponCatalogPackage>, String>>,
    craft_test: Option<(ResourceSource, ResourceTestResult)>,
    icon_test: Option<(ResourceSource, ResourceTestResult)>,
    directory_pick_error: Option<String>,
    authorized_user_local_directory: Option<AuthorizedUserLocalDirectory>,
    craft_progress: Option<CraftDataLoadProgress>,
    craft_data_source_info: Option<(String, String)>,
    weapon_catalog_source_info: Option<(String, String)>,
    settings_dirty: bool,
    save_feedback: Option<SaveFeedback>,
    validation: SettingsValidation,
    on_settings_change: EventHandler<ResourceSettings>,
    on_save: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_test_craft: EventHandler<ResourceSource>,
    on_test_icon: EventHandler<ResourceSource>,
    on_choose_user_local_dir: EventHandler<()>,
    on_update_craft_data_from_local: EventHandler<()>,
    on_reset_craft_data_to_builtin: EventHandler<()>,
    on_update_weapon_catalog_from_local: EventHandler<()>,
    on_reset_weapon_catalog_to_builtin: EventHandler<()>,
) -> Element {
    let user_local_path = settings.user_local_path.clone();
    let local_status = validation.user_local_status;
    let loading = craft_data.read().is_none() || weapon_catalog.read().is_none();
    let can_save = settings_dirty && validation.valid && !loading;
    let save_label = if loading {
        "加载中"
    } else {
        "保存并应用"
    };
    let settings_for_path = settings.clone();

    rsx! {
        div { class: "rounded-lg border bg-card text-card-foreground shadow-sm",
            // ── Header ──────────────────────────────────────────
            div { class: "flex flex-col gap-3 border-b px-4 py-3 md:flex-row md:items-center md:justify-between",
                div { class: "flex items-center gap-2 min-w-0",
                    Icon { kind: IconKind::Database, class: "h-4 w-4 shrink-0 text-muted-foreground" }
                    h2 { class: "text-sm font-semibold", "资源库" }
                    SettingsFeedbackBadge {
                        settings_dirty,
                        feedback: save_feedback,
                        loading,
                        validation: validation.clone(),
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        disabled: !settings_dirty,
                        onclick: move |_| on_cancel.call(()),
                        "取消"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Sm,
                        disabled: !can_save,
                        onclick: move |_| on_save.call(()),
                        "{save_label}"
                    }
                }
            }

            // ── Validation warning ──────────────────────────────
            if settings_dirty && !validation.valid && !validation.details.is_empty() {
                div { class: "border-b border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-900",
                    for detail in &validation.details {
                        div { "{detail}" }
                    }
                }
            }

            // ── Body ────────────────────────────────────────────
            div { class: "space-y-4 px-4 py-4",
                // Section: Sources
                SectionLabel { label: "数据来源" }
                div { class: "grid gap-3 md:grid-cols-2",
                    BuiltinSourceCard {}
                    UserLocalSourceCard {
                        path: user_local_path,
                        directory_error: directory_pick_error,
                        authorized_directory: authorized_user_local_directory,
                        status: local_status,
                        on_path_change: move |path| {
                            let mut next = settings_for_path.clone();
                            next.user_local_path = path;
                            on_settings_change.call(next);
                        },
                        on_choose: move |_| on_choose_user_local_dir.call(()),
                    }
                }

                // Section: Resource status
                SectionLabel { label: "资源状态" }
                ResourceStatusTable {
                    craft_data,
                    weapon_catalog,
                    craft_data_source_info,
                    weapon_catalog_source_info,
                    user_local_configured: local_status == UserLocalStatus::Configured,
                    on_update_craft_data_from_local,
                    on_reset_craft_data_to_builtin,
                    on_update_weapon_catalog_from_local,
                    on_reset_weapon_catalog_to_builtin,
                }

                // Section: Testing
                SectionLabel { label: "连接测试" }
                ResourceTestSection {
                    craft_test,
                    icon_test,
                    user_local_disabled: local_status != UserLocalStatus::Configured,
                    on_test_craft,
                    on_test_icon,
                }

                // Section: Current data
                SectionLabel { label: "当前数据" }
                DataStatus {
                    craft_data,
                    weapon_catalog,
                    craft_progress,
                }
            }
        }
    }
}

// ─── New sub-components ───────────────────────────────────────────────

/// Compact feedback badge that lives in the panel header.
#[component]
fn SettingsFeedbackBadge(
    settings_dirty: bool,
    feedback: Option<SaveFeedback>,
    loading: bool,
    validation: SettingsValidation,
) -> Element {
    let (text, variant) = if loading {
        ("加载中", BadgeVariant::Secondary)
    } else if settings_dirty && validation.valid {
        ("草稿可保存", BadgeVariant::Success)
    } else if settings_dirty {
        ("草稿无效", BadgeVariant::Warning)
    } else if feedback == Some(SaveFeedback::Saved) {
        ("已应用", BadgeVariant::Success)
    } else if feedback == Some(SaveFeedback::Cancelled) {
        ("已取消", BadgeVariant::Secondary)
    } else {
        ("无改动", BadgeVariant::Secondary)
    };

    rsx! {
        Badge { variant, class: "ml-2".to_string(), "{text}" }
    }
}

/// Small section label with optional count/status.
#[component]
fn SectionLabel(label: &'static str) -> Element {
    rsx! {
        div { class: "text-xs font-medium uppercase tracking-wider text-muted-foreground",
            "{label}"
        }
    }
}

/// Builtin source — always available, informational only.
#[component]
fn BuiltinSourceCard() -> Element {
    rsx! {
        div { class: "rounded-lg border bg-card p-4",
            div { class: "flex items-start gap-3",
                div { class: "flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-muted/50 text-muted-foreground",
                    Icon { kind: IconKind::PackageSearch, class: "h-4 w-4" }
                }
                div { class: "min-w-0 flex-1",
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm font-medium", "Builtin" }
                        Badge { variant: BadgeVariant::Success, "始终可用" }
                    }
                    div { class: "mt-1 text-xs text-muted-foreground",
                        "xiv-companion 内置资源，无需配置即可使用。"
                    }
                    div { class: "mt-2 flex flex-wrap gap-1.5",
                        span { class: "inline-flex h-5 items-center rounded bg-muted px-1.5 text-[11px] text-muted-foreground",
                            "CraftData"
                        }
                        span { class: "inline-flex h-5 items-center rounded bg-muted px-1.5 text-[11px] text-muted-foreground",
                            "ItemIcon URLs"
                        }
                    }
                }
            }
        }
    }
}

/// UserLocal source — browser local data import + optional sqpack directory verification.
#[component]
fn UserLocalSourceCard(
    path: String,
    directory_error: Option<String>,
    authorized_directory: Option<AuthorizedUserLocalDirectory>,
    status: UserLocalStatus,
    on_path_change: EventHandler<String>,
    on_choose: EventHandler<()>,
) -> Element {
    let status_badge = match status {
        UserLocalStatus::Configured => rsx! {
            Badge { variant: BadgeVariant::Success, "已配置" }
        },
        UserLocalStatus::PathProviderUnavailable => rsx! {
            Badge { variant: BadgeVariant::Warning, "不可用" }
        },
        _ => rsx! {
            Badge { variant: BadgeVariant::Warning, "{user_local_status_label(status)}" }
        },
    };

    let authorized_directory_state = authorized_directory.map(|directory| {
        let layout_available =
            !matches!(directory.layout, AuthorizedDirectoryLayout::MissingSqpack);
        (directory, layout_available)
    });

    rsx! {
        div { class: "rounded-lg border bg-card p-4",
            div { class: "flex items-start gap-3",
                div { class: "flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-muted/50 text-muted-foreground",
                    Icon { kind: IconKind::Folder, class: "h-4 w-4" }
                }
                div { class: "min-w-0 flex-1",
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm font-medium", "UserLocal" }
                        {status_badge}
                    }
                    div { class: "mt-1 text-xs text-muted-foreground",
                        "{user_local_status_summary(status)}"
                    }
                }
            }

            div { class: "mt-3 flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    class: "shrink-0".to_string(),
                    title: "浏览器授权 FF14 安装根目录或 game 目录".to_string(),
                    onclick: move |_| on_choose.call(()),
                    Icon { kind: IconKind::FolderPlus, class: "h-3.5 w-3.5" }
                    "选择游戏目录"
                }
            }
            if let Some(ref err) = directory_error {
                div { class: "mt-2 text-xs text-destructive", "{err}" }
            }

            div { class: "mt-3 hidden flex-col gap-2 sm:flex-row",
                input {
                    class: input_class("min-w-0"),
                    value: "{path}",
                    placeholder: "Native 路径模式预留：E:\\SquareEnix\\FINAL FANTASY XIV\\game",
                    oninput: move |event| on_path_change.call(event.value()),
                }
            }

            // Browser directory auth result
            if let Some((directory, layout_available)) = authorized_directory_state {
                AuthorizedDirectoryNotice {
                    directory,
                    layout_available,
                }
            }

            // Next action hint
            div { class: "mt-2 text-[11px] leading-relaxed text-muted-foreground",
                "{user_local_next_action(status)}"
            }
        }
    }
}

#[component]
fn AuthorizedDirectoryNotice(
    directory: AuthorizedUserLocalDirectory,
    layout_available: bool,
) -> Element {
    rsx! {
        div { class: cx(["mt-2 rounded-md border px-2.5 py-2 text-xs",
            if layout_available { "border-emerald-200 bg-emerald-50 text-emerald-900" }
            else { "border-amber-200 bg-amber-50 text-amber-900" },
        ]),
            div { class: "flex flex-wrap items-center gap-2",
                span { class: "font-medium", "{directory.name}" }
                Badge {
                    variant: if layout_available { BadgeVariant::Success } else { BadgeVariant::Warning },
                    "{authorized_directory_layout_label(directory.layout)}"
                }
            }
            div { class: "mt-1", "{authorized_directory_layout_summary(directory.layout)}" }
        }
    }
}

/// Resource status table — shows IndexedDB-backed resources and their current source.
#[component]
fn ResourceStatusTable(
    craft_data: Resource<Result<LoadedCraftData, String>>,
    weapon_catalog: Resource<Result<Rc<xiv_companion::WeaponCatalogPackage>, String>>,
    craft_data_source_info: Option<(String, String)>,
    weapon_catalog_source_info: Option<(String, String)>,
    user_local_configured: bool,
    on_update_craft_data_from_local: EventHandler<()>,
    on_reset_craft_data_to_builtin: EventHandler<()>,
    on_update_weapon_catalog_from_local: EventHandler<()>,
    on_reset_weapon_catalog_to_builtin: EventHandler<()>,
) -> Element {
    let (craft_version, craft_count, craft_source) =
        indexed_db_resource_status(&craft_data, craft_data_source_info.as_ref());
    let (weapon_version, weapon_count, weapon_source) =
        indexed_db_resource_status(&weapon_catalog, weapon_catalog_source_info.as_ref());
    let weapon_model_status = if user_local_configured {
        "已配置本地目录"
    } else {
        "未配置本地目录"
    };

    rsx! {
        div { class: "overflow-hidden rounded-lg border",
            // Table header
            div { class: "grid grid-cols-[1fr_auto] items-center gap-3 border-b bg-muted/30 px-4 py-2 md:grid-cols-[1fr_auto_1fr]",
                div { class: "text-xs font-medium text-muted-foreground", "资源" }
                div { class: "hidden text-xs font-medium text-muted-foreground md:block", "状态" }
                div { class: "text-right text-xs font-medium text-muted-foreground md:text-left", "操作" }
            }

            ResourceStatusRow {
                label: "CraftData",
                description: "配方、物品、来源数据",
                version: craft_version,
                count: craft_count,
                count_label: "物品",
                source: craft_source,
                user_local_configured,
                on_update: move |_| on_update_craft_data_from_local.call(()),
                on_reset: move |_| on_reset_craft_data_to_builtin.call(()),
            }

            ResourceStatusRow {
                label: "WeaponCatalog",
                description: "武器目录数据",
                version: weapon_version,
                count: weapon_count,
                count_label: "武器",
                source: weapon_source,
                user_local_configured,
                on_update: move |_| on_update_weapon_catalog_from_local.call(()),
                on_reset: move |_| on_reset_weapon_catalog_to_builtin.call(()),
            }

            ResourceStatusStaticRow {
                label: "WeaponModel",
                description: "本地 SqPack 武器模型预览",
                status: weapon_model_status,
            }

            ResourceStatusStaticRow {
                label: "ItemIcon",
                description: "物品图标 URL",
                status: "Builtin/API",
            }
        }
    }
}

fn indexed_db_resource_status<T>(
    data: &Resource<Result<T, String>>,
    cache_info: Option<&(String, String)>,
) -> (Option<String>, Option<usize>, &'static str)
where
    T: ResourceVersionInfo,
{
    let version = data
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok().map(|value| value.game_version().to_string()))
        .or_else(|| cache_info.map(|(_, version)| version.clone()));
    let count = data
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok().map(|value| value.item_count()));
    let source = match cache_info.map(|(tag, _)| tag.as_str()) {
        Some("local") => "local",
        _ => "builtin",
    };
    (version, count, source)
}

trait ResourceVersionInfo {
    fn game_version(&self) -> &str;
    fn item_count(&self) -> usize;
}

impl ResourceVersionInfo for LoadedCraftData {
    fn game_version(&self) -> &str {
        &self.data.game_version
    }
    fn item_count(&self) -> usize {
        self.data.counts.items
    }
}

impl ResourceVersionInfo for xiv_companion::WeaponCatalogPackage {
    fn game_version(&self) -> &str {
        &self.game_version
    }
    fn item_count(&self) -> usize {
        self.counts.items
    }
}

impl<T: ResourceVersionInfo> ResourceVersionInfo for Rc<T> {
    fn game_version(&self) -> &str {
        (**self).game_version()
    }
    fn item_count(&self) -> usize {
        (**self).item_count()
    }
}

#[component]
fn ResourceStatusRow(
    label: &'static str,
    description: &'static str,
    version: Option<String>,
    count: Option<usize>,
    count_label: &'static str,
    source: &'static str,
    user_local_configured: bool,
    on_update: EventHandler<MouseEvent>,
    on_reset: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "grid grid-cols-[1fr_auto] items-center gap-3 border-b border-border/50 px-4 py-2.5 last:border-b-0 md:grid-cols-[1fr_auto_1fr]",
            div { class: "min-w-0",
                div { class: "text-sm font-medium", "{label}" }
                div { class: "text-xs text-muted-foreground", "{description}" }
            }
            div { class: "hidden text-xs text-muted-foreground md:block",
                if let Some(version) = version {
                    div { "{version}" }
                    div { "来源 {source}" }
                    if let Some(count) = count {
                        div { "{count} {count_label}" }
                    }
                } else {
                    "加载中…"
                }
            }
            div { class: "flex justify-end md:justify-start gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    disabled: !user_local_configured,
                    onclick: move |event| on_update.call(event),
                    "从本地更新"
                }
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    onclick: move |event| on_reset.call(event),
                    "重置为 builtin"
                }
            }
        }
    }
}

#[component]
fn ResourceStatusStaticRow(
    label: &'static str,
    description: &'static str,
    status: &'static str,
) -> Element {
    rsx! {
        div { class: "grid grid-cols-[1fr_auto] items-center gap-3 border-b border-border/50 px-4 py-2.5 last:border-b-0 md:grid-cols-[1fr_auto_1fr]",
            div { class: "min-w-0",
                div { class: "text-sm font-medium", "{label}" }
                div { class: "text-xs text-muted-foreground", "{description}" }
            }
            div { class: "hidden text-xs text-muted-foreground md:block",
                "{status}"
            }
            div { class: "flex justify-end md:justify-start gap-2" }
        }
    }
}

/// Test section — per-resource test buttons with inline results.
#[component]
fn ResourceTestSection(
    craft_test: Option<(ResourceSource, ResourceTestResult)>,
    icon_test: Option<(ResourceSource, ResourceTestResult)>,
    user_local_disabled: bool,
    on_test_craft: EventHandler<ResourceSource>,
    on_test_icon: EventHandler<ResourceSource>,
) -> Element {
    rsx! {
        div { class: "space-y-3",
            // CraftData test
            ResourceTestRow {
                label: "CraftData",
                description: "配方与物品数据",
                result: craft_test,
                user_local_disabled,
                on_test_builtin: move |_| on_test_craft.call(ResourceSource::Builtin),
                on_test_local: move |_| on_test_craft.call(ResourceSource::UserLocal),
            }

            // ItemIcon test
            ResourceTestRow {
                label: "ItemIcon",
                description: "物品图标 URL（固定 Builtin/API，id=65000）",
                result: icon_test,
                user_local_disabled: true,
                on_test_builtin: move |_| on_test_icon.call(ResourceSource::Builtin),
                on_test_local: move |_| on_test_icon.call(ResourceSource::UserLocal),
            }

            if user_local_disabled {
                div { class: "rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900",
                    div { class: "font-medium", "UserLocal 暂不可测试" }
                    div { class: "mt-1", "请先在「数据来源 → UserLocal」中配置有效的游戏目录路径。" }
                }
            }
        }
    }
}

#[component]
fn ResourceTestRow(
    label: &'static str,
    description: &'static str,
    result: Option<(ResourceSource, ResourceTestResult)>,
    user_local_disabled: bool,
    on_test_builtin: EventHandler<MouseEvent>,
    on_test_local: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "rounded-lg border bg-card p-3",
            div { class: "flex flex-wrap items-center justify-between gap-3",
                div { class: "min-w-0",
                    div { class: "text-sm font-medium", "{label}" }
                    div { class: "text-xs text-muted-foreground", "{description}" }
                }
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        onclick: move |event| on_test_builtin.call(event),
                        "Builtin"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        disabled: user_local_disabled,
                        onclick: move |event| on_test_local.call(event),
                        "UserLocal"
                    }
                }
            }

            // Inline result
            {match &result {
                Some((source, ResourceTestResult::Ok(message))) => rsx! {
                    div { class: "mt-2 flex items-center gap-2 rounded-md bg-emerald-50 px-2.5 py-1.5 text-xs",
                        div { class: "flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-emerald-200 text-emerald-700",
                            span { class: "text-[10px] font-bold", "✓" }
                        }
                        span { class: "font-medium text-emerald-800", "{source_name(*source)}" }
                        span { class: "text-emerald-700", "{message}" }
                    }
                },
                Some((source, ResourceTestResult::Err(diag))) => rsx! {
                    div { class: "mt-2 space-y-1.5",
                        div { class: "flex items-center gap-2 rounded-md bg-red-50 px-2.5 py-1.5 text-xs",
                            div { class: "flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-red-200 text-red-700",
                                span { class: "text-[10px] font-bold", "✗" }
                            }
                            span { class: "font-medium text-red-800", "{source_name(*source)}" }
                            span { class: "text-red-700", "{diag.title}" }
                        }
                        div { class: "rounded-md border border-amber-200 bg-amber-50 px-2.5 py-1.5 text-xs text-amber-900",
                            div { "{diag.summary}" }
                            div { class: "mt-1 text-amber-800", "{diag.action}" }
                            if !diag.details.is_empty() {
                                details { class: "mt-1.5",
                                    summary { class: "cursor-pointer font-medium text-amber-800", "详情" }
                                    div { class: "mt-1 space-y-0.5",
                                        for detail in &diag.details {
                                            div { class: "break-all text-amber-900", "{detail}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                None => rsx! {
                    div { class: "mt-2 text-xs text-muted-foreground", "点击按钮测试数据读取" }
                },
            }}
        }
    }
}

#[component]
fn CraftDataProgressView(progress: CraftDataLoadProgress) -> Element {
    let percent = if progress.total == 0 {
        0.0
    } else {
        ((progress.current as f64 / progress.total as f64) * 100.0).clamp(0.0, 100.0)
    };
    let elapsed = log::format_elapsed(progress.elapsed_ms);
    let progress_width = format!("width: {percent:.1}%");
    rsx! {
        div { class: "rounded-md border bg-muted/30 px-2.5 py-2 text-xs text-muted-foreground",
            div { class: "flex flex-wrap items-center justify-between gap-2",
                div { class: "min-w-0",
                    span { class: "font-medium text-foreground", "{progress.stage}" }
                    if !progress.detail.is_empty() {
                        span { " · {progress.detail}" }
                    }
                }
                div { class: "shrink-0", "{progress.current}/{progress.total} · {elapsed}" }
            }
            div { class: "mt-2 h-1.5 overflow-hidden rounded-full bg-background",
                div {
                    class: cx([
                        "h-full rounded-full transition-all",
                        if progress.done { "bg-emerald-500" } else { "bg-foreground/70" },
                    ]),
                    style: "{progress_width}",
                }
            }
        }
    }
}

/// Compact data status footer showing loaded CraftData and WeaponCatalog info.
#[component]
fn DataStatus(
    craft_data: Resource<Result<LoadedCraftData, String>>,
    weapon_catalog: Resource<Result<Rc<xiv_companion::WeaponCatalogPackage>, String>>,
    craft_progress: Option<CraftDataLoadProgress>,
) -> Element {
    rsx! {
        div { class: "rounded-lg border bg-card p-3 space-y-3",
            // CraftData
            div { class: "space-y-2",
                div { class: "text-xs font-medium text-muted-foreground", "CraftData" }
                match craft_data.read().as_ref() {
                    Some(Ok(loaded)) => {
                        let data = &loaded.data;
                        rsx! {
                            div { class: "space-y-2",
                                div { class: "flex flex-wrap items-center gap-x-4 gap-y-1.5 text-sm",
                                    span { class: "font-medium", "{data.game_version}" }
                                    span { class: "text-muted-foreground",
                                        "来源 "
                                        span { class: "font-medium text-foreground", "{source_name(loaded.source)}" }
                                    }
                                    span { class: "text-muted-foreground",
                                        span { class: "font-medium text-foreground", "{format_integer(data.counts.items as f64)}" }
                                        " 物品"
                                    }
                                    span { class: "text-muted-foreground",
                                        span { class: "font-medium text-foreground", "{format_integer(data.counts.recipes as f64)}" }
                                        " 配方"
                                    }
                                    span { class: "text-muted-foreground",
                                        span { class: "font-medium text-foreground", "{format_integer(data.counts.sources as f64)}" }
                                        " 来源"
                                    }
                                }
                                if loaded.source == ResourceSource::UserLocal {
                                    if let Some(progress) = craft_progress.clone() {
                                        CraftDataProgressView { progress }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(error)) => rsx! {
                        div { class: "flex items-center gap-2 text-sm text-destructive",
                            div { class: "flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-red-100",
                                span { class: "text-[10px] font-bold text-red-600", "✗" }
                            }
                            "{error}"
                        }
                    },
                    None => rsx! {
                        div { class: "flex items-center gap-2 text-sm text-muted-foreground",
                            div { class: "h-4 w-4 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" }
                            "正在加载 CraftData…"
                        }
                    },
                }
            }

            // WeaponCatalog
            div { class: "space-y-2",
                div { class: "text-xs font-medium text-muted-foreground", "WeaponCatalog" }
                match weapon_catalog.read().as_ref() {
                    Some(Ok(catalog)) => {
                        rsx! {
                            div { class: "flex flex-wrap items-center gap-x-4 gap-y-1.5 text-sm",
                                span { class: "font-medium", "{catalog.game_version}" }
                                span { class: "text-muted-foreground",
                                    span { class: "font-medium text-foreground", "{format_integer(catalog.counts.items as f64)}" }
                                    " 武器"
                                }
                            }
                        }
                    },
                    Some(Err(error)) => rsx! {
                        div { class: "flex items-center gap-2 text-sm text-destructive",
                            div { class: "flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-red-100",
                                span { class: "text-[10px] font-bold text-red-600", "✗" }
                            }
                            "{error}"
                        }
                    },
                    None => rsx! {
                        div { class: "flex items-center gap-2 text-sm text-muted-foreground",
                            div { class: "h-4 w-4 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" }
                            "正在加载 WeaponCatalog…"
                        }
                    },
                }
            }
        }
    }
}
