use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CraftDataLoadProgress {
    pub stage: String,
    pub detail: String,
    pub current: u32,
    pub total: u32,
    pub elapsed_ms: f64,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WeaponModelLoadProgress {
    pub item_id: u32,
    pub stain_ids: [u8; 2],
    pub stage: String,
    pub detail: String,
    pub checked_resources: u32,
    pub loaded_resources: u32,
    pub loaded_bytes: u64,
    pub elapsed_ms: f64,
    pub done: bool,
}

thread_local! {
    static CRAFT_DATA_PROGRESS_SINK: RefCell<Option<Box<dyn FnMut(Option<CraftDataLoadProgress>)>>> = RefCell::new(None);
    static WEAPON_MODEL_PROGRESS_SINK: RefCell<Option<Box<dyn FnMut(Option<WeaponModelLoadProgress>)>>> = RefCell::new(None);
}

pub(crate) fn set_craft_data_progress_sink(
    sink: impl FnMut(Option<CraftDataLoadProgress>) + 'static,
) {
    CRAFT_DATA_PROGRESS_SINK.with(|cell| {
        *cell.borrow_mut() = Some(Box::new(sink));
    });
}

pub(crate) fn clear_craft_data_progress() {
    report_craft_data_progress(None);
}

pub(crate) fn report_craft_data_progress(progress: Option<CraftDataLoadProgress>) {
    CRAFT_DATA_PROGRESS_SINK.with(|cell| {
        if let Some(sink) = cell.borrow_mut().as_mut() {
            sink(progress);
        }
    });
}

pub(crate) fn set_weapon_model_progress_sink(
    sink: impl FnMut(Option<WeaponModelLoadProgress>) + 'static,
) {
    WEAPON_MODEL_PROGRESS_SINK.with(|cell| {
        *cell.borrow_mut() = Some(Box::new(sink));
    });
}

pub(crate) fn clear_weapon_model_progress() {
    report_weapon_model_progress(None);
}

pub(crate) fn report_weapon_model_progress(progress: Option<WeaponModelLoadProgress>) {
    WEAPON_MODEL_PROGRESS_SINK.with(|cell| {
        if let Some(sink) = cell.borrow_mut().as_mut() {
            sink(progress);
        }
    });
}
