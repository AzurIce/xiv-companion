use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path};
use std::sync::Arc;

use binrw::{BinRead, BinReaderExt, BinWrite, Endian, VecArgs, binread, binrw};
use flate2::read::DeflateDecoder;
use physis::model::ModelFileHeader;
use physis::resource::Resource;
use physis::{ByteBuffer, Language, Platform, ReadableFile};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::app::load_progress::{CraftDataLoadProgress, report_craft_data_progress};
use crate::app::log;
use crate::app::user_local_directory::ensure_window_user_local_directory_handle;

const SQPACK_READ_WINDOW: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct InMemoryPhysisResource {
    files: Arc<HashMap<String, Vec<u8>>>,
}

impl InMemoryPhysisResource {
    pub fn new(files: HashMap<String, Vec<u8>>) -> Self {
        Self {
            files: Arc::new(files),
        }
    }
}

impl Resource for InMemoryPhysisResource {
    fn read(&mut self, path: &str) -> Option<ByteBuffer> {
        self.files.get(&path.to_ascii_lowercase()).cloned()
    }

    fn exists(&mut self, path: &str) -> bool {
        self.files.contains_key(&path.to_ascii_lowercase())
    }

    fn platform(&self) -> Platform {
        Platform::Win32
    }
}

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

        let game_prefix = if directory_has_child_directory(&root, "sqpack").await {
            log::info("sqpack", "directory layout: game directory with sqpack");
            None
        } else if let Some(game) = get_child_directory_handle(&root, "game").await {
            if directory_has_child_directory(&game, "sqpack").await {
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
        log::debug(
            "sqpack",
            format!("reading game file: {path} (read_window={read_window} bytes)"),
        );
        let game_path = normalize_game_path(path);
        let (index_path, dat_base) = self.find_entry_location(&game_path, warn_missing).await?;
        let index = self.index_cache.get(&index_path).ok_or_else(|| {
            format!("internal error: index {index_path} was not cached while reading {path}")
        })?;
        let entry = index
            .find_entry(&game_path)
            .ok_or_else(|| format!("本地 SqPack 没有文件 {path}"))?;
        let dat_path = format!("{dat_base}.dat{}", entry.data_file_id);
        let bytes = self
            .read_sqpack_file_slice(&dat_path, entry.offset, read_window)
            .await?;
        let mut cursor = Cursor::new(bytes);
        let decoded = read_sqpack_entry_from_reader(&mut cursor)
            .ok_or_else(|| format!("解包本地 SqPack 文件失败: {path}"))?;
        log::debug(
            "sqpack",
            format!(
                "read game file: {path} from {dat_path} (read_window={read_window} bytes, slice={} bytes, decoded={} bytes)",
                cursor.get_ref().len(),
                decoded.len(),
            ),
        );
        Ok(decoded)
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
        log::info("sqpack", "preloading WeaponCatalog Item sheet");
        let mut files = HashMap::new();
        self.preload_sheet(&mut files, "Item", &[Language::ChineseSimplified])
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
        .ok_or_else(|| format!("解析 {sheet}.exh 失败"))?;

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
    ) -> Result<(String, String), String> {
        let normalized_path = normalize_game_path(path);
        if let Some(location) = self.entry_location_cache.get(&normalized_path) {
            return Ok(location.clone());
        }

        let start_ms = log::now_ms();
        let candidates = sqpack_index_candidates_for_path(&normalized_path)
            .ok_or_else(|| format!("不支持的本地 SqPack 资源路径: {path}"))?;

        for candidate in &candidates {
            if !self.load_index_file(&candidate.index_file).await {
                continue;
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
                return Ok(location);
            }
        }

        if warn_missing {
            log::warn("sqpack", format!("missing in SqPack index: {path}"));
        }
        Err(format!("本地 SqPack 索引中没有 {path}"))
    }

    async fn load_index_file(&mut self, index_file: &str) -> bool {
        if self.index_cache.contains_key(index_file) {
            return true;
        }
        if self.missing_index_cache.contains(index_file) {
            return false;
        }

        let Ok(bytes) = self.read_sqpack_file_all(index_file).await else {
            self.missing_index_cache.insert(index_file.to_string());
            return false;
        };
        match physis::sqpack::SqPackIndex::read_options(&mut Cursor::new(bytes), Endian::Little, ())
        {
            Ok(index) => {
                self.index_cache.insert(index_file.to_string(), index);
                true
            }
            Err(error) => {
                log::warn(
                    "sqpack",
                    format!("failed to parse SqPack index {index_file}: {error}"),
                );
                self.missing_index_cache.insert(index_file.to_string());
                false
            }
        }
    }

    async fn read_sqpack_file_all(&self, path: &str) -> Result<Vec<u8>, String> {
        let file = self.get_file(path).await?;
        let promise = call0(&file, "arrayBuffer")?;
        let buffer = JsFuture::from(promise).await.map_err(format_js_error)?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
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
        let mut current = self.root.clone();
        if let Some(prefix) = self.game_prefix {
            current = get_child_directory_handle(&current, prefix)
                .await
                .ok_or_else(|| format!("找不到目录 {prefix}"))?;
        }

        let components = Path::new(path).components().collect::<Vec<_>>();
        for component in &components[..components.len().saturating_sub(1)] {
            let Component::Normal(name) = component else {
                return Err(format!("非法路径组件: {path}"));
            };
            current = get_child_directory_handle(&current, &name.to_string_lossy())
                .await
                .ok_or_else(|| format!("找不到目录: {}", name.to_string_lossy()))?;
        }

        let Some(Component::Normal(filename)) = components.last() else {
            return Err(format!("非法文件路径: {path}"));
        };
        let handle = get_child_file_handle(&current, &filename.to_string_lossy())
            .await
            .ok_or_else(|| format!("找不到文件: {path}"))?;
        let promise = call0(&handle, "getFile")?;
        JsFuture::from(promise).await.map_err(format_js_error)
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct SqPackIndexCandidate {
    index_file: String,
    dat_base: String,
}

fn sqpack_index_candidates_for_path(path: &str) -> Option<Vec<SqPackIndexCandidate>> {
    let category = category_for_path(path)?;
    let repo = repository_for_path(path);
    let expansion = expansion_id_for_repository(&repo);
    let allow_full_scan = category_uses_variable_chunks(category);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    push_sqpack_index_candidates(&mut candidates, &mut seen, &repo, category, expansion, 0);
    if allow_full_scan {
        for chunk in 1..=254_u8 {
            push_sqpack_index_candidates(
                &mut candidates,
                &mut seen,
                &repo,
                category,
                expansion,
                chunk,
            );
        }
    }

    Some(candidates)
}

fn push_sqpack_index_candidates(
    candidates: &mut Vec<SqPackIndexCandidate>,
    seen: &mut HashSet<String>,
    repo: &str,
    category: u8,
    expansion: u8,
    chunk: u8,
) {
    let stem = format!("{category:02x}{expansion:02}{chunk:02}");
    for extension in ["index", "index2"] {
        let index_file = format!("sqpack/{repo}/{stem}.win32.{extension}");
        if !seen.insert(index_file.clone()) {
            continue;
        }
        candidates.push(SqPackIndexCandidate {
            dat_base: format!("sqpack/{repo}/{stem}.win32"),
            index_file,
        });
    }
}

fn category_uses_variable_chunks(category: u8) -> bool {
    matches!(category, 0x02 | 0x03 | 0x07 | 0x0c)
}

fn normalize_game_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn category_for_path(path: &str) -> Option<u8> {
    match path.split('/').next()? {
        "common" => Some(0x00),
        "bgcommon" => Some(0x01),
        "bg" => Some(0x02),
        "cut" => Some(0x03),
        "chara" => Some(0x04),
        "shader" => Some(0x05),
        "ui" => Some(0x06),
        "sound" => Some(0x07),
        "vfx" => Some(0x08),
        "ui_script" => Some(0x09),
        "exd" => Some(0x0a),
        "game_script" => Some(0x0b),
        "music" => Some(0x0c),
        "sqpack_test" => Some(0x12),
        "debug" => Some(0x13),
        _ => None,
    }
}

fn repository_for_path(path: &str) -> String {
    path.split('/')
        .nth(1)
        .filter(|part| expansion_number_from_repository(part).is_some())
        .map(ToString::to_string)
        .unwrap_or_else(|| "ffxiv".to_string())
}

fn expansion_id_for_repository(repository: &str) -> u8 {
    expansion_number_from_repository(repository).unwrap_or(0)
}

fn expansion_number_from_repository(repository: &str) -> Option<u8> {
    let number = repository.strip_prefix("ex")?;
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

async fn get_child_directory_handle(handle: &JsValue, name: &str) -> Option<JsValue> {
    let promise = call1(handle, "getDirectoryHandle", &JsValue::from_str(name)).ok()?;
    JsFuture::from(promise).await.ok()
}

async fn get_child_file_handle(handle: &JsValue, name: &str) -> Option<JsValue> {
    let promise = call1(handle, "getFileHandle", &JsValue::from_str(name)).ok()?;
    JsFuture::from(promise).await.ok()
}

async fn directory_has_child_directory(handle: &JsValue, name: &str) -> bool {
    get_child_directory_handle(handle, name).await.is_some()
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

#[binrw]
#[brw(repr = i32)]
#[derive(Debug, PartialEq, Eq)]
enum FileType {
    Empty = 1,
    Standard,
    Model,
    Texture,
}

#[binrw]
#[derive(Debug)]
struct StandardFileBlock {
    #[brw(pad_before = 8)]
    num_blocks: u32,
}

#[binrw]
#[derive(Debug)]
struct TextureLodBlock {
    compressed_offset: u32,
    compressed_size: u32,
    decompressed_size: u32,
    block_offset: u32,
    block_count: u32,
}

trait AnyNumberType<'a>:
    BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()> + std::ops::AddAssign + Copy + Default + 'static
{
}

impl<'a, T> AnyNumberType<'a> for T where
    T: BinRead<Args<'a> = ()>
        + BinWrite<Args<'a> = ()>
        + std::ops::AddAssign
        + Copy
        + Default
        + 'static
{
}

#[binrw]
#[derive(Debug)]
struct ModelMemorySizes<T: for<'a> AnyNumberType<'a>> {
    stack_size: T,
    runtime_size: T,
    vertex_buffer_size: [T; 3],
    edge_geometry_vertex_buffer_size: [T; 3],
    index_buffer_size: [T; 3],
}

impl<T: for<'a> AnyNumberType<'a>> ModelMemorySizes<T> {
    fn total(&self) -> T {
        let mut total = T::default();
        total += self.stack_size;
        total += self.runtime_size;
        for i in 0..3 {
            total += self.vertex_buffer_size[i];
            total += self.edge_geometry_vertex_buffer_size[i];
            total += self.index_buffer_size[i];
        }
        total
    }
}

#[binrw]
#[derive(Debug)]
struct ModelFileBlock {
    num_blocks: u32,
    num_used_blocks: u32,
    version: u32,
    uncompressed_size: ModelMemorySizes<u32>,
    compressed_size: ModelMemorySizes<u32>,
    offset: ModelMemorySizes<u32>,
    index: ModelMemorySizes<u16>,
    num: ModelMemorySizes<u16>,
    vertex_declaration_num: u16,
    material_num: u16,
    num_lods: u8,
    index_buffer_streaming_enabled: u8,
    #[brw(pad_after = 1)]
    edge_geometry_enabled: u8,
}

#[binrw]
#[derive(Debug)]
struct TextureBlock {
    #[br(pad_before = 8)]
    num_blocks: u32,
    #[br(count = num_blocks)]
    lods: Vec<TextureLodBlock>,
}

#[binrw]
#[derive(Debug)]
struct FileInfo {
    size: u32,
    file_type: FileType,
    file_size: u32,
    #[br(if(file_type == FileType::Standard))]
    standard_info: Option<StandardFileBlock>,
    #[br(if(file_type == FileType::Model))]
    model_info: Option<ModelFileBlock>,
    #[br(if(file_type == FileType::Texture))]
    texture_info: Option<TextureBlock>,
}

#[binrw]
struct Block {
    #[br(pad_after = 4)]
    offset: i32,
}

#[binread]
#[derive(Debug)]
#[br(import { x: i32, y: i32 })]
#[br(map = |_: i32| if x < 32000 {
    CompressionMode::Compressed { compressed_length: x, decompressed_length: y }
} else {
    CompressionMode::Uncompressed { file_size: y }
})]
enum CompressionMode {
    Compressed {
        compressed_length: i32,
        decompressed_length: i32,
    },
    Uncompressed {
        file_size: i32,
    },
}

#[binread]
#[derive(Debug)]
struct BlockHeader {
    #[br(pad_after = 4)]
    _size: u32,
    #[br(temp)]
    x: i32,
    #[br(temp)]
    y: i32,
    #[br(args { x, y })]
    #[br(restore_position)]
    compression: CompressionMode,
}

fn read_sqpack_entry_from_reader<R: Read + Seek>(stream: &mut R) -> Option<Vec<u8>> {
    let file_info = FileInfo::read_options(stream, Endian::Little, ()).ok()?;
    match file_info.file_type {
        FileType::Empty => None,
        FileType::Standard => read_standard_file(stream, &file_info),
        FileType::Model => read_model_file(stream, &file_info),
        FileType::Texture => read_texture_file(stream, &file_info),
    }
}

fn read_standard_file<R: Read + Seek>(stream: &mut R, file_info: &FileInfo) -> Option<Vec<u8>> {
    let standard = file_info.standard_info.as_ref()?;
    let mut blocks = Vec::with_capacity(standard.num_blocks as usize);
    for _ in 0..standard.num_blocks {
        blocks.push(Block::read_options(stream, Endian::Little, ()).ok()?);
    }

    let mut data = Vec::with_capacity(file_info.file_size as usize);
    let starting_position = file_info.size as u64;
    for block in blocks {
        data.extend(read_data_block(
            stream,
            starting_position + block.offset as u64,
        )?);
    }
    Some(data)
}

fn read_texture_file<R: Read + Seek>(stream: &mut R, file_info: &FileInfo) -> Option<Vec<u8>> {
    let texture = file_info.texture_info.as_ref()?;
    let mut data = Vec::with_capacity(file_info.file_size as usize);
    let mipmap_size = texture.lods.first()?.compressed_size;
    if mipmap_size != 0 {
        let original_pos = stream.stream_position().ok()?;
        stream.seek(SeekFrom::Start(file_info.size as u64)).ok()?;
        let header_len = texture.lods.first()?.compressed_offset as usize;
        let mut header = vec![0; header_len];
        stream.read_exact(&mut header).ok()?;
        data.extend(header);
        stream.seek(SeekFrom::Start(original_pos)).ok()?;
    }

    for lod in &texture.lods {
        let mut running_block_total = lod.compressed_offset as u64 + file_info.size as u64;
        for _ in 0..lod.block_count {
            let original_pos = stream.stream_position().ok()?;
            data.extend(read_data_block(stream, running_block_total)?);
            stream.seek(SeekFrom::Start(original_pos)).ok()?;
            running_block_total += stream.read_type_args::<i16>(Endian::Little, ()).ok()? as u64;
        }
    }

    Some(data)
}

fn read_model_file<R: Read + Seek>(stream: &mut R, file_info: &FileInfo) -> Option<Vec<u8>> {
    let model = file_info.model_info.as_ref()?;
    let mut buffer = Cursor::new(Vec::new());
    let base_offset = file_info.size as u64;
    let total_blocks = model.num.total();
    let compressed_block_sizes: Vec<u16> = stream
        .read_type_args(
            Endian::Little,
            VecArgs::builder().count(total_blocks as usize).finalize(),
        )
        .ok()?;
    let mut current_block = 0_usize;
    let mut vertex_offsets = [0_u32; 3];
    let mut vertex_sizes = [0_u32; 3];
    let mut index_offsets = [0_u32; 3];
    let mut index_sizes = [0_u32; 3];

    buffer.seek(SeekFrom::Start(0x44)).ok()?;

    let mut read_model_blocks =
        |stream: &mut R, offset: u64, size: usize, current_block: &mut usize| -> Option<u32> {
            stream.seek(SeekFrom::Start(base_offset + offset)).ok()?;
            let start = buffer.position();
            for _ in 0..size {
                let block_start = stream.stream_position().ok()?;
                let data = read_data_block(stream, block_start)?;
                buffer.write_all(&data).ok()?;
                stream
                    .seek(SeekFrom::Start(
                        block_start + u64::from(compressed_block_sizes[*current_block]),
                    ))
                    .ok()?;
                *current_block += 1;
            }
            Some((buffer.position() - start) as u32)
        };

    let stack_size = read_model_blocks(
        stream,
        model.offset.stack_size as u64,
        model.num.stack_size as usize,
        &mut current_block,
    )?;
    let runtime_size = read_model_blocks(
        stream,
        model.offset.runtime_size as u64,
        model.num.runtime_size as usize,
        &mut current_block,
    )?;

    let mut process_model_data = |stream: &mut R,
                                  lod: usize,
                                  block_count: u32,
                                  offset: u32,
                                  offsets: &mut [u32; 3],
                                  sizes: &mut [u32; 3],
                                  current_block: &mut usize|
     -> Option<()> {
        if block_count == 0 {
            return Some(());
        }

        let current_offset = buffer.position() as u32;
        if lod == 0 || current_offset != offsets[lod - 1] {
            offsets[lod] = current_offset;
        }

        stream
            .seek(SeekFrom::Start(base_offset + u64::from(offset)))
            .ok()?;
        for _ in 0..block_count {
            let block_start = stream.stream_position().ok()?;
            let data = read_data_block(stream, block_start)?;
            buffer.write_all(&data).ok()?;
            sizes[lod] += data.len() as u32;
            stream
                .seek(SeekFrom::Start(
                    block_start + u64::from(compressed_block_sizes[*current_block]),
                ))
                .ok()?;
            *current_block += 1;
        }
        Some(())
    };

    for lod in 0..3 {
        process_model_data(
            stream,
            lod,
            model.num.vertex_buffer_size[lod] as u32,
            model.offset.vertex_buffer_size[lod],
            &mut vertex_offsets,
            &mut vertex_sizes,
            &mut current_block,
        )?;
        process_model_data(
            stream,
            lod,
            model.num.index_buffer_size[lod] as u32,
            model.offset.index_buffer_size[lod],
            &mut index_offsets,
            &mut index_sizes,
            &mut current_block,
        )?;
    }

    let header = ModelFileHeader {
        version: model.version,
        stack_size,
        runtime_size,
        vertex_declaration_count: model.vertex_declaration_num,
        material_count: model.material_num,
        vertex_offsets,
        index_offsets,
        vertex_buffer_size: vertex_sizes,
        index_buffer_size: index_sizes,
        lod_count: model.num_lods,
        index_buffer_streaming_enabled: model.index_buffer_streaming_enabled != 0,
        has_edge_geometry: model.edge_geometry_enabled != 0,
    };
    buffer.seek(SeekFrom::Start(0)).ok()?;
    header.write_options(&mut buffer, Endian::Little, ()).ok()?;
    Some(buffer.into_inner())
}

fn read_data_block<R: Read + Seek>(stream: &mut R, starting_position: u64) -> Option<Vec<u8>> {
    stream.seek(SeekFrom::Start(starting_position)).ok()?;
    let header = BlockHeader::read_options(stream, Endian::Little, ()).ok()?;
    match header.compression {
        CompressionMode::Compressed {
            compressed_length,
            decompressed_length,
        } => {
            let mut compressed = vec![0; compressed_length as usize];
            stream.read_exact(&mut compressed).ok()?;
            let mut decoder = DeflateDecoder::new(compressed.as_slice());
            let mut decompressed = Vec::with_capacity(decompressed_length as usize);
            decoder.read_to_end(&mut decompressed).ok()?;
            Some(decompressed)
        }
        CompressionMode::Uncompressed { file_size } => {
            let mut data = vec![0; file_size as usize];
            stream.read_exact(&mut data).ok()?;
            Some(data)
        }
    }
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
