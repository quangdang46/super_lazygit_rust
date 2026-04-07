// Ported from ./references/lazygit-master/pkg/gui/filetree/commit_file_tree.go

use crate::filetree::collapsed_paths::CollapsedPaths;
use crate::filetree::commit_file_node::CommitFileNode;

pub struct CommitFileTree {
    get_files: fn() -> Vec<CommitFileNode>,
    tree: Option<CommitFileNode>,
    show_tree: bool,
    collapsed_paths: CollapsedPaths,
    text_filter: String,
    use_fuzzy_search: bool,
}

impl CommitFileTree {
    pub fn new(_get_files: fn() -> Vec<CommitFileNode>, _show_tree: bool) -> Self {
        Self {
            get_files: _get_files,
            tree: None,
            show_tree: _show_tree,
            collapsed_paths: CollapsedPaths::new(),
            text_filter: String::new(),
            use_fuzzy_search: false,
        }
    }

    pub fn collapse_all(&mut self) {}

    pub fn expand_all(&mut self) {
        self.collapsed_paths.expand_all();
    }

    pub fn expand_to_path(&mut self, _path: String) {}

    pub fn toggle_show_tree(&mut self) {
        self.show_tree = !self.show_tree;
    }

    pub fn get(&self, _index: i32) -> Option<CommitFileNode> {
        None
    }

    pub fn get_index_for_path(&self, _path: String) -> (i32, bool) {
        (0, false)
    }

    pub fn get_all_items(&self) -> Vec<CommitFileNode> {
        Vec::new()
    }

    pub fn len(&self) -> i32 {
        0
    }

    pub fn get_all_files(&self) -> Vec<CommitFile> {
        Vec::new()
    }

    pub fn set_text_filter(&mut self, _filter: String, _use_fuzzy_search: bool) {}

    pub fn get_text_filter(&self) -> String {
        String::new()
    }

    pub fn is_collapsed(&self, _path: &String) -> bool {
        false
    }

    pub fn toggle_collapsed(&mut self, _path: String) {}

    pub fn get_root(&self) -> Option<CommitFileNode> {
        None
    }

    pub fn collapsed_paths(&self) -> &CollapsedPaths {
        &self.collapsed_paths
    }

    pub fn get_file(&self, _path: &String) -> Option<CommitFile> {
        None
    }

    pub fn get_visual_depth(&self, _index: i32) -> i32 {
        0
    }

    pub fn in_tree_mode(&self) -> bool {
        self.show_tree
    }
}

pub struct CommitFile;
