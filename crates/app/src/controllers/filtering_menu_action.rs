// Ported from ./references/lazygit-master/pkg/gui/controllers/filtering_menu_action.go

pub struct FilteringMenuAction {
    context: ControllerCommon,
}

pub struct ControllerCommon;

impl FilteringMenuAction {
    pub fn new(context: ControllerCommon) -> Self {
        Self { context }
    }

    pub fn call(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering_path(&self, path: String) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering_author(&self, author: String) -> Result<(), String> {
        Ok(())
    }

    fn set_filtering(&self) -> Result<(), String> {
        Ok(())
    }
}

impl FilteringMenuAction {
    pub fn new() -> Self {
        Self {
            context: ControllerCommon,
        }
    }
}

impl Default for FilteringMenuAction {
    fn default() -> Self {
        Self::new()
    }
}
