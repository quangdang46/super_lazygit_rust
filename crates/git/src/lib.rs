use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super_lazygit_core::{
    BranchItem, CommitFileItem, CommitHistoryMode, CommitItem, ComparisonTarget, Diagnostics,
    DiagnosticsSnapshot, DiffHunk, DiffLine, DiffLineKind, DiffModel, DiffPresentation, FileStatus,
    FileStatusKind, GitCommand, GitCommandRequest, HeadKind, MergeState, PatchApplicationMode,
    RebaseKind, RebaseStartMode, RebaseState, ReflogItem, RemoteBranchItem, RemoteItem,
    RemoteSummary, RepoDetail, RepoId, RepoSummary, ResetMode, SelectedHunk, StashItem, StashMode,
    SubmoduleItem, TagItem, Timestamp, WatcherFreshness, WorktreeItem,
};
use thiserror::Error;

pub trait GitBackend: Send + Sync + 'static {
    fn kind(&self) -> GitBackendKind;

    fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult>;

    fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary>;

    fn read_repo_detail(&self, request: RepoDetailRequest) -> GitResult<RepoDetail>;

    fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel>;

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome>;

    fn apply_patch_selection(&self, request: PatchSelectionRequest)
        -> GitResult<GitCommandOutcome>;
}

#[derive(Clone)]
pub struct GitFacade {
    backend: Arc<dyn GitBackend>,
    routing: GitBackendRoutingPolicy,
    diagnostics: Diagnostics,
}

impl Default for GitFacade {
    fn default() -> Self {
        Self::new(CliGitBackend)
    }
}

impl GitFacade {
    #[must_use]
    pub fn new(backend: impl GitBackend) -> Self {
        Self {
            backend: Arc::new(backend),
            routing: GitBackendRoutingPolicy::default(),
            diagnostics: Diagnostics::default(),
        }
    }

    #[must_use]
    pub fn with_routing(backend: impl GitBackend, routing: GitBackendRoutingPolicy) -> Self {
        Self {
            backend: Arc::new(backend),
            routing,
            diagnostics: Diagnostics::default(),
        }
    }

    #[must_use]
    pub fn backend_kind(&self) -> GitBackendKind {
        self.backend.kind()
    }

    #[must_use]
    pub fn routing(&self) -> &GitBackendRoutingPolicy {
        &self.routing
    }

    pub fn set_routing(&mut self, routing: GitBackendRoutingPolicy) {
        self.routing = routing;
    }

    #[must_use]
    pub fn route_for(&self, operation: GitOperationKind) -> GitBackendRoute {
        self.routing.route_for(self.backend.kind(), operation)
    }

    pub fn scan_workspace(
        &mut self,
        request: WorkspaceScanRequest,
    ) -> GitResult<WorkspaceScanResult> {
        let operation = GitOperationKind::ScanWorkspace;
        self.execute_routed(operation, |backend| backend.scan_workspace(request))
    }

    pub fn read_repo_summary(&mut self, request: RepoSummaryRequest) -> GitResult<RepoSummary> {
        let operation = GitOperationKind::ReadRepoSummary;
        self.execute_routed(operation, |backend| backend.read_repo_summary(request))
    }

    pub fn read_repo_detail(&mut self, request: RepoDetailRequest) -> GitResult<RepoDetail> {
        let operation = GitOperationKind::ReadRepoDetail;
        self.execute_routed(operation, |backend| backend.read_repo_detail(request))
    }

    pub fn read_diff(&mut self, request: DiffRequest) -> GitResult<DiffModel> {
        let operation = GitOperationKind::ReadDiff;
        self.execute_routed(operation, |backend| backend.read_diff(request))
    }

    pub fn run_command(&mut self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        let operation = GitOperationKind::WriteCommand;
        self.execute_routed(operation, |backend| backend.run_command(request))
    }

    pub fn apply_patch_selection(
        &mut self,
        request: PatchSelectionRequest,
    ) -> GitResult<GitCommandOutcome> {
        let operation = GitOperationKind::WriteCommand;
        self.execute_routed(operation, |backend| backend.apply_patch_selection(request))
    }

    pub fn record_operation(&mut self, operation: impl Into<String>, success: bool) {
        let started_at = Instant::now();
        self.diagnostics
            .record_git_operation(operation, started_at.elapsed(), success);
    }

    #[must_use]
    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }

    fn execute_routed<T>(
        &mut self,
        operation: GitOperationKind,
        execute: impl FnOnce(&dyn GitBackend) -> GitResult<T>,
    ) -> GitResult<T> {
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = if route.backend == self.backend.kind() {
            execute(self.backend.as_ref())
        } else {
            Err(GitError::RouteUnavailable {
                operation: operation.label(),
                backend: route.backend.label(),
            })
        };
        self.finish_operation(operation, route, started_at, &result);
        result
    }

    fn finish_operation<T>(
        &mut self,
        operation: GitOperationKind,
        route: GitBackendRoute,
        started_at: Instant,
        result: &GitResult<T>,
    ) {
        self.diagnostics.record_git_operation(
            format!("{} via {}", operation.label(), route.backend.label()),
            started_at.elapsed(),
            result.is_ok(),
        );
    }
}

impl fmt::Debug for GitFacade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitFacade")
            .field("backend", &self.backend.kind())
            .field("routing", &self.routing)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBackendKind {
    Gix,
    Git2,
    Cli,
    Noop,
}

impl GitBackendKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Gix => "gix",
            Self::Git2 => "git2",
            Self::Cli => "git-cli",
            Self::Noop => "noop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOperationKind {
    ScanWorkspace,
    ReadRepoSummary,
    ReadRepoDetail,
    ReadDiff,
    WriteCommand,
}

impl GitOperationKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ScanWorkspace => "scan_workspace",
            Self::ReadRepoSummary => "read_repo_summary",
            Self::ReadRepoDetail => "read_repo_detail",
            Self::ReadDiff => "read_diff",
            Self::WriteCommand => "write_command",
        }
    }

    #[must_use]
    pub fn capability(self) -> GitBackendCapability {
        match self {
            Self::ScanWorkspace => GitBackendCapability::WorkspaceScan,
            Self::ReadRepoSummary => GitBackendCapability::SummaryRead,
            Self::ReadRepoDetail => GitBackendCapability::DetailRead,
            Self::ReadDiff => GitBackendCapability::DiffRead,
            Self::WriteCommand => GitBackendCapability::Write,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBackendCapability {
    WorkspaceScan,
    SummaryRead,
    DetailRead,
    DiffRead,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendPreference {
    PrimaryOnly,
    PreferPrimary,
    PreferCli,
    CliOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitBackendRoute {
    pub operation: GitOperationKind,
    pub capability: GitBackendCapability,
    pub backend: GitBackendKind,
    pub preference: BackendPreference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBackendRoutingPolicy {
    pub primary_backend: GitBackendKind,
    pub summary_reads: BackendPreference,
    pub detail_reads: BackendPreference,
    pub diff_reads: BackendPreference,
    pub writes: BackendPreference,
    pub workspace_scans: BackendPreference,
}

impl Default for GitBackendRoutingPolicy {
    fn default() -> Self {
        Self {
            primary_backend: GitBackendKind::Cli,
            summary_reads: BackendPreference::PreferPrimary,
            detail_reads: BackendPreference::PreferPrimary,
            diff_reads: BackendPreference::PreferCli,
            writes: BackendPreference::CliOnly,
            workspace_scans: BackendPreference::PreferPrimary,
        }
    }
}

impl GitBackendRoutingPolicy {
    #[must_use]
    pub fn route_for(
        &self,
        active_backend: GitBackendKind,
        operation: GitOperationKind,
    ) -> GitBackendRoute {
        let preference = match operation {
            GitOperationKind::ScanWorkspace => self.workspace_scans,
            GitOperationKind::ReadRepoSummary => self.summary_reads,
            GitOperationKind::ReadRepoDetail => self.detail_reads,
            GitOperationKind::ReadDiff => self.diff_reads,
            GitOperationKind::WriteCommand => self.writes,
        };

        GitBackendRoute {
            operation,
            capability: operation.capability(),
            backend: select_backend(active_backend, self.primary_backend, preference),
            preference,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceScanRequest {
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceScanResult {
    pub root: Option<PathBuf>,
    pub repo_ids: Vec<RepoId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSummaryRequest {
    pub repo_id: RepoId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDetailRequest {
    pub repo_id: RepoId,
    pub selected_path: Option<PathBuf>,
    pub diff_presentation: DiffPresentation,
    pub commit_ref: Option<String>,
    pub commit_history_mode: CommitHistoryMode,
    pub ignore_whitespace_in_diff: bool,
    pub diff_context_lines: u16,
    pub rename_similarity_threshold: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub repo_id: RepoId,
    pub comparison_target: Option<ComparisonTarget>,
    pub compare_with: Option<ComparisonTarget>,
    pub selected_path: Option<PathBuf>,
    pub diff_presentation: DiffPresentation,
    pub ignore_whitespace_in_diff: bool,
    pub diff_context_lines: u16,
    pub rename_similarity_threshold: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchSelectionRequest {
    pub repo_id: RepoId,
    pub path: PathBuf,
    pub mode: PatchApplicationMode,
    pub hunks: Vec<SelectedHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandOutcome {
    pub repo_id: RepoId,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffReadOptions {
    presentation: DiffPresentation,
    ignore_whitespace: bool,
    context_lines: u16,
    rename_similarity_threshold: u8,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GitError {
    #[error("git backend route unavailable for {operation} via {backend}")]
    RouteUnavailable {
        operation: &'static str,
        backend: &'static str,
    },
    #[error("repository not found: {repo_id:?}")]
    RepoNotFound { repo_id: RepoId },
    #[error("git operation failed: {message}")]
    OperationFailed { message: String },
}

pub type GitResult<T> = Result<T, GitError>;

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopGitBackend;

impl GitBackend for NoopGitBackend {
    fn kind(&self) -> GitBackendKind {
        GitBackendKind::Noop
    }

    fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult> {
        Ok(WorkspaceScanResult {
            root: request.root,
            repo_ids: Vec::new(),
        })
    }

    fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary> {
        Err(GitError::RepoNotFound {
            repo_id: request.repo_id,
        })
    }

    fn read_repo_detail(&self, request: RepoDetailRequest) -> GitResult<RepoDetail> {
        Err(GitError::RepoNotFound {
            repo_id: request.repo_id,
        })
    }

    fn read_diff(&self, _request: DiffRequest) -> GitResult<DiffModel> {
        Err(GitError::RouteUnavailable {
            operation: GitOperationKind::ReadDiff.label(),
            backend: self.kind().label(),
        })
    }

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        Err(GitError::OperationFailed {
            message: format!(
                "{} is not executable through the noop backend",
                git_command_label(&request)
            ),
        })
    }

    fn apply_patch_selection(
        &self,
        request: PatchSelectionRequest,
    ) -> GitResult<GitCommandOutcome> {
        Err(GitError::OperationFailed {
            message: format!(
                "{:?} patch selection for {} is not executable through the noop backend",
                request.mode,
                request.path.display()
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CliGitBackend;

impl GitBackend for CliGitBackend {
    fn kind(&self) -> GitBackendKind {
        GitBackendKind::Cli
    }

    fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult> {
        let Some(root) = request.root else {
            return Ok(WorkspaceScanResult::default());
        };

        let mut repos = Vec::new();
        collect_git_repos(&root, &mut repos)?;
        repos.sort();
        repos.dedup();

        Ok(WorkspaceScanResult {
            root: Some(root),
            repo_ids: repos
                .into_iter()
                .map(|path| RepoId::new(path_string(&path)))
                .collect(),
        })
    }

    fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary> {
        let repo_path = PathBuf::from(&request.repo_id.0);
        if !is_git_repo(&repo_path) {
            return Err(GitError::RepoNotFound {
                repo_id: request.repo_id,
            });
        }

        let parsed = read_status_snapshot(&repo_path)?;
        let last_fetch_at = fetch_head_timestamp(&repo_path)?;

        let display_name = repo_path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| request.repo_id.0.clone(), ToOwned::to_owned);
        let now = unix_timestamp_now();

        Ok(RepoSummary {
            repo_id: request.repo_id,
            display_name,
            real_path: repo_path.clone(),
            display_path: path_string(&repo_path),
            branch: parsed.branch.clone(),
            head_kind: parsed.head_kind,
            dirty: parsed.staged_count > 0
                || parsed.unstaged_count > 0
                || parsed.untracked_count > 0
                || parsed.conflicted,
            staged_count: parsed.staged_count,
            unstaged_count: parsed.unstaged_count,
            untracked_count: parsed.untracked_count,
            ahead_count: parsed.ahead_count,
            behind_count: parsed.behind_count,
            conflicted: parsed.conflicted,
            last_fetch_at,
            last_local_activity_at: Some(now),
            last_refresh_at: Some(now),
            watcher_freshness: WatcherFreshness::Fresh,
            remote_summary: RemoteSummary {
                tracking_branch: parsed.tracking_branch.clone(),
                remote_name: parsed.remote_name.clone(),
            },
            last_error: None,
        })
    }

    fn read_repo_detail(&self, request: RepoDetailRequest) -> GitResult<RepoDetail> {
        let repo_path = repo_path(&request.repo_id)?;
        let status = read_status_snapshot(&repo_path)?;
        let diff = read_diff_model(
            &repo_path,
            None,
            None,
            request.selected_path.or(status.first_path.clone()),
            DiffReadOptions {
                presentation: request.diff_presentation,
                ignore_whitespace: request.ignore_whitespace_in_diff,
                context_lines: request.diff_context_lines,
                rename_similarity_threshold: request.rename_similarity_threshold,
            },
        )?;
        let commit_history = read_commits(
            &repo_path,
            request.commit_ref.as_deref(),
            request.commit_history_mode,
        );
        let remote_branches = read_remote_branches(&repo_path);
        let remotes = read_remotes(&repo_path, &remote_branches);
        let submodules = read_submodules(&repo_path);
        Ok(RepoDetail {
            file_tree: status.file_tree,
            diff,
            branches: read_branches(&repo_path),
            remotes,
            remote_branches,
            tags: read_tags(&repo_path),
            commits: commit_history.commits,
            commit_graph_lines: commit_history.graph_lines,
            rebase_state: read_rebase_state(&repo_path),
            stashes: read_stashes(&repo_path),
            reflog_items: read_reflog(&repo_path),
            worktrees: read_worktrees(&repo_path),
            submodules,
            commit_input: String::new(),
            merge_state: read_merge_state(&repo_path),
        })
    }

    fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel> {
        let repo_path = repo_path(&request.repo_id)?;
        let selected_path = match request.selected_path {
            Some(path) => Some(path),
            None => read_status_snapshot(&repo_path)?.first_path,
        };
        read_diff_model(
            &repo_path,
            request.comparison_target.as_ref(),
            request.compare_with.as_ref(),
            selected_path,
            DiffReadOptions {
                presentation: request.diff_presentation,
                ignore_whitespace: request.ignore_whitespace_in_diff,
                context_lines: request.diff_context_lines,
                rename_similarity_threshold: request.rename_similarity_threshold,
            },
        )
    }

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        let repo_path = repo_path(&request.repo_id)?;
        let summary = match &request.command {
            GitCommand::StageSelection => {
                git(&repo_path, ["add", "."])?;
                "Staged current selection".to_string()
            }
            GitCommand::UnstageSelection => {
                git(
                    &repo_path,
                    ["restore", "--staged", "--source=HEAD", "--", "."],
                )?;
                "Unstaged current selection".to_string()
            }
            GitCommand::StageFile { path } => {
                git_path(&repo_path, ["add"], path)?;
                format!("Staged {}", path.display())
            }
            GitCommand::DiscardFile { path } => {
                discard_path(&repo_path, path)?;
                format!("Discarded changes for {}", path.display())
            }
            GitCommand::UnstageFile { path } => {
                unstage_path(&repo_path, path)?;
                format!("Unstaged {}", path.display())
            }
            GitCommand::CommitStaged { message } => {
                git(&repo_path, ["commit", "-m", message.as_str()])?;
                format!("Committed staged changes: {message}")
            }
            GitCommand::CommitStagedNoVerify { message } => {
                git(
                    &repo_path,
                    ["commit", "--no-verify", "-m", message.as_str()],
                )?;
                format!("Committed staged changes without hooks: {message}")
            }
            GitCommand::CommitStagedWithEditor => {
                return Err(GitError::OperationFailed {
                    message: "interactive commit must run through the app runtime".to_string(),
                });
            }
            GitCommand::AmendHead { message } => {
                match message.as_deref() {
                    Some(message) => git(&repo_path, ["commit", "--amend", "-m", message])?,
                    None => git(&repo_path, ["commit", "--amend", "--no-edit"])?,
                }
                "Amended HEAD commit".to_string()
            }
            GitCommand::CreateFixupCommit { commit } => {
                git(&repo_path, ["commit", "--fixup", commit])?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Created fixup commit for {short} {subject}")
            }
            GitCommand::RewordCommitWithEditor { .. } => {
                return Err(GitError::OperationFailed {
                    message: "interactive reword must run through the app runtime".to_string(),
                });
            }
            GitCommand::StartCommitRebase { commit, mode } => {
                start_commit_rebase(&repo_path, commit, mode)?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                match mode {
                    RebaseStartMode::Interactive => {
                        format!("Started interactive rebase at {short} {subject}")
                    }
                    RebaseStartMode::Amend => {
                        format!("Started amend flow at {short} {subject}")
                    }
                    RebaseStartMode::Fixup => {
                        format!("Started fixup autosquash for {short} {subject}")
                    }
                    RebaseStartMode::ApplyFixups => {
                        format!("Applied fixup autosquash for {short} {subject}")
                    }
                    RebaseStartMode::Squash => {
                        format!("Squashed {short} {subject} into its parent")
                    }
                    RebaseStartMode::Drop => {
                        format!("Dropped {short} {subject} from history")
                    }
                    RebaseStartMode::MoveUp { .. } => {
                        format!("Moved {short} {subject} up in history")
                    }
                    RebaseStartMode::MoveDown { .. } => {
                        format!("Moved {short} {subject} down in history")
                    }
                    RebaseStartMode::Reword { .. } => {
                        format!("Reworded {short} {subject}")
                    }
                }
            }
            GitCommand::CherryPickCommit { commit } => {
                cherry_pick_commit(&repo_path, commit)?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Cherry-picked {short} {subject}")
            }
            GitCommand::RevertCommit { commit } => {
                revert_commit(&repo_path, commit)?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Reverted {short} {subject}")
            }
            GitCommand::ResetToCommit { mode, target } => {
                reset_to_commit(&repo_path, *mode, target)?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", target.as_str()])
                    .unwrap_or_else(|_| target.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", target])
                    .unwrap_or_else(|_| target.clone());
                format!("{} reset to {} {}", mode.title(), short, subject)
            }
            GitCommand::RestoreSnapshot { target } => {
                reset_to_commit(&repo_path, ResetMode::Hard, target)?;
                format!("Restored HEAD to {target}")
            }
            GitCommand::ContinueRebase => {
                git_with_env(
                    &repo_path,
                    ["rebase", "--continue"],
                    &[("GIT_EDITOR", OsStr::new(":"))],
                )?;
                "Continued rebase".to_string()
            }
            GitCommand::AbortRebase => {
                git(&repo_path, ["rebase", "--abort"])?;
                "Aborted rebase".to_string()
            }
            GitCommand::SkipRebase => {
                git_with_env(
                    &repo_path,
                    ["rebase", "--skip"],
                    &[("GIT_EDITOR", OsStr::new(":"))],
                )?;
                "Skipped current rebase step".to_string()
            }
            GitCommand::CreateBranch { branch_name } => {
                git(&repo_path, ["checkout", "-b", branch_name.as_str()])?;
                format!("Created and checked out {branch_name}")
            }
            GitCommand::AddRemote {
                remote_name,
                remote_url,
            } => {
                git(
                    &repo_path,
                    ["remote", "add", remote_name.as_str(), remote_url.as_str()],
                )?;
                format!("Added remote {remote_name}")
            }
            GitCommand::CreateTag { tag_name } => {
                git(&repo_path, ["tag", tag_name.as_str()])?;
                format!("Created tag {tag_name}")
            }
            GitCommand::CreateTagFromCommit { tag_name, commit } => {
                git(&repo_path, ["tag", tag_name.as_str(), commit.as_str()])?;
                format!("Created tag {tag_name} at {commit}")
            }
            GitCommand::CreateBranchFromCommit {
                branch_name,
                commit,
            } => {
                git(
                    &repo_path,
                    ["checkout", "-b", branch_name.as_str(), commit.as_str()],
                )?;
                format!("Created and checked out {branch_name} from {commit}")
            }
            GitCommand::CreateBranchFromRef {
                branch_name,
                start_point,
            } => {
                git(
                    &repo_path,
                    ["checkout", "-b", branch_name.as_str(), start_point.as_str()],
                )?;
                format!("Created and checked out {branch_name} from {start_point}")
            }
            GitCommand::CreateBranchFromStash {
                stash_ref,
                branch_name,
            } => {
                git(
                    &repo_path,
                    ["stash", "branch", branch_name.as_str(), stash_ref.as_str()],
                )?;
                format!("Created and checked out {branch_name} from {stash_ref}")
            }
            GitCommand::CheckoutBranch { branch_ref } => {
                git(&repo_path, ["checkout", branch_ref.as_str()])?;
                format!("Checked out {branch_ref}")
            }
            GitCommand::CheckoutRemoteBranch {
                remote_branch_ref,
                local_branch_name,
            } => {
                if local_branch_exists(&repo_path, local_branch_name)? {
                    git(&repo_path, ["checkout", local_branch_name.as_str()])?;
                    format!("Checked out {local_branch_name}")
                } else {
                    git(
                        &repo_path,
                        [
                            "checkout",
                            "--track",
                            "-b",
                            local_branch_name.as_str(),
                            remote_branch_ref.as_str(),
                        ],
                    )?;
                    format!(
                        "Created and checked out {local_branch_name} tracking {remote_branch_ref}"
                    )
                }
            }
            GitCommand::CheckoutTag { tag_name } => {
                git(&repo_path, ["checkout", tag_name.as_str()])?;
                format!("Checked out tag {tag_name}")
            }
            GitCommand::CheckoutCommit { commit } => {
                git(&repo_path, ["checkout", commit.as_str()])?;
                format!("Checked out commit {commit}")
            }
            GitCommand::CheckoutCommitFile { commit, path } => {
                let path_value = path.to_string_lossy().into_owned();
                git(
                    &repo_path,
                    ["checkout", commit.as_str(), "--", path_value.as_str()],
                )?;
                format!("Checked out {} from {commit}", path.display())
            }
            GitCommand::RenameBranch {
                branch_name,
                new_name,
            } => {
                if current_branch_name(&repo_path)? == *branch_name {
                    git(&repo_path, ["branch", "-m", new_name.as_str()])?;
                } else {
                    git(
                        &repo_path,
                        ["branch", "-m", branch_name.as_str(), new_name.as_str()],
                    )?;
                }
                format!("Renamed {branch_name} to {new_name}")
            }
            GitCommand::EditRemote {
                current_name,
                new_name,
                remote_url,
            } => {
                let target_name = if current_name != new_name {
                    git(
                        &repo_path,
                        ["remote", "rename", current_name.as_str(), new_name.as_str()],
                    )?;
                    new_name.as_str()
                } else {
                    current_name.as_str()
                };
                git(
                    &repo_path,
                    ["remote", "set-url", target_name, remote_url.as_str()],
                )?;
                format!("Updated remote {current_name}")
            }
            GitCommand::RenameStash { stash_ref, message } => {
                rename_stash(&repo_path, stash_ref, message)?;
                format!("Renamed {stash_ref}")
            }
            GitCommand::DeleteBranch { branch_name } => {
                git(&repo_path, ["branch", "-D", branch_name.as_str()])?;
                format!("Deleted {branch_name}")
            }
            GitCommand::RemoveRemote { remote_name } => {
                git(&repo_path, ["remote", "remove", remote_name.as_str()])?;
                format!("Removed remote {remote_name}")
            }
            GitCommand::DeleteRemoteBranch {
                remote_name,
                branch_name,
            } => {
                git(
                    &repo_path,
                    [
                        "push",
                        remote_name.as_str(),
                        "--delete",
                        branch_name.as_str(),
                    ],
                )?;
                format!("Deleted remote branch {remote_name}/{branch_name}")
            }
            GitCommand::DeleteTag { tag_name } => {
                git(&repo_path, ["tag", "-d", tag_name.as_str()])?;
                format!("Deleted tag {tag_name}")
            }
            GitCommand::PushTag {
                remote_name,
                tag_name,
            } => {
                let refspec = format!("refs/tags/{tag_name}");
                git(&repo_path, ["push", remote_name.as_str(), refspec.as_str()])?;
                format!("Pushed tag {tag_name} to {remote_name}")
            }
            GitCommand::CreateStash { message, mode } => {
                match mode {
                    StashMode::Tracked => stash_push(&repo_path, &[], message.as_deref())?,
                    StashMode::KeepIndex => {
                        stash_push(&repo_path, &["--keep-index"], message.as_deref())?
                    }
                    StashMode::IncludeUntracked => {
                        stash_push(&repo_path, &["--include-untracked"], message.as_deref())?
                    }
                    StashMode::Staged => stash_push(&repo_path, &["--staged"], message.as_deref())?,
                    StashMode::Unstaged => stash_unstaged_changes(&repo_path, message.as_deref())?,
                };
                match (mode, message) {
                    (StashMode::Tracked, Some(message)) => {
                        format!("Stashed tracked changes: {message}")
                    }
                    (StashMode::Tracked, None) => "Stashed tracked changes".to_string(),
                    (StashMode::KeepIndex, Some(message)) => {
                        format!("Stashed tracked changes and kept staged changes: {message}")
                    }
                    (StashMode::KeepIndex, None) => {
                        "Stashed tracked changes and kept staged changes".to_string()
                    }
                    (StashMode::IncludeUntracked, Some(message)) => {
                        format!("Stashed all changes including untracked: {message}")
                    }
                    (StashMode::IncludeUntracked, None) => {
                        "Stashed all changes including untracked".to_string()
                    }
                    (StashMode::Staged, Some(message)) => {
                        format!("Stashed staged changes: {message}")
                    }
                    (StashMode::Staged, None) => "Stashed staged changes".to_string(),
                    (StashMode::Unstaged, Some(message)) => {
                        format!("Stashed unstaged changes: {message}")
                    }
                    (StashMode::Unstaged, None) => "Stashed unstaged changes".to_string(),
                }
            }
            GitCommand::ApplyStash { stash_ref } => {
                git(&repo_path, ["stash", "apply", stash_ref.as_str()])?;
                format!("Applied {stash_ref}")
            }
            GitCommand::PopStash { stash_ref } => {
                git(&repo_path, ["stash", "pop", stash_ref.as_str()])?;
                format!("Popped {stash_ref}")
            }
            GitCommand::DropStash { stash_ref } => {
                git(&repo_path, ["stash", "drop", stash_ref.as_str()])?;
                format!("Dropped {stash_ref}")
            }
            GitCommand::CreateWorktree { path, branch_ref } => {
                let output = Command::new("git")
                    .arg("worktree")
                    .arg("add")
                    .arg(path)
                    .arg(branch_ref)
                    .current_dir(&repo_path)
                    .output()
                    .map_err(io_error)?;
                if !output.status.success() {
                    return Err(command_failure(output));
                }
                format!("Created worktree {} from {branch_ref}", path.display())
            }
            GitCommand::RemoveWorktree { path } => {
                let output = Command::new("git")
                    .arg("worktree")
                    .arg("remove")
                    .arg(path)
                    .current_dir(&repo_path)
                    .output()
                    .map_err(io_error)?;
                if !output.status.success() {
                    return Err(command_failure(output));
                }
                format!("Removed worktree {}", path.display())
            }
            GitCommand::AddSubmodule { path, url } => {
                let path_value = path.to_string_lossy().into_owned();
                git_with_env(
                    &repo_path,
                    [
                        "-c",
                        "protocol.file.allow=always",
                        "submodule",
                        "add",
                        url.as_str(),
                        path_value.as_str(),
                    ],
                    &[],
                )?;
                format!("Added submodule {} from {url}", path.display())
            }
            GitCommand::EditSubmoduleUrl { name, path, url } => {
                edit_submodule_url(&repo_path, name, path, url)?;
                format!("Updated submodule {} URL", path.display())
            }
            GitCommand::InitSubmodule { path } => {
                let path_value = path.to_string_lossy().into_owned();
                git(
                    &repo_path,
                    [
                        "-c",
                        "protocol.file.allow=always",
                        "submodule",
                        "update",
                        "--init",
                        "--",
                        path_value.as_str(),
                    ],
                )?;
                format!("Initialized submodule {}", path.display())
            }
            GitCommand::UpdateSubmodule { path } => {
                let path_value = path.to_string_lossy().into_owned();
                git(
                    &repo_path,
                    [
                        "-c",
                        "protocol.file.allow=always",
                        "submodule",
                        "update",
                        "--remote",
                        "--",
                        path_value.as_str(),
                    ],
                )?;
                format!("Updated submodule {}", path.display())
            }
            GitCommand::RemoveSubmodule { path } => {
                remove_submodule(&repo_path, path)?;
                format!("Removed submodule {}", path.display())
            }
            GitCommand::SetBranchUpstream {
                branch_name,
                upstream_ref,
            } => {
                let upstream_arg = format!("--set-upstream-to={upstream_ref}");
                git(
                    &repo_path,
                    ["branch", upstream_arg.as_str(), branch_name.as_str()],
                )?;
                format!("Set upstream for {branch_name} to {upstream_ref}")
            }
            GitCommand::FetchRemote { remote_name } => {
                git(&repo_path, ["fetch", remote_name.as_str()])?;
                format!("Fetched {remote_name}")
            }
            GitCommand::FetchSelectedRepo => {
                run_fetch(&repo_path)?;
                "Fetched remote updates".to_string()
            }
            GitCommand::PullCurrentBranch => {
                run_pull(&repo_path)?;
                "Pulled current branch".to_string()
            }
            GitCommand::PushCurrentBranch => {
                run_push(&repo_path)?;
                "Pushed current branch".to_string()
            }
            GitCommand::NukeWorkingTree => {
                nuke_working_tree(&repo_path)?;
                "Discarded all local changes".to_string()
            }
            GitCommand::RefreshSelectedRepo => {
                git(&repo_path, ["status", "--short"])?;
                "Refreshed selected repository".to_string()
            }
        };

        Ok(GitCommandOutcome {
            repo_id: request.repo_id,
            summary,
        })
    }

    fn apply_patch_selection(
        &self,
        request: PatchSelectionRequest,
    ) -> GitResult<GitCommandOutcome> {
        let repo_path = repo_path(&request.repo_id)?;
        apply_patch_selection(&repo_path, &request)?;

        let summary = match request.mode {
            PatchApplicationMode::Stage => {
                format!(
                    "Staged {} selected hunk(s) for {}",
                    request.hunks.len(),
                    request.path.display()
                )
            }
            PatchApplicationMode::Unstage => {
                format!(
                    "Unstaged {} selected hunk(s) for {}",
                    request.hunks.len(),
                    request.path.display()
                )
            }
        };

        Ok(GitCommandOutcome {
            repo_id: request.repo_id,
            summary,
        })
    }
}

fn select_backend(
    active_backend: GitBackendKind,
    primary_backend: GitBackendKind,
    preference: BackendPreference,
) -> GitBackendKind {
    match preference {
        BackendPreference::PrimaryOnly => primary_backend,
        BackendPreference::PreferPrimary => prefer_backend(active_backend, primary_backend),
        BackendPreference::PreferCli => prefer_backend(active_backend, GitBackendKind::Cli),
        BackendPreference::CliOnly => GitBackendKind::Cli,
    }
}

fn prefer_backend(active_backend: GitBackendKind, preferred: GitBackendKind) -> GitBackendKind {
    if active_backend == preferred {
        active_backend
    } else {
        preferred
    }
}

fn git_command_label(request: &GitCommandRequest) -> &'static str {
    match &request.command {
        GitCommand::StageSelection => "stage_selection",
        GitCommand::UnstageSelection => "unstage_selection",
        GitCommand::StageFile { .. } => "stage_file",
        GitCommand::DiscardFile { .. } => "discard_file",
        GitCommand::UnstageFile { .. } => "unstage_file",
        GitCommand::CommitStaged { .. } => "commit_staged",
        GitCommand::CommitStagedNoVerify { .. } => "commit_staged_no_verify",
        GitCommand::CommitStagedWithEditor => "commit_staged_with_editor",
        GitCommand::AmendHead { .. } => "amend_head",
        GitCommand::CreateFixupCommit { .. } => "create_fixup_commit",
        GitCommand::RewordCommitWithEditor { .. } => "reword_commit_with_editor",
        GitCommand::StartCommitRebase { mode, .. } => match mode {
            RebaseStartMode::Interactive => "start_interactive_rebase",
            RebaseStartMode::Amend => "start_amend_rebase",
            RebaseStartMode::Fixup => "start_fixup_rebase",
            RebaseStartMode::ApplyFixups => "apply_fixups_rebase",
            RebaseStartMode::Squash => "start_squash_rebase",
            RebaseStartMode::Drop => "start_drop_rebase",
            RebaseStartMode::MoveUp { .. } => "move_commit_up_rebase",
            RebaseStartMode::MoveDown { .. } => "move_commit_down_rebase",
            RebaseStartMode::Reword { .. } => "start_reword_rebase",
        },
        GitCommand::CherryPickCommit { .. } => "cherry_pick_commit",
        GitCommand::RevertCommit { .. } => "revert_commit",
        GitCommand::ResetToCommit { mode, .. } => match mode {
            ResetMode::Soft => "reset_to_commit_soft",
            ResetMode::Mixed => "reset_to_commit_mixed",
            ResetMode::Hard => "reset_to_commit_hard",
        },
        GitCommand::RestoreSnapshot { .. } => "restore_snapshot",
        GitCommand::ContinueRebase => "continue_rebase",
        GitCommand::AbortRebase => "abort_rebase",
        GitCommand::SkipRebase => "skip_rebase",
        GitCommand::CreateBranch { .. } => "create_branch",
        GitCommand::AddRemote { .. } => "add_remote",
        GitCommand::CreateTag { .. } => "create_tag",
        GitCommand::CreateTagFromCommit { .. } => "create_tag_from_commit",
        GitCommand::CreateBranchFromCommit { .. } => "create_branch_from_commit",
        GitCommand::CreateBranchFromRef { .. } => "create_branch_from_ref",
        GitCommand::CreateBranchFromStash { .. } => "create_branch_from_stash",
        GitCommand::CheckoutBranch { .. } => "checkout_branch",
        GitCommand::CheckoutRemoteBranch { .. } => "checkout_remote_branch",
        GitCommand::CheckoutTag { .. } => "checkout_tag",
        GitCommand::CheckoutCommit { .. } => "checkout_commit",
        GitCommand::CheckoutCommitFile { .. } => "checkout_commit_file",
        GitCommand::RenameBranch { .. } => "rename_branch",
        GitCommand::EditRemote { .. } => "edit_remote",
        GitCommand::RenameStash { .. } => "rename_stash",
        GitCommand::DeleteBranch { .. } => "delete_branch",
        GitCommand::RemoveRemote { .. } => "remove_remote",
        GitCommand::DeleteRemoteBranch { .. } => "delete_remote_branch",
        GitCommand::DeleteTag { .. } => "delete_tag",
        GitCommand::PushTag { .. } => "push_tag",
        GitCommand::CreateStash {
            mode: StashMode::Tracked,
            ..
        } => "create_stash",
        GitCommand::CreateStash {
            mode: StashMode::KeepIndex,
            ..
        } => "create_stash_keep_index",
        GitCommand::CreateStash {
            mode: StashMode::IncludeUntracked,
            ..
        } => "create_stash_including_untracked",
        GitCommand::CreateStash {
            mode: StashMode::Staged,
            ..
        } => "create_stash_staged",
        GitCommand::CreateStash {
            mode: StashMode::Unstaged,
            ..
        } => "create_stash_unstaged",
        GitCommand::ApplyStash { .. } => "apply_stash",
        GitCommand::PopStash { .. } => "pop_stash",
        GitCommand::DropStash { .. } => "drop_stash",
        GitCommand::CreateWorktree { .. } => "create_worktree",
        GitCommand::RemoveWorktree { .. } => "remove_worktree",
        GitCommand::AddSubmodule { .. } => "add_submodule",
        GitCommand::EditSubmoduleUrl { .. } => "edit_submodule_url",
        GitCommand::InitSubmodule { .. } => "init_submodule",
        GitCommand::UpdateSubmodule { .. } => "update_submodule",
        GitCommand::RemoveSubmodule { .. } => "remove_submodule",
        GitCommand::SetBranchUpstream { .. } => "set_branch_upstream",
        GitCommand::FetchRemote { .. } => "fetch_remote",
        GitCommand::FetchSelectedRepo => "fetch_selected_repo",
        GitCommand::PullCurrentBranch => "pull_current_branch",
        GitCommand::PushCurrentBranch => "push_current_branch",
        GitCommand::NukeWorkingTree => "nuke_working_tree",
        GitCommand::RefreshSelectedRepo => "refresh_selected_repo",
    }
}

fn collect_git_repos(root: &Path, repos: &mut Vec<PathBuf>) -> GitResult<()> {
    let mut visited = HashSet::new();
    collect_git_repos_inner(root, repos, &mut visited);
    Ok(())
}

fn collect_git_repos_inner(root: &Path, repos: &mut Vec<PathBuf>, visited: &mut HashSet<PathBuf>) {
    let canonical_root = canonicalize_existing_path(root);
    if !visited.insert(canonical_root) {
        return;
    }

    if let Some(repo_root) = resolve_git_repo_root(root) {
        repos.push(repo_root);
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }

        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = if file_type.is_dir() {
            true
        } else if file_type.is_symlink() {
            fs::metadata(&path).is_ok_and(|metadata| metadata.is_dir())
        } else {
            false
        };
        if !is_dir {
            continue;
        }

        collect_git_repos_inner(&path, repos, visited);
    }
}

fn resolve_git_repo_root(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }

    let git_path = path.join(".git");
    let metadata = fs::metadata(&git_path).ok()?;
    if metadata.is_dir() {
        return Some(canonicalize_existing_path(path));
    }
    if !metadata.is_file() {
        return None;
    }

    let gitdir = parse_gitdir_file(&git_path)?;
    if gitdir.exists() {
        Some(canonicalize_existing_path(path))
    } else {
        None
    }
}

fn parse_gitdir_file(git_path: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(git_path).ok()?;
    let target = contents.trim().strip_prefix("gitdir:")?.trim();
    let target_path = PathBuf::from(target);
    Some(if target_path.is_absolute() {
        target_path
    } else {
        git_path.parent()?.join(target_path)
    })
}

fn canonicalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_git_repo(path: &Path) -> bool {
    git_stdout(path, ["rev-parse", "--show-toplevel"]).is_ok()
}

fn repo_path(repo_id: &RepoId) -> GitResult<PathBuf> {
    let repo_path = PathBuf::from(&repo_id.0);
    if !is_git_repo(&repo_path) {
        return Err(GitError::RepoNotFound {
            repo_id: repo_id.clone(),
        });
    }
    Ok(repo_path)
}

fn run_fetch(repo_path: &Path) -> GitResult<()> {
    if let Some(remote) = default_remote(repo_path)? {
        git(repo_path, ["fetch", remote.as_str()])
    } else {
        git(repo_path, ["fetch", "--all"])
    }
}

fn run_pull(repo_path: &Path) -> GitResult<()> {
    if has_upstream(repo_path)? {
        git(repo_path, ["pull", "--ff-only"])
    } else {
        Err(GitError::OperationFailed {
            message: "pull requires an upstream tracking branch".to_string(),
        })
    }
}

fn run_push(repo_path: &Path) -> GitResult<()> {
    if has_upstream(repo_path)? {
        git(repo_path, ["push"])
    } else {
        let branch = current_branch_name(repo_path)?;
        let remote = default_remote(repo_path)?.unwrap_or_else(|| "origin".to_string());
        git(
            repo_path,
            ["push", "--set-upstream", remote.as_str(), branch.as_str()],
        )
    }
}

fn default_remote(repo_path: &Path) -> GitResult<Option<String>> {
    let remote = git_stdout_allow_failure(repo_path, ["remote"])?;
    Ok(remote
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}

fn has_upstream(repo_path: &Path) -> GitResult<bool> {
    Ok(!git_stdout_allow_failure(
        repo_path,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )?
    .is_empty())
}

fn current_branch_name(repo_path: &Path) -> GitResult<String> {
    let branch = git_stdout(repo_path, ["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err(GitError::OperationFailed {
            message: "push requires an attached branch HEAD".to_string(),
        });
    }
    Ok(branch)
}

fn fetch_head_timestamp(repo_path: &Path) -> GitResult<Option<Timestamp>> {
    let git_dir = git_stdout(repo_path, ["rev-parse", "--git-dir"])?;
    let fetch_head = repo_path.join(git_dir).join("FETCH_HEAD");
    match fs::metadata(fetch_head) {
        Ok(metadata) => Ok(metadata.modified().ok().map(system_time_to_timestamp)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    }
}

fn apply_patch_selection(repo_path: &Path, request: &PatchSelectionRequest) -> GitResult<()> {
    if request.hunks.is_empty() {
        return Err(GitError::OperationFailed {
            message: format!(
                "{:?} patch selection for {} requires at least one hunk",
                request.mode,
                request.path.display()
            ),
        });
    }

    let diff = match request.mode {
        PatchApplicationMode::Stage => git_stdout_raw(
            repo_path,
            vec![
                OsStr::new("diff"),
                OsStr::new("--no-ext-diff"),
                OsStr::new("--binary"),
                OsStr::new("--unified=0"),
                OsStr::new("--"),
                request.path.as_os_str(),
            ],
        )?,
        PatchApplicationMode::Unstage => git_stdout_raw(
            repo_path,
            vec![
                OsStr::new("diff"),
                OsStr::new("--cached"),
                OsStr::new("--no-ext-diff"),
                OsStr::new("--binary"),
                OsStr::new("--unified=0"),
                OsStr::new("--"),
                request.path.as_os_str(),
            ],
        )?,
    };
    if diff.trim().is_empty() {
        return Err(GitError::OperationFailed {
            message: format!(
                "no {:?} patch data available for {}",
                request.mode,
                request.path.display()
            ),
        });
    }

    let selected_patch = build_selected_patch(&diff, &request.hunks)?;
    let mut args = vec![
        OsStr::new("apply"),
        OsStr::new("--cached"),
        OsStr::new("--unidiff-zero"),
        OsStr::new("--whitespace=nowarn"),
    ];
    if matches!(request.mode, PatchApplicationMode::Unstage) {
        args.push(OsStr::new("--reverse"));
    }

    git_with_stdin(repo_path, args, selected_patch.as_bytes())
}

fn build_selected_patch(diff: &str, selections: &[SelectedHunk]) -> GitResult<String> {
    let parsed = parse_patch(diff)?;
    let mut selected_hunks = Vec::new();

    for selection in selections {
        if let Some(hunk) = parsed
            .hunks
            .iter()
            .find(|hunk| hunk.selection == *selection)
        {
            selected_hunks.push(hunk.raw.clone());
            continue;
        }

        let Some(hunk) = parsed
            .hunks
            .iter()
            .find(|hunk| selection_within_hunk(*selection, hunk.selection))
        else {
            return Err(GitError::OperationFailed {
                message: format!(
                    "requested hunk -{},{} +{},{} was not found in patch",
                    selection.old_start,
                    selection.old_lines,
                    selection.new_start,
                    selection.new_lines
                ),
            });
        };
        selected_hunks.push(build_partial_hunk(hunk, *selection)?);
    }

    let mut patch = String::new();
    for line in &parsed.header_lines {
        patch.push_str(line);
        patch.push('\n');
    }
    for hunk in selected_hunks {
        patch.push_str(&hunk);
    }
    Ok(patch)
}

fn selection_within_hunk(selection: SelectedHunk, hunk: SelectedHunk) -> bool {
    let selection_old_end = selection.old_start.saturating_add(selection.old_lines);
    let selection_new_end = selection.new_start.saturating_add(selection.new_lines);
    let hunk_old_end = hunk.old_start.saturating_add(hunk.old_lines);
    let hunk_new_end = hunk.new_start.saturating_add(hunk.new_lines);

    selection.old_start >= hunk.old_start.saturating_sub(1)
        && selection.new_start >= hunk.new_start.saturating_sub(1)
        && selection_old_end <= hunk_old_end
        && selection_new_end <= hunk_new_end
}

fn build_partial_hunk(hunk: &ParsedHunk, selection: SelectedHunk) -> GitResult<String> {
    let mut raw_lines = hunk.raw.lines();
    let header_line = raw_lines.next().ok_or_else(|| GitError::OperationFailed {
        message: "patch hunk was empty".to_string(),
    })?;
    let header_suffix = hunk_header_suffix(header_line);
    let mut body = String::new();
    let mut old_cursor = hunk.selection.old_start;
    let mut new_cursor = hunk.selection.new_start;
    let old_end = selection.old_start.saturating_add(selection.old_lines);
    let new_end = selection.new_start.saturating_add(selection.new_lines);
    let mut previous_change_selected = false;

    for line in raw_lines {
        let include = match line.chars().next() {
            Some('-') => {
                let include = old_cursor >= selection.old_start && old_cursor < old_end;
                old_cursor = old_cursor.saturating_add(1);
                include
            }
            Some('+') => {
                let include = new_cursor >= selection.new_start && new_cursor < new_end;
                new_cursor = new_cursor.saturating_add(1);
                include
            }
            Some('\\') => previous_change_selected,
            _ => {
                return Err(GitError::OperationFailed {
                    message: format!(
                        "unexpected context in zero-context hunk while selecting partial patch: {line}"
                    ),
                });
            }
        };

        if include {
            body.push_str(line);
            body.push('\n');
        }
        previous_change_selected = matches!(line.chars().next(), Some('-' | '+')) && include;
    }

    if body.is_empty() {
        return Err(GitError::OperationFailed {
            message: format!(
                "requested hunk -{},{} +{},{} did not match any lines in patch",
                selection.old_start, selection.old_lines, selection.new_start, selection.new_lines
            ),
        });
    }

    let mut raw = format!(
        "@@ -{} +{} @@{}",
        format_patch_range(selection.old_start, selection.old_lines),
        format_patch_range(selection.new_start, selection.new_lines),
        header_suffix
    );
    raw.push('\n');
    raw.push_str(&body);
    Ok(raw)
}

fn format_patch_range(start: u32, lines: u32) -> String {
    if lines == 1 {
        start.to_string()
    } else {
        format!("{start},{lines}")
    }
}

fn hunk_header_suffix(header_line: &str) -> &str {
    header_line
        .strip_prefix("@@")
        .and_then(|rest| rest.split_once("@@"))
        .map(|(_, suffix)| suffix)
        .unwrap_or("")
}

fn parse_patch(diff: &str) -> GitResult<ParsedPatch> {
    let lines = diff.lines();
    let mut header_lines = Vec::new();
    let mut hunks = Vec::new();
    let mut current_hunk: Option<(SelectedHunk, String)> = None;

    for line in lines {
        if let Some(selection) = parse_hunk_header(line)? {
            if let Some((selection, raw)) = current_hunk.take() {
                hunks.push(ParsedHunk { selection, raw });
            }
            let mut raw = String::new();
            raw.push_str(line);
            raw.push('\n');
            current_hunk = Some((selection, raw));
            continue;
        }

        if let Some((_, raw)) = current_hunk.as_mut() {
            raw.push_str(line);
            raw.push('\n');
        } else {
            header_lines.push(line.to_string());
        }
    }

    if let Some((selection, raw)) = current_hunk.take() {
        hunks.push(ParsedHunk { selection, raw });
    }

    if hunks.is_empty() {
        return Err(GitError::OperationFailed {
            message: "patch did not contain any hunks".to_string(),
        });
    }

    Ok(ParsedPatch {
        header_lines,
        hunks,
    })
}

fn parse_hunk_header(line: &str) -> GitResult<Option<SelectedHunk>> {
    let Some(rest) = line.strip_prefix("@@ -") else {
        return Ok(None);
    };
    let Some((old_range, remainder)) = rest.split_once(" +") else {
        return Err(GitError::OperationFailed {
            message: format!("invalid patch hunk header: {line}"),
        });
    };
    let Some((new_range, _)) = remainder.split_once(" @@") else {
        return Err(GitError::OperationFailed {
            message: format!("invalid patch hunk header: {line}"),
        });
    };

    let (old_start, old_lines) = parse_patch_range(old_range)?;
    let (new_start, new_lines) = parse_patch_range(new_range)?;
    Ok(Some(SelectedHunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }))
}

fn parse_patch_range(range: &str) -> GitResult<(u32, u32)> {
    let (start, lines) = match range.split_once(',') {
        Some((start, lines)) => (start, lines),
        None => (range, "1"),
    };
    let start = start.parse().map_err(|error| GitError::OperationFailed {
        message: format!("invalid patch range start `{start}`: {error}"),
    })?;
    let lines = lines.parse().map_err(|error| GitError::OperationFailed {
        message: format!("invalid patch range length `{lines}`: {error}"),
    })?;
    Ok((start, lines))
}

fn read_diff_model(
    repo_path: &Path,
    comparison_target: Option<&ComparisonTarget>,
    compare_with: Option<&ComparisonTarget>,
    selected_path: Option<PathBuf>,
    options: DiffReadOptions,
) -> GitResult<DiffModel> {
    let diff_text = read_diff_text(
        repo_path,
        comparison_target,
        compare_with,
        selected_path.as_deref(),
        options,
    )?;
    Ok(parse_diff_model(
        selected_path,
        options.presentation,
        &diff_text,
    ))
}

fn read_diff_text(
    repo_path: &Path,
    comparison_target: Option<&ComparisonTarget>,
    compare_with: Option<&ComparisonTarget>,
    selected_path: Option<&Path>,
    options: DiffReadOptions,
) -> GitResult<String> {
    let mut args = vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--binary".to_string(),
        format!("--unified={}", options.context_lines),
        format!("--find-renames={}%", options.rename_similarity_threshold),
    ];

    if options.ignore_whitespace {
        args.push("--ignore-all-space".to_string());
    }

    if let Some(target) = comparison_target {
        args.push(match target {
            ComparisonTarget::Branch(branch) | ComparisonTarget::Commit(branch) => branch.clone(),
        });
        if let Some(compare_with) = compare_with {
            args.push(match compare_with {
                ComparisonTarget::Branch(branch) | ComparisonTarget::Commit(branch) => {
                    branch.clone()
                }
            });
        }
    } else if matches!(options.presentation, DiffPresentation::Staged) {
        args.push("--cached".to_string());
    }

    if let Some(path) = selected_path {
        args.push("--".to_string());
        args.push(path.display().to_string());
    }

    git_stdout(repo_path, args)
}

fn parse_diff_model(
    selected_path: Option<PathBuf>,
    presentation: DiffPresentation,
    diff: &str,
) -> DiffModel {
    let mut lines = Vec::new();
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;

    for (index, raw_line) in diff.lines().enumerate() {
        let kind = if raw_line.starts_with("@@") {
            if let Some(mut hunk) = current_hunk.take() {
                hunk.end_line_index = index;
                hunks.push(hunk);
            }
            current_hunk = Some(DiffHunk {
                header: raw_line.to_string(),
                selection: parse_hunk_header(raw_line)
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
                start_line_index: index,
                end_line_index: index + 1,
            });
            DiffLineKind::HunkHeader
        } else if raw_line.starts_with("diff --git")
            || raw_line.starts_with("index ")
            || raw_line.starts_with("--- ")
            || raw_line.starts_with("+++ ")
        {
            DiffLineKind::Meta
        } else if raw_line.starts_with('+') && !raw_line.starts_with("+++") {
            DiffLineKind::Addition
        } else if raw_line.starts_with('-') && !raw_line.starts_with("---") {
            DiffLineKind::Removal
        } else {
            DiffLineKind::Context
        };

        if let Some(hunk) = current_hunk.as_mut() {
            hunk.end_line_index = index + 1;
        }
        lines.push(DiffLine {
            kind,
            content: raw_line.to_string(),
        });
    }

    if let Some(mut hunk) = current_hunk.take() {
        hunk.end_line_index = lines.len();
        hunks.push(hunk);
    }

    DiffModel {
        selected_path,
        presentation,
        lines,
        selected_hunk: (!hunks.is_empty()).then_some(0),
        hunk_count: hunks.len(),
        hunks,
    }
}

fn git<I, S>(repo_path: &Path, args: I) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn git_with_stdin<I, S>(repo_path: &Path, args: I, stdin: &[u8]) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(io_error)?;

    let Some(mut child_stdin) = child.stdin.take() else {
        return Err(GitError::OperationFailed {
            message: "failed to open git stdin".to_string(),
        });
    };
    child_stdin.write_all(stdin).map_err(io_error)?;
    drop(child_stdin);

    let output = child.wait_with_output().map_err(io_error)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn git_stdout<I, S>(repo_path: &Path, args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    stdout_string(output)
}

fn git_stdout_allow_failure<I, S>(repo_path: &Path, args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Ok(String::new());
    }
    stdout_string(output)
}

fn git_output<I, S>(repo_path: &Path, args: I) -> GitResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git_output_with_env(repo_path, args, &[])
}

fn git_output_with_env<I, S>(
    repo_path: &Path,
    args: I,
    envs: &[(&str, &OsStr)],
) -> GitResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .args(args)
        .envs(envs.iter().copied())
        .current_dir(repo_path)
        .output()
        .map_err(io_error)
}

fn git_path<I, S>(repo_path: &Path, args: I, path: &Path) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .arg("--")
        .arg(path)
        .current_dir(repo_path)
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn git_path_output<I, S>(repo_path: &Path, args: I, path: &Path) -> GitResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .args(args)
        .arg("--")
        .arg(path)
        .current_dir(repo_path)
        .output()
        .map_err(io_error)
}

fn git_stdout_raw<I, S>(repo_path: &Path, args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    stdout_raw_string(output)
}

fn stdout_string(output: Output) -> GitResult<String> {
    stdout_raw_string(output).map(|value| value.trim().to_owned())
}

fn stdout_raw_string(output: Output) -> GitResult<String> {
    String::from_utf8(output.stdout).map_err(|error| GitError::OperationFailed {
        message: error.to_string(),
    })
}

fn stash_push(repo_path: &Path, extra_args: &[&str], message: Option<&str>) -> GitResult<()> {
    let mut command = Command::new("git");
    command.arg("stash").arg("push").current_dir(repo_path);
    command.args(extra_args);
    if let Some(message) = message {
        command.arg("-m").arg(message);
    }
    let output = command.output().map_err(io_error)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn stash_unstaged_changes(repo_path: &Path, message: Option<&str>) -> GitResult<()> {
    if has_staged_entries(repo_path)? {
        git(
            repo_path,
            [
                "commit",
                "--no-verify",
                "-m",
                "[lazygit] stashing unstaged changes",
            ],
        )?;
        if let Err(error) = stash_push(repo_path, &[], message) {
            let _ = git(repo_path, ["reset", "--soft", "HEAD^"]);
            return Err(error);
        }
        git(repo_path, ["reset", "--soft", "HEAD^"])?;
        Ok(())
    } else {
        stash_push(repo_path, &[], message)
    }
}

fn has_staged_entries(repo_path: &Path) -> GitResult<bool> {
    Ok(!git_stdout(repo_path, ["diff", "--cached", "--name-only"])?
        .trim()
        .is_empty())
}

fn rename_stash(repo_path: &Path, stash_ref: &str, message: &str) -> GitResult<()> {
    let hash = git_stdout(repo_path, ["rev-parse", stash_ref])?;
    git(repo_path, ["stash", "drop", stash_ref])?;

    let trimmed_message = message.trim();
    let mut command = Command::new("git");
    command.arg("stash").arg("store").current_dir(repo_path);
    if !trimmed_message.is_empty() {
        command.arg("-m").arg(trimmed_message);
    }
    command.arg(hash);

    let output = command.output().map_err(io_error)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn edit_submodule_url(repo_path: &Path, name: &str, path: &Path, url: &str) -> GitResult<()> {
    let url_key = format!("submodule.{name}.url");
    git(
        repo_path,
        ["config", "-f", ".gitmodules", url_key.as_str(), url],
    )?;
    git(repo_path, ["config", url_key.as_str(), url])?;
    git(
        repo_path,
        ["submodule", "sync", "--", &path.to_string_lossy()],
    )?;
    if repo_path.join(".gitmodules").exists() {
        git_path(repo_path, ["add"], Path::new(".gitmodules"))?;
    }
    Ok(())
}

fn remove_submodule(repo_path: &Path, path: &Path) -> GitResult<()> {
    let _ = git_path(repo_path, ["submodule", "deinit", "-f"], path);
    git_path(repo_path, ["rm", "-f"], path)?;
    if let Some(modules_path) = resolve_git_path(
        repo_path,
        format!("modules/{}", path.to_string_lossy()).as_str(),
    ) {
        if modules_path.exists() {
            if modules_path.is_dir() {
                fs::remove_dir_all(modules_path).map_err(io_error)?;
            } else {
                fs::remove_file(modules_path).map_err(io_error)?;
            }
        }
    }
    Ok(())
}

fn command_failure(output: Output) -> GitError {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    GitError::OperationFailed {
        message: format!(
            "git exited with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ),
    }
}

fn git_with_env<I, S>(repo_path: &Path, args: I, envs: &[(&str, &OsStr)]) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output_with_env(repo_path, args, envs)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn start_commit_rebase(repo_path: &Path, commit: &str, mode: &RebaseStartMode) -> GitResult<()> {
    match mode {
        RebaseStartMode::Interactive | RebaseStartMode::Amend => {
            run_scripted_rebase(repo_path, commit, "edit", None, false)
        }
        RebaseStartMode::Fixup => {
            git(repo_path, ["commit", "--fixup", commit])?;
            run_scripted_rebase(repo_path, commit, "pick", None, true)
        }
        RebaseStartMode::ApplyFixups => run_scripted_rebase(repo_path, commit, "pick", None, true),
        RebaseStartMode::Squash => run_scripted_rebase(repo_path, commit, "squash", None, false),
        RebaseStartMode::Drop => run_scripted_rebase(repo_path, commit, "drop", None, false),
        RebaseStartMode::MoveUp { adjacent_commit } => {
            run_reordered_rebase(repo_path, commit, adjacent_commit, true)
        }
        RebaseStartMode::MoveDown { adjacent_commit } => {
            run_reordered_rebase(repo_path, commit, adjacent_commit, false)
        }
        RebaseStartMode::Reword { message } => {
            run_scripted_rebase(repo_path, commit, "reword", Some(message.as_str()), false)
        }
    }
}

fn run_scripted_rebase(
    repo_path: &Path,
    commit: &str,
    todo_verb: &str,
    reword_message: Option<&str>,
    autosquash: bool,
) -> GitResult<()> {
    let tempdir = tempfile::tempdir().map_err(io_error)?;
    let sequence_editor = tempdir.path().join("sequence-editor.sh");
    let sequence_script = if autosquash {
        "#!/bin/sh\nset -eu\n:\n".to_string()
    } else {
        format!(
            "#!/bin/sh\nset -eu\nfile=\"$1\"\ntmp=\"$1.tmp\"\nawk 'BEGIN{{done=0}} {{ if (!done && $1 == \"pick\" && index(\"{commit}\", $2) == 1) {{ sub(/^pick /, \"{todo_verb} \"); done=1 }} print }}' \"$file\" > \"$tmp\"\nmv \"$tmp\" \"$file\"\n"
        )
    };
    write_executable_script(&sequence_editor, &sequence_script)?;

    let editor_path = tempdir.path().join("git-editor.sh");
    let mut envs: Vec<(&str, &OsStr)> = vec![("GIT_SEQUENCE_EDITOR", sequence_editor.as_os_str())];

    if let Some(message) = reword_message {
        write_executable_script(
            &editor_path,
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$SUPER_LAZYGIT_REWORD\" > \"$1\"\n",
        )?;
        envs.push(("GIT_EDITOR", editor_path.as_os_str()));
        envs.push(("SUPER_LAZYGIT_REWORD", OsStr::new(message)));
    } else {
        envs.push(("GIT_EDITOR", OsStr::new(":")));
    }

    let mut args = vec!["rebase".to_string(), "-i".to_string()];
    if autosquash {
        args.push("--autosquash".to_string());
    }
    if todo_verb == "squash" {
        let parent = git_stdout(repo_path, ["rev-parse", &format!("{commit}^")])?;
        args.extend(rebase_base_args(repo_path, &parent));
    } else {
        args.extend(rebase_base_args(repo_path, commit));
    }

    git_with_env(repo_path, args.iter().map(String::as_str), &envs)
}

fn run_reordered_rebase(
    repo_path: &Path,
    commit: &str,
    adjacent_commit: &str,
    move_up: bool,
) -> GitResult<()> {
    let (older, newer) = if move_up {
        (commit, adjacent_commit)
    } else {
        (adjacent_commit, commit)
    };
    let tempdir = tempfile::tempdir().map_err(io_error)?;
    let sequence_editor = tempdir.path().join("sequence-editor.sh");
    let sequence_script = format!(
        "#!/bin/sh\nset -eu\nfile=\"$1\"\ntmp=\"$1.tmp\"\nawk 'BEGIN{{swapped=0; older=\"{older}\"; newer=\"{newer}\"}} {{ if (!swapped && $1 == \"pick\" && index(older, $2) == 1) {{ older_line=$0; if ((getline newer_line) <= 0) {{ print older_line; next }} split(newer_line, newer_fields, \" \"); if (newer_fields[1] == \"pick\" && index(newer, newer_fields[2]) == 1) {{ print newer_line; print older_line; swapped=1; next }} print older_line; print newer_line; next }} print }} END {{ if (!swapped) exit 3 }}' \"$file\" > \"$tmp\"\nmv \"$tmp\" \"$file\"\n"
    );
    write_executable_script(&sequence_editor, &sequence_script)?;

    let envs: Vec<(&str, &OsStr)> = vec![
        ("GIT_SEQUENCE_EDITOR", sequence_editor.as_os_str()),
        ("GIT_EDITOR", OsStr::new(":")),
    ];
    let mut args = vec!["rebase".to_string(), "-i".to_string()];
    args.extend(rebase_base_args(repo_path, older));
    git_with_env(repo_path, args.iter().map(String::as_str), &envs)
}

fn write_executable_script(path: &Path, contents: &str) -> GitResult<()> {
    fs::write(path, contents).map_err(io_error)?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path).map_err(io_error)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(io_error)?;
    }
    Ok(())
}

fn rebase_base_args(repo_path: &Path, commit: &str) -> Vec<String> {
    git_stdout(repo_path, ["rev-parse", &format!("{commit}^")])
        .map(|parent| vec![parent])
        .unwrap_or_else(|_| vec!["--root".to_string()])
}

fn cherry_pick_commit(repo_path: &Path, commit: &str) -> GitResult<()> {
    git(repo_path, ["cherry-pick", commit])
}

fn revert_commit(repo_path: &Path, commit: &str) -> GitResult<()> {
    git(repo_path, ["revert", "--no-edit", commit])
}

fn discard_path(repo_path: &Path, path: &Path) -> GitResult<()> {
    if path_exists_in_head(repo_path, path)? {
        git_path(
            repo_path,
            ["restore", "--source=HEAD", "--staged", "--worktree"],
            path,
        )
    } else if path_exists_in_index(repo_path, path)? {
        git_path(repo_path, ["rm", "-f"], path)
    } else {
        git_path(repo_path, ["clean", "-f"], path)
    }
}

fn reset_to_commit(repo_path: &Path, mode: ResetMode, target: &str) -> GitResult<()> {
    git(repo_path, ["reset", reset_mode_flag(mode), target])
}

fn nuke_working_tree(repo_path: &Path) -> GitResult<()> {
    git(repo_path, ["reset", "--hard", "HEAD"])?;
    git(repo_path, ["clean", "-fd"])
}

fn unstage_path(repo_path: &Path, path: &Path) -> GitResult<()> {
    let restore = git_path_output(repo_path, ["restore", "--staged"], path)?;
    if restore.status.success() {
        return Ok(());
    }

    let rm_cached = git_path_output(repo_path, ["rm", "--cached"], path)?;
    if rm_cached.status.success() {
        return Ok(());
    }

    Err(GitError::OperationFailed {
        message: format!(
            "git restore --staged failed:\n{}\n\ngit rm --cached failed:\n{}",
            command_failure(restore),
            command_failure(rm_cached)
        ),
    })
}

fn io_error(error: std::io::Error) -> GitError {
    GitError::OperationFailed {
        message: error.to_string(),
    }
}

fn path_exists_in_head(repo_path: &Path, path: &Path) -> GitResult<bool> {
    let spec = format!("HEAD:{}", path.to_string_lossy());
    let output = Command::new("git")
        .arg("cat-file")
        .arg("-e")
        .arg(spec)
        .current_dir(repo_path)
        .output()
        .map_err(io_error)?;
    Ok(output.status.success())
}

fn path_exists_in_index(repo_path: &Path, path: &Path) -> GitResult<bool> {
    let output = git_path_output(repo_path, ["ls-files", "--error-unmatch", "--cached"], path)?;
    Ok(output.status.success())
}

fn reset_mode_flag(mode: ResetMode) -> &'static str {
    match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    }
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

fn unix_timestamp_now() -> Timestamp {
    system_time_to_timestamp(SystemTime::now())
}

fn system_time_to_timestamp(time: SystemTime) -> Timestamp {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Timestamp(seconds)
}

#[derive(Debug, Default)]
struct ParsedStatus {
    branch: Option<String>,
    head_kind: HeadKind,
    tracking_branch: Option<String>,
    remote_name: Option<String>,
    staged_count: u32,
    unstaged_count: u32,
    untracked_count: u32,
    ahead_count: u32,
    behind_count: u32,
    conflicted: bool,
    file_tree: Vec<FileStatus>,
    first_path: Option<PathBuf>,
}

fn read_status_snapshot(repo_path: &Path) -> GitResult<ParsedStatus> {
    let status = git_stdout(repo_path, ["status", "--short", "--branch"])?;
    Ok(parse_status(&status))
}

fn parse_status(status: &str) -> ParsedStatus {
    let mut parsed = ParsedStatus::default();

    for (index, line) in status.lines().enumerate() {
        if index == 0 {
            if let Some(branch_line) = line.strip_prefix("## ") {
                parse_branch_header(branch_line, &mut parsed);
                continue;
            }
        }

        let bytes = line.as_bytes();
        if bytes.len() < 3 {
            continue;
        }
        let staged = bytes[0] as char;
        let unstaged = bytes[1] as char;
        let path = status_path(&line[3..]);

        if staged == '?' && unstaged == '?' {
            parsed.untracked_count += 1;
            parsed.file_tree.push(FileStatus {
                path: path.clone(),
                kind: FileStatusKind::Untracked,
                staged_kind: None,
                unstaged_kind: Some(FileStatusKind::Untracked),
            });
            parsed.first_path.get_or_insert(path);
            continue;
        }

        if is_conflict_code(staged, unstaged) {
            parsed.conflicted = true;
        }
        if staged != ' ' && staged != '?' {
            parsed.staged_count += 1;
        }
        if unstaged != ' ' && unstaged != '?' {
            parsed.unstaged_count += 1;
        }

        parsed.file_tree.push(FileStatus {
            path: path.clone(),
            kind: status_kind(staged, unstaged),
            staged_kind: staged_status_kind(staged, unstaged),
            unstaged_kind: unstaged_status_kind(staged, unstaged),
        });
        parsed.first_path.get_or_insert(path);
    }

    parsed
}

fn parse_branch_header(header: &str, parsed: &mut ParsedStatus) {
    if let Some(branch) = header.strip_prefix("No commits yet on ") {
        parsed.branch = Some(branch.to_string());
        parsed.head_kind = HeadKind::Unborn;
        return;
    }

    if header.starts_with("HEAD ") {
        parsed.head_kind = HeadKind::Detached;
        return;
    }

    parsed.head_kind = HeadKind::Branch;
    let (branch_part, counts_part) = header
        .split_once(" [")
        .map_or((header, None), |(left, right)| {
            (left, Some(right.trim_end_matches(']')))
        });

    if let Some((branch, upstream)) = branch_part.split_once("...") {
        parsed.branch = Some(branch.to_string());
        parsed.tracking_branch = Some(upstream.to_string());
        parsed.remote_name = upstream.split('/').next().map(str::to_owned);
    } else {
        parsed.branch = Some(branch_part.to_string());
    }

    if let Some(divergence) = counts_part {
        for token in divergence.split(',').map(str::trim) {
            if let Some(count) = token.strip_prefix("ahead ") {
                parsed.ahead_count = count.parse().unwrap_or(0);
            }
            if let Some(count) = token.strip_prefix("behind ") {
                parsed.behind_count = count.parse().unwrap_or(0);
            }
        }
    }
}

fn status_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    let path = trimmed
        .rsplit(" -> ")
        .next()
        .unwrap_or(trimmed)
        .trim_matches('"');
    PathBuf::from(path)
}

fn status_kind(staged: char, unstaged: char) -> FileStatusKind {
    if is_conflict_code(staged, unstaged) {
        return FileStatusKind::Conflicted;
    }

    let code = if status_code_kind(staged).is_some() {
        staged
    } else {
        unstaged
    };

    status_code_kind(code).unwrap_or(FileStatusKind::Modified)
}

fn staged_status_kind(staged: char, unstaged: char) -> Option<FileStatusKind> {
    if is_conflict_code(staged, unstaged) {
        None
    } else {
        status_code_kind(staged)
    }
}

fn unstaged_status_kind(staged: char, unstaged: char) -> Option<FileStatusKind> {
    if is_conflict_code(staged, unstaged) {
        Some(FileStatusKind::Conflicted)
    } else {
        status_code_kind(unstaged)
    }
}

fn status_code_kind(code: char) -> Option<FileStatusKind> {
    Some(match code {
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        '?' => FileStatusKind::Untracked,
        ' ' => return None,
        _ => FileStatusKind::Modified,
    })
}

fn read_branches(repo_path: &Path) -> Vec<BranchItem> {
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--format=%(HEAD)%00%(refname:short)%00%(upstream:short)",
            "refs/heads",
        ],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let mut parts = trimmed.split('\0');
                let head = parts.next().unwrap_or_default().trim();
                let name = parts.next().unwrap_or_default().trim();
                let upstream = parts.next().unwrap_or_default().trim();
                if name.is_empty() {
                    return None;
                }
                Some(BranchItem {
                    name: name.to_string(),
                    is_head: head == "*",
                    upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

fn read_remotes(repo_path: &Path, remote_branches: &[RemoteBranchItem]) -> Vec<RemoteItem> {
    git_stdout(repo_path, ["remote", "-v"])
        .map(|output| {
            let mut remotes = BTreeMap::<String, RemoteItem>::new();
            for line in output.lines() {
                let mut parts = line.split_whitespace();
                let Some(name) = parts.next() else {
                    continue;
                };
                let Some(url) = parts.next() else {
                    continue;
                };
                let Some(direction) = parts.next() else {
                    continue;
                };

                let remote = remotes
                    .entry(name.to_string())
                    .or_insert_with(|| RemoteItem {
                        name: name.to_string(),
                        fetch_url: String::new(),
                        push_url: String::new(),
                        branch_count: 0,
                    });
                match direction.trim_matches(|ch| ch == '(' || ch == ')') {
                    "fetch" => remote.fetch_url = url.to_string(),
                    "push" => remote.push_url = url.to_string(),
                    _ => {}
                }
            }

            for remote in remotes.values_mut() {
                if remote.fetch_url.is_empty() {
                    remote.fetch_url = remote.push_url.clone();
                }
                if remote.push_url.is_empty() {
                    remote.push_url = remote.fetch_url.clone();
                }
                remote.branch_count = remote_branches
                    .iter()
                    .filter(|branch| branch.remote_name == remote.name)
                    .count();
            }

            remotes.into_values().collect()
        })
        .unwrap_or_default()
}

fn read_remote_branches(repo_path: &Path) -> Vec<RemoteBranchItem> {
    git_stdout(
        repo_path,
        ["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(|line| {
                let name = line.trim();
                if name.is_empty() || name.ends_with("/HEAD") {
                    return None;
                }
                let (remote_name, branch_name) = name.split_once('/')?;
                if branch_name.is_empty() {
                    return None;
                }
                Some(RemoteBranchItem {
                    name: name.to_string(),
                    remote_name: remote_name.to_string(),
                    branch_name: branch_name.to_string(),
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

fn read_tags(repo_path: &Path) -> Vec<TagItem> {
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(refname:short)%00%(objecttype)%00%(objectname)%00%(*objectname)%00%(subject)",
            "refs/tags",
        ],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let mut parts = trimmed.split('\0');
                let name = parts.next().unwrap_or_default().trim();
                let object_type = parts.next().unwrap_or_default().trim();
                let object_oid = parts.next().unwrap_or_default().trim();
                let peeled_oid = parts.next().unwrap_or_default().trim();
                let summary = parts.next().unwrap_or_default().trim();
                if name.is_empty() {
                    return None;
                }
                let target_oid = if peeled_oid.is_empty() {
                    object_oid
                } else {
                    peeled_oid
                };
                if target_oid.is_empty() {
                    return None;
                }
                Some(TagItem {
                    name: name.to_string(),
                    target_oid: target_oid.to_string(),
                    target_short_oid: target_oid.chars().take(7).collect(),
                    summary: summary.to_string(),
                    annotated: object_type == "tag",
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

fn local_branch_exists(repo_path: &Path, branch_name: &str) -> GitResult<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/heads/{branch_name}"))
        .status()
        .map_err(|error| GitError::OperationFailed {
            message: format!("failed to inspect local branch {branch_name}: {error}"),
        })?;
    Ok(status.success())
}

#[derive(Default)]
struct CommitHistoryResult {
    commits: Vec<CommitItem>,
    graph_lines: Vec<String>,
}

fn read_commits(
    repo_path: &Path,
    commit_ref: Option<&str>,
    commit_history_mode: CommitHistoryMode,
) -> CommitHistoryResult {
    match commit_history_mode {
        CommitHistoryMode::Linear => read_linear_commits(repo_path, commit_ref),
        CommitHistoryMode::Graph { reverse } => read_graph_commits(repo_path, reverse),
    }
}

fn read_linear_commits(repo_path: &Path, commit_ref: Option<&str>) -> CommitHistoryResult {
    let mut args = vec!["log", "--format=%H%x00%s", "-n", "64"];
    if let Some(commit_ref) = commit_ref {
        args.push(commit_ref);
    }
    git_stdout(repo_path, args)
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let (oid, summary) = line.split_once('\0')?;
                    let changed_files = read_commit_files(repo_path, oid);
                    let diff = read_commit_diff(repo_path, oid);
                    Some(CommitItem {
                        oid: oid.to_string(),
                        short_oid: oid.chars().take(7).collect(),
                        summary: summary.to_string(),
                        changed_files,
                        diff,
                    })
                })
                .collect::<Vec<_>>()
        })
        .map(|commits| CommitHistoryResult {
            commits,
            graph_lines: Vec::new(),
        })
        .unwrap_or_default()
}

fn read_graph_commits(repo_path: &Path, reverse: bool) -> CommitHistoryResult {
    let args = vec![
        "log",
        "--graph",
        "--decorate=short",
        "--format=%x1f%H%x00%h%x00%D%x00%s",
        "--all",
        "-n",
        "64",
    ];
    git_stdout(repo_path, args)
        .map(|output| {
            let mut commits = Vec::new();
            let mut graph_lines = Vec::new();
            for line in output.lines() {
                let Some((graph_prefix, payload)) = line.split_once('\x1f') else {
                    continue;
                };
                let mut parts = payload.splitn(4, '\0');
                let Some(oid) = parts.next() else {
                    continue;
                };
                let Some(short_oid) = parts.next() else {
                    continue;
                };
                let decorations = parts.next().unwrap_or_default().trim();
                let summary = parts.next().unwrap_or_default();
                let graph_line = if decorations.is_empty() {
                    format!("{graph_prefix}{short_oid} {summary}")
                } else {
                    format!("{graph_prefix}{short_oid} ({decorations}) {summary}")
                };
                let changed_files = read_commit_files(repo_path, oid);
                let diff = read_commit_diff(repo_path, oid);
                commits.push(CommitItem {
                    oid: oid.to_string(),
                    short_oid: short_oid.to_string(),
                    summary: summary.to_string(),
                    changed_files,
                    diff,
                });
                graph_lines.push(graph_line);
            }
            if reverse {
                commits.reverse();
                graph_lines.reverse();
            }
            CommitHistoryResult {
                commits,
                graph_lines,
            }
        })
        .unwrap_or_default()
}

fn read_commit_files(repo_path: &Path, oid: &str) -> Vec<CommitFileItem> {
    git_stdout(
        repo_path,
        ["show", "--format=", "--name-status", "--no-renames", oid],
    )
    .map(|output| parse_name_status_lines(&output))
    .unwrap_or_default()
}

fn read_commit_diff(repo_path: &Path, oid: &str) -> DiffModel {
    git_stdout(
        repo_path,
        [
            "show",
            "--format=",
            "--no-ext-diff",
            "--binary",
            "--unified=3",
            oid,
        ],
    )
    .map(|output| parse_diff_model(None, DiffPresentation::Comparison, &output))
    .unwrap_or_default()
}

fn commit_status_kind(code: &str) -> FileStatusKind {
    match code.chars().next().unwrap_or('M') {
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        'U' => FileStatusKind::Conflicted,
        _ => FileStatusKind::Modified,
    }
}

fn read_stashes(repo_path: &Path) -> Vec<StashItem> {
    git_stdout(repo_path, ["stash", "list", "--format=%gd%x00%s"])
        .map(|output| {
            output
                .lines()
                .map(|line| {
                    let (stash_ref, label) = line.split_once('\0').map_or_else(
                        || (line.to_string(), line.to_string()),
                        |(name, summary)| (name.to_string(), format!("{name}: {summary}")),
                    );
                    let changed_files = read_stash_files(repo_path, &stash_ref);
                    StashItem {
                        stash_ref,
                        label,
                        changed_files,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn read_stash_files(repo_path: &Path, stash_ref: &str) -> Vec<CommitFileItem> {
    git_stdout(
        repo_path,
        ["stash", "show", "--name-status", "--no-renames", stash_ref],
    )
    .map(|output| parse_name_status_lines(&output))
    .unwrap_or_default()
}

fn parse_name_status_lines(output: &str) -> Vec<CommitFileItem> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let (code, path) = trimmed
                .split_once('\t')
                .or_else(|| trimmed.split_once(char::is_whitespace))?;
            Some(CommitFileItem {
                path: PathBuf::from(path.trim()),
                kind: commit_status_kind(code),
            })
        })
        .collect()
}

fn read_reflog(repo_path: &Path) -> Vec<ReflogItem> {
    git_stdout(
        repo_path,
        ["reflog", "--format=%gD%x00%H%x00%h%x00%gs", "-n", "64"],
    )
    .map(|output| {
        output
            .lines()
            .map(|line| {
                let mut parts = line.split('\0');
                let selector = parts.next().unwrap_or_default().to_string();
                let oid = parts.next().unwrap_or_default().to_string();
                let short_oid = parts.next().unwrap_or_default().to_string();
                let summary = parts.next().unwrap_or_default().to_string();
                let description = if selector.is_empty() {
                    line.to_string()
                } else if summary.is_empty() {
                    selector.clone()
                } else {
                    format!("{selector}: {summary}")
                };
                ReflogItem {
                    selector,
                    oid,
                    short_oid,
                    summary,
                    description,
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

fn read_worktrees(repo_path: &Path) -> Vec<WorktreeItem> {
    let output = match git_stdout(repo_path, ["worktree", "list", "--porcelain"]) {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };

    let mut items = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(path) = current_path.take() {
                items.push(WorktreeItem {
                    path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(path));
            current_branch = None;
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_string());
        }
    }

    if let Some(path) = current_path {
        items.push(WorktreeItem {
            path,
            branch: current_branch,
        });
    }

    items
}

#[derive(Debug, Clone, Default)]
struct SubmoduleStatusSnapshot {
    short_oid: Option<String>,
    initialized: bool,
    dirty: bool,
    conflicted: bool,
    branch: Option<String>,
}

fn read_submodules(repo_path: &Path) -> Vec<SubmoduleItem> {
    let paths_output = match git_stdout_allow_failure(
        repo_path,
        [
            "config",
            "-f",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ],
    ) {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    if paths_output.trim().is_empty() {
        return Vec::new();
    }

    let status_by_path = read_submodule_statuses(repo_path);
    let mut submodules = Vec::new();
    for line in paths_output.lines() {
        let Some((key, path_value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Some(name) = key
            .strip_prefix("submodule.")
            .and_then(|value| value.strip_suffix(".path"))
        else {
            continue;
        };
        let path = PathBuf::from(path_value.trim());
        let url_key = format!("submodule.{name}.url");
        let url = git_stdout_allow_failure(
            repo_path,
            ["config", "-f", ".gitmodules", "--get", url_key.as_str()],
        )
        .unwrap_or_default();
        let status = status_by_path
            .get(path.as_os_str())
            .cloned()
            .unwrap_or_default();
        let submodule_repo_path = repo_path.join(&path);
        let initialized = status.initialized || submodule_repo_path.join(".git").exists();
        let short_oid = if initialized {
            git_stdout_allow_failure(&submodule_repo_path, ["rev-parse", "--short", "HEAD"])
                .ok()
                .filter(|value| !value.is_empty())
                .or(status.short_oid)
        } else {
            status.short_oid
        };
        let branch = if initialized {
            git_stdout_allow_failure(&submodule_repo_path, ["branch", "--show-current"])
                .ok()
                .filter(|value| !value.is_empty())
                .or(status.branch)
        } else {
            status.branch
        };
        let dirty = if initialized {
            let has_changes = git_stdout_allow_failure(&submodule_repo_path, ["status", "--short"])
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            status.dirty || has_changes
        } else {
            status.dirty
        };
        submodules.push(SubmoduleItem {
            name: name.to_string(),
            path,
            url,
            branch,
            short_oid,
            initialized,
            dirty,
            conflicted: status.conflicted,
        });
    }
    submodules.sort_by(|left, right| left.path.cmp(&right.path));
    submodules
}

fn read_submodule_statuses(
    repo_path: &Path,
) -> BTreeMap<std::ffi::OsString, SubmoduleStatusSnapshot> {
    let output = match git_stdout_allow_failure(repo_path, ["submodule", "status", "--recursive"]) {
        Ok(output) => output,
        Err(_) => return BTreeMap::new(),
    };

    let mut statuses = BTreeMap::new();
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let status_char = line.chars().next().unwrap_or(' ');
        let remainder = line[1..].trim_start();
        let mut parts = remainder.split_whitespace();
        let oid = parts.next().unwrap_or_default();
        let Some(path) = parts.next() else {
            continue;
        };
        let branch = line
            .split_once(" (")
            .and_then(|(_, suffix)| suffix.strip_suffix(')'))
            .map(normalize_submodule_branch_hint);
        statuses.insert(
            std::ffi::OsString::from(path),
            SubmoduleStatusSnapshot {
                short_oid: (!oid.is_empty()).then(|| oid.chars().take(7).collect()),
                initialized: status_char != '-',
                dirty: status_char == '+',
                conflicted: status_char == 'U',
                branch,
            },
        );
    }
    statuses
}

fn normalize_submodule_branch_hint(value: &str) -> String {
    value
        .strip_prefix("heads/")
        .or_else(|| value.strip_prefix("remotes/"))
        .unwrap_or(value)
        .to_string()
}

fn read_rebase_state(repo_path: &Path) -> Option<RebaseState> {
    let merge_dir = resolve_git_path(repo_path, "rebase-merge").filter(|path| path.exists());
    let apply_dir = resolve_git_path(repo_path, "rebase-apply").filter(|path| path.exists());
    let (dir, kind) = if let Some(dir) = merge_dir {
        let interactive = dir.join("interactive").exists();
        (
            dir,
            if interactive {
                RebaseKind::Interactive
            } else {
                RebaseKind::Apply
            },
        )
    } else if let Some(dir) = apply_dir {
        (dir, RebaseKind::Apply)
    } else {
        return None;
    };

    let step = read_usize_file(&dir.join("msgnum"))
        .or_else(|| read_usize_file(&dir.join("next")))
        .unwrap_or(0);
    let total = read_usize_file(&dir.join("end"))
        .or_else(|| read_usize_file(&dir.join("last")))
        .unwrap_or(step);
    let head_name = read_trimmed_file(&dir.join("head-name")).map(normalize_head_name);
    let onto = read_trimmed_file(&dir.join("onto"));
    let current_commit = git_stdout(repo_path, ["rev-parse", "--verify", "REBASE_HEAD"])
        .ok()
        .or_else(|| read_trimmed_file(&dir.join("stopped-sha")));
    let current_summary = current_commit
        .as_deref()
        .and_then(|commit| git_stdout(repo_path, ["show", "-s", "--format=%s", commit]).ok());

    Some(RebaseState {
        kind,
        step,
        total,
        head_name,
        onto,
        current_commit,
        current_summary,
        todo_preview: read_rebase_todo_preview(&dir),
    })
}

fn read_merge_state(repo_path: &Path) -> MergeState {
    if git_path_exists(repo_path, "MERGE_HEAD") {
        MergeState::MergeInProgress
    } else if git_path_exists(repo_path, "rebase-merge")
        || git_path_exists(repo_path, "rebase-apply")
    {
        MergeState::RebaseInProgress
    } else if git_path_exists(repo_path, "CHERRY_PICK_HEAD") {
        MergeState::CherryPickInProgress
    } else if git_path_exists(repo_path, "REVERT_HEAD") {
        MergeState::RevertInProgress
    } else {
        MergeState::None
    }
}

fn git_path_exists(repo_path: &Path, git_path: &str) -> bool {
    resolve_git_path(repo_path, git_path).is_some_and(|path| path.exists())
}

fn resolve_git_path(repo_path: &Path, git_path: &str) -> Option<PathBuf> {
    git_stdout(repo_path, ["rev-parse", "--git-path", git_path])
        .ok()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo_path.join(path)
            }
        })
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| contents.trim().to_string())
        .filter(|contents| !contents.is_empty())
}

fn read_usize_file(path: &Path) -> Option<usize> {
    read_trimmed_file(path)?.parse().ok()
}

fn normalize_head_name(value: String) -> String {
    value
        .strip_prefix("refs/heads/")
        .or_else(|| value.strip_prefix("refs/remotes/"))
        .unwrap_or(value.as_str())
        .to_string()
}

fn read_rebase_todo_preview(dir: &Path) -> Vec<String> {
    read_trimmed_file(&dir.join("git-rebase-todo"))
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .take(3)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn is_conflict_code(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPatch {
    header_lines: Vec<String>,
    hunks: Vec<ParsedHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHunk {
    selection: SelectedHunk,
    raw: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_lazygit_core::{
        DiffModel, GitCommand, GitCommandRequest, JobId, RebaseKind, RebaseStartMode, RepoId,
        ResetMode,
    };
    use super_lazygit_test_support::{
        clean_repo, conflicted_repo, detached_head_repo, dirty_repo, history_preview_repo,
        rebase_in_progress_repo, staged_and_unstaged_repo, stashed_repo, submodule_repo, temp_repo,
        upstream_diverged_repo, worktree_repo, TempRepo,
    };

    #[derive(Debug, Clone, Copy)]
    struct StubBackend {
        kind: GitBackendKind,
    }

    impl GitBackend for StubBackend {
        fn kind(&self) -> GitBackendKind {
            self.kind
        }

        fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult> {
            Ok(WorkspaceScanResult {
                root: request.root,
                repo_ids: vec![RepoId::new("repo-a")],
            })
        }

        fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary> {
            Ok(RepoSummary {
                repo_id: request.repo_id,
                display_name: "repo-a".to_string(),
                ..RepoSummary::default()
            })
        }

        fn read_repo_detail(&self, _request: RepoDetailRequest) -> GitResult<RepoDetail> {
            Ok(RepoDetail::default())
        }

        fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel> {
            Ok(DiffModel {
                selected_path: request.selected_path,
                presentation: request.diff_presentation,
                lines: vec![DiffLine {
                    kind: DiffLineKind::HunkHeader,
                    content: "@@ -1,1 +1,1 @@".to_string(),
                }],
                hunks: vec![DiffHunk {
                    header: "@@ -1,1 +1,1 @@".to_string(),
                    selection: SelectedHunk {
                        old_start: 1,
                        old_lines: 1,
                        new_start: 1,
                        new_lines: 1,
                    },
                    start_line_index: 0,
                    end_line_index: 1,
                }],
                selected_hunk: Some(0),
                hunk_count: 1,
            })
        }

        fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
            let summary = git_command_label(&request).to_string();
            Ok(GitCommandOutcome {
                repo_id: request.repo_id,
                summary,
            })
        }

        fn apply_patch_selection(
            &self,
            request: PatchSelectionRequest,
        ) -> GitResult<GitCommandOutcome> {
            Ok(GitCommandOutcome {
                repo_id: request.repo_id,
                summary: format!("{:?} patch selection", request.mode),
            })
        }
    }

    fn run_git(dir: &Path, args: &[&str]) -> std::io::Result<()> {
        let output = Command::new("git").args(args).current_dir(dir).output()?;
        if output.status.success() {
            return Ok(());
        }

        Err(std::io::Error::other(format!(
            "git {:?} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )))
    }

    fn init_repo_at(
        path: &Path,
        tracked_file: &str,
        contents: &str,
        message: &str,
    ) -> std::io::Result<()> {
        fs::create_dir_all(path)?;
        run_git(path, &["init", "--initial-branch=main"])?;
        run_git(path, &["config", "user.name", "Super Lazygit Tests"])?;
        run_git(path, &["config", "user.email", "tests@example.com"])?;
        fs::write(path.join(tracked_file), contents)?;
        run_git(path, &["add", "."])?;
        run_git(path, &["commit", "-m", message])?;
        Ok(())
    }

    fn nested_workspace() -> std::io::Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        let outer = root.path().join("outer");
        let inner = outer.join("vendor/inner");

        init_repo_at(&outer, "outer.txt", "outer\n", "outer init")?;
        init_repo_at(&inner, "inner.txt", "inner\n", "inner init")?;

        Ok(root)
    }

    fn linked_worktree_workspace() -> std::io::Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        let main_repo = root.path().join("main");
        let worktree = root.path().join("feature-tree");

        init_repo_at(&main_repo, "main.txt", "main\n", "initial")?;
        run_git(&main_repo, &["branch", "feature"])?;
        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                worktree.to_str().unwrap_or("feature-tree"),
                "feature",
            ],
        )?;

        Ok(root)
    }

    #[test]
    fn git_facade_records_operation_latency() {
        let mut git = GitFacade::default();

        git.record_operation("status", true);

        let snapshot = git.diagnostics();
        assert_eq!(snapshot.git_operations.len(), 1);
        assert_eq!(snapshot.git_operations[0].operation, "status");
        assert!(snapshot.git_operations[0].success);
    }

    #[test]
    fn default_routing_prefers_cli_for_writes_and_diffs() {
        let facade = GitFacade::with_routing(
            StubBackend {
                kind: GitBackendKind::Cli,
            },
            GitBackendRoutingPolicy::default(),
        );

        assert_eq!(
            facade.route_for(GitOperationKind::WriteCommand).backend,
            GitBackendKind::Cli
        );
        assert_eq!(
            facade.route_for(GitOperationKind::ReadDiff).backend,
            GitBackendKind::Cli
        );
        assert_eq!(
            facade.route_for(GitOperationKind::ReadRepoSummary).backend,
            GitBackendKind::Cli
        );
    }

    #[test]
    fn facade_fails_fast_when_route_backend_is_unavailable() {
        let mut facade = GitFacade::with_routing(
            StubBackend {
                kind: GitBackendKind::Git2,
            },
            GitBackendRoutingPolicy {
                primary_backend: GitBackendKind::Git2,
                ..GitBackendRoutingPolicy::default()
            },
        );

        let error = facade
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-1"),
                repo_id: RepoId::new("repo-a"),
                command: GitCommand::PushCurrentBranch,
            })
            .expect_err("route mismatch should fail fast");

        assert_eq!(
            error,
            GitError::RouteUnavailable {
                operation: GitOperationKind::WriteCommand.label(),
                backend: GitBackendKind::Cli.label(),
            }
        );
        assert_eq!(facade.diagnostics().git_operations.len(), 1);
        assert!(facade.diagnostics().git_operations[0]
            .operation
            .contains("write_command via git-cli"));
        assert!(!facade.diagnostics().git_operations[0].success);
    }

    #[test]
    fn facade_executes_when_route_matches_active_backend() {
        let mut facade = GitFacade::with_routing(
            StubBackend {
                kind: GitBackendKind::Git2,
            },
            GitBackendRoutingPolicy {
                primary_backend: GitBackendKind::Git2,
                writes: BackendPreference::PrimaryOnly,
                ..GitBackendRoutingPolicy::default()
            },
        );

        let summary = facade
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-2"),
                repo_id: RepoId::new("repo-a"),
                command: GitCommand::PushCurrentBranch,
            })
            .expect("primary-routed command should succeed");

        assert_eq!(summary.summary, "push_current_branch");
        assert_eq!(facade.diagnostics().git_operations.len(), 1);
        assert!(facade.diagnostics().git_operations[0]
            .operation
            .contains("write_command via git2"));
        assert!(facade.diagnostics().git_operations[0].success);
    }

    #[test]
    fn cli_backend_scans_workspace_root_repo() {
        let repo = clean_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let result = backend
            .scan_workspace(WorkspaceScanRequest {
                root: Some(repo.path().to_path_buf()),
            })
            .expect("scan succeeds");

        assert_eq!(
            result.repo_ids,
            vec![RepoId::new(
                fs::canonicalize(repo.path())
                    .expect("canonical repo path")
                    .display()
                    .to_string()
            )]
        );
    }

    #[test]
    fn cli_backend_stops_descending_once_repo_root_is_found() {
        let workspace = nested_workspace().expect("nested workspace");
        let backend = CliGitBackend;

        let result = backend
            .scan_workspace(WorkspaceScanRequest {
                root: Some(workspace.path().to_path_buf()),
            })
            .expect("scan succeeds");

        assert_eq!(result.repo_ids.len(), 1);
        assert_eq!(
            result.repo_ids,
            vec![RepoId::new(
                fs::canonicalize(workspace.path().join("outer"))
                    .expect("canonical outer repo")
                    .display()
                    .to_string()
            )]
        );
    }

    #[test]
    fn cli_backend_discovers_gitdir_file_worktrees() {
        let workspace = linked_worktree_workspace().expect("linked worktree workspace");
        let backend = CliGitBackend;

        let result = backend
            .scan_workspace(WorkspaceScanRequest {
                root: Some(workspace.path().to_path_buf()),
            })
            .expect("scan succeeds");

        assert_eq!(result.repo_ids.len(), 2);
        assert!(result.repo_ids.contains(&RepoId::new(
            fs::canonicalize(workspace.path().join("main"))
                .expect("canonical main repo")
                .display()
                .to_string()
        )));
        assert!(result.repo_ids.contains(&RepoId::new(
            fs::canonicalize(workspace.path().join("feature-tree"))
                .expect("canonical worktree repo")
                .display()
                .to_string()
        )));
    }

    #[test]
    fn cli_backend_ignores_broken_gitdir_files_and_keeps_valid_repos() {
        let workspace = tempfile::tempdir().expect("workspace");
        let broken = workspace.path().join("broken");
        let valid = workspace.path().join("valid");
        fs::create_dir_all(&broken).expect("broken dir");
        fs::write(broken.join(".git"), "gitdir: ../missing\n").expect("broken gitdir file");
        init_repo_at(&valid, "valid.txt", "valid\n", "valid init").expect("valid repo");
        let backend = CliGitBackend;

        let result = backend
            .scan_workspace(WorkspaceScanRequest {
                root: Some(workspace.path().to_path_buf()),
            })
            .expect("scan succeeds");

        assert_eq!(
            result.repo_ids,
            vec![RepoId::new(
                fs::canonicalize(valid)
                    .expect("canonical valid repo")
                    .display()
                    .to_string()
            )]
        );
    }

    #[test]
    fn cli_backend_canonicalizes_repo_ids_from_noncanonical_roots() {
        let workspace = tempfile::tempdir().expect("workspace");
        let repo = workspace.path().join("repo");
        init_repo_at(&repo, "repo.txt", "repo\n", "repo init").expect("repo");
        let backend = CliGitBackend;

        let noncanonical_root = workspace.path().join("repo").join("..");
        let result = backend
            .scan_workspace(WorkspaceScanRequest {
                root: Some(noncanonical_root),
            })
            .expect("scan succeeds");

        assert_eq!(
            result.repo_ids,
            vec![RepoId::new(
                fs::canonicalize(repo)
                    .expect("canonical repo")
                    .display()
                    .to_string()
            )]
        );
    }

    #[test]
    fn cli_backend_reads_clean_unborn_repo_summary() {
        let repo = clean_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary succeeds");

        assert_eq!(summary.branch.as_deref(), Some("main"));
        assert_eq!(summary.head_kind, HeadKind::Unborn);
        assert!(!summary.dirty);
        assert_eq!(summary.staged_count, 0);
        assert_eq!(summary.unstaged_count, 0);
        assert_eq!(summary.untracked_count, 0);
        assert_eq!(summary.ahead_count, 0);
        assert_eq!(summary.behind_count, 0);
        assert!(!summary.conflicted);
    }

    #[test]
    fn cli_backend_reads_dirty_untracked_repo_summary() {
        let repo = dirty_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary succeeds");

        assert_eq!(summary.branch.as_deref(), Some("main"));
        assert_eq!(summary.head_kind, HeadKind::Unborn);
        assert!(summary.dirty);
        assert_eq!(summary.staged_count, 0);
        assert_eq!(summary.unstaged_count, 0);
        assert_eq!(summary.untracked_count, 1);
        assert!(!summary.conflicted);
    }

    #[test]
    fn cli_backend_reads_staged_unstaged_and_untracked_counts() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary succeeds");

        assert!(summary.dirty);
        assert_eq!(summary.staged_count, 1);
        assert_eq!(summary.unstaged_count, 1);
        assert_eq!(summary.untracked_count, 1);
    }

    #[test]
    fn cli_backend_repo_detail_tracks_staged_and_unstaged_sections() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let staged = detail
            .file_tree
            .iter()
            .find(|item| item.path == Path::new("staged.txt"))
            .expect("staged file tracked");
        assert_eq!(staged.staged_kind, Some(FileStatusKind::Added));
        assert_eq!(staged.unstaged_kind, None);

        let tracked = detail
            .file_tree
            .iter()
            .find(|item| item.path == Path::new("tracked.txt"))
            .expect("tracked file tracked");
        assert_eq!(tracked.staged_kind, None);
        assert_eq!(tracked.unstaged_kind, Some(FileStatusKind::Modified));

        let untracked = detail
            .file_tree
            .iter()
            .find(|item| item.path == Path::new("untracked.txt"))
            .expect("untracked file tracked");
        assert_eq!(untracked.staged_kind, None);
        assert_eq!(untracked.unstaged_kind, Some(FileStatusKind::Untracked));
    }

    #[test]
    fn parse_status_tracks_both_sections_for_mixed_path() {
        let parsed = parse_status("## main\nMM src/lib.rs\n");
        let entry = parsed.file_tree.first().expect("mixed entry");

        assert_eq!(parsed.staged_count, 1);
        assert_eq!(parsed.unstaged_count, 1);
        assert_eq!(entry.path, Path::new("src/lib.rs"));
        assert_eq!(entry.kind, FileStatusKind::Modified);
        assert_eq!(entry.staged_kind, Some(FileStatusKind::Modified));
        assert_eq!(entry.unstaged_kind, Some(FileStatusKind::Modified));
    }

    #[test]
    fn cli_backend_reads_ahead_behind_and_remote() {
        let repo = upstream_diverged_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary succeeds");

        assert_eq!(summary.branch.as_deref(), Some("main"));
        assert_eq!(summary.ahead_count, 1);
        assert_eq!(summary.behind_count, 1);
        assert_eq!(
            summary.remote_summary.remote_name.as_deref(),
            Some("origin")
        );
        assert_eq!(
            summary.remote_summary.tracking_branch.as_deref(),
            Some("origin/main")
        );
        assert!(summary.last_fetch_at.is_some());
    }

    #[test]
    fn cli_backend_marks_conflicted_repo() {
        let repo = conflicted_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary succeeds");

        assert!(summary.conflicted);
        assert!(summary.dirty);
    }

    #[test]
    fn cli_backend_reads_merge_state_for_conflicted_repo() {
        let repo = conflicted_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::MergeInProgress);
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
    }

    #[test]
    fn cli_backend_reads_detail_lists() {
        let repo = stashed_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert!(!detail.branches.is_empty());
        assert!(!detail.commits.is_empty());
        assert!(!detail.stashes.is_empty());
        assert!(!detail.stashes[0].changed_files.is_empty());
        assert!(detail.stashes[0]
            .changed_files
            .iter()
            .any(|file| file.path == Path::new("stash.txt")));
        assert!(!detail.reflog_items.is_empty());
        assert!(detail
            .reflog_items
            .iter()
            .any(|entry| !entry.selector.is_empty() && !entry.oid.is_empty()));
    }

    #[test]
    fn cli_backend_reads_stash_changed_files_without_rename_coalescing() {
        let repo = stash_inventory_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        let changed_files = &detail.stashes[0].changed_files;
        assert!(changed_files.iter().any(|file| {
            file.path == Path::new("modified.txt") && file.kind == FileStatusKind::Modified
        }));
        assert!(changed_files.iter().any(|file| {
            file.path == Path::new("deleted.txt") && file.kind == FileStatusKind::Deleted
        }));
        assert!(changed_files.iter().any(|file| {
            file.path == Path::new("renamed-before.txt") && file.kind == FileStatusKind::Deleted
        }));
        assert!(changed_files.iter().any(|file| {
            file.path == Path::new("renamed-after.txt") && file.kind == FileStatusKind::Added
        }));
    }

    #[test]
    fn cli_backend_applies_and_drops_stash_entries() {
        let repo = stashed_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let applied = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-apply-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::ApplyStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            })
            .expect("stash apply should succeed");
        assert_eq!(applied.summary, "Applied stash@{0}");
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("fixture stash"));
        assert_eq!(
            std::fs::read_to_string(repo.path().join("stash.txt")).expect("read stash.txt"),
            "base\nstashed\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("stash-untracked.txt"))
                .expect("read stash-untracked.txt"),
            "untracked\n"
        );

        let dropped = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-drop-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::DropStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            })
            .expect("stash drop should succeed");
        assert_eq!(dropped.repo_id, repo_id);
        assert_eq!(dropped.summary, "Dropped stash@{0}");
        assert!(repo.stash_list().expect("stash list").trim().is_empty());
    }

    #[test]
    fn cli_backend_pops_stash_entries() {
        let repo = stashed_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let popped = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-pop-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::PopStash {
                    stash_ref: "stash@{0}".to_string(),
                },
            })
            .expect("stash pop should succeed");

        assert_eq!(popped.repo_id, repo_id);
        assert_eq!(popped.summary, "Popped stash@{0}");
        assert!(repo.stash_list().expect("stash list").trim().is_empty());
        assert_eq!(
            std::fs::read_to_string(repo.path().join("stash.txt")).expect("read stash.txt"),
            "base\nstashed\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("stash-untracked.txt"))
                .expect("read stash-untracked.txt"),
            "untracked\n"
        );
    }

    #[test]
    fn cli_backend_renames_stash_entries() {
        let repo = renameable_stash_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let renamed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-rename-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::RenameStash {
                    stash_ref: "stash@{1}".to_string(),
                    message: "foo baz".to_string(),
                },
            })
            .expect("stash rename should succeed");

        assert_eq!(renamed.repo_id, repo_id);
        assert_eq!(renamed.summary, "Renamed stash@{1}");
        let stash_list = repo.stash_list().expect("stash list");
        assert!(stash_list.contains("stash@{0}: foo baz"));
        assert!(stash_list.contains("stash@{1}: On main: bar"));
    }

    #[test]
    fn cli_backend_creates_branch_from_stash_entries() {
        let repo = renameable_stash_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let created = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-branch-from-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranchFromStash {
                    stash_ref: "stash@{1}".to_string(),
                    branch_name: "stash-feature".to_string(),
                },
            })
            .expect("stash branch creation should succeed");

        assert_eq!(created.repo_id, repo_id);
        assert_eq!(
            created.summary,
            "Created and checked out stash-feature from stash@{1}"
        );
        assert_eq!(
            repo.current_branch().expect("current branch"),
            "stash-feature"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("file.txt")).expect("file contents"),
            "change to stash1\n"
        );
        let stash_list = repo.stash_list().expect("stash list");
        assert!(!stash_list.contains("foo"));
        assert!(stash_list.contains("bar"));
    }

    #[test]
    fn cli_backend_creates_tracked_only_stash_entry() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("checkpoint".to_string()),
                    mode: StashMode::Tracked,
                },
            })
            .expect("stash push should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(stashed.summary, "Stashed tracked changes: checkpoint");
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("checkpoint"));
        assert_eq!(repo.status_porcelain().expect("status"), "?? untracked.txt");
    }

    #[test]
    fn cli_backend_creates_keep_index_stash_entry() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash-keep-index"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("index-safe checkpoint".to_string()),
                    mode: StashMode::KeepIndex,
                },
            })
            .expect("stash push --keep-index should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(
            stashed.summary,
            "Stashed tracked changes and kept staged changes: index-safe checkpoint"
        );
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("index-safe checkpoint"));
        assert_eq!(
            repo.status_porcelain().expect("status"),
            "A  staged.txt\n?? untracked.txt"
        );
    }

    #[test]
    fn cli_backend_creates_stash_including_untracked_entry() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash-all"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("full checkpoint".to_string()),
                    mode: StashMode::IncludeUntracked,
                },
            })
            .expect("stash push should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(
            stashed.summary,
            "Stashed all changes including untracked: full checkpoint"
        );
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("full checkpoint"));
        assert_eq!(repo.status_porcelain().expect("status"), "");
    }

    #[test]
    fn cli_backend_creates_staged_only_stash_entry() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash-staged"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("index checkpoint".to_string()),
                    mode: StashMode::Staged,
                },
            })
            .expect("stash push --staged should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(stashed.summary, "Stashed staged changes: index checkpoint");
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("index checkpoint"));
        assert_eq!(
            repo.status_porcelain().expect("status"),
            "M tracked.txt\n?? untracked.txt"
        );
    }

    #[test]
    fn cli_backend_creates_unstaged_only_stash_entry() {
        let repo = mixed_staged_and_unstaged_file_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash-unstaged"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("worktree checkpoint".to_string()),
                    mode: StashMode::Unstaged,
                },
            })
            .expect("stash push --keep-index should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(
            stashed.summary,
            "Stashed unstaged changes: worktree checkpoint"
        );
        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("worktree checkpoint"));
        assert_eq!(repo.status_porcelain().expect("status"), "M  mixed.txt");
        let stashed_patch = git_stdout_raw(repo.path(), ["stash", "show", "-p", "stash@{0}"])
            .expect("stash show should succeed");
        assert!(stashed_patch.contains("-2\n+2 unstaged"));
        assert!(!stashed_patch.contains("-1\n+1 staged"));
    }

    #[test]
    fn cli_backend_keep_index_stash_captures_index_changes_in_entry() {
        let repo = mixed_staged_and_unstaged_file_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let stashed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-stash-keep-index-mixed"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateStash {
                    message: Some("keep-index mixed checkpoint".to_string()),
                    mode: StashMode::KeepIndex,
                },
            })
            .expect("stash push --keep-index should succeed");

        assert_eq!(stashed.repo_id, repo_id);
        assert_eq!(repo.status_porcelain().expect("status"), "M  mixed.txt");
        let stashed_patch = git_stdout_raw(repo.path(), ["stash", "show", "-p", "stash@{0}"])
            .expect("stash show should succeed");
        assert!(stashed_patch.contains("+1 staged"));
        assert!(stashed_patch.contains("+2 unstaged"));
        let stashed_index_patch = git_stdout_raw(
            repo.path(),
            ["diff", "stash@{0}^1", "stash@{0}^2", "--", "mixed.txt"],
        )
        .expect("stash index diff should succeed");
        assert!(stashed_index_patch.contains("+1 staged"));
        assert!(!stashed_index_patch.contains("+2 unstaged"));
    }

    #[test]
    fn cli_backend_reads_local_branches_with_head_and_upstream() {
        let repo = upstream_diverged_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert!(detail.branches.iter().any(|branch| branch.name == "main"
            && branch.is_head
            && branch.upstream.as_deref() == Some("origin/main")));
        assert!(detail
            .branches
            .iter()
            .all(|branch| !branch.name.starts_with("origin/")));
    }

    #[test]
    fn cli_backend_reads_commit_history_in_reverse_chronological_order() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.commits[0].summary, "add lib");
        assert_eq!(detail.commits[1].summary, "second");
        assert_eq!(detail.commits[0].short_oid.len(), 7);
    }

    #[test]
    fn cli_backend_reads_commit_history_for_selected_branch_ref() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write shared file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("checkout feature branch");
        repo.write_file("feature.txt", "feature\n")
            .expect("write feature file");
        repo.commit_all("feature branch commit")
            .expect("commit feature branch");

        repo.checkout("main").expect("return to main");
        repo.write_file("main.txt", "main\n")
            .expect("write main file");
        repo.commit_all("main branch commit")
            .expect("commit main branch");

        let backend = CliGitBackend;
        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: Some("feature".to_string()),
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.commits[0].summary, "feature branch commit");
        assert!(detail
            .commits
            .iter()
            .all(|commit| commit.summary != "main branch commit"));
    }

    #[test]
    fn cli_backend_reads_all_branch_graph_history_and_reverse_order() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write shared file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("checkout feature branch");
        repo.write_file("feature.txt", "feature\n")
            .expect("write feature file");
        repo.commit_all("feature branch commit")
            .expect("commit feature branch");

        repo.checkout("main").expect("return to main");
        repo.write_file("main.txt", "main\n")
            .expect("write main file");
        repo.commit_all("main branch commit")
            .expect("commit main branch");
        repo.git(["merge", "--no-ff", "feature", "-m", "merge feature"])
            .expect("merge feature");

        let backend = CliGitBackend;
        let forward = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Graph { reverse: false },
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("forward graph should load");
        let reverse = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Graph { reverse: true },
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("reverse graph should load");

        assert_eq!(forward.commit_graph_lines.len(), forward.commits.len());
        assert_eq!(reverse.commit_graph_lines.len(), reverse.commits.len());
        assert_eq!(
            forward
                .commits
                .first()
                .map(|commit| commit.summary.as_str()),
            Some("merge feature")
        );
        assert_eq!(
            reverse
                .commits
                .first()
                .map(|commit| commit.summary.as_str()),
            Some("initial")
        );
        assert!(forward
            .commit_graph_lines
            .iter()
            .any(|line| line.contains("merge feature")));
        assert!(forward
            .commit_graph_lines
            .iter()
            .any(|line| line.contains("feature branch commit")));
        assert!(forward
            .commit_graph_lines
            .iter()
            .any(|line| line.contains("main branch commit")));
        assert!(forward
            .commit_graph_lines
            .iter()
            .any(|line| line.contains('*')));
    }

    #[test]
    fn cli_backend_reads_commit_changed_files_and_diff_preview() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        let head_commit = &detail.commits[0];
        assert_eq!(head_commit.short_oid.len(), 7);
        assert!(head_commit
            .changed_files
            .iter()
            .any(|file| file.path == std::path::Path::new("src/lib.rs")
                && file.kind == FileStatusKind::Added));
        assert!(head_commit
            .diff
            .lines
            .iter()
            .any(|line| line.content.contains("src/lib.rs")));
        assert!(head_commit
            .diff
            .lines
            .iter()
            .any(|line| line.content.contains("+pub fn answer() -> u32 {")));
    }

    #[test]
    fn cli_backend_reads_explicit_commit_comparison_diff() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let base = repo.rev_parse("HEAD~1").expect("base commit");
        let target = repo.rev_parse("HEAD").expect("target commit");

        let diff = backend
            .read_diff(DiffRequest {
                repo_id,
                comparison_target: Some(ComparisonTarget::Commit(base)),
                compare_with: Some(ComparisonTarget::Commit(target)),
                selected_path: None,
                diff_presentation: DiffPresentation::Comparison,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("comparison diff should load");

        assert_eq!(diff.presentation, DiffPresentation::Comparison);
        assert!(diff
            .lines
            .iter()
            .any(|line| line.content.contains("src/lib.rs")));
        assert!(diff
            .lines
            .iter()
            .any(|line| line.content.contains("+pub fn answer() -> u32 {")));
    }

    #[test]
    fn cli_backend_honors_whitespace_toggle_and_diff_context() {
        let repo = whitespace_context_repo().expect("fixture repo");
        let baseline = read_diff_text(
            repo.path(),
            None,
            None,
            Some(std::path::Path::new("diff.txt")),
            DiffReadOptions {
                presentation: DiffPresentation::Unstaged,
                ignore_whitespace: false,
                context_lines: 3,
                rename_similarity_threshold: 50,
            },
        )
        .expect("baseline diff should load");
        let ignore_whitespace = read_diff_text(
            repo.path(),
            None,
            None,
            Some(std::path::Path::new("diff.txt")),
            DiffReadOptions {
                presentation: DiffPresentation::Unstaged,
                ignore_whitespace: true,
                context_lines: 3,
                rename_similarity_threshold: 50,
            },
        )
        .expect("whitespace-ignored diff should load");
        let zero_context = read_diff_text(
            repo.path(),
            None,
            None,
            Some(std::path::Path::new("diff.txt")),
            DiffReadOptions {
                presentation: DiffPresentation::Unstaged,
                ignore_whitespace: false,
                context_lines: 0,
                rename_similarity_threshold: 50,
            },
        )
        .expect("zero-context diff should load");

        assert!(baseline.contains("-beta"));
        assert!(baseline.contains("+  beta"));
        assert!(ignore_whitespace.contains("+gamma changed"));
        assert!(!ignore_whitespace.contains("-beta"));
        assert!(!ignore_whitespace.contains("+  beta"));
        assert!(baseline.contains("\n alpha\n"));
        assert!(baseline.contains("\n delta\n"));
        assert!(!zero_context.contains("\n alpha\n"));
        assert!(!zero_context.contains("\n delta\n"));
    }

    #[test]
    fn cli_backend_honors_rename_similarity_threshold() {
        let repo = rename_similarity_diff_repo().expect("fixture repo");
        let detected = read_diff_text(
            repo.path(),
            None,
            None,
            None,
            DiffReadOptions {
                presentation: DiffPresentation::Staged,
                ignore_whitespace: false,
                context_lines: 3,
                rename_similarity_threshold: 50,
            },
        )
        .expect("rename diff should load");
        let suppressed = read_diff_text(
            repo.path(),
            None,
            None,
            None,
            DiffReadOptions {
                presentation: DiffPresentation::Staged,
                ignore_whitespace: false,
                context_lines: 3,
                rename_similarity_threshold: 90,
            },
        )
        .expect("suppressed rename diff should load");

        assert!(detected.contains("rename from old.txt"));
        assert!(detected.contains("rename to new.txt"));
        assert!(!suppressed.contains("rename from old.txt"));
        assert!(suppressed.contains("new file mode 100644"));
        assert!(suppressed.contains("deleted file mode 100644"));
    }

    #[test]
    fn cli_backend_marks_detached_head() {
        let repo = detached_head_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let summary = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
            })
            .expect("summary should load");

        assert_eq!(summary.head_kind, HeadKind::Detached);
    }

    #[test]
    fn cli_backend_reads_worktrees_and_diff_selection() {
        let repo = worktree_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");
        let diff = backend
            .read_diff(DiffRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                comparison_target: None,
                compare_with: None,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("diff should load");

        assert_eq!(detail.worktrees.len(), 2);
        assert!(detail
            .worktrees
            .iter()
            .any(|item| item.branch.as_deref() == Some("main")));
        assert!(detail
            .worktrees
            .iter()
            .any(|item| item.branch.as_deref() == Some("feature")));
        assert!(diff.selected_path.is_none());
        assert_eq!(
            diff.hunk_count,
            diff.lines
                .iter()
                .filter(|line| line.kind == DiffLineKind::HunkHeader)
                .count()
        );
    }

    #[test]
    fn cli_backend_creates_and_removes_worktrees() {
        let repo = worktree_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let worktree_parent = tempfile::tempdir().expect("tempdir");
        let created_path = worktree_parent.path().join("repo-hotfix");
        repo.git(["branch", "hotfix", "main"])
            .expect("create spare branch");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:create-worktree"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::CreateWorktree {
                    path: created_path.clone(),
                    branch_ref: "hotfix".to_string(),
                },
            })
            .expect("create worktree succeeds");

        let after_create = repo.worktree_list().expect("worktree list");
        assert!(after_create.contains(created_path.to_string_lossy().as_ref()));

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:remove-worktree"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::RemoveWorktree {
                    path: created_path.clone(),
                },
            })
            .expect("remove worktree succeeds");

        let after_remove = repo.worktree_list().expect("worktree list");
        assert!(!after_remove.contains(created_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn cli_backend_reads_rebase_in_progress_state() {
        let repo = rebase_in_progress_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::RebaseInProgress);
        assert_eq!(
            detail.rebase_state.as_ref().map(|state| state.kind),
            Some(RebaseKind::Interactive)
        );
        assert_eq!(
            detail
                .rebase_state
                .as_ref()
                .and_then(|state| state.current_summary.as_deref()),
            Some("feature change")
        );
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
    }

    #[test]
    fn cli_backend_starts_interactive_rebase_at_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");
        let original_head = repo.rev_parse("HEAD").expect("original head");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-interactive-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target.clone(),
                    mode: RebaseStartMode::Interactive,
                },
            })
            .expect("interactive rebase should start");

        assert!(outcome.summary.contains("second"));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::RebaseInProgress);
        assert_eq!(
            detail.rebase_state.as_ref().map(|state| state.kind),
            Some(RebaseKind::Interactive)
        );
        assert_eq!(
            detail
                .rebase_state
                .as_ref()
                .and_then(|state| state.current_commit.as_deref()),
            Some(target.as_str())
        );
        assert_eq!(
            detail
                .rebase_state
                .as_ref()
                .and_then(|state| state.current_summary.as_deref()),
            Some("second")
        );
        assert!(detail.rebase_state.as_ref().is_some_and(|state| state
            .todo_preview
            .iter()
            .any(|line| line.contains("add lib"))));
        assert_ne!(
            repo.rev_parse("HEAD").expect("head during rebase"),
            original_head
        );
    }

    #[test]
    fn cli_backend_continues_interactive_rebase_after_edit_stop() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");
        let original_head = repo.rev_parse("HEAD").expect("original head");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-interactive-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Interactive,
                },
            })
            .expect("interactive rebase should start");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:continue-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::ContinueRebase,
            })
            .expect("rebase should continue");

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
        assert!(detail.rebase_state.is_none());
        assert_eq!(repo.rev_parse("HEAD").expect("final head"), original_head);
    }

    #[test]
    fn cli_backend_aborts_interactive_rebase() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");
        let original_head = repo.rev_parse("HEAD").expect("original head");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-interactive-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Interactive,
                },
            })
            .expect("interactive rebase should start");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:abort-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::AbortRebase,
            })
            .expect("rebase abort should succeed");

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
        assert!(detail.rebase_state.is_none());
        assert_eq!(
            repo.rev_parse("HEAD").expect("head after abort"),
            original_head
        );
    }

    #[test]
    fn cli_backend_starts_older_commit_amend_flow() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-amend-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target.clone(),
                    mode: RebaseStartMode::Amend,
                },
            })
            .expect("amend flow should start");

        assert!(outcome.summary.contains("amend flow"));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::RebaseInProgress);
        assert_eq!(
            detail
                .rebase_state
                .as_ref()
                .and_then(|state| state.current_commit.as_deref()),
            Some(target.as_str())
        );
    }

    #[test]
    fn cli_backend_moves_selected_commit_up_in_history() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");
        let adjacent = repo.rev_parse("HEAD").expect("adjacent commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:move-commit-up-rebase"),
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::MoveUp {
                        adjacent_commit: adjacent,
                    },
                },
            })
            .expect("move up should succeed");

        assert!(outcome.summary.contains("up in history"));
        let log = stdout_string(
            repo.git_capture(["log", "--format=%s", "-n", "3"])
                .expect("log"),
        )
        .expect("utf8 log");
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            vec!["second", "add lib", "initial"]
        );
    }

    #[test]
    fn cli_backend_moves_selected_commit_down_in_history() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD").expect("target commit");
        let adjacent = repo.rev_parse("HEAD~1").expect("adjacent commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:move-commit-down-rebase"),
                repo_id,
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::MoveDown {
                        adjacent_commit: adjacent,
                    },
                },
            })
            .expect("move down should succeed");

        assert!(outcome.summary.contains("down in history"));
        let log = stdout_string(
            repo.git_capture(["log", "--format=%s", "-n", "3"])
                .expect("log"),
        )
        .expect("utf8 log");
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            vec!["second", "add lib", "initial"]
        );
    }

    #[test]
    fn cli_backend_fixups_selected_commit_with_autosquash() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.append_file("notes.md", "fixup line\n")
            .expect("append staged fixup");
        repo.stage("notes.md").expect("stage fixup file");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-fixup-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Fixup,
                },
            })
            .expect("fixup flow should succeed");

        assert!(outcome.summary.contains("fixup autosquash"));
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-list", "--count", "HEAD"])
                    .expect("commit count")
            )
            .expect("count"),
            "3"
        );
        assert!(!stdout_string(
            repo.git_capture(["log", "--format=%s", "-n", "3"])
                .expect("log")
        )
        .expect("log text")
        .contains("fixup!"));
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD~1:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("fixup line"));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
    }

    #[test]
    fn cli_backend_creates_fixup_commit_for_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.append_file("notes.md", "fixup line\n")
            .expect("append staged fixup");
        repo.stage("notes.md").expect("stage fixup file");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:create-fixup-commit"),
                repo_id,
                command: GitCommand::CreateFixupCommit { commit: target },
            })
            .expect("create fixup should succeed");

        assert!(outcome.summary.contains("Created fixup commit"));
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-list", "--count", "HEAD"])
                    .expect("commit count")
            )
            .expect("count"),
            "4"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["log", "--format=%s", "-n", "2"])
                    .expect("log")
            )
            .expect("log text"),
            "fixup! second\nadd lib"
        );
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("fixup line"));
    }

    #[test]
    fn cli_backend_applies_existing_fixup_commits_with_autosquash() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.append_file("notes.md", "fixup line\n")
            .expect("append staged fixup");
        repo.stage("notes.md").expect("stage fixup file");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:create-fixup-commit"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateFixupCommit {
                    commit: target.clone(),
                },
            })
            .expect("create fixup should succeed");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:apply-fixups-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::ApplyFixups,
                },
            })
            .expect("apply fixups should succeed");

        assert!(outcome.summary.contains("Applied fixup autosquash"));
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-list", "--count", "HEAD"])
                    .expect("commit count")
            )
            .expect("count"),
            "3"
        );
        assert!(!stdout_string(
            repo.git_capture(["log", "--format=%s", "-n", "3"])
                .expect("log")
        )
        .expect("log text")
        .contains("fixup!"));
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD~1:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("fixup line"));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
    }

    #[test]
    fn cli_backend_squashes_selected_commit_into_parent() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-squash-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Squash,
                },
            })
            .expect("squash flow should succeed");

        assert!(outcome.summary.contains("Squashed"));
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-list", "--count", "HEAD"])
                    .expect("commit count")
            )
            .expect("count"),
            "2"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["log", "--format=%s", "-n", "2"])
                    .expect("log")
            )
            .expect("log text"),
            "add lib\ninitial"
        );
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD~1:history.txt"])
                .expect("show history")
        )
        .expect("history text")
        .contains("two"));
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD~1:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("# Notes"));
    }

    #[test]
    fn cli_backend_drops_selected_commit_from_history() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-drop-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Drop,
                },
            })
            .expect("drop flow should succeed");

        assert!(outcome.summary.contains("Dropped"));
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-list", "--count", "HEAD"])
                    .expect("commit count")
            )
            .expect("count"),
            "2"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["log", "--format=%s", "-n", "2"])
                    .expect("log")
            )
            .expect("log text"),
            "add lib\ninitial"
        );
        let err = repo
            .git_capture(["show", "HEAD~1:notes.md"])
            .expect_err("dropped commit should remove notes file");
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "HEAD~1:history.txt"])
                    .expect("show history")
            )
            .expect("history text"),
            "one"
        );
    }

    #[test]
    fn cli_backend_rewords_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-reword-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::Reword {
                        message: "second rewritten".to_string(),
                    },
                },
            })
            .expect("reword should succeed");

        let log = stdout_string(
            repo.git_capture(["log", "--format=%s", "-n", "3"])
                .expect("log"),
        )
        .expect("log text");
        assert!(log.contains("second rewritten"));
        assert!(!log.contains("second\ninitial"));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
        assert!(detail.rebase_state.is_none());
    }

    #[test]
    fn cli_backend_cherry_picks_selected_commit() {
        let (repo, commit) = cherry_pick_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:cherry-pick"),
                repo_id: repo_id.clone(),
                command: GitCommand::CherryPickCommit {
                    commit: commit.clone(),
                },
            })
            .expect("cherry-pick should succeed");

        assert_eq!(outcome.repo_id, repo_id.clone());
        assert!(outcome.summary.contains("feature change"));
        assert_eq!(
            std::fs::read_to_string(repo.path().join("feature.txt")).expect("read feature.txt"),
            "feature\n"
        );
        assert_eq!(repo.status_porcelain().expect("status"), "");

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
    }

    #[test]
    fn cli_backend_reverts_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let commit = repo.rev_parse("HEAD").expect("head commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:revert"),
                repo_id: repo_id.clone(),
                command: GitCommand::RevertCommit { commit },
            })
            .expect("revert should succeed");

        assert_eq!(outcome.repo_id, repo_id.clone());
        assert!(outcome.summary.contains("add lib"));
        assert!(!repo.path().join("src/lib.rs").exists());
        assert_eq!(repo.status_porcelain().expect("status"), "");

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
    }

    #[test]
    fn cli_backend_reads_cherry_pick_in_progress_state_after_conflict() {
        let (repo, commit) = cherry_pick_conflict_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let error = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:cherry-pick-conflict"),
                repo_id: repo_id.clone(),
                command: GitCommand::CherryPickCommit { commit },
            })
            .expect_err("cherry-pick should conflict");

        assert!(matches!(error, GitError::OperationFailed { .. }));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::CherryPickInProgress);
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
    }

    #[test]
    fn cli_backend_reads_revert_in_progress_state_after_conflict() {
        let (repo, commit) = revert_conflict_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let error = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:revert-conflict"),
                repo_id: repo_id.clone(),
                command: GitCommand::RevertCommit { commit },
            })
            .expect_err("revert should conflict");

        assert!(matches!(error, GitError::OperationFailed { .. }));

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::RevertInProgress);
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
    }

    #[test]
    fn cli_backend_skips_conflicting_rebase_step() {
        let repo = rebase_in_progress_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:skip-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::SkipRebase,
            })
            .expect("rebase skip should succeed");

        assert_eq!(outcome.summary, "Skipped current rebase step");

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::None);
        assert!(detail.rebase_state.is_none());
        assert_eq!(
            std::fs::read_to_string(repo.path().join("rebase.txt")).expect("read rebase.txt"),
            "main\n"
        );
        assert_eq!(repo.current_branch().expect("current branch"), "feature");
    }

    #[test]
    fn cli_backend_pull_requires_upstream_tracking_branch() {
        let repo = clean_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let error = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-pull-no-upstream"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::PullCurrentBranch,
            })
            .expect_err("pull without upstream should fail");

        assert_eq!(
            error,
            GitError::OperationFailed {
                message: "pull requires an upstream tracking branch".to_string(),
            }
        );
    }

    #[test]
    fn cli_backend_fetch_surfaces_transport_failure() {
        let repo = clean_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let missing_remote = repo.path().join("missing-remote.git");
        let remote = missing_remote.display().to_string();
        repo.git(["remote", "add", "origin", remote.as_str()])
            .expect("add remote");

        let error = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-fetch-missing-remote"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::FetchSelectedRepo,
            })
            .expect_err("fetch against a missing remote should fail");

        assert!(
            matches!(error, GitError::OperationFailed { message } if message.contains("missing-remote.git"))
        );
    }

    #[test]
    fn cli_backend_push_requires_attached_branch_head() {
        let repo = detached_head_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let error = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-push-detached"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::PushCurrentBranch,
            })
            .expect_err("push from detached HEAD should fail");

        assert_eq!(
            error,
            GitError::OperationFailed {
                message: "push requires an attached branch HEAD".to_string(),
            }
        );
    }

    #[test]
    fn cli_backend_summary_rejects_missing_repo() {
        let backend = CliGitBackend;
        let repo_id = RepoId::new("/tmp/definitely-missing-super-lazygit-repo".to_string());

        let error = backend
            .read_repo_summary(RepoSummaryRequest {
                repo_id: repo_id.clone(),
            })
            .expect_err("missing repo should fail");

        assert_eq!(error, GitError::RepoNotFound { repo_id });
    }

    #[test]
    fn cli_backend_stages_single_selected_file() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-stage-file"),
                repo_id: repo_id.clone(),
                command: GitCommand::StageFile {
                    path: PathBuf::from("untracked.txt"),
                },
            })
            .expect("file staging should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Staged untracked.txt");
        assert!(repo
            .status_porcelain()
            .expect("status")
            .contains("A  untracked.txt"));
    }

    #[test]
    fn cli_backend_commits_staged_changes_without_verify() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        repo.stage("staged.txt").expect("stage staged.txt");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-commit-staged-no-verify"),
                repo_id: repo_id.clone(),
                command: GitCommand::CommitStagedNoVerify {
                    message: "ship without hooks".to_string(),
                },
            })
            .expect("no-verify commit should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(
            outcome.summary,
            "Committed staged changes without hooks: ship without hooks"
        );
        assert_eq!(
            String::from_utf8_lossy(
                &repo
                    .git_capture(["show", "-s", "--format=%s", "HEAD"])
                    .expect("head subject")
                    .stdout
            )
            .trim(),
            "ship without hooks"
        );
    }

    #[test]
    fn cli_backend_runs_branch_lifecycle_commands() {
        let repo = upstream_diverged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let created = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranch {
                    branch_name: "feature".to_string(),
                },
            })
            .expect("branch creation should succeed");
        assert_eq!(created.summary, "Created and checked out feature");
        assert_eq!(repo.current_branch().expect("current branch"), "feature");

        let upstream = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-set-upstream"),
                repo_id: repo_id.clone(),
                command: GitCommand::SetBranchUpstream {
                    branch_name: "feature".to_string(),
                    upstream_ref: "origin/main".to_string(),
                },
            })
            .expect("set upstream should succeed");
        assert_eq!(upstream.summary, "Set upstream for feature to origin/main");
        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: repo_id.clone(),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should refresh");
        assert!(detail
            .branches
            .iter()
            .any(|branch| branch.name == "feature"
                && branch.upstream.as_deref() == Some("origin/main")));

        let renamed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-rename-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::RenameBranch {
                    branch_name: "feature".to_string(),
                    new_name: "topic".to_string(),
                },
            })
            .expect("branch rename should succeed");
        assert_eq!(renamed.summary, "Renamed feature to topic");
        assert_eq!(repo.current_branch().expect("current branch"), "topic");

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-main"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutBranch {
                    branch_ref: "main".to_string(),
                },
            })
            .expect("checkout main should succeed");

        let deleted = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-delete-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::DeleteBranch {
                    branch_name: "topic".to_string(),
                },
            })
            .expect("branch delete should succeed");
        assert_eq!(deleted.summary, "Deleted topic");
        let branch_list = stdout_string(
            repo.git_capture(["branch", "--list"])
                .expect("branch list should load"),
        )
        .expect("branch output");
        assert!(!branch_list.contains("topic"));
    }

    #[test]
    fn cli_backend_reads_remote_branches_and_omits_symbolic_head() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("push main");
        seed.checkout_new_branch("feature")
            .expect("create feature branch");
        seed.write_file("feature.txt", "remote feature\n")
            .expect("write feature file");
        seed.commit_all("remote feature").expect("feature commit");
        seed.push("origin", "HEAD:feature").expect("push feature");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let remote_names = detail
            .remote_branches
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(remote_names, vec!["origin/feature", "origin/main"]);
        assert!(!remote_names.contains(&"origin/HEAD"));
    }

    #[test]
    fn cli_backend_runs_remote_branch_lifecycle_commands() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("push main");
        seed.checkout_new_branch("feature")
            .expect("create feature branch");
        seed.write_file("feature.txt", "remote feature\n")
            .expect("write feature file");
        seed.commit_all("remote feature").expect("feature commit");
        seed.push("origin", "HEAD:feature").expect("push feature");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let created = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-branch-from-ref"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranchFromRef {
                    branch_name: "feature-copy".to_string(),
                    start_point: "origin/feature".to_string(),
                },
            })
            .expect("create branch from remote should succeed");
        assert_eq!(
            created.summary,
            "Created and checked out feature-copy from origin/feature"
        );
        assert_eq!(
            repo.current_branch().expect("current branch"),
            "feature-copy"
        );

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-main"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutBranch {
                    branch_ref: "main".to_string(),
                },
            })
            .expect("checkout main should succeed");

        let checkout = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-remote-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutRemoteBranch {
                    remote_branch_ref: "origin/feature".to_string(),
                    local_branch_name: "feature".to_string(),
                },
            })
            .expect("remote checkout should succeed");
        assert_eq!(
            checkout.summary,
            "Created and checked out feature tracking origin/feature"
        );
        assert_eq!(repo.current_branch().expect("current branch"), "feature");
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
                    .expect("upstream ref"),
            )
            .expect("upstream text"),
            "origin/feature"
        );

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-main-again"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutBranch {
                    branch_ref: "main".to_string(),
                },
            })
            .expect("checkout main before delete should succeed");

        let deleted = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-delete-remote-branch"),
                repo_id,
                command: GitCommand::DeleteRemoteBranch {
                    remote_name: "origin".to_string(),
                    branch_name: "feature".to_string(),
                },
            })
            .expect("remote delete should succeed");
        assert_eq!(deleted.summary, "Deleted remote branch origin/feature");
        remote
            .git_expect_failure(["show-ref", "--verify", "--quiet", "refs/heads/feature"])
            .expect("remote branch should be deleted");
    }

    #[test]
    fn cli_backend_reads_remotes_with_metadata_and_branch_counts() {
        let origin = TempRepo::bare().expect("origin fixture");
        let mirror = TempRepo::bare().expect("mirror fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", origin.path())
            .expect("attach origin");
        seed.push("origin", "HEAD:main").expect("push main");
        seed.checkout_new_branch("feature")
            .expect("create feature branch");
        seed.write_file("feature.txt", "remote feature\n")
            .expect("write feature file");
        seed.commit_all("remote feature").expect("feature commit");
        seed.push("origin", "HEAD:feature").expect("push feature");

        let repo = TempRepo::clone_from(origin.path()).expect("clone fixture");
        repo.add_remote("mirror", mirror.path())
            .expect("attach mirror");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let origin_remote = detail
            .remotes
            .iter()
            .find(|remote| remote.name == "origin")
            .expect("origin remote present");
        assert_eq!(origin_remote.branch_count, 2);
        assert_eq!(
            origin_remote.fetch_url,
            stdout_string(
                repo.git_capture(["remote", "get-url", "origin"])
                    .expect("origin fetch url")
            )
            .expect("origin fetch output")
        );
        assert_eq!(
            origin_remote.push_url,
            stdout_string(
                repo.git_capture(["remote", "get-url", "--push", "origin"])
                    .expect("origin push url")
            )
            .expect("origin push output")
        );

        let mirror_remote = detail
            .remotes
            .iter()
            .find(|remote| remote.name == "mirror")
            .expect("mirror remote present");
        assert_eq!(mirror_remote.branch_count, 0);
        assert_eq!(
            mirror_remote.fetch_url,
            stdout_string(
                repo.git_capture(["remote", "get-url", "mirror"])
                    .expect("mirror fetch url")
            )
            .expect("mirror fetch output")
        );
    }

    #[test]
    fn cli_backend_runs_remote_management_commands() {
        let origin = TempRepo::bare().expect("origin fixture");
        let mirror = TempRepo::bare().expect("mirror fixture");
        let replacement = TempRepo::bare().expect("replacement fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", origin.path())
            .expect("attach origin");
        seed.push("origin", "HEAD:main").expect("push main");

        let repo = TempRepo::clone_from(origin.path()).expect("clone fixture");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let added = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-add-remote"),
                repo_id: repo_id.clone(),
                command: GitCommand::AddRemote {
                    remote_name: "mirror".to_string(),
                    remote_url: mirror.path().display().to_string(),
                },
            })
            .expect("add remote should succeed");
        assert_eq!(added.summary, "Added remote mirror");
        assert_eq!(
            stdout_string(
                repo.git_capture(["remote", "get-url", "mirror"])
                    .expect("mirror fetch url")
            )
            .expect("mirror fetch output"),
            mirror.path().display().to_string()
        );

        let edited = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-edit-remote"),
                repo_id: repo_id.clone(),
                command: GitCommand::EditRemote {
                    current_name: "mirror".to_string(),
                    new_name: "upstream".to_string(),
                    remote_url: replacement.path().display().to_string(),
                },
            })
            .expect("edit remote should succeed");
        assert_eq!(edited.summary, "Updated remote mirror");
        repo.git_expect_failure(["remote", "get-url", "mirror"])
            .expect("old remote name should be gone");
        assert_eq!(
            stdout_string(
                repo.git_capture(["remote", "get-url", "upstream"])
                    .expect("upstream fetch url")
            )
            .expect("upstream fetch output"),
            replacement.path().display().to_string()
        );

        let before_fetch = stdout_string(
            repo.git_capture(["rev-parse", "refs/remotes/origin/main"])
                .expect("origin remote ref before fetch"),
        )
        .expect("origin remote ref output before fetch");
        seed.write_file("tracked.txt", "base\nupdated\n")
            .expect("write updated tracked file");
        seed.commit_all("second").expect("second commit");
        let latest_origin_main = seed.rev_parse("HEAD").expect("latest origin main");
        seed.push("origin", "HEAD:main").expect("push updated main");

        let fetched = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-fetch-remote"),
                repo_id: repo_id.clone(),
                command: GitCommand::FetchRemote {
                    remote_name: "origin".to_string(),
                },
            })
            .expect("fetch remote should succeed");
        assert_eq!(fetched.summary, "Fetched origin");
        let after_fetch = stdout_string(
            repo.git_capture(["rev-parse", "refs/remotes/origin/main"])
                .expect("origin remote ref after fetch"),
        )
        .expect("origin remote ref output after fetch");
        assert_ne!(before_fetch, after_fetch);
        assert_eq!(after_fetch, latest_origin_main);

        let removed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-remove-remote"),
                repo_id,
                command: GitCommand::RemoveRemote {
                    remote_name: "upstream".to_string(),
                },
            })
            .expect("remove remote should succeed");
        assert_eq!(removed.summary, "Removed remote upstream");
        repo.git_expect_failure(["remote", "get-url", "upstream"])
            .expect("upstream remote should be deleted");
    }

    #[test]
    fn cli_backend_reads_lightweight_and_annotated_tags() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        let annotated_target = repo.rev_parse("HEAD").expect("annotated target");
        repo.git(["tag", "-a", "v1.0.0", "-m", "release v1.0.0"])
            .expect("create annotated tag");
        repo.write_file("tracked.txt", "base\nsnapshot\n")
            .expect("write second commit");
        repo.commit_all("snapshot commit").expect("snapshot commit");
        let lightweight_target = repo.rev_parse("HEAD").expect("lightweight target");
        repo.git(["tag", "snapshot"])
            .expect("create lightweight tag");

        let backend = CliGitBackend;
        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let annotated = detail
            .tags
            .iter()
            .find(|tag| tag.name == "v1.0.0")
            .expect("annotated tag present");
        assert!(annotated.annotated);
        assert_eq!(annotated.summary, "release v1.0.0");
        assert_eq!(annotated.target_oid, annotated_target);
        assert_eq!(
            annotated.target_short_oid,
            annotated_target.chars().take(7).collect::<String>()
        );

        let lightweight = detail
            .tags
            .iter()
            .find(|tag| tag.name == "snapshot")
            .expect("lightweight tag present");
        assert!(!lightweight.annotated);
        assert_eq!(lightweight.summary, "snapshot commit");
        assert_eq!(lightweight.target_oid, lightweight_target);
        assert_eq!(
            lightweight.target_short_oid,
            lightweight_target.chars().take(7).collect::<String>()
        );
    }

    #[test]
    fn cli_backend_runs_tag_lifecycle_commands() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("push main");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let created = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-tag"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateTag {
                    tag_name: "release-candidate".to_string(),
                },
            })
            .expect("create tag should succeed");
        assert_eq!(created.summary, "Created tag release-candidate");
        assert_eq!(
            stdout_string(
                repo.git_capture(["show-ref", "--verify", "refs/tags/release-candidate"])
                    .expect("local tag ref"),
            )
            .expect("local tag output")
            .lines()
            .count(),
            1
        );

        let pushed = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-push-tag"),
                repo_id: repo_id.clone(),
                command: GitCommand::PushTag {
                    remote_name: "origin".to_string(),
                    tag_name: "release-candidate".to_string(),
                },
            })
            .expect("push tag should succeed");
        assert_eq!(pushed.summary, "Pushed tag release-candidate to origin");
        assert_eq!(
            stdout_string(
                remote
                    .git_capture(["show-ref", "--verify", "refs/tags/release-candidate"])
                    .expect("remote tag ref"),
            )
            .expect("remote tag output")
            .lines()
            .count(),
            1
        );

        let deleted = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-delete-tag"),
                repo_id: repo_id.clone(),
                command: GitCommand::DeleteTag {
                    tag_name: "release-candidate".to_string(),
                },
            })
            .expect("delete tag should succeed");
        assert_eq!(deleted.summary, "Deleted tag release-candidate");
        repo.git_expect_failure([
            "show-ref",
            "--verify",
            "--quiet",
            "refs/tags/release-candidate",
        ])
        .expect("local tag should be deleted");
    }

    #[test]
    fn cli_backend_creates_tag_from_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target_commit = repo.rev_parse("HEAD~1").expect("target commit");
        let head_commit = repo.rev_parse("HEAD").expect("head commit");

        let created = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-tag-from-commit"),
                repo_id,
                command: GitCommand::CreateTagFromCommit {
                    tag_name: "release-prev".to_string(),
                    commit: target_commit.clone(),
                },
            })
            .expect("create tag from commit should succeed");

        assert_eq!(
            created.summary,
            format!("Created tag release-prev at {target_commit}")
        );
        assert_eq!(
            repo.rev_parse("refs/tags/release-prev^{commit}")
                .expect("tag target"),
            target_commit
        );
        assert_ne!(
            repo.rev_parse("refs/tags/release-prev^{commit}")
                .expect("tag target"),
            head_commit
        );
    }

    #[test]
    fn cli_backend_checkout_tag_detaches_head() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.git(["tag", "snapshot"]).expect("create tag");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let checkout = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-tag"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutTag {
                    tag_name: "snapshot".to_string(),
                },
            })
            .expect("checkout tag should succeed");

        assert_eq!(checkout.repo_id, repo_id);
        assert_eq!(checkout.summary, "Checked out tag snapshot");
        assert_eq!(
            stdout_string(
                repo.git_capture(["rev-parse", "--abbrev-ref", "HEAD"])
                    .expect("head name"),
            )
            .expect("head text"),
            "HEAD"
        );
    }

    #[test]
    fn cli_backend_unstages_single_selected_file() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-unstage-file"),
                repo_id: repo_id.clone(),
                command: GitCommand::UnstageFile {
                    path: PathBuf::from("staged.txt"),
                },
            })
            .expect("file unstaging should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Unstaged staged.txt");
        assert!(repo
            .status_porcelain()
            .expect("status")
            .contains("?? staged.txt"));
    }

    #[test]
    fn cli_backend_discards_unstaged_tracked_file_changes() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-discard-tracked"),
                repo_id: repo_id.clone(),
                command: GitCommand::DiscardFile {
                    path: PathBuf::from("tracked.txt"),
                },
            })
            .expect("discard tracked file should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Discarded changes for tracked.txt");
        assert!(!repo
            .status_porcelain()
            .expect("status")
            .contains(" M tracked.txt"));
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).expect("tracked.txt"),
            "base\n"
        );
    }

    #[test]
    fn cli_backend_discards_staged_new_file() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-discard-staged-new"),
                repo_id: repo_id.clone(),
                command: GitCommand::DiscardFile {
                    path: PathBuf::from("staged.txt"),
                },
            })
            .expect("discard staged file should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Discarded changes for staged.txt");
        assert!(!repo
            .status_porcelain()
            .expect("status")
            .contains("staged.txt"));
        assert!(!repo.path().join("staged.txt").exists());
    }

    #[test]
    fn cli_backend_discards_untracked_file() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-discard-untracked"),
                repo_id: repo_id.clone(),
                command: GitCommand::DiscardFile {
                    path: PathBuf::from("untracked.txt"),
                },
            })
            .expect("discard untracked file should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Discarded changes for untracked.txt");
        assert!(!repo
            .status_porcelain()
            .expect("status")
            .contains("untracked.txt"));
        assert!(!repo.path().join("untracked.txt").exists());
    }

    #[test]
    fn cli_backend_soft_resets_to_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target oid");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-reset-soft"),
                repo_id: repo_id.clone(),
                command: GitCommand::ResetToCommit {
                    mode: ResetMode::Soft,
                    target: target.clone(),
                },
            })
            .expect("soft reset should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Soft reset"));
        assert_eq!(repo.rev_parse("HEAD").expect("head"), target);
        assert!(repo
            .status_porcelain()
            .expect("status")
            .contains("A  src/lib.rs"));
    }

    #[test]
    fn cli_backend_mixed_resets_to_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target oid");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-reset-mixed"),
                repo_id: repo_id.clone(),
                command: GitCommand::ResetToCommit {
                    mode: ResetMode::Mixed,
                    target: target.clone(),
                },
            })
            .expect("mixed reset should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Mixed reset"));
        assert_eq!(repo.rev_parse("HEAD").expect("head"), target);
        assert!(repo.status_porcelain().expect("status").contains("?? src/"));
    }

    #[test]
    fn cli_backend_hard_resets_to_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target oid");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-reset-hard"),
                repo_id: repo_id.clone(),
                command: GitCommand::ResetToCommit {
                    mode: ResetMode::Hard,
                    target: target.clone(),
                },
            })
            .expect("hard reset should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Hard reset"));
        assert_eq!(repo.rev_parse("HEAD").expect("head"), target);
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert!(!repo.path().join("src/lib.rs").exists());
    }

    #[test]
    fn cli_backend_restores_head_to_selected_reflog_entry() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let expected_target = repo.rev_parse("HEAD~1").expect("target oid");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-restore-snapshot"),
                repo_id: repo_id.clone(),
                command: GitCommand::RestoreSnapshot {
                    target: "HEAD@{1}".to_string(),
                },
            })
            .expect("restore snapshot should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Restored HEAD to HEAD@{1}");
        assert_eq!(repo.rev_parse("HEAD").expect("head"), expected_target);
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert!(!repo.path().join("src/lib.rs").exists());
    }

    #[test]
    fn cli_backend_nukes_working_tree() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("job-nuke-working-tree"),
                repo_id: repo_id.clone(),
                command: GitCommand::NukeWorkingTree,
            })
            .expect("nuke should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Discarded all local changes");
        assert_eq!(repo.status_porcelain().expect("status"), "");
        assert!(!repo.path().join("staged.txt").exists());
        assert!(!repo.path().join("untracked.txt").exists());
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).expect("tracked.txt"),
            "base\n"
        );
    }

    #[test]
    fn cli_backend_stages_selected_hunk_from_unstaged_diff() {
        let repo = multi_hunk_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let diff = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("unstaged diff should load");
        let parsed = parse_patch(&diff).expect("diff should contain hunks");

        let outcome = backend
            .apply_patch_selection(PatchSelectionRequest {
                repo_id: repo_id.clone(),
                path: PathBuf::from("multi.txt"),
                mode: PatchApplicationMode::Stage,
                hunks: vec![parsed.hunks[0].selection],
            })
            .expect("patch selection should stage first hunk");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Staged 1 selected hunk(s)"));
        assert_eq!(repo.status_porcelain().expect("status"), "MM multi.txt");

        let staged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("cached diff should load");
        let unstaged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("worktree diff should load");

        assert!(staged.contains("+two staged"));
        assert!(!staged.contains("+five staged"));
        assert!(unstaged.contains("+five staged"));
        assert!(!unstaged.contains("+two staged"));
    }

    #[test]
    fn cli_backend_unstages_selected_hunk_from_index_diff() {
        let repo = multi_hunk_repo().expect("fixture repo");
        repo.stage("multi.txt").expect("stage file");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let diff = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("cached diff should load");
        let parsed = parse_patch(&diff).expect("diff should contain hunks");

        let outcome = backend
            .apply_patch_selection(PatchSelectionRequest {
                repo_id: repo_id.clone(),
                path: PathBuf::from("multi.txt"),
                mode: PatchApplicationMode::Unstage,
                hunks: vec![parsed.hunks[0].selection],
            })
            .expect("patch selection should unstage first hunk");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Unstaged 1 selected hunk(s)"));
        assert_eq!(repo.status_porcelain().expect("status"), "MM multi.txt");

        let staged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("cached diff should load");
        let unstaged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("worktree diff should load");

        assert!(staged.contains("+five staged"));
        assert!(!staged.contains("+two staged"));
        assert!(unstaged.contains("+two staged"));
        assert!(!unstaged.contains("+five staged"));
    }

    #[test]
    fn cli_backend_stages_selected_partial_lines_from_unstaged_diff() {
        let repo = multi_line_partial_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .apply_patch_selection(PatchSelectionRequest {
                repo_id: repo_id.clone(),
                path: PathBuf::from("multi.txt"),
                mode: PatchApplicationMode::Stage,
                hunks: vec![SelectedHunk {
                    old_start: 3,
                    old_lines: 1,
                    new_start: 3,
                    new_lines: 1,
                }],
            })
            .expect("patch selection should stage selected line");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Staged 1 selected hunk(s)"));
        assert_eq!(repo.status_porcelain().expect("status"), "MM multi.txt");

        let staged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("cached diff should load");
        let unstaged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("worktree diff should load");

        assert!(staged.contains("+three staged"));
        assert!(!staged.contains("+two staged"));
        assert!(!staged.contains("+four staged"));
        assert!(unstaged.contains("+two staged"));
        assert!(!unstaged.contains("+three staged"));
        assert!(unstaged.contains("+four staged"));
    }

    #[test]
    fn cli_backend_unstages_selected_partial_lines_from_index_diff() {
        let repo = multi_line_partial_repo().expect("fixture repo");
        repo.stage("multi.txt").expect("stage file");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .apply_patch_selection(PatchSelectionRequest {
                repo_id: repo_id.clone(),
                path: PathBuf::from("multi.txt"),
                mode: PatchApplicationMode::Unstage,
                hunks: vec![SelectedHunk {
                    old_start: 3,
                    old_lines: 2,
                    new_start: 3,
                    new_lines: 2,
                }],
            })
            .expect("patch selection should unstage selected lines");

        assert_eq!(outcome.repo_id, repo_id);
        assert!(outcome.summary.contains("Unstaged 1 selected hunk(s)"));
        assert_eq!(repo.status_porcelain().expect("status"), "MM multi.txt");

        let staged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--cached",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("cached diff should load");
        let unstaged = git_stdout_raw(
            repo.path(),
            [
                "diff",
                "--no-ext-diff",
                "--binary",
                "--unified=0",
                "--",
                "multi.txt",
            ],
        )
        .expect("worktree diff should load");

        assert!(staged.contains("+two staged"));
        assert!(!staged.contains("+three staged"));
        assert!(!staged.contains("+four staged"));
        assert!(!unstaged.contains("+two staged"));
        assert!(unstaged.contains("+three staged"));
        assert!(unstaged.contains("+four staged"));
    }

    #[test]
    fn cli_backend_reads_submodule_inventory() {
        let repo = submodule_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let submodule = detail
            .submodules
            .iter()
            .find(|item| item.path == Path::new("vendor/child-module"))
            .expect("fixture submodule is listed");

        assert_eq!(submodule.name, "vendor/child-module");
        assert!(submodule.initialized);
        assert_eq!(submodule.branch.as_deref(), Some("main"));
        assert!(submodule.short_oid.is_some());
    }

    #[test]
    fn cli_backend_runs_submodule_lifecycle_commands() {
        let repo = submodule_repo().expect("fixture repo");
        let extra_repo = temp_repo().expect("extra repo");
        extra_repo
            .write_file("extra.txt", "extra\n")
            .expect("write extra file");
        extra_repo
            .commit_all("extra init")
            .expect("commit extra repo");
        let alternate_repo = temp_repo().expect("alternate repo");
        alternate_repo
            .write_file("alt.txt", "alt\n")
            .expect("write alternate file");
        alternate_repo
            .commit_all("alternate init")
            .expect("commit alternate repo");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let base_request = RepoDetailRequest {
            repo_id: repo_id.clone(),
            selected_path: None,
            diff_presentation: DiffPresentation::Unstaged,
            commit_ref: None,
            commit_history_mode: CommitHistoryMode::Linear,
            ignore_whitespace_in_diff: false,
            diff_context_lines: 3,
            rename_similarity_threshold: 50,
        };

        let added = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:test:add-submodule"),
                repo_id: repo_id.clone(),
                command: GitCommand::AddSubmodule {
                    path: PathBuf::from("vendor/extra"),
                    url: extra_repo.path().display().to_string(),
                },
            })
            .expect("add submodule succeeds");
        assert!(added.summary.contains("Added submodule vendor/extra"));

        let detail = backend
            .read_repo_detail(base_request.clone())
            .expect("detail after add succeeds");
        let extra = detail
            .submodules
            .iter()
            .find(|item| item.path == Path::new("vendor/extra"))
            .expect("new submodule exists after add");
        assert_eq!(extra.url, extra_repo.path().display().to_string());
        assert!(extra.initialized);

        let edited = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:test:edit-submodule"),
                repo_id: repo_id.clone(),
                command: GitCommand::EditSubmoduleUrl {
                    name: "vendor/extra".to_string(),
                    path: PathBuf::from("vendor/extra"),
                    url: alternate_repo.path().display().to_string(),
                },
            })
            .expect("edit submodule succeeds");
        assert!(edited
            .summary
            .contains("Updated submodule vendor/extra URL"));

        let detail = backend
            .read_repo_detail(base_request.clone())
            .expect("detail after edit succeeds");
        let extra = detail
            .submodules
            .iter()
            .find(|item| item.path == Path::new("vendor/extra"))
            .expect("edited submodule exists");
        assert_eq!(extra.url, alternate_repo.path().display().to_string());

        repo.git(["submodule", "deinit", "-f", "--", "vendor/extra"])
            .expect("deinit submodule");
        let detail = backend
            .read_repo_detail(base_request.clone())
            .expect("detail after deinit succeeds");
        let extra = detail
            .submodules
            .iter()
            .find(|item| item.path == Path::new("vendor/extra"))
            .expect("deinitialized submodule still listed");
        assert!(!extra.initialized);

        let initialized = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:test:init-submodule"),
                repo_id: repo_id.clone(),
                command: GitCommand::InitSubmodule {
                    path: PathBuf::from("vendor/extra"),
                },
            })
            .expect("init submodule succeeds");
        assert!(initialized
            .summary
            .contains("Initialized submodule vendor/extra"));

        let detail = backend
            .read_repo_detail(base_request.clone())
            .expect("detail after init succeeds");
        let extra = detail
            .submodules
            .iter()
            .find(|item| item.path == Path::new("vendor/extra"))
            .expect("initialized submodule exists");
        assert!(extra.initialized);

        let updated = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:test:update-submodule"),
                repo_id: repo_id.clone(),
                command: GitCommand::UpdateSubmodule {
                    path: PathBuf::from("vendor/extra"),
                },
            })
            .expect("update submodule succeeds");
        assert!(updated.summary.contains("Updated submodule vendor/extra"));

        let removed = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:test:remove-submodule"),
                repo_id: repo_id.clone(),
                command: GitCommand::RemoveSubmodule {
                    path: PathBuf::from("vendor/extra"),
                },
            })
            .expect("remove submodule succeeds");
        assert!(removed.summary.contains("Removed submodule vendor/extra"));

        let detail = backend
            .read_repo_detail(base_request)
            .expect("detail after remove succeeds");
        assert!(detail
            .submodules
            .iter()
            .all(|item| item.path != Path::new("vendor/extra")));
    }

    fn multi_hunk_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("multi.txt", "one\ntwo\nthree\nfour\nfive\nsix\n")?;
        repo.commit_all("initial")?;
        repo.write_file(
            "multi.txt",
            "one\ntwo staged\nthree\nfour\nfive staged\nsix\n",
        )?;
        Ok(repo)
    }

    fn multi_line_partial_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("multi.txt", "one\ntwo\nthree\nfour\nfive\n")?;
        repo.commit_all("initial")?;
        repo.write_file(
            "multi.txt",
            "one\ntwo staged\nthree staged\nfour staged\nfive\n",
        )?;
        Ok(repo)
    }

    fn mixed_staged_and_unstaged_file_repo() -> std::io::Result<super_lazygit_test_support::TempRepo>
    {
        let repo = temp_repo()?;
        repo.write_file("mixed.txt", "1\n2\n3\n")?;
        repo.commit_all("initial")?;
        repo.write_file("mixed.txt", "1 staged\n2\n3\n")?;
        repo.stage("mixed.txt")?;
        repo.write_file("mixed.txt", "1 staged\n2 unstaged\n3\n")?;
        Ok(repo)
    }

    fn renameable_stash_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("file.txt", "base\n")?;
        repo.commit_all("initial")?;
        repo.write_file("file.txt", "change to stash1\n")?;
        repo.git(["stash", "push", "-m", "foo"])?;
        repo.write_file("file.txt", "change to stash2\n")?;
        repo.git(["stash", "push", "-m", "bar"])?;
        Ok(repo)
    }

    fn stash_inventory_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("modified.txt", "base\n")?;
        repo.write_file("renamed-before.txt", "base\n")?;
        repo.write_file("deleted.txt", "base\n")?;
        repo.commit_all("initial")?;

        repo.write_file("modified.txt", "changed\n")?;
        repo.git(["mv", "renamed-before.txt", "renamed-after.txt"])?;
        fs::remove_file(repo.path().join("deleted.txt"))?;
        repo.git(["stash", "push", "-m", "inventory"])?;

        Ok(repo)
    }

    fn cherry_pick_repo() -> std::io::Result<(super_lazygit_test_support::TempRepo, String)> {
        let repo = temp_repo()?;
        repo.write_file("shared.txt", "base\n")?;
        repo.commit_all("initial")?;

        repo.checkout_new_branch("feature")?;
        repo.write_file("feature.txt", "feature\n")?;
        repo.commit_all("feature change")?;
        let commit = repo.rev_parse("HEAD")?;

        repo.checkout("main")?;
        repo.write_file("main.txt", "main\n")?;
        repo.commit_all("main change")?;

        Ok((repo, commit))
    }

    fn cherry_pick_conflict_repo() -> std::io::Result<(super_lazygit_test_support::TempRepo, String)>
    {
        let repo = temp_repo()?;
        repo.write_file("conflict.txt", "base\n")?;
        repo.commit_all("initial")?;

        repo.checkout_new_branch("feature")?;
        repo.write_file("conflict.txt", "feature\n")?;
        repo.commit_all("feature change")?;
        let commit = repo.rev_parse("HEAD")?;

        repo.checkout("main")?;
        repo.write_file("conflict.txt", "main\n")?;
        repo.commit_all("main change")?;

        Ok((repo, commit))
    }

    fn revert_conflict_repo() -> std::io::Result<(super_lazygit_test_support::TempRepo, String)> {
        let repo = temp_repo()?;
        repo.write_file("conflict.txt", "base\n")?;
        repo.commit_all("initial")?;

        repo.write_file("conflict.txt", "feature\n")?;
        repo.commit_all("feature change")?;
        let commit = repo.rev_parse("HEAD")?;

        repo.write_file("conflict.txt", "main\n")?;
        repo.commit_all("main change")?;

        Ok((repo, commit))
    }

    fn whitespace_context_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("diff.txt", "alpha\nbeta\ngamma\ndelta\nepsilon\n")?;
        repo.commit_all("initial")?;
        repo.write_file("diff.txt", "alpha\n  beta\ngamma changed\ndelta\nepsilon\n")?;
        Ok(repo)
    }

    fn rename_similarity_diff_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file(
            "old.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        )?;
        repo.commit_all("initial")?;
        repo.git(["mv", "old.txt", "new.txt"])?;
        repo.write_file(
            "new.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nSEVEN\nEIGHT\nNINE\nTEN\n",
        )?;
        repo.git(["add", "-A"])?;
        Ok(repo)
    }
}
