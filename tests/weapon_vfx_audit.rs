#![cfg(feature = "game-data")]

//! 武器 VFX 审计：以 IMC 的 VfxId 为唯一判定来源，枚举目录里哪些武器模型挂了
//! avfx 特效、对应的 `vw####.avfx` 是否存在、avfx 顶层块构成，以及这些模型的
//! mtrl 用了哪些 shpk。不按物品名/系列猜测——发光武器的集合由数据自己说话。
//!
//! 在装有游戏客户端的机器上运行：
//! `XIV_GAME_DIR=... cargo test --features game-data --test weapon_vfx_audit -- --ignored`
//! 可选 `XIV_VFX_AUDIT_LIMIT=<n>` 只扫前 n 个模型做快速冒烟。

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};
use physis::resource::{Resource, SqPackResource};
use serde::Serialize;
use xiv_companion::{
    PackedModelId, WeaponCatalogItem,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    material_debug_info_from_mtrl_bytes, mdl_metadata_from_mdl_bytes,
    weapon_material_candidate_paths, weapon_model_candidate_paths,
};
use xiv_companion_data::imc::{ImcFile, ImcKind};

const MAX_AVFX_BLOCKS: usize = 4096;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponVfxAudit {
    game_dir: String,
    catalog_items: usize,
    unique_models: usize,
    scanned_models: usize,
    models_with_imc: usize,
    models_missing_imc: usize,
    imc_parse_failures: Vec<ImcParseFailure>,
    models_with_vfx: usize,
    vfx_entry_count: usize,
    distinct_vfx_ids: usize,
    avfx_found: usize,
    avfx_missing: usize,
    vw_padding_hits: BTreeMap<String, usize>,
    block_name_counts: BTreeMap<String, usize>,
    shader_package_counts: BTreeMap<String, usize>,
    models: Vec<VfxModelReport>,
    missing_avfx_entries: Vec<WeaponVfxEntry>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImcParseFailure {
    path: String,
    bytes: usize,
    error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VfxModelReport {
    model_raw: u64,
    model_id: u16,
    variant_id: u16,
    items: Vec<VfxItemRef>,
    entries: Vec<WeaponVfxEntry>,
    shader_packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VfxItemRef {
    item_id: u32,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponVfxEntry {
    model_id: u16,
    body_id: u16,
    variant: u16,
    material_set: u8,
    vfx_id: u8,
    animation: u8,
    /// 命中的 avfx 资源路径；None 表示所有 vw 命名变体都未命中。
    avfx_path: Option<String>,
    avfx_bytes: Option<usize>,
    /// 命中的 vw 文件名填充风格（"vw0001" / "vw001" / ...）。
    vw_padding: Option<String>,
    avfx_block_counts: Option<BTreeMap<String, usize>>,
}

#[test]
#[ignore = "scans weapon IMC/VFX from a local game SqPack into target/weapon-vfx-audit"]
fn audit_installed_weapon_vfx() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let output_dir = PathBuf::from("target").join("weapon-vfx-audit");
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "local-vfx-audit".to_string(),
    )
    .context("failed to export weapon catalog from local SqPack")?;

    let mut items_by_model = HashMap::<u64, Vec<VfxItemRef>>::new();
    for item in &catalog.items {
        for raw in model_raw_ids(item) {
            items_by_model.entry(raw).or_default().push(VfxItemRef {
                item_id: item.id,
                name: item.name.clone(),
            });
        }
    }
    let mut model_raws = items_by_model.keys().copied().collect::<Vec<_>>();
    model_raws.sort_unstable();

    let mut resource = SqPackResource::from_existing(game_dir_text);
    let mut audit = WeaponVfxAudit {
        game_dir: game_dir.display().to_string(),
        catalog_items: catalog.items.len(),
        unique_models: model_raws.len(),
        scanned_models: 0,
        models_with_imc: 0,
        models_missing_imc: 0,
        imc_parse_failures: Vec::new(),
        models_with_vfx: 0,
        vfx_entry_count: 0,
        distinct_vfx_ids: 0,
        avfx_found: 0,
        avfx_missing: 0,
        vw_padding_hits: BTreeMap::new(),
        block_name_counts: BTreeMap::new(),
        shader_package_counts: BTreeMap::new(),
        models: Vec::new(),
        missing_avfx_entries: Vec::new(),
        failures: Vec::new(),
    };
    let mut distinct_vfx_ids = HashSet::new();
    let scan_limit = vfx_audit_limit();

    for (index, model_raw) in model_raws.into_iter().enumerate() {
        if let Some(limit) = scan_limit {
            if index >= limit {
                break;
            }
        }
        if index % 500 == 0 {
            eprintln!("vfx audit: model {index}...");
        }
        audit.scanned_models += 1;
        let model = PackedModelId::from_raw(model_raw);
        let items = items_by_model.remove(&model_raw).unwrap_or_default();

        match audit_model_vfx(&mut resource, &model, &mut audit) {
            Ok(entries) if entries.is_empty() => {}
            Ok(entries) => {
                distinct_vfx_ids.extend(entries.iter().map(|entry| entry.vfx_id));
                let shader_packages = audit_model_shader_packages(&mut resource, &model);
                for name in &shader_packages {
                    *audit.shader_package_counts.entry(name.clone()).or_default() += 1;
                }
                audit.vfx_entry_count += entries.len();
                audit.models_with_vfx += 1;
                audit.models.push(VfxModelReport {
                    model_raw,
                    model_id: model.model_id,
                    variant_id: model.variant_id,
                    items,
                    entries,
                    shader_packages,
                });
            }
            Err(error) => audit
                .failures
                .push(format!("model {model_raw} ({}): {error:#}", model.model_id)),
        }
    }

    audit.distinct_vfx_ids = distinct_vfx_ids.len();
    audit.missing_avfx_entries = audit
        .models
        .iter()
        .flat_map(|report| report.entries.iter())
        .filter(|entry| entry.avfx_path.is_none())
        .cloned()
        .collect();
    audit.avfx_missing = audit.missing_avfx_entries.len();
    audit.avfx_found = audit.vfx_entry_count - audit.avfx_missing;

    let report_path = output_dir.join("report.json");
    let report_bytes =
        serde_json::to_vec_pretty(&audit).context("failed to encode weapon vfx audit report")?;
    fs::write(&report_path, report_bytes)
        .with_context(|| format!("failed to write {}", report_path.display()))?;
    write_index_markdown(&output_dir.join("index.md"), &audit)?;

    eprintln!("report: {}", report_path.display());
    eprintln!(
        "vfx audit: {} models scanned, {} with imc, {} with vfx, {}/{} avfx found",
        audit.scanned_models,
        audit.models_with_imc,
        audit.models_with_vfx,
        audit.avfx_found,
        audit.vfx_entry_count
    );
    if !audit.failures.is_empty() {
        anyhow::bail!(
            "weapon vfx audit recorded {} failures:\n{}",
            audit.failures.len(),
            audit.failures.join("\n")
        );
    }
    Ok(())
}

fn audit_model_vfx(
    resource: &mut SqPackResource,
    model: &PackedModelId,
    audit: &mut WeaponVfxAudit,
) -> Result<Vec<WeaponVfxEntry>> {
    let mut entries = Vec::new();
    let mut found_any_imc = false;
    for body_id in weapon_body_ids(model) {
        let imc_path = format!(
            "chara/weapon/w{:04}/obj/body/b{:04}/imc.imc",
            model.model_id, body_id
        );
        let Some(bytes) = resource.read(&imc_path) else {
            continue;
        };
        let imc = match ImcFile::parse(&bytes) {
            Ok(imc) => imc,
            Err(error) => {
                audit.imc_parse_failures.push(ImcParseFailure {
                    path: imc_path,
                    bytes: bytes.len(),
                    error: error.to_string(),
                });
                continue;
            }
        };
        if imc.kind != Some(ImcKind::NonSet) {
            audit.imc_parse_failures.push(ImcParseFailure {
                path: imc_path,
                bytes: bytes.len(),
                error: format!("expected NonSet imc, got kind {}", imc.raw_kind),
            });
            continue;
        }
        found_any_imc = true;

        let variants = std::iter::once((0_u16, imc.default_subset.as_slice())).chain(
            imc.subsets
                .iter()
                .enumerate()
                .map(|(index, subset)| (index as u16 + 1, subset.as_slice())),
        );
        for (variant, subset) in variants {
            for entry in subset {
                if entry.vfx == 0 {
                    continue;
                }
                if let Some((padding, path, bytes)) =
                    resolve_vw_avfx(resource, model.model_id, body_id, entry.vfx)
                {
                    let block_counts = avfx_top_level_block_counts(&bytes);
                    if let Some(counts) = &block_counts {
                        for (name, count) in counts {
                            *audit.block_name_counts.entry(name.clone()).or_default() += count;
                        }
                    }
                    *audit.vw_padding_hits.entry(padding.clone()).or_default() += 1;
                    entries.push(WeaponVfxEntry {
                        model_id: model.model_id,
                        body_id,
                        variant,
                        material_set: entry.material_set,
                        vfx_id: entry.vfx,
                        animation: entry.animation,
                        avfx_path: Some(path),
                        avfx_bytes: Some(bytes.len()),
                        vw_padding: Some(padding),
                        avfx_block_counts: block_counts,
                    });
                } else {
                    entries.push(WeaponVfxEntry {
                        model_id: model.model_id,
                        body_id,
                        variant,
                        material_set: entry.material_set,
                        vfx_id: entry.vfx,
                        animation: entry.animation,
                        avfx_path: None,
                        avfx_bytes: None,
                        vw_padding: None,
                        avfx_block_counts: None,
                    });
                }
            }
        }
    }
    if !found_any_imc {
        audit.models_missing_imc += 1;
    } else {
        audit.models_with_imc += 1;
    }
    Ok(entries)
}

/// 解析模型的第一份 .mdl 的材质表，汇总实际用到的 shpk 名（按材质去重）。
fn audit_model_shader_packages(
    resource: &mut SqPackResource,
    model: &PackedModelId,
) -> Vec<String> {
    let mut packages = HashSet::new();
    'models: for model_path in weapon_model_candidate_paths(*model) {
        let Some(mdl_bytes) = resource.read(&model_path) else {
            continue;
        };
        let Ok(metadata) = mdl_metadata_from_mdl_bytes(&model_path, &mdl_bytes) else {
            continue;
        };
        for material in &metadata.materials {
            let Some(material_name) = material.name.as_deref() else {
                continue;
            };
            for material_path in weapon_material_candidate_paths(*model, &model_path, material_name)
            {
                let Some(mtrl_bytes) = resource.read(&material_path) else {
                    continue;
                };
                if let Ok(debug) = material_debug_info_from_mtrl_bytes(&material_path, &mtrl_bytes)
                {
                    if !debug.shader_package_name.is_empty() {
                        packages.insert(debug.shader_package_name.clone());
                    }
                }
            }
        }
        break 'models;
    }
    let mut sorted = packages.into_iter().collect::<Vec<_>>();
    sorted.sort();
    sorted
}

/// 依次探测 vw 文件名的各种数字填充；返回首个命中的 (padding 标签, 路径, 内容)。
fn resolve_vw_avfx(
    resource: &mut SqPackResource,
    model_id: u16,
    body_id: u16,
    vfx_id: u8,
) -> Option<(String, String, Vec<u8>)> {
    for (padding, file) in vw_avfx_path_candidates(model_id, body_id, vfx_id) {
        if let Some(bytes) = resource.read(&file) {
            return Some((padding, file, bytes));
        }
    }
    None
}

/// avfx 顶层块（XFVA 内部）计数。块头为反写 4 字节名 + u32 size，size 按 4 字节对齐推进。
fn avfx_top_level_block_counts(bytes: &[u8]) -> Option<BTreeMap<String, usize>> {
    let blocks = avfx_top_level_blocks(bytes).ok()?;
    let mut counts = BTreeMap::new();
    for name in blocks {
        *counts.entry(name).or_default() += 1;
    }
    Some(counts)
}

fn avfx_top_level_blocks(bytes: &[u8]) -> Result<Vec<String>> {
    if bytes.len() < 8 {
        anyhow::bail!("avfx too short: {} bytes", bytes.len());
    }
    if &bytes[0..4] != b"AVFX" {
        anyhow::bail!(
            "avfx magic mismatch: expected AVFX, got {:?}",
            String::from_utf8_lossy(&bytes[0..4])
        );
    }
    let total = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let end = (8 + total).min(bytes.len());

    let mut names = Vec::new();
    let mut offset = 8;
    while offset + 8 <= end && names.len() < MAX_AVFX_BLOCKS {
        let name = reverse_block_name(&bytes[offset..offset + 4]);
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let data_size = size.div_ceil(4) * 4;
        offset += 8 + data_size;
        names.push(name);
    }
    Ok(names)
}

fn reverse_block_name(raw: &[u8]) -> String {
    let mut name = [0_u8; 4];
    for (index, byte) in raw.iter().rev().take(4).enumerate() {
        name[index] = *byte;
    }
    String::from_utf8_lossy(&name).trim().to_string()
}

fn weapon_body_ids(model: &PackedModelId) -> Vec<u16> {
    let mut body_ids = Vec::new();
    for path in weapon_model_candidate_paths(*model) {
        if let Some(body_id) = body_id_from_model_path(&path) {
            if !body_ids.contains(&body_id) {
                body_ids.push(body_id);
            }
        }
    }
    body_ids
}

fn body_id_from_model_path(path: &str) -> Option<u16> {
    let body = path.split("/obj/body/b").nth(1)?;
    let digits = body.get(..4)?;
    digits.parse().ok()
}

fn vw_avfx_path_candidates(model_id: u16, body_id: u16, vfx_id: u8) -> Vec<(String, String)> {
    let base = format!("chara/weapon/w{model_id:04}/obj/body/b{body_id:04}/vfx/eff/vw");
    let mut digit_forms = Vec::new();
    for digits in [
        format!("{:04}", vfx_id),
        format!("{:03}", vfx_id),
        format!("{:02}", vfx_id),
        vfx_id.to_string(),
    ] {
        if !digit_forms.contains(&digits) {
            digit_forms.push(digits);
        }
    }
    digit_forms
        .into_iter()
        .map(|digits| (format!("vw{digits}"), format!("{base}{digits}.avfx")))
        .collect()
}

fn model_raw_ids(item: &WeaponCatalogItem) -> Vec<u64> {
    let mut raws = vec![item.model_main];
    if item.model_sub != 0 && item.model_sub != item.model_main {
        raws.push(item.model_sub);
    }
    raws
}

fn write_index_markdown(path: &PathBuf, audit: &WeaponVfxAudit) -> Result<()> {
    let mut lines = Vec::new();
    lines.push("# Weapon VFX audit".to_string());
    lines.push(String::new());
    lines.push(format!("- gameDir: {}", audit.game_dir));
    lines.push(format!(
        "- catalog items: {} (unique models: {}, scanned: {})",
        audit.catalog_items, audit.unique_models, audit.scanned_models
    ));
    lines.push(format!(
        "- models with imc: {} (missing parse failures: {})",
        audit.models_with_imc,
        audit.imc_parse_failures.len()
    ));
    lines.push(format!(
        "- models with vfx: {} (entries: {}, distinct vfx ids: {})",
        audit.models_with_vfx, audit.vfx_entry_count, audit.distinct_vfx_ids
    ));
    lines.push(format!(
        "- avfx resolved: {}/{}; missing listed below",
        audit.avfx_found, audit.vfx_entry_count
    ));
    lines.push(format!("- vw padding hits: {:?}", audit.vw_padding_hits));
    lines.push(format!(
        "- avfx top-level blocks: {:?}",
        audit.block_name_counts
    ));
    lines.push(format!(
        "- shpk across vfx models: {:?}",
        audit.shader_package_counts
    ));
    lines.push(String::new());
    lines.push("| model | items | variant | vfx | avfx | blocks | shpk |".to_string());
    lines.push("| --- | --- | ---: | ---: | --- | --- | --- |".to_string());
    for report in &audit.models {
        let items = report
            .items
            .iter()
            .map(|item| format!("{} {}", item.item_id, item.name))
            .collect::<Vec<_>>()
            .join("; ");
        for entry in &report.entries {
            let blocks = entry
                .avfx_block_counts
                .as_ref()
                .map(|counts| {
                    counts
                        .iter()
                        .map(|(name, count)| format!("{name}x{count}"))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "| w{:04}b{:04} | {} | {} | {} | {} | {} | {} |",
                report.model_id,
                entry.body_id,
                items,
                entry.variant,
                entry.vfx_id,
                entry
                    .avfx_path
                    .clone()
                    .unwrap_or_else(|| "MISSING".to_string()),
                blocks,
                report.shader_packages.join(",")
            ));
        }
    }
    if !audit.missing_avfx_entries.is_empty() {
        lines.push(String::new());
        lines.push("## Missing avfx files".to_string());
        for entry in &audit.missing_avfx_entries {
            lines.push(format!(
                "- w{:04}b{:04} variant {} vfx {} (animation {})",
                entry.model_id, entry.body_id, entry.variant, entry.vfx_id, entry.animation
            ));
        }
    }
    fs::write(path, lines.join("\n")).with_context(|| format!("failed to write {}", path.display()))
}

fn game_dir() -> PathBuf {
    std::env::var_os("XIV_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\_ff14\game"))
}

fn vfx_audit_limit() -> Option<usize> {
    std::env::var("XIV_VFX_AUDIT_LIMIT").ok()?.parse().ok()
}

#[test]
fn avfx_block_walker_reads_synthetic_container() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AVFX");
    let mut children = Vec::new();
    // (stored reversed name, data)
    for (name, data) in [
        ("Schd", [0_u8; 4].as_slice()),
        ("Ptcl", [1, 2, 3, 4, 5].as_slice()),
    ] {
        let mut reversed = name.as_bytes().to_vec();
        reversed.reverse();
        children.extend_from_slice(&reversed);
        children.extend_from_slice(&(data.len() as u32).to_le_bytes());
        children.extend_from_slice(data);
        children.resize(children.len().div_ceil(4) * 4, 0);
    }
    bytes.extend_from_slice(&(children.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&children);

    assert_eq!(
        avfx_top_level_blocks(&bytes).expect("synthetic avfx should parse"),
        vec!["Schd".to_string(), "Ptcl".to_string()]
    );
    let counts = avfx_top_level_block_counts(&bytes).expect("counts should parse");
    assert_eq!(counts.get("Schd"), Some(&1));
    assert_eq!(counts.get("Ptcl"), Some(&1));
}

#[test]
fn avfx_block_walker_rejects_bad_magic_and_short_files() {
    assert!(avfx_top_level_blocks(b"NOPE").is_err());
    assert!(avfx_top_level_blocks(&[]).is_err());
    // 容器声明了大小但内容被截断时仍应返回已读到的块
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AVFX");
    bytes.extend_from_slice(&1024_u32.to_le_bytes());
    bytes.extend_from_slice(b"dhcS");
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    assert_eq!(
        avfx_top_level_blocks(&bytes).expect("truncated children should parse"),
        vec!["Schd".to_string()]
    );
}

#[test]
fn vw_candidates_cover_digit_paddings_in_order() {
    let candidates = vw_avfx_path_candidates(2001, 1, 13);
    let paths = candidates
        .iter()
        .map(|(padding, path)| (padding.as_str(), path.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            (
                "vw0013",
                "chara/weapon/w2001/obj/body/b0001/vfx/eff/vw0013.avfx"
            ),
            (
                "vw013",
                "chara/weapon/w2001/obj/body/b0001/vfx/eff/vw013.avfx"
            ),
            (
                "vw13",
                "chara/weapon/w2001/obj/body/b0001/vfx/eff/vw13.avfx"
            ),
        ]
    );
}

#[test]
fn body_id_extraction_reads_weapon_model_paths() {
    assert_eq!(
        body_id_from_model_path("chara/weapon/w2001/obj/body/b0180/model/w2001b0180.mdl"),
        Some(180)
    );
    assert_eq!(
        body_id_from_model_path("chara/equipment/e8903/model/c0101e8903_glv.mdl"),
        None
    );
}
