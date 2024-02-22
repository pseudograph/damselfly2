use crate::damselfly::memory::memory_update::MemoryUpdateType;

pub enum LowerPanelMode {
    SETTINGS,
    CALLSTACK,
    STATISTICS
}

pub struct AppDefaultState {
    pub block_size: usize,
    pub map_span: usize,
    pub current_block: Option<MemoryUpdateType>,
    pub lower_panel_mode: LowerPanelMode,
}

impl AppDefaultState {
    pub fn new(block_size: usize, map_span: usize, current_block: Option<MemoryUpdateType>) -> AppDefaultState {
        AppDefaultState {
            block_size,
            map_span,
            current_block,
            lower_panel_mode: LowerPanelMode::CALLSTACK
        }
    }
}
