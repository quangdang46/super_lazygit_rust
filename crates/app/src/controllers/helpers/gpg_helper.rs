// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/gpg_helper.go

pub struct GpgHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

pub struct CmdObj;

pub enum GpgConfigKey {
    CommitGpgSign,
}

impl GpgHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn with_gpg_handling(
        &self,
        _cmd_obj: &CmdObj,
        _config_key: GpgConfigKey,
        _waiting_status: &str,
        _on_success: Option<fn() -> Result<(), String>>,
        _refresh_scope: Vec<RefreshableView>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn run_and_stream(
        &self,
        _cmd_obj: &CmdObj,
        _waiting_status: &str,
        _on_success: Option<fn() -> Result<(), String>>,
        _refresh_scope: Vec<RefreshableView>,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub enum RefreshableView {
    Files,
    Branches,
    Commits,
    Stash,
    Remotes,
    Tags,
    Worktrees,
    Submodules,
}

impl GpgHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }
}

impl Default for GpgHelper {
    fn default() -> Self {
        Self::new()
    }
}
