// Ported from ./references/lazygit-master/pkg/gui/controllers/remotes_controller.go

use crate::controllers::common::ControllerCommon;
use crate::types::common::ItemOperation;
use crate::types::context::Context;
use crate::types::keybindings::Binding;

pub struct RemotesController {
    base_controller: BaseController,
    c: ControllerCommon,
    set_remote_branches: Box<dyn Fn(Vec<RemoteBranch>)>,
}

struct BaseController {}

pub struct RemoteBranch {
    pub name: String,
    pub branches: Vec<RemoteBranch>,
}

pub struct Remote {
    pub name: String,
    pub urls: Vec<String>,
    pub branches: Vec<RemoteBranch>,
}

impl RemotesController {
    pub fn new(c: ControllerCommon, set_remote_branches: Box<dyn Fn(Vec<RemoteBranch>)>) -> Self {
        Self {
            base_controller: BaseController {},
            c,
            set_remote_branches,
        }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> String {
        "Remotes".to_string()
    }

    pub fn enter(&self, _remote: &Remote) -> Result<(), String> {
        Ok(())
    }

    pub fn add(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(&self, _remote: &Remote) -> Result<(), String> {
        Ok(())
    }

    pub fn edit(&self, _remote: &Remote) -> Result<(), String> {
        Ok(())
    }

    pub fn fetch(&self, _remote: &Remote) -> Result<(), String> {
        Ok(())
    }

    pub fn add_fork(&self) -> Result<(), String> {
        Ok(())
    }
}
