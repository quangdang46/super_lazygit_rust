// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/cherry_pick_helper.go

pub struct Commit {
    pub hash: String,
}

pub struct ListContext;

pub struct CherryPicking;

pub struct MergeAndRebaseHelper;

pub struct HelperCommon;

pub struct CherryPickHelper {
    context: HelperCommon,
    rebase_helper: MergeAndRebaseHelper,
}

impl CherryPickHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            rebase_helper: MergeAndRebaseHelper,
        }
    }

    fn get_data(&self) -> CherryPicking {
        CherryPicking
    }

    pub fn copy_range(&self, _commits: &[Commit], _context: &ListContext) -> Result<(), String> {
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

impl Default for CherryPickHelper {
    fn default() -> Self {
        Self::new()
    }
}
