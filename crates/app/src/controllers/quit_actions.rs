// Ported from ./references/lazygit-master/pkg/gui/controllers/quit_actions.go

pub struct QuitActions {
    common: ControllerCommon,
}

pub struct ControllerCommon;

impl QuitActions {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn quit(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn quit_without_changing_directory(&self) -> Result<(), String> {
        Ok(())
    }

    fn quit_aux(&self) -> Result<(), String> {
        Ok(())
    }

    fn confirm_quit_during_update(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape_enabled(&self) -> bool {
        false
    }

    pub fn escape_description(&self) -> String {
        String::new()
    }
}
