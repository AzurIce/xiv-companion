/// Explicit snapshot of the client's four-word TLS xorshift state. Callers
/// share one mutable state across constructors in their actual creation order;
/// copying it for each Binder would repeat draws and change later objects.
/// This does not capture a live client stream or account for other consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VfxClientRandomState {
    words: [u32; 4],
}

impl VfxClientRandomState {
    pub fn from_words(words: [u32; 4]) -> Self {
        Self { words }
    }

    pub fn words(self) -> [u32; 4] {
        self.words
    }

    /// One original inline TLS transition; Spline consumes the low 16 bits.
    /// Preserve an all-zero snapshot instead of silently inventing a seed.
    pub fn next_u16(&mut self) -> u16 {
        let t = self.words[0] ^ (self.words[0] << 11);
        let last = self.words[3];
        self.words.copy_within(1..4, 0);
        self.words[3] = last ^ (last >> 19) ^ t ^ (t >> 8);
        self.words[3] as u16
    }
}

/// Explicit shared stream for an ordered group of preview documents. This
/// owns a replay seed, not a captured live client TLS state. Each attached
/// runtime continues the same stream at construction and input boundaries.
/// The host must reset once and reconstruct every playback before any input
/// when seeking the group backwards. Individual Document REST keeps it.
#[derive(Debug)]
pub struct VfxSharedRandomStream {
    pub(super) cell: VfxClientRandomCell,
    pub(super) initial: VfxClientRandomState,
}

impl VfxSharedRandomStream {
    pub fn new(initial: VfxClientRandomState) -> Self {
        Self {
            cell: VfxClientRandomCell::new(initial),
            initial,
        }
    }

    pub fn snapshot(&self) -> VfxClientRandomState {
        self.cell.get()
    }
    pub fn reset(&self) {
        self.cell.set(self.initial);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RandomResetPolicy {
    Owned,
    HostManaged,
}

// A Runtime clone gets an independent cell, so it also regains an owned
// replay reset. Sharing is opt-in through with_shared_client_random_stream.
impl Clone for RandomResetPolicy {
    fn clone(&self) -> Self {
        Self::Owned
    }
}

/// Runtime-owned stream. Clone copies the snapshot so independent playbacks
/// do not consume each other's words; consumers in one runtime share it.
#[derive(Debug)]
pub(super) struct VfxClientRandomCell(std::sync::Arc<std::sync::Mutex<VfxClientRandomState>>);

impl Clone for VfxClientRandomCell {
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl VfxClientRandomCell {
    pub(super) fn new(state: VfxClientRandomState) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(state)))
    }

    /// A retained playback tree shares its runtime's stream. Ordinary Clone
    /// still copies the snapshot so independent runtimes remain independent.
    pub(super) fn share(&self) -> Self {
        Self(std::sync::Arc::clone(&self.0))
    }

    pub(super) fn get(&self) -> VfxClientRandomState {
        *self.0.lock().expect("client random state poisoned")
    }

    pub(super) fn set(&self, state: VfxClientRandomState) {
        *self.0.lock().expect("client random state poisoned") = state;
    }

    pub(super) fn next_u16(&self) -> u16 {
        self.0
            .lock()
            .expect("client random state poisoned")
            .next_u16()
    }

    pub(super) fn with_mut<T>(&self, f: impl FnOnce(&mut VfxClientRandomState) -> T) -> T {
        f(&mut self.0.lock().expect("client random state poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use super::VfxClientRandomState;

    #[test]
    fn zero_client_snapshot_stays_zero() {
        let mut state = VfxClientRandomState::from_words([0; 4]);
        for _ in 0..8 {
            assert_eq!(state.next_u16(), 0);
        }
        assert_eq!(state.words(), [0; 4]);
    }

    #[test]
    fn shared_snapshot_matches_two_original_spline_factory_births() {
        // Original Item factory: two target births, both controls disabled,
        // seed and final words captured by probe-spline-target-factory.py.
        let seed = [123456792, 362436069, 521288629, 88675123];
        let mut state = VfxClientRandomState::from_words(seed);
        for _ in 0..2 {
            for _ in 0..4 {
                state.next_u16();
            }
        }
        assert_eq!(
            state.words(),
            [495446302, 2377263435, 2579005291, 717252425]
        );
        let mut per_object_copy = VfxClientRandomState::from_words(seed);
        for _ in 0..4 {
            per_object_copy.next_u16();
        }
        assert_ne!(state, per_object_copy);
    }
}
