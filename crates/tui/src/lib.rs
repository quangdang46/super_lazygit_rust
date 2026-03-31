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
    reduce, workspace_attention_score, Action, AppMode, AppState, CommitBoxMode, CommitFilesMode,
    CommitHistoryMode, CommitSubviewMode, Diagnostics, DiagnosticsSnapshot, DiffLineKind,
    DiffPresentation, Event, InputEvent, InputPromptOperation, KeyPress, PaneId, ReduceResult,
    RepoDetail, RepoId, RepoModeState, RepoSubview, RepoSummary, ScreenMode, StashSubviewMode,
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
                Constraint::Length(status_bar_height(&self.state)),
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

    fn repo_list_page_size(&self) -> usize {
        self.viewport.height.saturating_sub(8).max(1) as usize
    }

    fn repo_detail_supports_shared_list_navigation(&self) -> bool {
        self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
            !matches!(
                repo_mode.active_subview,
                RepoSubview::Status | RepoSubview::Compare | RepoSubview::Rebase
            )
        })
    }

    fn repo_focus_supports_shared_list_navigation(&self) -> bool {
        matches!(
            self.state.focused_pane,
            PaneId::RepoUnstaged | PaneId::RepoStaged
        ) || (self.state.focused_pane == PaneId::RepoDetail
            && self.repo_detail_supports_shared_list_navigation())
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
                } else if self.repo_subview_filter_focused() && !text.is_empty() {
                    let result = reduce(
                        self.state.clone(),
                        Event::Action(Action::AppendRepoSubviewFilter { text }),
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
            return Some(self.repo_escape_action());
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
        if self.repo_subview_filter_focused() {
            if self.binding_matches_action("cancel_repo_subview_filter", raw, normalized, &["esc"])
            {
                return Some(Action::CancelRepoSubviewFilter);
            }

            if self.binding_matches_action("blur_repo_subview_filter", raw, normalized, &["enter"])
            {
                return Some(Action::BlurRepoSubviewFilter);
            }

            if self.binding_matches_action(
                "backspace_repo_subview_filter",
                raw,
                normalized,
                &["backspace"],
            ) {
                return Some(Action::BackspaceRepoSubviewFilter);
            }

            return match raw {
                "space" | " " => Some(Action::AppendRepoSubviewFilter {
                    text: " ".to_string(),
                }),
                _ if raw.chars().count() == 1 => Some(Action::AppendRepoSubviewFilter {
                    text: raw.to_string(),
                }),
                _ => None,
            };
        }

        if self.binding_matches_action("open_recent_repos", raw, normalized, &["ctrl+r"]) {
            if self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                repo_mode.active_subview == RepoSubview::Commits
                    && repo_mode.commit_subview_mode == CommitSubviewMode::History
                    && repo_mode.copied_commit.is_some()
            }) {
                return Some(Action::ClearCopiedCommitSelection);
            }
            return Some(Action::OpenRecentRepos);
        }

        if self.binding_matches_action("open_command_log", raw, normalized, &["@"]) {
            return Some(Action::OpenCommandLog);
        }

        if self.binding_matches_action("next_screen_mode", raw, normalized, &["+"]) {
            return Some(Action::NextScreenMode);
        }

        if self.binding_matches_action("previous_screen_mode", raw, normalized, &["_"]) {
            return Some(Action::PreviousScreenMode);
        }

        if self.binding_matches_action("open_shell_command_prompt", raw, normalized, &[":"]) {
            return Some(Action::OpenInputPrompt {
                operation: InputPromptOperation::ShellCommand,
            });
        }

        if self.repo_focus_supports_shared_list_navigation() {
            if self.binding_matches_action(
                "page_down_repo_list",
                raw,
                normalized,
                &[".", "pagedown"],
            ) {
                return Some(Action::PageDownRepoList {
                    page_size: self.repo_list_page_size(),
                });
            }

            if self.binding_matches_action("page_up_repo_list", raw, normalized, &[",", "pageup"]) {
                return Some(Action::PageUpRepoList {
                    page_size: self.repo_list_page_size(),
                });
            }

            if self.binding_matches_action(
                "select_first_repo_list_entry",
                raw,
                normalized,
                &["<", "home"],
            ) {
                return Some(Action::SelectFirstRepoListEntry);
            }

            if self.binding_matches_action(
                "select_last_repo_list_entry",
                raw,
                normalized,
                &[">", "end"],
            ) {
                return Some(Action::SelectLastRepoListEntry);
            }
        }

        if self.binding_matches_action("select_previous_repo_subview", raw, normalized, &["["]) {
            return Some(Action::SelectPreviousRepoSubview);
        }

        if self.binding_matches_action("select_next_repo_subview", raw, normalized, &["]"]) {
            return Some(Action::SelectNextRepoSubview);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                repo_mode.subview_filter(repo_mode.active_subview).is_some()
            })
            && self.binding_matches_action("open_filter_options", raw, normalized, &["ctrl+s"])
        {
            return Some(Action::OpenFilterOptions);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(
                    repo_mode.active_subview,
                    RepoSubview::Status
                        | RepoSubview::Branches
                        | RepoSubview::Commits
                        | RepoSubview::Compare
                )
            })
            && self.binding_matches_action("open_diff_options", raw, normalized, &["W", "ctrl+e"])
        {
            return Some(Action::OpenDiffOptions);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(
                    repo_mode.active_subview,
                    RepoSubview::Status | RepoSubview::Commits | RepoSubview::Compare
                )
            })
        {
            if self.binding_matches_action(
                "toggle_whitespace_in_diff",
                raw,
                normalized,
                &["ctrl+w"],
            ) {
                return Some(Action::ToggleWhitespaceInDiff);
            }
            if self.binding_matches_action("increase_diff_context", raw, normalized, &["}"]) {
                return Some(Action::IncreaseDiffContext);
            }
            if self.binding_matches_action("decrease_diff_context", raw, normalized, &["{"]) {
                return Some(Action::DecreaseDiffContext);
            }
            if self.binding_matches_action(
                "increase_rename_similarity_threshold",
                raw,
                normalized,
                &[")"],
            ) {
                return Some(Action::IncreaseRenameSimilarityThreshold);
            }
            if self.binding_matches_action(
                "decrease_rename_similarity_threshold",
                raw,
                normalized,
                &["("],
            ) {
                return Some(Action::DecreaseRenameSimilarityThreshold);
            }
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(
                    repo_mode.active_subview,
                    RepoSubview::Commits | RepoSubview::Rebase
                )
            })
            && self.binding_matches_action("open_merge_rebase_options", raw, normalized, &["m"])
        {
            return Some(Action::OpenMergeRebaseOptions);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self
                .state
                .repo_mode
                .as_ref()
                .is_some_and(|repo_mode| repo_mode.active_subview == RepoSubview::Status)
            && self.binding_matches_action("open_patch_options", raw, normalized, &["ctrl+p"])
        {
            return Some(Action::OpenPatchOptions);
        }

        if matches!(self.state.focused_pane, PaneId::RepoDetail)
            && self.binding_matches_action("push_selected_tag", raw, normalized, &["P"])
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(repo_mode.active_subview, RepoSubview::Tags)
                    && selected_tag(
                        repo_mode.detail.as_ref(),
                        repo_mode.tags_view.selected_index,
                    )
                    .is_some()
            })
        {
            return Some(Action::PushSelectedTag);
        }

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

                if self.binding_matches_action(
                    "discard_selected_file",
                    raw,
                    normalized,
                    &["d", "D"],
                ) {
                    return Some(Action::DiscardSelectedFile);
                }

                if self.binding_matches_action("open_in_editor", raw, normalized, &["e"]) {
                    return Some(Action::OpenInEditor);
                }

                if self.binding_matches_action(
                    "copy_selected_status_path",
                    raw,
                    normalized,
                    &["y", "ctrl+o"],
                ) {
                    return Some(Action::CopySelectedStatusPath);
                }

                if self.binding_matches_action(
                    "open_selected_status_path_in_default_app",
                    raw,
                    normalized,
                    &["o"],
                ) {
                    return Some(Action::OpenSelectedStatusPathInDefaultApp);
                }

                if self.binding_matches_action(
                    "open_selected_status_path_in_external_difftool",
                    raw,
                    normalized,
                    &["ctrl+t"],
                ) {
                    return Some(Action::OpenSelectedStatusPathInExternalDiffTool);
                }

                if self.binding_matches_action("open_stash_options", raw, normalized, &["S"]) {
                    return Some(Action::OpenStashOptions);
                }

                if self.binding_matches_action("stash_all_changes", raw, normalized, &["s"]) {
                    return Some(Action::StashAllChanges);
                }

                if self.binding_matches_action("stage_selection", raw, normalized, &["a"]) {
                    return Some(if self.state.focused_pane == PaneId::RepoUnstaged {
                        Action::StageSelection
                    } else {
                        Action::UnstageSelection
                    });
                }

                if self.binding_matches_action(
                    "cycle_status_filter_mode",
                    raw,
                    normalized,
                    &["ctrl+b"],
                ) {
                    return Some(Action::CycleStatusFilterMode);
                }

                if self.binding_matches_action("toggle_status_tree", raw, normalized, &["`"]) {
                    return Some(Action::ToggleStatusTree);
                }

                if self.binding_matches_action("collapse_status_entry", raw, normalized, &["-"]) {
                    return Some(Action::CollapseStatusEntry);
                }

                if self.binding_matches_action("expand_status_entry", raw, normalized, &["="]) {
                    return Some(Action::ExpandStatusEntry);
                }

                if self.binding_matches_action("open_ignore_options", raw, normalized, &["i"]) {
                    return Some(Action::OpenIgnoreOptions);
                }

                if self.binding_matches_action("open_status_reset_options", raw, normalized, &["g"])
                {
                    return Some(Action::OpenStatusResetOptions);
                }

                if self.binding_matches_action("open_merge_rebase_options", raw, normalized, &["M"])
                {
                    return Some(Action::OpenMergeRebaseOptions);
                }

                if self.binding_matches_action("focus_repo_detail_pane", raw, normalized, &["0"]) {
                    return Some(Action::SetFocusedPane(PaneId::RepoDetail));
                }

                if self.binding_matches_action("focus_repo_subview_filter", raw, normalized, &["/"])
                {
                    return Some(Action::FocusRepoSubviewFilter);
                }

                if self.binding_matches_action(
                    "open_selected_status_entry",
                    raw,
                    normalized,
                    &["enter"],
                ) {
                    return Some(Action::OpenSelectedStatusEntry);
                }

                if self.state.focused_pane == PaneId::RepoUnstaged
                    && self.binding_matches_action(
                        "stage_selected_file",
                        raw,
                        normalized,
                        &["space"],
                    )
                {
                    return Some(Action::StageSelectedFile);
                }

                if self.state.focused_pane == PaneId::RepoStaged
                    && self.binding_matches_action(
                        "unstage_selected_file",
                        raw,
                        normalized,
                        &["space"],
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
                        | RepoSubview::Remotes
                        | RepoSubview::RemoteBranches
                        | RepoSubview::Tags
                        | RepoSubview::Commits
                        | RepoSubview::Compare
                        | RepoSubview::Rebase
                        | RepoSubview::Stash
                        | RepoSubview::Reflog
                        | RepoSubview::Worktrees
                        | RepoSubview::Submodules
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
                            "open_selected_branch_commits",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::OpenSelectedBranchCommits);
                        }

                        if self.binding_matches_action(
                            "checkout_selected_branch",
                            raw,
                            normalized,
                            &["space"],
                        ) {
                            return Some(Action::CheckoutSelectedBranch);
                        }

                        if self.binding_matches_action(
                            "checkout_previous_branch",
                            raw,
                            normalized,
                            &["-"],
                        ) {
                            return Some(Action::CheckoutBranch {
                                branch_ref: "-".to_string(),
                            });
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
                            "open_checkout_branch_prompt",
                            raw,
                            normalized,
                            &["c"],
                        ) {
                            return Some(Action::OpenInputPrompt {
                                operation: super_lazygit_core::InputPromptOperation::CheckoutBranch,
                            });
                        }

                        if self.binding_matches_action(
                            "open_create_branch_prompt",
                            raw,
                            normalized,
                            &["n"],
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
                            "open_branch_upstream_options",
                            raw,
                            normalized,
                            &["u"],
                        ) {
                            return Some(Action::OpenBranchUpstreamOptions);
                        }

                        if self.binding_matches_action(
                            "copy_selected_branch_name",
                            raw,
                            normalized,
                            &["y", "ctrl+o"],
                        ) {
                            return Some(Action::CopySelectedBranchName);
                        }

                        if self.binding_matches_action(
                            "open_branch_pull_request_options",
                            raw,
                            normalized,
                            &["o"],
                        ) {
                            return Some(Action::OpenBranchPullRequestOptions);
                        }

                        if self.binding_matches_action(
                            "open_branch_reset_options",
                            raw,
                            normalized,
                            &["g"],
                        ) {
                            return Some(Action::OpenBranchResetOptions);
                        }

                        if self.binding_matches_action(
                            "open_branch_sort_options",
                            raw,
                            normalized,
                            &["s"],
                        ) {
                            return Some(Action::OpenBranchSortOptions);
                        }

                        if self.binding_matches_action(
                            "open_branch_git_flow_options",
                            raw,
                            normalized,
                            &["G"],
                        ) {
                            return Some(Action::OpenBranchGitFlowOptions);
                        }

                        if self.binding_matches_action(
                            "force_checkout_selected_branch",
                            raw,
                            normalized,
                            &["F"],
                        ) {
                            return Some(Action::ForceCheckoutSelectedBranch);
                        }

                        if self.binding_matches_action(
                            "rebase_current_branch_onto_selected_branch",
                            raw,
                            normalized,
                            &["r"],
                        ) {
                            return Some(Action::RebaseCurrentBranchOntoSelectedBranch);
                        }

                        if self.binding_matches_action(
                            "merge_selected_branch_into_current",
                            raw,
                            normalized,
                            &["M"],
                        ) {
                            return Some(Action::MergeSelectedBranchIntoCurrent);
                        }

                        if self.binding_matches_action(
                            "create_tag_from_selected_branch",
                            raw,
                            normalized,
                            &["T"],
                        ) {
                            return Some(Action::CreateTagFromSelectedBranch);
                        }
                    }
                    RepoSubview::Remotes => {
                        if self.binding_matches_action(
                            "select_next_remote",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextRemote);
                        }

                        if self.binding_matches_action(
                            "select_previous_remote",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousRemote);
                        }

                        if self.binding_matches_action(
                            "open_selected_remote_branches",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::OpenSelectedRemoteBranches);
                        }

                        if self.binding_matches_action(
                            "open_create_remote_prompt",
                            raw,
                            normalized,
                            &["n"],
                        ) {
                            return Some(Action::OpenInputPrompt {
                                operation: super_lazygit_core::InputPromptOperation::CreateRemote,
                            });
                        }

                        if self.binding_matches_action(
                            "open_edit_remote_prompt",
                            raw,
                            normalized,
                            &["e"],
                        ) {
                            if let Some(remote) = selected_remote(
                                repo_mode.detail.as_ref(),
                                repo_mode.remotes_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation:
                                        super_lazygit_core::InputPromptOperation::EditRemote {
                                            current_name: remote.name.clone(),
                                            current_url: remote.fetch_url.clone(),
                                        },
                                });
                            }
                        }

                        if self.binding_matches_action(
                            "remove_selected_remote",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::DeleteSelectedRemote);
                        }

                        if self.binding_matches_action(
                            "fetch_selected_remote",
                            raw,
                            normalized,
                            &["f"],
                        ) {
                            return Some(Action::FetchSelectedRemote);
                        }

                        if self.binding_matches_action(
                            "open_fork_remote_prompt",
                            raw,
                            normalized,
                            &["F"],
                        ) {
                            if let Some(remote) = selected_remote(
                                repo_mode.detail.as_ref(),
                                repo_mode.remotes_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation:
                                        super_lazygit_core::InputPromptOperation::ForkRemote {
                                            suggested_name: fork_remote_suggested_name(
                                                &remote.name,
                                            ),
                                            remote_url: remote.fetch_url.clone(),
                                        },
                                });
                            }
                        }
                    }
                    RepoSubview::RemoteBranches => {
                        if self.binding_matches_action(
                            "select_next_remote_branch",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "select_previous_remote_branch",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "open_selected_remote_branch_commits",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::OpenSelectedRemoteBranchCommits);
                        }

                        if self.binding_matches_action(
                            "checkout_selected_remote_branch",
                            raw,
                            normalized,
                            &["space"],
                        ) {
                            return Some(Action::CheckoutSelectedRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "open_create_local_branch_from_remote_prompt",
                            raw,
                            normalized,
                            &["n"],
                        ) {
                            if let Some(branch) = selected_remote_branch(
                                repo_mode.detail.as_ref(),
                                repo_mode.remote_branches_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation:
                                        super_lazygit_core::InputPromptOperation::CreateBranchFromRemote {
                                            remote_branch_ref: branch.name.clone(),
                                            suggested_name: branch.branch_name.clone(),
                                        },
                                });
                            }
                        }

                        if self.binding_matches_action(
                            "delete_selected_remote_branch",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::DeleteSelectedRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "copy_selected_remote_branch_name",
                            raw,
                            normalized,
                            &["y", "ctrl+o"],
                        ) {
                            return Some(Action::CopySelectedRemoteBranchName);
                        }

                        if self.binding_matches_action(
                            "open_remote_branch_pull_request_options",
                            raw,
                            normalized,
                            &["o"],
                        ) {
                            return Some(Action::OpenRemoteBranchPullRequestOptions);
                        }

                        if self.binding_matches_action(
                            "open_remote_branch_reset_options",
                            raw,
                            normalized,
                            &["g"],
                        ) {
                            return Some(Action::OpenRemoteBranchResetOptions);
                        }

                        if self.binding_matches_action(
                            "open_remote_branch_sort_options",
                            raw,
                            normalized,
                            &["s"],
                        ) {
                            return Some(Action::OpenRemoteBranchSortOptions);
                        }

                        if self.binding_matches_action(
                            "set_current_branch_upstream_to_selected_remote_branch",
                            raw,
                            normalized,
                            &["u"],
                        ) {
                            return Some(Action::SetCurrentBranchUpstreamToSelectedRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "rebase_current_branch_onto_selected_remote_branch",
                            raw,
                            normalized,
                            &["r"],
                        ) {
                            return Some(Action::RebaseCurrentBranchOntoSelectedRemoteBranch);
                        }

                        if self.binding_matches_action(
                            "merge_selected_remote_branch_into_current",
                            raw,
                            normalized,
                            &["M"],
                        ) {
                            return Some(Action::MergeSelectedRemoteBranchIntoCurrent);
                        }

                        if self.binding_matches_action(
                            "create_tag_from_selected_remote_branch",
                            raw,
                            normalized,
                            &["T"],
                        ) {
                            return Some(Action::CreateTagFromSelectedRemoteBranch);
                        }
                    }
                    RepoSubview::Tags => {
                        if self.binding_matches_action(
                            "select_next_tag",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextTag);
                        }

                        if self.binding_matches_action(
                            "select_previous_tag",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousTag);
                        }

                        if self.binding_matches_action(
                            "open_selected_tag_commits",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::OpenSelectedTagCommits);
                        }

                        if self.binding_matches_action(
                            "checkout_selected_tag",
                            raw,
                            normalized,
                            &["space"],
                        ) {
                            return Some(Action::CheckoutSelectedTag);
                        }

                        if self.binding_matches_action(
                            "open_create_tag_prompt",
                            raw,
                            normalized,
                            &["n"],
                        ) {
                            return Some(Action::OpenInputPrompt {
                                operation: super_lazygit_core::InputPromptOperation::CreateTag,
                            });
                        }

                        if self.binding_matches_action(
                            "delete_selected_tag",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::DeleteSelectedTag);
                        }

                        if self.binding_matches_action(
                            "copy_selected_tag_name",
                            raw,
                            normalized,
                            &["ctrl+o", "y"],
                        ) {
                            return Some(Action::CopySelectedTagName);
                        }

                        if self.binding_matches_action(
                            "open_tag_reset_options",
                            raw,
                            normalized,
                            &["g"],
                        ) {
                            return Some(Action::OpenTagResetOptions);
                        }

                        if self.binding_matches_action(
                            "soft_reset_to_selected_tag",
                            raw,
                            normalized,
                            &["S"],
                        ) {
                            return Some(Action::SoftResetToSelectedTag);
                        }

                        if self.binding_matches_action(
                            "mixed_reset_to_selected_tag",
                            raw,
                            normalized,
                            &["M"],
                        ) {
                            return Some(Action::MixedResetToSelectedTag);
                        }

                        if self.binding_matches_action(
                            "hard_reset_to_selected_tag",
                            raw,
                            normalized,
                            &["H"],
                        ) {
                            return Some(Action::HardResetToSelectedTag);
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
                            return Some(Action::ActivateRepoSubviewSelection);
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

                        if self.binding_matches_action(
                            "open_config_file_in_default_app",
                            raw,
                            normalized,
                            &["o"],
                        ) {
                            return Some(Action::OpenConfigFileInDefaultApp);
                        }

                        if self.binding_matches_action(
                            "open_config_file_in_editor",
                            raw,
                            normalized,
                            &["e"],
                        ) {
                            return Some(Action::OpenConfigFileInEditor);
                        }

                        if self.binding_matches_action("check_for_updates", raw, normalized, &["u"])
                        {
                            return Some(Action::CheckForUpdates);
                        }

                        if self.binding_matches_action("nuke_working_tree", raw, normalized, &["X"])
                        {
                            return Some(Action::NukeWorkingTree);
                        }

                        if self.binding_matches_action(
                            "open_all_branch_graph",
                            raw,
                            normalized,
                            &["a"],
                        ) {
                            return Some(Action::OpenAllBranchGraph { reverse: false });
                        }

                        if self.binding_matches_action(
                            "open_all_branch_graph_reverse",
                            raw,
                            normalized,
                            &["A"],
                        ) {
                            return Some(Action::OpenAllBranchGraph { reverse: true });
                        }
                    }
                    RepoSubview::Commits => {
                        let commit_file_diff_active = repo_mode.commit_subview_mode
                            == CommitSubviewMode::Files
                            && repo_mode.commit_files_mode == CommitFilesMode::Diff;

                        if commit_file_diff_active {
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
                                "close_selected_commit_files",
                                raw,
                                normalized,
                                &["enter"],
                            ) {
                                return Some(Action::CloseSelectedCommitFiles);
                            }

                            if self.binding_matches_action(
                                "checkout_selected_commit_file",
                                raw,
                                normalized,
                                &["space"],
                            ) {
                                return Some(Action::CheckoutSelectedCommitFile);
                            }

                            if self.binding_matches_action(
                                "open_in_editor",
                                raw,
                                normalized,
                                &["e"],
                            ) {
                                return Some(Action::OpenInEditor);
                            }
                        } else {
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

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "open_selected_commit_files",
                                    raw,
                                    normalized,
                                    &["enter"],
                                )
                            {
                                return Some(Action::OpenSelectedCommitFiles);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::List
                                && self.binding_matches_action(
                                    "open_selected_commit_files",
                                    raw,
                                    normalized,
                                    &["enter"],
                                )
                            {
                                return Some(Action::OpenSelectedCommitFiles);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::Diff
                                && self.binding_matches_action(
                                    "close_selected_commit_files",
                                    raw,
                                    normalized,
                                    &["enter", "backspace", "left"],
                                )
                            {
                                return Some(Action::CloseSelectedCommitFiles);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::List
                                && self.binding_matches_action(
                                    "close_selected_commit_files",
                                    raw,
                                    normalized,
                                    &["backspace", "left"],
                                )
                            {
                                return Some(Action::CloseSelectedCommitFiles);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "checkout_selected_commit",
                                    raw,
                                    normalized,
                                    &["space"],
                                )
                            {
                                return Some(Action::CheckoutSelectedCommit);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && self.binding_matches_action(
                                    "checkout_selected_commit_file",
                                    raw,
                                    normalized,
                                    &["space"],
                                )
                            {
                                return Some(Action::CheckoutSelectedCommitFile);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && self.binding_matches_action(
                                    "open_in_editor",
                                    raw,
                                    normalized,
                                    &["e"],
                                )
                            {
                                return Some(Action::OpenInEditor);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::List
                                && self.binding_matches_action(
                                    "copy_selected_status_path",
                                    raw,
                                    normalized,
                                    &["y", "ctrl+o"],
                                )
                            {
                                return Some(Action::CopySelectedStatusPath);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::List
                                && self.binding_matches_action(
                                    "open_selected_status_path_in_default_app",
                                    raw,
                                    normalized,
                                    &["o"],
                                )
                            {
                                return Some(Action::OpenSelectedStatusPathInDefaultApp);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::Files
                                && repo_mode.commit_files_mode == CommitFilesMode::List
                                && self.binding_matches_action(
                                    "open_selected_status_path_in_external_difftool",
                                    raw,
                                    normalized,
                                    &["ctrl+t"],
                                )
                            {
                                return Some(Action::OpenSelectedStatusPathInExternalDiffTool);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "copy_selected_commit_for_cherry_pick",
                                    raw,
                                    normalized,
                                    &["C"],
                                )
                            {
                                return Some(Action::CopySelectedCommitForCherryPick);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "cherry_pick_copied_commit",
                                    raw,
                                    normalized,
                                    &["V"],
                                )
                            {
                                return Some(Action::CherryPickCopiedCommit);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "copy_selected_commit_hash",
                                    raw,
                                    normalized,
                                    &["ctrl+o"],
                                )
                            {
                                return Some(Action::CopySelectedCommitHash);
                            }

                            if repo_mode.commit_subview_mode == CommitSubviewMode::History
                                && self.binding_matches_action(
                                    "open_selected_commit_in_browser",
                                    raw,
                                    normalized,
                                    &["o"],
                                )
                            {
                                return Some(Action::OpenSelectedCommitInBrowser);
                            }
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_create_branch_from_commit_prompt",
                                raw,
                                normalized,
                                &["n"],
                            )
                        {
                            return Some(Action::CreateBranchFromSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_create_tag_from_commit_prompt",
                                raw,
                                normalized,
                                &["T"],
                            )
                        {
                            return Some(Action::CreateTagFromSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "start_interactive_rebase",
                                raw,
                                normalized,
                                &["i"],
                            )
                        {
                            return Some(Action::StartInteractiveRebase);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_commit_log_options",
                                raw,
                                normalized,
                                &["ctrl+l"],
                            )
                        {
                            return Some(Action::OpenCommitLogOptions);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_bisect_options",
                                raw,
                                normalized,
                                &["b"],
                            )
                        {
                            return Some(Action::OpenBisectOptions);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "amend_selected_commit",
                                raw,
                                normalized,
                                &["A"],
                            )
                        {
                            return Some(Action::AmendSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_commit_amend_attribute_options",
                                raw,
                                normalized,
                                &["a"],
                            )
                        {
                            return Some(Action::OpenCommitAmendAttributeOptions);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_commit_copy_options",
                                raw,
                                normalized,
                                &["y"],
                            )
                        {
                            return Some(Action::OpenCommitCopyOptions);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "open_commit_fixup_options",
                                raw,
                                normalized,
                                &["f"],
                            )
                        {
                            return Some(Action::OpenCommitFixupOptions);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "fixup_selected_commit",
                                raw,
                                normalized,
                                &["F"],
                            )
                        {
                            return Some(Action::FixupSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "set_fixup_message_for_selected_commit",
                                raw,
                                normalized,
                                &["c"],
                            )
                        {
                            return Some(Action::SetFixupMessageForSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "apply_fixup_commits",
                                raw,
                                normalized,
                                &["g"],
                            )
                        {
                            return Some(Action::ApplyFixupCommits);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "squash_selected_commit",
                                raw,
                                normalized,
                                &["s"],
                            )
                        {
                            return Some(Action::SquashSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "drop_selected_commit",
                                raw,
                                normalized,
                                &["d"],
                            )
                        {
                            return Some(Action::DropSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "move_selected_commit_up",
                                raw,
                                normalized,
                                &["ctrl+k"],
                            )
                        {
                            return Some(Action::MoveSelectedCommitUp);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "move_selected_commit_down",
                                raw,
                                normalized,
                                &["ctrl+j"],
                            )
                        {
                            return Some(Action::MoveSelectedCommitDown);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "reword_selected_commit",
                                raw,
                                normalized,
                                &["r"],
                            )
                        {
                            return Some(Action::RewordSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "reword_selected_commit_with_editor",
                                raw,
                                normalized,
                                &["R"],
                            )
                        {
                            return Some(Action::RewordSelectedCommitWithEditor);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "cherry_pick_selected_commit",
                                raw,
                                normalized,
                                &["C"],
                            )
                        {
                            return Some(Action::CherryPickSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "revert_selected_commit",
                                raw,
                                normalized,
                                &["t"],
                            )
                        {
                            return Some(Action::RevertSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "soft_reset_to_selected_commit",
                                raw,
                                normalized,
                                &["S"],
                            )
                        {
                            return Some(Action::SoftResetToSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "mixed_reset_to_selected_commit",
                                raw,
                                normalized,
                                &["M"],
                            )
                        {
                            return Some(Action::MixedResetToSelectedCommit);
                        }

                        if repo_mode.commit_subview_mode == CommitSubviewMode::History
                            && self.binding_matches_action(
                                "hard_reset_to_selected_commit",
                                raw,
                                normalized,
                                &["H"],
                            )
                        {
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
                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "select_next_stash",
                                raw,
                                normalized,
                                &["j", "down"],
                            )
                        {
                            return Some(Action::SelectNextStash);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::Files
                            && self.binding_matches_action(
                                "select_next_stash_file",
                                raw,
                                normalized,
                                &["j", "down"],
                            )
                        {
                            return Some(Action::SelectNextStashFile);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "select_previous_stash",
                                raw,
                                normalized,
                                &["k", "up"],
                            )
                        {
                            return Some(Action::SelectPreviousStash);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::Files
                            && self.binding_matches_action(
                                "select_previous_stash_file",
                                raw,
                                normalized,
                                &["k", "up"],
                            )
                        {
                            return Some(Action::SelectPreviousStashFile);
                        }

                        if self.binding_matches_action(
                            "activate_repo_subview_selection",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::ActivateRepoSubviewSelection);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "apply_selected_stash",
                                raw,
                                normalized,
                                &["space"],
                            )
                        {
                            return Some(Action::ApplySelectedStash);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "open_create_branch_from_stash_prompt",
                                raw,
                                normalized,
                                &["n"],
                            )
                        {
                            if let Some(stash) = selected_stash(
                                repo_mode.detail.as_ref(),
                                repo_mode.stash_view.selected_index,
                            ) {
                                return Some(Action::OpenInputPrompt {
                                    operation: super_lazygit_core::InputPromptOperation::CreateBranchFromStash {
                                        stash_ref: stash.stash_ref.clone(),
                                        stash_label: stash.label.clone(),
                                    },
                                });
                            }
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "open_rename_stash_prompt",
                                raw,
                                normalized,
                                &["r"],
                            )
                        {
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

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "pop_selected_stash",
                                raw,
                                normalized,
                                &["g"],
                            )
                        {
                            return Some(Action::PopSelectedStash);
                        }

                        if repo_mode.stash_subview_mode == StashSubviewMode::List
                            && self.binding_matches_action(
                                "drop_selected_stash",
                                raw,
                                normalized,
                                &["d"],
                            )
                        {
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
                            "open_selected_reflog_commits",
                            raw,
                            normalized,
                            &["enter"],
                        ) {
                            return Some(Action::OpenSelectedReflogCommits);
                        }

                        if self.binding_matches_action(
                            "checkout_selected_commit",
                            raw,
                            normalized,
                            &["space"],
                        ) {
                            return Some(Action::CheckoutSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "open_create_branch_from_commit_prompt",
                            raw,
                            normalized,
                            &["n"],
                        ) {
                            return Some(Action::CreateBranchFromSelectedCommit);
                        }

                        if self.binding_matches_action(
                            "open_create_tag_from_commit_prompt",
                            raw,
                            normalized,
                            &["T"],
                        ) {
                            return Some(Action::CreateTagFromSelectedCommit);
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
                            "copy_selected_reflog_commit_hash",
                            raw,
                            normalized,
                            &["ctrl+o", "y"],
                        ) {
                            return Some(Action::CopySelectedReflogCommitHash);
                        }

                        if self.binding_matches_action(
                            "open_selected_reflog_in_browser",
                            raw,
                            normalized,
                            &["o"],
                        ) {
                            return Some(Action::OpenSelectedReflogInBrowser);
                        }

                        if self.binding_matches_action(
                            "open_reflog_reset_options",
                            raw,
                            normalized,
                            &["g"],
                        ) {
                            return Some(Action::OpenReflogResetOptions);
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

                        if self.binding_matches_action(
                            "switch_to_selected_worktree",
                            raw,
                            normalized,
                            &["enter", "space"],
                        ) {
                            return Some(Action::ActivateRepoSubviewSelection);
                        }

                        if self.binding_matches_action(
                            "create_worktree",
                            raw,
                            normalized,
                            &["n", "c"],
                        ) {
                            return Some(Action::CreateWorktree);
                        }

                        if self.binding_matches_action("open_in_editor", raw, normalized, &["o"]) {
                            return Some(Action::OpenInEditor);
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
                    RepoSubview::Submodules => {
                        if self.binding_matches_action(
                            "select_next_submodule",
                            raw,
                            normalized,
                            &["j", "down"],
                        ) {
                            return Some(Action::SelectNextSubmodule);
                        }

                        if self.binding_matches_action(
                            "select_previous_submodule",
                            raw,
                            normalized,
                            &["k", "up"],
                        ) {
                            return Some(Action::SelectPreviousSubmodule);
                        }

                        if self.binding_matches_action(
                            "enter_selected_submodule",
                            raw,
                            normalized,
                            &["enter", "space"],
                        ) {
                            return Some(Action::ActivateRepoSubviewSelection);
                        }

                        if self.binding_matches_action("create_submodule", raw, normalized, &["n"])
                        {
                            return Some(Action::CreateSubmodule);
                        }

                        if self.binding_matches_action(
                            "copy_selected_submodule_name",
                            raw,
                            normalized,
                            &["ctrl+o", "y"],
                        ) {
                            return Some(Action::CopySelectedSubmoduleName);
                        }

                        if self.binding_matches_action(
                            "open_submodule_options",
                            raw,
                            normalized,
                            &["b"],
                        ) {
                            return Some(Action::OpenSubmoduleOptions);
                        }

                        if self.binding_matches_action(
                            "edit_selected_submodule",
                            raw,
                            normalized,
                            &["e"],
                        ) {
                            return Some(Action::EditSelectedSubmodule);
                        }

                        if self.binding_matches_action(
                            "init_selected_submodule",
                            raw,
                            normalized,
                            &["i"],
                        ) {
                            return Some(Action::InitSelectedSubmodule);
                        }

                        if self.binding_matches_action(
                            "update_selected_submodule",
                            raw,
                            normalized,
                            &["u"],
                        ) {
                            return Some(Action::UpdateSelectedSubmodule);
                        }

                        if self.binding_matches_action("open_in_editor", raw, normalized, &["o"]) {
                            return Some(Action::OpenInEditor);
                        }

                        if self.binding_matches_action(
                            "remove_selected_submodule",
                            raw,
                            normalized,
                            &["d"],
                        ) {
                            return Some(Action::RemoveSelectedSubmodule);
                        }
                    }
                }

                if matches!(repo_mode.active_subview, RepoSubview::Branches)
                    && self.binding_matches_action(
                        "toggle_comparison_selection",
                        raw,
                        normalized,
                        &["v"],
                    )
                {
                    return Some(Action::ToggleComparisonSelection);
                }

                if repo_mode.active_subview == RepoSubview::Commits
                    && repo_mode.commit_subview_mode == CommitSubviewMode::History
                    && self.binding_matches_action(
                        "toggle_comparison_selection",
                        raw,
                        normalized,
                        &["v"],
                    )
                {
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

        if self.state.focused_pane == PaneId::RepoDetail
            && self.binding_matches_action("focus_repo_main_pane", raw, normalized, &["0"])
        {
            return Some(Action::FocusRepoMainPane);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.binding_matches_action("focus_repo_subview_filter", raw, normalized, &["/"])
        {
            return Some(Action::FocusRepoSubviewFilter);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.binding_matches_action("open_repo_worktrees_subview", raw, normalized, &["w"])
        {
            return Some(Action::OpenRepoWorktreesSubview);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.binding_matches_action("open_repo_submodules_subview", raw, normalized, &["b"])
        {
            return Some(Action::OpenRepoSubmodulesSubview);
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

        if self.binding_matches_action("switch_repo_subview_remotes", raw, normalized, &["m"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Remotes));
        }

        if self.binding_matches_action(
            "switch_repo_subview_remote_branches",
            raw,
            normalized,
            &["9"],
        ) {
            return Some(Action::SwitchRepoSubview(RepoSubview::RemoteBranches));
        }

        if self.binding_matches_action("switch_repo_subview_tags", raw, normalized, &["t"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Tags));
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

        if self.binding_matches_action("switch_repo_subview_submodules", raw, normalized, &["b"]) {
            return Some(Action::SwitchRepoSubview(RepoSubview::Submodules));
        }

        if self.binding_matches_action("refresh_selected_repo", raw, normalized, &["r"]) {
            return Some(Action::RefreshSelectedRepo);
        }

        if self.binding_matches_action("refresh_selected_repo_deep", raw, normalized, &["R"]) {
            return Some(Action::RefreshSelectedRepoDeep);
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

    fn repo_escape_action(&self) -> Action {
        if self.repo_subview_filter_focused() {
            return Action::CancelRepoSubviewFilter;
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self
                .state
                .repo_mode
                .as_ref()
                .is_some_and(|repo_mode| repo_mode.active_subview == RepoSubview::Status)
        {
            return Action::FocusRepoMainPane;
        }

        Action::LeaveRepoMode
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

    fn repo_subview_filter_focused(&self) -> bool {
        self.state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.subview_filter(repo_mode.active_subview))
            .is_some_and(|filter| filter.focused)
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
        match self.state.settings.screen_mode {
            ScreenMode::Normal => self.render_repo_panes(layout[1], buffer, theme, 42),
            ScreenMode::HalfScreen => self.render_repo_panes(layout[1], buffer, theme, 32),
            ScreenMode::FullScreen => {
                self.render_repo_fullscreen(layout[1], buffer, theme);
            }
        }
    }

    fn render_repo_panes(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        theme: Theme,
        side_panel_width: u16,
    ) {
        let panes = split_repo_columns(area, side_panel_width);
        let left = split_repo_left_column(panes[0]);
        self.render_repo_unstaged(left[0], buffer, theme);
        self.render_repo_staged(left[1], buffer, theme);
        self.render_repo_detail(panes[1], buffer, theme);
    }

    fn render_repo_fullscreen(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let pane = self.fullscreen_repo_pane();
        match pane {
            PaneId::RepoUnstaged => self.render_repo_unstaged(area, buffer, theme),
            PaneId::RepoStaged => self.render_repo_staged(area, buffer, theme),
            _ => self.render_repo_detail(area, buffer, theme),
        }
    }

    fn fullscreen_repo_pane(&self) -> PaneId {
        match self.state.focused_pane {
            PaneId::RepoUnstaged | PaneId::RepoStaged | PaneId::RepoDetail => {
                self.state.focused_pane
            }
            _ => self
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.main_focus)
                .unwrap_or(PaneId::RepoDetail),
        }
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
                "Repo: {}  Branch: {}  Watch: {}  Screen: {}",
                title,
                self.selected_summary()
                    .and_then(|summary| summary.branch.as_deref())
                    .unwrap_or("detached"),
                watcher_health_label(&self.state.workspace.watcher_health),
                self.state.settings.screen_mode.label()
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
            Some(repo_mode),
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
        let lines = repo_staged_lines(repo_mode, self.state.focused_pane == PaneId::RepoStaged);
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
                    repo_mode
                        .subview_filter(RepoSubview::Branches)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Branches)
                        .is_some_and(|filter| filter.focused),
                    repo_mode.comparison_base.as_ref(),
                    repo_mode.comparison_target.as_ref(),
                    repo_mode.comparison_source,
                    repo_mode.branch_sort_mode,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Remotes => repo_remote_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.remotes_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Remotes)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Remotes)
                        .is_some_and(|filter| filter.focused),
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::RemoteBranches => repo_remote_branch_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.remote_branches_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::RemoteBranches)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::RemoteBranches)
                        .is_some_and(|filter| filter.focused),
                    repo_mode.remote_branch_sort_mode,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Tags => repo_tag_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.tags_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Tags)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Tags)
                        .is_some_and(|filter| filter.focused),
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Commits => {
                    repo_commit_lines(repo_mode, usize::from(area.height.saturating_sub(2)), theme)
                }
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
                    repo_mode.stash_files_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Stash)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Stash)
                        .is_some_and(|filter| filter.focused),
                    repo_mode.stash_subview_mode,
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Reflog => repo_reflog_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.reflog_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Reflog)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Reflog)
                        .is_some_and(|filter| filter.focused),
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Worktrees => repo_worktree_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.worktree_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Worktrees)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Worktrees)
                        .is_some_and(|filter| filter.focused),
                    self.state.focused_pane == PaneId::RepoDetail,
                    theme,
                ),
                RepoSubview::Submodules => repo_submodule_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.submodules_view.selected_index,
                    repo_mode
                        .subview_filter(RepoSubview::Submodules)
                        .map(|filter| filter.query.as_str())
                        .unwrap_or(""),
                    repo_mode
                        .subview_filter(RepoSubview::Submodules)
                        .is_some_and(|filter| filter.focused),
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
            if matches!(self.state.mode, AppMode::Repository) && self.state.modal_stack.is_empty() {
                lines.push(Line::from(repo_screen_status_line(&self.state)));
            }
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
                    lines.extend(menu_lines(&self.state, menu, theme));
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

fn split_repo_columns(area: Rect, side_panel_width: u16) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(side_panel_width),
            Constraint::Percentage(100_u16.saturating_sub(side_panel_width)),
        ])
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

#[allow(clippy::too_many_arguments)]
fn repo_branch_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    comparison_base: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_target: Option<&super_lazygit_core::ComparisonTarget>,
    comparison_source: Option<RepoSubview>,
    branch_sort_mode: super_lazygit_core::BranchSortMode,
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

    let visible_indices = visible_branch_indices(detail, filter_query, branch_sort_mode);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Branches",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.branches.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.branches.is_empty() {
                "No local branches available.".to_string()
            } else {
                format!("No branches match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .or_else(|| {
            detail
                .branches
                .iter()
                .enumerate()
                .find_map(|(index, branch)| {
                    (branch.is_head && visible_indices.contains(&index)).then_some(index)
                })
        })
        .unwrap_or(visible_indices[0]);
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
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.branches.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from(comparison_status_line(
            RepoSubview::Branches,
            comparison_base,
            comparison_target,
            comparison_source,
        )),
        Line::from("Context: Enter commits. Space checkout. F force checkout. 0 main. / filter."),
        Line::from("Other: w worktrees. - previous. c checkout by name. n create. R rename."),
        Line::from("       d delete. u upstream. o pull request. g reset. s sort. G git-flow."),
        Line::from("       y/Ctrl+O copy. r rebase current. M merge current. T tag. v compare."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let branch = &detail.branches[index];
        let style = branch_row_style(branch, index == selected_index, is_focused, theme);
        lines.push(Line::from(Span::styled(branch_row_label(branch), style)));
    }

    lines
}

fn repo_remote_branch_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    remote_branch_sort_mode: super_lazygit_core::RemoteBranchSortMode,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Remote Branches",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let visible_indices =
        visible_remote_branch_indices(detail, filter_query, remote_branch_sort_mode);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Remote Branches",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(
                filter_query,
                filter_focused,
                0,
                detail.remote_branches.len(),
            )
            .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.remote_branches.is_empty() {
                "No remote branches available.".to_string()
            } else {
                format!("No remote branches match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    let selected_branch = &detail.remote_branches[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Remote Branches",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_branch.name)),
        Line::from(format!("Remote: {}", selected_branch.remote_name)),
        Line::from(format!("Local branch: {}", selected_branch.branch_name)),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.remote_branches.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter commits. Space checkout. 0 main. / filter. w worktrees."),
        Line::from("Other: n create local branch. d delete remote branch. o pull request."),
        Line::from("       g reset. s sort. y/Ctrl+O copy. u set upstream."),
        Line::from("       r rebase current. M merge current. T tag."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let branch = &detail.remote_branches[index];
        let style = if index == selected_index {
            let mut style = Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
            style
        } else {
            Style::default().fg(theme.foreground)
        };
        lines.push(Line::from(Span::styled(
            remote_branch_row_label(branch),
            style,
        )));
    }

    lines
}

fn repo_remote_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Remotes",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let visible_indices = visible_remote_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Remotes",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.remotes.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.remotes.is_empty() {
                "No remotes are configured.".to_string()
            } else {
                format!("No remotes match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    let selected_remote = &detail.remotes[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Remotes",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_remote.name)),
        Line::from(format!("Fetch: {}", selected_remote.fetch_url)),
        Line::from(format!("Push: {}", selected_remote.push_url)),
        Line::from(format!("Branches: {}", selected_remote.branch_count)),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.remotes.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter branches. f fetch. 0 main. / filter. w worktrees."),
        Line::from("Other: n new remote. e edit remote. d remove remote. F fork remote."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let remote = &detail.remotes[index];
        let style = if index == selected_index {
            let mut style = Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
            style
        } else {
            Style::default().fg(theme.foreground)
        };
        lines.push(Line::from(Span::styled(
            format!("{}  [{} branches]", remote.name, remote.branch_count),
            style,
        )));
    }

    lines
}

fn repo_tag_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Tags",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let visible_indices = visible_tag_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Tags",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.tags.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.tags.is_empty() {
                "No tags available.".to_string()
            } else {
                format!("No tags match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    let selected_tag = &detail.tags[selected_index];

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Tags",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_tag.name)),
        Line::from(format!("Target: {}", selected_tag.target_short_oid)),
        Line::from(format!(
            "Type: {}",
            if selected_tag.annotated {
                "annotated"
            } else {
                "lightweight"
            }
        )),
        Line::from(format!("Summary: {}", selected_tag.summary)),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.tags.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from(
            "Context: Enter commits. Space checkout. Ctrl+O copy tag. g reset menu. 0 main. / filter. w worktrees.",
        ),
        Line::from("Other: n create tag. d delete tag. P push tag. S/M/H reset to tag."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let tag = &detail.tags[index];
        let style = if index == selected_index {
            let mut style = Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
            style
        } else {
            Style::default().fg(theme.foreground)
        };
        lines.push(Line::from(Span::styled(tag_row_label(tag), style)));
    }

    lines
}

fn branch_row_label(branch: &super_lazygit_core::BranchItem) -> String {
    let head = if branch.is_head { "*" } else { " " };
    let upstream = branch.upstream.as_deref().unwrap_or("-");
    format!("{head} {:<20} upstream={upstream}", branch.name)
}

fn remote_branch_row_label(branch: &super_lazygit_core::RemoteBranchItem) -> String {
    format!(
        "  {:<28} remote={} local={}",
        branch.name, branch.remote_name, branch.branch_name
    )
}

fn tag_row_label(tag: &super_lazygit_core::TagItem) -> String {
    let kind = if tag.annotated {
        "annotated"
    } else {
        "lightweight"
    };
    format!("  {:<24} {}  {}", tag.name, tag.target_short_oid, kind)
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

fn selected_remote(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::RemoteItem> {
    let detail = detail?;
    let selected_index = selected_index
        .filter(|index| *index < detail.remotes.len())
        .unwrap_or(0);
    detail.remotes.get(selected_index)
}

fn fork_remote_suggested_name(remote_name: &str) -> String {
    if remote_name == "origin" {
        "upstream".to_string()
    } else {
        format!("{remote_name}-fork")
    }
}

fn selected_remote_branch(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::RemoteBranchItem> {
    let detail = detail?;
    let selected_index = selected_index
        .filter(|index| *index < detail.remote_branches.len())
        .unwrap_or(0);
    detail.remote_branches.get(selected_index)
}

fn selected_tag(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::TagItem> {
    let detail = detail?;
    let selected_index = selected_index
        .filter(|index| *index < detail.tags.len())
        .unwrap_or(0);
    detail.tags.get(selected_index)
}

fn selected_stash(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
) -> Option<&super_lazygit_core::StashItem> {
    let detail = detail?;
    let selected_index = selected_index
        .filter(|index| *index < detail.stashes.len())
        .unwrap_or(0);
    detail.stashes.get(selected_index)
}

fn stash_message_label(label: &str) -> String {
    label
        .rsplit_once(": ")
        .map_or_else(|| label.to_string(), |(_, message)| message.to_string())
}

#[allow(clippy::too_many_arguments)]
fn repo_stash_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    selected_file_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    stash_subview_mode: StashSubviewMode,
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

    match stash_subview_mode {
        StashSubviewMode::List => repo_stash_list_lines(
            detail,
            selected_index,
            filter_query,
            filter_focused,
            is_focused,
            theme,
        ),
        StashSubviewMode::Files => {
            repo_stash_file_lines(detail, selected_index, selected_file_index, theme)
        }
    }
}

fn repo_stash_list_lines(
    detail: &RepoDetail,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let visible_indices = visible_stash_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Stashes",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.stashes.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.stashes.is_empty() {
                "No stashes are available.".to_string()
            } else {
                format!("No stashes match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
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
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.stashes.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter files. Space apply. 0 main. / filter. w worktrees."),
        Line::from("Other: n branches off. r renames. g pops. d drops."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let stash = &detail.stashes[index];
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

fn repo_stash_file_lines(
    detail: &RepoDetail,
    selected_stash_index: Option<usize>,
    selected_file_index: Option<usize>,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(selected_stash) = selected_stash(Some(detail), selected_stash_index) else {
        return vec![
            Line::from(vec![Span::styled(
                "Stash files",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("No stash is currently selected."),
        ];
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Stash files  {}  {}",
                selected_stash.stash_ref,
                stash_message_label(&selected_stash.label)
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(selected_stash.label.clone()),
        Line::from("Context: Enter stash list. 0 main. w worktrees."),
        Line::from("Other: apply/pop/drop/rename/new-branch stay on the stash list."),
        Line::from(""),
    ];

    if selected_stash.changed_files.is_empty() {
        lines.push(Line::from("No changed files were reported for this stash."));
        return lines;
    }

    let selected_file_index = selected_file_index
        .filter(|index| *index < selected_stash.changed_files.len())
        .unwrap_or(0);
    for (index, file) in selected_stash.changed_files.iter().enumerate() {
        let prefix = if index == selected_file_index {
            ">"
        } else {
            " "
        };
        lines.push(Line::from(format!(
            "{prefix} {} {}",
            file_status_kind_label(file.kind),
            file.path.display()
        )));
    }

    lines
}

fn repo_reflog_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
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

    let visible_indices = visible_reflog_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Reflog",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.reflog_items.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.reflog_items.is_empty() {
                "No reflog entries are available.".to_string()
            } else {
                format!("No reflog entries match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
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
            visible_indices
                .iter()
                .position(|index| *index == selected_index)
                .map(|index| index + 1)
                .unwrap_or(1),
            visible_indices.len()
        )),
        Line::from(format!(
            "{}  {}",
            selected_entry.selector,
            if selected_entry.short_oid.is_empty() {
                selected_entry.summary.clone()
            } else if selected_entry.summary.is_empty() {
                selected_entry.short_oid.clone()
            } else {
                format!("{} {}", selected_entry.short_oid, selected_entry.summary)
            }
        )),
        Line::from(selected_entry.description.clone()),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.reflog_items.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from(
            "Context: Enter commits. Space checkout. Ctrl+O copy hash. o browser. n branch. T tag. C cherry-pick.",
        ),
        Line::from(
            "Context: g reset menu. S/M/H reset by reflog target. u restore HEAD. 0 main. / filter. w worktrees.",
        ),
        Line::from("Use j/k to inspect recent HEAD and ref movement."),
        Line::from("Limits: no working tree undo; redo is manual by selecting another entry."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let entry = &detail.reflog_items[index];
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
    filter_query: &str,
    filter_focused: bool,
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

    let visible_indices = visible_worktree_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Worktrees",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.worktrees.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.worktrees.is_empty() {
                "No linked worktrees are available.".to_string()
            } else {
                format!("No worktrees match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
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
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.worktrees.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter/Space switch. 0 main. / filter."),
        Line::from("Other: n create. o open selected worktree. d remove."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let worktree = &detail.worktrees[index];
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

fn repo_submodule_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
    filter_query: &str,
    filter_focused: bool,
    is_focused: bool,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = detail else {
        return vec![
            Line::from(vec![Span::styled(
                "Submodules",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let visible_indices = visible_submodule_indices(detail, filter_query);
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "Submodules",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(filter_query, filter_focused, 0, detail.submodules.len())
                .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.submodules.is_empty() {
                "No submodules are configured for this repository.".to_string()
            } else {
                format!("No submodules match /{}.", filter_query)
            }),
        ];
    }

    let selected_index = selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    let selected_submodule = &detail.submodules[selected_index];
    let state_label = if selected_submodule.conflicted {
        "conflicted"
    } else if !selected_submodule.initialized {
        "uninitialized"
    } else if selected_submodule.dirty {
        "dirty"
    } else {
        "clean"
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Submodules",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Selected: {}", selected_submodule.path.display())),
        Line::from(format!("Name: {}", selected_submodule.name)),
        Line::from(format!(
            "State: {state_label}  Branch: {}  HEAD: {}",
            selected_submodule.branch.as_deref().unwrap_or("(detached)"),
            selected_submodule
                .short_oid
                .as_deref()
                .unwrap_or("(uninitialized)")
        )),
        Line::from(format!("URL: {}", selected_submodule.url)),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        detail.submodules.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter/Space open nested repo. Ctrl+O copy submodule. b options menu. 0 main. / filter."),
        Line::from("Other: n add. e edit URL. i init. u update. o open path. d remove."),
        Line::from(""),
    ]);

    for index in visible_indices {
        let submodule = &detail.submodules[index];
        let mut style = Style::default().fg(theme.foreground);
        if index == selected_index {
            style = style.add_modifier(Modifier::BOLD);
            if is_focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }
        let state_label = if submodule.conflicted {
            "conflicted"
        } else if !submodule.initialized {
            "uninitialized"
        } else if submodule.dirty {
            "dirty"
        } else {
            "clean"
        };
        let branch = submodule.branch.as_deref().unwrap_or("(detached)");
        let head = submodule.short_oid.as_deref().unwrap_or("-------");
        lines.push(Line::from(Span::styled(
            format!(
                "{}  [{}]  {head}  {state_label}",
                submodule.path.display(),
                branch
            ),
            style,
        )));
    }

    lines
}

fn repo_commit_lines(
    repo_mode: &RepoModeState,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return vec![
            Line::from(vec![Span::styled(
                if repo_mode.commit_history_mode.is_graph() {
                    "Commit graph"
                } else {
                    "Commit history"
                },
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Repository detail is still loading."),
        ];
    };

    let visible_indices = visible_commit_indices(detail, repo_mode.commits_filter.query.as_str());
    if visible_indices.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                if repo_mode.commit_history_mode.is_graph() {
                    "Commit graph"
                } else {
                    "Commit history"
                },
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            repo_filter_summary_line(
                repo_mode.commits_filter.query.as_str(),
                repo_mode.commits_filter.focused,
                0,
                detail.commits.len(),
            )
            .unwrap_or_else(|| Line::from("")),
            Line::from(if detail.commits.is_empty() {
                "No commits available for this repository.".to_string()
            } else {
                format!("No commits match /{}.", repo_mode.commits_filter.query)
            }),
        ];
    }

    let selected_index = repo_mode
        .commits_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    let selected = &detail.commits[selected_index];
    let selected_position = visible_indices
        .iter()
        .position(|index| *index == selected_index)
        .unwrap_or(0);

    if repo_mode.commit_subview_mode == CommitSubviewMode::Files {
        return match repo_mode.commit_files_mode {
            CommitFilesMode::List => {
                repo_commit_file_list_lines(repo_mode, selected, viewport_lines, theme)
            }
            CommitFilesMode::Diff => {
                repo_commit_file_diff_lines(repo_mode, selected, viewport_lines, theme)
            }
        };
    }

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Selected {}/{}  {}  {}",
                selected_position + 1,
                visible_indices.len(),
                selected.short_oid,
                selected.summary
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(comparison_status_line(
            RepoSubview::Commits,
            repo_mode.comparison_base.as_ref(),
            repo_mode.comparison_target.as_ref(),
            repo_mode.comparison_source,
        )),
        Line::from(history_operation_state_line(&detail.merge_state)),
        copied_commit_line(repo_mode, theme).unwrap_or_else(|| Line::from("")),
        Line::from(repo_commit_context_line(
            repo_mode.commit_history_mode,
            repo_mode.commit_history_ref.as_deref(),
            repo_mode.commits_filter.query.as_str(),
            repo_mode.commits_filter.focused,
            visible_indices.len(),
            detail.commits.len(),
        )),
        Line::from("Actions: Enter files  Space checkout  n branch  T tag  b bisect  i rebase"),
        Line::from("         A amend  f fixup menu  F fixup  g apply-fixups  s squash  d drop"),
        Line::from("         Ctrl+K/Ctrl+J move  r reword  R reword editor  y copy menu  C copy  V paste copied  t revert  S soft  M mixed  H hard  m menu"),
        Line::from("History:"),
    ];

    let window_start = selected_position.saturating_sub(1);
    let window_end = (window_start + 3).min(visible_indices.len());
    lines.extend(
        visible_indices[window_start..window_end]
            .iter()
            .map(|index| {
                let commit = &detail.commits[*index];
                let prefix = if *index == selected_index { ">" } else { " " };
                let row = if repo_mode.commit_history_mode.is_graph() {
                    detail
                        .commit_graph_lines
                        .get(*index)
                        .cloned()
                        .unwrap_or_else(|| format!("{} {}", commit.short_oid, commit.summary))
                } else {
                    format!("{} {}", commit.short_oid, commit.summary)
                };
                Line::from(format!("{prefix} {row}"))
            }),
    );

    if selected.changed_files.is_empty() {
        lines.push(Line::from("Files: (no changed files reported)"));
    } else {
        let first = &selected.changed_files[0];
        lines.push(Line::from(format!(
            "Files: {} {}",
            file_status_kind_label(first.kind),
            first.path.display()
        )));
        lines.extend(selected.changed_files.iter().skip(1).take(5).map(|file| {
            Line::from(format!(
                "       {} {}",
                file_status_kind_label(file.kind),
                file.path.display()
            ))
        }));
        if selected.changed_files.len() > 6 {
            lines.push(Line::from(format!(
                "       … {} more file(s)",
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

#[allow(clippy::too_many_arguments)]
fn repo_commit_file_list_lines(
    repo_mode: &RepoModeState,
    selected_commit: &super_lazygit_core::CommitItem,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let filter_query = repo_mode.commit_files_filter.query.as_str();
    let filter_focused = repo_mode.commit_files_filter.focused;
    let visible_indices = visible_commit_file_indices(selected_commit, filter_query);
    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Commit files  {}  {}",
                selected_commit.short_oid, selected_commit.summary
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(comparison_status_line(
            RepoSubview::Commits,
            repo_mode.comparison_base.as_ref(),
            repo_mode.comparison_target.as_ref(),
            repo_mode.comparison_source,
        )),
    ];
    if let Some(filter_line) = repo_filter_summary_line(
        filter_query,
        filter_focused,
        visible_indices.len(),
        selected_commit.changed_files.len(),
    ) {
        lines.push(filter_line);
    }
    lines.extend([
        Line::from("Context: Enter file diff. Left/backspace history. Space checkout file."),
        Line::from("Actions: e editor. y copy path. o open. Ctrl+T difftool. 0 main. / filter. w worktrees."),
        Line::from(""),
    ]);

    if visible_indices.is_empty() {
        lines.push(Line::from(if selected_commit.changed_files.is_empty() {
            "No changed files were reported for this commit.".to_string()
        } else {
            format!("No changed files match /{}.", filter_query)
        }));
        lines.truncate(viewport_lines.max(1));
        return lines;
    }

    let selected_index = repo_mode
        .commit_files_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices[0]);
    for index in visible_indices {
        let file = &selected_commit.changed_files[index];
        let prefix = if index == selected_index { ">" } else { " " };
        lines.push(Line::from(format!(
            "{prefix} {} {}",
            file_status_kind_label(file.kind),
            file.path.display()
        )));
    }

    lines.truncate(viewport_lines.max(1));
    lines
}

fn repo_commit_file_diff_lines(
    repo_mode: &RepoModeState,
    selected_commit: &super_lazygit_core::CommitItem,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    let visible_indices = visible_commit_file_indices(
        selected_commit,
        repo_mode.commit_files_filter.query.as_str(),
    );
    let selected_index = repo_mode
        .commit_files_view
        .selected_index
        .filter(|index| visible_indices.contains(index))
        .unwrap_or(visible_indices.first().copied().unwrap_or(0));
    let selected_file = selected_commit.changed_files.get(selected_index);
    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(
                "Commit file diff  {}  {}",
                selected_commit.short_oid, selected_commit.summary
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(comparison_status_line(
            RepoSubview::Commits,
            repo_mode.comparison_base.as_ref(),
            repo_mode.comparison_target.as_ref(),
            repo_mode.comparison_source,
        )),
        Line::from(format!(
            "File: {}",
            selected_file
                .map(|file| file.path.display().to_string())
                .unwrap_or_else(|| "No file selected".to_string())
        )),
        Line::from("Context: Enter files. Space checkout file. e open editor."),
        Line::from("Inspect: j/k hunks. J/K changed lines. v anchor."),
        Line::from(""),
    ];

    let remaining = viewport_lines.saturating_sub(lines.len()).max(1);
    lines.extend(repo_diff_lines(
        Some(repo_mode),
        repo_mode.detail.as_ref(),
        repo_mode.diff_scroll,
        remaining,
        theme,
    ));
    lines.truncate(viewport_lines.max(1));
    lines
}

fn visible_branch_indices(
    detail: &RepoDetail,
    filter_query: &str,
    branch_sort_mode: super_lazygit_core::BranchSortMode,
) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    let mut indices: Vec<_> = detail
        .branches
        .iter()
        .enumerate()
        .filter_map(|(index, branch)| {
            (normalized.is_empty()
                || super_lazygit_core::branch_matches_filter(branch, &normalized))
            .then_some(index)
        })
        .collect();
    if branch_sort_mode == super_lazygit_core::BranchSortMode::Name {
        indices.sort_by(|left, right| {
            detail.branches[*left]
                .name
                .cmp(&detail.branches[*right].name)
                .then_with(|| left.cmp(right))
        });
    }
    indices
}

fn visible_remote_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.remotes.len()).collect();
    }
    detail
        .remotes
        .iter()
        .enumerate()
        .filter_map(|(index, remote)| {
            super_lazygit_core::remote_matches_filter(remote, &normalized).then_some(index)
        })
        .collect()
}

fn visible_remote_branch_indices(
    detail: &RepoDetail,
    filter_query: &str,
    remote_branch_sort_mode: super_lazygit_core::RemoteBranchSortMode,
) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    let mut indices: Vec<_> = detail
        .remote_branches
        .iter()
        .enumerate()
        .filter_map(|(index, branch)| {
            (normalized.is_empty()
                || super_lazygit_core::remote_branch_matches_filter(branch, &normalized))
            .then_some(index)
        })
        .collect();
    if remote_branch_sort_mode == super_lazygit_core::RemoteBranchSortMode::Name {
        indices.sort_by(|left, right| {
            detail.remote_branches[*left]
                .name
                .cmp(&detail.remote_branches[*right].name)
                .then_with(|| left.cmp(right))
        });
    }
    indices
}

fn visible_tag_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.tags.len()).collect();
    }
    detail
        .tags
        .iter()
        .enumerate()
        .filter_map(|(index, tag)| {
            super_lazygit_core::tag_matches_filter(tag, &normalized).then_some(index)
        })
        .collect()
}

fn visible_commit_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.commits.len()).collect();
    }
    detail
        .commits
        .iter()
        .enumerate()
        .filter_map(|(index, commit)| {
            super_lazygit_core::commit_matches_filter(commit, &normalized).then_some(index)
        })
        .collect()
}

fn visible_commit_file_indices(
    commit: &super_lazygit_core::CommitItem,
    filter_query: &str,
) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..commit.changed_files.len()).collect();
    }
    commit
        .changed_files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            super_lazygit_core::commit_file_matches_filter(file, &normalized).then_some(index)
        })
        .collect()
}

fn visible_stash_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.stashes.len()).collect();
    }
    detail
        .stashes
        .iter()
        .enumerate()
        .filter_map(|(index, stash)| {
            super_lazygit_core::stash_matches_filter(stash, &normalized).then_some(index)
        })
        .collect()
}

fn visible_reflog_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.reflog_items.len()).collect();
    }
    detail
        .reflog_items
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            super_lazygit_core::reflog_matches_filter(entry, &normalized).then_some(index)
        })
        .collect()
}

fn visible_worktree_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.worktrees.len()).collect();
    }
    detail
        .worktrees
        .iter()
        .enumerate()
        .filter_map(|(index, worktree)| {
            super_lazygit_core::worktree_matches_filter(worktree, &normalized).then_some(index)
        })
        .collect()
}

fn visible_submodule_indices(detail: &RepoDetail, filter_query: &str) -> Vec<usize> {
    let normalized = super_lazygit_core::normalize_search_text(filter_query);
    if normalized.is_empty() {
        return (0..detail.submodules.len()).collect();
    }
    detail
        .submodules
        .iter()
        .enumerate()
        .filter_map(|(index, submodule)| {
            super_lazygit_core::submodule_matches_filter(submodule, &normalized).then_some(index)
        })
        .collect()
}

fn repo_filter_summary_line(
    filter_query: &str,
    filter_focused: bool,
    visible_count: usize,
    total_count: usize,
) -> Option<Line<'static>> {
    if filter_query.is_empty() && !filter_focused {
        return None;
    }
    let query = if filter_query.is_empty() {
        "_".to_string()
    } else if filter_focused {
        format!("{filter_query}_")
    } else {
        filter_query.to_string()
    };
    Some(Line::from(format!(
        "Filter /{query}  Matches: {visible_count}/{total_count}{}",
        if filter_focused { "  (focused)" } else { "" }
    )))
}

fn repo_commit_context_line(
    commit_history_mode: CommitHistoryMode,
    commit_history_ref: Option<&str>,
    filter_query: &str,
    filter_focused: bool,
    visible_count: usize,
    total_count: usize,
) -> String {
    let mut line = if commit_history_mode.is_graph() || commit_history_ref.is_some() {
        "Context: Enter files. Ctrl+O copy hash. a amend attrs. y copy menu. o browser. 3 current branch. Ctrl+L log menu. 0 main. / filter. w worktrees."
            .to_string()
    } else {
        "Context: Enter files. Ctrl+O copy hash. a amend attrs. y copy menu. o browser. Ctrl+L log menu. 0 main. / filter. w worktrees.".to_string()
    };
    if let Some(filter_line) =
        repo_filter_summary_line(filter_query, filter_focused, visible_count, total_count)
    {
        let rendered = filter_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        line.push_str("  ");
        line.push_str(&rendered);
    }
    line
}

fn copied_commit_line(repo_mode: &RepoModeState, theme: Theme) -> Option<Line<'static>> {
    repo_mode.copied_commit.as_ref().map(|commit| {
        Line::from(vec![Span::styled(
            format!(
                "Copied for cherry-pick: {} {}  V paste  Ctrl+R clear",
                commit.short_oid, commit.summary
            ),
            Style::default().fg(theme.success),
        )])
    })
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
        Line::from(diff_action_help_line(repo_mode, detail)),
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
        super_lazygit_core::ConfirmableOperation::FetchRemote { remote_name } => {
            format!("Fetch updates from remote {remote_name}?")
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
        super_lazygit_core::ConfirmableOperation::ApplyFixupCommits { summary, .. } => {
            format!(
                "Apply pending fixup and squash commits for {summary}? Git will rewrite the current branch with autosquash."
            )
        }
        super_lazygit_core::ConfirmableOperation::FixupCommit { summary, .. } => {
            format!(
                "Create a fixup commit for {summary} from the currently staged changes and autosquash it with rebase?"
            )
        }
        super_lazygit_core::ConfirmableOperation::SetFixupMessageForCommit { summary, .. } => {
            format!(
                "Fold {summary} into its parent and keep that commit message with git rebase fixup -C?"
            )
        }
        super_lazygit_core::ConfirmableOperation::SquashCommit { summary, .. } => {
            format!(
                "Squash {summary} into its parent commit? Git will rewrite history and keep the default combined message."
            )
        }
        super_lazygit_core::ConfirmableOperation::DropCommit { summary, .. } => {
            format!(
                "Drop {summary} from history? Git will rewrite the current branch and remove that commit."
            )
        }
        super_lazygit_core::ConfirmableOperation::MoveCommitUp {
            summary,
            adjacent_summary,
            ..
        } => {
            format!(
                "Move {summary} above {adjacent_summary}? Git will rewrite the current branch and swap those adjacent commits."
            )
        }
        super_lazygit_core::ConfirmableOperation::MoveCommitDown {
            summary,
            adjacent_summary,
            ..
        } => {
            format!(
                "Move {summary} below {adjacent_summary}? Git will rewrite the current branch and swap those adjacent commits."
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
        super_lazygit_core::ConfirmableOperation::UnsetBranchUpstream { branch_name } => {
            format!(
                "Unset upstream for {branch_name}? Future pulls and pushes will stop using its configured tracking branch."
            )
        }
        super_lazygit_core::ConfirmableOperation::FastForwardCurrentBranchFromUpstream {
            branch_name,
            upstream_ref,
        } => format!(
            "Fast-forward {branch_name} from {upstream_ref}? This runs git merge --ff-only {upstream_ref} on the current branch."
        ),
        super_lazygit_core::ConfirmableOperation::MergeRefIntoCurrent {
            source_label,
            target_ref,
        } => format!(
            "Merge {source_label} into the current branch? This runs git merge {target_ref}."
        ),
        super_lazygit_core::ConfirmableOperation::ForceCheckoutRef {
            source_label,
            target_ref,
        } => format!(
            "Force-checkout {source_label}? This runs git checkout -f {target_ref} and discards tracked working tree changes in the current checkout."
        ),
        super_lazygit_core::ConfirmableOperation::RebaseCurrentBranchOntoRef {
            source_label,
            target_ref,
        } => format!(
            "Rebase the current branch onto {source_label}? This runs git rebase {target_ref} and rewrites local history."
        ),
        super_lazygit_core::ConfirmableOperation::RemoveRemote { remote_name } => {
            format!("Remove remote {remote_name}? This deletes the configured remote entry.")
        }
        super_lazygit_core::ConfirmableOperation::DeleteRemoteBranch {
            remote_name,
            branch_name,
        } => format!(
            "Delete remote branch {remote_name}/{branch_name}? This runs git push {remote_name} --delete {branch_name}."
        ),
        super_lazygit_core::ConfirmableOperation::DeleteTag { tag_name } => {
            format!("Delete tag {tag_name}? This removes the local tag reference.")
        }
        super_lazygit_core::ConfirmableOperation::PushTag {
            remote_name,
            tag_name,
        } => format!(
            "Push tag {tag_name} to {remote_name}? This runs git push {remote_name} refs/tags/{tag_name}."
        ),
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
        super_lazygit_core::ConfirmableOperation::RemoveSubmodule { name, path } => {
            format!(
                "Remove submodule {name} at {}? This deinitializes it, removes the gitlink, and updates .gitmodules.",
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

fn diff_action_help_line(repo_mode: Option<&RepoModeState>, detail: &RepoDetail) -> String {
    let range = selected_diff_range_label(repo_mode, &detail.diff);
    match detail.diff.presentation {
        DiffPresentation::Unstaged => format!(
            "Line select: J/K cursor  v range  L stage lines  Enter/Space stage hunk  Mode: {}  Range: {}",
            if detail.merge_state == super_lazygit_core::MergeState::None {
                "working tree staging"
            } else {
                "merge resolution"
            },
            range
        ),
        DiffPresentation::Staged => format!(
            "Line select: J/K cursor  v range  L unstage lines  Enter/Space unstage hunk  Mode: staged changes  Range: {range}"
        ),
        DiffPresentation::Comparison => {
            format!("Read-only diff: j/k hunks  Down/Up scroll  Mode: comparison  Range: {range}")
        }
    }
}

fn input_prompt_copy(operation: &super_lazygit_core::InputPromptOperation) -> String {
    match operation {
        super_lazygit_core::InputPromptOperation::CheckoutBranch => {
            "Enter a branch name, remote ref, or -. Use - to switch back to the previous branch."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::CreateBranch => {
            "Enter the new branch name. The branch will be created from HEAD and checked out."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::CreateRemote => {
            "Enter remote details as: <name> <url>. Example: upstream git@github.com:owner/repo.git."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::ForkRemote {
            suggested_name,
            remote_url,
        } => format!(
            "Enter fork remote details as: <name> <url>. The prompt starts from {suggested_name} {remote_url} so you can keep or adjust both values before adding the remote."
        ),
        super_lazygit_core::InputPromptOperation::CreateTag => {
            "Enter the new tag name. The tag will be created at the current HEAD.".to_string()
        }
        super_lazygit_core::InputPromptOperation::CreateTagFromCommit { summary, .. } => format!(
            "Enter the new tag name. The tag will be created from {summary}."
        ),
        super_lazygit_core::InputPromptOperation::CreateTagFromRef { source_label, .. } => {
            format!("Enter the new tag name. The tag will be created from {source_label}.")
        }
        super_lazygit_core::InputPromptOperation::CreateBranchFromCommit {
            summary, ..
        } => format!(
            "Enter the new branch name. The branch will be created from {summary} and checked out."
        ),
        super_lazygit_core::InputPromptOperation::CreateBranchFromRemote {
            remote_branch_ref,
            ..
        } => format!(
            "Enter the new local branch name. The branch will be created from {remote_branch_ref} and checked out."
        ),
        super_lazygit_core::InputPromptOperation::RenameBranch { current_name } => {
            format!("Enter the new name for {current_name}.")
        }
        super_lazygit_core::InputPromptOperation::EditRemote { current_name, .. } => {
            format!(
                "Edit remote {current_name} as: <name> <url>. Renaming and URL updates are applied together."
            )
        }
        super_lazygit_core::InputPromptOperation::RenameStash { stash_ref, .. } => {
            format!(
                "Enter the new message for {stash_ref}. Leave it blank to use Git's default stash message."
            )
        }
        super_lazygit_core::InputPromptOperation::CreateBranchFromStash {
            stash_label, ..
        } => format!(
            "Enter the new branch name. The branch will be created from '{stash_label}', checked out, and the stash will be dropped if it applies cleanly."
        ),
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
        super_lazygit_core::InputPromptOperation::CreateSubmodule => {
            "Enter submodule details as: <path> <url>. Example: vendor/lib ../lib.git."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::ShellCommand => {
            "Enter a shell command to run in the current repository root. The terminal suspends until the command exits."
                .to_string()
        }
        super_lazygit_core::InputPromptOperation::EditSubmoduleUrl { name, .. } => {
            format!(
                "Enter the new URL for submodule {name}. This updates both .gitmodules and local submodule config."
            )
        }
        super_lazygit_core::InputPromptOperation::CreateAmendCommit {
            summary,
            include_file_changes,
            ..
        } => {
            if *include_file_changes {
                format!(
                    "Enter the replacement subject line for the amend! commit targeting {summary}. Staged changes stay attached to the amend! commit."
                )
            } else {
                format!(
                    "Enter the replacement subject line for the amend! commit targeting {summary}. This creates a message-only amend! commit with no file changes."
                )
            }
        }
        super_lazygit_core::InputPromptOperation::SetCommitCoAuthor { summary, .. } => {
            format!(
                "Enter the co-author for {summary} as: Name <email@example.com>. This amends the selected commit without changing its subject."
            )
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
        super_lazygit_core::MenuOperation::FilterOptions => {
            "Open the shipped text-filter flows for the current detail panel."
        }
        super_lazygit_core::MenuOperation::DiffOptions => {
            "Open the shipped comparison flows for the current branch, commit, or compare context."
        }
        super_lazygit_core::MenuOperation::CommitLogOptions => {
            "Switch the commits panel between current-branch history and the whole-repository graph."
        }
        super_lazygit_core::MenuOperation::BranchGitFlowOptions => {
            "Run one of the shipped git-flow finish commands for the selected branch."
        }
        super_lazygit_core::MenuOperation::BranchPullRequestOptions => {
            "Open or copy the browser pull request URL for the selected branch."
        }
        super_lazygit_core::MenuOperation::BranchResetOptions => {
            "Choose how aggressively to reset the current branch to the selected branch ref."
        }
        super_lazygit_core::MenuOperation::BranchSortOptions => {
            "Choose how the branch list should be ordered in this repository view."
        }
        super_lazygit_core::MenuOperation::CommitCopyOptions => {
            "Choose which selected commit attribute to copy to the clipboard."
        }
        super_lazygit_core::MenuOperation::TagResetOptions => {
            "Choose how aggressively to reset the current branch to the selected tag."
        }
        super_lazygit_core::MenuOperation::ReflogResetOptions => {
            "Choose how aggressively to reset the current branch to the selected reflog entry."
        }
        super_lazygit_core::MenuOperation::CommitAmendAttributeOptions => {
            "Choose which amend-only metadata change to apply to the selected commit."
        }
        super_lazygit_core::MenuOperation::CommitFixupOptions => {
            "Choose whether to create a fixup! commit or one of the amend! variants for the selected commit."
        }
        super_lazygit_core::MenuOperation::BisectOptions => {
            "Open the shipped bisect flows for the selected commit or the active bisect candidate."
        }
        super_lazygit_core::MenuOperation::BranchUpstreamOptions => {
            "Choose whether to set, unset, or fast-forward the selected branch's upstream relationship."
        }
        super_lazygit_core::MenuOperation::MergeRebaseOptions => {
            "Open the shipped merge/rebase flows that make sense in the current repository context."
        }
        super_lazygit_core::MenuOperation::IgnoreOptions => {
            "Choose whether the selected path should go to the shared ignore list or the local exclude file."
        }
        super_lazygit_core::MenuOperation::StatusResetOptions => {
            "Choose how aggressively to reset the current branch against its configured upstream."
        }
        super_lazygit_core::MenuOperation::RemoteBranchPullRequestOptions => {
            "Open or copy the browser pull request URL for the selected remote branch."
        }
        super_lazygit_core::MenuOperation::RemoteBranchResetOptions => {
            "Choose how aggressively to reset the current branch to the selected remote branch ref."
        }
        super_lazygit_core::MenuOperation::RemoteBranchSortOptions => {
            "Choose how the remote-branch list should be ordered in this repository view."
        }
        super_lazygit_core::MenuOperation::PatchOptions => {
            "Jump directly into the shipped hunk/line patch flows for the current status diff."
        }
        super_lazygit_core::MenuOperation::SubmoduleOptions => {
            "Choose a lifecycle or clipboard action for the selected submodule."
        }
        super_lazygit_core::MenuOperation::RecentRepos => {
            "Switch directly to one of the repositories you visited recently."
        }
        super_lazygit_core::MenuOperation::CommandLog => {
            "Review recent command and status messages recorded in this session."
        }
    }
}

fn menu_lines(
    state: &AppState,
    menu: &super_lazygit_core::PendingMenu,
    theme: Theme,
) -> Vec<Line<'static>> {
    let items: Vec<String> = match menu.operation {
        super_lazygit_core::MenuOperation::StashOptions => vec![
            "Stash tracked changes".to_string(),
            "Stash tracked changes and keep staged changes".to_string(),
            "Stash all changes including untracked".to_string(),
            "Stash staged changes".to_string(),
            "Stash unstaged changes".to_string(),
        ],
        super_lazygit_core::MenuOperation::FilterOptions => filter_menu_lines(state),
        super_lazygit_core::MenuOperation::DiffOptions => diff_menu_lines(state),
        super_lazygit_core::MenuOperation::CommitLogOptions => commit_log_menu_lines(state),
        super_lazygit_core::MenuOperation::BranchGitFlowOptions => {
            branch_git_flow_menu_lines(state)
        }
        super_lazygit_core::MenuOperation::BranchPullRequestOptions => {
            branch_pull_request_menu_lines(state)
        }
        super_lazygit_core::MenuOperation::BranchResetOptions => branch_reset_menu_lines(state),
        super_lazygit_core::MenuOperation::BranchSortOptions => branch_sort_menu_lines(state),
        super_lazygit_core::MenuOperation::CommitCopyOptions => vec![
            "Copy short hash".to_string(),
            "Copy full hash".to_string(),
            "Copy summary".to_string(),
            "Copy diff".to_string(),
            "Copy browser URL".to_string(),
        ],
        super_lazygit_core::MenuOperation::TagResetOptions => vec![
            "Soft reset to selected tag".to_string(),
            "Mixed reset to selected tag".to_string(),
            "Hard reset to selected tag".to_string(),
        ],
        super_lazygit_core::MenuOperation::ReflogResetOptions => vec![
            "Soft reset to selected reflog target".to_string(),
            "Mixed reset to selected reflog target".to_string(),
            "Hard reset to selected reflog target".to_string(),
        ],
        super_lazygit_core::MenuOperation::CommitAmendAttributeOptions => {
            vec!["Reset author".to_string(), "Set co-author".to_string()]
        }
        super_lazygit_core::MenuOperation::CommitFixupOptions => vec![
            "Create fixup! commit".to_string(),
            "Create amend! commit with staged changes".to_string(),
            "Create amend! commit without file changes".to_string(),
        ],
        super_lazygit_core::MenuOperation::BisectOptions => bisect_menu_lines(state),
        super_lazygit_core::MenuOperation::BranchUpstreamOptions => vec![
            "Set upstream...".to_string(),
            "Unset upstream".to_string(),
            "Fast-forward current branch from upstream".to_string(),
        ],
        super_lazygit_core::MenuOperation::MergeRebaseOptions => merge_rebase_menu_lines(state),
        super_lazygit_core::MenuOperation::IgnoreOptions => vec![
            "Add selected path to .gitignore".to_string(),
            "Add selected path to .git/info/exclude".to_string(),
        ],
        super_lazygit_core::MenuOperation::StatusResetOptions => status_reset_menu_lines(state),
        super_lazygit_core::MenuOperation::RemoteBranchPullRequestOptions => {
            remote_branch_pull_request_menu_lines(state)
        }
        super_lazygit_core::MenuOperation::RemoteBranchResetOptions => {
            remote_branch_reset_menu_lines(state)
        }
        super_lazygit_core::MenuOperation::RemoteBranchSortOptions => {
            remote_branch_sort_menu_lines(state)
        }
        super_lazygit_core::MenuOperation::PatchOptions => patch_menu_lines(state),
        super_lazygit_core::MenuOperation::SubmoduleOptions => vec![
            "Copy selected submodule".to_string(),
            "Open selected submodule in editor".to_string(),
            "Edit selected submodule URL".to_string(),
            "Initialize selected submodule".to_string(),
            "Update selected submodule".to_string(),
            "Remove selected submodule".to_string(),
        ],
        super_lazygit_core::MenuOperation::RecentRepos => recent_repo_menu_lines(state),
        super_lazygit_core::MenuOperation::CommandLog => command_log_menu_lines(state),
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

fn status_bar_height(state: &AppState) -> u16 {
    if !state.settings.show_help_footer {
        return 1;
    }

    if matches!(state.mode, AppMode::Repository) && state.modal_stack.is_empty() {
        3
    } else {
        2
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

fn repo_screen_status_line(state: &AppState) -> String {
    format!(
        "Screen mode {}  + next screen  _ previous screen  Ctrl+Z suspend terminal",
        state.settings.screen_mode.label()
    )
}

fn recent_repo_menu_lines(state: &AppState) -> Vec<String> {
    let current_repo_id = state
        .repo_mode
        .as_ref()
        .map(|repo_mode| &repo_mode.current_repo_id)
        .or(state.workspace.selected_repo_id.as_ref());
    let mut entries = Vec::new();
    for repo_id in state.recent_repo_stack.iter().rev() {
        if current_repo_id.is_some_and(|current| current == repo_id) {
            continue;
        }
        if entries
            .iter()
            .any(|line: &String| line.ends_with(&repo_id.0))
        {
            continue;
        }
        let label = state
            .workspace
            .repo_summaries
            .get(repo_id)
            .map(|summary| format!("{}  {}", summary.display_name, summary.display_path))
            .unwrap_or_else(|| repo_id.0.clone());
        entries.push(label);
    }
    entries
}

fn command_log_menu_lines(state: &AppState) -> Vec<String> {
    state
        .status_messages
        .iter()
        .rev()
        .map(|message| format!("[{:?}] {}", message.level, message.text))
        .collect()
}

fn filter_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(filter) = repo_mode.subview_filter(repo_mode.active_subview) else {
        return Vec::new();
    };

    let subject = filter_menu_subject(repo_mode);
    let mut entries = vec![if filter.query.trim().is_empty() {
        format!("Focus {subject} filter")
    } else {
        format!("Edit {subject} filter (/{})", filter.query)
    }];

    if !filter.query.trim().is_empty() {
        entries.push(format!("Clear {subject} filter"));
    }

    entries
}

fn filter_menu_subject(repo_mode: &RepoModeState) -> &'static str {
    match repo_mode.active_subview {
        RepoSubview::Branches => "branch list",
        RepoSubview::Remotes => "remote list",
        RepoSubview::RemoteBranches => "remote-branch list",
        RepoSubview::Tags => "tag list",
        RepoSubview::Commits => match repo_mode.commit_subview_mode {
            CommitSubviewMode::History => "commit history",
            CommitSubviewMode::Files => "commit-file list",
        },
        RepoSubview::Stash => "stash list",
        RepoSubview::Reflog => "reflog list",
        RepoSubview::Worktrees => "worktree list",
        RepoSubview::Submodules => "submodule list",
        RepoSubview::Status | RepoSubview::Compare | RepoSubview::Rebase => "detail panel",
    }
}

fn diff_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    if matches!(
        repo_mode.active_subview,
        RepoSubview::Branches | RepoSubview::Commits
    ) {
        if let Some(target) = diff_menu_selected_target(repo_mode) {
            let label = diff_menu_target_label(&target);
            let same_source = repo_mode.comparison_source == Some(repo_mode.active_subview);
            if repo_mode.comparison_base.is_none() || !same_source {
                entries.push(format!("Mark selected {label} as comparison base"));
            } else if repo_mode.comparison_base.as_ref() != Some(&target)
                || repo_mode.comparison_target.as_ref() != Some(&target)
            {
                entries.push(format!("Compare current base against selected {label}"));
            }
        }
    }

    if repo_mode.comparison_base.is_some() && repo_mode.comparison_target.is_some() {
        if repo_mode.active_subview != RepoSubview::Compare {
            entries.push("Open comparison diff".to_string());
        }
        if matches!(
            repo_mode.active_subview,
            RepoSubview::Branches | RepoSubview::Commits | RepoSubview::Compare
        ) {
            entries.push("Clear comparison".to_string());
        }
    }

    entries.push(format!(
        "{} whitespace changes in diff",
        if repo_mode.ignore_whitespace_in_diff {
            "Show"
        } else {
            "Ignore"
        }
    ));
    entries.push(format!(
        "Increase diff context (currently {} line{})",
        repo_mode.diff_context_lines,
        if repo_mode.diff_context_lines == 1 {
            ""
        } else {
            "s"
        }
    ));
    entries.push(format!(
        "Decrease diff context (currently {} line{})",
        repo_mode.diff_context_lines,
        if repo_mode.diff_context_lines == 1 {
            ""
        } else {
            "s"
        }
    ));
    entries.push(format!(
        "Increase rename similarity threshold (currently {}%)",
        repo_mode.rename_similarity_threshold
    ));
    entries.push(format!(
        "Decrease rename similarity threshold (currently {}%)",
        repo_mode.rename_similarity_threshold
    ));

    entries
}

fn commit_log_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };

    if repo_mode.active_subview != RepoSubview::Commits
        || repo_mode.commit_subview_mode != CommitSubviewMode::History
    {
        return Vec::new();
    }

    vec![
        "Show current branch history".to_string(),
        "Show whole git graph (newest first)".to_string(),
        "Show whole git graph (oldest first)".to_string(),
    ]
}

fn branch_git_flow_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch(
        repo_mode.detail.as_ref(),
        repo_mode.branches_view.selected_index,
    ) else {
        return Vec::new();
    };
    vec![
        format!("git flow feature finish {}", branch.name),
        format!("git flow release finish {}", branch.name),
        format!("git flow hotfix finish {}", branch.name),
    ]
}

fn branch_pull_request_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch(
        repo_mode.detail.as_ref(),
        repo_mode.branches_view.selected_index,
    ) else {
        return Vec::new();
    };
    vec![
        format!("Open pull request for {}", branch.name),
        format!("Copy pull request URL for {}", branch.name),
    ]
}

fn branch_reset_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_branch(
        repo_mode.detail.as_ref(),
        repo_mode.branches_view.selected_index,
    ) else {
        return Vec::new();
    };
    vec![
        format!("Soft reset to {}", branch.name),
        format!("Mixed reset to {}", branch.name),
        format!("Hard reset to {}", branch.name),
    ]
}

fn branch_sort_menu_lines(state: &AppState) -> Vec<String> {
    let current = state
        .repo_mode
        .as_ref()
        .map_or(super_lazygit_core::BranchSortMode::Natural, |repo_mode| {
            repo_mode.branch_sort_mode
        });
    vec![
        format!(
            "Natural order{}",
            if current == super_lazygit_core::BranchSortMode::Natural {
                " (current)"
            } else {
                ""
            }
        ),
        format!(
            "Sort by branch name{}",
            if current == super_lazygit_core::BranchSortMode::Name {
                " (current)"
            } else {
                ""
            }
        ),
    ]
}

fn diff_menu_selected_target(
    repo_mode: &RepoModeState,
) -> Option<super_lazygit_core::ComparisonTarget> {
    match repo_mode.active_subview {
        RepoSubview::Branches => selected_branch(
            repo_mode.detail.as_ref(),
            repo_mode.branches_view.selected_index,
        )
        .map(|branch| super_lazygit_core::ComparisonTarget::Branch(branch.name.clone())),
        RepoSubview::Commits => {
            let detail = repo_mode.detail.as_ref()?;
            let visible_indices = visible_commit_indices(detail, &repo_mode.commits_filter.query);
            let selected_index = repo_mode
                .commits_view
                .selected_index
                .filter(|index| visible_indices.contains(index))
                .unwrap_or(*visible_indices.first()?);
            detail
                .commits
                .get(selected_index)
                .map(|commit| super_lazygit_core::ComparisonTarget::Commit(commit.oid.clone()))
        }
        _ => None,
    }
}

fn diff_menu_target_label(target: &super_lazygit_core::ComparisonTarget) -> String {
    match target {
        super_lazygit_core::ComparisonTarget::Branch(name) => format!("branch '{name}'"),
        super_lazygit_core::ComparisonTarget::Commit(oid) => {
            format!("commit {}", oid.chars().take(8).collect::<String>())
        }
    }
}

fn merge_rebase_menu_lines(state: &AppState) -> Vec<String> {
    let mut entries = Vec::new();
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return entries;
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return entries;
    };

    if detail.merge_state == super_lazygit_core::MergeState::RebaseInProgress
        && detail.rebase_state.is_some()
    {
        entries.extend([
            "Continue active rebase".to_string(),
            "Skip current rebase step".to_string(),
            "Abort active rebase".to_string(),
        ]);
    }

    if repo_mode.active_subview == RepoSubview::Commits
        && repo_mode.commit_subview_mode == CommitSubviewMode::History
    {
        entries.extend([
            "Interactive rebase from selected commit".to_string(),
            "Amend older commit at selection".to_string(),
            "Create fixup commit for selected commit".to_string(),
            "Fixup onto selected commit".to_string(),
            "Apply pending fixup/squash commits".to_string(),
            "Squash selected commit into its parent".to_string(),
            "Drop selected commit".to_string(),
            "Move selected commit up".to_string(),
            "Move selected commit down".to_string(),
            "Reword selected commit".to_string(),
            "Reword selected commit in editor".to_string(),
        ]);
    }

    entries.push(
        if detail.merge_state == super_lazygit_core::MergeState::None {
            "Open rebase / merge panel".to_string()
        } else {
            "Open rebase / merge status panel".to_string()
        },
    );

    entries
}

fn remote_branch_pull_request_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_remote_branch(
        repo_mode.detail.as_ref(),
        repo_mode.remote_branches_view.selected_index,
    ) else {
        return Vec::new();
    };
    vec![
        format!("Open pull request for {}", branch.name),
        format!("Copy pull request URL for {}", branch.name),
    ]
}

fn remote_branch_reset_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(branch) = selected_remote_branch(
        repo_mode.detail.as_ref(),
        repo_mode.remote_branches_view.selected_index,
    ) else {
        return Vec::new();
    };
    vec![
        format!("Soft reset to {}", branch.name),
        format!("Mixed reset to {}", branch.name),
        format!("Hard reset to {}", branch.name),
    ]
}

fn remote_branch_sort_menu_lines(state: &AppState) -> Vec<String> {
    let current = state.repo_mode.as_ref().map_or(
        super_lazygit_core::RemoteBranchSortMode::Natural,
        |repo_mode| repo_mode.remote_branch_sort_mode,
    );
    vec![
        format!(
            "Natural order{}",
            if current == super_lazygit_core::RemoteBranchSortMode::Natural {
                " (current)"
            } else {
                ""
            }
        ),
        format!(
            "Sort by branch name{}",
            if current == super_lazygit_core::RemoteBranchSortMode::Name {
                " (current)"
            } else {
                ""
            }
        ),
    ]
}

fn bisect_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    if let Some(bisect) = detail.bisect_state.as_ref() {
        let target_label = if bisect.current_commit.is_some() {
            "current bisect commit"
        } else {
            "selected commit"
        };
        vec![
            format!("Mark {target_label} as {}", bisect.bad_term),
            format!("Mark {target_label} as {}", bisect.good_term),
            format!("Skip {target_label}"),
            "Reset active bisect".to_string(),
        ]
    } else {
        vec![
            "Start bisect by marking selected commit as bad".to_string(),
            "Start bisect by marking selected commit as good".to_string(),
        ]
    }
}

fn status_reset_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(summary) = state
        .workspace
        .repo_summaries
        .get(&repo_mode.current_repo_id)
    else {
        return Vec::new();
    };
    let Some(tracking_branch) = summary.remote_summary.tracking_branch.as_deref() else {
        return Vec::new();
    };
    vec![
        format!("Soft reset to {tracking_branch}"),
        format!("Mixed reset to {tracking_branch}"),
        format!("Hard reset to {tracking_branch}"),
    ]
}

fn patch_menu_lines(state: &AppState) -> Vec<String> {
    let Some(repo_mode) = state.repo_mode.as_ref() else {
        return Vec::new();
    };
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };
    if detail.diff.selected_path.is_none() || detail.diff.selected_hunk.is_none() {
        return Vec::new();
    }

    let mut entries = Vec::new();
    match detail.diff.presentation {
        DiffPresentation::Unstaged => {
            entries.push("Stage selected hunk".to_string());
            if repo_mode.diff_line_cursor.is_some() {
                entries.push("Stage selected line range".to_string());
            }
        }
        DiffPresentation::Staged => {
            entries.push("Unstage selected hunk".to_string());
            if repo_mode.diff_line_cursor.is_some() {
                entries.push("Unstage selected line range".to_string());
            }
        }
        DiffPresentation::Comparison => {}
    }

    entries
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
    repo_mode: Option<&RepoModeState>,
    is_focused: bool,
    progress: &super_lazygit_core::OperationProgress,
) -> Vec<Line<'static>> {
    let mut lines = repo_status_section_lines(repo_mode, is_focused, FileStatusSection::Unstaged);
    lines.push(Line::from(format!(
        "Progress: {}",
        operation_progress_label(progress)
    )));
    lines
}

fn repo_staged_lines(repo_mode: Option<&RepoModeState>, is_focused: bool) -> Vec<Line<'static>> {
    repo_status_section_lines(repo_mode, is_focused, FileStatusSection::Staged)
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
    repo_mode: Option<&RepoModeState>,
    is_focused: bool,
    section: FileStatusSection,
) -> Vec<Line<'static>> {
    let pane = match section {
        FileStatusSection::Unstaged => PaneId::RepoUnstaged,
        FileStatusSection::Staged => PaneId::RepoStaged,
    };
    let (focus_text, empty_text) = match section {
        FileStatusSection::Unstaged => (
            if is_focused {
                "j/k move  Space stage file  Enter open diff  a stage all  ` tree  / filter"
            } else {
                "Move focus here to inspect working tree changes."
            },
            "No working tree changes.",
        ),
        FileStatusSection::Staged => (
            if is_focused {
                "j/k move  Space unstage file  Enter open diff  a unstage all  ` tree  / filter"
            } else {
                "Move focus here to prep staged work."
            },
            "No staged changes.",
        ),
    };

    let mut lines = vec![Line::from(focus_text)];
    let Some(repo_mode) = repo_mode else {
        lines.push(Line::from("Repository detail is still loading."));
        return lines;
    };
    let selected_index = match pane {
        PaneId::RepoUnstaged => repo_mode.status_view.selected_index,
        PaneId::RepoStaged => repo_mode.staged_view.selected_index,
        _ => None,
    };
    let entries = super_lazygit_core::visible_status_entries(repo_mode, pane);

    if entries.is_empty() {
        lines.push(Line::from(format!(
            "Tree: {}  Status: {}  Filter: /{}",
            if repo_mode.status_tree_enabled {
                "on"
            } else {
                "off"
            },
            repo_mode.status_filter_mode.label(),
            repo_mode.status_filter.query
        )));
        lines.push(Line::from(empty_text));
        return lines;
    }

    lines.push(Line::from(format!(
        "Files: {}  Tree: {}  Status: {}  Filter: /{}",
        entries.len(),
        if repo_mode.status_tree_enabled {
            "on"
        } else {
            "off"
        },
        repo_mode.status_filter_mode.label(),
        repo_mode.status_filter.query
    )));
    lines.extend(entries.into_iter().enumerate().map(|(index, entry)| {
        let marker = if selected_index == Some(index) {
            ">"
        } else {
            " "
        };
        let indent = "  ".repeat(entry.depth);
        let kind = entry.kind.map(file_status_kind_label).unwrap_or(" ");
        let label = match entry.entry_kind {
            super_lazygit_core::VisibleStatusEntryKind::Directory { collapsed } => {
                format!(
                    "{}{} {}/",
                    if collapsed { "▸" } else { "▾" },
                    indent,
                    entry.label
                )
            }
            super_lazygit_core::VisibleStatusEntryKind::File => format!("{indent}{}", entry.label),
        };
        Line::from(format!("{marker} {kind} {label}"))
    }));
    lines
}

fn repo_subview_label(subview: RepoSubview) -> &'static str {
    match subview {
        RepoSubview::Status => "Status",
        RepoSubview::Branches => "Branches",
        RepoSubview::Remotes => "Remotes",
        RepoSubview::RemoteBranches => "Remote Branches",
        RepoSubview::Tags => "Tags",
        RepoSubview::Commits => "Commits",
        RepoSubview::Compare => "Compare",
        RepoSubview::Rebase => "Rebase",
        RepoSubview::Stash => "Stash",
        RepoSubview::Reflog => "Reflog",
        RepoSubview::Worktrees => "Worktrees",
        RepoSubview::Submodules => "Submodules",
    }
}

fn repo_subview_tabs(active: RepoSubview) -> Vec<Span<'static>> {
    let all = [
        (RepoSubview::Status, "1 Status"),
        (RepoSubview::Branches, "2 Branches"),
        (RepoSubview::Remotes, "m Remotes"),
        (RepoSubview::RemoteBranches, "9 Remote"),
        (RepoSubview::Tags, "t Tags"),
        (RepoSubview::Commits, "3 Commits"),
        (RepoSubview::Compare, "4 Compare"),
        (RepoSubview::Rebase, "5 Rebase"),
        (RepoSubview::Stash, "6 Stash"),
        (RepoSubview::Reflog, "7 Reflog"),
        (RepoSubview::Worktrees, "8 Worktrees"),
        (RepoSubview::Submodules, "b Submodules"),
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

            if state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.subview_filter(repo_mode.active_subview))
                .is_some_and(|filter| filter.focused)
            {
                return "Detail filter focused; type to narrow the current panel, Enter keeps the query, and Esc clears it."
                    .to_string();
            }

            match state.focused_pane {
            PaneId::RepoUnstaged => {
                "Working tree focus; j/k move, Space stages, Enter opens the diff, Ctrl+B cycles status filters, ` toggles the tree, -/= collapse or expand directories, a stages all, i opens ignore options, o opens the path, y copies the path, Ctrl+T opens the difftool, g opens reset options, M opens merge options, / filters, and 0 jumps to the main pane."
                    .to_string()
            }
            PaneId::RepoStaged => {
                "Staged focus; j/k move, Space unstages, Enter opens the diff, Ctrl+B cycles status filters, ` toggles the tree, -/= collapse or expand directories, a unstages all, i opens ignore options, o opens the path, y copies the path, Ctrl+T opens the difftool, g opens reset options, M opens merge options, / filters, 0 jumps to the main pane, c commits, w commits without hooks, and A amends HEAD."
                    .to_string()
            }
            PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
                || "Repository shell ready.".to_string(),
                |repo_mode| {
                    if repo_mode.active_subview == RepoSubview::Status {
                        status_detail_focus_help(repo_mode)
                    } else if repo_mode.active_subview == RepoSubview::Branches {
                        "Branches detail focus; Enter opens commits, Space checks out, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, W/Ctrl+E opens diff options, w opens worktrees, b opens submodules, v compares refs, x clears compare, c creates, R renames, d deletes, u opens upstream options, y copies, r rebases current onto the selected branch, and M merges the selected branch into the current branch."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Remotes {
                        "Remotes detail focus; Enter opens remote branches, f fetches the selected remote, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, w opens worktrees, b opens submodules, and n/e/d manage remotes."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::RemoteBranches {
                        "Remote branches detail focus; Enter opens commits, Space checks out, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, w opens worktrees, b opens submodules, n creates a local branch, d deletes the selected remote branch, y copies, u sets the current branch upstream, r rebases the current branch onto the selected remote branch, and M merges the selected remote branch into the current branch."
                            .to_string()
                } else if repo_mode.active_subview == RepoSubview::Commits {
                        "Commits detail focus; 3 returns to current-branch history, Ctrl+L opens log options, Ctrl+W toggles whitespace, {/} change diff context, (/) change rename similarity, 0 returns to the main pane, / filters history, Ctrl+S opens filter options, W/Ctrl+E opens diff options, w opens worktrees, b opens bisect options, n branches off the selected commit, T tags it, i starts a rebase, m opens merge/rebase options, A amends with staged changes, a opens amend attribute options, f opens fixup options, F fixups, g applies fixups, s squashes, d drops, Ctrl+K/Ctrl+J move the selected commit, y opens copy options, C cherry-picks, V pastes the copied commit, t reverts, S/M/H reset HEAD, v compares commits, and x clears compare."
                            .to_string()
                } else if repo_mode.active_subview == RepoSubview::Compare {
                        "Compare detail focus; j/k scroll the comparison diff, Ctrl+W toggles whitespace, {/} change diff context, (/) change rename similarity, W/Ctrl+E opens diff options, 0 returns to the main pane, and x clears compare."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Rebase {
                        "Rebase detail focus; c continues, s skips, A aborts, 0 returns to the main pane, and j/k scroll the active step."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Stash {
                        match repo_mode.stash_subview_mode {
                            StashSubviewMode::List => {
                                "Stash detail focus; Enter opens changed files, Space applies, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, w opens worktrees, b opens submodules, and g/d manage the selected stash."
                                    .to_string()
                            }
                            StashSubviewMode::Files => {
                                "Stash files focus; Enter returns to the stash list, 0 returns to the main pane, and w/b open worktrees or submodules. Apply/pop/drop/rename/new-branch stay on the stash list."
                                    .to_string()
                            }
                        }
                    } else if repo_mode.active_subview == RepoSubview::Reflog {
                        "Reflog detail focus; Enter opens commit history, Space detaches to the selected target, Ctrl+O copies the selected hash, o opens the selected target in the browser, n branches off it, T tags it, C cherry-picks, g opens reset options, S/M/H reset via the selector, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, w opens worktrees, b opens submodules, and u preserves the explicit restore flow."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Worktrees {
                        "Worktrees detail focus; Enter/Space switches worktrees, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, b opens submodules, and n/o/d manage the selected worktree."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Submodules {
                        "Submodules detail focus; Enter opens the selected nested repo, Ctrl+O copies the selected submodule name, b opens the submodule options menu, Esc returns to the parent repo, 0 returns to the main pane, / filters this panel, Ctrl+S opens filter options, and n/e/i/u/o/d manage the selected submodule."
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

fn status_detail_focus_help(repo_mode: &RepoModeState) -> String {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return "Repository shell ready.".to_string();
    };

    let action_copy = match detail.diff.presentation {
        DiffPresentation::Unstaged if detail.merge_state == super_lazygit_core::MergeState::None => {
            "Enter/Space stages the current hunk and L stages the selected line range"
        }
        DiffPresentation::Unstaged => {
            "Enter/Space stages the current hunk for merge resolution and L stages the selected line range"
        }
        DiffPresentation::Staged => {
            "Enter/Space unstages the current hunk and L unstages the selected line range"
        }
        DiffPresentation::Comparison => {
            "the current diff is read-only while j/k change hunks and Down/Up scroll"
        }
    };

    format!(
        "Status diff focus; {action_copy}, Ctrl+W toggles whitespace, {{/}} change diff context, (/) change rename similarity, W/Ctrl+E opens diff options, a/A open the all-branches graph, o opens the config file, e edits it, u checks for updates, Esc or 0 returns to the main pane, w opens worktrees, b opens submodules, D discards the current file, and X nukes the working tree."
    )
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

    if let Some(filter) = state
        .repo_mode
        .as_ref()
        .and_then(|repo_mode| repo_mode.subview_filter(repo_mode.active_subview))
        .filter(|filter| filter.focused)
    {
        return format!(
            "Detail filter  type to filter  Paste insert  Backspace delete  Enter keep  Esc clear  query=/{}",
            filter.query
        );
    }

    match state.focused_pane {
        PaneId::RepoUnstaged => {
            "Working tree pane  j/k move  ,/. page  </> top/bottom  Space stage file  Enter main diff  a stage all  Ctrl+B status filter  ` tree  -/= collapse/expand  / text filter  i ignore/exclude menu  y/Ctrl+O copy path  o open path  Ctrl+T external difftool  g reset menu  M merge menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  s stash tracked changes  S stash options  d/D discard file  0 main pane  l next pane  [/] detail tabs  1-9/t/m/b detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoStaged => {
            "Staged pane  j/k move  ,/. page  </> top/bottom  Space unstage file  Enter main diff  a unstage all  Ctrl+B status filter  ` tree  -/= collapse/expand  / text filter  i ignore/exclude menu  y/Ctrl+O copy path  o open path  Ctrl+T external difftool  g reset menu  M merge menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  s stash tracked changes  S stash options  d/D discard file  c commit  A amend HEAD  0 main pane  h/l change pane  [/] detail tabs  1-9/t/m/b detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
            || "Repository shell".to_string(),
            |repo_mode| {
                if repo_mode.active_subview == RepoSubview::Status {
                    "Status diff pane  j/k scroll diff  Ctrl+W whitespace  {/} context  (/) rename similarity  W/Ctrl+E diff menu  Enter apply hunk  Ctrl+P patch menu  o open config  e edit config  u check updates  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  w worktrees  b submodules  D discard file  X nuke working tree  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Branches {
                    "Branches pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter commits  Space checkout  F force checkout  Ctrl+S filter menu  W/Ctrl+E diff menu  Ctrl+R recent repos  : shell  @ command log  0 main pane  / filter  w worktrees  b submodules  v compare  x clear compare  - previous  c create/checkout  R rename  d delete  u upstream menu  o pull request menu  g reset menu  s sort menu  G git-flow menu  y/Ctrl+O copy  r rebase current  M merge into current  T tag  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Remotes {
                    "Remotes pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter branches  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  f fetch remote  0 main pane  / filter  w worktrees  b submodules  n add  e edit  d remove  F fork remote  h left pane  1-9/t/m/b switch view  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::RemoteBranches {
                    "Remote branches pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter commits  Space checkout  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  0 main pane  / filter  w worktrees  b submodules  n local branch  d delete  o pull request menu  g reset menu  s sort menu  y/Ctrl+O copy  u set upstream  r rebase current  M merge into current  T tag  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Tags {
                    "Tags pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter commits  Space checkout  Ctrl+O copy tag  g reset menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  w worktrees  b submodules  n create  d delete  P push  S/M/H reset  h left pane  1-9/t/m/b switch view  f fetch  p pull  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Commits {
                    "Commits pane  j/k move commit  ,/. page  </> top/bottom  [/] tabs  Enter files  Space checkout  Ctrl+O copy hash  a amend attrs  y copy menu  o browser  C copy  V paste copied  t revert  Ctrl+R clear copied  3 current branch  Ctrl+L log menu  n branch  T tag  b bisect menu  i start rebase  A amend  f fixup menu  F fixup+autosquash  c set fixup msg  g apply-fixups  s squash  d drop  Ctrl+K move up  Ctrl+J move down  r reword  R reword editor  S soft reset  M mixed reset  H hard reset  v compare  x clear compare  Ctrl+W whitespace  {/} context  (/) rename similarity  Ctrl+S filter menu  W/Ctrl+E diff menu  m merge/rebase menu  : shell  @ command log  0 main pane  / filter  w worktrees  h left pane  1-9/t switch view  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Compare {
                    "Compare pane  j/k scroll diff  Ctrl+W whitespace  {/} context  (/) rename similarity  W/Ctrl+E diff menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  x clear compare  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Rebase {
                    "Rebase pane  c continue  s skip  A abort  m merge/rebase menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  j/k scroll  0 main pane  h left pane  1-9/t/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Stash {
                    match repo_mode.stash_subview_mode {
                        StashSubviewMode::List => {
                            "Stash pane  j/k move stash  ,/. page  </> top/bottom  [/] tabs  Enter files  Space apply  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  / filter  w worktrees  b submodules  n branch  g pop  d drop  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                        }
                        StashSubviewMode::Files => {
                            "Stash files pane  j/k move file  ,/. page  </> top/bottom  [/] tabs  Enter stash list  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  w worktrees  b submodules  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                        }
                    }
                } else if repo_mode.active_subview == RepoSubview::Reflog {
                    "Reflog pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter commits  Space checkout  Ctrl+O copy hash  o browser  g reset menu  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  n branch  C cherry-pick  S/M/H reset  u restore  0 main pane  / filter  w worktrees  b submodules  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Worktrees {
                    "Worktrees pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter switch  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  / filter  b submodules  n create  o open  d delete  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Submodules {
                    "Submodules pane  j/k move  ,/. page  </> top/bottom  [/] tabs  Enter nested repo  Ctrl+O copy submodule  b options menu  Ctrl+S filter menu  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  0 main pane  / filter  n add  e edit-url  i init  u update  o open  d remove  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else {
                    format!(
                        "{} detail pane  Ctrl+R recent repos  : shell  @ command log  r refresh  R full refresh  h left pane  1-9/t/m/b switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace",
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

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn sample_repo_detail() -> RepoDetail {
        RepoDetail {
            file_tree: vec![
                FileStatus {
                    path: PathBuf::from("src/ui/lib.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: Some(FileStatusKind::Modified),
                    unstaged_kind: Some(FileStatusKind::Modified),
                },
                FileStatus {
                    path: PathBuf::from("src/ui/mod.rs"),
                    kind: FileStatusKind::Modified,
                    staged_kind: None,
                    unstaged_kind: Some(FileStatusKind::Modified),
                },
                FileStatus {
                    path: PathBuf::from("docs/README.md"),
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
                selected_path: Some(PathBuf::from("src/ui/lib.rs")),
                presentation: DiffPresentation::Unstaged,
                lines: vec![
                    DiffLine {
                        kind: DiffLineKind::Meta,
                        content: "diff --git a/src/ui/lib.rs b/src/ui/lib.rs".to_string(),
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
            remotes: vec![
                super_lazygit_core::RemoteItem {
                    name: "origin".to_string(),
                    fetch_url: "/tmp/origin.git".to_string(),
                    push_url: "/tmp/origin.git".to_string(),
                    branch_count: 2,
                },
                super_lazygit_core::RemoteItem {
                    name: "upstream".to_string(),
                    fetch_url: "git@github.com:example/upstream.git".to_string(),
                    push_url: "git@github.com:example/upstream.git".to_string(),
                    branch_count: 0,
                },
            ],
            remote_branches: vec![
                super_lazygit_core::RemoteBranchItem {
                    name: "origin/main".to_string(),
                    remote_name: "origin".to_string(),
                    branch_name: "main".to_string(),
                },
                super_lazygit_core::RemoteBranchItem {
                    name: "origin/feature".to_string(),
                    remote_name: "origin".to_string(),
                    branch_name: "feature".to_string(),
                },
            ],
            tags: vec![
                super_lazygit_core::TagItem {
                    name: "v1.0.0".to_string(),
                    target_oid: "abcdef1234567890".to_string(),
                    target_short_oid: "abcdef1".to_string(),
                    summary: "release v1.0.0".to_string(),
                    annotated: true,
                },
                super_lazygit_core::TagItem {
                    name: "snapshot".to_string(),
                    target_oid: "1234567890abcdef".to_string(),
                    target_short_oid: "1234567".to_string(),
                    summary: "second".to_string(),
                    annotated: false,
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
            commit_graph_lines: vec![
                "* abcdef1 (HEAD -> main) add lib".to_string(),
                "| * 1234567 second".to_string(),
            ],
            stashes: vec![
                super_lazygit_core::StashItem {
                    stash_ref: "stash@{0}".to_string(),
                    label: "stash@{0}: WIP on main: fixture stash".to_string(),
                    changed_files: vec![
                        super_lazygit_core::CommitFileItem {
                            path: std::path::PathBuf::from("stash.txt"),
                            kind: super_lazygit_core::FileStatusKind::Modified,
                        },
                        super_lazygit_core::CommitFileItem {
                            path: std::path::PathBuf::from("stash-untracked.txt"),
                            kind: super_lazygit_core::FileStatusKind::Added,
                        },
                    ],
                },
                super_lazygit_core::StashItem {
                    stash_ref: "stash@{1}".to_string(),
                    label: "stash@{1}: On feature: prior experiment".to_string(),
                    changed_files: vec![
                        super_lazygit_core::CommitFileItem {
                            path: std::path::PathBuf::from("src/lib.rs"),
                            kind: super_lazygit_core::FileStatusKind::Modified,
                        },
                        super_lazygit_core::CommitFileItem {
                            path: std::path::PathBuf::from("docs/notes.md"),
                            kind: super_lazygit_core::FileStatusKind::Deleted,
                        },
                    ],
                },
            ],
            reflog_items: vec![
                super_lazygit_core::ReflogItem {
                    selector: "HEAD@{0}".to_string(),
                    oid: "abcdef1234567890".to_string(),
                    short_oid: "abcdef1".to_string(),
                    summary: "checkout: moving from feature to main".to_string(),
                    description: "HEAD@{0}: checkout: moving from feature to main".to_string(),
                },
                super_lazygit_core::ReflogItem {
                    selector: "HEAD@{1}".to_string(),
                    oid: "1234567890abcdef".to_string(),
                    short_oid: "1234567".to_string(),
                    summary: "commit: add repo-mode stash flows".to_string(),
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
            submodules: vec![
                super_lazygit_core::SubmoduleItem {
                    name: "child-module".to_string(),
                    path: PathBuf::from("vendor/child-module"),
                    url: "../child-module.git".to_string(),
                    branch: Some("main".to_string()),
                    short_oid: Some("fedcba9".to_string()),
                    initialized: true,
                    dirty: false,
                    conflicted: false,
                },
                super_lazygit_core::SubmoduleItem {
                    name: "ui-kit".to_string(),
                    path: PathBuf::from("vendor/ui-kit"),
                    url: "git@github.com:example/ui-kit.git".to_string(),
                    branch: None,
                    short_oid: None,
                    initialized: false,
                    dirty: false,
                    conflicted: false,
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
    fn route_repository_status_detail_escape_returns_to_last_main_pane() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Status,
                main_focus: PaneId::RepoStaged,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let escaped = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "esc".to_string(),
        })));

        assert_eq!(escaped.state.mode, AppMode::Repository);
        assert_eq!(escaped.state.focused_pane, PaneId::RepoStaged);
        assert!(escaped.state.repo_mode.is_some());
    }

    #[test]
    fn route_repository_filter_escape_cancels_filter_before_leaving_repo_mode() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                status_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "tracked".to_string(),
                    focused: true,
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let escaped = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "esc".to_string(),
        })));

        assert_eq!(escaped.state.mode, AppMode::Repository);
        assert_eq!(escaped.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            escaped
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.status_filter.query.as_str()),
            Some("")
        );
        assert_eq!(
            escaped
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.status_filter.focused),
            Some(false)
        );
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
    fn route_status_detail_config_bindings_open_and_edit_loaded_config() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let repo_root = std::path::PathBuf::from(&repo_id.0);
        let config_path = std::path::PathBuf::from("/tmp/configs/super-lazygit.toml");
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            config_path: Some(config_path.clone()),
            repository_url: Some("https://github.com/quangdang/super_lazygit_rust".to_string()),
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
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let open = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert!(matches!(
            open.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { job_id, repo_id: actual_repo_id, command }
            )]
                if job_id == &super_lazygit_core::JobId::new("shell:/tmp/repo-1:run-command")
                    && actual_repo_id == &repo_id
                    && command.contains("xdg-open")
                    && command.contains(config_path.to_string_lossy().as_ref())
        ));

        let edit = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "e".to_string(),
        })));
        assert_eq!(
            edit.effects,
            vec![super_lazygit_core::Effect::OpenEditor {
                cwd: repo_root.clone(),
                target: config_path,
            }]
        );

        let update = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "u".to_string(),
        })));
        assert!(matches!(
            update.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { job_id, repo_id: actual_repo_id, command }
            )]
                if job_id == &super_lazygit_core::JobId::new("shell:/tmp/repo-1:run-command")
                    && actual_repo_id == &repo_id
                    && command.contains("https://github.com/quangdang/super_lazygit_rust/releases")
        ));
    }

    #[test]
    fn route_repository_global_recent_repo_command_log_and_shell_prompt_keys() {
        let current_repo_id = RepoId::new("/tmp/current");
        let recent_repo_id = RepoId::new("/tmp/recent");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            recent_repo_stack: vec![recent_repo_id.clone(), current_repo_id.clone()],
            status_messages: std::collections::VecDeque::from([
                super_lazygit_core::StatusMessage::info(1, "Fetch complete"),
            ]),
            workspace: WorkspaceState {
                discovered_repo_ids: vec![recent_repo_id.clone(), current_repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([
                    (
                        recent_repo_id.clone(),
                        workspace_repo_summary(&recent_repo_id.0, "recent"),
                    ),
                    (
                        current_repo_id.clone(),
                        workspace_repo_summary(&current_repo_id.0, "current"),
                    ),
                ]),
                selected_repo_id: Some(current_repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: current_repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(current_repo_id.clone())
            }),
            ..Default::default()
        };

        let mut recent_app = TuiApp::new(state.clone(), AppConfig::default());
        let recent = recent_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+r".to_string(),
        })));
        assert_eq!(
            recent
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::RecentRepos)
        );

        let mut log_app = TuiApp::new(state.clone(), AppConfig::default());
        let log = log_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "@".to_string(),
        })));
        assert_eq!(
            log.state.pending_menu.as_ref().map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::CommandLog)
        );

        let mut shell_app = TuiApp::new(state, AppConfig::default());
        let shell = shell_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: ":".to_string(),
        })));
        assert_eq!(
            shell
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::ShellCommand)
        );
    }

    #[test]
    fn route_repository_screen_mode_keys_cycle_modes() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let half = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "+".to_string(),
        })));
        assert_eq!(
            half.state.settings.screen_mode,
            super_lazygit_core::ScreenMode::HalfScreen
        );

        let fullscreen = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "+".to_string(),
        })));
        assert_eq!(
            fullscreen.state.settings.screen_mode,
            super_lazygit_core::ScreenMode::FullScreen
        );

        let previous = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "_".to_string(),
        })));
        assert_eq!(
            previous.state.settings.screen_mode,
            super_lazygit_core::ScreenMode::HalfScreen
        );
    }

    #[test]
    fn route_repository_shared_navigation_keys_cover_pages_edges_and_tabs() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let detail_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                branches_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                detail: Some(RepoDetail {
                    branches: vec![
                        super_lazygit_core::BranchItem {
                            name: "alpha".to_string(),
                            is_head: true,
                            upstream: None,
                        },
                        super_lazygit_core::BranchItem {
                            name: "beta".to_string(),
                            is_head: false,
                            upstream: None,
                        },
                        super_lazygit_core::BranchItem {
                            name: "gamma".to_string(),
                            is_head: false,
                            upstream: None,
                        },
                    ],
                    ..RepoDetail::default()
                }),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };

        let mut page_app = TuiApp::new(detail_state.clone(), AppConfig::default());
        page_app.resize(100, 10);
        let paged = page_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "pagedown".to_string(),
        })));
        assert_eq!(
            paged
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(2)
        );

        let mut edge_app = TuiApp::new(detail_state.clone(), AppConfig::default());
        let last = edge_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "end".to_string(),
        })));
        assert_eq!(
            last.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(2)
        );

        let first = edge_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "home".to_string(),
        })));
        assert_eq!(
            first
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(0)
        );

        let mut tabs_app = TuiApp::new(detail_state, AppConfig::default());
        let next_tab = tabs_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "]".to_string(),
        })));
        assert_eq!(
            next_tab
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Remotes)
        );

        let previous_tab = tabs_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "[".to_string(),
        })));
        assert_eq!(
            previous_tab
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Branches)
        );

        let main_pane_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                status_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut main_pane_app = TuiApp::new(main_pane_state, AppConfig::default());
        let main_last = main_pane_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: ">".to_string(),
        })));
        assert_eq!(
            main_last
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.status_view.selected_index),
            Some(5)
        );
    }

    #[test]
    fn route_repository_commits_merge_rebase_menu_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "m".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::MergeRebaseOptions)
        );
    }

    #[test]
    fn route_repository_status_patch_menu_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                diff_line_cursor: Some(1),
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+p".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::PatchOptions)
        );
    }

    #[test]
    fn repo_status_section_lines_render_tree_metadata_and_nested_entries() {
        let mut repo_mode = RepoModeState::new(RepoId::new("/tmp/repo-1"));
        repo_mode.detail = Some(sample_repo_detail());
        repo_mode.status_filter_mode = super_lazygit_core::StatusFilterMode::TrackedOnly;
        repo_mode.status_filter.query = "ui".to_string();
        repo_mode.status_view.selected_index = Some(0);

        let lines = repo_status_section_lines(Some(&repo_mode), true, FileStatusSection::Unstaged)
            .iter()
            .map(line_text)
            .collect::<Vec<_>>();

        assert_eq!(lines[1], "Files: 4  Tree: on  Status: tracked  Filter: /ui");
        assert!(lines.iter().any(|line| line.contains("▾ src/")));
        assert!(lines.iter().any(|line| line.contains("▾   ui/")));
        assert!(lines.iter().any(|line| line.contains("lib.rs")));
        assert!(lines.iter().any(|line| line.contains("mod.rs")));
    }

    #[test]
    fn route_repository_status_tree_and_shell_keys() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let mut summary = workspace_repo_summary(&repo_id.0, "repo-1");
        summary.remote_summary.remote_name = Some("origin".to_string());
        summary.remote_summary.tracking_branch = Some("origin/main".to_string());
        let base_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(repo_id.clone(), summary)]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let file_state = AppState {
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("/tmp/repo-1"),
                active_subview: RepoSubview::Status,
                status_tree_enabled: false,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..base_state.clone()
        };

        let mut cycle_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let cycled = cycle_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+b".to_string(),
        })));
        assert_eq!(
            cycled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.status_filter_mode),
            Some(super_lazygit_core::StatusFilterMode::TrackedOnly)
        );

        let mut tree_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let toggled = tree_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "`".to_string(),
        })));
        assert_eq!(
            toggled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.status_tree_enabled),
            Some(false)
        );

        let mut ignore_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let ignored = ignore_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "i".to_string(),
        })));
        assert_eq!(
            ignored
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::IgnoreOptions)
        );

        let mut reset_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let reset = reset_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(
            reset.state.pending_menu.as_ref().map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::StatusResetOptions)
        );

        let mut copy_app = TuiApp::new(file_state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "y".to_string(),
        })));
        assert!(copied
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));

        let mut open_app = TuiApp::new(file_state.clone(), AppConfig::default());
        let opened = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert!(opened
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));

        let mut difftool_app = TuiApp::new(file_state, AppConfig::default());
        let difftool = difftool_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+t".to_string(),
        })));
        assert!(difftool
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));

        let mut enter_app = TuiApp::new(base_state, AppConfig::default());
        let opened_detail = enter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(opened_detail.state.focused_pane, PaneId::RepoUnstaged);
        assert!(opened_detail
            .state
            .repo_mode
            .as_ref()
            .is_some_and(|repo_mode| !repo_mode.collapsed_status_dirs.is_empty()));
    }

    #[test]
    fn route_repository_status_enter_opens_detail_in_flat_mode() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Status,
                status_tree_enabled: false,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
    }

    #[test]
    fn route_repository_commit_filter_menu_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+s".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::FilterOptions)
        );
    }

    #[test]
    fn route_repository_commit_log_options_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::History,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+l".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::CommitLogOptions)
        );
    }

    #[test]
    fn route_repository_commit_file_shell_keys() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::Files,
                commit_files_mode: CommitFilesMode::List,
                detail: Some(sample_repo_detail()),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut copy_app = TuiApp::new(state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "y".to_string(),
        })));
        assert!(copied
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));

        let mut open_app = TuiApp::new(state.clone(), AppConfig::default());
        let opened = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert!(opened
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));

        let mut difftool_app = TuiApp::new(state, AppConfig::default());
        let difftool = difftool_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+t".to_string(),
        })));
        assert!(difftool
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));
    }

    #[test]
    fn route_repository_commit_history_ctrl_o_copies_selected_hash() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+o".to_string(),
        })));

        assert!(result
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));
    }

    #[test]
    fn route_repository_commit_history_y_opens_copy_menu() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "y".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::CommitCopyOptions)
        );
    }

    #[test]
    fn route_repository_commit_history_a_opens_amend_attribute_menu() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "a".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::CommitAmendAttributeOptions)
        );
    }

    #[test]
    fn route_repository_commit_history_r_opens_reword_prompt() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "r".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::RewordCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
                initial_message: "second".to_string(),
            })
        );
    }

    #[test]
    fn route_repository_commit_history_c_opens_set_fixup_message_confirmation() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "c".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::SetFixupMessageForCommit {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                }
            )
        );
    }

    #[test]
    fn route_repository_commit_history_o_opens_selected_commit_in_browser() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(state, AppConfig::default());
        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));

        assert!(result
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunShellCommand(_))));
    }

    #[test]
    fn route_repository_commit_copy_flow_keys() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let base_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id,
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };

        let mut copy_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "C".to_string(),
        })));
        assert_eq!(
            copied
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.copied_commit.as_ref())
                .map(|commit| commit.short_oid.as_str()),
            Some("1234567")
        );

        let mut paste_app = TuiApp::new(copied.state.clone(), AppConfig::default());
        let pasted = paste_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "V".to_string(),
        })));
        assert_eq!(
            pasted
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::CherryPickCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut revert_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let reverted = revert_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "t".to_string(),
        })));
        assert!(matches!(
            reverted
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::RevertCommit { .. })
        ));

        let mut clear_app = TuiApp::new(copied.state, AppConfig::default());
        let cleared = clear_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+r".to_string(),
        })));
        assert!(cleared
            .state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.copied_commit.as_ref())
            .is_none());
    }

    #[test]
    fn route_repository_branch_diff_menu_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+e".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::DiffOptions)
        );
    }

    #[test]
    fn route_repository_status_diff_menu_key() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+e".to_string(),
        })));

        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::DiffOptions)
        );
    }

    #[test]
    fn route_repository_live_diff_setting_keys() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let toggled = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+w".to_string(),
        })));
        assert_eq!(
            toggled
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.ignore_whitespace_in_diff),
            Some(true)
        );

        let increased_context = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "}".to_string(),
        })));
        assert_eq!(
            increased_context
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_context_lines),
            Some(super_lazygit_core::DEFAULT_DIFF_CONTEXT_LINES + 1)
        );

        let increased_similarity = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: ")".to_string(),
        })));
        assert_eq!(
            increased_similarity
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.rename_similarity_threshold),
            Some(super_lazygit_core::DEFAULT_RENAME_SIMILARITY_THRESHOLD + 5)
        );
    }

    #[test]
    fn route_repository_status_uppercase_refresh_triggers_deep_refresh() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "R".to_string(),
        })));

        assert!(result
            .effects
            .iter()
            .any(|effect| matches!(effect, super_lazygit_core::Effect::StartRepoScan)));
    }

    #[test]
    fn render_menu_modal_shows_recent_repo_and_command_log_entries() {
        let current_repo_id = RepoId::new("/tmp/current");
        let recent_repo_id = RepoId::new("/tmp/recent");
        let mut recent_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                super_lazygit_core::ModalKind::Menu,
                "Recent repositories",
            )],
            pending_menu: Some(super_lazygit_core::PendingMenu {
                repo_id: current_repo_id.clone(),
                operation: super_lazygit_core::MenuOperation::RecentRepos,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            recent_repo_stack: vec![recent_repo_id.clone(), current_repo_id.clone()],
            workspace: WorkspaceState {
                discovered_repo_ids: vec![recent_repo_id.clone(), current_repo_id],
                repo_summaries: std::collections::BTreeMap::from([(
                    recent_repo_id.clone(),
                    workspace_repo_summary(&recent_repo_id.0, "recent-repo"),
                )]),
                selected_repo_id: Some(RepoId::new("/tmp/current")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState::new(RepoId::new("/tmp/current"))),
            ..Default::default()
        };
        let mut recent_app = TuiApp::new(recent_state.clone(), AppConfig::default());
        assert!(recent_app.render_to_string().contains("recent-repo"));

        recent_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Command log",
        )];
        recent_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: RepoId::new("/tmp/current"),
            operation: super_lazygit_core::MenuOperation::CommandLog,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        recent_state.status_messages =
            std::collections::VecDeque::from([super_lazygit_core::StatusMessage::info(
                1,
                "Ran fetch",
            )]);
        let mut log_app = TuiApp::new(recent_state, AppConfig::default());
        assert!(log_app.render_to_string().contains("Ran fetch"));
    }

    #[test]
    fn render_menu_modal_shows_filter_diff_bisect_merge_rebase_and_patch_entries() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let mut merge_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![super_lazygit_core::Modal::new(
                super_lazygit_core::ModalKind::Menu,
                "Filter options",
            )],
            pending_menu: Some(super_lazygit_core::PendingMenu {
                repo_id: repo_id.clone(),
                operation: super_lazygit_core::MenuOperation::FilterOptions,
                selected_index: 0,
                return_focus: PaneId::RepoDetail,
            }),
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };
        let mut filter_app = TuiApp::new(merge_state.clone(), AppConfig::default());
        let filter_render = filter_app.render_to_string();
        assert!(filter_render.contains("Focus commit history filter"));

        merge_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Diffing options",
        )];
        merge_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: repo_id.clone(),
            operation: super_lazygit_core::MenuOperation::DiffOptions,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        merge_state.repo_mode = Some(RepoModeState {
            current_repo_id: repo_id.clone(),
            active_subview: RepoSubview::Branches,
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(repo_id.clone())
        });
        let mut diff_app = TuiApp::new(merge_state.clone(), AppConfig::default());
        let diff_render = diff_app.render_to_string();
        assert!(diff_render.contains("Mark selected branch 'main' as comparison base"));
        assert!(diff_render.contains("Ignore whitespace changes in diff"));
        assert!(diff_render.contains("Increase diff context (currently 3 lines)"));
        assert!(diff_render.contains("Increase rename similarity threshold (currently 50%)"));

        merge_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Commit log options",
        )];
        merge_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: repo_id.clone(),
            operation: super_lazygit_core::MenuOperation::CommitLogOptions,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        merge_state.repo_mode = Some(RepoModeState {
            current_repo_id: repo_id.clone(),
            active_subview: RepoSubview::Commits,
            commit_subview_mode: CommitSubviewMode::History,
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(repo_id.clone())
        });
        let mut commit_log_app = TuiApp::new(merge_state.clone(), AppConfig::default());
        let commit_log_render = commit_log_app.render_to_string();
        assert!(commit_log_render.contains("Show current branch history"));
        assert!(commit_log_render.contains("Show whole git graph (newest first)"));
        assert!(commit_log_render.contains("Show whole git graph (oldest first)"));

        merge_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Bisect options",
        )];
        merge_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: repo_id.clone(),
            operation: super_lazygit_core::MenuOperation::BisectOptions,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        merge_state.repo_mode = Some(RepoModeState {
            current_repo_id: repo_id.clone(),
            active_subview: RepoSubview::Commits,
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(repo_id.clone())
        });
        let mut bisect_app = TuiApp::new(merge_state.clone(), AppConfig::default());
        let bisect_render = bisect_app.render_to_string();
        assert!(bisect_render.contains("Start bisect by marking selected commit as bad"));
        assert!(bisect_render.contains("Start bisect by marking selected commit as good"));

        merge_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Merge / rebase options",
        )];
        merge_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: repo_id.clone(),
            operation: super_lazygit_core::MenuOperation::MergeRebaseOptions,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        merge_state.repo_mode = Some(RepoModeState {
            current_repo_id: repo_id.clone(),
            active_subview: RepoSubview::Commits,
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(repo_id.clone())
        });
        let mut merge_app = TuiApp::new(merge_state.clone(), AppConfig::default());
        let merge_render = merge_app.render_to_string();
        assert!(merge_render.contains("Interactive rebase from selected commit"));
        assert!(merge_render.contains("Create fixup commit for selected commit"));
        assert!(merge_render.contains("Apply pending fixup/squash commits"));
        assert!(merge_render.contains("Squash selected commit into its parent"));
        assert!(merge_render.contains("Drop selected commit"));

        merge_state.modal_stack = vec![super_lazygit_core::Modal::new(
            super_lazygit_core::ModalKind::Menu,
            "Patch options",
        )];
        merge_state.pending_menu = Some(super_lazygit_core::PendingMenu {
            repo_id: RepoId::new("/tmp/repo-1"),
            operation: super_lazygit_core::MenuOperation::PatchOptions,
            selected_index: 0,
            return_focus: PaneId::RepoDetail,
        });
        merge_state.repo_mode = Some(RepoModeState {
            current_repo_id: RepoId::new("/tmp/repo-1"),
            active_subview: RepoSubview::Status,
            diff_line_cursor: Some(1),
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
        });
        let mut patch_app = TuiApp::new(merge_state, AppConfig::default());
        let patch_render = patch_app.render_to_string();
        assert!(patch_render.contains("Stage selected hunk"));
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
        let set_fixup_message = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::SetFixupMessageForCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            },
        );
        let move_up = confirmation_copy(&super_lazygit_core::ConfirmableOperation::MoveCommitUp {
            commit: "1234567890abcdef".to_string(),
            adjacent_commit: "fedcba0987654321".to_string(),
            summary: "1234567 second".to_string(),
            adjacent_summary: "fedcba0 add lib".to_string(),
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
        assert!(set_fixup_message.contains("fixup -C"));
        assert!(move_up.contains("swap those adjacent commits"));
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

        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "9".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result.state.repo_mode.expect("repo mode").active_subview,
            RepoSubview::RemoteBranches
        );

        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoUnstaged,
            repo_mode: Some(RepoModeState::new(RepoId::new("repo-1"))),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "t".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::RepoDetail);
        assert_eq!(
            result.state.repo_mode.expect("repo mode").active_subview,
            RepoSubview::Tags
        );
    }

    #[test]
    fn repo_mode_detail_contract_routes_filter_worktrees_and_main_return() {
        let repo_id = RepoId::new("repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Branches,
                main_focus: PaneId::RepoStaged,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };

        let mut filter_app = TuiApp::new(state, AppConfig::default());
        let filter_focus = filter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "/".to_string(),
        })));
        assert_eq!(
            filter_focus
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.focused),
            Some(true)
        );

        let mut paste_app = TuiApp::new(filter_focus.state, AppConfig::default());
        let pasted = paste_app.dispatch(Event::Input(InputEvent::Paste("fea".to_string())));
        assert_eq!(
            pasted
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.branches_filter.query.as_str()),
            Some("fea")
        );
        assert_eq!(
            pasted
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.branches_view.selected_index),
            Some(1)
        );

        let mut blur_app = TuiApp::new(pasted.state, AppConfig::default());
        let blurred = blur_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
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

        let mut commit_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let branch_commits = commit_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            branch_commits
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            branch_commits
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("feature")
        );

        let mut current_branch_app =
            TuiApp::new(branch_commits.state.clone(), AppConfig::default());
        let current_branch =
            current_branch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "3".to_string(),
            })));
        assert_eq!(
            current_branch
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Linear)
        );
        assert_eq!(
            current_branch
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            None
        );

        let mut checkout_app = TuiApp::new(blurred.state, AppConfig::default());
        let branch_checkout =
            checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "space".to_string(),
            })));
        assert_eq!(
            branch_checkout.effects,
            vec![super_lazygit_core::Effect::RunGitCommand(
                super_lazygit_core::GitCommandRequest {
                    job_id: super_lazygit_core::JobId::new("git:repo-1:checkout-branch"),
                    repo_id: repo_id.clone(),
                    command: super_lazygit_core::GitCommand::CheckoutBranch {
                        branch_ref: "feature".to_string(),
                    },
                }
            )]
        );

        let mut worktree_app = TuiApp::new(branch_commits.state, AppConfig::default());
        let worktrees = worktree_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "w".to_string(),
        })));
        assert_eq!(
            worktrees
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Worktrees)
        );
        assert_eq!(worktrees.state.focused_pane, PaneId::RepoDetail);

        let mut return_app = TuiApp::new(worktrees.state, AppConfig::default());
        let returned = return_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "0".to_string(),
        })));
        assert_eq!(returned.state.focused_pane, PaneId::RepoStaged);
        assert_eq!(
            returned
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Worktrees)
        );

        let commit_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut commit_filter_app = TuiApp::new(commit_state, AppConfig::default());
        let commit_filter =
            commit_filter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "/".to_string(),
            })));
        assert_eq!(
            commit_filter
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commits_filter.focused),
            Some(true)
        );
    }

    #[test]
    fn repo_mode_tags_detail_routes_filter_navigation_and_actions() {
        let repo_id = RepoId::new("repo-1");
        let base_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    RepoSummary {
                        repo_id: repo_id.clone(),
                        display_name: "repo-1".to_string(),
                        display_path: "/tmp/repo-1".to_string(),
                        remote_summary: super_lazygit_core::RemoteSummary {
                            remote_name: Some("origin".to_string()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Tags,
                detail: Some(sample_repo_detail()),
                tags_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };

        let mut next_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let moved_next = next_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            moved_next
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.tags_view.selected_index),
            Some(1)
        );

        let mut previous_app = TuiApp::new(moved_next.state, AppConfig::default());
        let moved_previous =
            previous_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "k".to_string(),
            })));
        assert_eq!(
            moved_previous
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.tags_view.selected_index),
            Some(0)
        );

        let mut filter_app = TuiApp::new(base_state.clone(), AppConfig::default());
        let filter_focus = filter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "/".to_string(),
        })));
        assert_eq!(
            filter_focus
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.tags_filter.focused),
            Some(true)
        );

        let mut paste_app = TuiApp::new(filter_focus.state, AppConfig::default());
        let filtered = paste_app.dispatch(Event::Input(InputEvent::Paste("snap".to_string())));
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.tags_filter.query.as_str()),
            Some("snap")
        );
        assert_eq!(
            filtered
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.tags_view.selected_index),
            Some(1)
        );

        let mut blur_app = TuiApp::new(filtered.state.clone(), AppConfig::default());
        let blurred = blur_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            blurred
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.tags_filter.focused),
            Some(false)
        );

        let mut commits_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let tag_commits = commits_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            tag_commits
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            tag_commits
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("snapshot")
        );

        let mut checkout_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let checkout = checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert_eq!(
            checkout.effects,
            vec![super_lazygit_core::Effect::RunGitCommand(
                super_lazygit_core::GitCommandRequest {
                    job_id: super_lazygit_core::JobId::new("git:repo-1:checkout-tag"),
                    repo_id: repo_id.clone(),
                    command: super_lazygit_core::GitCommand::CheckoutTag {
                        tag_name: "snapshot".to_string(),
                    },
                }
            )]
        );

        let mut prompt_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let prompt = prompt_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(prompt.state.focused_pane, PaneId::Modal);
        assert_eq!(
            prompt
                .state
                .pending_input_prompt
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::CreateTag)
        );

        let mut delete_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let delete = delete_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(delete.state.focused_pane, PaneId::Modal);
        assert_eq!(
            delete
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::DeleteTag {
                tag_name: "snapshot".to_string(),
            })
        );

        let mut copy_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+o".to_string(),
        })));
        assert!(matches!(
            copied.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            )] if command.contains("snapshot")
        ));

        let mut push_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let push = push_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "P".to_string(),
        })));
        assert_eq!(push.state.focused_pane, PaneId::Modal);
        assert_eq!(
            push.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::PushTag {
                remote_name: "origin".to_string(),
                tag_name: "snapshot".to_string(),
            })
        );

        let mut menu_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
        let menu = menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(
            menu.state
                .pending_menu
                .as_ref()
                .map(|pending| pending.operation),
            Some(super_lazygit_core::MenuOperation::TagResetOptions)
        );

        for (key, mode) in [
            ("S", super_lazygit_core::ResetMode::Soft),
            ("M", super_lazygit_core::ResetMode::Mixed),
            ("H", super_lazygit_core::ResetMode::Hard),
        ] {
            let mut reset_app = TuiApp::new(blurred.state.clone(), AppConfig::default());
            let reset = reset_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: key.to_string(),
            })));
            assert_eq!(reset.state.focused_pane, PaneId::Modal);
            assert_eq!(
                reset
                    .state
                    .pending_confirmation
                    .as_ref()
                    .map(|pending| pending.operation.clone()),
                Some(super_lazygit_core::ConfirmableOperation::ResetToCommit {
                    mode,
                    commit: "snapshot".to_string(),
                    summary: "tag snapshot (1234567)".to_string(),
                }),
                "expected reset key {key} to target the selected tag",
            );
        }
    }

    #[test]
    fn repo_mode_commit_detail_enter_opens_selected_commit_files() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_subview_mode),
            Some(CommitSubviewMode::Files)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_files_mode),
            Some(CommitFilesMode::List)
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
    fn repo_mode_commit_file_list_enter_opens_file_diff() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::Files,
                commit_files_mode: CommitFilesMode::List,
                detail: Some(sample_repo_detail()),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| (repo_mode.commit_subview_mode, repo_mode.commit_files_mode)),
            Some((CommitSubviewMode::Files, CommitFilesMode::Diff))
        );
        assert_eq!(
            result.effects,
            vec![
                super_lazygit_core::Effect::LoadRepoDiff {
                    repo_id: RepoId::new("repo-1"),
                    comparison_target: Some(super_lazygit_core::ComparisonTarget::Commit(
                        "abcdef1234567890^!".to_string(),
                    )),
                    compare_with: None,
                    selected_path: Some(PathBuf::from("src/lib.rs")),
                    diff_presentation: DiffPresentation::Comparison,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: super_lazygit_core::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold:
                        super_lazygit_core::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                super_lazygit_core::Effect::ScheduleRender,
            ]
        );
    }

    #[test]
    fn repo_mode_commit_file_diff_enter_returns_to_file_list() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::Files,
                commit_files_mode: CommitFilesMode::Diff,
                detail: Some(sample_repo_detail()),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| (repo_mode.commit_subview_mode, repo_mode.commit_files_mode)),
            Some((CommitSubviewMode::Files, CommitFilesMode::List))
        );
    }

    #[test]
    fn repo_mode_commit_detail_space_queues_detached_checkout() {
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
            key: "space".to_string(),
        })));

        assert_eq!(
            result.effects,
            vec![super_lazygit_core::Effect::RunGitCommand(
                super_lazygit_core::GitCommandRequest {
                    job_id: super_lazygit_core::JobId::new("git:repo-1:checkout-commit"),
                    repo_id: RepoId::new("repo-1"),
                    command: super_lazygit_core::GitCommand::CheckoutCommit {
                        commit: "1234567890abcdef".to_string(),
                    },
                }
            )]
        );
    }

    #[test]
    fn repo_mode_commit_file_detail_space_queues_file_checkout() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::Files,
                detail: Some(sample_repo_detail()),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));

        assert_eq!(
            result.effects,
            vec![super_lazygit_core::Effect::RunGitCommand(
                super_lazygit_core::GitCommandRequest {
                    job_id: super_lazygit_core::JobId::new("git:repo-1:checkout-commit-file"),
                    repo_id: RepoId::new("repo-1"),
                    command: super_lazygit_core::GitCommand::CheckoutCommitFile {
                        commit: "abcdef1234567890".to_string(),
                        path: PathBuf::from("src/lib.rs"),
                    },
                }
            )]
        );
    }

    #[test]
    fn repo_mode_commit_detail_n_opens_branch_from_commit_prompt() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_input_prompt
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::InputPromptOperation::CreateBranchFromCommit {
                    commit: "abcdef1234567890".to_string(),
                    summary: "abcdef1 add lib".to_string(),
                }
            )
        );
    }

    #[test]
    fn repo_mode_commit_detail_shift_t_opens_tag_from_commit_prompt() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "T".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_input_prompt
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::InputPromptOperation::CreateTagFromCommit {
                    commit: "abcdef1234567890".to_string(),
                    summary: "abcdef1 add lib".to_string(),
                }
            )
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

        let mut copy_app = TuiApp::new(state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "C".to_string(),
        })));
        assert_eq!(
            copied
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.copied_commit.as_ref())
                .map(|commit| (
                    commit.oid.as_str(),
                    commit.short_oid.as_str(),
                    commit.summary.as_str(),
                )),
            Some(("1234567890abcdef", "1234567", "second"))
        );

        let mut cherry_pick_app = TuiApp::new(copied.state.clone(), AppConfig::default());
        let cherry_pick =
            cherry_pick_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "V".to_string(),
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

        let mut create_fixup_app = TuiApp::new(state.clone(), AppConfig::default());
        let create_fixup =
            create_fixup_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "f".to_string(),
            })));
        assert_eq!(create_fixup.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create_fixup
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::CommitFixupOptions)
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

        let mut apply_fixups_app = TuiApp::new(state.clone(), AppConfig::default());
        let apply_fixups =
            apply_fixups_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "g".to_string(),
            })));
        assert_eq!(
            apply_fixups
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::ApplyFixupCommits {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 second".to_string(),
                }
            )
        );

        let mut squash_app = TuiApp::new(state.clone(), AppConfig::default());
        let squash = squash_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "s".to_string(),
        })));
        assert_eq!(
            squash
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::SquashCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut drop_app = TuiApp::new(state.clone(), AppConfig::default());
        let drop = drop_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(
            drop.state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::DropCommit {
                commit: "1234567890abcdef".to_string(),
                summary: "1234567 second".to_string(),
            })
        );

        let mut move_up_app = TuiApp::new(state.clone(), AppConfig::default());
        let move_up = move_up_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+k".to_string(),
        })));
        assert_eq!(
            move_up
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::MoveCommitUp {
                commit: "1234567890abcdef".to_string(),
                adjacent_commit: "abcdef1234567890".to_string(),
                summary: "1234567 second".to_string(),
                adjacent_summary: "abcdef1 add lib".to_string(),
            })
        );

        let move_down_state = AppState {
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..state.clone()
        };
        let mut move_down_app = TuiApp::new(move_down_state, AppConfig::default());
        let move_down = move_down_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+j".to_string(),
        })));
        assert_eq!(
            move_down
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::MoveCommitDown {
                commit: "abcdef1234567890".to_string(),
                adjacent_commit: "1234567890abcdef".to_string(),
                summary: "abcdef1 add lib".to_string(),
                adjacent_summary: "1234567 second".to_string(),
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
            key: "t".to_string(),
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
        let mut checkout_app = TuiApp::new(state, AppConfig::default());

        let checkout_by_name =
            checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "c".to_string(),
            })));
        assert_eq!(checkout_by_name.state.focused_pane, PaneId::Modal);
        assert_eq!(
            checkout_by_name
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::CheckoutBranch,
                ""
            ))
        );

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
        let mut create_app = TuiApp::new(state, AppConfig::default());

        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(create.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((&super_lazygit_core::InputPromptOperation::CreateBranch, ""))
        );

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
        let mut previous_app = TuiApp::new(state, AppConfig::default());

        let previous = previous_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "-".to_string(),
        })));
        assert!(previous.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CheckoutBranch { branch_ref },
                ..
            }) if branch_ref == "-"
        )));

        let mut space_checkout_app = TuiApp::new(down.state.clone(), AppConfig::default());

        let space_checkout =
            space_checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "space".to_string(),
            })));
        assert!(space_checkout.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CheckoutBranch { branch_ref },
                ..
            }) if branch_ref == "feature"
        )));

        let mut upstream_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let upstream_menu =
            upstream_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "u".to_string(),
            })));
        assert_eq!(upstream_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            upstream_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BranchUpstreamOptions)
        );

        let mut copy_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "y".to_string(),
        })));
        assert!(copied.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            ) if command.contains("feature")
        )));

        let mut copy_alias_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let copied_alias =
            copy_alias_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "ctrl+o".to_string(),
            })));
        assert!(copied_alias.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            ) if command.contains("feature")
        )));

        let mut pr_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let pr_menu = pr_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert_eq!(pr_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            pr_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BranchPullRequestOptions)
        );

        let mut reset_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let reset_menu = reset_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(reset_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            reset_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BranchResetOptions)
        );

        let mut sort_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let sort_menu = sort_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "s".to_string(),
        })));
        assert_eq!(sort_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            sort_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BranchSortOptions)
        );

        let mut git_flow_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let git_flow_menu =
            git_flow_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "G".to_string(),
            })));
        assert_eq!(git_flow_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            git_flow_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BranchGitFlowOptions)
        );

        let mut rebase_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let rebase = rebase_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "r".to_string(),
        })));
        assert_eq!(rebase.state.focused_pane, PaneId::Modal);
        assert_eq!(
            rebase
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::RebaseCurrentBranchOntoRef {
                    target_ref: "feature".to_string(),
                    source_label: "feature".to_string(),
                }
            )
        );

        let mut merge_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let merge = merge_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "M".to_string(),
        })));
        assert_eq!(merge.state.focused_pane, PaneId::Modal);
        assert_eq!(
            merge
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::MergeRefIntoCurrent {
                    target_ref: "feature".to_string(),
                    source_label: "feature".to_string(),
                }
            )
        );

        let mut force_checkout_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let force_checkout =
            force_checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "F".to_string(),
            })));
        assert_eq!(force_checkout.state.focused_pane, PaneId::Modal);
        assert_eq!(
            force_checkout
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::ForceCheckoutRef {
                target_ref: "feature".to_string(),
                source_label: "feature".to_string(),
            })
        );

        let mut tag_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let tag = tag_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "T".to_string(),
        })));
        assert_eq!(tag.state.focused_pane, PaneId::Modal);
        assert_eq!(
            tag.state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateTagFromRef {
                    target_ref: "feature".to_string(),
                    source_label: "feature".to_string(),
                },
                ""
            ))
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

        let mut worktree_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let worktrees = worktree_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "w".to_string(),
        })));
        assert_eq!(
            worktrees
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Worktrees)
        );
    }

    #[test]
    fn repo_mode_remotes_detail_routes_selection_and_actions() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Remotes,
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
                .and_then(|repo_mode| repo_mode.remotes_view.selected_index),
            Some(1)
        );

        let mut open_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let opened = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            opened
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::RemoteBranches)
        );
        assert_eq!(
            opened
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.remote_branches_filter.query.as_str()),
            Some("upstream")
        );

        let mut create_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(create.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((&super_lazygit_core::InputPromptOperation::CreateRemote, ""))
        );

        let mut edit_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let edit = edit_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "e".to_string(),
        })));
        assert_eq!(edit.state.focused_pane, PaneId::Modal);
        assert_eq!(
            edit.state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::EditRemote {
                    current_name: "upstream".to_string(),
                    current_url: "git@github.com:example/upstream.git".to_string(),
                },
                "upstream git@github.com:example/upstream.git"
            ))
        );

        let mut fetch_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let fetch = fetch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "f".to_string(),
        })));
        assert_eq!(fetch.state.focused_pane, PaneId::Modal);
        assert_eq!(
            fetch
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::FetchRemote {
                remote_name: "upstream".to_string(),
            })
        );

        let mut fork_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let fork = fork_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "F".to_string(),
        })));
        assert_eq!(fork.state.focused_pane, PaneId::Modal);
        assert_eq!(
            fork.state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::ForkRemote {
                    suggested_name: "upstream-fork".to_string(),
                    remote_url: "git@github.com:example/upstream.git".to_string(),
                },
                "upstream-fork git@github.com:example/upstream.git"
            ))
        );

        let mut delete_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let delete = delete_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(delete.state.focused_pane, PaneId::Modal);
        assert_eq!(
            delete
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::RemoveRemote {
                remote_name: "upstream".to_string(),
            })
        );
    }

    #[test]
    fn repo_mode_remotes_filter_clear_keeps_create_remote_prompt_routable() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Remotes,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let focused = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "/".to_string(),
        })));
        let mut filtered_app = TuiApp::new(focused.state, AppConfig::default());
        let pasted = filtered_app.dispatch(Event::Input(InputEvent::Paste("orig".to_string())));
        let mut cleared_app = TuiApp::new(pasted.state, AppConfig::default());
        for _ in 0..4 {
            let next = cleared_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "backspace".to_string(),
            })));
            cleared_app = TuiApp::new(next.state, AppConfig::default());
        }
        let blurred = cleared_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        let mut create_app = TuiApp::new(blurred.state, AppConfig::default());
        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));

        assert_eq!(create.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((&super_lazygit_core::InputPromptOperation::CreateRemote, ""))
        );
    }

    #[test]
    fn repo_mode_remote_branch_detail_routes_selection_and_actions() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::RemoteBranches,
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
                .and_then(|repo_mode| repo_mode.remote_branches_view.selected_index),
            Some(1)
        );

        let mut open_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let opened = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            opened
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            opened
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commit_history_ref.as_deref()),
            Some("origin/feature")
        );

        let mut checkout_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let checkout = checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(checkout.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::CheckoutRemoteBranch {
                    remote_branch_ref,
                    local_branch_name,
                },
                ..
            }) if remote_branch_ref == "origin/feature" && local_branch_name == "feature"
        )));

        let mut create_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(create.state.focused_pane, PaneId::Modal);
        assert_eq!(
            create
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateBranchFromRemote {
                    remote_branch_ref: "origin/feature".to_string(),
                    suggested_name: "feature".to_string(),
                },
                "feature"
            ))
        );

        let mut delete_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let delete = delete_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(delete.state.focused_pane, PaneId::Modal);
        assert_eq!(
            delete
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::DeleteRemoteBranch {
                    remote_name: "origin".to_string(),
                    branch_name: "feature".to_string(),
                }
            )
        );

        let mut copy_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "y".to_string(),
        })));
        assert!(copied.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            ) if command.contains("origin/feature")
        )));

        let mut copy_alias_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let copied_alias =
            copy_alias_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "ctrl+o".to_string(),
            })));
        assert!(copied_alias.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            ) if command.contains("origin/feature")
        )));

        let mut pr_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let pr_menu = pr_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert_eq!(pr_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            pr_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::RemoteBranchPullRequestOptions)
        );

        let mut reset_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let reset_menu = reset_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(reset_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            reset_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::RemoteBranchResetOptions)
        );

        let mut sort_menu_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let sort_menu = sort_menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "s".to_string(),
        })));
        assert_eq!(sort_menu.state.focused_pane, PaneId::Modal);
        assert_eq!(
            sort_menu
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::RemoteBranchSortOptions)
        );

        let mut upstream_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let upstream = upstream_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "u".to_string(),
        })));
        assert!(upstream.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::SetBranchUpstream {
                    branch_name,
                    upstream_ref,
                },
                ..
            }) if branch_name == "main" && upstream_ref == "origin/feature"
        )));

        let mut rebase_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let rebase = rebase_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "r".to_string(),
        })));
        assert_eq!(rebase.state.focused_pane, PaneId::Modal);
        assert_eq!(
            rebase
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::RebaseCurrentBranchOntoRef {
                    target_ref: "origin/feature".to_string(),
                    source_label: "origin/feature".to_string(),
                }
            )
        );

        let mut merge_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let merge = merge_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "M".to_string(),
        })));
        assert_eq!(merge.state.focused_pane, PaneId::Modal);
        assert_eq!(
            merge
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(
                super_lazygit_core::ConfirmableOperation::MergeRefIntoCurrent {
                    target_ref: "origin/feature".to_string(),
                    source_label: "origin/feature".to_string(),
                }
            )
        );

        let mut tag_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let tag = tag_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "T".to_string(),
        })));
        assert_eq!(tag.state.focused_pane, PaneId::Modal);
        assert_eq!(
            tag.state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateTagFromRef {
                    target_ref: "origin/feature".to_string(),
                    source_label: "origin/feature".to_string(),
                },
                ""
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
                status_tree_enabled: false,
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
                path: std::path::PathBuf::from("src/ui/lib.rs"),
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
    fn input_prompt_copy_describes_branch_creation_from_stash() {
        let copy = input_prompt_copy(
            &super_lazygit_core::InputPromptOperation::CreateBranchFromStash {
                stash_ref: "stash@{1}".to_string(),
                stash_label: "stash@{1}: On feature: prior experiment".to_string(),
            },
        );

        assert!(copy.contains("new branch name"));
        assert!(copy.contains("stash@{1}: On feature: prior experiment"));
        assert!(copy.contains("stash will be dropped"));
    }

    #[test]
    fn input_prompt_copy_describes_checkout_by_name() {
        let copy = input_prompt_copy(&super_lazygit_core::InputPromptOperation::CheckoutBranch);

        assert!(copy.contains("branch name"));
        assert!(copy.contains("remote ref"));
        assert!(copy.contains("Use - to switch back"));
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
    fn repo_mode_stash_detail_routes_selection_create_branch_rename_apply_pop_and_drop() {
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

        let mut space_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let space = space_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(space.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::ApplyStash { stash_ref },
                ..
            }) if stash_ref == "stash@{1}"
        )));

        let mut branch_app = TuiApp::new(state.clone(), AppConfig::default());
        let down = branch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "j".to_string(),
        })));
        assert_eq!(
            down.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_view.selected_index),
            Some(1)
        );

        let branch = branch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(branch.state.focused_pane, PaneId::Modal);
        assert_eq!(
            branch
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| (&prompt.operation, prompt.value.as_str())),
            Some((
                &super_lazygit_core::InputPromptOperation::CreateBranchFromStash {
                    stash_ref: "stash@{1}".to_string(),
                    stash_label: "stash@{1}: On feature: prior experiment".to_string(),
                },
                ""
            ))
        );

        let mut worktree_app = TuiApp::new(down.state.clone(), AppConfig::default());
        let worktrees = worktree_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "w".to_string(),
        })));
        assert_eq!(
            worktrees
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Worktrees)
        );

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
    fn repo_mode_stash_detail_enter_opens_selected_stash_files() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
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
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.stash_subview_mode),
            Some(StashSubviewMode::Files)
        );
        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.stash_files_view.selected_index),
            Some(0)
        );
    }

    #[test]
    fn repo_mode_stash_file_detail_enter_returns_to_stash_list() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Stash,
                stash_subview_mode: StashSubviewMode::Files,
                detail: Some(sample_repo_detail()),
                stash_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                stash_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(
            result
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.stash_subview_mode),
            Some(StashSubviewMode::List)
        );
    }

    #[test]
    fn repo_mode_stash_file_detail_routes_file_navigation_only() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Stash,
                stash_subview_mode: StashSubviewMode::Files,
                detail: Some(sample_repo_detail()),
                stash_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                stash_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
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
                .and_then(|repo_mode| repo_mode.stash_files_view.selected_index),
            Some(1)
        );

        let ignored = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(ignored.effects.is_empty());
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
    fn repo_mode_reflog_detail_routes_commit_context_and_history_actions() {
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

        let mut open_app = TuiApp::new(state.clone(), AppConfig::default());
        let open = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            open.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Commits)
        );
        assert_eq!(
            open.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Graph { reverse: false })
        );
        assert_eq!(
            open.state
                .repo_mode
                .as_ref()
                .and_then(|repo_mode| repo_mode.commits_view.selected_index),
            Some(1)
        );

        let mut checkout_app = TuiApp::new(state.clone(), AppConfig::default());
        let checkout = checkout_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert!(checkout.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::RunGitCommand(job)
                if matches!(
                    job.command,
                    super_lazygit_core::GitCommand::CheckoutCommit { ref commit }
                    if commit == "1234567890abcdef"
                )
        )));

        let mut branch_app = TuiApp::new(state.clone(), AppConfig::default());
        let branch = branch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(
            branch
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| &prompt.operation),
            Some(
                &super_lazygit_core::InputPromptOperation::CreateBranchFromCommit {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 commit: add repo-mode stash flows".to_string(),
                }
            )
        );

        let mut tag_app = TuiApp::new(state.clone(), AppConfig::default());
        let tag = tag_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "T".to_string(),
        })));
        assert_eq!(
            tag.state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| &prompt.operation),
            Some(
                &super_lazygit_core::InputPromptOperation::CreateTagFromCommit {
                    commit: "1234567890abcdef".to_string(),
                    summary: "1234567 commit: add repo-mode stash flows".to_string(),
                }
            )
        );

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
                summary: "1234567 commit: add repo-mode stash flows".to_string(),
            })
        );

        let mut copy_app = TuiApp::new(state.clone(), AppConfig::default());
        let copied = copy_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+o".to_string(),
        })));
        assert!(matches!(
            copied.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            )] if command.contains("1234567")
        ));

        let mut browser_app = TuiApp::new(state.clone(), AppConfig::default());
        let browser = browser_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert!(matches!(
            browser.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            )] if command.contains("github.com/example/upstream/commit/1234567890abcdef")
        ));

        let mut menu_app = TuiApp::new(state.clone(), AppConfig::default());
        let menu = menu_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "g".to_string(),
        })));
        assert_eq!(
            menu.state
                .pending_menu
                .as_ref()
                .map(|pending| pending.operation),
            Some(super_lazygit_core::MenuOperation::ReflogResetOptions)
        );

        let mut reset_app = TuiApp::new(state, AppConfig::default());
        let reset = reset_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "S".to_string(),
        })));
        assert_eq!(
            reset
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::ResetToCommit {
                mode: super_lazygit_core::ResetMode::Soft,
                commit: "HEAD@{1}".to_string(),
                summary: "HEAD@{1}: commit: add repo-mode stash flows".to_string(),
            })
        );
    }

    #[test]
    fn repo_mode_worktree_detail_routes_selection_switch_create_open_and_remove() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("repo-1")],
                selected_repo_id: Some(RepoId::new("repo-1")),
                repo_summaries: std::collections::BTreeMap::from([(
                    RepoId::new("repo-1"),
                    RepoSummary {
                        repo_id: RepoId::new("repo-1"),
                        display_name: "repo-1".to_string(),
                        display_path: "/tmp/repo-1".to_string(),
                        real_path: PathBuf::from("/tmp/repo-1"),
                        ..RepoSummary::default()
                    },
                )]),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Worktrees,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };

        let mut focus_filter_app = TuiApp::new(state.clone(), AppConfig::default());
        let focused_filter =
            focus_filter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "/".to_string(),
            })));
        assert_eq!(
            focused_filter
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.focused),
            Some(true)
        );

        let mut filter_app = TuiApp::new(focused_filter.state, AppConfig::default());
        let filtered = filter_app.dispatch(Event::Input(InputEvent::Paste("feature".to_string())));
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

        let mut blur_filter_app = TuiApp::new(filtered.state.clone(), AppConfig::default());
        let blurred_filter =
            blur_filter_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
                key: "enter".to_string(),
            })));
        assert_eq!(
            blurred_filter
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.focused),
            Some(false)
        );
        assert_eq!(
            blurred_filter
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.worktree_filter.query.as_str()),
            Some("feature")
        );

        let mut switch_app = TuiApp::new(blurred_filter.state.clone(), AppConfig::default());
        let switch = switch_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "space".to_string(),
        })));
        assert_eq!(
            switch
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(RepoId::new("/tmp/repo-1-feature"))
        );
        assert_eq!(
            switch.effects,
            vec![
                super_lazygit_core::Effect::LoadRepoDetail {
                    repo_id: RepoId::new("/tmp/repo-1-feature"),
                    selected_path: None,
                    diff_presentation: DiffPresentation::Unstaged,
                    commit_ref: None,
                    commit_history_mode: CommitHistoryMode::Linear,
                    ignore_whitespace_in_diff: false,
                    diff_context_lines: super_lazygit_core::DEFAULT_DIFF_CONTEXT_LINES,
                    rename_similarity_threshold:
                        super_lazygit_core::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
                },
                super_lazygit_core::Effect::ScheduleRender,
            ]
        );

        let mut create_app = TuiApp::new(blurred_filter.state.clone(), AppConfig::default());
        let create = create_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
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

        let mut open_app = TuiApp::new(blurred_filter.state.clone(), AppConfig::default());
        let open = open_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "o".to_string(),
        })));
        assert_eq!(
            open.effects,
            vec![super_lazygit_core::Effect::OpenEditor {
                cwd: PathBuf::from("/tmp/repo-1"),
                target: PathBuf::from("/tmp/repo-1-feature"),
            }]
        );

        let mut remove_app = TuiApp::new(blurred_filter.state, AppConfig::default());
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
                status_tree_enabled: false,
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
        assert_eq!(enter.state.focused_pane, PaneId::RepoDetail);

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
                status_tree_enabled: false,
                detail: Some(sample_repo_detail()),
                staged_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let enter = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(enter.state.focused_pane, PaneId::RepoDetail);

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
        assert!(rendered_lines.contains(&"Path: src/ui/lib.rs (unstaged)".to_string()));
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
    fn repo_mode_status_detail_routes_all_branch_graph_shortcuts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Status,
                detail: Some(sample_repo_detail()),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state.clone(), AppConfig::default());

        let forward = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "a".to_string(),
        })));
        assert_eq!(
            forward
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Graph { reverse: false })
        );
        assert!(forward.effects.iter().any(|effect| matches!(
            effect,
            super_lazygit_core::Effect::LoadRepoDetail {
                commit_history_mode: CommitHistoryMode::Graph { reverse: false },
                ..
            }
        )));

        let mut reverse_app = TuiApp::new(state, AppConfig::default());
        let reverse = reverse_app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "A".to_string(),
        })));
        assert_eq!(
            reverse
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.commit_history_mode),
            Some(CommitHistoryMode::Graph { reverse: true })
        );
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
        assert!(rendered.contains("lib.rs"));
        assert!(rendered.contains("README.md"));
        assert!(rendered.contains("Path: src/ui/lib.rs"));
        assert!(rendered.contains("Hunks: 1"));
        assert!(rendered.contains("Line select: J/K cursor"));
        assert!(rendered.contains("@@ -1 +1 @@"));
        assert!(rendered.contains("+new line"));
        assert!(rendered.contains("Repository shell"));
        assert!(rendered.contains("Watch: unknown"));
        assert!(rendered.contains("Screen: normal"));
    }

    #[test]
    fn diff_action_help_line_tracks_stage_unstage_and_read_only_modes() {
        let repo_mode = RepoModeState::new(RepoId::new("repo-1"));
        let mut detail = sample_repo_detail();

        assert!(diff_action_help_line(Some(&repo_mode), &detail).contains("Enter/Space stage hunk"));
        assert!(
            diff_action_help_line(Some(&repo_mode), &detail).contains("Mode: working tree staging")
        );

        detail.merge_state = super_lazygit_core::MergeState::MergeInProgress;
        assert!(diff_action_help_line(Some(&repo_mode), &detail).contains("Mode: merge resolution"));

        detail.diff.presentation = DiffPresentation::Staged;
        assert!(
            diff_action_help_line(Some(&repo_mode), &detail).contains("Enter/Space unstage hunk")
        );
        assert!(diff_action_help_line(Some(&repo_mode), &detail).contains("Mode: staged changes"));

        detail.diff.presentation = DiffPresentation::Comparison;
        assert!(diff_action_help_line(Some(&repo_mode), &detail).contains("Read-only diff"));
        assert!(diff_action_help_line(Some(&repo_mode), &detail).contains("Mode: comparison"));
    }

    #[test]
    fn status_detail_focus_help_tracks_main_panel_submode_copy() {
        let repo_id = RepoId::new("repo-1");
        let mut repo_mode = RepoModeState {
            current_repo_id: repo_id.clone(),
            active_subview: RepoSubview::Status,
            detail: Some(sample_repo_detail()),
            ..RepoModeState::new(repo_id)
        };

        assert!(
            status_detail_focus_help(&repo_mode).contains("Enter/Space stages the current hunk")
        );
        assert!(status_detail_focus_help(&repo_mode).contains("Esc or 0 returns to the main pane"));
        assert!(status_detail_focus_help(&repo_mode).contains("o opens the config file"));
        assert!(status_detail_focus_help(&repo_mode).contains("u checks for updates"));

        if let Some(detail) = repo_mode.detail.as_mut() {
            detail.merge_state = super_lazygit_core::MergeState::MergeInProgress;
        }
        assert!(status_detail_focus_help(&repo_mode).contains("merge resolution"));

        if let Some(detail) = repo_mode.detail.as_mut() {
            detail.diff.presentation = DiffPresentation::Staged;
        }
        assert!(
            status_detail_focus_help(&repo_mode).contains("Enter/Space unstages the current hunk")
        );

        if let Some(detail) = repo_mode.detail.as_mut() {
            detail.diff.presentation = DiffPresentation::Comparison;
        }
        assert!(status_detail_focus_help(&repo_mode).contains("read-only"));
    }

    #[test]
    fn render_repo_shell_hides_side_panes_in_fullscreen_mode() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                screen_mode: super_lazygit_core::ScreenMode::FullScreen,
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
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Screen: fullscreen"));
        assert!(rendered.contains("Detail: Status"));
        assert!(!rendered.contains("Working tree"));
        assert!(!rendered.contains("Staged changes"));
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
        app.resize(160, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Commits"));
        assert!(rendered.contains("Selected 1/2"));
        assert!(rendered.contains("Compare base: abcdef1234567890"));
        assert!(rendered.contains("State: idle"));
        assert!(rendered.contains("> abcdef1 add lib"));
        assert!(rendered.contains("n branch"));
        assert!(rendered.contains("T tag"));
        assert!(rendered.contains("A amend"));
        assert!(rendered.contains("F fixup"));
        assert!(rendered.contains("r reword"));
        assert!(rendered.contains("R reword editor"));
        assert!(
            repo_commit_context_line(CommitHistoryMode::Linear, None, "", false, 2, 2)
                .contains("a amend attrs")
        );
        let help = repo_help_text(app.state());
        assert!(help.contains("a amend attrs"));
        assert!(help.contains("r reword"));
        assert!(help.contains("R reword editor"));
        assert!(help.contains("y copy menu"));
        assert!(help.contains("C copy"));
        assert!(help.contains("V paste copied"));
        assert!(help.contains("t revert"));
        assert!(help.contains("S soft"));
        assert!(rendered.contains("A src/lib.rs"));
    }

    #[test]
    fn render_repo_shell_shows_all_branch_graph_preview() {
        let mut detail = sample_repo_detail();
        detail.commit_graph_lines = vec![
            "* abcdef1 (HEAD -> main) add lib".to_string(),
            "| * 1234567 second".to_string(),
        ];
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
                commit_history_mode: CommitHistoryMode::Graph { reverse: true },
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
        app.resize(160, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Commits"));
        assert!(rendered.contains("* abcdef1 (HEAD -> main) add lib"));
        assert!(repo_commit_context_line(
            CommitHistoryMode::Graph { reverse: true },
            None,
            "",
            false,
            2,
            2
        )
        .contains("3 current branch. Ctrl+L log menu"));
        assert!(!rendered.contains("A oldest-first"));
    }

    #[test]
    fn render_repo_shell_shows_commit_file_view_actions() {
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
                commit_subview_mode: CommitSubviewMode::Files,
                detail: Some(sample_repo_detail()),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
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
        app.resize(120, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Commit files  abcdef1  add lib"));
        assert!(rendered.contains("Left/backspace history."));
        assert!(rendered.contains("Actions: e editor."));
        assert!(rendered.contains("y copy path"));
        assert!(rendered.contains("Ctrl+T difftool"));
        assert!(rendered.contains("> A src/lib.rs"));
    }

    #[test]
    fn render_repo_shell_shows_commit_file_diff_view() {
        let mut detail = sample_repo_detail();
        detail.diff = DiffModel {
            selected_path: Some(PathBuf::from("src/lib.rs")),
            presentation: DiffPresentation::Comparison,
            lines: detail.commits[0].diff.lines.clone(),
            hunks: detail.commits[0].diff.hunks.clone(),
            selected_hunk: detail.commits[0].diff.selected_hunk,
            hunk_count: detail.commits[0].diff.hunk_count,
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
                active_subview: RepoSubview::Commits,
                commit_subview_mode: CommitSubviewMode::Files,
                commit_files_mode: CommitFilesMode::Diff,
                detail: Some(detail),
                commit_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
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
        app.resize(120, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Commit file diff  abcdef1  add lib"));
        assert!(rendered.contains("File: src/lib.rs"));
        assert!(rendered.contains("Context: Enter files. Space checkout file. e open editor."));
        assert!(rendered.contains("Inspect: j/k hunks. J/K changed lines. v anchor."));
        assert!(rendered.contains("Path: src/lib.rs (comparison)"));
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
        app.resize(120, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("State: cherry-pick in progress"));
        let help = repo_help_text(app.state());
        assert!(help.contains("y copy menu"));
        assert!(help.contains("C copy"));
        assert!(help.contains("V paste copied"));
        assert!(help.contains("t revert"));
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
        app.resize(140, 18);

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
        app.resize(140, 18);

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
        app.resize(140, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Branches"));
        assert!(rendered.contains("Selected: feature"));
        assert!(rendered.contains("Context: Enter commits. Space checkout."));
        assert!(rendered.contains("checkout by name"));
        assert!(rendered.contains("u upstream"));
        assert!(rendered.contains("F force checkout"));
        assert!(rendered.contains("o pull request"));
        assert!(rendered.contains("g reset"));
        assert!(rendered.contains("s sort"));
        assert!(rendered.contains("G git-flow"));
        assert!(rendered.contains("T tag"));
        assert!(rendered.contains("* main"));
        assert!(rendered.contains("feature"));
        let help = repo_help_text(app.state());
        assert!(help.contains("F force checkout"));
        assert!(help.contains("u upstream menu"));
        assert!(help.contains("o pull request menu"));
        assert!(help.contains("g reset menu"));
        assert!(help.contains("s sort menu"));
        assert!(help.contains("G git-flow menu"));
        assert!(help.contains("y/Ctrl+O copy"));
        assert!(help.contains("r rebase current"));
        assert!(help.contains("M merge into current"));
        assert!(help.contains("T tag"));
    }

    #[test]
    fn render_repo_shell_shows_branch_filter_summary_when_focused() {
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
                branches_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "fea".to_string(),
                    focused: true,
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

        assert!(rendered.contains("Filter /fea_  Matches: 1/2  (focused)"));
        assert!(rendered.contains("Context: Enter commits. Space checkout."));
    }

    #[test]
    fn render_repo_shell_shows_remote_details() {
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
                active_subview: RepoSubview::Remotes,
                detail: Some(sample_repo_detail()),
                remotes_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                remotes_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "up".to_string(),
                    focused: true,
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

        assert!(rendered.contains("Detail: Remotes"));
        assert!(rendered.contains("Selected: upstream"));
        assert!(rendered.contains("Fetch: git@github.com:example/upstream.git"));
        assert!(rendered.contains("Push: git@github.com:example/upstream.git"));
        assert!(rendered.contains("Branches: 0"));
        assert!(rendered.contains("Filter /up_  Matches: 1/2  (focused)"));
        assert!(rendered.contains("Context: Enter branches. f fetch."));
        assert!(rendered.contains("n new remote"));
        assert!(rendered.contains("upstream  [0 branches]"));
        let mut help_state = app.state().clone();
        if let Some(repo_mode) = help_state.repo_mode.as_mut() {
            repo_mode.remotes_filter.focused = false;
        }
        let help = repo_help_text(&help_state);
        assert!(help.contains("F fork remote"));
    }

    #[test]
    fn render_repo_shell_shows_remote_branch_details() {
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
                active_subview: RepoSubview::RemoteBranches,
                detail: Some(sample_repo_detail()),
                remote_branches_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                remote_branches_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "fea".to_string(),
                    focused: true,
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

        assert!(rendered.contains("Detail: Remote Branches"));
        assert!(rendered.contains("Selected: origin/feature"));
        assert!(rendered.contains("Remote: origin"));
        assert!(rendered.contains("Local branch: feature"));
        assert!(rendered.contains("Filter /fea_  Matches: 1/2  (focused)"));
        assert!(rendered.contains("Context: Enter commits. Space checkout."));
        assert!(rendered.contains("n create local branch"));
        assert!(rendered.contains("d delete remote branch"));
        let mut help_state = app.state().clone();
        if let Some(repo_mode) = help_state.repo_mode.as_mut() {
            repo_mode.remote_branches_filter.focused = false;
        }
        let help = repo_help_text(&help_state);
        assert!(help.contains("o pull request menu"));
        assert!(help.contains("g reset menu"));
        assert!(help.contains("s sort menu"));
        assert!(help.contains("y/Ctrl+O copy"));
        assert!(help.contains("u set upstream"));
        assert!(help.contains("r rebase current"));
        assert!(help.contains("M merge into current"));
        assert!(help.contains("T tag"));
    }

    #[test]
    fn visible_branch_indices_follow_selected_sort_mode() {
        let detail = sample_repo_detail();

        assert_eq!(
            visible_branch_indices(&detail, "", super_lazygit_core::BranchSortMode::Natural),
            vec![0, 1]
        );
        assert_eq!(
            visible_branch_indices(&detail, "", super_lazygit_core::BranchSortMode::Name),
            vec![1, 0]
        );
    }

    #[test]
    fn visible_remote_branch_indices_follow_selected_sort_mode() {
        let detail = sample_repo_detail();

        assert_eq!(
            visible_remote_branch_indices(
                &detail,
                "",
                super_lazygit_core::RemoteBranchSortMode::Natural,
            ),
            vec![0, 1]
        );
        assert_eq!(
            visible_remote_branch_indices(
                &detail,
                "",
                super_lazygit_core::RemoteBranchSortMode::Name,
            ),
            vec![1, 0]
        );
    }

    #[test]
    fn render_repo_shell_shows_tag_details() {
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
                active_subview: RepoSubview::Tags,
                detail: Some(sample_repo_detail()),
                tags_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                tags_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "v1".to_string(),
                    focused: true,
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
        let mut help_state = app.state().clone();
        if let Some(repo_mode) = help_state.repo_mode.as_mut() {
            repo_mode.tags_filter.focused = false;
        }
        let help = repo_help_text(&help_state);

        assert!(rendered.contains("Detail: Tags"));
        assert!(rendered.contains("Selected: v1.0.0"));
        assert!(rendered.contains("Target: abcdef1"));
        assert!(rendered.contains("Type: annotated"));
        assert!(rendered.contains("Summary: release v1.0.0"));
        assert!(rendered.contains("Filter /v1_  Matches: 1/2  (focused)"));
        assert!(rendered.contains("Ctrl+O copy tag."));
        assert!(rendered.contains("n create tag"));
        assert!(help.contains("g reset menu"));
        assert!(rendered.contains("P push tag"));
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
        assert!(rendered.contains("Context: Enter files. Space apply."));
        assert!(rendered.contains("0 main. / filter."));
        assert!(rendered.contains("Other: n branches off. r renames. g pops. d drops."));
        assert!(rendered.contains("n branches off"));
        assert!(rendered.contains("g pops"));
        assert!(rendered.contains("stash@{0}: WIP on main: fixture stash"));
        assert!(rendered.contains("stash@{1}: On feature: prior experiment"));
    }

    #[test]
    fn render_repo_shell_shows_stash_file_details() {
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
                stash_subview_mode: StashSubviewMode::Files,
                detail: Some(sample_repo_detail()),
                stash_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                stash_files_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
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
        assert!(rendered.contains("Stash files  stash@{1}  prior experiment"));
        assert!(rendered.contains("Context: Enter stash list. 0 main. w worktrees."));
        assert!(rendered.contains("> M src/lib.rs"));
        assert!(rendered.contains("  D docs/notes.md"));
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
        let help = repo_help_text(app.state());

        assert!(rendered.contains("Detail: Reflog"));
        assert!(rendered.contains("Selected 2/2"));
        assert!(rendered.contains("HEAD@{1}  1234567 commit: add repo-mode stash flows"));
        assert!(help.contains("Ctrl+O copy hash"));
        assert!(help.contains("o browser"));
        assert!(help.contains("g reset menu"));
        assert!(rendered.contains("Use j/k to inspect recent HEAD and ref movement."));
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
    fn delete_remote_branch_confirmation_copy_mentions_push_delete() {
        let copy = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::DeleteRemoteBranch {
                remote_name: "origin".to_string(),
                branch_name: "feature".to_string(),
            },
        );

        assert!(copy.contains("Delete remote branch origin/feature?"));
        assert!(copy.contains("git push origin --delete feature"));
    }

    #[test]
    fn remote_confirmation_copy_mentions_fetch_and_remove_operations() {
        let fetch = confirmation_copy(&super_lazygit_core::ConfirmableOperation::FetchRemote {
            remote_name: "upstream".to_string(),
        });
        let remove = confirmation_copy(&super_lazygit_core::ConfirmableOperation::RemoveRemote {
            remote_name: "upstream".to_string(),
        });

        assert!(fetch.contains("Fetch updates from remote upstream?"));
        assert!(remove.contains("Remove remote upstream?"));
        assert!(remove.contains("configured remote entry"));
    }

    #[test]
    fn branch_ref_confirmation_copy_mentions_upstream_merge_and_rebase_commands() {
        let unset = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::UnsetBranchUpstream {
                branch_name: "feature".to_string(),
            },
        );
        let fast_forward = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::FastForwardCurrentBranchFromUpstream {
                branch_name: "main".to_string(),
                upstream_ref: "origin/main".to_string(),
            },
        );
        let merge = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::MergeRefIntoCurrent {
                target_ref: "feature".to_string(),
                source_label: "feature".to_string(),
            },
        );
        let rebase = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::RebaseCurrentBranchOntoRef {
                target_ref: "origin/feature".to_string(),
                source_label: "origin/feature".to_string(),
            },
        );

        assert!(unset.contains("Unset upstream for feature?"));
        assert!(unset.contains("tracking branch"));
        assert!(fast_forward.contains("git merge --ff-only origin/main"));
        assert!(merge.contains("git merge feature"));
        assert!(rebase.contains("git rebase origin/feature"));
        assert!(rebase.contains("rewrites local history"));
    }

    #[test]
    fn force_checkout_confirmation_copy_mentions_checkout_f() {
        let copy = confirmation_copy(
            &super_lazygit_core::ConfirmableOperation::ForceCheckoutRef {
                target_ref: "feature".to_string(),
                source_label: "feature".to_string(),
            },
        );

        assert!(copy.contains("Force-checkout feature?"));
        assert!(copy.contains("git checkout -f feature"));
        assert!(copy.contains("discards tracked working tree changes"));
    }

    #[test]
    fn tag_confirmation_copy_mentions_delete_and_push_commands() {
        let delete = confirmation_copy(&super_lazygit_core::ConfirmableOperation::DeleteTag {
            tag_name: "release-candidate".to_string(),
        });
        let push = confirmation_copy(&super_lazygit_core::ConfirmableOperation::PushTag {
            remote_name: "origin".to_string(),
            tag_name: "release-candidate".to_string(),
        });

        assert!(delete.contains("Delete tag release-candidate?"));
        assert!(delete.contains("local tag reference"));
        assert!(push.contains("Push tag release-candidate to origin?"));
        assert!(push.contains("git push origin refs/tags/release-candidate"));
    }

    #[test]
    fn create_branch_from_remote_prompt_copy_mentions_selected_ref() {
        let copy = input_prompt_copy(
            &super_lazygit_core::InputPromptOperation::CreateBranchFromRemote {
                remote_branch_ref: "origin/feature".to_string(),
                suggested_name: "feature".to_string(),
            },
        );

        assert!(copy.contains("origin/feature"));
        assert!(copy.contains("created from origin/feature and checked out"));
    }

    #[test]
    fn create_tag_prompt_copy_mentions_current_head() {
        let copy = input_prompt_copy(&super_lazygit_core::InputPromptOperation::CreateTag);

        assert!(copy.contains("new tag name"));
        assert!(copy.contains("created at the current HEAD"));
    }

    #[test]
    fn create_tag_from_commit_prompt_copy_mentions_selected_commit() {
        let copy = input_prompt_copy(
            &super_lazygit_core::InputPromptOperation::CreateTagFromCommit {
                commit: "abcdef1234567890".to_string(),
                summary: "abcdef1 add lib".to_string(),
            },
        );

        assert!(copy.contains("new tag name"));
        assert!(copy.contains("abcdef1 add lib"));
        assert!(copy.contains("created from"));
    }

    #[test]
    fn fork_remote_and_ref_tag_prompt_copy_describe_initial_values() {
        let fork = input_prompt_copy(&super_lazygit_core::InputPromptOperation::ForkRemote {
            suggested_name: "upstream".to_string(),
            remote_url: "git@github.com:example/upstream.git".to_string(),
        });
        let tag = input_prompt_copy(
            &super_lazygit_core::InputPromptOperation::CreateTagFromRef {
                target_ref: "origin/feature".to_string(),
                source_label: "origin/feature".to_string(),
            },
        );

        assert!(fork.contains("<name> <url>"));
        assert!(fork.contains("upstream git@github.com:example/upstream.git"));
        assert!(tag.contains("new tag name"));
        assert!(tag.contains("origin/feature"));
    }

    #[test]
    fn remote_prompt_copy_mentions_name_and_url_format() {
        let create = input_prompt_copy(&super_lazygit_core::InputPromptOperation::CreateRemote);
        let edit = input_prompt_copy(&super_lazygit_core::InputPromptOperation::EditRemote {
            current_name: "upstream".to_string(),
            current_url: "git@github.com:example/upstream.git".to_string(),
        });

        assert!(create.contains("<name> <url>"));
        assert!(create.contains("upstream"));
        assert!(edit.contains("Edit remote upstream"));
        assert!(edit.contains("<name> <url>"));
    }

    #[test]
    fn submodule_prompt_and_confirmation_copy_describe_lifecycle_actions() {
        let create = input_prompt_copy(&super_lazygit_core::InputPromptOperation::CreateSubmodule);
        let edit = input_prompt_copy(
            &super_lazygit_core::InputPromptOperation::EditSubmoduleUrl {
                name: "child-module".to_string(),
                path: PathBuf::from("vendor/child-module"),
                current_url: "../child-module.git".to_string(),
            },
        );
        let remove =
            confirmation_copy(&super_lazygit_core::ConfirmableOperation::RemoveSubmodule {
                name: "child-module".to_string(),
                path: PathBuf::from("vendor/child-module"),
            });

        assert!(create.contains("<path> <url>"));
        assert!(edit.contains("submodule child-module"));
        assert!(edit.contains(".gitmodules"));
        assert!(remove.contains("deinitializes it"));
        assert!(remove.contains(".gitmodules"));
    }

    #[test]
    fn render_repo_shell_shows_submodule_details() {
        let mut state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            settings: super_lazygit_core::SettingsSnapshot {
                show_help_footer: true,
                ..Default::default()
            },
            workspace: WorkspaceState {
                discovered_repo_ids: vec![RepoId::new("/tmp/repo-1")],
                selected_repo_id: Some(RepoId::new("/tmp/repo-1")),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("/tmp/repo-1"),
                active_subview: RepoSubview::Submodules,
                detail: Some(sample_repo_detail()),
                submodules_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
            }),
            ..Default::default()
        };
        state.workspace.repo_summaries.insert(
            RepoId::new("/tmp/repo-1"),
            RepoSummary {
                repo_id: RepoId::new("/tmp/repo-1"),
                display_name: "repo-1".to_string(),
                display_path: "/tmp/repo-1".to_string(),
                branch: Some("main".to_string()),
                ..Default::default()
            },
        );
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(100, 18);

        let rendered = app.render_to_string();
        let help = repo_help_text(app.state());

        assert!(rendered.contains("Detail: Submodules"));
        assert!(rendered.contains("Selected: vendor/ui-kit"));
        assert!(rendered.contains("State: uninitialized"));
        assert!(rendered.contains("URL: git@github.com:example/ui-kit.git"));
        assert!(help.contains("Ctrl+O copy submodule"));
        assert!(help.contains("b options menu"));
        assert!(rendered.contains("n add."));
        assert!(rendered.contains("e edit URL."));
        assert!(rendered.contains("i init."));
        assert!(rendered.contains("u update."));
        assert!(rendered.contains("o open path."));
        assert!(rendered.contains("vendor/child-module  [main]  fedcba9  clean"));
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
        assert!(rendered.contains("Context: Enter/Space switch. 0 main. / filter."));
        assert!(rendered.contains("o open selected worktree"));
        assert!(rendered.contains("d remove"));
        assert!(rendered.contains("/tmp/repo-1  [main]"));
        assert!(rendered.contains("/tmp/repo-1-feature  [feature]"));
    }

    #[test]
    fn render_repo_shell_shows_worktree_filter_empty_state() {
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
                worktree_filter: super_lazygit_core::RepoSubviewFilterState {
                    query: "qxz".to_string(),
                    focused: true,
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
        assert!(rendered.contains("Filter /qxz_"));
        assert!(rendered.contains("No worktrees match /qxz."));
    }

    #[test]
    fn route_repository_submodule_keys_cover_subview_and_actions() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let base_state = |active_subview| AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview,
                detail: Some(sample_repo_detail()),
                submodules_view: super_lazygit_core::ListViewState {
                    selected_index: Some(0),
                },
                ..RepoModeState::new(repo_id.clone())
            }),
            ..Default::default()
        };

        let mut app = TuiApp::new(base_state(RepoSubview::Status), AppConfig::default());
        let switched = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "b".to_string(),
        })));
        assert_eq!(
            switched
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview),
            Some(RepoSubview::Submodules)
        );

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let created = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "n".to_string(),
        })));
        assert_eq!(
            created
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::CreateSubmodule)
        );

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let copied = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "ctrl+o".to_string(),
        })));
        assert!(matches!(
            copied.effects.as_slice(),
            [super_lazygit_core::Effect::RunShellCommand(
                super_lazygit_core::ShellCommandRequest { command, .. }
            )] if command.contains("child-module")
        ));

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let menu = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "b".to_string(),
        })));
        assert_eq!(
            menu.state
                .pending_menu
                .as_ref()
                .map(|pending| pending.operation),
            Some(super_lazygit_core::MenuOperation::SubmoduleOptions)
        );

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let edited = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "e".to_string(),
        })));
        assert_eq!(
            edited
                .state
                .pending_input_prompt
                .as_ref()
                .map(|prompt| prompt.operation.clone()),
            Some(super_lazygit_core::InputPromptOperation::EditSubmoduleUrl {
                name: "child-module".to_string(),
                path: PathBuf::from("vendor/child-module"),
                current_url: "../child-module.git".to_string(),
            })
        );

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let initialized = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "i".to_string(),
        })));
        assert!(matches!(
            initialized.effects.as_slice(),
            [super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::InitSubmodule { path },
                ..
            })] if path == &PathBuf::from("vendor/child-module")
        ));

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let updated = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "u".to_string(),
        })));
        assert!(matches!(
            updated.effects.as_slice(),
            [super_lazygit_core::Effect::RunGitCommand(super_lazygit_core::GitCommandRequest {
                command: super_lazygit_core::GitCommand::UpdateSubmodule { path },
                ..
            })] if path == &PathBuf::from("vendor/child-module")
        ));

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let removed = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "d".to_string(),
        })));
        assert_eq!(
            removed
                .state
                .pending_confirmation
                .as_ref()
                .map(|pending| pending.operation.clone()),
            Some(super_lazygit_core::ConfirmableOperation::RemoveSubmodule {
                name: "child-module".to_string(),
                path: PathBuf::from("vendor/child-module"),
            })
        );

        let mut app = TuiApp::new(base_state(RepoSubview::Submodules), AppConfig::default());
        let entered = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));
        assert_eq!(
            entered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.current_repo_id.clone()),
            Some(RepoId::new("/tmp/repo-1/vendor/child-module"))
        );
        assert_eq!(
            entered
                .state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.parent_repo_ids.clone()),
            Some(vec![repo_id])
        );
    }

    #[test]
    fn route_repository_commit_history_b_opens_bisect_menu() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: std::collections::BTreeMap::from([(
                    repo_id.clone(),
                    workspace_repo_summary(&repo_id.0, "repo-1"),
                )]),
                selected_repo_id: Some(repo_id.clone()),
                ..Default::default()
            },
            repo_mode: Some(RepoModeState {
                current_repo_id: repo_id.clone(),
                active_subview: RepoSubview::Commits,
                detail: Some(sample_repo_detail()),
                commits_view: super_lazygit_core::ListViewState {
                    selected_index: Some(1),
                },
                ..RepoModeState::new(repo_id)
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "b".to_string(),
        })));

        assert_eq!(result.state.focused_pane, PaneId::Modal);
        assert_eq!(
            result
                .state
                .pending_menu
                .as_ref()
                .map(|menu| menu.operation),
            Some(super_lazygit_core::MenuOperation::BisectOptions)
        );
    }

    #[test]
    fn render_commit_history_explicit_ref_surfaces_current_branch_shortcut() {
        let repo_id = RepoId::new("/tmp/repo-1");
        let mut app = TuiApp::new(
            AppState {
                mode: AppMode::Repository,
                focused_pane: PaneId::RepoDetail,
                settings: super_lazygit_core::SettingsSnapshot {
                    show_help_footer: true,
                    ..Default::default()
                },
                workspace: WorkspaceState {
                    discovered_repo_ids: vec![repo_id.clone()],
                    selected_repo_id: Some(repo_id.clone()),
                    ..Default::default()
                },
                repo_mode: Some(RepoModeState {
                    current_repo_id: repo_id,
                    active_subview: RepoSubview::Commits,
                    commit_history_ref: Some("feature".to_string()),
                    detail: Some(sample_repo_detail()),
                    ..RepoModeState::new(RepoId::new("/tmp/repo-1"))
                }),
                ..Default::default()
            },
            AppConfig::default(),
        );
        app.resize(100, 22);

        let rendered = app.render_to_string();

        assert!(rendered.contains("3 returns to current-branch history"));
        let help = repo_help_text(app.state());
        assert!(help.contains("a amend attrs"));
        assert!(help.contains("Ctrl+O copy hash"));
        assert!(help.contains("o browser"));
        assert!(help.contains("C copy"));
        assert!(help.contains("c set fixup msg"));
        assert!(help.contains("t revert"));
        assert!(help.contains("Ctrl+L log menu"));
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
