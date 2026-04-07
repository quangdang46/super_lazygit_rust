// Ported from ./references/lazygit-master/pkg/gui/controllers/menu_controller.go
use crate::controllers::ControllerCommon;

pub struct MenuController {
    common: ControllerCommon,
}

pub struct ListControllerTrait;
pub struct MenuItem;
pub struct MenuContext;
pub struct OnFocusOpts;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
    pub description: String,
}

impl MenuController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_double_click(&self) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn(OnFocusOpts)> {
        Box::new(|_| {})
    }

    pub fn press(&self, _selected_item: &MenuItem) -> Result<(), String> {
        Ok(())
    }

    pub fn close(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn context(&self) -> MenuContext {
        MenuContext
    }
}
