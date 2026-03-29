use std::collections::VecDeque;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super_lazygit_core::{
    Diagnostics, DiagnosticsSnapshot, Effect, Event, GitCommand, Timestamp, WorkerEvent,
};
use super_lazygit_git::{GitFacade, RepoDetailRequest, RepoSummaryRequest, WorkspaceScanRequest};
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug)]
pub struct AppRuntime {
    app: TuiApp,
    workspace: WorkspaceRegistry,
    git: GitFacade,
    diagnostics: Diagnostics,
}

impl AppRuntime {
    #[must_use]
    pub fn new(app: TuiApp, workspace: WorkspaceRegistry, git: GitFacade) -> Self {
        Self {
            app,
            workspace,
            git,
            diagnostics: Diagnostics::default(),
        }
    }

    pub fn bootstrap(&mut self) -> std::io::Result<DiagnosticsSnapshot> {
        let started_at = Instant::now();

        self.workspace
            .mark_watcher_started(usize::from(self.workspace.root().is_some()));
        self.workspace.record_watcher_refresh(1);
        self.git.record_operation("bootstrap.git.probe", true);
        let _ = self.app.render();

        self.diagnostics
            .extend_snapshot(self.workspace.diagnostics());
        self.diagnostics.extend_snapshot(self.git.diagnostics());
        self.diagnostics
            .extend_snapshot(self.app.diagnostics_snapshot());
        self.diagnostics
            .record_startup_stage("app.runtime.bootstrap", started_at.elapsed());

        Ok(self.diagnostics.snapshot())
    }

    pub fn run<I>(&mut self, seed_events: I)
    where
        I: IntoIterator<Item = Event>,
    {
        let mut queue = VecDeque::from_iter(seed_events);

        while let Some(event) = queue.pop_front() {
            let result = self.app.dispatch(event);
            for follow_up in self.apply_effects(&result.effects) {
                queue.push_back(follow_up);
            }
        }
    }

    #[must_use]
    pub fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend_snapshot(self.app.diagnostics_snapshot());
        diagnostics.snapshot()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn app(&self) -> &TuiApp {
        &self.app
    }

    fn apply_effects(&mut self, effects: &[Effect]) -> Vec<Event> {
        let mut follow_up_events = Vec::new();

        for effect in effects {
            match effect {
                Effect::StartRepoScan => {
                    let result = self.git.scan_workspace(WorkspaceScanRequest {
                        root: self.workspace.root().cloned(),
                    });
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    match result {
                        Ok(scan) => {
                            self.workspace
                                .record_scan("runtime.start_repo_scan", scan.repo_ids.len());
                            self.diagnostics
                                .extend_snapshot(self.workspace.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoScanCompleted {
                                root: scan.root,
                                repo_ids: scan.repo_ids,
                                scanned_at: current_timestamp(),
                            }));
                        }
                        Err(error) => {
                            self.git.record_operation("scan_workspace_failed", false);
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoScanCompleted {
                                root: self.workspace.root().cloned(),
                                repo_ids: Vec::new(),
                                scanned_at: current_timestamp(),
                            }));
                            let _ = error;
                        }
                    }
                }
                Effect::RefreshRepoSummary { repo_id } => {
                    let result = self.git.read_repo_summary(RepoSummaryRequest {
                        repo_id: repo_id.clone(),
                    });
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    if let Ok(summary) = result {
                        follow_up_events
                            .push(Event::Worker(WorkerEvent::RepoSummaryUpdated { summary }));
                    }
                }
                Effect::LoadRepoDetail { repo_id } => {
                    let result = self.git.read_repo_detail(RepoDetailRequest {
                        repo_id: repo_id.clone(),
                    });
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    if let Ok(detail) = result {
                        follow_up_events.push(Event::Worker(WorkerEvent::RepoDetailLoaded {
                            repo_id: repo_id.clone(),
                            detail,
                        }));
                    }
                }
                Effect::RunGitCommand(request) => {
                    let summary = git_command_summary(&request.command);
                    follow_up_events.push(Event::Worker(WorkerEvent::GitOperationStarted {
                        job_id: request.job_id.clone(),
                        repo_id: request.repo_id.clone(),
                        summary: summary.to_string(),
                    }));

                    match self.git.run_command(request.clone()) {
                        Ok(outcome) => {
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(
                                WorkerEvent::GitOperationCompleted {
                                    job_id: request.job_id.clone(),
                                    repo_id: outcome.repo_id,
                                    summary: outcome.summary,
                                },
                            ));
                        }
                        Err(error) => {
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::GitOperationFailed {
                                job_id: request.job_id.clone(),
                                repo_id: request.repo_id.clone(),
                                error: error.to_string(),
                            }));
                        }
                    }
                }
                Effect::PersistCache | Effect::PersistConfig | Effect::ScheduleRender => {
                    let _ = self.app.render();
                }
            }
        }

        follow_up_events
    }
}

fn git_command_summary(command: &GitCommand) -> &'static str {
    match command {
        GitCommand::StageSelection => "stage_selection",
        GitCommand::CommitStaged { .. } => "commit_staged",
        GitCommand::AmendHead { .. } => "amend_head",
        GitCommand::CheckoutBranch { .. } => "checkout_branch",
        GitCommand::FetchSelectedRepo => "fetch_selected_repo",
        GitCommand::PullCurrentBranch => "pull_current_branch",
        GitCommand::PushCurrentBranch => "push_current_branch",
        GitCommand::RefreshSelectedRepo => "refresh_selected_repo",
    }
}

fn current_timestamp() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}
