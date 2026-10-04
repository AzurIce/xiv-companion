use super::{VfxBinderBirthClock, VfxCommonNumericState};
use crate::avfx::{AvfxScheduler, AvfxSchedulerItem};

/// Scheduler's Common self clock and +228 dispatch. The owner maintains the
/// actual child chain and removes retired empty Scheduler storage before Time.
#[derive(Clone, Debug)]
pub(super) struct SchedulerOrdinaryState {
    pub(super) flags: u32,
    pub(super) consumed: u32,
    pub(super) life_limit_enabled: bool,
    pub(super) active: bool,
    pub(super) clock: VfxBinderBirthClock,
    pub(super) scaled_delta: f32,
    pub(super) numeric: VfxCommonNumericState,
    pub(super) linked: bool,
}

pub(super) struct SchedulerOrdinaryCursor {
    next: usize,
    count: usize,
    age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SchedulerOrdinaryBirth {
    pub(super) item: usize,
    pub(super) age: f32,
}

/// The reader stores ItCn as one byte and both consumers sign-extend it.
pub(super) fn ordinary_count(scheduler: &AvfxScheduler) -> usize {
    (scheduler.item_count.unwrap_or(scheduler.items.len() as i32) as u8 as i8).max(0) as usize
}

/// 0x14039d01d reads a word, shifts by two and preserves the enabled bits;
/// 0x1403bc15d arithmetically shifts that packed signed word back by two.
pub(super) fn compiled_start(item: &AvfxSchedulerItem) -> f32 {
    ((item.start_time as i16).wrapping_shl(2) >> 2) as f32
}

pub(super) fn compiled_timeline(item: &AvfxSchedulerItem) -> i32 {
    i32::from(item.timeline_index as i16)
}

impl SchedulerOrdinaryState {
    /// 0x1403bb2f0 clears only enabled mapping bits, with 32-bit rotation,
    /// but counts all enabled entries independently in Common's low nine bits.
    pub(super) fn new(scheduler: &AvfxScheduler) -> Self {
        let mut consumed = u32::MAX;
        let mut pending = 0;
        for (index, item) in scheduler
            .items
            .iter()
            .take(ordinary_count(scheduler))
            .enumerate()
        {
            if item.enabled {
                consumed &= !1u32.rotate_left(index as u32);
                pending += 1;
            }
        }
        Self {
            flags: 0x3f00_0000 | pending,
            consumed,
            life_limit_enabled: pending == 0,
            // Even an empty constructor keeps the original +228 dispatcher.
            active: true,
            clock: VfxBinderBirthClock::new(0.0, -1, 0.0),
            scaled_delta: 0.0,
            numeric: VfxCommonNumericState::default(),
            linked: true,
        }
    }

    pub(super) fn retired(&self) -> bool {
        self.flags & 0x40000 != 0
    }

    /// Scheduler +10 is a no-op: self life expiry does not END descendants.
    pub(super) fn retire(&mut self) {
        self.flags = (self.flags & 0xc0ff_ffff) | 0x40000;
    }

    pub(super) fn set_registered_children(&mut self, count: usize) {
        self.flags = (self.flags & !0x3fe00) | (((count as u32) & 511) << 9);
    }

    pub(super) fn configure_fade(&mut self, duration: i32, mode: u32, flag: bool) {
        self.numeric
            .configure_fade(&mut self.flags, duration, mode, flag);
    }

    /// Scheduler +100 and +10 are original no-ops. Preserve cached alpha;
    /// self retirement never prevents the owner from refreshing children.
    pub(super) fn refresh_numeric(&mut self) {
        if let Some(step) =
            self.numeric
                .begin_refresh(&mut self.flags, self.scaled_delta, |_, _| {})
        {
            self.numeric.finish_refresh(step);
        }
    }

    pub(super) fn advance_time(&mut self, delta: f32) -> Result<(), String> {
        let delta = delta * self.clock.rate;
        let previous = delta + self.clock.previous_age;
        if !delta.is_finite() || !previous.is_finite() {
            return Err("nonfinite Scheduler time input".into());
        }
        if self.flags & 0x1000000 != 0 {
            let mut age = delta + self.clock.local_age;
            let total = delta + self.clock.total_age;
            if !age.is_finite() || !total.is_finite() {
                return Err("nonfinite Scheduler age".into());
            }
            if self.life_limit_enabled
                && self.clock.nominal_life >= 0.0
                && age > self.clock.nominal_life
            {
                age = self.clock.nominal_life;
                self.retire();
            }
            self.scaled_delta = delta;
            self.clock.local_age = age;
            self.clock.total_age = total;
        }
        self.clock.previous_age = previous;
        Ok(())
    }

    pub(super) fn begin(
        &self,
        scheduler: &AvfxScheduler,
        age: f32,
    ) -> Option<SchedulerOrdinaryCursor> {
        (self.linked && self.active && self.flags & 0x180000 != 0x180000).then(|| {
            SchedulerOrdinaryCursor {
                next: 0,
                count: ordinary_count(scheduler),
                age,
            }
        })
    }

    /// Lock age/count at entry; reread mapping and mask after every factory.
    /// All-ones disables subsequent invocations, never the current cursor.
    pub(super) fn next_due(
        &mut self,
        scheduler: &AvfxScheduler,
        cursor: &mut SchedulerOrdinaryCursor,
    ) -> Option<SchedulerOrdinaryBirth> {
        while cursor.next < cursor.count {
            let index = cursor.next;
            cursor.next += 1;
            let bit = 1u32.rotate_left(index as u32);
            if self.consumed & bit != 0 {
                continue;
            }
            let item = scheduler.items.get(index)?;
            let age = cursor.age - compiled_start(item);
            if age < 0.0 {
                continue;
            }
            self.consumed |= bit;
            if self.consumed == u32::MAX {
                self.active = false;
            }
            if item.enabled {
                return Some(SchedulerOrdinaryBirth { item: index, age });
            }
        }
        None
    }

    /// Original root factory 0x1403bc06f does this after allocation/attachment,
    /// including failure. Disabled alias consumption never calls the factory,
    /// so an all-ones mask does not necessarily restore the life-limit gate.
    pub(super) fn finish_factory(&mut self) {
        let pending = self.flags.wrapping_sub(1) & 0x1ff;
        self.flags = (self.flags & !0x1ff) | pending;
        self.life_limit_enabled = pending == 0;
    }

    /// Selected-Item REST reserves one factory without consuming an ordinary
    /// mapping bit. The matching finish restores the low-nine-bit counter,
    /// including wrapping 511 -> 0 -> 511 and rejected Attach.
    pub(super) fn reserve_rest_factory(&mut self) {
        let pending = self.flags.wrapping_add(1) & 0x1ff;
        self.flags = (self.flags & !0x1ff) | pending;
        self.life_limit_enabled = pending == 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture(count: usize, pattern: usize) -> AvfxScheduler {
        let edges = [8192, 16383, 16384, 65536, -8193, 32767];
        AvfxScheduler {
            items: (0..count)
                .map(|i| AvfxSchedulerItem {
                    enabled: match pattern {
                        0 | 3 => true,
                        1 => i >= 32,
                        _ => i % 2 == 0,
                    },
                    start_time: match pattern {
                        0 => [0, 5, 20][i % 3],
                        1 => 0,
                        2 => [-8192, -1, 0, 8191][i % 4],
                        _ => edges[i % 6],
                    },
                    timeline_index: 0,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn snapshot(state: &SchedulerOrdinaryState, scheduler: &AvfxScheduler, age: f32) -> Value {
        json!({"flags":state.flags,"consumed":state.consumed,"lifeEnabled":state.life_limit_enabled,"active":state.active,"age":age.to_bits(),"countByte":scheduler.item_count.unwrap_or(scheduler.items.len() as i32) as u8})
    }

    #[test]
    fn disabled_alias_consumption_does_not_complete_pending_factory() {
        let scheduler = fixture(33, 1);
        let mut state = SchedulerOrdinaryState::new(&scheduler);
        let mut cursor = state.begin(&scheduler, 0.0).unwrap();
        assert_eq!(state.next_due(&scheduler, &mut cursor), None);
        assert_eq!(state.consumed, u32::MAX);
        assert!(!state.active);
        assert_eq!(state.flags & 0x1ff, 1);
        assert!(!state.life_limit_enabled);
    }

    #[test]
    fn count_uses_signed_byte_and_compiled_start_wraps_signed_fourteen_bits() {
        assert_eq!(ordinary_count(&fixture(127, 0)), 127);
        for count in [128, 255, 256] {
            assert_eq!(ordinary_count(&fixture(count, 0)), 0);
        }
        assert_eq!(ordinary_count(&fixture(257, 0)), 1);
        for (raw, expected) in [
            (8191, 8191.0),
            (8192, -8192.0),
            (16383, -1.0),
            (16384, 0.0),
            (-8193, 8191.0),
        ] {
            let mut item = fixture(1, 0).items[0];
            item.start_time = raw;
            assert_eq!(compiled_start(&item), expected);
        }
    }

    #[test]
    #[ignore = "CPU: generate probe-scheduler-ordinary.py original observations first"]
    fn compare_original_scheduler_ordinary_constructor_and_dispatch() {
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(output.join("scheduler-ordinary-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut comparisons = 0;
        for (ordinal, case) in original["cases"].as_array().unwrap().iter().enumerate() {
            let mut scheduler = fixture(
                case["count"].as_u64().unwrap() as usize,
                case["pattern"].as_u64().unwrap() as usize,
            );
            let mut state = SchedulerOrdinaryState::new(&scheduler);
            state.flags |= (case["mode"].as_u64().unwrap() as u32) << 19;
            let mut age = f32::from_bits(case["inputAge"].as_u64().unwrap() as u32);
            assert_eq!(
                snapshot(&state, &scheduler, age),
                case["initial"],
                "initial {ordinal}"
            );
            for (events, final_state) in
                [("factories", "final"), ("secondFactories", "secondFinal")]
            {
                let mut observed = Vec::new();
                if let Some(mut cursor) = state.begin(&scheduler, age) {
                    while let Some(birth) = state.next_due(&scheduler, &mut cursor) {
                        let item = scheduler.items[birth.item];
                        let word =
                            (item.start_time as u16).wrapping_shl(2) | u16::from(item.enabled);
                        let mut event = json!({"index":birth.item,"age":birth.age.to_bits(),"entryWord":word,"timeline":compiled_timeline(&item),"before":snapshot(&state,&scheduler,age)});
                        state.finish_factory();
                        event["afterFactory"] = snapshot(&state, &scheduler, age);
                        if observed.is_empty() && case["mutation"] != 0 {
                            age = 9000.0;
                            state.flags = (state.flags & !0x180000) | 0x180000;
                            scheduler.item_count = Some(1);
                            if case["mutation"] == 2 {
                                for item in &mut scheduler.items {
                                    item.enabled = true;
                                    item.start_time = 0;
                                }
                                state.consumed = 0;
                            }
                        }
                        event["afterCallback"] = snapshot(&state, &scheduler, age);
                        observed.push(event);
                    }
                }
                assert_eq!(json!(observed), case[events], "{events} {ordinal}");
                comparisons += observed.len();
                let actual = snapshot(&state, &scheduler, age);
                assert_eq!(actual, case[final_state], "{final_state} {ordinal}");
            }
        }
        std::fs::write(output.join("scheduler-ordinary-rust-comparison.json"),serde_json::to_vec_pretty(&json!({"cases":original["cases"].as_array().unwrap().len(),"factoryCalls":comparisons,"differences":0,"productionCoreCompared":true,"scope":"Actual ordinary scalar constructor, mask/cursor and post-factory pending gate. Clock/definition mutations controlled; no owned children, full Scheduler Time/END, host or GPU."})).unwrap()).unwrap();
    }
}
