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
    audit::audit_craft_data,
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
    fs::write(
        out_dir.join("resource-manifest.json"),
        serde_json::to_string(&json!({
            "schemaVersion": 1,
            "resources": {
                "craft-data": {
                    "gameVersion": craft_data.game_version,
                    "revision": craft_data.generated_at,
                    "schemaRevision": 1,
                    "recordCount": craft_data.counts.items,
                },
                "weapon-catalog": {
                    "gameVersion": weapon_catalog.game_version,
                    "revision": weapon_catalog.generated_at,
                    "schemaRevision": xiv_companion::WEAPON_CATALOG_SCHEMA_REVISION,
                    "recordCount": weapon_catalog.counts.items,
                },
                "collection-catalog": {
                    "gameVersion": collection_catalog.game_version,
                    "revision": collection_catalog.generated_at,
                    "schemaRevision": collection_catalog.schema_version,
                    "recordCount": collection_catalog.counts.items,
                },
            }
        }))?,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            out_dir.join("resource-manifest.json").display()
        )
    })?;

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
