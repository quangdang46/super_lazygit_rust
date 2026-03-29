use anyhow::Result;
use clap::Parser;

use super_lazygit_config::AppConfig;
use super_lazygit_core::AppState;
use super_lazygit_git::GitFacade;
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug, Parser)]
#[command(name = "super-lazygit")]
#[command(about = "Workspace-first Lazygit-grade Rust TUI")]
struct Cli {
    /// Path to the workspace root to open.
    #[arg(long)]
    workspace: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::default();
    let state = AppState::default();
    let workspace = WorkspaceRegistry::new(cli.workspace);
    let git = GitFacade::default();
    let app = TuiApp::new(state, workspace, git, config);

    app.bootstrap()?;
    Ok(())
}
