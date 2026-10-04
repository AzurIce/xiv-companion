use super::VFX_IDENTITY_BASIS;
use crate::avfx::AvfxBinder;

/// Independent outputs of the target query, before Binder-specific update,
/// Document scale, screen adjustment and depth interpolation.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderQueryScale {
    pub target_scale: [f32; 3],
    pub vfx_scale: f32,
}

impl VfxBinderQueryScale {
    /// Point/Linear's +0x138 getter (0x1403bfc40), using cached query outputs.
    /// Transform/VFX bias, bBET, bTSc and Document scale do not enter this
    /// getter. The query state machine owns partial writes and deadline reuse.
    pub fn depth_offset_multiplier(&self, binder: &AvfxBinder, transform_depth_scale: f32) -> f32 {
        let mut factor = if binder.vfx_scale_depth_offset {
            self.vfx_scale
        } else {
            1.0
        };
        if binder.transform_scale_depth_offset {
            factor *= transform_depth_scale;
        }
        factor
    }
}

/// Column-major affine matrix used by Binder updates, separate from their
/// independent scale getter. Shear and zero/mirrored columns are retained.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderMatrix {
    pub basis: [[f32; 3]; 3],
    pub position: [f32; 3],
}

/// Resolved ElementId target in Document/camera coordinate space. Full affine
/// columns retain shear/mirrors; bone animation and object identity are external.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderTargetSnapshot {
    pub id: u32,
    pub matrix: VfxBinderMatrix,
}

/// One entry of the host's ordered target-object list. ElementIds are local to
/// this object; a missing object or ElementId is a query failure. List ordinals
/// are retained by existing Point/Linear objects across subsequent snapshots.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderTargetObjectSnapshot {
    pub object: Option<VfxBinderObjectSnapshot>,
    pub targets: Vec<VfxBinderTargetSnapshot>,
    pub vfx_scale: f32,
}

/// Resolved Character-listener object fields for BTPT=0, in the same space as
/// the Document and ElementId targets. This source has no ElementId or skeleton
/// dependency. Quaternion components are raw [x, y, z, w], not normalized.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderObjectSnapshot {
    pub position: [f32; 3],
    pub quaternion: [f32; 4],
    pub scale: [f32; 3],
}

impl VfxBinderObjectSnapshot {
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        quaternion: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };

    /// Original Character query 0x14042fad0, BTPT=0 branch +0x27a, using
    /// object +0x50/+0x60/+0x70 and quaternion helper 0x14033a310. Retains
    /// the helper's f32 multiplication/addition order, including zero terms.
    /// Numerical probes may supply nonfinite fields; host inputs use validate.
    pub fn matrix(self) -> VfxBinderMatrix {
        let [x, y, z, w] = self.quaternion;
        let dx = x + x;
        let dy = y + y;
        let dz = z + z;
        let xx = x * dx;
        let yy = y * dy;
        let zz = z * dz;
        let wx = w * dx;
        let wy = w * dy;
        let wz = w * dz;
        let rotation = [
            [(1.0 - yy) - zz, y * dx + wz, z * dx - wy],
            [x * dy - wz, (1.0 - xx) - zz, z * dy + wx],
            [wy + z * dx, z * dy - wx, (1.0 - xx) - yy],
        ];
        let rotate = |input: [f32; 3]| {
            std::array::from_fn(|row| {
                (input[1] * rotation[1][row] + input[0] * rotation[0][row])
                    + input[2] * rotation[2][row]
            })
        };
        let translation = rotate([0.0; 3]);
        VfxBinderMatrix {
            basis: VfxBinderMatrix::scale_matrix(self.scale).basis.map(rotate),
            position: [
                self.position[0] + translation[0],
                self.position[1] + translation[1],
                translation[2] + self.position[2],
            ],
        }
    }

    pub fn validate(self) -> Result<(), String> {
        let matrix = self.matrix();
        if !self
            .position
            .into_iter()
            .chain(self.quaternion)
            .chain(self.scale)
            .chain(matrix.basis.into_iter().flatten())
            .chain(matrix.position)
            .all(f32::is_finite)
        {
            return Err("Binder object fields and resulting matrix must be finite".into());
        }
        Ok(())
    }
}

impl VfxBinderMatrix {
    pub const IDENTITY: Self = Self {
        basis: VFX_IDENTITY_BASIS,
        position: [0.0; 3],
    };

    /// Root revision consumed by Point bAGS (0x14037b700/0x14037bce0).
    /// Unlike ordinary particle Euler matrices, this rotation is transposed.
    /// `angles` are radians; Document scale is independent of query scale.
    pub fn root_revision(angles: [f32; 3], scale: [f32; 3], position: [f32; 3]) -> Self {
        Self::root_revision_from_half_angle_trig(
            angles.map(|angle| [(angle * 0.5).sin(), (angle * 0.5).cos()]),
            scale,
            position,
        )
    }

    /// Same matrix core with resolved [sin, cos] for each half angle. This
    /// isolates CRT trigonometry from the native f32 quaternion/matrix order.
    pub fn root_revision_from_half_angle_trig(
        trig: [[f32; 2]; 3],
        scale: [f32; 3],
        position: [f32; 3],
    ) -> Self {
        let [[sx, cx], [sy, cy], [sz, cz]] = trig;
        let x = (cy * sx) * cz - (sy * cx) * sz;
        let y = (sy * cx) * cz + (cy * sx) * sz;
        let z = (cy * cx) * sz - (sy * sx) * cz;
        let w = (sy * sx) * sz + (cy * cx) * cz;
        let twice = |value: f32| value + value;
        let rotation = Self {
            basis: [
                [
                    1.0 - twice(z * z + y * y),
                    twice(y * x - w * z),
                    twice(w * y + z * x),
                ],
                [
                    twice(w * z + y * x),
                    1.0 - twice(z * z + x * x),
                    twice(z * y - w * x),
                ],
                [
                    twice(z * x - w * y),
                    twice(w * x + z * y),
                    1.0 - twice(y * y + x * x),
                ],
            ],
            position: [0.0; 3],
        };
        // The original 3x3 multiply evaluates all three products, including
        // zero terms. Preserve their rounding/nonfinite behavior, then copy Pos.
        let mut result = rotation.transform_matrix(Self::scale_matrix(scale));
        result.position = position;
        result
    }

    pub(super) fn scale_matrix(scale: [f32; 3]) -> Self {
        Self {
            basis: [
                [scale[0], 0.0, 0.0],
                [0.0, scale[1], 0.0],
                [0.0, 0.0, scale[2]],
            ],
            position: [0.0; 3],
        }
    }

    pub(super) fn transform_point(self, position: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|row| {
            ((position[0] * self.basis[0][row] + position[1] * self.basis[1][row])
                + position[2] * self.basis[2][row])
                + self.position[row]
        })
    }

    /// Apply this affine matrix after `local` (column vectors). This is the
    /// client's 0x14037c5a0 operation used by the separate +0x30 getter;
    /// it preserves shear, reflections and the native f32 addition order.
    pub fn transform_matrix(self, local: Self) -> Self {
        Self {
            basis: std::array::from_fn(|column| {
                std::array::from_fn(|row| {
                    let x = local.basis[column][0] * self.basis[0][row];
                    let y = local.basis[column][1] * self.basis[1][row];
                    let z = local.basis[column][2] * self.basis[2][row];
                    if column == 0 && row == 0 {
                        (x + y) + z
                    } else {
                        (y + x) + z
                    }
                })
            }),
            position: std::array::from_fn(|row| {
                ((local.position[0] * self.basis[0][row] + local.position[1] * self.basis[1][row])
                    + local.position[2] * self.basis[2][row])
                    + self.position[row]
            }),
        }
    }

    /// Point 3c1caf and Linear 3c31d5 apply this after local property offsets.
    /// The previous matrix's up column is the look-at input, not camera up.
    pub(super) fn apply_camera_rotation(
        &mut self,
        rotation_type: i32,
        camera: Option<VfxBinderCameraSnapshot>,
    ) {
        match rotation_type as i8 {
            1 => {
                self.basis = camera
                    .expect("RoTp=1 requires a resolved Binder camera")
                    .basis
            }
            mode @ (2 | 3) => {
                let camera = camera.expect("RoTp=2/3 requires a resolved Binder camera");
                let goal = if mode == 2 {
                    std::array::from_fn(|axis| {
                        self.position[axis] + camera.parallel_direction[axis]
                    })
                } else {
                    camera.position
                };
                if let Some(basis) = self.linear_look_at(goal) {
                    self.basis = basis;
                }
            }
            _ => {} // Original switch accepts only signed low-byte values 1..3.
        }
    }

    /// Linear's 0x14037cc80 basis, before property offsets. A rejected
    /// direction preserves the start basis; zero up/nonfinite inputs are
    /// deliberately not replaced by a convenient world axis.
    pub(super) fn linear_look_at(self, goal: [f32; 3]) -> Option<[[f32; 3]; 3]> {
        let delta: [f32; 3] = std::array::from_fn(|axis| goal[axis] - self.position[axis]);
        let length_squared = (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2];
        if length_squared < f32::from_bits(0x38d1b717) {
            return None;
        }
        let inverse = 1.0 / length_squared.sqrt();
        let z = delta.map(|value| value * inverse);
        let up = self.basis[1];
        let up_inverse = 1.0 / ((up[0] * up[0] + up[1] * up[1]) + up[2] * up[2]).sqrt();
        let dot = ((up[1] * up_inverse) * z[1] + (up[0] * up_inverse) * z[0])
            + (up[2] * up_inverse) * z[2];
        if dot.abs() > f32::from_bits(0x3f7d70a4) {
            return None;
        }
        let x = [
            z[2] * up[1] - z[1] * up[2],
            z[0] * up[2] - z[2] * up[0],
            z[1] * up[0] - z[0] * up[1],
        ];
        let inverse = 1.0 / ((x[1] * x[1] + x[0] * x[0]) + x[2] * x[2]).sqrt();
        let x = x.map(|value| value * inverse);
        let y = [
            x[2] * z[1] - x[1] * z[2],
            x[0] * z[2] - x[2] * z[0],
            x[1] * z[0] - x[0] * z[1],
        ];
        Some([x, y, z])
    }
}

/// Resolved Binder camera TLS fields, expressed in the same space as targets.
/// The basis is copied verbatim by RoTp=1; `parallel_direction` is the raw
/// +0x30 vector added to the current position by RoTp=2; `position` is +0x3c.
/// These inputs are independent, and this type does not infer one from another.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderCameraSnapshot {
    pub basis: [[f32; 3]; 3],
    pub parallel_direction: [f32; 3],
    pub position: [f32; 3],
}

impl VfxBinderCameraSnapshot {
    /// Normal camera-slot producer 3b7e30: transpose the view's 3x3, use
    /// its horizontal Z row for RoTp=2, and copy the inverse view translation.
    /// Caller-resolved view/inverse matrices may be nonfinite; host admission
    /// validates the result, while numerical probes retain native propagation.
    pub fn from_view_matrices(view: [f32; 16], inverse_view: [f32; 16]) -> Self {
        Self {
            basis: std::array::from_fn(|column| std::array::from_fn(|row| view[row * 4 + column])),
            parallel_direction: [view[2], 0.0, view[10]],
            position: [inverse_view[12], inverse_view[13], inverse_view[14]],
        }
    }

    /// Reject nonfinite host inputs before changing playback state.
    pub fn validate(self) -> Result<(), String> {
        if self
            .basis
            .into_iter()
            .flatten()
            .chain(self.parallel_direction)
            .chain(self.position)
            .all(f32::is_finite)
        {
            Ok(())
        } else {
            Err("VFX Binder camera snapshot must be finite".into())
        }
    }
}

/// Resolved Linear curve outputs. The start Pos reader runs twice in the
/// native update, so its later result is independent of its interpolation
/// input (for example when the reader consumes shared random state).
#[derive(Clone, Copy, Debug)]
pub struct VfxLinearBinderCurves {
    pub factor: f32,
    pub start_position: [f32; 3],
    pub goal_position: [f32; 3],
    pub start_position_after: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct VfxLinearBinderFrame {
    pub age: f32,
    pub query_deadlines: [f32; 2],
    pub camera_position: [f32; 3],
    pub camera: Option<VfxBinderCameraSnapshot>,
    pub document_scale: [f32; 3],
    pub self_targets: [bool; 2],
    /// Ordinary bAGS updates retain the initialized auxiliary matrix.
    pub root_ags: bool,
}

impl VfxLinearBinderFrame {
    /// Native construction sign-extends each PrpS/PrpG CoUF word.
    pub fn authored_query_deadlines(binder: &AvfxBinder) -> [f32; 2] {
        [
            binder.properties_start.as_ref(),
            binder.properties_goal.as_ref(),
        ]
        .map(|properties| properties.map_or(-1.0, |p| p.coord_update_frame as i16 as f32))
    }
}

/// Numeric initialization and the direction passed to a root emitter.
/// Timeline child construction uses the main matrix instead of this direction.
/// Child allocation/ctor/attachment, object flags and lifetime belong to the
/// caller. None means the early query failure bypassed that creation phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLinearBinderInitialization {
    pub queries: [Option<VfxBinderQueryStatus>; 2],
    pub update: Option<[VfxBinderQueryStatus; 2]>,
    pub child_direction: Option<[f32; 3]>,
}

/// Common Binder clocks at construction; the native Binder base fixes rate at 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxBinderBirthClock {
    pub local_age: f32,
    pub total_age: f32,
    pub previous_age: f32,
    pub nominal_life: f32,
    pub rate: f32,
    pub delay: f32,
}

impl VfxBinderBirthClock {
    pub fn new(input_age: f32, nominal_life: i32, delay: f32) -> Self {
        let rate = 1.0;
        let age = input_age * rate;
        Self {
            local_age: age,
            total_age: age,
            previous_age: age,
            nominal_life: nominal_life as f32,
            rate,
            delay,
        }
    }

    /// Both Linear constructors defer positive and unordered delays. Signed
    /// zero and negative delays initialize immediately without shifting age.
    pub fn initializes_immediately(self) -> bool {
        self.delay <= 0.0
    }
}

/// The two self-update phases dispatched by Common and the Binder base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxBinderStep {
    Update,
    Initialize,
}

/// Non-looping Binder self lifecycle (+60 prewarm or +68/+70 normal phases).
/// The latter commits age and clears dead children before running Binder work.
/// Child objects
/// still advance after a retired parent; their traversal/destruction is owned
/// by the caller. The constructors disable Common's age-loop flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxBinderLifecycle {
    pub clock: VfxBinderBirthClock,
    pub scaled_delta: f32,
    pub numeric: super::VfxCommonNumericState,
    flags: u32,
    pub life_limit_enabled: bool,
}

/// Pending Common +68 writes. Retirement callbacks run after flags change
/// and before these ages are committed, and may visit the owner's whole tree.
pub(super) struct VfxBinderTimeWrite {
    pub retired: bool,
    age: f32,
    total: f32,
}

impl VfxBinderLifecycle {
    pub fn new(clock: VfxBinderBirthClock) -> Self {
        Self {
            clock,
            scaled_delta: 0.0,
            numeric: super::VfxCommonNumericState::default(),
            flags: 0x3f00_0000,
            life_limit_enabled: true,
        }
    }

    pub fn raw_flags(self) -> u32 {
        self.flags
    }

    /// Returns whether the owner must visit its currently registered children.
    pub fn unlock_loop_point(&mut self) -> bool {
        super::unlock_loop_flags(&mut self.flags)
    }

    pub fn retired(self) -> bool {
        self.flags & 0x40000 != 0
    }

    pub fn retire(&mut self) {
        self.flags = (self.flags & 0xc0ff_ffff) | 0x40000;
    }

    pub fn configure_fade(&mut self, duration: i32, mode: u32, flag: bool) {
        self.numeric
            .configure_fade(&mut self.flags, duration, mode, flag);
    }

    /// Call after a successful attachment/+08, with the parent's current state.
    pub fn inherit_fade_after_attach(
        &mut self,
        parent_flags: u32,
        parent: super::VfxCommonNumericState,
    ) {
        self.numeric
            .inherit_fade_after_attach(&mut self.flags, parent_flags, parent);
    }

    /// Point/Linear Common +78 has no-op +100 and +10 callbacks. Derived Clip
    /// callbacks are not represented here and need their own begin/finish pair.
    /// The owner supplies global +18 adjustment and descendant traversal.
    pub fn refresh_numeric(&mut self) {
        if let Some(step) =
            self.numeric
                .begin_refresh(&mut self.flags, self.scaled_delta, |_, _| {})
        {
            self.numeric.finish_refresh(step);
        }
    }

    pub fn registered_children(self) -> u16 {
        ((self.flags >> 9) & 0x1ff) as u16
    }

    /// A deferred Binder keeps one factory in the low nine bits and disables
    /// the ordinary life limit until initialization consumes that factory.
    pub(super) fn defer_single_factory(&mut self) {
        self.flags |= 1;
        self.life_limit_enabled = false;
    }

    pub(super) fn consume_pending_factory(&mut self) {
        let pending = self.flags.wrapping_sub(1) & 0x1ff;
        self.flags = (self.flags & !0x1ff) | pending;
        self.life_limit_enabled = pending == 0;
    }

    /// The native count is a packed nine-bit field, independent of retirement.
    pub fn register_child(&mut self) {
        self.set_registered_children(self.registered_children().wrapping_add(1));
    }

    pub fn remove_child(&mut self) {
        self.set_registered_children(self.registered_children().wrapping_sub(1));
    }

    pub fn set_registered_children(&mut self, count: u16) {
        self.flags = (self.flags & !0x3fe00) | ((u32::from(count) & 0x1ff) << 9);
    }

    /// Common +68 selects its own age callback with packed flag bit 24.
    pub fn set_time_advance_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x100_0000) | (u32::from(enabled) << 24);
    }

    /// Common +70 and the Binder Prepare callback require bits 25 and 28.
    pub fn set_prepare_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x1200_0000) | if enabled { 0x1200_0000 } else { 0 };
    }

    pub fn set_numeric_self_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x0400_0000) | (u32::from(enabled) << 26);
    }

    pub fn set_numeric_base_refresh_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x2000_0000) | (u32::from(enabled) << 29);
    }

    fn limit_age(&mut self, age: &mut f32) {
        if self.life_limit_enabled
            && self.clock.nominal_life >= 0.0
            && *age > self.clock.nominal_life
        {
            *age = self.clock.nominal_life;
            if !self.retired() {
                self.retire();
            }
        }
    }

    /// Ordinary +68 (3b0420/3af6f0). Linear's +f0 is a no-op, so this only
    /// commits clocks. The caller then removes previously retired empty
    /// children before advancing the survivors; +70 is a separate tree pass.
    pub fn advance_time(&mut self, delta: f32) {
        if let Some(write) = self.begin_time(delta) {
            self.finish_time(write);
        }
    }

    pub(super) fn begin_time(&mut self, delta: f32) -> Option<VfxBinderTimeWrite> {
        let scaled_delta = delta * self.clock.rate;
        self.clock.previous_age += scaled_delta;
        if self.flags & 0x100_0000 == 0 {
            return None;
        }
        self.scaled_delta = scaled_delta;
        let mut next_age = scaled_delta + self.clock.local_age;
        let next_total = self.clock.total_age + scaled_delta;
        let was_retired = self.retired();
        self.limit_age(&mut next_age);
        Some(VfxBinderTimeWrite {
            retired: !was_retired && self.retired(),
            age: next_age,
            total: next_total,
        })
    }

    pub(super) fn finish_time(&mut self, write: VfxBinderTimeWrite) {
        self.clock.local_age = write.age;
        self.clock.total_age = write.total;
    }

    pub(super) fn begin_numeric(&mut self) -> Option<super::VfxCommonFadeStep> {
        self.numeric
            .begin_refresh(&mut self.flags, self.scaled_delta, |_, _| {})
    }

    /// +70 (3b0530/3af7d0 -> Linear +f8). Unlike +60, a delayed initializer's
    /// reset clocks persist. Children attached here missed this input's +68;
    /// the caller visits them in +70 without backfilling an age update.
    pub fn prepare_self(&mut self, step: impl FnMut(&mut Self, VfxBinderStep)) {
        if self.flags & 0x1200_0000 == 0x1200_0000 {
            self.update_and_delay(step);
        }
    }

    fn update_and_delay(&mut self, mut step: impl FnMut(&mut Self, VfxBinderStep)) {
        step(self, VfxBinderStep::Update);
        if self.clock.delay > 0.0 {
            self.clock.delay -= self.scaled_delta;
            if self.clock.delay <= 0.0 {
                let age = -self.clock.delay * self.clock.rate;
                self.clock.local_age = age;
                self.clock.total_age = age;
                step(self, VfxBinderStep::Initialize);
            }
        }
        if self.clock.nominal_life < 0.0 && self.registered_children() == 0 && !self.retired() {
            self.retire();
        }
    }

    /// Prewarm +60: run self phases before the caller advances its children. Both phases
    /// see the old clocks, except Initialize sees delay overshoot * rate.
    /// Common commits its pending ages after these callbacks, overwriting that
    /// temporary reset. `previous_age` advances even after retirement.
    pub fn advance_self(&mut self, delta: f32, step: impl FnMut(&mut Self, VfxBinderStep)) {
        let scaled_delta = delta * self.clock.rate;
        self.clock.previous_age += scaled_delta;
        if self.retired() {
            return;
        }
        self.scaled_delta = scaled_delta;
        let mut next_age = scaled_delta + self.clock.local_age;
        let next_total = self.clock.total_age + scaled_delta;
        self.limit_age(&mut next_age);
        if !self.retired() {
            self.update_and_delay(step);
        }
        self.clock.local_age = next_age;
        self.clock.total_age = next_total;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLinearBinderConstruction {
    pub state: VfxLinearBinderState,
    pub clock: VfxBinderBirthClock,
    /// None means construction deferred initialization because of the delay.
    pub initialization: Option<VfxLinearBinderInitialization>,
}

/// Persistent Linear numeric state, separate from external child factories
/// and provider histories. An attempted initializer, including failure, sets
/// `initialized`; subsequent updates can query again but never recreate children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxLinearBinderInstance {
    pub state: VfxLinearBinderState,
    pub lifecycle: VfxBinderLifecycle,
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VfxLinearBinderAdvance {
    pub update: Option<[VfxBinderQueryStatus; 2]>,
    pub initialization: Option<VfxLinearBinderInitialization>,
}

impl VfxLinearBinderConstruction {
    /// `registered_children` counts successful external factory attachments,
    /// not merely a successful numeric initializer.
    pub fn into_instance(self, registered_children: u16) -> VfxLinearBinderInstance {
        let mut lifecycle = VfxBinderLifecycle::new(self.clock);
        if self.initialization.is_none() {
            // Linear ctor 3c1f10 holds one pending factory, just like Point.
            // Initializer 3c25be..3c2610 consumes it before either endpoint query.
            lifecycle.flags |= 1;
            lifecycle.life_limit_enabled = false;
        }
        lifecycle.set_registered_children(registered_children);
        VfxLinearBinderInstance {
            state: self.state,
            lifecycle,
            initialized: self.initialization.is_some(),
        }
    }
}

impl VfxLinearBinderInstance {
    /// Prewarm +60. The external factory receives temporary initialization clocks,
    /// before Common writes back its pending ages and advances children.
    pub fn advance_self(
        &mut self,
        binder: &AvfxBinder,
        delta: f32,
        frame: VfxLinearBinderFrame,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        attach_child: impl FnMut(
            &VfxLinearBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxLinearBinderAdvance {
        self.run_self_step(
            binder,
            Some(delta),
            frame,
            target,
            listener_scale,
            curves,
            root_revision,
            attach_child,
        )
    }

    /// Advance +68 only. All objects in the tree do this before any +70 work.
    pub fn advance_time(&mut self, delta: f32) {
        self.lifecycle.advance_time(delta);
    }

    /// Execute +70 using the clocks already written by +68. Successful delayed
    /// initialization keeps its overshoot clock instead of the +60 writeback.
    pub fn prepare_self(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        attach_child: impl FnMut(
            &VfxLinearBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxLinearBinderAdvance {
        self.run_self_step(
            binder,
            None,
            frame,
            target,
            listener_scale,
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
        mut curves: impl FnMut(f32, f32) -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
        mut attach_child: impl FnMut(
            &VfxLinearBinderState,
            &VfxLinearBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxLinearBinderAdvance {
        let mut result = VfxLinearBinderAdvance::default();
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
                    let pending = lifecycle.flags.wrapping_sub(1) & 0x1ff;
                    lifecycle.flags = (lifecycle.flags & !0x1ff) | pending;
                    lifecycle.life_limit_enabled = pending == 0;
                    self.initialized = true;
                    let initialization = self.state.initialize(
                        binder,
                        frame,
                        &mut target,
                        &mut listener_scale,
                        || curves(frame.age, lifecycle.clock.total_age),
                        root_revision,
                    );
                    if initialization.child_direction.is_some()
                        && attach_child(&self.state, &initialization, lifecycle.clock)
                    {
                        lifecycle.register_child();
                    }
                    result.initialization = Some(initialization);
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

/// Linear rotation-type 0 core (0x1403c2c40). Endpoint queries have separate
/// deadlines, scalar and depth caches. Numeric initialization is provided
/// below; the caller owns object construction, provider identities, curve
/// clocks/RNG, camera modes and lifetime.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxLinearBinderState {
    pub matrix: VfxBinderMatrix,
    pub auxiliary_matrix: VfxBinderMatrix,
    pub targets: [VfxBinderMatrix; 2],
    /// +0x38 uses the start query scale, never its interpolation with goal.
    pub scale: [f32; 3],
    pub endpoint_vfx_scales: [f32; 2],
    pub endpoint_depth_scales: [f32; 2],
    pub vfx_scale: f32,
    pub transform_depth_scale: f32,
    pub start_property_world: [f32; 3],
}

impl VfxLinearBinderState {
    /// Prepared numeric cache for the update/initialization core. This does
    /// not execute either Linear constructor or its Document-derived defaults.
    pub fn new(matrix: VfxBinderMatrix) -> Self {
        Self {
            matrix,
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            targets: [VfxBinderMatrix::IDENTITY; 2],
            scale: [1.0; 3],
            endpoint_vfx_scales: [0.0; 2],
            endpoint_depth_scales: [1.0; 2],
            vfx_scale: 0.0,
            transform_depth_scale: 1.0,
            start_property_world: [0.0; 3],
        }
    }

    /// Numeric storage defaults of both 3c1f10 (Emitter) and 3c2240 (Timeline)
    /// constructors. The target caches are zero matrices, independent of the
    /// identity main matrix and the Document scale/auxiliary defaults.
    pub fn constructed(document_scale: [f32; 3]) -> Self {
        Self {
            auxiliary_matrix: VfxBinderMatrix::scale_matrix(document_scale),
            targets: [VfxBinderMatrix {
                basis: [[0.0; 3]; 3],
                position: [0.0; 3],
            }; 2],
            scale: document_scale,
            start_property_world: [1.0; 3],
            ..Self::new(VfxBinderMatrix::IDENTITY)
        }
    }

    /// Constructor numeric caches, Common birth clocks and the immediate
    /// initialization gate. Object allocation/parent pointers, coefficient
    /// RNG, child factory selection and lifecycle are caller-resolved.
    pub fn construct(
        binder: &AvfxBinder,
        mut frame: VfxLinearBinderFrame,
        delay: f32,
        target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(usize, &mut f32) -> bool,
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxLinearBinderConstruction {
        let clock = VfxBinderBirthClock::new(frame.age, binder.life, delay);
        frame.age = clock.local_age;
        frame.query_deadlines = VfxLinearBinderFrame::authored_query_deadlines(binder);
        let mut state = Self::constructed(frame.document_scale);
        let initialization = clock.initializes_immediately().then(|| {
            state.initialize(binder, frame, target, listener_scale, curves, root_revision)
        });
        VfxLinearBinderConstruction {
            state,
            clock,
            initialization,
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
            let status = self.query_endpoint(
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
                self.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.scale);
            }
            status
        });
        self.update_resolved(binder, curves);
        self.matrix
            .apply_camera_rotation(binder.rotation_type, frame.camera);
        statuses
    }

    /// 3c2590: unconditional start/goal first queries, then the normal gated
    /// update. Root revision replaces the auxiliary only after that update.
    /// `curves` is not evaluated when an initial query fails.
    pub fn initialize(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        mut target: impl FnMut(usize) -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(usize, &mut f32) -> bool,
        curves: impl FnOnce() -> VfxLinearBinderCurves,
        root_revision: VfxBinderMatrix,
    ) -> VfxLinearBinderInitialization {
        let mut result = VfxLinearBinderInitialization {
            queries: [None; 2],
            update: None,
            child_direction: None,
        };
        for endpoint in 0..2 {
            // First queries do not multiply Document scale. A goal failure
            // must leave the start scale raw, rather than partially initialized.
            let status = self.query_endpoint(
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
                self.vfx_scale = self.endpoint_vfx_scales[0];
                self.transform_depth_scale = self.endpoint_depth_scales[0];
            }
        }
        self.scale = std::array::from_fn(|axis| frame.document_scale[axis] * self.scale[axis]);
        // Initialization writes a diagonal even with bAGS. An ordinary
        // failed query may replace it; the root revision comes later.
        self.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.scale);
        result.update =
            Some(self.update_frame(binder, frame, &mut target, &mut listener_scale, curves()));
        let delta: [f32; 3] = std::array::from_fn(|axis| {
            self.targets[1].position[axis] - self.targets[0].position[axis]
        });
        let length_squared = (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2];
        result.child_direction = Some(if length_squared < f32::from_bits(0x38d1b717) {
            self.targets[0].basis[2]
        } else {
            delta
        });
        if frame.root_ags {
            self.auxiliary_matrix = root_revision;
        }
        result
    }

    pub(super) fn query_endpoint(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxLinearBinderFrame,
        endpoint: usize,
        document_scale: [f32; 3],
        target: impl FnOnce() -> Option<VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
    ) -> VfxBinderQueryStatus {
        // Shared original query semantics, with independent endpoint caches.
        let mut query = VfxPointBinderState::new(self.matrix);
        query.target = self.targets[endpoint];
        query.scale = if endpoint == 0 { self.scale } else { [0.0; 3] };
        query.vfx_scale = self.endpoint_vfx_scales[endpoint];
        query.transform_depth_scale = self.endpoint_depth_scales[endpoint];
        let status = query.query(
            binder,
            VfxPointBinderFrame {
                age: frame.age,
                query_deadline: frame.query_deadlines[endpoint],
                camera_position: frame.camera_position,
                camera: None,
                document_scale,
                self_target: frame.self_targets[endpoint],
                root_revision: None,
            },
            target,
            listener_scale,
        );
        self.targets[endpoint] = query.target;
        self.endpoint_vfx_scales[endpoint] = query.vfx_scale;
        self.endpoint_depth_scales[endpoint] = query.transform_depth_scale;
        if endpoint == 0 {
            self.scale = query.scale;
        }
        status
    }

    pub(super) fn update_resolved(&mut self, binder: &AvfxBinder, curves: VfxLinearBinderCurves) {
        let blend = |start: f32, goal: f32| (goal - start) * curves.factor + start;
        self.vfx_scale = if binder.vfx_scale_depth_offset && binder.vfx_scale_interpolation {
            blend(self.endpoint_vfx_scales[0], self.endpoint_vfx_scales[1])
        } else {
            self.endpoint_vfx_scales[0]
        };
        self.transform_depth_scale =
            if binder.transform_scale_depth_offset && binder.transform_scale_interpolation {
                blend(self.endpoint_depth_scales[0], self.endpoint_depth_scales[1])
            } else {
                self.endpoint_depth_scales[0]
            };
        // The horizontal Linear subtype BnVr=4 modifies the goal cache itself,
        // even when the query skipped. This is independent of RoTp (+0x2b).
        if binder.binder_type as u8 == 4 {
            self.targets[1].position[1] = self.targets[0].position[1];
        }
        let [start, goal] = self.targets;
        let basis = if binder.start_to_global_direction {
            start.linear_look_at(goal.position).unwrap_or(start.basis)
        } else {
            std::array::from_fn(|column| {
                let v: [f32; 3] = std::array::from_fn(|row| {
                    blend(start.basis[column][row], goal.basis[column][row])
                });
                let inverse = 1.0 / ((v[1] * v[1] + v[0] * v[0]) + v[2] * v[2]).sqrt();
                v.map(|value| value * inverse)
            })
        };
        let matrix = VfxBinderMatrix {
            basis,
            position: std::array::from_fn(|axis| blend(start.position[axis], goal.position[axis])),
        };
        let property = VfxBinderMatrix {
            position: std::array::from_fn(|axis| {
                blend(curves.start_position[axis], curves.goal_position[axis])
            }),
            ..VfxBinderMatrix::IDENTITY
        };
        // 37be80 uses y+x+z for the third column and the Y/Z translation,
        // unlike the independent auxiliary multiply's X/Y/Z translation.
        self.matrix = matrix.transform_matrix(property);
        self.matrix.position = std::array::from_fn(|row| {
            let x = property.position[0] * matrix.basis[0][row];
            let y = property.position[1] * matrix.basis[1][row];
            let z = property.position[2] * matrix.basis[2][row];
            (if row == 0 { x + y } else { y + x }) + z + matrix.position[row]
        });
        self.start_property_world = std::array::from_fn(|row| {
            ((curves.start_position_after[1] * start.basis[1][row]
                + curves.start_position_after[0] * start.basis[0][row])
                + curves.start_position_after[2] * start.basis[2][row])
                + start.position[row]
        });
    }

    pub fn depth_offset_multiplier(&self, binder: &AvfxBinder) -> f32 {
        VfxBinderQueryScale {
            target_scale: self.scale,
            vfx_scale: self.vfx_scale,
        }
        .depth_offset_multiplier(binder, self.transform_depth_scale)
    }
}

/// Point update state after caller-resolved curve evaluation
/// and query gating. Creation, curve RNG, camera compensation and dynamic
/// target providers are separate parts of the Binder runtime.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxPointBinderState {
    pub matrix: VfxBinderMatrix,
    /// Independent +0x2b0 matrix. Queries refresh its diagonal even on
    /// failure when root bAGS is off; deadline skips retain it. With bAGS
    /// it is replaced after initialization by the resolved root revision.
    pub auxiliary_matrix: VfxBinderMatrix,
    pub target: VfxBinderMatrix,
    pub scale: [f32; 3],
    /// Target query +0x268 scalar cache. Zero requests the listener again;
    /// nonzero (including NaN) is reused until an upstream reset.
    pub vfx_scale: f32,
    /// Target query +0x26c. Reset to 1 on each query attempt, even failures.
    pub transform_depth_scale: f32,
}

/// Caller-resolved inputs for Point update. The caller owns
/// construction, curve clocks/RNG, target identity and camera snapshot history.
#[derive(Clone, Copy, Debug)]
pub struct VfxPointBinderFrame {
    pub age: f32,
    pub query_deadline: f32,
    pub camera_position: [f32; 3],
    pub camera: Option<VfxBinderCameraSnapshot>,
    pub document_scale: [f32; 3],
    pub self_target: bool,
    /// None means root +f4 (bAGS) is off. Some supplies the already resolved
    /// Document-scale/root-rotation/root-position matrix; this core does not
    /// calculate its host inputs or the client's rotation math.
    pub root_revision: Option<VfxBinderMatrix>,
}

impl VfxPointBinderFrame {
    /// PrpS.CoUF is stored as a word by 0x14039bf90 and sign-extended by
    /// Point construction (0x1403c12b7). Negative values keep querying;
    /// nonnegative values include the deadline frame itself.
    pub fn authored_query_deadline(binder: &AvfxBinder) -> f32 {
        binder.properties_start.as_ref().map_or(-1.0, |properties| {
            properties.coord_update_frame as i16 as f32
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxBinderQueryStatus {
    Skipped,
    TargetUnavailable,
    ScaleUnavailable,
    Refreshed,
}

/// Query outcomes during native Point initialization. The first query bypasses
/// CoUF. Only its success permits the ordinary, deadline-gated update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxPointBinderInitialization {
    pub query: VfxBinderQueryStatus,
    pub update: Option<VfxBinderQueryStatus>,
}

impl VfxPointBinderInitialization {
    /// Point's emitter factory passes the zero-vector provider at
    /// 0x142a8f7c0 in params +0x18 (0x1403c15fc). This is independent of
    /// ElementId Euler, bFTO and the queried target's Z column. Linear's
    /// endpoint-derived direction belongs to its separate initialization.
    pub fn child_direction(self) -> Option<[f32; 3]> {
        self.child_ready().then_some([0.0; 3])
    }

    /// Only the first query gates the native child factory. A failed second
    /// query still reaches it with the partially updated numeric caches.
    pub fn child_ready(self) -> bool {
        self.query == VfxBinderQueryStatus::Refreshed
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxPointBinderCurves {
    pub spring: f32,
    pub position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxPointBinderConstruction {
    pub state: VfxPointBinderState,
    pub clock: VfxBinderBirthClock,
    pub initialization: Option<VfxPointBinderInitialization>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxPointBinderInstance {
    pub state: VfxPointBinderState,
    pub lifecycle: VfxBinderLifecycle,
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VfxPointBinderAdvance {
    pub update: Option<VfxBinderQueryStatus>,
    pub initialization: Option<VfxPointBinderInitialization>,
}

impl VfxPointBinderConstruction {
    pub fn into_instance(self, registered_children: u16) -> VfxPointBinderInstance {
        let mut lifecycle = VfxBinderLifecycle::new(self.clock);
        if self.initialization.is_none() {
            // Point ctor 3c1289 holds one low-nine-bit pending factory and
            // disables Common life checking until +120 initialization consumes
            // it (3c1348..3c1393), even if that first query then fails.
            lifecycle.flags |= 1;
            lifecycle.life_limit_enabled = false;
        }
        lifecycle.set_registered_children(registered_children);
        VfxPointBinderInstance {
            state: self.state,
            lifecycle,
            initialized: self.initialization.is_some(),
        }
    }
}

impl VfxPointBinderInstance {
    /// Point +f0 is the same no-op as Linear; Common +68 only advances clocks.
    pub fn advance_time(&mut self, delta: f32) {
        self.lifecycle.advance_time(delta);
    }

    pub fn prepare_self(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        target: impl FnMut() -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(&mut f32) -> bool,
        curves: impl FnMut(f32, f32) -> VfxPointBinderCurves,
        attach_child: impl FnMut(
            &VfxPointBinderState,
            VfxPointBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxPointBinderAdvance {
        self.run_self_step(
            binder,
            None,
            frame,
            target,
            listener_scale,
            curves,
            attach_child,
        )
    }

    pub fn advance_self(
        &mut self,
        binder: &AvfxBinder,
        delta: f32,
        frame: VfxPointBinderFrame,
        target: impl FnMut() -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(&mut f32) -> bool,
        curves: impl FnMut(f32, f32) -> VfxPointBinderCurves,
        attach_child: impl FnMut(
            &VfxPointBinderState,
            VfxPointBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxPointBinderAdvance {
        self.run_self_step(
            binder,
            Some(delta),
            frame,
            target,
            listener_scale,
            curves,
            attach_child,
        )
    }

    fn run_self_step(
        &mut self,
        binder: &AvfxBinder,
        prewarm_delta: Option<f32>,
        mut frame: VfxPointBinderFrame,
        mut target: impl FnMut() -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(&mut f32) -> bool,
        mut curves: impl FnMut(f32, f32) -> VfxPointBinderCurves,
        mut attach_child: impl FnMut(
            &VfxPointBinderState,
            VfxPointBinderInitialization,
            VfxBinderBirthClock,
        ) -> bool,
    ) -> VfxPointBinderAdvance {
        let mut result = VfxPointBinderAdvance::default();
        frame.query_deadline = VfxPointBinderFrame::authored_query_deadline(binder);
        let step = |lifecycle: &mut VfxBinderLifecycle, phase| {
            frame.age = lifecycle.clock.local_age;
            match phase {
                VfxBinderStep::Update if self.initialized => {
                    result.update = Some(self.state.update_with_curves(
                        binder,
                        frame,
                        &mut target,
                        &mut listener_scale,
                        || curves(frame.age, lifecycle.clock.total_age),
                    ));
                }
                VfxBinderStep::Update => {}
                VfxBinderStep::Initialize => {
                    let pending = lifecycle.flags.wrapping_sub(1) & 0x1ff;
                    lifecycle.flags = (lifecycle.flags & !0x1ff) | pending;
                    lifecycle.life_limit_enabled = pending == 0;
                    self.initialized = true;
                    let initialization = self.state.initialize_with_curves(
                        binder,
                        frame,
                        &mut target,
                        &mut listener_scale,
                        || curves(frame.age, lifecycle.clock.total_age),
                    );
                    if initialization.child_ready()
                        && attach_child(&self.state, initialization, lifecycle.clock)
                    {
                        lifecycle.register_child();
                    }
                    result.initialization = Some(initialization);
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

impl VfxPointBinderState {
    /// Consume the independent auxiliary matrix, as 0x1403d0300 does after
    /// the +0x30 getter follows the parent chain to Point +0x2b0. The caller
    /// owns selection of this callback: native factories disable it for
    /// PICd 2/3/8, and it must not replace the main or scale getter.
    pub fn transform_auxiliary(&self, local: VfxBinderMatrix) -> VfxBinderMatrix {
        self.auxiliary_matrix.transform_matrix(local)
    }

    /// Cache defaults written by the common Binder and Point constructors.
    /// The caller supplies the object's current matrix; this does not resolve
    /// object construction, parent inheritance, clocks or shared curve RNG.
    pub fn new(matrix: VfxBinderMatrix) -> Self {
        Self {
            matrix,
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            target: VfxBinderMatrix::IDENTITY,
            scale: [1.0; 3],
            vfx_scale: 0.0,
            transform_depth_scale: 1.0,
        }
    }

    /// Point's current matrix starts at identity in the original Common
    /// object constructor. Its parent pointer does not copy a parent matrix.
    pub fn constructed() -> Self {
        Self::new(VfxBinderMatrix::IDENTITY)
    }

    /// Numeric portion of Point construction. Identity caches and the
    /// immediate/deferred initializer match 3c1020; external child allocation
    /// and attachment remain the caller's responsibility.
    pub fn construct(
        binder: &AvfxBinder,
        mut frame: VfxPointBinderFrame,
        delay: f32,
        target: impl FnMut() -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(&mut f32) -> bool,
        curves: impl FnOnce() -> VfxPointBinderCurves,
    ) -> VfxPointBinderConstruction {
        let clock = VfxBinderBirthClock::new(frame.age, binder.life, delay);
        frame.age = clock.local_age;
        frame.query_deadline = VfxPointBinderFrame::authored_query_deadline(binder);
        let mut state = Self::constructed();
        let initialization = clock
            .initializes_immediately()
            .then(|| state.initialize_with_curves(binder, frame, target, listener_scale, curves));
        VfxPointBinderConstruction {
            state,
            clock,
            initialization,
        }
    }

    /// 0x1403c1310's initial query and matrix update, with
    /// caller-resolved curves and inputs. A failed initial query leaves its
    /// partial cache writes, without applying SpS/Pos. A successful one can
    /// query a second target during the normal update and reuses a nonzero
    /// VFX scalar. The caller supplies a resolved bAGS matrix; child creation,
    /// construction clocks, shared RNG and host input resolution remain separate.
    pub fn initialize_frame(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        target: impl FnMut() -> Option<VfxBinderTarget>,
        listener_scale: impl FnMut(&mut f32) -> bool,
        spring: f32,
        property_position: [f32; 3],
    ) -> VfxPointBinderInitialization {
        self.initialize_with_curves(binder, frame, target, listener_scale, || {
            VfxPointBinderCurves {
                spring,
                position: property_position,
            }
        })
    }

    pub fn initialize_with_curves(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        mut target: impl FnMut() -> Option<VfxBinderTarget>,
        mut listener_scale: impl FnMut(&mut f32) -> bool,
        curves: impl FnOnce() -> VfxPointBinderCurves,
    ) -> VfxPointBinderInitialization {
        let query = self.query(binder, frame, &mut target, &mut listener_scale);
        let update = (query == VfxBinderQueryStatus::Refreshed).then(|| {
            // 3c13f9..1428 writes this before ordinary update even with bAGS.
            self.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.scale);
            let status = self.update_with_curves(binder, frame, target, listener_scale, curves);
            if let Some(revision) = frame.root_revision {
                self.auxiliary_matrix = revision;
            }
            status
        });
        VfxPointBinderInitialization { query, update }
    }

    /// Execute the native query gate and cache writes before SpS/Pos. A failed
    /// scale callback can still write its output; pass that behavior through
    /// the mutable scalar, rather than treating query failure as a rollback.
    pub fn update_frame(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        target: impl FnOnce() -> Option<VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
        spring: f32,
        property_position: [f32; 3],
    ) -> VfxBinderQueryStatus {
        self.update_with_curves(binder, frame, target, listener_scale, || {
            VfxPointBinderCurves {
                spring,
                position: property_position,
            }
        })
    }

    pub fn update_with_curves(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        target: impl FnOnce() -> Option<VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
        curves: impl FnOnce() -> VfxPointBinderCurves,
    ) -> VfxBinderQueryStatus {
        // Native COMISS gate skips NaN deadlines and NaN ages unless the
        // deadline is negative; equality still executes the query.
        let status = if frame.query_deadline < 0.0 || frame.query_deadline >= frame.age {
            let status = self.query(binder, frame, target, listener_scale);
            // Native 3c19ca branches on root f4 after both successful and
            // failed queries. The deadline skip bypasses these writes.
            if frame.root_revision.is_none() {
                self.auxiliary_matrix = VfxBinderMatrix::scale_matrix(self.scale);
            }
            status
        } else {
            VfxBinderQueryStatus::Skipped
        };
        let curves = curves();
        self.update(None, frame.document_scale, curves.spring, curves.position);
        self.matrix
            .apply_camera_rotation(binder.rotation_type, frame.camera);
        status
    }

    fn query(
        &mut self,
        binder: &AvfxBinder,
        frame: VfxPointBinderFrame,
        target: impl FnOnce() -> Option<VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
    ) -> VfxBinderQueryStatus {
        let mut query = VfxBinderQueryCache {
            target: self.target,
            scale: self.scale,
            vfx_scale: self.vfx_scale,
            transform_depth_scale: self.transform_depth_scale,
        };
        let status = query.refresh(
            binder,
            frame.camera_position,
            frame.self_target,
            target,
            listener_scale,
        );
        self.target = query.target;
        self.scale = query.scale;
        self.vfx_scale = query.vfx_scale;
        self.transform_depth_scale = query.transform_depth_scale;
        if status == VfxBinderQueryStatus::Refreshed {
            self.scale = std::array::from_fn(|axis| self.scale[axis] * frame.document_scale[axis]);
        }
        status
    }

    pub fn depth_offset_multiplier(&self, binder: &AvfxBinder) -> f32 {
        VfxBinderQueryScale {
            target_scale: self.scale,
            vfx_scale: self.vfx_scale,
        }
        .depth_offset_multiplier(binder, self.transform_depth_scale)
    }

    /// Successful queries replace the cached target and multiply the query
    /// scale by Document +0x38, irrespective of bDSE. None retains both cached
    /// outputs, but still runs SpS/Pos. This consumes an already successful
    /// query; use update_frame for native query failure and deadline semantics.
    pub fn update(
        &mut self,
        query: Option<(VfxBinderTarget, VfxBinderQueryScale)>,
        document_scale: [f32; 3],
        spring: f32,
        property_position: [f32; 3],
    ) {
        if let Some((target, scale)) = query {
            self.target = VfxBinderMatrix {
                basis: target.basis,
                position: target.position,
            };
            self.scale =
                std::array::from_fn(|axis| scale.target_scale[axis] * document_scale[axis]);
            self.vfx_scale = scale.vfx_scale;
        }
        // Native 3c1aae..1b29 copies at 1 and skips at 0. Computing a lerp at
        // 1 can lose small target values through cancellation or overflow.
        if spring == 1.0 {
            self.matrix = self.target;
        } else if spring != 0.0 {
            let blend = |old, target| old + (target - old) * spring;
            self.matrix = VfxBinderMatrix {
                basis: std::array::from_fn(|column| {
                    std::array::from_fn(|row| {
                        blend(
                            self.matrix.basis[column][row],
                            self.target.basis[column][row],
                        )
                    })
                }),
                position: std::array::from_fn(|axis| {
                    blend(self.matrix.position[axis], self.target.position[axis])
                }),
            };
        }
        // 3c1ca3 applies the evaluated property translation after the spring;
        // this translated matrix, including the offset, is next step's history.
        let matrix = self.matrix;
        self.matrix = matrix.transform_matrix(VfxBinderMatrix {
            position: property_position,
            ..VfxBinderMatrix::IDENTITY
        });
        self.matrix.position = std::array::from_fn(|row| {
            let x = property_position[0] * matrix.basis[0][row];
            let y = property_position[1] * matrix.basis[1][row];
            let z = property_position[2] * matrix.basis[2][row];
            (if row == 0 { x + y } else { y + x }) + z + matrix.position[row]
        });
    }
}

/// Shared 0x1403bef20 cache writes before Binder-specific Document scaling.
/// Camera discards the queried matrix but consumes the same scalar history.
pub(super) struct VfxBinderQueryCache {
    pub target: VfxBinderMatrix,
    pub scale: [f32; 3],
    pub vfx_scale: f32,
    pub transform_depth_scale: f32,
}

impl VfxBinderQueryCache {
    pub fn refresh(
        &mut self,
        binder: &AvfxBinder,
        camera_position: [f32; 3],
        self_target: bool,
        target: impl FnOnce() -> Option<VfxBinderTarget>,
        listener_scale: impl FnOnce(&mut f32) -> bool,
    ) -> VfxBinderQueryStatus {
        // 3bef5d precedes the target callback, so failure still clears depth.
        self.transform_depth_scale = 1.0;
        let Some(target) = target() else {
            return VfxBinderQueryStatus::TargetUnavailable;
        };
        self.target = VfxBinderMatrix {
            basis: target.basis,
            position: target.position,
        };
        self.transform_depth_scale = target.query_depth_scale(binder, camera_position);
        // Basis/length writes happen before the scale listener. A later
        // failure leaves these raw (not biased, not Document-multiplied).
        self.scale = if binder.transform_scale as u8 != 0 {
            target.scale
        } else {
            [1.0; 3]
        };
        if binder.vfx_scale_enabled || binder.vfx_scale_depth_offset {
            if self.vfx_scale == 0.0 && !listener_scale(&mut self.vfx_scale) {
                return VfxBinderQueryStatus::ScaleUnavailable;
            }
        } else {
            self.vfx_scale = 1.0;
        }
        let query = target.query_scale(binder, self.vfx_scale, self_target);
        self.scale = query.target_scale;
        VfxBinderQueryStatus::Refreshed
    }
}

/// Result of the client's target-basis helper (0x1403bf370), before the
/// independent depth, transform, VFX and Document scale rules are applied.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxBinderTarget {
    pub position: [f32; 3],
    pub basis: [[f32; 3]; 3],
    /// Column lengths; bFTO adds EPSILON to these outputs as well as using
    /// them as normalization divisors. These are not the final Binder scale.
    pub scale: [f32; 3],
}

impl VfxBinderTarget {
    /// Independent bTSd output of target query 0x1403bef20. The client calls
    /// 0x1403bf740 before bTSc replaces the three target lengths with ones,
    /// and before VFX bias/Document scale are applied. This is the query's
    /// transform-depth factor, not the final particle DpOf multiplier.
    pub fn query_depth_scale(&self, binder: &AvfxBinder, camera_position: [f32; 3]) -> f32 {
        if !binder.transform_scale_depth_offset {
            return 1.0;
        }
        let ray: [f32; 3] = std::array::from_fn(|axis| camera_position[axis] - self.position[axis]);
        let length = ((ray[0] * ray[0] + ray[1] * ray[1]) + ray[2] * ray[2]).sqrt();
        let inverse = 1.0 / length;
        let ray = ray.map(|value| value * inverse);
        let projected: [f32; 3] = std::array::from_fn(|column| {
            // Match native y+x+z ordering, preserving shear and mirror signs.
            let dot = (ray[1] * self.basis[column][1] + ray[0] * self.basis[column][0])
                + ray[2] * self.basis[column][2];
            dot * self.scale[column]
        });
        // No zero-ray substitution: the native helper produces NaN when the
        // camera coincides with the target. Geometry guards belong downstream.
        ((projected[1] * projected[1] + projected[0] * projected[0]) + projected[2] * projected[2])
            .sqrt()
    }

    /// Scale part of 0x1403bef20 with an already resolved listener VFX scale.
    /// bTSd's camera helper and failed listener queries are handled upstream.
    /// bBET uses the separate uniform-factor branch only for self targets.
    pub fn query_scale(
        &self,
        binder: &AvfxBinder,
        listener_vfx_scale: f32,
        self_target: bool,
    ) -> VfxBinderQueryScale {
        let transform_enabled = binder.transform_scale as u8 != 0;
        let scale = if transform_enabled {
            self.scale
        } else {
            [1.0; 3]
        };
        let bias = if transform_enabled || binder.vfx_scale_enabled {
            binder.vfx_scale_bias
        } else {
            1.0
        };
        let vfx_scale = if binder.vfx_scale_enabled || binder.vfx_scale_depth_offset {
            listener_vfx_scale
        } else {
            1.0
        };
        let target_scale = if self_target && binder.bet {
            if binder.vfx_scale_enabled {
                let factor = (vfx_scale - 1.0) * bias + 1.0;
                scale.map(|value| value * factor)
            } else {
                scale
            }
        } else {
            scale.map(|value| {
                let value = if binder.vfx_scale_enabled {
                    value * vfx_scale
                } else {
                    value
                };
                (value - 1.0) * bias + 1.0
            })
        };
        VfxBinderQueryScale {
            target_scale,
            vfx_scale,
        }
    }

    /// Consume a column-major target matrix. Preserve shear and mirrored
    /// columns: the client normalizes each column independently, rather than
    /// extracting an orthogonal rotation or reconstructing TRS.
    pub fn from_matrix(matrix: [f32; 16], following_target_orientation: bool) -> Self {
        let columns: [[f32; 3]; 3] =
            std::array::from_fn(|column| std::array::from_fn(|row| matrix[column * 4 + row]));
        let scale = columns.map(|[x, y, z]| {
            let length = ((y * y + x * x) + z * z).sqrt();
            if following_target_orientation {
                length + f32::EPSILON
            } else {
                length
            }
        });
        let position = [matrix[12], matrix[13], matrix[14]];
        if !following_target_orientation {
            return Self {
                position,
                basis: VFX_IDENTITY_BASIS,
                scale,
            };
        }
        // 3bf4d3..6b3 multiplies the full raw basis by a diagonal inverse.
        // Its zero products still participate: NaN/Inf in one column can
        // contaminate other columns and even translation. A direct per-column
        // division incorrectly drops that behavior (and signed-zero additions).
        let inverses = scale.map(|value| 1.0 / value);
        let basis = std::array::from_fn(|column| {
            std::array::from_fn(|row| {
                let factors: [f32; 3] = std::array::from_fn(|axis| {
                    if axis == column {
                        inverses[column]
                    } else {
                        0.0
                    }
                });
                let x = columns[0][row] * factors[0];
                let y = columns[1][row] * factors[1];
                let z = columns[2][row] * factors[2];
                (x + y) + z
            })
        });
        let position = std::array::from_fn(|row| {
            ((columns[1][row] * 0.0 + columns[0][row] * 0.0) + columns[2][row] * 0.0)
                + position[row]
        });
        Self {
            position,
            basis,
            scale,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn numeric_fade_uses_cached_alpha_and_last_scaled_delta_independently_of_time() {
        let mut instance = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 100, 0.0));
        instance.numeric.alpha = 0.8;
        instance.configure_fade(4, 2, false);
        instance.advance_time(1.0);
        assert_eq!(instance.numeric.fade.age, 0.0);
        assert_eq!(instance.numeric.alpha, 0.8);
        instance.refresh_numeric();
        assert_eq!(instance.numeric.fade.age, 1.0);
        assert_eq!(instance.numeric.alpha, 0.8 * 0.75);
        instance.set_time_advance_enabled(false);
        instance.advance_time(100.0);
        assert_eq!(instance.clock.local_age, 1.0);
        assert_eq!(instance.scaled_delta, 1.0);
        instance.refresh_numeric();
        assert_eq!(instance.numeric.fade.age, 2.0);
        assert_eq!(instance.numeric.alpha, (0.8 * 0.75) * 0.5);
        instance.set_registered_children(2);
        for _ in 0..3 {
            instance.refresh_numeric();
        }
        assert!(instance.retired());
        assert_eq!(instance.clock.local_age, 1.0);
        assert_eq!(instance.registered_children(), 2);
        let numeric = instance.numeric;
        instance.advance_time(100.0);
        instance.refresh_numeric();
        assert_eq!(instance.numeric, numeric);
    }

    #[test]
    fn point_camera_rotation_runs_after_property_offset_and_survives_query_skip() {
        let basis = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let camera = VfxBinderCameraSnapshot {
            basis,
            parallel_direction: [0.0, 0.0, 1.0],
            position: [10.0, 0.0, 0.0],
        };
        let binder = AvfxBinder {
            rotation_type: 257,
            ..Default::default()
        };
        let mut state = cached_point();
        let frame = VfxPointBinderFrame {
            camera: Some(camera),
            document_scale: [1.0; 3],
            ..query_frame(1.0, 0.0)
        };
        let run = |state: &mut VfxPointBinderState| {
            state.update_frame(
                &binder,
                frame,
                || panic!("CoUF skips query"),
                |_| panic!(),
                0.0,
                [1.0, 0.0, 0.0],
            )
        };
        assert_eq!(run(&mut state), VfxBinderQueryStatus::Skipped);
        assert_eq!(state.matrix.position, [1.0, 0.0, 0.0]);
        assert_eq!(state.matrix.basis, basis);
        run(&mut state);
        assert_eq!(state.matrix.position, [1.0, 1.0, 0.0]);
        assert_eq!(state.target, VfxBinderMatrix::IDENTITY);
    }

    #[test]
    fn binder_camera_parallel_and_perspective_use_independent_vectors_and_reject_degenerate() {
        let mut camera = VfxBinderCameraSnapshot {
            basis: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]],
            parallel_direction: [1.0, 0.0, 0.0],
            position: [0.0, 0.0, 3.0],
        };
        let mut parallel = VfxBinderMatrix::IDENTITY;
        parallel.apply_camera_rotation(258, Some(camera));
        assert_eq!(
            parallel.basis,
            [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]
        );
        let mut perspective = VfxBinderMatrix::IDENTITY;
        perspective.apply_camera_rotation(259, Some(camera));
        assert_eq!(perspective.basis, VFX_IDENTITY_BASIS);
        for direction in [[0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 0.009]] {
            camera.parallel_direction = direction;
            let mut current = VfxBinderMatrix {
                basis: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, -4.0]],
                ..VfxBinderMatrix::IDENTITY
            };
            let previous = current;
            current.apply_camera_rotation(2, Some(camera));
            assert_eq!(current, previous);
        }
        let mut direct = VfxBinderMatrix::IDENTITY;
        direct.apply_camera_rotation(1, Some(camera));
        assert_eq!(direct.basis, camera.basis); // No normalization of TLS columns.
    }

    #[test]
    fn point_property_matrix_keeps_nonfinite_zero_products_before_camera_override() {
        let mut state = cached_point();
        state.target.basis[1][0] = f32::NAN;
        state.update(None, [1.0; 3], 1.0, [0.0; 3]);
        assert!(state.matrix.basis.iter().all(|column| column[0].is_nan()));
        assert!(state.matrix.position[0].is_nan());
    }

    #[test]
    fn binder_normal_prepare_keeps_delayed_reset_and_time_pass_does_not_start_it() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 30, 0.5));
        life.advance_time(0.75);
        assert_eq!(life.clock.local_age, 0.75);
        assert_eq!(life.clock.delay, 0.5);
        let mut observations = Vec::new();
        life.prepare_self(|state, phase| {
            observations.push((phase, state.clock.local_age, state.clock.previous_age));
        });
        assert_eq!(
            observations,
            [
                (VfxBinderStep::Update, 0.75, 0.75),
                (VfxBinderStep::Initialize, 0.25, 0.75)
            ]
        );
        assert_eq!(life.clock.local_age, 0.25);
        assert_eq!(life.clock.total_age, 0.25);
        assert_eq!(life.clock.delay, -0.25);
        life.advance_time(0.0);
        life.prepare_self(|state, phase| {
            assert_eq!(phase, VfxBinderStep::Update);
            assert_eq!(state.clock.local_age, 0.25);
        });
        assert_eq!(life.scaled_delta.to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn binder_normal_negative_life_checks_empty_children_only_during_prepare() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, -1, 10.0));
        life.advance_time(1.0);
        assert!(!life.retired());
        life.prepare_self(|_, phase| assert_eq!(phase, VfxBinderStep::Update));
        assert!(life.retired());
        assert_eq!(life.clock.delay, 9.0);
        life.advance_time(20.0);
        life.prepare_self(|_, _| panic!("retired prepare is disabled"));
        assert_eq!(life.clock.local_age, 1.0);
        assert_eq!(life.clock.previous_age, 21.0);
    }

    #[test]
    fn binder_normal_time_and_prepare_controls_are_independent() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 30, 0.5));
        life.scaled_delta = 0.75;
        life.set_time_advance_enabled(false);
        life.advance_time(2.0);
        assert_eq!(life.clock.local_age, 0.0);
        assert_eq!(life.clock.previous_age, 2.0);
        assert_eq!(life.scaled_delta, 0.75);
        life.prepare_self(|_, _| {});
        assert_eq!(life.clock.local_age, 0.25);
        life.set_prepare_enabled(false);
        life.set_time_advance_enabled(true);
        life.advance_time(1.0);
        life.prepare_self(|_, _| panic!("disabled prepare"));
        assert_eq!(life.clock.local_age, 1.25);
        assert_eq!(life.clock.previous_age, 3.0);
    }

    #[test]
    fn binder_normal_life_clamp_disables_later_prepare_before_any_query() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 1, 0.0));
        life.advance_time(1.0);
        let mut updates = 0;
        life.prepare_self(|_, phase| {
            assert_eq!(phase, VfxBinderStep::Update);
            updates += 1;
        });
        assert_eq!(updates, 1);
        life.advance_time(0.25);
        life.prepare_self(|_, _| panic!("clamp cleared prepare flags"));
        assert!(life.retired());
        assert_eq!(life.clock.local_age, 1.0);
        assert_eq!(life.clock.total_age, 1.25);
    }

    #[test]
    fn linear_normal_prepare_queries_use_committed_age_and_authored_deadlines() {
        use crate::avfx::AvfxBinderProperties;
        let binder = AvfxBinder {
            binder_type: 1,
            life: 30,
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 0,
                ..Default::default()
            }),
            properties_goal: Some(AvfxBinderProperties {
                coord_update_frame: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut frame = linear_frame();
        frame.age = 0.0;
        let birth = VfxLinearBinderState::construct(
            &binder,
            frame,
            0.0,
            |endpoint| Some(linear_target([endpoint as f32 * 4.0, 0.0, 0.0])),
            |_, _| false,
            || linear_curves(0.5),
            VfxBinderMatrix::IDENTITY,
        );
        let mut instance = birth.into_instance(1);
        instance.advance_time(1.0);
        let mut curve_clocks = Vec::new();
        let result = instance.prepare_self(
            &binder,
            frame,
            |_| panic!("deadline exceeded after +68"),
            |_, _| false,
            |age, total| {
                curve_clocks.push([age, total]);
                linear_curves(0.5)
            },
            VfxBinderMatrix::IDENTITY,
            |_, _, _| panic!("ordinary update cannot attach"),
        );
        assert_eq!(result.update, Some([VfxBinderQueryStatus::Skipped; 2]));
        assert_eq!(result.initialization, None);
        assert_eq!(curve_clocks, [[1.0, 1.0]]);
        assert_eq!(instance.lifecycle.clock.local_age, 1.0);
    }

    #[test]
    fn linear_normal_delayed_factory_and_following_curves_retain_overshoot_clock() {
        let binder = AvfxBinder {
            binder_type: 1,
            life: 30,
            ..Default::default()
        };
        let mut frame = linear_frame();
        frame.age = 0.0;
        let mut instance = VfxLinearBinderState::construct(
            &binder,
            frame,
            0.5,
            |_| panic!("deferred"),
            |_, _| false,
            || panic!("deferred"),
            VfxBinderMatrix::IDENTITY,
        )
        .into_instance(0);
        instance.advance_time(0.75);
        let first = instance.prepare_self(
            &binder,
            frame,
            |endpoint| Some(linear_target([endpoint as f32 * 4.0, 0.0, 0.0])),
            |_, _| false,
            |age, total| {
                assert_eq!([age, total], [0.25; 2]);
                linear_curves(0.5)
            },
            VfxBinderMatrix::IDENTITY,
            |state, _, clock| {
                assert_eq!(clock.local_age, 0.25);
                assert_eq!(clock.previous_age, 0.75);
                assert_eq!(state.matrix.position, [2.0, 0.0, 0.0]);
                true
            },
        );
        assert!(first.initialization.unwrap().child_direction.is_some());
        assert_eq!(instance.lifecycle.clock.local_age, 0.25);
        instance.advance_time(0.25);
        let second = instance.prepare_self(
            &binder,
            frame,
            |_| Some(linear_target([1.0, 2.0, 3.0])),
            |_, _| false,
            |age, total| {
                assert_eq!([age, total], [0.5; 2]);
                linear_curves(0.5)
            },
            VfxBinderMatrix::IDENTITY,
            |_, _, _| panic!("no second factory call"),
        );
        assert!(second.update.is_some());
        assert_eq!(instance.lifecycle.clock.previous_age, 1.0);
        assert_eq!(instance.lifecycle.registered_children(), 1);
    }

    #[test]
    fn binder_delayed_initialize_observes_overshoot_before_common_commits_clocks() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(4.0, 30, 0.5));
        life.clock.rate = 2.0;
        let mut observations = Vec::new();
        life.advance_self(0.375, |state, phase| {
            observations.push((phase, state.clock.local_age, state.clock.total_age));
        });
        assert_eq!(
            observations,
            [
                (VfxBinderStep::Update, 4.0, 4.0),
                (VfxBinderStep::Initialize, 0.5, 0.5)
            ]
        );
        assert_eq!(life.clock.delay, -0.25);
        assert_eq!(life.clock.local_age, 4.75);
        assert_eq!(life.clock.total_age, 4.75);
        assert_eq!(life.clock.previous_age, 4.75);
        life.advance_self(1.0, |_, phase| assert_eq!(phase, VfxBinderStep::Update));
    }

    #[test]
    fn binder_life_boundary_is_strict_and_retired_clocks_keep_previous_age_only() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 1, 0.5));
        life.advance_self(1.0, |state, phase| {
            if phase == VfxBinderStep::Initialize {
                state.register_child();
            }
        });
        assert!(!life.retired());
        let delta = life.scaled_delta;
        life.advance_self(0.25, |_, _| panic!("age beyond life must skip own update"));
        assert!(life.retired());
        assert_eq!(life.clock.local_age, 1.0);
        assert_eq!(life.clock.total_age, 1.25);
        assert_eq!(life.registered_children(), 1);
        life.advance_self(2.0, |_, _| panic!("retired own update"));
        assert_eq!(life.clock.previous_age, 3.25);
        assert_eq!(life.clock.total_age, 1.25);
        assert_eq!(life.scaled_delta, 0.25);
        assert_eq!(delta, 1.0);
    }

    #[test]
    fn binder_unbounded_empty_parent_retires_even_while_waiting_for_delay() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, -1, 10.0));
        life.advance_self(1.0, |_, phase| assert_eq!(phase, VfxBinderStep::Update));
        assert!(life.retired());
        assert_eq!(life.clock.delay, 9.0);
        life.advance_self(20.0, |_, _| {
            panic!("retired deferred Binder cannot initialize")
        });
        assert_eq!(life.clock.delay, 9.0);
        assert_eq!(life.clock.previous_age, 21.0);
    }

    #[test]
    fn binder_unordered_delay_and_life_never_trigger_start_or_age_clamp() {
        let mut life = VfxBinderLifecycle::new(VfxBinderBirthClock::new(0.0, 30, f32::NAN));
        life.clock.nominal_life = f32::NAN;
        life.advance_self(100.0, |_, phase| assert_eq!(phase, VfxBinderStep::Update));
        assert!(!life.retired());
        assert_eq!(life.clock.local_age, 100.0);
        assert!(life.clock.delay.is_nan());
    }

    #[test]
    fn linear_failed_delayed_initialization_can_refresh_but_never_retry_child_creation() {
        let binder = AvfxBinder {
            binder_type: 1,
            life: 30,
            ..Default::default()
        };
        let mut frame = linear_frame();
        frame.age = 0.0;
        let mut instance = VfxLinearBinderState::construct(
            &binder,
            frame,
            0.5,
            |_| panic!("deferred"),
            |_, _| panic!("deferred"),
            || panic!("deferred"),
            VfxBinderMatrix::IDENTITY,
        )
        .into_instance(0);
        let first = instance.advance_self(
            &binder,
            1.0,
            frame,
            |_| None,
            |_, _| panic!("failed target"),
            |_, _| panic!("initial target failure must not sample curves"),
            VfxBinderMatrix::IDENTITY,
            |_, _, _| panic!("initial target failure must not attach"),
        );
        assert!(instance.initialized);
        assert!(first.initialization.unwrap().child_direction.is_none());
        let mut curve_clocks = Vec::new();
        let second = instance.advance_self(
            &binder,
            1.0,
            frame,
            |_| Some(linear_target([2.0, 3.0, 4.0])),
            |_, _| panic!("scale disabled"),
            |age, total| {
                curve_clocks.push([age, total]);
                linear_curves(0.5)
            },
            VfxBinderMatrix::IDENTITY,
            |_, _, _| panic!("ordinary update must not retry factory"),
        );
        assert_eq!(second.initialization, None);
        assert_eq!(second.update, Some([VfxBinderQueryStatus::Refreshed; 2]));
        assert_eq!(curve_clocks, [[1.0, 1.0]]);
        assert_eq!(instance.lifecycle.registered_children(), 0);
        assert_eq!(instance.lifecycle.clock.local_age, 2.0);
    }

    #[test]
    fn linear_delayed_child_factory_observes_initialized_matrix_and_real_attachment_count() {
        let binder = AvfxBinder {
            binder_type: 1,
            life: 30,
            ..Default::default()
        };
        let mut frame = linear_frame();
        frame.age = 0.0;
        let birth = VfxLinearBinderState::construct(
            &binder,
            frame,
            0.5,
            |_| None,
            |_, _| false,
            || linear_curves(0.5),
            VfxBinderMatrix::IDENTITY,
        );
        for attaches in [false, true] {
            let mut instance = birth.into_instance(0);
            let mut curve_clocks = Vec::new();
            let result = instance.advance_self(
                &binder,
                0.75,
                frame,
                |endpoint| Some(linear_target([endpoint as f32 * 4.0, 0.0, 0.0])),
                |_, _| false,
                |age, total| {
                    curve_clocks.push([age, total]);
                    linear_curves(0.5)
                },
                VfxBinderMatrix::IDENTITY,
                |state, initialization, clock| {
                    assert_eq!(clock.local_age, 0.25);
                    assert_eq!(clock.total_age, 0.25);
                    assert_eq!(clock.previous_age, 0.75);
                    assert_eq!(initialization.child_direction, Some([4.0, 0.0, 0.0]));
                    assert_eq!(state.matrix.position, [2.0, 0.0, 0.0]);
                    attaches
                },
            );
            assert_eq!(result.update, None);
            assert_eq!(curve_clocks, [[0.25, 0.25]]);
            assert_eq!(instance.lifecycle.clock.local_age, 0.75);
            assert_eq!(
                instance.lifecycle.registered_children(),
                u16::from(attaches)
            );
        }
    }

    fn linear_curves(factor: f32) -> VfxLinearBinderCurves {
        VfxLinearBinderCurves {
            factor,
            start_position: [0.0; 3],
            goal_position: [0.0; 3],
            start_position_after: [0.0; 3],
        }
    }

    fn linear_frame() -> VfxLinearBinderFrame {
        VfxLinearBinderFrame {
            age: 1.0,
            query_deadlines: [-1.0; 2],
            camera_position: [1.0, 2.0, 7.0],
            camera: None,
            document_scale: [2.0, 3.0, 4.0],
            self_targets: [true; 2],
            root_ags: false,
        }
    }

    fn linear_target(position: [f32; 3]) -> VfxBinderTarget {
        VfxBinderTarget {
            position,
            basis: VFX_IDENTITY_BASIS,
            scale: [2.0, 3.0, 4.0],
        }
    }

    #[test]
    fn linear_deferred_construction_preserves_document_defaults_and_signed_zero_clocks() {
        use crate::avfx::AvfxBinderProperties;
        let binder = AvfxBinder {
            binder_type: 1,
            life: 30,
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 65536,
                ..Default::default()
            }),
            properties_goal: Some(AvfxBinderProperties {
                coord_update_frame: 65535,
                ..Default::default()
            }),
            ..Default::default()
        };
        let document_scale = [-2.0, 0.0, 3.0];
        for delay in [0.5, f32::NAN, f32::INFINITY] {
            let birth = VfxLinearBinderState::construct(
                &binder,
                VfxLinearBinderFrame {
                    age: -0.0,
                    document_scale,
                    ..linear_frame()
                },
                delay,
                |_| panic!("deferred construction must not query"),
                |_, _| panic!("deferred construction must not query scale"),
                || panic!("deferred construction must not sample curves"),
                VfxBinderMatrix::IDENTITY,
            );
            assert_eq!(birth.initialization, None);
            for age in [
                birth.clock.local_age,
                birth.clock.total_age,
                birth.clock.previous_age,
            ] {
                assert_eq!(age.to_bits(), (-0.0f32).to_bits());
            }
            assert_eq!(birth.clock.nominal_life, 30.0);
            assert_eq!(birth.clock.rate, 1.0);
            assert_eq!(birth.clock.delay.to_bits(), delay.to_bits());
            assert_eq!(birth.state.matrix, VfxBinderMatrix::IDENTITY);
            assert_eq!(
                birth.state.auxiliary_matrix,
                VfxBinderMatrix::scale_matrix(document_scale)
            );
            assert_eq!(birth.state.scale, document_scale);
            assert_eq!(
                birth.state.targets,
                [VfxBinderMatrix {
                    basis: [[0.0; 3]; 3],
                    position: [0.0; 3]
                }; 2]
            );
            assert_eq!(birth.state.start_property_world, [1.0; 3]);
            assert_eq!(birth.state.endpoint_vfx_scales, [0.0; 2]);
            assert_eq!(birth.state.endpoint_depth_scales, [1.0; 2]);
            assert_eq!(birth.state.vfx_scale, 0.0);
            assert_eq!(birth.state.transform_depth_scale, 1.0);
        }
    }

    #[test]
    fn linear_immediate_construction_uses_authored_deadlines_without_subtracting_delay() {
        use crate::avfx::AvfxBinderProperties;
        let binder = AvfxBinder {
            binder_type: 1,
            life: i32::MAX,
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 0,
                ..Default::default()
            }),
            properties_goal: Some(AvfxBinderProperties {
                coord_update_frame: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        for delay in [-1.0, 0.0, -0.0, f32::NEG_INFINITY] {
            let mut calls = [0; 2];
            let birth = VfxLinearBinderState::construct(
                &binder,
                VfxLinearBinderFrame {
                    age: 2.5,
                    query_deadlines: [-1.0; 2],
                    ..linear_frame()
                },
                delay,
                |endpoint| {
                    calls[endpoint] += 1;
                    Some(linear_target(if endpoint == 0 {
                        [1.0, 2.0, 3.0]
                    } else {
                        [5.0, 6.0, 7.0]
                    }))
                },
                |_, _| panic!("disabled scalar cannot query listener"),
                || linear_curves(0.5),
                VfxBinderMatrix::IDENTITY,
            );
            assert_eq!(calls, [1, 1]);
            let initialized = birth.initialization.unwrap();
            assert_eq!(
                initialized.queries,
                [Some(VfxBinderQueryStatus::Refreshed); 2]
            );
            assert_eq!(initialized.update, Some([VfxBinderQueryStatus::Skipped; 2]));
            assert_eq!(initialized.child_direction, Some([4.0; 3]));
            assert_eq!(birth.state.matrix.position, [3.0, 4.0, 5.0]);
            assert_eq!(birth.clock.local_age, 2.5);
            assert_eq!(birth.clock.previous_age, 2.5);
            assert_eq!(birth.clock.total_age, 2.5);
            assert_eq!(
                birth.clock.nominal_life.to_bits(),
                (i32::MAX as f32).to_bits()
            );
        }
    }

    #[test]
    fn linear_failed_constructed_first_query_keeps_zero_targets_and_document_auxiliary() {
        let binder = AvfxBinder {
            binder_type: 1,
            life: -1,
            ..Default::default()
        };
        let frame = VfxLinearBinderFrame {
            root_ags: true,
            document_scale: [-2.0, 0.0, 3.0],
            ..linear_frame()
        };
        let birth = VfxLinearBinderState::construct(
            &binder,
            frame,
            0.0,
            |endpoint| {
                assert_eq!(endpoint, 0);
                None
            },
            |_, _| panic!("failed target cannot query scale"),
            || panic!("failed initial query cannot sample curves"),
            VfxBinderMatrix {
                position: [10.0; 3],
                ..VfxBinderMatrix::IDENTITY
            },
        );
        assert_eq!(
            birth.initialization,
            Some(VfxLinearBinderInitialization {
                queries: [Some(VfxBinderQueryStatus::TargetUnavailable), None],
                update: None,
                child_direction: None,
            })
        );
        assert_eq!(
            birth.state,
            VfxLinearBinderState::constructed(frame.document_scale)
        );
        assert_eq!(birth.clock.nominal_life, -1.0);
    }

    #[test]
    fn linear_initial_start_failure_never_queries_goal_or_evaluates_birth_curves() {
        let binder = AvfxBinder {
            binder_type: 1,
            transform_scale: 255,
            vfx_scale_enabled: true,
            ..Default::default()
        };
        for target_ok in [false, true] {
            let mut state = VfxLinearBinderState::new(VfxBinderMatrix {
                position: [10.0, 20.0, 30.0],
                ..VfxBinderMatrix::IDENTITY
            });
            state.auxiliary_matrix = VfxBinderMatrix::scale_matrix([9.0; 3]);
            state.scale = [9.0; 3];
            state.vfx_scale = 5.0;
            state.transform_depth_scale = 6.0;
            state.endpoint_depth_scales = [7.0, 8.0];
            let initial = state;
            let result = state.initialize(
                &binder,
                VfxLinearBinderFrame {
                    query_deadlines: [f32::NAN; 2],
                    root_ags: true,
                    ..linear_frame()
                },
                |endpoint| {
                    assert_eq!(endpoint, 0);
                    target_ok.then_some(linear_target([1.0, 2.0, 3.0]))
                },
                |endpoint, value| {
                    assert!(target_ok);
                    assert_eq!(endpoint, 0);
                    *value = 2.0;
                    false
                },
                || panic!("initial failure cannot evaluate curves"),
                VfxBinderMatrix::IDENTITY,
            );
            assert_eq!(
                result,
                VfxLinearBinderInitialization {
                    queries: [
                        Some(if target_ok {
                            VfxBinderQueryStatus::ScaleUnavailable
                        } else {
                            VfxBinderQueryStatus::TargetUnavailable
                        }),
                        None,
                    ],
                    update: None,
                    child_direction: None,
                }
            );
            assert_eq!(state.matrix, initial.matrix);
            assert_eq!(state.auxiliary_matrix, initial.auxiliary_matrix);
            assert_eq!(state.vfx_scale, 5.0);
            assert_eq!(state.transform_depth_scale, 6.0);
            assert_eq!(state.endpoint_depth_scales, [1.0, 8.0]);
            assert_eq!(
                state.scale,
                if target_ok { [2.0, 3.0, 4.0] } else { [9.0; 3] }
            );
        }
    }

    #[test]
    fn linear_initial_goal_failure_keeps_successful_start_unmultiplied_by_document() {
        let binder = AvfxBinder {
            binder_type: 1,
            transform_scale: 255,
            vfx_scale_enabled: true,
            vfx_scale_bias: 0.5,
            transform_scale_depth_offset: true,
            ..Default::default()
        };
        for goal_target_ok in [false, true] {
            let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
            state.auxiliary_matrix = VfxBinderMatrix::scale_matrix([9.0; 3]);
            let result = state.initialize(
                &binder,
                linear_frame(),
                |endpoint| {
                    (endpoint == 0 || goal_target_ok).then_some(linear_target([1.0, 2.0, 3.0]))
                },
                |endpoint, value| {
                    *value = 2.0;
                    endpoint == 0
                },
                || panic!("goal failure cannot evaluate curves"),
                VfxBinderMatrix::IDENTITY,
            );
            assert_eq!(result.queries[0], Some(VfxBinderQueryStatus::Refreshed));
            assert_eq!(
                result.queries[1],
                Some(if goal_target_ok {
                    VfxBinderQueryStatus::ScaleUnavailable
                } else {
                    VfxBinderQueryStatus::TargetUnavailable
                })
            );
            assert_eq!(result.update, None);
            assert_eq!(result.child_direction, None);
            assert_eq!(state.scale, [2.5, 3.5, 4.5]);
            assert_eq!(state.vfx_scale, 2.0);
            assert_eq!(state.transform_depth_scale, 4.0);
            assert_eq!(state.matrix, VfxBinderMatrix::IDENTITY);
            assert_eq!(
                state.auxiliary_matrix,
                VfxBinderMatrix::scale_matrix([9.0; 3])
            );
        }
    }

    #[test]
    fn linear_late_failures_still_create_child_with_updated_direction_and_root_auxiliary() {
        use std::cell::{Cell, RefCell};
        let binder = AvfxBinder {
            binder_type: 4,
            transform_scale: 255,
            vfx_scale_depth_offset: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        };
        let revision = VfxBinderMatrix {
            basis: [[0.0, -2.0, 0.0], [-3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
            position: [10.0, 20.0, 30.0],
        };
        for root_ags in [false, true] {
            let calls = Cell::new([0; 2]);
            let events = RefCell::new(Vec::new());
            let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
            let result = state.initialize(
                &binder,
                VfxLinearBinderFrame {
                    root_ags,
                    ..linear_frame()
                },
                |endpoint| {
                    events.borrow_mut().push(["start", "goal"][endpoint]);
                    let mut count = calls.get();
                    count[endpoint] += 1;
                    calls.set(count);
                    match (endpoint, count[endpoint]) {
                        (0, 1) => Some(linear_target([1.0, 2.0, 3.0])),
                        (1, 1) => Some(linear_target([5.0, 6.0, 7.0])),
                        (0, 2) => None,
                        (1, 2) => Some(linear_target([9.0, 20.0, 11.0])),
                        _ => unreachable!(),
                    }
                },
                |endpoint, value| {
                    events
                        .borrow_mut()
                        .push(["start scale", "goal scale"][endpoint]);
                    if calls.get()[endpoint] == 1 {
                        *value = 0.0;
                        true
                    } else {
                        *value = 2.0;
                        false
                    }
                },
                || {
                    events.borrow_mut().push("curves");
                    let mut curves = linear_curves(0.5);
                    curves.start_position = [0.5, 0.0, 0.0];
                    curves.goal_position = curves.start_position;
                    curves
                },
                revision,
            );
            assert_eq!(result.queries, [Some(VfxBinderQueryStatus::Refreshed); 2]);
            assert_eq!(
                result.update,
                Some([
                    VfxBinderQueryStatus::TargetUnavailable,
                    VfxBinderQueryStatus::ScaleUnavailable,
                ])
            );
            assert_eq!(result.child_direction, Some([8.0, 0.0, 8.0]));
            assert_eq!(state.targets[1].position, [9.0, 2.0, 11.0]);
            assert_eq!(state.matrix.position, [5.5, 2.0, 7.0]);
            assert_eq!(state.scale, [4.0, 9.0, 16.0]);
            assert_eq!(
                state.auxiliary_matrix,
                if root_ags {
                    revision
                } else {
                    VfxBinderMatrix::scale_matrix([4.0, 9.0, 16.0])
                }
            );
            assert_eq!(
                *events.borrow(),
                [
                    "start",
                    "start scale",
                    "goal",
                    "goal scale",
                    "curves",
                    "start",
                    "goal",
                    "goal scale",
                ]
            );
        }
    }

    #[test]
    fn linear_first_queries_ignore_signed_word_deadlines_and_child_axis_is_not_normalized() {
        use crate::avfx::AvfxBinderProperties;
        let mut binder = AvfxBinder {
            binder_type: 1,
            properties_start: Some(AvfxBinderProperties {
                coord_update_frame: 0x1_0000,
                ..Default::default()
            }),
            properties_goal: Some(AvfxBinderProperties {
                coord_update_frame: 0x1_ffff,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            VfxLinearBinderFrame::authored_query_deadlines(&binder),
            [0.0, -1.0]
        );
        for x in [0.009, 0.01] {
            let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
            let mut calls = [0; 2];
            let result = state.initialize(
                &binder,
                VfxLinearBinderFrame {
                    query_deadlines: VfxLinearBinderFrame::authored_query_deadlines(&binder),
                    ..linear_frame()
                },
                |endpoint| {
                    calls[endpoint] += 1;
                    let mut target = linear_target(if endpoint == 0 {
                        [0.0; 3]
                    } else {
                        [if calls[endpoint] == 1 { 10.0 } else { x }, 0.0, 0.0]
                    });
                    target.basis[2] = [0.0, 0.0, 3.0];
                    Some(target)
                },
                |_, _| panic!("disabled scalar does not call listener"),
                || linear_curves(0.0),
                VfxBinderMatrix::IDENTITY,
            );
            assert_eq!(calls, [1, 2]);
            assert_eq!(
                result.update,
                Some([
                    VfxBinderQueryStatus::Skipped,
                    VfxBinderQueryStatus::Refreshed,
                ])
            );
            assert_eq!(
                result.child_direction,
                Some(if x == 0.009 {
                    [0.0, 0.0, 3.0]
                } else {
                    [0.01, 0.0, 0.0]
                })
            );
        }
        binder.properties_start.as_mut().unwrap().coord_update_frame = 0x8000;
        binder.properties_goal.as_mut().unwrap().coord_update_frame = 0x1_7fff;
        assert_eq!(
            VfxLinearBinderFrame::authored_query_deadlines(&binder),
            [-32768.0, 32767.0]
        );
    }

    #[test]
    fn linear_global_direction_rebuilds_basis_or_preserves_start_on_rejection() {
        let binder = AvfxBinder {
            binder_type: 1,
            start_to_global_direction: true,
            ..Default::default()
        };
        let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        state.targets[0].basis = [[-2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]];
        state.targets[1].position = [0.0, 0.0, -2.0];
        state.update_resolved(&binder, linear_curves(1.5));
        assert_eq!(
            state.matrix.basis,
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]
        );
        assert_eq!(state.matrix.position, [0.0, 0.0, -3.0]);
        for goal in [[0.009, 0.0, 0.0], [0.0, 2.0, 0.0]] {
            state.targets[1].position = goal;
            state.update_resolved(&binder, linear_curves(0.5));
            assert_eq!(state.matrix.basis, state.targets[0].basis);
            assert_eq!(state.matrix.position, goal.map(|value| value * 0.5));
        }
        // At length² = threshold the client accepts the direction.
        state.targets[1].position = [0.01, 0.0, 0.0];
        state.update_resolved(&binder, linear_curves(0.0));
        assert_eq!(state.matrix.basis[2], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn linear_normalized_columns_keep_cancellation_and_degenerate_results() {
        let binder = AvfxBinder {
            binder_type: 1,
            ..Default::default()
        };
        let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        state.targets[0].basis[0] = [1e20, 0.0, 0.0];
        state.targets[1].basis[0] = [0.25, 0.0, 0.0];
        state.targets[0].position[0] = 1e20;
        state.targets[1].position[0] = 0.25;
        state.update_resolved(&binder, linear_curves(1.0));
        assert!(
            state
                .matrix
                .basis
                .iter()
                .flatten()
                .any(|value| value.is_nan())
        );
        // Linear's endpoint arithmetic is unlike Point's exact copy at SpS=1.
        assert!(state.matrix.position[0].is_nan());
        assert_eq!(state.targets[1].position[0], 0.25);
        state.targets = [VfxBinderMatrix::IDENTITY; 2];
        state.targets[1].basis[0] = [-1.0, 0.0, 0.0];
        state.update_resolved(&binder, linear_curves(0.5));
        assert!(state.matrix.basis[0].iter().all(|value| value.is_nan()));
    }

    #[test]
    fn linear_endpoint_deadlines_auxiliary_and_depth_caches_are_independent() {
        let binder = AvfxBinder {
            binder_type: 1,
            vfx_scale_depth_offset: true,
            vfx_scale_interpolation: true,
            transform_scale_depth_offset: true,
            transform_scale_interpolation: true,
            ..Default::default()
        };
        let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        state.endpoint_vfx_scales = [2.0, 6.0];
        state.endpoint_depth_scales = [3.0, 7.0];
        state.scale = [9.0, 8.0, 7.0];
        let auxiliary = state.auxiliary_matrix;
        let frame = VfxLinearBinderFrame {
            age: 1.0,
            query_deadlines: [0.0, -1.0],
            camera_position: [0.0, 0.0, 3.0],
            camera: None,
            document_scale: [2.0; 3],
            self_targets: [true; 2],
            root_ags: false,
        };
        let statuses = state.update_frame(
            &binder,
            frame,
            |endpoint| {
                assert_eq!(endpoint, 1);
                None
            },
            |_, _| panic!("failed target cannot call scale"),
            linear_curves(0.25),
        );
        assert_eq!(
            statuses,
            [
                VfxBinderQueryStatus::Skipped,
                VfxBinderQueryStatus::TargetUnavailable
            ]
        );
        assert_eq!(state.auxiliary_matrix, auxiliary);
        assert_eq!(state.scale, [9.0, 8.0, 7.0]);
        assert_eq!(state.endpoint_depth_scales, [3.0, 1.0]);
        assert_eq!(state.vfx_scale, 3.0);
        assert_eq!(state.transform_depth_scale, 2.5);
        assert_eq!(state.depth_offset_multiplier(&binder), 7.5);
        let statuses = state.update_frame(
            &binder,
            VfxLinearBinderFrame {
                query_deadlines: [-1.0, f32::NAN],
                ..frame
            },
            |_| None,
            |_, _| panic!(),
            linear_curves(0.25),
        );
        assert_eq!(
            statuses,
            [
                VfxBinderQueryStatus::TargetUnavailable,
                VfxBinderQueryStatus::Skipped
            ]
        );
        assert_eq!(
            state.auxiliary_matrix,
            VfxBinderMatrix::scale_matrix([9.0, 8.0, 7.0])
        );
    }

    #[test]
    fn linear_repeated_start_property_reader_and_horizontal_subtype_are_separate() {
        let mut state = VfxLinearBinderState::new(VfxBinderMatrix::IDENTITY);
        state.targets[0].position = [1.0, 2.0, 3.0];
        state.targets[1].position = [5.0, 6.0, 7.0];
        let mut curves = linear_curves(0.5);
        curves.start_position = [2.0; 3];
        curves.goal_position = [4.0; 3];
        curves.start_position_after = [8.0; 3];
        state.update_resolved(
            &AvfxBinder {
                binder_type: 4,
                ..Default::default()
            },
            curves,
        );
        assert_eq!(state.targets[1].position, [5.0, 2.0, 7.0]);
        assert_eq!(state.matrix.position, [6.0, 5.0, 8.0]);
        assert_eq!(state.start_property_world, [9.0, 10.0, 11.0]);
    }

    #[test]
    fn following_target_full_zero_products_propagate_nonfinite_columns() {
        let mut raw = [0.0; 16];
        raw[0] = f32::NAN;
        for index in [5, 10, 15] {
            raw[index] = 1.0;
        }
        raw[12..15].copy_from_slice(&[1.0, 2.0, 3.0]);
        let target = VfxBinderTarget::from_matrix(raw, true);
        assert!(target.basis.iter().all(|column| column[0].is_nan()));
        assert!(target.position[0].is_nan());
        assert_eq!(target.position[1..], [2.0, 3.0]);
        let target = VfxBinderTarget::from_matrix(raw, false);
        assert_eq!(target.basis, VFX_IDENTITY_BASIS);
        assert_eq!(target.position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn auxiliary_consumption_keeps_main_matrix_and_scale_separate() {
        use super::{VfxBinderMatrix, VfxPointBinderState};
        let local = VfxBinderMatrix {
            basis: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 0.5]],
            position: [1.0, 2.0, 3.0],
        };
        let mut state = VfxPointBinderState::new(local);
        state.scale = [7.0, 8.0, 9.0];
        state.auxiliary_matrix = VfxBinderMatrix {
            basis: [[2.0, 0.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, -4.0]],
            position: [10.0, 20.0, 30.0],
        };
        assert_eq!(
            state.transform_auxiliary(local),
            VfxBinderMatrix {
                basis: [[1.0, 3.0, 0.0], [-2.0, 0.0, 0.0], [0.0, 0.0, -2.0]],
                position: [14.0, 26.0, 18.0],
            }
        );
        assert_eq!(state.matrix, local);
        assert_eq!(state.scale, [7.0, 8.0, 9.0]);
    }

    use super::*;

    fn cached_point() -> VfxPointBinderState {
        let matrix = VfxBinderMatrix {
            basis: VFX_IDENTITY_BASIS,
            position: [0.0; 3],
        };
        VfxPointBinderState {
            matrix,
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            target: matrix,
            scale: [9.0; 3],
            vfx_scale: 0.0,
            transform_depth_scale: 3.0,
        }
    }

    fn query_frame(age: f32, deadline: f32) -> VfxPointBinderFrame {
        VfxPointBinderFrame {
            age,
            query_deadline: deadline,
            camera_position: [5.0, 0.0, 0.0],
            camera: None,
            document_scale: [2.0, 3.0, 4.0],
            root_revision: None,
            self_target: true,
        }
    }

    #[test]
    fn point_constructed_first_failure_does_not_consume_curves() {
        let binder = AvfxBinder {
            life: -1,
            ..Default::default()
        };
        let birth = VfxPointBinderState::construct(
            &binder,
            query_frame(2.0, -1.0),
            0.0,
            || None,
            |_| panic!("target failed before listener"),
            || panic!("initial failure must bypass curves"),
        );
        assert!(!birth.initialization.unwrap().child_ready());
        assert_eq!(birth.state.matrix, VfxBinderMatrix::IDENTITY);
        assert_eq!(birth.state.auxiliary_matrix, VfxBinderMatrix::IDENTITY);
        assert_eq!(birth.clock.local_age, 2.0);
        assert!(birth.into_instance(0).initialized);
    }

    #[test]
    fn point_persistent_normal_delay_uses_overshoot_for_curves_and_child_clock() {
        let binder = AvfxBinder {
            life: 30,
            ..Default::default()
        };
        let frame = query_frame(0.0, -1.0);
        let birth = VfxPointBinderState::construct(
            &binder,
            frame,
            0.5,
            || panic!("deferred query"),
            |_| panic!("deferred listener"),
            || panic!("deferred curves"),
        );
        assert_eq!(birth.initialization, None);
        let mut instance = birth.into_instance(0);
        instance.advance_time(0.25);
        let result = instance.prepare_self(
            &binder,
            frame,
            || panic!("still waiting"),
            |_| panic!("still waiting"),
            |_, _| panic!("still waiting"),
            |_, _, _| panic!("still waiting"),
        );
        assert_eq!(result, VfxPointBinderAdvance::default());
        instance.advance_time(0.5);
        let mut ages = Vec::new();
        let mut attachments = 0;
        let result = instance.prepare_self(
            &binder,
            frame,
            || Some(linear_target([8.0, 0.0, 0.0])),
            |_| true,
            |age, total| {
                ages.push((age, total));
                VfxPointBinderCurves {
                    spring: 1.0,
                    position: [age, 0.0, 0.0],
                }
            },
            |state, init, clock| {
                assert!(init.child_ready());
                assert_eq!(state.matrix.position[0], 8.25);
                assert_eq!(clock.local_age, 0.25);
                assert_eq!(clock.previous_age, 0.75);
                attachments += 1;
                true
            },
        );
        assert!(result.initialization.unwrap().child_ready());
        assert_eq!(ages, [(0.25, 0.25)]);
        assert_eq!(attachments, 1);
        assert_eq!(instance.lifecycle.clock.local_age, 0.25);
        assert_eq!(instance.lifecycle.registered_children(), 1);
        instance.advance_time(0.5);
        let result = instance.prepare_self(
            &binder,
            frame,
            || Some(linear_target([8.0, 0.0, 0.0])),
            |_| true,
            |age, total| {
                ages.push((age, total));
                VfxPointBinderCurves {
                    spring: 1.0,
                    position: [age, 0.0, 0.0],
                }
            },
            |_, _, _| panic!("must not recreate the child"),
        );
        assert_eq!(result.initialization, None);
        assert_eq!(ages, [(0.25, 0.25), (0.75, 0.75)]);
        assert_eq!(instance.state.matrix.position[0], 8.75);
    }

    #[test]
    fn point_failed_birth_recovers_numeric_cache_without_factory_retry() {
        let binder = AvfxBinder {
            life: -1,
            ..Default::default()
        };
        let frame = query_frame(0.0, -1.0);
        let mut instance = VfxPointBinderState::construct(
            &binder,
            frame,
            0.0,
            || None,
            |_| true,
            || panic!("first query failed"),
        )
        .into_instance(0);
        instance.advance_time(1.0);
        let result = instance.prepare_self(
            &binder,
            frame,
            || Some(linear_target([3.0, 0.0, 0.0])),
            |_| true,
            |age, total| {
                assert_eq!((age, total), (1.0, 1.0));
                VfxPointBinderCurves {
                    spring: 1.0,
                    position: [0.0; 3],
                }
            },
            |_, _, _| panic!("initialized failure cannot retry child creation"),
        );
        assert_eq!(result.update, Some(VfxBinderQueryStatus::Refreshed));
        assert_eq!(result.initialization, None);
        assert_eq!(instance.state.matrix.position[0], 3.0);
        assert!(instance.lifecycle.retired());
        assert_eq!(instance.lifecycle.registered_children(), 0);
    }

    #[test]
    fn point_lazy_curves_follow_queries_and_late_failure_keeps_factory_gate() {
        use std::cell::RefCell;
        let events = RefCell::new(Vec::new());
        let binder = AvfxBinder {
            life: -1,
            ..Default::default()
        };
        let mut calls = 0;
        let birth = VfxPointBinderState::construct(
            &binder,
            query_frame(0.0, -1.0),
            0.0,
            || {
                events.borrow_mut().push("target");
                calls += 1;
                (calls == 1).then(|| linear_target([10.0, 0.0, 0.0]))
            },
            |_| true,
            || {
                events.borrow_mut().push("curves");
                VfxPointBinderCurves {
                    spring: 0.5,
                    position: [0.0; 3],
                }
            },
        );
        assert_eq!(*events.borrow(), ["target", "target", "curves"]);
        let init = birth.initialization.unwrap();
        assert!(init.child_ready());
        assert_eq!(init.update, Some(VfxBinderQueryStatus::TargetUnavailable));
        assert_eq!(birth.state.matrix.position[0], 5.0);
    }

    #[test]
    fn point_auxiliary_matrix_refreshes_partial_scale_on_failure_but_not_deadline_skip() {
        let binder = AvfxBinder {
            transform_scale: 1,
            vfx_scale_enabled: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        };
        let target = VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [10.0; 3],
            scale: [2.0, 3.0, 4.0],
        };
        let mut state = VfxPointBinderState::constructed();
        state.update_frame(
            &binder,
            query_frame(0.0, -1.0),
            || Some(target),
            |out| {
                *out = 2.0;
                true
            },
            1.0,
            [0.0; 3],
        );
        assert_eq!(
            state.auxiliary_matrix,
            VfxBinderMatrix::scale_matrix([8.0, 18.0, 32.0])
        );
        let old = state.auxiliary_matrix;
        state.scale = [9.0; 3];
        state.update_frame(
            &binder,
            query_frame(1.0, 0.0),
            || panic!("query past deadline"),
            |_| false,
            0.0,
            [0.0; 3],
        );
        assert_eq!(state.auxiliary_matrix, old);
        // Target failure keeps scale but ordinary update still writes the
        // auxiliary diagonal from that retained scale.
        state.update_frame(
            &binder,
            query_frame(1.0, -1.0),
            || None,
            |_| false,
            0.0,
            [0.0; 3],
        );
        assert_eq!(
            state.auxiliary_matrix,
            VfxBinderMatrix::scale_matrix([9.0; 3])
        );
        state.vfx_scale = 0.0;
        state.update_frame(
            &binder,
            query_frame(2.0, -1.0),
            || Some(target),
            |_| false,
            0.0,
            [0.0; 3],
        );
        // Late scale failure has already written raw target lengths, without
        // VFX bias or Document multiplication.
        assert_eq!(
            state.auxiliary_matrix,
            VfxBinderMatrix::scale_matrix([2.0, 3.0, 4.0])
        );
    }

    #[test]
    fn point_root_revision_is_installed_only_after_successful_initialization_then_retained() {
        let binder = AvfxBinder::default();
        let revision = VfxBinderMatrix {
            basis: [[2.0, 1.0, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 0.0]],
            position: [4.0, 5.0, 6.0],
        };
        let frame = VfxPointBinderFrame {
            root_revision: Some(revision),
            ..query_frame(0.0, -1.0)
        };
        let mut state = VfxPointBinderState::constructed();
        assert_eq!(state.matrix, VfxBinderMatrix::IDENTITY);
        state.initialize_frame(&binder, frame, || None, |_| false, 1.0, [0.5; 3]);
        assert_eq!(state.matrix, VfxBinderMatrix::IDENTITY);
        assert_eq!(state.auxiliary_matrix, VfxBinderMatrix::IDENTITY);
        let target = VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [10.0; 3],
            scale: [2.0; 3],
        };
        state.initialize_frame(&binder, frame, || Some(target), |_| true, 1.0, [0.5; 3]);
        assert_eq!(state.matrix.position, [10.5; 3]);
        assert_eq!(state.auxiliary_matrix, revision);
        // bAGS suppresses ordinary query diagonal rewrites; even a supplied
        // changed root input only takes effect on a later initialization.
        let next = VfxPointBinderFrame {
            root_revision: Some(VfxBinderMatrix::IDENTITY),
            ..frame
        };
        state.update_frame(&binder, next, || Some(target), |_| true, 1.0, [0.0; 3]);
        assert_eq!(state.auxiliary_matrix, revision);
    }

    #[test]
    fn point_initialization_queries_before_deadline_and_reuses_scalar_on_second_target() {
        let binder = AvfxBinder {
            vfx_scale_enabled: true,
            vfx_scale_bias: 1.0,
            ..Default::default()
        };
        let initial = cached_point().matrix;
        let target = |x| VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [x, 0.0, 0.0],
            scale: [1.0; 3],
        };
        for (deadline, expected_calls, expected_x, status) in [
            (0.0, 1, 1.5, VfxBinderQueryStatus::Skipped),
            (1.0, 2, 2.5, VfxBinderQueryStatus::Refreshed),
        ] {
            let mut state = VfxPointBinderState::new(initial);
            let mut target_calls = 0;
            let mut scalar_calls = 0;
            let outcome = state.initialize_frame(
                &binder,
                query_frame(1.0, deadline),
                || {
                    target_calls += 1;
                    Some(target(target_calls as f32))
                },
                |value| {
                    scalar_calls += 1;
                    *value = 2.0;
                    true
                },
                1.0,
                [0.5, 0.0, 0.0],
            );
            assert_eq!(outcome.query, VfxBinderQueryStatus::Refreshed);
            assert_eq!(outcome.update, Some(status));
            assert_eq!(target_calls, expected_calls);
            assert_eq!(scalar_calls, 1);
            assert_eq!(state.matrix.position, [expected_x, 0.0, 0.0]);
            // Document is applied to each fresh query, not multiplied twice.
            assert_eq!(state.scale, [4.0, 6.0, 8.0]);
        }
    }

    #[test]
    fn point_initial_query_failure_keeps_matrix_and_partial_cache_without_property_offset() {
        let binder = AvfxBinder {
            vfx_scale_enabled: true,
            ..Default::default()
        };
        let initial = VfxBinderMatrix {
            position: [3.0, 4.0, 5.0],
            ..cached_point().matrix
        };
        for target_ok in [false, true] {
            let mut state = VfxPointBinderState::new(initial);
            let outcome = state.initialize_frame(
                &binder,
                query_frame(0.0, -1.0),
                || {
                    target_ok.then_some(VfxBinderTarget {
                        position: [10.0; 3],
                        basis: VFX_IDENTITY_BASIS,
                        scale: [1.0; 3],
                    })
                },
                |value| {
                    *value = 2.0;
                    false
                },
                1.0,
                [0.5, 0.25, 0.75],
            );
            assert_eq!(
                outcome.query,
                if target_ok {
                    VfxBinderQueryStatus::ScaleUnavailable
                } else {
                    VfxBinderQueryStatus::TargetUnavailable
                }
            );
            assert_eq!(outcome.update, None);
            assert_eq!(state.matrix, initial);
            assert_eq!(
                state.target.position,
                if target_ok { [10.0; 3] } else { [0.0; 3] }
            );
            assert_eq!(state.vfx_scale, if target_ok { 2.0 } else { 0.0 });
            assert_eq!(state.scale, [1.0; 3]);
        }
    }

    #[test]
    fn point_coordinate_deadline_consumes_signed_low_word() {
        use crate::avfx::AvfxBinderProperties;
        let mut binder = AvfxBinder::default();
        assert_eq!(VfxPointBinderFrame::authored_query_deadline(&binder), -1.0);
        for (raw, expected) in [
            (-1, -1.0),
            (32767, 32767.0),
            (32768, -32768.0),
            (65535, -1.0),
            (65536, 0.0),
            (-65537, -1.0),
            (i32::MAX, -1.0),
        ] {
            binder.properties_start = Some(AvfxBinderProperties {
                coord_update_frame: raw,
                ..Default::default()
            });
            assert_eq!(
                VfxPointBinderFrame::authored_query_deadline(&binder),
                expected
            );
        }
    }

    #[test]
    fn skipped_query_preserves_depth_but_failed_target_resets_it_before_spring() {
        let mut state = cached_point();
        let binder = AvfxBinder {
            transform_scale_depth_offset: true,
            ..Default::default()
        };
        for frame in [
            query_frame(1.0, 0.0),
            query_frame(0.0, f32::NAN),
            query_frame(f32::NAN, 1.0),
        ] {
            let status = state.update_frame(
                &binder,
                frame,
                || panic!("gated target callback"),
                |_| panic!("gated scalar callback"),
                1.0,
                [0.25, 0.0, 0.0],
            );
            assert_eq!(status, VfxBinderQueryStatus::Skipped);
            assert_eq!(state.transform_depth_scale, 3.0);
            assert_eq!(state.matrix.position, [0.25, 0.0, 0.0]);
        }
        let old = state;
        for frame in [query_frame(0.0, 0.0), query_frame(f32::NAN, -1.0)] {
            assert_eq!(
                state.update_frame(
                    &binder,
                    frame,
                    || None,
                    |_| panic!("failed target must not ask for scale"),
                    1.0,
                    [0.25, 0.0, 0.0]
                ),
                VfxBinderQueryStatus::TargetUnavailable
            );
            assert_eq!(state.transform_depth_scale, 1.0);
            assert_eq!(state.target, old.target);
            assert_eq!(state.scale, old.scale);
            assert_eq!(state.vfx_scale, old.vfx_scale);
        }
    }

    #[test]
    fn failed_scale_keeps_partial_query_writes_and_nonzero_cache_skips_listener() {
        let mut state = cached_point();
        let binder = AvfxBinder {
            transform_scale: 255,
            vfx_scale_enabled: true,
            vfx_scale_depth_offset: true,
            transform_scale_depth_offset: true,
            vfx_scale_bias: 2.0,
            ..Default::default()
        };
        let target = VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [1.0, 0.0, 0.0],
            scale: [2.0, 3.0, 4.0],
        };
        assert_eq!(
            state.update_frame(
                &binder,
                query_frame(0.0, -1.0),
                || Some(target),
                |scale| {
                    *scale = 0.5;
                    false
                },
                1.0,
                [0.25, 0.0, 0.0]
            ),
            VfxBinderQueryStatus::ScaleUnavailable
        );
        assert_eq!(state.target.position, target.position);
        assert_eq!(state.matrix.position, [1.25, 0.0, 0.0]);
        assert_eq!(state.scale, [2.0, 3.0, 4.0]); // no bias/Document multiplication
        assert_eq!(state.vfx_scale, 0.5);
        assert_eq!(state.transform_depth_scale, 2.0);
        assert_eq!(state.depth_offset_multiplier(&binder), 1.0);
        assert_eq!(
            state.update_frame(
                &binder,
                query_frame(1.0, -1.0),
                || Some(target),
                |_| panic!("cached nonzero scalar"),
                0.0,
                [0.0; 3]
            ),
            VfxBinderQueryStatus::Refreshed
        );
        assert_eq!(state.scale, [2.0, 6.0, 12.0]);
        state.vfx_scale = f32::NAN;
        assert_eq!(
            state.update_frame(
                &binder,
                query_frame(2.0, -1.0),
                || Some(target),
                |_| panic!("cached NaN scalar"),
                0.0,
                [0.0; 3]
            ),
            VfxBinderQueryStatus::Refreshed
        );
        assert!(state.scale.into_iter().all(f32::is_nan));
    }

    #[test]
    fn zero_scalar_is_queried_again_and_camera_depth_updates_independently() {
        let mut state = cached_point();
        let binder = AvfxBinder {
            vfx_scale_depth_offset: true,
            transform_scale_depth_offset: true,
            ..Default::default()
        };
        let target = VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [1.0, 0.0, 0.0],
            scale: [2.0, 3.0, 4.0],
        };
        let mut callbacks = 0;
        let mut frame = query_frame(0.0, -1.0);
        state.update_frame(
            &binder,
            frame,
            || Some(target),
            |scale| {
                callbacks += 1;
                *scale = -0.0;
                true
            },
            1.0,
            [0.0; 3],
        );
        assert_eq!(state.transform_depth_scale, 2.0);
        frame.camera_position = [1.0, 5.0, 0.0];
        state.update_frame(
            &binder,
            frame,
            || Some(target),
            |scale| {
                callbacks += 1;
                *scale = 2.0;
                true
            },
            1.0,
            [0.0; 3],
        );
        assert_eq!(callbacks, 2);
        assert_eq!(state.transform_depth_scale, 3.0);
        frame.camera_position = [1.0, 0.0, 5.0];
        state.update_frame(
            &binder,
            frame,
            || Some(target),
            |_| panic!("nonzero VFX cache is independent of camera"),
            1.0,
            [0.0; 3],
        );
        assert_eq!(state.transform_depth_scale, 4.0);
        assert_eq!(state.depth_offset_multiplier(&binder), 8.0);
    }

    #[test]
    fn transform_depth_uses_raw_lengths_before_scale_flags_and_bias() {
        let target = VfxBinderTarget {
            basis: VFX_IDENTITY_BASIS,
            position: [1.0, 2.0, 3.0],
            scale: [2.0, 3.0, 4.0],
        };
        let mut binder = AvfxBinder {
            transform_scale_depth_offset: true,
            vfx_scale_enabled: true,
            vfx_scale_bias: -2.0,
            ..Default::default()
        };
        assert_eq!(target.query_depth_scale(&binder, [6.0, 2.0, 3.0]), 2.0);
        assert_eq!(target.query_depth_scale(&binder, [1.0, -3.0, 3.0]), 3.0);
        assert_eq!(target.query_depth_scale(&binder, [1.0, 2.0, 8.0]), 4.0);
        let before = target.query_depth_scale(&binder, [4.0, 6.0, 3.0]);
        assert!((before - 7.2f32.sqrt()).abs() < 1e-6);
        binder.transform_scale = 255;
        binder.vfx_scale_bias = 99.0;
        binder.bet = true;
        binder.document_scale_enabled = true;
        assert_eq!(target.query_depth_scale(&binder, [4.0, 6.0, 3.0]), before);
    }

    #[test]
    fn transform_depth_retains_zero_ray_and_sheared_basis_semantics() {
        let target = VfxBinderTarget {
            basis: [[-1.0, 0.0, 0.0], [0.6, 0.8, 0.0], [0.0; 3]],
            position: [0.0; 3],
            scale: [2.0, 5.0, f32::EPSILON],
        };
        let mut binder = AvfxBinder {
            transform_scale_depth_offset: true,
            ..Default::default()
        };
        assert_eq!(
            target.query_depth_scale(&binder, [5.0, 0.0, 0.0]),
            13.0f32.sqrt()
        );
        assert!(target.query_depth_scale(&binder, [0.0; 3]).is_nan());
        assert_eq!(target.query_depth_scale(&binder, [0.0, 0.0, 5.0]), 0.0);
        binder.transform_scale_depth_offset = false;
        assert_eq!(target.query_depth_scale(&binder, [f32::NAN; 3]), 1.0);
    }

    #[test]
    fn point_update_keeps_cached_scale_and_applies_offset_after_matrix_spring() {
        let initial = VfxBinderMatrix {
            basis: VFX_IDENTITY_BASIS,
            position: [0.0; 3],
        };
        let mut state = VfxPointBinderState {
            matrix: initial,
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            target: initial,
            scale: [1.0; 3],
            vfx_scale: 1.0,
            transform_depth_scale: 1.0,
        };
        let target = VfxBinderTarget {
            basis: [[-1.0, 0.0, 0.0], [0.5, 1.0, 0.0], [0.0; 3]],
            position: [1.0, 2.0, 3.0],
            scale: [2.0, 3.0, 0.0],
        };
        let scale = VfxBinderQueryScale {
            target_scale: [2.0, 3.0, 4.0],
            vfx_scale: 2.0,
        };
        state.update(
            Some((target, scale)),
            [2.0, -0.5, 0.0],
            1.0,
            [2.0, 3.0, 4.0],
        );
        assert_eq!(state.matrix.basis, target.basis);
        assert_eq!(state.matrix.position, [0.5, 5.0, 3.0]);
        assert_eq!(state.scale, [4.0, -1.5, 0.0]);
        state.update(None, [99.0; 3], 0.0, [2.0, 3.0, 4.0]);
        assert_eq!(state.matrix.position, [0.0, 8.0, 3.0]);
        assert_eq!(state.scale, [4.0, -1.5, 0.0]);
        state.update(None, [99.0; 3], 1.0, [2.0, 3.0, 4.0]);
        assert_eq!(state.matrix.position, [0.5, 5.0, 3.0]);
    }

    #[test]
    fn point_spring_one_copies_target_without_cancellation() {
        let target = VfxBinderMatrix {
            basis: VFX_IDENTITY_BASIS,
            position: [0.25, 1.0, -1.0],
        };
        let mut state = VfxPointBinderState {
            matrix: VfxBinderMatrix {
                basis: [[1e20; 3]; 3],
                position: [1e20; 3],
            },
            auxiliary_matrix: VfxBinderMatrix::IDENTITY,
            target,
            scale: [1.0; 3],
            vfx_scale: 1.0,
            transform_depth_scale: 1.0,
        };
        state.update(None, [1.0; 3], 1.0, [0.5, 0.0, 0.0]);
        assert_eq!(state.matrix.basis, target.basis);
        assert_eq!(state.matrix.position, [0.75, 1.0, -1.0]);
    }

    #[test]
    fn query_scale_bias_and_bet_distinguish_self_from_external_target() {
        let target = VfxBinderTarget {
            position: [0.0; 3],
            basis: VFX_IDENTITY_BASIS,
            scale: [2.0, 3.0, 4.0],
        };
        let mut binder = AvfxBinder {
            transform_scale: 255,
            vfx_scale_enabled: true,
            vfx_scale_bias: 0.5,
            ..Default::default()
        };
        assert_eq!(
            target.query_scale(&binder, 2.0, true).target_scale,
            [2.5, 3.5, 4.5]
        );
        binder.bet = true;
        assert_eq!(
            target.query_scale(&binder, 2.0, true).target_scale,
            [3.0, 4.5, 6.0]
        );
        assert_eq!(
            target.query_scale(&binder, 2.0, false).target_scale,
            [2.5, 3.5, 4.5]
        );
        binder.transform_scale = 256;
        assert_eq!(
            target.query_scale(&binder, 2.0, true).target_scale,
            [1.5; 3]
        );
        binder.transform_scale = 257;
        assert_eq!(
            target.query_scale(&binder, 2.0, true).target_scale,
            [3.0, 4.5, 6.0]
        );
    }

    #[test]
    fn disabled_vfx_scale_does_not_multiply_target_but_retains_depth_source() {
        let target = VfxBinderTarget {
            position: [0.0; 3],
            basis: VFX_IDENTITY_BASIS,
            scale: [2.0, 3.0, 4.0],
        };
        let mut binder = AvfxBinder {
            transform_scale: 1,
            vfx_scale_bias: 0.25,
            ..Default::default()
        };
        let before = target.query_scale(&binder, 9.0, true);
        assert_eq!(before.target_scale, [1.25, 1.5, 1.75]);
        assert_eq!(before.vfx_scale, 1.0);
        binder.vfx_scale_depth_offset = true;
        let after = target.query_scale(&binder, 9.0, true);
        assert_eq!(after.target_scale, before.target_scale);
        assert_eq!(after.vfx_scale, 9.0);
        binder.bet = true;
        assert_eq!(
            target.query_scale(&binder, 9.0, true).target_scale,
            target.scale
        );
    }

    #[test]
    fn target_basis_retains_shear_mirroring_and_separate_scale() {
        let matrix = [
            -2.0, 0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 7.0, -8.0, 9.0, 1.0,
        ];
        let enabled = VfxBinderTarget::from_matrix(matrix, true);
        assert_eq!(enabled.position, [7.0, -8.0, 9.0]);
        assert_eq!(enabled.scale, [2.0, 5.0, f32::EPSILON]);
        assert_eq!(enabled.basis[0], [-1.0, 0.0, 0.0]);
        assert!((enabled.basis[1][0] - 0.6).abs() < 1e-7);
        assert!((enabled.basis[1][1] - 0.8).abs() < 1e-7);
        assert_eq!(enabled.basis[2], [0.0; 3]);
        let disabled = VfxBinderTarget::from_matrix(matrix, false);
        assert_eq!(disabled.position, enabled.position);
        assert_eq!(disabled.scale, [2.0, 5.0, 0.0]);
        assert_eq!(disabled.basis, VFX_IDENTITY_BASIS);
    }

    #[test]
    fn following_orientation_preserves_epsilon_in_scale_output() {
        let matrix = [
            f32::EPSILON,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            2.0,
            3.0,
            1.0,
        ];
        let target = VfxBinderTarget::from_matrix(matrix, true);
        assert_eq!(
            target.scale,
            [2.0 * f32::EPSILON, 1.0 + f32::EPSILON, f32::EPSILON]
        );
        assert_eq!(target.basis[0][0], 0.5);
        assert_eq!(target.basis[1][1], 1.0 / (1.0 + f32::EPSILON));
        assert_eq!(target.basis[2], [0.0; 3]);
    }
}
