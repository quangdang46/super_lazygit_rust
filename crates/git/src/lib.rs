use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super_lazygit_core::{
    BranchItem, CommitFileItem, CommitItem, ComparisonTarget, Diagnostics, DiagnosticsSnapshot,
    DiffHunk, DiffLine, DiffLineKind, DiffModel, DiffPresentation, FileStatus, FileStatusKind,
    GitCommand, GitCommandRequest, HeadKind, MergeState, PatchApplicationMode, ReflogItem,
    RemoteSummary, RepoDetail, RepoId, RepoSummary, SelectedHunk, StashItem, Timestamp,
    WatcherFreshness, WorktreeItem,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub repo_id: RepoId,
    pub comparison_target: Option<ComparisonTarget>,
    pub selected_path: Option<PathBuf>,
    pub diff_presentation: DiffPresentation,
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
            request.selected_path.or(status.first_path.clone()),
            request.diff_presentation,
        )?;
        let commits = read_commits(&repo_path);
        let comparison_target = commits
            .first()
            .map(|commit| ComparisonTarget::Commit(commit.oid.clone()));

        Ok(RepoDetail {
            file_tree: status.file_tree,
            diff,
            branches: read_branches(&repo_path),
            commits,
            stashes: read_stashes(&repo_path),
            reflog_items: read_reflog(&repo_path),
            worktrees: read_worktrees(&repo_path),
            commit_input: String::new(),
            merge_state: read_merge_state(&repo_path),
            comparison_target,
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
            selected_path,
            request.diff_presentation,
        )
    }

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        let repo_path = repo_path(&request.repo_id)?;
        let summary = match &request.command {
            GitCommand::StageSelection => {
                git(&repo_path, ["add", "."])?;
                "Staged current selection".to_string()
            }
            GitCommand::StageFile { path } => {
                git_path(&repo_path, ["add"], path)?;
                format!("Staged {}", path.display())
            }
            GitCommand::UnstageFile { path } => {
                unstage_path(&repo_path, path)?;
                format!("Unstaged {}", path.display())
            }
            GitCommand::CommitStaged { message } => {
                git(&repo_path, ["commit", "-m", message.as_str()])?;
                format!("Committed staged changes: {message}")
            }
            GitCommand::AmendHead { message } => {
                match message.as_deref() {
                    Some(message) => git(&repo_path, ["commit", "--amend", "-m", message])?,
                    None => git(&repo_path, ["commit", "--amend", "--no-edit"])?,
                }
                "Amended HEAD commit".to_string()
            }
            GitCommand::CheckoutBranch { branch_ref } => {
                git(&repo_path, ["checkout", branch_ref.as_str()])?;
                format!("Checked out {branch_ref}")
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
        GitCommand::StageFile { .. } => "stage_file",
        GitCommand::UnstageFile { .. } => "unstage_file",
        GitCommand::CommitStaged { .. } => "commit_staged",
        GitCommand::AmendHead { .. } => "amend_head",
        GitCommand::CheckoutBranch { .. } => "checkout_branch",
        GitCommand::FetchSelectedRepo => "fetch_selected_repo",
        GitCommand::PullCurrentBranch => "pull_current_branch",
        GitCommand::PushCurrentBranch => "push_current_branch",
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
        let Some(hunk) = parsed
            .hunks
            .iter()
            .find(|hunk| hunk.selection == *selection)
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
        selected_hunks.push(hunk.raw.clone());
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
    selected_path: Option<PathBuf>,
    diff_presentation: DiffPresentation,
) -> GitResult<DiffModel> {
    let diff_text = read_diff_text(
        repo_path,
        comparison_target,
        selected_path.as_deref(),
        diff_presentation,
    )?;
    Ok(parse_diff_model(
        selected_path,
        diff_presentation,
        &diff_text,
    ))
}

fn read_diff_text(
    repo_path: &Path,
    comparison_target: Option<&ComparisonTarget>,
    selected_path: Option<&Path>,
    diff_presentation: DiffPresentation,
) -> GitResult<String> {
    let mut args = vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--binary".to_string(),
        "--unified=3".to_string(),
    ];

    if let Some(target) = comparison_target {
        args.push(match target {
            ComparisonTarget::Branch(branch) | ComparisonTarget::Commit(branch) => branch.clone(),
        });
    } else if matches!(diff_presentation, DiffPresentation::Staged) {
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
    Command::new("git")
        .args(args)
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
    git_stdout(repo_path, ["branch", "--all", "--no-color"])
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.contains(" -> ") {
                        return None;
                    }
                    Some(BranchItem {
                        name: trimmed.trim_start_matches('*').trim_start().to_string(),
                        is_head: trimmed.starts_with('*'),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn read_commits(repo_path: &Path) -> Vec<CommitItem> {
    git_stdout(repo_path, ["log", "--format=%H%x00%s", "-n", "64"])
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
                .collect()
        })
        .unwrap_or_default()
}

fn read_commit_files(repo_path: &Path, oid: &str) -> Vec<CommitFileItem> {
    git_stdout(
        repo_path,
        ["show", "--format=", "--name-status", "--no-renames", oid],
    )
    .map(|output| {
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
    })
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
                    let label = line.split_once('\0').map_or_else(
                        || line.to_string(),
                        |(name, summary)| format!("{name}: {summary}"),
                    );
                    StashItem { label }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn read_reflog(repo_path: &Path) -> Vec<ReflogItem> {
    git_stdout(repo_path, ["reflog", "--format=%gD%x00%gs", "-n", "64"])
        .map(|output| {
            output
                .lines()
                .map(|line| {
                    let description = line.split_once('\0').map_or_else(
                        || line.to_string(),
                        |(name, summary)| format!("{name}: {summary}"),
                    );
                    ReflogItem { description }
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

fn read_merge_state(repo_path: &Path) -> MergeState {
    if git_path_exists(repo_path, "MERGE_HEAD") {
        MergeState::MergeInProgress
    } else if git_path_exists(repo_path, "rebase-merge")
        || git_path_exists(repo_path, "rebase-apply")
    {
        MergeState::RebaseInProgress
    } else {
        MergeState::None
    }
}

fn git_path_exists(repo_path: &Path, git_path: &str) -> bool {
    git_stdout(repo_path, ["rev-parse", "--git-path", git_path])
        .map(PathBuf::from)
        .is_ok_and(|path| {
            if path.is_absolute() {
                path.exists()
            } else {
                repo_path.join(path).exists()
            }
        })
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
    use super_lazygit_core::{DiffModel, GitCommand, GitCommandRequest, RepoId};
    use super_lazygit_test_support::{
        clean_repo, conflicted_repo, detached_head_repo, dirty_repo, history_preview_repo,
        rebase_in_progress_repo, staged_and_unstaged_repo, stashed_repo, temp_repo,
        upstream_diverged_repo, worktree_repo,
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
            })
            .expect("detail should load");

        assert!(!detail.branches.is_empty());
        assert!(!detail.commits.is_empty());
        assert!(!detail.stashes.is_empty());
        assert!(!detail.reflog_items.is_empty());
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
            })
            .expect("detail should load");

        assert_eq!(detail.commits[0].summary, "add lib");
        assert_eq!(detail.commits[1].summary, "second");
        assert_eq!(
            detail.comparison_target,
            Some(ComparisonTarget::Commit(detail.commits[0].oid.clone()))
        );
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
            })
            .expect("detail should load");
        let diff = backend
            .read_diff(DiffRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                comparison_target: None,
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
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
    fn cli_backend_reads_rebase_in_progress_state() {
        let repo = rebase_in_progress_repo().expect("fixture repo");
        let backend = CliGitBackend;

        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
            })
            .expect("detail should load");

        assert_eq!(detail.merge_state, MergeState::RebaseInProgress);
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
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
}
