// Ported from ./references/lazygit-master/pkg/gui/controllers/git_flow_controller.go
use crate::controllers::ControllerCommon;

pub struct GitFlowController {
    context: ControllerCommon,
}

impl GitFlowController {
    pub fn new(context: ControllerCommon) -> Self {
        Self { context }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn handle_create_git_flow_menu(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    fn git_flow_finish_branch(&self, _branch_name: &str) -> Result<(), String> {
        Ok(())
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
    pub opens_menu: bool,
}

pub struct Branch {
    pub name: String,
}
