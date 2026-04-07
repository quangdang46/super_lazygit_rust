// Ported from ./references/lazygit-master/pkg/gui/context/suggestions_context.go

use crate::types::common::HasUrn;
use crate::types::context::{Context, IBaseContext, IList, IListContext, IListPanelState, OnFocusOpts, OnFocusLostOpts};
use crate::types::suggestion::Suggestion;

pub struct SuggestionsContext {
    pub key: String,
    state: SuggestionsContextState,
}

pub struct SuggestionsContextState {
    pub suggestions: Vec<Suggestion>,
    pub on_confirm: Option<fn() -> Result<(), String>>,
    pub on_close: Option<fn() -> Result<(), String>>,
    pub on_delete_suggestion: Option<fn() -> Result<(), String>>,
    pub allow_edit_suggestion: bool,
    pub find_suggestions: Option<fn(String) -> Vec<Suggestion>>,
}

impl SuggestionsContext {
    pub fn new() -> Self {
        Self {
            key: "SUGGESTIONS_CONTEXT_KEY".to_string(),
            state: SuggestionsContextState {
                suggestions: vec![],
                on_confirm: None,
                on_close: None,
                on_delete_suggestion: None,
                allow_edit_suggestion: false,
                find_suggestions: None,
            },
        }
    }

    pub fn set_suggestions(&mut self, suggestions: Vec<Suggestion>) {
        self.state.suggestions = suggestions;
        self.set_selection(0);
    }

    pub fn get_on_double_click(&self) -> Option<fn() -> Result<(), String>> {
        self.state.on_confirm
    }

    pub fn range_select_enabled(&self) -> bool {
        false
    }
}

impl Default for SuggestionsContext {
    fn default() -> Self {
        Self::new()
    }
}

impl IBaseContext for SuggestionsContext {
    fn get_kind(&self) -> crate::types::context::ContextKind {
        crate::types::context::ContextKind::PersistentPopup
    }

    fn get_view_name(&self) -> &str {
        ""
    }

    fn get_window_name(&self) -> &str {
        ""
    }

    fn set_window_name(&mut self, _name: &str) {}

    fn get_key(&self) -> crate::types::context::ContextKey {
        crate::types::context::ContextKey(self.key.clone())
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn is_transient(&self) -> bool {
        false
    }

    fn has_controlled_bounds(&self) -> bool {
        true
    }

    fn total_content_height(&self) -> i32 {
        0
    }

    fn needs_rerender_on_width_change(&self) -> crate::types::context::NeedsRerenderOnWidthChangeLevel {
        crate::types::context::NeedsRerenderOnWidthChangeLevel::None
    }

    fn needs_rerender_on_height_change(&self) -> bool {
        false
    }

    fn title(&self) -> &str {
        ""
    }
}

impl Context for SuggestionsContext {
    fn handle_focus(&mut self, _opts: OnFocusOpts) {}
    fn handle_focus_lost(&mut self, _opts: OnFocusLostOpts) {}
    fn focus_line(&mut self, _scroll_into_view: bool) {}
    fn handle_render(&mut self) {}
}

impl IListContext for SuggestionsContext {
    fn get_selected_item_id(&self) -> String {
        String::new()
    }

    fn get_selected_item_ids(&self) -> (Vec<String>, usize, usize) {
        (vec![], 0, 0)
    }

    fn is_item_visible(&self, _item: &dyn HasUrn) -> bool {
        true
    }

    fn get_list(&self) -> &dyn IList {
        self
    }

    fn view_index_to_model_index(&self, _idx: i32) -> i32 {
        0
    }

    fn model_index_to_view_index(&self, _idx: i32) -> i32 {
        0
    }

    fn is_list_context(&self) {}

    fn range_select_enabled(&self) -> bool {
        false
    }

    fn render_only_visible_lines(&self) -> bool {
        false
    }
}

impl IListPanelState for SuggestionsContext {
    fn set_selected_line_idx(&mut self, _idx: i32) {}

    fn set_selection(&mut self, _value: i32) {}

    fn get_selected_line_idx(&self) -> i32 {
        0
    }
}

impl IList for SuggestionsContext {
    fn len(&self) -> usize {
        0
    }

    fn get_item(&self, _index: usize) -> &dyn HasUrn {
        self
    }
}

impl HasUrn for SuggestionsContext {
    fn urn(&self) -> String {
        self.key.clone()
    }
}

impl crate::types::context::ParentContexter for SuggestionsContext {
    fn set_parent_context(&mut self, _ctx: ()) {}
    fn get_parent_context(&self) -> Option<()> {
        None
    }
}
