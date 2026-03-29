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
    let workspace_root = cli
        .workspace
        .or_else(|| config.workspace.roots.first().cloned())
        .or_else(|| std::env::current_dir().ok());
    let state = AppState::default();
    let app = TuiApp::new(state, config.clone());
    let workspace = WorkspaceRegistry::new(workspace_root);
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
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use super_lazygit_core::{AppMode, RepoId, RepoSummary, ScanStatus, Timestamp, WorkspaceState};

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

    #[test]
    fn bootstrap_hydrates_workspace_from_cache_before_scan_runs() {
        let root = tempfile::tempdir().expect("workspace root");
        let repo_path = root.path().join("repo-a");
        fs::create_dir_all(repo_path.join(".git")).expect("repo fixture");

        let repo_id = RepoId::new(repo_path.display().to_string());
        let workspace_state = WorkspaceState {
            current_root: Some(root.path().to_path_buf()),
            discovered_repo_ids: vec![repo_id.clone()],
            repo_summaries: BTreeMap::from([(
                repo_id.clone(),
                RepoSummary {
                    repo_id: repo_id.clone(),
                    display_name: String::from("repo-a"),
                    real_path: repo_path.clone(),
                    display_path: repo_path.display().to_string(),
                    last_refresh_at: Some(Timestamp(9)),
                    ..RepoSummary::default()
                },
            )]),
            selected_repo_id: Some(repo_id.clone()),
            scan_status: ScanStatus::Complete { scanned_repos: 1 },
            last_full_refresh_at: Some(Timestamp(9)),
            ..WorkspaceState::default()
        };

        let workspace_writer = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        workspace_writer
            .persist_cache(&workspace_state)
            .expect("persist cache");

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.bootstrap().expect("bootstrap succeeds");

        assert_eq!(
            runtime.app().state().workspace.selected_repo_id,
            Some(repo_id.clone())
        );
        assert_eq!(
            runtime.app().state().workspace.discovered_repo_ids,
            vec![repo_id.clone()]
        );
        assert_eq!(
            runtime
                .app()
                .state()
                .workspace
                .repo_summaries
                .get(&repo_id)
                .map(|summary| summary.display_name.as_str()),
            Some("repo-a")
        );
    }
}
