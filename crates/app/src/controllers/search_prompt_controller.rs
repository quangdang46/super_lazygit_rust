// Ported from ./references/lazygit-master/pkg/gui/controllers/search_prompt_controller.go

pub struct SearchPromptController {
    common: ControllerCommon,
}

pub struct ControllerCommon;

impl SearchPromptController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Context {
        Context
    }

    pub fn confirm(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prev_history(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn next_history(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct KeybindingsOpts;
pub struct Binding;
pub struct Context;
