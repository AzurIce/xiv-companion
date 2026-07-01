use js_sys::JsString;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::app::log;

const LOCAL_DIRECTORY_DB: &str = "xiv-companion-local-source";
const LOCAL_DIRECTORY_STORE: &str = "directories";
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
enum DirectoryPermission {
    Granted,
    Prompt,
    Denied,
    Unknown,
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

pub(crate) async fn save_current_user_local_directory_handle() -> Result<(), String> {
    let handle = current_window_user_local_directory_handle()?;
    save_user_local_directory_handle(handle).await
}

pub(crate) async fn restore_user_local_directory()
-> Result<Option<AuthorizedUserLocalDirectory>, String> {
    log::info("local-dir", "restoring saved directory handle");
    let Some(handle) = load_user_local_directory_handle().await? else {
        return Ok(None);
    };
    let name = directory_handle_name(&handle);
    let permission = query_directory_read_permission(&handle).await;
    log::info(
        "local-dir",
        format!("restored handle {name}; permission={}", permission.label()),
    );
    if !matches!(
        permission,
        DirectoryPermission::Granted | DirectoryPermission::Unknown
    ) {
        return Err(format!(
            "已恢复保存的目录 {name}，但浏览器读取权限是 {}；请重新选择游戏目录。",
            permission.label()
        ));
    }

    let layout = detect_authorized_directory_layout(&handle).await?;
    if layout == AuthorizedDirectoryLayout::MissingSqpack {
        log::warn(
            "local-dir",
            format!("restored handle {name} has no sqpack layout"),
        );
        return Ok(Some(AuthorizedUserLocalDirectory { name, layout }));
    }

    set_window_user_local_directory_handle(&handle)?;
    log::info(
        "local-dir",
        format!("restored UserLocal directory {name}: {layout:?}"),
    );
    Ok(Some(AuthorizedUserLocalDirectory { name, layout }))
}

pub(crate) async fn ensure_window_user_local_directory_handle() -> Result<JsValue, String> {
    if let Ok(handle) = current_window_user_local_directory_handle() {
        return Ok(handle);
    }

    let Some(directory) = restore_user_local_directory().await? else {
        return Err("尚未选择本地游戏目录".to_string());
    };
    if directory.layout == AuthorizedDirectoryLayout::MissingSqpack {
        return Err("选择的目录下没有 sqpack 或 game\\sqpack".to_string());
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

async fn local_directory_db() -> Result<indexed_db::Database<String>, String> {
    log::info("local-dir", "opening IndexedDB for saved directory handle");
    let factory =
        indexed_db::Factory::get().map_err(|error| format!("打开 IndexedDB 失败: {error}"))?;
    factory
        .open(LOCAL_DIRECTORY_DB, 1, |event| async move {
            let db = event.database();
            db.build_object_store(LOCAL_DIRECTORY_STORE).create()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("打开本地目录数据库失败: {error}"))
}

async fn save_user_local_directory_handle(handle: JsValue) -> Result<(), String> {
    let name = directory_handle_name(&handle);
    log::info("local-dir", format!("saving directory handle: {name}"));
    let db = local_directory_db().await?;
    db.transaction(&[LOCAL_DIRECTORY_STORE])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(LOCAL_DIRECTORY_STORE)?
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
    let db = local_directory_db().await?;
    let handle = db
        .transaction(&[LOCAL_DIRECTORY_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(LOCAL_DIRECTORY_STORE)?
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
