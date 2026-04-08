// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/merge_conflicts_helper.go

pub struct MergeConflictsHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl MergeConflictsHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn set_merge_state(&self, _path: &str) -> Result<bool, String> {
        Ok(false)
    }

    fn set_merge_state_without_lock(&self, _path: &str) -> Result<bool, String> {
        Ok(false)
    }

    pub fn reset_merge_state(&self) {}

    fn reset_merge_state_internal(&self) {}

    pub fn escape_merge(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn set_conflicts_and_render(&self, _path: &str) -> Result<bool, String> {
        Ok(false)
    }

    pub fn switch_to_merge(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn context(&self) -> MergeConflictsContext {
        MergeConflictsContext
    }

    pub fn render(&self) {}

    pub fn refresh_merge_state(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct MergeConflictsContext;

pub struct MergeConflictsState;
