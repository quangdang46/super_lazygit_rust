// Ported from ./references/lazygit-master/pkg/gui/controllers/worktree_options_controller.go
use crate::controllers::ControllerCommon;

pub struct WorktreeOptionsController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<String>,
    context: Box<dyn CanViewWorktreeOptions>,
}

pub trait CanViewWorktreeOptions {
    fn get_selected_item_id(&self) -> String;
    fn get_selected_item_ids(&self) -> Vec<String>;
}

impl WorktreeOptionsController {
    pub fn new(common: ControllerCommon, context: Box<dyn CanViewWorktreeOptions>) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
            context,
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn view_worktree_options(&self, _ref: String) -> Result<(), String> {
        Ok(())
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

pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
