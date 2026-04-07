// Ported from ./references/lazygit-master/pkg/gui/controllers/context_lines_controller.go

use crate::controllers::common::ControllerCommon;
use crate::types::context::{Context, IController};
use crate::types::keybindings::Binding;
use crate::types::common::ItemOperation;

pub struct ContextLinesController {
    base_controller: BaseController,
    c: ControllerCommon,
}

struct BaseController {}

impl ContextLinesController {
    pub fn new(c: ControllerCommon) -> Self {
        Self {
            base_controller: BaseController {},
            c,
        }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Option<String> {
        None
    }

    pub fn increase(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub fn decrease(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn apply_change(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn check_can_change_context(&self) -> Result<(), String> {
        Ok(())
    }
}
