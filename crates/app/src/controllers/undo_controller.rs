// Ported from ./references/lazygit-master/pkg/gui/controllers/undo_controller.go

pub struct UndoController {
    common: ControllerCommon,
}

pub enum ReflogActionKind {
    Checkout,
    Commit,
    Rebase,
    CurrentRebase,
}

pub struct ReflogAction {
    kind: ReflogActionKind,
    from: String,
    to: String,
}

impl UndoController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> String {
        String::new()
    }

    pub fn reflog_undo(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn reflog_redo(&self) -> Result<(), String> {
        Ok(())
    }

    fn parse_reflog_for_actions(
        &self,
        _on_user_action: fn(counter: i32, action: ReflogAction) -> (bool, Result<(), String>),
    ) -> Result<(), String> {
        Ok(())
    }

    fn hard_reset_with_auto_stash(
        &self,
        _commit_hash: String,
        _options: HardResetOptions,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub struct HardResetOptions {
    pub waiting_status: String,
    pub env_vars: Vec<String>,
}

pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
