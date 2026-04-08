// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/refs_helper.go

use std::sync::Arc;

pub struct RefsHelper {
    common: HelperCommon,
    rebase_helper: Arc<MergeAndRebaseHelper>,
}

pub struct HelperCommon;
pub struct MergeAndRebaseHelper;

#[derive(Default)]
pub struct CheckoutRefOptions {
    pub waiting_status: String,
    pub env_vars: Vec<String>,
    pub on_ref_not_found: Option<Box<dyn Fn(String) -> Result<(), String> + Send + Sync>>,
}


pub struct Branch;
pub struct Commit;
pub struct MenuItem;
pub struct PromptOpts;
pub struct ConfirmOpts;
pub struct CreateMenuOptions;

impl RefsHelper {
    pub fn new(common: HelperCommon, rebase_helper: Arc<MergeAndRebaseHelper>) -> Self {
        Self {
            common,
            rebase_helper,
        }
    }

    pub fn select_first_branch_and_first_commit(&self) {}

    pub fn checkout_ref(&self, _ref: &str, _options: CheckoutRefOptions) -> Result<(), String> {
        Ok(())
    }

    pub fn checkout_remote_branch(
        &self,
        _full_branch_name: &str,
        _local_branch_name: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn checkout_previous_ref(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn get_checked_out_ref(&self) -> Option<Branch> {
        None
    }

    pub fn reset_to_ref(
        &self,
        _ref: &str,
        _strength: &str,
        _env_vars: Vec<String>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn create_sort_order_menu(
        &self,
        _sort_options_order: Vec<String>,
        _menu_prompt: &str,
        _on_selected: fn(String) -> Result<(), String>,
        _current_value: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn create_git_reset_menu(&self, _name: &str, _ref: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn create_checkout_menu(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn new_branch(
        &self,
        _from: &str,
        _from_formatted_name: &str,
        _suggested_branch_name: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn move_commits_to_new_branch(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn can_move_commits_to_new_branch(&self) -> Option<DisabledReason> {
        None
    }

    pub fn parse_remote_branch_name(&self, _full_branch_name: &str) -> (String, String, bool) {
        (String::new(), String::new(), false)
    }
}

pub struct DisabledReason {
    pub text: String,
}

pub fn sanitized_branch_name(input: &str) -> String {
    input.replace(' ', "-")
}

pub fn is_switch_branch_uncommitted_changes_error(err: &str) -> bool {
    err.contains("Please commit your changes or stash them before you switch branch")
}

pub fn short_branch_name(base_branch_ref: &str) -> String {
    base_branch_ref.to_string()
}

pub fn is_working_tree_dirty_except_submodules(_files: &[File], _submodules: &[Submodule]) -> bool {
    false
}

pub struct File;
pub struct Submodule;
pub struct Remote;
pub struct Stash;
pub struct Git;

impl Git {
    pub fn branch(&self) -> BranchCommands {
        BranchCommands
    }
    pub fn stash(&self) -> StashCommands {
        StashCommands
    }
    pub fn commit(&self) -> CommitCommands {
        CommitCommands
    }
    pub fn rebase(&self) -> RebaseCommands {
        RebaseCommands
    }
}

pub struct BranchCommands;
pub struct StashCommands;
pub struct CommitCommands;
pub struct RebaseCommands;

impl BranchCommands {
    pub fn checkout(&self, _name: &str, _options: CheckoutOptions) -> Result<(), String> {
        Ok(())
    }
    pub fn create_with_upstream(&self, _name: &str, _upstream: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn new(&self, _name: &str, _start_point: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn new_without_tracking(&self, _name: &str, _start_point: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn new_without_checkout(&self, _name: &str, _start_point: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn current_branch_name(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

impl StashCommands {
    pub fn push(&self, _message: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn pop(&self, _index: usize) -> Result<(), String> {
        Ok(())
    }
}

impl CommitCommands {
    pub fn reset_to_commit(
        &self,
        _ref: &str,
        _strength: &str,
        _env_vars: Vec<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl RebaseCommands {
    pub fn cherry_pick_commits(&self, _commits: &[Commit]) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CheckoutOptions {
    pub force: bool,
    pub env_vars: Vec<String>,
}

