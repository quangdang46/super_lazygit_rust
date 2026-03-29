use std::time::Instant;

use super_lazygit_config::AppConfig;
use super_lazygit_core::{AppState, Diagnostics, DiagnosticsSnapshot};
use super_lazygit_git::GitFacade;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug)]
pub struct TuiApp {
    state: AppState,
    workspace: WorkspaceRegistry,
    git: GitFacade,
    config: AppConfig,
    diagnostics: Diagnostics,
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
            diagnostics: Diagnostics::default(),
        }
    }

    pub fn bootstrap(&mut self) -> std::io::Result<DiagnosticsSnapshot> {
        let started_at = Instant::now();
        let _ = &self.state;

        self.workspace
            .mark_watcher_started(usize::from(self.workspace.root().is_some()));
        self.workspace.record_watcher_refresh(1);
        self.git.record_operation("bootstrap.git.probe", true);
        self.record_render("bootstrap.frame");

        self.diagnostics
            .extend_snapshot(self.workspace.diagnostics());
        self.diagnostics.extend_snapshot(self.git.diagnostics());
        self.diagnostics
            .record_startup_stage("tui.bootstrap", started_at.elapsed());

        let snapshot = self.diagnostics.snapshot();
        if self.config.diagnostics.enabled && self.config.diagnostics.log_samples {
            log_diagnostics(&snapshot, &self.config);
        }

        Ok(snapshot)
    }

    fn record_render(&mut self, surface: &str) {
        let started_at = Instant::now();
        self.diagnostics
            .record_render(surface, started_at.elapsed());
    }
}

fn log_diagnostics(snapshot: &DiagnosticsSnapshot, config: &AppConfig) {
    eprintln!(
        "[diagnostics] startup_total_ms={} startup_stages={} scans={} git_ops={} watcher_events={} renders={}",
        snapshot.startup_total.as_millis(),
        snapshot.startup.len(),
        snapshot.scans.len(),
        snapshot.git_operations.len(),
        snapshot.watcher_churn_count(),
        snapshot.renders.len()
    );

    if let Some(render) = snapshot.slowest_render() {
        let threshold = u128::from(config.diagnostics.slow_render_threshold_ms);
        if render.elapsed.as_millis() >= threshold {
            eprintln!(
                "[diagnostics] slow_render surface={} elapsed_ms={} threshold_ms={}",
                render.surface,
                render.elapsed.as_millis(),
                threshold
            );
        }
    }

    if snapshot.watcher_churn_count() >= config.diagnostics.watcher_burst_threshold {
        eprintln!(
            "[diagnostics] watcher_churn count={} threshold={}",
            snapshot.watcher_churn_count(),
            config.diagnostics.watcher_burst_threshold
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_emits_diagnostics_snapshot() {
        let mut app = TuiApp::new(
            AppState::default(),
            WorkspaceRegistry::new(None),
            GitFacade::default(),
            AppConfig::default(),
        );

        let snapshot = app.bootstrap().expect("bootstrap should succeed");

        assert!(!snapshot.startup.is_empty());
        assert!(!snapshot.scans.is_empty());
        assert!(!snapshot.git_operations.is_empty());
        assert!(!snapshot.renders.is_empty());
    }
}
