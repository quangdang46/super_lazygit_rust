// Ported from ./references/lazygit-master/pkg/gui/controllers/remote_branches_controller.go

pub struct RemoteBranchesController {
    common: ControllerCommon,
}

pub struct ControllerCommon;

impl RemoteBranchesController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn delete(&self, _selected_branches: &[RemoteBranch]) -> Result<(), String> {
        Ok(())
    }

    pub fn merge(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }

    pub fn rebase(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }

    pub fn create_sort_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn create_reset_menu(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }

    pub fn set_as_upstream(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }

    pub fn new_local_branch(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }

    pub fn checkout_branch(&self, _selected_branch: &RemoteBranch) -> Result<(), String> {
        Ok(())
    }
}

pub struct KeybindingsOpts;
pub struct Binding;
pub struct RemoteBranch;
