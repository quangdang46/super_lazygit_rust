use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub mode: AppMode,
    pub focused_pane: PaneId,
    pub modal_stack: Vec<Modal>,
    pub status_messages: VecDeque<StatusMessage>,
    pub notifications: VecDeque<Notification>,
    pub background_jobs: BTreeMap<JobId, BackgroundJob>,
    pub settings: SettingsSnapshot,
    pub recent_repo_stack: Vec<RepoId>,
    pub workspace: WorkspaceState,
    pub repo_mode: Option<RepoModeState>,
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
    CommandPalette,
    InputPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusMessage {
    pub id: u64,
    pub level: MessageLevel,
    pub text: String,
}

impl StatusMessage {
    #[must_use]
    pub fn info(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            level: MessageLevel::Info,
            text: text.into(),
        }
    }

    #[must_use]
    pub fn error(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            level: MessageLevel::Error,
            text: text.into(),
        }
    }
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
    pub fn select_next(&mut self) -> Option<RepoId> {
        self.select_with_step(1)
    }

    pub fn select_previous(&mut self) -> Option<RepoId> {
        self.select_with_step(-1)
    }

    fn select_with_step(&mut self, step: isize) -> Option<RepoId> {
        if self.discovered_repo_ids.is_empty() {
            self.selected_repo_id = None;
            return None;
        }

        let current_index = self
            .selected_repo_id
            .as_ref()
            .and_then(|selected| {
                self.discovered_repo_ids
                    .iter()
                    .position(|candidate| candidate == selected)
            })
            .unwrap_or(0);

        let len = self.discovered_repo_ids.len() as isize;
        let next_index = (current_index as isize + step).rem_euclid(len) as usize;
        let next_repo = self.discovered_repo_ids[next_index].clone();
        self.selected_repo_id = Some(next_repo.clone());
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceFilterMode {
    #[default]
    All,
    DirtyOnly,
    ConflictedOnly,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewMode {
    #[default]
    Summary,
    DiffPreview,
    Hidden,
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
    pub active_subview: RepoSubview,
    pub diff_scroll: usize,
    pub status_view: ListViewState,
    pub branches_view: ListViewState,
    pub commits_view: ListViewState,
    pub stash_view: ListViewState,
    pub reflog_view: ListViewState,
    pub worktree_view: ListViewState,
    pub operation_progress: OperationProgress,
    pub detail: Option<RepoDetail>,
}

impl RepoModeState {
    #[must_use]
    pub fn new(current_repo_id: RepoId) -> Self {
        Self {
            current_repo_id,
            active_subview: RepoSubview::default(),
            diff_scroll: 0,
            status_view: ListViewState::default(),
            branches_view: ListViewState::default(),
            commits_view: ListViewState::default(),
            stash_view: ListViewState::default(),
            reflog_view: ListViewState::default(),
            worktree_view: ListViewState::default(),
            operation_progress: OperationProgress::Idle,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoSubview {
    #[default]
    Status,
    Branches,
    Commits,
    Stash,
    Reflog,
    Worktrees,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListViewState {
    pub selected_index: Option<usize>,
}

impl ListViewState {
    pub fn ensure_selection(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            return None;
        }

        let selected = self
            .selected_index
            .filter(|index| *index < len)
            .unwrap_or(0);
        self.selected_index = Some(selected);
        Some(selected)
    }

    pub fn select_with_step(&mut self, len: usize, step: isize) -> Option<usize> {
        if len == 0 {
            self.selected_index = None;
            return None;
        }

        let current = self.ensure_selection(len).unwrap_or(0);
        let next = (current as isize + step).rem_euclid(len as isize) as usize;
        self.selected_index = Some(next);
        Some(next)
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
    pub commits: Vec<CommitItem>,
    pub stashes: Vec<StashItem>,
    pub reflog_items: Vec<ReflogItem>,
    pub worktrees: Vec<WorktreeItem>,
    pub commit_input: String,
    pub merge_state: MergeState,
    pub comparison_target: Option<ComparisonTarget>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: PathBuf,
    pub kind: FileStatusKind,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffModel {
    pub selected_path: Option<PathBuf>,
    pub lines: Vec<DiffLine>,
    pub hunk_count: usize,
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
    pub is_head: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitItem {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub changed_files: Vec<CommitFileItem>,
    pub diff: DiffModel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitFileItem {
    pub path: PathBuf,
    pub kind: FileStatusKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StashItem {
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflogItem {
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeItem {
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeState {
    #[default]
    None,
    MergeInProgress,
    RebaseInProgress,
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

#[cfg(test)]
mod tests {
    use super::{
        ListViewState, OperationProgress, RepoId, RepoModeState, RepoSubview, WorkspaceState,
    };

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

        assert_eq!(selected, Some(RepoId::new("repo-b")));
        assert_eq!(workspace.selected_repo_id, Some(RepoId::new("repo-b")));
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
        assert_eq!(state.diff_scroll, 0);
        assert_eq!(state.operation_progress, OperationProgress::Idle);
        assert!(state.detail.is_none());
    }
}
