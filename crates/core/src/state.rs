use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub mode: AppMode,
    pub focused_pane: PaneId,
    pub modal_stack: Vec<Modal>,
    pub pending_confirmation: Option<PendingConfirmation>,
    pub pending_input_prompt: Option<PendingInputPrompt>,
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
pub struct PendingConfirmation {
    pub repo_id: RepoId,
    pub operation: ConfirmableOperation,
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
    FixupCommit {
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
    },
    DropStash {
        stash_ref: String,
    },
    RemoveWorktree {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInputPrompt {
    pub repo_id: RepoId,
    pub operation: InputPromptOperation,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputPromptOperation {
    CreateBranch,
    RenameBranch {
        current_name: String,
    },
    SetBranchUpstream {
        branch_name: String,
    },
    CreateWorktree,
    RewordCommit {
        commit: String,
        summary: String,
        initial_message: String,
    },
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
            return None;
        }

        if self
            .selected_repo_id
            .as_ref()
            .is_some_and(|selected| visible_repo_ids.iter().any(|repo_id| repo_id == selected))
        {
            return self.selected_repo_id.clone();
        }

        let next_repo = visible_repo_ids[0].clone();
        self.selected_repo_id = Some(next_repo.clone());
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
    pub diff_line_cursor: Option<usize>,
    pub diff_line_anchor: Option<usize>,
    pub status_view: ListViewState,
    pub staged_view: ListViewState,
    pub commit_box: CommitBoxState,
    pub branches_view: ListViewState,
    pub commits_view: ListViewState,
    pub stash_view: ListViewState,
    pub reflog_view: ListViewState,
    pub worktree_view: ListViewState,
    pub operation_progress: OperationProgress,
    pub comparison_base: Option<ComparisonTarget>,
    pub comparison_target: Option<ComparisonTarget>,
    pub comparison_source: Option<RepoSubview>,
    pub detail: Option<RepoDetail>,
}

impl RepoModeState {
    #[must_use]
    pub fn new(current_repo_id: RepoId) -> Self {
        Self {
            current_repo_id,
            active_subview: RepoSubview::default(),
            diff_scroll: 0,
            diff_line_cursor: None,
            diff_line_anchor: None,
            status_view: ListViewState::default(),
            staged_view: ListViewState::default(),
            commit_box: CommitBoxState::default(),
            branches_view: ListViewState::default(),
            commits_view: ListViewState::default(),
            stash_view: ListViewState::default(),
            reflog_view: ListViewState::default(),
            worktree_view: ListViewState::default(),
            operation_progress: OperationProgress::Idle,
            comparison_base: None,
            comparison_target: None,
            comparison_source: None,
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
    Compare,
    Rebase,
    Stash,
    Reflog,
    Worktrees,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitBoxState {
    pub focused: bool,
    pub mode: CommitBoxMode,
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
    pub rebase_state: Option<RebaseState>,
    pub stashes: Vec<StashItem>,
    pub reflog_items: Vec<ReflogItem>,
    pub worktrees: Vec<WorktreeItem>,
    pub commit_input: String,
    pub merge_state: MergeState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebaseKind {
    #[default]
    Interactive,
    Apply,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: PathBuf,
    pub kind: FileStatusKind,
    pub staged_kind: Option<FileStatusKind>,
    pub unstaged_kind: Option<FileStatusKind>,
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
    pub is_head: bool,
    pub upstream: Option<String>,
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
    pub stash_ref: String,
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
    CherryPickInProgress,
    RevertInProgress,
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

fn normalize_search_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn fuzzy_matches(haystack: &str, needle: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        workspace_attention_score, ListViewState, OperationProgress, RemoteSummary, RepoId,
        RepoModeState, RepoSubview, RepoSummary, Timestamp, WorkspaceFilterMode, WorkspaceSortMode,
        WorkspaceState,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

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
        assert_eq!(state.diff_line_cursor, None);
        assert_eq!(state.diff_line_anchor, None);
        assert_eq!(state.operation_progress, OperationProgress::Idle);
        assert!(state.detail.is_none());
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
