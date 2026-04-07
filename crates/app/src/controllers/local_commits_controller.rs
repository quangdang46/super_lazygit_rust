// Ported from ./references/lazygit-master/pkg/gui/controllers/local_commits_controller.go
use crate::controllers::ControllerCommon;

pub struct LocalCommitsController {
    common: ControllerCommon,
    pull_files: PullFilesFn,
}

pub struct ListControllerTrait;

pub struct Commit;

pub struct DisabledReason {
    pub text: String,
}

pub struct Binding {
    pub key: char,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
    pub description: String,
}

pub struct KeybindingsOpts;
pub struct ConfirmOpts;
pub struct CreateMenuOptions;
pub struct MenuItem;
pub struct RefreshOptions;

pub struct SelectionRangeAndMode {
    pub selected_hash: String,
    pub range_start_hash: String,
    pub mode: RangeSelectMode,
}

pub enum RangeSelectMode {
    Range,
    Single,
}

impl LocalCommitsController {
    pub fn new(common: ControllerCommon, pull_files: PullFilesFn) -> Self {
        Self { common, pull_files }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn squash_down(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn fixup(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn reword(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn drop(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn edit(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn quick_start_interactive_rebase(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn move_down(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn move_up(
        &self,
        _selected_commits: &[Commit],
        _start_idx: usize,
        _end_idx: usize,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn amend_to(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn revert(&self, _commits: &[Commit], _start: usize, _end: usize) -> Result<(), String> {
        Ok(())
    }

    pub fn create_tag(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn open_search(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_open_log_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn paste(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn mark_as_base_commit(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }
}

type PullFilesFn = Box<dyn Fn() -> Result<(), String>>;
