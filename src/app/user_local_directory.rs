use js_sys::JsString;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::app::log;

const APP_STATE_DB_NAME: &str = "xiv-companion-local-source";
const APP_STATE_STORE: &str = "state";
const LEGACY_DIRECTORY_STORE: &str = "directories";
const APP_STATE_DB_VERSION: u32 = 2;
const LOCAL_DIRECTORY_KEY: &str = "user-local-game";
const WINDOW_LOCAL_DIRECTORY_KEY: &str = "__xivCompanionUserLocalDirectory";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthorizedUserLocalDirectory {
    pub(crate) name: String,
    pub(crate) layout: AuthorizedDirectoryLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthorizedDirectoryLayout {
    GameDir,
    InstallRoot,
    MissingSqpack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryPermission {
    Granted,
    Prompt,
    Denied,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectoryPermissionAction {
    UseHandle,
    Reauthorize,
    Repick,
}

/// What to do with a saved directory handle given its current read permission.
///
/// `Unknown` keeps the historical behavior of using the handle: it only appears when the
/// reflection call fails or returns an unexpected state, and the follow-up layout reads are
/// the real usability check (handles without `queryPermission` may still be readable).
fn directory_permission_action(permission: DirectoryPermission) -> DirectoryPermissionAction {
    match permission {
        DirectoryPermission::Granted | DirectoryPermission::Unknown => {
            DirectoryPermissionAction::UseHandle
        }
        DirectoryPermission::Prompt => DirectoryPermissionAction::Reauthorize,
        DirectoryPermission::Denied => DirectoryPermissionAction::Repick,
    }
}

impl DirectoryPermission {
    fn label(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Prompt => "prompt",
            Self::Denied => "denied",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RestoreUserLocalDirectoryOutcome {
    /// The handle was restored with a usable read permission and stored in the window slot.
    Ready(AuthorizedUserLocalDirectory),
    /// A handle exists in IndexedDB but its read permission is `prompt`; the user must
    /// re-authorize it from a real user gesture.
    NeedsReauthorize { name: String },
    /// No directory handle has been saved yet.
    NotSaved,
    /// Restoration failed; the message is user-facing.
    Failed(String),
}

pub(crate) async fn save_current_user_local_directory_handle() -> Result<(), String> {
    let handle = current_window_user_local_directory_handle()?;
    save_user_local_directory_handle(handle).await
}

pub(crate) async fn restore_user_local_directory() -> RestoreUserLocalDirectoryOutcome {
    log::info("local-dir", "restoring saved directory handle");
    let Some(handle) = (match load_user_local_directory_handle().await {
        Ok(handle) => handle,
        Err(error) => return RestoreUserLocalDirectoryOutcome::Failed(error),
    }) else {
        return RestoreUserLocalDirectoryOutcome::NotSaved;
    };
    let name = directory_handle_name(&handle);
    let permission = query_directory_read_permission(&handle).await;
    log::info(
        "local-dir",
        format!("restored handle {name}; permission={}", permission.label()),
    );
    match directory_permission_action(permission) {
        DirectoryPermissionAction::UseHandle => {}
        DirectoryPermissionAction::Reauthorize => {
            return RestoreUserLocalDirectoryOutcome::NeedsReauthorize { name };
        }
        DirectoryPermissionAction::Repick => {
            return RestoreUserLocalDirectoryOutcome::Failed(format!(
                "已恢复保存的目录 {name}，但浏览器读取权限已拒绝；请重新选择游戏目录。"
            ));
        }
    }

    let layout = match detect_authorized_directory_layout(&handle).await {
        Ok(layout) => layout,
        Err(error) => return RestoreUserLocalDirectoryOutcome::Failed(error),
    };
    if layout == AuthorizedDirectoryLayout::MissingSqpack {
        log::warn(
            "local-dir",
            format!("restored handle {name} has no sqpack layout"),
        );
        return RestoreUserLocalDirectoryOutcome::Ready(AuthorizedUserLocalDirectory {
            name,
            layout,
        });
    }

    if let Err(error) = set_window_user_local_directory_handle(&handle) {
        return RestoreUserLocalDirectoryOutcome::Failed(error);
    }
    log::info(
        "local-dir",
        format!("restored UserLocal directory {name}: {layout:?}"),
    );
    RestoreUserLocalDirectoryOutcome::Ready(AuthorizedUserLocalDirectory { name, layout })
}

pub(crate) async fn ensure_window_user_local_directory_handle() -> Result<JsValue, String> {
    if let Ok(handle) = current_window_user_local_directory_handle() {
        return Ok(handle);
    }

    match restore_user_local_directory().await {
        RestoreUserLocalDirectoryOutcome::Ready(directory) => {
            if directory.layout == AuthorizedDirectoryLayout::MissingSqpack {
                return Err("选择的目录下没有 sqpack 或 game\\sqpack".to_string());
            }
        }
        RestoreUserLocalDirectoryOutcome::NeedsReauthorize { name } => {
            return Err(format!(
                "已保存的游戏目录 {name} 需要重新授权读取；请在设置页点击「重新授权读取」。"
            ));
        }
        RestoreUserLocalDirectoryOutcome::NotSaved => {
            return Err("尚未选择本地游戏目录".to_string());
        }
        RestoreUserLocalDirectoryOutcome::Failed(error) => return Err(error),
    }

    current_window_user_local_directory_handle()
}

pub(crate) async fn authorize_user_local_directory() -> Result<AuthorizedUserLocalDirectory, String>
{
    log::info("local-dir", "opening browser directory picker");
    let window = web_sys::window().ok_or_else(|| "当前运行环境没有 window".to_string())?;
    let picker = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("showDirectoryPicker"))
        .map_err(format_js_error)?;
    if !picker.is_function() {
        log::warn("local-dir", "showDirectoryPicker is unavailable");
        return Err("当前运行环境不支持目录选择".to_string());
    }

    let picker = picker
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "目录选择入口不可调用".to_string())?;
    let promise = picker.call0(window.as_ref()).map_err(format_js_error)?;
    let promise = promise
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "目录选择没有返回 Promise".to_string())?;
    let handle = JsFuture::from(promise).await.map_err(format_js_error)?;
    let name = directory_handle_name(&handle);
    let permission = query_directory_read_permission(&handle).await;
    log::info(
        "local-dir",
        format!(
            "selected directory {name}; permission={}",
            permission.label()
        ),
    );
    let layout = detect_authorized_directory_layout(&handle).await?;
    set_window_user_local_directory_handle(&handle)?;
    log::info(
        "local-dir",
        format!("selected UserLocal directory {name}: {layout:?}"),
    );
    Ok(AuthorizedUserLocalDirectory { name, layout })
}

/// Re-request read permission for the saved directory handle and reuse it on success.
///
/// Must be called from a real user gesture: `requestPermission` consumes transient
/// activation, so the synchronous call part is issued before the first `.await`
/// whenever the handle is already in the window slot.
pub(crate) async fn reauthorize_saved_user_local_directory()
-> Result<AuthorizedUserLocalDirectory, String> {
    log::info("local-dir", "re-authorizing saved directory handle");
    if let Ok(handle) = current_window_user_local_directory_handle() {
        return reauthorize_directory_handle(handle).await;
    }
    let Some(handle) = load_user_local_directory_handle().await? else {
        return Err("没有已保存的游戏目录；请重新选择游戏目录。".to_string());
    };
    reauthorize_directory_handle(handle).await
}

async fn reauthorize_directory_handle(
    handle: JsValue,
) -> Result<AuthorizedUserLocalDirectory, String> {
    let name = directory_handle_name(&handle);
    let permission = request_directory_read_permission(&handle).await;
    log::info(
        "local-dir",
        format!(
            "re-authorized handle {name}; permission={}",
            permission.label()
        ),
    );
    if permission != DirectoryPermission::Granted {
        return Err(reauthorize_rejection_message(&name, permission));
    }

    let layout = detect_authorized_directory_layout(&handle).await?;
    set_window_user_local_directory_handle(&handle)?;
    log::info(
        "local-dir",
        format!("re-authorized UserLocal directory {name}: {layout:?}"),
    );
    Ok(AuthorizedUserLocalDirectory { name, layout })
}

fn reauthorize_rejection_message(name: &str, permission: DirectoryPermission) -> String {
    match permission {
        DirectoryPermission::Prompt => format!(
            "已取消对目录 {name} 的读取授权；请再次点击「重新授权读取」，或重新选择游戏目录。"
        ),
        DirectoryPermission::Denied => format!(
            "浏览器已拒绝读取目录 {name}；请在浏览器站点设置中允许本站点访问文件，或重新选择游戏目录。"
        ),
        DirectoryPermission::Unknown => {
            format!("无法确认目录 {name} 的读取权限；请重新选择游戏目录。")
        }
        DirectoryPermission::Granted => "目录读取权限已授予。".to_string(),
    }
}

async fn app_state_db() -> Result<indexed_db::Database<String>, String> {
    log::info("local-dir", "opening IndexedDB for saved directory handle");
    let factory =
        indexed_db::Factory::get().map_err(|error| format!("打开 IndexedDB 失败: {error}"))?;
    factory
        .open(
            APP_STATE_DB_NAME,
            APP_STATE_DB_VERSION,
            |event| async move {
                let db = event.database();
                let names = db.object_store_names();
                if !names.iter().any(|name| name == APP_STATE_STORE) {
                    db.build_object_store(APP_STATE_STORE).create()?;
                }
                if names.iter().any(|name| name == LEGACY_DIRECTORY_STORE) {
                    let handle = event
                        .transaction()
                        .object_store(LEGACY_DIRECTORY_STORE)?
                        .get(&JsString::from(LOCAL_DIRECTORY_KEY))
                        .await?;
                    if let Some(handle) = handle {
                        event
                            .transaction()
                            .object_store(APP_STATE_STORE)?
                            .put_kv(&JsString::from(LOCAL_DIRECTORY_KEY), &handle)
                            .await?;
                    }
                    db.delete_object_store(LEGACY_DIRECTORY_STORE)?;
                }
                Ok(())
            },
        )
        .await
        .map_err(|error| format!("打开本地目录数据库失败: {error}"))
}

async fn save_user_local_directory_handle(handle: JsValue) -> Result<(), String> {
    let name = directory_handle_name(&handle);
    log::info("local-dir", format!("saving directory handle: {name}"));
    let db = app_state_db().await?;
    db.transaction(&[APP_STATE_STORE])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(APP_STATE_STORE)?
                .put_kv(&JsString::from(LOCAL_DIRECTORY_KEY), &handle)
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存目录授权失败: {error}"))?;
    log::info("local-dir", format!("saved directory handle: {name}"));
    Ok(())
}

async fn load_user_local_directory_handle() -> Result<Option<JsValue>, String> {
    let db = app_state_db().await?;
    let handle = db
        .transaction(&[APP_STATE_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(APP_STATE_STORE)?
                .get(&JsString::from(LOCAL_DIRECTORY_KEY))
                .await
        })
        .await
        .map_err(|error| format!("恢复目录授权失败: {error}"))?;
    log::info(
        "local-dir",
        if handle.is_some() {
            "found saved directory handle"
        } else {
            "no saved directory handle"
        },
    );
    Ok(handle)
}

fn directory_handle_name(handle: &JsValue) -> String {
    js_sys::Reflect::get(handle, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "UserLocal".to_string())
}

fn current_window_user_local_directory_handle() -> Result<JsValue, String> {
    let window = web_sys::window().ok_or_else(|| "当前运行环境没有 window".to_string())?;
    let handle = js_sys::Reflect::get(
        window.as_ref(),
        &JsValue::from_str(WINDOW_LOCAL_DIRECTORY_KEY),
    )
    .map_err(format_js_error)?;
    if handle.is_undefined() || handle.is_null() {
        Err("尚未选择游戏目录".to_string())
    } else {
        Ok(handle)
    }
}

fn set_window_user_local_directory_handle(handle: &JsValue) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "当前运行环境没有 window".to_string())?;
    js_sys::Reflect::set(
        window.as_ref(),
        &JsValue::from_str(WINDOW_LOCAL_DIRECTORY_KEY),
        handle,
    )
    .map_err(format_js_error)?;
    Ok(())
}

async fn query_directory_read_permission(handle: &JsValue) -> DirectoryPermission {
    let Ok(method) = js_sys::Reflect::get(handle, &JsValue::from_str("queryPermission")) else {
        return DirectoryPermission::Unknown;
    };
    let Ok(method) = method.dyn_into::<js_sys::Function>() else {
        return DirectoryPermission::Unknown;
    };
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &options,
        &JsValue::from_str("mode"),
        &JsValue::from_str("read"),
    );
    let Ok(promise) = method.call1(handle, &options) else {
        return DirectoryPermission::Unknown;
    };
    let Ok(promise) = promise.dyn_into::<js_sys::Promise>() else {
        return DirectoryPermission::Unknown;
    };
    match JsFuture::from(promise)
        .await
        .ok()
        .and_then(|value| value.as_string())
    {
        Some(value) if value == "granted" => DirectoryPermission::Granted,
        Some(value) if value == "prompt" => DirectoryPermission::Prompt,
        Some(value) if value == "denied" => DirectoryPermission::Denied,
        _ => DirectoryPermission::Unknown,
    }
}

async fn request_directory_read_permission(handle: &JsValue) -> DirectoryPermission {
    let Ok(method) = js_sys::Reflect::get(handle, &JsValue::from_str("requestPermission")) else {
        return DirectoryPermission::Unknown;
    };
    let Ok(method) = method.dyn_into::<js_sys::Function>() else {
        return DirectoryPermission::Unknown;
    };
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &options,
        &JsValue::from_str("mode"),
        &JsValue::from_str("read"),
    );
    let Ok(promise) = method.call1(handle, &options) else {
        return DirectoryPermission::Unknown;
    };
    let Ok(promise) = promise.dyn_into::<js_sys::Promise>() else {
        return DirectoryPermission::Unknown;
    };
    match JsFuture::from(promise)
        .await
        .ok()
        .and_then(|value| value.as_string())
    {
        Some(value) if value == "granted" => DirectoryPermission::Granted,
        Some(value) if value == "prompt" => DirectoryPermission::Prompt,
        Some(value) if value == "denied" => DirectoryPermission::Denied,
        _ => DirectoryPermission::Unknown,
    }
}

async fn detect_authorized_directory_layout(
    handle: &JsValue,
) -> Result<AuthorizedDirectoryLayout, String> {
    if directory_has_child_directory(handle, "sqpack").await? {
        return Ok(AuthorizedDirectoryLayout::GameDir);
    }

    let Some(game_handle) = get_child_directory_handle(handle, "game").await? else {
        return Ok(AuthorizedDirectoryLayout::MissingSqpack);
    };
    if directory_has_child_directory(&game_handle, "sqpack").await? {
        Ok(AuthorizedDirectoryLayout::InstallRoot)
    } else {
        Ok(AuthorizedDirectoryLayout::MissingSqpack)
    }
}

async fn directory_has_child_directory(handle: &JsValue, name: &str) -> Result<bool, String> {
    Ok(get_child_directory_handle(handle, name).await?.is_some())
}

async fn get_child_directory_handle(
    handle: &JsValue,
    name: &str,
) -> Result<Option<JsValue>, String> {
    let method = js_sys::Reflect::get(handle, &JsValue::from_str("getDirectoryHandle"))
        .map_err(format_js_error)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "目录 handle 没有 getDirectoryHandle 方法".to_string())?;
    let promise = match method.call1(handle, &JsValue::from_str(name)) {
        Ok(value) => value,
        Err(error) if js_error_name(&error).as_deref() == Some("NotFoundError") => return Ok(None),
        Err(error) => return Err(format_js_error(error)),
    };
    let promise = promise
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "getDirectoryHandle 没有返回 Promise".to_string())?;
    match JsFuture::from(promise).await {
        Ok(handle) => Ok(Some(handle)),
        Err(error) if js_error_name(&error).as_deref() == Some("NotFoundError") => Ok(None),
        Err(error) => Err(format_js_error(error)),
    }
}

fn js_error_name(error: &JsValue) -> Option<String> {
    js_sys::Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
}

fn format_js_error(error: JsValue) -> String {
    let name = js_error_name(&error);
    if name.as_deref() == Some("AbortError") {
        return "目录选择已取消".to_string();
    }

    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "目录选择失败".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_maps_to_restore_action() {
        let cases = [
            (
                DirectoryPermission::Granted,
                DirectoryPermissionAction::UseHandle,
            ),
            (
                DirectoryPermission::Prompt,
                DirectoryPermissionAction::Reauthorize,
            ),
            (
                DirectoryPermission::Denied,
                DirectoryPermissionAction::Repick,
            ),
            // Reflection failures keep the historical use-the-handle behavior so
            // environments without `queryPermission` do not regress to the picker.
            (
                DirectoryPermission::Unknown,
                DirectoryPermissionAction::UseHandle,
            ),
        ];
        for (permission, action) in cases {
            assert_eq!(directory_permission_action(permission), action);
        }
    }

    #[test]
    fn restore_outcome_variants_are_distinct() {
        let ready = RestoreUserLocalDirectoryOutcome::Ready(AuthorizedUserLocalDirectory {
            name: "game".to_string(),
            layout: AuthorizedDirectoryLayout::GameDir,
        });
        let reauthorize = RestoreUserLocalDirectoryOutcome::NeedsReauthorize {
            name: "game".to_string(),
        };
        assert_ne!(ready, reauthorize);
        assert_ne!(reauthorize, RestoreUserLocalDirectoryOutcome::NotSaved,);
        assert_ne!(
            RestoreUserLocalDirectoryOutcome::NotSaved,
            RestoreUserLocalDirectoryOutcome::Failed("denied".to_string()),
        );
    }

    #[test]
    fn reauthorize_rejection_messages_point_at_the_right_next_action() {
        let prompt = reauthorize_rejection_message("game", DirectoryPermission::Prompt);
        assert!(prompt.contains("重新授权读取"));
        assert!(prompt.contains("重新选择"));

        let denied = reauthorize_rejection_message("game", DirectoryPermission::Denied);
        assert!(denied.contains("重新选择"));
        assert!(!denied.contains("重新授权读取"));

        let unknown = reauthorize_rejection_message("game", DirectoryPermission::Unknown);
        assert!(unknown.contains("重新选择"));
    }
}
