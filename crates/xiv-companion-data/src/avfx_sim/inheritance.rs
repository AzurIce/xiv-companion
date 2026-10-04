use super::{AvfxEmitter, AvfxEmitterItem, InstanceClock, SplitMix64, eval3_seeded_at};

/// Client 0x14039f648 normalizes these modes and separately enables a ground
/// query. The preview has no scene collision data for that query.
pub(super) fn coordinate_mode(raw: i32) -> i32 {
    match raw {
        4 => 2,
        5 => 3,
        6 => 0,
        7 => 1,
        9 => 8,
        _ => raw,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Components {
    pub scale: [f32; 3],
    pub rotation: [f32; 3],
}

impl Components {
    const IDENTITY: Self = Self {
        scale: [1.0; 3],
        rotation: [0.0; 3],
    };
}

/// Scalar/Euler getters have their own parent chain, distinct from full matrix
/// composition. Each link can stop scale and rotation independently.
pub(super) struct EmitterComponents<'a> {
    pub emitter: &'a AvfxEmitter,
    pub emitter_index: usize,
    pub instance_seed: u64,
    pub clock: InstanceClock,
    pub parent: ParentComponents<'a>,
    pub creation_angle: [f32; 3],
}

impl EmitterComponents<'_> {
    pub fn at(&self, frame: f32) -> Components {
        self.at_ages(frame, self.clock.ages(frame))
    }

    pub fn at_ages(&self, frame: f32, ages: super::CurveAges) -> Components {
        let mut rng = SplitMix64::seeded(
            self.emitter_index as u64 ^ 0xE417,
            0,
            self.instance_seed,
            0x1A2B,
        );
        let _position_seed = rng.next_u64();
        let rotation = eval3_seeded_at(&self.emitter.rotation, ages, 0.0, rng.next_u64());
        let scale = eval3_seeded_at(&self.emitter.scale, ages, 1.0, rng.next_u64());
        let parent = self.parent.at(frame);
        Components {
            scale: std::array::from_fn(|axis| scale[axis] * parent.scale[axis]),
            rotation: std::array::from_fn(|axis| {
                rotation[axis] + self.creation_angle[axis] + parent.rotation[axis]
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ParentComponents<'a> {
    None,
    Initial(Components),
    Always {
        animation: &'a EmitterComponents<'a>,
        birth: f32,
        scale: bool,
        rotation: bool,
    },
}

impl<'a> ParentComponents<'a> {
    pub fn new(item: &AvfxEmitterItem, animation: &'a EmitterComponents<'a>, birth: f32) -> Self {
        let (scale, rotation) = match coordinate_mode(item.parent_influence_coord) {
            0 => return Self::None,
            3 => return Self::Initial(animation.at(birth)),
            2 | 8 => (true, true),
            _ => (item.influence_coord_scale, item.influence_coord_rot),
        };
        if !scale && !rotation {
            Self::None
        } else {
            Self::Always {
                animation,
                birth,
                scale,
                rotation,
            }
        }
    }

    pub fn at(self, age: f32) -> Components {
        match self {
            Self::None => Components::IDENTITY,
            Self::Initial(value) => value,
            Self::Always {
                animation,
                birth,
                scale,
                rotation,
            } => {
                let value = animation.at(birth + age.max(0.0));
                Components {
                    scale: if scale { value.scale } else { [1.0; 3] },
                    rotation: if rotation { value.rotation } else { [0.0; 3] },
                }
            }
        }
    }
}
