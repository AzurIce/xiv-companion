use super::{
    VfxBinderBirthClock, VfxBinderCameraSnapshot, VfxBinderLifecycle, VfxBinderMatrix,
    VfxBinderQueryScale, VfxBinderStep,
};
use crate::avfx::AvfxBinder;

/// Camera matrices from the renderer's actual view computation. The affine
/// inverse is independent of the transposed view basis used by RoTp. Height
/// is the renderer's unsigned pixel height consumed by ATS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCameraViewSnapshot {
    pub inverse_view: VfxBinderMatrix,
    pub camera: VfxBinderCameraSnapshot,
    pub screen_height: u32,
}

impl VfxCameraViewSnapshot {
    pub fn from_view_matrices(
        view: [f32; 16],
        inverse_view: [f32; 16],
        screen_height: u32,
    ) -> Self {
        Self {
            inverse_view: VfxBinderMatrix {
                basis: std::array::from_fn(|column| {
                    std::array::from_fn(|row| inverse_view[column * 4 + row])
                }),
                position: [inverse_view[12], inverse_view[13], inverse_view[14]],
            },
            camera: VfxBinderCameraSnapshot::from_view_matrices(view, inverse_view),
            screen_height,
        }
    }

    pub fn validate(self) -> Result<(), String> {
        self.camera.validate()?;
        if !self
            .inverse_view
            .basis
            .into_iter()
            .flatten()
            .chain(self.inverse_view.position)
            .all(f32::is_finite)
        {
            return Err("VFX Camera inverse-view snapshot must be finite".into());
        }
        Ok(())
    }
}

/// Resolved inputs consumed by Camera Binder's original update 0x1403c0750.
/// The inverse-view affine matrix is TLS camera +0x48, independent of the
/// orientation/parallel/position fields used by RoTp. Do not derive it from
/// the orientation basis: nonorthogonal inputs have different inverse columns.
#[derive(Clone, Copy, Debug)]
pub struct VfxCameraBinderFrame {
    pub age: f32,
    /// Compiled root +0xf4 branch selector: bAGS's DWORD nonzero value.
    /// Runtime snapshots must match the parsed file; this low-level numerical
    /// boundary still accepts an explicit value for original-function probes.
    pub root_flag_f4: bool,
    pub inverse_view: VfxBinderMatrix,
    pub camera: VfxBinderCameraSnapshot,
    pub document_scale: [f32; 3],
    /// Original renderer +0x8c, interpreted as unsigned pixels.
    pub screen_height: u32,
    /// Optional current provider +0x38 -> object +0xc8 -> transform +0x190.
    /// A missing object/transform makes bIFY a no-op, not a query failure.
    pub ify_scale: Option<f32>,
}

/// Camera Binder caches at +8, +0x280, +0x2b0, +0x268 and +0x26c.
/// This is the numerical update state, separate from the owned object factory
/// and the constructor's initial auxiliary-matrix setup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCameraBinderState {
    pub primary: VfxBinderMatrix,
    pub auxiliary: VfxBinderMatrix,
    pub scale: [f32; 3],
    pub query_vfx_scale: f32,
    pub transform_depth_scale: f32,
}

/// Resolved Common storage node, ordered from the Camera's own storage to its
/// ancestors. A Timeline node uses its separate +0x138 scale and terminates
/// inheritance, provided it still has a parent and enables scale inheritance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCameraBinderScaleNode {
    pub inherits_scale: bool,
    pub is_timeline: bool,
    pub scale: [f32; 3],
    pub timeline_scale: [f32; 3],
}

/// Resolved output of one Camera Binder's caster query. Optional writes model
/// failed queries that updated only part of the cached output. Distance may be
/// supplied by a host curve reader, including shared-RNG curves; None uses the
/// file's deterministic Dst reader. This does not infer raw provider semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct VfxCameraBinderSample {
    pub binder_index: usize,
    pub query_succeeded: bool,
    pub query_scale: Option<[f32; 3]>,
    pub query_vfx_scale: Option<f32>,
    pub query_depth_scale: Option<f32>,
    pub distance: Option<f32>,
}

impl VfxCameraBinderSample {
    pub(super) fn apply_query(&self, scale: &mut [f32; 3], vfx: &mut f32, depth: &mut f32) -> bool {
        if let Some(value) = self.query_scale {
            *scale = value;
        }
        if let Some(value) = self.query_vfx_scale {
            *vfx = value;
        }
        if let Some(value) = self.query_depth_scale {
            *depth = value;
        }
        self.query_succeeded
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VfxCameraBinderTimelineSnapshot {
    pub rotation: VfxBinderMatrix,
    pub storage_scale: [f32; 3],
    pub local: VfxCameraBinderScaleNode,
    pub ancestors: Vec<VfxCameraBinderScaleNode>,
}

impl VfxCameraBinderTimelineSnapshot {
    /// Fresh Camera Common storage from 0x1403ae720. Its storage parent is
    /// null even when the separate Common owner parent is present. The host's
    /// vtable +0x40 Euler result must be supplied independently.
    pub fn for_new_object(rotation: VfxBinderMatrix) -> Self {
        Self {
            rotation,
            storage_scale: [1.0; 3],
            local: VfxCameraBinderScaleNode {
                inherits_scale: false,
                is_timeline: false,
                scale: [1.0; 3],
                timeline_scale: [1.0; 3],
            },
            ancestors: Vec::new(),
        }
    }
}

/// Per-input Camera sources. Inverse view is independent of RoTp basis; the
/// compiled +f4 selector and constructor sources are explicit rather than
/// guessed from another Binder flag. Constructor matrices are read only at
/// each object's birth; later updates preserve its auxiliary cache.
#[derive(Clone, Debug, PartialEq)]
pub struct VfxCameraBinderSnapshot {
    pub inverse_view: VfxBinderMatrix,
    pub camera: VfxBinderCameraSnapshot,
    pub root_flag_f4: bool,
    pub screen_height: u32,
    pub ify_scale: Option<f32>,
    /// Item-owner birth rotation from compiled root +a0/a4/a8, independently
    /// of Document scale and the runtime Document's orientation.
    pub document_rotation: VfxBinderMatrix,
    pub root_position: [f32; 3],
    pub timeline: Option<VfxCameraBinderTimelineSnapshot>,
    pub samples: Vec<VfxCameraBinderSample>,
}

impl VfxCameraBinderSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        let matrix = |value: VfxBinderMatrix| {
            value
                .basis
                .into_iter()
                .flatten()
                .chain(value.position)
                .all(f32::is_finite)
        };
        self.camera.validate()?;
        if !matrix(self.inverse_view)
            || !matrix(self.document_rotation)
            || !finite(&self.root_position)
            || self.ify_scale.is_some_and(|value| !value.is_finite())
        {
            return Err("Camera Binder snapshot must be finite".into());
        }
        if let Some(value) = &self.timeline {
            if value.ancestors.len() > 256
                || !matrix(value.rotation)
                || !finite(&value.storage_scale)
                || std::iter::once(&value.local)
                    .chain(&value.ancestors)
                    .any(|node| !finite(&node.scale) || !finite(&node.timeline_scale))
            {
                return Err("invalid Camera Binder Timeline storage snapshot".into());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for sample in &self.samples {
            if !ids.insert(sample.binder_index)
                || sample.query_scale.is_some_and(|v| !finite(&v))
                || [
                    sample.query_vfx_scale,
                    sample.query_depth_scale,
                    sample.distance,
                ]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite())
            {
                return Err("invalid or duplicate Camera Binder sample".into());
            }
        }
        Ok(())
    }
}

/// Sources for the distinct original Camera constructors. Rotation matrices
/// are resolved 37b700 outputs; this boundary does not execute CRT trig, host
/// getters or the Common storage producer. Their translation is discarded.
#[derive(Clone, Copy, Debug)]
pub enum VfxCameraBinderOwner<'a> {
    Emitter {
        document_rotation: VfxBinderMatrix,
    },
    Timeline {
        host_rotation: VfxBinderMatrix,
        /// +0x98 getter output; independent from the update's scale getter.
        storage_scale: [f32; 3],
        local: VfxCameraBinderScaleNode,
        ancestors: &'a [VfxCameraBinderScaleNode],
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCameraBinderConstruction {
    pub state: VfxCameraBinderState,
    pub clock: VfxBinderBirthClock,
}

/// Camera owns a child independently of query success. Allocation/attachment
/// and complete descendant traversal are caller-owned; this object represents
/// its numerical cache and own Common Time/Prepare/Numeric state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCameraBinderInstance {
    pub state: VfxCameraBinderState,
    pub lifecycle: VfxBinderLifecycle,
}

impl VfxCameraBinderConstruction {
    pub fn into_instance(self, registered_children: u16) -> VfxCameraBinderInstance {
        let mut lifecycle = VfxBinderLifecycle::new(self.clock);
        lifecycle.set_registered_children(registered_children);
        VfxCameraBinderInstance {
            state: self.state,
            lifecycle,
        }
    }
}

impl VfxCameraBinderInstance {
    /// Original +0xf8: update at the current own age before Binder's empty-tree
    /// retirement check. Camera has no delayed initializer or query-success
    /// gate. Call advance_time separately, then traverse children as required.
    pub fn prepare_self(
        &mut self,
        binder: &AvfxBinder,
        mut frame: VfxCameraBinderFrame,
        mut distance: impl FnMut(f32, f32) -> f32,
        mut query: impl FnMut(&mut [f32; 3], &mut f32, &mut f32) -> bool,
    ) {
        self.lifecycle.prepare_self(|lifecycle, phase| {
            if phase == VfxBinderStep::Update {
                frame.age = lifecycle.clock.local_age;
                self.state.update_with_distance(
                    binder,
                    frame,
                    || distance(frame.age, lifecycle.clock.total_age),
                    &mut query,
                );
            }
        });
    }
}

impl VfxCameraBinderState {
    /// Shared original query for Camera's fixed caster index -1. Matrix
    /// output is discarded; partial cache writes are retained on failure.
    /// Document multiplication belongs to the subsequent Camera update.
    pub fn query_caster(
        binder: &AvfxBinder,
        camera_position: [f32; 3],
        scale: &mut [f32; 3],
        vfx: &mut f32,
        depth: &mut f32,
        target: impl FnOnce() -> Option<super::VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
    ) -> bool {
        let mut query = super::binder::VfxBinderQueryCache {
            target: VfxBinderMatrix::IDENTITY,
            scale: *scale,
            vfx_scale: *vfx,
            transform_depth_scale: *depth,
        };
        let status = query.refresh(binder, camera_position, true, target, listener_scale);
        *scale = query.scale;
        *vfx = query.vfx_scale;
        *depth = query.transform_depth_scale;
        status == super::VfxBinderQueryStatus::Refreshed
    }

    pub(super) fn is_finite(self) -> bool {
        self.primary
            .basis
            .into_iter()
            .flatten()
            .chain(self.primary.position)
            .chain(self.auxiliary.basis.into_iter().flatten())
            .chain(self.auxiliary.position)
            .chain(self.scale)
            .chain([self.query_vfx_scale, self.transform_depth_scale])
            .all(f32::is_finite)
    }

    /// Both original Camera constructors immediately update at the birth age,
    /// regardless of GenD or query success. Only the numeric/own-clock portion
    /// is represented here; the caller still creates and attaches the child.
    pub fn construct(
        binder: &AvfxBinder,
        frame: VfxCameraBinderFrame,
        distance: f32,
        owner: VfxCameraBinderOwner<'_>,
        root_position: [f32; 3],
        query: impl FnMut(&mut [f32; 3], &mut f32, &mut f32) -> bool,
    ) -> VfxCameraBinderConstruction {
        Self::construct_with_distance(binder, frame, |_, _| distance, owner, root_position, query)
    }

    /// Constructor with a deferred distance reader, preserving the same
    /// Dst-before-query ordering as later Prepare updates.
    pub fn construct_with_distance(
        binder: &AvfxBinder,
        mut frame: VfxCameraBinderFrame,
        distance: impl FnOnce(f32, f32) -> f32,
        owner: VfxCameraBinderOwner<'_>,
        root_position: [f32; 3],
        query: impl FnMut(&mut [f32; 3], &mut f32, &mut f32) -> bool,
    ) -> VfxCameraBinderConstruction {
        let clock = VfxBinderBirthClock::new(frame.age, binder.life, 0.0);
        frame.age = clock.local_age;
        let scale = match owner {
            VfxCameraBinderOwner::Emitter { .. } => frame.document_scale,
            VfxCameraBinderOwner::Timeline { .. } => [1.0; 3],
        };
        let mut state = Self {
            primary: VfxBinderMatrix::IDENTITY,
            auxiliary: VfxBinderMatrix::scale_matrix(scale),
            scale,
            query_vfx_scale: 0.0,
            transform_depth_scale: 1.0,
        };
        state.update_with_distance(
            binder,
            frame,
            || distance(clock.local_age, clock.total_age),
            query,
        );
        if frame.root_flag_f4 {
            state.auxiliary = match owner {
                VfxCameraBinderOwner::Emitter { document_rotation } => {
                    // 37bce0 multiplies all three terms, including zero terms;
                    // it is not the Timeline constructor's direct column scale.
                    document_rotation
                        .transform_matrix(VfxBinderMatrix::scale_matrix(frame.document_scale))
                }
                VfxCameraBinderOwner::Timeline {
                    host_rotation,
                    storage_scale,
                    local,
                    ancestors,
                } => {
                    let mut scale = storage_scale;
                    let mut current = local;
                    for parent in ancestors {
                        if !current.inherits_scale {
                            break;
                        }
                        let factor = if current.is_timeline {
                            current.timeline_scale
                        } else {
                            parent.scale
                        };
                        for (value, factor) in scale.iter_mut().zip(factor) {
                            *value *= factor;
                        }
                        if current.is_timeline {
                            break;
                        }
                        current = *parent;
                    }
                    for (value, factor) in scale.iter_mut().zip(state.scale) {
                        *value *= factor;
                    }
                    VfxBinderMatrix {
                        basis: std::array::from_fn(|axis| {
                            host_rotation.basis[axis].map(|value| value * scale[axis])
                        }),
                        position: [0.0; 3],
                    }
                }
            };
            state.auxiliary.position = root_position;
        }
        VfxCameraBinderConstruction { state, clock }
    }

    /// Execute the Camera numerical update with an already-resolved Dst/DstR
    /// curve output. The original reads that curve once on every update, even
    /// when its target query has expired. The caller owns curve/RNG semantics.
    ///
    /// `query` models phase0/caster(-1). It may modify any cached output before
    /// returning false; failure retains those writes. The target matrix is
    /// discarded. A successful query multiplies the cached scale by Document
    /// scale before the separate bDSE/ATS/IFY updates, in original f32 order.
    pub fn update(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxCameraBinderFrame,
        distance: f32,
        query: impl FnMut(&mut [f32; 3], &mut f32, &mut f32) -> bool,
    ) {
        self.update_with_distance(binder, frame, || distance, query);
    }

    /// Deferred distance reader preserves the original Dst-before-query call
    /// order. The caller still owns curve coefficient/shared RNG semantics.
    pub fn update_with_distance(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxCameraBinderFrame,
        distance: impl FnOnce() -> f32,
        mut query: impl FnMut(&mut [f32; 3], &mut f32, &mut f32) -> bool,
    ) {
        let distance = distance();
        if frame.root_flag_f4 {
            if let Some(start) = &binder.properties_start {
                let deadline = (start.coord_update_frame as i16) as f32;
                if (deadline < 0.0 || deadline >= frame.age)
                    && query(
                        &mut self.scale,
                        &mut self.query_vfx_scale,
                        &mut self.transform_depth_scale,
                    )
                {
                    for (scale, document) in self.scale.iter_mut().zip(frame.document_scale) {
                        *scale *= document;
                    }
                }
            }
        }

        // Distance is applied along inverse-view Z before RoTp overwrites the
        // basis. Translation survives every rotation mode, including mode1.
        self.primary = frame.inverse_view;
        self.primary.position = std::array::from_fn(|axis| {
            distance * self.primary.basis[2][axis] + self.primary.position[axis]
        });
        match binder.rotation_type as i8 {
            4 => {
                let [x, y, z] = self.primary.position;
                let [cx, _, cz] = frame.camera.position;
                let goal = [(x - cx) + x, y + 0.0, (z - cz) + z];
                if let Some(basis) = self.primary.linear_look_at(goal) {
                    self.primary.basis = basis;
                }
            }
            _ => self
                .primary
                .apply_camera_rotation(binder.rotation_type, Some(frame.camera)),
        }

        if !frame.root_flag_f4 {
            self.scale = [1.0; 3];
        }
        if binder.document_scale_enabled {
            for (scale, document) in self.scale.iter_mut().zip(frame.document_scale) {
                *scale *= document;
            }
        }
        if binder.adjust_to_screen_enabled {
            // Original constant 0x14215e694 is 1280, not a guessed 1080p.
            let factor = (frame.screen_height as f32 / 1280.0).sqrt().max(1.0);
            self.scale[0] *= factor;
            self.scale[1] *= factor;
        }
        if binder.ify {
            if let Some(factor) = frame.ify_scale {
                for scale in &mut self.scale {
                    *scale *= factor;
                }
            }
        }
        if !frame.root_flag_f4 {
            self.auxiliary = VfxBinderMatrix::scale_matrix(self.scale);
        }
    }

    /// The shared Binder +0x138 depth-offset getter uses cached query outputs,
    /// independently of Camera's final scale getter and DSE/ATS/IFY modifiers.
    pub fn depth_offset_multiplier(&self, binder: &AvfxBinder) -> f32 {
        VfxBinderQueryScale {
            target_scale: self.scale,
            vfx_scale: self.query_vfx_scale,
        }
        .depth_offset_multiplier(binder, self.transform_depth_scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::AvfxBinderProperties;

    fn state() -> VfxCameraBinderState {
        VfxCameraBinderState {
            primary: VfxBinderMatrix::IDENTITY,
            auxiliary: VfxBinderMatrix::IDENTITY,
            scale: [1.0; 3],
            query_vfx_scale: 1.0,
            transform_depth_scale: 1.0,
        }
    }

    fn frame() -> VfxCameraBinderFrame {
        VfxCameraBinderFrame {
            age: 0.0,
            root_flag_f4: false,
            inverse_view: VfxBinderMatrix {
                position: [1.0, 2.0, 3.0],
                ..VfxBinderMatrix::IDENTITY
            },
            camera: VfxBinderCameraSnapshot {
                basis: [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
                parallel_direction: [0.0, 0.0, 1.0],
                position: [1.0, 9.0, 3.0],
            },
            document_scale: [2.0, 3.0, 4.0],
            screen_height: 1280,
            ify_scale: None,
        }
    }

    fn node(scale: [f32; 3], inherits_scale: bool) -> VfxCameraBinderScaleNode {
        VfxCameraBinderScaleNode {
            inherits_scale,
            is_timeline: false,
            scale,
            timeline_scale: [1.0; 3],
        }
    }

    #[test]
    fn camera_owner_constructors_keep_distinct_failed_query_defaults_and_auxiliary() {
        let binder = AvfxBinder {
            binder_type: 3,
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: -1,
                generate_delay: 32767,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut input = frame();
        input.root_flag_f4 = true;
        input.age = 2.5;
        let emitter = VfxCameraBinderState::construct(
            &binder,
            input,
            2.0,
            VfxCameraBinderOwner::Emitter {
                document_rotation: VfxBinderMatrix::IDENTITY,
            },
            [4.0, 5.0, 6.0],
            |_, _, _| false,
        );
        let timeline = VfxCameraBinderState::construct(
            &binder,
            input,
            2.0,
            VfxCameraBinderOwner::Timeline {
                host_rotation: VfxBinderMatrix::IDENTITY,
                storage_scale: [1.0; 3],
                local: node([1.0; 3], true),
                ancestors: &[],
            },
            [4.0, 5.0, 6.0],
            |_, _, _| false,
        );
        assert_eq!(emitter.clock.delay, 0.0);
        assert_eq!(emitter.clock.local_age, 2.5);
        assert_eq!(emitter.state.scale, input.document_scale);
        assert_eq!(timeline.state.scale, [1.0; 3]);
        assert_ne!(emitter.state.auxiliary, timeline.state.auxiliary);
        assert_eq!(timeline.state.auxiliary.position, [4.0, 5.0, 6.0]);
        assert_eq!(timeline.state.query_vfx_scale, 0.0);
        assert_eq!(timeline.state.transform_depth_scale, 1.0);
        assert_eq!(timeline.into_instance(1).lifecycle.registered_children(), 1);
    }

    #[test]
    fn camera_timeline_scale_chain_stops_on_inheritance_gate_and_terminal_timeline() {
        let binder = AvfxBinder::default();
        let mut input = frame();
        input.root_flag_f4 = true;
        let local = node([2.0; 3], true);
        let timeline = VfxCameraBinderScaleNode {
            is_timeline: true,
            timeline_scale: [3.0; 3],
            ..node([4.0; 3], true)
        };
        for (ancestors, expected) in [
            (vec![], 2.0),
            (vec![timeline], 8.0),
            (vec![timeline, node([100.0; 3], true)], 24.0),
            (vec![node([4.0; 3], false), node([100.0; 3], true)], 8.0),
        ] {
            let birth = VfxCameraBinderState::construct(
                &binder,
                input,
                0.0,
                VfxCameraBinderOwner::Timeline {
                    host_rotation: VfxBinderMatrix::IDENTITY,
                    storage_scale: local.scale,
                    local,
                    ancestors: &ancestors,
                },
                [0.0; 3],
                |_, _, _| panic!("no start properties"),
            );
            assert_eq!(
                birth.state.auxiliary,
                VfxBinderMatrix::scale_matrix([expected; 3])
            );
        }
    }

    #[test]
    fn camera_prepare_preserves_constructor_auxiliary_and_stops_at_life_limit() {
        let binder = AvfxBinder {
            life: 2,
            document_scale_enabled: true,
            ..Default::default()
        };
        let mut input = frame();
        input.root_flag_f4 = true;
        let birth = VfxCameraBinderState::construct(
            &binder,
            input,
            0.0,
            VfxCameraBinderOwner::Emitter {
                document_rotation: VfxBinderMatrix::IDENTITY,
            },
            [4.0, 5.0, 6.0],
            |_, _, _| false,
        );
        let mut instance = birth.into_instance(1);
        instance.lifecycle.advance_time(1.0);
        instance.prepare_self(
            &binder,
            input,
            |age, total| {
                assert_eq!((age, total), (1.0, 1.0));
                2.0
            },
            |_, _, _| false,
        );
        assert_eq!(instance.state.auxiliary, birth.state.auxiliary);
        assert_eq!(instance.state.scale, [8.0, 27.0, 64.0]);
        instance.lifecycle.advance_time(3.0);
        instance.prepare_self(
            &binder,
            input,
            |_, _| panic!("retired owner cannot prepare"),
            |_, _, _| false,
        );
        assert!(instance.lifecycle.retired());
        assert_eq!(instance.lifecycle.clock.local_age, 2.0);
        assert_eq!(instance.lifecycle.clock.total_age, 4.0);
    }

    #[test]
    fn camera_prepare_reads_distance_before_query_and_after_cutoff() {
        let binder = AvfxBinder {
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut input = frame();
        input.root_flag_f4 = true;
        let mut instance = VfxCameraBinderConstruction {
            state: state(),
            clock: VfxBinderBirthClock::new(0.0, -1, 0.0),
        }
        .into_instance(1);
        let events = std::cell::RefCell::new(Vec::new());
        for delta in [0.0, 1.0] {
            instance.lifecycle.advance_time(delta);
            instance.prepare_self(
                &binder,
                input,
                |_, _| {
                    events.borrow_mut().push('D');
                    1.0
                },
                |_, _, _| {
                    events.borrow_mut().push('Q');
                    false
                },
            );
        }
        assert_eq!(*events.borrow(), ['D', 'Q', 'D']);
    }

    #[test]
    #[ignore = "requires pinned original Camera factory/constructor CPU probe"]
    fn compare_original_camera_binder_construction_and_own_phases() {
        use serde_json::{Value, json};
        fn float(value: &Value) -> f32 {
            f32::from_bits(value.as_u64().unwrap() as u32)
        }
        fn vector(value: &Value) -> [f32; 3] {
            std::array::from_fn(|axis| float(&value[axis]))
        }
        fn matrix(value: &Value) -> VfxBinderMatrix {
            VfxBinderMatrix {
                basis: std::array::from_fn(|column| {
                    std::array::from_fn(|row| float(&value[column * 3 + row]))
                }),
                position: std::array::from_fn(|row| float(&value[9 + row])),
            }
        }
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(output.join("camera-binder-construction-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 7200);
        let mut components = 0;
        let mut all_queries = 0;
        for (index, case) in cases.iter().enumerate() {
            let flags = case["flags"].as_u64().unwrap();
            let binder = AvfxBinder {
                binder_type: 3,
                life: case["life"].as_i64().unwrap() as i32,
                rotation_type: case["rotation"].as_i64().unwrap() as i32,
                properties_start: Some(AvfxBinderProperties {
                    coord_update_frame: case["deadline"].as_i64().unwrap() as i32,
                    generate_delay: 32767,
                    ..Default::default()
                }),
                document_scale_enabled: flags & 1 != 0,
                adjust_to_screen_enabled: flags & 2 != 0,
                ify: flags & 4 != 0,
                ..Default::default()
            };
            let inputs = &case["inputs"];
            let chain: Vec<_> = case["scaleChain"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| VfxCameraBinderScaleNode {
                    inherits_scale: value["inherit"].as_bool().unwrap(),
                    is_timeline: value["kind"] == 3,
                    scale: vector(&value["scale"]),
                    timeline_scale: vector(&value["timelineScale"]),
                })
                .collect();
            let frame = VfxCameraBinderFrame {
                age: float(&case["inputAge"]),
                root_flag_f4: case["rootF4"].as_bool().unwrap(),
                inverse_view: matrix(&inputs["inverseView"]),
                camera: VfxBinderCameraSnapshot {
                    basis: std::array::from_fn(|column| {
                        std::array::from_fn(|row| float(&inputs["cameraBasis"][column * 3 + row]))
                    }),
                    parallel_direction: vector(&inputs["parallelDirection"]),
                    position: vector(&inputs["cameraPosition"]),
                },
                document_scale: vector(&inputs["documentScale"]),
                screen_height: case["screenHeight"].as_u64().unwrap() as u32,
                ify_scale: Some(float(&case["ifyScale"])),
            };
            let owner = if case["ownerKind"] == 0 {
                VfxCameraBinderOwner::Emitter {
                    document_rotation: matrix(&inputs["rotationMatrix"]),
                }
            } else {
                VfxCameraBinderOwner::Timeline {
                    host_rotation: matrix(&inputs["rotationMatrix"]),
                    storage_scale: chain[0].scale,
                    local: chain[0],
                    ancestors: &chain[1..],
                }
            };
            let count = std::cell::Cell::new(0u64);
            let events = std::cell::RefCell::new(String::new());
            let curve_reads = std::cell::Cell::new(0u64);
            let mut query = |scale: &mut [f32; 3], vfx: &mut f32, depth: &mut f32| {
                count.set(count.get() + 1);
                events.borrow_mut().push('Q');
                if case["queryMode"].as_u64().unwrap() >= 2 {
                    *scale = [2.0, 0.25, -1.0];
                    *vfx = 0.25;
                    *depth = 2.0;
                }
                case["queryMode"] == 3
            };
            let mut instance = VfxCameraBinderState::construct_with_distance(
                &binder,
                frame,
                |_, _| {
                    curve_reads.set(curve_reads.get() + 1);
                    events.borrow_mut().push('D');
                    float(&case["distance"])
                },
                owner,
                vector(&inputs["rootPosition"]),
                &mut query,
            )
            .into_instance(1);
            for (boundary, native) in std::iter::once(&case["afterFactory"])
                .chain(case["updates"].as_array().unwrap())
                .enumerate()
            {
                if boundary != 0 {
                    instance.lifecycle.advance_time([0.0, 3.0][boundary - 1]);
                    instance.prepare_self(
                        &binder,
                        frame,
                        |_, _| {
                            curve_reads.set(curve_reads.get() + 1);
                            events.borrow_mut().push('D');
                            float(&case["distance"])
                        },
                        &mut query,
                    );
                }
                let state = instance.state;
                let clock = instance.lifecycle.clock;
                for (component, (actual, expected)) in state
                    .primary
                    .basis
                    .into_iter()
                    .flatten()
                    .chain(state.primary.position)
                    .chain(state.auxiliary.basis.into_iter().flatten())
                    .chain(state.auxiliary.position)
                    .chain(state.scale)
                    .chain([
                        state.query_vfx_scale,
                        state.transform_depth_scale,
                        clock.local_age,
                        clock.total_age,
                        clock.previous_age,
                        clock.delay,
                        clock.nominal_life,
                    ])
                    .zip(
                        native["primary"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .chain(native["auxiliary"].as_array().unwrap())
                            .chain(native["scale"].as_array().unwrap())
                            .chain([
                                &native["vfx"],
                                &native["depth"],
                                &native["age"],
                                &native["total"],
                                &native["previous"],
                                &native["delay"],
                                &native["life"],
                            ]),
                    )
                    .enumerate()
                {
                    assert_eq!(
                        actual.to_bits(),
                        expected.as_u64().unwrap() as u32,
                        "case {index} boundary {boundary} component {component}"
                    );
                    components += 1;
                }
                assert_eq!(
                    instance.lifecycle.raw_flags(),
                    native["flags"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    instance.lifecycle.life_limit_enabled,
                    native["lifeEnabled"].as_bool().unwrap()
                );
                assert_eq!(
                    instance.lifecycle.registered_children(),
                    native["children"].as_u64().unwrap() as u16
                );
                assert_eq!(count.get(), native["queryCalls"].as_u64().unwrap());
                assert_eq!(curve_reads.get(), native["curveCalls"].as_u64().unwrap());
                assert_eq!(*events.borrow(), native["events"].as_str().unwrap());
            }
            all_queries += count.get();
            assert_eq!(case["countGetters"], 0);
            assert_eq!(case["rotationReads"], u64::from(frame.root_flag_f4));
            assert_eq!(case["freeCount"], 6);
        }
        std::fs::write(
            output.join("camera-binder-construction-rust-comparison.json"),
            serde_json::to_vec_pretty(&json!({
                "cases": cases.len(), "numericalBoundaries": cases.len() * 3,
                "finiteComponentsCompared": components, "targetQueriesCompared": all_queries,
                "differences": 0, "productionConstructionAndOwnPhasesCompared": true,
                "fullOwnedPlaybackCompared": false,
                "scope": "Original Camera Item/Scheduler factory, Common/Camera constructors and own Time/Prepare vs production constructor/instance core. Primary/auxiliary/scale, query caches, all own clocks, flags, life gate, child count and query/resolved-distance calls compared at birth and two updates. Common storage inputs after real base ctor and Euler outputs controlled; no production factory, physical pool slots, full children/resource phases, live host/shared RNG/CRT trig/GPU proof."
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "requires pinned original Camera Update/shared caster query CPU probe"]
    fn compare_original_camera_caster_query_with_real_cache_writes() {
        use super::super::{VfxBinderObjectSnapshot, VfxBinderTarget};
        use crate::avfx::AvfxBinderProperties;
        use serde_json::{Value, json};
        use std::cell::Cell;
        fn float(value: &Value) -> f32 {
            f32::from_bits(value.as_u64().unwrap() as u32)
        }
        fn vector(value: &Value) -> [f32; 3] {
            std::array::from_fn(|axis| float(&value[axis]))
        }
        fn matrix(value: &Value) -> VfxBinderMatrix {
            VfxBinderMatrix {
                basis: std::array::from_fn(|column| {
                    std::array::from_fn(|row| float(&value[column * 3 + row]))
                }),
                position: std::array::from_fn(|row| float(&value[9 + row])),
            }
        }
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let native: Value = serde_json::from_slice(
            &std::fs::read(output.join("camera-caster-query-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = native["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 9216);
        let mut finite = 0;
        let mut nonfinite = 0;
        let mut all_queries = 0;
        let mut all_listener_calls = 0;
        for (index, case) in cases.iter().enumerate() {
            let sample = case["sample"].as_u64().unwrap();
            let flags = case["flags"].as_u64().unwrap();
            let binder = AvfxBinder {
                binder_type: 3,
                rotation_type: if sample & 1 != 0 { 4 } else { 0 },
                following_target_orientation: flags & 1 != 0,
                transform_scale: if flags & 2 != 0 { 255 } else { 0 },
                vfx_scale_enabled: flags & 4 != 0,
                vfx_scale_depth_offset: flags & 8 != 0,
                bet: flags & 16 != 0,
                transform_scale_depth_offset: flags & 32 != 0,
                document_scale_enabled: flags & 64 != 0,
                adjust_to_screen_enabled: sample == 2,
                ify: sample == 3,
                vfx_scale_bias: if sample & 1 != 0 { -0.5 } else { 1.5 },
                properties_start: Some(AvfxBinderProperties {
                    coord_update_frame: case["deadline"].as_i64().unwrap() as i32,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let inputs = &case["inputs"];
            let target_matrix: [f32; 16] = if case["sourceKind"] == 0 {
                let object = VfxBinderObjectSnapshot {
                    position: vector(&inputs["objectPosition"]),
                    quaternion: std::array::from_fn(|axis| {
                        float(&inputs["objectQuaternion"][axis])
                    }),
                    scale: vector(&inputs["objectScale"]),
                }
                .matrix();
                let [x, y, z] = object.basis;
                let [px, py, pz] = object.position;
                [
                    x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, px, py,
                    pz, 1.0,
                ]
            } else {
                std::array::from_fn(|component| float(&inputs["target"][component]))
            };
            let initial = &case["initial"]["state"];
            let mut state = VfxCameraBinderState {
                primary: matrix(&initial["primary"]),
                auxiliary: matrix(&initial["auxiliary"]),
                scale: vector(&initial["scale"]),
                query_vfx_scale: float(&initial["vfx"]),
                transform_depth_scale: float(&initial["depth"]),
            };
            let queries = Cell::new(0);
            let listener_calls = Cell::new(0);
            for (step, snapshot) in case["updates"].as_array().unwrap().iter().enumerate() {
                let frame = VfxCameraBinderFrame {
                    age: step as f32,
                    root_flag_f4: true,
                    inverse_view: matrix(&inputs["inverseView"]),
                    camera: super::super::VfxBinderCameraSnapshot {
                        basis: std::array::from_fn(|column| {
                            std::array::from_fn(|row| {
                                float(&inputs["cameraBasis"][column * 3 + row])
                            })
                        }),
                        position: vector(&inputs["cameraPosition"]),
                        parallel_direction: vector(&inputs["parallelDirection"]),
                    },
                    document_scale: vector(&inputs["documentScale"]),
                    screen_height: case["screenHeight"].as_u64().unwrap() as u32,
                    ify_scale: Some(float(&case["ifyScale"])),
                };
                state.update(
                    &binder,
                    frame,
                    float(&case["distance"]),
                    |scale, vfx, depth| {
                        VfxCameraBinderState::query_caster(
                            &binder,
                            frame.camera.position,
                            scale,
                            vfx,
                            depth,
                            || {
                                queries.set(queries.get() + 1);
                                (step != 1).then(|| {
                                    VfxBinderTarget::from_matrix(
                                        target_matrix,
                                        binder.following_target_orientation,
                                    )
                                })
                            },
                            |out| {
                                listener_calls.set(listener_calls.get() + 1);
                                if sample == 1 && step == 0 {
                                    return false;
                                }
                                if sample == 2 && step == 0 {
                                    *out = 0.75;
                                    return false;
                                }
                                *out = if sample == 3 && step == 0 { 0.0 } else { 2.0 };
                                true
                            },
                        )
                    },
                );
                let original = &snapshot["state"];
                for (component, (actual, expected)) in state
                    .primary
                    .basis
                    .into_iter()
                    .flatten()
                    .chain(state.primary.position)
                    .chain(state.auxiliary.basis.into_iter().flatten())
                    .chain(state.auxiliary.position)
                    .chain(state.scale)
                    .chain([
                        state.query_vfx_scale,
                        state.transform_depth_scale,
                        state.depth_offset_multiplier(&binder),
                    ])
                    .zip(
                        original["primary"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .chain(original["auxiliary"].as_array().unwrap())
                            .chain(original["scale"].as_array().unwrap())
                            .chain([
                                &original["vfx"],
                                &original["depth"],
                                &original["depthMultiplier"],
                            ]),
                    )
                    .enumerate()
                {
                    let expected = float(expected);
                    if expected.is_nan() {
                        assert!(
                            actual.is_nan(),
                            "case {index} step {step} component {component}: {actual} != NaN"
                        );
                        nonfinite += 1;
                    } else {
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "case {index} step {step} component {component}"
                        );
                        if expected.is_finite() {
                            finite += 1;
                        } else {
                            nonfinite += 1;
                        }
                    }
                }
                assert_eq!(queries.get(), original["queryCalls"].as_u64().unwrap());
                assert_eq!(
                    listener_calls.get(),
                    snapshot["listenerCalls"].as_u64().unwrap()
                );
                assert_eq!(original["curveCalls"], step + 1);
            }
            all_queries += queries.get();
            all_listener_calls += listener_calls.get();
        }
        std::fs::write(output.join("camera-caster-query-rust-comparison.json"),serde_json::to_vec_pretty(&json!({
            "cases":cases.len(),"updates":cases.len()*4,"finiteComponentsCompared":finite,"nonfiniteComponentsCompared":nonfinite,
            "targetQueriesCompared":all_queries,"listenerCallbacksCompared":all_listener_calls,"differences":0,
            "originalCameraAndSharedQueryExecuteTogether":true,
            "scope":"Original Camera Update/shared caster query/basis/depth and Character Origin/quaternion vs production cached numerical update and shared query. Success/failure partial writes, listener zero/nonzero cache, Document/DSE/ATS/IFY and deadline gate compared over four updates. Origin object storage and ElementId matrices/callback success/failure, TLS/curves/Document controlled; no original constructor/owned resource tree/live listener lookup/shared RNG/CRT trig/GPU/client pixels."
        })).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "requires pinned original compiled-root CPU parser probe"]
    fn compare_original_camera_root_ags_with_production_parser() {
        use crate::avfx::AvfxFile;
        use serde_json::{Value, json};
        fn block(name: &str, payload: &[u8]) -> Vec<u8> {
            let mut bytes: Vec<u8> = name.as_bytes().iter().rev().copied().collect();
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(payload);
            while bytes.len() % 4 != 0 {
                bytes.push(0);
            }
            bytes
        }
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let native: Value = serde_json::from_slice(
            &std::fs::read(output.join("camera-root-source-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = native["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 12289);
        let mut repeated = 0;
        let mut parsed = 0;
        for (index, case) in cases.iter().enumerate() {
            let words = case["words"].as_array().unwrap();
            for width in 1_u32..=4 {
                let payload: Vec<_> = words
                    .iter()
                    .flat_map(|word| {
                        let mut leaf =
                            block("bAGS", &(word.as_u64().unwrap() as u32).to_le_bytes());
                        // The actual DWORD bytes stay intact; a short declared
                        // payload leaves the remaining bytes in the alignment slot.
                        leaf[4..8].copy_from_slice(&width.to_le_bytes());
                        leaf
                    })
                    .collect();
                let file = AvfxFile::parse(&block("AVFX", &payload)).unwrap();
                assert_eq!(
                    file.global.ags_enabled,
                    case["rootF4"].as_bool().unwrap(),
                    "case {index}, width {width}: {words:?}"
                );
                parsed += 1;
            }
            repeated += usize::from(words.len() > 1);
        }
        std::fs::write(output.join("camera-root-source-rust-comparison.json"), serde_json::to_vec_pretty(&json!({
            "nativeCases": cases.len(), "productionParseCases": parsed,
            "declaredPayloadWidths": [1,2,3,4], "repeatedFieldSequences": repeated,
            "differences": 0,
            "scope": "Original compiled-root constructor default and bAGS DWORD parser branch vs production AVFX parse. Missing=false, any nonzero DWORD=true, last repeated write wins; declared widths1..4 retain and consume all DWORD slot bytes, including nonzero alignment. Full original parser cursor/allocation not executed; malformed empty/missing-padding boundary and other fields, live host and GPU are not covered."
        })).unwrap()).unwrap();
    }

    #[test]
    fn preview_camera_view_keeps_inverse_columns_independent_of_rotp_rows() {
        // An invertible shear with a separately supplied inverse: RoTp reads
        // transposed view rows, while distance reads the inverse's third column.
        let view = [
            1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -7.0, -2.0, -3.0, 1.0,
        ];
        let inverse = [
            1.0, 0.0, 0.0, 0.0, -2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0, 2.0, 3.0, 1.0,
        ];
        let input = VfxCameraViewSnapshot::from_view_matrices(view, inverse, 0x8000_0000);
        input.validate().unwrap();
        assert_eq!(
            input.inverse_view.basis,
            [[1.0, 0.0, 0.0], [-2.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(
            input.camera.basis,
            [[1.0, 2.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(input.inverse_view.position, [3.0, 2.0, 3.0]);
        assert_eq!(input.screen_height, 0x8000_0000);
        let mut invalid = input;
        invalid.inverse_view.basis[0][1] = f32::NAN;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn camera_distance_uses_inverse_view_before_rotation_and_horizontal_mode4() {
        for rotation in [0, 1, 4, 257, 260] {
            let mut state = state();
            state.update(
                &AvfxBinder {
                    rotation_type: rotation,
                    ..Default::default()
                },
                frame(),
                2.0,
                |_, _, _| panic!("root f4 is off: no target query"),
            );
            assert_eq!(state.primary.position, [1.0, 2.0, 5.0]);
            assert_eq!(
                state.primary.basis,
                if rotation as i8 == 1 {
                    frame().camera.basis
                } else {
                    VfxBinderMatrix::IDENTITY.basis
                }
            );
        }
    }

    #[test]
    fn camera_scale_resets_or_compounds_at_inclusive_signed_query_deadline() {
        let binder = AvfxBinder {
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 2,
                ..Default::default()
            }),
            document_scale_enabled: true,
            ..Default::default()
        };
        let mut state = state();
        for (age, expected) in [
            (0.0, [8.0, 18.0, 32.0]),
            (2.0, [8.0, 18.0, 32.0]),
            (2.5, [16.0, 54.0, 128.0]),
        ] {
            let mut input = frame();
            input.root_flag_f4 = true;
            input.age = age;
            state.update(&binder, input, 1.0, |scale, _, _| {
                *scale = [2.0; 3];
                true
            });
            assert_eq!(state.scale, expected);
            assert_eq!(state.auxiliary, VfxBinderMatrix::IDENTITY);
        }
        state.update(&binder, frame(), 1.0, |_, _, _| panic!("query disabled"));
        assert_eq!(state.scale, frame().document_scale);
        assert_eq!(
            state.auxiliary,
            VfxBinderMatrix::scale_matrix(frame().document_scale)
        );
    }

    #[test]
    fn camera_failed_query_preserves_partial_cache_writes_and_independent_depth() {
        let binder = AvfxBinder {
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 65535,
                ..Default::default()
            }),
            document_scale_enabled: true,
            vfx_scale_depth_offset: true,
            transform_scale_depth_offset: true,
            ..Default::default()
        };
        let mut input = frame();
        input.root_flag_f4 = true;
        input.age = 100.0;
        let mut state = state();
        state.update(&binder, input, 0.0, |scale, vfx, depth| {
            scale[0] = 7.0;
            *vfx = 0.5;
            *depth = 3.0;
            false
        });
        assert_eq!(state.scale, [14.0, 3.0, 4.0]);
        assert_eq!(state.depth_offset_multiplier(&binder), 1.5);
    }

    #[test]
    fn camera_screen_scale_uses_1280_height_xy_and_optional_ify_all_axes() {
        let binder = AvfxBinder {
            adjust_to_screen_enabled: true,
            ify: true,
            ..Default::default()
        };
        let mut input = frame();
        input.screen_height = 5120;
        input.ify_scale = Some(-0.5);
        let mut state = state();
        state.update(&binder, input, 0.0, |_, _, _| panic!("query disabled"));
        assert_eq!(state.scale, [-1.0, -1.0, -0.5]);
        input.screen_height = 0;
        input.ify_scale = None;
        state.update(&binder, input, 0.0, |_, _, _| panic!("query disabled"));
        assert_eq!(state.scale, [1.0; 3]);
    }

    #[test]
    #[ignore = "CPU: requires scripts/probe-camera-binder-update.py pinned original update observations"]
    fn compare_original_camera_binder_update_with_cached_state() {
        use serde_json::{Value, json};
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(output.join("camera-binder-update-client-probe.json")).unwrap(),
        )
        .unwrap();
        fn float(value: &Value) -> f32 {
            f32::from_bits(value.as_u64().unwrap() as u32)
        }
        fn vector(value: &Value) -> [f32; 3] {
            std::array::from_fn(|axis| float(&value[axis]))
        }
        fn matrix(value: &Value) -> VfxBinderMatrix {
            VfxBinderMatrix {
                basis: std::array::from_fn(|column| {
                    std::array::from_fn(|row| float(&value[column * 3 + row]))
                }),
                position: std::array::from_fn(|row| float(&value[9 + row])),
            }
        }
        let mut finite = 0usize;
        let mut nonfinite = 0usize;
        let mut queries = 0usize;
        for (case_index, case) in original["cases"].as_array().unwrap().iter().enumerate() {
            let flags = case["flags"].as_u64().unwrap();
            let binder = AvfxBinder {
                binder_type: 3,
                rotation_type: case["rotation"].as_i64().unwrap() as i32,
                properties_start: case["hasStart"].as_bool().unwrap().then(|| {
                    AvfxBinderProperties {
                        coord_update_frame: case["deadline"].as_i64().unwrap() as i32,
                        ..Default::default()
                    }
                }),
                vfx_scale_depth_offset: case["sample"].as_u64().unwrap() & 1 != 0,
                transform_scale_depth_offset: case["sample"].as_u64().unwrap() & 2 != 0,
                document_scale_enabled: flags & 2 != 0,
                adjust_to_screen_enabled: flags & 4 != 0,
                ify: flags & 8 != 0,
                ..Default::default()
            };
            let inputs = &case["inputs"];
            let mut state = VfxCameraBinderState {
                primary: matrix(&case["initial"]["primary"]),
                auxiliary: matrix(&case["initial"]["auxiliary"]),
                scale: vector(&case["initial"]["scale"]),
                query_vfx_scale: float(&case["initial"]["vfx"]),
                transform_depth_scale: float(&case["initial"]["depth"]),
            };
            let mut count = 0;
            for (step, age) in [0.0, 2.0, 2.5].into_iter().enumerate() {
                state.update(
                    &binder,
                    VfxCameraBinderFrame {
                        age,
                        root_flag_f4: flags & 1 != 0,
                        inverse_view: matrix(&inputs["inverseView"]),
                        camera: VfxBinderCameraSnapshot {
                            basis: std::array::from_fn(|column| {
                                std::array::from_fn(|row| {
                                    float(&inputs["cameraBasis"][column * 3 + row])
                                })
                            }),
                            parallel_direction: vector(&inputs["parallelDirection"]),
                            position: vector(&inputs["cameraPosition"]),
                        },
                        document_scale: vector(&inputs["documentScale"]),
                        screen_height: case["screenHeight"].as_u64().unwrap() as u32,
                        ify_scale: case["ifyPresent"]
                            .as_bool()
                            .unwrap()
                            .then(|| float(&case["ifyScale"])),
                    },
                    float(&case["distance"]),
                    |scale, vfx, depth| {
                        count += 1;
                        if case["queryMode"].as_u64().unwrap() >= 2 {
                            *scale = vector(&inputs["queryScale"]);
                            *vfx = float(&inputs["queryVfx"]);
                            *depth = float(&inputs["queryDepth"]);
                        }
                        case["queryMode"] == 3
                    },
                );
                let native = &case["updates"][step];
                assert_eq!(
                    count,
                    native["queryCalls"].as_u64().unwrap(),
                    "case {case_index} step {step}"
                );
                assert_eq!(
                    native["curveCalls"],
                    step + 1,
                    "native distance is always read"
                );
                for (actual, expected) in state
                    .primary
                    .basis
                    .into_iter()
                    .flatten()
                    .chain(state.primary.position)
                    .chain(state.auxiliary.basis.into_iter().flatten())
                    .chain(state.auxiliary.position)
                    .chain(state.scale)
                    .chain([
                        state.query_vfx_scale,
                        state.transform_depth_scale,
                        state.depth_offset_multiplier(&binder),
                    ])
                    .zip(
                        native["primary"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .chain(native["auxiliary"].as_array().unwrap())
                            .chain(native["scale"].as_array().unwrap())
                            .chain([&native["vfx"], &native["depth"], &native["depthMultiplier"]]),
                    )
                {
                    let expected = float(expected);
                    if expected.is_nan() {
                        assert!(
                            actual.is_nan(),
                            "case {case_index} step {step}: {actual} != NaN"
                        );
                        nonfinite += 1;
                    } else {
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "case {case_index} step {step}: {actual} != {expected}"
                        );
                        if expected.is_finite() {
                            finite += 1;
                        } else {
                            nonfinite += 1;
                        }
                    }
                }
            }
            queries += count as usize;
        }
        std::fs::write(output.join("camera-binder-update-rust-comparison.json"), serde_json::to_vec_pretty(&json!({
            "cases": original["cases"].as_array().unwrap().len(), "updateBoundaries": original["cases"].as_array().unwrap().len() * 3,
            "finiteComponentsCompared": finite, "nonfiniteComponentsCompared": nonfinite, "targetQueriesCompared": queries,
            "differences": 0, "productionNumericalStateCompared": true, "fullOwnedPlaybackCompared": false,
            "scope": "Original Camera Binder 3c0750 / look-at helper vs production cached numerical state across three sequential updates. Primary/auxiliary matrices, scale, VFX/depth caches and query counts compared. Finite and Infinity bitwise; NaN classification only. Controlled TLS inverse-view and orientation inputs, resolved distance getter, query writes, Document, viewport and optional IFY transform. Original distance reads observed, caller supplies pre-resolved values; production curve/RNG reads not compared. No constructor, factory, complete owned phases, live host/resources/shared RNG/GPU."
        })).unwrap()).unwrap();
    }
}
