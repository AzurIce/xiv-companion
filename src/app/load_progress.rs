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

thread_local! {
    static CRAFT_DATA_PROGRESS_SINK: RefCell<Option<Box<dyn FnMut(Option<CraftDataLoadProgress>)>>> = RefCell::new(None);
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
