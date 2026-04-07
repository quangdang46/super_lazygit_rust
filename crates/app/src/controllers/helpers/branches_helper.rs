// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/branches_helper.go

pub struct Branch {
    pub name: String,
    pub upstream_branch: Option<String>,
    pub upstream_remote: Option<String>,
}

pub struct RemoteBranch {
    pub name: String,
    pub remote_name: String,
}

pub struct Worktree {
    pub name: String,
}

pub struct HelperCommon;

pub struct WorktreeHelper;

pub struct BranchesHelper {
    context: HelperCommon,
    worktree_helper: WorktreeHelper,
}

impl BranchesHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            worktree_helper: WorktreeHelper,
        }
    }

    pub fn confirm_local_delete(&self, _branches: &[Branch]) -> Result<(), String> {
        Ok(())
    }

    pub fn confirm_delete_remote(
        &self,
        _remote_branches: &[RemoteBranch],
        _reset_remote_branches_selection: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn confirm_local_and_remote_delete(&self, _branches: &[Branch]) -> Result<(), String> {
        Ok(())
    }

    pub fn checked_out_by_other_worktree(&self, _branch: &Branch) -> bool {
        false
    }

    pub fn worktree_for_branch(&self, _branch: &Branch) -> Option<&Worktree> {
        None
    }

    pub fn prompt_worktree_branch_delete(&self, _selected_branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn all_branches_merged(&self, _branches: &[Branch]) -> Result<bool, String> {
        Ok(true)
    }

    pub fn delete_remote_branches(&self, _remote_branches: &[RemoteBranch]) -> Result<(), String> {
        Ok(())
    }

    pub fn auto_forward_branches(&self) -> Result<(), String> {
        Ok(())
    }
}

pub fn short_branch_name(full_branch_name: &str) -> String {
    full_branch_name.to_string()
}

impl Default for BranchesHelper {
    fn default() -> Self {
        Self::new()
    }
}
