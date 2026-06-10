use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use tsify::Tsify;

#[cfg(feature = "wasm")]
use crate::{CraftRecipe, RecipeLevelInfo, macro_action_key_by_game_action_id};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CrafterAttributes {
    pub level: u32,
    pub craftsmanship: u32,
    pub control: u32,
    pub craft_points: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct RaphaelSolveOptions {
    pub target_quality: Option<u32>,
    pub use_manipulation: bool,
    pub use_heart_and_soul: bool,
    pub use_quick_innovation: bool,
    pub use_trained_eye: bool,
    pub backload_progress: bool,
    pub adversarial: bool,
    pub stellar_steady_hand_charges: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MacroAction {
    pub id: String,
    pub wait_seconds: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MacroSolveResult {
    pub actions: Vec<MacroAction>,
    pub steps: usize,
    pub duration_seconds: u32,
    pub final_progress: u32,
    pub max_progress: u32,
    pub final_quality: u32,
    pub max_quality: u32,
    pub target_quality: u32,
    pub final_durability: u32,
    pub final_cp: u32,
}

#[cfg(feature = "wasm")]
pub fn solve_raphael_macro(
    recipe: &CraftRecipe,
    recipe_level: &RecipeLevelInfo,
    attrs: &CrafterAttributes,
    options: &RaphaelSolveOptions,
) -> Result<MacroSolveResult, String> {
    use raphael_simulator::{Action, ActionMask, Settings, SimulationState};
    use raphael_solver::{AtomicFlag, MacroSolver, SolverSettings};

    let progress_divider = recipe_level.progress_divider;
    let quality_divider = recipe_level.quality_divider;
    if progress_divider == 0 || quality_divider == 0 {
        return Err("配方等级数据缺少求解公式字段，请重新导出游戏数据。".to_string());
    }

    let mut allowed_actions = ActionMask::all();
    if !options.use_manipulation {
        allowed_actions = allowed_actions.remove(Action::Manipulation);
    }
    if !options.use_heart_and_soul {
        allowed_actions = allowed_actions.remove(Action::HeartAndSoul);
    }
    if !options.use_quick_innovation {
        allowed_actions = allowed_actions.remove(Action::QuickInnovation);
    }
    if !options.use_trained_eye
        || recipe.is_expert
        || attrs.level < recipe_level.class_job_level.saturating_add(10)
    {
        allowed_actions = allowed_actions.remove(Action::TrainedEye);
    }

    let max_progress = recipe_level.difficulty * recipe.difficulty_factor / 100;
    let max_quality = recipe_level.quality * recipe.quality_factor / 100;
    let max_durability = recipe_level.durability * recipe.durability_factor / 100;
    let target_quality = options
        .target_quality
        .unwrap_or(max_quality)
        .clamp(1, max_quality.max(1));

    let settings = Settings {
        max_cp: clamp_u16(attrs.craft_points),
        max_durability: clamp_u16(max_durability),
        max_progress: clamp_u16(max_progress),
        max_quality: clamp_u16(target_quality),
        base_progress: clamp_u16(base_progress(attrs, recipe_level)),
        base_quality: clamp_u16(base_quality(attrs, recipe_level)),
        job_level: attrs.level.min(u8::MAX as u32) as u8,
        allowed_actions,
        adversarial: options.adversarial,
        backload_progress: options.backload_progress,
        stellar_steady_hand_charges: options.stellar_steady_hand_charges,
    };

    let solver_settings = SolverSettings {
        simulator_settings: settings,
        allow_non_max_quality_solutions: true,
    };
    let mut solver = MacroSolver::new(
        solver_settings,
        Box::new(|_| {}),
        Box::new(|_| {}),
        AtomicFlag::new(),
    );
    let actions = solver.solve().map_err(|err| format!("{err:?}"))?;
    let final_state =
        SimulationState::from_macro(&settings, &actions).map_err(|err| format!("{err:?}"))?;
    let duration_seconds = actions
        .iter()
        .map(|action| u32::from(action.time_cost()))
        .sum();
    let actions = actions
        .into_iter()
        .map(|action| MacroAction {
            id: macro_action_key_by_game_action_id(action.action_id())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("action_{}", action.action_id())),
            wait_seconds: action.time_cost(),
        })
        .collect::<Vec<_>>();

    Ok(MacroSolveResult {
        steps: actions.len(),
        actions,
        duration_seconds,
        final_progress: u32::from(final_state.progress).min(max_progress),
        max_progress,
        final_quality: u32::from(final_state.quality).min(max_quality),
        max_quality,
        target_quality,
        final_durability: u32::from(final_state.durability),
        final_cp: u32::from(final_state.cp),
    })
}

#[cfg(feature = "wasm")]
fn base_progress(attrs: &CrafterAttributes, recipe_level: &RecipeLevelInfo) -> u32 {
    let mut value = attrs.craftsmanship as f32 * 10.0 / recipe_level.progress_divider as f32 + 2.0;
    if attrs.level <= recipe_level.class_job_level {
        value *= recipe_level.progress_modifier as f32 / 100.0;
    }
    value as u32
}

#[cfg(feature = "wasm")]
fn base_quality(attrs: &CrafterAttributes, recipe_level: &RecipeLevelInfo) -> u32 {
    let mut value = attrs.control as f32 * 10.0 / recipe_level.quality_divider as f32 + 35.0;
    if attrs.level <= recipe_level.class_job_level {
        value *= recipe_level.quality_modifier as f32 / 100.0;
    }
    value as u32
}

#[cfg(feature = "wasm")]
fn clamp_u16(value: u32) -> u16 {
    value.min(u16::MAX as u32) as u16
}
