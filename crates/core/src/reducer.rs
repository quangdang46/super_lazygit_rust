use crate::action::Action;
use crate::effect::{
    Effect, GitCommand, GitCommandRequest, PatchApplicationMode, PatchSelectionJob, RebaseStartMode,
};
use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
use crate::state::{
    AppMode, AppState, BackgroundJob, BackgroundJobKind, BackgroundJobState, CommitBoxMode,
    ComparisonTarget, ConfirmableOperation, DiffLineKind, DiffPresentation, InputPromptOperation,
    JobId, MergeState, MessageLevel, Notification, OperationProgress, PaneId, PendingInputPrompt,
    RepoModeState, ResetMode, ScanStatus, SelectedHunk, StatusMessage, WatcherHealth,
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
        Action::SelectNextBranch => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_branch_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousBranch => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_branch_selection(repo_mode, -1) {
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
        Action::StartInteractiveRebase => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err(
                        "Select an older commit before starting an interactive rebase.".to_string(),
                    );
                }
                Ok(ConfirmableOperation::StartInteractiveRebase {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(
                    state,
                    "Select a commit before starting an interactive rebase.",
                ),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::AmendSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before starting amend.".to_string());
                }
                Ok(ConfirmableOperation::AmendCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before starting amend."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::FixupSelectedCommit => {
            match pending_history_commit_operation(state, |detail, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before starting fixup.".to_string());
                }
                if staged_file_count(detail) == 0 {
                    return Err("Stage changes before starting fixup.".to_string());
                }
                Ok(ConfirmableOperation::FixupCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before starting fixup."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::RewordSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before starting reword.".to_string());
                }
                Ok(InputPromptOperation::RewordCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                    initial_message: commit.summary.clone(),
                })
            }) {
                Ok(Some((repo_id, operation))) => open_input_prompt(state, repo_id, operation),
                Ok(None) => push_warning(state, "Select a commit before starting reword."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::RewordSelectedCommitWithEditor => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before starting reword.".to_string());
                }
                Ok((
                    GitCommand::RewordCommitWithEditor {
                        commit: commit.oid.clone(),
                    },
                    format!("Reword {} {}", commit.short_oid, commit.summary),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => {
                    push_warning(state, "Select a commit before starting reword.");
                    effects.push(Effect::ScheduleRender);
                }
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CherryPickSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, _| {
                Ok(ConfirmableOperation::CherryPickCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before cherry-picking."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::RevertSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, _| {
                Ok(ConfirmableOperation::RevertCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before reverting."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SoftResetToSelectedCommit => {
            if open_reset_confirmation(state, ResetMode::Soft) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a commit before resetting HEAD.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::MixedResetToSelectedCommit => {
            if open_reset_confirmation(state, ResetMode::Mixed) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a commit before resetting HEAD.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::HardResetToSelectedCommit => {
            if open_reset_confirmation(state, ResetMode::Hard) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a commit before resetting HEAD.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ContinueRebase => {
            if let Some(job) =
                queue_rebase_job(state, GitCommand::ContinueRebase, "Continue rebase")
            {
                effects.push(Effect::RunGitCommand(job));
            } else {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::AbortRebase => {
            if ensure_rebase_active(state) {
                let repo_id = state
                    .repo_mode
                    .as_ref()
                    .expect("repo mode exists")
                    .current_repo_id
                    .clone();
                open_confirmation_modal(state, repo_id, ConfirmableOperation::AbortRebase);
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SkipRebase => {
            if ensure_rebase_active(state) {
                let repo_id = state
                    .repo_mode
                    .as_ref()
                    .expect("repo mode exists")
                    .current_repo_id
                    .clone();
                open_confirmation_modal(state, repo_id, ConfirmableOperation::SkipRebase);
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SelectNextStash => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_stash_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousStash => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_stash_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectNextReflog => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_reflog_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousReflog => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_reflog_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::RestoreSelectedReflogEntry => {
            match selected_reflog_restore_target(state) {
                Ok(Some((repo_id, target, summary))) => open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::RestoreReflogEntry { target, summary },
                ),
                Ok(None) => push_warning(state, "Select a reflog entry before restoring HEAD."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SelectNextWorktree => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_worktree_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousWorktree => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_worktree_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::ToggleComparisonSelection => {
            toggle_comparison_selection(state, effects);
        }
        Action::ClearComparison => {
            clear_comparison(state, effects);
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
        Action::SelectNextDiffLine => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_diff_line_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousDiffLine => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_diff_line_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::ToggleDiffLineAnchor => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if toggle_diff_line_anchor(repo_mode) {
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
            if state
                .modal_stack
                .last()
                .is_some_and(|modal| matches!(modal.kind, crate::state::ModalKind::InputPrompt))
            {
                state.pending_input_prompt = None;
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
        Action::OpenInputPrompt { operation } => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_input_prompt(state, repo_id, operation);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::AppendPromptInput { text } => {
            if let Some(prompt) = state.pending_input_prompt.as_mut() {
                prompt.value.push_str(&text);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::BackspacePromptInput => {
            if let Some(prompt) = state.pending_input_prompt.as_mut() {
                if prompt.value.pop().is_some() {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SubmitPromptInput => {
            if let Some(job) = submit_input_prompt(state) {
                effects.push(Effect::RunGitCommand(job));
            } else {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenInEditor => match selected_editor_target(state) {
            Ok((cwd, target)) => effects.push(Effect::OpenEditor { cwd, target }),
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
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
        Action::DiscardSelectedFile => {
            if let Some((repo_id, path)) = selected_discard_path(state) {
                open_confirmation_modal(state, repo_id, ConfirmableOperation::DiscardFile { path });
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a status entry before discarding changes.");
                effects.push(Effect::ScheduleRender);
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
        Action::StageSelectedLines => {
            match selected_line_patch_job(state, PatchApplicationMode::Stage) {
                Ok(Some(job)) => {
                    let summary = format!("Stage selected lines in {}", job.path.display());
                    enqueue_patch_job(state, &job, &summary);
                    effects.push(Effect::RunPatchSelection(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::UnstageSelectedLines => {
            match selected_line_patch_job(state, PatchApplicationMode::Unstage) {
                Ok(Some(job)) => {
                    let summary = format!("Unstage selected lines in {}", job.path.display());
                    enqueue_patch_job(state, &job, &summary);
                    effects.push(Effect::RunPatchSelection(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
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
        Action::CommitStagedWithEditor => {
            if let Some(job) = commit_with_editor_job(state) {
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
        Action::CommitStagedNoVerify { message } => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::CommitStagedNoVerify {
                        message: message.clone(),
                    },
                );
                enqueue_git_job(state, &job, "Commit staged changes without hooks");
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
        Action::CheckoutSelectedBranch => {
            if let Some((repo_id, branch_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) {
                let summary = format!("Checkout branch {branch_ref}");
                let job = git_job(repo_id, GitCommand::CheckoutBranch { branch_ref });
                enqueue_git_job(state, &job, &summary);
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
        Action::DeleteSelectedBranch => {
            if let Some((repo_id, branch_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::DeleteBranch { branch_name },
                );
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ApplySelectedStash => {
            if let Some((repo_id, stash_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_stash_item(repo_mode)
                    .map(|stash| (repo_mode.current_repo_id.clone(), stash.stash_ref.clone()))
            }) {
                let summary = format!("Apply stash {stash_ref}");
                let job = git_job(repo_id, GitCommand::ApplyStash { stash_ref });
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::DropSelectedStash => {
            if let Some((repo_id, stash_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_stash_item(repo_mode)
                    .map(|stash| (repo_mode.current_repo_id.clone(), stash.stash_ref.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::DropStash { stash_ref },
                );
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CreateWorktree => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_input_prompt(state, repo_id, InputPromptOperation::CreateWorktree);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::RemoveSelectedWorktree => {
            if let Some((repo_id, path)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_worktree_item(repo_mode)
                    .map(|item| (repo_mode.current_repo_id.clone(), item.path.clone()))
            }) {
                if path == std::path::Path::new(&repo_id.0) {
                    push_warning(state, "Use git directly to remove the primary worktree.");
                    effects.push(Effect::ScheduleRender);
                } else {
                    open_confirmation_modal(
                        state,
                        repo_id,
                        ConfirmableOperation::RemoveWorktree { path },
                    );
                    effects.push(Effect::ScheduleRender);
                }
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
        Action::NukeWorkingTree => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_confirmation_modal(state, repo_id, ConfirmableOperation::NukeWorkingTree);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SwitchRepoSubview(subview) => {
            let mut repo_detail_reload = None;
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.active_subview = subview;
                repo_mode.diff_scroll = 0;
                if !matches!(
                    subview,
                    crate::state::RepoSubview::Status | crate::state::RepoSubview::Compare
                ) {
                    close_commit_box(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Branches) {
                    sync_branch_selection(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Commits) {
                    sync_commit_selection(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Rebase) {
                    repo_mode.diff_scroll = 0;
                }
                if matches!(subview, crate::state::RepoSubview::Stash) {
                    sync_stash_selection(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Reflog) {
                    sync_reflog_selection(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Worktrees) {
                    sync_worktree_selection(repo_mode);
                }
                if matches!(subview, crate::state::RepoSubview::Compare)
                    && repo_mode.comparison_base.is_some()
                    && repo_mode.comparison_target.is_some()
                {
                    effects.push(load_comparison_diff_effect(repo_mode));
                } else if matches!(subview, crate::state::RepoSubview::Status)
                    && repo_mode.detail.as_ref().is_some_and(|detail| {
                        detail.diff.presentation == DiffPresentation::Comparison
                    })
                {
                    repo_detail_reload = Some(repo_mode.current_repo_id.clone());
                }
                state.focused_pane = PaneId::RepoDetail;
                effects.push(Effect::ScheduleRender);
            }
            if let Some(repo_id) = repo_detail_reload {
                effects.push(load_repo_detail_effect(state, repo_id));
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
                    sync_branch_selection(repo_mode);
                    sync_commit_selection(repo_mode);
                    sync_stash_selection(repo_mode);
                    sync_reflog_selection(repo_mode);
                    sync_worktree_selection(repo_mode);
                    sync_diff_selection(repo_mode);
                    repo_mode.operation_progress = OperationProgress::Idle;
                    let rebase_in_progress = repo_mode
                        .detail
                        .as_ref()
                        .is_some_and(repo_detail_has_rebase);
                    if rebase_in_progress {
                        repo_mode.active_subview = crate::state::RepoSubview::Rebase;
                        repo_mode.diff_scroll = 0;
                        close_commit_box(repo_mode);
                        state.focused_pane = PaneId::RepoDetail;
                    } else if repo_mode.active_subview == crate::state::RepoSubview::Rebase {
                        repo_mode.active_subview = crate::state::RepoSubview::Commits;
                        repo_mode.diff_scroll = 0;
                        state.focused_pane = PaneId::RepoDetail;
                    }
                    if repo_mode.active_subview == crate::state::RepoSubview::Compare
                        && repo_mode.comparison_base.is_some()
                        && repo_mode.comparison_target.is_some()
                    {
                        effects.push(load_comparison_diff_effect(repo_mode));
                    }
                }
            }
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoDiffLoaded { repo_id, diff } => {
            if let Some(repo_mode) = state
                .repo_mode
                .as_mut()
                .filter(|repo_mode| repo_mode.current_repo_id == repo_id)
            {
                if let Some(detail) = repo_mode.detail.as_mut() {
                    detail.diff = diff;
                    sync_diff_selection(repo_mode);
                    repo_mode.diff_scroll = 0;
                    repo_mode.operation_progress = OperationProgress::Idle;
                }
            }
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::RepoDiffLoadFailed { repo_id, error } => {
            if let Some(repo_mode) = state
                .repo_mode
                .as_mut()
                .filter(|repo_mode| repo_mode.current_repo_id == repo_id)
            {
                repo_mode.operation_progress = OperationProgress::Failed {
                    summary: error.clone(),
                };
            }
            state.notifications.push_back(Notification {
                id: 0,
                level: MessageLevel::Error,
                text: error,
                expires_at: None,
            });
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
            effects.push(Effect::RefreshRepoSummary {
                repo_id: repo_id.clone(),
            });
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        WorkerEvent::EditorLaunchFailed { error } => {
            push_warning(state, error);
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
    let Some(detail) = repo_mode.detail.as_ref() else {
        return false;
    };

    let before = repo_mode.commits_view.selected_index;
    let after = repo_mode
        .commits_view
        .select_with_step(detail.commits.len(), step);
    if after.is_some() && after != before {
        repo_mode.diff_scroll = 0;
        true
    } else {
        false
    }
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
    sync_diff_selection(repo_mode);
    true
}

fn step_diff_line_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let selectable = selected_hunk_change_lines(repo_mode)
        .map(|lines| lines.collect::<Vec<_>>())
        .unwrap_or_default();
    if selectable.is_empty() {
        repo_mode.diff_line_cursor = None;
        repo_mode.diff_line_anchor = None;
        return false;
    }

    let current_index = repo_mode.diff_line_cursor.and_then(|cursor| {
        selectable
            .iter()
            .position(|line_index| *line_index == cursor)
    });
    let next_index = match current_index {
        Some(index) => (index as isize + step).rem_euclid(selectable.len() as isize) as usize,
        None => 0,
    };
    let next_line = selectable[next_index];
    let changed = repo_mode.diff_line_cursor != Some(next_line);
    repo_mode.diff_line_cursor = Some(next_line);
    repo_mode.diff_scroll = next_line;
    changed || current_index.is_none()
}

fn toggle_diff_line_anchor(repo_mode: &mut RepoModeState) -> bool {
    let Some(cursor) = repo_mode.diff_line_cursor else {
        return false;
    };

    repo_mode.diff_line_anchor = match repo_mode.diff_line_anchor {
        Some(anchor) if anchor == cursor => None,
        _ => Some(cursor),
    };
    true
}

fn selected_hunk_change_lines(
    repo_mode: &RepoModeState,
) -> Option<impl Iterator<Item = usize> + '_> {
    let detail = repo_mode.detail.as_ref()?;
    let hunk = detail
        .diff
        .selected_hunk
        .and_then(|index| detail.diff.hunks.get(index))?;
    Some(
        (hunk.start_line_index + 1..hunk.end_line_index)
            .filter(|line_index| is_change_line(detail.diff.lines[*line_index].kind)),
    )
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
        CommitBoxMode::CommitNoVerify => {
            if staged_count == 0 {
                push_warning(state, "Stage at least one file before committing.");
                return None;
            }
            if message.is_empty() {
                push_warning(state, "Enter a commit message before confirming.");
                return None;
            }
            GitCommand::CommitStagedNoVerify { message }
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
        CommitBoxMode::CommitNoVerify => "Commit staged changes without hooks",
        CommitBoxMode::Amend => "Amend HEAD commit",
    };
    let job = git_job(repo_id, command);
    enqueue_git_job(state, &job, summary);
    Some(job)
}

fn commit_with_editor_job(state: &mut AppState) -> Option<GitCommandRequest> {
    let (repo_id, staged_count) = state.repo_mode.as_ref().and_then(|repo_mode| {
        repo_mode
            .detail
            .as_ref()
            .map(|detail| (repo_mode.current_repo_id.clone(), staged_file_count(detail)))
    })?;

    if staged_count == 0 {
        push_warning(state, "Stage at least one file before committing.");
        return None;
    }

    let job = git_job(repo_id, GitCommand::CommitStagedWithEditor);
    enqueue_git_job(state, &job, "Commit staged changes with editor");
    Some(job)
}

fn open_confirmation_modal(
    state: &mut AppState,
    repo_id: crate::state::RepoId,
    operation: ConfirmableOperation,
) {
    let title = confirmation_title(&operation);
    state.pending_confirmation = Some(crate::state::PendingConfirmation { repo_id, operation });
    state.modal_stack.push(crate::state::Modal::new(
        crate::state::ModalKind::Confirm,
        title,
    ));
    state.focused_pane = PaneId::Modal;
}

fn confirmation_title(operation: &ConfirmableOperation) -> String {
    match operation {
        ConfirmableOperation::Fetch => "Confirm fetch".to_string(),
        ConfirmableOperation::Pull => "Confirm pull".to_string(),
        ConfirmableOperation::Push => "Confirm push".to_string(),
        ConfirmableOperation::DiscardFile { path } => {
            format!("Discard changes for {}", path.display())
        }
        ConfirmableOperation::StartInteractiveRebase { summary, .. } => {
            format!("Start interactive rebase at {summary}")
        }
        ConfirmableOperation::AmendCommit { summary, .. } => {
            format!("Amend {summary}")
        }
        ConfirmableOperation::FixupCommit { summary, .. } => {
            format!("Fixup {summary}")
        }
        ConfirmableOperation::CherryPickCommit { summary, .. } => {
            format!("Cherry-pick {summary}")
        }
        ConfirmableOperation::RevertCommit { summary, .. } => {
            format!("Revert {summary}")
        }
        ConfirmableOperation::ResetToCommit { mode, summary, .. } => {
            format!("{} reset to {summary}", mode.title())
        }
        ConfirmableOperation::RestoreReflogEntry { summary, .. } => {
            format!("Restore HEAD to {summary}")
        }
        ConfirmableOperation::AbortRebase => "Abort rebase".to_string(),
        ConfirmableOperation::SkipRebase => "Skip rebase step".to_string(),
        ConfirmableOperation::NukeWorkingTree => "Discard all local changes".to_string(),
        ConfirmableOperation::DeleteBranch { branch_name } => {
            format!("Delete branch {branch_name}")
        }
        ConfirmableOperation::DropStash { stash_ref } => format!("Drop stash {stash_ref}"),
        ConfirmableOperation::RemoveWorktree { path } => {
            format!("Remove worktree {}", path.display())
        }
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
        ConfirmableOperation::DiscardFile { path } => (
            GitCommand::DiscardFile { path: path.clone() },
            "Discard file changes",
        ),
        ConfirmableOperation::StartInteractiveRebase { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Interactive,
            },
            "Start interactive rebase",
        ),
        ConfirmableOperation::AmendCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Amend,
            },
            "Start older-commit amend",
        ),
        ConfirmableOperation::FixupCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Fixup,
            },
            "Start fixup autosquash",
        ),
        ConfirmableOperation::CherryPickCommit { commit, .. } => (
            GitCommand::CherryPickCommit { commit },
            "Cherry-pick selected commit",
        ),
        ConfirmableOperation::RevertCommit { commit, .. } => (
            GitCommand::RevertCommit { commit },
            "Revert selected commit",
        ),
        ConfirmableOperation::ResetToCommit { mode, commit, .. } => (
            GitCommand::ResetToCommit {
                mode,
                target: commit,
            },
            "Reset to selected commit",
        ),
        ConfirmableOperation::RestoreReflogEntry { target, .. } => (
            GitCommand::RestoreSnapshot { target },
            "Restore HEAD from reflog",
        ),
        ConfirmableOperation::AbortRebase => (GitCommand::AbortRebase, "Abort rebase"),
        ConfirmableOperation::SkipRebase => (GitCommand::SkipRebase, "Skip rebase step"),
        ConfirmableOperation::NukeWorkingTree => {
            (GitCommand::NukeWorkingTree, "Discard all local changes")
        }
        ConfirmableOperation::DeleteBranch { branch_name } => (
            GitCommand::DeleteBranch {
                branch_name: branch_name.clone(),
            },
            "Delete branch",
        ),
        ConfirmableOperation::DropStash { stash_ref } => (
            GitCommand::DropStash {
                stash_ref: stash_ref.clone(),
            },
            "Drop stash",
        ),
        ConfirmableOperation::RemoveWorktree { path } => (
            GitCommand::RemoveWorktree { path: path.clone() },
            "Remove worktree",
        ),
    };
    let job = git_job(pending.repo_id, command);
    enqueue_git_job(state, &job, summary);
    Some(job)
}

fn open_input_prompt(
    state: &mut AppState,
    repo_id: crate::state::RepoId,
    operation: InputPromptOperation,
) {
    let title = input_prompt_title(&operation);
    let value = input_prompt_initial_value(&operation);
    state.pending_input_prompt = Some(PendingInputPrompt {
        repo_id,
        operation,
        value,
    });
    state.modal_stack.push(crate::state::Modal::new(
        crate::state::ModalKind::InputPrompt,
        title,
    ));
    state.focused_pane = PaneId::Modal;
}

fn input_prompt_title(operation: &InputPromptOperation) -> String {
    match operation {
        InputPromptOperation::CreateBranch => "Create branch".to_string(),
        InputPromptOperation::RenameBranch { current_name } => {
            format!("Rename branch {current_name}")
        }
        InputPromptOperation::SetBranchUpstream { branch_name } => {
            format!("Set upstream for {branch_name}")
        }
        InputPromptOperation::CreateWorktree => "Create worktree".to_string(),
        InputPromptOperation::RewordCommit { summary, .. } => format!("Reword {summary}"),
    }
}

fn input_prompt_initial_value(operation: &InputPromptOperation) -> String {
    match operation {
        InputPromptOperation::CreateBranch => String::new(),
        InputPromptOperation::RenameBranch { current_name } => current_name.clone(),
        InputPromptOperation::SetBranchUpstream { branch_name: _ } => String::new(),
        InputPromptOperation::CreateWorktree => String::new(),
        InputPromptOperation::RewordCommit {
            initial_message, ..
        } => initial_message.clone(),
    }
}

fn submit_input_prompt(state: &mut AppState) -> Option<GitCommandRequest> {
    let pending = state.pending_input_prompt.take()?;
    let value = pending.value.trim().to_string();
    if value.is_empty() {
        state.pending_input_prompt = Some(pending);
        return None;
    }

    state.modal_stack.pop();
    if state.modal_stack.is_empty() && matches!(state.mode, AppMode::Repository) {
        state.focused_pane = PaneId::RepoDetail;
    }

    let (command, summary) = match pending.operation {
        InputPromptOperation::CreateBranch => (
            GitCommand::CreateBranch {
                branch_name: value.clone(),
            },
            format!("Create branch {value}"),
        ),
        InputPromptOperation::RenameBranch { current_name } => (
            GitCommand::RenameBranch {
                branch_name: current_name.clone(),
                new_name: value.clone(),
            },
            format!("Rename branch {current_name} to {value}"),
        ),
        InputPromptOperation::SetBranchUpstream { branch_name } => (
            GitCommand::SetBranchUpstream {
                branch_name: branch_name.clone(),
                upstream_ref: value.clone(),
            },
            format!("Set upstream for {branch_name}"),
        ),
        InputPromptOperation::CreateWorktree => {
            let Some((path, branch_ref)) = parse_create_worktree_input(&value) else {
                push_warning(state, "Enter worktree details as: <path> <branch>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                GitCommand::CreateWorktree {
                    path: path.clone(),
                    branch_ref: branch_ref.clone(),
                },
                format!("Create worktree {} from {branch_ref}", path.display()),
            )
        }
        InputPromptOperation::RewordCommit {
            commit, summary, ..
        } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Reword {
                    message: value.clone(),
                },
            },
            format!("Reword {summary}"),
        ),
    };
    let job = git_job(pending.repo_id, command);
    enqueue_git_job(state, &job, &summary);
    Some(job)
}

fn parse_create_worktree_input(value: &str) -> Option<(std::path::PathBuf, String)> {
    let (path, branch_ref) = value.rsplit_once(char::is_whitespace)?;
    let path = path.trim();
    let branch_ref = branch_ref.trim();
    if path.is_empty() || branch_ref.is_empty() {
        return None;
    }
    Some((std::path::PathBuf::from(path), branch_ref.to_string()))
}

fn ensure_rebase_active(state: &mut AppState) -> bool {
    if state
        .repo_mode
        .as_ref()
        .and_then(|repo_mode| repo_mode.detail.as_ref())
        .is_some_and(repo_detail_has_rebase)
    {
        true
    } else {
        push_warning(state, "No rebase is currently in progress.");
        false
    }
}

fn queue_rebase_job(
    state: &mut AppState,
    command: GitCommand,
    summary: &str,
) -> Option<GitCommandRequest> {
    if !ensure_rebase_active(state) {
        return None;
    }
    let repo_id = state.repo_mode.as_ref()?.current_repo_id.clone();
    let job = git_job(repo_id, command);
    enqueue_git_job(state, &job, summary);
    Some(job)
}

fn repo_detail_has_rebase(detail: &crate::state::RepoDetail) -> bool {
    detail.merge_state == MergeState::RebaseInProgress && detail.rebase_state.is_some()
}

fn sync_branch_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.branches_view.selected_index = None;
        return;
    };

    if detail.branches.is_empty() {
        repo_mode.branches_view.selected_index = None;
        return;
    }

    if let Some(index) = repo_mode
        .branches_view
        .selected_index
        .filter(|index| *index < detail.branches.len())
    {
        repo_mode.branches_view.selected_index = Some(index);
        return;
    }

    let head_index = detail
        .branches
        .iter()
        .position(|branch| branch.is_head)
        .unwrap_or(0);
    repo_mode.branches_view.selected_index = Some(head_index);
}

fn sync_commit_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.commits_view.selected_index = None;
        return;
    };

    repo_mode
        .commits_view
        .ensure_selection(detail.commits.len());
}

fn sync_stash_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.stash_view.selected_index = None;
        return;
    };

    repo_mode.stash_view.ensure_selection(detail.stashes.len());
}

fn sync_reflog_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.reflog_view.selected_index = None;
        return;
    };

    repo_mode
        .reflog_view
        .ensure_selection(detail.reflog_items.len());
}

fn sync_worktree_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.worktree_view.selected_index = None;
        return;
    };

    repo_mode
        .worktree_view
        .ensure_selection(detail.worktrees.len());
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
        repo_mode.diff_line_cursor = None;
        repo_mode.diff_line_anchor = None;
        repo_mode.diff_scroll = 0;
        return;
    };

    let len = detail.diff.hunks.len();
    detail.diff.selected_hunk = detail.diff.selected_hunk.filter(|index| *index < len);
    if detail.diff.selected_hunk.is_none() && len > 0 {
        detail.diff.selected_hunk = Some(0);
    }
    let Some(selected_hunk_index) = detail.diff.selected_hunk else {
        repo_mode.diff_line_cursor = None;
        repo_mode.diff_line_anchor = None;
        repo_mode.diff_scroll = 0;
        return;
    };
    let Some(selected_hunk) = detail.diff.hunks.get(selected_hunk_index) else {
        repo_mode.diff_line_cursor = None;
        repo_mode.diff_line_anchor = None;
        repo_mode.diff_scroll = 0;
        return;
    };

    let selected_range = selected_hunk.start_line_index + 1..selected_hunk.end_line_index;
    let selectable_lines = selected_range
        .clone()
        .filter(|line_index| is_change_line(detail.diff.lines[*line_index].kind))
        .collect::<Vec<_>>();

    repo_mode.diff_line_cursor = repo_mode.diff_line_cursor.filter(|line_index| {
        selectable_lines
            .iter()
            .any(|candidate| candidate == line_index)
    });
    if repo_mode.diff_line_cursor.is_none() {
        repo_mode.diff_line_cursor = selectable_lines.first().copied();
    }

    repo_mode.diff_line_anchor = repo_mode.diff_line_anchor.filter(|line_index| {
        selectable_lines
            .iter()
            .any(|candidate| candidate == line_index)
    });

    repo_mode.diff_scroll = repo_mode
        .diff_line_cursor
        .unwrap_or(selected_hunk.start_line_index);
}

fn is_change_line(kind: DiffLineKind) -> bool {
    matches!(kind, DiffLineKind::Addition | DiffLineKind::Removal)
}

fn displayed_hunk_patch_blocks(
    diff: &crate::state::DiffModel,
    hunk_index: usize,
) -> Result<Vec<SelectedHunk>, String> {
    let hunk = diff
        .hunks
        .get(hunk_index)
        .ok_or_else(|| "Select a hunk before staging it.".to_string())?;
    let mut block_start = None;
    let mut selections = Vec::new();

    for line_index in hunk.start_line_index + 1..hunk.end_line_index {
        if is_change_line(diff.lines[line_index].kind) {
            block_start.get_or_insert(line_index);
            continue;
        }

        if let Some(start) = block_start.take() {
            selections.push(selection_for_display_range(
                diff,
                hunk_index,
                start,
                line_index - 1,
            )?);
        }
    }

    if let Some(start) = block_start {
        selections.push(selection_for_display_range(
            diff,
            hunk_index,
            start,
            hunk.end_line_index.saturating_sub(1),
        )?);
    }

    Ok(selections)
}

fn selection_for_display_range(
    diff: &crate::state::DiffModel,
    hunk_index: usize,
    start_line_index: usize,
    end_line_index: usize,
) -> Result<SelectedHunk, String> {
    let hunk = diff
        .hunks
        .get(hunk_index)
        .ok_or_else(|| "Select a hunk before staging it.".to_string())?;
    if start_line_index > end_line_index
        || start_line_index <= hunk.start_line_index
        || end_line_index >= hunk.end_line_index
    {
        return Err("Select a valid changed line range inside the current hunk.".to_string());
    }

    let mut old_cursor = hunk.selection.old_start;
    let mut new_cursor = hunk.selection.new_start;
    let mut first_old_cursor = None;
    let mut first_new_cursor = None;
    let mut old_lines = 0_u32;
    let mut new_lines = 0_u32;

    for line_index in hunk.start_line_index + 1..hunk.end_line_index {
        let line = &diff.lines[line_index];
        let in_range = (start_line_index..=end_line_index).contains(&line_index);
        match line.kind {
            DiffLineKind::Context => {
                if in_range {
                    return Err(
                        "Line staging only works within one contiguous change block. Use Enter for the whole hunk."
                            .to_string(),
                    );
                }
                old_cursor = old_cursor.saturating_add(1);
                new_cursor = new_cursor.saturating_add(1);
            }
            DiffLineKind::Meta | DiffLineKind::HunkHeader => {
                if in_range {
                    return Err("Only added and removed lines can be selected.".to_string());
                }
            }
            DiffLineKind::Removal => {
                if in_range {
                    first_old_cursor.get_or_insert(old_cursor);
                    first_new_cursor.get_or_insert(new_cursor);
                    old_lines = old_lines.saturating_add(1);
                }
                old_cursor = old_cursor.saturating_add(1);
            }
            DiffLineKind::Addition => {
                if in_range {
                    first_old_cursor.get_or_insert(old_cursor);
                    first_new_cursor.get_or_insert(new_cursor);
                    new_lines = new_lines.saturating_add(1);
                }
                new_cursor = new_cursor.saturating_add(1);
            }
        }
    }

    if old_lines == 0 && new_lines == 0 {
        return Err("Select a changed line before staging lines.".to_string());
    }

    let old_start_cursor = first_old_cursor
        .ok_or_else(|| "Select a changed line before staging lines.".to_string())?;
    let new_start_cursor = first_new_cursor
        .ok_or_else(|| "Select a changed line before staging lines.".to_string())?;

    Ok(SelectedHunk {
        old_start: if old_lines > 0 {
            old_start_cursor
        } else {
            old_start_cursor.saturating_sub(1)
        },
        old_lines,
        new_start: if new_lines > 0 {
            new_start_cursor
        } else {
            new_start_cursor.saturating_sub(1)
        },
        new_lines,
    })
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

fn selected_editor_target(
    state: &AppState,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    match state.mode {
        AppMode::Workspace => {
            let Some(repo_id) = state.workspace.selected_repo_id.as_ref() else {
                return Err("Select a repository before opening it in the editor.".to_string());
            };
            let repo_root = repo_root_for_id(state, repo_id);
            Ok((repo_root.clone(), repo_root))
        }
        AppMode::Repository => {
            let Some(repo_mode) = state.repo_mode.as_ref() else {
                return Err("Open a repository before using the editor.".to_string());
            };
            let repo_root = repo_root_for_id(state, &repo_mode.current_repo_id);
            let Some(target) = selected_repo_editor_path(state, repo_mode, &repo_root) else {
                return Err("Select a status file before opening it in the editor.".to_string());
            };
            Ok((repo_root, target))
        }
    }
}

fn repo_root_for_id(state: &AppState, repo_id: &crate::state::RepoId) -> std::path::PathBuf {
    state
        .workspace
        .repo_summaries
        .get(repo_id)
        .map(|summary| summary.real_path.clone())
        .unwrap_or_else(|| std::path::PathBuf::from(&repo_id.0))
}

fn selected_repo_editor_path(
    state: &AppState,
    repo_mode: &RepoModeState,
    repo_root: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let path = match state.focused_pane {
        PaneId::RepoUnstaged => selected_status_path(repo_mode, PaneId::RepoUnstaged),
        PaneId::RepoStaged => selected_status_path(repo_mode, PaneId::RepoStaged),
        PaneId::RepoDetail if repo_mode.active_subview == crate::state::RepoSubview::Status => {
            repo_mode
                .detail
                .as_ref()
                .and_then(|detail| detail.diff.selected_path.clone())
        }
        _ => None,
    }?;

    Some(if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    })
}

fn selected_discard_path(state: &AppState) -> Option<(crate::state::RepoId, std::path::PathBuf)> {
    let repo_mode = state.repo_mode.as_ref()?;
    let path = match state.focused_pane {
        PaneId::RepoUnstaged => selected_status_path(repo_mode, PaneId::RepoUnstaged),
        PaneId::RepoStaged => selected_status_path(repo_mode, PaneId::RepoStaged),
        PaneId::RepoDetail if repo_mode.active_subview == crate::state::RepoSubview::Status => {
            repo_mode
                .detail
                .as_ref()
                .and_then(|detail| detail.diff.selected_path.clone())
        }
        _ => None,
    }?;
    Some((repo_mode.current_repo_id.clone(), path))
}

fn selected_branch_item(repo_mode: &RepoModeState) -> Option<&crate::state::BranchItem> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .branches_view
        .selected_index
        .filter(|index| *index < detail.branches.len())
        .or_else(|| detail.branches.iter().position(|branch| branch.is_head))
        .unwrap_or(0);
    detail.branches.get(selected_index)
}

fn selected_commit_item(repo_mode: &RepoModeState) -> Option<&crate::state::CommitItem> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .commits_view
        .selected_index
        .filter(|index| *index < detail.commits.len())
        .unwrap_or(0);
    detail.commits.get(selected_index)
}

fn selected_reflog_restore_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Ok(None);
    };
    if let Some(message) = history_action_block_reason(&detail.merge_state) {
        return Err(message.to_string());
    }
    if !detail.file_tree.is_empty() {
        return Err(
            "Restore is only available on a clean working tree. Commit or discard changes first."
                .to_string(),
        );
    }

    let selected_index = repo_mode
        .reflog_view
        .selected_index
        .filter(|index| *index < detail.reflog_items.len())
        .unwrap_or(0);
    if selected_index == 0 {
        return Err("Select an older reflog entry to restore.".to_string());
    }

    let Some(entry) = detail.reflog_items.get(selected_index) else {
        return Ok(None);
    };
    let Some((target, _)) = entry.description.split_once(':') else {
        return Err("Selected reflog entry could not be parsed.".to_string());
    };

    Ok(Some((
        repo_mode.current_repo_id.clone(),
        target.trim().to_string(),
        entry.description.clone(),
    )))
}

fn pending_history_commit_operation<T, F>(
    state: &AppState,
    build: F,
) -> Result<Option<(crate::state::RepoId, T)>, String>
where
    F: FnOnce(&crate::state::RepoDetail, &crate::state::CommitItem, usize) -> Result<T, String>,
{
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Ok(None);
    };
    if let Some(message) = history_action_block_reason(&detail.merge_state) {
        return Err(message.to_string());
    }
    let selected_index = repo_mode
        .commits_view
        .selected_index
        .filter(|index| *index < detail.commits.len())
        .unwrap_or(0);
    let Some(commit) = detail.commits.get(selected_index) else {
        return Ok(None);
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        build(detail, commit, selected_index)?,
    )))
}

fn history_action_block_reason(merge_state: &MergeState) -> Option<&'static str> {
    match merge_state {
        MergeState::None => None,
        MergeState::MergeInProgress => Some("A merge is already in progress."),
        MergeState::RebaseInProgress => Some("A rebase is already in progress."),
        MergeState::CherryPickInProgress => Some("A cherry-pick is already in progress."),
        MergeState::RevertInProgress => Some("A revert is already in progress."),
    }
}

fn open_reset_confirmation(state: &mut AppState, mode: ResetMode) -> bool {
    let Some((repo_id, commit, summary)) = state.repo_mode.as_ref().and_then(|repo_mode| {
        selected_commit_item(repo_mode).map(|commit| {
            (
                repo_mode.current_repo_id.clone(),
                commit.oid.clone(),
                format!("{} {}", commit.short_oid, commit.summary),
            )
        })
    }) else {
        return false;
    };

    open_confirmation_modal(
        state,
        repo_id,
        ConfirmableOperation::ResetToCommit {
            mode,
            commit,
            summary,
        },
    );
    true
}

fn selected_comparison_target(repo_mode: &RepoModeState) -> Option<ComparisonTarget> {
    match repo_mode.active_subview {
        crate::state::RepoSubview::Branches => selected_branch_item(repo_mode)
            .map(|branch| ComparisonTarget::Branch(branch.name.clone())),
        crate::state::RepoSubview::Commits => selected_commit_item(repo_mode)
            .map(|commit| ComparisonTarget::Commit(commit.oid.clone())),
        _ => None,
    }
}

fn selected_stash_item(repo_mode: &RepoModeState) -> Option<&crate::state::StashItem> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .stash_view
        .selected_index
        .filter(|index| *index < detail.stashes.len())
        .unwrap_or(0);
    detail.stashes.get(selected_index)
}

fn selected_worktree_item(repo_mode: &RepoModeState) -> Option<&crate::state::WorktreeItem> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .worktree_view
        .selected_index
        .filter(|index| *index < detail.worktrees.len())
        .unwrap_or(0);
    detail.worktrees.get(selected_index)
}

fn step_branch_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.branches_view.selected_index = None;
        return false;
    };

    let before = repo_mode.branches_view.selected_index;
    let after = repo_mode
        .branches_view
        .select_with_step(detail.branches.len(), step);
    after.is_some() && after != before
}

fn step_stash_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.stash_view.selected_index = None;
        return false;
    };

    let before = repo_mode.stash_view.selected_index;
    let after = repo_mode
        .stash_view
        .select_with_step(detail.stashes.len(), step);
    after.is_some() && after != before
}

fn step_reflog_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.reflog_view.selected_index = None;
        return false;
    };

    let before = repo_mode.reflog_view.selected_index;
    let after = repo_mode
        .reflog_view
        .select_with_step(detail.reflog_items.len(), step);
    after.is_some() && after != before
}

fn step_worktree_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.worktree_view.selected_index = None;
        return false;
    };

    let before = repo_mode.worktree_view.selected_index;
    let after = repo_mode
        .worktree_view
        .select_with_step(detail.worktrees.len(), step);
    after.is_some() && after != before
}

fn diff_presentation_for_pane(pane: PaneId) -> DiffPresentation {
    match pane {
        PaneId::RepoStaged => DiffPresentation::Staged,
        _ => DiffPresentation::Unstaged,
    }
}

fn load_comparison_diff_effect(repo_mode: &RepoModeState) -> Effect {
    Effect::LoadRepoDiff {
        repo_id: repo_mode.current_repo_id.clone(),
        comparison_target: repo_mode.comparison_base.clone(),
        compare_with: repo_mode.comparison_target.clone(),
        selected_path: None,
        diff_presentation: DiffPresentation::Comparison,
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
        .map(|(selected_path, diff_presentation)| {
            if diff_presentation == DiffPresentation::Comparison {
                (None, DiffPresentation::Unstaged)
            } else {
                (selected_path, diff_presentation)
            }
        })
        .unwrap_or((None, DiffPresentation::Unstaged));
    Effect::LoadRepoDetail {
        repo_id,
        selected_path,
        diff_presentation,
    }
}

fn toggle_comparison_selection(state: &mut AppState, effects: &mut Vec<Effect>) {
    let Some(repo_mode) = state.repo_mode.as_mut() else {
        return;
    };
    let Some(target) = selected_comparison_target(repo_mode) else {
        return;
    };

    let source = repo_mode.active_subview;
    if repo_mode.comparison_source != Some(source) {
        repo_mode.comparison_base = None;
        repo_mode.comparison_target = None;
    }

    match repo_mode.comparison_base.clone() {
        None => {
            repo_mode.comparison_base = Some(target);
            repo_mode.comparison_target = None;
            repo_mode.comparison_source = Some(source);
            effects.push(Effect::ScheduleRender);
        }
        Some(base) if base == target => {
            effects.push(Effect::ScheduleRender);
        }
        Some(_) => {
            repo_mode.comparison_base = repo_mode.comparison_base.clone().or(Some(target.clone()));
            repo_mode.comparison_target = Some(target);
            repo_mode.comparison_source = Some(source);
            repo_mode.active_subview = crate::state::RepoSubview::Compare;
            repo_mode.diff_scroll = 0;
            close_commit_box(repo_mode);
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_comparison_diff_effect(repo_mode));
            effects.push(Effect::ScheduleRender);
        }
    }
}

fn clear_comparison(state: &mut AppState, effects: &mut Vec<Effect>) {
    let mut repo_detail_reload = None;
    if let Some(repo_mode) = state.repo_mode.as_mut() {
        if repo_mode.active_subview == crate::state::RepoSubview::Compare {
            repo_mode.active_subview = repo_mode
                .comparison_source
                .unwrap_or(crate::state::RepoSubview::Commits);
        }
        if repo_mode
            .detail
            .as_ref()
            .is_some_and(|detail| detail.diff.presentation == DiffPresentation::Comparison)
        {
            repo_detail_reload = Some(repo_mode.current_repo_id.clone());
        }
        repo_mode.comparison_base = None;
        repo_mode.comparison_target = None;
        repo_mode.comparison_source = None;
        repo_mode.diff_scroll = 0;
        state.focused_pane = PaneId::RepoDetail;
        effects.push(Effect::ScheduleRender);
    }
    if let Some(repo_id) = repo_detail_reload {
        effects.push(load_repo_detail_effect(state, repo_id));
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
    let selected_hunk_index = diff.selected_hunk?;
    let selections = displayed_hunk_patch_blocks(diff, selected_hunk_index).ok()?;
    if selections.is_empty() {
        return None;
    }

    Some(patch_job(
        repo_mode.current_repo_id.clone(),
        path,
        mode,
        selections,
    ))
}

fn selected_line_patch_job(
    state: &AppState,
    mode: PatchApplicationMode,
) -> Result<Option<PatchSelectionJob>, String> {
    let repo_mode = match state.repo_mode.as_ref() {
        Some(repo_mode) => repo_mode,
        None => return Ok(None),
    };
    let detail = match repo_mode.detail.as_ref() {
        Some(detail) => detail,
        None => return Ok(None),
    };
    let diff = &detail.diff;
    if diff.presentation == DiffPresentation::Comparison {
        return Ok(None);
    }

    let expected_presentation = match mode {
        PatchApplicationMode::Stage => DiffPresentation::Unstaged,
        PatchApplicationMode::Unstage => DiffPresentation::Staged,
    };
    if diff.presentation != expected_presentation {
        return Ok(None);
    }

    let path = match diff.selected_path.clone() {
        Some(path) => path,
        None => return Ok(None),
    };
    let hunk_index = diff
        .selected_hunk
        .ok_or_else(|| "Select a hunk before staging lines.".to_string())?;
    let cursor = repo_mode
        .diff_line_cursor
        .ok_or_else(|| "Use J/K to pick a changed line before staging lines.".to_string())?;
    let anchor = repo_mode.diff_line_anchor.unwrap_or(cursor);
    let start_line_index = anchor.min(cursor);
    let end_line_index = anchor.max(cursor);
    let selection =
        selection_for_display_range(diff, hunk_index, start_line_index, end_line_index)?;

    Ok(Some(patch_job(
        repo_mode.current_repo_id.clone(),
        path,
        mode,
        vec![selection],
    )))
}

fn job_suffix(command: &GitCommand) -> &'static str {
    match command {
        GitCommand::StageSelection => "stage-selection",
        GitCommand::StageFile { .. } => "stage-file",
        GitCommand::DiscardFile { .. } => "discard-file",
        GitCommand::UnstageFile { .. } => "unstage-file",
        GitCommand::CommitStaged { .. } => "commit-staged",
        GitCommand::CommitStagedNoVerify { .. } => "commit-staged-no-verify",
        GitCommand::CommitStagedWithEditor => "commit-staged-editor",
        GitCommand::AmendHead { .. } => "amend-head",
        GitCommand::RewordCommitWithEditor { .. } => "reword-commit-editor",
        GitCommand::StartCommitRebase { mode, .. } => match mode {
            RebaseStartMode::Interactive => "start-interactive-rebase",
            RebaseStartMode::Amend => "start-amend-rebase",
            RebaseStartMode::Fixup => "start-fixup-rebase",
            RebaseStartMode::Reword { .. } => "start-reword-rebase",
        },
        GitCommand::CherryPickCommit { .. } => "cherry-pick-commit",
        GitCommand::RevertCommit { .. } => "revert-commit",
        GitCommand::ResetToCommit { mode, .. } => match mode {
            ResetMode::Soft => "reset-soft",
            ResetMode::Mixed => "reset-mixed",
            ResetMode::Hard => "reset-hard",
        },
        GitCommand::RestoreSnapshot { .. } => "restore-snapshot",
        GitCommand::ContinueRebase => "continue-rebase",
        GitCommand::AbortRebase => "abort-rebase",
        GitCommand::SkipRebase => "skip-rebase",
        GitCommand::CreateBranch { .. } => "create-branch",
        GitCommand::CheckoutBranch { .. } => "checkout-branch",
        GitCommand::RenameBranch { .. } => "rename-branch",
        GitCommand::DeleteBranch { .. } => "delete-branch",
        GitCommand::ApplyStash { .. } => "apply-stash",
        GitCommand::DropStash { .. } => "drop-stash",
        GitCommand::CreateWorktree { .. } => "create-worktree",
        GitCommand::RemoveWorktree { .. } => "remove-worktree",
        GitCommand::SetBranchUpstream { .. } => "set-branch-upstream",
        GitCommand::FetchSelectedRepo => "fetch-selected-repo",
        GitCommand::PullCurrentBranch => "pull-current-branch",
        GitCommand::PushCurrentBranch => "push-current-branch",
        GitCommand::NukeWorkingTree => "nuke-working-tree",
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
    use crate::effect::{Effect, GitCommand, GitCommandRequest, RebaseStartMode};
    use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
    use crate::state::{
        AppMode, AppState, BackgroundJobKind, BackgroundJobState, CommitBoxMode, CommitFileItem,
        CommitItem, ConfirmableOperation, DiffHunk, DiffLine, DiffLineKind, DiffModel,
        DiffPresentation, FileStatus, FileStatusKind, InputPromptOperation, JobId, MergeState,
        MessageLevel, ModalKind, PaneId, RebaseKind, RebaseState, ReflogItem, RepoDetail, RepoId,
        RepoModeState, RepoSubview, RepoSummary, ScanStatus, SelectedHunk, StashItem, Timestamp,
        WatcherHealth, WorkspaceFilterMode, WorktreeItem,
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
    fn open_in_editor_from_workspace_targets_selected_repo_root() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let state = AppState {
            workspace: crate::state::WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    RepoSummary {
                        repo_id: repo_id.clone(),
                        display_name: "repo-1".to_string(),
                        real_path: repo_root.clone(),
                        display_path: repo_root.display().to_string(),
                        ..RepoSummary::default()
                    },
                )]),
                selected_repo_id: Some(repo_id),
                ..crate::state::WorkspaceState::default()
            },
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenInEditor));

        assert_eq!(
            result.effects,
            vec![Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: repo_root,
            }]
        );
    }

    #[test]
    fn open_in_editor_from_repo_mode_targets_selected_status_file() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: crate::state::WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    RepoSummary {
                        repo_id: repo_id.clone(),
                        display_name: "repo-1".to_string(),
                        real_path: repo_root.clone(),
                        display_path: repo_root.display().to_string(),
                        ..RepoSummary::default()
                    },
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..crate::state::WorkspaceState::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail {
                    diff: DiffModel {
                        selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                        presentation: DiffPresentation::Unstaged,
                        ..DiffModel::default()
                    },
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenInEditor));

        assert_eq!(
            result.effects,
            vec![Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: repo_root.join("src/lib.rs"),
            }]
        );
    }

    #[test]
    fn open_in_editor_without_selection_pushes_warning() {
        let result = reduce(AppState::default(), Event::Action(Action::OpenInEditor));

        assert!(result.effects.contains(&Effect::ScheduleRender));
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select a repository before opening it in the editor.")
        );
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
    fn leave_repo_mode_preserves_workspace_context() {
        let repo_alpha = RepoId::new("repo-alpha");
        let repo_beta = RepoId::new("repo-beta");
        let mut state = AppState::default();
        state.workspace.discovered_repo_ids = vec![repo_alpha.clone(), repo_beta.clone()];
        state.workspace.selected_repo_id = Some(repo_beta.clone());
        state.workspace.filter_mode = WorkspaceFilterMode::DirtyOnly;
        state.workspace.search_query = "beta".to_string();
        state.workspace.search_focused = true;
        state
            .workspace
            .repo_summaries
            .insert(repo_alpha.clone(), workspace_summary(&repo_alpha.0));
        state
            .workspace
            .repo_summaries
            .insert(repo_beta.clone(), workspace_summary(&repo_beta.0));

        let entered = reduce(
            state,
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_beta.clone(),
            }),
        )
        .state;

        assert_eq!(entered.workspace.selected_repo_id, Some(repo_beta.clone()));
        assert_eq!(
            entered.workspace.filter_mode,
            WorkspaceFilterMode::DirtyOnly
        );
        assert_eq!(entered.workspace.search_query, "beta");
        assert!(!entered.workspace.search_focused);

        let left = reduce(entered, Event::Action(Action::LeaveRepoMode)).state;

        assert_eq!(left.mode, AppMode::Workspace);
        assert_eq!(left.focused_pane, PaneId::WorkspaceList);
        assert_eq!(left.workspace.selected_repo_id, Some(repo_beta));
        assert_eq!(left.workspace.filter_mode, WorkspaceFilterMode::DirtyOnly);
        assert_eq!(left.workspace.search_query, "beta");
        assert!(!left.workspace.search_focused);
        assert_eq!(left.workspace.discovered_repo_ids.len(), 2);
        assert!(left.repo_mode.is_none());
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
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
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
    fn delete_selected_branch_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: None,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DeleteSelectedBranch));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::DeleteBranch {
                    branch_name: "feature".to_string()
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn hard_reset_selected_commit_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        crate::state::CommitItem {
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            summary: "add lib".to_string(),
                            changed_files: vec![],
                            diff: DiffModel::default(),
                        },
                        crate::state::CommitItem {
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            changed_files: vec![],
                            diff: DiffModel::default(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::HardResetToSelectedCommit));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::ResetToCommit {
                    mode: crate::state::ResetMode::Hard,
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn select_next_stash_advances_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                detail: Some(RepoDetail {
                    stashes: vec![
                        StashItem {
                            stash_ref: "stash@{0}".to_string(),
                            label: "stash@{0}: latest".to_string(),
                        },
                        StashItem {
                            stash_ref: "stash@{1}".to_string(),
                            label: "stash@{1}: older".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                stash_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SelectNextStash));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index),
            Some(1)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn select_next_reflog_advances_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Reflog,
                detail: Some(RepoDetail {
                    reflog_items: vec![
                        ReflogItem {
                            description: "HEAD@{0}: checkout".to_string(),
                        },
                        ReflogItem {
                            description: "HEAD@{1}: commit".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SelectNextReflog));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.reflog_view.selected_index),
            Some(1)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn restore_selected_reflog_entry_opens_confirmation_on_clean_tree() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Reflog,
                detail: Some(RepoDetail {
                    reflog_items: vec![
                        ReflogItem {
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            description: "HEAD@{1}: commit: prior".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RestoreSelectedReflogEntry));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::RestoreReflogEntry {
                    target: "HEAD@{1}".to_string(),
                    summary: "HEAD@{1}: commit: prior".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn restore_selected_reflog_entry_warns_when_worktree_is_dirty() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Reflog,
                detail: Some(RepoDetail {
                    file_tree: vec![crate::state::FileStatus {
                        path: std::path::PathBuf::from("dirty.txt"),
                        kind: crate::state::FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(crate::state::FileStatusKind::Modified),
                    }],
                    reflog_items: vec![
                        ReflogItem {
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            description: "HEAD@{1}: commit: prior".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RestoreSelectedReflogEntry));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .front()
                .map(|notification| notification.text.as_str()),
            Some("Restore is only available on a clean working tree. Commit or discard changes first.")
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn select_next_worktree_advances_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Worktrees,
                detail: Some(RepoDetail {
                    worktrees: vec![
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-main"),
                            branch: Some("main".to_string()),
                        },
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-feature"),
                            branch: Some("feature".to_string()),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                worktree_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SelectNextWorktree));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            Some(1)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn apply_selected_stash_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                detail: Some(RepoDetail {
                    stashes: vec![StashItem {
                        stash_ref: "stash@{0}".to_string(),
                        label: "stash@{0}: latest".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                stash_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ApplySelectedStash));
        let job_id = JobId::new("git:repo-1:apply-stash");

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
                command: GitCommand::ApplyStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            })]
        );
    }

    #[test]
    fn drop_selected_stash_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                detail: Some(RepoDetail {
                    stashes: vec![StashItem {
                        stash_ref: "stash@{0}".to_string(),
                        label: "stash@{0}: latest".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                stash_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DropSelectedStash));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::DropStash {
                    stash_ref: "stash@{0}".to_string()
                }
            ))
        );
        assert_eq!(
            result
                .state
                .modal_stack
                .last()
                .map(|modal| (&modal.kind, modal.title.as_str())),
            Some((&ModalKind::Confirm, "Drop stash stash@{0}"))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn create_worktree_opens_input_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CreateWorktree));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.value.clone()
            )),
            Some((repo_id, InputPromptOperation::CreateWorktree, String::new()))
        );
    }

    #[test]
    fn remove_selected_worktree_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let removable_path = std::path::PathBuf::from("/tmp/repo-feature");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Worktrees,
                detail: Some(RepoDetail {
                    worktrees: vec![WorktreeItem {
                        path: removable_path.clone(),
                        branch: Some("feature".to_string()),
                    }],
                    ..RepoDetail::default()
                }),
                worktree_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RemoveSelectedWorktree));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::RemoveWorktree {
                    path: removable_path,
                }
            ))
        );
    }

    #[test]
    fn submit_branch_create_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Create branch",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateBranch,
                value: "feature/new-ui".to_string(),
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-branch");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
                command: GitCommand::CreateBranch {
                    branch_name: "feature/new-ui".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_worktree_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Create worktree",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateWorktree,
                value: "../repo-feature feature".to_string(),
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-worktree");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CreateWorktree {
                    path: std::path::PathBuf::from("../repo-feature"),
                    branch_ref: "feature".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_reword_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Reword old1234 older commit",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::RewordCommit {
                    commit: "older".to_string(),
                    summary: "old1234 older commit".to_string(),
                    initial_message: "older commit".to_string(),
                },
                value: "reworded subject".to_string(),
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:start-reword-rebase");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: "older".to_string(),
                    mode: RebaseStartMode::Reword {
                        message: "reworded subject".to_string(),
                    },
                },
            })]
        );
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
    fn confirm_pending_operation_queues_drop_stash_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Drop stash stash@{0}",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::DropStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:drop-stash");

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
                command: GitCommand::DropStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_restore_snapshot_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Restore HEAD to HEAD@{1}: commit: prior",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::RestoreReflogEntry {
                    target: "HEAD@{1}".to_string(),
                    summary: "HEAD@{1}: commit: prior".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:restore-snapshot");

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
                command: GitCommand::RestoreSnapshot {
                    target: "HEAD@{1}".to_string(),
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_fixup_rebase_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Fixup old1234 older commit",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::FixupCommit {
                    commit: "older".to_string(),
                    summary: "old1234 older commit".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:start-fixup-rebase");

        assert!(result.state.modal_stack.is_empty());
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: "older".to_string(),
                    mode: RebaseStartMode::Fixup,
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_reset_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Hard reset to 1234567 second",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::ResetToCommit {
                    mode: crate::state::ResetMode::Hard,
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:reset-hard");

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
                command: GitCommand::ResetToCommit {
                    mode: crate::state::ResetMode::Hard,
                    target: "1234567890abcdef".to_string(),
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_cherry_pick_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Cherry-pick 1234567 second",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::CherryPickCommit {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:cherry-pick-commit");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CherryPickCommit {
                    commit: "1234567890abcdef".to_string(),
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_revert_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Revert 1234567 second",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::RevertCommit {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:revert-commit");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::RevertCommit {
                    commit: "1234567890abcdef".to_string(),
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_nuke_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Discard all local changes",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::NukeWorkingTree,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:nuke-working-tree");

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
                command: GitCommand::NukeWorkingTree,
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
    fn switch_repo_subview_branches_prefers_head_selection() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: None,
                        },
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Branches)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            None
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
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            None
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
    fn toggle_comparison_selection_marks_base_from_commits() {
        let detail = RepoDetail {
            commits: vec![
                CommitItem {
                    oid: "abcdef1234567890".to_string(),
                    short_oid: "abcdef1".to_string(),
                    summary: "add lib".to_string(),
                    ..CommitItem::default()
                },
                CommitItem {
                    oid: "1234567890abcdef".to_string(),
                    short_oid: "1234567".to_string(),
                    summary: "second".to_string(),
                    ..CommitItem::default()
                },
            ],
            ..RepoDetail::default()
        };
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ToggleComparisonSelection));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_base.clone()),
            Some(crate::state::ComparisonTarget::Commit(
                "abcdef1234567890".to_string()
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            None
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_source),
            Some(RepoSubview::Commits)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn toggle_comparison_selection_on_second_commit_opens_compare_diff() {
        let detail = RepoDetail {
            commits: vec![
                CommitItem {
                    oid: "abcdef1234567890".to_string(),
                    short_oid: "abcdef1".to_string(),
                    summary: "add lib".to_string(),
                    ..CommitItem::default()
                },
                CommitItem {
                    oid: "1234567890abcdef".to_string(),
                    short_oid: "1234567".to_string(),
                    summary: "second".to_string(),
                    ..CommitItem::default()
                },
            ],
            ..RepoDetail::default()
        };
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                comparison_base: Some(crate::state::ComparisonTarget::Commit(
                    "abcdef1234567890".to_string(),
                )),
                comparison_source: Some(RepoSubview::Commits),
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ToggleComparisonSelection));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Compare)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            Some(crate::state::ComparisonTarget::Commit(
                "1234567890abcdef".to_string()
            ))
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDiff {
                    repo_id: RepoId::new("repo-1"),
                    comparison_target: Some(crate::state::ComparisonTarget::Commit(
                        "abcdef1234567890".to_string(),
                    )),
                    compare_with: Some(crate::state::ComparisonTarget::Commit(
                        "1234567890abcdef".to_string(),
                    )),
                    selected_path: None,
                    diff_presentation: DiffPresentation::Comparison,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn clear_comparison_returns_to_history_and_restores_normal_detail_loading() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Compare,
                comparison_base: Some(crate::state::ComparisonTarget::Commit(
                    "abcdef1234567890".to_string(),
                )),
                comparison_target: Some(crate::state::ComparisonTarget::Commit(
                    "1234567890abcdef".to_string(),
                )),
                comparison_source: Some(RepoSubview::Commits),
                detail: Some(RepoDetail {
                    diff: DiffModel {
                        presentation: DiffPresentation::Comparison,
                        ..DiffModel::default()
                    },
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ClearComparison));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_base.clone()),
            None
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            None
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::ScheduleRender,
                Effect::LoadRepoDetail {
                    repo_id: RepoId::new("repo-1"),
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
            ]
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
    fn discard_selected_file_opens_confirmation_modal() {
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

        let result = reduce(state, Event::Action(Action::DiscardSelectedFile));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::DiscardFile {
                    path: std::path::PathBuf::from("README.md")
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn nuke_working_tree_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::NukeWorkingTree));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((repo_id, ConfirmableOperation::NukeWorkingTree))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
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
    fn stage_selected_hunk_splits_displayed_hunk_into_zero_context_blocks() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail {
                    diff: DiffModel {
                        selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                        presentation: DiffPresentation::Unstaged,
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::HunkHeader,
                                content: "@@ -1,4 +1,4 @@".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Removal,
                                content: "-before".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+after".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Context,
                                content: " shared".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Removal,
                                content: "-tail before".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+tail after".to_string(),
                            },
                        ],
                        hunks: vec![DiffHunk {
                            header: "@@ -1,4 +1,4 @@".to_string(),
                            selection: SelectedHunk {
                                old_start: 1,
                                old_lines: 4,
                                new_start: 1,
                                new_lines: 4,
                            },
                            start_line_index: 0,
                            end_line_index: 6,
                        }],
                        selected_hunk: Some(0),
                        hunk_count: 1,
                    },
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StageSelectedHunk));
        let job_id = JobId::new("git:repo-1:stage-hunk");

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
            vec![Effect::RunPatchSelection(
                crate::effect::PatchSelectionJob {
                    job_id,
                    repo_id,
                    path: std::path::PathBuf::from("src/lib.rs"),
                    mode: crate::effect::PatchApplicationMode::Stage,
                    hunks: vec![
                        SelectedHunk {
                            old_start: 1,
                            old_lines: 1,
                            new_start: 1,
                            new_lines: 1,
                        },
                        SelectedHunk {
                            old_start: 3,
                            old_lines: 1,
                            new_start: 3,
                            new_lines: 1,
                        },
                    ],
                }
            )]
        );
    }

    #[test]
    fn stage_selected_lines_uses_cursor_and_anchor_range() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                diff_line_cursor: Some(2),
                diff_line_anchor: Some(1),
                detail: Some(RepoDetail {
                    diff: DiffModel {
                        selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                        presentation: DiffPresentation::Unstaged,
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::HunkHeader,
                                content: "@@ -1 +1 @@".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Removal,
                                content: "-old".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+new".to_string(),
                            },
                        ],
                        hunks: vec![DiffHunk {
                            header: "@@ -1 +1 @@".to_string(),
                            selection: SelectedHunk {
                                old_start: 1,
                                old_lines: 1,
                                new_start: 1,
                                new_lines: 1,
                            },
                            start_line_index: 0,
                            end_line_index: 3,
                        }],
                        selected_hunk: Some(0),
                        hunk_count: 1,
                    },
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StageSelectedLines));
        let job_id = JobId::new("git:repo-1:stage-hunk");

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
            vec![Effect::RunPatchSelection(
                crate::effect::PatchSelectionJob {
                    job_id,
                    repo_id,
                    path: std::path::PathBuf::from("src/lib.rs"),
                    mode: crate::effect::PatchApplicationMode::Stage,
                    hunks: vec![SelectedHunk {
                        old_start: 1,
                        old_lines: 1,
                        new_start: 1,
                        new_lines: 1,
                    }],
                }
            )]
        );
    }

    #[test]
    fn stage_selected_lines_warns_when_range_crosses_context() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                diff_line_cursor: Some(5),
                diff_line_anchor: Some(1),
                detail: Some(RepoDetail {
                    diff: DiffModel {
                        selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                        presentation: DiffPresentation::Unstaged,
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::HunkHeader,
                                content: "@@ -1,4 +1,4 @@".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Removal,
                                content: "-before".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+after".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Context,
                                content: " shared".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Removal,
                                content: "-tail before".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+tail after".to_string(),
                            },
                        ],
                        hunks: vec![DiffHunk {
                            header: "@@ -1,4 +1,4 @@".to_string(),
                            selection: SelectedHunk {
                                old_start: 1,
                                old_lines: 4,
                                new_start: 1,
                                new_lines: 4,
                            },
                            start_line_index: 0,
                            end_line_index: 6,
                        }],
                        selected_hunk: Some(0),
                        hunk_count: 1,
                    },
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StageSelectedLines));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| (&notification.level, notification.text.as_str())),
            Some((
                &MessageLevel::Warning,
                "Line staging only works within one contiguous change block. Use Enter for the whole hunk."
            ))
        );
        assert!(!result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::RunPatchSelection(_))));
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
    fn submit_commit_box_queues_no_verify_commit_when_requested() {
        let repo_id = RepoId::new("repo-1");
        let mut detail = repo_detail_with_file_tree();
        detail.commit_input = "ship without hooks".to_string();
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(detail),
                commit_box: crate::state::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::CommitNoVerify,
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitCommitBox));
        let job_id = JobId::new("git:repo-1:commit-staged-no-verify");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: job_id.clone(),
                repo_id,
                command: GitCommand::CommitStagedNoVerify {
                    message: "ship without hooks".to_string(),
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
                summary: "Commit staged changes without hooks".to_string(),
            })
        );
    }

    #[test]
    fn commit_staged_with_editor_queues_job_when_staged_changes_exist() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CommitStagedWithEditor));

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:commit-staged-editor"),
                repo_id,
                command: GitCommand::CommitStagedWithEditor,
            })]
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: JobId::new("git:repo-1:commit-staged-editor"),
                summary: "Commit staged changes with editor".to_string(),
            })
        );
    }

    #[test]
    fn commit_staged_with_editor_warns_without_staged_changes() {
        let mut detail = repo_detail_with_file_tree();
        for item in &mut detail.file_tree {
            item.staged_kind = None;
        }
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(detail),
                ..crate::state::RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CommitStagedWithEditor));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| (&notification.level, notification.text.as_str())),
            Some((
                &MessageLevel::Warning,
                "Stage at least one file before committing."
            ))
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
            branches: vec![
                crate::state::BranchItem {
                    name: "feature".to_string(),
                    is_head: false,
                    upstream: None,
                },
                crate::state::BranchItem {
                    name: "main".to_string(),
                    is_head: true,
                    upstream: Some("origin/main".to_string()),
                },
            ],
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
            reflog_items: vec![ReflogItem {
                description: "HEAD@{0}: commit: add lib".to_string(),
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
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.reflog_view.selected_index),
            Some(0)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_target.clone()),
            None
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
    fn start_interactive_rebase_requires_older_commit_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "older".to_string(),
                            summary: "older".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StartInteractiveRebase));

        assert!(result.effects.contains(&Effect::ScheduleRender));
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select an older commit before starting an interactive rebase.")
        );
    }

    #[test]
    fn start_interactive_rebase_opens_confirmation_for_selected_commit() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StartInteractiveRebase));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::StartInteractiveRebase {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn amend_selected_commit_opens_confirmation_for_selected_commit() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::AmendSelectedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::AmendCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn fixup_selected_commit_requires_staged_changes() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::FixupSelectedCommit));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Stage changes before starting fixup.")
        );
    }

    #[test]
    fn reword_selected_commit_opens_prompt_with_selected_summary() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("tracked.txt"),
                        kind: FileStatusKind::Modified,
                        staged_kind: Some(FileStatusKind::Modified),
                        unstaged_kind: None,
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RewordSelectedCommit));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(InputPromptOperation::RewordCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
                initial_message: "older commit".to_string(),
            })
        );
    }

    #[test]
    fn reword_selected_commit_with_editor_queues_job_for_selected_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RewordSelectedCommitWithEditor));

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:reword-commit-editor"),
                repo_id,
                command: GitCommand::RewordCommitWithEditor {
                    commit: "older".to_string(),
                },
            })]
        );
    }

    #[test]
    fn cherry_pick_selected_commit_opens_confirmation_for_selected_commit() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CherryPickSelectedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::CherryPickCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn revert_selected_commit_opens_confirmation_for_selected_commit() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RevertSelectedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::RevertCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn repo_detail_loaded_switches_into_rebase_view_when_rebase_is_active() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "head".to_string(),
                        short_oid: "head".to_string(),
                        summary: "HEAD".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };
        let detail = RepoDetail {
            merge_state: MergeState::RebaseInProgress,
            rebase_state: Some(RebaseState {
                kind: RebaseKind::Interactive,
                step: 1,
                total: 2,
                current_commit: Some("older".to_string()),
                current_summary: Some("older commit".to_string()),
                ..RebaseState::default()
            }),
            commits: vec![CommitItem {
                oid: "older".to_string(),
                short_oid: "old1234".to_string(),
                summary: "older commit".to_string(),
                ..CommitItem::default()
            }],
            ..RepoDetail::default()
        };

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoDetailLoaded { repo_id, detail }),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Rebase)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
    }

    #[test]
    fn repo_detail_loaded_returns_to_commits_when_rebase_finishes() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Rebase,
                detail: Some(RepoDetail {
                    merge_state: MergeState::RebaseInProgress,
                    rebase_state: Some(RebaseState {
                        kind: RebaseKind::Interactive,
                        step: 1,
                        total: 2,
                        ..RebaseState::default()
                    }),
                    commits: vec![CommitItem {
                        oid: "older".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };
        let detail = RepoDetail {
            commits: vec![CommitItem {
                oid: "head".to_string(),
                short_oid: "head".to_string(),
                summary: "HEAD".to_string(),
                ..CommitItem::default()
            }],
            ..RepoDetail::default()
        };

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::RepoDetailLoaded { repo_id, detail }),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
    }

    #[test]
    fn continue_rebase_enqueues_git_command() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Rebase,
                detail: Some(RepoDetail {
                    merge_state: MergeState::RebaseInProgress,
                    rebase_state: Some(RebaseState {
                        kind: RebaseKind::Interactive,
                        step: 1,
                        total: 2,
                        ..RebaseState::default()
                    }),
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ContinueRebase));

        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RunGitCommand(GitCommandRequest {
                command: GitCommand::ContinueRebase,
                ..
            })
        )));
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
        assert_eq!(
            result.effects,
            vec![
                Effect::RefreshRepoSummary {
                    repo_id: RepoId::new("repo-1"),
                },
                Effect::LoadRepoDetail {
                    repo_id: RepoId::new("repo-1"),
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                },
                Effect::ScheduleRender,
            ]
        );
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
