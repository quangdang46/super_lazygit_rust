use std::cmp;
use std::collections::VecDeque;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ratatui::Frame;

use crate::watcher::{NullWatcherBackend, WatchRegistration, WatcherBackend};
use super_lazygit_core::{
    AppWatcherEvent, Diagnostics, DiagnosticsSnapshot, Effect, Event, GitCommand, JobId,
    PatchApplicationMode, PatchSelectionJob, RepoId, RepoSummary, TimerEvent, Timestamp,
    WorkerEvent,
};
use super_lazygit_git::{
    GitCommandOutcome, GitError, GitFacade, GitResult, PatchSelectionRequest, RepoDetailRequest,
    RepoSummaryRequest, WorkspaceScanRequest,
};
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;
use tempfile::tempdir;

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

    pub fn draw_frame(&mut self, frame: &mut Frame<'_>) {
        self.app.draw_frame(frame);
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
                Effect::OpenEditor { cwd, target } => {
                    if let Some(event) = self.open_editor(cwd, target) {
                        follow_up_events.push(event);
                    }
                }
                Effect::RunGitCommand(request) => {
                    let summary = git_command_summary(&request.command);
                    follow_up_events.push(Event::Worker(WorkerEvent::GitOperationStarted {
                        job_id: request.job_id.clone(),
                        repo_id: request.repo_id.clone(),
                        summary: summary.to_string(),
                    }));

                    let result = self
                        .run_interactive_git_command(request)
                        .unwrap_or_else(|| self.git.run_command(request.clone()));
                    self.diagnostics.extend_snapshot(self.git.diagnostics());

                    match result {
                        Ok(outcome) => {
                            follow_up_events.push(Event::Worker(
                                WorkerEvent::GitOperationCompleted {
                                    job_id: request.job_id.clone(),
                                    repo_id: outcome.repo_id,
                                    summary: outcome.summary,
                                },
                            ));
                        }
                        Err(error) => {
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

    fn open_editor(&self, cwd: &PathBuf, target: &PathBuf) -> Option<Event> {
        let editor = &self.app.config().editor;
        if editor.command.trim().is_empty() {
            return Some(Event::Worker(WorkerEvent::EditorLaunchFailed {
                error: "Editor command is empty.".to_string(),
            }));
        }

        let mut command = Command::new(&editor.command);
        command.current_dir(cwd);
        command.args(&editor.args);
        command.arg(target);

        crate::terminal::run_external_command(&mut command)
            .err()
            .map(|error| {
                Event::Worker(WorkerEvent::EditorLaunchFailed {
                    error: format!(
                        "Failed to open {} in the configured editor: {error}",
                        target.display()
                    ),
                })
            })
    }

    fn run_interactive_git_command(
        &mut self,
        request: &super_lazygit_core::GitCommandRequest,
    ) -> Option<GitResult<GitCommandOutcome>> {
        match &request.command {
            GitCommand::CommitStagedWithEditor => {
                Some(self.commit_staged_with_editor(&request.repo_id))
            }
            GitCommand::RewordCommitWithEditor { commit } => {
                Some(self.reword_commit_with_editor(&request.repo_id, commit))
            }
            _ => None,
        }
    }

    fn commit_staged_with_editor(&mut self, repo_id: &RepoId) -> GitResult<GitCommandOutcome> {
        let repo_path = self.repo_path(repo_id);
        let mut command = Command::new("git");
        command.arg("commit").current_dir(&repo_path);

        let result = crate::terminal::run_external_command_named(&mut command, "git")
            .map_err(io_error_to_git);
        self.git
            .record_operation("interactive_commit_staged_with_editor", result.is_ok());

        result?;
        Ok(GitCommandOutcome {
            repo_id: repo_id.clone(),
            summary: "Committed staged changes with editor".to_string(),
        })
    }

    fn reword_commit_with_editor(
        &mut self,
        repo_id: &RepoId,
        commit: &str,
    ) -> GitResult<GitCommandOutcome> {
        let repo_path = self.repo_path(repo_id);
        let tempdir = tempdir().map_err(io_error_to_git)?;
        let sequence_editor = tempdir.path().join("sequence-editor.sh");
        write_executable_script(
            &sequence_editor,
            "#!/bin/sh\nset -eu\nfile=\"$1\"\ntmp=\"$1.tmp\"\nawk 'BEGIN{done=0} { if (!done && $1 == \"pick\") { sub(/^pick /, \"reword \"); done=1 } print }' \"$file\" > \"$tmp\"\nmv \"$tmp\" \"$file\"\n",
        )?;

        let mut command = Command::new("git");
        command.arg("rebase").arg("-i").current_dir(&repo_path);
        command.env("GIT_SEQUENCE_EDITOR", &sequence_editor);
        for arg in rebase_base_args(&repo_path, commit)? {
            command.arg(arg);
        }

        let result = crate::terminal::run_external_command_named(&mut command, "git")
            .map_err(io_error_to_git);
        self.git
            .record_operation("interactive_reword_commit_with_editor", result.is_ok());

        result?;
        Ok(GitCommandOutcome {
            repo_id: repo_id.clone(),
            summary: "Reworded selected commit with editor".to_string(),
        })
    }

    fn repo_path(&self, repo_id: &RepoId) -> PathBuf {
        self.workspace
            .repo_path(repo_id)
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&repo_id.0))
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
        GitCommand::CommitStagedWithEditor => "commit_staged_with_editor",
        GitCommand::AmendHead { .. } => "amend_head",
        GitCommand::RewordCommitWithEditor { .. } => "reword_commit_with_editor",
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

fn rebase_base_args(repo_path: &std::path::Path, commit: &str) -> GitResult<Vec<String>> {
    let parent_spec = format!("{commit}^");
    let output = Command::new("git")
        .args(["rev-parse", parent_spec.as_str()])
        .current_dir(repo_path)
        .output()
        .map_err(io_error_to_git)?;
    if output.status.success() {
        Ok(vec![String::from_utf8(output.stdout)
            .map_err(|error| GitError::OperationFailed {
                message: error.to_string(),
            })?
            .trim()
            .to_string()])
    } else {
        Ok(vec!["--root".to_string()])
    }
}

fn write_executable_script(path: &std::path::Path, contents: &str) -> GitResult<()> {
    fs::write(path, contents).map_err(io_error_to_git)?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path).map_err(io_error_to_git)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(io_error_to_git)?;
    }
    Ok(())
}

fn io_error_to_git(error: std::io::Error) -> GitError {
    GitError::OperationFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use super_lazygit_config::AppConfig;
    use super_lazygit_core::{AppState, GitCommandRequest};
    use super_lazygit_test_support::{history_preview_repo, staged_and_unstaged_repo};

    #[test]
    fn interactive_commit_uses_git_editor() -> io::Result<()> {
        let repo = staged_and_unstaged_repo()?;
        repo.write_file(
            "editor.sh",
            "#!/bin/sh\nprintf 'editor commit\\n' > \"$1\"\n",
        )?;
        repo.git(["config", "core.editor", "sh editor.sh"])?;

        let repo_id = RepoId::new(repo.path().display().to_string());
        let mut runtime = AppRuntime::new(
            TuiApp::new(AppState::default(), AppConfig::default()),
            WorkspaceRegistry::new(Some(repo.path().to_path_buf())),
            GitFacade::default(),
        );

        let events = runtime.apply_effects(&[Effect::RunGitCommand(GitCommandRequest {
            job_id: JobId::new("git:repo:commit-staged-editor"),
            repo_id: repo_id.clone(),
            command: GitCommand::CommitStagedWithEditor,
        })]);

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Worker(WorkerEvent::GitOperationCompleted { summary, .. })
            if summary == "Committed staged changes with editor"
        )));

        let log = String::from_utf8(repo.git_capture(["log", "--format=%s", "-n", "1"])?.stdout)
            .map_err(io::Error::other)?;
        assert_eq!(log.trim(), "editor commit");
        Ok(())
    }

    #[test]
    fn interactive_reword_uses_git_editor() -> io::Result<()> {
        let repo = history_preview_repo()?;
        repo.write_file(
            "editor.sh",
            "#!/bin/sh\nprintf 'rewritten second\\n\\n' > \"$1\"\n",
        )?;
        repo.git(["config", "core.editor", "sh editor.sh"])?;

        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1")?;
        let mut runtime = AppRuntime::new(
            TuiApp::new(AppState::default(), AppConfig::default()),
            WorkspaceRegistry::new(Some(repo.path().to_path_buf())),
            GitFacade::default(),
        );

        let events = runtime.apply_effects(&[Effect::RunGitCommand(GitCommandRequest {
            job_id: JobId::new("git:repo:reword-commit-editor"),
            repo_id: repo_id.clone(),
            command: GitCommand::RewordCommitWithEditor { commit: target },
        })]);

        assert!(events.iter().any(|event| matches!(
            event,
            Event::Worker(WorkerEvent::GitOperationCompleted { summary, .. })
            if summary == "Reworded selected commit with editor"
        )));

        let log = String::from_utf8(repo.git_capture(["log", "--format=%s", "-n", "3"])?.stdout)
            .map_err(io::Error::other)?;
        assert!(log.lines().any(|line| line == "rewritten second"));
        Ok(())
    }
}
