// Ported from ./references/lazygit-master/pkg/gui/controllers/filter_controller.go
use crate::controllers::ControllerCommon;

pub struct FilterControllerFactory {
    context: ControllerCommon,
}

pub struct FilterController {
    context: ControllerCommon,
    filterable_context: Box<dyn IFilterableContext>,
}

pub trait IFilterableContext {
    fn get_key(&self) -> &str;
}

impl FilterControllerFactory {
    pub fn new(context: ControllerCommon) -> Self {
        Self { context }
    }

    pub fn create(&self, filterable_context: Box<dyn IFilterableContext>) -> FilterController {
        FilterController {
            context: self.context.clone(),
            filterable_context,
        }
    }
}

impl FilterController {
    pub fn context(&self) -> &dyn IFilterableContext {
        &*self.filterable_context
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn open_filter_prompt(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}
