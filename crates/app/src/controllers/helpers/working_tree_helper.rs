// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/working_tree_helper.go

pub struct WorkingTreeHelper {
    common: HelperCommon,
    ref_helper: RefsHelper,
    commits_helper: CommitsHelper,
    gpg_helper: GpgHelper,
    merge_and_rebase_helper: MergeAndRebaseHelper,
}

pub struct HelperCommon;
pub struct RefsHelper;
pub struct CommitsHelper;
pub struct GpgHelper;
pub struct MergeAndRebaseHelper;

impl WorkingTreeHelper {
    pub fn new(
        common: HelperCommon,
        ref_helper: RefsHelper,
        commits_helper: CommitsHelper,
        gpg_helper: GpgHelper,
        merge_and_rebase_helper: MergeAndRebaseHelper,
    ) -> Self {
        Self {
            common,
            ref_helper,
            commits_helper,
            gpg_helper,
            merge_and_rebase_helper,
        }
    }

    pub fn any_staged_files(&self) -> bool {
        false
    }

    pub fn any_staged_files_except_submodules(&self) -> bool {
        false
    }

    pub fn any_tracked_files(&self) -> bool {
        false
    }

    pub fn any_tracked_files_except_submodules(&self) -> bool {
        false
    }

    pub fn is_working_tree_dirty_except_submodules(&self) -> bool {
        false
    }

    pub fn file_for_submodule(&self, _submodule: &SubmoduleConfig) -> Option<File> {
        None
    }

    pub fn open_merge_tool(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_commit_press_with_message(
        &self,
        _initial_message: &str,
        _force_skip_hooks: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_commit_editor_press(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_wip_commit_press(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_commit_press(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn with_ensure_committable_files<F>(&self, _handler: F) -> Result<(), String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        Ok(())
    }

    pub fn create_merge_conflict_menu(
        &self,
        _selected_filepaths: Vec<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub struct File;
pub struct SubmoduleConfig;

pub fn any_staged_files(_files: &[File]) -> bool {
    false
}

pub fn any_tracked_files(_files: &[File]) -> bool {
    false
}

pub fn is_working_tree_dirty_except_submodules(
    _files: &[File],
    _submodule_configs: &[SubmoduleConfig],
) -> bool {
    false
}
