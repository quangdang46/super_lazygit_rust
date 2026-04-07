// Ported from ./references/lazygit-master/pkg/gui/controllers/sync_controller.go
use crate::controllers::ControllerCommon;

pub struct SyncController {
    common: ControllerCommon,
}

impl SyncController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> String {
        String::new()
    }

    pub fn handle_push(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_pull(&self) -> Result<(), String> {
        Ok(())
    }

    fn get_disabled_reason_for_push_or_pull(&self) -> Option<DisabledReason> {
        None
    }

    fn branch_checked_out(
        &self,
        _f: fn(&Branch) -> Result<(), String>,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn push(&self, _current_branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn pull(&self, _current_branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    fn set_current_branch_upstream(&self, _upstream: String) -> Result<(), String> {
        Ok(())
    }

    pub fn pull_aux(
        &self,
        _current_branch: &Branch,
        _opts: PullFilesOptions,
    ) -> Result<(), String> {
        Ok(())
    }

    fn pull_with_lock(&self, _opts: PullFilesOptions) -> Result<(), String> {
        Ok(())
    }

    pub fn push_aux(&self, _current_branch: &Branch, _opts: PushOpts) -> Result<(), String> {
        Ok(())
    }

    fn request_to_force_push(
        &self,
        _current_branch: &Branch,
        _opts: PushOpts,
    ) -> Result<(), String> {
        Ok(())
    }

    fn force_push_prompt(&self) -> String {
        String::new()
    }
}

pub struct Branch;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
pub struct DisabledReason {
    pub text: String,
}
pub struct PullFilesOptions {
    pub upstream_remote: String,
    pub upstream_branch: String,
    pub fast_forward_only: bool,
    pub action: String,
}
pub struct PushOpts {
    pub force: bool,
    pub force_with_lease: bool,
    pub upstream_remote: String,
    pub upstream_branch: String,
    pub set_upstream: bool,
    pub remote_branch_stored_locally: bool,
}
