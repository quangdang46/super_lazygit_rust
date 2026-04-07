// Ported from ./references/lazygit-master/pkg/gui/filetree/commit_file_tree_view_model.go

use crate::filetree::commit_file_node::CommitFileNode;
use crate::filetree::commit_file_tree::CommitFileTree;

pub struct CommitFileTreeViewModel {
    commit_file_tree: CommitFileTree,
    r#ref: Option<Ref>,
    ref_range: Option<RefRange>,
    can_rebase: bool,
}

impl CommitFileTreeViewModel {
    pub fn new(_get_files: fn() -> Vec<CommitFileNode>, _show_tree: bool) -> Self {
        Self {
            commit_file_tree: CommitFileTree::new(_get_files, _show_tree),
            r#ref: None,
            ref_range: None,
            can_rebase: false,
        }
    }

    pub fn get_ref(&self) -> Option<Ref> {
        self.r#ref.clone()
    }

    pub fn set_ref(&mut self, _ref: Ref) {}

    pub fn get_ref_range(&self) -> Option<RefRange> {
        self.ref_range.clone()
    }

    pub fn set_ref_range(&mut self, _ref_range: RefRange) {}

    pub fn get_can_rebase(&self) -> bool {
        self.can_rebase
    }

    pub fn set_can_rebase(&mut self, _can_rebase: bool) {}

    pub fn get_selected(&self) -> Option<CommitFileNode> {
        None
    }

    pub fn get_selected_item_id(&self) -> String {
        String::new()
    }

    pub fn get_selected_items(&self) -> (Vec<CommitFileNode>, i32, i32) {
        (Vec::new(), 0, 0)
    }

    pub fn get_selected_file(&self) -> Option<CommitFileNode> {
        None
    }

    pub fn get_selected_path(&self) -> String {
        String::new()
    }

    pub fn toggle_show_tree(&mut self) {}

    pub fn collapse_all(&mut self) {}

    pub fn expand_all(&mut self) {}

    pub fn select_path(&mut self, _filepath: String, _show_root_item: bool) {}

    pub fn set_filter(&mut self, _filter: String, _use_fuzzy_search: bool) {}

    pub fn get_filter(&self) -> String {
        String::new()
    }

    pub fn clear_filter(&mut self) {}

    pub fn re_apply_filter(&mut self, _use_fuzzy_search: bool) {}

    pub fn is_filtering(&self) -> bool {
        false
    }

    pub fn filter_prefix(&self) -> String {
        String::new()
    }
}

#[derive(Clone)]
pub struct Ref;

#[derive(Clone)]
pub struct RefRange;
