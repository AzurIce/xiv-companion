#![cfg(feature = "game-data")]

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};
use physis::{
    ReadableFile,
    resource::{Resource, SqPackResource},
};
use serde::Serialize;
use xiv_companion::{
    MaterialShaderFamily, PackedModelId, WeaponCatalogItem,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    material_shader_family, mdl_metadata_from_mdl_bytes, weapon_material_candidate_paths,
    weapon_model_candidate_paths,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFamilyAudit {
    game_dir: String,
    catalog_items: usize,
    unique_models: usize,
    scanned_models: usize,
    scanned_materials: usize,
    family_counts: BTreeMap<String, usize>,
    candidates: Vec<WeaponShaderFamilyCandidate>,
    unclassified_materials: Vec<WeaponShaderFamilyCandidate>,
    resource_collisions: Vec<WeaponMaterialResourceCollision>,
    unresolved_material_references: Vec<WeaponUnresolvedMaterialReference>,
    shape_models: Vec<WeaponShapeModel>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShapeModel {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    shape_count: usize,
    shape_mesh_count: usize,
    shape_value_count: usize,
    shape_names: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFamilyCandidate {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    material_path: String,
    shader_package_name: String,
    shader_family: MaterialShaderFamily,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialResourceCollision {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    candidate_path: String,
    resource_type: String,
    byte_length: usize,
    header: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponUnresolvedMaterialReference {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    candidate_paths: Vec<String>,
}

#[test]
#[ignore = "scans the installed FFXIV WeaponCatalog and writes target/weapon-shader-family-audit.json"]
fn audit_installed_weapon_shader_families() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "weapon-shader-family-audit".to_string(),
    )
    .context("failed to export weapon catalog")?;
    let catalog_items = catalog.items.len();
    let item_ids = item_ids();
    let selected_items = catalog
        .items
        .iter()
        .filter(|item| item_ids.as_ref().is_none_or(|ids| ids.contains(&item.id)))
        .cloned()
        .collect::<Vec<_>>();
    if item_ids.is_some() {
        for item in &selected_items {
            eprintln!(
                "selected item {} {}: main={:016X} {:?}, sub={:016X} {:?}",
                item.id,
                item.name,
                item.model_main,
                item.primary_model(),
                item.model_sub,
                item.secondary_model()
            );
        }
    }
    let models = catalog_models(&selected_items);
    let unique_models = models.len();
    let scan_limit = scan_limit().unwrap_or(unique_models);
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let mut report = WeaponShaderFamilyAudit {
        game_dir: game_dir.display().to_string(),
        catalog_items,
        unique_models,
        scanned_models: 0,
        scanned_materials: 0,
        family_counts: BTreeMap::new(),
        candidates: Vec::new(),
        unclassified_materials: Vec::new(),
        resource_collisions: Vec::new(),
        unresolved_material_references: Vec::new(),
        shape_models: Vec::new(),
        failures: Vec::new(),
    };

    for (index, (model, items)) in models.into_iter().take(scan_limit).enumerate() {
        scan_model(&mut resource, model, &items, &mut report);
        report.scanned_models += 1;
        if (index + 1) % 250 == 0 {
            eprintln!(
                "scanned {}/{} unique weapon models, {} materials, {} bg candidates",
                index + 1,
                scan_limit.min(unique_models),
                report.scanned_materials,
                report.candidates.len()
            );
        }
    }

    let output_path = PathBuf::from("target").join("weapon-shader-family-audit.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    eprintln!(
        "weapon shader audit: models={}, materials={}, candidates={}, failures={}, report={}",
        report.scanned_models,
        report.scanned_materials,
        report.candidates.len(),
        report.failures.len(),
        output_path.display()
    );

    assert!(report.scanned_models > 0);
    assert!(report.scanned_materials > 0);
    Ok(())
}

fn catalog_models(items: &[WeaponCatalogItem]) -> Vec<(PackedModelId, Vec<&WeaponCatalogItem>)> {
    let mut by_model = HashMap::<u64, Vec<&WeaponCatalogItem>>::new();
    for item in items {
        by_model.entry(item.model_main).or_default().push(item);
        if item.model_sub != 0 {
            by_model.entry(item.model_sub).or_default().push(item);
        }
    }
    let mut models = by_model
        .into_iter()
        .map(|(raw, mut items)| {
            items.sort_by_key(|item| std::cmp::Reverse(item.id));
            (PackedModelId::from_raw(raw), items)
        })
        .collect::<Vec<_>>();
    models.sort_by_key(|(model, items)| std::cmp::Reverse((items[0].id, model.raw)));
    models
}

fn scan_model<R: Resource>(
    resource: &mut R,
    model: PackedModelId,
    items: &[&WeaponCatalogItem],
    report: &mut WeaponShaderFamilyAudit,
) {
    let Some((model_path, model_bytes)) = weapon_model_candidate_paths(model)
        .into_iter()
        .find_map(|path| resource.read(&path).map(|bytes| (path, bytes)))
    else {
        report.failures.push(format!(
            "model {:016X} ({}) has no readable candidate",
            model.raw,
            item_label(items)
        ));
        return;
    };
    let metadata = match mdl_metadata_from_mdl_bytes(&model_path, &model_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            report.failures.push(format!(
                "{} ({}) metadata: {error:#}",
                model_path,
                item_label(items)
            ));
            return;
        }
    };
    if !metadata.shapes.is_empty() {
        report.shape_models.push(WeaponShapeModel {
            item_ids: items.iter().map(|item| item.id).collect(),
            item_names: items.iter().map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.clone(),
            shape_count: metadata.shapes.len(),
            shape_mesh_count: metadata.shape_meshes.len(),
            shape_value_count: metadata.shape_values.len(),
            shape_names: metadata
                .shapes
                .iter()
                .filter_map(|shape| shape.name.clone())
                .collect(),
        });
    }

    for material_name in metadata
        .materials
        .iter()
        .filter_map(|material| material.name.as_deref())
    {
        let material_candidates =
            weapon_material_candidate_paths(model, &model_path, material_name);
        let platform = resource.platform();
        let (material, readable_candidates) = first_valid_candidate(
            &material_candidates,
            |path| resource.read(path),
            |bytes| physis::mtrl::Material::from_existing(platform, bytes),
        );
        let Some((material_path, material)) = material else {
            if !readable_candidates.is_empty() {
                report.failures.push(format!(
                    "{} material {} ({}) has no parseable candidate; rejected: {}",
                    model_path,
                    material_name,
                    item_label(items),
                    readable_candidates
                        .iter()
                        .map(|(path, bytes)| format!(
                            "{} [{}; bytes={}; header={}]",
                            path,
                            resource_type_hint(bytes),
                            bytes.len(),
                            hex_prefix(bytes, 32)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            } else {
                report
                    .unresolved_material_references
                    .push(WeaponUnresolvedMaterialReference {
                        item_ids: items.iter().map(|item| item.id).collect(),
                        item_names: items.iter().map(|item| item.name.clone()).collect(),
                        model,
                        model_path: model_path.clone(),
                        material_name: material_name.to_string(),
                        candidate_paths: material_candidates,
                    });
            }
            continue;
        };
        report
            .resource_collisions
            .extend(
                readable_candidates
                    .into_iter()
                    .map(|(candidate_path, bytes)| WeaponMaterialResourceCollision {
                        item_ids: items.iter().map(|item| item.id).collect(),
                        item_names: items.iter().map(|item| item.name.clone()).collect(),
                        model,
                        model_path: model_path.clone(),
                        material_name: material_name.to_string(),
                        candidate_path,
                        resource_type: resource_type_hint(&bytes).to_string(),
                        byte_length: bytes.len(),
                        header: hex_prefix(&bytes, 32),
                    }),
            );
        let shader_package_name = material.shader_package_name;
        let shader_family = material_shader_family(Some(&shader_package_name));
        *report
            .family_counts
            .entry(format!("{shader_family:?}"))
            .or_default() += 1;
        report.scanned_materials += 1;
        let candidate = WeaponShaderFamilyCandidate {
            item_ids: items.iter().map(|item| item.id).collect(),
            item_names: items.iter().map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.clone(),
            material_name: material_name.to_string(),
            material_path,
            shader_package_name,
            shader_family,
        };
        if matches!(
            shader_family,
            MaterialShaderFamily::Bg | MaterialShaderFamily::BgUvScroll
        ) {
            report.candidates.push(candidate);
        } else if shader_family == MaterialShaderFamily::Unknown {
            report.unclassified_materials.push(candidate);
        }
    }
}

fn item_label(items: &[&WeaponCatalogItem]) -> String {
    items
        .first()
        .map(|item| format!("{} {}", item.id, item.name))
        .unwrap_or_else(|| "unknown item".to_string())
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn resource_type_hint(bytes: &[u8]) -> &'static str {
    match bytes.get(..4) {
        Some(b"pap ") => "pap",
        Some(b"mdl ") => "mdl",
        Some(b"shPk") => "shpk",
        _ => "unknown",
    }
}

fn first_valid_candidate<T, Read, Validate>(
    candidates: &[String],
    mut read: Read,
    mut validate: Validate,
) -> (Option<(String, T)>, Vec<(String, Vec<u8>)>)
where
    Read: FnMut(&str) -> Option<Vec<u8>>,
    Validate: FnMut(&[u8]) -> Option<T>,
{
    let mut rejected = Vec::new();
    for path in candidates {
        let Some(bytes) = read(path) else {
            continue;
        };
        if let Some(value) = validate(&bytes) {
            return (Some((path.clone(), value)), rejected);
        }
        rejected.push((path.clone(), bytes));
    }
    (None, rejected)
}

fn scan_limit() -> Option<usize> {
    std::env::var("XIV_WEAPON_SHADER_SCAN_LIMIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|limit| *limit > 0)
}

fn item_ids() -> Option<Vec<u32>> {
    let ids = std::env::var("XIV_WEAPON_SHADER_ITEM_IDS")
        .ok()?
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect::<Vec<_>>();
    (!ids.is_empty()).then_some(ids)
}

fn game_dir() -> PathBuf {
    std::env::var_os("XIV_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\_ff14\game"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_models_deduplicates_primary_and_secondary_models() {
        let item = |id, model_main, model_sub| WeaponCatalogItem {
            id,
            name: format!("item-{id}"),
            description: String::new(),
            icon: 0,
            item_ui_category: 1,
            item_search_category: 1,
            equip_slot_category: 1,
            price_mid: 0,
            price_low: 0,
            model_main,
            model_sub,
        };
        let items = [item(10, 100, 200), item(20, 100, 0), item(30, 300, 200)];
        let models = catalog_models(&items);

        assert_eq!(models.len(), 3);
        assert_eq!(models[0].0.raw, 300);
        assert_eq!(models[1].0.raw, 200);
        assert_eq!(models[2].0.raw, 100);
        assert_eq!(
            models[2].1.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 10]
        );
    }

    #[test]
    fn resource_type_hint_identifies_pap_hash_collisions() {
        assert_eq!(resource_type_hint(b"pap \x01\x00"), "pap");
        assert_eq!(resource_type_hint(&[0, 0, 3, 1]), "unknown");
    }

    #[test]
    fn candidate_validation_continues_after_wrong_resource_type() {
        let candidates = vec!["collision".to_string(), "material".to_string()];
        let (selected, rejected) = first_valid_candidate(
            &candidates,
            |path| match path {
                "collision" => Some(b"pap ".to_vec()),
                "material" => Some(vec![0, 0, 3, 1]),
                _ => None,
            },
            |bytes| (bytes == [0, 0, 3, 1]).then_some("mtrl"),
        );

        assert_eq!(selected, Some(("material".to_string(), "mtrl")));
        assert_eq!(rejected, vec![("collision".to_string(), b"pap ".to_vec())]);
    }
}
