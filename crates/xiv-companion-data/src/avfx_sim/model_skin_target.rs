//! ModelSkin target-list query and the gate around its Aura numeric callback.
//! Host callbacks supply already resolved handles; this is not an IFY adapter.

/// Shared constructor order for ModelSkin resources in one host playback group.
/// This records actual production creation order; it is not a captured QPC key.
/// The host resets it once before reconstructing every document in the group.
#[derive(Debug, Default)]
pub struct VfxModelSkinCreationOrder {
    next: std::sync::Arc<std::sync::Mutex<u64>>,
}

impl VfxModelSkinCreationOrder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reset(&self) {
        *self.next.lock().expect("ModelSkin creation order") = 0;
    }
    pub fn snapshot(&self) -> u64 {
        *self.next.lock().expect("ModelSkin creation order")
    }
}

#[derive(Debug)]
pub(super) struct CreationOrderEnvironment {
    next: std::sync::Arc<std::sync::Mutex<u64>>,
    initial: u64,
    host_managed: bool,
}

impl Clone for CreationOrderEnvironment {
    fn clone(&self) -> Self {
        Self {
            next: std::sync::Arc::new(std::sync::Mutex::new(
                *self.next.lock().expect("ModelSkin creation order"),
            )),
            initial: self.initial,
            host_managed: false,
        }
    }
}

impl CreationOrderEnvironment {
    pub fn new(order: &VfxModelSkinCreationOrder) -> Self {
        Self {
            next: order.next.clone(),
            initial: order.snapshot(),
            host_managed: true,
        }
    }
    pub fn share(&self) -> Self {
        Self {
            next: self.next.clone(),
            initial: self.initial,
            host_managed: self.host_managed,
        }
    }
    pub fn allocate(&self) -> Result<u64, String> {
        let mut next = self.next.lock().expect("ModelSkin creation order");
        let serial = *next;
        *next = next
            .checked_add(1)
            .ok_or("ModelSkin creation order limit")?;
        Ok(serial)
    }
    pub fn reset(&mut self) {
        let mut next = self.next.lock().expect("ModelSkin creation order");
        if self.host_managed {
            self.initial = *next;
        } else {
            *next = self.initial;
        }
    }
}

/// A resolved model handle and the byte inspected by the client target query.
/// Identity is opaque to the simulator. Only the low nibble of `type_code`
/// determines compatibility during the query's scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxModelSkinSurface {
    pub identity: u64,
    pub type_code: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxModelSkinCharacterTargets {
    pub generation: u32,
    /// Body, main hand, off hand, and the fourth character model slot.
    pub surfaces: [Option<VfxModelSkinSurface>; 4],
}

/// Results of the host callbacks for this query. `character` is the current
/// character for a negative selector, or the indexed character otherwise.
/// A fallback surface is consulted only for a negative selector without a
/// character, with `fallback_enabled` set. Its generation is a 16-bit value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VfxModelSkinTargetInput {
    pub listener_present: bool,
    pub character: Option<VfxModelSkinCharacterTargets>,
    pub fallback_enabled: bool,
    pub fallback: Option<(VfxModelSkinSurface, u16)>,
}

/// Explicit resolved host input. The selector is fixed at construction; host
/// snapshots may change at subsequent input boundaries. This is not IFY lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxModelSkinTargetSnapshot {
    pub selector: i32,
    pub input: VfxModelSkinTargetInput,
}

#[derive(Debug)]
pub(super) struct TargetEnvironment {
    pub selector: i32,
    input: std::sync::Arc<std::sync::Mutex<VfxModelSkinTargetInput>>,
}

// Independent runtimes own independent inputs; their retained tree shares the
// runtime's cell explicitly, just like the client random environment.
impl Clone for TargetEnvironment {
    fn clone(&self) -> Self {
        Self::new(VfxModelSkinTargetSnapshot {
            selector: self.selector,
            input: self.input(),
        })
    }
}

impl TargetEnvironment {
    pub fn new(snapshot: VfxModelSkinTargetSnapshot) -> Self {
        Self {
            selector: snapshot.selector,
            input: std::sync::Arc::new(std::sync::Mutex::new(snapshot.input)),
        }
    }

    pub fn share(&self) -> Self {
        Self {
            selector: self.selector,
            input: self.input.clone(),
        }
    }

    pub fn input(&self) -> VfxModelSkinTargetInput {
        *self.input.lock().expect("ModelSkin host input")
    }

    pub fn set_input(&self, input: VfxModelSkinTargetInput) -> bool {
        let mut current = self.input.lock().expect("ModelSkin host input");
        let changed = *current != input;
        *current = input;
        changed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxModelSkinTargetStatus {
    Missing,
    Ready,
    GenerationChanged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VfxModelSkinTargetList {
    pub surfaces: [Option<VfxModelSkinSurface>; 4],
    /// Body-only material filters. Auxiliary slots do not use this mask.
    pub body_filter: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxModelSkinTargetQuery {
    pub status: VfxModelSkinTargetStatus,
    /// Exact target value 16 returns Ready without writing the output list.
    pub list: Option<VfxModelSkinTargetList>,
}

/// Registration requests after a successful Aura parameter update. The host
/// owns their effects and the material handles represented by the filter bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxModelSkinRegistration {
    Document,
    Surface {
        surface: VfxModelSkinSurface,
        material_filters: u32,
    },
}

/// Persistent storage at the original object's +400/+404. The first nonzero
/// generation is retained, including a query with no compatible surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VfxModelSkinTargetState {
    pub selector: i32,
    pub cached_generation: u32,
}

impl VfxModelSkinTargetState {
    pub fn new(selector: i32) -> Self {
        Self {
            selector,
            cached_generation: 0,
        }
    }

    pub fn query(
        &mut self,
        target: u32,
        input: VfxModelSkinTargetInput,
    ) -> VfxModelSkinTargetQuery {
        use VfxModelSkinTargetStatus::{GenerationChanged, Missing, Ready};
        if target == 16 {
            return VfxModelSkinTargetQuery {
                status: Ready,
                list: None,
            };
        }
        let mut list = VfxModelSkinTargetList::default();
        let finish = |status| VfxModelSkinTargetQuery {
            status,
            list: Some(list),
        };
        if !input.listener_present {
            return finish(Missing);
        }
        let (generation, surfaces) = if let Some(character) = input.character {
            (character.generation, character.surfaces)
        } else if self.selector < 0 && input.fallback_enabled {
            let Some((surface, generation)) = input.fallback else {
                return finish(Missing);
            };
            (u32::from(generation), [Some(surface), None, None, None])
        } else {
            return finish(Missing);
        };
        if self.cached_generation == 0 {
            self.cached_generation = generation;
        } else if self.cached_generation != generation {
            return finish(GenerationChanged);
        }
        for (slot, surface) in surfaces.into_iter().enumerate() {
            if target & (1 << slot) != 0 {
                list.surfaces[slot] = surface;
            }
        }
        if target & 1 != 0 {
            list.body_filter = target & 0x3e0;
        }
        // The original returns immediately at the FIRST compatible slot. It
        // clears preceding incompatible slots, but never examines later slots.
        let mut status = Missing;
        for surface in &mut list.surfaces {
            if surface.is_some_and(|surface| surface.type_code & 15 == 3) {
                status = Ready;
                break;
            }
            *surface = None;
        }
        VfxModelSkinTargetQuery {
            status,
            list: Some(list),
        }
    }
}

impl VfxModelSkinTargetQuery {
    /// Apply the original +100 generation-change branch before Common's
    /// separate fade phase. Missing targets preserve flags. A changed target
    /// suppresses Aura and clears bits 24..29 only on the first transition.
    pub fn begin_numeric(self, common_flags: &mut u32) -> bool {
        if self.status == VfxModelSkinTargetStatus::GenerationChanged
            && *common_flags & 0x40000 == 0
        {
            *common_flags = (*common_flags & 0xc0ff_ffff) | 0x40000;
        }
        self.status == VfxModelSkinTargetStatus::Ready
    }

    pub fn registrations(self, aura_succeeded: bool) -> Vec<VfxModelSkinRegistration> {
        if !aura_succeeded || self.status != VfxModelSkinTargetStatus::Ready {
            return Vec::new();
        }
        let Some(list) = self.list else {
            return vec![VfxModelSkinRegistration::Document];
        };
        list.surfaces
            .into_iter()
            .enumerate()
            .filter_map(|(slot, surface)| {
                surface.map(|surface| VfxModelSkinRegistration::Surface {
                    surface,
                    material_filters: if slot == 0 { list.body_filter } else { 0 },
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "model_skin_target_tests.rs"]
mod tests;
