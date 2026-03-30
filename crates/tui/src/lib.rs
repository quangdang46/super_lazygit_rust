use std::{collections::BTreeMap, time::Instant};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
    Frame,
};
use super_lazygit_config::AppConfig;
use super_lazygit_core::{
    reduce, workspace_attention_score, Action, AppMode, AppState, CommitBoxMode, Diagnostics,
    DiagnosticsSnapshot, DiffLineKind, DiffPresentation, Event, InputEvent, KeyPress, PaneId,
    ReduceResult, RepoDetail, RepoId, RepoModeState, RepoSubview, RepoSummary,
};

#[derive(Debug)]
pub struct TuiApp {
    state: AppState,
    config: AppConfig,
    keybinding_overrides: BTreeMap<String, Vec<String>>,
    diagnostics: Diagnostics,
    viewport: Viewport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 120,
            height: 32,
        }
    }
}

impl TuiApp {
    #[must_use]
    pub fn new(mut state: AppState, config: AppConfig) -> Self {
        state.workspace.ensure_visible_selection();
        let keybinding_overrides = compile_keybinding_overrides(&config);
        Self {
            state,
            config,
            keybinding_overrides,
            diagnostics: Diagnostics::default(),
            viewport: Viewport::default(),
        }
    }

    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
    }

    #[must_use]
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn dispatch(&mut self, event: Event) -> ReduceResult {
        match event {
            Event::Input(input) => self.handle_input(input),
            other => {
                let result = reduce(self.state.clone(), other);
                self.state = result.state.clone();
                result
            }
        }
    }

    #[must_use]
    pub fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
    }

    #[must_use]
    pub fn render(&mut self) -> Buffer {
        let started_at = Instant::now();
        let area = Rect::new(
            0,
            0,
            self.viewport.width.max(1),
            self.viewport.height.max(1),
        );
        let mut buffer = Buffer::empty(area);
        let theme = Theme::from_config(&self.config);

        Block::default()
            .style(Style::default().bg(theme.background).fg(theme.foreground))
            .render(area, &mut buffer);

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(if self.state.settings.show_help_footer {
                    2
                } else {
                    1
                }),
            ])
            .split(area);

        self.render_mode(vertical[0], &mut buffer, theme);
        self.render_status_bar(vertical[1], &mut buffer, theme);

        if let Some(modal) = self.state.modal_stack.last() {
            let modal_area = centered_rect(area, 72, 45);
            Clear.render(modal_area, &mut buffer);
            Paragraph::new(self.modal_lines(modal, theme))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Modal")
                        .border_style(self.pane_style(PaneId::Modal, theme)),
                )
                .alignment(Alignment::Left)
                .render(modal_area, &mut buffer);
        }

        self.diagnostics
            .record_render("shell.frame", started_at.elapsed());
        buffer
    }

    #[must_use]
    pub fn render_to_string(&mut self) -> String {
        let buffer = self.render();
        buffer_to_string(&buffer)
    }

    pub fn draw_frame(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        if self.viewport.width != area.width || self.viewport.height != area.height {
            self.resize(area.width, area.height);
        }

        let buffer = self.render();
        let frame_buffer = frame.buffer_mut();

        for y in 0..area.height {
            for x in 0..area.width {
                let Some(source) = buffer.cell((x, y)) else {
                    continue;
                };
                if let Some(target) = frame_buffer.cell_mut((area.x + x, area.y + y)) {
                    *target = source.clone();
                }
            }
        }
    }

    fn binding_matches_action(
        &self,
        action_id: &str,
        raw: &str,
        normalized: &str,
        defaults: &[&str],
    ) -> bool {
        let canonical_action = canonicalize_action_id(action_id);
        if let Some(bindings) = self.keybinding_overrides.get(&canonical_action) {
            return bindings
                .iter()
                .any(|binding| binding_matches_key(binding, raw, normalized));
        }

        defaults
            .iter()
            .copied()
            .any(|binding| binding_matches_key(binding, raw, normalized))
    }

    fn handle_input(&mut self, input: InputEvent) -> ReduceResult {
        match input {
            InputEvent::Resize { width, height } => {
                self.resize(width, height);
                ReduceResult {
                    state: self.state.clone(),
                    effects: vec![super_lazygit_core::Effect::ScheduleRender],
                }
            }
            InputEvent::KeyPressed(key) => {
                if let Some(action) = self.route_key(key) {
                    let result = reduce(self.state.clone(), Event::Action(action));
                    self.state = result.state.clone();
                    result
                } else {
                    ReduceResult {
                        state: self.state.clone(),
                        effects: Vec::new(),
                    }
                }
            }
            InputEvent::Paste(text) => {
                if self.commit_box_focused() && !text.is_empty() {
                    let result = reduce(
                        self.state.clone(),
                        Event::Action(Action::AppendCommitInput { text }),
                    );
                    self.state = result.state.clone();
                    result
                } else if self.prompt_input_focused() && !text.is_empty() {
                    let result = reduce(
                        self.state.clone(),
                        Event::Action(Action::AppendPromptInput { text }),
                    );
                    self.state = result.state.clone();
                    result
                } else if self.workspace_search_focused() && !text.is_empty() {
                    let result = reduce(
                        self.state.clone(),
                        Event::Action(Action::AppendWorkspaceSearch { text }),
                    );
                    self.state = result.state.clone();
                    result
                } else {
                    ReduceResult {
                        state: self.state.clone(),
                        effects: Vec::new(),
                    }
                }
            }
        }
    }

    fn route_key(&self, key: KeyPress) -> Option<Action> {
        let raw = key.key.as_str();
        let normalized = raw.trim().to_ascii_lowercase();

        if !self.state.modal_stack.is_empty() {
            return match self.state.modal_stack.last().map(|modal| modal.kind) {
                Some(super_lazygit_core::ModalKind::Confirm) => {
                    if self.binding_matches_action(
                        "confirm_pending_operation",
                        raw,
                        &normalized,
                        &["enter", "y"],
                    ) {
                        Some(Action::ConfirmPendingOperation)
                    } else if self.binding_matches_action(
                        "close_top_modal",
                        raw,
                        &normalized,
                        &["esc", "q", "n"],
                    ) {
                        Some(Action::CloseTopModal)
                    } else {
                        None
                    }
                }
                Some(super_lazygit_core::ModalKind::InputPrompt) => {
                    if self.binding_matches_action(
                        "close_top_modal",
                        raw,
                        &normalized,
                        &["esc", "q"],
                    ) {
                        Some(Action::CloseTopModal)
                    } else if self.binding_matches_action(
                        "submit_prompt_input",
                        raw,
                        &normalized,
                        &["enter"],
                    ) {
                        Some(Action::SubmitPromptInput)
                    } else if self.binding_matches_action(
                        "backspace_prompt_input",
                        raw,
                        &normalized,
                        &["backspace"],
                    ) {
                        Some(Action::BackspacePromptInput)
                    } else if raw == "space" || raw == " " {
                        Some(Action::AppendPromptInput {
                            text: " ".to_string(),
                        })
                    } else if raw.chars().count() == 1 {
                        Some(Action::AppendPromptInput {
                            text: raw.to_string(),
                        })
                    } else {
                        None
                    }
                }
                Some(super_lazygit_core::ModalKind::Menu) => {
                    if self.binding_matches_action(
                        "close_top_modal",
                        raw,
                        &normalized,
                        &["esc", "q"],
                    ) {
                        Some(Action::CloseTopModal)
                    } else if self.binding_matches_action(
                        "submit_menu_selection",
                        raw,
                        &normalized,
                        &["enter"],
                    ) {
                        Some(Action::SubmitMenuSelection)
                    } else if self.binding_matches_action(
                        "select_next_menu_item",
                        raw,
                        &normalized,
                        &["j", "down"],
                    ) {
                        Some(Action::SelectNextMenuItem)
                    } else if self.binding_matches_action(
                        "select_previous_menu_item",
                        raw,
                        &normalized,
                        &["k", "up"],
                    ) {
                        Some(Action::SelectPreviousMenuItem)
                    } else {
                        None
                    }
                }
                _ => {
                    if self.binding_matches_action(
                        "close_top_modal",
                        raw,
                        &normalized,
                        &["esc", "q"],
                    ) {
                        Some(Action::CloseTopModal)
                    } else {
                        None
                    }
                }
            };
        };

        if self.commit_box_focused() {
            return self.route_commit_box_key(raw, &normalized);
        }

        let trimmed = raw.trim();

        if self.binding_matches_action("open_help", raw, &normalized, &["?"]) {
            return Some(Action::OpenModal {
                kind: super_lazygit_core::ModalKind::Help,
                title: "Help".to_string(),
            });
        }

        if self.binding_matches_action("next_focus", raw, &normalized, &["tab"]) {
            return self.next_focus_action();
        }

        if self.binding_matches_action("previous_focus", raw, &normalized, &["shift+tab"]) {
            return self.previous_focus_action();
        }

        if matches!(self.state.mode, AppMode::Repository)
            && self.binding_matches_action("leave_repo_mode", raw, &normalized, &["esc"])
        {
            return Some(Action::LeaveRepoMode);
        }

        match self.state.mode {
            AppMode::Workspace => self.route_workspace_key(raw, &normalized),
            AppMode::Repository => self.route_repo_key(trimmed, &normalized),
        }
    }

    fn route_workspace_key(&self, raw: &str, normalized: &str) -> Option<Action> {
        if self.workspace_search_focused() {
            if self.binding_matches_action("cancel_workspace_search", raw, normalized, &["esc"]) {
                return Some(Action::CancelWorkspaceSearch);
            }

            if self.binding_matches_action("blur_workspace_search", raw, normalized, &["enter"]) {
                return Some(Action::BlurWorkspaceSearch);
            }

            if self.binding_matches_action(
                "backspace_workspace_search",
                raw,
                normalized,
                &["backspace"],
            ) {
                return Some(Action::BackspaceWorkspaceSearch);
            }

            return match raw {
                "space" | " " => Some(Action::AppendWorkspaceSearch {
                    text: " ".to_string(),
                }),
                _ if raw.chars().count() == 1 => Some(Action::AppendWorkspaceSearch {
                    text: raw.to_string(),
                }),
                _ => None,
            };
        }

        if self.binding_matches_action("focus_workspace_search", raw, normalized, &["/"]) {
            return Some(Action::FocusWorkspaceSearch);
        }

        if self.binding_matches_action("select_next_repo", raw, normalized, &["j", "down"]) {
            return Some(Action::SelectNextRepo);
        }

        if self.binding_matches_action("select_previous_repo", raw, normalized, &["k", "up"]) {
            return Some(Action::SelectPreviousRepo);
        }

        if self.binding_matches_action("focus_workspace_preview", raw, normalized, &["l", "right"])
        {
            return Some(Action::SetFocusedPane(PaneId::WorkspacePreview));
        }

        if self.binding_matches_action("focus_workspace_list", raw, normalized, &["h", "left"]) {
            return Some(Action::SetFocusedPane(PaneId::WorkspaceList));
        }

        if self.binding_matches_action("cycle_workspace_filter", raw, normalized, &["f"]) {
            return Some(Action::CycleWorkspaceFilter);
        }

        if self.binding_matches_action("cycle_workspace_sort", raw, normalized, &["s"]) {
            return Some(Action::CycleWorkspaceSort);
        }

        if !self.state.workspace.search_query.is_empty()
            && self.binding_matches_action("cancel_workspace_search", raw, normalized, &["esc"])
        {
            return Some(Action::CancelWorkspaceSearch);
        }

        if self.binding_matches_action("enter_repo_mode", raw, normalized, &["enter"]) {
            return self
                .state
                .workspace
                .selected_repo_id
                .clone()
                .map(|repo_id| Action::EnterRepoMode { repo_id });
        }

        if self.binding_matches_action("open_in_editor", raw, normalized, &["e"]) {
            return Some(Action::OpenInEditor);
        }

        if self.binding_matches_action("refresh_visible_repos", raw, normalized, &["r"]) {
            return Some(Action::RefreshVisibleRepos);
        }

        None
    }

    fn route_repo_key(&self, raw: &str, normalized: &str) -> Option<Action> {
        if self.binding_matches_action("push_current_branch", raw, normalized, &["P"]) {
            return Some(Action::PushCurrentBranch);
        }

        if self.can_open_commit_box()
            && self.binding_matches_action("open_amend_commit_box", raw, normalized, &["A"])
        {
            return Some(Action::OpenCommitBox {
                mode: CommitBoxMode::Amend,
            });
        }

        match self.state.focused_pane {
            PaneId::RepoUnstaged | PaneId::RepoStaged => {
                if self.binding_matches_action(
                    "select_next_status_entry",
                    raw,
                    normalized,
                    &["j", "down"],
                ) {
                    return Some(Action::SelectNextStatusEntry);
                }

                if self.binding_matches_action(
                    "select_previous_status_entry",
                    raw,
                    normalized,
                    &["k", "up"],
                ) {
                    return Some(Action::SelectPreviousStatusEntry);
                }

                if self.binding_matches_action("discard_selected_file", raw, normalized, &["D"]) {
                    return Some(Action::DiscardSelectedFile);
                }

                if self.binding_matches_action("open_in_editor", raw, normalized, &["e"]) {
                    return Some(Action::OpenInEditor);
                }

                if self.binding_matches_action("open_stash_options", raw, normalized, &["S"]) {
                    return Some(Action::OpenStashOptions);
                }

                if self.binding_matches_action("stash_all_changes", raw, normalized, &["s"]) {
                    return Some(Action::StashAllChanges);
                }

                if self.state.focused_pane == PaneId::RepoUnstaged
                    && self.binding_matches_action(
                        "stage_selected_file",
                        raw,
                        normalized,
                        &["enter", "space"],
                    )
                {
                    return Some(Action::StageSelectedFile);
                }

                if self.state.focused_pane == PaneId::RepoStaged
                    && self.binding_matches_action(
                        "unstage_selected_file",
                        raw,
                        normalized,
                        &["enter", "space"],
                    )
                {
                    return Some(Action::UnstageSelectedFile);
                }

                if self.state.focused_pane == PaneId::RepoStaged
                    && self.can_open_commit_box()
                    && self.binding_matches_action("open_commit_box", raw, normalized, &["c"])
                {
                    return Some(Action::OpenCommitBox {
                        mode: CommitBoxMode::Commit,
                    });
                }

                if self.state.focused_pane == PaneId::RepoStaged
                    && self.can_open_commit_box()
                    && self.binding_matches_action(
                        "open_commit_no_verify_box",
                        raw,
                        normalized,
                        &["w"],
                    )
                {
                    return Some(Action::OpenCommitBox {
                        mode: CommitBoxMode::CommitNoVerify,
                    });
                }

                if self.binding_matches_action("commit_staged_with_editor", raw, normalized, &["C"])
                {
                    return Some(Action::CommitStagedWithEditor);
                }
            }
            _ => {}
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(
                    repo_mode.active_subview,
                    RepoSubview::Status
                        | RepoSubview::Branches
                        | RepoSubview::Commits
                        | RepoSubview::Compare
                        | RepoSubview::Rebase
                        | RepoSubview::Stash
                        | RepoSubview::Reflog
                        | RepoSubview::Worktrees
                )
            })
        {
            if let Some(repo_mode) = self.state.repo_mode.as_ref() {
                match repo_mode.active_subview {
                    RepoSubview::Branches => {
                        if self.binding_matches_action(
                            "select_next_branch",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextBranch);
                        }

                        if self.binding_matches_action(
                            "select_previous_branch",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousBranch);
                        }

                        if self.binding_matches_action(
                            "checkout_selected_branch",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::CheckoutSelectedBranch);
                        }

                        if self.binding_matches_action(
                            "open_rename_branch_prompt",
                            raw,
                            normalized,
                            &["R"],
                        ) {
                            if let Some(branch) = selected_branch(
                                repo_mode.detail.as_ref(),
                                repo_mode.branches_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation:
                                        super_lazygit_core::InputPromptOperation::RenameBranch {
                                            current_name: branch.name.clone(),
                                        },
                                });
                            }
                        }

                        if self.binding_matches_action(
                            "open_create_branch_prompt",
                            raw,
                            normalized,
                            &["c"],
                        ) {
                            return Some(Action::OpenInputPrompt {
                                operation: super_lazygit_core::InputPromptOperation::CreateBranch,
                            });
                        }

                        if self.binding_matches_action(
                            "delete_selected_branch",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::DeleteSelectedBranch);
                        }

                        if self.binding_matches_action(
                            "open_set_branch_upstream_prompt",
                            raw,
                            normalized,
                            &["u"],
                        ) {
                            if let Some(branch) = selected_branch(
                                repo_mode.detail.as_ref(),
                                repo_mode.branches_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation: super_lazygit_core::InputPromptOperation::SetBranchUpstream {
                                        branch_name: branch.name.clone(),
                                    },
                                });
                            }
                        }
                    }
                    RepoSubview::Status => {
                        if self.binding_matches_action(
                            "select_next_diff_line",
                            raw,
                            normalized,
                            &["J"],
                        ) {
                            return Some(Action::SelectNextDiffLine);
                        }

                        if self.binding_matches_action(
                            "select_previous_diff_line",
                            raw,
                            normalized,
                            &["K"],
                        ) {
                            return Some(Action::SelectPreviousDiffLine);
                        }

                        if self.binding_matches_action(
                            "select_next_diff_hunk",
                            raw,
                            normalized,
                            &["j"],
                        ) {
                            return Some(Action::SelectNextDiffHunk);
                        }

                        if self.binding_matches_action(
                            "select_previous_diff_hunk",
                            raw,
                            normalized,
                            &["k"],
                        ) {
                            return Some(Action::SelectPreviousDiffHunk);
                        }

                        if self.binding_matches_action(
                            "toggle_diff_line_anchor",
                            raw,
                            normalized,
                            &["v"],
                        ) {
                            return Some(Action::ToggleDiffLineAnchor);
                        }

                        if self.binding_matches_action(
                            "scroll_repo_detail_down",
                            raw,
                            normalized,
                            &["down"],
                        ) {
                            return Some(Action::ScrollRepoDetailDown);
                        }

                        if self.binding_matches_action(
                            "scroll_repo_detail_up",
                            raw,
                            normalized,
                            &["up"],
                        ) {
                            return Some(Action::ScrollRepoDetailUp);
                        }

                        if self.binding_matches_action(
                            "apply_selected_hunk",
                            raw,
                            normalized,
                            &["enter", "space"],
                        ) {
                            return match repo_mode
                                .detail
                                .as_ref()
                                .map(|detail| detail.diff.presentation)
                            {
                                Some(DiffPresentation::Unstaged) => Some(Action::StageSelectedHunk),
                                Some(DiffPresentation::Staged) => Some(Action::UnstageSelectedHunk),
                                _ => None,
                            };
                        }

                        if self.binding_matches_action(
                            "apply_selected_lines",
                            raw,
                            normalized,
                            &["L"],
                        ) {
                            return match repo_mode
                                .detail
                                .as_ref()
                                .map(|detail| detail.diff.presentation)
                            {
                                Some(DiffPresentation::Unstaged) => {
                                    Some(Action::StageSelectedLines)
                                }
                                Some(DiffPresentation::Staged) => {
                                    Some(Action::UnstageSelectedLines)
                                }
                                _ => None,
                            };
                        }

                        if self.binding_matches_action(
                            "discard_selected_file",
                            raw,
                            normalized,
                            &["D"],
                        ) {
                            return Some(Action::DiscardSelectedFile);
                        }

                        if self.binding_matches_action("open_in_editor", raw, normalized, &["e"]) {
                            return Some(Action::OpenInEditor);
                        }

                        if self.binding_matches_action("nuke_working_tree", raw, normalized, &["X"])
                        {
                            return Some(Action::NukeWorkingTree);
                        }
                    }
                    RepoSubview::Commits => {
                        if self.binding_matches_action(
                            "select_next_commit",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextCommit);
                        }

                        if self.binding_matches_action(
                            "select_previous_commit",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousCommit);
                        }

                        if self.binding_matches_action(
                            "start_interactive_rebase",
                            raw,
                            normalized,
                            &["i"],
                        ) {
                            return Some(Action::StartInteractiveRebase);
                        }

                        if self.binding_matches_action(
                            "amend_selected_commit",
                            raw,
                            normalized,
                            &["A"],
                        ) {
                            return Some(Action::AmendSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "fixup_selected_commit",
                            raw,
                            normalized,
                            &["F"],
                        ) {
                            return Some(Action::FixupSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "reword_selected_commit",
                            raw,
                            normalized,
                            &["R"],
                        ) {
                            return Some(Action::RewordSelectedCommitWithEditor);
                        }

                        if self.binding_matches_action(
                            "cherry_pick_selected_commit",
                            raw,
                            normalized,
                            &["C"],
                        ) {
                            return Some(Action::CherryPickSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "revert_selected_commit",
                            raw,
                            normalized,
                            &["V"],
                        ) {
                            return Some(Action::RevertSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "soft_reset_to_selected_commit",
                            raw,
                            normalized,
                            &["S"],
                        ) {
                            return Some(Action::SoftResetToSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "mixed_reset_to_selected_commit",
                            raw,
                            normalized,
                            &["M"],
                        ) {
                            return Some(Action::MixedResetToSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "hard_reset_to_selected_commit",
                            raw,
                            normalized,
                            &["H"],
                        ) {
                            return Some(Action::HardResetToSelectedCommit);
                        }
                    }
                    RepoSubview::Compare => {
                        if self.binding_matches_action(
                            "scroll_repo_detail_down",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::ScrollRepoDetailDown);
                        }

                        if self.binding_matches_action(
                            "scroll_repo_detail_up",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::ScrollRepoDetailUp);
                        }
                    }
                    RepoSubview::Rebase => {
                        if self.binding_matches_action(
                            "scroll_repo_detail_down",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::ScrollRepoDetailDown);
                        }

                        if self.binding_matches_action(
                            "scroll_repo_detail_up",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::ScrollRepoDetailUp);
                        }

                        if self.binding_matches_action("continue_rebase", raw, normalized, &["c"]) {
                            return Some(Action::ContinueRebase);
                        }

                        if self.binding_matches_action("skip_rebase", raw, normalized, &["s"]) {
                            return Some(Action::SkipRebase);
                        }

                        if self.binding_matches_action("abort_rebase", raw, normalized, &["A"]) {
                            return Some(Action::AbortRebase);
                        }
                    }
                    RepoSubview::Stash => {
                        if self.binding_matches_action(
                            "select_next_stash",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextStash);
                        }

                        if self.binding_matches_action(
                            "select_previous_stash",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousStash);
                        }

                        if self.binding_matches_action(
                            "apply_selected_stash",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::ApplySelectedStash);
                        }

                        if self.binding_matches_action(
                            "open_rename_stash_prompt",
                            raw,
                            normalized,
                            &["r"],
                        ) {
                            if let Some(stash) = selected_stash(
                                repo_mode.detail.as_ref(),
                                repo_mode.stash_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation:
                                        super_lazygit_core::InputPromptOperation::RenameStash {
                                            stash_ref: stash.stash_ref.clone(),
                                            current_name: stash_message_label(&stash.label),
                                        },
                                });
                            }
                        }

                        if self.binding_matches_action(
                            "pop_selected_stash",
                            raw,
                            normalized,
                            &["g"],
                        ) {
                            return Some(Action::PopSelectedStash);
                        }

                        if self.binding_matches_action(
                            "drop_selected_stash",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::DropSelectedStash);
                        }
                    }
                    RepoSubview::Reflog => {
                        if self.binding_matches_action(
                            "select_next_reflog",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextReflog);
                        }

                        if self.binding_matches_action(
                            "select_previous_reflog",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousReflog);
                        }

                        if self.binding_matches_action(
                            "restore_selected_reflog_entry",
                            raw,
                            normalized,
                            &["u"],
                        ) {
                            return Some(Action::RestoreSelectedReflogEntry);
                        }
                    }
                    RepoSubview::Worktrees => {
                        if self.binding_matches_action(
                            "select_next_worktree",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextWorktree);
                        }

                        if self.binding_matches_action(
                            "select_previous_worktree",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousWorktree);
                        }

                        if self.binding_matches_action("create_worktree", raw, normalized, &["c"]) {
                            return Some(Action::CreateWorktree);
                        }

                        if self.binding_matches_action(
                            "remove_selected_worktree",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::RemoveSelectedWorktree);
                        }
                    }
                }

                if matches!(
                    repo_mode.active_subview,
                    RepoSubview::Branches | RepoSubview::Commits
                ) && self.binding_matches_action(
                    "toggle_comparison_selection",
                    raw,
                    normalized,
                    &["v"],
                ) {
                    return Some(Action::ToggleComparisonSelection);
                }

                if repo_mode.comparison_base.is_some()
                    && matches!(
                        repo_mode.active_subview,
                        RepoSubview::Branches | RepoSubview::Commits | RepoSubview::Compare
                    )
                    && self.binding_matches_action("clear_comparison", raw, normalized, &["x"])
                {
                    return Some(Action::ClearComparison);
                }
            }
        }

        if self.binding_matches_action("focus_repo_left", raw, normalized, &["h", "left"]) {
            return self.repo_focus_left_action();
        }

        if self.binding_matches_action("focus_repo_right", raw, normalized, &["l", "right"]) {
            return self.repo_focus_right_action();
        }

        if self.binding_matches_action("switch_repo_subview_status", raw, normalized, &["1"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Status));
        }

        if self.binding_matches_action("switch_repo_subview_branches", raw, normalized, &["2"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Branches));
        }

        if self.binding_matches_action("switch_repo_subview_commits", raw, normalized, &["3"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Commits));
        }

        if self.binding_matches_action("switch_repo_subview_compare", raw, normalized, &["4"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Compare));
        }

        if self.binding_matches_action("switch_repo_subview_rebase", raw, normalized, &["5"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Rebase));
        }

        if self.binding_matches_action("switch_repo_subview_stash", raw, normalized, &["6"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Stash));
        }

        if self.binding_matches_action("switch_repo_subview_reflog", raw, normalized, &["7"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Reflog));
        }

        if self.binding_matches_action("switch_repo_subview_worktrees", raw, normalized, &["8"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Worktrees));
        }

        if self.binding_matches_action("refresh_selected_repo", raw, normalized, &["r"]) {
            return Some(Action::RefreshSelectedRepo);
        }

        if self.binding_matches_action("fetch_selected_repo", raw, normalized, &["f"]) {
            return Some(Action::FetchSelectedRepo);
        }

        if self.binding_matches_action("pull_current_branch", raw, normalized, &["p"]) {
            return Some(Action::PullCurrentBranch);
        }

        None
    }

    fn next_focus_action(&self) -> Option<Action> {
        match self.state.mode {
            AppMode::Workspace => Some(Action::SetFocusedPane(match self.state.focused_pane {
                PaneId::WorkspaceList => PaneId::WorkspacePreview,
                _ => PaneId::WorkspaceList,
            })),
            AppMode::Repository => Some(Action::SetFocusedPane(self.next_repo_pane())),
        }
    }

    fn previous_focus_action(&self) -> Option<Action> {
        match self.state.mode {
            AppMode::Workspace => self.next_focus_action(),
            AppMode::Repository => Some(Action::SetFocusedPane(self.previous_repo_pane())),
        }
    }

    fn next_repo_pane(&self) -> PaneId {
        match self.state.focused_pane {
            PaneId::RepoUnstaged => PaneId::RepoStaged,
            PaneId::RepoStaged => PaneId::RepoDetail,
            _ => PaneId::RepoUnstaged,
        }
    }

    fn previous_repo_pane(&self) -> PaneId {
        match self.state.focused_pane {
            PaneId::RepoDetail => PaneId::RepoStaged,
            PaneId::RepoStaged => PaneId::RepoUnstaged,
            _ => PaneId::RepoDetail,
        }
    }

    fn repo_focus_left_action(&self) -> Option<Action> {
        Some(Action::SetFocusedPane(match self.state.focused_pane {
            PaneId::RepoDetail => PaneId::RepoStaged,
            PaneId::RepoStaged => PaneId::RepoUnstaged,
            _ => PaneId::RepoUnstaged,
        }))
    }

    fn repo_focus_right_action(&self) -> Option<Action> {
        Some(Action::SetFocusedPane(match self.state.focused_pane {
            PaneId::RepoUnstaged => PaneId::RepoStaged,
            PaneId::RepoStaged => PaneId::RepoDetail,
            _ => PaneId::RepoDetail,
        }))
    }

    fn commit_box_focused(&self) -> bool {
        self.state
            .repo_mode
            .as_ref()
            .is_some_and(|repo_mode| repo_mode.commit_box.focused)
    }

    fn prompt_input_focused(&self) -> bool {
        self.state.pending_input_prompt.is_some()
            && self.state.modal_stack.last().is_some_and(|modal| {
                matches!(modal.kind, super_lazygit_core::ModalKind::InputPrompt)
            })
    }

    fn can_open_commit_box(&self) -> bool {
        self.state.focused_pane == PaneId::RepoStaged
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                repo_mode.active_subview == RepoSubview::Status && repo_mode.detail.is_some()
            })
    }

    fn workspace_search_focused(&self) -> bool {
        matches!(self.state.mode, AppMode::Workspace) && self.state.workspace.search_focused
    }

    fn route_commit_box_key(&self, raw: &str, normalized: &str) -> Option<Action> {
        if self.binding_matches_action("cancel_commit_box", raw, normalized, &["esc"]) {
            return Some(Action::CancelCommitBox);
        }

        if self.binding_matches_action("submit_commit_box", raw, normalized, &["enter"]) {
            return Some(Action::SubmitCommitBox);
        }

        if self.binding_matches_action("backspace_commit_input", raw, normalized, &["backspace"]) {
            return Some(Action::BackspaceCommitInput);
        }

        match raw {
            "space" | " " => Some(Action::AppendCommitInput {
                text: " ".to_string(),
            }),
            _ => {
                if raw.chars().count() == 1 {
                    Some(Action::AppendCommitInput {
                        text: raw.to_string(),
                    })
                } else {
                    None
                }
            }
        }
    }

    fn render_mode(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        match self.state.mode {
            AppMode::Workspace => self.render_workspace_shell(area, buffer, theme),
            AppMode::Repository => self.render_repo_shell(area, buffer, theme),
        }
    }

    fn render_workspace_shell(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let panes = split_two_columns(area);
        self.render_workspace_list(panes[0], buffer, theme);
        self.render_workspace_preview(panes[1], buffer, theme);
    }

    fn render_repo_shell(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);
        self.render_repo_header(layout[0], buffer, theme);

        let panes = split_repo_columns(layout[1]);
        let left = split_repo_left_column(panes[0]);
        self.render_repo_unstaged(left[0], buffer, theme);
        self.render_repo_staged(left[1], buffer, theme);
        self.render_repo_detail(panes[1], buffer, theme);
    }

    fn render_repo_header(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let Some(repo_mode) = &self.state.repo_mode else {
            return;
        };

        let title = self
            .selected_summary()
            .map(|summary| summary.display_name.as_str())
            .unwrap_or(repo_mode.current_repo_id.0.as_str());
        let lines = vec![
            Line::from(format!(
                "Repo: {}  Branch: {}  Watch: {}",
                title,
                self.selected_summary()
                    .and_then(|summary| summary.branch.as_deref())
                    .unwrap_or("detached"),
                watcher_health_label(&self.state.workspace.watcher_health)
            )),
            Line::from(repo_subview_tabs(repo_mode.active_subview)),
        ];

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Repository shell")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            )
            .render(area, buffer);
    }

    fn render_workspace_list(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let layout = workspace_table_layout(area.width.saturating_sub(2) as usize);
        let repo_ids = self.state.workspace.visible_repo_ids();
        let mut lines = vec![
            Line::from(self.workspace_root_label()),
            workspace_status_line(&self.state, repo_ids.len(), theme),
            workspace_table_header(layout, theme),
        ];

        if repo_ids.is_empty() {
            lines.extend(workspace_empty_list_lines(&self.state));
        } else {
            for repo_id in &repo_ids {
                let is_selected = self
                    .state
                    .workspace
                    .selected_repo_id
                    .as_ref()
                    .is_some_and(|selected| selected == repo_id);
                let summary = self.state.workspace.repo_summaries.get(repo_id);
                lines.push(workspace_repo_line(
                    repo_id,
                    summary,
                    is_selected,
                    self.state.focused_pane == PaneId::WorkspaceList,
                    layout,
                    theme,
                ));
            }
        }

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Workspace")
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::WorkspaceList, theme)),
            )
            .render(area, buffer);
    }

    fn render_workspace_preview(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let lines = if let Some(summary) = self.selected_summary() {
            workspace_preview_lines(summary)
        } else if let Some(repo_id) = self.state.workspace.selected_repo_id.as_ref() {
            workspace_pending_preview_lines(repo_id, &self.state)
        } else if self.state.workspace.visible_repo_ids().is_empty() {
            workspace_empty_preview_lines(&self.state)
        } else {
            vec![
                Line::from("Preview"),
                Line::from("Select a repository to inspect its state."),
            ]
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Preview")
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::WorkspacePreview, theme)),
            )
            .render(area, buffer);
    }

    fn render_repo_unstaged(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let Some(repo_mode) = &self.state.repo_mode else {
            Paragraph::new("Enter repo mode to inspect repository details.")
                .block(
                    Block::default()
                        .title("Working tree")
                        .borders(Borders::ALL)
                        .border_style(self.pane_style(PaneId::RepoUnstaged, theme)),
                )
                .render(area, buffer);
            return;
        };

        let lines = repo_unstaged_lines(
            repo_mode.detail.as_ref(),
            repo_mode.status_view.selected_index,
            self.state.focused_pane == PaneId::RepoUnstaged,
            &repo_mode.operation_progress,
        );

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Working tree")
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::RepoUnstaged, theme)),
            )
            .render(area, buffer);
    }

    fn render_repo_staged(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let repo_mode = self.state.repo_mode.as_ref();
        let lines = repo_staged_lines(
            repo_mode.and_then(|repo_mode| repo_mode.detail.as_ref()),
            repo_mode.and_then(|repo_mode| repo_mode.staged_view.selected_index),
            self.state.focused_pane == PaneId::RepoStaged,
        );
        let title = if repo_mode.is_some_and(|repo_mode| repo_mode.commit_box.focused) {
            match repo_mode.map(|repo_mode| repo_mode.commit_box.mode) {
                Some(CommitBoxMode::Commit) => "Staged changes · Commit",
                Some(CommitBoxMode::CommitNoVerify) => "Staged changes · Commit (No Verify)",
                Some(CommitBoxMode::Amend) => "Staged changes · Amend",
                None => "Staged changes",
            }
        } else {
            "Staged changes"
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::RepoStaged, theme)),
            )
            .render(area, buffer);

        if let Some(repo_mode) = repo_mode.filter(|repo_mode| repo_mode.commit_box.focused) {
            let commit_box_area = centered_rect(area, 92, 56);
            Clear.render(commit_box_area, buffer);
            Paragraph::new(commit_box_lines(
                repo_mode.detail.as_ref(),
                repo_mode.commit_box.mode,
                theme,
            ))
            .block(
                Block::default()
                    .title(commit_box_title(repo_mode.commit_box.mode))
                    .borders(Borders::ALL)
                    .border_style(
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
            )
            .render(commit_box_area, buffer);
        }
    }

    fn render_repo_detail(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let (title, lines) = if let Some(repo_mode) = &self.state.repo_mode {
            let lines = match repo_mode.active_subview {
                RepoSubview::Status => repo_diff_lines(
                    Some(repo_mode),
                    repo_mode.detail.as_ref(),
                    repo_mode.diff_scroll,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                RepoSubview::Branches => repo_branch_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.branches_view.selected_index,
                    repo_mode.comparison_base.as_ref(),
                    repo_mode.comparison_target.as_ref(),
                    repo_mode.comparison_source,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Commits => repo_commit_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.commits_view.selected_index,
                    repo_mode.comparison_base.as_ref(),
                    repo_mode.comparison_target.as_ref(),
                    repo_mode.comparison_source,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                RepoSubview::Compare => repo_compare_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.comparison_base.as_ref(),
                    repo_mode.comparison_target.as_ref(),
                    repo_mode.diff_scroll,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                RepoSubview::Rebase => repo_rebase_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.diff_scroll,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                RepoSubview::Stash => repo_stash_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.stash_view.selected_index,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Reflog => repo_reflog_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.reflog_view.selected_index,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Worktrees => repo_worktree_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.worktree_view.selected_index,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
            };
            (
                format!("Detail: {}", repo_subview_label(repo_mode.active_subview)),
                lines,
            )
        } else {
            (
                "Detail".to_string(),
                vec![Line::from("Repository detail will appear here.")],
            )
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::RepoDetail, theme)),
            )
            .render(area, buffer);
    }

    fn render_status_bar(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let mut lines = vec![Line::from(vec![
            Span::styled(
                mode_label(self.state.mode.clone()),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(format!("focus={:?}", self.state.focused_pane)),
            Span::raw("  "),
            Span::raw(format!(
                "size={}x{}",
                self.viewport.width, self.viewport.height
            )),
            Span::raw("  "),
            Span::raw(status_text(&self.state)),
        ])];

        if self.state.settings.show_help_footer {
            lines.push(Line::from(help_text(&self.state)));
        }

        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.accent)),
            )
            .style(Style::default().bg(theme.background).fg(theme.foreground))
            .render(area, buffer);
    }

    fn pane_style(&self, pane: PaneId, theme: Theme) -> Style {
        if self.state.focused_pane == pane {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        }
    }

    fn workspace_root_label(&self) -> String {
        self.state.workspace.current_root.as_ref().map_or_else(
            || "Root: current directory".to_string(),
            |root| format!("Root: {}", root.display()),
        )
    }

    fn selected_summary(&self) -> Option<&RepoSummary> {
        self.state
            .workspace
            .selected_repo_id
            .as_ref()
            .and_then(|repo_id| self.state.workspace.repo_summaries.get(repo_id))
    }

    fn modal_lines(&self, modal: &super_lazygit_core::Modal, theme: Theme) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(Span::styled(
            modal.title.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))];

        match modal.kind {
            super_lazygit_core::ModalKind::Confirm => {
                lines.push(Line::from(""));
                if let Some(pending) = self.state.pending_confirmation.as_ref() {
                    lines.push(Line::from(format!("Repo: {}", pending.repo_id.0)));
                    lines.push(Line::from(confirmation_copy(&pending.operation)));
                }
                lines.push(Line::from("Enter or y confirms. Esc, n, or q cancels."));
            }
            super_lazygit_core::ModalKind::InputPrompt => {
                lines.push(Line::from(""));
                if let Some(prompt) = self.state.pending_input_prompt.as_ref() {
                    lines.push(Line::from(format!("Repo: {}", prompt.repo_id.0)));
                    lines.push(Line::from(input_prompt_copy(&prompt.operation)));
                    lines.push(Line::from(""));
                    lines.push(Line::from(format!("> {}_", prompt.value)));
                    lines.push(Line::from("Enter submits. Esc cancels. Backspace deletes."));
                }
            }
            super_lazygit_core::ModalKind::Menu => {
                lines.push(Line::from(""));
                if let Some(menu) = self.state.pending_menu.as_ref() {
                    lines.push(Line::from(format!("Repo: {}", menu.repo_id.0)));
                    lines.push(Line::from(menu_copy(menu.operation)));
                    lines.push(Line::from(""));
                    lines.extend(menu_lines(menu, theme));
                    lines.push(Line::from(""));
                    lines.push(Line::from("j/k moves. Enter selects. Esc cancels."));
                }
            }
            _ => {
                lines.push(Line::from(""));
                lines.push(Line::from(format!("{:?}", modal.kind)));
                lines.push(Line::from("Esc closes this overlay."));
            }
        }

        lines
    }
}

#[derive(Debug, Clone, Copy)]
struct Theme {
    background: Color,
    foreground: Color,
    accent: Color,
    success: Color,
    danger: Color,
    muted: Color,
}

impl Theme {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            background: parse_hex_color(&config.theme.colors.background).unwrap_or(Color::Black),
            foreground: parse_hex_color(&config.theme.colors.foreground).unwrap_or(Color::White),
            accent: parse_hex_color(&config.theme.colors.accent).unwrap_or(Color::Cyan),
            success: parse_hex_color(&config.theme.colors.success).unwrap_or(Color::Green),
            danger: parse_hex_color(&config.theme.colors.danger).unwrap_or(Color::Red),
            muted: Color::DarkGray,
        }
    }
}

fn split_two_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    let constraints = if area.width >= 120 {
        [Constraint::Percentage(44), Constraint::Percentage(56)]
    } else if area.width >= 90 {
        [Constraint::Percentage(50), Constraint::Percentage(50)]
    } else {
        [Constraint::Percentage(58), Constraint::Percentage(42)]
    };

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
}

fn split_repo_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area)
}

fn split_repo_left_column(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area)
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .flex(Flex::Center)
        .split(vertical[1])[1]
}

#[derive(Debug, Clone, Copy)]
struct WorkspaceTableLayout {
    name_width: usize,
    branch_width: usize,
    dirty_width: usize,
    sync_width: Option<usize>,
    fetch_width: usize,
}

fn workspace_table_layout(width: usize) -> WorkspaceTableLayout {
    if width >= 54 {
        let branch_width = 14;
        let dirty_width = 12;
        let sync_width = 8;
        let fetch_width = 8;
        let name_width = width
            .saturating_sub(branch_width + dirty_width + sync_width + fetch_width + 4)
            .max(12);
        WorkspaceTableLayout {
            name_width,
            branch_width,
            dirty_width,
            sync_width: Some(sync_width),
            fetch_width,
        }
    } else if width >= 40 {
        let branch_width = 9;
        let dirty_width = 12;
        let fetch_width = 6;
        let name_width = width
            .saturating_sub(branch_width + dirty_width + fetch_width + 3)
            .max(10);
        WorkspaceTableLayout {
            name_width,
            branch_width,
            dirty_width,
            sync_width: None,
            fetch_width,
        }
    } else {
        let branch_width = 6;
        let dirty_width = 10;
        let fetch_width = 5;
        let name_width = width
            .saturating_sub(branch_width + dirty_width + fetch_width + 3)
            .max(8);
        WorkspaceTableLayout {
            name_width,
            branch_width,
            dirty_width,
            sync_width: None,
            fetch_width,
        }
    }
}

fn workspace_table_header(layout: WorkspaceTableLayout, theme: Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            pad_cell("REPO", layout.name_width),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_cell(
                if layout.sync_width.is_some() {
                    "BRANCH"
                } else {
                    "BR"
                },
                layout.branch_width,
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_cell(
                if layout.sync_width.is_some() {
                    "DIRTY"
                } else {
                    "STATE"
                },
                layout.dirty_width,
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(sync_width) = layout.sync_width {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell("SYNC", sync_width),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell("FETCH", layout.fetch_width),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell("AGE", layout.fetch_width),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn workspace_repo_line(
    repo_id: &RepoId,
    summary: Option<&RepoSummary>,
    is_selected: bool,
    list_is_focused: bool,
    layout: WorkspaceTableLayout,
    theme: Theme,
) -> Line<'static> {
    let prefix = if is_selected { ">" } else { " " };
    let prefix_style = if is_selected {
        let mut style = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);
        if list_is_focused {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        style
    } else {
        Style::default().fg(theme.muted)
    };

    let name = summary.map_or(repo_id.0.as_str(), |summary| summary.display_name.as_str());
    let branch = summary
        .and_then(|summary| summary.branch.as_deref())
        .unwrap_or("detached");
    let dirty = summary.map_or_else(
        || "pending".to_string(),
        |summary| workspace_dirty_cell(summary, layout.sync_width.is_none()),
    );
    let sync = summary.map_or_else(
        || "-".to_string(),
        |summary| format!("+{}/-{}", summary.ahead_count, summary.behind_count),
    );
    let fetch = summary
        .map(workspace_fetch_age)
        .unwrap_or_else(|| "pending".to_string());

    let mut spans = vec![
        Span::styled(format!("{prefix} "), prefix_style),
        Span::styled(
            pad_cell(name, layout.name_width.saturating_sub(2)),
            if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ),
        Span::raw(" "),
        Span::styled(
            pad_cell(branch, layout.branch_width),
            Style::default().fg(theme.foreground),
        ),
        Span::raw(" "),
        Span::styled(
            pad_cell(&dirty, layout.dirty_width),
            workspace_dirty_style(summary, theme),
        ),
    ];

    if let Some(sync_width) = layout.sync_width {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell(&sync, sync_width),
            workspace_sync_style(summary, theme),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell(&fetch, layout.fetch_width),
            workspace_fetch_style(summary, theme),
        ));
    } else {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            pad_cell(&fetch, layout.fetch_width),
            workspace_fetch_style(summary, theme),
        ));
    }

    Line::from(spans)
}

fn workspace_dirty_cell(summary: &RepoSummary, compact: bool) -> String {
    if summary.last_error.is_some() {
        return "error".to_string();
    }
    if summary.conflicted {
        return "conflict".to_string();
    }
    if !summary.dirty {
        return if compact {
            "clean +0/-0".to_string()
        } else {
            "clean".to_string()
        };
    }

    if compact {
        format!(
            "{}/{}/{} +{}/-{}",
            summary.staged_count,
            summary.unstaged_count,
            summary.untracked_count,
            summary.ahead_count,
            summary.behind_count
        )
    } else {
        format!(
            "{}S {}U {}?",
            summary.staged_count, summary.unstaged_count, summary.untracked_count
        )
    }
}

fn workspace_fetch_age(summary: &RepoSummary) -> String {
    summary
        .last_fetch_at
        .map(|timestamp| {
            let seconds = summary
                .last_refresh_at
                .unwrap_or(timestamp)
                .0
                .saturating_sub(timestamp.0);
            if seconds < 60 {
                format!("{seconds}s")
            } else if seconds < 3_600 {
                format!("{}m", seconds / 60)
            } else if seconds < 86_400 {
                format!("{}h", seconds / 3_600)
            } else {
                format!("{}d", seconds / 86_400)
            }
        })
        .unwrap_or_else(|| "never".to_string())
}

fn workspace_dirty_style(summary: Option<&RepoSummary>, theme: Theme) -> Style {
    match summary {
        Some(summary) if summary.last_error.is_some() => Style::default()
            .fg(theme.danger)
            .add_modifier(Modifier::BOLD),
        Some(summary) if summary.conflicted => Style::default()
            .fg(theme.danger)
            .add_modifier(Modifier::BOLD),
        Some(summary) if summary.dirty => Style::default().fg(theme.danger),
        Some(_) => Style::default().fg(theme.success),
        None => Style::default().fg(theme.muted),
    }
}

fn workspace_sync_style(summary: Option<&RepoSummary>, theme: Theme) -> Style {
    match summary {
        Some(summary) if summary.behind_count > 0 => Style::default()
            .fg(theme.danger)
            .add_modifier(Modifier::BOLD),
        Some(summary) if summary.ahead_count > 0 => Style::default().fg(theme.accent),
        Some(_) => Style::default().fg(theme.success),
        None => Style::default().fg(theme.muted),
    }
}

fn workspace_fetch_style(summary: Option<&RepoSummary>, theme: Theme) -> Style {
    match summary {
        Some(summary) if summary.last_fetch_at.is_none() => Style::default().fg(theme.danger),
        Some(_) => Style::default().fg(theme.muted),
        None => Style::default().fg(theme.muted),
    }
}

fn pad_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let truncated = truncate_cell(value, width);
    format!("{truncated:<width$}")
}

fn truncate_cell(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let mut truncated = value.chars().take(width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn workspace_preview_lines(summary: &RepoSummary) -> Vec<Line<'static>> {
    let remote = match (
        summary.remote_summary.remote_name.as_deref(),
        summary.remote_summary.tracking_branch.as_deref(),
    ) {
        (_, Some(tracking)) => tracking.to_string(),
        (Some(remote), None) => remote.to_string(),
        (None, None) => "-".to_string(),
    };
    let fetch_age = summary
        .last_fetch_at
        .map(|timestamp| {
            format!(
                "{}s",
                summary
                    .last_refresh_at
                    .unwrap_or(timestamp)
                    .0
                    .saturating_sub(timestamp.0)
            )
        })
        .unwrap_or_else(|| "never".to_string());

    let mut lines = vec![
        Line::from(format!("Path: {}", summary.display_path)),
        Line::from(format!(
            "Branch: {}",
            summary.branch.as_deref().unwrap_or("detached")
        )),
        Line::from(format!(
            "Attention: {}  Watcher: {:?}",
            workspace_attention_score(Some(summary)),
            summary.watcher_freshness
        )),
        Line::from(format!(
            "Changes: staged={} unstaged={} untracked={}",
            summary.staged_count, summary.unstaged_count, summary.untracked_count
        )),
        Line::from(format!(
            "Remote: {} ahead={} behind={} conflicted={}",
            remote, summary.ahead_count, summary.behind_count, summary.conflicted
        )),
        Line::from(format!("Fetch age: {}", fetch_age)),
    ];
    if let Some(error) = summary.last_error.as_deref() {
        lines.push(Line::from(format!("Last error: {error}")));
    }
    lines
}

fn workspace_pending_preview_lines(repo_id: &RepoId, state: &AppState) -> Vec<Line<'static>> {
    vec![
        Line::from("Preview"),
        Line::from(format!("Repo: {}", repo_id.0)),
        Line::from("State: waiting for repository summary"),
        Line::from(format!(
            "Scan: {}",
            workspace_scan_label(&state.workspace.scan_status)
        )),
        Line::from("Refresh this workspace with r if the row stays pending."),
    ]
}

fn workspace_empty_list_lines(state: &AppState) -> Vec<Line<'static>> {
    match &state.workspace.scan_status {
        super_lazygit_core::ScanStatus::Scanning
            if state.workspace.discovered_repo_ids.is_empty() =>
        {
            return vec![
                Line::from("Scanning workspace for repositories..."),
                Line::from("The table will populate as soon as the scan completes."),
            ];
        }
        super_lazygit_core::ScanStatus::Failed { message } => {
            return vec![
                Line::from("Workspace scan failed."),
                Line::from(format!("Reason: {message}")),
                Line::from("Press r to retry the scan."),
            ];
        }
        _ => {}
    }

    if state.workspace.discovered_repo_ids.is_empty() {
        return vec![
            Line::from("No repositories were found under the current workspace root."),
            Line::from("Press r to rescan after changing the workspace contents."),
        ];
    }

    vec![
        Line::from("No repositories match the current workspace triage settings."),
        Line::from(format!(
            "Filter={}  Search={}",
            state.workspace.filter_mode.label(),
            if state.workspace.search_query.is_empty() {
                "-".to_string()
            } else {
                state.workspace.search_query.clone()
            }
        )),
        Line::from("Press f to change filters, / to edit search, or Esc to clear search."),
    ]
}

fn workspace_empty_preview_lines(state: &AppState) -> Vec<Line<'static>> {
    match &state.workspace.scan_status {
        super_lazygit_core::ScanStatus::Scanning
            if state.workspace.discovered_repo_ids.is_empty() =>
        {
            vec![
                Line::from("Preview"),
                Line::from("Workspace scan in progress."),
                Line::from("A repository preview will appear when the first summary arrives."),
            ]
        }
        super_lazygit_core::ScanStatus::Failed { message } => vec![
            Line::from("Preview"),
            Line::from("Workspace scan failed."),
            Line::from(format!("Reason: {message}")),
            Line::from("Press r to retry."),
        ],
        _ => vec![
            Line::from("Preview"),
            Line::from("No repositories are currently visible."),
            Line::from(format!(
                "Filter: {}  Sort: {}",
                state.workspace.filter_mode.label(),
                state.workspace.sort_mode.label()
            )),
            Line::from(format!(
                "Search: {}",
                if state.workspace.search_query.is_empty() {
                    "-".to_string()
                } else {
                    state.workspace.search_query.clone()
                }
            )),
        ],
    }
}

fn repo_branch_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    comparison_base: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_target: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_source: Option<RepoSubview>,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Branches",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    if detail.branches.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Branches",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No local branches available."),
        ];
    }

    let selected_index = selected_index
        .filter(|index| *index < detail.branches.len())
        .or_else(|| detail.branches.iter().position(|branch| branch.is_head))
        .unwrap_or(0);
    let selected_branch = &detail.branches[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Branches",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_branch.name)),
        Line::from(format!(
            "Upstream: {}",
            selected_branch.upstream.as_deref().unwrap_or("-")
        )),
        Line::from(comparison_status_line(
            RepoSubview::Branches,
            comparison_base,
            comparison_target,
            comparison_source,
        )),
        Line::from("Enter checks out. v marks compare base/target. c creates. R renames. d deletes. u sets upstream."),
        Line::from(""),
    ];

    for (index, branch) in detail.branches.iter().enumerate() {
        let style = branch_row_style(branch, index == selected_index, is_focused, theme);
        lines.push(Line::from(Span::styled(branch_row_label(branch), style)));
    }

    lines
}

fn branch_row_label(branch: &super_lazygit_core::BranchItem) -> String {
    let head = if branch.is_head { "*" } else { " " };
    let upstream = branch.upstream.as_deref().unwrap_or("-");
    format!("{head} {:<20} upstream={upstream}", branch.name)
}

fn branch_row_style(
    branch: &super_lazygit_core::BranchItem,
    is_selected: bool,
    is_focused: bool,
    theme: Theme,
) -> Style {
    let mut style = if branch.is_head {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.foreground)
    };

    if is_selected {
        style = style.add_modifier(Modifier::BOLD);
        if is_focused {
            style = style.add_modifier(Modifier::REVERSED);
        }
    }

    style
}

fn selected_branch(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::BranchItem> {
    let detail = detail?;
    let selected_index = selected_index
        .filter(|index| *index < detail.branches.len())
        .or_else(|| detail.branches.iter().position(|branch| branch.is_head))
        .unwrap_or(0);
    detail.branches.get(selected_index)
}

fn selected_stash(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::StashItem> {
    let detail = detail?;
    let selected_index = selected_index.unwrap_or(0);
    detail.stashes.get(selected_index)
}

fn stash_message_label(label: &str) -> String {
    label
        .rsplit_once(": ")
        .map_or_else(|| label.to_string(), |(_, message)| message.to_string())
}

fn repo_stash_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Stashes",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    if detail.stashes.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Stashes",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No stashes are available."),
        ];
    }

    let selected_index = selected_index
        .filter(|index| *index < detail.stashes.len())
        .unwrap_or(0);
    let selected_stash = &detail.stashes[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Stashes",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_stash.stash_ref)),
        Line::from(selected_stash.label.clone()),
        Line::from("Enter applies. g pops. d drops."),
        Line::from(""),
    ];

    for (index, stash) in detail.stashes.iter().enumerate() {
        let mut style = Style::default().fg(theme.foreground);
        if index == selected_index {
            style = style.add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }
        lines.push(Line::from(Span::styled(stash.label.clone(), style)));
    }

    lines
}

fn repo_reflog_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Reflog",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    if detail.reflog_items.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Reflog",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No reflog entries are available."),
        ];
    }

    let selected_index = selected_index
        .filter(|index| *index < detail.reflog_items.len())
        .unwrap_or(0);
    let selected_entry = &detail.reflog_items[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Reflog",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "Selected {}/{}",
            selected_index + 1,
            detail.reflog_items.len()
        )),
        Line::from(selected_entry.description.clone()),
        Line::from("Use j/k to inspect recent HEAD and ref movement."),
        Line::from("u restore HEAD to the selected entry on a clean working tree."),
        Line::from("Limits: no working tree undo; redo is manual by selecting another entry."),
        Line::from(""),
    ];

    for (index, entry) in detail.reflog_items.iter().enumerate() {
        let mut style = Style::default().fg(theme.foreground);
        if index == selected_index {
            style = style.add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }
        lines.push(Line::from(Span::styled(entry.description.clone(), style)));
    }

    lines
}

fn repo_worktree_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Worktrees",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    if detail.worktrees.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Worktrees",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No linked worktrees are available."),
        ];
    }

    let selected_index = selected_index
        .filter(|index| *index < detail.worktrees.len())
        .unwrap_or(0);
    let selected_worktree = &detail.worktrees[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Worktrees",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_worktree.path.display())),
        Line::from(format!(
            "Branch: {}",
            selected_worktree.branch.as_deref().unwrap_or("(detached)")
        )),
        Line::from("c creates. d removes the selected linked worktree."),
        Line::from(""),
    ];

    for (index, worktree) in detail.worktrees.iter().enumerate() {
        let mut style = Style::default().fg(theme.foreground);
        if index == selected_index {
            style = style.add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }
        let branch = worktree.branch.as_deref().unwrap_or("(detached)");
        lines.push(Line::from(Span::styled(
            format!("{}  [{branch}]", worktree.path.display()),
            style,
        )));
    }

    lines
}

fn repo_commit_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    comparison_base: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_target: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_source: Option<RepoSubview>,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Commit history",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    if detail.commits.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Commit history",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No commits available for this repository."),
        ];
    }

    let selected_index = selected_index
        .filter(|index| *index < detail.commits.len())
        .unwrap_or(0);
    let selected = &detail.commits[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Selected {}/{}  {}  {}",
                selected_index + 1,
                detail.commits.len(),
                selected.short_oid,
                selected.summary
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(comparison_status_line(
            RepoSubview::Commits,
            comparison_base,
            comparison_target,
            comparison_source,
        )),
        Line::from(history_operation_state_line(&detail.merge_state)),
        Line::from("Actions: i rebase  A amend  F fixup  R reword"),
        Line::from("         C cherry-pick  V revert  S soft  M mixed  H hard"),
        Line::from("History:"),
    ];

    let window_start = selected_index.saturating_sub(2);
    let window_end = (window_start + 5).min(detail.commits.len());
    lines.extend(
        detail.commits[window_start..window_end]
            .iter()
            .enumerate()
            .map(|(offset, commit)| {
                let absolute_index = window_start + offset;
                let prefix = if absolute_index == selected_index {
                    ">"
                } else {
                    " "
                };
                Line::from(format!("{prefix} {} {}", commit.short_oid, commit.summary))
            }),
    );

    lines.push(Line::from("Files:"));
    if selected.changed_files.is_empty() {
        lines.push(Line::from("  (no changed files reported)"));
    } else {
        lines.extend(selected.changed_files.iter().take(6).map(|file| {
            Line::from(format!(
                "  {} {}",
                file_status_kind_label(file.kind),
                file.path.display()
            ))
        }));
        if selected.changed_files.len() > 6 {
            lines.push(Line::from(format!(
                "  … {} more file(s)",
                selected.changed_files.len() - 6
            )));
        }
    }

    lines.push(Line::from("Preview:"));
    if selected.diff.lines.is_empty() {
        lines.push(Line::from("No patch preview available for this commit."));
    } else {
        let remaining = viewport_lines.saturating_sub(lines.len()).max(1);
        lines.extend(selected.diff.lines.iter().take(remaining).map(|line| {
            render_diff_line(line.kind, &line.content, theme, false, false, false, false)
        }));
    }

    lines.truncate(viewport_lines.max(1));
    lines
}

fn repo_compare_lines(
    detail: Option<&RepoDetail>,
    comparison_base: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_target: Option<&super_lazygit_core::ComparisonTarget>,
    scroll: usize,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let base = comparison_base
        .map(comparison_target_label)
        .unwrap_or_else(|| "-".to_string());
    let target = comparison_target
        .map(comparison_target_label)
        .unwrap_or_else(|| "-".to_string());
    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!("Comparing {base} -> {target}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("x clears compare and returns to history."),
    ];
    let remaining = viewport_lines.saturating_sub(lines.len()).max(1);
    lines.extend(repo_diff_lines(None, detail, scroll, remaining, theme));
    lines.truncate(viewport_lines.max(1));
    lines
}

fn repo_rebase_lines(
    detail: Option<&RepoDetail>,
    scroll: usize,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Rebase",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let Some(rebase) = detail.rebase_state.as_ref() else {
        return vec![
            Line::from(vec![Span::styled(
                "Rebase",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No rebase is currently active."),
            Line::from("Use i from Commits to start an interactive rebase."),
        ];
    };

    let mut body = vec![
        Line::from(format!(
            "Mode: {}  Step: {}/{}",
            rebase_kind_label(rebase.kind),
            rebase.step.max(1),
            rebase.total.max(rebase.step.max(1))
        )),
        Line::from(format!(
            "Branch: {}  Onto: {}",
            rebase.head_name.as_deref().unwrap_or("detached"),
            rebase.onto.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "Current: {}  {}",
            rebase.current_commit.as_deref().unwrap_or("-"),
            rebase
                .current_summary
                .as_deref()
                .unwrap_or("waiting for git metadata")
        )),
        Line::from("c continue  s skip  A abort  j/k scroll"),
        Line::from("Older-commit amend: switch to Status, press A to amend, then continue."),
    ];

    if rebase.todo_preview.is_empty() {
        body.push(Line::from("Todo: no queued rebase commands remain."));
    } else {
        body.push(Line::from("Upcoming commands:"));
        body.extend(
            rebase
                .todo_preview
                .iter()
                .map(|line| Line::from(format!("  {line}"))),
        );
    }

    if detail.diff.lines.is_empty() {
        body.push(Line::from(
            "Diff preview: no working tree diff for the current rebase step.",
        ));
    } else {
        body.push(Line::from("Diff preview:"));
        body.extend(detail.diff.lines.iter().take(8).map(|line| {
            render_diff_line(line.kind, &line.content, theme, false, false, false, false)
        }));
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "Interactive rebase control",
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )])];
    let visible_capacity = viewport_lines.saturating_sub(lines.len()).max(1);
    let max_scroll = body.len().saturating_sub(visible_capacity);
    let scroll = scroll.min(max_scroll);
    lines.extend(body.into_iter().skip(scroll).take(visible_capacity));
    lines.truncate(viewport_lines.max(1));
    lines
}

fn repo_diff_lines(
    repo_mode: Option<&RepoModeState>,
    detail: Option<&RepoDetail>,
    scroll: usize,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let detail = detail.or_else(|| repo_mode.and_then(|repo_mode| repo_mode.detail.as_ref()));
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Status diff",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let selected = detail
        .diff
        .selected_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "working tree".to_string());

    if detail.diff.lines.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                format!("Path: {selected}"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No diff available for the current selection."),
        ];
    }

    let header_lines = 3;
    let visible_capacity = viewport_lines.saturating_sub(header_lines).max(1);
    let max_scroll = detail.diff.lines.len().saturating_sub(visible_capacity);
    let scroll = scroll.min(max_scroll);
    let end = (scroll + visible_capacity).min(detail.diff.lines.len());

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Path: {selected} ({})",
                diff_presentation_label(detail.diff.presentation)
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "Hunks: {}  Selected: {}  Changed lines: {}  Showing {}-{}",
            detail.diff.hunk_count,
            selected_hunk_label(&detail.diff),
            selected_diff_line_label(repo_mode, &detail.diff),
            scroll + 1,
            end
        )),
        Line::from(format!(
            "Line select: J/K cursor  v range  L apply lines  Enter whole hunk  Range: {}",
            selected_diff_range_label(repo_mode, &detail.diff)
        )),
    ];

    lines.extend(
        detail.diff.lines[scroll..end]
            .iter()
            .enumerate()
            .map(|(offset, line)| {
                let absolute_index = scroll + offset;
                let selected_hunk = detail
                    .diff
                    .selected_hunk
                    .and_then(|index| detail.diff.hunks.get(index));
                let is_selected_hunk_line = selected_hunk.is_some_and(|hunk| {
                    (hunk.start_line_index..hunk.end_line_index).contains(&absolute_index)
                });
                let is_selected_line = repo_mode
                    .and_then(|repo_mode| repo_mode.diff_line_cursor)
                    .is_some_and(|line_index| line_index == absolute_index);
                let is_anchor_line = repo_mode
                    .and_then(|repo_mode| repo_mode.diff_line_anchor)
                    .is_some_and(|line_index| line_index == absolute_index);
                let is_selected_range_line = repo_mode
                    .and_then(|repo_mode| {
                        repo_mode.diff_line_cursor.map(|cursor| {
                            let anchor = repo_mode.diff_line_anchor.unwrap_or(cursor);
                            let start = anchor.min(cursor);
                            let end = anchor.max(cursor);
                            (start..=end).contains(&absolute_index)
                        })
                    })
                    .unwrap_or(false);
                render_diff_line(
                    line.kind,
                    &line.content,
                    theme,
                    is_selected_hunk_line
                        && detail.diff.presentation != DiffPresentation::Comparison,
                    is_selected_range_line,
                    is_anchor_line,
                    is_selected_line,
                )
            }),
    );
    lines
}

fn file_status_kind_label(kind: super_lazygit_core::FileStatusKind) -> &'static str {
    match kind {
        super_lazygit_core::FileStatusKind::Added => "A",
        super_lazygit_core::FileStatusKind::Deleted => "D",
        super_lazygit_core::FileStatusKind::Renamed => "R",
        super_lazygit_core::FileStatusKind::Untracked => "?",
        super_lazygit_core::FileStatusKind::Conflicted => "U",
        super_lazygit_core::FileStatusKind::Modified => "M",
    }
}

fn render_diff_line(
    kind: DiffLineKind,
    content: &str,
    theme: Theme,
    selected_hunk_line: bool,
    selected_range_line: bool,
    selected_anchor_line: bool,
    selected_line: bool,
) -> Line<'static> {
    let style = match kind {
        DiffLineKind::Meta => Style::default().fg(theme.muted),
        DiffLineKind::HunkHeader => Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
        DiffLineKind::Addition => Style::default().fg(theme.success),
        DiffLineKind::Removal => Style::default().fg(theme.danger),
        DiffLineKind::Context => Style::default().fg(theme.foreground),
    };
    let style = if selected_hunk_line {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    let style = if selected_range_line {
        style.add_modifier(Modifier::UNDERLINED)
    } else {
        style
    };
    let style = if selected_anchor_line {
        style.add_modifier(Modifier::ITALIC)
    } else {
        style
    };
    let style = if selected_line {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    };
    Line::from(Span::styled(content.to_string(), style))
}

fn diff_presentation_label(presentation: DiffPresentation) -> &'static str {
    match presentation {
        DiffPresentation::Unstaged => "unstaged",
        DiffPresentation::Staged => "staged",
        DiffPresentation::Comparison => "comparison",
    }
}

fn comparison_target_label(target: &super_lazygit_core::ComparisonTarget) -> String {
    match target {
        super_lazygit_core::ComparisonTarget::Branch(name)
        | super_lazygit_core::ComparisonTarget::Commit(name) => name.clone(),
    }
}

fn comparison_status_line(
    subview: RepoSubview,
    comparison_base: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_target: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_source: Option<RepoSubview>,
) -> String {
    if comparison_source != Some(subview) {
        return "Compare: press v to mark a base.".to_string();
    }

    match (comparison_base, comparison_target) {
        (Some(base), Some(target)) => format!(
            "Compare: {} -> {} (press x to clear).",
            comparison_target_label(base),
            comparison_target_label(target)
        ),
        (Some(base), None) => format!(
            "Compare base: {} (press v on another item, x clears).",
            comparison_target_label(base)
        ),
        _ => "Compare: press v to mark a base.".to_string(),
    }
}

fn selected_hunk_label(diff: &super_lazygit_core::DiffModel) -> String {
    match (diff.selected_hunk, diff.hunks.len()) {
        (Some(index), len) if len > 0 => format!("{}/{}", index + 1, len),
        _ => "0/0".to_string(),
    }
}

fn selected_diff_line_label(
    repo_mode: Option<&RepoModeState>,
    diff: &super_lazygit_core::DiffModel,
) -> String {
    let Some(selected_hunk) = diff.selected_hunk.and_then(|index| diff.hunks.get(index)) else {
        return "0/0".to_string();
    };
    let selectable = (selected_hunk.start_line_index + 1..selected_hunk.end_line_index)
        .filter(|line_index| {
            matches!(
                diff.lines[*line_index].kind,
                DiffLineKind::Addition | DiffLineKind::Removal
            )
        })
        .collect::<Vec<_>>();
    if selectable.is_empty() {
        return "0/0".to_string();
    }

    let current = repo_mode
        .and_then(|repo_mode| repo_mode.diff_line_cursor)
        .and_then(|cursor| {
            selectable
                .iter()
                .position(|line_index| *line_index == cursor)
        })
        .map(|index| index + 1)
        .unwrap_or(0);
    format!("{current}/{}", selectable.len())
}

fn selected_diff_range_label(
    repo_mode: Option<&RepoModeState>,
    diff: &super_lazygit_core::DiffModel,
) -> String {
    let Some(repo_mode) = repo_mode else {
        return "0 line(s)".to_string();
    };
    let Some(selected_hunk) = diff.selected_hunk.and_then(|index| diff.hunks.get(index)) else {
        return "0 line(s)".to_string();
    };
    let selectable = (selected_hunk.start_line_index + 1..selected_hunk.end_line_index)
        .filter(|line_index| {
            matches!(
                diff.lines[*line_index].kind,
                DiffLineKind::Addition | DiffLineKind::Removal
            )
        })
        .collect::<Vec<_>>();
    let Some(cursor_index) = repo_mode.diff_line_cursor.and_then(|cursor| {
        selectable
            .iter()
            .position(|line_index| *line_index == cursor)
    }) else {
        return "0 line(s)".to_string();
    };
    let count = repo_mode
        .diff_line_anchor
        .and_then(|anchor| {
            selectable
                .iter()
                .position(|line_index| *line_index == anchor)
        })
        .map(|anchor_index| anchor_index.abs_diff(cursor_index) + 1)
        .unwrap_or(1);
    format!("{count} line(s)")
}

fn operation_progress_label(progress: &super_lazygit_core::OperationProgress) -> String {
    match progress {
        super_lazygit_core::OperationProgress::Idle => "idle".to_string(),
        super_lazygit_core::OperationProgress::Running { summary, .. } => {
            format!("running: {summary}")
        }
        super_lazygit_core::OperationProgress::Failed { summary } => {
            format!("failed: {summary}")
        }
    }
}

fn confirmation_copy(operation: &super_lazygit_core::ConfirmableOperation) -> String {
    match operation {
        super_lazygit_core::ConfirmableOperation::Fetch => {
            "Fetch remote updates for the current repository?".to_string()
        }
        super_lazygit_core::ConfirmableOperation::Pull => {
            "Pull remote changes into the current branch?".to_string()
        }
        super_lazygit_core::ConfirmableOperation::Push => {
            "Push the current branch to its configured upstream?".to_string()
        }
        super_lazygit_core::ConfirmableOperation::DiscardFile { path } => {
            format!(
                "Discard all staged and unstaged changes for {}? This only affects the selected file, and untracked files will be removed.",
                path.display()
            )
        }
        super_lazygit_core::ConfirmableOperation::StartInteractiveRebase { summary, .. } => {
            format!(
                "Start an interactive rebase at {summary}? The selected commit will be marked edit."
            )
        }
        super_lazygit_core::ConfirmableOperation::AmendCommit { summary, .. } => {
            format!(
                "Start older-commit amend for {summary}? Git will pause on that commit so you can stage changes, amend it from Status, and continue from Rebase."
            )
        }
        super_lazygit_core::ConfirmableOperation::FixupCommit { summary, .. } => {
            format!(
                "Create a fixup commit for {summary} from the currently staged changes and autosquash it with rebase?"
            )
        }
        super_lazygit_core::ConfirmableOperation::CherryPickCommit { summary, .. } => {
            format!("Cherry-pick {summary} onto the current HEAD?")
        }
        super_lazygit_core::ConfirmableOperation::RevertCommit { summary, .. } => {
            format!("Revert {summary} with an automatic revert commit?")
        }
        super_lazygit_core::ConfirmableOperation::ResetToCommit { mode, summary, .. } => {
            match mode {
                super_lazygit_core::ResetMode::Soft => format!(
                    "Soft reset HEAD to {summary}? This moves HEAD only and keeps both the index and working tree intact."
                ),
                super_lazygit_core::ResetMode::Mixed => format!(
                    "Mixed reset HEAD to {summary}? This moves HEAD and resets the index, but keeps working tree changes."
                ),
                super_lazygit_core::ResetMode::Hard => format!(
                    "Hard reset HEAD to {summary}? This moves HEAD and discards tracked staged and unstaged changes."
                ),
            }
        }
        super_lazygit_core::ConfirmableOperation::RestoreReflogEntry {
            target, summary, ..
        } => {
            format!(
                "Restore HEAD to {summary}? This uses git reset --hard {target}, so only committed HEAD movement is recoverable. Working tree edits and untracked files are not undone here."
            )
        }
        super_lazygit_core::ConfirmableOperation::AbortRebase => {
            "Abort the current rebase and restore the branch to its pre-rebase state?".to_string()
        }
        super_lazygit_core::ConfirmableOperation::SkipRebase => {
            "Skip the current rebase step? Git will drop the commit being replayed.".to_string()
        }
        super_lazygit_core::ConfirmableOperation::NukeWorkingTree => {
            "Discard all local changes in this repository? This runs git reset --hard HEAD and git clean -fd, removing tracked edits and untracked files/directories.".to_string()
        }
        super_lazygit_core::ConfirmableOperation::DeleteBranch { branch_name } => {
            format!(
                "Delete local branch {branch_name}? Git will refuse if it is not safely merged."
            )
        }
        super_lazygit_core::ConfirmableOperation::PopStash { stash_ref } => {
            format!("Pop {stash_ref}? This applies it and removes it from the stash list.")
        }
        super_lazygit_core::ConfirmableOperation::DropStash { stash_ref } => {
            format!("Drop {stash_ref}? This permanently removes the stash entry.")
        }
        super_lazygit_core::ConfirmableOperation::RemoveWorktree { path } => {
            format!(
                "Remove linked worktree {}? Git will delete the worktree checkout.",
                path.display()
            )
        }
    }
}

fn history_operation_state_line(merge_state: &super_lazygit_core::MergeState) -> String {
    match merge_state {
        super_lazygit_core::MergeState::None => "State: idle".to_string(),
        super_lazygit_core::MergeState::MergeInProgress => "State: merge in progress".to_string(),
        super_lazygit_core::MergeState::RebaseInProgress => "State: rebase in progress".to_string(),
        super_lazygit_core::MergeState::CherryPickInProgress => {
            "State: cherry-pick in progress".to_string()
        }
        super_lazygit_core::MergeState::RevertInProgress => "State: revert in progress".to_string(),
    }
}

fn input_prompt_copy(operation: &super_lazygit_core::InputPromptOperation) -> String {
    match operation {
        super_lazygit_core::InputPromptOperation::CreateBranch => {
            "Enter the new branch name. The branch will be created from HEAD and checked out."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::RenameBranch { current_name } => {
            format!("Enter the new name for {current_name}.")
        }
        super_lazygit_core::InputPromptOperation::RenameStash { stash_ref, .. } => {
            format!(
                "Enter the new message for {stash_ref}. Leave it blank to use Git's default stash message."
            )
        }
        super_lazygit_core::InputPromptOperation::SetBranchUpstream { branch_name } => {
            format!("Enter the upstream ref for {branch_name}, for example origin/main.")
        }
        super_lazygit_core::InputPromptOperation::CreateStash { mode } => match mode {
            super_lazygit_core::StashMode::Tracked => {
                "Enter an optional stash message. Leave it blank to use Git's default tracked-changes stash message."
                    .to_string()
            }
            super_lazygit_core::StashMode::KeepIndex => {
                "Enter an optional stash message. Leave it blank to stash tracked worktree changes and keep staged changes in place."
                    .to_string()
            }
            super_lazygit_core::StashMode::IncludeUntracked => {
                "Enter an optional stash message. Leave it blank to use Git's default stash message while including untracked files."
                    .to_string()
            }
            super_lazygit_core::StashMode::Staged => {
                "Enter an optional stash message. Leave it blank to stash only the current index state."
                    .to_string()
            }
            super_lazygit_core::StashMode::Unstaged => {
                "Enter an optional stash message. Leave it blank to stash only tracked unstaged changes. If staged changes exist, they are restored after the stash is created."
                    .to_string()
            }
        },
        super_lazygit_core::InputPromptOperation::CreateWorktree => {
            "Enter worktree details as: <path> <branch>. Example: ../repo-feature feature."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::RewordCommit { summary, .. } => {
            format!(
                "Enter a replacement subject line for {summary}. This rewrites the selected commit message via rebase."
            )
        }
    }
}

fn menu_copy(operation: super_lazygit_core::MenuOperation) -> &'static str {
    match operation {
        super_lazygit_core::MenuOperation::StashOptions => {
            "Choose which stash scope to save from the current repository state."
        }
    }
}

fn menu_lines(menu: &super_lazygit_core::PendingMenu, theme: Theme) -> Vec<Line<'static>> {
    let items: &[&str] = match menu.operation {
        super_lazygit_core::MenuOperation::StashOptions => &[
            "Stash tracked changes",
            "Stash tracked changes and keep staged changes",
            "Stash all changes including untracked",
            "Stash staged changes",
            "Stash unstaged changes",
        ],
    };

    items
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let prefix = if index == menu.selected_index {
                "> "
            } else {
                "  "
            };
            let style = if index == menu.selected_index {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.foreground)
            };
            Line::from(Span::styled(format!("{prefix}{label}"), style))
        })
        .collect()
}

fn mode_label(mode: AppMode) -> &'static str {
    match mode {
        AppMode::Workspace => "WORKSPACE",
        AppMode::Repository => "REPOSITORY",
    }
}

fn status_text(state: &AppState) -> String {
    state
        .status_messages
        .back()
        .map(|message| message.text.clone())
        .or_else(|| {
            state
                .notifications
                .back()
                .map(|notification| notification.text.clone())
        })
        .unwrap_or_else(|| default_status_text(state))
}

fn help_text(state: &AppState) -> String {
    if !state.modal_stack.is_empty() {
        if state.pending_input_prompt.is_some() {
            return "Prompt overlay  type value  Enter submit  Backspace delete  Paste insert  Esc cancel"
                .to_string();
        }
        return "Esc close  q close overlay".to_string();
    }

    match state.mode {
        AppMode::Workspace => {
            if state.workspace.search_focused {
                "Workspace search  type to filter  Paste insert  Backspace delete  Enter keep  Esc clear".to_string()
            } else {
                "j/k move  / search  f filter  s sort  Enter open repo  Tab swap pane  r refresh  ? help".to_string()
            }
        }
        AppMode::Repository => repo_help_text(state),
    }
}

fn compile_keybinding_overrides(config: &AppConfig) -> BTreeMap<String, Vec<String>> {
    let mut overrides = BTreeMap::new();

    for override_config in &config.keybindings.overrides {
        let action_id = canonicalize_action_id(&override_config.action);
        if action_id.is_empty() {
            continue;
        }

        let entry = overrides.entry(action_id).or_insert_with(Vec::new);
        for key in &override_config.keys {
            let Some(key) = canonicalize_keybinding(key) else {
                continue;
            };

            if !entry.contains(&key) {
                entry.push(key);
            }
        }
    }

    overrides
}

fn canonicalize_action_id(action_id: &str) -> String {
    action_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn canonicalize_keybinding(key: &str) -> Option<String> {
    if key == " " {
        return Some(String::from("space"));
    }

    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.eq_ignore_ascii_case("space") {
        return Some(String::from("space"));
    }

    if trimmed.chars().count() == 1 {
        return Some(trimmed.to_string());
    }

    Some(trimmed.to_ascii_lowercase())
}

fn binding_matches_key(binding: &str, raw: &str, normalized: &str) -> bool {
    let Some(binding) = canonicalize_keybinding(binding) else {
        return false;
    };

    if binding == "space" {
        return raw == " " || normalized == "space";
    }

    if binding.chars().count() == 1 {
        return raw.trim() == binding;
    }

    normalized == binding
}

fn repo_unstaged_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
    progress: &super_lazygit_core::OperationProgress,
) -> Vec<Line<'static>> {
    let mut lines = repo_status_section_lines(
        detail,
        selected_index,
        is_focused,
        FileStatusSection::Unstaged,
    );
    lines.push(Line::from(format!(
        "Progress: {}",
        operation_progress_label(progress)
    )));
    lines
}

fn repo_staged_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
) -> Vec<Line<'static>> {
    repo_status_section_lines(
        detail,
        selected_index,
        is_focused,
        FileStatusSection::Staged,
    )
}

fn commit_box_title(mode: CommitBoxMode) -> &'static str {
    match mode {
        CommitBoxMode::Commit => "Commit box",
        CommitBoxMode::CommitNoVerify => "Commit without hooks",
        CommitBoxMode::Amend => "Amend HEAD",
    }
}

fn commit_box_lines(
    detail: Option<&RepoDetail>,
    mode: CommitBoxMode,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from("Repository detail is still loading."),
            Line::from("Esc cancel"),
        ];
    };

    let staged_count = detail
        .file_tree
        .iter()
        .filter(|item| item.staged_kind.is_some())
        .count();
    let has_commits = !detail.commits.is_empty();
    let trimmed = detail.commit_input.trim();
    let message = if detail.commit_input.is_empty() {
        "_".to_string()
    } else {
        format!("{}_", detail.commit_input)
    };

    let validation = match mode {
        CommitBoxMode::Commit if staged_count == 0 => {
            "Validation: stage at least one file before committing.".to_string()
        }
        CommitBoxMode::Commit if trimmed.is_empty() => {
            "Validation: enter a commit message before confirming.".to_string()
        }
        CommitBoxMode::Commit => {
            format!("Ready: create a commit from {staged_count} staged file(s).")
        }
        CommitBoxMode::CommitNoVerify if staged_count == 0 => {
            "Validation: stage at least one file before committing.".to_string()
        }
        CommitBoxMode::CommitNoVerify if trimmed.is_empty() => {
            "Validation: enter a commit message before confirming.".to_string()
        }
        CommitBoxMode::CommitNoVerify => {
            format!("Ready: create a no-verify commit from {staged_count} staged file(s).")
        }
        CommitBoxMode::Amend if !has_commits => {
            "Validation: no commits available to amend.".to_string()
        }
        CommitBoxMode::Amend if trimmed.is_empty() => {
            "Ready: amend HEAD and keep the current commit message.".to_string()
        }
        CommitBoxMode::Amend => "Ready: amend HEAD with the edited message.".to_string(),
    };

    vec![
        Line::from(vec![Span::styled(
            match mode {
                CommitBoxMode::Commit => "Type a new commit message without leaving status view.",
                CommitBoxMode::CommitNoVerify => {
                    "Type a commit message and skip pre-commit hooks for this commit."
                }
                CommitBoxMode::Amend => {
                    "Type a replacement HEAD message, or leave it blank to reuse it."
                }
            },
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "Staged files: {staged_count}  Existing commits: {}",
            detail.commits.len()
        )),
        Line::from(vec![
            Span::styled("Message: ", Style::default().fg(theme.accent)),
            Span::raw(message),
        ]),
        Line::from(validation),
        Line::from("Enter confirm  Esc cancel  Backspace delete  Paste insert"),
    ]
}

#[derive(Clone, Copy)]
enum FileStatusSection {
    Staged,
    Unstaged,
}

fn repo_status_section_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    is_focused: bool,
    section: FileStatusSection,
) -> Vec<Line<'static>> {
    let (focus_text, empty_text) = match section {
        FileStatusSection::Unstaged => (
            if is_focused {
                "j/k move  Enter stage selected file."
            } else {
                "Move focus here to inspect working tree changes."
            },
            "No working tree changes.",
        ),
        FileStatusSection::Staged => (
            if is_focused {
                "j/k move  Enter unstage selected file."
            } else {
                "Move focus here to prep staged work."
            },
            "No staged changes.",
        ),
    };

    let mut lines = vec![Line::from(focus_text)];
    let Some(detail) = detail else {
        lines.push(Line::from("Repository detail is still loading."));
        return lines;
    };

    let entries = detail
        .file_tree
        .iter()
        .filter_map(|item| {
            let kind = match section {
                FileStatusSection::Staged => item.staged_kind,
                FileStatusSection::Unstaged => item.unstaged_kind,
            }?;
            Some((kind, item.path.display().to_string()))
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        lines.push(Line::from(empty_text));
        return lines;
    }

    lines.push(Line::from(format!("Files: {}", entries.len())));
    lines.extend(
        entries
            .into_iter()
            .enumerate()
            .map(|(index, (kind, path))| {
                let marker = if selected_index == Some(index) {
                    ">"
                } else {
                    " "
                };
                Line::from(format!("{marker} {} {path}", file_status_kind_label(kind)))
            }),
    );
    lines
}

fn repo_subview_label(subview: RepoSubview) -> &'static str {
    match subview {
        RepoSubview::Status => "Status",
        RepoSubview::Branches => "Branches",
        RepoSubview::Commits => "Commits",
        RepoSubview::Compare => "Compare",
        RepoSubview::Rebase => "Rebase",
        RepoSubview::Stash => "Stash",
        RepoSubview::Reflog => "Reflog",
        RepoSubview::Worktrees => "Worktrees",
    }
}

fn repo_subview_tabs(active: RepoSubview) -> Vec<Span<'static>> {
    let all = [
        (RepoSubview::Status, "1 Status"),
        (RepoSubview::Branches, "2 Branches"),
        (RepoSubview::Commits, "3 Commits"),
        (RepoSubview::Compare, "4 Compare"),
        (RepoSubview::Rebase, "5 Rebase"),
        (RepoSubview::Stash, "6 Stash"),
        (RepoSubview::Reflog, "7 Reflog"),
        (RepoSubview::Worktrees, "8 Worktrees"),
    ];

    let mut spans = Vec::with_capacity(all.len() * 2);
    for (index, (subview, label)) in all.iter().enumerate() {
        let rendered = if *subview == active {
            format!("[{label}]")
        } else {
            label.to_string()
        };
        spans.push(Span::raw(rendered));
        if index + 1 < all.len() {
            spans.push(Span::raw("  "));
        }
    }
    spans
}

fn default_status_text(state: &AppState) -> String {
    match state.mode {
        AppMode::Workspace => {
            if state.workspace.search_focused {
                "Workspace search focused; type to filter repos, Enter keeps it, and Esc clears it."
                    .to_string()
            } else {
                format!(
                    "Workspace triage ready; {} repo(s) visible with {} sort and {} filter.",
                    state.workspace.visible_repo_ids().len(),
                    state.workspace.sort_mode.label(),
                    state.workspace.filter_mode.label()
                )
            }
        }
        AppMode::Repository => {
            if let Some(repo_mode) = state
                .repo_mode
                .as_ref()
                .filter(|repo_mode| repo_mode.commit_box.focused)
            {
                return match repo_mode.commit_box.mode {
                    CommitBoxMode::Commit => {
                        "Commit box focused; type a message, Enter commits, and Esc cancels."
                            .to_string()
                    }
                    CommitBoxMode::CommitNoVerify => {
                        "No-verify commit box focused; type a message, Enter commits without hooks, and Esc cancels."
                            .to_string()
                    }
                    CommitBoxMode::Amend => {
                        "Amend box focused; Enter confirms, Esc cancels, and blank input keeps the HEAD message."
                            .to_string()
                    }
                };
            }

            match state.focused_pane {
            PaneId::RepoUnstaged => {
                "Working tree focus; j/k move, Enter stages, and D discards the selected file."
                    .to_string()
            }
            PaneId::RepoStaged => {
                "Staged focus; j/k move, Enter unstages, D discards, c commits, w commits without hooks, and A amends HEAD."
                    .to_string()
            }
            PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
                || "Repository shell ready.".to_string(),
                |repo_mode| {
                    if repo_mode.active_subview == RepoSubview::Status {
                        "Status diff focus; j/k move hunks, D discards the current file, and X nukes the working tree."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Branches {
                        "Branches detail focus; j/k move, v compares refs, x clears compare, Enter checks out, c creates, R renames, d deletes, and u sets upstream."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Commits {
                        "Commits detail focus; j/k browse history, i starts a rebase, C cherry-picks, V reverts, S/M/H reset HEAD, v compares commits, and x clears compare."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Compare {
                        "Compare detail focus; j/k scroll the comparison diff and x clears compare."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Rebase {
                        "Rebase detail focus; c continues, s skips, A aborts, and j/k scroll the active step."
                            .to_string()
                    } else {
                        format!(
                            "{} detail focus; deeper interactions are staged behind the shell bead.",
                            repo_subview_label(repo_mode.active_subview)
                        )
                    }
                },
            ),
            _ => "Repository shell ready.".to_string(),
        }
        }
    }
}

fn repo_help_text(state: &AppState) -> String {
    if let Some(repo_mode) = state
        .repo_mode
        .as_ref()
        .filter(|repo_mode| repo_mode.commit_box.focused)
    {
        return match repo_mode.commit_box.mode {
            CommitBoxMode::Commit => {
                "Commit box  type message  Enter commit  Esc cancel  Backspace delete  Paste insert".to_string()
            }
            CommitBoxMode::CommitNoVerify => {
                "No-verify commit box  type message  Enter commit without hooks  Esc cancel  Backspace delete  Paste insert".to_string()
            }
            CommitBoxMode::Amend => {
                "Amend box  type message  Enter amend HEAD  Esc cancel  Backspace delete  Paste insert".to_string()
            }
        };
    }

    match state.focused_pane {
        PaneId::RepoUnstaged => {
            "Working tree pane  j/k move  Enter stage file  s stash tracked changes  S stash options  D discard file  l next pane  1-8 detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoStaged => {
            "Staged pane  j/k move  Enter unstage file  s stash tracked changes  S stash options  D discard file  c commit  A amend HEAD  h/l change pane  1-8 detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
            || "Repository shell".to_string(),
            |repo_mode| {
                if repo_mode.active_subview == RepoSubview::Status {
                    "Status diff pane  j/k scroll diff  D discard file  X nuke working tree  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Branches {
                    "Branches pane  j/k move  v compare  x clear compare  Enter checkout  c create  R rename  d delete  u upstream  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Commits {
                    "Commits pane  j/k move commit  i start rebase  S soft reset  M mixed reset  H hard reset  v compare  x clear compare  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Compare {
                    "Compare pane  j/k scroll diff  x clear compare  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Rebase {
                    "Rebase pane  c continue  s skip  A abort  j/k scroll  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else {
                    format!(
                        "{} detail pane  h left pane  1-8 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace",
                        repo_subview_label(repo_mode.active_subview)
                    )
                }
            },
        ),
        _ => "Repository shell".to_string(),
    }
}

fn rebase_kind_label(kind: super_lazygit_core::RebaseKind) -> &'static str {
    match kind {
        super_lazygit_core::RebaseKind::Interactive => "interactive",
        super_lazygit_core::RebaseKind::Apply => "apply",
    }
}

fn workspace_status_line(state: &AppState, visible_count: usize, theme: Theme) -> Line<'static> {
    let scan = workspace_scan_label(&state.workspace.scan_status);
    let issues = workspace_repo_issue_count(state);

    let search = if state.workspace.search_query.is_empty() {
        "-".to_string()
    } else {
        truncate_cell(&state.workspace.search_query, 18)
    };

    Line::from(vec![
        Span::styled(
            format!(
                "repos={visible_count}/{}",
                state.workspace.discovered_repo_ids.len()
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("filter={}", state.workspace.filter_mode.label()),
            Style::default().fg(theme.foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!("sort={}", state.workspace.sort_mode.label()),
            Style::default().fg(theme.foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "search={}{}",
                search,
                if state.workspace.search_focused {
                    "*"
                } else {
                    ""
                }
            ),
            Style::default().fg(theme.foreground),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "watch={}",
                watcher_health_label(&state.workspace.watcher_health)
            ),
            workspace_watch_style(&state.workspace.watcher_health, theme),
        ),
        Span::raw("  "),
        Span::styled(
            format!("issues={issues}"),
            if issues == 0 {
                Style::default().fg(theme.muted)
            } else {
                Style::default()
                    .fg(theme.danger)
                    .add_modifier(Modifier::BOLD)
            },
        ),
        Span::raw("  "),
        Span::styled(format!("scan={scan}"), Style::default().fg(theme.muted)),
    ])
}

fn workspace_scan_label(scan_status: &super_lazygit_core::ScanStatus) -> String {
    match scan_status {
        super_lazygit_core::ScanStatus::Idle => "idle".to_string(),
        super_lazygit_core::ScanStatus::Scanning => "scanning".to_string(),
        super_lazygit_core::ScanStatus::Complete { scanned_repos } => {
            format!("ready:{scanned_repos}")
        }
        super_lazygit_core::ScanStatus::Failed { message } => {
            format!("failed:{}", truncate_cell(message, 18))
        }
    }
}

fn workspace_repo_issue_count(state: &AppState) -> usize {
    state
        .workspace
        .discovered_repo_ids
        .iter()
        .filter(|repo_id| {
            state
                .workspace
                .repo_summaries
                .get(*repo_id)
                .is_none_or(|summary| summary.last_error.is_some())
        })
        .count()
}

fn watcher_health_label(health: &super_lazygit_core::WatcherHealth) -> &'static str {
    match health {
        super_lazygit_core::WatcherHealth::Unknown => "unknown",
        super_lazygit_core::WatcherHealth::Healthy => "live",
        super_lazygit_core::WatcherHealth::Degraded { .. } => "polling",
    }
}

fn workspace_watch_style(health: &super_lazygit_core::WatcherHealth, theme: Theme) -> Style {
    match health {
        super_lazygit_core::WatcherHealth::Degraded { .. } => Style::default()
            .fg(theme.danger)
            .add_modifier(Modifier::BOLD),
        super_lazygit_core::WatcherHealth::Healthy => Style::default().fg(theme.success),
        super_lazygit_core::WatcherHealth::Unknown => Style::default().fg(theme.muted),
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return None;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(red, green, blue))
}

fn buffer_to_string(buffer: &Buffer) -> String {
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use super_lazygit_core::{
        BranchItem, CommitFileItem, CommitItem, ComparisonTarget, DiffLine, DiffLineKind,
        DiffModel, FileStatus, FileStatusKind, ModalKind, RebaseKind, RebaseState, RepoModeState,
        StatusMessage, Timestamp, WorkspaceFilterMode, WorkspaceState,
    };

    fn sample_repo_detail() -> RepoDetail {
        RepoDetail {
            file_tree: vec![
                FileStatus {
                    path: PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: Some(FileStatusKind::Modified),
                    unstaged_kind: Some(FileStatusKind::Modified),
                },
                FileStatus {
                    path: PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Untracked),
                },
                FileStatus {
                    path: PathBuf::from("Cargo.toml"),
                    kind: FileStatusKind::Added,
                    staged_kind: Some(FileStatusKind::Added),
                    unstaged_kind: None,
                },
            ],
            diff: DiffModel {
                selected_path: Some(PathBuf::from("src/lib.rs")),
                presentation: DiffPresentation::Unstaged,
                lines: vec![
                    DiffLine {
                        kind: DiffLineKind::Meta,
                        content: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Meta,
                        content: "index 1111111..2222222 100644".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::HunkHeader,
                        content: "@@ -1 +1 @@".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Removal,
                        content: "-old line".to_string(),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "+new line".to_string(),
                    },
                ],
                hunks: vec![super_lazygit_core::DiffHunk {
                    header: "@@ -1 +1 @@".to_string(),
                    selection: super_lazygit_core::SelectedHunk {
                        old_start: 1,
                        old_lines: 1,
                        new_start: 1,
                        new_lines: 1,
                    },
                    start_line_index: 2,
                    end_line_index: 5,
                }],
                selected_hunk: Some(0),
                hunk_count: 1,
            },
            branches: vec![
                BranchItem {
                    name: "main".to_string(),
                    is_head: true,
                    upstream: Some("origin/main".to_string()),
                },
                BranchItem {
                    name: "feature".to_string(),
                    is_head: false,
                    upstream: None,
                },
            ],
            commits: vec![
                CommitItem {
                    oid: "abcdef1234567890".to_string(),
                    short_oid: "abcdef1".to_string(),
                    summary: "add lib".to_string(),
                    changed_files: vec![CommitFileItem {
                        path: PathBuf::from("src/lib.rs"),
                        kind: FileStatusKind::Added,
                    }],
                    diff: DiffModel {
                        selected_path: None,
                        presentation: DiffPresentation::Comparison,
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::Meta,
                                content: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::HunkHeader,
                                content: "@@ -0,0 +1 @@".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+pub fn answer() -> u32 {".to_string(),
                            },
                        ],
                        hunks: vec![super_lazygit_core::DiffHunk {
                            header: "@@ -0,0 +1 @@".to_string(),
                            selection: super_lazygit_core::SelectedHunk {
                                old_start: 0,
                                old_lines: 0,
                                new_start: 1,
                                new_lines: 1,
                            },
                            start_line_index: 1,
                            end_line_index: 3,
                        }],
                        selected_hunk: Some(0),
                        hunk_count: 1,
                    },
                },
                CommitItem {
                    oid: "1234567890abcdef".to_string(),
                    short_oid: "1234567".to_string(),
                    summary: "second".to_string(),
                    changed_files: vec![CommitFileItem {
                        path: PathBuf::from("notes.md"),
                        kind: FileStatusKind::Added,
                    }],
                    diff: DiffModel {
                        selected_path: None,
                        presentation: DiffPresentation::Comparison,
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::Meta,
                                content: "diff --git a/notes.md b/notes.md".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+# Notes".to_string(),
                            },
                        ],
                        hunks: Vec::new(),
                        selected_hunk: None,
                        hunk_count: 0,
                    },
                },
            ],
            stashes: vec![
                super_lazygit_core::StashItem {
                    stash_ref: "stash@{0}".to_string(),
                    label: "stash@{0}: WIP on main: fixture stash".to_string(),
                },
                super_lazygit_core::StashItem {
                    stash_ref: "stash@{1}".to_string(),
                    label: "stash@{1}: On feature: prior experiment".to_string(),
                },
            ],
            reflog_items: vec![
                super_lazygit_core::ReflogItem {
                    description: "HEAD@{0}: checkout: moving from feature to main".to_string(),
                },
                super_lazygit_core::ReflogItem {
                    description: "HEAD@{1}: commit: add repo-mode stash flows".to_string(),
                },
            ],
            worktrees: vec![
                super_lazygit_core::WorktreeItem {
                    path: PathBuf::from("/tmp/repo-1"),
                    branch: Some("main".to_string()),
                },
                super_lazygit_core::WorktreeItem {
                    path: PathBuf::from("/tmp/repo-1-feature"),
                    branch: Some("feature".to_string()),
                },
            ],
            ..Default::default()
        }
    }

    fn workspace_repo_summary(repo_id: &str, display_name: &str) -> RepoSummary {
        RepoSummary {
            repo_id: RepoId::new(repo_id),
            display_name: display_name.to_string(),
            display_path: repo_id.to_string(),
            real_path: PathBuf::from(repo_id),
            branch: Some("main".to_string()),
            ..RepoSummary::default()
        }
    }

    #[test]
    fn render_workspace_shell_shows_status_and_help() {
        let mut state = AppState {
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            status_messages: std::collections::VecDeque::from([StatusMessage::info(
                1,
                "Ready to inspect",
            )]),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                dirty: true,
                staged_count: 1,
                unstaged_count: 2,
                untracked_count: 3,
                ahead_count: 4,
                behind_count: 1,
                last_fetch_at: Some(super_lazygit_core::Timestamp(60)),
                last_refresh_at: Some(super_lazygit_core::Timestamp(180)),
                ..Default::default()
            },
        );

        let mut app = TuiApp::new(state.clone(), AppConfig::default());
        app.resize(120, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Workspace"));
        assert!(rendered.contains("Preview"));
        assert!(rendered.contains("WORKSPACE"));
        assert!(rendered.contains("Ready to inspect"));
        assert!(rendered.contains("repos=1"));
        assert!(rendered.contains("filter=all"));
        assert!(rendered.contains("REPO"));
        assert!(rendered.contains("BR"));
        assert!(rendered.contains("STATE"));
        assert!(rendered.contains("AGE"));
        assert!(rendered.contains("repo-1"));
        assert!(rendered.contains("1/2/3 +4/-1"));
        assert!(rendered.contains("2m"));
        assert!(rendered.contains("Path: /tmp/repo-1"));
        assert!(rendered.contains("Branch: main"));
    }

    #[test]
    fn render_workspace_shell_shows_wide_repo_table_columns() {
        let mut state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1"), RepoId::new("repo-2")],
                selected_repo_id: Some(RepoId::new("repo-2")),
                ..Default::default()
            },
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                dirty: false,
                last_fetch_at: Some(super_lazygit_core::Timestamp(100)),
                last_refresh_at: Some(super_lazygit_core::Timestamp(130)),
                ..Default::default()
            },
        );
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-2"),
            RepoSummary {
                repo_id: RepoId::new("repo-2"),
                display_name: "repo-2".to_string(),
                display_path: "/tmp/repo-2".to_string(),
                branch: Some("feature/workspace-table".to_string()),
                dirty: true,
                staged_count: 2,
                unstaged_count: 1,
                untracked_count: 0,
                ahead_count: 3,
                behind_count: 2,
                last_fetch_at: Some(super_lazygit_core::Timestamp(25)),
                last_refresh_at: Some(super_lazygit_core::Timestamp(95)),
                ..Default::default()
            },
        );

        let mut app = TuiApp::new(state.clone(), AppConfig::default());
        app.resize(160, 22);

        let rendered = app.render_to_string();

        assert!(rendered.contains("REPO"));
        assert!(rendered.contains("BRANCH"));
        assert!(rendered.contains("DIRTY"));
        assert!(rendered.contains("SYNC"));
        assert!(rendered.contains("FETCH"));
        assert!(rendered.contains("> repo-2"));
        assert!(rendered.contains("2S 1U 0?"));
        assert!(rendered.contains("+3/-2"));
        assert!(rendered.contains("1m"));
    }

    #[test]
    fn render_workspace_shell_uses_visible_repo_list_and_reports_triage_state() {
        let repo_alpha = RepoId::new("/tmp/alpha");
        let repo_beta = RepoId::new("/tmp/beta-service");
        let repo_gamma = RepoId::new("/tmp/gamma");
        let mut beta = workspace_repo_summary(&repo_beta.0, "beta");
        beta.behind_count = 3;
        beta.last_local_activity_at = Some(Timestamp(90));
        beta.last_refresh_at = Some(Timestamp(120));
        let mut gamma = workspace_repo_summary(&repo_gamma.0, "gamma");
        gamma.dirty = true;
        gamma.unstaged_count = 2;
        gamma.last_refresh_at = Some(Timestamp(120));
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![
                    repo_alpha.clone(),
                    repo_beta.clone(),
                    repo_gamma.clone(),
                ],
                repo_summaries: std::collections::BTreeMap::from([
                    (
                        repo_alpha.clone(),
                        workspace_repo_summary(&repo_alpha.0, "alpha"),
                    ),
                    (repo_beta.clone(), beta),
                    (repo_gamma.clone(), gamma),
                ]),
                selected_repo_id: Some(repo_alpha),
                filter_mode: WorkspaceFilterMode::BehindOnly,
                search_query: "bta".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());
        app.resize(180, 22);

        let rendered = app.render_to_string();

        assert!(rendered.contains("repos=1/3"));
        assert!(rendered.contains("filter=behind"));
        assert!(rendered.contains("sort=attention"));
        assert!(rendered.contains("search=bta"));
        assert!(rendered.contains("issues=0"));
        assert!(rendered.contains("beta"));
        assert!(!rendered.contains("gamma"));
        assert!(rendered.contains("Attention:"));
    }

    #[test]
    fn render_workspace_shell_shows_polling_badges_when_watcher_is_degraded() {
        let repo_id = RepoId::new("/tmp/repo");
        let mut summary = workspace_repo_summary(&repo_id.0, "repo");
        summary.watcher_freshness = super_lazygit_core::WatcherFreshness::Stale;
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(repo_id.clone(), summary)]),
                selected_repo_id: Some(repo_id),
                watcher_health: super_lazygit_core::WatcherHealth::Degraded {
                    message: "watch backend unavailable".to_string(),
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());
        app.resize(160, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("watch=polling"));
        assert!(rendered.contains("Watcher: Stale"));
    }

    #[test]
    fn render_workspace_shell_shows_scanning_pending_preview() {
        let repo_id = RepoId::new("/tmp/repo-pending");
        let state = AppState {
            workspace: WorkspaceState {
                current_root: Some(PathBuf::from("/tmp/workspace")),
                discovered_repo_ids: vec![repo_id.clone()],
                selected_repo_id: Some(repo_id),
                scan_status: super_lazygit_core::ScanStatus::Scanning,
                ..WorkspaceState::default()
            },
            ..AppState::default()
        };
        let theme = Theme::from_config(&AppConfig::default());
        let status_line = workspace_status_line(&state, 1, theme);
        let status_text = status_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let preview_lines = workspace_pending_preview_lines(
            state
                .workspace
                .selected_repo_id
                .as_ref()
                .expect("selected repo"),
            &state,
        );
        let preview_text = preview_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(status_text.contains("scan=scanning"));
        assert!(status_text.contains("issues=1"));
        assert!(preview_text.contains("State: waiting for repository summary"));
    }

    #[test]
    fn render_workspace_shell_shows_scan_failure_empty_state() {
        let state = AppState {
            workspace: WorkspaceState {
                current_root: Some(PathBuf::from("/tmp/workspace")),
                scan_status: super_lazygit_core::ScanStatus::Failed {
                    message: "permission denied".to_string(),
                },
                ..WorkspaceState::default()
            },
            ..AppState::default()
        };
        let theme = Theme::from_config(&AppConfig::default());
        let status_line = workspace_status_line(&state, 0, theme);
        let status_text = status_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let empty_text = workspace_empty_list_lines(&state)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(empty_text.contains("Workspace scan failed."));
        assert!(empty_text.contains("Reason: permission denied"));
        assert!(status_text.contains("scan=failed:permission denied"));
    }

    #[test]
    fn render_workspace_preview_surfaces_summary_errors() {
        let repo_id = RepoId::new("/tmp/repo-error");
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                selected_repo_id: Some(repo_id.clone()),
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    RepoSummary {
                        repo_id,
                        display_name: "repo-error".to_string(),
                        display_path: "/tmp/repo-error".to_string(),
                        real_path: PathBuf::from("/tmp/repo-error"),
                        last_error: Some("summary refresh failed".to_string()),
                        ..RepoSummary::default()
                    },
                )]),
                scan_status: super_lazygit_core::ScanStatus::Complete { scanned_repos: 1 },
                ..WorkspaceState::default()
            },
            ..AppState::default()
        };
        let theme = Theme::from_config(&AppConfig::default());
        let status_line = workspace_status_line(&state, 1, theme);
        let status_text = status_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let preview_text = workspace_preview_lines(
            state
                .workspace
                .repo_summaries
                .get(&RepoId::new("/tmp/repo-error"))
                .expect("summary"),
        )
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
        let dirty = workspace_dirty_cell(
            state
                .workspace
                .repo_summaries
                .get(&RepoId::new("/tmp/repo-error"))
                .expect("summary"),
            false,
        );

        assert!(preview_text.contains("Last error: summary refresh failed"));
        assert!(status_text.contains("issues=1"));
        assert_eq!(dirty, "error");
    }

    #[test]
    fn route_workspace_enter_opens_repo_mode() {
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert!(result.state.repo_mode.is_some());
    }

    #[test]
    fn route_workspace_editor_binding_opens_selected_repo_root() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from(&repo_id.0);
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "e".to_string(),
        })));

        assert_eq!(
            result.effects,
            vec![super_lazygit_core::Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: repo_root,
            }]
        );
    }

    #[test]
    fn route_workspace_override_accepts_legacy_action_name_and_replaces_enter() {
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut config = AppConfig::default();
        config.keybindings.overrides = vec![super_lazygit_config::KeybindingOverride {
            action: "EnterRepoMode".to_string(),
            keys: vec!["o".to_string()],
        }];
        let mut app = TuiApp::new(state, config);

        let default_key = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(default_key.state.mode, AppMode::Workspace);
        assert!(default_key.state.repo_mode.is_none());

        let override_key = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert_eq!(override_key.state.mode, AppMode::Repository);
        assert_eq!(override_key.state.focused_pane, PaneId::RepoUnstaged);
        assert!(override_key.state.repo_mode.is_some());
    }

    #[test]
    fn route_repository_escape_returns_to_workspace_context() {
        let repo_alpha = RepoId::new("/tmp/alpha");
        let repo_beta = RepoId::new("/tmp/beta");
        let mut beta_summary = workspace_repo_summary(&repo_beta.0, "beta");
        beta_summary.dirty = true;
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_alpha.clone(), repo_beta.clone()],
                repo_summaries: std::collections::BTreeMap::from([
                    (
                        repo_alpha.clone(),
                        workspace_repo_summary(&repo_alpha.0, "alpha"),
                    ),
                    (repo_beta.clone(), beta_summary),
                ]),
                selected_repo_id: Some(repo_beta.clone()),
                filter_mode: WorkspaceFilterMode::DirtyOnly,
                search_query: "beta".to_string(),
                search_focused: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let entered = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(entered.state.mode, AppMode::Repository);
        assert_eq!(
            entered.state.workspace.selected_repo_id,
            Some(repo_beta.clone())
        );
        assert_eq!(
            entered.state.workspace.filter_mode,
            WorkspaceFilterMode::DirtyOnly
        );
        assert_eq!(entered.state.workspace.search_query, "beta");
        assert!(!entered.state.workspace.search_focused);

        let returned = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "esc".to_string(),
        })));
        assert_eq!(returned.state.mode, AppMode::Workspace);
        assert_eq!(returned.state.focused_pane, PaneId::WorkspaceList);
        assert_eq!(returned.state.workspace.selected_repo_id, Some(repo_beta));
        assert_eq!(
            returned.state.workspace.filter_mode,
            WorkspaceFilterMode::DirtyOnly
        );
        assert_eq!(returned.state.workspace.search_query, "beta");
        assert!(!returned.state.workspace.search_focused);
        assert!(returned.state.repo_mode.is_none());
    }

    #[test]
    fn route_repo_override_replaces_uppercase_push_binding() {
        let repo_id = RepoId::new("repo-1");
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            repo_id.clone(),
            workspace_repo_summary(&repo_id.0, "repo-1"),
        );

        let mut config = AppConfig::default();
        config.keybindings.overrides = vec![super_lazygit_config::KeybindingOverride {
            action: "push_current_branch".to_string(),
            keys: vec!["g".to_string()],
        }];
        let mut app = TuiApp::new(state, config);

        let default_key = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "P".to_string(),
        })));
        assert!(default_key.state.pending_confirmation.is_none());
        assert!(default_key.state.modal_stack.is_empty());
        assert!(default_key.effects.is_empty());

        let override_key = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert!(matches!(
            override_key
                .state
                .modal_stack
                .last()
                .map(|modal| modal.kind),
            Some(ModalKind::Confirm)
        ));
        assert!(override_key.state.pending_confirmation.is_some());
    }

    #[test]
    fn route_repo_editor_binding_opens_selected_status_file() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from(&repo_id.0);
        let mut detail = sample_repo_detail();
        detail.diff.selected_path = Some(std::path::PathBuf::from("src/lib.rs"));
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(detail),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            repo_id.clone(),
            workspace_repo_summary(&repo_id.0, "repo-1"),
        );
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "e".to_string(),
        })));

        assert_eq!(
            result.effects,
            vec![super_lazygit_core::Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: repo_root.join("src/lib.rs"),
            }]
        );
    }

    #[test]
    fn route_workspace_search_focus_paste_and_clear() {
        let repo_alpha = RepoId::new("/tmp/alpha");
        let repo_beta = RepoId::new("/tmp/beta");
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_alpha.clone(), repo_beta.clone()],
                repo_summaries: std::collections::BTreeMap::from([
                    (
                        repo_alpha.clone(),
                        workspace_repo_summary(&repo_alpha.0, "alpha"),
                    ),
                    (
                        repo_beta.clone(),
                        workspace_repo_summary(&repo_beta.0, "beta"),
                    ),
                ]),
                selected_repo_id: Some(repo_alpha.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let focused = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "/".to_string(),
        })));
        assert!(focused.state.workspace.search_focused);

        let pasted = app.dispatch(Event::Input(InputEvent::Paste("bet".to_string())));
        assert_eq!(pasted.state.workspace.search_query, "bet");
        assert_eq!(
            pasted.state.workspace.selected_repo_id,
            Some(repo_beta.clone())
        );
        assert!(pasted.state.workspace.search_focused);

        let blurred = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(!blurred.state.workspace.search_focused);
        assert_eq!(blurred.state.workspace.search_query, "bet");

        let cleared = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "esc".to_string(),
        })));
        assert!(cleared.state.workspace.search_query.is_empty());
        assert_eq!(cleared.state.workspace.selected_repo_id, Some(repo_beta));
    }

    #[test]
    fn route_workspace_filter_and_sort_keys_cycle_triage_modes() {
        let mut app = TuiApp::new(AppState::default(), AppConfig::default());

        let filter = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "f".to_string(),
        })));
        assert_eq!(
            filter.state.workspace.filter_mode,
            WorkspaceFilterMode::DirtyOnly
        );

        let sort = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "s".to_string(),
        })));
        assert_eq!(
            sort.state.workspace.sort_mode,
            super_lazygit_core::WorkspaceSortMode::Name
        );
    }

    #[test]
    fn route_resize_updates_viewport_without_mutating_state() {
        let mut app = TuiApp::new(AppState::default(), AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::Resize {
            width: 101,
            height: 33,
        }));

        assert_eq!(
            app.viewport(),
            Viewport {
                width: 101,
                height: 33
            }
        );
        assert_eq!(result.state, AppState::default());
        assert_eq!(
            result.effects,
            vec![super_lazygit_core::Effect::ScheduleRender]
        );
    }

    #[test]
    fn modal_overlay_takes_focus_and_renders() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(ModalKind::Help, "Help")],
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();
        assert!(rendered.contains("Modal"));
        assert!(rendered.contains("Esc closes this overlay."));
    }

    #[test]
    fn confirm_modal_renders_repo_specific_copy() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                ModalKind::Confirm,
                "Confirm pull",
            )],
            pending_confirmation: Some(super_lazygit_core::PendingConfirmation {
                repo_id: RepoId::new("repo-1"),
                operation: super_lazygit_core::ConfirmableOperation::Pull,
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();
        assert!(rendered.contains("Confirm pull"));
        assert!(rendered.contains("Repo: repo-1"));
        assert!(rendered.contains("Enter or y confirms"));
    }

    #[test]
    fn confirm_modal_renders_nuke_working_tree_copy() {
        let copy = confirmation_copy(&super_lazygit_core::ConfirmableOperation::NukeWorkingTree);

        assert!(copy.contains("git reset --hard HEAD"));
        assert!(copy.contains("git clean -fd"));
    }

    #[test]
    fn confirm_modal_renders_history_operation_copy() {
        let amend = confirmation_copy(&super_lazygit_core::ConfirmableOperation::AmendCommit {
            commit: "1234567890abcdef".to_string(),
            summary: "1234567 second".to_string(),
        });
        let fixup = confirmation_copy(&super_lazygit_core::ConfirmableOperation::FixupCommit {
            commit: "1234567890abcdef".to_string(),
            summary: "1234567 second".to_string(),
        });
        let cherry_pick = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::CherryPickCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            },
        );
        let revert = confirmation_copy(&super_lazygit_core::ConfirmableOperation::RevertCommit {
            commit: "1234567890abcdef".to_string(),
            summary: "1234567 second".to_string(),
        });

        assert!(amend.contains("older-commit amend"));
        assert!(fixup.contains("fixup commit"));
        assert!(cherry_pick.contains("Cherry-pick 1234567 second"));
        assert!(revert.contains("Revert 1234567 second"));
    }

    #[test]
    fn repo_mode_routes_subviews_and_focus() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "2".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result.state.repo_mode.expect("repo mode").active_subview,
            RepoSubview::Branches
        );
    }

    #[test]
    fn repo_mode_commit_detail_routes_interactive_rebase_start() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "i".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::StartInteractiveRebase {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                }
            )
        );
    }

    #[test]
    fn repo_mode_commit_detail_routes_reset_shortcuts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };

        let mut soft_app = TuiApp::new(state.clone(), AppConfig::default());
        let soft = soft_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "S".to_string(),
        })));
        assert_eq!(
            soft.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::ResetToCommit {
                mode: super_lazygit_core::ResetMode::Soft,
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut mixed_app = TuiApp::new(state.clone(), AppConfig::default());
        let mixed = mixed_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "M".to_string(),
        })));
        assert_eq!(
            mixed
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::ResetToCommit {
                mode: super_lazygit_core::ResetMode::Mixed,
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut hard_app = TuiApp::new(state, AppConfig::default());
        let hard = hard_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "H".to_string(),
        })));
        assert_eq!(
            hard.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::ResetToCommit {
                mode: super_lazygit_core::ResetMode::Hard,
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );
    }

    #[test]
    fn repo_mode_commit_detail_routes_history_shortcuts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };

        let mut cherry_pick_app = TuiApp::new(state.clone(), AppConfig::default());
        let cherry_pick =
            cherry_pick_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "C".to_string(),
            })));
        assert_eq!(
            cherry_pick
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::CherryPickCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut amend_app = TuiApp::new(state.clone(), AppConfig::default());
        let amend = amend_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "A".to_string(),
        })));
        assert_eq!(
            amend
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::AmendCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut fixup_app = TuiApp::new(state.clone(), AppConfig::default());
        let fixup = fixup_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "F".to_string(),
        })));
        assert_eq!(
            fixup
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::FixupCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut reword_app = TuiApp::new(state.clone(), AppConfig::default());
        let reword = reword_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "R".to_string(),
        })));
        assert!(reword.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::RewordCommitWithEditor { commit },
                ..
            }) if commit == "1234567890abcdef"
        )));

        let mut revert_app = TuiApp::new(state, AppConfig::default());
        let revert = revert_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "V".to_string(),
        })));
        assert_eq!(
            revert
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::RevertCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );
    }

    #[test]
    fn repo_mode_rebase_detail_routes_continue_skip_and_abort() {
        let mut detail = sample_repo_detail();
        detail.merge_state = super_lazygit_core::MergeState::RebaseInProgress;
        detail.rebase_state = Some(RebaseState {
            kind: RebaseKind::Interactive,
            step: 1,
            total: 2,
            head_name: Some("main".to_string()),
            onto: Some("1234567".to_string()),
            current_commit: Some("1234567890abcdef".to_string()),
            current_summary: Some("second".to_string()),
            todo_preview: vec!["pick abcdef1 add lib".to_string()],
        });
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Rebase,
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let continue_result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "c".to_string(),
        })));
        assert!(continue_result.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::ContinueRebase,
                ..
            })
        )));

        let mut skip_app = TuiApp::new(state.clone(), AppConfig::default());
        let skip_result = skip_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "s".to_string(),
        })));
        assert_eq!(skip_result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            skip_result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::SkipRebase)
        );

        let mut abort_app = TuiApp::new(state, AppConfig::default());
        let abort_result = abort_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "A".to_string(),
        })));
        assert_eq!(abort_result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            abort_result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::AbortRebase)
        );
    }

    #[test]
    fn repo_mode_branch_detail_routes_selection_and_prompts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Branches,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );

        let rename = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "R".to_string(),
        })));
        assert_eq!(rename.state.focused_pane, PaneId::Modal);
        assert_eq!(
            rename
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::RenameBranch {
                    current_name: "feature".to_string()
                },
                "feature"
            ))
        );
    }

    #[test]
    fn repo_mode_cycles_focus_across_three_panes() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let staged = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "tab".to_string(),
        })));
        assert_eq!(staged.state.focused_pane, PaneId::RepoStaged);

        let detail = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "tab".to_string(),
        })));
        assert_eq!(detail.state.focused_pane, PaneId::RepoDetail);

        let back = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "shift+tab".to_string(),
        })));
        assert_eq!(back.state.focused_pane, PaneId::RepoStaged);
    }

    #[test]
    fn repo_mode_status_shortcuts_route_discard_and_nuke() {
        let status_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut status_app = TuiApp::new(status_state, AppConfig::default());

        let discard = status_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "D".to_string(),
        })));
        assert_eq!(
            discard
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::DiscardFile {
                path: std::path::PathBuf::from("src/lib.rs"),
            })
        );

        let detail_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut detail_app = TuiApp::new(detail_state, AppConfig::default());

        let nuke = detail_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "X".to_string(),
        })));
        assert_eq!(
            nuke.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::NukeWorkingTree)
        );
    }

    #[test]
    fn repo_mode_routes_uppercase_push() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "P".to_string(),
        })));

        assert!(result
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::ScheduleRender)));
        assert_eq!(
            result
                .state
                .modal_stack
                .last()
                .map(|modal| (&modal.kind, modal.title.as_str())),
            Some((&ModalKind::Confirm, "Confirm push"))
        );
    }

    #[test]
    fn repo_mode_routes_uppercase_stash_to_options_menu() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "S".to_string(),
        })));

        assert_eq!(
            result
                .state
                .modal_stack
                .last()
                .map(|modal| (&modal.kind, modal.title.as_str())),
            Some((&ModalKind::Menu, "Stash options"))
        );
        assert_eq!(
            result.state.pending_menu.as_ref().map(|menu| (
                menu.operation,
                menu.selected_index,
                menu.return_focus
            )),
            Some((
                super_lazygit_core::MenuOperation::StashOptions,
                0,
                PaneId::RepoStaged
            ))
        );
    }

    #[test]
    fn confirm_modal_routes_enter_to_transport_job() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                ModalKind::Confirm,
                "Confirm fetch",
            )],
            pending_confirmation: Some(super_lazygit_core::PendingConfirmation {
                repo_id: RepoId::new("repo-1"),
                operation: super_lazygit_core::ConfirmableOperation::Fetch,
            }),
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::FetchSelectedRepo,
                ..
            })
        )));
    }

    #[test]
    fn input_prompt_routes_text_and_submit_branch_job() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                ModalKind::InputPrompt,
                "Create branch",
            )],
            pending_input_prompt: Some(super_lazygit_core::PendingInputPrompt {
                repo_id: RepoId::new("repo-1"),
                operation: super_lazygit_core::InputPromptOperation::CreateBranch,
                value: "feature".to_string(),
                return_focus: PaneId::RepoDetail,
            }),
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let typed = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "/".to_string(),
        })));
        assert_eq!(
            typed
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.value.as_str()),
            Some("feature/")
        );

        let submitted = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(submitted.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CreateBranch { branch_name },
                ..
            }) if branch_name == "feature/"
        )));
    }

    #[test]
    fn menu_modal_routes_navigation_and_submit_keep_index_stash_prompt() {
        let state = AppState {
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                ModalKind::Menu,
                "Stash options",
            )],
            pending_menu: Some(super_lazygit_core::PendingMenu {
                repo_id: RepoId::new("repo-1"),
                operation: super_lazygit_core::MenuOperation::StashOptions,
                selected_index: 0,
                return_focus: PaneId::RepoStaged,
            }),
            mode: AppMode::Repository,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let moved = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            moved
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.selected_index),
            Some(1)
        );

        let submitted = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            submitted
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.return_focus)),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateStash {
                    mode: super_lazygit_core::StashMode::KeepIndex,
                },
                PaneId::RepoStaged,
            ))
        );
    }

    #[test]
    fn input_prompt_copy_distinguishes_keep_index_from_unstaged_stash() {
        let keep_index =
            input_prompt_copy(&super_lazygit_core::InputPromptOperation::CreateStash {
                mode: super_lazygit_core::StashMode::KeepIndex,
            });
        let unstaged = input_prompt_copy(&super_lazygit_core::InputPromptOperation::CreateStash {
            mode: super_lazygit_core::StashMode::Unstaged,
        });

        assert!(keep_index.contains("keep staged changes in place"));
        assert!(unstaged.contains("stash only tracked unstaged changes"));
        assert!(unstaged.contains("restored after the stash is created"));
    }

    #[test]
    fn input_prompt_copy_describes_stash_rename() {
        let rename = input_prompt_copy(&super_lazygit_core::InputPromptOperation::RenameStash {
            stash_ref: "stash@{1}".to_string(),
            current_name: "prior experiment".to_string(),
        });

        assert!(rename.contains("new message for stash@{1}"));
        assert!(rename.contains("default stash message"));
    }

    #[test]
    fn repo_mode_commit_detail_routes_jk_between_commits() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
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

        let up = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "k".to_string(),
        })));
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index),
            Some(0)
        );
    }

    #[test]
    fn repo_mode_commit_detail_routes_compare_shortcuts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let base = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "v".to_string(),
        })));
        assert_eq!(
            base.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_base.clone()),
            Some(ComparisonTarget::Commit("abcdef1234567890".to_string()))
        );

        let _ = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        let compare = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "v".to_string(),
        })));
        assert_eq!(
            compare
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Compare)
        );
        assert!(compare.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::LoadRepoDiff {
                comparison_target: Some(ComparisonTarget::Commit(base)),
                compare_with: Some(ComparisonTarget::Commit(target)),
                diff_presentation: DiffPresentation::Comparison,
                ..
            } if base == "abcdef1234567890" && target == "1234567890abcdef"
        )));
    }

    #[test]
    fn repo_mode_compare_detail_routes_clear_shortcut() {
        let mut detail = sample_repo_detail();
        detail.diff.presentation = DiffPresentation::Comparison;
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Compare,
                comparison_base: Some(ComparisonTarget::Commit("abcdef1234567890".to_string())),
                comparison_target: Some(ComparisonTarget::Commit("1234567890abcdef".to_string())),
                comparison_source: Some(RepoSubview::Commits),
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let cleared = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "x".to_string(),
        })));
        assert_eq!(
            cleared
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            cleared
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.comparison_base.clone()),
            None
        );
        assert!(cleared.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::LoadRepoDetail {
                diff_presentation: DiffPresentation::Unstaged,
                ..
            }
        )));
    }

    #[test]
    fn repo_mode_stash_detail_routes_selection_rename_apply_pop_and_drop() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index),
            Some(1)
        );

        let enter = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(enter.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::ApplyStash { stash_ref },
                ..
            }) if stash_ref == "stash@{1}"
        )));

        let mut rename_app = TuiApp::new(state.clone(), AppConfig::default());
        let down = rename_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index),
            Some(1)
        );

        let rename = rename_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "r".to_string(),
        })));
        assert_eq!(rename.state.focused_pane, PaneId::Modal);
        assert_eq!(
            rename
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::RenameStash {
                    stash_ref: "stash@{1}".to_string(),
                    current_name: "prior experiment".to_string(),
                },
                "prior experiment"
            ))
        );

        let pop = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(pop.state.focused_pane, PaneId::Modal);
        assert_eq!(
            pop.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::PopStash {
                stash_ref: "stash@{1}".to_string(),
            })
        );

        let mut drop_app = TuiApp::new(state, AppConfig::default());
        let down = drop_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index),
            Some(1)
        );

        let drop = drop_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(drop.state.focused_pane, PaneId::Modal);
        assert_eq!(
            drop.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::DropStash {
                stash_ref: "stash@{1}".to_string(),
            })
        );
    }

    #[test]
    fn repo_mode_reflog_detail_routes_selection() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Reflog,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.reflog_view.selected_index),
            Some(1)
        );

        let up = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "k".to_string(),
        })));
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.reflog_view.selected_index),
            Some(0)
        );
    }

    #[test]
    fn repo_mode_reflog_detail_routes_restore_confirmation() {
        let mut detail = sample_repo_detail();
        detail.file_tree.clear();
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Reflog,
                detail: Some(detail),
                reflog_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let restore = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "u".to_string(),
        })));

        assert_eq!(restore.state.focused_pane, PaneId::Modal);
        assert_eq!(
            restore
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::RestoreReflogEntry {
                    target: "HEAD@{1}".to_string(),
                    summary: "HEAD@{1}: commit: add repo-mode stash flows".to_string(),
                }
            )
        );
    }

    #[test]
    fn repo_mode_worktree_detail_routes_selection_create_and_remove() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Worktrees,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.worktree_view.selected_index),
            Some(1)
        );

        let mut create_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "c".to_string(),
        })));
        assert_eq!(create.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::CreateWorktree)
        );

        let mut remove_app = TuiApp::new(down.state, AppConfig::default());
        let remove = remove_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(remove.state.focused_pane, PaneId::Modal);
        assert_eq!(
            remove
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::RemoveWorktree {
                path: PathBuf::from("/tmp/repo-1-feature"),
            })
        );
    }

    #[test]
    fn repo_mode_unstaged_pane_routes_status_navigation_and_stage_action() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                status_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.status_view.selected_index),
            Some(1)
        );

        let enter = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(enter.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::StageFile { .. },
                ..
            })
        )));

        let mut space_app = TuiApp::new(state, AppConfig::default());
        let space = space_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(space.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::StageFile { .. },
                ..
            })
        )));
    }

    #[test]
    fn repo_mode_staged_pane_routes_unstage_action() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                staged_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let enter = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(enter.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::UnstageFile { .. },
                ..
            })
        )));

        let mut space_app = TuiApp::new(state, AppConfig::default());
        let space = space_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(space.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::UnstageFile { .. },
                ..
            })
        )));
    }

    #[test]
    fn repo_mode_status_panes_route_commit_shortcuts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let commit = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "c".to_string(),
        })));
        assert_eq!(
            commit
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.mode),
            Some(CommitBoxMode::Commit)
        );
        assert_eq!(
            commit
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.focused),
            Some(true)
        );

        let mut editor_app = TuiApp::new(state.clone(), AppConfig::default());
        let editor_commit = editor_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "C".to_string(),
        })));
        assert!(editor_commit.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CommitStagedWithEditor,
                ..
            })
        )));

        let mut staged_stash_app = TuiApp::new(state.clone(), AppConfig::default());
        let staged_stash =
            staged_stash_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "s".to_string(),
            })));
        assert_eq!(
            staged_stash
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.return_focus)),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateStash {
                    mode: super_lazygit_core::StashMode::Tracked,
                },
                PaneId::RepoStaged
            ))
        );

        let mut staged_stash_options_app = TuiApp::new(state.clone(), AppConfig::default());
        let staged_stash_options =
            staged_stash_options_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "S".to_string(),
            })));
        assert_eq!(
            staged_stash_options
                .state
                .pending_menu
                .as_ref()
                .map(|menu| (menu.operation, menu.return_focus)),
            Some((
                super_lazygit_core::MenuOperation::StashOptions,
                PaneId::RepoStaged,
            ))
        );

        let mut unstaged_state = state.clone();
        unstaged_state.focused_pane = PaneId::RepoUnstaged;
        let mut unstaged_stash_app = TuiApp::new(unstaged_state, AppConfig::default());
        let unstaged_stash =
            unstaged_stash_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "s".to_string(),
            })));
        assert_eq!(
            unstaged_stash
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.return_focus)),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateStash {
                    mode: super_lazygit_core::StashMode::Tracked,
                },
                PaneId::RepoUnstaged
            ))
        );

        let mut amend_app = TuiApp::new(state.clone(), AppConfig::default());
        let amend = amend_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "A".to_string(),
        })));
        assert_eq!(
            amend
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.mode),
            Some(CommitBoxMode::Amend)
        );

        let mut no_verify_app = TuiApp::new(state, AppConfig::default());
        let no_verify = no_verify_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "w".to_string(),
        })));
        assert_eq!(
            no_verify
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.mode),
            Some(CommitBoxMode::CommitNoVerify)
        );
    }

    #[test]
    fn repo_mode_commit_pane_rewords_selected_commit_with_editor() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Commits,
                detail: Some(super_lazygit_core::RepoDetail {
                    commits: vec![
                        super_lazygit_core::CommitItem {
                            oid: "head".to_string(),
                            short_oid: "head".to_string(),
                            summary: "HEAD".to_string(),
                            ..Default::default()
                        },
                        super_lazygit_core::CommitItem {
                            oid: "older".to_string(),
                            short_oid: "old1234".to_string(),
                            summary: "older commit".to_string(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "R".to_string(),
        })));

        assert!(result.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::RewordCommitWithEditor { commit },
                ..
            }) if commit == "older"
        )));
    }

    #[test]
    fn repo_mode_commit_box_routes_text_input_and_submit() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let _ = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "c".to_string(),
        })));
        let pasted = app.dispatch(Event::Input(InputEvent::Paste("feat".to_string())));
        assert_eq!(
            pasted
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .map(|detail| detail.commit_input.as_str()),
            Some("feat")
        );

        let _ = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: " ".to_string(),
        })));
        let _ = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "x".to_string(),
        })));
        let _ = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "backspace".to_string(),
        })));

        let submit = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert!(submit.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CommitStaged { message },
                ..
            }) if message == "feat"
        )));
    }

    #[test]
    fn repo_mode_commit_box_escape_cancels_without_leaving_repo_mode() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                commit_box: super_lazygit_core::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::Commit,
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "esc".to_string(),
        })));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoStaged);
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_box.focused),
            Some(false)
        );
    }

    #[test]
    fn repo_mode_status_detail_scrolls_diff_and_preserves_orientation() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 8);

        let down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .and_then(|detail| detail.diff.selected_hunk),
            Some(0)
        );
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(3)
        );

        let repo_mode = app.state().repo_mode.as_ref().expect("repo mode");
        let visible_lines = repo_diff_lines(
            Some(repo_mode),
            None,
            repo_mode.diff_scroll,
            6,
            Theme::from_config(&AppConfig::default()),
        );
        let rendered_lines = visible_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(rendered_lines.contains(&"Path: src/lib.rs (unstaged)".to_string()));
        assert!(rendered_lines
            .iter()
            .any(|line| line.contains("Hunks: 1  Selected: 1/1")));
        assert!(rendered_lines
            .iter()
            .any(|line| line.contains("Line select: J/K cursor")));
        assert!(rendered_lines.contains(&"@@ -1 +1 @@".to_string()));

        let scroll_down = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "down".to_string(),
        })));
        assert_eq!(
            scroll_down
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(4)
        );

        let up = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "up".to_string(),
        })));
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(3)
        );
    }

    #[test]
    fn repo_mode_status_detail_routes_line_selection_keys() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                diff_line_cursor: Some(3),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let anchored = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "v".to_string(),
        })));
        assert_eq!(
            anchored
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_line_anchor),
            Some(Some(3))
        );

        let moved = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "J".to_string(),
        })));
        assert_eq!(
            moved
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_line_cursor),
            Some(Some(4))
        );

        let apply = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "L".to_string(),
        })));
        assert!(apply.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunPatchSelection(super_lazygit_core::PatchSelectionJob {
                hunks,
                ..
            }) if hunks
                == &vec![super_lazygit_core::SelectedHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                }]
        )));
    }

    #[test]
    fn repo_mode_status_detail_routes_space_to_toggle_selected_hunk() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let apply = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(apply.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunPatchSelection(super_lazygit_core::PatchSelectionJob {
                hunks,
                ..
            }) if hunks
                == &vec![super_lazygit_core::SelectedHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                }]
        )));
    }

    #[test]
    fn diagnostics_snapshot_includes_render_samples() {
        let mut app = TuiApp::new(AppState::default(), AppConfig::default());

        let _ = app.render();

        assert_eq!(app.diagnostics_snapshot().renders.len(), 1);
    }

    #[test]
    fn render_repo_shell_shows_three_pane_scaffold() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                staged_count: 2,
                unstaged_count: 3,
                untracked_count: 1,
                ahead_count: 1,
                behind_count: 0,
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Working tree"));
        assert!(rendered.contains("Staged changes"));
        assert!(rendered.contains("Detail: Status"));
        assert!(rendered.contains("M src/lib.rs"));
        assert!(rendered.contains("? README.md"));
        assert!(rendered.contains("A Cargo.toml"));
        assert!(rendered.contains("Path: src/lib.rs"));
        assert!(rendered.contains("Hunks: 1"));
        assert!(rendered.contains("Line select: J/K cursor"));
        assert!(rendered.contains("@@ -1 +1 @@"));
        assert!(rendered.contains("+new line"));
        assert!(rendered.contains("Repository shell"));
        assert!(rendered.contains("Watch: unknown"));
    }

    #[test]
    fn render_repo_shell_shows_commit_history_preview() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                comparison_base: Some(ComparisonTarget::Commit("abcdef1234567890".to_string())),
                comparison_source: Some(RepoSubview::Commits),
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(160, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Commits"));
        assert!(rendered.contains("Selected 1/2"));
        assert!(rendered.contains("Compare base: abcdef1234567890"));
        assert!(rendered.contains("State: idle"));
        assert!(rendered.contains("> abcdef1 add lib"));
        assert!(rendered.contains("A amend"));
        assert!(rendered.contains("F fixup"));
        assert!(rendered.contains("R reword"));
        assert!(rendered.contains("C cherry-pick"));
        assert!(rendered.contains("V revert"));
        assert!(rendered.contains("S soft"));
        assert!(rendered.contains("A src/lib.rs"));
    }

    #[test]
    fn render_repo_shell_shows_commit_history_operation_state() {
        let mut detail = sample_repo_detail();
        detail.merge_state = super_lazygit_core::MergeState::CherryPickInProgress;
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("State: cherry-pick in progress"));
        assert!(rendered.contains("C cherry-pick"));
        assert!(rendered.contains("V revert"));
    }

    #[test]
    fn render_repo_shell_shows_rebase_view() {
        let mut detail = sample_repo_detail();
        detail.merge_state = super_lazygit_core::MergeState::RebaseInProgress;
        detail.rebase_state = Some(RebaseState {
            kind: RebaseKind::Interactive,
            step: 1,
            total: 2,
            head_name: Some("main".to_string()),
            onto: Some("1234567".to_string()),
            current_commit: Some("1234567890abcdef".to_string()),
            current_summary: Some("second".to_string()),
            todo_preview: vec!["pick abcdef1 add lib".to_string()],
        });
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Rebase,
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Rebase"));
        assert!(rendered.contains("Interactive rebase control"));
        assert!(rendered.contains("Mode: interactive"));
        assert!(rendered.contains("c continue  s skip  A abort"));
        assert!(rendered.contains("pick abcdef1 add lib"));
    }

    #[test]
    fn render_repo_shell_shows_compare_view() {
        let mut detail = sample_repo_detail();
        detail.diff = DiffModel {
            selected_path: None,
            presentation: DiffPresentation::Comparison,
            lines: vec![
                DiffLine {
                    kind: DiffLineKind::Meta,
                    content: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                },
                DiffLine {
                    kind: DiffLineKind::Addition,
                    content: "+pub fn answer() -> u32 {".to_string(),
                },
            ],
            hunks: Vec::new(),
            selected_hunk: None,
            hunk_count: 0,
        };
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Compare,
                comparison_base: Some(ComparisonTarget::Commit("abcdef1234567890".to_string())),
                comparison_target: Some(ComparisonTarget::Commit("1234567890abcdef".to_string())),
                comparison_source: Some(RepoSubview::Commits),
                detail: Some(detail),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Compare"));
        assert!(rendered.contains("Comparing abcdef1234567890 -> 1234567890abcdef"));
        assert!(rendered.contains("x clears compare and returns to history."));
        assert!(rendered.contains("+pub fn answer() -> u32 {"));
    }

    #[test]
    fn render_repo_shell_shows_branch_management_details() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Branches,
                detail: Some(sample_repo_detail()),
                branches_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Branches"));
        assert!(rendered.contains("Selected: feature"));
        assert!(rendered.contains("Enter checks out."));
        assert!(rendered.contains("* main"));
        assert!(rendered.contains("feature"));
    }

    #[test]
    fn render_repo_shell_shows_stash_management_details() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Stash,
                detail: Some(sample_repo_detail()),
                stash_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Stash"));
        assert!(rendered.contains("Selected: stash@{1}"));
        assert!(rendered.contains("Enter applies. g pops. d drops."));
        assert!(rendered.contains("stash@{0}: WIP on main: fixture stash"));
        assert!(rendered.contains("stash@{1}: On feature: prior experiment"));
    }

    #[test]
    fn render_repo_shell_shows_reflog_details() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Reflog,
                detail: Some(sample_repo_detail()),
                reflog_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Reflog"));
        assert!(rendered.contains("Selected 2/2"));
        assert!(rendered.contains("Use j/k to inspect recent HEAD and ref movement."));
        assert!(rendered.contains("u restore HEAD to the selected entry"));
        assert!(rendered.contains("Limits: no working tree undo"));
        assert!(rendered.contains("HEAD@{0}: checkout: moving from feature to main"));
        assert!(rendered.contains("HEAD@{1}: commit: add repo-mode stash flows"));
    }

    #[test]
    fn restore_reflog_confirmation_copy_describes_hard_reset_limits() {
        let copy = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::RestoreReflogEntry {
                target: "HEAD@{1}".to_string(),
                summary: "HEAD@{1}: commit: add repo-mode stash flows".to_string(),
            },
        );

        assert!(copy.contains("git reset --hard HEAD@{1}"));
        assert!(copy.contains("Working tree edits and untracked files are not undone"));
    }

    #[test]
    fn pop_stash_confirmation_copy_mentions_apply_and_drop() {
        let copy = confirmation_copy(&super_lazygit_core::ConfirmableOperation::PopStash {
            stash_ref: "stash@{1}".to_string(),
        });

        assert!(copy.contains("Pop stash@{1}?"));
        assert!(copy.contains("applies it"));
        assert!(copy.contains("removes it from the stash list"));
    }

    #[test]
    fn render_repo_shell_shows_worktree_details() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Worktrees,
                detail: Some(sample_repo_detail()),
                worktree_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Worktrees"));
        assert!(rendered.contains("Selected: /tmp/repo-1-feature"));
        assert!(rendered.contains("Branch: feature"));
        assert!(rendered.contains("c creates. d removes the selected linked worktree."));
        assert!(rendered.contains("/tmp/repo-1  [main]"));
        assert!(rendered.contains("/tmp/repo-1-feature  [feature]"));
    }

    #[test]
    fn render_repo_shell_shows_commit_box_overlay() {
        let mut detail = sample_repo_detail();
        detail.commit_input = "feat: land repo commit box".to_string();
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                detail: Some(detail),
                commit_box: super_lazygit_core::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::Commit,
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                staged_count: 2,
                unstaged_count: 3,
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Staged changes · Commit"));
        assert!(rendered.contains("Commit box"));
        assert!(rendered.contains("Type a new commit message"));
    }

    #[test]
    fn commit_box_lines_show_message_cursor() {
        let mut detail = sample_repo_detail();
        detail.commit_input = "feat: land repo commit box".to_string();

        let rendered = commit_box_lines(
            Some(&detail),
            CommitBoxMode::Commit,
            Theme::from_config(&AppConfig::default()),
        )
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

        assert!(rendered
            .iter()
            .any(|line| line.contains("Message: feat: land repo commit box_")));
    }

    #[test]
    fn render_repo_shell_shows_no_verify_commit_box_overlay() {
        let mut detail = sample_repo_detail();
        detail.commit_input = "feat: bypass hooks".to_string();
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoStaged,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Status,
                detail: Some(detail),
                commit_box: super_lazygit_core::CommitBoxState {
                    focused: true,
                    mode: CommitBoxMode::CommitNoVerify,
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("repo-1"),
            RepoSummary {
                repo_id: RepoId::new("repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                staged_count: 2,
                unstaged_count: 3,
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 20);

        let rendered = app.render_to_string();
        let overlay_lines = commit_box_lines(
            app.state()
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.detail.as_ref()),
            CommitBoxMode::CommitNoVerify,
            Theme::from_config(&AppConfig::default()),
        )
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

        assert!(rendered.contains("Staged changes · Commit (No Verify)"));
        assert!(rendered.contains("Commit without hooks"));
        assert!(overlay_lines
            .iter()
            .any(|line| line.contains("skip pre-commit hooks")));
    }
}
