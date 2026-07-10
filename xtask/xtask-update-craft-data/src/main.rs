use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
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
                    "schemaRevision": 1,
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

fn apply_item_release_history(
    catalog: &mut xiv_companion::CollectionCatalogPackage,
    repo: &Path,
) -> Result<()> {
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
        let expansion = if index == 0 {
            "历史版本".to_string()
        } else {
            expansion_label(&patch, subject)
        };
        for item_id in item_ids_from_csv(&csv)? {
            if seen.insert(item_id) {
                releases.insert(
                    item_id,
                    ReleaseMetadata {
                        expansion: expansion.clone(),
                        patch: patch.clone(),
                    },
                );
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
    let label = subject
        .split_once("patch ")
        .map(|(_, patch)| patch.split_whitespace().next().unwrap_or(patch))
        .or_else(|| {
            subject
                .split_once("ver ")
                .map(|(_, version)| version.split_whitespace().next().unwrap_or(version))
        })
        .unwrap_or("未归档版本");
    if first_snapshot {
        format!("{label} 及以前")
    } else {
        label.to_string()
    }
}

fn expansion_label(patch: &str, subject: &str) -> String {
    match patch.chars().next() {
        Some('2') => "重生之境",
        Some('3') => "苍穹之禁城",
        Some('4') => "红莲之狂潮",
        Some('5') => "暗影之逆焰",
        Some('6') => "晓月之终途",
        Some('7') => "金曦之遗辉",
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
    }

    #[test]
    fn item_history_csv_handles_multiline_descriptions() {
        let csv = "key,name,description\n1,测试,\"第一行\n2,不是新记录\"\n3,另一项,描述\n";
        assert_eq!(item_ids_from_csv(csv).unwrap(), vec![1, 3]);
    }
}
