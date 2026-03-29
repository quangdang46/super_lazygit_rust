use std::time::Instant;

use anyhow::Result;
use clap::Parser;

mod runtime;

use runtime::AppRuntime;
use super_lazygit_config::AppConfig;
use super_lazygit_core::{Action, AppState, Diagnostics, Event};
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
    let app = TuiApp::new(state, config.clone());
    let workspace = WorkspaceRegistry::new(cli.workspace);
    let git = GitFacade::default();

    let mut diagnostics = Diagnostics::default();

    let mut runtime = AppRuntime::new(app, workspace, git);
    diagnostics.extend_snapshot(runtime.bootstrap()?);
    runtime.run([Event::Action(Action::RefreshVisibleRepos)]);

    diagnostics.extend_snapshot(runtime.diagnostics_snapshot());
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

#[cfg(test)]
mod tests {
    use super::*;
    use super_lazygit_core::{AppMode, ScanStatus};

    #[test]
    fn runtime_processes_effects_until_worker_events_update_state() {
        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(None);
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.run([Event::Action(Action::RefreshVisibleRepos)]);

        assert_eq!(runtime.app().state().mode, AppMode::Workspace);
        assert!(matches!(
            runtime.app().state().workspace.scan_status,
            ScanStatus::Complete { .. }
        ));
    }
}
