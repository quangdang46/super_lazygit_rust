// Ported from ./references/lazygit-master/pkg/gui/controllers/command_log_controller.go

use crate::controllers::common::ControllerCommon;
use crate::types::context::{Context, IController, OnFocusLostOpts};
use crate::types::keybindings::Binding;

pub struct CommandLogController {
    base_controller: BaseController,
    c: ControllerCommon,
}

struct BaseController {}

impl CommandLogController {
    pub fn new(c: ControllerCommon) -> Self {
        Self {
            base_controller: BaseController {},
            c,
        }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_focus_lost(&self) -> Option<Box<dyn Fn(OnFocusLostOpts)>> {
        Some(Box::new(|_opts| {
            // self.c.Views().Extras.Autoscroll = true
        }))
    }

    pub fn context(&self) -> String {
        "CommandLog".to_string()
    }
}
