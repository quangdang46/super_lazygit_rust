// Ported from ./references/lazygit-master/pkg/gui/controllers/options_menu_action.go

pub struct OptionsMenuAction {
    common: ControllerCommon,
}

pub struct ControllerCommon;
pub struct MenuItem;
pub struct MenuSection {
    pub title: String,
    pub column: i32,
}
pub struct Binding;
pub struct CreateMenuOptions;
pub struct DisabledReason;

impl OptionsMenuAction {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn call(&self) -> Result<(), String> {
        Ok(())
    }

    fn get_bindings(&self, _context: &dyn Context) -> (Vec<Binding>, Vec<Binding>, Vec<Binding>) {
        (Vec::new(), Vec::new(), Vec::new())
    }
}

pub trait Context {
    fn get_view_name(&self) -> String;
}
