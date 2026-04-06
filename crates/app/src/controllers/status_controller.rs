// Ported from ./references/lazygit-master/pkg/gui/controllers/status_controller.go

pub struct StatusController {
    common: ControllerCommon,
}

impl StatusController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn context(&self) -> String {
        String::new()
    }

    pub fn on_click(&self, _opts: ViewMouseBindingOpts) -> Result<(), String> {
        Ok(())
    }

    fn ask_for_config_file(
        &self,
        _action: fn(file: String) -> Result<(), String>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn open_config(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_config(&self) -> Result<(), String> {
        Ok(())
    }

    fn show_all_branch_logs(&self) {}

    fn switch_to_or_rotate_all_branches_logs(&self) {}

    fn switch_to_or_rotate_all_branches_logs_backward(&self) {}

    fn show_dashboard(&self) {}

    pub fn handle_check_for_update(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct ViewMouseBinding;
pub struct Binding {
    pub key: char,
    pub description: String,
}
pub struct ViewMouseBindingOpts {
    pub x: i32,
    pub y: i32,
}
pub struct CreateMenuOptions;
