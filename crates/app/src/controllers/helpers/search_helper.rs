// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/search_helper.go

use std::sync::Mutex;

pub struct SearchHelper {
    common: HelperCommon,
}

pub struct HelperCommon;

#[derive(Clone, Copy, PartialEq)]
pub enum SearchType {
    Filter,
    Search,
    None,
}

pub struct SearchState {
    pub prev_search_index: i32,
    pub context: Option<Box<dyn Context>>,
    pub search_type: SearchType,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            prev_search_index: -1,
            context: None,
            search_type: SearchType::None,
        }
    }
}

pub trait Context {
    fn get_key(&self) -> &str;
}

pub trait FilterableContext: Context {
    fn get_filter(&self) -> String;
    fn set_filter(&mut self, _filter: &str, _fuzzy: bool) {}
    fn clear_filter(&mut self) {}
    fn is_filtering(&self) -> bool;
    fn re_apply_filter(&mut self, _fuzzy: bool) {}
}

pub trait SearchableContext: Context {
    fn get_search_string(&self) -> String;
    fn set_search_string(&mut self, _s: &str) {}
    fn clear_search_string(&mut self) {}
    fn is_searching(&self) -> bool;
    fn model_search_results(
        &self,
        _search_str: &str,
        _case_sensitive: bool,
    ) -> Vec<SearchPosition> {
        Vec::new()
    }
}

pub struct SearchPosition {
    pub x: i32,
    pub y: i32,
}

impl SearchHelper {
    pub fn new(common: HelperCommon) -> Self {
        Self { common }
    }

    pub fn open_filter_prompt(&self, _context: &dyn FilterableContext) -> Result<(), String> {
        Ok(())
    }

    pub fn open_search_prompt(&self, _context: &dyn SearchableContext) -> Result<(), String> {
        Ok(())
    }

    pub fn display_filter_status(&self, _context: &dyn FilterableContext) {}

    pub fn display_search_status(&self, _context: &dyn SearchableContext) {}

    fn search_state(&self) -> SearchState {
        SearchState::new()
    }

    pub fn confirm(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn confirm_filter(&self) {}

    pub fn confirm_search(&self) {}

    pub fn cancel_prompt(&self) -> Result<(), String> {
        self.cancel()
    }

    pub fn scroll_history(&self, _scroll_increment: i32) {}

    pub fn cancel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn on_prompt_content_changed(&self, _search_string: &str) {}

    pub fn re_apply_filter(&self, _context: &dyn Context) {}

    pub fn re_apply_search(&self, _ctx: &dyn Context) {}

    pub fn render_search_status(&self, _c: &dyn Context) {}

    pub fn cancel_search_if_searching(&self, _c: &dyn Context) {}

    pub fn hide_prompt(&self) {}

    fn set_searching_frame_color(&self) {}

    fn set_non_searching_frame_color(&self) {}
}

pub struct View;

impl View {
    pub fn set_content(&self, _content: &str) {}
    pub fn clear_text_area(&self) {}
    pub fn render_text_area(&self) {}
    pub fn set_origin_y(&self, _y: i32) {}
    pub fn search(&self, _s: &str, _positions: Vec<SearchPosition>) {}
    pub fn clear_search(&self) {}
    pub fn update_search_results(&self, _s: &str, _positions: Vec<SearchPosition>) {}
}

pub struct History;

impl History {
    pub fn push(&self, _s: &str) {}
    pub fn peek_at(&self, _i: i32) -> Option<String> {
        None
    }
}
