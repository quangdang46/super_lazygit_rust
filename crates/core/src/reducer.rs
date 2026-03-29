use crate::action::Action;
use crate::effect::{
    Effect, GitCommand, GitCommandRequest, PatchApplicationMode, PatchSelectionJob,
};
use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
use crate::state::{
    AppMode, AppState, BackgroundJob, BackgroundJobKind, BackgroundJobState, CommitBoxMode,
    ComparisonTarget, ConfirmableOperation, DiffPresentation, JobId, MessageLevel, Notification,
    OperationProgress, PaneId, RepoModeState, ScanStatus, SelectedHunk, StatusMessage,
    WatcherHealth,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReduceResult {
    pub state: AppState,
    pub effects: Vec<Effect>,
}

pub fn reduce(state: AppState, event: Event) -> ReduceResult {
    let mut state = state;
    let mut effects = Vec::new();

    match event {
        Event::Action(action) => reduce_action(&mut state, action, &mut effects),
        Event::Worker(event) => reduce_worker_event(&mut state, event, &mut effects),
        Event::Watcher(event) => reduce_watcher_event(&mut state, event, &mut effects),
        Event::Timer(event) => reduce_timer_event(&mut state, event, &mut effects),
        Event::Input(_) => {}
    }

    ReduceResult { state, effects }
}

fn reduce_action(state: &mut AppState, action: Action, effects: &mut Vec<Effect>) {
    match action {
        Action::EnterRepoMode { repo_id } => {
            state.mode = AppMode::Repository;
            state.focused_pane = PaneId::RepoUnstaged;
            state.workspace.search_focused = false;
            state.workspace.selected_repo_id = Some(repo_id.clone());
            push_recent_repo(state, repo_id.clone());
            state.repo_mode = Some(RepoModeState::new(repo_id.clone()));
            effects.push(Effect::LoadRepoDetail {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
            });
            effects.push(Effect::ScheduleRender);
        }
        Action::LeaveRepoMode => {
            state.mode = AppMode::Workspace;
            state.focused_pane = PaneId::WorkspaceList;
            state.workspace.search_focused = false;
            state.repo_mode = None;
            effects.push(Effect::ScheduleRender);
        }
        Action::SelectNextRepo => {
            if state.workspace.select_next().is_some() {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SelectPreviousRepo => {
            if state.workspace.select_previous().is_some() {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::FocusWorkspaceSearch => {
            state.workspace.search_focused = true;
            effects.push(Effect::ScheduleRender);
        }
        Action::BlurWorkspaceSearch => {
            state.workspace.search_focused = false;
            effects.push(Effect::ScheduleRender);
        }
        Action::CancelWorkspaceSearch => {
            state.workspace.search_focused = false;
            if !state.workspace.search_query.is_empty() {
                state.workspace.search_query.clear();
                state.workspace.ensure_visible_selection();
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::AppendWorkspaceSearch { text } => {
            if !text.is_empty() {
                state.workspace.search_focused = true;
                state.workspace.search_query.push_str(&text);
                state.workspace.ensure_visible_selection();
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::BackspaceWorkspaceSearch => {
            if state.workspace.search_query.pop().is_some() {
                state.workspace.ensure_visible_selection();
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CycleWorkspaceFilter => {
            state.workspace.filter_mode = state.workspace.filter_mode.cycle_next();
            state.workspace.ensure_visible_selection();
            effects.push(Effect::ScheduleRender);
        }
        Action::CycleWorkspaceSort => {
            state.workspace.sort_mode = state.workspace.sort_mode.cycle_next();
            state.workspace.ensure_visible_selection();
            effects.push(Effect::ScheduleRender);
        }
        Action::SelectNextStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_status_selection(repo_mode, state.focused_pane, 1) {
                    if let Some((selected_path, diff_presentation)) =
                        selected_status_detail_request(repo_mode, state.focused_pane)
                    {
                        effects.push(Effect::LoadRepoDetail {
                            repo_id: repo_mode.current_repo_id.clone(),
                            selected_path: Some(selected_path),
                            diff_presentation,
                        });
                    }
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_status_selection(repo_mode, state.focused_pane, -1) {
                    if let Some((selected_path, diff_presentation)) =
                        selected_status_detail_request(repo_mode, state.focused_pane)
                    {
                        effects.push(Effect::LoadRepoDetail {
                            repo_id: repo_mode.current_repo_id.clone(),
                            selected_path: Some(selected_path),
                            diff_presentation,
                        });
                    }
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectNextCommit => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_commit_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousCommit => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_commit_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::ScrollRepoDetailUp => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.diff_scroll = repo_mode.diff_scroll.saturating_sub(1);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ScrollRepoDetailDown => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                let max_scroll = repo_mode
                    .detail
                    .as_ref()
                    .map_or(0, |detail| detail.diff.lines.len().saturating_sub(1));
                repo_mode.diff_scroll = (repo_mode.diff_scroll + 1).min(max_scroll);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SelectNextDiffHunk => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_diff_hunk_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousDiffHunk => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_diff_hunk_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SetFocusedPane(pane) => {
            state.focused_pane = pane;
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenModal { kind, title } => {
            state
                .modal_stack
                .push(crate::state::Modal::new(kind, title));
            state.focused_pane = PaneId::Modal;
            effects.push(Effect::ScheduleRender);
        }
        Action::CloseTopModal => {
            if state
                .modal_stack
                .last()
                .is_some_and(|modal| matches!(modal.kind, crate::state::ModalKind::Confirm))
            {
                state.pending_confirmation = None;
            }
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = match state.mode {
                    AppMode::Workspace => PaneId::WorkspaceList,
                    AppMode::Repository => PaneId::RepoUnstaged,
                };
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::ConfirmPendingOperation => {
            if let Some(job) = confirm_pending_operation(state) {
                effects.push(Effect::RunGitCommand(job));
            } else {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::RefreshSelectedRepo => {
            if let Some(repo_id) = state.workspace.selected_repo_id.clone() {
                effects.push(Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                });
                if matches!(state.mode, AppMode::Repository) {
                    effects.push(load_repo_detail_effect(state, repo_id));
                }
            }
        }
        Action::RefreshVisibleRepos => {
            state.workspace.scan_status = ScanStatus::Scanning;
            effects.push(Effect::ScheduleRender);
            effects.push(Effect::StartRepoScan);
        }
        Action::StageSelection => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::StageSelection,
                );
                enqueue_git_job(state, &job, "Stage selection");
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::StageSelectedFile => {
            if let Some(repo_mode) = &state.repo_mode {
                if let Some(path) = selected_status_path(repo_mode, PaneId::RepoUnstaged) {
                    let summary = format!("Stage {}", path.display());
                    let job = git_job(
                        repo_mode.current_repo_id.clone(),
                        GitCommand::StageFile { path },
                    );
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
            }
        }
        Action::UnstageSelectedFile => {
            if let Some(repo_mode) = &state.repo_mode {
                if let Some(path) = selected_status_path(repo_mode, PaneId::RepoStaged) {
                    let summary = format!("Unstage {}", path.display());
                    let job = git_job(
                        repo_mode.current_repo_id.clone(),
                        GitCommand::UnstageFile { path },
                    );
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
            }
        }
        Action::StageSelectedHunk => {
            if let Some(job) = selected_hunk_patch_job(state, PatchApplicationMode::Stage) {
                let summary = format!("Stage hunk in {}", job.path.display());
                enqueue_patch_job(state, &job, &summary);
                effects.push(Effect::RunPatchSelection(job));
            }
        }
        Action::UnstageSelectedHunk => {
            if let Some(job) = selected_hunk_patch_job(state, PatchApplicationMode::Unstage) {
                let summary = format!("Unstage hunk in {}", job.path.display());
                enqueue_patch_job(state, &job, &summary);
                effects.push(Effect::RunPatchSelection(job));
            }
        }
        Action::OpenCommitBox { mode } => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.commit_box.focused = true;
                repo_mode.commit_box.mode = mode;
                state.focused_pane = PaneId::RepoStaged;
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CancelCommitBox => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                close_commit_box(repo_mode);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::AppendCommitInput { text } => {
            if let Some(input) = commit_input_mut(state) {
                input.push_str(&text);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::BackspaceCommitInput => {
            if let Some(input) = commit_input_mut(state) {
                if input.pop().is_some() {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SubmitCommitBox => {
            if let Some(job) = submit_commit_box(state) {
                effects.push(Effect::RunGitCommand(job));
            } else {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CommitStaged { message } => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::CommitStaged {
                        message: message.clone(),
                    },
                );
                enqueue_git_job(state, &job, "Commit staged changes");
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::AmendHead { message } => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::AmendHead {
                        message: message.clone(),
                    },
                );
                enqueue_git_job(state, &job, "Amend HEAD commit");
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::CheckoutBranch { branch_ref } => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::CheckoutBranch {
                        branch_ref: branch_ref.clone(),
                    },
                );
                enqueue_git_job(state, &job, "Checkout branch");
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::FetchSelectedRepo => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_confirmation_modal(state, repo_id, ConfirmableOperation::Fetch);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::PullCurrentBranch => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_confirmation_modal(state, repo_id, ConfirmableOperation::Pull);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::PushCurrentBranch => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_confirmation_modal(state, repo_id, ConfirmableOperation::Push);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SwitchRepoSubview(subview) => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.active_subview = subview;
                repo_mode.diff_scroll = 0;
                if !matches!(subview, crate::state::RepoSubview::Status) {
                    close_commit_box(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Commits) {
                    sync_commit_selection(repo_mode);
                }
                state.focused_pane = PaneId::RepoDetail;
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ApplyWorkspaceScan(workspace) => {
            state.workspace = workspace;
            state.workspace.ensure_visible_selection();
            effects.push(Effect::ScheduleRender);
        }
    }
}

fn reduce_worker_event(state: &mut AppState, event: WorkerEvent, effects: &mut Vec<Effect>) {
    match event {
        WorkerEvent::RepoScanFailed { root, error } => {
            if let Some(root) = root {
                state.workspace.current_root = Some(root);
            }
            state.workspace.scan_status = ScanStatus::Failed {
                message: error.clone(),
            };
            state.notifications.push_back(Notification {
                id: 0,
                level: MessageLevel::Error,
                text: format!("Workspace scan failed: {error}"),
                expires_at: None,
            });
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoScanCompleted {
            root,
            repo_ids,
            scanned_at,
        } => {
            state.workspace.current_root = root;
            state.workspace.discovered_repo_ids = repo_ids;
            state.workspace.scan_status = ScanStatus::Complete {
                scanned_repos: state.workspace.discovered_repo_ids.len(),
            };
            state.workspace.last_full_refresh_at = Some(scanned_at);
            state.workspace.ensure_visible_selection();
            effects.push(Effect::ConfigureWatcher {
                repo_ids: state.workspace.discovered_repo_ids.clone(),
            });
            effects.push(Effect::RefreshRepoSummaries {
                repo_ids: state.workspace.discovered_repo_ids.clone(),
            });
            effects.push(Effect::PersistCache);
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoSummaryRefreshStarted { job_id, repo_id } => {
            state.background_jobs.insert(
                job_id.clone(),
                BackgroundJob {
                    id: job_id,
                    kind: BackgroundJobKind::RepoRefresh,
                    target_repo: Some(repo_id),
                    state: BackgroundJobState::Running,
                },
            );
        }
        WorkerEvent::RepoSummaryUpdated { job_id, summary } => {
            complete_job(state, &job_id, BackgroundJobState::Succeeded);
            state
                .workspace
                .repo_summaries
                .insert(summary.repo_id.clone(), summary);
            state.workspace.ensure_visible_selection();
            effects.push(Effect::PersistCache);
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoSummaryRefreshFailed {
            job_id,
            repo_id,
            error,
        } => {
            complete_job(
                state,
                &job_id,
                BackgroundJobState::Failed {
                    error: error.clone(),
                },
            );
            let summary = state
                .workspace
                .repo_summaries
                .entry(repo_id.clone())
                .or_insert_with(|| repo_summary_placeholder(&repo_id));
            summary.last_error = Some(error.clone());
            state.notifications.push_back(Notification {
                id: 0,
                level: MessageLevel::Error,
                text: error,
                expires_at: None,
            });
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoDetailLoaded { repo_id, detail } => {
            if state
                .repo_mode
                .as_ref()
                .is_some_and(|repo_mode| repo_mode.current_repo_id == repo_id)
            {
                if let Some(repo_mode) = state.repo_mode.as_mut() {
                    let commit_input = repo_mode
                        .detail
                        .as_ref()
                        .map(|detail| detail.commit_input.clone())
                        .unwrap_or_default();
                    let mut detail = detail;
                    detail.commit_input = commit_input;
                    repo_mode.detail = Some(detail);
                    sync_status_selection(repo_mode);
                    sync_commit_selection(repo_mode);
                    sync_diff_selection(repo_mode);
                    repo_mode.operation_progress = OperationProgress::Idle;
                }
            }
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::GitOperationStarted {
            job_id,
            repo_id,
            summary,
        } => {
            state.background_jobs.insert(
                job_id.clone(),
                BackgroundJob {
                    id: job_id.clone(),
                    kind: BackgroundJobKind::GitCommand,
                    target_repo: Some(repo_id),
                    state: BackgroundJobState::Running,
                },
            );
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.operation_progress = OperationProgress::Running { job_id, summary };
            }
        }
        WorkerEvent::GitOperationCompleted {
            job_id,
            repo_id,
            summary,
        } => {
            complete_job(state, &job_id, BackgroundJobState::Succeeded);
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if repo_mode.current_repo_id == repo_id {
                    repo_mode.operation_progress = OperationProgress::Idle;
                }
            }
            state
                .status_messages
                .push_back(StatusMessage::info(0, summary));
            effects.push(Effect::RefreshRepoSummary {
                repo_id: repo_id.clone(),
            });
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::GitOperationFailed {
            job_id,
            repo_id,
            error,
        } => {
            complete_job(
                state,
                &job_id,
                BackgroundJobState::Failed {
                    error: error.clone(),
                },
            );
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if repo_mode.current_repo_id == repo_id {
                    repo_mode.operation_progress = OperationProgress::Failed {
                        summary: error.clone(),
                    };
                }
            }
            state.notifications.push_back(Notification {
                id: 0,
                level: MessageLevel::Error,
                text: error,
                expires_at: None,
            });
            effects.push(Effect::ScheduleRender);
        }
    }
}

fn reduce_watcher_event(state: &mut AppState, event: WatcherEvent, effects: &mut Vec<Effect>) {
    match event {
        WatcherEvent::RepoInvalidated { repo_id } => {
            *state
                .workspace
                .pending_watcher_invalidations
                .entry(repo_id)
                .or_insert(0) += 1;
            if !state.workspace.watcher_debounce_pending {
                state.workspace.watcher_debounce_pending = true;
                effects.push(Effect::ScheduleWatcherDebounce);
            }
        }
        WatcherEvent::WatcherDegraded { message } => {
            state.workspace.watcher_health = WatcherHealth::Degraded { message };
            for summary in state.workspace.repo_summaries.values_mut() {
                summary.watcher_freshness = crate::state::WatcherFreshness::Stale;
            }
            effects.push(Effect::ScheduleRender);
        }
        WatcherEvent::WatcherRecovered => {
            state.workspace.watcher_health = WatcherHealth::Healthy;
            effects.push(Effect::ScheduleRender);
        }
    }
}

fn reduce_timer_event(state: &mut AppState, event: TimerEvent, effects: &mut Vec<Effect>) {
    match event {
        TimerEvent::PeriodicRefreshTick => {
            if matches!(
                state.workspace.scan_status,
                ScanStatus::Idle | ScanStatus::Complete { .. }
            ) {
                state.workspace.scan_status = ScanStatus::Scanning;
            }

            if matches!(
                state.workspace.watcher_health,
                WatcherHealth::Degraded { .. }
            ) {
                if state.workspace.discovered_repo_ids.is_empty() {
                    effects.push(Effect::StartRepoScan);
                } else {
                    let active_repo_id = state
                        .repo_mode
                        .as_ref()
                        .map(|repo_mode| &repo_mode.current_repo_id);
                    let repo_ids = state
                        .workspace
                        .prioritized_repo_ids(&state.workspace.discovered_repo_ids, active_repo_id);
                    effects.push(Effect::RefreshRepoSummaries { repo_ids });
                    if let Some(repo_id) = active_repo_id.cloned() {
                        effects.push(load_repo_detail_effect(state, repo_id));
                    }
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        TimerEvent::PeriodicFetchTick => {}
        TimerEvent::WatcherDebounceFlush => {
            let pending_repo_ids = state
                .workspace
                .pending_watcher_invalidations
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            let active_repo_id = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| &repo_mode.current_repo_id);
            let repo_ids = state
                .workspace
                .prioritized_repo_ids(&pending_repo_ids, active_repo_id);
            state.workspace.pending_watcher_invalidations.clear();
            state.workspace.watcher_debounce_pending = false;

            for repo_id in repo_ids {
                effects.push(Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                });
                if state
                    .repo_mode
                    .as_ref()
                    .is_some_and(|repo_mode| repo_mode.current_repo_id == repo_id)
                {
                    effects.push(load_repo_detail_effect(state, repo_id));
                }
            }
        }
        TimerEvent::ToastExpiryTick { now } => {
            state.notifications.retain(|notification| {
                notification
                    .expires_at
                    .is_none_or(|expires_at| expires_at > now)
            });
        }
    }
}

fn push_recent_repo(state: &mut AppState, repo_id: crate::state::RepoId) {
    state
        .recent_repo_stack
        .retain(|candidate| candidate != &repo_id);
    state.recent_repo_stack.push(repo_id);
}

fn step_status_selection(repo_mode: &mut RepoModeState, focused_pane: PaneId, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return false;
    };

    let len = status_entries_len(detail, focused_pane);
    if len == 0 {
        return false;
    }

    match focused_pane {
        PaneId::RepoUnstaged => repo_mode.status_view.select_with_step(len, step).is_some(),
        PaneId::RepoStaged => repo_mode.staged_view.select_with_step(len, step).is_some(),
        _ => false,
    }
}

fn step_commit_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_mut() else {
        return false;
    };

    let Some(selected) = repo_mode
        .commits_view
        .select_with_step(detail.commits.len(), step)
    else {
        detail.comparison_target = None;
        return false;
    };

    repo_mode.diff_scroll = 0;
    detail.comparison_target = detail
        .commits
        .get(selected)
        .map(|commit| ComparisonTarget::Commit(commit.oid.clone()));
    true
}

fn step_diff_hunk_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_mut() else {
        return false;
    };

    let diff = &mut detail.diff;
    if diff.presentation == DiffPresentation::Comparison {
        return false;
    }
    let len = diff.hunks.len();
    if len == 0 {
        diff.selected_hunk = None;
        return false;
    }
    let current = diff.selected_hunk.filter(|index| *index < len).unwrap_or(0);
    let selected = (current as isize + step).rem_euclid(len as isize) as usize;
    diff.selected_hunk = Some(selected);

    repo_mode.diff_scroll = detail
        .diff
        .hunks
        .get(selected)
        .map_or(0, |hunk| hunk.start_line_index);
    true
}

fn close_commit_box(repo_mode: &mut RepoModeState) {
    repo_mode.commit_box.focused = false;
    if let Some(detail) = repo_mode.detail.as_mut() {
        detail.commit_input.clear();
    }
}

fn commit_input_mut(state: &mut AppState) -> Option<&mut String> {
    state.repo_mode.as_mut().and_then(|repo_mode| {
        if repo_mode.commit_box.focused {
            repo_mode
                .detail
                .as_mut()
                .map(|detail| &mut detail.commit_input)
        } else {
            None
        }
    })
}

fn staged_file_count(detail: &crate::state::RepoDetail) -> usize {
    detail
        .file_tree
        .iter()
        .filter(|item| item.staged_kind.is_some())
        .count()
}

fn push_warning(state: &mut AppState, text: impl Into<String>) {
    state.notifications.push_back(Notification {
        id: 0,
        level: MessageLevel::Warning,
        text: text.into(),
        expires_at: None,
    });
}

fn submit_commit_box(state: &mut AppState) -> Option<GitCommandRequest> {
    let (repo_id, mode, message, staged_count, has_commits) =
        state.repo_mode.as_ref().and_then(|repo_mode| {
            if !repo_mode.commit_box.focused {
                return None;
            }
            repo_mode.detail.as_ref().map(|detail| {
                (
                    repo_mode.current_repo_id.clone(),
                    repo_mode.commit_box.mode,
                    detail.commit_input.trim().to_string(),
                    staged_file_count(detail),
                    !detail.commits.is_empty(),
                )
            })
        })?;

    let command = match mode {
        CommitBoxMode::Commit => {
            if staged_count == 0 {
                push_warning(state, "Stage at least one file before committing.");
                return None;
            }
            if message.is_empty() {
                push_warning(state, "Enter a commit message before confirming.");
                return None;
            }
            GitCommand::CommitStaged { message }
        }
        CommitBoxMode::Amend => {
            if !has_commits {
                push_warning(state, "No commits are available to amend.");
                return None;
            }
            GitCommand::AmendHead {
                message: if message.is_empty() {
                    None
                } else {
                    Some(message)
                },
            }
        }
    };

    if let Some(repo_mode) = state.repo_mode.as_mut() {
        close_commit_box(repo_mode);
    }
    let summary = match mode {
        CommitBoxMode::Commit => "Commit staged changes",
        CommitBoxMode::Amend => "Amend HEAD commit",
    };
    let job = git_job(repo_id, command);
    enqueue_git_job(state, &job, summary);
    Some(job)
}

fn open_confirmation_modal(
    state: &mut AppState,
    repo_id: crate::state::RepoId,
    operation: ConfirmableOperation,
) {
    state.pending_confirmation = Some(crate::state::PendingConfirmation { repo_id, operation });
    state.modal_stack.push(crate::state::Modal::new(
        crate::state::ModalKind::Confirm,
        confirmation_title(operation),
    ));
    state.focused_pane = PaneId::Modal;
}

fn confirmation_title(operation: ConfirmableOperation) -> &'static str {
    match operation {
        ConfirmableOperation::Fetch => "Confirm fetch",
        ConfirmableOperation::Pull => "Confirm pull",
        ConfirmableOperation::Push => "Confirm push",
    }
}

fn confirm_pending_operation(state: &mut AppState) -> Option<GitCommandRequest> {
    let pending = state.pending_confirmation.take()?;
    state.modal_stack.pop();
    if state.modal_stack.is_empty() {
        state.focused_pane = match state.mode {
            AppMode::Workspace => PaneId::WorkspaceList,
            AppMode::Repository => PaneId::RepoUnstaged,
        };
    }

    let (command, summary) = match pending.operation {
        ConfirmableOperation::Fetch => (GitCommand::FetchSelectedRepo, "Fetch remote updates"),
        ConfirmableOperation::Pull => (GitCommand::PullCurrentBranch, "Pull current branch"),
        ConfirmableOperation::Push => (GitCommand::PushCurrentBranch, "Push current branch"),
    };
    let job = git_job(pending.repo_id, command);
    enqueue_git_job(state, &job, summary);
    Some(job)
}

fn sync_commit_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_mut() else {
        repo_mode.commits_view.selected_index = None;
        return;
    };

    let selected = repo_mode
        .commits_view
        .ensure_selection(detail.commits.len());
    detail.comparison_target = selected.and_then(|index| {
        detail
            .commits
            .get(index)
            .map(|commit| ComparisonTarget::Commit(commit.oid.clone()))
    });
}

fn sync_status_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.status_view.selected_index = None;
        repo_mode.staged_view.selected_index = None;
        return;
    };

    let unstaged_len = status_entries_len(detail, PaneId::RepoUnstaged);
    repo_mode.status_view.ensure_selection(unstaged_len);

    let staged_len = status_entries_len(detail, PaneId::RepoStaged);
    repo_mode.staged_view.ensure_selection(staged_len);
}

fn sync_diff_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_mut() else {
        return;
    };

    let len = detail.diff.hunks.len();
    detail.diff.selected_hunk = detail.diff.selected_hunk.filter(|index| *index < len);
    if detail.diff.selected_hunk.is_none() && len > 0 {
        detail.diff.selected_hunk = Some(0);
    }
    repo_mode.diff_scroll = detail
        .diff
        .selected_hunk
        .and_then(|index| detail.diff.hunks.get(index))
        .map_or(0, |hunk| hunk.start_line_index);
}

fn status_entries_len(detail: &crate::state::RepoDetail, pane: PaneId) -> usize {
    detail
        .file_tree
        .iter()
        .filter(|item| match pane {
            PaneId::RepoUnstaged => item.unstaged_kind.is_some(),
            PaneId::RepoStaged => item.staged_kind.is_some(),
            _ => false,
        })
        .count()
}

fn selected_status_path(repo_mode: &RepoModeState, pane: PaneId) -> Option<std::path::PathBuf> {
    let detail = repo_mode.detail.as_ref()?;
    let entries = detail
        .file_tree
        .iter()
        .filter(|item| match pane {
            PaneId::RepoUnstaged => item.unstaged_kind.is_some(),
            PaneId::RepoStaged => item.staged_kind.is_some(),
            _ => false,
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return None;
    }

    let selected_index = match pane {
        PaneId::RepoUnstaged => repo_mode.status_view.selected_index,
        PaneId::RepoStaged => repo_mode.staged_view.selected_index,
        _ => None,
    }
    .filter(|index| *index < entries.len())
    .unwrap_or(0);

    entries.get(selected_index).map(|item| item.path.clone())
}

fn selected_status_detail_request(
    repo_mode: &RepoModeState,
    pane: PaneId,
) -> Option<(std::path::PathBuf, DiffPresentation)> {
    selected_status_path(repo_mode, pane).map(|path| (path, diff_presentation_for_pane(pane)))
}

fn diff_presentation_for_pane(pane: PaneId) -> DiffPresentation {
    match pane {
        PaneId::RepoStaged => DiffPresentation::Staged,
        _ => DiffPresentation::Unstaged,
    }
}

fn load_repo_detail_effect(state: &AppState, repo_id: crate::state::RepoId) -> Effect {
    let (selected_path, diff_presentation) = state
        .repo_mode
        .as_ref()
        .and_then(|repo_mode| {
            repo_mode
                .detail
                .as_ref()
                .map(|detail| (detail.diff.selected_path.clone(), detail.diff.presentation))
        })
        .unwrap_or((None, DiffPresentation::Unstaged));
    Effect::LoadRepoDetail {
        repo_id,
        selected_path,
        diff_presentation,
    }
}

fn repo_summary_placeholder(repo_id: &crate::state::RepoId) -> crate::state::RepoSummary {
    let display_name = std::path::Path::new(&repo_id.0)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(repo_id.0.as_str())
        .to_string();
    crate::state::RepoSummary {
        repo_id: repo_id.clone(),
        display_name,
        real_path: std::path::PathBuf::from(&repo_id.0),
        display_path: repo_id.0.clone(),
        watcher_freshness: crate::state::WatcherFreshness::Stale,
        ..crate::state::RepoSummary::default()
    }
}

fn git_job(repo_id: crate::state::RepoId, command: GitCommand) -> GitCommandRequest {
    let job_id = JobId::new(format!("git:{}:{}", repo_id.0, job_suffix(&command)));
    GitCommandRequest {
        job_id,
        repo_id,
        command,
    }
}

fn patch_job(
    repo_id: crate::state::RepoId,
    path: std::path::PathBuf,
    mode: PatchApplicationMode,
    hunks: Vec<SelectedHunk>,
) -> PatchSelectionJob {
    let job_id = JobId::new(format!(
        "git:{}:{}",
        repo_id.0,
        match mode {
            PatchApplicationMode::Stage => "stage-hunk",
            PatchApplicationMode::Unstage => "unstage-hunk",
        }
    ));
    PatchSelectionJob {
        job_id,
        repo_id,
        path,
        mode,
        hunks,
    }
}

fn selected_hunk_patch_job(
    state: &AppState,
    mode: PatchApplicationMode,
) -> Option<PatchSelectionJob> {
    let repo_mode = state.repo_mode.as_ref()?;
    let detail = repo_mode.detail.as_ref()?;
    let diff = &detail.diff;
    if diff.presentation == DiffPresentation::Comparison {
        return None;
    }

    let expected_presentation = match mode {
        PatchApplicationMode::Stage => DiffPresentation::Unstaged,
        PatchApplicationMode::Unstage => DiffPresentation::Staged,
    };
    if diff.presentation != expected_presentation {
        return None;
    }

    let path = diff.selected_path.clone()?;
    let selected_hunk = diff
        .selected_hunk
        .and_then(|index| diff.hunks.get(index))
        .map(|hunk| hunk.selection)?;

    Some(patch_job(
        repo_mode.current_repo_id.clone(),
        path,
        mode,
        vec![selected_hunk],
    ))
}

fn job_suffix(command: &GitCommand) -> &'static str {
    match command {
        GitCommand::StageSelection => "stage-selection",
        GitCommand::StageFile { .. } => "stage-file",
        GitCommand::UnstageFile { .. } => "unstage-file",
        GitCommand::CommitStaged { .. } => "commit-staged",
        GitCommand::AmendHead { .. } => "amend-head",
        GitCommand::CheckoutBranch { .. } => "checkout-branch",
        GitCommand::FetchSelectedRepo => "fetch-selected-repo",
        GitCommand::PullCurrentBranch => "pull-current-branch",
        GitCommand::PushCurrentBranch => "push-current-branch",
        GitCommand::RefreshSelectedRepo => "refresh-selected-repo",
    }
}

fn enqueue_git_job(state: &mut AppState, job: &GitCommandRequest, summary: &str) {
    state
        .background_jobs
        .insert(job.job_id.clone(), background_job(job));
    state
        .repo_mode
        .as_mut()
        .expect("repo mode exists")
        .operation_progress = OperationProgress::Running {
        job_id: job.job_id.clone(),
        summary: summary.to_string(),
    };
}

fn enqueue_patch_job(state: &mut AppState, job: &PatchSelectionJob, summary: &str) {
    state
        .background_jobs
        .insert(job.job_id.clone(), background_patch_job(job));
    state
        .repo_mode
        .as_mut()
        .expect("repo mode exists")
        .operation_progress = OperationProgress::Running {
        job_id: job.job_id.clone(),
        summary: summary.to_string(),
    };
}

fn background_job(job: &GitCommandRequest) -> BackgroundJob {
    BackgroundJob {
        id: job.job_id.clone(),
        kind: BackgroundJobKind::GitCommand,
        target_repo: Some(job.repo_id.clone()),
        state: BackgroundJobState::Queued,
    }
}

fn background_patch_job(job: &PatchSelectionJob) -> BackgroundJob {
    BackgroundJob {
        id: job.job_id.clone(),
        kind: BackgroundJobKind::GitCommand,
        target_repo: Some(job.repo_id.clone()),
        state: BackgroundJobState::Queued,
    }
}

fn complete_job(state: &mut AppState, job_id: &JobId, next_state: BackgroundJobState) {
    if let Some(job) = state.background_jobs.get_mut(job_id) {
        job.state = next_state;
    }
}

#[cfg(test)]
mod tests {
    use crate::action::Action;
    use crate::effect::{Effect, GitCommand, GitCommandRequest};
    use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
    use crate::state::{
        AppMode, AppState, BackgroundJobKind, BackgroundJobState, CommitBoxMode, CommitFileItem,
        CommitItem, ComparisonTarget, ConfirmableOperation, DiffHunk, DiffLine, DiffLineKind,
        DiffModel, DiffPresentation, FileStatus, FileStatusKind, JobId, MessageLevel, ModalKind,
        PaneId, RepoDetail, RepoId, RepoModeState, RepoSubview, RepoSummary, ScanStatus,
        SelectedHunk, Timestamp, WatcherHealth, WorkspaceFilterMode,
    };

    use super::reduce;

    fn workspace_summary(repo_id: &str) -> RepoSummary {
        RepoSummary {
            repo_id: RepoId::new(repo_id),
            display_name: repo_id.to_string(),
            display_path: repo_id.to_string(),
            ..RepoSummary::default()
        }
    }

    #[test]
    fn enter_repo_mode_creates_repo_state_and_load_effect() {
        let repo_id = RepoId::new("repo-1");

        let result = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        );

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result.state.workspace.selected_repo_id,
            Some(repo_id.clone())
        );
        assert_eq!(result.state.recent_repo_stack, vec![repo_id.clone()]);
        assert_eq!(
            result.effects,
            vec![
                crate::effect::Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
                crate::effect::Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn leave_repo_mode_returns_to_workspace() {
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: RepoId::new("repo-1"),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::LeaveRepoMode));

        assert_eq!(result.state.mode, AppMode::Workspace);
        assert_eq!(result.state.focused_pane, PaneId::WorkspaceList);
        assert!(result.state.repo_mode.is_none());
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn selection_changes_wrap_across_repos() {
        let mut state = AppState::default();
        state.workspace.discovered_repo_ids = vec![RepoId::new("a"), RepoId::new("b")];
        state.workspace.selected_repo_id = Some(RepoId::new("a"));

        let next = reduce(state.clone(), Event::Action(Action::SelectNextRepo));
        assert_eq!(
            next.state.workspace.selected_repo_id,
            Some(RepoId::new("b"))
        );
        assert_eq!(next.effects, vec![Effect::ScheduleRender]);

        let wrapped = reduce(next.state, Event::Action(Action::SelectNextRepo));
        assert_eq!(
            wrapped.state.workspace.selected_repo_id,
            Some(RepoId::new("a"))
        );
        assert_eq!(wrapped.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn workspace_search_actions_filter_and_preserve_query() {
        let repo_alpha = RepoId::new("/tmp/alpha");
        let repo_beta = RepoId::new("/tmp/beta");
        let mut state = AppState::default();
        state.workspace.discovered_repo_ids = vec![repo_alpha.clone(), repo_beta.clone()];
        state
            .workspace
            .repo_summaries
            .insert(repo_alpha.clone(), workspace_summary(&repo_alpha.0));
        state
            .workspace
            .repo_summaries
            .insert(repo_beta.clone(), workspace_summary(&repo_beta.0));
        state.workspace.selected_repo_id = Some(repo_alpha.clone());

        let focused = reduce(state, Event::Action(Action::FocusWorkspaceSearch));
        assert!(focused.state.workspace.search_focused);

        let appended = reduce(
            focused.state,
            Event::Action(Action::AppendWorkspaceSearch {
                text: "bet".to_string(),
            }),
        );
        assert_eq!(appended.state.workspace.search_query, "bet");
        assert_eq!(
            appended.state.workspace.selected_repo_id,
            Some(repo_beta.clone())
        );

        let blurred = reduce(appended.state, Event::Action(Action::BlurWorkspaceSearch));
        assert!(!blurred.state.workspace.search_focused);
        assert_eq!(blurred.state.workspace.search_query, "bet");

        let cancelled = reduce(blurred.state, Event::Action(Action::CancelWorkspaceSearch));
        assert!(!cancelled.state.workspace.search_focused);
        assert!(cancelled.state.workspace.search_query.is_empty());
        assert_eq!(cancelled.state.workspace.selected_repo_id, Some(repo_beta));
    }

    #[test]
    fn cycling_workspace_filter_reselects_first_visible_repo() {
        let repo_clean = RepoId::new("/tmp/clean");
        let repo_dirty = RepoId::new("/tmp/dirty");
        let mut dirty_summary = workspace_summary(&repo_dirty.0);
        dirty_summary.dirty = true;
        let mut state = AppState::default();
        state.workspace.discovered_repo_ids = vec![repo_clean.clone(), repo_dirty.clone()];
        state
            .workspace
            .repo_summaries
            .insert(repo_clean.clone(), workspace_summary(&repo_clean.0));
        state
            .workspace
            .repo_summaries
            .insert(repo_dirty.clone(), dirty_summary);
        state.workspace.selected_repo_id = Some(repo_clean);

        let result = reduce(state, Event::Action(Action::CycleWorkspaceFilter));

        assert_eq!(
            result.state.workspace.filter_mode,
            WorkspaceFilterMode::DirtyOnly
        );
        assert_eq!(result.state.workspace.selected_repo_id, Some(repo_dirty));
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn modal_open_and_close_updates_stack_and_focus() {
        let opened = reduce(
            AppState::default(),
            Event::Action(Action::OpenModal {
                kind: ModalKind::Help,
                title: "Help".to_string(),
            }),
        );

        assert_eq!(opened.state.modal_stack.len(), 1);
        assert_eq!(opened.state.focused_pane, PaneId::Modal);
        assert_eq!(opened.effects, vec![Effect::ScheduleRender]);

        let closed = reduce(opened.state, Event::Action(Action::CloseTopModal));
        assert!(closed.state.modal_stack.is_empty());
        assert_eq!(closed.state.focused_pane, PaneId::WorkspaceList);
        assert_eq!(closed.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn transport_actions_open_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::PullCurrentBranch));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation)),
            Some((repo_id, ConfirmableOperation::Pull))
        );
        assert_eq!(
            result
                .state
                .modal_stack
                .last()
                .map(|modal| (&modal.kind, modal.title.as_str())),
            Some((&ModalKind::Confirm, "Confirm pull"))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn confirm_pending_operation_queues_transport_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Confirm, "Confirm push")],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::Push,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:push-current-branch");

        assert!(result.state.modal_stack.is_empty());
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::PushCurrentBranch,
            })]
        );
    }

    #[test]
    fn closing_confirmation_modal_clears_pending_operation() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Confirm fetch",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: RepoId::new("repo-1"),
                operation: ConfirmableOperation::Fetch,
            }),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::CloseTopModal));

        assert!(result.state.modal_stack.is_empty());
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(result.state.focused_pane, PaneId::WorkspaceList);
    }

    fn repo_detail_with_file_tree() -> RepoDetail {
        RepoDetail {
            file_tree: vec![
                FileStatus {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: Some(FileStatusKind::Modified),
                    unstaged_kind: Some(FileStatusKind::Modified),
                },
                FileStatus {
                    path: std::path::PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Untracked),
                },
                FileStatus {
                    path: std::path::PathBuf::from("Cargo.toml"),
                    kind: FileStatusKind::Added,
                    staged_kind: Some(FileStatusKind::Added),
                    unstaged_kind: None,
                },
            ],
            ..RepoDetail::default()
        }
    }

    #[test]
    fn watcher_degraded_updates_health() {
        let repo_id = RepoId::new("/tmp/repo");
        let mut state = AppState::default();
        state.workspace.repo_summaries.insert(
            repo_id.clone(),
            RepoSummary {
                repo_id,
                watcher_freshness: crate::state::WatcherFreshness::Fresh,
                ..workspace_summary("/tmp/repo")
            },
        );
        let result = reduce(
            state,
            Event::Watcher(WatcherEvent::WatcherDegraded {
                message: "watch overflow".to_string(),
            }),
        );

        assert_eq!(
            result.state.workspace.watcher_health,
            WatcherHealth::Degraded {
                message: "watch overflow".to_string()
            }
        );
        assert!(result
            .state
            .workspace
            .repo_summaries
            .values()
            .all(|summary| summary.watcher_freshness == crate::state::WatcherFreshness::Stale));
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn repo_scan_completion_sets_selected_repo_when_missing() {
        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoScanCompleted {
                root: None,
                repo_ids: vec![RepoId::new("repo-1"), RepoId::new("repo-2")],
                scanned_at: Timestamp(42),
            }),
        );

        assert_eq!(
            result.state.workspace.selected_repo_id,
            Some(RepoId::new("repo-1"))
        );
        assert_eq!(
            result.state.workspace.last_full_refresh_at,
            Some(Timestamp(42))
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::ConfigureWatcher {
                    repo_ids: vec![RepoId::new("repo-1"), RepoId::new("repo-2")],
                },
                Effect::RefreshRepoSummaries {
                    repo_ids: vec![RepoId::new("repo-1"), RepoId::new("repo-2")],
                },
                Effect::PersistCache,
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn repo_summary_update_is_stored() {
        let summary = RepoSummary {
            display_name: "Repo 1".to_string(),
            ..workspace_summary("repo-1")
        };

        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoSummaryUpdated {
                job_id: JobId::new("summary-refresh:repo-1"),
                summary: summary.clone(),
            }),
        );

        assert_eq!(
            result.state.workspace.repo_summaries.get(&summary.repo_id),
            Some(&summary)
        );
        assert_eq!(
            result.effects,
            vec![Effect::PersistCache, Effect::ScheduleRender]
        );
    }

    #[test]
    fn repo_scan_completion_respects_active_filter_selection() {
        let mut state = AppState::default();
        state.workspace.filter_mode = WorkspaceFilterMode::DirtyOnly;
        state.workspace.selected_repo_id = Some(RepoId::new("/tmp/clean"));
        state
            .workspace
            .repo_summaries
            .insert(RepoId::new("/tmp/clean"), workspace_summary("/tmp/clean"));
        let mut dirty = workspace_summary("/tmp/dirty");
        dirty.dirty = true;
        state
            .workspace
            .repo_summaries
            .insert(RepoId::new("/tmp/dirty"), dirty);

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoScanCompleted {
                root: None,
                repo_ids: vec![RepoId::new("/tmp/clean"), RepoId::new("/tmp/dirty")],
                scanned_at: Timestamp(42),
            }),
        );

        assert_eq!(
            result.state.workspace.selected_repo_id,
            Some(RepoId::new("/tmp/dirty"))
        );
    }

    #[test]
    fn repo_summary_refresh_started_marks_repo_job_running() {
        let repo_id = RepoId::new("repo-1");
        let job_id = JobId::new("summary-refresh:repo-1");

        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoSummaryRefreshStarted {
                job_id: job_id.clone(),
                repo_id: repo_id.clone(),
            }),
        );

        assert_eq!(
            result.state.background_jobs.get(&job_id).map(|job| (
                &job.kind,
                &job.target_repo,
                &job.state
            )),
            Some((
                &BackgroundJobKind::RepoRefresh,
                &Some(repo_id),
                &BackgroundJobState::Running,
            ))
        );
    }

    #[test]
    fn repo_summary_refresh_failed_marks_job_failed_and_notifies() {
        let repo_id = RepoId::new("repo-1");
        let job_id = JobId::new("summary-refresh:repo-1");
        let state = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoSummaryRefreshStarted {
                job_id: job_id.clone(),
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoSummaryRefreshFailed {
                job_id: job_id.clone(),
                repo_id,
                error: "boom".to_string(),
            }),
        );

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Failed {
                error: "boom".to_string(),
            })
        );
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| (&notification.level, notification.text.as_str())),
            Some((&MessageLevel::Error, "boom"))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn refresh_selected_repo_routes_to_summary_and_detail_in_repo_mode() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::RefreshSelectedRepo));

        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                },
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
            ]
        );
    }

    #[test]
    fn refresh_selected_repo_only_refreshes_summary_in_workspace_mode() {
        let repo_id = RepoId::new("repo-1");
        let mut state = AppState::default();
        state.workspace.selected_repo_id = Some(repo_id.clone());

        let result = reduce(state, Event::Action(Action::RefreshSelectedRepo));

        assert_eq!(result.effects, vec![Effect::RefreshRepoSummary { repo_id }]);
    }

    #[test]
    fn refresh_visible_repos_marks_workspace_scanning_and_emits_scan_effect() {
        let result = reduce(
            AppState::default(),
            Event::Action(Action::RefreshVisibleRepos),
        );

        assert_eq!(result.state.workspace.scan_status, ScanStatus::Scanning);
        assert_eq!(
            result.effects,
            vec![Effect::ScheduleRender, Effect::StartRepoScan]
        );
    }

    #[test]
    fn repo_scan_failed_marks_workspace_failed_and_keeps_existing_rows() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let mut state = AppState::default();
        state.workspace.current_root = Some(std::path::PathBuf::from("/tmp/workspace"));
        state.workspace.discovered_repo_ids = vec![repo_id.clone()];
        state
            .workspace
            .repo_summaries
            .insert(repo_id.clone(), workspace_summary(&repo_id.0));

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoScanFailed {
                root: Some(std::path::PathBuf::from("/tmp/workspace")),
                error: "permission denied".to_string(),
            }),
        );

        assert_eq!(
            result.state.workspace.scan_status,
            ScanStatus::Failed {
                message: "permission denied".to_string(),
            }
        );
        assert_eq!(result.state.workspace.discovered_repo_ids, vec![repo_id]);
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn repo_summary_refresh_failed_creates_placeholder_summary_for_missing_repo() {
        let repo_id = RepoId::new("/tmp/missing-summary");
        let job_id = JobId::new("summary-refresh:/tmp/missing-summary");

        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoSummaryRefreshFailed {
                job_id,
                repo_id: repo_id.clone(),
                error: "repo summary failed".to_string(),
            }),
        );

        let summary = result
            .state
            .workspace
            .repo_summaries
            .get(&repo_id)
            .expect("placeholder summary");
        assert_eq!(summary.display_path, repo_id.0);
        assert_eq!(summary.last_error.as_deref(), Some("repo summary failed"));
    }

    #[test]
    fn switch_repo_subview_moves_focus_to_detail_pane() {
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: RepoId::new("repo-1"),
            }),
        )
        .state;

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Branches)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Branches)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn commit_selection_updates_selected_index_and_compare_target() {
        let detail = RepoDetail {
            commits: vec![
                CommitItem {
                    oid: "abcdef1234567890".to_string(),
                    short_oid: "abcdef1".to_string(),
                    summary: "add lib".to_string(),
                    changed_files: vec![CommitFileItem {
                        path: std::path::PathBuf::from("src/lib.rs"),
                        kind: FileStatusKind::Added,
                    }],
                    diff: DiffModel::default(),
                },
                CommitItem {
                    oid: "1234567890abcdef".to_string(),
                    short_oid: "1234567".to_string(),
                    summary: "second".to_string(),
                    changed_files: vec![CommitFileItem {
                        path: std::path::PathBuf::from("notes.md"),
                        kind: FileStatusKind::Added,
                    }],
                    diff: DiffModel::default(),
                },
            ],
            ..RepoDetail::default()
        };
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Commits,
                detail: Some(detail),
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let down = reduce(state, Event::Action(Action::SelectNextCommit));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index),
            Some(1)
        );
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .and_then(|detail| detail.comparison_target.clone()),
            Some(ComparisonTarget::Commit("1234567890abcdef".to_string()))
        );

        let up = reduce(down.state, Event::Action(Action::SelectPreviousCommit));
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index),
            Some(0)
        );
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .and_then(|detail| detail.comparison_target.clone()),
            Some(ComparisonTarget::Commit("abcdef1234567890".to_string()))
        );
    }

    #[test]
    fn scroll_repo_detail_actions_clamp_to_bounds() {
        let detail = RepoDetail {
            diff: DiffModel {
                selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                presentation: DiffPresentation::Unstaged,
                lines: vec![
                    DiffLine {
                        kind: DiffLineKind::Meta,
                        content: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::HunkHeader,
                        content: "@@ -1,1 +1,1 @@".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "+hello".to_string(),
                    },
                ],
                hunks: vec![DiffHunk {
                    header: "@@ -1,1 +1,1 @@".to_string(),
                    selection: SelectedHunk {
                        old_start: 1,
                        old_lines: 1,
                        new_start: 1,
                        new_lines: 1,
                    },
                    start_line_index: 1,
                    end_line_index: 3,
                }],
                selected_hunk: Some(0),
                hunk_count: 1,
            },
            ..RepoDetail::default()
        };
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(detail),
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let down_once = reduce(state, Event::Action(Action::ScrollRepoDetailDown));
        assert_eq!(
            down_once
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(1)
        );

        let down_twice = reduce(down_once.state, Event::Action(Action::ScrollRepoDetailDown));
        let down_thrice = reduce(
            down_twice.state,
            Event::Action(Action::ScrollRepoDetailDown),
        );
        assert_eq!(
            down_thrice
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(2)
        );

        let up_once = reduce(down_thrice.state, Event::Action(Action::ScrollRepoDetailUp));
        let up_twice = reduce(up_once.state, Event::Action(Action::ScrollRepoDetailUp));
        let up_thrice = reduce(up_twice.state, Event::Action(Action::ScrollRepoDetailUp));
        assert_eq!(
            up_thrice
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(0)
        );
    }

    #[test]
    fn close_top_modal_in_repo_mode_restores_repo_focus() {
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: RepoId::new("repo-1"),
            }),
        )
        .state;
        let state = reduce(
            state,
            Event::Action(Action::OpenModal {
                kind: ModalKind::Help,
                title: "Help".to_string(),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::CloseTopModal));

        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
    }

    #[test]
    fn stage_selection_queues_git_job_and_sets_progress() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::StageSelection));
        let job_id = JobId::new("git:repo-1:stage-selection");

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Stage selection".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StageSelection,
            })]
        );
    }

    #[test]
    fn status_selection_moves_with_focused_repo_pane() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let down = reduce(state, Event::Action(Action::SelectNextStatusEntry));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.status_view.selected_index),
            Some(1)
        );

        let staged_state = AppState {
            focused_pane: PaneId::RepoStaged,
            ..down.state
        };
        let staged_down = reduce(staged_state, Event::Action(Action::SelectNextStatusEntry));
        assert_eq!(
            staged_down
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.staged_view.selected_index),
            Some(1)
        );
    }

    #[test]
    fn stage_selected_file_queues_file_scoped_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                status_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StageSelectedFile));
        let job_id = JobId::new("git:repo-1:stage-file");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: job_id.clone(),
                repo_id,
                command: GitCommand::StageFile {
                    path: std::path::PathBuf::from("README.md"),
                },
            })]
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id,
                summary: "Stage README.md".to_string(),
            })
        );
    }

    #[test]
    fn unstage_selected_file_queues_file_scoped_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                staged_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::UnstageSelectedFile));
        let job_id = JobId::new("git:repo-1:unstage-file");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: job_id.clone(),
                repo_id,
                command: GitCommand::UnstageFile {
                    path: std::path::PathBuf::from("Cargo.toml"),
                },
            })]
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id,
                summary: "Unstage Cargo.toml".to_string(),
            })
        );
    }

    #[test]
    fn open_commit_box_focuses_staged_pane_and_tracks_mode() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenCommitBox {
                mode: CommitBoxMode::Amend,
            }),
        );

        assert_eq!(result.state.focused_pane, PaneId::RepoStaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box),
            Some(crate::state::CommitBoxState {
                focused: true,
                mode: CommitBoxMode::Amend,
            })
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn submit_commit_box_queues_commit_when_message_is_valid() {
        let repo_id = RepoId::new("repo-1");
        let mut detail = repo_detail_with_file_tree();
        detail.commit_input = "ship it".to_string();
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(detail),
                commit_box: crate::state::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::Commit,
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitCommitBox));
        let job_id = JobId::new("git:repo-1:commit-staged");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: job_id.clone(),
                repo_id,
                command: GitCommand::CommitStaged {
                    message: "ship it".to_string(),
                },
            })]
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.focused),
            Some(false)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .map(|detail| detail.commit_input.as_str()),
            Some("")
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id,
                summary: "Commit staged changes".to_string(),
            })
        );
    }

    #[test]
    fn submit_commit_box_rejects_empty_commit_messages() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                commit_box: crate::state::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::Commit,
                },
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitCommitBox));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| (&notification.level, notification.text.as_str())),
            Some((
                &MessageLevel::Warning,
                "Enter a commit message before confirming."
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.focused),
            Some(true)
        );
    }

    #[test]
    fn submit_commit_box_supports_amend_without_editing_message() {
        let repo_id = RepoId::new("repo-1");
        let mut detail = repo_detail_with_file_tree();
        detail.commits = vec![CommitItem {
            oid: "abcdef1234567890".to_string(),
            short_oid: "abcdef1".to_string(),
            summary: "init".to_string(),
            changed_files: vec![],
            diff: DiffModel::default(),
        }];
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(detail),
                commit_box: crate::state::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::Amend,
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitCommitBox));

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:amend-head"),
                repo_id,
                command: GitCommand::AmendHead { message: None },
            })]
        );
    }

    #[test]
    fn git_operation_started_marks_job_running() {
        let repo_id = RepoId::new("repo-1");
        let job_id = JobId::new("git:repo-1:stage-selection");

        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::GitOperationStarted {
                job_id: job_id.clone(),
                repo_id: repo_id.clone(),
                summary: "Stage selection".to_string(),
            }),
        );

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Running)
        );
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .and_then(|job| job.target_repo.as_ref()),
            Some(&repo_id)
        );
    }

    #[test]
    fn repo_detail_loaded_only_updates_matching_repo_mode() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;
        let detail = RepoDetail {
            file_tree: vec![
                FileStatus {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: Some(FileStatusKind::Modified),
                    unstaged_kind: Some(FileStatusKind::Modified),
                },
                FileStatus {
                    path: std::path::PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Untracked),
                },
            ],
            commits: vec![CommitItem {
                oid: "abcdef1234567890".to_string(),
                short_oid: "abcdef1".to_string(),
                summary: "add lib".to_string(),
                changed_files: vec![CommitFileItem {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Added,
                }],
                diff: DiffModel::default(),
            }],
            ..RepoDetail::default()
        };

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoDetailLoaded {
                repo_id: repo_id.clone(),
                detail: detail.clone(),
            }),
        );

        let stored_detail = result
            .state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.detail.as_ref())
            .expect("detail stored");
        assert_eq!(stored_detail.commits, detail.commits);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index),
            Some(0)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.status_view.selected_index),
            Some(0)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.staged_view.selected_index),
            Some(0)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .and_then(|detail| detail.comparison_target.clone()),
            Some(ComparisonTarget::Commit("abcdef1234567890".to_string()))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| &repo_mode.operation_progress),
            Some(&crate::state::OperationProgress::Idle)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn git_operation_completed_refreshes_repo_and_reports_status() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;
        let state = reduce(state, Event::Action(Action::StageSelection)).state;
        let job_id = JobId::new("git:repo-1:stage-selection");

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::GitOperationCompleted {
                job_id: job_id.clone(),
                repo_id: repo_id.clone(),
                summary: "Done".to_string(),
            }),
        );

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Succeeded)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| &repo_mode.operation_progress),
            Some(&crate::state::OperationProgress::Idle)
        );
        assert_eq!(
            result
                .state
                .status_messages
                .back()
                .map(|message| message.text.as_str()),
            Some("Done")
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                },
                Effect::LoadRepoDetail {
                    repo_id: repo_id.clone(),
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn git_operation_failed_sets_failed_progress_and_notification() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;
        let state = reduce(state, Event::Action(Action::StageSelection)).state;
        let job_id = JobId::new("git:repo-1:stage-selection");

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::GitOperationFailed {
                job_id: job_id.clone(),
                repo_id,
                error: "boom".to_string(),
            }),
        );

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Failed {
                error: "boom".to_string(),
            })
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Failed {
                summary: "boom".to_string(),
            })
        );
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| (&notification.level, notification.text.as_str())),
            Some((&MessageLevel::Error, "boom"))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn repo_invalidated_loads_detail_for_active_repo() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(
            state,
            Event::Watcher(WatcherEvent::RepoInvalidated {
                repo_id: repo_id.clone(),
            }),
        );

        assert_eq!(result.effects, vec![Effect::ScheduleWatcherDebounce]);
        assert_eq!(
            result
                .state
                .workspace
                .pending_watcher_invalidations
                .get(&repo_id),
            Some(&1)
        );
        assert!(result.state.workspace.watcher_debounce_pending);
    }

    #[test]
    fn repo_invalidated_outside_repo_mode_only_refreshes_summary() {
        let repo_id = RepoId::new("repo-1");

        let result = reduce(
            AppState::default(),
            Event::Watcher(WatcherEvent::RepoInvalidated {
                repo_id: repo_id.clone(),
            }),
        );

        assert_eq!(result.effects, vec![Effect::ScheduleWatcherDebounce]);
        assert_eq!(
            result
                .state
                .workspace
                .pending_watcher_invalidations
                .get(&repo_id),
            Some(&1)
        );
    }

    #[test]
    fn watcher_debounce_flush_coalesces_repeated_repo_invalidations() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;
        let state = reduce(
            state,
            Event::Watcher(WatcherEvent::RepoInvalidated {
                repo_id: repo_id.clone(),
            }),
        )
        .state;
        let state = reduce(
            state,
            Event::Watcher(WatcherEvent::RepoInvalidated {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Timer(TimerEvent::WatcherDebounceFlush));

        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                },
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
            ]
        );
        assert!(result
            .state
            .workspace
            .pending_watcher_invalidations
            .is_empty());
        assert!(!result.state.workspace.watcher_debounce_pending);
    }

    #[test]
    fn watcher_debounce_flush_outside_repo_mode_only_refreshes_summary() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Watcher(WatcherEvent::RepoInvalidated {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Timer(TimerEvent::WatcherDebounceFlush));

        assert_eq!(result.effects, vec![Effect::RefreshRepoSummary { repo_id }]);
    }

    #[test]
    fn watcher_debounce_flush_prioritizes_visible_repos_over_hidden_ones() {
        let repo_visible = RepoId::new("/tmp/visible");
        let repo_hidden = RepoId::new("/tmp/hidden");
        let mut visible = workspace_summary(&repo_visible.0);
        visible.dirty = true;
        let mut state = AppState::default();
        state.workspace.discovered_repo_ids = vec![repo_hidden.clone(), repo_visible.clone()];
        state.workspace.filter_mode = WorkspaceFilterMode::DirtyOnly;
        state
            .workspace
            .repo_summaries
            .insert(repo_visible.clone(), visible);
        state
            .workspace
            .repo_summaries
            .insert(repo_hidden.clone(), workspace_summary(&repo_hidden.0));
        state
            .workspace
            .pending_watcher_invalidations
            .insert(repo_hidden.clone(), 1);
        state
            .workspace
            .pending_watcher_invalidations
            .insert(repo_visible.clone(), 1);
        state.workspace.watcher_debounce_pending = true;

        let result = reduce(state, Event::Timer(TimerEvent::WatcherDebounceFlush));

        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: repo_visible,
                },
                Effect::RefreshRepoSummary {
                    repo_id: repo_hidden,
                },
            ]
        );
    }

    #[test]
    fn watcher_debounce_flush_prioritizes_active_repo_before_visible_and_hidden() {
        let repo_active = RepoId::new("/tmp/active");
        let repo_visible = RepoId::new("/tmp/visible");
        let repo_hidden = RepoId::new("/tmp/hidden");
        let mut visible = workspace_summary(&repo_visible.0);
        visible.dirty = true;
        let mut state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState::new(repo_active.clone())),
            ..AppState::default()
        };
        state.workspace.discovered_repo_ids = vec![
            repo_hidden.clone(),
            repo_visible.clone(),
            repo_active.clone(),
        ];
        state.workspace.filter_mode = WorkspaceFilterMode::DirtyOnly;
        state
            .workspace
            .repo_summaries
            .insert(repo_active.clone(), workspace_summary(&repo_active.0));
        state
            .workspace
            .repo_summaries
            .insert(repo_visible.clone(), visible);
        state
            .workspace
            .repo_summaries
            .insert(repo_hidden.clone(), workspace_summary(&repo_hidden.0));
        state
            .workspace
            .pending_watcher_invalidations
            .insert(repo_hidden.clone(), 1);
        state
            .workspace
            .pending_watcher_invalidations
            .insert(repo_visible.clone(), 1);
        state
            .workspace
            .pending_watcher_invalidations
            .insert(repo_active.clone(), 1);
        state.workspace.watcher_debounce_pending = true;

        let result = reduce(state, Event::Timer(TimerEvent::WatcherDebounceFlush));

        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: repo_active.clone(),
                },
                Effect::LoadRepoDetail {
                    repo_id: repo_active,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
                Effect::RefreshRepoSummary {
                    repo_id: repo_visible,
                },
                Effect::RefreshRepoSummary {
                    repo_id: repo_hidden,
                },
            ]
        );
    }

    #[test]
    fn repo_scan_completed_requests_watcher_configuration() {
        let repo_id = RepoId::new("repo-1");

        let result = reduce(
            AppState::default(),
            Event::Worker(WorkerEvent::RepoScanCompleted {
                root: Some(std::path::PathBuf::from("/tmp/workspace")),
                repo_ids: vec![repo_id.clone()],
                scanned_at: Timestamp(7),
            }),
        );

        assert_eq!(
            result.effects,
            vec![
                Effect::ConfigureWatcher {
                    repo_ids: vec![repo_id.clone()],
                },
                Effect::RefreshRepoSummaries {
                    repo_ids: vec![repo_id.clone()],
                },
                Effect::PersistCache,
                Effect::ScheduleRender,
            ]
        );
        assert_eq!(result.state.workspace.selected_repo_id, Some(repo_id));
    }

    #[test]
    fn watcher_recovered_sets_health_to_healthy() {
        let result = reduce(
            AppState::default(),
            Event::Watcher(WatcherEvent::WatcherRecovered),
        );

        assert_eq!(
            result.state.workspace.watcher_health,
            WatcherHealth::Healthy
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn periodic_refresh_tick_moves_idle_scan_to_scanning() {
        let result = reduce(
            AppState::default(),
            Event::Timer(TimerEvent::PeriodicRefreshTick),
        );

        assert_eq!(
            result.state.workspace.scan_status,
            crate::state::ScanStatus::Scanning
        );
        assert!(result.effects.is_empty());
    }

    #[test]
    fn periodic_refresh_tick_in_degraded_mode_polls_prioritized_repos() {
        let repo_active = RepoId::new("/tmp/active");
        let repo_visible = RepoId::new("/tmp/visible");
        let repo_hidden = RepoId::new("/tmp/hidden");
        let mut visible = workspace_summary(&repo_visible.0);
        visible.dirty = true;
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState::new(repo_active.clone())),
            workspace: crate::state::WorkspaceState {
                discovered_repo_ids: vec![
                    repo_hidden.clone(),
                    repo_visible.clone(),
                    repo_active.clone(),
                ],
                repo_summaries: std::collections::BTreeMap::from([
                    (repo_active.clone(), workspace_summary(&repo_active.0)),
                    (repo_visible.clone(), visible),
                    (repo_hidden.clone(), workspace_summary(&repo_hidden.0)),
                ]),
                filter_mode: WorkspaceFilterMode::DirtyOnly,
                watcher_health: WatcherHealth::Degraded {
                    message: "polling".to_string(),
                },
                ..Default::default()
            },
            ..AppState::default()
        };

        let result = reduce(state, Event::Timer(TimerEvent::PeriodicRefreshTick));

        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummaries {
                    repo_ids: vec![
                        repo_active.clone(),
                        repo_visible.clone(),
                        repo_hidden.clone(),
                    ],
                },
                Effect::LoadRepoDetail {
                    repo_id: repo_active,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn toast_expiry_tick_removes_expired_notifications() {
        let mut state = AppState::default();
        state.notifications.push_back(crate::state::Notification {
            id: 1,
            level: MessageLevel::Info,
            text: "stale".to_string(),
            expires_at: Some(Timestamp(5)),
        });
        state.notifications.push_back(crate::state::Notification {
            id: 2,
            level: MessageLevel::Info,
            text: "fresh".to_string(),
            expires_at: Some(Timestamp(15)),
        });

        let result = reduce(
            state,
            Event::Timer(TimerEvent::ToastExpiryTick { now: Timestamp(10) }),
        );

        assert_eq!(result.state.notifications.len(), 1);
        assert_eq!(
            result
                .state
                .notifications
                .front()
                .map(|notification| notification.id),
            Some(2)
        );
        assert!(result.effects.is_empty());
    }
}
