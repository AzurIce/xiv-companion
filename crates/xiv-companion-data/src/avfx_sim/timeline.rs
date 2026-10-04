use crate::avfx::{AvfxTimeline, AvfxTimelineItem};

use super::{VfxBinderBirthClock, VfxCommonNumericState};

/// Arguments passed by Timeline dispatch to an Item factory. The caller owns
/// the created object, its parent pointer and the subsequent Prepare traversal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxTimelineBirth {
    pub item_index: usize,
    pub age: f32,
    pub life: f32,
}

/// Locals retained across Item factory calls by 0x1403bce60. The caller must
/// finish one factory/attachment callback before asking for the next Item.
#[derive(Debug)]
pub(super) struct VfxTimelineDispatchCursor {
    next_index: usize,
    item_count: u8,
    age: f32,
}

/// Timeline's scalar Common constructor, +68 clock and +70 Item dispatch.
/// This is the scheduling core for a single shared Timeline instance; it does
/// not flatten its parent Binder or create emitter objects. Children and their
/// Time/Prepare/destruction passes remain the caller's responsibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxTimelineLifecycle {
    pub clock: VfxBinderBirthClock,
    pub scaled_delta: f32,
    pub numeric: VfxCommonNumericState,
    pub loop_start: f32,
    pub loop_end: f32,
    flags: u32,
    consumed: u32,
    dispatch_active: bool,
    life_limit_enabled: bool,
    item_count: u8,
    loop_generation: u64,
}

impl VfxTimelineLifecycle {
    /// Scheduler factory 0x1403bbf10 derives the child's supplied life from
    /// compiled loop bounds and every Item, including disabled/infinite ones.
    pub fn scheduler_child_life(timeline: &AvfxTimeline) -> f32 {
        let initial = ((timeline.loop_end as u32 & 0xff_ffff) + 1).max(1) as i32;
        timeline.items.iter().fold(initial, |life, item| {
            life.max(i32::from(item.start_time as i16) + 1)
                .max(i32::from(item.end_time as i16))
        }) as f32
    }

    /// The compiled definition packs loop bounds as unsigned 24-bit values and
    /// Item count as a byte. A type-2 Scheduler ancestor can disable age looping.
    pub fn new(
        timeline: &AvfxTimeline,
        input_age: f32,
        life: f32,
        parent_disables_loop: bool,
    ) -> Result<Self, &'static str> {
        if !input_age.is_finite() || !life.is_finite() {
            return Err("Timeline construction requires finite clocks");
        }
        let item_count = u8::try_from(timeline.items.len())
            .map_err(|_| "Timeline Item count exceeds the compiled byte")?;
        let loop_start = (timeline.loop_start as u32 & 0xff_ffff) as f32;
        let loop_end = (timeline.loop_end as u32 & 0xff_ffff) as f32;
        let loops = loop_end > 0.0 && loop_start < loop_end && !parent_disables_loop;
        let age = input_age * 1.0;
        Ok(Self {
            clock: VfxBinderBirthClock {
                local_age: age,
                total_age: age,
                previous_age: age,
                nominal_life: life,
                rate: 1.0,
                delay: 0.0,
            },
            scaled_delta: 0.0,
            numeric: VfxCommonNumericState::default(),
            loop_start,
            loop_end,
            flags: 0x3f00_0000 | if loops { 0x8040_0000 } else { 0 } | u32::from(item_count != 0),
            consumed: initial_consumed(item_count),
            dispatch_active: item_count != 0,
            life_limit_enabled: item_count == 0,
            item_count,
            loop_generation: 0,
        })
    }

    pub fn raw_flags(self) -> u32 {
        self.flags
    }

    /// Number of actual reset callbacks, including multiple wraps in one Time.
    pub fn loop_generation(self) -> u64 {
        self.loop_generation
    }

    pub fn consumed_mask(self) -> u32 {
        self.consumed
    }

    pub fn dispatch_active(self) -> bool {
        self.dispatch_active
    }

    pub fn life_limit_enabled(self) -> bool {
        self.life_limit_enabled
    }

    /// Returns whether the owner must visit its currently registered children.
    pub fn unlock_loop_point(&mut self) -> bool {
        super::unlock_loop_flags(&mut self.flags)
    }

    pub fn retired(self) -> bool {
        self.flags & 0x40000 != 0
    }

    /// Common END retires self once without changing its clock, consumption,
    /// registered children or separate Timeline dispatch callback.
    pub fn retire(&mut self) {
        if !self.retired() {
            self.flags = (self.flags & 0xc0ff_ffff) | 0x40000;
        }
    }

    pub fn configure_fade(&mut self, duration: i32, mode: u32, flag: bool) {
        self.numeric
            .configure_fade(&mut self.flags, duration, mode, flag);
    }

    /// The caller owns the attachment/+08 sequence and descendant traversal.
    pub fn inherit_fade_after_attach(&mut self, parent_flags: u32, parent: VfxCommonNumericState) {
        self.numeric
            .inherit_fade_after_attach(&mut self.flags, parent_flags, parent);
    }

    /// Timeline +78 has a no-op +100 and a no-op +10. It operates on cached
    /// alpha, independently from Time and Prepare/Item dispatch.
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

    pub fn set_registered_children(&mut self, count: u16) {
        self.flags = (self.flags & !0x3fe00) | ((u32::from(count) & 0x1ff) << 9);
    }

    pub fn set_time_advance_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x100_0000) | (u32::from(enabled) << 24);
    }

    pub fn set_prepare_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x1200_0000) | if enabled { 0x1200_0000 } else { 0 };
    }

    pub fn set_numeric_self_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x0400_0000) | (u32::from(enabled) << 26);
    }

    pub fn set_numeric_base_refresh_enabled(&mut self, enabled: bool) {
        self.flags = (self.flags & !0x2000_0000) | (u32::from(enabled) << 29);
    }

    /// +68 updates the previous clock even with self-Time disabled. Loop reset
    /// precedes the life check and restores Item consumption and creation work.
    pub fn advance_time(&mut self, delta: f32) -> Result<(), &'static str> {
        let delta = delta * self.clock.rate;
        let previous = delta + self.clock.previous_age;
        if !delta.is_finite() || !previous.is_finite() {
            return Err("nonfinite Timeline time input");
        }
        if self.flags & 0x100_0000 == 0 {
            self.clock.previous_age = previous;
            return Ok(());
        }
        let mut age = delta + self.clock.local_age;
        let total = delta + self.clock.total_age;
        if !age.is_finite() || !total.is_finite() {
            return Err("nonfinite Timeline age");
        }
        let mut resets = 0;
        if self.flags & 0x8000_0000 != 0 {
            // Native Common repeatedly subtracts the rounded period; modulo
            // gives different f32 results for a large crossing.
            while age >= self.loop_end {
                let next = age - (self.loop_end - self.loop_start);
                if next == age || resets == 16_384 {
                    return Err("Timeline loop work limit");
                }
                age = next;
                resets += 1;
            }
        }
        if resets != 0 {
            self.loop_generation = self
                .loop_generation
                .checked_add(resets)
                .ok_or("Timeline loop generation limit")?;
            self.reset_items();
        }
        self.clock.previous_age = previous;
        self.scaled_delta = delta;
        if self.life_limit_enabled
            && self.clock.nominal_life >= 0.0
            && age > self.clock.nominal_life
        {
            age = self.clock.nominal_life;
            if !self.retired() {
                self.flags = (self.flags & 0xc0ff_ffff) | 0x40000;
            }
        }
        self.clock.local_age = age;
        self.clock.total_age = total;
        Ok(())
    }

    fn reset_items(&mut self) {
        self.consumed = initial_consumed(self.item_count);
        self.dispatch_active = true;
        self.flags = (self.flags & 0xffff_fe01) | 1;
        self.life_limit_enabled = false;
    }

    /// Timeline +70 checks mode 3 and captures age/count once at entry, even
    /// when ordinary Prepare is disabled. Later factory callbacks may change
    /// those fields without restarting or cancelling this invocation.
    pub(super) fn begin_dispatch(&self) -> Option<VfxTimelineDispatchCursor> {
        if !self.dispatch_active || self.flags & 0x180000 == 0x180000 {
            return None;
        }
        Some(VfxTimelineDispatchCursor {
            next_index: 0,
            item_count: self.item_count,
            age: self.clock.local_age,
        })
    }

    /// Consume only up to the next enabled due Item. Native dispatch rereads
    /// the consumption mask after each factory; a loop-reset callback can
    /// rearm earlier bits, but the index and captured age still move forward.
    /// Consumption and life/dispatch gates change before the returned factory.
    pub(super) fn next_due_item(
        &mut self,
        timeline: &AvfxTimeline,
        cursor: &mut VfxTimelineDispatchCursor,
    ) -> Option<VfxTimelineBirth> {
        while cursor.next_index < usize::from(cursor.item_count) {
            let index = cursor.next_index;
            cursor.next_index += 1;
            let bit = 1u32.rotate_left(index as u32);
            if self.consumed & bit != 0 {
                continue;
            }
            // A synchronous REST can replace both object and definition while
            // this cursor retains the old count. The native dispatcher checks
            // the new mask before loading the new definition's Item pointer.
            let item = timeline.items.get(index)?;
            let age = cursor.age - (item.start_time as i16) as f32;
            if age < 0.0 {
                continue;
            }
            self.consumed |= bit;
            if self.consumed == u32::MAX {
                self.flags &= 0xffff_fe00;
                self.life_limit_enabled = true;
                self.dispatch_active = false;
            }
            if item.enabled {
                return Some(VfxTimelineBirth {
                    item_index: index,
                    age,
                    life: item_life(item),
                });
            }
        }
        None
    }

    /// Collect due Items when no factory callback needs to observe or mutate
    /// this Timeline between births. Owned playback uses the cursor directly.
    pub fn prepare_due_items(&mut self, timeline: &AvfxTimeline) -> Vec<VfxTimelineBirth> {
        let mut births = Vec::new();
        if let Some(mut cursor) = self.begin_dispatch() {
            while let Some(birth) = self.next_due_item(timeline, &mut cursor) {
                births.push(birth);
            }
        }
        births
    }
}

fn initial_consumed(count: u8) -> u32 {
    (0..u32::from(count)).fold(u32::MAX, |mask, index| mask & !1u32.rotate_left(index))
}

fn item_life(item: &AvfxTimelineItem) -> f32 {
    if item.end_time < 0 {
        -1.0
    } else {
        (i32::from(item.end_time as i16) - i32::from(item.start_time as i16)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> AvfxTimeline {
        AvfxTimeline {
            binder_index: -1,
            items: vec![
                AvfxTimelineItem {
                    enabled: true,
                    start_time: 2,
                    end_time: 12,
                    binder_index: -1,
                    emitter_index: 0,
                    effector_index: -1,
                    clip_index: -1,
                    platform: 0,
                },
                AvfxTimelineItem {
                    enabled: false,
                    start_time: 4,
                    end_time: -1,
                    binder_index: -1,
                    emitter_index: 0,
                    effector_index: -1,
                    clip_index: -1,
                    platform: 0,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn item_factory_observes_only_consumption_through_its_own_item() {
        let mut definition = timeline();
        definition.items[1].enabled = true;
        let mut instance = VfxTimelineLifecycle::new(&definition, 5.0, 100.0, false).unwrap();
        let mut cursor = instance.begin_dispatch().unwrap();
        assert_eq!(
            instance
                .next_due_item(&definition, &mut cursor)
                .unwrap()
                .item_index,
            0
        );
        assert_eq!(instance.consumed_mask(), u32::MAX & !2);
        assert!(instance.dispatch_active());
        assert!(!instance.life_limit_enabled());
        assert_eq!(instance.raw_flags() & 0x1ff, 1);
        assert_eq!(
            instance
                .next_due_item(&definition, &mut cursor)
                .unwrap()
                .item_index,
            1
        );
        assert_eq!(instance.consumed_mask(), u32::MAX);
        assert!(!instance.dispatch_active());
        assert!(instance.life_limit_enabled());
        assert_eq!(instance.raw_flags() & 0x1ff, 0);
        assert!(instance.next_due_item(&definition, &mut cursor).is_none());
    }

    #[test]
    fn synchronous_reconstruction_keeps_cursor_locals_and_replaces_live_state() {
        let mut definition = timeline();
        definition.items[0].start_time = 0;
        definition.items[1].enabled = true;
        definition.items[1].start_time = 2;
        definition.items[1].end_time = 30;
        let mut instance = VfxTimelineLifecycle::new(&definition, 20.0, 31.0, false).unwrap();
        instance.set_registered_children(2);
        let mut cursor = instance.begin_dispatch().unwrap();
        assert_eq!(
            instance
                .next_due_item(&definition, &mut cursor)
                .unwrap()
                .item_index,
            0
        );

        instance = VfxTimelineLifecycle::new(&definition, 0.0, 31.0, false).unwrap();
        let next = instance.next_due_item(&definition, &mut cursor).unwrap();
        assert_eq!(next.item_index, 1);
        assert_eq!(
            next.age, 18.0,
            "the active invocation still has the old age"
        );
        assert_eq!(instance.clock.local_age, 0.0);
        assert_eq!(instance.registered_children(), 0);
        assert_eq!(instance.consumed_mask(), u32::MAX & !1);
        assert!(
            instance.dispatch_active(),
            "the reconstructed first Item is pending"
        );
        assert!(instance.next_due_item(&definition, &mut cursor).is_none());
        assert_eq!(instance.prepare_due_items(&definition)[0].item_index, 0);
    }

    #[test]
    #[ignore = "CPU: requires scripts/probe-item-rest-lifetime.py exact-client observations"]
    fn compare_original_item_rest_reconstructed_timeline_dispatch() {
        use serde_json::json;
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(output.join("item-rest-lifetime-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 1920);
        let definition = |count: usize, start| AvfxTimeline {
            items: (0..count)
                .map(|i| AvfxTimelineItem {
                    enabled: i % 3 != 2,
                    start_time: start,
                    end_time: 30,
                    binder_index: -1,
                    emitter_index: -1,
                    effector_index: 0,
                    clip_index: -1,
                    platform: 0,
                })
                .collect(),
            binder_index: -1,
            ..Default::default()
        };
        let state = |instance: &VfxTimelineLifecycle, source| {
            json!({
                "age": instance.clock.local_age.to_bits(), "flags": instance.raw_flags(),
                "consumed": instance.consumed_mask(), "definition": source,
                "active": instance.dispatch_active(), "lifeEnabled": instance.life_limit_enabled(),
                "timelineVtable": true,
            })
        };
        let mut observations = 0;
        for (ordinal, case) in cases.iter().enumerate() {
            let old = definition(case["oldCount"].as_u64().unwrap() as usize, 0);
            let new = definition(
                case["newCount"].as_u64().unwrap() as usize,
                case["newStart"].as_i64().unwrap() as i32,
            );
            let mode = case["mode"].as_u64().unwrap();
            let age = f32::from_bits(case["inputAge"].as_u64().unwrap() as u32);
            let mut instance = VfxTimelineLifecycle::new(&old, age, 31.0, false).unwrap();
            instance.set_registered_children(case["childCount"].as_u64().unwrap() as u16);
            if mode != 0 {
                instance.flags |= 0x100000;
            }
            assert_eq!(
                state(&instance, 0),
                case["initial"],
                "case {ordinal} initial"
            );
            let mut cursor = instance.begin_dispatch().unwrap();
            assert_eq!(
                instance
                    .next_due_item(&old, &mut cursor)
                    .unwrap()
                    .item_index,
                0
            );
            // Original REST frees/reallocates/constructs the same storage. This
            // comparison replaces only the scalar lifecycle; it does not model
            // pool callbacks, ownership or downstream Effector construction.
            for _ in 0..case["repeats"].as_u64().unwrap() {
                instance = VfxTimelineLifecycle::new(
                    &new,
                    0.0,
                    VfxTimelineLifecycle::scheduler_child_life(&new),
                    false,
                )
                .unwrap();
                if mode != 0 {
                    instance.flags |= 0x100000;
                }
            }
            // The native fixture explicitly pads its allocation to 33 Items
            // with disabled records. Match that backing buffer for this check;
            // it is not a policy for undefined reads in production files.
            let mut backing = new.clone();
            while backing.items.len() < 33 {
                backing.items.push(AvfxTimelineItem {
                    enabled: false,
                    start_time: case["newStart"].as_i64().unwrap() as i32,
                    end_time: 30,
                    binder_index: -1,
                    emitter_index: -1,
                    effector_index: 0,
                    clip_index: -1,
                    platform: 0,
                });
            }
            let mut observed = Vec::new();
            while let Some(birth) = instance.next_due_item(&backing, &mut cursor) {
                observed.push(json!({"kind":"effector", "source":1, "item":birth.item_index,
                    "age":birth.age.to_bits(), "life":birth.life.to_bits(), "state":state(&instance, 1)}));
            }
            let expected = case["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == "effector")
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(observed, expected, "case {ordinal} factory observations");
            assert_eq!(state(&instance, 1), case["final"], "case {ordinal} final");
            observations += observed.len();
        }
        std::fs::write(output.join("item-rest-lifetime-rust-comparison.json"),
            serde_json::to_string_pretty(&json!({"cases":cases.len(),
                "effectorObservations":observations, "differences":0,
                "productionTimelineScalarCoreCompared":true, "productionOwnedPlaybackCompared":false,
                "scope":"Reconstructed scalar clock/flags/mask and continued Item cursor against original. Same explicit disabled padding as fixture; pool/owned children/Scheduler traversal not compared."})).unwrap()).unwrap();
    }

    #[test]
    fn active_dispatch_retains_entry_age_and_count_after_factory_callbacks() {
        let mut definition = timeline();
        definition.items[1].enabled = true;
        let mut instance = VfxTimelineLifecycle::new(&definition, 5.0, 100.0, false).unwrap();
        let mut cursor = instance.begin_dispatch().unwrap();
        assert_eq!(
            instance
                .next_due_item(&definition, &mut cursor)
                .unwrap()
                .age,
            3.0
        );
        instance.configure_fade(4, 3, false);
        instance.clock.local_age = 40.0;
        instance.reset_items();
        instance.item_count = 0;
        let next = instance.next_due_item(&definition, &mut cursor).unwrap();
        assert_eq!(next.item_index, 1);
        assert_eq!(next.age, 1.0);
        assert_eq!(instance.consumed_mask(), u32::MAX & !1);
        assert!(!instance.life_limit_enabled());
        assert!(instance.next_due_item(&definition, &mut cursor).is_none());
        assert!(
            instance.begin_dispatch().is_none(),
            "mode 3 gates the next invocation"
        );
    }

    #[test]
    fn factory_mutations_of_consumption_affect_later_item_checks() {
        let mut definition = timeline();
        definition.items[1].enabled = true;
        let mut instance = VfxTimelineLifecycle::new(&definition, 5.0, 100.0, false).unwrap();
        let mut cursor = instance.begin_dispatch().unwrap();
        assert!(instance.next_due_item(&definition, &mut cursor).is_some());
        instance.consumed = u32::MAX;
        assert!(instance.next_due_item(&definition, &mut cursor).is_none());
        assert!(
            instance.dispatch_active(),
            "only native consume transition closes dispatch"
        );
        assert!(!instance.life_limit_enabled());
    }

    #[test]
    #[ignore = "CPU: requires scripts/probe-timeline-dispatch.py output from exact installed client"]
    fn compare_original_timeline_dispatch_factory_interleaving() {
        use serde_json::{Value, json};
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = repo.join("target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(output.join("timeline-dispatch-client-probe.json")).unwrap(),
        )
        .unwrap();
        let state = |instance: &VfxTimelineLifecycle| {
            json!({
                "age": instance.clock.local_age.to_bits(),
                "flags": instance.raw_flags(), "consumed": instance.consumed_mask(),
                "lifeEnabled": instance.life_limit_enabled(), "active": instance.dispatch_active(),
            })
        };
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 420);
        let mut events_checked = 0;
        for (case_index, case) in cases.iter().enumerate() {
            let count = case["count"].as_u64().unwrap() as usize;
            let mutation = case["mutation"].as_u64().unwrap();
            let mode = case["mode"].as_u64().unwrap();
            let definition = AvfxTimeline {
                items: (0..count)
                    .map(|i| AvfxTimelineItem {
                        enabled: i % 4 != 1,
                        start_time: (i % 5) as i32,
                        end_time: if i % 3 == 2 { -1 } else { (i % 5 + 10) as i32 },
                        binder_index: -1,
                        emitter_index: if i % 3 == 1 { 0 } else { -1 },
                        effector_index: if i % 3 == 0 { 0 } else { -1 },
                        clip_index: if i % 3 == 2 { 0 } else { -1 },
                        platform: 0,
                    })
                    .collect(),
                ..Default::default()
            };
            let mut instance = VfxTimelineLifecycle::new(
                &definition,
                f32::from_bits(case["inputAge"].as_u64().unwrap() as u32),
                100.0,
                false,
            )
            .unwrap();
            instance.set_registered_children(2);
            if mode != 0 {
                instance.flags |= if mode == 1 { 0x100000 } else { 0x180000 };
            }
            let mut events = Vec::new();
            for pass in 0..2 {
                if let Some(mut cursor) = instance.begin_dispatch() {
                    while let Some(birth) = instance.next_due_item(&definition, &mut cursor) {
                        events.push(json!({
                            "item": birth.item_index, "kind": birth.item_index % 3, "pass": pass,
                            "age": birth.age.to_bits(), "life": birth.life.to_bits(), "state": state(&instance),
                        }));
                        if events.len() == 1 {
                            if mutation == 1 || mutation == 4 {
                                instance.flags |= 0x180000;
                            }
                            if matches!(mutation, 2 | 4 | 6) {
                                instance.clock.local_age = 40.0;
                            }
                            if mutation == 3 || mutation == 4 {
                                instance.reset_items();
                            }
                            if mutation == 5 {
                                instance.consumed = u32::MAX;
                            }
                            if mutation == 6 {
                                instance.item_count = 0;
                            }
                        }
                    }
                }
            }
            assert_eq!(
                json!(events),
                case["events"],
                "factories in case {case_index}"
            );
            assert_eq!(
                state(&instance),
                case["final"],
                "final in case {case_index}"
            );
            events_checked += events.len();
        }
        std::fs::write(output.join("timeline-dispatch-rust-comparison.json"), serde_json::to_string_pretty(&json!({
            "cases": cases.len(), "invocations": cases.len() * 2, "factoryEvents": events_checked,
            "differences": 0,
            "scope": "Original 0x1403bce60 dispatch, loop reset and dispatch noop; controlled scalar initialization and effector/emitter/clip factory callbacks. Full flags, consumed mask, life/dispatch gates and clock observed before each factory, and final state, compared exactly. Counts 0/1/2/4/33, disabled/infinite/future Items, entry modes 0/2/3, signed zero and mutations of mode, clock, loop reset, consumption and definition count. Production uses this cursor between actual factories; full factory, REST manager/destruction, Binder, RNG, host, GPU and client pixels excluded."
        })).unwrap()).unwrap();
    }

    #[test]
    fn fade_mode_controls_item_dispatch_independently_from_numeric_retirement() {
        for mode in [2, 3] {
            let definition = timeline();
            let mut instance = VfxTimelineLifecycle::new(&definition, 3.0, 100.0, false).unwrap();
            instance.configure_fade(4, mode, false);
            assert_eq!(
                instance.prepare_due_items(&definition).len(),
                usize::from(mode == 2)
            );
            instance.advance_time(2.0).unwrap();
            assert_eq!(instance.numeric.fade.age, 0.0, "Time is not the fade pass");
            instance.refresh_numeric();
            assert_eq!(instance.numeric.alpha, 0.5);
            instance.advance_time(2.0).unwrap();
            instance.refresh_numeric();
            assert_eq!(instance.numeric.alpha, 0.0);
            assert!(!instance.retired(), "equality is not completion");
            instance.advance_time(0.25).unwrap();
            instance.refresh_numeric();
            assert!(instance.retired());
        }
    }

    #[test]
    fn end_preserves_dispatch_and_clocks_and_is_idempotent() {
        let definition = timeline();
        let mut instance = VfxTimelineLifecycle::new(&definition, 3.0, 10.0, false).unwrap();
        instance.set_registered_children(2);
        let consumed = instance.consumed_mask();
        instance.retire();
        assert!(instance.retired());
        assert_eq!(instance.clock.local_age, 3.0);
        assert_eq!(instance.registered_children(), 2);
        assert_eq!(instance.consumed_mask(), consumed);
        instance.advance_time(2.0).unwrap();
        assert_eq!(instance.clock.local_age, 3.0);
        assert_eq!(instance.clock.previous_age, 5.0);
        assert_eq!(instance.prepare_due_items(&definition).len(), 1);
        // Already retired Common END leaves independently reenabled gates.
        instance.set_prepare_enabled(true);
        let flags = instance.raw_flags();
        instance.retire();
        assert_eq!(instance.raw_flags(), flags);
    }

    #[test]
    fn timeline_pending_items_defer_life_limit_and_dispatch_without_prepare() {
        let definition = timeline();
        let mut instance = VfxTimelineLifecycle::new(&definition, 0.0, 0.0, false).unwrap();
        instance.set_prepare_enabled(false);
        instance.advance_time(3.0).unwrap();
        assert!(!instance.retired());
        assert_eq!(
            instance.prepare_due_items(&definition),
            vec![VfxTimelineBirth {
                item_index: 0,
                age: 1.0,
                life: 10.0
            }]
        );
        instance.set_registered_children(1);
        instance.advance_time(2.0).unwrap();
        assert!(instance.prepare_due_items(&definition).is_empty());
        assert!(!instance.dispatch_active());
        assert!(instance.life_limit_enabled());
        assert!(!instance.retired());
        instance.advance_time(1.0).unwrap();
        assert!(instance.retired());
        assert_eq!(instance.clock.local_age, 0.0);
        assert_eq!(instance.clock.total_age, 6.0);
        assert_eq!(instance.registered_children(), 1);
    }

    #[test]
    fn timeline_loop_reset_rearms_items_before_life_check() {
        let mut definition = timeline();
        definition.loop_start = 2;
        definition.loop_end = 5;
        let mut instance = VfxTimelineLifecycle::new(&definition, 3.0, 0.0, false).unwrap();
        assert_eq!(instance.prepare_due_items(&definition).len(), 1);
        instance.advance_time(8.0).unwrap();
        assert_eq!(instance.clock.local_age, 2.0);
        assert_eq!(instance.clock.total_age, 11.0);
        assert!(!instance.retired());
        assert_eq!(
            instance.prepare_due_items(&definition),
            vec![VfxTimelineBirth {
                item_index: 0,
                age: 0.0,
                life: 10.0
            }]
        );
        let mut disabled = VfxTimelineLifecycle::new(&definition, 3.0, -1.0, true).unwrap();
        disabled.advance_time(8.0).unwrap();
        assert_eq!(disabled.clock.local_age, 11.0);
    }

    #[test]
    fn timeline_constructor_preserves_age_and_empty_infinite_does_not_use_binder_retirement() {
        let definition = AvfxTimeline::default();
        let mut instance = VfxTimelineLifecycle::new(&definition, 20.0, -1.0, false).unwrap();
        assert_eq!(instance.clock.local_age, 20.0);
        assert!(!instance.dispatch_active());
        instance.advance_time(1.0).unwrap();
        assert!(instance.prepare_due_items(&definition).is_empty());
        assert!(!instance.retired());
        let zero = VfxTimelineLifecycle::new(&definition, -0.0, 0.0, false).unwrap();
        assert_eq!(zero.clock.local_age.to_bits(), (-0.0f32).to_bits());
    }

    #[test]
    fn timeline_unbounded_loop_input_returns_error_without_changing_state() {
        let mut definition = timeline();
        definition.loop_end = 1;
        let mut instance = VfxTimelineLifecycle::new(&definition, 0.0, -1.0, false).unwrap();
        let before = instance;
        assert_eq!(
            instance.advance_time(f32::MAX),
            Err("Timeline loop work limit")
        );
        assert_eq!(instance, before);
        assert_eq!(
            instance.advance_time(f32::INFINITY),
            Err("nonfinite Timeline time input")
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn timeline_large_crossing_preserves_repeated_f32_subtraction() {
        let mut definition = timeline();
        definition.loop_end = 2047;
        let mut instance = VfxTimelineLifecycle::new(&definition, 0.0, -1.0, false).unwrap();
        instance.advance_time(16_787_216.0).unwrap();
        assert_eq!(instance.clock.local_age, 1_812.0);
        assert_ne!(instance.clock.local_age, 16_787_216.0f32 % 2047.0);
    }
}
