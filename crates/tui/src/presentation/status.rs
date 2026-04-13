// Ported from ./references/lazygit-master/pkg/gui/presentation/status.go

use super::item_operations::ItemOperation;

pub struct StatusOptions<'a> {
    pub repo_name: &'a str,
    pub current_branch: &'a Branch,
    pub item_operation: ItemOperation,
    pub linked_worktree_name: &'a str,
    pub working_tree_state: WorkingTreeState,
}

pub struct Branch {
    pub name: String,
}

pub enum WorkingTreeState {
    Normal,
    Merging,
    Rebasing,
    CherryPicking,
    Checking,
}

impl WorkingTreeState {
    pub fn any(&self) -> bool {
        true
    }

    pub fn lower_case_title(&self) -> String {
        "normal".to_string()
    }
}

pub fn format_status(_opts: StatusOptions) -> String {
    String::new()
}
