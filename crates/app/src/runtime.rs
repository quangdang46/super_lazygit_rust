use std::cmp;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::watcher::{NullWatcherBackend, WatchRegistration, WatcherBackend};
use super_lazygit_core::{
    AppWatcherEvent, Diagnostics, DiagnosticsSnapshot, Effect, Event, GitCommand, JobId,
    PatchApplicationMode, PatchSelectionJob, RepoId, RepoSummary, TimerEvent, Timestamp,
    WorkerEvent,
};
use super_lazygit_git::{
    GitFacade, GitResult, PatchSelectionRequest, RepoDetailRequest, RepoSummaryRequest,
    WorkspaceScanRequest,
};
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

const SUMMARY_REFRESH_WORKER_LIMIT: usize = 4;

#[derive(Debug)]
pub struct AppRuntime {
    app: TuiApp,
    workspace: WorkspaceRegistry,
    git: GitFacade,
    diagnostics: Diagnostics,
    watcher: Box<dyn WatcherBackend>,
    watcher_debounce_scheduled: bool,
}

impl AppRuntime {
    #[must_use]
    pub fn new(app: TuiApp, workspace: WorkspaceRegistry, git: GitFacade) -> Self {
        Self::with_watcher(app, workspace, git, NullWatcherBackend::default())
    }

    #[must_use]
    pub fn with_watcher<W>(
        app: TuiApp,
        workspace: WorkspaceRegistry,
        git: GitFacade,
        watcher: W,
    ) -> Self
    where
        W: WatcherBackend + 'static,
    {
        Self {
            app,
            workspace,
            git,
            diagnostics: Diagnostics::default(),
            watcher: Box::new(watcher),
            watcher_debounce_scheduled: false,
        }
    }

    pub fn bootstrap(&mut self) -> std::io::Result<DiagnosticsSnapshot> {
        let started_at = Instant::now();

        if let Some(cached_workspace) = self.workspace.load_cache() {
            let _ = self.app.dispatch(Event::Action(
                super_lazygit_core::Action::ApplyWorkspaceScan(cached_workspace),
            ));
        }
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
        queue.extend(self.drain_watcher_events());

        while let Some(event) = queue.pop_front() {
            let result = self.app.dispatch(event);
            for follow_up in self.apply_effects(&result.effects) {
                queue.push_back(follow_up);
            }
            for watcher_event in self.drain_watcher_events() {
                queue.push_back(watcher_event);
            }
            if queue.is_empty() && self.watcher_debounce_scheduled {
                self.watcher_debounce_scheduled = false;
                queue.push_back(Event::Timer(TimerEvent::WatcherDebounceFlush));
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

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn render_to_string(&mut self) -> String {
        self.app.render_to_string()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn persist_cache(&self) -> std::io::Result<()> {
        self.workspace.persist_cache(&self.app.state().workspace)
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
                            let repo_ids = self
                                .workspace
                                .register_scan(scan.root.clone(), &scan.repo_ids);
                            self.workspace
                                .record_scan("runtime.start_repo_scan", repo_ids.len());
                            self.diagnostics
                                .extend_snapshot(self.workspace.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoScanCompleted {
                                root: scan.root,
                                repo_ids,
                                scanned_at: current_timestamp(),
                            }));
                        }
                        Err(error) => {
                            self.git.record_operation("scan_workspace_failed", false);
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoScanFailed {
                                root: self.workspace.root().cloned(),
                                error: error.to_string(),
                            }));
                        }
                    }
                }
                Effect::ConfigureWatcher { repo_ids } => {
                    follow_up_events.extend(self.configure_watcher(repo_ids));
                }
                Effect::ScheduleWatcherDebounce => {
                    self.watcher_debounce_scheduled = true;
                }
                Effect::RefreshRepoSummaries { repo_ids } => {
                    follow_up_events.extend(self.refresh_repo_summaries(repo_ids.clone()));
                }
                Effect::RefreshRepoSummary { repo_id } => {
                    follow_up_events.extend(self.refresh_repo_summaries(vec![repo_id.clone()]));
                }
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path,
                    diff_presentation,
                } => {
                    let result = self.git.read_repo_detail(RepoDetailRequest {
                        repo_id: repo_id.clone(),
                        selected_path: selected_path.clone(),
                        diff_presentation: *diff_presentation,
                    });
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    if let Ok(detail) = result {
                        follow_up_events.push(Event::Worker(WorkerEvent::RepoDetailLoaded {
                            repo_id: repo_id.clone(),
                            detail,
                        }));
                    }
                }
                Effect::LoadRepoDiff {
                    repo_id,
                    comparison_target,
                    compare_with,
                    selected_path,
                    diff_presentation,
                } => {
                    let result = self.git.read_diff(super_lazygit_git::DiffRequest {
                        repo_id: repo_id.clone(),
                        comparison_target: comparison_target.clone(),
                        compare_with: compare_with.clone(),
                        selected_path: selected_path.clone(),
                        diff_presentation: *diff_presentation,
                    });
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    match result {
                        Ok(diff) => {
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoDiffLoaded {
                                repo_id: repo_id.clone(),
                                diff,
                            }));
                        }
                        Err(error) => {
                            follow_up_events.push(Event::Worker(WorkerEvent::RepoDiffLoadFailed {
                                repo_id: repo_id.clone(),
                                error: error.to_string(),
                            }));
                        }
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
                Effect::RunPatchSelection(job) => {
                    let summary = patch_selection_summary(job);
                    follow_up_events.push(Event::Worker(WorkerEvent::GitOperationStarted {
                        job_id: job.job_id.clone(),
                        repo_id: job.repo_id.clone(),
                        summary: summary.to_string(),
                    }));

                    match self.git.apply_patch_selection(PatchSelectionRequest {
                        repo_id: job.repo_id.clone(),
                        path: job.path.clone(),
                        mode: job.mode,
                        hunks: job.hunks.clone(),
                    }) {
                        Ok(outcome) => {
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(
                                WorkerEvent::GitOperationCompleted {
                                    job_id: job.job_id.clone(),
                                    repo_id: outcome.repo_id,
                                    summary: outcome.summary,
                                },
                            ));
                        }
                        Err(error) => {
                            self.diagnostics.extend_snapshot(self.git.diagnostics());
                            follow_up_events.push(Event::Worker(WorkerEvent::GitOperationFailed {
                                job_id: job.job_id.clone(),
                                repo_id: job.repo_id.clone(),
                                error: error.to_string(),
                            }));
                        }
                    }
                }
                Effect::PersistCache => {
                    let _ = self.workspace.persist_cache(&self.app.state().workspace);
                }
                Effect::PersistConfig => {}
                Effect::ScheduleRender => {
                    let _ = self.app.render();
                }
            }
        }

        follow_up_events
    }

    fn configure_watcher(&mut self, repo_ids: &[RepoId]) -> Vec<Event> {
        let registrations = match self.watch_registrations(repo_ids) {
            Ok(registrations) => registrations,
            Err(message) => {
                return vec![Event::Watcher(AppWatcherEvent::WatcherDegraded { message })];
            }
        };

        match self.watcher.configure(registrations) {
            Ok(path_count) => {
                self.workspace.mark_watcher_started(path_count);
                self.diagnostics
                    .extend_snapshot(self.workspace.diagnostics());
                vec![Event::Watcher(AppWatcherEvent::WatcherRecovered)]
            }
            Err(message) => vec![Event::Watcher(AppWatcherEvent::WatcherDegraded { message })],
        }
    }

    fn watch_registrations(&self, repo_ids: &[RepoId]) -> Result<Vec<WatchRegistration>, String> {
        repo_ids
            .iter()
            .map(|repo_id| {
                let path = self
                    .workspace
                    .repo_path(repo_id)
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from(&repo_id.0));
                Ok(WatchRegistration {
                    repo_id: repo_id.clone(),
                    path,
                })
            })
            .collect()
    }

    fn drain_watcher_events(&mut self) -> Vec<Event> {
        let events = self.watcher.drain();
        let invalidation_count = events
            .iter()
            .filter(|event| matches!(event, AppWatcherEvent::RepoInvalidated { .. }))
            .count();

        if invalidation_count > 0 {
            self.workspace.record_watcher_refresh(invalidation_count);
            self.diagnostics
                .extend_snapshot(self.workspace.diagnostics());
        }

        events.into_iter().map(Event::Watcher).collect()
    }

    fn refresh_repo_summaries(&mut self, repo_ids: Vec<RepoId>) -> Vec<Event> {
        if repo_ids.is_empty() {
            return Vec::new();
        }

        let worker_limit = cmp::min(SUMMARY_REFRESH_WORKER_LIMIT, repo_ids.len());
        let (sender, receiver) = mpsc::channel();
        let mut pending = VecDeque::from(repo_ids);
        let mut active_workers = 0usize;
        let mut follow_up_events = Vec::new();

        while active_workers < worker_limit {
            if let Some(repo_id) = pending.pop_front() {
                let job_id = summary_refresh_job_id(&repo_id);
                follow_up_events.push(Event::Worker(WorkerEvent::RepoSummaryRefreshStarted {
                    job_id: job_id.clone(),
                    repo_id: repo_id.clone(),
                }));
                spawn_summary_refresh_worker(self.git.clone(), sender.clone(), repo_id, job_id);
                active_workers += 1;
            } else {
                break;
            }
        }
        while active_workers > 0 {
            let Ok(outcome) = receiver.recv() else {
                break;
            };
            active_workers = active_workers.saturating_sub(1);
            self.diagnostics.extend_snapshot(outcome.diagnostics);

            match outcome.result {
                Ok(summary) => {
                    let summary = self.workspace.register_summary(summary);
                    follow_up_events.push(Event::Worker(WorkerEvent::RepoSummaryUpdated {
                        job_id: outcome.job_id,
                        summary,
                    }));
                }
                Err(error) => {
                    follow_up_events.push(Event::Worker(WorkerEvent::RepoSummaryRefreshFailed {
                        job_id: outcome.job_id,
                        repo_id: outcome.repo_id,
                        error: error.to_string(),
                    }));
                }
            }

            if let Some(repo_id) = pending.pop_front() {
                let job_id = summary_refresh_job_id(&repo_id);
                follow_up_events.push(Event::Worker(WorkerEvent::RepoSummaryRefreshStarted {
                    job_id: job_id.clone(),
                    repo_id: repo_id.clone(),
                }));
                spawn_summary_refresh_worker(self.git.clone(), sender.clone(), repo_id, job_id);
                active_workers += 1;
            }
        }

        follow_up_events
    }
}

#[derive(Debug)]
struct SummaryRefreshOutcome {
    job_id: JobId,
    repo_id: RepoId,
    result: GitResult<RepoSummary>,
    diagnostics: DiagnosticsSnapshot,
}

fn spawn_summary_refresh_worker(
    mut git: GitFacade,
    sender: mpsc::Sender<SummaryRefreshOutcome>,
    repo_id: RepoId,
    job_id: JobId,
) {
    thread::spawn(move || {
        let result = git.read_repo_summary(RepoSummaryRequest {
            repo_id: repo_id.clone(),
        });
        let diagnostics = git.diagnostics();
        let _ = sender.send(SummaryRefreshOutcome {
            job_id,
            repo_id,
            result,
            diagnostics,
        });
    });
}

fn summary_refresh_job_id(repo_id: &RepoId) -> JobId {
    JobId::new(format!("summary-refresh:{}", repo_id.0))
}

fn git_command_summary(command: &GitCommand) -> &'static str {
    match command {
        GitCommand::StageSelection => "stage_selection",
        GitCommand::StageFile { .. } => "stage_file",
        GitCommand::DiscardFile { .. } => "discard_file",
        GitCommand::UnstageFile { .. } => "unstage_file",
        GitCommand::CommitStaged { .. } => "commit_staged",
        GitCommand::AmendHead { .. } => "amend_head",
        GitCommand::StartCommitRebase { mode, .. } => match mode {
            super_lazygit_core::RebaseStartMode::Interactive => "start_interactive_rebase",
            super_lazygit_core::RebaseStartMode::Amend => "start_amend_rebase",
            super_lazygit_core::RebaseStartMode::Fixup => "start_fixup_rebase",
            super_lazygit_core::RebaseStartMode::Reword { .. } => "start_reword_rebase",
        },
        GitCommand::CherryPickCommit { .. } => "cherry_pick_commit",
        GitCommand::RevertCommit { .. } => "revert_commit",
        GitCommand::ResetToCommit { mode, .. } => match mode {
            super_lazygit_core::ResetMode::Soft => "reset_to_commit_soft",
            super_lazygit_core::ResetMode::Mixed => "reset_to_commit_mixed",
            super_lazygit_core::ResetMode::Hard => "reset_to_commit_hard",
        },
        GitCommand::RestoreSnapshot { .. } => "restore_snapshot",
        GitCommand::ContinueRebase => "continue_rebase",
        GitCommand::AbortRebase => "abort_rebase",
        GitCommand::SkipRebase => "skip_rebase",
        GitCommand::CheckoutBranch { .. } => "checkout_branch",
        GitCommand::CreateBranch { .. } => "create_branch",
        GitCommand::RenameBranch { .. } => "rename_branch",
        GitCommand::DeleteBranch { .. } => "delete_branch",
        GitCommand::ApplyStash { .. } => "apply_stash",
        GitCommand::DropStash { .. } => "drop_stash",
        GitCommand::CreateWorktree { .. } => "create_worktree",
        GitCommand::RemoveWorktree { .. } => "remove_worktree",
        GitCommand::SetBranchUpstream { .. } => "set_branch_upstream",
        GitCommand::FetchSelectedRepo => "fetch_selected_repo",
        GitCommand::PullCurrentBranch => "pull_current_branch",
        GitCommand::PushCurrentBranch => "push_current_branch",
        GitCommand::NukeWorkingTree => "nuke_working_tree",
        GitCommand::RefreshSelectedRepo => "refresh_selected_repo",
    }
}

fn patch_selection_summary(job: &PatchSelectionJob) -> &'static str {
    match job.mode {
        PatchApplicationMode::Stage => "stage_selected_hunk",
        PatchApplicationMode::Unstage => "unstage_selected_hunk",
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
