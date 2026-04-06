// Ported from ./references/lazygit-master/pkg/gui/controllers/switch_to_diff_files_controller.go

pub struct SwitchToDiffFilesController {
    common: ControllerCommon,
    context: Box<dyn CanSwitchToDiffFiles>,
}

pub trait CanSwitchToDiffFiles {
    fn get_selected_ref(&self) -> Ref;
    fn get_selected_ref_range_for_diff_files(&self) -> Option<RefRange>;
    fn can_rebase(&self) -> bool;
}

pub struct Ref;
pub struct RefRange;

impl SwitchToDiffFilesController {
    pub fn new(common: ControllerCommon, context: Box<dyn CanSwitchToDiffFiles>) -> Self {
        Self { common, context }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> String {
        String::new()
    }

    pub fn get_on_double_click(&self) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn enter(&self) -> Result<(), String> {
        Ok(())
    }

    fn can_enter(&self) -> Option<DisabledReason> {
        None
    }
}

pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
pub struct DisabledReason {
    pub text: String,
}
