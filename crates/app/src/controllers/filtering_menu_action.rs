// Ported from ./references/lazygit-master/pkg/gui/controllers/filtering_menu_action.go
use crate::controllers::ControllerCommon;

pub struct FilteringMenuAction {
    context: ControllerCommon,
}

impl FilteringMenuAction {
    pub fn new() -> Self {
        Self {
            context: ControllerCommon::default(),
        }
    }

    pub fn call(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering_path(&self, _path: String) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering_author(&self, _author: String) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Default for FilteringMenuAction {
    fn default() -> Self {
        Self::new()
    }
}
