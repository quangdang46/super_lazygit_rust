// Ported from ./references/lazygit-master/pkg/gui/types/search_state.go

#[derive(Clone, Copy)]
pub enum SearchType {
    None,
    Search,
    Filter,
}

pub struct SearchState {
    pub context: Context,
    pub prev_search_index: i32,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            context: Context,
            prev_search_index: -1,
        }
    }

    pub fn search_type(&self) -> SearchType {
        SearchType::None
    }
}

pub struct Context;
