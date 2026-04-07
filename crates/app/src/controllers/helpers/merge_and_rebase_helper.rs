// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/merge_and_rebase_helper.go

pub enum MergeVariant {
    Regular,
    NonFastForward,
    FastForward,
    Squash,
}

pub struct RefreshOptions {
    pub mode: RefreshMode,
    pub scope: Vec<RefreshableView>,
}

pub enum RefreshMode {
    Sync,
    Async,
}

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

pub enum RebaseOption {
    Continue,
    Abort,
    Skip,
}

pub struct HelperCommon;

pub struct MergeAndRebaseHelper {
    context: HelperCommon,
}

impl MergeAndRebaseHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }

    pub fn create_rebase_options_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn continue_rebase(&self) -> Result<(), String> {
        self.generic_merge_command("continue")
    }

    pub fn generic_merge_command(&self, _command: &str) -> Result<(), String> {
        Ok(())
    }

    fn has_exec_todos(&self) -> bool {
        false
    }

    pub fn check_merge_or_rebase_with_refresh_options(
        &self,
        _result: Result<(), String>,
        _refresh_options: &RefreshOptions,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn check_merge_or_rebase(&self, _result: Result<(), String>) -> Result<(), String> {
        Ok(())
    }

    pub fn check_for_conflicts(&self, _result: Result<(), String>) -> Result<(), String> {
        Ok(())
    }

    pub fn prompt_for_conflict_handling(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn abort_merge_or_rebase_with_confirm(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prompt_to_continue_rebase(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn rebase_onto_ref(&self, _ref: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn merge_ref_into_checked_out_branch(&self, _ref_name: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn regular_merge(&self, _ref_name: &str, _variant: MergeVariant) -> Result<(), String> {
        Ok(())
    }

    pub fn squash_merge_uncommitted(&self, _ref_name: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn squash_merge_committed(
        &self,
        _ref_name: &str,
        _checked_out_branch_name: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    fn fast_forward_merge_user_preference(&self) -> (bool, bool) {
        (false, false)
    }

    pub fn reset_marked_base_commit(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Default for MergeAndRebaseHelper {
    fn default() -> Self {
        Self::new()
    }
}
