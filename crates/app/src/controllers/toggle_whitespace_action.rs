// Ported from ./references/lazygit-master/pkg/gui/controllers/toggle_whitespace_action.go

pub struct ToggleWhitespaceAction {
    common: ControllerCommon,
}

impl ToggleWhitespaceAction {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn call(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ControllerCommon;
