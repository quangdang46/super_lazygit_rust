// Ported from ./references/lazygit-master/pkg/gui/controllers/prompt_controller.go
use crate::controllers::ControllerCommon;

pub struct PromptController {
    common: ControllerCommon,
}

pub struct ViewMouseBinding;
pub struct Binding;
pub struct KeybindingsOpts;

impl PromptController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn context(&self) -> PromptContext {
        PromptContext
    }

    pub fn switch_to_suggestions(&self) {}
}

pub struct PromptContext;
