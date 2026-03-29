use super_lazygit_config::AppConfig;
use super_lazygit_core::AppState;
use super_lazygit_git::GitFacade;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug)]
pub struct TuiApp {
    state: AppState,
    workspace: WorkspaceRegistry,
    git: GitFacade,
    config: AppConfig,
}

impl TuiApp {
    #[must_use]
    pub fn new(
        state: AppState,
        workspace: WorkspaceRegistry,
        git: GitFacade,
        config: AppConfig,
    ) -> Self {
        Self {
            state,
            workspace,
            git,
            config,
        }
    }

    pub fn bootstrap(&self) -> std::io::Result<()> {
        let _ = (&self.state, &self.workspace, &self.git, &self.config);
        Ok(())
    }
}
