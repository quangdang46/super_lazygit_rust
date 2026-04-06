// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/worktree_helper.go

pub struct WorktreeHelper {
    common: HelperCommon,
    repos_helper: ReposHelper,
    refs_helper: RefsHelper,
    suggestions_helper: SuggestionsHelper,
}

pub struct HelperCommon;
pub struct ReposHelper;
pub struct RefsHelper;
pub struct SuggestionsHelper;

impl WorktreeHelper {
    pub fn new(
        common: HelperCommon,
        repos_helper: ReposHelper,
        refs_helper: RefsHelper,
        suggestions_helper: SuggestionsHelper,
    ) -> Self {
        Self {
            common,
            repos_helper,
            refs_helper,
            suggestions_helper,
        }
    }

    pub fn get_main_worktree_name(&self) -> String {
        String::new()
    }

    pub fn get_linked_worktree_name(&self) -> String {
        String::new()
    }

    pub fn new_worktree(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn new_worktree_checkout(
        &self,
        _base: &str,
        _can_checkout_base: bool,
        _detached: bool,
        _context_key: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn switch(&self, _worktree: &Worktree, _context_key: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(&self, _worktree: &Worktree, _force: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn detach(&self, _worktree: &Worktree) -> Result<(), String> {
        Ok(())
    }

    pub fn view_worktree_options(&self, _context: &ListContext, _ref: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn view_branch_worktree_options(
        &self,
        _branch_name: &str,
        _can_checkout_base: bool,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub struct Worktree {
    pub is_main: bool,
    pub is_current: bool,
    pub name: String,
    pub path: String,
}

pub struct ListContext;
pub struct MenuItem;
pub struct PromptOpts;
pub struct ConfirmOpts;
