// Ported from ./references/lazygit-master/pkg/gui/controllers/search_controller.go
use crate::controllers::ControllerCommon;

pub struct SearchControllerFactory {
    common: ControllerCommon,
}

pub struct SearchController {
    common: ControllerCommon,
    context: SearchableContext,
}

#[derive(Clone)]
pub struct SearchableContext;

impl SearchControllerFactory {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn create(&self, context: SearchableContext) -> SearchController {
        SearchController {
            common: self.common.clone(),
            context,
        }
    }
}

impl SearchController {
    pub fn context(&self) -> SearchableContext {
        self.context.clone()
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn open_search_prompt(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct KeybindingsOpts;
pub struct Binding;
