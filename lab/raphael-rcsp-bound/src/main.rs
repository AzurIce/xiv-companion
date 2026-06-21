use std::{collections::HashMap, time::Instant};

use raphael_sim::{Action, ActionMask, Condition, Settings, SimulationState};
use raphael_solver::{AtomicFlag, MacroSolver, SolverSettings};

const ACTIONS: &[Action] = &[
    Action::Veneration,
    Action::Innovation,
    Action::WasteNot,
    Action::BasicSynthesis,
    Action::CarefulSynthesis,
    Action::BasicTouch,
    Action::StandardTouch,
    Action::MasterMend,
];

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    max_cp: u16,
    max_durability: u16,
    target_progress: u16,
    target_quality: u16,
    base_progress: u16,
    base_quality: u16,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "baseline",
        max_cp: 320,
        max_durability: 70,
        target_progress: 200,
        target_quality: 180,
        base_progress: 20,
        base_quality: 20,
    },
    Scenario {
        name: "higher_quality",
        max_cp: 360,
        max_durability: 70,
        target_progress: 200,
        target_quality: 220,
        base_progress: 20,
        base_quality: 20,
    },
    Scenario {
        name: "tighter_progress",
        max_cp: 320,
        max_durability: 70,
        target_progress: 230,
        target_quality: 180,
        base_progress: 20,
        base_quality: 20,
    },
];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Key {
    progress: u16,
    quality: u16,
    effects: u64,
}

#[derive(Clone, Copy, Debug)]
struct Label {
    progress: u16,
    quality: u16,
    cp: u16,
    durability: i16,
}

#[derive(Clone, Debug)]
struct Node {
    state: SimulationState,
}

#[derive(Debug)]
struct Stats {
    found: Option<usize>,
    expanded: u64,
    generated: u64,
    pruned_bound: u64,
    sizes: Vec<usize>,
}

fn settings(scenario: Scenario) -> Settings {
    let allowed_actions = ACTIONS
        .iter()
        .fold(ActionMask::none(), |mask, action| mask.add(*action));
    Settings {
        max_cp: scenario.max_cp,
        max_durability: scenario.max_durability,
        max_progress: scenario.target_progress,
        max_quality: scenario.target_quality,
        base_progress: scenario.base_progress,
        base_quality: scenario.base_quality,
        job_level: 100,
        allowed_actions,
        adversarial: false,
        backload_progress: false,
        stellar_steady_hand_charges: 0,
    }
}

fn run_raphael(settings: Settings, label: &str, initial_solution: Option<&[Action]>) -> Option<Vec<Action>> {
    let solver_settings = SolverSettings {
        simulator_settings: settings,
        allow_non_max_quality_solutions: false,
    };
    let mut solver = MacroSolver::new(
        solver_settings,
        Box::new(|_| {}),
        Box::new(|_| {}),
        AtomicFlag::default(),
    );
    let started_at = Instant::now();
    let result = match initial_solution {
        Some(actions) => solver.solve_with_initial_solution(actions),
        None => solver.solve(),
    };
    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(actions) => {
            let stats = solver.runtime_stats();
            println!(
                "{label}_solution_steps={},elapsed_ms={elapsed_ms:.3},actions={}",
                actions.len(),
                actions
                    .iter()
                    .map(|action| action_name(*action))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!("{label}_stats={stats:#?}");
            Some(actions)
        }
        Err(err) => {
            println!("{label}_error={err:?},elapsed_ms={elapsed_ms:.3}");
            println!("{label}_stats={:#?}", solver.runtime_stats());
            None
        }
    }
}

fn goal(settings: &Settings, state: &SimulationState) -> bool {
    state.progress >= settings.max_progress && state.quality >= settings.max_quality
}

fn key(settings: &Settings, state: &SimulationState) -> Key {
    Key {
        progress: state.progress.min(settings.max_progress),
        quality: state.quality.min(settings.max_quality),
        effects: state.effects.into_bits(),
    }
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::BasicSynthesis => "BasicSynthesis",
        Action::BasicTouch => "BasicTouch",
        Action::MasterMend => "MasterMend",
        Action::Observe => "Observe",
        Action::TricksOfTheTrade => "TricksOfTheTrade",
        Action::WasteNot => "WasteNot",
        Action::Veneration => "Veneration",
        Action::StandardTouch => "StandardTouch",
        Action::GreatStrides => "GreatStrides",
        Action::Innovation => "Innovation",
        Action::WasteNot2 => "WasteNot2",
        Action::ByregotsBlessing => "ByregotsBlessing",
        Action::PreciseTouch => "PreciseTouch",
        Action::MuscleMemory => "MuscleMemory",
        Action::CarefulSynthesis => "CarefulSynthesis",
        Action::Manipulation => "Manipulation",
        Action::PrudentTouch => "PrudentTouch",
        Action::AdvancedTouch => "AdvancedTouch",
        Action::Reflect => "Reflect",
        Action::PreparatoryTouch => "PreparatoryTouch",
        Action::Groundwork => "Groundwork",
        Action::DelicateSynthesis => "DelicateSynthesis",
        Action::IntensiveSynthesis => "IntensiveSynthesis",
        Action::TrainedEye => "TrainedEye",
        Action::HeartAndSoul => "HeartAndSoul",
        Action::PrudentSynthesis => "PrudentSynthesis",
        Action::TrainedFinesse => "TrainedFinesse",
        Action::RefinedTouch => "RefinedTouch",
        Action::QuickInnovation => "QuickInnovation",
        Action::ImmaculateMend => "ImmaculateMend",
        Action::TrainedPerfection => "TrainedPerfection",
        Action::StellarSteadyHand => "StellarSteadyHand",
        Action::RapidSynthesis => "RapidSynthesis",
        Action::HastyTouch => "HastyTouch",
        Action::DaringTouch => "DaringTouch",
    }
}

fn simulate_macro(settings: &Settings, actions: &[Action]) -> Option<SimulationState> {
    let mut state = SimulationState::new(settings);
    for action in actions {
        state = state.use_action(*action, Condition::Normal, settings).ok()?;
    }
    Some(state)
}

fn find_template_incumbent(settings: &Settings) -> Option<Vec<Action>> {
    let templates = [
        vec![
            Action::StandardTouch,
            Action::Innovation,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::WasteNot,
            Action::Veneration,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
        ],
        vec![
            Action::StandardTouch,
            Action::BasicTouch,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::StandardTouch,
            Action::MasterMend,
            Action::Veneration,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
        ],
        vec![
            Action::BasicTouch,
            Action::Innovation,
            Action::BasicTouch,
            Action::StandardTouch,
            Action::BasicTouch,
            Action::StandardTouch,
            Action::WasteNot,
            Action::Veneration,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
            Action::CarefulSynthesis,
        ],
    ];

    templates.into_iter().find(|actions| {
        simulate_macro(settings, actions)
            .is_some_and(|state| state.progress >= settings.max_progress && state.quality >= settings.max_quality)
    })
}

fn insert_bucket(settings: &Settings, map: &mut HashMap<Key, Vec<Node>>, node: Node) {
    let key = key(settings, &node.state);
    let bucket = map.entry(key).or_default();
    for i in 0..bucket.len() {
        let other = bucket[i].state;
        if other.cp >= node.state.cp && other.durability >= node.state.durability {
            return;
        }
    }
    let mut i = 0;
    while i < bucket.len() {
        let other = bucket[i].state;
        if node.state.cp >= other.cp && node.state.durability >= other.durability {
            bucket.swap_remove(i);
        } else {
            i += 1;
        }
    }
    bucket.push(node);
}

fn compact(settings: &Settings, nodes: Vec<Node>) -> Vec<Node> {
    let mut map = HashMap::new();
    for node in nodes {
        insert_bucket(settings, &mut map, node);
    }
    map.into_values().flatten().collect()
}

fn dominates(a: Label, b: Label) -> bool {
    a.progress >= b.progress
        && a.quality >= b.quality
        && a.cp <= b.cp
        && a.durability <= b.durability
}

fn prune_labels(settings: &Settings, labels: Vec<Label>) -> Vec<Label> {
    let mut out: Vec<Label> = Vec::new();
    'outer: for mut label in labels {
        label.progress = label.progress.min(settings.max_progress);
        label.quality = label.quality.min(settings.max_quality);
        label.durability = label
            .durability
            .clamp(-(settings.max_durability as i16), settings.max_durability as i16);
        for i in 0..out.len() {
            if dominates(out[i], label) {
                continue 'outer;
            }
        }
        let mut i = 0;
        while i < out.len() {
            if dominates(label, out[i]) {
                out.swap_remove(i);
            } else {
                i += 1;
            }
        }
        out.push(label);
    }
    out
}

fn optimistic_action_labels(settings: &Settings) -> Vec<Label> {
    let best_progress_effect = 15u32; // Veneration active.
    let best_quality_effect = 500u32; // IQ 10 + Great Strides + Innovation.
    let normal_condition = 2u32;
    let progress = |action_mod: u32| {
        (u32::from(settings.base_progress) * action_mod * best_progress_effect / 1000) as u16
    };
    let quality = |action_mod: u32| {
        (u32::from(settings.base_quality) * action_mod * best_quality_effect * normal_condition
            / 20000) as u16
    };

    vec![
        Label {
            progress: progress(120),
            quality: 0,
            cp: 0,
            durability: 5,
        },
        Label {
            progress: progress(180),
            quality: 0,
            cp: 7,
            durability: 5,
        },
        Label {
            progress: 0,
            quality: quality(100),
            cp: 18,
            durability: 5,
        },
        Label {
            progress: 0,
            quality: quality(125),
            cp: 18,
            durability: 5,
        },
        Label {
            progress: 0,
            quality: 0,
            cp: 88,
            durability: -30,
        },
    ]
}

fn suffix_tables(settings: &Settings, max_steps: usize) -> Vec<Vec<Label>> {
    let action_labels = optimistic_action_labels(settings);
    let mut tables = vec![vec![Label {
        progress: 0,
        quality: 0,
        cp: 0,
        durability: 0,
    }]];
    for remaining in 1..=max_steps {
        let mut next = tables[remaining - 1].clone();
        for label in &tables[remaining - 1] {
            for action in &action_labels {
                next.push(Label {
                    progress: label.progress.saturating_add(action.progress),
                    quality: label.quality.saturating_add(action.quality),
                    cp: label.cp.saturating_add(action.cp),
                    durability: label.durability.saturating_add(action.durability),
                });
            }
        }
        tables.push(prune_labels(settings, next));
    }
    tables
}

fn can_complete_relaxed(
    settings: &Settings,
    state: &SimulationState,
    remaining: usize,
    tables: &[Vec<Label>],
) -> bool {
    let need_progress = settings.max_progress.saturating_sub(state.progress);
    let need_quality = settings.max_quality.saturating_sub(state.quality);
    if need_progress == 0 && need_quality == 0 {
        return true;
    }
    tables[remaining].iter().any(|label| {
        label.progress >= need_progress
            && label.quality >= need_quality
            && label.cp <= state.cp
            && label.durability <= state.durability as i16
    })
}

fn search(settings: &Settings, tables: &[Vec<Label>], max_steps: usize, use_bound: bool) -> Stats {
    let mut layer = vec![Node {
        state: SimulationState::new(settings),
    }];
    let mut stats = Stats {
        found: None,
        expanded: 0,
        generated: 0,
        pruned_bound: 0,
        sizes: Vec::new(),
    };

    for step in 0..=max_steps {
        stats.sizes.push(layer.len());
        if layer.iter().any(|node| goal(settings, &node.state)) {
            stats.found = Some(step);
            return stats;
        }
        if step == max_steps {
            break;
        }

        let mut next = Vec::new();
        for node in &layer {
            stats.expanded += 1;
            for action in ACTIONS {
                let Ok(next_state) = node
                    .state
                    .use_action(*action, Condition::Normal, settings)
                else {
                    continue;
                };
                stats.generated += 1;
                if use_bound
                    && !can_complete_relaxed(settings, &next_state, max_steps - step - 1, tables)
                {
                    stats.pruned_bound += 1;
                    continue;
                }
                next.push(Node { state: next_state });
            }
        }
        layer = compact(settings, next);
    }
    stats
}

fn main() {
    for scenario in SCENARIOS {
        let settings = settings(*scenario);
        println!("scenario={}", scenario.name);
        let incumbent_started_at = Instant::now();
        let template_incumbent = find_template_incumbent(&settings);
        let template_elapsed_us = incumbent_started_at.elapsed().as_secs_f64() * 1_000_000.0;
        let baseline_solution = run_raphael(settings, "raphael_baseline", None);
        let (incumbent, incumbent_source) = if let Some(actions) = template_incumbent {
            (actions, "template")
        } else if let Some(actions) = baseline_solution {
            (actions, "raphael_baseline")
        } else {
            println!("incumbent=none");
            continue;
        };
        let incumbent_len = incumbent.len();
        run_raphael(settings, "raphael_seeded", Some(&incumbent));
        println!(
            "incumbent_steps={},incumbent_source={},template_probe_us={template_elapsed_us:.3},incumbent={}",
            incumbent_len,
            incumbent_source,
            incumbent
                .iter()
                .map(|action| action_name(*action))
                .collect::<Vec<_>>()
                .join(",")
        );
        println!("budget,found,forward_expanded,bounded_expanded,bounded_generated,pruned_bound,expanded_ratio,suffix_last_size,pdb_ms,forward_ms,bounded_ms");
        for max_steps in incumbent_len.saturating_sub(1)..=incumbent_len + 4 {
            let pdb_started_at = Instant::now();
            let tables = suffix_tables(&settings, max_steps);
            let pdb_ms = pdb_started_at.elapsed().as_secs_f64() * 1000.0;
            let forward_started_at = Instant::now();
            let forward = search(&settings, &tables, max_steps, false);
            let forward_ms = forward_started_at.elapsed().as_secs_f64() * 1000.0;
            let bounded_started_at = Instant::now();
            let bounded = search(&settings, &tables, max_steps, true);
            let bounded_ms = bounded_started_at.elapsed().as_secs_f64() * 1000.0;
            let ratio = bounded.expanded as f64 / forward.expanded as f64;
            println!(
                "{},{},{},{},{},{},{:.3},{},{:.3},{:.3},{:.3}",
                max_steps,
                bounded.found.map_or(String::from("none"), |step| step.to_string()),
                forward.expanded,
                bounded.expanded,
                bounded.generated,
                bounded.pruned_bound,
                ratio,
                tables.last().map_or(0, Vec::len),
                pdb_ms,
                forward_ms,
                bounded_ms,
            );
        }
    }
}
