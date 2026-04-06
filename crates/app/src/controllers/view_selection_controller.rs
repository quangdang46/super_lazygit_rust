// Ported from ./references/lazygit-master/pkg/gui/controllers/view_selection_controller.go

pub struct ViewSelectionControllerFactory {
    common: ControllerCommon,
}

impl ViewSelectionControllerFactory {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn create(&self, _context: String) -> ViewSelectionController {
        ViewSelectionController::new(self.common.clone())
    }
}

pub struct ViewSelectionController {
    common: ControllerCommon,
    context: String,
}

impl ViewSelectionController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            context: String::new(),
        }
    }

    pub fn context(&self) -> String {
        self.context.clone()
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    fn handle_line_change(&self, _delta: i32) {}

    pub fn handle_prev_line(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_line(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_prev_page(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_page(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_goto_top(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_goto_bottom(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ControllerCommon;

impl Clone for ControllerCommon {
    fn clone(&self) -> Self {
        ControllerCommon
    }
}

pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
    pub tag: String,
}
pub struct ViewMouseBinding;
