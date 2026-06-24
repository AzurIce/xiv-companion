use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::json;
use xiv_companion::{audit::audit_craft_data, game_data::export_craft_data};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Export XIV Companion crafting data from a game install"
)]
struct Args {
    /// FFXIV game directory. Accepts either the install root or the inner game directory.
    #[arg(long, value_name = "DIR")]
    game_dir: Option<PathBuf>,

    /// Output directory for craft-data.json and version.json.
    #[arg(long, value_name = "DIR", default_value = "assets")]
    out_dir: PathBuf,

    /// Only audit an existing craft-data.json without exporting.
    #[arg(long)]
    audit_only: bool,

    /// Skip the generated JSON audit after export.
    #[arg(long)]
    skip_audit: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = workspace_root()?;
    let out_dir = absolutize(&root, &args.out_dir);
    let data_path = out_dir.join("craft-data.json");

    if args.audit_only {
        audit_craft_data(&data_path)?;
        return Ok(());
    }

    let game_dir = args
        .game_dir
        .as_ref()
        .ok_or_else(|| anyhow!("--game-dir is required unless --audit-only is set"))?;
    let generated_at = chrono_like_timestamp();
    let data = export_craft_data(game_dir, generated_at.clone())?;
    let game_version = data.game_version.clone();

    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    fs::write(&data_path, serde_json::to_string(&data)?)
        .with_context(|| format!("failed to write {}", data_path.display()))?;
    fs::write(
        out_dir.join("version.json"),
        serde_json::to_string(&json!({
            "commit": game_version,
            "date": generated_at,
        }))?,
    )
    .with_context(|| format!("failed to write {}", out_dir.join("version.json").display()))?;

    println!("Items: {}", data.counts.items);
    println!("Recipes: {}", data.counts.recipes);
    println!("Sources: {}", data.counts.sources);
    println!("Output: {}", data_path.display());

    if !args.skip_audit {
        audit_craft_data(&data_path)?;
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
    let output = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output();

    output
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}
