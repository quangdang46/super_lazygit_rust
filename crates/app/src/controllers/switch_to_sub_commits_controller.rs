// Ported from ./references/lazygit-master/pkg/gui/controllers/switch_to_sub_commits_controller.go
use crate::controllers::ControllerCommon;

pub struct SwitchToSubCommitsController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<Ref>,
    context: Box<dyn CanSwitchToSubCommits>,
}

pub trait CanSwitchToSubCommits {
    fn get_selected_ref(&self) -> Option<Ref>;
    fn show_branch_heads_in_sub_commits(&self) -> bool;
}

pub struct Ref;

impl SwitchToSubCommitsController {
    pub fn new(common: ControllerCommon, context: Box<dyn CanSwitchToSubCommits>) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
            context,
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_double_click(&self) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn view_commits(&self) -> Result<(), String> {
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
