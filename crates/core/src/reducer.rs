use crate::action::Action;
use crate::effect::{
    Effect, GitCommand, GitCommandRequest, PatchApplicationMode, PatchSelectionJob,
    RebaseStartMode, ShellCommandRequest,
};
use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
use crate::hosting_service;
use crate::state::{
    AppMode, AppState, BackgroundJob, BackgroundJobKind, BackgroundJobState, CommitBoxMode,
    CommitFilesMode, CommitHistoryMode, ComparisonTarget, ConfirmableOperation, DiffLineKind,
    DiffPresentation, InputPromptOperation, JobId, MenuOperation, MergeFastForwardPreference,
    MergeState, MergeVariant, MessageLevel, Notification, OperationProgress, PaneId,
    PendingInputPrompt, PendingMenu, RepoModeState, ResetMode, ScanStatus, SelectedHunk, StashMode,
    StatusMessage, WatcherHealth, MAX_RENAME_SIMILARITY_THRESHOLD, MIN_RENAME_SIMILARITY_THRESHOLD,
    RENAME_SIMILARITY_THRESHOLD_STEP,
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

fn enter_repo_mode_with_parents(
    state: &mut AppState,
    repo_id: crate::state::RepoId,
    parent_repo_ids: Vec<crate::state::RepoId>,
    effects: &mut Vec<Effect>,
) {
    state.mode = AppMode::Repository;
    state.focused_pane = PaneId::RepoUnstaged;
    state.workspace.search_focused = false;
    state.workspace.selected_repo_id = Some(repo_id.clone());
    push_recent_repo(state, repo_id.clone());
    state.repo_mode = Some(RepoModeState::new_with_parent(
        repo_id.clone(),
        parent_repo_ids,
    ));
    effects.push(Effect::LoadRepoDetail {
        repo_id,
        selected_path: None,
        diff_presentation: DiffPresentation::Unstaged,
        commit_ref: None,
        commit_history_mode: CommitHistoryMode::Linear,
        ignore_whitespace_in_diff: false,
        diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
        rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
    });
    effects.push(Effect::ScheduleRender);
}

fn reduce_action(state: &mut AppState, action: Action, effects: &mut Vec<Effect>) {
    match action {
        Action::EnterRepoMode { repo_id } => {
            enter_repo_mode_with_parents(state, repo_id, Vec::new(), effects);
        }
        Action::EnterNestedRepoMode {
            repo_id,
            parent_repo_id,
        } => {
            let mut parent_repo_ids = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.parent_repo_ids.clone())
                .unwrap_or_default();
            parent_repo_ids.push(parent_repo_id);
            enter_repo_mode_with_parents(state, repo_id, parent_repo_ids, effects);
        }
        Action::LeaveRepoMode => {
            let parent_repo_id = state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.parent_repo_ids.last().cloned());
            if let Some(parent_repo_id) = parent_repo_id {
                let mut remaining_parent_repo_ids = state
                    .repo_mode
                    .as_ref()
                    .map(|repo_mode| repo_mode.parent_repo_ids.clone())
                    .unwrap_or_default();
                remaining_parent_repo_ids.pop();
                enter_repo_mode_with_parents(
                    state,
                    parent_repo_id,
                    remaining_parent_repo_ids,
                    effects,
                );
                if let Some(repo_mode) = state.repo_mode.as_mut() {
                    repo_mode.active_subview = crate::state::RepoSubview::Submodules;
                    state.focused_pane = PaneId::RepoDetail;
                }
            } else {
                state.mode = AppMode::Workspace;
                state.focused_pane = PaneId::WorkspaceList;
                state.workspace.search_focused = false;
                state.repo_mode = None;
                effects.push(Effect::ScheduleRender);
            }
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
                    enqueue_selected_status_detail_load(repo_mode, state.focused_pane, effects);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_status_selection(repo_mode, state.focused_pane, -1) {
                    enqueue_selected_status_detail_load(repo_mode, state.focused_pane, effects);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectStatusEntry { pane, index } => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if select_status_entry_at(repo_mode, pane, index) {
                    enqueue_selected_status_detail_load(repo_mode, pane, effects);
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
        Action::SelectNextRemote => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_remote_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousRemote => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_remote_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectNextRemoteBranch => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_remote_branch_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousRemoteBranch => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_remote_branch_selection(repo_mode, -1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectNextTag => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_tag_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousTag => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_tag_selection(repo_mode, -1) {
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
        Action::PageDownRepoList { page_size } => {
            if select_repo_list_page(state, page_size as isize, effects) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::PageUpRepoList { page_size } => {
            if select_repo_list_page(state, -(page_size as isize), effects) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SelectFirstRepoListEntry => {
            if select_repo_list_edge(state, false, effects) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SelectLastRepoListEntry => {
            if select_repo_list_edge(state, true, effects) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenSelectedBranchCommits => {
            let Some((repo_id, branch_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) else {
                push_warning(state, "Select a branch before opening its commits.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::Commits;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.commit_history_mode = CommitHistoryMode::Linear;
                repo_mode.commit_history_ref = Some(branch_ref);
                repo_mode.pending_commit_selection_oid = None;
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::Commits);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenSelectedRemoteBranches => {
            let Some(remote_name) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_item(repo_mode).map(|remote| remote.name.clone())
            }) else {
                push_warning(state, "Select a remote before opening its branches.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::RemoteBranches;
                repo_mode.remote_branches_filter.query = remote_name;
                repo_mode.remote_branches_filter.focused = false;
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::RemoteBranches);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenSelectedRemoteBranchCommits => {
            let Some((repo_id, branch_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) else {
                push_warning(state, "Select a remote branch before opening its commits.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::Commits;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.commit_history_mode = CommitHistoryMode::Linear;
                repo_mode.commit_history_ref = Some(branch_ref);
                repo_mode.pending_commit_selection_oid = None;
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::Commits);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenSelectedTagCommits => {
            let Some((repo_id, tag_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode)
                    .map(|tag| (repo_mode.current_repo_id.clone(), tag.name.clone()))
            }) else {
                push_warning(state, "Select a tag before opening its commits.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::Commits;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.commit_history_mode = CommitHistoryMode::Linear;
                repo_mode.commit_history_ref = Some(tag_ref);
                repo_mode.pending_commit_selection_oid = None;
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::Commits);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenAllBranchGraph { reverse } => {
            let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            else {
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::Commits;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.commit_history_mode = CommitHistoryMode::Graph { reverse };
                repo_mode.commit_history_ref = None;
                repo_mode.pending_commit_selection_oid = None;
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::Commits);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenSelectedReflogCommits => {
            let Some((repo_id, pending_selection)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_reflog_entry(repo_mode).and_then(|(_, entry)| {
                        (!entry.oid.is_empty()).then_some((
                            repo_mode.current_repo_id.clone(),
                            pending_reflog_commit_selection(entry),
                        ))
                    })
                })
            else {
                push_warning(
                    state,
                    "Select a reflog entry that still points to a commit before opening commit history.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };

            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.active_subview = crate::state::RepoSubview::Commits;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.commit_history_mode = CommitHistoryMode::Reflog;
                repo_mode.commit_history_ref = None;
                repo_mode.pending_commit_selection_oid = Some(pending_selection);
                repo_mode.commits_filter = crate::state::RepoSubviewFilterState::default();
                repo_mode.commit_files_filter = crate::state::RepoSubviewFilterState::default();
                repo_mode.diff_scroll = 0;
                close_commit_box(repo_mode);
                sync_repo_subview_selection(repo_mode, crate::state::RepoSubview::Commits);
            }
            state.focused_pane = PaneId::RepoDetail;
            effects.push(load_repo_detail_effect(state, repo_id));
            effects.push(Effect::ScheduleRender);
        }
        Action::CopySelectedReflogCommitHash => {
            let Some((repo_id, value)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_reflog_entry(repo_mode).and_then(|(_, entry)| {
                    let value = if entry.short_oid.is_empty() {
                        entry.oid.clone()
                    } else {
                        entry.short_oid.clone()
                    };
                    (!value.is_empty()).then_some((repo_mode.current_repo_id.clone(), value))
                })
            }) else {
                push_warning(
                    state,
                    "Select a reflog entry that still points to a commit before copying it.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };
            let command = clipboard_shell_command(std::ffi::OsStr::new(&value), &state.os);
            let job = shell_job(repo_id, command);
            enqueue_shell_job(state, &job, &format!("Copy {value}"));
            effects.push(Effect::RunShellCommand(job));
        }
        Action::OpenSelectedReflogInBrowser => {
            let Some((repo_id, url, label)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                let detail = repo_mode.detail.as_ref()?;
                let (_, entry) = selected_reflog_entry(repo_mode)?;
                let url = selected_commit_browser_url(state, detail, &entry.oid)?;
                Some((
                    repo_mode.current_repo_id.clone(),
                    url,
                    reflog_commit_label(entry),
                ))
            }) else {
                push_warning(
                    state,
                    "No browser-compatible remote URL found for the selected reflog entry.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };
            let command = open_in_default_app_command(
                std::ffi::OsStr::new(&url),
                &state.os,
                OsCommandTemplateKind::OpenLink,
            );
            let job = shell_job(repo_id, command);
            enqueue_shell_job(state, &job, &format!("Open {label} in browser"));
            effects.push(Effect::RunShellCommand(job));
        }
        Action::OpenReflogResetOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_reflog_entry(repo_mode)
                    .filter(|(_, entry)| !entry.selector.is_empty())
                    .map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "Select a reflog entry before opening reset options.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::ReflogResetOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenSelectedCommitFiles => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                match repo_mode.commit_subview_mode {
                    crate::state::CommitSubviewMode::History => {
                        if selected_commit_item(repo_mode).is_some() {
                            repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::Files;
                            repo_mode.commit_files_mode = CommitFilesMode::List;
                            repo_mode.commit_files_filter.focused = false;
                            sync_commit_file_selection(repo_mode);
                            state.focused_pane = PaneId::RepoDetail;
                            effects.push(Effect::ScheduleRender);
                        } else {
                            push_warning(state, "Select a commit before opening changed files.");
                            effects.push(Effect::ScheduleRender);
                        }
                    }
                    crate::state::CommitSubviewMode::Files => {
                        let Some((selected_path, effect)) =
                            load_selected_commit_file_diff_effect(repo_mode)
                        else {
                            push_warning(
                                state,
                                "Select a changed file before opening its commit diff.",
                            );
                            effects.push(Effect::ScheduleRender);
                            return;
                        };

                        repo_mode.commit_files_mode = CommitFilesMode::Diff;
                        repo_mode.commit_files_filter.focused = false;
                        repo_mode.diff_line_cursor = None;
                        repo_mode.diff_line_anchor = None;
                        repo_mode.diff_scroll = 0;
                        if let Some(detail) = repo_mode.detail.as_mut() {
                            detail.diff = crate::state::DiffModel {
                                selected_path: Some(selected_path),
                                presentation: DiffPresentation::Comparison,
                                ..crate::state::DiffModel::default()
                            };
                        }
                        state.focused_pane = PaneId::RepoDetail;
                        effects.push(effect);
                        effects.push(Effect::ScheduleRender);
                    }
                }
            }
        }
        Action::CloseSelectedCommitFiles => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if repo_mode.commit_subview_mode == crate::state::CommitSubviewMode::Files {
                    match repo_mode.commit_files_mode {
                        CommitFilesMode::Diff => {
                            repo_mode.commit_files_mode = CommitFilesMode::List;
                        }
                        CommitFilesMode::List => {
                            repo_mode.commit_subview_mode =
                                crate::state::CommitSubviewMode::History;
                            repo_mode.commit_files_mode = CommitFilesMode::List;
                        }
                    }
                    repo_mode.commit_files_filter.focused = false;
                    repo_mode.diff_line_cursor = None;
                    repo_mode.diff_line_anchor = None;
                    repo_mode.diff_scroll = 0;
                    state.focused_pane = PaneId::RepoDetail;
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenSelectedStashFiles => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if selected_stash_item(repo_mode).is_some() {
                    repo_mode.stash_subview_mode = crate::state::StashSubviewMode::Files;
                    repo_mode.stash_filter.focused = false;
                    sync_stash_file_selection(repo_mode);
                    state.focused_pane = PaneId::RepoDetail;
                    effects.push(Effect::ScheduleRender);
                } else {
                    push_warning(state, "Select a stash before opening changed files.");
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CloseSelectedStashFiles => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if repo_mode.stash_subview_mode == crate::state::StashSubviewMode::Files {
                    repo_mode.stash_subview_mode = crate::state::StashSubviewMode::List;
                    state.focused_pane = PaneId::RepoDetail;
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CheckoutSelectedCommit => match pending_checkoutable_history_target(state) {
            Ok(Some((repo_id, commit, summary))) => {
                let job = git_job(repo_id, GitCommand::CheckoutCommit { commit });
                enqueue_git_job(state, &job, &format!("Checkout commit {summary}"));
                effects.push(Effect::RunGitCommand(job));
            }
            Ok(None) => {
                push_warning(state, "Select a commit before checking it out.");
                effects.push(Effect::ScheduleRender);
            }
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CheckoutSelectedCommitFile => match selected_commit_file_checkout_target(state) {
            Ok(Some((repo_id, commit, path))) => {
                let summary = format!("Checkout {} from {}", path.display(), commit);
                let job = git_job(repo_id, GitCommand::CheckoutCommitFile { commit, path });
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
            Ok(None) => {
                push_warning(state, "Select a changed file before checking it out.");
                effects.push(Effect::ScheduleRender);
            }
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CreateBranchFromSelectedCommit => {
            match pending_checkoutable_history_target(state) {
                Ok(Some((repo_id, commit, summary))) => {
                    open_input_prompt(
                        state,
                        repo_id,
                        InputPromptOperation::CreateBranchFromCommit { commit, summary },
                    );
                    effects.push(Effect::ScheduleRender);
                }
                Ok(None) => {
                    push_warning(state, "Select a commit before creating a branch.");
                    effects.push(Effect::ScheduleRender);
                }
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CreateTagFromSelectedCommit => match pending_checkoutable_history_target(state) {
            Ok(Some((repo_id, commit, summary))) => {
                open_input_prompt(
                    state,
                    repo_id,
                    InputPromptOperation::CreateTagFromCommit { commit, summary },
                );
                effects.push(Effect::ScheduleRender);
            }
            Ok(None) => {
                push_warning(state, "Select a commit before creating a tag.");
                effects.push(Effect::ScheduleRender);
            }
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::OpenBisectOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if bisect_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Bisect options are only available from commit history when a commit is selected.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::BisectOptions);
                }
                effects.push(Effect::ScheduleRender);
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
        Action::OpenCommitFixupOptions => {
            match pending_history_commit_operation(state, |_, _, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before opening fixup options.".to_string());
                }
                Ok(())
            }) {
                Ok(Some((repo_id, ()))) => {
                    open_menu(state, repo_id, MenuOperation::CommitFixupOptions)
                }
                Ok(None) => push_warning(state, "Select a commit before opening fixup options."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenCommitCopyOptions => {
            match pending_history_commit_operation(state, |_, _, _| Ok(())) {
                Ok(Some((repo_id, ()))) => {
                    open_menu(state, repo_id, MenuOperation::CommitCopyOptions)
                }
                Ok(None) => push_warning(state, "Select a commit before opening copy options."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenCommitAmendAttributeOptions => {
            match pending_history_commit_operation(state, |_, _, _| Ok(())) {
                Ok(Some((repo_id, ()))) => {
                    open_menu(state, repo_id, MenuOperation::CommitAmendAttributeOptions)
                }
                Ok(None) => push_warning(
                    state,
                    "Select a commit before opening amend attribute options.",
                ),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::CopySelectedCommitForCherryPick => {
            let copied_commit =
                state
                    .repo_mode
                    .as_ref()
                    .and_then(selected_commit_item)
                    .map(|commit| crate::state::CopiedCommit {
                        oid: commit.oid.clone(),
                        short_oid: commit.short_oid.clone(),
                        summary: commit.summary.clone(),
                    });
            let Some(copied_commit) = copied_commit else {
                push_warning(state, "Select a commit before copying it for cherry-pick.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.copied_commit = Some(copied_commit);
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::CherryPickCopiedCommit => {
            let Some((repo_id, operation)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                repo_mode.copied_commit.as_ref().map(|commit| {
                    (
                        repo_mode.current_repo_id.clone(),
                        ConfirmableOperation::CherryPickCommit {
                            commit: commit.oid.clone(),
                            summary: format!("{} {}", commit.short_oid, commit.summary),
                        },
                    )
                })
            }) else {
                push_warning(
                    state,
                    "Copy a commit for cherry-pick before pasting it onto the current branch.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_confirmation_modal(state, repo_id, operation);
            effects.push(Effect::ScheduleRender);
        }
        Action::ClearCopiedCommitSelection => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.copied_commit = None;
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::StartBisectBad => {
            match pending_history_commit_operation(state, |detail, commit, _| {
                if detail.bisect_state.is_some() {
                    return Err("A bisect is already in progress.".to_string());
                }
                Ok((
                    GitCommand::StartBisect {
                        commit: commit.oid.clone(),
                        term: "bad".to_string(),
                    },
                    format!(
                        "Start bisect by marking {} {} as bad",
                        commit.short_oid, commit.summary
                    ),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before starting bisect."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::StartBisectGood => {
            match pending_history_commit_operation(state, |detail, commit, _| {
                if detail.bisect_state.is_some() {
                    return Err("A bisect is already in progress.".to_string());
                }
                Ok((
                    GitCommand::StartBisect {
                        commit: commit.oid.clone(),
                        term: "good".to_string(),
                    },
                    format!(
                        "Start bisect by marking {} {} as good",
                        commit.short_oid, commit.summary
                    ),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before starting bisect."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::MarkBisectBad => {
            match pending_bisect_target(state, |detail, commit, summary| {
                let term = detail
                    .bisect_state
                    .as_ref()
                    .map(|state| state.bad_term.clone())
                    .unwrap_or_else(|| "bad".to_string());
                Ok((
                    GitCommand::MarkBisect { commit, term },
                    format!("Mark {summary} as bad for bisect"),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before marking bisect state."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::MarkBisectGood => {
            match pending_bisect_target(state, |detail, commit, summary| {
                let term = detail
                    .bisect_state
                    .as_ref()
                    .map(|state| state.good_term.clone())
                    .unwrap_or_else(|| "good".to_string());
                Ok((
                    GitCommand::MarkBisect { commit, term },
                    format!("Mark {summary} as good for bisect"),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before marking bisect state."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SkipBisect => {
            match pending_bisect_target(state, |_, commit, summary| {
                Ok((
                    GitCommand::SkipBisect { commit },
                    format!("Skip {summary} during bisect"),
                ))
            }) {
                Ok(Some((repo_id, (command, summary)))) => {
                    let job = git_job(repo_id, command);
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before skipping bisect state."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::ResetBisect => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                repo_mode
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.bisect_state.as_ref())
                    .map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "No bisect is currently in progress.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            let job = git_job(repo_id, GitCommand::ResetBisect);
            enqueue_git_job(state, &job, "Reset active bisect");
            effects.push(Effect::RunGitCommand(job));
            effects.push(Effect::ScheduleRender);
        }
        Action::CreateFixupCommit => {
            match pending_history_commit_operation(state, |detail, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before creating a fixup.".to_string());
                }
                if staged_file_count(detail) == 0 {
                    return Err("Stage changes before creating a fixup commit.".to_string());
                }
                Ok((
                    commit.oid.clone(),
                    format!("{} {}", commit.short_oid, commit.summary),
                ))
            }) {
                Ok(Some((repo_id, (commit, summary)))) => {
                    let job = git_job(
                        repo_id,
                        GitCommand::CreateFixupCommit {
                            commit: commit.clone(),
                        },
                    );
                    enqueue_git_job(state, &job, &format!("Create fixup commit for {summary}"));
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => push_warning(state, "Select a commit before creating a fixup."),
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
        Action::SetFixupMessageForSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err(
                        "Select an older commit before setting the fixup message.".to_string()
                    );
                }
                Ok(ConfirmableOperation::SetFixupMessageForCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => {
                    push_warning(state, "Select a commit before setting the fixup message.")
                }
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::ApplyFixupCommits => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before applying fixups.".to_string());
                }
                Ok(ConfirmableOperation::ApplyFixupCommits {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before applying fixups."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SquashSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before starting squash.".to_string());
                }
                Ok(ConfirmableOperation::SquashCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before starting squash."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::DropSelectedCommit => {
            match pending_history_commit_operation(state, |_, commit, selected_index| {
                if selected_index == 0 {
                    return Err("Select an older commit before dropping it.".to_string());
                }
                Ok(ConfirmableOperation::DropCommit {
                    commit: commit.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before dropping it."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::MoveSelectedCommitUp => {
            match pending_history_commit_operation(state, |detail, commit, selected_index| {
                if selected_index == 0 {
                    return Err(
                        "Select a commit below another commit before moving it up.".to_string()
                    );
                }
                let adjacent = detail.commits.get(selected_index - 1).ok_or_else(|| {
                    "Select a commit below another commit before moving it up.".to_string()
                })?;
                Ok(ConfirmableOperation::MoveCommitUp {
                    commit: commit.oid.clone(),
                    adjacent_commit: adjacent.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                    adjacent_summary: format!("{} {}", adjacent.short_oid, adjacent.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before moving it up."),
                Err(message) => push_warning(state, message),
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::MoveSelectedCommitDown => {
            match pending_history_commit_operation(state, |detail, commit, selected_index| {
                let adjacent = detail.commits.get(selected_index + 1).ok_or_else(|| {
                    "Select a commit above another commit before moving it down.".to_string()
                })?;
                Ok(ConfirmableOperation::MoveCommitDown {
                    commit: commit.oid.clone(),
                    adjacent_commit: adjacent.oid.clone(),
                    summary: format!("{} {}", commit.short_oid, commit.summary),
                    adjacent_summary: format!("{} {}", adjacent.short_oid, adjacent.summary),
                })
            }) {
                Ok(Some((repo_id, operation))) => {
                    open_confirmation_modal(state, repo_id, operation)
                }
                Ok(None) => push_warning(state, "Select a commit before moving it down."),
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
            match pending_checkoutable_history_target(state) {
                Ok(Some((repo_id, commit, summary))) => open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::CherryPickCommit { commit, summary },
                ),
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
            match open_reset_confirmation(state, ResetMode::Soft) {
                Ok(true) => effects.push(Effect::ScheduleRender),
                Ok(false) => {
                    push_warning(state, "Select a commit before resetting HEAD.");
                    effects.push(Effect::ScheduleRender);
                }
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::MixedResetToSelectedCommit => {
            match open_reset_confirmation(state, ResetMode::Mixed) {
                Ok(true) => effects.push(Effect::ScheduleRender),
                Ok(false) => {
                    push_warning(state, "Select a commit before resetting HEAD.");
                    effects.push(Effect::ScheduleRender);
                }
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::HardResetToSelectedCommit => {
            match open_reset_confirmation(state, ResetMode::Hard) {
                Ok(true) => effects.push(Effect::ScheduleRender),
                Ok(false) => {
                    push_warning(state, "Select a commit before resetting HEAD.");
                    effects.push(Effect::ScheduleRender);
                }
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SoftResetToSelectedTag => {
            if open_tag_reset_confirmation(state, ResetMode::Soft) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a tag before resetting HEAD.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::MixedResetToSelectedTag => {
            if open_tag_reset_confirmation(state, ResetMode::Mixed) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a tag before resetting HEAD.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::HardResetToSelectedTag => {
            if open_tag_reset_confirmation(state, ResetMode::Hard) {
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a tag before resetting HEAD.");
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
        Action::SelectNextStashFile => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_stash_file_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousStashFile => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_stash_file_selection(repo_mode, -1) {
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
        Action::SelectNextSubmodule => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_submodule_selection(repo_mode, 1) {
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SelectPreviousSubmodule => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if step_submodule_selection(repo_mode, -1) {
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
        Action::SelectRepoDetailItem { index } => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if select_repo_detail_item_at(repo_mode, index) {
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
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if matches!(pane, PaneId::RepoUnstaged | PaneId::RepoStaged) {
                    repo_mode.main_focus = pane;
                }
                if pane != PaneId::RepoDetail {
                    clear_repo_subview_filter_focus(repo_mode);
                }
            }
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
        Action::ShowWarning { message } => {
            push_warning(state, message);
            effects.push(Effect::ScheduleRender);
        }
        Action::CloseTopModal => {
            let mut modal_return_focus = None;
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
                modal_return_focus = state
                    .pending_input_prompt
                    .take()
                    .map(|prompt| prompt.return_focus);
            }
            if state
                .modal_stack
                .last()
                .is_some_and(|modal| matches!(modal.kind, crate::state::ModalKind::Menu))
            {
                modal_return_focus = state.pending_menu.take().map(|menu| menu.return_focus);
            }
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = modal_return_focus.unwrap_or(match state.mode {
                    AppMode::Workspace => PaneId::WorkspaceList,
                    AppMode::Repository => PaneId::RepoUnstaged,
                });
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::SelectNextMenuItem => {
            if step_menu_selection(state, 1) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SelectPreviousMenuItem => {
            if step_menu_selection(state, -1) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::SubmitMenuSelection => {
            if submit_menu_selection(state, effects) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ConfirmPendingOperation => {
            if let Some(job) = confirm_pending_operation(state) {
                effects.push(Effect::RunGitCommand(job));
            } else {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenStashOptions => {
            if let Some((repo_id, has_local_changes)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    repo_mode.detail.as_ref().map(|detail| {
                        (repo_mode.current_repo_id.clone(), has_local_changes(detail))
                    })
                })
            {
                if has_local_changes {
                    open_menu(state, repo_id, MenuOperation::StashOptions);
                } else {
                    push_warning(state, "No local changes are available to stash.");
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenFilterOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if filter_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Filter options are only available from filterable detail panels.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::FilterOptions);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenDiffOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if diff_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Diff options are only available from status, branches, commits, or an active comparison.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::DiffOptions);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenCommitLogOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if commit_log_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Commit log options are only available from commit history.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::CommitLogOptions);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::ToggleWhitespaceInDiff => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.ignore_whitespace_in_diff = !repo_mode.ignore_whitespace_in_diff;
            }
            if let Some(effect) = active_diff_reload_effect(state) {
                effects.push(effect);
            }
            effects.push(Effect::ScheduleRender);
        }
        Action::IncreaseDiffContext => {
            let changed = state.repo_mode.as_mut().is_some_and(|repo_mode| {
                let next = repo_mode.diff_context_lines.saturating_add(1);
                if next == repo_mode.diff_context_lines {
                    return false;
                }
                repo_mode.diff_context_lines = next;
                true
            });
            if changed {
                if let Some(effect) = active_diff_reload_effect(state) {
                    effects.push(effect);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::DecreaseDiffContext => {
            let changed = state.repo_mode.as_mut().is_some_and(|repo_mode| {
                let next = repo_mode.diff_context_lines.saturating_sub(1);
                if next == repo_mode.diff_context_lines {
                    return false;
                }
                repo_mode.diff_context_lines = next;
                true
            });
            if changed {
                if let Some(effect) = active_diff_reload_effect(state) {
                    effects.push(effect);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::IncreaseRenameSimilarityThreshold => {
            let changed = state.repo_mode.as_mut().is_some_and(|repo_mode| {
                let next = repo_mode
                    .rename_similarity_threshold
                    .saturating_add(RENAME_SIMILARITY_THRESHOLD_STEP)
                    .min(MAX_RENAME_SIMILARITY_THRESHOLD);
                if next == repo_mode.rename_similarity_threshold {
                    return false;
                }
                repo_mode.rename_similarity_threshold = next;
                true
            });
            if changed {
                if let Some(effect) = active_diff_reload_effect(state) {
                    effects.push(effect);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::DecreaseRenameSimilarityThreshold => {
            let changed = state.repo_mode.as_mut().is_some_and(|repo_mode| {
                let next = repo_mode
                    .rename_similarity_threshold
                    .saturating_sub(RENAME_SIMILARITY_THRESHOLD_STEP)
                    .max(MIN_RENAME_SIMILARITY_THRESHOLD);
                if next == repo_mode.rename_similarity_threshold {
                    return false;
                }
                repo_mode.rename_similarity_threshold = next;
                true
            });
            if changed {
                if let Some(effect) = active_diff_reload_effect(state) {
                    effects.push(effect);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenMergeRebaseOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if merge_rebase_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Merge/rebase options are only available from commit history, branch lists, or an active rebase.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::MergeRebaseOptions);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenPatchOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if patch_menu_entries(state).is_empty() {
                    push_warning(
                        state,
                        "Patch options are only available from staged or unstaged status diffs.",
                    );
                } else {
                    open_menu(state, repo_id, MenuOperation::PatchOptions);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenRecentRepos => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if recent_repo_menu_repo_ids(state).is_empty() {
                    push_warning(state, "No recent repositories are available to reopen.");
                } else {
                    open_menu(state, repo_id, MenuOperation::RecentRepos);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenCommandLog => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if state.status_messages.is_empty() {
                    push_warning(state, "No command log entries are available yet.");
                } else {
                    open_menu(state, repo_id, MenuOperation::CommandLog);
                }
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::NextScreenMode => {
            state.settings.screen_mode = state.settings.screen_mode.next();
            state.status_messages.push_back(StatusMessage::info(
                0,
                format!("Screen mode: {}", state.settings.screen_mode.label()),
            ));
            effects.push(Effect::ScheduleRender);
        }
        Action::PreviousScreenMode => {
            state.settings.screen_mode = state.settings.screen_mode.previous();
            state.status_messages.push_back(StatusMessage::info(
                0,
                format!("Screen mode: {}", state.settings.screen_mode.label()),
            ));
            effects.push(Effect::ScheduleRender);
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
            if let Some(submission) = submit_input_prompt(state) {
                match submission {
                    PromptSubmission::Git(job) => effects.push(Effect::RunGitCommand(job)),
                    PromptSubmission::Shell(job) => effects.push(Effect::RunShellCommand(job)),
                }
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
        Action::OpenConfigFileInDefaultApp => match selected_config_target(state) {
            Ok(Some((repo_id, _, target))) => {
                let command = open_in_default_app_command(
                    target.as_os_str(),
                    &state.os,
                    OsCommandTemplateKind::OpenFile,
                );
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Open config {}", target.display()));
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::OpenConfigFileInEditor => match selected_config_target(state) {
            Ok(Some((_, cwd, target))) => effects.push(Effect::OpenEditor { cwd, target }),
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CheckForUpdates => match selected_update_check_target(state) {
            Ok(Some((repo_id, url))) => {
                let command = open_in_default_app_command(
                    std::ffi::OsStr::new(&url),
                    &state.os,
                    OsCommandTemplateKind::OpenLink,
                );
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, "Open release page");
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
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
        Action::RefreshSelectedRepoDeep => {
            if let Some(repo_id) = state.workspace.selected_repo_id.clone() {
                effects.push(Effect::RefreshRepoSummary {
                    repo_id: repo_id.clone(),
                });
                effects.push(Effect::StartRepoScan);
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
        Action::UnstageSelection => {
            if let Some(repo_mode) = &state.repo_mode {
                let job = git_job(
                    repo_mode.current_repo_id.clone(),
                    GitCommand::UnstageSelection,
                );
                enqueue_git_job(state, &job, "Unstage selection");
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
        Action::ToggleStatusTree => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.status_tree_enabled = !repo_mode.status_tree_enabled;
                sync_status_selection(repo_mode);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CollapseStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if update_status_directory_collapse(repo_mode, state.focused_pane, true) {
                    sync_status_selection(repo_mode);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::ExpandStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if update_status_directory_collapse(repo_mode, state.focused_pane, false) {
                    sync_status_selection(repo_mode);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CycleStatusFilterMode => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.status_filter_mode = repo_mode.status_filter_mode.cycle_next();
                sync_status_selection(repo_mode);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenSelectedStatusEntry => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if let Some(entry) = selected_status_entry(repo_mode, state.focused_pane) {
                    if entry.is_directory() {
                        let collapsed = entry.collapsed();
                        update_status_directory_for_path(repo_mode, entry.path, !collapsed);
                        sync_status_selection(repo_mode);
                    } else {
                        state.focused_pane = PaneId::RepoDetail;
                    }
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenIgnoreOptions => {
            let selected_path = state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| selected_status_display_path(repo_mode, state.focused_pane));
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if selected_path.is_some() {
                    open_menu(state, repo_id, MenuOperation::IgnoreOptions);
                    effects.push(Effect::ScheduleRender);
                } else {
                    push_warning(state, "Select a file or directory before ignoring it.");
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::IgnoreSelectedStatusPath => match selected_status_ignore_target(state, false) {
            Ok(Some((repo_id, _path, command, summary))) => {
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &summary);
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::ExcludeSelectedStatusPath => match selected_status_ignore_target(state, true) {
            Ok(Some((repo_id, _path, command, summary))) => {
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &summary);
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CopySelectedCommitHash => {
            let Some((repo_id, clipboard_value)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_commit_item(repo_mode).map(|commit| {
                    let clipboard_value = if commit.short_oid.is_empty() {
                        commit.oid.clone()
                    } else {
                        commit.short_oid.clone()
                    };
                    (repo_mode.current_repo_id.clone(), clipboard_value)
                })
            }) else {
                push_warning(state, "Select a commit before copying its hash.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            let command =
                clipboard_shell_command(std::ffi::OsStr::new(&clipboard_value), &state.os);
            let job = shell_job(repo_id, command);
            enqueue_shell_job(state, &job, &format!("Copy {clipboard_value}"));
            effects.push(Effect::RunShellCommand(job));
        }
        Action::OpenSelectedCommitInBrowser => match selected_commit_browser_target(state) {
            Ok(Some((repo_id, target))) => {
                let command = open_in_default_app_command(
                    std::ffi::OsStr::new(&target),
                    &state.os,
                    OsCommandTemplateKind::OpenLink,
                );
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Open {target}"));
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CopySelectedStatusPath => match selected_repo_shell_target(state, false) {
            Ok(Some((repo_id, path, is_directory, _))) => {
                let display_path = status_clipboard_path(&path, is_directory);
                let command = clipboard_shell_command(display_path.as_os_str(), &state.os);
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Copy {}", display_path.display()));
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::OpenSelectedStatusPathInDefaultApp => {
            match selected_repo_shell_target(state, true) {
                Ok(Some((repo_id, path, _, _))) => {
                    let command = open_in_default_app_command(
                        path.as_os_str(),
                        &state.os,
                        OsCommandTemplateKind::OpenFile,
                    );
                    let job = shell_job(repo_id, command);
                    enqueue_shell_job(state, &job, &format!("Open {}", path.display()));
                    effects.push(Effect::RunShellCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenSelectedStatusPathInExternalDiffTool => {
            match selected_repo_shell_target(state, false) {
                Ok(Some((repo_id, _path, is_directory, relative_path))) => {
                    if is_directory {
                        push_warning(state, "Select a file before opening the external difftool.");
                        effects.push(Effect::ScheduleRender);
                    } else {
                        let command = external_difftool_command(&relative_path, state.focused_pane);
                        let job = shell_job(repo_id, command);
                        enqueue_shell_job(
                            state,
                            &job,
                            &format!("Open difftool for {}", relative_path.display()),
                        );
                        effects.push(Effect::RunShellCommand(job));
                    }
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenStatusResetOptions => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                if repo_tracking_branch(state, &repo_id).is_some() {
                    open_menu(state, repo_id, MenuOperation::StatusResetOptions);
                    effects.push(Effect::ScheduleRender);
                } else {
                    push_warning(state, "Current branch has no upstream to reset against.");
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::SoftResetToUpstream => {
            if open_upstream_reset_confirmation(state, ResetMode::Soft) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::MixedResetToUpstream => {
            if open_upstream_reset_confirmation(state, ResetMode::Mixed) {
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::HardResetToUpstream => {
            if open_upstream_reset_confirmation(state, ResetMode::Hard) {
                effects.push(Effect::ScheduleRender);
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
        Action::CheckoutSelectedRemoteBranch => {
            if let Some((repo_id, remote_branch_ref, local_branch_name)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_remote_branch_item(repo_mode).map(|branch| {
                        (
                            repo_mode.current_repo_id.clone(),
                            branch.name.clone(),
                            branch.branch_name.clone(),
                        )
                    })
                })
            {
                let summary = format!("Checkout remote branch {remote_branch_ref}");
                let job = git_job(
                    repo_id,
                    GitCommand::CheckoutRemoteBranch {
                        remote_branch_ref,
                        local_branch_name,
                    },
                );
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            } else {
                push_warning(state, "Select a remote branch before checking it out.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CheckoutSelectedTag => {
            if let Some((repo_id, tag_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode)
                    .map(|tag| (repo_mode.current_repo_id.clone(), tag.name.clone()))
            }) {
                let summary = format!("Checkout tag {tag_name}");
                let job = git_job(repo_id, GitCommand::CheckoutTag { tag_name });
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            } else {
                push_warning(state, "Select a tag before checking it out.");
                effects.push(Effect::ScheduleRender);
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
        Action::OpenBranchGitFlowOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode).map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "Select a branch before opening git-flow options.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::BranchGitFlowOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenBranchPullRequestOptions => match selected_branch_pull_request_target(state) {
            Ok(Some((repo_id, _, _))) => {
                open_menu(state, repo_id, MenuOperation::BranchPullRequestOptions);
                effects.push(Effect::ScheduleRender);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::OpenBranchResetOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode).map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "Select a branch before opening reset options.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::BranchResetOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenBranchSortOptions => {
            let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            else {
                return;
            };
            open_menu(state, repo_id, MenuOperation::BranchSortOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenBranchUpstreamOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode).map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "Select a branch before opening upstream options.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            open_menu(state, repo_id, MenuOperation::BranchUpstreamOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::CopySelectedBranchName => {
            let Some((repo_id, branch_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) else {
                push_warning(state, "Select a branch before copying its name.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            let command = clipboard_shell_command(std::ffi::OsStr::new(&branch_name), &state.os);
            let job = shell_job(repo_id, command);
            enqueue_shell_job(state, &job, &format!("Copy {branch_name}"));
            effects.push(Effect::RunShellCommand(job));
        }
        Action::ForceCheckoutSelectedBranch => match selected_non_head_branch_ref(state) {
            Ok(Some((repo_id, target_ref))) => {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::ForceCheckoutRef {
                        source_label: target_ref.clone(),
                        target_ref,
                    },
                );
                effects.push(Effect::ScheduleRender);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::DeleteSelectedBranch => {
            if let Some((repo_id, branch_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) {
                effects.push(Effect::CheckBranchMerged {
                    repo_id,
                    branch_name,
                });
            }
        }
        Action::UnsetSelectedBranchUpstream => match selected_branch_upstream_target(state) {
            Ok(Some((repo_id, branch_name))) => {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::UnsetBranchUpstream { branch_name },
                );
                effects.push(Effect::ScheduleRender);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::FastForwardSelectedBranchFromUpstream => {
            match selected_branch_fast_forward_target(state) {
                Ok(Some((repo_id, branch_name, upstream_ref))) => {
                    open_confirmation_modal(
                        state,
                        repo_id,
                        ConfirmableOperation::FastForwardCurrentBranchFromUpstream {
                            branch_name,
                            upstream_ref,
                        },
                    );
                    effects.push(Effect::ScheduleRender);
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::MergeSelectedBranchIntoCurrent => match selected_non_head_branch_ref(state) {
            Ok(Some((repo_id, target_ref))) => {
                open_merge_confirmation(state, repo_id, target_ref, MergeVariant::Regular, effects);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::MergeSelectedRefIntoCurrent { variant } => match selected_merge_target(state) {
            Ok(Some((repo_id, target_ref))) => {
                open_merge_confirmation(state, repo_id, target_ref, variant, effects);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::RebaseCurrentBranchOntoSelectedBranch => {
            match selected_non_head_branch_ref(state) {
                Ok(Some((repo_id, target_ref))) => {
                    open_confirmation_modal(
                        state,
                        repo_id,
                        ConfirmableOperation::RebaseCurrentBranchOntoRef {
                            source_label: target_ref.clone(),
                            target_ref,
                        },
                    );
                    effects.push(Effect::ScheduleRender);
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenSelectedBranchPullRequest => match selected_branch_pull_request_target(state) {
            Ok(Some((repo_id, url, label))) => {
                let command = open_in_default_app_command(
                    std::ffi::OsStr::new(&url),
                    &state.os,
                    OsCommandTemplateKind::OpenLink,
                );
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Open pull request for {label}"));
                effects.push(Effect::RunShellCommand(job));
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::CopySelectedBranchPullRequestUrl => {
            match selected_branch_pull_request_target(state) {
                Ok(Some((repo_id, url, label))) => {
                    let command = clipboard_shell_command(std::ffi::OsStr::new(&url), &state.os);
                    let job = shell_job(repo_id, command);
                    enqueue_shell_job(state, &job, &format!("Copy pull request URL for {label}"));
                    effects.push(Effect::RunShellCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CreateTagFromSelectedBranch => {
            let Some((repo_id, target_ref, source_label)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_branch_item(repo_mode).map(|branch| {
                        (
                            repo_mode.current_repo_id.clone(),
                            branch.name.clone(),
                            branch.name.clone(),
                        )
                    })
                })
            else {
                push_warning(state, "Select a branch before creating a tag from it.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_input_prompt(
                state,
                repo_id,
                InputPromptOperation::CreateTagFromRef {
                    target_ref,
                    source_label,
                },
            );
            effects.push(Effect::ScheduleRender);
        }
        Action::SetBranchSortMode(mode) => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.branch_sort_mode = mode;
                sync_branch_selection(repo_mode);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::RunGitFlowFinish => {
            if let Some((repo_id, branch_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) {
                let job = git_job(
                    repo_id,
                    GitCommand::FinishGitFlow {
                        branch_name: branch_name.clone(),
                    },
                );
                enqueue_git_job(
                    state,
                    &job,
                    &format!("Finish git-flow branch {branch_name}"),
                );
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::DeleteSelectedRemote => {
            if let Some((repo_id, remote_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_item(repo_mode)
                    .map(|remote| (repo_mode.current_repo_id.clone(), remote.name.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::RemoveRemote { remote_name },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a remote before removing it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::DeleteSelectedRemoteBranch => {
            if let Some((repo_id, remote_name, branch_name)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_remote_branch_item(repo_mode).map(|branch| {
                        (
                            repo_mode.current_repo_id.clone(),
                            branch.remote_name.clone(),
                            branch.branch_name.clone(),
                        )
                    })
                })
            {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::DeleteRemoteBranch {
                        remote_name,
                        branch_name,
                    },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a remote branch before deleting it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CopySelectedRemoteBranchName => {
            let Some((repo_id, branch_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_branch_item(repo_mode)
                    .map(|branch| (repo_mode.current_repo_id.clone(), branch.name.clone()))
            }) else {
                push_warning(state, "Select a remote branch before copying its name.");
                effects.push(Effect::ScheduleRender);
                return;
            };

            let command = clipboard_shell_command(std::ffi::OsStr::new(&branch_name), &state.os);
            let job = shell_job(repo_id, command);
            enqueue_shell_job(state, &job, &format!("Copy {branch_name}"));
            effects.push(Effect::RunShellCommand(job));
        }
        Action::OpenRemoteBranchPullRequestOptions => {
            match selected_remote_branch_pull_request_target(state) {
                Ok(Some((repo_id, _, _))) => {
                    open_menu(
                        state,
                        repo_id,
                        MenuOperation::RemoteBranchPullRequestOptions,
                    );
                    effects.push(Effect::ScheduleRender);
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenRemoteBranchResetOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_branch_item(repo_mode).map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(
                    state,
                    "Select a remote branch before opening reset options.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::RemoteBranchResetOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::OpenRemoteBranchSortOptions => {
            let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            else {
                return;
            };
            open_menu(state, repo_id, MenuOperation::RemoteBranchSortOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::SetCurrentBranchUpstreamToSelectedRemoteBranch => {
            match selected_remote_branch_upstream_target(state) {
                Ok(Some((repo_id, branch_name, upstream_ref))) => {
                    let summary = format!("Set upstream for {branch_name} to {upstream_ref}");
                    let job = git_job(
                        repo_id,
                        GitCommand::SetBranchUpstream {
                            branch_name,
                            upstream_ref,
                        },
                    );
                    enqueue_git_job(state, &job, &summary);
                    effects.push(Effect::RunGitCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::MergeSelectedRemoteBranchIntoCurrent => match selected_remote_branch_ref(state) {
            Ok(Some((repo_id, target_ref))) => {
                open_merge_confirmation(state, repo_id, target_ref, MergeVariant::Regular, effects);
            }
            Ok(None) => {}
            Err(message) => {
                push_warning(state, message);
                effects.push(Effect::ScheduleRender);
            }
        },
        Action::RebaseCurrentBranchOntoSelectedRemoteBranch => {
            match selected_remote_branch_ref(state) {
                Ok(Some((repo_id, target_ref))) => {
                    open_confirmation_modal(
                        state,
                        repo_id,
                        ConfirmableOperation::RebaseCurrentBranchOntoRef {
                            source_label: target_ref.clone(),
                            target_ref,
                        },
                    );
                    effects.push(Effect::ScheduleRender);
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::OpenSelectedRemoteBranchPullRequest => {
            match selected_remote_branch_pull_request_target(state) {
                Ok(Some((repo_id, url, label))) => {
                    let command = open_in_default_app_command(
                        std::ffi::OsStr::new(&url),
                        &state.os,
                        OsCommandTemplateKind::OpenLink,
                    );
                    let job = shell_job(repo_id, command);
                    enqueue_shell_job(state, &job, &format!("Open pull request for {label}"));
                    effects.push(Effect::RunShellCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CopySelectedRemoteBranchPullRequestUrl => {
            match selected_remote_branch_pull_request_target(state) {
                Ok(Some((repo_id, url, label))) => {
                    let command = clipboard_shell_command(std::ffi::OsStr::new(&url), &state.os);
                    let job = shell_job(repo_id, command);
                    enqueue_shell_job(state, &job, &format!("Copy pull request URL for {label}"));
                    effects.push(Effect::RunShellCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::CreateTagFromSelectedRemoteBranch => {
            let Some((repo_id, target_ref, source_label)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_remote_branch_item(repo_mode).map(|branch| {
                        (
                            repo_mode.current_repo_id.clone(),
                            branch.name.clone(),
                            branch.name.clone(),
                        )
                    })
                })
            else {
                push_warning(
                    state,
                    "Select a remote branch before creating a tag from it.",
                );
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_input_prompt(
                state,
                repo_id,
                InputPromptOperation::CreateTagFromRef {
                    target_ref,
                    source_label,
                },
            );
            effects.push(Effect::ScheduleRender);
        }
        Action::SetRemoteBranchSortMode(mode) => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                repo_mode.remote_branch_sort_mode = mode;
                sync_remote_branch_selection(repo_mode);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::DeleteSelectedTag => {
            if let Some((repo_id, tag_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode)
                    .map(|tag| (repo_mode.current_repo_id.clone(), tag.name.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::DeleteTag { tag_name },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a tag before deleting it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CopySelectedTagName => {
            if let Some((repo_id, tag_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode)
                    .map(|tag| (repo_mode.current_repo_id.clone(), tag.name.clone()))
            }) {
                let command = clipboard_shell_command(std::ffi::OsStr::new(&tag_name), &state.os);
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Copy tag {tag_name}"));
                effects.push(Effect::RunShellCommand(job));
            } else {
                push_warning(state, "Select a tag before copying it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenTagResetOptions => {
            let Some(repo_id) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode).map(|_| repo_mode.current_repo_id.clone())
            }) else {
                push_warning(state, "Select a tag before opening reset options.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::TagResetOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::FetchSelectedRemote => {
            if let Some((repo_id, remote_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_remote_item(repo_mode)
                    .map(|remote| (repo_mode.current_repo_id.clone(), remote.name.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::FetchRemote { remote_name },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a remote before fetching it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::PushSelectedTag => {
            if let Some((repo_id, tag_name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_tag_item(repo_mode)
                    .map(|tag| (repo_mode.current_repo_id.clone(), tag.name.clone()))
            }) {
                let remote_name = state
                    .workspace
                    .repo_summaries
                    .get(&repo_id)
                    .and_then(|summary| summary.remote_summary.remote_name.clone())
                    .unwrap_or_else(|| "origin".to_string());
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::PushTag {
                        remote_name,
                        tag_name,
                    },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(state, "Select a tag before pushing it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CreateLocalBranchFromSelectedRemoteBranch => {
            if let Some((repo_id, remote_branch_ref, suggested_name)) =
                state.repo_mode.as_ref().and_then(|repo_mode| {
                    selected_remote_branch_item(repo_mode).map(|branch| {
                        (
                            repo_mode.current_repo_id.clone(),
                            branch.name.clone(),
                            branch.branch_name.clone(),
                        )
                    })
                })
            {
                open_input_prompt(
                    state,
                    repo_id,
                    InputPromptOperation::CreateBranchFromRemote {
                        remote_branch_ref,
                        suggested_name,
                    },
                );
                effects.push(Effect::ScheduleRender);
            } else {
                push_warning(
                    state,
                    "Select a remote branch before creating a local branch from it.",
                );
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::StashAllChanges => {
            if let Some((repo_id, stashable)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                repo_mode.detail.as_ref().map(|detail| {
                    (
                        repo_mode.current_repo_id.clone(),
                        has_stashable_changes(detail),
                    )
                })
            }) {
                if stashable {
                    open_input_prompt(
                        state,
                        repo_id,
                        InputPromptOperation::CreateStash {
                            mode: StashMode::Tracked,
                        },
                    );
                } else {
                    push_warning(state, "No tracked changes are available to stash.");
                }
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
        Action::PopSelectedStash => {
            if let Some((repo_id, stash_ref)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_stash_item(repo_mode)
                    .map(|stash| (repo_mode.current_repo_id.clone(), stash.stash_ref.clone()))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::PopStash { stash_ref },
                );
                effects.push(Effect::ScheduleRender);
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
        Action::CreateSubmodule => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                open_input_prompt(state, repo_id, InputPromptOperation::CreateSubmodule);
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::EditSelectedSubmodule => {
            if let Some((repo_id, item)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_submodule_item(repo_mode)
                    .cloned()
                    .map(|item| (repo_mode.current_repo_id.clone(), item))
            }) {
                open_input_prompt(
                    state,
                    repo_id,
                    InputPromptOperation::EditSubmoduleUrl {
                        name: item.name,
                        path: item.path,
                        current_url: item.url,
                    },
                );
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::CopySelectedSubmoduleName => {
            if let Some((repo_id, name)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_submodule_item(repo_mode)
                    .map(|item| (repo_mode.current_repo_id.clone(), item.name.clone()))
            }) {
                let command = clipboard_shell_command(std::ffi::OsStr::new(&name), &state.os);
                let job = shell_job(repo_id, command);
                enqueue_shell_job(state, &job, &format!("Copy submodule {name}"));
                effects.push(Effect::RunShellCommand(job));
            } else {
                push_warning(state, "Select a submodule before copying it.");
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenSubmoduleOptions => {
            let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            else {
                push_warning(state, "Open bulk submodule options from repo mode.");
                effects.push(Effect::ScheduleRender);
                return;
            };
            open_menu(state, repo_id, MenuOperation::BulkSubmoduleOptions);
            effects.push(Effect::ScheduleRender);
        }
        Action::InitAllSubmodules => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                let summary = "Initialize all submodules".to_string();
                let job = git_job(repo_id, GitCommand::InitAllSubmodules);
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::UpdateAllSubmodules => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                let summary = "Update all submodules".to_string();
                let job = git_job(repo_id, GitCommand::UpdateAllSubmodules);
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::UpdateAllSubmodulesRecursively => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                let summary = "Update all submodules recursively".to_string();
                let job = git_job(repo_id, GitCommand::UpdateAllSubmodulesRecursively);
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::DeinitAllSubmodules => {
            if let Some(repo_id) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone())
            {
                let summary = "Deinitialize all submodules".to_string();
                let job = git_job(repo_id, GitCommand::DeinitAllSubmodules);
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::InitSelectedSubmodule => {
            if let Some((repo_id, path)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_submodule_item(repo_mode)
                    .map(|item| (repo_mode.current_repo_id.clone(), item.path.clone()))
            }) {
                let summary = format!("Initialize submodule {}", path.display());
                let job = git_job(repo_id, GitCommand::InitSubmodule { path });
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::UpdateSelectedSubmodule => {
            if let Some((repo_id, path)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_submodule_item(repo_mode)
                    .map(|item| (repo_mode.current_repo_id.clone(), item.path.clone()))
            }) {
                let summary = format!("Update submodule {}", path.display());
                let job = git_job(repo_id, GitCommand::UpdateSubmodule { path });
                enqueue_git_job(state, &job, &summary);
                effects.push(Effect::RunGitCommand(job));
            }
        }
        Action::RemoveSelectedSubmodule => {
            if let Some((repo_id, item)) = state.repo_mode.as_ref().and_then(|repo_mode| {
                selected_submodule_item(repo_mode)
                    .cloned()
                    .map(|item| (repo_mode.current_repo_id.clone(), item))
            }) {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::RemoveSubmodule {
                        name: item.name,
                        path: item.path,
                    },
                );
                effects.push(Effect::ScheduleRender);
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
        Action::ActivateRepoSubviewSelection => activate_repo_subview_selection(state, effects),
        Action::FocusRepoMainPane => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                state.focused_pane = repo_mode.main_focus;
                effects.push(Effect::ScheduleRender);
            }
        }
        Action::OpenRepoWorktreesSubview => {
            if matches!(state.mode, AppMode::Repository) && state.focused_pane == PaneId::RepoDetail
            {
                reduce_action(
                    state,
                    Action::SwitchRepoSubview(crate::state::RepoSubview::Worktrees),
                    effects,
                );
            }
        }
        Action::OpenRepoSubmodulesSubview => {
            if matches!(state.mode, AppMode::Repository) && state.focused_pane == PaneId::RepoDetail
            {
                reduce_action(
                    state,
                    Action::SwitchRepoSubview(crate::state::RepoSubview::Submodules),
                    effects,
                );
            }
        }
        Action::SelectNextRepoSubview => {
            if let Some(subview) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| adjacent_repo_subview(repo_mode.active_subview, 1))
            {
                reduce_action(state, Action::SwitchRepoSubview(subview), effects);
            }
        }
        Action::SelectPreviousRepoSubview => {
            if let Some(subview) = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| adjacent_repo_subview(repo_mode.active_subview, -1))
            {
                reduce_action(state, Action::SwitchRepoSubview(subview), effects);
            }
        }
        Action::FocusRepoSubviewFilter => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                clear_repo_subview_filter_focus(repo_mode);
                if let Some(filter) = repo_mode.subview_filter_mut(repo_mode.active_subview) {
                    filter.focused = true;
                    state.focused_pane = PaneId::RepoDetail;
                    effects.push(Effect::ScheduleRender);
                }
            }
        }
        Action::BlurRepoSubviewFilter => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                if let Some(filter) = repo_mode.subview_filter_mut(repo_mode.active_subview) {
                    if filter.focused {
                        filter.focused = false;
                        effects.push(Effect::ScheduleRender);
                    }
                }
            }
        }
        Action::CancelRepoSubviewFilter => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                let active_subview = repo_mode.active_subview;
                if let Some(filter) = repo_mode.subview_filter_mut(active_subview) {
                    let had_focus = filter.focused;
                    let had_query = !filter.query.is_empty();
                    filter.focused = false;
                    if had_query {
                        filter.query.clear();
                        sync_repo_subview_selection(repo_mode, active_subview);
                    }
                    if had_focus || had_query {
                        effects.push(Effect::ScheduleRender);
                    }
                }
            }
        }
        Action::AppendRepoSubviewFilter { text } => {
            if !text.is_empty() {
                if let Some(repo_mode) = state.repo_mode.as_mut() {
                    let active_subview = repo_mode.active_subview;
                    if let Some(filter) = repo_mode.subview_filter_mut(active_subview) {
                        filter.focused = true;
                        filter.query.push_str(&text);
                        sync_repo_subview_selection(repo_mode, active_subview);
                        effects.push(Effect::ScheduleRender);
                    }
                }
            }
        }
        Action::BackspaceRepoSubviewFilter => {
            if let Some(repo_mode) = state.repo_mode.as_mut() {
                let active_subview = repo_mode.active_subview;
                if let Some(filter) = repo_mode.subview_filter_mut(active_subview) {
                    if filter.query.pop().is_some() {
                        sync_repo_subview_selection(repo_mode, active_subview);
                        effects.push(Effect::ScheduleRender);
                    }
                }
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
                let reset_explicit_commit_history =
                    matches!(repo_mode.active_subview, crate::state::RepoSubview::Commits)
                        && (repo_mode.commit_history_ref.is_some()
                            || repo_mode.commit_history_mode != CommitHistoryMode::Linear
                            || repo_mode.pending_commit_selection_oid.is_some());
                clear_repo_subview_filter_focus(repo_mode);
                repo_mode.commit_history_ref = None;
                repo_mode.commit_history_mode = CommitHistoryMode::Linear;
                repo_mode.pending_commit_selection_oid = None;
                repo_mode.commit_subview_mode = crate::state::CommitSubviewMode::History;
                repo_mode.stash_subview_mode = crate::state::StashSubviewMode::List;
                repo_mode.active_subview = subview;
                repo_mode.diff_scroll = 0;
                if !matches!(
                    subview,
                    crate::state::RepoSubview::Status | crate::state::RepoSubview::Compare
                ) {
                    close_commit_box(repo_mode);
                }
                sync_repo_subview_selection(repo_mode, subview);
                if matches!(subview, crate::state::RepoSubview::Rebase) {
                    repo_mode.diff_scroll = 0;
                }
                if matches!(subview, crate::state::RepoSubview::Compare)
                    && repo_mode.comparison_base.is_some()
                    && repo_mode.comparison_target.is_some()
                {
                    effects.push(load_comparison_diff_effect(repo_mode));
                } else if reset_explicit_commit_history
                    || (matches!(subview, crate::state::RepoSubview::Status)
                        && repo_mode.detail.as_ref().is_some_and(|detail| {
                            detail.diff.presentation == DiffPresentation::Comparison
                        }))
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
            let mut missing_history_target = false;
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
                    sync_remote_selection(repo_mode);
                    sync_commit_selection(repo_mode);
                    sync_commit_file_selection(repo_mode);
                    sync_stash_selection(repo_mode);
                    sync_reflog_selection(repo_mode);
                    sync_worktree_selection(repo_mode);
                    sync_diff_selection(repo_mode);
                    if repo_mode.pending_commit_selection_oid.is_some()
                        && repo_mode.active_subview == crate::state::RepoSubview::Commits
                    {
                        repo_mode.pending_commit_selection_oid = None;
                        missing_history_target = true;
                    }
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
            if missing_history_target {
                push_warning(
                    state,
                    "Selected history target is not visible in the current commit view snapshot.",
                );
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
        WorkerEvent::BranchMergeCheckCompleted {
            repo_id,
            branch_name,
            merged,
        } => {
            if merged {
                let job = git_job(
                    repo_id.clone(),
                    GitCommand::DeleteBranch {
                        branch_name,
                        force: true,
                    },
                );
                enqueue_git_job(state, &job, "Delete branch");
                effects.push(Effect::RunGitCommand(job));
            } else {
                open_confirmation_modal(
                    state,
                    repo_id,
                    ConfirmableOperation::DeleteBranch {
                        branch_name,
                        force: true,
                    },
                );
                effects.push(Effect::ScheduleRender);
            }
        }
        WorkerEvent::BranchMergeCheckFailed {
            repo_id: _,
            branch_name,
            error,
        } => {
            push_warning(
                state,
                format!("Failed to determine whether {branch_name} is merged: {error}"),
            );
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

fn activate_repo_subview_selection(state: &mut AppState, effects: &mut Vec<Effect>) {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return;
    };

    match repo_mode.active_subview {
        crate::state::RepoSubview::Status => {
            let Some(presentation) = repo_mode
                .detail
                .as_ref()
                .map(|detail| detail.diff.presentation)
            else {
                return;
            };
            let next_action = match presentation {
                DiffPresentation::Unstaged => Some(Action::StageSelectedHunk),
                DiffPresentation::Staged => Some(Action::UnstageSelectedHunk),
                DiffPresentation::Comparison => None,
            };
            if let Some(action) = next_action {
                reduce_action(state, action, effects);
            }
        }
        crate::state::RepoSubview::Branches => {
            reduce_action(state, Action::OpenSelectedBranchCommits, effects);
        }
        crate::state::RepoSubview::Remotes => {
            reduce_action(state, Action::OpenSelectedRemoteBranches, effects);
        }
        crate::state::RepoSubview::RemoteBranches => {
            reduce_action(state, Action::OpenSelectedRemoteBranchCommits, effects);
        }
        crate::state::RepoSubview::Tags => {
            reduce_action(state, Action::OpenSelectedTagCommits, effects);
        }
        crate::state::RepoSubview::Commits => {
            let action = match repo_mode.commit_subview_mode {
                crate::state::CommitSubviewMode::History => Action::OpenSelectedCommitFiles,
                crate::state::CommitSubviewMode::Files => match repo_mode.commit_files_mode {
                    CommitFilesMode::List => Action::OpenSelectedCommitFiles,
                    CommitFilesMode::Diff => Action::CloseSelectedCommitFiles,
                },
            };
            reduce_action(state, action, effects);
        }
        crate::state::RepoSubview::Stash => {
            let action = match repo_mode.stash_subview_mode {
                crate::state::StashSubviewMode::List => Action::OpenSelectedStashFiles,
                crate::state::StashSubviewMode::Files => Action::CloseSelectedStashFiles,
            };
            reduce_action(state, action, effects);
        }
        crate::state::RepoSubview::Worktrees => {
            let next_repo_id = selected_worktree_item(repo_mode).map(|worktree| {
                crate::state::RepoId::new(worktree.path.to_string_lossy().into_owned())
            });
            if let Some(repo_id) = next_repo_id {
                reduce_action(state, Action::EnterRepoMode { repo_id }, effects);
            }
        }
        crate::state::RepoSubview::Submodules => {
            let parent_repo_id = repo_mode.current_repo_id.clone();
            let repo_root = repo_root_for_id(state, &parent_repo_id);
            let next_repo_id = selected_submodule_item(repo_mode).map(|submodule| {
                crate::state::RepoId::new(
                    repo_root
                        .join(&submodule.path)
                        .to_string_lossy()
                        .into_owned(),
                )
            });
            if let Some(repo_id) = next_repo_id {
                reduce_action(
                    state,
                    Action::EnterNestedRepoMode {
                        repo_id,
                        parent_repo_id,
                    },
                    effects,
                );
            }
        }
        crate::state::RepoSubview::Reflog => {
            reduce_action(state, Action::OpenSelectedReflogCommits, effects);
        }
        crate::state::RepoSubview::Compare | crate::state::RepoSubview::Rebase => {}
    }
}

fn step_status_selection(repo_mode: &mut RepoModeState, focused_pane: PaneId, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        return false;
    }

    let len = status_entries_len(repo_mode, focused_pane);
    if len == 0 {
        return false;
    }

    match focused_pane {
        PaneId::RepoUnstaged => repo_mode.status_view.select_with_step(len, step).is_some(),
        PaneId::RepoStaged => repo_mode.staged_view.select_with_step(len, step).is_some(),
        _ => false,
    }
}

fn select_status_entry_at(repo_mode: &mut RepoModeState, pane: PaneId, index: usize) -> bool {
    if repo_mode.detail.is_none() {
        return false;
    }

    let len = status_entries_len(repo_mode, pane);
    if len == 0 {
        return false;
    }

    match pane {
        PaneId::RepoUnstaged => {
            let changed = repo_mode.status_view.selected_index != Some(index);
            repo_mode.status_view.set_selected(len, index).is_some() && changed
        }
        PaneId::RepoStaged => {
            let changed = repo_mode.staged_view.selected_index != Some(index);
            repo_mode.staged_view.set_selected(len, index).is_some() && changed
        }
        _ => false,
    }
}

fn step_commit_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        return false;
    }

    let changed = match repo_mode.commit_subview_mode {
        crate::state::CommitSubviewMode::History => {
            let visible_indices = filtered_commit_indices(repo_mode);
            step_filtered_selection(
                &mut repo_mode.commits_view.selected_index,
                &visible_indices,
                step,
            )
        }
        crate::state::CommitSubviewMode::Files => {
            let visible_indices = filtered_commit_file_indices(repo_mode);
            step_filtered_selection(
                &mut repo_mode.commit_files_view.selected_index,
                &visible_indices,
                step,
            )
        }
    };
    if changed {
        repo_mode.diff_scroll = 0;
    }
    changed
}

fn select_repo_list_page(state: &mut AppState, step: isize, effects: &mut Vec<Effect>) -> bool {
    match state.focused_pane {
        PaneId::RepoUnstaged | PaneId::RepoStaged => {
            let Some(repo_mode) = state.repo_mode.as_mut() else {
                return false;
            };

            if !step_status_selection(repo_mode, state.focused_pane, step) {
                return false;
            }

            if let Some((selected_path, diff_presentation)) =
                selected_status_detail_request(repo_mode, state.focused_pane)
            {
                effects.push(Effect::LoadRepoDetail {
                    repo_id: repo_mode.current_repo_id.clone(),
                    selected_path: Some(selected_path),
                    diff_presentation,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: repo_mode.ignore_whitespace_in_diff,
                    diff_context_lines: repo_mode.diff_context_lines,
                    rename_similarity_threshold: repo_mode.rename_similarity_threshold,
                });
            }
            true
        }
        PaneId::RepoDetail => {
            let Some(repo_mode) = state.repo_mode.as_mut() else {
                return false;
            };

            match repo_mode.active_subview {
                crate::state::RepoSubview::Branches => step_branch_selection(repo_mode, step),
                crate::state::RepoSubview::Remotes => step_remote_selection(repo_mode, step),
                crate::state::RepoSubview::RemoteBranches => {
                    step_remote_branch_selection(repo_mode, step)
                }
                crate::state::RepoSubview::Tags => step_tag_selection(repo_mode, step),
                crate::state::RepoSubview::Commits => step_commit_selection(repo_mode, step),
                crate::state::RepoSubview::Stash => match repo_mode.stash_subview_mode {
                    crate::state::StashSubviewMode::List => step_stash_selection(repo_mode, step),
                    crate::state::StashSubviewMode::Files => {
                        step_stash_file_selection(repo_mode, step)
                    }
                },
                crate::state::RepoSubview::Reflog => step_reflog_selection(repo_mode, step),
                crate::state::RepoSubview::Worktrees => step_worktree_selection(repo_mode, step),
                crate::state::RepoSubview::Submodules => step_submodule_selection(repo_mode, step),
                crate::state::RepoSubview::Status
                | crate::state::RepoSubview::Compare
                | crate::state::RepoSubview::Rebase => false,
            }
        }
        _ => false,
    }
}

fn select_repo_list_edge(
    state: &mut AppState,
    select_last: bool,
    effects: &mut Vec<Effect>,
) -> bool {
    match state.focused_pane {
        PaneId::RepoUnstaged | PaneId::RepoStaged => {
            let Some(repo_mode) = state.repo_mode.as_mut() else {
                return false;
            };

            if !select_status_edge(repo_mode, state.focused_pane, select_last) {
                return false;
            }

            if let Some((selected_path, diff_presentation)) =
                selected_status_detail_request(repo_mode, state.focused_pane)
            {
                effects.push(Effect::LoadRepoDetail {
                    repo_id: repo_mode.current_repo_id.clone(),
                    selected_path: Some(selected_path),
                    diff_presentation,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: repo_mode.ignore_whitespace_in_diff,
                    diff_context_lines: repo_mode.diff_context_lines,
                    rename_similarity_threshold: repo_mode.rename_similarity_threshold,
                });
            }
            true
        }
        PaneId::RepoDetail => {
            let Some(repo_mode) = state.repo_mode.as_mut() else {
                return false;
            };

            match repo_mode.active_subview {
                crate::state::RepoSubview::Branches => select_branch_edge(repo_mode, select_last),
                crate::state::RepoSubview::Remotes => select_remote_edge(repo_mode, select_last),
                crate::state::RepoSubview::RemoteBranches => {
                    select_remote_branch_edge(repo_mode, select_last)
                }
                crate::state::RepoSubview::Tags => select_tag_edge(repo_mode, select_last),
                crate::state::RepoSubview::Commits => select_commit_edge(repo_mode, select_last),
                crate::state::RepoSubview::Stash => match repo_mode.stash_subview_mode {
                    crate::state::StashSubviewMode::List => {
                        select_stash_edge(repo_mode, select_last)
                    }
                    crate::state::StashSubviewMode::Files => {
                        select_stash_file_edge(repo_mode, select_last)
                    }
                },
                crate::state::RepoSubview::Reflog => select_reflog_edge(repo_mode, select_last),
                crate::state::RepoSubview::Worktrees => {
                    select_worktree_edge(repo_mode, select_last)
                }
                crate::state::RepoSubview::Submodules => {
                    select_submodule_edge(repo_mode, select_last)
                }
                crate::state::RepoSubview::Status
                | crate::state::RepoSubview::Compare
                | crate::state::RepoSubview::Rebase => false,
            }
        }
        _ => false,
    }
}

fn step_diff_hunk_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let commit_file_diff_active = commit_file_diff_detail_active(repo_mode);
    let Some(detail) = repo_mode.detail.as_mut() else {
        return false;
    };

    let diff = &mut detail.diff;
    if diff.presentation == DiffPresentation::Comparison && !commit_file_diff_active {
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

fn clear_repo_subview_filter_focus(repo_mode: &mut RepoModeState) {
    repo_mode.branches_filter.focused = false;
    repo_mode.remote_branches_filter.focused = false;
    repo_mode.tags_filter.focused = false;
    repo_mode.commits_filter.focused = false;
    repo_mode.commit_files_filter.focused = false;
    repo_mode.stash_filter.focused = false;
    repo_mode.reflog_filter.focused = false;
    repo_mode.worktree_filter.focused = false;
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

fn has_unstaged_tracked_changes(detail: &crate::state::RepoDetail) -> bool {
    detail.file_tree.iter().any(|item| {
        item.unstaged_kind
            .is_some_and(|kind| kind != crate::state::FileStatusKind::Untracked)
    })
}

fn has_stashable_changes(detail: &crate::state::RepoDetail) -> bool {
    staged_file_count(detail) > 0 || has_unstaged_tracked_changes(detail)
}

fn has_local_changes(detail: &crate::state::RepoDetail) -> bool {
    detail
        .file_tree
        .iter()
        .any(|item| item.staged_kind.is_some() || item.unstaged_kind.is_some())
}

fn stash_mode_available(detail: &crate::state::RepoDetail, mode: StashMode) -> bool {
    match mode {
        StashMode::Tracked => has_stashable_changes(detail),
        StashMode::KeepIndex => has_unstaged_tracked_changes(detail),
        StashMode::IncludeUntracked => has_local_changes(detail),
        StashMode::Staged => staged_file_count(detail) > 0,
        StashMode::Unstaged => has_unstaged_tracked_changes(detail),
    }
}

fn stash_mode_unavailable_message(mode: StashMode) -> &'static str {
    match mode {
        StashMode::Tracked => "No tracked changes are available to stash.",
        StashMode::KeepIndex => {
            "No unstaged tracked changes are available to stash while keeping the index."
        }
        StashMode::IncludeUntracked => "No local changes are available to stash.",
        StashMode::Staged => "No staged changes are available to stash.",
        StashMode::Unstaged => "No unstaged tracked changes are available to stash.",
    }
}

fn stash_prompt_title(mode: StashMode) -> &'static str {
    match mode {
        StashMode::Tracked => "Stash tracked changes",
        StashMode::KeepIndex => "Stash tracked changes and keep staged changes",
        StashMode::IncludeUntracked => "Stash all changes including untracked",
        StashMode::Staged => "Stash staged changes",
        StashMode::Unstaged => "Stash unstaged changes",
    }
}

fn stash_operation_summary(mode: StashMode, message: Option<&str>) -> String {
    let prefix = match mode {
        StashMode::Tracked => "Stashed tracked changes",
        StashMode::KeepIndex => "Stashed tracked changes and kept staged changes",
        StashMode::IncludeUntracked => "Stashed all changes including untracked",
        StashMode::Staged => "Stashed staged changes",
        StashMode::Unstaged => "Stashed unstaged changes",
    };

    match message {
        Some(message) => format!("{prefix}: {message}"),
        None => prefix.to_string(),
    }
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
        ConfirmableOperation::FetchRemote { remote_name } => {
            format!("Fetch remote {remote_name}")
        }
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
        ConfirmableOperation::ApplyFixupCommits { summary, .. } => {
            format!("Apply fixup commits for {summary}")
        }
        ConfirmableOperation::FixupCommit { summary, .. } => {
            format!("Fixup {summary}")
        }
        ConfirmableOperation::SetFixupMessageForCommit { summary, .. } => {
            format!("Set fixup message from {summary}")
        }
        ConfirmableOperation::SquashCommit { summary, .. } => {
            format!("Squash {summary}")
        }
        ConfirmableOperation::DropCommit { summary, .. } => {
            format!("Drop {summary}")
        }
        ConfirmableOperation::MoveCommitUp {
            summary,
            adjacent_summary,
            ..
        } => {
            format!("Move {summary} above {adjacent_summary}")
        }
        ConfirmableOperation::MoveCommitDown {
            summary,
            adjacent_summary,
            ..
        } => {
            format!("Move {summary} below {adjacent_summary}")
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
        ConfirmableOperation::DeleteBranch { branch_name, force } => {
            if *force {
                format!("Force delete branch {branch_name}")
            } else {
                format!("Delete branch {branch_name}")
            }
        }
        ConfirmableOperation::UnsetBranchUpstream { branch_name } => {
            format!("Unset upstream for {branch_name}")
        }
        ConfirmableOperation::FastForwardCurrentBranchFromUpstream {
            branch_name,
            upstream_ref,
        } => format!("Fast-forward {branch_name} from {upstream_ref}"),
        ConfirmableOperation::ForceCheckoutRef { source_label, .. } => {
            format!("Force checkout {source_label}")
        }
        ConfirmableOperation::MergeRefIntoCurrent {
            source_label,
            variant,
            ..
        } => {
            format!("{} {source_label} into current branch", variant.title())
        }
        ConfirmableOperation::RebaseCurrentBranchOntoRef { source_label, .. } => {
            format!("Rebase current branch onto {source_label}")
        }
        ConfirmableOperation::RemoveRemote { remote_name } => {
            format!("Remove remote {remote_name}")
        }
        ConfirmableOperation::DeleteRemoteBranch {
            remote_name,
            branch_name,
        } => format!("Delete remote branch {remote_name}/{branch_name}"),
        ConfirmableOperation::DeleteTag { tag_name } => format!("Delete tag {tag_name}"),
        ConfirmableOperation::PushTag {
            remote_name,
            tag_name,
        } => format!("Push tag {tag_name} to {remote_name}"),
        ConfirmableOperation::PopStash { stash_ref } => format!("Pop stash {stash_ref}"),
        ConfirmableOperation::DropStash { stash_ref } => format!("Drop stash {stash_ref}"),
        ConfirmableOperation::RemoveWorktree { path } => {
            format!("Remove worktree {}", path.display())
        }
        ConfirmableOperation::RemoveSubmodule { name, .. } => {
            format!("Remove submodule {name}")
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
        ConfirmableOperation::ApplyFixupCommits { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::ApplyFixups,
            },
            "Apply fixup autosquash",
        ),
        ConfirmableOperation::FixupCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Fixup,
            },
            "Start fixup autosquash",
        ),
        ConfirmableOperation::SetFixupMessageForCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::FixupWithMessage,
            },
            "Set fixup message",
        ),
        ConfirmableOperation::SquashCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Squash,
            },
            "Start squash rebase",
        ),
        ConfirmableOperation::DropCommit { commit, .. } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::Drop,
            },
            "Drop selected commit",
        ),
        ConfirmableOperation::MoveCommitUp {
            commit,
            adjacent_commit,
            ..
        } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::MoveUp { adjacent_commit },
            },
            "Move selected commit up",
        ),
        ConfirmableOperation::MoveCommitDown {
            commit,
            adjacent_commit,
            ..
        } => (
            GitCommand::StartCommitRebase {
                commit,
                mode: RebaseStartMode::MoveDown { adjacent_commit },
            },
            "Move selected commit down",
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
        ConfirmableOperation::FetchRemote { remote_name } => (
            GitCommand::FetchRemote {
                remote_name: remote_name.clone(),
            },
            "Fetch remote",
        ),
        ConfirmableOperation::DeleteBranch { branch_name, force } => (
            GitCommand::DeleteBranch {
                branch_name: branch_name.clone(),
                force,
            },
            if force {
                "Force delete branch"
            } else {
                "Delete branch"
            },
        ),
        ConfirmableOperation::UnsetBranchUpstream { branch_name } => (
            GitCommand::UnsetBranchUpstream {
                branch_name: branch_name.clone(),
            },
            "Unset branch upstream",
        ),
        ConfirmableOperation::FastForwardCurrentBranchFromUpstream { upstream_ref, .. } => (
            GitCommand::FastForwardCurrentBranchFromUpstream {
                upstream_ref: upstream_ref.clone(),
            },
            "Fast-forward current branch from upstream",
        ),
        ConfirmableOperation::ForceCheckoutRef { target_ref, .. } => (
            GitCommand::ForceCheckoutRef {
                target_ref: target_ref.clone(),
            },
            "Force checkout selected ref",
        ),
        ConfirmableOperation::MergeRefIntoCurrent {
            target_ref,
            variant,
            ..
        } => (
            GitCommand::MergeRefIntoCurrent {
                target_ref: target_ref.clone(),
                variant,
            },
            variant.title(),
        ),
        ConfirmableOperation::RebaseCurrentBranchOntoRef { target_ref, .. } => (
            GitCommand::RebaseCurrentOntoRef {
                target_ref: target_ref.clone(),
            },
            "Rebase current branch onto selected ref",
        ),
        ConfirmableOperation::RemoveRemote { remote_name } => (
            GitCommand::RemoveRemote {
                remote_name: remote_name.clone(),
            },
            "Remove remote",
        ),
        ConfirmableOperation::DeleteRemoteBranch {
            remote_name,
            branch_name,
        } => (
            GitCommand::DeleteRemoteBranch {
                remote_name: remote_name.clone(),
                branch_name: branch_name.clone(),
            },
            "Delete remote branch",
        ),
        ConfirmableOperation::DeleteTag { tag_name } => (
            GitCommand::DeleteTag {
                tag_name: tag_name.clone(),
            },
            "Delete tag",
        ),
        ConfirmableOperation::PushTag {
            remote_name,
            tag_name,
        } => (
            GitCommand::PushTag {
                remote_name: remote_name.clone(),
                tag_name: tag_name.clone(),
            },
            "Push tag",
        ),
        ConfirmableOperation::PopStash { stash_ref } => (
            GitCommand::PopStash {
                stash_ref: stash_ref.clone(),
            },
            "Pop stash",
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
        ConfirmableOperation::RemoveSubmodule { path, .. } => (
            GitCommand::RemoveSubmodule { path: path.clone() },
            "Remove submodule",
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
    let return_focus = state.focused_pane;
    state.pending_input_prompt = Some(PendingInputPrompt {
        repo_id,
        operation,
        value,
        return_focus,
    });
    state.modal_stack.push(crate::state::Modal::new(
        crate::state::ModalKind::InputPrompt,
        title,
    ));
    state.focused_pane = PaneId::Modal;
}

fn open_menu(state: &mut AppState, repo_id: crate::state::RepoId, operation: MenuOperation) {
    let title = menu_title(operation);
    let return_focus = state.focused_pane;
    state.pending_menu = Some(PendingMenu {
        repo_id,
        operation,
        selected_index: 0,
        return_focus,
    });
    state.modal_stack.push(crate::state::Modal::new(
        crate::state::ModalKind::Menu,
        title,
    ));
    state.focused_pane = PaneId::Modal;
}

fn input_prompt_title(operation: &InputPromptOperation) -> String {
    match operation {
        InputPromptOperation::CheckoutBranch => "Check out branch".to_string(),
        InputPromptOperation::CreateBranch => "Create branch".to_string(),
        InputPromptOperation::StartGitFlow { branch_type } => {
            format!("New {} name", branch_type.command_name())
        }
        InputPromptOperation::CreateRemote => "Add remote".to_string(),
        InputPromptOperation::ForkRemote { suggested_name, .. } => {
            format!("Fork remote into {suggested_name}")
        }
        InputPromptOperation::CreateTag => "Create tag".to_string(),
        InputPromptOperation::CreateTagFromCommit { summary, .. } => {
            format!("New tag name from {summary}")
        }
        InputPromptOperation::CreateTagFromRef { source_label, .. } => {
            format!("New tag name from {source_label}")
        }
        InputPromptOperation::CreateBranchFromCommit { summary, .. } => {
            format!("New branch name from {summary}")
        }
        InputPromptOperation::CreateBranchFromRemote {
            remote_branch_ref, ..
        } => {
            format!("New local branch from {remote_branch_ref}")
        }
        InputPromptOperation::RenameBranch { current_name } => {
            format!("Rename branch {current_name}")
        }
        InputPromptOperation::EditRemote { current_name, .. } => {
            format!("Edit remote {current_name}")
        }
        InputPromptOperation::RenameStash { stash_ref, .. } => {
            format!("Rename stash: {stash_ref}")
        }
        InputPromptOperation::CreateBranchFromStash { stash_label, .. } => {
            format!("New branch name (branch is off of '{stash_label}')")
        }
        InputPromptOperation::SetBranchUpstream { branch_name } => {
            format!("Set upstream for {branch_name}")
        }
        InputPromptOperation::CreateStash { mode } => stash_prompt_title(*mode).to_string(),
        InputPromptOperation::CreateWorktree => "Create worktree".to_string(),
        InputPromptOperation::CreateSubmodule => "Add submodule".to_string(),
        InputPromptOperation::ShellCommand => "Run shell command".to_string(),
        InputPromptOperation::EditSubmoduleUrl { name, .. } => {
            format!("Edit submodule {name}")
        }
        InputPromptOperation::CreateAmendCommit {
            summary,
            include_file_changes,
            ..
        } => {
            if *include_file_changes {
                format!("Create amend! commit with changes for {summary}")
            } else {
                format!("Create amend! commit without changes for {summary}")
            }
        }
        InputPromptOperation::SetCommitCoAuthor { summary, .. } => {
            format!("Set co-author for {summary}")
        }
        InputPromptOperation::RewordCommit { summary, .. } => format!("Reword {summary}"),
    }
}

fn input_prompt_initial_value(operation: &InputPromptOperation) -> String {
    match operation {
        InputPromptOperation::CheckoutBranch => String::new(),
        InputPromptOperation::CreateBranch => String::new(),
        InputPromptOperation::StartGitFlow { .. } => String::new(),
        InputPromptOperation::CreateRemote => String::new(),
        InputPromptOperation::ForkRemote {
            suggested_name,
            remote_url,
        } => format!("{suggested_name} {remote_url}"),
        InputPromptOperation::CreateTag => String::new(),
        InputPromptOperation::CreateTagFromCommit { .. } => String::new(),
        InputPromptOperation::CreateTagFromRef { .. } => String::new(),
        InputPromptOperation::CreateBranchFromCommit { .. } => String::new(),
        InputPromptOperation::CreateBranchFromRemote { suggested_name, .. } => {
            suggested_name.clone()
        }
        InputPromptOperation::RenameBranch { current_name } => current_name.clone(),
        InputPromptOperation::EditRemote {
            current_name,
            current_url,
        } => format!("{current_name} {current_url}"),
        InputPromptOperation::RenameStash { current_name, .. } => current_name.clone(),
        InputPromptOperation::CreateBranchFromStash { .. } => String::new(),
        InputPromptOperation::SetBranchUpstream { branch_name: _ } => String::new(),
        InputPromptOperation::CreateStash { mode: _ } => String::new(),
        InputPromptOperation::CreateWorktree => String::new(),
        InputPromptOperation::CreateSubmodule => String::new(),
        InputPromptOperation::ShellCommand => String::new(),
        InputPromptOperation::EditSubmoduleUrl { current_url, .. } => current_url.clone(),
        InputPromptOperation::CreateAmendCommit {
            initial_message, ..
        } => initial_message.clone(),
        InputPromptOperation::SetCommitCoAuthor { .. } => String::new(),
        InputPromptOperation::RewordCommit {
            initial_message, ..
        } => initial_message.clone(),
    }
}

enum PromptSubmission {
    Git(GitCommandRequest),
    Shell(ShellCommandRequest),
}

fn submit_input_prompt(state: &mut AppState) -> Option<PromptSubmission> {
    let pending = state.pending_input_prompt.take()?;
    let value = pending.value.trim().to_string();
    if value.is_empty()
        && !matches!(
            pending.operation,
            InputPromptOperation::CreateStash { mode: _ }
                | InputPromptOperation::RenameStash { .. }
        )
    {
        state.pending_input_prompt = Some(pending);
        return None;
    }

    state.modal_stack.pop();
    if state.modal_stack.is_empty() {
        state.focused_pane = pending.return_focus;
    }

    let (submission, summary) = match pending.operation {
        InputPromptOperation::CheckoutBranch => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CheckoutBranch {
                    branch_ref: value.clone(),
                },
            )),
            format!("Checkout branch {value}"),
        ),
        InputPromptOperation::CreateBranch => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateBranch {
                    branch_name: value.clone(),
                },
            )),
            format!("Create branch {value}"),
        ),
        InputPromptOperation::StartGitFlow { branch_type } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::StartGitFlow {
                    branch_type,
                    name: value.clone(),
                },
            )),
            format!("Start git-flow {} {value}", branch_type.command_name()),
        ),
        InputPromptOperation::CreateRemote => {
            let Some((remote_name, remote_url)) = parse_remote_input(&value) else {
                push_warning(state, "Enter remote details as: <name> <url>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::AddRemote {
                        remote_name: remote_name.clone(),
                        remote_url: remote_url.clone(),
                    },
                )),
                format!("Add remote {remote_name}"),
            )
        }
        InputPromptOperation::ForkRemote { .. } => {
            let Some((remote_name, remote_url)) = parse_remote_input(&value) else {
                push_warning(state, "Enter remote details as: <name> <url>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::AddRemote {
                        remote_name: remote_name.clone(),
                        remote_url: remote_url.clone(),
                    },
                )),
                format!("Add fork remote {remote_name}"),
            )
        }
        InputPromptOperation::CreateTag => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateTag {
                    tag_name: value.clone(),
                },
            )),
            format!("Create tag {value}"),
        ),
        InputPromptOperation::CreateTagFromCommit { commit, summary } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateTagFromCommit {
                    tag_name: value.clone(),
                    commit,
                },
            )),
            format!("Create tag {value} from {summary}"),
        ),
        InputPromptOperation::CreateTagFromRef {
            target_ref,
            source_label,
        } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateTagFromCommit {
                    tag_name: value.clone(),
                    commit: target_ref,
                },
            )),
            format!("Create tag {value} from {source_label}"),
        ),
        InputPromptOperation::CreateBranchFromCommit { commit, summary } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateBranchFromCommit {
                    branch_name: value.clone(),
                    commit,
                },
            )),
            format!("Create branch {value} from {summary}"),
        ),
        InputPromptOperation::CreateBranchFromRemote {
            remote_branch_ref,
            suggested_name,
        } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateBranchFromRef {
                    branch_name: value.clone(),
                    start_point: remote_branch_ref.clone(),
                    track: value == *suggested_name,
                },
            )),
            format!("Create branch {value} from {remote_branch_ref}"),
        ),
        InputPromptOperation::RenameBranch { current_name } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::RenameBranch {
                    branch_name: current_name.clone(),
                    new_name: value.clone(),
                },
            )),
            format!("Rename branch {current_name} to {value}"),
        ),
        InputPromptOperation::EditRemote {
            ref current_name, ..
        } => {
            let Some((new_name, remote_url)) = parse_remote_input(&value) else {
                push_warning(state, "Enter remote details as: <name> <url>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::EditRemote {
                        current_name: current_name.clone(),
                        new_name: new_name.clone(),
                        remote_url,
                    },
                )),
                format!("Edit remote {current_name}"),
            )
        }
        InputPromptOperation::RenameStash { stash_ref, .. } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::RenameStash {
                    stash_ref: stash_ref.clone(),
                    message: value.clone(),
                },
            )),
            format!("Rename stash {stash_ref}"),
        ),
        InputPromptOperation::CreateBranchFromStash { stash_ref, .. } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateBranchFromStash {
                    stash_ref: stash_ref.clone(),
                    branch_name: value.clone(),
                },
            )),
            format!("Create branch {value} from {stash_ref}"),
        ),
        InputPromptOperation::SetBranchUpstream { branch_name } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::SetBranchUpstream {
                    branch_name: branch_name.clone(),
                    upstream_ref: value.clone(),
                },
            )),
            format!("Set upstream for {branch_name}"),
        ),
        InputPromptOperation::CreateStash { mode } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateStash {
                    message: if value.is_empty() {
                        None
                    } else {
                        Some(value.clone())
                    },
                    mode,
                },
            )),
            stash_operation_summary(mode, if value.is_empty() { None } else { Some(&value) }),
        ),
        InputPromptOperation::CreateWorktree => {
            let Some((path, branch_ref)) = parse_create_worktree_input(&value) else {
                push_warning(state, "Enter worktree details as: <path> <branch>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::CreateWorktree {
                        path: path.clone(),
                        branch_ref: branch_ref.clone(),
                    },
                )),
                format!("Create worktree {} from {branch_ref}", path.display()),
            )
        }
        InputPromptOperation::CreateSubmodule => {
            let Some((path, url)) = parse_create_submodule_input(&value) else {
                push_warning(state, "Enter submodule details as: <path> <url>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::AddSubmodule {
                        path: path.clone(),
                        url: url.clone(),
                    },
                )),
                format!("Add submodule {} from {url}", path.display()),
            )
        }
        InputPromptOperation::EditSubmoduleUrl { name, path, .. } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::EditSubmoduleUrl {
                    name: name.clone(),
                    path: path.clone(),
                    url: value.clone(),
                },
            )),
            format!("Edit submodule {name}"),
        ),
        InputPromptOperation::ShellCommand => (
            PromptSubmission::Shell(shell_job(pending.repo_id.clone(), value.clone())),
            format!("Run shell command: {value}"),
        ),
        InputPromptOperation::CreateAmendCommit {
            summary,
            original_subject,
            include_file_changes,
            ..
        } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::CreateAmendCommit {
                    original_subject: original_subject.clone(),
                    message: value.clone(),
                    include_file_changes,
                },
            )),
            if include_file_changes {
                format!("Create amend! commit with changes for {summary}")
            } else {
                format!("Create amend! commit without changes for {summary}")
            },
        ),
        InputPromptOperation::SetCommitCoAuthor {
            ref commit,
            ref summary,
        } => {
            let Some(co_author) = parse_commit_co_author_input(&value) else {
                push_warning(state, "Enter the co-author as: Name <email@example.com>.");
                state.pending_input_prompt = Some(pending);
                return None;
            };
            (
                PromptSubmission::Git(git_job(
                    pending.repo_id.clone(),
                    GitCommand::AmendCommitAttributes {
                        commit: commit.clone(),
                        reset_author: false,
                        co_author: Some(co_author),
                    },
                )),
                format!("Set co-author for {summary}"),
            )
        }
        InputPromptOperation::RewordCommit {
            commit, summary, ..
        } => (
            PromptSubmission::Git(git_job(
                pending.repo_id.clone(),
                GitCommand::StartCommitRebase {
                    commit,
                    mode: RebaseStartMode::Reword {
                        message: value.clone(),
                    },
                },
            )),
            format!("Reword {summary}"),
        ),
    };
    match &submission {
        PromptSubmission::Git(job) => enqueue_git_job(state, job, &summary),
        PromptSubmission::Shell(job) => enqueue_shell_job(state, job, &summary),
    }
    Some(submission)
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

fn parse_create_submodule_input(value: &str) -> Option<(std::path::PathBuf, String)> {
    let (path, url) = value.split_once(char::is_whitespace)?;
    let path = path.trim();
    let url = url.trim();
    if path.is_empty() || url.is_empty() {
        return None;
    }
    Some((std::path::PathBuf::from(path), url.to_string()))
}

fn parse_remote_input(value: &str) -> Option<(String, String)> {
    let (remote_name, remote_url) = value.split_once(char::is_whitespace)?;
    let remote_name = remote_name.trim();
    let remote_url = remote_url.trim();
    if remote_name.is_empty() || remote_url.is_empty() {
        return None;
    }
    Some((remote_name.to_string(), remote_url.to_string()))
}

fn parse_commit_co_author_input(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value
        .strip_prefix("Co-authored-by:")
        .map(str::trim)
        .unwrap_or(value);
    let start = value.rfind('<')?;
    let end = value.rfind('>')?;
    if start == 0 || end <= start + 1 || end != value.len() - 1 {
        return None;
    }
    let name = value[..start].trim();
    let email = value[start + 1..end].trim();
    if name.is_empty() || email.is_empty() {
        return None;
    }
    Some(format!("Co-authored-by: {name} <{email}>"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuEntry {
    label: String,
    action: Action,
}

fn menu_title(operation: MenuOperation) -> &'static str {
    match operation {
        MenuOperation::StashOptions => "Stash options",
        MenuOperation::FilterOptions => "Filter options",
        MenuOperation::DiffOptions => "Diffing options",
        MenuOperation::CommitLogOptions => "Commit log options",
        MenuOperation::CommitCopyOptions => "Copy commit attribute",
        MenuOperation::BranchGitFlowOptions => "Git-flow options",
        MenuOperation::BranchPullRequestOptions => "Pull request options",
        MenuOperation::BranchResetOptions => "Branch reset options",
        MenuOperation::BranchSortOptions => "Branch sort options",
        MenuOperation::TagResetOptions => "Tag reset options",
        MenuOperation::ReflogResetOptions => "Reflog reset options",
        MenuOperation::CommitAmendAttributeOptions => "Amend commit attributes",
        MenuOperation::CommitFixupOptions => "Fixup options",
        MenuOperation::BisectOptions => "Bisect options",
        MenuOperation::BranchUpstreamOptions => "Branch upstream options",
        MenuOperation::MergeRebaseOptions => "Merge / rebase options",
        MenuOperation::RemoteBranchPullRequestOptions => "Remote branch pull request options",
        MenuOperation::RemoteBranchResetOptions => "Remote branch reset options",
        MenuOperation::RemoteBranchSortOptions => "Remote branch sort options",
        MenuOperation::IgnoreOptions => "Ignore options",
        MenuOperation::StatusResetOptions => "Reset options",
        MenuOperation::PatchOptions => "Patch options",
        MenuOperation::BulkSubmoduleOptions => "Bulk submodule options",
        MenuOperation::RecentRepos => "Recent repositories",
        MenuOperation::CommandLog => "Command log",
    }
}

fn menu_item_count(state: &AppState, operation: MenuOperation) -> usize {
    match operation {
        MenuOperation::StashOptions => 5,
        MenuOperation::FilterOptions => filter_menu_entries(state).len(),
        MenuOperation::DiffOptions => diff_menu_entries(state).len(),
        MenuOperation::CommitLogOptions => commit_log_menu_entries(state).len(),
        MenuOperation::CommitCopyOptions => 5,
        MenuOperation::BranchGitFlowOptions => branch_git_flow_menu_entries(state).len(),
        MenuOperation::BranchPullRequestOptions => branch_pull_request_menu_entries(state).len(),
        MenuOperation::BranchResetOptions => branch_reset_menu_entries(state).len(),
        MenuOperation::BranchSortOptions => branch_sort_menu_entries(state).len(),
        MenuOperation::TagResetOptions => tag_reset_menu_entries(state).len(),
        MenuOperation::ReflogResetOptions => reflog_reset_menu_entries(state).len(),
        MenuOperation::CommitAmendAttributeOptions => 2,
        MenuOperation::CommitFixupOptions => 3,
        MenuOperation::BisectOptions => bisect_menu_entries(state).len(),
        MenuOperation::BranchUpstreamOptions => branch_upstream_menu_entries(state).len(),
        MenuOperation::MergeRebaseOptions => merge_rebase_menu_entries(state).len(),
        MenuOperation::RemoteBranchPullRequestOptions => {
            remote_branch_pull_request_menu_entries(state).len()
        }
        MenuOperation::RemoteBranchResetOptions => remote_branch_reset_menu_entries(state).len(),
        MenuOperation::RemoteBranchSortOptions => remote_branch_sort_menu_entries(state).len(),
        MenuOperation::IgnoreOptions => ignore_menu_entries(state).len(),
        MenuOperation::StatusResetOptions => status_reset_menu_entries(state).len(),
        MenuOperation::PatchOptions => patch_menu_entries(state).len(),
        MenuOperation::BulkSubmoduleOptions => bulk_submodule_menu_entries().len(),
        MenuOperation::RecentRepos => recent_repo_menu_repo_ids(state).len(),
        MenuOperation::CommandLog => state.status_messages.len(),
    }
}

fn branch_upstream_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Vec::new();
    };
    let upstream_label = branch.upstream.as_deref().unwrap_or("none");
    vec![
        MenuEntry {
            label: format!("Set upstream for {}...", branch.name),
            action: Action::OpenInputPrompt {
                operation: InputPromptOperation::SetBranchUpstream {
                    branch_name: branch.name.clone(),
                },
            },
        },
        MenuEntry {
            label: format!("Unset upstream ({upstream_label})"),
            action: Action::UnsetSelectedBranchUpstream,
        },
        MenuEntry {
            label: format!("Fast-forward current branch from upstream ({upstream_label})"),
            action: Action::FastForwardSelectedBranchFromUpstream,
        },
    ]
}

fn merge_fast_forward_warning(current_branch_name: &str, source_label: &str) -> String {
    format!("Cannot fast-forward merge {source_label} into {current_branch_name}.")
}

fn merge_menu_entry(label: String, action: Action, disabled_reason: Option<String>) -> MenuEntry {
    match disabled_reason {
        Some(reason) => MenuEntry {
            label: format!("{label} [disabled]"),
            action: Action::ShowWarning { message: reason },
        },
        None => MenuEntry { label, action },
    }
}

fn merge_rebase_branch_menu_entries(
    repo_mode: &RepoModeState,
    source_label: &str,
) -> Vec<MenuEntry> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let current_branch_name = current_branch_item(repo_mode)
        .map(|branch| branch.name.as_str())
        .unwrap_or("current branch");
    let can_fast_forward = detail
        .fast_forward_merge_targets
        .get(source_label)
        .copied()
        .unwrap_or(false);
    let prefer_fast_forward = matches!(
        detail.merge_fast_forward_preference,
        MergeFastForwardPreference::FastForward
    );
    let prefer_no_fast_forward = matches!(
        detail.merge_fast_forward_preference,
        MergeFastForwardPreference::NoFastForward
    );
    let prefer_regular_fast_forward =
        !prefer_no_fast_forward && (prefer_fast_forward || can_fast_forward);
    let disabled_reason =
        (!can_fast_forward).then(|| merge_fast_forward_warning(current_branch_name, source_label));

    let (first_entry, second_entry) = if prefer_regular_fast_forward {
        (
            merge_menu_entry(
                format!("Merge {source_label} into current branch (regular, fast-forward)"),
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::Regular,
                },
                disabled_reason.clone(),
            ),
            merge_menu_entry(
                format!("Merge {source_label} into current branch (no-fast-forward)"),
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::NoFastForward,
                },
                None,
            ),
        )
    } else {
        (
            merge_menu_entry(
                format!("Merge {source_label} into current branch (regular, no-fast-forward)"),
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::Regular,
                },
                None,
            ),
            merge_menu_entry(
                format!("Merge {source_label} into current branch (fast-forward)"),
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::FastForward,
                },
                disabled_reason,
            ),
        )
    };

    vec![
        first_entry,
        second_entry,
        MenuEntry {
            label: format!("Squash merge {source_label} into current branch"),
            action: Action::MergeSelectedRefIntoCurrent {
                variant: MergeVariant::Squash,
            },
        },
    ]
}

fn branch_git_flow_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("finish branch '{}'", branch.name),
            action: Action::RunGitFlowFinish,
        },
        MenuEntry {
            label: "start feature".to_string(),
            action: Action::OpenInputPrompt {
                operation: InputPromptOperation::StartGitFlow {
                    branch_type: crate::state::GitFlowBranchType::Feature,
                },
            },
        },
        MenuEntry {
            label: "start hotfix".to_string(),
            action: Action::OpenInputPrompt {
                operation: InputPromptOperation::StartGitFlow {
                    branch_type: crate::state::GitFlowBranchType::Hotfix,
                },
            },
        },
        MenuEntry {
            label: "start bugfix".to_string(),
            action: Action::OpenInputPrompt {
                operation: InputPromptOperation::StartGitFlow {
                    branch_type: crate::state::GitFlowBranchType::Bugfix,
                },
            },
        },
        MenuEntry {
            label: "start release".to_string(),
            action: Action::OpenInputPrompt {
                operation: InputPromptOperation::StartGitFlow {
                    branch_type: crate::state::GitFlowBranchType::Release,
                },
            },
        },
    ]
}

fn branch_pull_request_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Ok(Some((_, _, label))) = selected_branch_pull_request_target(state) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Open pull request for {label}"),
            action: Action::OpenSelectedBranchPullRequest,
        },
        MenuEntry {
            label: format!("Copy pull request URL for {label}"),
            action: Action::CopySelectedBranchPullRequestUrl,
        },
    ]
}

fn branch_reset_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Soft reset to {}", branch.name),
            action: Action::SoftResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Mixed reset to {}", branch.name),
            action: Action::MixedResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Hard reset to {}", branch.name),
            action: Action::HardResetToSelectedCommit,
        },
    ]
}

fn branch_sort_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let current = state
        .repo_mode
        .as_ref()
        .map_or(crate::state::BranchSortMode::Natural, |repo_mode| {
            repo_mode.branch_sort_mode
        });
    vec![
        MenuEntry {
            label: format!(
                "Natural order{}",
                if current == crate::state::BranchSortMode::Natural {
                    " (current)"
                } else {
                    ""
                }
            ),
            action: Action::SetBranchSortMode(crate::state::BranchSortMode::Natural),
        },
        MenuEntry {
            label: format!(
                "Sort by branch name{}",
                if current == crate::state::BranchSortMode::Name {
                    " (current)"
                } else {
                    ""
                }
            ),
            action: Action::SetBranchSortMode(crate::state::BranchSortMode::Name),
        },
    ]
}

fn tag_reset_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(tag) = selected_tag_item(repo_mode) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Soft reset to {}", tag.name),
            action: Action::SoftResetToSelectedTag,
        },
        MenuEntry {
            label: format!("Mixed reset to {}", tag.name),
            action: Action::MixedResetToSelectedTag,
        },
        MenuEntry {
            label: format!("Hard reset to {}", tag.name),
            action: Action::HardResetToSelectedTag,
        },
    ]
}

fn reflog_reset_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some((_, entry)) = selected_reflog_entry(repo_mode) else {
        return Vec::new();
    };
    let target = if entry.selector.is_empty() {
        reflog_commit_label(entry)
    } else {
        entry.selector.clone()
    };
    vec![
        MenuEntry {
            label: format!("Soft reset to {target}"),
            action: Action::SoftResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Mixed reset to {target}"),
            action: Action::MixedResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Hard reset to {target}"),
            action: Action::HardResetToSelectedCommit,
        },
    ]
}

fn remote_branch_pull_request_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Ok(Some((_, _, label))) = selected_remote_branch_pull_request_target(state) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Open pull request for {label}"),
            action: Action::OpenSelectedRemoteBranchPullRequest,
        },
        MenuEntry {
            label: format!("Copy pull request URL for {label}"),
            action: Action::CopySelectedRemoteBranchPullRequestUrl,
        },
    ]
}

fn remote_branch_reset_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_remote_branch_item(repo_mode) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Soft reset to {}", branch.name),
            action: Action::SoftResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Mixed reset to {}", branch.name),
            action: Action::MixedResetToSelectedCommit,
        },
        MenuEntry {
            label: format!("Hard reset to {}", branch.name),
            action: Action::HardResetToSelectedCommit,
        },
    ]
}

fn remote_branch_sort_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let current = state
        .repo_mode
        .as_ref()
        .map_or(crate::state::RemoteBranchSortMode::Natural, |repo_mode| {
            repo_mode.remote_branch_sort_mode
        });
    vec![
        MenuEntry {
            label: format!(
                "Natural order{}",
                if current == crate::state::RemoteBranchSortMode::Natural {
                    " (current)"
                } else {
                    ""
                }
            ),
            action: Action::SetRemoteBranchSortMode(crate::state::RemoteBranchSortMode::Natural),
        },
        MenuEntry {
            label: format!(
                "Sort by branch name{}",
                if current == crate::state::RemoteBranchSortMode::Name {
                    " (current)"
                } else {
                    ""
                }
            ),
            action: Action::SetRemoteBranchSortMode(crate::state::RemoteBranchSortMode::Name),
        },
    ]
}

fn bulk_submodule_menu_entries() -> Vec<MenuEntry> {
    vec![
        MenuEntry {
            label: "Initialize all submodules".to_string(),
            action: Action::InitAllSubmodules,
        },
        MenuEntry {
            label: "Update all submodules".to_string(),
            action: Action::UpdateAllSubmodules,
        },
        MenuEntry {
            label: "Update all submodules recursively".to_string(),
            action: Action::UpdateAllSubmodulesRecursively,
        },
        MenuEntry {
            label: "Deinitialize all submodules".to_string(),
            action: Action::DeinitAllSubmodules,
        },
    ]
}

fn step_menu_selection(state: &mut AppState, step: isize) -> bool {
    let Some((operation, selected_index)) = state
        .pending_menu
        .as_ref()
        .map(|menu| (menu.operation, menu.selected_index))
    else {
        return false;
    };
    let count = menu_item_count(state, operation);
    if count == 0 {
        return false;
    }
    let next = (selected_index as isize + step).rem_euclid(count as isize) as usize;
    if next == selected_index {
        return false;
    }
    if let Some(menu) = state.pending_menu.as_mut() {
        menu.selected_index = next;
    }
    true
}

fn submit_menu_selection(state: &mut AppState, effects: &mut Vec<Effect>) -> bool {
    let Some((operation, selected_index)) = state
        .pending_menu
        .as_ref()
        .map(|menu| (menu.operation, menu.selected_index))
    else {
        return false;
    };

    match operation {
        MenuOperation::StashOptions => {
            let mode = match selected_index {
                0 => StashMode::Tracked,
                1 => StashMode::KeepIndex,
                2 => StashMode::IncludeUntracked,
                3 => StashMode::Staged,
                _ => StashMode::Unstaged,
            };

            let Some(detail) = state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
            else {
                return false;
            };

            if !stash_mode_available(detail, mode) {
                push_warning(state, stash_mode_unavailable_message(mode));
                return true;
            }

            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            open_input_prompt(
                state,
                menu.repo_id,
                InputPromptOperation::CreateStash { mode },
            );
            true
        }
        MenuOperation::FilterOptions => {
            let entries = filter_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::DiffOptions => {
            let entries = diff_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::TagResetOptions => {
            let entries = tag_reset_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::ReflogResetOptions => {
            let entries = reflog_reset_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::CommitFixupOptions => {
            let prompt_operation = match selected_index {
                0 => None,
                1 => match pending_history_commit_operation(
                    state,
                    |detail, commit, selected_index| {
                        if selected_index == 0 {
                            return Err("Select an older commit before creating an amend! commit."
                                .to_string());
                        }
                        if staged_file_count(detail) == 0 {
                            return Err(
                                "Stage changes before creating an amend! commit with changes."
                                    .to_string(),
                            );
                        }
                        Ok(InputPromptOperation::CreateAmendCommit {
                            summary: format!("{} {}", commit.short_oid, commit.summary),
                            original_subject: commit.summary.clone(),
                            include_file_changes: true,
                            initial_message: commit.summary.clone(),
                        })
                    },
                ) {
                    Ok(Some((_, operation))) => Some(operation),
                    Ok(None) => {
                        push_warning(state, "Select a commit before creating an amend! commit.");
                        return true;
                    }
                    Err(message) => {
                        push_warning(state, message);
                        return true;
                    }
                },
                _ => match pending_history_commit_operation(state, |_, commit, selected_index| {
                    if selected_index == 0 {
                        return Err(
                            "Select an older commit before creating an amend! commit.".to_string()
                        );
                    }
                    Ok(InputPromptOperation::CreateAmendCommit {
                        summary: format!("{} {}", commit.short_oid, commit.summary),
                        original_subject: commit.summary.clone(),
                        include_file_changes: false,
                        initial_message: commit.summary.clone(),
                    })
                }) {
                    Ok(Some((_, operation))) => Some(operation),
                    Ok(None) => {
                        push_warning(state, "Select a commit before creating an amend! commit.");
                        return true;
                    }
                    Err(message) => {
                        push_warning(state, message);
                        return true;
                    }
                },
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            if let Some(operation) = prompt_operation {
                open_input_prompt(state, menu.repo_id, operation);
            } else {
                reduce_action(state, Action::CreateFixupCommit, effects);
            }
            true
        }
        MenuOperation::CommitCopyOptions => {
            let target = match selected_index {
                0 => CommitClipboardTarget::ShortHash,
                1 => CommitClipboardTarget::FullHash,
                2 => CommitClipboardTarget::Summary,
                3 => CommitClipboardTarget::Diff,
                _ => CommitClipboardTarget::BrowserUrl,
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            match selected_commit_clipboard_target(state, target) {
                Ok(Some((repo_id, value, summary))) => {
                    let command = clipboard_shell_command(std::ffi::OsStr::new(&value), &state.os);
                    let job = shell_job(repo_id, command);
                    enqueue_shell_job(state, &job, &summary);
                    effects.push(Effect::RunShellCommand(job));
                }
                Ok(None) => {}
                Err(message) => {
                    push_warning(state, message);
                    effects.push(Effect::ScheduleRender);
                }
            }
            true
        }
        MenuOperation::CommitAmendAttributeOptions => {
            let direct_job = if selected_index == 0 {
                match pending_history_commit_operation(state, |_, commit, _| {
                    Ok((
                        commit.oid.clone(),
                        format!("{} {}", commit.short_oid, commit.summary),
                    ))
                }) {
                    Ok(Some((repo_id, (commit, summary)))) => Some((repo_id, commit, summary)),
                    Ok(None) => {
                        push_warning(
                            state,
                            "Select a commit before resetting its author metadata.",
                        );
                        return true;
                    }
                    Err(message) => {
                        push_warning(state, message);
                        return true;
                    }
                }
            } else {
                None
            };
            let prompt_operation = if selected_index == 1 {
                match pending_history_commit_operation(state, |_, commit, _| {
                    Ok(InputPromptOperation::SetCommitCoAuthor {
                        commit: commit.oid.clone(),
                        summary: format!("{} {}", commit.short_oid, commit.summary),
                    })
                }) {
                    Ok(Some((_, operation))) => Some(operation),
                    Ok(None) => {
                        push_warning(state, "Select a commit before setting a co-author.");
                        return true;
                    }
                    Err(message) => {
                        push_warning(state, message);
                        return true;
                    }
                }
            } else {
                None
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            if let Some(operation) = prompt_operation {
                open_input_prompt(state, menu.repo_id, operation);
                return true;
            }
            if let Some((repo_id, commit, summary)) = direct_job {
                let job = git_job(
                    repo_id,
                    GitCommand::AmendCommitAttributes {
                        commit,
                        reset_author: true,
                        co_author: None,
                    },
                );
                enqueue_git_job(state, &job, &format!("Reset author for {summary}"));
                effects.push(Effect::RunGitCommand(job));
            }
            true
        }
        MenuOperation::CommitLogOptions => {
            let entries = commit_log_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BranchGitFlowOptions => {
            let entries = branch_git_flow_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BranchPullRequestOptions => {
            let entries = branch_pull_request_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BranchResetOptions => {
            let entries = branch_reset_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BranchSortOptions => {
            let entries = branch_sort_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BranchUpstreamOptions => {
            let entries = branch_upstream_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BisectOptions => {
            let entries = bisect_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::MergeRebaseOptions => {
            let entries = merge_rebase_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::RemoteBranchPullRequestOptions => {
            let entries = remote_branch_pull_request_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::RemoteBranchResetOptions => {
            let entries = remote_branch_reset_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::RemoteBranchSortOptions => {
            let entries = remote_branch_sort_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::IgnoreOptions => {
            let entries = ignore_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::StatusResetOptions => {
            let entries = status_reset_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::PatchOptions => {
            let entries = patch_menu_entries(state);
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::BulkSubmoduleOptions => {
            let entries = bulk_submodule_menu_entries();
            let Some(action) = entries
                .get(selected_index)
                .map(|entry| entry.action.clone())
            else {
                return false;
            };
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            reduce_action(state, action, effects);
            true
        }
        MenuOperation::RecentRepos => {
            let repo_ids = recent_repo_menu_repo_ids(state);
            let Some(repo_id) = repo_ids.get(selected_index).cloned() else {
                return false;
            };
            state.pending_menu.take();
            state.modal_stack.pop();
            enter_repo_mode_with_parents(state, repo_id, Vec::new(), effects);
            true
        }
        MenuOperation::CommandLog => {
            let Some(menu) = state.pending_menu.take() else {
                return false;
            };
            state.modal_stack.pop();
            if state.modal_stack.is_empty() {
                state.focused_pane = menu.return_focus;
            }
            true
        }
    }
}

fn filter_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(filter) = repo_mode.subview_filter(repo_mode.active_subview) else {
        return Vec::new();
    };

    let subject = filter_menu_subject(repo_mode);
    let mut entries = vec![MenuEntry {
        label: if filter.query.trim().is_empty() {
            format!("Focus {subject} filter")
        } else {
            format!("Edit {subject} filter (/{})", filter.query)
        },
        action: Action::FocusRepoSubviewFilter,
    }];

    if !filter.query.trim().is_empty() {
        entries.push(MenuEntry {
            label: format!("Clear {subject} filter"),
            action: Action::CancelRepoSubviewFilter,
        });
    }

    entries
}

fn filter_menu_subject(repo_mode: &RepoModeState) -> &'static str {
    match repo_mode.active_subview {
        crate::state::RepoSubview::Branches => "branch list",
        crate::state::RepoSubview::Remotes => "remote list",
        crate::state::RepoSubview::RemoteBranches => "remote-branch list",
        crate::state::RepoSubview::Tags => "tag list",
        crate::state::RepoSubview::Commits => match repo_mode.commit_subview_mode {
            crate::state::CommitSubviewMode::History => "commit history",
            crate::state::CommitSubviewMode::Files => "commit-file list",
        },
        crate::state::RepoSubview::Stash => "stash list",
        crate::state::RepoSubview::Reflog => "reflog list",
        crate::state::RepoSubview::Worktrees => "worktree list",
        crate::state::RepoSubview::Submodules => "submodule list",
        crate::state::RepoSubview::Status
        | crate::state::RepoSubview::Compare
        | crate::state::RepoSubview::Rebase => "detail panel",
    }
}

fn diff_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    if matches!(
        repo_mode.active_subview,
        crate::state::RepoSubview::Branches | crate::state::RepoSubview::Commits
    ) {
        if let Some(target) = selected_comparison_target(repo_mode) {
            let label = comparison_target_menu_label(&target);
            let same_source = repo_mode.comparison_source == Some(repo_mode.active_subview);
            if repo_mode.comparison_base.is_none() || !same_source {
                entries.push(MenuEntry {
                    label: format!("Mark selected {label} as comparison base"),
                    action: Action::ToggleComparisonSelection,
                });
            } else if repo_mode.comparison_base.as_ref() != Some(&target)
                || repo_mode.comparison_target.as_ref() != Some(&target)
            {
                entries.push(MenuEntry {
                    label: format!("Compare current base against selected {label}"),
                    action: Action::ToggleComparisonSelection,
                });
            }
        }
    }

    if repo_mode.comparison_base.is_some() && repo_mode.comparison_target.is_some() {
        if repo_mode.active_subview != crate::state::RepoSubview::Compare {
            entries.push(MenuEntry {
                label: "Open comparison diff".to_string(),
                action: Action::SwitchRepoSubview(crate::state::RepoSubview::Compare),
            });
        }
        if matches!(
            repo_mode.active_subview,
            crate::state::RepoSubview::Branches
                | crate::state::RepoSubview::Commits
                | crate::state::RepoSubview::Compare
        ) {
            entries.push(MenuEntry {
                label: "Clear comparison".to_string(),
                action: Action::ClearComparison,
            });
        }
    }

    entries.push(MenuEntry {
        label: format!(
            "{} whitespace changes in diff",
            if repo_mode.ignore_whitespace_in_diff {
                "Show"
            } else {
                "Ignore"
            }
        ),
        action: Action::ToggleWhitespaceInDiff,
    });
    entries.push(MenuEntry {
        label: format!(
            "Increase diff context (currently {} line{})",
            repo_mode.diff_context_lines,
            if repo_mode.diff_context_lines == 1 {
                ""
            } else {
                "s"
            }
        ),
        action: Action::IncreaseDiffContext,
    });
    entries.push(MenuEntry {
        label: format!(
            "Decrease diff context (currently {} line{})",
            repo_mode.diff_context_lines,
            if repo_mode.diff_context_lines == 1 {
                ""
            } else {
                "s"
            }
        ),
        action: Action::DecreaseDiffContext,
    });
    entries.push(MenuEntry {
        label: format!(
            "Increase rename similarity threshold (currently {}%)",
            repo_mode.rename_similarity_threshold
        ),
        action: Action::IncreaseRenameSimilarityThreshold,
    });
    entries.push(MenuEntry {
        label: format!(
            "Decrease rename similarity threshold (currently {}%)",
            repo_mode.rename_similarity_threshold
        ),
        action: Action::DecreaseRenameSimilarityThreshold,
    });

    entries
}

fn commit_log_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };

    if repo_mode.active_subview != crate::state::RepoSubview::Commits
        || repo_mode.commit_subview_mode != crate::state::CommitSubviewMode::History
    {
        return Vec::new();
    }

    vec![
        MenuEntry {
            label: "Show current branch history".to_string(),
            action: Action::SwitchRepoSubview(crate::state::RepoSubview::Commits),
        },
        MenuEntry {
            label: "Show whole git graph (newest first)".to_string(),
            action: Action::OpenAllBranchGraph { reverse: false },
        },
        MenuEntry {
            label: "Show whole git graph (oldest first)".to_string(),
            action: Action::OpenAllBranchGraph { reverse: true },
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitClipboardTarget {
    ShortHash,
    FullHash,
    Summary,
    Diff,
    BrowserUrl,
}

fn comparison_target_menu_label(target: &ComparisonTarget) -> String {
    match target {
        ComparisonTarget::Branch(name) => format!("branch '{name}'"),
        ComparisonTarget::Commit(oid) => {
            let short = oid.chars().take(8).collect::<String>();
            format!("commit {short}")
        }
    }
}

fn merge_rebase_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return entries;
    };

    if let Some(source_label) = selected_merge_target(state)
        .ok()
        .flatten()
        .map(|(_, source_label)| source_label)
    {
        entries.extend(merge_rebase_branch_menu_entries(repo_mode, &source_label));
    }

    if let Some(detail) = repo_mode.detail.as_ref() {
        if repo_detail_has_rebase(detail) {
            entries.extend([
                MenuEntry {
                    label: "Continue active rebase".to_string(),
                    action: Action::ContinueRebase,
                },
                MenuEntry {
                    label: "Skip current rebase step".to_string(),
                    action: Action::SkipRebase,
                },
                MenuEntry {
                    label: "Abort active rebase".to_string(),
                    action: Action::AbortRebase,
                },
            ]);
        }

        if repo_mode.active_subview == crate::state::RepoSubview::Commits
            && repo_mode.commit_subview_mode == crate::state::CommitSubviewMode::History
        {
            entries.extend([
                MenuEntry {
                    label: "Interactive rebase from selected commit".to_string(),
                    action: Action::StartInteractiveRebase,
                },
                MenuEntry {
                    label: "Amend older commit at selection".to_string(),
                    action: Action::AmendSelectedCommit,
                },
                MenuEntry {
                    label: "Create fixup commit for selected commit".to_string(),
                    action: Action::CreateFixupCommit,
                },
                MenuEntry {
                    label: "Fixup onto selected commit".to_string(),
                    action: Action::FixupSelectedCommit,
                },
                MenuEntry {
                    label: "Apply pending fixup/squash commits".to_string(),
                    action: Action::ApplyFixupCommits,
                },
                MenuEntry {
                    label: "Squash selected commit into its parent".to_string(),
                    action: Action::SquashSelectedCommit,
                },
                MenuEntry {
                    label: "Drop selected commit".to_string(),
                    action: Action::DropSelectedCommit,
                },
                MenuEntry {
                    label: "Move selected commit up".to_string(),
                    action: Action::MoveSelectedCommitUp,
                },
                MenuEntry {
                    label: "Move selected commit down".to_string(),
                    action: Action::MoveSelectedCommitDown,
                },
                MenuEntry {
                    label: "Reword selected commit".to_string(),
                    action: Action::RewordSelectedCommit,
                },
                MenuEntry {
                    label: "Reword selected commit in editor".to_string(),
                    action: Action::RewordSelectedCommitWithEditor,
                },
            ]);
        }

        entries.push(MenuEntry {
            label: if detail.merge_state == MergeState::None {
                "Open rebase / merge panel".to_string()
            } else {
                "Open rebase / merge status panel".to_string()
            },
            action: Action::SwitchRepoSubview(crate::state::RepoSubview::Rebase),
        });
    }

    entries
}

fn bisect_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    if repo_mode.active_subview != crate::state::RepoSubview::Commits
        || repo_mode.commit_subview_mode != crate::state::CommitSubviewMode::History
    {
        return Vec::new();
    }
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    if history_action_block_reason(&detail.merge_state).is_some() {
        return Vec::new();
    }
    if selected_commit_entry(repo_mode).is_none() {
        return Vec::new();
    }

    if let Some(bisect) = detail.bisect_state.as_ref() {
        let target_label = if bisect.current_commit.is_some() {
            "current bisect commit"
        } else {
            "selected commit"
        };
        vec![
            MenuEntry {
                label: format!("Mark {target_label} as {}", bisect.bad_term),
                action: Action::MarkBisectBad,
            },
            MenuEntry {
                label: format!("Mark {target_label} as {}", bisect.good_term),
                action: Action::MarkBisectGood,
            },
            MenuEntry {
                label: format!("Skip {target_label}"),
                action: Action::SkipBisect,
            },
            MenuEntry {
                label: "Reset active bisect".to_string(),
                action: Action::ResetBisect,
            },
        ]
    } else {
        vec![
            MenuEntry {
                label: "Start bisect by marking selected commit as bad".to_string(),
                action: Action::StartBisectBad,
            },
            MenuEntry {
                label: "Start bisect by marking selected commit as good".to_string(),
                action: Action::StartBisectGood,
            },
        ]
    }
}

fn ignore_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    if state.repo_mode.as_ref().is_none() {
        return Vec::new();
    }
    vec![
        MenuEntry {
            label: "Add selected path to .gitignore".to_string(),
            action: Action::IgnoreSelectedStatusPath,
        },
        MenuEntry {
            label: "Add selected path to .git/info/exclude".to_string(),
            action: Action::ExcludeSelectedStatusPath,
        },
    ]
}

fn status_reset_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(tracking_branch) = repo_tracking_branch(state, &repo_mode.current_repo_id) else {
        return Vec::new();
    };
    vec![
        MenuEntry {
            label: format!("Soft reset to {tracking_branch}"),
            action: Action::SoftResetToUpstream,
        },
        MenuEntry {
            label: format!("Mixed reset to {tracking_branch}"),
            action: Action::MixedResetToUpstream,
        },
        MenuEntry {
            label: format!("Hard reset to {tracking_branch}"),
            action: Action::HardResetToUpstream,
        },
    ]
}

fn patch_menu_entries(state: &AppState) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return entries;
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return entries;
    };

    match detail.diff.presentation {
        DiffPresentation::Unstaged => {
            if selected_hunk_patch_job(state, PatchApplicationMode::Stage).is_some() {
                entries.push(MenuEntry {
                    label: "Stage selected hunk".to_string(),
                    action: Action::StageSelectedHunk,
                });
            }
            if matches!(
                selected_line_patch_job(state, PatchApplicationMode::Stage),
                Ok(Some(_))
            ) {
                entries.push(MenuEntry {
                    label: "Stage selected line range".to_string(),
                    action: Action::StageSelectedLines,
                });
            }
        }
        DiffPresentation::Staged => {
            if selected_hunk_patch_job(state, PatchApplicationMode::Unstage).is_some() {
                entries.push(MenuEntry {
                    label: "Unstage selected hunk".to_string(),
                    action: Action::UnstageSelectedHunk,
                });
            }
            if matches!(
                selected_line_patch_job(state, PatchApplicationMode::Unstage),
                Ok(Some(_))
            ) {
                entries.push(MenuEntry {
                    label: "Unstage selected line range".to_string(),
                    action: Action::UnstageSelectedLines,
                });
            }
        }
        DiffPresentation::Comparison => {}
    }

    entries
}

fn recent_repo_menu_repo_ids(state: &AppState) -> Vec<crate::state::RepoId> {
    let current_repo_id = state
        .repo_mode
        .as_ref()
        .map(|repo_mode| &repo_mode.current_repo_id)
        .or(state.workspace.selected_repo_id.as_ref());
    let mut repo_ids = Vec::new();
    for repo_id in state.recent_repo_stack.iter().rev() {
        if current_repo_id.is_some_and(|current| current == repo_id) {
            continue;
        }
        if (state.workspace.repo_summaries.contains_key(repo_id)
            || state
                .workspace
                .discovered_repo_ids
                .iter()
                .any(|candidate| candidate == repo_id))
            && !repo_ids.iter().any(|candidate| candidate == repo_id)
        {
            repo_ids.push(repo_id.clone());
        }
    }
    repo_ids
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

    let visible_indices = filtered_branch_indices(repo_mode);
    if visible_indices.is_empty() {
        repo_mode.branches_view.selected_index = None;
        return;
    }

    if let Some(index) = repo_mode
        .branches_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
    {
        repo_mode.branches_view.selected_index = Some(index);
        return;
    }

    let head_index = detail
        .branches
        .iter()
        .enumerate()
        .find_map(|(index, branch)| {
            (branch.is_head && visible_indices.contains(&index)).then_some(index)
        })
        .unwrap_or(visible_indices[0]);
    repo_mode.branches_view.selected_index = Some(head_index);
}

fn sync_remote_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.remotes_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_remote_indices(repo_mode);
    repo_mode.remotes_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.remotes_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_remote_branch_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.remote_branches_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_remote_branch_indices(repo_mode);
    repo_mode.remote_branches_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.remote_branches_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_tag_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.tags_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_tag_indices(repo_mode);
    repo_mode.tags_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.tags_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_commit_selection(repo_mode: &mut RepoModeState) {
    let Some(detail) = repo_mode.detail.as_ref() else {
        repo_mode.commits_view.selected_index = None;
        return;
    };

    let visible_indices = filtered_commit_indices(repo_mode);
    if let Some(pending_selection) = repo_mode.pending_commit_selection_oid.as_deref() {
        if let Some(index) = visible_indices.iter().copied().find(|index| {
            detail
                .commits
                .get(*index)
                .is_some_and(|commit| commit_matches_pending_selection(commit, pending_selection))
        }) {
            repo_mode.commits_view.selected_index = Some(index);
            repo_mode.pending_commit_selection_oid = None;
            return;
        }
    }
    repo_mode.commits_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.commits_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_commit_file_selection(repo_mode: &mut RepoModeState) {
    let Some(commit) = selected_commit_item(repo_mode) else {
        repo_mode.commit_files_view.selected_index = None;
        return;
    };

    let visible_indices = filtered_commit_file_indices(repo_mode);
    repo_mode.commit_files_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.commit_files_view.selected_index == Some(*index))
        .or_else(|| {
            if commit.changed_files.is_empty() {
                None
            } else {
                visible_indices.first().copied()
            }
        });
}

fn commit_file_diff_detail_active(repo_mode: &RepoModeState) -> bool {
    repo_mode.active_subview == crate::state::RepoSubview::Commits
        && repo_mode.commit_subview_mode == crate::state::CommitSubviewMode::Files
        && repo_mode.commit_files_mode == CommitFilesMode::Diff
}

fn sync_stash_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.stash_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_stash_indices(repo_mode);
    repo_mode.stash_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.stash_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_stash_file_selection(repo_mode: &mut RepoModeState) {
    let Some(stash) = selected_stash_item(repo_mode) else {
        repo_mode.stash_files_view.selected_index = None;
        return;
    };

    let visible_indices = filtered_stash_file_indices(repo_mode);
    repo_mode.stash_files_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.stash_files_view.selected_index == Some(*index))
        .or_else(|| {
            if stash.changed_files.is_empty() {
                None
            } else {
                visible_indices.first().copied()
            }
        });
}

fn sync_reflog_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.reflog_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_reflog_indices(repo_mode);
    repo_mode.reflog_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.reflog_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_worktree_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.worktree_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_worktree_indices(repo_mode);
    repo_mode.worktree_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.worktree_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_submodule_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.submodules_view.selected_index = None;
        return;
    }

    let visible_indices = filtered_submodule_indices(repo_mode);
    repo_mode.submodules_view.selected_index = visible_indices
        .iter()
        .copied()
        .find(|index| repo_mode.submodules_view.selected_index == Some(*index))
        .or_else(|| visible_indices.first().copied());
}

fn sync_status_selection(repo_mode: &mut RepoModeState) {
    if repo_mode.detail.is_none() {
        repo_mode.status_view.selected_index = None;
        repo_mode.staged_view.selected_index = None;
        return;
    }

    let unstaged_len = status_entries_len(repo_mode, PaneId::RepoUnstaged);
    repo_mode.status_view.ensure_selection(unstaged_len);

    let staged_len = status_entries_len(repo_mode, PaneId::RepoStaged);
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

fn status_entries_len(repo_mode: &RepoModeState, pane: PaneId) -> usize {
    crate::state::visible_status_entries(repo_mode, pane).len()
}

fn selected_status_entry(
    repo_mode: &RepoModeState,
    pane: PaneId,
) -> Option<crate::state::VisibleStatusEntry> {
    let entries = crate::state::visible_status_entries(repo_mode, pane);
    let selected_index = match pane {
        PaneId::RepoUnstaged => repo_mode.status_view.selected_index,
        PaneId::RepoStaged => repo_mode.staged_view.selected_index,
        _ => None,
    }
    .filter(|index| *index < entries.len())
    .unwrap_or(0);

    entries.get(selected_index).cloned()
}

fn selected_status_display_path(
    repo_mode: &RepoModeState,
    pane: PaneId,
) -> Option<std::path::PathBuf> {
    selected_status_entry(repo_mode, pane).map(|entry| entry.path)
}

fn selected_status_path(repo_mode: &RepoModeState, pane: PaneId) -> Option<std::path::PathBuf> {
    selected_status_entry(repo_mode, pane)
        .filter(|entry| entry.is_file())
        .map(|entry| entry.path)
}

fn selected_status_detail_request(
    repo_mode: &RepoModeState,
    pane: PaneId,
) -> Option<(std::path::PathBuf, DiffPresentation)> {
    selected_status_path(repo_mode, pane).map(|path| (path, diff_presentation_for_pane(pane)))
}

fn enqueue_selected_status_detail_load(
    repo_mode: &RepoModeState,
    pane: PaneId,
    effects: &mut Vec<Effect>,
) {
    if let Some((selected_path, diff_presentation)) =
        selected_status_detail_request(repo_mode, pane)
    {
        effects.push(Effect::LoadRepoDetail {
            repo_id: repo_mode.current_repo_id.clone(),
            selected_path: Some(selected_path),
            diff_presentation,
            commit_ref: None,
            commit_history_mode: CommitHistoryMode::Linear,
            ignore_whitespace_in_diff: repo_mode.ignore_whitespace_in_diff,
            diff_context_lines: repo_mode.diff_context_lines,
            rename_similarity_threshold: repo_mode.rename_similarity_threshold,
        });
    }
}

fn update_status_directory_collapse(
    repo_mode: &mut RepoModeState,
    pane: PaneId,
    collapse: bool,
) -> bool {
    let Some(entry) = selected_status_entry(repo_mode, pane) else {
        return false;
    };
    let directory = if entry.is_directory() {
        Some(entry.path)
    } else {
        entry.path.parent().map(std::path::Path::to_path_buf)
    };
    let Some(directory) = directory else {
        return false;
    };
    update_status_directory_for_path(repo_mode, directory, collapse)
}

fn update_status_directory_for_path(
    repo_mode: &mut RepoModeState,
    directory: std::path::PathBuf,
    collapse: bool,
) -> bool {
    if directory.as_os_str().is_empty() {
        return false;
    }
    if collapse {
        repo_mode.collapsed_status_dirs.insert(directory)
    } else {
        repo_mode.collapsed_status_dirs.remove(&directory)
    }
}

fn selected_repo_shell_target(
    state: &AppState,
    allow_directories: bool,
) -> Result<
    Option<(
        crate::state::RepoId,
        std::path::PathBuf,
        bool,
        std::path::PathBuf,
    )>,
    String,
> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let (relative_path, is_directory) = match state.focused_pane {
        PaneId::RepoUnstaged | PaneId::RepoStaged | PaneId::RepoDetail
            if repo_mode.active_subview == crate::state::RepoSubview::Status =>
        {
            let Some(entry) = selected_status_entry(repo_mode, state.focused_pane) else {
                return Err("Select a file or directory before using that action.".to_string());
            };
            (entry.path.clone(), entry.is_directory())
        }
        PaneId::RepoDetail
            if repo_mode.active_subview == crate::state::RepoSubview::Commits
                && repo_mode.commit_subview_mode == crate::state::CommitSubviewMode::Files
                && repo_mode.commit_files_mode == crate::state::CommitFilesMode::List =>
        {
            let Some(file) = selected_commit_file_item(repo_mode) else {
                return Err("Select a file before using that action.".to_string());
            };
            (file.path.clone(), false)
        }
        _ => return Ok(None),
    };
    if !allow_directories && is_directory {
        return Err("Select a file before using that action.".to_string());
    }
    let repo_root = repo_root_for_id(state, &repo_mode.current_repo_id);
    let absolute_path = if relative_path.is_absolute() {
        relative_path.clone()
    } else {
        repo_root.join(&relative_path)
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        absolute_path,
        is_directory,
        relative_path,
    )))
}

fn selected_status_ignore_target(
    state: &AppState,
    exclude_only: bool,
) -> Result<Option<(crate::state::RepoId, std::path::PathBuf, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(entry) = selected_status_entry(repo_mode, state.focused_pane) else {
        return Err("Select a file or directory before ignoring it.".to_string());
    };
    let ignored_path = status_clipboard_path(&entry.path, entry.is_directory());
    let target_file = if exclude_only {
        ".git/info/exclude"
    } else {
        ".gitignore"
    };
    let command = format!(
        "mkdir -p .git/info && touch {target_file} && grep -Fqx -- {entry} {target_file} || printf '%s\\n' {entry} >> {target_file}",
        entry = shell_quote(ignored_path.as_os_str()),
    );
    let summary = if exclude_only {
        format!("Exclude {}", ignored_path.display())
    } else {
        format!("Ignore {}", ignored_path.display())
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        ignored_path,
        command,
        summary,
    )))
}

fn status_clipboard_path(path: &std::path::Path, is_directory: bool) -> std::path::PathBuf {
    if !is_directory {
        return path.to_path_buf();
    }
    let mut display_path = path.to_path_buf();
    display_path.push("");
    display_path
}

fn clipboard_shell_command(value: &std::ffi::OsStr, os: &crate::state::OsConfigSnapshot) -> String {
    let command = if os.copy_to_clipboard_cmd.is_empty() {
        let quoted = shell_quote(value);
        format!(
            "printf '%s' {quoted} | if command -v wl-copy >/dev/null 2>&1; then wl-copy; elif command -v xclip >/dev/null 2>&1; then xclip -selection clipboard; elif command -v pbcopy >/dev/null 2>&1; then pbcopy; else exit 1; fi"
        )
    } else {
        resolve_os_command_template(&os.copy_to_clipboard_cmd, "text", value)
    };

    shell_command_with_functions_file(command, os)
}

#[derive(Clone, Copy)]
enum OsCommandTemplateKind {
    OpenFile,
    OpenLink,
}

fn open_in_default_app_command(
    target: &std::ffi::OsStr,
    os: &crate::state::OsConfigSnapshot,
    kind: OsCommandTemplateKind,
) -> String {
    let command = match kind {
        OsCommandTemplateKind::OpenFile if !os.open.is_empty() => {
            resolve_os_command_template(&os.open, "filename", target)
        }
        OsCommandTemplateKind::OpenLink if !os.open_link.is_empty() => {
            resolve_os_command_template(&os.open_link, "link", target)
        }
        _ => {
            let quoted = shell_quote(target);
            format!(
                "if command -v xdg-open >/dev/null 2>&1; then xdg-open {quoted}; elif command -v open >/dev/null 2>&1; then open {quoted}; else exit 1; fi >/dev/null 2>&1"
            )
        }
    };

    shell_command_with_functions_file(command, os)
}

fn resolve_os_command_template(
    template: &str,
    placeholder: &str,
    value: &std::ffi::OsStr,
) -> String {
    let quoted = shell_quote(value);
    template
        .replace(&format!("{{{{{placeholder}}}}}"), &quoted)
        .replace(&format!("{{{{.{placeholder}}}}}"), &quoted)
}

fn shell_command_with_functions_file(
    command: String,
    os: &crate::state::OsConfigSnapshot,
) -> String {
    if os.shell_functions_file.is_empty() {
        return command;
    }

    #[cfg(windows)]
    {
        command
    }

    #[cfg(not(windows))]
    {
        let shell_file = shell_quote(std::ffi::OsStr::new(&os.shell_functions_file));
        format!(". {shell_file}\n{command}")
    }
}

fn selected_config_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, std::path::PathBuf, std::path::PathBuf)>, String> {
    let Some(config_path) = state.config_path.clone() else {
        return Err("No config file is loaded; built-in defaults are active.".to_string());
    };
    let Some(repo_id) = active_repo_id(state) else {
        return Ok(None);
    };
    let repo_root = repo_root_for_id(state, &repo_id);
    Ok(Some((repo_id, repo_root, config_path)))
}

fn selected_update_check_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repository_url) = state.repository_url.as_deref() else {
        return Err("No repository URL is configured for update checks.".to_string());
    };
    let Some(repo_id) = active_repo_id(state) else {
        return Ok(None);
    };
    Ok(Some((repo_id, repository_release_url(repository_url))))
}

fn repository_release_url(repository_url: &str) -> String {
    let repository_url = repository_url.trim().trim_end_matches('/');
    let repository_url = repository_url
        .strip_suffix(".git")
        .unwrap_or(repository_url);
    format!("{repository_url}/releases")
}

fn selected_commit_browser_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(commit) = selected_commit_item(repo_mode) else {
        return Err("Select a commit before opening it in the browser.".to_string());
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Err("Load repository details before opening a commit in the browser.".to_string());
    };
    let Some(target) = selected_commit_browser_url(state, detail, &commit.oid) else {
        return Err("No browser-compatible remote URL found for the selected commit.".to_string());
    };
    Ok(Some((repo_mode.current_repo_id.clone(), target)))
}

fn selected_commit_clipboard_target(
    state: &AppState,
    target: CommitClipboardTarget,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(commit) = selected_commit_item(repo_mode) else {
        return Err("Select a commit before copying a commit attribute.".to_string());
    };
    let commit_label = format!(
        "{} {}",
        if commit.short_oid.is_empty() {
            commit.oid.chars().take(8).collect::<String>()
        } else {
            commit.short_oid.clone()
        },
        commit.summary
    );

    let (value, summary) = match target {
        CommitClipboardTarget::ShortHash => {
            let value = if commit.short_oid.is_empty() {
                commit.oid.clone()
            } else {
                commit.short_oid.clone()
            };
            let summary = format!("Copy {value}");
            (value, summary)
        }
        CommitClipboardTarget::FullHash => {
            let value = commit.oid.clone();
            let summary = format!("Copy full hash for {commit_label}");
            (value, summary)
        }
        CommitClipboardTarget::Summary => {
            let value = commit.summary.clone();
            let summary = format!("Copy summary for {commit_label}");
            (value, summary)
        }
        CommitClipboardTarget::Diff => {
            if commit.diff.lines.is_empty() {
                return Err("No diff is loaded for the selected commit.".to_string());
            }
            let value = commit
                .diff
                .lines
                .iter()
                .map(|line| line.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let summary = format!("Copy diff for {commit_label}");
            (value, summary)
        }
        CommitClipboardTarget::BrowserUrl => {
            let Some(detail) = repo_mode.detail.as_ref() else {
                return Err(
                    "Load repository details before copying the commit browser URL.".to_string(),
                );
            };
            let Some(value) = selected_commit_browser_url(state, detail, &commit.oid) else {
                return Err(
                    "No browser-compatible remote URL found for the selected commit.".to_string(),
                );
            };
            let summary = format!("Copy browser URL for {commit_label}");
            (value, summary)
        }
    };

    Ok(Some((repo_mode.current_repo_id.clone(), value, summary)))
}

fn selected_commit_browser_url(
    state: &AppState,
    detail: &crate::state::RepoDetail,
    commit_oid: &str,
) -> Option<String> {
    detail
        .remotes
        .iter()
        .filter(|remote| remote.name == "origin")
        .chain(
            detail
                .remotes
                .iter()
                .filter(|remote| remote.name != "origin"),
        )
        .find_map(|remote| {
            hosting_service::commit_browser_url_for_remote(
                &remote.fetch_url,
                commit_oid,
                &state.service_domains,
                &mut Vec::new(),
            )
            .ok()
            .or_else(|| {
                hosting_service::commit_browser_url_for_remote(
                    &remote.push_url,
                    commit_oid,
                    &state.service_domains,
                    &mut Vec::new(),
                )
                .ok()
            })
        })
}

fn external_difftool_command(path: &std::path::Path, pane: PaneId) -> String {
    let quoted = shell_quote(path.as_os_str());
    let cached = if pane == PaneId::RepoStaged {
        "--cached "
    } else {
        ""
    };
    format!("git difftool {cached}--no-prompt -- {quoted}")
}

fn shell_quote(value: &std::ffi::OsStr) -> String {
    let text = value.to_string_lossy();
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

fn active_repo_id(state: &AppState) -> Option<crate::state::RepoId> {
    state
        .repo_mode
        .as_ref()
        .map(|repo_mode| repo_mode.current_repo_id.clone())
        .or_else(|| state.workspace.selected_repo_id.clone())
}

fn repo_tracking_branch(state: &AppState, repo_id: &crate::state::RepoId) -> Option<String> {
    state
        .workspace
        .repo_summaries
        .get(repo_id)
        .and_then(|summary| summary.remote_summary.tracking_branch.clone())
}

fn open_upstream_reset_confirmation(state: &mut AppState, mode: ResetMode) -> bool {
    let Some(repo_id) = state
        .repo_mode
        .as_ref()
        .map(|repo_mode| repo_mode.current_repo_id.clone())
    else {
        return false;
    };
    let Some(tracking_branch) = repo_tracking_branch(state, &repo_id) else {
        push_warning(state, "Current branch has no upstream to reset against.");
        return false;
    };
    open_confirmation_modal(
        state,
        repo_id,
        ConfirmableOperation::ResetToCommit {
            mode,
            commit: "@{upstream}".to_string(),
            summary: tracking_branch,
        },
    );
    true
}

fn sync_repo_subview_selection(repo_mode: &mut RepoModeState, subview: crate::state::RepoSubview) {
    match subview {
        crate::state::RepoSubview::Branches => sync_branch_selection(repo_mode),
        crate::state::RepoSubview::Remotes => sync_remote_selection(repo_mode),
        crate::state::RepoSubview::RemoteBranches => sync_remote_branch_selection(repo_mode),
        crate::state::RepoSubview::Tags => sync_tag_selection(repo_mode),
        crate::state::RepoSubview::Commits => match repo_mode.commit_subview_mode {
            crate::state::CommitSubviewMode::History => sync_commit_selection(repo_mode),
            crate::state::CommitSubviewMode::Files => sync_commit_file_selection(repo_mode),
        },
        crate::state::RepoSubview::Stash => match repo_mode.stash_subview_mode {
            crate::state::StashSubviewMode::List => sync_stash_selection(repo_mode),
            crate::state::StashSubviewMode::Files => sync_stash_file_selection(repo_mode),
        },
        crate::state::RepoSubview::Reflog => sync_reflog_selection(repo_mode),
        crate::state::RepoSubview::Worktrees => sync_worktree_selection(repo_mode),
        crate::state::RepoSubview::Submodules => sync_submodule_selection(repo_mode),
        crate::state::RepoSubview::Status => sync_status_selection(repo_mode),
        crate::state::RepoSubview::Compare | crate::state::RepoSubview::Rebase => {}
    }
}

fn adjacent_repo_subview(
    current: crate::state::RepoSubview,
    step: isize,
) -> crate::state::RepoSubview {
    const ORDER: [crate::state::RepoSubview; 12] = [
        crate::state::RepoSubview::Status,
        crate::state::RepoSubview::Branches,
        crate::state::RepoSubview::Remotes,
        crate::state::RepoSubview::RemoteBranches,
        crate::state::RepoSubview::Tags,
        crate::state::RepoSubview::Commits,
        crate::state::RepoSubview::Compare,
        crate::state::RepoSubview::Rebase,
        crate::state::RepoSubview::Stash,
        crate::state::RepoSubview::Reflog,
        crate::state::RepoSubview::Worktrees,
        crate::state::RepoSubview::Submodules,
    ];

    let current_index = ORDER
        .iter()
        .position(|subview| *subview == current)
        .unwrap_or(0);
    let next_index = (current_index as isize + step).rem_euclid(ORDER.len() as isize) as usize;
    ORDER[next_index]
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
                return Err(
                    "Select a file or directory before opening it in the editor.".to_string(),
                );
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
        PaneId::RepoUnstaged => selected_status_display_path(repo_mode, PaneId::RepoUnstaged),
        PaneId::RepoStaged => selected_status_display_path(repo_mode, PaneId::RepoStaged),
        PaneId::RepoDetail if repo_mode.active_subview == crate::state::RepoSubview::Status => {
            repo_mode
                .detail
                .as_ref()
                .and_then(|detail| detail.diff.selected_path.clone())
        }
        PaneId::RepoDetail if repo_mode.active_subview == crate::state::RepoSubview::Worktrees => {
            selected_worktree_item(repo_mode).map(|worktree| worktree.path.clone())
        }
        PaneId::RepoDetail if repo_mode.active_subview == crate::state::RepoSubview::Submodules => {
            selected_submodule_item(repo_mode).map(|submodule| submodule.path.clone())
        }
        PaneId::RepoDetail
            if repo_mode.active_subview == crate::state::RepoSubview::Commits
                && repo_mode.commit_subview_mode == crate::state::CommitSubviewMode::Files =>
        {
            selected_commit_file_item(repo_mode).map(|file| file.path.clone())
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
        .filter(|index| filtered_branch_indices(repo_mode).contains(index))
        .or_else(|| {
            detail
                .branches
                .iter()
                .enumerate()
                .find_map(|(index, branch)| {
                    (branch.is_head && filtered_branch_indices(repo_mode).contains(&index))
                        .then_some(index)
                })
        })
        .or_else(|| filtered_branch_indices(repo_mode).first().copied())?;
    detail.branches.get(selected_index)
}

fn selected_remote_item(repo_mode: &RepoModeState) -> Option<&crate::state::RemoteItem> {
    let detail = repo_mode.detail.as_ref()?;
    let visible_indices = filtered_remote_indices(repo_mode);
    let selected_index = repo_mode
        .remotes_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.remotes.get(selected_index)
}

fn selected_remote_branch_item(
    repo_mode: &RepoModeState,
) -> Option<&crate::state::RemoteBranchItem> {
    let detail = repo_mode.detail.as_ref()?;
    let visible_indices = filtered_remote_branch_indices(repo_mode);
    let selected_index = repo_mode
        .remote_branches_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.remote_branches.get(selected_index)
}

fn selected_branch_pull_request_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Err("Select a branch before opening pull request options.".to_string());
    };
    let Some(url) = detail
        .remotes
        .iter()
        .filter(|remote| remote.name == "origin")
        .chain(
            detail
                .remotes
                .iter()
                .filter(|remote| remote.name != "origin"),
        )
        .find_map(|remote| {
            hosting_service::pull_request_url_for_remote(
                &remote.fetch_url,
                &branch.name,
                None,
                &state.service_domains,
                &mut Vec::new(),
            )
            .ok()
            .or_else(|| {
                hosting_service::pull_request_url_for_remote(
                    &remote.push_url,
                    &branch.name,
                    None,
                    &state.service_domains,
                    &mut Vec::new(),
                )
                .ok()
            })
        })
    else {
        return Err("No browser-compatible remote URL found for the selected branch.".to_string());
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        url,
        branch.name.clone(),
    )))
}

fn selected_remote_branch_pull_request_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_remote_branch_item(repo_mode) else {
        return Err("Select a remote branch before opening pull request options.".to_string());
    };
    let Some(url) = detail
        .remotes
        .iter()
        .filter(|remote| remote.name == branch.remote_name)
        .chain(
            detail
                .remotes
                .iter()
                .filter(|remote| remote.name != branch.remote_name),
        )
        .find_map(|remote| {
            hosting_service::pull_request_url_for_remote(
                &remote.fetch_url,
                &branch.branch_name,
                None,
                &state.service_domains,
                &mut Vec::new(),
            )
            .ok()
            .or_else(|| {
                hosting_service::pull_request_url_for_remote(
                    &remote.push_url,
                    &branch.branch_name,
                    None,
                    &state.service_domains,
                    &mut Vec::new(),
                )
                .ok()
            })
        })
    else {
        return Err(
            "No browser-compatible remote URL found for the selected remote branch.".to_string(),
        );
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        url,
        branch.name.clone(),
    )))
}

fn current_branch_item(repo_mode: &RepoModeState) -> Option<&crate::state::BranchItem> {
    repo_mode
        .detail
        .as_ref()?
        .branches
        .iter()
        .find(|branch| branch.is_head)
}

fn selected_branch_upstream_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Err("Select a branch before unsetting its upstream.".to_string());
    };
    if branch.upstream.is_none() {
        return Err(format!("Branch {} does not have an upstream.", branch.name));
    }
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        branch.name.clone(),
    )))
}

fn selected_branch_fast_forward_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Err("Select a branch before fast-forwarding it.".to_string());
    };
    if !branch.is_head {
        return Err(format!(
            "Checkout {} before fast-forwarding it from upstream.",
            branch.name
        ));
    }
    let Some(upstream_ref) = branch.upstream.clone() else {
        return Err(format!("Branch {} does not have an upstream.", branch.name));
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        branch.name.clone(),
        upstream_ref,
    )))
}

fn selected_non_head_branch_ref(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_branch_item(repo_mode) else {
        return Err("Select a branch before using it as the target ref.".to_string());
    };
    if branch.is_head {
        return Err("Select a non-current branch for that action.".to_string());
    }
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        branch.name.clone(),
    )))
}

fn selected_merge_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    match repo_mode.active_subview {
        crate::state::RepoSubview::Branches => selected_non_head_branch_ref(state),
        crate::state::RepoSubview::RemoteBranches => selected_remote_branch_ref(state),
        _ => Err("Select a branch or remote branch before merging it.".to_string()),
    }
}

fn selected_remote_branch_ref(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(branch) = selected_remote_branch_item(repo_mode) else {
        return Err("Select a remote branch before using it as the target ref.".to_string());
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        branch.name.clone(),
    )))
}

fn open_merge_confirmation(
    state: &mut AppState,
    repo_id: crate::state::RepoId,
    target_ref: String,
    variant: MergeVariant,
    effects: &mut Vec<Effect>,
) {
    open_confirmation_modal(
        state,
        repo_id,
        ConfirmableOperation::MergeRefIntoCurrent {
            source_label: target_ref.clone(),
            target_ref,
            variant,
        },
    );
    effects.push(Effect::ScheduleRender);
}

fn selected_remote_branch_upstream_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, String)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(current_branch) = current_branch_item(repo_mode) else {
        return Err("Attach HEAD to a local branch before setting an upstream.".to_string());
    };
    let Some(remote_branch) = selected_remote_branch_item(repo_mode) else {
        return Err("Select a remote branch before setting it as upstream.".to_string());
    };
    if current_branch.upstream.as_deref() == Some(remote_branch.name.as_str()) {
        return Err(format!(
            "Branch {} already tracks {}.",
            current_branch.name, remote_branch.name
        ));
    }
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        current_branch.name.clone(),
        remote_branch.name.clone(),
    )))
}

fn selected_tag_item(repo_mode: &RepoModeState) -> Option<&crate::state::TagItem> {
    let detail = repo_mode.detail.as_ref()?;
    let visible_indices = filtered_tag_indices(repo_mode);
    let selected_index = repo_mode
        .tags_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.tags.get(selected_index)
}

fn selected_commit_entry(repo_mode: &RepoModeState) -> Option<(usize, &crate::state::CommitItem)> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .commits_view
        .selected_index
        .filter(|index| filtered_commit_indices(repo_mode).contains(index))
        .or_else(|| filtered_commit_indices(repo_mode).first().copied())?;
    detail
        .commits
        .get(selected_index)
        .map(|commit| (selected_index, commit))
}

fn selected_commit_item(repo_mode: &RepoModeState) -> Option<&crate::state::CommitItem> {
    selected_commit_entry(repo_mode).map(|(_, commit)| commit)
}

fn selected_reflog_entry(repo_mode: &RepoModeState) -> Option<(usize, &crate::state::ReflogItem)> {
    let detail = repo_mode.detail.as_ref()?;
    let selected_index = repo_mode
        .reflog_view
        .selected_index
        .filter(|index| filtered_reflog_indices(repo_mode).contains(index))
        .or_else(|| filtered_reflog_indices(repo_mode).first().copied())?;
    detail
        .reflog_items
        .get(selected_index)
        .map(|entry| (selected_index, entry))
}

fn pending_reflog_commit_selection(entry: &crate::state::ReflogItem) -> String {
    format!("{} {} {}", entry.oid, entry.unix_timestamp, entry.summary)
}

fn commit_matches_pending_selection(
    commit: &crate::state::CommitItem,
    pending_selection: &str,
) -> bool {
    let parts = pending_selection.splitn(3, ' ').collect::<Vec<_>>();
    match parts.as_slice() {
        [oid] => commit.oid == *oid,
        [oid, unix_timestamp, summary] => {
            commit.oid == *oid
                && commit.unix_timestamp.to_string() == *unix_timestamp
                && commit.summary == *summary
        }
        _ => false,
    }
}

fn selected_commit_file_item(repo_mode: &RepoModeState) -> Option<&crate::state::CommitFileItem> {
    let commit = selected_commit_item(repo_mode)?;
    let visible_indices = filtered_commit_file_indices(repo_mode);
    let selected_index = repo_mode
        .commit_files_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    commit.changed_files.get(selected_index)
}

fn load_selected_commit_file_diff_effect(
    repo_mode: &RepoModeState,
) -> Option<(std::path::PathBuf, Effect)> {
    let commit = selected_commit_item(repo_mode)?;
    let file = selected_commit_file_item(repo_mode)?;
    let selected_path = file.path.clone();
    Some((
        selected_path.clone(),
        Effect::LoadRepoDiff {
            repo_id: repo_mode.current_repo_id.clone(),
            comparison_target: Some(ComparisonTarget::Commit(format!("{}^!", commit.oid))),
            compare_with: None,
            selected_path: Some(selected_path),
            diff_presentation: DiffPresentation::Comparison,
            ignore_whitespace_in_diff: repo_mode.ignore_whitespace_in_diff,
            diff_context_lines: repo_mode.diff_context_lines,
            rename_similarity_threshold: repo_mode.rename_similarity_threshold,
        },
    ))
}

fn selected_commit_file_checkout_target(
    state: &AppState,
) -> Result<Option<(crate::state::RepoId, String, std::path::PathBuf)>, String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Ok(None);
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Ok(None);
    };
    if let Some(message) = history_action_block_reason(&detail.merge_state) {
        return Err(message.to_string());
    }
    let Some(commit) = selected_commit_item(repo_mode) else {
        return Ok(None);
    };
    let Some(file) = selected_commit_file_item(repo_mode) else {
        return Ok(None);
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        commit.oid.clone(),
        file.path.clone(),
    )))
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

    let Some((selected_index, entry)) = selected_reflog_entry(repo_mode) else {
        return Ok(None);
    };
    if selected_index == 0 {
        return Err("Select an older reflog entry to restore.".to_string());
    }
    if entry.selector.is_empty() {
        return Err("Selected reflog entry could not be parsed.".to_string());
    };

    Ok(Some((
        repo_mode.current_repo_id.clone(),
        entry.selector.clone(),
        entry.description.clone(),
    )))
}

fn reflog_commit_label(entry: &crate::state::ReflogItem) -> String {
    match (!entry.short_oid.is_empty(), !entry.summary.is_empty()) {
        (true, true) => format!("{} {}", entry.short_oid, entry.summary),
        (true, false) => entry.short_oid.clone(),
        (false, true) => entry.summary.clone(),
        (false, false) if !entry.oid.is_empty() => entry.oid.clone(),
        _ => entry.description.clone(),
    }
}

fn pending_checkoutable_history_target(
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

    match repo_mode.active_subview {
        crate::state::RepoSubview::Branches => {
            let Some(branch) = selected_branch_item(repo_mode) else {
                return Ok(None);
            };
            Ok(Some((
                repo_mode.current_repo_id.clone(),
                branch.name.clone(),
                format!("branch {}", branch.name),
            )))
        }
        crate::state::RepoSubview::RemoteBranches => {
            let Some(branch) = selected_remote_branch_item(repo_mode) else {
                return Ok(None);
            };
            Ok(Some((
                repo_mode.current_repo_id.clone(),
                branch.name.clone(),
                format!("remote branch {}", branch.name),
            )))
        }
        crate::state::RepoSubview::Reflog => {
            let Some((_, entry)) = selected_reflog_entry(repo_mode) else {
                return Ok(None);
            };
            if entry.oid.is_empty() {
                return Err(
                    "Select a reflog entry that still points to a commit before using history actions."
                        .to_string(),
                );
            }
            Ok(Some((
                repo_mode.current_repo_id.clone(),
                entry.oid.clone(),
                reflog_commit_label(entry),
            )))
        }
        _ => pending_history_commit_operation(state, |_, commit, _| {
            Ok((
                commit.oid.clone(),
                format!("{} {}", commit.short_oid, commit.summary),
            ))
        })
        .map(|result| result.map(|(repo_id, (commit, summary))| (repo_id, commit, summary))),
    }
}

fn pending_reset_history_target(
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

    match repo_mode.active_subview {
        crate::state::RepoSubview::Reflog => {
            let Some((_, entry)) = selected_reflog_entry(repo_mode) else {
                return Ok(None);
            };
            if entry.selector.is_empty() {
                return Err("Selected reflog entry could not be parsed.".to_string());
            }
            Ok(Some((
                repo_mode.current_repo_id.clone(),
                entry.selector.clone(),
                entry.description.clone(),
            )))
        }
        _ => pending_history_commit_operation(state, |_, commit, _| {
            Ok((
                commit.oid.clone(),
                format!("{} {}", commit.short_oid, commit.summary),
            ))
        })
        .map(|result| result.map(|(repo_id, (commit, summary))| (repo_id, commit, summary))),
    }
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
    let Some((selected_index, commit)) = selected_commit_entry(repo_mode) else {
        return Ok(None);
    };
    Ok(Some((
        repo_mode.current_repo_id.clone(),
        build(detail, commit, selected_index)?,
    )))
}

fn pending_bisect_target<T, F>(
    state: &AppState,
    build: F,
) -> Result<Option<(crate::state::RepoId, T)>, String>
where
    F: FnOnce(&crate::state::RepoDetail, String, String) -> Result<T, String>,
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
    let Some(bisect_state) = detail.bisect_state.as_ref() else {
        return Err("No bisect is currently in progress.".to_string());
    };

    let target = if let Some(current_commit) = bisect_state.current_commit.clone() {
        let summary = bisect_state
            .current_summary
            .as_ref()
            .map(|summary| format!("{} {}", short_oid_label(&current_commit), summary))
            .unwrap_or_else(|| short_oid_label(&current_commit));
        Some((current_commit, summary))
    } else {
        selected_commit_entry(repo_mode).map(|(_, commit)| {
            (
                commit.oid.clone(),
                format!("{} {}", commit.short_oid, commit.summary),
            )
        })
    };

    let Some((commit, summary)) = target else {
        return Ok(None);
    };

    Ok(Some((
        repo_mode.current_repo_id.clone(),
        build(detail, commit, summary)?,
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

fn short_oid_label(oid: &str) -> String {
    oid.chars().take(7).collect()
}

fn open_reset_confirmation(state: &mut AppState, mode: ResetMode) -> Result<bool, String> {
    let Some((repo_id, commit, summary)) = pending_reset_history_target(state)? else {
        return Ok(false);
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
    Ok(true)
}

fn open_tag_reset_confirmation(state: &mut AppState, mode: ResetMode) -> bool {
    let Some((repo_id, tag_name, summary)) = state.repo_mode.as_ref().and_then(|repo_mode| {
        selected_tag_item(repo_mode).map(|tag| {
            (
                repo_mode.current_repo_id.clone(),
                tag.name.clone(),
                format!("tag {} ({})", tag.name, tag.target_short_oid),
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
            commit: tag_name,
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
    let visible_indices = filtered_stash_indices(repo_mode);
    let selected_index = repo_mode
        .stash_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.stashes.get(selected_index)
}

fn selected_worktree_item(repo_mode: &RepoModeState) -> Option<&crate::state::WorktreeItem> {
    let detail = repo_mode.detail.as_ref()?;
    let visible_indices = filtered_worktree_indices(repo_mode);
    let selected_index = repo_mode
        .worktree_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.worktrees.get(selected_index)
}

fn selected_submodule_item(repo_mode: &RepoModeState) -> Option<&crate::state::SubmoduleItem> {
    let detail = repo_mode.detail.as_ref()?;
    let visible_indices = filtered_submodule_indices(repo_mode);
    let selected_index = repo_mode
        .submodules_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| visible_indices.first().copied())?;
    detail.submodules.get(selected_index)
}

fn step_branch_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.branches_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_branch_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.branches_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_remote_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.remotes_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_remote_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.remotes_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_remote_branch_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.remote_branches_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_remote_branch_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.remote_branches_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_tag_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.tags_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_tag_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.tags_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_stash_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.stash_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_stash_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.stash_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_stash_file_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    let Some(stash) = selected_stash_item(repo_mode) else {
        repo_mode.stash_files_view.selected_index = None;
        return false;
    };
    if stash.changed_files.is_empty() {
        repo_mode.stash_files_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_stash_file_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.stash_files_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_reflog_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.reflog_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_reflog_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.reflog_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_worktree_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.worktree_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_worktree_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.worktree_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_submodule_selection(repo_mode: &mut RepoModeState, step: isize) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.submodules_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_submodule_indices(repo_mode);
    step_filtered_selection(
        &mut repo_mode.submodules_view.selected_index,
        &visible_indices,
        step,
    )
}

fn step_filtered_selection(
    selected_index: &mut Option<usize>,
    visible_indices: &[usize],
    step: isize,
) -> bool {
    if visible_indices.is_empty() {
        *selected_index = None;
        return false;
    }

    let current_position = selected_index
        .and_then(|selected| visible_indices.iter().position(|index| *index == selected))
        .unwrap_or(0);
    let next_position =
        (current_position as isize + step).rem_euclid(visible_indices.len() as isize) as usize;
    let next_index = visible_indices[next_position];
    let changed = *selected_index != Some(next_index);
    *selected_index = Some(next_index);
    changed
}

fn select_filtered_visible_index(
    selected_index: &mut Option<usize>,
    visible_indices: &[usize],
    index: usize,
) -> bool {
    let Some(next_index) = visible_indices.get(index).copied() else {
        return false;
    };

    let changed = *selected_index != Some(next_index);
    *selected_index = Some(next_index);
    changed
}

fn select_repo_detail_item_at(repo_mode: &mut RepoModeState, index: usize) -> bool {
    let changed = match repo_mode.active_subview {
        crate::state::RepoSubview::Branches => {
            let visible_indices = filtered_branch_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.branches_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Remotes => {
            let visible_indices = filtered_remote_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.remotes_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::RemoteBranches => {
            let visible_indices = filtered_remote_branch_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.remote_branches_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Tags => {
            let visible_indices = filtered_tag_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.tags_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Commits => match repo_mode.commit_subview_mode {
            crate::state::CommitSubviewMode::History => {
                let visible_indices = filtered_commit_indices(repo_mode);
                select_filtered_visible_index(
                    &mut repo_mode.commits_view.selected_index,
                    &visible_indices,
                    index,
                )
            }
            crate::state::CommitSubviewMode::Files => match repo_mode.commit_files_mode {
                crate::state::CommitFilesMode::List => {
                    let visible_indices = filtered_commit_file_indices(repo_mode);
                    select_filtered_visible_index(
                        &mut repo_mode.commit_files_view.selected_index,
                        &visible_indices,
                        index,
                    )
                }
                crate::state::CommitFilesMode::Diff => false,
            },
        },
        crate::state::RepoSubview::Stash => match repo_mode.stash_subview_mode {
            crate::state::StashSubviewMode::List => {
                let visible_indices = filtered_stash_indices(repo_mode);
                select_filtered_visible_index(
                    &mut repo_mode.stash_view.selected_index,
                    &visible_indices,
                    index,
                )
            }
            crate::state::StashSubviewMode::Files => {
                let visible_indices = filtered_stash_file_indices(repo_mode);
                select_filtered_visible_index(
                    &mut repo_mode.stash_files_view.selected_index,
                    &visible_indices,
                    index,
                )
            }
        },
        crate::state::RepoSubview::Reflog => {
            let visible_indices = filtered_reflog_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.reflog_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Worktrees => {
            let visible_indices = filtered_worktree_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.worktree_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Submodules => {
            let visible_indices = filtered_submodule_indices(repo_mode);
            select_filtered_visible_index(
                &mut repo_mode.submodules_view.selected_index,
                &visible_indices,
                index,
            )
        }
        crate::state::RepoSubview::Status
        | crate::state::RepoSubview::Compare
        | crate::state::RepoSubview::Rebase => false,
    };

    if changed {
        repo_mode.diff_scroll = 0;
    }
    changed
}

fn select_filtered_edge(
    selected_index: &mut Option<usize>,
    visible_indices: &[usize],
    select_last: bool,
) -> bool {
    if visible_indices.is_empty() {
        *selected_index = None;
        return false;
    }

    let next_index = if select_last {
        *visible_indices.last().unwrap_or(&visible_indices[0])
    } else {
        visible_indices[0]
    };
    let changed = *selected_index != Some(next_index);
    *selected_index = Some(next_index);
    changed
}

fn clear_status_selection(repo_mode: &mut RepoModeState, focused_pane: PaneId) {
    match focused_pane {
        PaneId::RepoUnstaged => repo_mode.status_view.selected_index = None,
        PaneId::RepoStaged => repo_mode.staged_view.selected_index = None,
        _ => {}
    }
}

fn select_status_edge(
    repo_mode: &mut RepoModeState,
    focused_pane: PaneId,
    select_last: bool,
) -> bool {
    if repo_mode.detail.is_none() {
        clear_status_selection(repo_mode, focused_pane);
        return false;
    }

    let len = status_entries_len(repo_mode, focused_pane);
    if len == 0 {
        clear_status_selection(repo_mode, focused_pane);
        return false;
    }

    match focused_pane {
        PaneId::RepoUnstaged => {
            if select_last {
                repo_mode.status_view.select_last(len).is_some()
            } else {
                repo_mode.status_view.select_first(len).is_some()
            }
        }
        PaneId::RepoStaged => {
            if select_last {
                repo_mode.staged_view.select_last(len).is_some()
            } else {
                repo_mode.staged_view.select_first(len).is_some()
            }
        }
        _ => false,
    }
}

fn select_branch_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.branches_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_branch_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.branches_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_remote_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.remotes_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_remote_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.remotes_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_remote_branch_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.remote_branches_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_remote_branch_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.remote_branches_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_tag_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.tags_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_tag_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.tags_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_commit_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.commits_view.selected_index = None;
        repo_mode.commit_files_view.selected_index = None;
        return false;
    }

    let changed = match repo_mode.commit_subview_mode {
        crate::state::CommitSubviewMode::History => {
            let visible_indices = filtered_commit_indices(repo_mode);
            select_filtered_edge(
                &mut repo_mode.commits_view.selected_index,
                &visible_indices,
                select_last,
            )
        }
        crate::state::CommitSubviewMode::Files => {
            let visible_indices = filtered_commit_file_indices(repo_mode);
            select_filtered_edge(
                &mut repo_mode.commit_files_view.selected_index,
                &visible_indices,
                select_last,
            )
        }
    };
    if changed {
        repo_mode.diff_scroll = 0;
    }
    changed
}

fn select_stash_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.stash_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_stash_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.stash_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_stash_file_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    let Some(stash) = selected_stash_item(repo_mode) else {
        repo_mode.stash_files_view.selected_index = None;
        return false;
    };
    if stash.changed_files.is_empty() {
        repo_mode.stash_files_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_stash_file_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.stash_files_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_reflog_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.reflog_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_reflog_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.reflog_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_worktree_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.worktree_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_worktree_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.worktree_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn select_submodule_edge(repo_mode: &mut RepoModeState, select_last: bool) -> bool {
    if repo_mode.detail.is_none() {
        repo_mode.submodules_view.selected_index = None;
        return false;
    }

    let visible_indices = filtered_submodule_indices(repo_mode);
    select_filtered_edge(
        &mut repo_mode.submodules_view.selected_index,
        &visible_indices,
        select_last,
    )
}

fn filtered_branch_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let mut indices: Vec<_> = detail
        .branches
        .iter()
        .enumerate()
        .filter_map(|(index, branch)| {
            repo_mode
                .branches_filter
                .active_query()
                .is_none_or(|query| crate::state::branch_matches_filter(branch, &query))
                .then_some(index)
        })
        .collect();
    if repo_mode.branch_sort_mode == crate::state::BranchSortMode::Name {
        indices.sort_by(|left, right| {
            detail.branches[*left]
                .name
                .cmp(&detail.branches[*right].name)
                .then_with(|| left.cmp(right))
        });
    }
    indices
}

fn filtered_remote_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.remotes_filter.active_query() else {
        return (0..detail.remotes.len()).collect();
    };
    detail
        .remotes
        .iter()
        .enumerate()
        .filter_map(|(index, remote)| {
            crate::state::remote_matches_filter(remote, &query).then_some(index)
        })
        .collect()
}

fn filtered_remote_branch_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let mut indices: Vec<_> = detail
        .remote_branches
        .iter()
        .enumerate()
        .filter_map(|(index, branch)| {
            repo_mode
                .remote_branches_filter
                .active_query()
                .is_none_or(|query| crate::state::remote_branch_matches_filter(branch, &query))
                .then_some(index)
        })
        .collect();
    if repo_mode.remote_branch_sort_mode == crate::state::RemoteBranchSortMode::Name {
        indices.sort_by(|left, right| {
            detail.remote_branches[*left]
                .name
                .cmp(&detail.remote_branches[*right].name)
                .then_with(|| left.cmp(right))
        });
    }
    indices
}

fn filtered_tag_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.tags_filter.active_query() else {
        return (0..detail.tags.len()).collect();
    };
    detail
        .tags
        .iter()
        .enumerate()
        .filter_map(|(index, tag)| crate::state::tag_matches_filter(tag, &query).then_some(index))
        .collect()
}

fn filtered_commit_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.commits_filter.active_query() else {
        return (0..detail.commits.len()).collect();
    };
    detail
        .commits
        .iter()
        .enumerate()
        .filter_map(|(index, commit)| {
            crate::state::commit_matches_filter(commit, &query).then_some(index)
        })
        .collect()
}

fn filtered_commit_file_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(commit) = selected_commit_item(repo_mode) else {
        return Vec::new();
    };
    let Some(query) = repo_mode.commit_files_filter.active_query() else {
        return (0..commit.changed_files.len()).collect();
    };
    commit
        .changed_files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            crate::state::commit_file_matches_filter(file, &query).then_some(index)
        })
        .collect()
}

fn filtered_stash_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.stash_filter.active_query() else {
        return (0..detail.stashes.len()).collect();
    };
    detail
        .stashes
        .iter()
        .enumerate()
        .filter_map(|(index, stash)| {
            crate::state::stash_matches_filter(stash, &query).then_some(index)
        })
        .collect()
}

fn filtered_stash_file_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(stash) = selected_stash_item(repo_mode) else {
        return Vec::new();
    };
    (0..stash.changed_files.len()).collect()
}

fn filtered_reflog_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.reflog_filter.active_query() else {
        return (0..detail.reflog_items.len()).collect();
    };
    detail
        .reflog_items
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            crate::state::reflog_matches_filter(entry, &query).then_some(index)
        })
        .collect()
}

fn filtered_worktree_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.worktree_filter.active_query() else {
        return (0..detail.worktrees.len()).collect();
    };
    detail
        .worktrees
        .iter()
        .enumerate()
        .filter_map(|(index, worktree)| {
            crate::state::worktree_matches_filter(worktree, &query).then_some(index)
        })
        .collect()
}

fn filtered_submodule_indices(repo_mode: &RepoModeState) -> Vec<usize> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    let Some(query) = repo_mode.submodules_filter.active_query() else {
        return (0..detail.submodules.len()).collect();
    };
    detail
        .submodules
        .iter()
        .enumerate()
        .filter_map(|(index, submodule)| {
            crate::state::submodule_matches_filter(submodule, &query).then_some(index)
        })
        .collect()
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
        ignore_whitespace_in_diff: repo_mode.ignore_whitespace_in_diff,
        diff_context_lines: repo_mode.diff_context_lines,
        rename_similarity_threshold: repo_mode.rename_similarity_threshold,
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
    let commit_ref = state.repo_mode.as_ref().and_then(|repo_mode| {
        (repo_mode.active_subview == crate::state::RepoSubview::Commits)
            .then(|| repo_mode.commit_history_ref.clone())
            .flatten()
    });
    let commit_history_mode = state
        .repo_mode
        .as_ref()
        .filter(|repo_mode| repo_mode.active_subview == crate::state::RepoSubview::Commits)
        .map(|repo_mode| repo_mode.commit_history_mode)
        .unwrap_or(CommitHistoryMode::Linear);
    Effect::LoadRepoDetail {
        repo_id,
        selected_path,
        diff_presentation,
        commit_ref,
        commit_history_mode,
        ignore_whitespace_in_diff: state
            .repo_mode
            .as_ref()
            .map(|repo_mode| repo_mode.ignore_whitespace_in_diff)
            .unwrap_or(false),
        diff_context_lines: state
            .repo_mode
            .as_ref()
            .map(|repo_mode| repo_mode.diff_context_lines)
            .unwrap_or(crate::state::DEFAULT_DIFF_CONTEXT_LINES),
        rename_similarity_threshold: state
            .repo_mode
            .as_ref()
            .map(|repo_mode| repo_mode.rename_similarity_threshold)
            .unwrap_or(crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD),
    }
}

fn active_diff_reload_effect(state: &AppState) -> Option<Effect> {
    let repo_mode = state.repo_mode.as_ref()?;
    match repo_mode.active_subview {
        crate::state::RepoSubview::Compare
            if repo_mode.comparison_base.is_some() && repo_mode.comparison_target.is_some() =>
        {
            Some(load_comparison_diff_effect(repo_mode))
        }
        crate::state::RepoSubview::Commits if commit_file_diff_detail_active(repo_mode) => {
            load_selected_commit_file_diff_effect(repo_mode).map(|(_, effect)| effect)
        }
        crate::state::RepoSubview::Status | crate::state::RepoSubview::Commits => Some(
            load_repo_detail_effect(state, repo_mode.current_repo_id.clone()),
        ),
        _ => None,
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

fn shell_job(repo_id: crate::state::RepoId, command: String) -> ShellCommandRequest {
    let job_id = JobId::new(format!("shell:{}:run-command", repo_id.0));
    ShellCommandRequest::new(job_id, repo_id, command)
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
        GitCommand::UnstageSelection => "unstage-selection",
        GitCommand::StageFile { .. } => "stage-file",
        GitCommand::DiscardFile { .. } => "discard-file",
        GitCommand::UnstageFile { .. } => "unstage-file",
        GitCommand::CommitStaged { .. } => "commit-staged",
        GitCommand::CommitStagedNoVerify { .. } => "commit-staged-no-verify",
        GitCommand::CommitStagedWithEditor => "commit-staged-editor",
        GitCommand::AmendHead { .. } => "amend-head",
        GitCommand::CreateAmendCommit {
            include_file_changes,
            ..
        } => {
            if *include_file_changes {
                "create-amend-commit-with-changes"
            } else {
                "create-amend-commit-without-changes"
            }
        }
        GitCommand::AmendCommitAttributes {
            reset_author,
            co_author,
            ..
        } => match (*reset_author, co_author.is_some()) {
            (true, true) => "amend-commit-author-and-co-author",
            (true, false) => "amend-commit-reset-author",
            (false, true) => "amend-commit-set-co-author",
            (false, false) => "amend-commit-attributes",
        },
        GitCommand::CreateFixupCommit { .. } => "create-fixup-commit",
        GitCommand::RewordCommitWithEditor { .. } => "reword-commit-editor",
        GitCommand::StartCommitRebase { mode, .. } => match mode {
            RebaseStartMode::Interactive => "start-interactive-rebase",
            RebaseStartMode::Amend => "start-amend-rebase",
            RebaseStartMode::Fixup => "start-fixup-rebase",
            RebaseStartMode::FixupWithMessage => "set-fixup-message-rebase",
            RebaseStartMode::ApplyFixups => "apply-fixups-rebase",
            RebaseStartMode::Squash => "start-squash-rebase",
            RebaseStartMode::Drop => "start-drop-rebase",
            RebaseStartMode::MoveUp { .. } => "move-commit-up-rebase",
            RebaseStartMode::MoveDown { .. } => "move-commit-down-rebase",
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
        GitCommand::StartBisect { .. } => "start-bisect",
        GitCommand::MarkBisect { .. } => "mark-bisect",
        GitCommand::SkipBisect { .. } => "skip-bisect",
        GitCommand::ResetBisect => "reset-bisect",
        GitCommand::CreateBranch { .. } => "create-branch",
        GitCommand::StartGitFlow { branch_type, .. } => match branch_type {
            crate::state::GitFlowBranchType::Feature => "start-git-flow-feature",
            crate::state::GitFlowBranchType::Hotfix => "start-git-flow-hotfix",
            crate::state::GitFlowBranchType::Bugfix => "start-git-flow-bugfix",
            crate::state::GitFlowBranchType::Release => "start-git-flow-release",
        },
        GitCommand::CreateTag { .. } => "create-tag",
        GitCommand::CreateTagFromCommit { .. } => "create-tag-from-commit",
        GitCommand::CreateBranchFromCommit { .. } => "create-branch-from-commit",
        GitCommand::CreateBranchFromRef { .. } => "create-branch-from-ref",
        GitCommand::FinishGitFlow { .. } => "finish-git-flow",
        GitCommand::CheckoutBranch { .. } => "checkout-branch",
        GitCommand::ForceCheckoutRef { .. } => "force-checkout-ref",
        GitCommand::CheckoutRemoteBranch { .. } => "checkout-remote-branch",
        GitCommand::CheckoutTag { .. } => "checkout-tag",
        GitCommand::CheckoutCommit { .. } => "checkout-commit",
        GitCommand::CheckoutCommitFile { .. } => "checkout-commit-file",
        GitCommand::RenameBranch { .. } => "rename-branch",
        GitCommand::RenameStash { .. } => "rename-stash",
        GitCommand::CreateBranchFromStash { .. } => "create-branch-from-stash",
        GitCommand::DeleteBranch { .. } => "delete-branch",
        GitCommand::UnsetBranchUpstream { .. } => "unset-branch-upstream",
        GitCommand::FastForwardCurrentBranchFromUpstream { .. } => {
            "fast-forward-selected-branch-from-upstream"
        }
        GitCommand::MergeRefIntoCurrent { .. } => "merge-ref-into-current",
        GitCommand::RebaseCurrentOntoRef { .. } => "rebase-current-onto-ref",
        GitCommand::DeleteRemoteBranch { .. } => "delete-remote-branch",
        GitCommand::DeleteTag { .. } => "delete-tag",
        GitCommand::PushTag { .. } => "push-tag",
        GitCommand::AddRemote { .. } => "add-remote",
        GitCommand::EditRemote { .. } => "edit-remote",
        GitCommand::RemoveRemote { .. } => "remove-remote",
        GitCommand::FetchRemote { .. } => "fetch-remote",
        GitCommand::CreateStash {
            mode: StashMode::Tracked,
            ..
        } => "create-stash",
        GitCommand::CreateStash {
            mode: StashMode::KeepIndex,
            ..
        } => "create-stash-keep-index",
        GitCommand::CreateStash {
            mode: StashMode::IncludeUntracked,
            ..
        } => "create-stash-including-untracked",
        GitCommand::CreateStash {
            mode: StashMode::Staged,
            ..
        } => "create-stash-staged",
        GitCommand::CreateStash {
            mode: StashMode::Unstaged,
            ..
        } => "create-stash-unstaged",
        GitCommand::ApplyStash { .. } => "apply-stash",
        GitCommand::PopStash { .. } => "pop-stash",
        GitCommand::DropStash { .. } => "drop-stash",
        GitCommand::CreateWorktree { .. } => "create-worktree",
        GitCommand::RemoveWorktree { .. } => "remove-worktree",
        GitCommand::AddSubmodule { .. } => "add-submodule",
        GitCommand::EditSubmoduleUrl { .. } => "edit-submodule-url",
        GitCommand::InitSubmodule { .. } => "init-submodule",
        GitCommand::UpdateSubmodule { .. } => "update-submodule",
        GitCommand::InitAllSubmodules => "init-all-submodules",
        GitCommand::UpdateAllSubmodules => "update-all-submodules",
        GitCommand::UpdateAllSubmodulesRecursively => "update-all-submodules-recursively",
        GitCommand::DeinitAllSubmodules => "deinit-all-submodules",
        GitCommand::RemoveSubmodule { .. } => "remove-submodule",
        GitCommand::SetBranchUpstream { .. } => "set-branch-upstream",
        GitCommand::UpdateBranchRefs { .. } => "update-branch-refs",
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

fn enqueue_shell_job(state: &mut AppState, job: &ShellCommandRequest, summary: &str) {
    state
        .background_jobs
        .insert(job.job_id.clone(), background_shell_job(job));
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

fn background_shell_job(job: &ShellCommandRequest) -> BackgroundJob {
    BackgroundJob {
        id: job.job_id.clone(),
        kind: BackgroundJobKind::ShellCommand,
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
    use crate::effect::{
        Effect, GitCommand, GitCommandRequest, RebaseStartMode, ShellCommandRequest,
    };
    use crate::event::{Event, TimerEvent, WatcherEvent, WorkerEvent};
    use crate::state::{
        AppMode, AppState, BackgroundJobKind, BackgroundJobState, CommitBoxMode, CommitFileItem,
        CommitHistoryMode, CommitItem, ConfirmableOperation, DiffHunk, DiffLine, DiffLineKind,
        DiffModel, DiffPresentation, FileStatus, FileStatusKind, InputPromptOperation, JobId,
        MenuOperation, MergeFastForwardPreference, MergeState, MergeVariant, MessageLevel,
        ModalKind, OperationProgress, PaneId, RebaseKind, RebaseState, ReflogItem, RepoDetail,
        RepoId, RepoModeState, RepoSubview, RepoSubviewFilterState, RepoSummary, ScanStatus,
        SelectedHunk, StashItem, StashMode, SubmoduleItem, Timestamp, WatcherHealth,
        WorkspaceFilterMode, WorktreeItem,
    };

    use super::{merge_rebase_menu_entries, reduce};

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
    fn open_in_editor_from_worktree_detail_targets_selected_worktree() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let worktree_path = std::path::PathBuf::from("/tmp/repo-1-feature");
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
                active_subview: RepoSubview::Worktrees,
                detail: Some(RepoDetail {
                    worktrees: vec![
                        crate::state::WorktreeItem {
                            path: repo_root.clone(),
                            branch: Some("main".to_string()),
                            ..crate::state::WorktreeItem::default()
                        },
                        crate::state::WorktreeItem {
                            path: worktree_path.clone(),
                            branch: Some("feature".to_string()),
                            ..crate::state::WorktreeItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                worktree_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenInEditor));

        assert_eq!(
            result.effects,
            vec![Effect::OpenEditor {
                cwd: repo_root,
                target: worktree_path,
            }]
        );
    }

    #[test]
    fn open_in_editor_from_commit_file_mode_targets_selected_file() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let selected_path = std::path::PathBuf::from("src/lib.rs");
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
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![
                            CommitFileItem {
                                path: selected_path.clone(),
                                kind: FileStatusKind::Added,
                            },
                            CommitFileItem {
                                path: std::path::PathBuf::from("notes.md"),
                                kind: FileStatusKind::Added,
                            },
                        ],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenInEditor));

        assert_eq!(
            result.effects,
            vec![Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: repo_root.join(selected_path),
            }]
        );
    }

    #[test]
    fn open_config_file_actions_use_loaded_config_path() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let config_path = std::path::PathBuf::from("/tmp/configs/super-lazygit.toml");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            config_path: Some(config_path.clone()),
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
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let opened = reduce(
            state.clone(),
            Event::Action(Action::OpenConfigFileInDefaultApp),
        );
        assert!(matches!(
            opened.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id
                    && command.contains("xdg-open")
                    && command.contains(config_path.to_string_lossy().as_ref())
        ));

        let edited = reduce(state, Event::Action(Action::OpenConfigFileInEditor));
        assert_eq!(
            edited.effects,
            vec![Effect::OpenEditor {
                cwd: repo_root,
                target: config_path,
            }]
        );
    }

    #[test]
    fn check_for_updates_opens_release_page() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repository_url: Some("https://github.com/quangdang/super_lazygit_rust".to_string()),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CheckForUpdates));
        assert!(matches!(
            result.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id
                    && command.contains("https://github.com/quangdang/super_lazygit_rust/releases")
        ));
    }

    #[test]
    fn copy_selected_status_path_from_commit_file_mode_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let selected_path = std::path::PathBuf::from("src/lib.rs");
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
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::List,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![CommitFileItem {
                            path: selected_path.clone(),
                            kind: FileStatusKind::Added,
                        }],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CopySelectedStatusPath));

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::clipboard_shell_command(
                    repo_root.join(selected_path).as_os_str(),
                    &crate::state::OsConfigSnapshot::default(),
                ),
            ))]
        );
    }

    #[test]
    fn copy_selected_commit_hash_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CopySelectedCommitHash));

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::clipboard_shell_command(
                    std::ffi::OsStr::new("abcdef1"),
                    &crate::state::OsConfigSnapshot::default(),
                ),
            ))]
        );
    }

    #[test]
    fn copy_selected_tag_name_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Tags,
                detail: Some(RepoDetail {
                    tags: vec![crate::state::TagItem {
                        name: "snapshot".to_string(),
                        target_oid: "1234567890abcdef".to_string(),
                        target_short_oid: "1234567".to_string(),
                        summary: "second".to_string(),
                        annotated: false,
                    }],
                    ..RepoDetail::default()
                }),
                tags_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CopySelectedTagName));
        assert!(matches!(
            result.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id && command.contains("snapshot")
        ));
    }

    #[test]
    fn copy_selected_reflog_commit_hash_and_open_browser_queue_shell_jobs() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Reflog,
                detail: Some(RepoDetail {
                    remotes: vec![crate::state::RemoteItem {
                        name: "upstream".to_string(),
                        fetch_url: "git@github.com:example/repo.git".to_string(),
                        push_url: "git@github.com:example/repo.git".to_string(),
                        branch_count: 0,
                    }],
                    reflog_items: vec![ReflogItem {
                        selector: "HEAD@{1}".to_string(),
                        oid: "1234567890abcdef".to_string(),
                        short_oid: "1234567".to_string(),
                        unix_timestamp: 0,
                        summary: "commit: prior".to_string(),
                        description: "HEAD@{1}: commit: prior".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let copied = reduce(
            state.clone(),
            Event::Action(Action::CopySelectedReflogCommitHash),
        );
        assert!(matches!(
            copied.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id && command.contains("1234567")
        ));

        let opened = reduce(state, Event::Action(Action::OpenSelectedReflogInBrowser));
        assert!(matches!(
            opened.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id
                    && command.contains("github.com/example/repo/commit/1234567890abcdef")
        ));
    }

    #[test]
    fn copy_selected_submodule_name_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![SubmoduleItem {
                        name: "child-module".to_string(),
                        path: std::path::PathBuf::from("vendor/child-module"),
                        url: "../child-module.git".to_string(),
                        branch: Some("main".to_string()),
                        short_oid: Some("abcdef1".to_string()),
                        initialized: true,
                        dirty: false,
                        conflicted: false,
                    }],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CopySelectedSubmoduleName));
        assert!(matches!(
            result.effects.as_slice(),
            [Effect::RunShellCommand(ShellCommandRequest { repo_id: actual_repo_id, command, .. })]
                if actual_repo_id == &repo_id && command.contains("child-module")
        ));
    }

    #[test]
    fn open_selected_status_path_in_default_app_from_commit_file_mode_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let selected_path = std::path::PathBuf::from("src/lib.rs");
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
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::List,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![CommitFileItem {
                            path: selected_path.clone(),
                            kind: FileStatusKind::Added,
                        }],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenSelectedStatusPathInDefaultApp),
        );

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::open_in_default_app_command(
                    repo_root.join(selected_path).as_os_str(),
                    &crate::state::OsConfigSnapshot::default(),
                    super::OsCommandTemplateKind::OpenFile,
                ),
            ))]
        );
    }

    #[test]
    fn open_selected_commit_in_browser_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    remotes: vec![
                        crate::state::RemoteItem {
                            name: "origin".to_string(),
                            fetch_url: "/tmp/origin.git".to_string(),
                            push_url: "/tmp/origin.git".to_string(),
                            branch_count: 1,
                        },
                        crate::state::RemoteItem {
                            name: "upstream".to_string(),
                            fetch_url: "git@github.com:example/repo.git".to_string(),
                            push_url: "git@github.com:example/repo.git".to_string(),
                            branch_count: 0,
                        },
                    ],
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedCommitInBrowser));

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::open_in_default_app_command(
                    std::ffi::OsStr::new("https://github.com/example/repo/commit/abcdef1234567890"),
                    &crate::state::OsConfigSnapshot::default(),
                    super::OsCommandTemplateKind::OpenLink,
                ),
            ))]
        );
    }

    #[test]
    fn clipboard_shell_command_uses_override_and_shell_functions_file() {
        let os = crate::state::OsConfigSnapshot {
            copy_to_clipboard_cmd: "custom-copy {{text}}".to_string(),
            shell_functions_file: "/tmp/lazygit-shell-functions.sh".to_string(),
            ..crate::state::OsConfigSnapshot::default()
        };

        let command = super::clipboard_shell_command(std::ffi::OsStr::new("hello world"), &os);

        assert_eq!(
            command,
            ". '/tmp/lazygit-shell-functions.sh'\ncustom-copy 'hello world'"
        );
    }

    #[test]
    fn open_in_default_app_command_uses_matching_override_template() {
        let os = crate::state::OsConfigSnapshot {
            open: "custom-open {{filename}}".to_string(),
            open_link: "custom-link {{link}}".to_string(),
            ..crate::state::OsConfigSnapshot::default()
        };

        let file_command = super::open_in_default_app_command(
            std::ffi::OsStr::new("/tmp/file with spaces.txt"),
            &os,
            super::OsCommandTemplateKind::OpenFile,
        );
        let link_command = super::open_in_default_app_command(
            std::ffi::OsStr::new("https://example.com/repo"),
            &os,
            super::OsCommandTemplateKind::OpenLink,
        );

        assert_eq!(file_command, "custom-open '/tmp/file with spaces.txt'");
        assert_eq!(link_command, "custom-link 'https://example.com/repo'");
    }

    #[test]
    fn copy_selected_commit_for_cherry_pick_tracks_copied_commit() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::CopySelectedCommitForCherryPick),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.copied_commit.as_ref())
                .map(|commit| (
                    commit.oid.as_str(),
                    commit.short_oid.as_str(),
                    commit.summary.as_str()
                )),
            Some(("abcdef1234567890", "abcdef1", "add lib"))
        );
    }

    #[test]
    fn cherry_pick_copied_commit_opens_confirmation_for_copied_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                copied_commit: Some(crate::state::CopiedCommit {
                    oid: "copied".to_string(),
                    short_oid: "copy123".to_string(),
                    summary: "copied commit".to_string(),
                }),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CherryPickCopiedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::CherryPickCommit {
                commit: "copied".to_string(),
                summary: "copy123 copied commit".to_string(),
            })
        );
    }

    #[test]
    fn clear_copied_commit_selection_clears_repo_state() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                copied_commit: Some(crate::state::CopiedCommit {
                    oid: "copied".to_string(),
                    short_oid: "copy123".to_string(),
                    summary: "copied commit".to_string(),
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ClearCopiedCommitSelection));

        assert!(result
            .state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.copied_commit.as_ref())
            .is_none());
    }

    #[test]
    fn open_selected_status_path_in_external_difftool_from_commit_file_mode_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from("/tmp/repo-1");
        let selected_path = std::path::PathBuf::from("src/lib.rs");
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
                        real_path: repo_root,
                        display_path: "/tmp/repo-1".to_string(),
                        ..RepoSummary::default()
                    },
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..crate::state::WorkspaceState::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::List,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![CommitFileItem {
                            path: selected_path.clone(),
                            kind: FileStatusKind::Added,
                        }],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenSelectedStatusPathInExternalDiffTool),
        );

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::external_difftool_command(&selected_path, PaneId::RepoDetail),
            ))]
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
    fn focus_repo_main_pane_restores_last_main_focus_and_blurs_filter() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                main_focus: PaneId::RepoStaged,
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature-contract".to_string(),
                            is_head: false,
                            upstream: None,
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                branches_filter: RepoSubviewFilterState {
                    query: "fea".to_string(),
                    focused: true,
                },
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::FocusRepoMainPane));

        assert_eq!(result.state.focused_pane, PaneId::RepoStaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.query.as_str()),
            Some("fea")
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(false)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn repo_subview_filter_actions_sync_visible_selection_and_clear_focus() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature-contract".to_string(),
                            is_head: false,
                            upstream: None,
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "bugfix".to_string(),
                            is_head: false,
                            upstream: None,
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                branches_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let focused = reduce(state, Event::Action(Action::FocusRepoSubviewFilter));
        assert_eq!(
            focused
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(true)
        );

        let filtered = reduce(
            focused.state,
            Event::Action(Action::AppendRepoSubviewFilter {
                text: "fea".to_string(),
            }),
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.query.as_str()),
            Some("fea")
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );

        let blurred = reduce(filtered.state, Event::Action(Action::BlurRepoSubviewFilter));
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(false)
        );
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.query.as_str()),
            Some("fea")
        );

        let cancelled = reduce(
            blurred.state,
            Event::Action(Action::CancelRepoSubviewFilter),
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(false)
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );
    }

    #[test]
    fn activate_repo_subview_selection_enters_selected_worktree_repo() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let target_repo_id = RepoId::new("/tmp/repo-1-feature");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Worktrees,
                detail: Some(RepoDetail {
                    worktrees: vec![
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-1"),
                            branch: Some("main".to_string()),
                            ..WorktreeItem::default()
                        },
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-1-feature"),
                            branch: Some("feature".to_string()),
                            ..WorktreeItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                worktree_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ActivateRepoSubviewSelection));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(target_repo_id.clone())
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id: target_repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
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
    fn delete_selected_branch_checks_merge_status_first() {
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: None,
                            ..crate::state::BranchItem::default()
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

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result.effects,
            vec![Effect::CheckBranchMerged {
                repo_id,
                branch_name: "feature".to_string(),
            }]
        );
    }

    #[test]
    fn merged_branch_delete_runs_without_confirmation() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Branches,
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::BranchMergeCheckCompleted {
                repo_id: repo_id.clone(),
                branch_name: "feature".to_string(),
                merged: true,
            }),
        );

        let expected_job = GitCommandRequest {
            job_id: JobId::new("git:repo-1:delete-branch"),
            repo_id: repo_id.clone(),
            command: GitCommand::DeleteBranch {
                branch_name: "feature".to_string(),
                force: true,
            },
        };

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&expected_job.job_id)
                .map(|job| job.state.clone()),
            Some(BackgroundJobState::Queued)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(OperationProgress::Running {
                job_id: expected_job.job_id.clone(),
                summary: "Delete branch".to_string(),
            })
        );
        assert_eq!(result.effects, vec![Effect::RunGitCommand(expected_job)]);
    }

    #[test]
    fn unmerged_branch_delete_opens_force_delete_confirmation() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Branches,
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Worker(WorkerEvent::BranchMergeCheckCompleted {
                repo_id: repo_id.clone(),
                branch_name: "feature".to_string(),
                merged: false,
            }),
        );

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
                    branch_name: "feature".to_string(),
                    force: true,
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn open_selected_branch_commits_switches_to_history_and_loads_branch_ref() {
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: None,
                            ..crate::state::BranchItem::default()
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

        let result = reduce(state, Event::Action(Action::OpenSelectedBranchCommits));

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
                .map(|repo_mode| repo_mode.commit_subview_mode),
            Some(crate::state::CommitSubviewMode::History)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("feature")
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: Some("feature".to_string()),
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn open_branch_upstream_options_opens_menu_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "main".to_string(),
                        is_head: true,
                        upstream: Some("origin/main".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenBranchUpstreamOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::BranchUpstreamOptions)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn copy_selected_branch_name_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "feature".to_string(),
                        is_head: false,
                        upstream: Some("origin/feature".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CopySelectedBranchName));

        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                JobId::new("shell:/tmp/repo-1:run-command"),
                repo_id,
                super::clipboard_shell_command(
                    std::ffi::OsStr::new("feature"),
                    &crate::state::OsConfigSnapshot::default(),
                ),
            ))]
        );
    }

    #[test]
    fn unset_selected_branch_upstream_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "feature".to_string(),
                        is_head: false,
                        upstream: Some("origin/feature".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::UnsetSelectedBranchUpstream));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::UnsetBranchUpstream {
                    branch_name: "feature".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn fast_forward_selected_branch_from_upstream_opens_confirmation_for_head_branch() {
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                branches_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::FastForwardSelectedBranchFromUpstream),
        );

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::FastForwardCurrentBranchFromUpstream {
                    branch_name: "main".to_string(),
                    upstream_ref: "origin/main".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn merge_selected_branch_into_current_opens_confirmation_modal() {
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
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

        let result = reduce(state, Event::Action(Action::MergeSelectedBranchIntoCurrent));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::MergeRefIntoCurrent {
                    target_ref: "feature".to_string(),
                    source_label: "feature".to_string(),
                    variant: MergeVariant::Regular,
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn merge_selected_ref_into_current_uses_requested_variant_for_branch_view() {
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
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

        let result = reduce(
            state,
            Event::Action(Action::MergeSelectedRefIntoCurrent {
                variant: MergeVariant::Squash,
            }),
        );

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::MergeRefIntoCurrent {
                target_ref: "feature".to_string(),
                source_label: "feature".to_string(),
                variant: MergeVariant::Squash,
            })
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn merge_selected_ref_into_current_uses_requested_variant_for_remote_branch_view() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    remote_branches: vec![crate::state::RemoteBranchItem {
                        name: "origin/feature".to_string(),
                        remote_name: "origin".to_string(),
                        branch_name: "feature".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::MergeSelectedRefIntoCurrent {
                variant: MergeVariant::FastForward,
            }),
        );

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::MergeRefIntoCurrent {
                target_ref: "origin/feature".to_string(),
                source_label: "origin/feature".to_string(),
                variant: MergeVariant::FastForward,
            })
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn switch_repo_subview_commits_resets_explicit_history_to_current_branch() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::History,
                commit_history_mode: CommitHistoryMode::Graph { reverse: true },
                commit_history_ref: Some("feature".to_string()),
                pending_commit_selection_oid: Some("deadbeef".to_string()),
                detail: Some(RepoDetail::default()),
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Commits)),
        );

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
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Linear)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            None
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.pending_commit_selection_oid.as_deref()),
            None
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::LoadRepoDetail {
                repo_id: effect_repo_id,
                commit_ref,
                commit_history_mode: CommitHistoryMode::Linear,
                ..
            } if effect_repo_id == &repo_id && commit_ref.is_none()
        )));
    }

    #[test]
    fn delete_selected_remote_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Remotes,
                detail: Some(RepoDetail {
                    remotes: vec![
                        crate::state::RemoteItem {
                            name: "origin".to_string(),
                            fetch_url: "/tmp/origin.git".to_string(),
                            push_url: "/tmp/origin.git".to_string(),
                            branch_count: 2,
                        },
                        crate::state::RemoteItem {
                            name: "upstream".to_string(),
                            fetch_url: "/tmp/upstream.git".to_string(),
                            push_url: "/tmp/upstream.git".to_string(),
                            branch_count: 0,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remotes_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DeleteSelectedRemote));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::RemoveRemote {
                    remote_name: "upstream".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn fetch_selected_remote_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Remotes,
                detail: Some(RepoDetail {
                    remotes: vec![
                        crate::state::RemoteItem {
                            name: "origin".to_string(),
                            fetch_url: "/tmp/origin.git".to_string(),
                            push_url: "/tmp/origin.git".to_string(),
                            branch_count: 2,
                        },
                        crate::state::RemoteItem {
                            name: "upstream".to_string(),
                            fetch_url: "/tmp/upstream.git".to_string(),
                            push_url: "/tmp/upstream.git".to_string(),
                            branch_count: 0,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remotes_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::FetchSelectedRemote));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::FetchRemote {
                    remote_name: "upstream".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn delete_selected_remote_branch_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DeleteSelectedRemoteBranch));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::DeleteRemoteBranch {
                    remote_name: "origin".to_string(),
                    branch_name: "feature".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn open_selected_remote_branches_switches_to_remote_branch_filter() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Remotes,
                detail: Some(RepoDetail {
                    remotes: vec![
                        crate::state::RemoteItem {
                            name: "origin".to_string(),
                            fetch_url: "/tmp/origin.git".to_string(),
                            push_url: "/tmp/origin.git".to_string(),
                            branch_count: 2,
                        },
                        crate::state::RemoteItem {
                            name: "upstream".to_string(),
                            fetch_url: "/tmp/upstream.git".to_string(),
                            push_url: "/tmp/upstream.git".to_string(),
                            branch_count: 0,
                        },
                    ],
                    remote_branches: vec![crate::state::RemoteBranchItem {
                        name: "origin/main".to_string(),
                        remote_name: "origin".to_string(),
                        branch_name: "main".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                remotes_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedRemoteBranches));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::RemoteBranches)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.remote_branches_filter.query.as_str()),
            Some("upstream")
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.remote_branches_view.selected_index),
            None
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn open_selected_remote_branch_commits_switches_to_history_and_loads_branch_ref() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenSelectedRemoteBranchCommits),
        );

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
                .map(|repo_mode| repo_mode.commit_subview_mode),
            Some(crate::state::CommitSubviewMode::History)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("origin/feature")
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: Some("origin/feature".to_string()),
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn set_current_branch_upstream_to_selected_remote_branch_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "main".to_string(),
                        is_head: true,
                        upstream: Some("origin/main".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SetCurrentBranchUpstreamToSelectedRemoteBranch),
        );

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:set-branch-upstream"),
                repo_id,
                command: GitCommand::SetBranchUpstream {
                    branch_name: "main".to_string(),
                    upstream_ref: "origin/feature".to_string(),
                },
            })]
        );
    }

    #[test]
    fn rebase_current_branch_onto_selected_remote_branch_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::RebaseCurrentBranchOntoSelectedRemoteBranch),
        );

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::RebaseCurrentBranchOntoRef {
                    target_ref: "origin/feature".to_string(),
                    source_label: "origin/feature".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn delete_selected_tag_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Tags,
                detail: Some(RepoDetail {
                    tags: vec![
                        crate::state::TagItem {
                            name: "v1.0.0".to_string(),
                            target_oid: "abcdef1234567890".to_string(),
                            target_short_oid: "abcdef1".to_string(),
                            summary: "release v1.0.0".to_string(),
                            annotated: true,
                        },
                        crate::state::TagItem {
                            name: "snapshot".to_string(),
                            target_oid: "1234567890abcdef".to_string(),
                            target_short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            annotated: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                tags_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DeleteSelectedTag));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::DeleteTag {
                    tag_name: "snapshot".to_string(),
                }
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn open_selected_tag_commits_switches_to_history_and_loads_tag_ref() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Tags,
                detail: Some(RepoDetail {
                    tags: vec![
                        crate::state::TagItem {
                            name: "v1.0.0".to_string(),
                            target_oid: "abcdef1234567890".to_string(),
                            target_short_oid: "abcdef1".to_string(),
                            summary: "release v1.0.0".to_string(),
                            annotated: true,
                        },
                        crate::state::TagItem {
                            name: "snapshot".to_string(),
                            target_oid: "1234567890abcdef".to_string(),
                            target_short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            annotated: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                tags_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedTagCommits));

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
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("snapshot")
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: Some("snapshot".to_string()),
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn open_all_branch_graph_switches_to_graph_history_and_loads_detail() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    commits: vec![crate::state::CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![],
                        diff: DiffModel::default(),
                        ..crate::state::CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                status_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenAllBranchGraph { reverse: true }),
        );

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Graph { reverse: true })
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            None
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Graph { reverse: true },
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn switch_repo_subview_clears_graph_history_mode_without_losing_status_selection() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_history_mode: CommitHistoryMode::Reflog,
                status_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    file_tree: repo_detail_with_file_tree().file_tree,
                    commits: vec![crate::state::CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![],
                        diff: DiffModel::default(),
                        ..crate::state::CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Status)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Status)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Linear)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.status_view.selected_index),
            Some(1)
        );
    }

    #[test]
    fn shared_repo_list_navigation_respects_filtered_detail_subviews() {
        type NavStateFactory = fn() -> AppState;
        type NavSelection = fn(&AppState) -> Option<usize>;
        type NavCase = (&'static str, NavStateFactory, NavSelection);

        fn filtered_branch_state() -> AppState {
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                repo_mode: Some(RepoModeState {
                    current_repo_id: RepoId::new("repo-1"),
                    active_subview: RepoSubview::Branches,
                    branches_view: crate::state::ListViewState {
                        selected_index: Some(0),
                    },
                    branches_filter: crate::state::RepoSubviewFilterState {
                        query: "a".to_string(),
                        focused: false,
                    },
                    detail: Some(RepoDetail {
                        branches: vec![
                            crate::state::BranchItem {
                                name: "alpha".to_string(),
                                is_head: true,
                                upstream: None,
                                ..crate::state::BranchItem::default()
                            },
                            crate::state::BranchItem {
                                name: "hidden".to_string(),
                                is_head: false,
                                upstream: None,
                                ..crate::state::BranchItem::default()
                            },
                            crate::state::BranchItem {
                                name: "beta".to_string(),
                                is_head: false,
                                upstream: None,
                                ..crate::state::BranchItem::default()
                            },
                            crate::state::BranchItem {
                                name: "gamma".to_string(),
                                is_head: false,
                                upstream: None,
                                ..crate::state::BranchItem::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-1"))
                }),
                ..AppState::default()
            }
        }

        fn filtered_commit_state() -> AppState {
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                repo_mode: Some(RepoModeState {
                    current_repo_id: RepoId::new("repo-1"),
                    active_subview: RepoSubview::Commits,
                    commits_view: crate::state::ListViewState {
                        selected_index: Some(0),
                    },
                    commits_filter: crate::state::RepoSubviewFilterState {
                        query: "a".to_string(),
                        focused: false,
                    },
                    detail: Some(RepoDetail {
                        commits: vec![
                            crate::state::CommitItem {
                                oid: "0001".to_string(),
                                short_oid: "0001".to_string(),
                                summary: "alpha".to_string(),
                                changed_files: Vec::new(),
                                diff: DiffModel::default(),
                                ..crate::state::CommitItem::default()
                            },
                            crate::state::CommitItem {
                                oid: "0002".to_string(),
                                short_oid: "0002".to_string(),
                                summary: "hidden".to_string(),
                                changed_files: Vec::new(),
                                diff: DiffModel::default(),
                                ..crate::state::CommitItem::default()
                            },
                            crate::state::CommitItem {
                                oid: "0003".to_string(),
                                short_oid: "0003".to_string(),
                                summary: "beta".to_string(),
                                changed_files: Vec::new(),
                                diff: DiffModel::default(),
                                ..crate::state::CommitItem::default()
                            },
                            crate::state::CommitItem {
                                oid: "0004".to_string(),
                                short_oid: "0004".to_string(),
                                summary: "gamma".to_string(),
                                changed_files: Vec::new(),
                                diff: DiffModel::default(),
                                ..crate::state::CommitItem::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-1"))
                }),
                ..AppState::default()
            }
        }

        fn filtered_stash_state() -> AppState {
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                repo_mode: Some(RepoModeState {
                    current_repo_id: RepoId::new("repo-1"),
                    active_subview: RepoSubview::Stash,
                    stash_view: crate::state::ListViewState {
                        selected_index: Some(0),
                    },
                    stash_filter: crate::state::RepoSubviewFilterState {
                        query: "a".to_string(),
                        focused: false,
                    },
                    detail: Some(RepoDetail {
                        stashes: vec![
                            crate::state::StashItem {
                                stash_ref: "s0".to_string(),
                                label: "alpha".to_string(),
                                changed_files: Vec::new(),
                            },
                            crate::state::StashItem {
                                stash_ref: "s1".to_string(),
                                label: "hidden".to_string(),
                                changed_files: Vec::new(),
                            },
                            crate::state::StashItem {
                                stash_ref: "s2".to_string(),
                                label: "beta".to_string(),
                                changed_files: Vec::new(),
                            },
                            crate::state::StashItem {
                                stash_ref: "s3".to_string(),
                                label: "gamma".to_string(),
                                changed_files: Vec::new(),
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-1"))
                }),
                ..AppState::default()
            }
        }

        fn filtered_worktree_state() -> AppState {
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                repo_mode: Some(RepoModeState {
                    current_repo_id: RepoId::new("repo-1"),
                    active_subview: RepoSubview::Worktrees,
                    worktree_view: crate::state::ListViewState {
                        selected_index: Some(0),
                    },
                    worktree_filter: crate::state::RepoSubviewFilterState {
                        query: "a".to_string(),
                        focused: false,
                    },
                    detail: Some(RepoDetail {
                        worktrees: vec![
                            crate::state::WorktreeItem {
                                path: std::path::PathBuf::from("/tmp/alpha"),
                                branch: Some("main".to_string()),
                                ..crate::state::WorktreeItem::default()
                            },
                            crate::state::WorktreeItem {
                                path: std::path::PathBuf::from("/tmp/hidden"),
                                branch: Some("hidden".to_string()),
                                ..crate::state::WorktreeItem::default()
                            },
                            crate::state::WorktreeItem {
                                path: std::path::PathBuf::from("/tmp/beta"),
                                branch: Some("beta".to_string()),
                                ..crate::state::WorktreeItem::default()
                            },
                            crate::state::WorktreeItem {
                                path: std::path::PathBuf::from("/tmp/gamma"),
                                branch: Some("gamma".to_string()),
                                ..crate::state::WorktreeItem::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-1"))
                }),
                ..AppState::default()
            }
        }

        fn filtered_submodule_state() -> AppState {
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                repo_mode: Some(RepoModeState {
                    current_repo_id: RepoId::new("repo-1"),
                    active_subview: RepoSubview::Submodules,
                    submodules_view: crate::state::ListViewState {
                        selected_index: Some(0),
                    },
                    submodules_filter: crate::state::RepoSubviewFilterState {
                        query: "zed".to_string(),
                        focused: false,
                    },
                    detail: Some(RepoDetail {
                        submodules: vec![
                            crate::state::SubmoduleItem {
                                name: "alpha".to_string(),
                                path: std::path::PathBuf::from("vendor/alpha"),
                                url: "../alpha.git".to_string(),
                                branch: Some("zed".to_string()),
                                short_oid: Some("0001".to_string()),
                                initialized: true,
                                dirty: false,
                                conflicted: false,
                            },
                            crate::state::SubmoduleItem {
                                name: "hidden".to_string(),
                                path: std::path::PathBuf::from("vendor/hidden"),
                                url: "../hidden.git".to_string(),
                                branch: Some("none".to_string()),
                                short_oid: Some("0002".to_string()),
                                initialized: true,
                                dirty: false,
                                conflicted: false,
                            },
                            crate::state::SubmoduleItem {
                                name: "beta".to_string(),
                                path: std::path::PathBuf::from("vendor/beta"),
                                url: "../beta.git".to_string(),
                                branch: Some("zed".to_string()),
                                short_oid: Some("0003".to_string()),
                                initialized: true,
                                dirty: false,
                                conflicted: false,
                            },
                            crate::state::SubmoduleItem {
                                name: "gamma".to_string(),
                                path: std::path::PathBuf::from("vendor/gamma"),
                                url: "../gamma.git".to_string(),
                                branch: Some("zed".to_string()),
                                short_oid: Some("0004".to_string()),
                                initialized: true,
                                dirty: false,
                                conflicted: false,
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-1"))
                }),
                ..AppState::default()
            }
        }

        fn selected_branch_index(state: &AppState) -> Option<usize> {
            state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index)
        }

        fn selected_commit_index(state: &AppState) -> Option<usize> {
            state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index)
        }

        fn selected_stash_index(state: &AppState) -> Option<usize> {
            state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index)
        }

        fn selected_worktree_index(state: &AppState) -> Option<usize> {
            state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index)
        }

        fn selected_submodule_index(state: &AppState) -> Option<usize> {
            state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.submodules_view.selected_index)
        }

        let cases: [NavCase; 5] = [
            ("branches", filtered_branch_state, selected_branch_index),
            ("commits", filtered_commit_state, selected_commit_index),
            ("stash", filtered_stash_state, selected_stash_index),
            (
                "worktrees",
                filtered_worktree_state,
                selected_worktree_index,
            ),
            (
                "submodules",
                filtered_submodule_state,
                selected_submodule_index,
            ),
        ];

        for (label, state_fn, selected_index) in cases {
            let paged = reduce(
                state_fn(),
                Event::Action(Action::PageDownRepoList { page_size: 2 }),
            );
            assert_eq!(
                selected_index(&paged.state),
                Some(3),
                "page down should honor filtered {label} indices",
            );

            let paged_up = reduce(
                paged.state,
                Event::Action(Action::PageUpRepoList { page_size: 2 }),
            );
            assert_eq!(
                selected_index(&paged_up.state),
                Some(0),
                "page up should honor filtered {label} indices",
            );

            let last = reduce(state_fn(), Event::Action(Action::SelectLastRepoListEntry));
            assert_eq!(
                selected_index(&last.state),
                Some(3),
                "select last should honor filtered {label} indices",
            );

            let first = reduce(last.state, Event::Action(Action::SelectFirstRepoListEntry));
            assert_eq!(
                selected_index(&first.state),
                Some(0),
                "select first should honor filtered {label} indices",
            );
        }
    }

    #[test]
    fn adjacent_repo_subview_navigation_wraps_across_repo_tabs() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let previous = reduce(state, Event::Action(Action::SelectPreviousRepoSubview));
        assert_eq!(
            previous
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Submodules)
        );

        let next = reduce(previous.state, Event::Action(Action::SelectNextRepoSubview));
        assert_eq!(
            next.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Status)
        );

        let commits = reduce(
            next.state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Commits)),
        );
        let forward = reduce(commits.state, Event::Action(Action::SelectNextRepoSubview));
        assert_eq!(
            forward
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Compare)
        );

        let backward = reduce(
            forward.state,
            Event::Action(Action::SelectPreviousRepoSubview),
        );
        assert_eq!(
            backward
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
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
                            ..crate::state::CommitItem::default()
                        },
                        crate::state::CommitItem {
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            changed_files: vec![],
                            diff: DiffModel::default(),
                            ..crate::state::CommitItem::default()
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
    fn hard_reset_selected_tag_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::Tags,
                detail: Some(RepoDetail {
                    tags: vec![
                        crate::state::TagItem {
                            name: "v1.0.0".to_string(),
                            target_oid: "abcdef1234567890".to_string(),
                            target_short_oid: "abcdef1".to_string(),
                            summary: "release v1.0.0".to_string(),
                            annotated: true,
                        },
                        crate::state::TagItem {
                            name: "snapshot".to_string(),
                            target_oid: "1234567890abcdef".to_string(),
                            target_short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            annotated: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                tags_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::HardResetToSelectedTag));

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
                    commit: "snapshot".to_string(),
                    summary: "tag snapshot (1234567)".to_string(),
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
                            changed_files: vec![CommitFileItem {
                                path: std::path::PathBuf::from("stash.txt"),
                                kind: FileStatusKind::Modified,
                            }],
                        },
                        StashItem {
                            stash_ref: "stash@{1}".to_string(),
                            label: "stash@{1}: older".to_string(),
                            changed_files: vec![CommitFileItem {
                                path: std::path::PathBuf::from("stash-old.txt"),
                                kind: FileStatusKind::Added,
                            }],
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
    fn activate_repo_subview_selection_opens_selected_stash_files() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Stash,
                detail: Some(RepoDetail {
                    stashes: vec![StashItem {
                        stash_ref: "stash@{0}".to_string(),
                        label: "stash@{0}: latest".to_string(),
                        changed_files: vec![
                            CommitFileItem {
                                path: std::path::PathBuf::from("stash.txt"),
                                kind: FileStatusKind::Modified,
                            },
                            CommitFileItem {
                                path: std::path::PathBuf::from("new.txt"),
                                kind: FileStatusKind::Added,
                            },
                        ],
                    }],
                    ..RepoDetail::default()
                }),
                stash_filter: crate::state::RepoSubviewFilterState {
                    query: "stash".to_string(),
                    focused: true,
                },
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ActivateRepoSubviewSelection));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.stash_subview_mode),
            Some(crate::state::StashSubviewMode::Files)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_files_view.selected_index),
            Some(0)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.stash_filter.focused),
            Some(false)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn activate_repo_subview_selection_closes_stash_file_view() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Stash,
                stash_subview_mode: crate::state::StashSubviewMode::Files,
                detail: Some(RepoDetail {
                    stashes: vec![StashItem {
                        stash_ref: "stash@{0}".to_string(),
                        label: "stash@{0}: latest".to_string(),
                        changed_files: vec![CommitFileItem {
                            path: std::path::PathBuf::from("stash.txt"),
                            kind: FileStatusKind::Modified,
                        }],
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ActivateRepoSubviewSelection));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.stash_subview_mode),
            Some(crate::state::StashSubviewMode::List)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn select_next_stash_file_advances_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                stash_subview_mode: crate::state::StashSubviewMode::Files,
                detail: Some(RepoDetail {
                    stashes: vec![StashItem {
                        stash_ref: "stash@{0}".to_string(),
                        label: "stash@{0}: latest".to_string(),
                        changed_files: vec![
                            CommitFileItem {
                                path: std::path::PathBuf::from("stash.txt"),
                                kind: FileStatusKind::Modified,
                            },
                            CommitFileItem {
                                path: std::path::PathBuf::from("other.txt"),
                                kind: FileStatusKind::Added,
                            },
                        ],
                    }],
                    ..RepoDetail::default()
                }),
                stash_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SelectNextStashFile));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_files_view.selected_index),
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
                            selector: "HEAD@{0}".to_string(),
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            unix_timestamp: 0,
                            summary: "checkout".to_string(),
                            description: "HEAD@{0}: checkout".to_string(),
                        },
                        ReflogItem {
                            selector: "HEAD@{1}".to_string(),
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            unix_timestamp: 0,
                            summary: "commit".to_string(),
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
                            selector: "HEAD@{0}".to_string(),
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: current".to_string(),
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            selector: "HEAD@{1}".to_string(),
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: prior".to_string(),
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
                        ..FileStatus::default()
                    }],
                    reflog_items: vec![
                        ReflogItem {
                            selector: "HEAD@{0}".to_string(),
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: current".to_string(),
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            selector: "HEAD@{1}".to_string(),
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: prior".to_string(),
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
    fn open_selected_reflog_commits_switches_to_reflog_history_and_targets_entry() {
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
                            selector: "HEAD@{0}".to_string(),
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: current".to_string(),
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            selector: "HEAD@{1}".to_string(),
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: prior".to_string(),
                            description: "HEAD@{1}: commit: prior".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                commits_filter: crate::state::RepoSubviewFilterState {
                    query: "stale".to_string(),
                    focused: true,
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedReflogCommits));

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
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Reflog)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.pending_commit_selection_oid.as_deref()),
            Some(concat!(
                "1234567890abcdef",
                "\0",
                "0",
                "\0",
                "commit: prior"
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commits_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Reflog,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn checkout_selected_reflog_entry_queues_commit_checkout_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Reflog,
                detail: Some(RepoDetail {
                    reflog_items: vec![ReflogItem {
                        selector: "HEAD@{1}".to_string(),
                        oid: "1234567890abcdef".to_string(),
                        short_oid: "1234567".to_string(),
                        unix_timestamp: 0,
                        summary: "commit: prior".to_string(),
                        description: "HEAD@{1}: commit: prior".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                reflog_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CheckoutSelectedCommit));

        assert!(matches!(
            result.effects.as_slice(),
            [Effect::RunGitCommand(job)] if job.repo_id == repo_id
                && matches!(
                    job.command,
                    GitCommand::CheckoutCommit { ref commit }
                    if commit == "1234567890abcdef"
                )
        ));
    }

    #[test]
    fn hard_reset_selected_reflog_entry_uses_reflog_selector() {
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
                            selector: "HEAD@{0}".to_string(),
                            oid: "abcdef1234567890".to_string(),
                            short_oid: "abcdef1".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: current".to_string(),
                            description: "HEAD@{0}: commit: current".to_string(),
                        },
                        ReflogItem {
                            selector: "HEAD@{1}".to_string(),
                            oid: "1234567890abcdef".to_string(),
                            short_oid: "1234567".to_string(),
                            unix_timestamp: 0,
                            summary: "commit: prior".to_string(),
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
                    commit: "HEAD@{1}".to_string(),
                    summary: "HEAD@{1}: commit: prior".to_string(),
                }
            ))
        );
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
                            ..WorktreeItem::default()
                        },
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-feature"),
                            branch: Some("feature".to_string()),
                            ..WorktreeItem::default()
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
    fn worktree_filter_lifecycle_updates_visible_selection() {
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
                            ..WorktreeItem::default()
                        },
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-feature"),
                            branch: Some("feature".to_string()),
                            ..WorktreeItem::default()
                        },
                        WorktreeItem {
                            path: std::path::PathBuf::from("/tmp/repo-release"),
                            branch: Some("release".to_string()),
                            ..WorktreeItem::default()
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

        let focused = reduce(state, Event::Action(Action::FocusRepoSubviewFilter));
        assert_eq!(
            focused
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.focused),
            Some(true)
        );

        let filtered = reduce(
            focused.state,
            Event::Action(Action::AppendRepoSubviewFilter {
                text: "feature".to_string(),
            }),
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("feature")
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            Some(1)
        );

        let blurred = reduce(filtered.state, Event::Action(Action::BlurRepoSubviewFilter));
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.focused),
            Some(false)
        );
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("feature")
        );

        let cancelled = reduce(
            blurred.state,
            Event::Action(Action::CancelRepoSubviewFilter),
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            Some(1)
        );

        let refocused = reduce(
            cancelled.state,
            Event::Action(Action::FocusRepoSubviewFilter),
        );
        let emptied = reduce(
            refocused.state,
            Event::Action(Action::AppendRepoSubviewFilter {
                text: "qxz".to_string(),
            }),
        );
        assert_eq!(
            emptied
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("qxz")
        );
        assert_eq!(
            emptied
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            None
        );

        let recovered = reduce(
            emptied.state,
            Event::Action(Action::CancelRepoSubviewFilter),
        );
        assert_eq!(
            recovered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            recovered
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            Some(0)
        );
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
                        changed_files: vec![CommitFileItem {
                            path: std::path::PathBuf::from("stash.txt"),
                            kind: FileStatusKind::Modified,
                        }],
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
                        changed_files: vec![CommitFileItem {
                            path: std::path::PathBuf::from("stash.txt"),
                            kind: FileStatusKind::Modified,
                        }],
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
    fn pop_selected_stash_opens_confirmation_modal() {
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
                        changed_files: vec![CommitFileItem {
                            path: std::path::PathBuf::from("stash.txt"),
                            kind: FileStatusKind::Modified,
                        }],
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

        let result = reduce(state, Event::Action(Action::PopSelectedStash));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::PopStash {
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
            Some((&ModalKind::Confirm, "Pop stash stash@{0}"))
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
                prompt.value.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateWorktree,
                String::new(),
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn stash_all_changes_opens_input_prompt_from_status_pane() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("tracked.txt"),
                        kind: FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StashAllChanges));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.value.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateStash {
                    mode: StashMode::Tracked,
                },
                String::new(),
                PaneId::RepoUnstaged
            ))
        );
    }

    #[test]
    fn stash_all_changes_warns_when_only_untracked_files_exist() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("untracked.txt"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::StashAllChanges));

        assert!(result.state.pending_input_prompt.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("No tracked changes are available to stash.")
        );
    }

    #[test]
    fn open_stash_options_opens_menu_when_only_untracked_files_exist() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("untracked.txt"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenStashOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::StashOptions,
                0,
                PaneId::RepoUnstaged
            ))
        );
    }

    #[test]
    fn open_merge_rebase_options_opens_menu_from_commit_history() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head123".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old456".to_string(),
                            summary: "Older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenMergeRebaseOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::MergeRebaseOptions,
                0,
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn open_merge_rebase_options_opens_menu_from_branches() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenMergeRebaseOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::MergeRebaseOptions,
                0,
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn open_merge_rebase_options_opens_menu_from_remote_branches() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::RemoteBranches,
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                detail: Some(RepoDetail {
                    remote_branches: vec![crate::state::RemoteBranchItem {
                        name: "origin/feature".to_string(),
                        remote_name: "origin".to_string(),
                        branch_name: "feature".to_string(),
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenMergeRebaseOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::MergeRebaseOptions,
                0,
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn merge_rebase_menu_entries_follow_fast_forward_order_for_mergeable_branch() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    merge_fast_forward_preference: MergeFastForwardPreference::Default,
                    fast_forward_merge_targets: std::collections::BTreeMap::from([(
                        "feature".to_string(),
                        true,
                    )]),
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let entries = merge_rebase_menu_entries(&state);

        assert_eq!(
            entries
                .iter()
                .take(3)
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Merge feature into current branch (regular, fast-forward)",
                "Merge feature into current branch (no-fast-forward)",
                "Squash merge feature into current branch",
            ]
        );
        assert_eq!(
            entries
                .iter()
                .take(3)
                .map(|entry| entry.action.clone())
                .collect::<Vec<_>>(),
            vec![
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::Regular,
                },
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::NoFastForward,
                },
                Action::MergeSelectedRefIntoCurrent {
                    variant: MergeVariant::Squash,
                },
            ]
        );
    }

    #[test]
    fn merge_rebase_menu_entries_disable_fast_forward_when_preference_requires_it() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    branches: vec![
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "feature".to_string(),
                            is_head: false,
                            upstream: Some("origin/feature".to_string()),
                            ..crate::state::BranchItem::default()
                        },
                    ],
                    merge_fast_forward_preference: MergeFastForwardPreference::FastForward,
                    fast_forward_merge_targets: std::collections::BTreeMap::from([(
                        "feature".to_string(),
                        false,
                    )]),
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let entries = merge_rebase_menu_entries(&state);

        assert_eq!(
            entries.first().map(|entry| entry.label.as_str()),
            Some("Merge feature into current branch (regular, fast-forward) [disabled]")
        );
        assert_eq!(
            entries.first().map(|entry| entry.action.clone()),
            Some(Action::ShowWarning {
                message: "Cannot fast-forward merge feature into main.".to_string(),
            })
        );
        assert_eq!(
            entries.get(1).map(|entry| entry.action.clone()),
            Some(Action::MergeSelectedRefIntoCurrent {
                variant: MergeVariant::NoFastForward,
            })
        );
    }

    #[test]
    fn open_bisect_options_opens_menu_from_commit_history() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head123".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old456".to_string(),
                            summary: "Older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenBisectOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((repo_id, MenuOperation::BisectOptions, 0, PaneId::RepoDetail))
        );
    }

    #[test]
    fn submit_merge_rebase_options_selection_dispatches_selected_action() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Merge / rebase options",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::MergeRebaseOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head123".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old456".to_string(),
                            summary: "Older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::StartInteractiveRebase {
                commit: "older".to_string(),
                summary: "old456 Older commit".to_string(),
            })
        );
    }

    #[test]
    fn submit_bisect_options_selection_dispatches_start_and_mark_actions() {
        let repo_id = RepoId::new("repo-1");
        let start_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Bisect options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::BisectOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head123".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old456".to_string(),
                            summary: "Older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let start_result = reduce(start_state, Event::Action(Action::SubmitMenuSelection));

        assert!(start_result.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::RunGitCommand(GitCommandRequest {
                    repo_id: effect_repo_id,
                    command: GitCommand::StartBisect { commit, term },
                    ..
                }) if effect_repo_id == &repo_id && commit == "older" && term == "bad"
            )
        }));
        assert!(start_result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::ScheduleRender)));

        let mark_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Bisect options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::BisectOptions,
                selected_index: 1,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commits_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                detail: Some(RepoDetail {
                    commits: vec![
                        CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head123".to_string(),
                            summary: "HEAD".to_string(),
                            ..CommitItem::default()
                        },
                        CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old456".to_string(),
                            summary: "Older commit".to_string(),
                            ..CommitItem::default()
                        },
                    ],
                    bisect_state: Some(crate::state::BisectState {
                        bad_term: "bad".to_string(),
                        good_term: "good".to_string(),
                        current_commit: Some("candidate".to_string()),
                        current_summary: Some("candidate123 Candidate commit".to_string()),
                    }),
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let mark_result = reduce(mark_state, Event::Action(Action::SubmitMenuSelection));

        assert!(mark_result.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::RunGitCommand(GitCommandRequest {
                    repo_id: effect_repo_id,
                    command: GitCommand::MarkBisect { commit, term },
                    ..
                }) if effect_repo_id == &repo_id && commit == "candidate" && term == "good"
            )
        }));
        assert!(mark_result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::ScheduleRender)));
    }

    #[test]
    fn open_patch_options_opens_menu_from_status_diff() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                diff_line_cursor: Some(2),
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

        let result = reduce(state, Event::Action(Action::OpenPatchOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((repo_id, MenuOperation::PatchOptions, 0, PaneId::RepoDetail))
        );
    }

    #[test]
    fn submit_patch_options_selection_dispatches_selected_patch_action() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Patch options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::PatchOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
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

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));
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
            vec![
                Effect::RunPatchSelection(crate::effect::PatchSelectionJob {
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
                }),
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn submit_stash_options_selection_opens_keep_index_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::StashOptions,
                selected_index: 1,
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("tracked.txt"),
                        kind: FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus,
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateStash {
                    mode: StashMode::KeepIndex,
                },
                PaneId::RepoStaged,
            ))
        );
    }

    #[test]
    fn open_recent_repos_opens_recent_repo_menu() {
        let current_repo_id = RepoId::new("/tmp/current");
        let recent_repo_id = RepoId::new("/tmp/recent");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            recent_repo_stack: vec![recent_repo_id.clone(), current_repo_id.clone()],
            workspace: crate::state::WorkspaceState {
                discovered_repo_ids: vec![recent_repo_id.clone(), current_repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([
                    (recent_repo_id.clone(), workspace_summary(&recent_repo_id.0)),
                    (
                        current_repo_id.clone(),
                        workspace_summary(&current_repo_id.0),
                    ),
                ]),
                selected_repo_id: Some(current_repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState::new(current_repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenRecentRepos));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                current_repo_id,
                MenuOperation::RecentRepos,
                0,
                PaneId::RepoDetail,
            ))
        );
    }

    #[test]
    fn submit_recent_repo_selection_enters_selected_recent_repo() {
        let current_repo_id = RepoId::new("/tmp/current");
        let recent_repo_id = RepoId::new("/tmp/recent");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Recent repositories",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: current_repo_id.clone(),
                operation: MenuOperation::RecentRepos,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            recent_repo_stack: vec![recent_repo_id.clone(), current_repo_id.clone()],
            workspace: crate::state::WorkspaceState {
                discovered_repo_ids: vec![recent_repo_id.clone(), current_repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([
                    (recent_repo_id.clone(), workspace_summary(&recent_repo_id.0)),
                    (
                        current_repo_id.clone(),
                        workspace_summary(&current_repo_id.0),
                    ),
                ]),
                selected_repo_id: Some(current_repo_id),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState::new(RepoId::new("/tmp/current"))),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(recent_repo_id.clone())
        );
        assert_eq!(
            result.state.workspace.selected_repo_id,
            Some(recent_repo_id.clone())
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::LoadRepoDetail { repo_id, .. } if repo_id == &recent_repo_id
        )));
    }

    #[test]
    fn open_command_log_opens_command_log_menu() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            status_messages: std::collections::VecDeque::from([
                crate::state::StatusMessage::info(1, "Ran fetch"),
                crate::state::StatusMessage::info(2, "Ran pull"),
            ]),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenCommandLog));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::CommandLog)
        );
    }

    #[test]
    fn screen_mode_actions_cycle_settings_and_status_text() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..AppState::default()
        };

        let half = reduce(state, Event::Action(Action::NextScreenMode));
        assert_eq!(
            half.state.settings.screen_mode,
            crate::state::ScreenMode::HalfScreen
        );
        assert_eq!(
            half.state
                .status_messages
                .back()
                .map(|message| message.text.as_str()),
            Some("Screen mode: half")
        );
        assert_eq!(half.effects, vec![Effect::ScheduleRender]);

        let fullscreen = reduce(half.state, Event::Action(Action::NextScreenMode));
        assert_eq!(
            fullscreen.state.settings.screen_mode,
            crate::state::ScreenMode::FullScreen
        );

        let normal = reduce(fullscreen.state, Event::Action(Action::NextScreenMode));
        assert_eq!(
            normal.state.settings.screen_mode,
            crate::state::ScreenMode::Normal
        );

        let previous = reduce(normal.state, Event::Action(Action::PreviousScreenMode));
        assert_eq!(
            previous.state.settings.screen_mode,
            crate::state::ScreenMode::FullScreen
        );
    }

    #[test]
    fn open_filter_options_opens_filter_menu_from_commit_history() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenFilterOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::FilterOptions)
        );
    }

    #[test]
    fn open_commit_log_options_opens_menu_from_commit_history() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::History,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenCommitLogOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::CommitLogOptions,
                0,
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn submit_commit_log_options_selection_dispatches_current_branch_history_action() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Commit log options",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitLogOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::History,
                commit_history_mode: CommitHistoryMode::Graph { reverse: true },
                commit_history_ref: Some("feature".to_string()),
                pending_commit_selection_oid: Some("deadbeef".to_string()),
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Linear)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            None
        );
        assert!(result.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::LoadRepoDetail {
                    repo_id: effect_repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ..
                } if effect_repo_id == &repo_id
            )
        }));
    }

    #[test]
    fn submit_filter_options_selection_dispatches_filter_action() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Filter options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::FilterOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(true)
        );
    }

    #[test]
    fn open_diff_options_opens_diff_menu_from_branches() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "main".to_string(),
                        is_head: true,
                        upstream: Some("origin/main".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenDiffOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::DiffOptions)
        );
    }

    #[test]
    fn submit_diff_options_selection_dispatches_selected_action() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Diffing options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::DiffOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![crate::state::BranchItem {
                        name: "main".to_string(),
                        is_head: true,
                        upstream: Some("origin/main".to_string()),
                        ..crate::state::BranchItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_base.clone()),
            Some(crate::state::ComparisonTarget::Branch("main".to_string()))
        );
    }

    #[test]
    fn open_diff_options_opens_diff_menu_from_status() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenDiffOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::DiffOptions)
        );
    }

    #[test]
    fn submit_diff_options_selection_toggles_whitespace_from_status_menu() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Diffing options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::DiffOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.ignore_whitespace_in_diff),
            Some(true)
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: true,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn increase_rename_similarity_threshold_reloads_comparison_diff() {
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
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::IncreaseRenameSimilarityThreshold),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.rename_similarity_threshold),
            Some(crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD + 5)
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
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD
                        + 5,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn submit_stash_options_selection_warns_when_keep_index_scope_is_unavailable() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id,
                operation: MenuOperation::StashOptions,
                selected_index: 1,
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("untracked.txt"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_input_prompt.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("No unstaged tracked changes are available to stash while keeping the index.")
        );
    }

    #[test]
    fn submit_stash_options_selection_opens_include_untracked_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::StashOptions,
                selected_index: 2,
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("untracked.txt"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus,
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateStash {
                    mode: StashMode::IncludeUntracked,
                },
                PaneId::RepoStaged,
            ))
        );
    }

    #[test]
    fn submit_stash_options_selection_opens_staged_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::StashOptions,
                selected_index: 3,
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("staged.txt"),
                        kind: FileStatusKind::Added,
                        staged_kind: Some(FileStatusKind::Added),
                        unstaged_kind: None,
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus,
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateStash {
                    mode: StashMode::Staged,
                },
                PaneId::RepoStaged,
            ))
        );
    }

    #[test]
    fn submit_stash_options_selection_warns_when_staged_scope_is_unavailable() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id,
                operation: MenuOperation::StashOptions,
                selected_index: 3,
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("tracked.txt"),
                        kind: FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_input_prompt.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("No staged changes are available to stash.")
        );
    }

    #[test]
    fn submit_stash_options_selection_opens_unstaged_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::StashOptions,
                selected_index: 4,
                return_focus: PaneId::RepoUnstaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("tracked.txt"),
                        kind: FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus,
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateStash {
                    mode: StashMode::Unstaged,
                },
                PaneId::RepoUnstaged,
            ))
        );
    }

    #[test]
    fn submit_stash_options_selection_warns_when_unstaged_scope_is_unavailable() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Stash options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id,
                operation: MenuOperation::StashOptions,
                selected_index: 4,
                return_focus: PaneId::RepoUnstaged,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("untracked.txt"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_input_prompt.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("No unstaged tracked changes are available to stash.")
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
                        ..WorktreeItem::default()
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
                return_focus: PaneId::RepoDetail,
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
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
    fn submit_branch_checkout_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Check out branch",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CheckoutBranch,
                value: "origin/feature/new-ui".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:checkout-branch");

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
                command: GitCommand::CheckoutBranch {
                    branch_ref: "origin/feature/new-ui".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_create_branch_from_commit_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "New branch name from abcdef1 add lib",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateBranchFromCommit {
                    commit: "abcdef1234567890".to_string(),
                    summary: "abcdef1 add lib".to_string(),
                },
                value: "feature/from-commit".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-branch-from-commit");

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
                command: GitCommand::CreateBranchFromCommit {
                    branch_name: "feature/from-commit".to_string(),
                    commit: "abcdef1234567890".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_create_branch_from_remote_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "New local branch from origin/feature",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateBranchFromRemote {
                    remote_branch_ref: "origin/feature".to_string(),
                    suggested_name: "feature".to_string(),
                },
                value: "feature-local".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-branch-from-ref");

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
                command: GitCommand::CreateBranchFromRef {
                    branch_name: "feature-local".to_string(),
                    start_point: "origin/feature".to_string(),
                    track: false,
                },
            })]
        );
    }

    #[test]
    fn submit_create_branch_from_remote_prompt_tracks_when_using_suggested_name() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "New local branch from origin/feature",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateBranchFromRemote {
                    remote_branch_ref: "origin/feature".to_string(),
                    suggested_name: "feature".to_string(),
                },
                value: "feature".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-branch-from-ref");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CreateBranchFromRef {
                    branch_name: "feature".to_string(),
                    start_point: "origin/feature".to_string(),
                    track: true,
                },
            })]
        );
    }

    #[test]
    fn submit_create_remote_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Add remote",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateRemote,
                value: "upstream git@github.com:example/upstream.git".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:add-remote");

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
                command: GitCommand::AddRemote {
                    remote_name: "upstream".to_string(),
                    remote_url: "git@github.com:example/upstream.git".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_edit_remote_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Edit remote upstream",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::EditRemote {
                    current_name: "upstream".to_string(),
                    current_url: "git@github.com:example/upstream.git".to_string(),
                },
                value: "mirror git@github.com:example/mirror.git".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:edit-remote");

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
                command: GitCommand::EditRemote {
                    current_name: "upstream".to_string(),
                    new_name: "mirror".to_string(),
                    remote_url: "git@github.com:example/mirror.git".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_create_tag_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Create tag",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateTag,
                value: "release-candidate".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-tag");

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
                command: GitCommand::CreateTag {
                    tag_name: "release-candidate".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_create_tag_from_commit_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "New tag name from abcdef1 add lib",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateTagFromCommit {
                    commit: "abcdef1234567890".to_string(),
                    summary: "abcdef1 add lib".to_string(),
                },
                value: "release-candidate".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-tag-from-commit");

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
                command: GitCommand::CreateTagFromCommit {
                    tag_name: "release-candidate".to_string(),
                    commit: "abcdef1234567890".to_string(),
                },
            })]
        );
    }

    #[test]
    fn submit_stash_rename_prompt_queues_git_job_and_allows_empty_messages() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Rename stash: stash@{1}",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::RenameStash {
                    stash_ref: "stash@{1}".to_string(),
                    current_name: "prior experiment".to_string(),
                },
                value: String::new(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:rename-stash");

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
                command: GitCommand::RenameStash {
                    stash_ref: "stash@{1}".to_string(),
                    message: String::new(),
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
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-worktree");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
    fn submit_create_branch_from_stash_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "New branch name (branch is off of 'stash@{1}: On main: foo')",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateBranchFromStash {
                    stash_ref: "stash@{1}".to_string(),
                    stash_label: "stash@{1}: On main: foo".to_string(),
                },
                value: "feature/from-stash".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-branch-from-stash");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CreateBranchFromStash {
                    stash_ref: "stash@{1}".to_string(),
                    branch_name: "feature/from-stash".to_string(),
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
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:start-reword-rebase");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
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
    fn submit_shell_command_prompt_queues_shell_job() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Run shell command",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::ShellCommand,
                value: "git status --short".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("shell:/tmp/repo-1:run-command");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| (&job.kind, &job.state)),
            Some((
                &BackgroundJobKind::ShellCommand,
                &BackgroundJobState::Queued
            ))
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunShellCommand(ShellCommandRequest::new(
                job_id,
                repo_id,
                "git status --short",
            ))]
        );
    }

    #[test]
    fn submit_stash_prompt_queues_git_job_and_restores_original_focus() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Stash tracked changes",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateStash {
                    mode: StashMode::Tracked,
                },
                value: "checkpoint".to_string(),
                return_focus: PaneId::RepoStaged,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-stash");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoStaged);
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CreateStash {
                    message: Some("checkpoint".to_string()),
                    mode: StashMode::Tracked,
                },
            })]
        );
    }

    #[test]
    fn submit_blank_stash_prompt_queues_git_job_without_message() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Stash tracked changes",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: crate::state::InputPromptOperation::CreateStash {
                    mode: StashMode::Tracked,
                },
                value: "   ".to_string(),
                return_focus: PaneId::RepoUnstaged,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-stash");

        assert!(result.state.pending_input_prompt.is_none());
        assert!(result.state.modal_stack.is_empty());
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::CreateStash {
                    message: None,
                    mode: StashMode::Tracked,
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
    fn confirm_pending_operation_queues_pop_stash_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Pop stash stash@{0}",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::PopStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:pop-stash");

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
                command: GitCommand::PopStash {
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
    fn confirm_pending_operation_queues_set_fixup_message_rebase_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Set fixup message from old1234 older commit",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::SetFixupMessageForCommit {
                    commit: "older".to_string(),
                    summary: "old1234 older commit".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:set-fixup-message-rebase");

        assert!(result.state.modal_stack.is_empty());
        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: "older".to_string(),
                    mode: RebaseStartMode::FixupWithMessage,
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_move_up_rebase_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Move old1234 older commit above head HEAD",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::MoveCommitUp {
                    commit: "older".to_string(),
                    adjacent_commit: "head".to_string(),
                    summary: "old1234 older commit".to_string(),
                    adjacent_summary: "head HEAD".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:move-commit-up-rebase");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: "older".to_string(),
                    mode: RebaseStartMode::MoveUp {
                        adjacent_commit: "head".to_string(),
                    },
                },
            })]
        );
    }

    #[test]
    fn confirm_pending_operation_queues_move_down_rebase_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Confirm,
                "Move head HEAD below old1234 older commit",
            )],
            pending_confirmation: Some(crate::state::PendingConfirmation {
                repo_id: repo_id.clone(),
                operation: ConfirmableOperation::MoveCommitDown {
                    commit: "head".to_string(),
                    adjacent_commit: "older".to_string(),
                    summary: "head HEAD".to_string(),
                    adjacent_summary: "old1234 older commit".to_string(),
                },
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..Default::default()
        };

        let result = reduce(state, Event::Action(Action::ConfirmPendingOperation));
        let job_id = JobId::new("git:repo-1:move-commit-down-rebase");

        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: "head".to_string(),
                    mode: RebaseStartMode::MoveDown {
                        adjacent_commit: "older".to_string(),
                    },
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
                    ..FileStatus::default()
                },
                FileStatus {
                    path: std::path::PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Untracked),
                    ..FileStatus::default()
                },
                FileStatus {
                    path: std::path::PathBuf::from("Cargo.toml"),
                    kind: FileStatusKind::Added,
                    staged_kind: Some(FileStatusKind::Added),
                    unstaged_kind: None,
                    ..FileStatus::default()
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
    fn refresh_selected_repo_preserves_selected_branch_commit_history() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            workspace: crate::state::WorkspaceState {
                selected_repo_id: Some(repo_id.clone()),
                ..AppState::default().workspace
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_history_ref: Some("feature".to_string()),
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

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
                    commit_ref: Some("feature".to_string()),
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
            ]
        );
    }

    #[test]
    fn refresh_selected_repo_deep_adds_workspace_scan() {
        let repo_id = RepoId::new("repo-1");
        let state = reduce(
            AppState::default(),
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
        )
        .state;

        let result = reduce(state, Event::Action(Action::RefreshSelectedRepoDeep));

        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RefreshRepoSummary { repo_id: effect_repo_id } if effect_repo_id == &repo_id
        )));
        assert!(result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::StartRepoScan)));
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::LoadRepoDetail { repo_id: effect_repo_id, .. } if effect_repo_id == &repo_id
        )));
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
                            ..crate::state::BranchItem::default()
                        },
                        crate::state::BranchItem {
                            name: "main".to_string(),
                            is_head: true,
                            upstream: Some("origin/main".to_string()),
                            ..crate::state::BranchItem::default()
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
    fn switch_repo_subview_remote_branches_selects_first_visible_match() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(RepoDetail {
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature-contract".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature-contract".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_filter: crate::state::RepoSubviewFilterState {
                    query: "feature".to_string(),
                    focused: false,
                },
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::RemoteBranches)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.remote_branches_view.selected_index),
            Some(1)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
    }

    #[test]
    fn switch_repo_subview_remotes_selects_first_visible_match() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(RepoDetail {
                    remotes: vec![
                        crate::state::RemoteItem {
                            name: "origin".to_string(),
                            fetch_url: "/tmp/origin.git".to_string(),
                            push_url: "/tmp/origin.git".to_string(),
                            branch_count: 2,
                        },
                        crate::state::RemoteItem {
                            name: "upstream".to_string(),
                            fetch_url: "/tmp/upstream.git".to_string(),
                            push_url: "/tmp/upstream.git".to_string(),
                            branch_count: 0,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remotes_filter: crate::state::RepoSubviewFilterState {
                    query: "up".to_string(),
                    focused: false,
                },
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Remotes)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.remotes_view.selected_index),
            Some(1)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
    }

    #[test]
    fn switch_repo_subview_tags_selects_first_visible_match() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(RepoDetail {
                    tags: vec![
                        crate::state::TagItem {
                            name: "v1.0.0".to_string(),
                            target_oid: "abcdef1234567890".to_string(),
                            target_short_oid: "abcdef1".to_string(),
                            summary: "release v1.0.0".to_string(),
                            annotated: true,
                        },
                        crate::state::TagItem {
                            name: "release-candidate".to_string(),
                            target_oid: "1234567890abcdef".to_string(),
                            target_short_oid: "1234567".to_string(),
                            summary: "second".to_string(),
                            annotated: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                tags_filter: crate::state::RepoSubviewFilterState {
                    query: "candidate".to_string(),
                    focused: false,
                },
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::SwitchRepoSubview(RepoSubview::Tags)),
        );

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.tags_view.selected_index),
            Some(1)
        );
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
    }

    #[test]
    fn create_local_branch_from_selected_remote_branch_opens_prompt_with_suggested_name() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(crate::state::RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(RepoDetail {
                    remote_branches: vec![
                        crate::state::RemoteBranchItem {
                            name: "origin/main".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "main".to_string(),
                        },
                        crate::state::RemoteBranchItem {
                            name: "origin/feature-contract".to_string(),
                            remote_name: "origin".to_string(),
                            branch_name: "feature-contract".to_string(),
                        },
                    ],
                    ..RepoDetail::default()
                }),
                remote_branches_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::CreateLocalBranchFromSelectedRemoteBranch),
        );

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| {
                (
                    prompt.repo_id.clone(),
                    prompt.operation.clone(),
                    prompt.value.clone(),
                )
            }),
            Some((
                repo_id,
                InputPromptOperation::CreateBranchFromRemote {
                    remote_branch_ref: "origin/feature-contract".to_string(),
                    suggested_name: "feature-contract".to_string(),
                },
                "feature-contract".to_string(),
            ))
        );
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
                    ..CommitItem::default()
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
                    ..CommitItem::default()
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
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    path: std::path::PathBuf::from("src/lib.rs")
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
                    path: std::path::PathBuf::from("src/lib.rs"),
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
                summary: "Stage src/lib.rs".to_string(),
            })
        );
    }

    #[test]
    fn open_selected_status_entry_collapses_selected_directory() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                status_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedStatusEntry));

        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result.state.repo_mode.as_ref().map(|repo_mode| repo_mode
                .collapsed_status_dirs
                .contains(std::path::Path::new("src"))),
            Some(true)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn open_selected_status_entry_focuses_detail_for_files() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(crate::state::RepoModeState {
                detail: Some(repo_detail_with_file_tree()),
                status_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..crate::state::RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedStatusEntry));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.collapsed_status_dirs.is_empty()),
            Some(true)
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
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
                    path: std::path::PathBuf::from("src/lib.rs"),
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
                summary: "Unstage src/lib.rs".to_string(),
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
            ..CommitItem::default()
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
                    ..crate::state::BranchItem::default()
                },
                crate::state::BranchItem {
                    name: "main".to_string(),
                    is_head: true,
                    upstream: Some("origin/main".to_string()),
                    ..crate::state::BranchItem::default()
                },
            ],
            file_tree: vec![
                FileStatus {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: Some(FileStatusKind::Modified),
                    unstaged_kind: Some(FileStatusKind::Modified),
                    ..FileStatus::default()
                },
                FileStatus {
                    path: std::path::PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Untracked),
                    ..FileStatus::default()
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
                ..CommitItem::default()
            }],
            reflog_items: vec![ReflogItem {
                selector: "HEAD@{0}".to_string(),
                oid: "abcdef1234567890".to_string(),
                short_oid: "abcdef1".to_string(),
                unix_timestamp: 0,
                summary: "commit: add lib".to_string(),
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
    fn create_tag_from_selected_commit_opens_prompt_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::CreateTagFromSelectedCommit));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_input_prompt
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(InputPromptOperation::CreateTagFromCommit {
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
    fn create_fixup_commit_requires_staged_changes() {
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

        let result = reduce(state, Event::Action(Action::CreateFixupCommit));

        assert!(result.state.background_jobs.is_empty());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Stage changes before creating a fixup commit.")
        );
    }

    #[test]
    fn create_fixup_commit_queues_git_job_for_selected_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("notes.md"),
                        kind: FileStatusKind::Modified,
                        staged_kind: Some(FileStatusKind::Modified),
                        unstaged_kind: None,
                        ..FileStatus::default()
                    }],
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

        let result = reduce(state, Event::Action(Action::CreateFixupCommit));
        let job_id = JobId::new("git:repo-1:create-fixup-commit");

        assert_eq!(
            result.state.background_jobs.get(&job_id).map(|job| (
                job.kind,
                job.target_repo.clone(),
                job.state.clone()
            )),
            Some((
                BackgroundJobKind::GitCommand,
                Some(repo_id.clone()),
                BackgroundJobState::Queued
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Create fixup commit for old1234 older commit".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::RunGitCommand(GitCommandRequest {
                    job_id,
                    repo_id,
                    command: GitCommand::CreateFixupCommit {
                        commit: "older".to_string(),
                    },
                }),
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn open_commit_fixup_options_opens_menu_for_selected_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
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

        let result = reduce(state, Event::Action(Action::OpenCommitFixupOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::CommitFixupOptions,
                0,
                PaneId::RepoDetail,
            ))
        );
    }

    #[test]
    fn open_commit_copy_options_opens_menu_for_selected_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "older".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenCommitCopyOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::CommitCopyOptions,
                0,
                PaneId::RepoDetail,
            ))
        );
    }

    #[test]
    fn open_commit_amend_attribute_options_opens_menu_for_selected_commit() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "older".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(
            state,
            Event::Action(Action::OpenCommitAmendAttributeOptions),
        );

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.repo_id.clone(),
                menu.operation,
                menu.selected_index,
                menu.return_focus,
            )),
            Some((
                repo_id,
                MenuOperation::CommitAmendAttributeOptions,
                0,
                PaneId::RepoDetail,
            ))
        );
    }

    #[test]
    fn submit_commit_copy_options_selection_queues_shell_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Copy commit attribute",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitCopyOptions,
                selected_index: 1,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "older-full-hash".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));
        let job_id = JobId::new("shell:repo-1:run-command");

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RunShellCommand(ShellCommandRequest {
                repo_id: effect_repo_id,
                command,
                ..
            }) if effect_repo_id == &repo_id && command.contains("older-full-hash")
        )));
    }

    #[test]
    fn submit_commit_amend_attribute_selection_queues_reset_author_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Amend commit attributes",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitAmendAttributeOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "older-full-hash".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));
        let job_id = JobId::new("git:repo-1:amend-commit-reset-author");

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RunGitCommand(GitCommandRequest {
                repo_id: effect_repo_id,
                command: GitCommand::AmendCommitAttributes {
                    commit,
                    reset_author: true,
                    co_author: None,
                },
                ..
            }) if effect_repo_id == &repo_id && commit == "older-full-hash"
        )));
    }

    #[test]
    fn submit_commit_amend_attribute_selection_opens_coauthor_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::Menu,
                "Amend commit attributes",
            )],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitAmendAttributeOptions,
                selected_index: 1,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "older-full-hash".to_string(),
                        short_oid: "old1234".to_string(),
                        summary: "older commit".to_string(),
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::SetCommitCoAuthor {
                    commit: "older-full-hash".to_string(),
                    summary: "old1234 older commit".to_string(),
                },
                PaneId::RepoDetail,
            ))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }

    #[test]
    fn submit_commit_fixup_options_selection_queues_direct_fixup_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Fixup options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitFixupOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("notes.md"),
                        kind: FileStatusKind::Modified,
                        staged_kind: Some(FileStatusKind::Modified),
                        unstaged_kind: None,
                        ..FileStatus::default()
                    }],
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

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));
        let job_id = JobId::new("git:repo-1:create-fixup-commit");

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RunGitCommand(GitCommandRequest {
                repo_id: effect_repo_id,
                command: GitCommand::CreateFixupCommit { commit },
                ..
            }) if effect_repo_id == &repo_id && commit == "older"
        )));
    }

    #[test]
    fn submit_commit_fixup_options_selection_opens_amend_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(ModalKind::Menu, "Fixup options")],
            pending_menu: Some(crate::state::PendingMenu {
                repo_id: repo_id.clone(),
                operation: MenuOperation::CommitFixupOptions,
                selected_index: 1,
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    file_tree: vec![FileStatus {
                        path: std::path::PathBuf::from("notes.md"),
                        kind: FileStatusKind::Modified,
                        staged_kind: Some(FileStatusKind::Modified),
                        unstaged_kind: None,
                        ..FileStatus::default()
                    }],
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

        let result = reduce(state, Event::Action(Action::SubmitMenuSelection));

        assert!(result.state.pending_menu.is_none());
        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateAmendCommit {
                    summary: "old1234 older commit".to_string(),
                    original_subject: "older commit".to_string(),
                    include_file_changes: true,
                    initial_message: "older commit".to_string(),
                },
                PaneId::RepoDetail,
            ))
        );
    }

    #[test]
    fn submit_create_amend_commit_prompt_queues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![crate::state::Modal::new(
                ModalKind::InputPrompt,
                "Create amend! commit without changes for old1234 older commit",
            )],
            pending_input_prompt: Some(crate::state::PendingInputPrompt {
                repo_id: repo_id.clone(),
                operation: InputPromptOperation::CreateAmendCommit {
                    summary: "old1234 older commit".to_string(),
                    original_subject: "older commit".to_string(),
                    include_file_changes: false,
                    initial_message: "older commit".to_string(),
                },
                value: "rewritten subject".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SubmitPromptInput));
        let job_id = JobId::new("git:repo-1:create-amend-commit-without-changes");

        assert!(result.state.pending_input_prompt.is_none());
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .background_jobs
                .get(&job_id)
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            Effect::RunGitCommand(GitCommandRequest {
                repo_id: effect_repo_id,
                command: GitCommand::CreateAmendCommit {
                    original_subject,
                    message,
                    include_file_changes,
                },
                ..
            }) if effect_repo_id == &repo_id
                && original_subject == "older commit"
                && message == "rewritten subject"
                && !include_file_changes
        )));
    }

    #[test]
    fn apply_fixup_commits_requires_older_commit_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
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
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ApplyFixupCommits));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select an older commit before applying fixups.")
        );
    }

    #[test]
    fn apply_fixup_commits_opens_confirmation_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::ApplyFixupCommits));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::ApplyFixupCommits {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn set_fixup_message_opens_confirmation_for_selected_commit() {
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

        let result = reduce(
            state,
            Event::Action(Action::SetFixupMessageForSelectedCommit),
        );

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::SetFixupMessageForCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn squash_selected_commit_requires_older_commit_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
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
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::SquashSelectedCommit));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select an older commit before starting squash.")
        );
    }

    #[test]
    fn squash_selected_commit_opens_confirmation_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::SquashSelectedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::SquashCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn drop_selected_commit_requires_older_commit_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
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
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DropSelectedCommit));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select an older commit before dropping it.")
        );
    }

    #[test]
    fn drop_selected_commit_opens_confirmation_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::DropSelectedCommit));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::DropCommit {
                commit: "older".to_string(),
                summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn move_selected_commit_up_requires_commit_above_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
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
                commits_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::MoveSelectedCommitUp));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select a commit below another commit before moving it up.")
        );
    }

    #[test]
    fn move_selected_commit_up_opens_confirmation_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::MoveSelectedCommitUp));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::MoveCommitUp {
                commit: "older".to_string(),
                adjacent_commit: "head".to_string(),
                summary: "old1234 older commit".to_string(),
                adjacent_summary: "head HEAD".to_string(),
            })
        );
    }

    #[test]
    fn move_selected_commit_down_requires_commit_below_selection() {
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

        let result = reduce(state, Event::Action(Action::MoveSelectedCommitDown));

        assert!(result.state.pending_confirmation.is_none());
        assert_eq!(
            result
                .state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("Select a commit above another commit before moving it down.")
        );
    }

    #[test]
    fn move_selected_commit_down_opens_confirmation_for_selected_commit() {
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
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::MoveSelectedCommitDown));

        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(ConfirmableOperation::MoveCommitDown {
                commit: "head".to_string(),
                adjacent_commit: "older".to_string(),
                summary: "head HEAD".to_string(),
                adjacent_summary: "old1234 older commit".to_string(),
            })
        );
    }

    #[test]
    fn open_selected_commit_files_switches_into_file_mode() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "head".to_string(),
                        short_oid: "head".to_string(),
                        summary: "HEAD".to_string(),
                        changed_files: vec![
                            CommitFileItem {
                                path: std::path::PathBuf::from("src/lib.rs"),
                                kind: FileStatusKind::Modified,
                            },
                            CommitFileItem {
                                path: std::path::PathBuf::from("notes.md"),
                                kind: FileStatusKind::Added,
                            },
                        ],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedCommitFiles));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_subview_mode),
            Some(crate::state::CommitSubviewMode::Files)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_files_mode),
            Some(crate::state::CommitFilesMode::List)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_files_view.selected_index),
            Some(0)
        );
    }

    #[test]
    fn open_selected_commit_files_from_file_list_loads_commit_scoped_file_diff() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::List,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![CommitFileItem {
                            path: std::path::PathBuf::from("src/lib.rs"),
                            kind: FileStatusKind::Modified,
                        }],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSelectedCommitFiles));

        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDiff {
                    repo_id: repo_id.clone(),
                    comparison_target: Some(crate::state::ComparisonTarget::Commit(
                        "abcdef1234567890^!".to_string()
                    )),
                    compare_with: None,
                    selected_path: Some(std::path::PathBuf::from("src/lib.rs")),
                    diff_presentation: DiffPresentation::Comparison,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
        assert_eq!(
            result.state.repo_mode.as_ref().map(|repo_mode| {
                (
                    repo_mode.commit_subview_mode,
                    repo_mode.commit_files_mode,
                    repo_mode
                        .detail
                        .as_ref()
                        .and_then(|detail| detail.diff.selected_path.clone()),
                )
            }),
            Some((
                crate::state::CommitSubviewMode::Files,
                crate::state::CommitFilesMode::Diff,
                Some(std::path::PathBuf::from("src/lib.rs")),
            ))
        );
    }

    #[test]
    fn close_selected_commit_files_from_diff_returns_to_file_list() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::Diff,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CloseSelectedCommitFiles));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| (repo_mode.commit_subview_mode, repo_mode.commit_files_mode)),
            Some((
                crate::state::CommitSubviewMode::Files,
                crate::state::CommitFilesMode::List
            ))
        );
    }

    #[test]
    fn close_selected_commit_files_from_file_list_returns_to_history() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                commit_files_mode: crate::state::CommitFilesMode::List,
                detail: Some(RepoDetail::default()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CloseSelectedCommitFiles));

        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| (repo_mode.commit_subview_mode, repo_mode.commit_files_mode)),
            Some((
                crate::state::CommitSubviewMode::History,
                crate::state::CommitFilesMode::List
            ))
        );
    }

    #[test]
    fn checkout_selected_commit_queues_job_for_selected_commit() {
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

        let result = reduce(state, Event::Action(Action::CheckoutSelectedCommit));

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&JobId::new("git:repo-1:checkout-commit"))
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:checkout-commit"),
                repo_id,
                command: GitCommand::CheckoutCommit {
                    commit: "older".to_string(),
                },
            })]
        );
    }

    #[test]
    fn checkout_selected_commit_file_queues_job_for_selected_file() {
        let repo_id = RepoId::new("repo-1");
        let selected_path = std::path::PathBuf::from("src/lib.rs");
        let state = AppState {
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: crate::state::CommitSubviewMode::Files,
                detail: Some(RepoDetail {
                    commits: vec![CommitItem {
                        oid: "abcdef1234567890".to_string(),
                        short_oid: "abcdef1".to_string(),
                        summary: "add lib".to_string(),
                        changed_files: vec![
                            CommitFileItem {
                                path: selected_path.clone(),
                                kind: FileStatusKind::Added,
                            },
                            CommitFileItem {
                                path: std::path::PathBuf::from("notes.md"),
                                kind: FileStatusKind::Added,
                            },
                        ],
                        ..CommitItem::default()
                    }],
                    ..RepoDetail::default()
                }),
                commit_files_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CheckoutSelectedCommitFile));

        assert_eq!(
            result
                .state
                .background_jobs
                .get(&JobId::new("git:repo-1:checkout-commit-file"))
                .map(|job| &job.state),
            Some(&BackgroundJobState::Queued)
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id: JobId::new("git:repo-1:checkout-commit-file"),
                repo_id,
                command: GitCommand::CheckoutCommitFile {
                    commit: "abcdef1234567890".to_string(),
                    path: selected_path,
                },
            })]
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
                        ..FileStatus::default()
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
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

    #[test]
    fn leave_repo_mode_returns_to_parent_submodule_repo() {
        let parent_repo_id = RepoId::new("/tmp/repo-1");
        let child_repo_id = RepoId::new("/tmp/repo-1/vendor/child-module");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: child_repo_id,
                active_subview: RepoSubview::Status,
                parent_repo_ids: vec![parent_repo_id.clone()],
                ..RepoModeState::new(RepoId::new("/tmp/repo-1/vendor/child-module"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::LeaveRepoMode));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(parent_repo_id.clone())
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Submodules)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.parent_repo_ids.clone()),
            Some(Vec::new())
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id: parent_repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn activate_repo_subview_selection_enters_selected_submodule_repo() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let target_repo_id = RepoId::new("/tmp/repo-1/vendor/ui-kit");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![
                        SubmoduleItem {
                            name: "child-module".to_string(),
                            path: std::path::PathBuf::from("vendor/child-module"),
                            url: "../child-module.git".to_string(),
                            branch: Some("main".to_string()),
                            short_oid: Some("abcdef1".to_string()),
                            initialized: true,
                            dirty: false,
                            conflicted: false,
                        },
                        SubmoduleItem {
                            name: "ui-kit".to_string(),
                            path: std::path::PathBuf::from("vendor/ui-kit"),
                            url: "git@github.com:example/ui-kit.git".to_string(),
                            branch: None,
                            short_oid: None,
                            initialized: false,
                            dirty: false,
                            conflicted: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::ActivateRepoSubviewSelection));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(target_repo_id.clone())
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.parent_repo_ids.clone()),
            Some(vec![RepoId::new("/tmp/repo-1")])
        );
        assert_eq!(
            result.effects,
            vec![
                Effect::LoadRepoDetail {
                    repo_id: target_repo_id,
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: crate::state::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold: crate::state::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn submodule_filter_lifecycle_updates_visible_selection() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![
                        SubmoduleItem {
                            name: "child-module".to_string(),
                            path: std::path::PathBuf::from("vendor/child-module"),
                            url: "../child-module.git".to_string(),
                            branch: Some("main".to_string()),
                            short_oid: Some("abcdef1".to_string()),
                            initialized: true,
                            dirty: false,
                            conflicted: false,
                        },
                        SubmoduleItem {
                            name: "ui-kit".to_string(),
                            path: std::path::PathBuf::from("vendor/ui-kit"),
                            url: "git@github.com:example/ui-kit.git".to_string(),
                            branch: None,
                            short_oid: None,
                            initialized: false,
                            dirty: false,
                            conflicted: false,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..AppState::default()
        };

        let focused = reduce(state, Event::Action(Action::FocusRepoSubviewFilter));
        let filtered = reduce(
            focused.state,
            Event::Action(Action::AppendRepoSubviewFilter {
                text: "child".to_string(),
            }),
        );

        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.submodules_filter.query.as_str()),
            Some("child")
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.submodules_view.selected_index),
            Some(Some(0))
        );

        let blurred = reduce(filtered.state, Event::Action(Action::BlurRepoSubviewFilter));
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.submodules_filter.focused),
            Some(false)
        );

        let cancelled = reduce(
            blurred.state,
            Event::Action(Action::CancelRepoSubviewFilter),
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.submodules_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            cancelled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.submodules_view.selected_index),
            Some(Some(0))
        );
    }

    #[test]
    fn open_bulk_submodule_options_opens_menu_without_selection() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                ..RepoModeState::new(repo_id)
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::OpenSubmoduleOptions));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(MenuOperation::BulkSubmoduleOptions)
        );
    }

    #[test]
    fn init_all_submodules_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::InitAllSubmodules));
        let job_id = JobId::new("git:repo-1:init-all-submodules");

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Initialize all submodules".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::InitAllSubmodules,
            })]
        );
    }

    #[test]
    fn update_all_submodules_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::UpdateAllSubmodules));
        let job_id = JobId::new("git:repo-1:update-all-submodules");

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Update all submodules".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::UpdateAllSubmodules,
            })]
        );
    }

    #[test]
    fn update_all_submodules_recursively_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::UpdateAllSubmodulesRecursively));
        let job_id = JobId::new("git:repo-1:update-all-submodules-recursively");

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Update all submodules recursively".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::UpdateAllSubmodulesRecursively,
            })]
        );
    }

    #[test]
    fn deinit_all_submodules_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                ..RepoModeState::new(repo_id.clone())
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::DeinitAllSubmodules));
        let job_id = JobId::new("git:repo-1:deinit-all-submodules");

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Deinitialize all submodules".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::DeinitAllSubmodules,
            })]
        );
    }

    #[test]
    fn create_submodule_opens_input_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState::new(repo_id.clone())),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::CreateSubmodule));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.value.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::CreateSubmodule,
                String::new(),
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn edit_selected_submodule_opens_input_prompt() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![SubmoduleItem {
                        name: "child-module".to_string(),
                        path: std::path::PathBuf::from("vendor/child-module"),
                        url: "../child-module.git".to_string(),
                        branch: Some("main".to_string()),
                        short_oid: Some("abcdef1".to_string()),
                        initialized: true,
                        dirty: false,
                        conflicted: false,
                    }],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::EditSelectedSubmodule));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result.state.pending_input_prompt.as_ref().map(|prompt| (
                prompt.repo_id.clone(),
                prompt.operation.clone(),
                prompt.value.clone(),
                prompt.return_focus
            )),
            Some((
                repo_id,
                InputPromptOperation::EditSubmoduleUrl {
                    name: "child-module".to_string(),
                    path: std::path::PathBuf::from("vendor/child-module"),
                    current_url: "../child-module.git".to_string(),
                },
                "../child-module.git".to_string(),
                PaneId::RepoDetail
            ))
        );
    }

    #[test]
    fn init_selected_submodule_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![SubmoduleItem {
                        name: "ui-kit".to_string(),
                        path: std::path::PathBuf::from("vendor/ui-kit"),
                        url: "git@github.com:example/ui-kit.git".to_string(),
                        branch: None,
                        short_oid: None,
                        initialized: false,
                        dirty: false,
                        conflicted: false,
                    }],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::InitSelectedSubmodule));
        let job_id = JobId::new("git:repo-1:init-submodule");

        assert_eq!(
            result.state.background_jobs.get(&job_id).map(|job| (
                job.kind,
                job.target_repo.clone(),
                job.state.clone()
            )),
            Some((
                BackgroundJobKind::GitCommand,
                Some(repo_id.clone()),
                BackgroundJobState::Queued
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Initialize submodule vendor/ui-kit".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::InitSubmodule {
                    path: std::path::PathBuf::from("vendor/ui-kit"),
                },
            })]
        );
    }

    #[test]
    fn update_selected_submodule_enqueues_git_job() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![SubmoduleItem {
                        name: "child-module".to_string(),
                        path: std::path::PathBuf::from("vendor/child-module"),
                        url: "../child-module.git".to_string(),
                        branch: Some("main".to_string()),
                        short_oid: Some("abcdef1".to_string()),
                        initialized: true,
                        dirty: false,
                        conflicted: false,
                    }],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::UpdateSelectedSubmodule));
        let job_id = JobId::new("git:repo-1:update-submodule");

        assert_eq!(
            result.state.background_jobs.get(&job_id).map(|job| (
                job.kind,
                job.target_repo.clone(),
                job.state.clone()
            )),
            Some((
                BackgroundJobKind::GitCommand,
                Some(repo_id.clone()),
                BackgroundJobState::Queued
            ))
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.operation_progress.clone()),
            Some(crate::state::OperationProgress::Running {
                job_id: job_id.clone(),
                summary: "Update submodule vendor/child-module".to_string(),
            })
        );
        assert_eq!(
            result.effects,
            vec![Effect::RunGitCommand(GitCommandRequest {
                job_id,
                repo_id,
                command: GitCommand::UpdateSubmodule {
                    path: std::path::PathBuf::from("vendor/child-module"),
                },
            })]
        );
    }

    #[test]
    fn remove_selected_submodule_opens_confirmation_modal() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Submodules,
                detail: Some(RepoDetail {
                    submodules: vec![SubmoduleItem {
                        name: "child-module".to_string(),
                        path: std::path::PathBuf::from("vendor/child-module"),
                        url: "../child-module.git".to_string(),
                        branch: Some("main".to_string()),
                        short_oid: Some("abcdef1".to_string()),
                        initialized: true,
                        dirty: false,
                        conflicted: false,
                    }],
                    ..RepoDetail::default()
                }),
                submodules_view: crate::state::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..AppState::default()
        };

        let result = reduce(state, Event::Action(Action::RemoveSelectedSubmodule));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| (pending.repo_id.clone(), pending.operation.clone())),
            Some((
                repo_id,
                ConfirmableOperation::RemoveSubmodule {
                    name: "child-module".to_string(),
                    path: std::path::PathBuf::from("vendor/child-module"),
                }
            ))
        );
        assert_eq!(
            result
                .state
                .modal_stack
                .last()
                .map(|modal| (modal.kind, modal.title.as_str())),
            Some((ModalKind::Confirm, "Remove submodule child-module"))
        );
        assert_eq!(result.effects, vec![Effect::ScheduleRender]);
    }
}
