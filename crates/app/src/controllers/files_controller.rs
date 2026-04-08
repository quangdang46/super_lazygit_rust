// Ported from ./references/lazygit-master/pkg/gui/controllers/files_controller.go
use crate::controllers::ControllerCommon;


pub struct FilesController {
    context: ControllerCommon,
    list_trait: ListControllerTrait,
}

pub struct ListControllerTrait;

pub struct FileNode;

pub struct FileNodeChildren;

pub struct FileTreeViewModel;

impl FileTreeViewModel {
    pub fn collapse_all(&self) {}
    pub fn expand_all(&self) {}
    pub fn toggle_show_tree(&self) {}
    pub fn toggle_collapsed(&self, _path: &str) {}
    pub fn in_tree_mode(&self) -> bool {
        true
    }
    pub fn get_visual_depth(&self, _idx: usize) -> usize {
        0
    }
    pub fn get(&self, _idx: usize) -> Option<&FileNode> {
        None
    }
}

impl FileNode {
    pub fn file(&self) -> Option<&File> {
        None
    }
    pub fn get_path(&self) -> String {
        String::new()
    }
    pub fn get_internal_path(&self) -> String {
        String::new()
    }
    pub fn is_file(&self) -> bool {
        false
    }
    pub fn children(&self) -> &FileNodeChildren {
        &FileNodeChildren
    }
    pub fn get_has_unstaged_changes(&self) -> bool {
        false
    }
    pub fn get_has_staged_changes(&self) -> bool {
        false
    }
    pub fn get_has_inline_merge_conflicts(&self) -> bool {
        false
    }
    pub fn for_each_file<F>(&self, _f: F) -> Result<(), String>
    where
        F: FnMut(&File) -> Result<(), String>,
    {
        Ok(())
    }
}

impl FileNodeChildren {
    pub fn iter(&self) -> impl Iterator<Item = &FileNode> {
        std::iter::empty()
    }
}

pub struct File {
    pub path: String,
    pub name: String,
    pub short_status: String,
    pub tracked: bool,
    pub has_staged_changes: bool,
    pub has_merge_conflicts: bool,
    pub has_inline_merge_conflicts: bool,
}

impl File {
    pub fn names(&self) -> Vec<String> {
        vec![self.name.clone()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeDisplayFilter {
    DisplayAll,
    DisplayStaged,
    DisplayUnstaged,
    DisplayTracked,
    DisplayUntracked,
    DisplayConflicted,
}

impl FilesController {
    pub fn new(context: ControllerCommon) -> Self {
        Self {
            context,
            list_trait: ListControllerTrait,
        }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self) -> Vec<MouseBinding> {
        Vec::new()
    }

    pub fn context(&self) -> &WorkingTreeContext {
        &WorkingTreeContext
    }

    pub fn press(&self, _nodes: &[&FileNode]) -> Result<(), String> {
        Ok(())
    }

    pub fn enter(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn collapse_all(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn expand_all(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn toggle_tree_view(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn refresh(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn toggle_staged_all(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn stash(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn fetch(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn edit(&self, _nodes: &[&FileNode]) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(&self, _nodes: &[&FileNode]) -> Result<(), String> {
        Ok(())
    }

    pub fn open(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn open_diff_tool(&self, _node: &FileNode) -> Result<(), String> {
        Ok(())
    }

    pub fn ignore_or_exclude_menu(&self, _node: &FileNode) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_status_filter_pressed(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn create_stash_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn open_merge_conflict_menu(&self, _nodes: &[&FileNode]) -> Result<(), String> {
        Ok(())
    }

    pub fn open_copy_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_amend_commit_press(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_on_click(&self, _opts: ViewMouseBindingOpts) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_on_double_click(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct WorkingTreeContext;

impl WorkingTreeContext {
    pub fn file_tree_view_model(&self) -> &FileTreeViewModel {
        &FileTreeViewModel
    }
    pub fn get_selected(&self) -> Option<&FileNode> {
        None
    }
    pub fn get_selected_line_idx(&self) -> usize {
        0
    }
    pub fn get_view_name(&self) -> &str {
        "files"
    }
    pub fn get_status_filter(&self) -> FileTreeDisplayFilter {
        FileTreeDisplayFilter::DisplayAll
    }
    pub fn is_filtering(&self) -> bool {
        false
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}

pub struct MouseBinding {
    pub view_name: String,
}

pub struct ViewMouseBindingOpts {
    pub x: i64,
    pub y: i64,
}

pub struct RefreshOptions {
    pub mode: RefreshMode,
    pub scope: Vec<RefreshableView>,
}

#[derive(Debug, Clone)]
pub enum RefreshMode {
    Sync,
    Async,
}

#[derive(Debug, Clone)]
pub enum RefreshableView {
    Files,
    Branches,
    Commits,
    Stash,
    Remotes,
    Tags,
    Worktrees,
    Submodules,
}

pub struct MenuItem {
    pub label: String,
    pub on_press: Box<dyn Fn() -> Result<(), String>>,
    pub key: Option<char>,
}

pub struct CreateMenuOptions {
    pub title: String,
    pub items: Vec<MenuItem>,
}

pub struct TrStrings {
    pub stage: String,
    pub discard: String,
    pub file_filter: String,
    pub copy_to_clipboard_menu: String,
}

impl Default for TrStrings {
    fn default() -> Self {
        Self {
            stage: "Stage".to_string(),
            discard: "Discard".to_string(),
            file_filter: "File filter".to_string(),
            copy_to_clipboard_menu: "Copy to clipboard".to_string(),
        }
    }
}
