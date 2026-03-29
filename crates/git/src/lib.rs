use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super_lazygit_core::{
    BranchItem, CommitItem, ComparisonTarget, Diagnostics, DiagnosticsSnapshot, DiffModel,
    FileStatus, FileStatusKind, GitCommand, GitCommandRequest, HeadKind, MergeState, ReflogItem,
    RemoteSummary, RepoDetail, RepoId, RepoSummary, StashItem, Timestamp, WatcherFreshness,
    WorktreeItem,
};
use thiserror::Error;

pub trait GitBackend: Send + Sync + 'static {
    fn kind(&self) -> GitBackendKind;

    fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult>;

    fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary>;

    fn read_repo_detail(&self, request: RepoDetailRequest) -> GitResult<RepoDetail>;

    fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel>;

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome>;
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
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = self.backend.scan_workspace(request);
        self.finish_operation(operation, route, started_at, &result);
        result
    }

    pub fn read_repo_summary(&mut self, request: RepoSummaryRequest) -> GitResult<RepoSummary> {
        let operation = GitOperationKind::ReadRepoSummary;
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = self.backend.read_repo_summary(request);
        self.finish_operation(operation, route, started_at, &result);
        result
    }

    pub fn read_repo_detail(&mut self, request: RepoDetailRequest) -> GitResult<RepoDetail> {
        let operation = GitOperationKind::ReadRepoDetail;
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = self.backend.read_repo_detail(request);
        self.finish_operation(operation, route, started_at, &result);
        result
    }

    pub fn read_diff(&mut self, request: DiffRequest) -> GitResult<DiffModel> {
        let operation = GitOperationKind::ReadDiff;
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = self.backend.read_diff(request);
        self.finish_operation(operation, route, started_at, &result);
        result
    }

    pub fn run_command(&mut self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        let operation = GitOperationKind::WriteCommand;
        let route = self.route_for(operation);
        let started_at = Instant::now();
        let result = self.backend.run_command(request);
        self.finish_operation(operation, route, started_at, &result);
        result
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub repo_id: RepoId,
    pub comparison_target: Option<ComparisonTarget>,
    pub selected_path: Option<PathBuf>,
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

        Ok(RepoDetail {
            file_tree: status.file_tree,
            diff: DiffModel {
                selected_path: status.first_path,
            },
            branches: read_branches(&repo_path),
            commits: read_commits(&repo_path),
            stashes: read_stashes(&repo_path),
            reflog_items: read_reflog(&repo_path),
            worktrees: read_worktrees(&repo_path),
            commit_input: String::new(),
            merge_state: read_merge_state(&repo_path),
            comparison_target: None,
        })
    }

    fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel> {
        let repo_path = repo_path(&request.repo_id)?;
        let selected_path = match request.selected_path {
            Some(path) => Some(path),
            None => read_status_snapshot(&repo_path)?.first_path,
        };

        Ok(DiffModel { selected_path })
    }

    fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
        let repo_path = repo_path(&request.repo_id)?;
        let summary = match &request.command {
            GitCommand::StageSelection => {
                git(&repo_path, ["add", "."])?;
                "Staged current selection".to_string()
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
    if is_git_repo(root) {
        repos.push(root.to_path_buf());
        return Ok(());
    }

    let entries = fs::read_dir(root).map_err(io_error)?;
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if !entry.file_type().map_err(io_error)?.is_dir() {
            continue;
        }
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }
        collect_git_repos(&path, repos)?;
    }
    Ok(())
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

fn tracking_remote(repo_path: &Path) -> GitResult<(Option<String>, Option<String>)> {
    let upstream = git_stdout_allow_failure(
        repo_path,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )?;
    if upstream.is_empty() {
        return Ok((None, None));
    }

    let remote_name = upstream.split('/').next().map(str::to_owned);
    Ok((remote_name, Some(upstream)))
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

fn stdout_string(output: Output) -> GitResult<String> {
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| GitError::OperationFailed {
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

fn io_error(error: std::io::Error) -> GitError {
    GitError::OperationFailed {
        message: error.to_string(),
    }
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
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
        .map_or((header, None), |(left, right)| (left, Some(right.trim_end_matches(']'))));

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
    let path = raw
        .trim()
        .split(" -> ")
        .next_back()
        .unwrap_or(raw.trim())
        .trim_matches('"');
    PathBuf::from(path)
}

fn status_kind(staged: char, unstaged: char) -> FileStatusKind {
    if is_conflict_code(staged, unstaged) {
        return FileStatusKind::Conflicted;
    }

    let code = if staged != ' ' && staged != '?' {
        staged
    } else {
        unstaged
    };

    match code {
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        '?' => FileStatusKind::Untracked,
        _ => FileStatusKind::Modified,
    }
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
                    Some(CommitItem {
                        oid: oid.to_string(),
                        summary: summary.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
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
    } else if git_path_exists(repo_path, "rebase-merge") || git_path_exists(repo_path, "rebase-apply") {
        MergeState::RebaseInProgress
    } else {
        MergeState::None
    }
}

fn git_path_exists(repo_path: &Path, git_path: &str) -> bool {
    git_stdout(repo_path, ["rev-parse", "--git-path", git_path])
        .map(PathBuf::from)
        .is_ok_and(|path| path.exists())
}

fn is_conflict_code(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_lazygit_core::{DiffModel, GitCommand, GitCommandRequest, RepoId};
    use super_lazygit_test_support::{
        clean_repo, conflicted_repo, staged_and_unstaged_repo, upstream_diverged_repo,
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
            })
        }

        fn run_command(&self, request: GitCommandRequest) -> GitResult<GitCommandOutcome> {
            let summary = git_command_label(&request).to_string();
            Ok(GitCommandOutcome {
                repo_id: request.repo_id,
                summary,
            })
        }
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
    fn facade_delegates_and_records_backend_route() {
        let mut facade = GitFacade::with_routing(
            StubBackend {
                kind: GitBackendKind::Git2,
            },
            GitBackendRoutingPolicy {
                primary_backend: GitBackendKind::Git2,
                ..GitBackendRoutingPolicy::default()
            },
        );

        let summary = facade
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-1"),
                repo_id: RepoId::new("repo-a"),
                command: GitCommand::PushCurrentBranch,
            })
            .expect("stub command should succeed");

        assert_eq!(summary.summary, "push_current_branch");
        assert_eq!(facade.diagnostics().git_operations.len(), 1);
        assert!(facade.diagnostics().git_operations[0]
            .operation
            .contains("write_command via git-cli"));
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
            vec![RepoId::new(repo.path().display().to_string())]
        );
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
}
