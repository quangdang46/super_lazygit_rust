// Ported from ./references/lazygit-master/pkg/gui/controllers/jump_to_side_window_controller.go
use crate::controllers::ControllerCommon;

pub struct JumpToSideWindowController {
    common: ControllerCommon,
    next_tab_func: fn() -> Result<(), String>,
}

impl JumpToSideWindowController {
    pub fn new(common: ControllerCommon, next_tab_func: fn() -> Result<(), String>) -> Self {
        Self {
            common,
            next_tab_func,
        }
    }

    pub fn context(&self) -> Option<Context> {
        None
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    fn go_to_side_window(&self, _window: &str) -> impl Fn() -> Result<(), String> {
        move || Ok(())
    }
}

pub struct Context;
pub struct KeybindingsOpts;
pub struct Binding {
    pub view_name: String,
    pub key: char,
    pub modifier: String,
}
