use std::time::Instant;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use super_lazygit_config::AppConfig;
use super_lazygit_core::{
    reduce, Action, AppMode, AppState, Diagnostics, DiagnosticsSnapshot, DiffLineKind, Event,
    InputEvent, KeyPress, PaneId, ReduceResult, RepoDetail, RepoId, RepoSubview, RepoSummary,
};

#[derive(Debug)]
pub struct TuiApp {
    state: AppState,
    config: AppConfig,
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
    pub fn new(state: AppState, config: AppConfig) -> Self {
        Self {
            state,
            config,
            diagnostics: Diagnostics::default(),
            viewport: Viewport::default(),
        }
    }

    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
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
            Paragraph::new(vec![
                Line::from(Span::styled(
                    modal.title.clone(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("{:?}", modal.kind)),
                Line::from("Esc closes this overlay."),
            ])
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
            InputEvent::Paste(_) => ReduceResult {
                state: self.state.clone(),
                effects: Vec::new(),
            },
        }
    }

    fn route_key(&self, key: KeyPress) -> Option<Action> {
        let trimmed = key.key.trim();
        let normalized = trimmed.to_ascii_lowercase();

        if !self.state.modal_stack.is_empty() {
            return match normalized.as_str() {
                "esc" | "q" => Some(Action::CloseTopModal),
                _ => None,
            };
        }

        match normalized.as_str() {
            "?" => {
                return Some(Action::OpenModal {
                    kind: super_lazygit_core::ModalKind::Help,
                    title: "Help".to_string(),
                })
            }
            "tab" => return self.next_focus_action(),
            "shift+tab" => return self.previous_focus_action(),
            "esc" if matches!(self.state.mode, AppMode::Repository) => {
                return Some(Action::LeaveRepoMode)
            }
            _ => {}
        }

        match self.state.mode {
            AppMode::Workspace => self.route_workspace_key(&normalized),
            AppMode::Repository => self.route_repo_key(trimmed, &normalized),
        }
    }

    fn route_workspace_key(&self, key: &str) -> Option<Action> {
        match key {
            "j" | "down" => Some(Action::SelectNextRepo),
            "k" | "up" => Some(Action::SelectPreviousRepo),
            "l" | "right" => Some(Action::SetFocusedPane(PaneId::WorkspacePreview)),
            "h" | "left" => Some(Action::SetFocusedPane(PaneId::WorkspaceList)),
            "enter" => self
                .state
                .workspace
                .selected_repo_id
                .clone()
                .map(|repo_id| Action::EnterRepoMode { repo_id }),
            "r" => Some(Action::RefreshVisibleRepos),
            _ => None,
        }
    }

    fn route_repo_key(&self, raw: &str, normalized: &str) -> Option<Action> {
        if raw == "P" {
            return Some(Action::PushCurrentBranch);
        }

        if self.state.focused_pane == PaneId::RepoDetail
            && self.state.repo_mode.as_ref().is_some_and(|repo_mode| {
                matches!(
                    repo_mode.active_subview,
                    RepoSubview::Status | RepoSubview::Commits
                )
            })
        {
            if let Some(repo_mode) = self.state.repo_mode.as_ref() {
                match (repo_mode.active_subview, normalized) {
                    (RepoSubview::Status, "j" | "down") => {
                        return Some(Action::ScrollRepoDetailDown);
                    }
                    (RepoSubview::Status, "k" | "up") => {
                        return Some(Action::ScrollRepoDetailUp);
                    }
                    (RepoSubview::Commits, "j" | "down") => {
                        return Some(Action::SelectNextCommit);
                    }
                    (RepoSubview::Commits, "k" | "up") => {
                        return Some(Action::SelectPreviousCommit);
                    }
                    _ => {}
                }
            }
        }

        match normalized {
            "h" | "left" => self.repo_focus_left_action(),
            "l" | "right" => self.repo_focus_right_action(),
            "1" => Some(Action::SwitchRepoSubview(RepoSubview::Status)),
            "2" => Some(Action::SwitchRepoSubview(RepoSubview::Branches)),
            "3" => Some(Action::SwitchRepoSubview(RepoSubview::Commits)),
            "4" => Some(Action::SwitchRepoSubview(RepoSubview::Stash)),
            "5" => Some(Action::SwitchRepoSubview(RepoSubview::Reflog)),
            "6" => Some(Action::SwitchRepoSubview(RepoSubview::Worktrees)),
            "r" => Some(Action::RefreshSelectedRepo),
            "f" => Some(Action::FetchSelectedRepo),
            "p" => Some(Action::PullCurrentBranch),
            _ => None,
        }
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
                "Repo: {}  Branch: {}",
                title,
                self.selected_summary()
                    .and_then(|summary| summary.branch.as_deref())
                    .unwrap_or("detached")
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
        let mut lines = vec![Line::from(self.workspace_root_label())];
        let repo_ids = &self.state.workspace.discovered_repo_ids;

        if repo_ids.is_empty() {
            lines.push(Line::from("No repositories discovered yet."));
        } else {
            for repo_id in repo_ids {
                let is_selected = self
                    .state
                    .workspace
                    .selected_repo_id
                    .as_ref()
                    .is_some_and(|selected| selected == repo_id);
                let summary = self.state.workspace.repo_summaries.get(repo_id);
                lines.push(repo_line(repo_id, summary, is_selected));
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
            self.selected_summary(),
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
        let lines = repo_staged_lines(
            self.selected_summary(),
            self.state.focused_pane == PaneId::RepoStaged,
        );

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Staged changes")
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::RepoStaged, theme)),
            )
            .render(area, buffer);
    }

    fn render_repo_detail(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let (title, lines) = if let Some(repo_mode) = &self.state.repo_mode {
            let lines = match repo_mode.active_subview {
                RepoSubview::Status => repo_diff_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.diff_scroll,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                RepoSubview::Commits => repo_commit_lines(
                    repo_mode.detail.as_ref(),
                    repo_mode.commits_view.selected_index,
                    usize::from(area.height.saturating_sub(2)),
                    theme,
                ),
                _ => repo_detail_lines(repo_mode.active_subview, repo_mode.detail.as_ref()),
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
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
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

fn repo_line(repo_id: &RepoId, summary: Option<&RepoSummary>, is_selected: bool) -> Line<'static> {
    let prefix = if is_selected { ">" } else { " " };
    let name = summary.map_or(repo_id.0.as_str(), |summary| summary.display_name.as_str());
    let branch = summary
        .and_then(|summary| summary.branch.as_deref())
        .unwrap_or("-");
    let dirty = summary.is_some_and(|summary| summary.dirty);
    Line::from(format!("{prefix} {name} [{branch}] dirty={dirty}"))
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

    vec![
        Line::from(format!("Path: {}", summary.display_path)),
        Line::from(format!(
            "Branch: {}",
            summary.branch.as_deref().unwrap_or("detached")
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
    ]
}

fn repo_detail_lines(subview: RepoSubview, detail: Option<&RepoDetail>) -> Vec<Line<'static>> {
    match subview {
        RepoSubview::Status => vec![
            Line::from(format!(
                "Files: {}",
                detail.map_or(0, |detail| detail.file_tree.len())
            )),
            Line::from("Diff viewer and tree interactions land in the next repo-mode beads."),
            Line::from("Focus here for status details."),
        ],
        RepoSubview::Branches => vec![
            Line::from(format!(
                "Branches: {}",
                detail.map_or(0, |detail| detail.branches.len())
            )),
            Line::from("Checkout and branch management land after the shell bead."),
        ],
        RepoSubview::Commits => vec![
            Line::from(format!(
                "Commits: {}",
                detail.map_or(0, |detail| detail.commits.len())
            )),
            Line::from("Commit history preview is active when this pane has focus."),
        ],
        RepoSubview::Stash => vec![
            Line::from(format!(
                "Stashes: {}",
                detail.map_or(0, |detail| detail.stashes.len())
            )),
            Line::from("Stash flows are staged behind this shell scaffold."),
        ],
        RepoSubview::Reflog => vec![
            Line::from(format!(
                "Reflog entries: {}",
                detail.map_or(0, |detail| detail.reflog_items.len())
            )),
            Line::from("Recovery-oriented navigation lands in later beads."),
        ],
        RepoSubview::Worktrees => vec![
            Line::from(format!(
                "Worktrees: {}",
                detail.map_or(0, |detail| detail.worktrees.len())
            )),
            Line::from("Worktree creation and removal reuse this detail shell."),
        ],
    }
}

fn repo_commit_lines(
    detail: Option<&RepoDetail>,
    selected_index: Option<usize>,
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
    let compare_target = detail
        .comparison_target
        .as_ref()
        .map(|target| match target {
            super_lazygit_core::ComparisonTarget::Branch(name)
            | super_lazygit_core::ComparisonTarget::Commit(name) => name.as_str(),
        })
        .unwrap_or("-");

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
        Line::from(format!("Compare target: {compare_target}")),
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
                commit_file_kind_label(file.kind),
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
        lines.extend(
            selected
                .diff
                .lines
                .iter()
                .take(remaining)
                .map(|line| render_diff_line(line.kind, &line.content, theme)),
        );
    }

    lines.truncate(viewport_lines.max(1));
    lines
}

fn repo_diff_lines(
    detail: Option<&RepoDetail>,
    scroll: usize,
    viewport_lines: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
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

    let header_lines = 2;
    let visible_capacity = viewport_lines.saturating_sub(header_lines).max(1);
    let max_scroll = detail.diff.lines.len().saturating_sub(visible_capacity);
    let scroll = scroll.min(max_scroll);
    let end = (scroll + visible_capacity).min(detail.diff.lines.len());

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!("Path: {selected}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "Hunks: {}  Lines: {}  Showing {}-{}",
            detail.diff.hunk_count,
            detail.diff.lines.len(),
            scroll + 1,
            end
        )),
    ];

    lines.extend(
        detail.diff.lines[scroll..end]
            .iter()
            .map(|line| render_diff_line(line.kind, &line.content, theme)),
    );
    lines
}

fn commit_file_kind_label(kind: super_lazygit_core::FileStatusKind) -> &'static str {
    match kind {
        super_lazygit_core::FileStatusKind::Added => "A",
        super_lazygit_core::FileStatusKind::Deleted => "D",
        super_lazygit_core::FileStatusKind::Renamed => "R",
        super_lazygit_core::FileStatusKind::Untracked => "?",
        super_lazygit_core::FileStatusKind::Conflicted => "U",
        super_lazygit_core::FileStatusKind::Modified => "M",
    }
}

fn render_diff_line(kind: DiffLineKind, content: &str, theme: Theme) -> Line<'static> {
    let style = match kind {
        DiffLineKind::Meta => Style::default().fg(theme.muted),
        DiffLineKind::HunkHeader => Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
        DiffLineKind::Addition => Style::default().fg(theme.success),
        DiffLineKind::Removal => Style::default().fg(theme.danger),
        DiffLineKind::Context => Style::default().fg(theme.foreground),
    };
    Line::from(Span::styled(content.to_string(), style))
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
        return "Esc close  q close overlay".to_string();
    }

    match state.mode {
        AppMode::Workspace => {
            "j/k move  Enter open repo  Tab swap pane  ? help  r refresh".to_string()
        }
        AppMode::Repository => repo_help_text(state),
    }
}

fn repo_unstaged_lines(
    summary: Option<&RepoSummary>,
    is_focused: bool,
    progress: &super_lazygit_core::OperationProgress,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(if is_focused {
        "Focus here for status-tree navigation."
    } else {
        "Move focus here to inspect working tree changes."
    })];

    if let Some(summary) = summary {
        lines.push(Line::from(format!("Modified: {}", summary.unstaged_count)));
        lines.push(Line::from(format!(
            "Untracked: {}",
            summary.untracked_count
        )));
        lines.push(Line::from(format!(
            "Conflicts: {}",
            if summary.conflicted { "yes" } else { "no" }
        )));
        lines.push(Line::from(format!(
            "Progress: {}",
            operation_progress_label(progress)
        )));
    } else {
        lines.push(Line::from("Summary pending..."));
    }

    lines
}

fn repo_staged_lines(summary: Option<&RepoSummary>, is_focused: bool) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(if is_focused {
        "Focus here for staging and commit prep."
    } else {
        "Move focus here to prep staged work."
    })];

    if let Some(summary) = summary {
        lines.push(Line::from(format!("Staged: {}", summary.staged_count)));
        lines.push(Line::from(format!(
            "Branch: {}",
            summary.branch.as_deref().unwrap_or("detached")
        )));
        lines.push(Line::from(format!(
            "Remote delta: +{} / -{}",
            summary.ahead_count, summary.behind_count
        )));
    } else {
        lines.push(Line::from("Summary pending..."));
    }

    lines
}

fn repo_subview_label(subview: RepoSubview) -> &'static str {
    match subview {
        RepoSubview::Status => "Status",
        RepoSubview::Branches => "Branches",
        RepoSubview::Commits => "Commits",
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
        (RepoSubview::Stash, "4 Stash"),
        (RepoSubview::Reflog, "5 Reflog"),
        (RepoSubview::Worktrees, "6 Worktrees"),
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
        AppMode::Workspace => "Select a repository and press Enter to open repo mode.".to_string(),
        AppMode::Repository => match state.focused_pane {
            PaneId::RepoUnstaged => {
                "Working tree focus; file actions attach here in the next beads.".to_string()
            }
            PaneId::RepoStaged => {
                "Staged focus; commit and amend flows attach to this pane.".to_string()
            }
            PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
                || "Repository shell ready.".to_string(),
                |repo_mode| {
                    if repo_mode.active_subview == RepoSubview::Status {
                        "Status diff focus; j/k scroll through hunks and keep orientation here."
                            .to_string()
                    } else if repo_mode.active_subview == RepoSubview::Commits {
                        "Commits detail focus; j/k browse history and keep the selected commit compare-ready."
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
        },
    }
}

fn repo_help_text(state: &AppState) -> String {
    match state.focused_pane {
        PaneId::RepoUnstaged => {
            "Working tree pane  l next pane  1-6 detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoStaged => {
            "Staged pane  h/l change pane  1-6 detail view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
        }
        PaneId::RepoDetail => state.repo_mode.as_ref().map_or_else(
            || "Repository shell".to_string(),
            |repo_mode| {
                if repo_mode.active_subview == RepoSubview::Status {
                    "Status diff pane  j/k scroll diff  h left pane  1-6 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else if repo_mode.active_subview == RepoSubview::Commits {
                    "Commits pane  j/k move commit  h left pane  1-6 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace".to_string()
                } else {
                    format!(
                        "{} detail pane  h left pane  1-6 switch view  f fetch  p pull  P push  Tab cycle panes  ? help  Esc workspace",
                        repo_subview_label(repo_mode.active_subview)
                    )
                }
            },
        ),
        _ => "Repository shell".to_string(),
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
        CommitFileItem, CommitItem, ComparisonTarget, DiffLine, DiffLineKind, DiffModel,
        FileStatus, FileStatusKind, ModalKind, RepoModeState, StatusMessage, WorkspaceState,
    };

    fn sample_repo_detail() -> RepoDetail {
        RepoDetail {
            file_tree: vec![
                FileStatus {
                    path: PathBuf::from("src/lib.rs"),
                    kind: FileStatusKind::Modified,
                },
                FileStatus {
                    path: PathBuf::from("README.md"),
                    kind: FileStatusKind::Untracked,
                },
            ],
            diff: DiffModel {
                selected_path: Some(PathBuf::from("src/lib.rs")),
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
                        content: "@@ -1,1 +1,2 @@".to_string(),
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
                hunk_count: 1,
            },
            branches: vec![Default::default(), Default::default()],
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
                        lines: vec![
                            DiffLine {
                                kind: DiffLineKind::Meta,
                                content: "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::HunkHeader,
                                content: "@@ -0,0 +1,3 @@".to_string(),
                            },
                            DiffLine {
                                kind: DiffLineKind::Addition,
                                content: "+pub fn answer() -> u32 {".to_string(),
                            },
                        ],
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
                        hunk_count: 0,
                    },
                },
            ],
            comparison_target: Some(ComparisonTarget::Commit("abcdef1234567890".to_string())),
            ..Default::default()
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
                ..Default::default()
            },
        );

        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Workspace"));
        assert!(rendered.contains("Preview"));
        assert!(rendered.contains("WORKSPACE"));
        assert!(rendered.contains("Ready to inspect"));
        assert!(rendered.contains("repo-1"));
        assert!(rendered.contains("dirty=true"));
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
        let mut app = TuiApp::new(state, AppConfig::default());

        let result = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "enter".to_string(),
        })));

        assert_eq!(result.state.mode, AppMode::Repository);
        assert_eq!(result.state.focused_pane, PaneId::RepoUnstaged);
        assert!(result.state.repo_mode.is_some());
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
            .any(|effect| matches!(effect, super_lazygit_core::Effect::RunGitCommand(_))));
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
                .and_then(|repo_mode| repo_mode.detail.as_ref())
                .and_then(|detail| detail.comparison_target.clone()),
            Some(ComparisonTarget::Commit("1234567890abcdef".to_string()))
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
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(1)
        );

        let repo_mode = app.state().repo_mode.as_ref().expect("repo mode");
        let visible_lines = repo_diff_lines(
            repo_mode.detail.as_ref(),
            repo_mode.diff_scroll,
            3,
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
        assert!(rendered_lines.contains(&"Path: src/lib.rs".to_string()));
        assert!(rendered_lines.contains(&"index 1111111..2222222 100644".to_string()));
        assert!(!rendered_lines
            .iter()
            .any(|line| line == "diff --git a/src/lib.rs b/src/lib.rs"));

        let up = app.dispatch(Event::Input(InputEvent::KeyPressed(KeyPress {
            key: "k".to_string(),
        })));
        assert_eq!(
            up.state
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.diff_scroll),
            Some(0)
        );
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
        assert!(rendered.contains("Path: src/lib.rs"));
        assert!(rendered.contains("Hunks: 1"));
        assert!(rendered.contains("@@ -1,1 +1,2 @@"));
        assert!(rendered.contains("+new line"));
        assert!(rendered.contains("Repository shell"));
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
        app.resize(100, 18);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Detail: Commits"));
        assert!(rendered.contains("Selected 1/2"));
        assert!(rendered.contains("Compare target: abcdef1234567890"));
        assert!(rendered.contains("> abcdef1 add lib"));
        assert!(rendered.contains("A src/lib.rs"));
        assert!(rendered.contains("+pub fn answer() -> u32 {"));
    }
}
