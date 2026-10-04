use super::binder::{
    VfxBinderBirthClock, VfxBinderLifecycle, VfxBinderMatrix, VfxBinderQueryStatus, VfxBinderStep,
    VfxBinderTarget, VfxLinearBinderCurves, VfxLinearBinderFrame, VfxLinearBinderInitialization,
    VfxLinearBinderState,
};
use super::client_trig::{self, VfxClientTrigMode};
use crate::avfx::AvfxBinder;

/// One cached Spline Binder knot. The parameter is normalized RnPT/lifetime
/// for an interior control, or 0/1 for the queried start/goal. Positions are
/// already transformed into the Binder's space; this is not an AVFX curve key.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxSplineBinderKnot {
    pub parameter: f32,
    pub position: [f32; 3],
}

/// The Spline Binder's two to four cached world-space knots, sampled by
/// 0x1403c3750. Construction/query timing, control-point basis and RNG are
/// separate producers; callers must not use this cache to admit a full Binder.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxSplineBinderPath {
    knots: Vec<VfxSplineBinderKnot>,
}

impl VfxSplineBinderPath {
    /// Preserve the original cache order and values, including duplicate or
    /// reversed parameters. The original constructors always store 2..=4 knots.
    pub fn from_knots(knots: &[VfxSplineBinderKnot]) -> Result<Self, String> {
        if !(2..=4).contains(&knots.len()) {
            return Err("Spline Binder path requires two to four cached knots".into());
        }
        Ok(Self {
            knots: knots.to_vec(),
        })
    }

    pub fn knots(&self) -> &[VfxSplineBinderKnot] {
        &self.knots
    }

    /// Sample each axis with the native nonuniform Hermite tangents, binary
    /// segment selection, endpoint clamps and f32 operation order. The factor
    /// is already resolved COF; it is not clamped before selecting the knots.
    pub fn sample(&self, factor: f32) -> [f32; 3] {
        let first = self.knots[0];
        let last = self.knots[self.knots.len() - 1];
        if first.parameter >= factor {
            return first.position;
        }
        if factor >= last.parameter {
            return last.position;
        }
        // COMISS/JBE treats unordered comparisons as the lower branch. In
        // particular, a NaN factor reaches interpolation rather than a clamp.
        let mut low = 0i32;
        let mut high = self.knots.len() as i32 - 1;
        let mut segment = 0usize;
        while low <= high {
            let mid = low + ((high - low) >> 1);
            let index = mid as usize;
            if self.knots[index].parameter > factor {
                high = mid - 1;
            } else {
                low = mid + 1;
                if self.knots.get(index + 1).is_some_and(|next| {
                    factor.partial_cmp(&next.parameter) != Some(std::cmp::Ordering::Greater)
                }) {
                    segment = index;
                    break;
                }
            }
        }
        let left = self.knots[segment];
        let right = self.knots[segment + 1];
        let width = right.parameter - left.parameter;
        let t = (factor - left.parameter) / width;
        let square = t * t;
        let cube = square * t;
        let triple_square = square * 3.0;
        let double_cube = cube + cube;
        std::array::from_fn(|axis| {
            let delta = right.position[axis] - left.position[axis];
            let outgoing = if segment > 0 {
                let previous = self.knots[segment - 1];
                (width / (right.parameter - previous.parameter))
                    * ((left.position[axis] - previous.position[axis]) + delta)
            } else {
                delta
            };
            let incoming = if let Some(next) = self.knots.get(segment + 2) {
                (width / (next.parameter - left.parameter))
                    * ((next.position[axis] - right.position[axis]) + delta)
            } else {
                delta
            };
            let a = ((double_cube - triple_square) + 1.0) * left.position[axis];
            let b = (triple_square - double_cube) * right.position[axis];
            let c = ((cube - (square + square)) + t) * outgoing;
            let d = (cube - square) * incoming;
            ((a + b) + c) + d
        })
    }
}

/// Resolved constructor inputs for an interior Spline control. Its enabled
/// bit is Prp1/Prp2 bRng, position is RnPX/Y/Z, parameter is signed RnPT/life,
/// and offset is the separately cached constructor random displacement.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxSplineBinderControl {
    pub enabled: bool,
    pub parameter: f32,
    pub position: [f32; 3],
    pub offset: [f32; 3],
}

/// Numerical construction; allocation, attachment and child resources belong
/// to the caller. A missing initialization means the delay is still pending.
#[derive(Clone, Debug, PartialEq)]
pub struct VfxSplineBinderConstruction {
    pub state: VfxSplineBinderState,
    pub clock: VfxBinderBirthClock,
    pub initialization: Option<VfxLinearBinderInitialization>,
}

/// Persistent Spline numerical caches and own Common lifecycle. Child
/// allocation, attachment and descendant traversal remain caller-owned.
#[derive(Clone, Debug, PartialEq)]
pub struct VfxSplineBinderInstance {
    pub state: VfxSplineBinderState,
    pub lifecycle: VfxBinderLifecycle,
    pub initialized: bool,
    pub trig_mode: VfxClientTrigMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VfxSplineBinderAdvance {
    pub update: Option<[VfxBinderQueryStatus; 2]>,
    pub initialization: Option<VfxLinearBinderInitialization>,
}

impl VfxSplineBinderConstruction {
    /// Registered children count successful attachments, not merely a
    /// successful query. An attempted failed initializer is still consumed.
    pub fn into_instance(
        self,
        registered_children: u16,
        trig_mode: VfxClientTrigMode,
    ) -> VfxSplineBinderInstance {
        let mut lifecycle = VfxBinderLifecycle::new(self.clock);
        if self.initialization.is_none() {
            lifecycle.defer_single_factory();
        }
        lifecycle.set_registered_children(registered_children);
        VfxSplineBinderInstance {
            state: self.state,
            lifecycle,
            initialized: self.initialization.is_some(),
            trig_mode,
        }
    }
}

impl VfxSplineBinderInstance {
    pub fn advance_time(&mut self, delta: f32) {
        self.lifecycle.advance_time(delta);
    }

    /// Ordinary +70, after the whole owner's +68 pass. A newly attached child
    /// has missed that pass; its age must not be advanced retroactively.
    pub fn prepare_self(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        random: impl FnMut() -> u16,
        curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        attach_child: impl FnMut(
            &VfxSplineBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxSplineBinderAdvance {
        self.run_self_step(
            binder,
            None,
            frame,
            target,
            listener_scale,
            random,
            curves,
            root_revision,
            attach_child,
        )
    }

    /// Prewarm +60: initialization observes the delay overshoot, then Common
    /// overwrites that temporary age with the pending ordinary clock writes.
    pub fn advance_self(
        &mut self,
        binder: &AvfxBinder,
        delta: f32,
        frame: VfxLinearBinderFrame,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        random: impl FnMut() -> u16,
        curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        attach_child: impl FnMut(
            &VfxSplineBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxSplineBinderAdvance {
        self.run_self_step(
            binder,
            Some(delta),
            frame,
            target,
            listener_scale,
            random,
            curves,
            root_revision,
            attach_child,
        )
    }

    fn run_self_step(
        &mut self,
        binder: &AvfxBinder,
        prewarm_delta: Option<f32>,
        mut frame: VfxLinearBinderFrame,
        mut target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(usize, &mut f32) -> bool,
        mut random: impl FnMut() -> u16,
        mut curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        mut attach_child: impl FnMut(
            &VfxSplineBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxSplineBinderAdvance {
        let mut result = VfxSplineBinderAdvance::default();
        frame.query_deadlines = VfxLinearBinderFrame::authored_query_deadlines(binder);
        let step = |lifecycle: &mut VfxBinderLifecycle, phase| {
            frame.age = lifecycle.clock.local_age;
            match phase {
                VfxBinderStep::Update if self.initialized => {
                    result.update = Some(self.state.update_frame(
                        binder,
                        frame,
                        &mut target,
                        &mut listener_scale,
                        curves(frame.age, lifecycle.clock.total_age),
                    ));
                }
                VfxBinderStep::Update => {}
                VfxBinderStep::Initialize => {
                    lifecycle.consume_pending_factory();
                    self.initialized = true;
                    let initialized = self.state.initialize_with_client_trig(
                        binder,
                        frame,
                        lifecycle.clock.nominal_life,
                        &mut target,
                        &mut listener_scale,
                        &mut random,
                        self.trig_mode,
                        || curves(frame.age, lifecycle.clock.total_age),
                        root_revision,
                    );
                    if initialized.child_direction.is_some()
                        && attach_child(&self.state, &initialized, lifecycle.clock)
                    {
                        lifecycle.register_child();
                    }
                    result.initialization = Some(initialized);
                }
            }
        };
        if let Some(delta) = prewarm_delta {
            self.lifecycle.advance_self(delta, step);
        } else {
            self.lifecycle.prepare_self(step);
        }
        result
    }
}

/// Numeric Spline state around 3c4d60. Endpoint and scalar history follow the
/// shared Binder query, while Spline replaces Linear position with its own
/// nonuniform path. Numeric construction/initialization is provided below;
/// owned factories and live shared RNG wiring remain external. Native finite
/// angular arithmetic is available with an explicit client CRT dispatch snapshot.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxSplineBinderState {
    pub common: VfxLinearBinderState,
    controls: [VfxSplineBinderControl; 2],
    path: VfxSplineBinderPath,
}

impl VfxSplineBinderState {
    /// Both original Spline constructors retain zero target/path caches,
    /// Document scale, and two plus bRng enabled controls before initialization.
    pub fn constructed(binder: &AvfxBinder, document_scale: [f32; 3]) -> Self {
        let properties = [binder.properties_1.as_ref(), binder.properties_2.as_ref()];
        let controls = properties.map(|p| VfxSplineBinderControl {
            enabled: p.is_some_and(|p| p.ring_enabled),
            parameter: 0.0,
            position: p.map_or([0.0; 3], |p| p.ring_position),
            offset: [0.0; 3],
        });
        let count = 2 + controls.iter().filter(|c| c.enabled).count();
        Self {
            common: VfxLinearBinderState::constructed(document_scale),
            controls,
            path: VfxSplineBinderPath::from_knots(&vec![
                VfxSplineBinderKnot {
                    parameter: 0.0,
                    position: [0.0; 3],
                };
                count
            ])
            .unwrap(),
        }
    }

    /// Construct with the client's finite cosf/sinf arithmetic in original call
    /// order. The caller supplies the shared random stream and resolved CRT
    /// dispatch mode; host CPU capabilities do not select the client's mode.
    pub fn construct_with_client_trig(
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        delay: f32,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        random: impl FnMut() -> u16,
        mode: VfxClientTrigMode,
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxSplineBinderConstruction {
        Self::construct(
            binder,
            frame,
            delay,
            target,
            listener_scale,
            random,
            |angle| client_trig::spline_transverse(angle, mode),
            curves,
            root_revision,
        )
    }

    /// Initialize delayed numerical storage using a resolved client CRT mode.
    /// Both initial queries must succeed before any random draw is consumed.
    pub fn initialize_with_client_trig(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        life: f32,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        random: impl FnMut() -> u16,
        mode: VfxClientTrigMode,
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxLinearBinderInitialization {
        self.initialize(
            binder,
            frame,
            life,
            target,
            listener_scale,
            random,
            |angle| client_trig::spline_transverse(angle, mode),
            curves,
            root_revision,
        )
    }

    /// Numeric constructor with externally resolved angular returns, for
    /// controlled arithmetic comparisons or a separate CRT implementation.
    /// No alternate per-control RNG is seeded.
    pub fn construct(
        binder: &AvfxBinder,
        mut frame: VfxLinearBinderFrame,
        delay: f32,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        random: impl FnMut() -> u16,
        angular: impl FnMut(f32) -> [f32; 2],
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxSplineBinderConstruction {
        let clock = VfxBinderBirthClock::new(frame.age, binder.life, delay);
        frame.age = clock.local_age;
        frame.query_deadlines = VfxLinearBinderFrame::authored_query_deadlines(binder);
        let mut state = Self::constructed(binder, frame.document_scale);
        let initialization = clock.initializes_immediately().then(|| {
            state.initialize(
                binder,
                frame,
                clock.nominal_life,
                target,
                listener_scale,
                random,
                angular,
                curves,
                root_revision,
            )
        });
        VfxSplineBinderConstruction {
            state,
            clock,
            initialization,
        }
    }

    /// 3c4430 queries start then goal unconditionally; failure bypasses random
    /// draws, curves, direction and the child factory. Successful initialization
    /// draws both radii followed by both angles, even for disabled controls.
    pub fn initialize(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        life: f32,
        mut target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(usize, &mut f32) -> bool,
        mut random: impl FnMut() -> u16,
        mut angular: impl FnMut(f32) -> [f32; 2],
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxLinearBinderInitialization {
        let mut result = VfxLinearBinderInitialization {
            queries: [None; 2],
            update: None,
            child_direction: None,
        };
        for endpoint in 0..2 {
            let status = self.common.query_endpoint(
                binder,
                frame,
                endpoint,
                [1.0; 3],
                || target(endpoint),
                |value| listener_scale(endpoint, value),
            );
            result.queries[endpoint] = Some(status);
            if status != VfxBinderQueryStatus::Refreshed {
                return result;
            }
            if endpoint == 0 {
                self.common.vfx_scale = self.common.endpoint_vfx_scales[0];
                self.common.transform_depth_scale = self.common.endpoint_depth_scales[0];
            }
        }
        self.common.scale = std::array::from_fn(|a| frame.document_scale[a] * self.common.scale[a]);
        self.common.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.common.scale);
        let properties = [binder.properties_1.as_ref(), binder.properties_2.as_ref()];
        let radii = properties.map(|p| {
            (f32::from(random()) * 2.0 / 65535.0 - 1.0) * p.map_or(0.0, |p| p.ring_radius)
        });
        let angles: [f32; 2] = std::array::from_fn(|_| {
            f32::from(random()) * std::f32::consts::TAU / 65535.0 - std::f32::consts::PI
        });
        for i in 0..2 {
            let values = angular(angles[i]);
            self.controls[i].offset = [values[0] * radii[i], values[1] * radii[i], 0.0];
            self.controls[i].parameter =
                properties[i].map_or(0, |p| p.ring_progress_time) as i16 as f32 / life;
        }
        result.update = Some(self.update_frame(binder, frame, target, listener_scale, curves()));
        result.child_direction = Some(self.birth_direction());
        if frame.root_ags {
            self.common.auxiliary_matrix = root_revision;
        }
        result
    }

    /// 3c4060 points from the start toward the first enabled interior control,
    /// otherwise the goal. It does not normalize; tiny directions fall back
    /// to the start Z column. Recompute from raw endpoint caches, not COF.
    pub fn birth_direction(&self) -> [f32; 3] {
        let [start, goal] = self.common.targets;
        let frame = Self::control_frame(start, goal);
        let distance = Self::endpoint_distance(start, goal);
        let destination = self
            .controls
            .iter()
            .find(|c| c.enabled)
            .map_or(goal.position, |c| Self::control_world(*c, frame, distance));
        let delta: [f32; 3] = std::array::from_fn(|a| destination[a] - start.position[a]);
        // Birth direction uses X+Y+Z, unlike the Y+X+Z endpoint distance.
        if (delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2]
            < f32::from_bits(0x38d1b717)
        {
            start.basis[2]
        } else {
            delta
        }
    }

    fn endpoint_distance(start: VfxBinderMatrix, goal: VfxBinderMatrix) -> f32 {
        let delta: [f32; 3] = std::array::from_fn(|a| start.position[a] - goal.position[a]);
        ((delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2]).sqrt()
    }

    fn control_frame(start: VfxBinderMatrix, goal: VfxBinderMatrix) -> VfxBinderMatrix {
        let mut frame = VfxBinderMatrix {
            basis: [[0.0; 3]; 3],
            // 3c53d6..3c53f1 clears the entire temporary matrix before
            // either look-at call. Both failures preserve zero translation,
            // independently of COF and the main matrix's interpolated origin.
            position: [0.0; 3],
        };
        let source = VfxBinderMatrix {
            basis: [start.basis[0], goal.basis[1], start.basis[2]],
            ..start
        };
        let fallback = VfxBinderMatrix {
            position: std::array::from_fn(|a| goal.position[a] - goal.basis[2][a]),
            ..source
        };
        if let Some(basis) = source.linear_look_at(goal.position) {
            frame.basis = basis;
            frame.position = start.position;
        } else if let Some(basis) = fallback.linear_look_at(goal.position) {
            frame.basis = basis;
            frame.position = fallback.position;
        }
        frame
    }

    fn control_world(
        control: VfxSplineBinderControl,
        frame: VfxBinderMatrix,
        distance: f32,
    ) -> [f32; 3] {
        let local = [
            control.position[0] + control.offset[0],
            control.position[1] + control.offset[1],
            control.position[2] * distance + control.offset[2],
        ];
        std::array::from_fn(|row| {
            ((local[1] * frame.basis[1][row] + local[0] * frame.basis[0][row])
                + local[2] * frame.basis[2][row])
                + frame.position[row]
        })
    }

    pub fn controls(&self) -> &[VfxSplineBinderControl; 2] {
        &self.controls
    }

    pub fn path(&self) -> &VfxSplineBinderPath {
        &self.path
    }

    /// Install prepared numerical caches, not original constructor defaults.
    pub fn from_cache(common: VfxLinearBinderState, controls: [VfxSplineBinderControl; 2]) -> Self {
        let mut knots = vec![VfxSplineBinderKnot {
            parameter: 0.0,
            position: common.targets[0].position,
        }];
        knots.extend(
            controls
                .into_iter()
                .filter(|control| control.enabled)
                .map(|control| VfxSplineBinderKnot {
                    parameter: control.parameter,
                    position: [0.0; 3],
                }),
        );
        knots.push(VfxSplineBinderKnot {
            parameter: 1.0,
            position: common.targets[1].position,
        });
        Self {
            common,
            controls,
            path: VfxSplineBinderPath::from_knots(&knots).unwrap(),
        }
    }

    pub fn update_frame(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        mut target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(usize, &mut f32) -> bool,
        curves: VfxLinearBinderCurves,
    ) -> [VfxBinderQueryStatus; 2] {
        let statuses = std::array::from_fn(|endpoint| {
            let deadline = frame.query_deadlines[endpoint];
            if !(deadline < 0.0 || deadline >= frame.age) {
                return VfxBinderQueryStatus::Skipped;
            }
            let status = self.common.query_endpoint(
                binder,
                frame,
                endpoint,
                if endpoint == 0 {
                    frame.document_scale
                } else {
                    [1.0; 3]
                },
                || target(endpoint),
                |value| listener_scale(endpoint, value),
            );
            if endpoint == 0 && !frame.root_ags {
                self.common.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.common.scale);
            }
            status
        });
        // Basis, scalar interpolation, property caches and multiply have the
        // same original helpers as Linear. RoTp happens after Spline position.
        self.common.update_resolved(binder, curves);
        let [start, goal] = self.common.targets;
        let control_frame = Self::control_frame(start, goal);
        let distance = Self::endpoint_distance(start, goal);
        let mut index = 0;
        self.path.knots[index] = VfxSplineBinderKnot {
            parameter: 0.0,
            position: start.position,
        };
        index += 1;
        for control in self.controls {
            if !control.enabled {
                continue;
            }
            let world = Self::control_world(control, control_frame, distance);
            self.path.knots[index] = VfxSplineBinderKnot {
                parameter: control.parameter,
                position: world,
            };
            index += 1;
        }
        self.path.knots[index] = VfxSplineBinderKnot {
            parameter: 1.0,
            position: goal.position,
        };
        self.common.matrix.position = self.path.sample(curves.factor);
        self.common
            .matrix
            .apply_camera_rotation(binder.rotation_type, frame.camera);
        statuses
    }

    pub fn depth_offset_multiplier(&self, binder: &AvfxBinder) -> f32 {
        self.common.depth_offset_multiplier(binder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls(enabled: bool) -> [VfxSplineBinderControl; 2] {
        [
            VfxSplineBinderControl {
                enabled,
                parameter: 0.25,
                position: [1.0, 0.0, 0.0],
                offset: [0.0; 3],
            },
            VfxSplineBinderControl {
                enabled: false,
                parameter: 0.75,
                position: [0.0; 3],
                offset: [0.0; 3],
            },
        ]
    }

    fn frame() -> VfxLinearBinderFrame {
        VfxLinearBinderFrame {
            age: 0.0,
            query_deadlines: [-1.0; 2],
            camera_position: [0.0, 0.0, 5.0],
            camera: None,
            document_scale: [1.0; 3],
            self_targets: [true; 2],
            root_ags: false,
        }
    }

    fn curves() -> VfxLinearBinderCurves {
        VfxLinearBinderCurves {
            factor: 0.5,
            start_position: [10.0, 20.0, 30.0],
            goal_position: [40.0, 50.0, 60.0],
            start_position_after: [70.0, 80.0, 90.0],
        }
    }

    #[test]
    fn spline_initial_query_failure_bypasses_random_curves_and_direction() {
        let binder = AvfxBinder {
            binder_type: 2,
            life: 8,
            ..Default::default()
        };
        let construction = VfxSplineBinderState::construct(
            &binder,
            frame(),
            0.0,
            |_| None,
            |_, _| unreachable!(),
            || unreachable!(),
            |_| unreachable!(),
            || unreachable!(),
            VfxBinderMatrix::IDENTITY,
        );
        let init = construction.initialization.unwrap();
        assert_eq!(
            init.queries,
            [Some(VfxBinderQueryStatus::TargetUnavailable), None]
        );
        assert_eq!(init.child_direction, None);
        assert_eq!(construction.state.common.matrix, VfxBinderMatrix::IDENTITY);
        assert!(
            construction
                .state
                .path
                .knots()
                .iter()
                .all(|k| k.parameter == 0.0 && k.position == [0.0; 3])
        );
    }

    #[test]
    fn spline_disabled_controls_still_draw_both_radii_then_both_angles_and_keep_word_parameters() {
        let properties = crate::avfx::AvfxBinderProperties {
            ring_enabled: false,
            ring_radius: 2.0,
            ring_progress_time: 65535,
            ..Default::default()
        };
        let binder = AvfxBinder {
            binder_type: 2,
            life: 8,
            properties_1: Some(properties.clone()),
            properties_2: Some(properties),
            ..Default::default()
        };
        let mut draws = [0u16, 65535, 0, 65535].into_iter();
        let mut angles = Vec::new();
        let construction = VfxSplineBinderState::construct(
            &binder,
            frame(),
            -0.0,
            |e| {
                Some(VfxBinderTarget {
                    basis: VfxBinderMatrix::IDENTITY.basis,
                    scale: [1.0; 3],
                    position: [0.0, 0.0, e as f32 * 4.0],
                })
            },
            |_, _| unreachable!(),
            || draws.next().unwrap(),
            |a| {
                angles.push(a);
                [1.0, 0.0]
            },
            || curves(),
            VfxBinderMatrix::IDENTITY,
        );
        assert_eq!(draws.next(), None);
        assert_eq!(angles, [-std::f32::consts::PI, std::f32::consts::PI]);
        assert_eq!(construction.state.controls[0].offset, [-2.0, -0.0, 0.0]);
        assert_eq!(construction.state.controls[1].offset, [2.0, 0.0, 0.0]);
        assert_eq!(construction.state.controls[0].parameter, -0.125);
        assert_eq!(construction.state.path.knots().len(), 2);
        assert_eq!(
            construction.initialization.unwrap().child_direction,
            Some([0.0, 0.0, 4.0])
        );
    }

    fn lifecycle_target(e: usize) -> Option<VfxBinderTarget> {
        Some(VfxBinderTarget {
            basis: VfxBinderMatrix::IDENTITY.basis,
            scale: [1.0; 3],
            position: [0.0, 0.0, e as f32 * 4.0],
        })
    }

    #[test]
    fn spline_instance_normal_and_prewarm_delay_keep_distinct_final_and_child_birth_ages() {
        let binder = AvfxBinder {
            binder_type: 2,
            life: 8,
            ..Default::default()
        };
        for prewarm in [false, true] {
            let birth = VfxSplineBinderState::construct_with_client_trig(
                &binder,
                VfxLinearBinderFrame {
                    age: 3.0,
                    ..frame()
                },
                2.0,
                |_| unreachable!(),
                |_, _| unreachable!(),
                || unreachable!(),
                VfxClientTrigMode::Sse2,
                || unreachable!(),
                VfxBinderMatrix::IDENTITY,
            );
            let mut instance = birth.into_instance(0, VfxClientTrigMode::Sse2);
            assert_eq!(instance.lifecycle.raw_flags() & 0x1ff, 1);
            assert!(!instance.lifecycle.life_limit_enabled);
            instance.lifecycle.clock.rate = 0.5;
            let mut draws = 0;
            let mut random = || {
                draws += 1;
                0
            };
            let mut child_clock = None;
            let mut attach = |_: &VfxSplineBinderState,
                              _: &VfxLinearBinderInitialization,
                              clock: VfxBinderBirthClock| {
                child_clock = Some(clock);
                true
            };
            let mut sampled_ages = Vec::new();
            let mut sample_curves = |age, total| {
                sampled_ages.push([age, total]);
                curves()
            };
            let advanced = if prewarm {
                instance.advance_self(
                    &binder,
                    5.0,
                    frame(),
                    lifecycle_target,
                    |_, _| unreachable!(),
                    &mut random,
                    &mut sample_curves,
                    VfxBinderMatrix::IDENTITY,
                    &mut attach,
                )
            } else {
                instance.advance_time(5.0);
                instance.prepare_self(
                    &binder,
                    frame(),
                    lifecycle_target,
                    |_, _| unreachable!(),
                    &mut random,
                    &mut sample_curves,
                    VfxBinderMatrix::IDENTITY,
                    &mut attach,
                )
            };
            assert!(advanced.initialization.unwrap().child_direction.is_some());
            assert_eq!(draws, 4);
            assert_eq!(sampled_ages, [[0.25, 0.25]]);
            assert_eq!(child_clock.unwrap().local_age, 0.25);
            assert_eq!(child_clock.unwrap().previous_age, 5.5);
            assert_eq!(
                instance.lifecycle.clock.local_age,
                if prewarm { 5.5 } else { 0.25 }
            );
            assert_eq!(
                instance.lifecycle.clock.total_age,
                if prewarm { 5.5 } else { 0.25 }
            );
            assert_eq!(instance.lifecycle.clock.previous_age, 5.5);
            assert_eq!(instance.lifecycle.clock.delay, -0.5);
            assert_eq!(instance.lifecycle.registered_children(), 1);
            assert_eq!(instance.lifecycle.raw_flags() & 0x1ff, 0);
            assert!(instance.lifecycle.life_limit_enabled);
        }
    }

    #[test]
    fn spline_instance_failed_initialize_updates_later_without_retrying_factory_or_random() {
        let binder = AvfxBinder {
            binder_type: 2,
            life: 10,
            ..Default::default()
        };
        let birth = VfxSplineBinderState::construct_with_client_trig(
            &binder,
            frame(),
            2.0,
            |_| unreachable!(),
            |_, _| unreachable!(),
            || unreachable!(),
            VfxClientTrigMode::Sse2,
            || unreachable!(),
            VfxBinderMatrix::IDENTITY,
        );
        let mut instance = birth.into_instance(0, VfxClientTrigMode::Sse2);
        instance.advance_time(2.5);
        let first = instance.prepare_self(
            &binder,
            frame(),
            |_| None,
            |_, _| unreachable!(),
            || unreachable!(),
            |_, _| unreachable!(),
            VfxBinderMatrix::IDENTITY,
            |_, _, _| unreachable!(),
        );
        assert!(first.initialization.is_some());
        assert!(instance.initialized);
        assert!(instance.lifecycle.life_limit_enabled);
        assert_eq!(instance.lifecycle.raw_flags() & 0x1ff, 0);
        instance.advance_time(0.0);
        let recovered = instance.prepare_self(
            &binder,
            frame(),
            lifecycle_target,
            |_, _| unreachable!(),
            || unreachable!(),
            |_, _| curves(),
            VfxBinderMatrix::IDENTITY,
            |_, _, _| unreachable!(),
        );
        assert_eq!(recovered.update, Some([VfxBinderQueryStatus::Refreshed; 2]));
        assert!(recovered.initialization.is_none());
        assert_eq!(instance.lifecycle.registered_children(), 0);
    }

    #[test]
    fn spline_empty_infinite_life_retires_with_unconsumed_delay() {
        let binder = AvfxBinder {
            binder_type: 2,
            life: -1,
            ..Default::default()
        };
        let birth = VfxSplineBinderState::construct_with_client_trig(
            &binder,
            frame(),
            2.0,
            |_| unreachable!(),
            |_, _| unreachable!(),
            || unreachable!(),
            VfxClientTrigMode::Sse2,
            || unreachable!(),
            VfxBinderMatrix::IDENTITY,
        );
        let mut instance = birth.into_instance(0, VfxClientTrigMode::Sse2);
        instance.advance_time(0.0);
        let advanced = instance.prepare_self(
            &binder,
            frame(),
            |_| unreachable!(),
            |_, _| unreachable!(),
            || unreachable!(),
            |_, _| unreachable!(),
            VfxBinderMatrix::IDENTITY,
            |_, _, _| unreachable!(),
        );
        assert_eq!(advanced, VfxSplineBinderAdvance::default());
        assert_eq!(instance.lifecycle.raw_flags(), 0x40001);
        assert_eq!(instance.lifecycle.clock.delay, 2.0);
        assert!(!instance.initialized);
    }

    #[test]
    fn spline_two_failed_control_lookats_retain_zero_translation_independently_of_cof() {
        let binder = AvfxBinder {
            binder_type: 2,
            ..Default::default()
        };
        for factor in [0.5, f32::NAN] {
            let mut common = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
            common.targets = [VfxBinderMatrix {
                basis: [[0.0; 3]; 3],
                position: [2.0, -3.0, 4.0],
            }; 2];
            let mut state = VfxSplineBinderState::from_cache(common, controls(true));
            state.update_frame(
                &binder,
                frame(),
                |_| None,
                |_, _| unreachable!(),
                VfxLinearBinderCurves {
                    factor,
                    start_position: [0.0; 3],
                    goal_position: [0.0; 3],
                    start_position_after: [0.0; 3],
                },
            );
            assert_eq!(state.path.knots()[1].position, [0.0; 3]);
            assert_eq!(state.path.knots()[0].position, [2.0, -3.0, 4.0]);
        }
    }

    #[test]
    fn spline_client_trig_caches_original_rounding_and_draw_order_at_immediate_or_delayed_birth() {
        let properties = crate::avfx::AvfxBinderProperties {
            ring_enabled: true,
            ring_radius: 2.0,
            ring_progress_time: 2,
            ..Default::default()
        };
        let binder = AvfxBinder {
            binder_type: 2,
            life: 8,
            properties_1: Some(properties.clone()),
            properties_2: Some(properties),
            ..Default::default()
        };
        for mode in [VfxClientTrigMode::Sse2, VfxClientTrigMode::AvxFma] {
            for delay in [0.0, 2.0] {
                let mut draws = [0u16, 65535, 0, 65535].into_iter();
                let target = |e| {
                    Some(VfxBinderTarget {
                        basis: VfxBinderMatrix::IDENTITY.basis,
                        scale: [1.0; 3],
                        position: [0.0, 0.0, e as f32 * 4.0],
                    })
                };
                let mut construction = VfxSplineBinderState::construct_with_client_trig(
                    &binder,
                    frame(),
                    delay,
                    target,
                    |_, _| unreachable!(),
                    || draws.next().unwrap(),
                    mode,
                    curves,
                    VfxBinderMatrix::IDENTITY,
                );
                if delay > 0.0 {
                    assert!(construction.initialization.is_none());
                    assert_eq!(draws.len(), 4);
                    let initialized = construction.state.initialize_with_client_trig(
                        &binder,
                        frame(),
                        8.0,
                        target,
                        |_, _| unreachable!(),
                        || draws.next().unwrap(),
                        mode,
                        curves,
                        VfxBinderMatrix::IDENTITY,
                    );
                    assert!(initialized.child_direction.is_some());
                } else {
                    assert!(
                        construction
                            .initialization
                            .unwrap()
                            .child_direction
                            .is_some()
                    );
                }
                assert_eq!(draws.len(), 0);
                // Native cosf(±floatPI)=-1 and sinf(-floatPI)=0x33bbbd2e.
                // Preserve its tiny nonzero value rather than snapping to zero.
                let state = construction.state;
                assert_eq!(
                    state.controls[0].offset.map(f32::to_bits),
                    [2.0f32.to_bits(), 0xb43bbd2e, 0]
                );
                assert_eq!(
                    state.controls[1].offset.map(f32::to_bits),
                    [(-2.0f32).to_bits(), 0xb43bbd2e, 0]
                );
                assert_eq!(state.controls[0].parameter, 0.25);
                assert_eq!(state.path.knots().len(), 4);
            }
        }
    }

    #[test]
    fn spline_birth_direction_prefers_first_enabled_control_and_retains_unnormalized_or_tiny_fallback()
     {
        let mut common = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        common.targets[1].position = [0.0, 0.0, 4.0];
        let mut c = controls(true);
        c[1].enabled = true;
        c[1].position = [0.0, 2.0, 0.75];
        let mut state = VfxSplineBinderState::from_cache(common, c);
        assert_eq!(state.birth_direction(), [1.0, 0.0, 0.0]);
        state.controls[0].enabled = false;
        assert_eq!(state.birth_direction(), [0.0, 2.0, 3.0]);
        state.controls[1].position = [0.0; 3];
        assert_eq!(state.birth_direction(), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn spline_positive_and_unordered_delay_keep_zero_caches_without_consuming_sources() {
        let binder = AvfxBinder {
            binder_type: 2,
            life: 0,
            properties_1: Some(crate::avfx::AvfxBinderProperties {
                ring_enabled: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        for delay in [2.0, f32::NAN] {
            let construction = VfxSplineBinderState::construct(
                &binder,
                frame(),
                delay,
                |_| unreachable!(),
                |_, _| unreachable!(),
                || unreachable!(),
                |_| unreachable!(),
                || unreachable!(),
                VfxBinderMatrix::IDENTITY,
            );
            assert!(construction.initialization.is_none());
            assert_eq!(construction.state.path.knots().len(), 3);
            assert_eq!(construction.state.controls[0].parameter, 0.0);
            assert_eq!(construction.state.common.targets[0].position, [0.0; 3]);
        }
    }

    #[test]
    #[ignore = "compare original Spline constructors and initialize with real shared query, inline TLS RNG and direction"]
    fn compare_original_spline_binder_construction_and_initialization() {
        compare_spline_construction("spline-binder-construction", false, false);
    }

    #[test]
    #[ignore = "compare original Spline construction with both real CRT paths against production entrypoints"]
    fn compare_original_spline_binder_construction_with_client_trig() {
        compare_spline_construction("spline-binder-trig", true, false);
    }

    #[test]
    #[ignore = "compare original Spline normal/prewarm own lifecycle with successful child Common/Timeline construction"]
    fn compare_original_spline_binder_own_lifecycle() {
        compare_spline_construction("spline-binder-lifecycle", true, true);
    }

    #[test]
    #[ignore = "CPU: requires probe-spline-target-factory.py original dual factories"]
    fn compare_original_spline_dual_factories_with_shared_random_state() {
        use super::super::VfxClientRandomState;
        use serde_json::{Value, json};
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(folder.join("spline-target-factory-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 122880);
        let matrix_bits = |m: VfxBinderMatrix| {
            m.basis
                .into_iter()
                .flatten()
                .chain(m.position)
                .map(f32::to_bits)
                .collect::<Vec<_>>()
        };
        let mut objects = 0usize;
        let mut draws = 0usize;
        let mut rejected_objects = 0usize;
        let mut capacity_failures = 0usize;
        let mut components = 0usize;
        for (ordinal, case) in cases.iter().enumerate() {
            let start = case["startMode"].as_u64().unwrap() as i32;
            let goal = case["goalMode"].as_u64().unwrap() as i32;
            let count = case["targetCount"].as_i64().unwrap() as i32;
            let mask = case["mask"].as_u64().unwrap();
            let owner = case["ownerKind"].as_u64().unwrap();
            let success = case["querySuccessMask"].as_u64().unwrap();
            let retired = case["retired"].as_bool().unwrap();
            let mode = if case["trigMode"] == 0 {
                VfxClientTrigMode::Sse2
            } else {
                VfxClientTrigMode::AvxFma
            };
            let binder = AvfxBinder {
                binder_type: 2,
                life: -1,
                properties_start: Some(crate::avfx::AvfxBinderProperties {
                    bind_point_type: start,
                    generate_delay: case["generateDelay"].as_i64().unwrap() as i32,
                    coord_update_frame: -1,
                    ..Default::default()
                }),
                properties_goal: Some(crate::avfx::AvfxBinderProperties {
                    bind_point_type: goal,
                    generate_delay: 91,
                    coord_update_frame: -1,
                    ..Default::default()
                }),
                properties_1: Some(crate::avfx::AvfxBinderProperties {
                    ring_enabled: mask & 1 != 0,
                    ring_position: [0.125, -0.25, 0.3],
                    ring_radius: 2.0,
                    ring_progress_time: 1,
                    ..Default::default()
                }),
                properties_2: Some(crate::avfx::AvfxBinderProperties {
                    ring_enabled: mask & 2 != 0,
                    ring_position: [-0.375, 0.5, 0.7],
                    ring_radius: 1.25,
                    ring_progress_time: 3,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let frame = VfxLinearBinderFrame {
                age: f32::from_bits(case["inputAge"].as_u64().unwrap() as u32),
                query_deadlines: [-1.0; 2],
                camera_position: [0.0; 3],
                camera: None,
                document_scale: [1.0; 3],
                self_targets: [start & 3 == 0, goal & 3 == 0],
                root_ags: false,
            };
            let mut random = VfxClientRandomState::from_words(std::array::from_fn(|i| {
                case["seed"][i].as_u64().unwrap() as u32
            }));
            let mut free = 8usize;
            let mut children = Vec::new();
            let mut queries = Vec::new();
            let mut child_allocations = usize::from(start & 3 == 0 && goal & 3 == 0);
            let mut draw_count = 0usize;
            let mut tls_getters = 0usize;
            // The capacity is an explicit allocator input to the numerical
            // comparison, not a production allocator or playback resource tree.
            for birth in super::super::point_factory::dual_target_births(&binder, count) {
                if free == 0 {
                    capacity_failures += 1;
                    continue;
                }
                free -= 1;
                let constructed = VfxSplineBinderState::construct_with_client_trig(
                    &binder,
                    frame,
                    birth.delay,
                    |e| {
                        let target = birth.target_indices[e];
                        queries.push([if e == 0 { 0 } else { 3 }, target]);
                        (success & (1 << e) != 0).then_some(VfxBinderTarget {
                            position: [
                                if target < 0 {
                                    100.0
                                } else {
                                    target as f32 * 10.0
                                } + if e == 1 { 3.0 } else { 0.0 },
                                0.0,
                                0.0,
                            ],
                            basis: VfxBinderMatrix::IDENTITY.basis,
                            scale: [1.0; 3],
                        })
                    },
                    |_, value| {
                        *value = 1.0;
                        true
                    },
                    || {
                        draw_count += 1;
                        tls_getters += 1;
                        random.next_u16()
                    },
                    mode,
                    || VfxLinearBinderCurves {
                        factor: 0.5,
                        start_position: [0.0; 3],
                        goal_position: [0.0; 3],
                        start_position_after: [0.0; 3],
                    },
                    VfxBinderMatrix::IDENTITY,
                );
                let wants_child = constructed
                    .initialization
                    .and_then(|i| i.child_direction)
                    .is_some();
                child_allocations += usize::from(wants_child);
                let child = wants_child && free != 0;
                if child {
                    free -= 1;
                    if owner == 0 {
                        tls_getters += 2;
                    }
                }
                let instance = constructed.into_instance(u16::from(child), mode);
                objects += 1;
                if retired {
                    // Native construction and RNG precede rejected Attach.
                    rejected_objects += 1;
                    free += 1 + usize::from(child);
                    continue;
                }
                let state = &instance.state;
                let lifecycle = instance.lifecycle;
                let clock = lifecycle.clock;
                let clocks = [
                    clock.local_age,
                    clock.total_age,
                    lifecycle.scaled_delta,
                    clock.previous_age,
                    clock.nominal_life,
                ]
                .map(f32::to_bits);
                let inner = child.then(|| json!({
                    "flags": 0x3f000000u32 | u32::from(owner == 1),
                    "clocks": ([clock.local_age, clock.local_age, 0.0, clock.local_age, 30.0].map(f32::to_bits)),
                    "life": [30.0f32.to_bits()],
                    "main": matrix_bits(if owner == 0 { VfxBinderMatrix::IDENTITY } else { state.common.matrix }),
                }));
                let node = json!({
                    "targets": birth.target_indices, "flags": lifecycle.raw_flags(),
                    "lifeEnabled": lifecycle.life_limit_enabled, "initialized": instance.initialized,
                    "clocks": clocks, "life": [clock.nominal_life.to_bits()], "delay": [clock.delay.to_bits()],
                    "main": matrix_bits(state.common.matrix), "auxiliary": matrix_bits(state.common.auxiliary_matrix),
                    "scale": state.common.scale.map(f32::to_bits),
                    "offsets": state.controls.into_iter().flat_map(|p| p.offset.map(f32::to_bits)).collect::<Vec<_>>(),
                    "parameters": state.controls.map(|p| p.parameter.to_bits()),
                    "knots": state.path.knots().iter().map(|p| json!({"parameter":p.parameter.to_bits(),"position":p.position.map(f32::to_bits)})).collect::<Vec<_>>(),
                    "inner": inner,
                });
                components += 5
                    + 1
                    + 1
                    + 12
                    + 12
                    + 3
                    + 6
                    + 2
                    + 4 * state.path.knots().len()
                    + usize::from(child) * 18;
                children.push(node);
            }
            draws += draw_count;
            let parent_flags = if retired {
                0x40001u32
            } else {
                0x3f000001u32 | ((children.len() as u32) << 9)
            };
            let actual = json!({
                "parentFlags": parent_flags, "freeCount": free,
                "countGetters": usize::from(start & 3 == 1 || start & 3 == 0 && goal & 3 == 1),
                "allocatorFunctionCalls": child_allocations, "tlsGetterCalls": tls_getters,
                "children": children, "randomState": random.words(), "queryTargets": queries,
            });
            assert_eq!(
                actual, case["afterFactory"],
                "original factory case {ordinal}"
            );
        }
        std::fs::write(folder.join("spline-target-factory-rust-comparison.json"), serde_json::to_vec_pretty(&json!({
            "cases": cases.len(), "constructedSplineObjects": objects,
            "rejectedParentObjects": rejected_objects, "capacitySkippedBirths": capacity_failures,
            "numericComponents": components, "randomDraws": draws, "differences": 0,
            "scope": "Production dual-target birth plans, numerical Spline constructors and explicit shared TLS-state recurrence versus original Scheduler/Item factories. Ordered targets, query order, random state, matrices, controls/knots, clocks/flags and child Common/Timeline birth snapshots compared; controlled capacity8 allocator input includes exhaustion and rejected-parent cleanup. Actual production playback tree, child resources/phases, other global RNG consumers, host and GPU remain unverified."
        })).unwrap()).unwrap();
    }

    fn compare_spline_construction(prefix: &str, native_trig: bool, own_lifecycle: bool) {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join(format!("{prefix}-client-probe.json"))).unwrap(),
        )
        .unwrap();
        let cases = input["cases"].as_array().unwrap();
        let f = |v: &serde_json::Value| f32::from_bits(v.as_u64().unwrap() as u32);
        let vec3 = |v: &serde_json::Value, i: usize| std::array::from_fn(|a| f(&v[i + a]));
        let mut finite = 0usize;
        let mut nonfinite = 0usize;
        let mut boundaries = 0usize;
        let mut random_draws = 0usize;
        let mut queries = 0usize;
        let mut listeners = 0usize;
        for (ordinal, case) in cases.iter().enumerate() {
            assert_eq!(case["absentProgressAfterOriginalCtorAndEmptyRead"], 0);
            let mode = if native_trig {
                match case["trigMode"].as_u64().unwrap() {
                    0 => VfxClientTrigMode::Sse2,
                    1 => VfxClientTrigMode::AvxFma,
                    other => panic!("unknown original CRT mode {other}"),
                }
            } else {
                VfxClientTrigMode::Sse2
            };
            let sample = case["sample"].as_u64().unwrap() as usize;
            let mask = case["mask"].as_u64().unwrap() as usize;
            let qmode = case["queryMode"].as_u64().unwrap() as usize;
            let inputs = &case["inputs"];
            let binder = AvfxBinder {
                binder_type: 2,
                life: case["life"].as_i64().unwrap() as i32,
                start_to_global_direction: sample % 4 != 0,
                vfx_scale_enabled: sample % 4 != 0,
                vfx_scale_depth_offset: sample % 4 != 0,
                vfx_scale_interpolation: sample % 4 != 0,
                transform_scale: (sample % 2) as i32,
                transform_scale_depth_offset: sample % 3 != 0,
                transform_scale_interpolation: sample % 3 != 0,
                following_target_orientation: sample % 3 != 0,
                bet: sample % 3 != 0,
                vfx_scale_bias: 0.5,
                rotation_type: (sample % 5) as i32,
                properties_start: Some(crate::avfx::AvfxBinderProperties {
                    coord_update_frame: if sample % 3 != 0 { 1 } else { -1 },
                    ..Default::default()
                }),
                properties_goal: Some(crate::avfx::AvfxBinderProperties {
                    coord_update_frame: if sample % 2 != 0 { 0 } else { -1 },
                    ..Default::default()
                }),
                properties_1: Some(crate::avfx::AvfxBinderProperties {
                    ring_enabled: mask & 1 != 0,
                    ring_position: [0.125, -0.25, 0.3],
                    ring_progress_time: case["progress"][0].as_i64().unwrap() as i32,
                    ring_radius: f(&case["radii"][0]),
                    ..Default::default()
                }),
                properties_2: Some(crate::avfx::AvfxBinderProperties {
                    ring_enabled: mask & 2 != 0,
                    ring_position: [-0.375, 0.5, 0.7],
                    ring_progress_time: case["progress"][1].as_i64().unwrap() as i32,
                    ring_radius: f(&case["radii"][1]),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let frame = VfxLinearBinderFrame {
                age: f(&case["inputAge"]),
                query_deadlines: VfxLinearBinderFrame::authored_query_deadlines(&binder),
                camera_position: vec3(&inputs["camera"], 15),
                camera: Some(super::super::VfxBinderCameraSnapshot {
                    basis: std::array::from_fn(|c| vec3(&inputs["camera"], c * 3)),
                    parallel_direction: vec3(&inputs["camera"], 12),
                    position: vec3(&inputs["camera"], 15),
                }),
                document_scale: vec3(&inputs["documentScale"], 0),
                self_targets: [true, false],
                root_ags: case["ags"].as_bool().unwrap(),
            };
            let rotation = VfxBinderMatrix {
                basis: std::array::from_fn(|c| vec3(&inputs["rotationMatrix"], c * 3)),
                position: [0.0; 3],
            };
            let root_revision =
                rotation.transform_matrix(VfxBinderMatrix::scale_matrix(frame.document_scale));
            let mut state = VfxSplineBinderState::constructed(&binder, frame.document_scale);
            let birth_clock = VfxBinderBirthClock::new(frame.age, binder.life, f(&case["delay"]));
            let mut instance: Option<VfxSplineBinderInstance> = None;
            let mut random_state =
                super::super::VfxClientRandomState::from_words(std::array::from_fn(|i| {
                    inputs["seed"][i].as_u64().unwrap() as u32
                }));
            let mut snapshots = vec![&case["afterConstructor"]];
            if own_lifecycle {
                snapshots.extend(case["updates"].as_array().unwrap());
            } else {
                snapshots.push(&case["manualInitialization"]);
            }
            for (phase, snapshot) in snapshots.into_iter().enumerate() {
                if snapshot.is_null() {
                    continue;
                }
                let frame = if phase == 0 || own_lifecycle {
                    frame
                } else {
                    VfxLinearBinderFrame { age: 3.0, ..frame }
                };
                let mut target_calls = [0usize; 2];
                let mut scale_calls = [0usize; 2];
                let mut draw_calls = 0usize;
                let mut draw_values = Vec::new();
                let mut angular_args = Vec::new();
                let mut curve_calls = 0;
                let mut target = |e: usize| {
                    target_calls[e] += 1;
                    (qmode != e || (own_lifecycle && phase >= 2)).then(|| {
                        VfxBinderTarget::from_matrix(
                            std::array::from_fn(|i| f(&inputs["targets"][e * 16 + i])),
                            binder.following_target_orientation,
                        )
                    })
                };
                let mut listener = |e: usize, value: &mut f32| {
                    scale_calls[e] += 1;
                    if sample % 2 != 0 || qmode != 2 {
                        *value = if qmode == 3 { 0.0 } else { 2.0 + e as f32 };
                    }
                    qmode != 2
                };
                let mut random = || {
                    draw_calls += 1;
                    let draw = random_state.next_u16();
                    draw_values.push(draw);
                    draw
                };
                let mut angular = |angle: f32| {
                    angular_args.extend([angle, angle]);
                    [angle * 0.125 + 0.5, angle * -0.25 - 0.75]
                };
                let mut curves = || {
                    curve_calls += 1;
                    VfxLinearBinderCurves {
                        factor: f(&inputs["factor"][0]),
                        start_position: vec3(&inputs["positions"], 0),
                        goal_position: vec3(&inputs["positions"], 3),
                        start_position_after: vec3(&inputs["positions"], 6),
                    }
                };
                let initialization;
                let mut child_birth = None;
                if phase == 0 {
                    let constructed = if native_trig {
                        VfxSplineBinderState::construct_with_client_trig(
                            &binder,
                            frame,
                            birth_clock.delay,
                            &mut target,
                            &mut listener,
                            &mut random,
                            mode,
                            &mut curves,
                            root_revision,
                        )
                    } else {
                        VfxSplineBinderState::construct(
                            &binder,
                            frame,
                            birth_clock.delay,
                            &mut target,
                            &mut listener,
                            &mut random,
                            &mut angular,
                            &mut curves,
                            root_revision,
                        )
                    };
                    initialization = constructed.initialization;
                    if own_lifecycle {
                        let children =
                            u16::from(initialization.and_then(|i| i.child_direction).is_some());
                        if children != 0 {
                            child_birth = Some(constructed.clock);
                        }
                        instance = Some(constructed.into_instance(children, mode));
                        state = instance.as_ref().unwrap().state.clone();
                    } else {
                        state = constructed.state;
                    }
                } else if own_lifecycle {
                    let instance = instance.as_mut().unwrap();
                    if phase == 1 {
                        instance.lifecycle.clock.rate = f(&case["inputRate"]);
                    }
                    let mut attach =
                        |_: &VfxSplineBinderState,
                         _: &VfxLinearBinderInitialization,
                         clock: VfxBinderBirthClock| {
                            child_birth = Some(clock);
                            true
                        };
                    let mut age_curves = |_: f32, _: f32| curves();
                    let result = if case["ownMode"] == 1 {
                        instance.advance_self(
                            &binder,
                            [2.5, 0.0, 3.0, -1.0][phase - 1],
                            frame,
                            &mut target,
                            &mut listener,
                            &mut random,
                            &mut age_curves,
                            root_revision,
                            &mut attach,
                        )
                    } else {
                        instance.advance_time([2.5, 0.0, 3.0, -1.0][phase - 1]);
                        instance.prepare_self(
                            &binder,
                            frame,
                            &mut target,
                            &mut listener,
                            &mut random,
                            &mut age_curves,
                            root_revision,
                            &mut attach,
                        )
                    };
                    initialization = result.initialization;
                    state = instance.state.clone();
                } else {
                    initialization = Some(if native_trig {
                        state.initialize_with_client_trig(
                            &binder,
                            frame,
                            birth_clock.nominal_life,
                            &mut target,
                            &mut listener,
                            &mut random,
                            mode,
                            &mut curves,
                            root_revision,
                        )
                    } else {
                        state.initialize(
                            &binder,
                            frame,
                            birth_clock.nominal_life,
                            &mut target,
                            &mut listener,
                            &mut random,
                            &mut angular,
                            &mut curves,
                            root_revision,
                        )
                    });
                }
                if native_trig {
                    for draw in draw_values.iter().skip(2) {
                        let angle = f32::from(*draw) * std::f32::consts::TAU / 65535.0
                            - std::f32::consts::PI;
                        angular_args.extend([angle, angle]);
                    }
                }
                let clock = instance.as_ref().map_or(birth_clock, |i| i.lifecycle.clock);
                let should_init = if own_lifecycle {
                    instance.as_ref().unwrap().initialized
                } else {
                    phase != 0 || birth_clock.initializes_immediately()
                };
                if !own_lifecycle {
                    assert_eq!(
                        initialization.is_some(),
                        should_init,
                        "case {ordinal} phase {phase}"
                    );
                }
                for e in 0..2 {
                    assert_eq!(
                        target_calls[e],
                        snapshot["targetCalls"][e].as_u64().unwrap() as usize,
                        "case {ordinal} query {e}"
                    );
                    assert_eq!(
                        scale_calls[e],
                        snapshot["scaleCalls"][e].as_u64().unwrap() as usize,
                        "case {ordinal} scale {e}"
                    );
                }
                let succeeded = initialization.and_then(|i| i.child_direction).is_some();
                assert_eq!(snapshot["initialized"], should_init);
                if own_lifecycle {
                    let lifecycle = instance.as_ref().unwrap().lifecycle;
                    assert_eq!(snapshot["lifeEnabled"], lifecycle.life_limit_enabled);
                    assert_eq!(
                        snapshot["flags"],
                        lifecycle.raw_flags(),
                        "case {ordinal} phase {phase} flags"
                    );
                    assert_eq!(
                        snapshot["registeredChildren"],
                        lifecycle.registered_children()
                    );
                } else {
                    assert_eq!(snapshot["lifeEnabled"], should_init);
                    assert_eq!(
                        snapshot["flags"].as_u64().unwrap() & 0x1ff,
                        u64::from(!should_init)
                    );
                }
                assert_eq!(snapshot["allocationCalls"], usize::from(succeeded));
                assert_eq!(snapshot["directionCalls"], usize::from(succeeded));
                assert_eq!(snapshot["trigCalls"], angular_args.len());
                assert_eq!(snapshot["cofCalls"], curve_calls);
                assert_eq!(snapshot["positionCalls"], curve_calls * 3);
                // Update also calls the TLS getter for BET, separately from four
                // real inline random advances. Its full TLS call count is not RNG.
                assert_eq!(draw_calls, usize::from(succeeded) * 4);
                assert_eq!(
                    random_state.words(),
                    std::array::from_fn(|i| snapshot["randomState"][i].as_u64().unwrap() as u32)
                );
                let values = |m: VfxBinderMatrix| m.basis.into_iter().flatten().chain(m.position);
                let c = &state.common;
                let mut actual: Vec<f32> = values(c.matrix)
                    .chain(values(c.auxiliary_matrix))
                    .chain(c.scale)
                    .chain(c.targets.into_iter().flat_map(values))
                    .chain(c.start_property_world)
                    .chain(c.endpoint_vfx_scales)
                    .chain(c.endpoint_depth_scales)
                    .chain([c.vfx_scale, c.transform_depth_scale])
                    .chain(state.controls.into_iter().flat_map(|p| p.offset))
                    .chain(state.controls.map(|p| p.parameter))
                    .collect();
                let mut expected: Vec<f32> = [
                    "main",
                    "auxiliary",
                    "scale",
                    "targets",
                    "propertyWorld",
                    "vfx",
                    "depth",
                    "combined",
                    "offsets",
                    "parameters",
                ]
                .into_iter()
                .flat_map(|k| snapshot[k].as_array().unwrap())
                .map(f)
                .collect();
                let knots = snapshot["knots"].as_array().unwrap();
                assert_eq!(state.path.knots().len(), knots.len());
                for (k, original) in state.path.knots().iter().zip(knots) {
                    actual.extend(std::iter::once(k.parameter).chain(k.position));
                    expected.push(f(&original["parameter"]));
                    expected.extend(original["position"].as_array().unwrap().iter().map(f));
                }
                actual.extend(
                    initialization
                        .and_then(|i| i.child_direction)
                        .unwrap_or([0.0; 3]),
                );
                expected.extend(snapshot["direction"].as_array().unwrap().iter().map(f));
                angular_args.resize(4, 0.0);
                actual.extend(angular_args);
                expected.extend(snapshot["angularArgs"].as_array().unwrap().iter().map(f));
                if phase == 0 {
                    actual.extend([
                        clock.local_age,
                        clock.total_age,
                        clock.previous_age,
                        clock.nominal_life,
                        clock.delay,
                    ]);
                    expected.extend([
                        f(&snapshot["clocks"][0]),
                        f(&snapshot["clocks"][1]),
                        f(&snapshot["clocks"][3]),
                        f(&snapshot["life"][0]),
                        f(&snapshot["delay"][0]),
                    ]);
                }
                if own_lifecycle {
                    let lc = instance.as_ref().unwrap().lifecycle;
                    if phase != 0 {
                        actual.extend([
                            clock.local_age,
                            clock.total_age,
                            clock.previous_age,
                            clock.nominal_life,
                            clock.delay,
                        ]);
                        expected.extend([
                            f(&snapshot["clocks"][0]),
                            f(&snapshot["clocks"][1]),
                            f(&snapshot["clocks"][3]),
                            f(&snapshot["life"][0]),
                            f(&snapshot["delay"][0]),
                        ]);
                    }
                    actual.extend([clock.rate, lc.scaled_delta]);
                    expected.extend([f(&snapshot["rate"][0]), f(&snapshot["clocks"][2])]);
                    if let Some(birth) = child_birth {
                        assert!(
                            !snapshot["childBirth"].is_null(),
                            "case {ordinal} phase {phase} child attachment"
                        );
                        actual.extend([birth.local_age, birth.local_age, birth.local_age]);
                        expected.extend([
                            f(&snapshot["childBirth"]["age"]),
                            f(&snapshot["childBirth"]["total"]),
                            f(&snapshot["childBirth"]["previous"]),
                        ]);
                    }
                }
                assert_eq!(actual.len(), expected.len());
                for (component, (actual, expected)) in actual.into_iter().zip(expected).enumerate()
                {
                    if expected.is_nan() {
                        assert!(
                            actual.is_nan(),
                            "case {ordinal} phase {phase} component {component}: {actual} != NaN"
                        );
                        nonfinite += 1;
                    } else {
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "case {ordinal} phase {phase} component {component}: {actual} != {expected}"
                        );
                        if expected.is_finite() {
                            finite += 1;
                        } else {
                            nonfinite += 1;
                        }
                    }
                }
                boundaries += 1;
                random_draws += draw_calls;
                queries += target_calls.into_iter().sum::<usize>();
                listeners += scale_calls.into_iter().sum::<usize>();
            }
        }
        std::fs::write(folder.join(format!("{prefix}-rust-comparison.json")), serde_json::to_string_pretty(&serde_json::json!({
            "scenes":cases.len(),"boundaries":boundaries,"finiteComponentsCompared":finite,"nonfiniteComponentsCompared":nonfinite,
            "randomDrawsCompared":random_draws,"targetQueriesCompared":queries,"listenerCallbacksCompared":listeners,"differences":0,
            "realClientTrig":native_trig,"ownLifecycleCompared":own_lifecycle,
            "successfulOriginalChildConstruction":own_lifecycle,
            "scope":if own_lifecycle {
                "Original Spline constructors, real CRT/inline RNG and normal Time/Prepare or prewarm Common+60 against production persistent own instance. Actual pool allocation, child Common/Timeline construction and Attach succeed; count and birth clocks compared. Child phase callbacks/Emitter resources, Euler, curves/target/listener/Document/TLS/global unit scale controlled. No production owned Scheduler/Item factory or complete descendant resources/live host/GPU. Finite/Inf bitwise; NaN classification."
            } else if native_trig {
                "Both original Spline constructors and original SSE2/AVX-FMA cosf/sinf, Common storage, initialize/shared query/update/path, inline TLS xorshift and birth direction vs production client-trig entrypoints. Explicit dispatch/MXCSR; no Time/Prepare/actual owned factories/resources. Child allocation fails. Euler, curves/target/listener/Document/TLS controlled. Finite/Inf bitwise; NaN classification."
            } else { "Both original Spline constructors, real Common storage, initialize/shared query/update/path, inline TLS xorshift draws and birth direction vs numeric production construction. Constructors and explicit delayed initialization compared; no Time/Prepare or actual owned factories/resources. Child allocation deliberately fails. CRT angular returns, Euler, resolved curves/target/listener/Document/TLS controlled. Finite/Inf bitwise; NaN classification." }
        })).unwrap()+"\n").unwrap();
    }

    #[test]
    fn spline_update_uses_goal_up_and_lookat_origin_for_control_without_pos_curve_offset() {
        let binder = AvfxBinder {
            binder_type: 2,
            transform_scale: 1,
            ..Default::default()
        };
        let mut state = VfxSplineBinderState::from_cache(
            VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY),
            controls(true),
        );
        let statuses = state.update_frame(
            &binder,
            frame(),
            |endpoint| {
                Some(VfxBinderTarget {
                    basis: VfxBinderMatrix::IDENTITY.basis,
                    scale: [1.0; 3],
                    position: [0.0, 0.0, endpoint as f32 * 4.0],
                })
            },
            |_, _| unreachable!(),
            curves(),
        );
        assert_eq!(statuses, [VfxBinderQueryStatus::Refreshed; 2]);
        assert_eq!(state.path.knots()[1].position, [1.0, 0.0, 0.0]);
        let p = state.common.matrix.position;
        assert!((p[0] - 22.0 / 27.0).abs() < 1e-6);
        assert!((p[2] - 32.0 / 27.0).abs() < 1e-6);
        assert_eq!(state.common.start_property_world, [70.0, 80.0, 90.0]);
    }

    #[test]
    fn spline_update_failed_targets_keep_path_sources_and_reset_depth_then_skip_after_cutoff() {
        let binder = AvfxBinder {
            binder_type: 2,
            vfx_scale_enabled: true,
            vfx_scale_depth_offset: true,
            vfx_scale_interpolation: true,
            transform_scale_depth_offset: true,
            transform_scale_interpolation: true,
            ..Default::default()
        };
        let mut common = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        common.targets[1].position = [0.0, 0.0, 4.0];
        common.endpoint_vfx_scales = [2.0, 3.0];
        common.endpoint_depth_scales = [4.0, 5.0];
        let mut state = VfxSplineBinderState::from_cache(common, controls(false));
        let statuses = state.update_frame(
            &binder,
            frame(),
            |_| None,
            |_, _| unreachable!("unavailable targets never call the scale listener"),
            curves(),
        );
        assert_eq!(statuses, [VfxBinderQueryStatus::TargetUnavailable; 2]);
        assert_eq!(state.common.endpoint_vfx_scales, [2.0, 3.0]);
        assert_eq!(state.common.endpoint_depth_scales, [1.0; 2]);
        assert_eq!(state.common.matrix.position, [0.0, 0.0, 2.0]);
        assert_eq!(state.depth_offset_multiplier(&binder), 2.5);
        let statuses = state.update_frame(
            &binder,
            VfxLinearBinderFrame {
                age: 3.0,
                query_deadlines: [2.0; 2],
                ..frame()
            },
            |_| unreachable!(),
            |_, _| unreachable!(),
            VfxLinearBinderCurves {
                factor: 0.75,
                ..curves()
            },
        );
        assert_eq!(statuses, [VfxBinderQueryStatus::Skipped; 2]);
        assert_eq!(state.common.matrix.position, [0.0, 0.0, 3.0]);
        assert_eq!(state.common.endpoint_depth_scales, [1.0; 2]);
    }

    #[test]
    fn spline_update_degenerate_goal_up_uses_goal_z_control_frame_fallback() {
        let binder = AvfxBinder {
            binder_type: 2,
            ..Default::default()
        };
        let mut state = VfxSplineBinderState::from_cache(
            VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY),
            controls(true),
        );
        state.update_frame(
            &binder,
            frame(),
            |endpoint| {
                Some(VfxBinderTarget {
                    basis: VfxBinderMatrix::IDENTITY.basis,
                    scale: [1.0; 3],
                    position: [0.0, endpoint as f32 * 4.0, 0.0],
                })
            },
            |_, _| unreachable!(),
            curves(),
        );
        assert_eq!(state.path.knots()[1].position, [1.0, 4.0, -1.0]);
    }

    #[test]
    #[ignore = "compare prepared Spline update caches with original shared query and path functions"]
    fn compare_original_spline_binder_update_with_cached_state() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("spline-binder-update-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = input["cases"].as_array().unwrap();
        let f = |value: &serde_json::Value| f32::from_bits(value.as_u64().unwrap() as u32);
        let vec3 =
            |value: &serde_json::Value, index: usize| std::array::from_fn(|i| f(&value[index + i]));
        let mut state = None;
        let mut finite = 0usize;
        let mut nonfinite = 0usize;
        let mut queries = 0usize;
        let mut listeners = 0usize;
        for (ordinal, case) in cases.iter().enumerate() {
            let sample = case["sample"].as_u64().unwrap() as usize;
            let flags = case["flags"].as_u64().unwrap();
            let mask = case["mask"].as_u64().unwrap();
            let binder = AvfxBinder {
                binder_type: 2,
                start_to_global_direction: flags & 1 != 0,
                following_target_orientation: flags & 2 != 0,
                vfx_scale_enabled: flags & 4 != 0,
                vfx_scale_depth_offset: flags & 8 != 0,
                vfx_scale_interpolation: flags & 16 != 0,
                transform_scale_depth_offset: flags & 32 != 0,
                transform_scale_interpolation: flags & 64 != 0,
                bet: flags & 128 != 0,
                transform_scale: (sample % 3) as i32,
                vfx_scale_bias: (sample % 4) as f32 * 0.5,
                rotation_type: case["rotation"].as_i64().unwrap() as i32,
                ..Default::default()
            };
            if case["step"] == 0 {
                let mut common = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
                common.auxiliary_matrix = VfxBinderMatrix {
                    basis: [[2.0, 0.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, -4.0]],
                    position: [0.25, -0.5, 0.75],
                };
                common.scale = [9.0, 8.0, 7.0];
                let caches = [0.0, -0.0, -2.0, f32::NAN];
                common.endpoint_vfx_scales = [caches[sample % 4], caches[(sample + 1) % 4]];
                common.endpoint_depth_scales = [2.0, 3.0];
                state = Some(VfxSplineBinderState::from_cache(
                    common,
                    std::array::from_fn(|i| VfxSplineBinderControl {
                        enabled: mask & (1 << i) != 0,
                        parameter: f(&case["parameters"][i]),
                        position: vec3(&case["controlPositions"], i * 3),
                        offset: vec3(&case["controlOffsets"], i * 3),
                    }),
                ));
            }
            let state = state.as_mut().unwrap();
            let frame = VfxLinearBinderFrame {
                age: f(&case["age"]),
                query_deadlines: std::array::from_fn(|i| f(&case["deadlines"][i])),
                camera_position: vec3(&case["camera"], 15),
                camera: Some(super::super::VfxBinderCameraSnapshot {
                    basis: std::array::from_fn(|column| vec3(&case["camera"], column * 3)),
                    parallel_direction: vec3(&case["camera"], 12),
                    position: vec3(&case["camera"], 15),
                }),
                document_scale: vec3(&case["documentScale"], 0),
                self_targets: std::array::from_fn(|i| case["selfTargets"][i] == 1),
                root_ags: sample % 2 != 0,
            };
            let mut target_calls = [0usize; 2];
            let mut listener_calls = [0usize; 2];
            state.update_frame(
                &binder,
                frame,
                |endpoint| {
                    target_calls[endpoint] += 1;
                    (case["targetOk"][endpoint] == 1).then(|| {
                        VfxBinderTarget::from_matrix(
                            std::array::from_fn(|i| f(&case["inputTargets"][endpoint * 16 + i])),
                            binder.following_target_orientation,
                        )
                    })
                },
                |endpoint, value| {
                    listener_calls[endpoint] += 1;
                    if case["writeScale"][endpoint] == 1 {
                        *value = f(&case["listener"][endpoint]);
                    }
                    case["scaleOk"][endpoint] == 1
                },
                VfxLinearBinderCurves {
                    factor: f(&case["factor"]),
                    start_position: vec3(&case["positions"], 0),
                    goal_position: vec3(&case["positions"], 3),
                    start_position_after: vec3(&case["positions"], 6),
                },
            );
            for endpoint in 0..2 {
                assert_eq!(
                    target_calls[endpoint],
                    case["targetCalls"][endpoint].as_u64().unwrap() as usize,
                    "case {ordinal} endpoint {endpoint}"
                );
                assert_eq!(
                    listener_calls[endpoint],
                    case["scaleCalls"][endpoint].as_u64().unwrap() as usize,
                    "case {ordinal} endpoint {endpoint}"
                );
            }
            queries += target_calls.into_iter().sum::<usize>();
            listeners += listener_calls.into_iter().sum::<usize>();
            assert_eq!(case["positionCalls"], 3);
            assert_eq!(case["factorCalls"], 1);
            let matrix_values =
                |matrix: VfxBinderMatrix| matrix.basis.into_iter().flatten().chain(matrix.position);
            let c = &state.common;
            let actual: Vec<f32> = matrix_values(c.matrix)
                .chain(matrix_values(c.auxiliary_matrix))
                .chain(c.targets.into_iter().flat_map(matrix_values))
                .chain(c.scale)
                .chain(c.start_property_world)
                .chain(c.endpoint_vfx_scales)
                .chain(c.endpoint_depth_scales)
                .chain([
                    c.vfx_scale,
                    c.transform_depth_scale,
                    state.depth_offset_multiplier(&binder),
                ])
                .collect();
            let expected: Vec<_> = [
                "main",
                "auxiliary",
                "targets",
                "scale",
                "propertyWorld",
                "endpointVfx",
                "endpointDepth",
                "combined",
            ]
            .into_iter()
            .flat_map(|name| case[name].as_array().unwrap())
            .chain([&case["depthMultiplier"]])
            .map(f)
            .collect();
            assert_eq!(actual.len(), expected.len());
            let knot_values = state
                .path
                .knots()
                .iter()
                .flat_map(|knot| std::iter::once(knot.parameter).chain(knot.position));
            let original_knots = case["knots"].as_array().unwrap();
            assert_eq!(state.path.knots().len(), original_knots.len());
            for (component, (actual, expected)) in actual
                .into_iter()
                .zip(expected)
                .chain(
                    knot_values.zip(
                        original_knots
                            .iter()
                            .flat_map(|values| values.as_array().unwrap())
                            .map(f),
                    ),
                )
                .enumerate()
            {
                if expected.is_nan() {
                    assert!(
                        actual.is_nan(),
                        "case {ordinal} component {component}: {actual} != NaN"
                    );
                    nonfinite += 1;
                } else {
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "case {ordinal} component {component}: {actual} != {expected}; sample {sample} flags {flags} mask {mask}"
                    );
                    if expected.is_finite() {
                        finite += 1;
                    } else {
                        nonfinite += 1;
                    }
                }
            }
        }
        std::fs::write(folder.join("spline-binder-update-rust-comparison.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "scenes":cases.len()/4,"updates":cases.len(),"finiteComponentsCompared":finite,
                "nonfiniteComponentsCompared":nonfinite,"targetQueriesCompared":queries,
                "listenerCallbacksCompared":listeners,"differences":0,
                "scope":"Original Spline update/shared query/basis/depth/position/path and getters vs prepared production numeric state. Finite/Inf bitwise, NaN classification. Controlled constructor controls/offsets, curves/listener/targets/Document/TLS; no original constructors, complete owned playback/resources/live host/RNG/CRT trig/GPU."
            })).unwrap()+"\n").unwrap();
    }

    #[test]
    fn spline_path_uses_nonuniform_neighbor_tangents_and_endpoint_clamps() {
        let path = VfxSplineBinderPath::from_knots(&[
            VfxSplineBinderKnot {
                parameter: 0.0,
                position: [0.0; 3],
            },
            VfxSplineBinderKnot {
                parameter: 0.25,
                position: [1.0, -1.0, 2.0],
            },
            VfxSplineBinderKnot {
                parameter: 1.0,
                position: [0.0; 3],
            },
        ])
        .unwrap();
        assert_eq!(path.sample(-1.0), [0.0; 3]);
        assert_eq!(path.sample(0.25), [1.0, -1.0, 2.0]);
        // This overshoots the straight-segment midpoint because the interior
        // tangent spans the nonuniform neighbors. Linear interpolation is .5.
        assert_eq!(path.sample(0.125), [0.625, -0.625, 1.25]);
        assert_eq!(path.sample(2.0), [0.0; 3]);
        assert!(path.sample(f32::NAN).into_iter().all(f32::is_nan));
    }

    #[test]
    fn spline_path_preserves_control_order_and_rejects_impossible_cache_counts() {
        let knots = [
            VfxSplineBinderKnot {
                parameter: 0.0,
                position: [0.0; 3],
            },
            VfxSplineBinderKnot {
                parameter: 0.75,
                position: [1.0; 3],
            },
            VfxSplineBinderKnot {
                parameter: 0.25,
                position: [2.0; 3],
            },
            VfxSplineBinderKnot {
                parameter: 1.0,
                position: [3.0; 3],
            },
        ];
        assert_eq!(
            VfxSplineBinderPath::from_knots(&knots).unwrap().knots(),
            knots
        );
        assert!(VfxSplineBinderPath::from_knots(&[]).is_err());
        assert!(VfxSplineBinderPath::from_knots(&knots[..1]).is_err());
        assert!(VfxSplineBinderPath::from_knots(&[knots[0]; 5]).is_err());
    }

    #[test]
    #[ignore = "executes production Spline cache sampler against installed original CPU outputs"]
    fn compare_original_spline_binder_path_with_cached_knots() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("spline-binder-path-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = input["cases"].as_array().unwrap();
        let f = |value: &serde_json::Value| f32::from_bits(value.as_u64().unwrap() as u32);
        let mut finite = 0usize;
        let mut nonfinite = 0usize;
        for (ordinal, case) in cases.iter().enumerate() {
            let count = case["count"].as_u64().unwrap() as usize;
            let knots: Vec<_> = (0..count)
                .map(|index| VfxSplineBinderKnot {
                    parameter: f(&case["times"][index]),
                    position: std::array::from_fn(|axis| f(&case["positions"][index][axis])),
                })
                .collect();
            let path = VfxSplineBinderPath::from_knots(&knots).unwrap();
            let actual = path.sample(f(&case["factor"]));
            for axis in 0..3 {
                let expected = f(&case["actual"][axis]);
                if expected.is_nan() {
                    assert!(actual[axis].is_nan(), "case {ordinal}, axis {axis}: {case}");
                    nonfinite += 1;
                } else {
                    assert_eq!(
                        actual[axis].to_bits(),
                        expected.to_bits(),
                        "case {ordinal}, axis {axis}: {case}"
                    );
                    if expected.is_finite() {
                        finite += 1;
                    } else {
                        nonfinite += 1;
                    }
                }
            }
        }
        std::fs::write(folder.join("spline-binder-path-rust-comparison.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "cases":cases.len(), "finiteAxisOutputsCompared":finite,
                "nonfiniteAxisOutputsCompared":nonfinite, "differences":0,
                "scope":"Original 3c3750 cache sampler vs production path; controlled ordered knots/COF, no target/control producer, constructors, owned playback, RNG or GPU/client pixels. Finite and infinity bitwise; NaN classification only."
            })).unwrap()+"\n").unwrap();
    }
}
