use super::{
    ModelInstance, ModelRenderContext, SurfaceOverlayUniform, VfxAuraGpuTexture,
    VfxDocumentSortRange, VfxParticles, VfxTextureInput,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use wgpu::util::DeviceExt;
use xiv_companion_data::{
    AvfxParticle, AvfxParticleData, ModelSkeleton, SkeletonPose, VfxAuraTextureArrayRgba,
    VfxBindPoint, VfxBinderCameraSnapshot, VfxBinderTargetSnapshot, VfxCameraViewSnapshot,
    VfxClientBinderCurveDefaults, VfxClientColorCurveDefaults, VfxClientModelSkinCurveDefaults,
    VfxClientRandomState, VfxClientTrigMode, VfxDrawModel, VfxMeshInstance,
    VfxModelSkinCameraFacing, VfxModelSkinCharacterTargets, VfxModelSkinCreationOrder,
    VfxModelSkinInstance, VfxModelSkinSurface, VfxModelSkinTargetInput, VfxModelSkinTargetSnapshot,
    VfxPlayback, VfxQuad, VfxRuntime, VfxSharedRandomStream, WeaponVfxAttachments,
};

/// Independently advanced mounts sharing one GPU batch, so DwPr ordering spans
/// all attachments and both geometry types.
pub struct WeaponVfxParticles {
    particles: VfxParticles,
    samples: WeaponVfxPlayback,
    aura_resources: Vec<WeaponVfxAuraResource>,
    aura_bindings: WeaponVfxAuraBindings,
    aura_diagnostics: Vec<String>,
}

/// A prepared per-definition Aura target.
pub struct WeaponVfxAuraResource {
    pub attachment_index: usize,
    pub particle_index: usize,
    pub target_model_path: String,
    pub priority: u8,
    pub texture: VfxAuraGpuTexture,
    pub sampler: wgpu::Sampler,
    pub(crate) overlay_bind_group: wgpu::BindGroup,
    pub(crate) uniform_buffer: wgpu::Buffer,
    pub(crate) fresnel_type: i32,
    pub(crate) current_uniform: Cell<SurfaceOverlayUniform>,
    pub(crate) current_camera_facing: Cell<Option<VfxModelSkinCameraFacing>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponVfxAuraInstance {
    pub resource_index: usize,
    pub instance: VfxModelSkinInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuraBinding {
    resource_index: usize,
    instance_id: u64,
}

struct AuraTargetSlot {
    target_model_path: String,
    priority: u8,
    binding: Option<AuraBinding>,
    binding_attachment_index: Option<usize>,
    creation_order: Option<u64>,
    last_instance: Option<VfxModelSkinInstance>,
    last_uniform: Option<SurfaceOverlayUniform>,
    last_camera_facing: Option<VfxModelSkinCameraFacing>,
}

#[derive(Clone, Copy)]
struct AuraCandidate<'a> {
    target_model_path: &'a str,
    priority: u8,
    attachment_index: usize,
    creation_order: Option<u64>,
    binding: AuraBinding,
}

fn advance_aura_slots<'a>(
    slots: &mut Vec<AuraTargetSlot>,
    candidates: impl IntoIterator<Item = AuraCandidate<'a>>,
) {
    let mut highest: Vec<(&str, u8, Option<(AuraBinding, usize, Option<u64>)>)> = Vec::new();
    for candidate in candidates {
        if let Some((_, priority, binding)) = highest
            .iter_mut()
            .find(|(target, _, _)| *target == candidate.target_model_path)
        {
            if candidate.priority > *priority {
                *priority = candidate.priority;
                *binding = Some((
                    candidate.binding,
                    candidate.attachment_index,
                    candidate.creation_order,
                ));
            } else if candidate.priority == *priority {
                if let Some((selected, attachment_index, creation_order)) = binding {
                    if let (Some(previous), Some(next)) =
                        (*creation_order, candidate.creation_order)
                    {
                        // QPC keys prefer earlier construction; strict equality
                        // retains the first binding. Host serials preserve order.
                        if next < previous {
                            *selected = candidate.binding;
                            *attachment_index = candidate.attachment_index;
                            *creation_order = Some(next);
                        }
                    } else if *attachment_index != candidate.attachment_index {
                        *binding = None;
                    } else if candidate.binding.instance_id < selected.instance_id {
                        *selected = candidate.binding;
                        *creation_order = candidate.creation_order;
                    }
                }
            }
        } else {
            highest.push((
                candidate.target_model_path,
                candidate.priority,
                Some((
                    candidate.binding,
                    candidate.attachment_index,
                    candidate.creation_order,
                )),
            ));
        }
    }
    for (target, priority, binding) in highest {
        if let Some(slot) = slots
            .iter_mut()
            .find(|slot| slot.target_model_path == target)
        {
            let earlier = binding.is_some_and(|(binding, attachment, order)| {
                match (order, slot.creation_order) {
                    (Some(next), Some(previous)) => next < previous,
                    _ => {
                        slot.binding_attachment_index == Some(attachment)
                            && slot
                                .binding
                                .is_some_and(|old| binding.instance_id < old.instance_id)
                    }
                }
            });
            if priority > slot.priority || (priority == slot.priority && earlier) {
                slot.priority = priority;
                slot.binding = binding.map(|(binding, _, _)| binding);
                slot.binding_attachment_index = binding.map(|(_, attachment, _)| attachment);
                slot.creation_order = binding.and_then(|(_, _, order)| order);
                slot.last_instance = None;
                slot.last_uniform = None;
                slot.last_camera_facing = None;
            }
        } else {
            slots.push(AuraTargetSlot {
                target_model_path: target.to_owned(),
                priority,
                binding: binding.map(|(binding, _, _)| binding),
                binding_attachment_index: binding.map(|(_, attachment, _)| attachment),
                creation_order: binding.and_then(|(_, _, order)| order),
                last_instance: None,
                last_uniform: None,
                last_camera_facing: None,
            });
        }
    }
}

struct AuraResourceTarget {
    target_model_path: String,
    priority: u8,
    attachment_index: usize,
}

/// Production Aura target selection and last-parameter retention, without GPU
/// allocation. One coordinator belongs to one WeaponVfxPlayback group.
/// Prepared input indexes must match that playback's output resource indexes.
#[derive(Default)]
pub struct WeaponVfxAuraBindings {
    resources: Vec<AuraResourceTarget>,
    slots: Vec<AuraTargetSlot>,
    last_time: Option<f32>,
}

impl WeaponVfxAuraBindings {
    pub fn from_prepared_inputs(inputs: &[WeaponVfxAuraInput]) -> Self {
        Self {
            resources: inputs
                .iter()
                .map(|input| AuraResourceTarget {
                    target_model_path: input.target_model_path.clone(),
                    priority: input.priority,
                    attachment_index: input.attachment_index,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn from_resources(resources: &[WeaponVfxAuraResource]) -> Self {
        Self {
            resources: resources
                .iter()
                .map(|resource| AuraResourceTarget {
                    target_model_path: resource.target_model_path.clone(),
                    priority: resource.priority,
                    attachment_index: resource.attachment_index,
                })
                .collect(),
            ..Default::default()
        }
    }

    pub fn update(&mut self, time: f32, playback: &WeaponVfxPlayback) {
        self.note_time(time);
        if time.is_finite() && time >= 0.0 {
            self.sync(playback);
        }
    }

    /// Selected resources with their last successful parameters, including
    /// resources retained after source retirement or playback fallback.
    pub fn selected_instances(&self) -> Vec<WeaponVfxAuraInstance> {
        self.slots
            .iter()
            .filter_map(|slot| {
                Some(WeaponVfxAuraInstance {
                    resource_index: slot.binding?.resource_index,
                    instance: slot.last_instance?,
                })
            })
            .collect()
    }

    fn note_time(&mut self, time: f32) {
        if !time.is_finite() || time < 0.0 || self.last_time.is_some_and(|last| time < last) {
            self.slots.clear();
        }
        self.last_time = (time.is_finite() && time >= 0.0).then_some(time);
    }

    fn sync(&mut self, playback: &WeaponVfxPlayback) {
        advance_aura_slots(
            &mut self.slots,
            playback
                .aura_observed_instances
                .iter()
                .filter_map(|active| {
                    self.resources
                        .get(active.resource_index)
                        .map(|resource| AuraCandidate {
                            target_model_path: &resource.target_model_path,
                            priority: resource.priority,
                            attachment_index: resource.attachment_index,
                            creation_order: active.instance.aura_creation_order,
                            binding: AuraBinding {
                                resource_index: active.resource_index,
                                instance_id: active.instance.instance_id,
                            },
                        })
                }),
        );
        for slot in &mut self.slots {
            let Some(binding) = slot.binding else {
                continue;
            };
            if let Some(active) = playback.aura_observed_instances.iter().find(|active| {
                active.resource_index == binding.resource_index
                    && active.instance.instance_id == binding.instance_id
            }) {
                slot.last_instance = Some(active.instance);
                slot.last_uniform = Some(model_skin_aura_uniform(&active.instance));
                slot.last_camera_facing = active.instance.aura_camera_facing;
            }
        }
    }
}

fn model_skin_aura_uniform(instance: &VfxModelSkinInstance) -> SurfaceOverlayUniform {
    let mut aura_params = [[0.0; 4]; 16];
    aura_params[0] = instance.aura_color_begin;
    aura_params[1] = instance.aura_color_end;
    aura_params[2] = instance.aura_fresnel;
    aura_params[3][0] = instance.aura_fresnel_selector;
    aura_params[3][1] = instance.aura_sem;
    aura_params[3][2] = instance.aura_eem;
    aura_params[3][3] = instance.aura_distortion_power;
    aura_params[4] = instance.aura_color_combine;
    aura_params[5] = instance.aura_material_combine;
    aura_params[6] = instance.aura_combine_extras;
    aura_params[7] = instance.aura_uv_point_density;
    for (slot, rows) in instance.aura_texture_uv.iter().enumerate() {
        aura_params[8 + slot * 2] = rows[0];
        aura_params[9 + slot * 2] = rows[1];
    }
    for (index, control) in instance.aura_texture_controls.iter().enumerate() {
        aura_params[14][index] = f32::from_bits(*control);
    }
    // 140401b77 writes an integer enable bit, consumed by DXBC movc.
    aura_params[14][3] = f32::from_bits(u32::from(instance.aura_tc2_alpha_enabled != 0.0));
    aura_params[15][0] = instance.color[3];
    aura_params[15][1] = instance.aura_tc2_color_enabled;
    aura_params[15][2] = instance.aura_color_mix;
    // Preview-only active marker; the default overlay keeps this lane zero.
    aura_params[15][3] = 1.0;
    SurfaceOverlayUniform { aura_params }
}

#[cfg(test)]
fn aura_uniform_with_camera(
    uniform: SurfaceOverlayUniform,
    fresnel_type: i32,
    camera_position: [f32; 3],
    view_dir: [f32; 3],
    camera_facing: Option<VfxModelSkinCameraFacing>,
) -> Option<SurfaceOverlayUniform> {
    aura_uniform_with_camera_up(
        uniform,
        fresnel_type,
        camera_position,
        view_dir,
        None,
        camera_facing,
    )
}

fn aura_uniform_with_camera_up(
    mut uniform: SurfaceOverlayUniform,
    fresnel_type: i32,
    camera_position: [f32; 3],
    view_dir: [f32; 3],
    camera_up: Option<[f32; 3]>,
    camera_facing: Option<VfxModelSkinCameraFacing>,
) -> Option<SurfaceOverlayUniform> {
    match fresnel_type {
        1 => uniform.aura_params[2][..3].copy_from_slice(&camera_position),
        3 => {
            let facing = camera_facing?;
            let direction = match camera_up {
                Some(up) => facing.direction_with_camera_up(camera_position, view_dir, up)?,
                None => facing.direction_with_camera(camera_position, view_dir)?,
            };
            uniform.aura_params[2][..3].copy_from_slice(&direction);
        }
        _ => return None,
    }
    Some(uniform)
}

impl WeaponVfxAuraResource {
    #[cfg(all(test, feature = "test-support"))]
    pub(crate) fn uniform_with_camera(
        &self,
        camera_position: [f32; 3],
        view_dir: [f32; 3],
    ) -> Option<SurfaceOverlayUniform> {
        self.uniform_with_camera_up(camera_position, view_dir, None)
    }

    pub(crate) fn uniform_with_camera_up(
        &self,
        camera_position: [f32; 3],
        view_dir: [f32; 3],
        camera_up: Option<[f32; 3]>,
    ) -> Option<SurfaceOverlayUniform> {
        let uniform = aura_uniform_with_camera_up(
            self.current_uniform.get(),
            self.fresnel_type,
            camera_position,
            view_dir,
            camera_up,
            self.current_camera_facing.get(),
        )?;
        self.current_uniform.set(uniform);
        Some(uniform)
    }
}

impl ModelRenderContext {
    pub fn create_weapon_vfx_particles(
        &self,
        model: &ModelInstance,
        data: &WeaponVfxAttachments,
    ) -> WeaponVfxParticles {
        self.create_weapon_vfx_particles_inner(model, data, None, None)
    }

    /// Install camera inputs before constructing Binder and particle birth state.
    pub fn create_weapon_vfx_particles_with_camera(
        &self,
        model: &ModelInstance,
        data: &WeaponVfxAttachments,
        camera: VfxBinderCameraSnapshot,
    ) -> Result<WeaponVfxParticles, String> {
        camera.validate()?;
        for attachment in &data.attachments {
            local_camera(camera, model.mdl_preview_offset(&attachment.model_path)).validate()?;
        }
        Ok(self.create_weapon_vfx_particles_inner(model, data, Some(camera), None))
    }

    /// Install the actual inverse view and viewport before Camera Binder birth.
    pub fn create_weapon_vfx_particles_with_camera_view(
        &self,
        model: &ModelInstance,
        data: &WeaponVfxAttachments,
        view: VfxCameraViewSnapshot,
    ) -> Result<WeaponVfxParticles, String> {
        view.validate()?;
        // Validate all model-local inputs and authored constructor fields before
        // allocating GPU resources or constructing any mounted playback tree.
        for attachment in &data.attachments {
            let local = local_camera_view(view, model.mdl_preview_offset(&attachment.model_path));
            attachment
                .data
                .runtime()
                .with_preview_camera_host_rotation([0.0; 3])?
                .with_preview_camera_view(local)?;
        }
        Ok(self.create_weapon_vfx_particles_inner(model, data, None, Some(view)))
    }

    fn create_weapon_vfx_particles_inner(
        &self,
        model: &ModelInstance,
        data: &WeaponVfxAttachments,
        camera: Option<VfxBinderCameraSnapshot>,
        camera_view: Option<VfxCameraViewSnapshot>,
    ) -> WeaponVfxParticles {
        let (textures, models) = attachment_resources(data);
        let (aura_inputs, mut aura_diagnostics) =
            model_skin_aura_inputs_for_surfaces(data, |path| model.accepts_aura_target(path));
        let aura_resources: Vec<WeaponVfxAuraResource> = aura_inputs
            .into_iter()
            .filter_map(|input| {
                let texture = match self.upload_vfx_aura_texture_array(&input.packed) {
                    Ok(texture) => texture,
                    Err(error) => {
                        aura_diagnostics.push(format!(
                            "{} Ptcl[{}] Aura GPU upload: {error:?}",
                            data.attachments[input.attachment_index].data.avfx_path,
                            input.particle_index
                        ));
                        return None;
                    }
                };
                let mut descriptor = super::vfx::vfx_sampler_descriptor(input.border, input.filter);
                descriptor.label = Some("weapon ModelSkin Aura sampler");
                let sampler = self.device().create_sampler(&descriptor);
                let initial_uniform = SurfaceOverlayUniform {
                    aura_params: [[0.0; 4]; 16],
                };
                let uniform_buffer =
                    self.device()
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("weapon ModelSkin Aura uniform"),
                            contents: bytemuck::bytes_of(&initial_uniform),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        });
                let overlay_bind_group =
                    self.device().create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("weapon ModelSkin Aura surface overlay"),
                        layout: self.surface_overlay_bind_group_layout(),
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: uniform_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&texture.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&sampler),
                            },
                        ],
                    });
                Some(WeaponVfxAuraResource {
                    attachment_index: input.attachment_index,
                    particle_index: input.particle_index,
                    target_model_path: input.target_model_path,
                    priority: input.priority,
                    texture,
                    sampler,
                    overlay_bind_group,
                    uniform_buffer,
                    fresnel_type: input.fresnel_type,
                    current_uniform: Cell::new(initial_uniform),
                    current_camera_facing: Cell::new(None),
                })
            })
            .collect();
        WeaponVfxParticles {
            particles: self.create_vfx_particles(&textures, &models),
            samples: WeaponVfxPlayback::new_with_inputs(
                data,
                &aura_resources,
                camera,
                camera_view,
                |path| model.mdl_preview_offset(path),
            ),
            aura_bindings: WeaponVfxAuraBindings::from_resources(&aura_resources),
            aura_resources,
            aura_diagnostics,
        }
    }
}

impl WeaponVfxParticles {
    #[cfg(feature = "test-support")]
    pub(crate) fn sampled_quads(&self) -> &[VfxQuad] {
        &self.samples.quads
    }

    /// Queue a world-space preview camera for every owning MDL. Only position
    /// changes under the preview's per-MDL translation; facing stays unchanged.
    pub fn set_camera(
        &mut self,
        model: &ModelInstance,
        camera: VfxBinderCameraSnapshot,
    ) -> Result<(), String> {
        self.samples
            .set_camera(camera, |path| model.mdl_preview_offset(path))
    }

    /// Queue the complete preview view in each owning MDL's local space.
    pub fn set_camera_view(
        &mut self,
        model: &ModelInstance,
        view: VfxCameraViewSnapshot,
    ) -> Result<(), String> {
        self.samples
            .set_camera_view(view, |path| model.mdl_preview_offset(path))
    }

    /// Keep input history even when drawing is disabled or the surface is unavailable.
    pub fn advance_to(&mut self, time: f32) {
        self.note_aura_time(time);
        self.samples.advance_to(time);
        self.sync_aura_slots();
    }

    /// Queue this owner's local bone pose for the next absolute-time update.
    /// Each MDL retains its own skeleton and Binder birth/current history.
    /// This is not a skinning matrix or a character-hand/world transform.
    pub fn set_attachment_pose(
        &mut self,
        model_path: &str,
        pose: &SkeletonPose,
    ) -> Result<(), String> {
        self.samples.set_attachment_pose(model_path, pose)
    }

    /// Attach an external target Timeline to this MDL's Document. Trigger
    /// numbers are one based; no implicit Time/Prepare or GPU update occurs.
    /// The next advancing preview timestamp processes the target. Restarting
    /// or seeking backwards discards external inputs rather than replaying them.
    pub fn trigger_attachment_document(
        &mut self,
        model_path: &str,
        scheduler_index: i32,
        trigger_number: i32,
    ) -> Result<bool, String> {
        self.samples
            .trigger_attachment_document(model_path, scheduler_index, trigger_number)
    }

    pub fn binding_diagnostics(&self) -> Vec<(&str, &str)> {
        self.samples
            .attachments
            .iter()
            .filter_map(|attachment| {
                attachment
                    .target_error
                    .as_deref()
                    .or(attachment.playback.fallback_reason())
                    .map(|reason| (attachment.model_path.as_str(), reason))
            })
            .collect()
    }

    pub fn update(&mut self, context: &ModelRenderContext, model: &ModelInstance, time: f32) {
        if time.is_finite() && time >= 0.0 {
            self.samples
                .refresh_model_skin_targets(|path| model.accepts_aura_target(path));
        }
        self.note_aura_time(time);
        self.samples
            .sample(time, self.particles.capacity(), |path| {
                model.mdl_preview_offset(path)
            });
        self.sync_aura_slots();
        self.particles
            .document_sort_ranges
            .clone_from(&self.samples.document_sort_ranges);
        for slot in &self.aura_bindings.slots {
            let (Some(binding), Some(uniform)) = (slot.binding, slot.last_uniform) else {
                continue;
            };
            let resource = &self.aura_resources[binding.resource_index];
            resource.current_uniform.set(uniform);
            resource.current_camera_facing.set(slot.last_camera_facing);
            context
                .queue()
                .write_buffer(&resource.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        }
        self.particles.update(context, &self.samples.quads);
        self.particles.update_mesh(context, &self.samples.meshes);
    }

    pub fn particles(&self) -> &VfxParticles {
        &self.particles
    }

    pub fn aura_resources(&self) -> &[WeaponVfxAuraResource] {
        &self.aura_resources
    }

    pub fn active_aura_instances(&self) -> &[WeaponVfxAuraInstance] {
        &self.samples.aura_instances
    }

    fn note_aura_time(&mut self, time: f32) {
        self.aura_bindings.note_time(time);
    }

    fn sync_aura_slots(&mut self) {
        self.aura_bindings.sync(&self.samples);
    }

    /// The target retains an observed binding after the source instance dies
    /// or the preview switches this attachment to continuous fallback.
    /// Mounted documents share actual ModelSkin constructor order. Equal
    /// priority selects the earlier resource; equal keys retain the first bind.
    pub fn unambiguous_active_aura_resources(&self) -> Vec<&WeaponVfxAuraResource> {
        self.aura_bindings
            .slots
            .iter()
            .filter_map(|slot| slot.binding)
            .filter_map(|binding| self.aura_resources.get(binding.resource_index))
            .collect()
    }

    /// Compatibility path for callers that can draw only one active Aura.
    pub fn unambiguous_active_aura_resource(&self) -> Option<&WeaponVfxAuraResource> {
        let resources = self.unambiguous_active_aura_resources();
        if resources.len() == 1 {
            Some(resources[0])
        } else {
            None
        }
    }

    pub fn aura_diagnostics(&self) -> &[String] {
        &self.aura_diagnostics
    }
}

/// CPU-prepared Aura resource: a real target MDL, compatible rendered surface,
/// validated texture-array layers/mips and the authored sampler. Preparing
/// this resource does not assert GPU upload or client pixel equivalence.
pub struct WeaponVfxAuraInput {
    pub attachment_index: usize,
    pub particle_index: usize,
    pub target_model_path: String,
    pub priority: u8,
    pub packed: VfxAuraTextureArrayRgba,
    pub border: [i32; 2],
    pub filter: i32,
    pub fresnel_type: i32,
}

fn model_skin_aura_sampler(particle: &AvfxParticle) -> Option<([i32; 2], i32)> {
    for texture in [
        particle.texture_color2.as_ref(),
        particle.texture_color3.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter(|texture| texture.enabled)
    {
        return Some((
            [texture.texture_border_u, texture.texture_border_v],
            texture.texture_filter,
        ));
    }
    particle
        .texture_distortion
        .as_ref()
        .filter(|texture| texture.enabled)
        .map(|texture| {
            (
                [texture.texture_border_u, texture.texture_border_v],
                texture.texture_filter,
            )
        })
}

/// Prepare Aura inputs without a GPU, using the same flattened draw eligibility
/// and surface/material checks as model-instance creation. Options must match
/// the model instance whose batches will receive the overlays.
pub fn prepare_weapon_vfx_aura_inputs<M: super::ModelRenderData + ?Sized>(
    model: &M,
    data: &WeaponVfxAttachments,
    options: super::PreparedModelOptions,
) -> (Vec<WeaponVfxAuraInput>, Vec<String>) {
    let flattened = super::instance::flatten_model_with_options_and_skeleton(model, options, None);
    model_skin_aura_inputs_for_surfaces(data, |path| {
        flattened
            .draw_batches
            .iter()
            .any(|batch| batch.accepts_aura_target(path))
    })
}

#[cfg(test)]
fn model_skin_aura_inputs(data: &WeaponVfxAttachments) -> (Vec<WeaponVfxAuraInput>, Vec<String>) {
    model_skin_aura_inputs_for_surfaces(data, |_| true)
}

fn model_skin_aura_inputs_for_surfaces(
    data: &WeaponVfxAttachments,
    accepts: impl Fn(&str) -> bool,
) -> (Vec<WeaponVfxAuraInput>, Vec<String>) {
    let mut inputs = Vec::new();
    let mut diagnostics = Vec::new();
    for (attachment_index, attachment) in data.attachments.iter().enumerate() {
        for (particle_index, particle) in attachment.data.file.particles.iter().enumerate() {
            let AvfxParticleData::ModelSkin(model_skin) = &particle.data else {
                continue;
            };
            let flags = model_skin.aura_target as u32;
            for (bit, role, loaded) in [
                (1, "character", false),
                (2, "weapon", data.model_skin_targets.weapon.is_some()),
                (4, "off hand", data.model_skin_targets.off_hand.is_some()),
                (8, "summon", false),
            ] {
                if flags & bit != 0 && !loaded {
                    diagnostics.push(format!(
                        "{} Ptcl[{particle_index}] has no loaded Aura target for {role} (AuTT={})",
                        attachment.data.avfx_path, model_skin.aura_target
                    ));
                }
            }
            if flags & 15 == 0 {
                diagnostics.push(format!(
                    "{} Ptcl[{particle_index}] has no loaded Aura target for AuTT={}",
                    attachment.data.avfx_path, model_skin.aura_target
                ));
            }
            let target_paths = data
                .model_skin_target_paths(model_skin.aura_target)
                .filter(|path| {
                    if accepts(path) {
                        true
                    } else {
                        diagnostics.push(format!(
                            "{} Ptcl[{particle_index}] has no compatible Aura surface in {}",
                            attachment.data.avfx_path, path
                        ));
                        false
                    }
                })
                .collect::<Vec<_>>();
            if target_paths.is_empty() {
                continue;
            }
            let packed = match VfxAuraTextureArrayRgba::from_model_skin_particle(
                particle,
                &attachment.data.textures,
            ) {
                Ok(packed) => packed,
                Err(error) => {
                    diagnostics.push(format!(
                        "{} Ptcl[{particle_index}] Aura texture array: {error:?}",
                        attachment.data.avfx_path
                    ));
                    continue;
                }
            };
            let Some((border, filter)) = model_skin_aura_sampler(particle) else {
                diagnostics.push(format!(
                    "{} Ptcl[{particle_index}] has no Aura sampler source",
                    attachment.data.avfx_path
                ));
                continue;
            };
            let make_input = |target_model_path: &str, packed| WeaponVfxAuraInput {
                attachment_index,
                particle_index,
                target_model_path: target_model_path.to_string(),
                priority: attachment.data.file.global.a_pri as u8,
                packed,
                border,
                filter,
                fresnel_type: model_skin.fresnel_type & 3,
            };
            let (last, others) = target_paths
                .split_last()
                .expect("nonempty compatible targets");
            for target in others {
                inputs.push(make_input(target, packed.clone()));
            }
            inputs.push(make_input(last, packed));
        }
    }
    (inputs, diagnostics)
}

fn attachment_resources(
    data: &WeaponVfxAttachments,
) -> (Vec<Option<VfxTextureInput>>, Vec<VfxDrawModel>) {
    let mut textures = Vec::new();
    let mut models = Vec::new();
    for attachment in &data.attachments {
        let data = &attachment.data;
        textures.extend((0..data.file.texture_paths.len()).map(|index| {
            data.textures
                .get(index)
                .and_then(Option::as_ref)
                .map(VfxTextureInput::from)
        }));
        models.extend(
            data.file
                .models
                .iter()
                .map(|model| model.draw.clone().unwrap_or_default()),
        );
    }
    (textures, models)
}

fn local_camera(
    mut camera: VfxBinderCameraSnapshot,
    offset: Option<[f32; 3]>,
) -> VfxBinderCameraSnapshot {
    if let Some(offset) = offset {
        for (position, offset) in camera.position.iter_mut().zip(offset) {
            *position -= offset;
        }
    }
    camera
}

fn local_camera_view(
    mut view: VfxCameraViewSnapshot,
    offset: Option<[f32; 3]>,
) -> VfxCameraViewSnapshot {
    view.camera = local_camera(view.camera, offset);
    if let Some(offset) = offset {
        for (position, offset) in view.inverse_view.position.iter_mut().zip(offset) {
            *position -= offset;
        }
    }
    view
}

struct AttachmentRuntime {
    model_path: String,
    bind_points: Vec<VfxBindPoint>,
    skeleton: Option<ModelSkeleton>,
    targets: Option<Vec<VfxBinderTargetSnapshot>>,
    camera: Option<VfxBinderCameraSnapshot>,
    camera_view: Option<VfxCameraViewSnapshot>,
    requires_camera_view: bool,
    target_error: Option<String>,
    playback: VfxPlayback,
    soft_key_offset: f32,
    registration_serial: u32,
    aura_resource_indexes: HashMap<usize, Vec<usize>>,
    aura_resource_target_slots: HashMap<usize, u8>,
    texture_base: usize,
    texture_count: usize,
    model_base: usize,
    model_count: usize,
    particle_base: usize,
}

impl AttachmentRuntime {
    fn aura_instances(
        &self,
        instance: VfxModelSkinInstance,
    ) -> impl Iterator<Item = WeaponVfxAuraInstance> + '_ {
        self.aura_resource_indexes
            .get(&instance.particle_index)
            .into_iter()
            .flatten()
            .copied()
            .filter(move |resource_index| {
                instance.aura_registered_target_slots.is_none_or(|mask| {
                    self.aura_resource_target_slots
                        .get(resource_index)
                        .is_some_and(|slot| mask & slot != 0)
                })
            })
            .map(move |resource_index| WeaponVfxAuraInstance {
                resource_index,
                instance,
            })
    }

    fn texture_index(&self, index: i32) -> i32 {
        if index < 0 {
            return index;
        }
        // A positive invalid reference must still sample the fallback texture,
        // without aliasing another AVFX's slot or disabling its TC/TD layer.
        ((index as usize) < self.texture_count)
            .then(|| self.texture_base.checked_add(index as usize))
            .flatten()
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(i32::MAX)
    }
}

// The weapon preview owns one caster with prepared main/off-hand resources.
// These are preview identities, not game object handles or an IFY resolver.
fn preview_model_skin_input(
    paths: &[Option<String>; 2],
    accepts: impl Fn(&str) -> bool,
) -> VfxModelSkinTargetInput {
    let surface = |index: usize| {
        paths[index]
            .as_deref()
            .filter(|path| accepts(path))
            .map(|_| VfxModelSkinSurface {
                identity: if index == 0 { 2 } else { 4 },
                type_code: 3,
            })
    };
    VfxModelSkinTargetInput {
        listener_present: true,
        character: Some(VfxModelSkinCharacterTargets {
            // Model replacement creates new attachment playback/resources.
            generation: 1,
            surfaces: [None, surface(0), surface(1), None],
        }),
        ..Default::default()
    }
}

/// CPU playback used by mounted GPU effects. Prepared Aura inputs retain
/// their list indexes as output resource indexes; actual GPU creation uses
/// the same constructor and updates. This does not create/upload GPU resources.
pub struct WeaponVfxPlayback {
    random_stream: VfxSharedRandomStream,
    model_skin_creation_order: VfxModelSkinCreationOrder,
    last_input_time: Option<f32>,
    model_skin_paths: [Option<String>; 2],
    attachments: Vec<AttachmentRuntime>,
    document_sort_ranges: Vec<VfxDocumentSortRange>,
    aura_instances: Vec<WeaponVfxAuraInstance>,
    aura_observed_instances: Vec<WeaponVfxAuraInstance>,
    quads: Vec<VfxQuad>,
    meshes: Vec<VfxMeshInstance>,
    quad_scratch: Vec<VfxQuad>,
    mesh_scratch: Vec<VfxMeshInstance>,
}

impl WeaponVfxPlayback {
    pub fn from_prepared_aura_inputs(
        data: &WeaponVfxAttachments,
        inputs: &[WeaponVfxAuraInput],
        camera: Option<VfxBinderCameraSnapshot>,
        camera_view: Option<VfxCameraViewSnapshot>,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Self {
        Self::new_with_resource_indexes(
            data,
            inputs.iter().enumerate().map(|(index, input)| {
                (
                    input.attachment_index,
                    input.particle_index,
                    index,
                    input.target_model_path.as_str(),
                )
            }),
            camera,
            camera_view,
            offset,
        )
    }

    pub fn quads(&self) -> &[VfxQuad] {
        &self.quads
    }
    pub fn meshes(&self) -> &[VfxMeshInstance] {
        &self.meshes
    }
    pub fn aura_instances(&self) -> &[WeaponVfxAuraInstance] {
        &self.aura_instances
    }
    pub fn observed_aura_instances(&self) -> &[WeaponVfxAuraInstance] {
        &self.aura_observed_instances
    }
    pub fn random_state(&self) -> VfxClientRandomState {
        self.random_stream.snapshot()
    }
    pub fn fallback_reasons(&self) -> Vec<(&str, &str)> {
        self.attachments
            .iter()
            .filter_map(|attachment| {
                attachment
                    .target_error
                    .as_deref()
                    .or(attachment.playback.fallback_reason())
                    .map(|reason| (attachment.model_path.as_str(), reason))
            })
            .collect()
    }

    #[cfg(test)]
    fn new(data: &WeaponVfxAttachments) -> Self {
        Self::new_with_aura_resources(data, &[])
    }

    #[cfg(test)]
    fn new_with_aura_resources(
        data: &WeaponVfxAttachments,
        aura_resources: &[WeaponVfxAuraResource],
    ) -> Self {
        Self::new_with_camera(data, aura_resources, None, |_| None)
    }

    #[cfg(test)]
    fn new_with_camera(
        data: &WeaponVfxAttachments,
        aura_resources: &[WeaponVfxAuraResource],
        camera: Option<VfxBinderCameraSnapshot>,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Self {
        Self::new_with_inputs(data, aura_resources, camera, None, offset)
    }

    fn new_with_inputs(
        data: &WeaponVfxAttachments,
        aura_resources: &[WeaponVfxAuraResource],
        camera: Option<VfxBinderCameraSnapshot>,
        camera_view: Option<VfxCameraViewSnapshot>,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Self {
        Self::new_with_resource_indexes(
            data,
            aura_resources.iter().enumerate().map(|(index, resource)| {
                (
                    resource.attachment_index,
                    resource.particle_index,
                    index,
                    resource.target_model_path.as_str(),
                )
            }),
            camera,
            camera_view,
            offset,
        )
    }

    // This constructor is shared by actual GPU resources and CPU verification
    // of preparation -> retained playback -> every surface's output packet.
    fn new_with_resource_indexes<'a>(
        data: &WeaponVfxAttachments,
        resources: impl IntoIterator<Item = (usize, usize, usize, &'a str)>,
        camera: Option<VfxBinderCameraSnapshot>,
        camera_view: Option<VfxCameraViewSnapshot>,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Self {
        // Reproducible preview seed, not a live TLS snapshot. All mounts share
        // this stream in their input order, including constructors and REST.
        let random_stream = VfxSharedRandomStream::new(VfxClientRandomState::from_words([
            123456789, 362436069, 521288629, 88675123,
        ]));
        let model_skin_creation_order = VfxModelSkinCreationOrder::new();
        let mut indexes = HashMap::<usize, HashMap<usize, Vec<usize>>>::new();
        let mut prepared_paths = HashSet::new();
        let mut resource_slots = HashMap::<usize, HashMap<usize, u8>>::new();
        for (attachment, particle, resource, path) in resources {
            let slots = u8::from(data.model_skin_targets.weapon.as_deref() == Some(path)) * 2
                | u8::from(data.model_skin_targets.off_hand.as_deref() == Some(path)) * 4;
            resource_slots
                .entry(attachment)
                .or_default()
                .insert(resource, slots);
            prepared_paths.insert(path);
            indexes
                .entry(attachment)
                .or_default()
                .entry(particle)
                .or_default()
                .push(resource);
        }
        let model_skin_paths = [
            data.model_skin_targets.weapon.as_ref(),
            data.model_skin_targets.off_hand.as_ref(),
        ]
        .map(|path| {
            path.filter(|path| prepared_paths.contains(path.as_str()))
                .cloned()
        });
        let model_skin_input = preview_model_skin_input(&model_skin_paths, |_| true);
        let mut texture_base = 0;
        let mut model_base = 0;
        let mut particle_base = 0;
        let attachments = data
            .attachments
            .iter()
            .enumerate()
            .map(|(attachment_index, attachment)| {
                let texture_count = attachment.data.file.texture_paths.len();
                let model_count = attachment.data.file.models.len();
                let aura_resource_indexes = indexes.remove(&attachment_index).unwrap_or_default();
                let model_skin_indexes = aura_resource_indexes.keys().copied().collect::<Vec<_>>();
                let (targets, target_error) = match &attachment.data.skeleton {
                    Some(skeleton) => match attachment
                        .data
                        .binder_targets_for_pose(&SkeletonPose::rest_pose(skeleton))
                    {
                        Ok(targets) => (Some(targets), None),
                        Err(error) => (Some(Vec::new()), Some(error)),
                    },
                    None => (None, None),
                };
                let model_offset = offset(&attachment.model_path);
                let camera_view = camera_view.map(|view| local_camera_view(view, model_offset));
                let camera = camera_view
                    .map(|view| view.camera)
                    .or_else(|| camera.map(|camera| local_camera(camera, model_offset)));
                // Explicit preview baseline: current EXE initial image has
                // empty RanT=0, RGBA=1, XYZ random fallback=0. These are not a
                // capture of runtime-mutated globals or complete client RNG.
                let mut source = attachment
                    .data
                    .runtime()
                    .with_shared_model_skin_creation_order(&model_skin_creation_order)
                    .with_shared_client_random_stream(&random_stream, VfxClientTrigMode::Sse2)
                    .with_client_particle_constructor()
                    .with_client_color_curve_defaults(VfxClientColorCurveDefaults {
                        empty_random_type: 0,
                        empty_rgba: [1.0; 4],
                    });
                if attachment
                    .data
                    .file
                    .binders
                    .iter()
                    .any(|binder| binder.binder_type == 2)
                {
                    source =
                        source.with_client_binder_curve_defaults(VfxClientBinderCurveDefaults {
                            empty_scalar_random_type: 0,
                            vector_random_fallback: [0.0; 3],
                        });
                }
                if !model_skin_indexes.is_empty() {
                    source = source.with_client_model_skin_curve_defaults(
                        VfxClientModelSkinCurveDefaults {
                            vector_random_fallback: [0.0; 3],
                        },
                    );
                    source = source.with_model_skin_target_snapshot(VfxModelSkinTargetSnapshot {
                        selector: -1,
                        input: model_skin_input,
                    });
                }
                let object_source = targets.is_some()
                    || attachment.data.file.binders.iter().any(|binder| {
                        [
                            binder.properties_start.as_ref(),
                            binder.properties_goal.as_ref(),
                        ]
                        .into_iter()
                        .flatten()
                        .any(|point| point.bind_target_point_type == 0)
                    });
                if object_source {
                    // Packets and bone targets are in the owner MDL's local
                    // space. Its object origin is identity here; preview
                    // placement is applied once when copying output packets.
                    source = source
                        .with_binder_object_snapshot(
                            xiv_companion_data::VfxBinderObjectSnapshot::IDENTITY,
                        )
                        .expect("finite model-local object origin");
                }
                if let Some(view) = camera_view {
                    // Preview MDLs have translation placement only. Their
                    // owner Euler rotation is zero, independent of skeleton
                    // poses, authored root revision and the orbit camera.
                    source = source
                        .with_preview_camera_host_rotation([0.0; 3])
                        .expect("finite model-local host rotation")
                        .with_preview_camera_view(view)
                        .expect("validated complete preview camera");
                } else if let Some(camera) = camera {
                    source = source
                        .with_binder_camera_snapshot(camera)
                        .expect("validated preview camera");
                }
                if let Some(targets) = &targets {
                    // Already validated; an invalid initial pose supplies an
                    // empty provider so it cannot silently create static roots.
                    source = source
                        .with_binder_target_snapshot(targets)
                        .expect("validated pose targets");
                }
                if object_source {
                    // This model preview has a caster and no target objects.
                    // Supply that empty host list explicitly: Point target mode
                    // uses the original single-caster/GenD fallback.
                    source = source
                        .with_binder_target_objects(&[])
                        .expect("empty preview target-object list");
                }
                let runtime = AttachmentRuntime {
                    model_path: attachment.model_path.clone(),
                    bind_points: attachment.data.bind_points.clone(),
                    skeleton: attachment.data.skeleton.clone(),
                    targets,
                    camera,
                    camera_view,
                    requires_camera_view: attachment
                        .data
                        .file
                        .binders
                        .iter()
                        .any(|binder| binder.binder_type == 3),
                    target_error,
                    playback: VfxPlayback::new_with_model_skin(source, &model_skin_indexes),
                    soft_key_offset: attachment.data.file.global.soft_key_offset,
                    // The preview registers these mounts in input order. The
                    // client counter is global; its absolute value is unknown.
                    registration_serial: u32::try_from(attachment_index + 1).unwrap_or(u32::MAX),
                    aura_resource_indexes,
                    aura_resource_target_slots: resource_slots
                        .remove(&attachment_index)
                        .unwrap_or_default(),
                    texture_base,
                    texture_count,
                    model_base,
                    model_count,
                    particle_base,
                };
                texture_base += texture_count;
                model_base += model_count;
                particle_base += attachment.data.file.particles.len();
                runtime
            })
            .collect();
        Self {
            random_stream,
            model_skin_creation_order,
            last_input_time: None,
            model_skin_paths,
            attachments,
            document_sort_ranges: Vec::new(),
            aura_instances: Vec::new(),
            aura_observed_instances: Vec::new(),
            quads: Vec::new(),
            meshes: Vec::new(),
            quad_scratch: Vec::new(),
            mesh_scratch: Vec::new(),
        }
    }

    pub fn refresh_model_skin_targets(&mut self, accepts: impl Fn(&str) -> bool) {
        let input = preview_model_skin_input(&self.model_skin_paths, accepts);
        for attachment in &mut self.attachments {
            if !attachment.aura_resource_indexes.is_empty() {
                attachment
                    .playback
                    .set_model_skin_target_input(input)
                    .expect("prepared attachment has an explicit ModelSkin host");
            }
        }
    }

    pub fn advance_to(&mut self, time: f32) {
        self.aura_instances.clear();
        self.aura_observed_instances.clear();
        if !time.is_finite() || time < 0.0 {
            return;
        }
        if self.last_input_time.is_some_and(|previous| time < previous) {
            self.random_stream.reset();
            self.model_skin_creation_order.reset();
            // Match initial construction order: ALL constructors precede any
            // input update. Interleaving reset+Time per mount changes First.
            for attachment in &mut self.attachments {
                let result = if let Some(view) = attachment.camera_view {
                    attachment
                        .playback
                        .reset_preview_camera_view(attachment.targets.as_deref(), view)
                } else {
                    attachment.playback.reset_preview_binder_sources(
                        attachment.targets.as_deref(),
                        None,
                        attachment.camera,
                    )
                };
                if let Err(error) = result {
                    attachment.target_error = Some(error);
                }
            }
        }
        self.last_input_time = Some(time);
        for attachment in &mut self.attachments {
            let updated = if let Some(view) = attachment.camera_view {
                match attachment.playback.update_preview_camera_view_to(
                    time,
                    attachment.targets.as_deref(),
                    view,
                ) {
                    Ok(()) => true,
                    Err(error) => {
                        attachment.target_error = Some(error);
                        false
                    }
                }
            } else if let Some(camera) = attachment.camera {
                match attachment.playback.update_preview_camera_to(
                    time,
                    attachment.targets.as_deref(),
                    camera,
                ) {
                    Ok(()) => true,
                    Err(error) => {
                        attachment.target_error = Some(error);
                        false
                    }
                }
            } else {
                match &attachment.targets {
                    Some(targets) => match attachment
                        .playback
                        .update_to_with_binder_target_snapshot(time, targets)
                    {
                        Ok(()) => true,
                        Err(error) => {
                            attachment.target_error = Some(error);
                            false
                        }
                    },
                    None => attachment.playback.update_to(time),
                }
            };
            if updated {
                self.aura_instances.extend(
                    attachment
                        .playback
                        .model_skin_instances()
                        .into_iter()
                        .flat_map(|instance| attachment.aura_instances(instance)),
                );
                self.aura_observed_instances.extend(
                    attachment
                        .playback
                        .observed_model_skin_instances()
                        .into_iter()
                        .flat_map(|instance| attachment.aura_instances(instance)),
                );
            }
        }
    }

    pub fn set_camera(
        &mut self,
        camera: VfxBinderCameraSnapshot,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Result<(), String> {
        camera.validate()?;
        if self
            .attachments
            .iter()
            .any(|attachment| attachment.requires_camera_view)
        {
            return Err("Camera Binder requires a complete preview view".into());
        }
        let cameras = self
            .attachments
            .iter()
            .map(|attachment| {
                let camera = local_camera(camera, offset(&attachment.model_path));
                camera.validate()?;
                Ok(camera)
            })
            .collect::<Result<Vec<_>, String>>()?;
        for (attachment, camera) in self.attachments.iter_mut().zip(cameras) {
            attachment.camera = Some(camera);
            attachment.camera_view = None;
        }
        Ok(())
    }

    pub fn set_camera_view(
        &mut self,
        view: VfxCameraViewSnapshot,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) -> Result<(), String> {
        view.validate()?;
        if self
            .attachments
            .iter()
            .any(|attachment| attachment.requires_camera_view && attachment.camera_view.is_none())
        {
            return Err("Camera Binder requires a complete view at initial construction".into());
        }
        let views = self
            .attachments
            .iter()
            .map(|attachment| {
                let view = local_camera_view(view, offset(&attachment.model_path));
                view.validate()?;
                Ok(view)
            })
            .collect::<Result<Vec<_>, String>>()?;
        for (attachment, view) in self.attachments.iter_mut().zip(views) {
            attachment.camera = Some(view.camera);
            attachment.camera_view = Some(view);
        }
        Ok(())
    }

    pub fn set_attachment_pose(&mut self, path: &str, pose: &SkeletonPose) -> Result<(), String> {
        let attachment = self
            .attachments
            .iter_mut()
            .find(|attachment| attachment.model_path == path)
            .ok_or("VFX model attachment not found")?;
        let skeleton = attachment
            .skeleton
            .as_ref()
            .ok_or("VFX owner skeleton unavailable")?;
        let targets = VfxRuntime::bind_point_pose_targets(&attachment.bind_points, skeleton, pose)?;
        attachment.targets = Some(targets);
        attachment.target_error = None;
        Ok(())
    }

    pub fn trigger_attachment_document(
        &mut self,
        path: &str,
        scheduler_index: i32,
        trigger_number: i32,
    ) -> Result<bool, String> {
        self.attachments
            .iter_mut()
            .find(|attachment| attachment.model_path == path)
            .ok_or("VFX model attachment not found")?
            .playback
            .trigger_document(scheduler_index, trigger_number)
    }

    pub fn sample(
        &mut self,
        time: f32,
        capacity: usize,
        offset: impl Fn(&str) -> Option<[f32; 3]>,
    ) {
        self.quads.clear();
        self.meshes.clear();
        self.document_sort_ranges.clear();
        if !time.is_finite() || time < 0.0 {
            self.aura_instances.clear();
            self.aura_observed_instances.clear();
            return;
        }
        self.advance_to(time);
        let mut order_base = 0_u64;
        for attachment in &mut self.attachments {
            // Visibility and output capacity must not discard input updates.
            // Both geometry types consume the same simulation state.
            let Some(offset) = offset(&attachment.model_path) else {
                continue;
            };
            if self.quads.len() >= capacity && self.meshes.len() >= capacity {
                continue;
            }
            attachment
                .playback
                .sample(&mut self.quad_scratch, &mut self.mesh_scratch);
            let order_span = self
                .quad_scratch
                .iter()
                .filter_map(|quad| quad.draw_order)
                .chain(self.mesh_scratch.iter().filter_map(|mesh| mesh.draw_order))
                .max()
                .map_or(0, |last| last.saturating_add(1));
            if order_span != 0 {
                self.document_sort_ranges.push(VfxDocumentSortRange {
                    order_start: order_base,
                    order_end: order_base.saturating_add(order_span),
                    position: offset,
                    soft_key_offset: attachment.soft_key_offset,
                    registration_serial: attachment.registration_serial,
                });
            }
            if self.quads.len() < capacity {
                self.quads.extend(
                    self.quad_scratch
                        .iter()
                        .take(capacity - self.quads.len())
                        .map(|quad| {
                            let mut quad = *quad;
                            quad.draw_order = quad
                                .draw_order
                                .map(|order| order.saturating_add(order_base));
                            quad.particle_index += attachment.particle_base;
                            for (position, offset) in quad.position.iter_mut().zip(offset) {
                                *position += offset;
                            }
                            quad.texture_indexes = quad
                                .texture_indexes
                                .map(|index| attachment.texture_index(index));
                            quad.texture_distortion_index =
                                attachment.texture_index(quad.texture_distortion_index);
                            quad.texture_palette_index =
                                attachment.texture_index(quad.texture_palette_index);
                            quad
                        }),
                );
            }
            if self.meshes.len() < capacity {
                self.meshes.extend(
                    self.mesh_scratch
                        .iter()
                        .filter(|mesh| mesh.model_index < attachment.model_count)
                        .take(capacity - self.meshes.len())
                        .map(|mesh| {
                            let mut mesh = *mesh;
                            mesh.draw_order = mesh
                                .draw_order
                                .map(|order| order.saturating_add(order_base));
                            for (position, offset) in mesh.position.iter_mut().zip(offset) {
                                *position += offset;
                            }
                            mesh.texture_indexes = mesh
                                .texture_indexes
                                .map(|index| attachment.texture_index(index));
                            mesh.texture_distortion_index =
                                attachment.texture_index(mesh.texture_distortion_index);
                            mesh.texture_palette_index =
                                attachment.texture_index(mesh.texture_palette_index);
                            mesh.texture_normal_index =
                                attachment.texture_index(mesh.texture_normal_index);
                            mesh.reflection_texture_index =
                                attachment.texture_index(mesh.reflection_texture_index);
                            mesh.model_index += attachment.model_base;
                            mesh
                        }),
                );
            }
            // A hole between Documents stops compatible GPU instances from
            // being merged into a range that cannot later be reordered.
            order_base = order_base.saturating_add(order_span).saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiv_companion_data::*;

    #[test]
    fn aura_slots_respect_apri_and_retain_previous_binding() {
        let candidate = |target_model_path, priority, resource_index, instance_id| AuraCandidate {
            target_model_path,
            priority,
            attachment_index: 0,
            creation_order: None,
            binding: AuraBinding {
                resource_index,
                instance_id,
            },
        };
        let mut slots = Vec::new();
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::MOVE_DIRECTION_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [1.0, 0.0, 0.0],
            parent_y: [0.0, 1.0, 0.0],
            parent_z: [0.0, 0.0, 1.0],
            local_axis: [0.0, 1.0, 0.0],
            tree_old_linear: None,
        };
        advance_aura_slots(
            &mut slots,
            [
                candidate("main.mdl", 2, 0, 10),
                candidate("sub.mdl", 1, 1, 10),
                candidate("main.mdl", 3, 2, 10),
            ],
        );
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].binding.unwrap().resource_index, 2);
        assert_eq!(slots[1].binding.unwrap().resource_index, 1);
        slots[0].last_uniform = Some(SurfaceOverlayUniform {
            aura_params: [[0.25; 4]; 16],
        });
        slots[0].last_camera_facing = Some(facing);

        advance_aura_slots(&mut slots, [candidate("main.mdl", 3, 3, 11)]);
        assert_eq!(slots[0].binding.unwrap().resource_index, 2);
        advance_aura_slots(&mut slots, []);
        assert_eq!(slots[0].binding.unwrap().resource_index, 2);
        assert_eq!(slots[0].last_uniform.unwrap().aura_params[0][0], 0.25);
        assert_eq!(slots[0].last_camera_facing, Some(facing));
        advance_aura_slots(&mut slots, [candidate("main.mdl", 3, 3, 13)]);
        assert_eq!(slots[0].binding.unwrap().resource_index, 2);
        assert_eq!(slots[0].last_uniform.unwrap().aura_params[0][0], 0.25);
        assert_eq!(slots[0].last_camera_facing, Some(facing));

        advance_aura_slots(&mut slots, [candidate("main.mdl", 4, 4, 12)]);
        assert_eq!(slots[0].binding.unwrap().resource_index, 4);
        assert_eq!(slots[0].priority, 4);
        assert!(slots[0].last_uniform.is_none());
        assert!(slots[0].last_camera_facing.is_none());
        advance_aura_slots(&mut slots, [candidate("main.mdl", 3, 2, 10)]);
        assert_eq!(slots[0].binding.unwrap().resource_index, 4);
    }

    #[test]
    fn aura_first_sample_compares_shared_creation_order_and_preserves_unknown_ties() {
        let candidate = |priority, attachment_index, resource_index, instance_id| AuraCandidate {
            target_model_path: "main.mdl",
            priority,
            attachment_index,
            creation_order: None,
            binding: AuraBinding {
                resource_index,
                instance_id,
            },
        };
        let mut slots = Vec::new();
        advance_aura_slots(&mut slots, [candidate(2, 0, 1, 2), candidate(2, 0, 0, 1)]);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].priority, 2);
        assert_eq!(slots[0].binding.unwrap().resource_index, 0);
        advance_aura_slots(&mut slots, [candidate(2, 0, 1, 3)]);
        assert_eq!(slots[0].binding.unwrap().resource_index, 0);

        let mut cross_file = Vec::new();
        advance_aura_slots(
            &mut cross_file,
            [candidate(2, 0, 0, 1), candidate(2, 1, 1, 1)],
        );
        assert_eq!(cross_file.len(), 1);
        assert_eq!(cross_file[0].priority, 2);
        assert!(cross_file[0].binding.is_none());
        advance_aura_slots(&mut cross_file, [candidate(2, 1, 1, 1)]);
        assert!(cross_file[0].binding.is_none());
        advance_aura_slots(&mut cross_file, [candidate(3, 2, 2, 1)]);
        assert_eq!(cross_file[0].binding.unwrap().resource_index, 2);

        let mut mixed = Vec::new();
        advance_aura_slots(
            &mut mixed,
            [
                candidate(1, 0, 0, 1),
                candidate(2, 1, 1, 7),
                candidate(2, 1, 2, 6),
            ],
        );
        assert_eq!(mixed[0].binding.unwrap().resource_index, 2);

        let ordered = |attachment, resource, serial| AuraCandidate {
            creation_order: Some(serial),
            ..candidate(2, attachment, resource, 1)
        };
        let mut shared = Vec::new();
        advance_aura_slots(&mut shared, [ordered(1, 1, 10)]);
        advance_aura_slots(&mut shared, [ordered(0, 0, 5)]);
        assert_eq!(
            shared[0].binding.unwrap().resource_index,
            0,
            "an earlier-created resource can first register after the later one"
        );
        advance_aura_slots(&mut shared, [ordered(2, 2, 5)]);
        assert_eq!(
            shared[0].binding.unwrap().resource_index,
            0,
            "strictly equal keys retain the first binding"
        );
        advance_aura_slots(&mut shared, [ordered(1, 1, 10)]);
        assert_eq!(shared[0].binding.unwrap().resource_index, 0);
    }

    #[test]
    fn model_skin_colors_and_alpha_use_client_aura_registers() {
        let instance = VfxModelSkinInstance {
            aura_creation_order: None,
            aura_registered_target_slots: None,
            particle_index: 0,
            instance_id: 1,
            color: [0.2, 0.3, 0.4, 0.375],
            aura_color_begin: [0.1, 0.2, 0.3, 0.4],
            aura_color_end: [0.5, 0.6, 0.7, 0.8],
            aura_fresnel: [0.0, -1.0, 0.0, 2.5],
            aura_camera_facing: None,
            aura_fresnel_selector: 1.0,
            aura_sem: 1.25,
            aura_eem: 2.5,
            aura_distortion_power: 0.125,
            aura_texture_uv: [
                [[2.0, -3.0, 0.25, 0.0], [4.0, 5.0, -0.5, 0.0]],
                [[1.0, 0.0, 0.5, 0.0], [0.0, 1.0, 0.75, 0.0]],
                [[0.5, -1.0, 0.0, 0.0], [1.0, 0.5, 1.0, 0.0]],
            ],
            aura_color_combine: [0.0, 0.0, 0.0, 1.0],
            aura_material_combine: [0.0, 1.0, 0.0, 0.0],
            aura_combine_extras: [0.0, 0.0, 1.0, 0.0],
            aura_texture_controls: [1, 0, 2],
            aura_tc2_alpha_enabled: 1.0,
            aura_tc2_color_enabled: 0.0,
            aura_uv_point_density: [0.0, 3.0, 4.0, 1.0],
            aura_color_mix: 1.0,
            age: 0.0,
            total_age: 0.0,
            curve_age: 0.0,
            curve_total_age: 0.0,
            render_curve_age: 0.0,
            render_curve_total_age: 0.0,
        };
        let uniform = model_skin_aura_uniform(&instance);
        assert_eq!(std::mem::size_of::<SurfaceOverlayUniform>(), 256);
        assert_eq!(uniform.aura_params[0], instance.aura_color_begin);
        assert_eq!(uniform.aura_params[1], instance.aura_color_end);
        assert_eq!(uniform.aura_params[2], instance.aura_fresnel);
        assert_eq!(uniform.aura_params[3], [1.0, 1.25, 2.5, 0.125]);
        assert_eq!(uniform.aura_params[4], instance.aura_color_combine);
        assert_eq!(uniform.aura_params[5], instance.aura_material_combine);
        assert_eq!(uniform.aura_params[6], instance.aura_combine_extras);
        assert_eq!(uniform.aura_params[7], [0.0, 3.0, 4.0, 1.0]);
        for slot in 0..3 {
            assert_eq!(
                uniform.aura_params[8 + slot * 2],
                instance.aura_texture_uv[slot][0]
            );
            assert_eq!(
                uniform.aura_params[9 + slot * 2],
                instance.aura_texture_uv[slot][1]
            );
        }
        assert_eq!(uniform.aura_params[14].map(f32::to_bits), [1, 0, 2, 1]);
        assert_eq!(uniform.aura_params[15], [0.375, 0.0, 1.0, 1.0]);
        let bytes = bytemuck::bytes_of(&uniform);
        assert_eq!(&bytes[..16], bytemuck::bytes_of(&instance.aura_color_begin));
        assert_eq!(&bytes[16..32], bytemuck::bytes_of(&instance.aura_color_end));
        assert_eq!(&bytes[32..48], bytemuck::bytes_of(&instance.aura_fresnel));
        assert_eq!(&bytes[48..52], &1.0_f32.to_ne_bytes());
        assert_eq!(&bytes[52..56], &1.25_f32.to_ne_bytes());
        assert_eq!(&bytes[60..64], &0.125_f32.to_ne_bytes());
        assert_eq!(
            &bytes[112..128],
            bytemuck::bytes_of(&instance.aura_uv_point_density)
        );
        assert_eq!(&bytes[240..244], &0.375_f32.to_ne_bytes());
        assert_eq!(&bytes[224..228], &1_u32.to_ne_bytes());
        assert_eq!(&bytes[232..236], &2_u32.to_ne_bytes());
        assert_eq!(&bytes[236..240], &1_u32.to_ne_bytes());
        assert_eq!(&bytes[248..252], &1.0_f32.to_ne_bytes());
    }

    #[test]
    fn model_skin_camera_fresnel_refreshes_each_view_without_changing_exponent() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2] = [0.0, 0.0, 0.0, 2.5];
        uniform.aura_params[0] = [0.2, 0.4, 0.6, 0.8];

        let first =
            aura_uniform_with_camera(uniform, 1, [1.0, 2.0, 3.0], [0.0, 0.0, 1.0], None).unwrap();
        assert_eq!(first.aura_params[2], [1.0, 2.0, 3.0, 2.5]);
        let second =
            aura_uniform_with_camera(first, 1, [-4.0, 5.0, 6.0], [0.0, 0.0, 1.0], None).unwrap();
        assert_eq!(second.aura_params[2], [-4.0, 5.0, 6.0, 2.5]);
        assert_eq!(second.aura_params[0], uniform.aura_params[0]);
        assert_eq!(uniform.aura_params[2], [0.0, 0.0, 0.0, 2.5]);
        assert!(
            aura_uniform_with_camera(second, 2, [7.0, 8.0, 9.0], [0.0, 0.0, 1.0], None).is_none()
        );
        assert!(
            aura_uniform_with_camera(second, 0, [7.0, 8.0, 9.0], [0.0, 0.0, 1.0], None).is_none()
        );
    }

    #[test]
    fn model_skin_move_billboard_fresnel_refreshes_at_fixed_time() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        uniform.aura_params[0] = [0.2, 0.4, 0.6, 0.8];
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::MOVE_DIRECTION_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [1.0, 0.0, 0.0],
            parent_y: [0.0, 1.0, 0.0],
            parent_z: [0.0, 0.0, 1.0],
            local_axis: [0.0, -1.0, 0.0],
            tree_old_linear: None,
        };
        let first =
            aura_uniform_with_camera(uniform, 3, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0], Some(facing))
                .unwrap();
        assert_eq!(first.aura_params[2], [0.0, 0.0, -1.0, 2.5]);
        let second =
            aura_uniform_with_camera(first, 3, [0.0, 5.0, 0.0], [0.0, 0.0, 1.0], Some(facing))
                .unwrap();
        assert_eq!(second.aura_params[2], [0.0, -1.0, 0.0, 2.5]);
        assert_eq!(second.aura_params[0], uniform.aura_params[0]);
        assert!(
            aura_uniform_with_camera(second, 3, [0.0, 5.0, 0.0], [0.0, 0.0, 1.0], None).is_none()
        );
    }

    #[test]
    fn model_skin_axis_y_uniforms_refresh_from_view_or_position() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::BILLBOARD_AXIS_Y,
            position: [0.0; 3],
            movement_direction: [0.0; 3],
            parent_y: [0.0; 3],
            parent_z: [0.0; 3],
            local_axis: [0.0, 0.0, -2.0],
            tree_old_linear: None,
        };
        let view_front = [0.0, 0.0, -1.0];
        let view_side = [-1.0, 0.0, 0.0];
        let first = aura_uniform_with_camera(uniform, 3, [0.0, 0.0, 5.0], view_front, Some(facing))
            .unwrap();
        assert_eq!(first.aura_params[2], [0.0, 0.0, -1.0, 2.5]);
        let unchanged =
            aura_uniform_with_camera(first, 3, [5.0, 2.0, 0.0], view_front, Some(facing)).unwrap();
        assert_eq!(unchanged.aura_params[2], first.aura_params[2]);
        let turned =
            aura_uniform_with_camera(unchanged, 3, [5.0, 2.0, 0.0], view_side, Some(facing))
                .unwrap();
        assert_eq!(turned.aura_params[2], [-1.0, 0.0, 0.0, 2.5]);

        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::CAMERA_BILLBOARD_AXIS_Y,
            ..facing
        };
        let first = aura_uniform_with_camera(uniform, 3, [0.0, 2.0, 5.0], view_front, Some(facing))
            .unwrap();
        assert_eq!(first.aura_params[2], [0.0, 0.0, -1.0, 2.5]);
        let unchanged =
            aura_uniform_with_camera(first, 3, [0.0, 2.0, 5.0], view_side, Some(facing)).unwrap();
        assert_eq!(unchanged.aura_params[2], first.aura_params[2]);
        let moved =
            aura_uniform_with_camera(unchanged, 3, [5.0, 2.0, 0.0], view_side, Some(facing))
                .unwrap();
        assert_eq!(moved.aura_params[2], [-1.0, 0.0, 0.0, 2.5]);
    }

    #[test]
    fn model_skin_camera_billboard_uniform_uses_view_up_and_preserves_exponent() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::CAMERA_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [0.0; 3],
            parent_y: [0.0; 3],
            parent_z: [0.0; 3],
            local_axis: [0.0, -2.0, 0.0],
            tree_old_linear: None,
        };
        let pitched =
            aura_uniform_with_camera(uniform, 3, [5.0, 0.0, 0.0], [0.0, 0.6, 0.8], Some(facing))
                .unwrap();
        assert_eq!(pitched.aura_params[2], [0.0, -0.8, 0.6, 2.5]);
        let level =
            aura_uniform_with_camera(pitched, 3, [5.0, 0.0, 0.0], [0.0, 0.0, 1.0], Some(facing))
                .unwrap();
        assert_eq!(level.aura_params[2], [0.0, -1.0, 0.0, 2.5]);
        let retry =
            aura_uniform_with_camera(level, 3, [0.0; 3], [0.0, 0.6, 0.8], Some(facing)).unwrap();
        assert_eq!(retry.aura_params[2], [0.0, -1.0, 0.0, 2.5]);
    }

    #[test]
    fn model_skin_screen_billboard_uniform_uses_view_rotation_not_camera_position() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::SCREEN_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [0.0; 3],
            parent_y: [0.0; 3],
            parent_z: [0.0; 3],
            local_axis: [0.0, -2.0, 0.0],
            tree_old_linear: None,
        };
        let level =
            aura_uniform_with_camera(uniform, 3, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0], Some(facing))
                .unwrap();
        assert_eq!(level.aura_params[2], [0.0, -1.0, 0.0, 2.5]);
        let moved =
            aura_uniform_with_camera(level, 3, [5.0, 0.0, 0.0], [0.0, 0.0, 1.0], Some(facing))
                .unwrap();
        assert_eq!(moved.aura_params[2], level.aura_params[2]);
        let pitched =
            aura_uniform_with_camera(moved, 3, [5.0, 0.0, 0.0], [0.0, 0.6, 0.8], Some(facing))
                .unwrap();
        assert_eq!(pitched.aura_params[2], [0.0, -0.8, 0.6, 2.5]);
    }

    #[test]
    fn model_skin_camera_roll_refreshes_aura_direction_without_changing_exponent() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::SCREEN_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [0.0; 3],
            parent_y: [0.0; 3],
            parent_z: [0.0; 3],
            local_axis: [1.0, 0.0, 0.0],
            tree_old_linear: None,
        };
        let level = aura_uniform_with_camera_up(
            uniform,
            3,
            [0.0, 0.0, 5.0],
            [0.0, 0.0, 1.0],
            Some([0.0, 1.0, 0.0]),
            Some(facing),
        )
        .unwrap();
        assert_eq!(level.aura_params[2], [-1.0, 0.0, 0.0, 2.5]);
        let rolled = aura_uniform_with_camera_up(
            level,
            3,
            [0.0, 0.0, 5.0],
            [0.0, 0.0, 1.0],
            Some([-1.0, 0.0, 0.0]),
            Some(facing),
        )
        .unwrap();
        assert_eq!(rolled.aura_params[2], [0.0, -1.0, 0.0, 2.5]);
    }

    #[test]
    fn model_skin_tree_billboard_uniform_uses_old_y_and_refreshes_with_camera_position() {
        let mut uniform = SurfaceOverlayUniform {
            aura_params: [[0.0; 4]; 16],
        };
        uniform.aura_params[2][3] = 2.5;
        let facing = VfxModelSkinCameraFacing {
            mode: rotation_direction_base::TREE_BILLBOARD,
            position: [0.0; 3],
            movement_direction: [0.0; 3],
            parent_y: [0.0; 3],
            parent_z: [0.0; 3],
            local_axis: [0.0, 0.0, -2.0],
            tree_old_linear: Some([[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]]),
        };
        let front =
            aura_uniform_with_camera(uniform, 3, [0.0, 0.0, 5.0], [0.0; 3], Some(facing)).unwrap();
        assert_eq!(front.aura_params[2], [0.0, 0.0, -1.0, 2.5]);
        let side =
            aura_uniform_with_camera(front, 3, [5.0, 0.0, 0.0], [0.0; 3], Some(facing)).unwrap();
        assert_eq!(side.aura_params[2], [-1.0, 0.0, 0.0, 2.5]);
        assert!(aura_uniform_with_camera(side, 3, [0.0; 3], [0.0; 3], Some(facing)).is_none());
    }

    fn constant(value: f32) -> AvfxCurve {
        AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                interpolation: 1,
                x: 0.0,
                y: 0.0,
                z: value,
            }],
            ..Default::default()
        }
    }

    fn attachment(path: &str, position: [f32; 3]) -> WeaponVfxAttachment {
        let texture = |index| {
            Some(AvfxParticleTexture {
                enabled: true,
                texture_index: index,
                mask_texture_index: -1,
                texture_list: vec![index],
                ..Default::default()
            })
        };
        let quad = AvfxParticle {
            particle_type: Some(ParticleType::Quad),
            life: AvfxLife {
                enabled: true,
                value: 60.0,
                ..Default::default()
            },
            texture_color1: texture(1),
            texture_color2: texture(0),
            texture_color3: texture(2),
            texture_distortion: Some(AvfxParticleDistortion {
                enabled: true,
                texture_index: 0,
                power: constant(0.25),
                target_uv: [true; 4],
                ..Default::default()
            }),
            texture_palette: Some(AvfxParticleTexturePalette {
                enabled: true,
                texture_index: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mesh = AvfxParticle {
            particle_type: Some(ParticleType::LightModel),
            data: AvfxParticleData::LightModel { model_index: 1 },
            ..quad.clone()
        };
        WeaponVfxAttachment {
            model_path: path.to_string(),
            data: WeaponVfxData {
                bind_points: vec![VfxBindPoint {
                    parent_bone: None,
                    id: 4,
                    translate: position,
                    rotate: [0.0; 3],
                }],
                textures: vec![
                    None,
                    Some(VfxTextureRgba {
                        width: 1,
                        height: 1,
                        rgba: vec![255; 4],
                        ..Default::default()
                    }),
                ],
                file: AvfxFile {
                    texture_paths: vec!["missing.atex".into(), "color.atex".into()],
                    models: vec![
                        VfxModelGeometry::default(),
                        VfxModelGeometry {
                            draw: Some(VfxDrawModel::default()),
                            ..Default::default()
                        },
                    ],
                    schedulers: vec![AvfxScheduler {
                        items: vec![AvfxSchedulerItem {
                            enabled: true,
                            start_time: 0,
                            timeline_index: 0,
                        }],
                        ..Default::default()
                    }],
                    timelines: vec![AvfxTimeline {
                        binder_index: -1,
                        items: vec![AvfxTimelineItem {
                            enabled: true,
                            start_time: 0,
                            end_time: -1,
                            binder_index: 0,
                            effector_index: -1,
                            emitter_index: 0,
                            platform: 0,
                            clip_index: -1,
                        }],
                        ..Default::default()
                    }],
                    binders: vec![AvfxBinder {
                        bind_point_id: 4,
                        ..Default::default()
                    }],
                    emitters: vec![AvfxEmitter {
                        child_limit: 48,
                        create_count: constant(1.0),
                        create_interval: constant(1000.0),
                        particle_items: (0..2)
                            .map(|target_index| AvfxEmitterItem {
                                enabled: true,
                                target_index,
                                create_count: 1,
                                create_probability: 100,
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }],
                    particles: vec![quad, mesh],
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }

    fn mounts() -> WeaponVfxAttachments {
        WeaponVfxAttachments {
            attachments: vec![
                attachment("main.mdl", [1.0, 2.0, 3.0]),
                attachment("sub.mdl", [4.0, 5.0, 6.0]),
            ],
            ..Default::default()
        }
    }

    fn point_mount() -> WeaponVfxAttachments {
        let mut mount = attachment("main.mdl", [0.0; 3]);
        let file = &mut mount.data.file;
        file.binders[0].life = -1;
        file.particles.truncate(1);
        file.particles[0].life.enabled = false;
        file.particles[0].collision_type = -1;
        let emitter = &mut file.emitters[0];
        emitter.emitter_type = Some(EmitterType::Point);
        emitter.effector_index = -1;
        emitter.create_interval = constant(2.0);
        emitter.particle_items.truncate(1);
        emitter.particle_items[0].parameter_link = -1;
        emitter.position.x = Some(AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    z: 1.0,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 20,
                    interpolation: 1,
                    z: 21.0,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        });
        assert_eq!(
            VfxPlayback::new(mount.data.runtime()).fallback_reason(),
            None,
            "point attachment fixture must exercise staged playback"
        );
        WeaponVfxAttachments {
            attachments: vec![mount],
            ..Default::default()
        }
    }

    fn model_texture_fixture(case: &serde_json::Value) -> xiv_companion_data::avfx::AvfxParticle {
        use xiv_companion_data::avfx::*;
        let mode = case["mode"].as_u64().unwrap() as u32;
        let source = case["source"].as_u64().unwrap();
        let mut tx_random = constant(0.5);
        tx_random.random_type = mode;
        let mut tp_random = constant(0.25);
        tp_random.random_type = mode;
        let light = case["light"] == true;
        AvfxParticle {
            particle_type: Some(if light {
                ParticleType::LightModel
            } else {
                ParticleType::Model
            }),
            collision_type: -1,
            texture_color1: Some(AvfxParticleTexture {
                enabled: source != 4,
                use_screen_copy: source == 1 || source == 2,
                previous_frame_copy: source == 2,
                use_chara_portrait: source == 3,
                texture_list: vec![0, 1, 255, 254],
                tex_n: Some(constant(1.25)),
                tex_n_random: Some(tx_random),
                ..Default::default()
            }),
            texture_palette: Some(AvfxParticleTexturePalette {
                enabled: case["palette"] == true,
                texture_index: 7,
                offset: constant(-0.5),
                offset_random: tp_random,
                ..Default::default()
            }),
            data: if light {
                AvfxParticleData::LightModel { model_index: 0 }
            } else {
                AvfxParticleData::Model {
                    model_number_random_value: 0,
                    model_number_random_type: 0,
                    model_number_random_interval: 2,
                    fresnel_type: 0,
                    directional_light_type: 0,
                    point_light_type: 0,
                    is_lightning: false,
                    is_morph: false,
                    model_indexes: if case["emptyModels"] == true {
                        vec![]
                    } else {
                        vec![0, 1]
                    },
                    animation_number: None,
                    morph: None,
                    fresnel_curve: None,
                    fresnel_curve_random: None,
                    fresnel_rotation: None,
                    color_begin: Default::default(),
                    color_end: Default::default(),
                }
            },
            ..Default::default()
        }
    }

    #[test]
    fn default_model_texture_float_draw_reaches_attachment_meshes_and_replay() {
        use xiv_companion_data::avfx::{
            AvfxParticleData, VfxDrawModel, VfxDrawVertex, VfxModelGeometry,
        };
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/model-texture-draw.json"
        )))
        .unwrap();
        for case in cases.as_array().unwrap() {
            for invalid_models in [false, true] {
                let mut mounts = point_mount();
                let file = &mut mounts.attachments[0].data.file;
                file.emitters[0].create_interval = constant(1000.0);
                file.particles[0] = model_texture_fixture(case);
                if invalid_models {
                    if let AvfxParticleData::Model { model_indexes, .. } =
                        &mut file.particles[0].data
                    {
                        if !model_indexes.is_empty() {
                            model_indexes.fill(255);
                        }
                    }
                }
                file.models = vec![
                    VfxModelGeometry {
                        draw: Some(VfxDrawModel {
                            vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
                                .map(|position| VfxDrawVertex {
                                    position,
                                    position_w: 1.0,
                                    normal: [128, 128, 255, 0],
                                    tangent: [255, 128, 128, 0],
                                    uvs: [[0.0; 2]; 4],
                                    color: [255; 4],
                                })
                                .to_vec(),
                            indices: vec![0, 1, 2],
                        }),
                        ..Default::default()
                    };
                    2
                ];
                let mut playback = WeaponVfxPlayback::new(&mounts);
                for (i, time) in [0.0, 3.5 / 30.0, 10.0 / 30.0].into_iter().enumerate() {
                    let row = &case["instances"][0]["steps"][i];
                    playback.sample(time, 32, |_| Some([0.0; 3]));
                    assert!(playback.fallback_reasons().is_empty(), "{case}");
                    assert!(playback.quads.is_empty());
                    if case["light"] != true && (case["emptyModels"] == true || invalid_models) {
                        assert!(playback.meshes.is_empty());
                    } else {
                        assert_eq!(playback.meshes.len(), 1, "{case}");
                        let mesh = &playback.meshes[0];
                        assert_eq!(
                            serde_json::json!(mesh.texture_indexes[0]),
                            row["texture"],
                            "{case}"
                        );
                        assert_eq!(
                            serde_json::json!(mesh.palette_offset.to_bits()),
                            row["paletteOffset"],
                            "{case}"
                        );
                        let gpu = super::super::vfx::GpuVfxMeshInstance::from(mesh);
                        assert_eq!(
                            serde_json::json!(gpu.normal_params[3].to_bits()),
                            row["paletteOffset"],
                            "float TP must survive the actual GPU storage conversion"
                        );
                        assert_eq!(gpu.distortion[1], mesh.distortion_targets);
                        let expected_model = if case["light"] == true {
                            0
                        } else {
                            row["modelValue"].as_i64().unwrap() % 2
                        };
                        assert_eq!(mesh.model_index as i64, expected_model);
                    }
                    assert_eq!(
                        serde_json::json!(playback.random_state().words()),
                        row["state"],
                        "{case}"
                    );
                    let before = playback.meshes.clone();
                    playback.sample(time, 32, |_| Some([0.0; 3]));
                    assert_eq!(playback.meshes, before);
                    assert_eq!(
                        serde_json::json!(playback.random_state().words()),
                        row["state"]
                    );
                }
                playback.sample(0.0, 32, |_| Some([0.0; 3]));
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    case["instances"][0]["steps"][0]["state"]
                );
            }
        }
    }

    fn set_texture_shape(particle: &mut xiv_companion_data::avfx::AvfxParticle, shape: u64) {
        use xiv_companion_data::avfx::*;
        let (kind, data) = match shape {
            0 => (
                ParticleType::Disc,
                AvfxParticleData::Disc(AvfxParticleDataDisc {
                    parts_count: 1,
                    parts_count_u: 2,
                    parts_count_v: 2,
                    scaling_scale: 100,
                    angle: constant(1.0),
                    ..Default::default()
                }),
            ),
            1 => (
                ParticleType::Laser,
                AvfxParticleData::Laser(AvfxParticleDataLaser {
                    length: constant(1.0),
                    width: constant(1.0),
                    ..Default::default()
                }),
            ),
            2 => (
                ParticleType::Polygon,
                AvfxParticleData::Polygon(AvfxParticleDataPolygon {
                    count: constant(3.0),
                    ..Default::default()
                }),
            ),
            3 => (
                ParticleType::Windmill,
                AvfxParticleData::Windmill { uv_type: 0 },
            ),
            _ => panic!("unknown texture shape"),
        };
        particle.particle_type = Some(kind);
        particle.data = data;
    }

    fn disc_polygon_particle(case: &serde_json::Value) -> xiv_companion_data::avfx::AvfxParticle {
        use xiv_companion_data::avfx::{
            AvfxColorCurve, AvfxColorScaleRgb, AvfxParticleData, AvfxParticleDataDisc,
            AvfxParticleDataPolygon, ParticleType,
        };
        let mut p = xiv_companion_data::avfx::AvfxParticle {
            collision_type: -1,
            ..Default::default()
        };
        let shape = case["shape"].as_u64().unwrap();
        let mode = case["mode"].as_u64().unwrap() as u32;
        let profile = case["geometry"].as_u64().unwrap() as usize;
        let curve = |count: usize, base: f32, mode| AvfxCurve {
            random_type: mode,
            keys: (0..count)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
            ..Default::default()
        };
        let source = case["source"].as_u64().unwrap();
        p.texture_color1 = Some(xiv_companion_data::avfx::AvfxParticleTexture {
            enabled: source != 4,
            use_screen_copy: source == 1,
            use_chara_portrait: source == 3,
            texture_list: vec![0, 1, 255, 254],
            tex_n: Some(curve(1, 1.25, 0)),
            tex_n_random: Some(curve(1, 0.5, mode)),
            ..Default::default()
        });
        p.texture_palette = Some(xiv_companion_data::avfx::AvfxParticleTexturePalette {
            enabled: true,
            texture_index: -1,
            offset: curve(1, -0.5, 0),
            offset_random: curve(1, 0.25, mode),
            ..Default::default()
        });
        let pair = |axis: usize| {
            let profile = if profile == 9 {
                3
            } else {
                (profile + axis) % 9
            };
            let main = if shape == 0 {
                if axis == 0 { 1.0 } else { 0.125 * axis as f32 }
            } else {
                4.0
            };
            let main = if case["geometry"] == 9 {
                if shape == 0 {
                    if axis == 0 { 1.0 } else { 0.0 }
                } else {
                    3.0
                }
            } else {
                main
            };
            (
                curve(profile / 3, main, 0),
                curve(
                    profile % 3,
                    if axis % 2 == 0 { 0.5 } else { -0.25 },
                    (mode + axis as u32) & 7,
                ),
            )
        };
        let edge_color = |edge: u32| {
            let variant = case["color"].as_u64().unwrap();
            if variant == 0 {
                return AvfxColorCurve::default();
            }
            let count = if variant == 2 { 2 } else { 1 };
            let field = |slot: u32| {
                let random = slot >= 6 && slot != 10;
                let value = if random {
                    if variant == 3 {
                        0.0
                    } else if edge == 0 {
                        0.125
                    } else {
                        -0.125
                    }
                } else if slot == 0 {
                    0.5
                } else if slot == 1 {
                    0.75
                } else {
                    1.0
                };
                let mut c = curve(
                    count,
                    value,
                    if random {
                        (mode + edge * 2 + slot) & 7
                    } else {
                        0
                    },
                );
                if slot == 0 {
                    for k in &mut c.keys {
                        k.interpolation = 17;
                        k.x = k.z;
                        k.y = k.z;
                    }
                }
                c
            };
            AvfxColorCurve {
                rgb: Some(field(0)),
                alpha: Some(field(1)),
                brightness: Some(field(10)),
                scale_alpha: Some(field(5)),
                scale_rgb: Some(AvfxColorScaleRgb {
                    r: Some(field(2)),
                    g: Some(field(3)),
                    b: Some(field(4)),
                }),
                random: [6, 7, 8, 9, 11].map(|slot| Some(field(slot))),
            }
        };
        if shape == 0 {
            let pairs: [_; 9] = std::array::from_fn(pair);
            p.particle_type = Some(ParticleType::Disc);
            p.data = AvfxParticleData::Disc(AvfxParticleDataDisc {
                parts_count: 1,
                parts_count_u: 2,
                parts_count_v: 2,
                scaling_scale: 100,
                angle: pairs[0].0.clone(),
                angle_random: pairs[0].1.clone(),
                height_begin_inner: pairs[1].0.clone(),
                height_begin_inner_random: pairs[1].1.clone(),
                height_end_inner: pairs[2].0.clone(),
                height_end_inner_random: pairs[2].1.clone(),
                height_begin_outer: pairs[3].0.clone(),
                height_begin_outer_random: pairs[3].1.clone(),
                height_end_outer: pairs[4].0.clone(),
                height_end_outer_random: pairs[4].1.clone(),
                width_begin: pairs[5].0.clone(),
                width_begin_random: pairs[5].1.clone(),
                width_end: pairs[6].0.clone(),
                width_end_random: pairs[6].1.clone(),
                radius_begin: pairs[7].0.clone(),
                radius_begin_random: pairs[7].1.clone(),
                radius_end: pairs[8].0.clone(),
                radius_end_random: pairs[8].1.clone(),
                color_edge_inner: edge_color(0),
                color_edge_outer: edge_color(1),
                ..Default::default()
            });
        } else {
            let (count, count_random) = pair(0);
            p.particle_type = Some(ParticleType::Polygon);
            p.data = AvfxParticleData::Polygon(AvfxParticleDataPolygon {
                count,
                count_random,
                ..Default::default()
            });
        }
        if case["activeXYZ"] == true {
            let vectors: [_; 3] = std::array::from_fn(|index| AvfxCurve3Axis {
                x: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                y: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                z: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                random_x: Some(curve(1, 0.125, mode + index as u32 + 3)),
                random_y: Some(curve(1, 0.25, mode + index as u32 + 4)),
                random_z: Some(curve(1, 0.375, mode + index as u32 + 5)),
                ..Default::default()
            });
            [p.scale, p.rotation, p.position] = vectors;
        }
        p
    }
    #[test]
    fn default_disc_polygon_geometry_color_and_textures_reach_attachment_packets_and_replay() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/disc-polygon-geometry.json"
        )))
        .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["geometry"] == 4)
        {
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            file.particles[0] = disc_polygon_particle(case);
            // Match the native probe's successful palette-resource lookup.
            file.particles[0]
                .texture_palette
                .as_mut()
                .unwrap()
                .texture_index = 0;
            let mut playback = WeaponVfxPlayback::new(&mounts);
            for (i, time) in [0.0, 3.5 / 30.0, 10.0 / 30.0].into_iter().enumerate() {
                let row = &case["instances"][0]["steps"][i];
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert!(playback.fallback_reasons().is_empty(), "{case}");
                assert_eq!(playback.quads.len(), 1, "{case}");
                if let Some(disc) = playback.quads[0].disc {
                    assert_eq!(
                        serde_json::json!(
                            [
                                disc.angle,
                                disc.height_inner[0],
                                disc.height_inner[1],
                                disc.height_outer[0],
                                disc.height_outer[1],
                                disc.width[0],
                                disc.width[1],
                                disc.radius[0],
                                disc.radius[1]
                            ]
                            .map(f32::to_bits)
                        ),
                        row["geometryValues"],
                        "{case}"
                    );
                    assert_eq!(
                        serde_json::json!(
                            disc.color_inner
                                .into_iter()
                                .chain(disc.color_outer)
                                .map(f32::to_bits)
                                .collect::<Vec<_>>()
                        ),
                        row["colors"],
                        "{case}"
                    );
                } else {
                    assert_eq!(
                        serde_json::json!([playback.quads[0].polygon.unwrap().count]),
                        row["geometryValues"],
                        "{case}"
                    );
                }
                assert_eq!(
                    serde_json::json!(playback.quads[0].texture_indexes[0]),
                    row["texture"]
                );
                assert_eq!(
                    playback.quads[0].palette_offset.to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"],
                    "{case}"
                );
                let before = playback.quads.clone();
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(playback.quads, before);
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
            }
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(playback.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_polyline_cf_softness_positions_reach_attachment_and_replay() {
        use xiv_companion_data::avfx::{
            AvfxEmitterData, AvfxParticleDataPolyline, SphereModelEmitterData,
        };
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/polyline-history.json"
        )))
        .unwrap();
        for case in cases
            .iter()
            .filter(|c| c["production"] == true && c["distortion"] == 0)
        {
            let read = |name: &str| f32::from_bits(case[name].as_u64().unwrap() as u32);
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.timelines[0].items[0].binder_index = -1;
            file.emitters[0].position = Default::default();
            file.emitters[0].create_interval = constant(1000.0);
            file.emitters[0].particle_items[0].create_time = 0;
            file.emitters[0].emitter_type = Some(EmitterType::SphereModel);
            file.emitters[0].data = Some(AvfxEmitterData::SphereModel(SphereModelEmitterData {
                generate_method: 3,
                divide_x: 4,
                divide_y: 2,
                radius: constant(2.0),
                injection_speed: constant(0.0),
                ..Default::default()
            }));
            file.particles[0] = AvfxParticle {
                particle_type: Some(ParticleType::Polyline),
                collision_type: -1,
                rotation_direction_base: case["axis"].as_i64().unwrap() as i32,
                position: xiv_companion_data::avfx::AvfxCurve3Axis {
                    x: Some(AvfxCurve {
                        keys: vec![
                            AvfxCurveKey {
                                time: 0,
                                interpolation: 1,
                                z: 0.0,
                                x: 0.0,
                                y: 0.0,
                            },
                            AvfxCurveKey {
                                time: 32,
                                interpolation: 1,
                                z: 32.0,
                                x: 0.0,
                                y: 0.0,
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                texture_color1: Some(AvfxParticleTexture {
                    enabled: true,
                    texture_index: 1,
                    texture_list: vec![1],
                    ..Default::default()
                }),
                data: AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                    create_line_type: 1,
                    point_count: 4,
                    point_count_center: 2,
                    use_edge: true,
                    length: constant(read("length")),
                    cf: constant(read("cf")),
                    softness: constant(read("softness")),
                    width: constant(1.0),
                    width_begin: constant(1.0),
                    width_center: constant(1.0),
                    width_end: constant(1.0),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let mut value = WeaponVfxPlayback::new(&mounts);
            let times = [0.0, 0.25, 0.5, 0.5, 0.75, 1.0, 0.0];
            let mut initial_random = None;
            for (index, time) in times.into_iter().enumerate() {
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert!(value.fallback_reasons().is_empty(), "{case}");
                assert_eq!(value.quads.len(), 1, "{case}");
                let points: Vec<[u32; 3]> = value.quads[0]
                    .polyline
                    .unwrap()
                    .points()
                    .iter()
                    .map(|p| p.map(f32::to_bits))
                    .collect();
                assert_eq!(
                    serde_json::json!(points),
                    case["steps"][index]["points"],
                    "{case} step {index}"
                );
                let random = value.random_state();
                let output = value.quads.clone();
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(value.quads, output);
                assert_eq!(value.random_state(), random);
                if index == 0 {
                    initial_random = Some(random);
                }
                if index == 6 {
                    assert_eq!(Some(random), initial_random);
                }
            }
        }
    }

    #[test]
    fn default_polyline_widths_colors_and_textures_reach_attachment_packets_and_replay() {
        use xiv_companion_data::avfx::{AvfxParticleData, AvfxParticleDataPolyline};
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/polyline-geometry.json"
        )))
        .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["geometry"] == 4 && c["warm"] == false)
        {
            let mut shared = case.clone();
            shared["shape"] = 0.into();
            let mut particle = disc_polygon_particle(&shared);
            let AvfxParticleData::Disc(colors) = particle.data else {
                unreachable!()
            };
            let mode = case["mode"].as_u64().unwrap() as u32;
            let colors: [_; 6] = std::array::from_fn(|i| {
                let mut c = if i % 2 == 0 {
                    colors.color_edge_inner.clone()
                } else {
                    colors.color_edge_outer.clone()
                };
                for random in c.random.iter_mut().flatten() {
                    random.random_type = (random.random_type + 2 * (i - i % 2) as u32) & 7;
                }
                c
            });
            let pairs: [_; 7] = std::array::from_fn(|axis| {
                let profile = (4 + axis) % 9;
                let curve = |count: usize, base, random_type| xiv_companion_data::avfx::AvfxCurve {
                    random_type,
                    keys: (0..count)
                        .map(|i| xiv_companion_data::avfx::AvfxCurveKey {
                            time: (i * 10) as i16,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: base + 0.125 * i as f32,
                        })
                        .collect(),
                    ..Default::default()
                };
                (
                    curve(
                        profile / 3,
                        if axis == 5 {
                            2.0
                        } else if axis == 0 || axis == 6 {
                            0.0
                        } else {
                            1.0
                        },
                        0,
                    ),
                    curve(
                        profile % 3,
                        if axis % 2 == 0 { 0.5 } else { -0.25 },
                        (mode + axis as u32) & 7,
                    ),
                )
            });
            let [
                color_begin,
                color_center,
                color_end,
                color_edge_begin,
                color_edge_center,
                color_edge_end,
            ] = colors;
            particle.particle_type = Some(ParticleType::Polyline);
            particle.rotation_direction_base = 0;
            particle.data = AvfxParticleData::Polyline(AvfxParticleDataPolyline {
                create_line_type: 1,
                point_count: 4,
                point_count_center: case["center"].as_i64().unwrap() as i32,
                use_edge: case["edge"].as_bool().unwrap(),
                cf: pairs[0].0.clone(),
                cf_random: pairs[0].1.clone(),
                width: pairs[1].0.clone(),
                width_random: pairs[1].1.clone(),
                width_begin: pairs[2].0.clone(),
                width_begin_random: pairs[2].1.clone(),
                width_center: pairs[3].0.clone(),
                width_center_random: pairs[3].1.clone(),
                width_end: pairs[4].0.clone(),
                width_end_random: pairs[4].1.clone(),
                length: pairs[5].0.clone(),
                length_random: pairs[5].1.clone(),
                softness: pairs[6].0.clone(),
                softness_random: pairs[6].1.clone(),
                color_begin,
                color_center,
                color_end,
                color_edge_begin,
                color_edge_center,
                color_edge_end,
                ..Default::default()
            });
            particle.texture_palette.as_mut().unwrap().texture_index = 0;
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            file.particles[0] = particle;
            let mut value = WeaponVfxPlayback::new(&mounts);
            for (i, time) in [0.0, 3.5 / 30.0, 10.0 / 30.0].into_iter().enumerate() {
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert!(value.fallback_reasons().is_empty(), "{case}");
                assert_eq!(value.quads.len(), 1);
                let polyline = value.quads[0].polyline.unwrap();
                let row = &case["instances"][0]["steps"][i];
                let raw = row["geometryValues"].as_array().unwrap();
                let read = |i: usize| f32::from_bits(raw[i].as_u64().unwrap() as u32);
                assert_eq!(
                    polyline.widths.map(f32::to_bits),
                    [5, 6, 7].map(|i| (read(i) * read(4)).to_bits()),
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(value.quads[0].texture_indexes[0]),
                    row["texture"]
                );
                assert_eq!(
                    value.quads[0].palette_offset.to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    serde_json::json!(
                        polyline
                            .colors
                            .into_iter()
                            .chain(polyline.edge_colors)
                            .flatten()
                            .map(f32::to_bits)
                            .collect::<Vec<_>>()
                    ),
                    row["colors"],
                    "{case}"
                );
                let random = value.random_state();
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                let output = value.quads.clone();
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(value.quads, output);
                assert_eq!(value.random_state(), random);
            }
            value.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(value.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_line_length_and_colors_reach_attachment_packets_and_replay() {
        use xiv_companion_data::avfx::{AvfxParticleData, AvfxParticleDataLine};
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/line-geometry.json"
        )))
        .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["geometry"] == 4 && c["simple"] == false && c["warm"] == false)
        {
            let mut shared = case.clone();
            shared["shape"] = 0.into();
            let mut particle = disc_polygon_particle(&shared);
            let AvfxParticleData::Disc(colors) = particle.data else {
                unreachable!()
            };
            particle.particle_type = Some(ParticleType::Line);
            let mut length_random = constant(0.75);
            length_random.random_type = case["mode"].as_u64().unwrap() as u32;
            particle.data = AvfxParticleData::Line(AvfxParticleDataLine {
                line_count: 1,
                length: constant(2.0),
                length_random,
                color_begin: colors.color_edge_inner,
                color_end: colors.color_edge_outer,
                ..Default::default()
            });
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            file.particles[0] = particle;
            let mut value = WeaponVfxPlayback::new(&mounts);
            for (i, time) in [0.0, 3.5 / 30.0, 10.0 / 30.0].into_iter().enumerate() {
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert!(value.fallback_reasons().is_empty(), "{case}");
                assert_eq!(value.quads.len(), 1);
                let line = value.quads[0].line.unwrap();
                let row = &case["instances"][0]["steps"][i];
                assert_eq!(
                    serde_json::json!(line.length.to_bits()),
                    row["length"],
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(
                        line.color_begin
                            .into_iter()
                            .chain(line.color_end)
                            .map(f32::to_bits)
                            .collect::<Vec<_>>()
                    ),
                    row["colors"],
                    "{case}"
                );
                let random = value.random_state();
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                let output = value.quads.clone();
                value.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(value.quads, output);
                assert_eq!(value.random_state(), random);
            }
            value.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(value.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_laser_geometry_and_texture_draw_reaches_attachment_packets_and_replay() {
        use xiv_companion_data::avfx::{AvfxParticleTexture, AvfxParticleTexturePalette};
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/laser-geometry-draw.json"
        )))
        .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["geometry"] == 4)
        {
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            set_texture_shape(&mut file.particles[0], 1);
            let source = case["source"].as_u64().unwrap();
            let mode = case["mode"].as_u64().unwrap() as u32;
            let mut length_random = constant(0.75);
            length_random.random_type = mode;
            let two_keys = |first, last| {
                let mut value = constant(first);
                let mut key = value.keys[0].clone();
                key.time = 10;
                key.z = last;
                value.keys.push(key);
                value
            };
            let mut width_random = two_keys(-0.25, -0.125);
            width_random.random_type = (mode + 3) & 7;
            file.particles[0].data = xiv_companion_data::avfx::AvfxParticleData::Laser(
                xiv_companion_data::avfx::AvfxParticleDataLaser {
                    length: constant(2.0),
                    length_random,
                    width: two_keys(1.0, 1.125),
                    width_random,
                    ..Default::default()
                },
            );
            let mut random_texture = constant(0.5);
            random_texture.random_type = mode;
            let mut random_palette = constant(0.25);
            random_palette.random_type = mode;
            file.particles[0].texture_color1 = Some(AvfxParticleTexture {
                enabled: source != 4,
                use_screen_copy: source == 1,
                use_chara_portrait: source == 3,
                texture_list: vec![0, 1, 255, 254],
                tex_n: Some(constant(1.25)),
                tex_n_random: Some(random_texture),
                ..Default::default()
            });
            file.particles[0].texture_palette = Some(AvfxParticleTexturePalette {
                enabled: true,
                texture_index: 7,
                offset: constant(-0.5),
                offset_random: random_palette,
                ..Default::default()
            });
            let mut playback = WeaponVfxPlayback::new(&mounts);
            for (i, time) in [0.0, 3.5 / 30.0, 10.0 / 30.0].into_iter().enumerate() {
                let row = &case["instances"][0]["steps"][i];
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert!(playback.fallback_reasons().is_empty());
                assert_eq!(playback.quads.len(), 1);
                let dimensions = playback.quads[0].laser.unwrap();
                assert_eq!(
                    serde_json::json!([dimensions.length, dimensions.width].map(f32::to_bits)),
                    row["dimensions"],
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(playback.quads[0].texture_indexes[0]),
                    row["texture"]
                );
                assert_eq!(
                    playback.quads[0].palette_offset.to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"],
                    "{case}"
                );
                let before = playback.quads.clone();
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(playback.quads, before);
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
            }
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(playback.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_four_shape_texture_draw_reaches_attachment_packets_and_replay() {
        use xiv_companion_data::avfx::{AvfxParticleTexture, AvfxParticleTexturePalette};
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/shape-texture-draw.json"
        )))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            set_texture_shape(&mut file.particles[0], case["shape"].as_u64().unwrap());
            let source = case["source"].as_u64().unwrap();
            let mode = case["mode"].as_u64().unwrap() as u32;
            let mut random_texture = constant(0.5);
            random_texture.random_type = mode;
            let mut random_palette = constant(0.25);
            random_palette.random_type = mode;
            file.particles[0].texture_color1 = Some(AvfxParticleTexture {
                enabled: source != 4,
                use_screen_copy: source == 1,
                use_chara_portrait: source == 3,
                texture_list: vec![0, 1, 255, 254],
                tex_n: Some(constant(1.25)),
                tex_n_random: Some(random_texture),
                ..Default::default()
            });
            file.particles[0].texture_palette = Some(AvfxParticleTexturePalette {
                enabled: true,
                texture_index: 7,
                offset: constant(-0.5),
                offset_random: random_palette,
                ..Default::default()
            });
            let mut playback = WeaponVfxPlayback::new(&mounts);
            for i in 0..3 {
                let row = &case["instances"][0]["steps"][i];
                let time = i as f32 * 0.125;
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert!(playback.fallback_reasons().is_empty());
                assert_eq!(playback.quads.len(), 1);
                assert_eq!(
                    serde_json::json!(playback.quads[0].texture_indexes[0]),
                    row["texture"]
                );
                assert_eq!(
                    playback.quads[0].palette_offset.to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"],
                    "{case}"
                );
                let before = playback.quads.clone();
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(playback.quads, before);
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
            }
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(playback.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_quad_texture_draw_reaches_attachment_packets_and_replay() {
        use xiv_companion_data::avfx::{AvfxParticleTexture, AvfxParticleTexturePalette};
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/particle-textures.json"
        )))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            file.emitters[0].create_interval = constant(1000.0);
            let source = case["source"].as_u64().unwrap();
            let mode = case["mode"].as_u64().unwrap() as u32;
            let mut random_texture = constant(0.5);
            random_texture.random_type = mode;
            let mut random_palette = constant(0.25);
            random_palette.random_type = mode;
            file.particles[0].texture_color1 = Some(AvfxParticleTexture {
                enabled: source != 4,
                use_screen_copy: source == 1,
                use_chara_portrait: source == 3,
                texture_list: vec![0, 1, 255, 254],
                tex_n: Some(constant(1.25)),
                tex_n_random: Some(random_texture),
                ..Default::default()
            });
            file.particles[0].texture_palette = Some(AvfxParticleTexturePalette {
                enabled: true,
                texture_index: 7,
                offset: constant(-0.5),
                offset_random: random_palette,
                ..Default::default()
            });
            let mut playback = WeaponVfxPlayback::new(&mounts);
            for i in 0..3 {
                let row = &case["instances"][0]["steps"][i];
                let time = i as f32 * 0.125;
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert!(playback.fallback_reasons().is_empty());
                assert_eq!(playback.quads.len(), 1);
                assert_eq!(
                    serde_json::json!(playback.quads[0].texture_indexes[0]),
                    row["texture"]
                );
                assert_eq!(
                    playback.quads[0].palette_offset.to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                );
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"],
                    "{case}"
                );
                let before = playback.quads.clone();
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(playback.quads, before);
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
            }
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(playback.random_state().words()),
                case["instances"][0]["steps"][0]["state"]
            );
        }
    }

    #[test]
    fn default_particle_vr_birth_and_prewarm_reach_geometry_and_replay() {
        use xiv_companion_data::avfx::{
            AvfxEmitterData, AvfxParticleData, AvfxParticleDataLaser, ConeEmitterData, EmitterType,
            ParticleType,
        };
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../xiv-companion-data/src/avfx_sim/fixtures/particle-vr-default-draw.json"
        )))
        .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["crt"] == 0)
        {
            let mut mounts = point_mount();
            let file = &mut mounts.attachments[0].data.file;
            let emitter = &mut file.emitters[0];
            emitter.position = Default::default();
            emitter.create_interval = constant(1000.0);
            emitter.emitter_type = Some(EmitterType::Cone);
            emitter.data = Some(AvfxEmitterData::Cone(ConeEmitterData {
                inner_size: constant(0.0),
                outer_size: constant(0.0),
                injection_speed: constant(0.125),
                injection_angle: constant(0.0),
                ..Default::default()
            }));
            let item = &mut emitter.particle_items[0];
            item.local_direction = case["localDirection"].as_i64().unwrap() as i32;
            item.start_frame = if case["prewarm"] == true { 4 } else { 0 };
            item.start_frame_null_update = case["prewarm"] == true;
            let particle = &mut file.particles[0];
            if item.start_frame_null_update {
                particle.particle_type = Some(ParticleType::Laser);
                particle.data = AvfxParticleData::Laser(AvfxParticleDataLaser {
                    length: constant(1.0),
                    width: constant(1.0),
                    ..Default::default()
                });
            }
            let mode = case["mode"].as_u64().unwrap() as u32;
            particle.air_resistance = constant(0.5);
            particle.air_resistance_random = constant(0.25);
            particle.air_resistance_random.random_type = mode;
            particle.rotation_velocity = [0.25, -0.5, 0.75].map(constant);
            particle.rotation_velocity_random = std::array::from_fn(|_| {
                let mut c = constant(0.125);
                c.random_type = mode;
                c
            });
            let mut playback = WeaponVfxPlayback::new(&mounts);
            for (i, row) in case["steps"].as_array().unwrap().iter().enumerate() {
                let time = i as f32 * 0.125;
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert!(
                    playback.fallback_reasons().is_empty(),
                    "{:?}",
                    playback.fallback_reasons()
                );
                assert_eq!(playback.quads.len(), 1);
                assert_eq!(
                    serde_json::json!(playback.quads[0].position.map(f32::to_bits)),
                    row["offset"],
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
                let output = playback.quads.clone();
                playback.sample(time, 32, |_| Some([0.0; 3]));
                assert_eq!(playback.quads, output);
                assert_eq!(
                    serde_json::json!(playback.random_state().words()),
                    row["state"]
                );
            }
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            playback.sample(0.125, 32, |_| Some([0.0; 3]));
            assert_eq!(
                serde_json::json!(playback.quads[0].position.map(f32::to_bits)),
                case["steps"][1]["offset"]
            );
        }
    }

    #[test]
    fn default_particle_ars_first_reaches_geometry_and_replays() {
        use xiv_companion_data::avfx::{AvfxEmitterData, ConeEmitterData, EmitterType};
        let mut mounts = point_mount();
        let file = &mut mounts.attachments[0].data.file;
        file.emitters[0].position = Default::default();
        file.emitters[0].create_interval = constant(1000.0);
        file.emitters[0].emitter_type = Some(EmitterType::Cone);
        file.emitters[0].data = Some(AvfxEmitterData::Cone(ConeEmitterData {
            inner_size: constant(0.0),
            outer_size: constant(0.0),
            injection_speed: constant(0.125),
            injection_angle: constant(0.0),
            ..Default::default()
        }));
        file.particles[0].air_resistance = constant(0.5);
        file.particles[0].air_resistance_random = constant(0.25);
        file.particles[0].air_resistance_random.random_type = 0;
        let mut playback = WeaponVfxPlayback::new(&mounts);
        for (index, expected) in [0, 1040489382, 1048995430, 1053340729]
            .into_iter()
            .enumerate()
        {
            let time = index as f32 * 0.125;
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert!(playback.fallback_reasons().is_empty());
            assert_eq!(playback.quads.len(), 1);
            assert_eq!(playback.quads[0].position[2].to_bits(), expected);
            assert_eq!(
                playback.random_state().words(),
                [643481932, 934407046, 723553448, 3932869644]
            );
            let output = playback.quads.clone();
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert_eq!(playback.quads, output);
            assert_eq!(
                playback.random_state().words(),
                [643481932, 934407046, 723553448, 3932869644]
            );
        }
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        playback.sample(0.125, 32, |_| Some([0.0; 3]));
        assert_eq!(playback.quads[0].position[2].to_bits(), 1040489382);
    }

    #[test]
    fn default_particle_gravity_first_reaches_geometry_and_replays() {
        let mut mounts = point_mount();
        let file = &mut mounts.attachments[0].data.file;
        file.emitters[0].position = Default::default();
        file.emitters[0].create_interval = constant(1000.0);
        file.particles[0].gravity = constant(0.5);
        file.particles[0].gravity_random = constant(0.25);
        file.particles[0].gravity_random.random_type = 0;
        let mut playback = WeaponVfxPlayback::new(&mounts);
        for (index, expected) in [0, 1081430823, 1094517470, 1102795978]
            .into_iter()
            .enumerate()
        {
            let time = index as f32 * 0.125;
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert!(playback.fallback_reasons().is_empty());
            assert_eq!(playback.quads.len(), 1);
            assert_eq!(playback.quads[0].position[1].to_bits(), expected);
            assert_eq!(
                playback.random_state().words(),
                [643481932, 934407046, 723553448, 3932869644]
            );
            let output = playback.quads.clone();
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert_eq!(playback.quads, output);
            assert_eq!(
                playback.random_state().words(),
                [643481932, 934407046, 723553448, 3932869644]
            );
        }
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        playback.sample(0.125, 32, |_| Some([0.0; 3]));
        assert_eq!(playback.quads[0].position[1].to_bits(), 1081430823);
    }

    #[test]
    fn default_particle_xyz_cache_reaches_geometry_and_replays() {
        use xiv_companion_data::avfx::AvfxCurve3Axis;
        let mut mounts = point_mount();
        let file = &mut mounts.attachments[0].data.file;
        file.emitters[0].position = Default::default();
        file.emitters[0].create_interval = constant(1000.0);
        let particle = &mut file.particles[0];
        particle.rotation_direction_base = xiv_companion_data::avfx::rotation_direction_base::NONE;
        let main = [[2.0, 3.0, 4.0], [0.0; 3], [10.0, 20.0, 30.0]];
        let amplitude = [[0.5, 0.25, 0.125], [0.0, 0.0, 0.25], [0.5, 0.25, 0.125]];
        let vectors: [_; 3] = std::array::from_fn(|index| {
            let mut random = amplitude[index].map(constant);
            for c in &mut random {
                c.random_type = if index == 2 { 3 } else { 0 };
            }
            AvfxCurve3Axis {
                x: Some(constant(main[index][0])),
                y: Some(constant(main[index][1])),
                z: Some(constant(main[index][2])),
                random_x: Some(random[0].clone()),
                random_y: Some(random[1].clone()),
                random_z: Some(random[2].clone()),
                ..Default::default()
            }
        });
        particle.scale = vectors[0].clone();
        particle.rotation = vectors[1].clone();
        particle.position = vectors[2].clone();
        let mut playback = WeaponVfxPlayback::new(&mounts);
        // Original complete Quad constructor and subsequent property readers.
        // The first sample adds one zero-Time/Prepare callback to construction.
        let captures = [
            (
                [1092684281, 1101135464, 1106271419],
                [2728332712, 2381680799, 830734233, 2059906653],
            ),
            (
                [1092218370, 1100876904, 1106220839],
                [2059906653, 544153312, 20906778, 795757459],
            ),
            (
                [1092867676, 1100929569, 1106282646],
                [795757459, 1755102565, 811349640, 3380790346],
            ),
        ];
        for (i, (position, state)) in captures.into_iter().enumerate() {
            let time = i as f32 * 0.125;
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert!(playback.fallback_reasons().is_empty());
            assert_eq!(playback.quads.len(), 1);
            assert_eq!(playback.quads[0].position.map(f32::to_bits), position);
            assert_eq!(
                playback.quads[0].size,
                [
                    f32::from_bits(1075084001) * 0.5,
                    f32::from_bits(1077673984) * 0.5
                ]
            );
            assert_eq!(playback.random_state().words(), state);
            let output = playback.quads.clone();
            playback.sample(time, 32, |_| Some([0.0; 3]));
            assert_eq!(playback.quads, output);
            assert_eq!(playback.random_state().words(), state);
        }
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        assert_eq!(playback.quads[0].position.map(f32::to_bits), captures[0].0);
        assert_eq!(playback.random_state().words(), captures[0].1);
    }

    #[test]
    fn point_attachments_preserve_updates_and_cached_birth_positions() {
        let mounts = point_mount();
        let mut coarse = WeaponVfxPlayback::new(&mounts);
        let mut fine = WeaponVfxPlayback::new(&mounts);
        coarse.sample(0.0, 32, |_| Some([0.0; 3]));
        fine.sample(0.0, 32, |_| Some([0.0; 3]));
        coarse.sample(0.625, 32, |_| Some([0.0; 3]));
        // Exactly representable timestamps avoid assuming f32 subtraction of
        // frame/30 boundaries always reproduces the original frame delta.
        for seconds in [0.125, 0.25, 0.375, 0.5, 0.625] {
            fine.sample(seconds, 32, |_| Some([0.0; 3]));
        }
        assert_eq!(coarse.quads.len(), 2);
        assert_eq!(fine.quads.len(), 6);
        assert!(
            coarse
                .quads
                .iter()
                .all(|quad| quad.position == [1.0, 0.0, 0.0])
        );
        for (quad, x) in fine.quads.iter().zip([1.0, 1.0, 4.7, 8.5, 12.2, 16.0]) {
            assert!(
                (quad.position[0] - x).abs() < 1e-5,
                "{} != {x}",
                quad.position[0]
            );
        }
        let snapshot = coarse.quads.clone();
        coarse.sample(0.625, 32, |_| Some([0.0; 3]));
        assert_eq!(
            coarse.quads, snapshot,
            "reading the same frame must not update twice"
        );
        coarse.sample(0.0, 32, |_| Some([0.0; 3]));
        assert_eq!(
            coarse.quads.len(),
            1,
            "replacing/restarting an effect clears its history"
        );
    }

    fn posed_mounts() -> WeaponVfxAttachments {
        let mut mounts = point_mount();
        let mount = &mut mounts.attachments[0];
        mount.data.skeleton = Some(ModelSkeleton {
            bone_names: vec!["root".into()],
            parent_indices: vec![-1],
            rest_pose: vec![BoneTransform {
                translation: [2.0, 0.0, 0.0],
                scale: [2.0, 3.0, 1.0],
                ..BoneTransform::IDENTITY
            }],
        });
        mount.data.bind_points[0].translate = [0.25, 0.0, 0.0];
        mount.data.bind_points[0].parent_bone = Some("root".into());
        let binder = &mut mount.data.file.binders[0];
        binder.following_target_orientation = true;
        binder.transform_scale = 255;
        binder.vfx_scale_bias = 1.0;
        binder.properties_start = Some(AvfxBinderProperties {
            bind_point_id: 4,
            bind_target_point_type: 3,
            coord_update_frame: -1,
            ..Default::default()
        });
        binder.data = Some(AvfxBinderData {
            spring_strength: Some(constant(1.0)),
            ..Default::default()
        });
        mount.data.file.emitters[0].position.x = Some(constant(0.5));
        mount.data.file.emitters[0].create_interval = constant(1000.0);
        mount.data.file.emitters[0].particle_items[0].parent_influence_coord = 2;
        let mut second = mount.clone();
        second.model_path = "sub.mdl".into();
        second.data.file.emitters[0].particle_items[0].parent_influence_coord = 3;
        mounts.attachments.push(second);
        mounts
    }

    #[test]
    fn external_attachment_trigger_routes_owner_and_waits_for_next_input() {
        let mut mounts = posed_mounts();
        for mount in &mut mounts.attachments {
            let file = &mut mount.data.file;
            let mut target = file.timelines[0].clone();
            target.items[0].binder_index = -1;
            file.timelines.push(target);
            file.schedulers[0].triggers = vec![AvfxSchedulerItem {
                enabled: false,
                start_time: 99,
                timeline_index: 1,
            }];
        }
        let mut samples = WeaponVfxPlayback::new(&mounts);
        samples.sample(0.125, 32, |_| Some([0.0; 3]));
        let before = samples.quads.clone();
        assert_eq!(before.len(), 2);
        assert!(
            samples
                .trigger_attachment_document("absent.mdl", 0, 1)
                .is_err()
        );
        assert!(
            samples
                .trigger_attachment_document("main.mdl", 0, 0)
                .is_err()
        );
        assert!(
            samples
                .trigger_attachment_document("main.mdl", 0, 1)
                .unwrap()
        );
        samples.sample(0.125, 32, |_| Some([0.0; 3]));
        assert_eq!(samples.quads, before, "same input must not perform Prepare");
        samples.sample(0.25, 32, |_| Some([0.0; 3]));
        assert_eq!(samples.quads.len(), 3);
        for (index, expected) in [(0, 2), (1, 1)] {
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            samples.attachments[index]
                .playback
                .sample(&mut quads, &mut meshes);
            assert_eq!(quads.len(), expected, "trigger must route to exact owner");
        }
        samples.sample(0.0, 32, |_| Some([0.0; 3]));
        assert_eq!(samples.quads.len(), 2, "rewind discards external inputs");
    }

    fn complete_camera_mounts() -> WeaponVfxAttachments {
        let mut mounts = posed_mounts();
        for (index, mount) in mounts.attachments.iter_mut().enumerate() {
            let file = &mut mount.data.file;
            file.global.ags_enabled = false;
            file.binders = vec![AvfxBinder {
                binder_type: 3,
                life: -1,
                adjust_to_screen_enabled: true,
                properties_start: Some(AvfxBinderProperties {
                    coord_update_frame: -1,
                    generate_delay: 99,
                    ..Default::default()
                }),
                data: Some(AvfxBinderData {
                    distance: Some(constant(-2.0)),
                    ..Default::default()
                }),
                ..Default::default()
            }];
            file.emitters[0].position = Default::default();
            file.emitters[0].particle_items[0].influence_coord_scale = true;
            if index == 1 {
                file.timelines[0].binder_index = 0;
                file.timelines[0].items[0].binder_index = -1;
            }
        }
        mounts
    }

    fn complete_preview_view(height: u32) -> VfxCameraViewSnapshot {
        super::super::camera_uniform_and_vfx_view(
            [0.0; 3],
            1.0,
            [64, height],
            0.0,
            0.0,
            4.0,
            [0.0; 2],
            Default::default(),
        )
        .1
    }

    #[test]
    fn preview_spline_random_curves_reach_geometry_and_group_replay() {
        let mut mounts = posed_mounts();
        for mount in &mut mounts.attachments {
            let file = &mut mount.data.file;
            file.particles.truncate(1);
            file.emitters[0].particle_items.truncate(1);
            file.emitters[0].particle_items[0].parent_influence_coord = 2;
            file.emitters[0].create_interval = constant(1000.0);
            let mut first = constant(0.75);
            first.random_type = 1;
            let mut always = constant(0.5);
            always.random_type = 4;
            file.binders[0] = AvfxBinder {
                binder_type: 2,
                bind_point_id: 4,
                life: -1,
                properties_start: Some(AvfxBinderProperties {
                    bind_point_type: 0,
                    bind_target_point_type: 3,
                    bind_point_id: 4,
                    coord_update_frame: -1,
                    position: AvfxCurve3Axis {
                        random_x: Some(first),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                properties_goal: Some(AvfxBinderProperties {
                    bind_point_type: 0,
                    bind_target_point_type: 3,
                    bind_point_id: 4,
                    coord_update_frame: -1,
                    position: AvfxCurve3Axis {
                        x: Some(constant(5.0)),
                        random_x: Some(always.clone()),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                properties_1: Some(AvfxBinderProperties {
                    ring_enabled: true,
                    ring_radius: 2.0,
                    ring_progress_time: 1,
                    ..Default::default()
                }),
                data: Some(AvfxBinderData {
                    carry_over_factor: Some(constant(0.5)),
                    carry_over_factor_random: Some(always),
                    ..Default::default()
                }),
                ..Default::default()
            };
        }
        let mut samples = WeaponVfxPlayback::new(&mounts);
        samples.sample(0.0, 32, |_| Some([0.0; 3]));
        assert!(
            samples.fallback_reasons().is_empty(),
            "{:?}",
            samples.fallback_reasons()
        );
        assert_eq!(samples.quads.len(), 2);
        let first = samples.quads.clone();
        let initial_state = samples.random_state();
        samples.sample(0.125, 32, |_| Some([0.0; 3]));
        assert!(samples.fallback_reasons().is_empty());
        assert_eq!(samples.quads.len(), 2);
        assert_ne!(
            samples.quads, first,
            "random Spline endpoint/factor affect actual drawing transforms"
        );
        let latest = samples.quads.clone();
        let latest_state = samples.random_state();
        samples.sample(0.125, 32, |_| Some([0.0; 3]));
        assert_eq!(samples.quads, latest);
        assert_eq!(samples.random_state(), latest_state);
        samples.sample(0.0, 32, |_| Some([0.0; 3]));
        assert_eq!(samples.quads, first);
        assert_eq!(samples.random_state(), initial_state);
    }

    #[test]
    fn preview_shared_color_stream_keeps_first_and_always_phases_and_replays_group() {
        let mut mounts = posed_mounts();
        for mount in &mut mounts.attachments {
            let file = &mut mount.data.file;
            file.particles.truncate(1);
            file.emitters[0].particle_items.truncate(1);
            file.emitters[0].particle_items[0].parent_influence_color = 2;
            file.emitters[0].create_interval = constant(1000.0);
            let mut always = constant(0.25);
            always.random_type = 4;
            file.emitters[0].color.random[0] = Some(always.clone());
            file.particles[0].color.random[0] = Some(always);
            let mut first = constant(0.75);
            first.random_type = 1;
            file.particles[0].color.random[1] = Some(first);
        }
        let offset = |_: &str| Some([0.0; 3]);
        let mut samples = WeaponVfxPlayback::new(&mounts);
        samples.sample(0.0, 32, offset);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        assert_eq!(samples.quads.len(), 2);
        let initial = samples.quads.clone();
        assert_ne!(
            initial[0].color, initial[1].color,
            "mounts must not copy the same random seed"
        );
        let first_state = samples.random_stream.snapshot();
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.client_binder_random_state() == Some(first_state))
        );
        samples.sample(0.0, 32, offset);
        assert_eq!(samples.quads, initial);
        assert_eq!(samples.random_stream.snapshot(), first_state);
        samples.sample(0.125, 32, offset);
        for (quad, old) in samples.quads.iter().zip(&initial) {
            assert_ne!(quad.color[0], old.color[0], "Always runs on properties");
            assert_eq!(
                quad.color[1], old.color[1],
                "constructor First survives updates"
            );
        }
        let current = samples.quads.clone();
        let current_state = samples.random_stream.snapshot();
        samples.sample(f32::NAN, 32, offset);
        assert_eq!(samples.random_stream.snapshot(), current_state);
        samples.sample(0.125, 32, offset);
        assert_eq!(samples.quads, current);
        assert_eq!(samples.random_stream.snapshot(), current_state);
        let camera = VfxBinderCameraSnapshot {
            position: [0.0, 0.0, 4.0],
            basis: xiv_companion_data::VFX_IDENTITY_BASIS,
            parallel_direction: [0.0, 0.0, 1.0],
        };
        samples.set_camera(camera, offset).unwrap();
        samples.sample(0.125, 0, |_| None); // hidden/capacity zero still runs changed input once
        assert_ne!(samples.random_stream.snapshot(), current_state);
        let paused_state = samples.random_stream.snapshot();
        samples.sample(0.125, 32, offset);
        assert_eq!(samples.random_stream.snapshot(), paused_state);
        samples.sample(0.0, 32, offset);
        let mut fresh = WeaponVfxPlayback::new_with_camera(&mounts, &[], Some(camera), offset);
        fresh.sample(0.0, 32, offset);
        assert_eq!(
            samples.quads, fresh.quads,
            "all constructors must precede Time when replaying a group"
        );
        assert_eq!(
            samples.random_stream.snapshot(),
            fresh.random_stream.snapshot()
        );
        assert_eq!(samples.quads, initial);
    }

    #[test]
    fn complete_camera_view_mounts_translate_once_resize_while_paused_and_keep_birth() {
        let mounts = complete_camera_mounts();
        let offset = |path: &str| {
            Some(if path == "main.mdl" {
                [1.0, 0.0, 0.0]
            } else {
                [-2.0, 1.0, 0.0]
            })
        };
        let mut view = complete_preview_view(1280);
        let mut samples =
            WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), offset);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        for (index, position) in [[-1.0, 0.0, 4.0], [2.0, -1.0, 4.0]].into_iter().enumerate() {
            let local = samples.attachments[index].camera_view.unwrap();
            assert_eq!(local.camera.position, position);
            assert_eq!(local.inverse_view.position, position);
        }
        samples.sample(0.0, 8, offset);
        assert_eq!(samples.quads.len(), 2);
        for quad in &samples.quads {
            assert_eq!(quad.position, [0.0, 0.0, 2.0]);
        }
        let before = samples.quads.clone();
        view.screen_height = 5120;
        samples.set_camera_view(view, offset).unwrap();
        // Hidden/zero-capacity frames still commit the camera input.
        samples.sample(0.0, 0, |_| None);
        samples.sample(0.0, 8, offset);
        assert_eq!(samples.quads.len(), 2);
        assert_eq!(
            samples.quads[0].parent_basis[0][0],
            before[0].parent_basis[0][0] * 2.0
        );
        assert_eq!(
            samples.quads[1], before[1],
            "Initial retains its birth scale"
        );
        let resized = samples.quads.clone();
        samples.sample(0.0, 8, offset);
        assert_eq!(samples.quads, resized);
        view.inverse_view.position[0] = 5.0;
        view.camera.position[0] = 5.0;
        samples.set_camera_view(view, offset).unwrap();
        samples.sample(0.125, 8, offset);
        assert_eq!(samples.quads[0].position, [5.0, 0.0, 2.0]);
        assert_eq!(samples.quads[1].position, [0.0, 0.0, 2.0]);
        samples.sample(0.0, 8, offset);
        let mut fresh = WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), offset);
        fresh.sample(0.0, 8, offset);
        assert_eq!(samples.quads, fresh.quads);
        assert_eq!(
            samples.quads[1].position,
            [5.0, 0.0, 2.0],
            "rewind uses current source at birth"
        );
    }

    #[test]
    fn complete_camera_view_rejects_partial_updates_and_all_owner_overflow_atomically() {
        let mounts = complete_camera_mounts();
        let view = complete_preview_view(1280);
        let mut samples =
            WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), |_| Some([0.0; 3]));
        samples.sample(0.0, 8, |_| Some([0.0; 3]));
        let before = samples.quads.clone();
        let sources = samples
            .attachments
            .iter()
            .map(|a| a.camera_view)
            .collect::<Vec<_>>();
        assert!(samples.set_camera(view.camera, |_| Some([0.0; 3])).is_err());
        let mut invalid = view;
        invalid.inverse_view.position[0] = f32::MAX;
        assert!(
            samples
                .set_camera_view(invalid, |path| Some(if path == "sub.mdl" {
                    [-f32::MAX, 0.0, 0.0]
                } else {
                    [0.0; 3]
                }))
                .is_err()
        );
        assert_eq!(
            samples
                .attachments
                .iter()
                .map(|a| a.camera_view)
                .collect::<Vec<_>>(),
            sources
        );
        samples.sample(0.0, 8, |_| Some([0.0; 3]));
        assert_eq!(samples.quads, before);
        let mut incomplete =
            WeaponVfxPlayback::new_with_camera(&mounts, &[], Some(view.camera), |_| None);
        assert!(incomplete.set_camera_view(view, |_| None).is_err());
        assert!(
            incomplete
                .attachments
                .iter()
                .all(|a| a.camera_view.is_none())
        );
    }

    #[test]
    fn complete_camera_ags_on_mounts_use_owner_rotation_without_bone_or_view_rotation() {
        let mut mounts = complete_camera_mounts();
        for mount in &mut mounts.attachments {
            let file = &mut mount.data.file;
            file.global.ags_enabled = true;
            file.global.revised_rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
            file.binders[0].properties_start = None;
        }
        let offset = |path: &str| {
            Some(if path == "main.mdl" {
                [1.0, 0.0, 0.0]
            } else {
                [-2.0, 1.0, 0.0]
            })
        };
        let mut view = complete_preview_view(1280);
        let mut samples =
            WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), offset);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        samples.sample(0.0, 8, offset);
        assert_eq!(samples.quads.len(), 2);
        // Scheduler Camera reads the zero Euler owner, despite the owner's
        // nonuniform bone scales and the file's nonzero revised root rotation.
        assert_eq!(
            samples.quads[1].parent_basis,
            xiv_companion_data::VfxBinderMatrix::IDENTITY.basis
        );
        assert_eq!(samples.quads[1].position, [0.0, 0.0, 2.0]);
        assert_ne!(samples.quads[0].parent_basis, samples.quads[1].parent_basis);
        let initial_timeline = samples.quads[1].clone();
        view.inverse_view.basis = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        view.inverse_view.position[0] = 5.0;
        view.camera.basis = view.inverse_view.basis;
        view.camera.position[0] = 5.0;
        view.screen_height = 5120;
        samples.set_camera_view(view, offset).unwrap();
        samples.sample(0.125, 8, offset);
        assert_eq!(
            samples.quads[1], initial_timeline,
            "Initial preserves owner/view birth"
        );
        assert_ne!(samples.quads[0].position, samples.quads[1].position);
        samples.sample(0.0, 8, offset);
        let mut fresh = WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), offset);
        fresh.sample(0.0, 8, offset);
        assert_eq!(
            samples.quads, fresh.quads,
            "rewind uses fresh Common storage"
        );
        assert_ne!(samples.quads[1], initial_timeline);
    }

    #[test]
    fn complete_camera_caster_pose_queries_refresh_while_paused_and_keep_initial_birth() {
        let mut mounts = complete_camera_mounts();
        for mount in &mut mounts.attachments {
            let file = &mut mount.data.file;
            file.global.ags_enabled = true;
            file.binders[0].transform_scale = 255;
            file.binders[0].vfx_scale_bias = 1.0;
            file.binders[0].transform_scale_depth_offset = true;
            file.particles[0].depth_offset = 1.0;
            let point = file.binders[0].properties_start.as_mut().unwrap();
            point.bind_target_point_type = 3;
            point.bind_point_id = 4;
            point.bind_point_type = 1;
        }
        let view = complete_preview_view(1280);
        let mut samples =
            WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), |_| None);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        samples.sample(0.0, 8, |_| Some([0.0; 3]));
        assert_eq!(samples.quads.len(), 2);
        let before = samples.quads.clone();
        // ElementId offset .25 is transformed by the bone's scale before
        // translation. This independent ray projection matches bTSd.
        let depth = |position: [f32; 3], scale: [f32; 3]| {
            let ray = [-position[0], -position[1], 4.0 - position[2]];
            let length2 = ray.iter().map(|v| v * v).sum::<f32>();
            (ray.into_iter()
                .zip(scale)
                .map(|(r, s)| (r * s) * (r * s))
                .sum::<f32>()
                / length2)
                .sqrt()
        };
        for quad in &before {
            assert!((quad.depth_offset - depth([2.5, 0.0, 0.0], [2.0, 3.0, 1.0])).abs() < 1e-6);
        }
        let mut pose =
            SkeletonPose::rest_pose(mounts.attachments[0].data.skeleton.as_ref().unwrap());
        pose.set_transform(
            0,
            BoneTransform {
                translation: [80.0, -50.0, 40.0],
                scale: [4.0, 6.0, 2.0],
                ..BoneTransform::IDENTITY
            },
        )
        .unwrap();
        samples.set_attachment_pose("main.mdl", &pose).unwrap();
        samples.set_attachment_pose("sub.mdl", &pose).unwrap();
        samples.sample(0.0, 0, |_| None);
        samples.sample(0.0, 8, |_| Some([0.0; 3]));
        assert_eq!(samples.quads.len(), 2);
        assert_eq!(
            samples.quads[0].position, before[0].position,
            "Camera discards queried target position"
        );
        for (actual, birth) in samples.quads.iter().zip(&before) {
            assert_eq!(
                actual.parent_basis, birth.parent_basis,
                "AGS-on auxiliary is a birth cache, separate from query scale"
            );
            assert_eq!(
                actual.position, birth.position,
                "queried pose does not replace the view or Initial geometry"
            );
            assert!(
                (actual.depth_offset - depth([81.0, -50.0, 40.0], [4.0, 6.0, 2.0])).abs() < 1e-6
            );
            assert!(
                (actual.depth_offset - birth.depth_offset).abs() > 1.0,
                "zero-delta pose must refresh the current depth query"
            );
        }
        let paused = samples.quads.clone();
        samples.sample(0.0, 8, |_| Some([0.0; 3]));
        assert_eq!(samples.quads, paused);
    }

    #[test]
    fn complete_camera_view_missing_game_sources_remain_diagnostic_during_updates() {
        let mut mounts = complete_camera_mounts();
        for mount in &mut mounts.attachments {
            mount.data.file.global.ags_enabled = true;
            // FitGround is still an unknown game-only point producer, even
            // when the model's object and ElementId providers are available.
            mount.data.file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .bind_target_point_type = 1;
        }
        let view = complete_preview_view(1280);
        let mut samples =
            WeaponVfxPlayback::new_with_inputs(&mounts, &[], None, Some(view), |_| None);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_some())
        );
        samples
            .set_camera_view(complete_preview_view(5120), |_| None)
            .unwrap();
        samples.sample(0.125, 8, |_| Some([0.0; 3]));
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_some() && a.target_error.is_none())
        );
    }

    #[test]
    fn camera_does_not_block_existing_camera_independent_static_mounts() {
        let mounts = point_mount();
        let camera = VfxBinderCameraSnapshot {
            basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            parallel_direction: [0.0, 0.0, 1.0],
            position: [0.0, 0.0, 3.0],
        };
        let mut mounted =
            WeaponVfxPlayback::new_with_camera(&mounts, &[], Some(camera), |_| Some([0.0; 3]));
        let mut previous = WeaponVfxPlayback::new(&mounts);
        for time in [0.0, 0.125, 0.25, 0.0] {
            mounted
                .set_camera(
                    VfxBinderCameraSnapshot {
                        position: [time, 0.0, 3.0],
                        ..camera
                    },
                    |_| Some([0.0; 3]),
                )
                .unwrap();
            mounted.sample(time, 32, |_| Some([0.0; 3]));
            previous.sample(time, 32, |_| Some([0.0; 3]));
            assert_eq!(mounted.quads, previous.quads);
            assert_eq!(mounted.attachments[0].target_error, None);
            assert_eq!(mounted.attachments[0].playback.fallback_reason(), None);
        }
    }

    #[test]
    fn attachment_camera_is_local_at_birth_and_updates_hidden_paused_sources_atomically() {
        let mut mounts = posed_mounts();
        for mount in &mut mounts.attachments {
            mount.data.file.binders[0].rotation_type = 1;
        }
        let camera = VfxBinderCameraSnapshot {
            basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            parallel_direction: [0.0, 0.0, 1.0],
            position: [1.0, 2.0, 3.0],
        };
        let offset = |path: &str| {
            Some(if path == "main.mdl" {
                [1.0, 0.0, 0.0]
            } else {
                [-2.0, 1.0, 0.0]
            })
        };
        let mut samples = WeaponVfxPlayback::new_with_camera(&mounts, &[], Some(camera), offset);
        assert_eq!(
            samples.attachments[0].camera.unwrap().position,
            [0.0, 2.0, 3.0]
        );
        assert_eq!(
            samples.attachments[1].camera.unwrap().position,
            [3.0, 1.0, 3.0]
        );
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        samples.sample(0.125, 8, offset);
        let before = samples.quads.clone();
        assert_eq!(before.len(), 2);
        let moved = VfxBinderCameraSnapshot {
            basis: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            position: [4.0, 2.0, 3.0],
            ..camera
        };
        samples.set_camera(moved, offset).unwrap();
        samples.sample(0.125, 0, |_| None);
        assert!(samples.quads.is_empty());
        samples.sample(0.125, 8, offset);
        assert_ne!(samples.quads[0].position, before[0].position);
        assert_eq!(samples.quads[1], before[1], "Initial retains birth camera");
        let after = samples.quads.clone();
        samples.sample(0.125, 8, offset);
        assert_eq!(samples.quads, after);
        let inputs = samples
            .attachments
            .iter()
            .map(|a| a.camera)
            .collect::<Vec<_>>();
        assert!(
            samples
                .set_camera(camera, |path| if path == "sub.mdl" {
                    Some([f32::NAN, 0.0, 0.0])
                } else {
                    offset(path)
                })
                .is_err()
        );
        assert_eq!(
            samples
                .attachments
                .iter()
                .map(|a| a.camera)
                .collect::<Vec<_>>(),
            inputs
        );
        samples.set_camera(camera, offset).unwrap();
        samples.sample(0.0, 8, offset);
        let mut fresh = WeaponVfxPlayback::new_with_camera(&mounts, &[], Some(camera), offset);
        fresh.sample(0.0, 8, offset);
        assert_eq!(samples.quads, fresh.quads);
    }

    #[test]
    fn mounted_point_target_mode_with_empty_host_list_matches_caster_delay() {
        let mut targets = posed_mounts();
        for attachment in &mut targets.attachments {
            let file = &mut attachment.data.file;
            file.timelines[0].binder_index = 0;
            file.timelines[0].items[0].binder_index = -1;
            file.schedulers = vec![xiv_companion_data::avfx::AvfxScheduler {
                items: vec![xiv_companion_data::avfx::AvfxSchedulerItem {
                    enabled: true,
                    start_time: 0,
                    timeline_index: 0,
                }],
                ..Default::default()
            }];
            file.binders[0].life = 30;
            let properties = file.binders[0].properties_start.as_mut().unwrap();
            properties.bind_point_type = 1;
            properties.generate_delay = 2;
        }
        let mut caster = targets.clone();
        for attachment in &mut caster.attachments {
            attachment.data.file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .bind_point_type = 0;
        }
        let mut target_samples = WeaponVfxPlayback::new(&targets);
        let mut caster_samples = WeaponVfxPlayback::new(&caster);
        for time in [0.0, 2.0 / 30.0, 4.0 / 30.0, 0.0] {
            target_samples.sample(time, 32, |_| Some([0.0; 3]));
            caster_samples.sample(time, 32, |_| Some([0.0; 3]));
            assert_eq!(target_samples.quads, caster_samples.quads);
            assert!(
                target_samples
                    .attachments
                    .iter()
                    .all(|a| a.playback.fallback_reason().is_none())
            );
            assert_eq!(target_samples.quads.len(), if time == 0.0 { 0 } else { 2 });
        }
    }

    #[test]
    fn mounted_point_item_empty_targets_create_no_caster_particles() {
        let mut targets = posed_mounts();
        for attachment in &mut targets.attachments {
            let file = &mut attachment.data.file;
            file.timelines[0].binder_index = -1;
            file.timelines[0].items[0].binder_index = 0;
            file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .bind_point_type = 1;
        }
        let mut caster = targets.clone();
        for attachment in &mut caster.attachments {
            attachment.data.file.binders[0]
                .properties_start
                .as_mut()
                .unwrap()
                .bind_point_type = 0;
        }
        let mut target_samples = WeaponVfxPlayback::new(&targets);
        let mut caster_samples = WeaponVfxPlayback::new(&caster);
        for time in [0.0, 2.0 / 30.0, 0.0] {
            target_samples.sample(time, 32, |_| Some([0.0; 3]));
            caster_samples.sample(time, 32, |_| Some([0.0; 3]));
            assert!(target_samples.quads.is_empty());
            assert_eq!(caster_samples.quads.len(), 2);
            assert!(
                target_samples
                    .attachments
                    .iter()
                    .all(|a| a.playback.fallback_reason().is_none())
            );
        }
    }

    fn check_mounted_linear_empty_targets(outer: bool) {
        for kind in [1, 4] {
            for (start, goal) in [(0, 1), (1, 0), (1, 1)] {
                let mut targets = posed_mounts();
                for attachment in &mut targets.attachments {
                    let file = &mut attachment.data.file;
                    file.timelines[0].binder_index = if outer { 0 } else { -1 };
                    file.timelines[0].items[0].binder_index = if outer { -1 } else { 0 };
                    let binder = &mut file.binders[0];
                    binder.binder_type = kind;
                    let mut end = binder.properties_start.clone().unwrap();
                    end.bind_point_type = goal;
                    binder.properties_goal = Some(end);
                    binder.properties_start.as_mut().unwrap().bind_point_type = start;
                    binder.data = Some(AvfxBinderData {
                        carry_over_factor: Some(constant(0.5)),
                        ..Default::default()
                    });
                }
                let mut caster = targets.clone();
                for attachment in &mut caster.attachments {
                    let binder = &mut attachment.data.file.binders[0];
                    binder.properties_start.as_mut().unwrap().bind_point_type = 0;
                    binder.properties_goal.as_mut().unwrap().bind_point_type = 0;
                }
                let mut target_samples = WeaponVfxPlayback::new(&targets);
                let mut caster_samples = WeaponVfxPlayback::new(&caster);
                for time in [0.0, 2.0 / 30.0, 0.0] {
                    target_samples.sample(time, 32, |_| Some([0.0; 3]));
                    caster_samples.sample(time, 32, |_| Some([0.0; 3]));
                    assert!(target_samples.quads.is_empty());
                    assert_eq!(caster_samples.quads.len(), 2);
                    assert!(
                        target_samples
                            .attachments
                            .iter()
                            .all(|a| a.playback.fallback_reason().is_none())
                    );
                }
            }
        }
    }

    #[test]
    fn mounted_linear_item_empty_targets_create_no_caster_particles() {
        check_mounted_linear_empty_targets(false);
    }

    #[test]
    fn mounted_linear_scheduler_empty_targets_create_no_caster_particles() {
        check_mounted_linear_empty_targets(true);
    }

    #[test]
    fn bone_pose_attachments_keep_birth_history_and_owner_identity_while_hidden() {
        let mounts = posed_mounts();
        let mut samples = WeaponVfxPlayback::new(&mounts);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        samples.sample(0.0, 8, |_| Some([1.0, 0.0, 0.0]));
        let initial = samples.quads.clone();
        assert_eq!(initial.len(), 2);
        for quad in &initial {
            assert!((quad.position[0] - 4.5).abs() < 1e-5, "{:?}", quad.position);
        }
        let mut pose =
            SkeletonPose::rest_pose(mounts.attachments[0].data.skeleton.as_ref().unwrap());
        pose.set_transform(
            0,
            BoneTransform {
                translation: [8.0, 1.0, 0.0],
                rotation: quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2),
                scale: [3.0, 0.5, 1.0],
            },
        )
        .unwrap();
        samples.set_attachment_pose("main.mdl", &pose).unwrap();
        // Hidden / zero capacity must still commit the new source.
        samples.sample(0.125, 0, |_| None);
        assert!(samples.quads.is_empty());
        samples.sample(0.125, 8, |_| Some([1.0, 0.0, 0.0]));
        assert!((samples.quads[0].position[0] - 9.0).abs() < 1e-5);
        assert!((samples.quads[0].position[1] - 3.25).abs() < 1e-5);
        assert_eq!(
            samples.quads[1].position, initial[1].position,
            "other MDL must not consume main pose"
        );
        samples.set_attachment_pose("sub.mdl", &pose).unwrap();
        samples.sample(0.25, 8, |_| Some([1.0, 0.0, 0.0]));
        assert_eq!(
            samples.quads[1].position, initial[1].position,
            "Initial particle retains rest birth source"
        );
        let rest = SkeletonPose::rest_pose(mounts.attachments[0].data.skeleton.as_ref().unwrap());
        samples.set_attachment_pose("main.mdl", &rest).unwrap();
        samples.sample(0.25, 8, |_| Some([1.0, 0.0, 0.0]));
        assert!(
            (samples.quads[0].position[0] - 9.0).abs() < 1e-5,
            "equal timestamp must not update twice"
        );
        samples.sample(0.0, 8, |_| Some([1.0, 0.0, 0.0]));
        assert_eq!(samples.quads[0].position, initial[0].position);
        assert!(
            (samples.quads[1].position[0] - 9.0).abs() < 1e-5,
            "seek rebuilds sub birth at its current pose"
        );
    }

    #[test]
    fn object_origin_attachments_ignore_bone_pose_and_apply_owner_placement_once() {
        let mut mounts = posed_mounts();
        for mount in &mut mounts.attachments {
            let binder = &mut mount.data.file.binders[0];
            binder.bind_point_id = 0;
            binder
                .properties_start
                .as_mut()
                .unwrap()
                .bind_target_point_type = 0;
            binder.properties_start.as_mut().unwrap().bind_point_id = 0;
            mount.data.bind_points[0].id = 0;
            mount.data.bind_points[0].translate = [100.0, 0.0, 0.0];
        }
        let mut samples = WeaponVfxPlayback::new(&mounts);
        assert!(
            samples
                .attachments
                .iter()
                .all(|v| v.playback.fallback_reason().is_none())
        );
        let placement = [1.0, 2.0, 3.0];
        samples.sample(0.0, 8, |_| Some(placement));
        assert_eq!(samples.quads.len(), 2);
        for quad in &samples.quads {
            assert_eq!(quad.position, [1.5, 2.0, 3.0]);
        }
        let original = samples.quads.clone();
        let mut pose =
            SkeletonPose::rest_pose(mounts.attachments[0].data.skeleton.as_ref().unwrap());
        pose.set_translation(0, [8.0, -5.0, 4.0]).unwrap();
        samples.set_attachment_pose("main.mdl", &pose).unwrap();
        samples.sample(0.125, 8, |_| Some(placement));
        assert_eq!(
            samples.quads.iter().map(|q| q.position).collect::<Vec<_>>(),
            original.iter().map(|q| q.position).collect::<Vec<_>>()
        );
        samples.sample(0.125, 8, |_| Some([2.0, 3.0, 4.0]));
        for quad in &samples.quads {
            assert_eq!(quad.position, [2.5, 3.0, 4.0]);
        }
    }

    #[test]
    fn bone_pose_attachment_rejects_unknown_owner_and_invalid_pose_without_losing_source() {
        let mounts = posed_mounts();
        let mut samples = WeaponVfxPlayback::new(&mounts);
        let skeleton = mounts.attachments[0].data.skeleton.as_ref().unwrap();
        let mut pose = SkeletonPose::rest_pose(skeleton);
        assert!(samples.set_attachment_pose("absent.mdl", &pose).is_err());
        let before = samples.attachments[0].targets.clone();
        pose.set_translation(0, [f32::NAN, 0.0, 0.0]).unwrap();
        assert!(samples.set_attachment_pose("main.mdl", &pose).is_err());
        assert_eq!(samples.attachments[0].targets, before);
        let mut unavailable = WeaponVfxPlayback::new(&point_mount());
        assert!(
            unavailable
                .set_attachment_pose("main.mdl", &pose)
                .unwrap_err()
                .contains("skeleton")
        );
    }

    #[test]
    fn point_attachments_advance_while_hidden_or_over_output_capacity() {
        let mut mounts = point_mount();
        mounts.attachments[0].data.file.particles[0].gravity = constant(0.01);
        let mut samples = WeaponVfxPlayback::new(&mounts);
        samples.sample(0.0, 32, |_| Some([0.0; 3]));
        samples.sample(0.125, 32, |_| None);
        assert!(samples.quads.is_empty());
        samples.sample(0.25, 0, |_| Some([0.0; 3]));
        assert!(samples.quads.is_empty());
        samples.sample(0.375, 32, |_| Some([2.0, 0.0, 0.0]));
        assert_eq!(samples.quads.len(), 4);
        for (quad, x) in samples.quads.iter().zip([3.0, 3.0, 6.7, 10.5]) {
            assert!((quad.position[0] - x).abs() < 1e-5);
        }
        // Quantized steps 3.7, 3.8, 3.7 accumulate even without GPU output.
        for (quad, y) in samples.quads.iter().zip([0.8363, 0.4219, 0.1369, 0.0]) {
            assert!((quad.position[1] - y).abs() < 1e-5);
        }
        let mut hidden = WeaponVfxPlayback::new(&mounts);
        for time in [0.0, 0.125, 0.25, 0.375] {
            hidden.advance_to(time);
        }
        hidden.sample(0.375, 32, |_| Some([2.0, 0.0, 0.0]));
        assert_eq!(
            hidden.quads, samples.quads,
            "advancing without GPU submission retains the same history"
        );
    }

    #[test]
    fn attachments_preserve_resource_slots_and_invalid_references() {
        let mut mounts = mounts();
        mounts.attachments[0].data.file.global.soft_key_offset = -0.25;
        mounts.attachments[1].data.file.global.soft_key_offset = 1.5;
        // Missing decoded entries must not change the next file's resource base.
        mounts.attachments[0].data.textures.truncate(1);
        for particle in &mut mounts.attachments[0].data.file.particles {
            particle.texture_distortion.as_mut().unwrap().texture_index = 2;
            particle.texture_palette.as_mut().unwrap().texture_index = 2;
        }
        for attachment in &mut mounts.attachments {
            let mesh = &mut attachment.data.file.particles[1];
            mesh.texture_normal = Some(AvfxParticleTextureNormal {
                enabled: true,
                texture_index: 1,
                ..Default::default()
            });
            mesh.texture_reflection = Some(AvfxParticleTextureReflection {
                enabled: true,
                texture_index: 1,
                ..Default::default()
            });
        }
        let (textures, models) = attachment_resources(&mounts);
        assert_eq!(textures.len(), 4);
        assert!(textures[..3].iter().all(Option::is_none));
        assert_eq!(textures[3].as_ref().unwrap().rgba, [255; 4]);
        assert_eq!(models.len(), 4);
        let mut samples = WeaponVfxPlayback::new(&mounts);
        samples.sample(0.0, 8, |path| {
            Some(if path == "main.mdl" {
                [0.0, 0.0, 2.0]
            } else {
                [0.0, 0.0, -2.0]
            })
        });
        assert_eq!(samples.quads.len(), 2);
        assert_eq!(samples.meshes.len(), 2);
        assert_eq!(samples.quads[0].particle_type, Some(ParticleType::Quad));
        assert_eq!(samples.quads[1].particle_type, Some(ParticleType::Quad));
        assert_eq!(samples.quads[0].particle_index, 0);
        assert_eq!(samples.quads[1].particle_index, 2);
        assert_eq!(
            samples
                .quads
                .iter()
                .map(|quad| quad.soft_key_offset)
                .collect::<Vec<_>>(),
            [-0.25, 1.5]
        );
        assert_eq!(
            samples
                .meshes
                .iter()
                .map(|mesh| mesh.soft_key_offset)
                .collect::<Vec<_>>(),
            [-0.25, 1.5]
        );
        assert_eq!(
            samples
                .quads
                .iter()
                .map(|quad| quad.draw_order)
                .collect::<Vec<_>>(),
            [Some(0), Some(3)]
        );
        assert_eq!(
            samples
                .meshes
                .iter()
                .map(|mesh| mesh.draw_order)
                .collect::<Vec<_>>(),
            [Some(1), Some(4)]
        );
        assert_eq!(
            samples.document_sort_ranges,
            [
                VfxDocumentSortRange {
                    order_start: 0,
                    order_end: 2,
                    position: [0.0, 0.0, 2.0],
                    soft_key_offset: -0.25,
                    registration_serial: 1,
                },
                VfxDocumentSortRange {
                    order_start: 3,
                    order_end: 5,
                    position: [0.0, 0.0, -2.0],
                    soft_key_offset: 1.5,
                    registration_serial: 2,
                }
            ]
        );
        assert_eq!(samples.quads[0].texture_indexes, [1, 0, i32::MAX, -1]);
        assert_eq!(samples.quads[1].texture_indexes, [3, 2, i32::MAX, -1]);
        assert_eq!(samples.quads[0].texture_distortion_index, i32::MAX);
        assert_eq!(samples.quads[1].texture_distortion_index, 2);
        assert_eq!(samples.meshes[0].texture_distortion_index, i32::MAX);
        assert_eq!(samples.meshes[1].texture_distortion_index, 2);
        assert_eq!(samples.quads[0].texture_palette_index, i32::MAX);
        assert_eq!(samples.quads[1].texture_palette_index, 3);
        assert_eq!(samples.meshes[0].texture_palette_index, i32::MAX);
        assert_eq!(samples.meshes[1].texture_palette_index, 3);
        assert_eq!(samples.meshes[0].texture_normal_index, 1);
        assert_eq!(samples.meshes[1].texture_normal_index, 3);
        assert_eq!(samples.meshes[0].reflection_texture_index, 1);
        assert_eq!(samples.meshes[1].reflection_texture_index, 3);
        assert_eq!(samples.quads[0].distortion_power, 63.0 / 255.0);
        assert_eq!(samples.meshes[0].distortion_power, 0.25);
        assert_eq!(
            samples.meshes[0].texture_indexes,
            samples.quads[0].texture_indexes
        );
        assert_eq!(
            samples.meshes[1].texture_indexes,
            samples.quads[1].texture_indexes
        );
        assert_eq!(
            samples
                .meshes
                .iter()
                .map(|mesh| mesh.model_index)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(samples.attachments[1].texture_index(-1), -1);
        assert_eq!(samples.attachments[1].texture_index(0), 2);
        assert_eq!(samples.attachments[1].texture_index(i32::MAX), i32::MAX);
    }

    #[test]
    fn attachments_follow_current_owner_offsets_without_accumulating_translation() {
        let mut samples = WeaponVfxPlayback::new(&mounts());
        samples.sample(0.0, 8, |path| {
            Some(if path == "main.mdl" {
                [-2.0, 0.0, 0.0]
            } else {
                [2.0, 0.0, 0.0]
            })
        });
        assert_eq!(
            samples
                .quads
                .iter()
                .map(|quad| quad.position)
                .collect::<Vec<_>>(),
            [[-1.0, 2.0, 3.0], [6.0, 5.0, 6.0]]
        );
        for (quad, mesh) in samples.quads.iter().zip(&samples.meshes) {
            assert_eq!(quad.position, mesh.position);
        }
        samples.sample(0.5, 8, |_| Some([0.0; 3]));
        assert_eq!(samples.quads[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(samples.quads[1].position, [4.0, 5.0, 6.0]);
        samples.sample(0.0, 8, |path| (path == "sub.mdl").then_some([0.0; 3]));
        assert_eq!(samples.quads.len(), 1);
        assert_eq!(samples.meshes.len(), 1);
        assert_eq!(samples.quads[0].position, [4.0, 5.0, 6.0]);
        assert_eq!(samples.meshes[0].model_index, 3);
        samples.sample(0.0, 8, |_| None);
        assert!(samples.quads.is_empty() && samples.meshes.is_empty());
    }

    #[test]
    fn attachments_share_capacity_and_clear_previous_samples() {
        let mut samples = WeaponVfxPlayback::new(&mounts());
        for capacity in [2, 1, 0, 2] {
            samples.sample(0.0, capacity, |_| Some([0.0; 3]));
            assert_eq!(samples.quads.len(), capacity);
            assert_eq!(samples.meshes.len(), capacity);
        }
        samples.sample(3.0, 8, |_| Some([0.0; 3]));
        assert!(samples.quads.is_empty() && samples.meshes.is_empty());
    }

    #[test]
    fn model_skin_arrays_keep_definition_identity_and_target_role() {
        let mut mounts = mounts();
        mounts.attachments.truncate(1);
        mounts.model_skin_targets.weapon = Some("main.mdl".into());
        mounts.model_skin_targets.off_hand = Some("sub.mdl".into());
        let attachment = &mut mounts.attachments[0];
        attachment.data.avfx_path = "aura.avfx".into();
        attachment.data.file.global.a_pri = 0x102;
        attachment.data.textures = vec![
            Some(VfxTextureRgba {
                width: 1,
                height: 1,
                rgba: vec![10; 4],
                source_mip_count: 1,
                ..Default::default()
            }),
            Some(VfxTextureRgba {
                width: 1,
                height: 1,
                rgba: vec![20; 4],
                source_mip_count: 1,
                ..Default::default()
            }),
        ];
        attachment.data.file.particles = [(2, 0, 2, 3), (4, 1, 1, 0)]
            .map(
                |(aura_target, texture_index, border_u, filter)| AvfxParticle {
                    particle_type: Some(ParticleType::ModelSkin),
                    data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                        aura_target,
                        ..Default::default()
                    }),
                    texture_color2: Some(AvfxParticleTexture {
                        enabled: true,
                        texture_index,
                        texture_border_u: border_u,
                        texture_filter: filter,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .to_vec();

        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            inputs
                .iter()
                .map(|input| (
                    input.attachment_index,
                    input.particle_index,
                    input.target_model_path.as_str(),
                    input.packed.mips[0].rgba[0],
                    input.border,
                    input.filter,
                    input.priority,
                ))
                .collect::<Vec<_>>(),
            [
                (0, 0, "main.mdl", 10, [2, 0], 3, 2),
                (0, 1, "sub.mdl", 20, [1, 0], 0, 2),
            ]
        );

        mounts.attachments[0].data.textures[1] = None;
        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert_eq!(inputs.len(), 1);
        assert!(diagnostics[0].contains("Ptcl[1] Aura texture array"));
        mounts.model_skin_targets.weapon = None;
        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert!(inputs.is_empty());
        assert!(diagnostics[0].contains("Ptcl[0] has no loaded Aura target"));
    }

    #[test]
    fn cpu_aura_preparation_requires_the_actual_rendered_target_surface() {
        let mut mounts = mounts();
        mounts.attachments.truncate(1);
        mounts.model_skin_targets.weapon = Some("main.mdl".into());
        let attachment = &mut mounts.attachments[0];
        attachment.data.textures = vec![Some(VfxTextureRgba {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
            source_mip_count: 1,
            ..Default::default()
        })];
        attachment.data.file.particles = vec![AvfxParticle {
            particle_type: Some(ParticleType::ModelSkin),
            data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                aura_target: 2,
                ..Default::default()
            }),
            texture_color2: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                ..Default::default()
            }),
            ..Default::default()
        }];
        let mut material = super::super::material::fallback_material();
        material.shader_package_name = Some("character.shpk".into());
        let mut mesh = super::super::tests::test_mesh("aura", 0.0);
        mesh.path = "main.mdl#mesh0".into();
        let mut model = crate::ModelData {
            bounds: Default::default(),
            materials: vec![material],
            textures: vec![],
            meshes: vec![mesh],
        };
        let prepare = |model: &crate::ModelData| {
            prepare_weapon_vfx_aura_inputs(
                model,
                &mounts,
                super::super::PreparedModelOptions::default(),
            )
        };
        for shader in ["skin.shpk", "character.shpk", "characterlegacy.shpk"] {
            model.materials[0].shader_package_name = Some(shader.into());
            let (inputs, diagnostics) = prepare(&model);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert_eq!(inputs.len(), 1);
            assert_eq!(inputs[0].target_model_path, "main.mdl");
            assert_eq!(inputs[0].packed.mips[0].rgba, vec![255; 4]);
        }
        model.meshes[0].path = "other.mdl#mesh0".into();
        let (inputs, diagnostics) = prepare(&model);
        assert!(inputs.is_empty());
        assert!(diagnostics[0].contains("no compatible Aura surface"));
        model.meshes[0].path = "main.mdl#mesh0".into();
        model.materials[0].shader_package_name = Some("bg.shpk".into());
        assert!(prepare(&model).0.is_empty());
        model.materials[0].shader_package_name = Some("skin.shpk".into());
        model.materials[0].character_colors = Some(crate::ModelMaterialCharacterColors {
            colors: Default::default(),
            decal_texture: Some(0),
        });
        assert!(prepare(&model).0.is_empty());
        model.materials[0].character_colors = None;
        model.meshes[0].indices.clear();
        assert!(prepare(&model).0.is_empty());
    }

    #[test]
    fn default_modelskin_axes_reach_both_selected_surface_uniforms_and_replay() {
        // Literal outputs of client 140400a80 with its actual SSE2 CRT,
        // constant FrRt/FrC and identity or diagonal [2,-3,.5] model matrix.
        let rotation = [0x3ea0d97c, 0xbf3d2f1b, 0x3f9dbc02].map(f32::from_bits);
        for (mode, expected) in [
            (2, [0x3f775938, 0xbdf4b6b6, 0xbe69e3ae, 0x3ec00000]),
            (3, [0xbd95b086, 0xbf7be0f5, 0xbe26ff35, 0x3ec00000]),
        ] {
            let (mut mounts, _) = combined_model_skin_mount();
            let particle = &mut mounts.attachments[0].data.file.particles[0];
            if mode == 3 {
                particle.scale.x = Some(constant(2.0));
                particle.scale.y = Some(constant(-3.0));
                particle.scale.z = Some(constant(0.5));
            }
            let AvfxParticleData::ModelSkin(data) = &mut particle.data else {
                unreachable!()
            };
            data.fresnel_type = mode;
            data.fresnel_rotation.x = Some(constant(rotation[0]));
            data.fresnel_rotation.y = Some(constant(rotation[1]));
            data.fresnel_rotation.z = Some(constant(rotation[2]));
            data.fresnel_curve = constant(0.375);
            let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
            assert!(diagnostics.is_empty());
            let mut playback =
                WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                    Some([0.0; 3])
                });
            let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
            playback.sample(0.125, 32, |_| Some([0.0; 3]));
            assert!(playback.fallback_reasons().is_empty());
            bindings.update(0.125, &playback);
            let selected = bindings.selected_instances();
            assert_eq!(selected.len(), 2);
            for selected in &selected {
                assert_eq!(
                    selected.instance.aura_fresnel.map(f32::to_bits),
                    expected,
                    "mode {mode}"
                );
                assert_eq!(
                    model_skin_aura_uniform(&selected.instance).aura_params[2].map(f32::to_bits),
                    expected
                );
            }
            let random = playback.random_state();
            playback.sample(0.125, 32, |_| Some([0.0; 3]));
            bindings.update(0.125, &playback);
            assert_eq!(bindings.selected_instances(), selected);
            assert_eq!(playback.random_state(), random);
            playback.sample(0.25, 32, |_| Some([0.0; 3]));
            bindings.update(0.25, &playback);
            playback.sample(0.125, 32, |_| Some([0.0; 3]));
            bindings.update(0.125, &playback);
            assert_eq!(bindings.selected_instances(), selected);
            assert_eq!(playback.random_state(), random);
        }
    }

    #[test]
    fn default_modelskin_uv_shares_cached_rows_gates_draws_and_replays_both_surfaces() {
        use xiv_companion_data::avfx::{AvfxCurve2Axis, AvfxUvSet};

        let (mut mounts, _) = combined_model_skin_mount();
        let (baseline_inputs, _) = model_skin_aura_inputs(&mounts);
        let mut baseline = WeaponVfxPlayback::from_prepared_aura_inputs(
            &mounts,
            &baseline_inputs,
            None,
            None,
            |_| Some([0.0; 3]),
        );
        baseline.sample(0.0, 32, |_| Some([0.0; 3]));
        let mut expected = baseline.random_state();
        let random = |value, mode| {
            let mut curve = constant(value);
            curve.random_type = mode;
            curve
        };
        let set = AvfxUvSet {
            scale: AvfxCurve2Axis {
                x: Some(constant(2.0)),
                y: Some(constant(3.0)),
                random_x: Some(random(0.5, 0)),
                random_y: Some(random(0.25, 0)),
                ..Default::default()
            },
            scroll: AvfxCurve2Axis {
                random_x: Some(random(0.5, 3)),
                random_y: Some(random(0.25, 3)),
                ..Default::default()
            },
            ..Default::default()
        };
        let particle = &mut mounts.attachments[0].data.file.particles[0];
        // Two active sets: three First draws each and two Always draws per
        // Numeric. An unused empty set still consumes its five First draws.
        particle.uv_sets = vec![set.clone(), set, AvfxUvSet::default()];
        particle.texture_color3 = particle.texture_color2.clone();
        particle.texture_distortion = Some(AvfxParticleDistortion {
            enabled: true,
            texture_index: 0,
            uv_set_index: 1,
            ..Default::default()
        });
        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert!(diagnostics.is_empty());
        let mut playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        assert!(playback.fallback_reasons().is_empty());
        for _ in 0..15 {
            expected.next_u16();
        }
        assert_eq!(playback.random_state(), expected);
        bindings.update(0.0, &playback);
        let first = bindings.selected_instances();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].instance, first[1].instance);
        let rows = first[0].instance.aura_texture_uv;
        assert_eq!(rows[0], rows[1]); // TC2 and TC3 reference the same UV set.
        assert_ne!(rows[0], rows[2]); // TD references its own cached First/Always.
        for selected in &first {
            let uniform = model_skin_aura_uniform(&selected.instance);
            assert_eq!(&uniform.aura_params[8..14], rows.as_flattened());
        }
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        bindings.update(0.0, &playback);
        assert_eq!(bindings.selected_instances(), first);
        assert_eq!(playback.random_state(), expected);

        playback.sample(0.125, 32, |_| Some([0.0; 3]));
        bindings.update(0.125, &playback);
        let replay = bindings.selected_instances();
        let replay_random = playback.random_state();
        let retained_rows = replay[0].instance.aura_texture_uv;
        // First scale stays fixed; Always scrolling changes on numeric updates.
        assert_eq!(retained_rows[0][0][0], rows[0][0][0]);
        assert_eq!(retained_rows[0][1][1], rows[0][1][1]);
        assert_ne!(retained_rows[0][0][2], rows[0][0][2]);
        playback.refresh_model_skin_targets(|_| false);
        playback.sample(0.25, 32, |_| Some([0.0; 3]));
        bindings.update(0.25, &playback);
        assert_eq!(bindings.selected_instances(), replay);
        assert_eq!(playback.random_state(), replay_random);
        // Paused recovery runs one successful Numeric for the shared instance,
        // irrespective of its two surfaces and three texture references.
        playback.refresh_model_skin_targets(|_| true);
        playback.sample(0.25, 32, |_| Some([0.0; 3]));
        bindings.update(0.25, &playback);
        let mut recovered_random = replay_random;
        for _ in 0..4 {
            recovered_random.next_u16();
        }
        assert_eq!(playback.random_state(), recovered_random);
        let recovered = bindings.selected_instances();
        assert_eq!(recovered[0].instance, recovered[1].instance);
        assert_ne!(recovered[0].instance.aura_texture_uv, retained_rows);
        playback.sample(0.5, 32, |_| Some([0.0; 3]));
        bindings.update(0.5, &playback);
        // Reproduce the same input series, including its initial zero update.
        // Always draws belong to callbacks, so equal timestamps alone do not
        // identify equal random states after different update histories.
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        bindings.update(0.0, &playback);
        assert_eq!(bindings.selected_instances(), first);
        assert_eq!(playback.random_state(), expected);
        playback.sample(0.125, 32, |_| Some([0.0; 3]));
        bindings.update(0.125, &playback);
        assert_eq!(bindings.selected_instances(), replay);
        assert_eq!(playback.random_state(), replay_random);
    }

    fn combined_model_skin_mount() -> (WeaponVfxAttachments, crate::ModelData) {
        let mut mounts = point_mount();
        mounts.model_skin_targets = WeaponVfxModelTargets {
            weapon: Some("main.mdl".into()),
            off_hand: Some("sub.mdl".into()),
        };
        let attachment = &mut mounts.attachments[0];
        attachment.data.avfx_path = "combined-aura.avfx".into();
        attachment.data.textures = vec![Some(VfxTextureRgba {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
            source_mip_count: 1,
            ..Default::default()
        })];
        attachment.data.file.texture_paths = vec!["aura.atex".into()];
        attachment.data.file.emitters[0].create_interval = constant(1000.0);
        let mut sem = constant(1.0);
        sem.keys.push(AvfxCurveKey {
            time: 30,
            z: 4.0,
            ..sem.keys[0]
        });
        attachment.data.file.particles[0] = AvfxParticle {
            particle_type: Some(ParticleType::ModelSkin),
            collision_type: -1,
            life: AvfxLife {
                enabled: true,
                value: 10.0,
                ..Default::default()
            },
            data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                aura_target: 6,
                fresnel_type: 1,
                sem,
                ..Default::default()
            }),
            texture_color2: Some(AvfxParticleTexture {
                enabled: true,
                texture_index: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut material = super::super::material::fallback_material();
        material.shader_package_name = Some("character.shpk".into());
        let meshes = ["main.mdl", "sub.mdl"]
            .into_iter()
            .map(|path| {
                let mut mesh = super::super::tests::test_mesh(path, 0.0);
                mesh.path = format!("{path}#mesh0");
                mesh
            })
            .collect();
        (
            mounts,
            crate::ModelData {
                bounds: Default::default(),
                materials: vec![material],
                textures: vec![],
                meshes,
            },
        )
    }

    #[test]
    fn default_particle_constructor_prefix_reaches_aura_sem_and_replays() {
        let (mut mounts, _) = combined_model_skin_mount();
        let AvfxParticleData::ModelSkin(data) =
            &mut mounts.attachments[0].data.file.particles[0].data
        else {
            unreachable!()
        };
        data.fresnel_type = 0;
        data.sem = constant(1.0);
        data.sem_random = constant(0.5);
        data.sem_random.random_type = 0;
        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert!(diagnostics.is_empty());
        let mut playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        assert!(playback.fallback_reasons().is_empty());
        bindings.update(0.0, &playback);
        let initial = bindings.selected_instances();
        assert_eq!(initial.len(), 2);
        // Captured by probe-particle-construction: one original empty parent
        // Color First, complete base/ModelSkin constructors, original SEM
        // reader. Common setup/resources controlled; forty shared draws.
        let expected_state = [391216208, 3936394860, 1299350043, 4150927415];
        assert_eq!(playback.random_state().words(), expected_state);
        for selected in &initial {
            assert_eq!(selected.instance.aura_sem.to_bits(), 1062333317);
            assert_eq!(
                model_skin_aura_uniform(&selected.instance).aura_params[3][1].to_bits(),
                1062333317
            );
        }
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        bindings.update(0.0, &playback);
        assert_eq!(bindings.selected_instances(), initial);
        assert_eq!(playback.random_state().words(), expected_state);
        playback.sample(0.125, 32, |_| Some([0.0; 3]));
        bindings.update(0.125, &playback);
        playback.sample(0.0, 32, |_| Some([0.0; 3]));
        bindings.update(0.0, &playback);
        assert_eq!(bindings.selected_instances(), initial);
        assert_eq!(playback.random_state().words(), expected_state);
    }

    fn aura_pack_fixture(case: &serde_json::Value) -> (WeaponVfxPlayback, Vec<WeaponVfxAuraInput>) {
        use xiv_companion_data::avfx::{AvfxColorCurve, AvfxCurve2Axis, AvfxUvSet};
        let (mut mounts, _) = combined_model_skin_mount();
        // Resource allocation/readiness is controlled, as in the original
        // Numeric harness. This does not assert all-disabled sampler preparation.
        let (inputs, diagnostics) = model_skin_aura_inputs(&mounts);
        assert!(diagnostics.is_empty());
        let number = |value: &serde_json::Value| f32::from_bits(value.as_u64().unwrap() as u32);
        let color = |values: &serde_json::Value| {
            let mut rgb = constant(number(&values[2]));
            rgb.keys[0].x = number(&values[0]);
            rgb.keys[0].y = number(&values[1]);
            AvfxColorCurve {
                rgb: Some(rgb),
                alpha: Some(constant(number(&values[3]))),
                ..Default::default()
            }
        };
        let texture = |flags: u32| AvfxParticleTexture {
            enabled: flags & 1 != 0,
            texture_index: 0,
            uv_set_index: ((flags >> 4) & 7) as i32,
            color_to_alpha: flags & 4 != 0,
            calculate_color: ((flags >> 7) & 7) as i32,
            calculate_alpha: ((flags >> 10) & 3) as i32,
            ..Default::default()
        };
        let particle = &mut mounts.attachments[0].data.file.particles[0];
        particle.color = color(&case["base"]);
        particle.draw_mode = case["profile"].as_i64().unwrap() as i32 & 15;
        let AvfxParticleData::ModelSkin(data) = &mut particle.data else {
            unreachable!()
        };
        data.fresnel_type = case["mode"].as_i64().unwrap() as i32;
        data.cm = case["profile"].as_i64().unwrap() as i32 & 1;
        data.color_begin = color(&case["begin"]);
        data.color_end = color(&case["end"]);
        data.fresnel_curve = constant(number(&case["exponent"]));
        data.sem = constant(number(&case["sem"]));
        data.eem = constant(number(&case["eem"]));
        data.fresnel_rotation.x = Some(constant(number(&case["rotation"][0])));
        data.fresnel_rotation.y = Some(constant(number(&case["rotation"][1])));
        data.fresnel_rotation.z = Some(constant(number(&case["rotation"][2])));
        data.uv_point_density.x = Some(constant(number(&case["density"][0])));
        data.uv_point_density.y = Some(constant(number(&case["density"][1])));
        data.uv_point_density.z = Some(constant(number(&case["density"][2])));
        particle.uv_sets = case["uv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|values| AvfxUvSet {
                scale: AvfxCurve2Axis {
                    x: Some(constant(number(&values[0]))),
                    y: Some(constant(number(&values[1]))),
                    ..Default::default()
                },
                scroll: AvfxCurve2Axis {
                    x: Some(constant(number(&values[2]))),
                    y: Some(constant(number(&values[3]))),
                    ..Default::default()
                },
                rotation: constant(number(&values[4])),
                ..Default::default()
            })
            .collect();
        particle.texture_color2 = Some(texture(case["tc2"].as_u64().unwrap() as u32));
        particle.texture_color3 = case["tc3Present"]
            .as_bool()
            .unwrap()
            .then(|| texture(case["tc3"].as_u64().unwrap() as u32));
        let flags = case["td"].as_u64().unwrap() as u32;
        particle.texture_distortion = Some(AvfxParticleDistortion {
            enabled: flags & 1 != 0,
            texture_index: 0,
            uv_set_index: ((flags >> 5) & 7) as i32,
            target_uv: std::array::from_fn(|i| flags & (2 << i) != 0),
            power: constant(number(&case["power"])),
            ..Default::default()
        });
        let playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        assert!(playback.fallback_reasons().is_empty());
        (playback, inputs)
    }

    #[test]
    fn default_aura_pack_matches_original_disabled_and_signed_zero_layers() {
        // Captured from original 140401490 + 271e00 + two 37be80 calls,
        // EXE 2026.09.15.0000.0000; curves/resources are controlled fixtures.
        // Keep literals so ordinary tests do not require local game artifacts.
        let cases: serde_json::Value = serde_json::from_str(r#"
[
  {
    "crt": 0,
    "mode": 2,
    "profile": 0,
    "variant": 0,
    "tc2": 0,
    "tc3": 1424,
    "tc3Present": true,
    "td": 64,
    "base": [1048576000, 1056964608, 1061158912, 1059061760],
    "begin": [1052770304, 1061158912, 1066401792, 1069547520],
    "end": [1040187392, 1048576000, 1052770304, 1056964608],
    "rotation": [1050728828, 3208458011, 1067301890],
    "density": [0, 2147483648, 0],
    "exponent": 1052770304,
    "sem": 1067450368,
    "eem": 3204448256,
    "power": 3208642560,
    "uv": [[2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648]],
    "output": [1035993088, 1052770304, 1062731776, 1069547520, 1023410176, 1040187392, 1049624576, 1056964608, 1064786232, 3186931382, 3194610606, 1052770304, 0, 1067450368, 3204448256, 0, 0, 0, 0, 0, 1065353216, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1065353216, 0, 0, 0, 0, 1065353216, 0, 0, 1065353216, 0, 0, 0, 0, 1065353216, 0, 0, 1065353216, 0, 0, 0, 0, 1065353216, 0, 0, 0, 0, 0, 0, 1059061760, 0, 0, 0]
  },
  {
    "crt": 0,
    "mode": 2,
    "profile": 0,
    "variant": 1,
    "tc2": 1,
    "tc3": 1425,
    "tc3Present": true,
    "td": 65,
    "base": [1048576000, 1056964608, 1061158912, 1059061760],
    "begin": [1052770304, 1061158912, 1066401792, 1069547520],
    "end": [1040187392, 1048576000, 1052770304, 1056964608],
    "rotation": [1050728828, 3208458011, 1067301890],
    "density": [0, 2147483648, 0],
    "exponent": 1052770304,
    "sem": 1067450368,
    "eem": 3204448256,
    "power": 3208642560,
    "uv": [[2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648], [2147483648, 0, 2147483648, 2147483648, 2147483648]],
    "output": [1035993088, 1052770304, 1062731776, 1069547520, 1023410176, 1040187392, 1049624576, 1056964608, 1064786232, 3186931382, 3194610606, 1052770304, 0, 1067450368, 3204448256, 3208642560, 0, 0, 0, 1065353216, 1065353216, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1059061760, 1065353216, 0, 0]
  }
]
"#).unwrap();
        for case in cases.as_array().unwrap() {
            let (mut playback, inputs) = aura_pack_fixture(case);
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
            bindings.update(0.0, &playback);
            let selected = bindings.selected_instances();
            assert_eq!(selected.len(), 2);
            for selected in &selected {
                let uniform = model_skin_aura_uniform(&selected.instance);
                assert_eq!(uniform.aura_params[15][3], 1.0);
                for (lane, value) in uniform
                    .aura_params
                    .as_flattened()
                    .iter()
                    .take(63)
                    .enumerate()
                {
                    assert_eq!(
                        u64::from(value.to_bits()),
                        case["output"][lane].as_u64().unwrap(),
                        "variant {} lane {lane}",
                        case["variant"]
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "complete original Aura Numeric and 256-byte copy, controlled curves/host/resources"]
    fn compare_original_aura_numeric_pack_with_default_playback() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("modelskin-aura-pack-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut cases = 0;
        for (ordinal, case) in original["cases"].as_array().unwrap().iter().enumerate() {
            if case["crt"] != 0 {
                continue;
            } // Default preview explicitly uses SSE2.
            let (mut playback, inputs) = aura_pack_fixture(case);
            playback.sample(0.0, 32, |_| Some([0.0; 3]));
            assert!(playback.fallback_reasons().is_empty());
            let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
            bindings.update(0.0, &playback);
            let selected = bindings.selected_instances();
            assert_eq!(selected.len(), 2);
            for selected in &selected {
                let mut uniform = model_skin_aura_uniform(&selected.instance);
                if case["mode"] == 1 {
                    uniform =
                        aura_uniform_with_camera(uniform, 1, [3.0, -4.0, 5.0], [0.0; 3], None)
                            .unwrap();
                }
                assert_eq!(uniform.aura_params[15][3], 1.0); // Explicit preview-only active marker.
                uniform.aura_params[15][3] = 0.0;
                for (lane, value) in uniform.aura_params.as_flattened().iter().enumerate() {
                    assert_eq!(
                        u64::from(value.to_bits()),
                        case["output"][lane].as_u64().unwrap(),
                        "case {ordinal} lane {lane}"
                    );
                }
            }
            cases += 1;
        }
        assert_eq!(cases, 2048);
        std::fs::write(folder.join("modelskin-aura-pack-rust-comparison.json"), serde_json::to_string_pretty(&serde_json::json!({"cases":cases,"surfacesPerCase":2,"clientComponentsCompared":cases*2*63,"normalizedReservedComponents":cases*2,"differences":0,"scope":"Complete original 140401490, initialize, Fresnel helper, actual SSE2 CRT, two original affine multiplies per UV, layer controls and copied 256 bytes versus default production WeaponVfxPlayback -> selected surface uniform. Controlled constant curve/PICo outputs, camera TLS, identity incoming model matrix and ready resource allocation; no full constructor/query/provider, texture allocation/registration or GPU. Preview-only final active marker checked separately and normalized to original reserved zero for byte comparison."})).unwrap()+"\n").unwrap();
    }

    #[test]
    fn model_skin_combined_targets_keep_available_surfaces_and_diagnostics() {
        let (mut mounts, mut model) = combined_model_skin_mount();
        let prepare = |mounts: &WeaponVfxAttachments, model: &crate::ModelData| {
            prepare_weapon_vfx_aura_inputs(
                model,
                mounts,
                super::super::PreparedModelOptions::default(),
            )
        };
        let (inputs, diagnostics) = prepare(&mounts, &model);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(
            inputs
                .iter()
                .map(|input| (input.particle_index, input.target_model_path.as_str()))
                .collect::<Vec<_>>(),
            [(0, "main.mdl"), (0, "sub.mdl")]
        );
        assert_eq!(inputs[0].packed, inputs[1].packed);
        assert_eq!(
            (inputs[0].border, inputs[0].filter),
            (inputs[1].border, inputs[1].filter)
        );

        model.meshes[1].indices.clear();
        let (inputs, diagnostics) = prepare(&mounts, &model);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].target_model_path, "main.mdl");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("no compatible Aura surface in sub.mdl"));

        mounts.model_skin_targets.off_hand = None;
        let (inputs, diagnostics) = prepare(&mounts, &model);
        assert_eq!(inputs.len(), 1);
        assert!(diagnostics[0].contains("no loaded Aura target for off hand"));
        let AvfxParticleData::ModelSkin(data) =
            &mut mounts.attachments[0].data.file.particles[0].data
        else {
            unreachable!()
        };
        data.aura_target = 7;
        let (inputs, diagnostics) = prepare(&mounts, &model);
        assert_eq!(inputs.len(), 1);
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("no loaded Aura target for character"))
        );

        mounts.attachments[0].data.textures[0] = None;
        let (inputs, diagnostics) = prepare(&mounts, &model);
        assert!(inputs.is_empty());
        assert_eq!(
            diagnostics
                .iter()
                .filter(|message| message.contains("Aura texture array"))
                .count(),
            1
        );
    }

    #[test]
    fn model_skin_combined_targets_preserve_one_instance_across_both_outputs() {
        let (mounts, model) = combined_model_skin_mount();
        let (inputs, diagnostics) = prepare_weapon_vfx_aura_inputs(
            &model,
            &mounts,
            super::super::PreparedModelOptions::default(),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let mut samples = WeaponVfxPlayback::new_with_resource_indexes(
            &mounts,
            inputs.iter().enumerate().map(|(r, i)| {
                (
                    i.attachment_index,
                    i.particle_index,
                    r,
                    i.target_model_path.as_str(),
                )
            }),
            None,
            None,
            |_| Some([0.0; 3]),
        );
        assert_eq!(samples.attachments[0].playback.fallback_reason(), None);
        samples.advance_to(0.0);
        let initial = samples.aura_instances.clone();
        assert_eq!(initial.len(), 2);
        assert_eq!(
            (initial[0].resource_index, initial[1].resource_index),
            (0, 1)
        );
        assert_eq!(initial[0].instance, initial[1].instance);
        assert_eq!(samples.aura_observed_instances.len(), 2);
        assert_eq!(
            model_skin_aura_uniform(&initial[0].instance).aura_params,
            model_skin_aura_uniform(&initial[1].instance).aura_params
        );
        samples.advance_to(0.0);
        assert_eq!(samples.aura_instances, initial);

        samples.advance_to(0.125);
        assert_eq!(samples.aura_instances.len(), 2);
        assert_eq!(
            samples.aura_instances[0].instance,
            samples.aura_instances[1].instance
        );
        assert_eq!(
            samples.aura_instances[0].instance.instance_id,
            initial[0].instance.instance_id
        );
        assert!(samples.aura_instances[0].instance.aura_sem > initial[0].instance.aura_sem);
        let mut slots = Vec::new();
        advance_aura_slots(
            &mut slots,
            samples.aura_observed_instances.iter().map(|active| {
                let input = &inputs[active.resource_index];
                AuraCandidate {
                    target_model_path: &input.target_model_path,
                    priority: input.priority,
                    attachment_index: input.attachment_index,
                    creation_order: active.instance.aura_creation_order,
                    binding: AuraBinding {
                        resource_index: active.resource_index,
                        instance_id: active.instance.instance_id,
                    },
                }
            }),
        );
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].binding.unwrap().resource_index, 0);
        assert_eq!(slots[1].binding.unwrap().resource_index, 1);

        samples.advance_to(0.5);
        assert!(samples.aura_instances.is_empty());
        assert_eq!(
            samples.aura_observed_instances.len(),
            2,
            "both targets retain observed parameters"
        );
        samples.advance_to(0.0);
        assert_eq!(
            samples.aura_instances, initial,
            "seeking rebuilds both target outputs"
        );
    }

    #[test]
    fn preview_shared_modelskin_random_stream_gates_failed_queries_and_replays_both_mounts() {
        let (mut mounts, model) = combined_model_skin_mount();
        mounts.attachments.push(mounts.attachments[0].clone());
        mounts.attachments[1].model_path = "sub.mdl".into();
        for mount in &mut mounts.attachments {
            mount.data.file.particles[0].life.value = 100.0;
            let AvfxParticleData::ModelSkin(data) = &mut mount.data.file.particles[0].data else {
                unreachable!()
            };
            data.sem_random = constant(0.75);
            data.sem_random.random_type = 4;
        }
        let (inputs, diagnostics) = prepare_weapon_vfx_aura_inputs(
            &model,
            &mounts,
            super::super::PreparedModelOptions::default(),
        );
        assert!(diagnostics.is_empty());
        assert_eq!(inputs.len(), 4);
        let mut samples = WeaponVfxPlayback::new_with_resource_indexes(
            &mounts,
            inputs.iter().enumerate().map(|(r, i)| {
                (
                    i.attachment_index,
                    i.particle_index,
                    r,
                    i.target_model_path.as_str(),
                )
            }),
            None,
            None,
            |_| Some([0.0; 3]),
        );
        samples.advance_to(0.0);
        assert!(
            samples
                .attachments
                .iter()
                .all(|a| a.playback.fallback_reason().is_none())
        );
        let initial = samples.aura_instances.clone();
        assert_eq!(initial.len(), 4);
        assert_eq!(initial[0].instance, initial[1].instance);
        assert_eq!(initial[2].instance, initial[3].instance);
        assert_ne!(initial[0].instance.aura_sem, initial[2].instance.aura_sem);
        let initial_state = samples.random_stream.snapshot();
        samples.advance_to(0.0);
        assert_eq!(samples.aura_instances, initial);
        assert_eq!(samples.random_stream.snapshot(), initial_state);
        samples.advance_to(0.125);
        let latest = samples.aura_instances.clone();
        let before_missing = samples.random_stream.snapshot();
        samples.refresh_model_skin_targets(|_| false);
        samples.advance_to(0.25);
        assert_eq!(samples.aura_instances, latest);
        assert_eq!(
            samples.random_stream.snapshot(),
            before_missing,
            "failed query skips both SEM Always draws"
        );
        samples.refresh_model_skin_targets(|_| true);
        samples.advance_to(0.25);
        let mut expected = before_missing;
        expected.next_u16();
        expected.next_u16();
        assert_eq!(
            samples.random_stream.snapshot(),
            expected,
            "paused recovery draws once per document"
        );
        assert_eq!(samples.aura_instances.len(), 4);
        assert_ne!(
            samples.aura_instances[0].instance.aura_sem,
            latest[0].instance.aura_sem
        );
        samples.advance_to(0.0);
        assert_eq!(
            samples.aura_instances, initial,
            "group replay preserves constructor/draw order"
        );
        assert_eq!(samples.random_stream.snapshot(), initial_state);
    }

    fn ordered_model_skin_mounts() -> (WeaponVfxAttachments, Vec<WeaponVfxAuraInput>) {
        let (mut mounts, model) = combined_model_skin_mount();
        mounts.attachments.push(mounts.attachments[0].clone());
        mounts.attachments[1].model_path = "sub.mdl".into();
        let (inputs, diagnostics) = prepare_weapon_vfx_aura_inputs(
            &model,
            &mounts,
            super::super::PreparedModelOptions::default(),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(inputs.len(), 4);
        (mounts, inputs)
    }

    #[test]
    fn aura_group_selection_uses_constructor_order_and_retains_both_targets_after_retirement() {
        let (mut mounts, inputs) = ordered_model_skin_mounts();
        for mount in &mut mounts.attachments {
            mount.data.file.particles[0].life.value = 1.0;
        }
        let mut playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
        let selected = |bindings: &WeaponVfxAuraBindings| {
            let mut selected = bindings.selected_instances();
            selected.sort_by_key(|i| i.resource_index);
            selected
        };
        playback.advance_to(0.0);
        assert!(playback.fallback_reasons().is_empty());
        let initial_state = playback.random_state();
        let creation_state = playback.model_skin_creation_order.snapshot();
        assert_eq!(creation_state, 2);
        // Output enumeration is deliberately reversed; it must not become the
        // resource creation key. Both target routes share the same constructor.
        playback.aura_observed_instances.reverse();
        bindings.update(0.0, &playback);
        let initial = selected(&bindings);
        assert_eq!(initial.len(), 2);
        assert!(
            initial
                .iter()
                .all(|i| inputs[i.resource_index].attachment_index == 0)
        );
        assert!(
            initial
                .iter()
                .all(|i| i.instance.aura_creation_order == Some(0))
        );
        bindings.update(0.0, &playback);
        assert_eq!(selected(&bindings), initial);
        assert_eq!(playback.random_state(), initial_state);
        assert_eq!(
            playback.model_skin_creation_order.snapshot(),
            creation_state
        );
        playback.advance_to(0.125);
        assert!(playback.aura_instances().is_empty());
        bindings.update(0.125, &playback);
        assert_eq!(
            selected(&bindings),
            initial,
            "target retains dead source's full successful packet"
        );
        playback.advance_to(0.0);
        bindings.update(0.0, &playback);
        assert_eq!(selected(&bindings), initial);
        assert_eq!(
            playback.model_skin_creation_order.snapshot(),
            creation_state
        );
    }

    #[test]
    fn aura_group_late_first_sample_selects_later_mount_that_was_created_earlier() {
        let (mut mounts, inputs) = ordered_model_skin_mounts();
        mounts.attachments[0].data.file.schedulers[0].items[0].start_time = 1;
        for mount in &mut mounts.attachments {
            mount.data.file.particles[0].life.value = 100.0;
        }
        let mut playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
        playback.advance_to(0.125);
        assert!(
            playback.fallback_reasons().is_empty(),
            "{:?}",
            playback.fallback_reasons()
        );
        assert_eq!(playback.aura_instances().len(), 4);
        bindings.update(0.125, &playback);
        let selected = bindings.selected_instances();
        assert_eq!(selected.len(), 2);
        assert!(
            selected
                .iter()
                .all(|i| inputs[i.resource_index].attachment_index == 1),
            "mount index is not construction order"
        );
        assert!(
            selected
                .iter()
                .all(|i| i.instance.aura_creation_order == Some(0))
        );
        assert!(
            playback
                .aura_instances()
                .iter()
                .filter(|i| inputs[i.resource_index].attachment_index == 0)
                .all(|i| i.instance.aura_creation_order == Some(1))
        );
        let before = selected;
        playback.advance_to(0.25);
        bindings.update(0.25, &playback);
        assert_eq!(
            bindings
                .selected_instances()
                .iter()
                .map(|i| i.resource_index)
                .collect::<Vec<_>>(),
            before.iter().map(|i| i.resource_index).collect::<Vec<_>>()
        );
    }

    #[test]
    fn aura_group_older_resource_recovery_replaces_later_binding_without_new_constructor() {
        let (mut mounts, inputs) = ordered_model_skin_mounts();
        for mount in &mut mounts.attachments {
            mount.data.file.particles[0].life.value = 100.0;
        }
        let mut playback =
            WeaponVfxPlayback::from_prepared_aura_inputs(&mounts, &inputs, None, None, |_| {
                Some([0.0; 3])
            });
        let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(&inputs);
        let first = &mut playback.attachments[0].playback;
        assert!(first.observed_model_skin_instances().is_empty());
        first
            .set_model_skin_target_input(VfxModelSkinTargetInput::default())
            .unwrap();
        playback.advance_to(0.0);
        bindings.update(0.0, &playback);
        let later = bindings.selected_instances();
        assert_eq!(later.len(), 2);
        assert!(
            later
                .iter()
                .all(|i| inputs[i.resource_index].attachment_index == 1)
        );
        let before = playback.model_skin_creation_order.snapshot();
        assert_eq!(before, 2);
        playback.refresh_model_skin_targets(|_| true);
        playback.advance_to(0.0); // paused recovery, older resource now registers
        bindings.update(0.0, &playback);
        let older = bindings.selected_instances();
        assert_eq!(older.len(), 2);
        assert!(
            older
                .iter()
                .all(|i| inputs[i.resource_index].attachment_index == 0)
        );
        assert_eq!(playback.model_skin_creation_order.snapshot(), before);
        playback.refresh_model_skin_targets(|_| false);
        playback.advance_to(0.125);
        bindings.update(0.125, &playback);
        assert_eq!(bindings.selected_instances(), older);
        let random = playback.random_state();
        playback.advance_to(f32::NAN);
        bindings.update(f32::NAN, &playback);
        assert!(bindings.selected_instances().is_empty());
        assert_eq!(playback.random_state(), random);
        assert_eq!(playback.model_skin_creation_order.snapshot(), before);
    }

    #[test]
    fn model_skin_host_surface_loss_retains_both_uniforms_and_recovers_at_pause() {
        let (mounts, model) = combined_model_skin_mount();
        let (inputs, diagnostics) = prepare_weapon_vfx_aura_inputs(
            &model,
            &mounts,
            super::super::PreparedModelOptions::default(),
        );
        assert!(diagnostics.is_empty());
        let mut samples = WeaponVfxPlayback::new_with_resource_indexes(
            &mounts,
            inputs.iter().enumerate().map(|(r, i)| {
                (
                    i.attachment_index,
                    i.particle_index,
                    r,
                    i.target_model_path.as_str(),
                )
            }),
            None,
            None,
            |_| Some([0.0; 3]),
        );
        // Use the production refresh hook consumed by WeaponVfxParticles::update.
        samples.refresh_model_skin_targets(|_| false);
        samples.advance_to(0.0);
        assert!(samples.aura_instances.is_empty());
        assert!(samples.aura_observed_instances.is_empty());
        samples.refresh_model_skin_targets(|_| true);
        samples.advance_to(0.0);
        let first = samples.aura_instances.clone();
        assert_eq!(first.len(), 2);
        let uniforms = first
            .iter()
            .map(|v| model_skin_aura_uniform(&v.instance).aura_params)
            .collect::<Vec<_>>();
        let first_update = samples.attachments[0].playback.model_skin_instances();
        samples.refresh_model_skin_targets(|_| false);
        samples.advance_to(0.125);
        assert_eq!(samples.aura_instances, first);
        assert_eq!(samples.aura_observed_instances, first);
        assert_eq!(
            samples
                .aura_instances
                .iter()
                .map(|v| model_skin_aura_uniform(&v.instance).aura_params)
                .collect::<Vec<_>>(),
            uniforms
        );
        // One surviving hand is enough for the shared instance's successful Aura.
        samples.refresh_model_skin_targets(|path| path == "sub.mdl");
        samples.advance_to(0.125);
        assert_eq!(samples.aura_instances.len(), 2);
        assert_eq!(
            samples.aura_instances[0].instance,
            samples.aura_instances[1].instance
        );
        assert_eq!(
            samples.aura_instances[0].instance.instance_id,
            first_update[0].instance_id
        );
        assert!(samples.aura_instances[0].instance.aura_sem > first_update[0].aura_sem);
        let restored = samples.aura_instances.clone();
        samples.refresh_model_skin_targets(|path| path == "sub.mdl");
        samples.advance_to(0.125);
        assert_eq!(samples.aura_instances, restored);
        samples.advance_to(0.0);
        let mut only_off_hand = first[1];
        only_off_hand.instance.aura_registered_target_slots = Some(4);
        assert_eq!(
            samples.aura_instances,
            vec![only_off_hand],
            "seek rebuilds and registers only the current surviving target"
        );
        samples.refresh_model_skin_targets(|_| true);
        samples.advance_to(0.0);
        assert_eq!(
            samples.aura_instances, first,
            "newly available targets register on the paused update"
        );
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn mounted_documents_sort_by_sko_in_final_pixels() {
        use crate::test_support::{
            WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
        };

        let make_mount = |rgba: [u8; 4], soft_key_offset: f32| {
            let mut mount = attachment("main.mdl", [0.5, 0.5, 0.0]);
            let file = &mut mount.data.file;
            file.global.draw_layer = 2;
            file.global.soft_key_offset = soft_key_offset;
            file.particles.truncate(1);
            file.emitters[0].particle_items.truncate(1);
            file.texture_paths = vec!["solid.atex".into()];
            let quad = &mut file.particles[0];
            let tc1 = quad.texture_color1.as_mut().unwrap();
            tc1.texture_index = 0;
            tc1.texture_list = vec![0];
            tc1.calculate_color = 1;
            tc1.calculate_alpha = 1;
            quad.texture_color2 = None;
            quad.texture_color3 = None;
            quad.texture_distortion = None;
            quad.texture_palette = None;
            quad.depth_test = false;
            quad.depth_write = false;
            mount.data.textures = vec![Some(VfxTextureRgba {
                width: 1,
                height: 1,
                rgba: rgba.to_vec(),
                source_mip_count: 1,
                ..Default::default()
            })];
            mount
        };
        let mut mesh = super::super::tests::test_mesh("document-receiver", 0.0);
        mesh.path = "main.mdl#receiver".into();
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![super::super::material::fallback_material()],
            textures: Vec::new(),
            meshes: vec![mesh],
        };
        for msaa_samples in [1, 4] {
            let render = |red_sko, green_sko, label| {
                let mounts = WeaponVfxAttachments {
                    attachments: vec![
                        make_mount([255, 0, 0, 255], red_sko),
                        make_mount([0, 255, 0, 255], green_sko),
                    ],
                    ..Default::default()
                };
                let options = WeaponModelSnapshotOptions::new(format!(
                    "vfx-document-sko-{label}-{msaa_samples}x"
                ))
                .with_viewport(64, 64)
                .with_camera(0.0, 0.0, 3.0, [0.0; 2])
                .with_render_options(super::super::ModelRenderOptions {
                    msaa_samples,
                    ..Default::default()
                })
                .with_weapon_vfx(mounts, 0.0)
                .with_hdr_scene_capture();
                render_weapon_model_snapshot_with_options(options, &model)
                    .expect("render mounted documents")
                    .hdr_scene_rgba
                    .expect("capture HDR scene")[32 * 64 + 32]
            };
            let red_last = render(1.0, -1.0, "red-last");
            let green_last = render(-1.0, 1.0, "green-last");
            assert!(
                red_last[0] > red_last[1] + 0.1,
                "MSAA={msaa_samples}: {red_last:?}"
            );
            assert!(
                green_last[1] > green_last[0] + 0.1,
                "MSAA={msaa_samples}: {green_last:?}"
            );
        }
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn weapon_mount_prepares_distinct_model_skin_gpu_resources() {
        crate::test_support::require_gpu_test_opt_in().expect("explicit native GPU test opt-in");
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("native adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                    ..Default::default()
                })
                .await
                .expect("native device");
            let context =
                ModelRenderContext::new(device, queue, wgpu::TextureFormat::Rgba8UnormSrgb);

            let mut attachment = point_mount().attachments.remove(0);
            attachment.data.avfx_path = "double-aura.avfx".into();
            attachment.data.textures = vec![Some(VfxTextureRgba {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
                source_mip_count: 1,
                ..Default::default()
            })];
            attachment.data.file.texture_paths = vec!["aura.atex".into()];
            attachment.data.file.particles = (0..2)
                .map(|index| AvfxParticle {
                    particle_type: Some(ParticleType::ModelSkin),
                    collision_type: -1,
                    data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                        aura_target: 2,
                        fresnel_type: index + 1,
                        fresnel_curve: constant(2.5),
                        ..Default::default()
                    }),
                    texture_color2: Some(AvfxParticleTexture {
                        enabled: true,
                        texture_index: 0,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect();
            let mut second_item = attachment.data.file.emitters[0].particle_items[0];
            second_item.target_index = 1;
            attachment.data.file.emitters[0]
                .particle_items
                .push(second_item);
            let mounts = WeaponVfxAttachments {
                attachments: vec![attachment],
                model_skin_targets: WeaponVfxModelTargets {
                    weapon: Some("main.mdl".into()),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut material = super::super::material::fallback_material();
            material.shader_package_name = Some("character.shpk".into());
            let mut mesh = super::super::tests::test_mesh("aura", 0.0);
            mesh.path = "main.mdl#mesh0".into();
            let mut model_data = crate::ModelData {
                bounds: crate::ModelBounds::default(),
                materials: vec![material],
                textures: Vec::new(),
                meshes: vec![mesh],
            };
            let mut model = context.create_model(&model_data, Default::default());
            let mut particles = context.create_weapon_vfx_particles(&model, &mounts);
            assert!(particles.aura_diagnostics().is_empty());
            assert_eq!(particles.aura_resources().len(), 2);
            assert_eq!(particles.aura_resources()[0].particle_index, 0);
            assert_eq!(particles.aura_resources()[1].particle_index, 1);
            assert!(
                particles
                    .aura_resources()
                    .iter()
                    .all(|resource| resource.target_model_path == "main.mdl"
                        && resource.texture.slot_layers == [Some(0), None, None])
            );
            particles.advance_to(0.0);
            let first = particles.active_aura_instances().to_vec();
            assert_eq!(first.len(), 2);
            assert_eq!(
                particles
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            assert_eq!(first[0].resource_index, 0);
            assert_eq!(first[1].resource_index, 1);
            assert_eq!(first[0].instance.particle_index, 0);
            assert_eq!(first[1].instance.particle_index, 1);
            particles.update(&context, &model, 0.0);
            assert_eq!(
                particles.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                1.0
            );
            assert_eq!(
                particles.aura_resources()[1]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                0.0
            );
            particles.advance_to(0.0);
            assert_eq!(particles.active_aura_instances(), first);
            particles.advance_to(0.125);
            assert!(particles.active_aura_instances().len() > first.len());
            assert!(first.iter().all(|instance| {
                particles.active_aura_instances().iter().any(|active| {
                    active.resource_index == instance.resource_index
                        && active.instance.instance_id == instance.instance.instance_id
                })
            }));

            let mut single_mount = mounts.clone();
            single_mount.attachments[0].data.file.particles.pop();
            single_mount.attachments[0].data.file.emitters[0]
                .particle_items
                .pop();
            let mut combined_mount = single_mount.clone();
            combined_mount.model_skin_targets.off_hand = Some("sub.mdl".into());
            combined_mount.attachments[0].data.file.emitters[0].create_interval = constant(1000.0);
            let AvfxParticleData::ModelSkin(data) =
                &mut combined_mount.attachments[0].data.file.particles[0].data
            else {
                unreachable!()
            };
            data.aura_target = 6;
            data.sem = AvfxCurve {
                keys: vec![
                    AvfxCurveKey {
                        time: 0,
                        z: 1.0,
                        interpolation: 1,
                        x: 0.0,
                        y: 0.0,
                    },
                    AvfxCurveKey {
                        time: 30,
                        z: 4.0,
                        interpolation: 1,
                        x: 0.0,
                        y: 0.0,
                    },
                ],
                ..Default::default()
            };
            let mut combined_model_data = model_data.clone();
            let mut off_hand = combined_model_data.meshes[0].clone();
            off_hand.path = "sub.mdl#mesh0".into();
            combined_model_data.meshes.push(off_hand);
            let mut combined_model = context.create_model(&combined_model_data, Default::default());
            let mut combined =
                context.create_weapon_vfx_particles(&combined_model, &combined_mount);
            assert!(combined.aura_diagnostics().is_empty());
            assert_eq!(combined.aura_resources().len(), 2);
            combined.update(&context, &combined_model, 0.0);
            assert_eq!(combined.active_aura_instances().len(), 2);
            assert_eq!(
                combined.active_aura_instances()[0].instance,
                combined.active_aura_instances()[1].instance
            );
            assert_eq!(combined.unambiguous_active_aura_resources().len(), 2);
            assert_eq!(
                combined.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params,
                combined.aura_resources()[1]
                    .current_uniform
                    .get()
                    .aura_params
            );
            assert_eq!(
                combined.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                1.0
            );

            // Exercise actual ModelInstance compatibility refresh, not only
            // injected CPU host inputs. Resources and instance IDs stay alive.
            let before_loss = combined.active_aura_instances().to_vec();
            let before_uniform = combined.aura_resources()[0].current_uniform.get();
            combined_model_data.materials[0].shader_package_name = Some("hair.shpk".into());
            combined_model.update_materials(&context, &combined_model_data);
            combined.update(&context, &combined_model, 0.125);
            assert_eq!(combined.active_aura_instances(), before_loss);
            for resource in combined.aura_resources() {
                assert_eq!(
                    resource.current_uniform.get().aura_params,
                    before_uniform.aura_params
                );
            }
            combined_model_data.materials[0].shader_package_name = Some("character.shpk".into());
            combined_model.update_materials(&context, &combined_model_data);
            combined.update(&context, &combined_model, 0.125);
            assert_eq!(combined.active_aura_instances().len(), 2);
            assert_eq!(
                combined.active_aura_instances()[0].instance.instance_id,
                before_loss[0].instance.instance_id
            );
            assert!(
                combined.active_aura_instances()[0].instance.aura_sem
                    > before_loss[0].instance.aura_sem
            );
            assert_eq!(combined.unambiguous_active_aura_resources().len(), 2);
            assert_eq!(
                combined.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params,
                combined.aura_resources()[1]
                    .current_uniform
                    .get()
                    .aura_params
            );

            let mut single = context.create_weapon_vfx_particles(&model, &single_mount);
            single.advance_to(0.0);
            assert_eq!(single.active_aura_instances().len(), 1);
            single.update(&context, &model, 0.0);
            assert_eq!(
                single
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            let aura = single.unambiguous_active_aura_resource().unwrap();
            assert_eq!(
                aura.current_uniform.get().aura_params[5],
                [1.0, 0.0, 0.0, 0.0]
            );
            assert_eq!(aura.current_uniform.get().aura_params[14][3].to_bits(), 1);
            assert_eq!(aura.current_uniform.get().aura_params[15][1], 1.0);
            assert_eq!(
                aura.current_uniform.get().aura_params[2],
                [0.0, 0.0, 0.0, 2.5]
            );
            assert_eq!(
                aura.uniform_with_camera([1.0, 2.0, 3.0], [0.0, 0.0, 1.0])
                    .unwrap()
                    .aura_params[2],
                [1.0, 2.0, 3.0, 2.5]
            );
            assert_eq!(
                aura.uniform_with_camera([-4.0, 5.0, 6.0], [0.0, 0.0, 1.0])
                    .unwrap()
                    .aura_params[2],
                [-4.0, 5.0, 6.0, 2.5]
            );

            let mut camera_mount = single_mount.clone();
            let file = &mut camera_mount.attachments[0].data.file;
            file.emitters[0].emitter_type = Some(EmitterType::Cone);
            file.emitters[0].data = Some(AvfxEmitterData::Cone(ConeEmitterData {
                inner_size: constant(1.0),
                outer_size: constant(1.0),
                injection_speed: constant(0.0),
                injection_angle: constant(0.0),
                ..Default::default()
            }));
            file.emitters[0].rotation.y = Some(constant(std::f32::consts::FRAC_PI_2));
            file.emitters[0].create_interval = constant(1.0);
            file.particles[0].life = AvfxLife {
                enabled: true,
                value: 1.0,
                ..Default::default()
            };
            file.particles[0].rotation_direction_base =
                rotation_direction_base::MOVE_DIRECTION_BILLBOARD;
            file.particles[0].rotation.x = Some(constant(std::f32::consts::FRAC_PI_2));
            let AvfxParticleData::ModelSkin(data) = &mut file.particles[0].data else {
                unreachable!()
            };
            data.fresnel_type = 7;
            let mut camera_particles = context.create_weapon_vfx_particles(&model, &camera_mount);
            camera_particles.update(&context, &model, 0.0);
            assert!(camera_particles.aura_diagnostics().is_empty());
            let original = camera_particles.aura_resources()[0]
                .current_camera_facing
                .get()
                .unwrap();
            let original_id = camera_particles.active_aura_instances()[0]
                .instance
                .instance_id;
            let camera_at = |offset: [f32; 3]| -> [f32; 3] {
                std::array::from_fn(|axis| original.position[axis] + offset[axis])
            };
            let resource = &camera_particles.aura_resources()[0];
            let first = resource
                .uniform_with_camera(camera_at([0.0, 0.0, 5.0]), [0.0, 0.0, 1.0])
                .unwrap();
            let second = resource
                .uniform_with_camera(camera_at([0.0, 5.0, 0.0]), [0.0, 0.0, 1.0])
                .unwrap();
            assert_ne!(first.aura_params[2][..3], second.aura_params[2][..3]);
            assert_eq!(first.aura_params[2][3], second.aura_params[2][3]);
            camera_particles.update(&context, &model, 0.0625);
            assert!(
                camera_particles
                    .active_aura_instances()
                    .iter()
                    .any(|active| active.instance.instance_id > original_id)
            );
            assert!(
                camera_particles
                    .active_aura_instances()
                    .iter()
                    .all(|active| active.instance.instance_id != original_id)
            );
            assert_eq!(
                camera_particles.aura_resources()[0]
                    .current_camera_facing
                    .get(),
                Some(original)
            );
            camera_particles.samples.attachments[0].playback =
                VfxPlayback::new(camera_mount.attachments[0].data.runtime());
            camera_particles.update(&context, &model, 0.0625);
            assert_eq!(
                camera_particles.aura_resources()[0]
                    .current_camera_facing
                    .get(),
                Some(original)
            );
            assert_eq!(
                camera_particles.aura_resources()[0]
                    .uniform_with_camera(camera_at([0.0, 0.0, 5.0]), [0.0, 0.0, 1.0])
                    .unwrap()
                    .aura_params[2][..3],
                first.aura_params[2][..3]
            );

            for (mode, first_offset, first_view, second_offset, second_view) in [
                (
                    rotation_direction_base::SCREEN_BILLBOARD,
                    [0.0, 0.0, 5.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0, 5.0],
                    [0.0, 0.6, 0.8],
                ),
                (
                    rotation_direction_base::TREE_BILLBOARD,
                    [0.0, 0.0, 5.0],
                    [0.0, 0.0, 1.0],
                    [5.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                ),
                (
                    rotation_direction_base::BILLBOARD_AXIS_Y,
                    [0.0, 0.0, 5.0],
                    [0.0, 0.0, -1.0],
                    [0.0, 0.0, 5.0],
                    [-1.0, 0.0, 0.0],
                ),
                (
                    rotation_direction_base::CAMERA_BILLBOARD_AXIS_Y,
                    [0.0, 0.0, 5.0],
                    [0.0, 0.0, -1.0],
                    [5.0, 0.0, 0.0],
                    [0.0, 0.0, -1.0],
                ),
                (
                    rotation_direction_base::CAMERA_BILLBOARD,
                    [0.0, 0.0, 5.0],
                    [0.0, 0.6, 0.8],
                    [5.0, 0.0, 0.0],
                    [0.0, 0.6, 0.8],
                ),
            ] {
                let mut axis_mount = camera_mount.clone();
                let particle = &mut axis_mount.attachments[0].data.file.particles[0];
                particle.rotation_direction_base = mode;
                particle.rotation.x = None;
                let mut axis_particles = context.create_weapon_vfx_particles(&model, &axis_mount);
                axis_particles.update(&context, &model, 0.0);
                assert!(axis_particles.aura_diagnostics().is_empty());
                let original = axis_particles.aura_resources()[0]
                    .current_camera_facing
                    .get()
                    .unwrap();
                assert_eq!(original.mode, mode);
                let original_id = axis_particles.active_aura_instances()[0]
                    .instance
                    .instance_id;
                let at = |offset: [f32; 3]| -> [f32; 3] {
                    std::array::from_fn(|axis| original.position[axis] + offset[axis])
                };
                let first = axis_particles.aura_resources()[0]
                    .uniform_with_camera(at(first_offset), first_view)
                    .unwrap();
                let second = axis_particles.aura_resources()[0]
                    .uniform_with_camera(at(second_offset), second_view)
                    .unwrap();
                assert_ne!(first.aura_params[2][..3], second.aura_params[2][..3]);
                assert_eq!(first.aura_params[2][3], second.aura_params[2][3]);

                axis_particles.update(&context, &model, 0.0625);
                assert!(
                    axis_particles
                        .active_aura_instances()
                        .iter()
                        .all(|active| active.instance.instance_id != original_id)
                );
                assert_eq!(
                    axis_particles.aura_resources()[0]
                        .current_camera_facing
                        .get(),
                    Some(original)
                );
                axis_particles.samples.attachments[0].playback =
                    VfxPlayback::new(axis_mount.attachments[0].data.runtime());
                axis_particles.update(&context, &model, 0.0625);
                assert_eq!(
                    axis_particles.aura_resources()[0]
                        .uniform_with_camera(at(first_offset), first_view)
                        .unwrap()
                        .aura_params[2],
                    first.aura_params[2]
                );
            }

            let mut fallback = context.create_weapon_vfx_particles(&model, &single_mount);
            fallback.update(&context, &model, 0.0);
            let bound_uniform = fallback.aura_resources()[0].current_uniform.get();
            assert_eq!(bound_uniform.aura_params[15][3], 1.0);
            fallback.samples.attachments[0].playback =
                VfxPlayback::new(single_mount.attachments[0].data.runtime());
            assert!(
                fallback.samples.attachments[0]
                    .playback
                    .fallback_reason()
                    .is_some()
            );
            fallback.aura_resources()[0]
                .current_uniform
                .set(SurfaceOverlayUniform {
                    aura_params: [[0.0; 4]; 16],
                });
            fallback.update(&context, &model, 0.0);
            assert!(fallback.active_aura_instances().is_empty());
            assert_eq!(
                fallback
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            assert_eq!(
                fallback.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params,
                bound_uniform.aura_params
            );

            let mut budget_mount = single_mount.clone();
            budget_mount.attachments[0].data.file.emitters[0].loop_end = 1;
            let mut budget = context.create_weapon_vfx_particles(&model, &budget_mount);
            budget.update(&context, &model, 0.0);
            budget.samples.attachments[0]
                .playback
                .advance(100_000.0)
                .unwrap();
            assert!(
                budget.samples.attachments[0]
                    .playback
                    .fallback_reason()
                    .is_some()
            );
            budget.advance_to(100_000.0);
            assert!(budget.active_aura_instances().is_empty());
            assert_eq!(
                budget
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );

            let mut never_bound = context.create_weapon_vfx_particles(&model, &single_mount);
            never_bound.samples.attachments[0].playback =
                VfxPlayback::new(single_mount.attachments[0].data.runtime());
            never_bound.update(&context, &model, 0.0);
            assert!(never_bound.unambiguous_active_aura_resource().is_none());

            let mut priority_mounts = single_mount.clone();
            priority_mounts.attachments[0].data.file.global.a_pri = 1;
            let mut higher = priority_mounts.attachments[0].clone();
            higher.data.file.global.a_pri = 0x102;
            priority_mounts.attachments.push(higher);
            let mut priority = context.create_weapon_vfx_particles(&model, &priority_mounts);
            priority.advance_to(0.0);
            assert_eq!(priority.active_aura_instances().len(), 2);
            assert_eq!(priority.aura_resources()[1].priority, 2);
            assert_eq!(
                priority
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.attachment_index),
                Some(1)
            );
            priority.update(&context, &model, 0.0);
            assert_eq!(
                priority.aura_resources()[1]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                1.0
            );

            let mut tied_mounts = priority_mounts.clone();
            tied_mounts.attachments[1].data.file.global.a_pri = 1;
            let mut tied = context.create_weapon_vfx_particles(&model, &tied_mounts);
            tied.advance_to(0.0);
            assert_eq!(tied.active_aura_instances().len(), 2);
            assert_eq!(
                tied.unambiguous_active_aura_resource()
                    .map(|resource| resource.attachment_index),
                Some(0)
            );

            let mut retained_mount = single_mount.clone();
            retained_mount.attachments[0].data.file.emitters[0].create_interval = constant(1000.0);
            retained_mount.attachments[0].data.file.particles[0].life = AvfxLife {
                enabled: true,
                value: 1.0,
                ..Default::default()
            };
            let mut retained = context.create_weapon_vfx_particles(&model, &retained_mount);
            retained.advance_to(0.0);
            assert_eq!(retained.active_aura_instances().len(), 1);
            retained.advance_to(0.125);
            assert!(retained.active_aura_instances().is_empty());
            retained.update(&context, &model, 0.125);
            assert_eq!(
                retained
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            assert_eq!(
                retained.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                1.0
            );

            let mut late = context.create_weapon_vfx_particles(&model, &retained_mount);
            late.update(&context, &model, 0.125);
            assert!(late.active_aura_instances().is_empty());
            assert_eq!(
                late.unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            assert_eq!(
                late.aura_resources()[0].current_uniform.get().aura_params[15][3],
                1.0
            );

            let mut late_tied_mounts = mounts.clone();
            late_tied_mounts.attachments[0].data.file.emitters[0].create_interval =
                constant(1000.0);
            late_tied_mounts.attachments[0].data.file.particles[0].life = AvfxLife {
                enabled: true,
                value: 1.0,
                ..Default::default()
            };
            let mut late_tied = context.create_weapon_vfx_particles(&model, &late_tied_mounts);
            late_tied.update(&context, &model, 0.125);
            assert_eq!(late_tied.active_aura_instances().len(), 1);
            assert_eq!(late_tied.active_aura_instances()[0].resource_index, 1);
            assert_eq!(
                late_tied
                    .unambiguous_active_aura_resource()
                    .map(|resource| resource.particle_index),
                Some(0)
            );
            assert_eq!(
                late_tied.aura_resources()[0]
                    .current_uniform
                    .get()
                    .aura_params[15][3],
                1.0
            );

            let mut split_mounts = mounts.clone();
            split_mounts.model_skin_targets.off_hand = Some("sub.mdl".into());
            if let AvfxParticleData::ModelSkin(data) =
                &mut split_mounts.attachments[0].data.file.particles[1].data
            {
                data.aura_target = 4;
            }
            let mut sub_mesh = super::super::tests::test_mesh("offhand-aura", 0.0);
            sub_mesh.path = "sub.mdl#mesh0".into();
            model_data.meshes.push(sub_mesh);
            let split_model = context.create_model(&model_data, Default::default());
            let mut split = context.create_weapon_vfx_particles(&split_model, &split_mounts);
            split.advance_to(0.0);
            assert_eq!(split.active_aura_instances().len(), 2);
            assert!(split.unambiguous_active_aura_resource().is_none());
            assert_eq!(split.unambiguous_active_aura_resources().len(), 2);
            split.update(&context, &split_model, 0.0);
            assert_eq!(
                split.aura_resources()[0].current_uniform.get().aura_params[3][0],
                1.0
            );
            assert_eq!(
                split.aura_resources()[1].current_uniform.get().aura_params[3][0],
                0.0
            );

            for incompatible in ["other-model", "other-shader", "face-decal"] {
                model_data.meshes[0].path = if incompatible == "other-model" {
                    "other.mdl#mesh0".into()
                } else {
                    "main.mdl#mesh0".into()
                };
                model_data.materials[0].shader_package_name = Some(
                    if incompatible == "other-shader" {
                        "hair.shpk"
                    } else {
                        "character.shpk"
                    }
                    .into(),
                );
                model_data.materials[0].character_colors = (incompatible == "face-decal")
                    .then_some(crate::ModelMaterialCharacterColors {
                        colors: Default::default(),
                        decal_texture: Some(0),
                    });
                let model = context.create_model(&model_data, Default::default());
                let mut rejected = context.create_weapon_vfx_particles(&model, &mounts);
                assert!(rejected.aura_resources().is_empty(), "{incompatible}");
                assert_eq!(rejected.aura_diagnostics().len(), 2, "{incompatible}");
                assert!(
                    rejected
                        .aura_diagnostics()
                        .iter()
                        .all(|message| message.contains("no compatible Aura surface"))
                );
                rejected.advance_to(0.0);
                assert!(rejected.active_aura_instances().is_empty());
                assert!(
                    rejected.samples.attachments[0]
                        .playback
                        .fallback_reason()
                        .is_some()
                );
            }

            model_data.meshes[0].path = "main.mdl#mesh0".into();
            model_data.materials[0].character_colors = None;
            model_data.materials[0].shader_package_name = Some("hair.shpk".into());
            model.update_materials(&context, &model_data);
            assert!(!model.accepts_aura_target("main.mdl"));
            model_data.materials[0].shader_package_name = Some("character.shpk".into());
            model.update_materials(&context, &model_data);
            assert!(model.accepts_aura_target("main.mdl"));
        });
    }
}
