#![cfg(feature = "game-data")]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
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
use xiv_companion_data::{
    MaterialSamplerLogicalRole, ModelTextureKind, ShaderPackageKeyDefaultDebug,
    ShaderPackageMaterialConstantDebug, ShaderPackageSamplerResourceDebug,
    ShaderPackageSemanticDebug, material_debug_info_from_mtrl_bytes,
    shader_package_semantic_debug_from_resource,
};

const MAX_SEMANTIC_REPRESENTATIVES: usize = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFamilyAudit {
    game_dir: String,
    catalog_items: usize,
    unique_models: usize,
    scanned_models: usize,
    scanned_materials: usize,
    unique_material_resources: usize,
    unique_shader_packages: usize,
    family_counts: BTreeMap<String, usize>,
    sampler_coverage: Vec<WeaponMaterialSamplerCoverage>,
    material_key_coverage: Vec<WeaponMaterialKeyCoverage>,
    material_constant_coverage: Vec<WeaponMaterialConstantCoverage>,
    unknown_key_category_count: usize,
    unknown_key_value_count: usize,
    unknown_constant_id_count: usize,
    unknown_sampler_role_count: usize,
    unresolved_sampler_name_count: usize,
    candidates: Vec<WeaponShaderFamilyCandidate>,
    unclassified_materials: Vec<WeaponShaderFamilyCandidate>,
    resource_collisions: Vec<WeaponMaterialResourceCollision>,
    unresolved_material_references: Vec<WeaponUnresolvedMaterialReference>,
    shape_models: Vec<WeaponShapeModel>,
    semantic_failures: Vec<String>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialSamplerCoverage {
    shader_package_name: String,
    texture_usage: u32,
    texture_usage_hex: String,
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    flags: u32,
    flags_hex: String,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
enum WeaponShaderKeyScope {
    Material,
    System,
    Scene,
    MaterialOverrideOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponSemanticRepresentative {
    item_reference_count: usize,
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    material_path: String,
    shader_flags: u32,
    shader_flags_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialKeyValueCount {
    value: u32,
    value_hex: String,
    value_name: Option<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialKeyCoverage {
    shader_package_name: String,
    scope: WeaponShaderKeyScope,
    category: u32,
    category_hex: String,
    category_name: Option<String>,
    default_value: Option<u32>,
    default_value_hex: Option<String>,
    default_value_name: Option<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    material_override_resource_count: usize,
    material_override_reference_count: usize,
    non_default_override_resource_count: usize,
    non_default_override_reference_count: usize,
    observed_values: Vec<WeaponMaterialKeyValueCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialConstantValueCount {
    values: Vec<Option<f32>>,
    raw_values_hex: Vec<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFlagCount {
    shader_flags: u32,
    shader_flags_hex: String,
    material_resource_count: usize,
    material_reference_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialConstantCoverage {
    shader_package_name: String,
    id: u32,
    id_hex: String,
    name: Option<String>,
    package_byte_offset: Option<u16>,
    package_byte_size: Option<u16>,
    default_values: Option<Vec<Option<f32>>>,
    default_raw_values_hex: Option<Vec<String>>,
    material_resource_count: usize,
    material_reference_count: usize,
    material_override_resource_count: usize,
    material_override_reference_count: usize,
    non_default_override_resource_count: usize,
    non_default_override_reference_count: usize,
    malformed_override_resource_count: usize,
    malformed_override_reference_count: usize,
    non_finite_resource_count: usize,
    non_finite_reference_count: usize,
    unresolved_value_resource_count: usize,
    unresolved_value_reference_count: usize,
    value_width_resource_counts: BTreeMap<usize, usize>,
    malformed_override_value_size_resource_counts: BTreeMap<u16, usize>,
    observed_values: Vec<WeaponMaterialConstantValueCount>,
    shader_flag_counts: Vec<WeaponShaderFlagCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SemanticCount {
    resources: usize,
    references: usize,
}

impl SemanticCount {
    fn observe(&mut self, unique_resource: bool, references: usize) {
        self.references += references;
        if unique_resource {
            self.resources += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialKeyCoverageId {
    shader_package_name: String,
    scope: WeaponShaderKeyScope,
    category: u32,
}

#[derive(Debug)]
struct MaterialKeyValueAccumulator {
    value_name: Option<String>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug)]
struct MaterialKeyCoverageAccumulator {
    category_name: Option<String>,
    default_value: Option<u32>,
    default_value_name: Option<String>,
    count: SemanticCount,
    override_count: SemanticCount,
    non_default_override_count: SemanticCount,
    observed_values: BTreeMap<u32, MaterialKeyValueAccumulator>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialConstantCoverageId {
    shader_package_name: String,
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialSamplerCoverageId {
    shader_package_name: String,
    texture_usage: u32,
    flags: u32,
}

#[derive(Debug)]
struct MaterialSamplerCoverageAccumulator {
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialConstantValueKey(Vec<u32>);

#[derive(Debug)]
struct MaterialConstantValueAccumulator {
    values: Vec<f32>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

impl MaterialConstantValueAccumulator {
    fn observe(&mut self, unique_resource: bool, representative: &WeaponSemanticRepresentative) {
        self.count
            .observe(unique_resource, representative.item_reference_count);
        add_semantic_representative(&mut self.representatives, representative);
    }
}

#[derive(Debug)]
struct MaterialConstantCoverageAccumulator {
    name: Option<String>,
    package_byte_offset: Option<u16>,
    package_byte_size: Option<u16>,
    default_values: Option<Vec<f32>>,
    count: SemanticCount,
    override_count: SemanticCount,
    non_default_override_count: SemanticCount,
    malformed_override_count: SemanticCount,
    non_finite_count: SemanticCount,
    unresolved_value_count: SemanticCount,
    value_width_resource_counts: BTreeMap<usize, usize>,
    malformed_override_value_size_resource_counts: BTreeMap<u16, usize>,
    observed_values: BTreeMap<MaterialConstantValueKey, MaterialConstantValueAccumulator>,
    shader_flag_counts: BTreeMap<u32, SemanticCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug)]
struct ObservedMaterialKey {
    category: u32,
    category_name: Option<String>,
    value: u32,
    value_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ObservedMaterialConstant {
    id: u32,
    name: Option<String>,
    values: Vec<f32>,
    value_size: u16,
    malformed: bool,
    resolved: bool,
}

#[derive(Debug)]
struct ObservedMaterialConstantGroup {
    name: Option<String>,
    effective_values: Option<Vec<f32>>,
    malformed_value_sizes: BTreeSet<u16>,
    non_finite: bool,
}

#[derive(Clone, Debug)]
struct ObservedMaterialSampler {
    texture_usage: u32,
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    flags: u32,
}

#[derive(Default)]
struct MaterialSemanticCoverageBuilder {
    material_resources: HashSet<String>,
    shader_packages: HashSet<String>,
    shader_package_cache: HashMap<String, Option<ShaderPackageSemanticDebug>>,
    sampler_coverage: BTreeMap<MaterialSamplerCoverageId, MaterialSamplerCoverageAccumulator>,
    key_coverage: BTreeMap<MaterialKeyCoverageId, MaterialKeyCoverageAccumulator>,
    constant_coverage: BTreeMap<MaterialConstantCoverageId, MaterialConstantCoverageAccumulator>,
    failures: Vec<String>,
}

struct MaterialSemanticCoverageResult {
    unique_material_resources: usize,
    unique_shader_packages: usize,
    sampler_coverage: Vec<WeaponMaterialSamplerCoverage>,
    material_key_coverage: Vec<WeaponMaterialKeyCoverage>,
    material_constant_coverage: Vec<WeaponMaterialConstantCoverage>,
    unknown_key_category_count: usize,
    unknown_key_value_count: usize,
    unknown_constant_id_count: usize,
    unknown_sampler_role_count: usize,
    unresolved_sampler_name_count: usize,
    failures: Vec<String>,
}

impl MaterialSemanticCoverageBuilder {
    #[allow(clippy::too_many_arguments)]
    fn record_material<R: Resource>(
        &mut self,
        resource: &mut R,
        model: PackedModelId,
        items: &[&WeaponCatalogItem],
        model_path: &str,
        material_name: &str,
        material_path: &str,
        shader_package_name: &str,
        material_bytes: &[u8],
    ) {
        let debug = match material_debug_info_from_mtrl_bytes(material_path, material_bytes) {
            Ok(debug) => debug,
            Err(error) => {
                self.failures.push(format!(
                    "{} material semantic debug ({}) failed: {error:#}",
                    material_path,
                    item_label(items)
                ));
                return;
            }
        };

        self.shader_packages.insert(shader_package_name.to_string());
        if !self.shader_package_cache.contains_key(shader_package_name) {
            let package =
                match shader_package_semantic_debug_from_resource(resource, shader_package_name) {
                    Ok(package) => Some(package),
                    Err(error) => {
                        self.failures.push(format!(
                            "shader package {} ({}) failed: {error:#}",
                            shader_package_name,
                            item_label(items)
                        ));
                        None
                    }
                };
            self.shader_package_cache
                .insert(shader_package_name.to_string(), package);
        }
        let shader_package = self
            .shader_package_cache
            .get(shader_package_name)
            .and_then(Clone::clone);

        let keys = debug
            .summary
            .shader_keys
            .iter()
            .map(|key| ObservedMaterialKey {
                category: key.category,
                category_name: key.category_name.clone(),
                value: key.value,
                value_name: key.value_name.clone(),
            })
            .collect::<Vec<_>>();
        let constant_names = debug
            .summary
            .constants
            .iter()
            .map(|constant| (constant.id, constant.name.clone()))
            .collect::<HashMap<_, _>>();
        let constants = debug
            .constants
            .iter()
            .map(|constant| {
                let expected_count = usize::from(constant.value_size) / 4;
                ObservedMaterialConstant {
                    id: constant.id,
                    name: constant_names.get(&constant.id).cloned().flatten(),
                    values: constant.values.clone(),
                    value_size: constant.value_size,
                    malformed: constant.value_size < 4
                        || usize::from(constant.value_size) % 4 != 0
                        || constant.values.len() != expected_count,
                    resolved: constant.value_size >= 4 && constant.values.len() == expected_count,
                }
            })
            .collect::<Vec<_>>();
        let samplers = debug
            .samplers
            .iter()
            .map(|sampler| ObservedMaterialSampler {
                texture_usage: sampler.texture_usage,
                texture_usage_name: sampler.texture_usage_name.clone(),
                logical_role: sampler.logical_role,
                texture_kind: sampler.kind,
                flags: sampler.flags,
            })
            .collect::<Vec<_>>();
        let representative = WeaponSemanticRepresentative {
            item_reference_count: items.len(),
            item_ids: items.iter().take(3).map(|item| item.id).collect(),
            item_names: items.iter().take(3).map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.to_string(),
            material_name: material_name.to_string(),
            material_path: material_path.to_string(),
            shader_flags: debug.shader_flags,
            shader_flags_hex: hex_u32(debug.shader_flags),
        };

        self.observe_material(
            shader_package_name,
            shader_package.as_ref(),
            material_path,
            debug.shader_flags,
            &keys,
            &constants,
            &samplers,
            &representative,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_material(
        &mut self,
        shader_package_name: &str,
        shader_package: Option<&ShaderPackageSemanticDebug>,
        material_path: &str,
        shader_flags: u32,
        material_keys: &[ObservedMaterialKey],
        material_constants: &[ObservedMaterialConstant],
        material_samplers: &[ObservedMaterialSampler],
        representative: &WeaponSemanticRepresentative,
    ) {
        let unique_resource = self.material_resources.insert(material_path.to_string());
        self.shader_packages.insert(shader_package_name.to_string());
        let mut observed_sampler_ids = BTreeSet::new();
        for sampler in material_samplers {
            let package_resource = shader_package.and_then(|package| {
                package
                    .sampler_resources
                    .iter()
                    .find(|resource| resource.crc == sampler.texture_usage)
            });
            let id = MaterialSamplerCoverageId {
                shader_package_name: shader_package_name.to_string(),
                texture_usage: sampler.texture_usage,
                flags: sampler.flags,
            };
            if !observed_sampler_ids.insert(id.clone()) {
                continue;
            }
            let coverage = self.sampler_coverage.entry(id).or_insert_with(|| {
                MaterialSamplerCoverageAccumulator {
                    texture_usage_name: package_resource
                        .map(|resource| resource.name.clone())
                        .or_else(|| sampler.texture_usage_name.clone()),
                    logical_role: package_resource
                        .and_then(|resource| resource.logical_role)
                        .or(sampler.logical_role),
                    texture_kind: package_resource
                        .and_then(|resource| resource.kind)
                        .or(sampler.texture_kind),
                    count: SemanticCount::default(),
                    representatives: Vec::new(),
                }
            });
            coverage
                .count
                .observe(unique_resource, representative.item_reference_count);
            add_semantic_representative(&mut coverage.representatives, representative);
        }
        let key_overrides = material_keys
            .iter()
            .map(|key| (key.category, key))
            .collect::<HashMap<_, _>>();
        let mut constant_overrides = BTreeMap::<u32, ObservedMaterialConstantGroup>::new();
        for constant in material_constants {
            let group = constant_overrides.entry(constant.id).or_insert_with(|| {
                ObservedMaterialConstantGroup {
                    name: constant.name.clone(),
                    effective_values: None,
                    malformed_value_sizes: BTreeSet::new(),
                    non_finite: false,
                }
            });
            if group.name.is_none() {
                group.name = constant.name.clone();
            }
            if constant.malformed {
                group.malformed_value_sizes.insert(constant.value_size);
            }
            group.non_finite |= constant.values.iter().any(|value| !value.is_finite());
            if constant.resolved {
                group.effective_values = Some(constant.values.clone());
            }
        }
        let mut package_key_categories = HashSet::new();
        let mut package_constant_ids = HashSet::new();

        if let Some(shader_package) = shader_package {
            for key in &shader_package.material_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::Material,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }
            for key in &shader_package.system_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::System,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }
            for key in &shader_package.scene_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::Scene,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }

            for constant in &shader_package.material_constants {
                package_constant_ids.insert(constant.id);
                self.observe_constant(
                    shader_package_name,
                    constant.id,
                    constant.name.clone(),
                    Some(constant.byte_offset),
                    Some(constant.byte_size),
                    constant.default_values.as_deref(),
                    constant_overrides.get(&constant.id),
                    shader_flags,
                    unique_resource,
                    representative,
                );
            }
        }

        for key in material_keys
            .iter()
            .filter(|key| !package_key_categories.contains(&key.category))
        {
            self.observe_key(
                shader_package_name,
                WeaponShaderKeyScope::MaterialOverrideOnly,
                key.category,
                key.category_name.clone(),
                None,
                None,
                key.value,
                key.value_name.clone(),
                Some(key.value),
                unique_resource,
                representative,
            );
        }

        for (id, constant) in constant_overrides
            .iter()
            .filter(|(id, _)| !package_constant_ids.contains(id))
        {
            self.observe_constant(
                shader_package_name,
                *id,
                constant.name.clone(),
                None,
                None,
                None,
                Some(constant),
                shader_flags,
                unique_resource,
                representative,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_key(
        &mut self,
        shader_package_name: &str,
        scope: WeaponShaderKeyScope,
        category: u32,
        category_name: Option<String>,
        default_value: Option<u32>,
        default_value_name: Option<String>,
        effective_value: u32,
        effective_value_name: Option<String>,
        override_value: Option<u32>,
        unique_resource: bool,
        representative: &WeaponSemanticRepresentative,
    ) {
        let id = MaterialKeyCoverageId {
            shader_package_name: shader_package_name.to_string(),
            scope,
            category,
        };
        let coverage =
            self.key_coverage
                .entry(id)
                .or_insert_with(|| MaterialKeyCoverageAccumulator {
                    category_name: category_name.clone(),
                    default_value,
                    default_value_name: default_value_name.clone(),
                    count: SemanticCount::default(),
                    override_count: SemanticCount::default(),
                    non_default_override_count: SemanticCount::default(),
                    observed_values: BTreeMap::new(),
                    representatives: Vec::new(),
                });
        if coverage.category_name.is_none() {
            coverage.category_name = category_name;
        }
        if coverage.default_value_name.is_none() {
            coverage.default_value_name = default_value_name;
        }
        coverage
            .count
            .observe(unique_resource, representative.item_reference_count);
        if let Some(override_value) = override_value {
            coverage
                .override_count
                .observe(unique_resource, representative.item_reference_count);
            if default_value != Some(override_value) {
                coverage
                    .non_default_override_count
                    .observe(unique_resource, representative.item_reference_count);
            }
        }
        let value = coverage
            .observed_values
            .entry(effective_value)
            .or_insert_with(|| MaterialKeyValueAccumulator {
                value_name: effective_value_name.clone(),
                count: SemanticCount::default(),
                representatives: Vec::new(),
            });
        if value.value_name.is_none() {
            value.value_name = effective_value_name;
        }
        value
            .count
            .observe(unique_resource, representative.item_reference_count);
        add_semantic_representative(&mut value.representatives, representative);
        add_semantic_representative(&mut coverage.representatives, representative);
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_constant(
        &mut self,
        shader_package_name: &str,
        id: u32,
        name: Option<String>,
        package_byte_offset: Option<u16>,
        package_byte_size: Option<u16>,
        default_values: Option<&[f32]>,
        override_constant: Option<&ObservedMaterialConstantGroup>,
        shader_flags: u32,
        unique_resource: bool,
        representative: &WeaponSemanticRepresentative,
    ) {
        let coverage_id = MaterialConstantCoverageId {
            shader_package_name: shader_package_name.to_string(),
            id,
        };
        let coverage = self
            .constant_coverage
            .entry(coverage_id)
            .or_insert_with(|| MaterialConstantCoverageAccumulator {
                name: name.clone(),
                package_byte_offset,
                package_byte_size,
                default_values: default_values.map(<[f32]>::to_vec),
                count: SemanticCount::default(),
                override_count: SemanticCount::default(),
                non_default_override_count: SemanticCount::default(),
                malformed_override_count: SemanticCount::default(),
                non_finite_count: SemanticCount::default(),
                unresolved_value_count: SemanticCount::default(),
                value_width_resource_counts: BTreeMap::new(),
                malformed_override_value_size_resource_counts: BTreeMap::new(),
                observed_values: BTreeMap::new(),
                shader_flag_counts: BTreeMap::new(),
                representatives: Vec::new(),
            });
        if coverage.name.is_none() {
            coverage.name = name;
        }
        if coverage.package_byte_offset.is_none() {
            coverage.package_byte_offset = package_byte_offset;
        }
        if coverage.package_byte_size.is_none() {
            coverage.package_byte_size = package_byte_size;
        }
        let reference_count = representative.item_reference_count;
        coverage.count.observe(unique_resource, reference_count);
        let override_values = override_constant
            .and_then(|constant| constant.effective_values.as_ref().map(Vec::as_slice));
        let effective_values = override_values.or(default_values);
        if let Some(override_constant) = override_constant {
            coverage
                .override_count
                .observe(unique_resource, reference_count);
            if override_values.is_some_and(|values| {
                default_values.is_none_or(|default| !same_f32_bits(default, values))
            }) {
                coverage
                    .non_default_override_count
                    .observe(unique_resource, reference_count);
            }
            if !override_constant.malformed_value_sizes.is_empty() {
                coverage
                    .malformed_override_count
                    .observe(unique_resource, reference_count);
                if unique_resource {
                    for value_size in &override_constant.malformed_value_sizes {
                        *coverage
                            .malformed_override_value_size_resource_counts
                            .entry(*value_size)
                            .or_default() += 1;
                    }
                }
            }
        }
        if override_constant.is_some_and(|constant| constant.non_finite)
            || effective_values.is_some_and(|values| values.iter().any(|value| !value.is_finite()))
        {
            coverage
                .non_finite_count
                .observe(unique_resource, reference_count);
        }
        if let Some(effective_values) = effective_values {
            if unique_resource {
                *coverage
                    .value_width_resource_counts
                    .entry(effective_values.len())
                    .or_default() += 1;
            }
            let value_key = MaterialConstantValueKey(
                effective_values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            );
            coverage
                .observed_values
                .entry(value_key)
                .or_insert_with(|| MaterialConstantValueAccumulator {
                    values: effective_values.to_vec(),
                    count: SemanticCount::default(),
                    representatives: Vec::new(),
                })
                .observe(unique_resource, representative);
        } else {
            coverage
                .unresolved_value_count
                .observe(unique_resource, reference_count);
        }
        coverage
            .shader_flag_counts
            .entry(shader_flags)
            .or_default()
            .observe(unique_resource, reference_count);
        add_semantic_representative(&mut coverage.representatives, representative);
    }

    fn finish(self) -> MaterialSemanticCoverageResult {
        let sampler_coverage = self
            .sampler_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialSamplerCoverage {
                shader_package_name: id.shader_package_name,
                texture_usage: id.texture_usage,
                texture_usage_hex: hex_u32(id.texture_usage),
                texture_usage_name: coverage.texture_usage_name,
                logical_role: coverage.logical_role,
                texture_kind: coverage.texture_kind,
                flags: id.flags,
                flags_hex: hex_u32(id.flags),
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let material_key_coverage = self
            .key_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialKeyCoverage {
                shader_package_name: id.shader_package_name,
                scope: id.scope,
                category: id.category,
                category_hex: hex_u32(id.category),
                category_name: coverage.category_name,
                default_value: coverage.default_value,
                default_value_hex: coverage.default_value.map(hex_u32),
                default_value_name: coverage.default_value_name,
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                material_override_resource_count: coverage.override_count.resources,
                material_override_reference_count: coverage.override_count.references,
                non_default_override_resource_count: coverage.non_default_override_count.resources,
                non_default_override_reference_count: coverage
                    .non_default_override_count
                    .references,
                observed_values: coverage
                    .observed_values
                    .into_iter()
                    .map(|(value, count)| WeaponMaterialKeyValueCount {
                        value,
                        value_hex: hex_u32(value),
                        value_name: count.value_name,
                        material_resource_count: count.count.resources,
                        material_reference_count: count.count.references,
                        representatives: count.representatives,
                    })
                    .collect(),
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let material_constant_coverage = self
            .constant_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialConstantCoverage {
                shader_package_name: id.shader_package_name,
                id: id.id,
                id_hex: hex_u32(id.id),
                name: coverage.name,
                package_byte_offset: coverage.package_byte_offset,
                package_byte_size: coverage.package_byte_size,
                default_values: coverage.default_values.as_deref().map(json_f32_values),
                default_raw_values_hex: coverage.default_values.as_deref().map(f32_raw_values_hex),
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                material_override_resource_count: coverage.override_count.resources,
                material_override_reference_count: coverage.override_count.references,
                non_default_override_resource_count: coverage.non_default_override_count.resources,
                non_default_override_reference_count: coverage
                    .non_default_override_count
                    .references,
                malformed_override_resource_count: coverage.malformed_override_count.resources,
                malformed_override_reference_count: coverage.malformed_override_count.references,
                non_finite_resource_count: coverage.non_finite_count.resources,
                non_finite_reference_count: coverage.non_finite_count.references,
                unresolved_value_resource_count: coverage.unresolved_value_count.resources,
                unresolved_value_reference_count: coverage.unresolved_value_count.references,
                value_width_resource_counts: coverage.value_width_resource_counts,
                malformed_override_value_size_resource_counts: coverage
                    .malformed_override_value_size_resource_counts,
                observed_values: coverage
                    .observed_values
                    .into_values()
                    .map(|count| WeaponMaterialConstantValueCount {
                        values: json_f32_values(&count.values),
                        raw_values_hex: f32_raw_values_hex(&count.values),
                        material_resource_count: count.count.resources,
                        material_reference_count: count.count.references,
                        representatives: count.representatives,
                    })
                    .collect(),
                shader_flag_counts: coverage
                    .shader_flag_counts
                    .into_iter()
                    .map(|(shader_flags, count)| WeaponShaderFlagCount {
                        shader_flags,
                        shader_flags_hex: hex_u32(shader_flags),
                        material_resource_count: count.resources,
                        material_reference_count: count.references,
                    })
                    .collect(),
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let unknown_key_category_count = material_key_coverage
            .iter()
            .filter(|coverage| coverage.category_name.is_none())
            .count();
        let unknown_key_value_count = material_key_coverage
            .iter()
            .flat_map(|coverage| &coverage.observed_values)
            .filter(|value| value.value_name.is_none())
            .count();
        let unknown_constant_id_count = material_constant_coverage
            .iter()
            .filter(|coverage| coverage.name.is_none())
            .count();
        let unknown_sampler_role_count = sampler_coverage
            .iter()
            .filter(|coverage| coverage.logical_role.is_none())
            .count();
        let unresolved_sampler_name_count = sampler_coverage
            .iter()
            .filter(|coverage| coverage.texture_usage_name.is_none())
            .count();

        MaterialSemanticCoverageResult {
            unique_material_resources: self.material_resources.len(),
            unique_shader_packages: self.shader_packages.len(),
            sampler_coverage,
            material_key_coverage,
            material_constant_coverage,
            unknown_key_category_count,
            unknown_key_value_count,
            unknown_constant_id_count,
            unknown_sampler_role_count,
            unresolved_sampler_name_count,
            failures: self.failures,
        }
    }
}

fn add_semantic_representative(
    representatives: &mut Vec<WeaponSemanticRepresentative>,
    representative: &WeaponSemanticRepresentative,
) {
    if representatives.len() >= MAX_SEMANTIC_REPRESENTATIVES
        || representatives
            .iter()
            .any(|existing| existing.material_path == representative.material_path)
    {
        return;
    }
    representatives.push(representative.clone());
}

fn same_f32_bits(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn json_f32_values(values: &[f32]) -> Vec<Option<f32>> {
    values
        .iter()
        .map(|value| value.is_finite().then_some(*value))
        .collect()
}

fn f32_raw_values_hex(values: &[f32]) -> Vec<String> {
    values
        .iter()
        .map(|value| hex_u32(value.to_bits()))
        .collect()
}

fn hex_u32(value: u32) -> String {
    format!("0x{value:08x}")
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
    let mut semantic_coverage = MaterialSemanticCoverageBuilder::default();
    let mut report = WeaponShaderFamilyAudit {
        game_dir: game_dir.display().to_string(),
        catalog_items,
        unique_models,
        scanned_models: 0,
        scanned_materials: 0,
        unique_material_resources: 0,
        unique_shader_packages: 0,
        family_counts: BTreeMap::new(),
        sampler_coverage: Vec::new(),
        material_key_coverage: Vec::new(),
        material_constant_coverage: Vec::new(),
        unknown_key_category_count: 0,
        unknown_key_value_count: 0,
        unknown_constant_id_count: 0,
        unknown_sampler_role_count: 0,
        unresolved_sampler_name_count: 0,
        candidates: Vec::new(),
        unclassified_materials: Vec::new(),
        resource_collisions: Vec::new(),
        unresolved_material_references: Vec::new(),
        shape_models: Vec::new(),
        semantic_failures: Vec::new(),
        failures: Vec::new(),
    };

    for (index, (model, items)) in models.into_iter().take(scan_limit).enumerate() {
        scan_model(
            &mut resource,
            model,
            &items,
            &mut report,
            &mut semantic_coverage,
        );
        report.scanned_models += 1;
        if (index + 1) % 250 == 0 {
            eprintln!(
                "scanned {}/{} unique weapon models, {} material references, {} unique material resources, {} bg candidates",
                index + 1,
                scan_limit.min(unique_models),
                report.scanned_materials,
                semantic_coverage.material_resources.len(),
                report.candidates.len()
            );
        }
    }

    let semantic_coverage = semantic_coverage.finish();
    report.unique_material_resources = semantic_coverage.unique_material_resources;
    report.unique_shader_packages = semantic_coverage.unique_shader_packages;
    report.sampler_coverage = semantic_coverage.sampler_coverage;
    report.material_key_coverage = semantic_coverage.material_key_coverage;
    report.material_constant_coverage = semantic_coverage.material_constant_coverage;
    report.unknown_key_category_count = semantic_coverage.unknown_key_category_count;
    report.unknown_key_value_count = semantic_coverage.unknown_key_value_count;
    report.unknown_constant_id_count = semantic_coverage.unknown_constant_id_count;
    report.unknown_sampler_role_count = semantic_coverage.unknown_sampler_role_count;
    report.unresolved_sampler_name_count = semantic_coverage.unresolved_sampler_name_count;
    report.semantic_failures = semantic_coverage.failures;

    let output_path = PathBuf::from("target").join("weapon-shader-family-audit.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    eprintln!(
        "weapon shader audit: models={}, material references={}, unique materials={}, shader packages={}, sampler coverage={}, key coverage={}, constant coverage={}, candidates={}, failures={}, semantic failures={}, report={}",
        report.scanned_models,
        report.scanned_materials,
        report.unique_material_resources,
        report.unique_shader_packages,
        report.sampler_coverage.len(),
        report.material_key_coverage.len(),
        report.material_constant_coverage.len(),
        report.candidates.len(),
        report.failures.len(),
        report.semantic_failures.len(),
        output_path.display()
    );

    assert!(report.scanned_models > 0);
    assert!(report.scanned_materials > 0);
    assert!(report.unique_material_resources > 0);
    assert!(report.unique_shader_packages > 0);
    assert!(!report.sampler_coverage.is_empty());
    assert!(!report.material_key_coverage.is_empty());
    assert!(!report.material_constant_coverage.is_empty());
    assert!(
        report.failures.is_empty(),
        "weapon audit failure: {:?}",
        report.failures.first()
    );
    assert!(
        report.semantic_failures.is_empty(),
        "material semantic audit failure: {:?}",
        report.semantic_failures.first()
    );
    if item_ids.is_none() && scan_limit >= unique_models {
        assert_installed_special_character_boundary(&report);
    }
    Ok(())
}

fn assert_installed_special_character_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(
        report.family_counts,
        BTreeMap::from([
            ("Character".to_string(), 8091),
            ("CharacterGlass".to_string(), 6),
            ("Skin".to_string(), 15),
        ]),
        "installed special-family coverage changed; re-audit family semantics before extending the renderer"
    );

    let skin_key = |category_name: &str| {
        report
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == "skin.shpk"
                    && coverage.category_name.as_deref() == Some(category_name)
            })
            .unwrap_or_else(|| panic!("skin.shpk is missing {category_name} coverage"))
    };
    let material_value = skin_key("GetMaterialValue");
    assert_eq!(material_value.material_resource_count, 1);
    assert_eq!(material_value.material_reference_count, 35);
    assert!(material_value.representatives.iter().any(|representative| {
        representative.material_path
            == "chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl"
    }));
    assert_eq!(material_value.observed_values.len(), 1);
    assert_eq!(
        material_value.observed_values[0].value_name.as_deref(),
        Some("GetMaterialValueBody")
    );

    let decal_color = skin_key("GetDecalColor");
    assert_eq!(decal_color.material_resource_count, 1);
    assert_eq!(decal_color.material_reference_count, 35);
    assert_eq!(decal_color.observed_values.len(), 1);
    assert_eq!(
        decal_color.observed_values[0].value_name.as_deref(),
        Some("GetDecalColorOff")
    );

    let skin_samplers = report
        .sampler_coverage
        .iter()
        .filter(|coverage| coverage.shader_package_name == "skin.shpk")
        .collect::<Vec<_>>();
    assert_eq!(skin_samplers.len(), 3);
    assert_eq!(
        skin_samplers
            .iter()
            .filter_map(|coverage| coverage.texture_usage_name.as_deref())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["g_SamplerDiffuse", "g_SamplerMask", "g_SamplerNormal"])
    );
    assert!(skin_samplers.iter().all(|coverage| {
        coverage.material_resource_count == 1 && coverage.material_reference_count == 35
    }));
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
    semantic_coverage: &mut MaterialSemanticCoverageBuilder,
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
        let Some((material_path, material_bytes, material)) = material else {
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
        semantic_coverage.record_material(
            resource,
            model,
            items,
            &model_path,
            material_name,
            &material_path,
            &shader_package_name,
            &material_bytes,
        );
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
) -> (Option<(String, Vec<u8>, T)>, Vec<(String, Vec<u8>)>)
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
            return (Some((path.clone(), bytes, value)), rejected);
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

        assert_eq!(
            selected,
            Some(("material".to_string(), vec![0, 0, 3, 1], "mtrl"))
        );
        assert_eq!(rejected, vec![("collision".to_string(), b"pap ".to_vec())]);
    }

    #[test]
    fn semantic_coverage_separates_scopes_and_deduplicates_resources() {
        let package = ShaderPackageSemanticDebug {
            path: "shader/sm5/shpk/character.shpk".to_string(),
            name: "character.shpk".to_string(),
            sampler_resources: Vec::new(),
            material_keys: vec![test_package_key(
                0x100,
                Some("MaterialKey"),
                10,
                Some("MaterialDefault"),
            )],
            system_keys: vec![test_package_key(
                0x200,
                Some("SystemKey"),
                20,
                Some("SystemDefault"),
            )],
            scene_keys: vec![test_package_key(
                0x300,
                Some("SceneKey"),
                30,
                Some("SceneDefault"),
            )],
            material_constants: vec![
                test_package_constant(0x500, Some("g_Known"), 0, 4, Some(vec![0.0])),
                test_package_constant(0x900, Some("g_NoDefault"), 4, 4, None),
                test_package_constant(0xA00, Some("g_ZeroWidth"), 8, 0, Some(Vec::new())),
            ],
        };
        let override_keys = vec![
            ObservedMaterialKey {
                category: 0x100,
                category_name: Some("MaterialKey".to_string()),
                value: 11,
                value_name: None,
            },
            ObservedMaterialKey {
                category: 0x300,
                category_name: Some("SceneKey".to_string()),
                value: 31,
                value_name: Some("SceneOverride".to_string()),
            },
            ObservedMaterialKey {
                category: 0x400,
                category_name: None,
                value: 40,
                value_name: None,
            },
        ];
        let override_constants = vec![
            ObservedMaterialConstant {
                id: 0x500,
                name: Some("g_Known".to_string()),
                values: vec![1.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x500,
                name: Some("g_Known".to_string()),
                values: Vec::new(),
                value_size: 2,
                malformed: true,
                resolved: false,
            },
            ObservedMaterialConstant {
                id: 0x600,
                name: None,
                values: vec![2.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x600,
                name: None,
                values: vec![3.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x700,
                name: Some("g_NonFinite".to_string()),
                values: vec![f32::NAN],
                value_size: 6,
                malformed: true,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x800,
                name: Some("g_Malformed".to_string()),
                values: Vec::new(),
                value_size: 2,
                malformed: true,
                resolved: false,
            },
            ObservedMaterialConstant {
                id: 0x900,
                name: Some("g_NoDefault".to_string()),
                values: vec![4.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
        ];
        let first = test_representative("a.mtrl", 0x11, 3);
        let second = test_representative("b.mtrl", 0x01, 2);
        let mut builder = MaterialSemanticCoverageBuilder::default();

        for _ in 0..2 {
            builder.observe_material(
                "character.shpk",
                Some(&package),
                "a.mtrl",
                0x11,
                &override_keys,
                &override_constants,
                &[],
                &first,
            );
        }
        builder.observe_material(
            "character.shpk",
            Some(&package),
            "b.mtrl",
            0x01,
            &[],
            &[],
            &[],
            &second,
        );

        let result = builder.finish();
        assert_eq!(result.unique_material_resources, 2);
        assert_eq!(result.unique_shader_packages, 1);
        assert_eq!(result.unknown_key_category_count, 1);
        assert_eq!(result.unknown_key_value_count, 2);
        assert_eq!(result.unknown_constant_id_count, 1);

        let material_key = result
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.scope == WeaponShaderKeyScope::Material && coverage.category == 0x100
            })
            .expect("material key coverage");
        assert_eq!(material_key.material_resource_count, 2);
        assert_eq!(material_key.material_reference_count, 8);
        assert_eq!(material_key.material_override_resource_count, 1);
        assert_eq!(material_key.material_override_reference_count, 6);
        assert_eq!(material_key.observed_values.len(), 2);
        assert!(
            material_key
                .observed_values
                .iter()
                .any(|value| value.value == 11 && value.value_name.is_none())
        );

        let scene_key = result
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.scope == WeaponShaderKeyScope::Scene && coverage.category == 0x300
            })
            .expect("scene key coverage");
        assert_eq!(scene_key.material_override_reference_count, 6);
        assert!(
            scene_key
                .observed_values
                .iter()
                .any(|value| value.value == 31)
        );
        assert_eq!(scene_key.material_reference_count, 8);
        assert!(!result.material_key_coverage.iter().any(|coverage| {
            coverage.scope == WeaponShaderKeyScope::MaterialOverrideOnly
                && coverage.category == 0x300
        }));

        let known_constant = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x500)
            .expect("known constant coverage");
        assert_eq!(known_constant.material_resource_count, 2);
        assert_eq!(known_constant.material_reference_count, 8);
        assert_eq!(known_constant.material_override_resource_count, 1);
        assert_eq!(known_constant.material_override_reference_count, 6);
        assert_eq!(known_constant.malformed_override_reference_count, 6);
        assert_eq!(known_constant.shader_flag_counts.len(), 2);

        let duplicate_override_only = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x600)
            .expect("duplicate override-only constant coverage");
        assert_eq!(duplicate_override_only.material_resource_count, 1);
        assert_eq!(duplicate_override_only.material_reference_count, 6);
        assert_eq!(duplicate_override_only.observed_values.len(), 1);
        assert_eq!(
            duplicate_override_only.observed_values[0].values,
            vec![Some(3.0)]
        );

        let non_finite = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x700)
            .expect("non-finite constant coverage");
        assert_eq!(non_finite.non_finite_resource_count, 1);
        assert_eq!(non_finite.non_finite_reference_count, 6);
        assert_eq!(non_finite.malformed_override_reference_count, 6);
        assert_eq!(
            non_finite.malformed_override_value_size_resource_counts,
            BTreeMap::from([(6, 1)])
        );
        assert_eq!(non_finite.observed_values[0].values, vec![None]);
        let malformed = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x800)
            .expect("malformed constant coverage");
        assert_eq!(malformed.malformed_override_resource_count, 1);
        assert_eq!(malformed.malformed_override_reference_count, 6);
        assert_eq!(malformed.unresolved_value_reference_count, 6);

        let no_default = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x900)
            .expect("no-default package constant coverage");
        assert_eq!(no_default.material_reference_count, 8);
        assert_eq!(no_default.material_override_reference_count, 6);
        assert_eq!(no_default.unresolved_value_reference_count, 2);
        assert_eq!(no_default.observed_values[0].values, vec![Some(4.0)]);

        let zero_width = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0xA00)
            .expect("zero-width package constant coverage");
        assert_eq!(zero_width.default_values, Some(Vec::new()));
        assert_eq!(zero_width.unresolved_value_reference_count, 0);
        assert_eq!(
            zero_width.value_width_resource_counts,
            BTreeMap::from([(0, 2)])
        );
        serde_json::to_vec(&result.material_constant_coverage)
            .expect("non-finite coverage remains JSON serializable");
    }

    #[test]
    fn sampler_coverage_preserves_exact_skin_role_and_package_resource_name() {
        let texture_usage = physis::shpk::ShaderPackage::crc("g_SamplerSkinDiffuse");
        let package = ShaderPackageSemanticDebug {
            path: "shader/sm5/shpk/character.shpk".to_string(),
            name: "character.shpk".to_string(),
            sampler_resources: vec![ShaderPackageSamplerResourceDebug {
                name: "g_SamplerSkinDiffuse".to_string(),
                crc: texture_usage,
                crc_hex: hex_u32(texture_usage),
                slot: 2,
                size: 1,
                logical_role: Some(MaterialSamplerLogicalRole::SkinDiffuse),
                kind: Some(ModelTextureKind::BaseColor),
            }],
            material_keys: Vec::new(),
            system_keys: Vec::new(),
            scene_keys: Vec::new(),
            material_constants: Vec::new(),
        };
        let sampler = ObservedMaterialSampler {
            texture_usage,
            texture_usage_name: Some("known-crc-alias".to_string()),
            logical_role: Some(MaterialSamplerLogicalRole::SkinDiffuse),
            texture_kind: Some(ModelTextureKind::BaseColor),
            flags: 0x1234_5678,
        };
        let representative = test_representative("skin.mtrl", 0x11, 3);
        let mut builder = MaterialSemanticCoverageBuilder::default();

        builder.observe_material(
            "character.shpk",
            Some(&package),
            "skin.mtrl",
            0x11,
            &[],
            &[],
            &[sampler.clone(), sampler],
            &representative,
        );

        let result = builder.finish();
        assert_eq!(result.sampler_coverage.len(), 1);
        let coverage = &result.sampler_coverage[0];
        assert_eq!(
            coverage.texture_usage_name.as_deref(),
            Some("g_SamplerSkinDiffuse")
        );
        assert_eq!(
            coverage.logical_role,
            Some(MaterialSamplerLogicalRole::SkinDiffuse)
        );
        assert_eq!(coverage.texture_kind, Some(ModelTextureKind::BaseColor));
        assert_eq!(coverage.flags, 0x1234_5678);
        assert_eq!(coverage.material_resource_count, 1);
        assert_eq!(coverage.material_reference_count, 3);
        assert_eq!(result.unknown_sampler_role_count, 0);
        assert_eq!(result.unresolved_sampler_name_count, 0);
    }

    fn test_package_key(
        id: u32,
        name: Option<&str>,
        default_value: u32,
        default_value_name: Option<&str>,
    ) -> ShaderPackageKeyDefaultDebug {
        ShaderPackageKeyDefaultDebug {
            id,
            id_hex: hex_u32(id),
            name: name.map(str::to_string),
            default_value,
            default_value_hex: hex_u32(default_value),
            default_value_name: default_value_name.map(str::to_string),
        }
    }

    fn test_package_constant(
        id: u32,
        name: Option<&str>,
        byte_offset: u16,
        byte_size: u16,
        default_values: Option<Vec<f32>>,
    ) -> ShaderPackageMaterialConstantDebug {
        ShaderPackageMaterialConstantDebug {
            id,
            id_hex: hex_u32(id),
            name: name.map(str::to_string),
            byte_offset,
            byte_size,
            default_values,
        }
    }

    fn test_representative(
        material_path: &str,
        shader_flags: u32,
        item_reference_count: usize,
    ) -> WeaponSemanticRepresentative {
        WeaponSemanticRepresentative {
            item_reference_count,
            item_ids: vec![1],
            item_names: vec!["item".to_string()],
            model: PackedModelId::from_raw(1),
            model_path: "model.mdl".to_string(),
            material_name: material_path.to_string(),
            material_path: material_path.to_string(),
            shader_flags,
            shader_flags_hex: hex_u32(shader_flags),
        }
    }
}
