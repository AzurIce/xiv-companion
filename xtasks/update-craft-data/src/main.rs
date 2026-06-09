use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use serde_json::{Map, Value};

#[derive(Parser)]
#[command(author, version, about = "XIV Companion data maintenance tasks")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Export crafting data and run post-export sanity checks.
    UpdateCraftData {
        /// Skip the generated JSON audit.
        #[arg(long)]
        skip_audit: bool,
    },
    /// Audit the generated crafting data without exporting it again.
    AuditCraftData,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = workspace_root()?;

    match args.command {
        Commands::UpdateCraftData { skip_audit } => {
            run_existing_exporter(&root)?;
            if !skip_audit {
                audit_craft_data(&root)?;
            }
        }
        Commands::AuditCraftData => audit_craft_data(&root)?,
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

fn run_existing_exporter(root: &Path) -> Result<()> {
    let status = ProcessCommand::new("bun")
        .current_dir(root)
        .args(["run", "scripts/update-craft-data.ts"])
        .status()
        .context("failed to start bun exporter")?;

    if !status.success() {
        bail!("craft data exporter failed with status {status}");
    }

    Ok(())
}

fn audit_craft_data(root: &Path) -> Result<()> {
    let data_path = root.join("app/public/craft-data.json");
    let data = fs::read_to_string(&data_path)
        .with_context(|| format!("failed to read {}", data_path.display()))?;
    let data: Value = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", data_path.display()))?;

    let items = data
        .get("items")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("craft-data.json is missing items"))?;
    let sources = data
        .get("sources")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("craft-data.json is missing sources"))?;

    assert_known_exchange_costs(items, sources)?;

    let mut low_id_costs = Vec::new();
    let mut missing_cost_items = Vec::new();
    for (receive_item_id, source_list) in sources {
        let Some(source_list) = source_list.as_array() else {
            continue;
        };
        for source in source_list {
            if source.get("kind").and_then(Value::as_str) != Some("specialShop") {
                continue;
            }
            let shop_name = source
                .get("shopName")
                .and_then(Value::as_str)
                .unwrap_or("兑换");
            let Some(costs) = source.get("costs").and_then(Value::as_array) else {
                continue;
            };
            for cost in costs {
                let Some(cost_item_id) = cost.get("itemId").and_then(Value::as_u64) else {
                    continue;
                };
                if (2..=17).contains(&cost_item_id) {
                    low_id_costs.push(format!(
                        "{} {} -> {} x{} @ {}",
                        receive_item_id,
                        item_name(items, receive_item_id.parse().unwrap_or_default()),
                        item_name(items, cost_item_id),
                        cost.get("count")
                            .and_then(Value::as_u64)
                            .unwrap_or_default(),
                        shop_name,
                    ));
                }
                if !items.contains_key(&cost_item_id.to_string()) {
                    missing_cost_items.push(format!(
                        "{} has missing cost item {} @ {}",
                        receive_item_id, cost_item_id, shop_name
                    ));
                }
            }
        }
    }

    if !low_id_costs.is_empty() {
        bail!(
            "found suspicious shard/crystal SpecialShop costs:\n{}",
            low_id_costs
                .into_iter()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    if !missing_cost_items.is_empty() {
        bail!(
            "found SpecialShop costs with missing item rows:\n{}",
            missing_cost_items
                .into_iter()
                .take(20)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    println!("craft data audit passed: no suspicious low-id SpecialShop costs");
    Ok(())
}

fn assert_known_exchange_costs(
    items: &Map<String, Value>,
    sources: &Map<String, Value>,
) -> Result<()> {
    let checks = [
        ("克罗诺兽的粗皮", "亚拉戈天道神典石", 20),
        ("克罗诺兽的粗皮", "亚拉戈数理神典石", 10),
        ("高浓缩炼金药", "巧手橙票", 125),
        ("高浓缩炼金药", "宇宙信用点", 250),
    ];

    let ids_by_name = item_ids_by_name(items);
    for (item_name, cost_name, cost_count) in checks {
        let item_id = ids_by_name
            .get(item_name)
            .copied()
            .ok_or_else(|| anyhow!("missing known item {item_name}"))?;
        let cost_item_id = ids_by_name
            .get(cost_name)
            .copied()
            .ok_or_else(|| anyhow!("missing known cost item {cost_name}"))?;

        if !has_special_shop_cost(sources, item_id, cost_item_id, cost_count) {
            bail!("{item_name} is missing expected exchange cost {cost_name} x{cost_count}");
        }
    }

    Ok(())
}

fn item_ids_by_name(items: &Map<String, Value>) -> BTreeMap<String, u64> {
    items
        .iter()
        .filter_map(|(id, item)| {
            let id = id.parse().ok()?;
            let name = item.get("name")?.as_str()?.to_string();
            Some((name, id))
        })
        .collect()
}

fn item_name(items: &Map<String, Value>, item_id: u64) -> String {
    items
        .get(&item_id.to_string())
        .and_then(|item| item.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("#{item_id}"))
}

fn has_special_shop_cost(
    sources: &Map<String, Value>,
    item_id: u64,
    cost_item_id: u64,
    cost_count: u64,
) -> bool {
    sources
        .get(&item_id.to_string())
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|source| source.get("kind").and_then(Value::as_str) == Some("specialShop"))
        .flat_map(|source| {
            source
                .get("costs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .any(|cost| {
            cost.get("itemId").and_then(Value::as_u64) == Some(cost_item_id)
                && cost.get("count").and_then(Value::as_u64) == Some(cost_count)
        })
}
