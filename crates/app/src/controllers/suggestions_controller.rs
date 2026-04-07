// Ported from ./references/lazygit-master/pkg/gui/controllers/suggestions_controller.go
use crate::controllers::ControllerCommon;

pub struct SuggestionsController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<Suggestion>,
}

pub struct Suggestion;

impl SuggestionsController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn switch_to_prompt(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn(OnFocusLostOpts)> {
        Box::new(|_opts| {})
    }

    fn context(&self) -> SuggestionsContext {
        SuggestionsContext
    }
}

pub struct ListControllerTrait<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> ListControllerTrait<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct SuggestionsContext;
pub struct KeybindingsOpts;
pub struct ViewMouseBinding;
pub struct OnFocusLostOpts;
pub struct Binding {
    pub key: char,
}
