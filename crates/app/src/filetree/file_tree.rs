// Ported from ./references/lazygit-master/pkg/gui/filetree/file_tree.go

use crate::filetree::build_tree::{build_flat_tree_from_files, build_tree_from_files};
use crate::filetree::collapsed_paths::CollapsedPaths;
use crate::filetree::file_filter::File;
use crate::filetree::file_node::FileNode;
use crate::filetree::node::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeDisplayFilter {
    DisplayAll,
    DisplayStaged,
    DisplayUnstaged,
    DisplayTracked,
    DisplayUntracked,
    DisplayConflicted,
}

pub trait ITree<T> {
    fn in_tree_mode(&self) -> bool;
    fn expand_to_path(&mut self, path: String);
    fn toggle_show_tree(&mut self);
    fn get_index_for_path(&self, path: &str) -> (usize, bool);
    fn len(&self) -> usize;
    fn set_tree(&mut self);
    fn is_collapsed(&self, path: &str) -> bool;
    fn toggle_collapsed(&mut self, path: String);
    fn collapsed_paths(&self) -> &CollapsedPaths;
    fn collapse_all(&mut self);
    fn expand_all(&mut self);
    fn get_visual_depth(&self, index: usize) -> i32;
}

pub trait IFileTree {
    fn filter_files(&self, test: impl Fn(&File) -> bool) -> Vec<&File>;
    fn set_status_filter(&mut self, filter: FileTreeDisplayFilter);
    fn force_show_untracked(&self) -> bool;
    fn get(&self, index: usize) -> Option<FileNode>;
    fn get_file(&self, path: &str) -> Option<&File>;
    fn get_all_items(&self) -> Vec<FileNode>;
    fn get_all_files(&self) -> Vec<&File>;
    fn get_status_filter(&self) -> FileTreeDisplayFilter;
    fn get_root(&self) -> Option<FileNode>;
    fn set_text_filter(&mut self, filter: String, use_fuzzy_search: bool);
    fn get_text_filter(&self) -> &str;
}

pub struct FileTree {
    get_files: fn() -> Vec<File>,
    tree: Option<Node<File>>,
    show_tree: bool,
    filter: FileTreeDisplayFilter,
    collapsed_paths: CollapsedPaths,
    text_filter: String,
    use_fuzzy_search: bool,
}

impl FileTree {
    pub fn new(get_files: fn() -> Vec<File>, show_tree: bool) -> Self {
        Self {
            get_files,
            tree: None,
            show_tree,
            filter: FileTreeDisplayFilter::DisplayAll,
            collapsed_paths: CollapsedPaths::new(),
            text_filter: String::new(),
            use_fuzzy_search: false,
        }
    }

    pub fn in_tree_mode(&self) -> bool {
        self.show_tree
    }

    pub fn expand_to_path(&mut self, path: String) {
        self.collapsed_paths.expand_to_path(path);
    }

    fn get_files_for_display(&self) -> Vec<File> {
        let files = (self.get_files)();

        let filtered = match self.filter {
            FileTreeDisplayFilter::DisplayAll => files,
            FileTreeDisplayFilter::DisplayStaged => {
                self.filter_files(files, |file| file.has_staged_changes)
            }
            FileTreeDisplayFilter::DisplayUnstaged => {
                self.filter_files(files, |file| file.has_unstaged_changes)
            }
            FileTreeDisplayFilter::DisplayTracked => {
                self.filter_files(files, |file| file.tracked || file.has_staged_changes)
            }
            FileTreeDisplayFilter::DisplayUntracked => {
                self.filter_files(files, |file| !(file.tracked || file.has_staged_changes))
            }
            FileTreeDisplayFilter::DisplayConflicted => {
                self.filter_files(files, |file| file.has_merge_conflicts)
            }
        };

        if self.text_filter.is_empty() {
            filtered
        } else {
            filter_files_by_text(&filtered, &self.text_filter, self.use_fuzzy_search)
        }
    }

    pub fn force_show_untracked(&self) -> bool {
        self.filter == FileTreeDisplayFilter::DisplayUntracked
    }

    pub fn filter_files<'a>(&self, files: Vec<File>, test: impl Fn(&File) -> bool) -> Vec<File> {
        files.into_iter().filter(|file| test(&file)).collect()
    }

    pub fn set_status_filter(&mut self, filter: FileTreeDisplayFilter) {
        self.filter = filter;
        self.set_tree();
    }

    pub fn toggle_show_tree(&mut self) {
        self.show_tree = !self.show_tree;
        self.set_tree();
    }

    pub fn get(&self, index: usize) -> Option<FileNode> {
        self.tree.as_ref()?;
        let tree = self.tree.as_ref().unwrap();
        let node = tree.get_node_at_index(index + 1, &self.collapsed_paths);
        node.map(|n| FileNode::from((*n).clone()))
    }

    pub fn get_file(&self, path: &str) -> Option<File> {
        let files = (self.get_files)();
        files.into_iter().find(|f| f.path == path)
    }

    pub fn get_index_for_path(&self, path: &str) -> (usize, bool) {
        let tree = match self.tree.as_ref() {
            Some(t) => t,
            None => return (0, false),
        };
        let (index, found) = tree.get_index_for_path(path, &self.collapsed_paths);
        if found {
            (index.saturating_sub(1), true)
        } else {
            (0, false)
        }
    }

    pub fn get_all_items(&self) -> Vec<FileNode> {
        let tree = match self.tree.as_ref() {
            Some(t) => t,
            None => return Vec::new(),
        };
        let flattened = tree.flatten(&self.collapsed_paths);
        flattened
            .into_iter()
            .skip(1)
            .map(|n| FileNode::from((*n).clone()))
            .collect()
    }

    pub fn len(&self) -> usize {
        match self.tree.as_ref() {
            Some(tree) => tree.size(&self.collapsed_paths).saturating_sub(1),
            None => 0,
        }
    }

    pub fn get_all_files(&self) -> Vec<File> {
        (self.get_files)()
    }

    pub fn set_tree(&mut self) {
        let files_for_display = self.get_files_for_display();
        let show_root_item = true;
        self.tree = if self.show_tree {
            Some(build_tree_from_files(&files_for_display, show_root_item))
        } else {
            Some(build_flat_tree_from_files(
                &files_for_display,
                show_root_item,
            ))
        };
    }

    pub fn is_collapsed(&self, path: &str) -> bool {
        self.collapsed_paths.is_collapsed(path)
    }

    pub fn toggle_collapsed(&mut self, path: String) {
        self.collapsed_paths.toggle_collapsed(path);
    }

    pub fn collapse_all(&mut self) {
        let dir_paths: Vec<String> = self
            .get_all_items()
            .iter()
            .filter(|f| !f.get_is_file())
            .map(|f| f.raw().get_internal_path().to_string())
            .collect();

        for path in dir_paths {
            self.collapsed_paths.collapse(path);
        }
    }

    pub fn expand_all(&mut self) {
        self.collapsed_paths.expand_all();
    }

    pub fn tree(&self) -> Option<FileNode> {
        self.tree.as_ref().map(|t| FileNode::from(t.clone()))
    }

    pub fn get_root(&self) -> Option<FileNode> {
        self.tree.as_ref().map(|t| FileNode::from(t.clone()))
    }

    pub fn collapsed_paths(&self) -> &CollapsedPaths {
        &self.collapsed_paths
    }

    pub fn get_visual_depth(&self, index: usize) -> i32 {
        match self.tree.as_ref() {
            Some(tree) => tree.get_visual_depth_at_index(index + 1, &self.collapsed_paths),
            None => 0,
        }
    }

    pub fn get_status_filter(&self) -> FileTreeDisplayFilter {
        self.filter
    }

    pub fn set_text_filter(&mut self, filter: String, use_fuzzy_search: bool) {
        self.text_filter = filter;
        self.use_fuzzy_search = use_fuzzy_search;
        self.set_tree();
    }

    pub fn get_text_filter(&self) -> &str {
        &self.text_filter
    }
}

fn filter_files_by_text(files: &[File], text_filter: &str, use_fuzzy_search: bool) -> Vec<File> {
    if use_fuzzy_search {
        files
            .iter()
            .filter(|f| fuzzy_contains(&f.path, text_filter))
            .cloned()
            .collect()
    } else {
        let lower_filter = text_filter.to_lowercase();
        files
            .iter()
            .filter(|f| f.path.to_lowercase().contains(&lower_filter))
            .cloned()
            .collect()
    }
}

fn fuzzy_contains(s: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars();
    for c in s.chars() {
        if let Some(pc) = pattern_chars.next() {
            if c.to_lowercase().next() == Some(pc.to_lowercase().next().unwrap_or(c)) {
                continue;
            }
        }
    }
    pattern_chars.next().is_none()
}
