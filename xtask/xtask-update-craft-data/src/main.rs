use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde::Deserialize;
use serde_json::json;
use xiv_companion::{
    CHARA_CATALOG_SCHEMA_VERSION, CharaActionRow, CharaItemInfo, CharaModelCharaRow,
    CharaModelLinkRow, CraftDataPackage, FURNITURE_CATALOG_SCHEMA_VERSION, FurnitureHousingRow,
    FurnitureItemInfo,
    audit::audit_craft_data,
    build_chara_catalog, build_furniture_catalog,
    game_data::{export_collection_catalog, export_craft_data, export_weapon_catalog},
};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Export XIV Companion game data from a game install"
)]
struct Args {
    /// FFXIV game directory. Accepts either the install root or the inner game directory.
    #[arg(long, value_name = "DIR")]
    game_dir: Option<PathBuf>,

    /// Output directory for generated JSON files and version.json.
    #[arg(long, value_name = "DIR", default_value = "assets")]
    out_dir: PathBuf,

    /// Only audit existing JSON files without exporting.
    #[arg(long)]
    audit_only: bool,

    /// Skip the generated JSON audit after export.
    #[arg(long)]
    skip_audit: bool,

    /// Generate the furniture catalog from datamining CSVs and craft-data.json
    /// instead of exporting from a game install.
    #[arg(long)]
    furniture_catalog: bool,

    /// Generate the minion/mount catalog from datamining CSVs and craft-data.json
    /// instead of exporting from a game install.
    #[arg(long)]
    chara_catalog: bool,

    /// HousingFurniture.csv used for --furniture-catalog. Overrides
    /// --datamining-repo and the default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    housing_furniture_csv: Option<PathBuf>,

    /// HousingYardObject.csv used for --furniture-catalog. Overrides
    /// --datamining-repo and the default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    housing_yard_object_csv: Option<PathBuf>,

    /// ItemAction.csv used for --chara-catalog. Overrides --datamining-repo and
    /// the default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    item_action_csv: Option<PathBuf>,

    /// Companion.csv used for --chara-catalog. Overrides --datamining-repo and
    /// the default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    companion_csv: Option<PathBuf>,

    /// Mount.csv used for --chara-catalog. Overrides --datamining-repo and the
    /// default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    mount_csv: Option<PathBuf>,

    /// ModelChara.csv used for --chara-catalog. Overrides --datamining-repo and
    /// the default download from ffxiv-datamining-cn.
    #[arg(long, value_name = "FILE")]
    model_chara_csv: Option<PathBuf>,

    /// Optional ffxiv-datamining-cn repository used to attach first-seen patch metadata.
    #[arg(long, value_name = "DIR")]
    datamining_repo: Option<PathBuf>,

    /// Garland Tools patch metadata used only to fill item releases before patch 4.45.
    #[arg(
        long,
        value_name = "FILE",
        default_value = "third_party/garland-tools/patches.json"
    )]
    garland_patches: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = workspace_root()?;
    let out_dir = absolutize(&root, &args.out_dir);
    let craft_data_path = out_dir.join("craft-data.json");
    let weapon_catalog_path = out_dir.join("weapon-catalog.json");
    let collection_catalog_path = out_dir.join("collection-catalog.json");

    if args.audit_only {
        audit_craft_data(&craft_data_path)?;
        return Ok(());
    }

    if args.furniture_catalog {
        return export_furniture_catalog(&out_dir, &args);
    }

    if args.chara_catalog {
        return export_chara_catalog(&out_dir, &args);
    }

    let game_dir = args
        .game_dir
        .as_ref()
        .ok_or_else(|| anyhow!("--game-dir is required unless --audit-only is set"))?;
    let generated_at = chrono_like_timestamp();

    let craft_data = export_craft_data(game_dir, generated_at.clone())?;
    let weapon_catalog = export_weapon_catalog(game_dir, generated_at.clone())?;
    let mut collection_catalog = export_collection_catalog(game_dir, generated_at.clone())?;
    if let Some(repo) = args.datamining_repo.as_deref() {
        apply_item_release_history(&mut collection_catalog, repo)?;
    }
    apply_garland_item_patches(
        &mut collection_catalog,
        &absolutize(&root, &args.garland_patches),
    )?;
    let game_version = craft_data.game_version.clone();

    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    fs::write(&craft_data_path, serde_json::to_string(&craft_data)?)
        .with_context(|| format!("failed to write {}", craft_data_path.display()))?;
    fs::write(
        &weapon_catalog_path,
        serde_json::to_string(&weapon_catalog)?,
    )
    .with_context(|| format!("failed to write {}", weapon_catalog_path.display()))?;
    fs::write(
        &collection_catalog_path,
        serde_json::to_string(&collection_catalog)?,
    )
    .with_context(|| format!("failed to write {}", collection_catalog_path.display()))?;
    fs::write(
        out_dir.join("version.json"),
        serde_json::to_string(&json!({
            "commit": game_version,
            "date": generated_at,
        }))?,
    )
    .with_context(|| format!("failed to write {}", out_dir.join("version.json").display()))?;
    let mut manifest_entries = serde_json::Map::new();
    manifest_entries.insert(
        "craft-data".to_string(),
        json!({
            "gameVersion": craft_data.game_version,
            "revision": craft_data.generated_at,
            "schemaRevision": 1,
            "recordCount": craft_data.counts.items,
        }),
    );
    manifest_entries.insert(
        "weapon-catalog".to_string(),
        json!({
            "gameVersion": weapon_catalog.game_version,
            "revision": weapon_catalog.generated_at,
            "schemaRevision": xiv_companion::WEAPON_CATALOG_SCHEMA_REVISION,
            "recordCount": weapon_catalog.counts.items,
        }),
    );
    manifest_entries.insert(
        "collection-catalog".to_string(),
        json!({
            "gameVersion": collection_catalog.game_version,
            "revision": collection_catalog.generated_at,
            "schemaRevision": collection_catalog.schema_version,
            "recordCount": collection_catalog.counts.items,
        }),
    );
    write_resource_manifest(&out_dir, manifest_entries)?;

    println!("CraftData items: {}", craft_data.counts.items);
    println!("CraftData recipes: {}", craft_data.counts.recipes);
    println!("CraftData sources: {}", craft_data.counts.sources);
    println!("WeaponCatalog items: {}", weapon_catalog.counts.items);
    println!(
        "CollectionCatalog items: {} equipment: {} rolls: {} mounts: {} minions: {}",
        collection_catalog.counts.items,
        collection_catalog.counts.equipment,
        collection_catalog.counts.orchestrion_rolls,
        collection_catalog.counts.mounts,
        collection_catalog.counts.minions,
    );
    println!("Output: {}", out_dir.display());

    if !args.skip_audit {
        audit_craft_data(&craft_data_path)?;
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let xtask_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask_root
        .join("../..")
        .canonicalize()
        .context("failed to resolve workspace root")
}

fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn chrono_like_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

const HOUSING_FURNITURE_CSV: &str = "HousingFurniture.csv";
const HOUSING_YARD_OBJECT_CSV: &str = "HousingYardObject.csv";
const ITEM_ACTION_CSV: &str = "ItemAction.csv";
const COMPANION_CSV: &str = "Companion.csv";
const MOUNT_CSV: &str = "Mount.csv";
const MODEL_CHARA_CSV: &str = "ModelChara.csv";
const DATAMINING_RAW_BASE: &str =
    "https://raw.githubusercontent.com/thewakingsands/ffxiv-datamining-cn/master";

/// 生成家具目录：housing CSV（显式路径 > --datamining-repo 工作区 > curl 下载）
/// 提供 ModelKey→物品映射，物品名/图标 join 现有 craft-data.json，游戏版本沿用
/// craft-data 的版本。只更新 resource-manifest.json 的 furniture-catalog 条目。
fn export_furniture_catalog(out_dir: &Path, args: &Args) -> Result<()> {
    let craft_data_path = out_dir.join("craft-data.json");
    let craft_data = serde_json::from_str::<CraftDataPackage>(
        &fs::read_to_string(&craft_data_path)
            .with_context(|| format!("failed to read {}", craft_data_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", craft_data_path.display()))?;

    let furniture_csv_path = obtain_datamining_csv(
        args.housing_furniture_csv.as_deref(),
        args.datamining_repo.as_deref(),
        HOUSING_FURNITURE_CSV,
    )?;
    let yard_object_csv_path = obtain_datamining_csv(
        args.housing_yard_object_csv.as_deref(),
        args.datamining_repo.as_deref(),
        HOUSING_YARD_OBJECT_CSV,
    )?;
    // HousingFurniture.csv 的 Item 在第 7 列，HousingYardObject.csv 在第 6 列。
    let indoor_rows = parse_housing_rows(&read_csv_text(&furniture_csv_path)?, 7)?;
    let outdoor_rows = parse_housing_rows(&read_csv_text(&yard_object_csv_path)?, 6)?;

    let generated_at = chrono_like_timestamp();
    let catalog = build_furniture_catalog(
        &indoor_rows,
        &outdoor_rows,
        |item_id| {
            craft_data
                .items
                .get(&item_id.to_string())
                .map(|item| FurnitureItemInfo {
                    name: item.name.clone(),
                    icon: item.icon,
                })
        },
        generated_at.clone(),
        craft_data.game_version.clone(),
        format!(
            "{} + {} + craft-data.json",
            HOUSING_FURNITURE_CSV, HOUSING_YARD_OBJECT_CSV
        ),
    );

    let catalog_path = out_dir.join("furniture-catalog.json");
    fs::write(&catalog_path, serde_json::to_string(&catalog)?)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;

    let mut manifest_entries = serde_json::Map::new();
    manifest_entries.insert(
        "furniture-catalog".to_string(),
        json!({
            "gameVersion": craft_data.game_version,
            "revision": generated_at,
            "schemaRevision": FURNITURE_CATALOG_SCHEMA_VERSION,
            "recordCount": catalog.counts.items,
        }),
    );
    write_resource_manifest(out_dir, manifest_entries)?;

    println!(
        "FurnitureCatalog items: {} indoor: {} outdoor: {} skipped missing items: {}",
        catalog.counts.items,
        catalog.counts.indoor,
        catalog.counts.outdoor,
        catalog.counts.skipped_missing_items,
    );
    println!("Output: {}", catalog_path.display());
    Ok(())
}

/// 生成宠物/坐骑目录：collection-catalog.json 提供 物品→ItemAction 行 链接
/// （Item.ItemAction 列）与物品名/图标，ItemAction.csv（Type 853=宠物/
/// 1322=坐骑，Data[0]=目标行）→ Companion/Mount 的 ModelChara 链接（第 8 列）
/// → ModelChara 三元组；游戏版本沿用 collection-catalog 的版本。只更新
/// resource-manifest.json 的 chara-catalog 条目。
fn export_chara_catalog(out_dir: &Path, args: &Args) -> Result<()> {
    let collection_catalog_path = out_dir.join("collection-catalog.json");
    let collection_catalog = serde_json::from_str::<xiv_companion::CollectionCatalogPackage>(
        &fs::read_to_string(&collection_catalog_path)
            .with_context(|| format!("failed to read {}", collection_catalog_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", collection_catalog_path.display()))?;

    let datamining_repo = args.datamining_repo.as_deref();
    let item_action_csv_path = obtain_datamining_csv(
        args.item_action_csv.as_deref(),
        datamining_repo,
        ITEM_ACTION_CSV,
    )?;
    let companion_csv_path = obtain_datamining_csv(
        args.companion_csv.as_deref(),
        datamining_repo,
        COMPANION_CSV,
    )?;
    let mount_csv_path =
        obtain_datamining_csv(args.mount_csv.as_deref(), datamining_repo, MOUNT_CSV)?;
    let model_chara_csv_path = obtain_datamining_csv(
        args.model_chara_csv.as_deref(),
        datamining_repo,
        MODEL_CHARA_CSV,
    )?;

    let actions = parse_item_action_rows(&read_csv_text(&item_action_csv_path)?)?;
    // Companion.csv 与 Mount.csv 的 ModelChara 链接都在第 8 列。
    let companions = parse_chara_link_rows(&read_csv_text(&companion_csv_path)?, 8)?;
    let mounts = parse_chara_link_rows(&read_csv_text(&mount_csv_path)?, 8)?;
    let model_charas = parse_model_chara_rows(&read_csv_text(&model_chara_csv_path)?)?;
    let unlock_items = collection_catalog
        .items
        .iter()
        .map(|item| xiv_companion::CharaUnlockItemRow {
            item_id: item.id,
            action_id: item.item_action,
        })
        .collect::<Vec<_>>();

    let generated_at = chrono_like_timestamp();
    let catalog = build_chara_catalog(
        &actions,
        &unlock_items,
        &companions,
        &mounts,
        &model_charas,
        |item_id| {
            collection_catalog
                .items
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| CharaItemInfo {
                    name: item.name.clone(),
                    icon: item.icon,
                })
        },
        generated_at.clone(),
        collection_catalog.game_version.clone(),
        format!(
            "{} + {} + {} + {} + collection-catalog.json",
            ITEM_ACTION_CSV, COMPANION_CSV, MOUNT_CSV, MODEL_CHARA_CSV
        ),
    );

    let catalog_path = out_dir.join("chara-catalog.json");
    fs::write(&catalog_path, serde_json::to_string(&catalog)?)
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;

    let mut manifest_entries = serde_json::Map::new();
    manifest_entries.insert(
        "chara-catalog".to_string(),
        json!({
            "gameVersion": collection_catalog.game_version,
            "revision": generated_at,
            "schemaRevision": CHARA_CATALOG_SCHEMA_VERSION,
            "recordCount": catalog.counts.items,
        }),
    );
    write_resource_manifest(out_dir, manifest_entries)?;

    println!(
        "CharaCatalog items: {} minions: {} mounts: {} skipped empty targets: {} unsupported models: {} missing items: {}",
        catalog.counts.items,
        catalog.counts.minions,
        catalog.counts.mounts,
        catalog.counts.skipped_empty_targets,
        catalog.counts.skipped_unsupported_models,
        catalog.counts.skipped_missing_items,
    );
    println!("Output: {}", catalog_path.display());
    Ok(())
}

fn read_csv_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn obtain_datamining_csv(
    explicit: Option<&Path>,
    datamining_repo: Option<&Path>,
    file_name: &str,
) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(repo) = datamining_repo {
        let path = repo.join(file_name);
        if !path.exists() {
            return Err(anyhow!(
                "{} not found in datamining repo {}",
                file_name,
                repo.display()
            ));
        }
        return Ok(path);
    }

    let url = format!("{DATAMINING_RAW_BASE}/{file_name}");
    let path = std::env::temp_dir().join(format!("xiv-companion-{file_name}"));
    download_file(&url, &path)?;
    Ok(path)
}

fn download_file(url: &str, path: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args(["-fsSL", url, "-o"])
        .arg(path)
        .status()
        .with_context(|| format!("failed to run curl for {url}"))?;
    if !status.success() {
        return Err(anyhow!("curl failed for {url}: {status}"));
    }
    Ok(())
}

/// ffxiv-datamining-cn EXD CSV：3 行表头（key 行 / # 名称行 / 类型行）后接数据。
/// record 第 0 列是行 key，EXD 列从索引 1 开始：ModelKey(uint16) 在 EXD 列 0，
/// Item(uint32，0 = 无物品) 在 EXD 列 `item_column`（HousingFurniture 为 7，
/// HousingYardObject 为 6）。
fn parse_housing_rows(csv: &str, item_column: usize) -> Result<Vec<FurnitureHousingRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records().skip(3) {
        let record = record.context("failed to parse housing CSV")?;
        let (Some(model_key), Some(item_id)) = (
            record.get(1).and_then(|value| value.parse::<u16>().ok()),
            record
                .get(item_column + 1)
                .and_then(|value| value.parse::<u32>().ok()),
        ) else {
            continue;
        };
        rows.push(FurnitureHousingRow { model_key, item_id });
    }
    Ok(rows)
}

/// ItemAction.csv：key=**ItemAction 行 id**（物品经 Item.ItemAction 列链接
/// 到这里），EXD 列 4=Type（uint16，record 索引 5），列 5=Data[0]（uint16，
/// record 索引 6，Companion/Mount 行 key）。
fn parse_item_action_rows(csv: &str) -> Result<Vec<CharaActionRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records().skip(3) {
        let record = record.context("failed to parse ItemAction CSV")?;
        let (Some(action_id), Some(action_type), Some(target_id)) = (
            record.get(0).and_then(|value| value.parse::<u32>().ok()),
            record.get(5).and_then(|value| value.parse::<u32>().ok()),
            record.get(6).and_then(|value| value.parse::<u32>().ok()),
        ) else {
            continue;
        };
        rows.push(CharaActionRow {
            action_id,
            action_type,
            target_id,
        });
    }
    Ok(rows)
}

/// Companion.csv / Mount.csv：key=行 key，ModelChara 链接（Companion 列名
/// Model、Mount 列名 ModelChara）在 EXD 列 `link_column`（均为 8）。
fn parse_chara_link_rows(csv: &str, link_column: usize) -> Result<Vec<CharaModelLinkRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records().skip(3) {
        let record = record.context("failed to parse chara link CSV")?;
        let (Some(id), Some(model_chara_id)) = (
            record.get(0).and_then(|value| value.parse::<u32>().ok()),
            record
                .get(link_column + 1)
                .and_then(|value| value.parse::<u32>().ok()),
        ) else {
            continue;
        };
        rows.push(CharaModelLinkRow { id, model_chara_id });
    }
    Ok(rows)
}

/// ModelChara.csv：key=行 key，EXD 列 0=Type(byte)、1=Model(uint16)、
/// 2=Base(byte)、3=Variant(byte)。
fn parse_model_chara_rows(csv: &str) -> Result<Vec<CharaModelCharaRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records().skip(3) {
        let record = record.context("failed to parse ModelChara CSV")?;
        let (Some(id), Some(type_id), Some(model_id), Some(base_id), Some(variant_id)) = (
            record.get(0).and_then(|value| value.parse::<u32>().ok()),
            record.get(1).and_then(|value| value.parse::<u8>().ok()),
            record.get(2).and_then(|value| value.parse::<u16>().ok()),
            record.get(3).and_then(|value| value.parse::<u16>().ok()),
            record.get(4).and_then(|value| value.parse::<u16>().ok()),
        ) else {
            continue;
        };
        rows.push(CharaModelCharaRow {
            id,
            type_id,
            model_id,
            base_id,
            variant_id,
        });
    }
    Ok(rows)
}

/// 写入 resource-manifest.json：保留已有 manifest 中本次不管理的条目（例如家具
/// 目录由 --furniture-catalog 独立生成），再覆盖本次管理的条目。
fn write_resource_manifest(
    out_dir: &Path,
    entries: serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let manifest_path = out_dir.join("resource-manifest.json");
    let mut resources = serde_json::Map::new();
    if let Ok(existing) = fs::read_to_string(&manifest_path) {
        if let Ok(serde_json::Value::Object(mut manifest)) =
            serde_json::from_str::<serde_json::Value>(&existing)
        {
            if let Some(serde_json::Value::Object(existing_resources)) =
                manifest.remove("resources")
            {
                resources.extend(existing_resources);
            }
        }
    }
    resources.extend(entries);
    fs::write(
        &manifest_path,
        serde_json::to_string(&json!({
            "schemaVersion": 1,
            "resources": resources,
        }))?,
    )
    .with_context(|| format!("failed to write {}", manifest_path.display()))
}

#[derive(Clone, Debug)]
struct ReleaseMetadata {
    expansion: String,
    patch: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExpansionBoundary {
    name: String,
    item_id_start: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct GarlandPatchEntry {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    patch: f64,
}

fn apply_garland_item_patches(
    catalog: &mut xiv_companion::CollectionCatalogPackage,
    path: &Path,
) -> Result<()> {
    let json = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read Garland patch metadata from {}",
            path.display()
        )
    })?;
    let item_patches = garland_item_patches(&json).with_context(|| {
        format!(
            "failed to parse Garland patch metadata from {}",
            path.display()
        )
    })?;

    for item in &mut catalog.items {
        let Some(&patch) = item_patches.get(&item.id) else {
            continue;
        };
        if patch < 2.0 {
            item.expansion = "旧版遗留".to_string();
            item.patch = "1.x（具体版本未知）".to_string();
            continue;
        }
        let patch = format_patch_number(patch);
        item.expansion = expansion_label(&patch, "");
        item.patch = patch;
    }
    Ok(())
}

fn garland_item_patches(json: &str) -> Result<HashMap<u32, f64>> {
    let entries =
        serde_json::from_str::<Vec<GarlandPatchEntry>>(json.trim_start_matches('\u{feff}'))?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry.kind == "item" && entry.patch < 4.45)
        .filter_map(|entry| entry.id.parse::<u32>().ok().map(|id| (id, entry.patch)))
        .collect())
}

fn format_patch_number(patch: f64) -> String {
    if patch.fract() == 0.0 {
        format!("{patch:.1}")
    } else {
        patch.to_string()
    }
}

fn apply_item_release_history(
    catalog: &mut xiv_companion::CollectionCatalogPackage,
    repo: &Path,
) -> Result<()> {
    let expansion_boundaries =
        expansion_boundaries_from_csv(&git_output(repo, &["show", "HEAD:ExVersion.csv"])?)?;
    let log = git_output(
        repo,
        &["log", "--reverse", "--format=%H%x09%s", "--", "Item.csv"],
    )?;
    let commits = log
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect::<Vec<_>>();
    if commits.is_empty() {
        return Err(anyhow!("no Item.csv history found in {}", repo.display()));
    }

    let mut seen = HashSet::new();
    let mut releases = HashMap::new();
    for (index, (commit, subject)) in commits.iter().enumerate() {
        let csv = git_output(repo, &["show", &format!("{commit}:Item.csv")])?;
        let patch = release_label(subject, index == 0);
        for item_id in item_ids_from_csv(&csv)? {
            if seen.insert(item_id) {
                let (expansion, patch) = if index == 0 {
                    baseline_release(item_id, &patch, &expansion_boundaries)
                } else {
                    (expansion_label(&patch, subject), patch.clone())
                };
                releases.insert(item_id, ReleaseMetadata { expansion, patch });
            }
        }
    }

    for item in &mut catalog.items {
        if let Some(release) = releases.get(&item.id) {
            item.expansion.clone_from(&release.expansion);
            item.patch.clone_from(&release.patch);
        }
    }
    Ok(())
}

fn expansion_boundaries_from_csv(csv: &str) -> Result<Vec<ExpansionBoundary>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut boundaries = Vec::new();
    for record in reader.records().skip(3) {
        let record = record.context("failed to parse ExVersion.csv")?;
        let Some(name) = record.get(1).filter(|name| !name.is_empty()) else {
            continue;
        };
        let Some(item_id_start) = record.get(4).and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        boundaries.push(ExpansionBoundary {
            name: name.to_string(),
            item_id_start,
        });
    }
    boundaries.sort_by_key(|boundary| boundary.item_id_start);
    if boundaries.is_empty() {
        return Err(anyhow!(
            "no expansion item boundaries found in ExVersion.csv"
        ));
    }
    Ok(boundaries)
}

fn baseline_release(
    item_id: u32,
    first_snapshot_patch: &str,
    boundaries: &[ExpansionBoundary],
) -> (String, String) {
    let Some(boundary) = boundaries
        .iter()
        .rev()
        .find(|boundary| item_id >= boundary.item_id_start)
    else {
        return ("未归档".to_string(), first_snapshot_patch.to_string());
    };
    let patch = match boundary.name.as_str() {
        "重生之境" => "2.x".to_string(),
        "苍穹之禁城" => "3.x".to_string(),
        "红莲之狂潮" => first_snapshot_patch.to_string(),
        _ => first_snapshot_patch.to_string(),
    };
    (boundary.name.clone(), patch)
}

fn git_output(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", repo.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn item_ids_from_csv(csv: &str) -> Result<Vec<u32>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut ids = Vec::new();
    for record in reader.records() {
        let record = record.context("failed to parse Item.csv history snapshot")?;
        if let Some(id) = record.get(0).and_then(|value| value.parse::<u32>().ok()) {
            ids.push(id);
        }
    }
    Ok(ids)
}

fn release_label(subject: &str, first_snapshot: bool) -> String {
    let raw_label = subject
        .split_once("patch ")
        .map(|(_, patch)| patch.split_whitespace().next().unwrap_or(patch))
        .or_else(|| {
            subject
                .split_once("ver ")
                .map(|(_, version)| version.split_whitespace().next().unwrap_or(version))
        })
        .unwrap_or("未归档版本");
    let label = patch_for_cn_build(raw_label).unwrap_or(raw_label);
    if first_snapshot {
        format!("{label} 及以前")
    } else {
        label.to_string()
    }
}

fn patch_for_cn_build(build: &str) -> Option<&'static str> {
    match build {
        "2025.12.09.0000.0000" | "2025.12.18.0000.0000" | "2025.12.23.0000.0000" => Some("7.4"),
        "2026.01.21.0000.0000" => Some("7.41"),
        "2026.02.20.0000.0000" | "2026.03.07.0000.0000" => Some("7.45"),
        "2026.04.21.0000.0000" | "2026.05.01.0000.0000" => Some("7.5"),
        _ => None,
    }
}

fn expansion_label(patch: &str, subject: &str) -> String {
    match patch.split_once('.').map(|(major, _)| major) {
        Some("2") => "重生之境",
        Some("3") => "苍穹之禁城",
        Some("4") => "红莲之狂潮",
        Some("5") => "暗影之逆焰",
        Some("6") => "晓月之终途",
        Some("7") => "金曦之遗辉",
        _ if subject.contains("2019") || subject.contains("2020") || subject.contains("2021") => {
            "暗影之逆焰"
        }
        _ if subject.contains("2022") || subject.contains("2023") => "晓月之终途",
        _ if subject.contains("2024") || subject.contains("2025") || subject.contains("2026") => {
            "金曦之遗辉"
        }
        _ => "未归档",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_action_csv_reads_type_and_data0() {
        let csv = "\u{feff}key,0,1,2,3,4,5,6\n#,CondLv,CondBattle,CondPVP,CondPVPOnly,Type,Data[0],Data[1]\nint32,byte,bit&01,bit&02,bit&04,uint16,uint16,uint16\n252,0,True,True,False,853,1,0\n324,0,True,True,False,1322,1,0\n";
        let rows = parse_item_action_rows(csv).unwrap();
        assert_eq!(
            rows,
            vec![
                CharaActionRow {
                    action_id: 252,
                    action_type: 853,
                    target_id: 1,
                },
                CharaActionRow {
                    action_id: 324,
                    action_type: 1322,
                    target_id: 1,
                },
            ]
        );
    }

    #[test]
    fn chara_link_csv_reads_the_model_chara_column() {
        // Companion.csv：ModelChara 链接（列名 Model）在第 8 列。
        let csv = "key,0,1,2,3,4,5,6,7,8,9\n#,Singular,Adjective,Plural,PossessivePronoun,StartsWithVowel,,Pronoun,Article,Model,Scale\nint32,str,sbyte,str,sbyte,sbyte,sbyte,sbyte,sbyte,ModelChara,byte\n1,爆弹仔,-1,爆弹仔,0,0,0,0,0,427,1\n";
        let rows = parse_chara_link_rows(csv, 8).unwrap();
        assert_eq!(
            rows,
            vec![CharaModelLinkRow {
                id: 1,
                model_chara_id: 427,
            }]
        );
    }

    #[test]
    fn model_chara_csv_reads_type_model_base_variant() {
        let csv = "key,0,1,2,3,4\n#,Type,Model,Base,Variant,SEPack\nint32,byte,uint16,byte,byte,uint16\n427,3,8003,1,1,3556\n1,2,1,1,1,3073\n";
        let rows = parse_model_chara_rows(csv).unwrap();
        assert_eq!(
            rows,
            vec![
                CharaModelCharaRow {
                    id: 427,
                    type_id: 3,
                    model_id: 8003,
                    base_id: 1,
                    variant_id: 1,
                },
                CharaModelCharaRow {
                    id: 1,
                    type_id: 2,
                    model_id: 1,
                    base_id: 1,
                    variant_id: 1,
                },
            ]
        );
    }

    #[test]
    fn housing_csv_rows_skip_headers_and_parse_the_item_column() {
        let csv = "\u{feff}key,0,1,2,3,4,5,6,7\n#,ModelKey,HousingItemCategory,UsageType,UsageParameter,,AquariumTier,CustomTalk,Item\nint32,uint16,byte,byte,uint32,byte,byte,CustomTalk,Item\n196608,0,0,0,0,0,0,0,0\n196609,42,1,0,0,0,0,0,19770\n";
        let rows = parse_housing_rows(csv, 7).unwrap();
        assert_eq!(
            rows,
            vec![
                FurnitureHousingRow {
                    model_key: 0,
                    item_id: 0,
                },
                FurnitureHousingRow {
                    model_key: 42,
                    item_id: 19770,
                },
            ]
        );

        // HousingYardObject.csv 的 Item 在第 6 列。
        let csv = "key,0,1,2,3,4,5,6\n#,ModelKey,HousingItemCategory,UsageType,UsageParameter,,CustomTalk,Item\nint32,uint16,byte,byte,uint32,byte,CustomTalk,Item\n131072,7,17,0,0,0,0,9710\n";
        let rows = parse_housing_rows(csv, 6).unwrap();
        assert_eq!(
            rows,
            vec![FurnitureHousingRow {
                model_key: 7,
                item_id: 9710,
            }]
        );
    }

    #[test]
    fn release_subjects_keep_patch_labels() {
        assert_eq!(release_label("ver 2021.03.29 patch 5.4", false), "5.4");
        assert_eq!(
            release_label("ver 2019.03.19 patch 4.45", true),
            "4.45 及以前"
        );
        assert_eq!(expansion_label("6.5", ""), "晓月之终途");
        assert_eq!(release_label("ver 2025.12.09.0000.0000", false), "7.4");
        assert_eq!(release_label("ver 2026.04.21.0000.0000", false), "7.5");
        assert_eq!(
            release_label("ver 2027.01.01.0000.0000", false),
            "2027.01.01.0000.0000"
        );
        assert_eq!(
            expansion_label("7.5", "ver 2026.04.21.0000.0000"),
            "金曦之遗辉"
        );
    }

    #[test]
    fn item_history_csv_handles_multiline_descriptions() {
        let csv = "key,name,description\n1,测试,\"第一行\n2,不是新记录\"\n3,另一项,描述\n";
        assert_eq!(item_ids_from_csv(csv).unwrap(), vec![1, 3]);
    }

    #[test]
    fn exversion_boundaries_split_the_first_history_snapshot() {
        let csv = "key,0,1,2,3,4\n#,Name,AcceptJingle,CompleteJingle,,\nint32,str,ScreenImage,ScreenImage,uint32,uint32\n0,重生之境,1,2,0,61875\n1,苍穹之禁城,342,343,8240,61876\n2,红莲之狂潮,344,345,16090,61877\n";
        let boundaries = expansion_boundaries_from_csv(csv).unwrap();
        assert_eq!(
            baseline_release(8239, "4.45 及以前", &boundaries),
            ("重生之境".to_string(), "2.x".to_string())
        );
        assert_eq!(
            baseline_release(8240, "4.45 及以前", &boundaries),
            ("苍穹之禁城".to_string(), "3.x".to_string())
        );
        assert_eq!(
            baseline_release(16090, "4.45 及以前", &boundaries),
            ("红莲之狂潮".to_string(), "4.45 及以前".to_string())
        );
    }

    #[test]
    fn garland_patch_numbers_keep_major_minor_format() {
        assert_eq!(format_patch_number(2.0), "2.0");
        assert_eq!(format_patch_number(2.35), "2.35");
        assert_eq!(format_patch_number(4.4), "4.4");
    }

    #[test]
    fn garland_legacy_patch_is_not_presented_as_exact_1_0() {
        let mut catalog = xiv_companion::CollectionCatalogPackage {
            schema_version: xiv_companion::COLLECTION_CATALOG_SCHEMA_VERSION,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: xiv_companion::CollectionCatalogCounts::default(),
            items: vec![xiv_companion::CollectionItem {
                id: 1,
                kind: xiv_companion::CollectionKind::Equipment,
                name: "旧版物品".to_string(),
                description: String::new(),
                icon: 0,
                item_ui_category: 0,
                item_search_category: 0,
                item_action: 0,
                equip_slot_category: 1,
                slot_name: String::new(),
                slot_order: 0,
                level_item: 0,
                level_equip: 0,
                rarity: 0,
                class_job_category: 0,
                class_job_category_name: String::new(),
                item_series: 0,
                set_id: "item:1".to_string(),
                set_name: "旧版物品".to_string(),
                set_item_ids: Vec::new(),
                expansion: String::new(),
                patch: String::new(),
                model_main: 0,
                model_sub: 0,
                appearance_key: String::new(),
            }],
        };
        let path = std::env::temp_dir().join("xiv-companion-garland-legacy-test.json");
        fs::write(&path, "[{\"type\":\"item\",\"id\":\"1\",\"patch\":1.0}]").unwrap();
        apply_garland_item_patches(&mut catalog, &path).unwrap();
        let _ = fs::remove_file(path);
        assert_eq!(catalog.items[0].expansion, "旧版遗留");
        assert_eq!(catalog.items[0].patch, "1.x（具体版本未知）");
    }

    #[test]
    fn garland_history_keeps_only_early_item_entries() {
        let patches = garland_item_patches(
            "\u{feff}[{\"type\":\"item\",\"id\":\"1\",\"patch\":2.0},{\"type\":\"quest\",\"id\":\"2\",\"patch\":2.1},{\"type\":\"item\",\"id\":\"3\",\"patch\":4.45}]",
        )
        .unwrap();
        assert_eq!(patches, HashMap::from([(1, 2.0)]));
    }
}
