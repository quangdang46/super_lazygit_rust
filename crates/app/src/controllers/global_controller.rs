// Ported from ./references/lazygit-master/pkg/gui/controllers/global_controller.go

pub struct GlobalController {
    context: ControllerCommon,
}

pub struct ControllerCommon;

impl GlobalController {
    pub fn new(context: ControllerCommon) -> Self {
        Self { context }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Option<&Context> {
        None
    }

    pub fn shell_command(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn create_custom_patch_options_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn refresh(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn next_screen_mode(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prev_screen_mode(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn cycle_pagers(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn can_cycle_pagers(&self) -> Option<DisabledReason> {
        None
    }

    pub fn create_options_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn options_menu_disabled_reason(&self) -> Option<DisabledReason> {
        None
    }

    pub fn create_filtering_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn create_diffing_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn quit(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn quit_without_changing_directory(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape_description(&self) -> String {
        String::new()
    }

    pub fn escape_enabled(&self) -> Option<DisabledReason> {
        None
    }

    pub fn toggle_whitespace(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn can_show_rebase_options(&self) -> Option<DisabledReason> {
        None
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
    pub modifier: String,
    pub opens_menu: bool,
}

pub struct Context;

pub struct DisabledReason {
    pub text: String,
}

impl GlobalController {
    pub fn new() -> Self {
        Self {
            context: ControllerCommon,
        }
    }
}

impl Default for GlobalController {
    fn default() -> Self {
        Self::new()
    }
}
