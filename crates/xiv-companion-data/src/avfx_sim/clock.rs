use super::{AvfxEmitter, AvfxEmitterItem, SplitMix64, roll_amplitude};
use crate::avfx::AvfxLife;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CurveAges {
    pub local: f32,
    pub total: f32,
}

/// Framework quantization precedes the manager's two f32 normalization steps.
#[derive(Default)]
pub(super) struct FrameworkClock {
    pub remainder: f32,
}

impl FrameworkClock {
    pub fn advance(&mut self, seconds: f32) -> Result<f32, String> {
        let frames = seconds * super::AVFX_FPS + self.remainder;
        let tenths = frames * 10.0;
        if !seconds.is_finite() || seconds < 0.0 || !(-2147483648.0..2147483648.0).contains(&tenths)
        {
            return Err(
                "update seconds must be nonnegative, finite and fit the client clock".into(),
            );
        }
        let delta = (tenths as i32) as f32 / 10.0;
        self.remainder = frames - delta;
        Ok(delta * 60.0 / 60.0)
    }
}

/// Continuous preview of the client's separate instance age, curve loop and
/// lifetime clock. Local loops preserve the instance and its random state.
#[derive(Clone, Copy, Debug)]
pub(super) struct InstanceClock {
    pub life: f32,
    pub rate: f32,
    pub loop_start: f32,
    pub loop_end: f32,
    /// Virtual elapsed time during construction, with the external parent frozen.
    pub warmup: f32,
    /// An age assignment skips updates and therefore contributes no motion.
    curve_start: f32,
    loop_enabled: bool,
}

impl InstanceClock {
    pub fn root(emitter: &AvfxEmitter, duration: f32) -> Self {
        Self::new(duration, 1.0, emitter.loop_start, emitter.loop_end)
    }

    pub fn new(life: f32, rate: f32, start: i32, end: i32) -> Self {
        let (loop_start, loop_end) = if end > 0 && end > start {
            (start as f32 * rate, end as f32 * rate)
        } else {
            (0.0, 0.0)
        };
        Self {
            life,
            rate,
            loop_start,
            loop_end,
            warmup: 0.0,
            curve_start: 0.0,
            loop_enabled: true,
        }
    }

    pub fn child(
        self,
        life: AvfxLife,
        loops: [i32; 2],
        item: &AvfxEmitterItem,
        random: &mut SplitMix64,
    ) -> Self {
        let nominal = if life.enabled { life.value } else { -1.0 };
        let selected = if item.inherit_parent_life {
            self.life
        } else if item.override_life {
            let spread = item.override_life_random as f32;
            item.override_life_value as f32
                + if spread == 0.0 {
                    0.0
                } else {
                    (random.next_f32() * (2.0 * spread + 1.0)).floor() - spread
                }
        } else {
            nominal
                + if life.enabled && life.value_random != 0.0 {
                    roll_amplitude(random, life.random_type, life.value_random)
                } else {
                    0.0
                }
        };
        let rate = if nominal > 0.0 && selected > 0.0 {
            nominal / selected
        } else {
            1.0
        };
        let mut clock = Self::new(nominal, rate, loops[0], loops[1]);
        let start = item.start_frame.max(0) as f32;
        if item.start_frame_null_update {
            clock.warmup = start;
        } else {
            // The first update loops the assigned age before checking Life.
            clock.curve_start = clock.loop_age(start * rate);
        }
        clock
    }

    /// A conservative real-time lifetime bound before per-instance randomness.
    /// Any reachable immortal loop prevents discarding earlier births.
    pub fn max_child_duration(
        self,
        life: AvfxLife,
        loops: [i32; 2],
        item: &AvfxEmitterItem,
    ) -> Option<f32> {
        if !life.enabled || life.value < 0.0 {
            return None;
        }
        let selected_max = if item.inherit_parent_life {
            self.life
        } else if item.override_life {
            item.override_life_value as f32 + (item.override_life_random as f32).abs()
        } else {
            life.value + life.value_random.abs()
        };
        // A nonpositive selected lifetime falls back to rate 1.
        let maximum = if life.value == 0.0 {
            0.0
        } else {
            selected_max.max(life.value)
        };
        let slowest_rate = if life.value > 0.0 {
            life.value / maximum
        } else {
            1.0
        };
        let loops_enabled = loops[1] > 0 && loops[1] > loops[0];
        if loops_enabled && loops[1] as f32 * slowest_rate <= life.value {
            return None;
        }
        // An assigned age may wrap into a negative loop start. That skipped
        // prefix can extend the remaining finite lifetime in real frames.
        let extension = if loops_enabled && item.start_frame > 0 && !item.start_frame_null_update {
            -(loops[0] as f32).min(0.0)
        } else {
            0.0
        };
        // Reproduce the two rounded divisions used by the instance clock;
        // `selected_max` alone can round its true finish down by one ULP.
        let duration = life.value / slowest_rate;
        Some((duration + extension).next_up().next_up())
    }

    pub fn loops(self) -> bool {
        self.loop_enabled && self.loop_end > 0.0 && self.loop_end > self.loop_start
    }

    pub fn unlock_loop_point(&mut self) {
        self.loop_enabled = false;
    }

    pub fn loop_age(self, age: f32) -> f32 {
        if self.loops() && age >= self.loop_end {
            self.loop_start + (age - self.loop_start).rem_euclid(self.loop_end - self.loop_start)
        } else {
            age
        }
    }

    /// Client checks the looped age, strictly after Life. A loop that returns
    /// before that boundary keeps the instance alive until an external finish.
    pub fn finish_frame(self) -> Option<f32> {
        (self.life >= 0.0 && (!self.loops() || self.life < self.loop_end))
            .then_some((self.life - self.curve_start).max(0.0) / self.rate - self.warmup)
    }

    pub fn elapsed(self, frame: f32) -> f32 {
        (self
            .finish_frame()
            .map_or(frame, |finish| frame.min(finish))
            + self.warmup)
            .max(0.0)
    }

    pub fn motion_range(self, frame: f32) -> [f32; 2] {
        [
            self.curve_start,
            self.curve_start + self.elapsed(frame) * self.rate,
        ]
    }

    /// Snapshots store already rated curve time. Recover the frame argument
    /// expected by continuous integration without multiplying the rate again.
    pub fn frame_for_curve_total(self, total: f32) -> f32 {
        (total - self.curve_start) / self.rate - self.warmup
    }

    pub fn age(self, frame: f32) -> f32 {
        // StFr is written after initialization. Without null updates, the
        // initial geometry and creation callbacks still use age zero.
        if frame <= 0.0 && self.warmup == 0.0 {
            return 0.0;
        }
        let age = self.loop_age(self.motion_range(frame)[1]);
        if self.finish_frame().is_some() {
            age.min(self.life)
        } else {
            age
        }
    }

    pub fn ages(self, frame: f32) -> CurveAges {
        let total = self.motion_range(frame)[1];
        CurveAges {
            local: self.age(frame),
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looped_and_accumulated_curve_ages_remain_distinct() {
        let clock = InstanceClock::new(-1.0, 1.0, 2, 10);
        assert_eq!(
            clock.ages(25.0),
            CurveAges {
                local: 9.0,
                total: 25.0,
            }
        );
    }
}
