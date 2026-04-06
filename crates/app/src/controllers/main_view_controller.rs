// Ported from ./references/lazygit-master/pkg/gui/controllers/main_view_controller.go

pub struct MainViewController {
    common: ControllerCommon,
    context: MainContext,
    other_context: MainContext,
}

pub struct ControllerCommon;
pub struct MainContext;
pub struct ViewMouseBindingOpts {
    pub y: i32,
}

pub struct Binding {
    pub key: char,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
    pub description: String,
}

pub struct KeybindingsOpts;

impl MainViewController {
    pub fn new(common: ControllerCommon, context: MainContext, other_context: MainContext) -> Self {
        Self {
            common,
            context,
            other_context,
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn context(&self) -> MainContext {
        self.context.clone()
    }

    pub fn toggle_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn on_click_in_already_focused_view(
        &self,
        _opts: ViewMouseBindingOpts,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn on_click_in_other_view_of_main_view_pair(
        &self,
        _opts: ViewMouseBindingOpts,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn open_search(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ViewMouseBinding;
