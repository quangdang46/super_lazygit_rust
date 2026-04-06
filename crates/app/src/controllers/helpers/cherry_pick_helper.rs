// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/cherry_pick_helper.go

pub struct CherryPickHelper {
    context: HelperCommon,
    rebase_helper: MergeAndRebaseHelper,
}

pub struct HelperCommon;

pub struct MergeAndRebaseHelper;

pub struct Commit {
    pub hash: String,
}

pub struct CherryPicking;

impl CherryPickHelper {
    pub fn new(context: HelperCommon, rebase_helper: MergeAndRebaseHelper) -> Self {
        Self {
            context,
            rebase_helper,
        }
    }

    fn get_data(&self) -> CherryPicking {
        CherryPicking
    }

    pub fn copy_range(
        &self,
        commits_list: &[Commit],
        _context: &ListContext,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn paste(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn can_paste(&self) -> bool {
        false
    }

    pub fn reset(&self) -> Result<(), String> {
        Ok(())
    }

    fn reset_if_necessary(&self, _context: &ListContext) -> Result<(), String> {
        Ok(())
    }

    fn rerender(&self) {}
}

pub struct ListContext;

impl CherryPickHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            rebase_helper: MergeAndRebaseHelper,
        }
    }
}

impl Default for CherryPickHelper {
    fn default() -> Self {
        Self::new()
    }
}
