// Ported from ./references/lazygit-master/pkg/gui/context/worktrees_context.go

use crate::types::common::HasUrn;
use crate::types::context::{Context, IBaseContext, IList, IListContext, OnFocusOpts, OnFocusLostOpts};

pub struct WorktreesContext {
    pub key: String,
}

impl WorktreesContext {
    pub fn new() -> Self {
        Self {
            key: "WORKTREES_CONTEXT_KEY".to_string(),
        }
    }
}

impl Default for WorktreesContext {
    fn default() -> Self {
        Self::new()
    }
}

impl IBaseContext for WorktreesContext {
    fn get_kind(&self) -> crate::types::context::ContextKind {
        crate::types::context::ContextKind::SideContext
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
        false
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

impl Context for WorktreesContext {
    fn handle_focus(&mut self, _opts: OnFocusOpts) {}
    fn handle_focus_lost(&mut self, _opts: OnFocusLostOpts) {}
    fn focus_line(&mut self, _scroll_into_view: bool) {}
    fn handle_render(&mut self) {}
}

impl IListContext for WorktreesContext {
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
        true
    }

    fn render_only_visible_lines(&self) -> bool {
        false
    }
}

impl IList for WorktreesContext {
    fn len(&self) -> usize {
        0
    }

    fn get_item(&self, _index: usize) -> &dyn HasUrn {
        self
    }
}

impl HasUrn for WorktreesContext {
    fn urn(&self) -> String {
        self.key.clone()
    }
}

impl crate::types::context::ParentContexter for WorktreesContext {
    fn set_parent_context(&mut self, _ctx: ()) {}
    fn get_parent_context(&self) -> Option<()> {
        None
    }
}
