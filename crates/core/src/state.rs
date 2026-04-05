use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::lines::split_lines;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub mode: AppMode,
    pub focused_pane: PaneId,
    pub modal_stack: Vec<Modal>,
    pub pending_confirmation: Option<PendingConfirmation>,
    pub pending_input_prompt: Option<PendingInputPrompt>,
    pub pending_menu: Option<PendingMenu>,
    pub return_context_stack: Vec<ReturnContext>,
    pub status_messages: VecDeque<StatusMessage>,
    pub notifications: VecDeque<Notification>,
    pub background_jobs: BTreeMap<JobId, BackgroundJob>,
    pub settings: SettingsSnapshot,
    pub background: BackgroundSettingsSnapshot,
    pub service_domains: BTreeMap<String, String>,
    pub os: OsConfigSnapshot,
    pub config_path: Option<PathBuf>,
    pub repository_url: Option<String>,
    pub recent_repo_stack: Vec<RepoId>,
    pub workspace: WorkspaceState,
    pub repo_mode: Option<RepoModeState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundSettingsSnapshot {
    pub auto_fetch: bool,
    pub auto_refresh: bool,
    pub show_bottom_line: bool,
}

impl Default for BackgroundSettingsSnapshot {
    fn default() -> Self {
        Self {
            auto_fetch: true,
            auto_refresh: true,
            show_bottom_line: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsConfigSnapshot {
    pub open: String,
    pub open_link: String,
    pub copy_to_clipboard_cmd: String,
    pub read_from_clipboard_cmd: String,
    pub shell_functions_file: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMode {
    #[default]
    Workspace,
    Repository,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneId {
    #[default]
    WorkspaceList,
    WorkspacePreview,
    RepoUnstaged,
    RepoStaged,
    RepoDetail,
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiContextId {
    WorkspaceList,
    WorkspacePreview,
    RepoUnstaged,
    RepoStaged,
    RepoStatus,
    RepoBranches,
    RepoRemotes,
    RepoRemoteBranches,
    RepoTags,
    RepoCommits,
    RepoCompare,
    RepoRebase,
    RepoStash,
    RepoReflog,
    RepoWorktrees,
    RepoSubmodules,
    ModalHelp,
    ModalConfirm,
    ModalMenu,
    ModalCommandPalette,
    ModalInputPrompt,
}

impl AppState {
    #[must_use]
    pub fn active_context_id(&self) -> UiContextId {
        if let Some(modal) = self.modal_stack.last() {
            return match modal.kind {
                ModalKind::Help => UiContextId::ModalHelp,
                ModalKind::Confirm => UiContextId::ModalConfirm,
                ModalKind::Menu => UiContextId::ModalMenu,
                ModalKind::CommandPalette => UiContextId::ModalCommandPalette,
                ModalKind::InputPrompt => UiContextId::ModalInputPrompt,
            };
        }

        match self.focused_pane {
            PaneId::WorkspaceList => UiContextId::WorkspaceList,
            PaneId::WorkspacePreview => UiContextId::WorkspacePreview,
            PaneId::RepoUnstaged => UiContextId::RepoUnstaged,
            PaneId::RepoStaged => UiContextId::RepoStaged,
            PaneId::RepoDetail => match self
                .repo_mode
                .as_ref()
                .map(|repo_mode| repo_mode.active_subview)
                .unwrap_or_default()
            {
                RepoSubview::Status => UiContextId::RepoStatus,
                RepoSubview::Branches => UiContextId::RepoBranches,
                RepoSubview::Remotes => UiContextId::RepoRemotes,
                RepoSubview::RemoteBranches => UiContextId::RepoRemoteBranches,
                RepoSubview::Tags => UiContextId::RepoTags,
                RepoSubview::Commits => UiContextId::RepoCommits,
                RepoSubview::Compare => UiContextId::RepoCompare,
                RepoSubview::Rebase => UiContextId::RepoRebase,
                RepoSubview::Stash => UiContextId::RepoStash,
                RepoSubview::Reflog => UiContextId::RepoReflog,
                RepoSubview::Worktrees => UiContextId::RepoWorktrees,
                RepoSubview::Submodules => UiContextId::RepoSubmodules,
            },
            PaneId::Modal => UiContextId::ModalMenu,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenMode {
    #[default]
    Normal,
    HalfScreen,
    FullScreen,
}

impl ScreenMode {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::HalfScreen,
            Self::HalfScreen => Self::FullScreen,
            Self::FullScreen => Self::Normal,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Normal => Self::FullScreen,
            Self::HalfScreen => Self::Normal,
            Self::FullScreen => Self::HalfScreen,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::HalfScreen => "half",
            Self::FullScreen => "fullscreen",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modal {
    pub kind: ModalKind,
    pub title: String,
}

impl Modal {
    #[must_use]
    pub fn new(kind: ModalKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModalKind {
    Help,
    Confirm,
    Menu,
    CommandPalette,
    InputPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingConfirmation {
    pub repo_id: RepoId,
    pub operation: ConfirmableOperation,
    pub return_focus: PaneId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Mixed => "mixed",
            Self::Hard => "hard",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Mixed => "Mixed",
            Self::Hard => "Hard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmableOperation {
    Fetch,
    Pull,
    Push,
    FetchRemote {
        remote_name: String,
    },
    DiscardFile {
        path: PathBuf,
    },
    StartInteractiveRebase {
        commit: String,
        summary: String,
    },
    AmendCommit {
        commit: String,
        summary: String,
    },
    ApplyFixupCommits {
        commit: String,
        summary: String,
    },
    FindBaseCommitForFixup {
        pending_selection: String,
        stage_all: bool,
    },
    FixupCommit {
        commit: String,
        summary: String,
    },
    SetFixupMessageForCommit {
        commit: String,
        summary: String,
        keep_message: bool,
    },
    SquashCommit {
        commit: String,
        summary: String,
    },
    DropCommit {
        commit: String,
        summary: String,
    },
    MoveCommitUp {
        commit: String,
        adjacent_commit: String,
        summary: String,
        adjacent_summary: String,
    },
    MoveCommitDown {
        commit: String,
        adjacent_commit: String,
        summary: String,
        adjacent_summary: String,
    },
    RewordCommitInEditor {
        commit: String,
        summary: String,
    },
    CherryPickCommit {
        commit: String,
        summary: String,
    },
    RevertCommit {
        commit: String,
        summary: String,
    },
    ResetToCommit {
        mode: ResetMode,
        commit: String,
        summary: String,
    },
    RestoreReflogEntry {
        target: String,
        summary: String,
    },
    AbortRebase,
    SkipRebase,
    NukeWorkingTree,
    DeleteBranch {
        branch_name: String,
        force: bool,
    },
    UnsetBranchUpstream {
        branch_name: String,
    },
    FastForwardCurrentBranchFromUpstream {
        branch_name: String,
        upstream_ref: String,
    },
    ForceCheckoutRef {
        target_ref: String,
        source_label: String,
    },
    MergeRefIntoCurrent {
        target_ref: String,
        source_label: String,
        variant: MergeVariant,
    },
    RebaseCurrentBranchOntoRef {
        target_ref: String,
        source_label: String,
    },
    RemoveRemote {
        remote_name: String,
    },
    DeleteRemoteBranch {
        remote_name: String,
        branch_name: String,
    },
    DeleteTag {
        tag_name: String,
    },
    PushTag {
        remote_name: String,
        tag_name: String,
    },
    PopStash {
        stash_ref: String,
    },
    DropStash {
        stash_ref: String,
    },
    RemoveWorktree {
        path: PathBuf,
        force: bool,
    },
    RemoveSubmodule {
        name: String,
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitFlowBranchType {
    Feature,
    Hotfix,
    Bugfix,
    Release,
}

impl GitFlowBranchType {
    #[must_use]
    pub const fn command_name(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Hotfix => "hotfix",
            Self::Bugfix => "bugfix",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInputPrompt {
    pub repo_id: RepoId,
    pub operation: InputPromptOperation,
    pub value: String,
    pub return_focus: PaneId,
    pub suggestions: Vec<PromptSuggestion>,
    pub selected_suggestion: usize,
    pub suggestion_provider: Option<PromptSuggestionProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSuggestion {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptSuggestionProvider {
    CheckoutBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnContext {
    pub pane: PaneId,
    pub repo_subview: Option<RepoSubview>,
}

impl ReturnContext {
    #[must_use]
    pub const fn new(pane: PaneId, repo_subview: Option<RepoSubview>) -> Self {
        Self { pane, repo_subview }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputPromptOperation {
    CheckoutBranch,
    CreateBranch,
    StartGitFlow {
        branch_type: GitFlowBranchType,
    },
    CreateRemote,
    CreateRemoteUrl {
        remote_name: String,
    },
    ForkRemote {
        suggested_name: String,
        remote_url: String,
    },
    CreateTag,
    CreateTagFromCommit {
        commit: String,
        summary: String,
    },
    CreateTagFromRef {
        target_ref: String,
        source_label: String,
    },
    CreateBranchFromCommit {
        commit: String,
        summary: String,
    },
    CreateBranchFromRemote {
        remote_branch_ref: String,
        suggested_name: String,
    },
    RenameBranch {
        current_name: String,
    },
    EditRemote {
        current_name: String,
        current_url: String,
    },
    EditRemoteUrl {
        current_name: String,
        new_name: String,
        current_url: String,
    },
    RenameStash {
        stash_ref: String,
        current_name: String,
    },
    CreateBranchFromStash {
        stash_ref: String,
        stash_label: String,
    },
    SetBranchUpstream {
        branch_name: String,
    },
    CreateStash {
        mode: StashMode,
    },
    CreateWorktree,
    CreateSubmodule,
    ShellCommand,
    EditSubmoduleUrl {
        name: String,
        path: PathBuf,
        current_url: String,
    },
    CreateAmendCommit {
        summary: String,
        original_subject: String,
        include_file_changes: bool,
        initial_message: String,
    },
    SetCommitCoAuthor {
        commit: String,
        summary: String,
    },
    RewordCommit {
        commit: String,
        summary: String,
        initial_message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMenu {
    pub repo_id: RepoId,
    pub operation: MenuOperation,
    pub selected_index: usize,
    pub return_focus: PaneId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuOperation {
    StashOptions,
    FilterOptions,
    DiffOptions,
    CommitLogOptions,
    CommitCopyOptions,
    BranchGitFlowOptions,
    BranchPullRequestOptions,
    BranchResetOptions,
    BranchSortOptions,
    TagResetOptions,
    ReflogResetOptions,
    CommitAmendAttributeOptions,
    CommitFixupOptions,
    CommitSetFixupMessageOptions,
    BisectOptions,
    BranchUpstreamOptions,
    MergeRebaseOptions,
    RemoteBranchPullRequestOptions,
    RemoteBranchResetOptions,
    RemoteBranchSortOptions,
    IgnoreOptions,
    StatusResetOptions,
    PatchOptions,
    BulkSubmoduleOptions,
    RecentRepos,
    CommandLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StashMode {
    Tracked,
    KeepIndex,
    IncludeUntracked,
    Staged,
    Unstaged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusMessage {
    pub id: u64,
    pub level: MessageLevel,
    pub text: String,
    pub command_log_kind: CommandLogKind,
}

impl StatusMessage {
    #[must_use]
    pub fn info(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            level: MessageLevel::Info,
            text: text.into(),
            command_log_kind: CommandLogKind::Message,
        }
    }

    #[must_use]
    pub fn error(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            level: MessageLevel::Error,
            text: text.into(),
            command_log_kind: CommandLogKind::Message,
        }
    }

    #[must_use]
    pub fn command_log_action(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            level: MessageLevel::Info,
            text: text.into(),
            command_log_kind: CommandLogKind::Action,
        }
    }

    #[must_use]
    pub fn command_log_command(id: u64, text: impl Into<String>, command_line: bool) -> Self {
        Self {
            id,
            level: MessageLevel::Info,
            text: text.into(),
            command_log_kind: CommandLogKind::Command { command_line },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandLogKind {
    Message,
    Action,
    Command { command_line: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub id: u64,
    pub level: MessageLevel,
    pub text: String,
    pub expires_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub theme_name: String,
    pub keymap_name: String,
    pub confirm_destructive: bool,
    pub show_help_footer: bool,
    pub show_list_footer: bool,
    pub show_file_tree: bool,
    pub show_root_item_in_file_tree: bool,
    pub screen_mode: ScreenMode,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RepoId(pub String);

impl RepoId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl Default for RepoId {
    fn default() -> Self {
        Self("default".to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub current_root: Option<PathBuf>,
    pub discovered_repo_ids: Vec<RepoId>,
    pub repo_summaries: BTreeMap<RepoId, RepoSummary>,
    pub selected_repo_id: Option<RepoId>,
    pub sort_mode: WorkspaceSortMode,
    pub filter_mode: WorkspaceFilterMode,
    pub search_query: String,
    pub search_match_index: usize,
    #[serde(skip)]
    pub search_focused: bool,
    pub preview_mode: PreviewMode,
    pub scan_status: ScanStatus,
    pub watcher_health: WatcherHealth,
    pub last_full_refresh_at: Option<Timestamp>,
    #[serde(skip)]
    pub pending_watcher_invalidations: BTreeMap<RepoId, usize>,
    #[serde(skip)]
    pub watcher_debounce_pending: bool,
}

impl WorkspaceState {
    #[must_use]
    pub fn visible_repo_ids(&self) -> Vec<RepoId> {
        let mut candidates = self
            .discovered_repo_ids
            .iter()
            .enumerate()
            .filter_map(|(index, repo_id)| {
                let summary = self.repo_summaries.get(repo_id);
                if !self.filter_mode.matches(summary) {
                    return None;
                }
                if !matches_search(repo_id, summary, &self.search_query) {
                    return None;
                }
                Some(VisibleRepo {
                    repo_id,
                    summary,
                    index,
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| compare_visible_repos(left, right, self.sort_mode));
        candidates
            .into_iter()
            .map(|candidate| candidate.repo_id.clone())
            .collect()
    }

    pub fn ensure_visible_selection(&mut self) -> Option<RepoId> {
        let visible_repo_ids = self.visible_repo_ids();
        if visible_repo_ids.is_empty() {
            self.selected_repo_id = None;
            self.search_match_index = 0;
            return None;
        }

        if let Some(selected) = self.selected_repo_id.as_ref() {
            if let Some(index) = visible_repo_ids
                .iter()
                .position(|repo_id| repo_id == selected)
            {
                self.search_match_index = index;
                return self.selected_repo_id.clone();
            }
        }

        let next_repo = visible_repo_ids[0].clone();
        self.selected_repo_id = Some(next_repo.clone());
        self.search_match_index = 0;
        Some(next_repo)
    }

    #[must_use]
    pub fn search_match_count(&self) -> usize {
        self.visible_repo_ids().len()
    }

    #[must_use]
    pub fn current_search_match_position(&self) -> Option<(usize, usize)> {
        let visible_repo_ids = self.visible_repo_ids();
        if visible_repo_ids.is_empty() {
            return None;
        }

        let selected = self.selected_repo_id.as_ref()?;
        let index = visible_repo_ids
            .iter()
            .position(|repo_id| repo_id == selected)?;
        Some((index, visible_repo_ids.len()))
    }

    pub fn select_next_search_match(&mut self) -> Option<RepoId> {
        self.select_search_match_with_step(1)
    }

    pub fn select_previous_search_match(&mut self) -> Option<RepoId> {
        self.select_search_match_with_step(-1)
    }

    fn select_search_match_with_step(&mut self, step: isize) -> Option<RepoId> {
        let visible_repo_ids = self.visible_repo_ids();
        if visible_repo_ids.is_empty() {
            self.selected_repo_id = None;
            self.search_match_index = 0;
            return None;
        }

        let len = visible_repo_ids.len() as isize;
        let current_index = self
            .selected_repo_id
            .as_ref()
            .and_then(|selected| {
                visible_repo_ids
                    .iter()
                    .position(|candidate| candidate == selected)
            })
            .unwrap_or_else(|| self.search_match_index.min(visible_repo_ids.len() - 1));
        let next_index = (current_index as isize + step).rem_euclid(len) as usize;
        let next_repo = visible_repo_ids[next_index].clone();
        self.selected_repo_id = Some(next_repo.clone());
        self.search_match_index = next_index;
        Some(next_repo)
    }

    #[must_use]
    pub fn prioritized_repo_ids(
        &self,
        repo_ids: &[RepoId],
        active_repo_id: Option<&RepoId>,
    ) -> Vec<RepoId> {
        let mut ordered = Vec::new();
        if let Some(active_repo_id) = active_repo_id
            .filter(|active_repo_id| repo_ids.iter().any(|repo_id| repo_id == *active_repo_id))
        {
            ordered.push(active_repo_id.clone());
        }

        for repo_id in self.visible_repo_ids() {
            if repo_ids.iter().any(|candidate| candidate == &repo_id)
                && !ordered.iter().any(|candidate| candidate == &repo_id)
            {
                ordered.push(repo_id);
            }
        }

        for repo_id in &self.discovered_repo_ids {
            if repo_ids.iter().any(|candidate| candidate == repo_id)
                && !ordered.iter().any(|candidate| candidate == repo_id)
            {
                ordered.push(repo_id.clone());
            }
        }

        for repo_id in repo_ids {
            if !ordered.iter().any(|candidate| candidate == repo_id) {
                ordered.push(repo_id.clone());
            }
        }

        ordered
    }

    pub fn select_next(&mut self) -> Option<RepoId> {
        self.select_with_step(1)
    }

    pub fn select_previous(&mut self) -> Option<RepoId> {
        self.select_with_step(-1)
    }

    fn select_with_step(&mut self, step: isize) -> Option<RepoId> {
        let visible_repo_ids = self.visible_repo_ids();
        if visible_repo_ids.is_empty() {
            self.selected_repo_id = None;
            self.search_match_index = 0;
            return None;
        }

        let current_index = self.selected_repo_id.as_ref().and_then(|selected| {
            visible_repo_ids
                .iter()
                .position(|candidate| candidate == selected)
        });
        let next_index = match current_index {
            Some(index) => {
                let len = visible_repo_ids.len() as isize;
                (index as isize + step).rem_euclid(len) as usize
            }
            None if step < 0 => visible_repo_ids.len() - 1,
            None => 0,
        };
        let next_repo = visible_repo_ids[next_index].clone();
        self.selected_repo_id = Some(next_repo.clone());
        self.search_match_index = next_index;
        Some(next_repo)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceSortMode {
    #[default]
    Attention,
    Name,
    Path,
    LastActivity,
}

impl WorkspaceSortMode {
    #[must_use]
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::Attention => Self::Name,
            Self::Name => Self::Path,
            Self::Path => Self::LastActivity,
            Self::LastActivity => Self::Attention,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Attention => "attention",
            Self::Name => "name",
            Self::Path => "path",
            Self::LastActivity => "activity",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceFilterMode {
    #[default]
    All,
    DirtyOnly,
    AheadOnly,
    BehindOnly,
    ConflictedOnly,
}

impl WorkspaceFilterMode {
    #[must_use]
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::All => Self::DirtyOnly,
            Self::DirtyOnly => Self::AheadOnly,
            Self::AheadOnly => Self::BehindOnly,
            Self::BehindOnly => Self::ConflictedOnly,
            Self::ConflictedOnly => Self::All,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::DirtyOnly => "dirty",
            Self::AheadOnly => "ahead",
            Self::BehindOnly => "behind",
            Self::ConflictedOnly => "conflicts",
        }
    }

    #[must_use]
    pub fn matches(self, summary: Option<&RepoSummary>) -> bool {
        match self {
            Self::All => true,
            Self::DirtyOnly => summary.is_some_and(repo_is_dirty),
            Self::AheadOnly => summary.is_some_and(|summary| summary.ahead_count > 0),
            Self::BehindOnly => summary.is_some_and(|summary| summary.behind_count > 0),
            Self::ConflictedOnly => summary.is_some_and(|summary| summary.conflicted),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewMode {
    #[default]
    Summary,
    DiffPreview,
    Hidden,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchSortMode {
    #[default]
    Natural,
    Name,
}

impl BranchSortMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Name => "name",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteBranchSortMode {
    #[default]
    Natural,
    Name,
}

impl RemoteBranchSortMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Name => "name",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanStatus {
    #[default]
    Idle,
    Scanning,
    Complete {
        scanned_repos: usize,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatcherHealth {
    #[default]
    Unknown,
    Healthy,
    Degraded {
        message: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoModeState {
    pub current_repo_id: RepoId,
    pub parent_repo_ids: Vec<RepoId>,
    pub active_subview: RepoSubview,
    pub commit_subview_mode: CommitSubviewMode,
    pub commit_files_mode: CommitFilesMode,
    pub stash_subview_mode: StashSubviewMode,
    pub commit_history_mode: CommitHistoryMode,
    pub main_focus: PaneId,
    pub diff_scroll: usize,
    pub diff_line_cursor: Option<usize>,
    pub diff_line_anchor: Option<usize>,
    pub status_view: ListViewState,
    pub staged_view: ListViewState,
    pub status_filter: RepoSubviewFilterState,
    pub status_filter_mode: StatusFilterMode,
    pub status_tree_enabled: bool,
    pub show_root_item_in_file_tree: bool,
    pub collapsed_status_dirs: BTreeSet<PathBuf>,
    pub commit_box: CommitBoxState,
    pub branches_view: ListViewState,
    pub branch_sort_mode: BranchSortMode,
    pub remotes_view: ListViewState,
    pub remote_branches_view: ListViewState,
    pub remote_branch_sort_mode: RemoteBranchSortMode,
    pub tags_view: ListViewState,
    pub commits_view: ListViewState,
    pub commit_files_view: ListViewState,
    pub stash_view: ListViewState,
    pub stash_files_view: ListViewState,
    pub reflog_view: ListViewState,
    pub worktree_view: ListViewState,
    pub submodules_view: ListViewState,
    pub branches_filter: RepoSubviewFilterState,
    pub remotes_filter: RepoSubviewFilterState,
    pub remote_branches_filter: RepoSubviewFilterState,
    pub tags_filter: RepoSubviewFilterState,
    pub commits_filter: RepoSubviewFilterState,
    pub commit_files_filter: RepoSubviewFilterState,
    pub commit_history_ref: Option<String>,
    pub sub_commit_parent_ref: Option<String>,
    pub sub_commit_divergence_ref: Option<String>,
    pub sub_commit_show_branch_heads: bool,
    pub sub_commit_limit: bool,
    pub pending_commit_selection_oid: Option<String>,
    pub pending_remote_flow: Option<PendingRemoteFlow>,
    pub stash_filter: RepoSubviewFilterState,
    pub reflog_filter: RepoSubviewFilterState,
    pub worktree_filter: RepoSubviewFilterState,
    pub submodules_filter: RepoSubviewFilterState,
    pub operation_progress: OperationProgress,
    pub ignore_whitespace_in_diff: bool,
    pub diff_context_lines: u16,
    pub rename_similarity_threshold: u8,
    pub comparison_base: Option<ComparisonTarget>,
    pub comparison_target: Option<ComparisonTarget>,
    pub comparison_source: Option<RepoSubview>,
    pub copied_commit: Option<CopiedCommit>,
    pub detail: Option<RepoDetail>,
}

pub const DEFAULT_DIFF_CONTEXT_LINES: u16 = 3;
pub const MIN_DIFF_CONTEXT_LINES: u16 = 0;
pub const DEFAULT_RENAME_SIMILARITY_THRESHOLD: u8 = 50;
pub const MIN_RENAME_SIMILARITY_THRESHOLD: u8 = 5;
pub const MAX_RENAME_SIMILARITY_THRESHOLD: u8 = 100;
pub const RENAME_SIMILARITY_THRESHOLD_STEP: u8 = 5;

impl RepoModeState {
    #[must_use]
    pub fn new(current_repo_id: RepoId) -> Self {
        Self::new_with_parent(current_repo_id, Vec::new())
    }

    #[must_use]
    pub fn new_with_parent(current_repo_id: RepoId, parent_repo_ids: Vec<RepoId>) -> Self {
        Self {
            current_repo_id,
            parent_repo_ids,
            active_subview: RepoSubview::default(),
            commit_subview_mode: CommitSubviewMode::default(),
            commit_files_mode: CommitFilesMode::default(),
            stash_subview_mode: StashSubviewMode::default(),
            commit_history_mode: CommitHistoryMode::default(),
            main_focus: PaneId::RepoUnstaged,
            diff_scroll: 0,
            diff_line_cursor: None,
            diff_line_anchor: None,
            status_view: ListViewState::default(),
            staged_view: ListViewState::default(),
            status_filter: RepoSubviewFilterState::default(),
            status_filter_mode: StatusFilterMode::default(),
            status_tree_enabled: true,
            show_root_item_in_file_tree: false,
            collapsed_status_dirs: BTreeSet::default(),
            commit_box: CommitBoxState::default(),
            branches_view: ListViewState::default(),
            branch_sort_mode: BranchSortMode::default(),
            remotes_view: ListViewState::default(),
            remote_branches_view: ListViewState::default(),
            remote_branch_sort_mode: RemoteBranchSortMode::default(),
            tags_view: ListViewState::default(),
            commits_view: ListViewState::default(),
            commit_files_view: ListViewState::default(),
            stash_view: ListViewState::default(),
            stash_files_view: ListViewState::default(),
            reflog_view: ListViewState::default(),
            worktree_view: ListViewState::default(),
            submodules_view: ListViewState::default(),
            branches_filter: RepoSubviewFilterState::default(),
            remotes_filter: RepoSubviewFilterState::default(),
            remote_branches_filter: RepoSubviewFilterState::default(),
            tags_filter: RepoSubviewFilterState::default(),
            commits_filter: RepoSubviewFilterState::default(),
            commit_files_filter: RepoSubviewFilterState::default(),
            commit_history_ref: None,
            sub_commit_parent_ref: None,
            sub_commit_divergence_ref: None,
            sub_commit_show_branch_heads: false,
            sub_commit_limit: true,
            pending_commit_selection_oid: None,
            pending_remote_flow: None,
            stash_filter: RepoSubviewFilterState::default(),
            reflog_filter: RepoSubviewFilterState::default(),
            worktree_filter: RepoSubviewFilterState::default(),
            submodules_filter: RepoSubviewFilterState::default(),
            operation_progress: OperationProgress::Idle,
            ignore_whitespace_in_diff: false,
            diff_context_lines: DEFAULT_DIFF_CONTEXT_LINES,
            rename_similarity_threshold: DEFAULT_RENAME_SIMILARITY_THRESHOLD,
            comparison_base: None,
            comparison_target: None,
            comparison_source: None,
            copied_commit: None,
            detail: None,
        }
    }

    #[must_use]
    pub fn subview_filter(&self, subview: RepoSubview) -> Option<&RepoSubviewFilterState> {
        match subview {
            RepoSubview::Status => Some(&self.status_filter),
            RepoSubview::Branches => Some(&self.branches_filter),
            RepoSubview::Remotes => Some(&self.remotes_filter),
            RepoSubview::RemoteBranches => Some(&self.remote_branches_filter),
            RepoSubview::Tags => Some(&self.tags_filter),
            RepoSubview::Commits => Some(match self.commit_subview_mode {
                CommitSubviewMode::History => &self.commits_filter,
                CommitSubviewMode::SubHistory => &self.commits_filter,
                CommitSubviewMode::Files => &self.commit_files_filter,
            }),
            RepoSubview::Stash => match self.stash_subview_mode {
                StashSubviewMode::List => Some(&self.stash_filter),
                StashSubviewMode::Files => None,
            },
            RepoSubview::Reflog => Some(&self.reflog_filter),
            RepoSubview::Worktrees => Some(&self.worktree_filter),
            RepoSubview::Submodules => Some(&self.submodules_filter),
            RepoSubview::Compare | RepoSubview::Rebase => None,
        }
    }

    pub fn subview_filter_mut(
        &mut self,
        subview: RepoSubview,
    ) -> Option<&mut RepoSubviewFilterState> {
        match subview {
            RepoSubview::Status => Some(&mut self.status_filter),
            RepoSubview::Branches => Some(&mut self.branches_filter),
            RepoSubview::Remotes => Some(&mut self.remotes_filter),
            RepoSubview::RemoteBranches => Some(&mut self.remote_branches_filter),
            RepoSubview::Tags => Some(&mut self.tags_filter),
            RepoSubview::Commits => Some(match self.commit_subview_mode {
                CommitSubviewMode::History => &mut self.commits_filter,
                CommitSubviewMode::SubHistory => &mut self.commits_filter,
                CommitSubviewMode::Files => &mut self.commit_files_filter,
            }),
            RepoSubview::Stash => match self.stash_subview_mode {
                StashSubviewMode::List => Some(&mut self.stash_filter),
                StashSubviewMode::Files => None,
            },
            RepoSubview::Reflog => Some(&mut self.reflog_filter),
            RepoSubview::Worktrees => Some(&mut self.worktree_filter),
            RepoSubview::Submodules => Some(&mut self.submodules_filter),
            RepoSubview::Compare | RepoSubview::Rebase => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitSubviewMode {
    #[default]
    History,
    SubHistory,
    Files,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitFilesMode {
    #[default]
    List,
    Diff,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StashSubviewMode {
    #[default]
    List,
    Files,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitHistoryMode {
    #[default]
    Linear,
    Graph {
        reverse: bool,
    },
    Reflog,
    SubHistory,
}

impl CommitHistoryMode {
    #[must_use]
    pub const fn is_graph(self) -> bool {
        matches!(self, Self::Graph { .. })
    }

    #[must_use]
    pub const fn reverse(self) -> bool {
        matches!(self, Self::Graph { reverse: true })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoSubview {
    #[default]
    Status,
    Branches,
    Remotes,
    RemoteBranches,
    Tags,
    Commits,
    Compare,
    Rebase,
    Stash,
    Reflog,
    Worktrees,
    Submodules,
}

impl RepoSubview {
    #[must_use]
    pub const fn supports_filter(self) -> bool {
        matches!(
            self,
            Self::Status
                | Self::Branches
                | Self::Remotes
                | Self::RemoteBranches
                | Self::Tags
                | Self::Commits
                | Self::Stash
                | Self::Reflog
                | Self::Worktrees
                | Self::Submodules
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusFilterMode {
    #[default]
    All,
    TrackedOnly,
    UntrackedOnly,
    ConflictedOnly,
}

impl StatusFilterMode {
    #[must_use]
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::All => Self::TrackedOnly,
            Self::TrackedOnly => Self::UntrackedOnly,
            Self::UntrackedOnly => Self::ConflictedOnly,
            Self::ConflictedOnly => Self::All,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::TrackedOnly => "tracked",
            Self::UntrackedOnly => "untracked",
            Self::ConflictedOnly => "conflicts",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSubviewFilterState {
    pub query: String,
    pub history: Vec<String>,
    #[serde(skip)]
    pub focused: bool,
    #[serde(skip)]
    pub history_index: isize,
}

impl RepoSubviewFilterState {
    #[must_use]
    pub fn active_query(&self) -> Option<String> {
        let normalized = normalize_search_text(&self.query);
        (!normalized.is_empty()).then_some(normalized)
    }

    pub fn push_history_entry(&mut self) {
        let Some(query) = self.active_query() else {
            return;
        };
        self.history.insert(0, query);
        self.history_index = -1;
    }

    pub fn recall_previous_history(&mut self) -> bool {
        let next_index = self.history_index + 1;
        let Some(next_query) = self.history.get(next_index as usize).cloned() else {
            return false;
        };
        self.history_index = next_index;
        self.query = next_query;
        true
    }

    pub fn recall_next_history(&mut self) -> bool {
        if self.history_index < 0 {
            return false;
        }

        let next_index = self.history_index - 1;
        self.history_index = next_index;
        if next_index < 0 {
            self.query.clear();
        } else if let Some(next_query) = self.history.get(next_index as usize).cloned() {
            self.query = next_query;
        }
        true
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitBoxState {
    pub focused: bool,
    pub mode: CommitBoxMode,
    pub preserved_on_close: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitBoxMode {
    #[default]
    Commit,
    CommitNoVerify,
    Amend,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListViewState {
    pub selected_index: Option<usize>,
    pub selection_anchor: Option<usize>,
}

impl ListViewState {
    pub fn ensure_selection(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            self.selection_anchor = None;
            return None;
        }

        let selected = self
            .selected_index
            .filter(|index| *index < len)
            .unwrap_or(0);
        self.selection_anchor = self.selection_anchor.filter(|index| *index < len);
        self.selected_index = Some(selected);
        Some(selected)
    }

    pub fn select_with_step(&mut self, len: usize, step: isize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            self.selection_anchor = None;
            return None;
        }

        let current = self.ensure_selection(len).unwrap_or(0);
        let next = (current as isize + step).rem_euclid(len as isize) as usize;
        self.selected_index = Some(next);
        Some(next)
    }

    pub fn select_first(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            self.selection_anchor = None;
            return None;
        }

        self.selected_index = Some(0);
        Some(0)
    }

    pub fn select_last(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            self.selection_anchor = None;
            return None;
        }

        let last = len.saturating_sub(1);
        self.selected_index = Some(last);
        Some(last)
    }

    pub fn set_selected(&mut self, len: usize, index: usize) -> Option<usize> {
        if len == 0 || index >= len {
            self.selected_index = None;
            self.selection_anchor = None;
            return None;
        }

        self.selected_index = Some(index);
        Some(index)
    }

    pub fn toggle_selection_anchor(&mut self, len: usize) -> Option<usize> {
        let selected = self.ensure_selection(len)?;
        if self.selection_anchor == Some(selected) {
            self.selection_anchor = None;
        } else {
            self.selection_anchor = Some(selected);
        }
        Some(selected)
    }

    pub fn clear_selection_anchor(&mut self) {
        self.selection_anchor = None;
    }

    pub fn selection_range(&mut self, len: usize) -> Option<(usize, usize)> {
        let selected = self.ensure_selection(len)?;
        let anchor = self.selection_anchor.unwrap_or(selected);
        Some((anchor.min(selected), anchor.max(selected)))
    }

    pub fn selected_item<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let index = self.ensure_selection(items.len())?;
        items.get(index)
    }

    pub fn selected_items<'a, T>(&mut self, items: &'a [T]) -> Option<(&'a [T], usize, usize)> {
        let (start, end) = self.selection_range(items.len())?;
        Some((&items[start..=end], start, end))
    }

    pub fn selected_item_id<T, F>(&mut self, items: &[T], mut get_id: F) -> Option<String>
    where
        F: FnMut(&T) -> &str,
    {
        self.selected_item(items)
            .map(|item| get_id(item).to_string())
    }

    pub fn selected_item_ids<T, F>(
        &mut self,
        items: &[T],
        mut get_id: F,
    ) -> Option<(Vec<String>, usize, usize)>
    where
        F: FnMut(&T) -> &str,
    {
        let (items, start, end) = self.selected_items(items)?;
        Some((
            items.iter().map(|item| get_id(item).to_string()).collect(),
            start,
            end,
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationProgress {
    #[default]
    Idle,
    Running {
        job_id: JobId,
        summary: String,
    },
    Failed {
        summary: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSummary {
    pub repo_id: RepoId,
    pub display_name: String,
    pub real_path: PathBuf,
    pub display_path: String,
    pub branch: Option<String>,
    pub head_kind: HeadKind,
    pub dirty: bool,
    pub staged_count: u32,
    pub unstaged_count: u32,
    pub untracked_count: u32,
    pub ahead_count: u32,
    pub behind_count: u32,
    pub conflicted: bool,
    pub last_fetch_at: Option<Timestamp>,
    pub last_local_activity_at: Option<Timestamp>,
    pub last_refresh_at: Option<Timestamp>,
    pub watcher_freshness: WatcherFreshness,
    pub remote_summary: RemoteSummary,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadKind {
    #[default]
    Branch,
    Detached,
    Unborn,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatcherFreshness {
    #[default]
    Unknown,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSummary {
    pub tracking_branch: Option<String>,
    pub remote_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoDetail {
    pub file_tree: Vec<FileStatus>,
    pub diff: DiffModel,
    pub branches: Vec<BranchItem>,
    pub remotes: Vec<RemoteItem>,
    pub remote_branches: Vec<RemoteBranchItem>,
    pub tags: Vec<TagItem>,
    pub commits: Vec<CommitItem>,
    pub commit_graph_lines: Vec<String>,
    pub bisect_state: Option<BisectState>,
    pub rebase_state: Option<RebaseState>,
    pub stashes: Vec<StashItem>,
    pub reflog_items: Vec<ReflogItem>,
    pub worktrees: Vec<WorktreeItem>,
    pub submodules: Vec<SubmoduleItem>,
    pub working_tree_state: WorkingTreeState,
    pub commit_input: String,
    pub merge_state: MergeState,
    pub merge_fast_forward_preference: MergeFastForwardPreference,
    pub fast_forward_merge_targets: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebaseKind {
    #[default]
    Interactive,
    Apply,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BisectState {
    pub bad_term: String,
    pub good_term: String,
    pub current_commit: Option<String>,
    pub current_summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebaseState {
    pub kind: RebaseKind,
    pub step: usize,
    pub total: usize,
    pub head_name: Option<String>,
    pub onto: Option<String>,
    pub current_commit: Option<String>,
    pub current_summary: Option<String>,
    pub todo_preview: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingTreeState {
    pub rebasing: bool,
    pub merging: bool,
    pub cherry_picking: bool,
    pub reverting: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectiveWorkingTreeState {
    #[default]
    None,
    Rebasing,
    Merging,
    CherryPicking,
    Reverting,
}

impl WorkingTreeState {
    #[must_use]
    pub const fn any(self) -> bool {
        self.rebasing || self.merging || self.cherry_picking || self.reverting
    }

    #[must_use]
    pub const fn none(self) -> bool {
        !self.any()
    }

    #[must_use]
    pub const fn effective(self) -> EffectiveWorkingTreeState {
        if self.reverting {
            EffectiveWorkingTreeState::Reverting
        } else if self.cherry_picking {
            EffectiveWorkingTreeState::CherryPicking
        } else if self.merging {
            EffectiveWorkingTreeState::Merging
        } else if self.rebasing {
            EffectiveWorkingTreeState::Rebasing
        } else {
            EffectiveWorkingTreeState::None
        }
    }

    #[must_use]
    pub const fn command_name(self) -> &'static str {
        match self.effective() {
            EffectiveWorkingTreeState::None => "",
            EffectiveWorkingTreeState::Rebasing => "rebase",
            EffectiveWorkingTreeState::Merging => "merge",
            EffectiveWorkingTreeState::CherryPicking => "cherry-pick",
            EffectiveWorkingTreeState::Reverting => "revert",
        }
    }

    #[must_use]
    pub const fn can_show_todos(self) -> bool {
        self.rebasing || self.cherry_picking || self.reverting
    }

    #[must_use]
    pub const fn can_skip(self) -> bool {
        self.rebasing || self.cherry_picking || self.reverting
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileStatus {
    pub path: PathBuf,
    pub previous_path: Option<PathBuf>,
    pub kind: FileStatusKind,
    pub staged_kind: Option<FileStatusKind>,
    pub unstaged_kind: Option<FileStatusKind>,
    pub short_status: String,
    pub inline_merge_conflicts: Option<bool>,
    pub display_string: String,
    pub lines_added: u32,
    pub lines_deleted: u32,
    pub is_worktree: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatusFields {
    pub has_staged_changes: bool,
    pub has_unstaged_changes: bool,
    pub tracked: bool,
    pub deleted: bool,
    pub added: bool,
    pub has_merge_conflicts: bool,
    pub has_inline_merge_conflicts: bool,
    pub short_status: String,
}

impl FileStatus {
    #[must_use]
    pub fn is_rename(&self) -> bool {
        self.previous_path.is_some()
    }

    #[must_use]
    pub fn names(&self) -> Vec<&Path> {
        let mut names = vec![self.path.as_path()];
        if let Some(previous_path) = self.previous_path.as_deref() {
            names.push(previous_path);
        }
        names
    }

    #[must_use]
    pub fn matches_file(&self, other: &Self) -> bool {
        self.names()
            .iter()
            .any(|path| other.names().iter().any(|other_path| path == other_path))
    }

    #[must_use]
    pub fn has_staged_changes(&self) -> bool {
        self.staged_kind.is_some()
    }

    #[must_use]
    pub fn has_staged_or_tracked_changes(&self) -> bool {
        self.has_staged_changes() || self.tracked()
    }

    #[must_use]
    pub fn has_unstaged_changes(&self) -> bool {
        self.unstaged_kind.is_some()
    }

    #[must_use]
    pub fn tracked(&self) -> bool {
        !matches!(self.short_status.as_str(), "??" | "A " | "AM")
    }

    #[must_use]
    pub fn added(&self) -> bool {
        self.short_status.chars().any(|ch| ch == 'A') || !self.tracked()
    }

    #[must_use]
    pub fn deleted(&self) -> bool {
        self.short_status.chars().any(|ch| ch == 'D')
    }

    #[must_use]
    pub fn has_inline_merge_conflicts(&self) -> bool {
        self.inline_merge_conflicts
            .unwrap_or(matches!(self.short_status.as_str(), "UU" | "AA"))
    }

    #[must_use]
    pub fn has_merge_conflicts(&self) -> bool {
        self.has_inline_merge_conflicts()
            || matches!(self.short_status.as_str(), "DD" | "AU" | "UA" | "UD" | "DU")
    }

    #[must_use]
    pub fn merge_state_description(&self) -> Option<&'static str> {
        match self.short_status.as_str() {
            "DD" => Some("Conflict: this file was moved or renamed both in the current and the incoming changes, but to different destinations. I don't know which ones, but they should both show up as conflicts too (marked 'AU' and 'UA', respectively). The most likely resolution is to delete this file, and pick one of the destinations and delete the other."),
            "AU" => Some("Conflict: this file is the destination of a move or rename in the current changes, but was moved or renamed to a different destination in the incoming changes. That other destination should also show up as a conflict (marked 'UA'), as well as the file that both were renamed from (marked 'DD')."),
            "UA" => Some("Conflict: this file is the destination of a move or rename in the incoming changes, but was moved or renamed to a different destination in the current changes. That other destination should also show up as a conflict (marked 'AU'), as well as the file that both were renamed from (marked 'DD')."),
            "DU" => Some("Conflict: this file was deleted in the current changes and modified in the incoming changes.\n\nThe most likely resolution is to delete the file after applying the incoming modifications manually to some other place in the code."),
            "UD" => Some("Conflict: this file was modified in the current changes and deleted in incoming changes.\n\nThe most likely resolution is to delete the file after applying the current modifications manually to some other place in the code."),
            _ => None,
        }
    }

    #[must_use]
    pub fn derived_status_fields(short_status: &str) -> FileStatusFields {
        let staged_change = short_status.chars().next().unwrap_or(' ');
        let unstaged_change = short_status.chars().nth(1).unwrap_or(' ');
        let tracked = !matches!(short_status, "??" | "A " | "AM");
        let has_staged_changes = !matches!(staged_change, ' ' | 'U' | '?');
        let has_inline_merge_conflicts = matches!(short_status, "UU" | "AA");
        let has_merge_conflicts =
            has_inline_merge_conflicts || matches!(short_status, "DD" | "AU" | "UA" | "UD" | "DU");

        FileStatusFields {
            has_staged_changes,
            has_unstaged_changes: unstaged_change != ' ',
            tracked,
            deleted: unstaged_change == 'D' || staged_change == 'D',
            added: unstaged_change == 'A' || !tracked,
            has_merge_conflicts,
            has_inline_merge_conflicts,
            short_status: short_status.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleStatusEntry {
    pub path: PathBuf,
    pub kind: Option<FileStatusKind>,
    pub depth: usize,
    pub label: String,
    pub entry_kind: VisibleStatusEntryKind,
}

impl VisibleStatusEntry {
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        matches!(self.entry_kind, VisibleStatusEntryKind::Directory { .. })
    }

    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.entry_kind, VisibleStatusEntryKind::File)
    }

    #[must_use]
    pub const fn collapsed(&self) -> bool {
        matches!(
            self.entry_kind,
            VisibleStatusEntryKind::Directory { collapsed: true }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleStatusEntryKind {
    Directory { collapsed: bool },
    File,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatusKind {
    #[default]
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[must_use]
pub fn visible_status_entries(repo_mode: &RepoModeState, pane: PaneId) -> Vec<VisibleStatusEntry> {
    let Some(detail) = repo_mode.detail.as_ref() else {
        return Vec::new();
    };

    let normalized_query = repo_mode.status_filter.active_query();
    let filtered_files = detail
        .file_tree
        .iter()
        .filter(|item| {
            status_kind_for_pane(item, pane).is_some_and(|kind| {
                status_filter_mode_matches(repo_mode.status_filter_mode, kind)
                    && normalized_query
                        .as_deref()
                        .is_none_or(|query| status_entry_matches_query(item, kind, query))
            })
        })
        .collect::<Vec<_>>();

    if !repo_mode.status_tree_enabled {
        let mut flat_files = filtered_files;
        flat_files.sort_by(|left, right| left.path.cmp(&right.path));
        flat_files.sort_by_key(|item| flat_status_sort_key(item));

        return flat_files
            .into_iter()
            .map(|item| VisibleStatusEntry {
                path: item.path.clone(),
                kind: status_kind_for_pane(item, pane),
                depth: 0,
                label: status_entry_label(item, false),
                entry_kind: VisibleStatusEntryKind::File,
            })
            .collect();
    }

    let tree = build_status_tree(filtered_files);
    let mut entries = Vec::new();

    if repo_mode.show_root_item_in_file_tree && should_show_status_root_item(&tree) {
        let collapsed = repo_mode.collapsed_status_dirs.contains(Path::new("."));
        entries.push(VisibleStatusEntry {
            path: PathBuf::from("."),
            kind: tree.aggregate_kind(pane),
            depth: 0,
            label: ".".to_string(),
            entry_kind: VisibleStatusEntryKind::Directory { collapsed },
        });

        if !collapsed {
            tree.append_entries(1, pane, repo_mode, &mut entries);
        }

        return entries;
    }

    tree.append_entries(0, pane, repo_mode, &mut entries);
    entries
}

fn should_show_status_root_item(tree: &StatusTreeNode<'_>) -> bool {
    if tree.directories.is_empty() && tree.files.is_empty() {
        return false;
    }

    !(tree.directories.is_empty() && tree.files.len() == 1)
}

fn flat_status_sort_key(item: &FileStatus) -> u8 {
    if item.has_merge_conflicts() {
        0
    } else if item.tracked() {
        1
    } else {
        2
    }
}

fn build_status_tree<'a>(files: Vec<&'a FileStatus>) -> StatusTreeNode<'a> {
    let mut root = StatusTreeNode::root();
    for item in files {
        root.insert_file(item);
    }
    root.compress_children();
    root
}

#[derive(Debug, Clone)]
struct StatusTreeNode<'a> {
    path: PathBuf,
    directories: Vec<StatusTreeNode<'a>>,
    files: Vec<&'a FileStatus>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusTreeFlatKind {
    Directory,
    File,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusTreeFlatEntry {
    path: PathBuf,
    visual_depth: usize,
    kind: StatusTreeFlatKind,
}

impl<'a> StatusTreeNode<'a> {
    fn root() -> Self {
        Self {
            path: PathBuf::new(),
            directories: Vec::new(),
            files: Vec::new(),
        }
    }

    fn insert_file(&mut self, item: &'a FileStatus) {
        self.insert_file_components(item, item.path.components().collect::<Vec<_>>().as_slice());
    }

    fn insert_file_components(
        &mut self,
        item: &'a FileStatus,
        components: &[std::path::Component<'_>],
    ) {
        if components.len() <= 1 {
            self.files.push(item);
            self.files.sort_by(|left, right| left.path.cmp(&right.path));
            return;
        }

        let mut directory_path = self.path.clone();
        directory_path.push(components[0].as_os_str());
        let existing_index = self
            .directories
            .iter()
            .position(|node| node.path == directory_path);
        let index = if let Some(index) = existing_index {
            index
        } else {
            self.directories.push(StatusTreeNode {
                path: directory_path.clone(),
                directories: Vec::new(),
                files: Vec::new(),
            });
            self.directories
                .sort_by(|left, right| left.path.cmp(&right.path));
            self.directories
                .iter()
                .position(|node| node.path == directory_path)
                .unwrap_or(0)
        };
        self.directories[index].insert_file_components(item, &components[1..]);
    }

    fn compress_children(&mut self) {
        for directory in &mut self.directories {
            directory.compress();
        }
    }

    fn compress(&mut self) {
        self.compress_children();

        while self.files.is_empty() && self.directories.len() == 1 {
            let child = self.directories.remove(0);
            self.path = child.path;
            self.directories = child.directories;
            self.files = child.files;
        }
    }

    fn aggregate_kind(&self, pane: PaneId) -> Option<FileStatusKind> {
        aggregate_status_kind(self.all_files().into_iter(), pane)
    }

    fn all_files(&self) -> Vec<&'a FileStatus> {
        let mut files = self.files.clone();
        for directory in &self.directories {
            files.extend(directory.all_files());
        }
        files
    }

    fn append_entries(
        &self,
        depth: usize,
        pane: PaneId,
        repo_mode: &RepoModeState,
        entries: &mut Vec<VisibleStatusEntry>,
    ) {
        for directory in &self.directories {
            let collapsed = repo_mode
                .collapsed_status_dirs
                .contains(directory.path.as_path());
            let kind = directory.aggregate_kind(pane);
            let label = if self.path.as_os_str().is_empty() {
                directory.path.display().to_string()
            } else {
                directory
                    .path
                    .strip_prefix(&self.path)
                    .ok()
                    .and_then(Path::to_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| directory.path.display().to_string())
            };
            entries.push(VisibleStatusEntry {
                path: directory.path.clone(),
                kind,
                depth,
                label,
                entry_kind: VisibleStatusEntryKind::Directory { collapsed },
            });
            if !collapsed {
                directory.append_entries(depth + 1, pane, repo_mode, entries);
            }
        }

        for item in &self.files {
            if let Some(kind) = status_kind_for_pane(item, pane) {
                entries.push(VisibleStatusEntry {
                    path: item.path.clone(),
                    kind: Some(kind),
                    depth,
                    label: status_entry_label(item, true),
                    entry_kind: VisibleStatusEntryKind::File,
                });
            }
        }
    }

    #[cfg(test)]
    fn flatten_visible(
        &self,
        collapsed_dirs: &BTreeSet<PathBuf>,
        depth: usize,
        entries: &mut Vec<StatusTreeFlatEntry>,
    ) {
        for directory in &self.directories {
            entries.push(StatusTreeFlatEntry {
                path: directory.path.clone(),
                visual_depth: depth,
                kind: StatusTreeFlatKind::Directory,
            });
            if !collapsed_dirs.contains(directory.path.as_path()) {
                directory.flatten_visible(collapsed_dirs, depth + 1, entries);
            }
        }

        for item in &self.files {
            entries.push(StatusTreeFlatEntry {
                path: item.path.clone(),
                visual_depth: depth,
                kind: StatusTreeFlatKind::File,
            });
        }
    }

    #[cfg(test)]
    fn visible_entries(
        &self,
        collapsed_dirs: &BTreeSet<PathBuf>,
        show_root_item: bool,
    ) -> Vec<StatusTreeFlatEntry> {
        let mut entries = Vec::new();
        if show_root_item && should_show_status_root_item(self) {
            entries.push(StatusTreeFlatEntry {
                path: PathBuf::from("."),
                visual_depth: 0,
                kind: StatusTreeFlatKind::Directory,
            });

            if !collapsed_dirs.contains(Path::new(".")) {
                self.flatten_visible(collapsed_dirs, 1, &mut entries);
            }
            return entries;
        }

        self.flatten_visible(collapsed_dirs, 0, &mut entries);
        entries
    }

    #[cfg(test)]
    fn get_visual_depth_at_index(
        &self,
        index: usize,
        collapsed_dirs: &BTreeSet<PathBuf>,
        show_root_item: bool,
    ) -> Option<usize> {
        self.visible_entries(collapsed_dirs, show_root_item)
            .get(index)
            .map(|entry| entry.visual_depth)
    }
}

fn aggregate_status_kind<'a>(
    items: impl Iterator<Item = &'a FileStatus>,
    pane: PaneId,
) -> Option<FileStatusKind> {
    let mut saw_any = false;
    let mut saw_conflict = false;
    let mut saw_modified = false;
    let mut saw_added = false;
    let mut saw_deleted = false;
    let mut saw_renamed = false;
    let mut saw_untracked = false;

    for item in items {
        let Some(kind) = status_kind_for_pane(item, pane) else {
            continue;
        };
        saw_any = true;
        match kind {
            FileStatusKind::Conflicted => saw_conflict = true,
            FileStatusKind::Modified => saw_modified = true,
            FileStatusKind::Added => saw_added = true,
            FileStatusKind::Deleted => saw_deleted = true,
            FileStatusKind::Renamed => saw_renamed = true,
            FileStatusKind::Untracked => saw_untracked = true,
        }
    }

    if !saw_any {
        return None;
    }
    if saw_conflict {
        return Some(FileStatusKind::Conflicted);
    }
    if saw_modified {
        return Some(FileStatusKind::Modified);
    }
    if saw_added {
        return Some(FileStatusKind::Added);
    }
    if saw_deleted {
        return Some(FileStatusKind::Deleted);
    }
    if saw_renamed {
        return Some(FileStatusKind::Renamed);
    }
    if saw_untracked {
        return Some(FileStatusKind::Untracked);
    }
    None
}

fn status_kind_for_pane(item: &FileStatus, pane: PaneId) -> Option<FileStatusKind> {
    match pane {
        PaneId::RepoUnstaged => item.unstaged_kind,
        PaneId::RepoStaged => item.staged_kind,
        _ => None,
    }
}

fn status_filter_mode_matches(mode: StatusFilterMode, kind: FileStatusKind) -> bool {
    match mode {
        StatusFilterMode::All => true,
        StatusFilterMode::TrackedOnly => kind != FileStatusKind::Untracked,
        StatusFilterMode::UntrackedOnly => kind == FileStatusKind::Untracked,
        StatusFilterMode::ConflictedOnly => kind == FileStatusKind::Conflicted,
    }
}

fn status_entry_label(item: &FileStatus, tree_mode: bool) -> String {
    if let Some(previous_path) = item.previous_path.as_ref() {
        if tree_mode {
            let current = item
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(|| item.path.display().to_string(), ToString::to_string);
            let previous = previous_path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(|| previous_path.display().to_string(), ToString::to_string);
            return format!("{previous} -> {current}");
        }
        return format!("{} -> {}", previous_path.display(), item.path.display());
    }

    if tree_mode {
        return item
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| item.path.display().to_string(), ToString::to_string);
    }

    item.path.display().to_string()
}

fn status_entry_matches_query(item: &FileStatus, kind: FileStatusKind, query: &str) -> bool {
    let path_text = normalize_search_text(&item.path.display().to_string());
    if fuzzy_matches(&path_text, query) || fuzzy_matches(status_kind_search_term(kind), query) {
        return true;
    }

    if let Some(previous_path) = item.previous_path.as_ref() {
        let previous_path_text = normalize_search_text(&previous_path.display().to_string());
        if fuzzy_matches(&previous_path_text, query) {
            return true;
        }
    }

    if !item.display_string.is_empty() {
        let display_text = normalize_search_text(&item.display_string);
        if fuzzy_matches(&display_text, query) {
            return true;
        }
    }

    false
}

fn status_kind_search_term(kind: FileStatusKind) -> &'static str {
    match kind {
        FileStatusKind::Modified => "modified",
        FileStatusKind::Added => "added",
        FileStatusKind::Deleted => "deleted",
        FileStatusKind::Renamed => "renamed",
        FileStatusKind::Untracked => "untracked",
        FileStatusKind::Conflicted => "conflicted",
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffModel {
    pub selected_path: Option<PathBuf>,
    pub presentation: DiffPresentation,
    pub lines: Vec<DiffLine>,
    pub hunks: Vec<DiffHunk>,
    pub selected_hunk: Option<usize>,
    pub hunk_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffPresentation {
    #[default]
    Unstaged,
    Staged,
    Comparison,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub selection: SelectedHunk,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    #[default]
    Context,
    Meta,
    HunkHeader,
    Addition,
    Removal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchItem {
    pub name: String,
    pub display_name: Option<String>,
    pub is_head: bool,
    pub detached_head: bool,
    pub upstream: Option<String>,
    pub recency: String,
    pub ahead_for_pull: String,
    pub behind_for_pull: String,
    pub ahead_for_push: String,
    pub behind_for_push: String,
    pub upstream_gone: bool,
    pub upstream_remote: Option<String>,
    pub upstream_branch: Option<String>,
    pub subject: String,
    pub commit_hash: String,
    pub commit_timestamp: Option<Timestamp>,
    pub behind_base_branch: i32,
}

impl BranchItem {
    #[must_use]
    pub fn urn(&self) -> String {
        format!("branch-{}", self.ref_name())
    }

    #[must_use]
    pub fn full_upstream_ref_name(&self) -> String {
        self.upstream_remote
            .as_deref()
            .zip(self.upstream_branch.as_deref())
            .map(|(remote, branch)| format!("refs/remotes/{remote}/{branch}"))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn short_upstream_ref_name(&self) -> String {
        self.upstream_remote
            .as_deref()
            .zip(self.upstream_branch.as_deref())
            .map(|(remote, branch)| format!("{remote}/{branch}"))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_tracking_remote(&self) -> bool {
        self.upstream_remote.is_some()
    }

    #[must_use]
    pub fn worktree_for_branch<'a>(
        &self,
        worktrees: &'a [WorktreeItem],
    ) -> Option<&'a WorktreeItem> {
        worktrees
            .iter()
            .find(|worktree| worktree.branch.as_deref() == Some(self.name.as_str()))
    }

    #[must_use]
    pub fn checked_out_by_other_worktree(&self, worktrees: &[WorktreeItem]) -> bool {
        self.worktree_for_branch(worktrees)
            .map(|worktree| !worktree.is_current)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn remote_branch_stored_locally(&self) -> bool {
        self.is_tracking_remote() && self.ahead_for_pull != "?" && self.behind_for_pull != "?"
    }

    #[must_use]
    pub fn remote_branch_not_stored_locally(&self) -> bool {
        self.is_tracking_remote() && self.ahead_for_pull == "?" && self.behind_for_pull == "?"
    }

    #[must_use]
    pub fn matches_upstream(&self) -> bool {
        self.remote_branch_stored_locally()
            && self.ahead_for_pull == "0"
            && self.behind_for_pull == "0"
    }

    #[must_use]
    pub fn is_ahead_for_pull(&self) -> bool {
        self.remote_branch_stored_locally() && self.ahead_for_pull != "0"
    }

    #[must_use]
    pub fn is_behind_for_pull(&self) -> bool {
        self.remote_branch_stored_locally() && self.behind_for_pull != "0"
    }

    #[must_use]
    pub fn is_behind_for_push(&self) -> bool {
        self.remote_branch_stored_locally() && self.behind_for_push != "0"
    }

    #[must_use]
    pub fn is_real_branch(&self) -> bool {
        !self.ahead_for_pull.is_empty() && !self.behind_for_pull.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteItem {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
    pub branch_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingRemoteFlow {
    AwaitDetailAfterAdd {
        remote_name: String,
        branch_to_checkout: Option<String>,
    },
    AwaitFetchCompletion {
        remote_name: String,
        branch_to_checkout: Option<String>,
    },
    AwaitBranchCheckoutCompletion,
}

impl RemoteItem {
    #[must_use]
    pub fn ref_name(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn id(&self) -> String {
        self.ref_name()
    }

    #[must_use]
    pub fn urn(&self) -> String {
        format!("remote-{}", self.id())
    }

    #[must_use]
    pub fn description(&self) -> String {
        self.ref_name()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteBranchItem {
    pub name: String,
    pub remote_name: String,
    pub branch_name: String,
}

impl RemoteBranchItem {
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.remote_name, self.branch_name)
    }

    #[must_use]
    pub fn id(&self) -> String {
        self.ref_name()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    #[must_use]
    pub fn combined(&self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagItem {
    pub name: String,
    pub target_oid: String,
    pub target_short_oid: String,
    pub summary: String,
    pub annotated: bool,
}

impl TagItem {
    #[must_use]
    pub fn id(&self) -> String {
        self.ref_name()
    }

    #[must_use]
    pub fn urn(&self) -> String {
        format!("tag-{}", self.id())
    }
}

pub trait GitRef {
    fn full_ref_name(&self) -> String;
    fn ref_name(&self) -> String;
    fn short_ref_name(&self) -> String;
    fn parent_ref_name(&self) -> String;
    fn description(&self) -> String;
}

impl GitRef for BranchItem {
    fn full_ref_name(&self) -> String {
        if self.detached_head {
            return self.name.clone();
        }
        format!("refs/heads/{}", self.name)
    }

    fn ref_name(&self) -> String {
        self.name.clone()
    }

    fn short_ref_name(&self) -> String {
        self.ref_name()
    }

    fn parent_ref_name(&self) -> String {
        format!("{}^", self.ref_name())
    }

    fn description(&self) -> String {
        self.ref_name()
    }
}

impl GitRef for RemoteBranchItem {
    fn full_ref_name(&self) -> String {
        format!("refs/remotes/{}", self.full_name())
    }

    fn ref_name(&self) -> String {
        self.full_name()
    }

    fn short_ref_name(&self) -> String {
        self.ref_name()
    }

    fn parent_ref_name(&self) -> String {
        format!("{}^", self.ref_name())
    }

    fn description(&self) -> String {
        self.ref_name()
    }
}

impl GitRef for TagItem {
    fn full_ref_name(&self) -> String {
        format!("refs/tags/{}", self.ref_name())
    }

    fn ref_name(&self) -> String {
        self.name.clone()
    }

    fn short_ref_name(&self) -> String {
        self.ref_name()
    }

    fn parent_ref_name(&self) -> String {
        format!("{}^", self.ref_name())
    }

    fn description(&self) -> String {
        self.summary.clone()
    }
}

impl GitRef for StashItem {
    fn full_ref_name(&self) -> String {
        format!("refs/{}", self.ref_name())
    }

    fn ref_name(&self) -> String {
        format!("stash@{{{}}}", self.index)
    }

    fn short_ref_name(&self) -> String {
        self.ref_name()
    }

    fn parent_ref_name(&self) -> String {
        format!("{}^", self.ref_name())
    }

    fn description(&self) -> String {
        format!("{}: {}", self.ref_name(), self.name)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitStatus {
    #[default]
    None,
    Unpushed,
    Pushed,
    Merged,
    Rebasing,
    CherryPickingOrReverting,
    Conflicted,
    Reflog,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitTodoAction {
    #[default]
    None,
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Exec,
    Break,
    Drop,
    Label,
    Reset,
    Merge,
    UpdateRef,
    Revert,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitDivergence {
    #[default]
    None,
    Left,
    Right,
}

pub const EMPTY_TREE_COMMIT_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitItem {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub extra_info: String,
    pub author_name: String,
    pub author_email: String,
    pub unix_timestamp: i64,
    pub parents: Vec<String>,
    pub status: CommitStatus,
    pub todo_action: CommitTodoAction,
    pub todo_action_flag: String,
    pub divergence: CommitDivergence,
    pub filter_paths: Vec<PathBuf>,
    pub changed_files: Vec<CommitFileItem>,
    pub diff: DiffModel,
}

impl CommitItem {
    #[must_use]
    pub fn is_first_commit(&self) -> bool {
        self.parents.is_empty()
    }

    #[must_use]
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    #[must_use]
    pub fn is_todo(&self) -> bool {
        self.todo_action != CommitTodoAction::None
    }
}

#[must_use]
pub fn is_head_commit(commits: &[CommitItem], index: usize) -> bool {
    commits.get(index).is_some_and(|commit| {
        !commit.is_todo() && (index == 0 || commits.get(index - 1).is_some_and(CommitItem::is_todo))
    })
}

impl GitRef for CommitItem {
    fn full_ref_name(&self) -> String {
        self.oid.clone()
    }

    fn ref_name(&self) -> String {
        self.oid.clone()
    }

    fn short_ref_name(&self) -> String {
        self.short_oid.clone()
    }

    fn parent_ref_name(&self) -> String {
        if self.is_first_commit() {
            return EMPTY_TREE_COMMIT_HASH.to_string();
        }
        format!("{}^", self.ref_name())
    }

    fn description(&self) -> String {
        format!("{} {}", self.short_ref_name(), self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopiedCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitFileItem {
    pub path: PathBuf,
    pub kind: FileStatusKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StashItem {
    pub index: usize,
    pub recency: String,
    pub name: String,
    pub hash: String,
    pub stash_ref: String,
    pub label: String,
    pub changed_files: Vec<CommitFileItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflogItem {
    pub selector: String,
    pub oid: String,
    pub short_oid: String,
    pub unix_timestamp: i64,
    pub summary: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeItem {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub name: String,
    pub is_main: bool,
    pub is_current: bool,
    pub is_path_missing: bool,
    pub git_dir: Option<PathBuf>,
}

impl WorktreeItem {
    #[must_use]
    pub fn ref_name(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn id(&self) -> String {
        self.path.display().to_string()
    }

    #[must_use]
    pub fn description(&self) -> String {
        self.ref_name()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmoduleItem {
    pub name: String,
    pub path: PathBuf,
    pub url: String,
    pub branch: Option<String>,
    pub short_oid: Option<String>,
    pub initialized: bool,
    pub dirty: bool,
    pub conflicted: bool,
}

impl SubmoduleItem {
    #[must_use]
    pub fn full_name(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn full_path(&self) -> PathBuf {
        self.path.clone()
    }

    #[must_use]
    pub fn ref_name(&self) -> String {
        self.full_name()
    }

    #[must_use]
    pub fn id(&self) -> String {
        self.ref_name()
    }

    #[must_use]
    pub fn description(&self) -> String {
        self.ref_name()
    }

    #[must_use]
    pub fn git_dir_path(&self, repo_git_dir_path: &Path) -> PathBuf {
        self.name
            .split('/')
            .filter(|segment| !segment.is_empty())
            .fold(repo_git_dir_path.to_path_buf(), |path, segment| {
                path.join("modules").join(segment)
            })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeState {
    #[default]
    None,
    MergeInProgress,
    RebaseInProgress,
    CherryPickInProgress,
    RevertInProgress,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeConflictSelection {
    #[default]
    Top,
    Middle,
    Bottom,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeConflict {
    pub start: usize,
    pub ancestor: Option<usize>,
    pub target: usize,
    pub end: usize,
}

impl MergeConflict {
    #[must_use]
    pub const fn has_ancestor(&self) -> bool {
        self.ancestor.is_some()
    }

    #[must_use]
    pub fn is_marker_line(&self, index: usize) -> bool {
        index == self.start
            || self.ancestor.is_some_and(|ancestor| ancestor == index)
            || index == self.target
            || index == self.end
    }
}

impl MergeConflictSelection {
    #[must_use]
    pub fn bounds(self, conflict: &MergeConflict) -> (usize, usize) {
        match self {
            Self::Top => {
                if let Some(ancestor) = conflict.ancestor {
                    (conflict.start, ancestor)
                } else {
                    (conflict.start, conflict.target)
                }
            }
            Self::Middle => (conflict.ancestor.unwrap_or(conflict.start), conflict.target),
            Self::Bottom => (conflict.target, conflict.end),
            Self::All => (conflict.start, conflict.end),
        }
    }

    #[must_use]
    pub fn selected(self, conflict: &MergeConflict, index: usize) -> bool {
        let (start, end) = self.bounds(conflict);
        start < index && index < end
    }

    #[must_use]
    pub fn is_index_to_keep(self, conflict: &MergeConflict, index: usize) -> bool {
        if index < conflict.start || conflict.end < index {
            return true;
        }
        if conflict.is_marker_line(index) {
            return false;
        }
        self.selected(conflict, index)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeConflictSessionState {
    pub path: String,
    pub contents: Vec<String>,
    pub conflicts: Vec<MergeConflict>,
    pub conflict_index: usize,
    pub selection_index: usize,
}

impl MergeConflictSessionState {
    #[must_use]
    pub fn active(&self) -> bool {
        !self.path.is_empty()
    }

    #[must_use]
    pub fn get_content(&self) -> String {
        self.contents.last().cloned().unwrap_or_default()
    }

    #[must_use]
    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn set_content(&mut self, content: String, path: String) {
        if content == self.get_content() && path == self.path {
            return;
        }
        self.path = path;
        self.contents.clear();
        self.push_content(content);
    }

    pub fn push_content(&mut self, content: String) {
        self.contents.push(content.clone());
        self.set_conflicts(find_merge_conflicts(&content));
    }

    pub fn undo(&mut self) -> bool {
        if self.contents.len() <= 1 {
            return false;
        }
        self.contents.pop();
        self.set_conflicts(find_merge_conflicts(&self.get_content()));
        true
    }

    pub fn reset(&mut self) {
        self.contents.clear();
        self.path.clear();
        self.conflicts.clear();
        self.conflict_index = 0;
    }

    pub fn reset_conflict_selection(&mut self) {
        self.conflict_index = 0;
    }

    #[must_use]
    pub fn no_conflicts(&self) -> bool {
        self.conflicts.is_empty()
    }

    #[must_use]
    pub fn all_conflicts_resolved(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn select_next_conflict_hunk(&mut self) {
        self.set_selection_index(self.selection_index.saturating_add(1));
    }

    pub fn select_prev_conflict_hunk(&mut self) {
        self.set_selection_index(self.selection_index.saturating_sub(1));
    }

    pub fn select_next_conflict(&mut self) {
        self.set_conflict_index(self.conflict_index.saturating_add(1));
    }

    pub fn select_prev_conflict(&mut self) {
        self.set_conflict_index(self.conflict_index.saturating_sub(1));
    }

    #[must_use]
    pub fn selection(&self) -> MergeConflictSelection {
        self.available_selections()
            .get(self.selection_index)
            .copied()
            .unwrap_or(MergeConflictSelection::Top)
    }

    #[must_use]
    pub fn get_conflict_middle(&self) -> usize {
        self.current_conflict()
            .map(|conflict| conflict.target)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn get_selected_line(&self) -> usize {
        let Some(conflict) = self.current_conflict() else {
            return 1;
        };
        let (start, _) = self.selection().bounds(conflict);
        start + 1
    }

    #[must_use]
    pub fn get_selected_range(&self) -> (usize, usize) {
        let Some(conflict) = self.current_conflict() else {
            return (0, 0);
        };
        self.selection().bounds(conflict)
    }

    #[must_use]
    pub fn plain_render_selected(&self) -> String {
        let (start, end) = self.get_selected_range();
        let content_lines = split_lines(&self.get_content());
        if content_lines.is_empty() || start > end || end >= content_lines.len() {
            return String::new();
        }
        content_lines[start..=end].join("\n")
    }

    pub fn content_after_conflict_resolve(
        &self,
        selection: MergeConflictSelection,
    ) -> Option<String> {
        let conflict = self.current_conflict()?;
        let rendered = split_lines(&self.get_content())
            .into_iter()
            .enumerate()
            .filter_map(|(index, line)| selection.is_index_to_keep(conflict, index).then_some(line))
            .collect::<Vec<_>>()
            .join("\n");
        Some(rendered)
    }

    fn current_conflict(&self) -> Option<&MergeConflict> {
        self.conflicts.get(self.conflict_index)
    }

    fn available_selections(&self) -> Vec<MergeConflictSelection> {
        let Some(conflict) = self.current_conflict() else {
            return Vec::new();
        };
        if conflict.has_ancestor() {
            vec![
                MergeConflictSelection::Top,
                MergeConflictSelection::Middle,
                MergeConflictSelection::Bottom,
            ]
        } else {
            vec![MergeConflictSelection::Top, MergeConflictSelection::Bottom]
        }
    }

    fn set_conflict_index(&mut self, index: usize) {
        if self.conflicts.is_empty() {
            self.conflict_index = 0;
        } else {
            self.conflict_index = index.min(self.conflicts.len() - 1);
        }
        self.set_selection_index(self.selection_index);
    }

    fn set_selection_index(&mut self, index: usize) {
        let selections = self.available_selections();
        if !selections.is_empty() {
            self.selection_index = index.min(selections.len() - 1);
        }
    }

    fn set_conflicts(&mut self, conflicts: Vec<MergeConflict>) {
        self.conflicts = conflicts;
        self.set_conflict_index(self.conflict_index);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeConflictLineType {
    Start,
    Ancestor,
    Target,
    End,
    NotMarker,
}

#[must_use]
pub fn find_merge_conflicts(content: &str) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();
    if content.is_empty() {
        return conflicts;
    }

    let mut new_conflict: Option<MergeConflict> = None;
    for (index, line) in split_lines(content).into_iter().enumerate() {
        match determine_merge_conflict_line_type(&line) {
            MergeConflictLineType::Start => {
                new_conflict = Some(MergeConflict {
                    start: index,
                    ancestor: None,
                    target: index,
                    end: index,
                });
            }
            MergeConflictLineType::Ancestor => {
                if let Some(conflict) = new_conflict.as_mut() {
                    conflict.ancestor = Some(index);
                }
            }
            MergeConflictLineType::Target => {
                if let Some(conflict) = new_conflict.as_mut() {
                    conflict.target = index;
                }
            }
            MergeConflictLineType::End => {
                if let Some(mut conflict) = new_conflict.take() {
                    conflict.end = index;
                    conflicts.push(conflict);
                }
            }
            MergeConflictLineType::NotMarker => {}
        }
    }

    conflicts
}

fn determine_merge_conflict_line_type(line: &str) -> MergeConflictLineType {
    let trimmed_line = line.strip_prefix("++").unwrap_or(line);
    if trimmed_line.starts_with("<<<<<<< ") {
        MergeConflictLineType::Start
    } else if trimmed_line.starts_with("||||||| ") {
        MergeConflictLineType::Ancestor
    } else if trimmed_line == "=======" {
        MergeConflictLineType::Target
    } else if trimmed_line.starts_with(">>>>>>> ") {
        MergeConflictLineType::End
    } else {
        MergeConflictLineType::NotMarker
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeVariant {
    #[default]
    Regular,
    FastForward,
    NoFastForward,
    Squash,
}

impl MergeVariant {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Regular => "Merge",
            Self::FastForward => "Fast-forward merge",
            Self::NoFastForward => "Non-fast-forward merge",
            Self::Squash => "Squash merge",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeFastForwardPreference {
    #[default]
    Default,
    FastForward,
    NoFastForward,
}

impl MergeFastForwardPreference {
    #[must_use]
    pub const fn prefers_fast_forward(self) -> bool {
        matches!(self, Self::FastForward)
    }

    #[must_use]
    pub const fn prefers_no_fast_forward(self) -> bool {
        matches!(self, Self::NoFastForward)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonTarget {
    Branch(String),
    Commit(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JobId(pub String);

impl JobId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self("job-0".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundJob {
    pub id: JobId,
    pub kind: BackgroundJobKind,
    pub target_repo: Option<RepoId>,
    pub state: BackgroundJobState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundJobKind {
    RepoScan,
    RepoRefresh,
    RepoDetailLoad,
    GitCommand,
    ShellCommand,
    PersistCache,
    PersistConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundJobState {
    Queued,
    Running,
    Succeeded,
    Failed { error: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

#[derive(Clone, Copy)]
struct VisibleRepo<'a> {
    repo_id: &'a RepoId,
    summary: Option<&'a RepoSummary>,
    index: usize,
}

fn compare_visible_repos(
    left: &VisibleRepo<'_>,
    right: &VisibleRepo<'_>,
    sort_mode: WorkspaceSortMode,
) -> Ordering {
    match sort_mode {
        WorkspaceSortMode::Attention => compare_attention(left, right),
        WorkspaceSortMode::Name => display_name(left)
            .cmp(&display_name(right))
            .then_with(|| repo_path(left).cmp(&repo_path(right)))
            .then_with(|| left.index.cmp(&right.index)),
        WorkspaceSortMode::Path => repo_path(left)
            .cmp(&repo_path(right))
            .then_with(|| display_name(left).cmp(&display_name(right)))
            .then_with(|| left.index.cmp(&right.index)),
        WorkspaceSortMode::LastActivity => repo_last_activity(right)
            .cmp(&repo_last_activity(left))
            .then_with(|| {
                workspace_attention_score(right.summary)
                    .cmp(&workspace_attention_score(left.summary))
            })
            .then_with(|| display_name(left).cmp(&display_name(right)))
            .then_with(|| left.index.cmp(&right.index)),
    }
}

fn compare_attention(left: &VisibleRepo<'_>, right: &VisibleRepo<'_>) -> Ordering {
    workspace_attention_score(right.summary)
        .cmp(&workspace_attention_score(left.summary))
        .then_with(|| repo_last_activity(right).cmp(&repo_last_activity(left)))
        .then_with(|| display_name(left).cmp(&display_name(right)))
        .then_with(|| left.index.cmp(&right.index))
}

fn display_name(repo: &VisibleRepo<'_>) -> String {
    repo.summary
        .map(|summary| normalize_search_text(&summary.display_name))
        .unwrap_or_else(|| normalize_search_text(&repo.repo_id.0))
}

fn repo_path(repo: &VisibleRepo<'_>) -> String {
    repo.summary
        .map(|summary| normalize_search_text(&summary.display_path))
        .unwrap_or_else(|| normalize_search_text(&repo.repo_id.0))
}

fn repo_last_activity(repo: &VisibleRepo<'_>) -> Timestamp {
    repo.summary
        .and_then(|summary| summary.last_local_activity_at)
        .unwrap_or_default()
}

#[must_use]
pub fn workspace_attention_score(summary: Option<&RepoSummary>) -> u32 {
    let Some(summary) = summary else {
        return 0;
    };

    let mut score = 0;
    if summary.conflicted {
        score += 1_000;
    }
    if summary.ahead_count > 0 && summary.behind_count > 0 {
        score += 850;
    } else if summary.behind_count > 0 {
        score += 650 + summary.behind_count.min(25) * 10;
    }
    if summary.dirty && summary.ahead_count > 0 {
        score += 500 + summary.ahead_count.min(25) * 5;
    }
    if summary.dirty && summary.unstaged_count > 0 {
        score += 325 + summary.unstaged_count.min(25) * 3;
    }
    if summary.staged_count > 0 {
        score += 275 + summary.staged_count.min(25) * 4;
    }
    if summary.last_fetch_at.is_none() {
        score += 225;
    } else if fetch_age_seconds(summary) >= 3_600 {
        score += 175;
    }
    if summary
        .last_local_activity_at
        .zip(summary.last_refresh_at)
        .is_some_and(|(activity, refreshed)| refreshed.0.saturating_sub(activity.0) <= 3_600)
    {
        score += 90;
    }
    if summary.last_error.is_some() {
        score += 400;
    }
    score
}

fn fetch_age_seconds(summary: &RepoSummary) -> u64 {
    summary
        .last_fetch_at
        .map(|timestamp| {
            summary
                .last_refresh_at
                .unwrap_or(timestamp)
                .0
                .saturating_sub(timestamp.0)
        })
        .unwrap_or(0)
}

fn matches_search(repo_id: &RepoId, summary: Option<&RepoSummary>, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }

    let mut fields = vec![repo_id.0.as_str()];
    if let Some(summary) = summary {
        fields.push(summary.display_name.as_str());
        fields.push(summary.display_path.as_str());
        if let Some(branch) = summary.branch.as_deref() {
            fields.push(branch);
        }
        if let Some(remote_name) = summary.remote_summary.remote_name.as_deref() {
            fields.push(remote_name);
        }
        if let Some(tracking_branch) = summary.remote_summary.tracking_branch.as_deref() {
            fields.push(tracking_branch);
        }
    }

    fields.into_iter().any(|field| {
        fuzzy_matches(
            &normalize_search_text(field),
            &normalize_search_text(trimmed),
        )
    })
}

pub fn normalize_search_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn fuzzy_matches(haystack: &str, needle: &str) -> bool {
    if haystack.contains(needle) {
        return true;
    }

    let mut needle_chars = needle.chars();
    let Some(mut current) = needle_chars.next() else {
        return true;
    };

    for candidate in haystack.chars() {
        if candidate == current {
            if let Some(next) = needle_chars.next() {
                current = next;
            } else {
                return true;
            }
        }
    }

    false
}

fn repo_is_dirty(summary: &RepoSummary) -> bool {
    summary.dirty
        || summary.conflicted
        || summary.staged_count > 0
        || summary.unstaged_count > 0
        || summary.untracked_count > 0
}

#[must_use]
pub fn branch_matches_filter(branch: &BranchItem, normalized_query: &str) -> bool {
    [
        branch.name.as_str(),
        branch.display_name.as_deref().unwrap_or(""),
        branch.upstream.as_deref().unwrap_or("-"),
        branch.subject.as_str(),
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn remote_matches_filter(remote: &RemoteItem, normalized_query: &str) -> bool {
    [
        remote.name.as_str(),
        remote.fetch_url.as_str(),
        remote.push_url.as_str(),
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn remote_branch_matches_filter(branch: &RemoteBranchItem, normalized_query: &str) -> bool {
    [
        branch.name.as_str(),
        branch.remote_name.as_str(),
        branch.branch_name.as_str(),
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn tag_matches_filter(tag: &TagItem, normalized_query: &str) -> bool {
    [
        tag.name.as_str(),
        tag.target_oid.as_str(),
        tag.target_short_oid.as_str(),
        tag.summary.as_str(),
        if tag.annotated {
            "annotated"
        } else {
            "lightweight"
        },
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn commit_matches_filter(commit: &CommitItem, normalized_query: &str) -> bool {
    [
        commit.oid.as_str(),
        commit.short_oid.as_str(),
        commit.summary.as_str(),
        commit.extra_info.as_str(),
        commit.author_name.as_str(),
        commit.author_email.as_str(),
        commit.todo_action_flag.as_str(),
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
        || commit
            .parents
            .iter()
            .map(|field| normalize_search_text(field))
            .any(|field| fuzzy_matches(&field, normalized_query))
        || commit
            .tags
            .iter()
            .map(|field| normalize_search_text(field))
            .any(|field| fuzzy_matches(&field, normalized_query))
        || commit.changed_files.iter().any(|file| {
            fuzzy_matches(
                &normalize_search_text(&file.path.to_string_lossy()),
                normalized_query,
            )
        })
}

#[must_use]
pub fn commit_file_matches_filter(file: &CommitFileItem, normalized_query: &str) -> bool {
    fuzzy_matches(
        &normalize_search_text(&file.path.to_string_lossy()),
        normalized_query,
    )
}

#[must_use]
pub fn stash_matches_filter(stash: &StashItem, normalized_query: &str) -> bool {
    [stash.stash_ref.as_str(), stash.label.as_str()]
        .into_iter()
        .map(normalize_search_text)
        .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn reflog_matches_filter(entry: &ReflogItem, normalized_query: &str) -> bool {
    [
        entry.selector.as_str(),
        entry.oid.as_str(),
        entry.short_oid.as_str(),
        entry.summary.as_str(),
        entry.description.as_str(),
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn worktree_matches_filter(worktree: &WorktreeItem, normalized_query: &str) -> bool {
    [
        worktree.name.as_str(),
        worktree.path.to_string_lossy().as_ref(),
        worktree.branch.as_deref().unwrap_or("(detached)"),
        worktree.head.as_str(),
        if worktree.is_main { "main" } else { "linked" },
        if worktree.is_current {
            "current"
        } else {
            "other"
        },
        if worktree.is_path_missing {
            "missing"
        } else {
            "present"
        },
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[must_use]
pub fn submodule_matches_filter(submodule: &SubmoduleItem, normalized_query: &str) -> bool {
    [
        submodule.name.as_str(),
        submodule.path.to_string_lossy().as_ref(),
        submodule.url.as_str(),
        submodule.branch.as_deref().unwrap_or("(detached)"),
        submodule.short_oid.as_deref().unwrap_or("(uninitialized)"),
        if submodule.conflicted {
            "conflicted"
        } else if !submodule.initialized {
            "uninitialized"
        } else if submodule.dirty {
            "dirty"
        } else {
            "clean"
        },
    ]
    .into_iter()
    .map(normalize_search_text)
    .any(|field| fuzzy_matches(&field, normalized_query))
}

#[cfg(test)]
mod tests {
    use super::{
        find_merge_conflicts, visible_status_entries, workspace_attention_score, AppMode, AppState,
        Author, BranchItem, CommitFilesMode, EffectiveWorkingTreeState, FileStatus, FileStatusKind,
        GitRef, ListViewState, MergeConflict, MergeConflictSelection, MergeConflictSessionState,
        Modal, ModalKind, OperationProgress, PaneId, RemoteBranchItem, RemoteSummary, RepoDetail,
        RepoId, RepoModeState, RepoSubview, RepoSummary, StatusFilterMode, SubmoduleItem, TagItem,
        Timestamp, UiContextId, VisibleStatusEntryKind, WorkingTreeState, WorkspaceFilterMode,
        WorkspaceSortMode, WorkspaceState, DEFAULT_DIFF_CONTEXT_LINES,
        DEFAULT_RENAME_SIMILARITY_THRESHOLD,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    fn summary(repo_id: &str, display_name: &str) -> RepoSummary {
        RepoSummary {
            repo_id: RepoId::new(repo_id),
            display_name: display_name.to_string(),
            real_path: PathBuf::from(repo_id),
            display_path: repo_id.to_string(),
            remote_summary: RemoteSummary {
                remote_name: Some("origin".to_string()),
                tracking_branch: Some("origin/main".to_string()),
            },
            ..RepoSummary::default()
        }
    }

    fn status_repo_mode() -> RepoModeState {
        RepoModeState {
            detail: Some(RepoDetail {
                file_tree: vec![
                    FileStatus {
                        path: PathBuf::from("src/ui/lib.rs"),
                        kind: FileStatusKind::Modified,
                        staged_kind: Some(FileStatusKind::Modified),
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("src/ui/mod.rs"),
                        kind: FileStatusKind::Modified,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("docs/README.md"),
                        kind: FileStatusKind::Untracked,
                        staged_kind: None,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    },
                ],
                ..RepoDetail::default()
            }),
            ..RepoModeState::new(RepoId::new("repo-a"))
        }
    }

    #[test]
    fn git_ref_branch_matches_upstream_semantics() {
        let branch = BranchItem {
            name: "main".to_string(),
            ..BranchItem::default()
        };
        assert_eq!(branch.full_ref_name(), "refs/heads/main");
        assert_eq!(branch.ref_name(), "main");
        assert_eq!(branch.short_ref_name(), "main");
        assert_eq!(branch.parent_ref_name(), "main^");
        assert_eq!(branch.description(), "main");

        let detached = BranchItem {
            name: "abc1234".to_string(),
            detached_head: true,
            ..BranchItem::default()
        };
        assert_eq!(detached.full_ref_name(), "abc1234");
    }

    #[test]
    fn active_context_id_prefers_top_modal_kind() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            modal_stack: vec![Modal::new(ModalKind::Confirm, "Confirm push")],
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Branches,
                ..RepoModeState::new(RepoId::new("repo-a"))
            }),
            ..AppState::default()
        };

        assert_eq!(state.active_context_id(), UiContextId::ModalConfirm);
    }

    #[test]
    fn active_context_id_maps_repo_detail_to_active_subview() {
        let state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Reflog,
                ..RepoModeState::new(RepoId::new("repo-a"))
            }),
            ..AppState::default()
        };

        assert_eq!(state.active_context_id(), UiContextId::RepoReflog);
    }

    #[test]
    fn active_context_id_maps_setup_owned_modal_and_auxiliary_surfaces() {
        let prompt_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::Modal,
            modal_stack: vec![Modal::new(ModalKind::InputPrompt, "Prompt")],
            ..AppState::default()
        };
        assert_eq!(
            prompt_state.active_context_id(),
            UiContextId::ModalInputPrompt
        );

        let command_log_state = AppState {
            mode: AppMode::Repository,
            focused_pane: PaneId::RepoDetail,
            repo_mode: Some(RepoModeState {
                active_subview: RepoSubview::Stash,
                ..RepoModeState::new(RepoId::new("repo-a"))
            }),
            ..AppState::default()
        };
        assert_eq!(
            command_log_state.active_context_id(),
            UiContextId::RepoStash
        );

        let workspace_state = AppState {
            mode: AppMode::Workspace,
            focused_pane: PaneId::WorkspaceList,
            ..AppState::default()
        };
        assert_eq!(
            workspace_state.active_context_id(),
            UiContextId::WorkspaceList
        );
    }

    #[test]
    fn submodule_item_helpers_match_upstream_submodule_config_semantics() {
        let nested = SubmoduleItem {
            name: "vendor/child-module".to_string(),
            path: PathBuf::from("deps/child-module"),
            url: "https://example.com/child.git".to_string(),
            ..SubmoduleItem::default()
        };

        assert_eq!(nested.full_name(), "vendor/child-module");
        assert_eq!(nested.full_path(), PathBuf::from("deps/child-module"));
        assert_eq!(nested.ref_name(), "vendor/child-module");
        assert_eq!(nested.id(), "vendor/child-module");
        assert_eq!(nested.description(), "vendor/child-module");
        assert_eq!(
            nested.git_dir_path(Path::new("/repo/.git")),
            PathBuf::from("/repo/.git/modules/vendor/modules/child-module")
        );

        let top_level = SubmoduleItem {
            name: "top".to_string(),
            path: PathBuf::from("top"),
            ..SubmoduleItem::default()
        };
        assert_eq!(
            top_level.git_dir_path(Path::new("/repo/.git")),
            PathBuf::from("/repo/.git/modules/top")
        );
    }

    #[test]
    fn branch_helpers_match_upstream_semantics() {
        let branch = BranchItem {
            name: "feature/test".to_string(),
            upstream_remote: Some("origin".to_string()),
            upstream_branch: Some("feature/test".to_string()),
            ahead_for_pull: "0".to_string(),
            behind_for_pull: "0".to_string(),
            ahead_for_push: "2".to_string(),
            behind_for_push: "3".to_string(),
            ..BranchItem::default()
        };

        assert_eq!(
            branch.full_upstream_ref_name(),
            "refs/remotes/origin/feature/test"
        );
        assert_eq!(branch.urn(), "branch-feature/test");
        assert_eq!(branch.short_upstream_ref_name(), "origin/feature/test");
        assert!(branch.is_tracking_remote());
        assert!(branch.remote_branch_stored_locally());
        assert!(!branch.remote_branch_not_stored_locally());
        assert!(branch.matches_upstream());
        assert!(!branch.is_ahead_for_pull());
        assert!(!branch.is_behind_for_pull());
        assert!(branch.is_behind_for_push());
        assert!(branch.is_real_branch());
    }

    #[test]
    fn branch_helpers_handle_missing_or_unknown_upstream_state_like_lazygit() {
        let no_upstream = BranchItem {
            name: "feature/no-upstream".to_string(),
            ahead_for_pull: "".to_string(),
            behind_for_pull: "".to_string(),
            ..BranchItem::default()
        };
        assert_eq!(no_upstream.full_upstream_ref_name(), "");
        assert_eq!(no_upstream.short_upstream_ref_name(), "");
        assert!(!no_upstream.is_tracking_remote());
        assert!(!no_upstream.remote_branch_stored_locally());
        assert!(!no_upstream.remote_branch_not_stored_locally());
        assert!(!no_upstream.matches_upstream());
        assert!(!no_upstream.is_ahead_for_pull());
        assert!(!no_upstream.is_behind_for_pull());
        assert!(!no_upstream.is_behind_for_push());
        assert!(!no_upstream.is_real_branch());

        let unknown_remote_state = BranchItem {
            name: "feature/question-marks".to_string(),
            upstream_remote: Some("origin".to_string()),
            upstream_branch: Some("feature/question-marks".to_string()),
            ahead_for_pull: "?".to_string(),
            behind_for_pull: "?".to_string(),
            ..BranchItem::default()
        };
        assert!(unknown_remote_state.is_tracking_remote());
        assert!(!unknown_remote_state.remote_branch_stored_locally());
        assert!(unknown_remote_state.remote_branch_not_stored_locally());
        assert!(!unknown_remote_state.matches_upstream());
        assert!(!unknown_remote_state.is_ahead_for_pull());
        assert!(!unknown_remote_state.is_behind_for_pull());
        assert!(unknown_remote_state.is_real_branch());
    }

    #[test]
    fn detached_head_branch_helpers_match_upstream_semantics() {
        let branch = BranchItem {
            name: "6f71c57a".to_string(),
            display_name: Some("(HEAD detached at 6f71c57a)".to_string()),
            detached_head: true,
            ahead_for_pull: "?".to_string(),
            behind_for_pull: "?".to_string(),
            ..BranchItem::default()
        };

        assert_eq!(branch.full_ref_name(), "6f71c57a");
        assert_eq!(branch.ref_name(), "6f71c57a");
        assert_eq!(branch.short_ref_name(), "6f71c57a");
        assert_eq!(branch.parent_ref_name(), "6f71c57a^");
        assert_eq!(branch.urn(), "branch-6f71c57a");
        assert_eq!(branch.description(), "6f71c57a");
        assert!(!branch.is_tracking_remote());
        assert!(!branch.remote_branch_stored_locally());
        assert!(!branch.remote_branch_not_stored_locally());
        assert!(!branch.matches_upstream());
        assert!(!branch.is_ahead_for_pull());
        assert!(!branch.is_behind_for_pull());
        assert!(!branch.is_behind_for_push());
        assert!(branch.is_real_branch());
    }

    #[test]
    fn git_ref_remote_branch_matches_upstream_semantics() {
        let branch = RemoteBranchItem {
            name: "origin/feature".to_string(),
            remote_name: "origin".to_string(),
            branch_name: "feature".to_string(),
        };
        assert_eq!(branch.full_name(), "origin/feature");
        assert_eq!(branch.full_ref_name(), "refs/remotes/origin/feature");
        assert_eq!(branch.ref_name(), "origin/feature");
        assert_eq!(branch.short_ref_name(), "origin/feature");
        assert_eq!(branch.parent_ref_name(), "origin/feature^");
        assert_eq!(branch.id(), "origin/feature");
        assert_eq!(branch.description(), "origin/feature");
    }

    #[test]
    fn remote_helpers_match_upstream_semantics() {
        let remote = crate::state::RemoteItem {
            name: "origin".to_string(),
            ..crate::state::RemoteItem::default()
        };

        assert_eq!(remote.ref_name(), "origin");
        assert_eq!(remote.id(), "origin");
        assert_eq!(remote.urn(), "remote-origin");
        assert_eq!(remote.description(), "origin");
    }

    #[test]
    fn git_ref_tag_matches_upstream_semantics() {
        let tag = TagItem {
            name: "v1.2.3".to_string(),
            summary: "release summary".to_string(),
            ..TagItem::default()
        };
        assert_eq!(tag.full_ref_name(), "refs/tags/v1.2.3");
        assert_eq!(tag.ref_name(), "v1.2.3");
        assert_eq!(tag.short_ref_name(), "v1.2.3");
        assert_eq!(tag.parent_ref_name(), "v1.2.3^");
        assert_eq!(tag.id(), "v1.2.3");
        assert_eq!(tag.urn(), "tag-v1.2.3");
        assert_eq!(tag.description(), "release summary");
    }

    #[test]
    fn git_ref_commit_matches_upstream_semantics() {
        let commit = crate::state::CommitItem {
            oid: "abcdef1234567890".to_string(),
            short_oid: "abcdef1".to_string(),
            summary: "implement parity".to_string(),
            parents: vec!["parent-1".to_string()],
            ..crate::state::CommitItem::default()
        };
        assert_eq!(commit.full_ref_name(), "abcdef1234567890");
        assert_eq!(commit.ref_name(), "abcdef1234567890");
        assert_eq!(commit.short_ref_name(), "abcdef1");
        assert_eq!(commit.parent_ref_name(), "abcdef1234567890^");
        assert_eq!(commit.description(), "abcdef1 implement parity");

        let first = crate::state::CommitItem {
            oid: "1234567890abcdef".to_string(),
            short_oid: "1234567".to_string(),
            summary: "root commit".to_string(),
            ..crate::state::CommitItem::default()
        };
        assert_eq!(
            first.parent_ref_name(),
            crate::state::EMPTY_TREE_COMMIT_HASH
        );
    }

    #[test]
    fn commit_helpers_match_upstream_semantics() {
        let merge_commit = crate::state::CommitItem {
            parents: vec!["parent-1".to_string(), "parent-2".to_string()],
            ..crate::state::CommitItem::default()
        };
        assert!(merge_commit.is_merge());
        assert!(!merge_commit.is_first_commit());
        assert!(!merge_commit.is_todo());

        let todo_commit = crate::state::CommitItem {
            todo_action: crate::state::CommitTodoAction::Fixup,
            ..crate::state::CommitItem::default()
        };
        assert!(todo_commit.is_todo());
        assert!(crate::state::CommitItem::default().is_first_commit());
    }

    #[test]
    fn is_head_commit_matches_upstream_todo_boundary_semantics() {
        let normal = crate::state::CommitItem {
            oid: "normal".to_string(),
            ..crate::state::CommitItem::default()
        };
        let todo = crate::state::CommitItem {
            oid: "todo".to_string(),
            todo_action: crate::state::CommitTodoAction::Fixup,
            ..crate::state::CommitItem::default()
        };
        let after_todo = crate::state::CommitItem {
            oid: "after-todo".to_string(),
            ..crate::state::CommitItem::default()
        };

        let commits = vec![normal.clone(), todo, after_todo.clone()];
        assert!(crate::state::is_head_commit(&commits, 0));
        assert!(!crate::state::is_head_commit(&commits, 1));
        assert!(crate::state::is_head_commit(&commits, 2));

        let prefixed_by_normal = vec![normal, after_todo];
        assert!(!crate::state::is_head_commit(&prefixed_by_normal, 1));
        assert!(!crate::state::is_head_commit(&prefixed_by_normal, 99));
    }

    #[test]
    fn author_combined_matches_upstream_format() {
        let author = Author {
            name: "Jane Dev".to_string(),
            email: "jane@example.com".to_string(),
        };

        assert_eq!(author.combined(), "Jane Dev <jane@example.com>");
    }

    #[test]
    fn git_ref_stash_matches_upstream_semantics() {
        let stash = crate::state::StashItem {
            index: 2,
            name: "WIP on main: example stash".to_string(),
            ..crate::state::StashItem::default()
        };

        assert_eq!(stash.full_ref_name(), "refs/stash@{2}");
        assert_eq!(stash.ref_name(), "stash@{2}");
        assert_eq!(stash.short_ref_name(), "stash@{2}");
        assert_eq!(stash.parent_ref_name(), "stash@{2}^");
        assert_eq!(stash.description(), "stash@{2}: WIP on main: example stash");
    }

    #[test]
    fn worktree_helpers_match_upstream_semantics() {
        let branch = crate::state::BranchItem {
            name: "feature-branch".to_string(),
            ..crate::state::BranchItem::default()
        };
        let current_worktree = crate::state::WorktreeItem {
            name: "feature-branch".to_string(),
            path: PathBuf::from("/tmp/repo-feature"),
            branch: Some("feature-branch".to_string()),
            is_current: true,
            ..crate::state::WorktreeItem::default()
        };
        let other_worktree = crate::state::WorktreeItem {
            name: "feature-branch".to_string(),
            path: PathBuf::from("/tmp/repo-feature-linked"),
            branch: Some("feature-branch".to_string()),
            is_current: false,
            ..crate::state::WorktreeItem::default()
        };
        let detached_worktree = crate::state::WorktreeItem {
            name: "detached-review".to_string(),
            path: PathBuf::from("/tmp/repo-review"),
            branch: None,
            is_current: false,
            ..crate::state::WorktreeItem::default()
        };

        assert_eq!(current_worktree.ref_name(), "feature-branch");
        assert_eq!(current_worktree.id(), "/tmp/repo-feature");
        assert_eq!(current_worktree.description(), "feature-branch");

        let current_only = vec![current_worktree.clone(), detached_worktree.clone()];
        assert_eq!(
            branch
                .worktree_for_branch(&current_only)
                .map(|item| item.path.clone()),
            Some(PathBuf::from("/tmp/repo-feature"))
        );
        assert!(!branch.checked_out_by_other_worktree(&current_only));

        let other_only = vec![other_worktree.clone(), detached_worktree.clone()];
        assert_eq!(
            branch
                .worktree_for_branch(&other_only)
                .map(|item| item.path.clone()),
            Some(PathBuf::from("/tmp/repo-feature-linked"))
        );
        assert!(branch.checked_out_by_other_worktree(&other_only));

        let no_match = vec![detached_worktree];
        assert!(branch.worktree_for_branch(&no_match).is_none());
        assert!(!branch.checked_out_by_other_worktree(&no_match));
    }

    #[test]
    fn working_tree_state_effective_matches_upstream_priority() {
        assert_eq!(
            WorkingTreeState::default().effective(),
            EffectiveWorkingTreeState::None
        );
        assert_eq!(
            WorkingTreeState {
                rebasing: true,
                merging: true,
                cherry_picking: true,
                reverting: true,
            }
            .effective(),
            EffectiveWorkingTreeState::Reverting
        );
        assert_eq!(
            WorkingTreeState {
                rebasing: true,
                merging: true,
                cherry_picking: true,
                reverting: false,
            }
            .effective(),
            EffectiveWorkingTreeState::CherryPicking
        );
        assert_eq!(
            WorkingTreeState {
                rebasing: true,
                merging: true,
                cherry_picking: false,
                reverting: false,
            }
            .effective(),
            EffectiveWorkingTreeState::Merging
        );
        assert_eq!(
            WorkingTreeState {
                rebasing: true,
                ..WorkingTreeState::default()
            }
            .effective(),
            EffectiveWorkingTreeState::Rebasing
        );
    }

    #[test]
    fn working_tree_state_helpers_match_upstream_behavior() {
        let state = WorkingTreeState {
            rebasing: true,
            ..WorkingTreeState::default()
        };
        assert!(state.any());
        assert!(!state.none());
        assert_eq!(state.command_name(), "rebase");
        assert!(state.can_show_todos());
        assert!(state.can_skip());

        let merge = WorkingTreeState {
            merging: true,
            ..WorkingTreeState::default()
        };
        assert_eq!(merge.command_name(), "merge");
        assert!(!merge.can_show_todos());
        assert!(!merge.can_skip());
    }

    #[test]
    fn select_next_wraps_to_first_repo() {
        let mut workspace = WorkspaceState {
            discovered_repo_ids: vec![RepoId::new("repo-a"), RepoId::new("repo-b")],
            selected_repo_id: Some(RepoId::new("repo-b")),
            ..WorkspaceState::default()
        };

        let selected = workspace.select_next();

        assert_eq!(selected, Some(RepoId::new("repo-a")));
        assert_eq!(workspace.selected_repo_id, Some(RepoId::new("repo-a")));
    }

    #[test]
    fn select_previous_wraps_to_last_repo() {
        let mut workspace = WorkspaceState {
            discovered_repo_ids: vec![RepoId::new("repo-a"), RepoId::new("repo-b")],
            selected_repo_id: Some(RepoId::new("repo-a")),
            ..WorkspaceState::default()
        };

        let selected = workspace.select_previous();

        assert_eq!(selected, Some(RepoId::new("repo-b")));
        assert_eq!(workspace.selected_repo_id, Some(RepoId::new("repo-b")));
    }

    #[test]
    fn select_next_defaults_to_first_repo_when_selection_is_missing() {
        let mut workspace = WorkspaceState {
            discovered_repo_ids: vec![RepoId::new("repo-a"), RepoId::new("repo-b")],
            selected_repo_id: None,
            ..WorkspaceState::default()
        };

        let selected = workspace.select_next();

        assert_eq!(selected, Some(RepoId::new("repo-a")));
        assert_eq!(workspace.selected_repo_id, Some(RepoId::new("repo-a")));
    }

    #[test]
    fn selecting_in_empty_workspace_clears_selection() {
        let mut workspace = WorkspaceState {
            selected_repo_id: Some(RepoId::new("repo-a")),
            ..WorkspaceState::default()
        };

        let selected = workspace.select_next();

        assert_eq!(selected, None);
        assert_eq!(workspace.selected_repo_id, None);
    }

    #[test]
    fn list_view_selection_defaults_to_first_item() {
        let mut view = ListViewState::default();

        let selected = view.ensure_selection(3);

        assert_eq!(selected, Some(0));
        assert_eq!(view.selected_index, Some(0));
    }

    #[test]
    fn list_view_selection_wraps_with_step() {
        let mut view = ListViewState {
            selected_index: Some(0),
            selection_anchor: None,
        };

        let previous = view.select_with_step(3, -1);
        assert_eq!(previous, Some(2));
        assert_eq!(view.selected_index, Some(2));

        let next = view.select_with_step(3, 1);
        assert_eq!(next, Some(0));
        assert_eq!(view.selected_index, Some(0));
    }

    #[test]
    fn repo_mode_state_new_starts_on_status_subview_with_idle_progress() {
        let state = RepoModeState::new(RepoId::new("repo-a"));

        assert_eq!(state.current_repo_id, RepoId::new("repo-a"));
        assert_eq!(state.active_subview, RepoSubview::Status);
        assert_eq!(state.commit_files_mode, CommitFilesMode::List);
        assert_eq!(state.diff_scroll, 0);
        assert_eq!(state.diff_line_cursor, None);
        assert_eq!(state.diff_line_anchor, None);
        assert_eq!(state.operation_progress, OperationProgress::Idle);
        assert!(!state.ignore_whitespace_in_diff);
        assert_eq!(state.diff_context_lines, DEFAULT_DIFF_CONTEXT_LINES);
        assert_eq!(
            state.rename_similarity_threshold,
            DEFAULT_RENAME_SIMILARITY_THRESHOLD
        );
        assert!(state.detail.is_none());
    }

    #[test]
    fn status_filter_mode_cycles_through_all_modes() {
        assert_eq!(
            StatusFilterMode::All.cycle_next(),
            StatusFilterMode::TrackedOnly
        );
        assert_eq!(
            StatusFilterMode::TrackedOnly.cycle_next(),
            StatusFilterMode::UntrackedOnly
        );
        assert_eq!(
            StatusFilterMode::UntrackedOnly.cycle_next(),
            StatusFilterMode::ConflictedOnly
        );
        assert_eq!(
            StatusFilterMode::ConflictedOnly.cycle_next(),
            StatusFilterMode::All
        );
    }

    #[test]
    fn visible_status_entries_build_tree_and_honor_collapsed_directories() {
        let mut repo_mode = status_repo_mode();

        let expanded = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| {
                (
                    entry.path.display().to_string(),
                    entry.depth,
                    entry.entry_kind,
                    entry.label,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            expanded,
            vec![
                (
                    "docs".to_string(),
                    0,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    "docs".to_string(),
                ),
                (
                    "docs/README.md".to_string(),
                    1,
                    VisibleStatusEntryKind::File,
                    "README.md".to_string(),
                ),
                (
                    "src/ui".to_string(),
                    0,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    "src/ui".to_string(),
                ),
                (
                    "src/ui/lib.rs".to_string(),
                    1,
                    VisibleStatusEntryKind::File,
                    "lib.rs".to_string(),
                ),
                (
                    "src/ui/mod.rs".to_string(),
                    1,
                    VisibleStatusEntryKind::File,
                    "mod.rs".to_string(),
                ),
            ]
        );

        repo_mode
            .collapsed_status_dirs
            .insert(PathBuf::from("src/ui"));
        let collapsed = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| (entry.path.display().to_string(), entry.entry_kind))
            .collect::<Vec<_>>();
        assert_eq!(
            collapsed,
            vec![
                (
                    "docs".to_string(),
                    VisibleStatusEntryKind::Directory { collapsed: false },
                ),
                ("docs/README.md".to_string(), VisibleStatusEntryKind::File),
                (
                    "src/ui".to_string(),
                    VisibleStatusEntryKind::Directory { collapsed: true },
                ),
            ]
        );
    }

    #[test]
    fn visible_status_entries_show_root_item_when_enabled() {
        let mut repo_mode = status_repo_mode();
        repo_mode.show_root_item_in_file_tree = true;

        let entries = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| {
                (
                    entry.path.display().to_string(),
                    entry.depth,
                    entry.entry_kind,
                    entry.label,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            entries,
            vec![
                (
                    ".".to_string(),
                    0,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    ".".to_string(),
                ),
                (
                    "docs".to_string(),
                    1,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    "docs".to_string(),
                ),
                (
                    "docs/README.md".to_string(),
                    2,
                    VisibleStatusEntryKind::File,
                    "README.md".to_string(),
                ),
                (
                    "src/ui".to_string(),
                    1,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    "src/ui".to_string(),
                ),
                (
                    "src/ui/lib.rs".to_string(),
                    2,
                    VisibleStatusEntryKind::File,
                    "lib.rs".to_string(),
                ),
                (
                    "src/ui/mod.rs".to_string(),
                    2,
                    VisibleStatusEntryKind::File,
                    "mod.rs".to_string(),
                ),
            ]
        );

        repo_mode.collapsed_status_dirs.insert(PathBuf::from("."));
        let collapsed = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| (entry.path.display().to_string(), entry.entry_kind))
            .collect::<Vec<_>>();
        assert_eq!(
            collapsed,
            vec![(
                ".".to_string(),
                VisibleStatusEntryKind::Directory { collapsed: true },
            )]
        );
    }

    #[test]
    fn visible_status_entries_apply_status_mode_and_query_filter() {
        let mut repo_mode = status_repo_mode();
        repo_mode.status_filter_mode = StatusFilterMode::UntrackedOnly;

        let untracked = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| entry.path.display().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            untracked,
            vec!["docs".to_string(), "docs/README.md".to_string()]
        );

        repo_mode.status_filter_mode = StatusFilterMode::All;
        repo_mode.status_filter.query = "mod".to_string();
        let queried = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| entry.path.display().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            queried,
            vec![
                "src/ui".to_string(),
                "src/ui/lib.rs".to_string(),
                "src/ui/mod.rs".to_string(),
            ]
        );
    }

    #[test]
    fn visible_status_entries_compress_single_child_directory_chains() {
        let repo_mode = RepoModeState {
            detail: Some(RepoDetail {
                file_tree: vec![FileStatus {
                    path: PathBuf::from("src/ui/lib.rs"),
                    kind: FileStatusKind::Modified,
                    unstaged_kind: Some(FileStatusKind::Modified),
                    ..FileStatus::default()
                }],
                ..RepoDetail::default()
            }),
            ..RepoModeState::new(RepoId::new("repo-a"))
        };

        let entries = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| {
                (
                    entry.path.display().to_string(),
                    entry.depth,
                    entry.entry_kind,
                    entry.label,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            entries,
            vec![
                (
                    "src/ui".to_string(),
                    0,
                    VisibleStatusEntryKind::Directory { collapsed: false },
                    "src/ui".to_string(),
                ),
                (
                    "src/ui/lib.rs".to_string(),
                    1,
                    VisibleStatusEntryKind::File,
                    "lib.rs".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn visible_status_entries_flat_mode_orders_conflicts_then_tracked_then_untracked() {
        let mut repo_mode = RepoModeState {
            status_tree_enabled: false,
            detail: Some(RepoDetail {
                file_tree: vec![
                    FileStatus {
                        path: PathBuf::from("a2"),
                        short_status: "??".to_string(),
                        kind: FileStatusKind::Untracked,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("a1"),
                        short_status: "??".to_string(),
                        kind: FileStatusKind::Untracked,
                        unstaged_kind: Some(FileStatusKind::Untracked),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("c2"),
                        short_status: "UU".to_string(),
                        kind: FileStatusKind::Conflicted,
                        unstaged_kind: Some(FileStatusKind::Conflicted),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("c1"),
                        short_status: "AA".to_string(),
                        kind: FileStatusKind::Conflicted,
                        unstaged_kind: Some(FileStatusKind::Conflicted),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("b2"),
                        short_status: "M ".to_string(),
                        kind: FileStatusKind::Modified,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    },
                    FileStatus {
                        path: PathBuf::from("b1"),
                        short_status: "M ".to_string(),
                        kind: FileStatusKind::Modified,
                        unstaged_kind: Some(FileStatusKind::Modified),
                        ..FileStatus::default()
                    },
                ],
                ..RepoDetail::default()
            }),
            ..RepoModeState::new(RepoId::new("repo-a"))
        };

        repo_mode.status_filter_mode = StatusFilterMode::All;
        let entries = visible_status_entries(&repo_mode, PaneId::RepoUnstaged)
            .into_iter()
            .map(|entry| entry.path.display().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            entries,
            vec![
                "c1".to_string(),
                "c2".to_string(),
                "b1".to_string(),
                "b2".to_string(),
                "a1".to_string(),
                "a2".to_string(),
            ]
        );
    }

    #[test]
    fn status_tree_node_get_visual_depth_at_index_matches_lazygit_cases() {
        struct Scenario {
            name: &'static str,
            repo_mode: RepoModeState,
            expected_depths: Vec<usize>,
        }

        let scenarios = vec![
            Scenario {
                name: "flat files with root item",
                repo_mode: RepoModeState {
                    show_root_item_in_file_tree: true,
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 1, 1],
            },
            Scenario {
                name: "flat files without root item",
                repo_mode: RepoModeState {
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 0],
            },
            Scenario {
                name: "nested directories with root item",
                repo_mode: RepoModeState {
                    show_root_item_in_file_tree: true,
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("dir/a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("dir/b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("c"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 1, 2, 2, 1],
            },
            Scenario {
                name: "compressed paths with root item",
                repo_mode: RepoModeState {
                    show_root_item_in_file_tree: true,
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("dir1/dir3/a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("dir2/dir4/b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 1, 2, 1, 2],
            },
            Scenario {
                name: "compressed paths without root item",
                repo_mode: RepoModeState {
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("dir1/dir3/a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("dir2/dir4/b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 1, 0, 1],
            },
            Scenario {
                name: "collapsed directory hides children",
                repo_mode: RepoModeState {
                    show_root_item_in_file_tree: true,
                    collapsed_status_dirs: BTreeSet::from([PathBuf::from("dir")]),
                    detail: Some(RepoDetail {
                        file_tree: vec![
                            FileStatus {
                                path: PathBuf::from("dir/a"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("dir/b"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                            FileStatus {
                                path: PathBuf::from("c"),
                                unstaged_kind: Some(FileStatusKind::Modified),
                                ..FileStatus::default()
                            },
                        ],
                        ..RepoDetail::default()
                    }),
                    ..RepoModeState::new(RepoId::new("repo-a"))
                },
                expected_depths: vec![0, 1, 1],
            },
        ];

        for scenario in scenarios {
            let detail = scenario.repo_mode.detail.as_ref().expect("detail");
            let files = detail.file_tree.iter().collect::<Vec<_>>();
            let tree = super::build_status_tree(files);
            for (index, expected_depth) in scenario.expected_depths.iter().enumerate() {
                let actual_depth = tree
                    .get_visual_depth_at_index(
                        index,
                        &scenario.repo_mode.collapsed_status_dirs,
                        scenario.repo_mode.show_root_item_in_file_tree,
                    )
                    .expect("depth in range");
                assert_eq!(
                    actual_depth, *expected_depth,
                    "{} index {} depth mismatch",
                    scenario.name, index
                );
            }

            assert_eq!(
                tree.get_visual_depth_at_index(
                    scenario.expected_depths.len(),
                    &scenario.repo_mode.collapsed_status_dirs,
                    scenario.repo_mode.show_root_item_in_file_tree,
                ),
                None,
                "{} expected out-of-range depth to be None",
                scenario.name
            );
        }
    }

    #[test]
    fn file_status_helpers_match_upstream_file_model_semantics() {
        let renamed = FileStatus {
            path: PathBuf::from("src/new_name.rs"),
            previous_path: Some(PathBuf::from("src/old_name.rs")),
            short_status: "R ".to_string(),
            staged_kind: Some(FileStatusKind::Renamed),
            ..FileStatus::default()
        };
        let same_file = FileStatus {
            path: PathBuf::from("src/old_name.rs"),
            short_status: "M ".to_string(),
            ..FileStatus::default()
        };

        assert!(renamed.is_rename());
        assert!(renamed.matches_file(&same_file));
        assert!(renamed.has_staged_changes());
        assert!(renamed.has_staged_or_tracked_changes());
        assert!(!renamed.has_unstaged_changes());

        let tracked_without_staged_changes = FileStatus {
            path: PathBuf::from("src/lib.rs"),
            short_status: " M".to_string(),
            unstaged_kind: Some(FileStatusKind::Modified),
            ..FileStatus::default()
        };
        let untracked_without_staged_changes = FileStatus {
            path: PathBuf::from("README.new"),
            short_status: "??".to_string(),
            unstaged_kind: Some(FileStatusKind::Untracked),
            ..FileStatus::default()
        };

        assert!(tracked_without_staged_changes.tracked());
        assert!(tracked_without_staged_changes.has_unstaged_changes());
        assert!(tracked_without_staged_changes.has_staged_or_tracked_changes());
        assert!(!untracked_without_staged_changes.tracked());
        assert!(!untracked_without_staged_changes.has_staged_or_tracked_changes());

        let derived = FileStatus::derived_status_fields("AM");
        assert!(derived.has_staged_changes);
        assert!(derived.has_unstaged_changes);
        assert!(!derived.tracked);
        assert!(derived.added);
        assert!(!derived.deleted);
        assert!(!derived.has_merge_conflicts);
        assert!(!derived.has_inline_merge_conflicts);
    }

    #[test]
    fn file_status_merge_conflict_descriptions_match_upstream_copy() {
        let dd = FileStatus {
            short_status: "DD".to_string(),
            ..FileStatus::default()
        };
        let ud = FileStatus {
            short_status: "UD".to_string(),
            ..FileStatus::default()
        };
        let uu = FileStatus {
            short_status: "UU".to_string(),
            ..FileStatus::default()
        };

        assert!(dd.has_merge_conflicts());
        assert!(!dd.has_inline_merge_conflicts());
        assert_eq!(
            dd.merge_state_description(),
            Some("Conflict: this file was moved or renamed both in the current and the incoming changes, but to different destinations. I don't know which ones, but they should both show up as conflicts too (marked 'AU' and 'UA', respectively). The most likely resolution is to delete this file, and pick one of the destinations and delete the other.")
        );
        assert_eq!(
            ud.merge_state_description(),
            Some("Conflict: this file was modified in the current changes and deleted in incoming changes.\n\nThe most likely resolution is to delete the file after applying the current modifications manually to some other place in the code.")
        );
        assert!(uu.has_merge_conflicts());
        assert!(uu.has_inline_merge_conflicts());
        assert_eq!(uu.merge_state_description(), None);
    }

    #[test]
    fn find_merge_conflicts_matches_upstream_cases() {
        let content = "++<<<<<<< HEAD\nfoo\n++=======\nbar\n++>>>>>>> branch\n\n<<<<<<< HEAD\nfoo\n||||||| ancestor\nbar\n=======\nbaz\n>>>>>>> branch\n";

        assert_eq!(
            find_merge_conflicts(content),
            vec![
                MergeConflict {
                    start: 0,
                    ancestor: None,
                    target: 2,
                    end: 4,
                },
                MergeConflict {
                    start: 6,
                    ancestor: Some(8),
                    target: 10,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn merge_conflict_session_selection_and_content_stack_match_upstream_semantics() {
        let mut state = MergeConflictSessionState::default();
        let content = "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\n";
        state.set_content(content.to_string(), "conflict.txt".to_string());

        assert!(state.active());
        assert_eq!(state.get_path(), "conflict.txt");
        assert_eq!(state.conflicts.len(), 1);
        assert_eq!(state.selection(), MergeConflictSelection::Top);
        assert_eq!(state.get_selected_line(), 1);
        assert_eq!(state.get_selected_range(), (0, 2));
        assert_eq!(state.plain_render_selected(), "<<<<<<< HEAD\nours\n=======");

        state.select_next_conflict_hunk();
        assert_eq!(state.selection(), MergeConflictSelection::Bottom);
        assert_eq!(state.get_selected_range(), (2, 4));
        assert_eq!(
            state.content_after_conflict_resolve(state.selection()),
            Some("theirs".to_string())
        );

        state.push_content("resolved\n".to_string());
        assert!(state.all_conflicts_resolved());
        assert!(state.undo());
        assert!(!state.all_conflicts_resolved());
        assert_eq!(state.get_content(), content.to_string());
    }

    #[test]
    fn merge_conflict_session_clamps_selection_and_resets_like_upstream() {
        let mut state = MergeConflictSessionState::default();
        let content =
            "<<<<<<< HEAD\nours\n||||||| ancestor\nbase\n=======\ntheirs\n>>>>>>> branch\n";
        state.set_content(content.to_string(), "conflict.txt".to_string());

        state.select_next_conflict_hunk();
        state.select_next_conflict_hunk();
        state.select_next_conflict_hunk();
        assert_eq!(state.selection(), MergeConflictSelection::Bottom);
        assert_eq!(state.get_conflict_middle(), 4);

        state.reset_conflict_selection();
        assert_eq!(state.conflict_index, 0);
        assert_eq!(state.selection(), MergeConflictSelection::Bottom);

        state.reset();
        assert!(!state.active());
        assert!(state.contents.is_empty());
        assert!(state.conflicts.is_empty());
    }

    #[test]
    fn visible_repo_ids_apply_attention_sort_filter_and_search() {
        let repo_conflict = RepoId::new("/tmp/repo-conflict");
        let repo_dirty = RepoId::new("/tmp/repo-dirty");
        let repo_behind = RepoId::new("/tmp/repo-behind");
        let repo_clean = RepoId::new("/tmp/repo-clean");
        let mut conflict = summary(&repo_conflict.0, "conflict");
        conflict.conflicted = true;
        conflict.dirty = true;
        conflict.behind_count = 2;
        conflict.last_local_activity_at = Some(Timestamp(110));
        conflict.last_refresh_at = Some(Timestamp(120));
        let mut dirty = summary(&repo_dirty.0, "dirty");
        dirty.dirty = true;
        dirty.unstaged_count = 2;
        dirty.last_local_activity_at = Some(Timestamp(90));
        dirty.last_refresh_at = Some(Timestamp(120));
        let mut behind = summary(&repo_behind.0, "behind");
        behind.behind_count = 4;
        behind.branch = Some("feature/triage".to_string());
        behind.last_refresh_at = Some(Timestamp(120));
        let mut clean = summary(&repo_clean.0, "clean");
        clean.branch = Some("release".to_string());
        clean.last_refresh_at = Some(Timestamp(120));

        let workspace = WorkspaceState {
            discovered_repo_ids: vec![
                repo_clean.clone(),
                repo_dirty.clone(),
                repo_behind.clone(),
                repo_conflict.clone(),
            ],
            repo_summaries: BTreeMap::from([
                (repo_conflict.clone(), conflict),
                (repo_dirty.clone(), dirty),
                (repo_behind.clone(), behind),
                (repo_clean.clone(), clean),
            ]),
            ..WorkspaceState::default()
        };

        assert_eq!(
            workspace.visible_repo_ids(),
            vec![
                repo_conflict.clone(),
                repo_behind.clone(),
                repo_dirty.clone(),
                repo_clean
            ]
        );

        let mut behind_only = workspace.clone();
        behind_only.filter_mode = WorkspaceFilterMode::BehindOnly;
        assert_eq!(
            behind_only.visible_repo_ids(),
            vec![repo_conflict, repo_behind]
        );

        let mut search = workspace;
        search.search_query = "ftg".to_string();
        assert_eq!(
            search.visible_repo_ids(),
            vec![RepoId::new("/tmp/repo-behind")]
        );
    }

    #[test]
    fn ensure_visible_selection_falls_back_to_first_visible_repo() {
        let repo_a = RepoId::new("/tmp/repo-a");
        let repo_b = RepoId::new("/tmp/repo-b");
        let mut summary_a = summary(&repo_a.0, "alpha");
        summary_a.dirty = true;
        let summary_b = summary(&repo_b.0, "beta");
        let mut workspace = WorkspaceState {
            discovered_repo_ids: vec![repo_a.clone(), repo_b.clone()],
            repo_summaries: BTreeMap::from([
                (repo_a.clone(), summary_a),
                (repo_b.clone(), summary_b),
            ]),
            selected_repo_id: Some(repo_b),
            filter_mode: WorkspaceFilterMode::DirtyOnly,
            ..WorkspaceState::default()
        };

        let selected = workspace.ensure_visible_selection();

        assert_eq!(selected, Some(repo_a.clone()));
        assert_eq!(workspace.selected_repo_id, Some(repo_a));
    }

    #[test]
    fn workspace_search_match_navigation_cycles_visible_results() {
        let repo_a = RepoId::new("/tmp/repo-alpha");
        let repo_b = RepoId::new("/tmp/repo-beta");
        let repo_c = RepoId::new("/tmp/repo-gamma");
        let workspace = WorkspaceState {
            discovered_repo_ids: vec![repo_a.clone(), repo_b.clone(), repo_c.clone()],
            repo_summaries: BTreeMap::from([
                (repo_a.clone(), summary(&repo_a.0, "alpha")),
                (repo_b.clone(), summary(&repo_b.0, "beta")),
                (repo_c.clone(), summary(&repo_c.0, "alphabet soup")),
            ]),
            selected_repo_id: Some(repo_a.clone()),
            search_query: "alp".to_string(),
            ..WorkspaceState::default()
        };

        let mut workspace = workspace;
        assert_eq!(workspace.current_search_match_position(), Some((0, 2)));
        assert_eq!(workspace.select_next_search_match(), Some(repo_c.clone()));
        assert_eq!(workspace.current_search_match_position(), Some((1, 2)));
        assert_eq!(workspace.select_next_search_match(), Some(repo_a.clone()));
        assert_eq!(workspace.select_previous_search_match(), Some(repo_c));
        assert_eq!(workspace.current_search_match_position(), Some((1, 2)));
    }

    #[test]
    fn name_sort_uses_display_name_instead_of_discovery_order() {
        let repo_z = RepoId::new("/tmp/repo-z");
        let repo_a = RepoId::new("/tmp/repo-a");
        let workspace = WorkspaceState {
            discovered_repo_ids: vec![repo_z.clone(), repo_a.clone()],
            repo_summaries: BTreeMap::from([
                (repo_z.clone(), summary(&repo_z.0, "zulu")),
                (repo_a.clone(), summary(&repo_a.0, "alpha")),
            ]),
            sort_mode: WorkspaceSortMode::Name,
            ..WorkspaceState::default()
        };

        assert_eq!(workspace.visible_repo_ids(), vec![repo_a, repo_z]);
    }

    #[test]
    fn attention_score_prioritizes_conflicts_and_sync_pressure() {
        let mut conflict = summary("/tmp/conflict", "conflict");
        conflict.conflicted = true;
        conflict.behind_count = 1;
        conflict.last_refresh_at = Some(Timestamp(100));
        let mut behind = summary("/tmp/behind", "behind");
        behind.behind_count = 3;
        behind.last_refresh_at = Some(Timestamp(100));
        let mut clean = summary("/tmp/clean", "clean");
        clean.last_refresh_at = Some(Timestamp(100));

        assert!(
            workspace_attention_score(Some(&conflict)) > workspace_attention_score(Some(&behind))
        );
        assert!(workspace_attention_score(Some(&behind)) > workspace_attention_score(Some(&clean)));
    }

    #[test]
    fn list_view_selection_range_defaults_to_selected_index_without_anchor() {
        let mut state = ListViewState::default();
        state.set_selected(4, 2);

        assert_eq!(state.selection_range(4), Some((2, 2)));
        assert_eq!(state.selection_anchor, None);
    }

    #[test]
    fn list_view_selection_range_tracks_anchor_and_selected_index() {
        let mut state = ListViewState::default();
        state.set_selected(5, 1);
        assert_eq!(state.toggle_selection_anchor(5), Some(1));

        state.set_selected(5, 3);
        assert_eq!(state.selection_range(5), Some((1, 3)));

        state.set_selected(5, 0);
        assert_eq!(state.selection_range(5), Some((0, 1)));
    }

    #[test]
    fn list_view_selected_items_and_ids_match_upstream_range_semantics() {
        #[derive(Debug, Clone, PartialEq, Eq)]
        struct Item {
            id: &'static str,
            value: i32,
        }

        let items = vec![
            Item { id: "a", value: 10 },
            Item { id: "b", value: 20 },
            Item { id: "c", value: 30 },
            Item { id: "d", value: 40 },
        ];
        let mut state = ListViewState::default();
        state.set_selected(items.len(), 1);
        state.toggle_selection_anchor(items.len());
        state.set_selected(items.len(), 3);

        let (selected, start, end) = state.selected_items(&items).expect("selected items");
        assert_eq!((start, end), (1, 3));
        assert_eq!(
            selected.iter().map(|item| item.value).collect::<Vec<_>>(),
            vec![20, 30, 40]
        );

        let (ids, start, end) = state
            .selected_item_ids(&items, |item| item.id)
            .expect("selected item ids");
        assert_eq!((start, end), (1, 3));
        assert_eq!(ids, vec!["b".to_string(), "c".to_string(), "d".to_string()]);
    }

    #[test]
    fn list_view_selected_item_and_id_follow_current_selection() {
        #[derive(Debug, Clone, PartialEq, Eq)]
        struct Item {
            id: &'static str,
        }

        let items = vec![Item { id: "left" }, Item { id: "right" }];
        let mut state = ListViewState::default();

        assert_eq!(
            state.selected_item(&items).map(|item| item.id),
            Some("left")
        );
        assert_eq!(
            state.selected_item_id(&items, |item| item.id),
            Some("left".to_string())
        );

        state.select_last(items.len());
        assert_eq!(
            state.selected_item(&items).map(|item| item.id),
            Some("right")
        );
        assert_eq!(
            state.selected_item_id(&items, |item| item.id),
            Some("right".to_string())
        );
    }

    #[test]
    fn list_view_clears_anchor_when_list_becomes_empty() {
        let mut state = ListViewState::default();
        state.set_selected(3, 2);
        state.toggle_selection_anchor(3);

        assert_eq!(state.ensure_selection(0), None);
        assert_eq!(state.selected_index, None);
        assert_eq!(state.selection_anchor, None);
    }

    #[test]
    fn prioritized_repo_ids_orders_active_then_visible_then_hidden() {
        let repo_active = RepoId::new("/tmp/active");
        let repo_visible = RepoId::new("/tmp/visible");
        let repo_hidden = RepoId::new("/tmp/hidden");
        let mut visible = summary(&repo_visible.0, "visible");
        visible.dirty = true;
        let workspace = WorkspaceState {
            discovered_repo_ids: vec![
                repo_hidden.clone(),
                repo_visible.clone(),
                repo_active.clone(),
            ],
            repo_summaries: BTreeMap::from([
                (repo_active.clone(), summary(&repo_active.0, "active")),
                (repo_visible.clone(), visible),
                (repo_hidden.clone(), summary(&repo_hidden.0, "hidden")),
            ]),
            filter_mode: WorkspaceFilterMode::DirtyOnly,
            ..WorkspaceState::default()
        };

        let ordered = workspace.prioritized_repo_ids(
            &[
                repo_hidden.clone(),
                repo_visible.clone(),
                repo_active.clone(),
            ],
            Some(&repo_active),
        );

        assert_eq!(ordered, vec![repo_active, repo_visible, repo_hidden]);
    }
}
