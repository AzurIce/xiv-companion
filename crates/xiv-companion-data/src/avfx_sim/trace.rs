//! Shared staged Point-emitter instance management. The explicit-input trace
//! records scheduling; playback uses a narrower, independently checked subset.

use super::{
    AvfxEmitterItem, AvfxFile, CurveAges, InstanceClock, SplitMix64, VfxRuntime,
    curve_value_seeded_at, emitter_create_count_at, emitter_create_count_at_seeded,
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxTracePhase {
    Initialize,
    Advance,
    Prepare,
    Numeric,
    NullUpdate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxTraceTarget {
    Particle,
    Emitter,
    Helper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VfxTraceAction {
    Create,
    /// Observed by the parent after a child update, or at a helper's callback.
    Finish,
    Retire,
    /// A constructed object was destroyed after ordinary attachment to an
    /// already retired parent failed. It never became a registered child.
    AttachmentRejected,
    CapacityRejected,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxCreationEvent {
    /// Zero is construction; subsequent values index the supplied updates + 1.
    pub update: usize,
    pub phase: VfxTracePhase,
    pub action: VfxTraceAction,
    pub target: VfxTraceTarget,
    pub parent_instance: u64,
    pub instance: Option<u64>,
    pub emitter_index: usize,
    pub item_index: usize,
    pub target_index: usize,
    pub trigger: i32,
    /// The parent's stored age at the callback, before writeback during finish
    /// and null updates, after writeback during normal preparation.
    pub emitter_age: f32,
    /// Per-item signed-short counter, including capacity-rejected attempts
    /// after probability acceptance.
    pub injection_ordinal: Option<i16>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxCreationTrace {
    pub emitter_index: usize,
    pub root_life: f32,
    pub update_seconds: Vec<f32>,
    pub update_frames: Vec<f32>,
    pub remainder_frames: f32,
    pub root_age: f32,
    pub root_finished: bool,
    pub events: Vec<VfxCreationEvent>,
    /// Includes dead instances waiting for retirement; excludes the root.
    pub retained_instances: usize,
    pub live_particles: usize,
}

impl VfxRuntime {
    /// Trace one root emitter and its descendants using explicitly supplied
    /// Framework deltas. `root_life` is the Timeline-item duration, or -1.
    ///
    /// The probe rejects unsupported scheduling inputs (including random
    /// counts/lifetimes, PrLk and non-Point shapes) instead of silently making
    /// them client-equivalent. It assumes successful allocation, ordinary
    /// enabled Document updates at rate 1, manager divisor 60 and no external
    /// finish. It does not run
    /// Scheduler/Timeline, geometry, motion, binders or rendering.
    pub fn trace_emitter_updates(
        &self,
        emitter_index: usize,
        root_life: f32,
        update_seconds: &[f32],
    ) -> Result<VfxCreationTrace, String> {
        if !root_life.is_finite() {
            return Err("root lifetime must be finite (negative means unlimited)".into());
        }
        if update_seconds.len() > 100_000 {
            return Err("creation trace update limit".into());
        }
        validate(
            &self.file,
            emitter_index,
            &mut vec![false; self.file.emitters.len()],
            0,
        )?;
        let mut framework_clock = super::clock::FrameworkClock::default();
        let mut update_frames = Vec::with_capacity(update_seconds.len());
        for &seconds in update_seconds {
            update_frames.push(framework_clock.advance(seconds)?);
        }
        let mut context = Context {
            file: &self.file,
            events: Vec::new(),
            creation_path: Vec::new(),
            creation_source: None,
            root_constructor_cache: false,
            root_color_seed: None,
            particle_birth_root: None,
            color_environment: self.color_curve_environment()?,
            model_skin_host: self.model_skin_host.as_ref().map(|host| host.share()),
            model_skin_creation_order: self
                .model_skin_creation_order
                .as_ref()
                .map(|order| order.share()),
            update: 0,
            phase: VfxTracePhase::Initialize,
            next_instance: 1,
            operations: 1_000_000,
            record_events: true,
            simulate_motion: false,
            retained: 1,
            instance_limit: 100_001,
        };
        let definition = &self.file.emitters[emitter_index];
        let mut root = Node::emitter(
            0,
            emitter_index,
            InstanceClock::root(definition, root_life),
            None,
            &mut context,
            0,
        )?;
        for (index, &delta) in update_frames.iter().enumerate() {
            context.update = index + 1;
            context.phase = VfxTracePhase::Advance;
            root.advance(delta, false, &mut context, 0, None)?;
            context.phase = VfxTracePhase::Prepare;
            root.prepare(&mut context, 0)?;
        }
        let (retained_instances, live_particles) = root.population();
        Ok(VfxCreationTrace {
            emitter_index,
            root_life,
            update_seconds: update_seconds.to_vec(),
            update_frames,
            remainder_frames: framework_clock.remainder,
            root_age: root.age,
            root_finished: root.dead,
            events: context.events,
            retained_instances: retained_instances - 1,
            live_particles,
        })
    }
}

// Probe budgets return an error, never a truncated successful trace.
pub(super) const OPERATION_LIMIT: usize = 100_000;

struct Context<'a> {
    file: &'a AvfxFile,
    events: Vec<VfxCreationEvent>,
    creation_path: Vec<EmitterSnapshot>,
    creation_source: Option<ItemRef>,
    update: usize,
    phase: VfxTracePhase,
    next_instance: u64,
    operations: usize,
    record_events: bool,
    simulate_motion: bool,
    retained: usize,
    instance_limit: usize,
    root_constructor_cache: bool,
    root_color_seed: Option<u64>,
    particle_birth_root: Option<std::sync::Arc<super::playback::ParticleBirthRoot>>,
    color_environment: Option<super::color_curves::VfxClientColorCurveEnvironment>,
    model_skin_host: Option<super::model_skin_target::TargetEnvironment>,
    model_skin_creation_order: Option<super::model_skin_target::CreationOrderEnvironment>,
}

impl Context<'_> {
    fn tick(&mut self) -> Result<(), String> {
        self.charge(1)
    }

    fn charge(&mut self, operations: usize) -> Result<(), String> {
        self.operations = self
            .operations
            .checked_sub(operations)
            .ok_or("creation trace operation limit")?;
        Ok(())
    }

    fn allocate(&mut self) -> Result<u64, String> {
        self.tick()?;
        if self.retained >= self.instance_limit {
            return Err("staged instance limit".into());
        }
        self.retained += 1;
        let result = self.next_instance;
        self.next_instance = self
            .next_instance
            .checked_add(1)
            .ok_or("instance id limit")?;
        Ok(result)
    }

    fn record(&mut self, event: VfxCreationEvent) -> Result<(), String> {
        if !self.record_events {
            return Ok(());
        }
        if self.events.len() == 100_000 {
            return Err("creation trace event limit".into());
        }
        self.events.push(event);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ItemRef {
    kind: usize,
    index: usize,
}

impl ItemRef {
    fn get(self, file: &AvfxFile, emitter: usize) -> AvfxEmitterItem {
        let definition = &file.emitters[emitter];
        [
            definition.particle_items.as_slice(),
            definition.emitter_items.as_slice(),
        ][self.kind][self.index]
    }

    fn target(self) -> VfxTraceTarget {
        if self.kind == 0 {
            VfxTraceTarget::Particle
        } else {
            VfxTraceTarget::Emitter
        }
    }
}

struct Delay {
    item: ItemRef,
    delay: i32,
    count: i32,
    by_one: bool,
    called: i32,
    trigger: i32,
    armed_update: Option<usize>,
}

struct EmitterState {
    countdown: f32,
    overshoot: f32,
    creation_event: u64,
    interval_evaluations: u64,
    shape_ordinal: u64,
    counters: [Vec<i16>; 2],
    /// Stable local attempt ordinals used for 1..99 probability draws.
    /// This preserves repeatability without claiming the client's process-wide
    /// TLS RNG sequence.
    probability_attempts: [Vec<u64>; 2],
}

struct Node {
    id: u64,
    definition: usize,
    kind: VfxTraceTarget,
    clock: InstanceClock,
    age: f32,
    total_age: f32,
    curve_age: f32,
    curve_total_age: f32,
    render_curve_age: f32,
    render_curve_total_age: f32,
    parent_curve_age: f32,
    parent_curve_total_age: f32,
    parent_gravity_offset: f32,
    gravity_velocity: f32,
    gravity_offset: f32,
    gravity_seed: u64,
    gravity_evaluations: u64,
    injection_ordinal: i16,
    shape_ordinal: u64,
    delta: f32,
    input_delta: f32,
    powder_prewarm: Vec<PowderPrewarm>,
    ancestor_path: Vec<EmitterSnapshot>,
    birth_path: std::sync::Arc<[EmitterSnapshot]>,
    dead: bool,
    common_flags: u32,
    numeric: super::VfxCommonNumericState,
    color_seed: u64,
    color_parent_mode: i32,
    initial_parent_color: [f32; 4],
    /// Cached +d0..dc; properties replace it before Time age writeback,
    /// emitter Prepare and nonempty particle +e8 compose PICo separately.
    /// Retired parents retain it; draw consumers read rather than resample it.
    color: [f32; 4],
    client_color: Option<super::VfxClientColorCurveState>,
    client_particle: Option<super::particle_curves::VfxClientParticleCurveState>,
    client_injection: Option<super::particle_curves::VfxClientParticleInjectionCache>,
    client_model_skin: Option<super::model_skin_curves::VfxClientModelSkinCurveState>,
    client_model_skin_uv: Option<Vec<super::uv_curves::VfxClientUvCurveState>>,
    model_skin_target: Option<super::VfxModelSkinTargetState>,
    model_skin_numeric_revision: u64,
    model_skin_creation_serial: Option<u64>,
    model_skin_numeric_color: Option<[f32; 4]>,
    model_skin_numeric_targets: Option<u8>,
    source: Option<ItemRef>,
    emitter: Option<EmitterState>,
    helper: Option<Delay>,
    children: Vec<Node>,
}

impl Node {
    fn emitter(
        id: u64,
        definition: usize,
        clock: InstanceClock,
        injection: Option<(i16, u64)>,
        context: &mut Context,
        depth: usize,
    ) -> Result<Self, String> {
        Self::emitter_at_age(id, definition, clock, injection, context, depth, 0.0)
    }

    fn emitter_at_age(
        id: u64,
        definition: usize,
        clock: InstanceClock,
        injection: Option<(i16, u64)>,
        context: &mut Context,
        depth: usize,
        initial_age: f32,
    ) -> Result<Self, String> {
        if depth > 64 {
            return Err("creation trace nesting limit".into());
        }
        let emitter = &context.file.emitters[definition];
        let mut node = Self {
            id,
            definition,
            kind: VfxTraceTarget::Emitter,
            clock,
            age: initial_age,
            total_age: initial_age,
            curve_age: initial_age,
            curve_total_age: initial_age,
            render_curve_age: initial_age,
            render_curve_total_age: initial_age,
            parent_curve_age: context.creation_path.last().map_or(0.0, |p| p.curve_age),
            parent_curve_total_age: context
                .creation_path
                .last()
                .map_or(0.0, |p| p.curve_total_age),
            parent_gravity_offset: context
                .creation_path
                .last()
                .map_or(0.0, |p| p.gravity_offset),
            gravity_velocity: 0.0,
            gravity_offset: 0.0,
            gravity_seed: SplitMix64::seeded(id, definition as u64, 1, 0x6A17).next_u64(),
            gravity_evaluations: 0,
            // Constructor initialization can create descendants immediately;
            // their birth snapshots must already see this incoming injection.
            injection_ordinal: injection.map_or(0, |value| value.0),
            shape_ordinal: injection.map_or(0, |value| value.1),
            delta: 0.0,
            input_delta: 0.0,
            powder_prewarm: Vec::new(),
            ancestor_path: context.creation_path.clone(),
            birth_path: context.creation_path.clone().into(),
            dead: false,
            common_flags: 0x3f00_0000 | if clock.loops() { 0x8040_0000 } else { 0 },
            numeric: super::VfxCommonNumericState::default(),
            color_seed: if context.creation_path.is_empty() {
                context.root_color_seed.unwrap_or(id)
            } else {
                id
            },
            color_parent_mode: context
                .creation_path
                .last()
                .zip(context.creation_source)
                .map_or(0, |(parent, source)| {
                    source
                        .get(context.file, parent.definition)
                        .parent_influence_color
                }),
            initial_parent_color: context
                .creation_path
                .last()
                .map_or([1.0; 4], |parent| parent.color),
            color: [1.0; 4],
            client_color: context
                .color_environment
                .as_ref()
                .map(|environment| environment.construct(&emitter.color)),
            client_model_skin: None,
            client_particle: None,
            client_injection: None,
            client_model_skin_uv: None,
            model_skin_target: None,
            model_skin_numeric_revision: 0,
            model_skin_creation_serial: None,
            model_skin_numeric_color: None,
            model_skin_numeric_targets: None,
            source: context.creation_source,
            emitter: Some(EmitterState {
                countdown: 0.0,
                overshoot: 0.0,
                creation_event: 0,
                interval_evaluations: 0,
                shape_ordinal: 0,
                counters: [
                    vec![0; emitter.particle_items.len()],
                    vec![0; emitter.emitter_items.len()],
                ],
                probability_attempts: [
                    vec![0; emitter.particle_items.len()],
                    vec![0; emitter.emitter_items.len()],
                ],
            }),
            helper: None,
            children: Vec::new(),
        };
        node.refresh_emitter_color(context);
        node.initialize(context, depth)?;
        Ok(node)
    }

    fn initialize(&mut self, context: &mut Context, depth: usize) -> Result<(), String> {
        // Base initialization allocates all helpers before calling CrTm 1/0.
        // Helpers use ordinary attachment, not the ClCn-limited factories.
        for source in self.items(context.file) {
            let item = source.get(context.file, self.definition);
            if item.enabled
                && item.create_time == 1
                && item.generate_delay != 0
                && (item.create_count > 1 || !item.generate_delay_by_one)
            {
                let id = context.allocate()?;
                self.event(
                    source,
                    VfxTraceAction::Create,
                    VfxTraceTarget::Helper,
                    Some(id),
                    None,
                    context,
                )?;
                self.children.push(Self {
                    id,
                    definition: item.target_index as usize,
                    kind: VfxTraceTarget::Helper,
                    clock: InstanceClock::new(-1.0, 1.0, 0, 0),
                    age: 0.0,
                    total_age: 0.0,
                    curve_age: 0.0,
                    curve_total_age: 0.0,
                    render_curve_age: 0.0,
                    render_curve_total_age: 0.0,
                    parent_curve_age: 0.0,
                    parent_curve_total_age: 0.0,
                    parent_gravity_offset: 0.0,
                    gravity_velocity: 0.0,
                    gravity_offset: 0.0,
                    gravity_seed: 0,
                    gravity_evaluations: 0,
                    injection_ordinal: 0,
                    shape_ordinal: 0,
                    delta: 0.0,
                    input_delta: 0.0,
                    powder_prewarm: Vec::new(),
                    ancestor_path: context.creation_path.clone(),
                    birth_path: context.creation_path.clone().into(),
                    dead: false,
                    common_flags: 0x3f00_0000,
                    numeric: super::VfxCommonNumericState::default(),
                    color_seed: 0,
                    color_parent_mode: 0,
                    initial_parent_color: [1.0; 4],
                    color: [1.0; 4],
                    client_color: None,
                    client_model_skin: None,
                    client_particle: None,
                    client_injection: None,
                    client_model_skin_uv: None,
                    model_skin_target: None,
                    model_skin_numeric_revision: 0,
                    model_skin_creation_serial: None,
                    model_skin_numeric_color: None,
                    model_skin_numeric_targets: None,
                    source: Some(source),
                    emitter: None,
                    helper: Some(Delay {
                        item: source,
                        delay: item.generate_delay,
                        count: item.create_count,
                        by_one: item.generate_delay_by_one,
                        called: 0,
                        trigger: item.create_time,
                        armed_update: None,
                    }),
                    children: Vec::new(),
                });
                let helper = self.children.last_mut().unwrap();
                helper.numeric.inherit_fade_after_attach(
                    &mut helper.common_flags,
                    self.common_flags,
                    self.numeric,
                );
                if self.common_flags & 0x0080_0000 != 0 {
                    helper.unlock_loop_recursive(context)?;
                }
            }
        }
        self.batch(1, context, depth)?;
        self.batch(0, context, depth)?;
        Ok(())
    }

    fn items(&self, file: &AvfxFile) -> impl Iterator<Item = ItemRef> + use<> {
        let emitter = &file.emitters[self.definition];
        (0..emitter.particle_items.len())
            .map(|index| ItemRef { kind: 0, index })
            .chain((0..emitter.emitter_items.len()).map(|index| ItemRef { kind: 1, index }))
    }

    fn event(
        &self,
        source: ItemRef,
        action: VfxTraceAction,
        target: VfxTraceTarget,
        instance: Option<u64>,
        injection_ordinal: Option<i16>,
        context: &mut Context,
    ) -> Result<(), String> {
        let item = source.get(context.file, self.definition);
        context.record(VfxCreationEvent {
            update: context.update,
            phase: context.phase,
            action,
            target,
            parent_instance: self.id,
            instance,
            emitter_index: self.definition,
            item_index: source.index,
            target_index: item.target_index as usize,
            trigger: item.create_time,
            emitter_age: self.age,
            injection_ordinal,
        })
    }

    fn creation_interval(&mut self, file: &AvfxFile) -> f32 {
        let emitter = &file.emitters[self.definition];
        let random = &emitter.create_interval_random;
        let state = self.emitter.as_mut().unwrap();
        let evaluation = if !random.keys.is_empty() && matches!(random.random_type & 7, 3..=5) {
            let current = state.interval_evaluations;
            state.interval_evaluations = current.wrapping_add(1);
            current
        } else {
            0
        };
        let seed =
            SplitMix64::seeded(self.id, self.definition as u64, evaluation, 0x4352_4952).next_u64();
        curve_value_seeded_at(
            &emitter.create_interval,
            random,
            CurveAges {
                local: self.age,
                total: self.total_age,
            },
            0.0,
            seed,
        )
    }

    fn batch(&mut self, trigger: i32, context: &mut Context, depth: usize) -> Result<(), String> {
        for source in self.items(context.file) {
            let item = source.get(context.file, self.definition);
            if !item.enabled || item.create_time != trigger {
                continue;
            }
            if trigger == 2 && item.generate_delay > 0 {
                // The client allocates a termination helper only for multi-create
                // items. A single delayed termination item has no executable path.
                if item.create_count <= 1 {
                    continue;
                }
                let id = context.allocate()?;
                self.event(
                    source,
                    VfxTraceAction::Create,
                    VfxTraceTarget::Helper,
                    Some(id),
                    None,
                    context,
                )?;
                if self.dead {
                    // Original +10 constructs this helper, but 3b0060 receives
                    // force=false and rejects the retired parent. No child is
                    // registered; its destructor immediately returns the slot.
                    // PrLk other than -1 is excluded by staged admission.
                    context.retained -= 1;
                    self.event(
                        source,
                        VfxTraceAction::AttachmentRejected,
                        VfxTraceTarget::Helper,
                        Some(id),
                        None,
                        context,
                    )?;
                    continue;
                }
                self.children.push(Self {
                    id,
                    definition: item.target_index as usize,
                    kind: VfxTraceTarget::Helper,
                    clock: InstanceClock::new(-1.0, 1.0, 0, 0),
                    age: 0.0,
                    total_age: 0.0,
                    curve_age: 0.0,
                    curve_total_age: 0.0,
                    render_curve_age: 0.0,
                    render_curve_total_age: 0.0,
                    parent_curve_age: self.age,
                    parent_curve_total_age: self.total_age,
                    parent_gravity_offset: self.gravity_offset,
                    gravity_velocity: 0.0,
                    gravity_offset: 0.0,
                    gravity_seed: 0,
                    gravity_evaluations: 0,
                    injection_ordinal: 0,
                    shape_ordinal: 0,
                    delta: 0.0,
                    input_delta: 0.0,
                    powder_prewarm: Vec::new(),
                    ancestor_path: context.creation_path.clone(),
                    birth_path: context.creation_path.clone().into(),
                    dead: false,
                    common_flags: 0x3f00_0000,
                    numeric: super::VfxCommonNumericState::default(),
                    color_seed: 0,
                    color_parent_mode: 0,
                    initial_parent_color: [1.0; 4],
                    color: [1.0; 4],
                    client_color: None,
                    client_model_skin: None,
                    client_particle: None,
                    client_injection: None,
                    client_model_skin_uv: None,
                    model_skin_target: None,
                    model_skin_numeric_revision: 0,
                    model_skin_creation_serial: None,
                    model_skin_numeric_color: None,
                    model_skin_numeric_targets: None,
                    source: Some(source),
                    emitter: None,
                    helper: Some(Delay {
                        item: source,
                        delay: item.generate_delay,
                        count: item.create_count,
                        by_one: item.generate_delay_by_one,
                        called: 0,
                        trigger,
                        armed_update: Some(context.update),
                    }),
                    children: Vec::new(),
                });
                let helper = self.children.last_mut().unwrap();
                helper.numeric.inherit_fade_after_attach(
                    &mut helper.common_flags,
                    self.common_flags,
                    self.numeric,
                );
                if self.common_flags & 0x0080_0000 != 0 {
                    helper.unlock_loop_recursive(context)?;
                }
                continue;
            }
            if trigger != 0 && item.generate_delay > 0 {
                continue;
            }
            let emitter = &context.file.emitters[self.definition];
            let count = if trigger == 0 {
                let event = self.emitter.as_ref().unwrap().creation_event;
                emitter_create_count_at_seeded(
                    emitter,
                    &item,
                    self.age,
                    self.total_age,
                    event,
                    self.id,
                )
            } else {
                emitter_create_count_at(emitter, &item, self.age, self.total_age)
            };
            for _ in 0..count {
                self.create(source, context, depth)?;
            }
        }
        if trigger == 0 {
            let next_event = self
                .emitter
                .as_ref()
                .unwrap()
                .creation_event
                .wrapping_add(1);
            self.emitter.as_mut().unwrap().creation_event = next_event;
        }
        let interval = self.creation_interval(context.file);
        let state = self.emitter.as_mut().unwrap();
        state.countdown = interval + state.overshoot;
        if !state.countdown.is_finite() {
            return Err("nonfinite creation interval".into());
        }
        Ok(())
    }

    fn create(
        &mut self,
        source: ItemRef,
        context: &mut Context,
        depth: usize,
    ) -> Result<(), String> {
        let mut path = self.ancestor_path.clone();
        path.push(self.emitter_snapshot_source(context.update, context.root_constructor_cache));
        let previous = std::mem::replace(&mut context.creation_path, path);
        let previous_source = context.creation_source.replace(source);
        let result = self.create_with_path(source, context, depth);
        context.creation_source = previous_source;
        context.creation_path = previous;
        result
    }

    fn create_with_path(
        &mut self,
        source: ItemRef,
        context: &mut Context,
        depth: usize,
    ) -> Result<(), String> {
        context.tick()?;
        let item = source.get(context.file, self.definition);
        let probability = item.create_probability.clamp(0, 100);
        if probability < 100 {
            let state = self.emitter.as_mut().unwrap();
            let attempt = state.probability_attempts[source.kind][source.index];
            state.probability_attempts[source.kind][source.index] = attempt.wrapping_add(1);
            let draw = SplitMix64::seeded(
                self.id,
                ((source.kind as u64) << 32) | source.index as u64,
                attempt,
                0x4352_5052,
            )
            .next_u64() as u16;
            if (u32::from(draw) * 100 / 65_536) >= probability as u32 {
                return Ok(());
            }
        }
        let counter = &mut self.emitter.as_mut().unwrap().counters[source.kind][source.index];
        let ordinal = *counter;
        *counter = counter.wrapping_add(1);
        let limit = context.file.emitters[self.definition].child_limit;
        if limit > 0 && (self.children.len() & 0x1ff) >= limit as usize {
            return self.event(
                source,
                VfxTraceAction::CapacityRejected,
                source.target(),
                None,
                Some(ordinal),
                context,
            );
        }
        let target = item.target_index as usize;
        let (life, loops) = if source.kind == 0 {
            let particle = &context.file.particles[target];
            (particle.life, [particle.loop_start, particle.loop_end])
        } else {
            let emitter = &context.file.emitters[target];
            (emitter.life, [emitter.loop_start, emitter.loop_end])
        };
        let mut life_random = SplitMix64::seeded(
            self.id,
            ((source.kind as u64) << 32) | source.index as u64,
            ordinal as u16 as u64,
            target as u64,
        );
        let clock = self.clock.child(life, loops, &item, &mut life_random);
        if !clock.rate.is_finite() || clock.rate <= 0.0 {
            return Err("invalid instance rate".into());
        }
        let id = context.allocate()?;
        let state = self.emitter.as_mut().unwrap();
        let shape_ordinal = state.shape_ordinal;
        state.shape_ordinal = state.shape_ordinal.wrapping_add(1);
        self.event(
            source,
            VfxTraceAction::Create,
            source.target(),
            Some(id),
            Some(ordinal),
            context,
        )?;
        let mut child = if source.kind == 1 {
            Self::emitter(
                id,
                target,
                clock,
                Some((ordinal, shape_ordinal)),
                context,
                depth + 1,
            )?
        } else {
            Self {
                id,
                definition: target,
                kind: VfxTraceTarget::Particle,
                clock,
                age: 0.0,
                total_age: 0.0,
                curve_age: 0.0,
                curve_total_age: 0.0,
                render_curve_age: 0.0,
                render_curve_total_age: 0.0,
                parent_curve_age: self.curve_age,
                parent_curve_total_age: self.curve_total_age,
                parent_gravity_offset: self.gravity_offset,
                gravity_velocity: 0.0,
                gravity_offset: 0.0,
                gravity_seed: SplitMix64::seeded(id, target as u64, source.index as u64, 0x6A17)
                    .next_u64(),
                gravity_evaluations: 0,
                injection_ordinal: ordinal,
                shape_ordinal,
                delta: 0.0,
                input_delta: 0.0,
                powder_prewarm: Vec::new(),
                ancestor_path: context.creation_path.clone(),
                birth_path: context.creation_path.clone().into(),
                dead: false,
                common_flags: 0x3f00_0000 | if clock.loops() { 0x8040_0000 } else { 0 },
                numeric: super::VfxCommonNumericState::default(),
                color_seed: SplitMix64::seeded(self.color_seed, source.index as u64, id, 0x5054)
                    .next_u64(),
                color_parent_mode: item.parent_influence_color,
                initial_parent_color: self.color,
                color: [1.0; 4],
                client_color: context.color_environment.as_ref().and_then(|environment| {
                    (!environment.particle_construction)
                        .then(|| environment.construct(&context.file.particles[target].color))
                }),
                client_model_skin: None,
                client_particle: None,
                client_injection: None,
                client_model_skin_uv: None,
                model_skin_target: None,
                model_skin_numeric_revision: 0,
                model_skin_creation_serial: None,
                model_skin_numeric_color: None,
                model_skin_numeric_targets: None,
                source: None,
                emitter: None,
                helper: None,
                children: Vec::new(),
            }
        };
        if child.kind == VfxTraceTarget::Particle {
            if let Some(environment) = context
                .color_environment
                .as_ref()
                .filter(|env| env.particle_construction)
            {
                let (state, color) =
                    environment.construct_particle(&context.file.particles[target])?;
                child.client_particle = Some(state);
                child.client_injection = Some(Default::default());
                child.client_color = Some(color);
            }
        }
        if child.kind == VfxTraceTarget::Particle && child.client_color.is_some() {
            // Base 3e0f10 calls 3e1cb0 before resetting alpha at 3e1c52.
            // A later concrete/factory callback refreshes it again below.
            // Always channels must consume both calls even at the same age.
            child.refresh_particle_color(context);
            child.color[3] = 0.0;
            child.numeric.alpha = 0.0;
        }
        if let Some(state) = child.client_particle.as_mut() {
            context
                .color_environment
                .as_ref()
                .expect("particle environment retained")
                .construct_particle_derived(state, &context.file.particles[target]);
        }
        if child.kind == VfxTraceTarget::Particle {
            if let crate::avfx::AvfxParticleData::ModelSkin(data) =
                &context.file.particles[target].data
            {
                child.model_skin_creation_serial = context
                    .model_skin_creation_order
                    .as_ref()
                    .map(|order| order.allocate())
                    .transpose()?;
                child.model_skin_target = context
                    .model_skin_host
                    .as_ref()
                    .map(|host| super::VfxModelSkinTargetState::new(host.selector));
                child.client_model_skin_uv = context
                    .color_environment
                    .as_ref()
                    .map(|environment| {
                        environment.construct_model_skin_uv(&context.file.particles[target].uv_sets)
                    })
                    .transpose()?
                    .flatten();
                child.client_model_skin = context
                    .color_environment
                    .as_ref()
                    .map(|environment| environment.construct_model_skin(data))
                    .transpose()?
                    .flatten();
                child.refresh_model_skin_properties(context);
            }
        }
        child.numeric.inherit_fade_after_attach(
            &mut child.common_flags,
            self.common_flags,
            self.numeric,
        );
        if self.common_flags & 0x0080_0000 != 0 {
            child.unlock_loop_recursive(context)?;
        }
        child.shape_ordinal = shape_ordinal;
        child.injection_ordinal = ordinal;
        child.source = Some(source);
        if child.kind == VfxTraceTarget::Particle {
            // Derived constructors/factory refresh properties after the base
            // constructor's alpha reset, before attach and StFr processing.
            child.refresh_particle_color(context);
        }
        if source.kind == 1 {
            child.parent_curve_age = self.curve_age;
            child.parent_curve_total_age = self.curve_total_age;
            child.parent_gravity_offset = self.gravity_offset;
        }
        if let Some(cache) = &child.client_injection {
            if let Some(root) = &context.particle_birth_root {
                if let Some(input) = root.injection(
                    context.file,
                    super::playback::ParticleBirth {
                        id: child.id,
                        item_index: source.index,
                        shape_ordinal: child.shape_ordinal,
                        ages: CurveAges {
                            local: child.parent_curve_age,
                            total: child.parent_curve_total_age,
                        },
                        gravity: child.parent_gravity_offset,
                        path: child.birth_path.clone(),
                    },
                ) {
                    cache.bind_input(input);
                }
            }
        }
        self.children.push(child);
        let child_index = self.children.len() - 1;
        if item.start_frame_null_update {
            let phase = context.phase;
            context.phase = VfxTracePhase::NullUpdate;
            for _ in 0..item.start_frame.max(0) {
                let was_dead = self.children[child_index].dead;
                let parent =
                    self.emitter_snapshot_source(context.update, context.root_constructor_cache);
                self.children[child_index].advance(1.0, true, context, depth + 1, Some(parent))?;
                if !was_dead && self.children[child_index].dead {
                    self.event(
                        source,
                        VfxTraceAction::Finish,
                        source.target(),
                        Some(id),
                        None,
                        context,
                    )?;
                }
            }
            context.phase = phase;
        } else {
            // The constructor assigns the age without looping it or updating
            // descendants. The next update performs the loop/Life checks.
            self.children[child_index].age = item.start_frame.max(0) as f32 * clock.rate;
            self.children[child_index].total_age = item.start_frame.max(0) as f32 * clock.rate;
        }
        Ok(())
    }

    fn periodic(&mut self, context: &mut Context, depth: usize) -> Result<(), String> {
        if self.emitter.as_ref().unwrap().countdown > 0.0 {
            let state = self.emitter.as_mut().unwrap();
            state.countdown -= self.delta;
            if state.countdown > 0.0 {
                return Ok(());
            }
            state.overshoot = state.countdown;
        } else {
            let interval = self.creation_interval(context.file);
            let state = self.emitter.as_mut().unwrap();
            state.countdown = interval;
            if !state.countdown.is_finite() {
                return Err("nonfinite creation interval".into());
            }
            if state.countdown <= 0.0 {
                return Ok(());
            }
            state.overshoot = 0.0;
        }
        self.batch(0, context, depth)
    }

    fn emitter_snapshot(&self, input_update: usize) -> EmitterSnapshot {
        self.emitter_snapshot_source(input_update, false)
    }

    fn emitter_snapshot_source(
        &self,
        input_update: usize,
        constructor_cache: bool,
    ) -> EmitterSnapshot {
        EmitterSnapshot {
            input_update,
            constructor_cache,
            birth_path: self.birth_path.clone(),
            id: self.id,
            definition: self.definition,
            clock: self.clock,
            injection_ordinal: self.injection_ordinal,
            shape_ordinal: self.shape_ordinal,
            source_item_index: self.source.map(|source| source.index),
            parent_curve_age: self.parent_curve_age,
            parent_curve_total_age: self.parent_curve_total_age,
            parent_gravity_offset: self.parent_gravity_offset,
            curve_age: self.curve_age,
            curve_total_age: self.curve_total_age,
            gravity_offset: self.gravity_offset,
            color: self.color,
        }
    }

    fn refresh_emitter_color(&mut self, context: &Context) {
        let file = context.file;
        if let Some(state) = self.client_color {
            self.color = context
                .color_environment
                .as_ref()
                .expect("retained Color environment")
                .evaluate(
                    state,
                    &file.emitters[self.definition].color,
                    self.curve_age,
                    self.curve_total_age,
                );
            self.numeric.alpha = self.color[3];
            return;
        }
        let animation = super::EmitterColorAnimation {
            emitter: &file.emitters[self.definition],
            emitter_index: self.definition,
            instance_seed: self.color_seed,
            clock: self.clock,
            parent: super::ParentColor::None,
        };
        self.color = animation.at_ages(
            self.curve_age,
            CurveAges {
                local: self.curve_age,
                total: self.curve_total_age,
            },
        );
        self.numeric.alpha = self.color[3];
    }

    fn compose_parent_color(&mut self) {
        let parent = match self.color_parent_mode {
            1 => self.initial_parent_color,
            2 | 8 => self.ancestor_path.last().map_or([1.0; 4], |p| p.color),
            _ => return,
        };
        self.color = std::array::from_fn(|axis| parent[axis] * self.color[axis]);
        self.numeric.alpha = self.color[3];
    }

    fn refresh_model_skin_properties(&mut self, context: &Context) {
        if let Some(state) = &mut self.client_model_skin {
            let crate::avfx::AvfxParticleData::ModelSkin(data) =
                &context.file.particles[self.definition].data
            else {
                unreachable!()
            };
            context
                .color_environment
                .as_ref()
                .expect("retained Color environment")
                .evaluate_model_skin_properties(
                    state,
                    data,
                    [self.curve_age, self.curve_total_age],
                );
        }
    }

    fn refresh_particle_color(&mut self, context: &Context) {
        let file = context.file;
        if let Some(state) = &mut self.client_particle {
            context
                .color_environment
                .as_ref()
                .expect("retained particle curve environment")
                .evaluate_particle(
                    state,
                    &file.particles[self.definition],
                    [self.curve_age, self.curve_total_age],
                );
        }
        if let Some(state) = self.client_color {
            self.color = context
                .color_environment
                .as_ref()
                .expect("retained Color environment")
                .evaluate(
                    state,
                    &file.particles[self.definition].color,
                    self.curve_age,
                    self.curve_total_age,
                );
            self.numeric.alpha = self.color[3];
            return;
        }
        self.color = super::color_curve_seeded_at(
            &file.particles[self.definition].color,
            CurveAges {
                local: self.curve_age,
                total: self.curve_total_age,
            },
            self.color_seed,
        );
        self.numeric.alpha = self.color[3];
    }

    fn mark_dead(&mut self) {
        self.dead = true;
        self.common_flags = (self.common_flags & 0xc0ff_ffff) | 0x40000;
    }

    fn configure_fade_recursive(
        &mut self,
        duration: i32,
        mode: u32,
        flag: bool,
        context: &mut Context,
    ) -> Result<(), String> {
        context.tick()?;
        self.numeric
            .configure_fade(&mut self.common_flags, duration, mode, flag);
        for child in &mut self.children {
            child.configure_fade_recursive(duration, mode, flag, context)?;
        }
        Ok(())
    }

    fn unlock_loop_recursive(&mut self, context: &mut Context) -> Result<(), String> {
        context.tick()?;
        if !super::unlock_loop_flags(&mut self.common_flags) {
            return Ok(());
        }
        self.clock.unlock_loop_point();
        for child in &mut self.children {
            child.unlock_loop_recursive(context)?;
        }
        Ok(())
    }

    fn clear_input_delta(&mut self) {
        self.input_delta = 0.0;
        self.powder_prewarm.clear();
        for child in &mut self.children {
            child.clear_input_delta();
        }
    }

    fn advance(
        &mut self,
        delta: f32,
        null_update: bool,
        context: &mut Context,
        depth: usize,
        parent: Option<EmitterSnapshot>,
    ) -> Result<(), String> {
        context.tick()?;
        let mut child_delta = delta;
        if self.kind == VfxTraceTarget::Helper
            && self.helper.as_ref().and_then(|helper| helper.armed_update) == Some(context.update)
        {
            // A termination helper is born at the callback boundary. It must
            // not consume the parent update's already elapsed time.
            child_delta = 0.0;
        }
        if !self.dead {
            self.delta = child_delta * self.clock.rate;
            if !null_update {
                self.input_delta += self.delta;
            }
            let mut next = self.age + self.delta;
            let next_total = self.total_age + self.delta;
            if !next.is_finite() {
                return Err("nonfinite instance age".into());
            }
            if self.clock.loops() {
                while next >= self.clock.loop_end {
                    context.tick()?;
                    next -= self.clock.loop_end - self.clock.loop_start;
                }
            }
            if self.clock.life >= 0.0 && next > self.clock.life {
                next = self.clock.life;
                self.mark_dead();
                if self.emitter.is_some() {
                    // PrLk=-1 finish helpers cannot attach to a dead emitter.
                    self.batch(2, context, depth)?;
                }
            }
            if !self.dead {
                // Verified nonempty +e8 callbacks refresh transform/color and motion.
                // Quad/Windmill +e8 are empty; UV/Data caches only refresh on +f0.
                let prewarm_properties = self.kind == VfxTraceTarget::Particle
                    && matches!(
                        context.file.particles[self.definition].particle_type,
                        Some(
                            crate::avfx::ParticleType::Model
                                | crate::avfx::ParticleType::Line
                                | crate::avfx::ParticleType::LightModel
                                | crate::avfx::ParticleType::Powder
                                | crate::avfx::ParticleType::ModelSkin
                                | crate::avfx::ParticleType::Laser
                                | crate::avfx::ParticleType::Disc
                                | crate::avfx::ParticleType::Polyline
                                | crate::avfx::ParticleType::Polygon
                                | crate::avfx::ParticleType::Decal
                                | crate::avfx::ParticleType::DecalRing
                        )
                    );
                if !null_update || self.emitter.is_some() || prewarm_properties {
                    self.curve_age = self.age;
                    self.curve_total_age = self.total_age;
                }
                if self.emitter.is_some() {
                    self.refresh_emitter_color(context);
                    if null_update {
                        self.compose_parent_color();
                    }
                } else if self.kind == VfxTraceTarget::Particle
                    && (!null_update || prewarm_properties)
                {
                    if !null_update {
                        self.refresh_model_skin_properties(context);
                        // Disc +f0 refreshes CEI/CEO before base XYZ/Col. Its
                        // +e8 prewarm updates XYZ/motion but retains Disc/Line/Polyline colors.
                        if let Some(state) = self.client_particle.as_mut() {
                            context
                                .color_environment
                                .as_ref()
                                .expect("particle environment retained")
                                .evaluate_particle_shape_properties(
                                    state,
                                    &context.file.particles[self.definition],
                                    [self.curve_age, self.curve_total_age],
                                );
                        }
                    }
                    self.refresh_particle_color(context);
                    if null_update {
                        // Nonempty +e8 executes +130 after property/motion;
                        // its PICo result remains in +d0..dc until next +f0.
                        self.compose_parent_color();
                    }
                }
                if !null_update {
                    self.render_curve_age = self.age;
                    self.render_curve_total_age = self.total_age;
                }
                // These +e8 callbacks integrate at the old age; Quad +e8 is empty.
                if null_update && (self.emitter.is_some() || prewarm_properties) {
                    self.update_gravity(context)?;
                    self.update_injection(context)?;
                }
                if null_update && self.kind == VfxTraceTarget::Particle {
                    let definition = &context.file.particles[self.definition];
                    if matches!(
                        definition.particle_type,
                        Some(crate::avfx::ParticleType::Powder | crate::avfx::ParticleType::Line)
                    ) && definition.simple_anim_enable
                    {
                        context.charge(self.ancestor_path.len())?;
                        self.powder_prewarm.push(PowderPrewarm {
                            client_particle_xyz: self.client_particle.map(|state| state.cache),
                            client_injection: self
                                .client_injection
                                .as_ref()
                                .map(|cache| cache.snapshot()),
                            delta: self.delta,
                            curve_age: self.curve_age,
                            curve_total_age: self.curve_total_age,
                            gravity_offset: self.gravity_offset,
                            parent,
                            ancestor_path: self.ancestor_path.clone(),
                        });
                    }
                }
                if null_update && self.emitter.is_some() {
                    self.periodic(context, depth)?;
                }
            }
            self.age = next;
            self.total_age = next_total;
        }
        let mut index = 0;
        while index < self.children.len() {
            if !null_update && self.children[index].dead && self.children[index].children.is_empty()
            {
                self.retire(index, context)?;
                continue;
            }
            let was_dead = self.children[index].dead;
            let parent =
                self.emitter_snapshot_source(context.update, context.root_constructor_cache);
            self.children[index].ancestor_path = self.ancestor_path.clone();
            self.children[index].ancestor_path.push(parent.clone());
            self.children[index].advance(
                child_delta,
                null_update,
                context,
                depth + 1,
                Some(parent),
            )?;
            if !was_dead && self.children[index].dead {
                let child = &self.children[index];
                self.event(
                    child.source.unwrap(),
                    VfxTraceAction::Finish,
                    child.kind,
                    Some(child.id),
                    None,
                    context,
                )?;
            }
            if null_update && self.children[index].dead && self.children[index].children.is_empty()
            {
                self.retire(index, context)?;
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    /// Common END calls +10 once, then walks the current children, including
    /// forced children appended by that callback. Retirement does not unlink.
    fn end_recursive(&mut self, context: &mut Context, depth: usize) -> Result<(), String> {
        context.tick()?;
        if !self.dead {
            self.mark_dead();
            if self.emitter.is_some() {
                self.batch(2, context, depth)?;
            }
        }
        for child in &mut self.children {
            child.end_recursive(context, depth + 1)?;
        }
        Ok(())
    }

    fn retire(&mut self, index: usize, context: &mut Context) -> Result<(), String> {
        // Vec removal shifts later siblings; account for that work as well.
        context.charge(self.children.len() - index)?;
        let child = self.children.remove(index);
        context.retained -= 1;
        self.event(
            child.source.unwrap(),
            VfxTraceAction::Retire,
            child.kind,
            Some(child.id),
            None,
            context,
        )
    }

    fn prepare(&mut self, context: &mut Context, depth: usize) -> Result<(), String> {
        context.tick()?;
        if !self.dead && self.emitter.is_some() {
            self.compose_parent_color();
            self.update_gravity(context)?;
            self.periodic(context, depth)?;
        }
        // +70 follows next after each callback. Appended children can be visited
        // this phase, including zero-age, one-by-one helper callbacks.
        let mut index = 0;
        while index < self.children.len() {
            let child = &self.children[index];
            let request = child
                .helper
                .as_ref()
                .filter(|_| !child.dead)
                .and_then(|helper| {
                    if helper.trigger == 2 && helper.armed_update == Some(context.update) {
                        return None;
                    }
                    let threshold = if helper.by_one {
                        helper.delay.wrapping_mul(helper.called)
                    } else {
                        helper.delay
                    };
                    (child.age >= threshold as f32).then_some((
                        helper.item,
                        if helper.by_one {
                            1
                        } else {
                            helper.count.max(0)
                        },
                    ))
                });
            if let Some((source, count)) = request {
                for _ in 0..count {
                    self.create(source, context, depth)?;
                }
                let child = &mut self.children[index];
                let helper = child.helper.as_mut().unwrap();
                helper.called = helper.called.wrapping_add(count);
                if !helper.by_one || helper.called == helper.count {
                    child.mark_dead();
                    let id = child.id;
                    self.event(
                        source,
                        VfxTraceAction::Finish,
                        VfxTraceTarget::Helper,
                        Some(id),
                        None,
                        context,
                    )?;
                }
            } else if self.children[index].helper.is_none() {
                // +70 creation observes ancestors after their gravity callback,
                // not the snapshots left by the earlier age-update traversal.
                let parent =
                    self.emitter_snapshot_source(context.update, context.root_constructor_cache);
                self.children[index].ancestor_path = self.ancestor_path.clone();
                self.children[index].ancestor_path.push(parent.clone());
                self.children[index].prepare(context, depth + 1)?;
            }
            index += 1;
        }
        Ok(())
    }

    fn update_gravity(&mut self, context: &Context) -> Result<(), String> {
        if !context.simulate_motion {
            return Ok(());
        }
        if let Some(state) = self.client_particle {
            let value = context
                .color_environment
                .as_ref()
                .expect("particle curve environment retained")
                .evaluate_particle_gravity(
                    self.definition,
                    state,
                    &context.file.particles[self.definition],
                    [self.age, self.total_age],
                );
            // Original bit11 bypasses the entire callback for neutral Gra.
            if let Some(value) = value {
                self.gravity_velocity += value * self.delta;
                self.gravity_offset += self.gravity_velocity * self.delta;
                if !self.gravity_velocity.is_finite() || !self.gravity_offset.is_finite() {
                    return Err("staged gravity overflow".into());
                }
            }
            return Ok(());
        }
        let (curve, random) = match self.kind {
            VfxTraceTarget::Emitter => {
                let emitter = &context.file.emitters[self.definition];
                (&emitter.gravity, &emitter.gravity_random)
            }
            VfxTraceTarget::Particle => {
                let particle = &context.file.particles[self.definition];
                (&particle.gravity, &particle.gravity_random)
            }
            VfxTraceTarget::Helper => return Ok(()),
        };
        // Client stores f32 velocity first, then uses that velocity for this step.
        // First random modes retain one instance coefficient while their amplitude
        // curve continues to use the instance's local/total curve clocks.
        let ages = CurveAges {
            local: self.age,
            total: self.total_age,
        };
        let value = if !random.keys.is_empty() && matches!(random.random_type & 7, 3..=5) {
            let evaluation = self.gravity_evaluations;
            self.gravity_evaluations = evaluation.wrapping_add(1);
            curve.value_at(ages.local, ages.total, 0.0)
                + super::random_curve_offset(
                    random.random_type,
                    random.value_at(ages.local, ages.total, 0.0),
                    evaluation,
                    self.gravity_seed,
                )
        } else {
            curve_value_seeded_at(curve, random, ages, 0.0, self.gravity_seed)
        };
        self.gravity_velocity += value * self.delta;
        self.gravity_offset += self.gravity_velocity * self.delta;
        if !self.gravity_velocity.is_finite() || !self.gravity_offset.is_finite() {
            return Err("staged gravity overflow".into());
        }
        Ok(())
    }

    fn update_injection(&self, context: &Context) -> Result<(), String> {
        if !context.simulate_motion {
            return Ok(());
        }
        let Some(cache) = &self.client_injection else {
            return Ok(());
        };
        let state = self
            .client_particle
            .expect("particle motion owns First states");
        context
            .color_environment
            .as_ref()
            .expect("particle curve environment retained")
            .advance_particle_injection(
                self.definition,
                state,
                &context.file.particles[self.definition],
                [self.age, self.total_age],
                self.delta,
                cache,
            )
    }

    fn prepare_particles(&mut self, context: &mut Context) -> Result<(), String> {
        context.tick()?;
        if self.common_flags & 0x2400_0000 == 0x2400_0000 && self.kind == VfxTraceTarget::Particle {
            self.update_gravity(context)?;
            self.update_injection(context)?;
        }
        let mut aura_ready = self.common_flags & 0x2400_0000 == 0x2400_0000;
        if aura_ready {
            if let Some(target) = &mut self.model_skin_target {
                let crate::avfx::AvfxParticleData::ModelSkin(data) =
                    &context.file.particles[self.definition].data
                else {
                    unreachable!()
                };
                let input = context
                    .model_skin_host
                    .as_ref()
                    .expect("retained ModelSkin host")
                    .input();
                let query = target.query(data.aura_target as u32, input);
                aura_ready = query.begin_numeric(&mut self.common_flags);
                if aura_ready {
                    self.model_skin_numeric_targets = query.list.map(|list| {
                        list.surfaces
                            .iter()
                            .enumerate()
                            .fold(0, |mask, (slot, surface)| {
                                mask | if surface.is_some() { 1 << slot } else { 0 }
                            })
                    });
                }
            }
        }
        if aura_ready {
            if self.client_model_skin.is_some() {
                self.compose_parent_color();
            }
            if let Some(state) = &mut self.client_model_skin {
                let crate::avfx::AvfxParticleData::ModelSkin(data) =
                    &context.file.particles[self.definition].data
                else {
                    unreachable!()
                };
                context
                    .color_environment
                    .as_ref()
                    .expect("retained Color environment")
                    .evaluate_model_skin_numeric(
                        self.definition,
                        state,
                        data,
                        [self.age, self.total_age],
                    );
            }
        }
        if aura_ready {
            if let Some(states) = &mut self.client_model_skin_uv {
                context
                    .color_environment
                    .as_ref()
                    .expect("retained Color environment")
                    .evaluate_model_skin_uv(
                        self.definition,
                        states,
                        &context.file.particles[self.definition].uv_sets,
                        [self.age, self.total_age],
                    );
            }
        }
        if aura_ready && self.model_skin_target.is_some() {
            self.model_skin_numeric_revision = self
                .model_skin_numeric_revision
                .checked_add(1)
                .ok_or("ModelSkin Numeric revision limit")?;
            // Aura runs before Common's separate fade callback. The full
            // packet cache must not pick up later Time/property/fade changes.
            self.model_skin_numeric_color = Some(self.color);
        }
        if let Some(step) =
            self.numeric
                .begin_refresh(&mut self.common_flags, self.delta, |_, _| {})
        {
            if step.retire {
                // Flags are retired before +10, but the callback's factories
                // still capture the pre-zero alpha. Walk newly appended children
                // only after the final write, just as Common +78 does.
                self.dead = true;
                if self.emitter.is_some() {
                    self.batch(2, context, self.ancestor_path.len())?;
                }
            }
            self.numeric.finish_refresh(step);
            self.color[3] = self.numeric.alpha;
        }
        // +78 runs after every root's +70, including children created by +70.
        // Newborn delta is zero unless StFr left a null-update delta behind.
        for index in 0..self.children.len() {
            let was_dead = self.children[index].dead;
            // A descendant +10 factory sees ancestors after their numeric
            // callback, while its own alpha still precedes the final zero write.
            let parent =
                self.emitter_snapshot_source(context.update, context.root_constructor_cache);
            self.children[index].ancestor_path = self.ancestor_path.clone();
            self.children[index].ancestor_path.push(parent);
            self.children[index].prepare_particles(context)?;
            if !was_dead && self.children[index].dead {
                let child = &self.children[index];
                self.event(
                    child.source.unwrap(),
                    VfxTraceAction::Finish,
                    child.kind,
                    Some(child.id),
                    None,
                    context,
                )?;
            }
        }
        Ok(())
    }

    fn population(&self) -> (usize, usize) {
        self.children.iter().map(Self::population).fold(
            (
                1,
                usize::from(self.kind == VfxTraceTarget::Particle && !self.dead),
            ),
            |(nodes, particles), (child_nodes, child_particles)| {
                (nodes + child_nodes, particles + child_particles)
            },
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct PowderPrewarm {
    pub client_injection: Option<super::particle_curves::VfxClientParticleInjectionCache>,
    pub client_particle_xyz: Option<[[f32; 3]; 3]>,
    pub ancestor_path: Vec<EmitterSnapshot>,
    pub parent: Option<EmitterSnapshot>,
    pub delta: f32,
    pub curve_age: f32,
    pub curve_total_age: f32,
    pub gravity_offset: f32,
}

#[derive(Clone, Debug)]
pub(super) struct EmitterSnapshot {
    /// External Binder/Document cache generation, independent of instance age.
    pub input_update: usize,
    /// Root Item Binder construction can create particles before that Binder's
    /// ordinary Prepare changes its cache in the same input generation.
    pub constructor_cache: bool,
    pub birth_path: std::sync::Arc<[EmitterSnapshot]>,
    pub id: u64,
    pub definition: usize,
    pub clock: InstanceClock,
    pub injection_ordinal: i16,
    pub shape_ordinal: u64,
    pub source_item_index: Option<usize>,
    pub parent_curve_age: f32,
    pub parent_curve_total_age: f32,
    pub parent_gravity_offset: f32,
    pub curve_age: f32,
    pub curve_total_age: f32,
    pub gravity_offset: f32,
    pub color: [f32; 4],
}

pub(super) struct ParticleView {
    pub client_particle: Option<super::particle_curves::VfxClientParticleCurveState>,
    pub client_injection: Option<super::particle_curves::VfxClientParticleInjectionCache>,
    pub client_particle_xyz: Option<[[f32; 3]; 3]>,
    pub client_model_skin: Option<super::model_skin_curves::VfxClientModelSkinCurveCache>,
    pub client_model_skin_uv: Option<Vec<Option<super::uv_curves::VfxClientUvCurveCache>>>,
    pub model_skin_numeric_revision: Option<u64>,
    pub model_skin_creation_serial: Option<u64>,
    pub model_skin_numeric_color: Option<[f32; 4]>,
    pub model_skin_numeric_targets: Option<u8>,
    pub color: [f32; 4],
    pub parent_color: Option<[f32; 4]>,
    pub birth_path: std::sync::Arc<[EmitterSnapshot]>,
    pub id: u64,
    pub definition: usize,
    pub item_index: usize,
    /// Emitter definition which owns this particle. Nested particles retain
    /// the child emitter's definition instead of being mistaken for a root item.
    pub emitter_id: u64,
    pub emitter_definition: usize,
    pub emitter_curve_age: f32,
    pub emitter_curve_total_age: f32,
    pub emitter_gravity_offset: f32,
    pub curve_age: f32,
    pub curve_total_age: f32,
    pub render_curve_age: f32,
    pub render_curve_total_age: f32,
    pub age: f32,
    pub total_age: f32,
    pub input_delta: f32,
    pub powder_prewarm: Vec<PowderPrewarm>,
    pub parent_curve_age: f32,
    pub parent_curve_total_age: f32,
    pub parent_gravity_offset: f32,
    pub gravity_offset: f32,
    pub injection_ordinal: i16,
    pub shape_ordinal: u64,
    pub emitter_path: Vec<EmitterSnapshot>,
}

#[cfg(test)]
pub(super) struct CommonNodeSnapshot {
    pub flags: u32,
    pub children: usize,
    pub numeric: super::VfxCommonNumericState,
    pub delta: f32,
    pub age: f32,
    pub total: f32,
}

/// Playback never keeps an event log or replays elapsed history. Particle
/// views are collected recursively so admitted child emitters can contribute
/// their own particles.
pub(super) struct Instances {
    roots: Vec<RootSlot>,
    update: usize,
    next_instance: u64,
    retained: usize,
    elapsed_frames: f32,
    color_environment: Option<super::color_curves::VfxClientColorCurveEnvironment>,
    model_skin_host: Option<super::model_skin_target::TargetEnvironment>,
    model_skin_creation_order: Option<super::model_skin_target::CreationOrderEnvironment>,
}

/// The Binder owns a slot before its emitter factory succeeds. Its child is
/// removed in the next Time pass after becoming retired and empty.
struct RootSlot {
    particle_birth_root: Option<std::sync::Arc<super::playback::ParticleBirthRoot>>,
    color_seed: Option<u64>,
    child: Option<Node>,
    definition: usize,
    life: f32,
    binder: bool,
    /// Due Timeline age and authored initial age. No emitter exists
    /// until dispatch; a crossing update passes its overshoot to the constructor.
    pending_start: Option<(f32, f32)>,
}

#[derive(Clone, Copy)]
pub(super) struct RootInitialization {
    pub color_seed: Option<u64>,
    pub definition: usize,
    pub life: f32,
    pub start_delay: f32,
    pub binder: bool,
    pub child_age: Option<f32>,
}

pub(super) struct RootPreparation<'a, 'b> {
    roots: &'a mut Vec<RootSlot>,
    context: &'a mut Context<'b>,
}

impl RootPreparation<'_, '_> {
    pub fn set_particle_birth_root(
        &mut self,
        root: usize,
        source: super::playback::ParticleBirthRoot,
    ) {
        self.roots[root].particle_birth_root = Some(std::sync::Arc::new(source));
    }

    #[cfg(test)]
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    pub fn advance_root(&mut self, root: usize, frames: f32) -> Result<(), String> {
        let slot = &mut self.roots[root];
        self.context.particle_birth_root = slot.particle_birth_root.clone();
        if slot.binder
            && slot
                .child
                .as_ref()
                .is_some_and(|child| child.dead && child.children.is_empty())
        {
            slot.child = None;
            self.context.retained -= 1;
        }
        if let Some(child) = &mut slot.child {
            child.advance(frames, false, self.context, 0, None)?;
        }
        Ok(())
    }

    pub fn refresh_root_numeric(&mut self, root: usize) -> Result<(), String> {
        self.context.particle_birth_root = self.roots[root].particle_birth_root.clone();
        if let Some(child) = &mut self.roots[root].child {
            child.prepare_particles(self.context)?;
        }
        Ok(())
    }

    pub fn charge_work(&mut self) -> Result<(), String> {
        self.context.tick()
    }

    pub fn add_binder_root(
        &mut self,
        definition: usize,
        life: f32,
        color_seed: u64,
    ) -> Result<usize, String> {
        self.context.tick()?;
        let index = self.roots.len();
        self.roots.push(RootSlot {
            color_seed: Some(color_seed),
            particle_birth_root: None,
            child: None,
            definition,
            life,
            binder: true,
            pending_start: None,
        });
        Ok(index)
    }

    /// A child attached after the whole Time pass joins this Prepare pass.
    pub fn prepare_root(&mut self, root_index: usize, input_end: f32) -> Result<(), String> {
        let root = &mut self.roots[root_index];
        self.context.particle_birth_root = root.particle_birth_root.clone();
        self.context.root_color_seed = root.color_seed;
        if let Some((due_age, initial_age)) =
            root.pending_start.filter(|(due, _)| *due <= input_end)
        {
            let id = self.context.allocate()?;
            let phase = self.context.phase;
            self.context.phase = VfxTracePhase::Initialize;
            root.child = Some(Node::emitter_at_age(
                id,
                root.definition,
                InstanceClock::root(&self.context.file.emitters[root.definition], root.life),
                None,
                self.context,
                0,
                initial_age + (input_end - due_age),
            )?);
            root.pending_start = None;
            self.context.phase = phase;
        }
        if let Some(child) = &mut root.child {
            child.prepare(self.context, 0)?;
        }
        Ok(())
    }

    pub fn registered_children(&self, root: usize) -> u16 {
        u16::from(self.roots[root].child.is_some())
    }

    pub fn configure_root_fade(
        &mut self,
        root: usize,
        duration: i32,
        mode: u32,
        flag: bool,
    ) -> Result<(), String> {
        if let Some(child) = &mut self.roots[root].child {
            child.configure_fade_recursive(duration, mode, flag, self.context)?;
        }
        Ok(())
    }

    /// Register and run the emitter's +08 before inheriting only its own fade.
    pub fn inherit_root_after_attach(
        &mut self,
        root: usize,
        parent_flags: u32,
        parent: super::VfxCommonNumericState,
    ) -> Result<(), String> {
        if let Some(child) = &mut self.roots[root].child {
            child
                .numeric
                .inherit_fade_after_attach(&mut child.common_flags, parent_flags, parent);
            if parent_flags & 0x0080_0000 != 0 {
                child.unlock_loop_recursive(self.context)?;
            }
        }
        Ok(())
    }

    pub fn unlock_root_loop(&mut self, root: usize) -> Result<(), String> {
        if let Some(child) = &mut self.roots[root].child {
            child.unlock_loop_recursive(self.context)?;
        }
        Ok(())
    }

    pub fn end_root(&mut self, root: usize) -> Result<(), String> {
        let root = &mut self.roots[root];
        // END may construct forced particles and run their null-update prewarm
        // between inputs. Select this owner, including when another root was
        // visited immediately before it.
        self.context.particle_birth_root = root.particle_birth_root.clone();
        self.context.root_color_seed = root.color_seed;
        root.pending_start = None;
        if let Some(child) = &mut root.child {
            child.end_recursive(self.context, 0)?;
        }
        Ok(())
    }

    /// A deleting destructor frees the registered tree synchronously. It does
    /// not call Common END/+10 or generate termination batches/particle tails.
    /// Keep the outer vector slot until the owner's post-phase compaction.
    pub fn destroy_root(&mut self, root: usize) -> Result<Vec<u64>, String> {
        fn ids(node: &Node, output: &mut Vec<u64>) {
            output.push(node.id);
            for child in &node.children {
                ids(child, output);
            }
        }
        let slot = &mut self.roots[root];
        let mut released = Vec::new();
        if let Some(child) = &slot.child {
            ids(child, &mut released);
        }
        self.context.charge(released.len().max(1))?;
        self.context.retained -= released.len();
        slot.pending_start = None;
        slot.child = None;
        Ok(released)
    }

    pub fn create_child(&mut self, root: usize, age: f32) -> Result<(), String> {
        self.create_child_with_cache(root, age, false)
    }

    pub fn input_update(&self) -> usize {
        self.context.update
    }

    pub fn create_child_with_cache(
        &mut self,
        root: usize,
        age: f32,
        constructor_cache: bool,
    ) -> Result<(), String> {
        let slot = &mut self.roots[root];
        self.context.particle_birth_root = slot.particle_birth_root.clone();
        self.context.root_color_seed = slot.color_seed;
        if !slot.binder || slot.child.is_some() {
            return Err("staged playback: duplicate Binder child factory".into());
        }
        let id = self.context.allocate()?;
        let phase = self.context.phase;
        self.context.phase = VfxTracePhase::Initialize;
        let previous_cache = self.context.root_constructor_cache;
        self.context.root_constructor_cache = constructor_cache;
        slot.child = Some(Node::emitter_at_age(
            id,
            slot.definition,
            InstanceClock::root(&self.context.file.emitters[slot.definition], slot.life),
            None,
            self.context,
            0,
            age,
        )?);
        self.context.phase = phase;
        self.context.root_constructor_cache = previous_cache;
        Ok(())
    }
}

impl Instances {
    fn context<'a>(&self, file: &'a AvfxFile) -> Context<'a> {
        Context {
            file,
            events: Vec::new(),
            creation_path: Vec::new(),
            creation_source: None,
            root_constructor_cache: false,
            root_color_seed: None,
            particle_birth_root: None,
            color_environment: self.color_environment.clone(),
            model_skin_host: self.model_skin_host.as_ref().map(|host| host.share()),
            model_skin_creation_order: self
                .model_skin_creation_order
                .as_ref()
                .map(|order| order.share()),
            update: self.update,
            phase: VfxTracePhase::Initialize,
            next_instance: self.next_instance,
            operations: OPERATION_LIMIT,
            record_events: false,
            simulate_motion: true,
            retained: self.retained,
            instance_limit: 16_384,
        }
    }

    #[cfg(test)]
    pub fn new(file: &AvfxFile, roots: &[(usize, f32, f32)]) -> Result<Self, String> {
        let roots = roots
            .iter()
            .map(|&(definition, life, start_delay)| RootInitialization {
                color_seed: None,
                definition,
                life,
                start_delay,
                binder: false,
                child_age: Some(0.0),
            })
            .collect::<Vec<_>>();
        Self::new_with_roots(file, &roots)
    }

    #[cfg(test)]
    pub fn new_with_roots(file: &AvfxFile, roots: &[RootInitialization]) -> Result<Self, String> {
        Self::new_with_roots_continuing(file, roots, None, None, None, None)
    }

    /// Document replacement releases the old tree, while instance identities
    /// and the enclosing input counter continue across the constructor.
    #[cfg(test)]
    pub fn new_with_roots_continuing(
        file: &AvfxFile,
        roots: &[RootInitialization],
        previous: Option<&Self>,
        color_environment: Option<super::color_curves::VfxClientColorCurveEnvironment>,
        model_skin_host: Option<super::model_skin_target::TargetEnvironment>,
        model_skin_creation_order: Option<super::model_skin_target::CreationOrderEnvironment>,
    ) -> Result<Self, String> {
        Self::new_with_birth_roots(
            file,
            roots,
            previous,
            Vec::new(),
            color_environment,
            model_skin_host,
            model_skin_creation_order,
        )
    }

    pub fn new_with_birth_roots(
        file: &AvfxFile,
        roots: &[RootInitialization],
        previous: Option<&Self>,
        births: Vec<super::playback::ParticleBirthRoot>,
        color_environment: Option<super::color_curves::VfxClientColorCurveEnvironment>,
        model_skin_host: Option<super::model_skin_target::TargetEnvironment>,
        model_skin_creation_order: Option<super::model_skin_target::CreationOrderEnvironment>,
    ) -> Result<Self, String> {
        let mut state = Self {
            roots: Vec::new(),
            update: previous.map_or(0, |value| value.update),
            next_instance: previous.map_or(0, |value| value.next_instance),
            retained: 0,
            elapsed_frames: 0.0,
            color_environment,
            model_skin_host,
            model_skin_creation_order,
        };
        let mut context = state.context(file);
        for (root_index, root) in roots.iter().enumerate() {
            context.particle_birth_root = births.get(root_index).cloned().map(std::sync::Arc::new);
            context.root_color_seed = root.color_seed;
            let pending_start = (!root.binder && root.start_delay > 0.0)
                .then(|| root.child_age.map(|age| (root.start_delay, age)))
                .flatten();
            let child = if pending_start.is_some() {
                None
            } else if let Some(age) = root.child_age {
                let id = context.allocate()?;
                Some(Node::emitter_at_age(
                    id,
                    root.definition,
                    InstanceClock::root(&file.emitters[root.definition], root.life),
                    None,
                    &mut context,
                    0,
                    age,
                )?)
            } else {
                None
            };
            state.roots.push(RootSlot {
                color_seed: root.color_seed,
                particle_birth_root: context.particle_birth_root.clone(),
                child,
                definition: root.definition,
                life: root.life,
                binder: root.binder,
                pending_start,
            });
        }
        state.next_instance = context.next_instance;
        state.retained = context.retained;
        Ok(state)
    }

    pub fn release_document_tree(&mut self) {
        self.roots.clear();
        self.retained = 0;
        self.elapsed_frames = 0.0;
    }

    /// External control between inputs does not start an input, run ordinary
    /// phases or clear deltas. Forced END births may allocate identities and
    /// consume RNG during construction/prewarm; retain those changes.
    pub fn control_roots(
        &mut self,
        file: &AvfxFile,
        control: impl FnOnce(&mut RootPreparation) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut context = self.context(file);
        control(&mut RootPreparation {
            roots: &mut self.roots,
            context: &mut context,
        })?;
        self.next_instance = context.next_instance;
        self.retained = context.retained;
        Ok(())
    }

    #[cfg(test)]
    pub fn update(&mut self, file: &AvfxFile, frames: f32) -> Result<(), String> {
        self.begin_input_update()?;
        let mut operations = OPERATION_LIMIT;
        self.update_with_budget(file, frames, &mut operations)
    }

    pub fn begin_input_update(&mut self) -> Result<(), String> {
        for root in &mut self.roots {
            if let Some(child) = &mut root.child {
                child.clear_input_delta();
            }
        }
        self.update = self
            .update
            .checked_add(1)
            .ok_or("creation trace update limit")?;
        Ok(())
    }

    #[cfg(test)]
    pub fn update_with_budget(
        &mut self,
        file: &AvfxFile,
        frames: f32,
        operations: &mut usize,
    ) -> Result<(), String> {
        self.update_with_budget_before_prepare(file, frames, operations, |_, _| Ok(()))
    }

    /// External Binder preparation follows the whole tree's age pass and
    /// precedes root emitter creation/preparation. New births use that cache.
    #[cfg(test)]
    pub fn update_with_budget_before_prepare(
        &mut self,
        file: &AvfxFile,
        frames: f32,
        operations: &mut usize,
        mut before_prepare: impl FnMut(usize, &mut RootPreparation) -> Result<(), String>,
    ) -> Result<(), String> {
        let input_end = self.elapsed_frames + frames.max(0.0);
        self.update_with_budget_custom_prepare(file, frames, operations, |preparation| {
            for root_index in 0..preparation.root_count() {
                before_prepare(root_index, preparation)?;
                preparation.prepare_root(root_index, input_end)?;
            }
            Ok(())
        })
    }

    /// The owner schedules parent factories and child traversal between the
    /// full Time pass and independent Numeric. New roots can join this pass.
    #[cfg(test)]
    pub fn update_with_budget_custom_prepare(
        &mut self,
        file: &AvfxFile,
        frames: f32,
        operations: &mut usize,
        mut prepare: impl FnMut(&mut RootPreparation) -> Result<(), String>,
    ) -> Result<(), String> {
        self.update_with_budget_custom_phases(file, frames, operations, |phase, roots| {
            match phase {
                VfxTracePhase::Advance => {
                    for index in 0..roots.root_count() {
                        roots.advance_root(index, frames)?;
                    }
                }
                VfxTracePhase::Prepare => prepare(roots)?,
                VfxTracePhase::Numeric => {
                    for index in 0..roots.root_count() {
                        roots.refresh_root_numeric(index)?;
                    }
                }
                _ => unreachable!(),
            }
            Ok(())
        })
    }

    /// Owners interleave their own phase callbacks with the corresponding
    /// emitter subtrees. A retirement callback can therefore END later siblings
    /// before their Time or Numeric callbacks run.
    pub fn update_with_budget_custom_phases(
        &mut self,
        file: &AvfxFile,
        frames: f32,
        operations: &mut usize,
        mut phase: impl FnMut(VfxTracePhase, &mut RootPreparation) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut context = self.context(file);
        context.operations = *operations;
        let input_end = self.elapsed_frames + frames.max(0.0);
        for current in [
            VfxTracePhase::Advance,
            VfxTracePhase::Prepare,
            VfxTracePhase::Numeric,
        ] {
            context.phase = current;
            phase(
                current,
                &mut RootPreparation {
                    roots: &mut self.roots,
                    context: &mut context,
                },
            )?;
        }
        self.next_instance = context.next_instance;
        self.retained = context.retained;
        self.elapsed_frames = input_end;
        *operations = context.operations;
        Ok(())
    }

    pub fn add_root_with_birth(
        &mut self,
        file: &AvfxFile,
        index: usize,
        life: f32,
        start_delay: f32,
        color_seed: u64,
        birth: Option<super::playback::ParticleBirthRoot>,
        operations: &mut usize,
    ) -> Result<(), String> {
        let mut context = self.context(file);
        context.operations = *operations;
        context.root_color_seed = Some(color_seed);
        context.particle_birth_root = birth.map(std::sync::Arc::new);
        let pending_start = (start_delay > 0.0).then_some((self.elapsed_frames + start_delay, 0.0));
        let child = if pending_start.is_some() {
            None
        } else {
            let id = context.allocate()?;
            Some(Node::emitter(
                id,
                index,
                InstanceClock::root(&file.emitters[index], life),
                None,
                &mut context,
                0,
            )?)
        };
        self.roots.push(RootSlot {
            color_seed: Some(color_seed),
            particle_birth_root: context.particle_birth_root.clone(),
            child,
            definition: index,
            life,
            binder: false,
            pending_start,
        });
        self.next_instance = context.next_instance;
        self.retained = context.retained;
        *operations = context.operations;
        Ok(())
    }

    pub fn retire_finished_roots_with_binders(&mut self, binder_retired: &[bool]) -> Vec<bool> {
        let mut retained_roots = Vec::with_capacity(self.roots.len());
        let mut index = 0;
        self.roots.retain(|root| {
            let retained = if root.binder {
                root.child.is_some() || !binder_retired[index]
            } else {
                root.pending_start.is_some()
                    || root
                        .child
                        .as_ref()
                        .is_some_and(|child| !child.dead || !child.children.is_empty())
            };
            index += 1;
            retained_roots.push(retained);
            if !retained && root.child.is_some() {
                self.retained -= 1;
            }
            retained
        });
        retained_roots
    }

    pub fn input_update(&self) -> usize {
        self.update
    }

    /// Keep the external cache used by every retained birth path, including
    /// finished ancestors and emitters which have not created a particle yet.
    pub fn oldest_birth_update(&self, root: usize) -> Option<usize> {
        fn path_min(
            path: &[EmitterSnapshot],
            seen: &mut HashSet<(u64, usize)>,
            oldest: &mut Option<usize>,
        ) {
            for snapshot in path {
                if seen.insert((snapshot.id, snapshot.input_update)) {
                    *oldest = Some(oldest.map_or(snapshot.input_update, |value| {
                        value.min(snapshot.input_update)
                    }));
                    path_min(&snapshot.birth_path, seen, oldest);
                }
            }
        }
        fn visit(node: &Node, seen: &mut HashSet<(u64, usize)>, oldest: &mut Option<usize>) {
            path_min(&node.birth_path, seen, oldest);
            for child in &node.children {
                visit(child, seen, oldest);
            }
        }
        let mut oldest = None;
        if let Some(child) = &self.roots[root].child {
            visit(child, &mut HashSet::new(), &mut oldest);
        }
        oldest
    }

    pub fn root_id(&self, root: usize) -> Option<u64> {
        self.roots[root].child.as_ref().map(|child| child.id)
    }

    #[cfg(test)]
    pub fn common_tree_states(&self, root: usize) -> Vec<CommonNodeSnapshot> {
        fn visit(node: &Node, output: &mut Vec<CommonNodeSnapshot>) {
            output.push(CommonNodeSnapshot {
                flags: node.common_flags,
                children: node.children.len(),
                numeric: node.numeric,
                delta: node.delta,
                age: node.age,
                total: node.total_age,
            });
            for child in &node.children {
                visit(child, output)
            }
        }
        let mut output = Vec::new();
        if let Some(child) = &self.roots[root].child {
            visit(child, &mut output)
        }
        output
    }

    #[cfg(test)]
    pub fn curve_ages(&self, root: usize) -> super::CurveAges {
        super::CurveAges {
            local: self.roots[root].child.as_ref().unwrap().curve_age,
            total: self.roots[root].child.as_ref().unwrap().curve_total_age,
        }
    }

    pub fn needs_particle_motion_birth(&self) -> bool {
        fn needs(node: &Node) -> bool {
            (!node.dead
                && node
                    .client_injection
                    .as_ref()
                    .is_some_and(|cache| cache.needs_birth()))
                || node.children.iter().any(needs)
        }
        self.roots
            .iter()
            .filter_map(|root| root.child.as_ref())
            .any(needs)
    }

    pub fn particle_textures(
        &self,
        index: usize,
        state: super::particle_curves::VfxClientParticleCurveState,
        particle: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
    ) -> Option<super::particle_curves::VfxClientParticleTextureValues> {
        self.color_environment
            .as_ref()?
            .evaluate_particle_textures(index, state, particle, ages)
    }

    pub fn particles(&self, root: usize) -> impl Iterator<Item = ParticleView> + '_ {
        let mut views = Vec::new();
        if let Some(root_node) = &self.roots[root].child {
            let mut path = Vec::new();
            collect_particles(root_node, self.update, &mut path, &mut views);
        }
        views.into_iter()
    }
}

fn collect_particles(
    node: &Node,
    input_update: usize,
    path: &mut Vec<EmitterSnapshot>,
    views: &mut Vec<ParticleView>,
) {
    let pushed = node.kind == VfxTraceTarget::Emitter;
    if pushed {
        path.push(node.emitter_snapshot(input_update));
    }
    for child in &node.children {
        if child.kind == VfxTraceTarget::Particle && !child.dead {
            let source = child.source.expect("particle source must be retained");
            views.push(ParticleView {
                client_particle: child.client_particle,
                client_particle_xyz: child.client_particle.map(|state| state.cache),
                client_injection: child.client_injection.clone(),
                client_model_skin: child.client_model_skin.map(|state| state.cache),
                client_model_skin_uv: child
                    .client_model_skin_uv
                    .as_ref()
                    .map(|states| states.iter().map(|state| state.cache).collect()),
                model_skin_creation_serial: child.model_skin_creation_serial,
                model_skin_numeric_revision: child
                    .model_skin_target
                    .map(|_| child.model_skin_numeric_revision),
                model_skin_numeric_color: child.model_skin_numeric_color,
                model_skin_numeric_targets: child.model_skin_numeric_targets,
                color: child.color,
                parent_color: match child.color_parent_mode {
                    1 => Some(child.initial_parent_color),
                    2 | 8 => Some(node.color),
                    _ => None,
                },
                birth_path: child.birth_path.clone(),
                id: child.id,
                definition: child.definition,
                emitter_definition: node.definition,
                emitter_id: node.id,
                emitter_curve_age: node.curve_age,
                emitter_curve_total_age: node.curve_total_age,
                emitter_gravity_offset: node.gravity_offset,
                item_index: source.index,
                curve_age: child.curve_age,
                curve_total_age: child.curve_total_age,
                render_curve_age: child.render_curve_age,
                render_curve_total_age: child.render_curve_total_age,
                age: child.age,
                total_age: child.total_age,
                input_delta: child.input_delta,
                powder_prewarm: child.powder_prewarm.clone(),
                parent_curve_age: child.parent_curve_age,
                parent_curve_total_age: child.parent_curve_total_age,
                parent_gravity_offset: child.parent_gravity_offset,
                gravity_offset: child.gravity_offset,
                injection_ordinal: child.injection_ordinal,
                shape_ordinal: child.shape_ordinal,
                emitter_path: path.clone(),
            });
        }
        // A finished emitter can retain live particle tails until they retire.
        if child.kind == VfxTraceTarget::Emitter {
            collect_particles(child, input_update, path, views);
        }
    }
    if pushed {
        path.pop();
    }
}

pub(super) fn validate(
    file: &AvfxFile,
    index: usize,
    visited: &mut [bool],
    depth: usize,
) -> Result<(), String> {
    validate_inner(file, index, visited, depth, false, &HashSet::new())
}

pub(super) fn validate_staged(
    file: &AvfxFile,
    index: usize,
    visited: &mut [bool],
    depth: usize,
    model_skin_targets: &HashSet<usize>,
) -> Result<(), String> {
    validate_inner(file, index, visited, depth, true, model_skin_targets)
}

fn validate_inner(
    file: &AvfxFile,
    index: usize,
    visited: &mut [bool],
    depth: usize,
    allow_sphere_model: bool,
    model_skin_targets: &HashSet<usize>,
) -> Result<(), String> {
    if depth > 64 {
        return Err("creation trace nesting limit".into());
    }
    let emitter = file
        .emitters
        .get(index)
        .ok_or_else(|| format!("invalid emitter index {index}"))?;
    if visited[index] {
        return Ok(());
    }
    visited[index] = true;
    let nonzero = |curve: &crate::avfx::AvfxCurve| curve.keys.iter().any(|key| key.z != 0.0);
    // Life ValR is selected when the child clock is constructed. It does not
    // depend on the shape used by the parent emitter.
    let allow_random_lifetime = allow_sphere_model;
    if !matches!(emitter.emitter_type, Some(crate::avfx::EmitterType::Point))
        && !(allow_sphere_model
            && matches!(
                emitter.emitter_type,
                Some(crate::avfx::EmitterType::Cone)
                    | Some(crate::avfx::EmitterType::ConeModel)
                    | Some(crate::avfx::EmitterType::CylinderModel)
                    | Some(crate::avfx::EmitterType::Model)
                    | Some(crate::avfx::EmitterType::SphereModel)
            ))
    {
        return Err(format!(
            "Emit[{index}]: probe requires Point or supported staged shape"
        ));
    }
    if emitter.effector_index != -1 {
        let effector = usize::try_from(emitter.effector_index)
            .ok()
            .and_then(|index| file.effectors.get(index))
            .ok_or_else(|| format!("Emit[{index}]: invalid emitter effector"))?;
        if effector.affect_other_vfx {
            return Err(format!(
                "Emit[{index}]: probe excludes VFX-affecting emitter effector"
            ));
        }
    }
    if depth > 0
        && emitter
            .rotation_velocity
            .iter()
            .chain(&emitter.rotation_velocity_random)
            .any(|curve| curve.keys.iter().any(|key| key.z != 0.0))
    {
        return Err(format!(
            "Emit[{index}]: staged playback excludes nonzero emitter VR history"
        ));
    }
    if nonzero(&emitter.create_interval_random) && !allow_sphere_model {
        return Err(format!(
            "Emit[{index}]: probe excludes nonzero CrIR interval randomness"
        ));
    }
    if nonzero(&emitter.create_count_random) && !allow_sphere_model {
        return Err(format!(
            "Emit[{index}]: probe excludes nonzero CrCR count randomness"
        ));
    }
    for (kind, items) in [(0, &emitter.particle_items), (1, &emitter.emitter_items)] {
        for (item_index, item) in items.iter().enumerate().filter(|(_, item)| item.enabled) {
            let location = format!(
                "Emit[{index}].{}[{item_index}]",
                if kind == 0 { "ItPr" } else { "ItEm" }
            );
            if item.parameter_link != -1
                || item.generate_delay < 0
                || !(0..=100).contains(&item.create_probability)
                || !(0..=2).contains(&item.create_time)
            {
                return Err(format!(
                    "{location}: unsupported PrLk, negative GenD, probability or CrTm"
                ));
            }
            let target = usize::try_from(item.target_index)
                .map_err(|_| format!("{location}: invalid target"))?;
            let life = if kind == 0 {
                let particle = file
                    .particles
                    .get(target)
                    .ok_or_else(|| format!("{location}: invalid target"))?;
                let model_skin =
                    particle.particle_type == Some(crate::avfx::ParticleType::ModelSkin);
                let model_skin_ready = model_skin && model_skin_targets.contains(&target);
                if model_skin && !model_skin_ready {
                    return Err(format!(
                        "{location}: staged ModelSkin requires a compatible model surface target"
                    ));
                }
                if model_skin_ready
                    && matches!(
                        &particle.data,
                        crate::avfx::AvfxParticleData::ModelSkin(data)
                            if data.fresnel_type & 3 == 3
                    )
                    && !matches!(particle.rotation_direction_base, 0..=10)
                {
                    return Err(format!(
                        "{location}: staged ModelSkin FrsT=3 requires an unsupported RBDT facing basis"
                    ));
                }
                if !model_skin_ready
                    && !matches!(
                        particle.particle_type,
                        Some(
                            crate::avfx::ParticleType::Quad
                                | crate::avfx::ParticleType::Powder
                                | crate::avfx::ParticleType::Disc
                                | crate::avfx::ParticleType::Polygon
                                | crate::avfx::ParticleType::Laser
                                | crate::avfx::ParticleType::Line
                                | crate::avfx::ParticleType::Polyline
                                | crate::avfx::ParticleType::Windmill
                                | crate::avfx::ParticleType::Decal
                                | crate::avfx::ParticleType::DecalRing
                                | crate::avfx::ParticleType::Model
                                | crate::avfx::ParticleType::LightModel
                        )
                    )
                {
                    return Err(format!("{location}: unsupported staged particle type"));
                }
                if particle.collision_type != -1 {
                    return Err(format!("{location}: staged collisions are not implemented"));
                }
                Some(particle.life)
            } else {
                file.emitters.get(target).map(|emitter| emitter.life)
            }
            .ok_or_else(|| format!("{location}: invalid target"))?;
            if !life.value.is_finite()
                || (!allow_random_lifetime && life.enabled && life.value_random != 0.0)
            {
                return Err(format!(
                    "{location}: unsupported nonfinite or random lifetime"
                ));
            }
            if kind == 1 {
                validate_inner(
                    file,
                    target,
                    visited,
                    depth + 1,
                    allow_sphere_model,
                    model_skin_targets,
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{
        AvfxCurve, AvfxCurveKey, AvfxEmitter, AvfxLife, AvfxParticle, EmitterType, ParticleType,
    };

    #[test]
    #[ignore = "CPU: original property/Prepare cached-color trees versus production"]
    fn compare_original_emitter_color_phases() {
        use crate::avfx::{AvfxColorCurve, AvfxColorScaleRgb};
        use serde_json::json;
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("emitter-color-client-probe.json")).unwrap(),
        )
        .unwrap();
        let bases = [
            [0.5, 0.75, 1.0, 0.5],
            [0.75, 0.5, 0.25, 0.75],
            [0.5, 1.0, 0.75, 0.5],
            [0.75, 0.5, 1.0, 0.75],
        ];
        let slopes = [
            [0.125, 0.0625, 0.0, 0.125],
            [0.0625, 0.0, 0.125, 0.0625],
            [0.0, 0.125, 0.0625, 0.0],
            [0.0625, 0.0, 0.0625, 0.0625],
        ];
        let channel = |index: usize, axis: usize| AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    z: bases[index][axis],
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 16,
                    z: bases[index][axis] + 16.0 * slopes[index][axis],
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        let color = |index| AvfxColorCurve {
            alpha: Some(channel(index, 3)),
            scale_rgb: Some(AvfxColorScaleRgb {
                r: Some(channel(index, 0)),
                g: Some(channel(index, 1)),
                b: Some(channel(index, 2)),
            }),
            ..Default::default()
        };
        let mut components = 0;
        let mut compare = |actual: [f32; 4], expected: &serde_json::Value| {
            for axis in 0..4 {
                assert_eq!(
                    actual[axis].to_bits() as u64,
                    expected[axis].as_u64().unwrap(),
                    "axis={axis}, actual={actual:?}, expected={expected}"
                );
                components += 1;
            }
        };
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 432);
        let mut inputs = 0;
        for case in cases {
            let mut file = fixture();
            file.emitters[0].particle_items.clear();
            file.emitters[0].create_interval = constant(1000.0);
            for _ in 1..3 {
                file.emitters.push(file.emitters[0].clone());
            }
            for index in 0..3 {
                file.emitters[index].color = color(index);
                if index < 2 {
                    file.emitters[index].emitter_items.push(AvfxEmitterItem {
                        target_index: (index + 1) as i32,
                        parent_influence_color: case[if index == 0 {
                            "childMode"
                        } else {
                            "grandchildMode"
                        }]
                        .as_i64()
                        .unwrap() as i32,
                        ..item(1)
                    });
                }
            }
            file.emitters[2].particle_items.push(AvfxEmitterItem {
                parent_influence_color: case["particleMode"].as_i64().unwrap() as i32,
                ..item(1)
            });
            file.particles[0].color = color(3);
            let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
            let check_emitters =
                |root: &Node,
                 values: &serde_json::Value,
                 compare: &mut dyn FnMut([f32; 4], &serde_json::Value)| {
                    compare(root.color, &values[0]);
                    compare(root.children[0].color, &values[1]);
                    compare(root.children[0].children[0].color, &values[2]);
                    compare(root.children[0].children[0].children[0].color, &values[3]);
                };
            check_emitters(
                state.roots[0].child.as_ref().unwrap(),
                &case["initial"],
                &mut compare,
            );
            for (step, original) in case["steps"].as_array().unwrap().iter().enumerate() {
                state.begin_input_update().unwrap();
                if case["retireRoot"].as_bool().unwrap() && step == 2 {
                    // Controlled self retirement matches the probe's bit mask;
                    // this is not recursive END or original finite-Life execution.
                    state.roots[0].child.as_mut().unwrap().mark_dead();
                }
                let mut context = state.context(&file);
                context.phase = VfxTracePhase::Advance;
                let root = state.roots[0].child.as_mut().unwrap();
                let delta = f32::from_bits(original["delta"].as_u64().unwrap() as u32);
                root.advance(delta, false, &mut context, 0, None).unwrap();
                check_emitters(root, &original["afterTime"], &mut compare);
                context.phase = VfxTracePhase::Prepare;
                root.prepare(&mut context, 0).unwrap();
                root.prepare_particles(&mut context).unwrap();
                check_emitters(root, &original["afterPrepare"], &mut compare);
                let particle = state.particles(0).next().unwrap();
                let local = particle.color;
                let draw = particle.parent_color.map_or(local, |parent| {
                    std::array::from_fn(|axis| parent[axis] * local[axis])
                });
                compare(draw, &original["drawColor"]);
                inputs += 1;
            }
        }
        let prewarm_cases = original["prewarmCases"].as_array().unwrap();
        assert_eq!(prewarm_cases.len(), 36);
        let mut prewarm_inputs = 0;
        for case in prewarm_cases {
            let mut file = fixture();
            file.emitters[0].particle_items.clear();
            file.emitters[0].create_interval = constant(1000.0);
            for _ in 1..3 {
                file.emitters.push(file.emitters[0].clone());
            }
            for index in 0..3 {
                file.emitters[index].color = color(index);
                if index < 2 {
                    file.emitters[index].emitter_items.push(AvfxEmitterItem {
                        target_index: (index + 1) as i32,
                        ..item(1)
                    });
                }
            }
            file.emitters[2].particle_items.push(AvfxEmitterItem {
                parent_influence_color: case["particleMode"].as_i64().unwrap() as i32,
                start_frame: case["prewarmFrames"].as_i64().unwrap() as i32,
                start_frame_null_update: true,
                ..item(1)
            });
            file.particles[0].color = color(3);
            if case["skin"].as_bool().unwrap() {
                file.particles[0].particle_type = Some(ParticleType::ModelSkin);
            }
            let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
            let mut check = |state: &Instances, original: &serde_json::Value| {
                let particle = state.particles(0).next().unwrap();
                compare(particle.color, &original["cached"]);
                let draw = particle.parent_color.map_or(particle.color, |parent| {
                    std::array::from_fn(|axis| parent[axis] * particle.color[axis])
                });
                compare(draw, &original["drawColor"]);
            };
            check(&state, case);
            for original in case["steps"].as_array().unwrap() {
                state.begin_input_update().unwrap();
                let mut context = state.context(&file);
                let root = state.roots[0].child.as_mut().unwrap();
                let delta = f32::from_bits(original["delta"].as_u64().unwrap() as u32);
                root.advance(delta, false, &mut context, 0, None).unwrap();
                root.prepare(&mut context, 0).unwrap();
                root.prepare_particles(&mut context).unwrap();
                check(&state, original);
                prewarm_inputs += 1;
            }
        }
        std::fs::write(directory.join("emitter-color-comparison.json"),serde_json::to_vec_pretty(
            &json!({"cases":cases.len(),"inputs":inputs,"prewarmCases":prewarm_cases.len(),"prewarmInputs":prewarm_inputs,"finiteBitComparisons":components,"differences":0,
                "scope":"Original Common Time, null-update, Prepare and independent numeric passes versus production emitter and particle color caches. Actual Quad vtable/time/empty prewarm and ModelSkin prewarm/+130 execute. ModelSkin normal +f0 uses controlled Quad property dispatch and its +100 attachment provider is no-op; Quad +100 executes with registration/motion controlled. Curves, private transforms, constructor/snapshot/flag initialization, root self retirement and restore-after-draw boundary controlled. Cached base alpha is now retained; full numeric fade/KILL tree/deadline, global adjustments, shared RNG and GPU/client pixels not verified."}),
        ).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "CPU: original Common numeric tree versus production owned caches"]
    fn compare_original_owned_numeric_trees() {
        use crate::avfx::{AvfxColorCurve, AvfxColorScaleRgb};
        use serde_json::json;
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("owned-numeric-client-probe.json")).unwrap(),
        )
        .unwrap();
        let bases = [
            [0.5, 0.75, 1.0, 0.5],
            [0.75, 0.5, 0.25, 0.75],
            [0.5, 1.0, 0.75, 0.5],
            [0.75, 0.5, 1.0, 0.75],
        ];
        let slopes = [
            [0.125, 0.0625, 0.0, 0.125],
            [0.0625, 0.0, 0.125, 0.0625],
            [0.0, 0.125, 0.0625, 0.0],
            [0.0625, 0.0, 0.0625, 0.0625],
        ];
        let channel = |index: usize, axis: usize| AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    z: bases[index][axis],
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 16,
                    z: bases[index][axis] + 16.0 * slopes[index][axis],
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        let color = |index| AvfxColorCurve {
            alpha: Some(channel(index, 3)),
            scale_rgb: Some(AvfxColorScaleRgb {
                r: Some(channel(index, 0)),
                g: Some(channel(index, 1)),
                b: Some(channel(index, 2)),
            }),
            ..Default::default()
        };
        fn find(node: &Node, id: u64) -> Option<&Node> {
            if node.id == id {
                return Some(node);
            }
            node.children.iter().find_map(|child| find(child, id))
        }
        let mut finite = 0;
        let mut nan = 0;
        let mut inputs = 0;
        let mut float = |actual: f32, expected: &serde_json::Value| {
            let bits = expected.as_u64().unwrap() as u32;
            if f32::from_bits(bits).is_nan() {
                assert!(actual.is_nan());
                nan += 1;
            } else {
                assert_eq!(
                    actual.to_bits(),
                    bits,
                    "actual={actual},expected={expected}"
                );
                finite += 1;
            }
        };
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 192);
        for (case_index, case) in cases.iter().enumerate() {
            let mut file = fixture();
            file.emitters[0].particle_items.clear();
            file.emitters[0].create_interval = constant(1000.0);
            for _ in 1..3 {
                file.emitters.push(file.emitters[0].clone());
            }
            let pic = case["parentColorMode"].as_i64().unwrap() as i32;
            for index in 0..3 {
                file.emitters[index].color = color(index);
                if index < 2 {
                    file.emitters[index].emitter_items.push(AvfxEmitterItem {
                        target_index: (index + 1) as i32,
                        parent_influence_color: pic,
                        ..item(1)
                    });
                }
            }
            file.emitters[2].particle_items.push(AvfxEmitterItem {
                parent_influence_color: pic,
                ..item(1)
            });
            file.emitters[0].particle_items.push(AvfxEmitterItem {
                parent_influence_color: 1,
                ..item(2)
            });
            file.particles[0].color = color(3);
            let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
            let compare =
                |root: &Node,
                 values: &serde_json::Value,
                 float: &mut dyn FnMut(f32, &serde_json::Value)| {
                    for (index, value) in values.as_array().unwrap().iter().enumerate() {
                        let node = find(root, index as u64);
                        assert_eq!(
                            node.is_some(),
                            value["registered"].as_bool().unwrap(),
                            "case={case_index}, node={index}, values={values}"
                        );
                        let Some(node) = node else { continue };
                        assert_eq!(
                            node.common_flags & 0x7f3c0000,
                            value["flags"].as_u64().unwrap() as u32,
                            "case={case_index}, index={index}, id={}, values={values}",
                            node.id
                        );
                        float(node.numeric.fade.duration, &value["duration"]);
                        float(node.numeric.fade.age, &value["fadeAge"]);
                        float(node.delta, &value["delta"]);
                        float(node.age, &value["age"]);
                        float(node.total_age, &value["totalAge"]);
                        for axis in 0..4 {
                            float(node.color[axis], &value["color"][axis]);
                        }
                        assert_eq!(node.numeric.alpha.to_bits(), node.color[3].to_bits());
                        if index == 4 {
                            for axis in 0..4 {
                                float(
                                    node.initial_parent_color[axis],
                                    &value["birthParentColor"][axis],
                                );
                            }
                        }
                    }
                };
            for (step_index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
                state.begin_input_update().unwrap();
                let delta = f32::from_bits(step["delta"].as_u64().unwrap() as u32);
                let input_end = state.elapsed_frames + delta;
                let mut operations = OPERATION_LIMIT;
                state
                    .update_with_budget_custom_prepare(&file, delta, &mut operations, |p| {
                        if step_index == 0 {
                            p.configure_root_fade(
                                0,
                                case["duration"].as_i64().unwrap() as i32,
                                case["mode"].as_u64().unwrap() as u32,
                                case["flag"].as_bool().unwrap(),
                            )?;
                            if !case["selfEnabled"].as_bool().unwrap() {
                                p.roots[0].child.as_mut().unwrap().common_flags &= !0x0400_0000;
                            }
                        }
                        p.prepare_root(0, input_end)?;
                        compare(
                            p.roots[0].child.as_ref().unwrap(),
                            &step["beforeNumeric"],
                            &mut float,
                        );
                        Ok(())
                    })
                    .unwrap();
                let root = state.roots[0].child.as_ref().unwrap();
                compare(root, &step["afterNumeric"], &mut float);
                assert_eq!(
                    root.emitter.as_ref().unwrap().counters[0][0] != 0,
                    step["bornOnce"].as_bool().unwrap()
                );
                assert!(
                    step["retireCallbacks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|x| x.as_u64().unwrap() <= 1)
                );
                inputs += 1;
            }
        }
        std::fs::write(directory.join("owned-numeric-comparison.json"),serde_json::to_vec_pretty(&json!({
            "cases":cases.len(),"inputs":inputs,"finiteBitComparisons":finite,"nanClassifications":nan,"differences":0,
            "scope":"Original Common Time/Prepare/+78, recursive fade and forced Attach versus actual production Node emitter/particle caches, flags, fade/instance clocks, pre-zero birth snapshots and same-pass terminal creation/next-Time collection. Actual Quad properties/numeric and original Emitter numeric no-op execute; UV/motion/registration/private transforms/curve readers, constructors/+08 and emitter +10 terminal factory controlled. Global +18 disabled. Playback Clip/Scheduler/Binder ownership, KILL deadline and pixels not verified."
        })).unwrap()).unwrap();
    }

    fn constant(value: f32) -> AvfxCurve {
        AvfxCurve {
            keys: vec![AvfxCurveKey {
                time: 0,
                interpolation: 1,
                z: value,
                x: 0.0,
                y: 0.0,
            }],
            ..Default::default()
        }
    }

    fn item(trigger: i32) -> AvfxEmitterItem {
        AvfxEmitterItem {
            enabled: true,
            target_index: 0,
            create_time: trigger,
            create_count: 1,
            create_probability: 100,
            parameter_link: -1,
            ..Default::default()
        }
    }

    fn emitter() -> AvfxEmitter {
        AvfxEmitter {
            emitter_type: Some(EmitterType::Point),
            effector_index: -1,
            create_count: constant(1.0),
            create_interval: constant(2.0),
            particle_items: vec![item(0)],
            ..Default::default()
        }
    }

    fn fixture() -> AvfxFile {
        AvfxFile {
            emitters: vec![emitter()],
            particles: vec![AvfxParticle {
                particle_type: Some(ParticleType::Quad),
                collision_type: -1,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn owned_numeric_pass_fades_real_caches_and_visits_live_descendants() {
        for self_enabled in [true, false] {
            let mut file = fixture();
            file.emitters[0].create_interval = constant(1000.0);
            file.emitters[0].color.alpha = Some(constant(0.8));
            file.emitters[0].particle_items[0].create_time = 1;
            file.emitters[0].particle_items[0].parent_influence_color = 2;
            file.particles[0].color.alpha = Some(constant(0.6));
            let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
            let mut operations = OPERATION_LIMIT;
            state
                .update_with_budget_custom_prepare(&file, 2.0, &mut operations, |p| {
                    p.configure_root_fade(0, 4, 2, false)?;
                    if !self_enabled {
                        p.roots[0].child.as_mut().unwrap().common_flags &= !0x0400_0000;
                    }
                    p.prepare_root(0, 2.0)
                })
                .unwrap();
            let root = state.roots[0].child.as_ref().unwrap();
            assert_eq!(root.color[3], if self_enabled { 0.4 } else { 0.8 });
            assert_eq!(root.numeric.fade.age, if self_enabled { 2.0 } else { 0.0 });
            let particle = state.particles(0).next().unwrap();
            assert_eq!(particle.color[3], 0.3);
            assert_eq!(
                particle.color[3] * particle.parent_color.unwrap()[3],
                if self_enabled { 0.3 * 0.4 } else { 0.3 * 0.8 }
            );
            state.update(&file, 0.0).unwrap();
            let root = state.roots[0].child.as_ref().unwrap();
            assert_eq!(root.color[3], if self_enabled { 0.4 } else { 0.8 });
            assert_eq!(state.particles(0).next().unwrap().color[3], 0.3);
        }
    }

    #[test]
    fn numeric_retirement_birth_captures_pre_zero_alpha_and_joins_same_pass() {
        let mut file = fixture();
        file.emitters[0].create_interval = constant(1000.0);
        file.emitters[0].color.alpha = Some(constant(0.8));
        file.emitters[0].particle_items[0].create_time = 1;
        file.emitters[0].particle_items.push(AvfxEmitterItem {
            parent_influence_color: 1,
            ..item(2)
        });
        let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        let mut operations = OPERATION_LIMIT;
        state
            .update_with_budget_custom_prepare(&file, 4.0, &mut operations, |p| {
                p.configure_root_fade(0, 4, 2, false)?;
                p.prepare_root(0, 4.0)
            })
            .unwrap();
        let root = state.roots[0].child.as_ref().unwrap();
        assert!(!root.dead, "equality does not retire");
        assert_eq!(root.color[3], 0.0);
        state.update(&file, 0.25).unwrap();
        let root = state.roots[0].child.as_ref().unwrap();
        assert!(root.dead);
        assert_eq!(root.color[3], 0.0);
        assert_eq!(root.children.len(), 2, "+10 birth is still registered");
        let born = &root.children[1];
        assert_eq!(born.initial_parent_color[3], 0.8, "+10 precedes zero write");
        assert_eq!(born.numeric.fade.age, 4.25, "copies current parent fade");
        assert_eq!(born.color[3], 0.0);
        assert!(born.dead, "new child joins this numeric traversal");
        assert_eq!(state.particles(0).count(), 0);
        state.update(&file, 0.0).unwrap();
        assert!(state.roots[0].child.as_ref().unwrap().children.is_empty());
    }

    #[test]
    fn late_attachment_inherits_self_after_initializing_its_grandchildren() {
        let mut file = fixture();
        file.emitters[0].create_interval = constant(1000.0);
        let mut child = file.emitters[0].clone();
        child.particle_items[0].create_time = 1;
        file.emitters.push(child);
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items.push(AvfxEmitterItem {
            target_index: 1,
            generate_delay: 1,
            ..item(1)
        });
        let mut state = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        let mut operations = OPERATION_LIMIT;
        state
            .update_with_budget_custom_prepare(&file, 1.0, &mut operations, |p| {
                p.configure_root_fade(0, 4, 2, false)?;
                p.prepare_root(0, 1.0)
            })
            .unwrap();
        let root = state.roots[0].child.as_ref().unwrap();
        assert_eq!(root.numeric.fade.age, 1.0);
        let child = root
            .children
            .iter()
            .find(|child| child.emitter.is_some())
            .unwrap();
        assert_eq!(child.numeric.fade.duration, 4.0);
        assert_eq!(
            child.numeric.fade.age, 0.0,
            "birth does not borrow elapsed Time"
        );
        assert_eq!(child.common_flags & 0x4038_0000, 0x4010_0000);
        let grandchild = &child.children[0];
        assert_eq!(grandchild.common_flags & 0x4038_0000, 0);
        assert_eq!(
            grandchild.numeric.fade,
            super::super::VfxCommonFadeState::default()
        );
        state.update(&file, 1.0).unwrap();
        let root = state.roots[0].child.as_ref().unwrap();
        let child = root
            .children
            .iter()
            .find(|child| child.emitter.is_some())
            .unwrap();
        assert_eq!(child.numeric.fade.age, 1.0);
        assert_eq!(child.children[0].numeric.fade.age, 0.0);
    }

    #[test]
    fn always_gravity_draws_follow_callbacks_not_elapsed_frame_ages() {
        for particle in [false, true] {
            for mode in [1, 3, 4, 5] {
                let mut file = fixture();
                file.emitters[0].particle_items[0].create_time = 1;
                let random = AvfxCurve {
                    random_type: mode,
                    ..constant(100.0)
                };
                if particle {
                    file.particles[0].gravity_random = random;
                } else {
                    file.emitters[0].gravity_random = random;
                }
                let mut short = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
                let mut long = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
                let motion = |instances: &Instances| {
                    let root = instances.roots[0].child.as_ref().unwrap();
                    let node = if particle { &root.children[0] } else { root };
                    (node.gravity_velocity, node.gravity_offset)
                };
                for _ in 0..2 {
                    short.update(&file, 1.0).unwrap();
                    long.update(&file, 2.0).unwrap();
                    let (short_velocity, short_offset) = motion(&short);
                    let (long_velocity, long_offset) = motion(&long);
                    assert_eq!(
                        long_velocity,
                        short_velocity * 2.0,
                        "particle={particle}, mode={mode}"
                    );
                    assert_eq!(
                        long_offset,
                        short_offset * 4.0,
                        "particle={particle}, mode={mode}"
                    );
                }
            }
        }
    }

    fn trace(file: &AvfxFile, frames: &[f32]) -> VfxCreationTrace {
        VfxRuntime::new(file)
            .trace_emitter_updates(
                0,
                -1.0,
                &frames.iter().map(|frame| frame / 30.0).collect::<Vec<_>>(),
            )
            .unwrap()
    }

    fn births(trace: &VfxCreationTrace) -> Vec<(usize, u64, f32)> {
        trace
            .events
            .iter()
            .filter(|event| {
                event.action == VfxTraceAction::Create && event.target == VfxTraceTarget::Particle
            })
            .map(|event| (event.update, event.parent_instance, event.emitter_age))
            .collect()
    }

    #[test]
    fn long_updates_emit_once_and_preserve_negative_overshoot() {
        let file = fixture();
        assert_eq!(births(&trace(&file, &[10.0])), [(0, 0, 0.0), (1, 0, 10.0)]);
        assert_eq!(births(&trace(&file, &[2.0; 5])).len(), 6);
        assert_eq!(
            births(&trace(&file, &[2.5, 1.5, 0.5])),
            [(0, 0, 0.0), (1, 0, 2.5), (2, 0, 4.0)]
        );
        // A nonpositive countdown recovers once without subtracting delta;
        // a subsequent zero update with a positive countdown does nothing.
        assert_eq!(
            births(&trace(&file, &[10.0, 0.0, 0.0])),
            [(0, 0, 0.0), (1, 0, 10.0), (2, 0, 10.0)]
        );
    }

    #[test]
    fn periodic_creation_recovers_when_the_curve_becomes_positive() {
        let mut file = fixture();
        file.emitters[0].create_interval = AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: 2,
                    z: -1.0,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 3,
                    interpolation: 2,
                    z: 2.0,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            births(&trace(&file, &[2.0, 1.0, 2.0])),
            [(0, 0, 0.0), (2, 0, 3.0), (3, 0, 5.0)]
        );
    }

    #[test]
    fn staged_interval_randomness_changes_countdown_and_resamples_always() {
        let mut file = fixture();
        file.emitters[0].create_interval = constant(10.0);
        file.emitters[0].create_interval_random = constant(3.0);
        file.emitters[0].create_interval_random.random_type = 4;
        let mut instances = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        let first = instances.roots[0]
            .child
            .as_ref()
            .unwrap()
            .emitter
            .as_ref()
            .unwrap()
            .countdown;
        assert!((10.0..13.0).contains(&first), "first interval={first}");
        instances.update(&file, first).unwrap();
        let second = instances.roots[0]
            .child
            .as_ref()
            .unwrap()
            .emitter
            .as_ref()
            .unwrap()
            .countdown;
        assert!((10.0..13.0).contains(&second), "second interval={second}");
        assert_ne!(second, first);
        let same_age_first = instances.roots[0]
            .child
            .as_mut()
            .unwrap()
            .creation_interval(&file);
        let same_age_second = instances.roots[0]
            .child
            .as_mut()
            .unwrap()
            .creation_interval(&file);
        assert_ne!(same_age_first, same_age_second);

        file.emitters[0].create_interval_random.random_type = 1;
        let mut instances = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        let first = instances.roots[0]
            .child
            .as_ref()
            .unwrap()
            .emitter
            .as_ref()
            .unwrap()
            .countdown;
        instances.update(&file, first).unwrap();
        assert_eq!(
            instances.roots[0]
                .child
                .as_ref()
                .unwrap()
                .emitter
                .as_ref()
                .unwrap()
                .countdown,
            first
        );
    }

    #[test]
    fn staged_interval_randomness_can_restart_a_nonpositive_countdown() {
        let mut file = fixture();
        file.emitters[0].create_interval = AvfxCurve {
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                    z: -2.0,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 3,
                    interpolation: AvfxCurveKey::INTERPOLATION_STEP,
                    z: 0.0,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        file.emitters[0].create_interval_random = constant(2.0);
        file.emitters[0].create_interval_random.random_type = 4;
        let mut instances = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        assert_eq!(instances.roots[0].child.as_ref().unwrap().children.len(), 1);
        assert!(
            instances.roots[0]
                .child
                .as_ref()
                .unwrap()
                .emitter
                .as_ref()
                .unwrap()
                .countdown
                <= 0.0
        );
        instances.update(&file, 3.0).unwrap();
        assert_eq!(instances.roots[0].child.as_ref().unwrap().children.len(), 2);
    }

    #[test]
    fn staged_first_random_creation_count_is_stable_across_batches() {
        let mut file = fixture();
        file.emitters[0].create_interval = constant(2.0);
        file.emitters[0].create_count = constant(1.0);
        file.emitters[0].create_count_random = constant(100.0);
        file.emitters[0].create_count_random.random_type = 1;
        let mut instances = Instances::new(&file, &[(0, -1.0, 0.0)]).unwrap();
        let first = instances.roots[0].child.as_ref().unwrap().children.len();
        assert!(first > 1);
        instances.update(&file, 2.0).unwrap();
        let second = instances.roots[0].child.as_ref().unwrap().children.len() - first;
        instances.update(&file, 2.0).unwrap();
        let third = instances.roots[0].child.as_ref().unwrap().children.len() - first - second;
        assert_eq!((first, second, third), (first, first, first));
    }

    #[test]
    fn periodic_interval_relatched_after_loop_uses_accumulated_age_for_add() {
        let mut file = fixture();
        file.emitters[0].loop_start = 0;
        file.emitters[0].loop_end = 2;
        file.emitters[0].create_interval = AvfxCurve {
            post_behavior: crate::avfx::BEHAVIOR_ADD,
            keys: vec![
                AvfxCurveKey {
                    time: 0,
                    interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                    z: 2.0,
                    x: 0.0,
                    y: 0.0,
                },
                AvfxCurveKey {
                    time: 2,
                    interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                    z: 4.0,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            ..Default::default()
        };
        let actual = trace(&file, &[2.0, 2.0, 2.0]);
        assert_eq!(actual.root_age, 0.0);
        assert_eq!(births(&actual), [(0, 0, 0.0), (1, 0, 0.0), (3, 0, 0.0)]);
    }

    #[test]
    fn delayed_helpers_use_callback_count_and_batch_thresholds() {
        for by_one in [false, true] {
            let mut file = fixture();
            file.emitters[0].particle_items[0] = AvfxEmitterItem {
                create_count: 3,
                generate_delay: 2,
                generate_delay_by_one: by_one,
                ..item(1)
            };
            let actual = births(&trace(&file, &[10.0, 0.0, 0.0]));
            let expected = if by_one {
                vec![(1, 0, 10.0), (2, 0, 10.0), (3, 0, 10.0)]
            } else {
                vec![(1, 0, 10.0); 3]
            };
            assert_eq!(actual, expected);
            if !by_one {
                assert_eq!(births(&trace(&file, &[1.0, 1.0])), [(2, 0, 2.0); 3]);
            }
        }
    }

    #[test]
    fn helpers_occupy_capacity_before_initial_batches_and_until_retirement() {
        let mut file = fixture();
        let emitter = &mut file.emitters[0];
        emitter.child_limit = 1;
        emitter.create_interval = constant(1.0);
        emitter.particle_items.push(AvfxEmitterItem {
            generate_delay: 1,
            ..item(1)
        });
        let result = trace(&file, &[1.0, 1.0]);
        assert_eq!(births(&result), [(2, 0, 2.0)]);
        let rejected: Vec<_> = result
            .events
            .iter()
            .filter(|event| event.action == VfxTraceAction::CapacityRejected)
            .map(|event| (event.update, event.item_index, event.injection_ordinal))
            .collect();
        assert_eq!(
            rejected,
            [(0, 0, Some(0)), (1, 0, Some(1)), (1, 1, Some(0))]
        );
        assert_eq!(
            result
                .events
                .iter()
                .find(|event| event.action == VfxTraceAction::Retire)
                .unwrap()
                .update,
            2
        );
    }

    #[test]
    fn normal_update_retires_previously_dead_children_before_creating() {
        let mut file = fixture();
        file.emitters[0].child_limit = 1;
        file.emitters[0].create_interval = constant(1.0);
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };
        let result = trace(&file, &[1.0, 1.0, 1.0]);
        assert_eq!(births(&result), [(0, 0, 0.0), (2, 0, 2.0)]);
        let rejected: Vec<_> = result
            .events
            .iter()
            .filter(|event| event.action == VfxTraceAction::CapacityRejected)
            .map(|event| (event.update, event.injection_ordinal))
            .collect();
        assert_eq!(rejected, [(1, Some(1)), (3, Some(3))]);
        assert_eq!((result.retained_instances, result.live_particles), (1, 0));
    }

    #[test]
    fn dead_emitters_keep_their_capacity_slot_until_the_last_child_is_retired() {
        let mut file = fixture();
        let mut child = emitter();
        child.life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };
        child.particle_items[0].create_time = 1;
        file.emitters.push(child);
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items = vec![AvfxEmitterItem {
            target_index: 1,
            ..item(0)
        }];
        file.emitters[0].create_interval = constant(1.0);
        file.emitters[0].child_limit = 1;
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 1.0,
            ..Default::default()
        };
        let result = trace(&file, &[1.0; 4]);
        let updates: Vec<_> = result
            .events
            .iter()
            .filter(|event| {
                event.target == VfxTraceTarget::Emitter && event.action == VfxTraceAction::Create
            })
            .map(|event| event.update)
            .collect();
        assert_eq!(updates, [0, 4]);
        assert_eq!(result.live_particles, 1);
    }

    #[test]
    fn prewarm_ages_helpers_without_creating_and_does_not_inherit_parent_rate() {
        let mut file = fixture();
        let mut child = emitter();
        child.life = AvfxLife {
            enabled: true,
            value: 10.0,
            ..Default::default()
        };
        child.particle_items[0] = AvfxEmitterItem {
            create_count: 3,
            generate_delay: 2,
            generate_delay_by_one: true,
            ..item(1)
        };
        file.emitters.push(child);
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items = vec![AvfxEmitterItem {
            target_index: 1,
            start_frame: 3,
            start_frame_null_update: true,
            override_life: true,
            override_life_value: 20,
            ..item(1)
        }];
        let result = trace(&file, &[0.0, 0.0, 1.0]);
        assert_eq!(births(&result), [(1, 1, 1.5), (2, 1, 1.5), (3, 1, 2.0)]);
        assert!(
            result
                .events
                .iter()
                .filter(|event| event.phase == VfxTracePhase::NullUpdate)
                .all(|event| event.target != VfxTraceTarget::Particle)
        );
    }

    #[test]
    fn null_creation_reads_old_age_and_retires_newly_dead_children_immediately() {
        let mut file = fixture();
        let mut child = emitter();
        child.create_interval = constant(1.0);
        child.child_limit = 1;
        file.emitters.push(child);
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items = vec![AvfxEmitterItem {
            target_index: 1,
            start_frame: 3,
            start_frame_null_update: true,
            ..item(1)
        }];
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };
        let result = trace(&file, &[]);
        assert_eq!(births(&result), [(0, 1, 0.0), (0, 1, 1.0), (0, 1, 2.0)]);
        assert_eq!(result.live_particles, 0);
        assert_eq!(result.retained_instances, 1);
    }

    #[test]
    fn preparation_visits_newly_appended_emitters_and_their_zero_age_helpers() {
        let mut file = fixture();
        let mut child = emitter();
        child.particle_items[0] = AvfxEmitterItem {
            create_count: 2,
            generate_delay: 5,
            generate_delay_by_one: true,
            ..item(1)
        };
        file.emitters.push(child);
        file.emitters[0].particle_items.clear();
        file.emitters[0].emitter_items = vec![AvfxEmitterItem {
            target_index: 1,
            ..item(0)
        }];
        let result = trace(&file, &[2.0]);
        assert_eq!(births(&result), [(1, 1, 2.0), (1, 3, 0.0)]);
    }

    #[test]
    fn framework_fractional_frames_are_retained_across_input_updates() {
        let result = VfxRuntime::new(&fixture())
            .trace_emitter_updates(0, -1.0, &[0.001; 4])
            .unwrap();
        assert_eq!(result.update_frames, [0.0, 0.0, 0.0, 0.1]);
        assert!((result.remainder_frames - 0.02).abs() < 1e-7);
        // MULSS can round the tenths upward before truncation. DIVSS then
        // exceeds the input by one ULP, leaving a valid negative remainder.
        let result = VfxRuntime::new(&fixture())
            .trace_emitter_updates(0, -1.0, &[0.13, 0.0])
            .unwrap();
        assert_eq!(result.update_frames, [3.9, 0.0]);
        assert_eq!(result.remainder_frames, -f32::EPSILON * 2.0);
        let result = VfxRuntime::new(&fixture())
            .trace_emitter_updates(0, -1.0, &[60_731.3])
            .unwrap();
        assert_eq!(result.update_frames, [1_821_938.875]);
    }

    #[test]
    fn signed_injection_counter_wraps_even_when_capacity_rejects_creation() {
        let mut file = fixture();
        file.emitters[0].child_limit = 1;
        file.emitters[0].particle_items[0] = AvfxEmitterItem {
            create_count: 32_769,
            ..item(1)
        };
        let result = trace(&file, &[]);
        assert_eq!(
            result.events.last().unwrap().injection_ordinal,
            Some(i16::MIN)
        );
        assert_eq!(result.live_particles, 1);
    }

    #[test]
    fn probability_draws_are_repeatable_and_counter_starts_after_acceptance() {
        let mut file = fixture();
        file.emitters[0].particle_items[0] = AvfxEmitterItem {
            create_count: 100,
            create_probability: 50,
            ..item(1)
        };
        let first = trace(&file, &[]);
        let second = trace(&file, &[]);
        let event_shape = |trace: &VfxCreationTrace| {
            trace
                .events
                .iter()
                .map(|event| {
                    (
                        event.action,
                        event.target,
                        event.item_index,
                        event.injection_ordinal,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(event_shape(&first), event_shape(&second));
        let created: Vec<_> = first
            .events
            .iter()
            .filter(|event| {
                event.action == VfxTraceAction::Create && event.target == VfxTraceTarget::Particle
            })
            .collect();
        assert!((1..100).contains(&created.len()));
        assert_eq!(
            created.first().and_then(|event| event.injection_ordinal),
            Some(0)
        );
        assert_eq!(
            created.last().and_then(|event| event.injection_ordinal),
            Some((created.len() - 1) as i16)
        );
    }

    #[test]
    fn loops_are_applied_before_the_strict_lifetime_check() {
        let mut file = fixture();
        file.emitters[0].loop_end = 2;
        file.emitters[0].create_interval = constant(1.0);
        let runtime = VfxRuntime::new(&file);
        assert_eq!(
            births(
                &runtime
                    .trace_emitter_updates(0, 1.0, &[3.0 / 30.0])
                    .unwrap()
            ),
            [(0, 0, 0.0), (1, 0, 1.0)]
        );
        assert_eq!(
            births(
                &runtime
                    .trace_emitter_updates(0, 1.0, &[1.5 / 30.0])
                    .unwrap()
            ),
            [(0, 0, 0.0)]
        );
    }

    #[test]
    fn finish_creation_reads_the_previous_age_and_advances_new_children() {
        let mut file = fixture();
        file.emitters[0].particle_items[0] = item(2);
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };
        let result = VfxRuntime::new(&file)
            .trace_emitter_updates(0, 10.0, &[9.0 / 30.0, 2.0 / 30.0])
            .unwrap();
        assert_eq!(births(&result), [(2, 0, 9.0)]);
        assert_eq!((result.root_age, result.root_finished), (10.0, true));
        assert_eq!((result.live_particles, result.retained_instances), (0, 1));
        let creation = result
            .events
            .iter()
            .find(|event| event.action == VfxTraceAction::Create)
            .unwrap();
        assert_eq!(creation.phase, VfxTracePhase::Advance);
        file.emitters[0].particle_items[0].generate_delay = 2;
        file.emitters[0].particle_items[0].create_count = 3;
        assert!(
            births(
                &VfxRuntime::new(&file)
                    .trace_emitter_updates(0, 10.0, &[1.0])
                    .unwrap()
            )
            .is_empty()
        );
    }

    #[test]
    #[ignore = "CPU: run scripts/probe-end-tree.py against the exact installed client first"]
    fn compare_original_end_tree_and_collection() {
        use serde_json::json;
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let probe: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("end-tree-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = probe["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        let mut comparisons = Vec::new();
        for case in cases {
            let count = case["appendCount"].as_i64().unwrap() as i32;
            let already_dead = case["rootInitiallyRetired"].as_bool().unwrap();
            let mut file = fixture();
            file.emitters[0].create_interval = constant(100.0);
            file.emitters.push(file.emitters[0].clone());
            file.emitters[0].emitter_items.push(AvfxEmitterItem {
                target_index: 1,
                create_count: count,
                ..item(2)
            });
            let mut state = Instances::new_with_roots(
                &file,
                &[RootInitialization {
                    color_seed: None,
                    definition: 0,
                    life: 100.0,
                    start_delay: 0.0,
                    binder: true,
                    child_age: Some(0.0),
                }],
            )
            .unwrap();
            let mut operations = OPERATION_LIMIT;
            state
                .update_with_budget_custom_prepare(&file, 3.0, &mut operations, |p| {
                    if already_dead {
                        p.roots[0].child.as_mut().unwrap().mark_dead();
                    }
                    p.end_root(0)?;
                    p.end_root(0)
                })
                .unwrap();
            fn collect(node: &Node, values: &mut Vec<(bool, f32, f32)>) {
                values.push((node.dead, node.age, node.total_age));
                for child in &node.children {
                    collect(child, values);
                }
            }
            let mut values = Vec::new();
            collect(state.roots[0].child.as_ref().unwrap(), &mut values);
            let expected = case["nodes"].as_array().unwrap();
            assert_eq!(values.len(), expected.len());
            for ((dead, age, total), original) in values.iter().zip(expected) {
                assert_eq!(*dead, original["flags"].as_u64().unwrap() & 0x40000 != 0);
                assert_eq!(
                    age.to_bits(),
                    (original["age"].as_f64().unwrap() as f32).to_bits()
                );
                assert_eq!(
                    total.to_bits(),
                    (original["totalAge"].as_f64().unwrap() as f32).to_bits()
                );
            }
            assert_eq!(
                state.retained as u64,
                case["retainedAfterEnd"].as_u64().unwrap()
            );
            assert_eq!(state.next_instance, state.retained as u64);
            let mut retained = Vec::new();
            for original in case["retainedAfterTime"].as_array().unwrap() {
                state
                    .update_with_budget_custom_prepare(&file, 0.0, &mut operations, |_| Ok(()))
                    .unwrap();
                assert_eq!(state.retained as u64, original.as_u64().unwrap());
                retained.push(state.retained);
            }
            comparisons.push(json!({"initiallyRetired":already_dead,"appendCount":count,
                "states":values,"retainedAfterTime":retained,"differences":0}));
        }
        std::fs::write(directory.join("end-tree-comparison.json"), serde_json::to_vec_pretty(
            &json!({"cases":comparisons,"differences":0,
                "scope":"Six original END tree/Time fixtures versus actual production RootPreparation, immediate emitter/particle creation and deferred collection. Original emitter +10/constructor/destructor effects controlled, ages/retired state/retained counts compared. Original ordinary rejection is characterized separately. No GPU, live host or rejected-factory constructor/TLS side effects."}),
        ).unwrap()).unwrap();
    }

    #[test]
    fn end_recurses_into_callback_births_once_and_defers_nested_collection() {
        let mut file = fixture();
        file.emitters[0].create_interval = constant(100.0);
        let mut child = file.emitters[0].clone();
        child.emitter_items.clear();
        file.emitters.push(child);
        file.emitters[0].emitter_items.push(AvfxEmitterItem {
            target_index: 1,
            ..item(2)
        });
        let mut state = Instances::new_with_roots(
            &file,
            &[RootInitialization {
                color_seed: None,
                definition: 0,
                life: 100.0,
                start_delay: 0.0,
                binder: true,
                child_age: Some(0.0),
            }],
        )
        .unwrap();
        let mut operations = OPERATION_LIMIT;
        state
            .update_with_budget_custom_prepare(&file, 3.0, &mut operations, |p| {
                p.end_root(0)?;
                // Repeated END still traverses, without rerunning +10 creation.
                p.end_root(0)
            })
            .unwrap();
        let root = state.roots[0].child.as_ref().unwrap();
        assert_eq!(root.age, 3.0);
        assert!(root.dead);
        assert_eq!(root.children.len(), 2);
        assert!(root.children.iter().all(|child| child.dead));
        assert_eq!(root.children[0].age, 3.0);
        let new_emitter = &root.children[1];
        assert_eq!(new_emitter.age, 0.0);
        assert_eq!(new_emitter.children.len(), 1);
        assert!(new_emitter.children[0].dead);
        assert_eq!(new_emitter.children[0].age, 0.0);
        assert_eq!((state.retained, state.next_instance), (4, 4));
        for expected in [2, 1, 0] {
            state
                .update_with_budget_custom_prepare(&file, 0.0, &mut operations, |_| Ok(()))
                .unwrap();
            assert_eq!(state.retained, expected);
        }
    }

    #[test]
    fn retired_emitter_does_not_retain_unlinked_termination_helpers() {
        for kind in 0..2 {
            for by_one in [false, true] {
                let mut file = fixture();
                file.emitters.push(file.emitters[0].clone());
                file.emitters[1].particle_items.clear();
                let delayed = AvfxEmitterItem {
                    target_index: if kind == 0 { 0 } else { 1 },
                    create_time: 2,
                    create_count: 3,
                    generate_delay: 2,
                    generate_delay_by_one: by_one,
                    ..item(2)
                };
                file.emitters[0].particle_items.clear();
                if kind == 0 {
                    file.emitters[0].particle_items.push(delayed);
                } else {
                    file.emitters[0].emitter_items.push(delayed);
                }
                let result = VfxRuntime::new(&file)
                    .trace_emitter_updates(0, 2.0, &[3.0 / 30.0, 2.0 / 30.0, 2.0 / 30.0])
                    .unwrap();
                assert!(result.root_finished);
                assert_eq!(
                    result.retained_instances, 0,
                    "retired parent rejects helper"
                );
                assert_eq!(result.live_particles, 0);
                assert!(births(&result).is_empty());
            }
        }
    }

    #[test]
    fn delayed_finish_attachment_is_rejected_before_any_helper_time() {
        let mut file = fixture();
        file.emitters[0].particle_items[0] = AvfxEmitterItem {
            create_time: 2,
            create_count: 3,
            generate_delay: 2,
            generate_delay_by_one: false,
            ..item(2)
        };
        let result = VfxRuntime::new(&file)
            .trace_emitter_updates(0, 2.0, &[3.0 / 30.0, 2.0 / 30.0, 2.0 / 30.0])
            .unwrap();
        assert!(births(&result).is_empty());
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| event.target == VfxTraceTarget::Helper)
                .map(|event| (event.action, event.update))
                .collect::<Vec<_>>(),
            [
                (VfxTraceAction::Create, 1),
                (VfxTraceAction::AttachmentRejected, 1),
            ]
        );
    }

    #[test]
    fn delayed_finish_by_one_also_rejects_retired_parent_attachment() {
        let mut file = fixture();
        file.emitters[0].particle_items[0] = AvfxEmitterItem {
            create_time: 2,
            create_count: 3,
            generate_delay: 2,
            generate_delay_by_one: true,
            ..item(2)
        };
        let result = VfxRuntime::new(&file)
            .trace_emitter_updates(0, 2.0, &[3.0 / 30.0; 4])
            .unwrap();
        assert!(births(&result).is_empty());
        assert_eq!(result.retained_instances, 0);
    }

    #[test]
    fn particle_prewarm_reports_death_before_later_retirement() {
        let mut file = fixture();
        file.emitters[0].particle_items[0] = AvfxEmitterItem {
            start_frame: 1,
            start_frame_null_update: true,
            ..item(1)
        };
        file.particles[0].life = AvfxLife {
            enabled: true,
            value: 0.0,
            ..Default::default()
        };
        let result = trace(&file, &[0.0]);
        assert_eq!(
            result
                .events
                .iter()
                .map(|event| (event.action, event.phase))
                .collect::<Vec<_>>(),
            [
                (VfxTraceAction::Create, VfxTracePhase::Initialize),
                (VfxTraceAction::Finish, VfxTracePhase::NullUpdate),
                (VfxTraceAction::Retire, VfxTracePhase::Advance),
            ]
        );
    }

    #[test]
    fn unsupported_inputs_and_exhausted_budgets_never_return_successful_traces() {
        let mut file = fixture();
        let runtime = VfxRuntime::new(&file);
        for delta in [f32::NAN, f32::INFINITY, -0.1, 10_000_000.0] {
            assert!(runtime.trace_emitter_updates(0, -1.0, &[delta]).is_err());
        }
        assert!(runtime.trace_emitter_updates(100, -1.0, &[]).is_err());
        file.particles[0].collision_type = 0;
        assert!(
            VfxRuntime::new(&file)
                .trace_emitter_updates(0, -1.0, &[])
                .unwrap_err()
                .contains("collisions")
        );
        file.particles[0].collision_type = -1;
        file.particles[0].particle_type = Some(ParticleType::ModelSkin);
        assert!(
            VfxRuntime::new(&file)
                .trace_emitter_updates(0, -1.0, &[])
                .unwrap_err()
                .contains("ModelSkin requires a compatible model surface target")
        );
        file.particles[0].particle_type = Some(ParticleType::Quad);
        file.emitters[0].particle_items[0].parameter_link = 0;
        assert!(
            VfxRuntime::new(&file)
                .trace_emitter_updates(0, -1.0, &[])
                .unwrap_err()
                .contains("PrLk")
        );
        file.emitters[0].particle_items[0].parameter_link = -1;
        file.emitters[0].create_count_random = constant(1.0);
        assert!(
            VfxRuntime::new(&file)
                .trace_emitter_updates(0, -1.0, &[])
                .unwrap_err()
                .contains("CrCR")
        );
        file.emitters[0].create_count_random = constant(0.0);
        file.emitters[0].loop_end = 1;
        assert!(
            VfxRuntime::new(&file)
                .trace_emitter_updates(0, -1.0, &[100_000.0])
                .unwrap_err()
                .contains("operation limit")
        );
    }
}

#[cfg(test)]
#[path = "color_phases_tests.rs"]
mod color_phases_tests;

#[cfg(test)]
#[path = "model_skin_phases_tests.rs"]
pub(super) mod model_skin_phases_tests;
