use std::time::Instant;

use anyhow::Result;
use clap::Parser;

use super_lazygit_config::AppConfig;
use super_lazygit_core::{AppState, Diagnostics};
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
    let startup_started_at = Instant::now();
    let cli = Cli::parse();
    let config = AppConfig::default();
    let state = AppState::default();
    let workspace = WorkspaceRegistry::new(cli.workspace);
    let git = GitFacade::default();
    let mut app = TuiApp::new(state, workspace, git, config.clone());

    let mut diagnostics = Diagnostics::default();
    diagnostics.extend_snapshot(app.bootstrap()?);
    diagnostics.record_startup_stage("app.main", startup_started_at.elapsed());

    if config.diagnostics.enabled && config.diagnostics.log_samples {
        let snapshot = diagnostics.snapshot();
        eprintln!(
            "[diagnostics] app_main startup_total_ms={} stages={}",
            snapshot.startup_total.as_millis(),
            snapshot.startup.len()
        );
    }

    Ok(())
}
