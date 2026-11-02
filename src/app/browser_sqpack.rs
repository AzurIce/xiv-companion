use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Component, Path};

use physis::{Language, Platform, ReadableFile};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
pub use xiv_companion_data::resource::MemoryResource as InMemoryPhysisResource;
use xiv_companion_data::sqpack::{normalize_game_path, sqpack_index_candidates_for_path};

use crate::app::load_progress::{CraftDataLoadProgress, report_craft_data_progress};
use crate::app::log;
use crate::app::user_local_directory::ensure_window_user_local_directory_handle;

const SQPACK_READ_WINDOW: u64 = 64 * 1024 * 1024;

pub struct BrowserSqPack {
    root: JsValue,
    game_prefix: Option<&'static str>,
    index_cache: HashMap<String, physis::sqpack::SqPackIndex>,
    missing_index_cache: HashSet<String>,
    entry_location_cache: HashMap<String, (String, String)>,
}

impl BrowserSqPack {
    pub async fn from_window_handle() -> Result<Self, String> {
        log::debug(
            "sqpack",
            "opening BrowserSqPack from selected directory handle",
        );
        let root = ensure_window_user_local_directory_handle().await?;
        Self::from_handle(root).await
    }

    pub async fn from_handle(root: JsValue) -> Result<Self, String> {
        if root.is_undefined() || root.is_null() {
            return Err("尚未选择本地游戏目录".to_string());
        }

        let game_prefix = if directory_has_child_directory(&root, "sqpack").await? {
            log::info("sqpack", "directory layout: game directory with sqpack");
            None
        } else if let Some(game) = get_child_directory_handle(&root, "game").await? {
            if directory_has_child_directory(&game, "sqpack").await? {
                log::info("sqpack", "directory layout: install root with game/sqpack");
                Some("game")
            } else {
                return Err("选择的目录下没有 sqpack 或 game\\sqpack".to_string());
            }
        } else {
            return Err("选择的目录下没有 sqpack 或 game\\sqpack".to_string());
        };

        Ok(Self {
            root,
            game_prefix,
            index_cache: HashMap::new(),
            missing_index_cache: HashSet::new(),
            entry_location_cache: HashMap::new(),
        })
    }

    pub async fn read_game_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        self.read_game_file_with_window(path, SQPACK_READ_WINDOW)
            .await
    }

    pub async fn game_version(&self) -> Result<String, String> {
        let bytes = self.read_sqpack_file_all("ffxivgame.ver").await?;
        String::from_utf8(bytes)
            .map(|version| version.trim().to_string())
            .map_err(|error| format!("读取本地游戏版本失败: {error}"))
    }

    /// 尝试读取候选资源路径；未命中 SqPack 索引时不输出 warn。
    ///
    /// 模型材质/贴图解析会按多个可能路径探测，前几个候选缺失是正常情况。
    pub async fn try_read_game_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        self.read_game_file_with_window_and_log_mode(path, SQPACK_READ_WINDOW, false)
            .await
    }

    /// `None` means no entry was found in the available indices. Permission,
    /// index parsing, data-file and decompression failures remain errors.
    pub async fn try_read_optional_game_file(
        &mut self,
        path: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        self.read_optional_game_file_with_window(path, SQPACK_READ_WINDOW, false)
            .await
    }

    pub async fn read_game_file_with_window(
        &mut self,
        path: &str,
        read_window: u64,
    ) -> Result<Vec<u8>, String> {
        self.read_game_file_with_window_and_log_mode(path, read_window, true)
            .await
    }

    async fn read_game_file_with_window_and_log_mode(
        &mut self,
        path: &str,
        read_window: u64,
        warn_missing: bool,
    ) -> Result<Vec<u8>, String> {
        self.read_optional_game_file_with_window(path, read_window, warn_missing)
            .await?
            .ok_or_else(|| format!("本地 SqPack 索引中没有 {path}"))
    }

    async fn read_optional_game_file_with_window(
        &mut self,
        path: &str,
        read_window: u64,
        warn_missing: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        log::debug(
            "sqpack",
            format!("reading game file: {path} (read_window={read_window} bytes)"),
        );
        let game_path = normalize_game_path(path);
        let Some((index_path, dat_base)) =
            self.find_entry_location(&game_path, warn_missing).await?
        else {
            return Ok(None);
        };
        let index = self.index_cache.get(&index_path).ok_or_else(|| {
            format!("internal error: index {index_path} was not cached while reading {path}")
        })?;
        let entry = index
            .find_entry(&game_path)
            .ok_or_else(|| format!("本地 SqPack 没有文件 {path}"))?;
        let dat_path = format!("{dat_base}.dat{}", entry.data.data_file_id);
        let offset = entry.data.offset;
        let bytes = self
            .read_sqpack_file_slice(&dat_path, offset, read_window)
            .await
            .map_err(|error| format!("读取 {dat_path} 的偏移 {offset} 失败: {error}"))?;
        let mut cursor = Cursor::new(bytes);
        let decoded = xiv_companion_data::sqpack::read_entry(Platform::Win32, &mut cursor, 0)
            .map_err(|error| format!("解包本地 SqPack 文件失败: {path}: {error}"))?;
        log::debug(
            "sqpack",
            format!(
                "read game file: {path} from {dat_path} (read_window={read_window} bytes, slice={} bytes, decoded={} bytes)",
                cursor.get_ref().len(),
                decoded.len(),
            ),
        );
        Ok(Some(decoded))
    }

    pub async fn craft_data_cache_fingerprint(&self) -> Result<String, String> {
        self.exd_index_cache_fingerprint("craft-data", "CraftData")
            .await
    }

    pub async fn weapon_catalog_cache_fingerprint(&self) -> Result<String, String> {
        self.exd_index_cache_fingerprint("weapon-catalog-v2", "WeaponCatalog")
            .await
    }

    async fn exd_index_cache_fingerprint(
        &self,
        cache_name: &str,
        label: &str,
    ) -> Result<String, String> {
        let mut parts = Vec::new();
        for path in [
            "sqpack/ffxiv/0a0000.win32.index",
            "sqpack/ffxiv/0a0000.win32.index2",
        ] {
            if let Ok(file) = self.get_file(path).await {
                let size = js_sys::Reflect::get(&file, &JsValue::from_str("size"))
                    .ok()
                    .and_then(|value| value.as_f64())
                    .unwrap_or_default();
                let last_modified = js_sys::Reflect::get(&file, &JsValue::from_str("lastModified"))
                    .ok()
                    .and_then(|value| value.as_f64())
                    .unwrap_or_default();
                parts.push(format!("{path}:{size:.0}:{last_modified:.0}"));
            }
        }

        if parts.is_empty() {
            Err(format!(
                "无法读取 EXD SqPack 索引元数据，不能校验本地 {label} 缓存"
            ))
        } else {
            Ok(format!(
                "browser-sqpack-{cache_name}-v1|{}",
                parts.join("|")
            ))
        }
    }

    pub async fn preload_craft_data_resource(&mut self) -> Result<InMemoryPhysisResource, String> {
        let start_ms = log::now_ms();
        let plan = craft_data_sheet_plan();
        let total = plan.len() as u32;
        log::info(
            "sqpack",
            format!("preloading CraftData sheets ({total} sheets)"),
        );
        let mut files = HashMap::new();
        for (index, (sheet, languages)) in plan.into_iter().enumerate() {
            report_craft_data_progress(Some(CraftDataLoadProgress {
                stage: "预加载本地表".to_string(),
                detail: sheet.to_string(),
                current: index as u32 + 1,
                total,
                elapsed_ms: log::elapsed_ms(start_ms),
                done: false,
            }));
            self.preload_sheet(&mut files, sheet, &languages).await?;
        }
        let elapsed_ms = log::elapsed_ms(start_ms);
        log::info(
            "sqpack",
            format!(
                "preloaded CraftData sheets: {} files in {}",
                files.len(),
                log::format_elapsed(elapsed_ms),
            ),
        );
        report_craft_data_progress(Some(CraftDataLoadProgress {
            stage: "预加载完成".to_string(),
            detail: format!("{} 个文件", files.len()),
            current: total,
            total,
            elapsed_ms,
            done: false,
        }));
        Ok(InMemoryPhysisResource::new(files))
    }

    pub async fn preload_weapon_catalog_resource(
        &mut self,
    ) -> Result<InMemoryPhysisResource, String> {
        let start_ms = log::now_ms();
        log::info("sqpack", "preloading WeaponCatalog Item and Stain sheets");
        let mut files = HashMap::new();
        self.preload_sheet(&mut files, "Item", &[Language::ChineseSimplified])
            .await?;
        self.preload_sheet(&mut files, "Stain", &[Language::ChineseSimplified])
            .await?;
        log::info(
            "sqpack",
            format!(
                "preloaded WeaponCatalog sheets: {} files in {}",
                files.len(),
                log::format_elapsed(log::elapsed_ms(start_ms)),
            ),
        );
        Ok(InMemoryPhysisResource::new(files))
    }

    pub async fn preload_collection_catalog_resource(
        &mut self,
    ) -> Result<InMemoryPhysisResource, String> {
        let start_ms = log::now_ms();
        log::info("sqpack", "preloading CollectionCatalog sheets");
        let mut files = HashMap::new();
        self.preload_sheet(&mut files, "Item", &[Language::ChineseSimplified])
            .await?;
        self.preload_sheet(
            &mut files,
            "FittingShopItemSet",
            &[Language::ChineseSimplified],
        )
        .await?;
        self.preload_sheet(&mut files, "MirageStoreSetItem", &[Language::None])
            .await?;
        self.preload_sheet(
            &mut files,
            "ClassJobCategory",
            &[Language::ChineseSimplified],
        )
        .await?;
        self.preload_sheet(&mut files, "ItemAction", &[Language::None])
            .await?;
        log::info(
            "sqpack",
            format!(
                "preloaded CollectionCatalog sheets: {} files in {}",
                files.len(),
                log::format_elapsed(log::elapsed_ms(start_ms)),
            ),
        );
        Ok(InMemoryPhysisResource::new(files))
    }

    async fn preload_sheet(
        &mut self,
        files: &mut HashMap<String, Vec<u8>>,
        sheet: &str,
        languages: &[Language],
    ) -> Result<(), String> {
        log::debug("sqpack", format!("preloading sheet: {sheet}"));
        let name = sheet.to_ascii_lowercase();
        let exh_path = format!("exd/{name}.exh");
        if !files.contains_key(&exh_path) {
            let bytes = self.read_game_file(&exh_path).await?;
            files.insert(exh_path.clone(), bytes);
        }

        let exh = physis::exh::EXH::from_existing(
            Platform::Win32,
            files
                .get(&exh_path)
                .ok_or_else(|| format!("internal error: missing cached {exh_path}"))?,
        )
        .map_err(|error| format!("解析 {sheet}.exh 失败: {error}"))?;

        for language in languages {
            for page in &exh.pages {
                let exd_name = physis::exd::EXD::calculate_filename(sheet, *language, page);
                let path = format!("exd/{exd_name}").to_ascii_lowercase();
                if files.contains_key(&path) {
                    continue;
                }
                match self.read_game_file(&path).await {
                    Ok(bytes) => {
                        log::debug("sqpack", format!("loaded {path} ({} bytes)", bytes.len()));
                        files.insert(path, bytes);
                    }
                    Err(error) if *language != Language::None => {
                        // Some sheets expose language metadata but are absent for a locale in older clients.
                        // The runtime loader can still fall back to Language::None if that was preloaded.
                        if !languages.contains(&Language::None) {
                            return Err(error);
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(())
    }

    async fn find_entry_location(
        &mut self,
        path: &str,
        warn_missing: bool,
    ) -> Result<Option<(String, String)>, String> {
        let normalized_path = normalize_game_path(path);
        if let Some(location) = self.entry_location_cache.get(&normalized_path) {
            return Ok(Some(location.clone()));
        }

        let start_ms = log::now_ms();
        let candidates = sqpack_index_candidates_for_path(&normalized_path)
            .ok_or_else(|| format!("不支持的本地 SqPack 资源路径: {path}"))?;

        let mut failures = Vec::new();
        for candidate in &candidates {
            match self.load_index_file(&candidate.index_file).await {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            }

            if self
                .index_cache
                .get(&candidate.index_file)
                .is_some_and(|index| index.exists(&normalized_path))
            {
                let location = (candidate.index_file.clone(), candidate.dat_base.clone());
                self.entry_location_cache
                    .insert(normalized_path.clone(), location.clone());
                log::debug(
                    "sqpack",
                    format!(
                        "resolved SqPack index for {path}: {} in {} ({} candidates)",
                        candidate.index_file,
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                        candidates.len(),
                    ),
                );
                return Ok(Some(location));
            }
        }

        if !failures.is_empty() {
            return Err(format!(
                "无法查找本地 SqPack 资源 {path}:\n{}",
                failures.join("\n")
            ));
        }
        if warn_missing {
            log::warn("sqpack", format!("missing in SqPack index: {path}"));
        }
        Ok(None)
    }

    async fn load_index_file(&mut self, index_file: &str) -> Result<bool, String> {
        if self.index_cache.contains_key(index_file) {
            return Ok(true);
        }
        if self.missing_index_cache.contains(index_file) {
            return Ok(false);
        }

        let Some(file) = self
            .get_optional_file(index_file)
            .await
            .map_err(|error| format!("读取 SqPack 索引 {index_file} 失败: {error}"))?
        else {
            self.missing_index_cache.insert(index_file.to_string());
            return Ok(false);
        };
        let bytes = read_file_all(&file)
            .await
            .map_err(|error| format!("读取 SqPack 索引 {index_file} 失败: {error}"))?;
        let index = physis::sqpack::SqPackIndex::from_existing(Platform::Win32, &bytes)
            .map_err(|error| format!("解析 SqPack 索引 {index_file} 失败: {error}"))?;
        self.index_cache.insert(index_file.to_string(), index);
        Ok(true)
    }

    async fn read_sqpack_file_all(&self, path: &str) -> Result<Vec<u8>, String> {
        let file = self.get_file(path).await?;
        read_file_all(&file).await
    }

    async fn read_sqpack_file_slice(
        &self,
        path: &str,
        start: u64,
        len: u64,
    ) -> Result<Vec<u8>, String> {
        let file = self.get_file(path).await?;
        let start_f = u64_to_f64(start)?;
        let end_f = u64_to_f64(start.saturating_add(len))?;
        let slice = call2(
            &file,
            "slice",
            &JsValue::from_f64(start_f),
            &JsValue::from_f64(end_f),
        )?;
        let promise = call0(&slice, "arrayBuffer")?;
        let buffer = JsFuture::from(promise).await.map_err(format_js_error)?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    }

    async fn get_file(&self, path: &str) -> Result<JsValue, String> {
        self.get_optional_file(path)
            .await?
            .ok_or_else(|| format!("找不到文件: {path}"))
    }

    async fn get_optional_file(&self, path: &str) -> Result<Option<JsValue>, String> {
        let mut current = self.root.clone();
        if let Some(prefix) = self.game_prefix {
            let Some(directory) = get_child_directory_handle(&current, prefix).await? else {
                return Ok(None);
            };
            current = directory;
        }

        let components = Path::new(path).components().collect::<Vec<_>>();
        for component in &components[..components.len().saturating_sub(1)] {
            let Component::Normal(name) = component else {
                return Err(format!("非法路径组件: {path}"));
            };
            let Some(directory) =
                get_child_directory_handle(&current, &name.to_string_lossy()).await?
            else {
                return Ok(None);
            };
            current = directory;
        }

        let Some(Component::Normal(filename)) = components.last() else {
            return Err(format!("非法文件路径: {path}"));
        };
        let Some(handle) = get_child_file_handle(&current, &filename.to_string_lossy()).await?
        else {
            return Ok(None);
        };
        let promise = call0(&handle, "getFile")?;
        optional_handle_result(JsFuture::from(promise).await)
            .map_err(|error| format!("getFile {path}: {error}"))
    }
}

fn craft_data_sheet_plan() -> Vec<(&'static str, Vec<Language>)> {
    vec![
        ("Item", vec![Language::ChineseSimplified]),
        ("Recipe", vec![Language::None]),
        ("RecipeLevelTable", vec![Language::None]),
        ("SecretRecipeBook", vec![Language::ChineseSimplified]),
        ("Action", vec![Language::ChineseSimplified]),
        ("CraftAction", vec![Language::ChineseSimplified]),
        ("GeneralAction", vec![Language::ChineseSimplified]),
        ("GatheringItem", vec![Language::None]),
        ("FishingSpot", vec![Language::ChineseSimplified]),
        ("GilShop", vec![Language::ChineseSimplified]),
        ("GilShopItem", vec![Language::None]),
        ("SpecialShop", vec![Language::ChineseSimplified]),
    ]
}

async fn get_child_directory_handle(
    handle: &JsValue,
    name: &str,
) -> Result<Option<JsValue>, String> {
    get_child_handle(handle, "getDirectoryHandle", name).await
}

async fn get_child_file_handle(handle: &JsValue, name: &str) -> Result<Option<JsValue>, String> {
    get_child_handle(handle, "getFileHandle", name).await
}

async fn get_child_handle(
    handle: &JsValue,
    method: &str,
    name: &str,
) -> Result<Option<JsValue>, String> {
    let promise = call1(handle, method, &JsValue::from_str(name))?;
    optional_handle_result(JsFuture::from(promise).await)
        .map_err(|error| format!("{method} {name}: {error}"))
}

fn optional_handle_result(result: Result<JsValue, JsValue>) -> Result<Option<JsValue>, String> {
    match result {
        Ok(handle) => Ok(Some(handle)),
        Err(error) => {
            let name = js_sys::Reflect::get(&error, &JsValue::from_str("name"))
                .ok()
                .and_then(|name| name.as_string());
            if name.as_deref() == Some("NotFoundError") {
                Ok(None)
            } else {
                Err(format_js_error(error))
            }
        }
    }
}

async fn directory_has_child_directory(handle: &JsValue, name: &str) -> Result<bool, String> {
    Ok(get_child_directory_handle(handle, name).await?.is_some())
}

async fn read_file_all(file: &JsValue) -> Result<Vec<u8>, String> {
    let promise = call0(file, "arrayBuffer")?;
    let buffer = JsFuture::from(promise).await.map_err(format_js_error)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn call0(target: &JsValue, method: &str) -> Result<js_sys::Promise, String> {
    let value =
        js_sys::Reflect::get(target, &JsValue::from_str(method)).map_err(format_js_error)?;
    let function = value
        .dyn_into::<js_sys::Function>()
        .map_err(|_| format!("{method} 不是函数"))?;
    function
        .call0(target)
        .map_err(format_js_error)?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "调用没有返回 Promise".to_string())
}

fn call1(target: &JsValue, method: &str, arg: &JsValue) -> Result<js_sys::Promise, String> {
    let value =
        js_sys::Reflect::get(target, &JsValue::from_str(method)).map_err(format_js_error)?;
    let function = value
        .dyn_into::<js_sys::Function>()
        .map_err(|_| format!("{method} 不是函数"))?;
    function
        .call1(target, arg)
        .map_err(format_js_error)?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "调用没有返回 Promise".to_string())
}

fn call2(target: &JsValue, method: &str, a: &JsValue, b: &JsValue) -> Result<JsValue, String> {
    let value =
        js_sys::Reflect::get(target, &JsValue::from_str(method)).map_err(format_js_error)?;
    let function = value
        .dyn_into::<js_sys::Function>()
        .map_err(|_| format!("{method} 不是函数"))?;
    function.call2(target, a, b).map_err(format_js_error)
}

fn format_js_error(error: JsValue) -> String {
    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "浏览器文件系统调用失败".to_string())
}

fn u64_to_f64(value: u64) -> Result<f64, String> {
    if u64::BITS - value.leading_zeros() >= f64::MANTISSA_DIGITS {
        return Err(format!("offset {value} 不能安全转换为 f64"));
    }
    Ok(value as f64)
}

#[cfg(test)]
mod sqpack_index_candidate_tests {
    use super::*;

    #[test]
    fn exd_paths_use_single_base_exd_index_pair() {
        let candidates = sqpack_index_candidates_for_path("exd/item.exh").unwrap();

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.index_file.as_str())
                .collect::<Vec<_>>(),
            vec![
                "sqpack/ffxiv/0a0000.win32.index",
                "sqpack/ffxiv/0a0000.win32.index2",
            ]
        );
    }

    #[test]
    fn weapon_paths_use_single_base_chara_index_pair() {
        let candidates = sqpack_index_candidates_for_path(
            "chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl",
        )
        .unwrap();

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].index_file, "sqpack/ffxiv/040000.win32.index");
        assert_eq!(candidates[1].index_file, "sqpack/ffxiv/040000.win32.index2");
    }

    #[test]
    fn ui_icon_paths_use_single_base_ui_index_pair() {
        let candidates = sqpack_index_candidates_for_path("ui/icon/000000/000001.tex").unwrap();

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].index_file, "sqpack/ffxiv/060000.win32.index");
    }

    #[test]
    fn background_expansion_paths_keep_full_chunk_fallback() {
        let candidates =
            sqpack_index_candidates_for_path("bg/ex2/01_gyr_g3/fld/g3fb/level/planner.lgb")
                .unwrap();

        assert_eq!(candidates[0].index_file, "sqpack/ex2/020200.win32.index");
        assert_eq!(candidates[1].index_file, "sqpack/ex2/020200.win32.index2");
        assert!(candidates.len() > 2);
    }

    #[test]
    fn non_expansion_second_path_segment_stays_in_base_repository() {
        let candidates =
            sqpack_index_candidates_for_path("chara/weapon/w0001/model/example.mdl").unwrap();

        assert_eq!(candidates[0].index_file, "sqpack/ffxiv/040000.win32.index");
    }
}
