use std::{cmp::Ordering, collections::BinaryHeap};

use super::{
    AvfxEmitter, AvfxEmitterItem, AvfxFile, AvfxParticle, InstanceClock, SplitMix64,
    emitter_can_create, item_events,
};
use crate::avfx::{AvfxCurve, BEHAVIOR_CONST};

fn constant_curve(curve: &AvfxCurve) -> bool {
    curve
        .keys
        .first()
        .is_none_or(|first| curve.keys.iter().all(|key| key.z == first.z))
}

fn constant_curve_tail(curve: &AvfxCurve) -> Option<f32> {
    if constant_curve(curve) {
        return Some(0.0);
    }
    if curve.post_behavior & 3 != BEHAVIOR_CONST
        || !curve
            .keys
            .windows(2)
            .all(|keys| keys[0].scalar_time() <= keys[1].scalar_time())
    {
        return None;
    }
    curve.keys.last().map(|key| f32::from(key.scalar_time()))
}

fn constant_random_count(random: &AvfxCurve) -> bool {
    random.random_type & 7 >= 6
        || random.keys.iter().all(|key| key.z == 0.0)
        || (random.random_type & 7 <= 2 && constant_curve(random))
}

fn constant_count_tail(emitter: &AvfxEmitter, clock: InstanceClock) -> Option<f32> {
    if clock.loops() {
        return None;
    }
    let random = &emitter.create_count_random;
    let random_tail = if constant_random_count(random) {
        Some(0.0)
    } else if random.random_type & 7 <= 2 {
        constant_curve_tail(random)
    } else {
        None
    };
    Some(constant_curve_tail(&emitter.create_count)?.max(random_tail?))
}

fn constant_create_count(emitter: &AvfxEmitter, item: &AvfxEmitterItem) -> bool {
    item.create_time != 0
        || (constant_curve(&emitter.create_count)
            && constant_random_count(&emitter.create_count_random))
}

/// Per-item signed-short counter. It advances after probability acceptance,
/// before factory capacity/lifetime/geometry rejection, and survives loops.
#[derive(Clone, Default)]
pub(super) struct InjectionCounter {
    next_event: u64,
    ordinal: i16,
}

impl InjectionCounter {
    pub fn begin_event(
        &mut self,
        emitter: &AvfxEmitter,
        clock: InstanceClock,
        item: &AvfxEmitterItem,
        event: u64,
        frame: f32,
        interval_seed: u64,
        random: impl Fn(u64, u64) -> SplitMix64,
    ) {
        if item.by_injection_angle != [0.0; 3] && event > self.next_event {
            if constant_create_count(emitter, item) && item.create_probability >= 100 {
                let count = super::emitter_create_count_at_seeded(
                    emitter,
                    item,
                    0.0,
                    0.0,
                    0,
                    interval_seed,
                ) as i16;
                self.ordinal = self
                    .ordinal
                    .wrapping_add(((event - self.next_event) as i16).wrapping_mul(count));
            } else {
                // Recover counters from dead history using the sampler's own
                // probability draws. This does not reproduce the client RNG.
                let constant_tail = constant_count_tail(emitter, clock);
                for (index, birth) in item_events(
                    emitter,
                    clock,
                    item.create_time,
                    -clock.warmup,
                    frame,
                    interval_seed,
                )
                .take_while(|&(index, _)| index < event)
                {
                    if index < self.next_event {
                        continue;
                    }
                    let ages = clock.ages(birth);
                    let count = super::emitter_create_count_at_seeded(
                        emitter,
                        item,
                        ages.local,
                        ages.total,
                        index,
                        interval_seed,
                    );
                    if item.create_probability >= 100
                        && constant_tail.is_some_and(|end| ages.local > end)
                    {
                        self.ordinal = self
                            .ordinal
                            .wrapping_add(((event - index) as i16).wrapping_mul(count as i16));
                        break;
                    }
                    let accepted = if item.create_probability >= 100 {
                        count
                    } else {
                        (0..count)
                            .filter(|&copy| {
                                random(index, copy).next_f32() * 100.0
                                    < item.create_probability as f32
                            })
                            .count() as u64
                    };
                    self.ordinal = self.ordinal.wrapping_add(accepted as i16);
                }
            }
        }
        self.next_event = event.saturating_add(1);
    }

    pub fn take_angle(&mut self, item: &AvfxEmitterItem) -> [f32; 3] {
        let ordinal = self.ordinal;
        self.ordinal = self.ordinal.wrapping_add(1);
        item.by_injection_angle
            .map(|angle| angle * f32::from(ordinal))
    }
}

fn accepted_creations_between(
    emitter: &AvfxEmitter,
    clock: InstanceClock,
    item: &AvfxEmitterItem,
    start_event: u64,
    end_event: u64,
    frame: f32,
    interval_seed: u64,
    random: impl Fn(u64, u64) -> SplitMix64,
) -> u64 {
    if end_event <= start_event {
        return 0;
    }
    if constant_create_count(emitter, item) && item.create_probability >= 100 {
        let count =
            super::emitter_create_count_at_seeded(emitter, item, 0.0, 0.0, 0, interval_seed);
        return (end_event - start_event).wrapping_mul(count);
    }

    let constant_tail = constant_count_tail(emitter, clock);
    let mut accepted = 0u64;
    for (index, birth) in item_events(
        emitter,
        clock,
        item.create_time,
        -clock.warmup,
        frame,
        interval_seed,
    )
    .take_while(|&(index, _)| index < end_event)
    {
        if index < start_event {
            continue;
        }
        let ages = clock.ages(birth);
        let count = super::emitter_create_count_at_seeded(
            emitter,
            item,
            ages.local,
            ages.total,
            index,
            interval_seed,
        );
        if item.create_probability >= 100 && constant_tail.is_some_and(|end| ages.local > end) {
            accepted = accepted.wrapping_add((end_event - index).wrapping_mul(count));
            break;
        }
        let count = if item.create_probability >= 100 {
            count
        } else {
            (0..count)
                .filter(|&copy| {
                    random(index, copy).next_f32() * 100.0 < item.create_probability as f32
                })
                .count() as u64
        };
        accepted = accepted.wrapping_add(count);
    }
    accepted
}

#[derive(Clone, Copy)]
pub(super) enum CreationTarget<'a> {
    Particle(&'a AvfxParticle),
    Emitter(&'a AvfxEmitter),
}

pub(super) struct Creation<'a> {
    pub target: CreationTarget<'a>,
    pub item: &'a AvfxEmitterItem,
    pub item_index: usize,
    pub event: u64,
    pub frame: f32,
}

struct ItemStream<'a> {
    kind: u8,
    target: CreationTarget<'a>,
    item: &'a AvfxEmitterItem,
    item_index: usize,
    first_event: Option<(u64, f32)>,
    events: Box<dyn Iterator<Item = (u64, f32)> + 'a>,
}

struct Pending {
    frame: f32,
    phase: u8,
    event: u64,
    kind: u8,
    item_index: usize,
    stream: usize,
}

impl Ord for Pending {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max heap. Reverse the full chronological key, keeping
        // initialization before periodic startup and ItPr before ItEm.
        other
            .frame
            .total_cmp(&self.frame)
            .then_with(|| other.phase.cmp(&self.phase))
            .then_with(|| other.event.cmp(&self.event))
            .then_with(|| other.kind.cmp(&self.kind))
            .then_with(|| other.item_index.cmp(&self.item_index))
            .then_with(|| other.stream.cmp(&self.stream))
    }
}

impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Pending {}

/// Merge each item's causal history with one pending event per item. Keeping
/// the original event ordinals preserves seeds when dead history is skipped.
pub(super) struct CreationSchedule<'a> {
    emitter: &'a AvfxEmitter,
    clock: InstanceClock,
    until: f32,
    streams: Vec<ItemStream<'a>>,
    pending: BinaryHeap<Pending>,
    interval_seed: u64,
}

impl<'a> CreationSchedule<'a> {
    pub fn new(
        file: &'a AvfxFile,
        emitter: &'a AvfxEmitter,
        clock: InstanceClock,
        until: f32,
        interval_seed: u64,
    ) -> Self {
        let mut oldest = until;
        for (kind, items) in [(0, &emitter.particle_items), (1, &emitter.emitter_items)] {
            for item in items {
                if !item.enabled
                    || item.target_index < 0
                    || item.create_probability <= 0
                    || !emitter_can_create(emitter, item)
                {
                    continue;
                }
                let max_life = if kind == 0 {
                    let Some(particle) = file.particles.get(item.target_index as usize) else {
                        continue;
                    };
                    clock.max_child_duration(
                        particle.life,
                        [particle.loop_start, particle.loop_end],
                        item,
                    )
                } else {
                    let Some(_child) = file.emitters.get(item.target_index as usize) else {
                        continue;
                    };
                    None
                };
                let candidate =
                    max_life.map_or(-clock.warmup, |life| (until - life).next_down().next_down());
                oldest = oldest.min(candidate);
            }
        }
        let mut schedule = Self {
            emitter,
            clock,
            until,
            streams: Vec::new(),
            pending: BinaryHeap::new(),
            interval_seed,
        };
        for (kind, items) in [(0, &emitter.particle_items), (1, &emitter.emitter_items)] {
            for (item_index, item) in items.iter().enumerate() {
                if !item.enabled
                    || item.target_index < 0
                    || item.create_probability <= 0
                    || !emitter_can_create(emitter, item)
                {
                    continue;
                }
                let target = if kind == 0 {
                    let Some(particle) = file.particles.get(item.target_index as usize) else {
                        continue;
                    };
                    CreationTarget::Particle(particle)
                } else {
                    let Some(child) = file.emitters.get(item.target_index as usize) else {
                        continue;
                    };
                    CreationTarget::Emitter(child)
                };
                let events = item_events(
                    emitter,
                    clock,
                    item.create_time,
                    oldest,
                    until,
                    interval_seed,
                )
                .filter(move |&(_, birth)| birth >= oldest);
                let stream = schedule.streams.len();
                schedule.streams.push(ItemStream {
                    kind,
                    target,
                    item,
                    item_index,
                    first_event: None,
                    events: Box::new(events),
                });
                schedule.advance(stream);
            }
        }
        schedule
    }

    fn advance(&mut self, stream: usize) {
        let source = &mut self.streams[stream];
        if let Some((event, frame)) = source.events.next() {
            source.first_event.get_or_insert((event, frame));
            self.pending.push(Pending {
                frame: if frame == 0.0 { 0.0 } else { frame },
                phase: match source.item.create_time {
                    1 => 0,
                    0 => 1,
                    _ => 2,
                },
                event,
                kind: match source.target {
                    CreationTarget::Particle(_) => 0,
                    CreationTarget::Emitter(_) => 1,
                },
                item_index: source.item_index,
                stream,
            });
        }
    }

    /// Number of probability-accepted factory calls omitted before the common
    /// live-history window. Ordered shape modes use this as their shared
    /// emitter-instance counter prefix.
    pub fn skipped_creations(&self, instance_seed: u64) -> u64 {
        self.streams
            .iter()
            .map(|stream| {
                let (event, frame) = stream.first_event.unwrap_or_else(|| {
                    item_events(
                        self.emitter,
                        self.clock,
                        stream.item.create_time,
                        self.until,
                        self.until,
                        self.interval_seed,
                    )
                    .last()
                    .map_or((0, self.until), |(event, frame)| {
                        (event.saturating_add(1), frame)
                    })
                });
                let (item_salt, draw_salt) = if stream.kind == 0 {
                    (0x17_5052, 0xC2B2_AE3D)
                } else {
                    (0x17_454D, 0x3C4D_5E6F)
                };
                let item_seed =
                    SplitMix64::seeded(instance_seed, stream.item_index as u64, 0, item_salt)
                        .next_u64();
                accepted_creations_between(
                    self.emitter,
                    self.clock,
                    stream.item,
                    0,
                    event,
                    frame,
                    self.interval_seed,
                    |event, copy| SplitMix64::seeded(item_seed, event, copy, draw_salt),
                )
            })
            .fold(0u64, u64::wrapping_add)
    }
}

impl<'a> Iterator for CreationSchedule<'a> {
    type Item = Creation<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.pending.pop()?;
        self.advance(next.stream);
        let source = &self.streams[next.stream];
        Some(Creation {
            target: source.target,
            item: source.item,
            item_index: source.item_index,
            event: next.event,
            frame: next.frame,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey, AvfxLife};

    fn curve(start: f32, end: f32, end_frame: i16) -> AvfxCurve {
        let first = AvfxCurveKey {
            time: 0,
            interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
            x: 0.0,
            y: 0.0,
            z: start,
        };
        AvfxCurve {
            keys: vec![
                first,
                AvfxCurveKey {
                    time: end_frame,
                    z: end,
                    ..first
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn random_count_tail_recovery_matches_event_by_event_history() {
        for (mode, random_curve) in [(1, curve(0.0, 100.0, 19)), (4, curve(100.0, 100.0, 19))] {
            let mut emitter = AvfxEmitter {
                create_count: curve(1.0, 2.0, 4),
                create_interval: curve(1.0, 1.0, 1),
                create_count_random: random_curve,
                ..Default::default()
            };
            emitter.create_count_random.random_type = mode;
            let item = AvfxEmitterItem {
                enabled: true,
                target_index: 0,
                create_time: 0,
                create_probability: 100,
                by_injection_angle: [0.0, 0.0, 0.01],
                ..Default::default()
            };
            let seed = (0..100)
                .find(|&seed| {
                    let counts = (4..20)
                        .map(|event| {
                            super::super::emitter_create_count_at_seeded(
                                &emitter,
                                &item,
                                event as f32,
                                event as f32,
                                event,
                                seed,
                            )
                        })
                        .collect::<Vec<_>>();
                    counts.iter().any(|&count| count != counts[0])
                })
                .expect("random counts must vary after the CrC tail");
            let counts = (0..19)
                .map(|event| {
                    super::super::emitter_create_count_at_seeded(
                        &emitter,
                        &item,
                        event as f32,
                        event as f32,
                        event,
                        seed,
                    )
                })
                .collect::<Vec<_>>();
            let clock = InstanceClock::root(&emitter, -1.0);
            let accepted = accepted_creations_between(
                &emitter,
                clock,
                &item,
                3,
                19,
                19.0,
                seed,
                |event, copy| SplitMix64::seeded(0, event, copy, 0),
            );
            assert_eq!(accepted, counts[3..].iter().sum::<u64>(), "mode={mode}");

            let mut counter = InjectionCounter::default();
            counter.begin_event(&emitter, clock, &item, 19, 19.0, seed, |event, copy| {
                SplitMix64::seeded(0, event, copy, 0)
            });
            let ordinal = counts.iter().sum::<u64>() as i16;
            assert_eq!(
                counter.take_angle(&item)[2],
                0.01 * f32::from(ordinal),
                "mode={mode}"
            );

            if mode == 1 {
                assert_eq!(constant_count_tail(&emitter, clock), Some(19.0));
                let tail_count = super::super::emitter_create_count_at_seeded(
                    &emitter, &item, 20.0, 20.0, 20, seed,
                );
                let expected = counts.iter().sum::<u64>()
                    + super::super::emitter_create_count_at_seeded(
                        &emitter, &item, 19.0, 19.0, 19, seed,
                    )
                    + (1_000_000 - 20) * tail_count;
                assert_eq!(
                    accepted_creations_between(
                        &emitter,
                        clock,
                        &item,
                        0,
                        1_000_000,
                        1_000_000.0,
                        seed,
                        |event, copy| SplitMix64::seeded(0, event, copy, 0),
                    ),
                    expected
                );
            }

            emitter.particle_items.push(item);
            let file = AvfxFile {
                emitters: vec![emitter],
                particles: vec![AvfxParticle {
                    life: AvfxLife {
                        enabled: true,
                        value: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                ..Default::default()
            };
            let schedule = CreationSchedule::new(&file, &file.emitters[0], clock, 19.0, seed);
            assert_eq!(
                schedule.skipped_creations(seed),
                counts.iter().sum::<u64>(),
                "mode={mode}"
            );
        }
    }

    #[test]
    fn first_random_constant_count_skips_long_histories_with_its_seeded_value() {
        let mut emitter = AvfxEmitter {
            create_count: curve(2.0, 2.0, 1),
            create_count_random: curve(100.0, 100.0, 1),
            create_interval: curve(1.0, 1.0, 1),
            loop_start: 2,
            loop_end: 7,
            ..Default::default()
        };
        emitter.create_count_random.random_type = 1;
        let item = AvfxEmitterItem {
            create_time: 0,
            create_probability: 100,
            by_injection_angle: [0.0, 0.0, 0.01],
            ..Default::default()
        };
        let clock = InstanceClock::root(&emitter, -1.0);
        let seed = (0..100)
            .find(|&seed| {
                super::super::emitter_create_count_at_seeded(&emitter, &item, 0.0, 0.0, 0, seed)
                    != 2
            })
            .unwrap();
        let count =
            super::super::emitter_create_count_at_seeded(&emitter, &item, 0.0, 0.0, 0, seed);
        assert!(constant_create_count(&emitter, &item));
        assert_eq!(
            accepted_creations_between(
                &emitter,
                clock,
                &item,
                0,
                1_000_000,
                1_000_000.0,
                seed,
                |event, copy| SplitMix64::seeded(0, event, copy, 0),
            ),
            1_000_000 * count
        );

        let mut counter = InjectionCounter::default();
        counter.begin_event(
            &emitter,
            clock,
            &item,
            1_000_000,
            1_000_000.0,
            seed,
            |event, copy| SplitMix64::seeded(0, event, copy, 0),
        );
        assert_eq!(
            counter.take_angle(&item)[2],
            0.01 * f32::from((1_000_000 * count) as i16)
        );
    }

    #[test]
    fn duplicate_last_count_keys_do_not_fast_forward_at_the_key_time() {
        let mut emitter = AvfxEmitter {
            create_count: curve(1.0, 2.0, 4),
            create_interval: curve(1.0, 1.0, 1),
            ..Default::default()
        };
        emitter.create_count.keys.push(AvfxCurveKey {
            time: 4,
            z: 3.0,
            ..emitter.create_count.keys[0]
        });
        let item = AvfxEmitterItem {
            create_time: 0,
            create_probability: 100,
            ..Default::default()
        };
        let counts = (0..10)
            .map(|event| {
                super::super::emitter_create_count_at_seeded(
                    &emitter,
                    &item,
                    event as f32,
                    event as f32,
                    event,
                    0,
                )
            })
            .collect::<Vec<_>>();
        assert_ne!(counts[4], counts[5]);
        assert_eq!(
            accepted_creations_between(
                &emitter,
                InstanceClock::root(&emitter, -1.0),
                &item,
                0,
                10,
                10.0,
                0,
                |event, copy| SplitMix64::seeded(0, event, copy, 0),
            ),
            counts.iter().sum::<u64>()
        );
    }
}
