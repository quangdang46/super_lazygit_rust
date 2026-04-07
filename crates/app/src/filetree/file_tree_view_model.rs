// Ported from ./references/lazygit-master/pkg/gui/filetree/file_tree_view_model.go

pub struct FileTreeViewModel {
    file_tree: FileTree,
    search_history: Vec<String>,
}

impl FileTreeViewModel {
    pub fn new(_get_files: fn() -> Vec<File>, _show_tree: bool) -> Self {
        Self {
            file_tree: FileTree::new(_get_files, _show_tree),
            search_history: Vec::new(),
        }
    }

    pub fn get_selected(&self) -> Option<FileNode> {
        None
    }

    pub fn get_selected_item_id(&self) -> String {
        String::new()
    }

    pub fn get_selected_items(&self) -> (Vec<FileNode>, i32, i32) {
        (Vec::new(), 0, 0)
    }

    pub fn get_selected_file(&self) -> Option<File> {
        None
    }

    pub fn get_selected_path(&self) -> String {
        String::new()
    }

    pub fn set_tree(&mut self) {}

    fn find_new_selected_idx(&self, _prev_nodes: &[FileNode], _curr_nodes: &[FileNode]) -> i32 {
        -1
    }

    pub fn set_status_filter(&mut self, _filter: FileTreeDisplayFilter) {}

    pub fn toggle_show_tree(&mut self) {}

    pub fn collapse_all(&mut self) {}

    pub fn expand_all(&mut self) {}

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

pub struct FileTree;

impl FileTree {
    pub fn new(_get_files: fn() -> Vec<File>, _show_tree: bool) -> Self {
        Self
    }
}
pub struct FileNode;
pub struct File;
pub enum FileTreeDisplayFilter {
    None,
    Status,
}
