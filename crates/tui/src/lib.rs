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
    reduce, Action, AppMode, AppState, Diagnostics, DiagnosticsSnapshot, Event, InputEvent,
    KeyPress, PaneId, ReduceResult, RepoDetail, RepoId, RepoSubview, RepoSummary,
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
        let normalized = key.key.trim().to_ascii_lowercase();

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
            AppMode::Repository => self.route_repo_key(&normalized),
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

    fn route_repo_key(&self, key: &str) -> Option<Action> {
        match key {
            "h" | "left" => Some(Action::SetFocusedPane(PaneId::RepoStatus)),
            "l" | "right" => Some(Action::SetFocusedPane(PaneId::RepoDetail)),
            "1" => Some(Action::SwitchRepoSubview(RepoSubview::Status)),
            "2" => Some(Action::SwitchRepoSubview(RepoSubview::Branches)),
            "3" => Some(Action::SwitchRepoSubview(RepoSubview::Commits)),
            "4" => Some(Action::SwitchRepoSubview(RepoSubview::Stash)),
            "5" => Some(Action::SwitchRepoSubview(RepoSubview::Reflog)),
            "6" => Some(Action::SwitchRepoSubview(RepoSubview::Worktrees)),
            "r" => Some(Action::RefreshSelectedRepo),
            _ => None,
        }
    }

    fn next_focus_action(&self) -> Option<Action> {
        match self.state.mode {
            AppMode::Workspace => Some(Action::SetFocusedPane(match self.state.focused_pane {
                PaneId::WorkspaceList => PaneId::WorkspacePreview,
                _ => PaneId::WorkspaceList,
            })),
            AppMode::Repository => Some(Action::SetFocusedPane(match self.state.focused_pane {
                PaneId::RepoStatus => PaneId::RepoDetail,
                _ => PaneId::RepoStatus,
            })),
        }
    }

    fn previous_focus_action(&self) -> Option<Action> {
        self.next_focus_action()
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
        let panes = split_two_columns(area);
        self.render_repo_status(panes[0], buffer, theme);
        self.render_repo_detail(panes[1], buffer, theme);
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
                lines.push(Line::from(repo_line(repo_id, summary, is_selected)));
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

    fn render_repo_status(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let Some(repo_mode) = &self.state.repo_mode else {
            Paragraph::new("Enter repo mode to inspect repository details.")
                .block(
                    Block::default()
                        .title("Repository")
                        .borders(Borders::ALL)
                        .border_style(self.pane_style(PaneId::RepoStatus, theme)),
                )
                .render(area, buffer);
            return;
        };

        let summary = self.selected_summary();
        let mut lines = vec![
            Line::from(format!("Repo: {}", repo_mode.current_repo_id.0)),
            Line::from(format!("Subview: {:?}", repo_mode.active_subview)),
            Line::from(format!("Focus: {:?}", self.state.focused_pane)),
        ];
        if let Some(summary) = summary {
            lines.extend(workspace_preview_lines(summary));
        }
        lines.push(Line::from(format!(
            "Progress: {}",
            operation_progress_label(&repo_mode.operation_progress)
        )));

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Repo status")
                    .borders(Borders::ALL)
                    .border_style(self.pane_style(PaneId::RepoStatus, theme)),
            )
            .render(area, buffer);
    }

    fn render_repo_detail(&self, area: Rect, buffer: &mut Buffer, theme: Theme) {
        let lines = if let Some(repo_mode) = &self.state.repo_mode {
            repo_detail_lines(repo_mode.active_subview, repo_mode.detail.as_ref())
        } else {
            vec![Line::from("Repository detail will appear here.")]
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Repo detail")
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
    muted: Color,
}

impl Theme {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            background: parse_hex_color(&config.theme.colors.background).unwrap_or(Color::Black),
            foreground: parse_hex_color(&config.theme.colors.foreground).unwrap_or(Color::White),
            accent: parse_hex_color(&config.theme.colors.accent).unwrap_or(Color::Cyan),
            muted: Color::DarkGray,
        }
    }
}

fn split_two_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    std::rc::Rc::from(
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area),
    )
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
        RepoSubview::Status => vec![Line::from(format!(
            "Files: {}",
            detail.map_or(0, |detail| detail.file_tree.len())
        ))],
        RepoSubview::Branches => vec![Line::from(format!(
            "Branches: {}",
            detail.map_or(0, |detail| detail.branches.len())
        ))],
        RepoSubview::Commits => vec![Line::from(format!(
            "Commits: {}",
            detail.map_or(0, |detail| detail.commits.len())
        ))],
        RepoSubview::Stash => vec![Line::from(format!(
            "Stashes: {}",
            detail.map_or(0, |detail| detail.stashes.len())
        ))],
        RepoSubview::Reflog => vec![Line::from(format!(
            "Reflog entries: {}",
            detail.map_or(0, |detail| detail.reflog_items.len())
        ))],
        RepoSubview::Worktrees => vec![Line::from(format!(
            "Worktrees: {}",
            detail.map_or(0, |detail| detail.worktrees.len())
        ))],
    }
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
        .unwrap_or_else(|| "Ready".to_string())
}

fn help_text(state: &AppState) -> String {
    if !state.modal_stack.is_empty() {
        return "Esc close  q close overlay".to_string();
    }

    match state.mode {
        AppMode::Workspace => {
            "j/k move  Enter open repo  Tab swap pane  ? help  r refresh".to_string()
        }
        AppMode::Repository => {
            "h/l focus  1-6 subviews  Esc workspace  Tab swap pane  ? help".to_string()
        }
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
    use super::*;
    use super_lazygit_core::{ModalKind, RepoModeState, StatusMessage, WorkspaceState};

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
        assert_eq!(result.state.focused_pane, PaneId::RepoStatus);
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
            focused_pane: PaneId::RepoStatus,
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
    fn diagnostics_snapshot_includes_render_samples() {
        let mut app = TuiApp::new(AppState::default(), AppConfig::default());

        let _ = app.render();

        assert_eq!(app.diagnostics_snapshot().renders.len(), 1);
    }

    #[test]
    fn render_repo_detail_uses_loaded_detail_counts() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                current_repo_id: RepoId::new("repo-1"),
                active_subview: RepoSubview::Branches,
                detail: Some(RepoDetail {
                    branches: vec![Default::default(), Default::default()],
                    ..Default::default()
                }),
                ..RepoModeState::new(RepoId::new("repo-1"))
            }),
            ..Default::default()
        };
        let mut app = TuiApp::new(state, AppConfig::default());
        app.resize(80, 20);

        let rendered = app.render_to_string();

        assert!(rendered.contains("Branches: 2"));
    }
}
