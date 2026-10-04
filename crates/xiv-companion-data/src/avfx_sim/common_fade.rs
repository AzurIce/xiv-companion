/// Common +0x1e8/+0x1ec fade duration and accumulated age. This numeric core
/// does not walk children, refresh base alpha, call retirement virtuals, or
/// gate Time callbacks; the caller must preserve the original phase order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VfxCommonFadeState {
    pub duration: f32,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCommonFadeStep {
    pub alpha: f32,
    /// Flags have already been updated. Invoke the object's +10 callback
    /// exactly here; recursive retirement is a separate control operation.
    pub retire: bool,
}

/// Common cached alpha and the independent +78 numeric/fade pass. Flags and
/// scaled delta belong to the owning lifecycle; Time/property callbacks can
/// refresh alpha separately. This is not an elapsed-time opacity envelope.
/// The caller also owns child traversal and the conditional global +18
/// adjustment after this numeric/fade segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxCommonNumericState {
    pub fade: VfxCommonFadeState,
    pub alpha: f32,
}

impl Default for VfxCommonNumericState {
    fn default() -> Self {
        Self {
            fade: VfxCommonFadeState::default(),
            alpha: 1.0,
        }
    }
}

impl VfxCommonNumericState {
    pub fn configure_fade(&mut self, flags: &mut u32, duration: i32, mode: u32, flag: bool) {
        self.fade.configure(flags, duration, mode, flag);
    }

    /// Apply only after successful registration and the child's +08 callback.
    /// Read the parent's state again at that boundary: +08 can change it.
    /// Original Attach does not walk the child's descendants or copy alpha.
    /// Mode zero skips inheritance even when fade-active bit 30 is set.
    pub fn inherit_fade_after_attach(&mut self, flags: &mut u32, parent_flags: u32, parent: Self) {
        if parent_flags & 0x18_0000 != 0 {
            const MASK: u32 = 0x4038_0000;
            *flags = (*flags & !MASK) | (parent_flags & MASK);
            self.fade = parent.fade;
        }
    }

    /// Begin Common +78: bit 26 gates the entire self callback, bit 29 gates
    /// numeric +100, then bit 30 gates fade. +100 may be a no-op; preserve cached
    /// alpha instead of implicitly resetting it to one. Re-read flags and fade
    /// state after +100, since that callback can reconfigure or retire self.
    ///
    /// If `retire` is returned, flags are already retired but alpha still holds
    /// the value visible inside +10. Run that callback before finish_refresh;
    /// it can create children, reconfigure fade, or retire the Document.
    /// A disabled self callback still permits traversal of children by callers.
    pub fn begin_refresh(
        &mut self,
        flags: &mut u32,
        scaled_delta: f32,
        refresh_numeric_values: impl FnOnce(&mut Self, &mut u32),
    ) -> Option<VfxCommonFadeStep> {
        if *flags & 0x0400_0000 == 0 {
            return None;
        }
        if *flags & 0x2000_0000 != 0 {
            refresh_numeric_values(self, flags);
        }
        Some(if *flags & 0x4000_0000 != 0 {
            self.fade.advance(flags, scaled_delta, self.alpha)
        } else {
            VfxCommonFadeStep {
                alpha: self.alpha,
                retire: false,
            }
        })
    }

    /// Native fade-out writes zero alpha after +10 returns. Do not restore the
    /// pre-callback fade fields/flags here: that callback may have changed them.
    pub fn finish_refresh(&mut self, step: VfxCommonFadeStep) {
        self.alpha = step.alpha;
    }
}

impl VfxCommonFadeState {
    /// Common control 0x1403b0730. Mode uses the low two bits. The flag
    /// inhibits fade-completion retirement, not the age/life-limit callback.
    /// The original function applies this same operation to all descendants.
    pub fn configure(&mut self, flags: &mut u32, duration: i32, mode: u32, flag: bool) {
        self.age = 0.0;
        self.duration = duration as f32;
        *flags =
            (*flags & 0xffc7_ffff) | ((mode & 3) << 19) | (u32::from(flag) << 21) | 0x4000_0000;
    }

    /// Original fade callback 0x1403af8c0, after numeric +100 has produced
    /// base alpha. Consume the actual scaled delta, including zero/negative
    /// and nonfinite inputs; this core does not sanitize native arithmetic.
    /// A finished fade-out retires only at age > duration, never at equality.
    pub fn advance(&mut self, flags: &mut u32, scaled_delta: f32, alpha: f32) -> VfxCommonFadeStep {
        self.age = scaled_delta + self.age;
        let mut result = VfxCommonFadeStep {
            alpha,
            retire: false,
        };
        match (*flags >> 19) & 3 {
            1 => {
                if self.age > self.duration {
                    *flags &= 0xbfc7_ffff;
                    self.duration = 0.0;
                    self.age = 0.0;
                } else {
                    result.alpha = (self.age / self.duration) * alpha;
                }
            }
            2 | 3 => {
                if self.age > self.duration {
                    if *flags & 0x24_0000 == 0 {
                        *flags = (*flags & 0xc0ff_ffff) | 0x4_0000;
                        result.retire = true;
                    }
                    result.alpha = 0.0;
                } else {
                    result.alpha = (1.0 - self.age / self.duration) * alpha;
                }
            }
            _ => {}
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_phase_gates_and_retirement_callback_preserve_write_order() {
        let mut state = VfxCommonNumericState::default();
        let mut flags = 0x3f00_0000;
        state.configure_fade(&mut flags, 0, 3, false);
        let step = state
            .begin_refresh(&mut flags, 1.0, |state, _| state.alpha = 0.8)
            .unwrap();
        assert!(step.retire);
        assert_ne!(flags & 0x40000, 0);
        assert_eq!(
            state.alpha, 0.8,
            "+10 observes refreshed alpha, before the zero write"
        );
        // A callback can change fade; the native final alpha write leaves that
        // reconfiguration in place, rather than restoring a pre-call snapshot.
        state.configure_fade(&mut flags, 8, 1, true);
        state.finish_refresh(step);
        assert_eq!(state.alpha, 0.0);
        assert_eq!(
            state.fade,
            VfxCommonFadeState {
                duration: 8.0,
                age: 0.0
            }
        );
        assert!(
            state
                .begin_refresh(&mut flags, 5.0, |_, _| panic!("retired self is gated"))
                .is_none()
        );
        assert_eq!(state.fade.age, 0.0);
        flags |= 0x0400_0000;
        let step = state
            .begin_refresh(&mut flags, 2.0, |_, _| panic!("bit 29 is disabled"))
            .unwrap();
        assert_eq!(state.fade.age, 2.0);
        state.finish_refresh(step);
        assert_eq!(state.alpha, 0.0);
    }

    #[test]
    fn attachment_inherits_current_nonzero_mode_without_alpha_or_descendant_rewrite() {
        let parent = VfxCommonNumericState {
            fade: VfxCommonFadeState {
                duration: 12.0,
                age: 7.0,
            },
            alpha: 0.3,
        };
        let mut child = VfxCommonNumericState {
            fade: VfxCommonFadeState {
                duration: 2.0,
                age: 1.0,
            },
            alpha: 0.8,
        };
        let original = child;
        let mut flags = 0x3f38_0000;
        child.inherit_fade_after_attach(&mut flags, 0x4000_0000, parent);
        assert_eq!(child, original, "active mode zero does not inherit");
        assert_eq!(flags, 0x3f38_0000);
        child.inherit_fade_after_attach(&mut flags, 0x4030_0000, parent);
        assert_eq!(flags, 0x7f30_0000);
        assert_eq!(child.fade, parent.fade);
        assert_eq!(child.alpha, 0.8);
        // Inactive nonzero mode also copies, clearing the child's active bit.
        child.inherit_fade_after_attach(&mut flags, 0x0008_0000, parent);
        assert_eq!(flags, 0x3f08_0000);
    }

    #[test]
    fn completion_equality_and_retirement_flag_have_distinct_effects() {
        for flag in [false, true] {
            let mut state = VfxCommonFadeState::default();
            let mut flags = 0x3f00_0000;
            state.configure(&mut flags, 4, 3, flag);
            assert_eq!(state.advance(&mut flags, 4.0, 0.8).alpha, 0.0);
            assert_eq!(flags & 0x40000, 0, "equality stays live");
            let step = state.advance(&mut flags, 0.25, 0.8);
            assert_eq!(step.alpha, 0.0);
            assert_eq!(step.retire, !flag);
            assert!(!state.advance(&mut flags, 1.0, 0.8).retire);
        }
    }

    #[test]
    fn fade_in_clears_at_crossing_but_zero_duration_preserves_nan() {
        let mut state = VfxCommonFadeState::default();
        let mut flags = 0x3f00_0000;
        state.configure(&mut flags, 4, 1, true);
        assert_eq!(state.advance(&mut flags, 2.0, 0.8).alpha, 0.4);
        assert_eq!(state.advance(&mut flags, 2.0, 0.6).alpha, 0.6);
        assert_ne!(flags & 0x4000_0000, 0);
        assert_eq!(state.advance(&mut flags, 0.25, 0.8).alpha, 0.8);
        assert_eq!(flags & 0x4038_0000, 0);
        assert_eq!(state, VfxCommonFadeState::default());
        state.configure(&mut flags, 0, 3, false);
        assert!(state.advance(&mut flags, 0.0, 0.8).alpha.is_nan());
    }
}
